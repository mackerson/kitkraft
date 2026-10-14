//! The command engine.
//!
//! Every user-visible action is one [`CommandSpec`] in [`catalog()`]. Menus, toolbar,
//! scripts, the CLI and the MCP server all resolve a name to the same spec and call the
//! same `execute`, so behaviour cannot drift between front ends. `execute` takes its
//! arguments as JSON matching `args_schema` and never opens a dialog; interactive
//! front ends gather arguments first, then call it.
//!
//! Sessions, undo, and validation rules land here in M2 (see `plan/execution-plan.md`).
//!
//! LAYER: L4

/// Whether a catalog entry does anything yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Listed so every front end agrees on the name and arguments; not callable yet.
    Planned,
    /// Callable from every front end.
    Implemented,
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
        summary: "Resolved record, winning override, and every overrider",
        args_schema: r#"{"type":"object","properties":{"form":{"type":"string","description":"FormID (hex, load-order or file-relative) or EditorID"}},"required":["form"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "query",
        label: "Query Records...",
        menu_path: "Record/Query...",
        shortcut: Some("Ctrl+F"),
        aliases: &["find"],
        summary: "Find records by type and field predicate",
        args_schema: r#"{"type":"object","properties":{"type":{"type":"string","minLength":4,"maxLength":4},"where":{"type":"string"},"limit":{"type":"integer","minimum":1}},"required":["type"],"additionalProperties":false}"#,
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
        args_schema: r#"{"type":"object","properties":{"plugin":{"type":"string"},"verify":{"type":"boolean","default":true},"out":{"type":"string"}},"required":["plugin"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "ba2.list",
        label: "List Archive",
        menu_path: "Archive/List",
        shortcut: None,
        aliases: &[],
        summary: "List files in a .ba2 archive",
        args_schema: r#"{"type":"object","properties":{"archive":{"type":"string"},"filter":{"type":"string"}},"required":["archive"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "ba2.extract",
        label: "Extract from Archive...",
        menu_path: "Archive/Extract...",
        shortcut: None,
        aliases: &[],
        summary: "Extract files matching a regex from a .ba2 archive",
        args_schema: r#"{"type":"object","properties":{"archive":{"type":"string"},"pattern":{"type":"string"},"out_dir":{"type":"string"}},"required":["archive","pattern","out_dir"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "ba2.pack",
        label: "Pack Archive...",
        menu_path: "Archive/Pack...",
        shortcut: None,
        aliases: &[],
        summary: "Pack a directory into a .ba2 archive",
        args_schema: r#"{"type":"object","properties":{"dir":{"type":"string"},"out":{"type":"string"},"format":{"type":"string","enum":["gnrl","dx10"]}},"required":["dir","out"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "papyrus.disasm",
        label: "Disassemble Script",
        menu_path: "Papyrus/Disassemble",
        shortcut: None,
        aliases: &["pexdis"],
        summary: "Disassemble a compiled Papyrus .pex",
        args_schema: r#"{"type":"object","properties":{"pex":{"type":"string"}},"required":["pex"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "papyrus.compile",
        label: "Compile Script...",
        menu_path: "Papyrus/Compile...",
        shortcut: None,
        aliases: &[],
        summary: "Compile .psc with Bethesda's compiler (never -release)",
        args_schema: r#"{"type":"object","properties":{"source":{"type":"string"},"out_dir":{"type":"string"},"imports":{"type":"array","items":{"type":"string"}}},"required":["source","out_dir"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "strings.dump",
        label: "Dump Strings",
        menu_path: "Strings/Dump",
        shortcut: None,
        aliases: &[],
        summary: "Dump a .STRINGS/.DLSTRINGS/.ILSTRINGS table",
        args_schema: r#"{"type":"object","properties":{"table":{"type":"string"}},"required":["table"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
    CommandSpec {
        id: "strings.merge",
        label: "Merge Strings...",
        menu_path: "Strings/Merge...",
        shortcut: None,
        aliases: &[],
        summary: "Merge string tables into one",
        args_schema: r#"{"type":"object","properties":{"tables":{"type":"array","items":{"type":"string"},"minItems":2},"out":{"type":"string"}},"required":["tables","out"],"additionalProperties":false}"#,
        status: Status::Planned,
    },
];

#[cfg(test)]
mod tests {
    use super::{catalog, find_command};
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
}
