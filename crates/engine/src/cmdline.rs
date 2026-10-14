//! The command-line grammar shared by `kit` and the MCP `command_line` tool: words in,
//! `(command, JSON arguments)` out. Generated from each command's `args_schema` via
//! [`crate::params`], so a new command needs no parser changes.
//!
//! Grammar, per command:
//!
//! - Command words: `roundtrip`, `ba2 list` (or the dotted id `ba2.list`), or an alias.
//! - Required string/integer/number arguments are positional, in the schema's
//!   `required` order: `roundtrip <plugin>`, `ba2 extract <archive> <out_dir>`.
//! - Every argument is also a flag: `--name value` or `--name=value`; `_` in a name may
//!   be written `-` (`--out-dir`).
//! - Booleans: `--name` is `true`, `--no-name` is `false`, `--name=true|false` works too.
//! - String lists repeat the flag: `--tables a.STRINGS --tables b.STRINGS`.
//! - Object (and other) arguments take JSON text: `--fields '{"FULL":"Rifle"}'`.
//! - `--` ends flags; everything after it is positional.
//! - `-h` / `--help` anywhere, or `help [command...]`, asks for help instead of running.
//!
//! The parser only shapes JSON. Missing required arguments and value constraints are
//! reported by schema validation in [`crate::execute`], so every front end gets the same
//! JSON-pointer errors.
//!
//! [`split_words`] tokenizes one line for `command_line`: whitespace separates words,
//! single or double quotes group them. There are no backslash escapes, so Windows paths
//! need no doubling (`"C:\Modding\My Mod.esm"`); put a quote character inside the other
//! kind of quotes.

use serde_json::{Map, Number, Value};

use crate::params::{params, Param, ParamKind};
use crate::{catalog, find_command, CommandSpec, EngineError, Status};

/// What a command line asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Invocation {
    /// Run `command` with `args` (not yet schema-validated; pass to `execute`).
    Run {
        /// The resolved catalog entry.
        command: &'static CommandSpec,
        /// The arguments object.
        args: Value,
    },
    /// Show help: the catalog (`None`) or one command.
    Help {
        /// The command asked about, if any.
        command: Option<&'static CommandSpec>,
    },
}

/// Splits one line into words (see the module docs for quoting).
pub fn split_words(text: &str) -> Result<Vec<String>, EngineError> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for ch in text.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                in_word = true;
            }
            None if ch.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            None => {
                current.push(ch);
                in_word = true;
            }
        }
    }
    if let Some(q) = quote {
        return Err(usage(None, format!("unterminated {q} quote")));
    }
    if in_word {
        words.push(current);
    }
    Ok(words)
}

/// Parses one line: [`split_words`] then [`parse_words`].
pub fn parse_line(text: &str) -> Result<Invocation, EngineError> {
    parse_words(&split_words(text)?)
}

/// Parses command words and arguments (without the program name).
pub fn parse_words<S: AsRef<str>>(words: &[S]) -> Result<Invocation, EngineError> {
    let words: Vec<&str> = words.iter().map(AsRef::as_ref).collect();
    match words.split_first() {
        None => Ok(Invocation::Help { command: None }),
        Some((&("-h" | "--help"), _)) => Ok(Invocation::Help { command: None }),
        Some((&"help", rest)) => {
            if rest.is_empty() {
                return Ok(Invocation::Help { command: None });
            }
            let (spec, used) = resolve(rest)?;
            match rest.get(used..) {
                Some([]) | None => Ok(Invocation::Help {
                    command: Some(spec),
                }),
                Some(extra) => Err(usage(
                    Some(spec),
                    format!("`help` takes one command, got extra `{}`", extra.join(" ")),
                )),
            }
        }
        Some(_) => {
            let (spec, used) = resolve(&words)?;
            let rest = words.get(used..).unwrap_or_default();
            parse_args(spec, rest)
        }
    }
}

/// Resolves the command words at the start of `words`; returns the spec and how many
/// words named it.
fn resolve(words: &[&str]) -> Result<(&'static CommandSpec, usize), EngineError> {
    let Some(first) = words.first() else {
        return Err(usage(None, "no command given"));
    };
    if let Some(second) = words.get(1).filter(|w| !w.starts_with('-')) {
        if let Some(spec) = find_command(&format!("{first}.{second}")) {
            return Ok((spec, 2));
        }
    }
    if let Some(spec) = find_command(first) {
        return Ok((spec, 1));
    }
    let prefix = format!("{first}.");
    let subs: Vec<&str> = catalog()
        .iter()
        .filter_map(|spec| spec.id.strip_prefix(prefix.as_str()))
        .collect();
    if subs.is_empty() {
        return Err(EngineError::UnknownCommand((*first).to_string()));
    }
    let what = match words.get(1) {
        Some(second) => format!("unknown subcommand `{first} {second}`"),
        None => format!("`{first}` needs a subcommand"),
    };
    Err(usage(None, format!("{what}; one of: {}", subs.join(", "))))
}

fn parse_args(spec: &'static CommandSpec, words: &[&str]) -> Result<Invocation, EngineError> {
    let params = params(spec)?;
    let positional: Vec<&Param> = params.iter().filter(|p| is_positional(p)).collect();
    let mut next_positional = positional.iter();
    let mut map = Map::new();
    let mut flags_done = false;
    let mut iter = words.iter();
    while let Some(&word) = iter.next() {
        if !flags_done && word == "--" {
            flags_done = true;
            continue;
        }
        if !flags_done && (word == "-h" || word == "--help") {
            return Ok(Invocation::Help {
                command: Some(spec),
            });
        }
        if let Some(body) = word.strip_prefix("--").filter(|_| !flags_done) {
            let (name, inline) = match body.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (body, None),
            };
            let (param, negated) = lookup(&params, name)
                .ok_or_else(|| usage(Some(spec), format!("unknown option `--{name}`")))?;
            let value = if param.kind == ParamKind::Boolean {
                match (negated, inline) {
                    (true, None) => Value::Bool(false),
                    (false, None | Some("true")) => Value::Bool(true),
                    (false, Some("false")) => Value::Bool(false),
                    (_, Some(other)) => {
                        return Err(usage(
                            Some(spec),
                            format!("`--{name}` takes true or false, got `{other}`"),
                        ))
                    }
                }
            } else {
                let raw = match inline {
                    Some(raw) => raw,
                    None => iter
                        .next()
                        .copied()
                        .ok_or_else(|| usage(Some(spec), format!("`--{name}` needs a value")))?,
                };
                convert(spec, param, raw)?
            };
            insert(spec, &mut map, param, value)?;
            continue;
        }
        if !flags_done && word.len() > 1 && word.starts_with('-') && word.parse::<f64>().is_err() {
            return Err(usage(Some(spec), format!("unknown option `{word}`")));
        }
        let param = next_positional
            .next()
            .ok_or_else(|| usage(Some(spec), format!("unexpected argument `{word}`")))?;
        let value = convert(spec, param, word)?;
        insert(spec, &mut map, param, value)?;
    }
    Ok(Invocation::Run {
        command: spec,
        args: Value::Object(map),
    })
}

fn is_positional(param: &Param) -> bool {
    param.required
        && matches!(
            param.kind,
            ParamKind::String | ParamKind::Integer | ParamKind::Number
        )
}

/// Finds the parameter a `--flag` names; the bool is true for a `--no-` negation.
fn lookup<'p>(params: &'p [Param], flag: &str) -> Option<(&'p Param, bool)> {
    let wanted = flag.replace('-', "_");
    if let Some(param) = params.iter().find(|p| p.name == wanted) {
        return Some((param, false));
    }
    let negated = wanted.strip_prefix("no_")?;
    params
        .iter()
        .find(|p| p.name == negated && p.kind == ParamKind::Boolean)
        .map(|p| (p, true))
}

fn convert(spec: &CommandSpec, param: &Param, raw: &str) -> Result<Value, EngineError> {
    let bad = |what: &str| {
        usage(
            Some(spec),
            format!("`{}` expects {what}, got `{raw}`", param.name),
        )
    };
    match param.kind {
        ParamKind::String | ParamKind::StringList => Ok(Value::String(raw.to_string())),
        ParamKind::Integer => raw
            .parse::<i64>()
            .map(|n| Value::Number(n.into()))
            .map_err(|_| bad("an integer")),
        ParamKind::Number => raw
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| bad("a number")),
        ParamKind::Boolean => match raw {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(bad("true or false")),
        },
        ParamKind::Json => serde_json::from_str(raw).map_err(|_| bad("JSON")),
    }
}

fn insert(
    spec: &CommandSpec,
    map: &mut Map<String, Value>,
    param: &Param,
    value: Value,
) -> Result<(), EngineError> {
    if param.kind == ParamKind::StringList {
        let entry = map
            .entry(param.name.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(items) = entry {
            items.push(value);
        }
        return Ok(());
    }
    if map.contains_key(&param.name) {
        return Err(usage(
            Some(spec),
            format!("`{}` given more than once", param.name),
        ));
    }
    map.insert(param.name.clone(), value);
    Ok(())
}

fn usage(spec: Option<&CommandSpec>, message: impl Into<String>) -> EngineError {
    EngineError::Usage {
        command: spec.map(|s| s.id.to_string()),
        message: message.into(),
    }
}

/// The words that invoke `spec`: `ba2.list` -> `ba2 list`.
pub fn command_words(spec: &CommandSpec) -> String {
    spec.id.replace('.', " ")
}

/// One line per catalog command: words, summary, and `(planned)` when not callable.
pub fn catalog_help() -> String {
    let mut out = String::new();
    for spec in catalog() {
        let state = match spec.status {
            Status::Planned => " (planned)",
            Status::Implemented => "",
        };
        out.push_str(&format!(
            "  {:<18} {}{state}\n",
            command_words(spec),
            spec.summary
        ));
    }
    out
}

/// Usage and argument list for one command. `program` prefixes the usage line (`"kit "`
/// for the CLI, `""` for `command_line`).
pub fn command_help(spec: &CommandSpec, program: &str) -> Result<String, EngineError> {
    let params = params(spec)?;
    let mut line = format!("usage: {program}{}", command_words(spec));
    let mut rows: Vec<(String, String)> = Vec::new();
    for param in &params {
        let flag = param.name.replace('_', "-");
        let (synopsis, row) = if is_positional(param) {
            (format!("<{}>", param.name), format!("<{}>", param.name))
        } else {
            let value = match param.kind {
                ParamKind::Boolean => String::new(),
                ParamKind::StringList => format!(" <{}>...", param.name),
                ParamKind::Json => " <json>".to_string(),
                _ if !param.choices.is_empty() => format!(" <{}>", param.choices.join("|")),
                _ => format!(" <{}>", param.name),
            };
            let s = format!("--{flag}{value}");
            if param.required {
                (s.clone(), s)
            } else {
                (format!("[{s}]"), s)
            }
        };
        line.push(' ');
        line.push_str(&synopsis);
        let mut text = param.description.clone().unwrap_or_default();
        if let Some(default) = &param.default {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&format!("(default: {default})"));
        }
        rows.push((row, text));
    }
    let mut out = format!("{line}\n\n{}", spec.summary);
    if spec.status == Status::Planned {
        out.push_str(" (planned: not implemented yet)");
    }
    out.push('\n');
    if !spec.aliases.is_empty() {
        out.push_str(&format!("aliases: {}\n", spec.aliases.join(", ")));
    }
    if !rows.is_empty() {
        out.push_str("\narguments:\n");
        for (row, text) in rows {
            let line = format!("  {row:<24} {text}");
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{command_help, parse_line, parse_words, split_words, Invocation};
    use crate::{execute, find_command};
    use serde_json::{json, Value};

    fn run(line: &str) -> (&'static str, Value) {
        match parse_line(line).unwrap() {
            Invocation::Run { command, args } => (command.id, args),
            other => panic!("expected Run, got {other:?}"),
        }
    }

    fn kind(line: &str) -> &'static str {
        parse_line(line).unwrap_err().kind()
    }

    #[test]
    fn split_handles_quotes_and_windows_paths() {
        assert_eq!(
            split_words(r#"roundtrip "C:\Modding\My Mod.esm"  --out='a b' x"#).unwrap(),
            [r"roundtrip", r"C:\Modding\My Mod.esm", "--out=a b", "x"]
        );
        assert_eq!(
            split_words(r#"a "" 'say "hi"'"#).unwrap(),
            ["a", "", r#"say "hi""#]
        );
        assert_eq!(split_words("   ").unwrap(), Vec::<String>::new());
        assert_eq!(split_words("a \"b").unwrap_err().kind(), "usage");
    }

    #[test]
    fn positional_flags_and_booleans() {
        assert_eq!(
            run("roundtrip X.esm"),
            ("roundtrip", json!({"plugin": "X.esm"}))
        );
        assert_eq!(
            run("roundtrip X.esm --verify --out Y.esm"),
            (
                "roundtrip",
                json!({"plugin": "X.esm", "verify": true, "out": "Y.esm"})
            )
        );
        assert_eq!(run("roundtrip --no-verify X.esm").1["verify"], json!(false));
        assert_eq!(
            run("roundtrip X.esm --verify=false").1["verify"],
            json!(false)
        );
        assert_eq!(
            run("roundtrip --plugin=X.esm").1,
            json!({"plugin": "X.esm"})
        );
        assert_eq!(
            run("roundtrip -- -odd.esm").1,
            json!({"plugin": "-odd.esm"})
        );
    }

    #[test]
    fn subcommands_aliases_and_dotted_ids() {
        assert_eq!(
            run("ba2 extract A.ba2 out --pattern \\.nif$"),
            (
                "ba2.extract",
                json!({"archive": "A.ba2", "out_dir": "out", "pattern": "\\.nif$"})
            )
        );
        assert_eq!(run("ba2.list A.ba2").0, "ba2.list");
        assert_eq!(
            run("show 0001A2B3"),
            ("inspect", json!({"form": "0001A2B3"}))
        );
        assert_eq!(run("pexdis x.pex").0, "papyrus.disasm");
        assert_eq!(run("ba2 extract A.ba2 --out-dir o").1["out_dir"], "o");
    }

    #[test]
    fn typed_values() {
        assert_eq!(
            run("query WEAP --limit 5").1,
            json!({"type": "WEAP", "limit": 5})
        );
        assert_eq!(
            run(r#"set 0001A2B3 out.esm --fields '{"FULL":"Rifle"}'"#).1,
            json!({"form": "0001A2B3", "into": "out.esm", "fields": {"FULL": "Rifle"}})
        );
        assert_eq!(
            run("strings merge --tables a --tables b --out c").1,
            json!({"tables": ["a", "b"], "out": "c"})
        );
        assert_eq!(kind("query WEAP --limit five"), "usage");
        assert_eq!(kind("set F out.esm --fields {nope"), "usage");
    }

    #[test]
    fn usage_errors() {
        assert_eq!(kind("roundtrip X.esm Y.esm"), "usage");
        assert_eq!(kind("roundtrip X.esm --bogus 1"), "usage");
        assert_eq!(kind("roundtrip X.esm -x"), "usage");
        assert_eq!(kind("roundtrip X.esm --out"), "usage");
        assert_eq!(kind("roundtrip X.esm --verify=maybe"), "usage");
        assert_eq!(kind("roundtrip X.esm --plugin Y.esm"), "usage");
        assert_eq!(kind("ba2"), "usage");
        assert_eq!(kind("ba2 frobnicate"), "usage");
        assert_eq!(kind("frobnicate"), "unknown_command");
        assert_eq!(kind("help frobnicate"), "unknown_command");
    }

    #[test]
    fn missing_required_is_left_to_schema_validation() {
        let (id, args) = run("roundtrip");
        let err = execute(id, args).unwrap_err();
        assert_eq!(err.kind(), "invalid_args");
    }

    #[test]
    fn help_requests() {
        assert_eq!(
            parse_words::<&str>(&[]).unwrap(),
            Invocation::Help { command: None }
        );
        assert_eq!(
            parse_line("help").unwrap(),
            Invocation::Help { command: None }
        );
        let roundtrip = find_command("roundtrip");
        assert_eq!(
            parse_line("help roundtrip").unwrap(),
            Invocation::Help { command: roundtrip }
        );
        assert_eq!(
            parse_line("roundtrip --help").unwrap(),
            Invocation::Help { command: roundtrip }
        );
        assert_eq!(
            parse_line("help ba2 list").unwrap(),
            Invocation::Help {
                command: find_command("ba2.list")
            }
        );
    }

    #[test]
    fn every_command_has_help_and_parses_its_own_synopsis_shape() {
        for spec in crate::catalog() {
            let text = command_help(spec, "kit ").unwrap();
            assert!(text.starts_with(&format!("usage: kit {}", spec.id.replace('.', " "))));
        }
        let text = command_help(find_command("roundtrip").unwrap(), "kit ").unwrap();
        assert!(
            text.contains("usage: kit roundtrip <plugin> [--out <out>] [--verify]"),
            "{text}"
        );
        assert!(text.contains("(default: false)"), "{text}");
    }

    #[test]
    fn line_result_equals_execute_with_json() {
        let (id, args) = run("roundtrip Constellation.esm");
        let spec = find_command(id).unwrap();
        assert_eq!(
            crate::prepare_args(spec, args).unwrap(),
            crate::prepare_args(spec, json!({"plugin": "Constellation.esm"})).unwrap()
        );
        // Same envelope (here the same failure) either way.
        let (id, args) = run("roundtrip no-such-dir/Constellation.esm");
        assert_eq!(
            execute(id, args).unwrap_err().to_string(),
            execute(
                "roundtrip",
                json!({"plugin": "no-such-dir/Constellation.esm"})
            )
            .unwrap_err()
            .to_string()
        );
    }
}
