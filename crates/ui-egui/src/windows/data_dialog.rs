//! The Data dialog (M7.M2, pipeline W): choose the plugins to load and the active file,
//! over `load` and `inspect` of each plugin's `TES4`, as `docs/ck-parity.md` observes the
//! CK's (row "Data dialog", `ck-20261101-01` to `-03`).
//!
//! Observed and reproduced here: each plugin listed with a status, "Master File", "Plugin
//! File" or "Active File"; a double-click ticks a row; "Set as Active File" makes the
//! selected plugin the active file and ticks it; OK loads; a master-flagged plugin cannot
//! be the active file. Not observed (KitKraft's own until a sitting shows the CK's):
//! the details pane's layout and labels, the check boxes as boxes (the CK ticks by
//! double-click; how it draws a tick was not exposed), "No Active File", and the
//! type-ahead selection (typing a name selects its row in the CK; not built).
//!
//! The list is the session's load order (the Load tab's `load` result) and the plugins
//! its profile lists but does not activate (these cannot be ticked: the engine reports
//! no path for them). OK sends `load` with the ticked plugins when the ticks differ from
//! the load order, through `Slot::Load`, so the Load tab and the session follow; then
//! "Set as Active File"'s choice becomes `Action::SetActiveFile`, the file pipeline E's
//! writes go into (ADR 0007 Amendment 4). The details are `inspect` of the selected
//! plugin's first record (FormID 0, its `TES4`) in single-plugin mode.
//!
//! Refused as the active file, before anything is sent: a plugin the engine's `load`
//! result marks master-flagged (`flag_names` has `master`); an official master or
//! Creation, by the engine's activation rule (`hardcoded-official`, `Starfield.ccc`,
//! `blueprint-companion-of:`) or by name (the names ADR 0007 Amendment 4's gate check
//! lists); and anything while the session was loaded from the `Default` profile (the
//! owner's live game is read-only input, never a target: AGENTS.md). The engine's output
//! guard refuses the rest on its own (`edit::patch::guard_target`).

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::envelope::{list, num, text, text_or_empty};
use crate::gate::{Request, LIVE_PROFILE};
use crate::panels::common::Slot;
use crate::panels::load::LoadView;

/// What a Data dialog request sent through the shell's tickets was for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataTicket {
    /// The load order the dialog lists. Unused: the dialog's `load` goes through
    /// `Slot::Load`, so the session and the Load tab follow it.
    #[allow(dead_code)]
    Load,
    /// One plugin's details (its `TES4`): the plugin's path.
    Details(String),
}

/// One plugin row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRow {
    /// File name.
    pub name: String,
    /// The path the load order read it from (empty: the engine reported none).
    pub path: String,
    /// The engine's `load` reports the master flag (`flag_names` has `master`).
    pub master: bool,
    /// Why it is official (an engine activation rule, or its name), if it is.
    pub official: Option<String>,
    /// In the session's load order.
    pub loaded: bool,
    /// Ticked: OK loads it.
    pub ticked: bool,
}

/// A plugin's `TES4`, as `inspect` reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Details {
    /// The plugin's path (what the request named).
    pub path: String,
    /// `CNAM`.
    pub author: Option<String>,
    /// `SNAM`.
    pub description: Option<String>,
    /// Each `MAST`, in order.
    pub masters: Vec<String>,
    /// The record header's flags (`0x%08X`).
    pub flags: String,
    /// The record header's timestamp field, as stored (not decoded into a date).
    pub timestamp: Option<u64>,
    /// The engine's error, when there are no details.
    pub error: Option<String>,
}

impl Details {
    /// Reads an `inspect` envelope of `path`'s first record.
    pub fn from_envelope(path: &str, env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let record = data.get("record").unwrap_or(&Value::Null);
        let mut d = Details {
            path: path.to_string(),
            flags: text_or_empty(record, "flags"),
            timestamp: num(record, "timestamp"),
            ..Details::default()
        };
        if text(record, "signature").as_deref() != Some("TES4") {
            d.error = Some(format!(
                "{path}: the first record is not a TES4 header ({})",
                text(record, "signature").unwrap_or_else(|| "none".to_string())
            ));
            return d;
        }
        for s in list(data, "subrecords").iter().take(4096) {
            let value = || {
                let mut t = zstring(&text_or_empty(s, "payload_hex"));
                if s.get("payload_truncated").and_then(Value::as_bool) == Some(true) {
                    t.push_str(" (cut)");
                }
                t
            };
            match text(s, "signature").as_deref() {
                Some("CNAM") => d.author = Some(value()),
                Some("SNAM") => d.description = Some(value()),
                Some("MAST") => d.masters.push(value()),
                _ => {}
            }
        }
        d
    }
}

/// The text of a NUL-terminated string given as hex (up to the first NUL; a pair that is
/// not hex ends it).
fn zstring(hex: &str) -> String {
    let mut bytes = Vec::new();
    for pair in hex.as_bytes().chunks(2).take(8192) {
        let Some(b) = std::str::from_utf8(pair)
            .ok()
            .filter(|p| p.len() == 2)
            .and_then(|p| u8::from_str_radix(p, 16).ok())
        else {
            break;
        };
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// An official master or Creation by name: the names ADR 0007 Amendment 4's gate check
/// lists (`Starfield.esm`, `Constellation.esm`, `OldMars.esm`, `ShatteredSpace.esm`,
/// `BlueprintShips-*.esm`, `SFBGS*.esm`), ASCII case-insensitive.
pub fn is_official_name(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    [
        "starfield.esm",
        "constellation.esm",
        "oldmars.esm",
        "shatteredspace.esm",
    ]
    .contains(&n.as_str())
        || (n.ends_with(".esm") && (n.starts_with("blueprintships-") || n.starts_with("sfbgs")))
}

/// Why `name` is official, from the engine's activation rule (`load`'s `active`) or its
/// name; `None` for any other plugin.
fn official_reason(name: &str, active: &str) -> Option<String> {
    if active == "hardcoded-official" || active == "Starfield.ccc" {
        return Some(format!(
            "the engine's load order activates it as `{active}`"
        ));
    }
    if active.starts_with("blueprint-companion-of:") {
        return Some(format!("the engine's load order lists it as `{active}`"));
    }
    is_official_name(name).then(|| "its name is an official master's".to_string())
}

/// `profile:<dir>` names the owner's live profile (its last path component, read with
/// `Path::file_name` as `app.rs` does; KK-001).
fn live_source(source: &str) -> bool {
    source.strip_prefix("profile:").is_some_and(|dir| {
        Path::new(dir.trim())
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(LIVE_PROFILE))
    })
}

/// The Data dialog's state (kept in `crate::windows::Shell`).
#[derive(Debug, Default)]
pub struct DataDialogState {
    /// The rows were built for this opening (cleared when the window closes).
    pub initialized: bool,
    /// The load the rows were built from (`LoadTab::data`).
    pub generation: Option<u64>,
    /// The load's `source` (`profile:<dir>` or `plugins`).
    pub source: String,
    pub rows: Vec<DataRow>,
    /// The selected row.
    pub selected: Option<usize>,
    /// "Set as Active File"'s choice (a path), applied on OK.
    pub active: Option<String>,
    /// Why the last "Set as Active File" was refused.
    pub refusal: Option<String>,
    pub details: Option<Details>,
    /// The path whose details are on the way.
    pub details_pending: Option<String>,
}

/// What the Data dialog asks the shell for.
#[derive(Debug, PartialEq)]
pub enum DataAsk {
    /// The details of the selected plugin (its path; `inspect` of its `TES4`).
    Details(String, Request),
    /// OK: load the ticked plugins (when they differ from the load order), then set the
    /// active file.
    Accept {
        load: Option<Request>,
        active: Option<String>,
    },
    /// Cancel: nothing changes.
    Cancel,
}

impl DataDialogState {
    /// The window closed: the next opening starts from the session again.
    pub fn closed(&mut self) {
        self.initialized = false;
    }

    /// Builds the rows from the session's load order on opening and after each new
    /// load; on opening, the active file starts as the window's.
    pub fn sync(&mut self, load: Option<&LoadView>, generation: u64, active_file: Option<&str>) {
        let fresh = !self.initialized;
        if !fresh && self.generation == Some(generation) {
            return;
        }
        self.initialized = true;
        self.generation = Some(generation);
        self.selected = None;
        self.refusal = None;
        if fresh {
            self.active = active_file.map(str::to_string);
        }
        self.source = load.map(|l| l.source.clone()).unwrap_or_default();
        self.rows = load.map_or_else(Vec::new, |l| {
            let loaded = l.plugins.iter().map(|p| DataRow {
                name: p.name.clone(),
                path: p.path.clone(),
                master: p.flags.split(", ").any(|f| f == "master"),
                official: official_reason(&p.name, &p.active),
                loaded: true,
                ticked: true,
            });
            let inactive = l.inactive.iter().map(|n| DataRow {
                name: n.clone(),
                path: String::new(),
                master: false,
                official: official_reason(n, ""),
                loaded: false,
                ticked: false,
            });
            loaded.chain(inactive).take(65_536).collect()
        });
        // Review M1: an active file chosen under another load (the MO2 `mods` folder is
        // shared by every profile) is dropped when this load refuses it.
        if self.active.as_deref().is_some_and(|a| !self.active_ok(a)) {
            self.active = None;
        }
    }

    /// Whether `path` may stay the active file under this load: never while the session
    /// came from `Default`, and never when the load lists it on a refused row.
    fn active_ok(&self, path: &str) -> bool {
        !live_source(&self.source)
            && self
                .rows
                .iter()
                .position(|r| r.path == path)
                .is_none_or(|i| self.refused(i).is_none())
    }

    /// The CK's status of row `i`.
    pub fn status(&self, i: usize) -> &'static str {
        let Some(r) = self.rows.get(i) else {
            return "";
        };
        if !r.path.is_empty() && self.active.as_deref() == Some(r.path.as_str()) {
            "Active File"
        } else if r.master {
            "Master File"
        } else {
            "Plugin File"
        }
    }

    /// Selects row `i`; the details request when they are not shown or on the way.
    pub fn select(&mut self, i: usize) -> Option<DataAsk> {
        let path = self.rows.get(i)?.path.clone();
        self.selected = Some(i);
        self.refusal = None;
        if path.is_empty()
            || self.details_pending.as_deref() == Some(path.as_str())
            || self.details.as_ref().is_some_and(|d| d.path == path)
        {
            return None;
        }
        self.details_pending = Some(path.clone());
        let req = Request::new(
            "inspect",
            json!({"form": "0", "plugin": path, "max_bytes": 4096}),
        );
        Some(DataAsk::Details(path, req))
    }

    /// Ticks or unticks row `i` (a row the engine gave no path cannot be ticked).
    /// Nothing is ticked or unticked while the session came from `Default` (review M2).
    pub fn toggle(&mut self, i: usize) {
        if !self.tickable() {
            return;
        }
        if let Some(r) = self.rows.get_mut(i).filter(|r| !r.path.is_empty()) {
            r.ticked = !r.ticked;
        }
    }

    /// The ticks may change: not while the session came from `Default`, whose order is
    /// read-only input (reloading it as a plugin list would drop the `Default` refusal;
    /// review M2).
    pub fn tickable(&self) -> bool {
        !live_source(&self.source)
    }

    /// Why row `i` cannot be the active file, if it cannot.
    pub fn refused(&self, i: usize) -> Option<String> {
        let Some(r) = self.rows.get(i) else {
            return Some("select a plugin first".to_string());
        };
        if live_source(&self.source) {
            return Some(format!(
                "the session was loaded from `{LIVE_PROFILE}`, the owner's live game: read-only input, never a target; load the dev profile or a plugin list"
            ));
        }
        if let Some(why) = &r.official {
            return Some(format!(
                "{} is an official master or Creation ({why}): never the active file",
                r.name
            ));
        }
        if r.master {
            return Some(format!(
                "{} has the master flag: a master-flagged plugin cannot be the active file (as in the CK, ck-20261101-01)",
                r.name
            ));
        }
        if r.path.is_empty() {
            return Some(format!(
                "{}: not in the load order (the engine reported no path)",
                r.name
            ));
        }
        None
    }

    /// "Set as Active File" on the selected row: ticks it and makes it the active file,
    /// or says why not (the active file is then unchanged).
    pub fn set_active(&mut self) -> Result<(), String> {
        let i = self.selected.unwrap_or(usize::MAX);
        if let Some(why) = self.refused(i) {
            self.refusal = Some(why.clone());
            return Err(why);
        }
        if let Some(r) = self.rows.get_mut(i) {
            r.ticked = true;
            self.active = Some(r.path.clone());
        }
        self.refusal = None;
        Ok(())
    }

    /// OK: `load` of the ticked plugins (with the session's `data_dir`, `language` and
    /// `no_cache`) when they differ from the load order, and the active file.
    pub fn accept(&self, session: Option<&Map<String, Value>>) -> DataAsk {
        let changed = self.tickable() && self.rows.iter().any(|r| r.ticked != r.loaded);
        let ticked: Vec<&str> = self
            .rows
            .iter()
            .filter(|r| r.ticked && !r.path.is_empty())
            .map(|r| r.path.as_str())
            .collect();
        let load = (changed && !ticked.is_empty()).then(|| {
            let mut args = Map::new();
            args.insert("plugins".into(), json!(ticked));
            for key in ["data_dir", "language", "no_cache"] {
                if let Some(v) = session.and_then(|s| s.get(key)) {
                    args.insert(key.into(), v.clone());
                }
            }
            Request::new("load", Value::Object(args))
        });
        // The active file only if this load does not refuse it (review M1) and, when it
        // is one of the listed plugins, only if it is still ticked: an unticked one is not
        // loaded.
        let active = self
            .active
            .clone()
            .filter(|a| self.active_ok(a) && self.rows.iter().any(|r| r.ticked && r.path == *a));
        DataAsk::Accept { load, active }
    }

    /// Takes the reply to a request sent with `Pending::Data(ticket)`.
    pub fn receive(&mut self, ticket: &DataTicket, result: Result<&Value, String>) {
        let DataTicket::Details(path) = ticket else {
            return;
        };
        if self.details_pending.as_deref() == Some(path.as_str()) {
            self.details_pending = None;
        }
        self.details = Some(match result {
            Ok(env) => Details::from_envelope(path, env),
            Err(e) => Details {
                path: path.clone(),
                error: Some(e),
                ..Details::default()
            },
        });
    }

    /// Takes a successful reply that came back on `Slot::DataLoad` or
    /// `Slot::DataDetails` (the dialog sends neither today; a details reply there is
    /// taken as one sent with a ticket).
    pub(crate) fn receive_slot(&mut self, slot: Slot, request: &Request, env: &Value) {
        if slot == Slot::DataDetails {
            let path = text_or_empty(&request.args, "plugin");
            self.receive(&DataTicket::Details(path), Ok(env));
        }
    }
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, ScrollArea};

use crate::theme::{self, Status, View};

impl DataDialogState {
    /// Draws the dialog; `active_file` is the window's active file (shown at the top),
    /// `session` the arguments the session was loaded with.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        busy: bool,
        active_file: Option<&str>,
        session: Option<&Map<String, Value>>,
    ) -> Option<DataAsk> {
        let mut ask = None;
        ui.horizontal_wrapped(|ui| {
            ui.strong("Active file:");
            match active_file {
                Some(a) => ui.monospace(a),
                None => ui.weak("none"),
            };
        });
        if self.rows.is_empty() {
            theme::status_line(
                ui,
                Status::NotRun,
                "nothing loaded: load a profile or a plugin list (Load tab) first; the list is the session's load order",
            );
        } else {
            ui.weak(format!(
                "{} ({} plugin(s); double-click a row to tick it)",
                self.source,
                self.rows.len()
            ));
        }
        theme::gap(ui);
        let tickable = self.tickable();
        if !tickable {
            theme::status_line(
                ui,
                Status::NotRun,
                &format!("loaded from `{LIVE_PROFILE}`: its order is read-only input here (no ticks, no active file)"),
            );
        }
        let height = theme::view_height(ui, View::Medium);
        ui.columns(2, |cols| {
            let [left, right] = cols else {
                return;
            };
            ScrollArea::vertical()
                .id_salt("data-dialog-rows")
                .auto_shrink([false, false])
                .max_height(height)
                .show(left, |ui| {
                    theme::table(ui, "data-dialog-grid").show(ui, |ui| {
                        for i in 0..self.rows.len() {
                            let status = self.status(i);
                            let selected = self.selected == Some(i);
                            let Some(r) = self.rows.get_mut(i) else {
                                break;
                            };
                            ui.add_enabled(
                                tickable && !r.path.is_empty(),
                                egui::Checkbox::without_text(&mut r.ticked),
                            );
                            let name = ui.selectable_label(selected, &r.name);
                            ui.label(status);
                            ui.end_row();
                            if name.double_clicked() {
                                self.toggle(i);
                            }
                            if name.clicked() || name.double_clicked() {
                                if let Some(a) = self.select(i) {
                                    ask = Some(a);
                                }
                            }
                        }
                    });
                });
            self.details_ui(right);
        });
        if let Some(why) = &self.refusal {
            theme::status_line(ui, Status::Fail, why);
        }
        theme::gap(ui);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.selected.is_some(),
                    egui::Button::new("Set as Active File"),
                )
                .clicked()
            {
                // A refusal is kept in `refusal` and shown above.
                let _ = self.set_active();
            }
            if ui
                .add_enabled(self.active.is_some(), egui::Button::new("No Active File"))
                .clicked()
            {
                self.active = None;
                self.refusal = None;
            }
            ui.separator();
            if ui.add_enabled(!busy, egui::Button::new("OK")).clicked() {
                ask = Some(self.accept(session));
            }
            if ui.button("Cancel").clicked() {
                ask = Some(DataAsk::Cancel);
            }
        });
        ask
    }

    /// The selected plugin's details (KitKraft's own pane: the CK's is not observed).
    fn details_ui(&self, ui: &mut egui::Ui) {
        let Some(r) = self.selected.and_then(|i| self.rows.get(i)) else {
            ui.weak("select a plugin to see its header");
            return;
        };
        ui.strong(&r.name);
        if r.path.is_empty() {
            ui.weak("not in the load order: no file to read");
            return;
        }
        ui.monospace(&r.path);
        let details = self.details.as_ref().filter(|d| d.path == r.path);
        let Some(d) = details else {
            if self.details_pending.as_deref() == Some(r.path.as_str()) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("inspect TES4");
                });
            }
            return;
        };
        if let Some(e) = &d.error {
            theme::status_line(ui, Status::Fail, e);
            return;
        }
        let none = || "-".to_string();
        ui.label(format!("Author: {}", d.author.clone().unwrap_or_else(none)));
        ui.label(format!(
            "Description: {}",
            d.description.clone().unwrap_or_else(none)
        ));
        let masters = if d.masters.is_empty() {
            none()
        } else {
            d.masters.join(", ")
        };
        ui.label(format!("Masters: {masters}"));
        ui.label(format!("Header flags: {}", d.flags));
        ui.label(format!(
            "Timestamp (as stored, not a date): {}",
            d.timestamp.map_or_else(none, |t| t.to_string())
        ));
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::panels::load::PluginRow;

    fn row(name: &str, flags: &str, active: &str) -> PluginRow {
        PluginRow {
            index: 0,
            name: name.to_string(),
            path: std::env::temp_dir()
                .join("kitkraft-ui-data")
                .join(name)
                .display()
                .to_string(),
            scale: "full".to_string(),
            flags: flags.to_string(),
            active: active.to_string(),
            provider: "data".to_string(),
            records: Some(1),
            runtime_slot: String::new(),
            cache: "hit".to_string(),
            missing_masters: Vec::new(),
            late_masters: Vec::new(),
            error: None,
        }
    }

    fn view(source: &str) -> LoadView {
        LoadView {
            source: source.to_string(),
            plugins: vec![
                row("Starfield.esm", "master, localized", "hardcoded-official"),
                row("Base.esm", "master", "listed"),
                row("SFBGS099.esm", "", "listed"),
                row("Mine.esp", "", "listed"),
            ],
            inactive: vec!["Off.esp".to_string()],
            missing_active: Vec::new(),
        }
    }

    fn opened(source: &str) -> DataDialogState {
        let mut s = DataDialogState::default();
        s.sync(Some(&view(source)), 1, None);
        s
    }

    #[test]
    fn rows_carry_the_cks_statuses_and_the_engines_flags() {
        let s = opened("plugins");
        let names: Vec<&str> = s.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Starfield.esm",
                "Base.esm",
                "SFBGS099.esm",
                "Mine.esp",
                "Off.esp"
            ]
        );
        let statuses: Vec<&str> = (0..5).map(|i| s.status(i)).collect();
        assert_eq!(
            statuses,
            [
                "Master File",
                "Master File",
                "Plugin File",
                "Plugin File",
                "Plugin File"
            ]
        );
        assert!(s.rows.iter().take(4).all(|r| r.loaded && r.ticked));
        assert!(!s.rows[4].ticked && s.rows[4].path.is_empty());
    }

    /// An official master, a master-flagged plugin, an official name without the flag
    /// and a plugin with no path are each refused; the active file stays as it was.
    #[test]
    fn only_a_loaded_plugin_file_can_be_the_active_file() {
        let mut s = opened("plugins");
        for (i, want) in [
            (0, "hardcoded-official"),
            (1, "master flag"),
            (2, "official master"),
            (4, "not in the load order"),
        ] {
            s.select(i);
            let err = s.set_active().unwrap_err();
            assert!(err.contains(want), "{i}: {err}");
            assert_eq!(s.active, None);
            assert_eq!(s.refusal.as_deref(), Some(err.as_str()));
        }
        s.rows[3].ticked = false;
        s.select(3);
        s.set_active().unwrap();
        assert_eq!(s.active.as_deref(), Some(s.rows[3].path.as_str()));
        assert!(s.rows[3].ticked, "Set as Active File ticks the row");
        assert_eq!(s.status(3), "Active File");
        assert!(s.refusal.is_none());
    }

    #[test]
    fn the_default_profile_is_never_a_target() {
        let dir = std::env::temp_dir().join("profiles").join("Default");
        let mut s = opened(&format!("profile:{}", dir.display()));
        s.select(3);
        let err = s.set_active().unwrap_err();
        assert!(err.contains("`Default`"), "{err}");
        assert_eq!(s.active, None);
        let dev = std::env::temp_dir().join("profiles").join("KitKraft Dev");
        let mut s = opened(&format!("profile:{}", dev.display()));
        s.select(3);
        s.set_active().unwrap();
    }

    /// Review M1: an active file chosen under another profile does not survive a `Default`
    /// load, and OK does not re-assert it.
    #[test]
    fn a_stale_active_file_is_dropped_under_default() {
        let mut s = opened("plugins");
        s.select(3);
        s.set_active().unwrap();
        let path = s.rows[3].path.clone();
        s.closed();
        let live = std::env::temp_dir().join("profiles").join("Default");
        s.sync(
            Some(&view(&format!("profile:{}", live.display()))),
            2,
            Some(&path),
        );
        assert_eq!(s.status(3), "Plugin File");
        let DataAsk::Accept { active, .. } = s.accept(None) else {
            panic!("accept");
        };
        assert_eq!(active, None);
        // A new load (same opening) that refuses the row drops it as well.
        let mut s = opened("plugins");
        s.select(3);
        s.set_active().unwrap();
        s.sync(Some(&view(&format!("profile:{}", live.display()))), 2, None);
        assert_eq!(s.active, None);
    }

    /// Review M2: OK in a `Default` session never reloads it as a plugin list (which would
    /// drop the `Default` refusal): the ticks stay and OK sends no `load`.
    #[test]
    fn ok_never_turns_a_default_session_into_a_plugin_list() {
        let live = std::env::temp_dir().join("profiles").join("Default");
        let mut s = opened(&format!("profile:{}", live.display()));
        s.toggle(2);
        assert!(s.rows[2].ticked, "the Default order is read-only input");
        s.rows[2].ticked = false; // as if a box were clicked anyway
        assert_eq!(
            s.accept(None),
            DataAsk::Accept {
                load: None,
                active: None
            }
        );
    }

    #[test]
    fn ok_loads_only_when_the_ticks_change_and_keeps_the_session_arguments() {
        let mut s = opened("plugins");
        s.select(3);
        s.set_active().unwrap();
        let path = s.rows[3].path.clone();
        assert_eq!(
            s.accept(None),
            DataAsk::Accept {
                load: None,
                active: Some(path.clone())
            }
        );
        s.toggle(2);
        s.toggle(4); // no path: stays unticked
        let session = json!({"plugins": ["x"], "data_dir": "D", "language": "fr"});
        let DataAsk::Accept { load, active } = s.accept(session.as_object()) else {
            panic!("accept");
        };
        let load = load.unwrap();
        assert_eq!(load.id, "load");
        crate::gate::check(&load).unwrap();
        let plugins = load.args["plugins"].as_array().unwrap();
        assert_eq!(plugins.len(), 3);
        assert_eq!(load.args["data_dir"], "D");
        assert_eq!(load.args["language"], "fr");
        assert_eq!(active.as_deref(), Some(path.as_str()));
        // An unticked active file is not loaded, so it is not the active file.
        s.toggle(3);
        let DataAsk::Accept { active, .. } = s.accept(None) else {
            panic!("accept");
        };
        assert_eq!(active, None);
    }

    #[test]
    fn details_read_the_tes4_header() {
        let mut s = opened("plugins");
        let Some(DataAsk::Details(path, req)) = s.select(1) else {
            panic!("a details request");
        };
        assert_eq!(req.id, "inspect");
        assert_eq!(req.args["form"], "0");
        crate::gate::check(&req).unwrap();
        assert!(s.select(1).is_none(), "one request per plugin");
        let env = json!({"data": {"record": {"signature": "TES4", "flags": "0x00000001", "timestamp": 7},
            "subrecords": [
                {"signature": "HEDR", "payload_hex": "00"},
                {"signature": "CNAM", "payload_hex": "4B6974417574686F7200"},
                {"signature": "SNAM", "payload_hex": "4120706C7567696E", "payload_truncated": true},
                {"signature": "MAST", "payload_hex": "537461726669656C642E65736D00"},
                {"signature": "MAST", "payload_hex": "zz"}]}});
        s.receive(&DataTicket::Details(path.clone()), Ok(&env));
        let d = s.details.clone().unwrap();
        assert_eq!(d.author.as_deref(), Some("KitAuthor"));
        assert_eq!(d.description.as_deref(), Some("A plugin (cut)"));
        assert_eq!(d.masters, ["Starfield.esm", ""]);
        assert_eq!(d.flags, "0x00000001");
        assert_eq!(d.timestamp, Some(7));
        assert!(s.details_pending.is_none());
        let not = Details::from_envelope("p", &json!({"data": {"record": {"signature": "GLOB"}}}));
        assert!(not.error.unwrap().contains("not a TES4"));
        s.receive(&DataTicket::Details(path), Err("no file".into()));
        assert_eq!(s.details.unwrap().error.as_deref(), Some("no file"));
    }

    #[test]
    fn official_names_follow_the_gates_list() {
        for n in [
            "Starfield.esm",
            "constellation.ESM",
            "OldMars.esm",
            "ShatteredSpace.esm",
            "BlueprintShips-Starfield.esm",
            "SFBGS004.esm",
        ] {
            assert!(is_official_name(n), "{n}");
        }
        for n in ["Mine.esm", "SFBGS004.esp", "MyStarfield.esm", ""] {
            assert!(!is_official_name(n), "{n}");
        }
    }

    #[test]
    fn a_new_load_rebuilds_the_rows_and_reopening_takes_the_windows_active_file() {
        let mut s = opened("plugins");
        s.select(3);
        s.set_active().unwrap();
        s.sync(Some(&view("plugins")), 1, None);
        assert_eq!(s.selected, Some(3), "same load: nothing rebuilt");
        s.sync(Some(&view("plugins")), 2, None);
        assert_eq!(s.selected, None);
        assert!(s.active.is_some(), "a new load keeps the dialog's choice");
        s.closed();
        s.sync(Some(&view("plugins")), 2, Some("X.esp"));
        assert_eq!(s.active.as_deref(), Some("X.esp"));
        s.closed();
        s.sync(None, 3, None);
        assert!(s.rows.is_empty() && s.active.is_none());
    }
}
