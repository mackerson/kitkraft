//! `clone` (M7.E3, pipeline E; ADR 0007 Amendment 4, model A): a copy of a record into
//! the active file as a new form (Edit > Duplicate), previewed and confirmed per click.
//! The form is `super::new::form_ui`; the Object Window's hook is
//! `super::new::object_window_actions`.
//!
//! [`preview_request`] is `clone --dry-run` (M7.S6): the copy written and validated on a
//! staged copy of the active file, which stays untouched. The confirm box's one click
//! sends [`write_request`], the same arguments without `dry_run`. This file is the only
//! one that builds a confirmed `clone` (the gate's per-entry source scan holds it).

use serde_json::{json, Value};

use crate::gate::Request;

/// The preview of copying `form` into `into` (the active file) as a new form with the
/// EditorID `edid`: a dry run, which the gate passes like a read.
#[allow(dead_code)] // the editor's Duplicate form sends it from the next commit (M7.E3)
pub(crate) fn preview_request(form: &str, edid: &str, into: &str) -> Request {
    Request::new(
        "clone",
        json!({"form": form, "edid": edid, "into": into, "dry_run": true}),
    )
}

/// The confirmed copy of a previewed `clone`: the preview's arguments with `dry_run`
/// dropped, built once per confirm click. `None` when `preview` is not a `clone` dry run.
#[allow(dead_code)] // the confirm box sends it from the next commit (M7.E3)
pub(crate) fn write_request(preview: &Request) -> Option<Request> {
    if preview.id != "clone" || preview.args.get("dry_run") != Some(&Value::Bool(true)) {
        return None;
    }
    let mut args = preview.args.clone();
    args.as_object_mut()?.remove("dry_run");
    Some(Request::confirmed("clone", args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::check;

    #[test]
    fn the_copy_is_the_preview_without_dry_run_and_confirmed() {
        let preview = preview_request("Fixture.esm:000800", "Glob800Copy", r"C:\out\Patch.esp");
        assert!(!preview.is_confirmed());
        assert!(check(&preview).is_ok(), "the dry run passes like a read");
        let write = write_request(&preview).unwrap();
        assert!(write.is_confirmed());
        assert_eq!(write.id, "clone");
        assert_eq!(write.args.get("dry_run"), None);
        assert_eq!(write.args["edid"], json!("Glob800Copy"));
        assert!(check(&write).is_ok());
        assert!(write_request(&write).is_none());
        assert!(write_request(&Request::new("new", json!({"dry_run": true}))).is_none());
    }
}
