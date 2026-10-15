//! The read-only gate: the one place in this crate that holds an [`Engine`].
//!
//! ADR 0007 Amendment 1: the workbench offers only commands that do not write, and no
//! argument form for a writing one. The [`Engine`] is a private field of [`ReadOnlyEngine`],
//! whose only way in is [`ReadOnlyEngine::run`], which checks every request against
//! [`ALLOWLIST`] (by canonical id, so an alias cannot slip past) and each entry's argument
//! guard before the engine sees it.
//!
//! This is also the only file of the crate that names the engine crate at all: a test
//! scans every other source file (and the integration tests) for the token, so neither a
//! renamed import (`use <engine>::{execute as go}`) nor a glob import can reach the
//! engine around the gate. The few engine helpers other modules need (catalog lookup and
//! command words for the `kit` line, the engine's own output-location guard) are
//! re-exported below.

use std::path::Path;

use kitkraft_engine::{Engine, EngineError};
use serde_json::{json, Value};

pub use kitkraft_engine::cmdline::command_words;
pub use kitkraft_engine::find_command;
/// The engine's documented data shapes, so the view-models' canned envelopes are checked
/// against what the engine really returns.
#[cfg(test)]
pub(crate) use kitkraft_engine::shapes::{check_shape, data_shape};
/// The envelope's value markers (M4.W0, `docs/mcp.md` "Data shapes and markers"): the
/// view-models format a value by its `kind`, never by its shape.
pub use kitkraft_engine::shapes::{kind as marker, KIND};
#[cfg(test)]
pub(crate) use kitkraft_engine::{catalog, parse_line, Invocation};

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
    "roundtrip",
];

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
}

impl Request {
    /// A request for `id` with `args`.
    pub fn new(id: &str, args: Value) -> Self {
        Self {
            id: id.to_string(),
            args,
        }
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

/// Checks `request` against [`ALLOWLIST`] and [`GUARDED`] without running it. Returns
/// the entry.
pub fn check(request: &Request) -> Result<&'static Allowed, RunError> {
    let spec = find_command(&request.id)
        .ok_or_else(|| RunError::Refused(format!("`{}` is not a catalog command", request.id)))?;
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
    let args = match &request.args {
        Value::Null => None,
        Value::Object(map) => Some(map),
        _ => {
            return Err(RunError::Refused(
                "arguments must be a JSON object".to_string(),
            ))
        }
    };
    for name in entry.refuse_args {
        let given = args
            .and_then(|m| m.get(*name))
            .is_some_and(|v| !v.is_null() && v != &Value::Bool(false));
        if given {
            return Err(RunError::Refused(format!(
                "`{}` with `{name}` would write a file; the workbench never writes",
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
    Ok(entry)
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
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The lists here are a second entry for the catalog's `writes` flag: the allowlist
    /// holds no writing command, `WRITING` is exactly the writing set, and a guarded
    /// entry is a writing command with its writing arguments refused.
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
    }

    #[test]
    fn every_catalog_command_is_either_allowed_writing_or_refused_on_purpose() {
        // Read-only commands no panel uses yet; refused until one does.
        const UNUSED_READ_ONLY: &[&str] = &[
            "ba2.list",
            "papyrus.disasm",
            "papyrus.events",
            "strings.dump",
            "deploy.status",
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
            for name in std::iter::once(&spec.id).chain(spec.aliases.iter()) {
                let err = engine.run(&Request::new(name, json!({}))).unwrap_err();
                assert!(matches!(err, RunError::Refused(_)), "{name}: {err:?}");
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
        for needle in [".execute(", "execute_in(", "with_session(", "Session"] {
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
