//! `new` and `clone` (M4.E1): new base forms in a patch plugin.
//!
//! - `new {type, edid, fields?, masters?, into}`: a record of a type the schema defines and
//!   that lives in its top-level group — a new `WRLD` worldspace written alone in its group,
//!   no cells yet — or a new interior `CELL` (lane D round 4: `DATA` flags `Interior` unless
//!   given, placed by the writer in the interior block and sub-block its FormID gives, no
//!   references), with its `EDID`, the members the schema marks
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
    active_file_arg, check, describe, edits, extend_once, find_form, guard_output, install,
    outcome, safe_source, stage, validate_json, winning_copy, write_with, Origin,
};
use super::{failed, str_arg};
use crate::edit::patch::{Copy, Edit, FormName, Nested, ParentForm};
use crate::edit::scaffold::{
    component_name, fill_groups, first_unplaced, lead_refusal, member_order, missing_lead,
    placeholder, record_order, slot_of, slot_placeholder, wanted_slots, Slot,
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
    /// The parent of a nested record (KK-275, KK-276).
    pub parent: Option<ParentArg>,
}

/// A `clone` request.
#[derive(Debug, Clone)]
pub(crate) struct CloneRequest {
    pub label: String,
    pub form: String,
    pub plugin: Option<String>,
    pub edid: String,
    pub edits: Vec<Edit>,
    /// The parent of a nested record (KK-275, KK-276).
    pub parent: Option<ParentArg>,
}

/// The `parent` of a new or cloned nested record (`docs/design/dialogue.md`).
#[derive(Debug, Clone)]
pub(crate) enum ParentArg {
    /// A form of the load order as `set` names one (`plugin` numbers a hex FormID).
    Form {
        form: String,
        plugin: Option<String>,
    },
    /// A form the same build writes (`new:<name>`): its EditorID and record type (`None`
    /// when unknown).
    Local {
        edid: String,
        signature: Option<[u8; 4]>,
    },
}

/// The record type a nested record's parent has: a `QUST` for a topic, a `DIAL` for a
/// response. `None` for every other type (`SCEN` and `DLBR` included: a scene's actions are
/// raw, a branch's members are an inventory definition).
pub(crate) fn parent_type(sig: [u8; 4]) -> Option<[u8; 4]> {
    match &sig {
        b"DIAL" => Some(*b"QUST"),
        b"INFO" => Some(*b"DIAL"),
        _ => None,
    }
}

/// A parent as planned: where the writer puts the record, and the parent's winning version
/// to carry (none for a form of the same build).
pub(crate) struct PlannedParent {
    pub nested: Nested,
    /// The carried copy and the form it is.
    pub carried: Option<(Copy, GlobalForm)>,
    pub limitations: Vec<String>,
}

/// The signature of the winning version of `form`.
fn form_signature(loaded: &Loaded, form: GlobalForm) -> Option<[u8; 4]> {
    let lo = &loaded.lo;
    let winner = lo.forms.versions(form).last()?;
    lo.record(winner.plugin, winner.record)
        .map(|loc| loc.signature)
}

/// Resolves the `parent` of a record of type `sig` (`code` is `new` or `clone`):
/// refused when a nested type has none, when a top-level type has one, when it is missing,
/// deleted or of the wrong type. A form of the load order is carried as an unedited copy of
/// its winning version (its own parents ride along), which the writer keeps out when the
/// target already has it.
pub(crate) fn plan_parent(
    command: &str,
    code: &str,
    loaded: &Loaded,
    at: &str,
    sig: [u8; 4],
    parent: Option<&ParentArg>,
    limits: &DecodeLimits,
) -> Result<Option<PlannedParent>, EngineError> {
    let text = String::from_utf8_lossy(&sig).into_owned();
    let Some(want) = parent_type(sig) else {
        if parent.is_some() {
            return Err(failed(
                command,
                format!("{code}.parent-not-nested: {at}: a {text} lives in its own top-level group; `parent` is only for a DIAL (its quest) or an INFO (its topic)"),
            ));
        }
        return Ok(None);
    };
    let want_text = String::from_utf8_lossy(&want).into_owned();
    let Some(parent) = parent else {
        return Err(failed(
            command,
            format!("{code}.no-parent: {at}: a {text} lives in the children group of its {want_text}; name it with `parent`"),
        ));
    };
    let wrap = |form: ParentForm| {
        if &sig == b"DIAL" {
            Nested::Quest(form)
        } else {
            Nested::Topic(form)
        }
    };
    match parent {
        ParentArg::Local { edid, signature } => {
            if let Some(s) = signature.filter(|s| *s != want) {
                return Err(failed(
                    command,
                    format!(
                        "{code}.parent-type: {at}: the parent `{edid}` is a {}, not a {want_text}",
                        String::from_utf8_lossy(&s)
                    ),
                ));
            }
            Ok(Some(PlannedParent {
                nested: wrap(ParentForm::Local(edid.clone())),
                carried: None,
                limitations: Vec::new(),
            }))
        }
        ParentArg::Form { form, plugin } => {
            let g = find_form(command, loaded, form, plugin.as_deref()).map_err(|e| {
                failed(command, format!("{code}.parent-missing: {at}: parent: {e}"))
            })?;
            let label = loaded.lo.form_label(g);
            match form_signature(loaded, g) {
                Some(s) if s == want => {}
                Some(s) => {
                    return Err(failed(
                        command,
                        format!(
                            "{code}.parent-type: {at}: the parent {label} is a {}, not a {want_text}",
                            String::from_utf8_lossy(&s)
                        ),
                    ))
                }
                None => {
                    return Err(failed(
                        command,
                        format!("{code}.parent-missing: {at}: the parent {label} has no version in the load order"),
                    ))
                }
            }
            let (mut copy, origin) = winning_copy(command, loaded, g, Vec::new(), limits)
                .map_err(|e| failed(command, format!("{code}.parent-missing: {at}: {e}")))?;
            copy.carried = true;
            let plugin_name = loaded
                .lo
                .plugin(g.plugin)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let mut limitations = vec![format!(
                "{at}: the parent {want_text} {label} is carried into the patch as a copy of its winning version (from {}) unless the patch has it: the patch then overrides it, and a later plugin that edits it conflicts with the patch",
                origin.plugin
            )];
            if &want == b"DIAL" {
                limitations.push(format!(
                    "{at}: the topic {label} lists the response in its TIFL (TIFC counted again), so the patch overrides the topic and carries its quest"
                ));
            }
            Ok(Some(PlannedParent {
                nested: wrap(ParentForm::Form(FormName {
                    plugin: plugin_name,
                    object_id: g.object_id,
                })),
                carried: Some((copy, g)),
                limitations,
            }))
        }
    }
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

/// Record types `new` refuses: [`nested_or_world`] less `CELL` and `WRLD`. A new `CELL` is
/// an interior cell (lane D round 4): the writer puts it in the interior block and sub-block
/// its FormID computes (`kitkraft_esm::cells::interior_block`), with no children; a new
/// `WRLD` is written alone in its top-level group, no world children and no cells (`place`
/// adds them); a new exterior cell (a world and a grid) is still not written.
fn new_refused(sig: [u8; 4]) -> bool {
    !matches!(&sig, b"CELL" | b"WRLD") && parent_type(sig).is_none() && nested_or_world(sig)
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

/// What the Creation Kit writes into a required member of a new record of the type, where
/// it is not all zeros: each payload as the CK saved it for a record KitKraft had created
/// with its EditorID only (the CK bench batch, `ck-diff:` sittings ck-20261117-13, -14 and
/// -16; `docs/bench/ck-batch/triage-20261117.md`, class A). Same size as our zeros.
const CK_NEW_DEFAULTS: &[([u8; 4], [u8; 4], &[u8])] = &[
    // WFIR trigger thresholds (primary, second stage) 0.33.
    (
        *b"WEAP",
        *b"WFIR",
        &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0xC3, 0xF5, 0xA8, 0x3E, 0, 0xC3, 0xF5, 0xA8, 0x3E, 0, 0, 0, 0, 0, 0,
        ],
    ),
    // WGEN base speed 1.0.
    (
        *b"WEAP",
        *b"WGEN",
        &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x3F, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ),
    // WMEL reach 1.0.
    (
        *b"WEAP",
        *b"WMEL",
        &[0, 0, 0, 0, 0, 0, 0, 0x80, 0x3F, 0, 0, 0, 0],
    ),
    // WRLO reload speed 1.0.
    (*b"WEAP", *b"WRLO", &[0, 0, 0, 0, 0, 0, 0x80, 0x3F, 0, 0]),
    // DAT2 time -1, falloff exponent 1.0, FOV 90.0, radius falloff exponent 4.0.
    (
        *b"LIGH",
        *b"DAT2",
        &[
            0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x3F, 0, 0,
            0xB4, 0x42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x40,
        ],
    ),
    // LLLD light layer 15.
    (*b"LIGH", *b"LLLD", &[0x0F, 0, 0, 0]),
    // MNAM active markers 0x40000000.
    (*b"FURN", *b"MNAM", &[0, 0, 0, 0x40]),
    (*b"TERM", *b"MNAM", &[0, 0, 0, 0x40]),
    // NAM5 FF00; NAM6 height min 1.0.
    (*b"NPC_", *b"NAM5", &[0xFF, 0]),
    (*b"NPC_", *b"NAM6", &[0, 0, 0x80, 0x3F]),
    // LRNM learn method 3.
    (*b"COBJ", *b"LRNM", &[3]),
    // TRDA emotion "none" is FFFFFFFF (the CK's, ck-20261117-17; all 53,822 official
    // responses without an emotion carry FFFFFFFF, none 0).
    (
        *b"INFO",
        *b"TRDA",
        &[0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
];

/// The CK's default for a required member of a new `record` ([`CK_NEW_DEFAULTS`]) when it
/// has the size of `zeros`, our placeholder; else `zeros`.
pub(crate) fn ck_new_default(record: [u8; 4], member: [u8; 4], zeros: Vec<u8>) -> Vec<u8> {
    CK_NEW_DEFAULTS
        .iter()
        .find(|(r, m, b)| *r == record && *m == member && b.len() == zeros.len())
        .map_or(zeros, |(_, _, b)| b.to_vec())
}

/// The members the Creation Kit adds to a new record that the definition does not require
/// (the CK bench batch, sittings ck-20261117-11 to -18: each record KitKraft had created
/// with its EditorID only, as the CK saved it; `docs/bench/ck-batch/triage-20261117.md`,
/// class A). Each is the record type, the member, which slot of that signature it takes
/// (the `k`-th in schema member order, groups flattened: WEAP's `FLLD` 0 is the model's,
/// 2 the first-person model's, 1 a destruction stage's; ARMA's four biped models take 0
/// to 3, its repeating `LightLayers` the other two) and its payload as
/// the CK saved it, in hex (`0*<n>`: `n` zero bytes). `new` writes them unless the fields
/// give that signature. Left out: NPC_ `RNAM` (the CK's race `Starfield.esm:00347D`, a
/// form of a master the plugin may not have) and PNDT's keyword component (component
/// layouts are another lane's tonight). BOOK `CNAM` is written as the 4 zero bytes of its
/// layout and of all 992 official BOOKs; the CK narrows it to one (class B rule
/// `BOOK CNAM zero-narrowed`).
const CK_NEW_MEMBERS: &[([u8; 4], [u8; 4], usize, &str)] = &[
    (*b"ALCH", *b"FLLD", 0, "01000000"),
    (*b"ALCH", *b"DESC", 0, "00"),
    (*b"ALCH", *b"DNAM", 0, "00"),
    (*b"ARMA", *b"FLLD", 0, "01000000"),
    (*b"ARMA", *b"FLLD", 1, "01000000"),
    (*b"ARMA", *b"FLLD", 2, "01000000"),
    (*b"ARMA", *b"FLLD", 3, "01000000"),
    (*b"ARMA", *b"FLLD", 4, "01000000"),
    (*b"ARMA", *b"FLLD", 4, "01000000"),
    (*b"ARMA", *b"MNAM", 0, "00000000"),
    (*b"ARMA", *b"TNAM", 0, "00"),
    (*b"ARMA", *b"SNAM", 0, "00"),
    (*b"ARMA", *b"VNAM", 0, "00"),
    (*b"ARMA", *b"BSMP", 0, "00000000"),
    (*b"ARMA", *b"BSMP", 0, "01000000"),
    (*b"ARMO", *b"FLLD", 0, "01000000"),
    (*b"ARMO", *b"FLLD", 1, "01000000"),
    (*b"ARMO", *b"STOP", 0, ""),
    (*b"BOOK", *b"FLLD", 0, "01000000"),
    (*b"BOOK", *b"DESC", 0, "00"),
    (*b"BOOK", *b"CNAM", 0, "00000000"),
    (*b"CELL", *b"XCLL", 0, "000000000000000000000000000000000000000000000000000000000000803F000000000000803F000000000000803F00000000000000009F0700000000000000401C4600000000000000000000803F0000803F0000803F0000803F0000803F000000000000000000000000"),
    (*b"CELL", *b"LTMP", 0, "00000000"),
    (*b"CELL", *b"XCLW", 0, "FFFF7F7F"),
    (*b"CELL", *b"XILS", 0, "0000803F"),
    (*b"KEYM", *b"FLLD", 0, "01000000"),
    (*b"LCTN", *b"DATA", 0, "0000000000001000"),
    (*b"LCTN", *b"RNAM", 0, "00000000"),
    (*b"LCTN", *b"ANAM", 0, "0000803F"),
    (*b"LCTN", *b"TNAM", 0, "00000000"),
    (*b"LVLI", *b"FLLD", 0, "01000000"),
    (*b"LVLN", *b"FLLD", 0, "01000000"),
    (*b"LVLP", *b"FLLD", 0, "01000000"),
    (*b"MISC", *b"FLLD", 0, "01000000"),
    (*b"NPC_", *b"STOP", 0, ""),
    (*b"NPC_", *b"DNAM", 0, "0000000000000100"),
    (*b"NPC_", *b"NAM4", 0, "0000803F"),
    (*b"NPC_", *b"MWGT", 0, "000000000000000000000000"),
    (*b"NPC_", *b"EDCT", 0, "00"),
    (*b"NPC_", *b"STON", 0, "00"),
    (*b"NPC_", *b"PRON", 0, "00"),
    (*b"PACK", *b"POBA", 0, ""),
    (*b"PACK", *b"INAM", 0, "00000000"),
    (*b"PACK", *b"PDTO", 0, "0000000000000000"),
    (*b"PACK", *b"POEA", 0, ""),
    (*b"PACK", *b"INAM", 1, "00000000"),
    (*b"PACK", *b"PDTO", 1, "0000000000000000"),
    (*b"PACK", *b"POCA", 0, ""),
    (*b"PACK", *b"INAM", 2, "00000000"),
    (*b"PACK", *b"PDTO", 2, "0000000000000000"),
    (*b"WEAP", *b"FLLD", 0, "01000000"),
    (*b"WEAP", *b"FLLD", 2, "01000000"),
    (*b"WEAP", *b"STOP", 0, ""),
];

/// A [`CK_NEW_MEMBERS`] payload: hex, or `0*<n>` for `n` zero bytes.
fn ck_payload(text: &str) -> Vec<u8> {
    if let Some(n) = text.strip_prefix("0*") {
        return vec![0; n.parse().unwrap_or(0)];
    }
    text.as_bytes()
        .chunks(2)
        .filter_map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

/// The [`CK_NEW_MEMBERS`] of a new `record` laid out as `order`: each with the index of
/// its slot and its payload. An entry whose slot the order lacks is skipped.
pub(crate) fn ck_new_members(record: [u8; 4], order: &[Slot]) -> Vec<(usize, [u8; 4], Vec<u8>)> {
    CK_NEW_MEMBERS
        .iter()
        .filter(|(r, _, _, _)| *r == record)
        .filter_map(|(_, m, k, hex)| {
            let slot = order
                .iter()
                .enumerate()
                .filter(|(_, e)| e.sig == *m)
                .nth(*k)
                .map(|(i, _)| i)?;
            Some((slot, *m, ck_payload(hex)))
        })
        .collect()
}

/// The patch-writer copy for a `new` request.
pub(crate) fn plan_new(
    command: &str,
    loaded: &Loaded,
    req: &NewRequest,
) -> Result<NewPlan, EngineError> {
    let at = &req.label;
    let sig = req.signature;
    let text = String::from_utf8_lossy(&sig).into_owned();
    if !has_definition(Signature(sig)) {
        return Err(failed(command, format!("new.untyped: {at}: {text} has no record definition in the schema; a new form is written only for a typed type")));
    }
    if new_refused(sig) {
        return Err(failed(command, format!("new.not-top-level: {at}: a {text} lives in a world, cell or quest's children (use `place` for references); `new` writes records of their own top-level group, interior cells, worldspaces, and topics and responses under a `parent`")));
    }
    check_edid(command, "new", loaded, at, &req.edid)?;
    if let Some(e) = req
        .edits
        .iter()
        .find(|e| crate::edit::scaffold::is_condition_removal(e))
    {
        return Err(failed(
            command,
            format!(
                "new.remove: {at}: {} = null removes a condition, and a new record has none",
                e.path.text
            ),
        ));
    }
    let planned = plan_parent(
        command,
        "new",
        loaded,
        at,
        sig,
        req.parent.as_ref(),
        &DecodeLimits::default(),
    )?;
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
    // The component layout a new record holds: the one its fields select, or none
    // (scaffold.rs `record_order`; KK-220 for QUST, generalised for LVLI). The block's
    // other layouts are not slots of this record.
    let order = record_order(sig, &wanted, component_name(&req.edits));
    let mut edid = req.edid.as_bytes().to_vec();
    edid.push(0);
    // Each subrecord with the position of its slot; EDID first.
    let edid_slot = slot_of(&order, *b"EDID", 0, 1).unwrap_or(0);
    let mut subs: Vec<(usize, Subrecord<'static>)> =
        vec![(edid_slot, Subrecord::new(Signature(*b"EDID"), edid))];
    let mut limitations = Vec::new();
    let (nested, carried) = match planned {
        Some(p) => {
            limitations.extend(p.limitations);
            (Some(p.nested), p.carried)
        }
        None => (None, None),
    };
    let mut defaulted: Vec<String> = Vec::new();
    for (i, slot) in order.iter().enumerate() {
        let m = slot.sig;
        if slot.card == Cardinality::Required
            && m != *b"EDID"
            && !subs.iter().any(|(_, s)| s.signature().0 == m)
            && !wanted.contains_key(&m)
        {
            match placeholder(sig, m, false).map(|zeros| ck_new_default(sig, m, zeros)) {
                Some(bytes) => {
                    defaulted.push(String::from_utf8_lossy(&m).into_owned());
                    subs.push((i, Subrecord::new(Signature(m), bytes)));
                }
                // A topic's response list starts empty: every official topic has a TIFL,
                // the 5,744 without responses an empty one (`docs/design/dialogue.md`).
                None if &sig == b"DIAL" && &m == b"TIFL" => {
                    subs.push((i, Subrecord::new(Signature(m), Vec::new())));
                }
                None => limitations.push(format!(
                    "{at}: the schema requires {} on a {text}, which has no fixed size; give it in `fields` as hex:",
                    String::from_utf8_lossy(&m)
                )),
            }
        }
    }
    // The members the CK adds to a new record of the type, unless the fields give the
    // signature or the slot is taken (a required member's).
    for (slot, m, bytes) in ck_new_members(sig, &order) {
        let repeats = order.get(slot).is_some_and(|e| e.repeated);
        if wanted.contains_key(&m) || (!repeats && subs.iter().any(|(i, _)| *i == slot)) {
            continue;
        }
        defaulted.push(String::from_utf8_lossy(&m).into_owned());
        subs.push((slot, Subrecord::new(Signature(m), bytes)));
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
                placeholder(sig, *s, whole)
                    .map(|zeros| ck_new_default(sig, *s, zeros))
                    .or_else(from_slot)
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
    if let Some((i, bad)) = first_unplaced(sig, &subs, |_| true) {
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
            "{at}: members the schema requires were written as zeros (or an empty string), or as the Creation Kit's default where the CK bench observed one (WEAP, LIGH, FURN, TERM, NPC_, COBJ), unless given in fields: {}",
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
    // A nested record's path is the writer's (`Copy::nested`); this one is a placeholder.
    let path = if nested.is_some() {
        vec![GroupHeader::top(Signature(*b"QUST"))]
    } else {
        vec![GroupHeader::top(Signature(sig))]
    };
    Ok((
        Copy {
            source: SourcePlugin {
                name: String::new(),
                path: PathBuf::new(),
                masters: masters.into_iter().map(|(_, n)| n).collect(),
                flags: 0,
            },
            record,
            path,
            parents: Vec::new(),
            edits: req.edits.clone(),
            formref: format!("new {text}"),
            carried: false,
            new_form: Some(req.edid.clone()),
            scripts: Vec::new(),
            nested,
        },
        limitations,
        carried,
    ))
}

/// The patch-writer copy for a `clone` request: the base form cloned (its winner) and its
/// signature.
pub(crate) fn plan_clone(
    command: &str,
    loaded: &Loaded,
    req: &CloneRequest,
    limits: &DecodeLimits,
) -> Result<ClonePlan, EngineError> {
    let at = &req.label;
    let form = find_form(command, loaded, &req.form, req.plugin.as_deref())
        .map_err(|e| failed(command, format!("clone.no-form: {at}: {e}")))?;
    let mut unrelated = safe_source(command, loaded, form)?;
    check_edid(command, "clone", loaded, at, &req.edid)?;
    let (mut copy, origin) = winning_copy(command, loaded, form, req.edits.clone(), limits)?;
    let sig = copy.record.header.signature.0;
    if parent_type(sig).is_some() {
        if req.parent.is_none() {
            let source_parent = copy
                .parents
                .last()
                .map(|(r, _)| {
                    let edid = r
                        .subrecords()
                        .and_then(|subs| subs.iter().find(|s| s.signature() == Signature(*b"EDID")))
                        .and_then(|s| crate::edit::fields::text_of(s.data()));
                    match edid {
                        Some(e) => format!("{} `{e}`", r.header.signature),
                        None => format!(
                            "{} {:08X} as stored in {}",
                            r.header.signature, r.header.form_id, copy.source.name
                        ),
                    }
                })
                .unwrap_or_else(|| "unknown".to_string());
            return Err(failed(
                command,
                format!(
                    "clone.no-parent: {at}: {} is a {} in nested groups; name the clone's parent with `parent` (the source's is the {source_parent}); a clone is never placed under its source's parent implicitly",
                    loaded.lo.form_label(form),
                    String::from_utf8_lossy(&sig)
                ),
            ));
        }
        let planned = plan_parent(
            command,
            "clone",
            loaded,
            at,
            sig,
            req.parent.as_ref(),
            limits,
        )?
        .ok_or_else(|| failed(command, format!("clone.no-parent: {at}: no parent")))?;
        unrelated.extend(planned.limitations);
        // The clone hangs under its new parent only: its source's parents stay out.
        copy.parents.clear();
        copy.path = vec![GroupHeader::top(Signature(*b"QUST"))];
        copy.nested = Some(planned.nested);
        if &sig == b"DIAL" {
            unrelated.extend(reset_topic(command, at, &mut copy.record, limits)?);
        }
        copy.new_form = Some(req.edid.clone());
        return Ok((copy, origin, form, unrelated, planned.carried));
    }
    // A parent for a top-level type is refused (`parent-not-nested`).
    plan_parent(
        command,
        "clone",
        loaded,
        at,
        sig,
        req.parent.as_ref(),
        limits,
    )?;
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
    Ok((copy, origin, form, unrelated, None))
}

/// What [`plan_new`] gives: the copy, limitations, and the parent to carry.
pub(crate) type NewPlan = (Copy, Vec<String>, Option<(Copy, GlobalForm)>);

/// What [`plan_clone`] gives: the copy, its origin, the form cloned, limitations, and the
/// parent to carry (a nested clone's).
pub(crate) type ClonePlan = (
    Copy,
    Origin,
    GlobalForm,
    Vec<String>,
    Option<(Copy, GlobalForm)>,
);

/// A cloned topic owns no responses: its `TIFL` is emptied and `TIFC` set to 0 (every
/// official topic without responses has both so), and its `QNAM` cleared for the writer to
/// set to the new parent. Notes the branch (`BNAM`) kept as stored.
fn reset_topic(
    command: &str,
    at: &str,
    record: &mut Record<'static>,
    limits: &DecodeLimits,
) -> Result<Vec<String>, EngineError> {
    record
        .decompress(limits)
        .map_err(|e| failed(command, format!("clone: {at}: {e}")))?;
    let mut notes = Vec::new();
    let Some(subs) = record.subrecords_mut() else {
        return Ok(notes);
    };
    for s in subs.iter_mut() {
        match &s.signature().0 {
            b"TIFL" if !s.data().is_empty() => {
                notes.push(format!(
                    "{at}: the clone owns no responses: TIFL emptied ({} listed in the source) and TIFC 0; clone each response with the clone as its `parent`",
                    s.data().len() / 4
                ));
                s.set_data(Vec::new());
            }
            b"TIFC" => s.set_data(0u32.to_le_bytes().to_vec()),
            b"QNAM" => s.set_data(0u32.to_le_bytes().to_vec()),
            b"BNAM" => notes.push(format!(
                "{at}: BNAM (the dialogue branch) is kept as stored; official topics name a branch of their own quest, so check it when the parent quest is another"
            )),
            _ => {}
        }
    }
    Ok(notes)
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
    in_place: Option<&Path>,
) -> Result<Outcome, EngineError> {
    let staged = stage(command, into, true)?;
    let mut report = write_with(
        command,
        loaded,
        &staged.path,
        copies,
        Vec::new(),
        allow_untyped,
        in_place,
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

/// The `parent` argument (KK-275, KK-276): a form of the load order; `plugin` numbers a
/// hex FormID.
fn parent_arg(args: &Value, plugin: Option<&str>) -> Option<ParentArg> {
    args.get("parent")
        .and_then(Value::as_str)
        .map(|form| ParentArg::Form {
            form: form.to_string(),
            plugin: plugin.map(str::to_string),
        })
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
        parent: parent_arg(args, None),
    };
    let dry_run = super::preview::requested(args).then(|| super::load::schema_of(session));
    let loaded = acquire(session, args, ID)?;
    let in_place = guard_output(ID, loaded, &into, active_file_arg(args))?;
    let (mut copy, notes, carried) = plan_new(ID, loaded, &req)?;
    target_edid(ID, &into, ID, &req.edid)?;
    // Scripts to attach (M4.E9): refused here when a script or property is unknown.
    let scripts = super::scripts::from_args(ID, args, loaded, &into)?;
    copy.scripts = scripts.scripts;
    let mut limitations = loaded.limitations.clone();
    extend_once(&mut limitations, notes);
    extend_once(&mut limitations, scripts.limitations);
    let mut copies: Vec<Copy> = carried.into_iter().map(|(c, _)| c).collect();
    copies.push(copy);
    let mut result = commit(
        ID,
        loaded,
        &into,
        copies,
        false,
        limitations,
        &[],
        dry_run,
        in_place.as_deref(),
    )?;
    if let Some(obj) = result.data.as_object_mut() {
        if !scripts.reports.is_empty() {
            obj.insert("scripts".into(), Value::Array(scripts.reports));
        }
    }
    after_in_place(
        session,
        ID,
        in_place.is_some(),
        dry_run.is_some(),
        &mut result,
    );
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
        parent: parent_arg(args, args.get("plugin").and_then(Value::as_str)),
    };
    let dry_run = super::preview::requested(args).then(|| super::load::schema_of(session));
    let loaded = acquire(session, args, ID)?;
    let in_place = guard_output(ID, loaded, &into, active_file_arg(args))?;
    let limits = DecodeLimits::default();
    let (mut copy, origin, _, unrelated, carried) = plan_clone(ID, loaded, &req, &limits)?;
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
    let mut copies: Vec<Copy> = carried.into_iter().map(|(c, _)| c).collect();
    copies.push(copy);
    let mut result = commit(
        ID,
        loaded,
        &into,
        copies,
        allow_untyped,
        limitations,
        &[origin],
        dry_run,
        in_place.as_deref(),
    )?;
    if let Some(obj) = result.data.as_object_mut() {
        if let Some(report) = swap_report {
            obj.insert("swap_overrides".into(), Value::Array(report));
        }
    }
    after_in_place(
        session,
        ID,
        in_place.is_some(),
        dry_run.is_some(),
        &mut result,
    );
    Ok(result)
}

/// After a write into the loaded active file (A6), the session's load order is read again
/// (`set::reload`); a dry run wrote nothing, so nothing is re-read.
fn after_in_place(
    session: &mut Session,
    command: &str,
    in_place: bool,
    dry_run: bool,
    result: &mut Outcome,
) {
    if in_place && !dry_run {
        if let Some(note) = super::set::reload(session, command) {
            result.limitations.push(note);
        }
    }
}
