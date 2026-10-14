//! The default handler table: which catalog commands are wired, and to what.
//!
//! To wire a command to a format crate, write its handler here (or in a submodule) and
//! add it to [`defaults`]. Flip the catalog `status` to `Implemented` only when the
//! handler does the real work; front ends treat a `Planned` command's result as a stub.

mod coverage;
mod inspect;
mod load;
mod roundtrip;
#[cfg(test)]
pub(crate) mod testdata;

use std::path::Path;

use crate::exec::{CommandHandler, EngineError, SessionHandler};

/// Stateless handlers registered by [`crate::Engine::with_default_handlers`].
pub(crate) fn defaults() -> Vec<(&'static str, Box<dyn CommandHandler>)> {
    vec![
        ("roundtrip", Box::new(roundtrip::run)),
        ("coverage", Box::new(coverage::run)),
    ]
}

/// Session-aware handlers (`fn(&mut Session, &Value) -> Result<Outcome, EngineError>`)
/// registered by [`crate::Engine::with_default_handlers`]: commands that read or change
/// the loaded load order. A command appears in this table or in [`defaults`], not both.
pub(crate) fn session_defaults() -> Vec<(&'static str, Box<dyn SessionHandler>)> {
    vec![
        ("load", Box::new(load::run)),
        ("inspect", Box::new(inspect::run)),
    ]
}

/// A required string argument. `execute` validated the schema already, so a missing one
/// is a catalog/handler mismatch, reported as a failure rather than a panic.
fn str_arg<'v>(
    command: &str,
    args: &'v serde_json::Value,
    name: &str,
) -> Result<&'v str, EngineError> {
    args.get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| failed(command, format!("argument `{name}` is missing")))
}

fn failed(command: &str, message: impl Into<String>) -> EngineError {
    EngineError::Failed {
        command: command.to_string(),
        message: message.into(),
    }
}

/// The file name shown in evidence (not the full path).
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}
