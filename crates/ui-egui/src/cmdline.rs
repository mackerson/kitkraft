//! The `kit` command line and the MCP `execute` call that produce the same envelope as a
//! panel's request, shown under every result so the workbench teaches the CLI.
//!
//! The text is written for the shared grammar (the engine's `parse_line`, docs/mcp.md
//! "`command_line` grammar"): every argument as `--name value` (`_` written `-`), booleans
//! as `--name` / `--no-name`, string lists by repeating the flag, objects as JSON text.
//! Quotes group words and there are no escapes, so a value holding both quote characters
//! cannot be written exactly ([`CommandLine::exact`] is then false). A test parses every
//! line back with the engine's parser.

use crate::gate::{command_words, find_command};
use serde_json::{json, Map, Value};

/// Commands whose `kit` line needs the session's load order repeated: each `kit` process
/// starts with an empty session, while the workbench (like `kit mcp`) keeps one.
pub const SESSION_COMMANDS: &[&str] = &["inspect", "query", "conflicts", "validate"];

/// The load-order arguments of an executed `load` request, as a session command's line
/// repeats them (`profile`, `plugins`, `data_dir`).
pub fn session_args(load_args: &Value) -> Map<String, Value> {
    let mut args = Map::new();
    for key in ["profile", "plugins", "data_dir"] {
        if let Some(v) = load_args.get(key).filter(|v| !v.is_null()) {
            args.insert(key.into(), v.clone());
        }
    }
    args
}

/// The line shown under one result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultLine {
    /// The `kit` line, or `None` when no line would reproduce the result: a session
    /// command run while no `load` had succeeded.
    pub line: Option<CommandLine>,
    /// Load-order arguments were added from the session's `load`.
    pub session_added: bool,
}

/// The `kit` line for an executed request. `session` holds the arguments of the `load`
/// that had succeeded last when the request ran (the session it ran against), never the
/// load form's current text, which may have been edited since.
pub fn result_line(id: &str, args: &Value, session: Option<&Map<String, Value>>) -> ResultLine {
    let canonical = find_command(id).map_or(id, |spec| spec.id);
    if !SESSION_COMMANDS.contains(&canonical) {
        return ResultLine {
            line: Some(command_line(id, args)),
            session_added: false,
        };
    }
    let Some(session) = session else {
        return ResultLine {
            line: None,
            session_added: false,
        };
    };
    let mut merged = args.clone();
    if let Some(map) = merged.as_object_mut() {
        for (k, v) in session {
            map.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    ResultLine {
        line: Some(command_line(id, &merged)),
        session_added: !session.is_empty(),
    }
}

/// A command line for one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    /// `kit inspect --form GameYear ...`.
    pub text: String,
    /// Whether `text` parses back to exactly the request's arguments.
    pub exact: bool,
}

/// The `kit` command line for `id` with `args` (a JSON object; `null` members skipped).
pub fn command_line(id: &str, args: &Value) -> CommandLine {
    let words = find_command(id).map_or_else(|| id.replace('.', " "), command_words);
    let mut text = format!("kit {words}");
    let mut exact = true;
    if let Some(map) = args.as_object() {
        for (name, value) in map {
            let flag = name.replace('_', "-");
            match value {
                Value::Null => {}
                Value::Bool(true) => text.push_str(&format!(" --{flag}")),
                Value::Bool(false) => text.push_str(&format!(" --no-{flag}")),
                Value::Number(n) => text.push_str(&format!(" --{flag} {n}")),
                Value::String(s) => {
                    let (word, ok) = quote(s);
                    exact &= ok;
                    text.push_str(&format!(" --{flag} {word}"));
                }
                Value::Array(items) if items.iter().all(Value::is_string) => {
                    for item in items.iter().filter_map(Value::as_str) {
                        let (word, ok) = quote(item);
                        exact &= ok;
                        text.push_str(&format!(" --{flag} {word}"));
                    }
                }
                other => {
                    let (word, ok) = quote(&other.to_string());
                    exact &= ok;
                    text.push_str(&format!(" --{flag} {word}"));
                }
            }
        }
    }
    CommandLine { text, exact }
}

/// The MCP `execute` tool call for the same request (docs/mcp.md, "Tools").
pub fn mcp_call(id: &str, args: &Value) -> String {
    let id = find_command(id).map_or(id, |spec| spec.id);
    let call = json!({ "command": id, "args": args });
    serde_json::to_string(&call).unwrap_or_default()
}

/// One word: bare when it needs no quotes, else in the quote character it does not hold.
fn quote(s: &str) -> (String, bool) {
    let plain = !s.is_empty()
        && !s
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\'');
    if plain {
        return (s.to_string(), true);
    }
    if !s.contains('"') {
        (format!("\"{s}\""), true)
    } else if !s.contains('\'') {
        (format!("'{s}'"), true)
    } else {
        (format!("\"{s}\""), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{parse_line, Invocation};

    fn parses_back(id: &str, args: Value) {
        let line = command_line(id, &args);
        assert!(line.exact, "{}", line.text);
        let text = line.text.strip_prefix("kit ").unwrap();
        match parse_line(text).unwrap() {
            Invocation::Run {
                command,
                args: parsed,
            } => {
                assert_eq!(command.id, find_command(id).unwrap().id);
                let mut want = args.clone();
                want.as_object_mut().unwrap().retain(|_, v| !v.is_null());
                assert_eq!(parsed, want, "{}", line.text);
            }
            Invocation::Help { .. } => panic!("help: {}", line.text),
        }
    }

    #[test]
    fn lines_parse_back_to_the_same_arguments() {
        parses_back(
            "load",
            json!({"profile": r"C:\Modding\MO2-Starfield\profiles\KitKraft Dev", "no_cache": false}),
        );
        parses_back(
            "load",
            json!({"plugins": ["Starfield.esm", "My Mod.esp"], "data_dir": r"D:\Game Data"}),
        );
        parses_back(
            "query",
            json!({"type": "WEAP", "where": "FULL ~ \"Rifle\" and not has KWDA", "scope": "all", "limit": 50}),
        );
        parses_back("query", json!({"type": "KYWD", "where": "@edid ^= 'Ammo'"}));
        parses_back(
            "inspect",
            json!({"form": "Starfield.esm:01A2B3", "plugin": null}),
        );
        parses_back("schema", json!({"type": "WEAP", "deep": true}));
        parses_back(
            "validate",
            json!({"plugin": "out.esp", "rules": ["masters.missing", "formid.dangling"]}),
        );
        parses_back("roundtrip", json!({"plugin": "A.esm", "verify": true}));
        parses_back("diff", json!({"a": "A.esm", "b": "B.esm", "limit": 10}));
        parses_back(
            "conflicts",
            json!({"plugin": "Mod.esp", "only_conflicts": true, "count_classes": false}),
        );
    }

    #[test]
    fn a_value_with_both_quotes_is_marked_inexact() {
        let line = command_line("query", &json!({"type": "WEAP", "where": "a ~ \"x'y\""}));
        assert!(!line.exact);
    }

    /// The line repeats the load that ran, not the load form as edited since, and a
    /// session command gets no line at all when no load succeeded.
    #[test]
    fn session_lines_use_the_executed_load_or_none() {
        let executed =
            json!({"profile": r"C:\MO2\profiles\KitKraft Dev", "no_cache": true, "data_dir": null});
        let session = session_args(&executed);
        assert_eq!(
            Value::Object(session.clone()),
            json!({"profile": r"C:\MO2\profiles\KitKraft Dev"})
        );
        let shown = result_line("find", &json!({"type": "WEAP"}), Some(&session));
        let line = shown.line.unwrap();
        assert!(shown.session_added);
        assert!(line.text.starts_with("kit query "), "{}", line.text);
        assert!(line.text.contains("--type WEAP"), "{}", line.text);
        assert!(
            line.text
                .contains(r#"--profile "C:\MO2\profiles\KitKraft Dev""#),
            "{}",
            line.text
        );
        assert!(!line.text.contains("no-cache"), "{}", line.text);
        // An argument the request itself gives wins over the session's.
        let own = result_line(
            "inspect",
            &json!({"form": "A.esm:000800", "profile": "P"}),
            Some(&session),
        );
        assert!(own.line.unwrap().text.ends_with("--profile P"));

        let none = result_line("query", &json!({"type": "WEAP"}), None);
        assert_eq!(none.line, None);
        assert!(!none.session_added);
        // Commands that take no session keep their line.
        let schema = result_line("schema", &json!({"type": "WEAP"}), None);
        assert_eq!(schema.line.unwrap().text, "kit schema --type WEAP");
        let load = result_line("load", &executed, None);
        assert!(load.line.unwrap().text.starts_with("kit load"));
    }

    #[test]
    fn mcp_call_uses_the_canonical_id() {
        assert_eq!(
            mcp_call("defs", &json!({"type": "GLOB"})),
            r#"{"args":{"type":"GLOB"},"command":"schema"}"#
        );
    }
}
