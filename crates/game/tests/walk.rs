//! The owner walk recorder (`kitkraft_game::walk`, `kit walk`) against the fake console:
//! the committed walk (`docs/bench/owner-walk/stations.toml`) passes every station when a
//! console model answers as the game is expected to and the owner answers `p`; a lost
//! answer is read again after readiness and is `inconclusive` when it stays lost; an
//! owner's `f` fails a station whatever the console says; Enter alone on a station the
//! owner's eyes decide is `inconclusive`; `s` skips and `q` stops; nothing is written
//! outside the walk's folder, and no save, load or cell move is ever sent.
//!
//! The model's reply shapes are the observed ones: the echo, `Command: <command>`, then a
//! getter's `GetItemCount >> 1.00` (game-20261101-T1, game-20261116-04), and the ready
//! answer `SFSE version: 0.2.21, release idx 26, runtime 01100F40` (game-20261101-T1 row b).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::type_complexity
)]

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kitkraft_game::console::HttpConsole;
use kitkraft_game::testing::FakeConsole;
use kitkraft_game::walk::{run, WalkFile, WalkReport, WalkSetup};

const SFSE: &str = "SFSE version: 0.2.21, release idx 26, runtime 01100F40";

/// The game as the console sees it: getter values by command (lowercased), and getters
/// that answer only their echo for their next `n` sends.
#[derive(Default)]
struct Game {
    values: HashMap<String, f64>,
    mute: HashMap<String, u32>,
    /// Muted answers arrive late, in the next reply, instead of being dropped.
    late: bool,
    queued: Vec<String>,
}

type Shared = Arc<Mutex<Game>>;

/// The function name a getter's answer line starts with (`player.getitemcount` ->
/// `GetItemCount`), as the game prints it.
fn answer_name(verb: &str) -> String {
    let known = [
        "GetItemCount",
        "GetGlobalValue",
        "GetInCell",
        "GetStolenItemValueNoCrime",
        "GetStolenItemValue",
        "GetCrimeGold",
        "GetIsID",
    ];
    known
        .iter()
        .find(|k| k.eq_ignore_ascii_case(verb))
        .map_or_else(|| verb.to_string(), |k| (*k).to_string())
}

fn respond(game: &Shared, command: &str) -> Option<String> {
    let mut g = game.lock().unwrap();
    let late: String = g.queued.drain(..).collect();
    let echo = format!(
        "{command}
Command: {command}
{late}"
    );
    let lower = command.trim().to_ascii_lowercase();
    if lower == "getsfseversion" {
        return Some(format!(
            "{echo}{SFSE}
"
        ));
    }
    let muted = match g.mute.get_mut(&lower) {
        Some(n) if *n > 0 => {
            *n -= 1;
            true
        }
        _ => false,
    };
    let mut words = lower.split_whitespace();
    let first = words.next().unwrap_or_default().to_string();
    let verb = first.rsplit('.').next().unwrap_or_default().to_string();
    if verb == "additem" {
        let item = words.next().unwrap_or_default().to_string();
        let n: f64 = words.next().and_then(|w| w.parse().ok()).unwrap_or(1.0);
        let key = format!("player.getitemcount {item}");
        *g.values.entry(key).or_insert(0.0) += n;
        // The game's own condition checks print after some commands (game-20261117-07).
        return Some(format!(
            "{echo}GetGlobalValue >> 2.00
"
        ));
    }
    if verb.starts_with("get") {
        let v = g.values.get(&lower).copied().unwrap_or(0.0);
        let line = format!(
            "{} >> {v:.2}
",
            answer_name(&verb)
        );
        if muted {
            // Dropped, or (with `late`) delivered in the next reply.
            if g.late {
                g.queued.push(line);
            }
            return Some(echo);
        }
        return Some(format!("{echo}{line}"));
    }
    Some(echo)
}

/// The owner: typed lines in order, each one after its action has changed the game.
struct Owner {
    lines: VecDeque<String>,
    acts: VecDeque<Box<dyn FnMut()>>,
    buf: Vec<u8>,
    pos: usize,
}

impl Owner {
    fn new(script: Vec<(&str, Box<dyn FnMut()>)>) -> Owner {
        let (lines, acts): (VecDeque<String>, VecDeque<Box<dyn FnMut()>>) = script
            .into_iter()
            .map(|(l, a)| (format!("{l}\n"), a))
            .unzip();
        Owner {
            lines,
            acts,
            buf: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for Owner {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = {
            let avail = self.fill_buf()?;
            let n = avail.len().min(out.len());
            out[..n].copy_from_slice(&avail[..n]);
            n
        };
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for Owner {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.pos >= self.buf.len() {
            match self.lines.pop_front() {
                Some(line) => {
                    if let Some(mut act) = self.acts.pop_front() {
                        act();
                    }
                    self.buf = line.into_bytes();
                    self.pos = 0;
                }
                None => return Ok(&[]),
            }
        }
        Ok(&self.buf[self.pos..])
    }

    fn consume(&mut self, n: usize) {
        self.pos += n;
    }
}

fn noop() -> Box<dyn FnMut()> {
    Box::new(|| {})
}

fn root(tag: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!("kitkraft-walk-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(&r).unwrap();
    r
}

fn setup(file: &WalkFile, root: &Path, ids: BTreeMap<String, String>) -> WalkSetup {
    let mut s = WalkSetup::live(
        file,
        "walk-test",
        "2026-11-17",
        root.join("observe").join("walk-test"),
    );
    s.ids = ids;
    s.poll = Duration::from_millis(5);
    s.retry_pause = Duration::from_millis(5);
    s.quiet = Duration::from_millis(5);
    s.ready_limit = Duration::from_secs(5);
    s.call_timeout = Duration::from_secs(5);
    s.header = vec![("Game build".into(), "`Starfield.exe` 1.16.244.0".into())];
    s
}

fn walk(
    file: &WalkFile,
    s: &WalkSetup,
    fake: &FakeConsole,
    owner: &mut Owner,
) -> (WalkReport, String) {
    let mut console = HttpConsole::new(&fake.url()).unwrap();
    let mut screen: Vec<u8> = Vec::new();
    let mut shoot = |p: &Path| std::fs::write(p, b"png").map_err(|e| e.to_string());
    let report = run(file, s, &mut console, owner, &mut screen, &mut shoot).unwrap();
    (report, String::from_utf8_lossy(&screen).into_owned())
}

fn labels(r: &WalkReport) -> Vec<(String, String)> {
    r.rows
        .iter()
        .map(|x| (x.row.row.clone(), x.row.verdict.text()))
        .collect()
}

fn repo_walk() -> WalkFile {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/bench/owner-walk/stations.toml");
    WalkFile::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_repo_walk_passes_every_station_when_the_game_does_what_it_should() {
    let file = repo_walk();
    assert_eq!(file.stations.len(), 13);
    let ids: BTreeMap<String, String> = file
        .editor_ids()
        .into_iter()
        .enumerate()
        .map(|(i, e)| (e, format!("2C0010{i:02X}")))
        .collect();
    let id = |e: &str| ids[e].to_ascii_lowercase();
    let count = |e: &str| format!("player.getitemcount {}", id(e));
    let root = root("repo");
    let papyrus = root.join("Papyrus.0.log");
    std::fs::write(&papyrus, "[00:00:00] Papyrus log opened (PC-64)\n").unwrap();
    let game: Shared = Arc::new(Mutex::new(Game::default()));
    game.lock()
        .unwrap()
        .values
        .insert("player.getincell 0021966f".into(), 1.0);
    let fake = FakeConsole::start().unwrap();
    let g = Arc::clone(&game);
    fake.respond_with(move |c| respond(&g, c));

    // What each station's action does to the game, then the owner's line.
    let act = |f: Box<dyn Fn(&mut Game)>| -> Box<dyn FnMut()> {
        let g = Arc::clone(&game);
        Box::new(move || f(&mut g.lock().unwrap()))
    };
    let add = |key: String, n: f64| -> Box<dyn Fn(&mut Game)> {
        Box::new(move |g: &mut Game| *g.values.entry(key.clone()).or_insert(0.0) += n)
    };
    let p = papyrus.clone();
    let glob = format!("getglobalvalue {}", id("KKW02Glob"));
    // The profile's own saves folder: the owner's `save KitKraftWalk` writes there (W-09).
    let saves = root.join("profile").join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    let save_dir = saves.clone();
    let owner_script: Vec<(&str, Box<dyn FnMut()>)> = vec![
        ("", noop()),                                               // W-00
        ("p the prompt read Steal", noop()),                        // W-01
        ("p stolen, no bounty", act(add(count("KKW10Misc"), 1.0))), // W-02
        ("p Take", act(add(count("KKW11Misc"), 1.0))),              // W-03
        ("p Take", act(add(count("KKW12Misc"), 1.0))),              // W-04
        ("p owned, stolen", act(add(count("KKW13Misc"), 3.0))),     // W-05
        (
            "",
            act(Box::new(move |g: &mut Game| {
                *g.values.entry(glob.clone()).or_insert(0.0) += 1.0;
                let mut log = std::fs::read_to_string(&p).unwrap();
                log.push_str("[00:01:00] KKW-A01 activated by-player=true count=1\n");
                std::fs::write(&p, log).unwrap();
            })),
        ), // W-06
        (
            "p crafted",
            act(Box::new(|g: &mut Game| {
                *g.values
                    .entry("player.getitemcount 000547a1".into())
                    .or_insert(0.0) += 6.0;
                *g.values
                    .entry("player.getitemcount 000057c7".into())
                    .or_insert(0.0) -= 1.0;
                *g.values
                    .entry("player.getitemcount 000057d6".into())
                    .or_insert(0.0) -= 1.0;
            })),
        ), // W-07
        ("p Old Soviet Marksman Rifle", noop()),                    // W-08
        (
            "p 24 5 48, saved",
            Box::new(move || std::fs::write(save_dir.join("KitKraftWalk.sfs"), b"sfs").unwrap()),
        ), // W-09
        (
            "p same after the reload",
            act(add(count("KKW14Misc"), -1.0)),
        ), // W-10
        (
            "p Take, no crime",
            act(Box::new(|g: &mut Game| {
                g.values.insert("player.getincell 00017307".into(), 1.0);
            })),
        ), // W-11
        ("p built", noop()),                                        // W-12
    ];
    let mut owner = Owner::new(owner_script);
    let mut s = setup(&file, &root, ids.clone());
    s.papyrus_log = Some(papyrus.clone());
    s.saves_dir = Some(saves.clone());
    let (report, screen) = walk(&file, &s, &fake, &mut owner);

    for (row, label) in labels(&report) {
        assert_eq!(label, "pass", "{row}: {label}\n{screen}");
    }
    assert_eq!(report.status.label(), "pass");
    // The marker added after the save proves the reload (W-10).
    let w10 = report.rows.iter().find(|r| r.row.row == "W-10").unwrap();
    let marker = &w10.readings[0];
    assert_eq!((marker.1.value, marker.2.value), (Some(1.0), Some(0.0)));
    // Ungraded watches are recorded: W-07's iron went 10 -> 9.
    let w07 = report.rows.iter().find(|r| r.row.row == "W-07").unwrap();
    assert!(
        w07.row.seen.contains("iron count: 10 -> 9"),
        "{}",
        w07.row.seen
    );
    // No save, load, cell move or quit was ever sent.
    for c in fake.received() {
        let verb = c
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        assert!(
            !["save", "load", "coc", "qqq", "quitgame", "bat"].contains(&verb.as_str()),
            "{c}"
        );
    }
    // The raw files, and the draft entry in the ledger's format.
    let dir = &s.observe;
    for f in [
        "transcript.txt",
        "report.json",
        "ledger-draft.md",
        "papyrus-tail.log",
    ] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    assert!(dir.join("screenshots").join("W-01.png").is_file());
    let draft = std::fs::read_to_string(dir.join("ledger-draft.md")).unwrap();
    assert!(
        draft.contains(
            "### game-20261117-walk: owner walk walk-test (`kit walk`), KitKraftOwnerWalk.esm"
        ),
        "{draft}"
    );
    assert!(draft.contains("| Row | Step | Forms | Expected | Seen | Result | Evidence |"));
    assert!(draft.contains("| W-06 (OnActivate) |"), "{draft}");
    assert!(draft.contains("**pass** (owner report)"), "{draft}");
    assert!(
        draft.contains("W-01: owner report: p \"the prompt read Steal\""),
        "{draft}"
    );
    assert!(draft.contains(SFSE), "{draft}");
    let tail = std::fs::read_to_string(dir.join("papyrus-tail.log")).unwrap();
    assert!(
        tail.contains("KKW-A01") && !tail.contains("log opened"),
        "{tail}"
    );
    // report.json has the controller's row shape `cargo xtask session-evidence` reads.
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("report.json")).unwrap()).unwrap();
    assert_eq!(json["ledger_entry"], "game-20261117-walk");
    assert_eq!(json["rows"][6]["row"], "W-06");
    assert_eq!(json["rows"][6]["verdict"]["status"], "pass");
    assert!(json["rows"][6]["title"]
        .as_str()
        .unwrap()
        .contains("ACTI `KKW30Acti`"));
    // Nothing outside the walk's folder (and the test's log).
    let mut top: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    top.sort();
    assert_eq!(top, ["Papyrus.0.log", "observe", "profile"]);
    // The saves folder holds only the owner's save: the recorder listed it, nothing more.
    let in_saves: Vec<String> = std::fs::read_dir(&saves)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(in_saves, ["KitKraftWalk.sfs"]);
    let w09 = report.rows.iter().find(|r| r.row.row == "W-09").unwrap();
    assert!(
        w09.row
            .seen
            .contains("`KitKraftWalk.sfs` is in the profile's saves folder"),
        "{}",
        w09.row.seen
    );
    let _ = std::fs::remove_dir_all(&root);
}

fn small(judge: &str, extra: &str) -> WalkFile {
    WalkFile::parse(&format!(
        r#"
format = 1
title = "t"
plugin = "X.esm"

[[station]]
id = "S-01"
title = "take the mug"
say = ["Take the mug."]
expected = "one more mug"
judge = "{judge}"
{extra}
[[station.watch]]
command = "player.getitemcount {{id:Mug}}"
change = 1
"#
    ))
    .unwrap()
}

fn mug_ids() -> BTreeMap<String, String> {
    BTreeMap::from([("Mug".to_string(), "2C001000".to_string())])
}

fn mug_game() -> (Shared, FakeConsole) {
    let game: Shared = Arc::new(Mutex::new(Game::default()));
    let fake = FakeConsole::start().unwrap();
    let g = Arc::clone(&game);
    fake.respond_with(move |c| respond(&g, c));
    (game, fake)
}

fn take_mug(game: &Shared) -> Box<dyn FnMut()> {
    let g = Arc::clone(game);
    Box::new(move || {
        *g.lock()
            .unwrap()
            .values
            .entry("player.getitemcount 2c001000".into())
            .or_insert(0.0) += 1.0;
    })
}

#[test]
fn a_lost_answer_is_read_again_after_readiness_and_passes_when_it_comes() {
    let file = small("console", "");
    let (game, fake) = mug_game();
    let root = root("reread");
    let s = setup(&file, &root, mug_ids());
    // The after-read answers only its echo three times (as within ~6 s of a `coc`).
    let g = Arc::clone(&game);
    let mut act = take_mug(&game);
    let mut owner = Owner::new(vec![(
        "",
        Box::new(move || {
            act();
            g.lock()
                .unwrap()
                .mute
                .insert("player.getitemcount 2c001000".into(), 3);
        }),
    )]);
    let (report, _) = walk(&file, &s, &fake, &mut owner);
    assert_eq!(labels(&report), [("S-01".to_string(), "pass".to_string())]);
    let reading = &report.rows[0].readings[0].2;
    assert_eq!(reading.sends, 4, "three sends, then the re-read");
    // Between the third and the fourth send: a readiness poll.
    let got = fake.received();
    let after: Vec<&String> = got
        .iter()
        .skip_while(|c| !c.contains("2C001000"))
        .skip(1)
        .collect();
    let k = after.iter().position(|c| c.contains("2C001000")).unwrap();
    let sends: Vec<usize> = after
        .iter()
        .enumerate()
        .filter(|(_, c)| c.contains("2C001000"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(sends.len(), 4, "{after:?}");
    assert!(
        after[sends[2]..sends[3]]
            .iter()
            .any(|c| *c == "GetSFSEVersion"),
        "{after:?} ({k})"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_late_answer_reaches_its_read_not_the_next_one() {
    let file = small("console", "");
    let (game, fake) = mug_game();
    game.lock().unwrap().late = true;
    let root = root("late");
    let s = setup(&file, &root, mug_ids());
    // The after-read's answer comes one reply late (in the reply to its retry).
    let g = Arc::clone(&game);
    let mut act = take_mug(&game);
    let mut owner = Owner::new(vec![(
        "",
        Box::new(move || {
            act();
            g.lock()
                .unwrap()
                .mute
                .insert("player.getitemcount 2c001000".into(), 1);
        }),
    )]);
    let (report, _) = walk(&file, &s, &fake, &mut owner);
    assert_eq!(labels(&report), [("S-01".to_string(), "pass".to_string())]);
    assert_eq!(report.rows[0].readings[0].2.value, Some(1.0));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_answer_that_stays_lost_is_inconclusive_never_a_pass() {
    let file = small("console", "");
    let (game, fake) = mug_game();
    let root = root("lost");
    let s = setup(&file, &root, mug_ids());
    let g = Arc::clone(&game);
    let mut act = take_mug(&game);
    let mut owner = Owner::new(vec![(
        "p",
        Box::new(move || {
            act();
            g.lock()
                .unwrap()
                .mute
                .insert("player.getitemcount 2c001000".into(), 99);
        }),
    )]);
    let (report, _) = walk(&file, &s, &fake, &mut owner);
    let v = &report.rows[0].row.verdict;
    assert_eq!(v.label(), "inconclusive", "{v:?}");
    assert!(v.reason().contains("answer lost"), "{v:?}");
    assert_eq!(report.status.label(), "inconclusive");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_owner_fail_fails_the_station_though_the_console_passes() {
    let file = small("console", "");
    let (game, fake) = mug_game();
    let root = root("ownerfail");
    let s = setup(&file, &root, mug_ids());
    let mut owner = Owner::new(vec![("f the notification never showed", take_mug(&game))]);
    let (report, _) = walk(&file, &s, &fake, &mut owner);
    let v = &report.rows[0].row.verdict;
    assert_eq!(v.label(), "fail");
    assert!(
        v.reason()
            .contains("owner report: the notification never showed"),
        "{v:?}"
    );
    // The watch itself passed and is in the row's evidence.
    assert!(
        report.rows[0].row.seen.contains("0 -> 1"),
        "{}",
        report.rows[0].row.seen
    );
    let draft = std::fs::read_to_string(s.observe.join("ledger-draft.md")).unwrap();
    assert!(
        draft.contains("S-01: owner report: f \"the notification never showed\""),
        "{draft}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn enter_alone_where_the_owner_decides_is_inconclusive_and_skip_and_quit_are_not_run() {
    let file = WalkFile::parse(
        r#"
format = 1
title = "t"
plugin = "X.esm"

[[station]]
id = "S-01"
title = "look"
say = ["Look."]
expected = "Steal"
judge = "owner"

[[station]]
id = "S-02"
title = "look again"
say = ["Look."]
expected = "Take"
judge = "owner"

[[station]]
id = "S-03"
title = "third"
say = ["Go."]
expected = "x"
judge = "owner"

[[station]]
id = "S-04"
title = "fourth"
say = ["Go."]
expected = "x"
judge = "owner"
"#,
    )
    .unwrap();
    let (_, fake) = mug_game();
    let root = root("answers");
    let s = setup(&file, &root, BTreeMap::new());
    let mut owner = Owner::new(vec![
        ("", noop()),
        ("s no time", noop()),
        ("q", noop()),
        ("p never read", noop()),
    ]);
    let (report, _) = walk(&file, &s, &fake, &mut owner);
    let got = labels(&report);
    assert!(
        got[0].1.starts_with("inconclusive (no owner report"),
        "{got:?}"
    );
    assert_eq!(got[1].1, "not-run (skipped by the owner: no time)");
    assert!(
        got[2]
            .1
            .starts_with("not-run (the owner stopped the walk at S-03"),
        "{got:?}"
    );
    assert!(
        got[3]
            .1
            .starts_with("not-run (the owner stopped the walk at S-03"),
        "{got:?}"
    );
    // A station with no console part sends nothing at all.
    assert!(fake.received().is_empty(), "{:?}", fake.received());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_walk_file_that_saves_loads_or_moves_is_refused() {
    for setup_cmd in [
        "save KitKraftWalk",
        "load KitKraftWalk",
        "coc X",
        "player.moveto 0001",
    ] {
        let text = format!(
            "format = 1\ntitle = \"t\"\nplugin = \"X.esm\"\n[[station]]\nid = \"S\"\ntitle = \"t\"\nsay = [\"x\"]\nexpected = \"x\"\njudge = \"owner\"\nsetup = [\"{setup_cmd}\"]\n"
        );
        assert!(WalkFile::parse(&text).is_err(), "{setup_cmd}");
    }
}

#[test]
fn an_unknown_editor_id_stops_the_walk_before_anything_is_sent() {
    let file = small("console", "");
    let (_, fake) = mug_game();
    let root = root("ids");
    let s = setup(&file, &root, BTreeMap::new());
    let mut console = HttpConsole::new(&fake.url()).unwrap();
    let mut owner = Owner::new(vec![]);
    let mut screen = Vec::new();
    let mut shoot = |_: &Path| Ok(());
    let err = run(&file, &s, &mut console, &mut owner, &mut screen, &mut shoot).unwrap_err();
    assert!(err.to_string().contains("Mug"), "{err}");
    assert!(fake.received().is_empty());
    assert!(!s.observe.exists());
    let _ = std::fs::remove_dir_all(&root);
}

fn save_walk() -> WalkFile {
    WalkFile::parse(
        r#"
format = 1
title = "t"
plugin = "X.esm"

[[station]]
id = "S-09"
title = "save"
say = ["save KitKraftWalk"]
expected = "the save is written"
judge = "owner"
makes_save = "KitKraftWalk"

[[station]]
id = "S-10"
title = "load"
say = ["load KitKraftWalk"]
expected = "back from the save"
judge = "owner"
needs_save = "KitKraftWalk"
"#,
    )
    .unwrap()
}

#[test]
fn a_save_that_did_not_land_is_asked_for_again_and_passes_once_written() {
    let file = save_walk();
    let (_game, fake) = mug_game();
    let root = root("save-again");
    let saves = root.join("profile").join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    let dir = saves.clone();
    let mut owner = Owner::new(vec![
        ("p saved", noop()),
        // Asked again: this time the save lands before the Enter.
        (
            "",
            Box::new(move || std::fs::write(dir.join("KitKraftWalk.sfs"), b"sfs").unwrap()),
        ),
        ("p same", noop()),
    ]);
    let mut s = setup(&file, &root, BTreeMap::new());
    s.saves_dir = Some(saves.clone());
    let (report, screen) = walk(&file, &s, &fake, &mut owner);
    assert_eq!(
        labels(&report),
        [
            ("S-09".into(), "pass".into()),
            ("S-10".into(), "pass".into())
        ],
        "{screen}"
    );
    assert!(screen.contains("The save did not land"), "{screen}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_save_that_never_lands_fails_its_station_and_the_load_is_not_asked() {
    let file = save_walk();
    let (_game, fake) = mug_game();
    let root = root("save-missing");
    let saves = root.join("profile").join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("Other.sfs"), b"sfs").unwrap();
    let mut owner = Owner::new(vec![("p saved", noop()), ("", noop()), ("", noop())]);
    let mut s = setup(&file, &root, BTreeMap::new());
    s.saves_dir = Some(saves.clone());
    let (report, screen) = walk(&file, &s, &fake, &mut owner);
    let l = labels(&report);
    assert!(l[0].1.starts_with("fail"), "{l:?}\n{screen}");
    assert!(l[0].1.contains("was not written"), "{l:?}");
    assert!(l[1].1.starts_with("not-run"), "{l:?}");
    assert!(l[1].1.contains("nothing to load"), "{l:?}");
    // Nothing was added to, renamed in or removed from the saves folder.
    let names: Vec<String> = std::fs::read_dir(&saves)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["Other.sfs"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_save_older_than_the_walk_does_not_count() {
    use kitkraft_game::walk::find_save;
    let root = root("save-old");
    let saves = root.join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("KitKraftWalk.sfs"), b"sfs").unwrap();
    let later = std::time::SystemTime::now() + Duration::from_secs(3600);
    assert_eq!(find_save(&saves, "kitkraftwalk", later), Ok(None));
    let earlier = std::time::SystemTime::UNIX_EPOCH;
    assert!(find_save(&saves, "kitkraftwalk", earlier)
        .unwrap()
        .is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_owners_primary_saves_folder_is_never_listed() {
    use kitkraft_game::walk::{find_save, saves_dir_allowed};
    let primary = PathBuf::from(r"C:\Users\someone\Documents\My Games\Starfield\Saves");
    assert!(!saves_dir_allowed(&primary));
    assert!(find_save(&primary, "KitKraftWalk", std::time::SystemTime::UNIX_EPOCH).is_err());
    assert!(saves_dir_allowed(&PathBuf::from(
        r"C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\saves"
    )));
    assert!(!saves_dir_allowed(&PathBuf::from(r"C:\Modding\elsewhere")));
}

#[test]
fn a_save_name_with_a_path_is_refused() {
    let text = r#"
format = 1
title = "t"
plugin = "X.esm"

[[station]]
id = "S-09"
title = "save"
say = ["save"]
expected = "e"
judge = "owner"
makes_save = "..\\Default\\x"
"#;
    assert!(WalkFile::parse(text).is_err());
}
