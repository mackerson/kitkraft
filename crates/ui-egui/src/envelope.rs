//! The part of every result the workbench shows the same way (docs/mcp.md, "Result
//! envelope"): summary, pass/fail, coverage, limitations (never hidden), evidence.
//! Tolerant of missing members: what the envelope does not say is shown as unknown.

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

/// What the window shows for one finished request, built once when the reply arrives so
/// a big envelope (a 1000-row `query`, a `load` of a whole profile) costs nothing per
/// frame: the [`EnvelopeView`] and the pretty-printed raw JSON as lines, which the window
/// lays out only where they are scrolled into view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// The envelope's common members; `None` when the request failed.
    pub view: Option<EnvelopeView>,
    /// `data` (or, for a failure, the `{"error": ...}` JSON), pretty-printed, by line.
    pub raw: Vec<String>,
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
        let mut raw_truncated = false;
        let mut raw = Vec::new();
        for (i, line) in text.lines().enumerate() {
            if i >= RAW_LINES {
                raw_truncated = true;
                break;
            }
            if line.len() > RAW_LINE_CHARS && line.chars().count() > RAW_LINE_CHARS {
                raw_truncated = true;
                let cut: String = line.chars().take(RAW_LINE_CHARS).collect();
                raw.push(format!("{cut}..."));
            } else {
                raw.push(line.to_string());
            }
        }
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
        // The lines' heap before the one-buffer change: each line's String capacity,
        // the 24-byte String header per line, and the Vec's own capacity.
        let heap_bytes: usize = p
            .raw
            .iter()
            .map(|l| l.capacity() + std::mem::size_of::<String>())
            .sum::<usize>()
            + p.raw.capacity() * std::mem::size_of::<String>();
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
