//! The controller end to end against the fake `/console` endpoint and a fake host, on a
//! fixture MO2 instance (M4.T3 and M5.C1 acceptance): rows, timeouts, missing evidence,
//! rollback of a changed `plugins.txt`, the authorized-path list, start/status/stop.

#![allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kitkraft_game::controller::{run, Deps, Phase, Plan, Shared};
use kitkraft_game::probes::{Matcher, Probe, ProbeFile, Step, Verdict, FORMAT};
use kitkraft_game::testing::{mo2_fixture, tree, FakeConsole, FakeHost, Fixture};
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
    let mut manual = probe("T-03", false, vec![]);
    manual.matcher = Some("manual".into());
    manual.reason = Some("the prompt text is UI".into());
    ProbeFile {
        format: FORMAT,
        plugin: "X.esm".into(),
        sha256: "00".into(),
        generated_by: "test".into(),
        profile: "KitKraft Dev".into(),
        runtime_id_basis: "inferred".into(),
        notes: vec![],
        probes: vec![
            probe(
                "T-01",
                false,
                vec![step(
                    "help KitKraftCrate 4",
                    Some(Matcher::Contains),
                    "04001000",
                    None,
                )],
            ),
            probe(
                "T-02",
                true,
                vec![
                    step("prid KitKraftCrate", None, "", None),
                    step("getpos x", Some(Matcher::Number), "-17.0", Some(0.5)),
                ],
            ),
            manual,
            probe(
                "T-04",
                false,
                vec![
                    step("coc CityNewAtlantisLodgeInt", None, "", None),
                    step("save {save}", None, "", None),
                    step("load {save}", None, "", None),
                    step("prid KitKraftCrate", None, "", None),
                    step("getpos x", Some(Matcher::Number), "-17.0", Some(0.5)),
                ],
            ),
            probe(
                "T-05",
                false,
                vec![step("", Some(Matcher::PapyrusLog), "KitKraft: hello", None)],
            ),
        ],
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
    c.answer("GetSFSEVersion", "SFSE version: 0.2.21");
    c.answer("help KitKraftCrate", "REFR: KitKraftCrate (04001000)");
    c.answer("getpos x", "GetPos: X >> -17.10");
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

fn verdicts(rows: &[kitkraft_game::ledger::RowReport]) -> Vec<String> {
    rows.iter().map(|r| r.verdict.text()).collect()
}

#[test]
fn a_session_runs_every_row_and_writes_only_authorized_paths() {
    let f = fixture("happy");
    let papyrus = f.root.join("Papyrus.0.log");
    std::fs::write(&papyrus, "old line\n").unwrap();
    let c = console();
    let papyrus_for_hook = papyrus.clone();
    c.on_command(move |cmd| {
        if cmd.starts_with("coc ") {
            let mut text = std::fs::read_to_string(&papyrus_for_hook).unwrap();
            text.push_str("[10/16] KitKraft: hello\n");
            std::fs::write(&papyrus_for_hook, text).unwrap();
        }
    });
    let mo2_before = tree(&f.instance);
    let h = host();
    let p = plan(&f, &c, &papyrus, "2026-10-16-T01");
    let shared = Shared::new(&p.session);
    let report = run(&p, deps(&h, &c), &shared);

    assert_eq!(
        verdicts(&report.rows),
        [
            "pass",
            "pass",
            "not-run (manual: the prompt text is UI)",
            "pass",
            "pass"
        ]
    );
    assert_eq!(
        report.status.label(),
        "not-run",
        "a manual row keeps the session open for the owner"
    );
    // The commands, in order: readiness, the base save, then the rows ({save} filled).
    let sent = c.received();
    assert_eq!(sent[0], "GetSFSEVersion");
    assert!(sent.contains(&"load KitKraftBase".to_string()));
    assert!(sent.contains(&"save KitKraftSession-20261016-01".to_string()));
    assert!(sent.contains(&"load KitKraftSession-20261016-01".to_string()));
    // Launched through MO2 on the named profile; ended at the end.
    let launched = h.launched();
    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].1, ["-p", "Kitkraft Dev", "moshortcut://:SFSE"]);
    assert!(
        h.now_running().iter().all(|r| r == "explorer.exe"),
        "{:?}",
        h.now_running()
    );
    assert!(report.terminated.contains(&"Starfield.exe".to_string()));
    // Evidence: the screenshot of T-02, the copied Papyrus log, the missing history log.
    assert_eq!(report.rows[1].screenshots, ["T-02-1.png"]);
    assert!(p.observe.join("screenshots").join("T-02-1.png").is_file());
    assert!(report
        .logs
        .iter()
        .any(|(n, c)| n == "papyrus" && c.is_some()));
    assert!(report
        .logs
        .iter()
        .any(|(n, c)| n == "console-history" && c.is_none()));
    // Backups with hashes; nothing changed; the MO2 instance is byte-identical.
    assert_eq!(report.backups.len(), 8);
    assert!(report.changes.is_empty());
    assert_eq!(tree(&f.instance), mo2_before);
    // The ledger got one entry; every write is on the authorized list.
    let ledger = std::fs::read_to_string(&f.ledger).unwrap();
    assert_eq!(report.ledger_entry.as_deref(), Some("game-20261016-01"));
    assert!(ledger.contains("### game-20261016-01: agent session 2026-10-16-T01"));
    assert!(ledger.contains("| T-01 | `help KitKraftCrate 4` |"));
    for w in &report.written {
        assert!(
            w.starts_with(&p.observe) || *w == f.ledger,
            "unauthorized write {}",
            w.display()
        );
    }
    assert!(p.observe.join("report.json").is_file());
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn a_changed_plugins_txt_is_restored_and_the_session_is_inconclusive() {
    let f = fixture("rollback");
    let c = console();
    let h = host();
    let plugins = f.profile_dir.join("plugins.txt");
    let original = std::fs::read(&plugins).unwrap();
    let target = plugins.clone();
    h.on_launch(move || std::fs::write(&target, "# header\r\n*X.esm\r\n*Other.esm\r\n").unwrap());
    let p = plan(&f, &c, &f.root.join("no-papyrus.log"), "2026-10-16-T02");
    let report = run(&p, deps(&h, &c), &Shared::new(&p.session));
    assert_eq!(std::fs::read(&plugins).unwrap(), original);
    assert_eq!(report.changes.len(), 1);
    assert!(report.changes[0].restored);
    assert_eq!(report.status.label(), "inconclusive");
    assert!(report
        .findings
        .iter()
        .any(|x| x.contains("plugins.txt changed during the session")));
    let ledger = std::fs::read_to_string(&f.ledger).unwrap();
    assert!(ledger.contains("| Status | inconclusive ("));
    assert!(ledger.contains("Finding: "));
    // The restore is the one write outside observe, and it is on the list.
    assert!(report.written.contains(&plugins));
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn missing_evidence_makes_rows_inconclusive() {
    let f = fixture("evidence");
    let c = console();
    c.answer("help KitKraftCrate", "");
    let h = host();
    h.fail_screenshots();
    let p = plan(&f, &c, &f.root.join("never-written.log"), "2026-10-16-T03");
    let report = run(&p, deps(&h, &c), &Shared::new(&p.session));
    let v = verdicts(&report.rows);
    assert_eq!(v[0], "inconclusive (no console output to match)");
    assert!(
        v[1].starts_with("inconclusive (screenshot not taken"),
        "{v:?}"
    );
    assert_eq!(v[4], "inconclusive (the Papyrus log was not written)");
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn a_probe_timeout_ends_the_session_and_the_rest_is_not_run() {
    let f = fixture("timeout");
    let c = console();
    c.delay("getpos", Duration::from_secs(4));
    let h = host();
    let p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T04");
    let report = run(&p, deps(&h, &c), &Shared::new(&p.session));
    let v = verdicts(&report.rows);
    assert_eq!(v[0], "pass");
    assert!(v[1].starts_with("inconclusive (timeout"), "{v:?}");
    for later in &v[2..] {
        assert_eq!(later, "not-run (timeout at row T-02)");
    }
    assert!(h.now_running().iter().all(|r| r == "explorer.exe"));
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn an_endpoint_that_never_answers_leaves_every_row_not_run() {
    let f = fixture("noready");
    let c = console();
    c.set_down(true);
    let h = host();
    let p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T05");
    let report = run(&p, deps(&h, &c), &Shared::new(&p.session));
    assert!(report
        .rows
        .iter()
        .all(|r| matches!(&r.verdict, Verdict::NotRun(w) if w.contains("timeout after 3s waiting for the console endpoint"))));
    assert!(
        report.terminated.contains(&"Starfield.exe".to_string()),
        "{:?}",
        report.terminated
    );
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn saves_and_loads_outside_the_session_names_are_refused() {
    let f = fixture("refuse");
    let c = console();
    let h = host();
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T06");
    p.probes.probes[3].steps[1].command = "save MyRealSave".into();
    p.probes.probes[0].steps[0].command = "bat kitkraft-test".into();
    let report = run(&p, deps(&h, &c), &Shared::new(&p.session));
    let v = verdicts(&report.rows);
    assert!(v[0].contains("`bat` is never sent"), "{v:?}");
    assert!(v[3].contains("saves only to `KitKraftSession-*`"), "{v:?}");
    assert!(!c
        .received()
        .iter()
        .any(|s| s.contains("MyRealSave") || s.starts_with("bat")));
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn start_status_probe_and_stop_follow_a_session_on_its_thread() {
    let f = fixture("thread");
    let c = console();
    let h = host();
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T07");
    p.stay = true;
    let shared = Shared::new(&p.session);
    let d = deps(&h, &c);
    let s2 = Arc::clone(&shared);
    let plan2 = p.clone();
    let handle = std::thread::spawn(move || run(&plan2, d, &s2));
    let mut polls = 0;
    while shared.snapshot().phase != Phase::Idle {
        polls += 1;
        assert!(polls < 500, "{:?}", shared.snapshot());
        std::thread::sleep(Duration::from_millis(20));
    }
    let snap = shared.snapshot();
    assert_eq!(snap.rows.len(), 5);
    assert_eq!(snap.screenshots.len(), 1);
    let rerun = shared.probe("T-01", true, Duration::from_secs(5)).unwrap();
    assert_eq!(rerun.verdict, Verdict::Pass);
    assert_eq!(rerun.screenshots, ["T-01-1.png"]);
    let unknown = shared.probe("T-99", false, Duration::from_secs(5)).unwrap();
    assert_eq!(unknown.verdict.label(), "not-run");
    shared.request_stop();
    let report = handle.join().unwrap();
    assert_eq!(report.reruns.len(), 2);
    assert_eq!(shared.snapshot().phase, Phase::Finished);
    assert!(shared.snapshot().report.is_some());
    assert!(shared.probe("T-01", false, Duration::from_secs(1)).is_err());
    std::fs::remove_dir_all(&f.root).unwrap();
}
