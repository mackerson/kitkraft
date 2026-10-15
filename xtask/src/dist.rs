//! `cargo xtask dist [--out DIR]`: release-build `kit.exe` and `kit-ui.exe` and write
//! `<out>\KitKraft\` (default `<root>\dist`, gitignored): the two programs, double-click
//! `.cmd` files for the common tasks, and `README.txt` (docs/QUICKSTART.md).
//!
//! The `.cmd` files hold no logic of their own beyond prompting: detection is
//! `kit doctor`, the PATH and MCP edits are `kit install`. Every one quotes its paths
//! (the folder may have spaces or parentheses), keeps `%~dp0` before any `shift`, and
//! pauses before exiting on an error so a double-clicked window stays readable. Lines
//! end in CRLF.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// The folder written under `--out`.
pub(crate) const FOLDER: &str = "KitKraft";
/// The programs copied from the release build.
pub(crate) const PROGRAMS: &[&str] = &["kit.exe", "kit-ui.exe"];

/// `kit.exe` missing next to the script: the common tail of every script.
const NO_KIT: &str = r#":nokit
echo kit.exe is not next to this file. Build the folder again with "cargo xtask dist".
pause
exit /b 2
"#;

const HEADER: &str = r#"@echo off
setlocal EnableExtensions DisableDelayedExpansion
set "KIT=%~dp0kit.exe"
if not exist "%KIT%" goto nokit
"#;

const WORKBENCH: &str = r#"title KitKraft Workbench
echo Starting the KitKraft workbench (read-only). Keep this window open: closing it
echo closes the workbench. It opens the KitKraft Dev profile when "kit doctor" finds
echo one, and the profile picker otherwise.
"%KIT%" ui --dev
if errorlevel 1 goto failed
exit /b 0

:failed
echo.
echo The workbench stopped with an error (see above). "KitKraft Shell.cmd" runs "kit doctor".
pause
exit /b 1

"#;

const SHELL: &str = r#"set "KK=%~dp0"
set "PATH=%KK%;%PATH%"
rem Start in the repository when the dist folder is inside one, so out\ paths work.
if exist "%KK%..\..\Cargo.toml" cd /d "%KK%..\.."
title KitKraft Shell
where powershell.exe >nul 2>nul
if errorlevel 1 goto cmdshell
powershell.exe -NoLogo -NoExit -ExecutionPolicy Bypass -Command "kit doctor; Write-Host ''; Write-Host 'KitKraft Shell: kit is on PATH in this window. Try: kit help'"
exit /b 0

:cmdshell
kit doctor
echo.
echo KitKraft Shell: kit is on PATH in this window. Try: kit help
cmd /k
exit /b 0

"#;

const VALIDATE: &str = r#"if "%~1"=="" goto usage
set "RESULT=0"

:next
if "%~1"=="" goto done
echo.
echo === Validating "%~nx1"
"%KIT%" validate --plugin "%~f1"
set "RC=%ERRORLEVEL%"
if "%RC%"=="1" if "%RESULT%"=="0" set "RESULT=1"
if not "%RC%"=="0" if not "%RC%"=="1" set "RESULT=2"
shift
goto next

:done
echo.
if "%RESULT%"=="0" echo Result: PASSED - no problems found.
if "%RESULT%"=="1" echo Result: PROBLEMS FOUND - see the findings above.
if "%RESULT%"=="2" echo Result: a file could not be checked - see the error above.
pause
exit /b %RESULT%

:usage
echo Drag one or more plugins (.esm, .esp or .esl) onto "Validate Plugin.cmd" to check them.
echo From a terminal: "Validate Plugin.cmd" path\to\Plugin.esm
pause
exit /b 2

"#;

const LOAD_DEV: &str = r#"title KitKraft: load the KitKraft Dev profile
echo Loading the KitKraft Dev profile (read-only) and confirming runtime IDs from its
echo newest save. The command is:
echo.
echo   kit load --profile "KitKraft Dev" --from-save newest
echo.
"%KIT%" load --profile "KitKraft Dev" --from-save newest
if errorlevel 1 goto failed
echo.
pause
exit /b 0

:failed
echo.
echo The load failed (see above). "kit doctor" shows whether the KitKraft Dev profile was found.
pause
exit /b 1

"#;

const DEPLOY: &str = r#"title KitKraft: deploy to the dev profile
if "%~1"=="" goto usage
set "SRC=%~f1"
set "MO2=%~2"
if not "%MO2%"=="" goto havemo2
rem Ask kit doctor for the MO2 folder. Run from this folder, as .\kit.exe, so no quoted
rem path is needed (a quoted path with parentheses breaks for /f) and the current folder
rem is used even where NoDefaultCurrentDirectoryInExePath is set.
pushd "%~dp0"
for /f "delims=" %%M in ('.\kit.exe doctor --get mo2') do set "MO2=%%M"
popd
:havemo2
if "%MO2%"=="" goto nomo2
echo Source: "%SRC%"
echo MO2:    "%MO2%"
echo.
echo Step 1 of 2: dry run (nothing is written)
"%KIT%" deploy "%SRC%" "%MO2%" --dev --dry-run --profile "KitKraft Dev"
if errorlevel 1 goto failed
echo.
set "ANSWER="
set /p "ANSWER=Deploy for real into its KitKraft Dev mod folder? Type y and press Enter (anything else cancels): "
if /i not "%ANSWER%"=="y" goto cancelled
echo.
echo Step 2 of 2: deploying
"%KIT%" deploy "%SRC%" "%MO2%" --dev --profile "KitKraft Dev"
if errorlevel 1 goto failed
echo.
echo Deployed. The first time, enable the "KitKraft Dev - <plugin>" mod and its plugin in
echo MO2's KitKraft Dev profile (never Default), then refresh MO2.
pause
exit /b 0

:cancelled
echo Cancelled; nothing was deployed.
pause
exit /b 0

:failed
echo.
echo The deploy stopped (see above). Nothing after the failed step was written.
pause
exit /b 1

:nomo2
echo No MO2 instance found. "kit doctor" shows where it looked.
pause
exit /b 2

:usage
echo Drag a plugin (.esm, .esp, .esl) or a packaged mod folder onto "Deploy to Dev.cmd".
echo It shows a dry run first and asks before writing anything.
pause
exit /b 2

"#;

const FINISH_MOVE: &str = r#"@echo off
setlocal EnableExtensions DisableDelayedExpansion
set "SCRIPT=C:\Modding\scripts\finish_kitkraft_move.ps1"
set "NEWKIT=C:\Modding\kitkraft\dist\KitKraft\kit.exe"
if /i "%~1"=="--from-temp" goto run
title KitKraft: finish the folder move
echo This renames C:\Modding\kitcraft to C:\Modding\kitkraft, re-points the MCP
echo registrations (.claude.json in your user folder, the project .mcp.json) and runs
echo "git worktree prune", using C:\Modding\scripts\finish_kitkraft_move.ps1.
echo.
echo BEFORE YOU GO ON:
echo   1. Close EVERY Claude Code session that has C:\Modding\kitcraft open. The script
echo      stops every program running from that folder, kit.exe MCP servers included.
echo   2. No lane worktree under C:\Modding\kitcraft\.claude\worktrees may be in use.
echo   3. Close Explorer windows and terminals that are inside C:\Modding\kitcraft.
echo.
if not exist "C:\Modding\kitcraft\" goto already
if not exist "%SCRIPT%" goto noscript
set "ANSWER="
set /p "ANSWER=Type YES (capitals) when every session is closed, anything else cancels: "
if not "%ANSWER%"=="YES" goto cancelled
rem This file lives in the folder being renamed: run the rest from a copy in TEMP.
copy /y "%~f0" "%TEMP%\kitkraft-finish-move.cmd" >nul
if errorlevel 1 goto failed
cd /d "%TEMP%"
"%TEMP%\kitkraft-finish-move.cmd" --from-temp

:run
cd /d "%TEMP%"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%"
if errorlevel 1 goto failed
if not exist "C:\Modding\kitkraft\dist\KitKraft\install-state.json" goto done
echo.
echo KitKraft was installed from the old folder: pointing the PATH at the new one.
"%NEWKIT%" install --apply
if errorlevel 1 goto failed

:done
echo.
echo Done. Open new Claude Code sessions on C:\Modding\kitkraft.
pause
exit /b 0

:already
echo C:\Modding\kitcraft does not exist: the move is done already (or was never needed).
pause
exit /b 0

:noscript
echo %SCRIPT% is missing.
pause
exit /b 2

:cancelled
echo Cancelled; nothing was changed.
pause
exit /b 0

:failed
echo.
echo The move stopped with an error (see above). Run "kit doctor" to see the current state.
pause
exit /b 1
"#;

/// `Install.cmd` / `Uninstall.cmd`: dry run, ask, apply.
fn installer(verb: &str, intro: &str, done: &str) -> String {
    format!(
        r#"title KitKraft: {verb}
{intro}
echo Close Claude Code first: it rewrites .claude.json while it runs.
echo.
echo What would change:
echo.
"%KIT%" {verb} --dry-run
if errorlevel 1 goto failed
echo.
set "ANSWER="
set /p "ANSWER=Make these changes? Type y and press Enter (anything else cancels): "
if /i not "%ANSWER%"=="y" goto cancelled
echo.
"%KIT%" {verb} --apply
if errorlevel 1 goto failed
echo.
{done}
pause
exit /b 0

:cancelled
echo Cancelled; nothing was changed.
pause
exit /b 0

:failed
echo.
echo kit {verb} stopped with an error (see above).
pause
exit /b 1

"#
    )
}

const README: &str = r#"KitKraft - open-source Creation Kit tooling for Starfield
=========================================================

This folder is everything you need day to day. Double-click a file:

  KitKraft Workbench.cmd   Opens the read-only workbench window on your KitKraft Dev
                           MO2 profile (or a profile picker if there is none).
  KitKraft Shell.cmd       Opens a terminal where "kit" works, and first runs
                           "kit doctor": what KitKraft found on this PC (the game,
                           its Data folder, MO2, profiles, tools) with OK or a fix.
  Validate Plugin.cmd      Drag a plugin (.esm/.esp/.esl) onto it: runs the QA
                           check and keeps the window open to show the result.
  Load Dev Profile.cmd     Loads the KitKraft Dev profile (read-only) and checks
                           its newest save, so the in-game IDs are known.
  Deploy to Dev.cmd        Drag a plugin or a packaged mod folder onto it: shows a
                           dry run, asks y/n, then copies it into its own
                           "KitKraft Dev - <plugin>" mod in MO2. It never touches
                           the Default profile (your real playthrough).
  Install.cmd              Puts this folder on your PATH (so "kit" works in any new
                           terminal) and registers kit.exe as the "kitkraft" MCP
                           server for Claude Code. Shows every change first and asks.
                           Backups are made first. Running it twice is harmless.
  Uninstall.cmd            Undoes Install.cmd.
  Finish Move.cmd          One time only: renames C:\Modding\kitcraft to
                           C:\Modding\kitkraft. Close every Claude Code session first.

No environment variables are needed: KitKraft finds the game through Steam, and MO2
at C:\Modding\MO2-Starfield. If something is not found, "kit doctor" says what to do.

Where things live
  This folder              kit.exe, kit-ui.exe and these files (rebuilt by
                           "cargo xtask dist"; install-state.json and backups\ are
                           written by Install.cmd).
  Index cache              %LOCALAPPDATA%\kitkraft\cache (safe to delete; it is
                           rebuilt on the next load).
  MCP registration         .claude.json in your user folder, and .mcp.json in the
                           repository.
  Built plugins            out\ in the repository.

More: docs\QUICKSTART.md in the repository.
"#;

/// The files written into the folder: name and text (LF; written as CRLF).
pub(crate) fn files() -> Vec<(&'static str, String)> {
    let with_kit = |body: &str| format!("{HEADER}{body}{NO_KIT}");
    vec![
        ("KitKraft Workbench.cmd", with_kit(WORKBENCH)),
        ("KitKraft Shell.cmd", with_kit(SHELL)),
        ("Validate Plugin.cmd", with_kit(VALIDATE)),
        ("Load Dev Profile.cmd", with_kit(LOAD_DEV)),
        ("Deploy to Dev.cmd", with_kit(DEPLOY)),
        ("Finish Move.cmd", FINISH_MOVE.to_string()),
        (
            "Install.cmd",
            with_kit(&installer(
                "install",
                "echo KitKraft install: puts this folder on your user PATH and registers kit.exe\necho as the \"kitkraft\" MCP server for Claude Code (user config and project .mcp.json).",
                "echo Installed. Open a new terminal to use \"kit\", and restart Claude Code to load the MCP server.",
            )),
        ),
        (
            "Uninstall.cmd",
            with_kit(&installer(
                "uninstall",
                "echo KitKraft uninstall: takes this folder off your user PATH and removes the\necho \"kitkraft\" MCP registrations that point here, restoring what install replaced.",
                "echo Uninstalled. This folder itself is left in place; delete it if you like.",
            )),
        ),
        ("README.txt", README.to_string()),
    ]
}

fn crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// Writes the folder at `dest` from the programs in `release` (the cargo release dir).
pub(crate) fn write_folder(dest: &Path, release: &Path) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    let mut written = Vec::new();
    for program in PROGRAMS {
        let from = release.join(program);
        let to = dest.join(program);
        fs::copy(&from, &to).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32) {
                anyhow::anyhow!(
                    "{} is in use (a running MCP server or workbench): close Claude Code sessions and the workbench that use it, then run `cargo xtask dist` again",
                    to.display()
                )
            } else {
                anyhow::anyhow!("copying {} to {}: {e}", from.display(), to.display())
            }
        })?;
        written.push(to);
    }
    for (name, text) in files() {
        let to = dest.join(name);
        fs::write(&to, crlf(&text)).with_context(|| format!("writing {}", to.display()))?;
        written.push(to);
    }
    Ok(written)
}

/// The target directory cargo builds into (honours `CARGO_TARGET_DIR`).
fn target_dir(root: &Path) -> Result<PathBuf> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(&cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .with_context(|| format!("could not start `{cargo} metadata`"))?;
    if !out.status.success() {
        bail!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).context("cargo metadata")?;
    v.get("target_directory")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .context("cargo metadata has no target_directory")
}

/// `--out DIR` from the task's arguments (default `<root>\dist`).
fn out_dir(root: &Path, args: &[String]) -> Result<PathBuf> {
    match args {
        [] => Ok(root.join("dist")),
        [flag, dir] if flag == "--out" => Ok(PathBuf::from(dir)),
        _ => bail!("dist: expected `cargo xtask dist [--out DIR]`"),
    }
}

/// The task.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let out = out_dir(root, args)?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    println!("xtask dist: cargo build --release -p kit-cli -p kit");
    let status = Command::new(&cargo)
        .args(["build", "--release", "-p", "kit-cli", "-p", "kit"])
        .current_dir(root)
        .status()
        .with_context(|| format!("could not start `{cargo}`"))?;
    if !status.success() {
        bail!("`cargo build --release` failed ({status})");
    }
    let release = target_dir(root)?.join("release");
    let dest = out.join(FOLDER);
    let written = write_folder(&dest, &release)?;
    println!(
        "xtask dist: wrote {} files to {}",
        written.len(),
        dest.display()
    );
    for w in &written {
        println!("  {}", w.display());
    }
    println!(
        "xtask dist: next, double-click \"{}\" (or run `kit install --dry-run` there)",
        dest.join("Install.cmd").display()
    );
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn scripts() -> Vec<(&'static str, String)> {
        files()
            .into_iter()
            .filter(|(n, _)| n.ends_with(".cmd"))
            .collect()
    }

    #[test]
    fn every_listed_script_is_there() {
        let names: Vec<&str> = files().iter().map(|(n, _)| *n).collect();
        for want in [
            "KitKraft Workbench.cmd",
            "KitKraft Shell.cmd",
            "Validate Plugin.cmd",
            "Load Dev Profile.cmd",
            "Deploy to Dev.cmd",
            "Finish Move.cmd",
            "Install.cmd",
            "Uninstall.cmd",
            "README.txt",
        ] {
            assert!(names.contains(&want), "{want}");
        }
    }

    /// Every `goto` has its label, every error exit pauses first, and every path that
    /// may hold spaces or parentheses is quoted.
    #[test]
    fn scripts_jump_to_labels_pause_on_errors_and_quote_paths() {
        for (name, text) in scripts() {
            assert!(text.starts_with("@echo off\n"), "{name}");
            let lines: Vec<&str> = text.lines().map(str::trim).collect();
            let labels: Vec<&str> = lines.iter().filter_map(|l| l.strip_prefix(':')).collect();
            for l in &lines {
                if let Some(target) = l.split("goto ").nth(1) {
                    let target = target.trim();
                    assert!(
                        labels.contains(&target),
                        "{name}: goto {target} has no label"
                    );
                }
            }
            for (i, l) in lines.iter().enumerate() {
                let Some(code) = l.strip_prefix("exit /b ") else {
                    continue;
                };
                if code != "0" && i > 0 {
                    assert_eq!(
                        lines[i - 1],
                        "pause",
                        "{name}: `{l}` without a pause before it"
                    );
                }
            }
            for l in &lines {
                if l.starts_with("echo") || l.starts_with("rem") || l.starts_with("set \"") {
                    continue;
                }
                for var in [
                    "%KIT%", "%~dp0", "%~f1", "%SRC%", "%MO2%", "%KK%", "%SCRIPT%", "%NEWKIT%",
                ] {
                    for (at, _) in l.match_indices(var) {
                        let before = l[..at].chars().last();
                        assert_eq!(before, Some('"'), "{name}: unquoted {var} in `{l}`");
                    }
                }
            }
            // `%~dp0` after a `shift` would name the wrong file.
            if let Some(shift) = lines.iter().position(|l| *l == "shift") {
                assert!(
                    lines[shift..].iter().all(|l| !l.contains("%~dp0")),
                    "{name}: %~dp0 after shift"
                );
            }
        }
    }

    #[test]
    fn the_scripts_call_the_commands_the_quickstart_names() {
        let get = |n: &str| files().into_iter().find(|(f, _)| *f == n).unwrap().1;
        assert!(get("Validate Plugin.cmd").contains("\"%KIT%\" validate --plugin \"%~f1\""));
        assert!(get("Load Dev Profile.cmd")
            .contains("\"%KIT%\" load --profile \"KitKraft Dev\" --from-save newest"));
        let deploy = get("Deploy to Dev.cmd");
        let dry = deploy.find("--dev --dry-run").unwrap();
        let ask = deploy.find("set /p").unwrap();
        let real = deploy.find("--dev --profile").unwrap();
        assert!(dry < ask && ask < real, "dry run, then ask, then deploy");
        // for /f cannot take a quoted path with parentheses, and the bare name is not
        // found where NoDefaultCurrentDirectoryInExePath is set (seen under Claude Code).
        assert!(deploy.contains("('.\\kit.exe doctor --get mo2')"));
        assert!(get("KitKraft Workbench.cmd").contains("\"%KIT%\" ui --dev"));
        assert!(get("Install.cmd").contains("\"%KIT%\" install --dry-run"));
        assert!(get("Uninstall.cmd").contains("\"%KIT%\" uninstall --apply"));
        assert!(get("Finish Move.cmd").contains("finish_kitkraft_move.ps1"));
        assert!(get("KitKraft Shell.cmd").contains("kit doctor"));
    }

    #[test]
    fn folder_is_written_with_crlf_and_the_programs() {
        let root = std::env::temp_dir().join(format!("kitkraft-xtask-dist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let release = root.join("release");
        fs::create_dir_all(&release).unwrap();
        for p in PROGRAMS {
            fs::write(release.join(p), b"MZ").unwrap();
        }
        let dest = root.join("out dir (x86)").join(FOLDER);
        let written = write_folder(&dest, &release).unwrap();
        assert_eq!(written.len(), PROGRAMS.len() + files().len());
        let cmd = fs::read_to_string(dest.join("Install.cmd")).unwrap();
        assert!(cmd.contains("\r\n") && !cmd.replace("\r\n", "").contains('\n'));
        assert_eq!(fs::read(dest.join("kit.exe")).unwrap(), b"MZ");
        // Again over an existing folder.
        write_folder(&dest, &release).unwrap();
        // Bad arguments.
        assert!(out_dir(&root, &["--out".into()]).is_err());
        assert_eq!(
            out_dir(&root, &["--out".into(), "D".into()]).unwrap(),
            PathBuf::from("D")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
