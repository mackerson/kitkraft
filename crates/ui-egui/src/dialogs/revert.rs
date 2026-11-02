//! `revert` (M7.E6, pipeline E; ADR 0007 Amendment 4, model A): undo of the editor's last
//! confirmed write into the active file, previewed and confirmed per click.
//!
//! The editor offers it only for a write that added the record to the active file (the
//! write's envelope says `outcome: inserted`). Then the engine's `revert` with
//! `to: remove` (M7.S7) drops that override, and the active file is what it was before
//! the write. For a write that replaced a version the file already held (a second edit of
//! a record, see the editor's H1 note), `revert` would drop that earlier version too, so
//! the undo is refused there: it would undo more than one write.
//!
//! [`preview_request`] is `revert --dry-run`: a staged copy, its `diff` and `validate`,
//! leaving the active file untouched. The confirm box's one click sends
//! [`write_request`], the same arguments without `dry_run`. This file is the only one
//! that builds a confirmed `revert` (the gate's per-entry source scan holds it).

use serde_json::{json, Value};

use crate::gate::Request;

/// The preview of dropping `form`'s override from `plugin`, the active file: a dry run,
/// which the gate passes like a read.
#[allow(dead_code)] // the editor's undo sends it from the next commit (M7.E6)
pub(crate) fn preview_request(plugin: &str, form: &str) -> Request {
    Request::new(
        "revert",
        json!({"plugin": plugin, "form": form, "to": "remove", "dry_run": true}),
    )
}

/// The confirmed undo of a previewed `revert`: the preview's arguments with `dry_run`
/// dropped, built once per confirm click. `None` when `preview` is not a `revert` dry run.
#[allow(dead_code)] // the confirm box sends it from the next commit (M7.E6)
pub(crate) fn write_request(preview: &Request) -> Option<Request> {
    if preview.id != "revert" || preview.args.get("dry_run") != Some(&Value::Bool(true)) {
        return None;
    }
    let mut args = preview.args.clone();
    args.as_object_mut()?.remove("dry_run");
    Some(Request::confirmed("revert", args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::check;

    #[test]
    fn the_undo_is_the_preview_without_dry_run_and_confirmed() {
        let preview = preview_request(r"C:\out\Patch.esp", "Fixture.esm:000800");
        assert!(!preview.is_confirmed());
        assert!(check(&preview).is_ok(), "the dry run passes like a read");
        let undo = write_request(&preview).unwrap();
        assert!(undo.is_confirmed());
        assert_eq!(undo.id, "revert");
        assert_eq!(undo.args.get("dry_run"), None);
        assert_eq!(undo.args["to"], json!("remove"));
        assert_eq!(undo.args["plugin"], json!(r"C:\out\Patch.esp"));
        assert!(check(&undo).is_ok());
        assert!(write_request(&undo).is_none());
        assert!(write_request(&Request::new("set", json!({"dry_run": true}))).is_none());
    }
}
