//! Cockpit panel (M6.G2, M6.G3, M6.G8; ADR 0007 Amendment 3): the plugin under review
//! and what follows from it, in one place.
//!
//! - **Review** (M6.G8): one fixed sequence of reads, `validate`, `roundtrip --verify`,
//!   then the plugin against its masters (`conflicts --plugin` when the plugin is in the
//!   loaded order, else a `diff` against each master `validate` found), and a summary
//!   with the six statuses of AGENTS.md "Reporting": `parse` and `byte_roundtrip` from
//!   what ran, the rest `not-run` until each runs.
//! - **Dev deployment** (M6.G3): `deploy.status` (read-only), then `deploy --dev` and
//!   `deploy.rollback`, each previewed with `dry_run: true` (every path listed) and
//!   written only from the confirm box's button ([`DeployForm::confirmed_request`]).
//!   The panel fills `mo2` and `profile` from the loaded dev profile and keeps the
//!   confirm box closed when they differ from it, when the loaded profile is `Default`,
//!   or when the preview is missing, refused or for other arguments
//!   ([`confirm_gate`]). The gate checks `dev: true` and the profile again; the engine
//!   refuses `Default` on its own.
//! - **Ledger**: the entries of `docs/game-observations.md` that name the plugin's file
//!   or its SHA-256 (the hash comes from the deploy preview: the engine hashes the
//!   source, the window computes nothing).
//! - **Game session** buttons (M6.G4 to M6.G6): their state is read from the catalog
//!   and the gate ([`game_button`]). `game.session` and `game.stop` are permitted now
//!   (M6.G4): [`session::SessionForm`] builds their previews and confirmed requests;
//!   the button wiring is the next ticket. `game.probe` stays disabled with M6.G5.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::envelope::{list, short, strings, text, text_or_empty};
use crate::gate::{self, Request, LIVE_PROFILE};
use crate::panels::diff::DiffView;
use crate::panels::start::instance_of_profile;
use crate::panels::validate::{RoundtripView, ValidateView};

/// The game session's form and request builders (M6.G4, first half); no drawing yet.
pub mod session;

/// Where agents and builds put plugins, under the checkout.
pub const OUT_DIR: &str = "out";
/// Folder depth searched under [`OUT_DIR`].
const OUT_DEPTH: u8 = 4;
/// Plugins listed from [`OUT_DIR`] at most.
const OUT_MAX: usize = 200;

/// The plugin files (`.esm`, `.esp`, `.esl`) under `dir`, a few folders deep, sorted.
/// A file picker for the panel; it reads names only.
pub fn plugins_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0u8)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() && depth < OUT_DEPTH {
                stack.push((path, depth.saturating_add(1)));
            } else if kind.is_file() && is_plugin(&path) {
                out.push(path.display().to_string());
            }
        }
    }
    out.sort();
    out.truncate(OUT_MAX);
    out
}

fn is_plugin(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["esm", "esp", "esl"].contains(&e.to_ascii_lowercase().as_str()))
}

/// The file name of a path as typed (`C:\x\My.esm` -> `My.esm`).
pub fn file_name(path: &str) -> String {
    Path::new(path.trim())
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The file stem (`My.esm` -> `My`): the dev mod's name is `KitKraft Dev - <stem>`.
pub fn stem(path: &str) -> String {
    Path::new(path.trim())
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// ---- the dev target ---------------------------------------------------------------------

/// Where deploy and roll back go: the MO2 instance and profile name of the loaded
/// profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevTarget {
    /// The instance root (holds `ModOrganizer.ini`).
    pub mo2: String,
    /// The profile's folder name (`KitKraft Dev`).
    pub profile: String,
}

/// The dev target from the profile the session loaded, or why there is none. Never
/// `Default`: the owner's live game is not a target (AGENTS.md).
pub fn dev_target(loaded_profile: Option<&str>) -> Result<DevTarget, String> {
    let path = loaded_profile
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| {
            "no MO2 profile is loaded: load the dev profile (Start or Load tab); deploy and roll back take their target from it".to_string()
        })?;
    let profile = file_name(path);
    if profile.trim().eq_ignore_ascii_case(LIVE_PROFILE) {
        return Err(format!(
            "the loaded profile is `{LIVE_PROFILE}`, the owner's live game: deploy and roll back stay closed; load the dev profile"
        ));
    }
    let mo2 = instance_of_profile(path)
        .ok_or_else(|| format!("{path}: not a folder in an MO2 instance's `profiles` folder"))?;
    Ok(DevTarget { mo2, profile })
}

/// Two paths name the same folder, as typed: case, separators and a trailing separator
/// do not count.
fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

// ---- deploy and roll back -----------------------------------------------------------------

/// The deploy form: the source (the plugin under review, or a package folder), the
/// instance and the profile (filled from the loaded dev profile).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeployForm {
    /// A plugin or a package folder.
    pub source: String,
    /// The MO2 instance root.
    pub mo2: String,
    /// The profile name (`KitKraft Dev`).
    pub profile: String,
}

/// Which write a preview or a confirm box is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteKind {
    /// `deploy --dev`.
    Deploy,
    /// `deploy.rollback`.
    Rollback,
}

impl WriteKind {
    /// The catalog id.
    pub fn id(self) -> &'static str {
        match self {
            WriteKind::Deploy => "deploy",
            WriteKind::Rollback => "deploy.rollback",
        }
    }
}

impl DeployForm {
    /// Fills `mo2` and `profile` from `target`.
    pub fn fill(&mut self, target: &DevTarget) {
        self.mo2.clone_from(&target.mo2);
        self.profile.clone_from(&target.profile);
    }

    /// The dev mod's name for the source (`KitKraft Dev - <stem>`), as `deploy.status`
    /// takes it.
    pub fn mod_name(&self) -> String {
        stem(&self.source)
    }

    fn base(&self) -> Result<Map<String, Value>, String> {
        let mo2 = self.mo2.trim();
        if mo2.is_empty() {
            return Err("no MO2 instance: load the dev profile".to_string());
        }
        let mut a = Map::new();
        a.insert("mo2".into(), json!(mo2));
        a.insert("dev".into(), json!(true));
        if !self.profile.trim().is_empty() {
            a.insert("profile".into(), json!(self.profile.trim()));
        }
        Ok(a)
    }

    /// The writing form's arguments for `kind` (no `dry_run`). A rollback names the dev
    /// mod and, when `deploy.status` showed one, the `.previous` id it expects (`to`).
    pub fn args(&self, kind: WriteKind, to: Option<&str>) -> Result<Value, String> {
        let mut a = self.base()?;
        match kind {
            WriteKind::Deploy => {
                let source = self.source.trim();
                if source.is_empty() {
                    return Err("pick the plugin under review first".to_string());
                }
                a.insert("source".into(), json!(source));
            }
            WriteKind::Rollback => {
                let m = self.mod_name();
                if m.is_empty() {
                    return Err("pick the plugin whose dev mod to roll back".to_string());
                }
                a.insert("mod".into(), json!(m));
                if let Some(to) = to.filter(|t| !t.is_empty()) {
                    a.insert("to".into(), json!(to));
                }
            }
        }
        Ok(Value::Object(a))
    }

    /// The preview: the writing form's arguments with `dry_run: true`. It writes nothing
    /// and passes the gate like a read.
    pub fn preview_request(&self, kind: WriteKind, to: Option<&str>) -> Result<Request, String> {
        let mut a = self.args(kind, to)?;
        if let Some(m) = a.as_object_mut() {
            m.insert("dry_run".into(), json!(true));
        }
        Ok(Request::new(kind.id(), a))
    }

    /// The confirmed writing request for the arguments a preview ran with (`dry_run`
    /// dropped). Called only by the confirm box's button, once per click: the one place
    /// this crate builds a confirmed `deploy` or `deploy.rollback` (gate test).
    pub fn confirmed_request(kind: WriteKind, previewed: &Value) -> Request {
        let mut a = previewed.clone();
        if let Some(m) = a.as_object_mut() {
            m.remove("dry_run");
        }
        match kind {
            WriteKind::Deploy => Request::confirmed("deploy", a),
            WriteKind::Rollback => Request::confirmed("deploy.rollback", a),
        }
    }

    /// `deploy.status` for the source's dev mod (read-only).
    pub fn status_request(&self) -> Result<Request, String> {
        let mut a = Map::new();
        let mo2 = self.mo2.trim();
        if mo2.is_empty() {
            return Err("no MO2 instance: load the dev profile".to_string());
        }
        a.insert("mo2".into(), json!(mo2));
        let m = self.mod_name();
        if !m.is_empty() {
            a.insert("mod".into(), json!(m));
        }
        if !self.profile.trim().is_empty() {
            a.insert("profile".into(), json!(self.profile.trim()));
        }
        Ok(Request::new("deploy.status", Value::Object(a)))
    }
}

/// A `deploy` or `deploy.rollback` result: the plan of a dry run, or what was written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeployPlan {
    /// The arguments it ran with (a preview's are what its confirm sends).
    pub args: Value,
    /// `dry_run` was true.
    pub dry_run: bool,
    /// `data.passed`.
    pub passed: Option<bool>,
    /// `plan`, `refused`, `unchanged`, `deployed`, `rolled back`.
    pub outcome: String,
    /// The dev mod.
    pub mod_name: String,
    /// Its folder.
    pub target: String,
    /// The deployment written or restored.
    pub deployment_id: String,
    /// `action path [-> to]`, each path the dry run would touch.
    pub would_touch: Vec<String>,
    /// Each path written.
    pub touched: Vec<String>,
    /// Why the engine refuses.
    pub refusals: Vec<String>,
    /// Warnings.
    pub warnings: Vec<String>,
    /// `files: 1 added, 0 replaced, ...` (deploy).
    pub files: String,
    /// The plugin's SHA-256 as the engine hashed the source (deploy).
    pub plugin_sha256: Option<String>,
    /// MO2's active profile, as the engine read it.
    pub active_profile: Option<String>,
}

fn touched_lines(v: &Value, key: &str) -> Vec<String> {
    list(v, key)
        .iter()
        .map(|t| {
            let mut s = format!(
                "{} {}",
                text_or_empty(t, "action"),
                text_or_empty(t, "path")
            );
            if let Some(to) = text(t, "to") {
                s.push_str(&format!(" -> {to}"));
            }
            s
        })
        .collect()
}

impl DeployPlan {
    /// Reads a `deploy` or `deploy.rollback` envelope run with `args`.
    pub fn from_envelope(env: &Value, args: &Value) -> Self {
        let d = env.get("data").unwrap_or(&Value::Null);
        let plugin = text_or_empty(d, "plugin");
        let files = d.get("files").unwrap_or(&Value::Null);
        let count = |k: &str| list(files, k).len();
        let plugin_sha256 = ["added", "replaced", "unchanged"]
            .iter()
            .flat_map(|k| list(files, k))
            .find(|f| !plugin.is_empty() && text_or_empty(f, "path").eq_ignore_ascii_case(&plugin))
            .and_then(|f| text(f, "sha256"));
        Self {
            args: args.clone(),
            dry_run: d.get("dry_run").and_then(Value::as_bool).unwrap_or(false),
            passed: d.get("passed").and_then(Value::as_bool),
            outcome: text_or_empty(d, "outcome"),
            mod_name: text_or_empty(d, "mod"),
            target: text_or_empty(d, "target"),
            deployment_id: text_or_empty(d, "deployment_id"),
            would_touch: touched_lines(d, "would_touch"),
            touched: touched_lines(d, "touched"),
            refusals: list(d, "refusals")
                .iter()
                .map(|r| {
                    format!(
                        "[{}] {}",
                        text_or_empty(r, "rule"),
                        text_or_empty(r, "message")
                    )
                })
                .collect(),
            warnings: strings(d, "warnings"),
            files: if files.is_object() {
                format!(
                    "files: {} added, {} replaced, {} removed, {} unchanged",
                    count("added"),
                    count("replaced"),
                    count("removed"),
                    count("unchanged")
                )
            } else {
                String::new()
            },
            plugin_sha256,
            active_profile: text(d, "active_profile"),
        }
    }

    /// A dry run that would write something.
    pub fn can_confirm(&self) -> bool {
        self.dry_run && self.passed == Some(true) && self.outcome == "plan"
    }
}

/// Why the confirm box for `kind` stays closed, or `Ok` when it may open. It opens only
/// for a dev profile that is loaded and not `Default`, a form whose `mo2` and `profile`
/// are that profile's, and a passing dry run of exactly the arguments the confirm would
/// send.
pub fn confirm_gate(
    kind: WriteKind,
    form: &DeployForm,
    target: &Result<DevTarget, String>,
    preview: Option<&DeployPlan>,
    to: Option<&str>,
) -> Result<Value, String> {
    let target = target.as_ref().map_err(Clone::clone)?;
    if !same_path(&form.mo2, &target.mo2) {
        return Err(format!(
            "`mo2` ({}) is not the loaded profile's instance ({}): the confirm box stays closed",
            form.mo2.trim(),
            target.mo2
        ));
    }
    if !form
        .profile
        .trim()
        .eq_ignore_ascii_case(target.profile.trim())
    {
        return Err(format!(
            "`profile` ({}) is not the loaded profile ({}): the confirm box stays closed",
            form.profile.trim(),
            target.profile
        ));
    }
    let args = form.args(kind, to)?;
    let Some(plan) = preview else {
        return Err("preview first: the dry run lists every path it would touch".to_string());
    };
    let mut previewed = plan.args.clone();
    if let Some(m) = previewed.as_object_mut() {
        m.remove("dry_run");
    }
    if previewed != args || !plan.dry_run {
        return Err("the form changed since the preview: preview again".to_string());
    }
    if !plan.can_confirm() {
        return Err(match plan.outcome.as_str() {
            "unchanged" => "the dry run says nothing would change".to_string(),
            _ => format!(
                "the dry run did not pass ({}): nothing to confirm",
                if plan.outcome.is_empty() {
                    "no outcome"
                } else {
                    &plan.outcome
                }
            ),
        });
    }
    Ok(plan.args.clone())
}

/// One dev mod in a `deploy.status` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevMod {
    /// `KitKraft Dev - <plugin>`.
    pub name: String,
    /// `ok` or `error`.
    pub state: String,
    /// `managed`, `absent`, `not-kitkraft`, `error`.
    pub live: String,
    /// The live deployment's id.
    pub live_id: String,
    /// When it was deployed.
    pub live_at: String,
    /// Its integrity: `clean`, or what changed.
    pub integrity: String,
    /// The `.previous` slot's state.
    pub previous: String,
    /// The `.previous` deployment's id: what a rollback restores (`to`).
    pub previous_id: String,
    /// An interrupted operation is pending.
    pub pending: bool,
    /// A lock is held.
    pub locked: bool,
    /// The profile's view of it, compact.
    pub profile: String,
}

/// A `deploy.status` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeployStatus {
    /// The instance.
    pub mo2: String,
    /// MO2's active profile.
    pub active_profile: Option<String>,
    /// The dev mods.
    pub mods: Vec<DevMod>,
    /// Warnings.
    pub warnings: Vec<String>,
}

impl DeployStatus {
    /// Reads a `deploy.status` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let d = env.get("data").unwrap_or(&Value::Null);
        let mods = list(d, "mods")
            .iter()
            .map(|m| {
                let live = m.get("live").unwrap_or(&Value::Null);
                let prev = m.get("previous").unwrap_or(&Value::Null);
                let id = |v: &Value| {
                    v.pointer("/manifest/deployment_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                let integrity = match live.get("integrity") {
                    Some(i) if i.get("clean") == Some(&Value::Bool(true)) => "clean".to_string(),
                    Some(Value::Null) | None => String::new(),
                    Some(i) => short(i),
                };
                DevMod {
                    name: text_or_empty(m, "mod"),
                    state: text_or_empty(m, "state"),
                    live: text_or_empty(live, "state"),
                    live_id: id(live),
                    live_at: live
                        .pointer("/manifest/deployed_at")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    integrity,
                    previous: text_or_empty(prev, "state"),
                    previous_id: id(prev),
                    pending: m.get("pending_journal").is_some_and(|j| !j.is_null()),
                    locked: m.get("lock").is_some_and(|l| !l.is_null()),
                    profile: match m.get("profile") {
                        Some(Value::Null) | None => String::new(),
                        Some(p) => short(p),
                    },
                }
            })
            .collect();
        Self {
            mo2: text_or_empty(d, "mo2"),
            active_profile: text(d, "active_profile"),
            mods,
            warnings: strings(d, "warnings"),
        }
    }

    /// The dev mod for plugin stem `stem` (`KitKraft Dev - <stem>`).
    pub fn find(&self, stem: &str) -> Option<&DevMod> {
        let want = format!("KitKraft Dev - {stem}");
        self.mods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(&want))
    }
}

// ---- the review sequence (M6.G8) ---------------------------------------------------------

/// The review's steps, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `validate`.
    Validate,
    /// `roundtrip --verify`.
    Roundtrip,
    /// `conflicts --plugin` or a `diff` per master.
    Masters,
    /// `deploy --dev --dry-run`.
    DeployPreview,
    /// The ledger entries naming the plugin.
    Ledger,
}

/// How the plugin is compared with its masters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MastersRoute {
    /// The plugin is in the loaded order: `conflicts --plugin <name>`.
    Conflicts(String),
    /// Not loaded: a `diff` of each master (a) against the plugin (b); the session's
    /// load order is left as it is.
    Diffs,
}

/// `validate` and `roundtrip --verify` for `plugin`: the review's first two requests.
pub fn review_requests(plugin: &str) -> Result<Vec<Request>, String> {
    let plugin = plugin.trim();
    if plugin.is_empty() {
        return Err("pick the plugin under review first".to_string());
    }
    Ok(vec![
        Request::new("validate", json!({"plugin": plugin})),
        Request::new("roundtrip", json!({"plugin": plugin, "verify": true})),
    ])
}

/// Whether `conflicts` takes a `detail` argument (lane D's M6.P5b): read from the
/// catalog's schema, so the panel asks for per-field rows once the engine offers them.
pub fn conflicts_has_detail() -> bool {
    gate::find_command("conflicts")
        .and_then(|s| serde_json::from_str::<Value>(s.args_schema).ok())
        .is_some_and(|s| s.pointer("/properties/detail").is_some())
}

/// The masters step's requests: `conflicts --plugin` (with `detail: fields` when the
/// engine has it) when the plugin is loaded, else one `diff` per master `validate`
/// found on disk. The masters it could not find are returned beside.
pub fn masters_requests(
    plugin: &str,
    route: &MastersRoute,
    masters: &[(String, Option<String>)],
) -> (Vec<Request>, Vec<String>) {
    match route {
        MastersRoute::Conflicts(name) => {
            let mut a = Map::new();
            a.insert("plugin".into(), json!(name));
            a.insert("limit".into(), json!(50));
            if conflicts_has_detail() {
                a.insert("detail".into(), json!("fields"));
            }
            (
                vec![Request::new("conflicts", Value::Object(a))],
                Vec::new(),
            )
        }
        MastersRoute::Diffs => {
            let mut reqs = Vec::new();
            let mut missing = Vec::new();
            for (name, path) in masters {
                match path {
                    Some(p) => reqs.push(Request::new(
                        "diff",
                        json!({"a": p, "b": plugin.trim(), "limit": 50}),
                    )),
                    None => missing.push(name.clone()),
                }
            }
            (reqs, missing)
        }
    }
}

/// The masters `validate` listed, as (name, path on disk when found).
pub fn masters_of(validate_env: &Value) -> Vec<(String, Option<String>)> {
    let d = validate_env.get("data").unwrap_or(&Value::Null);
    list(d, "masters")
        .iter()
        .map(|m| (text_or_empty(m, "name"), text(m, "path")))
        .collect()
}

/// The six statuses of AGENTS.md "Reporting", filled from what the review ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewSummary {
    /// (row, status word, why), in the YAML block's order.
    pub rows: Vec<(&'static str, &'static str, String)>,
}

/// The summary: `parse` from `validate` (it parsed the plugin and its masters) and
/// `roundtrip`, `byte_roundtrip` from `roundtrip --verify`; `ck_open`, `ck_reexport`,
/// `game_runtime` and `save_reload` are `not-run`: the cockpit runs no CK, and the game
/// session is M6.G4.
pub fn review_summary(
    validate: Option<Result<&ValidateView, &str>>,
    roundtrip: Option<Result<&RoundtripView, &str>>,
) -> ReviewSummary {
    let parse = match (validate, roundtrip) {
        (Some(Err(e)), _) => ("fail", format!("validate failed: {e}")),
        (_, Some(Err(e))) => ("fail", format!("roundtrip failed: {e}")),
        (Some(Ok(_)), Some(Ok(_))) => ("pass", "validate and roundtrip read every record".into()),
        (Some(Ok(_)), None) | (None, Some(Ok(_))) => (
            "pass",
            "read by the step that ran; the other has not".into(),
        ),
        (None, None) => ("not-run", "run the review".into()),
    };
    let byte = match roundtrip {
        Some(Ok(r)) => match r.passed {
            Some(true) => ("pass", format!("byte-identical ({})", r.counts)),
            Some(false) => (
                "fail",
                r.mismatch.clone().unwrap_or_else(|| "mismatch".into()),
            ),
            None => ("inconclusive", "the result has no `passed`".into()),
        },
        Some(Err(e)) => ("fail", e.to_string()),
        None => ("not-run", "run the review".into()),
    };
    ReviewSummary {
        rows: vec![
            ("parse", parse.0, parse.1),
            ("byte_roundtrip", byte.0, byte.1),
            (
                "ck_open",
                "not-run",
                "the cockpit runs no Creation Kit (owner sittings, docs/ck-observations.md)".into(),
            ),
            ("ck_reexport", "not-run", "as ck_open".into()),
            (
                "game_runtime",
                "not-run",
                "a game session (M6.G4) or an owner session records it".into(),
            ),
            ("save_reload", "not-run", "as game_runtime".into()),
        ],
    }
}

// ---- the ledger --------------------------------------------------------------------------

/// A ledger entry that names the plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerHit {
    /// The entry's heading (`game-20261015-02: gun overhaul build 2 ...`).
    pub entry: String,
    /// It names the SHA-256 (when one is known).
    pub names_hash: bool,
    /// The first line naming the file or the hash.
    pub line: String,
}

/// The entries of the ledger's `## Ledger` section (`### game-...`) that name `file`
/// (case-insensitive) or `sha256`. Text matching over the repo doc, as the Ledger tab
/// reads its templates.
pub fn ledger_hits(ledger: &str, file: &str, sha256: Option<&str>) -> Vec<LedgerHit> {
    let file = file.trim().to_ascii_lowercase();
    let hash = sha256.map(str::to_ascii_lowercase);
    if file.is_empty() && hash.is_none() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut in_ledger = false;
    let mut current: Option<(String, Vec<&str>)> = None;
    let flush = |cur: Option<(String, Vec<&str>)>, out: &mut Vec<LedgerHit>| {
        let Some((entry, lines)) = cur else { return };
        let names = |l: &str, n: &str| !n.is_empty() && l.to_ascii_lowercase().contains(n);
        let by_hash = hash.as_deref().map(|h| lines.iter().find(|l| names(l, h)));
        let by_file = lines.iter().find(|l| names(l, &file));
        let line = by_hash.flatten().or(by_file);
        if let Some(line) = line {
            out.push(LedgerHit {
                entry,
                names_hash: by_hash.flatten().is_some(),
                line: line.trim().to_string(),
            });
        }
    };
    for line in ledger.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            flush(current.take(), &mut out);
            in_ledger = h.trim() == "Ledger";
        } else if let Some(h) = line.strip_prefix("### ") {
            flush(current.take(), &mut out);
            if in_ledger {
                current = Some((h.trim().to_string(), Vec::new()));
            }
        } else if let Some((_, lines)) = &mut current {
            lines.push(line);
        }
    }
    flush(current.take(), &mut out);
    out
}

// ---- the game buttons (M6.G2) ------------------------------------------------------------

/// One action-bar button for a game command: on, or off with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonState {
    /// The button's label.
    pub label: &'static str,
    /// The command it runs.
    pub id: &'static str,
    /// Clickable.
    pub enabled: bool,
    /// Why not (the ticket that turns it on), or what it does.
    pub reason: String,
}

/// The game buttons: (label, command, the window ticket that wires it).
pub const GAME_BUTTONS: &[(&str, &str, &str)] = &[
    ("Launch and run probes", "game.session", "M6.G4"),
    ("Stop", "game.stop", "M6.G4"),
    ("Re-run probe", "game.probe", "M6.G5"),
    ("Screenshot", "game.probe", "M6.G5"),
];

/// A game button's state, read from the catalog (`implemented`: `Some(true)` when the
/// command is there and `Implemented`) and the gate (`permitted`): on only when both
/// hold. Off with "M4.T3" while the catalog lacks the command, else with the window
/// ticket that adds it to the gate's permitted writes.
pub fn game_button(
    (label, id, ticket): (&'static str, &'static str, &'static str),
    implemented: &dyn Fn(&str) -> Option<bool>,
    permitted: &dyn Fn(&str) -> bool,
) -> ButtonState {
    let (enabled, reason) = match implemented(id) {
        None | Some(false) => (
            false,
            format!("waits on M4.T3: `{id}` is not an Implemented catalog command"),
        ),
        Some(true) if !permitted(id) => (
            false,
            format!(
                "waits on {ticket}: `{id}` is in the catalog (M4.T3), but the window's gate does not permit it until {ticket} wires its preview and confirm"
            ),
        ),
        Some(true) => (true, format!("`{id}`: preview, then confirm")),
    };
    ButtonState {
        label,
        id,
        enabled,
        reason,
    }
}

/// The game buttons as the real catalog and gate have them.
pub fn game_buttons() -> Vec<ButtonState> {
    let implemented = |id: &str| gate::find_command(id).map(|s| s.status.label() == "implemented");
    let permitted = |id: &str| gate::permitted(id).is_some();
    GAME_BUTTONS
        .iter()
        .map(|b| game_button(*b, &implemented, &permitted))
        .collect()
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, CollapsingHeader, ComboBox, RichText, ScrollArea, TextEdit};

use crate::panels::common::{
    action_bar, envelope_block, findings_grid, panel_head, virtual_table, virtual_table_for,
    Action, Shown, Slot, SlotKey, TabCtx,
};
use crate::theme::{self, Field, Status, View};

/// What the shell hands the Cockpit besides the frame's context: the profile the
/// session loaded, the docs checkout, and the ledger's text.
pub(crate) struct CockpitEnv<'a> {
    pub loaded_profile: Option<&'a str>,
    pub checkout: Option<&'a Path>,
    pub ledger: Option<&'a str>,
}

/// The status component's status for a step or a `Reporting` word.
pub fn status_of(word: &str) -> Status {
    match word {
        "pass" => Status::Pass,
        "fail" => Status::Fail,
        "warning" | "warn" => Status::Warn,
        "not-run" => Status::NotRun,
        "inconclusive" => Status::Inconclusive,
        _ => Status::Unknown,
    }
}

/// The Cockpit tab's state.
#[derive(Default)]
pub(crate) struct CockpitTab {
    /// The plugin under review.
    pub plugin: String,
    /// Plugins found under `out\` (refreshed on demand).
    pub found: Vec<String>,
    pub found_in: Option<PathBuf>,
    pub form: DeployForm,
    /// The review's requests as submitted (to tell their replies, and failures, apart).
    pub submitted: Vec<(Slot, Request)>,
    pub route: Option<MastersRoute>,
    pub validate: Option<ValidateView>,
    pub roundtrip: Option<RoundtripView>,
    /// Each master's comparison, with the data key its table starts over on
    /// ([`virtual_table_for`]): a fresh one per reply, so a new review of the plugin
    /// starts every master's table at the top with fresh column widths.
    pub masters: Vec<(String, u64, DiffView)>,
    /// Replies the masters step has received, the source of those keys.
    pub masters_replies: u64,
    pub masters_missing: Vec<String>,
    pub status: Option<DeployStatus>,
    pub deploy: Option<DeployPlan>,
    pub rollback: Option<DeployPlan>,
    /// The confirm box open for this write and these previewed arguments.
    pub pending: Option<(WriteKind, Value)>,
}

impl CockpitTab {
    /// Picks `plugin` for review: forgets every result about the previous one.
    pub(crate) fn pick(&mut self, plugin: &str) {
        if self.plugin.trim() != plugin.trim() {
            self.plugin = plugin.trim().to_string();
        }
        self.form.source.clone_from(&self.plugin);
        self.submitted.clear();
        self.route = None;
        self.validate = None;
        self.roundtrip = None;
        self.masters.clear();
        self.masters_missing.clear();
        self.deploy = None;
        self.rollback = None;
        self.pending = None;
    }

    /// The review's first requests (M6.G8): `validate` and `roundtrip --verify`; the
    /// masters step follows `validate`'s reply. `loaded` are the session's plugin names.
    pub(crate) fn start_review(
        &mut self,
        loaded: &[(String, String)],
    ) -> Result<Vec<(Slot, Request)>, String> {
        let plugin = self.plugin.clone();
        self.pick(&plugin);
        let reqs = review_requests(&plugin)?;
        let name = file_name(&plugin);
        self.route = Some(
            if loaded.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                MastersRoute::Conflicts(name)
            } else {
                MastersRoute::Diffs
            },
        );
        let slots = [Slot::CockpitValidate, Slot::CockpitRoundtrip];
        let out: Vec<(Slot, Request)> = slots.into_iter().zip(reqs).collect();
        self.submitted.extend(out.iter().cloned());
        Ok(out)
    }

    /// A reply for one of the Cockpit's slots: the view it fills, and the requests that
    /// follow (the masters step after `validate`, `deploy.status` after a write).
    pub(crate) fn receive(
        &mut self,
        slot: Slot,
        request: &Request,
        env: &Value,
    ) -> Vec<(Slot, Request)> {
        let mut next = Vec::new();
        match slot {
            Slot::CockpitValidate => {
                if !self.submitted.iter().any(|(_, r)| r == request) {
                    return next;
                }
                self.validate = Some(ValidateView::from_envelope(env));
                if let Some(route) = &self.route {
                    let (reqs, missing) = masters_requests(&self.plugin, route, &masters_of(env));
                    self.masters_missing = missing;
                    for r in reqs {
                        self.submitted.push((Slot::CockpitMasters, r.clone()));
                        next.push((Slot::CockpitMasters, r));
                    }
                }
            }
            Slot::CockpitRoundtrip => {
                if self.submitted.iter().any(|(_, r)| r == request) {
                    self.roundtrip = Some(RoundtripView::from_envelope(env));
                }
            }
            Slot::CockpitMasters => {
                if self.submitted.iter().any(|(_, r)| r == request) {
                    let label = match request.id.as_str() {
                        "diff" => text_or_empty(&request.args, "a"),
                        _ => "the loaded order".to_string(),
                    };
                    let name = file_name(&label);
                    let shown = if name.is_empty() { label } else { name };
                    self.masters_replies = self.masters_replies.wrapping_add(1);
                    self.masters
                        .push((shown, self.masters_replies, DiffView::from_envelope(env)));
                }
            }
            Slot::CockpitStatus => self.status = Some(DeployStatus::from_envelope(env)),
            Slot::CockpitDeploy | Slot::CockpitRollback => {
                let plan = DeployPlan::from_envelope(env, &request.args);
                let wrote = !plan.dry_run;
                if slot == Slot::CockpitDeploy {
                    self.deploy = Some(plan);
                } else {
                    self.rollback = Some(plan);
                }
                if wrote {
                    // What was written is read back at once.
                    if let Ok(r) = self.form.status_request() {
                        next.push((Slot::CockpitStatus, r));
                    }
                }
            }
            _ => {}
        }
        next
    }

    /// A review step's status: from its view, or the error its reply carried.
    fn step(
        &self,
        step: Step,
        results: &std::collections::BTreeMap<SlotKey, Shown>,
    ) -> (Status, String) {
        let failed = |slot: Slot| -> Option<String> {
            results.get(&slot.into()).and_then(|s| {
                let ours = self
                    .submitted
                    .iter()
                    .any(|(sl, r)| *sl == slot && *r == s.request);
                s.error.as_ref().filter(|_| ours).map(ToString::to_string)
            })
        };
        let sent = |slot: Slot| self.submitted.iter().any(|(s, _)| *s == slot);
        match step {
            Step::Validate => match (&self.validate, failed(Slot::CockpitValidate)) {
                (_, Some(e)) => (Status::Fail, e),
                (Some(v), _) if v.passed == Some(true) => (
                    Status::Pass,
                    format!("{} warning(s); {}", v.warnings, v.checked_line),
                ),
                (Some(v), _) => (
                    Status::Fail,
                    format!("{} error(s), {} warning(s)", v.errors, v.warnings),
                ),
                (None, None) if sent(Slot::CockpitValidate) => (Status::NotRun, "running".into()),
                (None, None) => (Status::NotRun, "not run".into()),
            },
            Step::Roundtrip => match (&self.roundtrip, failed(Slot::CockpitRoundtrip)) {
                (_, Some(e)) => (Status::Fail, e),
                (Some(r), _) => match r.passed {
                    Some(true) => (Status::Pass, format!("byte-identical; {}", r.counts)),
                    Some(false) => (
                        Status::Fail,
                        r.mismatch.clone().unwrap_or_else(|| "mismatch".into()),
                    ),
                    None => (Status::Inconclusive, "no `passed` in the result".into()),
                },
                (None, None) if sent(Slot::CockpitRoundtrip) => (Status::NotRun, "running".into()),
                (None, None) => (Status::NotRun, "not run".into()),
            },
            Step::Masters => {
                let asked = self
                    .submitted
                    .iter()
                    .filter(|(s, _)| *s == Slot::CockpitMasters)
                    .count();
                if let Some(e) = failed(Slot::CockpitMasters) {
                    (Status::Fail, e)
                } else if asked == 0 && self.validate.is_some() {
                    (
                        if self.masters_missing.is_empty() {
                            Status::Pass
                        } else {
                            Status::Inconclusive
                        },
                        if self.masters_missing.is_empty() {
                            "no masters".to_string()
                        } else {
                            format!("masters not found: {}", self.masters_missing.join(", "))
                        },
                    )
                } else if asked == 0 {
                    (Status::NotRun, "after validate".into())
                } else if self.masters.len() < asked {
                    (
                        Status::NotRun,
                        format!("running ({} of {asked})", self.masters.len()),
                    )
                } else if !self.masters_missing.is_empty() {
                    (
                        Status::Inconclusive,
                        format!("masters not found: {}", self.masters_missing.join(", ")),
                    )
                } else {
                    (Status::Pass, format!("{asked} comparison(s) read"))
                }
            }
            Step::DeployPreview => match &self.deploy {
                Some(p) if p.dry_run && p.can_confirm() => {
                    (Status::Pass, format!("would deploy: {}", p.files))
                }
                Some(p) if p.dry_run && p.outcome == "unchanged" => {
                    (Status::Pass, "already deployed with these files".into())
                }
                Some(p) if p.dry_run => {
                    (Status::Fail, format!("refused: {}", p.refusals.join("; ")))
                }
                Some(p) => (Status::Pass, format!("{} {}", p.outcome, p.deployment_id)),
                None => (Status::NotRun, "Preview deploy (below)".into()),
            },
            Step::Ledger => (Status::NotRun, String::new()),
        }
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>, env: &CockpitEnv<'_>) {
        let busy = ctx.busy;
        panel_head(
            ui,
            "Cockpit",
            "The plugin under review: its validate summary, byte roundtrip and its records against its masters (reads), its dev deployment (deploy --dev and roll back, each previewed and confirmed per click: ADR 0007 Amendment 3), and the ledger entries that name it. Every value comes from an engine command.",
        );

        // -- the action bar: the plugin under review and the game session's buttons
        if self.found_in.is_none() {
            if let Some(root) = env.checkout {
                let out = root.join(OUT_DIR);
                self.found = plugins_under(&out);
                self.found_in = Some(out);
            }
        }
        action_bar(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::subheading(ui, "Plugin under review"));
                let edit = ui.add(
                    TextEdit::singleline(&mut self.plugin)
                        .desired_width(theme::width(ui, Field::Long))
                        .hint_text(r"out\built\MyPlugin.esm, or any path"),
                );
                if edit.lost_focus() {
                    self.form.source.clone_from(&self.plugin);
                }
                let mut picked = None;
                ComboBox::from_id_salt("cockpit-out")
                    .selected_text(format!("{OUT_DIR}\\... ({})", self.found.len()))
                    .width(theme::combo_width(ui))
                    .show_ui(ui, |ui| {
                        let base = self.found_in.clone().unwrap_or_default();
                        for p in &self.found {
                            let shown = Path::new(p)
                                .strip_prefix(&base)
                                .map_or_else(|_| p.clone(), |r| r.display().to_string());
                            if ui.selectable_label(*p == self.plugin, shown).clicked() {
                                picked = Some(p.clone());
                            }
                        }
                    });
                if let Some(p) = picked {
                    self.pick(&p);
                }
                if ui
                    .small_button("rescan")
                    .on_hover_text("list the plugins under the checkout's out folder again")
                    .clicked()
                {
                    self.found_in = None;
                }
                if ui
                .add_enabled(
                    !busy && !self.plugin.trim().is_empty(),
                    egui::Button::new("Review"),
                )
                .on_hover_text(
                    "validate, roundtrip --verify, then the plugin against its masters: reads only",
                )
                .clicked()
            {
                match self.start_review(ctx.plugins) {
                    Ok(reqs) => ctx
                        .actions
                        .extend(reqs.into_iter().map(|(s, r)| Action::Submit(s, r))),
                    Err(e) => *ctx.notice = Some(e),
                }
            }
            });
            if env.checkout.is_none() {
                ui.weak("no checkout found (Help tab): out\\ is not listed; type a path");
            }
            ui.separator();
            // the game buttons (M6.G4 to M6.G6)
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::subheading(ui, "Game session"));
                for b in game_buttons() {
                    let r = ui
                        .add_enabled(b.enabled && !busy, egui::Button::new(b.label))
                        .on_hover_text(&b.reason)
                        .on_disabled_hover_text(&b.reason);
                    if r.clicked() {
                        *ctx.notice = Some(format!("{}: not wired in this window yet", b.id));
                    }
                }
            });
            let mut reasons: Vec<String> = Vec::new();
            for b in game_buttons().into_iter().filter(|b| !b.enabled) {
                if !reasons.contains(&b.reason) {
                    reasons.push(b.reason);
                }
            }
            for r in reasons {
                ui.weak(r);
            }
            ui.weak("no session started: the window starts one with game.session --start and follows it with game.status (M6.G4)");
        });

        // -- the review summary
        let name = file_name(&self.plugin);
        let hash = self
            .deploy
            .as_ref()
            .and_then(|d| d.plugin_sha256.clone())
            .filter(|_| self.form.source.trim() == self.plugin.trim());
        let hits = env
            .ledger
            .map(|l| ledger_hits(l, &name, hash.as_deref()))
            .unwrap_or_default();
        ui.label(theme::subheading(ui, "Review"));
        theme::table(ui, "cockpit-steps").show(ui, |ui| {
            for h in ["step", "status", "detail"] {
                ui.strong(h);
            }
            ui.end_row();
            for (step, label) in [
                (Step::Validate, "validate"),
                (Step::Roundtrip, "roundtrip --verify"),
                (Step::Masters, "against its masters"),
                (Step::DeployPreview, "deploy --dev (dry run)"),
            ] {
                let (st, detail) = self.step(step, ctx.results);
                ui.label(label);
                let word = if step == Step::DeployPreview && st == Status::Fail {
                    "refused"
                } else {
                    ""
                };
                theme::status(ui, st, word);
                ui.label(detail);
                ui.end_row();
            }
            ui.label("ledger");
            match (env.ledger, hits.is_empty()) {
                (None, _) => {
                    theme::status(ui, Status::Unknown, "");
                    ui.label("the ledger was not read (no checkout; Help tab)");
                }
                (Some(_), true) => {
                    theme::status(ui, Status::NotRun, "");
                    ui.label(format!("no ledger entry names {name}"));
                }
                (Some(_), false) => {
                    theme::status(ui, Status::Pass, "found");
                    ui.label(format!("{} entr(ies) name it (below)", hits.len()));
                }
            }
            ui.end_row();
        });
        let summary = review_summary(
            match (&self.validate, self.step(Step::Validate, ctx.results)) {
                (Some(v), _) => Some(Ok(v)),
                (None, (Status::Fail, _)) => Some(Err("the engine refused or failed")),
                _ => None,
            },
            match (&self.roundtrip, self.step(Step::Roundtrip, ctx.results)) {
                (Some(r), _) => Some(Ok(r)),
                (None, (Status::Fail, _)) => Some(Err("the engine refused or failed")),
                _ => None,
            },
        );
        CollapsingHeader::new("validation (AGENTS.md \"Reporting\")")
            .id_salt("cockpit-yaml")
            .default_open(true)
            .show(ui, |ui| {
                theme::table(ui, "cockpit-yaml-grid").show(ui, |ui| {
                    for (row, word, why) in &summary.rows {
                        ui.monospace(*row);
                        theme::status(ui, status_of(word), word);
                        ui.label(why);
                        ui.end_row();
                    }
                });
            });
        if let Some(v) = &self.validate {
            if v.findings_total > 0 {
                CollapsingHeader::new(format!("validate findings ({})", v.findings_total))
                    .id_salt("cockpit-findings")
                    .show(ui, |ui| {
                        findings_grid(
                            ui,
                            "cockpit",
                            v.groups.iter().flat_map(|g| g.findings.iter()),
                        );
                    });
            }
        }
        if !self.masters.is_empty() || !self.masters_missing.is_empty() {
            CollapsingHeader::new("against its masters")
                .id_salt("cockpit-masters")
                .default_open(true)
                .show(ui, |ui| {
                    for m in &self.masters_missing {
                        theme::status(
                            ui,
                            Status::Inconclusive,
                            &format!("{m}: not found on disk; not compared"),
                        );
                    }
                    for (master, data, view) in &self.masters {
                        ui.label(RichText::new(master).strong());
                        ui.label(view.counts.join(", "));
                        ui.weak("removed: the master's records the plugin does not carry (expected for a plugin over a master); added: its new records; changed: its overrides");
                        if !view.added.is_empty() {
                            ui.weak(format!(
                                "new records ({}): {}",
                                view.added.len(),
                                view.added
                                    .iter()
                                    .take(20)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ));
                        }
                        // Every changed record, laid out only where scrolled into view.
                        virtual_table_for(
                            ui,
                            ("cockpit-master", self.plugin.as_str(), master.as_str()),
                            *data,
                            &["record", "class", "sides", "fields"],
                            view.records.len(),
                            View::Medium,
                            |row, i| {
                                let Some(r) = view.records.get(i) else {
                                    return;
                                };
                                row.cell(|ui| {
                                    if ui
                                        .add_enabled(
                                            !busy,
                                            egui::Link::new(format!("{} {}", r.signature, r.form)),
                                        )
                                        .on_hover_text("inspect")
                                        .clicked()
                                    {
                                        ctx.actions.push(Action::Inspect(r.form.clone()));
                                    }
                                });
                                row.cell(|ui| ui.label(&r.class));
                                let sides: Vec<String> = r
                                    .sides
                                    .split(" -> ")
                                    .map(|p| {
                                        let n = file_name(p);
                                        if n.is_empty() {
                                            p.to_string()
                                        } else {
                                            n
                                        }
                                    })
                                    .collect();
                                row.cell(|ui| ui.label(sides.join(" -> ")).on_hover_text(&r.sides));
                                let all: Vec<String> = r
                                    .rows
                                    .iter()
                                    .map(|f| format!("{}: {} -> {}", f.path, f.before, f.after))
                                    .collect();
                                let first = all.first().cloned().unwrap_or_default();
                                let more = all.len().saturating_sub(1);
                                let shown = if more > 0 {
                                    format!("{first} (+{more} more, hover)")
                                } else {
                                    first
                                };
                                row.cell(|ui| ui.label(shown).on_hover_text(all.join("\n")));
                            },
                        );
                    }
                });
        }
        theme::gap(ui);

        // -- dev deployment
        self.deploy_ui(ui, ctx, env);
        theme::gap(ui);

        // -- the ledger
        ui.label(theme::subheading(ui, "Ledger entries naming this plugin"));
        match &hash {
            Some(h) => {
                ui.weak(format!(
                    "SHA-256 {h} (from the deploy preview: the engine hashed the source)"
                ));
            }
            None => {
                ui.weak("SHA-256 unknown until Preview deploy (the engine hashes the source); entries are matched by file name");
            }
        }
        if hits.is_empty() {
            ui.weak("none");
        } else {
            virtual_table(
                ui,
                "cockpit-ledger",
                &["entry", "names", "line"],
                hits.len(),
                View::Medium,
                |row, i| {
                    let Some(hit) = hits.get(i) else { return };
                    row.cell(|ui| ui.label(&hit.entry));
                    row.cell(|ui| {
                        if hit.names_hash {
                            theme::status(ui, Status::Pass, "this SHA-256")
                        } else if hash.is_some() {
                            theme::status(ui, Status::Warn, "the file name only")
                        } else {
                            theme::status(ui, Status::Unknown, "the file name")
                        }
                    });
                    row.cell(|ui| ui.label(&hit.line));
                },
            );
        }
    }

    fn deploy_ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>, env: &CockpitEnv<'_>) {
        let busy = ctx.busy;
        ui.label(theme::subheading(
            ui,
            "Dev deployment (deploy --dev, ADR 0009)",
        ));
        let target = dev_target(env.loaded_profile);
        match &target {
            Ok(t) => {
                theme::status(
                    ui,
                    Status::Pass,
                    &format!("target: profile `{}` of {}", t.profile, t.mo2),
                );
                if self.form.mo2.trim().is_empty() && self.form.profile.trim().is_empty() {
                    self.form.fill(t);
                }
            }
            Err(e) => {
                theme::status_line(ui, Status::Warn, e);
            }
        }
        if self.form.source.trim().is_empty() {
            self.form.source.clone_from(&self.plugin);
        }
        let wide = theme::width(ui, Field::Long);
        theme::form_grid(ui, "cockpit-deploy-form").show(ui, |ui| {
            ui.label("source");
            ui.add(TextEdit::singleline(&mut self.form.source).desired_width(wide));
            ui.end_row();
            ui.label("MO2 instance");
            ui.add(TextEdit::singleline(&mut self.form.mo2).desired_width(wide));
            ui.end_row();
            ui.label("profile");
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.form.profile)
                        .desired_width(theme::width(ui, Field::Medium)),
                );
                if let Ok(t) = &target {
                    if ui
                        .small_button("use the loaded profile")
                        .on_hover_text("fill the instance and profile from the session's load")
                        .clicked()
                    {
                        self.form.fill(t);
                    }
                }
            });
            ui.end_row();
        });
        let dev_mod = self
            .status
            .as_ref()
            .and_then(|s| s.find(&self.form.mod_name()))
            .cloned();
        let to = dev_mod
            .as_ref()
            .map(|m| m.previous_id.clone())
            .filter(|t| !t.is_empty());
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Status"))
                .on_hover_text("deploy.status: the dev mod, its live and previous deployments (read-only)")
                .clicked()
            {
                match self.form.status_request() {
                    Ok(r) => ctx.actions.push(Action::Submit(Slot::CockpitStatus, r)),
                    Err(e) => *ctx.notice = Some(e),
                }
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Preview deploy"))
                .on_hover_text("deploy --dev --dry-run: the plan and every path it would touch; writes nothing")
                .clicked()
            {
                self.pending = None;
                match self.form.preview_request(WriteKind::Deploy, None) {
                    Ok(r) => ctx.actions.push(Action::Submit(Slot::CockpitDeploy, r)),
                    Err(e) => *ctx.notice = Some(e),
                }
            }
            let gate_d = confirm_gate(WriteKind::Deploy, &self.form, &target, self.deploy.as_ref(), None);
            let open_d = ui
                .add_enabled(
                    !busy && self.pending.is_none() && gate_d.is_ok(),
                    egui::Button::new("Deploy..."),
                )
                .on_disabled_hover_text(gate_d.as_ref().err().cloned().unwrap_or_default());
            if open_d.clicked() {
                if let Ok(args) = gate_d {
                    self.pending = Some((WriteKind::Deploy, args));
                }
            }
            ui.separator();
            if ui
                .add_enabled(!busy && dev_mod.is_some(), egui::Button::new("Preview roll back"))
                .on_hover_text("deploy.rollback --dry-run: swap the .previous deployment back in; writes nothing")
                .on_disabled_hover_text("read the dev mod's Status first: roll back needs its .previous deployment")
                .clicked()
            {
                self.pending = None;
                match self.form.preview_request(WriteKind::Rollback, to.as_deref()) {
                    Ok(r) => ctx.actions.push(Action::Submit(Slot::CockpitRollback, r)),
                    Err(e) => *ctx.notice = Some(e),
                }
            }
            let gate_r = confirm_gate(
                WriteKind::Rollback,
                &self.form,
                &target,
                self.rollback.as_ref(),
                to.as_deref(),
            );
            let open_r = ui
                .add_enabled(
                    !busy && self.pending.is_none() && gate_r.is_ok(),
                    egui::Button::new("Roll back..."),
                )
                .on_disabled_hover_text(gate_r.as_ref().err().cloned().unwrap_or_default());
            if open_r.clicked() {
                if let Ok(args) = gate_r {
                    self.pending = Some((WriteKind::Rollback, args));
                }
            }
        });
        for (kind, plan) in [
            (WriteKind::Deploy, &self.deploy),
            (WriteKind::Rollback, &self.rollback),
        ] {
            let Some(p) = plan else { continue };
            if self.pending.as_ref().is_some_and(|(k, _)| *k == kind) {
                continue;
            }
            if let Err(why) = confirm_gate(
                kind,
                &self.form,
                &target,
                Some(p),
                to.as_deref().filter(|_| kind == WriteKind::Rollback),
            ) {
                if p.dry_run {
                    theme::status_line(
                        ui,
                        Status::NotRun,
                        &format!("{}: confirm closed: {why}", kind.id()),
                    );
                }
            }
        }

        // The confirm box: what will be written, then the one click.
        let mut close = false;
        if let Some((kind, args)) = &self.pending {
            let plan = match kind {
                WriteKind::Deploy => self.deploy.as_ref(),
                WriteKind::Rollback => self.rollback.as_ref(),
            };
            // Re-checked every frame: a changed form or a new load closes it.
            let still = confirm_gate(
                *kind,
                &self.form,
                &target,
                plan,
                to.as_deref().filter(|_| *kind == WriteKind::Rollback),
            );
            if still.as_ref().ok() != Some(args) {
                close = true;
            } else {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    theme::status_line(
                        ui,
                        Status::Warn,
                        &format!(
                            "{} into {}? It writes these paths (the dry run's list), and nothing else; profiles, plugins.txt and other mods are never touched.",
                            match kind {
                                WriteKind::Deploy => "Deploy",
                                WriteKind::Rollback => "Roll back",
                            },
                            self.form.mo2.trim()
                        ),
                    );
                    if let Some(p) = plan {
                        ScrollArea::vertical()
                            .id_salt("cockpit-confirm-paths")
                            .max_height(theme::view_height(ui, View::Short))
                            .show(ui, |ui| {
                                for l in &p.would_touch {
                                    ui.monospace(l);
                                }
                            });
                    }
                    ui.horizontal(|ui| {
                        let label = match kind {
                            WriteKind::Deploy => "Confirm: deploy",
                            WriteKind::Rollback => "Confirm: roll back",
                        };
                        if ui
                            .add_enabled(!busy, egui::Button::new(RichText::new(label).strong()))
                            .clicked()
                        {
                            let slot = match kind {
                                WriteKind::Deploy => Slot::CockpitDeploy,
                                WriteKind::Rollback => Slot::CockpitRollback,
                            };
                            ctx.actions
                                .push(Action::Submit(slot, DeployForm::confirmed_request(*kind, args)));
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
        }
        if close {
            self.pending = None;
        }

        // The dev mod as deploy.status read it.
        if let Some(s) = &self.status {
            for w in &s.warnings {
                theme::status_line(ui, Status::Warn, w);
            }
            match &dev_mod {
                Some(m) => {
                    theme::table(ui, "cockpit-devmod").show(ui, |ui| {
                        for (k, v) in [
                            ("dev mod", m.name.clone()),
                            ("live", format!("{} {} {}", m.live, m.live_id, m.live_at)),
                            ("integrity", m.integrity.clone()),
                            ("previous", format!("{} {}", m.previous, m.previous_id)),
                            (
                                "pending operation",
                                if m.pending {
                                    "yes: the next deploy or rollback recovers it".into()
                                } else {
                                    "none".into()
                                },
                            ),
                            (
                                "lock",
                                if m.locked {
                                    "held".into()
                                } else {
                                    "none".into()
                                },
                            ),
                            ("profile", m.profile.clone()),
                        ] {
                            ui.label(k);
                            ui.label(v);
                            ui.end_row();
                        }
                    });
                }
                None => {
                    ui.weak(format!(
                        "no dev mod `KitKraft Dev - {}` in {} ({} dev mod(s) there)",
                        self.form.mod_name(),
                        s.mo2,
                        s.mods.len()
                    ));
                }
            }
        }
        for (title, plan, slot) in [
            ("deploy", &self.deploy, Slot::CockpitDeploy),
            ("roll back", &self.rollback, Slot::CockpitRollback),
        ] {
            let Some(p) = plan else { continue };
            CollapsingHeader::new(format!(
                "{title}: {}{}",
                if p.dry_run { "dry run, " } else { "" },
                if p.outcome.is_empty() {
                    "?"
                } else {
                    &p.outcome
                }
            ))
            .id_salt(("cockpit-plan", title))
            .default_open(true)
            .show(ui, |ui| {
                for r in &p.refusals {
                    theme::status_line(ui, Status::Fail, r);
                }
                for w in &p.warnings {
                    theme::status_line(ui, Status::Warn, w);
                }
                if !p.files.is_empty() {
                    ui.label(&p.files);
                }
                let (head, lines) = if p.dry_run {
                    ("would touch", &p.would_touch)
                } else {
                    ("touched", &p.touched)
                };
                ui.weak(format!("{head} ({}):", lines.len()));
                for l in lines.iter().take(200) {
                    ui.monospace(l);
                }
                envelope_block(ui, ctx.results.get(&slot.into()), |_| {});
            });
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
pub(crate) mod fake_mo2 {
    //! A throwaway MO2 instance in a temp folder (never the owner's), for the deploy
    //! tests: `ModOrganizer.ini`, `profiles\Default` and `profiles\KitKraft Dev` with
    //! their list files, an unrelated mod, `overwrite\`, `downloads\`, a game folder and
    //! a source folder.

    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    pub(crate) struct FakeMo2 {
        root: PathBuf,
    }

    impl FakeMo2 {
        pub(crate) fn new(tag: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("kitkraft-ui-mo2-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            for d in [
                "mo2/mods/Unrelated Mod",
                "mo2/profiles/Default",
                "mo2/profiles/KitKraft Dev",
                "mo2/overwrite",
                "mo2/downloads",
                "Game/Data",
                "src",
            ] {
                fs::create_dir_all(root.join(d)).unwrap();
            }
            let m = Self { root };
            m.select("KitKraft Dev");
            for p in ["Default", "KitKraft Dev"] {
                m.write(
                    &format!("mo2/profiles/{p}/modlist.txt"),
                    b"+Unrelated Mod\r\n",
                );
                m.write(
                    &format!("mo2/profiles/{p}/plugins.txt"),
                    b"*Unrelated.esm\r\n",
                );
                m.write(
                    &format!("mo2/profiles/{p}/loadorder.txt"),
                    b"Starfield.esm\r\nUnrelated.esm\r\n",
                );
            }
            m.write("mo2/mods/Unrelated Mod/Unrelated.esm", b"TES4 unrelated");
            m.write("Game/Data/Starfield.esm", b"TES4");
            m
        }

        /// MO2's active profile.
        pub(crate) fn select(&self, profile: &str) {
            let game = self
                .root
                .join("Game")
                .to_string_lossy()
                .replace('\\', "\\\\");
            self.write(
                "mo2/ModOrganizer.ini",
                format!("[General]\r\ngameName=Starfield\r\nselected_profile=@ByteArray({profile})\r\ngamePath=@ByteArray({game})\r\n[Settings]\r\n").as_bytes(),
            );
        }

        fn write(&self, rel: &str, bytes: &[u8]) {
            let p = self.root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, bytes).unwrap();
        }

        /// The instance root.
        pub(crate) fn instance(&self) -> String {
            self.root.join("mo2").display().to_string()
        }

        /// A profile folder.
        pub(crate) fn profile(&self, name: &str) -> String {
            self.root
                .join("mo2")
                .join("profiles")
                .join(name)
                .display()
                .to_string()
        }

        /// The source plugin `MyMod.esm`, holding `TES4 <content>`.
        pub(crate) fn plugin(&self, content: &str) -> String {
            let p = self.root.join("src").join("MyMod.esm");
            fs::write(&p, format!("TES4 {content}")).unwrap();
            p.display().to_string()
        }

        /// The dev mod's folder.
        pub(crate) fn mod_dir(&self) -> PathBuf {
            self.root.join("mo2/mods/KitKraft Dev - MyMod")
        }

        /// Every file of the instance and its bytes.
        pub(crate) fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
            let mut out = BTreeMap::new();
            collect(&self.root.join("mo2"), &mut out);
            out
        }
    }

    fn collect(dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.insert(p.display().to_string(), Vec::new());
                collect(&p, out);
            } else {
                out.insert(p.display().to_string(), fs::read(&p).unwrap());
            }
        }
    }

    /// A minimal plugin: a TES4 header (masters `masters`, flags `flags`) and one GLOB
    /// group with a record per FormID.
    pub(crate) fn plugin_bytes(masters: &[&str], flags: u32, forms: &[u32], value: f32) -> Vec<u8> {
        fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut v = sig.to_vec();
            v.extend((data.len() as u16).to_le_bytes());
            v.extend(data);
            v
        }
        let mut hedr = Vec::new();
        hedr.extend(0.96f32.to_le_bytes());
        hedr.extend((forms.len() as u32).to_le_bytes());
        hedr.extend(0x800u32.to_le_bytes());
        let mut body = sub(b"HEDR", &hedr);
        for m in masters {
            let mut n = m.as_bytes().to_vec();
            n.push(0);
            body.extend(sub(b"MAST", &n));
        }
        let record = |sig: &[u8; 4], flags: u32, id: u32, body: &[u8]| {
            let mut r = sig.to_vec();
            r.extend((body.len() as u32).to_le_bytes());
            r.extend(flags.to_le_bytes());
            r.extend(id.to_le_bytes());
            r.extend([0u8; 4]);
            r.extend(0x0200u16.to_le_bytes());
            r.extend(0u16.to_le_bytes());
            r.extend(body);
            r
        };
        let mut out = record(b"TES4", flags, 0, &body);
        let mut recs = Vec::new();
        for (i, id) in forms.iter().enumerate() {
            let mut b = sub(b"EDID", format!("Glob{id:X}\0").as_bytes());
            b.extend(sub(b"FLTV", &(i as f32 + value).to_le_bytes()));
            recs.extend(record(b"GLOB", 0, *id, &b));
        }
        let mut grup = b"GRUP".to_vec();
        grup.extend(((recs.len() + 24) as u32).to_le_bytes());
        grup.extend(*b"GLOB");
        grup.extend(0u32.to_le_bytes());
        grup.extend([0u8; 8]);
        grup.extend(recs);
        out.extend(grup);
        out
    }

    impl Drop for FakeMo2 {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::fake_mo2::FakeMo2;
    use super::*;
    use crate::gate::{check, ReadOnlyEngine, RunError};

    fn target(mo2: &FakeMo2) -> Result<DevTarget, String> {
        dev_target(Some(&mo2.profile("KitKraft Dev")))
    }

    /// The sample paths, spelled with the platform's separator (`C:\M\...` on Windows,
    /// `C/M/...` elsewhere) so the same shapes are exercised on both. The first segment is
    /// a drive letter, which gets its `:` on Windows only.
    fn sep_path(segments: &[&str], trailing_sep: bool) -> String {
        let mut s = segments.join(std::path::MAIN_SEPARATOR_STR);
        if cfg!(windows) {
            s.insert(1, ':');
        }
        if trailing_sep {
            s.push(std::path::MAIN_SEPARATOR);
        }
        s
    }

    #[test]
    fn the_dev_target_comes_from_the_loaded_profile_and_is_never_default() {
        let t = dev_target(Some(&sep_path(
            &["C", "M", "profiles", "KitKraft Dev"],
            false,
        )))
        .unwrap();
        assert_eq!(t.mo2, sep_path(&["C", "M"], false));
        assert_eq!(t.profile, "KitKraft Dev");
        for d in [
            sep_path(&["C", "M", "profiles", "Default"], false),
            sep_path(&["C", "M", "profiles", "default"], true),
        ] {
            let e = dev_target(Some(&d)).unwrap_err();
            assert!(e.contains("live game"), "{e}");
        }
        assert!(dev_target(None)
            .unwrap_err()
            .contains("load the dev profile"));
        assert!(dev_target(Some(&sep_path(&["C", "elsewhere", "KitKraft Dev"], false))).is_err());
    }

    #[test]
    fn preview_is_a_dry_run_and_the_confirm_drops_it() {
        let source = sep_path(&["C", "kk", "out", "built", "My.esm"], false);
        let mo2 = sep_path(&["C", "M"], false);
        let form = DeployForm {
            source: source.clone(),
            mo2: mo2.clone(),
            profile: "KitKraft Dev".into(),
        };
        let p = form.preview_request(WriteKind::Deploy, None).unwrap();
        assert!(!p.is_confirmed());
        assert_eq!(
            p.args,
            json!({"source": source, "mo2": mo2, "dev": true,
                   "profile": "KitKraft Dev", "dry_run": true})
        );
        check(&p).unwrap();
        let c = DeployForm::confirmed_request(WriteKind::Deploy, &p.args);
        assert!(c.is_confirmed());
        assert!(c.args.get("dry_run").is_none());
        check(&c).unwrap();
        // The same writing arguments without the click are refused.
        assert!(check(&Request::new("deploy", c.args.clone())).is_err());
        let r = form
            .preview_request(WriteKind::Rollback, Some("2-b"))
            .unwrap();
        assert_eq!(r.args["mod"], "My");
        assert_eq!(r.args["to"], "2-b");
        check(&r).unwrap();
        check(&DeployForm::confirmed_request(WriteKind::Rollback, &r.args)).unwrap();
        assert_eq!(form.status_request().unwrap().args["mod"], "My");
    }

    fn plan(args: &Value, outcome: &str, passed: bool) -> DeployPlan {
        DeployPlan::from_envelope(
            &json!({"data": {"dry_run": true, "passed": passed, "outcome": outcome,
                    "would_touch": [{"action": "create-dir", "path": r"C:\M\mods\x"}]}}),
            args,
        )
    }

    /// The confirm box opens only for the loaded dev profile, matching fields and a
    /// passing preview of the same arguments.
    #[test]
    fn a_profile_mismatch_keeps_the_confirm_box_closed() {
        let loaded = Ok(DevTarget {
            mo2: r"C:\M".into(),
            profile: "KitKraft Dev".into(),
        });
        let mut form = DeployForm {
            source: r"C:\s\My.esm".into(),
            mo2: r"c:/m/".into(),
            profile: "kitkraft dev".into(),
        };
        let mut args = form.preview_request(WriteKind::Deploy, None).unwrap().args;
        let ok = plan(&args, "plan", true);
        assert!(confirm_gate(WriteKind::Deploy, &form, &loaded, Some(&ok), None).is_ok());
        // No preview, a refused one, an unchanged one: closed.
        for p in [
            None,
            Some(plan(&args, "refused", false)),
            Some(plan(&args, "unchanged", true)),
        ] {
            assert!(confirm_gate(WriteKind::Deploy, &form, &loaded, p.as_ref(), None).is_err());
        }
        // A form changed after the preview: closed.
        args["source"] = json!(r"C:\s\Other.esm");
        let stale = plan(&args, "plan", true);
        assert!(
            confirm_gate(WriteKind::Deploy, &form, &loaded, Some(&stale), None)
                .unwrap_err()
                .contains("preview again")
        );
        // Another instance or profile than the loaded one: closed.
        let args = form.preview_request(WriteKind::Deploy, None).unwrap().args;
        let ok = plan(&args, "plan", true);
        form.profile = "Other".into();
        assert!(
            confirm_gate(WriteKind::Deploy, &form, &loaded, Some(&ok), None)
                .unwrap_err()
                .contains("not the loaded profile")
        );
        form.profile = "KitKraft Dev".into();
        form.mo2 = r"C:\N".into();
        assert!(confirm_gate(WriteKind::Deploy, &form, &loaded, Some(&ok), None).is_err());
        // Default loaded, or nothing loaded: closed.
        form.mo2 = r"C:\M".into();
        let default = dev_target(Some(r"C:\M\profiles\Default"));
        assert!(confirm_gate(WriteKind::Deploy, &form, &default, Some(&ok), None).is_err());
        assert!(
            confirm_gate(WriteKind::Deploy, &form, &dev_target(None), Some(&ok), None).is_err()
        );
    }

    /// The shell model of M6.G3 against a synthetic MO2 instance: status, preview (writes
    /// nothing), the confirmed deploy, status read back, a second deploy, the rollback
    /// preview and its confirmed request, status again; through the gate and the engine.
    #[test]
    fn deploy_read_back_roll_back_and_read_again_on_a_synthetic_instance() {
        let mo2 = FakeMo2::new("cockpit");
        let engine = ReadOnlyEngine::new();
        let loaded = target(&mo2);
        let mut tab = CockpitTab::default();
        let source = mo2.plugin("v1");
        tab.pick(&source);
        tab.form.fill(loaded.as_ref().unwrap());
        let run = |tab: &mut CockpitTab, slot: Slot, req: Request| -> Vec<(Slot, Request)> {
            let env = engine.run(&req).unwrap();
            tab.receive(slot, &req, &env)
        };
        let status = tab.form.status_request().unwrap();
        run(&mut tab, Slot::CockpitStatus, status.clone());
        assert!(tab
            .status
            .as_ref()
            .unwrap()
            .find("MyMod")
            .is_some_and(|m| m.live == "absent"));

        let before = mo2.snapshot();
        let preview = tab.form.preview_request(WriteKind::Deploy, None).unwrap();
        assert!(run(&mut tab, Slot::CockpitDeploy, preview).is_empty());
        assert_eq!(mo2.snapshot(), before, "the preview wrote");
        let p = tab.deploy.clone().unwrap();
        assert!(p.can_confirm(), "{p:?}");
        assert!(
            p.would_touch
                .iter()
                .any(|l| l.contains("KitKraft Dev - MyMod")),
            "{p:?}"
        );
        assert_eq!(p.plugin_sha256.as_deref().map(str::len), Some(64));
        // Nothing is written without the click: the unconfirmed writing form is refused.
        let args = confirm_gate(
            WriteKind::Deploy,
            &tab.form,
            &loaded,
            tab.deploy.as_ref(),
            None,
        )
        .unwrap();
        let unconfirmed = Request::new(
            "deploy",
            DeployForm::confirmed_request(WriteKind::Deploy, &args).args,
        );
        assert!(matches!(
            engine.run(&unconfirmed),
            Err(RunError::Refused(_))
        ));
        assert_eq!(mo2.snapshot(), before);
        // The click.
        let next = run(
            &mut tab,
            Slot::CockpitDeploy,
            DeployForm::confirmed_request(WriteKind::Deploy, &args),
        );
        assert_eq!(tab.deploy.as_ref().unwrap().outcome, "deployed");
        assert_eq!(next.len(), 1, "a write is read back with deploy.status");
        for (s, r) in next {
            run(&mut tab, s, r);
        }
        let first = tab.status.as_ref().unwrap().find("MyMod").unwrap().clone();
        assert_eq!(first.live, "managed");
        assert_eq!(first.previous, "absent");

        // A second build, deployed the same way: v1 becomes the previous.
        tab.pick(&mo2.plugin("v2"));
        let preview = tab.form.preview_request(WriteKind::Deploy, None).unwrap();
        run(&mut tab, Slot::CockpitDeploy, preview);
        let args = confirm_gate(
            WriteKind::Deploy,
            &tab.form,
            &loaded,
            tab.deploy.as_ref(),
            None,
        )
        .unwrap();
        for (s, r) in run(
            &mut tab,
            Slot::CockpitDeploy,
            DeployForm::confirmed_request(WriteKind::Deploy, &args),
        ) {
            run(&mut tab, s, r);
        }
        let second = tab.status.as_ref().unwrap().find("MyMod").unwrap().clone();
        assert_eq!(second.previous_id, first.live_id);
        assert_eq!(
            std::fs::read(mo2.mod_dir().join("MyMod.esm")).unwrap(),
            b"TES4 v2"
        );

        // Roll back: `to` from deploy.status, preview, confirm, read again.
        let to = Some(second.previous_id.as_str());
        let before = mo2.snapshot();
        let preview = tab.form.preview_request(WriteKind::Rollback, to).unwrap();
        run(&mut tab, Slot::CockpitRollback, preview);
        assert_eq!(mo2.snapshot(), before, "the rollback preview wrote");
        let args = confirm_gate(
            WriteKind::Rollback,
            &tab.form,
            &loaded,
            tab.rollback.as_ref(),
            to,
        )
        .unwrap();
        assert_eq!(args["to"], second.previous_id.as_str());
        for (s, r) in run(
            &mut tab,
            Slot::CockpitRollback,
            DeployForm::confirmed_request(WriteKind::Rollback, &args),
        ) {
            run(&mut tab, s, r);
        }
        assert_eq!(tab.rollback.as_ref().unwrap().outcome, "rolled back");
        let after = tab.status.as_ref().unwrap().find("MyMod").unwrap().clone();
        assert_eq!(after.live_id, first.live_id);
        assert_eq!(after.previous_id, second.live_id);
        assert_eq!(
            std::fs::read(mo2.mod_dir().join("MyMod.esm")).unwrap(),
            b"TES4 v1"
        );
        // The profiles were never touched.
        let profiles = |s: &std::collections::BTreeMap<String, Vec<u8>>| -> Vec<(String, Vec<u8>)> {
            s.iter()
                .filter(|(k, _)| k.contains("profiles"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };
        let fresh = FakeMo2::new("cockpit-ref");
        let rel = |s: &std::collections::BTreeMap<String, Vec<u8>>,
                   root: &str|
         -> Vec<(String, Vec<u8>)> {
            profiles(s)
                .into_iter()
                .map(|(k, v)| (k.replace(root, ""), v))
                .collect()
        };
        assert_eq!(
            rel(&mo2.snapshot(), &mo2.instance()),
            rel(&fresh.snapshot(), &fresh.instance())
        );
    }

    /// M6.G8 on a fixture plugin: the fixed sequence (validate, roundtrip, then a diff per
    /// master since the plugin is not loaded), each step not-run until its reply, the
    /// summary filled from what ran, and nothing written.
    #[test]
    fn review_runs_the_fixed_sequence_on_a_fixture_plugin() {
        let dir = std::env::temp_dir().join(format!("kitkraft-ui-review-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let master = dir.join("Base.esm");
        let plugin = dir.join("Over.esp");
        std::fs::write(
            &master,
            super::fake_mo2::plugin_bytes(&[], 1, &[0x800], 1.0),
        )
        .unwrap();
        std::fs::write(
            &plugin,
            super::fake_mo2::plugin_bytes(&["Base.esm"], 0, &[0x800, 0x0100_0801], 5.0),
        )
        .unwrap();
        let before: Vec<Vec<u8>> = [&master, &plugin]
            .iter()
            .map(|p| std::fs::read(p).unwrap())
            .collect();

        let engine = ReadOnlyEngine::new();
        let mut tab = CockpitTab {
            plugin: plugin.display().to_string(),
            ..CockpitTab::default()
        };
        let results = std::collections::BTreeMap::new();
        for s in [
            Step::Validate,
            Step::Roundtrip,
            Step::Masters,
            Step::DeployPreview,
        ] {
            assert_eq!(tab.step(s, &results).0, Status::NotRun, "{s:?}");
        }
        let mut queue: std::collections::VecDeque<(Slot, Request)> =
            tab.start_review(&[]).unwrap().into();
        assert_eq!(tab.route, Some(MastersRoute::Diffs));
        let mut ids = Vec::new();
        while let Some((slot, req)) = queue.pop_front() {
            ids.push(req.id.clone());
            check(&req).unwrap();
            let env = engine.run(&req).unwrap();
            queue.extend(tab.receive(slot, &req, &env));
        }
        assert_eq!(ids, ["validate", "roundtrip", "diff"]);
        assert_eq!(tab.step(Step::Validate, &results).0, Status::Pass);
        assert_eq!(tab.step(Step::Roundtrip, &results).0, Status::Pass);
        assert_eq!(tab.step(Step::Masters, &results).0, Status::Pass);
        assert_eq!(tab.step(Step::DeployPreview, &results).0, Status::NotRun);
        let (_, _, view) = tab.masters.first().unwrap();
        assert!(
            view.counts.iter().any(|c| c == "added 1"),
            "{:?}",
            view.counts
        );
        assert!(
            view.counts.iter().any(|c| c == "changed 1"),
            "{:?}",
            view.counts
        );
        let s = review_summary(
            tab.validate.as_ref().map(Ok),
            tab.roundtrip.as_ref().map(Ok),
        );
        let words: Vec<(&str, &str)> = s.rows.iter().map(|(r, w, _)| (*r, *w)).collect();
        assert_eq!(
            words,
            [
                ("parse", "pass"),
                ("byte_roundtrip", "pass"),
                ("ck_open", "not-run"),
                ("ck_reexport", "not-run"),
                ("game_runtime", "not-run"),
                ("save_reload", "not-run")
            ]
        );
        let after: Vec<Vec<u8>> = [&master, &plugin]
            .iter()
            .map(|p| std::fs::read(p).unwrap())
            .collect();
        assert_eq!(before, after);
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            2,
            "the review wrote a file"
        );
        let _ = std::fs::remove_dir_all(&dir);
        // Before anything ran, the summary is all not-run.
        assert!(review_summary(None, None)
            .rows
            .iter()
            .all(|(_, w, _)| *w == "not-run"));
        assert_eq!(review_summary(Some(Err("x")), None).rows[0].1, "fail");
    }

    #[test]
    fn the_masters_step_uses_conflicts_for_a_loaded_plugin() {
        let plugin = sep_path(&["C", "x", "My.esm"], false);
        let mut tab = CockpitTab {
            plugin: plugin.clone(),
            ..CockpitTab::default()
        };
        tab.start_review(&[("my.esm".into(), plugin)]).unwrap();
        assert_eq!(tab.route, Some(MastersRoute::Conflicts("My.esm".into())));
        let (reqs, missing) = masters_requests(&tab.plugin, tab.route.as_ref().unwrap(), &[]);
        assert!(missing.is_empty());
        assert_eq!(reqs[0].id, "conflicts");
        assert_eq!(reqs[0].args["plugin"], "My.esm");
        // Read off the catalog's own schema, not through `conflicts_has_detail`.
        let schema: Value =
            serde_json::from_str(gate::find_command("conflicts").unwrap().args_schema).unwrap();
        if schema.pointer("/properties/detail").is_some() {
            assert_eq!(reqs[0].args["detail"], "fields");
        } else {
            assert!(reqs[0].args.get("detail").is_none());
        }
        check(&reqs[0]).unwrap();
        let a = sep_path(&["C", "A.esm"], false);
        let (reqs, missing) = masters_requests(
            "p.esp",
            &MastersRoute::Diffs,
            &[("A.esm".into(), Some(a.clone())), ("B.esm".into(), None)],
        );
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].args, json!({"a": a, "b": "p.esp", "limit": 50}));
        assert_eq!(missing, ["B.esm"]);
    }

    /// Fable review of the wave 3 fix round, follow-up (a): each masters reply carries a
    /// fresh data key, so a second review of the same plugin starts each master's table
    /// over instead of keeping the last review's scroll and widths.
    #[test]
    fn each_masters_reply_has_its_own_table_key() {
        let mut tab = CockpitTab {
            plugin: "p.esp".into(),
            ..CockpitTab::default()
        };
        let a = sep_path(&["C", "A.esm"], false);
        let req = Request::new("diff", json!({"a": a, "b": "p.esp", "limit": 50}));
        let env = json!({"data": {}});
        let mut keys = Vec::new();
        for _ in 0..2 {
            tab.pick("p.esp");
            tab.submitted.push((Slot::CockpitMasters, req.clone()));
            tab.receive(Slot::CockpitMasters, &req, &env);
            let (name, key, _) = tab.masters.first().unwrap();
            assert_eq!(name, "A.esm");
            keys.push(*key);
        }
        assert_ne!(keys[0], keys[1]);
        // A reply nobody asked for adds no table.
        tab.pick("p.esp");
        tab.receive(Slot::CockpitMasters, &req, &env);
        assert!(tab.masters.is_empty());
    }

    /// M6.G2: with a catalog lacking `game.*` the buttons are off with "M4.T3"; with a
    /// fake `Implemented` spec (and the gate permitting it) they turn on.
    #[test]
    fn game_buttons_follow_the_catalog_and_the_gate() {
        let none = |_: &str| None;
        let planned = |_: &str| Some(false);
        let implemented = |_: &str| Some(true);
        let no = |_: &str| false;
        let yes = |_: &str| true;
        for b in GAME_BUTTONS {
            for lookup in [&none as &dyn Fn(&str) -> Option<bool>, &planned] {
                let s = game_button(*b, lookup, &yes);
                assert!(!s.enabled);
                assert!(s.reason.contains("M4.T3"), "{}", s.reason);
            }
            let s = game_button(*b, &implemented, &no);
            assert!(!s.enabled);
            assert!(s.reason.contains(b.2), "{}", s.reason);
            let s = game_button(*b, &implemented, &yes);
            assert!(s.enabled, "{}", s.reason);
        }
        // Today: game.session and game.stop are in the catalog and the gate permits
        // them (M6.G4); game.probe stays disabled with M6.G5.
        for s in game_buttons() {
            assert!(gate::find_command(s.id).is_some(), "{}", s.id);
            if s.id == "game.probe" {
                assert!(!s.enabled, "{}: {}", s.id, s.reason);
                assert!(s.reason.contains("M6.G5"), "{}: {}", s.id, s.reason);
            } else {
                assert!(s.enabled, "{}: {}", s.id, s.reason);
            }
        }
    }

    #[test]
    fn ledger_entries_naming_the_plugin_by_name_or_hash() {
        let ledger = "# Game observations\n\n## Template: O2\n\n| x | OldSoviet.esm |\n\n## Ledger\n\n### game-20261014-01: rename\n\n| Plugins | `OldSoviet.esm`, SHA-256 `9c0b28a6aa` |\n\n### game-20261015-02: other\n\nnothing here\n\n### game-20261016-01: by hash only\n\nSHA-256 `ABCDEF01`\n";
        let hits = ledger_hits(ledger, "oldsoviet.esm", None);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].entry, "game-20261014-01: rename");
        assert!(!hits[0].names_hash);
        let hits = ledger_hits(ledger, "OldSoviet.esm", Some("abcdef01"));
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert!(hits[1].names_hash);
        assert!(ledger_hits(ledger, "", None).is_empty());
        // The repo's ledger names the gun overhaul's plugin.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let text = std::fs::read_to_string(root.join("docs").join("game-observations.md")).unwrap();
        let hits = ledger_hits(
            &text,
            "KitKraftGunOverhaul.esm",
            Some("5e2f84e7b1dfd6a7876664a3bf203c3bbeff8679907bccd240c6ecb148f982d1"),
        );
        assert!(hits.iter().any(|h| h.names_hash), "{hits:?}");
    }

    #[test]
    fn status_reads_a_canned_envelope() {
        let env = json!({"data": {"mo2": r"C:\M", "active_profile": "KitKraft Dev", "warnings": [],
            "mods": [{"mod": "KitKraft Dev - My", "state": "ok", "target": "t",
              "live": {"state": "managed", "manifest": {"deployment_id": "2-b", "deployed_at": "2026"}, "integrity": {"clean": true}},
              "previous": {"state": "managed", "manifest": {"deployment_id": "1-a"}, "integrity": null},
              "pending_journal": null, "lock": null, "leftovers": [], "profile": null}]}});
        let s = DeployStatus::from_envelope(&env);
        let m = s.find("My").unwrap();
        assert_eq!((m.live_id.as_str(), m.previous_id.as_str()), ("2-b", "1-a"));
        assert_eq!(m.integrity, "clean");
        assert!(!m.pending && !m.locked);
        assert!(s.find("Other").is_none());
    }
}
