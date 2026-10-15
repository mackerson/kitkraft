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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn a_bare_object_is_unknown_not_complete() {
        let v = EnvelopeView::from_value(&json!({}));
        assert_eq!(v.coverage_status, "unknown");
        assert_eq!(v.passed, None);
        assert!(v.limitations.is_empty());
    }
}
