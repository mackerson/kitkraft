//! The patch writer (M2.E4): copy records into a patch plugin, apply edits, write.
//!
//! Lossless first: a copy is every byte of the source record (header, flags, form
//! version, every subrecord in order, unknown ones included); edits change only the
//! subrecords they name (`kitkraft-esm`'s edit API), and a compressed record is
//! recompressed only when it changed. FormIDs are copied as stored.
//!
//! Rules (`plan/execution-plan.md` M2.E4, `docs/corpus.md` "Writing rules"):
//!
//! - **Non-localized.** Every lstring of a copied record from a localized plugin is written
//!   as inline text: the edited value, or the source's text from its string tables
//!   ([`super::strings`]); string ID 0 is written as the empty string. A string that cannot
//!   be resolved refuses the write, naming every unresolved subrecord (the Resupply door:
//!   a copied ID read as text by the game). Lstrings are the schema's, `FULL`/`DESC`, and
//!   the corpus table's ([`super::lstrings`]); a non-zero 4-byte subrecord none of them
//!   describes may be a string ID too, so it refuses the write unless
//!   [`PatchOptions::allow_untyped_lstrings`] (then a limitation names it). Inline text with
//!   bytes >= 0x80 is named in a limitation (how the game decodes it is unverified).
//! - **Scale.** A copy whose source, or a master of the source, is small or medium (or of
//!   unknown scale) is refused: its FormIDs address masters by scale class (and its own
//!   records full-style-self), and a verbatim copy into a full-scale patch would change
//!   what they mean, until a scale-aware remap exists.
//! - **Masters.** A new target gets the first source's masters in order, then the source.
//!   A record joins a target whose master list starts with its source's masters plus the
//!   source (FormIDs then mean the same in both). When the target's list is shorter and a
//!   prefix of that, the target's list is extended (refused if the target has records of
//!   its own, whose FormIDs would change meaning). Otherwise the record's FormIDs are
//!   remapped through the generated FormID visitors (M2.B3), which is refused when any of
//!   its subrecords is opaque (a FormID could hide there), or when a plugin involved is
//!   small or medium (scale classes are lane D's).
//! - **Header.** A new target: `HEDR` 0.96, next object ID `0xFF001000`, form version 582,
//!   `CNAM` "DEFAULT", the masters, `INCC` (interior cells), master flag when the file name
//!   ends in `.esm`, never localized, no `ONAM`, `SNAM`, `BNAM` or `CHGL`; the `HEDR` record
//!   count is the majority meaning (lane A writes it). `.esl` is refused.
//! - **Atomic.** The file is written next to the target under a temporary name, every
//!   written record is located in it (the evidence), and only then is it renamed over the
//!   target; any failure leaves the target as it was.
//! - **Where.** Never over a plugin in the load order, never inside the game's Data
//!   directory (a directory holding `Starfield.esm`), the user Data directory
//!   (`My Games\Starfield\Data`) or an MO2 instance (a directory holding
//!   `ModOrganizer.ini`, or a profile with `modlist.txt` and `plugins.txt`), at any depth.

use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use kitkraft_esm::bridge::Ctx;
use kitkraft_esm::build::{set_masters, upsert_file_to_path, HeaderSpec, Placed, Upsert};
use kitkraft_esm::cells::{CellPlace, PlacementStyle};
use kitkraft_esm::driver::RewriteOptions;
use kitkraft_esm::forms::NewForms;
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions, PluginIndex};
use kitkraft_esm::refs::{compose_reference, RefSpec};
use kitkraft_esm::tes4::{Scale, Tes4, FLAG_LOCALIZED, FLAG_MASTER, FLAG_MEDIUM, FLAG_SMALL};
use kitkraft_esm::{DecodeLimits, GroupHeader, Plugin, Record, Signature, Subrecord};
use kitkraft_schema::FieldType;
use serde_json::{json, Value};

use super::fields::{self, FieldError, FieldPath, Op, ValueCtx};
use super::lstrings;
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
    /// Carried only to hold the groups of a new record (`place`'s cell): reported as a
    /// parent, never edited.
    pub carried: bool,
    /// A new form (`new`, `clone`, M4.E1) with this EditorID: the record gets a new own
    /// FormID from the allocator once the master list is final, and its `EDID` set. A
    /// `new` record has a source with an empty name, whose `masters` are the masters it
    /// asks for.
    pub new_form: Option<String>,
}

/// A form named by its defining plugin's file name and its object ID, mapped into the
/// target's master list by the writer once that list is final.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormName {
    /// The defining plugin (`Starfield.esm`).
    pub plugin: String,
    /// The object ID (low 24 bits).
    pub object_id: u32,
}

impl FormName {
    /// `Starfield.esm:00ABCDEF`.
    pub fn label(&self) -> String {
        format!("{}:{:08X}", self.plugin, self.object_id)
    }
}

/// The cell a new reference goes into ([`kitkraft_esm::cells::CellPlace`] in names).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefPlace {
    /// An interior cell.
    Interior { cell: FormName },
    /// An exterior cell of `world` at `grid`.
    Exterior {
        world: FormName,
        cell: FormName,
        grid: kitkraft_esm::cells::Grid,
    },
    /// The persistent cell of `world` (where a world's persistent references live).
    Persistent { world: FormName, cell: FormName },
}

impl RefPlace {
    /// The cell.
    pub fn cell(&self) -> &FormName {
        match self {
            RefPlace::Interior { cell }
            | RefPlace::Exterior { cell, .. }
            | RefPlace::Persistent { cell, .. } => cell,
        }
    }

    /// The world, for an exterior or persistent cell.
    pub fn world(&self) -> Option<&FormName> {
        match self {
            RefPlace::Interior { .. } => None,
            RefPlace::Exterior { world, .. } | RefPlace::Persistent { world, .. } => Some(world),
        }
    }
}

/// A new placed reference (`place`, M4.E2): composed by `kitkraft_esm::refs` with a FormID
/// from `kitkraft_esm::forms::NewForms` once the target's master list is final, and placed
/// by `kitkraft_esm::cells::placement`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRef {
    /// `REFR`, or `ACHR` for an NPC base.
    pub signature: Signature,
    /// The base form.
    pub base: FormName,
    /// Instead of `base`: a new form written in the same call, by its EditorID (a spec's
    /// `new:<name>` naming a `[[new]]` or `[[clone]]`).
    pub base_local: Option<String>,
    /// Where it goes.
    pub place: RefPlace,
    /// Position X, Y, Z.
    pub position: [f32; 3],
    /// Rotation X, Y, Z.
    pub rotation: [f32; 3],
    /// Persistent (record flag 0x400, persistent children).
    pub persistent: bool,
    /// Initially disabled (record flag 0x800; below `corpus` confidence).
    pub initially_disabled: bool,
    /// EditorID.
    pub editor_id: Option<String>,
    /// Members below `corpus` confidence the caller allows (`DATA.Rotation`, ...).
    pub allow_reference: Vec<String>,
    /// The entry that asked for it (`place`, `place[0]`), for reports.
    pub label: String,
}

/// What the writer did for one [`NewRef`].
#[derive(Debug, Clone, PartialEq)]
pub struct NewFormReport {
    /// The entry that asked for it.
    pub label: String,
    /// The new record's FormID as stored.
    pub form_id: u32,
    /// The base's FormID as stored in the target.
    pub base: u32,
    /// The cell's FormID as stored in the target.
    pub cell: u32,
    /// The world's, for an exterior or persistent cell.
    pub world: Option<u32>,
    /// The group path it was placed at: `(group type, label as u32)`, outermost first.
    pub path: Vec<(i32, u32)>,
    /// Every member written, with its confidence and source.
    pub members: Vec<kitkraft_esm::refs::MemberUse>,
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
    /// Copy 4-byte subrecords of unknown layout from a localized source as stored (each
    /// named in a limitation) instead of refusing (module docs, "Non-localized").
    pub allow_untyped_lstrings: bool,
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
    /// The new records ([`NewRef`]) written, in order.
    pub new_forms: Vec<NewFormReport>,
    /// The `HEDR` next object ID after the write, when new forms advanced it.
    pub next_object_id: Option<u32>,
}

/// The `ck-diff` half of M2.E3 that is not observed yet (M2.O4), and the header
/// subrecords the official files have that a new patch does not write.
pub const CK_DIFF_PENDING: &str = "header of a new plugin follows the corpus rules in \
     docs/corpus.md (Writing rules): CNAM \"DEFAULT\" and INCC (interior cell count) as in \
     the two BlueprintShips files; SNAM, BNAM (branch name) and CHGL (changelist), which \
     the official build pipeline writes, are deliberately not written. What the Creation Kit \
     writes for a new non-localized plugin (flags, HEDR next object ID, these subrecords, \
     inline text encoding) is not observed yet (M2.E3 ck-diff part, M2.O4)";

/// `CNAM` (author) of a new patch: the value the two official files not built by the
/// "Python"/"Script" pipeline carry (`BlueprintShips-Starfield.esm`,
/// `BlueprintShips-SFBGS050.esm`; `corpus:` `corpus_tes4_metadata`).
pub const NEW_PLUGIN_AUTHOR: &[u8] = b"DEFAULT";

const INCC: Signature = Signature(*b"INCC");

/// Whether `placed` is an interior cell: a `CELL` under the top-level `CELL` group
/// (exterior cells live under `WRLD`). `INCC` counts these (`corpus:` all 14 files,
/// `corpus_tes4_metadata`).
fn is_interior_cell(placed: &Placed) -> bool {
    placed.record.header.signature == Signature(*b"CELL")
        && placed
            .path
            .first()
            .is_some_and(|g| g.group_type == 0 && g.label == *b"CELL")
}

/// Refuses a copy whose FormIDs use scale classes (Fable review round 2, finding 3): its
/// source is small or medium (its own records are found by lane D's `full-style-self`
/// assumption and its FormIDs address masters by scale class), or one of the source's
/// masters is, or a master's scale is unknown. A verbatim copy into a full-scale patch
/// would change what those FormIDs mean; refused until a scale-aware remap exists.
fn refuse_scaled(copy: &Copy, flag_of: &dyn Fn(&str) -> Option<u32>) -> Result<(), PatchError> {
    let what = format!("{} {}", copy.record.header.signature, copy.formref);
    if copy.source.scaled() {
        return Err(refused(format!(
            "{what}: its version comes from {}, a small or medium plugin, whose FormIDs address masters by scale class; a verbatim copy into a full-scale patch would change what they mean (refused until a scale-aware remap exists, lane D)",
            copy.source.name
        )));
    }
    for m in &copy.source.masters {
        match flag_of(m) {
            None => {
                return Err(refused(format!(
                    "{what}: the scale of {m}, a master of {}, is unknown (its header was not read), so what the copy's FormIDs mean cannot be checked; add it to the load order",
                    copy.source.name
                )))
            }
            Some(f) if f & (FLAG_SMALL | FLAG_MEDIUM) != 0 => {
                return Err(refused(format!(
                    "{what}: {} has the small or medium master {m}, so its FormIDs address masters by scale class; a verbatim copy into a full-scale patch could change what they mean (refused until a scale-aware remap exists, lane D)",
                    copy.source.name
                )))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Copies `copies` into the plugin at `target` (created, or updated in place), applying
/// their edits. See the module docs for the rules; on any refusal nothing is written.
pub fn write_patch(
    target: &Path,
    copies: Vec<Copy>,
    options: &PatchOptions<'_>,
) -> Result<PatchReport, PatchError> {
    write_patch_with(target, copies, Vec::new(), options)
}

/// [`write_patch`] that also adds new placed references (`place`, M4.E2): after the
/// copies (a reference's cell, and its world, are carried as copies), each [`NewRef`] gets
/// a new own FormID (`kitkraft_esm::forms::NewForms`: full-scale targets only), its base
/// and cell mapped into the final master list (the plugins they name are added as masters
/// when missing, which a target with records of its own refuses), is composed by
/// `kitkraft_esm::refs::compose_reference` (members below `corpus` refused unless allowed)
/// and placed by `kitkraft_esm::cells::placement`; the `HEDR` next object ID is advanced.
pub fn write_patch_with(
    target: &Path,
    copies: Vec<Copy>,
    new_refs: Vec<NewRef>,
    options: &PatchOptions<'_>,
) -> Result<PatchReport, PatchError> {
    if copies.is_empty() && new_refs.is_empty() {
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

    let flag_of = |name: &str| {
        options
            .plugin_flags
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, f)| *f)
    };

    // Pass 1: no scaled sources, the target's master list, and which copies need FormIDs
    // remapped.
    for copy in copies.iter().filter(|c| !c.source.name.is_empty()) {
        refuse_scaled(copy, &flag_of)?;
    }
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
        if copy.source.name.is_empty() {
            // A `new` record: the masters it asks for join the list (no FormIDs of its
            // own to keep in place).
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
            continue;
        }
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
    // New references name their base, cell and world: those plugins become masters.
    for new in &new_refs {
        let mut named = vec![new.place.cell()];
        if new.base_local.is_none() {
            named.push(&new.base);
        }
        named.extend(new.place.world());
        for form in named {
            let name = &form.plugin;
            if name.eq_ignore_ascii_case(&target_name) {
                return Err(refused(format!(
                    "{}: {} is a form of {target_name} itself; a new reference names forms of the target's masters",
                    new.label,
                    form.label()
                )));
            }
            if !masters.iter().any(|m| m.eq_ignore_ascii_case(name)) {
                if own_records {
                    return Err(refused(format!(
                        "{}: {target_name} has records of its own; adding the master {name} (for {}) would change what their FormIDs mean; write into a new plugin",
                        new.label,
                        form.label()
                    )));
                }
                masters.push(name.clone());
            }
        }
    }
    // Pass 2: each copy into the target's FormID space, edits, lstrings.
    let mut limitations: Vec<String> = Vec::new();
    let mut placed: Vec<Placed> = Vec::new();
    let mut reports: Vec<RecordReport> = Vec::new();
    let mut seen: BTreeSet<(u32, [u8; 4])> = BTreeSet::new();
    // New forms waiting for their FormID: (index in `placed` and `reports`, EditorID).
    let mut pending_new: Vec<(usize, String)> = Vec::new();
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
            let inlined = inline_lstrings(
                &mut record,
                &copy.source,
                &BTreeSet::new(),
                options,
                &mut limitations,
            )?;
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
        // A carried record the target already has stays as it is, where it is (its group
        // path there may differ from the source's: a block the formula does not predict).
        if copy.carried
            && existing.as_ref().is_some_and(|(_, index)| {
                index.records.iter().any(|r| {
                    r.form_id == copy.record.header.form_id
                        && r.signature == copy.record.header.signature
                })
            })
        {
            push_once(
                &mut limitations,
                format!(
                    "{} {} is already in {target_name}; kept as it is",
                    copy.record.header.signature, copy.formref
                ),
            );
            continue;
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
            for note in &change.notes {
                push_once(&mut limitations, note.clone());
            }
            if marks_written(edit, &change) {
                written.insert(change.subrecord_index);
            }
            changes.push(change);
        }
        let inlined = inline_lstrings(
            &mut record,
            &copy.source,
            &written,
            options,
            &mut limitations,
        )?;
        let key = (record.header.form_id, record.header.signature.0);
        if let Some(edid) = &copy.new_form {
            pending_new.push((placed.len(), edid.clone()));
        } else if !seen.insert(key) {
            return Err(refused(format!(
                "{} {} is written twice in one patch; merge its edits into one",
                record.header.signature, copy.formref
            )));
        }
        reports.push(report_for(
            &record,
            &copy,
            &masters,
            copy.carried,
            changes,
            inlined,
        ));
        placed.push(Placed {
            path: copy.path,
            record,
        });
    }

    // New references: FormIDs from the allocator over the final header, then composed and
    // placed.
    let created_header = |masters: &[String]| {
        let mut spec = HeaderSpec::new(masters.iter().map(|m| m.as_bytes().to_vec()).collect());
        spec.author = Some(NEW_PLUGIN_AUTHOR.to_vec());
        if is_esm(target) {
            spec.flags |= FLAG_MASTER;
        }
        spec
    };
    let mut forms: Option<NewForms> = None;
    let mut new_forms: Vec<NewFormReport> = Vec::new();
    if !new_refs.is_empty() || !pending_new.is_empty() {
        // The allocator's own prefix is the master count, which holds only when every
        // master is full-scale (small and medium masters take no full slot in the game).
        for m in &masters {
            match flag_of(m) {
                None => {
                    return Err(refused(format!(
                        "place.scaled-master: the scale of {m}, a master of {target_name}, is unknown (its header was not read), so the FormID of a new form cannot be chosen; add it to the load order"
                    )))
                }
                Some(f) if f & (FLAG_SMALL | FLAG_MEDIUM) != 0 => {
                    return Err(refused(format!(
                        "place.scaled-master: {m}, a master of {target_name}, is a small or medium plugin; new forms are written only when every master is full-scale (the own-FormID prefix counts full masters; refused until lane A's scale-aware allocator)"
                    )))
                }
                Some(_) => {}
            }
        }
        let names: Vec<Vec<u8>> = masters.iter().map(|m| m.as_bytes().to_vec()).collect();
        // Every master is full-scale here (checked just above); lane A's allocator takes
        // the scales in master-list order.
        let master_scales: Vec<Option<Scale>> = masters
            .iter()
            .map(|m| flag_of(m).and_then(Scale::of_flags))
            .collect();
        let allocator = match &existing {
            None => NewForms::new(&created_header(&masters).to_record(), &master_scales, None),
            Some((_, index)) if masters == original_masters => {
                NewForms::for_index(index, &master_scales)
            }
            Some((_, index)) => {
                // Extended masters: the target has no records of its own (checked above).
                let mut header = index.header.clone();
                set_masters(&mut header, &names);
                NewForms::new(&header, &master_scales, None)
            }
        }
        .map_err(|e| refused(format!("{target_name}: {e}")))?;
        let mut allocator = allocator;
        // New forms (`new`, `clone`): their FormIDs and EditorIDs.
        let mut local: Vec<(String, u32)> = Vec::new();
        for (i, edid) in &pending_new {
            let form_id = allocator
                .allocate()
                .map_err(|e| refused(format!("{edid}: {e}")))?;
            let (Some(p), Some(r)) = (placed.get_mut(*i), reports.get_mut(*i)) else {
                return Err(PatchError::Io("a new form lost its place".into()));
            };
            p.record
                .decompress(&options.limits)
                .map_err(|e| PatchError::Io(format!("{edid}: {e}")))?;
            p.record.header.form_id = form_id;
            let mut data = edid.as_bytes().to_vec();
            data.push(0);
            let edid_sig = Signature(*b"EDID");
            if let Some(subs) = p.record.subrecords_mut() {
                match subs.iter_mut().find(|s| s.signature() == edid_sig) {
                    Some(s) => s.set_data(data),
                    None => subs.insert(0, Subrecord::new(edid_sig, data)),
                }
            }
            if !seen.insert((form_id, p.record.header.signature.0)) {
                return Err(refused(format!(
                    "{edid}: FormID {form_id:08X} allocated twice"
                )));
            }
            r.source = format!("new ({})", r.formref);
            r.formref = format!("{target_name}:{:08X}", form_id & 0x00FF_FFFF);
            r.form_id = form_id;
            r.editor_id = Some(edid.clone());
            local.push((edid.clone(), form_id));
        }
        let target_masters = masters.clone();
        let stored = |form: &FormName| -> Result<u32, PatchError> {
            formref_in(
                &form.plugin,
                form.object_id,
                &target_masters,
                &target_name,
                &flag_of,
            )
            .map_err(|m| refused(format!("{}: {m}", form.label())))
        };
        for new in new_refs {
            let base = match &new.base_local {
                Some(edid) => local
                    .iter()
                    .find(|(e, _)| e.eq_ignore_ascii_case(edid))
                    .map(|(_, id)| *id)
                    .ok_or_else(|| {
                        refused(format!(
                            "{}: no new form `{edid}` is written in this call",
                            new.label
                        ))
                    })?,
                None => stored(&new.base)?,
            };
            let cell = stored(new.place.cell())?;
            let world = new.place.world().map(stored).transpose()?;
            let place = match (&new.place, world) {
                (RefPlace::Interior { .. }, _) => CellPlace::INTERIOR,
                (RefPlace::Exterior { grid, .. }, Some(world)) => {
                    CellPlace::Exterior { world, grid: *grid }
                }
                (RefPlace::Persistent { .. }, Some(world)) => CellPlace::Persistent { world },
                _ => {
                    return Err(refused(format!(
                        "{}: a world cell without its world",
                        new.label
                    )))
                }
            };
            let form_id = allocator
                .allocate()
                .map_err(|e| refused(format!("{}: {e}", new.label)))?;
            let mut spec = RefSpec::new(new.signature, form_id, base, new.position, new.rotation);
            spec.persistent = new.persistent;
            spec.initially_disabled = new.initially_disabled;
            spec.editor_id = new.editor_id.as_ref().map(|e| e.as_bytes().to_vec());
            let allow: Vec<&str> = new.allow_reference.iter().map(String::as_str).collect();
            let mut composed = compose_reference(&spec, &allow)
                .map_err(|e| refused(format!("{}: {e}", new.label)))?;
            let mut at = composed
                .placed(place, cell, &PlacementStyle::OFFICIAL)
                .map_err(|e| refused(format!("{}: {e}", new.label)))?;
            // The cell's own group path where it is observed (the target, else the
            // carried copy's source), not recomputed: a cell may sit in a block the
            // formula does not predict. The computed children groups follow it.
            let in_target = existing.as_ref().and_then(|(_, index)| {
                index
                    .records
                    .iter()
                    .find(|r| r.form_id == cell && r.signature == Signature(*b"CELL"))
                    .map(|r| index.group_headers(r))
            });
            let observed = in_target.or_else(|| {
                placed
                    .iter()
                    .find(|p| {
                        p.record.header.form_id == cell
                            && p.record.header.signature == Signature(*b"CELL")
                    })
                    .map(|p| p.path.clone())
            });
            let Some(observed) = observed else {
                return Err(refused(format!(
                    "place.parent-not-carried: {}: the cell {} is neither in {target_name} nor carried into it; a children group needs its CELL record first",
                    new.label,
                    new.place.cell().label()
                )));
            };
            if let Some(w) = world {
                let has_world = existing.as_ref().is_some_and(|(_, index)| {
                    index
                        .records
                        .iter()
                        .any(|r| r.form_id == w && r.signature == Signature(*b"WRLD"))
                }) || placed.iter().any(|p| {
                    p.record.header.form_id == w && p.record.header.signature == Signature(*b"WRLD")
                });
                if !has_world {
                    return Err(refused(format!(
                        "place.parent-not-carried: {}: the world {} is neither in {target_name} nor carried into it",
                        new.label,
                        new.place.world().map(FormName::label).unwrap_or_default()
                    )));
                }
            }
            let children = at.path.split_off(at.path.len().saturating_sub(2));
            at.path = observed;
            at.path.extend(children);
            // A FormID the target already has would be a duplicate (or a second copy under
            // other children).
            if existing
                .as_ref()
                .is_some_and(|(_, index)| index.records.iter().any(|r| r.form_id == form_id))
            {
                return Err(refused(format!(
                    "place.formid-exists: {}: {form_id:08X} is already in {target_name}",
                    new.label
                )));
            }
            for l in &composed.limitations {
                push_once(&mut limitations, format!("{}: {l}", new.label));
            }
            if !seen.insert((form_id, new.signature.0)) {
                return Err(refused(format!(
                    "{}: FormID {form_id:08X} allocated twice",
                    new.label
                )));
            }
            reports.push(RecordReport {
                signature: new.signature.to_string(),
                form_id,
                formref: format!("{target_name}:{:08X}", form_id & 0x00FF_FFFF),
                editor_id: new.editor_id.clone(),
                source: format!("new ({})", new.label),
                outcome: "inserted",
                parent: false,
                changes: Vec::new(),
                inlined: Vec::new(),
                offset: None,
                length: None,
                edited_subrecords: Vec::new(),
            });
            new_forms.push(NewFormReport {
                label: new.label.clone(),
                form_id,
                base,
                cell,
                world,
                path: at
                    .path
                    .iter()
                    .map(|g| (g.group_type, g.label_u32()))
                    .collect(),
                members: composed.members,
            });
            placed.push(at);
        }
        for l in allocator.limitations() {
            push_once(&mut limitations, l);
        }
        forms = Some(allocator);
    }

    // Write.
    // Write to a temporary file next to the target, find every written record in it (the
    // evidence), and only then rename it over the target: a failure leaves the target as
    // it was.
    let created = existing.is_none();
    let extended = masters != original_masters;
    // Overrides of a master's records in topic, temporary cell or quest children need
    // `ONAM` (lane A's M4.A6 corpus rule).
    let master_count = masters.len();
    let overrides_children = placed.iter().any(|p| {
        p.path
            .iter()
            .any(|g| kitkraft_esm::cells::ONAM_GROUP_TYPES.contains(&g.group_type))
            && usize::try_from(p.record.header.form_id >> 24).is_ok_and(|i| i < master_count)
    });
    let temp = temp_path(target);
    let discard = |e: PatchError| {
        let _ = std::fs::remove_file(&temp);
        e
    };
    let (outcomes, flags): (Vec<Upsert>, u32) = match &existing {
        None => {
            let spec = created_header(&masters);
            // INCC after the masters, as in every official file.
            let interior = placed.iter().filter(|p| is_interior_cell(p)).count();
            let mut header = spec.to_record();
            if let Some(subs) = header.subrecords_mut() {
                let count = u32::try_from(interior).unwrap_or(u32::MAX);
                subs.push(Subrecord::new(INCC, count.to_le_bytes().to_vec()));
            }
            if let Some(f) = &forms {
                f.write_hedr(&mut header)
                    .map_err(|e| PatchError::Io(format!("{target_name}: HEDR: {e}")))?;
            }
            let mut plugin = Plugin::new(header);
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
            std::fs::write(&temp, &bytes)
                .map_err(|e| discard(PatchError::Io(format!("writing {}: {e}", temp.display()))))?;
            (outcomes, spec.flags)
        }
        Some((plugin, index)) => {
            let names: Vec<Vec<u8>> = masters.iter().map(|m| m.as_bytes().to_vec()).collect();
            let added_cells = placed
                .iter()
                .filter(|p| {
                    is_interior_cell(p)
                        && !index.records.iter().any(|r| {
                            r.form_id == p.record.header.form_id
                                && r.signature == p.record.header.signature
                        })
                })
                .count();
            let has_incc = index
                .header
                .subrecords()
                .is_some_and(|subs| subs.iter().any(|s| s.signature() == INCC));
            if added_cells > 0 && !has_incc {
                limitations.push(format!(
                    "{target_name} has no INCC (interior cell count); {added_cells} interior cell(s) were added without one"
                ));
            }
            let report =
                upsert_file_to_path(target, &temp, &RewriteOptions::default(), placed, |tes4| {
                    if extended {
                        set_masters(tes4, &names);
                    }
                    if added_cells > 0 {
                        add_to_incc(tes4, added_cells);
                    }
                    if let Some(f) = &forms {
                        f.write_hedr(tes4)?;
                    }
                    Ok(())
                })
                .map_err(|e| discard(PatchError::Io(format!("updating {target_name}: {e}"))))?;
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
            Upsert::Moved => "moved",
        };
    }
    if overrides_children {
        let listed = apply_onam(&temp).map_err(discard)?;
        push_once(&mut limitations, onam_note(listed));
    }
    let bytes = finish(&temp, target, &mut reports, &options.limits)?;
    limitations.push(CK_DIFF_PENDING.to_string());
    Ok(PatchReport {
        target: target.to_path_buf(),
        created,
        masters,
        flags,
        records: reports,
        bytes,
        limitations,
        new_forms,
        next_object_id: forms.map(|f| f.hedr_next_object_id()),
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

/// Refuses targets the module docs rule out: not `.esm`/`.esp`, a protected (load-order)
/// plugin, or inside the game's Data directory, the user Data directory or an MO2
/// instance or profile, at any depth. `build` checks its `out` with it too.
pub fn guard_target(target: &Path, protected: &[PathBuf]) -> Result<(), PatchError> {
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
    guard_location(target)
}

/// Refuses any output file (a plugin, or a spec `overhaul.scaffold` writes) inside the
/// game's Data directory, the user Data directory, or an MO2 instance or profile, at any
/// depth.
pub fn guard_location(target: &Path) -> Result<(), PatchError> {
    if let Some(link) = first_link(target) {
        return Err(refused(format!(
            "{} passes through {}, a symbolic link or junction; outputs are written to real paths only, so no write can be redirected into the game, the user Data folder or MO2",
            target.display(),
            link.display()
        )));
    }
    let abs_target = absolute(target);
    let mut dir = abs_target.parent();
    while let Some(d) = dir {
        if d.join("Starfield.esm").is_file() {
            return Err(refused(format!(
                "{} is inside the game's Data directory ({}); write the patch elsewhere and install it through MO2",
                target.display(),
                d.display()
            )));
        }
        if is_user_data_dir(d) {
            return Err(refused(format!(
                "{} is inside the game's user Data directory ({}), which the game also loads from; write the patch elsewhere",
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

/// The per-user Data folder the game also loads plugins from: a directory named `Data`
/// whose parent is `My Games\Starfield`, or whose parent holds `StarfieldCustom.ini` or
/// `StarfieldPrefs.ini` (the same test lane G's `ba2.extract` applies).
pub(crate) fn is_user_data_dir(dir: &Path) -> bool {
    let named = |p: &Path, want: &str| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(want))
    };
    if !named(dir, "Data") {
        return false;
    }
    let Some(parent) = dir.parent() else {
        return false;
    };
    (named(parent, "Starfield") && parent.parent().is_some_and(|g| named(g, "My Games")))
        || ["StarfieldCustom.ini", "StarfieldPrefs.ini"]
            .iter()
            .any(|ini| parent.join(ini).is_file())
}

/// `path` made absolute and canonical as far as it exists: the deepest existing ancestor
/// is canonicalized (links and junctions resolved) and the components that do not exist
/// yet are appended. A path through a junction followed by folders still to be created
/// therefore resolves to where a write would land (Fable review, deploy round, finding 2:
/// canonicalizing only the parent let `out\<junction>\new\...` keep its unresolved
/// spelling). Shared by `set`, `build` and `package`.
pub(crate) fn absolute(path: &Path) -> PathBuf {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    let mut cur: &Path = &abs;
    loop {
        if let Ok(c) = std::fs::canonicalize(cur) {
            return rest.iter().rev().fold(c, |p, name| p.join(name));
        }
        match (cur.parent(), cur.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                cur = parent;
            }
            _ => return abs.clone(),
        }
    }
}

/// The first component of `path` (made absolute, links not resolved), from the path
/// itself up to its root, that is a symbolic link or junction (a name-surrogate reparse
/// point; Rust's `is_symlink` covers both on Windows).
pub(crate) fn first_link(path: &Path) -> Option<PathBuf> {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    abs.ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .find(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
        .map(Path::to_path_buf)
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
    let subs = record.subrecords().unwrap_or_default();
    let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
    for (sub, layout) in subs.iter().zip(fields::layouts(sig, &sigs)) {
        let text = sub.signature().to_string();
        let opaque = match layout {
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

/// Whether a layout contains bytes that are not decoded (opaque, or a union).
pub fn has_opaque(ty: &FieldType) -> bool {
    match ty {
        FieldType::Opaque { .. } | FieldType::Union { .. } => true,
        FieldType::Struct { fields } => fields.iter().any(|f| has_opaque(&f.ty)),
        FieldType::Array { element, .. } => has_opaque(element),
        FieldType::Ref { ty, .. } => has_opaque(ty),
        _ => false,
    }
}

/// The lstring spans of one subrecord laid out as `layout` (its layout at its position,
/// `fields::layout_at`): `(member label, start, len)`.
pub fn lstring_spans(
    layout: Option<&fields::Layout>,
    sub_sig: [u8; 4],
    payload: &[u8],
    form_version: u16,
) -> Result<Vec<(String, usize, usize)>, FieldError> {
    let text = String::from_utf8_lossy(&sub_sig).into_owned();
    let Some(layout) = layout else {
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
/// `written` holds subrecord indices an edit already wrote (whole, or its lstring tail).
///
/// An lstring is what the schema types as one, `FULL`/`DESC` by the fallback, or what the
/// corpus table (`super::lstrings`) lists. A subrecord none of them describes (no layout,
/// or opaque bytes) whose payload is a non-zero 4-byte value the table does not class as
/// "not an lstring" may be a string ID: the write is refused, naming each, unless
/// `allow_untyped_lstrings` (then they are copied as stored and named in `limitations`).
fn inline_lstrings(
    record: &mut Record<'static>,
    source: &SourcePlugin,
    written: &BTreeSet<usize>,
    options: &PatchOptions<'_>,
    limitations: &mut Vec<String>,
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
    let mut untyped: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut updates: Vec<(usize, Vec<u8>)> = Vec::new();
    let sigs: Vec<[u8; 4]> = subs.iter().map(|(s, _)| s.0).collect();
    let all = fields::layouts(sig, &sigs);
    for (i, (s, payload)) in subs.iter().enumerate() {
        let n = nth.entry(s.0).or_insert(0);
        let occurrence = *n;
        *n = n.saturating_add(1);
        if written.contains(&i) {
            continue;
        }
        let layout = all.get(i).cloned().flatten();
        if layout.as_ref().is_none_or(|l| has_opaque(&l.ty)) {
            let maybe_id = payload.len() == 4 && payload.iter().any(|b| *b != 0);
            if maybe_id && lstrings::class(sig, s.0) == lstrings::Class::Unclassified {
                untyped.push(format!("{s}[{occurrence}]"));
            }
            continue;
        }
        let spans = lstring_spans(layout.as_ref(), s.0, payload, form_version)?;
        if !spans.is_empty() {
            if let Some(note) = layout.and_then(|l| l.fallback) {
                notes.push(note);
            }
        }
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
            notes.extend(super::high_bytes_note(&at, &text));
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
    if !untyped.is_empty() {
        let list = untyped.join(", ");
        if !options.allow_untyped_lstrings {
            return Err(refused(format!(
                "{what} from localized {} has 4-byte subrecords of unknown layout that may be string IDs: {list}. Neither the schema nor the corpus lstring table (docs/validate.md) says what they are, and a string ID copied into a non-localized patch shows as garbage text; pass allow_untyped_lstrings (--allow-untyped-lstrings) to copy them as stored",
                source.name
            )));
        }
        notes.push(format!(
            "{what} from localized {}: {list} copied as stored under allow_untyped_lstrings (4-byte payloads of unknown layout; if any is a string ID the game shows its bytes as text)",
            source.name
        ));
    }
    for note in notes {
        push_once(limitations, note);
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
pub(crate) fn locate_written(
    target: &Path,
    reports: &mut [RecordReport],
    limits: &DecodeLimits,
) -> Result<u64, PatchError> {
    let io = |e: kitkraft_esm::Error| PatchError::Io(format!("{}: {e}", target.display()));
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
    target.with_file_name(format!(".{name}.kitkraft-{}.tmp", std::process::id()))
}

/// Whether an edit leaves its subrecord for the lstring inliner to skip: it wrote the whole
/// subrecord, or an lstring member (a struct's tail, the only lstring a struct holds here;
/// Fable review round 2, finding 7: inlining it again would read the new text as an ID).
pub(crate) fn marks_written(edit: &Edit, change: &FieldChange) -> bool {
    edit.path.steps.is_empty() || change.lstring
}

/// Finds every written record in the staged file `temp` (the evidence), then renames it
/// over `target`. On any failure `temp` is removed and `target` is left as it was (Fable
/// review round 2, finding 12: the evidence pass used to run after the rename).
pub(crate) fn finish(
    temp: &Path,
    target: &Path,
    reports: &mut [RecordReport],
    limits: &DecodeLimits,
) -> Result<u64, PatchError> {
    let done = locate_written(temp, reports, limits).and_then(|bytes| {
        std::fs::rename(temp, target)
            .map(|()| bytes)
            .map_err(|e| PatchError::Io(format!("replacing {}: {e}", target.display())))
    });
    if done.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    done
}

/// The limitation an `ONAM` write carries.
pub(crate) fn onam_note(listed: usize) -> String {
    format!(
        "TES4 ONAM lists the {listed} override(s) of master records in topic, temporary cell and quest children (lane A's corpus rule, M4.A6), in file order; the Creation Kit's own order is fixture F23's question, and whether each master has the record is not checked"
    )
}

/// Rewrites the `TES4` `ONAM` of the plugin at `path` by lane A's corpus rule (M4.A6,
/// `kitkraft_esm::cells::onam_forms`): every record in a group of type 7, 9 or 10 whose
/// FormID names one of the plugin's masters, in file order. Returns how many are listed.
/// Reads the whole plugin into memory (patches are small).
pub(crate) fn apply_onam(path: &Path) -> Result<usize, PatchError> {
    let io = |e: &dyn std::fmt::Display| PatchError::Io(format!("{}: {e}", path.display()));
    let bytes = std::fs::read(path).map_err(|e| io(&e))?;
    let mut plugin = Plugin::parse(&bytes).map_err(|e| io(&e))?;
    let masters = plugin.tes4().masters().len();
    let forms = kitkraft_esm::cells::onam_forms(&plugin, |id| {
        usize::try_from(id >> 24).is_ok_and(|i| i < masters)
    });
    if forms != plugin.tes4().overridden_forms() {
        Tes4::set_overridden_forms(plugin.header_mut(), &forms);
        let out = plugin.to_bytes().map_err(|e| io(&e))?;
        std::fs::write(path, out).map_err(|e| io(&e))?;
    }
    Ok(forms.len())
}

/// Adds `added` to the `INCC` count of a `TES4` record, if it has one.
fn add_to_incc(tes4: &mut Record<'_>, added: usize) {
    let Some(subs) = tes4.subrecords_mut() else {
        return;
    };
    for s in subs.iter_mut().filter(|s| s.signature() == INCC) {
        if let Ok(old) = <[u8; 4]>::try_from(s.data()) {
            let count =
                u32::from_le_bytes(old).saturating_add(u32::try_from(added).unwrap_or(u32::MAX));
            s.set_data(count.to_le_bytes().to_vec());
        }
    }
}

/// Appends `note` unless it is already there.
fn push_once(limitations: &mut Vec<String>, note: String) {
    if !limitations.contains(&note) {
        limitations.push(note);
    }
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
    let header = kitkraft_esm::index::read_record_at(&mut file, 0, None)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let tes4 = Tes4::new(&header);
    Ok((
        tes4.masters().iter().map(|m| m.name_lossy()).collect(),
        tes4.flags(),
    ))
}
