//! Load panel (M4.W2): an MO2 profile or a plugin list into the session through `load`.
//! The profile picker only lists the folders of an MO2 `profiles` directory; nothing is
//! written anywhere.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::envelope::{list, num, strings, text, text_or_empty};
use crate::gate::Request;

/// The owner's live-game profile (AGENTS.md): read-only input, never a test target.
pub const LIVE_PROFILE: &str = "Default";

/// The panel's inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadForm {
    /// MO2 profile directory.
    pub profile: String,
    /// Plugins in load order, one per line (used when `profile` is empty).
    pub plugins: String,
    /// Game Data dir (empty: the engine's default).
    pub data_dir: String,
    /// Index from the files instead of the cache.
    pub no_cache: bool,
}

impl LoadForm {
    /// The plugin lines, trimmed, blanks dropped.
    pub fn plugin_list(&self) -> Vec<String> {
        self.plugins
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// The `load` request, or why there is none.
    pub fn request(&self) -> Result<Request, String> {
        let mut args = Map::new();
        let profile = self.profile.trim();
        let plugins = self.plugin_list();
        if !profile.is_empty() {
            args.insert("profile".into(), json!(profile));
        } else if !plugins.is_empty() {
            args.insert("plugins".into(), json!(plugins));
        } else {
            return Err("give an MO2 profile or at least one plugin".to_string());
        }
        if !self.data_dir.trim().is_empty() {
            args.insert("data_dir".into(), json!(self.data_dir.trim()));
        }
        if self.no_cache {
            args.insert("no_cache".into(), json!(true));
        }
        Ok(Request::new("load", Value::Object(args)))
    }
}

/// One plugin of the loaded order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRow {
    /// Load index.
    pub index: u64,
    /// File name.
    pub name: String,
    /// Full path the load order read it from (empty when not reported).
    pub path: String,
    /// `full`, `medium`, `small`, or `unknown` (the engine could not tell).
    pub scale: String,
    /// `master, localized`.
    pub flags: String,
    /// The rule that made it active.
    pub active: String,
    /// `mod:<name>`, `data`, ...
    pub provider: String,
    /// Records indexed (`None`: not reported).
    pub records: Option<u64>,
    /// Runtime slot (assumed).
    pub runtime_slot: String,
    /// `hit`, `built`, `uncached`.
    pub cache: String,
    /// Masters absent from the load order.
    pub missing_masters: Vec<String>,
    /// Masters loading after this plugin.
    pub late_masters: Vec<String>,
    /// Parse error, if the plugin failed to load.
    pub error: Option<String>,
}

/// The `load` result as rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadView {
    /// `profile:<dir>` or `plugins`.
    pub source: String,
    /// The plugins in load order.
    pub plugins: Vec<PluginRow>,
    /// Listed by the profile but inactive.
    pub inactive: Vec<String>,
    /// Active per the profile but with no file.
    pub missing_active: Vec<String>,
}

impl LoadView {
    /// Reads a `load` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let plugins = list(data, "plugins")
            .iter()
            .map(|p| PluginRow {
                index: num(p, "index").unwrap_or(0),
                name: text_or_empty(p, "name"),
                path: text_or_empty(p, "path"),
                scale: text(p, "scale").unwrap_or_else(|| "unknown".to_string()),
                flags: strings(p, "flag_names").join(", "),
                active: text_or_empty(p, "active"),
                provider: text_or_empty(p, "provider"),
                records: num(p, "records"),
                runtime_slot: text_or_empty(p, "runtime_slot"),
                cache: text_or_empty(p, "cache"),
                missing_masters: names(p, "missing_masters"),
                late_masters: names(p, "late_masters"),
                error: text(p, "error"),
            })
            .collect();
        Self {
            source: text_or_empty(data, "source"),
            plugins,
            inactive: names(data, "inactive"),
            missing_active: names(data, "missing_active_plugins"),
        }
    }

    /// The scale of `plugin` in this load order (ASCII case-insensitive), `unknown` when
    /// the plugin is not loaded or its scale could not be read.
    pub fn scale_of(&self, plugin: &str) -> String {
        self.plugins
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(plugin))
            .map_or_else(|| "unknown".to_string(), |p| p.scale.clone())
    }

    /// Plugin names in load order.
    pub fn names(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.name.clone()).collect()
    }

    /// `(name, path)` in load order. File arguments (`coverage`, `diff`, `validate`,
    /// `roundtrip`) get the path, so the `kit` line shown under a result also works in a
    /// fresh process: a bare name of a plugin in an MO2 mod folder resolves only in a
    /// session's load order. The name stands in when no path was reported.
    pub fn files(&self) -> Vec<(String, String)> {
        self.plugins
            .iter()
            .map(|p| (p.name.clone(), self.path_of(&p.name)))
            .collect()
    }

    /// The path of `plugin` (by name, ASCII case-insensitive), else the name as given.
    pub fn path_of(&self, plugin: &str) -> String {
        self.plugins
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(plugin) && !p.path.is_empty())
            .map_or_else(|| plugin.to_string(), |p| p.path.clone())
    }
}

/// A list member that is a string or an object with `name`.
fn names(v: &Value, key: &str) -> Vec<String> {
    list(v, key)
        .iter()
        .filter_map(|item| {
            item.as_str()
                .map(str::to_string)
                .or_else(|| text(item, "name"))
        })
        .collect()
}

/// One profile folder of an MO2 instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEntry {
    /// Folder name.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
    /// The owner's live-game profile: loading it reads it, nothing more.
    pub live: bool,
}

/// The profile folders under `profiles_dir` (an MO2 instance's `profiles`), sorted.
/// Read-only: lists directory names, opens nothing.
pub fn list_profiles(profiles_dir: &Path) -> Vec<ProfileEntry> {
    let Ok(entries) = std::fs::read_dir(profiles_dir) else {
        return Vec::new();
    };
    let mut out: Vec<ProfileEntry> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some(ProfileEntry {
                live: name == LIVE_PROFILE,
                name,
                path,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, TextEdit};

use crate::panels::common::{
    action_bar, envelope_block, panel_head, virtual_table, Action, Slot, TabCtx,
};
use crate::theme::{self, Field, Status, View};

/// The Load tab's state: the form, the profiles folder and its listing, and the last
/// `load` result (what the session holds).
#[derive(Default)]
pub(crate) struct LoadTab {
    pub form: LoadForm,
    pub profiles_dir: String,
    pub profiles: Vec<ProfileEntry>,
    pub view: Option<LoadView>,
}

impl LoadTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let actions = &mut ctx.actions;
        panel_head(
            ui,
            "Load",
            "Loads a profile or plugin list into the session (`load`). Profiles are read, never changed; `Default` is the owner's live game and is only ever read.",
        );
        action_bar(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("MO2 profiles folder");
                ui.add(
                    TextEdit::singleline(&mut self.profiles_dir)
                        .desired_width(theme::width(ui, Field::Long)),
                );
                if ui.button("List").clicked() {
                    self.profiles = list_profiles(Path::new(self.profiles_dir.trim()));
                }
            });
            for p in &self.profiles {
                ui.horizontal(|ui| {
                    if ui.link(&p.name).clicked() {
                        actions.push(Action::UseProfile(p.path.clone()));
                    }
                    if p.live {
                        theme::status(
                            ui,
                            Status::Warn,
                            "owner's live game: loading it only reads it",
                        );
                    }
                });
            }
            let wide = theme::width(ui, Field::Wide);
            theme::form_grid(ui, "load-form").show(ui, |ui| {
                ui.label("Profile");
                ui.add(TextEdit::singleline(&mut self.form.profile).desired_width(wide));
                ui.end_row();
                ui.label("or plugins (one per line)");
                ui.add(
                    TextEdit::multiline(&mut self.form.plugins)
                        .desired_rows(3)
                        .desired_width(wide),
                );
                ui.end_row();
                ui.label("Data dir (optional)");
                ui.add(TextEdit::singleline(&mut self.form.data_dir).desired_width(wide));
                ui.end_row();
            });
            ui.checkbox(
                &mut self.form.no_cache,
                "no cache (index every plugin from its file)",
            );
            if ui.add_enabled(!busy, egui::Button::new("Load")).clicked() {
                match self.form.request() {
                    Ok(req) => actions.push(Action::Submit(Slot::Load, req)),
                    Err(e) => *ctx.notice = Some(e),
                }
            }
        });
        envelope_block(ui, ctx.results.get(&Slot::Load.into()), |ui| {
            let Some(view) = &self.view else { return };
            for (label, list) in [
                ("Inactive", &view.inactive),
                ("Active but missing", &view.missing_active),
            ] {
                if !list.is_empty() {
                    theme::status_line(ui, Status::Warn, &format!("{label}: {}", list.join(", ")));
                }
            }
            virtual_table(
                ui,
                "load-grid",
                &[
                    "#", "plugin", "scale", "flags", "records", "slot", "active", "provider",
                    "cache", "problems",
                ],
                view.plugins.len(),
                View::Tall,
                |row, i| {
                    let Some(p) = view.plugins.get(i) else { return };
                    row.num(p.index.to_string());
                    row.cell(|ui| ui.label(&p.name));
                    row.cell(|ui| {
                        if p.scale == "unknown" {
                            theme::status(ui, Status::Unknown, "unknown")
                        } else {
                            ui.label(&p.scale)
                        }
                    });
                    row.cell(|ui| ui.label(&p.flags));
                    row.num(p.records.map_or_else(|| "?".to_string(), |n| n.to_string()));
                    row.cell(|ui| ui.label(&p.runtime_slot));
                    row.cell(|ui| ui.label(&p.active));
                    row.cell(|ui| ui.label(&p.provider));
                    row.cell(|ui| ui.label(&p.cache));
                    let mut problems = Vec::new();
                    if !p.missing_masters.is_empty() {
                        problems.push(format!("missing masters: {}", p.missing_masters.join(", ")));
                    }
                    if !p.late_masters.is_empty() {
                        problems.push(format!("late masters: {}", p.late_masters.join(", ")));
                    }
                    if let Some(e) = &p.error {
                        problems.push(e.clone());
                    }
                    row.cell(|ui| {
                        if problems.is_empty() {
                            ui.label("")
                        } else {
                            theme::status(ui, Status::Fail, &problems.join("; "))
                        }
                    });
                },
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_wins_over_plugins_and_blank_fields_are_left_out() {
        let form = LoadForm {
            profile: r" C:\MO2\profiles\KitKraft Dev ".into(),
            plugins: "A.esm\n".into(),
            ..LoadForm::default()
        };
        let req = form.request().unwrap();
        assert_eq!(req.id, "load");
        assert_eq!(
            req.args,
            json!({"profile": r"C:\MO2\profiles\KitKraft Dev"})
        );
    }

    #[test]
    fn plugin_lines_become_the_plugins_list() {
        let form = LoadForm {
            plugins: "A.esm\n\n  B.esp  \n".into(),
            data_dir: r"D:\Data".into(),
            no_cache: true,
            ..LoadForm::default()
        };
        assert_eq!(
            form.request().unwrap().args,
            json!({"plugins": ["A.esm", "B.esp"], "data_dir": r"D:\Data", "no_cache": true})
        );
        assert!(LoadForm::default().request().is_err());
    }

    #[test]
    fn rows_from_a_canned_envelope_keep_unknown_scale_unknown() {
        let env = json!({"command": "load", "data": {
        "source": "plugins",
        "inactive": [],
        "missing_active_plugins": ["Gone.esp"],
        "plugins": [
            {"index": 0, "name": "Base.esm", "scale": "full", "flag_names": ["master"], "active": "listed",
             "provider": "path", "records": 10, "runtime_slot": "00", "cache": "built",
             "missing_masters": [], "late_masters": [], "error": null},
            {"index": 1, "name": "Patch.esp", "path": r"C:\Data\Patch.esp", "scale": null, "flag_names": [], "active": "listed",
             "provider": "path", "records": 2, "runtime_slot": "01", "cache": "hit",
             "missing_masters": ["Lost.esm"], "late_masters": [], "error": null}
        ]}});
        let view = LoadView::from_envelope(&env);
        assert_eq!(view.plugins.len(), 2);
        assert_eq!(view.plugins[0].flags, "master");
        assert_eq!(view.plugins[1].scale, "unknown");
        assert_eq!(view.plugins[1].missing_masters, vec!["Lost.esm"]);
        assert_eq!(view.missing_active, vec!["Gone.esp"]);
        assert_eq!(view.scale_of("base.ESM"), "full");
        assert_eq!(view.scale_of("Lost.esm"), "unknown");
        assert_eq!(view.path_of("patch.ESP"), r"C:\Data\Patch.esp");
        assert_eq!(view.path_of("Base.esm"), "Base.esm");
        assert_eq!(view.files()[1].1, r"C:\Data\Patch.esp");
    }

    #[test]
    fn profiles_are_listed_and_the_live_one_is_marked() {
        let dir = std::env::temp_dir().join(format!("kitkraft-ui-profiles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Default")).unwrap();
        std::fs::create_dir_all(dir.join("KitKraft Dev")).unwrap();
        std::fs::write(dir.join("not-a-profile.txt"), b"x").unwrap();
        let found = list_profiles(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["Default", "KitKraft Dev"]);
        assert!(found[0].live);
        assert!(!found[1].live);
        assert!(list_profiles(Path::new("Z:/no/such/dir")).is_empty());
    }
}
