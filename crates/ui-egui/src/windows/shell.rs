//! The CK-style shell's glue (M7.M0, M7.M1, M7.M4, M7.W3, M7.W5): a child module of
//! `app.rs` (declared there with `#[path]`, so it can reach the shell's tabs and
//! state), holding everything `app.rs` calls through its one-line hooks:
//!
//! - `ck_menu_bar`: the menu bar from the catalog, above the tabs;
//! - `ck_after_top_bar`: a floating tab is never drawn twice (the central panel falls
//!   back to the last docked tab and the window comes to the front);
//! - `ck_windows`: the Warnings, Use Info and Edit Dialog windows and the floating tabs,
//!   then `ui.json` when the layout changed;
//! - `ck_receive`: replies to the requests those windows sent;
//! - `ck_tour`: the screenshot tour's CK-shell images, after the tab images.
//!
//! Every request goes through the worker and the gate like any tab's; nothing here
//! writes but `ui.json` (UI state), and not during the tour.

use std::path::Path;

use eframe::egui::{self, ScrollArea};
use serde_json::Value;

use super::{App, Tab, TABS};
use crate::gate::Request;
use crate::menu::{self, MenuPick, Target};
use crate::panels::cockpit::CockpitEnv;
use crate::panels::common::{Action, Slot, TabCtx};
use crate::windows::cell_view::CellViewAsk;
use crate::windows::use_info::UseInfoAsk;
use crate::windows::warnings::{self, WarningsAsk};
use crate::windows::{self, Pending, WinSpec, WindowKind};
use crate::worker::Reply;

/// The tab with label `label`.
fn tab_of(label: &str) -> Option<Tab> {
    TABS.iter().find(|(_, l)| *l == label).map(|(t, _)| *t)
}

/// The label of `tab`.
fn label_of(tab: Tab) -> Option<&'static str> {
    TABS.iter().find(|(t, _)| *t == tab).map(|(_, l)| *l)
}

/// The tour's CK-shell images, in order (`ck-<name>.png`).
const TOUR: &[&str] = &[
    "menu-file",
    "menu-view",
    "menu-record",
    "menu-navmesh",
    "warnings",
    "use-info",
    "edit-weap",
    "edit-default",
    "layout",
];

/// A record of a type with no layout file, for the tour's default-layout image (the
/// player's base form in `Starfield.esm`).
const TOUR_DEFAULT_FORM: &str = "Starfield.esm:000007";

impl App {
    /// The inspected record (the Edit dialog's and Use Info's subject), as a formref.
    fn ck_selected(&self) -> Option<String> {
        self.records
            .record_view
            .as_ref()
            .map(|r| r.global.clone())
            .filter(|g| !g.trim().is_empty() && !g.ends_with(':'))
    }

    fn ck_submit(&mut self, pending: Pending, req: Request) {
        match self.worker.submit(req) {
            Ok(ticket) => {
                self.shell.tickets.insert(ticket, pending);
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    /// Hook: the menu bar, drawn before the top bar so it sits above the tabs.
    pub(super) fn ck_menu_bar(&mut self, ui: &mut egui::Ui) {
        if !self.shell.loaded {
            self.shell.loaded = true;
            // The tour starts from a closed layout, so its images do not depend on
            // what a user left open.
            if self.tour.is_none() {
                self.shell.layout = windows::layout_from_json(self.prefs.ck_windows.as_ref());
            }
        }
        if let Some(name) = self.shell.tour_menu.clone() {
            if let Some((_, id)) = self.shell.menu_ids.iter().find(|(n, _)| *n == name) {
                egui::Popup::open_id(ui.ctx(), id.with("popup"));
            }
        }
        let menus = menu::build(crate::gate::catalog());
        let labels: Vec<&'static str> = TABS.iter().map(|(_, l)| *l).collect();
        let selected = self.records.record_view.as_ref().and_then(|r| {
            let s = format!("{} {}", r.signature, r.editor_id);
            (!r.signature.is_empty()).then(|| s.trim().to_string())
        });
        let pick = menu::menu_bar(ui, &menus, &mut self.shell, &labels, selected.as_deref());
        match pick {
            Some(MenuPick::Open(Target::Tab(label))) => self.ck_open_tab(label),
            Some(MenuPick::Open(Target::Window(kind))) => self.shell.set_open(kind, true),
            Some(MenuPick::FloatTab(label)) => {
                let float = !self.shell.is_floating(label);
                let docked = TABS
                    .iter()
                    .filter(|(_, l)| *l != label && !self.shell.is_floating(l))
                    .count();
                if float && docked == 0 {
                    self.notice = Some("keep one tab in the main window".to_string());
                } else {
                    self.shell.set_floating(label, float);
                    if !float {
                        if let Some(t) = tab_of(label) {
                            self.tab = t;
                        }
                    }
                }
            }
            None => {}
        }
    }

    /// Shows tab `label`: in the central panel, or its window when it floats.
    fn ck_open_tab(&mut self, label: &'static str) {
        if self.shell.is_floating(label) {
            self.shell.focus = Some(windows::tab_key(label));
        } else if let Some(t) = tab_of(label) {
            self.tab = t;
        }
    }

    /// Hook: after the top bar. A tab that floats is drawn only in its window: the central
    /// panel shows the last docked tab instead, and the window comes to the front.
    pub(super) fn ck_after_top_bar(&mut self) {
        let Some(label) = label_of(self.tab) else {
            return;
        };
        if !self.shell.is_floating(label) {
            self.shell.last_docked = Some(label);
            return;
        }
        self.shell.focus = Some(windows::tab_key(label));
        let back = self
            .shell
            .last_docked
            .filter(|l| !self.shell.is_floating(l))
            .or_else(|| {
                TABS.iter()
                    .map(|(_, l)| *l)
                    .find(|l| !self.shell.is_floating(l))
            });
        if let Some(t) = back.and_then(tab_of) {
            self.tab = t;
        }
    }

    /// Hook: the replies to the windows' own requests (any ticket the tabs did not send).
    pub(super) fn ck_receive<B>(&mut self, reply: Reply<B>) {
        let result = reply.result.as_ref().map_err(ToString::to_string);
        match self.shell.tickets.remove(&reply.ticket) {
            Some(Pending::UseInfo) => self.shell.use_info.receive(result),
            Some(Pending::CellView) => self.shell.cell_view.receive(result),
            Some(Pending::Schema(sig)) => self.shell.dialog.receive_schema(&sig, result),
            None => {}
        }
    }

    /// Hook: the CK-style windows and the floating tabs, after the central panel.
    pub(super) fn ck_windows(&mut self, ctx: &egui::Context) {
        let busy = self.worker.is_busy();
        let focus = self.shell.focus.take();
        let plugins = self.loaded_plugins();
        let mut actions = Vec::new();
        let is_focus = |key: &str| focus.as_deref() == Some(key);

        // Warnings (M7.M4).
        let mut warn_ask = None;
        if self.shell.is_open(WindowKind::Warnings) {
            let load_lims = self
                .results
                .get(&Slot::Load.into())
                .and_then(|s| s.prepared.view.as_ref())
                .map(|v| v.limitations.clone())
                .unwrap_or_default();
            let validated = self
                .results
                .get(&Slot::Validate.into())
                .and_then(|s| s.request.args.get("plugin"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let rows = warnings::rows(
                self.load.view.as_ref(),
                &load_lims,
                self.validate.view.as_ref(),
                &validated,
            );
            let kind = WindowKind::Warnings;
            let state = self
                .shell
                .layout
                .entry(kind.title().to_string())
                .or_default();
            let w = &mut self.shell.warnings;
            windows::floating(
                ctx,
                state,
                &mut self.shell.dirty,
                WinSpec {
                    key: kind.title(),
                    title: kind.title(),
                    default_pos: kind.default_pos(),
                    default_size: kind.default_size(),
                    focus: is_focus(kind.title()),
                },
                |ui| warn_ask = w.ui(ui, &rows, busy, &plugins, &validated),
            );
        }
        match warn_ask {
            Some(WarningsAsk::Validate(plugin)) => {
                self.validate.form.plugin = plugin;
                match self.validate.form.request() {
                    Ok(req) => self.submit(Slot::Validate, req),
                    Err(e) => self.notice = Some(e),
                }
            }
            Some(WarningsAsk::Inspect(form)) => actions.push(Action::Inspect(form)),
            None => {}
        }

        // Use Info (M7.W5): follows the selection while open.
        let selected = self.ck_selected();
        let mut info_ask = None;
        if self.shell.is_open(WindowKind::UseInfo) {
            if let Some(req) = selected
                .as_deref()
                .and_then(|s| self.shell.use_info.follow(s))
            {
                self.ck_submit(Pending::UseInfo, req);
            }
            let kind = WindowKind::UseInfo;
            let title = format!("{} (refs --incoming)", kind.title());
            let state = self
                .shell
                .layout
                .entry(kind.title().to_string())
                .or_default();
            let u = &self.shell.use_info;
            windows::floating(
                ctx,
                state,
                &mut self.shell.dirty,
                WinSpec {
                    key: kind.title(),
                    title: &title,
                    default_pos: kind.default_pos(),
                    default_size: kind.default_size(),
                    focus: is_focus(kind.title()),
                },
                |ui| info_ask = u.ui(ui, selected.as_deref(), busy),
            );
        }
        match info_ask {
            Some(UseInfoAsk::Inspect(form)) => actions.push(Action::Inspect(form)),
            Some(UseInfoAsk::Page(cursor)) => {
                if let Some(req) = self.shell.use_info.page(cursor) {
                    self.ck_submit(Pending::UseInfo, req);
                }
            }
            None => {}
        }

        // Cell View (M7.W2): worldspaces, cells and references over `cell`.
        let mut cell_ask = None;
        if self.shell.is_open(WindowKind::CellView) {
            // Opening it with nothing shown asks for the worldspaces once.
            if self.shell.cell_view.list_mode.is_none()
                && self.shell.cell_view.cell.is_none()
                && !self.shell.cell_view.list_pending
            {
                if let Some(req) = self.shell.cell_view.worlds() {
                    self.ck_submit(Pending::CellView, req);
                }
            }
            let kind = WindowKind::CellView;
            let state = self
                .shell
                .layout
                .entry(kind.title().to_string())
                .or_default();
            let c = &self.shell.cell_view;
            windows::floating(
                ctx,
                state,
                &mut self.shell.dirty,
                WinSpec {
                    key: kind.title(),
                    title: kind.title(),
                    default_pos: kind.default_pos(),
                    default_size: kind.default_size(),
                    focus: is_focus(kind.title()),
                },
                |ui| cell_ask = c.ui(ui, busy),
            );
        }
        match cell_ask {
            Some(CellViewAsk::Interiors) => {
                if let Some(req) = self.shell.cell_view.interiors() {
                    self.ck_submit(Pending::CellView, req);
                }
            }
            Some(CellViewAsk::World(form)) => {
                if let Some(req) = self.shell.cell_view.world(&form) {
                    self.ck_submit(Pending::CellView, req);
                }
            }
            Some(CellViewAsk::Cell(form)) => {
                if let Some(req) = self.shell.cell_view.cell(&form) {
                    self.ck_submit(Pending::CellView, req);
                }
            }
            Some(CellViewAsk::Page(cursor)) => {
                if let Some(req) = self.shell.cell_view.page(cursor) {
                    self.ck_submit(Pending::CellView, req);
                }
            }
            Some(CellViewAsk::Select(form)) => actions.push(Action::Select(form)),
            Some(CellViewAsk::Inspect(form)) => actions.push(Action::Inspect(form)),
            None => {}
        }

        // The edit dialog (M7.W3), read-only.
        let mut schema_req = None;
        if self.shell.is_open(WindowKind::EditDialog) {
            let kind = WindowKind::EditDialog;
            let title = match self.records.record_view.as_ref() {
                Some(r) if !r.signature.is_empty() => {
                    format!("{} {} (read-only)", r.signature, r.editor_id)
                }
                _ => kind.title().to_string(),
            };
            let state = self
                .shell
                .layout
                .entry(kind.title().to_string())
                .or_default();
            let d = &mut self.shell.dialog;
            let record = self.records.record_view.as_ref();
            windows::floating(
                ctx,
                state,
                &mut self.shell.dirty,
                WinSpec {
                    key: kind.title(),
                    title: &title,
                    default_pos: kind.default_pos(),
                    default_size: kind.default_size(),
                    focus: is_focus(kind.title()),
                },
                |ui| schema_req = d.ui(ui, record),
            );
        }
        if let Some(req) = schema_req {
            let sig = req
                .args
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            self.ck_submit(Pending::Schema(sig), req);
        }

        // Floating tabs (M7.M0): any tab as a window.
        let tour = self.tour.is_some();
        for (tab, label) in TABS {
            if !self.shell.is_floating(label) {
                continue;
            }
            let key = windows::tab_key(label);
            let mut ctx_tab = TabCtx {
                busy,
                tour,
                results: &self.results,
                plugins: &plugins,
                notice: &mut self.notice,
                actions: Vec::new(),
                theme: self.theme,
                theme_source: self.theme_source,
                last_profile: self.prefs.profile.as_deref(),
                prefs_path: self.prefs_path.as_deref(),
            };
            let state = self.shell.layout.entry(key.clone()).or_default();
            let (load, records, links, planet) = (
                &mut self.load,
                &mut self.records,
                &mut self.links,
                &mut self.planet,
            );
            let (diff, diag, validate, schema) = (
                &mut self.diff,
                &mut self.diag,
                &mut self.validate,
                &mut self.schema,
            );
            let (ledger, help, cockpit, coverage, render, start) = (
                &mut self.ledger,
                &mut self.help,
                &mut self.cockpit,
                &mut self.coverage,
                &mut self.render,
                &mut self.start,
            );
            let session = self.session.as_ref();
            windows::floating(
                ctx,
                state,
                &mut self.shell.dirty,
                WinSpec {
                    key: &key,
                    title: label,
                    default_pos: [160.0, 140.0],
                    default_size: [720.0, 520.0],
                    focus: is_focus(&key),
                },
                |ui| match tab {
                    Tab::Records => records.ui(ui, &mut ctx_tab),
                    Tab::Render => render.ui(ui, &mut ctx_tab),
                    _ => {
                        ScrollArea::both()
                            .id_salt(("float-tab-scroll", *tab))
                            .auto_shrink([false, false])
                            .show(ui, |ui| match tab {
                                Tab::Start => start.ui(ui, &mut ctx_tab),
                                Tab::Load => load.ui(ui, &mut ctx_tab),
                                Tab::Links => links.ui(ui, &mut ctx_tab),
                                Tab::Planet => planet.ui(ui, &mut ctx_tab),
                                Tab::Diff => diff.ui(ui, &mut ctx_tab),
                                Tab::Diagnostics => diag.ui(ui, &mut ctx_tab),
                                Tab::Validate => validate.ui(ui, &mut ctx_tab),
                                Tab::Schema => schema.ui(ui, &mut ctx_tab),
                                Tab::Ledger => ledger.ui(ui, &mut ctx_tab),
                                Tab::Help => help.ui(ui, &mut ctx_tab),
                                Tab::Cockpit => {
                                    let env = CockpitEnv {
                                        loaded_profile: session
                                            .and_then(|s| s.get("profile"))
                                            .and_then(Value::as_str),
                                        checkout: help.checkout.as_ref().map(|(p, _)| p.as_path()),
                                        ledger: ledger.observations.as_deref(),
                                    };
                                    cockpit.ui(ui, &mut ctx_tab, &env);
                                }
                                Tab::Coverage => {
                                    let checkout: Option<&Path> =
                                        help.checkout.as_ref().map(|(p, _)| p.as_path());
                                    coverage.ui(ui, &mut ctx_tab, checkout);
                                }
                                Tab::Records | Tab::Render => {}
                            });
                    }
                },
            );
            actions.append(&mut ctx_tab.actions);
        }
        self.apply(actions);

        // `ui.json`, when a window moved, resized, opened or closed (never in the tour).
        if self.shell.dirty {
            self.shell.dirty = false;
            if self.tour.is_none() {
                self.prefs.ck_windows = Some(windows::layout_to_json(&self.shell.layout));
                self.save_prefs();
            }
        }
    }

    /// Hook: the tour's CK-shell images once every tab is shot. `true` while it has
    /// images left to take (the caller then does nothing else this frame).
    pub(super) fn ck_tour(&mut self, ctx: &egui::Context) -> bool {
        let Some(dir) = self.tour.as_ref().map(|t| t.dir.clone()) else {
            return false;
        };
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        let Some(step) = TOUR.get(self.shell.tour_step).copied() else {
            self.shell.tour_menu = None;
            return false;
        };
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            if self.shell.tour_frames == 6 {
                let [w, h] = image.size;
                let rgba: Vec<[u8; 4]> = image.pixels.iter().map(|c| c.to_array()).collect();
                let path = dir.join(format!("ck-{step}.png"));
                match crate::shots::png(w, h, &rgba).map(|b| std::fs::write(&path, b)) {
                    Some(Ok(())) => {}
                    other => {
                        self.notice = Some(format!("screenshot {}: {other:?}", path.display()));
                    }
                }
                self.shell.tour_step = self.shell.tour_step.saturating_add(1);
                self.shell.tour_frames = 0;
            }
            return true;
        }
        // Frame 0 sets the step up; then, once the engine is idle and the windows have
        // faded in (real time: frames can come faster than the fade), frame 5 asks for
        // the image and frame 6 waits for it.
        let waiting = self.worker.is_busy()
            || self.shell.use_info.pending
            || self.shell.cell_view.list_pending
            || self.shell.cell_view.refs_pending
            || !self.shell.dialog.pending.is_empty();
        let settled = self
            .shell
            .tour_since
            .is_some_and(|t| t.elapsed() >= std::time::Duration::from_millis(600));
        match self.shell.tour_frames {
            0 => {
                egui::Popup::close_all(ctx);
                self.ck_tour_setup(step);
                self.shell.tour_since = Some(std::time::Instant::now());
            }
            1..=4 => self.shell.tour_frames = self.shell.tour_frames.saturating_add(1),
            5 if !waiting && settled => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.shell.tour_frames = 6;
            }
            _ => {}
        }
        true
    }

    fn ck_tour_setup(&mut self, step: &str) {
        let only = |shell: &mut windows::Shell, open: &[WindowKind]| {
            for k in WindowKind::ALL {
                shell.set_open(k, open.contains(&k));
            }
        };
        self.shell.tour_menu = None;
        self.tab = Tab::Load;
        match step {
            "menu-file" => {
                only(&mut self.shell, &[]);
                self.shell.tour_menu = Some("File".into());
            }
            "menu-view" => self.shell.tour_menu = Some("View".into()),
            "menu-record" => self.shell.tour_menu = Some("Record".into()),
            "menu-navmesh" => self.shell.tour_menu = Some("NavMesh".into()),
            "warnings" => only(&mut self.shell, &[WindowKind::Warnings]),
            "use-info" => only(&mut self.shell, &[WindowKind::UseInfo]),
            "edit-weap" => only(&mut self.shell, &[WindowKind::EditDialog]),
            "edit-default" => {
                self.apply(vec![Action::Inspect(TOUR_DEFAULT_FORM.to_string())]);
                self.tab = Tab::Load;
            }
            "layout" => {
                only(&mut self.shell, &WindowKind::ALL);
                self.shell.set_floating("Schema", true);
            }
            _ => {}
        }
        self.shell.tour_frames = 1;
    }
}
