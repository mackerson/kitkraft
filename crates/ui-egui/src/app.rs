//! The egui window's shell: the top bar (title and tabs), the status line (the engine's
//! state and the session), and the central panel, which hands the frame to the open
//! tab's `ui` (`crate::panels`). Thin by design: every action is a [`Request`] to the
//! worker; every value shown comes from a view-model built from the envelope the engine
//! returned; a tab's clicks come back as [`Action`]s and are applied here after the
//! frame. Colors, sizes and spacing are `crate::theme`'s.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, ScrollArea};
use serde_json::{Map, Value};

use crate::cmdline::{mcp_call, result_line, session_args};
use crate::envelope::Prepared;
use crate::gate::{Request, RunError};
use crate::panels::common::{Action, Shown, Slot, SlotKey, TabCtx};
use crate::panels::diag::{self, DiagTab, DiagView};
use crate::panels::diff::{DiffMode, DiffTab, DiffView};
use crate::panels::help::{self, HelpTab};
use crate::panels::ledger::{ledger_path, template_row, LedgerForm, LedgerTab, LedgerView};
use crate::panels::links::{LinksForm, LinksTab, LinksView};
use crate::panels::load::{list_profiles, LoadForm, LoadTab, LoadView};
use crate::panels::planet::{PlanetTab, PlanetView};
use crate::panels::records::{
    coverage_request, inspect_request, schema_request, types_from_coverage, types_from_schema,
    QueryView, RecordView, RecordsTab,
};
use crate::panels::render::RenderTab;
use crate::panels::schema::{SchemaTab, SchemaView};
use crate::panels::start::{
    instance_of_profile, profiles_request, StartProfile, StartTab, StartView,
};
use crate::panels::validate::{RoundtripView, ValidateTab, ValidateView};
// Lane G (M6).
use crate::panels::cockpit::{CockpitEnv, CockpitTab};
use crate::panels::coverage::{CoverageTab, MatrixView};
use crate::prefs::{self, Prefs};
use crate::theme::{self, ThemeChoice};
use crate::worker::{Reply, Wake, Worker};
use crate::{LaunchOptions, UiError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Tab {
    Start,
    Load,
    Records,
    Links,
    Planet,
    Diff,
    Diagnostics,
    Validate,
    Schema,
    Ledger,
    Help,
    // Lane G (M6.G2, M6.G7).
    Cockpit,
    Coverage,
    Render,
}

const TABS: &[(Tab, &str)] = &[
    (Tab::Start, "Start"),
    (Tab::Load, "Load"),
    (Tab::Records, "Records"),
    (Tab::Links, "Links"),
    (Tab::Planet, "Planet"),
    (Tab::Diff, "Diff"),
    (Tab::Diagnostics, "Diagnostics"),
    (Tab::Validate, "Validate"),
    (Tab::Schema, "Schema"),
    (Tab::Ledger, "Ledger"),
    (Tab::Help, "Help"),
    (Tab::Cockpit, "Cockpit"),
    (Tab::Coverage, "Coverage"),
    (Tab::Render, "Render Window"),
];

/// The screenshot tour (`crate::shots`): seed every tab with read-only requests, then
/// save one image per tab and close.
struct Tour {
    dir: PathBuf,
    step: TourStep,
    tab: usize,
    frames: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TourStep {
    WaitLoad,
    Seeded(u8),
    Shooting,
    Done,
}

struct App {
    tour: Option<Tour>,
    /// The preset and mode in use, and where the choice came from (`--theme`, `ui.json`,
    /// the Start tab's picker, or the default).
    theme: ThemeChoice,
    theme_source: &'static str,
    /// The picker changed the theme: apply it before the next frame.
    theme_changed: bool,
    worker: Worker,
    tab: Tab,
    slots: BTreeMap<u64, Slot>,
    notice: Option<String>,
    results: BTreeMap<SlotKey, Shown>,
    /// Load-order arguments of the `load` that succeeded last: what the engine's session
    /// holds (a failed `load` keeps the previous one). `None` until one succeeds, and
    /// again after the worker restarts.
    session: Option<Map<String, Value>>,

    /// The per-user prefs (`crate::prefs`) and where they live (`None`: no
    /// `%LOCALAPPDATA%`, nothing is remembered).
    prefs: Prefs,
    prefs_path: Option<PathBuf>,

    start: StartTab,
    load: LoadTab,
    records: RecordsTab,
    links: LinksTab,
    planet: PlanetTab,
    diff: DiffTab,
    diag: DiagTab,
    validate: ValidateTab,
    schema: SchemaTab,
    ledger: LedgerTab,
    help: HelpTab,
    cockpit: CockpitTab,
    coverage: CoverageTab,
    render: RenderTab,
}

/// Opens the window and blocks until it closes.
pub(crate) fn run_window(opts: LaunchOptions) -> Result<(), UiError> {
    let fonts = fonts()?;
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KitKraft Workbench (read-only)")
            .with_inner_size(if std::env::var_os(crate::shots::SHOTS_ENV).is_some() {
                crate::shots::TOUR_SIZE
            } else {
                theme::WINDOW
            }),
        // A depth buffer for the Render Window's viewport (M7.R1); egui draws without one.
        depth_buffer: 24,
        ..Default::default()
    };
    eframe::run_native(
        "KitKraft Workbench",
        native,
        Box::new(move |cc| {
            cc.egui_ctx.set_fonts(fonts);
            let ctx = cc.egui_ctx.clone();
            let wake: Wake = std::sync::Arc::new(move || ctx.request_repaint());
            let worker = Worker::spawn(Some(wake))?;
            let app = App::new(worker, opts);
            theme::apply(&cc.egui_ctx, &app.theme.preset.theme(), app.theme.mode);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| UiError::Window(e.to_string()))
}

/// System fonts on Windows (no font is compiled in unless the `default_fonts` feature
/// is on; see ADR 0007, dependency list).
fn fonts() -> Result<FontDefinitions, UiError> {
    if cfg!(feature = "default_fonts") {
        return Ok(FontDefinitions::default());
    }
    let windir = std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let dir = windir.join("Fonts");
    let read = |names: &[&str]| -> Option<Vec<u8>> {
        names.iter().find_map(|n| std::fs::read(dir.join(n)).ok())
    };
    let prop = read(&["segoeui.ttf", "arial.ttf", "tahoma.ttf"]).ok_or_else(|| {
        UiError::NoFont(format!(
            "no Segoe UI, Arial or Tahoma in {}; build with `--features default_fonts`",
            dir.display()
        ))
    })?;
    let mono = read(&["consola.ttf", "cour.ttf"]);
    let mut defs = FontDefinitions::empty();
    defs.font_data.insert(
        "prop".into(),
        std::sync::Arc::new(FontData::from_owned(prop)),
    );
    let mono_name = if let Some(m) = mono {
        defs.font_data
            .insert("mono".into(), std::sync::Arc::new(FontData::from_owned(m)));
        "mono"
    } else {
        "prop"
    };
    defs.families.insert(
        FontFamily::Proportional,
        vec!["prop".into(), mono_name.into()],
    );
    defs.families
        .insert(FontFamily::Monospace, vec![mono_name.into(), "prop".into()]);
    Ok(defs)
}

impl App {
    fn new(worker: Worker, opts: LaunchOptions) -> Self {
        let load_form = LoadForm {
            profile: opts.profile.clone().unwrap_or_default(),
            plugins: opts.plugins.join("\n"),
            data_dir: opts.data_dir.clone().unwrap_or_default(),
            no_cache: false,
        };
        let profiles_dir = opts
            .profile
            .as_deref()
            .and_then(|p| Path::new(p).parent())
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        // First run: no load-order arguments, so the start screen opens.
        let first_run = opts.profile.is_none() && opts.plugins.is_empty();
        let prefs_path = prefs::default_path();
        let prefs = prefs_path.as_deref().map(Prefs::load).unwrap_or_default();
        // The launcher's profile names its instance; else the last one used, else the
        // engine looks.
        let start_instance = opts
            .profile
            .as_deref()
            .and_then(instance_of_profile)
            .or_else(|| prefs.instance.clone())
            .unwrap_or_default();
        // `--theme` wins over `ui.json`; an unknown name in the file falls back to the
        // default (Workbench, M6.O1) with a notice (the window always opens).
        let mut theme_notice = None;
        let (theme, theme_source) = match (opts.theme, prefs.theme.as_deref()) {
            (Some(choice), _) => (choice, "--theme"),
            (None, Some(name)) => match ThemeChoice::parse(name) {
                Ok(choice) => (choice, "ui.json"),
                Err(why) => {
                    theme_notice = Some(format!("ui.json: {why}; using the default"));
                    (ThemeChoice::default(), "default")
                }
            },
            (None, None) => (ThemeChoice::default(), "default"),
        };
        let records = RecordsTab {
            split: prefs.records_split,
            ..RecordsTab::default()
        };
        let mut app = Self {
            theme,
            theme_source,
            theme_changed: false,
            worker,
            tab: if first_run { Tab::Start } else { Tab::Load },
            slots: BTreeMap::new(),
            notice: None,
            results: BTreeMap::new(),
            session: None,
            prefs,
            prefs_path,
            start: StartTab {
                instance: start_instance,
                view: None,
            },
            load: LoadTab {
                profiles: if profiles_dir.is_empty() {
                    Vec::new()
                } else {
                    list_profiles(Path::new(&profiles_dir))
                },
                form: load_form,
                profiles_dir,
                view: None,
            },
            records,
            links: LinksTab::default(),
            planet: PlanetTab::default(),
            diff: DiffTab::default(),
            diag: DiagTab::default(),
            validate: ValidateTab::default(),
            schema: SchemaTab::default(),
            ledger: LedgerTab::default(),
            help: HelpTab::default(),
            cockpit: CockpitTab::default(),
            coverage: CoverageTab::default(),
            tour: None,
            render: RenderTab::default(),
        };
        let mut shots_refused = None;
        if let Some(value) = std::env::var_os(crate::shots::SHOTS_ENV) {
            match crate::shots::shots_dir(&value) {
                Ok(dir) => {
                    app.tour = Some(Tour {
                        dir,
                        step: TourStep::WaitLoad,
                        tab: 0,
                        frames: 0,
                    });
                }
                Err(why) => shots_refused = Some(why),
            }
        }
        app.submit(
            Slot::Types,
            Request::new("schema", Value::Object(Map::new())),
        );
        let req = profiles_request(Some(&app.start.instance));
        app.submit(Slot::Profiles, req);
        if opts.profile.is_some() || !opts.plugins.is_empty() {
            if let Ok(req) = app.load.form.request() {
                app.submit(Slot::Load, req);
            }
        }
        app.reload_help();
        if shots_refused.is_some() {
            app.notice = shots_refused;
        } else if theme_notice.is_some() {
            app.notice = theme_notice;
        }
        app
    }

    fn submit(&mut self, slot: Slot, request: Request) {
        match self.worker.submit(request) {
            Ok(ticket) => {
                self.slots.insert(ticket, slot);
                self.notice = None;
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    fn receive(&mut self, reply: Reply) {
        let Some(slot) = self.slots.remove(&reply.ticket) else {
            return;
        };
        if let Ok(env) = &reply.result {
            match slot {
                Slot::Profiles => {
                    let view = StartView::from_envelope(env);
                    if self.start.instance.trim().is_empty() {
                        self.start.instance = view.instance.clone().unwrap_or_default();
                    }
                    self.start.view = Some(view);
                }
                Slot::Load => {
                    self.load.view = Some(LoadView::from_envelope(env));
                    self.session = Some(session_args(&reply.request.args));
                }
                Slot::Types => {
                    self.records.type_rows = if reply.request.id == "coverage" {
                        types_from_coverage(env)
                    } else {
                        types_from_schema(env)
                    };
                }
                Slot::Query => self.records.query_view = Some(QueryView::from_envelope(env)),
                Slot::Inspect => {
                    let view = RecordView::from_envelope(env);
                    if !view.signature.is_empty() {
                        let req = schema_request(&view.signature);
                        self.submit(Slot::InspectSchema, req);
                    }
                    self.records.record_view = Some(view);
                }
                Slot::InspectSchema => {
                    if let Some(view) = &mut self.records.record_view {
                        view.join_schema(env);
                    }
                }
                Slot::Links => self.links.view = Some(LinksView::from_envelope(env)),
                Slot::Planet => self.planet.view = Some(PlanetView::from_envelope(env)),
                Slot::Diff => self.diff.view = Some(DiffView::from_envelope(env)),
                Slot::DiagInspect => {
                    let view = DiagView::from_inspect(env, self.load.view.as_ref());
                    if self.diag.validate_plugin.trim().is_empty() {
                        self.diag.validate_plugin = self
                            .load
                            .view
                            .as_ref()
                            .map_or_else(|| view.winner.clone(), |l| l.path_of(&view.winner));
                    }
                    self.diag.view = Some(view);
                }
                Slot::DiagValidate => {
                    if let Some(view) = &mut self.diag.view {
                        view.with_validate(env);
                    }
                }
                Slot::Validate => self.validate.view = Some(ValidateView::from_envelope(env)),
                Slot::Roundtrip => {
                    self.validate.roundtrip = Some(RoundtripView::from_envelope(env));
                }
                Slot::Schema => self.schema.view = Some(SchemaView::from_envelope(env)),
                Slot::Ledger => {
                    let view = LedgerView::from_envelope(env);
                    if view.appended {
                        // One row per confirmed click: the note does not carry over.
                        self.ledger.form.note.clear();
                        self.reload_help();
                    }
                    self.ledger.view = Some(view);
                }
                Slot::CockpitStatus
                | Slot::CockpitValidate
                | Slot::CockpitRoundtrip
                | Slot::CockpitMasters
                | Slot::CockpitDeploy
                | Slot::CockpitRollback => {
                    for (s, r) in self.cockpit.receive(slot, &reply.request, env) {
                        self.submit(s, r);
                    }
                }
                Slot::Coverage => self.coverage.view = Some(MatrixView::from_envelope(env)),
            }
        }
        // Replies arrive in submission order, so the session now is the one this
        // request ran against.
        let line = result_line(
            &reply.request.id,
            &reply.request.args,
            self.session.as_ref(),
        );
        let mcp = mcp_call(&reply.request.id, &reply.request.args);
        let (prepared, error) = match reply.result {
            Ok(env) => (Prepared::new(Some(&env), None), None),
            Err(err) => {
                let json = match &err {
                    RunError::Failed { json, .. } => Some(json),
                    RunError::Refused(_) => None,
                };
                (Prepared::new(None, json), Some(err))
            }
        };
        self.results.insert(
            slot.into(),
            Shown {
                request: reply.request,
                error,
                elapsed: reply.elapsed,
                prepared,
                line,
                mcp,
            },
        );
    }

    /// Drops every queued request by restarting the engine thread; the new one has an
    /// empty session, so the load is forgotten too.
    fn drop_pending(&mut self) {
        match self.worker.restart() {
            Ok(n) => {
                self.slots.clear();
                self.session = None;
                self.load.view = None;
                self.results.remove(&Slot::Load.into());
                self.notice = Some(format!(
                    "dropped {n} request(s); the engine restarted with an empty session: load again"
                ));
            }
            Err(e) => self.notice = Some(format!("could not restart the engine: {e}")),
        }
    }

    fn apply(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::Submit(slot, req) => self.submit(slot, req),
                Action::Inspect(form) => {
                    self.records.record_form.clone_from(&form);
                    if let Ok(req) = inspect_request(&form, None) {
                        self.submit(Slot::Inspect, req);
                    }
                    self.tab = Tab::Records;
                }
                Action::Diagnose(form) => {
                    self.diag.form.clone_from(&form);
                    if let Ok(req) = inspect_request(&form, None) {
                        self.submit(Slot::DiagInspect, req);
                    }
                    self.tab = Tab::Diagnostics;
                }
                Action::Links(form, incoming) => {
                    self.links.form = if incoming {
                        LinksForm::use_info(&form)
                    } else {
                        LinksForm::outgoing(&form)
                    };
                    self.apply(vec![Action::LinksPage(None)]);
                    self.tab = Tab::Links;
                }
                Action::LinksPage(cursor) => {
                    self.links.cursor.clone_from(&cursor);
                    match self.links.form.request(cursor.as_deref()) {
                        Ok(req) => self.submit(Slot::Links, req),
                        Err(e) => self.notice = Some(e),
                    }
                }
                Action::Planet(form) => {
                    self.planet.form.form = form;
                    self.apply(vec![Action::PlanetPage(None)]);
                    self.tab = Tab::Planet;
                }
                Action::PlanetPage(cursor) => {
                    self.planet.cursor.clone_from(&cursor);
                    match self.planet.form.request(cursor.as_deref()) {
                        Ok(req) => self.submit(Slot::Planet, req),
                        Err(e) => self.notice = Some(e),
                    }
                }
                Action::PickType(sig) => {
                    self.records.query_form.record_type = sig;
                    self.records.query_cursor = None;
                    if let Ok(req) = self.records.query_form.request(None) {
                        self.submit(Slot::Query, req);
                    }
                }
                Action::QueryPage(cursor) => {
                    self.records.query_cursor.clone_from(&cursor);
                    match self.records.query_form.request(cursor.as_deref()) {
                        Ok(req) => self.submit(Slot::Query, req),
                        Err(e) => self.notice = Some(e),
                    }
                }
                Action::UseProfile(path) => {
                    self.load.form.profile = path.display().to_string();
                }
                Action::LoadProfile(profile) => match profile.load_request() {
                    Ok(req) => {
                        self.load.form = LoadForm {
                            profile: profile.path.clone(),
                            ..LoadForm::default()
                        };
                        self.submit(Slot::Load, req);
                        self.remember(&profile);
                        self.tab = Tab::Load;
                    }
                    Err(e) => self.notice = Some(e),
                },
                Action::LedgerAppend(form) => match form.append_request() {
                    Ok(req) => self.submit(Slot::Ledger, req),
                    Err(e) => self.notice = Some(e),
                },
                Action::SetTheme(choice) => {
                    self.theme = choice;
                    self.theme_source = "the Start tab (kept in ui.json)";
                    self.theme_changed = true;
                    self.prefs.theme = Some(choice.id());
                    self.save_prefs();
                }
                Action::UseCheckout(dir) => {
                    if help::is_checkout(Path::new(&dir)) {
                        self.prefs.repo_dir = Some(dir);
                        self.save_prefs();
                        self.reload_help();
                    } else {
                        self.notice = Some(format!(
                            "{dir}: not a KitKraft checkout (no AGENTS.md and docs there)"
                        ));
                    }
                }
                Action::ReloadHelp => self.reload_help(),
            }
        }
    }

    /// Writes the start screen's choice to the prefs file (the one file the workbench
    /// writes on its own; `crate::prefs`).
    fn remember(&mut self, profile: &StartProfile) {
        self.prefs.profile = Some(profile.path.clone());
        self.prefs.instance = self
            .start
            .view
            .as_ref()
            .and_then(|v| v.instance.clone())
            .or_else(|| instance_of_profile(&profile.path));
        self.save_prefs();
    }

    /// Finds the checkout again and rereads the Help tab's docs.
    fn reload_help(&mut self) {
        self.help.checkout = help::find_checkout_here(self.prefs.repo_dir.as_deref());
        match &self.help.checkout {
            Some((root, _)) => {
                self.help.field = root.display().to_string();
                self.help.docs = help::load(root);
                let ledger = ledger_path(root);
                self.ledger.observations = std::fs::read_to_string(&ledger).ok();
                if self.ledger.form.file.trim().is_empty() {
                    self.ledger.form.file = ledger.display().to_string();
                }
            }
            None => self.help.docs.clear(),
        }
    }

    fn save_prefs(&mut self) {
        if let Some(path) = &self.prefs_path {
            if let Err(e) = self.prefs.save(path) {
                self.notice = Some(format!("UI prefs not saved: {e}"));
            }
        }
    }

    fn loaded_plugins(&self) -> Vec<(String, String)> {
        self.load
            .view
            .as_ref()
            .map(LoadView::files)
            .unwrap_or_default()
    }

    /// The top bar: the title and the tabs (they wrap rather than push anything off the
    /// window), and the notice line under them.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let top = theme::top_frame(ui);
        egui::Panel::top("kitkraft-tabs").frame(top).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("KitKraft Workbench");
                ui.separator();
                for (tab, label) in TABS {
                    ui.selectable_value(&mut self.tab, *tab, *label);
                }
            });
            if let Some(n) = &self.notice {
                ui.colored_label(theme::bad(ui), n);
            }
        });
    }

    /// The status line at the bottom: the engine's state (running, queued, drop pending,
    /// or idle) and the session, always visible whatever the window's width.
    fn status_line(&mut self, ui: &mut egui::Ui, busy: bool) {
        let frame = theme::top_frame(ui);
        let mut drop_pending = false;
        egui::Panel::bottom("kitkraft-status")
            .frame(frame)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if busy {
                        ui.spinner();
                        let pending = self.worker.pending();
                        ui.label(format!(
                            "running: {}",
                            pending.first().copied().unwrap_or_default()
                        ));
                        let queued = self.worker.queued();
                        if queued > 0 {
                            ui.label(format!("queued {queued}"));
                        }
                        drop_pending = ui
                            .small_button("drop pending")
                            .on_hover_text(
                                "restart the engine: queued requests are dropped, the running one's \
                                 reply is discarded, and the session is emptied (load again)",
                            )
                            .clicked();
                    } else {
                        ui.weak("idle");
                    }
                    ui.separator();
                    match &self.load.view {
                        Some(l) => ui.label(format!(
                            "session: {} ({} plugins)",
                            l.source,
                            l.plugins.len()
                        )),
                        None => ui.weak("session: nothing loaded"),
                    };
                    ui.separator();
                    ui.weak("read-only: every view is one engine command");
                });
            });
        if drop_pending {
            self.drop_pending();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.theme_changed {
            self.theme_changed = false;
            theme::apply(ui.ctx(), &self.theme.preset.theme(), self.theme.mode);
        }
        for reply in self.worker.poll() {
            self.receive(reply);
        }
        let busy = self.worker.is_busy();
        if busy {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
        self.top_bar(ui);
        self.status_line(ui, busy);
        let plugins = self.loaded_plugins();
        let tab = self.tab;
        let tour = self.tour.is_some();
        let mut ctx = TabCtx {
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
        let central = theme::central_frame(ui);
        egui::CentralPanel::default_margins()
            .frame(central)
            .show(ui, |ui| {
                if tab == Tab::Records {
                    // Two panes, each with its own scroll (the inspector beside the table).
                    self.records.ui(ui, &mut ctx);
                    return;
                }
                if tab == Tab::Render {
                    // The viewport fills the tab and takes the wheel: no scroll area.
                    self.render.ui(ui, &mut ctx);
                    return;
                }
                ScrollArea::vertical()
                    .id_salt(("tab-scroll", tab))
                    // The tour shoots each tab in one step (an animated scroll restarted
                    // every frame never arrives).
                    .animated(!tour)
                    .auto_shrink([false, false])
                    .show(ui, |ui| match tab {
                        Tab::Start => self.start.ui(ui, &mut ctx),
                        Tab::Load => self.load.ui(ui, &mut ctx),
                        Tab::Records => {}
                        Tab::Links => self.links.ui(ui, &mut ctx),
                        Tab::Planet => self.planet.ui(ui, &mut ctx),
                        Tab::Diff => self.diff.ui(ui, &mut ctx),
                        Tab::Diagnostics => self.diag.ui(ui, &mut ctx),
                        Tab::Validate => self.validate.ui(ui, &mut ctx),
                        Tab::Schema => self.schema.ui(ui, &mut ctx),
                        Tab::Ledger => self.ledger.ui(ui, &mut ctx),
                        Tab::Help => self.help.ui(ui, &mut ctx),
                        Tab::Cockpit => {
                            let env = CockpitEnv {
                                loaded_profile: self
                                    .session
                                    .as_ref()
                                    .and_then(|s| s.get("profile"))
                                    .and_then(Value::as_str),
                                checkout: self.help.checkout.as_ref().map(|(p, _)| p.as_path()),
                                ledger: self.ledger.observations.as_deref(),
                            };
                            self.cockpit.ui(ui, &mut ctx, &env);
                        }
                        Tab::Coverage => {
                            let checkout = self.help.checkout.as_ref().map(|(p, _)| p.as_path());
                            self.coverage.ui(ui, &mut ctx, checkout);
                        }
                        Tab::Render => {}
                    });
            });
        let actions = ctx.actions;
        self.apply(actions);
        if let Some(share) = self.records.split_changed.take() {
            self.prefs.records_split = Some(share);
            self.save_prefs();
        }
        if self.tour.is_some() {
            self.drive_tour(ui.ctx());
        }
    }
}

impl App {
    /// One step of the screenshot tour (only with `KITKRAFT_WORKBENCH_SHOTS` set).
    fn drive_tour(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(Duration::from_millis(100));
        let busy = self.worker.is_busy();
        let Some(step) = self.tour.as_ref().map(|t| t.step) else {
            return;
        };
        match step {
            TourStep::WaitLoad if !busy && self.load.view.is_some() => {
                let q = &mut self.records.query_form;
                q.record_type = "WEAP".into();
                q.where_text = "FULL ~ 'Rifle'".into();
                q.limit = 20;
                self.apply(vec![Action::QueryPage(None)]);
                self.set_step(TourStep::Seeded(1));
            }
            TourStep::Seeded(1) if !busy => {
                let form = self
                    .records
                    .query_view
                    .as_ref()
                    .and_then(|q| q.rows.first())
                    .map(|r| r.form.clone());
                // The biggest plugin the profile itself activates.
                let plugin = self.load.view.as_ref().and_then(|l| {
                    l.plugins
                        .iter()
                        .filter(|p| p.active == "plugins.txt")
                        .max_by_key(|p| p.records.unwrap_or(0))
                        .map(|p| (p.name.clone(), l.path_of(&p.name)))
                });
                let mut actions = Vec::new();
                if let Some(form) = form {
                    actions.push(Action::Inspect(form.clone()));
                    actions.push(Action::Links(form.clone(), true));
                    actions.push(Action::Diagnose(form));
                }
                // A Planet Content Manager tree of Starfield.esm (`planet_corpus.rs`).
                self.planet.form.limit = 60;
                actions.push(Action::Planet("PCM_QuestRequest".into()));
                if let Some((name, path)) = plugin {
                    self.records.coverage_plugin.clone_from(&path);
                    actions.push(Action::Submit(Slot::Types, coverage_request(&path)));
                    self.diff.form.mode = DiffMode::VsMasters;
                    self.diff.form.a = name;
                    if let Ok(req) = self.diff.form.request(None) {
                        actions.push(Action::Submit(Slot::Diff, req));
                    }
                    self.validate.form.plugin = path;
                    if let Ok(req) = self.validate.form.request() {
                        actions.push(Action::Submit(Slot::Validate, req));
                    }
                    if let Ok(req) = self.validate.form.roundtrip_request() {
                        actions.push(Action::Submit(Slot::Roundtrip, req));
                    }
                }
                self.schema.form.record_type = "WEAP".into();
                actions.push(Action::Submit(Slot::Schema, self.schema.form.request()));
                // The Ledger tab: a preview of C3 (nothing written) with its confirm box
                // open, never clicked.
                self.ledger.form.row = "C3".into();
                if let Some(t) = self
                    .ledger
                    .observations
                    .as_deref()
                    .and_then(|o| template_row(o, "C3"))
                {
                    self.ledger.form.forms = t.forms.join(", ");
                }
                self.ledger.form.note = "screenshot tour: a preview, nothing written".into();
                if let Ok(req) = self.ledger.form.preview_request() {
                    actions.push(Action::Submit(Slot::Ledger, req));
                }
                self.ledger.pending = Some(LedgerForm::clone(&self.ledger.form));
                self.apply(actions);
                self.set_step(TourStep::Seeded(2));
            }
            TourStep::Seeded(2) if !busy => {
                self.seed_lane_g_tour();
                if let Ok(req) = diag::validate_request(&self.diag.validate_plugin) {
                    self.submit(Slot::DiagValidate, req);
                }
                self.set_step(TourStep::Seeded(3));
            }
            TourStep::Seeded(3) if !busy => self.set_step(TourStep::Shooting),
            TourStep::Shooting => self.shoot(ctx),
            _ => {}
        }
    }

    fn set_step(&mut self, step: TourStep) {
        if let Some(t) = &mut self.tour {
            t.step = step;
            t.frames = 0;
        }
    }

    fn shoot(&mut self, ctx: &egui::Context) {
        let Some(tour) = &mut self.tour else { return };
        let Some((tab, label)) = TABS.get(tour.tab) else {
            tour.step = TourStep::Done;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        self.tab = *tab;
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let rgba: Vec<[u8; 4]> = image.pixels.iter().map(|c| c.to_array()).collect();
            let path = tour.dir.join(format!("{}.png", label.to_ascii_lowercase()));
            match crate::shots::png(w, h, &rgba).map(|b| std::fs::write(&path, b)) {
                Some(Ok(())) => {}
                other => self.notice = Some(format!("screenshot {}: {other:?}", path.display())),
            }
            tour.tab = tour.tab.saturating_add(1);
            tour.frames = 0;
            return;
        }
        tour.frames = tour.frames.saturating_add(1);
        // A few frames for the tab to lay out, then one request (frame 5 only).
        if tour.frames == 5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
    }
}

// Lane G (M6.G2, M6.G7, M6.G8): the screenshot tour's reads for the Cockpit and Coverage
// tabs. Dry runs only: nothing is written.
impl App {
    fn seed_lane_g_tour(&mut self) {
        use crate::panels::cockpit::{dev_target, plugins_under, WriteKind, OUT_DIR};
        let checkout = self.help.checkout.as_ref().map(|(p, _)| p.clone());
        let found = checkout
            .as_deref()
            .map(|r| plugins_under(&r.join(OUT_DIR)))
            .unwrap_or_default();
        let plugins = self.loaded_plugins();
        if let Some(p) = found.first() {
            self.cockpit.plugin.clone_from(p);
            if let Ok(reqs) = self.cockpit.start_review(&plugins) {
                for (slot, req) in reqs {
                    self.submit(slot, req);
                }
            }
        }
        let loaded = self
            .session
            .as_ref()
            .and_then(|s| s.get("profile"))
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Ok(t) = dev_target(loaded.as_deref()) {
            self.cockpit.form.fill(&t);
            if let Ok(req) = self.cockpit.form.status_request() {
                self.submit(Slot::CockpitStatus, req);
            }
            if let Ok(req) = self.cockpit.form.preview_request(WriteKind::Deploy, None) {
                self.submit(Slot::CockpitDeploy, req);
            }
        }
        self.coverage.form.types = "WEAP AMMO OMOD".into();
        if let Some(root) = &checkout {
            self.coverage.form.evidence = root
                .join("docs")
                .join("schema")
                .join("evidence.toml")
                .display()
                .to_string();
        }
        if let Ok(req) = self.coverage.form.request() {
            self.submit(Slot::Coverage, req);
        }
    }
}
