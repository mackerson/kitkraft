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

/// A step's `wait_secs` pauses before the next step (game-20261101-T1: the game answers
/// `player.moveto` at once and a command sent while it moves answers with only its echo).
#[test]
fn a_step_wait_pauses_before_the_next_step() {
    let f = fixture("wait");
    let c = console();
    type Sent = Arc<std::sync::Mutex<Vec<(String, std::time::Instant)>>>;
    let at: Sent = Arc::default();
    let log = Arc::clone(&at);
    c.on_command(move |cmd| {
        log.lock()
            .unwrap()
            .push((cmd.to_string(), std::time::Instant::now()));
    });
    let mut moveto = step("player.moveto 06001000", None, "", None);
    moveto.wait_secs = Some(1);
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T09");
    p.probes.probes = vec![probe(
        "T-01",
        false,
        vec![
            moveto,
            step("getpos x", Some(Matcher::Number), "-17.0", Some(0.5)),
        ],
    )];
    let report = run(&p, deps(&host(), &c), &Shared::new(&p.session));
    assert_eq!(verdicts(&report.rows), ["pass"]);
    assert!(
        report.rows[0].seen.contains("(waited 1s)"),
        "{}",
        report.rows[0].seen
    );
    {
        let at = at.lock().unwrap();
        let when = |c: &str| at.iter().find(|(x, _)| x == c).map(|(_, t)| *t).unwrap();
        assert!(
            when("getpos x").duration_since(when("player.moveto 06001000"))
                >= Duration::from_secs(1),
            "getpos x was sent before the wait was over"
        );
    }
    // A wait longer than the row's deadline (2 s here) is cut at the deadline: the next
    // step is then a timeout, never a pass.
    let mut long = step("player.moveto 06001000", None, "", None);
    long.wait_secs = Some(30);
    p.session = "2026-10-16-T10".into();
    p.observe = f.observe_root.join(&p.session);
    p.probes.probes = vec![probe(
        "T-01",
        false,
        vec![
            long,
            step("getpos x", Some(Matcher::Number), "-17.0", Some(0.5)),
        ],
    )];
    let started = std::time::Instant::now();
    let report = run(&p, deps(&host(), &c), &Shared::new(&p.session));
    assert!(started.elapsed() < Duration::from_secs(25));
    assert!(
        report.rows[0]
            .verdict
            .text()
            .starts_with("inconclusive (timeout"),
        "{}",
        report.rows[0].verdict.text()
    );
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

/// 2026-11-01-T03, row T-05: `load` answered "Save file not found." (sent before the save
/// was written) and the later getter passed on the state the load never left. A console
/// failure line fails the row, naming the step, whatever the later matchers say.
#[test]
fn a_console_failure_line_fails_the_row_whatever_the_later_matchers_say() {
    let f = fixture("notfound");
    let c = console();
    c.answer(
        "load KitKraftSession-",
        "load KitKraftSession-20261016-01\nCommand: load KitKraftSession-20261016-01\nSave file not found.\n",
    );
    c.answer(
        "prid KitKraftCrate",
        "prid KitKraftCrate\nCommand: prid KitKraftCrate\nItem 'KitKraftCrate' not found for parameter ObjectReferenceID.\n",
    );
    let p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T11");
    let report = run(&p, deps(&host(), &c), &Shared::new(&p.session));
    let v = verdicts(&report.rows);
    // T-02: `prid` is step 1, the getpos after it still "passes".
    assert_eq!(
        v[1],
        "fail (step 1 `prid KitKraftCrate`: the console answered `Item 'KitKraftCrate' not found for parameter ObjectReferenceID.`)"
    );
    // T-04: the load is step 3; the first failure is named, not the later prid.
    assert_eq!(
        v[3],
        "fail (step 3 `load KitKraftSession-20261016-01`: the console answered `Save file not found.`)"
    );
    // Every step still ran (the evidence is kept).
    assert!(
        report.rows[3].seen.contains("getpos x -> "),
        "{}",
        report.rows[3].seen
    );
    // A missing command fails a row that has no matcher on that step.
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T12");
    c.answer(
        "isdisabled",
        "isdisabled\nCommand: isdisabled\nScript command \"isdisabled\" not found.",
    );
    p.probes.probes = vec![probe(
        "T-01",
        false,
        vec![
            step("isdisabled", None, "", None),
            step("getpos x", Some(Matcher::Number), "-17.0", Some(0.5)),
        ],
    )];
    let report = run(&p, deps(&host(), &c), &Shared::new(&p.session));
    assert_eq!(
        verdicts(&report.rows),
        ["fail (step 1 `isdisabled`: the console answered `Script command \"isdisabled\" not found.`)"]
    );
    std::fs::remove_dir_all(&f.root).unwrap();
}

/// A base save the game does not find leaves the game at the main menu: no row runs.
#[test]
fn a_base_save_not_found_leaves_every_row_not_run() {
    let f = fixture("nobase");
    let c = console();
    c.answer(
        "load KitKraftBase",
        "load KitKraftBase\nCommand: load KitKraftBase\nSave file not found.\n",
    );
    let p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T13");
    let report = run(&p, deps(&host(), &c), &Shared::new(&p.session));
    assert!(
        report.rows.iter().all(|r| matches!(&r.verdict,
            Verdict::NotRun(w) if w.contains("the base save did not load") && w.contains("Save file not found."))),
        "{:?}",
        verdicts(&report.rows)
    );
    assert!(!c.received().iter().any(|s| s.starts_with("help ")));
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

// The readiness rules (game-20261101-T1/b'): no command before the window, none during
// the settle, an empty answer is not ready, ready after a non-empty one, and one deadline
// over it all.

fn first_rows_reason(report: &kitkraft_game::controller::Report) -> String {
    match report.rows.first().map(|r| &r.verdict) {
        Some(Verdict::NotRun(why)) => why.clone(),
        v => format!("not a not-run row: {v:?}"),
    }
}

#[test]
fn no_command_is_sent_before_the_game_has_a_main_window() {
    let f = fixture("nowindow");
    let c = console();
    let h = host();
    h.set_window_after(None);
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T08");
    p.timeouts.ready_secs = 1;
    let report = run(&p, deps(&h, &c), &Shared::new(&p.session));
    assert!(c.received().is_empty(), "{:?}", c.received());
    let why = first_rows_reason(&report);
    assert!(
        why.contains("timeout after 1s waiting for the console endpoint")
            && why.contains("no main window of Starfield.exe yet; nothing sent"),
        "{why}"
    );
    assert!(report.rows.iter().all(|r| r.verdict.label() == "not-run"));
    assert!(report.terminated.contains(&"Starfield.exe".to_string()));
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn no_command_is_sent_until_the_window_has_settled() {
    let f = fixture("settle");
    let c = console();
    let first = Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
    let seen = Arc::clone(&first);
    c.on_command(move |_| {
        seen.lock()
            .unwrap()
            .get_or_insert_with(std::time::Instant::now);
    });
    let h = host();
    h.set_window_after(Some(Duration::from_millis(300)));
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T09");
    p.timeouts.settle_secs = 1;
    p.timeouts.ready_secs = 5;
    let shared = Shared::new(&p.session);
    let report = run(&p, deps(&h, &c), &shared);
    assert_eq!(report.rows[0].verdict, Verdict::Pass, "{:?}", report.rows);
    let window = h.window_seen_at().unwrap();
    let first = first.lock().unwrap().unwrap();
    assert!(
        first >= window + Duration::from_secs(1),
        "a command {:?} after the window, inside the 1s settle",
        first.duration_since(window)
    );
    assert_eq!(c.received()[0], "GetSFSEVersion");
    assert!(shared
        .snapshot()
        .events
        .iter()
        .any(|e| e.contains("the main window of Starfield.exe is up")
            && e.contains("no command for 1s")));
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn an_empty_answer_is_not_ready() {
    let f = fixture("empty");
    let c = console();
    c.answer("GetSFSEVersion", " \r\n");
    let h = host();
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T10");
    assert!(p.game.console.ready_expect.is_empty());
    p.timeouts.ready_secs = 1;
    let report = run(&p, deps(&h, &c), &Shared::new(&p.session));
    let why = first_rows_reason(&report);
    assert!(
        why.contains("timeout after 1s waiting for the console endpoint")
            && why.contains("answered an empty body, not ready"),
        "{why}"
    );
    let sent = c.received();
    assert!(sent.len() > 1, "polled more than once: {sent:?}");
    assert!(sent.iter().all(|s| s == "GetSFSEVersion"), "{sent:?}");
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn the_endpoint_is_ready_after_its_first_non_empty_answer() {
    let f = fixture("nonempty");
    let c = console();
    c.answer_in_turn("GetSFSEVersion", &["", "  ", "SFSE version: 0.2.21"]);
    let h = host();
    let p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T11");
    let shared = Shared::new(&p.session);
    let report = run(&p, deps(&h, &c), &shared);
    let sent = c.received();
    assert_eq!(
        sent[..4],
        [
            "GetSFSEVersion",
            "GetSFSEVersion",
            "GetSFSEVersion",
            "load KitKraftBase"
        ],
        "{sent:?}"
    );
    assert_eq!(report.rows[0].verdict, Verdict::Pass);
    assert!(shared
        .snapshot()
        .events
        .iter()
        .any(|e| e.contains("`GetSFSEVersion` answered `SFSE version: 0.2.21`")));
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn the_ready_deadline_covers_the_window_and_the_settle() {
    let f = fixture("deadline");
    let c = console();
    let h = host();
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T12");
    p.timeouts.settle_secs = 30;
    p.timeouts.ready_secs = 1;
    let started = std::time::Instant::now();
    let report = run(&p, deps(&h, &c), &Shared::new(&p.session));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the deadline held"
    );
    assert!(c.received().is_empty(), "{:?}", c.received());
    let why = first_rows_reason(&report);
    assert!(
        why.contains("timeout after 1s waiting for the console endpoint")
            && why.contains("settling: the main window has been up")
            && why.contains("of 30s; nothing sent"),
        "{why}"
    );
    std::fs::remove_dir_all(&f.root).unwrap();
}

/// Ruling R20 (2026-11-03-T02/T03): a getter that answers with only its echo is sent again
/// after a pause; still only the echo after three tries is inconclusive, never a fail. An
/// answer that arrives on a retry is judged as usual.
#[test]
fn an_echo_only_answer_is_retried_then_inconclusive() {
    let f = fixture("echo");
    let c = console();
    c.answer(
        "help KitKraftEcho",
        "help KitKraftEcho 4\nCommand: help KitKraftEcho 4",
    );
    c.answer_in_turn(
        "getpos x",
        &[
            "getpos x\nCommand: getpos x",
            "getpos x\nCommand: getpos x\nGetPos: X >> -17.10",
        ],
    );
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T11");
    p.probes.probes = vec![
        probe(
            "T-01",
            false,
            vec![step(
                "help KitKraftEcho 4",
                Some(Matcher::Contains),
                "04001000",
                None,
            )],
        ),
        probe(
            "T-02",
            false,
            vec![step("getpos x", Some(Matcher::Number), "-17.0", Some(0.5))],
        ),
    ];
    let report = run(&p, deps(&host(), &c), &Shared::new(&p.session));
    let v = verdicts(&report.rows);
    assert!(
        v[0].starts_with(
            "inconclusive (`help KitKraftEcho 4` answered with only its echo, 3 time(s)"
        ),
        "{v:?}"
    );
    assert_eq!(v[1], "pass", "{v:?}");
    let sent = c.received();
    assert_eq!(
        sent.iter()
            .filter(|x| x.starts_with("help KitKraftEcho"))
            .count(),
        3
    );
    assert_eq!(sent.iter().filter(|x| x.as_str() == "getpos x").count(), 2);
    std::fs::remove_dir_all(&f.root).unwrap();
}

/// Ruling R28 (game-20261103-03, 2026-11-03-T05): the console's answers to `help` came one
/// or more requests late, each with the echo of the request it was sent for. A reply line
/// goes to the request it names (the `help` term, the getter's function), never to the
/// request whose reply it arrived in; a line for an earlier request is re-queued to it and
/// that row is judged again; a request with no line of its own after the retries is
/// `inconclusive` ("no correlated reply"), never `fail`. The transcript is T05's: T-03's
/// reply held T-01's `KYWD`, T-04's held T-02's `TMLM`, T-07's held T-03's `TERM`.
#[test]
fn a_reply_goes_to_the_request_it_names_not_the_one_it_arrived_in() {
    let f = fixture("correlate");
    let c = console();
    let echo = |cmd: &str| cmd.to_string();
    let with = |cmd: &str, line: &str| format!("{cmd}\n{line}");
    let t01 = "help KitKraftLocTypeMiniSite 4";
    let t02 = "help \"KitKraft Mini Site Menu\" 4";
    let t03 = "help \"KitKraft Site Terminal\" 4";
    let t04 = "help KitKraftMiniSitePackIn 4";
    let t06 = "help \"KitKraft Mini Site Crew\" 4";
    let t07 = "help \"KitKraft Mini Site\" 4";
    let kywd = "KYWD: KitKraftLocTypeMiniSite (09001000) ''";
    let tmlm = "TMLM:  (09001001) 'KitKraft Mini Site Menu'";
    let term = "TERM:  (09001002) 'KitKraft Site Terminal'";
    let lctn = "LCTN: KitKraftMiniSiteLocation (09001006) 'KitKraft Mini Site'";
    c.answer(t01, &echo(t01));
    c.answer(t02, &echo(t02));
    c.answer_in_turn(t03, &[&echo(t03), &with(t03, kywd), &echo(t03)]);
    c.answer_in_turn(t04, &[&echo(t04), &echo(t04), &with(t04, tmlm)]);
    c.answer(
        "getpos x",
        "getpos x\nCommand: getpos x\nGetPos: X >> -17.00",
    );
    c.answer(t06, &echo(t06));
    c.answer_in_turn(t07, &[&with(t07, term), &with(t07, lctn)]);
    let mut p = plan(&f, &c, &f.root.join("p.log"), "2026-10-16-T12");
    let help = |row: &str, cmd: &str, id: &str| {
        probe(
            row,
            false,
            vec![step(cmd, Some(Matcher::Contains), id, None)],
        )
    };
    p.probes.probes = vec![
        help("T-01", t01, "09001000"),
        help("T-02", t02, "09001001"),
        help("T-03", t03, "09001002"),
        help("T-04", t04, "09001003"),
        probe(
            "T-05",
            false,
            vec![step("getpos x", Some(Matcher::Number), "-17", Some(0.5))],
        ),
        help("T-06", t06, "09001005"),
        help("T-07", t07, "09001006"),
    ];
    let report = run(&p, deps(&host(), &c), &Shared::new(&p.session));
    let v = verdicts(&report.rows);
    // Late, but correlated: judged on the line that names them.
    assert_eq!(v[0], "pass", "T-01 {v:?}");
    assert_eq!(v[1], "pass", "T-02 {v:?}");
    assert_eq!(v[2], "pass", "T-03 {v:?}");
    // T-04 never got its own line: inconclusive, not the fail T05 recorded.
    assert!(
        v[3].starts_with("inconclusive (`help KitKraftMiniSitePackIn 4`: no correlated reply"),
        "T-04 {v:?}"
    );
    // An answer in order is judged as before.
    assert_eq!(v[4], "pass", "T-05 {v:?}");
    // Only the echo, three times.
    assert!(
        v[5].starts_with(
            "inconclusive (`help \"KitKraft Mini Site Crew\" 4` answered with only its echo, 3 time(s): no correlated reply"
        ),
        "T-06 {v:?}"
    );
    // T-07's first reply held T-03's line; its own came on the retry.
    assert_eq!(v[6], "pass", "T-07 {v:?}");
    assert!(
        report.rows[0].seen.contains("09001000"),
        "{}",
        report.rows[0].seen
    );
    assert!(
        report.rows[2].seen.contains("re-queued"),
        "{}",
        report.rows[2].seen
    );
    assert!(!v.iter().any(|x| x.starts_with("fail")), "{v:?}");
    std::fs::remove_dir_all(&f.root).unwrap();
}

// ------------------------------------------------- reviewer probes (rev-x5, reviews/x-controller-R27-R28.md)
// P1 to P8 as the review wrote them, each with the grade the review calls the only
// defensible one (H1, H2, M1, M2); then one test per routing rule that survived mutation
// (M3).

fn rx_run(tag: &str, c: &FakeConsole, rows: Vec<Probe>) -> (Vec<String>, Vec<String>) {
    let f = fixture(tag);
    let mut p = plan(&f, c, &f.root.join("p.log"), "2026-10-16-T13");
    p.probes.probes = rows;
    let report = run(&p, deps(&host(), c), &Shared::new(&p.session));
    let v = verdicts(&report.rows);
    let seen: Vec<String> = report.rows.iter().map(|r| r.seen.clone()).collect();
    let ledger = std::fs::read_to_string(&f.ledger).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&f.root);
    eprintln!("VERDICTS {tag}: {v:#?}");
    eprintln!("SEEN {tag}: {seen:#?}");
    eprintln!(
        "LEDGER-HAS-AMBIG {tag}: {}",
        ledger.to_ascii_lowercase().contains("ambig") || ledger.contains("cannot tell")
    );
    (v, seen)
}

fn one(row: &str, cmd: &str, m: Matcher, exp: &str, tol: Option<f64>) -> Probe {
    probe(row, false, vec![step(cmd, Some(m), exp, tol)])
}

/// P1: the same getter twice. T-04's three sends answer only the echo; T-06 (same command)
/// gets one `GetPos` line, which may be the late answer to any of T-04's three sends.
#[test]
fn rx_p1_same_getter_twice_ambiguous_line() {
    let c = console();
    c.answer_in_turn(
        "getpos x",
        &[
            "getpos x",
            "getpos x",
            "getpos x",
            "getpos x\nGetPos: X >> -17.00",
        ],
    );
    let (v, seen) = rx_run(
        "rx-p1",
        &c,
        vec![
            one("T-04", "getpos x", Matcher::Number, "-17", Some(0.5)),
            one("T-06", "getpos x", Matcher::Number, "-17", Some(0.5)),
        ],
    );
    assert!(
        v[1].starts_with("inconclusive (ambiguous reply"),
        "P1 {v:?}"
    );
    assert!(seen[1].contains("ambiguous"), "P1 {seen:?}");
}

/// P2: the persistence row (testplan `in_cell` x3). `GetInCell` names no argument, so
/// every getincell claims every GetInCell line equally. Step 2 is sent twice and answered
/// once: one send is outstanding when steps 5 and 7 take a line each.
#[test]
fn rx_p2_save_reload_row_shifted_getincell() {
    let c = console();
    let l = "player.getincell 0006BB29";
    let a = "player.getincell 0021966F";
    let one_line = |cmd: &str| format!("{cmd}\nGetInCell >> 1.00");
    let l1 = one_line(l);
    c.answer_in_turn(l, &[l, &l1, &l1]);
    c.answer(a, &one_line(a));
    let re = r"(?m)>>\s*1(\.0+)?\s*$";
    let (v, _) = rx_run(
        "rx-p2",
        &c,
        vec![probe(
            "T-08",
            false,
            vec![
                step("coc CityNewAtlantisLodgeInt", None, "", None),
                step(l, Some(Matcher::Regex), re, None),
                step("save {save}", None, "", None),
                step("coc CityNewAtlantisApartmentTEMPLATE", None, "", None),
                step(a, Some(Matcher::Regex), re, None),
                step("load {save}", None, "", None),
                step(l, Some(Matcher::Regex), re, None),
            ],
        )],
    );
    assert!(
        v[0].starts_with("inconclusive (ambiguous reply"),
        "P2 {v:?}"
    );
}

/// P3a: different args; the late line names the earlier getter's args.
#[test]
fn rx_p3a_late_line_names_the_other_args() {
    let c = console();
    c.answer("getpos y", "getpos y");
    c.answer("getpos x", "getpos x\nGetPos: Y >> -4.00");
    let (v, _) = rx_run(
        "rx-p3a",
        &c,
        vec![
            one("T-04", "getpos y", Matcher::Number, "-4", Some(0.5)),
            one("T-05", "getpos x", Matcher::Number, "-4", Some(0.5)),
        ],
    );
    assert_eq!(v[0], "pass", "P3a {v:?}");
    assert!(v[1].starts_with("inconclusive"), "P3a {v:?}");
}

/// P3b: the line names args no request of this session has (`Z`); the reply's own
/// request is `getpos x` and still takes it (first word matches, args named: 0).
#[test]
fn rx_p3b_line_names_args_the_request_does_not_have() {
    let c = console();
    c.answer("getpos x", "getpos x\nGetPos: Z >> -4.00");
    let (v, _) = rx_run(
        "rx-p3b",
        &c,
        vec![one("T-05", "getpos x", Matcher::Number, "-4", Some(0.5))],
    );
    assert!(v[0].starts_with("inconclusive"), "P3b {v:?}");
}

/// P4a: two help terms of equal length (9) in one line, the reply's own one of them.
#[test]
fn rx_p4a_help_equal_length_tie_with_own() {
    let c = console();
    let t1 = "help \"Mini Site\" 4";
    let t2 = "help \"Site Menu\" 4";
    c.answer(t1, t1);
    c.answer(
        t2,
        &format!("{t2}\nTMLM:  (09001001) 'KitKraft Mini Site Menu'"),
    );
    let (v, _) = rx_run(
        "rx-p4a",
        &c,
        vec![
            one("T-01", t1, Matcher::Contains, "09001001", None),
            one("T-02", t2, Matcher::Contains, "09001001", None),
        ],
    );
    assert_eq!(v[1], "pass", "P4a {v:?}");
}

/// P4b: equal-length tie, neither is the reply's own (arrives in a prid reply).
#[test]
fn rx_p4b_help_equal_length_tie_without_own() {
    let c = console();
    let t1 = "help \"Mini Site\" 4";
    let t2 = "help \"Site Menu\" 4";
    c.answer(t1, t1);
    c.answer(t2, t2);
    c.answer(
        "prid 09001001",
        "prid 09001001\nTMLM:  (09001001) 'KitKraft Mini Site Menu'",
    );
    let (v, seen) = rx_run(
        "rx-p4b",
        &c,
        vec![
            one("T-01", t1, Matcher::Contains, "09001001", None),
            one("T-02", t2, Matcher::Contains, "09001001", None),
            probe("T-03", false, vec![step("prid 09001001", None, "", None)]),
        ],
    );
    assert_eq!(v[0], "pass", "P4b {v:?}");
    assert!(v[1].starts_with("inconclusive"), "P4b {v:?}");
    assert!(seen[2].contains("(tie between"), "P4b {seen:?}");
}

/// P4c: a longer earlier term steals the reply's own line, which holds the own term.
#[test]
fn rx_p4c_longer_term_steals_own_line() {
    let c = console();
    let t2 = "help \"KitKraft Mini Site Menu\" 4";
    let t7 = "help \"KitKraft Mini Site\" 4";
    c.answer(
        t2,
        &format!("{t2}\nTMLM:  (09001001) 'KitKraft Mini Site Menu'"),
    );
    c.answer(
        t7,
        &format!("{t7}\nTMLM:  (09001001) 'KitKraft Mini Site Menu'\nLCTN: KitKraftMiniSiteLocation (09001006) 'KitKraft Mini Site'"),
    );
    let (v, _) = rx_run(
        "rx-p4c",
        &c,
        vec![
            one("T-02", t2, Matcher::Contains, "09001001", None),
            one("T-07", t7, Matcher::Contains, "09001001", None),
        ],
    );
    assert_eq!(v[0], "pass", "P4c {v:?}");
    assert_eq!(v[1], "pass", "P4c {v:?}");
}

/// P5a: a getter answered with a console failure line only (game-20261101-T1 row d:
/// `GetInCell <EDID>` answered "Item ... not found for parameter Cell").
#[test]
fn rx_p5a_getter_failure_line_only() {
    let c = console();
    let cmd = "player.getincell CityNewAtlantisApartmentTEMPLATE";
    c.answer(
        cmd,
        &format!("{cmd}\nItem 'CityNewAtlantisApartmentTEMPLATE' not found for parameter Cell."),
    );
    let (v, _) = rx_run(
        "rx-p5a",
        &c,
        vec![one(
            "T-01",
            cmd,
            Matcher::Regex,
            r"(?m)>>\s*1(\.0+)?\s*$",
            None,
        )],
    );
    assert!(v[0].starts_with("fail"), "P5a {v:?}");
}

/// P5b: failure line in the first reply, a getter line on the retry.
#[test]
fn rx_p5b_failure_then_line_on_retry() {
    let c = console();
    let cmd = "player.getitemcount 06001000";
    let r1 = format!("{cmd}\nItem '06001000' not found for parameter InventoryObject.");
    let r2 = format!("{cmd}\nGetItemCount >> 1.00");
    c.answer_in_turn(cmd, &[&r1, &r2]);
    let (v, _) = rx_run(
        "rx-p5b",
        &c,
        vec![one(
            "T-01",
            cmd,
            Matcher::Regex,
            r"(?m)>>\s*1(\.0+)?\s*$",
            None,
        )],
    );
    assert!(v[0].starts_with("fail"), "P5b {v:?}");
}

/// P5c: a failure line starting with a getter's name, plus the getter's own line.
#[test]
fn rx_p5c_failure_line_starting_with_getter_name() {
    let c = console();
    c.answer(
        "getpos x",
        "getpos x\nGetPos: Item 'Q' not found for parameter Axis.\nGetPos: X >> -17.00",
    );
    let (v, _) = rx_run(
        "rx-p5c",
        &c,
        vec![one("T-01", "getpos x", Matcher::Number, "-17", Some(0.5))],
    );
    assert!(v[0].starts_with("fail"), "P5c {v:?}");
}

/// P5d: an earlier row's late failure line (naming that row's item) lands in a later
/// getter reply that also holds its own line.
#[test]
fn rx_p5d_late_failure_line_of_another_row() {
    let c = console();
    c.answer("prid KitKraftFoo", "prid KitKraftFoo");
    c.answer(
        "getpos x",
        "getpos x\nItem 'KitKraftFoo' not found for parameter ObjectReferenceID.\nGetPos: X >> -17.00",
    );
    let (v, _) = rx_run(
        "rx-p5d",
        &c,
        vec![
            probe(
                "T-01",
                false,
                vec![step("prid KitKraftFoo", None, "", None)],
            ),
            one("T-02", "getpos x", Matcher::Number, "-17", Some(0.5)),
        ],
    );
    assert!(v[0].starts_with("fail"), "P5d {v:?}");
    assert!(v[1].starts_with("inconclusive"), "P5d {v:?}");
}

/// P6: one reply holds lines for two earlier requests and its own.
#[test]
fn rx_p6_two_late_lines_in_one_reply() {
    let c = console();
    let a = "help KitKraftAlpha 4";
    let b = "help KitKraftBravo 4";
    c.answer(a, a);
    c.answer(b, b);
    c.answer(
        "getpos x",
        "getpos x\nKYWD: KitKraftAlpha (09001000) ''\nGetPos: X >> -17.00\nKYWD: KitKraftBravo (09001001) ''",
    );
    let (v, _) = rx_run(
        "rx-p6",
        &c,
        vec![
            one("T-01", a, Matcher::Contains, "09001000", None),
            one("T-02", b, Matcher::Contains, "09001001", None),
            one("T-03", "getpos x", Matcher::Number, "-17", Some(0.5)),
        ],
    );
    assert_eq!(v, vec!["pass", "pass", "pass"], "P6");
}

/// P7: late lines arrive after their rows were graded inconclusive and the session moved
/// on; one with a passing value, one with a failing value.
#[test]
fn rx_p7_late_line_after_moving_on() {
    let c = console();
    c.answer("getpos y", "getpos y");
    c.answer("getpos z", "getpos z");
    c.answer(
        "help KitKraftAlpha 4",
        "help KitKraftAlpha 4\nGetPos: Y >> -4.00\nGetPos: Z >> 300.00\nKYWD: KitKraftAlpha (09001000) ''",
    );
    let (v, _) = rx_run(
        "rx-p7",
        &c,
        vec![
            one("T-01", "getpos y", Matcher::Number, "-4", Some(0.5)),
            one("T-02", "getpos z", Matcher::Number, "4.63", Some(0.5)),
            one(
                "T-03",
                "help KitKraftAlpha 4",
                Matcher::Contains,
                "09001000",
                None,
            ),
        ],
    );
    assert_eq!(v[0], "pass", "P7 {v:?}");
    assert!(
        v[1].starts_with("inconclusive") && v[1].contains("answered late"),
        "P7 {v:?}"
    );
    assert_eq!(v[2], "pass", "P7 {v:?}");
}

/// P8: the console never answers anything but the echo; the session ends, bounded.
#[test]
fn rx_p8_console_only_echoes_bounded() {
    let c = console();
    for cmd in ["getpos x", "getpos y", "getpos z", "help A 4", "help B 4"] {
        c.answer(cmd, cmd);
    }
    let t = std::time::Instant::now();
    let (v, _) = rx_run(
        "rx-p8",
        &c,
        vec![
            probe(
                "T-01",
                false,
                vec![
                    step("getpos x", Some(Matcher::Number), "1", Some(0.5)),
                    step("getpos y", Some(Matcher::Number), "1", Some(0.5)),
                    step("getpos z", Some(Matcher::Number), "1", Some(0.5)),
                ],
            ),
            one("T-02", "help A 4", Matcher::Contains, "x", None),
            one("T-03", "help B 4", Matcher::Contains, "x", None),
        ],
    );
    let sent = c.received();
    eprintln!(
        "P8 observed: {v:?}; elapsed {:?}; sends {}",
        t.elapsed(),
        sent.len()
    );
    assert!(v.iter().all(|x| x.starts_with("inconclusive")), "P8 {v:?}");
    for cmd in ["getpos x", "getpos y", "getpos z", "help A 4", "help B 4"] {
        assert_eq!(
            sent.iter().filter(|s| s.as_str() == cmd).count(),
            3,
            "P8 {cmd}"
        );
    }
}

/// M3: a keyed step is judged on its correlated lines only: another request's line that
/// carries the expected text does not make it pass.
#[test]
fn rx_m3_judged_on_correlated_lines_only() {
    let c = console();
    c.answer("help KitKraftAlpha 4", "help KitKraftAlpha 4");
    c.answer(
        "help KitKraftBravo 4",
        "help KitKraftBravo 4
KYWD: KitKraftBravo (09001001) ''
KYWD: KitKraftAlpha (09001000) ''",
    );
    let (v, _) = rx_run(
        "rx-m3a",
        &c,
        vec![
            one(
                "T-01",
                "help KitKraftAlpha 4",
                Matcher::Contains,
                "09001000",
                None,
            ),
            one(
                "T-02",
                "help KitKraftBravo 4",
                Matcher::Contains,
                "09001000",
                None,
            ),
        ],
    );
    assert_eq!(v[0], "pass", "{v:?}");
    assert!(v[1].starts_with("fail"), "{v:?}");
}

/// M3: a getter claims a line only when the line's first word is its function.
#[test]
fn rx_m3_function_must_be_the_first_word() {
    let c = console();
    c.answer(
        "getglobalvalue KitKraftGlobal",
        "getglobalvalue KitKraftGlobal
IsInSpace >> 0.00 GetGlobalValue >> 1.00",
    );
    let (v, _) = rx_run(
        "rx-m3b",
        &c,
        vec![one(
            "T-01",
            "getglobalvalue KitKraftGlobal",
            Matcher::Regex,
            r"(?m)>>\s*1(\.0+)?\s*$",
            None,
        )],
    );
    assert!(v[0].starts_with("inconclusive"), "{v:?}");
}

/// M3: lines nobody names stay with their reply, so a failure line beside a re-queued
/// line still fails the step it arrived for.
#[test]
fn rx_m3_unclaimed_lines_are_kept() {
    let c = console();
    c.answer("help KitKraftAlpha 4", "help KitKraftAlpha 4");
    c.answer(
        "load KitKraftSession",
        "load x
KYWD: KitKraftAlpha (09001000) ''
Save file not found.",
    );
    let (v, _) = rx_run(
        "rx-m3c",
        &c,
        vec![
            one(
                "T-01",
                "help KitKraftAlpha 4",
                Matcher::Contains,
                "09001000",
                None,
            ),
            probe("T-02", false, vec![step("load {save}", None, "", None)]),
        ],
    );
    assert_eq!(v[0], "pass", "{v:?}");
    assert!(v[1].starts_with("fail"), "{v:?}");
}
