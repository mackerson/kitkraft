//! `revert` (M7.S7): undo an edit to the active file. Given a plugin (the file a `set`,
//! `build` or `place` wrote; never a load-order plugin, the game's Data folder or MO2) and
//! a form it overrides, either remove the override (`to: "remove"`, the default: the form
//! is then the version before it in the load order again) or replace it with a copy of
//! that version (`to: "previous"`: the record stays in the file, with the master's data).
//!
//! The write goes through the same staging as `set`: the plugin is copied to a private
//! directory under the OS temp dir, changed there, validated against the load order, and
//! moved over the file only when the call is not a `dry_run` and the result passes. The
//! envelope carries `diff`, the record-level diff of the file before and after (the
//! `diff` command's data), so a preview shows exactly what the revert changes.
//!
//! A form the plugin itself defines (a new form, not an override) has no earlier version
//! to return to: `revert` refuses it (`revert.own-form`), since removing a new form can
//! leave other records naming it.

use std::path::PathBuf;

use kitkraft_doc::{FormRef, GlobalForm, MasterMap, PluginFlags, Scale};
use kitkraft_esm::{DecodeLimits, Entry, Plugin, Signature};
use serde_json::{json, Value};

use super::load::{acquire, Loaded};
use super::set::{
    check, describe, extend_once, guard, install, safe_source, stage, validate_json, winning_copy,
    write,
};
use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;
use crate::validate::{Finding, Severity};

const ID: &str = "revert";

/// Deepest group nesting followed when looking for the record.
const MAX_DEPTH: usize = 16;

/// The plugin's master map against the load order, its own index one past the last
/// plugin (it is not in the load order).
fn plugin_map(
    loaded: &Loaded,
    name: &str,
    masters: &[String],
    flags: u32,
) -> Result<MasterMap, EngineError> {
    let lo = &loaded.lo;
    let own = u16::try_from(lo.plugins.len()).map_err(|_| failed(ID, "too many plugins"))?;
    let mut list = Vec::with_capacity(masters.len());
    for m in masters {
        let i = lo.find(m).ok_or_else(|| {
            failed(
                ID,
                format!("revert.master-missing: {name} names the master {m}, which is not in the load order (pass the plugin's masters with `plugins` or `profile`)"),
            )
        })?;
        let scale = lo
            .plugin(i)
            .and_then(kitkraft_doc::LoadedPlugin::scale)
            .ok_or_else(|| failed(ID, format!("the master {m} has no readable scale")))?;
        list.push((i, scale));
    }
    let own_scale = PluginFlags(flags).scale().unwrap_or(Scale::Full);
    Ok(MasterMap::new(name, own, own_scale, &list))
}

/// A record found in the plugin: its FormID, signature and header offset.
type Found = (u32, Signature, Option<u64>);

/// The first record (not `TES4`) whose header FormID is `raw`, or whose EditorID is
/// `edid` (ASCII case-insensitive).
fn find_raw(entries: &[Entry<'_>], raw: Option<u32>, edid: &str, depth: usize) -> Option<Found> {
    if depth > MAX_DEPTH {
        return None;
    }
    for e in entries {
        match e {
            Entry::Group(g) => {
                if let Some(f) = find_raw(g.children(), raw, edid, depth.saturating_add(1)) {
                    return Some(f);
                }
            }
            Entry::Record(r) => {
                let found = (r.header.form_id, r.header.signature, r.offset());
                if raw == Some(r.header.form_id) {
                    return Some(found);
                }
                if raw.is_none() {
                    let limits = DecodeLimits::default();
                    let decoded = r.decode(&limits).ok().flatten();
                    let subs = decoded.as_deref().or_else(|| r.subrecords());
                    let named = subs
                        .and_then(|s| s.iter().find(|s| s.signature() == Signature(*b"EDID")))
                        .map(|s| kitkraft_doc::index::zstring(s.data()));
                    if named.is_some_and(|n| n.eq_ignore_ascii_case(edid)) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

/// Removes the record `raw` (and every group left empty on its path); its signature.
fn remove(entries: &mut Vec<Entry<'_>>, raw: u32, depth: usize) -> Option<Signature> {
    if depth > MAX_DEPTH {
        return None;
    }
    let at = entries
        .iter()
        .position(|e| matches!(e, Entry::Record(r) if r.header.form_id == raw));
    if let Some(i) = at {
        let removed = entries.remove(i);
        return match removed {
            Entry::Record(r) => Some(r.header.signature),
            Entry::Group(_) => None,
        };
    }
    for i in 0..entries.len() {
        let Some(Entry::Group(g)) = entries.get(i) else {
            continue;
        };
        if find_raw(g.children(), Some(raw), "", depth.saturating_add(1)).is_none() {
            continue;
        }
        let Some(Entry::Group(g)) = entries.get_mut(i) else {
            continue;
        };
        let sig = remove(g.children_mut(), raw, depth.saturating_add(1));
        if g.children().is_empty() {
            entries.remove(i);
        }
        return sig;
    }
    None
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let path = PathBuf::from(str_arg(ID, args, "plugin")?);
    let form_arg = str_arg(ID, args, "form")?;
    let dry_run = super::preview::requested(args);
    let to_previous = match args.get("to").and_then(Value::as_str) {
        None | Some("remove") => false,
        Some("previous") => true,
        Some(other) => {
            return Err(failed(
                ID,
                format!("to `{other}`: expected remove or previous"),
            ))
        }
    };
    let loaded = acquire(session, args, ID)?;
    guard(ID, loaded, &path)?;
    let bytes = std::fs::read(&path).map_err(|e| failed(ID, format!("{}: {e}", path.display())))?;
    let parsed =
        Plugin::parse(&bytes).map_err(|e| failed(ID, format!("{}: {e}", path.display())))?;
    let name = file_name(&path);
    let info = kitkraft_doc::index::header_info(parsed.header());
    let map = plugin_map(loaded, &name, &info.masters, info.flags.0)?;

    // The record: a formref of the load order (or of the plugin itself), a hex FormID as
    // stored in the plugin, or an EditorID.
    let wanted = match FormRef::parse(form_arg) {
        Some(f) if f.plugin.eq_ignore_ascii_case(&name) => Some(
            map.to_raw(GlobalForm {
                plugin: map.own_index,
                object_id: f.object_id,
            })
            .ok_or_else(|| failed(ID, format!("{form_arg}: object ID does not fit")))?,
        ),
        Some(f) => {
            let p = loaded
                .lo
                .find(&f.plugin)
                .ok_or_else(|| failed(ID, format!("`{}` is not in the load order", f.plugin)))?;
            Some(
                map.to_raw(GlobalForm {
                    plugin: p,
                    object_id: f.object_id,
                })
                .ok_or_else(|| {
                    failed(
                        ID,
                        format!(
                            "revert.not-in-plugin: {name} does not have {} among its masters",
                            f.plugin
                        ),
                    )
                })?,
            )
        }
        None => super::inspect::parse_form_id(form_arg),
    };
    let (raw, record_sig, record_at) = find_raw(parsed.entries(), wanted, form_arg, 0)
        .or_else(|| wanted.and_then(|_| find_raw(parsed.entries(), None, form_arg, 0)))
        .ok_or_else(|| {
            failed(
                ID,
                format!("revert.not-in-plugin: {name} has no record `{form_arg}`"),
            )
        })?;
    let g = map
        .resolve(raw)
        .map_err(|e| failed(ID, format!("{raw:08X} in {name}: {e}")))?;
    if g.plugin == map.own_index {
        return Err(failed(
            ID,
            format!(
                "revert.own-form: {name}:{:06X} is a form {name} defines, not an override: there is no earlier version to return to (remove its uses first, then the form)",
                g.object_id
            ),
        ));
    }
    let label = loaded.lo.form_label(g);
    let mut limitations = loaded.limitations.clone();

    // The change, in a staged copy.
    let staged = stage(ID, &path, true)?;
    let mut summary_of_change = String::new();
    if to_previous {
        if loaded.lo.forms.versions(g).is_empty() {
            return Err(failed(
                ID,
                format!("revert.no-previous: {label} has no version in the load order to copy"),
            ));
        }
        extend_once(&mut limitations, safe_source(ID, loaded, g)?);
        let (copy, origin) = winning_copy(ID, loaded, g, Vec::new(), &DecodeLimits::default())?;
        let report = write(ID, loaded, &staged.path, vec![copy], false)?;
        extend_once(&mut limitations, report.limitations.clone());
        summary_of_change = format!(
            "replaced by a copy of the version in {} ({} at offset {})",
            origin.plugin, origin.label, origin.offset
        );
    } else {
        let mut tree = parsed.clone();
        let sig = remove(tree.entries_mut(), raw, 0)
            .ok_or_else(|| failed(ID, format!("{name}: record {raw:08X} not removed")))?;
        let out = tree
            .to_bytes()
            .map_err(|e| failed(ID, format!("writing {name}: {e}")))?;
        std::fs::write(&staged.path, out)
            .map_err(|e| failed(ID, format!("{}: {e}", staged.path.display())))?;
        let _ = std::fmt::Write::write_fmt(
            &mut summary_of_change,
            format_args!("{sig}:{raw:08X} removed"),
        );
        limitations.push(
            "remove: parent records the plugin carries only to hold the removed record's group (a CELL or WRLD copied for a placed reference) stay; groups left empty are removed".to_string(),
        );
    }
    let checked = check(ID, loaded, &staged)?;
    // The masters' directory for the diff (the staged copy sits in the OS temp dir).
    let masters_dir = args
        .get("data_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| {
            loaded
                .lo
                .plugins
                .first()
                .and_then(|p| p.path.parent().map(std::path::Path::to_path_buf))
        });
    let mut diff_args = json!({
        "a": path.display().to_string(),
        "b": staged.path.display().to_string(),
    });
    if let (Some(dir), Some(obj)) = (masters_dir, diff_args.as_object_mut()) {
        obj.insert("data_dir".into(), json!(dir.display().to_string()));
    }
    let diff = super::diff::run(&diff_args)?;
    let blocking: Vec<&Finding> = checked
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    if !dry_run {
        if !blocking.is_empty() {
            return Err(failed(
                ID,
                format!(
                    "revert.fails-validate: the reverted plugin fails validate, so {} was not changed: {}",
                    path.display(),
                    describe(&blocking)
                ),
            ));
        }
        install(ID, &staged, &path)?;
    } else {
        limitations.push(super::preview::DRY_RUN.to_string());
        if !blocking.is_empty() {
            limitations.push(format!(
                "the reverted plugin fails validate, so a real call would refuse it: {}",
                describe(&blocking)
            ));
        }
    }
    let mut evidence = Vec::new();
    if let Some(offset) = record_at {
        evidence.push(Evidence {
            file: name.clone(),
            offset,
            length: kitkraft_esm::HEADER_LEN as u64,
            record: Some(format!("{record_sig}:{raw:08X}")),
            subrecord: None,
        });
    }
    evidence.extend(diff.evidence.iter().cloned());
    extend_once(&mut limitations, diff.limitations.clone());
    let summary = format!(
        "{}{name}: {label} ({raw:08X}) {summary_of_change}",
        if dry_run { "dry run: " } else { "" }
    );
    Ok(Outcome {
        data: json!({
            "plugin": path.display().to_string(),
            "file": name,
            "form": label,
            "raw_form_id": format!("{raw:08X}"),
            "to": if to_previous { "previous" } else { "remove" },
            "dry_run": dry_run,
            "written": !dry_run,
            "would_write": blocking.is_empty(),
            "diff": diff.data,
            "validate": validate_json(&checked),
            "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
}
