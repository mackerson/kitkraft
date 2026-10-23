//! Runs the built `kit install` / `kit uninstall` against a temp home, a temp project and
//! a JSON stand-in for `HKCU\Environment` (`--env-file`): the real registry, PATH and
//! `~/.claude.json` are never read or written here.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

/// The installed binary's name: `kit.exe` on Windows, `kit` elsewhere
/// (`crates/engine/src/install.rs` spells it the same way).
fn kit_name() -> String {
    format!("kit{}", std::env::consts::EXE_SUFFIX)
}

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Sandbox {
        let root =
            std::env::temp_dir().join(format!("kitkraft-cli-install-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dist = root.join("my repo").join("dist").join("KitKraft");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::copy(env!("CARGO_BIN_EXE_kit"), dist.join(kit_name())).unwrap();
        std::fs::write(root.join("my repo").join(".mcp.json.example"), b"{}").unwrap();
        std::fs::write(root.join("my repo").join("Cargo.toml"), b"").unwrap();
        std::fs::create_dir_all(root.join("home")).unwrap();
        std::fs::write(
            root.join("env.json"),
            serde_json::to_vec(
                &json!({"Path": {"kind": "REG_EXPAND_SZ", "value": "%SystemRoot%;C:\\Tools"}}),
            )
            .unwrap(),
        )
        .unwrap();
        Sandbox { root }
    }

    fn dist(&self) -> PathBuf {
        self.root.join("my repo").join("dist").join("KitKraft")
    }

    /// Runs the copied binary (so its own folder is the dist folder). The temp home is
    /// not the one a running Claude Code rewrites, so the running check is skipped
    /// (`--force-while-running`); [`Sandbox::kit_unforced`] keeps it.
    fn kit(&self, args: &[&str]) -> Output {
        let mut all: Vec<&str> = args.to_vec();
        all.push("--force-while-running");
        self.kit_unforced(&all, &[])
    }

    /// [`Sandbox::kit`] without `--force-while-running`, with extra variables.
    fn kit_unforced(&self, args: &[&str], vars: &[(&str, &str)]) -> Output {
        let home = self.root.join("home");
        let env = self.root.join("env.json");
        let mut all: Vec<&str> = args.to_vec();
        all.extend([
            "--home",
            home.to_str().unwrap(),
            "--env-file",
            env.to_str().unwrap(),
        ]);
        let mut cmd = Command::new(self.dist().join(kit_name()));
        cmd.args(&all).env_remove("KITKRAFT_MO2_DIR");
        for (k, v) in vars {
            cmd.env(k, v);
        }
        cmd.output().expect("run kit")
    }

    fn json(&self, path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn dry_run_then_apply_then_uninstall() {
    let s = Sandbox::new("cycle");
    let env_before = std::fs::read(s.root.join("env.json")).unwrap();

    // The default is a dry run: it prints the plan and changes nothing.
    let o = s.kit(&["install"]);
    assert!(o.status.success(), "{}", text(&o));
    let out = text(&o);
    assert!(out.contains("kit install would change"), "{out}");
    assert!(
        out.contains("%SystemRoot%;C:\\Tools"),
        "old PATH printed: {out}"
    );
    assert!(out.contains("Dry run: nothing was changed"), "{out}");
    assert_eq!(std::fs::read(s.root.join("env.json")).unwrap(), env_before);
    assert!(!s.root.join("home").join(".claude.json").exists());
    assert!(!s.root.join("my repo").join(".mcp.json").exists());

    let o = s.kit(&["install", "--apply"]);
    assert!(o.status.success(), "{}", text(&o));
    let env = s.json(&s.root.join("env.json"));
    assert_eq!(
        env["Path"]["value"],
        json!(format!("%SystemRoot%;C:\\Tools;{}", s.dist().display()))
    );
    assert_eq!(env["Path"]["kind"], "REG_EXPAND_SZ");
    let claude = s.json(&s.root.join("home").join(".claude.json"));
    let command = claude["mcpServers"]["kitkraft"]["command"]
        .as_str()
        .unwrap();
    assert!(
        command.ends_with(&format!("/dist/KitKraft/{}", kit_name())),
        "{command}"
    );
    let project = s.json(&s.root.join("my repo").join(".mcp.json"));
    assert_eq!(
        project["mcpServers"]["kitkraft"],
        claude["mcpServers"]["kitkraft"]
    );
    assert!(s.dist().join("install-state.json").is_file());

    // Idempotent.
    let o = s.kit(&["install", "--apply"]);
    assert!(text(&o).contains("Nothing to change"), "{}", text(&o));

    let o = s.kit(&["uninstall", "--apply"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        s.json(&s.root.join("env.json"))["Path"]["value"],
        "%SystemRoot%;C:\\Tools"
    );
    // Install created both files, so uninstall removes them.
    assert!(!s.root.join("home").join(".claude.json").exists());
    assert!(!s.root.join("my repo").join(".mcp.json").exists());
    assert!(!s.dist().join("install-state.json").exists());
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Black box on this machine: when `tasklist` shows Claude Code (as it does when the gate
/// runs under it), `--apply` stops before anything changes and says so in one line.
#[test]
fn apply_stops_while_claude_code_runs() {
    let s = Sandbox::new("running");
    let env_before = std::fs::read(s.root.join("env.json")).unwrap();
    let check = s.kit_unforced(&["install", "--check-running"], &[]);
    let said = text(&check);
    match check.status.code() {
        Some(0) => {
            assert!(said.contains("not running"), "{said}");
            eprintln!("Claude Code is not running here; the refusal is covered by the unit test");
        }
        Some(1) => {
            assert!(said.contains("Claude Code is running"), "{said}");
            let o = s.kit_unforced(&["install", "--apply"], &[]);
            assert_eq!(o.status.code(), Some(2), "{}", text(&o));
            assert!(text(&o).contains(said.trim()), "{}", text(&o));
            assert!(text(&o).contains("nothing was changed"), "{}", text(&o));
            assert!(!s.root.join("home").join(".claude.json").exists());
            assert_eq!(std::fs::read(s.root.join("env.json")).unwrap(), env_before);
        }
        other => panic!("--check-running exited {other:?}: {said}"),
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

/// `KITKRAFT_DETECT=0` turns detection off for the default MCP args too: no `--profile`
/// of a detected instance (the dev box has one at C:\Modding\MO2-Starfield).
#[test]
fn detection_off_gives_plain_mcp_args() {
    let s = Sandbox::new("nodetect");
    let o = s.kit_unforced(&["install"], &[("KITKRAFT_DETECT", "0")]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains(r#"+ args: ["mcp"]"#), "{}", text(&o));
    assert!(
        text(&o).contains("limitation: MCP --profile: no MO2 instance found"),
        "{}",
        text(&o)
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A temp MO2 instance (`KITKRAFT_MO2_DIR`) with `Default` active and a `Kitkraft Dev`
/// profile, and a temp home whose entry preloads `Default` (what the first Install.cmd
/// wrote on the dev box): the dry run re-points it to the dev profile as a diff;
/// `--profile Default` keeps it; an unknown `--profile` is refused.
#[test]
fn the_dry_run_repoints_a_default_profile_to_kitkraft_dev() {
    let s = Sandbox::new("devprofile");
    let mo2 = s.root.join("MO2");
    for p in ["Default", "Kitkraft Dev"] {
        std::fs::create_dir_all(mo2.join("profiles").join(p)).unwrap();
        std::fs::write(mo2.join("profiles").join(p).join("modlist.txt"), b"").unwrap();
    }
    std::fs::write(
        mo2.join("ModOrganizer.ini"),
        "[General]\r\ngameName=Starfield\r\nselected_profile=@ByteArray(Default)\r\n",
    )
    .unwrap();
    let slash = |p: PathBuf| p.display().to_string().replace('\\', "/");
    let live = slash(mo2.join("profiles").join("Default"));
    let dev = slash(mo2.join("profiles").join("Kitkraft Dev"));
    let claude = s.root.join("home").join(".claude.json");
    let before = serde_json::to_vec(&json!({"mcpServers": {"kitkraft": {
        "type": "stdio",
        "command": slash(s.dist().join(kit_name())),
        "args": ["mcp", "--profile", live],
        "env": {}
    }}}))
    .unwrap();
    std::fs::write(&claude, &before).unwrap();
    let vars = [("KITKRAFT_MO2_DIR", mo2.to_str().unwrap())];

    let o = s.kit_unforced(&["install", "--no-project"], &vars);
    let out = text(&o);
    assert!(o.status.success(), "{out}");
    assert!(out.contains("change mcpServers.kitkraft"), "{out}");
    assert!(
        out.contains(&format!(r#"- args: ["mcp","--profile","{live}"]"#)),
        "{out}"
    );
    assert!(
        out.contains(&format!(r#"+ args: ["mcp","--profile","{dev}"]"#)),
        "{out}"
    );
    assert!(out.contains("note: MCP --profile"), "{out}");
    assert_eq!(
        std::fs::read(&claude).unwrap(),
        before,
        "a dry run writes nothing"
    );

    let o = s.kit_unforced(&["install", "--no-project", "--profile", "default"], &vars);
    let out = text(&o);
    assert!(o.status.success(), "{out}");
    assert!(!out.contains("change mcpServers.kitkraft"), "{out}");
    assert!(out.contains("the --profile argument `default`"), "{out}");

    let o = s.kit_unforced(&["install", "--profile", "Nope"], &vars);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    assert!(text(&o).contains("not an MO2 profile"), "{}", text(&o));
    let _ = std::fs::remove_dir_all(&s.root);
}

#[test]
fn dry_run_and_apply_conflict() {
    let s = Sandbox::new("conflict");
    let o = s.kit(&["install", "--dry-run", "--apply"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    let _ = std::fs::remove_dir_all(&s.root);
}
