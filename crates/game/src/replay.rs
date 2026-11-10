//! A session's transcript and its replay at the desk (M4.T7, ruling R29).
//!
//! The controller records every command it sends and the full reply, in order, in the
//! session's `report.json` (`transcript`): the readiness polls, the base `load`, each step,
//! each retry and each readiness poll after a `load`. Before this, a session kept its replies
//! only as `seen`, each clipped to 120 characters, so no recorded session could be replayed.
//!
//! [`replay`] runs the real controller again over a transcript, with no game: a
//! [`ReplayConsole`] answers the k-th send with the k-th recorded reply when the command is
//! the recorded one, on a fixture MO2 instance and a fake host in a work folder. The
//! controller decides what to send from reply text alone (its pauses only wait), so a
//! replay sends what the session sent and grades with today's rules. A command that differs
//! from the recorded one, or a send past the transcript's end, is a divergence: the replay
//! stops agreeing with the session there, and every row is judged from then on as the
//! controller judges a refused command (`inconclusive`). [`compare`] then sets the replayed
//! labels against the ledger's.
//!
//! Limits: a row's screenshot is the fake host's, so a screenshot the live session failed to
//! take is not reproduced; a `papyrus-log` step reads the session's copied log as a whole
//! (written when the replay sends its first command); the replay waits nothing, so a live
//! retry cut short by its row's deadline can make the replay diverge.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::console::{Console, ConsoleError};
use crate::controller::{run, Deps, Plan, Shared};
use crate::error::Result;
use crate::probes::{ProbeFile, Verdict};
use crate::testing::{mo2_fixture, FakeHost};
use crate::GameProfile;

/// One command sent to the console and what came back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    /// The command as sent.
    pub command: String,
    /// The reply, in full.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
    /// The error instead of a reply (`timeout`, an unreachable endpoint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Milliseconds since the session started.
    pub elapsed_ms: u64,
}

/// The sends of a session, shared between the recording console and the report.
#[derive(Debug, Clone, Default)]
pub struct Transcript(Arc<Mutex<Vec<Sent>>>);

impl Transcript {
    /// Every send so far, in order.
    pub fn sends(&self) -> Vec<Sent> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn push(&self, s: Sent) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(s);
    }
}

/// A console that records every send of the one it wraps.
pub struct Recording {
    inner: Box<dyn Console>,
    log: Transcript,
    start: Instant,
}

impl Recording {
    /// Wraps `inner`; the sends go to `log`.
    pub fn new(inner: Box<dyn Console>, log: Transcript) -> Recording {
        Recording {
            inner,
            log,
            start: Instant::now(),
        }
    }
}

impl Console for Recording {
    fn run(
        &mut self,
        command: &str,
        timeout: Duration,
    ) -> std::result::Result<String, ConsoleError> {
        let answer = self.inner.run(command, timeout);
        self.log.push(Sent {
            command: command.to_string(),
            reply: answer.as_ref().ok().cloned(),
            error: answer.as_ref().err().map(ToString::to_string),
            elapsed_ms: u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX),
        });
        answer
    }
}

#[derive(Debug, Default)]
struct ReplayState {
    next: usize,
    diverged: Option<String>,
}

/// A console that answers from a transcript, send by send.
pub struct ReplayConsole {
    sends: Vec<Sent>,
    state: Arc<Mutex<ReplayState>>,
    first: Option<Box<dyn FnOnce() + Send>>,
}

impl Console for ReplayConsole {
    fn run(
        &mut self,
        command: &str,
        _timeout: Duration,
    ) -> std::result::Result<String, ConsoleError> {
        if let Some(f) = self.first.take() {
            f();
        }
        let mut st = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let k = st.next;
        let n = k.saturating_add(1);
        match self.sends.get(k) {
            Some(s) if s.command == command => {
                st.next = n;
                match (&s.reply, &s.error) {
                    (Some(r), _) => Ok(r.clone()),
                    (None, Some(e)) if e == "timeout" => Err(ConsoleError::Timeout),
                    (None, Some(e)) => Err(ConsoleError::Unreachable(e.clone())),
                    (None, None) => Ok(String::new()),
                }
            }
            Some(s) => {
                let why = format!("send {n}: sent `{command}`, recorded `{}`", s.command);
                st.diverged.get_or_insert(why.clone());
                Err(ConsoleError::Refused(format!("replay diverged at {why}")))
            }
            None => {
                let why = format!("send {n}: sent `{command}` after the transcript's last send");
                st.diverged.get_or_insert(why.clone());
                Err(ConsoleError::Refused(format!("replay diverged at {why}")))
            }
        }
    }
}

/// What a replay needs from a recorded session.
#[derive(Debug, Clone)]
pub struct Recorded<'a> {
    /// The session id (`2026-11-10-T01`).
    pub session: &'a str,
    /// The probes the session ran (today's, for a replay after a change).
    pub probes: &'a ProbeFile,
    /// Its transcript.
    pub transcript: &'a [Sent],
    /// The base save's name, as sent in `load`.
    pub base_save: &'a str,
    /// The session's own save name.
    pub session_save: &'a str,
    /// The session's Papyrus log as copied at its end, if any.
    pub papyrus: Option<&'a str>,
}

/// A replay's outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct Replayed {
    /// `(row, verdict)`, in the probes' order.
    pub rows: Vec<(String, Verdict)>,
    /// Where the replay stopped agreeing with the transcript, if it did.
    pub diverged: Option<String>,
    /// Sends of the transcript the replay never made.
    pub unused: usize,
    /// Why nothing was replayed (no transcript: a session recorded before M4.T7, or one that
    /// sent nothing).
    pub not_run: Option<String>,
}

/// Why a session with no transcript is not replayed (R29).
pub const NO_TRANSCRIPT: &str =
    "no transcript (a session recorded before M4.T7, or one that sent nothing)";

/// A row whose grade a deliberate grader change moved since the session (R29: listed by name
/// with a reason). One without a reason is not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regrade {
    /// The row.
    pub row: String,
    /// Its label under today's grader.
    pub now: String,
    /// Why it moved (the ruling or commit that moved it).
    pub why: String,
}

/// What a replay's comparison with the ledger found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every row grades as the ledger recorded it.
    Pass,
    /// What differs, one line each.
    Fail(Vec<String>),
    /// The replay did not reproduce the session's sends (a divergence, or recorded sends left
    /// unsent), so its grades say nothing about the ledger's (R29).
    Inconclusive(String),
    /// Nothing was replayed, with why: neither a pass nor a fail (R29).
    NotRun(String),
}

fn digest(b: &[u8]) -> String {
    // Change detection for the fixture's backups only (FNV-1a); never a recorded hash.
    let mut h: u64 = 1469598103934665603;
    for x in b {
        h = (h ^ u64::from(*x)).wrapping_mul(1099511628211);
    }
    format!("{h:016x}")
}

/// Replays `rec` in `work` (created, and its fixture removed after) with today's controller.
pub fn replay(rec: &Recorded<'_>, work: &Path) -> Result<Replayed> {
    // A session recorded before M4.T7 has no transcript, nor does a later one that failed
    // before it sent anything: nothing to answer from, so nothing
    // is sent and every row is `not-run` (R29; review M1 of M4.T7). Replaying it would only
    // diverge at the first send.
    if rec.transcript.is_empty() {
        return Ok(Replayed {
            rows: rec
                .probes
                .probes
                .iter()
                .map(|p| (p.row.clone(), Verdict::NotRun(NO_TRANSCRIPT.into())))
                .collect(),
            diverged: None,
            unused: 0,
            not_run: Some(NO_TRANSCRIPT.into()),
        });
    }
    let f = mo2_fixture(&work.join("mo2"))?;
    let mut game = GameProfile::named("starfield")?;
    game.launch.evidence = "replay".into();
    game.console.evidence = "replay".into();
    game.saves.evidence = "replay".into();
    // No game: nothing to wait for but the controller's own deadlines.
    game.timeouts.probe_secs = 8;
    game.timeouts.ready_secs = 10;
    game.timeouts.settle_secs = 0;
    game.timeouts.ready_poll_secs = 0;
    game.timeouts.load_secs = 10;
    game.timeouts.exit_secs = 1;
    game.timeouts.session_secs = 3600;
    let papyrus = work.join("papyrus.log");
    let mut probes = rec.probes.clone();
    for p in &mut probes.probes {
        for s in &mut p.steps {
            s.wait_secs = None;
        }
    }
    let backups = [
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
    let plan = Plan {
        session: rec.session.to_string(),
        date: "replay".into(),
        timeouts: game.timeouts.clone(),
        game,
        instance: f.instance.clone(),
        mod_manager: f.instance.join("ModOrganizer.exe"),
        profile: f.profile.clone(),
        profile_dir: f.profile_dir.clone(),
        observe: work.join("observe").join(rec.session),
        ledger: None,
        base_save: rec.base_save.to_string(),
        base_save_sha256: String::new(),
        session_save: rec.session_save.to_string(),
        probes,
        probes_path: work.join("probes.toml"),
        probes_sha256: String::new(),
        plugins: Vec::new(),
        backups,
        logs: vec![("papyrus".into(), papyrus.clone())],
        versions: vec![("Game build".into(), "replay".into())],
        stay: false,
        poll: Duration::from_millis(5),
    };
    let state = Arc::new(Mutex::new(ReplayState::default()));
    let text = rec.papyrus.map(str::to_string);
    let first: Option<Box<dyn FnOnce() + Send>> = text.map(|t| {
        let path = papyrus.clone();
        Box::new(move || {
            let _ = std::fs::write(&path, t);
        }) as Box<dyn FnOnce() + Send>
    });
    let console = ReplayConsole {
        sends: rec.transcript.to_vec(),
        state: Arc::clone(&state),
        first,
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
        host: host as Arc<dyn crate::host::Host>,
        console: Box::new(console),
        digest,
    };
    let report = run(&plan, deps, &Shared::new(&plan.session));
    let _ = std::fs::remove_dir_all(&f.root);
    let st = state.lock().unwrap_or_else(PoisonError::into_inner);
    Ok(Replayed {
        rows: report
            .rows
            .iter()
            .map(|r| (r.row.clone(), r.verdict.clone()))
            .collect(),
        diverged: st.diverged.clone(),
        unused: rec.transcript.len().saturating_sub(st.next),
        not_run: None,
    })
}

/// The ledger's labels against a replay's, row by row: `NotRun` when nothing was replayed;
/// `Inconclusive` when the replay did not make the session's sends (it diverged, or left
/// recorded sends unsent: R29, review L4); otherwise `Pass` when every row grades as recorded,
/// or `Fail` with a line for each row whose replayed label differs and each row only one side
/// has. `regraded` lists rows whose grade a deliberate grader change moved; such a row is
/// compared with its label now, and a re-grade with no reason is itself a difference (review
/// L3 of M4.T7).
pub fn compare(ledger: &[(String, String)], replayed: &Replayed, regraded: &[Regrade]) -> Outcome {
    if let Some(why) = &replayed.not_run {
        return Outcome::NotRun(why.clone());
    }
    if let Some(d) = &replayed.diverged {
        return Outcome::Inconclusive(format!("the replay diverged from the transcript at {d}"));
    }
    if replayed.unused > 0 {
        return Outcome::Inconclusive(format!(
            "the replay left {} recorded send(s) unsent",
            replayed.unused
        ));
    }
    let mut out = Vec::new();
    for g in regraded {
        if g.why.trim().is_empty() {
            out.push(format!(
                "{}: re-graded to {} with no reason (R29 wants one)",
                g.row, g.now
            ));
        }
    }
    for (row, recorded) in ledger {
        let want = regraded
            .iter()
            .find(|g| &g.row == row && !g.why.trim().is_empty())
            .map_or(recorded.as_str(), |g| g.now.as_str());
        match replayed.rows.iter().find(|(r, _)| r == row) {
            Some((_, v)) if v.label() == want => {}
            Some((_, v)) => out.push(format!(
                "{row}: the ledger says {want}, the replay grades {}",
                v.text()
            )),
            None => out.push(format!("{row}: in the ledger, not in the replay")),
        }
    }
    for (row, v) in &replayed.rows {
        if !ledger.iter().any(|(r, _)| r == row) {
            out.push(format!(
                "{row}: graded {} by the replay, not in the ledger",
                v.label()
            ));
        }
    }
    if out.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Fail(out)
    }
}
