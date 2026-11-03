//! Pipeline R's harness tests (sprint 2026-11-02: the Render Window's NIF bounds and
//! camera; M7.R1, M7.R3).
//!
//! M7.R3 (viewport step 3, the NIF variant): a cell's references are drawn with their
//! base's model box (`mesh.bounds`), `OBND` when the model gives none, and their type's
//! shape when neither does, which the status line counts as a limitation. The
//! `Slot::MeshBounds` arm of `App::receive` is the integrator's (ask A3 of M7.S4), so the
//! replies are filed through `RenderTab::receive_bounds` directly, as that arm will.

use egui_kittest::kittest::Queryable;
use glam::Vec3;
use serde_json::{json, Value};

use super::*;
use crate::panels::render::{cell_request, CellPick};
use crate::viewport::cell::box_sources;
use crate::viewport::scene::Shape;

/// A `cell` reply with three references: two of the crate (OBND), one marker (no OBND).
fn cell_env() -> Value {
    let item = |form: &str, base: &str, sig: &str, at: [f32; 3], obnd: Value| {
        json!({"form": form, "signature": "REFR",
               "base": {"kind": "formref", "form": base, "raw": "00000800"},
               "base_signature": sig, "position": at, "rotation": [0.0, 0.0, 0.0],
               "scale": null, "bounds": obnd, "persistent": false, "plugin": "T.esm"})
    };
    let obnd = json!({"min": [-1.0, -1.0, 0.0], "max": [1.0, 1.0, 1.0]});
    json!({
        "command": "cell",
        "data": {"mode": "cell", "cell": "T.esm:000900", "cell_editor_id": "RoomR3",
                 "world": null, "total": 3, "next_cursor": null, "summary": "3 reference(s)",
                 "items": [
                     item("T.esm:000901", "T.esm:000800", "STAT", [0.0, 0.0, 0.0], obnd.clone()),
                     item("T.esm:000902", "T.esm:000800", "STAT", [10.0, 0.0, 0.0], obnd),
                     item("T.esm:000903", "T.esm:000801", "IDLM", [0.0, 10.0, 0.0], Value::Null),
                 ]},
        "coverage": {}, "limitations": [],
    })
}

fn bounds_env(form: &str, model: &str, bounds: Value, limitation: &str) -> Value {
    json!({"command": "mesh.bounds",
           "data": {"form": form, "model": model, "bounds": bounds},
           "limitations": [limitation]})
}

fn status_has(h: &Bench, text: &str) -> bool {
    h.query_all_by_label_contains(text).next().is_some()
}

#[test]
fn m7_r3_model_boxes_replace_obnd_and_a_base_without_a_bound_is_a_limitation() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Render);
    let req = cell_request(&CellPick::Cell("RoomR3".into()), None);
    inject(&mut h, Slot::RenderCell, req, Ok(cell_env()));
    settle(&mut h);

    // Before any reply: each distinct base asked once, OBND drawn meanwhile.
    let render = &h.state().app.render;
    assert_eq!(render.bounds_progress(), (2, 0));
    let crate_box = render.viewport.markers()[0];
    assert_eq!(crate_box.shape, Shape::Box);
    assert_eq!(crate_box.half_extents, Vec3::new(1.0, 1.0, 0.5));
    assert!(status_has(&h, "0 with their model's box, 2 with OBND"));
    assert!(status_has(&h, "model boxes: 0 of 2 bases answered"));

    // The replies, as the app.rs arm (ask A3) will file them.
    {
        let render = &mut h.state_mut().app.render;
        render.receive_bounds(&bounds_env(
            "T.esm:000800",
            "Test\\Crate.nif",
            json!({"min": [-2.0, -2.0, 0.0], "max": [2.0, 2.0, 3.0]}),
            "the box is the union of the bounds ...",
        ));
        render.receive_bounds(&bounds_env(
            "T.esm:000801",
            "Markers\\Idle.nif",
            Value::Null,
            "no bound in the model: it has no geometry block",
        ));
    }
    h.run_steps(4);

    let render = &h.state().app.render;
    assert_eq!(render.bounds_progress(), (2, 2));
    let m = render.viewport.markers();
    assert_eq!(m.len(), 3);
    // Both crates are drawn with the model's box (larger than their OBND).
    for crate_ref in &m[..2] {
        assert_eq!(crate_ref.shape, Shape::Box);
        assert_eq!(crate_ref.half_extents, Vec3::new(2.0, 2.0, 1.5));
        assert_eq!(crate_ref.offset, Vec3::new(0.0, 0.0, 1.5));
    }
    // The marker base: no model box, no OBND: its type's shape, never a box.
    assert_ne!(m[2].shape, Shape::Box);
    assert_eq!(box_sources(&render.page_refs()), (2, 0, 1));
    assert!(status_has(&h, "2 with their model's box, 0 with OBND"));
    assert!(status_has(&h, "1 without a bound"));
    assert!(status_has(&h, "1 base(s) without a model box"));
    assert!(!status_has(&h, "bases answered"));
}
