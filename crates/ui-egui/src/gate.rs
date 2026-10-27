//! The read-only gate: the one place in this crate that holds an [`Engine`].
//!
//! ADR 0007 Amendment 1: the workbench offers only commands that do not write, and no
//! argument form for a writing one. The [`Engine`] is a private field of [`ReadOnlyEngine`],
//! whose only way in is [`ReadOnlyEngine::run`], which checks every request against
//! [`ALLOWLIST`] (by canonical id, so an alias cannot slip past) and each entry's argument
//! guard before the engine sees it.
//!
//! ADR 0007 Amendments 2 and 3: the writes the window may make are [`PERMITTED_WRITES`],
//! a subset of [`COCKPIT_SCOPE`] (the six commands Amendment 3 names: `ledger`, `deploy`
//! `--dev`, `deploy.rollback`, `game.session`, `game.probe`, `game.stop`). Each entry
//! has a preview form that writes nothing and passes like a read (`ledger` without
//! `confirm`, `deploy*` with `dry_run: true`), and a writing form that passes only in a
//! request built by [`Request::confirmed`], which one panel file per entry makes once per
//! confirm click (a source scan holds it); the confirmation is bound to that request's id
//! and arguments, so changing either afterwards voids it. No other request may be
//! confirmed. For
//! `deploy` and `deploy.rollback` the gate also checks what it can from the arguments
//! alone: `dev: true`, and `profile` present (writing form) and never `Default`. The
//! panel compares `mo2` and `profile` with the loaded dev profile; the engine refuses
//! `Default` on its own, from MO2's active profile. For `game.session` the writing form
//! is `start: true` with `dry_run` absent or false, and a form that has neither (a
//! synchronous session) is refused in every form: the window starts sessions with
//! `start: true` and follows them with `game.status`, because a synchronous session
//! would block the window's one engine worker; `profile` naming `Default` is refused in
//! every form of `game.session`. For `game.stop` the writing form is `dry_run` absent.
//!
//! This is also the only file of the crate that names the engine crate at all: a test
//! scans every other source file (and the integration tests) for the token, so neither a
//! renamed import (`use <engine>::{execute as go}`) nor a glob import can reach the
//! engine around the gate. The few engine helpers other modules need (catalog lookup and
//! command words for the `kit` line, the engine's own output-location guard) are
//! re-exported below.

use std::path::Path;
use std::sync::Arc;

use kitkraft_engine::{Engine, EngineError};
use serde_json::{json, Map, Value};

pub use kitkraft_engine::cmdline::command_words;
pub use kitkraft_engine::find_command;
/// The engine's documented data shapes, so the view-models' canned envelopes are checked
/// against what the engine really returns.
#[cfg(test)]
pub(crate) use kitkraft_engine::shapes::{check_shape, data_shape};
/// The envelope's value markers (M4.W0, `docs/mcp.md` "Data shapes and markers"): the
/// view-models format a value by its `kind`, never by its shape.
pub use kitkraft_engine::shapes::{kind as marker, KIND};
/// The engine's fake game session (M6.G4): a fixture MO2 instance, a fake console and a
/// fake process host, for the gate's tests against the fakes.
#[cfg(test)]
pub(crate) use kitkraft_engine::testing::GameFakes;
/// A progress event from a long command (M6.P2b), for the worker's status line (M6.P2a).
pub use kitkraft_engine::Progress;
/// The catalog itself, read by the menu bar (M7.M1, `crate::menu`): ids, labels,
/// `menu_path`, shortcuts, `writes` and status. Reading it runs nothing.
pub use kitkraft_engine::{catalog, CommandSpec, Status as CommandStatus};
#[cfg(test)]
pub(crate) use kitkraft_engine::{parse_line, Invocation};

/// One command the workbench may run, with the arguments that would make it write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allowed {
    /// Canonical catalog id.
    pub id: &'static str,
    /// Arguments that must be absent (they name an output or allow replacing one).
    pub refuse_args: &'static [&'static str],
    /// Boolean arguments that must be `true` (the read-only mode of the command).
    pub require_true: &'static [&'static str],
    /// Why the command is on the list: what it reads, and that it writes nothing.
    pub why: &'static str,
}

/// The commands the workbench runs as they are: none of them writes (the catalog's
/// `writes` is false for each; a test holds this). Besides these, only the [`GUARDED`]
/// entries run, in their non-writing form. Everything else in the catalog (`set`,
/// `build`, `new`, `clone`, `place`, `package`, `deploy*`, `testplan`,
/// `overhaul.scaffold`, `ba2.*`, `strings.*`, `papyrus.*`) is refused, writing or not,
/// until a panel needs it.
pub const ALLOWLIST: &[Allowed] = &[
    Allowed {
        id: "load",
        refuse_args: &[],
        require_true: &[],
        why:
            "reads a profile or plugin list into the session (the index cache is the engine's own)",
    },
    Allowed {
        id: "inspect",
        refuse_args: &[],
        require_true: &[],
        why: "reads one record across the load order",
    },
    Allowed {
        id: "query",
        refuse_args: &[],
        require_true: &[],
        why: "reads records matching a where predicate",
    },
    Allowed {
        id: "diff",
        refuse_args: &[],
        require_true: &[],
        why: "reads two plugins",
    },
    Allowed {
        id: "conflicts",
        refuse_args: &[],
        require_true: &[],
        why: "reads overrides in the load order",
    },
    Allowed {
        id: "validate",
        refuse_args: &[],
        require_true: &[],
        why: "reads a plugin and its masters",
    },
    Allowed {
        id: "coverage",
        refuse_args: &[],
        require_true: &[],
        why: "reads one plugin against the schema",
    },
    Allowed {
        id: "schema",
        refuse_args: &[],
        require_true: &[],
        why: "reads the builtin schema; no plugin",
    },
    Allowed {
        id: "refs",
        refuse_args: &[],
        require_true: &[],
        why: "reads a form's typed FormID links, or its referrers (the reverse index is the engine's own cache)",
    },
    Allowed {
        id: "planet",
        refuse_args: &[],
        require_true: &[],
        why: "reads the Planet Content Manager chain of one record",
    },
    Allowed {
        id: "profiles",
        refuse_args: &[],
        require_true: &[],
        why: "reads an MO2 instance's ModOrganizer.ini and lists its profile folders (the start screen)",
    },
    Allowed {
        id: "deploy.status",
        refuse_args: &[],
        require_true: &[],
        why: "reads the KitKraft dev mods of an MO2 instance, their manifests and previous deployments (the Cockpit tab, M6.G1)",
    },
    Allowed {
        id: "doctor",
        refuse_args: &[],
        require_true: &[],
        why: "reads where the game, Data dir, MO2 instance and profiles, tools and cache are (M6.G1; first run, M6.X4)",
    },
    // Lane R round 2 (M7.R2, M7.R3): the Render Window's cell picker; the Cell View
    // (M7.W2) will use it as well.
    Allowed {
        id: "cell",
        refuse_args: &[],
        require_true: &[],
        why: "reads worldspaces, cells and one cell's references (with their bases' OBND)",
    },
    // Lane C (M4.T3): the Cockpit follows its started sessions with it (M6.G4).
    Allowed {
        id: "game.status",
        refuse_args: &[],
        require_true: &[],
        why: "reads a started game session's phase, the row running, the probe rows so far, the screenshots taken, the events and the report once it has finished (lane C, M4.T3; M6.G4)",
    },
];

/// Writing commands the workbench runs only in a non-writing form: every argument that
/// would write is refused and the read-only mode is required. Each id is also in
/// [`WRITING`]; a test holds that every entry refuses at least one argument.
pub const GUARDED: &[Allowed] = &[Allowed {
    id: "roundtrip",
    refuse_args: &["out", "overwrite"],
    require_true: &["verify"],
    why: "verify only: rewrites in memory and compares; `out` is refused",
}];

/// Catalog commands that can write a file or into MO2: exactly the catalog's `writes`
/// set (a test holds this). Never on [`ALLOWLIST`]; on [`GUARDED`] only with their
/// writing arguments refused.
pub const WRITING: &[&str] = &[
    "set",
    "build",
    "new",
    "clone",
    "place",
    "testplan",
    "overhaul.scaffold",
    "package",
    "ba2.extract",
    "ba2.pack",
    "papyrus.compile",
    "strings.merge",
    "deploy",
    "deploy.rollback",
    // ADR 0009 Amendment 2: edits the dev profile's lists; refused in the window (not in
    // the cockpit scope of ADR 0007 Amendment 3).
    "deploy.enable",
    "deploy.disable",
    "roundtrip",
    "ledger",
    // Lane C (M4.T3): the window may run `game.session` and `game.stop` with their
    // confirmations (M6.G4); `game.probe` waits on M6.G5.
    "game.session",
    "game.probe",
    "game.stop",
    // lane D round 3 (M7.S7): refused in the window until M7.E1/M7.O4 rule on it.
    "revert",
    // Lane T round 2 (M4.T10): the brief-to-mod chain writes through every stage; refused in the window.
    "mod.run",
];

/// What tells a permitted write's preview from its writing form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preview {
    /// The command writes when this boolean argument is given and not `false`
    /// (`ledger`'s `confirm`); absent or `false` is the preview.
    WritesWhen(&'static str),
    /// The command writes unless this boolean argument is `true` (`deploy`'s `dry_run`);
    /// `true` is the preview.
    PreviewWhen(&'static str),
}

/// One write the window may make (ADR 0007 Amendments 2 and 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermittedWrite {
    /// Canonical catalog id; also in [`COCKPIT_SCOPE`] and [`WRITING`].
    pub id: &'static str,
    /// The argument test that tells its preview from its writing form.
    pub preview: Preview,
    /// The one panel file (under `src/`) allowed to build its confirmed request.
    pub panel: &'static str,
    /// `dev: true` required in every form, `profile` required in the writing form and
    /// refused as `Default` in any form (`deploy`, `deploy.rollback`: Amendment 3, "What
    /// the gate checks").
    pub dev_profile: bool,
    /// Booleans that must be `true` in the writing form (`game.session`'s `start`); a
    /// form that is not the preview and lacks one is refused (M6.G4).
    pub writing_requires: &'static [&'static str],
    /// Refuse `profile` naming the live profile (any case, trimmed) in every form,
    /// without requiring `dev` or a profile (`game.session`: M6.G4).
    pub refuse_live_profile: bool,
    /// What it writes, and why it is allowed.
    pub why: &'static str,
}

/// The cockpit write scope of ADR 0007 Amendment 3 (accepted by the owner 2026-10-16,
/// M6.O3): the only ids [`PERMITTED_WRITES`] may hold. It changes only with an accepted
/// amendment to ADR 0007 (M7's editor writes are Amendment 4, M7.E1); the gate test
/// fails on a permitted id outside it.
pub const COCKPIT_SCOPE: &[&str] = &[
    "ledger",
    "deploy",
    "deploy.rollback",
    "game.session",
    "game.probe",
    "game.stop",
];

/// The MO2 profile that is the owner's live game: never a target (AGENTS.md).
pub const LIVE_PROFILE: &str = "Default";

/// The writes the window may make, each confirmed per click. They grow entry by entry
/// with the ticket that implements them (M6.G3: `deploy`, `deploy.rollback`; M6.G4:
/// `game.session`, `game.stop`; M6.G5: `game.probe`), always inside [`COCKPIT_SCOPE`].
pub const PERMITTED_WRITES: &[PermittedWrite] = &[
    PermittedWrite {
        id: "ledger",
        preview: Preview::WritesWhen("confirm"),
        panel: "panels/ledger.rs",
        dev_profile: false,
        writing_requires: &[],
        refuse_live_profile: false,
        why: "appends one dated row to docs/game-observations.md, a repo doc (not game data); the engine refuses any other path; one confirm click per row (Amendment 2)",
    },
    PermittedWrite {
        id: "deploy",
        preview: Preview::PreviewWhen("dry_run"),
        panel: "panels/cockpit.rs",
        dev_profile: true,
        writing_requires: &[],
        refuse_live_profile: false,
        why: "copies one plugin or package into the isolated MO2 mod `KitKraft Dev - <plugin>` (ADR 0009); the engine refuses Default, profiles, plugins.txt and every other mod (Amendment 3, M6.G3)",
    },
    PermittedWrite {
        id: "deploy.rollback",
        preview: Preview::PreviewWhen("dry_run"),
        panel: "panels/cockpit.rs",
        dev_profile: true,
        writing_requires: &[],
        refuse_live_profile: false,
        why: "swaps one dev mod's `.previous` deployment back in, keeping the replaced one as the new previous: the way back from a deploy (Amendment 3, M6.G3)",
    },
    // M6.G4: the game session's start and stop (`game.probe` joins with M6.G5).
    PermittedWrite {
        id: "game.session",
        preview: Preview::PreviewWhen("dry_run"),
        panel: "panels/cockpit/session.rs",
        dev_profile: false,
        writing_requires: &["start"],
        refuse_live_profile: true,
        why: "starts an agent-driven game session on the dev profile (ADR 0010); the window starts it with `start: true` and follows it with `game.status` (Amendment 3, M6.G4)",
    },
    PermittedWrite {
        id: "game.stop",
        preview: Preview::PreviewWhen("dry_run"),
        panel: "panels/cockpit/session.rs",
        dev_profile: false,
        writing_requires: &[],
        refuse_live_profile: false,
        why: "ends a started game session: remaining rows not-run, the game ended, logs copied, the profile files restored if changed, one ledger entry (Amendment 3, M6.G4)",
    },
];

/// The permitted write for canonical id `id`.
pub fn permitted(id: &str) -> Option<&'static PermittedWrite> {
    PERMITTED_WRITES.iter().find(|p| p.id == id)
}

/// Refuses `dir` as a place the workbench writes into (its screenshot tour) when it is,
/// or is inside, the game's Data folder, the user Data folder, an MO2 instance or
/// profile, or passes through a link: the engine's own output-location guard, applied to
/// a file in `dir`.
pub fn guard_output_dir(dir: &Path) -> Result<(), String> {
    kitkraft_engine::edit::patch::guard_location(&dir.join("workbench.bmp"))
        .map_err(|e| e.to_string())
}

/// A command and its JSON arguments, as a panel builds them.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Catalog id (canonical or alias).
    pub id: String,
    /// The arguments object.
    pub args: Value,
    /// Made by a confirm click ([`Request::confirmed`]): the id and arguments the user
    /// confirmed, the only way a [`PERMITTED_WRITES`] entry runs in its writing form.
    /// Private, so outside this module it is set only through that constructor (read it
    /// with [`Request::is_confirmed`]); a source scan keeps the constructor, per id, to
    /// that entry's panel file. Bound to the id and arguments: [`check`] refuses a
    /// confirmed request whose public `id` or `args` were changed after the click.
    confirmed_for: Option<(String, Value)>,
}

impl Request {
    /// A request for `id` with `args`.
    pub fn new(id: &str, args: Value) -> Self {
        Self {
            id: id.to_string(),
            args,
            confirmed_for: None,
        }
    }

    /// A request the user confirmed with a click on a confirm button, one per click. The
    /// gate takes it for [`PERMITTED_WRITES`] only; outside this file `id` is a string
    /// literal naming the command (the source scan reads it).
    pub fn confirmed(id: &str, args: Value) -> Self {
        Self {
            confirmed_for: Some((id.to_string(), args.clone())),
            ..Self::new(id, args)
        }
    }

    /// Whether the request came from [`Request::confirmed`] and still carries the id and
    /// arguments that were confirmed.
    pub fn is_confirmed(&self) -> bool {
        self.confirmed_for
            .as_ref()
            .is_some_and(|(id, args)| *id == self.id && *args == self.args)
    }
}

/// Why a request did not produce an envelope.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RunError {
    /// The workbench refused it before the engine saw it.
    #[error("refused by the read-only workbench: {0}")]
    Refused(String),
    /// The engine ran it and failed; `kind` is the engine's stable error kind.
    #[error("{kind}: {message}")]
    Failed {
        /// `EngineError::kind()`.
        kind: String,
        /// The error message.
        message: String,
        /// `{"error": {kind, message, violations?, searched?}}`, the shape `kit --json`
        /// prints.
        json: Value,
    },
}

impl RunError {
    fn from_engine(err: &EngineError) -> Self {
        let mut body = serde_json::Map::new();
        body.insert("kind".into(), json!(err.kind()));
        body.insert("message".into(), json!(err.to_string()));
        match err {
            EngineError::InvalidArgs { violations, .. } => {
                let list: Vec<Value> = violations
                    .iter()
                    .map(|v| json!({"pointer": v.pointer, "rule": v.rule, "message": v.message}))
                    .collect();
                body.insert("violations".into(), Value::Array(list));
            }
            EngineError::NotFound { searched, .. } => {
                body.insert("searched".into(), json!(searched));
            }
            _ => {}
        }
        Self::Failed {
            kind: err.kind().to_string(),
            message: err.to_string(),
            json: json!({ "error": body }),
        }
    }
}

/// Whether boolean argument `name` is given and not `false` (`null` counts as absent).
fn on(args: Option<&Map<String, Value>>, name: &str) -> bool {
    args.and_then(|m| m.get(name))
        .is_some_and(|v| !v.is_null() && v != &Value::Bool(false))
}

/// The arguments as an object (`None` for `null`), or the refusal.
fn object(args: &Value) -> Result<Option<&Map<String, Value>>, RunError> {
    match args {
        Value::Null => Ok(None),
        Value::Object(m) => Ok(Some(m)),
        _ => Err(RunError::Refused(
            "arguments must be a JSON object".to_string(),
        )),
    }
}

/// The check of one [`PERMITTED_WRITES`] entry: its preview passes like a read, its
/// writing form only confirmed; `deploy*` also need `dev: true` and a profile that is not
/// `Default`, and `game.session` refuses `Default` in every form and accepts only the
/// `start: true` writing form (M6.G4).
fn check_permitted(p: &PermittedWrite, request: &Request) -> Result<(), RunError> {
    let args = object(&request.args)?;
    let writes = match p.preview {
        Preview::WritesWhen(arg) => on(args, arg),
        Preview::PreviewWhen(arg) => args.and_then(|m| m.get(arg)) != Some(&Value::Bool(true)),
    };
    if p.dev_profile {
        if args.and_then(|m| m.get("dev")) != Some(&Value::Bool(true)) {
            return Err(RunError::Refused(format!(
                "`{}` runs here only with `dev: true` (ADR 0009: development deployment is opt-in on every call)",
                p.id
            )));
        }
        match args.and_then(|m| m.get("profile")) {
            Some(Value::String(name)) if name.trim().eq_ignore_ascii_case(LIVE_PROFILE) => {
                return Err(RunError::Refused(format!(
                    "`{}`: `{LIVE_PROFILE}` is the owner's live game and never a target; use the dev profile",
                    p.id
                )));
            }
            Some(Value::String(name)) if !name.trim().is_empty() => {}
            None | Some(Value::Null) if !writes => {}
            _ => {
                return Err(RunError::Refused(format!(
                    "`{}` writes only with `profile` naming the dev profile (ADR 0007 Amendment 3)",
                    p.id
                )));
            }
        }
    }
    if p.refuse_live_profile {
        if let Some(Value::String(name)) = args.and_then(|m| m.get("profile")) {
            if name.trim().eq_ignore_ascii_case(LIVE_PROFILE) {
                return Err(RunError::Refused(format!(
                    "`{}`: `{LIVE_PROFILE}` is the owner's live game and never a target; use the dev profile",
                    p.id
                )));
            }
        }
    }
    if writes {
        for name in p.writing_requires {
            if args.and_then(|m| m.get(*name)) != Some(&Value::Bool(true)) {
                return Err(RunError::Refused(format!(
                    "`{}`: the window starts sessions with `start: true` and follows them with `game.status`; a synchronous session would block the window's one engine worker",
                    p.id
                )));
            }
        }
        if !request.is_confirmed() {
            return Err(RunError::Refused(format!(
                "`{}` in its writing form is sent only from its panel's confirm button ({}; ADR 0007 Amendment 3)",
                p.id, p.panel
            )));
        }
    }
    Ok(())
}

/// Checks `request` against [`ALLOWLIST`], [`GUARDED`] and [`PERMITTED_WRITES`] without
/// running it.
pub fn check(request: &Request) -> Result<(), RunError> {
    if request.confirmed_for.is_some() && !request.is_confirmed() {
        return Err(RunError::Refused(format!(
            "`{}`: the request was changed after its confirm click; a confirmation covers only the id and arguments it was made for",
            request.id
        )));
    }
    let spec = find_command(&request.id)
        .ok_or_else(|| RunError::Refused(format!("`{}` is not a catalog command", request.id)))?;
    if let Some(p) = permitted(spec.id) {
        return check_permitted(p, request);
    }
    if request.confirmed_for.is_some() {
        return Err(RunError::Refused(format!(
            "`{}`: only {} take a confirmed write; nothing else in the workbench writes",
            spec.id,
            PERMITTED_WRITES
                .iter()
                .map(|p| format!("`{}`", p.id))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let entry = ALLOWLIST
        .iter()
        .chain(GUARDED)
        .find(|a| a.id == spec.id)
        .ok_or_else(|| {
            RunError::Refused(format!(
                "`{}` is not on the workbench's read-only list (ADR 0007 Amendment 1); run it from `kit` or MCP",
                spec.id
            ))
        })?;
    let args = object(&request.args)?;
    for name in entry.refuse_args {
        if on(args, name) {
            return Err(RunError::Refused(format!(
                "`{}` with `{name}` would write a file; the workbench writes only its confirmed writes",
                spec.id
            )));
        }
    }
    for name in entry.require_true {
        let on = args.and_then(|m| m.get(*name)) == Some(&Value::Bool(true));
        if !on {
            return Err(RunError::Refused(format!(
                "`{}` runs here only with `{name}: true`",
                spec.id
            )));
        }
    }
    Ok(())
}

/// The engine behind the gate. Construct one per worker; it keeps one session (the
/// loaded load order), as `kit mcp` does.
pub struct ReadOnlyEngine {
    engine: Engine,
}

impl std::fmt::Debug for ReadOnlyEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadOnlyEngine").finish_non_exhaustive()
    }
}

impl Default for ReadOnlyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadOnlyEngine {
    /// An engine with the default handlers and an empty session.
    pub fn new() -> Self {
        Self {
            engine: Engine::with_default_handlers(),
        }
    }

    /// Runs `request` when [`check`] allows it; returns the envelope as JSON.
    pub fn run(&self, request: &Request) -> Result<Value, RunError> {
        check(request)?;
        self.engine
            .execute(&request.id, request.args.clone())
            .map_err(|e| RunError::from_engine(&e))
    }

    /// As [`ReadOnlyEngine::run`], sending the command's [`Progress`] events to `sink`
    /// while it runs (M6.P2a; the envelope is the same with or without one).
    pub fn run_with_progress<F>(&self, request: &Request, sink: F) -> Result<Value, RunError>
    where
        F: Fn(&Progress) + Send + Sync + 'static,
    {
        check(request)?;
        self.engine
            .execute_with_progress(&request.id, request.args.clone(), Arc::new(sink))
            .map_err(|e| RunError::from_engine(&e))
    }
}

#[cfg(test)]
impl ReadOnlyEngine {
    /// A gate over an engine with `f`'s fixture installed (M6.G4): the gate's checks run
    /// against the fakes, the engine itself is untouched.
    pub(crate) fn with_game_fakes(f: &GameFakes) -> Self {
        let engine = Self::new();
        f.install(&engine.engine);
        engine
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    /// Why `permitted` (the ids of a `PERMITTED_WRITES`) breaks the rule of ADR 0007
    /// Amendment 3, "The test that enforces it": every id inside `scope`
    /// ([`COCKPIT_SCOPE`]), in the catalog, marked `writes` and `Implemented`, listed
    /// once, and neither allowed outright nor guarded.
    fn scope_violations(permitted: &[&str], scope: &[&str]) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for id in permitted {
            if !seen.insert(*id) {
                out.push(format!("{id}: listed twice"));
            }
            if !scope.contains(id) {
                out.push(format!(
                    "{id}: not in COCKPIT_SCOPE; a new write needs an accepted amendment to ADR 0007"
                ));
            }
            match find_command(id) {
                Some(spec) if spec.id != *id => out.push(format!("{id}: not the canonical id")),
                Some(spec) if !spec.writes => {
                    out.push(format!("{id}: the catalog does not mark it writes"))
                }
                Some(spec) if spec.status != kitkraft_engine::Status::Implemented => {
                    out.push(format!("{id}: not Implemented"))
                }
                Some(_) => {}
                None => out.push(format!(
                    "{id}: not in the catalog (listed only once it exists)"
                )),
            }
            if ALLOWLIST.iter().chain(GUARDED).any(|a| a.id == *id) {
                out.push(format!("{id}: also allowed or guarded"));
            }
        }
        out
    }

    /// The lists here are a second entry for the catalog's `writes` flag: the allowlist
    /// holds no writing command, `WRITING` is exactly the writing set, a guarded entry is
    /// a writing command with its writing arguments refused, and the writing commands
    /// the window can run in a writing form are exactly the ids of `PERMITTED_WRITES`,
    /// all inside `COCKPIT_SCOPE` (ADR 0007 Amendment 3).
    #[test]
    fn allowlist_is_disjoint_from_writes_and_writing_equals_it() {
        let writes: BTreeSet<&str> = catalog()
            .iter()
            .filter(|s| s.writes)
            .map(|s| s.id)
            .collect();
        for a in ALLOWLIST {
            assert!(
                !writes.contains(a.id),
                "{}: the catalog says it writes; move it to GUARDED with its writing arguments refused",
                a.id
            );
        }
        let writing: BTreeSet<&str> = WRITING.iter().copied().collect();
        assert_eq!(writing.len(), WRITING.len(), "WRITING has a duplicate");
        assert_eq!(
            writing, writes,
            "WRITING must equal the catalog's `writes` set"
        );
        for g in GUARDED {
            assert!(writes.contains(g.id), "{}: guarded but not writing", g.id);
            assert!(
                !g.refuse_args.is_empty(),
                "{}: a guarded command must refuse its writing arguments",
                g.id
            );
            assert!(!ALLOWLIST.iter().any(|a| a.id == g.id), "{}", g.id);
        }
        // The scope is Amendment 3's six commands, and changes only with an amendment.
        assert_eq!(
            COCKPIT_SCOPE,
            [
                "ledger",
                "deploy",
                "deploy.rollback",
                "game.session",
                "game.probe",
                "game.stop"
            ]
        );
        let ids: Vec<&str> = PERMITTED_WRITES.iter().map(|p| p.id).collect();
        let bad = scope_violations(&ids, COCKPIT_SCOPE);
        assert!(bad.is_empty(), "{bad:#?}");
        // What the window can run in a writing form: an allowed id that writes (none),
        // or a permitted one.
        let runnable_writes: BTreeSet<&str> = writes
            .iter()
            .copied()
            .filter(|w| ALLOWLIST.iter().any(|a| a.id == *w) || permitted(w).is_some())
            .collect();
        let permitted_ids: BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(runnable_writes, permitted_ids);
        // M6.G3: the ledger and the dev deployment and its rollback; M6.G4 adds the game
        // session's start and stop; `game.probe` joins with M6.G5.
        assert_eq!(
            ids,
            [
                "ledger",
                "deploy",
                "deploy.rollback",
                "game.session",
                "game.stop"
            ]
        );
        for p in PERMITTED_WRITES {
            assert!(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("src")
                    .join(p.panel)
                    .is_file(),
                "{}: panel file {} missing",
                p.id,
                p.panel
            );
        }
    }

    /// The test above fails when a permitted id is outside the scope: `revert` (M7,
    /// Amendment 4), `testplan` (kept out by Amendment 3) or an id the catalog lacks.
    #[test]
    fn a_permitted_write_outside_the_scope_fails_the_rule() {
        let mut ids: Vec<&str> = PERMITTED_WRITES.iter().map(|p| p.id).collect();
        assert!(scope_violations(&ids, COCKPIT_SCOPE).is_empty());
        for extra in ["testplan", "set", "package", "revert"] {
            ids.push(extra);
            let bad = scope_violations(&ids, COCKPIT_SCOPE);
            assert!(
                bad.iter()
                    .any(|b| b.starts_with(extra) && b.contains("COCKPIT_SCOPE")),
                "{extra}: {bad:?}"
            );
            ids.pop();
        }
        // In scope but read-only, or twice: refused too.
        assert!(!scope_violations(&["ledger", "ledger"], COCKPIT_SCOPE).is_empty());
        assert!(!scope_violations(&["schema"], &["schema"]).is_empty());
        assert!(!scope_violations(&["no.such"], &["no.such"]).is_empty());
    }

    /// Where `text` builds a confirmed request: each `Request::confirmed(` with the id
    /// it names (`None` when the id is not a string literal), and any other way of
    /// setting the flag.
    fn confirmed_calls(text: &str) -> Vec<Option<String>> {
        let mut out = Vec::new();
        for (at, _) in text.match_indices("Request::confirmed(") {
            let rest = &text[at + "Request::confirmed(".len()..];
            let rest = rest.trim_start();
            let id = rest
                .strip_prefix('"')
                .and_then(|r| r.split_once('"'))
                .map(|(id, _)| id.to_string());
            out.push(id);
        }
        if text.contains("confirmed: true")
            || text.contains(".confirmed =")
            || text.contains("confirmed_for")
        {
            out.push(None);
        }
        out
    }

    /// Each permitted write's confirmed request is built only in `gate.rs` and that
    /// entry's panel file (ADR 0007 Amendment 3, the per-entry source scan); a confirmed
    /// request for any other id, or with an id that is not a string literal, is found.
    #[test]
    fn only_each_permitted_writes_panel_builds_its_confirmed_request() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let gate = root.join("src").join("gate.rs");
        let mut stack = vec![root.join("src"), root.join("tests")];
        let mut found = Vec::new();
        let mut builders = BTreeSet::new();
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs") || path == gate {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                for call in confirmed_calls(&text) {
                    let ok = call.as_deref().is_some_and(|id| {
                        permitted(id).is_some_and(|p| path == root.join("src").join(p.panel))
                    });
                    if ok {
                        builders.insert(call.unwrap_or_default());
                    } else {
                        found.push(format!("{}: {call:?}", path.display()));
                    }
                }
            }
        }
        assert!(found.is_empty(), "{found:#?}");
        // Every permitted write has its one builder.
        let ids: BTreeSet<String> = PERMITTED_WRITES.iter().map(|p| p.id.to_string()).collect();
        assert_eq!(builders, ids);
        // The scan reads ids, and catches the other spellings.
        assert_eq!(
            confirmed_calls("Request::confirmed( \"deploy\", a); Request::confirmed(id, a)"),
            [Some("deploy".to_string()), None]
        );
        assert_eq!(confirmed_calls("r.confirmed = true;"), [None]);
        assert_eq!(confirmed_calls("r.confirmed_for = x;"), [None]);
    }

    /// A confirmation covers the id and arguments it was made for: changing a confirmed
    /// request's public `id` or `args` afterwards makes `check` refuse it, so a panel
    /// allowed to confirm one write cannot turn its request into another.
    #[test]
    fn a_confirmed_request_changed_after_the_click_is_refused() {
        let ledger = json!({"row": "C3", "result": "pass", "confirm": true});
        assert!(check(&Request::confirmed("ledger", ledger.clone())).is_ok());
        let deploy = json!({"plugin": "X.esp", "dev": true, "profile": "KitKraft Dev"});
        // The id changed: a confirmed ledger request turned into a deploy.
        let mut r = Request::confirmed("ledger", ledger.clone());
        r.id = "deploy".to_string();
        assert!(!r.is_confirmed());
        let e = check(&r);
        assert!(
            matches!(&e, Err(RunError::Refused(m)) if m.contains("changed after")),
            "{e:?}"
        );
        r.args = deploy.clone();
        assert!(matches!(check(&r), Err(RunError::Refused(_))));
        // The arguments changed, id kept.
        let mut r = Request::confirmed("ledger", ledger);
        r.args = json!({"row": "C4", "result": "fail", "confirm": true});
        assert!(matches!(check(&r), Err(RunError::Refused(_))));
        // A confirmed deploy whose arguments were swapped.
        let mut r = Request::confirmed("deploy", deploy);
        r.args = json!({"plugin": "Y.esp", "dev": true, "profile": "KitKraft Dev"});
        assert!(matches!(check(&r), Err(RunError::Refused(_))));
    }

    /// The ledger previews without a confirmation and writes only from a confirmed
    /// request; a command outside `PERMITTED_WRITES` takes no confirmed request.
    #[test]
    fn the_ledger_is_one_write_and_needs_a_confirmed_request() {
        let args = json!({"row": "C3", "result": "pass", "confirm": true});
        let refused = check(&Request::new("ledger", args.clone()));
        assert!(
            matches!(&refused, Err(RunError::Refused(m)) if m.contains("confirm button")),
            "{refused:?}"
        );
        assert!(check(&Request::new(
            "ledger",
            json!({"row": "C3", "confirm": "yes"})
        ))
        .is_err());
        assert!(check(&Request::confirmed("ledger", args)).is_ok());
        for preview in [
            json!({"row": "C3", "result": "pass"}),
            json!({"row": "C3", "result": "pass", "confirm": false}),
        ] {
            assert!(check(&Request::new("ledger", preview)).is_ok());
        }
        for (id, a) in [
            ("schema", json!({})),
            ("set", json!({"form": "x", "fields": {}, "into": "y.esp"})),
            ("roundtrip", json!({"plugin": "x.esm", "verify": true})),
            ("testplan", json!({})),
        ] {
            let e = check(&Request::confirmed(id, a));
            assert!(matches!(e, Err(RunError::Refused(_))), "{id}: {e:?}");
        }
    }

    /// `deploy` and `deploy.rollback` (ADR 0007 Amendment 3, "What the gate checks"): the
    /// dry run passes unconfirmed; the writing form only confirmed; `dev: true` always;
    /// `profile` present for the writing form and never `Default` (any case).
    #[test]
    fn deploy_and_rollback_preview_unconfirmed_and_write_only_confirmed() {
        for (id, base) in [
            (
                "deploy",
                json!({"source": "x.esm", "mo2": "m", "dev": true}),
            ),
            (
                "deploy.rollback",
                json!({"mo2": "m", "mod": "X", "dev": true, "to": "1-a"}),
            ),
        ] {
            let with = |extra: Value| {
                let mut a = base.clone();
                for (k, v) in extra.as_object().unwrap() {
                    a[k] = v.clone();
                }
                a
            };
            let dev = json!({"profile": "KitKraft Dev"});
            // Preview: dry run, with or without a profile.
            assert!(check(&Request::new(id, with(json!({"dry_run": true})))).is_ok());
            assert!(check(&Request::new(
                id,
                with(json!({"dry_run": true, "profile": "KitKraft Dev"}))
            ))
            .is_ok());
            // Writing form: refused unconfirmed (dry_run absent or false).
            for w in [
                dev.clone(),
                json!({"profile": "KitKraft Dev", "dry_run": false}),
            ] {
                let e = check(&Request::new(id, with(w.clone())));
                assert!(
                    matches!(&e, Err(RunError::Refused(m)) if m.contains("confirm button")),
                    "{id}: {e:?}"
                );
                assert!(check(&Request::confirmed(id, with(w))).is_ok(), "{id}");
            }
            // `dev` must be true, in every form.
            for bad in [
                json!({"dev": false}),
                json!({"dev": null}),
                json!({"dev": "true"}),
            ] {
                let mut a = with(json!({"dry_run": true}));
                a["dev"] = bad["dev"].clone();
                assert!(check(&Request::confirmed(id, a)).is_err(), "{id}: {bad}");
            }
            let mut no_dev = with(json!({"dry_run": true}));
            no_dev.as_object_mut().unwrap().remove("dev");
            assert!(check(&Request::new(id, no_dev)).is_err());
            // The writing form needs a profile; Default is refused in any form.
            assert!(
                check(&Request::confirmed(id, base.clone())).is_err(),
                "{id}"
            );
            assert!(check(&Request::confirmed(id, with(json!({"profile": " "})))).is_err());
            for d in ["Default", "default", " DEFAULT "] {
                for extra in [
                    json!({"profile": d}),
                    json!({"profile": d, "dry_run": true}),
                ] {
                    let e = check(&Request::confirmed(id, with(extra)));
                    assert!(
                        matches!(&e, Err(RunError::Refused(m)) if m.contains("live game")),
                        "{id} {d}: {e:?}"
                    );
                }
            }
        }
    }

    /// `game.session` (ADR 0007 Amendment 3, "What the cockpit may run in a writing
    /// form"; M6.G4): the dry run passes unconfirmed; the writing form is `start: true`
    /// with `dry_run` absent or false, and only confirmed; a form with neither
    /// (a synchronous session) is refused, confirmed or not; `profile` naming `Default`
    /// (any case, trimmed) is refused in every form.
    #[test]
    fn game_session_previews_unconfirmed_and_starts_only_confirmed() {
        let base = json!({"mo2": "m", "probes": "p"});
        let with = |extra: Value| {
            let mut a = base.clone();
            for (k, v) in extra.as_object().unwrap() {
                a[k] = v.clone();
            }
            a
        };
        // Preview: a dry run, with or without a profile, passes unconfirmed.
        assert!(check(&Request::new(
            "game.session",
            with(json!({"dry_run": true}))
        ))
        .is_ok());
        assert!(check(&Request::new(
            "game.session",
            with(json!({"dry_run": true, "profile": "KitKraft Dev"}))
        ))
        .is_ok());
        // Writing form: `start: true` with `dry_run` absent or false; only confirmed.
        for w in [
            json!({"start": true}),
            json!({"start": true, "dry_run": false}),
            json!({"start": true, "profile": "KitKraft Dev"}),
        ] {
            let e = check(&Request::new("game.session", with(w.clone())));
            assert!(
                matches!(&e, Err(RunError::Refused(m)) if m.contains("confirm button")),
                "{w}: {e:?}"
            );
            assert!(
                check(&Request::confirmed("game.session", with(w.clone()))).is_ok(),
                "{w}"
            );
        }
        // The synchronous form (neither `start` nor `dry_run`) is refused, confirmed or
        // not: it would block the window's one engine worker.
        for sync in [json!({}), json!({"profile": "KitKraft Dev"})] {
            for req in [
                Request::new("game.session", with(sync.clone())),
                Request::confirmed("game.session", with(sync)),
            ] {
                let e = check(&req).unwrap_err();
                assert!(
                    matches!(&e, RunError::Refused(m) if m.contains("one engine worker")),
                    "{e:?}"
                );
            }
        }
        // `profile` naming `Default` (any case, trimmed) is refused in every form.
        for d in ["Default", "default", " default ", " DEFAULT "] {
            for extra in [
                json!({"profile": d, "dry_run": true}),
                json!({"profile": d, "start": true}),
            ] {
                for confirmed in [false, true] {
                    let req = if confirmed {
                        Request::confirmed("game.session", with(extra.clone()))
                    } else {
                        Request::new("game.session", with(extra.clone()))
                    };
                    let e = check(&req).unwrap_err();
                    assert!(
                        matches!(&e, RunError::Refused(m) if m.contains("live game")),
                        "{d} confirmed={confirmed}: {e:?}"
                    );
                }
            }
        }
    }

    /// `game.stop` (ADR 0007 Amendment 3; M6.G4): the dry run passes unconfirmed; the
    /// writing form (no `dry_run`) only confirmed.
    #[test]
    fn game_stop_previews_unconfirmed_and_stops_only_confirmed() {
        let base = json!({"id": "2026-10-27-T1"});
        let with = |extra: Value| {
            let mut a = base.clone();
            for (k, v) in extra.as_object().unwrap() {
                a[k] = v.clone();
            }
            a
        };
        // Preview: a dry run, unconfirmed.
        assert!(check(&Request::new("game.stop", with(json!({"dry_run": true})))).is_ok());
        // Writing form: refused unconfirmed, passes confirmed.
        for w in [json!({}), json!({"dry_run": false})] {
            let e = check(&Request::new("game.stop", with(w.clone())));
            assert!(
                matches!(&e, Err(RunError::Refused(m)) if m.contains("confirm button")),
                "{w}: {e:?}"
            );
            assert!(
                check(&Request::confirmed("game.stop", with(w.clone()))).is_ok(),
                "{w}"
            );
        }
    }

    /// Through the gate and the engine on the fakes' fixture MO2 instance (never the
    /// owner's): the preview writes nothing; the confirmed start returns the session id
    /// and launches once; `game.status` follows it to `idle`; the confirmed stop returns
    /// the report (M6.G4).
    #[test]
    fn a_confirmed_session_starts_and_stops_against_the_fakes() {
        use std::time::Duration;

        use crate::panels::cockpit::session::SessionForm;

        let fakes = GameFakes::new("gate-session").unwrap();
        let engine = ReadOnlyEngine::with_game_fakes(&fakes);
        let form = SessionForm::from_args(&fakes.session_args(json!(null)));
        // The preview: a dry run that passes unconfirmed and writes nothing.
        let preview = form.preview_request();
        let env = engine.run(&preview).unwrap();
        assert_eq!(env["data"]["dry_run"], json!(true), "{env}");
        assert_eq!(env["data"]["would_start"], json!(true), "{env}");
        assert!(
            !fakes.observe().exists(),
            "the dry run wrote into {}",
            fakes.observe().display()
        );
        // The confirm: the previewed arguments with `dry_run` dropped and `start: true`
        // added, built by the panel, not here.
        let start = SessionForm::start_request(&preview.args);
        assert!(start.is_confirmed());
        let env = engine.run(&start).unwrap();
        assert_eq!(env["data"]["started"], json!(true), "{env}");
        let id = env["data"]["session"].as_str().unwrap().to_string();
        // Follow it with `game.status` until the session is `idle`.
        let mut polls = 0;
        loop {
            let env = engine
                .run(&Request::new("game.status", json!({"id": id})))
                .unwrap();
            polls += 1;
            if env["data"]["phase"] == json!("idle") {
                break;
            }
            assert!(polls < 400, "still running after {polls} polls: {env}");
            std::thread::sleep(Duration::from_millis(25));
        }
        // Stop it: the preview, then the confirmed stop and its report.
        assert!(engine.run(&SessionForm::stop_preview(&id)).is_ok());
        let stopped = engine.run(&SessionForm::stop_request(&id)).unwrap();
        assert!(stopped["data"]["report"].is_object(), "{stopped}");
        // The start launched the fake game once (the session thread is joined now).
        assert_eq!(fakes.launched(), 1);
    }

    /// Through the gate and the engine on a synthetic MO2 instance (never the owner's):
    /// the preview writes nothing, the confirmed deploy and rollback write, and the
    /// engine refuses on its own when MO2's active profile is `Default`, whatever the
    /// `profile` argument says.
    #[test]
    fn deploy_reaches_the_engine_only_confirmed_and_the_engine_refuses_default() {
        use crate::panels::cockpit::fake_mo2::FakeMo2;
        let mo2 = FakeMo2::new("gate");
        let engine = ReadOnlyEngine::new();
        let args = json!({"source": mo2.plugin("v1"), "mo2": mo2.instance(), "dev": true,
                          "profile": "KitKraft Dev"});
        let mut preview = args.clone();
        preview["dry_run"] = json!(true);
        let before = mo2.snapshot();
        let plan = engine.run(&Request::new("deploy", preview)).unwrap();
        assert_eq!(plan["data"]["outcome"], "plan", "{plan}");
        assert_eq!(mo2.snapshot(), before, "a preview wrote");
        assert!(engine.run(&Request::new("deploy", args.clone())).is_err());
        assert_eq!(mo2.snapshot(), before, "an unconfirmed request wrote");
        let done = engine
            .run(&Request::confirmed("deploy", args.clone()))
            .unwrap();
        assert_eq!(done["data"]["outcome"], "deployed", "{done}");
        assert!(mo2.mod_dir().join("MyMod.esm").is_file());
        // A second deploy makes a previous; roll back to it, confirmed.
        let mut v2 = args.clone();
        v2["source"] = json!(mo2.plugin("v2"));
        engine.run(&Request::confirmed("deploy", v2)).unwrap();
        let rb = json!({"mo2": mo2.instance(), "mod": "MyMod", "dev": true,
                        "profile": "KitKraft Dev"});
        assert!(engine
            .run(&Request::new("deploy.rollback", rb.clone()))
            .is_err());
        let back = engine
            .run(&Request::confirmed("deploy.rollback", rb))
            .unwrap();
        assert_eq!(back["data"]["outcome"], "rolled back", "{back}");
        assert_eq!(
            std::fs::read(mo2.mod_dir().join("MyMod.esm")).unwrap(),
            b"TES4 v1"
        );
        // MO2 on Default: the gate lets the dev profile argument through, the engine
        // refuses.
        mo2.select("Default");
        let refused = engine.run(&Request::confirmed("deploy", args)).unwrap_err();
        assert!(
            matches!(&refused, RunError::Failed { message, .. } if message.contains("Default")),
            "{refused:?}"
        );
    }

    #[test]
    fn every_catalog_command_is_either_allowed_writing_or_refused_on_purpose() {
        // Read-only commands no panel uses yet; refused until one does.
        const UNUSED_READ_ONLY: &[&str] = &[
            "ba2.list",
            "papyrus.disasm",
            "papyrus.events",
            "strings.dump",
        ];
        for spec in catalog() {
            let listed = ALLOWLIST.iter().any(|a| a.id == spec.id);
            let writing = WRITING.contains(&spec.id);
            let unused = UNUSED_READ_ONLY.contains(&spec.id);
            assert_eq!(
                u8::from(listed) + u8::from(writing) + u8::from(unused),
                1,
                "{}: classify it in gate.rs (allowed, writing, or unused read-only)",
                spec.id
            );
        }
        for a in ALLOWLIST.iter().chain(GUARDED) {
            assert!(find_command(a.id).is_some(), "{} not in the catalog", a.id);
        }
    }

    #[test]
    fn writing_commands_and_their_aliases_are_refused_before_the_engine() {
        let engine = ReadOnlyEngine::new();
        for spec in catalog()
            .iter()
            .filter(|s| !ALLOWLIST.iter().any(|a| a.id == s.id))
        {
            // A permitted write's writing form is what is refused (its preview runs).
            let args = match spec.id {
                "ledger" => json!({"row": "C1", "result": "pass", "confirm": true}),
                "deploy" => json!({"source": "x.esm", "mo2": "m", "dev": true,
                                   "profile": "KitKraft Dev"}),
                "deploy.rollback" => json!({"mo2": "m", "mod": "X", "dev": true,
                                            "profile": "KitKraft Dev"}),
                _ => json!({}),
            };
            for name in std::iter::once(&spec.id).chain(spec.aliases.iter()) {
                let err = engine.run(&Request::new(name, args.clone())).unwrap_err();
                assert!(matches!(err, RunError::Refused(_)), "{name}: {err:?}");
                // The progress path is gated the same way, and its sink never hears from
                // the engine (Fable review of wave 3, finding 5).
                let heard = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let flag = Arc::clone(&heard);
                let err = engine
                    .run_with_progress(&Request::new(name, args.clone()), move |_| {
                        flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    })
                    .unwrap_err();
                assert!(
                    matches!(err, RunError::Refused(_)),
                    "{name} with progress: {err:?}"
                );
                assert!(!heard.load(std::sync::atomic::Ordering::SeqCst), "{name}");
            }
        }
        for id in [
            "set",
            "build",
            "place",
            "deploy",
            "package",
            "ba2.pack",
            "strings.merge",
        ] {
            assert!(!ALLOWLIST.iter().any(|a| a.id == id), "{id}");
            assert!(WRITING.contains(&id), "{id}");
        }
    }

    #[test]
    fn roundtrip_runs_only_as_verify_and_never_with_out() {
        let req = |args: Value| Request::new("roundtrip", args);
        assert!(check(&req(json!({"plugin": "x.esm", "verify": true}))).is_ok());
        assert!(check(&req(json!({"plugin": "x.esm"}))).is_err());
        assert!(check(&req(json!({"plugin": "x.esm", "verify": false}))).is_err());
        let out = check(&req(
            json!({"plugin": "x.esm", "verify": true, "out": "y.esm"}),
        ));
        assert!(matches!(out, Err(RunError::Refused(m)) if m.contains("out")));
        assert!(check(&req(
            json!({"plugin": "x.esm", "verify": true, "overwrite": true})
        ))
        .is_err());
        // `overwrite: false` and `out: null` say "no", which is allowed.
        assert!(check(&req(
            json!({"plugin": "x.esm", "verify": true, "overwrite": false, "out": null})
        ))
        .is_ok());
    }

    #[test]
    fn an_allowed_command_reaches_the_engine_and_returns_the_envelope() {
        let engine = ReadOnlyEngine::new();
        let env = engine
            .run(&Request::new("defs", json!({"type": "GLOB"})))
            .unwrap();
        assert_eq!(env["command"], "schema");
        for key in ["data", "coverage", "limitations", "evidence"] {
            assert!(env.get(key).is_some(), "{key}");
        }
    }

    /// `refs` and `planet` (and their aliases) pass the gate and reach the engine, which
    /// fails them here only because nothing is loaded.
    #[test]
    fn refs_and_planet_reach_the_engine() {
        let engine = ReadOnlyEngine::new();
        for id in ["refs", "useinfo", "planet", "pcm"] {
            let err = engine
                .run(&Request::new(id, json!({"form": "Base.esm:000800"})))
                .unwrap_err();
            assert!(
                matches!(&err, RunError::Failed { message, .. } if message.contains("no load order")),
                "{id}: {err:?}"
            );
        }
    }

    #[test]
    fn engine_errors_keep_their_kind_and_violations() {
        let engine = ReadOnlyEngine::new();
        let err = engine
            .run(&Request::new("query", json!({"type": 5})))
            .unwrap_err();
        match err {
            RunError::Failed { kind, json, .. } => {
                assert_eq!(kind, "invalid_args");
                assert!(json["error"]["violations"].is_array(), "{json}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// Why `text` (a source file other than this one) could reach the engine around the
    /// gate. The engine crate's name is denied outright, so renamed (`as`) and glob
    /// imports are caught with everything else; the execution API's names are denied
    /// besides, in case a path to them is ever re-exported by mistake.
    fn engine_reach(text: &str) -> Vec<String> {
        let mut found = Vec::new();
        if text.contains("kitkraft_engine") {
            found.push("names the engine crate `kitkraft_engine`; use what gate re-exports".into());
        }
        // `Engine` as a whole identifier (`ReadOnlyEngine` is fine).
        for (at, _) in text.match_indices("Engine") {
            let before = text[..at].chars().next_back();
            let after = text[at + "Engine".len()..].chars().next();
            let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            if !(word(before) || word(after)) {
                found.push("names `Engine`; go through gate::ReadOnlyEngine".into());
            }
        }
        // `Session` as a whole identifier (the engine's session handle; a longer word
        // such as `SessionForm` is fine, as `ReadOnlyEngine` is above).
        for (at, _) in text.match_indices("Session") {
            let before = text[..at].chars().next_back();
            let after = text[at + "Session".len()..].chars().next();
            let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            if !(word(before) || word(after)) {
                found.push("names `Session`; go through gate::ReadOnlyEngine".into());
            }
        }
        for needle in [".execute(", "execute_in(", "with_session("] {
            if text.contains(needle) {
                found.push(format!("names `{needle}`; go through gate::ReadOnlyEngine"));
            }
        }
        found
    }

    #[test]
    fn renamed_and_glob_imports_of_the_engine_are_caught() {
        for bypass in [
            "use kitkraft_engine::{execute as go};\nfn f() { let _ = go(\"set\", x); }",
            "use kitkraft_engine::*;\nfn f() { let _ = execute(\"set\", x); }",
            "use ::kitkraft_engine as ke;",
            "extern crate kitkraft_engine as ke;",
        ] {
            assert!(!engine_reach(bypass).is_empty(), "not caught: {bypass}");
        }
        let fine = "use crate::gate::{ReadOnlyEngine, Request};\nlet e = ReadOnlyEngine::new();";
        assert!(engine_reach(fine).is_empty(), "{:?}", engine_reach(fine));
    }

    /// Only this file may name the engine crate; every other module (and every
    /// integration test) goes through [`ReadOnlyEngine::run`] and the re-exports above.
    /// The manifest may not rename the engine dependency (`package = ...`), which would
    /// give it a name this scan does not know.
    #[test]
    fn no_other_file_reaches_the_engine_directly() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut stack = vec![root.join("src"), root.join("tests")];
        let mut checked = 0;
        let mut problems = Vec::new();
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path == root.join("src").join("gate.rs")
                    || path.extension().and_then(|e| e.to_str()) != Some("rs")
                {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                for why in engine_reach(&text) {
                    problems.push(format!("{}: {why}", path.display()));
                }
                checked += 1;
            }
        }
        assert!(problems.is_empty(), "{problems:#?}");
        assert!(checked > 10, "scanned {checked} files");
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(
            !manifest
                .lines()
                .any(|l| l.split('#').next().unwrap_or("").contains("package") && l.contains('=')),
            "Cargo.toml renames a dependency (`package = ...`)"
        );
    }
}
