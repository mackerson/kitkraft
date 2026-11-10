//! The reviewer's adversarial transcripts for R41 (`reviews/g-R41.md`, probes A1 to A8), as
//! tests (R37): a scripted console in send order with, per send, a cap on the queued answers
//! its reply carries, a drop, an empty body or a literal reply. Sends: 0 launch poll, 1 base
//! load, 2-4 base wait, 5 getter-1, 6 row load, 7-9 post-load polls, 10 on getter-2.
//! A2 (a failure line split across two replies) and A8 (the base save's line dropped) print
//! their grades only: no grader can see either (the limit in `docs/game-sessions.md`).

#![allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kitkraft_game::console::{Console, ConsoleError};
use kitkraft_game::controller::{run, Deps, Plan, Shared};
use kitkraft_game::probes::{Matcher, Probe, ProbeFile, Step, FORMAT};
use kitkraft_game::testing::{mo2_fixture, FakeHost};
use kitkraft_game::GameProfile;

const SFSE: &str = "SFSE version: 0.2.21, release idx 26, runtime 01100F40";
const NF: &str = "Save file not found.";
const SAVE: &str = "KitKraftSession-20261110-01";

#[derive(Clone)]
enum Act {
    Cap(usize),
    Drop,
    Empty,
    /// Echo plus these lines; queue own answer?
    Reply(&'static str, bool),
}

struct Scripted {
    n: usize,
    acts: Vec<(usize, Act)>,
    default: Act,
    queue: VecDeque<Vec<String>>,
    log: Arc<Mutex<Vec<String>>>,
    fail_base: bool,
}

fn own(cmd: &str, fail_base: bool) -> Vec<String> {
    let l = cmd.to_ascii_lowercase();
    if l.starts_with("getsfseversion") {
        vec![SFSE.into()]
    } else if l.starts_with("load kitkraftsession") {
        vec![NF.into()]
    } else if l.starts_with("load kitkraftbase") {
        if fail_base {
            vec![NF.into()]
        } else {
            vec![]
        }
    } else if l.starts_with("player.getpos y") {
        vec!["GetPos: Y >> -4.00".into()]
    } else if l.starts_with("player.getpos x") {
        vec!["GetPos: X >> -17.00".into()]
    } else if l.starts_with("player.getincell") {
        vec!["GetInCell >> 1.00".into()]
    } else {
        vec![]
    }
}

impl Console for Scripted {
    fn run(&mut self, command: &str, _t: Duration) -> Result<String, ConsoleError> {
        let n = self.n;
        self.n += 1;
        let act = self
            .acts
            .iter()
            .find(|(i, _)| *i == n)
            .map(|(_, a)| a.clone())
            .unwrap_or_else(|| self.default.clone());
        let mine = own(command, self.fail_base);
        let mut reply = format!("{command}\nCommand: {command}");
        match act {
            Act::Drop => {}
            Act::Empty => {
                self.queue.push_back(mine);
                reply = String::new();
            }
            Act::Reply(extra, q) => {
                if q {
                    self.queue.push_back(mine);
                }
                if !extra.is_empty() {
                    reply.push('\n');
                    reply.push_str(extra);
                }
            }
            Act::Cap(c) => {
                self.queue.push_back(mine);
                for _ in 0..c {
                    let Some(lines) = self.queue.pop_front() else {
                        break;
                    };
                    for l in lines {
                        reply.push('\n');
                        reply.push_str(&l);
                    }
                }
            }
        }
        self.log
            .lock()
            .unwrap()
            .push(format!("{n}: {}", reply.replace('\n', " | ")));
        Ok(reply)
    }
}

fn step(command: &str, expected: &str, tol: f64) -> Step {
    Step {
        command: command.into(),
        matcher: (!expected.is_empty()).then_some(Matcher::Number),
        expected: (!expected.is_empty()).then(|| expected.into()),
        tolerance: (!expected.is_empty()).then_some(tol),
        why: None,
        wait_secs: None,
    }
}
fn load() -> Step {
    step("load {save}", "", 0.0)
}
fn probe(row: &str, steps: Vec<Step>) -> Probe {
    Probe {
        row: row.into(),
        title: format!("row {row}"),
        forms: vec!["X.esm:001000".into()],
        screenshot: false,
        matcher: None,
        reason: None,
        steps,
    }
}
fn digest(b: &[u8]) -> String {
    let mut h: u64 = 1469598103934665603;
    for x in b {
        h = (h ^ u64::from(*x)).wrapping_mul(1099511628211);
    }
    format!("{h:016x}")
}

/// Runs; returns (labels, log, findings).
fn go(
    tag: &str,
    rows: Vec<Probe>,
    acts: Vec<(usize, Act)>,
    default: Act,
    fail_base: bool,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let root = std::env::temp_dir().join(format!("kitkraft-game-adv-{tag}-{}", std::process::id()));
    let f = mo2_fixture(&root).unwrap();
    let mut game = GameProfile::named("starfield").unwrap();
    game.launch.evidence = "adv".into();
    game.console.evidence = "adv".into();
    game.saves.evidence = "adv".into();
    game.timeouts.probe_secs = 8;
    game.timeouts.ready_secs = 10;
    game.timeouts.settle_secs = 0;
    game.timeouts.ready_poll_secs = 0;
    game.timeouts.load_secs = 10;
    game.timeouts.exit_secs = 1;
    game.timeouts.session_secs = 3600;
    let backups = vec![f.profile_dir.join("plugins.txt")];
    let session = tag.to_string();
    let p = Plan {
        session: session.clone(),
        date: "adv".into(),
        timeouts: game.timeouts.clone(),
        game,
        instance: f.instance.clone(),
        mod_manager: f.instance.join("ModOrganizer.exe"),
        profile: f.profile.clone(),
        profile_dir: f.profile_dir.clone(),
        observe: f.observe_root.join(&session),
        ledger: None,
        base_save: "KitKraftBase".into(),
        base_save_sha256: String::new(),
        session_save: SAVE.into(),
        probes: ProbeFile {
            format: FORMAT,
            plugin: "X.esm".into(),
            sha256: "00".into(),
            generated_by: "test".into(),
            profile: "KitKraft Dev".into(),
            runtime_id_basis: "inferred".into(),
            notes: vec![],
            probes: rows,
        },
        probes_path: f.root.join("probes.toml"),
        probes_sha256: String::new(),
        plugins: Vec::new(),
        backups,
        logs: Vec::new(),
        versions: vec![("Game build".into(), "adv".into())],
        stay: false,
        poll: Duration::ZERO,
    };
    let log = Arc::new(Mutex::new(Vec::new()));
    let console = Scripted {
        n: 0,
        acts,
        default,
        queue: VecDeque::new(),
        log: Arc::clone(&log),
        fail_base,
    };
    let host = Arc::new(FakeHost::new(
        &["explorer.exe"],
        &[
            "ModOrganizer.exe",
            "sfse_loader.exe",
            "Starfield.exe",
            "usvfs_proxy_x64.exe",
        ],
    ));
    let deps = Deps {
        host: host as Arc<dyn kitkraft_game::host::Host>,
        console: Box::new(console),
        digest,
    };
    let report = run(&p, deps, &Shared::new(&p.session));
    let _ = std::fs::remove_dir_all(&root);
    let labels: Vec<String> = report
        .rows
        .iter()
        .map(|r| format!("{} {}", r.row, r.verdict.text()))
        .collect();
    let l = log.lock().unwrap().clone();
    let mut out = format!("=== {tag}\n");
    for x in &l {
        out.push_str(&format!("  {x}\n"));
    }
    for x in &labels {
        out.push_str(&format!("  -> {x}\n"));
    }
    for x in &report.findings {
        out.push_str(&format!("  finding: {x}\n"));
    }
    eprint!("{out}");
    (labels, l, report.findings.clone())
}

fn so07() -> Vec<Probe> {
    vec![probe(
        "SO-07",
        vec![
            step("player.getincell 0021966F", "1", 0.01),
            load(),
            step("player.getincell 0021966F", "1", 0.01),
        ],
    )]
}
fn cross() -> Vec<Probe> {
    vec![
        probe("X-A", vec![step("player.getpos x", "-17", 0.5), load()]),
        probe("X-B", vec![step("player.getincell 0006BB29", "1", 0.01)]),
    ]
}
fn pass(l: &str) -> bool {
    l.split_whitespace().nth(1) == Some("pass")
}

// Sends: 0 launch poll, 1 base load, 2-4 base wait, 5 getter-1, 6 row load, 7-9 post polls,
// 10.. getter-2.

/// A1: the failure line in the third pacing poll (the one that ends the wait).
#[test]
fn a1_failure_in_third_pacing_poll() {
    let (l, _, _) = go(
        "a1",
        so07(),
        vec![
            (6, Act::Cap(0)),
            (7, Act::Reply("", false)),
            (8, Act::Reply("", false)),
        ],
        Act::Cap(usize::MAX),
        false,
    );
    assert!(!l.first().is_some_and(|x| pass(x)), "{l:?}");
}

/// A1b: cross shape, failure in the third pacing poll.
#[test]
fn a1b_cross_failure_in_third_pacing_poll() {
    let (l, _, _) = go(
        "a1b",
        cross(),
        vec![
            (6, Act::Cap(0)),
            (7, Act::Reply("", false)),
            (8, Act::Reply("", false)),
        ],
        Act::Cap(usize::MAX),
        false,
    );
    assert!(!l.first().is_some_and(|x| pass(x)), "{l:?}");
}

/// A2: the failure line split across two poll replies.
#[test]
fn a2_failure_split_across_two_replies() {
    let (l, _, _) = go(
        "a2",
        so07(),
        vec![
            (6, Act::Reply("", false)),
            (7, Act::Reply("Save file", true)),
            (8, Act::Reply("not found.", true)),
        ],
        Act::Cap(usize::MAX),
        false,
    );
    eprintln!("A2 (split line): {l:?}");
}

/// A2b: the failure text without its period, with a prefix, any case.
#[test]
fn a2b_failure_variant_text() {
    let (l, _, _) = go(
        "a2b",
        cross(),
        vec![
            (6, Act::Reply("", false)),
            (7, Act::Reply("Error: save FILE not found", true)),
        ],
        Act::Cap(usize::MAX),
        false,
    );
    assert!(!l.first().is_some_and(|x| pass(x)), "{l:?}");
}

/// A3: the load's echo dropped (empty body), its failure arrives in the first poll.
#[test]
fn a3_load_echo_dropped_failure_arrives() {
    let (l, _, _) = go(
        "a3",
        cross(),
        vec![(6, Act::Empty)],
        Act::Cap(usize::MAX),
        false,
    );
    assert!(!l.first().is_some_and(|x| pass(x)), "{l:?}");
}

fn three() -> Vec<Probe> {
    vec![
        probe("L-A", vec![step("player.getpos x", "-17", 0.5), load()]),
        probe(
            "L-B",
            vec![step("player.getincell 0006BB29", "1", 0.01), load()],
        ),
        probe("L-C", vec![step("player.getpos y", "-4", 0.5)]),
    ]
}

/// A4: two rows' loads failing in a row, both lines late.
#[test]
fn a4_two_loads_failing_in_a_row() {
    let acts = (6..=16).map(|i| (i, Act::Cap(0))).collect();
    let (l, _, _) = go("a4", three(), acts, Act::Cap(usize::MAX), false);
    assert!(!l.first().is_some_and(|x| pass(x)), "{l:?}");
    assert!(!l.get(1).is_some_and(|x| pass(x)), "{l:?}");
}

/// A4b: two loads, A's failure dropped, B's delivered late.
#[test]
fn a4b_two_loads_first_dropped() {
    let acts = vec![
        (6, Act::Drop),
        (11, Act::Cap(0)),
        (12, Act::Cap(0)),
        (13, Act::Cap(0)),
        (14, Act::Cap(0)),
    ];
    let (l, _, _) = go("a4b", three(), acts, Act::Cap(usize::MAX), false);
    assert!(!l.get(1).is_some_and(|x| pass(x)), "{l:?}");
}

/// A5: garbage in every pacing poll, the load's line then arrives in the next row's getter
/// after the getter's own value (one reply).
#[test]
fn a5_garbage_polls_then_failure_after_own_value() {
    let acts = vec![
        (6, Act::Reply("", false)),
        (7, Act::Reply("Loading...", false)),
        (8, Act::Reply("Loading...", false)),
        (9, Act::Reply("Loading...", false)),
        (
            10,
            Act::Reply("GetInCell >> 1.00\nSave file not found.", false),
        ),
    ];
    let (l, _, _) = go("a5", cross(), acts, Act::Cap(usize::MAX), false);
    assert!(!l.first().is_some_and(|x| pass(x)), "{l:?}");
}

/// A5b: garbage polls, failure in getter-2 before its value (send order).
#[test]
fn a5b_garbage_polls_then_failure_in_order() {
    let acts = vec![
        (6, Act::Reply("", false)),
        (7, Act::Reply("Loading...", false)),
        (8, Act::Reply("Loading...", false)),
        (9, Act::Reply("Loading...", false)),
        (
            10,
            Act::Reply("Save file not found.\nGetInCell >> 1.00", false),
        ),
    ];
    let (l, _, _) = go("a5b", cross(), acts, Act::Cap(usize::MAX), false);
    assert!(!l.first().is_some_and(|x| pass(x)), "{l:?}");
}

/// A6: a launch poll the game took but answered with an empty body; its line, late, settles
/// the base load in the base wait; the base save is missing and its failure arrives in row
/// A's getter. Does row B pass on a base save that never loaded?
#[test]
fn a6_stale_launch_line_settles_the_base_load() {
    let rows = vec![
        probe("B-A", vec![step("player.getpos x", "-17", 0.5)]),
        probe("B-B", vec![step("player.getincell 0006BB29", "1", 0.01)]),
    ];
    let acts = vec![
        (0, Act::Empty),
        (1, Act::Cap(1)),
        (2, Act::Cap(0)),
        (3, Act::Cap(1)),
        (4, Act::Cap(0)),
        (5, Act::Cap(0)),
        (6, Act::Cap(1)),
    ];
    let (l, _, f) = go("a6", rows, acts, Act::Cap(usize::MAX), true);
    assert!(l.iter().all(|x| !pass(x)), "{l:?} {f:?}");
}

/// A6b: no stale line; base save missing, the base wait all echoes (cap 0), the base
/// failure arrives in row A's getter: every pass inconclusive?
#[test]
fn a6b_base_failure_late_in_first_getter() {
    let rows = vec![
        probe("B-A", vec![step("player.getpos x", "-17", 0.5)]),
        probe("B-B", vec![step("player.getincell 0006BB29", "1", 0.01)]),
    ];
    let acts = vec![
        (1, Act::Cap(0)),
        (2, Act::Cap(0)),
        (3, Act::Cap(0)),
        (4, Act::Cap(0)),
        (5, Act::Cap(1)),
    ];
    let (l, _, f) = go("a6b", rows, acts, Act::Cap(usize::MAX), true);
    assert!(l.iter().all(|x| !pass(x)), "{l:?} {f:?}");
    // Review R41 L1: the late base failure is the session's finding, and each row says why.
    assert!(
        f.iter()
            .any(|x| x.contains("the base save did not load") && x.contains("(R41)")),
        "{f:?}"
    );
    assert!(l.iter().all(|x| x.contains("base save")), "{l:?}");
}

/// A7: queued at session end: cross, A's load line still queued when the session ends.
#[test]
fn a7_queued_at_end() {
    let acts = (6..=12).map(|i| (i, Act::Cap(0))).collect();
    let (l, _, _) = go("a7", cross(), acts, Act::Cap(usize::MAX), false);
    assert!(!l.first().is_some_and(|x| pass(x)), "{l:?}");
}

/// A8: the base save missing and its line dropped: rows pass on the main menu (the limit).
#[test]
fn a8_base_failure_dropped() {
    let rows = vec![probe("B-A", vec![step("player.getpos x", "-17", 0.5)])];
    let acts = vec![
        (1, Act::Drop),
        (2, Act::Drop),
        (3, Act::Drop),
        (4, Act::Drop),
    ];
    let (l, _, _) = go("a8", rows, acts, Act::Cap(usize::MAX), true);
    eprintln!("A8 (limit): {l:?}");
}
