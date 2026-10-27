//! The part of every result the workbench shows the same way (docs/mcp.md, "Result
//! envelope"): summary, pass/fail, coverage, limitations (never hidden), evidence.
//! Tolerant of missing members: what the envelope does not say is shown as unknown.

use std::mem::size_of;
use std::ops::Range;

use serde_json::Value;

/// One evidence range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRow {
    /// File name as loaded.
    pub file: String,
    /// Absolute byte offset.
    pub offset: u64,
    /// Length in bytes.
    pub length: u64,
    /// `SIGN:FORMID`, or empty.
    pub record: String,
    /// Subrecord signature, or empty.
    pub subrecord: String,
}

/// The envelope's `build` member (M6.P6b): which build of the engine produced it. The
/// window and the engine are one process, so it names the window's build too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    /// The workspace version.
    pub version: String,
    /// Short commit hash (`-dirty` for a tree with changes), or `unknown`.
    pub commit: String,
    /// Build date (UTC), or `unknown`.
    pub date: String,
    /// `debug` or `release`.
    pub profile: String,
}

impl BuildInfo {
    /// Reads `env.build`; `None` when the envelope has no such object. A missing member
    /// reads as `unknown`, never as a guess.
    pub fn from_envelope(env: &Value) -> Option<Self> {
        let b = env.get("build").filter(|b| b.is_object())?;
        let known = |k: &str| {
            text(b, k)
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "unknown".to_string())
        };
        Some(Self {
            version: known("version"),
            commit: known("commit"),
            date: known("date"),
            profile: known("profile"),
        })
    }

    /// `0.0.0 (5d03d01, 2026-10-16, release)`, as `kit --version` prints it after `kit `.
    pub fn line(&self) -> String {
        format!(
            "{} ({}, {}, {})",
            self.version, self.commit, self.date, self.profile
        )
    }

    /// A debug build (far slower than release; `cargo xtask ui` builds release).
    pub fn is_debug(&self) -> bool {
        self.profile == "debug"
    }
}

/// The common view of one envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeView {
    /// Canonical command id.
    pub command: String,
    /// `data.summary`.
    pub summary: Option<String>,
    /// `data.passed` (absent or null: no pass/fail claim).
    pub passed: Option<bool>,
    /// `complete`, `partial`, `unknown`.
    pub coverage_status: String,
    /// Decoded subrecords (`None`: not counted).
    pub decoded: Option<u64>,
    /// Opaque subrecords (`None`: not counted).
    pub opaque: Option<u64>,
    /// Inputs not examined.
    pub not_examined: Vec<String>,
    /// Limitations, in order.
    pub limitations: Vec<String>,
    /// Evidence ranges, in order.
    pub evidence: Vec<EvidenceRow>,
    /// The build that produced it (`None`: the envelope does not say).
    pub build: Option<BuildInfo>,
}

/// `v[key]` as text, if it is a string.
pub(crate) fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// `v[key]` as text, or empty.
pub(crate) fn text_or_empty(v: &Value, key: &str) -> String {
    text(v, key).unwrap_or_default()
}

/// `v[key]` as an unsigned number.
pub(crate) fn num(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}

/// `v[key]` as a list (empty when absent).
pub(crate) fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// `v[key]` as a list of strings.
pub(crate) fn strings(v: &Value, key: &str) -> Vec<String> {
    list(v, key)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// A JSON value as one short line: strings bare, `null` as `null`, the rest compact.
pub fn short(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl EnvelopeView {
    /// Reads the envelope members of `env`.
    pub fn from_value(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let coverage = env.get("coverage").unwrap_or(&Value::Null);
        let evidence = list(env, "evidence")
            .iter()
            .map(|e| EvidenceRow {
                file: text_or_empty(e, "file"),
                offset: num(e, "offset").unwrap_or(0),
                length: num(e, "length").unwrap_or(0),
                record: text_or_empty(e, "record"),
                subrecord: text_or_empty(e, "subrecord"),
            })
            .collect();
        Self {
            command: text_or_empty(env, "command"),
            summary: text(data, "summary"),
            passed: data.get("passed").and_then(Value::as_bool),
            coverage_status: text(coverage, "status").unwrap_or_else(|| "unknown".to_string()),
            decoded: num(coverage, "decoded_subrecords"),
            opaque: num(coverage, "opaque_subrecords"),
            not_examined: strings(coverage, "not_examined"),
            limitations: strings(env, "limitations"),
            evidence,
            build: BuildInfo::from_envelope(env),
        }
    }

    /// `partial: 14 decoded, 2 opaque` (`not counted` for a null count).
    pub fn coverage_line(&self) -> String {
        let count = |n: Option<u64>| n.map_or_else(|| "not counted".to_string(), |n| n.to_string());
        let mut line = format!(
            "{}: decoded {}, opaque {}",
            self.coverage_status,
            count(self.decoded),
            count(self.opaque)
        );
        if !self.not_examined.is_empty() {
            line.push_str(&format!("; not examined: {}", self.not_examined.join(", ")));
        }
        line
    }
}

/// Lines of raw JSON kept per result; the MCP call or `kit --json` has the rest.
pub const RAW_LINES: usize = 500_000;
/// Characters kept per raw JSON line (a long string value is cut).
pub const RAW_LINE_CHARS: usize = 2_000;

/// The kept raw JSON lines in one buffer (M6.P5a): a single `String` holding every
/// kept line (already cut at [`RAW_LINE_CHARS`]), each followed by `\n`, and a
/// `Vec<usize>` of the byte start of each line, so a line is a `&str` slice of the
/// buffer. One buffer and one offset list instead of one heap string per line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLines {
    /// Every kept line, each followed by `\n`.
    buffer: String,
    /// The byte start of each kept line in `buffer`.
    starts: Vec<usize>,
}

impl RawLines {
    /// The kept lines of the pretty-printed JSON `text`: up to [`RAW_LINES`] lines,
    /// each cut at [`RAW_LINE_CHARS`] characters (a cut line ends with `...`). The
    /// second result is true when a line was cut or lines were left out.
    fn from_pretty(text: &str) -> (Self, bool) {
        let mut buffer = String::new();
        let mut starts = Vec::new();
        let mut truncated = false;
        for (i, line) in text.lines().enumerate() {
            if i >= RAW_LINES {
                truncated = true;
                break;
            }
            starts.push(buffer.len());
            if line.len() > RAW_LINE_CHARS && line.chars().count() > RAW_LINE_CHARS {
                truncated = true;
                // Cut on a character boundary: the first RAW_LINE_CHARS chars, then `...`.
                let mut cut = String::with_capacity(RAW_LINE_CHARS + 3);
                cut.extend(line.chars().take(RAW_LINE_CHARS));
                cut.push_str("...");
                buffer.push_str(&cut);
            } else {
                buffer.push_str(line);
            }
            buffer.push('\n');
        }
        // No slack in what is kept: the buffer and the offsets fit their lines exactly.
        buffer.shrink_to_fit();
        starts.shrink_to_fit();
        (Self { buffer, starts }, truncated)
    }

    /// The number of kept lines.
    pub fn len(&self) -> usize {
        self.starts.len()
    }

    /// No kept lines.
    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }

    /// The lines the range covers, in order, as slices of the buffer. `None` when the
    /// range is out of bounds, as `Vec::get`.
    pub fn get(&self, range: Range<usize>) -> Option<Vec<&str>> {
        let n = self.starts.len();
        if range.start > range.end || range.start > n || range.end > n {
            return None;
        }
        Some((range.start..range.end).map(|i| self.line(i)).collect())
    }

    /// Every kept line, in order.
    pub fn iter(&self) -> impl Iterator<Item = &str> + '_ {
        (0..self.starts.len()).map(|i| self.line(i))
    }

    /// The heap held, in bytes: the buffer's and the offsets' capacities.
    pub fn heap_bytes(&self) -> usize {
        // The sum cannot overflow (each allocation is at most isize::MAX bytes),
        // but the checked forms keep it so without a panic.
        self.starts
            .capacity()
            .checked_mul(size_of::<usize>())
            .and_then(|offsets| self.buffer.capacity().checked_add(offsets))
            .unwrap_or(usize::MAX)
    }

    /// Line `i` as a slice of the buffer, or empty when `i` is out of bounds.
    fn line(&self, i: usize) -> &str {
        let s = match self.starts.get(i) {
            Some(&s) => s,
            None => return "",
        };
        // Every kept line is followed by a `\n`, so its content ends one byte before
        // the next line's start (or the buffer's end).
        let e = match i.checked_add(1).and_then(|j| self.starts.get(j).copied()) {
            Some(e) => e,
            None => self.buffer.len(),
        };
        self.buffer.get(s..e.saturating_sub(1)).unwrap_or("")
    }
}

/// What the window shows for one finished request, built once when the reply arrives so
/// a big envelope (a 1000-row `query`, a `load` of a whole profile) costs nothing per
/// frame: the [`EnvelopeView`] and the pretty-printed raw JSON as lines, which the window
/// lays out only where they are scrolled into view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// The envelope's common members; `None` when the request failed.
    pub view: Option<EnvelopeView>,
    /// `data` (or, for a failure, the `{"error": ...}` JSON), pretty-printed, in one
    /// buffer of lines.
    pub raw: RawLines,
    /// Lines left out after [`RAW_LINES`], or lines cut at [`RAW_LINE_CHARS`].
    pub raw_truncated: bool,
}

impl Prepared {
    /// Builds the view and the raw lines of `env` (an envelope) or of `error_json`.
    pub fn new(env: Option<&Value>, error_json: Option<&Value>) -> Self {
        let view = env.map(EnvelopeView::from_value);
        let raw_value = match env {
            Some(env) => env.get("data"),
            None => error_json,
        };
        let text = raw_value
            .and_then(|v| serde_json::to_string_pretty(v).ok())
            .unwrap_or_default();
        let (raw, raw_truncated) = RawLines::from_pretty(&text);
        Self {
            view,
            raw,
            raw_truncated,
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_big_envelope_is_prepared_once_as_view_and_lines() {
        let rows: Vec<Value> = (0..20_000)
            .map(|i| json!({"form": format!("A.esm:{i:06X}"), "edid": "x".repeat(10)}))
            .collect();
        let evidence: Vec<Value> = (0..20_000)
            .map(|i| json!({"file": "A.esm", "offset": i, "length": 4}))
            .collect();
        let env = json!({"command": "query", "data": {"summary": "20000 matched", "rows": rows, "long": "y".repeat(5000)},
            "coverage": {"status": "complete"}, "limitations": [], "evidence": evidence});
        let p = Prepared::new(Some(&env), None);
        let view = p.view.as_ref().unwrap();
        assert_eq!(view.evidence.len(), 20_000);
        assert_eq!(view.summary.as_deref(), Some("20000 matched"));
        // `{`, `"long"`, `"rows": [`, 4 lines per row, `]`, `"summary"`, `}`.
        assert_eq!(p.raw.len(), 6 + 4 * 20_000);
        assert!(p.raw_truncated, "the 5000-character line is cut");
        assert!(p
            .raw
            .iter()
            .all(|l| l.chars().count() <= RAW_LINE_CHARS + 3));

        let failed = Prepared::new(None, Some(&json!({"error": {"kind": "not_found"}})));
        assert!(failed.view.is_none());
        assert!(failed.raw.iter().any(|l| l.contains("not_found")));
        assert!(!failed.raw_truncated);
    }

    #[test]
    fn raw_lines_share_one_buffer() {
        // A few hundred short rows plus one multi-byte line over RAW_LINE_CHARS.
        let rows: Vec<Value> = (0..500)
            .map(|i| json!({"form": format!("A.esm:{i:06X}"), "edid": "x".repeat(20)}))
            .collect();
        let long = "é".repeat(RAW_LINE_CHARS + 10);
        let env = json!({"command": "query", "data": {"rows": rows, "long": long},
            "coverage": {"status": "complete"}, "limitations": [], "evidence": []});
        let p = Prepared::new(Some(&env), None);
        // The lines live in one buffer: less than the bytes the same lines would take
        // as separate Strings (each line's String capacity, the 24-byte String header
        // per line, and the Vec's own capacity).
        let separate: usize = p
            .raw
            .iter()
            .map(|l| l.to_string().capacity() + size_of::<String>())
            .sum::<usize>()
            + p.raw.len() * size_of::<String>();
        assert!(
            p.raw.heap_bytes() < separate,
            "one buffer: {} !< {}",
            p.raw.heap_bytes(),
            separate
        );
        // The first two lines, as &str slices of the buffer; out of bounds is None.
        let two = p.raw.get(0..2).unwrap_or_default();
        assert_eq!(two.len(), 2);
        assert_eq!(two.first().copied(), Some("{"));
        // The second line as `get` returns it, as `iter` sees it: the same slice.
        assert_eq!(two.get(1).copied(), p.raw.iter().nth(1));
        assert!(p.raw.get(p.raw.len()..p.raw.len() + 1).is_none());
        assert!(p.raw.get(999_999..1_000_000).is_none());
        // The long line is cut at a character boundary, on its first RAW_LINE_CHARS
        // characters, and ends with `...`.
        let cut_line = p.raw.iter().find(|l| l.ends_with("...")).unwrap_or("");
        let prefix = "  \"long\": \"";
        assert_eq!(
            cut_line,
            format!("{prefix}{}...", "é".repeat(RAW_LINE_CHARS - prefix.len()))
        );
    }

    #[test]
    fn reads_every_member_and_reports_unknown_counts_as_not_counted() {
        let env = json!({
            "command": "roundtrip",
            "data": {"summary": "Test.esm: identical", "passed": true},
            "coverage": {"status": "complete", "decoded_subrecords": null, "opaque_subrecords": 3, "not_examined": ["X.esm"]},
            "limitations": ["container level"],
            "evidence": [{"file": "Test.esm", "offset": 10, "length": 4, "record": "GLOB:00000800", "subrecord": null}]
        });
        let v = EnvelopeView::from_value(&env);
        assert_eq!(v.command, "roundtrip");
        assert_eq!(v.passed, Some(true));
        assert_eq!(v.summary.as_deref(), Some("Test.esm: identical"));
        assert_eq!(
            v.coverage_line(),
            "complete: decoded not counted, opaque 3; not examined: X.esm"
        );
        assert_eq!(v.limitations, vec!["container level"]);
        assert_eq!(v.evidence.len(), 1);
        assert_eq!(v.evidence[0].record, "GLOB:00000800");
        assert_eq!(v.evidence[0].subrecord, "");
    }

    /// M6.V5 / M6.P6a: the About line and the title bar read the envelope's `build`
    /// member; the engine's own envelope (through the gate) carries it.
    #[test]
    fn the_build_member_is_read_from_the_engines_envelope() {
        let engine = crate::gate::ReadOnlyEngine::new();
        let env = engine
            .run(&crate::gate::Request::new("schema", json!({})))
            .unwrap();
        let b = EnvelopeView::from_value(&env)
            .build
            .expect("a build member");
        assert_eq!(
            b.profile,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        assert_eq!(b.is_debug(), cfg!(debug_assertions));
        assert!(
            b.line().starts_with(&format!("{} (", b.version)),
            "{}",
            b.line()
        );
        let partial =
            BuildInfo::from_envelope(&json!({"build": {"version": "1.2.3", "profile": "release"}}))
                .unwrap();
        assert_eq!(partial.line(), "1.2.3 (unknown, unknown, release)");
        assert!(!partial.is_debug());
        assert_eq!(BuildInfo::from_envelope(&json!({"build": "x"})), None);
        assert_eq!(EnvelopeView::from_value(&json!({})).build, None);
    }

    /// M6.P5a bench: the raw lines of a ~30 MB synthetic envelope, `Prepared::new`
    /// timed (median of 5 release runs). Ignored: run with
    /// `cargo test -p kitkraft-ui-egui --release --lib -- --ignored --exact
    /// envelope::tests::raw_bench --nocapture`.
    #[test]
    #[ignore]
    fn raw_bench() {
        let rows: Vec<Value> = (0..150_000)
            .map(|i| {
                json!({
                    "form": format!("A.esm:{i:06X}"),
                    "edid": "x".repeat(120),
                    "note": "y".repeat(40)
                })
            })
            .collect();
        let env = json!({"command": "query", "data": {"summary": "150000 matched", "rows": rows},
            "coverage": {"status": "complete"}, "limitations": [], "evidence": []});
        let mut ms: Vec<u128> = Vec::new();
        let mut p = Prepared::new(Some(&env), None);
        for _ in 0..5 {
            let t = std::time::Instant::now();
            p = Prepared::new(Some(&env), None);
            ms.push(t.elapsed().as_nanos());
        }
        ms.sort_unstable_by_key(|n| *n);
        let Some(&median) = ms.get(2) else {
            return;
        };
        let lines = p.raw.len();
        // The lines' heap: the one buffer and the offsets (M6.P5a).
        let heap_bytes = p.raw.heap_bytes();
        println!(
            "raw-bench: lines={lines} heap_bytes={heap_bytes} median_ms={}",
            median as f64 / 1.0e6
        );
    }

    #[test]
    fn a_bare_object_is_unknown_not_complete() {
        let v = EnvelopeView::from_value(&json!({}));
        assert_eq!(v.coverage_status, "unknown");
        assert_eq!(v.passed, None);
        assert!(v.limitations.is_empty());
    }
}
