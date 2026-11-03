//! The CK-style shell's glue (M7.M0, M7.M1, M7.M2, M7.M4, M7.W3, M7.W5): a child module of
//! `app.rs` (declared there with `#[path]`, so it can reach the shell's tabs and
//! state), holding everything `app.rs` calls through its one-line hooks:
//!
//! - `ck_menu_bar`: the menu bar from the catalog, above the tabs;
//! - `ck_after_top_bar`: a floating tab is never drawn twice (the central panel falls
//!   back to the last docked tab and the window comes to the front);
//! - `ck_windows`: the Warnings, Use Info, Edit Dialog, Data dialog (M7.M2), Object
//!   Window (M7.W1) and Preferences (M7.M3) windows and the floating tabs, then `ui.json` when the layout changed;
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
use crate::windows::data_dialog::{DataAsk, DataTicket};
use crate::windows::preferences::{PrefsAsk, PrefsView};
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
    pub(super) fn ck_selected(&self) -> Option<String> {
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
                // M7.M3: the zoom Preferences keeps.
                if let Some(z) = self.prefs.zoom {
                    ui.ctx().set_zoom_factor(z);
                }
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
        if let Some(p) = pick {
            self.ck_pick(p);
        }
        // M7.M5: the catalog's shortcuts (KitKraft's own keys; the CK's are unobserved).
        if self.tour.is_none() {
            if let Some(p) = menu::pressed(ui.ctx(), &menus) {
                self.ck_pick(p);
            }
        }
    }

    /// What a menu pick does: a tab, a window, the editor's form for a write, or a tab
    /// floated or docked.
    pub(super) fn ck_pick(&mut self, pick: MenuPick) {
        match pick {
            MenuPick::Open(Target::Tab(label)) => self.ck_open_tab(label),
            MenuPick::Open(Target::Window(kind)) => self.shell.set_open(kind, true),
            MenuPick::Open(Target::Editor(command)) => {
                let form = self.ck_selected();
                let ty = self
                    .records
                    .record_view
                    .as_ref()
                    .map(|r| r.signature.clone())
                    .filter(|s| !s.is_empty());
                self.apply(vec![editor_action(command, form.as_deref(), ty.as_deref())])
            }
            MenuPick::FloatTab(label) => {
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
            // Step 0 of sprint 2026-11-02: pipeline W's windows (M7.W1, M7.M2).
            Some(Pending::Object(t)) => self.shell.object_window.receive(&t, result),
            Some(Pending::Data(t)) => self.shell.data_dialog.receive(&t, result),
            // Pipeline E's editor (M7.E2 to M7.E6).
            Some(Pending::Editor(t)) => self.shell.editor.receive(&t, result),
            None => {}
        }
    }

    /// The Cell View is shown this frame (docked, or floating): shown again after an
    /// error, it clears it; shown with nothing in it, it asks for the worldspaces once.
    pub(super) fn ck_cell_view_shown(&mut self) {
        if !self.shell.cell_view_open && !self.shell.cell_view_shown {
            self.shell.cell_view.on_reopen();
        }
        self.shell.cell_view_shown = true;
        if self.shell.cell_view.list_mode.is_none()
            && self.shell.cell_view.cell.is_none()
            && !self.shell.cell_view.list_pending
        {
            if let Some(req) = self.shell.cell_view.worlds() {
                self.ck_submit(Pending::CellView, req);
            }
        }
        // E's ask D2: a new load (the active file written, then loaded) refreshes the
        // shown cell's references, so a placed reference shows.
        if let Some(req) = self.shell.cell_view.reload_on(self.load.data) {
            self.ck_submit(Pending::CellView, req);
        }
    }

    /// What a click in the Cell View asked for: its own `cell` pages, or an action for the
    /// shell (the shared selection, the inspector, the Render Window).
    pub(super) fn ck_cell_view_ask(&mut self, ask: Option<CellViewAsk>, actions: &mut Vec<Action>) {
        match ask {
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
            Some(CellViewAsk::ShowCell(form)) => {
                // The Render Window's own slot, so the reply reaches its `receive_page`
                // unchanged; the tab (or floating window) comes to the front with it.
                actions.push(Action::Submit(
                    Slot::RenderCell,
                    self.render.show_cell(&form),
                ));
                self.ck_open_tab("Render Window");
            }
            Some(CellViewAsk::Page(cursor)) => {
                if let Some(req) = self.shell.cell_view.page(cursor) {
                    self.ck_submit(Pending::CellView, req);
                }
            }
            Some(CellViewAsk::Select(form)) => actions.push(Action::Select(form)),
            Some(CellViewAsk::Inspect(form)) => actions.push(Action::Inspect(form)),
            // Pipeline E's place bar (M7.E4; step 0 of sprint 2026-11-02).
            Some(CellViewAsk::Place(p)) => actions.push(Action::PlaceInCell {
                base: p.base,
                cell: p.cell,
            }),
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

        // The edit dialog (M7.W3; its field edits, M7.E2): what its buttons ask for, and
        // the `schema` request a type without a layout needs, once.
        let mut dialog_actions = Vec::new();
        let mut schema_req = None;
        if self.shell.is_open(WindowKind::EditDialog) {
            let kind = WindowKind::EditDialog;
            let title = match self.records.record_view.as_ref() {
                Some(r) if !r.signature.is_empty() => {
                    format!("{} {}", r.signature, r.editor_id)
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
                |ui| dialog_actions = d.ui(ui, record),
            );
            schema_req = self
                .records
                .record_view
                .as_ref()
                .map(|r| r.signature.clone())
                .filter(|s| !s.is_empty())
                .and_then(|sig| self.shell.dialog.schema_request(&sig));
        }
        actions.append(&mut dialog_actions);
        if let Some(req) = schema_req {
            let sig = req
                .args
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            self.ck_submit(Pending::Schema(sig), req);
        }

        // The Data dialog (M7.M2): the session's load order, the active file.
        let mut data_ask = None;
        if self.shell.is_open(WindowKind::DataDialog) {
            let kind = WindowKind::DataDialog;
            let state = self
                .shell
                .layout
                .entry(kind.title().to_string())
                .or_default();
            let d = &mut self.shell.data_dialog;
            d.sync(
                self.load.view.as_ref(),
                self.load.data,
                self.active_file.as_deref(),
            );
            let active = self.active_file.as_deref();
            let session = self.session.as_ref();
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
                |ui| data_ask = d.ui(ui, busy, active, session),
            );
        } else {
            self.shell.data_dialog.closed();
        }
        match data_ask {
            Some(DataAsk::Details(path, req)) => {
                self.ck_submit(Pending::Data(DataTicket::Details(path)), req);
            }
            Some(DataAsk::Accept { load, active }) => {
                if let Some(req) = load {
                    actions.push(Action::Submit(Slot::Load, req));
                }
                actions.push(Action::SetActiveFile(active));
                self.shell.set_open(WindowKind::DataDialog, false);
            }
            Some(DataAsk::Cancel) => self.shell.set_open(WindowKind::DataDialog, false),
            None => {}
        }

        // The Object Window (M7.W1): the category tree from `schema`, a type's records.
        let mut object_actions = Vec::new();
        if self.shell.is_open(WindowKind::ObjectWindow) {
            self.shell.object_window.shown();
            let kind = WindowKind::ObjectWindow;
            let state = self
                .shell
                .layout
                .entry(kind.title().to_string())
                .or_default();
            let o = &mut self.shell.object_window;
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
                |ui| object_actions = o.ui(ui, busy),
            );
        }
        if let Some((ticket, req)) = self.shell.object_window.take_request() {
            self.ck_submit(Pending::Object(ticket), req);
        }
        actions.extend(object_actions);

        // Preferences (M7.M3): the workbench's settings, kept in `ui.json`.
        let mut pref_asks = Vec::new();
        if self.shell.is_open(WindowKind::Preferences) {
            let kind = WindowKind::Preferences;
            let view = PrefsView {
                theme: self.theme,
                theme_source: self.theme_source,
                zoom: ctx.zoom_factor(),
                instance: self.prefs.instance.as_deref(),
                profile: self.prefs.profile.as_deref(),
                checkout: self.prefs.repo_dir.as_deref(),
                prefs_path: self.prefs_path.as_deref(),
                catalog: crate::gate::catalog(),
            };
            let state = self
                .shell
                .layout
                .entry(kind.title().to_string())
                .or_default();
            let p = &mut self.shell.preferences;
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
                |ui| pref_asks = p.ui(ui, &view),
            );
        } else {
            self.shell.preferences.closed();
        }
        for ask in pref_asks {
            match ask {
                PrefsAsk::Theme(choice) => actions.push(Action::SetTheme(choice)),
                PrefsAsk::Checkout(dir) => actions.push(Action::UseCheckout(dir)),
                PrefsAsk::Zoom(z) => {
                    ctx.set_zoom_factor(z);
                    self.prefs.zoom = Some(z);
                    self.save_prefs();
                }
            }
        }

        // Pipeline E's editor (M7.E2 to M7.E6; step 0 of sprint 2026-11-02): its
        // per-frame hook, with the active file and the shared selection.
        actions.extend(crate::dialogs::editor::ui(
            ctx,
            &mut self.shell.editor,
            busy,
            self.active_file.as_deref(),
            selected.as_deref(),
        ));

        // Floating tabs (M7.M0): any tab as a window.
        let tour = self.tour.is_some();
        let mut cell_ask = None;
        for (tab, label) in TABS {
            if !self.shell.is_floating(label) {
                continue;
            }
            if *tab == Tab::CellView {
                self.ck_cell_view_shown();
            }
            let cell_inspected = self.ck_selected();
            let cell_rendered = self.render.shown_cell().map(str::to_string);
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
                active_file: self.active_file.as_deref(),
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
            let cell_view = &mut self.shell.cell_view;
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
                    Tab::CellView => {
                        cell_ask = cell_view_tab(
                            ui,
                            cell_view,
                            busy,
                            cell_inspected.as_deref(),
                            cell_rendered.as_deref(),
                        );
                    }
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
                                Tab::Records | Tab::Render | Tab::CellView => {}
                            });
                    }
                },
            );
            actions.append(&mut ctx_tab.actions);
        }
        self.ck_cell_view_ask(cell_ask.take(), &mut actions);
        // The Cell View that was not shown this frame is closed: shown again after an
        // error, it asks for the worldspaces again.
        self.shell.cell_view_open = std::mem::take(&mut self.shell.cell_view_shown);
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

/// The action a menu item for editor write `command` sends (E's asks B1 and C1): the editor
/// opens that command's form. `clone` (Clone Form..., Edit > Duplicate) copies the
/// inspected record `form`; `new` starts from the inspected record's type `ty`, or none;
/// `build` names no record (its spec names its own).
pub(super) fn editor_action(command: &str, form: Option<&str>, ty: Option<&str>) -> Action {
    let on = match command {
        "clone" => form,
        "new" => ty,
        _ => None,
    };
    Action::OpenEditor {
        command: command.to_string(),
        form: on.unwrap_or_default().to_string(),
    }
}

/// The Cell View tab (M7.W2; a tab since owner finding 1, 2026-11-01): the panel frame's
/// head, then the window's lists (`crate::windows::cell_view`).
pub(super) fn cell_view_tab(
    ui: &mut egui::Ui,
    state: &mut crate::windows::cell_view::CellViewState,
    busy: bool,
    inspected: Option<&str>,
    rendered: Option<&str>,
) -> Option<CellViewAsk> {
    crate::panels::common::panel_head(
        ui,
        "Cell View",
        "The worldspaces (\"Interior\" first), a world's exterior cells or the interior cells on the left, the clicked cell's references on the right (`cell`, paged). Click a reference to select it, double-click to inspect it.",
    );
    state.ui(ui, busy, inspected, rendered)
}
