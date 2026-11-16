//! The bench verification mod's probes (`docs/bench/verify-mod.md`): the committed
//! `docs/bench/verify-mod/probes.toml` parses, and the controller grades it as the bench
//! session needs when a console model answers every command as the game is expected to.
//! The model's reply shapes are the observed ones (`GetIsID >> 1.00`, `GetInCell >> 1.00`,
//! a `help` line `TYPE: EDID (ID) 'NAME'`), and since the bench of 2026-11-16
//! (`game-20261116-04`) also those of `getglobalvalue`, `getinfaction`, `hasperk`
//! (`Perk Rank >> 1`), `getav` (`GetActorValue: <EDID> >> 7.00`), `getstage`,
//! `getitemcount` and `getequipped`. These tests show the grading and the rows'
//! independence; whether the plugin works is the session's to show.

#![allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use kitkraft_game::controller::{run, Deps, Plan, Shared};
use kitkraft_game::probes::{Matcher, ProbeFile};
use kitkraft_game::testing::{mo2_fixture, FakeConsole, FakeHost};
use kitkraft_game::{console::HttpConsole, GameProfile};

fn probes_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/bench/verify-mod/probes.toml")
}

fn probes() -> ProbeFile {
    let text = std::fs::read_to_string(probes_path()).unwrap();
    ProbeFile::parse(&text).unwrap()
}

fn digest(b: &[u8]) -> String {
    let mut h: u64 = 1469598103934665603;
    for x in b {
        h = (h ^ u64::from(*x)).wrapping_mul(1099511628211);
    }
    format!("{h:016x}")
}

/// The reply the game is expected to give when the plugin works.
fn model(cmd: &str, help: &HashMap<String, String>) -> Option<String> {
    let echo = format!("{cmd}\nCommand: {cmd}");
    let lower = cmd.to_ascii_lowercase();
    let verb = lower.split_whitespace().next().unwrap_or_default();
    let verb = verb.rsplit('.').next().unwrap_or(verb);
    let line = match verb {
        "getsfseversion" => "SFSE version: 0.2.21, release idx 26, runtime 01100F40".to_string(),
        "getincell" => "GetInCell >> 1.00".into(),
        "getisid" => "GetIsID >> 1.00".into(),
        "getitemcount" => "GetItemCount >> 1.00".into(),
        "getequipped" => "GetEquipped >> 1.00".into(),
        "getinfaction" => "GetInFaction >> 1.00".into(),
        "getglobalvalue" => "GetGlobalValue >> 4242.00".into(),
        // The lines the game printed in game-20261116-04 (sends 47 to 53).
        "getav" => format!(
            "GetActorValue: {} >> 7.00",
            cmd.split_whitespace().nth(1).unwrap_or_default()
        ),
        "getstage" => "GetStage >> 10.00".into(),
        "hasperk" => "Perk Rank >> 1".into(),
        "help" => help.get(cmd).cloned()?,
        _ => return Some(echo),
    };
    Some(format!("{echo}\n{line}"))
}

/// Runs the committed probes against the model; `log` is the Papyrus log the reporter
/// writes. Returns `(row, verdict)` in order.
fn session(tag: &str, log: &str) -> Vec<(String, String)> {
    let mut pf = probes();
    for p in &mut pf.probes {
        for s in &mut p.steps {
            s.wait_secs = None;
        }
    }
    // `help "<term>" 4 <TYPE>` answers the form's line, naming the runtime ID the row
    // expects, as in game-20261116-04 (`TMLM:  (0A001008) 'KKV18 Terminal Menu'`).
    let mut help = HashMap::new();
    for p in &pf.probes {
        for s in &p.steps {
            if let Some(rest) = s.command.strip_prefix("help ") {
                let (term, ty) = match rest.strip_prefix('"') {
                    Some(q) => {
                        let (t, after) = q.split_once('"').unwrap();
                        (t, after.split_whitespace().nth(1).unwrap_or("XXXX"))
                    }
                    None => {
                        let w: Vec<&str> = rest.split_whitespace().collect();
                        (
                            w.first().copied().unwrap_or_default(),
                            w.get(2).copied().unwrap_or("XXXX"),
                        )
                    }
                };
                let id = s.expected.clone().unwrap();
                help.insert(s.command.clone(), format!("{ty}:  ({id}) '{term}'"));
            }
        }
    }
    let c = FakeConsole::start().unwrap();
    c.respond_with(move |cmd| model(cmd, &help));
    let root = std::env::temp_dir().join(format!("kitkraft-verify-{tag}-{}", std::process::id()));
    let f = mo2_fixture(&root).unwrap();
    // The controller reads the log written during the session: the reporter's lines appear
    // once the game answers its first command (as replay() in tests/session.rs does).
    let papyrus = f.root.join("papyrus.log");
    let (pp, text, written) = (
        papyrus.clone(),
        log.to_string(),
        std::sync::Mutex::new(false),
    );
    c.on_command(move |_| {
        let mut w = written.lock().unwrap();
        if !*w {
            std::fs::write(&pp, &text).unwrap();
            *w = true;
        }
    });
    let mut game = GameProfile::named("starfield").unwrap();
    game.launch.evidence = "fixture".into();
    game.console.evidence = "fixture".into();
    game.saves.evidence = "fixture".into();
    game.console.url = c.url();
    game.timeouts.ready_secs = 3;
    game.timeouts.settle_secs = 0;
    game.timeouts.ready_poll_secs = 0;
    game.timeouts.load_secs = 3;
    game.timeouts.exit_secs = 1;
    let plan = Plan {
        session: "2026-11-16-T01".into(),
        date: "2026-11-16".into(),
        timeouts: game.timeouts.clone(),
        game,
        instance: f.instance.clone(),
        mod_manager: f.instance.join("ModOrganizer.exe"),
        profile: f.profile.clone(),
        profile_dir: f.profile_dir.clone(),
        observe: f.observe_root.join("2026-11-16-T01"),
        ledger: Some(f.ledger.clone()),
        base_save: "KitKraftBase".into(),
        base_save_sha256: digest(kitkraft_game::testing::FIXTURE_SAVE),
        session_save: "KitKraftSession-20261116-01".into(),
        probes: pf,
        probes_path: probes_path(),
        probes_sha256: "11".repeat(32),
        plugins: vec![("KitKraftVerify.esm".into(), "22".repeat(32))],
        backups: vec![f.profile_dir.join("plugins.txt"), f.base_save.clone()],
        logs: vec![
            ("papyrus".into(), papyrus.clone()),
            ("console-history".into(), f.root.join("absent.log")),
        ],
        versions: vec![("Game build".into(), "unknown (fixture)".into())],
        stay: false,
        poll: Duration::from_millis(20),
    };
    let host = Arc::new(FakeHost::new(
        &["explorer.exe"],
        &["ModOrganizer.exe", "sfse_loader.exe", "Starfield.exe"],
    ));
    let deps = Deps {
        host: host as Arc<dyn kitkraft_game::host::Host>,
        console: Box::new(HttpConsole::new(&c.url()).unwrap()),
        digest,
    };
    let report = run(&plan, deps, &Shared::new(&plan.session));
    let _ = std::fs::remove_dir_all(&root);
    report
        .rows
        .iter()
        .map(|r| (r.row.clone(), r.verdict.text()))
        .collect()
}

/// Every line the reporter writes when every check holds: the `papyrus-log` steps'
/// expectations.
fn reporter_log() -> String {
    let mut log = String::from("[00:00:00] Papyrus log opened (PC-64)\n");
    for p in probes().probes {
        for s in p.steps {
            if s.matcher == Some(Matcher::PapyrusLog) {
                log.push_str(&format!("[00:00:01] {}\n", s.expected.unwrap()));
            }
        }
    }
    log
}

#[test]
fn the_verify_mod_probes_parse_and_every_row_checks_something() {
    let pf = probes();
    assert_eq!(pf.plugin, "KitKraftVerify.esm");
    assert_eq!(pf.probes.len(), 106);
    for p in &pf.probes {
        assert!(p.matcher.is_none(), "{}: no manual rows", p.row);
        assert!(p.steps.iter().any(|s| s.matcher.is_some()), "{}", p.row);
    }
    // The reporter's lines in the probes are the ones VerifyReport.psc writes: every
    // `KKV-Pnn` label is in the script or in the build spec's Labels.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/bench/verify-mod");
    let psc = std::fs::read_to_string(dir.join("scripts/KitKraft/VerifyReport.psc")).unwrap();
    let spec = std::fs::read_to_string(dir.join("verify-mod.toml")).unwrap();
    for p in &pf.probes {
        for s in &p.steps {
            if s.matcher == Some(Matcher::PapyrusLog) {
                let e = s.expected.clone().unwrap();
                let label = e.get(4..7).unwrap();
                assert!(
                    psc.contains(&format!("\"{label}\"")) || spec.contains(&format!("\"{label} ")),
                    "{}: `{e}` names no line of the reporter",
                    p.row
                );
            }
        }
    }
}

#[test]
fn every_row_passes_when_the_game_answers_as_the_plugin_predicts() {
    let rows = session("all", &reporter_log());
    let bad: Vec<_> = rows.iter().filter(|(_, v)| v != "pass").collect();
    assert!(bad.is_empty(), "{bad:#?}");
    assert_eq!(rows.len(), 106);
}

#[test]
fn a_failed_check_fails_its_own_row_only() {
    // The global reads 0 and the pack-in is not its reference's base (game-20261103-08
    // T-20's answer): two rows fail, every other row still passes.
    let log = reporter_log()
        .replace("KKV-P01 GLOB value=4242", "KKV-P01 GLOB value=0")
        .replace("KKV-P29 PKIN base-is=True", "KKV-P29 PKIN base-is=False");
    let rows = session("one", &log);
    let failed: Vec<&str> = rows
        .iter()
        .filter(|(_, v)| v != "pass")
        .map(|(r, _)| r.as_str())
        .collect();
    assert_eq!(failed, ["V-P01", "V-P29"]);
}

/// The short re-run of 2026-11-16 (`docs/bench/verify-mod/rerun.toml`, verify-mod.md
/// "Re-run of the corrected rows") is rows of the full file, unchanged.
#[test]
fn the_rerun_file_is_rows_of_the_full_file() {
    let path = probes_path().with_file_name("rerun.toml");
    let rerun = ProbeFile::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let full = probes();
    assert_eq!(rerun.plugin, full.plugin);
    assert_eq!(rerun.probes.len(), 25);
    for p in &rerun.probes {
        let f = full.probes.iter().find(|f| f.row == p.row).unwrap();
        let steps = |x: &kitkraft_game::probes::Probe| {
            x.steps
                .iter()
                .map(|s| (s.command.clone(), s.expected.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(steps(p), steps(f), "{}", p.row);
    }
}
