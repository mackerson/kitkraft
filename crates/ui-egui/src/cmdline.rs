//! The `kit` command line and the MCP `execute` call that produce the same envelope as a
//! panel's request, shown under every result so the workbench teaches the CLI.
//!
//! The text is written for the shared grammar (`kitkraft_engine::parse_line`, docs/mcp.md
//! "`command_line` grammar"): every argument as `--name value` (`_` written `-`), booleans
//! as `--name` / `--no-name`, string lists by repeating the flag, objects as JSON text.
//! Quotes group words and there are no escapes, so a value holding both quote characters
//! cannot be written exactly ([`CommandLine::exact`] is then false). A test parses every
//! line back with the engine's parser.

use kitkraft_engine::{cmdline::command_words, find_command};
use serde_json::{json, Value};

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
    use kitkraft_engine::{parse_line, Invocation};

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

    #[test]
    fn mcp_call_uses_the_canonical_id() {
        assert_eq!(
            mcp_call("defs", &json!({"type": "GLOB"})),
            r#"{"args":{"type":"GLOB"},"command":"schema"}"#
        );
    }
}
