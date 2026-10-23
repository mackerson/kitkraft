//! Input names to files, one rule for every command (M4.F1).
//!
//! A bare name (`Constellation.esm`, `Constellation - Localization.ba2`: one plain path
//! component) given for an input file resolves, in order:
//!
//! 1. in the session's load order: the plugin of that name (ASCII case-insensitive);
//! 2. as given, against the working directory (a file next to the `kit` process);
//! 3. in the Data dir: `data_dir`, else `KITKRAFT_DATA_DIR`.
//!
//! Anything else (an absolute path, or a relative one with a directory) is used as given.
//! A bare name found nowhere is [`EngineError::NotFound`] naming each place searched.
//!
//! Only inputs resolve here. Output arguments (`out`, `out_dir`, `into`) are never passed
//! through this module, so a bare output name still lands in the working directory and the
//! write-target guards (never the game Data dir) see it unchanged.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::load::Loaded;
use crate::envelope::Outcome;
use crate::exec::{EngineError, SessionHandler};
use crate::session::Session;

/// One input argument of a command: a string, or an array of strings.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Input {
    /// The argument name.
    pub arg: &'static str,
    /// When this argument is present, `arg` is not a file (a path inside an archive).
    pub unless: Option<&'static str>,
}

impl Input {
    /// An input file argument.
    pub const fn file(arg: &'static str) -> Self {
        Self { arg, unless: None }
    }

    /// An input that is a path inside the archive named by `archive` when that is given.
    pub const fn unless(arg: &'static str, archive: &'static str) -> Self {
        Self {
            arg,
            unless: Some(archive),
        }
    }
}

/// Whether `name` is one plain file name with no directory. On Windows that is exactly
/// what `Path::components` reports, so this returns it unchanged there. Elsewhere a
/// Windows-spelled path (`Data\X.esm`, `C:\Data\X.esm`) and a drive-relative name (`C:`,
/// `C:foo`) arrive as one plain component, but Windows parses them as paths, so they
/// count as paths here too.
pub(crate) fn is_bare(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    if !matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(_)), None)
    ) {
        return false;
    }
    cfg!(windows) || (!name.contains('\\') && !drive_relative(name))
}

/// A Windows drive-relative name (`C:`, `C:foo`): a letter and a `:` as its first two
/// bytes (byte 1 is then a character boundary).
fn drive_relative(name: &str) -> bool {
    name.as_bytes()
        .first()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && name.as_bytes().get(1) == Some(&b':')
}

/// Resolves one input name (see the module docs). `argument` is the JSON pointer named in
/// the error (`/plugin`, `/tables/1`).
pub(crate) fn resolve(
    session: Option<&Session>,
    data_dir: Option<&Path>,
    command: &str,
    argument: &str,
    name: &str,
) -> Result<PathBuf, EngineError> {
    if !is_bare(name) {
        return Ok(PathBuf::from(name));
    }
    let loaded = session.and_then(Session::get::<Loaded>);
    if let Some(p) = loaded.and_then(|l| {
        l.lo.plugins
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }) {
        return Ok(p.path.clone());
    }
    let here = PathBuf::from(name);
    if here.exists() {
        return Ok(here);
    }
    if let Some(dir) = data_dir {
        let there = dir.join(name);
        if there.exists() {
            return Ok(there);
        }
    }
    let mut searched = vec![match loaded {
        Some(l) => format!("the session's load order ({} plugins)", l.lo.plugins.len()),
        None => "the session's load order (none loaded)".to_string(),
    }];
    searched.push(match std::env::current_dir() {
        Ok(cwd) => format!("the working directory {}", cwd.display()),
        Err(_) => "the working directory".to_string(),
    });
    searched.push(match data_dir {
        Some(dir) => format!("the Data dir {}", dir.display()),
        None => format!(
            "no Data dir (pass data_dir or set {}; `kit doctor` shows why none was detected)",
            super::DATA_DIR_ENV
        ),
    });
    Err(EngineError::NotFound {
        command: command.to_string(),
        argument: argument.to_string(),
        name: name.to_string(),
        searched,
    })
}

/// `args` with every input in `inputs` resolved to a path (see the module docs); the Data
/// dir is `args.data_dir`, else `KITKRAFT_DATA_DIR`.
pub(crate) fn inputs(
    session: Option<&Session>,
    command: &str,
    args: &Value,
    inputs: &[Input],
) -> Result<Value, EngineError> {
    let data_dir = super::data_dir(args);
    let mut out = args.clone();
    for input in inputs {
        if input.unless.is_some_and(|a| args.get(a).is_some()) {
            continue;
        }
        let Some(slot) = out.get_mut(input.arg) else {
            continue;
        };
        let pointer = format!("/{}", input.arg);
        match slot {
            Value::String(name) => {
                let path = resolve(session, data_dir.as_deref(), command, &pointer, name)?;
                *slot = Value::String(path.display().to_string());
            }
            Value::Array(items) => {
                for (i, item) in items.iter_mut().enumerate() {
                    if let Value::String(name) = item {
                        let at = format!("{pointer}/{i}");
                        let path = resolve(session, data_dir.as_deref(), command, &at, name)?;
                        *item = Value::String(path.display().to_string());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// A stateless handler whose input arguments resolve first, run with the session it needs
/// for step 1.
pub(super) struct Resolving {
    /// Canonical command id (named in errors).
    pub id: &'static str,
    /// Its input arguments.
    pub inputs: &'static [Input],
    /// The handler, given the resolved arguments.
    pub run: fn(&Value) -> Result<Outcome, EngineError>,
}

impl SessionHandler for Resolving {
    fn run(&self, session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
        let args = inputs(Some(session), self.id, args, self.inputs)?;
        (self.run)(&args)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_bare, resolve};
    use crate::handlers::testdata::temp_dir;

    #[test]
    fn bare_names_are_one_plain_component() {
        assert!(is_bare("Constellation.esm"));
        assert!(is_bare("Constellation - Localization.ba2"));
        assert!(!is_bare(r"Data\Constellation.esm"));
        assert!(!is_bare("Data/Constellation.esm"));
        assert!(!is_bare(r"C:\Data\Constellation.esm"));
        assert!(!is_bare(".."));
        assert!(!is_bare(""));
    }

    #[test]
    fn a_bare_name_resolves_in_the_data_dir_else_names_every_place() {
        let dir = temp_dir("resolve-order");
        std::fs::write(dir.join("Only.esm"), b"x").unwrap();
        let got = resolve(None, Some(&dir), "coverage", "/plugin", "Only.esm").unwrap();
        assert_eq!(got, dir.join("Only.esm"));
        // A path with a directory is used as given, found or not.
        let given = resolve(None, Some(&dir), "coverage", "/plugin", r"x\Only.esm").unwrap();
        assert_eq!(given, std::path::PathBuf::from(r"x\Only.esm"));
        let err = resolve(None, Some(&dir), "coverage", "/plugin", "Missing.esm").unwrap_err();
        assert_eq!(err.kind(), "not_found");
        let text = err.to_string();
        assert!(text.contains("Missing.esm"), "{text}");
        assert!(text.contains("load order"), "{text}");
        assert!(text.contains("working directory"), "{text}");
        assert!(text.contains(&dir.display().to_string()), "{text}");
        let err = resolve(None, None, "coverage", "/plugin", "Missing.esm").unwrap_err();
        assert!(err.to_string().contains("KITKRAFT_DATA_DIR"), "{err}");
    }
}
