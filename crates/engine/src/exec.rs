//! `execute`: resolve a command, validate its JSON arguments, dispatch to a handler, and
//! wrap the result in the envelope.
//!
//! Handlers are the seam to the format crates. A command is wired by registering a
//! [`CommandHandler`] for its id in [`Engine::with_default_handlers`]; front ends never
//! change when a handler is added or replaced.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::args::{self, Violation};
use crate::envelope::{Envelope, Outcome};
use crate::{find_command, handlers, CommandSpec};

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
    /// The command is in the catalog but has no handler yet.
    #[error("`{0}` is in the catalog but not implemented yet")]
    NotImplemented(String),
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
            Self::NotImplemented(_) => "not_implemented",
            Self::BadSchema { .. } => "bad_schema",
            Self::Registration(..) => "registration",
            Self::Failed { .. } => "failed",
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

/// A registry of command handlers keyed by canonical command id.
#[derive(Default)]
pub struct Engine {
    handlers: BTreeMap<&'static str, Box<dyn CommandHandler>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
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
        for (id, handler) in handlers::defaults() {
            // The default table is static and covered by a test; a bad entry is skipped
            // here rather than taking the process down.
            let _ = engine.register(id, handler);
        }
        engine
    }

    /// Registers `handler` for the catalog command `id` (canonical id, not an alias).
    pub fn register(
        &mut self,
        id: &str,
        handler: Box<dyn CommandHandler>,
    ) -> Result<(), EngineError> {
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

    /// Resolves `id` (or an alias), validates `args` against the command's
    /// `args_schema`, runs its handler, and returns the envelope
    /// `{command, data, coverage, limitations, evidence}` as JSON.
    pub fn execute(&self, id: &str, args: Value) -> Result<Value, EngineError> {
        let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        let args = prepare_args(spec, args)?;
        let handler = self
            .handlers
            .get(spec.id)
            .ok_or_else(|| EngineError::NotImplemented(spec.id.to_string()))?;
        let outcome = handler.run(&args)?;
        let envelope = Envelope {
            command: spec.id,
            outcome,
        };
        Ok(serde_json::to_value(envelope)?)
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

/// Runs a command on the default engine. See [`Engine::execute`].
pub fn execute(id: &str, args: Value) -> Result<Value, EngineError> {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE
        .get_or_init(Engine::with_default_handlers)
        .execute(id, args)
}
