//! Panel view-models: each builds requests from its inputs and turns the envelopes the
//! engine returns into rows. No egui types and no engine logic: every value shown comes
//! from an envelope, and the only computation is choosing and formatting members.

pub mod diag;
pub mod diff;
pub mod load;
pub mod records;
pub mod schema;
pub mod validate;

use serde_json::Value;

/// Longest value text a cell shows; the raw JSON below each result has the rest.
pub const MAX_CELL: usize = 240;

/// A decoded value as one line. FormIDs (`{raw, form}` from `inspect`, `{form}` from
/// `diff`) show the resolved formref with the raw ID beside it; the null FormID shows as
/// `NULL`; an opaque member shows its byte range; structs and arrays are compact.
///
/// A FormID is recognized by shape (an object with `form` and at most four members):
/// the envelope has no explicit FormID marker yet (docs/gaps.md, the workbench row).
pub fn fmt_value(v: &Value) -> String {
    let mut s = String::new();
    write_value(&mut s, v, 0);
    if s.chars().count() > MAX_CELL {
        let cut: String = s.chars().take(MAX_CELL).collect();
        return format!("{cut}...");
    }
    s
}

fn write_value(out: &mut String, v: &Value, depth: u8) {
    if depth > 16 {
        out.push_str("...");
        return;
    }
    let next = depth.saturating_add(1);
    match v {
        Value::Null => out.push_str("(absent)"),
        Value::String(s) => out.push_str(s),
        Value::Bool(_) | Value::Number(_) => out.push_str(&v.to_string()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(out, item, next);
            }
            out.push(']');
        }
        Value::Object(map) => {
            if let Some(opaque) = map.get("opaque") {
                let at = opaque
                    .get("file_offset")
                    .and_then(Value::as_u64)
                    .map_or_else(|| "?".to_string(), |o| o.to_string());
                let len = opaque
                    .get("length")
                    .and_then(Value::as_u64)
                    .map_or_else(|| "?".to_string(), |o| o.to_string());
                out.push_str(&format!("opaque bytes @{at} len {len}"));
                return;
            }
            if map.contains_key("form") && map.len() <= 4 {
                let raw = map.get("raw").and_then(Value::as_str);
                let form = map.get("form").and_then(Value::as_str);
                let is_null = map.get("null") == Some(&Value::Bool(true));
                match (form, is_null) {
                    (_, true) => out.push_str("NULL"),
                    (Some(f), false) => out.push_str(f),
                    (None, false) => {
                        out.push_str("unresolved");
                        if let Some(e) = map.get("error").and_then(Value::as_str) {
                            out.push_str(&format!(" ({e})"));
                        }
                    }
                }
                if let Some(raw) = raw {
                    out.push_str(&format!(" [raw {raw}]"));
                }
                return;
            }
            out.push('{');
            for (i, (k, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(k);
                out.push_str(": ");
                write_value(out, item, next);
            }
            out.push('}');
        }
    }
}

/// Every FormID inside a decoded value, as `(path, raw, resolved)`; `resolved` is the
/// formref, `NULL`, or `unresolved`. Paths are `Member.Field[2]`.
pub fn formids(v: &Value, path: &str, out: &mut Vec<(String, String, String)>) {
    walk_formids(v, path, out, 0);
}

fn walk_formids(v: &Value, path: &str, out: &mut Vec<(String, String, String)>, depth: u8) {
    if depth > 16 {
        return;
    }
    let next = depth.saturating_add(1);
    match v {
        Value::Object(map) => {
            if let (Some(raw), true) = (
                map.get("raw").and_then(Value::as_str),
                map.contains_key("form"),
            ) {
                let resolved = if map.get("null") == Some(&Value::Bool(true)) {
                    "NULL".to_string()
                } else {
                    map.get("form")
                        .and_then(Value::as_str)
                        .map_or_else(|| "unresolved".to_string(), str::to_string)
                };
                out.push((path.to_string(), raw.to_string(), resolved));
                return;
            }
            for (k, item) in map {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                walk_formids(item, &p, out, next);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                walk_formids(item, &format!("{path}[{i}]"), out, next);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn formids_show_resolved_and_raw_side_by_side() {
        assert_eq!(
            fmt_value(&json!({"raw": "01000801", "form": "Base.esm:000801"})),
            "Base.esm:000801 [raw 01000801]"
        );
        assert_eq!(
            fmt_value(&json!({"raw": "00000000", "form": null, "null": true})),
            "NULL [raw 00000000]"
        );
        assert_eq!(fmt_value(&json!({"form": null, "null": true})), "NULL");
        assert_eq!(
            fmt_value(&json!({"form": null, "raw": "05000800", "error": "no master slot 05"})),
            "unresolved (no master slot 05) [raw 05000800]"
        );
        assert_eq!(
            fmt_value(&json!({"opaque": {"payload_offset": 4, "length": 8, "file_offset": 120}})),
            "opaque bytes @120 len 8"
        );
        assert_eq!(
            fmt_value(&json!({"Speed": 1.5, "Flags": [1, 2]})),
            "{Flags: [1, 2], Speed: 1.5}"
        );
        assert_eq!(fmt_value(&Value::Null), "(absent)");
        assert!(fmt_value(&json!("x".repeat(1000))).ends_with("..."));
    }

    #[test]
    fn formids_are_found_at_any_depth() {
        let mut out = Vec::new();
        formids(
            &json!({"Forms": [{"raw": "00000800", "form": "Test.esm:000800"}, {"raw": "00000000", "form": null, "null": true}], "Count": 2}),
            "KWDA",
            &mut out,
        );
        assert_eq!(
            out,
            vec![
                (
                    "KWDA.Forms[0]".into(),
                    "00000800".into(),
                    "Test.esm:000800".into()
                ),
                ("KWDA.Forms[1]".into(), "00000000".into(), "NULL".into()),
            ]
        );
    }
}
