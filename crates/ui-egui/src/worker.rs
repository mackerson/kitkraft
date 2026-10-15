//! The engine on a worker thread (ADR 0007, option A item 4): the window sends requests
//! over a channel and polls for replies, so a long `load` or `coverage` never freezes it.
//! The thread runs one command at a time, in submission order.
//!
//! Queued requests can be dropped by restarting the worker ([`Worker::restart`]): a new
//! thread with a new, empty session takes over, and the old one stops before its next
//! command. A command already running cannot be interrupted (`execute` has no
//! cancellation); the old thread finishes it and its reply is discarded.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::gate::{check, ReadOnlyEngine, Request, RunError};

/// Called on the worker thread after each reply (the window passes `request_repaint`).
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// What the worker returns for one request.
#[derive(Debug, Clone)]
pub struct Reply {
    /// The ticket [`Worker::submit`] returned.
    pub ticket: u64,
    /// The request as submitted.
    pub request: Request,
    /// The envelope, or why there is none.
    pub result: Result<Value, RunError>,
    /// Wall time on the worker thread.
    pub elapsed: Duration,
}

struct Job {
    ticket: u64,
    request: Request,
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
pub struct Worker {
    jobs: Option<Sender<Job>>,
    replies: Receiver<Reply>,
    next: u64,
    pending: Vec<(u64, String)>,
    stop: Arc<AtomicBool>,
    wake: Option<Wake>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

impl Worker {
    /// Starts the engine thread with an empty session.
    pub fn spawn(wake: Option<Wake>) -> std::io::Result<Self> {
        let (job_tx, job_rx) = channel::<Job>();
        let (reply_tx, reply_rx) = channel::<Reply>();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_wake = wake.clone();
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
                    let start = Instant::now();
                    let result = engine.run(&job.request);
                    let reply = Reply {
                        ticket: job.ticket,
                        request: job.request,
                        result,
                        elapsed: start.elapsed(),
                    };
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
            thread: Some(thread),
        })
    }

    /// Queues `request`. A request the gate refuses never reaches the thread: the refusal
    /// is returned here.
    pub fn submit(&mut self, request: Request) -> Result<u64, RunError> {
        check(&request)?;
        let ticket = self.next;
        self.next = self.next.wrapping_add(1);
        let label = request.id.clone();
        self.jobs
            .as_ref()
            .ok_or_else(|| RunError::Refused("the engine thread has stopped".to_string()))?
            .send(Job { ticket, request })
            .map_err(|_| RunError::Refused("the engine thread has stopped".to_string()))?;
        self.pending.push((ticket, label));
        Ok(ticket)
    }

    /// Replies that arrived since the last call, without blocking.
    pub fn poll(&mut self) -> Vec<Reply> {
        let mut out = Vec::new();
        while let Ok(reply) = self.replies.try_recv() {
            self.pending.retain(|(t, _)| *t != reply.ticket);
            out.push(reply);
        }
        out
    }

    /// Waits up to `timeout` for the next reply (tests and the launcher's preload).
    pub fn wait(&mut self, timeout: Duration) -> Option<Reply> {
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

    /// Drops every queued request by replacing the engine thread with a new one (a new,
    /// empty session: the load order must be loaded again). Returns how many requests
    /// were dropped, the running one included; no reply arrives for any of them. Tickets
    /// keep counting up, so a stale ticket never matches a new request.
    pub fn restart(&mut self) -> std::io::Result<usize> {
        let mut fresh = Worker::spawn(self.wake.clone())?;
        fresh.next = self.next;
        let dropped = self.pending.len();
        // Dropping the old handle stops its thread (see the type's docs).
        drop(std::mem::replace(self, fresh));
        Ok(dropped)
    }
}

impl Drop for Worker {
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
mod tests {
    use super::*;
    use serde_json::json;

    const WAIT: Duration = Duration::from_secs(60);

    #[test]
    fn a_command_runs_through_the_worker_without_a_window() {
        let mut worker = Worker::spawn(None).unwrap();
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
    }

    #[test]
    fn replies_come_back_in_submission_order() {
        let mut worker = Worker::spawn(None).unwrap();
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
        let mut worker = Worker::spawn(None).unwrap();
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
        let mut worker = Worker::spawn(Some(wake)).unwrap();
        worker.submit(Request::new("schema", json!({}))).unwrap();
        rx.recv_timeout(WAIT).unwrap();
        assert!(worker.wait(WAIT).is_some());
    }

    /// Queued jobs are still in the channel when the handle goes; the stop flag keeps
    /// the thread from running them.
    #[test]
    fn a_stopped_thread_runs_no_queued_command() {
        let mut worker = Worker::spawn(None).unwrap();
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
        let mut worker = Worker::spawn(None).unwrap();
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
