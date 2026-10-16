//! M4.T10 transcript check: what a fresh agent session does with
//! `docs/playbooks/mod-from-brief.md` and a brief, up to the session that preflight
//! refuses today. The rename brief (`docs/smoke-rename.toml`) and the first POI spec
//! (`docs/poi/first-poi.toml`) run through the playbook's own `kit mod run` command line
//! (parsed by the CLI grammar, executed through `execute`, as `kit` does) against a
//! fixture MO2 instance (`kitkraft_game::testing`, as crates/game's tests use). Reads the
//! official `Starfield.esm` in place through `KITKRAFT_DATA_DIR` (skipped when unset);
//! writes only under a temp folder (and lane D's index cache).
//!
//! The playbook's "done when" (a session that reports `pass` with no owner step) needs
//! M4.T1's launch, console and save evidence and M4.O7's console tools; until then this
//! check proves every stage before the session and that the session is refused for
//! exactly that reason, with every probe `not-run` and the run `partial` (exit 0).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::{Path, PathBuf};

use kitkraft_engine::{execute, parse_line, Invocation};
use kitkraft_game::testing::{mo2_fixture, FIXTURE_SAVE};
use serde_json::Value;

fn data_dir() -> Option<PathBuf> {
    match std::env::var_os("KITKRAFT_DATA_DIR")
        .or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
        .map(PathBuf::from)
    {
        Some(dir) if dir.join("Starfield.esm").is_file() => Some(dir),
        _ => {
            println!(
                "skipped: KITKRAFT_DATA_DIR is unset or has no Starfield.esm (docs/corpus.md)"
            );
            None
        }
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// SHA-256 of `kitkraft_game::testing::FIXTURE_SAVE`, as the owner records the base save's
/// hash at M4.O7 (`Get-FileHash`).
const FIXTURE_SAVE_SHA256: &str =
    "76ae08b02848a4c1b8b9baa1887bf09875a9cf37f7a8048b6a9537a2c1741af7";

#[test]
fn the_fixture_save_is_the_hashed_bytes() {
    assert_eq!(FIXTURE_SAVE, b"fixture base save");
}

/// One brief through the playbook's command line; returns `data` of the run.
fn run_brief(spec: &Path, name: &str, data: &Path) -> Value {
    let root = std::env::temp_dir().join(format!(
        "kitkraft-mod-from-brief-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let fx = mo2_fixture(&root.join("fx")).unwrap();
    // The one owner step deploy never does (ADR 0009): enable the dev mod in the profile.
    std::fs::write(
        fx.profile_dir.join("modlist.txt"),
        format!("+KitKraft Dev - {name}\r\n+Console API\r\n"),
    )
    .unwrap();
    let out = root.join("out");
    // The playbook, step 3 (the paths are this check's; the words are the playbook's).
    let line = format!(
        "mod run \"{}\" --name {name} --out \"{}\" --mo2 \"{}\" --base-save-sha256 {} --observe \"{}\" --ledger \"{}\" --data-dir \"{}\"",
        spec.display(),
        out.display(),
        fx.instance.display(),
        FIXTURE_SAVE_SHA256,
        fx.observe_root.display(),
        fx.ledger.display(),
        data.display(),
    );
    println!("$ kit {line}");
    let Invocation::Run { command, args } = parse_line(&line).unwrap() else {
        panic!("not a run: {line}");
    };
    assert_eq!(command.id, "mod.run");
    let result = execute(command.id, args).unwrap();
    let d = result["data"].clone();
    for s in d["stages"].as_array().unwrap() {
        println!(
            "  {:<9} {:<8} {}",
            s["stage"].as_str().unwrap(),
            s["status"].as_str().unwrap(),
            s["reason"].as_str().unwrap_or_default()
        );
    }
    for p in d["probes"].as_array().unwrap() {
        println!(
            "  probe {} {} ({})",
            p["row"].as_str().unwrap(),
            p["status"].as_str().unwrap(),
            p["reason"].as_str().unwrap_or_default()
        );
    }
    println!("  => {}", d["summary"].as_str().unwrap());
    let report = std::fs::read_to_string(out.join(name).join("report.md")).unwrap();
    assert!(report.contains("| Result | **partial** |"), "{report}");
    assert!(report.contains("`unverified-profile`"), "{report}");
    // The fixture MO2 now holds the dev mod; the ledger is untouched; nothing launched.
    assert!(fx
        .instance
        .join("mods")
        .join(format!("KitKraft Dev - {name}"))
        .join(format!("{name}.esm"))
        .is_file());
    assert!(!fx.observe_root.exists());
    let _ = std::fs::remove_dir_all(&root);
    d
}

fn check_partial(d: &Value) {
    let stages: Vec<(&str, &str)> = d["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["stage"].as_str().unwrap(), s["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        stages,
        [
            ("build", "pass"),
            ("validate", "pass"),
            ("roundtrip", "pass"),
            ("package", "pass"),
            ("deploy", "pass"),
            ("testplan", "pass"),
            ("session", "not-run"),
        ],
        "{d:#}"
    );
    assert_eq!(d["result"], "partial");
    assert_eq!(d["passed"], true);
    let rules: Vec<&str> = d["session"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect();
    // `running` too when the owner's MO2 or game happens to be open: never a launch.
    assert!(rules.contains(&"unverified-profile"), "{rules:?}");
    assert!(
        rules
            .iter()
            .all(|r| *r == "unverified-profile" || *r == "running"),
        "{rules:?}"
    );
    let probes = d["probes"].as_array().unwrap();
    assert!(!probes.is_empty());
    assert!(probes.iter().all(|p| p["status"] == "not-run"), "{d:#}");
    assert_eq!(d["ledger_entry"], Value::Null);
}

#[test]
fn the_rename_brief_runs_to_the_refused_session() {
    let Some(data) = data_dir() else { return };
    let spec = repo().join("docs").join("smoke-rename.toml");
    let d = run_brief(&spec, "OldSovietMarksmanRifle", &data);
    check_partial(&d);
    // The same bytes as docs/smoke-rename.md's result.
    assert_eq!(
        d["stages"][0]["facts"]["sha256"],
        "9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27"
    );
    assert_eq!(d["probes"][0]["row"], "T-01");
}

#[test]
fn the_first_poi_spec_runs_to_the_refused_session() {
    let Some(data) = data_dir() else { return };
    let spec = repo().join("docs").join("poi").join("first-poi.toml");
    let d = run_brief(&spec, "KitKraftFirstPOI", &data);
    check_partial(&d);
    assert!(d["probes"].as_array().unwrap().len() >= 4, "{d:#}");
}
