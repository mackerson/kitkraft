//! `set` (M2.E5): write field values on the winning version of a form into a patch plugin.
//!
//! The load order comes from `profile` / `plugins` / `data_dir` (loaded first, as for
//! `inspect`), or from the session. The form is a formref (`Starfield.esm:21BBCD`), an
//! EditorID, or a hex FormID in `plugin`'s space; its winning version is copied into `into`
//! through the patch writer (`crate::edit::patch`), with `fields` applied
//! (`crate::edit::fields`): `{"FULL": "Old Soviet Marksman Rifle"}`, or the CLI shorthand
//! `FULL="..."`.

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
use crate::edit::patch::{write_patch, Copy, Edit, PatchError, PatchOptions, PatchReport};
use crate::edit::source::{placement_at, SourcePlugin};
use crate::edit::strings::TableStrings;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

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

/// The text lane D's `load` puts in a limitation for active plugins with no file.
const NO_FILE: &str = "active plugins with no file";

/// Refuses a load order whose winners cannot be trusted yet (lane D round 2 pending): a
/// plugin that failed to load, a plugin that loads before one of its masters or misses
/// one, an active plugin with no file. `set` and `build` copy the winning version, so on
/// such a load order they stop instead of guessing.
pub(crate) fn verify_load_order(command: &str, loaded: &Loaded) -> Result<(), EngineError> {
    let mut problems: Vec<String> = Vec::new();
    for (i, p) in loaded.lo.plugins.iter().enumerate() {
        if let Some(e) = &p.error {
            problems.push(format!("{} failed to load ({e})", p.name));
        }
        let late = p.late_masters(u16::try_from(i).unwrap_or(u16::MAX));
        if !late.is_empty() {
            problems.push(format!(
                "{} loads before its master(s) {}",
                p.name,
                late.join(", ")
            ));
        }
        let missing = p.missing_masters();
        if !missing.is_empty() {
            problems.push(format!(
                "{} misses master(s) {}",
                p.name,
                missing.join(", ")
            ));
        }
    }
    problems.extend(
        loaded
            .limitations
            .iter()
            .filter(|l| l.starts_with(NO_FILE))
            .cloned(),
    );
    if problems.is_empty() {
        return Ok(());
    }
    Err(failed(
        command,
        format!(
            "load order unverified, so its winning versions are not a safe source: {} (fix the load order, or name the plugins with `plugins`)",
            problems.join("; ")
        ),
    ))
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
        },
        origin,
    ))
}

/// The note for a copy from a small or medium plugin: its own records were found by
/// lane D's `full-style-self` assumption (`docs/load-order.md`).
pub(crate) fn scale_note(copy: &Copy) -> Option<String> {
    copy.source.scaled().then(|| {
        format!(
            "{} is a small or medium plugin: its version of {} was found by the `full-style-self` assumption (docs/load-order.md), not yet observed in the CK or in game",
            copy.source.name, copy.formref
        )
    })
}

/// Writes `copies` into `into` with the load order's string tables, protected files and
/// plugin flags.
pub(crate) fn write(
    command: &str,
    loaded: &mut Loaded,
    into: &Path,
    copies: Vec<Copy>,
) -> Result<PatchReport, EngineError> {
    let protected: Vec<PathBuf> = loaded.lo.plugins.iter().map(|p| p.path.clone()).collect();
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
    };
    write_patch(into, copies, &options).map_err(|e| match e {
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
    let loaded = acquire(session, args, ID)?;
    verify_load_order(ID, loaded)?;
    let limits = DecodeLimits::default();
    let g = find_form(ID, loaded, form, args.get("plugin").and_then(Value::as_str))?;
    let (copy, origin) = winning_copy(ID, loaded, g, edits, &limits)?;
    let mut limitations = loaded.limitations.clone();
    limitations.extend(scale_note(&copy));
    let report = write(ID, loaded, &into, vec![copy])?;
    Ok(outcome(&report, &[origin], limitations))
}
