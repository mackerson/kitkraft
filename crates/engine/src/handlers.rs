//! The default handler table: which catalog commands are wired, and to what.
//!
//! To wire a command to a format crate, write its handler here (or in a submodule) and
//! add it to [`defaults`]. Flip the catalog `status` to `Implemented` only when the
//! handler does the real work; front ends treat a `Planned` command's result as a stub.

mod ba2;
mod build;
#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod build_v3_tests;
mod conflicts;
mod coverage;
mod deploy;
mod diff;
mod inspect;
mod load;
mod newform;
#[cfg(test)]
mod newform_tests;
mod overhaul;
#[cfg(test)]
mod overhaul_tests;
mod package;
mod papyrus;
mod place;
#[cfg(test)]
pub(crate) mod place_tests;
mod query;
mod resolve;
#[cfg(test)]
mod resolve_tests;
mod roundtrip;
mod schema;
mod set;
#[cfg(test)]
mod set_tests;
mod strings;
#[cfg(test)]
pub(crate) mod testdata;
mod testplan;
#[cfg(test)]
mod testplan_tests;
mod validate;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::exec::{CommandHandler, EngineError, SessionHandler};
use resolve::{Input, Resolving};

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
        ("schema", Box::new(schema::run)),
        ("ba2.pack", Box::new(ba2::pack)),
        ("papyrus.compile", Box::new(papyrus::compile::run)),
        ("deploy", Box::new(deploy::run)),
        ("deploy.rollback", Box::new(deploy::rollback)),
        ("deploy.status", Box::new(deploy::status)),
        ("papyrus.events", Box::new(papyrus::events::run)),
    ]
}

/// Single-file commands whose input names resolve through [`resolve`] (the session's load
/// order, then the working directory, then the Data dir) before the stateless handler runs.
fn resolving() -> Vec<Resolving> {
    const PLUGIN: &[Input] = &[Input::file("plugin")];
    const AB: &[Input] = &[Input::file("a"), Input::file("b")];
    const ARCHIVE: &[Input] = &[Input::file("archive")];
    const TABLE: &[Input] = &[Input::file("archive"), Input::unless("table", "archive")];
    const TABLES: &[Input] = &[Input::file("tables")];
    const PEX: &[Input] = &[Input::file("archive"), Input::unless("pex", "archive")];
    type Run = fn(&serde_json::Value) -> Result<crate::Outcome, EngineError>;
    let entry =
        |id: &'static str, inputs: &'static [Input], run: Run| Resolving { id, inputs, run };
    vec![
        entry("roundtrip", PLUGIN, roundtrip::run),
        entry("coverage", PLUGIN, coverage::run),
        entry("diff", AB, diff::run),
        entry("ba2.list", ARCHIVE, ba2::list),
        entry("ba2.extract", ARCHIVE, ba2::extract),
        entry("strings.dump", TABLE, strings::dump),
        entry("strings.merge", TABLES, strings::merge),
        entry("papyrus.disasm", PEX, papyrus::disasm),
    ]
}

/// Session-aware handlers (`fn(&mut Session, &Value) -> Result<Outcome, EngineError>`)
/// registered by [`crate::Engine::with_default_handlers`]: commands that read or change
/// the loaded load order, or resolve input names in it ([`resolving`]). A command appears
/// in this table or in [`defaults`], not both.
pub(crate) fn session_defaults() -> Vec<(&'static str, Box<dyn SessionHandler>)> {
    let mut table: Vec<(&'static str, Box<dyn SessionHandler>)> = vec![
        ("load", Box::new(load::run)),
        ("inspect", Box::new(inspect::run)),
        ("query", Box::new(query::run)),
        ("conflicts", Box::new(conflicts::run)),
        ("set", Box::new(set::run)),
        ("validate", Box::new(validate::run)),
        ("build", Box::new(build::run)),
        ("overhaul.scaffold", Box::new(overhaul::run)),
        ("package", Box::new(package::run)),
        ("place", Box::new(place::run)),
        ("new", Box::new(newform::run_new)),
        ("clone", Box::new(newform::run_clone)),
        ("testplan", Box::new(testplan::run)),
    ];
    for r in resolving() {
        table.push((r.id, Box::new(r)));
    }
    table
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
