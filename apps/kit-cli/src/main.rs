//! `kit`: the command-line front end.
//!
//! One subcommand per engine command. No format or editing logic lives here: the CLI
//! parses words into a command id plus JSON arguments and hands them to the engine.
//! Pre-alpha: only `help` and `--version` work today.
//!
//! LAYER: APP

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str);
    match first {
        None | Some("help" | "-h" | "--help") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("kit {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("mcp") => {
            eprintln!("kit: the MCP server is not implemented yet (see docs/mcp.md)");
            ExitCode::from(2)
        }
        Some(word) => {
            let dotted = match args.get(1) {
                Some(second) if !second.starts_with('-') => format!("{word}.{second}"),
                _ => word.to_string(),
            };
            let spec = kitcraft_engine::find_command(&dotted)
                .or_else(|| kitcraft_engine::find_command(word));
            match spec {
                Some(spec) => eprintln!(
                    "kit: `{}` is in the catalog but not implemented yet (pre-alpha)",
                    spec.id
                ),
                None => eprintln!("kit: unknown command `{word}`; run `kit help`"),
            }
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!(
        "kit {} - open-source Creation Kit tooling for Starfield (pre-alpha)",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("USAGE: kit <command> [args] [--json]");
    println!();
    println!("COMMANDS:");
    for spec in kitcraft_engine::catalog() {
        let name = spec.id.replace('.', " ");
        let state = match spec.status {
            kitcraft_engine::Status::Planned => " (planned)",
            kitcraft_engine::Status::Implemented => "",
        };
        println!("  {name:<18} {}{state}", spec.summary);
    }
    println!(
        "  {:<18} Serve the same catalog over MCP on stdio (planned)",
        "mcp"
    );
    println!();
    println!("Every command is also an MCP tool call and a future menu item.");
}
