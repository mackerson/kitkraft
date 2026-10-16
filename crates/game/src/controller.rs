//! The session controller (M4.T3, with ADR 0010 Amendment 1 delivered as M5.C1).
//!
//! One session, in order: back up the dev profile's list files, INIs and the base save
//! into `observe\<session>\backup\`; start the mod manager with the game profile's command
//! line; wait for the console endpoint; load the base save; run the probes in order, each
//! with its own deadline and the session's wall-clock budget over all; (with `stay`, serve
//! `game.probe` re-runs until stopped); end the game and the processes the session
//! started; copy the logs; compare the backed-up files' hashes and restore any that
//! changed (the session is then `inconclusive`); append the ledger entry; write
//! `report.json`. Every write goes through [`Authorized`].
//!
//! A probe that times out is `inconclusive (timeout ...)`, the rows after it are
//! `not-run (timeout at row X)`, and the game is ended. `game.stop` makes the remaining
//! rows `not-run (stopped before row X)`. A `save` or `load` to a name the session does not
//! own, and `bat`, `saveini`, `qqq` and `quitgame`, are refused before they are sent.
//!
//! [`run`] is synchronous; the engine runs it inline (the one-shot CLI) or on a thread it
//! owns, following it through [`Shared`].

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::authorized::Authorized;
use crate::backup::{self, Backup, Change, Digest};
use crate::console::{Console, ConsoleError};
use crate::host::{is_running, Host, Launched};
use crate::ledger::{self, Entry, RowReport};
use crate::probes::{clip, judge, Probe, ProbeFile, Verdict};
use crate::profile::{fill, GameProfile, Timeouts};

/// Commands the controller never sends: a batch file would bypass the save rules, and the
/// others write the INIs or quit on their own.
pub const REFUSED_COMMANDS: &[&str] = &["bat", "saveini", "qqq", "quitgame"];

/// Everything one session needs, decided before it starts (the dry run prints it).
#[derive(Debug, Clone)]
pub struct Plan {
    /// `yyyy-mm-dd-T<nn>`, also the observe folder's name.
    pub session: String,
    /// yyyy-mm-dd.
    pub date: String,
    /// The game profile.
    pub game: GameProfile,
    /// The MO2 instance root.
    pub instance: PathBuf,
    /// The mod manager executable in it.
    pub mod_manager: PathBuf,
    /// The MO2 profile name, as its folder is spelled.
    pub profile: String,
    /// The profile folder.
    pub profile_dir: PathBuf,
    /// `observe\<session>\`.
    pub observe: PathBuf,
    /// The ledger to append to (`None`: no entry, e.g. a test).
    pub ledger: Option<PathBuf>,
    /// The base save's name (no extension).
    pub base_save: String,
    /// Its SHA-256, as proven by preflight.
    pub base_save_sha256: String,
    /// The session's own save name (`KitKraftSession-<yyyymmdd>-<nn>`).
    pub session_save: String,
    /// The probes.
    pub probes: ProbeFile,
    /// Where they came from.
    pub probes_path: PathBuf,
    /// Their SHA-256.
    pub probes_sha256: String,
    /// `(plugin, SHA-256)` of the plugins under test.
    pub plugins: Vec<(String, String)>,
    /// The profile files and the base save, backed up and verified.
    pub backups: Vec<PathBuf>,
    /// `(name, path)` of the logs copied at the end.
    pub logs: Vec<(String, PathBuf)>,
    /// Budgets.
    pub timeouts: Timeouts,
    /// `(what, version)` for the ledger header.
    pub versions: Vec<(String, String)>,
    /// Keep the session open after the probes, serving `game.probe`, until stopped.
    pub stay: bool,
    /// How often to poll the endpoint and the request queue.
    pub poll: Duration,
}

impl Plan {
    /// The mod manager's arguments.
    pub fn launch_args(&self) -> Vec<String> {
        self.game.launch_args(&self.profile)
    }

    /// The write guard.
    pub fn authorized(&self) -> Authorized {
        Authorized::new(&self.observe, self.ledger.as_deref(), &self.backups)
    }

    /// The plan as JSON (what `--dry-run` prints).
    pub fn describe(&self) -> Value {
        let rows: Vec<Value> = self
            .probes
            .probes
            .iter()
            .map(|p| {
                json!({
                    "row": p.row, "title": p.title, "manual": p.reason.clone().filter(|_| p.is_manual()),
                    "screenshot": p.screenshot,
                    "commands": p.steps.iter().map(|s| self.fill(&s.command)).filter(|c| !c.is_empty()).collect::<Vec<_>>(),
                })
            })
            .collect();
        json!({
            "session": self.session,
            "game": self.game.game.id,
            "instance": self.instance.display().to_string(),
            "profile": self.profile,
            "launch": {
                "program": self.mod_manager.display().to_string(),
                "args": self.launch_args(),
                "evidence": self.game.launch.evidence,
            },
            "console": self.game.console.url,
            "base_save": {"name": self.base_save, "sha256": self.base_save_sha256},
            "session_save": self.session_save,
            "probes": {"path": self.probes_path.display().to_string(), "sha256": self.probes_sha256, "rows": rows},
            "plugins": self.plugins.iter().map(|(n, h)| json!({"name": n, "sha256": h})).collect::<Vec<_>>(),
            "backups": self.backups.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "logs": self.logs.iter().map(|(n, p)| json!({"name": n, "path": p.display().to_string()})).collect::<Vec<_>>(),
            "ends": self.game.executables.ended(),
            "timeouts": self.timeouts,
            "authorized_writes": self.authorized().describe(),
            "ledger": self.ledger.as_ref().map(|p| p.display().to_string()),
        })
    }

    fn fill(&self, command: &str) -> String {
        fill(
            command,
            &[("save", &self.session_save), ("base_save", &self.base_save)],
        )
    }

    /// Why `command` may not be sent, if it may not.
    pub fn refuse(&self, command: &str) -> Option<String> {
        let mut words = command.split_whitespace();
        let verb = words.next().unwrap_or_default().to_ascii_lowercase();
        let arg = words.collect::<Vec<_>>().join(" ");
        let arg = arg.trim().trim_matches('"');
        if REFUSED_COMMANDS.contains(&verb.as_str()) {
            return Some(format!("`{verb}` is never sent by the controller"));
        }
        let save_verb = first_word(&self.game.saves.save_command);
        let load_verb = first_word(&self.game.saves.load_command);
        let prefix = &self.game.saves.session_prefix;
        if verb == save_verb && !arg.starts_with(prefix.as_str()) {
            return Some(format!(
                "`{command}`: the session saves only to `{prefix}*` names"
            ));
        }
        if verb == load_verb && !(arg.starts_with(prefix.as_str()) || arg == self.base_save) {
            return Some(format!(
                "`{command}`: the session loads only the base save `{}` or its own `{prefix}*` saves",
                self.base_save
            ));
        }
        None
    }
}

fn first_word(s: &str) -> String {
    s.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Where a session is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Not started.
    Pending,
    /// Copying the profile files.
    BackingUp,
    /// Starting the mod manager.
    Launching,
    /// Waiting for the console endpoint.
    WaitingForConsole,
    /// Loading the base save.
    LoadingSave,
    /// Running the probes.
    Probing,
    /// Probes done; serving `game.probe` until stopped.
    Idle,
    /// Ending the game, copying logs, verifying, writing the ledger.
    Stopping,
    /// Done; the report is in.
    Finished,
}

/// The end of a session.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// The session id.
    pub session: String,
    /// The session's status.
    pub status: Verdict,
    /// Every probe row.
    pub rows: Vec<RowReport>,
    /// Rows re-run with `game.probe`.
    pub reruns: Vec<RowReport>,
    /// Profile files that changed (restored when they existed before).
    pub changes: Vec<Change>,
    /// The backups taken.
    pub backups: Vec<Backup>,
    /// Findings (for the ledger and `plan/STATUS.md` "Requests").
    pub findings: Vec<String>,
    /// The processes ended.
    pub terminated: Vec<String>,
    /// `(log, copy)`: `None` when the log was not written.
    pub logs: Vec<(String, Option<PathBuf>)>,
    /// The ledger entry id, when one was appended.
    pub ledger_entry: Option<String>,
    /// The entry's text.
    pub ledger_text: String,
    /// Every path the controller wrote.
    pub written: Vec<PathBuf>,
}

/// What `game.status` shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    /// The session id.
    pub session: String,
    /// Where it is.
    pub phase: Phase,
    /// The row running now.
    pub current_row: Option<String>,
    /// Rows done so far.
    pub rows: Vec<RowReport>,
    /// Rows re-run with `game.probe`.
    pub reruns: Vec<RowReport>,
    /// Screenshots taken (paths under the observe folder).
    pub screenshots: Vec<PathBuf>,
    /// What happened, in order.
    pub events: Vec<String>,
    /// Seconds since the start.
    pub elapsed_secs: u64,
    /// The end report, once finished.
    pub report: Option<Report>,
}

enum Request {
    Probe {
        row: String,
        screenshot: bool,
        reply: mpsc::Sender<RowReport>,
    },
}

/// The state a running session shares with `game.status`, `game.probe` and `game.stop`.
pub struct Shared {
    snap: Mutex<Snapshot>,
    started: Instant,
    stop: AtomicBool,
    requests: Mutex<VecDeque<Request>>,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("stop", &self.stop.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl Shared {
    /// A new, pending session.
    pub fn new(session: &str) -> Arc<Shared> {
        Arc::new(Shared {
            snap: Mutex::new(Snapshot {
                session: session.to_string(),
                phase: Phase::Pending,
                current_row: None,
                rows: Vec::new(),
                reruns: Vec::new(),
                screenshots: Vec::new(),
                events: Vec::new(),
                elapsed_secs: 0,
                report: None,
            }),
            started: Instant::now(),
            stop: AtomicBool::new(false),
            requests: Mutex::new(VecDeque::new()),
        })
    }

    fn with<R>(&self, f: impl FnOnce(&mut Snapshot) -> R) -> R {
        let mut s = self.snap.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut s)
    }

    /// The state now.
    pub fn snapshot(&self) -> Snapshot {
        let elapsed = self.started.elapsed().as_secs();
        self.with(|s| {
            s.elapsed_secs = elapsed;
            s.clone()
        })
    }

    /// Asks the session to end (after the command in flight).
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Whether a stop was asked for.
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Queues a re-run of `row` for a session in [`Phase::Idle`] and waits for it.
    pub fn probe(&self, row: &str, screenshot: bool, wait: Duration) -> Result<RowReport, String> {
        let phase = self.with(|s| s.phase);
        if !matches!(phase, Phase::Idle | Phase::Probing) {
            return Err(format!(
                "the session is {}; probes are re-run only while it is open (started with start, probes done)",
                serde_json::to_value(phase)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default()
            ));
        }
        let (tx, rx) = mpsc::channel();
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(Request::Probe {
                row: row.to_string(),
                screenshot,
                reply: tx,
            });
        rx.recv_timeout(wait)
            .map_err(|_| format!("no answer for row {row} within {}s", wait.as_secs()))
    }

    fn phase(&self, phase: Phase, event: &str) {
        self.with(|s| {
            s.phase = phase;
            if !event.is_empty() {
                s.events.push(event.to_string());
            }
        });
    }

    fn event(&self, event: String) {
        self.with(|s| s.events.push(event));
    }

    fn next_request(&self) -> Option<Request> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
    }
}

/// The operating system and the console, for one session.
pub struct Deps {
    /// Processes and screenshots.
    pub host: Arc<dyn Host>,
    /// The console channel.
    pub console: Box<dyn Console>,
    /// SHA-256.
    pub digest: Digest,
}

impl std::fmt::Debug for Deps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deps").finish_non_exhaustive()
    }
}

struct Run<'a> {
    plan: &'a Plan,
    deps: Deps,
    shared: &'a Shared,
    guard: Authorized,
    start: Instant,
    papyrus: Option<(PathBuf, u64)>,
    findings: Vec<String>,
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

impl Run<'_> {
    fn budget_left(&self) -> Duration {
        secs(self.plan.timeouts.session_secs).saturating_sub(self.start.elapsed())
    }

    /// Polls the readiness command until it answers (and contains the expected text).
    fn wait_ready(&mut self, limit: Duration, what: &str) -> Result<(), String> {
        let started = Instant::now();
        let limit = limit.min(self.budget_left());
        let cmd = self.plan.game.console.ready_command.clone();
        let expect = self.plan.game.console.ready_expect.to_ascii_lowercase();
        let mut last = String::from("no answer");
        loop {
            if self.shared.stop_requested() {
                return Err(format!("stopped while waiting for {what}"));
            }
            let left = limit.saturating_sub(started.elapsed());
            if left.is_zero() {
                return Err(format!(
                    "timeout after {}s waiting for {what} ({last})",
                    limit.as_secs()
                ));
            }
            match self.deps.console.run(&cmd, left.min(secs(5))) {
                Ok(out) if expect.is_empty() || out.to_ascii_lowercase().contains(&expect) => {
                    self.shared
                        .event(format!("{what}: `{cmd}` answered `{}`", clip(&out, 80)));
                    return Ok(());
                }
                Ok(out) => last = format!("answer `{}`", clip(&out, 80)),
                Err(e) => last = e.to_string(),
            }
            std::thread::sleep(self.plan.poll.min(limit.saturating_sub(started.elapsed())));
        }
    }

    fn papyrus_text(&self) -> Option<String> {
        let (path, from) = self.papyrus.as_ref()?;
        let bytes = std::fs::read(path).ok()?;
        let from = usize::try_from(*from).unwrap_or(0);
        let tail = bytes.get(from..).unwrap_or(&bytes);
        Some(String::from_utf8_lossy(tail).into_owned())
    }

    fn screenshot(&mut self, row: &str, report: &mut RowReport) -> Option<Verdict> {
        let n = report.screenshots.len().saturating_add(1);
        let name = format!("{row}-{n}.png");
        let dest = self.plan.observe.join("screenshots").join(&name);
        let taken = self
            .guard
            .claim(&dest)
            .and_then(|()| self.deps.host.screenshot(&dest));
        match taken {
            Ok(()) if dest.is_file() => {
                report.screenshots.push(name);
                self.shared.with(|s| s.screenshots.push(dest.clone()));
                None
            }
            Ok(()) => Some(Verdict::Inconclusive("screenshot not written".into())),
            Err(e) => Some(Verdict::Inconclusive(format!("screenshot not taken: {e}"))),
        }
    }

    fn run_probe(&mut self, probe: &Probe, screenshot: bool) -> RowReport {
        let expected: Vec<String> = probe
            .steps
            .iter()
            .filter_map(|s| {
                let m = serde_json::to_value(s.matcher?).ok()?;
                Some(format!(
                    "{} {}{}",
                    m.as_str().unwrap_or_default(),
                    s.expected.as_deref().unwrap_or_default(),
                    s.tolerance.map(|t| format!(" +/- {t}")).unwrap_or_default()
                ))
            })
            .collect();
        let mut report = RowReport {
            row: probe.row.clone(),
            title: probe.title.clone(),
            forms: probe.forms.clone(),
            commands: Vec::new(),
            expected: if probe.is_manual() {
                probe.title.clone()
            } else {
                expected.join("; ")
            },
            seen: String::new(),
            verdict: Verdict::NotRun("not started".into()),
            screenshots: Vec::new(),
        };
        self.shared
            .with(|s| s.current_row = Some(probe.row.clone()));
        if probe.is_manual() {
            let reason = probe.reason.clone().unwrap_or_default();
            report.verdict = if screenshot {
                match self.screenshot(&probe.row, &mut report) {
                    None => Verdict::Inconclusive(format!(
                        "manual: {reason}; screenshot {} is for the owner",
                        report.screenshots.join(", ")
                    )),
                    Some(v) => Verdict::NotRun(format!("manual: {reason}; {}", v.reason())),
                }
            } else {
                Verdict::NotRun(format!("manual: {reason}"))
            };
            return report;
        }
        let deadline = secs(self.plan.timeouts.probe_secs).min(self.budget_left());
        let started = Instant::now();
        let mut verdicts = Vec::new();
        let mut seen = Vec::new();
        for step in &probe.steps {
            let cmd = self.plan.fill(&step.command);
            if cmd.trim().is_empty() {
                verdicts.push(judge(step, None, self.papyrus_text().as_deref()));
                continue;
            }
            if let Some(why) = self.plan.refuse(&cmd) {
                verdicts.push(Verdict::NotRun(format!("refused: {why}")));
                break;
            }
            let left = deadline.saturating_sub(started.elapsed());
            if left.is_zero() {
                verdicts.push(Verdict::Inconclusive(format!(
                    "timeout: the row's {}s ran out before `{cmd}`",
                    deadline.as_secs()
                )));
                break;
            }
            report.commands.push(cmd.clone());
            match self.deps.console.run(&cmd, left) {
                Ok(out) => {
                    seen.push(format!("{cmd} -> {}", clip(&out, 120)));
                    if first_word(&cmd) == first_word(&self.plan.game.saves.load_command) {
                        let limit = secs(self.plan.timeouts.load_secs);
                        if let Err(e) = self.wait_ready(limit, "the save to load") {
                            verdicts.push(Verdict::Inconclusive(format!("timeout: {e}")));
                            break;
                        }
                    }
                    verdicts.push(judge(step, Some(&out), self.papyrus_text().as_deref()));
                }
                Err(ConsoleError::Timeout) => {
                    seen.push(format!("{cmd} -> (no answer)"));
                    verdicts.push(Verdict::Inconclusive(format!(
                        "timeout: no answer to `{cmd}` within the row's {}s",
                        deadline.as_secs()
                    )));
                    break;
                }
                Err(e) => {
                    seen.push(format!("{cmd} -> ({e})"));
                    verdicts.push(Verdict::Inconclusive(format!("`{cmd}`: {e}")));
                    break;
                }
            }
        }
        if screenshot {
            if let Some(v) = self.screenshot(&probe.row, &mut report) {
                verdicts.push(v);
            }
        }
        report.seen = seen.join("; ");
        report.verdict = Verdict::combine(&verdicts);
        report
    }

    fn not_run(&self, from: usize, why: &str) -> Vec<RowReport> {
        self.plan
            .probes
            .probes
            .iter()
            .skip(from)
            .map(|p| RowReport {
                row: p.row.clone(),
                title: p.title.clone(),
                forms: p.forms.clone(),
                commands: Vec::new(),
                expected: String::new(),
                seen: String::new(),
                verdict: Verdict::NotRun(why.to_string()),
                screenshots: Vec::new(),
            })
            .collect()
    }

    fn push_row(&self, rows: &mut Vec<RowReport>, r: RowReport) {
        self.shared.with(|s| {
            s.rows.push(r.clone());
            s.current_row = None;
        });
        rows.push(r);
    }

    /// The probes in order; returns whether the session may stay open.
    fn probes(&mut self, rows: &mut Vec<RowReport>) -> bool {
        let probes = self.plan.probes.probes.clone();
        for (i, probe) in probes.iter().enumerate() {
            if self.shared.stop_requested() {
                let why = format!("stopped by game.stop before row {}", probe.row);
                self.findings.push(why.clone());
                for r in self.not_run(i, &why) {
                    self.push_row(rows, r);
                }
                return false;
            }
            if self.budget_left().is_zero() {
                let why = format!(
                    "timeout at row {} (session budget {}s)",
                    probe.row, self.plan.timeouts.session_secs
                );
                self.findings.push(why.clone());
                for r in self.not_run(i, &why) {
                    self.push_row(rows, r);
                }
                return false;
            }
            let r = self.run_probe(probe, probe.screenshot);
            let timed_out =
                matches!(&r.verdict, Verdict::Inconclusive(why) if why.starts_with("timeout"));
            self.shared
                .event(format!("row {}: {}", r.row, r.verdict.text()));
            self.push_row(rows, r);
            if timed_out {
                let why = format!("timeout at row {}", probe.row);
                self.findings
                    .push(format!("row {} timed out; the game was ended", probe.row));
                for r in self.not_run(i.saturating_add(1), &why) {
                    self.push_row(rows, r);
                }
                return false;
            }
        }
        true
    }

    fn idle(&mut self) -> Vec<RowReport> {
        self.shared.phase(
            Phase::Idle,
            "probes done; open for game.probe until game.stop",
        );
        let mut reruns = Vec::new();
        loop {
            if self.shared.stop_requested() {
                break;
            }
            if self.budget_left().is_zero() {
                self.findings.push(format!(
                    "the session budget ({}s) ended the open session",
                    self.plan.timeouts.session_secs
                ));
                break;
            }
            match self.shared.next_request() {
                Some(Request::Probe {
                    row,
                    screenshot,
                    reply,
                }) => {
                    let probe = self
                        .plan
                        .probes
                        .probes
                        .iter()
                        .find(|p| p.row == row)
                        .cloned();
                    let r = match probe {
                        Some(p) => self.run_probe(&p, screenshot || p.screenshot),
                        None => RowReport {
                            row: row.clone(),
                            title: String::new(),
                            forms: Vec::new(),
                            commands: Vec::new(),
                            expected: String::new(),
                            seen: String::new(),
                            verdict: Verdict::NotRun(format!("no row {row} in the probes")),
                            screenshots: Vec::new(),
                        },
                    };
                    self.shared.with(|s| {
                        s.reruns.push(r.clone());
                        s.current_row = None;
                    });
                    reruns.push(r.clone());
                    let _ = reply.send(r);
                }
                None => std::thread::sleep(self.plan.poll),
            }
        }
        reruns
    }

    /// Ends the processes and waits for them to go.
    fn end(&mut self, child: Option<Box<dyn Launched>>) -> Vec<String> {
        let images = self.plan.game.executables.ended();
        let mut ended = match self.deps.host.terminate(&images) {
            Ok(e) => e,
            Err(e) => {
                self.findings.push(format!("ending the game failed: {e}"));
                Vec::new()
            }
        };
        if let Some(mut c) = child {
            if c.kill().is_ok() {
                ended.push(format!("pid {}", c.id()));
            }
        }
        let started = Instant::now();
        let limit = secs(self.plan.timeouts.exit_secs);
        loop {
            match self.deps.host.running() {
                Ok(r) if !images.iter().any(|i| is_running(&r, i)) => break,
                Ok(r) if started.elapsed() >= limit => {
                    let left: Vec<&String> = images.iter().filter(|i| is_running(&r, i)).collect();
                    self.findings.push(format!(
                        "still running after {}s: {left:?}",
                        limit.as_secs()
                    ));
                    break;
                }
                Ok(_) => std::thread::sleep(self.plan.poll),
                Err(e) => {
                    self.findings
                        .push(format!("could not list processes after ending: {e}"));
                    break;
                }
            }
        }
        ended
    }

    fn copy_logs(&mut self) -> Vec<(String, Option<PathBuf>)> {
        let mut out = Vec::new();
        for (name, path) in &self.plan.logs {
            if !path.is_file() {
                out.push((name.clone(), None));
                continue;
            }
            let file = path
                .file_name()
                .map_or_else(|| "log".to_string(), |n| n.to_string_lossy().into_owned());
            let dest = self
                .plan
                .observe
                .join("logs")
                .join(format!("{name}-{file}"));
            match self.guard.copy_in(path, &dest) {
                Ok(_) => out.push((name.clone(), Some(dest))),
                Err(e) => {
                    self.findings.push(format!("log {name} not copied: {e}"));
                    out.push((name.clone(), None));
                }
            }
        }
        out
    }
}

/// Runs a whole session. Never panics; every failure becomes a row status or a finding.
pub fn run(plan: &Plan, deps: Deps, shared: &Shared) -> Report {
    let papyrus = plan
        .logs
        .iter()
        .find(|(n, _)| n == "papyrus")
        .map(|(_, p)| {
            let len = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            (p.clone(), len)
        });
    let mut r = Run {
        plan,
        deps,
        shared,
        guard: plan.authorized(),
        start: Instant::now(),
        papyrus,
        findings: Vec::new(),
    };
    let mut rows: Vec<RowReport> = Vec::new();
    let mut reruns = Vec::new();
    shared.phase(
        Phase::BackingUp,
        "backing up the profile files and the base save",
    );
    let backups = r.guard.create_dir_all(&plan.observe).and_then(|()| {
        backup::take(
            &plan.backups,
            &plan.observe.join("backup"),
            &r.guard,
            r.deps.digest,
        )
    });
    let mut terminated = Vec::new();
    let backups = match backups {
        Ok(b) => {
            shared.phase(Phase::Launching, "starting the game through MO2");
            let launched =
                r.deps
                    .host
                    .launch(&plan.mod_manager, &plan.launch_args(), &plan.instance);
            match launched {
                Err(e) => {
                    r.findings.push(format!("launch failed: {e}"));
                    for x in r.not_run(0, &format!("launch failed: {e}")) {
                        r.push_row(&mut rows, x);
                    }
                }
                Ok(child) => {
                    shared.phase(Phase::WaitingForConsole, "waiting for the console endpoint");
                    let ready =
                        r.wait_ready(secs(plan.timeouts.ready_secs), "the console endpoint");
                    let loaded = ready.and_then(|()| {
                        shared.phase(Phase::LoadingSave, "loading the base save");
                        let cmd = fill(&plan.game.saves.load_command, &[("save", &plan.base_save)]);
                        let limit = secs(plan.timeouts.load_secs);
                        let _ = r.deps.console.run(&cmd, limit.min(secs(30)));
                        r.wait_ready(limit, "the base save to load")
                    });
                    match loaded {
                        Err(why) => {
                            r.findings.push(why.clone());
                            for x in r.not_run(0, &why) {
                                r.push_row(&mut rows, x);
                            }
                        }
                        Ok(()) => {
                            shared.phase(Phase::Probing, "running the probes");
                            let open = r.probes(&mut rows);
                            if open && plan.stay {
                                reruns = r.idle();
                            }
                        }
                    }
                    shared.phase(Phase::Stopping, "ending the game");
                    terminated = r.end(Some(child));
                }
            }
            b
        }
        Err(e) => {
            r.findings
                .push(format!("backup failed, nothing launched: {e}"));
            for x in r.not_run(0, &format!("backup failed: {e}")) {
                r.push_row(&mut rows, x);
            }
            Vec::new()
        }
    };
    shared.phase(Phase::Stopping, "copying logs and verifying the profile");
    let logs = r.copy_logs();
    let mut changes = backup::verify(&backups, r.deps.digest).unwrap_or_else(|e| {
        r.findings
            .push(format!("verifying the profile failed: {e}"));
        Vec::new()
    });
    if !changes.is_empty() {
        backup::restore(&backups, &mut changes, &r.guard, r.deps.digest);
        for c in &changes {
            r.findings.push(format!(
                "{} changed during the session ({} -> {}): {}",
                c.path.display(),
                c.before.as_deref().unwrap_or("absent"),
                c.after.as_deref().unwrap_or("absent"),
                c.note
            ));
        }
    }
    let status = if changes.is_empty() {
        Verdict::combine(&rows.iter().map(|x| x.verdict.clone()).collect::<Vec<_>>())
    } else {
        Verdict::Inconclusive(format!(
            "{} profile file(s) changed during the session and were restored or reported",
            changes.len()
        ))
    };
    let mut entry = Entry {
        id: String::new(),
        session: plan.session.clone(),
        date: plan.date.clone(),
        status: status.clone(),
        versions: plan.versions.clone(),
        profile: plan.profile.clone(),
        instance: plan.instance.display().to_string(),
        base_save: (plan.base_save.clone(), plan.base_save_sha256.clone()),
        session_save: plan.session_save.clone(),
        plugins: plan.plugins.clone(),
        probes: (
            plan.probes_path.display().to_string(),
            plan.probes_sha256.clone(),
        ),
        observe: plan.observe.display().to_string(),
        rows: rows.clone(),
        findings: r.findings.clone(),
    };
    let mut ledger_entry = None;
    if let Some(path) = &plan.ledger {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        entry.id = ledger::next_id(&existing, &plan.date);
        let text = ledger::render(&entry);
        match r.guard.append_ledger(path, &text) {
            Ok(()) => ledger_entry = Some(entry.id.clone()),
            Err(e) => r.findings.push(format!("ledger entry not written: {e}")),
        }
    } else {
        entry.id = format!("game-{}-xx", ledger::compact(&plan.date));
    }
    let ledger_text = ledger::render(&entry);
    let mut report = Report {
        session: plan.session.clone(),
        status,
        rows,
        reruns,
        changes,
        backups,
        findings: r.findings.clone(),
        terminated,
        logs,
        ledger_entry,
        ledger_text,
        written: Vec::new(),
    };
    let report_path = plan.observe.join("report.json");
    if let Ok(text) = serde_json::to_string_pretty(&report) {
        if let Err(e) = r.guard.write(&report_path, text.as_bytes()) {
            report
                .findings
                .push(format!("report.json not written: {e}"));
        }
    }
    report.written = r.guard.written();
    shared.with(|s| {
        s.phase = Phase::Finished;
        s.current_row = None;
        s.events.push(format!("finished: {}", report.status.text()));
        s.report = Some(report.clone());
    });
    report
}

/// The first free session id and save name for `date` (yyyy-mm-dd): `yyyy-mm-dd-T<nn>`
/// (no folder of that name under `observe_root`, not in `taken`) and
/// `<prefix><yyyymmdd>-<nn>`.
pub fn next_session(
    observe_root: &Path,
    date: &str,
    prefix: &str,
    taken: &[String],
) -> (String, String) {
    let mut n: u32 = 1;
    loop {
        let id = format!("{date}-T{n:02}");
        if !observe_root.join(&id).exists() && !taken.contains(&id) || n >= 99 {
            return (id, format!("{prefix}{}-{n:02}", ledger::compact(date)));
        }
        n = n.saturating_add(1);
    }
}

/// Today in UTC as `yyyy-mm-dd`.
pub fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    civil(secs).map_or_else(
        || "1970-01-01".into(),
        |(y, m, d)| format!("{y:04}-{m:02}-{d:02}"),
    )
}

/// The UTC day `secs` after the epoch (days-to-civil, for dates on or after 1970).
pub fn civil(secs: u64) -> Option<(u64, u64, u64)> {
    let z = secs.checked_div(86_400)?.checked_add(719_468)?;
    let era = z.checked_div(146_097)?;
    let doe = z.checked_sub(era.checked_mul(146_097)?)?;
    let yoe = doe
        .checked_sub(doe.checked_div(1460)?)?
        .checked_add(doe.checked_div(36_524)?)?
        .checked_sub(doe.checked_div(146_096)?)?
        .checked_div(365)?;
    let doy = doe.checked_sub(
        yoe.checked_mul(365)?
            .checked_add(yoe.checked_div(4)?)?
            .checked_sub(yoe.checked_div(100)?)?,
    )?;
    let mp = doy.checked_mul(5)?.checked_add(2)?.checked_div(153)?;
    let day = doy
        .checked_sub(mp.checked_mul(153)?.checked_add(2)?.checked_div(5)?)?
        .checked_add(1)?;
    let month = if mp < 10 {
        mp.checked_add(3)?
    } else {
        mp.checked_sub(9)?
    };
    let year = yoe.checked_add(era.checked_mul(400)?)?;
    let year = if month <= 2 {
        year.checked_add(1)?
    } else {
        year
    };
    Some((year, month, day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_and_session_ids() {
        assert_eq!(civil(0), Some((1970, 1, 1)));
        assert_eq!(civil(1_792_108_800), Some((2026, 10, 16)));
        let root = std::env::temp_dir().join(format!("kitkraft-game-ids-{}", std::process::id()));
        std::fs::create_dir_all(root.join("2026-10-16-T01")).unwrap();
        let (id, save) = next_session(
            &root,
            "2026-10-16",
            "KitKraftSession-",
            &["2026-10-16-T02".into()],
        );
        assert_eq!(id, "2026-10-16-T03");
        assert_eq!(save, "KitKraftSession-20261016-03");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
