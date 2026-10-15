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

use crate::cmdline::{command_line, mcp_call};
use crate::envelope::EnvelopeView;
use crate::gate::{Request, RunError};
use crate::panels::diag::{self, DiagView};
use crate::panels::diff::{DiffForm, DiffMode, DiffView};
use crate::panels::load::{list_profiles, LoadForm, LoadView, ProfileEntry};
use crate::panels::records::{
    coverage_request, inspect_request, schema_request, types_from_coverage, types_from_schema,
    QueryForm, QueryView, RecordView, TypeRow,
};
use crate::panels::schema::{SchemaForm, SchemaView};
use crate::panels::validate::{RoundtripView, ValidateForm, ValidateView};
use crate::worker::{Reply, Worker};
use crate::{LaunchOptions, UiError};

/// Rows of evidence shown per result; the raw JSON has the rest.
const EVIDENCE_ROWS: usize = 300;
/// Characters of a limitation shown on its line (the full text is on hover).
const LIMITATION_CHARS: usize = 200;
/// Characters of raw `data` JSON shown per result.
const RAW_CHARS: usize = 200_000;
/// Commands whose `kit` line needs the load order repeated (each `kit` process starts
/// with an empty session; the workbench, like `kit mcp`, keeps one).
const SESSION_COMMANDS: &[&str] = &["inspect", "query", "conflicts", "validate"];

const WARN: Color32 = Color32::from_rgb(230, 170, 60);
const BAD: Color32 = Color32::from_rgb(230, 90, 80);
const GOOD: Color32 = Color32::from_rgb(110, 190, 110);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Load,
    Records,
    Diff,
    Diagnostics,
    Validate,
    Schema,
}

const TABS: &[(Tab, &str)] = &[
    (Tab::Load, "Load"),
    (Tab::Records, "Records"),
    (Tab::Diff, "Diff"),
    (Tab::Diagnostics, "Diagnostics"),
    (Tab::Validate, "Validate"),
    (Tab::Schema, "Schema"),
];

/// Where a reply goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Load,
    Types,
    Query,
    Inspect,
    InspectSchema,
    Diff,
    DiagInspect,
    DiagValidate,
    Validate,
    Roundtrip,
    Schema,
}

/// One finished request.
struct Shown {
    request: Request,
    result: Result<Value, RunError>,
    elapsed: Duration,
}

/// What a click asks for, applied after the frame is drawn.
enum Action {
    Submit(Slot, Request),
    Inspect(String),
    Diagnose(String),
    PickType(String),
    QueryPage(Option<String>),
    UseProfile(PathBuf),
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
            let wake: Box<dyn Fn() + Send> = Box::new(move || ctx.request_repaint());
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
        let mut app = Self {
            worker,
            tab: Tab::Load,
            slots: BTreeMap::new(),
            notice: None,
            results: BTreeMap::new(),
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
            tour: std::env::var_os(crate::shots::SHOTS_ENV).map(|dir| Tour {
                dir: PathBuf::from(dir),
                step: TourStep::WaitLoad,
                tab: 0,
                frames: 0,
            }),
        };
        app.submit(
            Slot::Types,
            Request::new("schema", Value::Object(Map::new())),
        );
        if opts.profile.is_some() || !opts.plugins.is_empty() {
            if let Ok(req) = app.load_form.request() {
                app.submit(Slot::Load, req);
            }
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
                Slot::Load => self.load_view = Some(LoadView::from_envelope(env)),
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
        self.results.insert(
            slot.into(),
            Shown {
                request: reply.request,
                result: reply.result,
                elapsed: reply.elapsed,
            },
        );
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
                if busy {
                    ui.spinner();
                    ui.label(format!("running: {}", self.worker.pending().join(", ")));
                } else {
                    ui.weak("idle");
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
                    Tab::Load => self.tab_load(ui, busy, &mut actions),
                    Tab::Records => self.tab_records(ui, busy, &mut actions),
                    Tab::Diff => self.tab_diff(ui, busy, &mut actions),
                    Tab::Diagnostics => self.tab_diag(ui, busy, &mut actions),
                    Tab::Validate => self.tab_validate(ui, busy, &mut actions),
                    Tab::Schema => self.tab_schema(ui, busy, &mut actions),
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
                    actions.push(Action::Diagnose(form));
                }
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
        let extra = Map::new();
        envelope_block(ui, self.results.get(&Slot::Load.into()), &extra, |ui| {
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
        let extra = self.load_form.load_order_args();
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
                envelope_block(
                    ui,
                    self.results.get(&Slot::Types.into()),
                    &Map::new(),
                    |ui| {
                        let filter = self.type_filter.trim().to_ascii_uppercase();
                        ScrollArea::vertical()
                            .id_salt("types-rows")
                            .max_height(220.0)
                            .show(ui, |ui| {
                                Grid::new("types-grid").striped(true).show(ui, |ui| {
                                    for h in [
                                        "type",
                                        "definition",
                                        "records",
                                        "confidence / occurrences",
                                    ] {
                                        ui.strong(h);
                                    }
                                    ui.end_row();
                                    for t in self.type_rows.iter().filter(|t| {
                                        filter.is_empty() || t.signature.contains(&filter)
                                    }) {
                                        if ui
                                            .link(&t.signature)
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
                                        ui.label(
                                            t.records.map_or_else(String::new, |n| n.to_string()),
                                        );
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
                    },
                );
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
                envelope_block(ui, self.results.get(&Slot::Query.into()), &extra, |ui| {
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
                                    if ui.link(&r.form).on_hover_text("inspect").clicked() {
                                        actions.push(Action::Inspect(r.form.clone()));
                                    }
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
                envelope_block(ui, self.results.get(&Slot::Inspect.into()), &extra, |ui| {
                    let Some(view) = &self.record_view else {
                        return;
                    };
                    record_view(ui, view, actions);
                });
                if let Some(shown) = self.results.get(&Slot::InspectSchema.into()) {
                    CollapsingHeader::new("provenance source: schema --deep")
                        .id_salt("record-schema")
                        .show(ui, |ui| {
                            envelope_block(ui, Some(shown), &Map::new(), |_| {})
                        });
                }
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
        let extra = self.load_form.load_order_args();
        envelope_block(ui, self.results.get(&Slot::Diff.into()), &extra, |ui| {
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
                        if ui.link("inspect across the load order").clicked() {
                            actions.push(Action::Inspect(r.form.clone()));
                        }
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
        ui.label("Raw stored FormIDs beside master-aware identities (`inspect`), then `validate`'s master and FormID rules. Incoming references wait for `refs` (M4.D1).");
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
        let extra = self.load_form.load_order_args();
        envelope_block(
            ui,
            self.results.get(&Slot::DiagInspect.into()),
            &extra,
            |ui| {
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
                            for h in ["at", "raw", "resolved"] {
                                ui.strong(h);
                            }
                            ui.end_row();
                            for r in &view.references {
                                ui.monospace(&r.at);
                                ui.monospace(&r.raw);
                                if r.resolved == "unresolved" {
                                    ui.colored_label(BAD, &r.resolved);
                                } else if r.resolved == "NULL" {
                                    ui.label(&r.resolved);
                                } else if ui.link(&r.resolved).clicked() {
                                    actions.push(Action::Diagnose(r.resolved.clone()));
                                }
                                ui.end_row();
                            }
                        });
                    });
            },
        );
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
        envelope_block(
            ui,
            self.results.get(&Slot::DiagValidate.into()),
            &extra,
            |ui| {
                let Some(view) = &self.diag_view else { return };
                if view.findings.is_empty() && view.validate_summary.is_some() {
                    ui.colored_label(GOOD, "no master or FormID finding");
                }
                findings_grid(ui, "diag-findings", view.findings.iter());
            },
        );
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
        let extra = self.load_form.load_order_args();
        envelope_block(ui, self.results.get(&Slot::Validate.into()), &extra, |ui| {
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
        envelope_block(
            ui,
            self.results.get(&Slot::Roundtrip.into()),
            &Map::new(),
            |ui| {
                let Some(view) = &self.roundtrip_view else {
                    return;
                };
                ui.label(&view.counts);
                if let Some(m) = &view.mismatch {
                    ui.colored_label(BAD, format!("first mismatch {m}"));
                }
            },
        );
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
        envelope_block(
            ui,
            self.results.get(&Slot::Schema.into()),
            &Map::new(),
            |ui| {
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
                            if ui.link(ty).clicked() {
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
            },
        );
    }
}

// ---- shared pieces --------------------------------------------------------------------

fn conf_label(ui: &mut egui::Ui, confidence: &str) {
    if confidence == "reference" {
        ui.colored_label(WARN, "reference (unconfirmed)")
            .on_hover_text(
                "documented in the pinned xEdit file; not yet confirmed by the corpus or the CK",
            );
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

fn record_view(ui: &mut egui::Ui, view: &RecordView, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.strong(format!(
            "{} {} {}",
            view.signature, view.global, view.editor_id
        ));
        ui.label(format!(
            "matched by {}; winner {}; runtime ID {}",
            view.matched_by, view.winner, view.runtime_id
        ));
        if !view.global.is_empty() && view.mode == "load_order" && ui.link("diagnose").clicked() {
            actions.push(Action::Diagnose(view.global.clone()));
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
/// panel's own `content`, the evidence table and the raw `data`.
fn envelope_block(
    ui: &mut egui::Ui,
    shown: Option<&Shown>,
    extra: &Map<String, Value>,
    content: impl FnOnce(&mut egui::Ui),
) {
    let Some(shown) = shown else {
        return;
    };
    ui.separator();
    let id = shown.request.id.as_str();
    let mut cli_args = shown.request.args.clone();
    let session = SESSION_COMMANDS.contains(&id) && !extra.is_empty();
    if session {
        if let Some(map) = cli_args.as_object_mut() {
            for (k, v) in extra {
                map.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
    }
    let line = command_line(id, &cli_args);
    let mcp = mcp_call(id, &shown.request.args);
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
    if session {
        ui.weak("load-order arguments added: each `kit` process starts with an empty session (this window keeps one, like `kit mcp`)");
    }
    ui.horizontal(|ui| {
        if ui.small_button("copy").clicked() {
            ui.ctx().copy_text(mcp.clone());
        }
        ui.weak("MCP execute:");
        ui.monospace(&mcp);
    });
    let env = match &shown.result {
        Ok(env) => env,
        Err(err) => {
            ui.colored_label(BAD, err.to_string());
            if let RunError::Failed { json, .. } = err {
                ui.monospace(serde_json::to_string_pretty(json).unwrap_or_default());
            }
            return;
        }
    };
    let view = EnvelopeView::from_value(env);
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
    CollapsingHeader::new("raw data (JSON)")
        .id_salt(("raw", id))
        .show(ui, |ui| {
            let text = env
                .get("data")
                .and_then(|d| serde_json::to_string_pretty(d).ok())
                .unwrap_or_default();
            let shown: String = text.chars().take(RAW_CHARS).collect();
            ScrollArea::vertical()
                .id_salt(("raw-rows", id))
                .max_height(400.0)
                .show(ui, |ui| {
                    ui.monospace(shown);
                });
        });
}
