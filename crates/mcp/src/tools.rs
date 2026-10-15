//! MCP tool shapes, derived from the engine catalog. The power is in the command catalog,
//! not the tool count: `execute` reaches every command, and the shortcuts exist because
//! agents call those commands constantly.
//!
//! Nothing here is hand-maintained per command. `execute`'s `command` enum and its
//! description list the catalog as it is, and a shortcut's `inputSchema` is its command's
//! `args_schema` verbatim, so a new or changed catalog command needs no edit in this crate.

use kitkraft_engine::{catalog, find_command, params, CommandSpec, EngineError, Param, Status};
use serde_json::{json, Map, Value};

/// What a tool call runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Any catalog command by id: `{command, args}`.
    Execute,
    /// A `kit`-style command line: `{text}`, parsed by `kitkraft_engine::parse_line`.
    CommandLine,
    /// One catalog command; the tool's arguments are the command's arguments.
    Command(&'static str),
}

/// One MCP tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tool {
    /// Tool name in `tools/list` and `tools/call`.
    pub name: &'static str,
    /// Human-readable title.
    pub title: &'static str,
    /// What it runs.
    pub target: Target,
    /// Never writes a file (`readOnlyHint`). `execute`, `command_line` and `roundtrip`
    /// (which writes with `out`) can.
    pub read_only: bool,
    /// May replace or change an existing file (`destructiveHint`): `execute` and
    /// `command_line` reach `set`, `build` and the archive writers, `roundtrip` replaces
    /// its `out` file when `overwrite` is `true`. `false` only for read-only tools.
    pub destructive: bool,
}

/// Tools exposed by `kit mcp`, in the order `tools/list` returns them. `render` comes later.
pub const TOOLS: &[Tool] = &[
    Tool {
        name: "execute",
        title: "Run a KitKraft command",
        target: Target::Execute,
        read_only: false,
        destructive: true,
    },
    Tool {
        name: "command_line",
        title: "Run a kit command line",
        target: Target::CommandLine,
        read_only: false,
        destructive: true,
    },
    Tool {
        name: "inspect_record",
        title: "Inspect a record",
        target: Target::Command("inspect"),
        read_only: true,
        destructive: false,
    },
    Tool {
        name: "query_records",
        title: "Query records",
        target: Target::Command("query"),
        read_only: true,
        destructive: false,
    },
    Tool {
        name: "diff_plugins",
        title: "Diff two plugins",
        target: Target::Command("diff"),
        read_only: true,
        destructive: false,
    },
    Tool {
        name: "coverage",
        title: "Schema coverage of a plugin",
        target: Target::Command("coverage"),
        read_only: true,
        destructive: false,
    },
    Tool {
        name: "record_schema",
        title: "Record schema",
        target: Target::Command("schema"),
        read_only: true,
        destructive: false,
    },
    Tool {
        name: "roundtrip",
        title: "Roundtrip a plugin",
        target: Target::Command("roundtrip"),
        read_only: false,
        destructive: true,
    },
];

/// The tool named `name`.
pub fn find_tool(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.name == name)
}

/// The catalog entry a shortcut tool runs.
fn spec_of(tool: &Tool, id: &str) -> Result<&'static CommandSpec, EngineError> {
    find_command(id).ok_or_else(|| {
        EngineError::Registration(tool.name.to_string(), "shortcut names no catalog command")
    })
}

/// The tool's `inputSchema`.
pub fn input_schema(tool: &Tool) -> Result<Value, EngineError> {
    match tool.target {
        Target::Execute => {
            let ids: Vec<Value> = catalog()
                .iter()
                .map(|s| Value::String(s.id.to_string()))
                .collect();
            Ok(json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "enum": ids,
                        "description": "Catalog command id; the tool description lists each with its arguments"
                    },
                    "args": {
                        "type": "object",
                        "description": "Arguments matching the command's args_schema (resource kitkraft://commands); omit for none"
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }))
        }
        Target::CommandLine => Ok(json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "minLength": 1,
                    "description": "A kit command line without the program name, e.g. `coverage C:\\Data\\Plugin.esm --no-decode` or `help inspect`; quotes group words, no backslash escapes"
                }
            },
            "required": ["text"],
            "additionalProperties": false
        })),
        Target::Command(id) => {
            let spec = spec_of(tool, id)?;
            serde_json::from_str(spec.args_schema).map_err(|e| EngineError::BadSchema {
                command: spec.id.to_string(),
                message: format!("not JSON: {e}"),
            })
        }
    }
}

/// The tool's description: for a shortcut, its command's summary and status; for
/// `execute`, one line per catalog command with its arguments.
pub fn description(tool: &Tool) -> Result<String, EngineError> {
    match tool.target {
        Target::Execute => {
            let mut text = String::from(
                "Run any KitKraft catalog command by id with JSON args; returns the evidence envelope {command, data, coverage, limitations, evidence}. Commands (`?` = optional):",
            );
            for spec in catalog() {
                text.push_str(&format!("\n- {} {}: {}", spec.id, signature(spec)?, summary(spec)));
            }
            Ok(text)
        }
        Target::CommandLine => Ok(
            "Run a kit command line (same parser as the `kit` CLI): `<command words> <positional args> [--flag value]`. `help` or `help <command>` returns usage text instead of running. Returns the evidence envelope."
                .to_string(),
        ),
        Target::Command(id) => {
            let spec = spec_of(tool, id)?;
            Ok(format!(
                "{} Same as `execute` with command `{}`; returns the evidence envelope.",
                summary(spec),
                spec.id
            ))
        }
    }
}

/// `{plugin, verify?, out?}`: argument names, optional ones marked.
fn signature(spec: &CommandSpec) -> Result<String, EngineError> {
    let names: Vec<String> = params(spec)?
        .iter()
        .map(|p: &Param| {
            if p.required {
                p.name.clone()
            } else {
                format!("{}?", p.name)
            }
        })
        .collect();
    Ok(format!("{{{}}}", names.join(", ")))
}

/// The summary with the status note when the command is not implemented.
fn summary(spec: &CommandSpec) -> String {
    match spec.status {
        Status::Implemented => spec.summary.to_string(),
        Status::Planned | Status::Deferred { .. } => {
            format!("{} ({})", spec.summary, spec.status.note())
        }
    }
}

/// One `tools/list` entry.
pub fn definition(tool: &Tool) -> Result<Value, EngineError> {
    let mut def = Map::new();
    def.insert("name".into(), Value::String(tool.name.to_string()));
    def.insert("title".into(), Value::String(tool.title.to_string()));
    def.insert("description".into(), Value::String(description(tool)?));
    def.insert("inputSchema".into(), input_schema(tool)?);
    def.insert(
        "annotations".into(),
        json!({
            "title": tool.title,
            "readOnlyHint": tool.read_only,
            "destructiveHint": tool.destructive,
            "openWorldHint": false
        }),
    );
    Ok(Value::Object(def))
}

/// Every `tools/list` entry, in [`TOOLS`] order.
pub fn definitions() -> Result<Vec<Value>, EngineError> {
    TOOLS.iter().map(definition).collect()
}

/// The MCP tools that run catalog command `id`: `execute` and `command_line` reach every
/// command; a shortcut reaches its own.
pub fn tools_for(id: &str) -> Vec<&'static str> {
    TOOLS
        .iter()
        .filter(|t| match t.target {
            Target::Execute | Target::CommandLine => true,
            Target::Command(c) => c == id,
        })
        .map(|t| t.name)
        .collect()
}

/// The `kitkraft://commands` resource: the catalog with parsed argument schemas, the CLI
/// words and the MCP tools of each command.
pub fn catalog_json() -> Result<Value, EngineError> {
    let mut out = Vec::new();
    for spec in catalog() {
        let schema: Value =
            serde_json::from_str(spec.args_schema).map_err(|e| EngineError::BadSchema {
                command: spec.id.to_string(),
                message: format!("not JSON: {e}"),
            })?;
        let (reason, milestone) = match spec.status {
            Status::Deferred { reason, milestone } => (Some(reason), Some(milestone)),
            Status::Planned | Status::Implemented => (None, None),
        };
        out.push(json!({
            "id": spec.id,
            "label": spec.label,
            "menu_path": spec.menu_path,
            "shortcut": spec.shortcut,
            "aliases": spec.aliases,
            "summary": spec.summary,
            "args_schema": schema,
            "status": spec.status.label(),
            "deferred_reason": reason,
            "deferred_to": milestone,
            "cli": format!("kit {}", kitkraft_engine::cmdline::command_words(spec)),
            "tools": tools_for(spec.id),
        }));
    }
    Ok(Value::Array(out))
}

#[cfg(test)]
mod tests {
    use super::{catalog_json, definitions, description, find_tool, input_schema, Target, TOOLS};
    use kitkraft_engine::{catalog, find_command, Status};
    use serde_json::Value;

    #[test]
    fn tool_names_are_unique_and_well_formed() {
        let mut names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TOOLS.len());
        for tool in TOOLS {
            assert!(
                tool.name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')),
                "{}",
                tool.name
            );
            assert_eq!(find_tool(tool.name), Some(tool));
        }
        assert!(find_tool("nope").is_none());
    }

    /// Annotations are honest: a tool that can write is not read-only and may be
    /// destructive; a read-only tool never is.
    #[test]
    fn annotations_match_what_tools_write() {
        let defs = definitions().unwrap();
        for (tool, def) in TOOLS.iter().zip(&defs) {
            let a = &def["annotations"];
            assert_eq!(a["readOnlyHint"], tool.read_only, "{}", tool.name);
            assert_eq!(a["destructiveHint"], !tool.read_only, "{}", tool.name);
        }
        for name in ["execute", "command_line", "roundtrip"] {
            assert!(find_tool(name).unwrap().destructive, "{name}");
        }
    }

    /// Every shortcut's schema is its catalog entry's `args_schema`, and its description
    /// is the entry's summary.
    #[test]
    fn shortcut_schemas_match_the_catalog() {
        for tool in TOOLS {
            let Target::Command(id) = tool.target else {
                continue;
            };
            let spec =
                find_command(id).unwrap_or_else(|| panic!("{}: {id} not in catalog", tool.name));
            assert_eq!(spec.id, id, "{}: use the canonical id", tool.name);
            let expected: Value = serde_json::from_str(spec.args_schema).unwrap();
            assert_eq!(input_schema(tool).unwrap(), expected, "{}", tool.name);
            let text = description(tool).unwrap();
            assert!(text.starts_with(spec.summary), "{}: {text}", tool.name);
            if spec.status != Status::Implemented {
                assert!(text.contains(&spec.status.note()), "{}: {text}", tool.name);
            }
        }
    }

    /// `execute`'s enum is the catalog, in catalog order, and its description names
    /// every command: adding a command changes both with no edit here.
    #[test]
    fn execute_enumerates_the_catalog() {
        let execute = find_tool("execute").unwrap();
        let schema = input_schema(execute).unwrap();
        let ids: Vec<&str> = schema["properties"]["command"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let expected: Vec<&str> = catalog().iter().map(|s| s.id).collect();
        assert_eq!(ids, expected);
        let text = description(execute).unwrap();
        for spec in catalog() {
            assert!(text.contains(&format!("\n- {} {{", spec.id)), "{}", spec.id);
        }
        assert!(
            text.contains("- roundtrip {plugin, data_dir?, decode?, out?, overwrite?, verify?}"),
            "{text}"
        );
    }

    /// Tool schemas only use keywords the engine's validator checks, so the server can
    /// validate tool arguments with it.
    #[test]
    fn tool_schemas_are_valid_and_checkable() {
        let defs = definitions().unwrap();
        assert_eq!(defs.len(), TOOLS.len());
        for (tool, def) in TOOLS.iter().zip(&defs) {
            assert_eq!(def["name"], tool.name);
            assert_eq!(def["inputSchema"]["type"], "object", "{}", tool.name);
            assert!(def["description"].as_str().is_some_and(|d| !d.is_empty()));
            kitkraft_engine::args::check_schema(tool.name, &def["inputSchema"])
                .unwrap_or_else(|e| panic!("{}: {e}", tool.name));
        }
    }

    #[test]
    fn catalog_resource_lists_every_command() {
        let list = catalog_json().unwrap();
        let list = list.as_array().unwrap();
        assert_eq!(list.len(), catalog().len());
        assert_eq!(list[0]["id"], catalog()[0].id);
        let roundtrip = list.iter().find(|c| c["id"] == "roundtrip").unwrap();
        assert_eq!(roundtrip["status"], "implemented");
        assert_eq!(roundtrip["args_schema"]["required"][0], "plugin");
        assert_eq!(roundtrip["deferred_reason"], Value::Null);
        assert_eq!(roundtrip["cli"], "kit roundtrip");
        assert_eq!(
            roundtrip["tools"],
            serde_json::json!(["execute", "command_line", "roundtrip"])
        );
        let extract = list.iter().find(|c| c["id"] == "ba2.extract").unwrap();
        assert_eq!(extract["cli"], "kit ba2 extract");
        assert_eq!(
            extract["tools"],
            serde_json::json!(["execute", "command_line"])
        );
        let schema = list.iter().find(|c| c["id"] == "schema").unwrap();
        assert!(schema["tools"].to_string().contains("record_schema"));
    }
}
