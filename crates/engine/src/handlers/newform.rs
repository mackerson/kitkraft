//! `new` and `clone` (M4.E1): new base forms in a patch plugin.
//!
//! - `new {type, edid, fields?, masters?, into}`: a record of a type the schema defines and
//!   that lives in its top-level group, with its `EDID`, the members the schema marks
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
use kitkraft_schema::{Cardinality, FieldType, Member};
use serde_json::{json, Value};

use super::load::{acquire, Loaded};
use super::place::{check_target_edids, edid_owner, valid_edid, PlaceRequest};
use super::set::{
    check, describe, edits, extend_once, find_form, guard, install, outcome, safe_source, stage,
    validate_json, winning_copy, write_with, Origin,
};
use super::{failed, str_arg};
use crate::edit::fields;
use crate::edit::patch::{Copy, Edit};
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
/// children) or that `new` does not create (cells and worlds: their groups are built by
/// `place`'s carried copies only).
fn nested_or_world(sig: [u8; 4]) -> bool {
    let s = Signature(sig);
    CELL_CHILD_TYPES.contains(&s)
        || QUEST_CHILD_TYPES.contains(&s)
        || matches!(&sig, b"INFO" | b"CELL" | b"WRLD" | b"TES4" | b"LAND")
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

/// One subrecord slot of a record type, in schema member order (groups flattened).
#[derive(Debug, Clone)]
struct Slot {
    sig: [u8; 4],
    /// Required at the record's level (a group's members never are on their own).
    card: Cardinality,
    ty: FieldType,
    /// Repeated: the slot takes any number of consecutive occurrences.
    repeated: bool,
    /// Inside a component layout of the base form component block
    /// (`kitkraft_schema::in_component_layout`).
    component: bool,
}

/// Every subrecord slot of `sig` in schema order (groups flattened).
fn member_order(sig: [u8; 4]) -> Vec<Slot> {
    fn walk(members: &[Member], prefix: &str, repeated: bool, out: &mut Vec<Slot>) {
        for m in members {
            let path = if prefix.is_empty() {
                m.name().to_string()
            } else {
                format!("{prefix}.{}", m.name())
            };
            match m {
                Member::Subrecord(s) => out.push(Slot {
                    sig: s.signature.as_bytes(),
                    card: s.cardinality,
                    ty: s.ty.clone(),
                    repeated: repeated || s.cardinality == Cardinality::Repeated,
                    component: kitkraft_schema::in_component_layout(&path),
                }),
                Member::Group(g) => {
                    // A group's members are required only inside the group, which may
                    // itself be absent: never added on their own.
                    let start = out.len();
                    let again = repeated || g.cardinality == Cardinality::Repeated;
                    walk(&g.members, &path, again, out);
                    for e in out.iter_mut().skip(start) {
                        e.card = Cardinality::Optional;
                    }
                }
                Member::AnyUntil(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    if let Some(def) = kitkraft_schema::builtin()
        .ok()
        .and_then(|s| s.record(kitkraft_schema::Signature::from_bytes(sig)?))
    {
        walk(&def.members, "", false, &mut out);
    }
    out
}

/// Where occurrence `k` of signature `s` goes when a new record has `n` of them: its slot's
/// position in [`member_order`]. One occurrence takes the record's own member (before a
/// member of the same signature inside a component layout: PKIN `VNAM` is `Version`);
/// several take the slots in schema order, `k`-th to `k`-th, the last slot taking the rest
/// when it repeats. `None` when there is no slot for it.
fn slot_of(order: &[Slot], s: [u8; 4], k: usize, n: usize) -> Option<usize> {
    let slots: Vec<usize> = order
        .iter()
        .enumerate()
        .filter(|(_, e)| e.sig == s)
        .map(|(i, _)| i)
        .collect();
    if n <= 1 {
        return order
            .iter()
            .position(|e| e.sig == s && !e.component)
            .or_else(|| slots.first().copied());
    }
    match slots.get(k) {
        Some(i) => Some(*i),
        None => slots
            .last()
            .copied()
            .filter(|i| order.get(*i).is_some_and(|e| e.repeated)),
    }
}

/// The bytes a created subrecord in a slot of layout `ty` starts with: zeros of its fixed
/// size, or an empty string.
fn slot_placeholder(ty: &FieldType, whole_value: bool) -> Option<Vec<u8>> {
    if let Some(n) = ty.fixed_size(Some(RecordStamps::default().form_version)) {
        return Some(vec![0; usize::try_from(n).ok()?]);
    }
    match ty {
        FieldType::ZString | FieldType::LString => Some(vec![0]),
        _ if whole_value => Some(Vec::new()),
        _ => None,
    }
}

/// The bytes a created subrecord starts with: zeros of its fixed size, or an empty string.
fn placeholder(record: [u8; 4], sub: [u8; 4], whole_value: bool) -> Option<Vec<u8>> {
    let layout = fields::layout_at(record, &[*b"EDID", sub], 1)?;
    if let Some(n) = layout
        .ty
        .fixed_size(Some(RecordStamps::default().form_version))
    {
        return Some(vec![0; usize::try_from(n).ok()?]);
    }
    match layout.ty {
        FieldType::ZString | FieldType::LString => Some(vec![0]),
        _ if whole_value => Some(Vec::new()),
        _ => None,
    }
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
    if nested_or_world(sig) {
        return Err(failed(command, format!("new.not-top-level: {at}: a {text} lives in a world, cell or quest's children (use `place` for references); `new` writes records of their own top-level group")));
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
    let order = member_order(sig);
    // How many occurrences of each signature the fields name (`PRPS[1]` makes two).
    let mut wanted: std::collections::BTreeMap<[u8; 4], usize> = std::collections::BTreeMap::new();
    for edit in &req.edits {
        let n = wanted.entry(edit.path.signature).or_insert(0);
        *n = (*n).max(edit.path.occurrence.saturating_add(1));
    }
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
    let subs: Vec<Subrecord<'static>> = subs.into_iter().map(|(_, s)| s).collect();
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

fn fields_arg(command: &str, args: &Value) -> Result<Vec<Edit>, EngineError> {
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
    let (copy, origin, _, unrelated) = plan_clone(ID, loaded, &req, &limits)?;
    target_edid(ID, &into, ID, &req.edid)?;
    let mut limitations = loaded.limitations.clone();
    extend_once(&mut limitations, unrelated);
    commit(
        ID,
        loaded,
        &into,
        vec![copy],
        allow_untyped,
        limitations,
        &[origin],
        dry_run,
    )
}
