//! Progress events from long commands (M6.P2b).
//!
//! A front end that wants to show progress passes a [`ProgressSink`] to
//! [`crate::Engine::execute_with_progress`] (or [`crate::execute_with_progress`]); the
//! commands that take long call it with typed [`Progress`] events while they run. Without
//! a sink nothing is emitted, and the sink never changes the result: the envelope is
//! identical with or without one except timings (the `*_ms` members and the `summary` text
//! that quotes them differ from run to run; a test compares the rest).
//!
//! What emits:
//!
//! | Stage           | From                                                    | `item`               |
//! | --------------- | ------------------------------------------------------- | -------------------- |
//! | `index`         | every command that loads a load order (`load`, and `conflicts`, `refs`, `validate`, `build`, `query`, ... given `profile` or `plugins`); one event with `done` 0 before indexing, then one per plugin as its index is read from the cache or built | the plugin |
//! | `reverse-index` | `refs --incoming` (first use per load order), one per plugin | the plugin |
//! | `records`       | `coverage`, one per top-level group read                | the group's record type |
//! | `roundtrip`     | `roundtrip`, at the start (0 of 1) and the end (1 of 1) | the plugin           |
//! | `scan`          | `conflicts`, before the override table is scanned       | -                    |
//! | `validate`      | `validate` and `build`, before the rules run            | the plugin           |
//! | `classes`, `rules`, `write` | `build`, one per class, one per rule, then the write | the class or rule key |
//!
//! Index events arrive in the order plugins finish (largest files start first), with
//! `done` counting up by one; they come from the indexing worker threads, so a sink must
//! be `Send + Sync` and should return quickly (the CLI prints one line to stderr).
//!
//! The sink travels in a thread-local scope for the duration of the call, so handler
//! signatures do not change; a handler that hands work to other threads takes
//! [`current`] first and moves the [`Reporter`] there.

use std::cell::RefCell;
use std::sync::Arc;

use serde::Serialize;

/// One progress event. Serializes as `{command, stage, done, total, item}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progress {
    /// Canonical id of the command running (`load`, `refs`, ...).
    pub command: String,
    /// What it is doing (see the module docs for the stages).
    pub stage: &'static str,
    /// Items of this stage finished so far.
    pub done: u64,
    /// Items in this stage, when known.
    pub total: Option<u64>,
    /// The plugin, record type, class or rule the event is about, when there is one.
    pub item: Option<String>,
}

impl Progress {
    /// One human line: `load: index 3/16 Starfield.esm`.
    pub fn line(&self) -> String {
        let count = match self.total {
            Some(total) => format!("{}/{total}", self.done),
            None => self.done.to_string(),
        };
        match &self.item {
            Some(item) => format!("{}: {} {count} {item}", self.command, self.stage),
            None => format!("{}: {} {count}", self.command, self.stage),
        }
    }
}

/// Receives [`Progress`] events. Any `Fn(&Progress) + Send + Sync` is a sink.
pub trait ProgressSink: Send + Sync {
    /// Called once per event; must not block for long.
    fn progress(&self, event: &Progress);
}

impl<F> ProgressSink for F
where
    F: Fn(&Progress) + Send + Sync,
{
    fn progress(&self, event: &Progress) {
        self(event);
    }
}

/// The sink of the running command and its id; cheap to clone and `Send + Sync`, so a
/// handler can move it to worker threads.
#[derive(Clone)]
pub struct Reporter {
    sink: Arc<dyn ProgressSink>,
    command: &'static str,
}

impl std::fmt::Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporter")
            .field("command", &self.command)
            .finish_non_exhaustive()
    }
}

impl Reporter {
    /// Sends one event.
    pub fn emit(&self, stage: &'static str, done: usize, total: Option<usize>, item: Option<&str>) {
        let count = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
        self.sink.progress(&Progress {
            command: self.command.to_string(),
            stage,
            done: count(done),
            total: total.map(count),
            item: item.map(str::to_string),
        });
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Reporter>> = const { RefCell::new(None) };
}

/// The reporter of the command running on this thread, if its caller passed a sink.
pub fn current() -> Option<Reporter> {
    CURRENT.with(|c| c.try_borrow().ok().and_then(|r| r.clone()))
}

/// Sends one event to the current sink; nothing when there is none.
pub(crate) fn emit(stage: &'static str, done: usize, total: Option<usize>, item: Option<&str>) {
    if let Some(r) = current() {
        r.emit(stage, done, total, item);
    }
}

/// A `kitkraft_doc` per-plugin callback that forwards to `reporter` as `stage` events.
pub(crate) fn per_plugin<'r>(
    reporter: Option<&'r Reporter>,
    stage: &'static str,
) -> impl Fn(kitkraft_doc::PluginDone<'_>) + Sync + 'r {
    move |d| {
        if let Some(r) = reporter {
            r.emit(stage, d.done, Some(d.total), Some(d.plugin));
        }
    }
}

/// Installs a sink for the life of the guard and restores the previous one on drop.
pub(crate) struct Scope {
    previous: Option<Reporter>,
}

impl Scope {
    pub(crate) fn enter(sink: Arc<dyn ProgressSink>, command: &'static str) -> Scope {
        let next = Some(Reporter { sink, command });
        let previous = CURRENT.with(|c| match c.try_borrow_mut() {
            Ok(mut slot) => std::mem::replace(&mut *slot, next),
            Err(_) => None,
        });
        Scope { previous }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|c| {
            if let Ok(mut slot) = c.try_borrow_mut() {
                *slot = previous;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn a_scope_installs_and_restores_the_sink() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        assert!(current().is_none());
        emit("index", 0, None, None);
        {
            let _scope = Scope::enter(
                Arc::new(move |p: &Progress| log.lock().unwrap().push(p.line())),
                "load",
            );
            emit("index", 1, Some(2), Some("A.esm"));
            emit("records", 3, None, None);
        }
        emit("index", 2, Some(2), Some("B.esm"));
        assert!(current().is_none());
        assert_eq!(
            *seen.lock().unwrap(),
            ["load: index 1/2 A.esm", "load: records 3"]
        );
    }
}
