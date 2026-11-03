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
    assert!(status_has(&h, "boxes: 0 model, 2 OBND, 1 none"));
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
    assert!(status_has(&h, "boxes: 2 model, 0 OBND, 1 none"));
    assert!(status_has(&h, "1 without a bound"));
    assert!(status_has(&h, "1 base(s) without a model box"));
    assert!(!status_has(&h, "bases answered"));
}

/// Each limitation line of the status row lies wholly inside the window (review M2 of
/// M7.R3: at 1400 points the "without a bound" and "without a position" lines ran past
/// the right edge).
fn assert_wholly_in_view(h: &Bench, text: &str) {
    let nodes: Vec<_> = h.query_all_by_label_contains(text).collect();
    assert!(!nodes.is_empty(), "no `{text}`");
    let fits = nodes.iter().any(|n| {
        let r = n.rect();
        in_view(n) && r.max.x <= SIZE[0] && r.min.x >= 0.0
    });
    assert!(
        fits,
        "`{text}` is not wholly inside the {SIZE:?} window: {:?}",
        nodes.iter().map(|n| n.rect()).collect::<Vec<_>>()
    );
}

#[test]
fn m7_r3_the_status_lines_fit_the_window() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Render);
    let mut env = cell_env();
    // A fourth reference without a position.
    env["data"]["items"].as_array_mut().unwrap().push(
        json!({"form": "T.esm:000904", "signature": "REFR",
                     "base": {"kind": "formref", "form": "T.esm:000800"},
                     "base_signature": "STAT", "position": null}),
    );
    env["data"]["total"] = json!(4);
    let req = cell_request(&CellPick::Cell("RoomR3".into()), None);
    inject(&mut h, Slot::RenderCell, req, Ok(env));
    settle(&mut h);
    assert_wholly_in_view(&h, "without a bound: neither");
    assert_wholly_in_view(&h, "without a position");
    assert_wholly_in_view(&h, "model boxes: 0 of 2 bases answered");
}

/// A cell drawn with both bases answered (the crate with a model box).
fn answered_cell(h: &mut Bench) {
    // As the Load button does: the tab starts the cell (its old page dropped).
    let req = h.state_mut().app.render.show_cell("RoomR3");
    inject(h, Slot::RenderCell, req, Ok(cell_env()));
    settle(h);
    let render = &mut h.state_mut().app.render;
    render.receive_bounds(&bounds_env(
        "T.esm:000800",
        "Test/Crate.nif",
        json!({"min": [-2.0, -2.0, 0.0], "max": [2.0, 2.0, 3.0]}),
        "the box is the union of the bounds ...",
    ));
    render.receive_bounds(&bounds_env(
        "T.esm:000801",
        "Markers/Idle.nif",
        Value::Null,
        "no bound in the model: it has no geometry block",
    ));
    h.run_steps(4);
    assert_eq!(h.state().app.render.bounds_progress(), (2, 2));
    assert_eq!(box_sources(&h.state().app.render.page_refs()).0, 2);
}

#[test]
fn m7_r3_model_boxes_are_forgotten_on_a_new_load_and_on_an_engine_restart() {
    // Review M1 of M7.R3: the boxes outlived the load order they came from, and after
    // "Drop pending" restarted the engine the bases asked before were never asked again.
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Render);
    answered_cell(&mut h);

    // A new load order: the boxes of the old one are dropped and OBND is drawn again.
    let load = json!({"command": "load", "data": {"source": "plugins", "inactive": [],
        "missing_active_plugins": [], "plugins": [
            {"index": 0, "name": "Other.esm", "scale": "full", "flag_names": ["master"],
             "active": "listed", "provider": "path", "records": 1, "runtime_slot": "00",
             "cache": "built", "missing_masters": [], "late_masters": [], "error": null}]},
        "coverage": {}, "limitations": []});
    let req = Request::new("load", json!({"plugins": ["Other.esm"]}));
    inject(&mut h, Slot::Load, req, Ok(load));
    open_tab(&mut h, Tab::Render);
    let render = &h.state().app.render;
    assert_eq!(render.bounds_progress(), (0, 0));
    assert_eq!(box_sources(&render.page_refs()), (0, 2, 1));

    // The same cell under the new load asks both bases again.
    answered_cell(&mut h);

    // An engine restart forgets them too.
    h.state_mut().app.drop_pending();
    h.run_steps(4);
    let render = &h.state().app.render;
    assert_eq!(render.bounds_progress(), (0, 0));
    assert_eq!(box_sources(&render.page_refs()), (0, 2, 1));
}
