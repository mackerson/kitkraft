//! The readiness wait near its own deadline: a poll whose timeout the deadline cut short
//! says nothing new, so the wait's reason keeps what the last reply showed. Under load the
//! last poll of `an_empty_answer_is_not_ready` (session.rs, 1 s budget) ran into the
//! deadline and the reason became "(timeout)", failing the test now and then. Here a slow
//! console makes that happen every time. Helpers copied from `session.rs`.

#![allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kitkraft_game::controller::{run, Deps, Plan, Shared};
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

#[test]
fn a_poll_cut_short_by_the_deadline_keeps_the_last_replys_reason() {
    let f = fixture("ready-deadline");
    let c = console();
    c.answer(
        "GetSFSEVersion",
        " 
",
    );
    // Each poll takes 400 ms, so the last one before the 1 s deadline gets less than that.
    c.delay("GetSFSEVersion", Duration::from_millis(400));
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T10");
    p.game.console.ready_expect.clear();
    p.timeouts.ready_secs = 1;
    let report = run(&p, deps(&host(), &c), &Shared::new(&p.session));
    let why = report.rows.first().map(|r| r.verdict.text()).unwrap();
    std::fs::remove_dir_all(&f.root).unwrap();
    assert!(
        why.contains("timeout after 1s waiting for the console endpoint")
            && why.contains("answered an empty body, not ready"),
        "{why}"
    );
}
