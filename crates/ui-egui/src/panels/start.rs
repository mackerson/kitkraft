//! Start panel (first run): the MO2 instance the engine's `profiles` command finds, its
//! profiles with the owner's live game marked, the Data folder a profile load would use,
//! and one load button per profile. Everything shown comes from the `profiles` envelope;
//! the only thing the panel adds is the last choice, from the UI prefs file.

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::envelope::{list, text, text_or_empty};
use crate::gate::Request;
use crate::panels::load::LoadForm;

/// What the start screen says next to the owner's live-game profile.
pub const LIVE_BADGE: &str = "owner's live game, read-only";
/// What it says next to the dev profile.
pub const DEV_BADGE: &str = "in-game checks run here";

/// The `profiles` request: the instance given (from the prefs, or the launcher's
/// profile), else the engine looks for one.
pub fn profiles_request(instance: Option<&str>) -> Request {
    let mut args = Map::new();
    if let Some(i) = instance.map(str::trim).filter(|i| !i.is_empty()) {
        args.insert("instance".into(), json!(i));
    }
    Request::new("profiles", Value::Object(args))
}

/// The instance root a profile folder belongs to (`<root>\profiles\<name>`), when the
/// folder sits in a `profiles` folder.
pub fn instance_of_profile(profile: &str) -> Option<String> {
    let profiles = Path::new(profile.trim()).parent()?;
    profiles
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("profiles"))
        .then(|| profiles.parent().map(|r| r.display().to_string()))
        .flatten()
}

/// One profile of the instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartProfile {
    /// Folder name.
    pub name: String,
    /// Full path (what `load` gets).
    pub path: String,
    /// The owner's live game (`Default`).
    pub live: bool,
    /// The dev profile (`KitKraft Dev`).
    pub dev: bool,
    /// MO2's selected profile.
    pub selected: bool,
    /// Has a `plugins.txt`.
    pub plugins_txt: bool,
}

impl StartProfile {
    /// The marker shown beside it.
    pub fn badge(&self) -> Option<&'static str> {
        if self.live {
            Some(LIVE_BADGE)
        } else if self.dev {
            Some(DEV_BADGE)
        } else {
            None
        }
    }

    /// The `load` request its button sends (read-only for every profile).
    pub fn load_request(&self) -> Result<Request, String> {
        LoadForm {
            profile: self.path.clone(),
            ..LoadForm::default()
        }
        .request()
    }
}

/// The `profiles` result as the start screen shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartView {
    /// Instance root, when one was found.
    pub instance: Option<String>,
    /// How it was found (`argument`, `KITKRAFT_MO2_DIR`, ...).
    pub found_by: String,
    /// `gameName` of the instance.
    pub game_name: String,
    /// MO2's selected profile.
    pub selected_profile: String,
    /// `(path, source, found)` of every place searched.
    pub searched: Vec<(String, String, bool)>,
    /// The Data-dir line.
    pub data_dir: String,
    /// Whether that folder holds `Starfield.esm`.
    pub data_ok: bool,
    /// The profiles, sorted by name.
    pub profiles: Vec<StartProfile>,
}

impl StartView {
    /// Reads a `profiles` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let inst = data.get("instance").unwrap_or(&Value::Null);
        let flag = |v: &Value, k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        let dd = data.get("data_dir").unwrap_or(&Value::Null);
        let data_ok = flag(dd, "starfield_esm");
        let data_dir = match text(dd, "path") {
            Some(p) => {
                let source = text(dd, "source").unwrap_or_else(|| "?".to_string());
                let state = if data_ok {
                    "Starfield.esm found"
                } else if flag(dd, "exists") {
                    "no Starfield.esm there"
                } else {
                    "folder missing"
                };
                format!("{p} (from {source}; {state})")
            }
            None => {
                "unknown: the instance has no gamePath and KITKRAFT_DATA_DIR is not set".to_string()
            }
        };
        Self {
            instance: text(inst, "root"),
            found_by: text_or_empty(inst, "found_by"),
            game_name: text_or_empty(inst, "game_name"),
            selected_profile: text_or_empty(inst, "selected_profile"),
            searched: list(data, "searched")
                .iter()
                .map(|s| {
                    (
                        text_or_empty(s, "path"),
                        text_or_empty(s, "source"),
                        flag(s, "found"),
                    )
                })
                .collect(),
            data_dir,
            data_ok,
            profiles: list(data, "profiles")
                .iter()
                .map(|p| StartProfile {
                    name: text_or_empty(p, "name"),
                    path: text_or_empty(p, "path"),
                    live: flag(p, "live"),
                    dev: flag(p, "dev"),
                    selected: flag(p, "selected"),
                    plugins_txt: flag(p, "plugins_txt"),
                })
                .collect(),
        }
    }

    /// Whether `profile` (a path from the prefs) is this profile, ignoring case.
    pub fn is_last(profile: &StartProfile, last: Option<&str>) -> bool {
        last.is_some_and(|l| l.trim().eq_ignore_ascii_case(profile.path.trim()))
    }
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, CollapsingHeader, ComboBox, TextEdit};

use crate::envelope::BuildInfo;
use crate::panels::common::{
    action_bar, build_of, envelope_block, panel_head, Action, Slot, TabCtx,
};
use crate::theme::{self, Field, Mode, Preset, Status, ThemeChoice};

/// The Start tab's state: the instance field and the last `profiles` result.
#[derive(Default)]
pub(crate) struct StartTab {
    pub instance: String,
    pub view: Option<StartView>,
}

impl StartTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        panel_head(
            ui,
            "Start",
            "The MO2 instance, its profiles and the Data folder, as the engine's `profiles` command reads them. Each button loads a profile read-only; `Default` is the owner's live game and is only ever read. In-game checks run on `KitKraft Dev`.",
        );
        about_line(ui, build_of(ctx.results));
        theme_line(ui, ctx);
        theme::gap(ui);
        let actions = &mut ctx.actions;
        action_bar(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("MO2 instance");
                ui.add(
                    TextEdit::singleline(&mut self.instance)
                        .desired_width(theme::width(ui, Field::Long)),
                )
                .on_hover_text(
                    "the folder holding ModOrganizer.ini; empty: the engine looks for one",
                );
                if ui
                    .add_enabled(!busy, egui::Button::new("Look again"))
                    .clicked()
                {
                    actions.push(Action::Submit(
                        Slot::Profiles,
                        profiles_request(Some(&self.instance)),
                    ));
                }
            })
        });
        let last = ctx.last_profile;
        let prefs_path = ctx.prefs_path;
        envelope_block(ui, ctx.results.get(&Slot::Profiles.into()), |ui| {
            let Some(view) = &self.view else { return };
            theme::table(ui, "start-instance")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.strong("MO2 instance");
                    match &view.instance {
                        Some(root) => {
                            ui.label(format!(
                                "{root}  (found by {}; game {}; MO2's selected profile {})",
                                view.found_by,
                                if view.game_name.is_empty() {
                                    "not set"
                                } else {
                                    &view.game_name
                                },
                                if view.selected_profile.is_empty() {
                                    "not set"
                                } else {
                                    &view.selected_profile
                                }
                            ));
                        }
                        None => {
                            theme::status(
                                ui,
                                Status::Fail,
                                "none found: type the folder holding ModOrganizer.ini above",
                            );
                        }
                    }
                    ui.end_row();
                    ui.strong("Data dir");
                    if view.data_ok {
                        ui.label(&view.data_dir);
                    } else {
                        theme::status(ui, Status::Warn, &view.data_dir);
                    }
                    ui.end_row();
                });
            CollapsingHeader::new(format!("places searched ({})", view.searched.len()))
                .id_salt("start-searched")
                .show(ui, |ui| {
                    for (path, source, found) in &view.searched {
                        ui.label(format!(
                            "{} {path} ({source})",
                            if *found { "found:" } else { "no instance:" }
                        ));
                    }
                });
            theme::gap(ui);
            if view.profiles.is_empty() && view.instance.is_some() {
                theme::status(
                    ui,
                    Status::Warn,
                    "the instance has no profiles (folders with a modlist.txt)",
                );
            }
            theme::table(ui, "start-profiles").show(ui, |ui| {
                for p in &view.profiles {
                    let label = if p.live {
                        format!("Load {} (read-only)", p.name)
                    } else {
                        format!("Load {}", p.name)
                    };
                    if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                        actions.push(Action::LoadProfile(p.clone()));
                    }
                    match p.badge() {
                        Some(b) if p.live => {
                            theme::status(ui, Status::Warn, b);
                        }
                        Some(b) => {
                            theme::status(ui, Status::Pass, b);
                        }
                        None => {
                            ui.label("");
                        }
                    }
                    let mut notes = Vec::new();
                    if p.selected {
                        notes.push("MO2's selected profile");
                    }
                    if StartView::is_last(p, last) {
                        notes.push("last used");
                    }
                    if !p.plugins_txt {
                        notes.push("no plugins.txt");
                    }
                    ui.weak(notes.join("; "));
                    ui.monospace(&p.path);
                    ui.end_row();
                }
            });
            match prefs_path {
                Some(path) => ui.weak(format!(
                    "the choice is remembered in {} (UI prefs, not game data; the only file the workbench writes on its own)",
                    path.display()
                )),
                None => ui.weak("no %LOCALAPPDATA%: the choice is not remembered"),
            };
        });
    }
}

/// The About line: which build this is, from the envelope's `build` member (version,
/// commit, date, profile; M6.P6b), with a warning on a debug build. Before the first
/// result it says the build is not known yet.
fn about_line(ui: &mut egui::Ui, build: Option<&BuildInfo>) {
    ui.horizontal_wrapped(|ui| {
        ui.strong("About");
        match build {
            Some(b) => {
                ui.monospace(format!("KitKraft {}", b.line()))
                    .on_hover_text("the `build` member of every result envelope; `kit --version` prints the same");
                if b.is_debug() {
                    theme::status(
                        ui,
                        Status::Warn,
                        "debug build: a profile load is about 5x slower; `cargo xtask ui` builds release",
                    );
                }
            }
            None => {
                ui.weak("build not known yet: every result envelope names it (`build`)");
            }
        }
    });
}

/// The preset's name in the picker (`Classic` is marked legacy).
fn preset_label(p: Preset) -> String {
    let t = p.theme();
    if p.legacy() {
        format!("{} (legacy)", t.name)
    } else if p == Preset::Workbench {
        format!("{} (default)", t.name)
    } else {
        t.name.to_string()
    }
}

/// The theme line: the active preset and mode, where the choice came from, a picker
/// (preset and light, dark or system; kept in `ui.json`), and the statuses as the preset
/// draws them.
fn theme_line(ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
    let current = ctx.theme;
    let active = current.preset.theme();
    let mut pick = current;
    ui.horizontal_wrapped(|ui| {
        ui.strong("Theme");
        ComboBox::from_id_salt("theme-preset")
            .selected_text(preset_label(current.preset))
            .show_ui(ui, |ui| {
                for p in Preset::ALL {
                    ui.selectable_value(&mut pick.preset, p, preset_label(p));
                }
            });
        for (mode, label) in [
            (Mode::System, "system"),
            (Mode::Light, "light"),
            (Mode::Dark, "dark"),
        ] {
            ui.radio_value(&mut pick.mode, mode, label);
        }
        ui.weak(format!(
            "from {}: {}. Also `kit ui --theme <name>`: {}",
            ctx.theme_source,
            active.summary,
            ThemeChoice::HELP
        ));
    });
    if pick != current {
        ctx.actions.push(Action::SetTheme(pick));
    }
    ui.horizontal_wrapped(|ui| {
        ui.weak("statuses:");
        for s in Status::ALL {
            theme::status(ui, s, "");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canned() -> Value {
        json!({"command": "profiles", "data": {
            "instance": {"root": r"C:\Modding\MO2-Starfield", "found_by": "dev box default",
                         "game_name": "Starfield", "game_dir": r"C:\Games\Starfield",
                         "profiles_dir": r"C:\Modding\MO2-Starfield\profiles",
                         "mods_dir": r"C:\Modding\MO2-Starfield\mods", "selected_profile": "Default"},
            "searched": [{"path": r"C:\Modding\MO2-Starfield", "source": "dev box default", "found": true}],
            "profiles": [
                {"name": "Default", "path": r"C:\Modding\MO2-Starfield\profiles\Default", "live": true, "dev": false, "selected": true, "plugins_txt": true},
                {"name": "KitKraft Dev", "path": r"C:\Modding\MO2-Starfield\profiles\KitKraft Dev", "live": false, "dev": true, "selected": false, "plugins_txt": true}
            ],
            "data_dir": {"path": r"C:\Games\Starfield\Data", "source": "instance gamePath", "exists": true, "starfield_esm": true},
            "summary": "x"
        }, "coverage": {"status": "complete"}, "limitations": [], "evidence": []})
    }

    #[test]
    fn the_live_profile_is_marked_and_every_button_is_a_read_only_load() {
        let view = StartView::from_envelope(&canned());
        assert_eq!(view.instance.as_deref(), Some(r"C:\Modding\MO2-Starfield"));
        assert_eq!(view.profiles.len(), 2);
        assert_eq!(view.profiles[0].badge(), Some(LIVE_BADGE));
        assert_eq!(view.profiles[1].badge(), Some(DEV_BADGE));
        assert!(view.data_ok);
        assert_eq!(
            view.data_dir,
            r"C:\Games\Starfield\Data (from instance gamePath; Starfield.esm found)"
        );
        for p in &view.profiles {
            let req = p.load_request().unwrap();
            assert_eq!(req.id, "load");
            assert_eq!(req.args, json!({"profile": p.path}));
            crate::gate::check(&req).unwrap();
        }
        let last = Some(r"c:\modding\mo2-starfield\profiles\kitkraft dev");
        assert!(StartView::is_last(&view.profiles[1], last));
        assert!(!StartView::is_last(&view.profiles[0], last));
    }

    #[test]
    fn nothing_found_says_so() {
        let view = StartView::from_envelope(&json!({"data": {"instance": null, "searched": [],
            "profiles": [], "data_dir": {"path": null, "source": null, "exists": false, "starfield_esm": false}}}));
        assert!(view.instance.is_none());
        assert!(view.data_dir.starts_with("unknown"));
        assert!(!view.data_ok);
    }

    #[test]
    fn requests_and_instance_roots() {
        assert_eq!(profiles_request(None).args, json!({}));
        // Absolute samples on both platforms: `C:\...` on Windows (the original spelling),
        // `/...` elsewhere. Spaces at both ends of the instance: both must be trimmed.
        let base = if cfg!(windows) {
            std::path::PathBuf::from(r"C:\")
        } else {
            std::path::PathBuf::from("/")
        };
        let mo2 = base.join("MO2");
        assert_eq!(
            profiles_request(Some(&format!(" {} ", mo2.display()))).args,
            json!({"instance": mo2.display().to_string()})
        );
        crate::gate::check(&profiles_request(None)).unwrap();
        let root_path = base.join("Modding").join("MO2-Starfield");
        let profile = root_path
            .join("profiles")
            .join("KitKraft Dev")
            .display()
            .to_string();
        let root = instance_of_profile(&profile);
        assert_eq!(root.map(std::path::PathBuf::from), Some(root_path));
        let elsewhere = base
            .join("elsewhere")
            .join("KitKraft Dev")
            .display()
            .to_string();
        assert_eq!(instance_of_profile(&elsewhere), None);
    }

    /// The canned envelope is the engine's own shape: run `profiles` on a scratch
    /// instance through the gate and read it the same way.
    #[test]
    fn the_engine_envelope_reads_the_same_way() {
        let base = std::env::temp_dir().join(format!("kitkraft-ui-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mo2 = base.join("MO2");
        for p in ["Default", "KitKraft Dev"] {
            std::fs::create_dir_all(mo2.join("profiles").join(p)).unwrap();
            std::fs::write(mo2.join("profiles").join(p).join("modlist.txt"), "").unwrap();
        }
        std::fs::write(
            mo2.join("ModOrganizer.ini"),
            "[General]\ngameName=Starfield\n",
        )
        .unwrap();
        let engine = crate::gate::ReadOnlyEngine::new();
        let env = engine
            .run(&profiles_request(Some(&mo2.display().to_string())))
            .unwrap();
        let _ = std::fs::remove_dir_all(&base);
        let view = StartView::from_envelope(&env);
        let canned_keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v["data"].as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(canned_keys(&env), canned_keys(&canned()));
        assert_eq!(view.found_by, "argument");
        assert_eq!(view.profiles.len(), 2);
        assert_eq!(view.profiles[0].badge(), Some(LIVE_BADGE));
        assert!(!view.profiles[0].plugins_txt);
    }
}
