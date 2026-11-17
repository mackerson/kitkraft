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
    /// The member path, group names included (`MapMarker.Name`).
    pub(crate) path: String,
    /// Required at the record's level (a group's members never are on their own).
    pub(crate) card: Cardinality,
    pub(crate) ty: FieldType,
    /// Repeated: the slot takes any number of consecutive occurrences.
    pub(crate) repeated: bool,
    /// Inside a component layout of the base form component block
    /// (`kitkraft_schema::in_component_layout`).
    pub(crate) component: bool,
    /// The groups this slot is inside, innermost first: each group's member path and the
    /// signatures it can start with (its leads: its members up to and including the first
    /// required one, as the decoder matches a group). A group is read only from a lead, so
    /// a member written without one sits where the definition does not place it (Fable
    /// review of wave 3, finding 1: `XPDO` without `XPDD` on a REFR).
    pub(crate) groups: Vec<(String, Vec<[u8; 4]>)>,
}

/// Most nested groups [`member_order`] descends (the decoder's own limit).
const MAX_GROUP_DEPTH: usize = 16;

/// The signatures a group of `members` can start with: each member's own (a nested
/// group's leads) up to and including the first required member (`kitkraft_doc::typed`'s
/// matcher, `starts`).
fn leads(members: &[Member], depth: usize) -> Vec<[u8; 4]> {
    let mut out = Vec::new();
    if depth > MAX_GROUP_DEPTH {
        return out;
    }
    for m in members {
        let required = match m {
            Member::Subrecord(s) => {
                out.push(s.signature.as_bytes());
                s.cardinality == Cardinality::Required
            }
            Member::Group(g) => {
                out.extend(leads(&g.members, depth.saturating_add(1)));
                g.cardinality == Cardinality::Required
            }
            Member::AnyUntil(_) => false,
        };
        if required {
            break;
        }
    }
    out
}

/// Every subrecord slot of `sig` in schema order (groups flattened, the component block's
/// layouts all included).
pub(crate) fn member_order(sig: [u8; 4]) -> Vec<Slot> {
    order_in_layout(sig, None)
}

/// [`member_order`] in which the base form component block holds only the component
/// layout `layout`: a record holds one component (its `BFCB` names it), and the block's
/// other layouts are not slots of the record. An empty `layout` names none: the block
/// keeps only its own members (`BFCB`, `BFCE`).
pub(crate) fn member_order_of_layout(sig: [u8; 4], layout: &str) -> Vec<Slot> {
    order_in_layout(sig, Some(layout))
}

/// The slots of a record of `sig` holding `counts` occurrences of each signature: the
/// record's own members and the slots of the component layout those occurrences select
/// (KK-220, generalised for LVLI). A signature the record cannot place wholly in its own
/// slots (more occurrences than it has slots outside the block's layouts, the last of
/// them not repeating) sits in a block layout that carries it, and the layout is the
/// intersection of those; a signature its own slots can take constrains nothing (its
/// `k`-th occurrence is the record's `k`-th own slot), and one with no layout member
/// names no layout.
///
/// - One layout left: the block holds only it ([`member_order_of_layout`]).
/// - No signature constrains a layout: the block holds none, so no layout slot takes an
///   occurrence (a new LVLI with a list condition and an entry condition puts them in the
///   record's two `Conditions` groups, not the first into the PlanetContentProperties
///   layout's).
/// - Signatures of several layouts (the intersection is empty): every layout stays, as in
///   [`member_order`] (a record may hold several components).
///
/// When several layouts are left (`DATA` leads six variants of the block's subrecord union,
/// ADR 0015), `component` (the `BFCB` name the fields give) picks the union variant that
/// lists it; without one, or when no variant lists it, the first layout that is not a union
/// variant (a variant without its name would not be read), else the first.
pub(crate) fn record_order(
    sig: [u8; 4],
    counts: &BTreeMap<[u8; 4], usize>,
    component: Option<&str>,
) -> Vec<Slot> {
    let all = member_order(sig);
    let mut layout: Option<Vec<String>> = None;
    for (s, n) in counts {
        let own: Vec<&Slot> = all.iter().filter(|e| e.sig == *s && !e.component).collect();
        // The record's own slots take every occurrence: one each, the last taking the
        // rest when it repeats (a third LVLI condition goes to the repeating Entries).
        if *n <= own.len() || own.last().is_some_and(|e| e.repeated) {
            continue;
        }
        let mut in_layouts: Vec<String> = Vec::new();
        for e in all.iter().filter(|e| e.sig == *s && e.component) {
            if let Some(l) = e.path.split('.').nth(1) {
                if !in_layouts.iter().any(|x| x == l) {
                    in_layouts.push(l.to_string());
                }
            }
        }
        if in_layouts.is_empty() {
            continue;
        }
        layout = Some(match layout {
            None => in_layouts,
            Some(prev) => prev
                .into_iter()
                .filter(|l| in_layouts.iter().any(|x| x == l))
                .collect(),
        });
    }
    match layout {
        None => member_order_of_layout(sig, ""),
        Some(l) => {
            // Several layouts carry every constrained signature. A union variant the `BFCB`
            // name picks wins (ADR 0015); otherwise prefer the first plain layout that one of
            // the record's signatures can start (a lead of the layout's group, as the decoder
            // enters it): a lone FLLD is the LightLayer layout's lead, while inside
            // Destructible it needs a DSTD before it (KK-289). Else `pick_layout`'s order.
            let leads_of = |name: &str| -> Vec<[u8; 4]> {
                let path = format!("{}.{name}", kitkraft_schema::COMPONENT_BLOCK);
                all.iter()
                    .flat_map(|e| e.groups.iter())
                    .find(|(p, _)| *p == path)
                    .map(|(_, leads)| leads.clone())
                    .unwrap_or_default()
            };
            let named = component.and_then(|c| {
                l.iter()
                    .find(|n| union_variant(sig, n).is_some_and(|v| v.when.iter().any(|w| w == c)))
            });
            let startable = l.iter().find(|name| {
                if union_variant(sig, name).is_some() {
                    return false;
                }
                let leads = leads_of(name);
                counts.keys().any(|s| leads.contains(s))
            });
            match named
                .or(startable)
                .or_else(|| pick_layout(sig, &l, component))
            {
                Some(one) => member_order_of_layout(sig, one),
                None => all,
            }
        }
    }
}

/// The component block's union variant named `layout` in record type `sig`, if it is one.
fn union_variant(sig: [u8; 4], layout: &str) -> Option<&'static kitkraft_schema::VariantOf> {
    let schema = kitkraft_schema::builtin().ok()?;
    let def = schema.record(kitkraft_schema::Signature::from_bytes(sig)?)?;
    def.members.iter().find_map(|m| match m {
        Member::Group(block) if block.name == kitkraft_schema::COMPONENT_BLOCK => {
            block.members.iter().find_map(|inner| match inner {
                Member::Group(g) if g.name == layout => g.variant_of.as_ref(),
                _ => None,
            })
        }
        _ => None,
    })
}

/// Which of the candidate component layouts `layouts` a record holds ([`record_order`]).
fn pick_layout<'l>(
    sig: [u8; 4],
    layouts: &'l [String],
    component: Option<&str>,
) -> Option<&'l String> {
    let named = component.and_then(|c| {
        layouts
            .iter()
            .find(|l| union_variant(sig, l).is_some_and(|v| v.when.iter().any(|w| w == c)))
    });
    named
        .or_else(|| layouts.iter().find(|l| union_variant(sig, l).is_none()))
        .or_else(|| layouts.first())
}

/// The component name the edits give the record's block (`BFCB` set to a string), if any.
pub(crate) fn component_name(edits: &[Edit]) -> Option<&str> {
    edits.iter().find_map(|e| match &e.op {
        super::fields::Op::Set(serde_json::Value::String(name))
            if e.path.signature == *b"BFCB" && e.path.steps.is_empty() =>
        {
            Some(name.as_str())
        }
        _ => None,
    })
}

fn order_in_layout(sig: [u8; 4], layout: Option<&str>) -> Vec<Slot> {
    fn walk(
        members: &[Member],
        prefix: &str,
        repeated: bool,
        groups: &[(String, Vec<[u8; 4]>)],
        layout: Option<&str>,
        out: &mut Vec<Slot>,
    ) {
        if groups.len() > MAX_GROUP_DEPTH {
            return;
        }
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
                    path: path.clone(),
                    card: s.cardinality,
                    ty: s.ty.clone(),
                    repeated: repeated || s.cardinality == Cardinality::Repeated,
                    component: kitkraft_schema::in_component_layout(&path),
                    // Innermost first.
                    groups: groups.iter().rev().cloned().collect(),
                }),
                Member::Group(g) => {
                    // A direct child of the component block that is not the layout the
                    // record holds: a different component, not a slot of this record.
                    if prefix == kitkraft_schema::COMPONENT_BLOCK
                        && layout.is_some_and(|l| l != g.name)
                    {
                        continue;
                    }
                    // A group's members are required only inside the group, which may
                    // itself be absent: never added on their own, only when the group is
                    // started ([`fill_groups`]).
                    let start = out.len();
                    let again = repeated || g.cardinality == Cardinality::Repeated;
                    let mut inner = groups.to_vec();
                    inner.push((path.clone(), leads(&g.members, groups.len())));
                    walk(&g.members, &path, again, &inner, layout, out);
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
        walk(&def.members, "", false, &[], layout, &mut out);
    }
    out
}

/// Where occurrence `k` of signature `s` goes when a record has `n` of them: the `k`-th
/// slot of the signature in [`member_order`] — the record's own members and, of the base
/// form component block, the slots of the one component layout the record holds. So the
/// `k`-th occurrence lands in the `k`-th slot of the record in schema order: one
/// occurrence takes the record's own member (before a member of the same signature inside
/// a component layout: PKIN `VNAM` is `Version`); several take the slots in schema order,
/// `k`-th to `k`-th, the last slot taking the rest when it repeats. `None` when there is
/// no slot for it.
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

/// `slot`, or, when the group around it lacks a lead ([`missing_lead`]), another slot of
/// the same signature (not a component's) whose groups all have one: a signature several
/// groups use (QUST `FNAM` is an objective's flags and a reference alias's) goes to the
/// group the record has or the fields start.
pub(crate) fn led_slot(order: &[Slot], slot: usize, present: impl Fn([u8; 4]) -> bool) -> usize {
    if missing_lead(order, slot, &present).is_none() {
        return slot;
    }
    let Some(sig) = order.get(slot).map(|e| e.sig) else {
        return slot;
    };
    order
        .iter()
        .enumerate()
        .filter(|(i, e)| e.sig == sig && !e.component && *i != slot)
        .find(|(i, _)| missing_lead(order, *i, &present).is_none())
        .map_or(slot, |(i, _)| i)
}

/// The group a subrecord added at `slot` would lack a lead for: the innermost group around
/// it (or around a group it starts) that the subrecord does not itself start and none of
/// whose leads `present` says the record has or gets. Returns the group's member path and
/// its leads. A group member written without a lead sits where the definition does not
/// place it, and the game reads it at a position no member accepts; the fields must supply
/// the lead too, which is then added first, at its own slot (Fable review of wave 3,
/// finding 1).
pub(crate) fn missing_lead(
    order: &[Slot],
    slot: usize,
    present: impl Fn([u8; 4]) -> bool,
) -> Option<(String, Vec<[u8; 4]>)> {
    let s = order.get(slot)?;
    s.groups
        .iter()
        .find(|(_, leads)| !leads.contains(&s.sig) && !leads.iter().any(|l| present(*l)))
        .cloned()
}

/// The refusal for subrecord `sub` (shown as `shown`) in group `group`, none of whose
/// `leads` the record has or the fields give.
pub(crate) fn lead_refusal(shown: &str, sub: [u8; 4], group: &str, leads: &[[u8; 4]]) -> String {
    let text = |b: &[u8; 4]| String::from_utf8_lossy(b).into_owned();
    let names: Vec<String> = leads.iter().map(text).collect();
    let lead = match names.as_slice() {
        [one] => one.clone(),
        many => format!("one of {}", many.join(", ")),
    };
    format!(
        "{shown} belongs to the group `{group}`, which starts with {lead}; the record has none and the fields do not give one, so {} would sit where the definition does not place it (give {lead} in the fields too)",
        text(&sub),
    )
}

/// The first subrecord of a record of type `record` (signatures `subs`) that sits where the
/// definition does not place it although the definition names its signature
/// (`kitkraft_doc::typed::LayoutError::Unplaced`, what `validate`'s `layout.unplaced`
/// flags), among the indexes `check` accepts.
pub(crate) fn first_unplaced<K: kitkraft_doc::typed::SubKey>(
    record: [u8; 4],
    subs: &[K],
    check: impl Fn(usize) -> bool,
) -> Option<(usize, [u8; 4])> {
    let schema = kitkraft_schema::builtin().ok()?;
    let placed = kitkraft_doc::typed::placements(schema, record, subs)?;
    subs.iter().enumerate().find_map(|(i, sub)| {
        if !check(i) {
            return None;
        }
        let sub = sub.signature();
        let unplaced = matches!(
            kitkraft_doc::typed::layout_of(schema, record, sub, i, placed.get(i)),
            Err(kitkraft_doc::typed::LayoutError::Unplaced { .. })
        );
        unplaced.then_some((i, sub))
    })
}

/// Most rounds [`fill_groups`] takes: each one can start a nested required group.
const MAX_FILL_ROUNDS: usize = MAX_GROUP_DEPTH + 2;

/// Adds to `subs` (each with its slot in `order`, whether it is being written, and the
/// subrecord, sorted by slot) every member the definition requires inside a group that
/// a subrecord being written starts or joins and that the group lacks: zeros of the
/// member's fixed size (or an empty string), at its own slot, as `new` fills the record's
/// own required members. A required group nested in it is started from its first
/// required lead, then filled the same way (Fable review of the wave 3 fix round,
/// follow-up (b): `XMRK` and `FNAM` on a REFR wrote a `MapMarker` without its `FULL`,
/// `TNAM` and `VNAM`). Returns the members added (`TNAM (MapMarker.MarkerType)`), or
/// the member and why it cannot be added: a required member with no fixed size to
/// start from, which the fields must give.
pub(crate) fn fill_groups(
    record: [u8; 4],
    order: &[Slot],
    subs: &mut Vec<(usize, bool, Subrecord<'static>)>,
) -> Result<Vec<String>, ([u8; 4], String)> {
    let mut added: Vec<String> = Vec::new();
    let Ok(schema) = kitkraft_schema::builtin() else {
        return Ok(added);
    };
    for _ in 0..MAX_FILL_ROUNDS {
        let keys: Vec<([u8; 4], &[u8])> = subs
            .iter()
            .map(|(_, _, s)| (s.signature().0, s.data()))
            .collect();
        let gaps = kitkraft_doc::typed::group_gaps(schema, record, &keys).unwrap_or_default();
        let written = |a: usize, b: usize| subs.iter().take(b).skip(a).any(|(_, w, _)| *w);
        let gaps: Vec<_> = gaps
            .into_iter()
            .filter(|g| written(g.start, g.end))
            .collect();
        if gaps.is_empty() {
            return Ok(added);
        }
        let mut more: Vec<(usize, bool, Subrecord<'static>)> = Vec::new();
        for gap in &gaps {
            for m in &gap.missing {
                let text = String::from_utf8_lossy(&m.signature).into_owned();
                let slot = order
                    .iter()
                    .position(|e| e.path == m.path && e.sig == m.signature)
                    .ok_or_else(|| {
                        (
                            m.signature,
                            format!(
                                "the group `{}` requires {text} ({}), which has no slot to add it at",
                                gap.path, m.path
                            ),
                        )
                    })?;
                let bytes = order
                    .get(slot)
                    .and_then(|e| slot_placeholder(&e.ty, false))
                    .ok_or_else(|| {
                        (
                            m.signature,
                            format!(
                                "the group `{}` is started and requires {text} ({}), whose layout gives a new subrecord nothing to start from (give {text} in the fields too, as hex:)",
                                gap.path, m.path
                            ),
                        )
                    })?;
                if !more.iter().any(|(s, _, _)| *s == slot) {
                    added.push(format!("{text} ({})", m.path));
                    more.push((slot, true, Subrecord::new(Signature(m.signature), bytes)));
                }
            }
        }
        subs.extend(more);
        // Stable: equal slots keep their order, then the added ones.
        subs.sort_by_key(|(slot, _, _)| *slot);
    }
    let keys: Vec<([u8; 4], &[u8])> = subs
        .iter()
        .map(|(_, _, s)| (s.signature().0, s.data()))
        .collect();
    match kitkraft_doc::typed::group_gaps(schema, record, &keys)
        .unwrap_or_default()
        .first()
        .and_then(|g| Some((g, g.missing.first()?)))
    {
        Some((gap, m)) => Err((
            m.signature,
            format!(
                "the group `{}` still lacks {} ({}) after its required members were added",
                gap.path,
                String::from_utf8_lossy(&m.signature),
                m.path
            ),
        )),
        None => Ok(added),
    }
}

/// The parent of a member path (`Rank.Conditions` -> `Rank`, `Conditions` -> ``).
fn parent_path(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(parent, _)| parent)
}

/// Whether the slot at `at` of `order` is a condition count: a `CITC` whose next slot is the
/// `CTDA` of a `Condition` group that follows it in the same member list (KK-271,
/// `docs/design/conditions.md`, "CITC"). STBH's `CITC`, followed by a `CTDA` member of its
/// own (no condition group), is not one.
fn counts_conditions(order: &[Slot], at: usize) -> bool {
    let (Some(citc), Some(next)) = (order.get(at), order.get(at.saturating_add(1))) else {
        return false;
    };
    citc.sig == *b"CITC"
        && next.sig == *b"CTDA"
        && parent_path(parent_path(&next.path)) == parent_path(&citc.path)
        && parent_path(&next.path) != parent_path(&citc.path)
}

/// One run of conditions in a record (KK-271): a condition count (`CITC`) the definition
/// puts before a `Condition` group, and the conditions of that group's instances right
/// after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConditionRun {
    /// Index of the `CITC` among the record's subrecords.
    pub(crate) citc: usize,
    /// How many `CTDA` follow it in its group's consecutive instances.
    pub(crate) conditions: usize,
}

/// Every condition count of a record of type `record` (signatures `subs`, in order) and the
/// conditions it counts: the `CITC` subrecords placed at a member that heads a `Condition`
/// group (`counts_conditions`), each with the number of `CTDA` placed in that group's
/// instances directly after it (`CIS1` / `CIS2` belong to the run; anything else ends it).
/// Empty for a type the schema does not define.
pub(crate) fn condition_runs(record: [u8; 4], subs: &[[u8; 4]]) -> Vec<ConditionRun> {
    let Ok(schema) = kitkraft_schema::builtin() else {
        return Vec::new();
    };
    let Some(placed) = kitkraft_doc::typed::placements(schema, record, subs) else {
        return Vec::new();
    };
    let order = member_order(record);
    let path_of = |i: usize| match placed.get(i) {
        Some(kitkraft_doc::typed::Placement::Member { path, .. }) => Some(path.as_str()),
        _ => None,
    };
    let mut out = Vec::new();
    for (i, sig) in subs.iter().enumerate() {
        if sig != b"CITC" {
            continue;
        }
        let Some(path) = path_of(i) else {
            continue;
        };
        let Some(at) = order
            .iter()
            .position(|e| e.sig == *b"CITC" && e.path == path)
        else {
            continue;
        };
        if !counts_conditions(&order, at) {
            continue;
        }
        let group = order
            .get(at.saturating_add(1))
            .map(|e| parent_path(&e.path).to_string())
            .unwrap_or_default();
        let mut conditions = 0usize;
        for (j, s) in subs.iter().enumerate().skip(i.saturating_add(1)) {
            let inside = matches!(s, b"CTDA" | b"CIS1" | b"CIS2")
                && path_of(j).is_some_and(|p| parent_path(p) == group);
            if !inside {
                break;
            }
            if s == b"CTDA" {
                conditions = conditions.saturating_add(1);
            }
        }
        out.push(ConditionRun {
            citc: i,
            conditions,
        });
    }
    out
}

/// The condition count a record of type `record` lacks for conditions being added: when
/// the record has no `CITC` at all and gets its first `CTDA` in the record's own first
/// condition group, whose definition puts a counted `CITC` right before it (ACTI, TERM,
/// HAZD, MUST, ...; KK-271). A `CITC` the definition requires inside a group is added with
/// the group (`fill_groups`).
fn condition_count_slot(order: &[Slot], have: &BTreeMap<[u8; 4], usize>) -> Option<[u8; 4]> {
    if have.get(b"CITC").copied().unwrap_or(0) > 0 || have.get(b"CTDA").copied().unwrap_or(0) > 0 {
        return None;
    }
    let first = order
        .iter()
        .position(|e| e.sig == *b"CTDA" && !e.component)?;
    let at = first.checked_sub(1)?;
    let citc = order.get(at)?;
    (counts_conditions(order, at) && citc.groups.is_empty()).then_some(*b"CITC")
}

/// Whether `edit` removes a condition: `CTDA[n] = null` (KK-271).
pub(crate) fn is_condition_removal(edit: &Edit) -> bool {
    edit.path.signature == *b"CTDA"
        && edit.path.steps.is_empty()
        && matches!(edit.op, super::fields::Op::Set(serde_json::Value::Null))
}

/// Removes the conditions `occurrences` name (`CTDA` occurrences of `record`, as the record
/// is now), each with the `CIS1` / `CIS2` right after it, highest first. Returns the
/// indices removed, ascending. A missing occurrence is a `no-subrecord` error.
pub(crate) fn remove_conditions(
    record: &mut Record<'_>,
    occurrences: &[usize],
    limits: &kitkraft_esm::DecodeLimits,
) -> Result<Vec<usize>, FieldError> {
    let mut wanted: Vec<usize> = occurrences.to_vec();
    wanted.sort_unstable();
    wanted.dedup();
    let mut gone: Vec<usize> = Vec::new();
    let sigs: Vec<[u8; 4]> = record
        .subrecords()
        .map(|s| s.iter().map(|s| s.signature().0).collect())
        .unwrap_or_default();
    let count = sigs.iter().filter(|s| *s == b"CTDA").count();
    for k in &wanted {
        let at = record
            .find_subrecord(Signature(*b"CTDA"), *k)
            .ok_or_else(|| {
                FieldError::new(
                    &format!("CTDA[{k}]"),
                    "no-subrecord",
                    format!("the record has {count} condition(s); CTDA[{k}] cannot be removed"),
                )
            })?;
        gone.push(at);
        for (j, s) in sigs.iter().enumerate().skip(at.saturating_add(1)) {
            if !matches!(s, b"CIS1" | b"CIS2") {
                break;
            }
            gone.push(j);
        }
    }
    gone.sort_unstable();
    let subs = record
        .edit_subrecords(limits)
        .map_err(|e| FieldError::new("CTDA", "payload", e.to_string()))?;
    for i in gone.iter().rev() {
        if *i < subs.len() {
            subs.remove(*i);
        }
    }
    Ok(gone)
}

/// Sets every condition count of `record` (the `CITC` of each [`condition_runs`] run) to
/// the number of conditions after it (KK-271). Returns `(index, old, new)` for each one
/// that changed; a `CITC` whose payload is not 4 bytes is left as it is.
pub(crate) fn sync_condition_counts(
    record: &mut Record<'_>,
    limits: &kitkraft_esm::DecodeLimits,
) -> Result<Vec<(usize, u32, u32)>, FieldError> {
    let sigs: Vec<[u8; 4]> = record
        .subrecords()
        .map(|s| s.iter().map(|s| s.signature().0).collect())
        .unwrap_or_default();
    let mut changed = Vec::new();
    for run in condition_runs(record.header.signature.0, &sigs) {
        let stored = record
            .subrecords()
            .and_then(|s| s.get(run.citc))
            .and_then(|s| <[u8; 4]>::try_from(s.data()).ok())
            .map(u32::from_le_bytes);
        let (Some(old), Ok(new)) = (stored, u32::try_from(run.conditions)) else {
            continue;
        };
        if old != new {
            record
                .set_subrecord(run.citc, new.to_le_bytes().to_vec(), limits)
                .map_err(|e| FieldError::new("CITC", "payload", e.to_string()))?;
            changed.push((run.citc, old, new));
        }
    }
    Ok(changed)
}

/// How many occurrences of each signature `edits` need in a record that has `have` of
/// each: the occurrences the edits name, plus the count subrecord of a whole array
/// subrecord the record lacks along with its count ([`count_slot`]), and the condition
/// count of a record that gets its first condition ([`condition_count_slot`]).
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
    let adds_condition = edits
        .iter()
        .any(|e| e.path.signature == *b"CTDA" && e.path.occurrence >= present(b"CTDA"));
    if adds_condition {
        if let Some(citc) = condition_count_slot(order, have) {
            wanted.entry(citc).or_insert(1);
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
/// A group an added subrecord starts or joins gets the members its definition requires
/// in it ([`fill_groups`]); returns those, for the caller's limitations.
pub(crate) fn add_slots(
    record: &mut Record<'static>,
    edits: &[Edit],
    fill_gaps: bool,
) -> Result<Vec<String>, FieldError> {
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
        return Ok(Vec::new());
    }
    let wanted = wanted_slots(&member_order(sig), edits, &have);
    // The record's slots: the component layout its subrecords and the added ones select,
    // or none ([`record_order`]), so the k-th occurrence of a signature lands in the
    // record's k-th slot of it, as `new` places it.
    let mut counts = have.clone();
    for (s, n) in &wanted {
        let c = counts.entry(*s).or_insert(0);
        *c = (*c).max(*n);
    }
    let order = record_order(sig, &counts, component_name(edits));
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
            let gets = |m: [u8; 4]| {
                have.get(&m).copied().unwrap_or(0) > 0 || wanted.get(&m).is_some_and(|n| *n > 0)
            };
            if let Some((group, leads)) = missing_lead(&order, slot, gets) {
                return Err(FieldError::new(
                    &shown,
                    "no-subrecord",
                    lead_refusal(&shown, *s, &group, &leads),
                ));
            }
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
        return Ok(Vec::new());
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
    // (slot, added here, subrecord)
    let mut all: Vec<(usize, bool, Subrecord<'static>)> = Vec::new();
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
        all.push((last, false, s.into_owned()));
    }
    all.extend(added.into_iter().map(|(slot, s)| (slot, true, s)));
    // Stable: equal slots keep the record's order, then the added ones.
    all.sort_by_key(|(slot, _, _)| *slot);
    // The members a group an added subrecord starts or joins requires (follow-up (b)).
    let defaulted = match fill_groups(sig, &order, &mut all) {
        Ok(d) => d,
        Err((member, why)) => {
            let shown = String::from_utf8_lossy(&member).into_owned();
            subs.extend(all.into_iter().filter(|(_, a, _)| !*a).map(|(_, _, s)| s));
            return Err(FieldError::new(&shown, "no-subrecord", why));
        }
    };
    // An added subrecord the definition would not place where it landed (its group lead
    // is elsewhere in the record): refused, and the record keeps its own subrecords.
    let keys: Vec<([u8; 4], &[u8])> = all
        .iter()
        .map(|(_, _, s)| (s.signature().0, s.data()))
        .collect();
    let fresh: Vec<bool> = all.iter().map(|(_, a, _)| *a).collect();
    if let Some((_, bad)) = first_unplaced(sig, &keys, |i| fresh.get(i).copied().unwrap_or(false)) {
        let shown = String::from_utf8_lossy(&bad).into_owned();
        subs.extend(all.into_iter().filter(|(_, a, _)| !*a).map(|(_, _, s)| s));
        return Err(FieldError::new(
            &shown,
            "no-subrecord",
            format!(
                "an added {shown} would sit where the {type_name} definition does not place it (its group lead is missing or out of order there)"
            ),
        ));
    }
    subs.extend(all.into_iter().map(|(_, _, s)| s));
    Ok(defaulted)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use std::collections::BTreeMap;

    use super::{member_order, member_order_of_layout, record_order, slot_of};

    #[test]
    fn a_record_holds_only_the_component_layout_its_occurrences_select() {
        let ctda = |order: &[super::Slot]| -> Vec<String> {
            order
                .iter()
                .filter(|e| e.sig == *b"CTDA")
                .map(|e| e.path.clone())
                .collect()
        };
        // An LVLI with a list condition and an entry condition and nothing of a layout:
        // the block holds none, so the two CTDAs take the record's own two groups.
        let two: BTreeMap<[u8; 4], usize> = [(*b"CTDA", 2)].into();
        let order = record_order(*b"LVLI", &two, None);
        assert_eq!(ctda(&order), ["Conditions.Data", "Entries.Conditions.Data"]);
        assert_eq!(
            slot_of(&order, *b"CTDA", 0, 2),
            order.iter().position(|e| e.path == "Conditions.Data")
        );
        // FLCS lives only in the ContactShadow layout: it selects that layout.
        let shadow: BTreeMap<[u8; 4], usize> = [(*b"CTDA", 2), (*b"FLCS", 1)].into();
        assert!(record_order(*b"LVLI", &shadow, None)
            .iter()
            .any(|e| e.path == "BfcbBlock.ContactShadow.ContactShadows"));
        assert_eq!(ctda(&record_order(*b"LVLI", &shadow, None)).len(), 2);
        // A third CTDA: the repeating Entries group takes the rest, so it selects no
        // layout either.
        let three: BTreeMap<[u8; 4], usize> = [(*b"CTDA", 3)].into();
        assert_eq!(
            ctda(&record_order(*b"LVLI", &three, None)),
            ["Conditions.Data", "Entries.Conditions.Data"]
        );
        // A layout signature selects its layout: ZNAM selects PlanetContentProperties,
        // whose condition then comes first.
        let pcp: BTreeMap<[u8; 4], usize> = [(*b"CTDA", 3), (*b"ZNAM", 1)].into();
        let order = record_order(*b"LVLI", &pcp, None);
        assert_eq!(ctda(&order).len(), 3);
        assert!(ctda(&order)
            .first()
            .is_some_and(|p| p.starts_with("BfcbBlock.PlanetContentProperties.")));
        // Signatures of two layouts: every layout stays, as in `member_order`.
        let both: BTreeMap<[u8; 4], usize> = [(*b"FLCS", 1), (*b"ZNAM", 1)].into();
        assert_eq!(
            record_order(*b"LVLI", &both, None).len(),
            member_order(*b"LVLI").len()
        );
    }

    #[test]
    fn a_lone_light_layer_selects_the_layout_it_can_start() {
        // FLLD sits in three layouts of the block: Destructible (inside a stage, after a
        // DSTD), LightLayer (its lead) and PlanetModel (after an optional MODL). A record
        // given only FLLD holds the first layout FLLD can start (KK-289).
        let flld: BTreeMap<[u8; 4], usize> = [(*b"FLLD", 1)].into();
        let order = record_order(*b"STDT", &flld, None);
        let paths: Vec<&str> = order
            .iter()
            .filter(|e| e.sig == *b"FLLD")
            .map(|e| e.path.as_str())
            .collect();
        assert_eq!(paths, ["BfcbBlock.LightLayer.LightLayer"]);
        // With MODL the planet model is the only layout left.
        let model: BTreeMap<[u8; 4], usize> = [(*b"FLLD", 1), (*b"MODL", 1)].into();
        assert!(record_order(*b"STDT", &model, None)
            .iter()
            .any(|e| e.path == "BfcbBlock.PlanetModel.LightLayer"));
    }

    #[test]
    fn a_repeated_signature_counts_every_slot_of_a_repeating_group() {
        // A QUST's `CTDA` has one slot per condition group, in schema order: the ones in
        // the BFCB block's PlanetContentProperties and Crowd layouts first, then the nine of the
        // record itself (dialogue and story manager, each stage log entry's and
        // reward's, each objective target's, each reference alias's and location
        // alias's match conditions, a planet content request's, and a collection alias
        // template's). Every group repeats, so the k-th occurrence takes the k-th slot
        // for every k the definition has a slot for, and past the last slot the last
        // one takes the rest.
        let all = member_order(*b"QUST");
        let ctda: Vec<usize> = all
            .iter()
            .enumerate()
            .filter(|(_, e)| e.sig == *b"CTDA")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(ctda.len(), 12);
        for (k, slot) in ctda.iter().copied().enumerate() {
            assert_eq!(slot_of(&all, *b"CTDA", k, ctda.len()), Some(slot));
        }
        assert_eq!(
            slot_of(&all, *b"CTDA", ctda.len(), ctda.len() + 1),
            Some(*ctda.last().unwrap())
        );
        // A record holding the ContactShadow component: the block's condition is not a
        // slot of it, so its `CTDA`s are the record's own nine groups, k-th to k-th, and
        // past the ninth the last (repeating) one takes the rest.
        let contact_shadow = member_order_of_layout(*b"QUST", "ContactShadow");
        let own: Vec<usize> = contact_shadow
            .iter()
            .enumerate()
            .filter(|(_, e)| e.sig == *b"CTDA")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(own.len(), 9);
        for (k, slot) in own.iter().copied().enumerate() {
            assert_eq!(slot_of(&contact_shadow, *b"CTDA", k, own.len()), Some(slot));
        }
        assert_eq!(
            slot_of(&contact_shadow, *b"CTDA", own.len(), own.len() + 1),
            Some(*own.last().unwrap())
        );
        // A signature whose last slot does not repeat reports `None` past it: a QUST's
        // own `DNAM` is the only non-repeating slot after the block's activity tracker
        // argument names (since 2026-11-17) and AnimationGraph response file.
        let dnam: Vec<usize> = all
            .iter()
            .enumerate()
            .filter(|(_, e)| e.sig == *b"DNAM")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(dnam.len(), 3);
        for (k, slot) in dnam.iter().copied().enumerate() {
            assert_eq!(slot_of(&all, *b"DNAM", k, dnam.len()), Some(slot));
        }
        assert_eq!(slot_of(&all, *b"DNAM", dnam.len(), dnam.len() + 1), None);
    }
}
