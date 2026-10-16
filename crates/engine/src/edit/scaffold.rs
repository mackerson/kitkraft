//! Subrecord slots of a record type in schema member order, and the bytes a created
//! subrecord starts with: what `new` (M4.E1) builds a record from, and what `place` (lane D
//! round 4) adds to a composed reference before applying its `fields`.

use kitkraft_esm::refs::RecordStamps;
use kitkraft_esm::{Record, Signature, Subrecord};
use kitkraft_schema::{Cardinality, FieldType, Member};

use super::fields;
use super::patch::Edit;

/// One subrecord slot of a record type, in schema member order (groups flattened).
#[derive(Debug, Clone)]
pub(crate) struct Slot {
    pub(crate) sig: [u8; 4],
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

/// Adds to `record` a subrecord for every occurrence `edits` name that it lacks (zero bytes
/// of the slot's fixed size, or an empty string), each at its slot in schema member order,
/// so the edits can then be applied as `set` applies them. The record's own subrecords keep
/// their order; one with no slot keeps its place before the slots after it. A signature
/// with no slot, or a variable layout given no whole value, is an error naming the field.
pub(crate) fn add_slots(record: &mut Record<'static>, edits: &[Edit]) -> Result<(), String> {
    let sig = record.header.signature.0;
    let order = member_order(sig);
    let Some(subs) = record.subrecords_mut() else {
        return Err("the record body is compressed".into());
    };
    let mut wanted: std::collections::BTreeMap<[u8; 4], usize> = std::collections::BTreeMap::new();
    for edit in edits {
        let n = wanted.entry(edit.path.signature).or_insert(0);
        *n = (*n).max(edit.path.occurrence.saturating_add(1));
    }
    let mut have: std::collections::BTreeMap<[u8; 4], usize> = std::collections::BTreeMap::new();
    for s in subs.iter() {
        let n = have.entry(s.signature().0).or_insert(0);
        *n = n.saturating_add(1);
    }
    let mut added: Vec<(usize, Subrecord<'static>)> = Vec::new();
    for (s, n) in &wanted {
        let present = have.get(s).copied().unwrap_or(0);
        let total = (*n).max(present);
        for k in present..*n {
            let whole = edits.iter().any(|e| {
                e.path.signature == *s && e.path.occurrence == k && e.path.steps.is_empty()
            });
            let shown = String::from_utf8_lossy(s).into_owned();
            let slot = slot_of(&order, *s, k, total).ok_or_else(|| {
                format!(
                    "`{shown}[{k}]`: {} has no slot for it",
                    String::from_utf8_lossy(&sig)
                )
            })?;
            let from_slot = || order.get(slot).and_then(|e| slot_placeholder(&e.ty, whole));
            let bytes = if total <= 1 {
                placeholder(sig, *s, whole).or_else(from_slot)
            } else {
                from_slot()
            }
            .ok_or_else(|| {
                format!("`{shown}[{k}]` has no layout a new subrecord can start from (give the whole subrecord as hex:)")
            })?;
            added.push((slot, Subrecord::new(Signature(*s), bytes)));
        }
    }
    if added.is_empty() {
        return Ok(());
    }
    // The existing subrecords with their slots (occurrence by occurrence); one with no slot
    // takes the slot of the one before it.
    let mut seen: std::collections::BTreeMap<[u8; 4], usize> = std::collections::BTreeMap::new();
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
