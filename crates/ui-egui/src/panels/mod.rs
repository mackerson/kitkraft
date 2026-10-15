//! Panel view-models: each builds requests from its inputs and turns the envelopes the
//! engine returns into rows. No egui types and no engine logic: every value shown comes
//! from an envelope, and the only computation is choosing and formatting members.

pub mod diag;
pub mod diff;
pub mod links;
pub mod load;
pub mod planet;
pub mod records;
pub mod schema;
pub mod start;
pub mod validate;

use serde_json::{Map, Value};

use crate::gate::{marker, KIND};

/// Longest value text a cell shows; the raw JSON below each result has the rest.
pub const MAX_CELL: usize = 240;

/// The marker of an object (`docs/mcp.md`, "Data shapes and markers"): its `kind` when
/// that is one of the engine's marker names, else `None` (a struct).
pub fn marker_of(map: &Map<String, Value>) -> Option<&'static str> {
    let k = map.get(KIND).and_then(Value::as_str)?;
    marker::ALL.iter().copied().find(|m| *m == k)
}

/// A `formref` marker as text: the formref, `NULL`, or `unresolved (why)`, with the
/// stored FormID beside it when the command read it.
fn formref_text(map: &Map<String, Value>) -> String {
    let mut out = if map.get("null") == Some(&Value::Bool(true)) {
        "NULL".to_string()
    } else {
        match map.get("form").and_then(Value::as_str) {
            Some(f) => f.to_string(),
            None => match map.get("error").and_then(Value::as_str) {
                Some(e) => format!("unresolved ({e})"),
                None => "unresolved".to_string(),
            },
        }
    };
    if let Some(raw) = map.get("raw").and_then(Value::as_str) {
        out.push_str(&format!(" [raw {raw}]"));
    }
    out
}

/// A decoded value as one line, formatted by marker: a `formref` shows the resolved
/// formref with the raw ID beside it (`NULL` for the null FormID, `unresolved` with the
/// reason), an `lstring` its string ID, an `opaque` member its byte range (or its bytes),
/// a `link` its path and target; structs and arrays are compact. An object without a
/// marker is a struct, whatever members it has.
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
        Value::Object(map) => match marker_of(map) {
            Some(marker::FORMREF) => out.push_str(&formref_text(map)),
            Some(marker::LSTRING) => {
                let id = map.get("string_id").and_then(Value::as_str).unwrap_or("?");
                out.push_str(&format!("(string {id})"));
            }
            Some(marker::OPAQUE) => match map.get("opaque") {
                Some(Value::String(hex)) => out.push_str(&format!("opaque bytes {hex}")),
                Some(range) => {
                    let n = |k: &str| {
                        range
                            .get(k)
                            .and_then(Value::as_u64)
                            .map_or_else(|| "?".to_string(), |o| o.to_string())
                    };
                    out.push_str(&format!(
                        "opaque bytes @{} len {}",
                        n("file_offset"),
                        n("length")
                    ));
                }
                None => out.push_str("opaque bytes"),
            },
            Some(marker::LINK) => out.push_str(&planet::link_text(v)),
            _ => {
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
        },
    }
}

/// Every `formref` marker inside a decoded value, as `(path, raw, resolved)`; `resolved`
/// is the formref, `NULL`, or `unresolved`. Paths are `Member.Field[2]`.
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
            match marker_of(map) {
                Some(marker::FORMREF) => {
                    let resolved = if map.get("null") == Some(&Value::Bool(true)) {
                        "NULL".to_string()
                    } else {
                        map.get("form")
                            .and_then(Value::as_str)
                            .map_or_else(|| "unresolved".to_string(), str::to_string)
                    };
                    let raw = map
                        .get("raw")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    out.push((path.to_string(), raw, resolved));
                    return;
                }
                Some(_) => return,
                None => {}
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
    fn formrefs_show_resolved_and_raw_side_by_side() {
        assert_eq!(
            fmt_value(&json!({"kind": "formref", "raw": "01000801", "form": "Base.esm:000801"})),
            "Base.esm:000801 [raw 01000801]"
        );
        assert_eq!(
            fmt_value(&json!({"kind": "formref", "raw": "00000000", "form": null, "null": true})),
            "NULL [raw 00000000]"
        );
        assert_eq!(
            fmt_value(&json!({"kind": "formref", "form": null, "null": true})),
            "NULL"
        );
        assert_eq!(
            fmt_value(
                &json!({"kind": "formref", "form": null, "raw": "05000800", "error": "no master slot 05"})
            ),
            "unresolved (no master slot 05) [raw 05000800]"
        );
        assert_eq!(
            fmt_value(
                &json!({"kind": "opaque", "opaque": {"payload_offset": 4, "length": 8, "file_offset": 120}})
            ),
            "opaque bytes @120 len 8"
        );
        assert_eq!(
            fmt_value(&json!({"kind": "opaque", "opaque": "0108"})),
            "opaque bytes 0108"
        );
        assert_eq!(
            fmt_value(&json!({"kind": "lstring", "string_id": "0x00000010"})),
            "(string 0x00000010)"
        );
        assert_eq!(
            fmt_value(&json!({"Speed": 1.5, "Flags": [1, 2]})),
            "{Flags: [1, 2], Speed: 1.5}"
        );
        assert_eq!(fmt_value(&Value::Null), "(absent)");
        assert!(fmt_value(&json!("x".repeat(1000))).ends_with("..."));
    }

    /// The workbench keys on the marker, not the shape (the docs/gaps.md row this closes):
    /// a struct that happens to have a `form` member stays a struct, and an object whose
    /// `kind` is no marker name is a struct too.
    #[test]
    fn an_unmarked_object_is_a_struct_whatever_its_members() {
        assert_eq!(
            fmt_value(&json!({"form": "Base.esm:000801", "raw": "00000801"})),
            "{form: Base.esm:000801, raw: 00000801}"
        );
        assert_eq!(fmt_value(&json!({"kind": "Rifle"})), "{kind: Rifle}");
        let mut out = Vec::new();
        formids(
            &json!({"Pair": {"form": "Base.esm:000801", "raw": "00000801"}}),
            "X",
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn formids_are_found_at_any_depth() {
        let mut out = Vec::new();
        formids(
            &json!({"Forms": [{"kind": "formref", "raw": "00000800", "form": "Test.esm:000800"},
                              {"kind": "formref", "raw": "00000000", "form": null, "null": true}],
                    "Count": 2, "Rest": {"kind": "opaque", "opaque": {"payload_offset": 0, "length": 4, "file_offset": null}}}),
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
