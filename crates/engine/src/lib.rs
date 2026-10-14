//! The command engine.
//!
//! Every user-visible action is one [`CommandSpec`] in [`catalog()`]. Menus, toolbar,
//! scripts, the CLI and the MCP server all resolve a name to the same spec and call the
//! same `execute`, so behaviour cannot drift between front ends. `execute` takes its
//! arguments as JSON matching `args_schema` and never opens a dialog; interactive
//! front ends gather arguments first, then call it.
//!
//! [`execute`] resolves an id or alias, validates the arguments against the command's
//! `args_schema` (the subset documented in [`args`]), dispatches to the registered
//! [`CommandHandler`], and returns the result envelope from `docs/mcp.md`:
//! `{command, data, coverage, limitations, evidence}`.
//!
//! An [`Engine`] owns a [`Session`] (the loaded load order and caches) that
//! [`SessionHandler`]s read and change; the free [`execute`] runs on a fresh session.
//! Undo and validation rules land in M2 and later (see `plan/execution-plan.md`).
//!
//! LAYER: L4

pub mod args;
pub mod cmdline;
mod envelope;
mod exec;
mod handlers;
mod params;
mod session;

pub use args::Violation;
pub use cmdline::{parse_line, parse_words, Invocation};
pub use envelope::{Coverage, CoverageStatus, Evidence, Outcome, NOT_IMPLEMENTED};
pub use exec::{execute, prepare_args, CommandHandler, Engine, EngineError, SessionHandler};
pub use params::{params, Param, ParamKind};
pub use session::Session;

/// Whether a catalog entry does anything yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Listed so every front end agrees on the name and arguments; not callable yet.
    /// Allowed only while a milestone task owns it (see the `catalog_closure` test).
    Planned,
    /// Callable from every front end.
    Implemented,
    /// Deliberately not in this milestone. `execute` refuses it with
    /// [`EngineError::Deferred`]; `kit help` and MCP `tools/list` show the reason.
    Deferred {
        /// Why it is not done now.
        reason: &'static str,
        /// Where it goes (`M3`).
        milestone: &'static str,
    },
}

impl Status {
    /// Short label for help text and listings: `implemented`, `planned`, `deferred`.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Implemented => "implemented",
            Self::Deferred { .. } => "deferred",
        }
    }

    /// The note front ends append to a command's summary: empty when implemented.
    pub fn note(&self) -> String {
        match self {
            Self::Planned => "planned: not implemented yet".to_string(),
            Self::Implemented => String::new(),
            Self::Deferred { reason, milestone } => format!("deferred to {milestone}: {reason}"),
        }
    }
}

/// One user-visible action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    /// Stable dotted id: `inspect`, `ba2.extract`. The CLI spells it `kit ba2 extract`.
    pub id: &'static str,
    /// Human label for menus and palettes.
    pub label: &'static str,
    /// Menu location in the future UI, `/`-separated.
    pub menu_path: &'static str,
    /// Default keyboard shortcut, if any.
    pub shortcut: Option<&'static str>,
    /// Other names accepted by the CLI, `command_line` and the palette.
    pub aliases: &'static [&'static str],
    /// One-line description used in `kit help` and MCP `tools/list`.
    pub summary: &'static str,
    /// JSON Schema (draft 2020-12) for the arguments object.
    pub args_schema: &'static str,
    /// Whether it is callable yet.
    pub status: Status,
}

/// The full command catalog, in help order.
pub fn catalog() -> &'static [CommandSpec] {
    CATALOG
}

/// Finds a command by id or alias. CLI words are joined with `.` before lookup
/// (`kit ba2 list` -> `ba2.list`).
pub fn find_command(name: &str) -> Option<&'static CommandSpec> {
    CATALOG
        .iter()
        .find(|spec| spec.id == name || spec.aliases.contains(&name))
}

static CATALOG: &[CommandSpec] = &[
    CommandSpec {
        id: "load",
        label: "Load Order...",
        menu_path: "File/Load Order...",
        shortcut: Some("Ctrl+O"),
        aliases: &["open"],
        summary: "Load plugins from an MO2 profile or an explicit plugin list",
        args_schema: r#"{"type":"object","properties":{"profile":{"type":"string"},"plugins":{"type":"array","items":{"type":"string"}},"data_dir":{"type":"string"}},"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "inspect",
        label: "Inspect Record",
        menu_path: "Record/Inspect",
        shortcut: Some("Enter"),
        aliases: &["show"],
        summary: "One record by FormID or EditorID: header fields and subrecords (typed fields and overrides later)",
        args_schema: r#"{"type":"object","properties":{"form":{"type":"string","description":"FormID (hex, file-relative) or EditorID"},"plugin":{"type":"string","description":"Plugin to search (until load order exists)"}},"required":["form","plugin"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "query",
        label: "Query Records...",
        menu_path: "Record/Query...",
        shortcut: Some("Ctrl+F"),
        aliases: &["find"],
        summary: "Find records by type and field predicate",
        args_schema: r#"{"type":"object","properties":{"type":{"type":"string","minLength":4,"maxLength":4},"where":{"type":"string"},"limit":{"type":"integer","minimum":1},"cursor":{"type":"string","description":"Opaque paging token from the previous result"}},"required":["type"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "set",
        label: "Set Field",
        menu_path: "Record/Set Field...",
        shortcut: None,
        aliases: &[],
        summary: "Write field values on a record into a target plugin",
        args_schema: r#"{"type":"object","properties":{"form":{"type":"string"},"fields":{"type":"object"},"into":{"type":"string"}},"required":["form","fields","into"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "diff",
        label: "Diff Plugins...",
        menu_path: "Tools/Diff Plugins...",
        shortcut: None,
        aliases: &[],
        summary: "Record-level semantic diff of two plugins",
        args_schema: r#"{"type":"object","properties":{"a":{"type":"string"},"b":{"type":"string"},"form":{"type":"string"}},"required":["a","b"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "build",
        label: "Build Patch...",
        menu_path: "File/Build Patch...",
        shortcut: Some("Ctrl+B"),
        aliases: &[],
        summary: "Compile a data-driven patch spec (TOML) into one plugin",
        args_schema: r#"{"type":"object","properties":{"spec":{"type":"string"},"out":{"type":"string"}},"required":["spec","out"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "validate",
        label: "Validate Plugin",
        menu_path: "Tools/Validate",
        shortcut: Some("F7"),
        aliases: &["check"],
        summary: "Missing masters, bad links, localized FULL copies, stripped FULL/NTRM",
        args_schema: r#"{"type":"object","properties":{"plugin":{"type":"string"}},"required":["plugin"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "roundtrip",
        label: "Roundtrip Plugin",
        menu_path: "Tools/Roundtrip",
        shortcut: None,
        aliases: &[],
        summary: "Parse and rewrite a plugin; --verify demands byte-identical output (M1 gate)",
        args_schema: r#"{"type":"object","properties":{"plugin":{"type":"string"},"verify":{"type":"boolean","default":false,"description":"Compare the rewritten bytes with the input and report the first mismatch"},"out":{"type":"string","description":"Write the rewritten plugin here; an existing file is refused unless overwrite is true. A relative path resolves against the kit process's working directory"},"overwrite":{"type":"boolean","default":false,"description":"Let out replace an existing file"}},"required":["plugin"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "coverage",
        label: "Schema Coverage",
        menu_path: "Tools/Schema Coverage",
        shortcut: None,
        aliases: &[],
        summary: "Record types and subrecords in a plugin vs what the schema defines (typed, opaque, unknown)",
        args_schema: r#"{"type":"object","properties":{"plugin":{"type":"string"},"decode":{"type":"boolean","default":true,"description":"Decompress compressed records (default limits) and count their subrecords; --no-decode skips them"},"signatures":{"type":"boolean","description":"List each record type's subrecord signatures (record_types[].signatures; null when false). Omitted: true from kit, false over MCP"},"top":{"type":"integer","minimum":0,"description":"List only the N record types with the most records; the totals still cover every type. Omitted: all from kit, 20 over MCP"}},"required":["plugin"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "ba2.list",
        label: "List Archive",
        menu_path: "Archive/List",
        shortcut: None,
        aliases: &[],
        summary: "List files in a .ba2 archive (GNRL or DX10): sizes, compression, texture formats",
        args_schema: r#"{"type":"object","properties":{"archive":{"type":"string"},"filter":{"type":"string","description":"Case-insensitive regex over archive paths"},"limit":{"type":"integer","minimum":1,"default":1000,"description":"Most entries to return"},"start":{"type":"integer","minimum":0,"default":0,"description":"Skip this many matching entries"}},"required":["archive"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "ba2.extract",
        label: "Extract from Archive...",
        menu_path: "Archive/Extract...",
        shortcut: None,
        aliases: &[],
        summary: "Extract files from a .ba2 archive (all, or those matching --pattern)",
        args_schema: r#"{"type":"object","properties":{"archive":{"type":"string"},"pattern":{"type":"string","description":"Case-insensitive regex over archive paths; omit to extract everything"},"out_dir":{"type":"string","description":"Never under the game Data directory"},"overwrite":{"type":"boolean","default":false,"description":"Replace files that already exist"},"max_entry_bytes":{"type":"integer","minimum":0,"default":1073741824,"description":"Refuse, before writing anything, an entry that extracts to more bytes than this (default 1 GiB)"}},"required":["archive","out_dir"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "ba2.pack",
        label: "Pack Archive...",
        menu_path: "Archive/Pack...",
        shortcut: None,
        aliases: &[],
        summary: "Pack a directory (as the Data root) into a .ba2 archive; deterministic",
        args_schema: r#"{"type":"object","properties":{"dir":{"type":"string"},"out":{"type":"string","description":"Never under the game Data directory"},"format":{"type":"string","enum":["gnrl","dx10"],"default":"gnrl","description":"dx10 packs .dds textures"},"version":{"type":"integer","minimum":2,"maximum":3,"default":2,"description":"2: zlib, 3: LZ4 blocks"},"compress":{"type":"boolean","default":true}},"required":["dir","out"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "papyrus.disasm",
        label: "Disassemble Script",
        menu_path: "Papyrus/Disassemble",
        shortcut: None,
        aliases: &["pexdis"],
        summary: "Disassemble a compiled Papyrus .pex (loose or inside a .ba2) as pexdis.py does, or as JSON",
        args_schema: r#"{"type":"object","properties":{"pex":{"type":"string","description":"Script file, or its path inside archive"},"archive":{"type":"string","description":"Read the script from this .ba2"},"format":{"type":"string","enum":["text","json","both"],"default":"text","description":"text: pexdis.py format; json: the structured model with offsets"}},"required":["pex"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "papyrus.compile",
        label: "Compile Script...",
        menu_path: "Papyrus/Compile...",
        shortcut: None,
        aliases: &[],
        summary: "Compile one script with Bethesda's PapyrusCompiler.exe by name with import dirs (never -release)",
        args_schema: r#"{"type":"object","properties":{"source":{"type":"string","description":"Script name (MyScript, Ns:MyScript) or a .psc path (its folder is imported first); .ppj projects are refused"},"out_dir":{"type":"string","description":"Where the .pex goes; never under the game Data directory"},"imports":{"type":"array","items":{"type":"string"},"description":"Import directories, in order (e.g. an extraction of Tools/ContentResources.zip Scripts/Source)"},"flags":{"type":"string","description":"Flags file; default: Starfield_Papyrus_Flags.flg from the first import directory that has it"},"compiler":{"type":"string","description":"PapyrusCompiler.exe; default: KITCRAFT_PAPYRUS_COMPILER, then the game's Tools folder"},"optimize":{"type":"boolean","default":false}},"required":["source","out_dir"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "strings.dump",
        label: "Dump Strings",
        menu_path: "Strings/Dump",
        shortcut: None,
        aliases: &[],
        summary: "Dump a .STRINGS/.DLSTRINGS/.ILSTRINGS table (loose or inside a .ba2), by ID or text",
        args_schema: r#"{"type":"object","properties":{"table":{"type":"string","description":"Table file, or its path inside archive"},"archive":{"type":"string","description":"Read the table from this .ba2"},"id":{"type":"string","description":"String ID, 0x-prefixed hex or decimal"},"contains":{"type":"string","description":"Case-insensitive text filter"},"limit":{"type":"integer","minimum":1,"default":100},"start":{"type":"integer","minimum":0,"default":0}},"required":["table"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "strings.merge",
        label: "Merge Strings...",
        menu_path: "Strings/Merge...",
        shortcut: None,
        aliases: &[],
        summary: "Merge string tables of one kind into one (later table wins; conflicts reported; deterministic)",
        args_schema: r#"{"type":"object","properties":{"tables":{"type":"array","items":{"type":"string"},"minItems":2},"out":{"type":"string","description":"Same extension as the inputs; never under the game Data directory"}},"required":["tables","out"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
];

#[cfg(test)]
mod tests {
    use super::{catalog, find_command, Status};
    use std::collections::BTreeSet;

    #[test]
    fn ids_and_aliases_are_unique() {
        let mut seen = BTreeSet::new();
        for spec in catalog() {
            assert!(seen.insert(spec.id), "duplicate name {}", spec.id);
            for alias in spec.aliases {
                assert!(seen.insert(*alias), "duplicate name {alias}");
            }
        }
    }

    #[test]
    fn every_args_schema_is_a_json_object_schema() {
        for spec in catalog() {
            let value: serde_json::Value = serde_json::from_str(spec.args_schema)
                .unwrap_or_else(|e| panic!("{}: args_schema is not JSON: {e}", spec.id));
            assert_eq!(value["type"], "object", "{}", spec.id);
        }
    }

    #[test]
    fn lookup_by_id_and_alias() {
        assert_eq!(find_command("ba2.list").map(|s| s.id), Some("ba2.list"));
        assert_eq!(find_command("show").map(|s| s.id), Some("inspect"));
        assert!(find_command("nope").is_none());
    }

    /// Commands still `Planned` during M2, each with the M2 task that implements it (or
    /// defers it). M2.F8: at M2 exit this list is empty and every command is
    /// `Implemented` or `Deferred`. Lanes remove their entry when they flip the status;
    /// a stale entry (already implemented) is reported by `catalog_closure_list_is_current`.
    const PLANNED_IN_M2: &[(&str, &str)] = &[
        ("load", "M2.D8"),
        ("query", "M2.D11"),
        ("set", "M2.E5"),
        ("diff", "M2.D12"),
        ("build", "M2.E9"),
        ("validate", "M2.E6"),
        ("ba2.list", "M2.G4"),
        ("ba2.extract", "M2.G4"),
        ("ba2.pack", "M2.G4"),
        ("papyrus.disasm", "M2.G6"),
        ("papyrus.compile", "M2.G7"),
        ("strings.dump", "M2.G9"),
        ("strings.merge", "M2.G9"),
    ];

    /// M2.F8: every command is `Implemented`, `Deferred` with a reason and a milestone,
    /// or `Planned` under a named M2 task.
    #[test]
    fn catalog_closure() {
        for spec in catalog() {
            match spec.status {
                Status::Implemented => {}
                Status::Deferred { reason, milestone } => {
                    assert!(!reason.trim().is_empty(), "{}: empty reason", spec.id);
                    assert!(
                        milestone.starts_with('M') && milestone.len() > 1,
                        "{}: milestone `{milestone}` is not like `M3`",
                        spec.id
                    );
                }
                Status::Planned => assert!(
                    PLANNED_IN_M2.iter().any(|(id, _)| *id == spec.id),
                    "{} is Planned but no M2 task owns it: implement it, defer it with \
                     Status::Deferred {{ reason, milestone }}, or add it to PLANNED_IN_M2",
                    spec.id
                ),
            }
        }
    }

    /// Entries of [`PLANNED_IN_M2`] name catalog commands. An entry whose command is no
    /// longer `Planned` is only printed, so the lane that flips a status does not have
    /// to edit this file in the same commit; lane F prunes the list.
    #[test]
    fn catalog_closure_list_is_current() {
        for (id, task) in PLANNED_IN_M2 {
            let spec = find_command(id).unwrap_or_else(|| panic!("{id} ({task}) not in catalog"));
            assert_eq!(spec.id, *id, "{id}: use the canonical id, not an alias");
            assert!(
                task.starts_with("M2."),
                "{id}: task `{task}` is not an M2 task"
            );
            if spec.status != Status::Planned {
                println!(
                    "PLANNED_IN_M2 entry {id} ({task}) is stale: status is {}",
                    spec.status.label()
                );
            }
        }
    }

    #[test]
    fn status_notes() {
        assert_eq!(Status::Implemented.note(), "");
        assert_eq!(Status::Planned.note(), "planned: not implemented yet");
        let deferred = Status::Deferred {
            reason: "needs the renderer",
            milestone: "M4",
        };
        assert_eq!(deferred.note(), "deferred to M4: needs the renderer");
        assert_eq!(deferred.label(), "deferred");
    }

    #[test]
    fn every_args_schema_uses_only_supported_keywords() {
        for spec in catalog() {
            let schema = crate::args::parse_schema(spec.id, spec.args_schema).unwrap();
            crate::args::check_schema(spec.id, &schema)
                .unwrap_or_else(|e| panic!("{}: {e}", spec.id));
        }
    }
}

#[cfg(test)]
mod execute_tests {
    use super::{
        catalog, execute, CommandHandler, Coverage, Engine, EngineError, Evidence, Outcome,
        Session, Status,
    };
    use serde_json::{json, Value};

    #[test]
    fn unknown_id_is_a_typed_error() {
        let err = execute("nope", json!({})).unwrap_err();
        assert!(matches!(err, EngineError::UnknownCommand(ref id) if id == "nope"));
        assert_eq!(err.kind(), "unknown_command");
    }

    #[test]
    fn schema_invalid_args_are_a_typed_error_with_pointer_and_rule() {
        let err = execute("roundtrip", json!({"verify": "yes", "bogus": 1})).unwrap_err();
        let EngineError::InvalidArgs {
            command,
            violations,
        } = &err
        else {
            panic!("expected InvalidArgs, got {err:?}");
        };
        assert_eq!(command, "roundtrip");
        let got: Vec<(&str, &str)> = violations
            .iter()
            .map(|v| (v.pointer.as_str(), v.rule))
            .collect();
        assert_eq!(
            got,
            [
                ("/plugin", "required"),
                ("/bogus", "additionalProperties"),
                ("/verify", "type"),
            ]
        );
        assert_eq!(err.kind(), "invalid_args");
        assert!(err.to_string().contains("/plugin"));
    }

    #[test]
    fn catalog_accepts_paging_and_extract_all() {
        let query = crate::find_command("query").unwrap();
        crate::prepare_args(query, json!({"type": "WEAP", "cursor": "abc"})).unwrap();
        let extract = crate::find_command("ba2.extract").unwrap();
        crate::prepare_args(extract, json!({"archive": "a.ba2", "out_dir": "o"})).unwrap();
        let coverage = crate::find_command("coverage").unwrap();
        assert_eq!(coverage.status, Status::Implemented);
    }

    #[test]
    fn args_must_be_an_object() {
        let err = execute("roundtrip", json!(["x.esm"])).unwrap_err();
        assert_eq!(err.kind(), "invalid_args");
    }

    #[test]
    fn planned_command_without_handler_is_not_implemented() {
        let err = execute("query", json!({"type": "WEAP"})).unwrap_err();
        assert!(matches!(err, EngineError::NotImplemented(ref id) if id == "query"));
        // Validation still runs first.
        assert_eq!(
            execute("query", json!({})).unwrap_err().kind(),
            "invalid_args"
        );
    }

    #[test]
    fn stub_handler_returns_the_full_envelope() {
        let mut engine = Engine::empty();
        engine
            .register(
                "validate",
                Box::new(|_: &Value| Ok(Outcome::not_implemented())),
            )
            .unwrap();
        let result = engine
            .execute("validate", json!({"plugin": "Constellation.esm"}))
            .unwrap();
        assert_eq!(
            result,
            json!({
                "command": "validate",
                "data": null,
                "coverage": {
                    "status": "unknown",
                    "decoded_subrecords": null,
                    "opaque_subrecords": null,
                    "not_examined": []
                },
                "limitations": ["not implemented"],
                "evidence": []
            })
        );
    }

    #[test]
    fn alias_resolves_to_canonical_id_in_envelope() {
        let mut engine = Engine::empty();
        engine
            .register(
                "inspect",
                Box::new(|_: &Value| Ok(Outcome::not_implemented())),
            )
            .unwrap();
        let result = engine
            .execute("show", json!({"form": "0001A2B3", "plugin": "X.esm"}))
            .unwrap();
        assert_eq!(result["command"], "inspect");
    }

    struct Echo;
    impl CommandHandler for Echo {
        fn run(&self, args: &Value) -> Result<Outcome, EngineError> {
            Ok(Outcome {
                data: args.clone(),
                coverage: Coverage::unknown(),
                limitations: Vec::new(),
                evidence: vec![Evidence {
                    file: "X.esm".into(),
                    offset: 24,
                    length: 6,
                    record: Some("WEAP:0001A2B3".into()),
                    subrecord: Some("FULL".into()),
                }],
            })
        }
    }

    #[test]
    fn handlers_see_defaults_and_evidence_is_serialized() {
        let mut engine = Engine::empty();
        engine.register("roundtrip", Box::new(Echo)).unwrap();
        let result = engine
            .execute("roundtrip", json!({"plugin": "X.esm"}))
            .unwrap();
        assert_eq!(
            result["data"],
            json!({"plugin": "X.esm", "verify": false, "overwrite": false})
        );
        assert_eq!(result["limitations"], json!([]));
        assert_eq!(
            result["evidence"],
            json!([{"file": "X.esm", "offset": 24, "length": 6,
                    "record": "WEAP:0001A2B3", "subrecord": "FULL"}])
        );
    }

    #[test]
    fn null_args_mean_empty_object() {
        let mut engine = Engine::empty();
        engine.register("load", Box::new(Echo)).unwrap();
        let result = engine.execute("load", Value::Null).unwrap();
        assert_eq!(result["data"], json!({}));
    }

    #[test]
    fn registration_rejects_unknown_and_duplicate_ids() {
        let mut engine = Engine::empty();
        assert_eq!(
            engine.register("nope", Box::new(Echo)).unwrap_err().kind(),
            "registration"
        );
        assert_eq!(
            engine.register("show", Box::new(Echo)).unwrap_err().kind(),
            "registration"
        );
        engine.register("inspect", Box::new(Echo)).unwrap();
        assert_eq!(
            engine
                .register("inspect", Box::new(Echo))
                .unwrap_err()
                .kind(),
            "registration"
        );
    }

    #[test]
    fn handler_failures_propagate() {
        let mut engine = Engine::empty();
        engine
            .register(
                "validate",
                Box::new(|_: &Value| -> Result<Outcome, EngineError> {
                    Err(EngineError::Failed {
                        command: "validate".into(),
                        message: "bad record at offset 0x18".into(),
                    })
                }),
            )
            .unwrap();
        let err = engine
            .execute("validate", json!({"plugin": "X.esm"}))
            .unwrap_err();
        assert_eq!(err.kind(), "failed");
    }

    /// A session handler that counts its calls in a session slot and mirrors the count in
    /// the load-order listing.
    fn counter(session: &mut Session, _: &Value) -> Result<Outcome, EngineError> {
        struct Calls(u64);
        let calls = match session.get_mut::<Calls>() {
            Some(calls) => {
                calls.0 = calls.0.saturating_add(1);
                calls.0
            }
            None => {
                session.insert(Calls(1));
                1
            }
        };
        session.set_load_order(Some(json!({ "calls": calls })));
        Ok(Outcome {
            data: json!({ "calls": calls }),
            ..Outcome::not_implemented()
        })
    }

    #[test]
    fn session_handler_keeps_state_across_calls_on_one_engine() {
        let mut engine = Engine::empty();
        engine.register_session("load", Box::new(counter)).unwrap();
        engine.register("validate", Box::new(Echo)).unwrap();
        assert!(engine.has_handler("load"));
        assert_eq!(
            engine.execute("load", json!({})).unwrap()["data"]["calls"],
            1
        );
        // A stateless command in between leaves the session alone.
        engine
            .execute("validate", json!({"plugin": "X.esm"}))
            .unwrap();
        assert_eq!(
            engine.execute("open", json!({})).unwrap()["data"]["calls"],
            2
        );
        let listing = engine.with_session(|s| s.load_order().cloned());
        assert_eq!(listing, Some(json!({"calls": 2})));

        // A caller-owned session is separate from the engine's.
        let mut fresh = Session::new();
        assert!(fresh.is_empty());
        let result = engine.execute_in(&mut fresh, "load", json!({})).unwrap();
        assert_eq!(result["data"]["calls"], 1);
        assert!(!fresh.is_empty());
        fresh.clear();
        assert!(fresh.is_empty());
        assert_eq!(
            engine.execute("load", json!({})).unwrap()["data"]["calls"],
            3
        );
    }

    #[test]
    fn session_slots_are_typed() {
        let mut session = Session::new();
        assert_eq!(session.insert(5u32), None);
        assert_eq!(session.insert(7u32), Some(5));
        session.insert(String::from("x"));
        assert_eq!(session.get::<u32>(), Some(&7));
        assert_eq!(session.get::<String>().map(String::as_str), Some("x"));
        assert_eq!(session.get::<u64>(), None);
        assert_eq!(session.remove::<u32>(), Some(7));
        assert_eq!(session.get::<u32>(), None);
    }

    #[test]
    fn deferred_commands_are_refused_with_their_reason() {
        let spec = crate::CommandSpec {
            id: "render",
            label: "Render",
            menu_path: "View/Render",
            shortcut: None,
            aliases: &[],
            summary: "Render a cell",
            args_schema: r#"{"type":"object"}"#,
            status: Status::Deferred {
                reason: "needs crates/render",
                milestone: "M4",
            },
        };
        let err = crate::exec::refuse_deferred(&spec).unwrap_err();
        assert_eq!(err.kind(), "deferred");
        assert_eq!(
            err.to_string(),
            "`render` is deferred to M4: needs crates/render"
        );
        let implemented = crate::find_command("roundtrip").unwrap();
        assert!(crate::exec::refuse_deferred(implemented).is_ok());
    }

    #[test]
    fn default_handlers_cover_every_implemented_command() {
        let engine = Engine::with_default_handlers();
        assert!(engine.has_handler("roundtrip"));
        assert!(engine.has_handler("inspect"));
        assert!(engine.has_handler("coverage"));
        for spec in catalog() {
            assert_eq!(
                spec.status == Status::Implemented,
                engine.has_handler(spec.id),
                "{}: status and handler disagree",
                spec.id
            );
        }
    }
}
