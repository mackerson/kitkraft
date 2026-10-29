//! The egui window's shell: the top bar (title and tabs), the status line (the engine's
//! state and the session), and the central panel, which hands the frame to the open
//! tab's `ui` (`crate::panels`). Thin by design: every action is a [`Request`] to the
//! worker; every value shown comes from a view-model built from the envelope the engine
//! returned; a tab's clicks come back as [`Action`]s and are applied here after the
//! frame. Colors, sizes and spacing are `crate::theme`'s.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, ScrollArea};
use serde_json::{Map, Value};

use crate::cmdline::{mcp_call, result_line, session_args};
use crate::gate::{Request, LIVE_PROFILE};
use crate::panels::common::{Action, Shown, Slot, SlotKey, TabCtx};
use crate::panels::diag::{self, DiagTab};
use crate::panels::diff::{DiffMode, DiffTab};
use crate::panels::help::{self, HelpTab};
use crate::panels::ledger::{ledger_path, template_row, LedgerForm, LedgerTab};
use crate::panels::links::{LinksForm, LinksTab};
use crate::panels::load::{list_profiles, LoadForm, LoadTab, LoadView};
use crate::panels::planet::PlanetTab;
use crate::panels::records::{coverage_request, inspect_request, schema_request, RecordsTab};
use crate::panels::render::RenderTab;
use crate::panels::schema::SchemaTab;
use crate::panels::start::{instance_of_profile, profiles_request, StartProfile, StartTab};
use crate::panels::validate::ValidateTab;
// Lane G (M6).
use crate::panels::cockpit::session::POLL_INTERVAL;
use crate::panels::cockpit::{CockpitEnv, CockpitTab};
use crate::panels::coverage::CoverageTab;
use crate::perf::{self, Frames, Wheel};
use crate::prefs::{self, Prefs, WindowPrefs};
use crate::prepare::{build, prepare, Built, View};
use crate::theme::{self, Field, ThemeChoice};
use crate::worker::{Reply, Wake, Worker};
use crate::{LaunchOptions, UiError};
// Lane M (M7): the CK-style shell's glue, a child module so it reaches the tabs.
#[path = "windows/shell.rs"]
mod ck_shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl Tab {
    fn name(self) -> &'static str {
        TABS.iter()
            .find(|(t, _)| *t == self)
            .map_or("?", |(_, label)| label)
    }

    /// The tab that draws `slot`'s result.
    fn of_slot(slot: Slot) -> Tab {
        match slot {
            Slot::Profiles => Tab::Start,
            Slot::Load => Tab::Load,
            Slot::Types | Slot::Query | Slot::Inspect | Slot::InspectSchema => Tab::Records,
            Slot::Links => Tab::Links,
            Slot::Planet => Tab::Planet,
            Slot::Diff => Tab::Diff,
            Slot::DiagInspect | Slot::DiagValidate => Tab::Diagnostics,
            Slot::Validate | Slot::Roundtrip => Tab::Validate,
            Slot::Schema => Tab::Schema,
            Slot::Ledger => Tab::Ledger,
            Slot::CockpitStatus
            | Slot::CockpitValidate
            | Slot::CockpitRoundtrip
            | Slot::CockpitMasters
            | Slot::CockpitDeploy
            | Slot::CockpitRollback
            | Slot::CockpitSession
            | Slot::CockpitGameStatus
            | Slot::CockpitGameStop => Tab::Cockpit,
            Slot::Coverage => Tab::Coverage,
            Slot::RenderCell => Tab::Render,
        }
    }
}

/// The perf trace's state (`crate::perf`; empty and unused without it).
#[derive(Default)]
struct PerfState {
    frames: Frames,
    first_frame: bool,
    painted: BTreeSet<Tab>,
    /// Submit time per ticket.
    queued: BTreeMap<u64, Instant>,
    /// Results received and not yet drawn: (tab, ticket, command, submit time).
    to_draw: Vec<(Tab, u64, String, Instant)>,
    /// Drawn this frame; their frame time is reported on the next one.
    drawn: Vec<String>,
    bench: Option<Bench>,
    wheel: Wheel,
}

/// The window bench (`crate::perf::BENCH_ENV`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Bench {
    WaitLoad,
    WaitQuery,
    Settle(Instant, Tab),
    Scroll(Instant, Tab),
    WaitDiff,
    Done,
}

/// Bench timings: settle before scrolling, scroll down, then up.
const BENCH_SETTLE: Duration = Duration::from_secs(1);
const BENCH_DOWN: Duration = Duration::from_secs(3);
const BENCH_UP: Duration = Duration::from_secs(2);
/// Points per frame of the bench's wheel.
const BENCH_STEP: f32 = 60.0;

/// The names the bench finds its scroll targets by.
pub(crate) const PERF_TARGET_RECORDS: &str = "records-query";
const PERF_TARGET_CENTRAL: &str = "central";

/// Remembers `rect` as the bench's scroll target `name` (the Records query table, the
/// central panel); nothing without the bench.
pub(crate) fn note_perf_target(ctx: &egui::Context, name: &str, rect: egui::Rect) {
    if perf::bench_requested() {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(("perf-target", name)), rect));
    }
}

fn perf_target(ctx: &egui::Context, name: &str) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp(egui::Id::new(("perf-target", name))))
}

/// How long after start a minimized window is restored (M6.P7): a window started
/// minimized from a background shell is brought back; one the user minimizes later is
/// left alone.
const RESTORE_GRACE: Duration = Duration::from_secs(10);
/// A moved or resized window is remembered once it has been still this long.
const WINDOW_SETTLE: Duration = Duration::from_secs(1);

/// The window's state the shell follows (M6.P7).
struct WindowState {
    started: Instant,
    /// Focus asked for once at start.
    focus_asked: bool,
    /// Focus last seen (the trace logs each change).
    focused: Option<bool>,
    /// The saved place was clamped to the monitor present.
    clamped: bool,
    /// The place last seen, and when it changed (not yet saved).
    last: Option<WindowPrefs>,
    changed_at: Option<Instant>,
    /// Native pixels per point last seen (a DPI change between monitors).
    ppp: Option<f32>,
    /// Whether the place is remembered (not during the tour or the bench).
    remember: bool,
}

/// Whether `request` (a `load`) names the load order `held` holds: the same profile,
/// plugins and Data dir, compared without trailing separators and, on Windows, without
/// case. A `no_cache` load always runs (it asks to read the plugins again) (M6.P3a).
pub(crate) fn same_load(request: &Request, held: Option<&Map<String, Value>>) -> bool {
    let Some(held) = held else { return false };
    if request.args.get("no_cache").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    fn norm(v: &Value) -> Value {
        match v {
            Value::String(s) => {
                let t = s.trim().trim_end_matches(['\\', '/']).replace('/', "\\");
                Value::String(if cfg!(windows) { t.to_lowercase() } else { t })
            }
            Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
            other => other.clone(),
        }
    }
    let norm_map = |m: &Map<String, Value>| -> Vec<(String, Value)> {
        m.iter().map(|(k, v)| (k.clone(), norm(v))).collect()
    };
    norm_map(&session_args(&request.args)) == norm_map(held)
}

/// The profile (or the plugin count) a `load` request names, for a notice.
fn load_label(request: &Request) -> String {
    if let Some(p) = request.args.get("profile").and_then(Value::as_str) {
        return p.to_string();
    }
    let n = request
        .args
        .get("plugins")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    format!("{n} plugin(s)")
}

/// The load form built from the launch arguments (`--profile`, plugins, Data dir),
/// as the window's Load tab shows it.
fn load_form_from_opts(opts: &LaunchOptions) -> LoadForm {
    LoadForm {
        profile: opts.profile.clone().unwrap_or_default(),
        plugins: opts.plugins.join("\n"),
        data_dir: opts.data_dir.clone().unwrap_or_default(),
        no_cache: false,
    }
}

/// The profile `ui.json` remembers, trimmed (a blank entry is no profile at all).
fn remembered_profile(prefs: &Prefs) -> Option<String> {
    prefs
        .profile
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
}

/// The last path component of `profile` is the owner's live profile (`Default`, in any
/// case)? Read with `Path::file_name` (trailing separators trimmed), never by splitting
/// on `\` or `/`: the gate runs on Linux, the merge gate on Windows (KK-001).
fn live_profile_name(profile: &Path) -> bool {
    profile
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(LIVE_PROFILE))
}

/// M6.P3a: the `load` the window starts with, or `None` (the Start tab, as before).
/// The launch arguments win: with `--profile` or plugins, the load form's request, as
/// now. Otherwise, the last profile loaded from the Start tab (`prefs.profile`), from
/// the index cache (no `no_cache`), only when its folder still exists and its last path
/// component is not the owner's live profile (never loaded on its own; the Start tab
/// can still pick it, read-only). Never in a seeded window: the screenshot tour
/// (`crate::shots::SHOTS_ENV`) and the perf bench (`crate::perf::bench_requested()`)
/// seed their own loads, so they come in as `seeded` and keep this pure.
fn startup_load(
    opts: &LaunchOptions,
    prefs: &Prefs,
    seeded: bool,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<Request> {
    if opts.profile.is_some() || !opts.plugins.is_empty() {
        return load_form_from_opts(opts).request().ok();
    }
    if seeded {
        return None;
    }
    let profile = remembered_profile(prefs)?;
    let path = Path::new(profile.as_str());
    if live_profile_name(path) || !exists(path) {
        return None;
    }
    LoadForm {
        profile,
        ..LoadForm::default()
    }
    .request()
    .ok()
}

/// Why a start-up load was skipped, for the notice line under the tabs (only with no
/// launch arguments and a not-seeded window): the remembered profile is the owner's
/// live game, never loaded on its own, or its folder is gone.
fn start_up_skip_notice(prefs: &Prefs, exists: &dyn Fn(&Path) -> bool) -> Option<String> {
    let profile = remembered_profile(prefs)?;
    let path = Path::new(profile.as_str());
    if live_profile_name(path) {
        Some(format!(
            "the last profile {profile} is the owner's live game: it is never loaded on its own (the Start tab can load it, read-only)"
        ))
    } else if !exists(path) {
        Some(format!(
            "the last profile {profile} has no folder anymore: pick one from the Start tab"
        ))
    } else {
        None
    }
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
    perf: PerfState,
    /// The preset and mode in use, and where the choice came from (`--theme`, `ui.json`,
    /// the Start tab's picker, or the default).
    theme: ThemeChoice,
    theme_source: &'static str,
    /// The title bar was checked against the first envelope's `build` (M6.P6a).
    titled: bool,
    /// The picker changed the theme: apply it before the next frame.
    theme_changed: bool,
    worker: Worker<Built>,
    /// Load-order arguments of the `load` queued or running, if any (M6.P3a).
    loading: Option<Map<String, Value>>,
    /// An information line under the tabs (not an error).
    info: Option<String>,
    window: WindowState,
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
    shell: crate::windows::Shell,
}

/// Opens the window and blocks until it closes.
pub(crate) fn run_window(opts: LaunchOptions) -> Result<(), UiError> {
    let fonts = fonts()?;
    perf::line("fonts read", "");
    // The tour and the bench run at a fixed size; otherwise the window opens where it
    // was (`ui.json`, M6.P7), clamped to the monitor once it is known.
    let fixed = std::env::var_os(crate::shots::SHOTS_ENV).is_some() || perf::bench_requested();
    let saved = if fixed {
        None
    } else {
        prefs::default_path()
            .as_deref()
            .map(Prefs::load)
            .and_then(|p| p.window)
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(crate::panels::common::WINDOW_TITLE)
        .with_active(true)
        .with_inner_size(match saved {
            Some(w) => [w.width, w.height],
            None if fixed => crate::shots::TOUR_SIZE,
            None => theme::WINDOW,
        });
    if let Some(w) = saved {
        if let (Some(x), Some(y)) = (w.x, w.y) {
            viewport = viewport.with_position(egui::pos2(x, y));
        }
        viewport = viewport.with_maximized(w.maximized);
    }
    let native = eframe::NativeOptions {
        viewport,
        // A depth buffer for the Render Window's viewport (M7.R1); egui draws without one.
        depth_buffer: 24,
        ..Default::default()
    };
    eframe::run_native(
        "KitKraft Workbench",
        native,
        Box::new(move |cc| {
            perf::line("window created", "");
            cc.egui_ctx.set_fonts(fonts);
            let ctx = cc.egui_ctx.clone();
            let wake: Wake = std::sync::Arc::new(move || ctx.request_repaint());
            let worker = Worker::<Built>::spawn(Some(wake))?;
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
    fn new(worker: Worker<Built>, opts: LaunchOptions) -> Self {
        let mut load_form = load_form_from_opts(&opts);
        let profiles_dir = opts
            .profile
            .as_deref()
            .and_then(|p| Path::new(p).parent())
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let prefs_path = prefs::default_path();
        let prefs = prefs_path.as_deref().map(Prefs::load).unwrap_or_default();
        // M6.P3a: with no load-order arguments and a not-seeded window (the tour and
        // the bench seed their own loads), the last profile loads at start, from the
        // index cache; the owner's live profile never does.
        let seeded = std::env::var_os(crate::shots::SHOTS_ENV).is_some() || perf::bench_requested();
        let start_up = startup_load(&opts, &prefs, seeded, &|p| p.is_dir());
        // A skipped start-up load leaves the Start tab; when the folder is the live
        // profile or is gone, the notice line says why once.
        let start_up_notice =
            (start_up.is_none() && opts.profile.is_none() && opts.plugins.is_empty() && !seeded)
                .then(|| start_up_skip_notice(&prefs, &|p| p.is_dir()))
                .flatten();
        if start_up.is_some() && opts.profile.is_none() && opts.plugins.is_empty() {
            // The Load tab opens with the remembered profile in the form (as --profile).
            load_form.profile = remembered_profile(&prefs).unwrap_or_default();
        }
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
            titled: false,
            theme_changed: false,
            worker,
            // With launch arguments the Load tab opens as now; a start-up load opens it
            // too; otherwise the Start tab, as before.
            tab: if opts.profile.is_some() || !opts.plugins.is_empty() || start_up.is_some() {
                Tab::Load
            } else {
                Tab::Start
            },
            slots: BTreeMap::new(),
            notice: None,
            loading: None,
            info: None,
            window: WindowState {
                started: Instant::now(),
                focus_asked: false,
                focused: None,
                clamped: false,
                last: None,
                changed_at: None,
                ppp: None,
                remember: !seeded,
            },
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
                data: 0,
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
            perf: PerfState {
                bench: perf::bench_requested().then_some(Bench::WaitLoad),
                ..PerfState::default()
            },
            render: RenderTab::default(),
            shell: crate::windows::Shell::default(),
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
        if let Some(req) = start_up {
            app.submit(Slot::Load, req);
        }
        app.reload_help();
        if shots_refused.is_some() {
            app.notice = shots_refused;
        } else if theme_notice.is_some() {
            app.notice = theme_notice;
        } else if start_up_notice.is_some() {
            app.notice = start_up_notice;
        }
        app
    }

    fn submit(&mut self, slot: Slot, request: Request) {
        // M6.P3a: the session already holds (or is loading) this order: no second load.
        if slot == Slot::Load
            && (same_load(&request, self.session.as_ref())
                || same_load(&request, self.loading.as_ref()))
        {
            self.info = Some(format!(
                "{}: already loaded in this session; the views use it (tick \"no cache\" to read the plugins again)",
                load_label(&request)
            ));
            perf::line("request load reused", "");
            self.tab = Tab::Load;
            return;
        }
        let id = request.id.clone();
        // The Diagnostics view resolves plugin names through the load view.
        let load = (slot == Slot::DiagInspect)
            .then(|| self.load.view.clone())
            .flatten();
        let load_args = (slot == Slot::Load).then(|| session_args(&request.args));
        match self.worker.submit_with(request, Some(prepare(slot, load))) {
            Ok(ticket) => {
                if perf::enabled() {
                    perf::line(&format!("request {id} queued"), &format!("ticket={ticket}"));
                    self.perf.queued.insert(ticket, Instant::now());
                }
                if let Some(args) = load_args {
                    self.loading = Some(args);
                }
                self.slots.insert(ticket, slot);
                self.notice = None;
                self.info = None;
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    fn receive(&mut self, reply: Reply<Built>) {
        let Some(slot) = self.slots.remove(&reply.ticket) else {
            self.ck_receive(reply);
            return;
        };
        let received = Instant::now();
        if perf::enabled() {
            perf::line(
                &format!("request {} done", reply.request.id),
                &format!(
                    "ticket={} wait_ms={} run_ms={} first_progress_ms={} bytes={}",
                    reply.ticket,
                    perf::ms(reply.waited),
                    perf::ms(reply.elapsed),
                    reply
                        .first_progress
                        .map_or_else(|| "-".to_string(), perf::ms),
                    reply
                        .bytes
                        .map_or_else(|| "-".to_string(), |b| b.to_string()),
                ),
            );
        }
        let ticket = reply.ticket;
        let command = reply.request.id.clone();
        if slot == Slot::Load {
            self.loading = None;
        }
        let on_worker = reply.built.is_some();
        let Built { prepared, view } = match reply.built {
            Some(b) => b,
            None => build(slot, &reply.request, &reply.result, self.load.view.as_ref()),
        };
        if let Ok(env) = &reply.result {
            match view {
                View::Start(view) => {
                    if self.start.instance.trim().is_empty() {
                        self.start.instance = view.instance.clone().unwrap_or_default();
                    }
                    self.start.view = Some(view);
                }
                // A result that replaces a table's rows bumps its data key: the table
                // starts at the top with fresh column widths (bumped on arrival, not on
                // submit, so the old rows keep their place while the request runs).
                View::Load(view) => {
                    self.load.view = Some(view);
                    self.load.data = self.load.data.wrapping_add(1);
                    self.session = Some(session_args(&reply.request.args));
                }
                View::Types(rows) => {
                    self.records.type_rows = rows;
                    self.records.types_data = self.records.types_data.wrapping_add(1);
                }
                View::Query(view) => {
                    // A new query, page or type pick replaces the rows: the table starts
                    // at the top with fresh column widths.
                    let old = self.records.query_view.replace(view);
                    self.records.query_data = self.records.query_data.wrapping_add(1);
                    self.worker.dispose(old);
                }
                View::Record(view) => {
                    if !view.signature.is_empty() {
                        let req = schema_request(&view.signature);
                        self.submit(Slot::InspectSchema, req);
                    }
                    self.records.record_view = Some(view);
                }
                View::Links(view) => {
                    let old = self.links.view.replace(view);
                    self.links.data = self.links.data.wrapping_add(1);
                    self.worker.dispose(old);
                }
                View::Planet(view) => {
                    let old = self.planet.view.replace(view);
                    self.worker.dispose(old);
                }
                View::Diff(view) => {
                    let old = self.diff.view.replace(view);
                    self.diff.data = self.diff.data.wrapping_add(1);
                    self.worker.dispose(old);
                }
                View::Diag(view) => {
                    if self.diag.validate_plugin.trim().is_empty() {
                        self.diag.validate_plugin = self
                            .load
                            .view
                            .as_ref()
                            .map_or_else(|| view.winner.clone(), |l| l.path_of(&view.winner));
                    }
                    self.diag.view = Some(view);
                    self.diag.data = self.diag.data.wrapping_add(1);
                }
                View::Validate(view) => {
                    let old = self.validate.view.replace(view);
                    self.worker.dispose(old);
                }
                View::Roundtrip(view) => self.validate.roundtrip = Some(view),
                View::Schema(view) => self.schema.view = Some(view),
                View::Ledger(view) => {
                    if view.appended {
                        // One row per confirmed click: the note does not carry over.
                        self.ledger.form.note.clear();
                        self.reload_help();
                    }
                    self.ledger.view = Some(view);
                }
                View::Coverage(view) => {
                    self.coverage.view = Some(view);
                    self.coverage.data = self.coverage.data.wrapping_add(1);
                }
                // Replies joined into state the window holds (small, on the UI thread).
                View::None => match slot {
                    Slot::InspectSchema => {
                        if let Some(view) = &mut self.records.record_view {
                            view.join_schema(env);
                        }
                    }
                    Slot::DiagValidate => {
                        if let Some(view) = &mut self.diag.view {
                            view.with_validate(env);
                        }
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
                    // M6.G4: the session's replies join its state directly (no view).
                    Slot::CockpitSession => {
                        if reply.request.id == "game.probe" {
                            // A confirmed re-run: its one row joins the probe rows (M6.G5).
                            self.cockpit.session.receive_probe(env)
                        } else {
                            self.cockpit.session.receive_session(&reply.request, env)
                        }
                    }
                    Slot::CockpitGameStatus => self.cockpit.session.receive_status(env),
                    Slot::CockpitGameStop => self.cockpit.session.receive_stop(env),
                    _ => {}
                },
                // Lane R (M7.R2): the next page of the cell, until the draw cap.
                View::Cell(page) => {
                    if let Some(req) = self.render.receive_page(page) {
                        self.submit(Slot::RenderCell, req);
                    }
                }
            }
        } else if slot == Slot::RenderCell {
            self.render.failed();
        } else if matches!(
            slot,
            Slot::CockpitSession | Slot::CockpitGameStatus | Slot::CockpitGameStop
        ) {
            // Only a failed poll (or a failed start) ends the following; a failed
            // preview leaves it in place, and a failed re-run is kept as its own
            // line. The error stays in the slot's result.
            self.cockpit
                .session
                .failed(&reply.request, reply.result.as_ref().err());
        }
        // Replies arrive in submission order, so the session now is the one this
        // request ran against.
        let line = result_line(
            &reply.request.id,
            &reply.request.args,
            self.session.as_ref(),
        );
        let mcp = mcp_call(&reply.request.id, &reply.request.args);
        let error = reply.result.err();
        // The result this one replaces is freed on the engine thread (M6.P5a).
        let replaced = self.results.insert(
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
        if let Some(old) = replaced {
            self.worker.dispose(old);
        }
        if perf::enabled() {
            perf::line(
                &format!("request {command} prepared"),
                &format!(
                    "ticket={ticket} prep_ms={} on={} apply_ms={}",
                    perf::ms(reply.prep),
                    if on_worker { "worker" } else { "ui" },
                    perf::ms(received.elapsed())
                ),
            );
            let queued_at = self.perf.queued.remove(&ticket).unwrap_or(received);
            self.perf
                .to_draw
                .push((Tab::of_slot(slot), ticket, command, queued_at));
        }
    }

    /// Drops every queued request by restarting the engine thread; the new one has an
    /// empty session, so the load is forgotten too.
    fn drop_pending(&mut self) {
        match self.worker.restart() {
            Ok(n) => {
                self.slots.clear();
                self.session = None;
                self.loading = None;
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
                // The Cockpit's "Record in Ledger": nothing is written here — the form is
                // pre-filled on the Ledger tab, where the owner previews and confirms.
                Action::LedgerPrefill(form) => {
                    self.ledger.prefill(form);
                    self.tab = Tab::Ledger;
                }
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
                // Lane R (M7.R2): the shared selection, without leaving the tab.
                Action::Select(form) => {
                    self.records.record_form.clone_from(&form);
                    if let Ok(req) = inspect_request(&form, None) {
                        self.submit(Slot::Inspect, req);
                    }
                }
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
                theme::status_line(ui, theme::Status::Fail, n);
            }
            if let Some(i) = &self.info {
                ui.label(i);
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
                        let command = pending.first().copied().unwrap_or_default();
                        // M6.P2a: the running command's stage and progress.
                        match self.worker.running() {
                            Some(r) => {
                                let secs = r.started.elapsed().as_secs_f32();
                                match &r.progress {
                                    Some(p) => {
                                        if let Some(total) = p.total.filter(|t| *t > 0) {
                                            let share = (p.done as f32 / total as f32).clamp(0.0, 1.0);
                                            ui.add(
                                                egui::ProgressBar::new(share)
                                                    .desired_width(theme::width(ui, Field::Short))
                                                    .show_percentage(),
                                            );
                                        }
                                        ui.label(format!("{} ({secs:.1} s)", p.line()));
                                    }
                                    None => {
                                        ui.label(format!("running: {command} ({secs:.1} s)"));
                                    }
                                }
                            }
                            None => {
                                ui.label(format!("running: {command}"));
                            }
                        }
                        let queued = self.worker.queued();
                        if queued > 0 {
                            ui.label(format!("queued {queued}"));
                        }
                        drop_pending = ui
                            .small_button("drop pending")
                            .on_hover_text(
                                "restart the engine: queued requests are dropped, the running one's \
                                 reply is discarded, and the session is emptied (load again). The \
                                 engine cannot stop a command midway, so the running one finishes \
                                 in the background",
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
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.perf.wheel.inject(raw_input);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        // The viewport's GL objects, while the context is still current (eframe calls
        // this before it destroys the painter).
        if let Some(gl) = gl {
            crate::viewport::release_gl(gl);
        }
        self.perf.frames.print_all();
        if self.window.remember {
            if let Some(w) = self.window.last {
                if self.prefs.window != Some(w) {
                    self.prefs.window = Some(w);
                    self.save_prefs();
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        // egui may run a frame in more than one pass; eframe times the whole frame.
        if perf::enabled() && ui.ctx().current_pass_index() == 0 {
            self.perf_frame_start(frame.info().cpu_usage);
        }
        self.window_frame(ui.ctx());
        if self.theme_changed {
            self.theme_changed = false;
            theme::apply(ui.ctx(), &self.theme.preset.theme(), self.theme.mode);
        }
        for reply in self.worker.poll() {
            self.receive(reply);
        }
        if !self.titled {
            // M6.P6a: a debug build says so in the title bar, from the first envelope's
            // `build` member.
            if let Some(b) = crate::panels::common::build_of(&self.results) {
                self.titled = true;
                if let Some(title) = crate::panels::common::debug_title(b) {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Title(title));
                }
            }
        }
        let busy = self.worker.is_busy();
        if busy {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
        // M6.G4: the session's following runs on every frame, even while another tab
        // is shown: submit the poll that is due (one ordinary request on the same
        // worker), and keep the window painting at the interval while a session is
        // followed.
        if let Some(poll) = self.cockpit.session.poll_due(Instant::now()) {
            self.submit(Slot::CockpitGameStatus, poll);
        }
        if self.cockpit.session.following() {
            ui.ctx().request_repaint_after(POLL_INTERVAL);
        }
        self.ck_menu_bar(ui);
        self.top_bar(ui);
        self.ck_after_top_bar();
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
        let central_out = egui::CentralPanel::default_margins()
            .frame(central)
            .show(ui, |ui| {
                if tab == Tab::Records {
                    // Two panes, each with its own scroll (the inspector beside the table).
                    self.records.ui(ui, &mut ctx);
                    return;
                }
                if tab == Tab::Render {
                    // The viewport fills the tab and takes the wheel: no scroll area. The
                    // Records inspector's record is the shared selection (M7.R2).
                    self.render.inspected =
                        self.records.record_view.as_ref().map(|v| v.global.clone());
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
        note_perf_target(ui.ctx(), PERF_TARGET_CENTRAL, central_out.response.rect);
        let actions = ctx.actions;
        self.apply(actions);
        self.ck_windows(ui.ctx());
        if let Some(share) = self.records.split_changed.take() {
            self.prefs.records_split = Some(share);
            self.save_prefs();
        }
        if self.tour.is_some() {
            self.drive_tour(ui.ctx());
        }
        if perf::enabled() {
            self.perf_frame_end(ui.ctx(), frame_start.elapsed());
        }
        if self.perf.bench.is_some() {
            self.drive_bench(ui.ctx());
        }
    }
}

impl App {
    /// Window quirks, once per frame (M6.P7): a window that starts minimized is restored
    /// and focused; the saved place is clamped to the monitor present; a DPI change
    /// repaints; the place is remembered in `ui.json` once it settles.
    fn window_frame(&mut self, ctx: &egui::Context) {
        let (minimized, focused, inner, outer, maximized, monitor, ppp) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.minimized,
                v.focused,
                v.inner_rect,
                v.outer_rect,
                v.maximized,
                v.monitor_size,
                v.native_pixels_per_point,
            )
        });
        let w = &mut self.window;
        if minimized == Some(true) && (w.started.elapsed() < RESTORE_GRACE || self.tour.is_some()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            perf::line("window restored", "it was minimized");
        }
        if !w.focus_asked {
            w.focus_asked = true;
            if focused != Some(true) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        if focused != w.focused {
            perf::line(
                "window focus",
                &format!("focused={focused:?} minimized={minimized:?}"),
            );
            w.focused = focused;
        }
        if !w.clamped && w.remember {
            if let Some(m) = monitor {
                w.clamped = true;
                if let Some(saved) = self.prefs.window {
                    let c = saved.clamp(Some([m.x, m.y]));
                    if (c.width, c.height) != (saved.width, saved.height) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            c.width, c.height,
                        )));
                    }
                    if saved.x.is_some() && c.x.is_none() {
                        let x = ((m.x - c.width) / 2.0).max(0.0);
                        let y = ((m.y - c.height) / 2.0).max(0.0);
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                            x, y,
                        )));
                    }
                }
            }
        }
        if ppp != w.ppp {
            if w.ppp.is_some() {
                perf::line("dpi changed", &format!("pixels_per_point={ppp:?}"));
                ctx.request_repaint();
            }
            w.ppp = ppp;
        }
        if !w.remember || minimized == Some(true) {
            return;
        }
        let Some(inner) = inner else { return };
        let maximized = maximized == Some(true);
        let previous = w.last.or(self.prefs.window);
        // Maximized: keep the normal size and place to come back to.
        let now = match (maximized, previous) {
            (true, Some(p)) => WindowPrefs {
                maximized: true,
                monitor: monitor.map(|m| [m.x, m.y]).or(p.monitor),
                ..p
            },
            _ => WindowPrefs {
                width: inner.width(),
                height: inner.height(),
                x: outer.map(|r| r.min.x),
                y: outer.map(|r| r.min.y),
                maximized,
                monitor: monitor.map(|m| [m.x, m.y]),
            },
        };
        if w.last != Some(now) {
            w.last = Some(now);
            w.changed_at = Some(Instant::now());
            ctx.request_repaint_after(WINDOW_SETTLE);
        } else if w.changed_at.is_some_and(|t| t.elapsed() >= WINDOW_SETTLE) {
            w.changed_at = None;
            if self.prefs.window != Some(now) {
                self.prefs.window = Some(now);
                self.save_prefs();
            }
        }
    }

    /// The phase a frame's time is counted in: the bench's scroll phase, else the tab.
    fn perf_phase(&self) -> String {
        match self.perf.bench {
            Some(Bench::Scroll(_, tab)) => format!("bench-scroll-{}", tab.name()),
            _ => self.tab.name().to_string(),
        }
    }

    /// Records the previous frame's time (eframe measures a frame after painting it,
    /// without the vsync wait) and reports the results drawn in it.
    fn perf_frame_start(&mut self, last_frame_secs: Option<f32>) {
        let Some(secs) = last_frame_secs else { return };
        let ms = secs * 1000.0;
        let phase = self.perf_phase();
        self.perf.frames.record(&phase, ms);
        for event in std::mem::take(&mut self.perf.drawn) {
            perf::line(&event, &format!("frame_ms={ms:.2}"));
        }
    }

    /// First frame, each tab's first paint, and results drawn this frame.
    fn perf_frame_end(&mut self, ctx: &egui::Context, ui_time: Duration) {
        let ui_ms = perf::ms(ui_time);
        if !self.perf.first_frame {
            self.perf.first_frame = true;
            let state = ctx.input(|i| {
                let v = i.viewport();
                format!(
                    "minimized={:?} focused={:?} inner={:?}",
                    v.minimized,
                    v.focused,
                    v.inner_rect.map(|r| r.size())
                )
            });
            perf::line("first-frame", &format!("ui_ms={ui_ms} {state}"));
            self.perf.drawn.push("first-frame painted".to_string());
        }
        if self.perf.painted.insert(self.tab) {
            perf::line(
                &format!("tab {} first-paint", self.tab.name()),
                &format!("ui_ms={ui_ms}"),
            );
            self.perf
                .drawn
                .push(format!("tab {} first-paint painted", self.tab.name()));
        }
        let tab = self.tab;
        let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.perf.to_draw)
            .into_iter()
            .partition(|(t, ..)| *t == tab);
        self.perf.to_draw = later;
        for (_, ticket, command, queued) in now {
            perf::line(
                &format!("request {command} drawn"),
                &format!(
                    "ticket={ticket} total_ms={} ui_ms={ui_ms}",
                    perf::ms(queued.elapsed())
                ),
            );
            self.perf
                .drawn
                .push(format!("request {command} painted ticket={ticket}"));
        }
    }

    /// One step of the window bench (`crate::perf::BENCH_ENV`).
    fn drive_bench(&mut self, ctx: &egui::Context) {
        ctx.request_repaint();
        let busy = self.worker.is_busy();
        let Some(step) = self.perf.bench else { return };
        // This tab's results are drawn and their frame reported.
        let drawn =
            !self.perf.to_draw.iter().any(|(t, ..)| *t == self.tab) && self.perf.drawn.is_empty();
        match step {
            Bench::WaitLoad if !busy && self.load.view.is_some() && drawn => {
                let q = &mut self.records.query_form;
                q.record_type = "REFR".into();
                q.where_text.clear();
                q.limit = 1000;
                self.tab = Tab::Records;
                self.apply(vec![Action::QueryPage(None)]);
                self.perf.bench = Some(Bench::WaitQuery);
            }
            Bench::WaitQuery if !busy && drawn && self.records.query_view.is_some() => {
                self.perf.bench = Some(Bench::Settle(Instant::now(), Tab::Records));
            }
            Bench::WaitDiff if !busy && drawn && self.diff.view.is_some() => {
                self.perf.bench = Some(Bench::Settle(Instant::now(), Tab::Diff));
            }
            Bench::Settle(at, tab) if at.elapsed() >= BENCH_SETTLE => {
                let target = if tab == Tab::Records {
                    PERF_TARGET_RECORDS
                } else {
                    PERF_TARGET_CENTRAL
                };
                self.perf.wheel = Wheel {
                    target: perf_target(ctx, target),
                    step: -BENCH_STEP,
                };
                if self.perf.wheel.target.is_none() {
                    perf::line("bench", &format!("no scroll target {target}"));
                }
                self.perf.bench = Some(Bench::Scroll(Instant::now(), tab));
            }
            Bench::Scroll(at, tab) => {
                let t = at.elapsed();
                if t >= BENCH_DOWN.saturating_add(BENCH_UP) {
                    self.perf.wheel = Wheel::default();
                    let phase = self.perf_phase();
                    self.perf.frames.flush(&phase);
                    if tab == Tab::Records {
                        self.bench_diff();
                    } else {
                        self.perf.bench = Some(Bench::Done);
                    }
                } else if t >= BENCH_DOWN {
                    self.perf.wheel.step = BENCH_STEP;
                }
            }
            Bench::Done => {
                perf::line("bench done", "");
                self.perf.bench = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {}
        }
    }

    /// The bench's Diff request: the plugin `KITKRAFT_WORKBENCH_PERF_DIFF` names, else
    /// the tour's (the biggest plugin the profile activates), against its masters.
    fn bench_diff(&mut self) {
        let chosen = std::env::var(perf::BENCH_DIFF_ENV)
            .ok()
            .filter(|p| !p.trim().is_empty());
        let plugin = chosen.or_else(|| {
            self.load.view.as_ref().and_then(|l| {
                l.plugins
                    .iter()
                    .filter(|p| p.active == "plugins.txt")
                    .max_by_key(|p| p.records.unwrap_or(0))
                    .map(|p| p.name.clone())
            })
        });
        let Some(name) = plugin else {
            perf::line("bench", "no plugin activated by plugins.txt: Diff skipped");
            self.perf.bench = Some(Bench::Done);
            return;
        };
        perf::line("bench diff", &format!("plugin={name}"));
        self.diff.form.mode = DiffMode::VsMasters;
        self.diff.form.a = name;
        self.tab = Tab::Diff;
        match self.diff.form.request(None) {
            Ok(req) => {
                self.submit(Slot::Diff, req);
                self.perf.bench = Some(Bench::WaitDiff);
            }
            Err(e) => {
                perf::line("bench", &format!("diff request: {e}"));
                self.perf.bench = Some(Bench::Done);
            }
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
                // Lane R (M7.R2): a cell in the Render Window, when the tour names one.
                let render = self.render.tour_seed();
                self.apply(render);
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
        if self.tour.as_ref().is_some_and(|t| t.tab >= TABS.len()) && self.ck_tour(ctx) {
            return;
        }
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
        // A minimized window paints nothing, so a screenshot would never arrive: wait
        // for the restore (`window_frame`) before counting (the tour used to hang here).
        if ctx.input(|i| i.viewport().minimized) == Some(true) {
            tour.frames = 0;
            return;
        }
        tour.frames = tour.frames.saturating_add(1);
        // A few frames for the tab to lay out, then one request (frame 5), asked again
        // if no image came back within a second or so.
        if tour.frames == 5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        } else if tour.frames >= 60 {
            tour.frames = 0;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// M6.P3a: a second `load` of the order the session holds is answered from the
    /// session; another order, or a `no_cache` load, runs.
    #[test]
    fn a_repeated_load_is_answered_from_the_session() {
        let held = session_args(&json!({"profile": r"C:\MO2\profiles\Kitkraft Dev"}));
        let same = Request::new("load", json!({"profile": "c:/mo2/profiles/kitkraft dev/"}));
        let other = Request::new("load", json!({"profile": r"C:\MO2\profiles\Other"}));
        let fresh = Request::new(
            "load",
            json!({"profile": r"C:\MO2\profiles\Kitkraft Dev", "no_cache": true}),
        );
        let data_dir = Request::new(
            "load",
            json!({"profile": r"C:\MO2\profiles\Kitkraft Dev", "data_dir": "D"}),
        );
        assert_eq!(same_load(&same, Some(&held)), cfg!(windows));
        assert!(same_load(
            &Request::new("load", json!({"profile": r"C:\MO2\profiles\Kitkraft Dev"})),
            Some(&held)
        ));
        assert!(!same_load(&other, Some(&held)));
        assert!(!same_load(&fresh, Some(&held)));
        assert!(!same_load(&data_dir, Some(&held)));
        assert!(!same_load(&same, None));
        let plugins = session_args(&json!({"plugins": ["A.esm", "B.esp"]}));
        assert!(same_load(
            &Request::new("load", json!({"plugins": ["A.esm", "B.esp"]})),
            Some(&plugins)
        ));
        assert!(!same_load(
            &Request::new("load", json!({"plugins": ["B.esp", "A.esm"]})),
            Some(&plugins)
        ));
        assert_eq!(load_label(&other), r"C:\MO2\profiles\Other");
    }

    /// M6.P3a: without launch arguments, the profile `ui.json` remembers (an existing
    /// folder) loads at start, from the index cache (no `no_cache`).
    #[test]
    fn the_last_profile_loads_at_start() {
        let base = std::env::temp_dir().join(format!("kitkraft-ui-startup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let profile = base.join("profiles").join("KitKraft Dev");
        std::fs::create_dir_all(&profile).unwrap();
        let prefs = Prefs {
            profile: Some(profile.display().to_string()),
            ..Prefs::default()
        };
        let req = startup_load(&LaunchOptions::default(), &prefs, false, &|p| p.is_dir())
            .expect("the remembered folder exists and is not the live profile");
        assert_eq!(req.id, "load");
        assert_eq!(req.args, json!({"profile": profile.display().to_string()}));
        assert!(req.args.get("no_cache").is_none());
        crate::gate::check(&req).unwrap();
        let _ = std::fs::remove_dir_all(&base);
    }

    /// M6.P3a: no start-up load for the owner's live profile (in any case), a gone
    /// folder, a seeded window (the tour or the bench seed their own loads), or no
    /// remembered profile; launch arguments win over the remembered profile.
    #[test]
    fn no_start_up_load_for_default_a_missing_profile_or_launch_arguments() {
        let base =
            std::env::temp_dir().join(format!("kitkraft-ui-startup-skip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for name in ["Default", "default", "KitKraft Dev"] {
            std::fs::create_dir_all(base.join("profiles").join(name)).unwrap();
        }
        let exists = |p: &Path| p.is_dir();
        for name in ["Default", "default"] {
            let prefs = Prefs {
                profile: Some(base.join("profiles").join(name).display().to_string()),
                ..Prefs::default()
            };
            assert!(startup_load(&LaunchOptions::default(), &prefs, false, &exists).is_none());
        }
        let gone = Prefs {
            profile: Some(base.join("profiles").join("Gone").display().to_string()),
            ..Prefs::default()
        };
        assert!(startup_load(&LaunchOptions::default(), &gone, false, &exists).is_none());
        let dev = base.join("profiles").join("KitKraft Dev");
        let dev_prefs = Prefs {
            profile: Some(dev.display().to_string()),
            ..Prefs::default()
        };
        assert!(startup_load(&LaunchOptions::default(), &dev_prefs, true, &exists).is_none());
        let x = base.join("profiles").join("X").display().to_string();
        let with_args = LaunchOptions {
            profile: Some(x.clone()),
            ..LaunchOptions::default()
        };
        let req = startup_load(&with_args, &dev_prefs, false, &exists)
            .expect("launch arguments name a profile");
        assert_eq!(req.args, json!({"profile": x}));
        assert!(
            startup_load(&LaunchOptions::default(), &Prefs::default(), false, &exists).is_none()
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
