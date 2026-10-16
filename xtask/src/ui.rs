//! `cargo xtask ui [ARGS...]` (M6.P6a): release-build `kit` and `kit-ui`, then start the
//! window through `kit ui`, on the KitKraft Dev profile (`--dev`) unless the arguments name
//! a profile or plugins. A debug window loads a profile about 5x slower
//! (`docs/design/workbench-performance.md`), so this is the way to open it from a checkout.
//!
//! Also the build identity (M6.P6b): [`build_env`] is what this task and `cargo xtask dist`
//! set when they build, so `kit --version`, the envelope's `build` and the window name the
//! commit and date (`kitkraft_engine::identity`). No `build.rs` (AGENTS.md): a missing git
//! or clock leaves the value `unknown` instead.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

/// The compile-time variables of `kitkraft_engine::identity` (`COMMIT_ENV`, `DATE_ENV`).
pub(crate) const COMMIT_ENV: &str = "KITKRAFT_BUILD_COMMIT";
pub(crate) const DATE_ENV: &str = "KITKRAFT_BUILD_DATE";

/// `[(KITKRAFT_BUILD_COMMIT, <short hash>[-dirty]), (KITKRAFT_BUILD_DATE, <yyyy-mm-dd>)]`
/// for a build of `root`; `unknown` for what git or the clock cannot say.
pub(crate) fn build_env(root: &Path) -> Vec<(&'static str, String)> {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let commit = git(&["rev-parse", "--short", "HEAD"])
        .filter(|c| !c.is_empty())
        .map(|c| {
            // Tracked changes only: build outputs and scratch files do not make a tree dirty.
            match git(&["status", "--porcelain", "--untracked-files=no"]) {
                Some(s) if s.is_empty() => c,
                Some(_) => format!("{c}-dirty"),
                None => c,
            }
        });
    vec![
        (COMMIT_ENV, commit.unwrap_or_else(|| "unknown".to_string())),
        (
            DATE_ENV,
            crate::observe::today().unwrap_or_else(|| "unknown".to_string()),
        ),
    ]
}

/// The words handed to `kit ui`: `--dev` first unless the caller names a profile or plugins
/// (or asks for none with `--no-dev`, which is dropped).
pub(crate) fn kit_ui_args(args: &[String]) -> Vec<String> {
    let named = args
        .iter()
        .any(|a| matches!(a.as_str(), "--profile" | "--plugins" | "--dev" | "--no-dev"));
    let mut out = vec!["ui".to_string()];
    if !named {
        out.push("--dev".to_string());
    }
    out.extend(args.iter().filter(|a| *a != "--no-dev").cloned());
    out
}

/// The task: build release with the build identity, then run `kit ui` from the release
/// dir; fails when the window exits with an error.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let env = build_env(root);
    println!(
        "xtask ui: cargo build --release -p kit-cli -p kit ({})",
        env.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let status = Command::new(&cargo)
        .args(["build", "--release", "-p", "kit-cli", "-p", "kit"])
        .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
        .current_dir(root)
        .status()
        .with_context(|| format!("could not start `{cargo}`"))?;
    if !status.success() {
        bail!("`cargo build --release` failed ({status})");
    }
    let kit = crate::dist::target_dir(root)?
        .join("release")
        .join(format!("kit{}", std::env::consts::EXE_SUFFIX));
    let words = kit_ui_args(args);
    println!("xtask ui: {} {}", kit.display(), words.join(" "));
    let status = Command::new(&kit)
        .args(&words)
        .status()
        .with_context(|| format!("could not start {}", kit.display()))?;
    if !status.success() {
        bail!("`kit ui` exited with {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_dev_profile_unless_one_is_named() {
        assert_eq!(kit_ui_args(&[]), ["ui", "--dev"]);
        assert_eq!(
            kit_ui_args(&words(&["--theme", "studio"])),
            ["ui", "--dev", "--theme", "studio"]
        );
        assert_eq!(
            kit_ui_args(&words(&["--profile", "P"])),
            ["ui", "--profile", "P"]
        );
        assert_eq!(
            kit_ui_args(&words(&["--plugins", "A.esm"])),
            ["ui", "--plugins", "A.esm"]
        );
        assert_eq!(kit_ui_args(&words(&["--no-dev"])), ["ui"]);
    }

    #[test]
    fn build_env_names_commit_and_date_or_unknown() {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let env = build_env(here);
        let get = |k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone());
        let commit = get(COMMIT_ENV).unwrap();
        let date = get(DATE_ENV).unwrap();
        assert!(!commit.is_empty());
        assert!(
            date == "unknown" || (date.len() == 10 && date.as_bytes()[4] == b'-'),
            "{date}"
        );
        // Outside any repository git cannot say: unknown, never an error.
        let nowhere = std::env::temp_dir().join(format!("xtask-ui-nogit-{}", std::process::id()));
        std::fs::create_dir_all(&nowhere).unwrap();
        let outside = build_env(&nowhere);
        let _ = std::fs::remove_dir_all(&nowhere);
        // The temp dir may sit inside a repository on some machines; only check the shape.
        assert_eq!(outside.len(), 2);
    }
}
