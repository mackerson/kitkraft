//! The owner walk recorder (`kit walk`, `docs/bench/owner-walk/README.md`): the owner plays,
//! the recorder reads the console around each thing the owner does.
//!
//! Unlike a session ([`crate::controller`]) it never starts, stops or waits for the game's
//! window, never sends `save`, `load` or a cell move, and touches no profile file: the owner
//! launches the game through MO2 on `Kitkraft Dev` and keeps the controls. It talks to the
//! same loopback console endpoint ([`crate::console::Console`]) and writes only inside one
//! folder of its own (`observe`).
//!
//! A walk file (`stations.toml`) lists stations in walking order. For each one the recorder
//!
//! 1. prints what to do in a line or two;
//! 2. waits for readiness (the ready command answers with its expected text), then sends
//!    nothing for the quiet period: commands sent during a `coc` load or within about 6 s of
//!    the ready signal are lost (`plan/STATUS.md`, the send-order item; KK-369);
//! 3. sends the station's setup commands (once each: an `additem` sent twice adds twice),
//!    then reads each watched getter ("before");
//! 4. waits for the owner's line: Enter, or `p`, `f` or `s` and an optional note
//!    (`q` stops the walk);
//! 5. waits for readiness again, reads the watched getters ("after"), runs the checks
//!    (console matchers or the Papyrus log written since the walk started), and takes a
//!    screenshot when the station asks for one.
//!
//! A station may name a save the owner makes (`makes_save`) or loads (`needs_save`). The
//! recorder then lists the profile's own saves folder ([`WalkSetup::saves_dir`], read only:
//! a listing and the entry's modified time, never a file opened, written, renamed or
//! deleted, and never a folder under `My Games`, [`saves_dir_allowed`]) for `<name>.sfs`
//! written since the walk started. A save that did not land is asked for again, then fails
//! its station; the station that loads it is then not asked (not-run). The owner walk of
//! 2026-11-17 (`game-20261117-08`, W-10) found its save missing only at the load.
//!
//! Every command and its whole reply is appended to `transcript.txt` as it happens; the
//! report (`report.json`, in the controller's row shape so `cargo xtask session-evidence`
//! reads it) and the draft ledger entry (`ledger-draft.md`, the format of
//! `docs/game-observations.md`) are rewritten after every station, so a walk cut short
//! keeps what it saw.
//!
//! Lost and late answers. Replies are routed by [`crate::correlate::Correlator`] (a late
//! line goes to the read it names). A getter answered only by its echo, or by nothing, is
//! sent again after a pause, twice; then the recorder waits for readiness and the quiet
//! period once more and reads it again. A read still without its line is `inconclusive
//! (answer lost)`, never a guess. Once a read has its line, or is given up after that
//! re-read, it is closed ([`crate::correlate::Correlator::close`]): the game drops, not
//! delays, an answer it does not give, and every new read follows a readiness poll and a
//! quiet period, which a late line would arrive in.
//!
//! Verdicts are the four of [`Verdict`]. A station judged by the owner (`judge = "owner"`
//! or `"both"`) needs the owner's `p` to pass; Enter alone is `inconclusive (no owner
//! report)`. An owner's `f` fails any station, and `s` makes it `not-run`. Console parts
//! (watches with an expected change, checks) count for `judge = "console"` and `"both"`;
//! a console failure line or a lost answer makes such a part `inconclusive`, since it
//! shows nothing of the game's behaviour. Missing evidence is never `pass`.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::console::Console;
use crate::correlate::Correlator;
use crate::error::{GameError, Result};
use crate::ledger::{cell, RowReport};
use crate::probes::{answer_number, clip, console_failure, judge, Matcher, Step, Verdict};

/// The walk file format this build reads.
pub const FORMAT: u32 = 1;

/// Commands a walk never sends: the controller's refused list, and everything that moves
/// the player, saves, loads or quits (the owner is at the controls; saves are theirs).
pub const REFUSED: &[&str] = &[
    "bat",
    "saveini",
    "qqq",
    "quitgame",
    "save",
    "load",
    "coc",
    "cow",
    "moveto",
    "player.moveto",
    "disable",
    "markfordelete",
];

/// Sends of one getter before the re-read (the first and two retries).
pub const TRIES: u32 = 3;

fn default_ready_command() -> String {
    "GetSFSEVersion".into()
}
fn default_ready_expect() -> String {
    "SFSE version".into()
}
fn default_quiet() -> u64 {
    8
}
fn default_ready_secs() -> u64 {
    180
}

/// A walk file (`stations.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkFile {
    /// [`FORMAT`].
    pub format: u32,
    /// What the walk is.
    pub title: String,
    /// The plugin whose EditorIDs `{id:<EditorID>}` names.
    pub plugin: String,
    /// Other plugins whose SHA-256 the entry records (the rename, the overhaul).
    #[serde(default)]
    pub also_record: Vec<String>,
    /// Notes for the reader of the entry.
    #[serde(default)]
    pub notes: Vec<String>,
    /// The readiness command.
    #[serde(default = "default_ready_command")]
    pub ready_command: String,
    /// Text the ready answer holds.
    #[serde(default = "default_ready_expect")]
    pub ready_expect: String,
    /// Seconds to send nothing after the ready answer.
    #[serde(default = "default_quiet")]
    pub quiet_secs: u64,
    /// Longest wait for the ready answer.
    #[serde(default = "default_ready_secs")]
    pub ready_secs: u64,
    /// The stations, in walking order.
    #[serde(default, rename = "station")]
    pub stations: Vec<Station>,
}

/// Who decides a station.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Judge {
    /// The owner's eyes: `p` passes, `f` fails; the console is recorded, not graded.
    Owner,
    /// The console's watches and checks; an owner's `f` still fails.
    Console,
    /// Both must pass.
    Both,
}

/// One station.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Station {
    /// Row id in the entry (`W-01`).
    pub id: String,
    /// What it checks (the entry's Step column).
    pub title: String,
    /// The template or milestone rows it covers (`O4-01`, `W12`).
    #[serde(default)]
    pub covers: Vec<String>,
    /// Forms (`Plugin.esm:XXXXXX`, EditorIDs).
    #[serde(default)]
    pub forms: Vec<String>,
    /// What to do, one or two plain lines.
    pub say: Vec<String>,
    /// What should happen (the entry's Expected column).
    pub expected: String,
    /// Who decides.
    pub judge: Judge,
    /// The question shown at the prompt.
    #[serde(default)]
    pub ask: Option<String>,
    /// A screenshot after the owner's answer.
    #[serde(default)]
    pub screenshot: bool,
    /// Commands sent once, after readiness, before the first reading.
    #[serde(default)]
    pub setup: Vec<String>,
    /// Getters read before and after the owner acts.
    #[serde(default)]
    pub watch: Vec<Watch>,
    /// Checks made after the owner acts (a console matcher or `papyrus-log`).
    #[serde(default)]
    pub check: Vec<Step>,
    /// The owner saves here under this name (`KitKraftWalk`): after the owner's answer the
    /// recorder lists the profile's saves folder (read only) for `<name>.sfs` written since
    /// the walk started, and asks again while it is missing (game-20261117-08, W-10).
    #[serde(default)]
    pub makes_save: Option<String>,
    /// This station loads the save of that name: when the profile's saves folder has no
    /// `<name>.sfs` written since the walk started, the station is not asked (not-run).
    #[serde(default)]
    pub needs_save: Option<String>,
}

/// A getter read before and after; graded when it names an expected change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watch {
    /// The getter.
    pub command: String,
    /// What it reads, in words.
    #[serde(default)]
    pub label: Option<String>,
    /// The expected change (after minus before), within `tolerance`.
    #[serde(default)]
    pub change: Option<f64>,
    /// For `change`.
    #[serde(default)]
    pub tolerance: Option<f64>,
    /// The smallest accepted change.
    #[serde(default)]
    pub min: Option<f64>,
    /// The largest accepted change.
    #[serde(default)]
    pub max: Option<f64>,
    /// Where the expectation comes from.
    #[serde(default)]
    pub why: Option<String>,
}

impl Watch {
    fn graded(&self) -> bool {
        self.change.is_some() || self.min.is_some() || self.max.is_some()
    }

    fn name(&self) -> String {
        self.label.clone().unwrap_or_else(|| self.command.clone())
    }

    fn expectation(&self) -> String {
        if let Some(c) = self.change {
            match self.tolerance.filter(|t| *t > 0.0) {
                Some(t) => format!("change {c} +/- {t}"),
                None => format!("change {c}"),
            }
        } else {
            match (self.min, self.max) {
                (Some(a), Some(b)) => format!("change {a} to {b}"),
                (Some(a), None) => format!("change at least {a}"),
                (None, Some(b)) => format!("change at most {b}"),
                (None, None) => "recorded".into(),
            }
        }
    }

    /// The verdict on `before` and `after`.
    pub fn verdict(&self, before: &Reading, after: &Reading) -> Verdict {
        let (b, a) = match (before.value, after.value) {
            (Some(b), Some(a)) => (b, a),
            _ => {
                let why = [("before", before), ("after", after)]
                    .iter()
                    .filter(|(_, r)| r.value.is_none())
                    .map(|(w, r)| format!("{w}: {}", r.why()))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Verdict::Inconclusive(format!("`{}`: {why}", self.command));
            }
        };
        let d = a - b;
        let ok = if let Some(c) = self.change {
            (d - c).abs() <= self.tolerance.unwrap_or(0.0).abs()
        } else {
            self.min.is_none_or(|m| d >= m) && self.max.is_none_or(|m| d <= m)
        };
        if ok {
            Verdict::Pass
        } else {
            Verdict::Fail(format!(
                "`{}` went from {b} to {a} (change {d}; expected {})",
                self.command,
                self.expectation()
            ))
        }
    }
}

impl WalkFile {
    /// Parses and checks a walk file.
    pub fn parse(text: &str) -> Result<WalkFile> {
        let f: WalkFile = toml::from_str(text).map_err(|e| GameError::Probes(e.to_string()))?;
        f.check()?;
        Ok(f)
    }

    /// Checks ids, commands and expectations.
    pub fn check(&self) -> Result<()> {
        let bad = |m: String| Err(GameError::Probes(m));
        if self.format != FORMAT {
            return bad(format!(
                "walk format {} (this build reads {FORMAT})",
                self.format
            ));
        }
        if self.stations.is_empty() {
            return bad("the walk has no station".into());
        }
        let mut seen = BTreeSet::new();
        for s in &self.stations {
            if s.id.trim().is_empty() || !seen.insert(s.id.as_str()) {
                return bad(format!("station id `{}` empty or used twice", s.id));
            }
            if s.say.is_empty() {
                return bad(format!("station {}: nothing to say", s.id));
            }
            let graded = s.watch.iter().any(Watch::graded) || !s.check.is_empty();
            if s.judge != Judge::Owner && !graded {
                return bad(format!(
                    "station {}: judged by the console but no watch has an expected change and no check is set",
                    s.id
                ));
            }
            let commands = s
                .setup
                .iter()
                .chain(s.watch.iter().map(|w| &w.command))
                .chain(s.check.iter().map(|c| &c.command));
            for c in commands {
                if c.contains('\n') || c.contains('\r') || c.len() > crate::probes::MAX_COMMAND {
                    return bad(format!("station {}: command `{}`", s.id, clip(c, 60)));
                }
                if let Some(why) = refused(c) {
                    return bad(format!("station {}: {why}", s.id));
                }
            }
            for name in s.makes_save.iter().chain(s.needs_save.iter()) {
                if !save_name_ok(name) {
                    return bad(format!(
                        "station {}: save name `{}` (letters, digits, `-` and `_` only)",
                        s.id,
                        clip(name, 40)
                    ));
                }
            }
            for c in &s.check {
                if c.matcher.is_none() {
                    return bad(format!("station {}: a check needs a matcher", s.id));
                }
                if c.command.trim().is_empty() && c.matcher != Some(Matcher::PapyrusLog) {
                    return bad(format!("station {}: empty check command", s.id));
                }
            }
        }
        Ok(())
    }

    /// The EditorIDs `{id:<EditorID>}` names, sorted.
    pub fn editor_ids(&self) -> Vec<String> {
        let mut out = BTreeSet::new();
        for s in &self.stations {
            let texts = s
                .setup
                .iter()
                .chain(s.watch.iter().map(|w| &w.command))
                .chain(s.check.iter().map(|c| &c.command))
                .chain(s.say.iter());
            for t in texts {
                let mut rest = t.as_str();
                while let Some(at) = rest.find("{id:") {
                    let after = rest.get(at.saturating_add(4)..).unwrap_or_default();
                    match after.find('}') {
                        Some(end) => {
                            out.insert(after.get(..end).unwrap_or_default().to_string());
                            rest = after.get(end..).unwrap_or_default();
                        }
                        None => break,
                    }
                }
            }
        }
        out.into_iter().collect()
    }
}

/// A save name a station may name: 1 to 64 letters, digits, `-` and `_` (no path).
pub fn save_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Whether the recorder may list `dir` for saves: never a folder under `My Games` (the
/// owner's primary saves, AGENTS.md "Owner's live game is off limits"); only a profile's
/// own `saves` folder, which `kit walk` passes when the profile says `LocalSaves=true`.
pub fn saves_dir_allowed(dir: &Path) -> bool {
    let under_my_games = dir.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("My Games")
    });
    let named_saves = dir
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("saves"));
    !under_my_games && named_saves
}

/// `<name>.sfs` in `dir`, if it was modified at or after `since`: its modified time. Lists
/// the folder and reads the entry's metadata; never opens, writes, renames or deletes a
/// file. `Err` when the folder cannot be listed or is not one a walk may list.
pub fn find_save(
    dir: &Path,
    name: &str,
    since: std::time::SystemTime,
) -> std::result::Result<Option<std::time::SystemTime>, String> {
    if !saves_dir_allowed(dir) {
        return Err(format!(
            "{} is not a profile's own saves folder: not listed",
            dir.display()
        ));
    }
    let want = format!("{name}.sfs");
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for e in entries.flatten() {
        if !e.file_name().to_string_lossy().eq_ignore_ascii_case(&want) {
            continue;
        }
        let modified = e.metadata().and_then(|m| m.modified()).ok();
        return Ok(modified.filter(|m| *m >= since));
    }
    Ok(None)
}

/// Why `command` may not be sent, if it may not.
pub fn refused(command: &str) -> Option<String> {
    let verb = command
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let short = verb.rsplit('.').next().unwrap_or(&verb).to_string();
    REFUSED.iter().any(|r| *r == verb || *r == short).then(|| {
        format!(
            "`{command}`: a walk never sends `{verb}` (the owner keeps the controls and the saves)"
        )
    })
}

/// `text` with `{id:<EditorID>}` replaced from `ids` (runtime IDs); an unknown name is an
/// error naming it.
pub fn fill(text: &str, ids: &BTreeMap<String, String>) -> std::result::Result<String, String> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("{id:") {
        out.push_str(rest.get(..at).unwrap_or_default());
        let after = rest.get(at.saturating_add(4)..).unwrap_or_default();
        let Some(end) = after.find('}') else {
            return Err(format!("`{text}`: `{{id:` without `}}`"));
        };
        let name = after.get(..end).unwrap_or_default();
        let id = ids
            .get(name)
            .ok_or_else(|| format!("`{text}`: no runtime ID for `{name}`"))?;
        out.push_str(id);
        rest = after.get(end.saturating_add(1)..).unwrap_or_default();
    }
    out.push_str(rest);
    Ok(out)
}

/// What the owner typed at a station's prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "answer", content = "note", rename_all = "kebab-case")]
pub enum Answer {
    /// Enter, or a note without a verdict letter.
    Done(String),
    /// `p`: it looked right.
    Pass(String),
    /// `f`: it looked wrong.
    Fail(String),
    /// `s`: not done.
    Skip(String),
    /// `q`, or the input ended: stop the walk here.
    Quit(String),
}

impl Answer {
    /// Reads one typed line: `p`, `f`, `s` or `q` as the first word (any case), then a note.
    pub fn parse(line: &str) -> Answer {
        let t = line.trim();
        let (first, rest) = match t.split_once(char::is_whitespace) {
            Some((a, b)) => (a, b.trim().to_string()),
            None => (t, String::new()),
        };
        match first.to_ascii_lowercase().as_str() {
            "p" | "pass" => Answer::Pass(rest),
            "f" | "fail" => Answer::Fail(rest),
            "s" | "skip" => Answer::Skip(rest),
            "q" | "quit" => Answer::Quit(rest),
            _ => Answer::Done(t.to_string()),
        }
    }

    fn note(&self) -> &str {
        match self {
            Answer::Done(n)
            | Answer::Pass(n)
            | Answer::Fail(n)
            | Answer::Skip(n)
            | Answer::Quit(n) => n,
        }
    }

    fn text(&self) -> String {
        let (word, note) = match self {
            Answer::Done(n) => ("Enter", n),
            Answer::Pass(n) => ("p", n),
            Answer::Fail(n) => ("f", n),
            Answer::Skip(n) => ("s", n),
            Answer::Quit(n) => ("q", n),
        };
        if note.is_empty() {
            word.to_string()
        } else {
            format!("{word} \"{note}\"")
        }
    }
}

/// One getter reading.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reading {
    /// The command sent.
    pub command: String,
    /// The number after `>>` in the lines naming it.
    pub value: Option<f64>,
    /// The lines that answered it.
    pub lines: Vec<String>,
    /// Sends (with retries and the re-read).
    pub sends: u32,
    /// Why there is no value, when there is none.
    pub missing: Option<String>,
}

impl Reading {
    fn why(&self) -> String {
        self.missing
            .clone()
            .unwrap_or_else(|| "no number in the answer".into())
    }

    fn shown(&self) -> String {
        match self.value {
            Some(v) => format!("{v}"),
            None => format!("? ({})", self.why()),
        }
    }
}

/// One station's record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StationReport {
    /// The row, as the controller reports rows (`cargo xtask session-evidence` reads it).
    #[serde(flatten)]
    pub row: RowReport,
    /// The rows it covers.
    pub covers: Vec<String>,
    /// Who judged it.
    pub judge: Judge,
    /// What the owner typed (an owner report).
    pub owner: Option<Answer>,
    /// `(watch, before, after)`.
    pub readings: Vec<(String, Reading, Reading)>,
}

/// The whole walk.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalkReport {
    /// `walk-<yyyy-mm-dd>-<hhmmss>`.
    pub session: String,
    /// The draft entry's id (renumber it when the entry is entered).
    pub ledger_entry: Option<String>,
    /// The worst station.
    pub status: Verdict,
    /// Header fields of the entry.
    pub header: Vec<(String, String)>,
    /// The stations, in order.
    pub rows: Vec<StationReport>,
    /// Findings.
    pub findings: Vec<String>,
    /// Where the walk wrote.
    pub observe: PathBuf,
}

/// Everything a walk needs besides the file and the channels.
#[derive(Debug, Clone)]
pub struct WalkSetup {
    /// `walk-<yyyy-mm-dd>-<hhmmss>`.
    pub session: String,
    /// yyyy-mm-dd.
    pub date: String,
    /// The draft ledger id.
    pub ledger_id: String,
    /// The folder the walk writes (created; nothing is written outside it).
    pub observe: PathBuf,
    /// `{id:<EditorID>}` -> runtime ID.
    pub ids: BTreeMap<String, String>,
    /// Header fields from the caller (game build, profile, plugin hashes, console URL).
    pub header: Vec<(String, String)>,
    /// The Papyrus log, when known.
    pub papyrus_log: Option<PathBuf>,
    /// Between readiness polls.
    pub poll: Duration,
    /// The first retry's pause (doubled on the second).
    pub retry_pause: Duration,
    /// Nothing is sent this long after the ready answer.
    pub quiet: Duration,
    /// Longest wait for readiness.
    pub ready_limit: Duration,
    /// One console call's limit.
    pub call_timeout: Duration,
    /// The profile's own saves folder (`<profile>\saves`, when its `settings.ini` says
    /// `LocalSaves=true`), listed read-only for `makes_save` and `needs_save`; `None`
    /// turns those checks off (they are then recorded as not verified).
    pub saves_dir: Option<PathBuf>,
    /// How many times a missing save is asked for again before the station records it.
    pub save_asks: u32,
}

impl WalkSetup {
    /// The live timings of `file` (polls every second, retries after 2 s and 4 s).
    pub fn live(file: &WalkFile, session: &str, date: &str, observe: PathBuf) -> WalkSetup {
        WalkSetup {
            session: session.into(),
            date: date.into(),
            ledger_id: format!("game-{}-walk", crate::ledger::compact(date)),
            observe,
            ids: BTreeMap::new(),
            header: Vec::new(),
            papyrus_log: None,
            poll: Duration::from_secs(1),
            retry_pause: Duration::from_secs(2),
            quiet: Duration::from_secs(file.quiet_secs),
            ready_limit: Duration::from_secs(file.ready_secs),
            call_timeout: Duration::from_secs(10),
            saves_dir: None,
            save_asks: 2,
        }
    }
}

/// How a station's screenshot is taken (the host's screen capture in `kit walk`).
pub type Shoot<'a> = &'a mut dyn FnMut(&Path) -> std::result::Result<(), String>;

struct Walker<'a> {
    file: &'a WalkFile,
    setup: &'a WalkSetup,
    console: &'a mut dyn Console,
    input: &'a mut dyn BufRead,
    out: &'a mut dyn Write,
    shoot: Shoot<'a>,
    corr: Correlator,
    start: Instant,
    started: std::time::SystemTime,
    papyrus_mark: Option<u64>,
    sfse: Option<String>,
    findings: Vec<String>,
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> GameError + '_ {
    move |e| GameError::io(path, e)
}

impl Walker<'_> {
    fn ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn say(&mut self, text: &str) {
        let _ = writeln!(self.out, "{text}");
        let _ = self.out.flush();
    }

    fn raw(&mut self, text: &str) {
        let path = self.setup.observe.join("transcript.txt");
        let line = format!("[+{:>9.3}s] {text}\n", self.ms() as f64 / 1000.0);
        let opened = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path);
        match opened.and_then(|mut f| f.write_all(line.as_bytes())) {
            Ok(()) => {}
            Err(e) => {
                let note = format!("transcript not written ({}): {e}", path.display());
                if !self.findings.contains(&note) {
                    self.findings.push(note);
                }
            }
        }
    }

    /// Sends `command` once; logs it and its whole reply.
    fn send(&mut self, command: &str) -> std::result::Result<String, String> {
        self.raw(&format!("> {command}"));
        match self.console.run(command, self.setup.call_timeout) {
            Ok(reply) => {
                let shown = if reply.trim().is_empty() {
                    "(empty)".to_string()
                } else {
                    reply.trim_end().replace('\n', "\n              | ")
                };
                self.raw(&format!("< {shown}"));
                Ok(reply)
            }
            Err(e) => {
                self.raw(&format!("< ({e})"));
                Err(e.to_string())
            }
        }
    }

    /// Polls the ready command until its answer holds the expected text, then sends
    /// nothing for the quiet period. Replies are routed, so a late line reaches its read.
    fn wait_ready(&mut self, row: &str) -> std::result::Result<(), String> {
        let started = Instant::now();
        let cmd = self.file.ready_command.clone();
        let expect = self.file.ready_expect.to_ascii_lowercase();
        let poll = self.corr.open_poll(row, usize::MAX, &cmd);
        let mut told = false;
        let mut last = String::from("no answer yet");
        let result = loop {
            if started.elapsed() >= self.setup.ready_limit {
                break Err(format!(
                    "the game did not answer `{cmd}` with `{}` within {}s ({last})",
                    self.file.ready_expect,
                    self.setup.ready_limit.as_secs()
                ));
            }
            match self.send(&cmd) {
                Ok(reply) => {
                    let _ = self.corr.route(poll, &reply);
                    let hit = reply
                        .lines()
                        .find(|l| l.to_ascii_lowercase().contains(&expect))
                        .map(|l| l.trim().to_string());
                    if let Some(line) = hit {
                        let line = line
                            .find(&self.file.ready_expect)
                            .and_then(|i| line.get(i..))
                            .unwrap_or(&line)
                            .to_string();
                        self.sfse.get_or_insert(line);
                        break Ok(());
                    }
                    last = if reply.trim().is_empty() {
                        "an empty answer".into()
                    } else {
                        format!("`{}`", clip(&reply, 60))
                    };
                }
                Err(e) => last = e,
            }
            if !told {
                self.say("   (waiting for the game to answer the console...)");
                told = true;
            }
            std::thread::sleep(self.setup.poll);
        };
        self.corr.resend(poll);
        self.corr.close(poll);
        if result.is_ok() {
            self.raw(&format!(
                "(ready; quiet for {} ms, nothing sent)",
                self.setup.quiet.as_millis()
            ));
            std::thread::sleep(self.setup.quiet);
        }
        result
    }

    /// Reads one getter, with retries and one re-read after readiness (module docs).
    fn read(&mut self, row: &str, step: usize, command: &str) -> (usize, Reading) {
        let id = self.corr.open(row, step, command);
        let mut sends = 0u32;
        let mut reread = false;
        let mut failure: Option<String> = None;
        let mut error: Option<String>;
        let has_line = |c: &Correlator| c.request(id).is_some_and(|r| !r.lines.is_empty());
        loop {
            if sends > 0 {
                self.corr.resend(id);
            }
            sends = sends.saturating_add(1);
            match self.send(command) {
                Ok(reply) => {
                    let routed = self.corr.route(id, &reply);
                    failure = failure.or_else(|| console_failure(command, &routed.kept()));
                    error = None;
                }
                Err(e) => error = Some(e),
            }
            if has_line(&self.corr) || failure.is_some() {
                break;
            }
            if sends < TRIES {
                let pause = self.setup.retry_pause.saturating_mul(sends);
                self.raw(&format!(
                    "(no answer of its own for `{command}`; again in {} ms)",
                    pause.as_millis()
                ));
                std::thread::sleep(pause);
                continue;
            }
            if reread {
                break;
            }
            reread = true;
            self.raw(&format!(
                "(`{command}` still unanswered after {sends} sends: waiting for readiness and reading it again)"
            ));
            if let Err(e) = self.wait_ready(row) {
                error = Some(e);
                break;
            }
            if has_line(&self.corr) {
                // Its line came late, in a readiness poll's reply: routed to it there.
                self.raw(&format!(
                    "(`{command}`: its answer arrived late, in the readiness poll)"
                ));
                break;
            }
        }
        let lines = self
            .corr
            .request(id)
            .map(|r| r.lines.clone())
            .unwrap_or_default();
        self.corr.close(id);
        let value = if failure.is_some() {
            None
        } else {
            answer_number(&lines.join("\n"))
        };
        let missing = if let Some(f) = &failure {
            Some(format!("the console answered `{f}`"))
        } else if lines.is_empty() {
            Some(match error {
                Some(e) => format!("answer lost ({e}) after {sends} sends"),
                None => format!("answer lost: only the echo or nothing in {sends} sends"),
            })
        } else if value.is_none() {
            Some(format!(
                "no number after `>>` in `{}`",
                clip(&lines.join(" "), 80)
            ))
        } else {
            None
        };
        (
            id,
            Reading {
                command: command.to_string(),
                value,
                lines,
                sends,
                missing,
            },
        )
    }

    fn papyrus_text(&self) -> Option<String> {
        let path = self.setup.papyrus_log.as_ref()?;
        let bytes = std::fs::read(path).ok()?;
        let from = self.papyrus_mark.unwrap_or(0);
        let from = usize::try_from(from).unwrap_or(0);
        // A shorter log than at the start was rewritten (the game started again): all of it.
        let tail = bytes.get(from..).unwrap_or(&bytes);
        Some(String::from_utf8_lossy(tail).into_owned())
    }

    /// Whether `<name>.sfs` was written since the walk started, in words for the Seen
    /// column: `Ok(Some(..))` found, `Ok(None)` missing, `Err` not checkable.
    fn save_seen(&self, name: &str) -> std::result::Result<Option<String>, String> {
        let dir = self.setup.saves_dir.as_ref().ok_or_else(|| {
            "no saves folder to list (the profile does not keep its own saves)".to_string()
        })?;
        let found = find_save(dir, name, self.started)?;
        Ok(found.map(|m| {
            let after = m
                .duration_since(self.started)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!(
                "`{name}.sfs` is in the profile's saves folder, written {after} s after the walk started"
            )
        }))
    }

    /// After the owner's answer at a station that makes a save: looks for it, and while it
    /// is missing says so and asks the owner to save again (up to `save_asks` times).
    fn check_made_save(&mut self, name: &str, seen: &mut Vec<String>) -> Verdict {
        let mut asks = 0u32;
        loop {
            // The game writes the file a moment after the console command: a few polls.
            let mut status = self.save_seen(name);
            for _ in 0..5 {
                if !matches!(status, Ok(None)) {
                    break;
                }
                std::thread::sleep(self.setup.poll);
                status = self.save_seen(name);
            }
            match status {
                Ok(Some(found)) => {
                    self.raw(&format!("(save check: {found})"));
                    self.say(&format!("   ok: {found}"));
                    seen.push(found);
                    return Verdict::Pass;
                }
                Err(e) => {
                    let line = format!("save `{name}` not verified: {e}");
                    self.raw(&format!("(save check: {line})"));
                    self.say(&format!("   ! {line}"));
                    seen.push(line.clone());
                    return Verdict::Inconclusive(line);
                }
                Ok(None) => {}
            }
            let dir = self
                .setup
                .saves_dir
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            let missing = format!("no `{name}.sfs` written since the walk started in {dir}");
            self.raw(&format!("(save check: {missing})"));
            if asks >= self.setup.save_asks {
                let line = format!(
                    "the save `{name}` was not written: {missing} (asked {asks} more time(s))"
                );
                self.say(&format!("   ! {line}"));
                seen.push(line.clone());
                return Verdict::Fail(line);
            }
            asks = asks.saturating_add(1);
            self.say(&format!("   ! The save did not land: {missing}."));
            self.say(&format!(
                "     Open the console, type  save {name}  and press Enter in the console; wait until the game has saved, close the console."
            ));
            let _ = write!(
                self.out,
                " > Press Enter here when it has saved (s goes on without the save): "
            );
            let _ = self.out.flush();
            let mut line = String::new();
            let reply = match self.input.read_line(&mut line) {
                Ok(0) | Err(_) => "q".to_string(),
                Ok(_) => line.trim().to_string(),
            };
            self.raw(&format!("owner (save asked again): {reply:?}"));
            let first = reply.chars().next().map(|c| c.to_ascii_lowercase());
            if matches!(first, Some('s') | Some('q')) {
                let line = format!(
                    "the save `{name}` was not written: {missing} (the owner went on without it)"
                );
                seen.push(line.clone());
                return Verdict::Fail(line);
            }
        }
    }

    fn answer(&mut self, station: &Station) -> Answer {
        let ask = station.ask.clone().unwrap_or_else(|| "Done?".to_string());
        let _ = write!(
            self.out,
            " > {ask} Enter when done, or p / f / s and a note (q stops): "
        );
        let _ = self.out.flush();
        let mut line = String::new();
        match self.input.read_line(&mut line) {
            Ok(0) | Err(_) => Answer::Quit("the input ended".into()),
            Ok(_) => Answer::parse(&line),
        }
    }

    fn station(&mut self, n: usize, station: &Station) -> (StationReport, bool) {
        let total = self.file.stations.len();
        let covers = if station.covers.is_empty() {
            String::new()
        } else {
            format!(" ({})", station.covers.join(", "))
        };
        self.say("");
        self.say(&format!(
            "== {}{covers}: {}   [{}/{total}]",
            station.id,
            station.title,
            n.saturating_add(1)
        ));
        self.raw(&format!("== station {}: {}", station.id, station.title));
        let setup = self.setup;
        let fill_all = |t: &str| fill(t, &setup.ids);
        for line in &station.say {
            let text = fill_all(line).unwrap_or_else(|e| format!("{line} [{e}]"));
            self.say(&format!("   {text}"));
        }
        self.say(&format!("   Expected: {}", station.expected));
        let mut seen: Vec<String> = Vec::new();
        if let Some(name) = &station.needs_save {
            match self.save_seen(name) {
                Ok(Some(found)) => {
                    self.raw(&format!("(save check: {found})"));
                    seen.push(found);
                }
                Ok(None) => {
                    let dir = self
                        .setup
                        .saves_dir
                        .as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_default();
                    let why = format!(
                        "the save `{name}` this station loads is not in the profile's saves folder ({dir}; none written since the walk started): nothing to load"
                    );
                    self.raw(&format!("(save check: {why})"));
                    self.say(&format!("   ! {why}. Skipping this station."));
                    let report = self.row(
                        station,
                        Vec::new(),
                        vec![why.clone()],
                        Verdict::NotRun(why),
                        Vec::new(),
                        None,
                        Vec::new(),
                    );
                    return (report, false);
                }
                Err(e) => {
                    let line = format!("save `{name}` not verified before the load: {e}");
                    self.raw(&format!("(save check: {line})"));
                    seen.push(line);
                }
            }
        }
        let mut parts: Vec<Verdict> = Vec::new();
        let mut commands = Vec::new();
        let console_counts = station.judge != Judge::Owner;
        let needs_console =
            !station.setup.is_empty() || !station.watch.is_empty() || !station.check.is_empty();

        // Before.
        let mut befores: Vec<Reading> = Vec::new();
        let mut ready_before = true;
        if !station.setup.is_empty() || !station.watch.is_empty() {
            if let Err(e) = self.wait_ready(&station.id) {
                ready_before = false;
                seen.push(format!("before: {e}"));
                self.say(&format!("   ! {e}"));
            }
        }
        for c in &station.setup {
            let cmd = match fill_all(c) {
                Ok(c) => c,
                Err(e) => {
                    parts.push(Verdict::Inconclusive(e));
                    continue;
                }
            };
            commands.push(cmd.clone());
            if !ready_before {
                self.say(&format!(
                    "   ! not sent (the game was not ready): type it yourself: {cmd}"
                ));
                seen.push(format!("setup `{cmd}` not sent"));
                continue;
            }
            match self.send(&cmd) {
                Ok(r) if !r.trim().is_empty() => {
                    if let Some(f) = console_failure(&cmd, &r) {
                        seen.push(format!("setup `{cmd}`: `{f}`"));
                        parts.push(Verdict::Inconclusive(format!(
                            "setup `{cmd}`: the console answered `{f}`"
                        )));
                        self.say(&format!("   ! the console answered `{f}` to `{cmd}`"));
                    } else {
                        seen.push(format!("setup `{cmd}` sent"));
                    }
                }
                other => {
                    let why = other.err().unwrap_or_else(|| "an empty answer".into());
                    seen.push(format!("setup `{cmd}`: {why}"));
                    self.say(&format!(
                        "   ! `{cmd}` may not have arrived ({why}); if what it gives is missing, type it in the console"
                    ));
                }
            }
        }
        for (i, w) in station.watch.iter().enumerate() {
            let reading = match fill_all(&w.command) {
                Ok(cmd) if ready_before => {
                    commands.push(cmd.clone());
                    self.read(&station.id, i, &cmd).1
                }
                Ok(cmd) => Reading {
                    command: cmd,
                    value: None,
                    lines: Vec::new(),
                    sends: 0,
                    missing: Some("not read: the game was not ready".into()),
                },
                Err(e) => Reading {
                    command: w.command.clone(),
                    value: None,
                    lines: Vec::new(),
                    sends: 0,
                    missing: Some(e),
                },
            };
            befores.push(reading);
        }

        // The owner.
        let answer = self.answer(station);
        self.raw(&format!("owner: {}", answer.text()));
        if let Answer::Quit(_) = answer {
            let report = self.row(
                station,
                commands,
                seen,
                Verdict::NotRun(format!(
                    "the owner stopped the walk at {} ({})",
                    station.id,
                    answer.text()
                )),
                Vec::new(),
                None,
                Vec::new(),
            );
            return (report, true);
        }
        if let Answer::Skip(note) = &answer {
            let why = if note.is_empty() {
                "skipped by the owner".to_string()
            } else {
                format!("skipped by the owner: {note}")
            };
            let report = self.row(
                station,
                commands,
                seen,
                Verdict::NotRun(why),
                Vec::new(),
                Some(answer.clone()),
                Vec::new(),
            );
            return (report, false);
        }
        match (&answer, station.judge) {
            (Answer::Pass(_), Judge::Owner | Judge::Both) => parts.push(Verdict::Pass),
            (Answer::Fail(n), _) => parts.push(Verdict::Fail(format!(
                "owner report: {}",
                if n.is_empty() {
                    "it did not look right"
                } else {
                    n
                }
            ))),
            (Answer::Done(_), Judge::Owner | Judge::Both) => parts.push(Verdict::Inconclusive(
                "no owner report: the row needs the owner's eyes (type p or f)".into(),
            )),
            _ => {}
        }
        seen.push(format!("owner report: {}", answer.text()));
        if let Some(name) = &station.makes_save {
            let v = self.check_made_save(name, &mut seen);
            parts.push(v);
        }

        // After.
        let mut readings = Vec::new();
        let mut ready_after = true;
        if needs_console {
            if let Err(e) = self.wait_ready(&station.id) {
                ready_after = false;
                seen.push(format!("after: {e}"));
                self.say(&format!("   ! {e}"));
            }
        }
        for (i, (w, before)) in station.watch.iter().zip(befores).enumerate() {
            let after = if ready_after && before.sends > 0 {
                self.read(&station.id, i, &before.command).1
            } else {
                Reading {
                    command: before.command.clone(),
                    value: None,
                    lines: Vec::new(),
                    sends: 0,
                    missing: Some(if ready_after {
                        "not read (no reading before)".into()
                    } else {
                        "not read: the game was not ready".into()
                    }),
                }
            };
            seen.push(format!(
                "{}: {} -> {}",
                w.name(),
                before.shown(),
                after.shown()
            ));
            if w.graded() && console_counts {
                parts.push(w.verdict(&before, &after));
            }
            readings.push((w.name(), before, after));
        }
        for (i, c) in station.check.iter().enumerate() {
            let v = if c.matcher == Some(Matcher::PapyrusLog) {
                let text = self.papyrus_text();
                if text.is_none() {
                    seen.push("Papyrus log: not readable".into());
                }
                let v = judge(c, None, text.as_deref());
                seen.push(format!(
                    "Papyrus log `{}`: {}",
                    c.expected.as_deref().unwrap_or_default(),
                    v.label()
                ));
                v
            } else {
                match fill_all(&c.command) {
                    Ok(cmd) if ready_after && crate::correlate::subject(&cmd).is_none() => {
                        // A command whose answer names nothing: sent once, judged on its
                        // reply without the lines that name other reads.
                        commands.push(cmd.clone());
                        let id = self.corr.open(&station.id, usize::MAX, &cmd);
                        let v = match self.send(&cmd) {
                            Ok(reply) => {
                                let kept = self.corr.route(id, &reply).kept();
                                seen.push(format!("`{cmd}` -> {}", clip(&kept, 80)));
                                match console_failure(&cmd, &kept) {
                                    Some(f) => Verdict::Inconclusive(format!(
                                        "`{cmd}`: the console answered `{f}`"
                                    )),
                                    None => judge(c, Some(&kept), None),
                                }
                            }
                            Err(e) => Verdict::Inconclusive(format!("`{cmd}`: {e}")),
                        };
                        self.corr.close(id);
                        v
                    }
                    Ok(cmd) if ready_after => {
                        commands.push(cmd.clone());
                        let step = station.watch.len().saturating_add(i);
                        let (_, r) = self.read(&station.id, step, &cmd);
                        let v = if r.lines.is_empty() {
                            Verdict::Inconclusive(format!("`{cmd}`: {}", r.why()))
                        } else {
                            judge(c, Some(&r.lines.join("\n")), None)
                        };
                        seen.push(format!("`{cmd}` -> {}", clip(&r.lines.join(" "), 80)));
                        v
                    }
                    Ok(cmd) => {
                        Verdict::Inconclusive(format!("`{cmd}`: not sent, the game was not ready"))
                    }
                    Err(e) => Verdict::Inconclusive(e),
                }
            };
            if console_counts {
                parts.push(v);
            }
        }
        let mut shots = Vec::new();
        if station.screenshot {
            let name = format!("{}.png", station.id);
            let dest = self.setup.observe.join("screenshots").join(&name);
            let taken = std::fs::create_dir_all(self.setup.observe.join("screenshots"))
                .map_err(|e| e.to_string())
                .and_then(|()| (self.shoot)(&dest));
            match taken {
                Ok(()) if dest.is_file() => shots.push(name),
                Ok(()) => seen.push("screenshot not written".into()),
                Err(e) => seen.push(format!("screenshot not taken: {e}")),
            }
        }
        let verdict = if parts.is_empty() {
            Verdict::Inconclusive("nothing was judged (no owner report and no console part)".into())
        } else {
            Verdict::combine(&parts)
        };
        let report = self.row(
            station,
            commands,
            seen,
            verdict,
            shots,
            Some(answer),
            readings,
        );
        (report, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        station: &Station,
        commands: Vec<String>,
        seen: Vec<String>,
        verdict: Verdict,
        screenshots: Vec<String>,
        owner: Option<Answer>,
        readings: Vec<(String, Reading, Reading)>,
    ) -> StationReport {
        self.say(&format!("   {} -> {}", station.id, verdict.text()));
        self.raw(&format!("result {}: {}", station.id, verdict.text()));
        StationReport {
            row: RowReport {
                row: station.id.clone(),
                title: station.title.clone(),
                forms: station
                    .forms
                    .iter()
                    .map(|f| fill(f, &self.setup.ids).unwrap_or_else(|_| f.clone()))
                    .collect(),
                commands,
                expected: station.expected.clone(),
                seen: seen.join("; "),
                verdict,
                screenshots,
            },
            covers: station.covers.clone(),
            judge: station.judge,
            owner,
            readings,
        }
    }

    fn report(&self, rows: &[StationReport]) -> WalkReport {
        let verdicts: Vec<Verdict> = rows.iter().map(|r| r.row.verdict.clone()).collect();
        let mut header = self.setup.header.clone();
        header.push((
            "Script extender".into(),
            self.sfse
                .clone()
                .map(|l| {
                    format!(
                        "{l} (the console endpoint's answer to `{}`)",
                        self.file.ready_command
                    )
                })
                .unwrap_or_else(|| "unknown (no ready answer)".into()),
        ));
        WalkReport {
            session: self.setup.session.clone(),
            ledger_entry: Some(self.setup.ledger_id.clone()),
            status: Verdict::combine(&verdicts),
            header,
            rows: rows.to_vec(),
            findings: self.findings.clone(),
            observe: self.setup.observe.clone(),
        }
    }

    fn save(&mut self, report: &WalkReport) {
        let json = serde_json::to_string_pretty(report).unwrap_or_default();
        let files = [
            ("report.json", json),
            ("ledger-draft.md", render(report, self.file)),
        ];
        for (name, text) in files {
            let path = self.setup.observe.join(name);
            if let Err(e) = std::fs::write(&path, text) {
                let note = format!("{} not written: {e}", path.display());
                if !self.findings.contains(&note) {
                    self.findings.push(note);
                }
            }
        }
    }
}

/// Runs a walk: `console` is the endpoint, `input` the owner's typed lines, `out` the
/// screen, `shoot` the screen capture. Writes only under `setup.observe`.
pub fn run(
    file: &WalkFile,
    setup: &WalkSetup,
    console: &mut dyn Console,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    shoot: Shoot<'_>,
) -> Result<WalkReport> {
    file.check()?;
    // Every placeholder resolves before anything is sent.
    let missing: Vec<String> = file
        .editor_ids()
        .into_iter()
        .filter(|n| !setup.ids.contains_key(n))
        .collect();
    if !missing.is_empty() {
        return Err(GameError::Probes(format!(
            "no runtime ID for {} (is {} enabled and built from the recipe?)",
            missing.join(", "),
            file.plugin
        )));
    }
    std::fs::create_dir_all(&setup.observe).map_err(io(&setup.observe))?;
    let papyrus_mark = setup
        .papyrus_log
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len());
    let mut w = Walker {
        file,
        setup,
        console,
        input,
        out,
        shoot,
        corr: Correlator::default(),
        start: Instant::now(),
        started: std::time::SystemTime::now(),
        papyrus_mark,
        sfse: None,
        findings: Vec::new(),
    };
    w.say(&format!(
        "{} ({} stations)",
        file.title,
        file.stations.len()
    ));
    w.say(&format!("Recording into {}", setup.observe.display()));
    w.raw(&format!(
        "walk {} started; {} stations",
        setup.session,
        file.stations.len()
    ));
    if setup.papyrus_log.is_some() && papyrus_mark.is_none() {
        let note = "the Papyrus log was not readable at the start; the log rows read all of it if it appears".to_string();
        w.findings.push(note);
    }
    let mut rows: Vec<StationReport> = Vec::new();
    let mut stopped: Option<String> = None;
    for (n, station) in file.stations.iter().enumerate() {
        if let Some(why) = &stopped {
            rows.push(StationReport {
                row: RowReport {
                    row: station.id.clone(),
                    title: station.title.clone(),
                    forms: station.forms.clone(),
                    commands: Vec::new(),
                    expected: station.expected.clone(),
                    seen: String::new(),
                    verdict: Verdict::NotRun(why.clone()),
                    screenshots: Vec::new(),
                },
                covers: station.covers.clone(),
                judge: station.judge,
                owner: None,
                readings: Vec::new(),
            });
            continue;
        }
        let (row, quit) = w.station(n, station);
        if quit {
            stopped = Some(row.row.verdict.reason().to_string());
        }
        rows.push(row);
        let report = w.report(&rows);
        w.save(&report);
    }
    // The Papyrus log written during the walk, for the script rows.
    match w.papyrus_text() {
        Some(text) => {
            let path = setup.observe.join("papyrus-tail.log");
            if let Err(e) = std::fs::write(&path, text) {
                w.findings
                    .push(format!("{} not written: {e}", path.display()));
            }
        }
        None if setup.papyrus_log.is_some() => w
            .findings
            .push("the Papyrus log was not readable at the end: no papyrus-tail.log".into()),
        None => w
            .findings
            .push("no Papyrus log path was given: the log rows are inconclusive".into()),
    }
    let report = w.report(&rows);
    w.save(&report);
    w.say("");
    w.say(&format!("Walk {}: {}", setup.session, report.status.text()));
    w.say(&format!(
        "Draft ledger entry: {}",
        setup.observe.join("ledger-draft.md").display()
    ));
    Ok(report)
}

/// The draft ledger entry (`docs/game-observations.md` format, marked as a draft).
pub fn render(r: &WalkReport, file: &WalkFile) -> String {
    let mut s = String::new();
    let plugins = std::iter::once(file.plugin.as_str())
        .chain(file.also_record.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(", ");
    s.push_str(&format!(
        "\n### {}: owner walk {} (`kit walk`), {}\n\n",
        r.ledger_entry.as_deref().unwrap_or("game-draft"),
        r.session,
        plugins
    ));
    s.push_str(
        "DRAFT written by `kit walk`: give it the next free `game-<yyyymmdd>-<nn>` id (in `report.json` too) when entering it, and read each row's Seen before keeping a result.\n\n",
    );
    s.push_str("| Field | Content |\n| ----- | ------- |\n");
    let mut field = |k: &str, v: &str| s.push_str(&format!("| {k} | {} |\n", cell(v)));
    field(
        "Session",
        &format!(
            "`{}`, an owner sitting recorded by `kit walk` (the owner played; the recorder only read the console)",
            r.session
        ),
    );
    field("Status", &r.status.text());
    for (k, v) in &r.header {
        field(k, v);
    }
    field("Observe", &r.observe.display().to_string());
    s.push_str("\n| Row | Step | Forms | Expected | Seen | Result | Evidence |\n");
    s.push_str("| --- | ---- | ----- | -------- | ---- | ------ | -------- |\n");
    for row in &r.rows {
        let id = if row.covers.is_empty() {
            row.row.row.clone()
        } else {
            format!("{} ({})", row.row.row, row.covers.join(", "))
        };
        let result = match &row.row.verdict {
            Verdict::Pass => "**pass**".to_string(),
            v => v.text(),
        };
        let result = if row
            .owner
            .as_ref()
            .is_some_and(|a| matches!(a, Answer::Pass(_) | Answer::Fail(_)))
        {
            format!("{result} (owner report)")
        } else {
            result
        };
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            cell(&id),
            cell(&row.row.title),
            cell(&row.row.forms.join(", ")),
            cell(&row.row.expected),
            cell(&row.row.seen),
            cell(&result),
            cell(&row.row.screenshots.join(", "))
        ));
    }
    let answers: Vec<String> = r
        .rows
        .iter()
        .filter_map(|row| {
            let a = row.owner.as_ref()?;
            (!a.note().is_empty() || matches!(a, Answer::Pass(_) | Answer::Fail(_)))
                .then(|| format!("- {}: owner report: {}", row.row.row, a.text()))
        })
        .collect();
    if !answers.is_empty() {
        s.push_str("\nOwner's answers (owner report, verbatim):\n\n");
        for a in answers {
            s.push_str(&a);
            s.push('\n');
        }
    }
    if !r.findings.is_empty() {
        s.push('\n');
        for f in &r.findings {
            s.push_str(&format!("Finding: {f}\n"));
        }
    }
    for n in &file.notes {
        s.push_str(&format!("\nNote: {n}\n"));
    }
    s
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn answers_parse_by_their_first_word() {
        assert_eq!(Answer::parse("\n"), Answer::Done(String::new()));
        assert_eq!(
            Answer::parse("p the prompt read Steal\r\n"),
            Answer::Pass("the prompt read Steal".into())
        );
        assert_eq!(Answer::parse("F  no"), Answer::Fail("no".into()));
        assert_eq!(Answer::parse("s"), Answer::Skip(String::new()));
        assert_eq!(Answer::parse("q"), Answer::Quit(String::new()));
        assert_eq!(
            Answer::parse("looked odd"),
            Answer::Done("looked odd".into())
        );
    }

    #[test]
    fn placeholders_fill_or_name_what_is_missing() {
        let mut ids = BTreeMap::new();
        ids.insert("KKW10Misc".to_string(), "0B001000".to_string());
        assert_eq!(
            fill("player.getitemcount {id:KKW10Misc}", &ids).unwrap(),
            "player.getitemcount 0B001000"
        );
        assert!(fill("x {id:Nope}", &ids).unwrap_err().contains("Nope"));
        assert!(fill("x {id:Open", &ids).is_err());
    }

    #[test]
    fn saves_loads_and_moves_are_refused() {
        for c in [
            "save X",
            "load X",
            "coc Y",
            "qqq",
            "player.moveto 1",
            "bat x",
        ] {
            assert!(refused(c).is_some(), "{c}");
        }
        assert!(refused("player.additem 0000000F 1").is_none());
        assert!(refused("player.getitemcount 1").is_none());
    }

    #[test]
    fn a_watch_grades_its_change_and_missing_is_inconclusive() {
        let w = Watch {
            command: "g".into(),
            label: None,
            change: Some(1.0),
            tolerance: None,
            min: None,
            max: None,
            why: None,
        };
        let r = |v: Option<f64>| Reading {
            command: "g".into(),
            value: v,
            lines: Vec::new(),
            sends: 1,
            missing: v.is_none().then(|| "answer lost".into()),
        };
        assert_eq!(w.verdict(&r(Some(0.0)), &r(Some(1.0))), Verdict::Pass);
        assert_eq!(w.verdict(&r(Some(0.0)), &r(Some(2.0))).label(), "fail");
        assert_eq!(w.verdict(&r(None), &r(Some(1.0))).label(), "inconclusive");
        let range = Watch {
            change: None,
            min: Some(1.0),
            max: Some(10.0),
            ..w
        };
        assert_eq!(range.verdict(&r(Some(5.0)), &r(Some(9.0))), Verdict::Pass);
        assert_eq!(range.verdict(&r(Some(5.0)), &r(Some(5.0))).label(), "fail");
    }
}
