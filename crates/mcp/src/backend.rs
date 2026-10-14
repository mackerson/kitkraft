//! Maps MCP tool calls and resource reads onto the engine. Every tool resolves to
//! `Engine::execute` on the server's one engine (and so its one session); results are the
//! engine's evidence envelope, returned both as `structuredContent` and as JSON text.
//!
//! Error mapping (`docs/mcp.md`, "Errors"):
//!
//! | Engine error kind                                       | MCP                          |
//! | ------------------------------------------------------- | ---------------------------- |
//! | `unknown_command`, `usage`, `invalid_args`              | tool result, `isError: true` |
//! | `not_implemented`, `deferred`, `failed`                 | tool result, `isError: true` |
//! | `bad_schema`, `registration`, `serialize` (engine bugs) | JSON-RPC `-32603`            |
//! | (no such MCP tool)                                      | JSON-RPC `-32602`            |
//!
//! Argument errors are tool results, not protocol errors, so the model sees the JSON
//! pointer and rule and can correct its call (MCP 2026-07-28, "Tools", "Error Handling":
//! input validation errors are tool execution errors).

use kitcraft_engine::{
    args, catalog, cmdline, find_command, parse_line, Engine, EngineError, Invocation,
};
use serde_json::{json, Value};

use crate::tools::{self, Target, Tool};

/// URI of the catalog resource.
pub const COMMANDS_URI: &str = "kitcraft://commands";
/// URI of the session's load-order resource.
pub const LOAD_ORDER_URI: &str = "kitcraft://load-order";

/// Engine error kinds that are engine bugs, not caller mistakes.
const INTERNAL_KINDS: &[&str] = &["bad_schema", "registration", "serialize"];

/// What a tool call produced: the `CallToolResult` minus protocol framing.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    /// `isError`: the call ran into a caller-visible error (bad arguments, a failed
    /// command). A check that ran and failed (`data.passed: false`) is not an error.
    pub is_error: bool,
    /// `structuredContent`: the envelope, `{"error": {...}}`, or `{"help": text}`.
    pub structured: Value,
    /// The text content block.
    pub text: String,
}

/// A tool call the protocol layer must reject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// No tool has this name (`-32602`).
    UnknownTool(String),
    /// An engine bug (`-32603`).
    Internal(String),
}

/// The engine behind the server.
#[derive(Debug)]
pub struct Backend {
    engine: Engine,
}

impl Backend {
    /// A backend over `engine` (normally `Engine::with_default_handlers()`).
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }

    /// The engine (for preloading through `load` before serving).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Runs tool `name` with `arguments` (`null` means `{}`).
    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutput, CallError> {
        let tool =
            tools::find_tool(name).ok_or_else(|| CallError::UnknownTool(name.to_string()))?;
        let arguments = if arguments.is_null() {
            Value::Object(serde_json::Map::new())
        } else {
            arguments
        };
        let result = match tool.target {
            Target::Command(id) => self.engine.execute(id, arguments),
            Target::Execute => self.run_execute(tool, arguments),
            Target::CommandLine => match self.run_command_line(tool, arguments) {
                Ok(Line::Help(text)) => {
                    return Ok(ToolOutput {
                        is_error: false,
                        structured: json!({ "help": text }),
                        text,
                    })
                }
                Ok(Line::Ran(envelope)) => Ok(envelope),
                Err(err) => Err(err),
            },
        };
        match result {
            Ok(envelope) => Ok(ToolOutput {
                is_error: false,
                text: envelope.to_string(),
                structured: envelope,
            }),
            Err(err) if INTERNAL_KINDS.contains(&err.kind()) => {
                Err(CallError::Internal(err.to_string()))
            }
            Err(err) => {
                let structured = error_json(&err);
                Ok(ToolOutput {
                    is_error: true,
                    text: structured.to_string(),
                    structured,
                })
            }
        }
    }

    fn run_execute(&self, tool: &Tool, mut arguments: Value) -> Result<Value, EngineError> {
        // `args: null` means `{}` (as for every command), so it is not a type violation.
        if let Some(obj) = arguments.as_object_mut() {
            if obj.get("args").is_some_and(Value::is_null) {
                obj.insert("args".into(), Value::Object(serde_json::Map::new()));
            }
        }
        check_tool_args(tool, &arguments)?;
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let args = arguments.get("args").cloned().unwrap_or(Value::Null);
        self.engine.execute(command, args)
    }

    fn run_command_line(&self, tool: &Tool, arguments: Value) -> Result<Line, EngineError> {
        check_tool_args(tool, &arguments)?;
        let text = arguments
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match parse_line(text)? {
            Invocation::Run { command, args } => {
                self.engine.execute(command.id, args).map(Line::Ran)
            }
            Invocation::Help { command: None } => Ok(Line::Help(format!(
                "usage: <command> [args]   (help <command> for one command)\n\ncommands:\n{}",
                cmdline::catalog_help()
            ))),
            Invocation::Help {
                command: Some(spec),
            } => cmdline::command_help(spec, "").map(Line::Help),
        }
    }

    /// The `resources/list` entries.
    pub fn resources(&self) -> Vec<Value> {
        vec![
            json!({
                "uri": COMMANDS_URI,
                "name": "commands",
                "title": "Command catalog",
                "description": "Every KitCraft command: id, label, aliases, summary, args_schema, status",
                "mimeType": "application/json"
            }),
            json!({
                "uri": LOAD_ORDER_URI,
                "name": "load-order",
                "title": "Session load order",
                "description": "The plugins this server's session has loaded (with `execute load`), with masters and indices",
                "mimeType": "application/json"
            }),
        ]
    }

    /// The text of resource `uri`, or `None` when there is no such resource.
    pub fn read_resource(&self, uri: &str) -> Option<Result<String, CallError>> {
        let value = match uri {
            COMMANDS_URI => tools::catalog_json().map_err(|e| CallError::Internal(e.to_string())),
            LOAD_ORDER_URI => Ok(self.engine.with_session(|session| {
                session.load_order().cloned().unwrap_or_else(|| {
                    json!({
                        "loaded": false,
                        "plugins": [],
                        "note": "no load order yet: call `execute` with command `load`"
                    })
                })
            })),
            _ => return None,
        };
        Some(value.map(|v| v.to_string()))
    }

    /// Number of catalog commands (for the server's instructions).
    pub fn command_count(&self) -> usize {
        catalog().len()
    }
}

enum Line {
    Ran(Value),
    Help(String),
}

/// Validates `execute`/`command_line` arguments against the tool's own `inputSchema`
/// with the engine's validator, so a bad call reads like a bad command call.
fn check_tool_args(tool: &Tool, arguments: &Value) -> Result<(), EngineError> {
    let schema = tools::input_schema(tool)?;
    let mut violations = args::validate(&schema, arguments);
    if tool.target == Target::Execute {
        if let Some(command) = arguments.get("command").and_then(Value::as_str) {
            if find_command(command).is_none() {
                // An unknown id is reported as such, not as an enum dump, whatever else
                // is wrong with the call.
                return Err(EngineError::UnknownCommand(command.to_string()));
            }
            // The enum lists canonical ids; aliases (`show`) are accepted too.
            violations.retain(|v| !(v.pointer == "/command" && v.rule == "enum"));
        }
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(EngineError::InvalidArgs {
        command: tool.name.to_string(),
        violations,
    })
}

/// `{"error": {"kind", "message", "violations"?}}`, the shape `kit --json` prints.
fn error_json(err: &EngineError) -> Value {
    let mut body = json!({ "kind": err.kind(), "message": err.to_string() });
    if let EngineError::InvalidArgs { violations, .. } = err {
        let list: Vec<Value> = violations
            .iter()
            .map(|v| json!({"pointer": v.pointer, "rule": v.rule, "message": v.message}))
            .collect();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("violations".to_string(), Value::Array(list));
        }
    }
    json!({ "error": body })
}

#[cfg(test)]
mod tests {
    use super::{Backend, ToolOutput};
    use kitcraft_engine::Engine;
    use serde_json::json;

    fn backend() -> Backend {
        Backend::new(Engine::with_default_handlers())
    }

    fn error_kind(out: &ToolOutput) -> &str {
        assert!(out.is_error, "{}", out.text);
        out.structured["error"]["kind"].as_str().unwrap_or_default()
    }

    /// `execute {command, args: null}` is `{}` args, not a type violation of `args`.
    #[test]
    fn execute_null_args_mean_empty_object() {
        let b = backend();
        let null = b
            .call_tool("execute", json!({"command": "roundtrip", "args": null}))
            .unwrap();
        let omitted = b
            .call_tool("execute", json!({"command": "roundtrip"}))
            .unwrap();
        // Both reach the command and fail its own schema (`plugin` is required).
        assert_eq!(error_kind(&null), "invalid_args");
        assert_eq!(null.structured, omitted.structured);
        assert_eq!(
            null.structured["error"]["violations"][0]["pointer"],
            "/plugin"
        );
        // A command without required arguments runs (here: the planned stub's error).
        let out = b
            .call_tool("execute", json!({"command": "load", "args": null}))
            .unwrap();
        assert_eq!(error_kind(&out), "not_implemented");
        // Any other non-object is still a violation of the tool's own schema.
        let out = b
            .call_tool("execute", json!({"command": "load", "args": [1]}))
            .unwrap();
        assert_eq!(error_kind(&out), "invalid_args");
        assert_eq!(out.structured["error"]["violations"][0]["pointer"], "/args");
    }

    /// An unknown command is `unknown_command` even when the call has other faults,
    /// never an `invalid_args` dump of the catalog enum.
    #[test]
    fn unknown_command_wins_over_other_violations() {
        let b = backend();
        for arguments in [
            json!({"command": "frobnicate"}),
            json!({"command": "frobnicate", "args": "x"}),
            json!({"command": "frobnicate", "args": {}, "extra": 1}),
        ] {
            let out = b.call_tool("execute", arguments.clone()).unwrap();
            assert_eq!(error_kind(&out), "unknown_command", "{arguments}");
            assert!(out.structured["error"].get("violations").is_none());
            assert!(!out.text.contains("papyrus"), "no enum dump: {}", out.text);
        }
        // A known alias with an extra member is an argument error on the tool.
        let out = b
            .call_tool("execute", json!({"command": "show", "extra": 1}))
            .unwrap();
        assert_eq!(error_kind(&out), "invalid_args");
        assert_eq!(
            out.structured["error"]["violations"][0]["pointer"],
            "/extra"
        );
    }
}
