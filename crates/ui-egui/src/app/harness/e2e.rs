//! The integrator's end-to-end check (sprint 2026-11-02, wave 1): the pipelines' pieces
//! joined in the window's own frame, as one user would click them. The Data dialog sets a
//! scratch active file (M7.M2, R17), the edit dialog edits a field of a fixture record,
//! OK runs the `set` dry run, the confirm writes it (M7.E2), `inspect` reads it back, and
//! the edit dialog's undo drops it again (M7.E6), so `diff` against the active file as it
//! was before the edit shows no change. The fixture is made here
//! (`fake_mo2::plugin_bytes`), never the game's Data, and is unchanged at the end.

use std::path::{Path, PathBuf};

use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::{json, Value};

use super::*;
use crate::panels::cockpit::fake_mo2::plugin_bytes;
use crate::panels::common::Action;
use crate::windows::WindowKind;

/// A scratch folder under the OS temp dir with `Fixture.esm` (GLOB 000800, FLTV 1.5, and
/// GLOB 000801, FLTV 2.5); removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("kitkraft-ui-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Fixture.esm"),
            plugin_bytes(&[], 1, &[0x800, 0x801], 1.5),
        )
        .unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).display().to_string()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Clicks the one button labelled `label`, then lets the engine answer.
fn press(h: &mut Bench, label: &str) {
    let count = h
        .query_all_by_label(label)
        .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .count();
    assert_eq!(count, 1, "one `{label}` button\n{}", visible_text(h));
    one(h, label).scroll_to_me();
    h.run_steps(2);
    one(h, label).click();
    h.run_steps(3);
    settle(h);
}

/// Whether the one node labelled `label` is enabled.
fn enabled(h: &Bench, label: &str) -> bool {
    !one(h, label).accesskit_node().is_disabled()
}

/// The one text input whose value is `value` (`by_value`) or whose label is `value`.
fn input<'a>(h: &'a Bench, value: &'a str, by_value: bool) -> Node<'a> {
    let all: Vec<Node<'a>> = h
        .query_all_by(move |n| {
            n.role() == egui::accesskit::Role::TextInput
                && if by_value {
                    n.value().as_deref() == Some(value)
                } else {
                    n.label().as_deref() == Some(value)
                }
        })
        .collect();
    assert_eq!(all.len(), 1, "one text box `{value}`\n{}", visible_text(h));
    all.into_iter().next().expect("checked above")
}

/// Replaces the text of the edit box showing `from` with `to`, as a user types it.
fn retype(h: &mut Bench, from: &str, to: &str) {
    input(h, from, true).focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    input(h, from, true).type_text(to);
    h.run_steps(3);
    input(h, to, true);
}

/// Inspects `form` in the Records tab, so the edit dialog shows it.
fn show(h: &mut Bench, form: &str, editor_id: &str) {
    h.state_mut()
        .app
        .apply(vec![Action::Inspect(form.to_string())]);
    settle(h);
    assert_in_view(h, editor_id);
}

/// FLTV of GLOB `form` (hex FormID) as `path` alone holds it, read through `inspect`.
fn fltv_in(path: &str, form: &str) -> Value {
    let env = ReadOnlyEngine::new()
        .run(&Request::new(
            "inspect",
            json!({"form": form, "plugin": path}),
        ))
        .unwrap();
    env["data"]["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == json!("FLTV"))
        .map(|s| s["payload_hex"].clone())
        .unwrap()
}

fn hex(f: f32) -> Value {
    json!(f
        .to_le_bytes()
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<String>())
}

/// `diff`'s counts of `a` against `b` (added, removed, changed).
fn diff_counts(a: &str, b: &str) -> [Value; 3] {
    let env = ReadOnlyEngine::new()
        .run(&Request::new("diff", json!({"a": a, "b": b})))
        .unwrap();
    let c = &env["data"]["counts"];
    [
        c["added"].clone(),
        c["removed"].clone(),
        c["changed"].clone(),
    ]
}

/// The wave-1 end-to-end check (TRACK "Merge order per wave", wave 1): Data dialog ->
/// scratch active file -> edit dialog -> `set` dry run -> confirm -> `inspect` read-back
/// -> Undo last write -> `diff` against the active file as it was before the edit,
/// 0/0/0; the fixture's bytes are the same at the end.
#[test]
fn e2e_data_dialog_active_file_set_confirm_read_back_undo_and_diff() {
    let s = Scratch::new();
    let fixture = s.path("Fixture.esm");
    let fixture_bytes = std::fs::read(&fixture).unwrap();
    let patch = s.0.join("out").join("Patch.esp").display().to_string();
    let original = s.path("Original.esp");
    let Some(mut h) = workbench() else { return };
    let load = Request::new("load", json!({"plugins": [fixture], "no_cache": true}));
    h.state_mut()
        .app
        .apply(vec![Action::Submit(Slot::Load, load)]);
    settle(&mut h);

    // 1. The Data dialog: the scratch path typed into the active-file field, "Use This
    //    File", OK: it is the window's active file, and nothing is written yet.
    h.state_mut()
        .app
        .apply(vec![Action::OpenWindow(WindowKind::DataDialog)]);
    h.run_steps(3);
    settle(&mut h);
    input(&h, "Active file path:", false).focus();
    h.run_steps(1);
    input(&h, "Active file path:", false).type_text(&patch);
    h.run_steps(2);
    press(&mut h, "Use This File");
    press(&mut h, "OK");
    assert_eq!(h.state().app.active_file.as_deref(), Some(patch.as_str()));
    assert!(!Path::new(&patch).exists(), "the Data dialog wrote {patch}");

    // 2. The edit dialog on a record; a first write (000801) gives the active file its
    //    state before the edit under test, kept as `original`.
    h.state_mut()
        .app
        .shell
        .set_open(WindowKind::EditDialog, true);
    show(&mut h, "Fixture.esm:000801", "Glob801");
    retype(&mut h, "2.5", "9");
    press(&mut h, "OK");
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "written:");
    press(&mut h, "Close");
    std::fs::copy(&patch, &original).unwrap();

    // 3. The edit under test: OK runs the `set` dry run and writes nothing; the confirm
    //    writes it; `inspect` reads the new value back from the active file.
    show(&mut h, "Fixture.esm:000800", "Glob800");
    retype(&mut h, "1.5", "7.25");
    let before = std::fs::read(&patch).unwrap();
    press(&mut h, "OK");
    assert_in_view(&h, "Fields changed (dry run)");
    assert_eq!(std::fs::read(&patch).unwrap(), before, "the dry run wrote");
    assert!(enabled(&h, "Write to Patch.esp"));
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "written: FLTV 1.5 -> 7.25 into Patch.esp");
    assert_in_view(&h, "Read back from Patch.esp");
    assert_eq!(fltv_in(&patch, "00000800"), hex(7.25));
    assert_ne!(
        diff_counts(&original, &patch),
        [json!(0), json!(0), json!(0)]
    );
    press(&mut h, "Close");

    // 4. Undo last write: its dry run shows the one record going and writes nothing;
    //    the confirm drops it, and `diff` against the original shows no change.
    let written = std::fs::read(&patch).unwrap();
    press(&mut h, "Undo last write");
    assert_in_view(
        &h,
        "Diff of the file (dry run): 1 removed, 0 changed, 0 added",
    );
    assert_eq!(
        std::fs::read(&patch).unwrap(),
        written,
        "the undo's dry run wrote"
    );
    press(&mut h, "Undo in Patch.esp");
    assert_in_view(&h, "undone: 1 removed");
    assert_eq!(
        diff_counts(&original, &patch),
        [json!(0), json!(0), json!(0)],
        "the undo left a difference"
    );

    // The fixture is untouched.
    assert_eq!(
        std::fs::read(&fixture).unwrap(),
        fixture_bytes,
        "the fixture changed"
    );
}
