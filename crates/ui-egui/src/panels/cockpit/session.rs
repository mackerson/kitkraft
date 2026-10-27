//! The game session (M6.G4, second half; ADR 0007 Amendment 3): the form, the state
//! the window follows, and its drawing (below the `// ---- drawing` line).
//!
//! The gate lets the window start a session with `game.session` in its writing form
//! (`start: true`, `dry_run` absent) only as a confirmed request, follow it with
//! `game.status` (allowed) and end it with `game.stop` only as a confirmed request
//! (its preview is `dry_run: true`). This file is the one panel that builds the
//! confirmed `game.session` and `game.stop` requests (the gate's source scan holds it):
//! [`SessionForm::start_request`] for the start, [`SessionForm::stop_request`] for the
//! stop. The window never launches the game itself: every request goes through the one
//! engine worker (`crate::worker`); a poll is one ordinary `game.status` on it, never a
//! second worker, thread or process.

use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use crate::gate::Request;

/// The interval between two `game.status` polls: one per second (M6.G4).
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The session form: where the probes and the game come from, which profile runs them,
/// and where the session writes (empty fields stay out of the arguments; the engine
/// fills the rest with its defaults).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionForm {
    /// The MO2 instance root.
    pub mo2: String,
    /// The profile name (the engine's default is `KitKraft Dev`; never `Default`).
    pub profile: String,
    /// The probes file: the session's rows.
    pub probes: String,
    /// The SHA-256 of the base save, as the engine checks it.
    pub base_save_sha256: String,
    /// The game's folder (its launcher and console settings).
    pub game: String,
    /// The folder the session writes its rows, screenshots and logs into.
    pub observe: String,
    /// The ledger file the session appends its entry to.
    pub ledger: String,
}

impl SessionForm {
    /// The form from a request's arguments (absent or non-string members stay empty).
    pub fn from_args(args: &Value) -> Self {
        let get = |k: &str| {
            args.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        Self {
            mo2: get("mo2"),
            profile: get("profile"),
            probes: get("probes"),
            base_save_sha256: get("base_save_sha256"),
            game: get("game"),
            observe: get("observe"),
            ledger: get("ledger"),
        }
    }

    /// The session's arguments: the filled fields, trimmed; empty ones left out.
    pub(crate) fn args(&self) -> Map<String, Value> {
        let mut a = Map::new();
        for (k, v) in [
            ("mo2", &self.mo2),
            ("profile", &self.profile),
            ("probes", &self.probes),
            ("base_save_sha256", &self.base_save_sha256),
            ("game", &self.game),
            ("observe", &self.observe),
            ("ledger", &self.ledger),
        ] {
            let v = v.trim();
            if !v.is_empty() {
                a.insert(k.into(), json!(v));
            }
        }
        a
    }

    /// The preview: `game.session` with `dry_run: true`. It writes nothing and passes
    /// the gate like a read.
    pub fn preview_request(&self) -> Request {
        let mut a = self.args();
        a.insert("dry_run".into(), json!(true));
        Request::new("game.session", Value::Object(a))
    }

    /// The confirmed start request for the arguments a preview ran with: `dry_run`
    /// dropped, `start: true` added. Called only by the confirm box's button, once per
    /// click: the one place this crate builds a confirmed `game.session` (gate test).
    /// The window starts sessions with `start: true` and follows them with
    /// `game.status`; a synchronous session would block the window's one engine worker.
    pub fn start_request(previewed: &Value) -> Request {
        let mut a = match &previewed {
            Value::Object(m) => Value::Object(m.clone()),
            _ => Value::Object(Map::new()),
        };
        if let Some(m) = a.as_object_mut() {
            m.remove("dry_run");
            m.insert("start".into(), json!(true));
        }
        Request::confirmed("game.session", a)
    }

    /// The stop's preview: `game.stop` for `id` with `dry_run: true`. It writes nothing
    /// and passes the gate like a read.
    pub fn stop_preview(id: &str) -> Request {
        Request::new("game.stop", json!({"id": id, "dry_run": true}))
    }

    /// The confirmed stop request for `id`: the one place this crate builds a
    /// confirmed `game.stop` (gate test).
    pub fn stop_request(id: &str) -> Request {
        Request::confirmed("game.stop", json!({"id": id}))
    }
}

/// The result of a `game.session` dry run: what the confirm box shows, and the
/// arguments its button sends (with `dry_run` dropped and `start: true` added).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionPreview {
    /// The arguments it ran with.
    pub args: Value,
    /// `data.would_start`.
    pub would_start: bool,
    /// `data.refusals`, one `[rule] message` line each.
    pub refusals: Vec<String>,
    /// The plan's key lines: the session, its instance and profile, the probes and
    /// their rows, the base save, and how the session ends.
    pub plan: Vec<String>,
    /// `data.summary`.
    pub summary: String,
}

impl SessionPreview {
    /// The preview from the request it ran and the dry run's reply.
    pub fn from_reply(request: &Request, env: &Value) -> Self {
        let refusals = env["data"]["refusals"]
            .as_array()
            .map(|r| {
                r.iter()
                    .map(|r| {
                        format!(
                            "[{}] {}",
                            r["rule"].as_str().unwrap_or("?"),
                            r["message"].as_str().unwrap_or("")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let plan = env["data"]["plan"]
            .as_object()
            .map(|p| {
                let mut lines = Vec::new();
                if let Some(s) = p.get("session").and_then(Value::as_str) {
                    lines.push(format!("session {s}"));
                }
                if let Some(i) = p.get("instance").and_then(Value::as_str) {
                    lines.push(format!("instance: {i}"));
                }
                if let Some(pr) = p.get("profile").and_then(Value::as_str) {
                    lines.push(format!("profile: {pr}"));
                }
                if let Some(pr) = p.get("probes") {
                    let rows = pr["rows"].as_array().map_or(0, |r| r.len());
                    lines.push(format!("probes: {} ({} rows)", pr["path"].as_str().unwrap_or_default(), rows));
                }
                if let Some(bs) = p.get("base_save").and_then(|b| b["name"].as_str()) {
                    lines.push(format!("base save: {} ({})", bs, p["base_save"]["sha256"].as_str().unwrap_or_default()));
                }
                if let Some(e) = p.get("ends").and_then(Value::as_str) {
                    lines.push(format!("ends: {e}"));
                }
                lines
            })
            .unwrap_or_default();
        Self {
            args: request.args.clone(),
            would_start: env["data"]["would_start"] == json!(true),
            refusals,
            plan,
            summary: env["data"]["summary"].as_str().unwrap_or_default().to_string(),
        }
    }
}

/// The result of a `game.stop` dry run: the `would` lines its confirm box shows.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionStopPreview {
    /// The session id it previews.
    pub id: String,
    /// The phase it showed, if any.
    pub phase: Option<String>,
    /// The `would` lines, in order.
    pub would: Vec<String>,
    /// `data.remaining_rows_become`.
    pub remaining: String,
}

impl SessionStopPreview {
    /// The preview from the dry run's reply.
    pub fn from_reply(env: &Value) -> Self {
        let data = &env["data"];
        Self {
            id: data["session"].as_str().unwrap_or_default().to_string(),
            phase: data["phase"].as_str().map(str::to_string),
            would: data["would"]
                .as_array()
                .map(|w| w.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            remaining: data["remaining_rows_become"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        }
    }
}

/// What a session's confirm box is open for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    /// The confirmed `game.session` start.
    Start,
    /// The confirmed `game.stop`.
    Stop,
}

/// What the window knows about the game session it started, toolkit-free: the drawing
/// below the `// ---- drawing` line reads it, and the shell drives it through
/// [`Self::poll_due`] and the `receive_*` methods on its one engine worker.
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    /// The session id (`data.session` of the start reply); kept after a stop so the
    /// report stays drawn.
    pub id: Option<String>,
    /// The last status's phase (kebab-case), if a status arrived.
    pub phase: Option<String>,
    /// The row running now, if the last status named one.
    pub current_row: Option<String>,
    /// The rows done so far (the last status's `rows`).
    pub rows_done: usize,
    /// The seconds the session has run, from the last status.
    pub elapsed_secs: Option<u64>,
    /// The last status's events, in order.
    pub events: Vec<String>,
    /// The end report (`data.report` of the last status or of the stop), kept when
    /// present.
    pub report: Option<Value>,
    /// A `game.status` is on the worker now; while it is, no other one is queued.
    pub poll_pending: bool,
    /// When the last poll went out; `None` before the first.
    pub last_poll: Option<Instant>,
    /// The last `game.session` dry run, if one arrived.
    pub preview: Option<SessionPreview>,
    /// The last `game.stop` dry run, if one arrived.
    pub stop_preview: Option<SessionStopPreview>,
    /// The confirm box is open for this action (its button builds the confirmed
    /// request once per click).
    pub pending: Option<SessionAction>,
    /// A `game.status` failed (for example "no session" after a stop): polling
    /// stopped, and why.
    pub poll_error: Option<String>,
}

impl SessionState {
    /// A session is started and still to be followed: not `finished`, not stopped
    /// (no report yet), and its polls have not failed.
    pub fn following(&self) -> bool {
        self.id.is_some()
            && self.phase.as_deref() != Some("finished")
            && self.report.is_none()
            && self.poll_error.is_none()
    }

    /// The next `game.status` request when one is due: a second after the last went
    /// out, none while one is pending, and none once the session is done or its polls
    /// failed.
    pub fn poll_due(&mut self, now: Instant) -> Option<Request> {
        let Some(id) = self.id.as_deref() else {
            return None;
        };
        if !self.following() || self.poll_pending {
            return None;
        }
        let due = match self.last_poll {
            Some(t) => now.saturating_duration_since(t) >= POLL_INTERVAL,
            None => true,
        };
        if !due {
            return None;
        }
        self.poll_pending = true;
        self.last_poll = Some(now);
        Some(Request::new("game.status", json!({"id": id})))
    }

    /// The reply to a `game.session` request: the dry run or the start, by
    /// `data.dry_run`.
    pub fn receive_session(&mut self, request: &Request, env: &Value) {
        if env["data"]["dry_run"] == json!(true) {
            self.receive_preview(request, env);
        } else {
            self.receive_start(env);
        }
    }

    /// The dry run's reply: what the confirm box shows and checks.
    pub fn receive_preview(&mut self, request: &Request, env: &Value) {
        self.preview = Some(SessionPreview::from_reply(request, env));
    }

    /// The start's reply: the id comes back at once, and the following starts (the
    /// first poll is due right away).
    pub fn receive_start(&mut self, env: &Value) {
        self.id = env["data"]["session"].as_str().map(str::to_string);
        self.phase = None;
        self.current_row = None;
        self.rows_done = 0;
        self.elapsed_secs = None;
        self.events.clear();
        self.report = None;
        self.pending = None;
        self.poll_error = None;
        self.poll_pending = false;
        self.last_poll = None;
    }

    /// A `game.status` reply: the phase and counts to show; a poll is no longer
    /// pending.
    pub fn receive_status(&mut self, env: &Value) {
        self.poll_pending = false;
        let data = &env["data"];
        self.phase = data["phase"].as_str().map(str::to_string);
        self.current_row = data["current_row"].as_str().map(str::to_string);
        self.rows_done = data["rows"].as_array().map_or(0, |r| r.len());
        self.elapsed_secs = data["elapsed_secs"].as_u64();
        self.events = data["events"]
            .as_array()
            .map(|e| e.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        if data["report"].is_object() {
            self.report = Some(data["report"].clone());
        }
    }

    /// The reply to a `game.stop` request: the dry run or the confirmed stop, by
    /// `data.dry_run`. The confirmed stop's report is kept, and the following ends.
    pub fn receive_stop(&mut self, env: &Value) {
        if env["data"]["dry_run"] == json!(true) {
            self.stop_preview = Some(SessionStopPreview::from_reply(env));
            return;
        }
        // The confirmed stop: its report is kept, and the following ends.
        if env["data"]["report"].is_object() {
            self.report = Some(env["data"]["report"].clone());
        }
        self.pending = None;
        self.poll_pending = false;
    }

    /// An error reply for the session's requests: polling stops (the shell keeps the
    /// error in its results).
    pub fn failed(&mut self) {
        self.poll_pending = false;
        if self.id.is_some() && self.report.is_none() {
            self.poll_error = Some("the status polls failed; polling stopped".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate;

    /// The preview is a dry run with no `start`; the confirm turns those arguments into
    /// the writing form: `start: true`, no `dry_run`, confirmed, and it passes the gate.
    #[test]
    fn the_session_preview_is_a_dry_run_and_the_confirm_starts() {
        let form = SessionForm {
            mo2: "M".into(),
            profile: "KitKraft Dev".into(),
            probes: "P".into(),
            base_save_sha256: "abc".into(),
            game: "G".into(),
            observe: "O".into(),
            ledger: "L".into(),
        };
        let preview = form.preview_request();
        assert_eq!(preview.args["dry_run"], json!(true));
        assert!(preview.args.get("start").is_none(), "{}", preview.args);
        assert!(!preview.is_confirmed());
        assert!(gate::check(&preview).is_ok(), "{}", preview.args);
        let start = SessionForm::start_request(&preview.args);
        assert!(start.is_confirmed());
        assert_eq!(start.args["start"], json!(true));
        assert!(start.args.get("dry_run").is_none(), "{}", start.args);
        assert_eq!(start.args["mo2"], json!("M"));
        assert!(gate::check(&start).is_ok(), "{}", start.args);
        // Empty fields stay out of the arguments.
        let thin = SessionForm {
            mo2: "M".into(),
            ..Default::default()
        };
        assert_eq!(
            thin.preview_request().args,
            json!({"mo2": "M", "dry_run": true})
        );
    }

    /// The start reply's `data.session` (the canned reply the ticket gives the state).
    fn started_env() -> Value {
        json!({
            "command": "game.session",
            "data": {
                "session": "2026-10-25-T01",
                "started": true,
                "observe": "observe/",
                "summary": "session started"
            }
        })
    }

    /// M6.G4: a started session asks for its first poll at once — and then stops
    /// asking, on a `"finished"` status (the envelope shape the engine's
    /// `game.status` handler writes) and, separately, once the polls fail.
    #[test]
    fn a_finished_session_stops_polling() {
        let mut s = SessionState::default();
        assert!(s.poll_due(Instant::now()).is_none(), "no session yet");
        s.receive_start(&started_env());
        assert_eq!(s.id.as_deref(), Some("2026-10-25-T01"));

        // The first poll is due right away, and none while it is pending.
        let now = Instant::now();
        let due = s.poll_due(now).expect("a poll is due right after the start");
        assert_eq!(due.id, "game.status");
        assert_eq!(due.args["id"], json!("2026-10-25-T01"));
        assert!(s.poll_pending);
        assert!(s.poll_due(now).is_none(), "one poll is pending: no other is due");
        assert!(s.poll_due(now + POLL_INTERVAL).is_none(), "still the same pending poll");

        // The finished status ends the following: its report is kept, no more polls.
        let finished = json!({
            "command": "game.status",
            "data": {
                "session": "2026-10-25-T01",
                "phase": "finished",
                "current_row": null,
                "rows": [],
                "reruns": [],
                "screenshots": [],
                "events": [],
                "elapsed_secs": 3,
                "report": {"rows": 4}
            }
        });
        s.receive_status(&finished);
        assert_eq!(s.phase.as_deref(), Some("finished"));
        assert!(s.report.is_some(), "the report is kept");
        assert!(!s.following());
        assert!(s.poll_due(now + POLL_INTERVAL).is_none(), "finished: no more polls");

        // Separately: a failed status poll stops the following too, and it stays
        // stopped past the interval.
        let mut s = SessionState::default();
        s.receive_start(&started_env());
        let probing = json!({
            "command": "game.status",
            "data": {
                "session": "2026-10-25-T01",
                "phase": "probing",
                "current_row": "row 1",
                "rows": ["row 1"],
                "reruns": [],
                "screenshots": [],
                "events": ["row 1 done"],
                "elapsed_secs": 1,
                "report": null
            }
        });
        s.receive_status(&probing);
        assert!(s.following(), "a probing session is still followed");
        assert!(s.poll_due(now).is_some(), "a poll is due after a status");
        s.failed();
        assert!(s.poll_error.is_some(), "the failure is remembered");
        assert!(!s.following());
        assert!(s.poll_due(now).is_none(), "failed: no poll");
        assert!(
            s.poll_due(now + POLL_INTERVAL).is_none(),
            "failed: still no poll a second later"
        );
    }

    /// M6.G4: the window follows a session it started, on the engine's one worker:
    /// the dry run says `would_start`, the start gives the id at once, every poll is
    /// an ordinary `game.status` on that worker with a read right behind it (the
    /// replies come back in submission order), and the stop keeps the report.
    #[test]
    fn the_window_follows_a_started_session_on_its_one_worker() {
        use crate::gate::{GameFakes, ReadOnlyEngine};
        use crate::worker::Worker;

        const WAIT: Duration = Duration::from_secs(60);

        let fakes = GameFakes::new("cockpit-follow").unwrap();
        let mut worker: Worker =
            Worker::spawn_with_engine(ReadOnlyEngine::with_game_fakes(&fakes), None).unwrap();
        let form = SessionForm::from_args(&fakes.session_args(json!({})));
        let mut state = SessionState::default();

        // The preview: the dry run says `would_start`.
        let preview = form.preview_request();
        worker.submit(preview.clone()).unwrap();
        let env = worker
            .wait(WAIT)
            .expect("the preview reply is due")
            .result
            .unwrap();
        state.receive_session(&preview, &env);
        let pv = state.preview.as_ref().expect("the preview is stored");
        assert!(pv.would_start, "{}", pv.summary);
        assert!(
            pv.plan.iter().any(|l| l.starts_with("probes")),
            "{:?}",
            pv.plan
        );

        // The start: the id comes back at once.
        let start = SessionForm::start_request(&pv.args);
        worker.submit(start.clone()).unwrap();
        let env = worker
            .wait(WAIT)
            .expect("the start reply is due")
            .result
            .unwrap();
        state.receive_session(&start, &env);
        let id = state.id.clone().expect("the start gave the id");

        // The following: each turn submits the due poll and, right after it, a read;
        // no poll is due while one is pending.
        let mut now = Instant::now();
        let mut polls = 0u64;
        let mut turns = 0u64;
        while state.phase.as_deref() != Some("idle") {
            turns = turns.saturating_add(1);
            assert!(turns < 400, "still running after {turns} turns");
            let poll = state.poll_due(now).expect("a poll is due this turn");
            assert!(
                state.poll_due(now).is_none(),
                "no second poll while one is pending"
            );
            worker.submit(poll).unwrap();
            worker
                .submit(Request::new("schema", json!({"type": "WEAP"})))
                .unwrap();
            let status = worker
                .wait(WAIT)
                .expect("the poll reply is due")
                .result
                .unwrap();
            state.receive_status(&status);
            let read = worker
                .wait(WAIT)
                .expect("the read reply is due")
                .result
                .unwrap();
            assert_eq!(read["data"]["defined"], json!(true), "{read}");
            polls = polls.saturating_add(1);
            now = now + POLL_INTERVAL;
        }
        assert!(
            polls >= 2,
            "{polls} polls: at least two, with a read served between them"
        );

        // The stop: the preview's `would` lines, then the confirmed stop; the report
        // is kept, and polling ends.
        let stop = SessionForm::stop_preview(&id);
        worker.submit(stop).unwrap();
        let env = worker
            .wait(WAIT)
            .expect("the stop preview reply is due")
            .result
            .unwrap();
        state.receive_stop(&env);
        let sp = state
            .stop_preview
            .as_ref()
            .expect("the stop preview is stored");
        assert_eq!(sp.id, id);
        assert!(!sp.would.is_empty(), "{:?}", sp.would);
        worker.submit(SessionForm::stop_request(&id)).unwrap();
        let env = worker
            .wait(WAIT)
            .expect("the stop reply is due")
            .result
            .unwrap();
        state.receive_stop(&env);
        assert!(state.report.is_some(), "the stop's report is kept");
        assert!(!state.following());
        assert!(
            state.poll_due(now).is_none(),
            "after the stop, no more polls are due"
        );
    }
}
