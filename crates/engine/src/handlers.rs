//! The default handler table: which catalog commands are wired, and to what.
//!
//! To wire a command to a format crate, write its handler here (or in a submodule) and
//! add it to [`defaults`]. Flip the catalog `status` to `Implemented` only when the
//! handler does the real work; front ends treat a `Planned` command's result as a stub.

mod ba2;
mod coverage;
mod inspect;
mod roundtrip;
mod strings;
#[cfg(test)]
pub(crate) mod testdata;

use std::path::Path;

use crate::exec::{CommandHandler, EngineError};

/// Handlers registered by [`crate::Engine::with_default_handlers`].
pub(crate) fn defaults() -> Vec<(&'static str, Box<dyn CommandHandler>)> {
    vec![
        ("roundtrip", Box::new(roundtrip::run)),
        ("inspect", Box::new(inspect::run)),
        ("coverage", Box::new(coverage::run)),
        ("ba2.list", Box::new(ba2::list)),
        ("ba2.extract", Box::new(ba2::extract)),
        ("ba2.pack", Box::new(ba2::pack)),
        ("strings.dump", Box::new(strings::dump)),
        ("strings.merge", Box::new(strings::merge)),
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
