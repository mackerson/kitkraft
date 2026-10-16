//! `execute`: resolve a command, validate its JSON arguments, dispatch to a handler, and
//! wrap the result in the envelope.
//!
//! Handlers are the seam to the format crates. A command is wired by registering a
//! [`CommandHandler`] (stateless) or a [`SessionHandler`] (reads or changes the engine's
//! [`Session`]) for its id in [`Engine::with_default_handlers`]; front ends never change
//! when a handler is added or replaced.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde_json::Value;

use crate::args::{self, Violation};
use crate::envelope::{Envelope, Outcome};
use crate::progress::{self, ProgressSink};
use crate::session::Session;
use crate::{find_command, handlers, CommandSpec, Status};

/// Everything `execute` can fail with. Each variant has a stable [`EngineError::kind`].
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// No catalog entry has this id or alias.
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    /// The arguments do not match the command's `args_schema`.
    #[error("invalid arguments for `{command}`: {}", describe(violations))]
    InvalidArgs {
        /// Canonical command id.
        command: String,
        /// Every rule broken; never empty.
        violations: Vec<Violation>,
    },
    /// A command line could not be turned into arguments (unknown flag, bad value, ...).
    #[error("{}{message}", command.as_deref().map(|c| format!("`{c}`: ")).unwrap_or_default())]
    Usage {
        /// Canonical command id, when the command word was recognised.
        command: Option<String>,
        /// What is wrong.
        message: String,
    },
    /// The command is in the catalog but has no handler yet.
    #[error("`{0}` is in the catalog but not implemented yet")]
    NotImplemented(String),
    /// The command is in the catalog but deliberately deferred to a later milestone.
    #[error("`{command}` is deferred to {milestone}: {reason}")]
    Deferred {
        /// Canonical command id.
        command: String,
        /// Why it is not in this milestone.
        reason: String,
        /// The milestone it is deferred to (`M3`).
        milestone: String,
    },
    /// A catalog schema is malformed or uses an unsupported keyword (a bug in the catalog).
    #[error("args_schema of `{command}` is invalid: {message}")]
    BadSchema {
        /// Canonical command id.
        command: String,
        /// What is wrong with it.
        message: String,
    },
    /// Two handlers registered for one id, or a handler for an id not in the catalog.
    #[error("handler registration for `{0}` rejected: {1}")]
    Registration(String, &'static str),
    /// The command ran and failed (unreadable file, parse error, ...).
    #[error("`{command}` failed: {message}")]
    Failed {
        /// Canonical command id.
        command: String,
        /// What went wrong; includes the byte offset when the failure is in a plugin.
        message: String,
    },
    /// A bare input file name (`Constellation.esm`) is in none of the places an input
    /// name resolves (`docs/mcp.md`, "Paths"): the session's load order, the working
    /// directory, the Data dir.
    #[error("`{command}`: {argument} `{name}` was not found; searched {}", searched.join("; "))]
    NotFound {
        /// Canonical command id.
        command: String,
        /// JSON pointer of the argument (`/plugin`, `/tables/1`).
        argument: String,
        /// The name as given.
        name: String,
        /// Every place looked, in order.
        searched: Vec<String>,
    },
    /// The result could not be serialized (a bug in a handler).
    #[error("could not serialize the result: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl EngineError {
    /// Stable machine-readable error kind for `--json` and MCP error payloads.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::UnknownCommand(_) => "unknown_command",
            Self::InvalidArgs { .. } => "invalid_args",
            Self::Usage { .. } => "usage",
            Self::NotImplemented(_) => "not_implemented",
            Self::Deferred { .. } => "deferred",
            Self::BadSchema { .. } => "bad_schema",
            Self::Registration(..) => "registration",
            Self::Failed { .. } => "failed",
            Self::NotFound { .. } => "not_found",
            Self::Serialize(_) => "serialize",
        }
    }
}

fn describe(violations: &[Violation]) -> String {
    let parts: Vec<String> = violations
        .iter()
        .map(|v| {
            let at = if v.pointer.is_empty() {
                "arguments"
            } else {
                v.pointer.as_str()
            };
            format!("{at}: {} ({})", v.message, v.rule)
        })
        .collect();
    parts.join("; ")
}

/// Runs one command. Implementations receive arguments that already passed the
/// command's `args_schema`, with top-level `default`s filled in. They must never prompt,
/// open a dialog, or block on input.
pub trait CommandHandler: Send + Sync {
    /// Executes the command.
    fn run(&self, args: &Value) -> Result<Outcome, EngineError>;
}

/// Any `fn(&Value) -> Result<Outcome, EngineError>` is a handler.
impl<F> CommandHandler for F
where
    F: Fn(&Value) -> Result<Outcome, EngineError> + Send + Sync,
{
    fn run(&self, args: &Value) -> Result<Outcome, EngineError> {
        self(args)
    }
}

/// Runs one command that reads or changes the engine's [`Session`] (`load`, and commands
/// that work across the loaded load order). Same rules as [`CommandHandler`]: arguments
/// are already validated, and it never prompts or blocks on input.
pub trait SessionHandler: Send + Sync {
    /// Executes the command against `session`.
    fn run(&self, session: &mut Session, args: &Value) -> Result<Outcome, EngineError>;
}

/// Any `fn(&mut Session, &Value) -> Result<Outcome, EngineError>` is a session handler.
impl<F> SessionHandler for F
where
    F: Fn(&mut Session, &Value) -> Result<Outcome, EngineError> + Send + Sync,
{
    fn run(&self, session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
        self(session, args)
    }
}

/// A registered handler of either kind.
enum Handler {
    Stateless(Box<dyn CommandHandler>),
    Session(Box<dyn SessionHandler>),
}

/// A registry of command handlers keyed by canonical command id, plus the [`Session`]
/// they share.
///
/// [`Engine::execute`] runs on the engine's own session (the MCP server keeps one engine,
/// so a `load` is seen by the `inspect` that follows). [`Engine::execute_in`] runs on a
/// session the caller owns; the free [`execute`] uses a fresh one per call.
#[derive(Default)]
pub struct Engine {
    handlers: BTreeMap<&'static str, Handler>,
    session: Mutex<Session>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl Engine {
    /// An engine with no handlers: every command is `NotImplemented`.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The engine every front end uses: one handler per wired command.
    pub fn with_default_handlers() -> Self {
        let mut engine = Self::empty();
        // The default tables are static and covered by a test; a bad entry is skipped
        // here rather than taking the process down.
        for (id, handler) in handlers::defaults() {
            let _ = engine.register(id, handler);
        }
        for (id, handler) in handlers::session_defaults() {
            let _ = engine.register_session(id, handler);
        }
        engine
    }

    /// Registers a stateless `handler` for the catalog command `id` (canonical id, not an
    /// alias).
    pub fn register(
        &mut self,
        id: &str,
        handler: Box<dyn CommandHandler>,
    ) -> Result<(), EngineError> {
        self.insert(id, Handler::Stateless(handler))
    }

    /// Registers a session-aware `handler` for the catalog command `id`.
    pub fn register_session(
        &mut self,
        id: &str,
        handler: Box<dyn SessionHandler>,
    ) -> Result<(), EngineError> {
        self.insert(id, Handler::Session(handler))
    }

    fn insert(&mut self, id: &str, handler: Handler) -> Result<(), EngineError> {
        let spec = crate::catalog()
            .iter()
            .find(|spec| spec.id == id)
            .ok_or_else(|| EngineError::Registration(id.to_string(), "not a catalog id"))?;
        if self.handlers.contains_key(spec.id) {
            return Err(EngineError::Registration(
                id.to_string(),
                "already registered",
            ));
        }
        self.handlers.insert(spec.id, handler);
        Ok(())
    }

    /// Whether a handler is registered for the canonical id.
    pub fn has_handler(&self, id: &str) -> bool {
        self.handlers.contains_key(id)
    }

    /// Runs `f` on the engine's session (for front ends that show session state, such as
    /// the MCP `kitkraft://load-order` resource).
    pub fn with_session<R>(&self, f: impl FnOnce(&mut Session) -> R) -> R {
        // Panics are denied workspace-wide, so a poisoned lock can only come from a bug
        // outside our code; the session it holds is still usable.
        let mut session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut session)
    }

    /// Resolves `id` (or an alias), validates `args` against the command's
    /// `args_schema`, runs its handler on the engine's own session, and returns the
    /// envelope `{command, data, coverage, limitations, evidence, build}` as JSON.
    pub fn execute(&self, id: &str, args: Value) -> Result<Value, EngineError> {
        self.with_session(|session| self.execute_in(session, id, args))
    }

    /// As [`Engine::execute`], sending [`crate::Progress`] events to `sink` while the command
    /// runs (`crate::progress` lists which commands emit). The result is the same as
    /// without a sink.
    pub fn execute_with_progress(
        &self,
        id: &str,
        args: Value,
        sink: Arc<dyn ProgressSink>,
    ) -> Result<Value, EngineError> {
        self.with_session(|session| self.execute_in_with_progress(session, id, args, sink))
    }

    /// As [`Engine::execute_with_progress`], on a session the caller owns.
    pub fn execute_in_with_progress(
        &self,
        session: &mut Session,
        id: &str,
        args: Value,
        sink: Arc<dyn ProgressSink>,
    ) -> Result<Value, EngineError> {
        let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        let _scope = progress::Scope::enter(sink, spec.id);
        self.execute_in(session, id, args)
    }

    /// As [`Engine::execute`], on a session the caller owns.
    pub fn execute_in(
        &self,
        session: &mut Session,
        id: &str,
        args: Value,
    ) -> Result<Value, EngineError> {
        let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        let args = prepare_args(spec, args)?;
        refuse_deferred(spec)?;
        let handler = self
            .handlers
            .get(spec.id)
            .ok_or_else(|| EngineError::NotImplemented(spec.id.to_string()))?;
        let outcome = match handler {
            Handler::Stateless(handler) => handler.run(&args)?,
            Handler::Session(handler) => handler.run(session, &args)?,
        };
        let envelope = Envelope {
            command: spec.id,
            outcome,
            build: crate::identity::build_identity(),
        };
        Ok(serde_json::to_value(envelope)?)
    }
}

/// A `Deferred` command is refused before any handler runs, even one registered for it.
pub(crate) fn refuse_deferred(spec: &CommandSpec) -> Result<(), EngineError> {
    match spec.status {
        Status::Deferred { reason, milestone } => Err(EngineError::Deferred {
            command: spec.id.to_string(),
            reason: reason.to_string(),
            milestone: milestone.to_string(),
        }),
        Status::Planned | Status::Implemented => Ok(()),
    }
}

/// Validates `args` for `spec` and fills top-level defaults. `null` is treated as `{}`
/// so callers with no arguments can pass `Value::Null`.
pub fn prepare_args(spec: &CommandSpec, args: Value) -> Result<Value, EngineError> {
    let schema = args::parse_schema(spec.id, spec.args_schema)?;
    args::check_schema(spec.id, &schema)?;
    let mut args = if args.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        args
    };
    let violations = args::validate(&schema, &args);
    if !violations.is_empty() {
        return Err(EngineError::InvalidArgs {
            command: spec.id.to_string(),
            violations,
        });
    }
    args::apply_defaults(&schema, &mut args);
    Ok(args)
}

/// Runs a command on the default handlers with a fresh, empty [`Session`] (one command,
/// no state carried over: the CLI's model). See [`Engine::execute`].
pub fn execute(id: &str, args: Value) -> Result<Value, EngineError> {
    default_engine().execute_in(&mut Session::new(), id, args)
}

/// As [`execute`], sending [`crate::Progress`] events to `sink` while the command runs (the CLI's
/// `--progress`). See [`Engine::execute_with_progress`].
pub fn execute_with_progress(
    id: &str,
    args: Value,
    sink: Arc<dyn ProgressSink>,
) -> Result<Value, EngineError> {
    default_engine().execute_in_with_progress(&mut Session::new(), id, args, sink)
}

fn default_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::with_default_handlers)
}
