//! The default handler table: which catalog commands are wired, and to what.
//!
//! To wire a command to a format crate, write its handler here (or in a submodule) and
//! add it to [`defaults`]. Flip the catalog `status` to `Implemented` only when the
//! handler does the real work; front ends treat a `Planned` command's result as a stub.

mod ba2;
mod build;
#[cfg(test)]
mod build_tests;
mod conflicts;
mod coverage;
mod diff;
mod inspect;
mod load;
mod papyrus;
mod query;
mod roundtrip;
mod schema;
mod set;
#[cfg(test)]
mod set_tests;
mod strings;
#[cfg(test)]
pub(crate) mod testdata;
mod validate;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::exec::{CommandHandler, EngineError, SessionHandler};

/// Names the game Data dir when a command is not given `data_dir` (`docs/corpus.md`).
pub(crate) const DATA_DIR_ENV: &str = "KITCRAFT_DATA_DIR";

/// The `data_dir` argument, else `KITCRAFT_DATA_DIR` when it is set and not empty.
fn data_dir(args: &serde_json::Value) -> Option<PathBuf> {
    data_dir_or(args, std::env::var_os(DATA_DIR_ENV))
}

/// [`data_dir`] with the environment value passed in (testable without the process
/// environment).
fn data_dir_or(args: &serde_json::Value, env: Option<OsString>) -> Option<PathBuf> {
    args.get("data_dir")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .or_else(|| env.filter(|d| !d.is_empty()).map(PathBuf::from))
}

/// Stateless handlers registered by [`crate::Engine::with_default_handlers`].
pub(crate) fn defaults() -> Vec<(&'static str, Box<dyn CommandHandler>)> {
    vec![
        ("roundtrip", Box::new(roundtrip::run)),
        ("coverage", Box::new(coverage::run)),
        ("schema", Box::new(schema::run)),
        ("diff", Box::new(diff::run)),
        ("ba2.list", Box::new(ba2::list)),
        ("ba2.extract", Box::new(ba2::extract)),
        ("ba2.pack", Box::new(ba2::pack)),
        ("strings.dump", Box::new(strings::dump)),
        ("strings.merge", Box::new(strings::merge)),
        ("papyrus.disasm", Box::new(papyrus::disasm)),
        ("papyrus.compile", Box::new(papyrus::compile::run)),
    ]
}

/// Session-aware handlers (`fn(&mut Session, &Value) -> Result<Outcome, EngineError>`)
/// registered by [`crate::Engine::with_default_handlers`]: commands that read or change
/// the loaded load order. A command appears in this table or in [`defaults`], not both.
pub(crate) fn session_defaults() -> Vec<(&'static str, Box<dyn SessionHandler>)> {
    vec![
        ("load", Box::new(load::run)),
        ("inspect", Box::new(inspect::run)),
        ("query", Box::new(query::run)),
        ("conflicts", Box::new(conflicts::run)),
        ("set", Box::new(set::run)),
        ("validate", Box::new(validate::run)),
        ("build", Box::new(build::run)),
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

#[cfg(test)]
mod tests {
    use super::data_dir_or;
    use serde_json::json;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn data_dir_argument_wins_then_the_environment() {
        let env = || Some(OsString::from(r"C:\Game\Data"));
        assert_eq!(
            data_dir_or(&json!({"data_dir": r"D:\Other"}), env()),
            Some(PathBuf::from(r"D:\Other"))
        );
        assert_eq!(
            data_dir_or(&json!({}), env()),
            Some(PathBuf::from(r"C:\Game\Data"))
        );
        assert_eq!(data_dir_or(&json!({}), Some(OsString::new())), None);
        assert_eq!(data_dir_or(&json!({}), None), None);
    }
}
