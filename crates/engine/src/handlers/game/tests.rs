//! `game.*` through the catalog on a fixture MO2 instance, with the fake `/console`
//! endpoint and a fake host in the engine's session (M4.T3 acceptance): each preflight
//! refusal, the dry run writing nothing, a synchronous session, and start, status, probe
//! and stop on one engine.

#![allow(clippy::arithmetic_side_effects)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use kitkraft_game::probes::{Matcher, Probe, ProbeFile, Step, FORMAT};
use kitkraft_game::testing::{mo2_fixture, tree, FakeConsole, FakeHost, Fixture, FIXTURE_SAVE};
use kitkraft_game::GameProfile;
use serde_json::{json, Value};

use super::GameHost;
use crate::{parse_line, Engine, Invocation};

fn sha(b: &[u8]) -> String {
    crate::deploy::sha256::hex(b)
}

fn probes_file(f: &Fixture) -> std::path::PathBuf {
    let step = |c: &str, m: Option<Matcher>, e: &str, t: Option<f64>| Step {
        command: c.into(),
        matcher: m,
        expected: (!e.is_empty()).then(|| e.into()),
        tolerance: t,
        why: None,
    };
    let p = ProbeFile {
        format: FORMAT,
        plugin: "X.esm".into(),
        sha256: sha(b"TES4 fixture"),
        generated_by: "test".into(),
        profile: "KitKraft Dev".into(),
        runtime_id_basis: "inferred".into(),
        notes: vec![],
        probes: vec![
            Probe {
                row: "T-01".into(),
                title: "help".into(),
                forms: vec!["X.esm:001000".into()],
                screenshot: false,
                matcher: None,
                reason: None,
                steps: vec![step(
                    "help KitKraftCrate 4",
                    Some(Matcher::Contains),
                    "04001000",
                    None,
                )],
            },
            Probe {
                row: "T-02".into(),
                title: "getpos".into(),
                forms: vec![],
                screenshot: true,
                matcher: None,
                reason: None,
                steps: vec![
                    step("prid KitKraftCrate", None, "", None),
                    step("getpos x", Some(Matcher::Number), "-17", Some(0.5)),
                ],
            },
        ],
    };
    let path = f.root.join("probes.toml");
    std::fs::write(&path, p.to_toml().unwrap()).unwrap();
    path
}

/// A Starfield profile with evidence filled in, the fake endpoint, short timeouts and logs
/// inside the fixture.
fn game_file(f: &Fixture, c: &FakeConsole) -> std::path::PathBuf {
    let mut g = GameProfile::named("starfield").unwrap();
    g.launch.evidence = "fixture".into();
    g.console.evidence = "fixture".into();
    g.saves.evidence = "fixture".into();
    g.console.url = c.url();
    g.timeouts.probe_secs = 3;
    g.timeouts.ready_secs = 3;
    g.timeouts.load_secs = 3;
    g.timeouts.exit_secs = 1;
    g.timeouts.session_secs = 60;
    for l in &mut g.logs {
        l.path = f.root.join(format!("{}.log", l.name)).display().to_string();
    }
    let path = f.root.join("test-game.toml");
    std::fs::write(&path, toml::to_string(&g).unwrap()).unwrap();
    path
}

fn console() -> FakeConsole {
    let c = FakeConsole::start().unwrap();
    c.answer("GetSFSEVersion", "SFSE 0.2.21");
    c.answer("help KitKraftCrate", "REFR KitKraftCrate (04001000)");
    c.answer("getpos x", "GetPos >> -17.00");
    c
}

fn engine(running: &[&str]) -> (Engine, Arc<FakeHost>) {
    let engine = Engine::with_default_handlers();
    let host = Arc::new(FakeHost::new(
        running,
        &["ModOrganizer.exe", "sfse_loader.exe", "Starfield.exe"],
    ));
    let h: Arc<dyn kitkraft_game::host::Host> = host.clone();
    engine.with_session(|s| s.insert(GameHost(h)));
    (engine, host)
}

fn args(f: &Fixture, probes: &Path, game: &Path, extra: Value) -> Value {
    let mut a = json!({
        "mo2": f.instance.display().to_string(),
        "probes": probes.display().to_string(),
        "game": game.display().to_string(),
        "profile": "KitKraft Dev",
        "base_save_sha256": sha(FIXTURE_SAVE),
        "observe": f.observe_root.display().to_string(),
        "ledger": f.ledger.display().to_string(),
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

fn fixture(tag: &str) -> Fixture {
    mo2_fixture(
        &std::env::temp_dir().join(format!("kitkraft-engine-game-{tag}-{}", std::process::id())),
    )
    .unwrap()
}

#[test]
fn every_preflight_refusal_names_its_rule_and_writes_nothing() {
    let c = console();
    type Change = Box<dyn Fn(&Fixture) -> Value>;
    let cases: Vec<(&str, &[&str], Change)> = vec![
        (
            "live-profile",
            &[],
            Box::new(|_| json!({"profile": "Default"})),
        ),
        (
            "selected-profile",
            &[],
            Box::new(|f| {
                std::fs::write(
                    f.instance.join("ModOrganizer.ini"),
                    "[General]\r\nselected_profile=@ByteArray(Default)\r\n",
                )
                .unwrap();
                json!({})
            }),
        ),
        ("running", &["Starfield.exe"], Box::new(|_| json!({}))),
        ("running", &["ModOrganizer.exe"], Box::new(|_| json!({}))),
        ("running", &["CreationKit.exe"], Box::new(|_| json!({}))),
        (
            "profile-settings",
            &[],
            Box::new(|f| {
                std::fs::write(
                    f.profile_dir.join("settings.ini"),
                    "[General]\r\nLocalSaves=true\r\nLocalSettings=false\r\n",
                )
                .unwrap();
                json!({})
            }),
        ),
        (
            "ini-switch",
            &[],
            Box::new(|f| {
                std::fs::write(
                    f.profile_dir.join("StarfieldCustom.ini"),
                    "[Archive]\r\nbInvalidateOlderFiles=1\r\n",
                )
                .unwrap();
                json!({})
            }),
        ),
        (
            "console-plugin",
            &[],
            Box::new(|f| {
                std::fs::write(
                    f.profile_dir.join("modlist.txt"),
                    "+KitKraft Dev - X\r\n-Console API\r\n",
                )
                .unwrap();
                json!({})
            }),
        ),
        (
            "base-save",
            &[],
            Box::new(|_| json!({"base_save_sha256": "00".repeat(32)})),
        ),
        (
            "base-save",
            &[],
            Box::new(|f| {
                std::fs::remove_file(&f.base_save).unwrap();
                json!({})
            }),
        ),
        (
            "plugin-not-deployed",
            &[],
            Box::new(|f| {
                std::fs::remove_file(f.plugin.with_file_name("kitkraft-deploy.json")).unwrap();
                json!({})
            }),
        ),
        (
            "plugin-not-deployed",
            &[],
            Box::new(|f| {
                std::fs::write(
                    f.profile_dir.join("modlist.txt"),
                    "-KitKraft Dev - X\r\n+Console API\r\n",
                )
                .unwrap();
                json!({})
            }),
        ),
        (
            "unverified-profile",
            &[],
            Box::new(|_| json!({"game": "starfield"})),
        ),
        (
            "unverified-profile",
            &[],
            Box::new(|_| json!({"game": "skyrimse"})),
        ),
        (
            "ledger-path",
            &[],
            Box::new(|f| json!({"ledger": f.root.join("elsewhere.md").display().to_string()})),
        ),
    ];
    for (i, (rule, running, change)) in cases.into_iter().enumerate() {
        let f = fixture(&format!("refuse{i}"));
        let probes = probes_file(&f);
        let game = game_file(&f, &c);
        let extra = change(&f);
        let before = tree(&f.instance);
        let ledger_before = std::fs::read(&f.ledger).unwrap();
        let (e, host) = engine(running);
        let err = e
            .execute("game.session", args(&f, &probes, &game, extra.clone()))
            .unwrap_err();
        assert!(
            err.to_string().contains(&format!("refused [{rule}]")),
            "{rule}: {err}"
        );
        assert!(host.launched().is_empty(), "{rule}: launched");
        assert_eq!(tree(&f.instance), before, "{rule}: MO2 changed");
        assert_eq!(std::fs::read(&f.ledger).unwrap(), ledger_before, "{rule}");
        assert!(!f.observe_root.exists(), "{rule}: observe written");
        // The dry run reports the same refusal and writes nothing either.
        let mut dry = args(&f, &probes, &game, extra);
        dry["dry_run"] = json!(true);
        let r = e.execute("game.session", dry).unwrap();
        assert_eq!(r["data"]["would_start"], json!(false), "{rule}");
        assert!(
            r["data"]["refusals"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["rule"] == json!(rule)),
            "{rule}"
        );
        assert!(!f.observe_root.exists());
        std::fs::remove_dir_all(&f.root).unwrap();
    }
    assert!(
        c.received().is_empty(),
        "no refused session talks to the console"
    );
}

#[test]
fn the_real_starfield_profile_is_refused_until_m4_t1_and_says_why() {
    let c = console();
    let f = fixture("unverified");
    let probes = probes_file(&f);
    let game = game_file(&f, &c);
    let (e, _) = engine(&[]);
    let err = e
        .execute(
            "game.session",
            args(&f, &probes, &game, json!({"game": "starfield"})),
        )
        .unwrap_err()
        .to_string();
    for fact in ["`launch`", "`console`", "`saves`", "M4.T1 (a)"] {
        assert!(err.contains(fact), "{fact}: {err}");
    }
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn a_dry_run_prints_the_plan_and_writes_nothing() {
    let c = console();
    let f = fixture("dry");
    let probes = probes_file(&f);
    let game = game_file(&f, &c);
    let (e, host) = engine(&["explorer.exe"]);
    let before = tree(&f.root);
    let r = e
        .execute(
            "game.session",
            args(&f, &probes, &game, json!({"dry_run": true})),
        )
        .unwrap();
    let d = &r["data"];
    assert_eq!(d["would_start"], json!(true), "{d}");
    assert_eq!(d["plan"]["profile"], json!("Kitkraft Dev"));
    assert_eq!(
        d["plan"]["launch"]["args"],
        json!(["-p", "Kitkraft Dev", "moshortcut://:SFSE"])
    );
    assert_eq!(d["plan"]["probes"]["rows"].as_array().unwrap().len(), 2);
    assert_eq!(d["plan"]["backups"].as_array().unwrap().len(), 4 + 3 + 1);
    assert!(d["plan"]["session"].as_str().unwrap().ends_with("-T01"));
    assert_eq!(tree(&f.root), before, "the dry run wrote something");
    assert!(host.launched().is_empty());
    assert!(c.received().is_empty());
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn a_synchronous_session_fills_the_ledger_and_leaves_mo2_unchanged() {
    let c = console();
    let f = fixture("sync");
    let probes = probes_file(&f);
    let game = game_file(&f, &c);
    let (e, host) = engine(&["explorer.exe"]);
    let before = tree(&f.instance);
    let r = e
        .execute("game.session", args(&f, &probes, &game, json!({})))
        .unwrap();
    let report = &r["data"]["report"];
    assert_eq!(report["status"], json!({"status": "pass"}), "{report}");
    assert_eq!(report["rows"].as_array().unwrap().len(), 2);
    assert_eq!(host.launched().len(), 1);
    assert_eq!(tree(&f.instance), before);
    let ledger = std::fs::read_to_string(&f.ledger).unwrap();
    let entry = report["ledger_entry"].as_str().unwrap();
    assert!(ledger.contains(&format!("### {entry}: agent session")));
    assert!(ledger.contains("Console API 2.17.0"), "{ledger}");
    assert!(ledger.contains(&sha(FIXTURE_SAVE)));
    for w in report["written"].as_array().unwrap() {
        let w = Path::new(w.as_str().unwrap());
        assert!(
            w.starts_with(&f.observe_root) || w == f.ledger,
            "{}",
            w.display()
        );
    }
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn start_status_probe_and_stop_through_the_catalog() {
    let c = console();
    let f = fixture("start");
    let probes = probes_file(&f);
    let game = game_file(&f, &c);
    let (e, _) = engine(&["explorer.exe"]);
    let r = e
        .execute(
            "game.session",
            args(&f, &probes, &game, json!({"start": true})),
        )
        .unwrap();
    let id = r["data"]["session"].as_str().unwrap().to_string();
    assert_eq!(r["data"]["started"], json!(true));
    // Polls, with another read served on the same engine between them.
    let mut polls = 0;
    loop {
        let s = e.execute("game.status", json!({"id": id})).unwrap();
        polls += 1;
        e.execute("schema", json!({"type": "WEAP"})).ok();
        if s["data"]["phase"] == json!("idle") {
            assert_eq!(s["data"]["rows"].as_array().unwrap().len(), 2);
            let shots = s["data"]["screenshots"].as_array().unwrap();
            assert_eq!(shots[0]["file"], json!("T-02-1.png"));
            assert!(Path::new(shots[0]["path"].as_str().unwrap()).is_file());
            break;
        }
        assert!(polls < 400, "{s}");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(polls >= 1);
    let p = e
        .execute(
            "game.probe",
            json!({"id": id, "row": "T-01", "screenshot": true}),
        )
        .unwrap();
    assert_eq!(p["data"]["row"]["verdict"], json!({"status": "pass"}));
    let dry = e
        .execute("game.stop", json!({"id": id, "dry_run": true}))
        .unwrap();
    assert_eq!(dry["data"]["dry_run"], json!(true));
    assert_eq!(
        e.execute("game.status", json!({"id": id})).unwrap()["data"]["phase"],
        json!("idle")
    );
    let stopped = e.execute("game.stop", json!({"id": id})).unwrap();
    let report = &stopped["data"]["report"];
    assert_eq!(report["reruns"].as_array().unwrap().len(), 1);
    assert!(report["ledger_entry"].as_str().is_some());
    let gone = e.execute("game.status", json!({"id": id})).unwrap_err();
    assert!(gone.to_string().contains("no session"), "{gone}");
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn a_started_session_is_stopped_properly_when_the_engine_goes_away() {
    let c = console();
    let f = fixture("drop");
    let probes = probes_file(&f);
    let game = game_file(&f, &c);
    let (e, host) = engine(&["explorer.exe"]);
    let r = e
        .execute(
            "game.session",
            args(&f, &probes, &game, json!({"start": true})),
        )
        .unwrap();
    assert_eq!(r["data"]["started"], json!(true));
    drop(e);
    assert!(
        host.now_running().iter().all(|p| p == "explorer.exe"),
        "{:?}",
        host.now_running()
    );
    let ledger = std::fs::read_to_string(&f.ledger).unwrap();
    assert!(ledger.contains(": agent session "), "{ledger}");
    std::fs::remove_dir_all(&f.root).unwrap();
}

#[test]
fn the_cli_spells_them_kit_game_verb() {
    let Invocation::Run { command, args } = parse_line("game status 2026-10-16-T01").unwrap()
    else {
        panic!("not a run");
    };
    assert_eq!(command.id, "game.status");
    assert_eq!(args, json!({"id": "2026-10-16-T01"}));
    let Invocation::Run { command, args } =
        parse_line("game session --mo2 C:\\MO2 --probes out\\probes.toml --start --dry-run")
            .unwrap()
    else {
        panic!("not a run");
    };
    assert_eq!(command.id, "game.session");
    assert_eq!(args["start"], json!(true));
    assert_eq!(args["dry_run"], json!(true));
    let Invocation::Run { command, args } = parse_line("game probe X T-03 --screenshot").unwrap()
    else {
        panic!("not a run");
    };
    assert_eq!(command.id, "game.probe");
    assert_eq!(args, json!({"id": "X", "row": "T-03", "screenshot": true}));
    assert!(crate::find_command("game.stop").unwrap().writes);
    assert!(!crate::find_command("game.status").unwrap().writes);
}
