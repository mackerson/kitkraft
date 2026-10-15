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

/// Top-level subrecord members of `sig` in schema order (groups flattened).
fn member_order(sig: [u8; 4]) -> Vec<([u8; 4], Cardinality, FieldType)> {
    fn walk(members: &[Member], out: &mut Vec<([u8; 4], Cardinality, FieldType)>) {
        for m in members {
            match m {
                Member::Subrecord(s) => {
                    out.push((s.signature.as_bytes(), s.cardinality, s.ty.clone()))
                }
                Member::Group(g) => {
                    // A group's members are required only inside the group, which may
                    // itself be absent: never added on their own.
                    let start = out.len();
                    walk(&g.members, out);
                    for e in out.iter_mut().skip(start) {
                        e.1 = Cardinality::Optional;
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
        walk(&def.members, &mut out);
    }
    out
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
    let rank = |s: [u8; 4]| {
        order
            .iter()
            .position(|(m, _, _)| *m == s)
            .unwrap_or(usize::MAX)
    };
    let mut edid = req.edid.as_bytes().to_vec();
    edid.push(0);
    let mut subs: Vec<Subrecord<'static>> = vec![Subrecord::new(Signature(*b"EDID"), edid)];
    let mut limitations = Vec::new();
    let mut defaulted: Vec<String> = Vec::new();
    for (m, card, _) in &order {
        if *card == Cardinality::Required
            && m != b"EDID"
            && !subs.iter().any(|s| s.signature().0 == *m)
            && !req.edits.iter().any(|e| e.path.signature == *m)
        {
            match placeholder(sig, *m, false) {
                Some(bytes) => {
                    defaulted.push(String::from_utf8_lossy(m).into_owned());
                    subs.push(Subrecord::new(Signature(*m), bytes));
                }
                None => limitations.push(format!(
                    "{at}: the schema requires {} on a {text}, which has no fixed size; give it in `fields` as hex:",
                    String::from_utf8_lossy(m)
                )),
            }
        }
    }
    for edit in &req.edits {
        let s = edit.path.signature;
        if edit.path.occurrence != 0 {
            return Err(failed(command, format!("new.occurrence: {at}: `{}`: a new record has one subrecord of each signature here", edit.path.text)));
        }
        if s == *b"EDID" {
            return Err(failed(
                command,
                format!("new.edid-field: {at}: give the EditorID as `edid`, not in fields"),
            ));
        }
        if subs.iter().any(|x| x.signature().0 == s) {
            continue;
        }
        let bytes = placeholder(sig, s, edit.path.steps.is_empty()).ok_or_else(|| {
            failed(
                command,
                format!(
                    "new.unknown-member: {at}: `{}` is not a member of {text} with a layout a new subrecord can start from (give the whole subrecord as hex:)",
                    edit.path.text
                ),
            )
        })?;
        subs.push(Subrecord::new(Signature(s), bytes));
    }
    subs.sort_by_key(|s| rank(s.signature().0));
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

/// Stage, write, validate, install: the tail `new` and `clone` share.
fn commit(
    command: &str,
    loaded: &mut Loaded,
    into: &Path,
    copies: Vec<Copy>,
    allow_untyped: bool,
    mut limitations: Vec<String>,
    origins: &[Origin],
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
    if !other.is_empty() {
        limitations.push(format!(
            "{} has {} validate error(s) on records this call did not write: {}",
            into.display(),
            other.len(),
            describe(&other)
        ));
    }
    install(command, &staged, into)?;
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
    let loaded = acquire(session, args, ID)?;
    guard(ID, loaded, &into)?;
    let (copy, notes) = plan_new(ID, loaded, &req)?;
    target_edid(ID, &into, ID, &req.edid)?;
    let mut limitations = loaded.limitations.clone();
    extend_once(&mut limitations, notes);
    commit(ID, loaded, &into, vec![copy], false, limitations, &[])
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
    )
}
