//! The egui window: tabs over the panel view-models. Thin by design: every action is a
//! [`Request`] to the worker; every value shown comes from a view-model built from the
//! envelope the engine returned.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{
    self, CollapsingHeader, Color32, ComboBox, FontData, FontDefinitions, FontFamily, Grid,
    RichText, ScrollArea, TextEdit,
};
use serde_json::{Map, Value};

use crate::cmdline::{mcp_call, result_line, session_args, ResultLine};
use crate::envelope::Prepared;
use crate::gate::{Request, RunError};
use crate::md::{Block, Inline};
use crate::panels::diag::{self, DiagView};
use crate::panels::diff::{DiffForm, DiffMode, DiffView};
use crate::panels::help::{self, HelpDoc};
use crate::panels::links::{LinksForm, LinksView, MAX_DEPTH};
use crate::panels::load::{list_profiles, LoadForm, LoadView, ProfileEntry};
use crate::panels::planet::{PlanetForm, PlanetView, PLANET_TYPES};
use crate::panels::records::{
    coverage_request, inspect_request, schema_request, types_from_coverage, types_from_schema,
    QueryForm, QueryView, RecordView, TypeRow,
};
use crate::panels::schema::{SchemaForm, SchemaView};
use crate::panels::start::{instance_of_profile, profiles_request, StartProfile, StartView};
use crate::panels::validate::{RoundtripView, ValidateForm, ValidateView};
use crate::prefs::{self, Prefs};
use crate::worker::{Reply, Wake, Worker};
use crate::{LaunchOptions, UiError};

/// Rows of evidence shown per result; the raw JSON has the rest.
const EVIDENCE_ROWS: usize = 300;
/// Characters of a limitation shown on its line (the full text is on hover).
const LIMITATION_CHARS: usize = 200;

const WARN: Color32 = Color32::from_rgb(230, 170, 60);
const BAD: Color32 = Color32::from_rgb(230, 90, 80);
const GOOD: Color32 = Color32::from_rgb(110, 190, 110);
const CODE: Color32 = Color32::from_rgb(140, 190, 235);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Help,
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
    (Tab::Help, "Help"),
];

/// Where a reply goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Profiles,
    Load,
    Types,
    Query,
    Inspect,
    InspectSchema,
    Links,
    Planet,
    Diff,
    DiagInspect,
    DiagValidate,
    Validate,
    Roundtrip,
    Schema,
}

/// One finished request, with everything the frame draws built once on arrival.
struct Shown {
    request: Request,
    /// Why there is no envelope.
    error: Option<RunError>,
    elapsed: Duration,
    /// The envelope view and raw JSON lines.
    prepared: Prepared,
    /// The `kit` line, from the request as run and the session it ran against.
    line: ResultLine,
    /// The MCP `execute` call.
    mcp: String,
}

/// What a click asks for, applied after the frame is drawn.
enum Action {
    Submit(Slot, Request),
    Inspect(String),
    Diagnose(String),
    /// `refs` on a form: outgoing links, or its referrers ("Use info") when `true`.
    Links(String, bool),
    LinksPage(Option<String>),
    Planet(String),
    PlanetPage(Option<String>),
    PickType(String),
    QueryPage(Option<String>),
    UseProfile(PathBuf),
    /// A start-screen button: load this profile and remember the choice.
    LoadProfile(StartProfile),
}

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
    start_instance: String,
    start_view: Option<StartView>,

    /// The checkout whose docs the Help tab shows, and how it was found.
    checkout: Option<(PathBuf, &'static str)>,
    checkout_field: String,
    help_docs: Vec<HelpDoc>,

    load_form: LoadForm,
    profiles_dir: String,
    profiles: Vec<ProfileEntry>,
    load_view: Option<LoadView>,

    type_rows: Vec<TypeRow>,
    type_filter: String,
    coverage_plugin: String,
    query_form: QueryForm,
    query_view: Option<QueryView>,
    query_cursor: Option<String>,
    record_form: String,
    record_plugin: String,
    record_view: Option<RecordView>,

    links_form: LinksForm,
    links_view: Option<LinksView>,
    links_cursor: Option<String>,

    planet_form: PlanetForm,
    planet_view: Option<PlanetView>,
    planet_cursor: Option<String>,

    diff_form: DiffForm,
    diff_view: Option<DiffView>,

    diag_form: String,
    diag_plugin: String,
    diag_validate_plugin: String,
    diag_view: Option<DiagView>,

    validate_form: ValidateForm,
    validate_view: Option<ValidateView>,
    roundtrip_view: Option<RoundtripView>,

    schema_form: SchemaForm,
    schema_view: Option<SchemaView>,
}

/// `Slot` as a map key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SlotKey(u8);

impl From<Slot> for SlotKey {
    fn from(s: Slot) -> Self {
        SlotKey(s as u8)
    }
}

/// Opens the window and blocks until it closes.
pub(crate) fn run_window(opts: LaunchOptions) -> Result<(), UiError> {
    let fonts = fonts()?;
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KitKraft Workbench (read-only)")
            .with_inner_size([1400.0, 900.0]),
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
            Ok(Box::new(App::new(worker, opts)))
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
        let mut app = Self {
            worker,
            tab: if first_run { Tab::Start } else { Tab::Load },
            slots: BTreeMap::new(),
            notice: None,
            results: BTreeMap::new(),
            session: None,
            prefs,
            prefs_path,
            start_instance,
            start_view: None,
            checkout: None,
            checkout_field: String::new(),
            help_docs: Vec::new(),
            profiles: if profiles_dir.is_empty() {
                Vec::new()
            } else {
                list_profiles(Path::new(&profiles_dir))
            },
            load_form,
            profiles_dir,
            load_view: None,
            type_rows: Vec::new(),
            type_filter: String::new(),
            coverage_plugin: String::new(),
            query_form: QueryForm::default(),
            query_view: None,
            query_cursor: None,
            record_form: String::new(),
            record_plugin: String::new(),
            record_view: None,
            links_form: LinksForm::default(),
            links_view: None,
            links_cursor: None,
            planet_form: PlanetForm::default(),
            planet_view: None,
            planet_cursor: None,
            diff_form: DiffForm::default(),
            diff_view: None,
            diag_form: String::new(),
            diag_plugin: String::new(),
            diag_validate_plugin: String::new(),
            diag_view: None,
            validate_form: ValidateForm::default(),
            validate_view: None,
            roundtrip_view: None,
            schema_form: SchemaForm::default(),
            schema_view: None,
            tour: None,
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
        let req = profiles_request(Some(&app.start_instance));
        app.submit(Slot::Profiles, req);
        if opts.profile.is_some() || !opts.plugins.is_empty() {
            if let Ok(req) = app.load_form.request() {
                app.submit(Slot::Load, req);
            }
        }
        app.reload_help();
        if shots_refused.is_some() {
            app.notice = shots_refused;
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
                    if self.start_instance.trim().is_empty() {
                        self.start_instance = view.instance.clone().unwrap_or_default();
                    }
                    self.start_view = Some(view);
                }
                Slot::Load => {
                    self.load_view = Some(LoadView::from_envelope(env));
                    self.session = Some(session_args(&reply.request.args));
                }
                Slot::Types => {
                    self.type_rows = if reply.request.id == "coverage" {
                        types_from_coverage(env)
                    } else {
                        types_from_schema(env)
                    };
                }
                Slot::Query => self.query_view = Some(QueryView::from_envelope(env)),
                Slot::Inspect => {
                    let view = RecordView::from_envelope(env);
                    if !view.signature.is_empty() {
                        let req = schema_request(&view.signature);
                        self.submit(Slot::InspectSchema, req);
                    }
                    self.record_view = Some(view);
                }
                Slot::InspectSchema => {
                    if let Some(view) = &mut self.record_view {
                        view.join_schema(env);
                    }
                }
                Slot::Links => self.links_view = Some(LinksView::from_envelope(env)),
                Slot::Planet => self.planet_view = Some(PlanetView::from_envelope(env)),
                Slot::Diff => self.diff_view = Some(DiffView::from_envelope(env)),
                Slot::DiagInspect => {
                    let view = DiagView::from_inspect(env, self.load_view.as_ref());
                    if self.diag_validate_plugin.trim().is_empty() {
                        self.diag_validate_plugin = self
                            .load_view
                            .as_ref()
                            .map_or_else(|| view.winner.clone(), |l| l.path_of(&view.winner));
                    }
                    self.diag_view = Some(view);
                }
                Slot::DiagValidate => {
                    if let Some(view) = &mut self.diag_view {
                        view.with_validate(env);
                    }
                }
                Slot::Validate => self.validate_view = Some(ValidateView::from_envelope(env)),
                Slot::Roundtrip => self.roundtrip_view = Some(RoundtripView::from_envelope(env)),
                Slot::Schema => self.schema_view = Some(SchemaView::from_envelope(env)),
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
                self.load_view = None;
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
                    self.record_form.clone_from(&form);
                    if let Ok(req) = inspect_request(&form, None) {
                        self.submit(Slot::Inspect, req);
                    }
                    self.tab = Tab::Records;
                }
                Action::Diagnose(form) => {
                    self.diag_form.clone_from(&form);
                    if let Ok(req) = inspect_request(&form, None) {
                        self.submit(Slot::DiagInspect, req);
                    }
                    self.tab = Tab::Diagnostics;
                }
                Action::Links(form, incoming) => {
                    self.links_form = if incoming {
                        LinksForm::use_info(&form)
                    } else {
                        LinksForm::outgoing(&form)
                    };
                    self.apply(vec![Action::LinksPage(None)]);
                    self.tab = Tab::Links;
                }
                Action::LinksPage(cursor) => {
                    self.links_cursor.clone_from(&cursor);
                    match self.links_form.request(cursor.as_deref()) {
                        Ok(req) => self.submit(Slot::Links, req),
                        Err(e) => self.notice = Some(e),
                    }
                }
                Action::Planet(form) => {
                    self.planet_form.form = form;
                    self.apply(vec![Action::PlanetPage(None)]);
                    self.tab = Tab::Planet;
                }
                Action::PlanetPage(cursor) => {
                    self.planet_cursor.clone_from(&cursor);
                    match self.planet_form.request(cursor.as_deref()) {
                        Ok(req) => self.submit(Slot::Planet, req),
                        Err(e) => self.notice = Some(e),
                    }
                }
                Action::PickType(sig) => {
                    self.query_form.record_type = sig;
                    self.query_cursor = None;
                    if let Ok(req) = self.query_form.request(None) {
                        self.submit(Slot::Query, req);
                    }
                }
                Action::QueryPage(cursor) => {
                    self.query_cursor.clone_from(&cursor);
                    match self.query_form.request(cursor.as_deref()) {
                        Ok(req) => self.submit(Slot::Query, req),
                        Err(e) => self.notice = Some(e),
                    }
                }
                Action::UseProfile(path) => {
                    self.load_form.profile = path.display().to_string();
                }
                Action::LoadProfile(profile) => match profile.load_request() {
                    Ok(req) => {
                        self.load_form = LoadForm {
                            profile: profile.path.clone(),
                            ..LoadForm::default()
                        };
                        self.submit(Slot::Load, req);
                        self.remember(&profile);
                        self.tab = Tab::Load;
                    }
                    Err(e) => self.notice = Some(e),
                },
            }
        }
    }

    /// Writes the start screen's choice to the prefs file (the one file the workbench
    /// writes on its own; `crate::prefs`).
    fn remember(&mut self, profile: &StartProfile) {
        self.prefs.profile = Some(profile.path.clone());
        self.prefs.instance = self
            .start_view
            .as_ref()
            .and_then(|v| v.instance.clone())
            .or_else(|| instance_of_profile(&profile.path));
        self.save_prefs();
    }

    /// Finds the checkout again and rereads the Help tab's docs.
    fn reload_help(&mut self) {
        self.checkout = help::find_checkout_here(self.prefs.repo_dir.as_deref());
        match &self.checkout {
            Some((root, _)) => {
                self.checkout_field = root.display().to_string();
                self.help_docs = help::load(root);
            }
            None => self.help_docs.clear(),
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
        self.load_view
            .as_ref()
            .map(LoadView::files)
            .unwrap_or_default()
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        for reply in self.worker.poll() {
            self.receive(reply);
        }
        let busy = self.worker.is_busy();
        if busy {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
        egui::Panel::top("kitkraft-tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("KitKraft Workbench");
                ui.weak("read-only: every view is one engine command");
                ui.separator();
                for (tab, label) in TABS {
                    ui.selectable_value(&mut self.tab, *tab, *label);
                }
                ui.separator();
                let mut drop_pending = false;
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
                if drop_pending {
                    self.drop_pending();
                }
                match &self.load_view {
                    Some(l) => ui.weak(format!(
                        "session: {} ({} plugins)",
                        l.source,
                        l.plugins.len()
                    )),
                    None => ui.weak("session: nothing loaded"),
                };
            });
            if let Some(n) = &self.notice {
                ui.colored_label(BAD, n);
            }
        });
        let mut actions = Vec::new();
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("tab-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| match self.tab {
                    Tab::Start => self.tab_start(ui, busy, &mut actions),
                    Tab::Load => self.tab_load(ui, busy, &mut actions),
                    Tab::Records => self.tab_records(ui, busy, &mut actions),
                    Tab::Links => self.tab_links(ui, busy, &mut actions),
                    Tab::Planet => self.tab_planet(ui, busy, &mut actions),
                    Tab::Diff => self.tab_diff(ui, busy, &mut actions),
                    Tab::Diagnostics => self.tab_diag(ui, busy, &mut actions),
                    Tab::Validate => self.tab_validate(ui, busy, &mut actions),
                    Tab::Schema => self.tab_schema(ui, busy, &mut actions),
                    Tab::Help => self.tab_help(ui),
                });
        });
        self.apply(actions);
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
            TourStep::WaitLoad if !busy && self.load_view.is_some() => {
                self.query_form.record_type = "WEAP".into();
                self.query_form.where_text = "FULL ~ 'Rifle'".into();
                self.query_form.limit = 20;
                self.apply(vec![Action::QueryPage(None)]);
                self.set_step(TourStep::Seeded(1));
            }
            TourStep::Seeded(1) if !busy => {
                let form = self
                    .query_view
                    .as_ref()
                    .and_then(|q| q.rows.first())
                    .map(|r| r.form.clone());
                // The biggest plugin the profile itself activates.
                let plugin = self.load_view.as_ref().and_then(|l| {
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
                self.planet_form.limit = 60;
                actions.push(Action::Planet("PCM_QuestRequest".into()));
                if let Some((name, path)) = plugin {
                    self.coverage_plugin.clone_from(&path);
                    actions.push(Action::Submit(Slot::Types, coverage_request(&path)));
                    self.diff_form.mode = DiffMode::VsMasters;
                    self.diff_form.a = name;
                    if let Ok(req) = self.diff_form.request(None) {
                        actions.push(Action::Submit(Slot::Diff, req));
                    }
                    self.validate_form.plugin = path;
                    if let Ok(req) = self.validate_form.request() {
                        actions.push(Action::Submit(Slot::Validate, req));
                    }
                    if let Ok(req) = self.validate_form.roundtrip_request() {
                        actions.push(Action::Submit(Slot::Roundtrip, req));
                    }
                }
                self.schema_form.record_type = "WEAP".into();
                actions.push(Action::Submit(Slot::Schema, self.schema_form.request()));
                self.apply(actions);
                self.set_step(TourStep::Seeded(2));
            }
            TourStep::Seeded(2) if !busy => {
                if let Ok(req) = diag::validate_request(&self.diag_validate_plugin) {
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
            let path = tour.dir.join(format!("{}.bmp", label.to_ascii_lowercase()));
            match crate::shots::bmp(w, h, &rgba).map(|b| std::fs::write(&path, b)) {
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

// ---- tabs -----------------------------------------------------------------------------

impl App {
    fn tab_start(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("Start");
        ui.label("The MO2 instance, its profiles and the Data folder, as the engine's `profiles` command reads them. Each button loads a profile read-only; `Default` is the owner's live game and is only ever read. In-game checks run on `KitKraft Dev`.");
        ui.horizontal(|ui| {
            ui.label("MO2 instance");
            ui.add(TextEdit::singleline(&mut self.start_instance).desired_width(420.0))
                .on_hover_text(
                    "the folder holding ModOrganizer.ini; empty: the engine looks for one",
                );
            if ui
                .add_enabled(!busy, egui::Button::new("Look again"))
                .clicked()
            {
                actions.push(Action::Submit(
                    Slot::Profiles,
                    profiles_request(Some(&self.start_instance)),
                ));
            }
        });
        let last = self.prefs.profile.clone();
        envelope_block(ui, self.results.get(&Slot::Profiles.into()), |ui| {
            let Some(view) = &self.start_view else { return };
            Grid::new("start-instance").num_columns(2).show(ui, |ui| {
                ui.strong("MO2 instance");
                match &view.instance {
                    Some(root) => ui.label(format!(
                        "{root}  (found by {}; game {}; MO2's selected profile {})",
                        view.found_by,
                        if view.game_name.is_empty() {
                            "not set"
                        } else {
                            &view.game_name
                        },
                        if view.selected_profile.is_empty() {
                            "not set"
                        } else {
                            &view.selected_profile
                        }
                    )),
                    None => ui.colored_label(
                        BAD,
                        "none found: type the folder holding ModOrganizer.ini above",
                    ),
                };
                ui.end_row();
                ui.strong("Data dir");
                if view.data_ok {
                    ui.label(&view.data_dir);
                } else {
                    ui.colored_label(WARN, &view.data_dir);
                }
                ui.end_row();
            });
            CollapsingHeader::new(format!("places searched ({})", view.searched.len()))
                .id_salt("start-searched")
                .show(ui, |ui| {
                    for (path, source, found) in &view.searched {
                        ui.label(format!(
                            "{} {path} ({source})",
                            if *found { "found:" } else { "no instance:" }
                        ));
                    }
                });
            ui.add_space(6.0);
            if view.profiles.is_empty() && view.instance.is_some() {
                ui.colored_label(
                    WARN,
                    "the instance has no profiles (folders with a modlist.txt)",
                );
            }
            Grid::new("start-profiles").striped(true).show(ui, |ui| {
                for p in &view.profiles {
                    let label = if p.live {
                        format!("Load {} (read-only)", p.name)
                    } else {
                        format!("Load {}", p.name)
                    };
                    if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                        actions.push(Action::LoadProfile(p.clone()));
                    }
                    match p.badge() {
                        Some(b) if p.live => {
                            ui.colored_label(WARN, b);
                        }
                        Some(b) => {
                            ui.colored_label(GOOD, b);
                        }
                        None => {
                            ui.label("");
                        }
                    }
                    let mut notes = Vec::new();
                    if p.selected {
                        notes.push("MO2's selected profile");
                    }
                    if StartView::is_last(p, last.as_deref()) {
                        notes.push("last used");
                    }
                    if !p.plugins_txt {
                        notes.push("no plugins.txt");
                    }
                    ui.weak(notes.join("; "));
                    ui.monospace(&p.path);
                    ui.end_row();
                }
            });
            match &self.prefs_path {
                Some(path) => ui.weak(format!(
                    "the choice is remembered in {} (UI prefs, not game data; the only file the workbench writes on its own)",
                    path.display()
                )),
                None => ui.weak("no %LOCALAPPDATA%: the choice is not remembered"),
            };
        });
    }

    fn tab_load(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("Load");
        ui.label("Loads a profile or plugin list into the session (`load`). Profiles are read, never changed; `Default` is the owner's live game and is only ever read.");
        ui.horizontal(|ui| {
            ui.label("MO2 profiles folder");
            ui.add(TextEdit::singleline(&mut self.profiles_dir).desired_width(420.0));
            if ui.button("List").clicked() {
                self.profiles = list_profiles(Path::new(self.profiles_dir.trim()));
            }
        });
        for p in &self.profiles {
            ui.horizontal(|ui| {
                if ui.link(&p.name).clicked() {
                    actions.push(Action::UseProfile(p.path.clone()));
                }
                if p.live {
                    ui.colored_label(WARN, "owner's live game: loading it only reads it");
                }
            });
        }
        Grid::new("load-form").num_columns(2).show(ui, |ui| {
            ui.label("Profile");
            ui.add(TextEdit::singleline(&mut self.load_form.profile).desired_width(520.0));
            ui.end_row();
            ui.label("or plugins (one per line)");
            ui.add(
                TextEdit::multiline(&mut self.load_form.plugins)
                    .desired_rows(3)
                    .desired_width(520.0),
            );
            ui.end_row();
            ui.label("Data dir (optional)");
            ui.add(TextEdit::singleline(&mut self.load_form.data_dir).desired_width(520.0));
            ui.end_row();
        });
        ui.checkbox(
            &mut self.load_form.no_cache,
            "no cache (index every plugin from its file)",
        );
        if ui.add_enabled(!busy, egui::Button::new("Load")).clicked() {
            match self.load_form.request() {
                Ok(req) => actions.push(Action::Submit(Slot::Load, req)),
                Err(e) => self.notice = Some(e),
            }
        }
        envelope_block(ui, self.results.get(&Slot::Load.into()), |ui| {
            let Some(view) = &self.load_view else { return };
            for (label, list) in [
                ("Inactive", &view.inactive),
                ("Active but missing", &view.missing_active),
            ] {
                if !list.is_empty() {
                    ui.colored_label(WARN, format!("{label}: {}", list.join(", ")));
                }
            }
            ScrollArea::vertical()
                .id_salt("load-rows")
                .max_height(420.0)
                .show(ui, |ui| {
                    Grid::new("load-grid").striped(true).show(ui, |ui| {
                        for h in [
                            "#", "plugin", "scale", "flags", "records", "slot", "active",
                            "provider", "cache", "problems",
                        ] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for p in &view.plugins {
                            ui.label(p.index.to_string());
                            ui.label(&p.name);
                            if p.scale == "unknown" {
                                ui.colored_label(WARN, "unknown");
                            } else {
                                ui.label(&p.scale);
                            }
                            ui.label(&p.flags);
                            ui.label(p.records.map_or_else(|| "?".to_string(), |n| n.to_string()));
                            ui.label(&p.runtime_slot);
                            ui.label(&p.active);
                            ui.label(&p.provider);
                            ui.label(&p.cache);
                            let mut problems = Vec::new();
                            if !p.missing_masters.is_empty() {
                                problems.push(format!(
                                    "missing masters: {}",
                                    p.missing_masters.join(", ")
                                ));
                            }
                            if !p.late_masters.is_empty() {
                                problems
                                    .push(format!("late masters: {}", p.late_masters.join(", ")));
                            }
                            if let Some(e) = &p.error {
                                problems.push(e.clone());
                            }
                            if problems.is_empty() {
                                ui.label("");
                            } else {
                                ui.colored_label(BAD, problems.join("; "));
                            }
                            ui.end_row();
                        }
                    });
                });
        });
    }

    fn tab_records(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("Records");
        let plugins = self.loaded_plugins();
        CollapsingHeader::new("Record types")
            .id_salt("types")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!busy, egui::Button::new("Defined types (schema)"))
                        .clicked()
                    {
                        actions.push(Action::Submit(
                            Slot::Types,
                            Request::new("schema", Value::Object(Map::new())),
                        ));
                    }
                    ui.label("or types in");
                    plugin_combo(ui, "cov-plugin", &mut self.coverage_plugin, &plugins, false);
                    if ui
                        .add_enabled(
                            !busy && !self.coverage_plugin.is_empty(),
                            egui::Button::new("Count (coverage)"),
                        )
                        .clicked()
                    {
                        actions.push(Action::Submit(
                            Slot::Types,
                            coverage_request(&self.coverage_plugin),
                        ));
                    }
                    ui.label("filter");
                    ui.add(TextEdit::singleline(&mut self.type_filter).desired_width(80.0));
                });
                envelope_block(ui, self.results.get(&Slot::Types.into()), |ui| {
                    let filter = self.type_filter.trim().to_ascii_uppercase();
                    ScrollArea::vertical()
                        .id_salt("types-rows")
                        .max_height(220.0)
                        .show(ui, |ui| {
                            Grid::new("types-grid").striped(true).show(ui, |ui| {
                                for h in
                                    ["type", "definition", "records", "confidence / occurrences"]
                                {
                                    ui.strong(h);
                                }
                                ui.end_row();
                                for t in self
                                    .type_rows
                                    .iter()
                                    .filter(|t| filter.is_empty() || t.signature.contains(&filter))
                                {
                                    if link(ui, !busy, &t.signature)
                                        .on_hover_text("query this type")
                                        .clicked()
                                    {
                                        actions.push(Action::PickType(t.signature.clone()));
                                    }
                                    match &t.definition {
                                        Some(d) => ui.label(d),
                                        None => ui.colored_label(
                                            WARN,
                                            "no definition (subrecords unknown)",
                                        ),
                                    };
                                    ui.label(t.records.map_or_else(String::new, |n| n.to_string()));
                                    ui.label(
                                        t.occurrences
                                            .clone()
                                            .or_else(|| t.confidence.clone())
                                            .unwrap_or_default(),
                                    );
                                    ui.end_row();
                                }
                            });
                        });
                });
            });
        CollapsingHeader::new("Query")
            .id_salt("query")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("type");
                    ui.add(
                        TextEdit::singleline(&mut self.query_form.record_type).desired_width(50.0),
                    );
                    ui.label("where");
                    ui.add(
                        TextEdit::singleline(&mut self.query_form.where_text)
                            .desired_width(420.0)
                            .hint_text("FULL ~ \"Rifle\" and not has KWDA"),
                    );
                    ui.checkbox(&mut self.query_form.all_versions, "all versions");
                    if ui.add_enabled(!busy, egui::Button::new("Run")).clicked() {
                        actions.push(Action::QueryPage(None));
                    }
                });
                envelope_block(ui, self.results.get(&Slot::Query.into()), |ui| {
                    let Some(view) = &self.query_view else { return };
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} matched, {} undecided on this page",
                            view.matched, view.unknown
                        ));
                        if self.query_cursor.is_some()
                            && ui
                                .add_enabled(!busy, egui::Button::new("First page"))
                                .clicked()
                        {
                            actions.push(Action::QueryPage(None));
                        }
                        if let Some(next) = &view.next_cursor {
                            if ui
                                .add_enabled(!busy, egui::Button::new("Next page"))
                                .clicked()
                            {
                                actions.push(Action::QueryPage(Some(next.clone())));
                            }
                        }
                    });
                    for (form, reasons) in &view.unknown_records {
                        ui.colored_label(WARN, format!("undecided {form}: {reasons}"));
                    }
                    ScrollArea::vertical()
                        .id_salt("query-rows")
                        .max_height(300.0)
                        .show(ui, |ui| {
                            Grid::new("query-grid").striped(true).show(ui, |ui| {
                                for h in [
                                    "form",
                                    "",
                                    "editor id",
                                    "name",
                                    "plugin",
                                    "raw FormID",
                                    "runtime ID",
                                    "",
                                ] {
                                    ui.strong(h);
                                }
                                ui.end_row();
                                for r in &view.rows {
                                    if link(ui, !busy, &r.form).on_hover_text("inspect").clicked() {
                                        actions.push(Action::Inspect(r.form.clone()));
                                    }
                                    record_actions(ui, busy, &r.form, actions);
                                    ui.label(&r.editor_id);
                                    ui.label(&r.full);
                                    ui.label(&r.plugin);
                                    ui.monospace(&r.raw_form_id);
                                    ui.monospace(&r.runtime_id);
                                    let mut tags = Vec::new();
                                    if r.winner {
                                        tags.push("winner");
                                    }
                                    if r.deleted {
                                        tags.push("deleted");
                                    }
                                    ui.label(tags.join(", "));
                                    ui.end_row();
                                }
                            });
                        });
                });
            });
        CollapsingHeader::new("Record")
            .id_salt("record")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("form");
                    ui.add(
                        TextEdit::singleline(&mut self.record_form)
                            .desired_width(260.0)
                            .hint_text("Plugin.esm:XXXXXX, EditorID, runtime:XXXXXXXX"),
                    );
                    ui.label("plugin (hex FormID numbering)");
                    ui.add(TextEdit::singleline(&mut self.record_plugin).desired_width(180.0));
                    if ui
                        .add_enabled(!busy, egui::Button::new("Inspect"))
                        .clicked()
                    {
                        match inspect_request(&self.record_form, Some(&self.record_plugin)) {
                            Ok(req) => actions.push(Action::Submit(Slot::Inspect, req)),
                            Err(e) => self.notice = Some(e),
                        }
                    }
                });
                envelope_block(ui, self.results.get(&Slot::Inspect.into()), |ui| {
                    let Some(view) = &self.record_view else {
                        return;
                    };
                    record_view(ui, view, busy, actions);
                });
                if let Some(shown) = self.results.get(&Slot::InspectSchema.into()) {
                    CollapsingHeader::new("provenance source: schema --deep")
                        .id_salt("record-schema")
                        .show(ui, |ui| envelope_block(ui, Some(shown), |_| {}));
                }
            });
    }

    fn tab_links(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("Links");
        ui.label("A form's typed FormID links (`refs`), or the records that name it (`refs --incoming`, the CK's Use Info). Every row comes from a `link` or `referrer` marker; a status other than resolved or null is shown in red.");
        ui.horizontal(|ui| {
            ui.label("form");
            ui.add(
                TextEdit::singleline(&mut self.links_form.form)
                    .desired_width(260.0)
                    .hint_text("Plugin.esm:XXXXXX, EditorID, runtime:XXXXXXXX"),
            );
            ui.radio_value(&mut self.links_form.incoming, false, "outgoing links");
            ui.radio_value(&mut self.links_form.incoming, true, "use info (incoming)");
            ui.label("depth");
            ui.add(egui::DragValue::new(&mut self.links_form.depth).range(1..=MAX_DEPTH));
            if self.links_form.incoming {
                ui.checkbox(&mut self.links_form.all_versions, "every version");
            }
            ui.label("types");
            ui.add(
                TextEdit::singleline(&mut self.links_form.types)
                    .desired_width(120.0)
                    .hint_text("WEAP, LVLI"),
            );
            if ui.add_enabled(!busy, egui::Button::new("Run")).clicked() {
                actions.push(Action::LinksPage(None));
            }
        });
        envelope_block(ui, self.results.get(&Slot::Links.into()), |ui| {
            let Some(view) = &self.links_view else { return };
            let incoming = view.incoming();
            ui.horizontal(|ui| {
                ui.strong(format!("{} {}", view.global, view.label));
                record_actions(ui, busy, &view.global, actions);
            });
            ui.label(format!(
                "{} {} in total; {}",
                view.total,
                if incoming { "referrer(s)" } else { "link(s)" },
                view.counts
            ));
            if view.truncated {
                ui.colored_label(WARN, "the walk stopped at its cap: lower the depth");
            }
            if view.unrecognized > 0 {
                ui.colored_label(
                    WARN,
                    format!(
                        "{} edge(s) with a marker this view does not know (raw JSON below)",
                        view.unrecognized
                    ),
                );
            }
            if let Some(n) = &view.not_searched {
                ui.weak(n);
            }
            ui.horizontal(|ui| {
                if self.links_cursor.is_some()
                    && ui
                        .add_enabled(!busy, egui::Button::new("First page"))
                        .clicked()
                {
                    actions.push(Action::LinksPage(None));
                }
                if let Some(next) = &view.next_cursor {
                    if ui
                        .add_enabled(!busy, egui::Button::new("Next page"))
                        .clicked()
                    {
                        actions.push(Action::LinksPage(Some(next.clone())));
                    }
                }
            });
            if !view.placed_by_cell.is_empty() {
                CollapsingHeader::new(format!(
                    "placed referrers by cell ({})",
                    view.placed_by_cell.len()
                ))
                .id_salt("links-cells")
                .show(ui, |ui| {
                    for c in &view.placed_by_cell {
                        ui.monospace(c);
                    }
                });
            }
            ScrollArea::vertical()
                .id_salt("links-rows")
                .max_height(420.0)
                .show(ui, |ui| {
                    Grid::new("links-grid").striped(true).show(ui, |ui| {
                        let heads: &[&str] = if incoming {
                            &[
                                "hop", "referrer", "", "path", "raw", "status", "plugin", "placed",
                            ]
                        } else {
                            &["hop", "from", "path", "raw", "status", "to", "", "plugin"]
                        };
                        for h in heads {
                            ui.strong(*h);
                        }
                        ui.end_row();
                        for e in &view.edges {
                            ui.label(e.depth.to_string());
                            if incoming {
                                form_cell(ui, busy, e.from.as_deref(), &e.from_label, actions);
                                match e.from.as_deref() {
                                    Some(f) => record_actions(ui, busy, f, actions),
                                    None => {
                                        ui.label("");
                                    }
                                }
                            } else {
                                form_cell(ui, busy, e.from.as_deref(), &e.from_label, actions);
                            }
                            ui.monospace(&e.path);
                            ui.monospace(&e.raw);
                            if e.problem() {
                                ui.colored_label(BAD, &e.status);
                            } else {
                                ui.label(&e.status);
                            }
                            if incoming {
                                ui.label(&e.plugin);
                                ui.label(e.placed.as_deref().unwrap_or(""));
                            } else {
                                if e.problem() {
                                    ui.colored_label(BAD, &e.to_label);
                                } else {
                                    form_cell(ui, busy, e.to.as_deref(), &e.to_label, actions);
                                }
                                match e.to.as_deref() {
                                    Some(f) => record_actions(ui, busy, f, actions),
                                    None => {
                                        ui.label("");
                                    }
                                }
                                if e.target_mismatch {
                                    ui.colored_label(
                                        WARN,
                                        format!("{} (target type not listed)", e.plugin),
                                    );
                                } else {
                                    ui.label(&e.plugin);
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
        });
    }

    fn tab_planet(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("Planet");
        ui.label("The Planet Content Manager chain of one record (`planet`): a tree, branch or content node shows its subtree; a planet, star, biome, location, worldspace or pack-in the content nodes that place it. Conditions are shown as stored and never evaluated.");
        ui.horizontal(|ui| {
            ui.label("form");
            ui.add(
                TextEdit::singleline(&mut self.planet_form.form)
                    .desired_width(260.0)
                    .hint_text("PCM_QuestRequest, Plugin.esm:XXXXXX"),
            );
            ui.label("nodes per page");
            ui.add(egui::DragValue::new(&mut self.planet_form.limit).range(1..=5000));
            if ui.add_enabled(!busy, egui::Button::new("Show")).clicked() {
                actions.push(Action::PlanetPage(None));
            }
        });
        envelope_block(ui, self.results.get(&Slot::Planet.into()), |ui| {
            let Some(view) = &self.planet_view else {
                return;
            };
            ui.horizontal(|ui| {
                ui.strong(format!("{} {}  [{}]", view.global, view.label, view.kind));
                record_actions(ui, busy, &view.global, actions);
            });
            if let Some(tree) = &view.tree {
                ui.horizontal(|ui| {
                    ui.label("tree");
                    if tree != &view.global && link(ui, !busy, tree).clicked() {
                        actions.push(Action::Planet(tree.clone()));
                    } else if tree == &view.global {
                        ui.monospace(tree);
                    }
                });
            }
            if let Some(c) = &view.counts {
                ui.label(c);
            }
            for u in &view.unresolved {
                ui.colored_label(BAD, format!("unresolved: {u}"));
            }
            ui.horizontal(|ui| {
                if self.planet_cursor.is_some()
                    && ui
                        .add_enabled(!busy, egui::Button::new("First page"))
                        .clicked()
                {
                    actions.push(Action::PlanetPage(None));
                }
                if let Some(next) = &view.next_cursor {
                    if ui
                        .add_enabled(!busy, egui::Button::new("Next page"))
                        .clicked()
                    {
                        actions.push(Action::PlanetPage(Some(next.clone())));
                    }
                }
            });
            for (title, rows) in [("ancestors", &view.ancestors), ("nodes", &view.nodes)] {
                if rows.is_empty() {
                    continue;
                }
                ui.strong(format!("{title} ({})", rows.len()));
                ScrollArea::vertical()
                    .id_salt(("planet-rows", title))
                    .max_height(380.0)
                    .show(ui, |ui| {
                        Grid::new(("planet-grid", title))
                            .striped(true)
                            .show(ui, |ui| {
                                for h in ["node", "", "order", "selection", "conditions", "content"]
                                {
                                    ui.strong(h);
                                }
                                ui.end_row();
                                for n in rows {
                                    let indent = "    "
                                        .repeat(usize::try_from(n.depth.min(12)).unwrap_or(0));
                                    if link(ui, !busy, format!("{indent}{} {}", n.form, n.label))
                                        .on_hover_text("planet chain of this node")
                                        .clicked()
                                    {
                                        actions.push(Action::Planet(n.form.clone()));
                                    }
                                    record_actions(ui, busy, &n.form, actions);
                                    if n.unreadable {
                                        ui.colored_label(BAD, "not readable");
                                    } else {
                                        ui.label(&n.order);
                                    }
                                    ui.label(&n.selection);
                                    ui.label(n.conditions.to_string());
                                    match &n.content {
                                        Some((text, Some(to))) => {
                                            if link(ui, !busy, text)
                                                .on_hover_text("inspect the content")
                                                .clicked()
                                            {
                                                actions.push(Action::Inspect(to.clone()));
                                            }
                                        }
                                        Some((text, None)) => {
                                            ui.colored_label(BAD, text);
                                        }
                                        None => {
                                            ui.label("");
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    });
            }
            if !view.placements.is_empty() {
                ui.strong(format!(
                    "placed by {} content node(s)",
                    view.placements.len()
                ));
                Grid::new("planet-placements").striped(true).show(ui, |ui| {
                    for h in ["at", "content node", "tree", "chain (nearest first)"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for p in &view.placements {
                        ui.weak(&p.at);
                        if link(ui, !busy, format!("{} {}", p.node, p.editor_id))
                            .on_hover_text("planet chain of this node")
                            .clicked()
                        {
                            actions.push(Action::Planet(p.node.clone()));
                        }
                        if p.tree.is_empty() {
                            ui.label("");
                        } else if link(ui, !busy, &p.tree).clicked() {
                            actions.push(Action::Planet(p.tree.clone()));
                        }
                        ui.label(p.chain.join(" <- "));
                        ui.end_row();
                    }
                });
            }
            CollapsingHeader::new(format!("every typed link ({})", view.links.len()))
                .id_salt("planet-links")
                .default_open(view.nodes.is_empty() && view.placements.is_empty())
                .show(ui, |ui| {
                    Grid::new("planet-links-grid").striped(true).show(ui, |ui| {
                        for h in ["where", "link", ""] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for l in &view.links {
                            ui.weak(&l.at);
                            match (&l.to, l.problem) {
                                (Some(to), false) => {
                                    if link(ui, !busy, &l.text).clicked() {
                                        actions.push(Action::Inspect(to.clone()));
                                    }
                                    record_actions(ui, busy, to, actions);
                                }
                                _ => {
                                    ui.colored_label(
                                        if l.problem { BAD } else { Color32::GRAY },
                                        &l.text,
                                    );
                                    ui.label("");
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
        });
    }

    fn tab_diff(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("Diff");
        let plugins = self.loaded_plugins();
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.diff_form.mode,
                DiffMode::TwoPlugins,
                "two plugins (diff)",
            );
            ui.radio_value(
                &mut self.diff_form.mode,
                DiffMode::VsMasters,
                "a plugin vs the versions before it (conflicts; needs a load)",
            );
        });
        ui.horizontal(|ui| {
            match self.diff_form.mode {
                DiffMode::TwoPlugins => {
                    ui.label("a");
                    ui.add(TextEdit::singleline(&mut self.diff_form.a).desired_width(300.0));
                    plugin_combo(ui, "diff-a", &mut self.diff_form.a, &plugins, false);
                    ui.label("b");
                    ui.add(TextEdit::singleline(&mut self.diff_form.b).desired_width(300.0));
                    plugin_combo(ui, "diff-b", &mut self.diff_form.b, &plugins, false);
                    ui.label("form");
                }
                DiffMode::VsMasters => {
                    ui.label("plugin");
                    ui.add(TextEdit::singleline(&mut self.diff_form.a).desired_width(300.0));
                    plugin_combo(ui, "diff-p", &mut self.diff_form.a, &plugins, true);
                    ui.checkbox(
                        &mut self.diff_form.only_conflicts,
                        "only forms 2+ plugins override",
                    );
                    ui.label("type");
                }
            }
            ui.add(TextEdit::singleline(&mut self.diff_form.filter).desired_width(140.0));
            if ui
                .add_enabled(!busy, egui::Button::new("Compare"))
                .clicked()
            {
                match self.diff_form.request(None) {
                    Ok(req) => actions.push(Action::Submit(Slot::Diff, req)),
                    Err(e) => self.notice = Some(e),
                }
            }
        });
        envelope_block(ui, self.results.get(&Slot::Diff.into()), |ui| {
            let Some(view) = &self.diff_view else { return };
            ui.label(view.counts.join("; "));
            if !view.added.is_empty() {
                ui.label(format!("added: {}", view.added.join(", ")));
            }
            if !view.removed.is_empty() {
                ui.label(format!("removed: {}", view.removed.join(", ")));
            }
            if let Some(next) = &view.next_cursor {
                if ui
                    .add_enabled(!busy, egui::Button::new("Next page"))
                    .clicked()
                {
                    if let Ok(req) = self.diff_form.request(Some(next)) {
                        actions.push(Action::Submit(Slot::Diff, req));
                    }
                }
            }
            for (i, r) in view.records.iter().enumerate() {
                let title = format!(
                    "{} {}  {}  [{}]{}",
                    r.signature,
                    r.form,
                    r.sides,
                    r.class,
                    if r.flags_differ { "  flags differ" } else { "" }
                );
                CollapsingHeader::new(title)
                    .id_salt(("diff-rec", i))
                    .default_open(i < 5)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if link(ui, !busy, "inspect across the load order").clicked() {
                                actions.push(Action::Inspect(r.form.clone()));
                            }
                            record_actions(ui, busy, &r.form, actions);
                        });
                        Grid::new(("diff-grid", i)).striped(true).show(ui, |ui| {
                            for h in ["subrecord", "change", "path", "before", "after"] {
                                ui.strong(h);
                            }
                            ui.end_row();
                            for f in &r.rows {
                                ui.monospace(&f.subrecord);
                                ui.label(&f.change);
                                ui.monospace(&f.path);
                                ui.label(&f.before);
                                ui.label(&f.after);
                                ui.end_row();
                            }
                        });
                    });
            }
        });
    }

    fn tab_diag(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("FormID and reference diagnostics");
        ui.label("Raw stored FormIDs beside master-aware identities (`inspect`), then `validate`'s master and FormID rules. \"use info\" lists the records that name a form (`refs --incoming`, Links tab).");
        ui.horizontal(|ui| {
            ui.label("form");
            ui.add(
                TextEdit::singleline(&mut self.diag_form)
                    .desired_width(260.0)
                    .hint_text("Plugin.esm:XXXXXX, EditorID, runtime:XXXXXXXX, hex"),
            );
            ui.label("plugin (for a hex FormID)");
            ui.add(TextEdit::singleline(&mut self.diag_plugin).desired_width(180.0));
            if ui
                .add_enabled(!busy, egui::Button::new("Resolve"))
                .clicked()
            {
                self.diag_validate_plugin.clear();
                match inspect_request(&self.diag_form, Some(&self.diag_plugin)) {
                    Ok(req) => actions.push(Action::Submit(Slot::DiagInspect, req)),
                    Err(e) => self.notice = Some(e),
                }
            }
        });
        envelope_block(ui, self.results.get(&Slot::DiagInspect.into()), |ui| {
            let Some(view) = &self.diag_view else { return };
            Grid::new("diag-id").num_columns(2).show(ui, |ui| {
                for (k, v) in [
                    ("identity", &view.global),
                    ("matched by", &view.matched_by),
                    ("defining plugin", &view.defining_plugin),
                    ("object ID", &view.object_id),
                    ("runtime ID", &view.runtime_id),
                    ("winner", &view.winner),
                ] {
                    ui.strong(k);
                    ui.monospace(v);
                    ui.end_row();
                }
            });
            if !view.global.is_empty() {
                record_actions(ui, busy, &view.global, actions);
            }
            ui.label("Versions: the FormID each plugin stores, and how it resolved");
            Grid::new("diag-versions").striped(true).show(ui, |ui| {
                for h in ["plugin", "scale", "raw FormID", "resolved by", ""] {
                    ui.strong(h);
                }
                ui.end_row();
                for r in &view.identities {
                    ui.label(&r.plugin);
                    if r.scale == "unknown" {
                        ui.colored_label(WARN, "unknown");
                    } else {
                        ui.label(&r.scale);
                    }
                    ui.monospace(&r.raw_form_id);
                    if r.resolved_by == "direct" {
                        ui.label(&r.resolved_by);
                    } else {
                        ui.colored_label(WARN, &r.resolved_by);
                    }
                    ui.label(match (r.winner, r.deleted) {
                        (true, true) => "winner, deleted",
                        (true, false) => "winner",
                        (false, true) => "deleted",
                        (false, false) => "",
                    });
                    ui.end_row();
                }
            });
            ui.label(format!(
                "FormIDs the winner stores ({}; typed members only)",
                view.references.len()
            ));
            ScrollArea::vertical()
                .id_salt("diag-refs")
                .max_height(260.0)
                .show(ui, |ui| {
                    Grid::new("diag-refs-grid").striped(true).show(ui, |ui| {
                        for h in ["at", "raw", "resolved", ""] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for r in &view.references {
                            ui.monospace(&r.at);
                            ui.monospace(&r.raw);
                            if r.resolved == "unresolved" {
                                ui.colored_label(BAD, &r.resolved);
                                ui.label("");
                            } else if r.resolved == "NULL" {
                                ui.label(&r.resolved);
                                ui.label("");
                            } else {
                                if link(ui, !busy, &r.resolved).clicked() {
                                    actions.push(Action::Diagnose(r.resolved.clone()));
                                }
                                record_actions(ui, busy, &r.resolved, actions);
                            }
                            ui.end_row();
                        }
                    });
                });
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("check references of plugin");
            ui.add(TextEdit::singleline(&mut self.diag_validate_plugin).desired_width(300.0));
            if ui
                .add_enabled(!busy, egui::Button::new("validate masters + FormIDs"))
                .clicked()
            {
                match diag::validate_request(&self.diag_validate_plugin) {
                    Ok(req) => actions.push(Action::Submit(Slot::DiagValidate, req)),
                    Err(e) => self.notice = Some(e),
                }
            }
        });
        envelope_block(ui, self.results.get(&Slot::DiagValidate.into()), |ui| {
            let Some(view) = &self.diag_view else { return };
            if view.findings.is_empty() && view.validate_summary.is_some() {
                ui.colored_label(GOOD, "no master or FormID finding");
            }
            findings_grid(ui, "diag-findings", view.findings.iter());
        });
    }

    fn tab_validate(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("Validate");
        let plugins = self.loaded_plugins();
        ui.horizontal(|ui| {
            ui.label("plugin");
            ui.add(TextEdit::singleline(&mut self.validate_form.plugin).desired_width(380.0));
            plugin_combo(
                ui,
                "val-plugin",
                &mut self.validate_form.plugin,
                &plugins,
                false,
            );
            ui.label("rules (empty: all)");
            ui.add(TextEdit::singleline(&mut self.validate_form.rules).desired_width(260.0));
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("validate"))
                .clicked()
            {
                match self.validate_form.request() {
                    Ok(req) => actions.push(Action::Submit(Slot::Validate, req)),
                    Err(e) => self.notice = Some(e),
                }
            }
            if ui
                .add_enabled(!busy, egui::Button::new("roundtrip --verify"))
                .clicked()
            {
                match self.validate_form.roundtrip_request() {
                    Ok(req) => actions.push(Action::Submit(Slot::Roundtrip, req)),
                    Err(e) => self.notice = Some(e),
                }
            }
            ui.weak("build --plan is not offered yet (the gate refuses build)");
        });
        envelope_block(ui, self.results.get(&Slot::Validate.into()), |ui| {
            let Some(view) = &self.validate_view else {
                return;
            };
            ui.label(format!(
                "{} error(s), {} warning(s), {} finding(s) in total; {}",
                view.errors, view.warnings, view.findings_total, view.checked_line
            ));
            for g in &view.groups {
                let color = if g.errors > 0 {
                    BAD
                } else if g.warnings > 0 {
                    WARN
                } else {
                    GOOD
                };
                let title = RichText::new(format!(
                    "{}: {} error(s), {} warning(s)",
                    g.rule, g.errors, g.warnings
                ))
                .color(color);
                CollapsingHeader::new(title)
                    .id_salt(("rule", g.rule.as_str()))
                    .default_open(!g.findings.is_empty() && g.findings.len() < 50)
                    .show(ui, |ui| {
                        findings_grid(ui, &g.rule, g.findings.iter());
                    });
            }
        });
        envelope_block(ui, self.results.get(&Slot::Roundtrip.into()), |ui| {
            let Some(view) = &self.roundtrip_view else {
                return;
            };
            ui.label(&view.counts);
            if let Some(m) = &view.mismatch {
                ui.colored_label(BAD, format!("first mismatch {m}"));
            }
        });
    }

    fn tab_help(&mut self, ui: &mut egui::Ui) {
        ui.heading("Help");
        ui.label("The owner's QA material, read from the checkout's own docs when the window opens or Reload is clicked. Click any code to copy it. In-game steps run only on the MO2 profile KitKraft Dev with a disposable save, never on Default.");
        let mut reload = false;
        let mut use_field = false;
        ui.horizontal(|ui| {
            ui.label("Checkout");
            ui.add(TextEdit::singleline(&mut self.checkout_field).desired_width(420.0));
            use_field = ui
                .button("Use this checkout")
                .on_hover_text("remembered in the UI prefs file")
                .clicked();
            reload = ui.button("Reload").clicked();
        });
        match &self.checkout {
            Some((root, how)) => ui.weak(format!("docs from {} (found {how})", root.display())),
            None => ui.colored_label(
                BAD,
                format!(
                    "no KitKraft checkout found (a folder with AGENTS.md and docs): set {} or type it above",
                    help::REPO_ENV
                ),
            ),
        };
        if use_field {
            let dir = self.checkout_field.trim().to_string();
            if help::is_checkout(Path::new(&dir)) {
                self.prefs.repo_dir = Some(dir);
                self.save_prefs();
                reload = true;
            } else {
                self.notice = Some(format!(
                    "{dir}: not a KitKraft checkout (no AGENTS.md and docs there)"
                ));
            }
        }
        if reload {
            self.reload_help();
        }
        for (i, doc) in self.help_docs.iter().enumerate() {
            CollapsingHeader::new(doc.source.title)
                .id_salt(("help", i))
                .default_open(true)
                .show(ui, |ui| {
                    ui.weak(format!(
                        "{} (from the heading \"{}\")",
                        doc.path.display(),
                        doc.source.heading
                    ));
                    match &doc.blocks {
                        Ok(blocks) => md_blocks(ui, i, blocks),
                        Err(e) => {
                            ui.colored_label(BAD, e);
                        }
                    }
                });
        }
    }

    fn tab_schema(&mut self, ui: &mut egui::Ui, busy: bool, actions: &mut Vec<Action>) {
        ui.heading("Schema and provenance");
        ui.horizontal(|ui| {
            ui.label("type");
            ui.add(
                TextEdit::singleline(&mut self.schema_form.record_type)
                    .desired_width(60.0)
                    .hint_text("WEAP"),
            );
            ui.label("member");
            ui.add(TextEdit::singleline(&mut self.schema_form.member).desired_width(140.0));
            ui.checkbox(&mut self.schema_form.deep, "deep");
            if ui.add_enabled(!busy, egui::Button::new("Show")).clicked() {
                actions.push(Action::Submit(Slot::Schema, self.schema_form.request()));
            }
        });
        envelope_block(ui, self.results.get(&Slot::Schema.into()), |ui| {
            let Some(view) = &self.schema_view else {
                return;
            };
            if view.defined == Some(false) {
                ui.colored_label(
                    WARN,
                    "no definition: its subrecords are unknown to the schema (not absent)",
                );
            }
            if !view.name.is_empty() {
                ui.label(format!(
                    "{} [{}]  {}",
                    view.name, view.confidence, view.confidence_counts
                ));
                ui.weak(&view.source);
            }
            if !view.definitions.is_empty() {
                Grid::new("schema-defs").striped(true).show(ui, |ui| {
                    for (ty, name, conf) in &view.definitions {
                        if link(ui, !busy, ty).clicked() {
                            let mut form = self.schema_form.clone();
                            form.record_type.clone_from(ty);
                            form.member.clear();
                            actions.push(Action::Submit(Slot::Schema, form.request()));
                        }
                        ui.label(name);
                        conf_label(ui, conf);
                        ui.end_row();
                    }
                });
            }
            Grid::new("schema-members").striped(true).show(ui, |ui| {
                for h in [
                    "member",
                    "signature",
                    "kind",
                    "card",
                    "type",
                    "confidence",
                    "source",
                ] {
                    ui.strong(h);
                }
                ui.end_row();
                for m in &view.members {
                    let indent = "    ".repeat(m.depth.min(8));
                    let name = format!("{indent}{}", m.name);
                    let resp = ui.label(name);
                    let mut tip = String::new();
                    for e in &m.evidence {
                        tip.push_str(e);
                        tip.push('\n');
                    }
                    tip.push_str(&m.note);
                    if !tip.trim().is_empty() {
                        resp.on_hover_text(tip);
                    }
                    ui.monospace(&m.signature);
                    ui.label(&m.kind);
                    ui.label(&m.card);
                    ui.monospace(&m.ty);
                    conf_label(ui, &m.confidence);
                    ui.weak(&m.source);
                    ui.end_row();
                }
            });
        });
    }
}

// ---- shared pieces --------------------------------------------------------------------

/// A link that submits a request: disabled while the engine is busy, so clicks do not
/// pile up a queue behind a long command (the top bar can drop one).
fn link(ui: &mut egui::Ui, enabled: bool, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add_enabled(enabled, egui::Link::new(text))
}

/// The two `refs` actions every record row offers: "links" (its outgoing typed links) and
/// "use info" (the records that name it).
fn record_actions(ui: &mut egui::Ui, busy: bool, form: &str, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        if link(ui, !busy, "links")
            .on_hover_text("Outgoing links: the typed FormIDs this record stores (refs)")
            .clicked()
        {
            actions.push(Action::Links(form.to_string(), false));
        }
        if link(ui, !busy, "use info")
            .on_hover_text("Use info: the records that name this form (refs --incoming)")
            .clicked()
        {
            actions.push(Action::Links(form.to_string(), true));
        }
    });
}

/// A form cell: a link that inspects `form` (labelled `form label`), or the label alone
/// when the form does not resolve.
fn form_cell(
    ui: &mut egui::Ui,
    busy: bool,
    form: Option<&str>,
    label: &str,
    actions: &mut Vec<Action>,
) {
    match form {
        Some(f) => {
            if link(ui, !busy, format!("{f} {label}"))
                .on_hover_text("inspect")
                .clicked()
            {
                actions.push(Action::Inspect(f.to_string()));
            }
        }
        None => {
            ui.label(label);
        }
    }
}

fn conf_label(ui: &mut egui::Ui, confidence: &str) {
    if confidence == "reference" {
        ui.colored_label(WARN, "reference (unconfirmed)")
            .on_hover_text(
                "documented in the pinned xEdit file; not yet confirmed by the corpus or the CK",
            );
    } else if confidence == "container-reference" {
        ui.colored_label(WARN, "container-reference (unconfirmed)")
            .on_hover_text("a container fact from UESP's Skyrim pages that no official file shows");
    } else {
        ui.label(confidence);
    }
}

/// A picker over the loaded plugins: sets `value` to the plugin's path, or its name when
/// `by_name` (arguments that filter by plugin name, such as `conflicts --plugin`).
fn plugin_combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    plugins: &[(String, String)],
    by_name: bool,
) {
    if plugins.is_empty() {
        return;
    }
    ComboBox::from_id_salt(id)
        .selected_text("loaded...")
        .width(110.0)
        .show_ui(ui, |ui| {
            for (name, path) in plugins {
                let pick = if by_name { name } else { path };
                ui.selectable_value(value, pick.clone(), name);
            }
        });
}

fn findings_grid<'a>(
    ui: &mut egui::Ui,
    id: &str,
    findings: impl Iterator<Item = &'a crate::panels::validate::FindingRow>,
) {
    Grid::new(("findings", id)).striped(true).show(ui, |ui| {
        for h in [
            "severity",
            "rule",
            "record",
            "subrecord",
            "offset",
            "message",
        ] {
            ui.strong(h);
        }
        ui.end_row();
        for f in findings.take(1000) {
            if f.severity == "error" {
                ui.colored_label(BAD, &f.severity);
            } else {
                ui.colored_label(WARN, &f.severity);
            }
            ui.monospace(&f.rule);
            ui.monospace(&f.record);
            ui.monospace(&f.subrecord);
            ui.monospace(
                f.offset
                    .map_or_else(|| "?".to_string(), |o| format!("{o} (0x{o:X})")),
            );
            ui.label(&f.message);
            ui.end_row();
        }
    });
}

fn record_view(ui: &mut egui::Ui, view: &RecordView, busy: bool, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.strong(format!(
            "{} {} {}",
            view.signature, view.global, view.editor_id
        ));
        ui.label(format!(
            "matched by {}; winner {}; runtime ID {}",
            view.matched_by, view.winner, view.runtime_id
        ));
        if !view.global.is_empty() && view.mode == "load_order" {
            if link(ui, !busy, "diagnose").clicked() {
                actions.push(Action::Diagnose(view.global.clone()));
            }
            record_actions(ui, busy, &view.global, actions);
            if PLANET_TYPES.contains(&view.signature.as_str())
                && link(ui, !busy, "planet chain")
                    .on_hover_text("the Planet Content Manager chain of this record (planet)")
                    .clicked()
            {
                actions.push(Action::Planet(view.global.clone()));
            }
        }
    });
    let unconfirmed = view.unconfirmed();
    if !unconfirmed.is_empty() {
        ui.colored_label(
            WARN,
            format!(
                "reference-confidence members (unconfirmed): {}",
                unconfirmed.join(", ")
            ),
        );
    }
    for (i, v) in view.versions.iter().enumerate() {
        let mut title = format!(
            "{} {}  raw {}  {}",
            v.load_index.map_or_else(String::new, |n| format!("[{n}]")),
            v.plugin,
            v.raw_form_id,
            v.class
        );
        if v.winner {
            title.push_str("  WINNER");
        }
        if v.deleted {
            title.push_str("  DELETED");
        }
        if !v.resolved_by.is_empty() && v.resolved_by != "direct" {
            title.push_str(&format!("  ({})", v.resolved_by));
        }
        CollapsingHeader::new(title)
            .id_salt(("version", i))
            .default_open(v.winner || view.versions.len() == 1)
            .show(ui, |ui| {
                if !v.changes.is_empty() {
                    ui.label(format!("vs previous: {}", v.changes.join(", ")));
                }
                ui.weak(format!("record flags {}", v.flags));
                Grid::new(("subs", i)).striped(true).show(ui, |ui| {
                    for h in [
                        "sig",
                        "member",
                        "value",
                        "offset",
                        "size",
                        "confidence",
                        "source",
                    ] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for s in &v.subrecords {
                        ui.monospace(&s.signature);
                        ui.label(&s.member);
                        ui.label(&s.value);
                        ui.monospace(s.offset.map_or_else(|| "?".to_string(), |o| o.to_string()));
                        ui.monospace(s.size.map_or_else(|| "?".to_string(), |n| n.to_string()));
                        match &s.confidence {
                            Some(c) => conf_label(ui, c),
                            None => {
                                ui.weak("not named by the schema");
                            }
                        }
                        ui.weak(s.source.as_deref().unwrap_or(""));
                        ui.end_row();
                    }
                });
            });
    }
}

/// The common frame around one result: the command line and MCP call that produce it
/// (copyable), errors, summary, pass/fail, coverage, limitations (always shown), the
/// panel's own `content`, the evidence table and the raw `data`. Everything it shows was
/// built once when the reply arrived ([`Shown`]); per frame it only lays out what is on
/// screen.
fn envelope_block(ui: &mut egui::Ui, shown: Option<&Shown>, content: impl FnOnce(&mut egui::Ui)) {
    let Some(shown) = shown else {
        return;
    };
    ui.separator();
    let id = shown.request.id.as_str();
    match &shown.line.line {
        Some(line) => {
            ui.horizontal(|ui| {
                ui.weak(format!("{} ms", shown.elapsed.as_millis()));
                if ui.small_button("copy").clicked() {
                    ui.ctx().copy_text(line.text.clone());
                }
                ui.monospace(&line.text);
            });
            if !line.exact {
                ui.colored_label(
                    WARN,
                    "a value holds both quote characters: the line above is not exact; use the MCP form",
                );
            }
            if shown.line.session_added {
                ui.weak("load-order arguments added from the session's load: each `kit` process starts with an empty session (this window keeps one, like `kit mcp`)");
            }
        }
        None => {
            ui.horizontal(|ui| {
                ui.weak(format!("{} ms", shown.elapsed.as_millis()));
                ui.weak("no `kit` line: no load had succeeded in this session, so a fresh `kit` process would not see what this ran against; the MCP call below is exact");
            });
        }
    }
    ui.horizontal(|ui| {
        if ui.small_button("copy").clicked() {
            ui.ctx().copy_text(shown.mcp.clone());
        }
        ui.weak("MCP execute:");
        ui.monospace(&shown.mcp);
    });
    if let Some(err) = &shown.error {
        ui.colored_label(BAD, err.to_string());
        raw_lines(ui, id, "error JSON", &shown.prepared, true);
        return;
    }
    let Some(view) = &shown.prepared.view else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        match view.passed {
            Some(true) => {
                ui.colored_label(GOOD, "PASSED");
            }
            Some(false) => {
                ui.colored_label(BAD, "FAILED");
            }
            None => {}
        }
        if let Some(s) = &view.summary {
            // A multi-line summary is a text table (`schema`): keep its columns.
            if s.contains('\n') {
                ui.monospace(s);
            } else {
                ui.label(s);
            }
        }
    });
    ui.label(format!("coverage {}", view.coverage_line()));
    if view.limitations.is_empty() {
        ui.weak("limitations: none claimed");
    } else {
        ui.colored_label(WARN, format!("limitations ({})", view.limitations.len()));
        // Every limitation stays on screen; a long one shows its first line's worth and
        // the rest on hover.
        for l in &view.limitations {
            if l.chars().count() > LIMITATION_CHARS {
                let head: String = l.chars().take(LIMITATION_CHARS).collect();
                ui.colored_label(WARN, format!("  - {head}... (hover for all)"))
                    .on_hover_text(l);
            } else {
                ui.colored_label(WARN, format!("  - {l}"));
            }
        }
    }
    content(ui);
    CollapsingHeader::new(format!("evidence ({} ranges)", view.evidence.len()))
        .id_salt(("evidence", id))
        .show(ui, |ui| {
            ScrollArea::vertical().id_salt(("evidence-rows", id)).max_height(260.0).show(ui, |ui| {
                Grid::new(("evidence-grid", id)).striped(true).show(ui, |ui| {
                    for h in ["file", "offset", "length", "record", "subrecord"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for e in view.evidence.iter().take(EVIDENCE_ROWS) {
                        ui.label(&e.file);
                        ui.monospace(format!("{} (0x{:X})", e.offset, e.offset));
                        ui.monospace(e.length.to_string());
                        ui.monospace(&e.record);
                        ui.monospace(&e.subrecord);
                        ui.end_row();
                    }
                });
                if view.evidence.len() > EVIDENCE_ROWS {
                    ui.weak(format!("first {EVIDENCE_ROWS} shown; the raw JSON (MCP or `kit --json`) has all"));
                }
            });
        });
    raw_lines(ui, id, "raw data (JSON)", &shown.prepared, false);
}

/// Code in the Help tab: monospace, copied on click.
fn copy_code(ui: &mut egui::Ui, code: &str) {
    let r = ui
        .add(
            egui::Label::new(RichText::new(code).monospace().color(CODE))
                .sense(egui::Sense::click()),
        )
        .on_hover_text("click to copy");
    if r.clicked() {
        ui.ctx().copy_text(code.to_string());
    }
}

/// Inline runs, laid out in a wrapping row.
fn md_inlines(ui: &mut egui::Ui, runs: &[Inline], strong: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for r in runs {
            match r {
                Inline::Text(t) if strong => {
                    ui.label(RichText::new(t).strong());
                }
                Inline::Text(t) => {
                    ui.label(t);
                }
                Inline::Strong(t) => {
                    ui.label(RichText::new(t).strong());
                }
                Inline::Struck(t) => {
                    ui.label(RichText::new(t).strikethrough().weak());
                }
                Inline::Code(c) => copy_code(ui, c),
            }
        }
    });
}

/// One table cell, wrapped at `width`.
fn md_cell(ui: &mut egui::Ui, runs: &[Inline], width: f32, strong: bool) {
    ui.vertical(|ui| {
        ui.set_max_width(width);
        md_inlines(ui, runs, strong);
    });
}

/// A parsed doc section (`crate::md`), drawn plainly.
fn md_blocks(ui: &mut egui::Ui, doc: usize, blocks: &[Block]) {
    for (i, b) in blocks.iter().enumerate() {
        match b {
            Block::Heading { level, text } => {
                let t = crate::md::plain(text);
                ui.add_space(4.0);
                match level {
                    1 | 2 => ui.label(RichText::new(t).heading()),
                    3 => ui.label(RichText::new(t).strong().size(16.0)),
                    _ => ui.label(RichText::new(t).strong()),
                };
            }
            Block::Para(text) => md_inlines(ui, text, false),
            Block::Item {
                depth,
                marker,
                text,
            } => {
                ui.horizontal(|ui| {
                    ui.add_space(f32::from(*depth) * 18.0 + 6.0);
                    ui.label(format!("{marker} "));
                    ui.vertical(|ui| md_inlines(ui, text, false));
                });
            }
            Block::Table { header, rows } => {
                let width = match header.len() {
                    0..=2 => 520.0,
                    3..=4 => 360.0,
                    _ => 230.0,
                };
                ScrollArea::horizontal()
                    .id_salt(("md-table-scroll", doc, i))
                    .show(ui, |ui| {
                        Grid::new(("md-table", doc, i))
                            .striped(true)
                            .show(ui, |ui| {
                                for c in header {
                                    md_cell(ui, c, width, true);
                                }
                                ui.end_row();
                                for row in rows {
                                    for c in row {
                                        md_cell(ui, c, width, false);
                                    }
                                    ui.end_row();
                                }
                            });
                    });
            }
            Block::Code(lines) => {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    if ui.small_button("copy all").clicked() {
                        ui.ctx().copy_text(lines.join("\n"));
                    }
                    for l in lines {
                        copy_code(ui, l);
                    }
                });
            }
        }
    }
}

/// The prepared raw JSON in a scroll area that lays out only the visible lines.
fn raw_lines(ui: &mut egui::Ui, id: &str, title: &str, prepared: &Prepared, open: bool) {
    CollapsingHeader::new(format!("{title} ({} lines)", prepared.raw.len()))
        .id_salt(("raw", id, title))
        .default_open(open)
        .show(ui, |ui| {
            if prepared.raw_truncated {
                ui.weak(format!(
                    "cut to {} lines of {} characters; the MCP call or `kit --json` has all",
                    crate::envelope::RAW_LINES,
                    crate::envelope::RAW_LINE_CHARS
                ));
            }
            let row = ui.text_style_height(&egui::TextStyle::Monospace);
            ScrollArea::both()
                .id_salt(("raw-rows", id, title))
                .max_height(400.0)
                .auto_shrink([false, true])
                .show_rows(ui, row, prepared.raw.len(), |ui, range| {
                    for line in prepared.raw.get(range).unwrap_or_default() {
                        ui.add(
                            egui::Label::new(RichText::new(line).monospace())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    }
                });
        });
}
