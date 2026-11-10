//! Ruling R41, the pacing poll: the enumeration that gates it (G's design note of
//! 2026-11-10, "Readiness debt: T01 reading and fix design", section 3).
//!
//! A console in send order is modelled as a fifo: each send's own answer (its lines, if it
//! has any) joins a queue, and its reply is its echo followed by at most `cap` queued answers,
//! oldest first. Each of six sends of a session (by role, below) takes one of five cases: a
//! cap of 0, 1, 2 or all, or **drop**: the reply is the echo alone and the send's own answer
//! is discarded, as the game did after the base `load` in 2026-11-10-T01. A role's case
//! applies to every send of that role (a getter's retries, every poll of a wait); the launch
//! polls, outside R41, answer in full. 5^6 = 15,625 runs per shape, through the real
//! controller with no game and no waits.
//!
//! Roles: the base `load`, the base wait's polls, the getter before the row's `load`, the
//! row's `load {save}` (the session save does not exist: `Save file not found.`), the polls
//! after it, the getter after it. Shapes:
//!
//! - `so07`: one row, `player.getincell 0021966F`, `load {save}`, the same getter again
//!   (SO-07: the load on time; two getters whose answers name no argument).
//! - `so08`: one row, `player.getpos y`, `load {save}`, `player.getincell 0006BB29` (SO-08:
//!   the load behind a getter that may still be waiting).
//! - `cross`: row A `player.getpos x`, `load {save}`; row B `player.getincell 0006BB29` (the
//!   load's failure may land in the next row's reply: rv7, fx1a to fx1h).
//!
//! Checks: (i) a `Save file not found.` delivered in any reply never leaves the load's row
//! `pass`; (ii) every run reaches the rows (T01's shape, the base wait's answers dropped,
//! included); (iii) the runs where the load's line never reached the controller (dropped,
//! or still queued when the session ended) are counted, with how many of them leave the
//! load's row `pass`: no grader can see a failure it never receives (the limit
//! `docs/game-sessions.md` states).
//!
//! The gate runs every 16th case of each shape (977 runs a shape, 2,931 in all); the full
//! 46,875 runs are `cargo test -p kitkraft-game --test pacing -- --ignored`.

#![allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kitkraft_game::console::{Console, ConsoleError};
use kitkraft_game::controller::{run, Deps, Plan, Shared};
use kitkraft_game::probes::{Matcher, Probe, ProbeFile, Step, FORMAT};
use kitkraft_game::testing::{mo2_fixture, FakeHost, Fixture};
use kitkraft_game::GameProfile;

const SFSE: &str = "SFSE version: 0.2.21, release idx 26, runtime 01100F40";
const NOT_FOUND: &str = "Save file not found.";
const SAVE: &str = "KitKraftSession-20261110-01";

/// A case: a cap on the queued answers a reply carries, or `None` for drop.
type Case = Option<usize>;
const CASES: [Case; 5] = [Some(0), Some(1), Some(2), Some(usize::MAX), None];

fn case_name(c: Case) -> &'static str {
    match c {
        Some(0) => "0",
        Some(1) => "1",
        Some(2) => "2",
        Some(_) => "all",
        None => "drop",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Launch,
    BaseWait,
    Row,
    PostWait,
    After,
}

/// Roles, in the order of a run's six cases.
const BASE_LOAD: usize = 0;
const BASE_WAIT: usize = 1;
const GETTER_BEFORE: usize = 2;
const ROW_LOAD: usize = 3;
const POST_WAIT: usize = 4;
const GETTER_AFTER: usize = 5;

#[derive(Debug, Default, Clone)]
struct Seen {
    /// The role of the send whose reply carried the load's `Save file not found.`.
    delivered: Option<usize>,
    sends: usize,
}

struct Fifo {
    cases: [Case; 6],
    phase: Phase,
    /// Queued answers: `(the row load's?, lines)`.
    queue: VecDeque<(bool, Vec<String>)>,
    seen: Arc<Mutex<Seen>>,
}

fn own(cmd: &str) -> Vec<String> {
    let l = cmd.to_ascii_lowercase();
    if l.starts_with("getsfseversion") {
        vec![SFSE.into()]
    } else if l.starts_with("load kitkraftsession") {
        vec![NOT_FOUND.into()]
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

impl Console for Fifo {
    fn run(&mut self, command: &str, _timeout: Duration) -> Result<String, ConsoleError> {
        let l = command.to_ascii_lowercase();
        let role = if l.starts_with("getsfseversion") {
            match self.phase {
                Phase::BaseWait => Some(BASE_WAIT),
                Phase::PostWait => Some(POST_WAIT),
                _ => None,
            }
        } else if l.starts_with("load kitkraftbase") {
            self.phase = Phase::BaseWait;
            Some(BASE_LOAD)
        } else if l.starts_with("load kitkraftsession") {
            self.phase = Phase::PostWait;
            Some(ROW_LOAD)
        } else {
            match self.phase {
                Phase::BaseWait | Phase::Row => {
                    self.phase = Phase::Row;
                    Some(GETTER_BEFORE)
                }
                _ => {
                    self.phase = Phase::After;
                    Some(GETTER_AFTER)
                }
            }
        };
        let case = role.map_or(Some(usize::MAX), |r| self.cases.get(r).copied().flatten());
        let mut seen = self.seen.lock().unwrap();
        seen.sends += 1;
        let mut reply = format!("{command}\nCommand: {command}");
        let Some(cap) = case else {
            // Drop: the echo only; the answer is lost.
            return Ok(reply);
        };
        self.queue.push_back((role == Some(ROW_LOAD), own(command)));
        for _ in 0..cap {
            let Some((is_load, lines)) = self.queue.pop_front() else {
                break;
            };
            if is_load {
                seen.delivered.get_or_insert(role.unwrap_or(usize::MAX));
            }
            for line in lines {
                reply.push('\n');
                reply.push_str(&line);
            }
        }
        Ok(reply)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    So07,
    So08,
    Cross,
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

fn rows(shape: Shape) -> Vec<Probe> {
    let load = step("load {save}", "", 0.0);
    match shape {
        Shape::So07 => vec![probe(
            "SO-07",
            vec![
                step("player.getincell 0021966F", "1", 0.01),
                load,
                step("player.getincell 0021966F", "1", 0.01),
            ],
        )],
        Shape::So08 => vec![probe(
            "SO-08",
            vec![
                step("player.getpos y", "-4", 0.5),
                load,
                step("player.getincell 0006BB29", "1", 0.01),
            ],
        )],
        Shape::Cross => vec![
            probe("X-A", vec![step("player.getpos x", "-17", 0.5), load]),
            probe("X-B", vec![step("player.getincell 0006BB29", "1", 0.01)]),
        ],
    }
}

fn digest(b: &[u8]) -> String {
    let mut h: u64 = 1469598103934665603;
    for x in b {
        h = (h ^ u64::from(*x)).wrapping_mul(1099511628211);
    }
    format!("{h:016x}")
}

fn plan(f: &Fixture, shape: Shape, session: &str) -> Plan {
    let mut game = GameProfile::named("starfield").unwrap();
    game.launch.evidence = "enumeration".into();
    game.console.evidence = "enumeration".into();
    game.saves.evidence = "enumeration".into();
    game.timeouts.probe_secs = 8;
    game.timeouts.ready_secs = 10;
    game.timeouts.settle_secs = 0;
    game.timeouts.ready_poll_secs = 0;
    game.timeouts.load_secs = 10;
    game.timeouts.exit_secs = 1;
    game.timeouts.session_secs = 3600;
    // One backup, the list file MO2 rewrites: the runs are about the console, not the files.
    let backups = vec![f.profile_dir.join("plugins.txt")];
    Plan {
        session: session.into(),
        date: "enumeration".into(),
        timeouts: game.timeouts.clone(),
        game,
        instance: f.instance.clone(),
        mod_manager: f.instance.join("ModOrganizer.exe"),
        profile: f.profile.clone(),
        profile_dir: f.profile_dir.clone(),
        observe: f.observe_root.join(session),
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
            probes: rows(shape),
        },
        probes_path: f.root.join("probes.toml"),
        probes_sha256: String::new(),
        plugins: Vec::new(),
        backups,
        logs: Vec::new(),
        versions: vec![("Game build".into(), "enumeration".into())],
        stay: false,
        // No waits: every pause the controller takes scales with this.
        poll: Duration::ZERO,
    }
}

fn cases_of(index: usize) -> [Case; 6] {
    let mut c = [Some(0); 6];
    let mut n = index;
    for slot in &mut c {
        *slot = CASES.get(n % 5).copied().flatten();
        n /= 5;
    }
    c
}

fn label(cases: &[Case; 6]) -> String {
    let names = [
        "base-load",
        "base-wait",
        "getter-1",
        "load",
        "post-wait",
        "getter-2",
    ];
    names
        .iter()
        .zip(cases)
        .map(|(n, c)| format!("{n}={}", case_name(*c)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One run: `(the load's row label, every row's label, what the console saw)`.
fn one(f: &Fixture, shape: Shape, index: usize) -> (String, Vec<String>, Seen) {
    let cases = cases_of(index);
    let session = format!("run-{index}");
    let p = plan(f, shape, &session);
    let seen = Arc::new(Mutex::new(Seen::default()));
    let console = Fifo {
        cases,
        phase: Phase::Launch,
        queue: VecDeque::new(),
        seen: Arc::clone(&seen),
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
    let _ = std::fs::remove_dir_all(&p.observe);
    let labels: Vec<String> = report
        .rows
        .iter()
        .map(|r| format!("{} {}", r.row, r.verdict.text()))
        .collect();
    let load_row = report
        .rows
        .first()
        .map_or("none", |r| r.verdict.label())
        .to_string();
    let s = seen.lock().unwrap().clone();
    (load_row, labels, s)
}

/// `(case index, the load row's label, every row's label, what the console saw)`.
type Outcome = (usize, String, Vec<String>, Seen);

#[derive(Debug, Default)]
struct Tally {
    runs: usize,
    /// (i): runs where the load's line was delivered and its row passed.
    false_passes: Vec<String>,
    /// (ii): runs where a row was not run.
    not_reached: Vec<String>,
    /// (iii): runs where the load's line never reached the controller.
    undelivered: usize,
    /// ... and of those, the runs whose load row passed.
    undelivered_passes: Vec<String>,
    /// Delivered runs by the role of the reply that carried the line.
    delivered_in: [usize; 6],
    /// Labels of the load's row over delivered runs.
    delivered_labels: std::collections::BTreeMap<String, usize>,
}

fn enumerate(shape: Shape, every: usize) -> Tally {
    let all: Vec<usize> = (0..15_625).step_by(every).collect();
    let threads = std::thread::available_parallelism()
        .map_or(4, std::num::NonZeroUsize::get)
        .clamp(1, 8);
    let chunks: Vec<Vec<usize>> = (0..threads)
        .map(|t| all.iter().copied().skip(t).step_by(threads).collect())
        .collect();
    let tag = format!("{shape:?}-{every}").to_ascii_lowercase();
    let results: Vec<Vec<Outcome>> = std::thread::scope(|sc| {
        let handles: Vec<_> = chunks
            .iter()
            .enumerate()
            .map(|(t, chunk)| {
                let tag = tag.clone();
                sc.spawn(move || {
                    let root = std::env::temp_dir().join(format!(
                        "kitkraft-game-pacing-{tag}-{t}-{}",
                        std::process::id()
                    ));
                    let f = mo2_fixture(&root).unwrap();
                    let out: Vec<_> = chunk
                        .iter()
                        .map(|i| {
                            let (l, labels, s) = one(&f, shape, *i);
                            (*i, l, labels, s)
                        })
                        .collect();
                    let _ = std::fs::remove_dir_all(&root);
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut t = Tally::default();
    for (i, load_row, labels, s) in results.into_iter().flatten() {
        t.runs += 1;
        let what = format!("{} -> {}", label(&cases_of(i)), labels.join(" | "));
        if labels.iter().any(|l| l.contains(" not-run")) {
            t.not_reached.push(what.clone());
        }
        match s.delivered {
            Some(role) => {
                if let Some(n) = t.delivered_in.get_mut(role) {
                    *n += 1;
                }
                *t.delivered_labels.entry(load_row.clone()).or_default() += 1;
                if load_row == "pass" {
                    t.false_passes.push(what);
                }
            }
            None => {
                t.undelivered += 1;
                if load_row == "pass" {
                    t.undelivered_passes.push(what);
                }
            }
        }
    }
    if std::env::var_os("KITKRAFT_PACING_LIST").is_some() {
        for w in &t.undelivered_passes {
            eprintln!("{shape:?} undelivered, load row pass: {w}");
        }
    }
    eprintln!(
        "{shape:?}: {} runs; delivered {} (in base-load {}, base-wait {}, getter-1 {}, load {}, post-wait {}, getter-2 {}; load row {:?}); never delivered {} (load row pass in {}); not reached {}; delivered-failure passes {}",
        t.runs,
        t.runs - t.undelivered,
        t.delivered_in[0],
        t.delivered_in[1],
        t.delivered_in[2],
        t.delivered_in[3],
        t.delivered_in[4],
        t.delivered_in[5],
        t.delivered_labels,
        t.undelivered,
        t.undelivered_passes.len(),
        t.not_reached.len(),
        t.false_passes.len()
    );
    t
}

fn check(shape: Shape, every: usize) -> Tally {
    let t = enumerate(shape, every);
    assert_eq!(t.runs, 15_625_usize.div_ceil(every));
    // (i) A delivered load failure never leaves its row a pass.
    assert!(
        t.false_passes.is_empty(),
        "{shape:?}: {} delivered-failure passes, first {:?}",
        t.false_passes.len(),
        t.false_passes.first()
    );
    // (ii) Every run reaches its rows, T01's shape (base wait dropped) among them.
    assert!(
        t.not_reached.is_empty(),
        "{shape:?}: {} runs never reached the rows, first {:?}",
        t.not_reached.len(),
        t.not_reached.first()
    );
    // (iii) The undelivered runs are counted: every one has the load's own send dropped or
    // its line still queued at the end, and only those may leave the load's row a pass.
    assert!(t.undelivered > 0);
    t
}

/// T01's shape exactly (the base `load` answered with its echo, every base-wait answer
/// dropped, everything else in full) reaches the rows and fails the load's row on its line.
#[test]
fn t01s_shape_reaches_the_rows_and_fails_the_load() {
    let f = mo2_fixture(
        &std::env::temp_dir().join(format!("kitkraft-game-pacing-t01-{}", std::process::id())),
    )
    .unwrap();
    // Index of [0, drop, all, all, all, all] in `cases_of`'s base 5.
    let index = 4 * 5 + 3 * 25 + 3 * 125 + 3 * 625 + 3 * 3125;
    assert_eq!(
        label(&cases_of(index)),
        "base-load=0 base-wait=drop getter-1=all load=all post-wait=all getter-2=all"
    );
    for shape in [Shape::So07, Shape::So08, Shape::Cross] {
        let (load_row, labels, s) = one(&f, shape, index);
        assert_eq!(load_row, "fail", "{shape:?} {labels:?}");
        assert_eq!(s.delivered, Some(ROW_LOAD), "{shape:?}");
    }
    let _ = std::fs::remove_dir_all(&f.root);
}

#[test]
fn pacing_enumeration_so07_sampled() {
    check(Shape::So07, 16);
}

#[test]
fn pacing_enumeration_so08_sampled() {
    check(Shape::So08, 16);
}

#[test]
fn pacing_enumeration_cross_sampled() {
    check(Shape::Cross, 16);
}

#[test]
#[ignore = "the full 15,625 runs a shape; the gate runs every 16th"]
fn pacing_enumeration_full() {
    let mut total = 0;
    for shape in [Shape::So07, Shape::So08, Shape::Cross] {
        let t = check(shape, 1);
        total += t.runs;
    }
    assert_eq!(total, 46_875);
}
