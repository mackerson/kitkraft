//! `mod.run` through the catalog (M4.T10) on synthetic Data (no corpus needed) and a
//! fixture MO2 instance with the fake console and host of `kitkraft_game::testing`: the
//! whole chain to a refused session (`partial`, every probe `not-run`), the same chain
//! with a session that runs against the fake endpoint (ledger entry, a verdict per probe),
//! stopping at the first failing stage, `until`, the dry run, and folder reuse.

#![allow(clippy::arithmetic_side_effects)]

use std::path::PathBuf;
use std::sync::Arc;

use kitkraft_game::testing::{mo2_fixture, tree, FakeConsole, FakeHost, Fixture, FIXTURE_SAVE};
use kitkraft_game::GameProfile;
use serde_json::{json, Value};

use super::game::GameHost;
use super::modrun::{KIND, REPORT_JSON, REPORT_MD};
use super::set_tests::data;
use crate::edit::patch_tests::dir;
use crate::{parse_line, Engine, Invocation};

const RENAME: &str = "version = 1\n\n[plugin]\nlocalized = false\n\n[load_order]\nplugins = [\"Source.esm\", \"Override.esp\"]\ndata_dir = '{DATA}'\nno_cache = true\n\n[[set]]\nform = \"Source.esm:000800\"\nfields = { FULL = \"Old Soviet Marksman Rifle\" }\n";

struct Setup {
    root: PathBuf,
    spec: PathBuf,
    out: PathBuf,
    fixture: Fixture,
}

fn setup(tag: &str, spec_body: &str) -> Setup {
    let root = dir(&format!("modrun-{tag}"));
    let data = data(&root);
    let spec = root.join("rename.toml");
    std::fs::write(
        &spec,
        spec_body.replace("{DATA}", &data.display().to_string()),
    )
    .unwrap();
    let fixture = mo2_fixture(&root.join("fx")).unwrap();
    // The owner enables the dev mod once in MO2 (deploy never edits a profile).
    std::fs::write(
        fixture.profile_dir.join("modlist.txt"),
        "+KitKraft Dev - Rename\r\n+KitKraft Dev - X\r\n+Console API\r\n",
    )
    .unwrap();
    Setup {
        out: root.join("out"),
        root,
        spec,
        fixture,
    }
}

fn engine() -> Engine {
    let engine = Engine::with_default_handlers();
    let host = Arc::new(FakeHost::new(
        &[],
        &["ModOrganizer.exe", "sfse_loader.exe", "Starfield.exe"],
    ));
    let h: Arc<dyn kitkraft_game::host::Host> = host;
    engine.with_session(|s| s.insert(GameHost(h)));
    engine
}

fn args(s: &Setup, extra: Value) -> Value {
    let mut a = json!({
        "spec": s.spec.display().to_string(),
        "name": "Rename",
        "out": s.out.display().to_string(),
        "mo2": s.fixture.instance.display().to_string(),
        "base_save_sha256": crate::deploy::sha256::hex(FIXTURE_SAVE),
        "observe": s.fixture.observe_root.display().to_string(),
        "ledger": s.fixture.ledger.display().to_string(),
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

fn statuses(result: &Value) -> Vec<(String, String)> {
    result["data"]["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["stage"].as_str().unwrap().to_string(),
                s["status"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

#[test]
fn the_chain_runs_to_a_refused_session_and_reports_partial() {
    let s = setup("refused", RENAME);
    let ledger_before = std::fs::read(&s.fixture.ledger).unwrap();
    let r = engine().execute("mod.run", args(&s, json!({}))).unwrap();
    let d = &r["data"];
    assert_eq!(
        statuses(&r),
        pairs(&[
            ("build", "pass"),
            ("validate", "pass"),
            ("roundtrip", "pass"),
            ("package", "pass"),
            ("deploy", "pass"),
            ("testplan", "pass"),
            ("session", "not-run"),
        ]),
        "{d:#}"
    );
    assert_eq!(d["result"], json!("partial"));
    assert_eq!(d["passed"], json!(true));
    assert_eq!(d["ledger_entry"], Value::Null);
    // Today's refusal: the Starfield profile has no launch, console or save evidence.
    assert_eq!(d["session"]["refused"], json!(true));
    assert_eq!(
        d["session"]["rules"],
        json!(["unverified-profile"]),
        "{d:#}"
    );
    let probes = d["probes"].as_array().unwrap();
    assert!(!probes.is_empty());
    for p in probes {
        assert_eq!(p["status"], json!("not-run"), "{p}");
        assert!(p["reason"].as_str().unwrap().contains("unverified-profile"));
    }
    // The files: plugin, package, deployed mod, testplan, report, stage envelopes.
    let dir = s.out.join("Rename");
    for f in [
        "Rename.esm",
        "package/Rename/Rename.esm",
        "testplan/probes.toml",
        "testplan/checklist.md",
        REPORT_MD,
        REPORT_JSON,
        "stages/01-build.json",
        "stages/06-testplan.json",
        "stages/07-session-preflight.json",
    ] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    let deployed = s
        .fixture
        .instance
        .join("mods")
        .join("KitKraft Dev - Rename");
    assert!(deployed.join("Rename.esm").is_file());
    assert!(deployed.join("kitkraft-deploy.json").is_file());
    let md = std::fs::read_to_string(dir.join(REPORT_MD)).unwrap();
    for needle in [
        "# Mod run: Rename",
        "| Result | **partial** |",
        "| 7 | session | `game.session` | not-run (refused by preflight: unverified-profile)",
        "`unverified-profile`",
        "| T-01 |",
        "none (no session ran)",
        "M4.T1",
    ] {
        assert!(md.contains(needle), "{needle}\n{md}");
    }
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(REPORT_JSON)).unwrap()).unwrap();
    assert_eq!(json["kind"], json!(KIND));
    // A refused session writes nothing: no ledger entry, no observe folder.
    assert_eq!(std::fs::read(&s.fixture.ledger).unwrap(), ledger_before);
    assert!(!s.fixture.observe_root.exists());

    // Rerun (the fix loop): the folder is the run's own, so it is reused; the deployed
    // mod is unchanged.
    let again = engine().execute("mod.run", args(&s, json!({}))).unwrap();
    assert_eq!(again["data"]["result"], json!("partial"));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The Starfield profile with evidence filled in, the fake endpoint and short timeouts:
/// what the chain does once M4.T1 and M4.O7 are done.
fn verified_game(s: &Setup, c: &FakeConsole) -> PathBuf {
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
        l.path = s.root.join(format!("{}.log", l.name)).display().to_string();
    }
    let path = s.root.join("test-game.toml");
    std::fs::write(&path, toml::to_string(&g).unwrap()).unwrap();
    path
}

#[test]
fn with_a_verified_profile_the_session_runs_and_each_probe_has_a_verdict() {
    let s = setup("live", RENAME);
    let c = FakeConsole::start().unwrap();
    c.answer("GetSFSEVersion", "SFSE 0.2.21");
    // The rename's probe: the weapon's name finds its runtime ID.
    c.answer("help", "WEAP: (00000800) 'Old Soviet Marksman Rifle'");
    c.answer("player.getincell", "GetInCell >> 1.00");
    let game = verified_game(&s, &c);
    let r = engine()
        .execute(
            "mod.run",
            args(&s, json!({"game": game.display().to_string()})),
        )
        .unwrap();
    let d = &r["data"];
    assert_eq!(d["session"]["ran"], json!(true), "{d:#}");
    let entry = d["ledger_entry"].as_str().unwrap();
    assert!(entry.starts_with("game-"), "{entry}");
    let ledger = std::fs::read_to_string(&s.fixture.ledger).unwrap();
    assert!(ledger.contains(entry));
    let rows: Vec<(String, String)> = d["probes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["row"].as_str().unwrap().to_string(),
                p["status"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    // T-01 finds the renamed weapon's runtime ID, T-02 is the save/reload, T-03 is manual
    // for a plugin that places nothing.
    assert_eq!(
        rows,
        pairs(&[("T-01", "pass"), ("T-02", "pass"), ("T-03", "not-run")]),
        "{d:#}"
    );
    // The verdict is read off the rows: a manual row keeps it partial.
    assert_eq!(d["result"], json!("partial"));
    let md = std::fs::read_to_string(s.out.join("Rename").join(REPORT_MD)).unwrap();
    assert!(
        md.contains(&format!("| Ledger entry | `{entry}` |")),
        "{md}"
    );
    assert!(
        md.contains("| T-01 | help \"Old Soviet Marksman Rifle\" 4"),
        "{md}"
    );

    // The game shows another ID: the probe fails, so the run fails (exit 1) at `session`.
    c.answer(
        "help \"Old Soviet",
        "WEAP: (00000801) 'Old Soviet Marksman Rifle'",
    );
    let r = engine()
        .execute(
            "mod.run",
            args(&s, json!({"game": game.display().to_string()})),
        )
        .unwrap();
    let d = &r["data"];
    assert_eq!(d["probes"][0]["status"], json!("fail"), "{d:#}");
    assert_eq!(d["result"], json!("fail"));
    assert_eq!(d["passed"], json!(false));
    assert_eq!(d["failed_stage"], json!("session"));
    assert_ne!(d["ledger_entry"].as_str().unwrap(), entry);
    let _ = std::fs::remove_dir_all(&s.root);
}

#[test]
fn a_failing_stage_stops_the_run_with_its_evidence() {
    // No mo2: the deploy stage fails, later stages are not-run, the report is written.
    let s = setup("nomo2", RENAME);
    let mut a = args(&s, json!({}));
    a.as_object_mut().unwrap().remove("mo2");
    let r = engine().execute("mod.run", a).unwrap();
    let d = &r["data"];
    assert_eq!(d["result"], json!("fail"));
    assert_eq!(d["passed"], json!(false));
    assert_eq!(d["failed_stage"], json!("deploy"));
    let st = statuses(&r);
    assert_eq!(st[4], ("deploy".to_string(), "fail".to_string()));
    assert_eq!(st[5].1, "not-run");
    assert_eq!(st[6].1, "not-run");
    assert!(d["stages"][5]["reason"]
        .as_str()
        .unwrap()
        .contains("stopped at the failing `deploy` stage"));
    assert!(d["next_steps"][0]
        .as_str()
        .unwrap()
        .contains("docs/playbooks/mod-from-brief.md"));
    assert!(s.out.join("Rename").join(REPORT_MD).is_file());
    let _ = std::fs::remove_dir_all(&s.root);

    // A spec naming a form that is not there: build fails with its error as evidence.
    let s = setup("badspec", &RENAME.replace("000800", "000999"));
    let r = engine().execute("mod.run", args(&s, json!({}))).unwrap();
    let d = &r["data"];
    assert_eq!(d["failed_stage"], json!("build"), "{d:#}");
    assert_eq!(d["stages"][0]["error"]["kind"], json!("failed"));
    assert!(d["stages"][1]["reason"]
        .as_str()
        .unwrap()
        .starts_with("not-run"));
    let _ = std::fs::remove_dir_all(&s.root);
}

#[test]
fn until_stops_after_the_named_stage_and_dry_run_writes_nothing() {
    let s = setup("until", RENAME);
    let mut a = args(&s, json!({"until": "roundtrip"}));
    a.as_object_mut().unwrap().remove("mo2");
    let r = engine().execute("mod.run", a).unwrap();
    let d = &r["data"];
    assert_eq!(d["result"], json!("partial"), "{d:#}");
    let st = statuses(&r);
    assert!(st[..3].iter().all(|(_, v)| v == "pass"), "{st:?}");
    assert!(st[3..].iter().all(|(_, v)| v == "not-run"), "{st:?}");
    assert!(d["stages"][3]["reason"]
        .as_str()
        .unwrap()
        .contains("--until roundtrip"));
    let _ = std::fs::remove_dir_all(&s.root);

    let s = setup("dry", RENAME);
    let before = tree(&s.root);
    let r = engine()
        .execute("mod.run", args(&s, json!({"dry_run": true})))
        .unwrap();
    let d = &r["data"];
    assert_eq!(d["dry_run"], json!(true));
    assert_eq!(d["result"], json!("dry-run"), "{d:#}");
    assert_eq!(d["stages"][0]["status"], json!("pass"));
    assert_eq!(d["stages"][4]["args"]["dev"], json!(true));
    assert_eq!(tree(&s.root), before, "a dry run writes nothing");
    let _ = std::fs::remove_dir_all(&s.root);
}

#[test]
fn a_folder_another_tool_wrote_is_never_reused() {
    let s = setup("foreign", RENAME);
    let mine = s.out.join("Rename");
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(mine.join("notes.txt"), b"mine").unwrap();
    let err = engine()
        .execute("mod.run", args(&s, json!({})))
        .unwrap_err()
        .to_string();
    assert!(err.contains("not a mod run folder"), "{err}");
    let err = engine()
        .execute("mod.run", args(&s, json!({"name": "../up"})))
        .unwrap_err()
        .to_string();
    assert!(err.contains("ASCII letters"), "{err}");
    let err = engine()
        .execute("mod.run", args(&s, json!({"plugin": "a/b.esm"})))
        .unwrap_err()
        .to_string();
    assert!(err.contains("not a path"), "{err}");
    let _ = std::fs::remove_dir_all(&s.root);
}

#[test]
fn kit_mod_run_parses_to_the_catalog_command() {
    let Invocation::Run { command, args } = parse_line(
        "mod run docs/smoke-rename.toml --name OldSovietMarksmanRifle --until package --dry-run",
    )
    .unwrap() else {
        panic!("not a run");
    };
    assert_eq!(command.id, "mod.run");
    assert_eq!(args["spec"], json!("docs/smoke-rename.toml"));
    assert_eq!(args["until"], json!("package"));
    assert_eq!(args["dry_run"], json!(true));
}
