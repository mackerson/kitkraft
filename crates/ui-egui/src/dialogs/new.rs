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

use eframe::egui;
use serde_json::{json, Value};

use crate::gate::Request;
use crate::panels::common::Action;

/// The preview of a new form of `record_type` (a signature) with the EditorID `edid` in
/// `into`, the active file: a dry run, which the gate passes like a read.
#[allow(dead_code)] // the editor's New form sends it from the next commit (M7.E3)
pub(crate) fn preview_request(record_type: &str, edid: &str, into: &str) -> Request {
    Request::new(
        "new",
        json!({"type": record_type, "edid": edid, "into": into, "dry_run": true}),
    )
}

/// The confirmed write of a previewed `new`: the preview's arguments with `dry_run`
/// dropped, built once per confirm click. `None` when `preview` is not a `new` dry run.
#[allow(dead_code)] // the confirm box sends it from the next commit (M7.E3)
pub(crate) fn write_request(preview: &Request) -> Option<Request> {
    if preview.id != "new" || preview.args.get("dry_run") != Some(&Value::Bool(true)) {
        return None;
    }
    let mut args = preview.args.clone();
    args.as_object_mut()?.remove("dry_run");
    Some(Request::confirmed("new", args))
}

/// The Object Window's `new`/`clone` controls for `selected` (the record or type the
/// window has selected); returns the actions they ask for (`Action::OpenEditor`).
pub(crate) fn object_window_actions(_ui: &mut egui::Ui, _selected: Option<&str>) -> Vec<Action> {
    Vec::new()
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
}
