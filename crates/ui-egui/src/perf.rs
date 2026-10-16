//! The workbench's performance trace (M6.P1, `docs/design/workbench-performance.md`).
//!
//! With `KITKRAFT_WORKBENCH_PERF=1` the window prints `perf:` lines to stderr, one per
//! event, each with the milliseconds since the process started (`t=`):
//!
//! | Event | Fields |
//! | ----- | ------ |
//! | `first-frame` | the first frame's own time (`ui_ms`) |
//! | `tab <Name> first-paint` | the first frame that drew the tab (`ui_ms`) |
//! | `request <id> queued` | the ticket |
//! | `request <id> done` | time queued (`wait_ms`), in the engine (`run_ms`), first progress event (`first_progress_ms`), envelope bytes |
//! | `request <id> prepared` | view-model and raw JSON time (`prep_ms`) and the thread that did it (`on=worker` or `on=ui`) |
//! | `request <id> drawn` | click (queued) to the first frame that drew the result (`total_ms`), that frame's own time (`ui_ms`) |
//! | `frames <phase>` | frame count and frame time p50, p95 and max (`frame_ms`: eframe's CPU time per frame, layout and painting without the vsync wait) |
//!
//! It writes no file: `kit-ui` is a console program, so redirect stderr to keep the
//! lines. Without the variable nothing is measured beyond two clock reads per frame.
//!
//! `KITKRAFT_WORKBENCH_PERF_BENCH=1` (with a `--profile` launch) also runs the protocol's
//! window bench once and closes (the shell's `drive_bench`): a 1000-row `query REFR` on
//! Records (first paint), five seconds of wheel scrolling over its table (T1), the Diff
//! tour request (first paint) and three seconds of scrolling the Diff tab.

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Turns the trace on (`1`).
pub const PERF_ENV: &str = "KITKRAFT_WORKBENCH_PERF";
/// Runs the window bench (`1`) and closes; implies the trace.
pub const BENCH_ENV: &str = "KITKRAFT_WORKBENCH_PERF_BENCH";

static START: OnceLock<Instant> = OnceLock::new();
static ON: OnceLock<bool> = OnceLock::new();
/// The printer: a write to a redirected stderr takes milliseconds on Windows, so lines
/// are stamped where they happen and written by a thread of their own (trace only).
static PRINTER: OnceLock<Option<Mutex<Sender<Msg>>>> = OnceLock::new();

enum Msg {
    Line(String),
    Flush(Sender<()>),
}

fn printer() -> Option<&'static Mutex<Sender<Msg>>> {
    PRINTER
        .get_or_init(|| {
            let (tx, rx) = channel::<Msg>();
            std::thread::Builder::new()
                .name("kitkraft-perf".to_string())
                .spawn(move || {
                    let mut err = std::io::stderr();
                    while let Ok(msg) = rx.recv() {
                        match msg {
                            Msg::Line(l) => {
                                let _ = writeln!(err, "{l}");
                            }
                            Msg::Flush(done) => {
                                let _ = done.send(());
                            }
                        }
                    }
                })
                .ok()
                .map(|_| Mutex::new(tx))
        })
        .as_ref()
}

/// Marks the process start; the launcher calls it first thing (else the first use does).
pub fn mark_start() {
    let _ = START.get_or_init(Instant::now);
}

/// Time since [`mark_start`].
pub fn since_start() -> Duration {
    START.get_or_init(Instant::now).elapsed()
}

fn flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| {
        let v = v.trim();
        !v.is_empty() && v != "0"
    })
}

/// Whether the trace is on (read once).
pub fn enabled() -> bool {
    *ON.get_or_init(|| flag(PERF_ENV) || flag(BENCH_ENV))
}

/// Whether the window bench runs.
pub fn bench_requested() -> bool {
    flag(BENCH_ENV)
}

/// Milliseconds with one decimal.
pub fn ms(d: Duration) -> String {
    format!("{:.1}", d.as_secs_f64() * 1000.0)
}

/// Prints `perf: t=<ms> <event> <fields>` when the trace is on (stamped now, written by
/// the printer thread).
pub fn line(event: &str, fields: &str) {
    if !enabled() {
        return;
    }
    let text = format_line(since_start(), event, fields);
    match printer().and_then(|p| p.lock().ok()) {
        Some(tx) => {
            if let Err(e) = tx.send(Msg::Line(text)) {
                if let Msg::Line(t) = e.0 {
                    eprintln!("{t}");
                }
            }
        }
        None => eprintln!("{text}"),
    }
}

/// Waits (up to a second) until every line sent so far is written; the launcher calls it
/// before the process exits.
pub fn flush() {
    if !enabled() {
        return;
    }
    let (done_tx, done_rx) = channel();
    let sent = printer()
        .and_then(|p| p.lock().ok())
        .is_some_and(|tx| tx.send(Msg::Flush(done_tx)).is_ok());
    if sent {
        let _ = done_rx.recv_timeout(Duration::from_secs(1));
    }
}

/// One trace line (no trailing space when there are no fields).
pub fn format_line(t: Duration, event: &str, fields: &str) -> String {
    if fields.is_empty() {
        format!("perf: t={} {event}", ms(t))
    } else {
        format!("perf: t={} {event} {fields}", ms(t))
    }
}

/// p50, p95 and max of `samples` (milliseconds), nearest rank; `None` when empty.
pub fn percentiles(samples: &[f32]) -> Option<(f32, f32, f32)> {
    let mut v: Vec<f32> = samples.iter().copied().filter(|s| s.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f32::total_cmp);
    let last = v.len().saturating_sub(1);
    let rank = |p: usize| -> f32 {
        // Nearest rank: ceil(p/100 * n) - 1, clamped to the last sample.
        let n = v.len();
        let r = n
            .saturating_mul(p)
            .div_ceil(100)
            .saturating_sub(1)
            .min(last);
        v.get(r).copied().unwrap_or(0.0)
    };
    Some((rank(50), rank(95), v.get(last).copied().unwrap_or(0.0)))
}

/// Frame times by phase (a tab's name, or a bench phase).
#[derive(Debug, Default)]
pub struct Frames {
    by_phase: BTreeMap<String, Vec<f32>>,
}

impl Frames {
    /// Records one frame's time in `phase`.
    pub fn record(&mut self, phase: &str, frame_ms: f32) {
        if let Some(v) = self.by_phase.get_mut(phase) {
            v.push(frame_ms);
        } else {
            self.by_phase.insert(phase.to_string(), vec![frame_ms]);
        }
    }

    /// The summary line's fields for `phase`, or `None` when it has no frames.
    pub fn summary(&self, phase: &str) -> Option<String> {
        let v = self.by_phase.get(phase)?;
        let (p50, p95, max) = percentiles(v)?;
        Some(format!(
            "n={} frame_ms_p50={p50:.2} frame_ms_p95={p95:.2} frame_ms_max={max:.2}",
            v.len()
        ))
    }

    /// Prints one `frames <phase>` line per phase.
    pub fn print_all(&self) {
        for phase in self.by_phase.keys() {
            if let Some(s) = self.summary(phase) {
                line(&format!("frames {phase}"), &s);
            }
        }
    }

    /// Prints and forgets `phase`.
    pub fn flush(&mut self, phase: &str) {
        if let Some(s) = self.summary(phase) {
            line(&format!("frames {phase}"), &s);
        }
        self.by_phase.remove(phase);
    }
}

/// Synthetic wheel input for the bench: while a target rectangle is set, each frame's
/// raw input gets the pointer over it and one wheel step.
#[derive(Debug, Default, Clone, Copy)]
pub struct Wheel {
    /// Where to scroll (the table's rectangle, in points).
    pub target: Option<eframe::egui::Rect>,
    /// Points per frame; negative scrolls the content up (towards later rows).
    pub step: f32,
}

impl Wheel {
    /// Adds the pointer move and the wheel step to `raw` when a target is set.
    pub fn inject(&self, raw: &mut eframe::egui::RawInput) {
        use eframe::egui::{Event, Modifiers, MouseWheelUnit, TouchPhase, Vec2};
        let Some(rect) = self.target else { return };
        raw.events.push(Event::PointerMoved(rect.center()));
        raw.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: Vec2::new(0.0, self.step),
            phase: TouchPhase::Move,
            modifiers: Modifiers::default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_are_nearest_rank() {
        let v: Vec<f32> = (1..=100).map(|i| i as f32).collect();
        assert_eq!(percentiles(&v), Some((50.0, 95.0, 100.0)));
        assert_eq!(percentiles(&[3.0]), Some((3.0, 3.0, 3.0)));
        assert_eq!(percentiles(&[]), None);
        assert_eq!(percentiles(&[f32::NAN, 2.0, 1.0]), Some((1.0, 2.0, 2.0)));
    }

    #[test]
    fn lines_carry_the_time_event_and_fields() {
        let l = format_line(Duration::from_micros(1_234_567), "first-frame", "ui_ms=3.0");
        assert_eq!(l, "perf: t=1234.6 first-frame ui_ms=3.0");
        assert_eq!(
            format_line(Duration::ZERO, "x", ""),
            "perf: t=0.0 x",
            "no trailing space"
        );
    }

    #[test]
    fn frames_summarize_by_phase_and_flush_forgets() {
        let mut f = Frames::default();
        for i in 0..20 {
            f.record("Records", i as f32);
        }
        f.record("Diff", 7.0);
        let s = f.summary("Records").unwrap();
        assert!(
            s.starts_with("n=20 frame_ms_p50=9.00 frame_ms_p95=18.00"),
            "{s}"
        );
        f.flush("Records");
        assert!(f.summary("Records").is_none());
        assert!(f.summary("Diff").is_some());
    }

    #[test]
    fn the_wheel_adds_nothing_without_a_target() {
        let mut raw = eframe::egui::RawInput::default();
        Wheel::default().inject(&mut raw);
        assert!(raw.events.is_empty());
        let w = Wheel {
            target: Some(eframe::egui::Rect::from_min_max(
                eframe::egui::pos2(0.0, 0.0),
                eframe::egui::pos2(10.0, 20.0),
            )),
            step: -40.0,
        };
        w.inject(&mut raw);
        assert_eq!(raw.events.len(), 2);
    }
}
