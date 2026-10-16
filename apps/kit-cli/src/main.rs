//! `kit`: the command-line front end.
//!
//! clap owns the shell: `--json`, `--version`, `--help`, the `mcp` launcher, and one
//! subcommand per catalog word (generated from the engine catalog, so typos get clap's
//! suggestions). The words after the command go to the engine's own command-line parser
//! (`kitkraft_engine::parse_words`, shared with the MCP `command_line` tool), and the
//! result to `kitkraft_engine::execute`. No format or editing logic lives here.
//!
//! Exit codes: 0 success or help; 1 the command ran and its check failed (`data.passed`
//! is `false`: a `roundtrip --verify` mismatch, later validation findings); 2 usage error,
//! unknown command, invalid arguments, a command that is not implemented yet (including
//! a stub result from a `Planned` command, and a `Deferred` one), or a failed command.
//! `kit mcp [--profile DIR]` serves MCP on stdio until stdin closes (exit 0; 2 on an I/O
//! error); a failed `--profile` preload is reported on stderr and in the server's
//! instructions, and the server starts with an empty session.
//! `kit ui [--dev | --profile DIR] [--plugins FILE...] [--data-dir DIR] [--theme PRESET]`
//! starts the read-only workbench window (`kit-ui`, built from `apps/kit`, next to this
//! binary) with those arguments and returns its exit code; 2 when `kit-ui` is not there.
//! `--dev` opens the KitKraft Dev profile `kit doctor` finds (else the window's profile
//! picker); `--theme` picks the visual preset (`kit-ui --help`).
//! `kit install` / `kit uninstall [--dry-run | --apply] [--home DIR] ...` print, and with
//! `--apply` make, the PATH and MCP registration changes of `kitkraft_engine::install`
//! (front-end setup like `mcp` and `ui`, so no `CommandSpec` and not reachable over MCP);
//! exit 0, or 2 on an error. `--apply` stops (exit 2, nothing changed) when the plan
//! rewrites `.claude.json` while Claude Code runs, unless `--force-while-running`;
//! `--check-running` only says whether it runs (exit 1 when it does).
//!
//! Text output prints `data.summary` when the command provides one, otherwise `data` as
//! JSON, then one `limitation:` line per limitation. `--json` prints the envelope as is.
//!
//! LAYER: APP

use std::collections::BTreeSet;
use std::process::ExitCode;

use clap::{Arg, ArgAction, Command};
use kitkraft_engine::{
    catalog, cmdline, find_command, install, CommandSpec, EngineError, Invocation, Status,
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
        Some(("ui", sub)) => run_ui(&ui_words(
            sub.get_many::<String>("words")
                .into_iter()
                .flatten()
                .cloned()
                .collect(),
        )),
        Some(("install", sub)) => run_install(sub, install::Action::Install),
        Some(("uninstall", sub)) => run_install(sub, install::Action::Uninstall),
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
    let mut server = kitkraft_mcp::Server::new(kitkraft_engine::Engine::with_default_handlers());
    if let Some(profile) = profile {
        match server
            .backend()
            .engine()
            .execute("load", json!({ "profile": profile }))
        {
            Ok(_) => {
                eprintln!("kit mcp: loaded profile {profile}");
                server.add_note(format!(
                    "The profile {profile} was loaded at startup (see kitkraft://load-order)."
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
    match kitkraft_mcp::serve_stdio(&mut server) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kit mcp: stdio error: {err}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// `kit ui --dev`: the `--dev` word becomes `--profile <the KitKraft Dev profile>` when
/// `kit doctor` finds one; without one the window opens on its profile picker. Other
/// words pass through unchanged.
fn ui_words(words: Vec<String>) -> Vec<String> {
    if !words.iter().any(|w| w == "--dev") {
        return words;
    }
    let mut out: Vec<String> = words.into_iter().filter(|w| w != "--dev").collect();
    let dev = kitkraft_engine::execute("doctor", json!({"get": "dev_profile"}))
        .ok()
        .and_then(|r| {
            r.pointer("/data/value")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    match dev {
        Some(dir) if !out.iter().any(|w| w == "--profile") => {
            eprintln!("kit ui: opening the KitKraft Dev profile {dir}");
            out.splice(0..0, ["--profile".to_string(), dir]);
        }
        Some(_) => {}
        None => eprintln!(
            "kit ui: no KitKraft Dev profile found (run `kit doctor`); opening the profile picker"
        ),
    }
    out
}

/// `kit install` / `kit uninstall` (front-end setup, no `CommandSpec`: see
/// `kitkraft_engine::install`). Prints the plan; changes things only with `--apply`.
fn run_install(sub: &clap::ArgMatches, action: install::Action) -> ExitCode {
    let name = match action {
        install::Action::Install => "install",
        install::Action::Uninstall => "uninstall",
    };
    let dist = sub
        .get_one::<String>("dist")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(std::path::Path::to_path_buf))
        });
    if sub.get_flag("check-running") {
        // For Install.cmd: the same one-line message `--apply` stops with, before it asks.
        return match install::system_processes().map(|p| install::claude_running(&p)) {
            Ok(Some(message)) => {
                println!("{message}");
                ExitCode::from(EXIT_NEGATIVE)
            }
            Ok(None) => {
                println!("Claude Code is not running.");
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("kit {name}: {err}");
                ExitCode::from(EXIT_ERROR)
            }
        };
    }
    let Some(dist) = dist else {
        eprintln!("kit {name}: cannot tell where kit.exe is; pass --dist <folder>");
        return ExitCode::from(EXIT_ERROR);
    };
    let mut opts = install::Options::detect(dist);
    if let Some(home) = sub.get_one::<String>("home") {
        opts.home = home.into();
    }
    if let Some(project) = sub.get_one::<String>("project") {
        opts.project = Some(project.into());
    }
    if sub.get_flag("no-project") {
        opts.project = None;
    }
    if let Some(profile) = sub.try_get_one::<String>("profile").ok().flatten() {
        if let Err(err) = opts.set_profile(profile) {
            eprintln!("kit {name}: {err}");
            return ExitCode::from(EXIT_ERROR);
        }
    }
    opts.path = !sub.get_flag("no-path");
    opts.mcp = !sub.get_flag("no-mcp");
    let mut env: Box<dyn install::UserEnv> = match sub.get_one::<String>("env-file") {
        Some(file) => Box::new(install::FileEnv { path: file.into() }),
        None => Box::new(install::SystemEnv),
    };
    let plan = match install::plan(&opts, env.as_ref(), action) {
        Ok(plan) => plan,
        Err(err) => {
            eprintln!("kit {name}: {err}");
            return ExitCode::from(EXIT_ERROR);
        }
    };
    if !sub.get_flag("apply") {
        print!("{}", plan.render(false));
        if plan.changes_anything() {
            println!(
                "Dry run: nothing was changed. Run `kit {name} --apply` to make these changes."
            );
        }
        return ExitCode::SUCCESS;
    }
    // Claude Code keeps .claude.json in memory and writes it back: stop before anything
    // changes while it runs (nothing is stopped or killed).
    if !sub.get_flag("force-while-running") && install::touches_claude_json(&plan, &opts) {
        let check = install::system_processes()
            .and_then(|p| install::refuse_while_running(&plan, &opts, &p));
        if let Err(err) = check {
            eprintln!("kit {name}: {err}");
            eprintln!("kit {name}: nothing was changed");
            return ExitCode::from(EXIT_ERROR);
        }
    }
    print!("{}", plan.render(true));
    match install::apply(&plan, env.as_mut()) {
        Ok(done) => {
            for line in done {
                println!("done: {line}");
            }
            if plan.changes_anything() {
                println!("Open a new terminal, and restart Claude Code, to pick the changes up.");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("kit {name}: {err}");
            eprintln!("kit {name}: the steps printed as done above stay applied; nothing after the failed one was changed");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// The arguments of `kit install` and `kit uninstall`.
fn install_command(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .action(ArgAction::SetTrue)
                .conflicts_with("apply")
                .help("Print what would change and change nothing (the default)"),
        )
        .arg(
            Arg::new("apply")
                .long("apply")
                .action(ArgAction::SetTrue)
                .help("Make the changes (backups first)"),
        )
        .arg(
            Arg::new("force-while-running")
                .long("force-while-running")
                .action(ArgAction::SetTrue)
                .help("Apply even though Claude Code is running (it may write .claude.json back and drop the entry)"),
        )
        .arg(
            Arg::new("check-running")
                .long("check-running")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["apply", "dry-run"])
                .help("Only say whether Claude Code is running (exit 1 when it is); used by Install.cmd"),
        )
        .arg(
            Arg::new("home")
                .long("home")
                .value_name("DIR")
                .help("Folder holding .claude.json (default %USERPROFILE%)"),
        )
        .arg(
            Arg::new("project")
                .long("project")
                .value_name("DIR")
                .help("Project folder for .mcp.json (default: the repository above the dist folder)"),
        )
        .arg(
            Arg::new("no-project")
                .long("no-project")
                .action(ArgAction::SetTrue)
                .help("Leave the project .mcp.json alone"),
        )
        .arg(
            Arg::new("dist")
                .long("dist")
                .value_name("DIR")
                .help("The dist folder holding kit.exe (default: this program's folder)"),
        )
        .arg(
            Arg::new("no-path")
                .long("no-path")
                .action(ArgAction::SetTrue)
                .help("Leave the user PATH alone"),
        )
        .arg(
            Arg::new("no-mcp")
                .long("no-mcp")
                .action(ArgAction::SetTrue)
                .help("Leave the MCP registrations alone"),
        )
        .arg(
            Arg::new("env-file")
                .long("env-file")
                .value_name("FILE")
                .help("A JSON file standing in for HKCU\\Environment (tests; the real registry is not read)"),
        )
}

/// `kit ui`: starts the workbench binary (`kit-ui`, next to `kit`) with the given
/// arguments (it parses them). A launcher: it acts on no data, so it has no `CommandSpec`.
fn run_ui(args: &[String]) -> ExitCode {
    let name = format!("kit-ui{}", std::env::consts::EXE_SUFFIX);
    let path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&name)));
    let Some(path) = path.filter(|p| p.is_file()) else {
        eprintln!(
            "kit ui: {name} is not next to kit; build it with `cargo build -p kit` (or run `cargo run -p kit -- --profile DIR`)"
        );
        return ExitCode::from(EXIT_ERROR);
    };
    match std::process::Command::new(&path).args(args).status() {
        Ok(status) => {
            let code = status.code().and_then(|c| u8::try_from(c).ok());
            code.map_or(ExitCode::from(EXIT_ERROR), ExitCode::from)
        }
        Err(err) => {
            eprintln!("kit ui: could not start {}: {err}", path.display());
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
            "ui",
            "Open the read-only workbench window (kit ui [--dev | --profile DIR] [--plugins F...] [--data-dir DIR] [--theme PRESET])"
                .to_string(),
        ))
        .subcommand(
            install_command(
                "install",
                "Put this folder on the user PATH and register it as the kitkraft MCP server (--dry-run by default; --apply)",
            )
            .arg(
                Arg::new("profile")
                    .long("profile")
                    .value_name("NAME-OR-DIR")
                    .help("MO2 profile the MCP server preloads (default: the KitKraft Dev profile, else MO2's active one)"),
            ),
        )
        .subcommand(install_command(
            "uninstall",
            "Undo kit install: PATH entry, MCP registrations, install record (--dry-run by default; --apply)",
        ))
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
    let invocation = match kitkraft_engine::parse_words(words) {
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
    match kitkraft_engine::execute(spec.id, args) {
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
    if let (EngineError::NotFound { searched, .. }, Some(obj)) = (err, body.as_object_mut()) {
        obj.insert("searched".to_string(), json!(searched));
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
    println!(
        "  {:<18} Open the read-only workbench window (kit ui [--dev | --profile DIR] [--theme PRESET])",
        "ui"
    );
    println!(
        "  {:<18} Put this folder on PATH, register the MCP server (kit install [--apply])",
        "install"
    );
    println!(
        "  {:<18} Undo kit install (kit uninstall [--apply])",
        "uninstall"
    );
    println!();
    println!("DEFAULTS: `kit doctor` shows what is detected on this machine (game, Data dir, MO2,");
    println!("profiles). Precedence everywhere: argument > environment variable > detected.");
    println!(
        "A bare --profile name (\"KitKraft Dev\") is that profile of the detected MO2 instance."
    );
    println!();
    println!("ENVIRONMENT (optional; overrides detection):");
    println!(
        "  {:<18} Game Data dir; the default for --data-dir: plugin names (--plugins,",
        "KITKRAFT_DATA_DIR"
    );
    println!(
        "  {:<18} validate --plugin) resolve there, and masters are searched in the load",
        ""
    );
    println!(
        "  {:<18} order, next to the plugin, then there. A profile uses MO2's gamePath first.",
        ""
    );
    println!(
        "  {:<18} MO2 instance folder; the default before the detected one",
        "KITKRAFT_MO2_DIR"
    );
    println!(
        "  {:<18} 0 turns the detected defaults off (arguments and variables still apply)",
        "KITKRAFT_DETECT"
    );
    println!(
        "  {:<18} Index cache (default %LOCALAPPDATA%\\kitkraft\\cache)",
        "KITKRAFT_CACHE_DIR"
    );
    println!(
        "  {:<18} The pre-rename KITCRAFT_* names are still read when unset (deprecated).",
        ""
    );
    println!();
    println!("Every command is also an MCP tool call and a future menu item.");
    println!("Exit codes: 0 ok, 1 check failed (e.g. roundtrip --verify mismatch), 2 error.");
}

#[cfg(test)]
mod tests {
    use super::{result_code, EXIT_ERROR, EXIT_NEGATIVE};
    use kitkraft_engine::find_command;
    use serde_json::json;

    #[test]
    fn exit_code_follows_status_and_passed() {
        let roundtrip = find_command("roundtrip").unwrap();
        // No catalog command is Planned any more; a spec built here stands in for one.
        let planned = &kitkraft_engine::CommandSpec {
            status: kitkraft_engine::Status::Planned,
            ..*find_command("build").unwrap()
        };
        let with = |passed| json!({"data": {"passed": passed}});
        assert_eq!(result_code(roundtrip, &with(json!(true))), 0);
        assert_eq!(result_code(roundtrip, &with(json!(null))), 0);
        assert_eq!(result_code(roundtrip, &json!({"data": null})), 0);
        assert_eq!(result_code(roundtrip, &with(json!(false))), EXIT_NEGATIVE);
        assert_eq!(result_code(planned, &with(json!(true))), EXIT_ERROR);
    }

    #[test]
    fn ui_words_pass_through_without_dev() {
        let words = vec!["--plugins".to_string(), "A.esm".to_string()];
        assert_eq!(super::ui_words(words.clone()), words);
        // `--dev` never reaches kit-ui, whatever doctor finds; an explicit --profile wins.
        let out = super::ui_words(vec![
            "--dev".to_string(),
            "--profile".to_string(),
            "P".to_string(),
        ]);
        assert_eq!(out, ["--profile", "P"]);
    }
}
