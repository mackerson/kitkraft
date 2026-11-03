//! The workbench's headless UI harness (`docs/design/workbench-ux.md`): the window's own
//! frame ([`App::frame`]) driven by `egui_kittest`, read through egui's AccessKit tree, with
//! no window, GPU or platform adapter. The engine runs on its worker as in the window, over
//! [`ReadOnlyEngine`] (no profile loaded unless a test loads one) or the engine's game fakes
//! ([`GameFakes`]); a reply a test needs that only a real profile gives is filed through
//! [`inject`], the same path a worker reply takes. Nothing here reads or writes `ui.json`.
//!
//! The checks are the UX spec's: a click shows its feedback inside the viewport
//! ([`assert_in_view`]), a control is found once ([`one`]), and so on; each test names the
//! principle and, for a fix, the owner's finding (2026-11-01) it holds.

use std::time::{Duration, Instant};

use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, Node};
use serde_json::Value;

use super::{fonts, App, Tab, TABS};
use crate::gate::{ReadOnlyEngine, Request, RunError};
use crate::panels::common::Slot;
use crate::prepare::{build, Built};
use crate::theme;
use crate::worker::{Reply, Worker};
use crate::LaunchOptions;

/// The harness's window: the workbench's usual 1400 x 900 points (`theme::WINDOW`).
pub(crate) const SIZE: [f32; 2] = theme::WINDOW;

/// How long [`settle`] waits for the engine before the test fails.
const ENGINE_WAIT: Duration = Duration::from_secs(120);

/// The harness's state: the app, and the fonts the first pass installs.
pub(crate) struct Wb {
    pub(crate) app: App,
    fonts: Option<egui::FontDefinitions>,
}

pub(crate) type Bench = Harness<'static, Wb>;

/// The workbench over an engine with no profile loaded; `None` (with a line on stderr)
/// on a machine with no usable font, as the corpus tests skip without the game.
pub(crate) fn workbench() -> Option<Bench> {
    workbench_over(ReadOnlyEngine::new())
}

/// The workbench over `engine` (for example [`ReadOnlyEngine::with_game_fakes`]).
pub(crate) fn workbench_over(engine: ReadOnlyEngine) -> Option<Bench> {
    let Some(fonts) = harness_fonts() else {
        eprintln!(
            r"harness: skipped: no system font (Segoe UI, Arial or Tahoma under %WINDIR%\Fonts, or a TrueType font under /usr/share/fonts)"
        );
        return None;
    };
    let worker = Worker::spawn_with_engine(engine, None).expect("the engine thread starts");
    let app = App::with_prefs(worker, LaunchOptions::default(), None);
    let state = Wb {
        app,
        fonts: Some(fonts),
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(SIZE[0], SIZE[1]))
        .build_ui_state(
            |ui, wb: &mut Wb| {
                // The fonts and the theme, as `run_window` sets them before the first
                // frame; this first pass draws nothing.
                if let Some(f) = wb.fonts.take() {
                    ui.ctx().set_fonts(f);
                    theme::apply(ui.ctx(), &wb.app.theme.preset.theme(), wb.app.theme.mode);
                    ui.ctx().request_repaint();
                    return;
                }
                wb.app.frame(ui, None);
            },
            state,
        );
    settle(&mut h);
    Some(h)
}

/// The window's fonts (Windows' system fonts), or on another system the first TrueType
/// font found under the usual font folders (the gate also runs on Linux; no font is
/// compiled in, ADR 0007, M6.O2).
fn harness_fonts() -> Option<egui::FontDefinitions> {
    if let Ok(f) = fonts() {
        return Some(f);
    }
    let mut stack: Vec<(std::path::PathBuf, u8)> = ["/usr/share/fonts", "/usr/local/share/fonts"]
        .iter()
        .map(|d| (std::path::PathBuf::from(d), 0))
        .collect();
    let mut found: Vec<std::path::PathBuf> = Vec::new();
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() && depth < 4 {
                stack.push((path, depth.saturating_add(1)));
            } else if path.extension().and_then(|x| x.to_str()) == Some("ttf") {
                found.push(path);
            }
        }
    }
    // A regular sans face when there is one (DejaVu Sans, Liberation Sans), else any.
    found.sort();
    let pick = found
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "DejaVuSans.ttf" || n == "LiberationSans-Regular.ttf")
        })
        .or_else(|| found.first())?;
    let bytes = std::fs::read(pick).ok()?;
    eprintln!("harness: fonts from {}", pick.display());
    let mut defs = egui::FontDefinitions::empty();
    defs.font_data.insert(
        "prop".into(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        defs.families.insert(family, vec!["prop".into()]);
    }
    Some(defs)
}

/// Frames until the engine has answered every request (and the requests those replies
/// sent), then a few more so tables settle their column widths.
pub(crate) fn settle(h: &mut Bench) {
    let deadline = Instant::now().checked_add(ENGINE_WAIT).expect("a deadline");
    loop {
        h.step();
        let a = &h.state().app;
        if a.slots.is_empty() && a.shell.tickets.is_empty() && !a.worker.is_busy() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the engine did not answer within {ENGINE_WAIT:?}: {:?}",
            a.worker.pending()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(4);
}

/// Files `result` as the worker's reply to `request` in `slot`: the same path
/// ([`App::receive`]) a reply from the engine takes, for replies only a real profile or
/// game gives.
pub(crate) fn inject(h: &mut Bench, slot: Slot, request: Request, result: Result<Value, RunError>) {
    let app = &mut h.state_mut().app;
    let ticket = u64::MAX.wrapping_sub(app.slots.len() as u64);
    app.slots.insert(ticket, slot);
    let built: Built = build(slot, &request, &result, app.load.view.as_ref());
    app.receive(Reply {
        ticket,
        request,
        result,
        elapsed: Duration::ZERO,
        waited: Duration::ZERO,
        first_progress: None,
        last_progress: None,
        bytes: None,
        built: Some(built),
        prep: Duration::ZERO,
    });
    h.run_steps(4);
}

/// Shows `tab` through its button in the tab bar (principle 8: every panel is reachable
/// from the tab bar).
pub(crate) fn open_tab(h: &mut Bench, tab: Tab) {
    let label = tab.name();
    tab_button(h, label).click();
    h.run_steps(2);
    assert_eq!(
        h.state().app.tab,
        tab,
        "the {label} tab button shows the {label} tab"
    );
    settle(h);
}

/// The tab bar's button named `label`, exactly one: a selectable value (a button with a
/// toggled state, which the menu bar's buttons lack).
pub(crate) fn tab_button<'a>(h: &'a Bench, label: &'a str) -> Node<'a> {
    let tabs: Vec<Node<'a>> = h
        .query_all_by_label(label)
        .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .filter(|n| n.accesskit_node().toggled().is_some())
        .collect();
    assert_eq!(tabs.len(), 1, "one `{label}` button in the tab bar");
    tabs.into_iter().next().expect("checked above")
}

/// The one node labelled exactly `label` (principle 3: one control per action).
pub(crate) fn one<'a>(h: &'a Bench, label: &'a str) -> Node<'a> {
    let all: Vec<Node<'a>> = h.query_all_by_label(label).collect();
    assert_eq!(
        all.len(),
        1,
        "exactly one `{label}`; found {}: {:?}",
        all.len(),
        all.iter().map(Node::rect).collect::<Vec<_>>()
    );
    all.into_iter().next().expect("checked above")
}

/// The viewport the owner sees: the window less nothing (the menu bar, tabs and status line
/// are part of it).
pub(crate) fn viewport() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SIZE[0], SIZE[1]))
}

/// Whether `n` lies inside the viewport, with some area (principle 1).
pub(crate) fn in_view(n: &Node<'_>) -> bool {
    let r = n.rect();
    let v = viewport();
    r.width() > 0.0 && r.height() > 0.0 && v.contains(r.center())
}

/// What a node shows: its label, or a plain label's text (egui puts a `Label`'s text in
/// the AccessKit value) or a text field's contents.
pub(crate) fn text_of(n: &egui_kittest::kittest::AccessKitNode<'_>) -> Option<String> {
    n.label().or_else(|| n.value()).filter(|t| !t.is_empty())
}

/// Every node whose text ([`text_of`]) contains `text`.
pub(crate) fn nodes_with<'a>(h: &'a Bench, text: &'a str) -> Vec<Node<'a>> {
    h.query_all_by(move |n| text_of(n).is_some_and(|t| t.contains(text)))
        .collect()
}

/// Principle 1: some node showing `text` is inside the viewport.
#[track_caller]
pub(crate) fn assert_in_view(h: &Bench, text: &str) {
    let all = nodes_with(h, text);
    assert!(
        all.iter().any(in_view),
        "`{text}` is not shown inside the {SIZE:?} viewport; nodes: {:?}\n{}",
        all.iter().map(Node::rect).collect::<Vec<_>>(),
        visible_text(h)
    );
}

/// Principle 1 (negative): nothing showing `text`.
#[track_caller]
pub(crate) fn assert_absent(h: &Bench, text: &str) {
    let all = nodes_with(h, text);
    assert!(
        all.is_empty(),
        "`{text}` is shown: {:?}",
        all.iter().map(Node::rect).collect::<Vec<_>>()
    );
}

/// The labels of the nodes inside the viewport, top to bottom, for failure messages and
/// the audit.
pub(crate) fn visible_text(h: &Bench) -> String {
    let mut rows: Vec<(f32, f32, String)> = Vec::new();
    collect(h.root(), &mut rows);
    rows.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    rows.into_iter()
        .map(|(y, x, l)| format!("{y:>6.0} {x:>6.0}  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect(n: Node<'_>, out: &mut Vec<(f32, f32, String)>) {
    if let Some(label) = text_of(&n.accesskit_node()) {
        if in_view(&n) {
            let r = n.rect();
            out.push((r.min.y, r.min.x, label));
        }
    }
    for c in n.children() {
        collect(c, out);
    }
}

/// Every tab label, in the tab bar's order.
pub(crate) fn tab_labels() -> Vec<&'static str> {
    TABS.iter().map(|(_, l)| *l).collect()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests;

// Step 0 of sprint 2026-11-02: each pipeline's harness tests in a file of its own, so the
// writers never share this one (pipeline E, W, R; X's live checks).
// The integrator's end-to-end check across the pipelines (sprint 2026-11-02, wave 1).
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod e2e;
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod live;
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests_e;
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests_r;
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests_w;
