//! The patch writer (M2.E4): copy records into a patch plugin, apply edits, write.
//!
//! Lossless first: a copy is every byte of the source record (header, flags, form
//! version, every subrecord in order, unknown ones included); edits change only the
//! subrecords they name (`kitcraft-esm`'s edit API), and a compressed record is
//! recompressed only when it changed. FormIDs are copied as stored.
//!
//! Rules (`plan/execution-plan.md` M2.E4, `docs/corpus.md` "Writing rules"):
//!
//! - **Non-localized.** Every lstring of a copied record from a localized plugin is written
//!   as inline text: the edited value, or the source's text from its string tables
//!   ([`super::strings`]); string ID 0 is written as the empty string. A string that cannot
//!   be resolved refuses the write, naming every unresolved subrecord (the Resupply door:
//!   a copied ID read as text by the game).
//! - **Masters.** A new target gets the first source's masters in order, then the source.
//!   A record joins a target whose master list starts with its source's masters plus the
//!   source (FormIDs then mean the same in both). When the target's list is shorter and a
//!   prefix of that, the target's list is extended (refused if the target has records of
//!   its own, whose FormIDs would change meaning). Otherwise the record's FormIDs are
//!   remapped through the generated FormID visitors (M2.B3), which is refused when any of
//!   its subrecords is opaque (a FormID could hide there), or when a plugin involved is
//!   small or medium (scale classes are lane D's).
//! - **Header.** A new target: `HEDR` 0.96, next object ID `0xFF001000`, form version 582,
//!   master flag when the file name ends in `.esm`, never localized, no `ONAM`; the `HEDR`
//!   record count is the majority meaning (lane A writes it). `.esl` is refused.
//! - **Where.** Never over a plugin in the load order, never inside the game's Data
//!   directory (a directory holding `Starfield.esm`) or an MO2 instance (a directory
//!   holding `ModOrganizer.ini`, or a profile with `modlist.txt` and `plugins.txt`), at any
//!   depth.

use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use kitcraft_esm::bridge::Ctx;
use kitcraft_esm::build::{set_masters, upsert_file_to_path, HeaderSpec, Placed, Upsert};
use kitcraft_esm::driver::RewriteOptions;
use kitcraft_esm::index::{index_file, read_indexed, IndexOptions, PluginIndex};
use kitcraft_esm::tes4::{Tes4, FLAG_LOCALIZED, FLAG_MASTER, FLAG_MEDIUM, FLAG_SMALL};
use kitcraft_esm::{DecodeLimits, GroupHeader, Plugin, Record, Signature};
use kitcraft_schema::FieldType;
use serde_json::{json, Value};

use super::fields::{self, FieldError, FieldPath, Op, ValueCtx};
use super::source::{file_name, SourcePlugin};
use super::strings::StringLookup;
use super::{apply_field, FieldChange};

/// Why a patch was not written.
#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    /// A field path or value is wrong (the caller's arguments).
    #[error(transparent)]
    Field(#[from] FieldError),
    /// The write would break a rule (module docs); nothing was written.
    #[error("{0}")]
    Refused(String),
    /// Reading or writing a plugin failed.
    #[error("{0}")]
    Io(String),
}

fn refused(message: impl Into<String>) -> PatchError {
    PatchError::Refused(message.into())
}

/// One edit: a path and what to do there.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    /// The field.
    pub path: FieldPath,
    /// The value or operation.
    pub op: Op,
}

/// One record to copy into the patch.
#[derive(Debug, Clone)]
pub struct Copy {
    /// The plugin it comes from.
    pub source: SourcePlugin,
    /// The record as read (decoded if compressed).
    pub record: Record<'static>,
    /// Its group path in the source (where the copy goes).
    pub path: Vec<GroupHeader>,
    /// Records owning the groups on `path` (cell, world, topic), outermost first, as
    /// unedited copies with their own paths; placed first unless the target has them.
    pub parents: Vec<(Record<'static>, Vec<GroupHeader>)>,
    /// The edits, applied in order.
    pub edits: Vec<Edit>,
    /// `Plugin.esm:00ABCDEF` of the form, for reports.
    pub formref: String,
}

/// What the writer needs besides the records.
pub struct PatchOptions<'a> {
    /// Text for lstrings of localized sources.
    pub strings: &'a dyn StringLookup,
    /// Files that must never be written (the load order's plugins).
    pub protected: Vec<PathBuf>,
    /// TES4 flags of plugins by lower-case file name, for formref values and the scale
    /// check (the sources and anything else the caller read).
    pub plugin_flags: Vec<(String, u32)>,
    /// Decompression bounds.
    pub limits: DecodeLimits,
}

/// One lstring written inline in place of a string ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inlined {
    /// `SIG[n]` (and the member, for a struct tail).
    pub at: String,
    /// The string ID the source held.
    pub id: u32,
    /// The text written.
    pub text: String,
    /// The table it came from (`None` for ID 0).
    pub table: Option<String>,
}

/// One record the patch now holds.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordReport {
    /// Signature.
    pub signature: String,
    /// The FormID as stored in the target.
    pub form_id: u32,
    /// `Plugin.esm:00ABCDEF`.
    pub formref: String,
    /// EditorID, if any.
    pub editor_id: Option<String>,
    /// The plugin copied from.
    pub source: String,
    /// `replaced` (the target had it) or `inserted`.
    pub outcome: &'static str,
    /// Whether this is a parent record copied only to hold the group of another.
    pub parent: bool,
    /// The edits.
    pub changes: Vec<FieldChange>,
    /// lstrings written inline from the string tables.
    pub inlined: Vec<Inlined>,
    /// Offset and length of the record in the written file.
    pub offset: Option<u64>,
    /// Bytes of the record in the written file, header included.
    pub length: Option<u64>,
    /// `(signature, offset, length)` of each edited subrecord's payload in the written file
    /// (`None` offsets inside a compressed record).
    pub edited_subrecords: Vec<(String, Option<u64>, u64)>,
}

impl RecordReport {
    /// The JSON shape `set` and `build` report.
    pub fn to_json(&self) -> Value {
        json!({
            "signature": self.signature,
            "form_id": format!("{:08X}", self.form_id),
            "form": self.formref,
            "editor_id": self.editor_id,
            "source": self.source,
            "outcome": self.outcome,
            "parent": self.parent,
            "fields": self.changes.iter().map(FieldChange::to_json).collect::<Vec<_>>(),
            "inlined_lstrings": self.inlined.iter().map(|i| json!({
                "at": i.at, "string_id": format!("0x{:08X}", i.id), "text": i.text, "table": i.table,
            })).collect::<Vec<_>>(),
            "offset": self.offset,
            "length": self.length,
        })
    }
}

/// What [`write_patch`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchReport {
    /// The file written.
    pub target: PathBuf,
    /// Whether the file was created (else updated).
    pub created: bool,
    /// The target's masters after the write.
    pub masters: Vec<String>,
    /// Raw TES4 flags of the target.
    pub flags: u32,
    /// Every record written, parents included, in write order.
    pub records: Vec<RecordReport>,
    /// Size of the written file.
    pub bytes: u64,
    /// Limitations to report.
    pub limitations: Vec<String>,
}

/// The `ck-diff` half of M2.E3 that is not observed yet (M2.O4).
pub const CK_DIFF_PENDING: &str = "header of a new plugin follows the corpus rules in \
     docs/corpus.md (Writing rules); what the Creation Kit writes for a new non-localized \
     plugin (flags, HEDR next object ID, inline text encoding) is not observed yet (M2.E3 \
     ck-diff part, M2.O4)";

/// Copies `copies` into the plugin at `target` (created, or updated in place), applying
/// their edits. See the module docs for the rules; on any refusal nothing is written.
pub fn write_patch(
    target: &Path,
    copies: Vec<Copy>,
    options: &PatchOptions<'_>,
) -> Result<PatchReport, PatchError> {
    if copies.is_empty() {
        return Err(refused("nothing to write: no records"));
    }
    guard_target(target, &options.protected)?;
    let target_name = file_name(target);
    let existing = if target.exists() {
        let index = index_file(target, &IndexOptions::default())
            .map_err(|e| PatchError::Io(format!("{}: {e}", target.display())))?;
        let plugin = SourcePlugin::from_index(target, &index);
        if plugin.localized() {
            return Err(refused(format!(
                "{target_name} is localized; M2 patches are non-localized and write no string tables"
            )));
        }
        if plugin.scaled() {
            return Err(refused(format!(
                "{target_name} has a small or medium flag; scaled patches wait for M2.D3 (c)"
            )));
        }
        Some((plugin, index))
    } else {
        None
    };

    // Pass 1: the target's master list, and which copies need FormIDs remapped.
    let original_masters: Vec<String> = existing
        .as_ref()
        .map(|(p, _)| p.masters.clone())
        .unwrap_or_default();
    let own_records = existing.as_ref().is_some_and(|(p, index)| {
        index
            .records
            .iter()
            .skip(1)
            .any(|r| usize::try_from(r.form_id >> 24).ok() == Some(p.masters.len()))
    });
    let mut masters = original_masters.clone();
    let mut remap = vec![false; copies.len()];
    for (copy, needs) in copies.iter().zip(remap.iter_mut()) {
        let mut wanted = copy.source.masters.clone();
        wanted.push(copy.source.name.clone());
        if wanted.iter().any(|m| m.eq_ignore_ascii_case(&target_name)) {
            return Err(refused(format!(
                "{} depends on {target_name}; a patch cannot be its own master",
                copy.source.name
            )));
        }
        if is_prefix(&wanted, &masters) {
            continue;
        }
        if is_prefix(&masters, &wanted) && !(own_records && masters.len() < wanted.len()) {
            masters = wanted;
            continue;
        }
        *needs = true;
        for name in wanted {
            if !masters.iter().any(|m| m.eq_ignore_ascii_case(&name)) {
                if own_records {
                    return Err(refused(format!(
                        "{target_name} has records of its own; adding the master {name} would change what their FormIDs mean"
                    )));
                }
                masters.push(name);
            }
        }
    }
    let flag_of = |name: &str| {
        options
            .plugin_flags
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, f)| *f)
    };

    // Pass 2: each copy into the target's FormID space, edits, lstrings.
    let mut limitations: Vec<String> = Vec::new();
    let mut placed: Vec<Placed> = Vec::new();
    let mut reports: Vec<RecordReport> = Vec::new();
    let mut seen: BTreeSet<(u32, [u8; 4])> = BTreeSet::new();
    for (copy, needs_remap) in copies.into_iter().zip(remap) {
        let source_masters = {
            let mut m = copy.source.masters.clone();
            m.push(copy.source.name.clone());
            m
        };
        let map = |raw: u32| -> Result<u32, String> { map_form_id(raw, &source_masters, &masters) };
        // Parents first (unedited), unless the target or an earlier copy has them.
        for (mut record, path) in copy.parents.clone() {
            if needs_remap {
                remap_record(&mut record, &copy.source, &map, &flag_of, &options.limits)?;
            }
            let key = (record.header.form_id, record.header.signature.0);
            let in_target = existing.as_ref().is_some_and(|(_, index)| {
                index.records.iter().any(|r| {
                    r.form_id == record.header.form_id && r.signature == record.header.signature
                })
            });
            if in_target || !seen.insert(key) {
                continue;
            }
            let inlined = inline_lstrings(&mut record, &copy.source, &BTreeSet::new(), options)?;
            reports.push(report_for(
                &record,
                &copy,
                &masters,
                true,
                Vec::new(),
                inlined,
            ));
            placed.push(Placed { path, record });
        }
        let mut record = copy.record.clone();
        if needs_remap {
            remap_record(&mut record, &copy.source, &map, &flag_of, &options.limits)?;
            limitations.push(format!(
                "{} {}: FormIDs remapped into {target_name}'s master list",
                record.header.signature, copy.formref
            ));
        }
        let target_masters = masters.clone();
        let formref = |plugin: &str, id: u32| -> Result<u32, String> {
            formref_in(plugin, id, &target_masters, &target_name, &flag_of)
        };
        let ctx = ValueCtx {
            localized_target: false,
            localized_source: copy.source.localized(),
            form_version: record.header.form_version,
            formref: &formref,
        };
        let mut changes = Vec::new();
        let mut written: BTreeSet<usize> = BTreeSet::new();
        for edit in &copy.edits {
            let change = apply_field(&mut record, &edit.path, &edit.op, &ctx, &options.limits)?;
            if let Some(note) = &change.note {
                if !limitations.contains(note) {
                    limitations.push(note.clone());
                }
            }
            if edit.path.steps.is_empty() {
                written.insert(change.subrecord_index);
            }
            changes.push(change);
        }
        let inlined = inline_lstrings(&mut record, &copy.source, &written, options)?;
        let key = (record.header.form_id, record.header.signature.0);
        if !seen.insert(key) {
            return Err(refused(format!(
                "{} {} is written twice in one patch; merge its edits into one",
                record.header.signature, copy.formref
            )));
        }
        reports.push(report_for(
            &record, &copy, &masters, false, changes, inlined,
        ));
        placed.push(Placed {
            path: copy.path,
            record,
        });
    }

    // Write.
    let created = existing.is_none();
    let extended = masters != original_masters;
    let (outcomes, flags): (Vec<Upsert>, u32) = match &existing {
        None => {
            let mut spec = HeaderSpec::new(masters.iter().map(|m| m.as_bytes().to_vec()).collect());
            if is_esm(target) {
                spec.flags |= FLAG_MASTER;
            }
            let mut plugin = Plugin::create(&spec);
            let mut outcomes = Vec::new();
            for p in placed {
                outcomes.push(
                    plugin
                        .upsert(&p.path, p.record)
                        .map_err(|e| PatchError::Io(format!("placing a record: {e}")))?,
                );
            }
            let bytes = plugin
                .to_bytes()
                .map_err(|e| PatchError::Io(format!("writing {target_name}: {e}")))?;
            write_atomically(target, &bytes)?;
            (outcomes, spec.flags)
        }
        Some((plugin, _)) => {
            let temp = temp_path(target);
            let names: Vec<Vec<u8>> = masters.iter().map(|m| m.as_bytes().to_vec()).collect();
            let report =
                upsert_file_to_path(target, &temp, &RewriteOptions::default(), placed, |tes4| {
                    if extended {
                        set_masters(tes4, &names);
                    }
                    Ok(())
                })
                .map_err(|e| {
                    let _ = std::fs::remove_file(&temp);
                    PatchError::Io(format!("updating {target_name}: {e}"))
                })?;
            std::fs::rename(&temp, target).map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                PatchError::Io(format!("replacing {}: {e}", target.display()))
            })?;
            (report.outcomes, plugin.flags)
        }
    };
    if extended && !created {
        limitations.push(format!(
            "{target_name}'s master list was extended to {}",
            masters.join(", ")
        ));
    }
    for (report, outcome) in reports.iter_mut().zip(&outcomes) {
        report.outcome = match outcome {
            Upsert::Replaced => "replaced",
            Upsert::Inserted => "inserted",
        };
    }
    let bytes = locate_written(target, &mut reports, &options.limits)?;
    limitations.push(CK_DIFF_PENDING.to_string());
    Ok(PatchReport {
        target: target.to_path_buf(),
        created,
        masters,
        flags,
        records: reports,
        bytes,
        limitations,
    })
}

fn is_esm(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("esm"))
}

/// Whether `prefix` is a (case-insensitive) prefix of `list`.
fn is_prefix(prefix: &[String], list: &[String]) -> bool {
    prefix.len() <= list.len()
        && prefix
            .iter()
            .zip(list)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// Refuses targets the module docs rule out.
fn guard_target(target: &Path, protected: &[PathBuf]) -> Result<(), PatchError> {
    let name = file_name(target);
    let ext = target
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "esm" | "esp" => {}
        "esl" => {
            return Err(refused(format!(
                "{name}: .esl (small) patches wait for M2.D3 (c); write .esm or .esp"
            )))
        }
        _ => {
            return Err(refused(format!(
                "{name}: the target must be a .esm or .esp file"
            )))
        }
    }
    let abs_target = absolute(target);
    for p in protected {
        if absolute(p) == abs_target {
            return Err(refused(format!(
                "{} is a plugin of the load order; a patch is always a separate file",
                target.display()
            )));
        }
    }
    let mut dir = abs_target.parent();
    while let Some(d) = dir {
        if d.join("Starfield.esm").is_file() {
            return Err(refused(format!(
                "{} is inside the game's Data directory ({}); write the patch elsewhere and install it through MO2",
                target.display(),
                d.display()
            )));
        }
        if d.join("ModOrganizer.ini").is_file()
            || (d.join("modlist.txt").is_file() && d.join("plugins.txt").is_file())
        {
            return Err(refused(format!(
                "{} is inside a Mod Organizer instance or profile ({}), which this tool never writes",
                target.display(),
                d.display()
            )));
        }
        dir = d.parent();
    }
    Ok(())
}

fn absolute(path: &Path) -> PathBuf {
    if let Ok(p) = std::fs::canonicalize(path) {
        return p;
    }
    // A file that does not exist yet: canonicalize its directory.
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => {
            let dir = if dir.as_os_str().is_empty() {
                Path::new(".")
            } else {
                dir
            };
            std::fs::canonicalize(dir)
                .map(|d| d.join(name))
                .unwrap_or_else(|_| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    }
}

/// A FormID stored by a plugin whose master list (masters, then itself) is `from`, as
/// stored in a plugin whose masters are `to`. FormID 0 (none) stays 0.
fn map_form_id(raw: u32, from: &[String], to: &[String]) -> Result<u32, String> {
    if raw == 0 {
        return Ok(0);
    }
    let index = usize::try_from(raw >> 24).unwrap_or(usize::MAX);
    let owner = from.get(index).ok_or_else(|| {
        format!("FormID {raw:08X} names master {index}, beyond the source's list")
    })?;
    let at = to
        .iter()
        .position(|m| m.eq_ignore_ascii_case(owner))
        .and_then(|i| u32::try_from(i).ok())
        .ok_or_else(|| format!("FormID {raw:08X}: {owner} is not a master of the target"))?;
    Ok((at << 24) | (raw & 0x00FF_FFFF))
}

/// A formref value as stored in the target.
fn formref_in(
    plugin: &str,
    id: u32,
    masters: &[String],
    target: &str,
    flag_of: &dyn Fn(&str) -> Option<u32>,
) -> Result<u32, String> {
    if plugin.eq_ignore_ascii_case(target) {
        return Err(format!(
            "{plugin}:{id:06X}: the patch creates no forms of its own in M2"
        ));
    }
    let at = masters
        .iter()
        .position(|m| m.eq_ignore_ascii_case(plugin))
        .ok_or_else(|| {
            format!(
                "{plugin} is not a master of the target (masters: {})",
                masters.join(", ")
            )
        })?;
    match flag_of(plugin) {
        None => Err(format!(
            "{plugin}'s header was not read, so its scale (full, small, medium) is unknown; add it to the load order"
        )),
        Some(f) if f & (FLAG_SMALL | FLAG_MEDIUM) != 0 => Err(format!(
            "{plugin} is a small or medium plugin; FormIDs into it need lane D's scale-aware mapping"
        )),
        Some(_) => u32::try_from(at)
            .ok()
            .filter(|i| *i < 0xFD)
            .map(|i| (i << 24) | id)
            .ok_or_else(|| format!("{plugin}: master index {at} out of range")),
    }
}

/// Remaps every FormID of `record` (header and typed members) with `map`; refused when a
/// subrecord could hold FormIDs we cannot see.
fn remap_record(
    record: &mut Record<'static>,
    source: &SourcePlugin,
    map: &dyn Fn(u32) -> Result<u32, String>,
    flag_of: &dyn Fn(&str) -> Option<u32>,
    limits: &DecodeLimits,
) -> Result<(), PatchError> {
    let what = format!("{} {:08X}", record.header.signature, record.header.form_id);
    let mut involved: Vec<&str> = source.masters.iter().map(String::as_str).collect();
    involved.push(&source.name);
    for name in involved {
        if flag_of(name).is_none_or(|f| f & (FLAG_SMALL | FLAG_MEDIUM) != 0) {
            return Err(refused(format!(
                "{what} needs its FormIDs remapped, but the scale of {name} is unknown or small/medium (lane D)"
            )));
        }
    }
    record
        .decompress(limits)
        .map_err(|e| PatchError::Io(format!("{what}: {e}")))?;
    let sig = record.header.signature.0;
    for sub in record.subrecords().unwrap_or_default() {
        let text = sub.signature().to_string();
        let opaque = match fields::layout_for(&text, sig, sub.signature().0)? {
            Some(layout) => has_opaque(&layout.ty),
            None => true,
        };
        if opaque {
            return Err(refused(format!(
                "{what} needs its FormIDs remapped (its masters differ from the target's), but its {text} subrecord is opaque and could hold FormIDs; write it to a new patch instead"
            )));
        }
    }
    let ctx = Ctx::for_record(record, source.localized());
    let mut typed = record
        .typed(ctx)
        .map_err(|_| refused(format!("{what}: no typed definition to remap through")))?;
    if !typed.extra().is_empty() {
        return Err(refused(format!(
            "{what}: some subrecords do not decode, so their FormIDs cannot be remapped"
        )));
    }
    for (_, id) in typed.form_ids_mut() {
        id.0 = map(id.0).map_err(|m| refused(format!("{what}: {m}")))?;
    }
    record.set_typed(&typed);
    record.header.form_id =
        map(record.header.form_id).map_err(|m| refused(format!("{what}: {m}")))?;
    Ok(())
}

fn has_opaque(ty: &FieldType) -> bool {
    match ty {
        FieldType::Opaque { .. } | FieldType::Union { .. } => true,
        FieldType::Struct { fields } => fields.iter().any(|f| has_opaque(&f.ty)),
        FieldType::Array { element, .. } => has_opaque(element),
        FieldType::Ref { ty, .. } => has_opaque(ty),
        _ => false,
    }
}

/// The lstring spans of one subrecord: `(member label, start, len)`.
fn lstring_spans(
    record_sig: [u8; 4],
    sub_sig: [u8; 4],
    payload: &[u8],
    form_version: u16,
) -> Result<Vec<(String, usize, usize)>, FieldError> {
    let text = String::from_utf8_lossy(&sub_sig).into_owned();
    let Some(layout) = fields::layout_for(&text, record_sig, sub_sig)? else {
        return Ok(Vec::new());
    };
    Ok(match &layout.ty {
        FieldType::LString => vec![(String::new(), 0, payload.len())],
        FieldType::Struct { fields } => {
            let last = fields
                .iter()
                .rev()
                .find(|f| f.present_in(Some(form_version)));
            match last {
                Some(f) if matches!(f.ty, FieldType::LString) => {
                    let steps = [fields::Step::Member(f.name.clone())];
                    match fields::locate(&text, &layout.ty, payload, &steps, form_version) {
                        Ok(at) => vec![(format!(".{}", f.name), at.start, at.len)],
                        Err(_) => Vec::new(),
                    }
                }
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    })
}

/// Writes every lstring of a localized source's record as inline text (module docs).
/// `written` holds subrecord indices an edit already set whole.
fn inline_lstrings(
    record: &mut Record<'static>,
    source: &SourcePlugin,
    written: &BTreeSet<usize>,
    options: &PatchOptions<'_>,
) -> Result<Vec<Inlined>, PatchError> {
    if !source.localized() {
        return Ok(Vec::new());
    }
    let what = format!("{} {:08X}", record.header.signature, record.header.form_id);
    record
        .decompress(&options.limits)
        .map_err(|e| PatchError::Io(format!("{what}: {e}")))?;
    let sig = record.header.signature.0;
    let form_version = record.header.form_version;
    let subs: Vec<(Signature, Vec<u8>)> = record
        .subrecords()
        .unwrap_or_default()
        .iter()
        .map(|s| (s.signature(), s.data().to_vec()))
        .collect();
    let mut nth: std::collections::BTreeMap<[u8; 4], usize> = std::collections::BTreeMap::new();
    let mut inlined = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut updates: Vec<(usize, Vec<u8>)> = Vec::new();
    for (i, (s, payload)) in subs.iter().enumerate() {
        let n = nth.entry(s.0).or_insert(0);
        let occurrence = *n;
        *n = n.saturating_add(1);
        if written.contains(&i) {
            continue;
        }
        let spans = lstring_spans(sig, s.0, payload, form_version)?;
        let mut next = payload.clone();
        for (member, start, len) in spans.into_iter().rev() {
            let at = format!("{s}[{occurrence}]{member}");
            let Some(id) = payload
                .get(start..start.saturating_add(len))
                .and_then(|b| <[u8; 4]>::try_from(b).ok())
                .map(u32::from_le_bytes)
            else {
                unresolved.push(format!(
                    "{at}: a localized lstring of {len} bytes (not a 4-byte string ID)"
                ));
                continue;
            };
            let (text, table) = if id == 0 {
                (Vec::new(), None)
            } else {
                match options.strings.text(&source.name, id) {
                    Ok(Some(found)) => (found.bytes, Some(found.table)),
                    Ok(None) => {
                        unresolved.push(format!(
                            "{at}: string 0x{id:08X} is in none of {}'s tables",
                            source.name
                        ));
                        continue;
                    }
                    Err(e) => {
                        unresolved.push(format!("{at}: string 0x{id:08X}: {e}"));
                        continue;
                    }
                }
            };
            if text.contains(&0) {
                unresolved.push(format!("{at}: string 0x{id:08X} contains a NUL"));
                continue;
            }
            let mut inline = text.clone();
            inline.push(0);
            next.splice(start..start.saturating_add(len), inline);
            inlined.push(Inlined {
                at,
                id,
                text: String::from_utf8_lossy(&text).into_owned(),
                table,
            });
        }
        if next != *payload {
            updates.push((i, next));
        }
    }
    if !unresolved.is_empty() {
        return Err(refused(format!(
            "{what} from localized {} has lstrings that cannot be written as text (patches are non-localized; copying a string ID makes the game show garbage): {}",
            source.name,
            unresolved.join("; ")
        )));
    }
    for (i, data) in updates {
        record
            .set_subrecord(i, data, &options.limits)
            .map_err(|e| PatchError::Io(format!("{what}: {e}")))?;
    }
    Ok(inlined)
}

fn report_for(
    record: &Record<'static>,
    copy: &Copy,
    masters: &[String],
    parent: bool,
    changes: Vec<FieldChange>,
    inlined: Vec<Inlined>,
) -> RecordReport {
    let editor_id = record
        .subrecords()
        .and_then(|subs| subs.iter().find(|s| s.signature() == Signature(*b"EDID")))
        .and_then(|s| fields::text_of(s.data()));
    let formref = if parent {
        usize::try_from(record.header.form_id >> 24)
            .ok()
            .and_then(|i| masters.get(i))
            .map(|o| format!("{o}:{:08X}", record.header.form_id & 0x00FF_FFFF))
            .unwrap_or_default()
    } else {
        copy.formref.clone()
    };
    RecordReport {
        signature: record.header.signature.to_string(),
        form_id: record.header.form_id,
        formref,
        editor_id,
        source: copy.source.name.clone(),
        outcome: "inserted",
        parent,
        changes,
        inlined,
        offset: None,
        length: None,
        edited_subrecords: Vec::new(),
    }
}

/// Finds each written record in the output (evidence) and returns the file size.
fn locate_written(
    target: &Path,
    reports: &mut [RecordReport],
    limits: &DecodeLimits,
) -> Result<u64, PatchError> {
    let io = |e: kitcraft_esm::Error| PatchError::Io(format!("{}: {e}", target.display()));
    let index: PluginIndex = index_file(target, &IndexOptions::default()).map_err(io)?;
    let mut file = File::open(target).map_err(|e| PatchError::Io(e.to_string()))?;
    for report in reports {
        let Some(loc) =
            index.records.iter().skip(1).find(|r| {
                r.form_id == report.form_id && r.signature.to_string() == report.signature
            })
        else {
            continue;
        };
        report.offset = Some(loc.offset);
        report.length = Some(loc.len());
        if report.changes.is_empty() {
            continue;
        }
        let record = read_indexed(&mut file, loc, Some(limits)).map_err(io)?;
        let subs = record.subrecords().unwrap_or_default();
        for change in &report.changes {
            if let Some(sub) = subs.get(change.subrecord_index) {
                let offset = if record.header.is_compressed() {
                    None
                } else {
                    sub.data_offset()
                };
                report.edited_subrecords.push((
                    sub.signature().to_string(),
                    offset,
                    sub.data().len() as u64,
                ));
            }
        }
    }
    Ok(index.file_len)
}

fn temp_path(target: &Path) -> PathBuf {
    let name = file_name(target);
    target.with_file_name(format!(".{name}.kitcraft-{}.tmp", std::process::id()))
}

fn write_atomically(target: &Path, bytes: &[u8]) -> Result<(), PatchError> {
    let temp = temp_path(target);
    std::fs::write(&temp, bytes)
        .and_then(|()| std::fs::rename(&temp, target))
        .map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            PatchError::Io(format!("writing {}: {e}", target.display()))
        })
}

/// The flags a source contributes to [`PatchOptions::plugin_flags`].
pub fn flags_entry(plugin: &SourcePlugin) -> (String, u32) {
    (plugin.name.to_ascii_lowercase(), plugin.flags)
}

/// Whether `flags` has the localized bit.
pub fn is_localized(flags: u32) -> bool {
    flags & FLAG_LOCALIZED != 0
}

/// The masters of the plugin at `path` and its flags (for callers checking a target).
pub fn header_facts(path: &Path) -> Result<(Vec<String>, u32), String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let header = kitcraft_esm::index::read_record_at(&mut file, 0, None)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let tes4 = Tes4::new(&header);
    Ok((
        tes4.masters().iter().map(|m| m.name_lossy()).collect(),
        tes4.flags(),
    ))
}
