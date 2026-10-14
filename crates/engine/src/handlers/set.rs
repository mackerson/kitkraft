//! `set` (M2.E5): write field values on the winning version of a form into a patch plugin.
//!
//! The load order comes from `profile` / `plugins` / `data_dir` (loaded first, as for
//! `inspect`), or from the session. The form is a formref (`Starfield.esm:21BBCD`), an
//! EditorID, or a hex FormID in `plugin`'s space; its winning version is copied into `into`
//! through the patch writer (`crate::edit::patch`), with `fields` applied
//! (`crate::edit::fields`): `{"FULL": "Old Soviet Marksman Rifle"}`, or the CLI shorthand
//! `FULL="..."`. The write goes to a staged copy of `into` (OS temp dir), the plugin rules
//! of `validate` run on it as `build`'s do, and `into` is replaced only when the records
//! written pass (`docs/validate.md`).

use std::cell::RefCell;
use std::fs::File;
use std::path::{Path, PathBuf};

use kitcraft_doc::GlobalForm;
use kitcraft_esm::index::read_record_at;
use kitcraft_esm::DecodeLimits;
use serde_json::{json, Map, Value};

use super::load::{acquire, Loaded};
use super::{failed, str_arg};
use crate::args::Violation;
use crate::edit::fields::{parse_op, parse_path};
use crate::edit::patch::{
    guard_target, write_patch_with, Copy, Edit, NewRef, PatchError, PatchOptions, PatchReport,
};
use crate::edit::source::{placement_at, SourcePlugin};
use crate::edit::strings::TableStrings;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;
use crate::validate::{self, Severity};

const ID: &str = "set";

/// Record flag "deleted" (UESP, Skyrim_Mod:Mod_File_Format, "Records").
const DELETED: u32 = 0x0000_0020;

/// The edits in a `fields` object; a bad path or value is `invalid_args` at
/// `/<arg>/<path>` (JSON pointer, `~` and `/` escaped).
pub(crate) fn edits(
    command: &str,
    pointer: &str,
    fields: &Map<String, Value>,
) -> Result<Vec<Edit>, EngineError> {
    let invalid = |key: &str, rule: &'static str, message: String| EngineError::InvalidArgs {
        command: command.to_string(),
        violations: vec![Violation {
            pointer: format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1")),
            rule,
            message,
        }],
    };
    if fields.is_empty() {
        return Err(invalid("", "minProperties", "no fields to set".into()));
    }
    fields
        .iter()
        .map(|(key, value)| {
            let path = parse_path(key).map_err(|e| invalid(key, e.rule, e.to_string()))?;
            let op = parse_op(key, value).map_err(|e| invalid(key, e.rule, e.to_string()))?;
            Ok(Edit { path, op })
        })
        .collect()
}

/// The form `form` names in the load order (formref, hex FormID in `plugin`, EditorID).
pub(crate) fn find_form(
    command: &str,
    loaded: &Loaded,
    form: &str,
    plugin: Option<&str>,
) -> Result<GlobalForm, EngineError> {
    let lo = &loaded.lo;
    if let Some(f) = kitcraft_doc::FormRef::parse(form) {
        let p = lo
            .find(&f.plugin)
            .ok_or_else(|| failed(command, format!("`{}` is not in the load order", f.plugin)))?;
        return Ok(GlobalForm {
            plugin: p,
            object_id: f.object_id,
        });
    }
    if let (Some(raw), Some(name)) = (super::inspect::parse_form_id(form), plugin) {
        let p = lo
            .find(name)
            .ok_or_else(|| failed(command, format!("`{name}` is not in the load order")))?;
        return lo
            .resolve(p, raw)
            .map_err(|e| failed(command, e.to_string()));
    }
    for (i, p) in lo.plugins.iter().enumerate() {
        let index = u16::try_from(i).unwrap_or(u16::MAX);
        let Some(idx) = p.index.as_deref() else {
            continue;
        };
        for loc in &idx.records {
            if idx.edid(loc).is_some_and(|e| e.eq_ignore_ascii_case(form)) {
                if let Ok(g) = lo.resolve(index, loc.form_id) {
                    return Ok(g);
                }
            }
        }
    }
    Err(failed(
        command,
        format!("no record with formref or EditorID `{form}` in the load order (a hex FormID needs `plugin`)"),
    ))
}

/// Refuses to copy the winning version of `form` when lane D's source check (M3.D1,
/// `kitcraft_doc::source`) finds a problem that can change which version wins or what its
/// FormIDs mean: a plugin holding a version loads before its master, or a plugin after the
/// winner failed to load or has unresolved records that could be this form, or the winner
/// is missing or deleted. Problems elsewhere in the load order (an active plugin with no
/// file, an unrelated ordering error) are returned for `limitations` (M3.E1; this replaces
/// the blanket "load order unverified" refusal).
pub(crate) fn safe_source(
    command: &str,
    loaded: &Loaded,
    form: GlobalForm,
) -> Result<Vec<String>, EngineError> {
    let check = loaded.source_check(form);
    if !check.blocking.is_empty() {
        return Err(failed(
            command,
            format!(
                "the winning version of {} is not a safe source: {} (fix the load order, or name the plugins with `plugins`)",
                loaded.lo.form_label(form),
                check.blocking.join("; ")
            ),
        ));
    }
    Ok(check
        .unrelated
        .into_iter()
        .map(|u| {
            format!(
                "load order problem that does not affect {}: {u}",
                loaded.lo.form_label(form)
            )
        })
        .collect())
}

/// Appends each of `more` to `limitations` unless it is already there.
pub(crate) fn extend_once(limitations: &mut Vec<String>, more: Vec<String>) {
    for l in more {
        if !limitations.contains(&l) {
            limitations.push(l);
        }
    }
}

/// Where the copied version came from, for the report.
#[derive(Debug, Clone)]
pub(crate) struct Origin {
    pub plugin: String,
    pub load_index: u16,
    pub offset: u64,
    pub length: u64,
    pub label: String,
    pub versions: usize,
}

/// A copy of the winning version of `form`, with `edits`, ready for the patch writer.
pub(crate) fn winning_copy(
    command: &str,
    loaded: &Loaded,
    form: GlobalForm,
    edits: Vec<Edit>,
    limits: &DecodeLimits,
) -> Result<(Copy, Origin), EngineError> {
    let lo = &loaded.lo;
    let versions = lo.forms.versions(form);
    let label = lo.form_label(form);
    let winner = versions
        .last()
        .ok_or_else(|| failed(command, format!("{label} has no version in the load order")))?;
    let plugin = lo
        .plugin(winner.plugin)
        .ok_or_else(|| failed(command, "winning plugin missing"))?;
    let header = plugin
        .header()
        .ok_or_else(|| failed(command, format!("{} has no readable header", plugin.name)))?;
    let loc = lo
        .record(winner.plugin, winner.record)
        .copied()
        .ok_or_else(|| failed(command, "winning record missing from the index"))?;
    let read = |offset: u64| {
        let mut file = File::open(&plugin.path)
            .map_err(|e| failed(command, format!("{}: {e}", plugin.path.display())))?;
        read_record_at(&mut file, offset, Some(limits))
            .map_err(|e| failed(command, format!("{}: {e}", plugin.name)))
    };
    let record = read(loc.offset)?;
    if record.header.flags & DELETED != 0 {
        return Err(failed(
            command,
            format!(
                "the winning version of {label} in {} is deleted (record flag 0x20); a patch does not edit a deleted record",
                plugin.name
            ),
        ));
    }
    let placement = placement_at(&plugin.path, loc.offset).map_err(|m| failed(command, m))?;
    let mut parents = Vec::new();
    for (depth, offset) in &placement.owners {
        let path = placement.path.get(..*depth).unwrap_or_default().to_vec();
        parents.push((read(*offset)?, path));
    }
    let source = SourcePlugin {
        name: plugin.name.clone(),
        path: plugin.path.clone(),
        masters: header.masters.clone(),
        flags: header.flags.0,
    };
    let origin = Origin {
        plugin: plugin.name.clone(),
        load_index: winner.plugin,
        offset: loc.offset,
        length: u64::from(loc.data_size).saturating_add(24),
        label: format!("{}:{:08X}", record.header.signature, record.header.form_id),
        versions: versions.len(),
    };
    Ok((
        Copy {
            source,
            record,
            path: placement.path,
            parents,
            edits,
            formref: label,
            carried: false,
        },
        origin,
    ))
}

/// The load order's plugin files: never written.
pub(crate) fn protected(loaded: &Loaded) -> Vec<PathBuf> {
    loaded.lo.plugins.iter().map(|p| p.path.clone()).collect()
}

/// Refuses an output path the patch writer would refuse (`guard_target`: extension, a
/// load-order plugin, the game or user Data directory, an MO2 instance), before anything
/// is staged.
pub(crate) fn guard(command: &str, loaded: &Loaded, out: &Path) -> Result<(), EngineError> {
    guard_target(out, &protected(loaded)).map_err(|e| failed(command, e.to_string()))
}

/// Writes `copies` into `into` with the load order's string tables, protected files and
/// plugin flags.
pub(crate) fn write(
    command: &str,
    loaded: &mut Loaded,
    into: &Path,
    copies: Vec<Copy>,
    allow_untyped_lstrings: bool,
) -> Result<PatchReport, EngineError> {
    write_with(
        command,
        loaded,
        into,
        copies,
        Vec::new(),
        allow_untyped_lstrings,
    )
}

/// [`write`] with new placed references (`place`, spec v3 `[[place]]`).
pub(crate) fn write_with(
    command: &str,
    loaded: &mut Loaded,
    into: &Path,
    copies: Vec<Copy>,
    new_refs: Vec<NewRef>,
    allow_untyped_lstrings: bool,
) -> Result<PatchReport, EngineError> {
    let protected = protected(loaded);
    let plugin_flags: Vec<(String, u32)> = loaded
        .lo
        .plugins
        .iter()
        .filter_map(|p| Some((p.name.to_ascii_lowercase(), p.header()?.flags.0)))
        .collect();
    let strings = TableStrings(RefCell::new(&mut loaded.strings));
    let options = PatchOptions {
        strings: &strings,
        protected,
        plugin_flags,
        limits: DecodeLimits::default(),
        allow_untyped_lstrings,
    };
    write_patch_with(into, copies, new_refs, &options).map_err(|e| match e {
        PatchError::Field(f) => EngineError::InvalidArgs {
            command: command.to_string(),
            violations: vec![Violation {
                pointer: format!("/fields/{}", f.path.replace('~', "~0").replace('/', "~1")),
                rule: f.rule,
                message: f.to_string(),
            }],
        },
        PatchError::Refused(m) | PatchError::Io(m) => failed(command, m),
    })
}

/// A private directory under the OS temp dir where `set` and `build` write and validate a
/// plugin before it replaces its destination; removed on drop. Directories a crashed run
/// left behind (older than a day) are removed when a new one is made, and so are `build`'s
/// old `.kitcraft-build-<pid>` directories next to the destination (the layout before
/// 2026-10-14, older than an hour).
pub(crate) struct Staged {
    dir: PathBuf,
    /// The staged plugin (same file name as the destination).
    pub path: PathBuf,
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Prefix of staging directories in the OS temp dir.
const STAGE_PREFIX: &str = "kitcraft-stage-";

static STAGE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Removes directories in `dir` whose name starts with `prefix` and that were last
/// modified more than `age` ago (best effort).
pub(crate) fn remove_stale(dir: &Path, prefix: &str, age: std::time::Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let named = entry.file_name().to_string_lossy().starts_with(prefix);
        let old = entry
            .metadata()
            .ok()
            .filter(std::fs::Metadata::is_dir)
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|e| e > age);
        if named && old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Stages `out`: a fresh directory under the OS temp dir holding `out`'s file name, with a
/// copy of `out` when `copy_existing` and it exists.
pub(crate) fn stage(command: &str, out: &Path, copy_existing: bool) -> Result<Staged, EngineError> {
    let temp = std::env::temp_dir();
    remove_stale(
        &temp,
        STAGE_PREFIX,
        std::time::Duration::from_secs(24 * 3600),
    );
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    remove_stale(
        parent,
        ".kitcraft-build-",
        std::time::Duration::from_secs(3600),
    );
    let name = out
        .file_name()
        .ok_or_else(|| failed(command, format!("{}: not a file path", out.display())))?;
    let n = STAGE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = temp.join(format!("{STAGE_PREFIX}{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| failed(command, format!("{}: {e}", dir.display())))?;
    let staged = Staged {
        path: dir.join(name),
        dir,
    };
    if copy_existing && out.exists() {
        std::fs::copy(out, &staged.path)
            .map_err(|e| failed(command, format!("staging {}: {e}", out.display())))?;
    }
    Ok(staged)
}

/// Moves the staged plugin to `out`: copied next to `out` first, then renamed over it (the
/// staging dir may be on another volume; the rename stays on `out`'s).
pub(crate) fn install(command: &str, staged: &Staged, out: &Path) -> Result<(), EngineError> {
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = out.with_file_name(format!(".{name}.kitcraft-{}.tmp", std::process::id()));
    std::fs::copy(&staged.path, &temp)
        .and_then(|_| std::fs::rename(&temp, out))
        .map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            failed(
                command,
                format!("moving the plugin to {}: {e}", out.display()),
            )
        })
}

/// Runs `validate` on the staged plugin against the load order.
pub(crate) fn check(
    command: &str,
    loaded: &Loaded,
    staged: &Staged,
) -> Result<validate::Report, EngineError> {
    let options = validate::Options {
        plugin: Some(staged.path.clone()),
        plugins: protected(loaded),
        indexed: super::validate::indexes(&loaded.lo),
        ..validate::Options::default()
    };
    validate::validate(&options).map_err(|m| failed(command, format!("validating the output: {m}")))
}

/// `{rule} {record}: {message}` of up to 20 findings.
pub(crate) fn describe(findings: &[&validate::Finding]) -> String {
    findings
        .iter()
        .take(20)
        .map(|f| {
            format!(
                "{} {}: {}",
                f.rule,
                f.record.as_deref().unwrap_or("-"),
                f.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The `validate` part of a `set` or `build` result.
pub(crate) fn validate_json(checked: &validate::Report) -> Value {
    json!({
        "passed": checked.passed(),
        "errors": checked.count(Severity::Error),
        "warnings": checked.count(Severity::Warning),
        "findings": checked.findings,
        "rules": checked.rules,
    })
}

/// The envelope for a written patch (shared with `build`).
pub(crate) fn outcome(
    report: &PatchReport,
    origins: &[Origin],
    mut limitations: Vec<String>,
) -> Outcome {
    let target = report
        .target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut evidence = Vec::new();
    let mut records = Vec::new();
    let mut origins = origins.iter();
    for r in &report.records {
        let mut json = r.to_json();
        if !r.parent {
            if let Some(o) = origins.next() {
                if let Some(obj) = json.as_object_mut() {
                    obj.insert(
                        "copied_from".into(),
                        json!({
                            "plugin": o.plugin, "load_index": o.load_index, "offset": o.offset,
                            "record": o.label, "versions_in_load_order": o.versions,
                        }),
                    );
                }
                evidence.push(Evidence {
                    file: o.plugin.clone(),
                    offset: o.offset,
                    length: o.length,
                    record: Some(o.label.clone()),
                    subrecord: None,
                });
            }
        }
        if let (Some(offset), Some(length)) = (r.offset, r.length) {
            evidence.push(Evidence {
                file: target.clone(),
                offset,
                length,
                record: Some(format!("{}:{:08X}", r.signature, r.form_id)),
                subrecord: None,
            });
        }
        for (sig, offset, length) in &r.edited_subrecords {
            if let Some(offset) = offset {
                evidence.push(Evidence {
                    file: target.clone(),
                    offset: *offset,
                    length: *length,
                    record: Some(format!("{}:{:08X}", r.signature, r.form_id)),
                    subrecord: Some(sig.clone()),
                });
            }
        }
        records.push(json);
    }
    let changed: Vec<String> = report
        .records
        .iter()
        .filter(|r| !r.parent)
        .map(|r| {
            let fields: Vec<String> = r
                .changes
                .iter()
                .map(|c| format!("{} {} -> {}", c.path, c.before, c.after))
                .collect();
            format!(
                "{} {} ({}): {}",
                r.signature,
                r.formref,
                r.editor_id.as_deref().unwrap_or("-"),
                if fields.is_empty() {
                    "copied".to_string()
                } else {
                    fields.join(", ")
                }
            )
        })
        .collect();
    let summary = format!(
        "{target}: {} ({} bytes, masters {}); {}",
        if report.created { "created" } else { "updated" },
        report.bytes,
        report.masters.join(", "),
        changed.join("; ")
    );
    for l in &report.limitations {
        if !limitations.contains(l) {
            limitations.push(l.clone());
        }
    }
    Outcome {
        data: json!({
            "target": report.target.display().to_string(),
            "file": target,
            "created": report.created,
            "masters": report.masters,
            "flags": format!("0x{:08X}", report.flags),
            "bytes": report.bytes,
            "records": records,
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
    }
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let form = str_arg(ID, args, "form")?;
    let into = PathBuf::from(str_arg(ID, args, "into")?);
    let fields = args
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| failed(ID, "argument `fields` is missing"))?;
    let edits = edits(ID, "/fields", fields)?;
    let allow_untyped = args
        .get("allow_untyped_lstrings")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let loaded = acquire(session, args, ID)?;
    guard(ID, loaded, &into)?;
    let limits = DecodeLimits::default();
    let g = find_form(ID, loaded, form, args.get("plugin").and_then(Value::as_str))?;
    let unrelated = safe_source(ID, loaded, g)?;
    let (copy, origin) = winning_copy(ID, loaded, g, edits, &limits)?;
    let mut limitations = loaded.limitations.clone();
    extend_once(&mut limitations, unrelated);

    // Write into a staged copy of `into`, run the record rules on it (as `build` does),
    // and replace `into` only when the records this call wrote pass.
    let staged = stage(ID, &into, true)?;
    let mut report = write(ID, loaded, &staged.path, vec![copy], allow_untyped)?;
    let checked = check(ID, loaded, &staged)?;
    let written: Vec<String> = report
        .records
        .iter()
        .map(|r| format!("{}:{:08X}", r.signature, r.form_id))
        .collect();
    let (blocking, other): (Vec<&validate::Finding>, Vec<&validate::Finding>) = checked
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .partition(|f| f.record.as_ref().is_none_or(|r| written.contains(r)));
    if !blocking.is_empty() {
        return Err(failed(
            ID,
            format!(
                "the written record fails validate, so {} was not changed: {}",
                into.display(),
                describe(&blocking)
            ),
        ));
    }
    if !other.is_empty() {
        limitations.push(format!(
            "{} has {} validate error(s) on records this call did not write: {}",
            into.display(),
            other.len(),
            describe(&other)
        ));
    }
    install(ID, &staged, &into)?;
    report.target = into.clone();
    let mut result = outcome(&report, &[origin], limitations);
    if let Some(obj) = result.data.as_object_mut() {
        obj.insert("validate".into(), validate_json(&checked));
    }
    Ok(result)
}
