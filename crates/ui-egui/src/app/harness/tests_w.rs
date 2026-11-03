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

/// R17 (the wave-1 end-to-end check's first step): a scratch active file under a temp
/// folder, typed into the Data dialog's active-file field, becomes `App.active_file` on
/// OK; the file is not created (the first write does that).
#[test]
fn a_typed_scratch_path_becomes_the_active_file() {
    let Some(mut h) = workbench() else { return };
    let dir = Scratch::new("r17");
    let mine = dir.write("Mine.esp", &plugin_bytes(&[], 0, &[0x800], 3.0));
    let load = Request::new("load", json!({"plugins": [mine], "no_cache": true}));
    h.state_mut()
        .app
        .apply(vec![Action::Submit(Slot::Load, load)]);
    settle(&mut h);
    open_data_dialog(&mut h);
    let scratch = dir.0.join("out").join("Scratch.esp").display().to_string();
    let input: Vec<Node<'_>> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::TextInput
                && n.label().as_deref() == Some("Active file path:")
        })
        .collect();
    assert_eq!(input.len(), 1, "one active-file field");
    input[0].focus();
    h.run_steps(1);
    let input: Vec<Node<'_>> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::TextInput
                && n.label().as_deref() == Some("Active file path:")
        })
        .collect();
    input[0].type_text(&scratch);
    h.run_steps(2);
    click_button(&mut h, "Use This File");
    assert_in_view(&h, "chosen, not in the load order");
    click_button(&mut h, "OK");
    assert_eq!(h.state().app.active_file.as_deref(), Some(scratch.as_str()));
    assert!(
        !std::path::Path::new(&scratch).exists(),
        "nothing is written"
    );
}

// ---- the Object Window (M7.W1) -------------------------------------------------------------

/// M7.W1 over the real engine: the tree is `schema`'s ("*All" at the top as observed, every
/// type in "Other", marked unobserved), a type's node lists its records from `query`, and
/// the Text Filter narrows them through the engine.
#[test]
fn the_object_window_lists_a_types_records_and_filters_them() {
    let Some(mut h) = workbench() else { return };
    let dir = Scratch::new("ow");
    let mine = dir.write(
        "Mine.esm",
        &plugin_bytes(&[], 1, &[0x800, 0x801, 0x802], 1.0),
    );
    let load = Request::new("load", json!({"plugins": [mine], "no_cache": true}));
    h.state_mut()
        .app
        .apply(vec![Action::Submit(Slot::Load, load)]);
    settle(&mut h);
    h.state_mut()
        .app
        .apply(vec![Action::OpenWindow(WindowKind::ObjectWindow)]);
    h.run_steps(3);
    settle(&mut h);
    assert_in_view(&h, "*All");
    assert_in_view(&h, "Other (unobserved)");
    assert_in_view(&h, "Object Window categories");
    // The tree is long (every defined type is in "Other"): wheel it until GLOB is in view.
    let is_glob = |n: &egui_kittest::kittest::AccessKitNode<'_>| {
        text_of(n).is_some_and(|t| t.starts_with("GLOB ")) && n.toggled().is_some()
    };
    for _ in 0..80 {
        if h.query_all_by(is_glob).next().is_some_and(|n| in_view(&n)) {
            break;
        }
        h.query_all_by(|n| text_of(n).as_deref() == Some("*All") || is_glob(n))
            .next()
            .unwrap()
            .scroll_down();
        h.run_steps(1);
    }
    // The wheel scrolls smoothly: let it come to rest before clicking.
    h.run_steps(30);
    let glob = h.query_all_by(is_glob).next().unwrap();
    assert!(in_view(&glob), "GLOB node at {:?}", glob.rect());
    glob.click();
    h.run_steps(2);
    settle(&mut h);
    for id in ["Glob800", "Glob801", "Glob802"] {
        assert_in_view(&h, id);
    }
    assert_in_view(&h, "3 row(s) on this page");
    let input: Vec<Node<'_>> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::TextInput
                && n.label().as_deref() == Some("Text Filter")
        })
        .collect();
    assert_eq!(input.len(), 1, "one Text Filter");
    input[0].focus();
    h.run_steps(1);
    let input: Vec<Node<'_>> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::TextInput
                && n.label().as_deref() == Some("Text Filter")
        })
        .collect();
    input[0].type_text("801");
    h.run_steps(2);
    click_button(&mut h, "Filter");
    assert_in_view(&h, "1 row(s) on this page");
    assert_in_view(&h, "Glob801");
    assert_absent(&h, "Glob802");
    // *All asks for a type (query takes one); wheel the tree back to its top first.
    let is_all = |n: &egui_kittest::kittest::AccessKitNode<'_>| {
        text_of(n).as_deref() == Some("*All") && n.toggled().is_some()
    };
    for _ in 0..80 {
        if h.query_all_by(is_all).next().is_some_and(|n| in_view(&n)) {
            break;
        }
        h.query_all_by(is_glob).next().unwrap().scroll_up();
        h.run_steps(1);
    }
    h.run_steps(30);
    let all = h
        .query_all_by(|n| text_of(n).as_deref() == Some("*All") && n.toggled().is_some())
        .next()
        .unwrap();
    all.click();
    h.run_steps(2);
    assert_in_view(&h, "pick one below *All");
}

/// M7.W1's T1 (`docs/design/workbench-performance.md`: frame time while scrolling a
/// 1,000-row page) in the Object Window, on `Starfield.esm` from KITKRAFT_DATA_DIR: the
/// engine's first page of the type with the most records (REFR, 3,291,860 in the corpus)
/// and of STAT, cold (the first query after the load) and warm (the same query again), then
/// the harness frames while the wheel scrolls the list. Headless: a frame here is egui's
/// layout and tessellation without the GPU, so it bounds the window's own cost from below.
/// `#[ignore]`d (reads the corpus; run with `--release -- --ignored --nocapture`).
#[test]
#[ignore = "T1 on the corpus: needs KITKRAFT_DATA_DIR; run in release with --ignored"]
fn t1_object_window_on_the_largest_type() {
    let Some(data) = std::env::var_os("KITKRAFT_DATA_DIR") else {
        eprintln!("skipped: KITKRAFT_DATA_DIR unset");
        return;
    };
    let esm = std::path::Path::new(&data).join("Starfield.esm");
    if !esm.is_file() {
        eprintln!("skipped: no Starfield.esm");
        return;
    }
    let Some(mut h) = workbench() else { return };
    let t = std::time::Instant::now();
    let load = Request::new("load", json!({"plugins": [esm.display().to_string()]}));
    h.state_mut()
        .app
        .apply(vec![Action::Submit(Slot::Load, load)]);
    settle(&mut h);
    eprintln!("T1: load Starfield.esm {} ms", t.elapsed().as_millis());
    h.state_mut()
        .app
        .apply(vec![Action::OpenWindow(WindowKind::ObjectWindow)]);
    h.run_steps(3);
    settle(&mut h);
    for ty in ["REFR", "STAT"] {
        for pass in ["cold", "warm"] {
            let t = std::time::Instant::now();
            h.state_mut().app.shell.object_window.select_node(ty);
            h.run_steps(1);
            settle(&mut h);
            let rows = h
                .state()
                .app
                .shell
                .object_window
                .list
                .as_ref()
                .map_or(0, |v| v.rows.len());
            eprintln!(
                "T1: {ty} first page {pass}: {rows} rows in {} ms (query and the frames that show it)",
                t.elapsed().as_millis()
            );
            assert_eq!(rows, 1000, "{ty}: a full page");
        }
        // Frames while the wheel scrolls the 1,000 rows (the wheel over a row's FormID).
        let forms: Vec<String> = h
            .state()
            .app
            .shell
            .object_window
            .list
            .as_ref()
            .map(|v| v.rows.iter().map(|r| r.form.clone()).collect())
            .unwrap_or_default();
        let mut frames: Vec<f64> = Vec::new();
        for _ in 0..300 {
            if let Some(n) = h
                .query_all_by(|n| text_of(n).is_some_and(|t| forms.contains(&t)))
                .find(|n| in_view(n))
            {
                n.scroll_down();
            }
            let t = std::time::Instant::now();
            h.step();
            frames.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        // The wheel moved the list: a row far down the page is drawn now.
        let far: Vec<String> = h
            .state()
            .app
            .shell
            .object_window
            .list
            .as_ref()
            .map(|v| {
                v.rows
                    .iter()
                    .skip(150)
                    .take(800)
                    .map(|r| r.form.clone())
                    .collect()
            })
            .unwrap_or_default();
        let shown = far.iter().any(|f| nodes_with(&h, f).iter().any(in_view));
        eprintln!("T1: {ty} a row past the 150th is in view after the scroll: {shown}");
        assert!(shown, "the wheel scrolled the list");
        frames.sort_by(f64::total_cmp);
        let at = |q: f64| frames[((frames.len() - 1) as f64 * q) as usize];
        eprintln!(
            "T1: {ty} scrolling, {} frames: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms",
            frames.len(),
            at(0.5),
            at(0.95),
            frames[frames.len() - 1]
        );
    }
}

// ---- Preferences (M7.M3) -------------------------------------------------------------------

/// M7.M3: Preferences shows the theme, the zoom, the paths and the shortcuts, says its
/// layout is KitKraft's own (the CK's dialog is unobserved), and its changes reach the
/// window and the prefs (`ui.json`; the harness has no prefs file, so nothing is written).
#[test]
fn preferences_change_the_zoom_theme_and_checkout() {
    let Some(mut h) = workbench() else { return };
    h.state_mut()
        .app
        .apply(vec![Action::OpenWindow(WindowKind::Preferences)]);
    h.run_steps(3);
    assert_in_view(&h, "KitKraft's own layout");
    assert_in_view(&h, "Zoom");
    assert_in_view(&h, "Shortcuts");
    assert_in_view(&h, "Ctrl+O");
    assert_in_view(&h, "not kept: no %LOCALAPPDATA%");
    click_button(&mut h, "Zoom in");
    assert_eq!(h.state().app.prefs.zoom, Some(1.1));
    assert!((h.ctx.zoom_factor() - 1.1).abs() < 1e-4);
    click_button(&mut h, "Reset zoom");
    assert_eq!(h.state().app.prefs.zoom, Some(1.0));
    // The theme: the window's "dark" radio (not the Start tab's, behind it) sends
    // Action::SetTheme, which the window applies.
    let left = h
        .query_all_by(|n| text_of(n).is_some_and(|t| t.starts_with("KitKraft's own layout")))
        .map(|n| n.rect().min.x)
        .fold(f32::MAX, f32::min);
    let dark: Vec<Node<'_>> = h
        .query_all_by(|n| {
            text_of(n).as_deref() == Some("dark") && n.role() == egui::accesskit::Role::RadioButton
        })
        .filter(|n| n.rect().min.x >= left - 1.0)
        .collect();
    assert_eq!(dark.len(), 1, "the window's dark radio");
    dark[0].click();
    h.run_steps(3);
    assert_eq!(h.state().app.theme.mode, crate::theme::Mode::Dark);
    assert_eq!(
        h.state()
            .app
            .prefs
            .theme
            .as_deref()
            .map(|t| t.ends_with("-dark")),
        Some(true)
    );
    // The checkout: this repository.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .display()
        .to_string();
    let field: Vec<Node<'_>> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::TextInput
                && n.label().as_deref() == Some("KitKraft checkout:")
        })
        .collect();
    assert_eq!(field.len(), 1);
    field[0].focus();
    h.run_steps(1);
    let field: Vec<Node<'_>> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::TextInput
                && n.label().as_deref() == Some("KitKraft checkout:")
        })
        .collect();
    field[0].type_text(&repo);
    h.run_steps(2);
    click_button(&mut h, "Use This Checkout");
    assert_eq!(h.state().app.prefs.repo_dir.as_deref(), Some(repo.as_str()));
}

// ---- E's ask B1: File > Build Patch opens the build form ------------------------------------

/// The menu item labelled `label` (its text may carry the shortcut), in the open menu.
fn menu_item<'a>(h: &'a Bench, label: &'a str) -> Node<'a> {
    let all: Vec<Node<'a>> = h
        .query_all_by(move |n| {
            n.role() == egui::accesskit::Role::Button
                && text_of(n).is_some_and(|t| t.starts_with(label))
        })
        .collect();
    assert_eq!(all.len(), 1, "one `{label}` item");
    all.into_iter().next().expect("checked above")
}

/// Where the menu bar draws `command`'s own item: its top-level menu and label, read from
/// the menu model (so a `menu_path` move, such as M7.S1's, moves the test with it).
fn item_of(command: &str) -> (String, &'static str) {
    let menus = crate::menu::build(crate::gate::catalog());
    menus
        .iter()
        .find_map(|m| {
            m.items
                .iter()
                .find(|i| i.catalog && i.id == command)
                .map(|i| (m.name.clone(), i.label))
        })
        .unwrap_or_else(|| panic!("`{command}` has no item"))
}

/// What the editor shows once it has `command`'s form open: the form's title (E's: "Build
/// Spec", "New Form", "Duplicate"), or, for a command the editor does not have yet, its note
/// naming `` `command` ``.
fn editor_shows(h: &Bench, command: &str, label: &str) -> bool {
    let title = match command {
        "build" => "Build Spec",
        "new" => "New Form",
        "clone" => "Copy of",
        _ => "\u{0}",
    };
    visible_text(h).lines().any(|l| {
        let text = l.get(15..).unwrap_or_default().trim();
        text != label
            && !text.starts_with(label)
            && (text.contains(title) || text.contains(&format!("`{command}`")))
    })
}

/// An editor write's own menu item (found through the menu model): enabled exactly when the
/// gate permits `command`; a click opens its form in the editor exactly then, and does
/// nothing when it is disabled. Then the pick itself, as the shell takes it (`ck_pick`),
/// reaches the editor whichever the gate says.
fn editor_item_follows_the_gate(command: &'static str) {
    let Some(mut h) = workbench() else { return };
    let (menu, label) = item_of(command);
    assert!(
        !editor_shows(&h, command, label),
        "nothing open at the start"
    );
    h.state_mut().app.shell.tour_menu = Some(menu.clone());
    h.run_steps(3);
    let item = menu_item(&h, label);
    let permitted = crate::gate::permitted(command).is_some();
    assert_eq!(
        !item.accesskit_node().is_disabled(),
        permitted,
        "{menu} > {label}: enabled exactly when the gate permits `{command}`"
    );
    item.click();
    h.state_mut().app.shell.tour_menu = None;
    h.run_steps(4);
    assert_eq!(
        editor_shows(&h, command, label),
        permitted,
        "{menu} > {label}: the editor opened `{command}`'s form exactly when it is permitted\n{}",
        visible_text(&h)
    );
    h.state_mut()
        .app
        .ck_pick(crate::menu::MenuPick::Open(crate::menu::Target::Editor(
            command,
        )));
    h.run_steps(4);
    assert!(
        editor_shows(&h, command, label),
        "the pick reaches the editor:\n{}",
        visible_text(&h)
    );
}

/// E's ask B1 (M7.E5), W5 review L1: File > Build Patch follows the gate and opens the
/// build form through the editor.
#[test]
fn file_build_patch_follows_the_gate_and_opens_the_build_form() {
    assert_eq!(item_of("build"), ("File".to_string(), "Build Patch..."));
    editor_item_follows_the_gate("build");
    match super::super::ck_shell::editor_action("build", Some("A.esm:000800"), Some("GLOB")) {
        Action::OpenEditor { command, form } => {
            assert_eq!((command.as_str(), form.as_str()), ("build", ""))
        }
        _ => panic!("OpenEditor"),
    }
}

/// E's ask C1 (M7.E3): Record > New Form..., Record > Clone Form... and Edit > Duplicate
/// follow the gate and open the `new`/`clone` forms through the editor, not the plain Edit
/// dialog; `clone` copies the inspected record, `new` starts from its type.
#[test]
fn new_clone_and_duplicate_open_the_editors_forms() {
    editor_item_follows_the_gate("new");
    editor_item_follows_the_gate("clone");
    let action =
        |c: &str, f: Option<&str>, t: Option<&str>| match super::super::ck_shell::editor_action(
            c, f, t,
        ) {
            Action::OpenEditor { command, form } => (command, form),
            _ => panic!("OpenEditor"),
        };
    let sel = Some("A.esm:000800");
    assert_eq!(
        action("clone", sel, Some("GLOB")),
        ("clone".to_string(), "A.esm:000800".to_string())
    );
    assert_eq!(
        action("new", sel, Some("GLOB")),
        ("new".to_string(), "GLOB".to_string())
    );
    assert_eq!(
        action("new", None, None),
        ("new".to_string(), String::new())
    );
}

// ---- the catalog's shortcuts (M7.M5, mechanism first) ---------------------------------------

/// M7.M5: each catalog shortcut does what its menu item does (the item's own target, so it
/// follows the menu and the gate); a disabled item's key does nothing; a bare key (Enter)
/// waits while a text field has the keyboard.
#[test]
fn the_catalog_shortcuts_open_what_their_items_open() {
    use egui::{Key, Modifiers};
    let Some(mut h) = workbench() else { return };
    let menus = crate::menu::build(crate::gate::catalog());
    let target = |id: &str| {
        menus
            .iter()
            .flat_map(|m| m.items.iter())
            .find(|i| i.catalog && i.id == id)
            .map(|i| i.target.clone())
            .unwrap()
    };
    let shown = |h: &Bench| {
        (
            h.state().app.tab,
            h.state().app.shell.is_open(WindowKind::DataDialog),
        )
    };
    for (id, mods, key) in [
        ("query", Modifiers::COMMAND, Key::F),
        ("validate", Modifiers::NONE, Key::F7),
        ("load", Modifiers::COMMAND, Key::O),
        ("inspect", Modifiers::NONE, Key::Enter),
    ] {
        open_tab(&mut h, Tab::Start);
        h.key_press_modifiers(mods, key);
        h.run_steps(3);
        match target(id) {
            Ok(crate::menu::Target::Tab(label)) => {
                assert_eq!(
                    h.state().app.tab.name(),
                    label,
                    "`{id}`'s key shows {label}"
                )
            }
            Ok(crate::menu::Target::Window(kind)) => {
                assert!(
                    h.state().app.shell.is_open(kind),
                    "`{id}`'s key opens {kind:?}"
                );
                h.state_mut().app.shell.set_open(kind, false);
            }
            other => panic!("`{id}`: {other:?}"),
        }
    }
    // Ctrl+B is `build`'s: enabled only when the gate permits it.
    open_tab(&mut h, Tab::Start);
    let before = shown(&h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::B);
    h.run_steps(3);
    if crate::gate::permitted("build").is_none() {
        assert_eq!(shown(&h), before, "a disabled item's key does nothing");
        assert!(!visible_text(&h).contains("`build`"));
    }
    // Enter while a text field has the keyboard stays the field's.
    h.state_mut()
        .app
        .apply(vec![Action::OpenWindow(WindowKind::Preferences)]);
    h.run_steps(3);
    let field: Vec<Node<'_>> = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::TextInput
                && n.label().as_deref() == Some("KitKraft checkout:")
        })
        .collect();
    field[0].focus();
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert_eq!(
        h.state().app.tab,
        Tab::Start,
        "Enter in a text field is the field's"
    );
}
