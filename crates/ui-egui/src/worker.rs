//! The engine on a worker thread (ADR 0007, option A item 4): the window sends requests
//! over a channel and polls for replies, so a long `load` or `coverage` never freezes it.
//! The thread runs one command at a time, in submission order. It is the only engine
//! thread the window has.
//!
//! Besides running the command, the thread also builds what the window will show from
//! the envelope (M6.P5a): a request may carry a [`Prepare`] closure, run on this thread
//! right after the command, whose result comes back in [`Reply::built`]. So a 13 MB
//! `conflicts` envelope is turned into view-models and raw JSON lines here, and the
//! window's frame only draws.
//!
//! While a command runs, its latest progress event (M6.P2b, `crate::gate::Progress`) is
//! readable through [`Worker::running`] for the status line (M6.P2a); each event wakes the
//! window.
//!
//! Queued requests can be dropped by restarting the worker ([`Worker::restart`]): a new
//! thread with a new, empty session takes over, and the old one stops before its next
//! command. A command already running cannot be interrupted (`execute` has no
//! cancellation); the old thread finishes it and its reply is discarded.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::gate::{check, Progress, ReadOnlyEngine, Request, RunError};

/// Called on the worker thread after each reply and each progress event (the window
/// passes `request_repaint`).
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Builds what the window shows from one reply, on the worker thread (M6.P5a).
pub type Prepare<B> = Box<dyn FnOnce(&Request, &Result<Value, RunError>) -> B + Send>;

/// What the worker returns for one request.
#[derive(Debug, Clone)]
pub struct Reply<B = ()> {
    /// The ticket [`Worker::submit`] returned.
    pub ticket: u64,
    /// The request as submitted.
    pub request: Request,
    /// The envelope, or why there is none.
    pub result: Result<Value, RunError>,
    /// Wall time of the command on the worker thread.
    pub elapsed: Duration,
    /// Time from submit to the start of the run (queued behind other commands).
    pub waited: Duration,
    /// Time from the start of the run to the engine's first progress event, if any.
    pub first_progress: Option<Duration>,
    /// The last progress event the command sent, if any.
    pub last_progress: Option<Progress>,
    /// The envelope's size as compact JSON (only with the perf trace on, `crate::perf`).
    pub bytes: Option<usize>,
    /// What the request's [`Prepare`] built, when it had one.
    pub built: Option<B>,
    /// Time the [`Prepare`] took on the worker thread.
    pub prep: Duration,
}

/// The command running now, for the status line.
#[derive(Debug, Clone, PartialEq)]
pub struct Running {
    /// Its id as submitted.
    pub command: String,
    /// When it started on the worker thread.
    pub started: Instant,
    /// Its latest progress event, if it sent one.
    pub progress: Option<Progress>,
}

struct Job<B> {
    ticket: u64,
    request: Request,
    queued_at: Instant,
    prepare: Option<Prepare<B>>,
}

type Shared = Arc<Mutex<Option<Running>>>;

fn set_running(shared: &Shared, value: Option<Running>) {
    *shared.lock().unwrap_or_else(PoisonError::into_inner) = value;
}

/// A handle to the engine thread.
///
/// Dropping it sets the thread's stop flag and closes its channels without waiting: the
/// thread runs no further queued command, finishes the one it is running (if any; a
/// `load` can take most of a minute in a debug build) and exits. Drop joins the thread
/// only when it has already finished, so closing the window never blocks on a command;
/// at process exit a still-running command is simply ended with the process. A cache
/// entry it was writing is left as a temp file, which the next start removes
/// (`kitkraft-doc`'s cache sweep).
pub struct Worker<B: Send + 'static = ()> {
    jobs: Option<Sender<Job<B>>>,
    replies: Receiver<Reply<B>>,
    next: u64,
    pending: Vec<(u64, String)>,
    stop: Arc<AtomicBool>,
    wake: Option<Wake>,
    running: Shared,
    thread: Option<JoinHandle<()>>,
}

impl<B: Send + 'static> std::fmt::Debug for Worker<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

impl<B: Send + 'static> Worker<B> {
    /// Starts the engine thread with an empty session.
    pub fn spawn(wake: Option<Wake>) -> std::io::Result<Self> {
        let (job_tx, job_rx) = channel::<Job<B>>();
        let (reply_tx, reply_rx) = channel::<Reply<B>>();
        let stop = Arc::new(AtomicBool::new(false));
        let running: Shared = Arc::new(Mutex::new(None));
        let thread_stop = Arc::clone(&stop);
        let thread_wake = wake.clone();
        let thread_running = Arc::clone(&running);
        let thread = std::thread::Builder::new()
            .name("kitkraft-engine".to_string())
            .spawn(move || {
                let engine = ReadOnlyEngine::new();
                while let Ok(job) = job_rx.recv() {
                    // Queued jobs stay in the channel after the handle is dropped; the
                    // flag keeps them from running.
                    if thread_stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let reply = run_job(&engine, job, &thread_running, thread_wake.as_ref());
                    set_running(&thread_running, None);
                    if reply_tx.send(reply).is_err() {
                        break;
                    }
                    if let Some(w) = &thread_wake {
                        w();
                    }
                }
            })?;
        Ok(Self {
            jobs: Some(job_tx),
            replies: reply_rx,
            next: 0,
            pending: Vec::new(),
            stop,
            wake,
            running,
            thread: Some(thread),
        })
    }

    /// Queues `request`. A request the gate refuses never reaches the thread: the refusal
    /// is returned here.
    pub fn submit(&mut self, request: Request) -> Result<u64, RunError> {
        self.submit_with(request, None)
    }

    /// Queues `request` with a [`Prepare`] to run on the worker thread after it.
    pub fn submit_with(
        &mut self,
        request: Request,
        prepare: Option<Prepare<B>>,
    ) -> Result<u64, RunError> {
        check(&request)?;
        let ticket = self.next;
        self.next = self.next.wrapping_add(1);
        let label = request.id.clone();
        self.jobs
            .as_ref()
            .ok_or_else(|| RunError::Refused("the engine thread has stopped".to_string()))?
            .send(Job {
                ticket,
                request,
                queued_at: Instant::now(),
                prepare,
            })
            .map_err(|_| RunError::Refused("the engine thread has stopped".to_string()))?;
        self.pending.push((ticket, label));
        Ok(ticket)
    }

    /// Replies that arrived since the last call, without blocking.
    pub fn poll(&mut self) -> Vec<Reply<B>> {
        let mut out = Vec::new();
        while let Ok(reply) = self.replies.try_recv() {
            self.pending.retain(|(t, _)| *t != reply.ticket);
            out.push(reply);
        }
        out
    }

    /// Waits up to `timeout` for the next reply (tests and the launcher's preload).
    pub fn wait(&mut self, timeout: Duration) -> Option<Reply<B>> {
        match self.replies.recv_timeout(timeout) {
            Ok(reply) => {
                self.pending.retain(|(t, _)| *t != reply.ticket);
                Some(reply)
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
        }
    }

    /// The commands queued or running, oldest first (the busy indicator).
    pub fn pending(&self) -> Vec<&str> {
        self.pending.iter().map(|(_, id)| id.as_str()).collect()
    }

    /// Whether a command is queued or running.
    pub fn is_busy(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Commands waiting behind the one running.
    pub fn queued(&self) -> usize {
        self.pending.len().saturating_sub(1)
    }

    /// The command running now and its latest progress event (M6.P2a); `None` between
    /// commands.
    pub fn running(&self) -> Option<Running> {
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Drops every queued request by replacing the engine thread with a new one (a new,
    /// empty session: the load order must be loaded again). Returns how many requests
    /// were dropped, the running one included; no reply arrives for any of them. Tickets
    /// keep counting up, so a stale ticket never matches a new request.
    pub fn restart(&mut self) -> std::io::Result<usize> {
        let mut fresh = Worker::<B>::spawn(self.wake.clone())?;
        fresh.next = self.next;
        let dropped = self.pending.len();
        // Dropping the old handle stops its thread (see the type's docs).
        drop(std::mem::replace(self, fresh));
        Ok(dropped)
    }
}

/// Runs one job on the worker thread: the command with a progress sink, then its
/// [`Prepare`].
fn run_job<B>(
    engine: &ReadOnlyEngine,
    job: Job<B>,
    running: &Shared,
    wake: Option<&Wake>,
) -> Reply<B> {
    let start = Instant::now();
    let waited = start.saturating_duration_since(job.queued_at);
    set_running(
        running,
        Some(Running {
            command: job.request.id.clone(),
            started: start,
            progress: None,
        }),
    );
    let first = Arc::new(OnceLock::<Duration>::new());
    let seen = Arc::clone(&first);
    let sink_running = Arc::clone(running);
    let sink_wake = wake.cloned();
    let result = engine.run_with_progress(&job.request, move |p: &Progress| {
        let _ = seen.get_or_init(|| start.elapsed());
        if let Some(r) = sink_running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            r.progress = Some(p.clone());
        }
        if let Some(w) = &sink_wake {
            w();
        }
    });
    let elapsed = start.elapsed();
    let last_progress = running
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|r| r.progress.clone());
    let bytes = if crate::perf::enabled() {
        result
            .as_ref()
            .ok()
            .and_then(|env| serde_json::to_vec(env).ok())
            .map(|b| b.len())
    } else {
        None
    };
    let prep_start = Instant::now();
    let built = job.prepare.map(|prepare| prepare(&job.request, &result));
    let prep = prep_start.elapsed();
    Reply {
        ticket: job.ticket,
        request: job.request,
        result,
        elapsed,
        waited,
        first_progress: first.get().copied(),
        last_progress,
        bytes,
        built,
        prep,
    }
}

impl<B: Send + 'static> Drop for Worker<B> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use serde_json::json;

    const WAIT: Duration = Duration::from_secs(60);

    fn worker() -> Worker {
        Worker::spawn(None).unwrap()
    }

    #[test]
    fn a_command_runs_through_the_worker_without_a_window() {
        let mut worker = worker();
        let ticket = worker
            .submit(Request::new("schema", json!({"type": "KYWD"})))
            .unwrap();
        assert!(worker.is_busy());
        let reply = worker.wait(WAIT).unwrap();
        assert_eq!(reply.ticket, ticket);
        let env = reply.result.unwrap();
        assert_eq!(env["command"], "schema");
        assert_eq!(env["data"]["defined"], true);
        assert!(!worker.is_busy());
        assert!(reply.built.is_none());
        assert!(worker.running().is_none());
    }

    /// M6.P5a: the reply's view is built on the engine thread, not the caller's.
    #[test]
    fn prepare_runs_on_the_engine_thread_after_the_command() {
        let mut worker: Worker<(String, bool)> = Worker::spawn(None).unwrap();
        let prepare: Prepare<(String, bool)> = Box::new(|req, result| {
            let name = std::thread::current().name().unwrap_or("").to_string();
            (name, req.id == "schema" && result.is_ok())
        });
        worker
            .submit_with(Request::new("schema", json!({})), Some(prepare))
            .unwrap();
        let reply = worker.wait(WAIT).unwrap();
        assert_eq!(reply.built, Some(("kitkraft-engine".to_string(), true)));
    }

    /// M6.P2a: a `load` sends index progress through the worker (a one-record plugin
    /// written to a temp folder).
    #[test]
    fn a_load_reports_its_progress() {
        let dir = std::env::temp_dir().join(format!("kitkraft-ui-progress-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plugin = dir.join("Tiny.esp");
        // A TES4 header record alone: HEDR (version 0.96, 0 records, next id 0x800).
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"TES4");
        bytes.extend_from_slice(&18u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(b"HEDR");
        bytes.extend_from_slice(&12u16.to_le_bytes());
        bytes.extend_from_slice(&0.96f32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0x800u32.to_le_bytes());
        std::fs::write(&plugin, &bytes).unwrap();
        let mut worker = worker();
        worker
            .submit(Request::new(
                "load",
                json!({"plugins": [plugin.display().to_string()], "no_cache": true}),
            ))
            .unwrap();
        let reply = worker.wait(WAIT).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(reply.result.is_ok(), "{:?}", reply.result);
        let last = reply.last_progress.expect("no progress event");
        assert_eq!((last.stage, last.done, last.total), ("index", 1, Some(1)));
        assert!(reply.first_progress.is_some());
        assert!(worker.running().is_none());
    }

    #[test]
    fn replies_come_back_in_submission_order() {
        let mut worker = worker();
        let a = worker.submit(Request::new("schema", json!({}))).unwrap();
        let b = worker
            .submit(Request::new("schema", json!({"type": "GLOB"})))
            .unwrap();
        assert_eq!(worker.pending(), vec!["schema", "schema"]);
        assert_eq!(worker.queued(), 1);
        assert_eq!(worker.wait(WAIT).unwrap().ticket, a);
        assert_eq!(worker.wait(WAIT).unwrap().ticket, b);
        assert_eq!(worker.queued(), 0);
    }

    #[test]
    fn a_writing_command_is_refused_at_submit() {
        let mut worker = worker();
        let err = worker
            .submit(Request::new(
                "set",
                json!({"form": "x", "fields": {}, "into": "y.esp"}),
            ))
            .unwrap_err();
        assert!(matches!(err, RunError::Refused(_)));
        assert!(!worker.is_busy());
    }

    #[test]
    fn the_wake_callback_runs_after_a_reply() {
        let (tx, rx) = channel::<()>();
        let tx = std::sync::Mutex::new(tx);
        let wake: Wake = Arc::new(move || {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(());
            }
        });
        let mut worker: Worker = Worker::spawn(Some(wake)).unwrap();
        worker.submit(Request::new("schema", json!({}))).unwrap();
        rx.recv_timeout(WAIT).unwrap();
        assert!(worker.wait(WAIT).is_some());
    }

    /// Queued jobs are still in the channel when the handle goes; the stop flag keeps
    /// the thread from running them.
    #[test]
    fn a_stopped_thread_runs_no_queued_command() {
        let mut worker = worker();
        worker.stop.store(true, Ordering::SeqCst);
        for _ in 0..3 {
            worker.submit(Request::new("schema", json!({}))).unwrap();
        }
        worker.jobs = None;
        worker.thread.take().unwrap().join().unwrap();
        assert!(worker.poll().is_empty(), "a queued command ran after stop");
    }

    #[test]
    fn restart_drops_pending_and_keeps_tickets_unique() {
        let mut worker = worker();
        let mut last = 0;
        for _ in 0..4 {
            last = worker.submit(Request::new("schema", json!({}))).unwrap();
        }
        assert_eq!(worker.restart().unwrap(), 4);
        assert!(!worker.is_busy());
        let fresh = worker.submit(Request::new("schema", json!({}))).unwrap();
        assert!(fresh > last, "ticket {fresh} reused");
        let reply = worker.wait(WAIT).unwrap();
        assert_eq!(reply.ticket, fresh, "a dropped request replied");
        assert!(worker.wait(Duration::from_millis(200)).is_none());
    }
}
