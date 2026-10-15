//! Diff panel (M4.W3): two plugins through `diff`, or a plugin's overrides against the
//! versions before them through `conflicts`; per-field rows (`fields[]` paths with before
//! and after), opaque members as byte ranges, untyped subrecords as byte spans.

use serde_json::{json, Map, Value};

use super::fmt_value;
use crate::envelope::{list, num, short, text, text_or_empty};
use crate::gate::Request;

/// What to compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    /// `diff a b`: two files, FormIDs matched through their masters.
    #[default]
    TwoPlugins,
    /// `conflicts --plugin`: a plugin's overrides in the loaded order, each against the
    /// version before it (its masters, or an earlier plugin).
    VsMasters,
}

/// The panel's inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffForm {
    /// Mode.
    pub mode: DiffMode,
    /// First plugin (`diff`), or the overriding plugin (`conflicts`).
    pub a: String,
    /// Second plugin (`diff` only).
    pub b: String,
    /// Only this form (`diff`) / only this record type (`conflicts`).
    pub filter: String,
    /// Only forms two or more plugins override (`conflicts`).
    pub only_conflicts: bool,
}

impl DiffForm {
    /// The request for the current mode.
    pub fn request(&self, cursor: Option<&str>) -> Result<Request, String> {
        let mut args = Map::new();
        let filter = self.filter.trim();
        match self.mode {
            DiffMode::TwoPlugins => {
                if self.a.trim().is_empty() || self.b.trim().is_empty() {
                    return Err("give two plugins".to_string());
                }
                args.insert("a".into(), json!(self.a.trim()));
                args.insert("b".into(), json!(self.b.trim()));
                if !filter.is_empty() {
                    args.insert("form".into(), json!(filter));
                }
                args.insert("limit".into(), json!(200));
                Ok(Request::new("diff", Value::Object(args)))
            }
            DiffMode::VsMasters => {
                if self.a.trim().is_empty() {
                    return Err("give the plugin whose overrides to compare".to_string());
                }
                args.insert("plugin".into(), json!(self.a.trim()));
                if !filter.is_empty() {
                    args.insert("type".into(), json!(filter));
                }
                if self.only_conflicts {
                    args.insert("only_conflicts".into(), json!(true));
                }
                args.insert("limit".into(), json!(50));
                if let Some(c) = cursor {
                    args.insert("cursor".into(), json!(c));
                }
                Ok(Request::new("conflicts", Value::Object(args)))
            }
        }
    }
}

/// One compared value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRow {
    /// `SIG#occurrence`.
    pub subrecord: String,
    /// `added`, `removed`, `changed`.
    pub change: String,
    /// Field path (`KWDA.Forms[1]`), or the signature when compared as bytes.
    pub path: String,
    /// Value before (`(absent)` when the member is missing on that side).
    pub before: String,
    /// Value after.
    pub after: String,
}

/// One record that differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDiff {
    /// Formref.
    pub form: String,
    /// Signature.
    pub signature: String,
    /// `differs`, `identical-to-previous`, `deleted`, ... (conflicts), or the diff class.
    pub class: String,
    /// `A.esm -> B.esm` (diff) or `previous -> plugin` (conflicts).
    pub sides: String,
    /// Header flags differ.
    pub flags_differ: bool,
    /// Per-field rows.
    pub rows: Vec<FieldRow>,
}

/// A diff or conflicts result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffView {
    /// `added 1, removed 0, changed 2, unchanged 50` or the per-plugin override counts.
    pub counts: Vec<String>,
    /// Records added in b (`diff`).
    pub added: Vec<String>,
    /// Records removed in b (`diff`).
    pub removed: Vec<String>,
    /// Records that differ.
    pub records: Vec<RecordDiff>,
    /// Next page (`conflicts`).
    pub next_cursor: Option<String>,
}

impl DiffView {
    /// Reads a `diff` or `conflicts` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        if text(env, "command").as_deref() == Some("conflicts") {
            return Self::from_conflicts(data);
        }
        let counts = data.get("counts").map_or_else(Vec::new, |c| {
            c.as_object()
                .map(|m| m.iter().map(|(k, v)| format!("{k} {}", short(v))).collect())
                .unwrap_or_default()
        });
        let entry = |r: &Value| {
            format!(
                "{} {}",
                text_or_empty(r, "signature"),
                text_or_empty(r, "form")
            )
        };
        let a = text_or_empty(data, "a");
        let b = text_or_empty(data, "b");
        let records = list(data, "changed")
            .iter()
            .map(|r| RecordDiff {
                form: text_or_empty(r, "form"),
                signature: text_or_empty(r, "signature"),
                class: text_or_empty(r, "class"),
                sides: format!("{a} -> {b}"),
                flags_differ: r
                    .get("flags_differ")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                rows: field_rows(r),
            })
            .collect();
        Self {
            counts,
            added: list(data, "added").iter().map(entry).collect(),
            removed: list(data, "removed").iter().map(entry).collect(),
            records,
            next_cursor: None,
        }
    }

    fn from_conflicts(data: &Value) -> Self {
        let counts = list(data, "counts")
            .iter()
            .map(|c| {
                format!(
                    "{} overrides {}: {}",
                    text_or_empty(c, "plugin"),
                    text_or_empty(c, "master"),
                    num(c, "total").unwrap_or(0)
                )
            })
            .collect();
        let mut records = Vec::new();
        for item in list(data, "items") {
            for o in list(item, "overrides") {
                records.push(RecordDiff {
                    form: text_or_empty(item, "form"),
                    signature: text_or_empty(item, "signature"),
                    class: text_or_empty(o, "class"),
                    sides: format!(
                        "{} -> {}",
                        text(o, "previous").unwrap_or_else(|| "(none)".to_string()),
                        text_or_empty(o, "plugin")
                    ),
                    flags_differ: o
                        .get("flags_differ")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    rows: field_rows(o),
                });
            }
        }
        Self {
            counts,
            added: Vec::new(),
            removed: Vec::new(),
            records,
            next_cursor: text(data, "next_cursor"),
        }
    }
}

/// The rows of one record's `subrecords[]`: one per `fields[]` entry, or one byte-span
/// row when the subrecord is compared as bytes.
fn field_rows(record: &Value) -> Vec<FieldRow> {
    let mut rows = Vec::new();
    for s in list(record, "subrecords") {
        let sig = text_or_empty(s, "signature");
        let subrecord = format!("{sig}#{}", num(s, "occurrence").unwrap_or(0));
        let change = text_or_empty(s, "change");
        let fields = list(s, "fields");
        if fields.is_empty() {
            let reason = text(s, "as_bytes").map_or_else(String::new, |r| format!(" ({r})"));
            rows.push(FieldRow {
                subrecord,
                change,
                path: format!("{sig} as bytes{reason}"),
                before: span(s.get("a")),
                after: span(s.get("b")),
            });
            continue;
        }
        for f in fields {
            rows.push(FieldRow {
                subrecord: subrecord.clone(),
                change: change.clone(),
                path: text_or_empty(f, "path"),
                before: fmt_value(f.get("before").unwrap_or(&Value::Null)),
                after: fmt_value(f.get("after").unwrap_or(&Value::Null)),
            });
        }
    }
    rows
}

/// `bytes @file_offset len n` for a diff span; `(absent)` when the side has no such
/// subrecord (`null`); `bytes (range not reported)` when the result carries no spans
/// (`conflicts` does not): unknown is never shown as absent.
fn span(v: Option<&Value>) -> String {
    match v {
        Some(s) if s.is_object() => format!(
            "bytes @{} len {}",
            num(s, "file_offset").map_or_else(|| "?".to_string(), |n| n.to_string()),
            num(s, "length").map_or_else(|| "?".to_string(), |n| n.to_string())
        ),
        Some(_) => "(absent)".to_string(),
        None => "bytes (range not reported)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_per_mode() {
        let mut form = DiffForm {
            a: "A.esm".into(),
            b: "B.esm".into(),
            ..DiffForm::default()
        };
        assert_eq!(
            form.request(None).unwrap().args,
            json!({"a": "A.esm", "b": "B.esm", "limit": 200})
        );
        form.mode = DiffMode::VsMasters;
        form.filter = "WEAP".into();
        form.only_conflicts = true;
        let req = form.request(Some("c1")).unwrap();
        assert_eq!(req.id, "conflicts");
        assert_eq!(
            req.args,
            json!({"plugin": "A.esm", "type": "WEAP", "only_conflicts": true, "limit": 50, "cursor": "c1"})
        );
        assert!(DiffForm::default().request(None).is_err());
    }

    #[test]
    fn diff_rows_show_fields_opaque_ranges_and_byte_spans() {
        let env = json!({"command": "diff", "data": {"a": "A.esm", "b": "B.esm",
        "counts": {"added": 1, "removed": 0, "changed": 1, "unchanged": 3},
        "added": [{"form": "B.esm:000900", "signature": "GLOB", "raw_form_id": "00000900", "offset": 10}],
        "removed": [],
        "changed": [{"form": "A.esm:000800", "signature": "WEAP", "class": "differs", "flags_differ": false,
            "subrecords": [
                {"signature": "DNAM", "occurrence": 0, "change": "changed", "member": "Data",
                 "fields": [
                    {"path": "DNAM.Speed", "before": 1.0, "after": 1.5},
                    {"path": "DNAM.Rest", "before": {"opaque": {"payload_offset": 8, "length": 4, "file_offset": 300}}, "after": null}
                 ]},
                {"signature": "KWDA", "occurrence": 0, "change": "changed",
                 "fields": [{"path": "KWDA.Forms[1]", "before": {"form": "A.esm:000801"}, "after": {"form": null, "null": true}}]},
                {"signature": "XXXX", "occurrence": 1, "change": "added", "as_bytes": "no layout",
                 "a": null, "b": {"file_offset": 500, "body_offset": 20, "length": 6}}
            ]}]}});
        let view = DiffView::from_envelope(&env);
        assert_eq!(
            view.counts,
            vec!["added 1", "changed 1", "removed 0", "unchanged 3"]
        );
        assert_eq!(view.added, vec!["GLOB B.esm:000900"]);
        let rows = &view.records[0].rows;
        assert_eq!(rows.len(), 4);
        assert_eq!(
            (rows[0].before.as_str(), rows[0].after.as_str()),
            ("1.0", "1.5")
        );
        assert_eq!(rows[1].before, "opaque bytes @300 len 4");
        assert_eq!(rows[1].after, "(absent)");
        assert_eq!(rows[2].after, "NULL");
        assert_eq!(rows[3].path, "XXXX as bytes (no layout)");
        assert_eq!(
            (rows[3].before.as_str(), rows[3].after.as_str()),
            ("(absent)", "bytes @500 len 6")
        );
        assert_eq!(view.records[0].sides, "A.esm -> B.esm");
    }

    #[test]
    fn conflicts_rows_are_one_record_per_override() {
        let env = json!({"command": "conflicts", "data": {
            "counts": [{"master": "Base.esm", "plugin": "Mod.esp", "total": 2, "types": {"GLOB": 2}}],
            "next_cursor": "c2",
            "items": [{"form": "Base.esm:000800", "signature": "GLOB", "editor_id": "TestGlobal",
                "overrides": [{"plugin": "Mod.esp", "previous": "Base.esm", "class": "differs", "flags_differ": false,
                    "subrecords": [{"signature": "FLTV", "occurrence": 0, "change": "changed",
                        "fields": [{"path": "FLTV", "before": 1.0, "after": 2.0}]},
                        {"signature": "NVMI", "occurrence": 0, "change": "changed"}]}]}]}});
        let view = DiffView::from_envelope(&env);
        assert_eq!(view.counts, vec!["Mod.esp overrides Base.esm: 2"]);
        assert_eq!(view.records[0].sides, "Base.esm -> Mod.esp");
        assert_eq!(view.records[0].rows[0].after, "2.0");
        // `conflicts` carries no byte spans: unknown, not absent.
        assert_eq!(view.records[0].rows[1].before, "bytes (range not reported)");
        assert_eq!(view.next_cursor.as_deref(), Some("c2"));
    }
}
