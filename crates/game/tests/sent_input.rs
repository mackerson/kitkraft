//! Agent-sent input in the session report (ADR 0010 Amendment 2, KK-362): what
//! `Shared::record_input` records reaches `Report::inputs`, `report.json` and, when there is
//! any, one finding. Helpers copied from `session.rs`.

#![allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kitkraft_game::controller::{run, Deps, Plan, Report, Shared};
use kitkraft_game::input::SentInput;
use kitkraft_game::probes::{Matcher, Probe, ProbeFile, Step, FORMAT};
use kitkraft_game::testing::{mo2_fixture, FakeConsole, FakeHost, Fixture};
use kitkraft_game::{console::HttpConsole, GameProfile};

fn digest(b: &[u8]) -> String {
    // Not SHA-256: the engine passes the real one; any stable digest shows a change.
    let mut h: u64 = 1469598103934665603;
    for x in b {
        h = (h ^ u64::from(*x)).wrapping_mul(1099511628211);
    }
    format!("{h:016x}")
}

fn step(command: &str, m: Option<Matcher>, expected: &str, tol: Option<f64>) -> Step {
    Step {
        command: command.into(),
        matcher: m,
        expected: (!expected.is_empty()).then(|| expected.into()),
        tolerance: tol,
        why: None,
        wait_secs: None,
    }
}

fn probe(row: &str, screenshot: bool, steps: Vec<Step>) -> Probe {
    Probe {
        row: row.into(),
        title: format!("row {row}"),
        forms: vec!["X.esm:001000".into()],
        screenshot,
        matcher: None,
        reason: None,
        steps,
    }
}

fn probes() -> ProbeFile {
    ProbeFile {
        format: FORMAT,
        plugin: "X.esm".into(),
        sha256: "00".into(),
        generated_by: "test".into(),
        profile: "KitKraft Dev".into(),
        runtime_id_basis: "inferred".into(),
        notes: vec![],
        probes: vec![probe(
            "T-01",
            false,
            vec![step(
                "help KitKraftCrate 4",
                Some(Matcher::Contains),
                "04001000",
                None,
            )],
        )],
    }
}

fn plan(f: &Fixture, console: &FakeConsole, papyrus: &Path, session: &str) -> Plan {
    let mut game = GameProfile::named("starfield").unwrap();
    game.launch.evidence = "fixture".into();
    game.console.evidence = "fixture".into();
    game.saves.evidence = "fixture".into();
    game.console.url = console.url();
    game.timeouts.probe_secs = 2;
    game.timeouts.ready_secs = 3;
    game.timeouts.settle_secs = 0;
    game.timeouts.ready_poll_secs = 0;
    game.timeouts.load_secs = 3;
    game.timeouts.exit_secs = 1;
    game.timeouts.session_secs = 60;
    let profile_files: Vec<PathBuf> = [
        "modlist.txt",
        "plugins.txt",
        "loadorder.txt",
        "lockedorder.txt",
        "StarfieldCustom.ini",
        "StarfieldPrefs.ini",
        "settings.ini",
    ]
    .iter()
    .map(|n| f.profile_dir.join(n))
    .chain(std::iter::once(f.base_save.clone()))
    .collect();
    Plan {
        session: session.into(),
        date: "2026-10-16".into(),
        timeouts: game.timeouts.clone(),
        game,
        instance: f.instance.clone(),
        mod_manager: f.instance.join("ModOrganizer.exe"),
        profile: f.profile.clone(),
        profile_dir: f.profile_dir.clone(),
        observe: f.observe_root.join(session),
        ledger: Some(f.ledger.clone()),
        base_save: "KitKraftBase".into(),
        base_save_sha256: digest(kitkraft_game::testing::FIXTURE_SAVE),
        session_save: "KitKraftSession-20261016-01".into(),
        probes: probes(),
        probes_path: f.root.join("probes.toml"),
        probes_sha256: "11".repeat(32),
        plugins: vec![("X.esm".into(), "22".repeat(32))],
        backups: profile_files,
        logs: vec![
            ("papyrus".into(), papyrus.to_path_buf()),
            ("console-history".into(), f.root.join("absent-history.log")),
        ],
        versions: vec![("Game build".into(), "unknown (fixture)".into())],
        stay: false,
        poll: Duration::from_millis(20),
    }
}

fn console() -> FakeConsole {
    let c = FakeConsole::start().unwrap();
    c.answer(
        "GetSFSEVersion",
        "SFSE version: 0.2.21, release idx 26, runtime 01100F40",
    );
    c.answer("help KitKraftCrate", "REFR: KitKraftCrate (04001000)");
    c
}

fn deps(host: &Arc<FakeHost>, c: &FakeConsole) -> Deps {
    Deps {
        host: Arc::clone(host) as Arc<dyn kitkraft_game::host::Host>,
        console: Box::new(HttpConsole::new(&c.url()).unwrap()),
        digest,
    }
}

fn host() -> Arc<FakeHost> {
    Arc::new(FakeHost::new(
        &["explorer.exe"],
        &[
            "ModOrganizer.exe",
            "sfse_loader.exe",
            "Starfield.exe",
            "usvfs_proxy_x64.exe",
        ],
    ))
}

fn fixture(tag: &str) -> Fixture {
    mo2_fixture(&std::env::temp_dir().join(format!("kitkraft-game-{tag}-{}", std::process::id())))
        .unwrap()
}

/// Runs one session against the fakes, letting `before` record input first; returns the
/// report and the `report.json` the session wrote.
fn session(tag: &str, before: impl FnOnce(&Shared)) -> (Report, serde_json::Value) {
    let f = fixture(tag);
    let c = console();
    let p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T01");
    let shared = Shared::new(&p.session);
    before(&shared);
    let report = run(&p, deps(&host(), &c), &shared);
    let text = std::fs::read_to_string(p.observe.join("report.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    std::fs::remove_dir_all(&f.root).unwrap();
    (report, json)
}

#[test]
fn recorded_input_is_in_the_report_in_order() {
    let (report, json) = session("sent-input", |shared| {
        shared.record_input(Some("T-01"), "key", "F1");
        shared.record_input(None, "focus", "game window");
    });
    assert_eq!(report.inputs.len(), 2, "{:?}", report.inputs);
    let (a, b) = (&report.inputs[0], &report.inputs[1]);
    assert_eq!(
        (a.row.as_deref(), a.kind.as_str(), a.detail.as_str()),
        (Some("T-01"), "key", "F1")
    );
    assert_eq!(
        (b.row.as_deref(), b.kind.as_str(), b.detail.as_str()),
        (None, "focus", "game window")
    );
    assert!(b.at_ms >= a.at_ms, "{a:?} {b:?}");
    let said: Vec<&String> = report
        .findings
        .iter()
        .filter(|l| l.starts_with("agent input sent"))
        .collect();
    assert_eq!(said.len(), 1, "{:?}", report.findings);
    assert!(said[0].starts_with("agent input sent: 2"), "{said:?}");
    // The ledger entry carries the findings too.
    assert!(
        report.ledger_text.contains("agent input sent: 2"),
        "{}",
        report.ledger_text
    );
    let inputs = json["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 2, "{json}");
    let back: Vec<SentInput> = serde_json::from_value(json["inputs"].clone()).unwrap();
    assert_eq!(back, report.inputs);
}

#[test]
fn a_session_without_input_reports_an_empty_list() {
    let (report, json) = session("no-input", |_| {});
    assert!(report.inputs.is_empty(), "{:?}", report.inputs);
    assert!(
        !report
            .findings
            .iter()
            .any(|l| l.starts_with("agent input sent")),
        "{:?}",
        report.findings
    );
    assert_eq!(json["inputs"], serde_json::json!([]), "{json}");
}
