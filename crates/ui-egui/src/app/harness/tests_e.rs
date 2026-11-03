//! Pipeline E's harness tests (sprint 2026-11-02: the editor's writes, M7.E2 to M7.E6, and
//! the observed layouts, M7.W4).
//!
//! M7.E2 (`set`, ADR 0007 Amendment 4): the window's own frame edits a field of a scratch
//! fixture in the edit dialog, OK previews it, the confirm box writes it into the active
//! file, and the test reads it back with `inspect`; each refusal (no active file, an
//! official master or a `Default`-profile path as the active file, the active file
//! changed after the preview) writes nothing. The fixture is one GLOB made here
//! (`fake_mo2::plugin_bytes`), never the game's Data.

use std::path::{Path, PathBuf};

use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::{json, Value};

use super::*;
use crate::panels::cockpit::fake_mo2::plugin_bytes;
use crate::panels::common::Action;
use crate::windows::WindowKind;

/// The fixture's one GLOB and its value.
const FORM: &str = "Fixture.esm:000800";

/// A scratch folder under the OS temp dir with `Fixture.esm` (GLOB 000800 `Glob800`,
/// FLTV 1.5, and GLOB 000801 `Glob801`, FLTV 2.5); removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("kitkraft-ui-editor-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Fixture.esm"),
            plugin_bytes(&[], 1, &[0x800, 0x801], 1.5),
        )
        .unwrap();
        Self(dir)
    }

    fn fixture(&self) -> PathBuf {
        self.0.join("Fixture.esm")
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

/// The workbench with the fixture loaded, its GLOB inspected and the edit dialog open
/// (its default layout built from `schema`), and `active` as the active file.
fn editing(s: &Scratch, active: Option<String>) -> Option<Bench> {
    let mut h = workbench()?;
    let load = Request::new(
        "load",
        json!({"plugins": [s.fixture().display().to_string()], "no_cache": true}),
    );
    h.state_mut().app.submit(Slot::Load, load);
    settle(&mut h);
    let app = &mut h.state_mut().app;
    app.apply(vec![Action::Inspect(FORM.to_string())]);
    app.shell.set_open(WindowKind::EditDialog, true);
    app.active_file = active;
    settle(&mut h);
    assert_in_view(&h, "Glob800");
    Some(h)
}

/// The edit box showing `value` (exactly one).
fn field<'a>(h: &'a Bench, value: &'a str) -> Node<'a> {
    let all: Vec<Node<'a>> = h
        .query_all_by(move |n| {
            n.role() == egui::accesskit::Role::TextInput && n.value().as_deref() == Some(value)
        })
        .collect();
    assert_eq!(
        all.len(),
        1,
        "one edit box showing `{value}`\n{}",
        visible_text(h)
    );
    all.into_iter().next().expect("checked above")
}

/// Replaces the text of the edit box showing `from` with `to`, as a user types it.
fn retype(h: &mut Bench, from: &str, to: &str) {
    field(h, from).focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    field(h, from).type_text(to);
    h.run_steps(3);
    field(h, to);
}

/// Clicks the one button labelled `label`, then lets the engine answer.
fn press(h: &mut Bench, label: &str) {
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

/// FLTV of GLOB 000800 as `path` alone holds it, read through the gate's `inspect`.
fn fltv_in(path: &str) -> Value {
    let engine = ReadOnlyEngine::new();
    let env = engine
        .run(&Request::new(
            "inspect",
            json!({"form": "00000800", "plugin": path}),
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

/// M7.E2's acceptance: a field edited in the dialog, OK previews it (nothing written),
/// the confirm writes it into the active file, and `inspect` reads the new value back;
/// the fixture is untouched and a second click writes nothing more.
#[test]
fn m7_e2_a_field_edited_in_the_dialog_is_previewed_confirmed_and_read_back() {
    let s = Scratch::new("set");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    let master = std::fs::read(s.fixture()).unwrap();
    assert!(!enabled(&h, "OK"), "OK waits for an edit");
    retype(&mut h, "1.5", "7.25");
    assert_in_view(&h, "was 1.5");
    assert!(enabled(&h, "OK"));
    press(&mut h, "OK");
    // The preview: the dry run's change and its validate line; nothing written.
    assert_in_view(&h, "Fields changed (dry run)");
    assert_in_view(&h, "validate on the staged result");
    assert_in_view(&h, "7.25");
    assert!(!Path::new(&patch).exists(), "the preview wrote {patch}");
    assert!(enabled(&h, "Write to Patch.esp"));
    // The confirm: one write, then the read-back.
    press(&mut h, "Write to Patch.esp");
    assert!(Path::new(&patch).is_file(), "the confirm did not write");
    assert_in_view(&h, "written: FLTV 1.5 -> 7.25 into Patch.esp");
    assert_in_view(&h, "Read back from Patch.esp");
    assert_eq!(fltv_in(&patch), hex(7.25));
    assert_eq!(
        std::fs::read(s.fixture()).unwrap(),
        master,
        "the master changed"
    );
    // One write per OK: the confirm is spent.
    assert!(!enabled(&h, "Write to Patch.esp"));
    let written = std::fs::read(&patch).unwrap();
    one(&h, "Write to Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert_eq!(std::fs::read(&patch).unwrap(), written);
    // The edits are dropped after the write: the box shows the record's value again.
    field(&h, "1.5");
}

/// Without an active file, OK says to choose one and nothing is sent or written.
#[test]
fn m7_e2_without_an_active_file_ok_writes_nothing() {
    let s = Scratch::new("none");
    let Some(mut h) = editing(&s, None) else {
        return;
    };
    retype(&mut h, "1.5", "2");
    press(&mut h, "OK");
    assert_in_view(&h, "no active file");
    assert_absent(&h, "Fields changed (dry run)");
    let files: Vec<_> = std::fs::read_dir(&s.0).unwrap().collect();
    assert_eq!(files.len(), 1, "only the fixture: {files:?}");
}

/// An official master or a path in the profile `Default` as the active file: the gate's
/// check refuses the preview in the window, nothing is sent and nothing is written.
#[test]
fn m7_e2_an_official_master_or_default_path_as_active_file_is_refused() {
    let s = Scratch::new("refused");
    std::fs::create_dir_all(s.0.join("profiles").join("Default")).unwrap();
    for (active, why) in [
        (s.path("Starfield.esm"), "official master"),
        (s.path("SFBGS003.esm"), "official master"),
        (
            s.0.join("profiles")
                .join("Default")
                .join("Patch.esp")
                .display()
                .to_string(),
            "live game",
        ),
    ] {
        let Some(mut h) = editing(&s, Some(active.clone())) else {
            return;
        };
        retype(&mut h, "1.5", "3");
        press(&mut h, "OK");
        assert_in_view(&h, why);
        assert_absent(&h, "Fields changed (dry run)");
        assert!(!Path::new(&active).exists(), "{active} was written");
    }
}

/// The active file changed after the preview: the confirm stays disabled (the dialog
/// compares the output with the active file), and nothing is written to either file.
#[test]
fn m7_e2_the_confirm_is_closed_when_the_active_file_changed_after_the_preview() {
    let s = Scratch::new("moved");
    let patch = s.path("Patch.esp");
    let other = s.path("Other.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    retype(&mut h, "1.5", "4");
    press(&mut h, "OK");
    assert_in_view(&h, "Fields changed (dry run)");
    assert!(enabled(&h, "Write to Patch.esp"));
    h.state_mut().app.active_file = Some(other.clone());
    h.run_steps(3);
    assert!(!enabled(&h, "Write to Patch.esp"));
    one(&h, "Write to Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert!(!Path::new(&patch).exists() && !Path::new(&other).exists());
}

/// The bytes of `sig` in GLOB 000800 as `path` alone holds them (raw, single-plugin).
fn payload_in(path: &str, sig: &str) -> Value {
    let engine = ReadOnlyEngine::new();
    let env = engine
        .run(&Request::new(
            "inspect",
            json!({"form": "00000800", "plugin": path, "full": true}),
        ))
        .unwrap();
    env["data"]["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == json!(sig))
        .map(|s| s["payload_hex"].clone())
        .unwrap()
}

/// Review finding H1 (probe R-P6): a second edit of the same record keeps the first.
/// `set` starts from the load order's winner, so the editor reads the active file's
/// version of the record and carries what it already holds into the next write; the
/// confirm box shows the whole difference that will be written. Two sequential edits
/// on one record, both read back.
#[test]
fn m7_e2_a_second_edit_of_the_same_record_keeps_the_first() {
    let s = Scratch::new("twice");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    retype(&mut h, "1.5", "7.25");
    press(&mut h, "OK");
    press(&mut h, "Write to Patch.esp");
    assert_eq!(fltv_in(&patch), hex(7.25));
    press(&mut h, "Close");
    retype(&mut h, "Glob800", "Glob800x");
    press(&mut h, "OK");
    // The full difference: the kept FLTV and the new EditorID.
    assert_in_view(&h, "kept from Patch.esp: FLTV");
    assert_in_view(&h, "Glob800x");
    assert!(enabled(&h, "Write to Patch.esp"));
    press(&mut h, "Write to Patch.esp");
    assert_eq!(
        payload_in(&patch, "FLTV"),
        hex(7.25),
        "the first edit was lost"
    );
    assert_eq!(
        payload_in(&patch, "EDID"),
        json!("476C6F623830307800"),
        "EDID Glob800x"
    );
}

/// Review finding M1 (probe R-P5): a write that failed spends the confirm. The button
/// stays disabled on the old preview, so one OK sends one write; the next OK runs a new
/// dry run and asks again.
#[test]
fn m7_e2_a_failed_write_needs_a_new_ok_and_a_new_dry_run() {
    let s = Scratch::new("failed");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    retype(&mut h, "1.5", "5");
    press(&mut h, "OK");
    assert!(enabled(&h, "Write to Patch.esp"));
    // The write fails: a folder stands where the active file would be written.
    std::fs::create_dir_all(&patch).unwrap();
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "the write");
    assert!(
        !enabled(&h, "Write to Patch.esp"),
        "re-armed on the old preview"
    );
    std::fs::remove_dir_all(&patch).unwrap();
    one(&h, "Write to Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert!(!Path::new(&patch).exists(), "a second write from one OK");
    // A new OK: a new dry run, then one write.
    press(&mut h, "OK");
    assert_in_view(&h, "Fields changed (dry run)");
    assert!(enabled(&h, "Write to Patch.esp"));
    press(&mut h, "Write to Patch.esp");
    assert_eq!(fltv_in(&patch), hex(5.0));
}

// ---- M7.E6: undo of the editor's last write (`revert`) ---------------------------------

/// Inspects `form` in the Records tab, so the edit dialog shows it.
fn show(h: &mut Bench, form: &str) {
    h.state_mut()
        .app
        .apply(vec![Action::Inspect(form.to_string())]);
    settle(h);
}

/// Edits the box showing `from` to `to`, OK, the confirm, then closes the box.
fn write_field(h: &mut Bench, from: &str, to: &str) {
    retype(h, from, to);
    press(h, "OK");
    press(h, "Write to Patch.esp");
    assert_in_view(h, "written:");
    press(h, "Close");
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

/// M7.E6's acceptance: edit, undo, and `diff` against the original shows no change. The
/// active file holds a write of 000801 (the original); a write of 000800 is undone from
/// the edit dialog: the dry run shows the one record going and writes nothing, one
/// confirm drops it, and the file has no difference from the original. A second undo
/// has nothing to undo.
#[test]
fn m7_e6_edit_undo_and_diff_against_the_original_shows_no_change() {
    let s = Scratch::new("undo");
    let patch = s.path("Patch.esp");
    let original = s.path("Original.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    show(&mut h, "Fixture.esm:000801");
    write_field(&mut h, "2.5", "9");
    std::fs::copy(&patch, &original).unwrap();
    show(&mut h, FORM);
    write_field(&mut h, "1.5", "7.25");
    assert_ne!(
        diff_counts(&original, &patch),
        [json!(0), json!(0), json!(0)]
    );
    let before = std::fs::read(&patch).unwrap();
    press(&mut h, "Undo last write");
    assert_in_view(
        &h,
        "Diff of the file (dry run): 1 removed, 0 changed, 0 added",
    );
    assert_eq!(std::fs::read(&patch).unwrap(), before, "the dry run wrote");
    assert!(enabled(&h, "Undo in Patch.esp"));
    press(&mut h, "Undo in Patch.esp");
    assert_in_view(&h, "undone: 1 removed");
    assert_eq!(
        diff_counts(&original, &patch),
        [json!(0), json!(0), json!(0)],
        "the undo left a difference"
    );
    // One undo per write.
    assert!(!enabled(&h, "Undo in Patch.esp"));
    press(&mut h, "Close");
    press(&mut h, "Undo last write");
    assert_in_view(&h, "nothing to undo");
    assert_absent(&h, "Undo in Patch.esp");
}

/// An undo that would revert more than one write is refused: the last write replaced a
/// version the active file already held (a second edit of the record), and `revert`
/// would drop both. Nothing is sent and nothing changes.
#[test]
fn m7_e6_an_undo_that_would_drop_more_than_one_write_is_refused() {
    let s = Scratch::new("undo-two");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    write_field(&mut h, "1.5", "7.25");
    write_field(&mut h, "Glob800", "Glob800x");
    let before = std::fs::read(&patch).unwrap();
    press(&mut h, "Undo last write");
    assert_in_view(&h, "undo refused");
    assert_absent(&h, "Undo in Patch.esp");
    assert_eq!(std::fs::read(&patch).unwrap(), before);
}

/// The undo's confirm is closed when the active file changed after its preview, and an
/// undo with no write this session has nothing to undo.
#[test]
fn m7_e6_the_undo_confirm_follows_the_active_file() {
    let s = Scratch::new("undo-moved");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    press(&mut h, "Undo last write");
    assert_in_view(&h, "nothing to undo");
    press(&mut h, "Close");
    write_field(&mut h, "1.5", "3");
    let before = std::fs::read(&patch).unwrap();
    press(&mut h, "Undo last write");
    assert!(enabled(&h, "Undo in Patch.esp"));
    h.state_mut().app.active_file = Some(s.path("Other.esp"));
    h.run_steps(3);
    assert!(!enabled(&h, "Undo in Patch.esp"));
    one(&h, "Undo in Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert_eq!(std::fs::read(&patch).unwrap(), before);
}

/// Review finding M1 of M7.E6 (the reviewer's engine probe, here in the window's own
/// frame; needs the corpus, skipped without KITKRAFT_DATA_DIR). Editing a placed
/// reference copies its parents into the active file: the exterior CELL and its WRLD,
/// both `inserted`. `revert` drops only the REFR, so an undo would leave about 0.9 MB of
/// copied parents behind (diff against the original: 2 added). The undo is refused,
/// with the reason, and nothing is sent.
#[test]
fn m7_e6_an_undo_that_would_leave_copied_parents_behind_is_refused() {
    let Some(data) = std::env::var_os("KITKRAFT_DATA_DIR") else {
        eprintln!("skipped: KITKRAFT_DATA_DIR is not set");
        return;
    };
    if !Path::new(&data).join("Starfield.esm").is_file() {
        eprintln!("skipped: no Starfield.esm in KITKRAFT_DATA_DIR");
        return;
    }
    let s = Scratch::new("undo-refr");
    let patch = s.path("Patch.esp");
    let Some(mut h) = workbench() else {
        return;
    };
    let load = Request::new("load", json!({"plugins": ["Starfield.esm"]}));
    h.state_mut().app.submit(Slot::Load, load);
    settle(&mut h);
    let app = &mut h.state_mut().app;
    app.apply(vec![Action::Inspect("Starfield.esm:000807".to_string())]);
    app.shell.set_open(WindowKind::EditDialog, true);
    app.active_file = Some(patch.clone());
    settle(&mut h);
    // XSCL (Scale) 2 -> 3, previewed and written: the REFR and its two parents.
    retype(&mut h, "2.0", "3");
    press(&mut h, "OK");
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "written:");
    press(&mut h, "Close");
    let before = std::fs::read(&patch).unwrap();
    press(&mut h, "Undo last write");
    assert_in_view(&h, "undo refused");
    assert_in_view(&h, "CELL");
    assert_absent(&h, "Undo in Patch.esp");
    assert_eq!(std::fs::read(&patch).unwrap(), before);
}
