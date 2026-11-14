//! The harness's tests: each tab opened from the tab bar, its controls clicked, and the
//! feedback each click gives checked against the UX spec (`docs/design/workbench-ux.md`).

use super::*;

/// Principle 8: every panel opens from its tab-bar button and shows its heading in view.
#[test]
fn every_tab_opens_from_the_tab_bar_with_its_heading_in_view() {
    let Some(mut h) = workbench() else { return };
    for (tab, _) in TABS {
        open_tab(&mut h, *tab);
        let heading = match tab {
            Tab::Diagnostics => "FormID and reference diagnostics",
            Tab::Schema => "Schema and provenance",
            t => t.name(),
        };
        assert_in_view(&h, heading);
        if std::env::var_os("KITKRAFT_UI_DUMP").is_some() {
            eprintln!("==== {}\n{}", tab.name(), visible_text(&h));
        }
    }
}

// ---- helpers for the findings' tests -------------------------------------------------------

use serde_json::json;

use crate::gate::GameFakes;

/// A `cell` worlds reply with `n` worldspaces (`World0001`..), as the engine shapes it.
fn worlds_envelope(n: usize) -> Value {
    let items: Vec<Value> = (1..=n)
        .map(|i| {
            json!({
                "form": format!("Starfield.esm:{:06X}", 0x1000 + i),
                "editor_id": format!("World{i:04}"),
                "child_records": i * 10,
            })
        })
        .collect();
    json!({
        "command": "cell",
        "data": {"mode": "worlds", "items": items, "total": n},
        "limitations": [],
        "coverage": {},
    })
}

/// Files a `cell` reply in the Cell View, as `ck_receive` does for a pending request.
fn file_cell_view(h: &mut Bench, env: &Value) {
    h.state_mut().app.shell.cell_view.receive(Ok(env));
    h.run_steps(4);
}

/// The loaded profile (`load`'s arguments the session holds), without a load: the
/// Cockpit reads its dev target from it.
fn pretend_loaded(h: &mut Bench, profile: &std::path::Path) {
    let args = json!({"profile": profile.display().to_string()});
    h.state_mut().app.session = args.as_object().cloned();
    h.run_steps(2);
}

/// Scrolls the node labelled `label` into view and clicks it, then lets the engine
/// answer.
fn click(h: &mut Bench, label: &str) {
    one(h, label).scroll_to_me();
    h.run_steps(3);
    one(h, label).click();
    h.run_steps(2);
    settle(h);
}

/// The tab's button labelled `label`: the tab bar's toggle of the same name carries a
/// toggled state, the button lacks it.
fn plain_button<'a>(h: &'a Bench, label: &'a str) -> Node<'a> {
    let all: Vec<Node<'a>> = h
        .query_all_by_label(label)
        .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .filter(|n| n.accesskit_node().toggled().is_none())
        .collect();
    assert_eq!(all.len(), 1, "one plain `{label}` button");
    all.into_iter().next().expect("checked above")
}

/// `click` for a button that shares its label with a tab bar toggle (Load's "Load").
fn click_plain(h: &mut Bench, label: &str) {
    plain_button(h, label).scroll_to_me();
    h.run_steps(3);
    plain_button(h, label).click();
    h.run_steps(2);
    settle(h);
}

/// Principle 2 (audit A1): some node showing `text` is in view and starts below
/// `button`, within a sixth of the window's height.
fn refused_by(h: &Bench, text: &str, button: &egui::Rect) {
    assert_in_view(h, text);
    let first = nodes_with(h, text)
        .into_iter()
        .find(|n| in_view(n))
        .expect("the refusal is in view")
        .rect();
    assert!(
        first.min.y >= button.max.y && first.min.y - button.max.y < SIZE[1] / 6.0,
        "the refusal starts below its button: {first:?} vs {button:?}"
    );
}

// ---- the owner's findings, 2026-11-01 ------------------------------------------------------

/// Finding 1 (principle 8): the Cell View has a tab-bar button and draws in the main
/// panel like any tab, not as a window docked over the current tab.
#[test]
fn finding_1_cell_view_is_a_tab() {
    let Some(mut h) = workbench() else { return };
    tab_button(&h, "Cell View");
    open_tab(&mut h, Tab::CellView);
    assert_in_view(&h, "worldspaces");
    // Back on another tab, nothing of the Cell View is drawn over it.
    open_tab(&mut h, Tab::Load);
    assert_absent(&h, "worldspaces");
    assert_absent(&h, "load a profile first");
}

/// Finding 2 (principle 5): a list of 551 worldspaces shows its first rows, "Interior"
/// first, inside the viewport, one row under the other.
#[test]
fn finding_2_the_cell_view_list_shows_its_rows() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::CellView);
    file_cell_view(&mut h, &worlds_envelope(551));
    assert_in_view(&h, "551 row(s)");
    assert_in_view(&h, "Interior");
    for i in 1..=8 {
        assert_in_view(&h, &format!("World{i:04}"));
    }
    let top = |t: &str| {
        nodes_with(&h, t)
            .iter()
            .find(|n| in_view(n))
            .map(|n| n.rect())
    };
    let (a, b) = (top("World0001").unwrap(), top("World0002").unwrap());
    assert!(b.min.y > a.min.y, "rows stack vertically: {a:?} then {b:?}");
    assert!(
        (a.min.x - b.min.x).abs() < 1.0,
        "rows share a column: {a:?} {b:?}"
    );
    // The list scrolls: a wheel over it brings later rows in.
    h.query_all_by(|n| text_of(n).as_deref() == Some("World0004"))
        .next()
        .unwrap()
        .scroll_down();
    h.run_steps(6);
    assert_in_view(&h, "World0010");
}

/// An absolute path that never exists, built with the host's separators (a `C:\` literal
/// is one file name on Linux, so the gate there would see no folders in it).
fn fake_path(parts: &[&str]) -> std::path::PathBuf {
    let mut p = std::env::temp_dir().join("kitkraft-ui-harness-fake");
    p.extend(parts);
    p
}

/// Fifteen plugins under a fake `out\`, with long nested paths.
fn fake_out(h: &mut Bench) -> Vec<String> {
    let base = fake_path(&["fake-checkout", "out"]);
    let found: Vec<String> = (1..=15)
        .map(|i| {
            base.join(format!("built-{i:02}"))
                .join("a-long-folder-name-for-the-overhaul-build")
                .join(format!("KitKraftOverhaulBuildNumber{i:02}.esm"))
                .display()
                .to_string()
        })
        .collect();
    let c = &mut h.state_mut().app.cockpit;
    c.found.clone_from(&found);
    c.found_in = Some(base);
    found
}

/// The node of the Cockpit's `out\` picker (a combo box).
fn out_picker<'a>(h: &'a Bench) -> Node<'a> {
    h.get_by(|n| n.role() == egui::accesskit::Role::ComboBox)
}

/// Finding 4 (principles 1 and 6): the `out\` picker shows every entry in full, without a
/// scroll, and the plugin under review can be cleared back to empty.
#[test]
fn finding_4_the_plugin_picker_shows_every_entry_and_clears() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Cockpit);
    let found = fake_out(&mut h);
    h.run_steps(2);
    out_picker(&h).click();
    h.run_steps(4);
    let base = fake_path(&["fake-checkout", "out"]);
    let shown: Vec<String> = found
        .iter()
        .map(|p| {
            std::path::Path::new(p)
                .strip_prefix(&base)
                .unwrap()
                .display()
                .to_string()
        })
        .collect();
    for s in &shown {
        let n = h.get_by(|n| text_of(n).as_deref() == Some(s.as_str()));
        assert!(in_view(&n), "{s} is in view: {:?}", n.rect());
        // Shown in full: the entry is as wide as its text.
        let style = h.ctx.global_style();
        let (font, ink) = (
            egui::TextStyle::Button.resolve(&style),
            style.visuals.text_color(),
        );
        let full = h
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(s.clone(), font, ink).size().x);
        assert!(
            n.rect().width() + 0.5 >= full,
            "{s}: {:?} < {full}",
            n.rect()
        );
    }
    // Every entry fits: the list has no scroll bar of its own.
    let first = h
        .get_by(|n| text_of(n).as_deref() == Some(shown[0].as_str()))
        .rect();
    let last = h
        .get_by(|n| text_of(n).as_deref() == Some(shown[14].as_str()))
        .rect();
    let bars: Vec<egui::Rect> = h
        .query_all_by(|n| n.role() == egui::accesskit::Role::ScrollBar)
        .map(|n| n.rect())
        .filter(|r| r.min.y < last.max.y && r.max.y > first.min.y)
        .filter(|r| r.min.x > first.min.x && r.min.x - first.max.x < first.height() * 2.0)
        .collect();
    assert!(bars.is_empty(), "the picker scrolls: {bars:?}");
    h.get_by(|n| text_of(n).as_deref() == Some(shown[3].as_str()))
        .click();
    h.run_steps(3);
    assert_eq!(h.state().app.cockpit.plugin, found[3]);
    click(&mut h, "Clear");
    assert_eq!(h.state().app.cockpit.plugin, "");
    assert_eq!(h.state().app.cockpit.form.source, "");
}

/// The ledger rows of the Cockpit's table, as drawn.
fn ledger_rows(h: &Bench) -> Vec<egui::Rect> {
    h.query_all_by(|n| {
        n.role() == egui::accesskit::Role::Label
            && text_of(n).is_some_and(|t| t.starts_with("game-202611"))
    })
    .map(|n| n.rect())
    .collect()
}

/// The resize grip of the table `id` (a splitter named after the table).
fn grip<'a>(h: &'a Bench, id: &'a str) -> Node<'a> {
    h.get_by(move |n| {
        n.role() == egui::accesskit::Role::Splitter && text_of(n).is_some_and(|t| t.contains(id))
    })
}

/// A mouse drag from `from` to `to`: hover, press, two moves, release.
fn drag(h: &mut Bench, from: egui::Pos2, to: egui::Pos2) {
    h.hover_at(from);
    h.run_steps(1);
    h.drag_at(from);
    h.run_steps(1);
    h.hover_at(from.lerp(to, 0.5));
    h.run_steps(1);
    h.hover_at(to);
    h.run_steps(1);
    h.drop_at(to);
    h.run_steps(4);
}

/// Finding 5 (principle 7): the ledger table has row spacing and can be made taller.
#[test]
fn finding_5_the_ledger_table_has_row_spacing_and_resizes() {
    let Some(mut h) = workbench() else { return };
    let ledger = (1..=30)
        .map(|i| {
            format!("### game-202611{i:02}-01: a session\n\n| Plugins | `KitKraftThing.esm` |\n\n")
        })
        .collect::<String>();
    let ledger = format!("# Ledger sample\n\n## Ledger\n\n{ledger}");
    h.state_mut().app.ledger.observations = Some(ledger);
    let thing = fake_path(&["x", "KitKraftThing.esm"]);
    h.state_mut().app.cockpit.pick(&thing.display().to_string());
    open_tab(&mut h, Tab::Cockpit);
    grip(&h, "cockpit-ledger").scroll_to_me();
    h.run_steps(4);
    let r = ledger_rows(&h);
    assert!(r.len() >= 3, "ledger rows drawn: {r:?}");
    let gap = r[1].min.y - r[0].max.y;
    assert!(
        gap >= 4.0,
        "rows are spaced: {} then {} (gap {gap})",
        r[0].max.y,
        r[1].min.y
    );
    // A drag on the table's resize grip makes it taller: the grip follows the pointer,
    // and once the grip is back in view more rows are drawn above it.
    let rows_before = ledger_rows(&h).len();
    let at = grip(&h, "cockpit-ledger").rect().center();
    let to = at + egui::Vec2::DOWN * (SIZE[1] / 5.0);
    drag(&mut h, at, to);
    let moved = grip(&h, "cockpit-ledger").rect().center();
    assert!(
        moved.y > at.y + SIZE[1] / 10.0,
        "the grip moved with the drag: {at:?} -> {moved:?}"
    );
    grip(&h, "cockpit-ledger").scroll_to_me();
    h.run_steps(4);
    let rows_after = ledger_rows(&h).len();
    assert!(
        rows_after > rows_before,
        "{rows_after} rows after the drag, {rows_before} before"
    );
}

/// The Cockpit's section headings, top to bottom.
fn section_tops(h: &Bench) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = [
        "Plugin under review",
        "Review",
        "Dev deployment",
        "Game session",
        "Ledger entries naming this plugin",
    ]
    .iter()
    .filter_map(|t| {
        h.query_all_by(|n| {
            n.role() == egui::accesskit::Role::Label && text_of(n).is_some_and(|x| x.starts_with(t))
        })
        .map(|n| n.rect().min.y)
        .reduce(f32::min)
        .map(|y| ((*t).to_string(), y))
    })
    .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out
}

/// Finding 6 (principle 4): the Cockpit's sections follow the work (plugin, review, dev
/// deployment, game session, ledger), and each section's controls sit inside it.
#[test]
fn finding_6_the_cockpit_is_grouped_by_task() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Cockpit);
    let tops = section_tops(&h);
    let names: Vec<&str> = tops.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "Plugin under review",
            "Review",
            "Dev deployment",
            "Game session",
            "Ledger entries naming this plugin"
        ],
        "{tops:?}"
    );
    let y = |name: &str| {
        tops.iter()
            .find(|(n, _)| n == name)
            .map(|(_, y)| *y)
            .unwrap()
    };
    let inside = |label: &str, from: &str, to: &str| {
        let r = h
            .query_all_by(|n| text_of(n).as_deref() == Some(label))
            .map(|n| n.rect().min.y)
            .reduce(f32::min)
            .unwrap_or_else(|| panic!("no `{label}`"));
        assert!(
            r > y(from) && r < y(to),
            "`{label}` ({r}) is not between `{from}` and `{to}`: {tops:?}"
        );
    };
    inside("Preview deploy", "Dev deployment", "Game session");
    inside(
        "probes",
        "Game session",
        "Ledger entries naming this plugin",
    );
    inside(
        "base save sha256",
        "Game session",
        "Ledger entries naming this plugin",
    );
    inside(
        "Preview session",
        "Game session",
        "Ledger entries naming this plugin",
    );
}

/// Finding 8 (principle 3): one control per action: the session's preview and launch
/// are the session section's, with no second row of game buttons at the top.
#[test]
fn finding_8_no_duplicate_game_buttons() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Cockpit);
    assert_absent(&h, "Launch and run probes");
    one(&h, "Preview session");
    one(&h, "Launch...");
}

/// Puts the Cockpit's session form on the fakes, with `extra` over their arguments.
fn session_on_fakes(h: &mut Bench, fakes: &GameFakes, extra: Value) {
    let profile = fakes.instance().join("profiles").join("Kitkraft Dev");
    pretend_loaded(h, &profile);
    let args = fakes.session_args(extra);
    let form = crate::panels::cockpit::session::SessionForm::from_args(&args);
    h.state_mut().app.cockpit.session_form = form;
    h.run_steps(2);
}

/// Findings 7 and 9 (principles 1 and 2): "Preview session" shows its result inside the
/// viewport, next to the button, and a refused dry run shows every refusal in full.
#[test]
fn findings_7_and_9_a_refused_preview_shows_its_refusals_by_the_button() {
    let fakes = GameFakes::new("ui-harness-refused").unwrap();
    let Some(mut h) = workbench_over(ReadOnlyEngine::with_game_fakes(&fakes)) else {
        return;
    };
    open_tab(&mut h, Tab::Cockpit);
    session_on_fakes(&mut h, &fakes, json!({"base_save_sha256": "00".repeat(32)}));
    click(&mut h, "Preview session");
    let preview = h
        .state()
        .app
        .cockpit
        .session
        .preview
        .clone()
        .expect("a preview arrived");
    assert!(!preview.would_start);
    assert!(!preview.refusals.is_empty());
    assert_in_view(&h, "session would be refused");
    for r in &preview.refusals {
        assert!(r.starts_with('['), "{r}");
        assert_in_view(&h, r);
    }
    // Next to the button: the result starts below it, within a third of the window.
    let button = one(&h, "Preview session").rect();
    let first = nodes_with(&h, &preview.refusals[0])
        .into_iter()
        .find(|n| in_view(n))
        .unwrap()
        .rect();
    assert!(
        first.min.y >= button.min.y && first.min.y - button.max.y < SIZE[1] / 3.0,
        "the refusals ({first:?}) are next to the button ({button:?})"
    );
    // The confirm box stays closed and says why without pointing elsewhere.
    assert_absent(&h, "(the refusals, above)");
}

/// Finding 9: a canned refused dry run with the owner's two refusals shows both lines.
#[test]
fn finding_9_the_owners_two_refusals_are_shown() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Cockpit);
    let req = h.state().app.cockpit.session_form.preview_request();
    let env = json!({
        "command": "game.session",
        "data": {
            "dry_run": true,
            "would_start": false,
            "refusals": [
                {"rule": "selected-profile", "message": "MO2's selected profile is `Default`, not the dev profile"},
                {"rule": "running", "message": "ModOrganizer.exe is running"}
            ],
            "plan": {},
            "summary": "session would be refused (2 reason(s)); nothing written"
        },
        "limitations": [],
    });
    inject(&mut h, Slot::CockpitSession, req, Ok(env));
    one(&h, "Preview session").scroll_to_me();
    h.run_steps(4);
    assert_in_view(&h, "[selected-profile] MO2's selected profile is `Default`");
    assert_in_view(&h, "[running] ModOrganizer.exe is running");
}

/// The ledger's newest passing session for `KitKraftFirstPOI.esm` prefills the session's
/// probes and base save (owner, 2026-11-01), and says where they came from.
#[test]
fn the_session_form_is_prefilled_from_the_newest_passing_ledger_session() {
    let Some(mut h) = workbench() else { return };
    let ledger = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/game-observations-sample.md"
    ))
    .unwrap();
    h.state_mut().app.ledger.observations = Some(ledger);
    let poi = fake_path(&["any", "out", "KitKraftFirstPOI.esm"]);
    h.state_mut().app.cockpit.pick(&poi.display().to_string());
    open_tab(&mut h, Tab::Cockpit);
    let form = &h.state().app.cockpit.session_form;
    assert!(
        form.probes
            .replace('\\', "/")
            .ends_with("observe/2026-11-01-poi-plan3/probes.toml"),
        "{}",
        form.probes
    );
    assert_eq!(
        form.base_save_sha256,
        "fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf"
    );
    one(&h, "probes").scroll_to_me();
    h.run_steps(3);
    assert_in_view(&h, "from the ledger: game-20261101-04");
}

// ---- the audit (docs/design/workbench-ux.md, "Findings") ------------------------------------

/// The buttons the audit may click on an engine with nothing loaded: reads, previews and
/// dry runs only. Anything with "..." opens a confirm box; "Confirm" and "Load <profile>"
/// are never clicked.
const AUDIT_SAFE: &[&str] = &[
    "Load",
    "Run",
    "Inspect",
    "validate",
    "roundtrip --verify",
    "Compare",
    "Show",
    "List",
    "Review",
    "Preview deploy",
    "Preview roll back",
    "Status",
    "Look again",
    "Reload",
    "Diagnose",
    "Use info",
    "Links",
    "Planet",
    "Diff",
    "Coverage",
    "Schema",
];

/// The tab's buttons below the menu and tab bars: (label, rect, enabled).
fn buttons(h: &Bench) -> Vec<(String, egui::Rect, bool)> {
    h.query_all_by(|n| {
        n.role() == egui::accesskit::Role::Button && n.toggled().is_none() && n.label().is_some()
    })
    .filter(|n| n.rect().min.y > SIZE[1] / 12.0)
    .map(|n| {
        let disabled = n.accesskit_node().is_disabled();
        (
            text_of(&n.accesskit_node()).unwrap_or_default(),
            n.rect(),
            !disabled,
        )
    })
    .collect()
}

/// Prints each tab's inventory and what each safe click showed, for the spec's findings
/// (`cargo test -p kitkraft-ui-egui --lib audit -- --ignored --nocapture`).
#[test]
#[ignore = "an audit report, not a check: run it by hand"]
fn audit_every_tab() {
    let Some(mut h) = workbench() else { return };
    for label in tab_labels() {
        let tab = TABS
            .iter()
            .find(|(_, l)| *l == label)
            .map(|(t, _)| *t)
            .unwrap();
        open_tab(&mut h, tab);
        let all = buttons(&h);
        let mut seen = std::collections::BTreeMap::<String, usize>::new();
        for (l, _, _) in &all {
            *seen.entry(l.clone()).or_default() += 1;
        }
        let dups: Vec<_> = seen.iter().filter(|(_, n)| **n > 1).collect();
        let combos = h
            .query_all_by(|n| n.role() == egui::accesskit::Role::ComboBox)
            .count();
        let inputs = h
            .query_all_by(|n| n.role() == egui::accesskit::Role::TextInput)
            .count();
        let grips = h
            .query_all_by(|n| n.role() == egui::accesskit::Role::Splitter)
            .count();
        let bottom = h
            .query_all_by(|n| text_of(n).is_some())
            .map(|n| n.rect().max.y)
            .fold(0.0_f32, f32::max);
        eprintln!(
            "== {label}: {} buttons ({} enabled), duplicates {dups:?}, {combos} pickers, {inputs} text fields, {grips} table grips, content to y={bottom:.0}",
            all.len(),
            all.iter().filter(|b| b.2).count()
        );
        for (l, r, on) in all {
            if !on || !AUDIT_SAFE.contains(&l.as_str()) {
                continue;
            }
            let before = visible_text(&h);
            let notice_before = h.state().app.notice.clone();
            if !viewport().contains(r.center()) {
                eprintln!("   [{l}] at y={:.0}: below the fold", r.min.y);
                continue;
            }
            let Some(node) = h
                .query_all_by(|n| text_of(n).as_deref() == Some(l.as_str()))
                .find(|n| (n.rect().min.y - r.min.y).abs() < 1.0)
            else {
                eprintln!("   [{l}] moved after an earlier click; not clicked");
                continue;
            };
            node.click();
            h.run_steps(2);
            settle(&mut h);
            let after = visible_text(&h);
            let new: Vec<&str> = after
                .lines()
                .filter(|x| !before.contains(x.get(14..).unwrap_or("")))
                .take(4)
                .collect();
            let notice = h.state().app.notice.clone();
            eprintln!(
                "   [{l}] at y={:.0}: notice {:?}; new in view: {:?}",
                r.min.y,
                (notice != notice_before).then_some(notice).flatten(),
                new
            );
            h.state_mut().app.notice = None;
            open_tab(&mut h, tab);
        }
    }
}

/// Audit A1 (principle 2): a form a button refuses is shown by that button — in view,
/// starting below it within a sixth of the window's height — not only in the notice
/// line under the tabs.
#[test]
fn audit_a1_a_refused_form_is_shown_by_its_button() {
    let Some(mut h) = workbench() else { return };

    // Load: "Load" with an empty form.
    open_tab(&mut h, Tab::Load);
    let button = plain_button(&h, "Load").rect();
    click_plain(&mut h, "Load");
    refused_by(&h, "give an MO2 profile or at least one plugin", &button);

    // Validate: "validate" with no plugin.
    open_tab(&mut h, Tab::Validate);
    let button = one(&h, "validate").rect();
    click(&mut h, "validate");
    refused_by(&h, "give a plugin", &button);

    // Diff: "Compare" with one plugin (the other side is empty).
    open_tab(&mut h, Tab::Diff);
    h.state_mut().app.diff.form.a = "A.esm".to_string();
    h.run_steps(2);
    let button = one(&h, "Compare").rect();
    click(&mut h, "Compare");
    refused_by(&h, "give two plugins", &button);
}

/// Audit A2 (principle 1): "List" on a folder with no profiles says so by the button.
#[test]
fn audit_a2_an_empty_profiles_listing_says_so() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Load);
    let dir = std::env::temp_dir().join(format!("kitkraft-ui-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    h.state_mut().app.load.profiles_dir = dir.display().to_string();
    h.run_steps(2);
    click(&mut h, "List");
    let _ = std::fs::remove_dir_all(&dir);
    assert_in_view(&h, "no profile folders in");
}

/// Audit A4 (principle 1): Help's "Reload" says what it reread.
#[test]
fn audit_a4_help_reload_says_what_it_reread() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Help);
    click(&mut h, "Reload");
    assert_in_view(&h, "Help: re");
}

/// Audit A3 (principle 1): the Records tab opens with its query form's Run in view.
#[test]
fn audit_a3_the_records_query_opens_in_view() {
    let Some(mut h) = workbench() else { return };
    open_tab(&mut h, Tab::Records);
    let run = one(&h, "Run");
    assert!(in_view(&run), "Run at {:?}", run.rect());
    // The types are one click away.
    one(&h, "Record types").click();
    h.run_steps(4);
    assert_in_view(&h, "AACT");
}
