//! `kit-ui`: opens the read-only developer workbench (`kitkraft-ui-egui`). A front-end
//! launcher like `kit mcp` (AGENTS.md, "Everything is a command"): it takes the same
//! load-order arguments, starts the window with a `load` when given them, and acts on no
//! data itself. `kit ui [...]` starts this binary.
//!
//! LAYER: APP

use std::process::ExitCode;

use clap::{Arg, ArgAction, Command};

fn main() -> ExitCode {
    let matches = match cli().try_get_matches() {
        Ok(m) => m,
        Err(err) => {
            let _ = err.print();
            return if err.use_stderr() {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    let opts = kitkraft_ui_egui::LaunchOptions {
        profile: matches.get_one::<String>("profile").cloned(),
        plugins: matches
            .get_many::<String>("plugins")
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
        data_dir: matches.get_one::<String>("data-dir").cloned(),
    };
    match kitkraft_ui_egui::run(opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kit-ui: {err}");
            ExitCode::from(2)
        }
    }
}

fn cli() -> Command {
    Command::new("kit-ui")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Read-only KitKraft developer workbench over the command catalog")
        .arg(
            Arg::new("profile")
                .long("profile")
                .value_name("DIR")
                .help("MO2 profile to load (read-only) when the window opens"),
        )
        .arg(
            Arg::new("plugins")
                .long("plugins")
                .value_name("FILE")
                .action(ArgAction::Append)
                .help("Plugins to load in order (repeat the flag); used without --profile"),
        )
        .arg(
            Arg::new("data-dir")
                .long("data-dir")
                .value_name("DIR")
                .help("Game Data dir (default KITKRAFT_DATA_DIR)"),
        )
}

#[cfg(test)]
mod tests {
    use super::cli;

    #[test]
    fn arguments_match_kit_mcp_plus_plugins_and_data_dir() {
        let m = cli()
            .try_get_matches_from([
                "kit-ui",
                "--plugins",
                "A.esm",
                "--plugins",
                "B.esp",
                "--data-dir",
                "D",
            ])
            .unwrap();
        let plugins: Vec<&String> = m.get_many::<String>("plugins").unwrap().collect();
        assert_eq!(plugins, ["A.esm", "B.esp"]);
        assert_eq!(m.get_one::<String>("data-dir").unwrap(), "D");
        assert!(cli()
            .try_get_matches_from(["kit-ui", "--set", "x"])
            .is_err());
    }
}
