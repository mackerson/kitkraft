//! Dry runs of the writing commands (M7.S6): `set`, `new` and `clone` with `dry_run: true`
//! write into the staged copy of `into` (OS temp dir) exactly as a real call does, run
//! `validate` on it, and return what the write would do without installing it, so `into`
//! stays byte-identical (or absent when it did not exist).
//!
//! The result is the command's own envelope (records, fields changed, evidence offsets in
//! the file that would be written) plus `dry_run: true`, `before` (the version the edit
//! starts from, in `inspect`'s `versions[]` shape; `null` for a new form), `after` (each
//! record as it would be written, in the same shape), `changed` (the member paths the edit
//! changes) and `would_write` (false when the written records fail `validate`, which a
//! real call refuses).

use std::fs::File;
use std::path::Path;

use kitkraft_doc::{GlobalForm, LoadedRecord, MasterMap, PluginFlags, RecordReader, Scale};
use kitkraft_esm::DecodeLimits;
use serde_json::{json, Value};

use super::inspect::{version_entry, VersionAt};
use super::load::Loaded;
use super::set::Staged;
use crate::edit::patch::PatchReport;
use crate::envelope::{Evidence, Outcome};
use crate::validate::{self, Finding};

/// Stated on every dry run.
pub(super) const DRY_RUN: &str = "dry run: nothing was written; the plugin was written to a \
     staged copy under the OS temp dir (removed after the call) and validated there, so the \
     destination is unchanged; evidence offsets name the bytes the destination would hold";

/// Whether `dry_run` is set.
pub(super) fn requested(args: &Value) -> bool {
    args.get("dry_run").and_then(Value::as_bool) == Some(true)
}

/// The master map of a staged plugin (`masters`, TES4 `flags`), its own index one past
/// the load order's plugins.
fn staged_map(
    loaded: &Loaded,
    name: &str,
    masters: &[String],
    flags: u32,
) -> Result<MasterMap, String> {
    let lo = &loaded.lo;
    let own = u16::try_from(lo.plugins.len()).map_err(|_| "too many plugins".to_string())?;
    let mut list = Vec::with_capacity(masters.len());
    for m in masters {
        let i = lo
            .find(m)
            .ok_or_else(|| format!("master {m} is not in the load order"))?;
        let scale = lo
            .plugin(i)
            .and_then(kitkraft_doc::LoadedPlugin::scale)
            .ok_or_else(|| format!("master {m} has no readable scale"))?;
        list.push((i, scale));
    }
    let own_scale = PluginFlags(flags).scale().unwrap_or(Scale::Full);
    Ok(MasterMap::new(name, own, own_scale, &list))
}

/// The winning version of `form` (the version an edit starts from), as an `inspect`
/// version entry; `None` when it cannot be read (the reason goes to `notes`).
pub(super) fn before(
    schema: Option<&kitkraft_schema::Schema>,
    loaded: &mut Loaded,
    form: GlobalForm,
    evidence: &mut Vec<Evidence>,
    notes: &mut Vec<String>,
) -> Option<Value> {
    let v = loaded.lo.forms.winner(form).copied()?;
    let loc = loaded.lo.record(v.plugin, v.record).copied()?;
    let rec = match RecordReader::new().read(&loaded.lo, v.plugin, &loc) {
        Ok(r) => r,
        Err(e) => {
            notes.push(format!("before: {e}"));
            return None;
        }
    };
    Some(version_entry(
        loaded,
        schema,
        &rec,
        &VersionAt::LoadOrder(v),
        evidence,
        notes,
    ))
}

/// Every record the call wrote (parents carried only to hold a group left out), read back
/// from the staged plugin, as `inspect` version entries named by `into`'s file name.
pub(super) fn after(
    schema: Option<&kitkraft_schema::Schema>,
    loaded: &mut Loaded,
    staged: &Staged,
    report: &PatchReport,
    into: &Path,
    notes: &mut Vec<String>,
) -> Vec<Value> {
    let name = into
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let map = match staged_map(loaded, &name, &report.masters, report.flags) {
        Ok(m) => m,
        Err(e) => {
            notes.push(format!(
                "after: FormIDs of the written records not resolved: {e}"
            ));
            return Vec::new();
        }
    };
    let localized = PluginFlags(report.flags).localized();
    let mut out = Vec::new();
    let mut ignored = Vec::new();
    for r in report.records.iter().filter(|r| !r.parent) {
        let Some(offset) = r.offset else {
            notes.push(format!(
                "after: {} has no offset in the written file",
                r.formref
            ));
            continue;
        };
        let read = File::open(&staged.path)
            .map_err(|e| e.to_string())
            .and_then(|mut f| {
                kitkraft_doc::record::read_record_at(
                    &mut f,
                    &name,
                    offset,
                    None,
                    &DecodeLimits::default(),
                )
                .map_err(|e| e.to_string())
            });
        let rec: LoadedRecord = match read {
            Ok(rec) => rec,
            Err(e) => {
                notes.push(format!("after: {}: {e}", r.formref));
                continue;
            }
        };
        out.push(version_entry(
            loaded,
            schema,
            &rec,
            &VersionAt::Outside {
                file: &name,
                localized,
                map: &map,
            },
            &mut ignored,
            notes,
        ));
    }
    out
}

/// Adds the dry run's members to `result` (the command's own envelope over the staged
/// write): `dry_run`, `before`, `after`, `changed`, `would_write`, `validate`.
pub(super) fn attach(
    result: &mut Outcome,
    before: Option<Value>,
    after: Vec<Value>,
    report: &PatchReport,
    checked: &validate::Report,
    blocking: &[&Finding],
    mut notes: Vec<String>,
) {
    let changed: Vec<Value> = report
        .records
        .iter()
        .filter(|r| !r.parent)
        .flat_map(|r| {
            r.changes.iter().map(move |c| {
                json!({"form": r.formref, "path": c.path, "before": c.before, "after": c.after})
            })
        })
        .collect();
    if let Some(obj) = result.data.as_object_mut() {
        obj.insert("dry_run".into(), json!(true));
        obj.insert("would_write".into(), json!(blocking.is_empty()));
        obj.insert("before".into(), before.unwrap_or(Value::Null));
        obj.insert("after".into(), Value::Array(after));
        obj.insert("changed".into(), Value::Array(changed));
        obj.insert("validate".into(), super::set::validate_json(checked));
    }
    result.limitations.push(DRY_RUN.to_string());
    if !blocking.is_empty() {
        result.limitations.push(format!(
            "the written record fails validate, so a real call would refuse it: {}",
            super::set::describe(blocking)
        ));
    }
    notes.sort();
    notes.dedup();
    for n in notes {
        if !result.limitations.contains(&n) {
            result.limitations.push(n);
        }
    }
}
