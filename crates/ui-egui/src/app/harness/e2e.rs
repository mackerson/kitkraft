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
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("kitkraft-ui-e2e-{tag}-{}", std::process::id()));
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
    let s = Scratch::new("set");
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

// ---- wave 2: Duplicate from the Object Window, Place into the Cell View's cell, Undo -----

/// Builds `World.esp` beside the fixture through the engine's own confirmed writes, as
/// pipeline E's place tests do: a STAT `KkStat` (World.esp:001000) and an interior CELL
/// `KkCell` (World.esp:001001) over `Fixture.esm`, and `Seed.esp` with one reference of
/// KkStat in KkCell (so the Cell View lists the cell) and a STAT `KkSeedStat`. Returns
/// (World.esp, Seed.esp).
fn world(s: &Scratch) -> (String, String) {
    use crate::dialogs::new;
    let engine = ReadOnlyEngine::new();
    let fixture = s.path("Fixture.esm");
    engine
        .run(&Request::new(
            "load",
            json!({"plugins": [fixture], "no_cache": true}),
        ))
        .unwrap();
    let world = s.path("World.esp");
    for (ty, edid) in [("STAT", "KkStat"), ("CELL", "KkCell")] {
        let preview = new::preview_request(ty, edid, &world);
        engine.run(&preview).unwrap();
        engine.run(&new::write_request(&preview).unwrap()).unwrap();
    }
    // One reference in KkCell, so the Cell View lists the cell (its list comes from the
    // cells' child records; a cell with none is not listed, `cell`'s limitation).
    use crate::dialogs::place;
    engine
        .run(&Request::new(
            "load",
            json!({"plugins": [world], "no_cache": true}),
        ))
        .unwrap();
    let seed = s.path("Seed.esp");
    let preview = place::preview_request(STAT, CELL, [0.0, 0.0, 0.0], &seed);
    engine.run(&preview).unwrap();
    engine
        .run(&place::write_request(&preview).unwrap())
        .unwrap();
    // A STAT of Seed.esp's own (KkSeedStat), the record the test duplicates: the active
    // file then has Seed.esp, which holds KkCell's winning version, among its masters
    // before the placement (`place` refuses to add a master to a file that has records
    // of its own).
    let preview = new::preview_request("STAT", "KkSeedStat", &seed);
    engine.run(&preview).unwrap();
    engine.run(&new::write_request(&preview).unwrap()).unwrap();
    (world, seed)
}

/// The fixture world's base (KkStat) and its interior cell (KkCell).
const STAT: &str = "World.esp:001000";
const CELL: &str = "World.esp:001001";

/// Types `text` into the box labelled `label`.
fn type_into(h: &mut Bench, label: &str, text: &str) {
    input(h, label, false).focus();
    h.run_steps(2);
    input(h, label, false).type_text(text);
    h.run_steps(3);
}

/// Clicks the first node whose text is `text` (a list row).
fn click_text(h: &mut Bench, text: &str) {
    let count = h
        .query_all_by(|n| text_of(n).as_deref() == Some(text))
        .count();
    assert!(count > 0, "no `{text}`\n{}", visible_text(h));
    h.query_all_by(|n| text_of(n).as_deref() == Some(text))
        .next()
        .expect("checked above")
        .click();
    h.run_steps(2);
    settle(h);
}

/// The Data dialog's active file, typed and used, as in the wave-1 check.
fn choose_active_file(h: &mut Bench, path: &str) {
    h.state_mut()
        .app
        .apply(vec![Action::OpenWindow(WindowKind::DataDialog)]);
    h.run_steps(3);
    settle(h);
    input(h, "Active file path:", false).focus();
    h.run_steps(1);
    input(h, "Active file path:", false).type_text(path);
    h.run_steps(2);
    press(h, "Use This File");
    press(h, "OK");
    assert_eq!(h.state().app.active_file.as_deref(), Some(path));
}

/// The wave-2 end-to-end check (TRACK "Merge order per wave", wave 2): the Data dialog's
/// active file -> the Object Window lists STAT -> Duplicate on KkStat (`clone`: Preview,
/// confirm, the copy's EditorID read back) -> the Cell View shows KkCell and its place bar
/// asks to place the inspected base there (`Action::PlaceInCell`) -> the Place form (a
/// typed position, Preview with the carried CELL and "at (x, y, z)", confirm, the REFR's
/// NAME and DATA read back) -> "Undo last write" on the placed reference is refused with
/// its reason (the placement copied the CELL parent; M7.E4's design) and the file is
/// unchanged -> the fixtures are unchanged.
#[test]
fn e2e_object_window_duplicate_cell_view_place_and_refused_undo() {
    let s = Scratch::new("place");
    let (world, seed) = world(&s);
    let fixture = s.path("Fixture.esm");
    let fixtures = [
        (fixture.clone(), std::fs::read(&fixture).unwrap()),
        (world.clone(), std::fs::read(&world).unwrap()),
        (seed.clone(), std::fs::read(&seed).unwrap()),
    ];
    let patch = s.0.join("out").join("Patch.esp").display().to_string();
    let Some(mut h) = workbench() else { return };
    let load = Request::new("load", json!({"plugins": [world, seed], "no_cache": true}));
    h.state_mut()
        .app
        .apply(vec![Action::Submit(Slot::Load, load)]);
    settle(&mut h);

    // 1. The active file from the Data dialog.
    choose_active_file(&mut h, &patch);

    // 2. The Object Window lists STAT (its tree node, wheeled into view), and KkStat is
    //    selected in its list.
    h.state_mut()
        .app
        .apply(vec![Action::OpenWindow(WindowKind::ObjectWindow)]);
    h.run_steps(3);
    settle(&mut h);
    let is_stat = |n: &egui_kittest::kittest::AccessKitNode<'_>| {
        text_of(n).is_some_and(|t| t.starts_with("STAT ")) && n.toggled().is_some()
    };
    // Wheel the tree until the node lies inside the tree's own scroll area (below the
    // category note, above the window's bottom edge), not only inside the viewport.
    let inside = |r: egui::Rect| r.min.y > 300.0 && r.max.y < 560.0;
    for _ in 0..200 {
        let at = h
            .query_all_by(is_stat)
            .next()
            .map(|n| n.rect())
            .filter(|r| r.min.y.is_finite());
        match at {
            Some(r) if inside(r) => break,
            Some(r) if r.min.y <= 300.0 => {
                h.query_all_by(is_stat).next().unwrap().scroll_up();
            }
            _ => {
                h.query_all_by(|n| text_of(n).as_deref() == Some("*All") || is_stat(n))
                    .next()
                    .unwrap()
                    .scroll_down();
            }
        }
        h.run_steps(10);
    }
    h.run_steps(30);
    let stat = h.query_all_by(is_stat).next().unwrap();
    assert!(inside(stat.rect()), "STAT node at {:?}", stat.rect());
    stat.click();
    h.run_steps(2);
    settle(&mut h);
    assert_in_view(&h, "KkSeedStat");
    click_text(&mut h, "KkSeedStat");

    // 3. Duplicate (`clone`): Preview writes nothing, the confirm writes the copy, and its
    //    EditorID is read back from the active file.
    press(&mut h, "Duplicate");
    assert_in_view(&h, "Copy of");
    type_into(&mut h, "EditorID", "KkSeedStatCopy");
    press(&mut h, "Preview");
    assert_in_view(&h, "as KkSeedStatCopy into Patch.esp");
    assert!(!Path::new(&patch).exists(), "the preview wrote {patch}");
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "written: the new form");
    assert_in_view(&h, "Read back from Patch.esp");
    let copy = std::fs::read(&patch).unwrap();
    assert!(
        copy.windows(15).any(|w| w == b"KkSeedStatCopy\0"),
        "the copy's EditorID is not in the active file"
    );

    // The Duplicate form and its confirm box are closed (they float over the Cell View).
    for _ in 0..3 {
        let Some(close) = h
            .query_all_by_label("Close")
            .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        else {
            break;
        };
        close.click();
        h.run_steps(3);
        settle(&mut h);
    }
    assert_absent(&h, "Copy of");

    // 4. The Cell View shows KkCell; with KkStat inspected, its place bar asks to place it
    //    there. The Object Window is closed first: it floats over the Cell View's list.
    h.state_mut()
        .app
        .shell
        .set_open(WindowKind::ObjectWindow, false);
    h.run_steps(2);
    // KkStat is inspected first (Records), the shared selection the place bar reads.
    h.state_mut()
        .app
        .apply(vec![Action::Inspect(STAT.to_string())]);
    settle(&mut h);
    open_tab(&mut h, Tab::CellView);
    click_text(&mut h, "Interior");
    click_text(&mut h, "KkCell");
    let bar: Vec<String> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::Button
                && text_of(n).is_some_and(|t| t.starts_with("Place ") && t.ends_with(" here"))
        })
        .filter_map(|n| text_of(&n.accesskit_node()))
        .collect();
    assert_eq!(bar.len(), 1, "one place bar button\n{}", visible_text(&h));
    let bar = bar.into_iter().next().expect("checked above");
    press(&mut h, &bar);

    // 5. The Place form: a typed position; Preview shows the carried CELL and the
    //    position and writes nothing; the confirm writes the REFR, read back.
    assert_in_view(&h, "Place Reference");
    type_into(&mut h, "X", "12.5");
    press(&mut h, "Preview");
    assert_in_view(&h, "Placed (dry run)");
    assert_in_view(&h, "carries CELL");
    assert_in_view(&h, "at (12.5, 0, 0)");
    assert_eq!(std::fs::read(&patch).unwrap(), copy, "the preview wrote");
    press(&mut h, "Write to Patch.esp");
    assert_in_view(&h, "placed: Patch.esp:");
    assert_in_view(&h, "Read back from Patch.esp");
    let placed = visible_text(&h)
        .lines()
        .find_map(|l| {
            let at = l.find("placed: Patch.esp:")?;
            l.get(at + "placed: ".len()..)?
                .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ')')
                .next()
                .map(str::to_string)
        })
        .expect("the placed reference's formref");
    let env = ReadOnlyEngine::new()
        .run(&Request::new(
            "inspect",
            json!({"form": placed, "plugins": [world, seed, patch], "no_cache": true, "full": true}),
        ))
        .unwrap();
    assert_eq!(env["data"]["signature"], json!("REFR"), "{env}");
    let winner = &env["data"]["versions"][0];
    assert_eq!(winner["plugin"], json!("Patch.esp"), "{env}");
    let sub = |sig: &str| {
        winner["subrecords"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["signature"] == json!(sig))
            .cloned()
            .unwrap_or_else(|| panic!("no {sig} in {env}"))
    };
    // NAME is the base, KkStat; DATA starts with the typed position (X, Y, Z as f32).
    assert_eq!(sub("NAME")["value"]["form"], json!(STAT), "{env}");
    let xyz: String = [12.5f32, 0.0, 0.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .map(|b| format!("{b:02X}"))
        .collect();
    let data = sub("DATA")["payload_hex"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(data.starts_with(&xyz), "DATA {data}");

    // 6. "Undo last write" on the placed reference is refused with its reason, and the
    //    active file is unchanged.
    let before = std::fs::read(&patch).unwrap();
    // The placed reference lives only in the active file: load it too, so the Records
    // inspector (and the edit dialog) can show it.
    let load = Request::new(
        "load",
        json!({"plugins": [world, seed, patch], "no_cache": true}),
    );
    h.state_mut()
        .app
        .apply(vec![Action::Submit(Slot::Load, load)]);
    settle(&mut h);
    h.state_mut()
        .app
        .apply(vec![Action::Inspect(placed.clone())]);
    h.state_mut()
        .app
        .shell
        .set_open(WindowKind::EditDialog, true);
    settle(&mut h);
    press(&mut h, "Undo last write");
    assert_in_view(&h, "undo refused");
    assert_in_view(&h, "CELL");
    assert_absent(&h, "Undo in Patch.esp");
    assert_eq!(
        std::fs::read(&patch).unwrap(),
        before,
        "the refused undo wrote"
    );

    // The fixtures are untouched.
    for (path, bytes) in &fixtures {
        assert_eq!(&std::fs::read(path).unwrap(), bytes, "{path} changed");
    }
}
