//! Pipeline W's harness tests (sprint 2026-11-02: the Data dialog, the menu bar, the
//! Object Window and Preferences; M7.M1 to M7.M3, M7.W1).

use super::*;

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::panels::cockpit::fake_mo2::plugin_bytes;
use crate::panels::common::Action;
use crate::windows::WindowKind;

// ---- the Data dialog (M7.M2) ---------------------------------------------------------------

/// A throwaway folder for one test (never the game's Data folder or MO2).
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("kitkraft-ui-w-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> String {
        let p = self.0.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p.display().to_string()
    }

    /// Every file below the folder and its bytes.
    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![self.0.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    let b = std::fs::read(&p).unwrap();
                    out.push((p, b));
                }
            }
        }
        out.sort();
        out
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `plugin_bytes` with a `CNAM` (author) after `HEDR`, the record's size fixed up.
fn with_author(mut bytes: Vec<u8>, author: &str) -> Vec<u8> {
    let mut cnam = b"CNAM".to_vec();
    let mut text = author.as_bytes().to_vec();
    text.push(0);
    cnam.extend((text.len() as u16).to_le_bytes());
    cnam.extend(text);
    // TES4 header (24 bytes), then HEDR (6 + 12): CNAM goes after it.
    let at = 24 + 18;
    let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) + cnam.len() as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes.splice(at..at, cnam);
    bytes
}

/// The dialog's row named `name`: the one selectable label with that text.
fn row<'a>(h: &'a Bench, name: &'a str) -> Node<'a> {
    let rows: Vec<Node<'a>> = h
        .query_all_by_label(name)
        .filter(|n| n.accesskit_node().toggled().is_some())
        .collect();
    assert_eq!(rows.len(), 1, "one `{name}` row in the Data dialog");
    rows.into_iter().next().expect("checked above")
}

/// How many nodes are labelled exactly `text`.
fn labelled(h: &Bench, text: &str) -> usize {
    h.query_all_by(|n| text_of(n).as_deref() == Some(text))
        .count()
}

fn click_row(h: &mut Bench, name: &str) {
    row(h, name).click();
    h.run_steps(2);
    settle(h);
}

fn click_button(h: &mut Bench, label: &str) {
    let b: Vec<Node<'_>> = h
        .query_all_by_label(label)
        .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .collect();
    assert_eq!(b.len(), 1, "one `{label}` button");
    b.into_iter().next().expect("checked above").click();
    h.run_steps(2);
    settle(h);
}

fn open_data_dialog(h: &mut Bench) {
    h.state_mut()
        .app
        .apply(vec![Action::OpenWindow(WindowKind::DataDialog)]);
    h.run_steps(3);
    settle(h);
    assert!(h.state().app.shell.is_open(WindowKind::DataDialog));
}

/// M7.M2 over the real engine: a plugin list loaded, the Data dialog lists it with the
/// CK's statuses, refuses the master-flagged plugin and the official master as the
/// active file (the refusal read from the engine's `load` result), shows a plugin's
/// `TES4` from `inspect`, and OK makes the plugin file the active file. Nothing on disk
/// changes.
#[test]
fn the_data_dialog_sets_a_plugin_file_active_and_refuses_masters() {
    let Some(mut h) = workbench() else { return };
    let dir = Scratch::new("data");
    let base = dir.write(
        "Base.esm",
        &with_author(plugin_bytes(&[], 1, &[0x800], 1.0), "KitAuthor"),
    );
    // An official name without the master flag: refused by name.
    let official = dir.write("sub/Starfield.esm", &plugin_bytes(&[], 0, &[0x801], 2.0));
    let mine = dir.write(
        "Mine.esp",
        &plugin_bytes(&["Base.esm"], 0, &[0x0100_0800], 3.0),
    );
    let before = dir.snapshot();
    let load = Request::new(
        "load",
        json!({"plugins": [base, official, mine], "no_cache": true}),
    );
    h.state_mut()
        .app
        .apply(vec![Action::Submit(Slot::Load, load)]);
    settle(&mut h);
    // The engine's `load` result is what the refusal reads: Base.esm is master-flagged.
    let flags: Vec<String> = h
        .state()
        .app
        .load
        .view
        .as_ref()
        .unwrap()
        .plugins
        .iter()
        .map(|p| format!("{}={}", p.name, p.flags))
        .collect();
    assert!(
        flags.iter().any(|f| f.starts_with("Base.esm=master")),
        "{flags:?}"
    );
    assert!(flags.iter().any(|f| f == "Mine.esp="), "{flags:?}");

    open_data_dialog(&mut h);
    assert_in_view(&h, "Active file:");
    assert_in_view(&h, "Master File");
    assert_in_view(&h, "Plugin File");

    // The master-flagged plugin: its TES4 shown, then refused.
    click_row(&mut h, "Base.esm");
    assert_in_view(&h, "Author: KitAuthor");
    click_button(&mut h, "Set as Active File");
    assert_in_view(&h, "has the master flag");
    assert_eq!(h.state().app.active_file, None);
    // The official master, by name.
    click_row(&mut h, "Starfield.esm");
    click_button(&mut h, "Set as Active File");
    assert_in_view(&h, "is an official master");
    assert_eq!(labelled(&h, "Active File"), 0, "no row is the active file");

    // The plugin file: its masters from TES4, then the active file on OK.
    click_row(&mut h, "Mine.esp");
    assert_in_view(&h, "Masters: Base.esm");
    click_button(&mut h, "Set as Active File");
    assert_absent(&h, "has the master flag");
    assert!(labelled(&h, "Active File") > 0, "a row shows Active File");
    assert_eq!(h.state().app.shell.data_dialog.status(2), "Active File");
    assert_eq!(h.state().app.active_file, None, "applied on OK only");
    click_button(&mut h, "OK");
    assert_eq!(h.state().app.active_file.as_deref(), Some(mine.as_str()));
    assert!(!h.state().app.shell.is_open(WindowKind::DataDialog));
    // Ticks unchanged: OK sent no second load.
    assert_eq!(h.state().app.load.data, 1);

    // Reopened, it shows the window's active file; Cancel keeps it.
    open_data_dialog(&mut h);
    assert_in_view(&h, &mine);
    click_button(&mut h, "Cancel");
    assert_eq!(h.state().app.active_file.as_deref(), Some(mine.as_str()));
    assert_eq!(dir.snapshot(), before, "the dialog writes nothing");
}

/// M7.M2: unticking a plugin (its check box; the CK's double-click is wired to the same
/// toggle, `DataDialogState::toggle`, but egui_kittest's clicks do not make a double-click)
/// and OK loads the ticked plugins
/// through `load`, a read the gate passes, and the Load tab follows.
#[test]
fn ok_reloads_the_ticked_plugins() {
    let Some(mut h) = workbench() else { return };
    let dir = Scratch::new("tick");
    let base = dir.write("Base.esm", &plugin_bytes(&[], 1, &[0x800], 1.0));
    let mine = dir.write("Mine.esp", &plugin_bytes(&[], 0, &[0x800], 3.0));
    let load = Request::new("load", json!({"plugins": [base, mine], "no_cache": true}));
    h.state_mut()
        .app
        .apply(vec![Action::Submit(Slot::Load, load)]);
    settle(&mut h);
    open_data_dialog(&mut h);
    // The rows' check boxes, in list order (the dialog is the only window open).
    let boxes: Vec<Node<'_>> = h
        .query_all_by(|n| n.role() == egui::accesskit::Role::CheckBox)
        .collect();
    assert_eq!(boxes.len(), 2, "one check box per row");
    boxes[1].click();
    h.run_steps(2);
    settle(&mut h);
    assert!(
        !h.state().app.shell.data_dialog.rows[1].ticked,
        "the box unticks the row"
    );
    click_button(&mut h, "OK");
    let names = h.state().app.load.view.as_ref().unwrap().names();
    assert_eq!(names, ["Base.esm"]);
    assert_eq!(h.state().app.active_file, None);
}

/// M7.M2 and AGENTS.md: a session loaded from the `Default` profile (the owner's live
/// game) gives no active file, whatever is selected; OK sends nothing and the profile's
/// files are unchanged.
#[test]
fn the_default_profile_gives_no_active_file() {
    let Some(mut h) = workbench() else { return };
    let dir = Scratch::new("default");
    let profile = dir.0.join("profiles").join("Default");
    dir.write("profiles/Default/plugins.txt", b"*Mine.esp\r\n");
    dir.write(
        "profiles/Default/loadorder.txt",
        b"Starfield.esm\r\nMine.esp\r\n",
    );
    let mine = dir.write("mods/Mine/Mine.esp", &plugin_bytes(&[], 0, &[0x800], 1.0));
    let before = dir.snapshot();
    let env = json!({
        "command": "load",
        "data": {"loaded": true, "source": format!("profile:{}", profile.display()),
                 "plugins": [{"index": 0, "name": "Mine.esp", "path": mine, "flag_names": [],
                              "active": "plugins.txt", "provider": "mod:Mine"}]},
        "limitations": [], "coverage": {},
    });
    let req = Request::new("load", json!({"profile": profile.display().to_string()}));
    inject(&mut h, Slot::Load, req, Ok(env));
    open_data_dialog(&mut h);
    click_row(&mut h, "Mine.esp");
    click_button(&mut h, "Set as Active File");
    assert_in_view(&h, "`Default`, the owner's live game");
    click_button(&mut h, "OK");
    let app = &h.state().app;
    assert_eq!(app.active_file, None);
    assert!(app.slots.is_empty(), "OK sent no load");
    assert_eq!(dir.snapshot(), before);
    assert!(Path::new(&mine).is_file());
}
