//! `new` and `clone` (M4.E1): new base forms in a patch plugin.
//!
//! - `new {type, edid, fields?, masters?, into}`: a record of a type the schema defines and
//!   that lives in its top-level group, or a new interior `CELL` (lane D round 4: `DATA`
//!   flags `Interior` unless given, placed by the writer in the interior block and sub-block
//!   its FormID gives, no references), with its `EDID`, the members the schema marks
//!   required written as zero bytes (an empty string for a string), and `fields` applied
//!   (each field's subrecord is created in schema member order first, zero bytes or an
//!   empty string, then written as `set` writes it). `masters` names the plugins its
//!   FormID fields point into.
//! - `clone {form, edid, fields?, into}`: a copy of the winning version of `form` (a
//!   top-level record; references are placed with `place`) with a new FormID and
//!   EditorID, `fields` applied; FormID members are copied as stored (and checked by
//!   `formid.dangling`), lstrings of a localized source inlined, and a source whose
//!   masters differ from the target's is remapped through the typed visitors or refused
//!   when a subrecord is opaque (the patch writer's rule).
//!
//! Both take their FormID from lane A's allocator (`kitkraft_esm::forms::NewForms`) once
//! the target's master list is final: full-scale targets with full-scale masters only. An
//! EditorID clash (load order, target) is refused. Written as `set` writes: staged,
//! validated, then moved. Refusals are `failed` with `new.<code>:` / `clone.<code>:`.

use std::path::{Path, PathBuf};

use kitkraft_doc::GlobalForm;
use kitkraft_esm::bridge::has_definition;
use kitkraft_esm::cells::{CELL_CHILD_TYPES, QUEST_CHILD_TYPES};
use kitkraft_esm::refs::RecordStamps;
use kitkraft_esm::{Body, DecodeLimits, GroupHeader, Record, RecordHeader, Signature, Subrecord};
use kitkraft_schema::Cardinality;
use serde_json::{json, Value};

use super::load::{acquire, Loaded};
use super::place::{check_target_edids, edid_owner, valid_edid, PlaceRequest};
use super::set::{
    check, describe, edits, extend_once, find_form, guard, install, outcome, safe_source, stage,
    validate_json, winning_copy, write_with, Origin,
};
use super::{failed, str_arg};
use crate::edit::patch::{Copy, Edit};
use crate::edit::scaffold::{
    fill_groups, first_unplaced, lead_refusal, member_order, member_order_of_layout, missing_lead,
    placeholder, slot_of, slot_placeholder, wanted_slots,
};
use crate::edit::source::SourcePlugin;
use crate::envelope::Outcome;
use crate::exec::EngineError;
use crate::session::Session;
use crate::validate::{self, Severity};

/// A `new` request.
#[derive(Debug, Clone)]
pub(crate) struct NewRequest {
    pub label: String,
    pub signature: [u8; 4],
    pub edid: String,
    pub edits: Vec<Edit>,
    pub masters: Vec<String>,
}

/// A `clone` request.
#[derive(Debug, Clone)]
pub(crate) struct CloneRequest {
    pub label: String,
    pub form: String,
    pub plugin: Option<String>,
    pub edid: String,
    pub edits: Vec<Edit>,
}

/// Record types that never live in their own top-level group (cell, world and quest
/// children) or that `clone` does not copy (cells and worlds: their groups are built by
/// `place`'s carried copies).
fn nested_or_world(sig: [u8; 4]) -> bool {
    let s = Signature(sig);
    CELL_CHILD_TYPES.contains(&s)
        || QUEST_CHILD_TYPES.contains(&s)
        || matches!(&sig, b"INFO" | b"CELL" | b"WRLD" | b"TES4" | b"LAND")
}

/// The `DATA` flag of an interior cell (`CELL.toml`, `Flags.Interior`).
pub(crate) const CELL_INTERIOR: u32 = 0x1;

/// Record types `new` refuses: [`nested_or_world`] less `CELL`. A new `CELL` is an
/// interior cell (lane D round 4): the writer puts it in the interior block and sub-block
/// its FormID computes (`kitkraft_esm::cells::interior_block`), with no children; a new
/// exterior cell (a world and a grid) is not written.
fn new_refused(sig: [u8; 4]) -> bool {
    &sig != b"CELL" && nested_or_world(sig)
}

fn check_edid(
    command: &str,
    code_prefix: &str,
    loaded: &Loaded,
    label: &str,
    edid: &str,
) -> Result<(), EngineError> {
    if !valid_edid(edid) {
        return Err(failed(
            command,
            format!("{code_prefix}.edid-invalid: {label}: EditorID `{edid}` must be 1 to 200 ASCII letters, digits or `_`"),
        ));
    }
    if let Some(owner) = edid_owner(loaded, edid) {
        return Err(failed(
            command,
            format!(
                "{code_prefix}.edid-clash: {label}: EditorID `{edid}` is already used by {owner}"
            ),
        ));
    }
    Ok(())
}

/// The patch-writer copy for a `new` request.
pub(crate) fn plan_new(
    command: &str,
    loaded: &Loaded,
    req: &NewRequest,
) -> Result<(Copy, Vec<String>), EngineError> {
    let at = &req.label;
    let sig = req.signature;
    let text = String::from_utf8_lossy(&sig).into_owned();
    if !has_definition(Signature(sig)) {
        return Err(failed(command, format!("new.untyped: {at}: {text} has no record definition in the schema; a new form is written only for a typed type")));
    }
    if new_refused(sig) {
        return Err(failed(command, format!("new.not-top-level: {at}: a {text} lives in a world, cell or quest's children (use `place` for references); `new` writes records of their own top-level group, and interior cells")));
    }
    check_edid(command, "new", loaded, at, &req.edid)?;
    let mut masters: Vec<(u16, String)> = Vec::new();
    for m in &req.masters {
        let i = loaded.lo.find(m).ok_or_else(|| {
            failed(
                command,
                format!("new.no-master: {at}: `{m}` is not in the load order"),
            )
        })?;
        let name = loaded
            .lo
            .plugin(i)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        if !masters.iter().any(|(_, n)| n.eq_ignore_ascii_case(&name)) {
            masters.push((i, name));
        }
    }
    masters.sort_by_key(|(i, _)| *i);
    let all = member_order(sig);
    // How many occurrences of each signature the fields name (`PRPS[1]` makes two), and the
    // count subrecord of a whole counted array (`KWDA` brings `KSIZ`; lane D round 5).
    let wanted = wanted_slots(&all, &req.edits, &std::collections::BTreeMap::new());
    // The component layout a new record holds: the wanted signatures the record cannot
    // place wholly at its own level sit in the block's layout that carries them, and the
    // layout is the intersection of those. A wanted signature with a slot of its own in
    // the record places at most all of its occurrences there (its k-th occurrence may map
    // into a slot of another layout in the full order, so it constrains nothing); a
    // signature with no block member names no layout. The block's other layouts are not
    // slots of this record.
    let mut layout: Option<Vec<String>> = None;
    for (s, n) in &wanted {
        let own = all.iter().filter(|e| e.sig == *s && !e.component).count();
        let mut in_layouts: Vec<String> = Vec::new();
        for e in all.iter().filter(|e| e.sig == *s && e.component) {
            if let Some(l) = e.path.split('.').nth(1) {
                if !in_layouts.iter().any(|x| x == l) {
                    in_layouts.push(l.to_string());
                }
            }
        }
        if in_layouts.is_empty() || *n <= own {
            continue;
        }
        layout = match layout {
            None => Some(in_layouts),
            Some(prev) => Some(
                prev.into_iter()
                    .filter(|l| in_layouts.iter().any(|x| x == l))
                    .collect(),
            ),
        };
    }
    let order = match layout.and_then(|l| l.into_iter().next()) {
        Some(layout) => member_order_of_layout(sig, &layout),
        None => all,
    };
    let mut edid = req.edid.as_bytes().to_vec();
    edid.push(0);
    // Each subrecord with the position of its slot; EDID first.
    let edid_slot = slot_of(&order, *b"EDID", 0, 1).unwrap_or(0);
    let mut subs: Vec<(usize, Subrecord<'static>)> =
        vec![(edid_slot, Subrecord::new(Signature(*b"EDID"), edid))];
    let mut limitations = Vec::new();
    let mut defaulted: Vec<String> = Vec::new();
    for (i, slot) in order.iter().enumerate() {
        let m = slot.sig;
        if slot.card == Cardinality::Required
            && m != *b"EDID"
            && !subs.iter().any(|(_, s)| s.signature().0 == m)
            && !wanted.contains_key(&m)
        {
            match placeholder(sig, m, false) {
                Some(bytes) => {
                    defaulted.push(String::from_utf8_lossy(&m).into_owned());
                    subs.push((i, Subrecord::new(Signature(m), bytes)));
                }
                None => limitations.push(format!(
                    "{at}: the schema requires {} on a {text}, which has no fixed size; give it in `fields` as hex:",
                    String::from_utf8_lossy(&m)
                )),
            }
        }
    }
    // A new cell is interior: its DATA flags start with Interior unless `fields` give DATA
    // (the writer refuses one that clears it).
    if &sig == b"CELL" && !wanted.contains_key(b"DATA") {
        if let Some(slot) = slot_of(&order, *b"DATA", 0, 1) {
            subs.push((
                slot,
                Subrecord::new(Signature(*b"DATA"), CELL_INTERIOR.to_le_bytes().to_vec()),
            ));
        }
    }
    if &sig == b"CELL" {
        limitations.push(format!(
            "{at}: a new CELL is an interior cell (DATA flag Interior), written in the interior block and sub-block its FormID gives, with no references; what the Creation Kit writes for a new interior cell (its default members and stamps) is not observed"
        ));
    }
    for edit in &req.edits {
        if edit.path.signature == *b"EDID" {
            return Err(failed(
                command,
                format!("new.edid-field: {at}: give the EditorID as `edid`, not in fields"),
            ));
        }
    }
    for (s, n) in &wanted {
        for k in 0..*n {
            // A whole value for this occurrence (no member steps): a variable layout may
            // start empty.
            let whole = req.edits.iter().any(|e| {
                e.path.signature == *s && e.path.occurrence == k && e.path.steps.is_empty()
            });
            let shown = req
                .edits
                .iter()
                .find(|e| e.path.signature == *s && e.path.occurrence == k)
                .map_or_else(
                    || format!("{}[{k}]", String::from_utf8_lossy(s)),
                    |e| e.path.text.clone(),
                );
            // A signature the definition has no slot for (the FULL/DESC and corpus lstring
            // fallbacks) goes last, one occurrence only.
            let slot = match slot_of(&order, *s, k, *n) {
                Some(slot) => slot,
                None if *n <= 1 => usize::MAX,
                None => {
                    return Err(failed(
                        command,
                        format!(
                            "new.occurrence: {at}: `{shown}`: {text} has no slot for occurrence {k} of {} (the schema lists {}, none repeating)",
                            String::from_utf8_lossy(s),
                            order.iter().filter(|e| e.sig == *s).count()
                        ),
                    ))
                }
            };
            // A group member needs its group's lead in the fields too (Fable review of
            // wave 3, finding 1).
            let gets = |m: [u8; 4]| {
                wanted.get(&m).is_some_and(|n| *n > 0)
                    || subs.iter().any(|(_, s)| s.signature().0 == m)
            };
            if let Some((group, leads)) = missing_lead(&order, slot, gets) {
                return Err(failed(
                    command,
                    format!(
                        "new.no-group-lead: {at}: `{shown}`: {}",
                        lead_refusal(&shown, *s, &group, &leads)
                    ),
                ));
            }
            // One occurrence: the record's own member's layout, as `set` resolves it
            // after `EDID`; several: each slot's own layout.
            // A member that cannot follow EDID alone (the BFCE that ends a component)
            // starts from its slot's layout too.
            let from_slot = || order.get(slot).and_then(|e| slot_placeholder(&e.ty, whole));
            let bytes = if *n <= 1 {
                placeholder(sig, *s, whole).or_else(from_slot)
            } else {
                from_slot()
            };
            let bytes = bytes.ok_or_else(|| {
                failed(
                    command,
                    format!(
                        "new.unknown-member: {at}: `{shown}` is not a member of {text} with a layout a new subrecord can start from (give the whole subrecord as hex:)"
                    ),
                )
            })?;
            subs.push((slot, Subrecord::new(Signature(*s), bytes)));
        }
    }
    // Stable: occurrences of one repeating slot keep their order.
    subs.sort_by_key(|(slot, _)| *slot);
    // A group the fields start gets the members its definition requires in it, as the
    // record's own required members are (Fable review of the wave 3 fix round,
    // follow-up (b)).
    let mut all: Vec<(usize, bool, Subrecord<'static>)> =
        subs.into_iter().map(|(slot, s)| (slot, true, s)).collect();
    match fill_groups(sig, &order, &mut all) {
        Ok(added) => defaulted.extend(added),
        Err((_, why)) => {
            return Err(failed(
                command,
                format!("new.group-incomplete: {at}: {why}"),
            ))
        }
    }
    let subs: Vec<Subrecord<'static>> = all.into_iter().map(|(_, _, s)| s).collect();
    let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
    if let Some((i, bad)) = first_unplaced(sig, &sigs, |_| true) {
        return Err(failed(
            command,
            format!(
                "new.unplaced: {at}: {} (subrecord {i}) would sit where the {text} definition does not place it",
                String::from_utf8_lossy(&bad)
            ),
        ));
    }
    if !defaulted.is_empty() {
        limitations.push(format!(
            "{at}: members the schema requires were written as zeros (or an empty string) unless given in fields: {}",
            defaulted.join(", ")
        ));
    }
    let stamps = RecordStamps::default();
    let record = Record::new(
        RecordHeader {
            signature: Signature(sig),
            flags: 0,
            form_id: 0,
            timestamp: stamps.timestamp,
            version_control: stamps.version_control,
            form_version: stamps.form_version,
            unknown: stamps.unknown,
        },
        Body::Raw(subs),
    );
    limitations.push(format!(
        "{at}: record header form version {} and timestamp, version control and unknown 0: 0 is rare or unobserved in the corpus for these stamps and their meaning is unknown, pending the Creation Kit's observation (F06, F20)",
        stamps.form_version
    ));
    Ok((
        Copy {
            source: SourcePlugin {
                name: String::new(),
                path: PathBuf::new(),
                masters: masters.into_iter().map(|(_, n)| n).collect(),
                flags: 0,
            },
            record,
            path: vec![GroupHeader::top(Signature(sig))],
            parents: Vec::new(),
            edits: req.edits.clone(),
            formref: format!("new {text}"),
            carried: false,
            new_form: Some(req.edid.clone()),
            scripts: Vec::new(),
        },
        limitations,
    ))
}

/// The patch-writer copy for a `clone` request: the base form cloned (its winner) and its
/// signature.
pub(crate) fn plan_clone(
    command: &str,
    loaded: &Loaded,
    req: &CloneRequest,
    limits: &DecodeLimits,
) -> Result<(Copy, Origin, GlobalForm, Vec<String>), EngineError> {
    let at = &req.label;
    let form = find_form(command, loaded, &req.form, req.plugin.as_deref())
        .map_err(|e| failed(command, format!("clone.no-form: {at}: {e}")))?;
    let unrelated = safe_source(command, loaded, form)?;
    check_edid(command, "clone", loaded, at, &req.edid)?;
    let (mut copy, origin) = winning_copy(command, loaded, form, req.edits.clone(), limits)?;
    let sig = copy.record.header.signature.0;
    if copy.path.len() != 1 || !copy.parents.is_empty() || nested_or_world(sig) {
        return Err(failed(
            command,
            format!(
                "clone.not-top-level: {at}: {} is a {} in nested groups; `clone` copies records of their own top-level group (place references with `place`)",
                loaded.lo.form_label(form),
                String::from_utf8_lossy(&sig)
            ),
        ));
    }
    copy.new_form = Some(req.edid.clone());
    Ok((copy, origin, form, unrelated))
}

pub(crate) fn fields_arg(command: &str, args: &Value) -> Result<Vec<Edit>, EngineError> {
    match args.get("fields").and_then(Value::as_object) {
        Some(f) if !f.is_empty() => edits(command, "/fields", f),
        _ => Ok(Vec::new()),
    }
}

/// Stage, write, validate, install: the tail `new` and `clone` share. With `dry_run`
/// (the schema for the preview), nothing is installed (M7.S6, `super::preview`).
#[allow(clippy::too_many_arguments)]
fn commit(
    command: &str,
    loaded: &mut Loaded,
    into: &Path,
    copies: Vec<Copy>,
    allow_untyped: bool,
    mut limitations: Vec<String>,
    origins: &[Origin],
    dry_run: Option<Option<&kitkraft_schema::Schema>>,
) -> Result<Outcome, EngineError> {
    let staged = stage(command, into, true)?;
    let mut report = write_with(
        command,
        loaded,
        &staged.path,
        copies,
        Vec::new(),
        allow_untyped,
    )?;
    let checked = check(command, loaded, &staged)?;
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
    let mut preview = None;
    if let Some(schema) = dry_run {
        // M7.S6: the same write and validation, reported without installing it.
        let mut notes = Vec::new();
        let after = super::preview::after(schema, loaded, &staged, &report, into, &mut notes);
        preview = Some((after, notes));
    } else {
        if !blocking.is_empty() {
            return Err(failed(
                command,
                format!(
                    "{command}.fails-validate: the written record fails validate, so {} was not changed: {}",
                    into.display(),
                    describe(&blocking)
                ),
            ));
        }
        install(command, &staged, into)?;
    }
    if !other.is_empty() {
        limitations.push(format!(
            "{} has {} validate error(s) on records this call did not write: {}",
            into.display(),
            other.len(),
            describe(&other)
        ));
    }
    report.target = into.to_path_buf();
    let mut result = outcome(&report, origins, limitations);
    let new_forms: Vec<Value> = report
        .records
        .iter()
        .filter(|r| r.source.starts_with("new ("))
        .map(|r| {
            json!({
                "form": r.formref, "form_id": format!("{:08X}", r.form_id),
                "signature": r.signature, "editor_id": r.editor_id, "from": r.source,
                "offset": r.offset, "length": r.length,
            })
        })
        .collect();
    if let Some(obj) = result.data.as_object_mut() {
        obj.insert("new_forms".into(), Value::Array(new_forms));
        obj.insert(
            "next_object_id".into(),
            json!(report.next_object_id.map(|n| format!("{n:08X}"))),
        );
        obj.insert("validate".into(), validate_json(&checked));
    }
    if let Some((after, notes)) = preview {
        super::preview::attach(
            &mut result,
            None,
            after,
            &report,
            &checked,
            &blocking,
            notes,
        );
    }
    Ok(result)
}

fn edid_arg(command: &str, args: &Value) -> Result<String, EngineError> {
    Ok(str_arg(command, args, "edid")?.to_string())
}

fn target_edid(command: &str, into: &Path, label: &str, edid: &str) -> Result<(), EngineError> {
    let mut probe = PlaceRequest::probe(label);
    probe.edid = Some(edid.to_string());
    check_target_edids(command, Some(into), &[&probe]).map_err(|e| {
        failed(
            command,
            e.to_string()
                .replace("place.edid-clash", &format!("{command}.edid-clash")),
        )
    })
}

/// `new`.
pub(super) fn run_new(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "new";
    let sig: [u8; 4] = str_arg(ID, args, "type")?
        .to_ascii_uppercase()
        .as_bytes()
        .try_into()
        .map_err(|_| {
            failed(
                ID,
                "new.bad-type: `type` must be a four-character record type",
            )
        })?;
    let into = PathBuf::from(str_arg(ID, args, "into")?);
    let req = NewRequest {
        label: ID.to_string(),
        signature: sig,
        edid: edid_arg(ID, args)?,
        edits: fields_arg(ID, args)?,
        masters: args
            .get("masters")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    };
    let dry_run = super::preview::requested(args).then(|| super::load::schema_of(session));
    let loaded = acquire(session, args, ID)?;
    guard(ID, loaded, &into)?;
    let (mut copy, notes) = plan_new(ID, loaded, &req)?;
    target_edid(ID, &into, ID, &req.edid)?;
    // Scripts to attach (M4.E9): refused here when a script or property is unknown.
    let scripts = super::scripts::from_args(ID, args, loaded, &into)?;
    copy.scripts = scripts.scripts;
    let mut limitations = loaded.limitations.clone();
    extend_once(&mut limitations, notes);
    extend_once(&mut limitations, scripts.limitations);
    let mut result = commit(
        ID,
        loaded,
        &into,
        vec![copy],
        false,
        limitations,
        &[],
        dry_run,
    )?;
    if let Some(obj) = result.data.as_object_mut() {
        if !scripts.reports.is_empty() {
            obj.insert("scripts".into(), Value::Array(scripts.reports));
        }
    }
    Ok(result)
}

/// `clone`.
pub(super) fn run_clone(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "clone";
    let into = PathBuf::from(str_arg(ID, args, "into")?);
    let allow_untyped = args
        .get("allow_untyped_lstrings")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let req = CloneRequest {
        label: ID.to_string(),
        form: str_arg(ID, args, "form")?.to_string(),
        plugin: args
            .get("plugin")
            .and_then(Value::as_str)
            .map(str::to_string),
        edid: edid_arg(ID, args)?,
        edits: fields_arg(ID, args)?,
    };
    let dry_run = super::preview::requested(args).then(|| super::load::schema_of(session));
    let loaded = acquire(session, args, ID)?;
    guard(ID, loaded, &into)?;
    let limits = DecodeLimits::default();
    let (mut copy, origin, _, unrelated) = plan_clone(ID, loaded, &req, &limits)?;
    // The material swap, as a whole-REFL edit of the LMSW's current REFL (KK-019).
    let swap_report = if let Some(swaps) = args.get("swap_overrides").filter(|s| !s.is_null()) {
        let (edit, report) = crate::edit::swaps::convert(ID, &copy.record, swaps, &copy.edits)?;
        copy.edits.push(edit);
        Some(report)
    } else {
        None
    };
    target_edid(ID, &into, ID, &req.edid)?;
    let mut limitations = loaded.limitations.clone();
    extend_once(&mut limitations, unrelated);
    let mut result = commit(
        ID,
        loaded,
        &into,
        vec![copy],
        allow_untyped,
        limitations,
        &[origin],
        dry_run,
    )?;
    if let Some(obj) = result.data.as_object_mut() {
        if let Some(report) = swap_report {
            obj.insert("swap_overrides".into(), Value::Array(report));
        }
    }
    Ok(result)
}
