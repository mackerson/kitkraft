//! The default handler table: which catalog commands are wired, and to what.
//!
//! To wire a command to a format crate, write its handler here (or in a submodule) and
//! add it to [`defaults`]. Flip the catalog `status` to `Implemented` only when the
//! handler does the real work; front ends treat a `Planned` command's result as a stub.

#[cfg(test)]
mod authoring_tests;
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
mod doctor;
mod game;
mod inspect;
mod ledger;
mod load;
mod matrix;
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
mod planet;
mod profiles;
mod query;
mod refs;
mod resolve;
#[cfg(test)]
mod resolve_tests;
mod roundtrip;
mod schema;
mod scripts;
#[cfg(test)]
mod scripts_tests;
mod set;
#[cfg(test)]
mod set_tests;
#[cfg(test)]
mod shapes_tests;
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
pub(crate) const DATA_DIR_ENV: &str = "KITKRAFT_DATA_DIR";

/// `name` (a `KITKRAFT_*` variable) from the environment, else its pre-rename `KITCRAFT_*`
/// spelling. Deprecated fallback kept for one release after the KitKraft rename
/// (`docs/corpus.md`, "Environment"); remove it in the release after.
pub(crate) fn env_var_os(name: &str) -> Option<OsString> {
    std::env::var_os(name).or_else(|| {
        let rest = name.strip_prefix("KITKRAFT_")?;
        std::env::var_os(format!("KITCRAFT_{rest}"))
    })
}

/// The `data_dir` argument, else `KITKRAFT_DATA_DIR` when it is set and not empty, else
/// the detected game Data dir ([`crate::host`]: argument > environment > detected).
fn data_dir(args: &serde_json::Value) -> Option<PathBuf> {
    data_dir_source(args).map(|(dir, _)| dir)
}

/// [`data_dir`] and where it came from: `data_dir`, `KITKRAFT_DATA_DIR`, or
/// `the detected game Data dir (kit doctor)`.
pub(crate) fn data_dir_source(args: &serde_json::Value) -> Option<(PathBuf, &'static str)> {
    data_dir_or(
        args,
        env_var_os(DATA_DIR_ENV),
        crate::host::detected()
            .data_dir
            .as_ref()
            .map(|f| f.path.as_path()),
    )
}

/// What [`data_dir_source`] names when the Data dir was detected.
pub(crate) const DETECTED_DATA_DIR: &str = "the detected game Data dir (kit doctor)";

/// [`data_dir_source`] with the environment value and the detected dir passed in
/// (testable without the process environment).
fn data_dir_or(
    args: &serde_json::Value,
    env: Option<OsString>,
    detected: Option<&Path>,
) -> Option<(PathBuf, &'static str)> {
    args.get("data_dir")
        .and_then(serde_json::Value::as_str)
        .map(|d| (PathBuf::from(d), "data_dir"))
        .or_else(|| {
            env.filter(|d| !d.is_empty())
                .map(|d| (PathBuf::from(d), DATA_DIR_ENV))
        })
        .or_else(|| detected.map(|d| (d.to_path_buf(), DETECTED_DATA_DIR)))
}

/// The Data dir with no `data_dir` argument: `KITKRAFT_DATA_DIR`, else the detected one.
pub(crate) fn default_data_dir() -> Option<PathBuf> {
    data_dir(&serde_json::Value::Null)
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
        ("doctor", Box::new(doctor::run)),
        ("profiles", Box::new(profiles::run)),
        ("ledger", Box::new(ledger::run)),
    ]
}

/// Single-file commands whose input names resolve through [`resolve`] (the session's load
/// order, then the working directory, then the Data dir) before the stateless handler runs.
fn resolving() -> Vec<Resolving> {
    const PLUGIN: &[Input] = &[Input::file("plugin")];
    const COVERAGE: &[Input] = &[Input::file("plugin"), Input::file("plugins")];
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
        entry("coverage", COVERAGE, coverage::run),
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
        ("refs", Box::new(refs::run)),
        ("planet", Box::new(planet::run)),
        ("set", Box::new(set::run)),
        ("validate", Box::new(validate::run)),
        ("build", Box::new(build::run)),
        ("overhaul.scaffold", Box::new(overhaul::run)),
        ("package", Box::new(package::run)),
        ("place", Box::new(place::run)),
        ("new", Box::new(newform::run_new)),
        ("clone", Box::new(newform::run_clone)),
        ("testplan", Box::new(testplan::run)),
        // Lane C (runtime, M4.T3): game sessions; the session owns the running ones.
        ("game.session", Box::new(game::session)),
        ("game.status", Box::new(game::status)),
        ("game.probe", Box::new(game::probe)),
        ("game.stop", Box::new(game::stop)),
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

/// The base game's script declarations next to `data_dir` (`papyrus.events`'s index of
/// `Tools\ContentResources.zip`), for script attachment (`validate::attached`).
pub(crate) fn papyrus_declarations(
    data_dir: Option<&Path>,
) -> Option<(PathBuf, Vec<papyrus::events::Declaration>)> {
    papyrus::events::declarations(data_dir?)
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
    use std::path::{Path, PathBuf};

    #[test]
    fn data_dir_argument_wins_then_the_environment_then_detection() {
        let env = || Some(OsString::from(r"C:\Game\Data"));
        let detected = Path::new(r"E:\Detected\Data");
        let dir = |v: Option<(PathBuf, &'static str)>| v.map(|(d, _)| d);
        assert_eq!(
            dir(data_dir_or(
                &json!({"data_dir": r"D:\Other"}),
                env(),
                Some(detected)
            )),
            Some(PathBuf::from(r"D:\Other"))
        );
        assert_eq!(
            data_dir_or(&json!({}), env(), Some(detected)),
            Some((PathBuf::from(r"C:\Game\Data"), super::DATA_DIR_ENV))
        );
        assert_eq!(
            data_dir_or(&json!({}), Some(OsString::new()), Some(detected)),
            Some((detected.to_path_buf(), super::DETECTED_DATA_DIR))
        );
        assert_eq!(data_dir_or(&json!({}), Some(OsString::new()), None), None);
        assert_eq!(data_dir_or(&json!({}), None, None), None);
    }
}
