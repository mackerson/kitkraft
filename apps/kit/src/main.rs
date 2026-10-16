//! `kit-ui`: opens the read-only developer workbench (`kitkraft-ui-egui`). A front-end
//! launcher like `kit mcp` (AGENTS.md, "Everything is a command"): it takes the same
//! load-order arguments, starts the window with a `load` when given them, and acts on no
//! data itself. `kit ui [...]` starts this binary.
//!
//! LAYER: APP

use std::process::ExitCode;

use clap::{Arg, ArgAction, Command};
use kitkraft_ui_egui::theme::ThemeChoice;

fn main() -> ExitCode {
    kitkraft_ui_egui::perf::mark_start();
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
    if let Some(warning) = debug_warning() {
        eprintln!("{warning}");
    }
    let opts = kitkraft_ui_egui::LaunchOptions {
        profile: matches.get_one::<String>("profile").cloned(),
        plugins: matches
            .get_many::<String>("plugins")
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
        data_dir: matches.get_one::<String>("data-dir").cloned(),
        theme: matches.get_one::<ThemeChoice>("theme").copied(),
    };
    let result = kitkraft_ui_egui::run(opts);
    kitkraft_ui_egui::perf::flush();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kit-ui: {err}");
            ExitCode::from(2)
        }
    }
}

/// One line for a debug build, which is far slower and bigger than a release one
/// (docs/workbench.md, "Launch").
fn debug_warning() -> Option<&'static str> {
    cfg!(debug_assertions).then_some(
        "kit-ui: debug build: a profile load takes about a minute and 2 GB; build with --release (cargo build --release -p kit -p kit-cli)",
    )
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
        .arg(
            Arg::new("theme")
                .long("theme")
                .value_name("PRESET")
                .value_parser(|s: &str| ThemeChoice::parse(s))
                .help(format!(
                    "Visual preset: {} (default: `theme` in ui.json, else workbench)",
                    ThemeChoice::HELP
                )),
        )
}

#[cfg(test)]
mod tests {
    use super::{cli, debug_warning};

    #[test]
    fn a_debug_build_says_so_in_one_line() {
        let w = debug_warning();
        assert_eq!(w.is_some(), cfg!(debug_assertions));
        if let Some(w) = w {
            assert!(w.contains("--release") && !w.contains('\n'), "{w}");
        }
    }

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

    #[test]
    fn theme_takes_a_preset_and_refuses_unknown_names() {
        use kitkraft_ui_egui::theme::{Mode, Preset, ThemeChoice};
        let m = cli()
            .try_get_matches_from(["kit-ui", "--theme", "studio-dark"])
            .unwrap();
        let t = m.get_one::<ThemeChoice>("theme").unwrap();
        assert_eq!((t.preset, t.mode), (Preset::Studio, Mode::Dark));
        assert!(cli()
            .try_get_matches_from(["kit-ui"])
            .unwrap()
            .get_one::<ThemeChoice>("theme")
            .is_none());
        assert!(cli()
            .try_get_matches_from(["kit-ui", "--theme", "neon"])
            .is_err());
    }
}
