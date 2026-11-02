//! The `set` dialog's requests (M7.E2, pipeline E; ADR 0007 Amendment 4, model A): the
//! edit dialog's changed fields of one record, written into the active file one
//! confirmed write per OK click.
//!
//! OK sends [`preview_request`], `set --dry-run` (M7.S6): the engine writes a staged copy
//! of the active file, runs `validate` on it and answers with the record before and after,
//! the member paths changed and the findings, leaving the active file untouched. The
//! confirm box shows that answer; its one confirm click sends [`write_request`], the same
//! arguments without `dry_run`. This file is the only one that builds a confirmed `set`
//! (the gate's per-entry source scan holds it). [`readback_request`] reads the written
//! record back from the active file alone.

use serde_json::{json, Map, Value};

use crate::gate::Request;

/// The preview of writing `fields` (field path -> value, as `set` takes them) of `form`
/// into `into`, the active file: a dry run, which the gate passes like a read.
#[allow(dead_code)] // the edit dialog sends it from the next commit (M7.E2)
pub(crate) fn preview_request(form: &str, fields: &Map<String, Value>, into: &str) -> Request {
    Request::new(
        "set",
        json!({"form": form, "fields": fields, "into": into, "dry_run": true}),
    )
}

/// The confirmed write of a previewed `set`: the preview's arguments with `dry_run`
/// dropped, built once per confirm click. `None` when `preview` is not a `set` dry run.
#[allow(dead_code)] // the confirm box sends it from the next commit (M7.E2)
pub(crate) fn write_request(preview: &Request) -> Option<Request> {
    if preview.id != "set" || preview.args.get("dry_run") != Some(&Value::Bool(true)) {
        return None;
    }
    let mut args = preview.args.clone();
    args.as_object_mut()?.remove("dry_run");
    Some(Request::confirmed("set", args))
}

/// The record a confirmed `set` wrote, read back from `into` alone (`inspect` in
/// single-plugin mode, by the FormID as the write's envelope says it is stored there).
/// `None` when the envelope names no written record.
#[allow(dead_code)] // the confirm box sends it from the next commit (M7.E2)
pub(crate) fn readback_request(written: &Value, into: &str) -> Option<Request> {
    let form_id = written
        .pointer("/data/records")?
        .as_array()?
        .iter()
        .find(|r| r.get("parent") != Some(&Value::Bool(true)))?
        .get("form_id")?
        .as_str()?;
    Some(Request::new(
        "inspect",
        json!({"form": form_id, "plugin": into}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::check;

    #[test]
    fn the_write_is_the_preview_without_dry_run_and_confirmed() {
        let mut fields = Map::new();
        fields.insert("FLTV".into(), json!(7.25));
        let preview = preview_request("Fixture.esm:000800", &fields, r"C:\out\Patch.esp");
        assert!(!preview.is_confirmed());
        assert!(check(&preview).is_ok(), "the dry run passes like a read");
        let write = write_request(&preview).unwrap();
        assert!(write.is_confirmed());
        assert_eq!(write.id, "set");
        assert_eq!(write.args.get("dry_run"), None);
        assert_eq!(write.args["fields"], json!({"FLTV": 7.25}));
        assert_eq!(write.args["into"], json!(r"C:\out\Patch.esp"));
        assert!(check(&write).is_ok());
        // Only a `set` dry run becomes a write.
        assert!(write_request(&write).is_none());
        assert!(write_request(&Request::new("inspect", json!({"dry_run": true}))).is_none());
        let written = json!({"data": {"records": [
            {"form_id": "00000801", "parent": true},
            {"form_id": "00000800", "parent": false}
        ]}});
        let readback = readback_request(&written, r"C:\out\Patch.esp").unwrap();
        assert_eq!(readback.args["form"], json!("00000800"));
        assert!(check(&readback).is_ok());
        assert!(readback_request(&json!({"data": {}}), "x.esp").is_none());
    }
}
