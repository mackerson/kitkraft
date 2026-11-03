//! `new` and `clone` from the Object Window (M7.E3, pipeline E; ADR 0007 Amendment 4,
//! model A): a new form of the selected type, or a copy of the selected record (Edit >
//! Duplicate), each into the active file, previewed and confirmed per click.
//!
//! [`preview_request`] is `new --dry-run` (M7.S6): the new form written and validated on
//! a staged copy of the active file, which stays untouched. The confirm box's one click
//! sends [`write_request`], the same arguments without `dry_run`. This file is the only
//! one that builds a confirmed `new` (the gate's per-entry source scan holds it); `clone`'s
//! requests are `super::clone`'s.
//!
//! Step 0 of sprint 2026-11-02 declared this file and the Object Window's one call to
//! [`object_window_actions`] (`windows/object_window.rs`).
//!
//! Labels: "Duplicate" is the CK's observed Edit menu item (`docs/ck-parity.md` row 132).
//! "New" and the form's own labels (the "New Form" and "Duplicate" titles, "Copy of",
//! "Record type", "EditorID", "Preview") are KitKraft's, not observed: the Object
//! Window's context menu is still open as sitting step S1-11.

use eframe::egui;
use serde_json::{json, Value};

use crate::gate::Request;
use crate::panels::common::Action;
use crate::theme::{self, Status};

/// The New Form form's fields: which command, its source (a record type for `new`, the
/// record copied for `clone`) and the new EditorID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewForm {
    pub command: &'static str,
    pub source: String,
    pub edid: String,
}

impl NewForm {
    /// The form for `command` (`new` or `clone`) on `source`: for `new` a 4-character
    /// signature is taken as the type (anything else leaves the type to type in).
    pub(crate) fn open(command: &str, source: &str) -> Self {
        let source = source.trim();
        if command == "clone" {
            Self {
                command: "clone",
                source: source.to_string(),
                edid: String::new(),
            }
        } else {
            Self {
                command: "new",
                source: if source.len() == 4 {
                    source.to_string()
                } else {
                    String::new()
                },
                edid: String::new(),
            }
        }
    }
}

/// What the New Form form's buttons asked for this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FormAsk {
    pub preview: bool,
    pub close: bool,
}

/// The New Form form: the record type (`new`) or the record copied (`clone`), the new
/// EditorID, the active file it goes into, and Preview and Close. Nothing is sent from
/// here: Preview asks the editor for the dry run.
pub(crate) fn form_ui(ui: &mut egui::Ui, f: &mut NewForm, active_file: Option<&str>) -> FormAsk {
    let mut ask = FormAsk::default();
    if f.command == "clone" {
        ui.horizontal(|ui| {
            ui.label("Copy of");
            ui.monospace(&f.source);
        });
    } else {
        ui.horizontal(|ui| {
            let label = ui.label("Record type");
            ui.add(
                egui::TextEdit::singleline(&mut f.source)
                    .hint_text("GLOB, KYWD, ...")
                    .desired_width(theme::width(ui, theme::Field::Tiny)),
            )
            .labelled_by(label.id);
        });
    }
    ui.horizontal(|ui| {
        let label = ui.label("EditorID");
        ui.add(
            egui::TextEdit::singleline(&mut f.edid)
                .hint_text("letters, digits, _; unique")
                .desired_width(theme::width(ui, theme::Field::Medium)),
        )
        .labelled_by(label.id);
    });
    match active_file.filter(|a| !a.trim().is_empty()) {
        Some(a) => {
            ui.label(format!("into the active file: {a}"));
        }
        None => {
            theme::status_line(
                ui,
                Status::NotRun,
                "no active file: choose one in File > Data... (Set as Active File)",
            );
        }
    }
    let ready = !f.source.trim().is_empty() && !f.edid.trim().is_empty();
    ui.horizontal(|ui| {
        ask.preview = ui
            .add_enabled(ready, egui::Button::new("Preview"))
            .on_hover_text(format!(
                "`{} --dry-run`: the new form written and validated on a staged copy; nothing written",
                f.command
            ))
            .clicked();
        ask.close = ui.button("Close").clicked();
    });
    ask
}

/// The preview of a new form of `record_type` (a signature) with the EditorID `edid` in
/// `into`, the active file: a dry run, which the gate passes like a read.
pub(crate) fn preview_request(record_type: &str, edid: &str, into: &str) -> Request {
    Request::new(
        "new",
        json!({"type": record_type, "edid": edid, "into": into, "dry_run": true}),
    )
}

/// The confirmed write of a previewed `new`: the preview's arguments with `dry_run`
/// dropped, built once per confirm click. `None` when `preview` is not a `new` dry run.
pub(crate) fn write_request(preview: &Request) -> Option<Request> {
    if preview.id != "new" || preview.args.get("dry_run") != Some(&Value::Bool(true)) {
        return None;
    }
    let mut args = preview.args.clone();
    args.as_object_mut()?.remove("dry_run");
    Some(Request::confirmed("new", args))
}

/// The Object Window's `new`/`clone` controls for `selected` (the record or type the
/// window has selected): New (a new form of the selected type, or of the selected
/// record's type left to type in) and Duplicate (a copy of the selected record, the CK's
/// Edit > Duplicate). Returns the actions they ask for (`Action::OpenEditor`).
pub(crate) fn object_window_actions(ui: &mut egui::Ui, selected: Option<&str>) -> Vec<Action> {
    let mut out = Vec::new();
    let selected = selected.map(str::trim).filter(|s| !s.is_empty());
    let record = selected.filter(|s| s.contains(':'));
    ui.horizontal(|ui| {
        if ui
            .button("New")
            .on_hover_text("a new form into the active file: `new --dry-run` first, then a confirm")
            .clicked()
        {
            out.push(Action::OpenEditor {
                command: "new".to_string(),
                form: selected.unwrap_or_default().to_string(),
            });
        }
        if ui
            .add_enabled(record.is_some(), egui::Button::new("Duplicate"))
            .on_hover_text("a copy of the selected record into the active file: `clone --dry-run` first, then a confirm")
            .on_disabled_hover_text("select a record to duplicate")
            .clicked()
        {
            out.push(Action::OpenEditor {
                command: "clone".to_string(),
                form: record.unwrap_or_default().to_string(),
            });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::check;

    #[test]
    fn the_new_form_is_the_preview_without_dry_run_and_confirmed() {
        let preview = preview_request("GLOB", "KkNewGlobal", r"C:\out\Patch.esp");
        assert!(!preview.is_confirmed());
        assert!(check(&preview).is_ok(), "the dry run passes like a read");
        let write = write_request(&preview).unwrap();
        assert!(write.is_confirmed());
        assert_eq!(write.id, "new");
        assert_eq!(write.args.get("dry_run"), None);
        assert_eq!(write.args["type"], json!("GLOB"));
        assert!(check(&write).is_ok());
        assert!(write_request(&write).is_none());
        assert!(write_request(&Request::new("set", json!({"dry_run": true}))).is_none());
    }

    /// New takes a selected signature as its type and leaves a selected record's type to
    /// type in; Duplicate copies the selected record.
    #[test]
    fn the_form_opens_on_its_source() {
        assert_eq!(NewForm::open("new", "GLOB").source, "GLOB");
        assert_eq!(NewForm::open("new", "Fixture.esm:000800").source, "");
        let c = NewForm::open("clone", " Fixture.esm:000800 ");
        assert_eq!(
            (c.command, c.source.as_str()),
            ("clone", "Fixture.esm:000800")
        );
        assert_eq!(NewForm::open("other", "GLOB").command, "new");
    }
}
