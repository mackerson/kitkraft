//! The engine on a worker thread (ADR 0007, option A item 4): the window sends requests
//! over a channel and polls for replies, so a long `load` or `coverage` never freezes it.
//! The thread runs one command at a time, in submission order.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::gate::{check, ReadOnlyEngine, Request, RunError};

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

/// A handle to the engine thread. Dropping it closes the channel; the thread ends after
/// the command it is running.
pub struct Worker {
    jobs: Option<Sender<Job>>,
    replies: Receiver<Reply>,
    next: u64,
    pending: Vec<(u64, String)>,
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
    /// Starts the engine thread. `wake` is called on the worker thread after each reply
    /// (the window passes `request_repaint`).
    pub fn spawn(wake: Option<Box<dyn Fn() + Send>>) -> std::io::Result<Self> {
        let (job_tx, job_rx) = channel::<Job>();
        let (reply_tx, reply_rx) = channel::<Reply>();
        let thread = std::thread::Builder::new()
            .name("kitkraft-engine".to_string())
            .spawn(move || {
                let engine = ReadOnlyEngine::new();
                while let Ok(job) = job_rx.recv() {
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
                    if let Some(w) = &wake {
                        w();
                    }
                }
            })?;
        Ok(Self {
            jobs: Some(job_tx),
            replies: reply_rx,
            next: 0,
            pending: Vec::new(),
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
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Closing the job channel ends the thread's loop after its current command; the
        // window does not wait for a long command to finish.
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
        assert_eq!(worker.wait(WAIT).unwrap().ticket, a);
        assert_eq!(worker.wait(WAIT).unwrap().ticket, b);
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
        let wake: Box<dyn Fn() + Send> = Box::new(move || {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(());
            }
        });
        let mut worker = Worker::spawn(Some(wake)).unwrap();
        worker.submit(Request::new("schema", json!({}))).unwrap();
        rx.recv_timeout(WAIT).unwrap();
        assert!(worker.wait(WAIT).is_some());
    }
}
