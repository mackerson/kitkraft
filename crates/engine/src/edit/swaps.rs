//! `swap_overrides`: the material-swap edit of an `LMSW`, taken by `set` and `clone`.
//!
//! The argument is a JSON array of `{"key": <path>, "override": <path>}` objects. Only an
//! `LMSW` source may take it (any other type is refused, naming its signature); each
//! `override` must end in `.mat` (ASCII, case-insensitive); forward slashes in both paths
//! are stored as backslashes, as the corpus stores them. A key that names no entry, or
//! more than one, is refused by `refl::swap_overrides` naming it; naming the same key
//! twice in one argument lets the last override win.
//!
//! [`convert`] reads the source record's current `REFL`, rewrites the named entries'
//! overrides through `kitkraft_doc::refl::swap_overrides`, and returns an ordinary
//! whole-subrecord edit of `REFL` — `Op::Set` of `hex:` of the new bytes — so `dry_run`,
//! `validate` and the rest of the write path see no difference from a hand-written
//! `fields: {"REFL": "hex:..."}`. The result of `set` and `clone` gains
//! `swap_overrides: [{key, from, to}]`, one item per argument entry: `key` and `to` as
//! stored, `from` the override the entry had before. A `swap_overrides` together with a
//! `fields` edit of `REFL` is refused: both rewrite the whole `REFL` subrecord.

use kitkraft_doc::refl;
use kitkraft_esm::{Record, Signature};
use serde_json::{json, Value};

use super::fields::{parse_path, to_hex, Op};
use super::patch::Edit;
use crate::exec::EngineError;

/// The `swap_overrides` argument, as a whole-`REFL` edit and the result report.
pub(crate) fn convert(
    command: &str,
    record: &Record<'_>,
    swaps: &Value,
    edits: &[Edit],
) -> Result<(Edit, Vec<Value>), EngineError> {
    if record.header.signature != Signature(*b"LMSW") {
        return Err(failed(
            command,
            format!(
                "swap_overrides: the source is a {}; swap_overrides edits only the material swap of an LMSW",
                record.header.signature
            ),
        ));
    }
    if edits.iter().any(|e| e.path.signature == *b"REFL") {
        return Err(failed(
            command,
            "swap_overrides and a fields edit of REFL together: both rewrite the whole REFL subrecord",
        ));
    }
    let Some(index) = record.find_subrecord(Signature(*b"REFL"), 0) else {
        return Err(failed(
            command,
            "swap_overrides: the LMSW has no REFL subrecord",
        ));
    };
    let payload = record
        .subrecords()
        .and_then(|subs| subs.get(index))
        .map(|s| s.data().to_vec())
        .ok_or_else(|| failed(command, "swap_overrides: the LMSW has no REFL payload"))?;
    let refl =
        refl::decode(&payload).map_err(|e| failed(command, format!("swap_overrides: {e}")))?;
    let entries =
        refl::lmsw_entries(&refl).map_err(|e| failed(command, format!("swap_overrides: {e}")))?;
    let items = swaps
        .as_array()
        .ok_or_else(|| failed(command, "swap_overrides: the argument is not an array"))?;
    let mut pairs: Vec<(String, String)> = Vec::with_capacity(items.len());
    let mut report: Vec<Value> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let key = item
            .get("key")
            .and_then(Value::as_str)
            .map(|s| s.replace('/', "\\"))
            .ok_or_else(|| {
                failed(
                    command,
                    format!("swap_overrides: entry {i} has no `key` string"),
                )
            })?;
        let to = item
            .get("override")
            .and_then(Value::as_str)
            .map(|s| s.replace('/', "\\"))
            .ok_or_else(|| {
                failed(
                    command,
                    format!("swap_overrides: entry {i} has no `override` string"),
                )
            })?;
        if !to.to_ascii_lowercase().ends_with(".mat") {
            return Err(failed(
                command,
                format!(
                    "swap_overrides: the override for key {key} must end in .mat (ASCII, case-insensitive); got {to}"
                ),
            ));
        }
        let from = entries.iter().find_map(|e| match e {
            refl::SwapEntry::Swap {
                key: k, r#override, ..
            } if k == &key => Some(r#override.clone()),
            _ => None,
        });
        pairs.push((key.clone(), to.clone()));
        report.push(json!({ "key": key, "from": from.as_ref(), "to": to }));
    }
    let next = refl::swap_overrides(&refl, &pairs)
        .map_err(|e| failed(command, format!("swap_overrides: {e}")))?;
    let bytes = refl::encode(&next);
    let path = parse_path("REFL").map_err(|e| failed(command, format!("swap_overrides: {e}")))?;
    Ok((
        Edit {
            path,
            op: Op::Set(Value::String(format!("hex:{}", to_hex(&bytes)))),
        },
        report,
    ))
}

fn failed(command: &str, message: impl Into<String>) -> EngineError {
    EngineError::Failed {
        command: command.to_string(),
        message: message.into(),
    }
}
