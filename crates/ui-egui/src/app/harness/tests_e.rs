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

/// Review finding L1 of M7.E6 (the M1 shape for `revert`): an undo that failed spends its
/// confirm. A folder stands where the active file is, so the undo fails. With the file
/// back, the old preview's button stays disabled and a click changes nothing. A new
/// Undo last write runs a new dry run, and its one undo restores the original.
#[test]
fn m7_e6_a_failed_undo_needs_a_new_undo_and_a_new_dry_run() {
    let s = Scratch::new("undo-failed");
    let patch = s.path("Patch.esp");
    let original = s.path("Original.esp");
    let aside = s.path("Patch.esp.aside");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    show(&mut h, "Fixture.esm:000801");
    write_field(&mut h, "2.5", "9");
    std::fs::copy(&patch, &original).unwrap();
    show(&mut h, FORM);
    write_field(&mut h, "1.5", "7.25");
    press(&mut h, "Undo last write");
    assert!(enabled(&h, "Undo in Patch.esp"));
    std::fs::rename(&patch, &aside).unwrap();
    std::fs::create_dir_all(&patch).unwrap();
    press(&mut h, "Undo in Patch.esp");
    assert_in_view(&h, "the undo:");
    assert!(
        !enabled(&h, "Undo in Patch.esp"),
        "re-armed on the old preview"
    );
    std::fs::remove_dir_all(&patch).unwrap();
    std::fs::rename(&aside, &patch).unwrap();
    one(&h, "Undo in Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert_eq!(
        diff_counts(&original, &patch),
        [json!(1), json!(0), json!(0)],
        "a second undo from one Undo"
    );
    press(&mut h, "Close");
    press(&mut h, "Undo last write");
    assert_in_view(&h, "Diff of the file (dry run): 1 removed");
    press(&mut h, "Undo in Patch.esp");
    assert_eq!(
        diff_counts(&original, &patch),
        [json!(0), json!(0), json!(0)]
    );
}

/// Review finding L3 of M7.E6: the undo assumes the active file is as the editor's write
/// left it. Another front end changing the file in between (here: its bytes rewritten
/// with one more byte) makes the undo refuse, and nothing is sent.
#[test]
fn m7_e6_an_undo_of_a_file_changed_since_the_write_is_refused() {
    let s = Scratch::new("undo-changed");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    write_field(&mut h, "1.5", "6");
    let mut bytes = std::fs::read(&patch).unwrap();
    bytes.push(0);
    std::fs::write(&patch, &bytes).unwrap();
    press(&mut h, "Undo last write");
    assert_in_view(&h, "changed since the editor's write");
    assert_absent(&h, "Undo in Patch.esp");
    assert_eq!(std::fs::read(&patch).unwrap(), bytes);
}

// ---- M7.E5: a build spec into the active file (`build`) --------------------------------

/// A one-`[[set]]` spec in the scratch folder: FLTV of GLOB 000800 to 7.25.
fn spec_file(s: &Scratch) -> String {
    let path = s.0.join("spec.toml");
    std::fs::write(
        &path,
        "version = 1\n\n[plugin]\nlocalized = false\n\n[[set]]\nform = \"Fixture.esm:000800\"\nfields = { FLTV = 7.25 }\n",
    )
    .unwrap();
    path.display().to_string()
}

/// The Build Spec form's spec box (exactly one).
fn spec_box(h: &Bench) -> Node<'_> {
    let all: Vec<Node<'_>> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::TextInput
                && n.label().as_deref() == Some("Spec file")
        })
        .collect();
    assert_eq!(
        all.len(),
        1,
        "one Spec file box
{}",
        visible_text(h)
    );
    all.into_iter().next().expect("checked above")
}

/// Opens the Build Spec form (as the menu's Build Patch will, through
/// `Action::OpenEditor`), types `spec` into it and presses Plan.
fn plan_build(h: &mut Bench, spec: &str) {
    h.state_mut().app.apply(vec![Action::OpenEditor {
        command: "build".to_string(),
        form: String::new(),
    }]);
    h.run_steps(3);
    spec_box(h).focus();
    h.run_steps(2);
    spec_box(h).type_text(spec);
    h.run_steps(3);
    press(h, "Plan");
}

/// M7.E5's acceptance in the window's own frame: a spec's plan lists the change and writes
/// nothing; one confirm builds the active file; the written record is read back (FLTV
/// 7.25); the confirm is then spent.
#[test]
fn m7_e5_a_spec_is_planned_built_into_the_active_file_and_read_back() {
    let s = Scratch::new("build");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    let spec = spec_file(&s);
    plan_build(&mut h, &spec);
    assert_in_view(&h, "Changes (plan; nothing written)");
    assert_in_view(&h, "Fixture.esm:000800 FLTV");
    assert!(!Path::new(&patch).exists(), "the plan wrote {patch}");
    assert!(enabled(&h, "Build into Patch.esp"));
    press(&mut h, "Build into Patch.esp");
    assert!(Path::new(&patch).is_file(), "the confirm did not build");
    assert_in_view(&h, "built:");
    assert_in_view(&h, "Read back from Patch.esp");
    assert_eq!(fltv_in(&patch), hex(7.25));
    assert!(!enabled(&h, "Build into Patch.esp"), "one build per Plan");
}

/// The build's refusals in the window: an active file that exists (no `overwrite`), no
/// active file, an official master or a `Default`-profile path as the active file are
/// refused before anything is sent; an active file changed after the plan closes the
/// confirm. Nothing is written in any of them.
#[test]
fn m7_e5_a_build_is_refused_where_it_would_replace_or_leave_the_active_file() {
    let s = Scratch::new("build-refused");
    let spec = spec_file(&s);
    let existing = s.path("Existing.esp");
    std::fs::write(&existing, b"x").unwrap();
    std::fs::create_dir_all(s.0.join("profiles").join("Default")).unwrap();
    let default =
        s.0.join("profiles")
            .join("Default")
            .join("Patch.esp")
            .display()
            .to_string();
    for (active, why) in [
        (Some(existing.clone()), "exists"),
        (None, "no active file"),
        (Some(s.path("Starfield.esm")), "official master"),
        (Some(default.clone()), "live game"),
    ] {
        let Some(mut h) = editing(&s, active.clone()) else {
            return;
        };
        plan_build(&mut h, &spec);
        assert_in_view(&h, why);
        assert_absent(&h, "Changes (plan; nothing written)");
    }
    assert_eq!(std::fs::read(&existing).unwrap(), b"x");
    assert!(!Path::new(&default).exists() && !Path::new(&s.path("Starfield.esm")).exists());
    // The active file changed after the plan: the confirm is closed.
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    plan_build(&mut h, &spec);
    assert!(enabled(&h, "Build into Patch.esp"));
    h.state_mut().app.active_file = Some(s.path("Other.esp"));
    h.run_steps(3);
    assert!(!enabled(&h, "Build into Patch.esp"));
    one(&h, "Build into Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert!(!Path::new(&patch).exists() && !Path::new(&s.path("Other.esp")).exists());
}

/// Review finding M1 of M7.E5 (probe RV-E5-B): the confirm covers the spec's content, not
/// only its path. The spec is rewritten after the plan (000800 to 99.5, and a second
/// record); the box still shows the old plan, so "Build into" stays disabled, a click
/// writes nothing, and a new Plan previews the new spec.
#[test]
fn m7_e5_a_spec_changed_after_the_plan_is_not_built() {
    let s = Scratch::new("build-spec-changed");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    let spec = spec_file(&s);
    plan_build(&mut h, &spec);
    assert_in_view(&h, "Fixture.esm:000800 FLTV");
    assert!(enabled(&h, "Build into Patch.esp"));
    std::fs::write(
        &spec,
        "version = 1\n\n[plugin]\nlocalized = false\n\n[[set]]\nform = \"Fixture.esm:000800\"\nfields = { FLTV = 99.5 }\n\n[[set]]\nform = \"Fixture.esm:000801\"\nfields = { FLTV = 42.0 }\n",
    )
    .unwrap();
    h.run_steps(3);
    assert!(
        !enabled(&h, "Build into Patch.esp"),
        "the old plan still confirms"
    );
    one(&h, "Build into Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert!(
        !Path::new(&patch).exists(),
        "a spec never previewed was built"
    );
    press(&mut h, "Plan");
    assert_in_view(&h, "99.5");
    press(&mut h, "Build into Patch.esp");
    assert_eq!(fltv_in(&patch), hex(99.5));
}

/// Review finding L1 of M7.E5 (probe RV-E5-C): an active file that appears between the
/// plan and the click (another writer) closes the confirm with the reason, and the file
/// stays as that writer left it.
#[test]
fn m7_e5_an_active_file_created_after_the_plan_closes_the_confirm() {
    let s = Scratch::new("build-appeared");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    plan_build(&mut h, &spec_file(&s));
    assert!(enabled(&h, "Build into Patch.esp"));
    std::fs::write(&patch, b"another writer's file").unwrap();
    h.run_steps(3);
    assert!(!enabled(&h, "Build into Patch.esp"));
    one(&h, "Build into Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert_eq!(std::fs::read(&patch).unwrap(), b"another writer's file");
}

// ---- M7.E3: New and Duplicate (`new`, `clone`) ------------------------------------------

/// The one text box labelled `label`.
fn labelled_box<'a>(h: &'a Bench, label: &'a str) -> Node<'a> {
    let all: Vec<Node<'a>> = h
        .query_all_by(move |n| {
            n.role() == egui::accesskit::Role::TextInput && n.label().as_deref() == Some(label)
        })
        .collect();
    assert_eq!(all.len(), 1, "one `{label}` box\n{}", visible_text(h));
    all.into_iter().next().expect("checked above")
}

/// Types `text` into the box labelled `label`.
fn type_into(h: &mut Bench, label: &str, text: &str) {
    labelled_box(h, label).focus();
    h.run_steps(2);
    labelled_box(h, label).type_text(text);
    h.run_steps(3);
}

/// Opens the New Form form for `command` on `form`, as the Object Window's New and
/// Duplicate (and the menu's Edit > Duplicate) ask for it: `Action::OpenEditor`. The
/// Object Window itself is W's (M7.W1) and not merged into ws/e yet, so the tests send
/// the action its buttons send.
fn open_new_form(h: &mut Bench, command: &str, form: &str) {
    h.state_mut().app.apply(vec![Action::OpenEditor {
        command: command.to_string(),
        form: form.to_string(),
    }]);
    h.run_steps(3);
}

/// M7.E3's Duplicate (`clone`), through the editor: the copy of GLOB 000800 is previewed
/// (nothing written), one confirm writes it into the active file as a new form, and its
/// EditorID is read back from there; the master is untouched.
#[test]
fn m7_e3_duplicate_previews_confirms_and_reads_back_the_copy() {
    let s = Scratch::new("clone");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    let master = std::fs::read(s.fixture()).unwrap();
    open_new_form(&mut h, "clone", FORM);
    assert_in_view(&h, "Copy of");
    assert!(!enabled(&h, "Preview"), "Preview waits for an EditorID");
    type_into(&mut h, "EditorID", "Glob800Copy");
    press(&mut h, "Preview");
    assert_in_view(
        &h,
        "duplicate Fixture.esm:000800 as Glob800Copy into Patch.esp",
    );
    assert!(!Path::new(&patch).exists(), "the preview wrote {patch}");
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "written: the new form");
    assert_in_view(&h, "Read back from Patch.esp");
    assert_eq!(
        std::fs::read(s.fixture()).unwrap(),
        master,
        "the master changed"
    );
    assert!(!enabled(&h, "Write to Patch.esp"), "one write per Preview");
    // The copy is a form of its own, read back by the FormID the write reports.
    let back = std::fs::read(&patch).unwrap();
    assert!(
        back.windows(12).any(|w| w == b"Glob800Copy\0"),
        "the copy's EditorID is not in the active file"
    );
}

/// M7.E3's New (`new`): a new GLOB of the typed type is previewed, one confirm writes it,
/// and it reads back from the active file.
#[test]
fn m7_e3_new_previews_confirms_and_reads_back_the_new_form() {
    let s = Scratch::new("new");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    open_new_form(&mut h, "new", "GLOB");
    type_into(&mut h, "EditorID", "KkNewGlobal");
    press(&mut h, "Preview");
    assert_in_view(&h, "new GLOB as KkNewGlobal into Patch.esp");
    assert!(!Path::new(&patch).exists(), "the preview wrote {patch}");
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "written: the new form");
    assert_in_view(&h, "Read back from Patch.esp");
    let back = std::fs::read(&patch).unwrap();
    assert!(back.windows(12).any(|w| w == b"KkNewGlobal\0"));
}

/// New and Duplicate are refused before anything is sent without an active file, or into
/// an official master or a `Default`-profile path; the confirm closes when the active
/// file changes after the preview. Nothing is written.
#[test]
fn m7_e3_new_and_duplicate_are_refused_outside_the_active_file() {
    let s = Scratch::new("new-refused");
    std::fs::create_dir_all(s.0.join("profiles").join("Default")).unwrap();
    let default =
        s.0.join("profiles")
            .join("Default")
            .join("Patch.esp")
            .display()
            .to_string();
    for (command, form) in [("clone", FORM), ("new", "GLOB")] {
        for (active, why) in [
            (None, "no active file"),
            (Some(s.path("SFBGS003.esm")), "official master"),
            (Some(default.clone()), "live game"),
        ] {
            let Some(mut h) = editing(&s, active.clone()) else {
                return;
            };
            open_new_form(&mut h, command, form);
            type_into(&mut h, "EditorID", "KkRefused");
            press(&mut h, "Preview");
            assert_in_view(&h, why);
            assert_absent(&h, "Write to");
        }
    }
    assert!(!Path::new(&default).exists() && !Path::new(&s.path("SFBGS003.esm")).exists());
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    open_new_form(&mut h, "clone", FORM);
    type_into(&mut h, "EditorID", "Glob800Moved");
    press(&mut h, "Preview");
    assert!(enabled(&h, "Write to Patch.esp"));
    h.state_mut().app.active_file = Some(s.path("Other.esp"));
    h.run_steps(3);
    assert!(!enabled(&h, "Write to Patch.esp"));
    one(&h, "Write to Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    assert!(!Path::new(&patch).exists() && !Path::new(&s.path("Other.esp")).exists());
}

/// Review finding L2 of M7.E3 (probe RV-E4-I): after a write, opening New again starts
/// clean. The old confirm box (its "written:" note) no longer covers the form, so a
/// second New in the same session previews and writes by mouse click.
#[test]
fn m7_e3_two_news_in_one_session_both_write() {
    let s = Scratch::new("new-twice");
    let patch = s.path("Patch.esp");
    let Some(mut h) = editing(&s, Some(patch.clone())) else {
        return;
    };
    for edid in ["KkFirstNew", "KkSecondNew"] {
        open_new_form(&mut h, "new", "GLOB");
        type_into(&mut h, "EditorID", edid);
        press(&mut h, "Preview");
        assert_in_view(&h, &format!("new GLOB as {edid} into Patch.esp"));
        press(&mut h, "Write to Patch.esp");
        assert_in_view(&h, "written: the new form");
    }
    let bytes = std::fs::read(&patch).unwrap();
    for edid in [&b"KkFirstNew\0"[..], &b"KkSecondNew\0"[..]] {
        assert!(bytes.windows(edid.len()).any(|w| w == edid), "{edid:?}");
    }
}

// ---- M7.E4: Place into the Cell View's cell (`place`) -----------------------------------

/// The base and the cell of the scratch world.
const STAT: &str = "World.esp:001000";
const CELL: &str = "World.esp:001001";

/// Builds `World.esp` in the scratch folder through the gate's own confirmed `new`s (a
/// STAT `KkStat`, World.esp:001000, and an interior CELL `KkCell`, World.esp:001001, over
/// `Fixture.esm`), then the workbench with `World.esp` loaded and `active` as the active
/// file.
fn placing(s: &Scratch, active: Option<String>) -> Option<Bench> {
    use crate::dialogs::new;
    let engine = ReadOnlyEngine::new();
    let fixture = s.fixture().display().to_string();
    engine
        .run(&Request::new(
            "load",
            json!({"plugins": [fixture], "no_cache": true}),
        ))
        .unwrap();
    let world = s.path("World.esp");
    if !Path::new(&world).exists() {
        for (ty, edid) in [("STAT", "KkStat"), ("CELL", "KkCell")] {
            let preview = new::preview_request(ty, edid, &world);
            engine.run(&preview).unwrap();
            engine.run(&new::write_request(&preview).unwrap()).unwrap();
        }
    }
    let mut h = workbench()?;
    let load = Request::new("load", json!({"plugins": [world], "no_cache": true}));
    h.state_mut().app.submit(Slot::Load, load);
    settle(&mut h);
    h.state_mut().app.active_file = active;
    h.run_steps(2);
    Some(h)
}

/// Asks to place `base` in `cell`, as the Cell View's place bar does
/// (`Action::PlaceInCell`). The Cell View on ws/e is the stub's and its list comes from
/// a real load; W's Cell View (M7.W2) is not merged, so the tests send its action.
fn ask_place(h: &mut Bench, base: &str, cell: &str) {
    h.state_mut().app.apply(vec![Action::PlaceInCell {
        base: base.to_string(),
        cell: cell.to_string(),
    }]);
    h.run_steps(3);
}

/// M7.E4's acceptance through the editor: a placement of KkStat in KkCell at a typed
/// position is previewed (the reference and the cell it carries; nothing written), one
/// confirm writes it into the active file, and the new reference is read back from there
/// (its NAME is the base). Its undo is refused with the reason.
#[test]
fn m7_e4_place_previews_confirms_and_reads_back_the_reference() {
    let s = Scratch::new("place");
    let patch = s.path("Patch.esp");
    let Some(mut h) = placing(&s, Some(patch.clone())) else {
        return;
    };
    let world = std::fs::read(s.path("World.esp")).unwrap();
    ask_place(&mut h, STAT, CELL);
    assert_in_view(&h, "Place Reference");
    type_into(&mut h, "X", "12.5");
    press(&mut h, "Preview");
    assert_in_view(
        &h,
        "place World.esp:001000 in World.esp:001001 into Patch.esp",
    );
    assert_in_view(&h, "Placed (dry run)");
    assert_in_view(&h, "carries CELL");
    assert!(!Path::new(&patch).exists(), "the preview wrote {patch}");
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "placed: Patch.esp:");
    assert_in_view(&h, "Read back from Patch.esp");
    assert_in_view(&h, "NAME");
    assert_eq!(
        std::fs::read(s.path("World.esp")).unwrap(),
        world,
        "the master changed"
    );
    assert!(!enabled(&h, "Write to Patch.esp"), "one write per Preview");
    // The undo of a placement is refused, with the reason (the copied CELL, and a new form
    // of the active file).
    let env = ReadOnlyEngine::new()
        .run(&Request::new(
            "inspect",
            json!({"form": "01001000", "plugin": patch}),
        ))
        .unwrap();
    assert_eq!(env["data"]["record"]["signature"], json!("REFR"), "{env}");
    let before = std::fs::read(&patch).unwrap();
    h.state_mut().app.apply(vec![Action::OpenEditor {
        command: "revert".to_string(),
        form: "Patch.esp:00001000".to_string(),
    }]);
    h.run_steps(3);
    assert_in_view(&h, "undo refused");
    assert_in_view(&h, "CELL");
    assert_absent(&h, "Undo in Patch.esp");
    assert_eq!(std::fs::read(&patch).unwrap(), before);
}

/// Place is refused before anything is sent without an active file, or into an official
/// master or a `Default`-profile path; a position that is not a number keeps Preview
/// disabled; the confirm closes when the active file changes after the preview. Nothing
/// is written.
#[test]
fn m7_e4_place_is_refused_outside_the_active_file() {
    let s = Scratch::new("place-refused");
    std::fs::create_dir_all(s.0.join("profiles").join("Default")).unwrap();
    let default =
        s.0.join("profiles")
            .join("Default")
            .join("Patch.esp")
            .display()
            .to_string();
    for (active, why) in [
        (None, "no active file"),
        (Some(s.path("Starfield.esm")), "official master"),
        (Some(default.clone()), "live game"),
    ] {
        let Some(mut h) = placing(&s, active.clone()) else {
            return;
        };
        ask_place(&mut h, STAT, CELL);
        press(&mut h, "Preview");
        assert_in_view(&h, why);
        assert_absent(&h, "Write to");
    }
    let patch = s.path("Patch.esp");
    let Some(mut h) = placing(&s, Some(patch.clone())) else {
        return;
    };
    ask_place(&mut h, STAT, CELL);
    type_into(&mut h, "Y", "north");
    assert!(!enabled(&h, "Preview"), "a position that is not a number");
    assert_in_view(&h, "position Y");
    press(&mut h, "Close");
    ask_place(&mut h, STAT, CELL);
    press(&mut h, "Preview");
    assert!(enabled(&h, "Write to Patch.esp"));
    h.state_mut().app.active_file = Some(s.path("Other.esp"));
    h.run_steps(3);
    assert!(!enabled(&h, "Write to Patch.esp"));
    one(&h, "Write to Patch.esp").click();
    h.run_steps(3);
    settle(&mut h);
    for f in [
        &patch,
        &default,
        &s.path("Starfield.esm"),
        &s.path("Other.esp"),
    ] {
        assert!(!Path::new(f).exists(), "{f} was written");
    }
}
