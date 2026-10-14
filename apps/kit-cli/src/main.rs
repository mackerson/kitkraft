//! `kit`: the command-line front end.
//!
//! clap owns the shell: `--json`, `--version`, `--help`, the `mcp` launcher, and one
//! subcommand per catalog word (generated from the engine catalog, so typos get clap's
//! suggestions). The words after the command go to the engine's own command-line parser
//! (`kitcraft_engine::parse_words`, shared with the MCP `command_line` tool), and the
//! result to `kitcraft_engine::execute`. No format or editing logic lives here.
//!
//! Exit codes: 0 success or help; 1 the command ran and its check failed (`data.passed`
//! is `false`: a `roundtrip --verify` mismatch, later validation findings); 2 usage error,
//! unknown command, invalid arguments, a command that is not implemented yet (including
//! a stub result from a `Planned` command, and a `Deferred` one), or a failed command.
//! `kit mcp [--profile DIR]` serves MCP on stdio until stdin closes (exit 0; 2 on an I/O
//! error); a failed `--profile` preload is reported on stderr and in the server's
//! instructions, and the server starts with an empty session.
//!
//! Text output prints `data.summary` when the command provides one, otherwise `data` as
//! JSON, then one `limitation:` line per limitation. `--json` prints the envelope as is.
//!
//! LAYER: APP

use std::collections::BTreeSet;
use std::process::ExitCode;

use clap::{Arg, ArgAction, Command};
use kitcraft_engine::{
    catalog, cmdline, find_command, CommandSpec, EngineError, Invocation, Status,
};
use serde_json::{json, Value};

const EXIT_NEGATIVE: u8 = 1;
const EXIT_ERROR: u8 = 2;

fn main() -> ExitCode {
    let matches = match cli().try_get_matches() {
        Ok(matches) => matches,
        Err(err) => {
            // Help and version print to stdout and exit 0; real errors exit 2.
            let _ = err.print();
            return if err.use_stderr() {
                ExitCode::from(EXIT_ERROR)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    let json = matches.get_flag("json");
    if matches.get_flag("help") {
        print_catalog_help();
        return ExitCode::SUCCESS;
    }
    match matches.subcommand() {
        None => {
            print_catalog_help();
            ExitCode::SUCCESS
        }
        Some(("mcp", sub)) => run_mcp(sub.get_one::<String>("profile").map(String::as_str)),
        Some((word, sub)) => {
            let mut words = vec![word.to_string()];
            words.extend(
                sub.get_many::<String>("words")
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
            let (words, json_after) = strip_json_flag(words);
            run(&words, json || json_after)
        }
    }
}

/// `kit mcp`: one engine (one session) for the life of the process, served on stdio.
/// stdout carries only MCP messages; everything human goes to stderr.
fn run_mcp(profile: Option<&str>) -> ExitCode {
    let mut server = kitcraft_mcp::Server::new(kitcraft_engine::Engine::with_default_handlers());
    if let Some(profile) = profile {
        match server
            .backend()
            .engine()
            .execute("load", json!({ "profile": profile }))
        {
            Ok(_) => {
                eprintln!("kit mcp: loaded profile {profile}");
                server.add_note(format!(
                    "The profile {profile} was loaded at startup (see kitcraft://load-order)."
                ));
            }
            Err(err) => {
                eprintln!(
                    "kit mcp: preloading --profile failed: {err}; serving with an empty session"
                );
                server.add_note(format!(
                    "Preloading the profile {profile} at startup failed ({err}); the session is empty."
                ));
            }
        }
    }
    match kitcraft_mcp::serve_stdio(&mut server) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kit mcp: stdio error: {err}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// The clap shell. Catalog commands take their words raw; the engine parses them.
fn cli() -> Command {
    let mut cmd = Command::new("kit")
        .bin_name("kit")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Open-source Creation Kit tooling for Starfield (pre-alpha)")
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Print the result envelope (or the error) as JSON"),
        )
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue)
                .help("Print the command catalog"),
        )
        .subcommand(
            Command::new("mcp")
                .about("Serve the same catalog over MCP on stdio (see docs/mcp.md)")
                .arg(
                    Arg::new("profile")
                        .long("profile")
                        .value_name("DIR")
                        .help("MO2 profile to load (through `load`) before serving"),
                ),
        )
        .subcommand(passthrough(
            "help",
            "Print the catalog, or one command's usage".to_string(),
        ));
    for (word, about) in catalog_words() {
        cmd = cmd.subcommand(passthrough(word, about));
    }
    cmd
}

/// A subcommand that hands every following word to the engine parser.
fn passthrough(name: &'static str, about: String) -> Command {
    Command::new(name).about(about).disable_help_flag(true).arg(
        Arg::new("words")
            .num_args(0..)
            .trailing_var_arg(true)
            .allow_hyphen_values(true)
            .value_parser(clap::value_parser!(String)),
    )
}

/// One clap subcommand per first word of a catalog id, plus one per alias.
fn catalog_words() -> Vec<(&'static str, String)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for spec in catalog() {
        let first = spec.id.split('.').next().unwrap_or(spec.id);
        if seen.insert(first) {
            let about = if first == spec.id {
                spec.summary.to_string()
            } else {
                let subs: Vec<&str> = catalog()
                    .iter()
                    .filter_map(|s| s.id.strip_prefix(first)?.strip_prefix('.'))
                    .collect();
                format!("{first} commands: {}", subs.join(", "))
            };
            out.push((first, about));
        }
    }
    for spec in catalog() {
        for alias in spec.aliases {
            if seen.insert(alias) {
                out.push((
                    *alias,
                    format!("alias of `{}`", cmdline::command_words(spec)),
                ));
            }
        }
    }
    out
}

/// Removes `--json` words placed after the command (before any `--`).
fn strip_json_flag(words: Vec<String>) -> (Vec<String>, bool) {
    let mut found = false;
    let mut after_separator = false;
    let mut out = Vec::with_capacity(words.len());
    for word in words {
        if word == "--" {
            after_separator = true;
        }
        if !after_separator && word == "--json" {
            found = true;
            continue;
        }
        out.push(word);
    }
    (out, found)
}

fn run(words: &[String], json: bool) -> ExitCode {
    let invocation = match kitcraft_engine::parse_words(words) {
        Ok(invocation) => invocation,
        Err(err) => return fail(&err, json),
    };
    let (spec, args) = match invocation {
        Invocation::Help { command: None } => {
            print_catalog_help();
            return ExitCode::SUCCESS;
        }
        Invocation::Help {
            command: Some(spec),
        } => {
            return match cmdline::command_help(spec, "kit ") {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(err) => fail(&err, json),
            };
        }
        Invocation::Run { command, args } => (command, args),
    };
    match kitcraft_engine::execute(spec.id, args) {
        Ok(result) => {
            print_result(&result, json);
            let code = result_code(spec, &result);
            if spec.status == Status::Planned {
                eprintln!(
                    "kit: `{}` is planned but not implemented yet; the result is a stub",
                    cmdline::command_words(spec)
                );
            } else if code == EXIT_NEGATIVE {
                eprintln!("kit: `{}` check failed", cmdline::command_words(spec));
            }
            ExitCode::from(code)
        }
        Err(err) => fail(&err, json),
    }
}

/// Exit code for a result `execute` returned: a `Planned` command's stub is an error, a
/// result whose `data.passed` is `false` is a negative result, anything else is success.
fn result_code(spec: &CommandSpec, result: &Value) -> u8 {
    if spec.status == Status::Planned {
        return EXIT_ERROR;
    }
    let passed = result.get("data").and_then(|d| d.get("passed"));
    if passed == Some(&Value::Bool(false)) {
        EXIT_NEGATIVE
    } else {
        0
    }
}

fn print_result(result: &Value, json: bool) {
    if json {
        println!("{}", to_pretty(result));
        return;
    }
    if let Some(data) = result.get("data").filter(|d| !d.is_null()) {
        match data.get("summary").and_then(Value::as_str) {
            Some(summary) => println!("{summary}"),
            None => println!("{}", to_pretty(data)),
        }
    }
    let limitations = result.get("limitations").and_then(Value::as_array);
    for limitation in limitations.into_iter().flatten() {
        let text = limitation
            .as_str()
            .map_or_else(|| limitation.to_string(), str::to_string);
        println!("limitation: {text}");
    }
}

fn fail(err: &EngineError, json: bool) -> ExitCode {
    if json {
        println!("{}", to_pretty(&error_json(err)));
    }
    eprintln!("kit: {err}");
    if let Some(spec) = failing_command(err) {
        if matches!(
            err,
            EngineError::Usage { .. } | EngineError::InvalidArgs { .. }
        ) {
            eprintln!("run `kit help {}` for usage", cmdline::command_words(spec));
        }
    } else if matches!(
        err,
        EngineError::UnknownCommand(_) | EngineError::Usage { .. }
    ) {
        eprintln!("run `kit help` for the command list");
    }
    ExitCode::from(EXIT_ERROR)
}

fn failing_command(err: &EngineError) -> Option<&'static CommandSpec> {
    let id = match err {
        EngineError::Usage {
            command: Some(id), ..
        }
        | EngineError::InvalidArgs { command: id, .. } => id.as_str(),
        _ => return None,
    };
    find_command(id)
}

fn error_json(err: &EngineError) -> Value {
    let mut body = json!({"kind": err.kind(), "message": err.to_string()});
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

fn to_pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn print_catalog_help() {
    println!(
        "kit {} - open-source Creation Kit tooling for Starfield (pre-alpha)",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("USAGE: kit [--json] <command> [args]");
    println!("       kit help <command>    usage of one command");
    println!();
    println!("COMMANDS:");
    print!("{}", cmdline::catalog_help());
    println!(
        "  {:<18} Serve the same catalog over MCP on stdio (kit mcp [--profile DIR])",
        "mcp"
    );
    println!();
    println!("Every command is also an MCP tool call and a future menu item.");
    println!("Exit codes: 0 ok, 1 check failed (e.g. roundtrip --verify mismatch), 2 error.");
}

#[cfg(test)]
mod tests {
    use super::{result_code, EXIT_ERROR, EXIT_NEGATIVE};
    use kitcraft_engine::find_command;
    use serde_json::json;

    #[test]
    fn exit_code_follows_status_and_passed() {
        let roundtrip = find_command("roundtrip").unwrap();
        let planned = find_command("ba2.list").unwrap();
        let with = |passed| json!({"data": {"passed": passed}});
        assert_eq!(result_code(roundtrip, &with(json!(true))), 0);
        assert_eq!(result_code(roundtrip, &with(json!(null))), 0);
        assert_eq!(result_code(roundtrip, &json!({"data": null})), 0);
        assert_eq!(result_code(roundtrip, &with(json!(false))), EXIT_NEGATIVE);
        assert_eq!(result_code(planned, &with(json!(true))), EXIT_ERROR);
    }
}
