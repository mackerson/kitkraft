//! The read-only gate: the one place in this crate that holds an [`Engine`].
//!
//! ADR 0007 Amendment 1: the workbench offers only commands that do not write, and no
//! argument form for a writing one. The [`Engine`] is a private field of [`ReadOnlyEngine`],
//! whose only way in is [`ReadOnlyEngine::run`], which checks every request against
//! [`ALLOWLIST`] (by canonical id, so an alias cannot slip past) and each entry's argument
//! guard before the engine sees it. A test scans this crate's sources so that no other
//! module names the engine's execution API.

use kitkraft_engine::{find_command, Engine, EngineError};
use serde_json::{json, Value};

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

/// Every command the workbench can reach. Everything else in the catalog (`set`, `build`,
/// `new`, `clone`, `place`, `package`, `deploy*`, `testplan`, `overhaul.scaffold`,
/// `ba2.*`, `strings.*`, `papyrus.*`) is refused, writing or not, until a panel needs it.
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
        id: "roundtrip",
        refuse_args: &["out", "overwrite"],
        require_true: &["verify"],
        why: "verify only: rewrites in memory and compares; `out` is refused",
    },
];

/// Catalog commands that write a file or into MO2. Never on [`ALLOWLIST`]; a test holds
/// both lists against each other and against the catalog.
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
];

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

/// Checks `request` against [`ALLOWLIST`] without running it. Returns the entry.
pub fn check(request: &Request) -> Result<&'static Allowed, RunError> {
    let spec = find_command(&request.id)
        .ok_or_else(|| RunError::Refused(format!("`{}` is not a catalog command", request.id)))?;
    let entry = ALLOWLIST
        .iter()
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
    use kitkraft_engine::catalog;

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
        for a in ALLOWLIST {
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

    /// Only this module may name the engine's execution API; every other module goes
    /// through [`ReadOnlyEngine::run`].
    #[test]
    fn no_other_module_reaches_the_engine_directly() {
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut stack = vec![std::path::PathBuf::from(src)];
        let mut checked = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.file_name().and_then(|n| n.to_str()) == Some("gate.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                // `Engine` as a whole identifier (`ReadOnlyEngine` is fine).
                for (at, _) in text.match_indices("Engine") {
                    let before = text[..at].chars().next_back();
                    let after = text[at + "Engine".len()..].chars().next();
                    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
                    assert!(
                        word(before) || word(after),
                        "{}: names `Engine`; go through gate::ReadOnlyEngine",
                        path.display()
                    );
                }
                for needle in [
                    "kitkraft_engine::execute",
                    ".execute(",
                    "execute_in(",
                    "with_session(",
                    "Session",
                ] {
                    assert!(
                        !text.contains(needle),
                        "{}: names `{needle}`; go through gate::ReadOnlyEngine",
                        path.display()
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 3, "scanned {checked} files");
    }
}
