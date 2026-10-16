//! Subrecord slots of a record type in schema member order, and the bytes a created
//! subrecord starts with: what `new` (M4.E1) builds a record from, what `place` (lane D
//! round 4) adds to a composed reference before applying its `fields`, and what `set`,
//! `clone` and a spec's `[[set]]` add to a copied record that lacks a subrecord its fields
//! name (lane D round 5).
//!
//! A whole array subrecord counted by the subrecord before it (`KSIZ` before `KWDA`:
//! [`count_slot`]) brings that count subrecord along when both are absent; `apply_field`
//! then writes the count.

use std::collections::BTreeMap;

use kitkraft_esm::refs::RecordStamps;
use kitkraft_esm::{Record, Signature, Subrecord};
use kitkraft_schema::{ArrayCount, Cardinality, FieldType, Member};

use super::fields::{self, FieldError};
use super::patch::Edit;

/// One subrecord slot of a record type, in schema member order (groups flattened).
#[derive(Debug, Clone)]
pub(crate) struct Slot {
    pub(crate) sig: [u8; 4],
    /// The member's schema name (`Count` for `KSIZ`).
    pub(crate) name: String,
    /// Required at the record's level (a group's members never are on their own).
    pub(crate) card: Cardinality,
    pub(crate) ty: FieldType,
    /// Repeated: the slot takes any number of consecutive occurrences.
    pub(crate) repeated: bool,
    /// Inside a component layout of the base form component block
    /// (`kitkraft_schema::in_component_layout`).
    pub(crate) component: bool,
}

/// Every subrecord slot of `sig` in schema order (groups flattened).
pub(crate) fn member_order(sig: [u8; 4]) -> Vec<Slot> {
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
                    name: s.name.clone(),
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
pub(crate) fn slot_of(order: &[Slot], s: [u8; 4], k: usize, n: usize) -> Option<usize> {
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
pub(crate) fn slot_placeholder(ty: &FieldType, whole_value: bool) -> Option<Vec<u8>> {
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
pub(crate) fn placeholder(record: [u8; 4], sub: [u8; 4], whole_value: bool) -> Option<Vec<u8>> {
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

/// The count subrecord of the array subrecord `array` in a record type laid out as `order`:
/// the slot just before the array's, when the array runs to the end of its payload with
/// fixed-size elements and that slot is a lone integer named `...Count` (`KSIZ` `Count`
/// before `KWDA`; `XWCN` before `XWCU`). Returns the count's signature and layout.
pub(crate) fn count_slot(order: &[Slot], array: [u8; 4]) -> Option<([u8; 4], FieldType)> {
    let at = order.iter().position(|e| e.sig == array && !e.component)?;
    let slot = order.get(at)?;
    let fixed_elements = match deref(&slot.ty) {
        FieldType::Array {
            element,
            count: ArrayCount::Rest,
        } => element
            .fixed_size(Some(RecordStamps::default().form_version))
            .is_some_and(|n| n > 0),
        _ => false,
    };
    let before = order.get(at.checked_sub(1)?)?;
    let integer = matches!(deref(&before.ty), FieldType::Scalar(s) if s.is_integer());
    (fixed_elements
        && integer
        && !before.repeated
        && !slot.repeated
        && before.sig != array
        && before.name.ends_with("Count"))
    .then(|| (before.sig, before.ty.clone()))
}

fn deref(ty: &FieldType) -> &FieldType {
    match ty {
        FieldType::Ref { ty, .. } => deref(ty),
        other => other,
    }
}

/// How many occurrences of each signature `edits` need in a record that has `have` of
/// each: the occurrences the edits name, plus the count subrecord of a whole array
/// subrecord the record lacks along with its count ([`count_slot`]).
pub(crate) fn wanted_slots(
    order: &[Slot],
    edits: &[Edit],
    have: &BTreeMap<[u8; 4], usize>,
) -> BTreeMap<[u8; 4], usize> {
    let mut wanted: BTreeMap<[u8; 4], usize> = BTreeMap::new();
    for edit in edits {
        let n = wanted.entry(edit.path.signature).or_insert(0);
        *n = (*n).max(edit.path.occurrence.saturating_add(1));
    }
    let present = |s: &[u8; 4]| have.get(s).copied().unwrap_or(0);
    for edit in edits.iter().filter(|e| e.path.steps.is_empty()) {
        let s = edit.path.signature;
        if present(&s) > 0 {
            continue;
        }
        if let Some((count, _)) = count_slot(order, s) {
            if present(&count) == 0 {
                wanted.entry(count).or_insert(1);
            }
        }
    }
    wanted
}

/// Adds to `record` a subrecord for every occurrence `edits` name that it lacks (zero bytes
/// of the slot's fixed size, or an empty string), each at its slot in schema member order,
/// so the edits can then be applied as `set` applies them; a whole array subrecord brings
/// its count subrecord ([`wanted_slots`]). The record's own subrecords keep their order;
/// one with no slot keeps its place before the slots after it. A signature with no slot
/// in the record type, or a variable layout given no whole value, is a `no-subrecord`
/// error naming the field. A record that lacks nothing is not touched. Without
/// `fill_gaps` (a copied record: `set`, `clone`), every occurrence added must be one the
/// edits name: `FULL[5]` on a record with one `FULL` is refused rather than inventing
/// `FULL[1]` to `FULL[4]`; a composed reference (`place`) fills them as `new` does.
pub(crate) fn add_slots(
    record: &mut Record<'static>,
    edits: &[Edit],
    fill_gaps: bool,
) -> Result<(), FieldError> {
    let sig = record.header.signature.0;
    let type_name = String::from_utf8_lossy(&sig).into_owned();
    let Some(current) = record.subrecords() else {
        return Err(FieldError::new(
            &type_name,
            "payload",
            "the record body is compressed",
        ));
    };
    let mut have: BTreeMap<[u8; 4], usize> = BTreeMap::new();
    for s in current {
        let n = have.entry(s.signature().0).or_insert(0);
        *n = n.saturating_add(1);
    }
    let lacks = edits
        .iter()
        .any(|e| have.get(&e.path.signature).copied().unwrap_or(0) <= e.path.occurrence);
    if !lacks {
        return Ok(());
    }
    let order = member_order(sig);
    let wanted = wanted_slots(&order, edits, &have);
    let mut added: Vec<(usize, Subrecord<'static>)> = Vec::new();
    for (s, n) in &wanted {
        let present = have.get(s).copied().unwrap_or(0);
        let total = (*n).max(present);
        for k in present..*n {
            let named = edits
                .iter()
                .find(|e| e.path.signature == *s && e.path.occurrence == k);
            // The edit that needs this occurrence: the one naming it, else the furthest
            // one of the signature (a gap before it), else none (a count subrecord).
            let needing = named.or_else(|| {
                edits
                    .iter()
                    .filter(|e| e.path.signature == *s)
                    .max_by_key(|e| e.path.occurrence)
            });
            let whole = edits.iter().any(|e| {
                e.path.signature == *s && e.path.occurrence == k && e.path.steps.is_empty()
            });
            let shown = needing.map_or_else(
                || format!("{}[{k}]", String::from_utf8_lossy(s)),
                |e| e.path.text.clone(),
            );
            if named.is_none() && needing.is_some() && !fill_gaps {
                return Err(FieldError::new(
                    &shown,
                    "no-subrecord",
                    format!(
                        "the record has {present} {} subrecord(s); an occurrence is added only when named, so {}[{k}] would be invented (name every occurrence up to it)",
                        String::from_utf8_lossy(s),
                        String::from_utf8_lossy(s)
                    ),
                ));
            }
            let slot = slot_of(&order, *s, k, total).ok_or_else(|| {
                let why = if order.iter().any(|e| e.sig == *s) {
                    format!(
                        "the record has {present} {} subrecord(s) and {type_name} has no slot for occurrence {k}",
                        String::from_utf8_lossy(s)
                    )
                } else {
                    format!(
                        "the record has no {} subrecord and {type_name} has no slot for one in the schema",
                        String::from_utf8_lossy(s)
                    )
                };
                FieldError::new(&shown, "no-subrecord", why)
            })?;
            let from_slot = || order.get(slot).and_then(|e| slot_placeholder(&e.ty, whole));
            let bytes = if total <= 1 {
                placeholder(sig, *s, whole).or_else(from_slot)
            } else {
                from_slot()
            }
            .ok_or_else(|| {
                FieldError::new(
                    &shown,
                    "no-subrecord",
                    "the record lacks it and its layout gives a new subrecord nothing to start from (give the whole subrecord as hex:)",
                )
            })?;
            added.push((slot, Subrecord::new(Signature(*s), bytes)));
        }
    }
    if added.is_empty() {
        return Ok(());
    }
    let Some(subs) = record.subrecords_mut() else {
        return Err(FieldError::new(
            &type_name,
            "payload",
            "the record body is compressed",
        ));
    };
    // The existing subrecords with their slots (occurrence by occurrence); one with no slot
    // takes the slot of the one before it.
    let mut seen: BTreeMap<[u8; 4], usize> = BTreeMap::new();
    let mut last = 0usize;
    let mut all: Vec<(usize, Subrecord<'static>)> = Vec::new();
    for s in subs.drain(..) {
        let k = seen.entry(s.signature().0).or_insert(0);
        let total = wanted
            .get(&s.signature().0)
            .copied()
            .unwrap_or(0)
            .max(have.get(&s.signature().0).copied().unwrap_or(0));
        if let Some(slot) = slot_of(&order, s.signature().0, *k, total) {
            last = last.max(slot);
        }
        *k = k.saturating_add(1);
        all.push((last, s.into_owned()));
    }
    all.extend(added);
    // Stable: equal slots keep the record's order, then the added ones.
    all.sort_by_key(|(slot, _)| *slot);
    subs.extend(all.into_iter().map(|(_, s)| s));
    Ok(())
}
