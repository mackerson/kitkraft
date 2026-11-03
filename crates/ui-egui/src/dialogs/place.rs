//! `place` (M7.E4, pipeline E): a reference of a base object placed into the Cell
//! View's cell at a typed position, previewed and confirmed per click.
//!
//! Step 0 of sprint 2026-11-02 declared this file and the Cell View's one call to
//! [`cell_view_bar`] (`windows/cell_view.rs`), whose answer the shell turns into
//! `Action::PlaceInCell`. The bar draws nothing until M7.E4.

use eframe::egui;
use serde_json::{json, Value};

use crate::gate::Request;

/// The preview of placing a reference of `base` in `cell` at `position` into `into`, the
/// active file: `place --dry-run` (lane D round 5), which writes and validates a staged
/// copy and leaves the active file untouched; the gate passes it like a read.
#[allow(dead_code)] // the editor's Place form sends it from the next commit (M7.E4)
pub(crate) fn preview_request(base: &str, cell: &str, position: [f64; 3], into: &str) -> Request {
    Request::new(
        "place",
        json!({"base": base, "cell": cell, "position": position, "into": into, "dry_run": true}),
    )
}

/// The confirmed placement of a previewed `place`: the preview's arguments with `dry_run`
/// dropped, built once per confirm click; the only confirmed `place` in the crate (the
/// gate's per-entry source scan holds it). `None` when `preview` is not a `place` dry run.
#[allow(dead_code)] // the confirm box sends it from the next commit (M7.E4)
pub(crate) fn write_request(preview: &Request) -> Option<Request> {
    if preview.id != "place" || preview.args.get("dry_run") != Some(&Value::Bool(true)) {
        return None;
    }
    let mut args = preview.args.clone();
    args.as_object_mut()?.remove("dry_run");
    Some(Request::confirmed("place", args))
}

/// What the place bar asks for: place `base` into `cell` (formrefs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceAsk {
    pub base: String,
    pub cell: String,
}

/// The Cell View's place bar, drawn above the references of `cell`; `inspected` is the
/// shared selection (the base to place, once E decides how one is picked).
pub fn cell_view_bar(
    _ui: &mut egui::Ui,
    _cell: &str,
    _inspected: Option<&str>,
) -> Option<PlaceAsk> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::check;

    #[test]
    fn the_placement_is_the_preview_without_dry_run_and_confirmed() {
        let preview = preview_request(
            "World.esp:001000",
            "World.esp:001001",
            [1.0, 2.0, 3.0],
            r"C:\out\Patch.esp",
        );
        assert!(!preview.is_confirmed());
        assert!(check(&preview).is_ok(), "the dry run passes like a read");
        assert_eq!(preview.args["position"], json!([1.0, 2.0, 3.0]));
        let write = write_request(&preview).unwrap();
        assert!(write.is_confirmed());
        assert_eq!(write.id, "place");
        assert_eq!(write.args.get("dry_run"), None);
        assert!(check(&write).is_ok());
        assert!(write_request(&write).is_none());
        assert!(write_request(&Request::new("set", json!({"dry_run": true}))).is_none());
    }
}
