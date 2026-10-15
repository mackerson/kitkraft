//! Runs the built `kit install` / `kit uninstall` against a temp home, a temp project and
//! a JSON stand-in for `HKCU\Environment` (`--env-file`): the real registry, PATH and
//! `~/.claude.json` are never read or written here.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

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
        std::fs::copy(env!("CARGO_BIN_EXE_kit"), dist.join("kit.exe")).unwrap();
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

    /// Runs the copied `kit.exe` (so its own folder is the dist folder).
    fn kit(&self, args: &[&str]) -> Output {
        let home = self.root.join("home");
        let env = self.root.join("env.json");
        let mut all: Vec<&str> = args.to_vec();
        all.extend([
            "--home",
            home.to_str().unwrap(),
            "--env-file",
            env.to_str().unwrap(),
        ]);
        Command::new(self.dist().join("kit.exe"))
            .args(&all)
            .output()
            .expect("run kit")
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
    assert!(command.ends_with("/dist/KitKraft/kit.exe"), "{command}");
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

#[test]
fn dry_run_and_apply_conflict() {
    let s = Sandbox::new("conflict");
    let o = s.kit(&["install", "--dry-run", "--apply"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    let _ = std::fs::remove_dir_all(&s.root);
}
