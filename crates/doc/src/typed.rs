//! Typed field values for `where` paths, read through the schema definitions
//! (`kitkraft_schema::builtin()`, lane B), by interpreting a subrecord's [`FieldType`]
//! over its payload.
//!
//! This is a read-only decoder for queries: the generated records in `kitkraft-esm` stay the
//! write path. A payload whose layout does not account for every byte, a value under an
//! opaque member, or a union variant no definition claims is unknown, never a guess.

use kitkraft_schema::{
    select_variant, ArrayCount, Cardinality, Decider, FieldType, Member, Scalar, Schema, Signature,
    SubrecordDef,
};

/// Most array elements decoded from one payload.
const MAX_ELEMENTS: usize = 1 << 20;

/// A decoded value.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// Integer (any width, enums and flags included).
    Int(i128),
    /// Float.
    Float(f64),
    /// A FormID as stored (resolve through the plugin's masters).
    Form(u32),
    /// Four signature bytes.
    Sig([u8; 4]),
    /// Text (zstring, wstring, bstring, inline lstring).
    Text(String),
    /// A localized string ID.
    LStringId(u32),
    /// Named fields.
    Struct(Vec<(String, Decoded)>),
    /// Elements.
    Array(Vec<Decoded>),
    /// Bytes the schema does not decode: where they are in the payload.
    Opaque {
        /// Offset in the subrecord payload.
        at: usize,
        /// Length in bytes.
        len: usize,
    },
}

/// What decoding depends on besides the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ctx {
    /// The record's form version.
    pub form_version: u16,
    /// The plugin's localized flag.
    pub localized: bool,
    /// First byte of the record's EditorID (union decider).
    pub edid_first: Option<u8>,
}

struct Cursor<'d> {
    data: &'d [u8],
    pos: usize,
}

impl<'d> Cursor<'d> {
    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    fn take(&mut self, n: usize) -> Result<&'d [u8], String> {
        let end = self.pos.checked_add(n).ok_or("size overflow")?;
        let s = self.data.get(self.pos..end).ok_or_else(|| {
            format!(
                "payload ends at {} (needed {n} more bytes)",
                self.data.len()
            )
        })?;
        self.pos = end;
        Ok(s)
    }
    fn rest(&mut self) -> &'d [u8] {
        let s = self.data.get(self.pos..).unwrap_or(&[]);
        self.pos = self.data.len();
        s
    }
}

fn le(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

fn scalar(s: Scalar, c: &mut Cursor<'_>) -> Result<Decoded, String> {
    let n = usize::try_from(s.size()).map_err(|_| "size")?;
    let b = c.take(n)?;
    let raw = le(b);
    Ok(match s {
        Scalar::U8 | Scalar::U16 | Scalar::U32 | Scalar::U64 => Decoded::Int(i128::from(raw)),
        Scalar::I8 => Decoded::Int(i128::from(raw as u8 as i8)),
        Scalar::I16 => Decoded::Int(i128::from(raw as u16 as i16)),
        Scalar::I32 => Decoded::Int(i128::from(raw as u32 as i32)),
        Scalar::I64 => Decoded::Int(i128::from(raw as i64)),
        Scalar::F32 => Decoded::Float(f64::from(f32::from_bits(raw as u32))),
        Scalar::F64 => Decoded::Float(f64::from_bits(raw)),
    })
}

fn zstring(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(bytes.get(..end).unwrap_or(bytes)).into_owned()
}

/// Where an lstring sits in a subrecord payload ([`lstring_spans`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LStringSpan {
    /// The member path inside the subrecord: empty for a whole-subrecord lstring, else
    /// `.Name`, `.Entries[2].Text`, `.Value1.Text` (union variants by name).
    pub member: String,
    /// Offset in the payload.
    pub at: usize,
    /// Length in bytes (4 in a localized plugin; the rest of the payload otherwise).
    pub len: usize,
}

/// One step of the member path a decode is at.
#[derive(Debug, Clone, Copy)]
enum PathPart<'t> {
    Member(&'t str),
    Index(usize),
}

/// What a decode records besides the value.
#[derive(Debug, Default)]
struct Trace<'t> {
    /// Payload offset of every FormID, in walk order.
    forms: Vec<usize>,
    /// Every lstring, when `track` is set.
    lstrings: Vec<LStringSpan>,
    /// The member path being decoded (kept only when `track` is set).
    path: Vec<PathPart<'t>>,
    /// Whether to keep the path and the lstrings.
    track: bool,
}

impl<'t> Trace<'t> {
    fn enter(&mut self, part: PathPart<'t>) {
        if self.track {
            self.path.push(part);
        }
    }
    fn leave(&mut self) {
        if self.track {
            self.path.pop();
        }
    }
    fn lstring(&mut self, at: usize, len: usize) {
        if !self.track {
            return;
        }
        let mut member = String::new();
        for p in &self.path {
            match p {
                PathPart::Member(m) => {
                    member.push('.');
                    member.push_str(m);
                }
                PathPart::Index(i) => member.push_str(&format!("[{i}]")),
            }
        }
        self.lstrings.push(LStringSpan { member, at, len });
    }
}

fn decode_in<'t>(
    ty: &'t FieldType,
    c: &mut Cursor<'_>,
    ctx: &Ctx,
    siblings: &[(String, Decoded)],
    trace: &mut Trace<'t>,
) -> Result<Decoded, String> {
    match ty {
        FieldType::Scalar(s) => scalar(*s, c),
        FieldType::Enum { base, .. } | FieldType::Flags { base, .. } => scalar(*base, c),
        FieldType::FormId { .. } => {
            trace.forms.push(c.pos);
            Ok(Decoded::Form(le(c.take(4)?) as u32))
        }
        FieldType::Sig => {
            let b = c.take(4)?;
            Ok(Decoded::Sig(b.try_into().map_err(|_| "sig")?))
        }
        FieldType::ZString => Ok(Decoded::Text(zstring(c.rest()))),
        FieldType::LString => {
            let at = c.pos;
            if ctx.localized {
                let id = le(c.take(4)?) as u32;
                trace.lstring(at, 4);
                Ok(Decoded::LStringId(id))
            } else {
                let rest = c.rest();
                trace.lstring(at, rest.len());
                Ok(Decoded::Text(zstring(rest)))
            }
        }
        FieldType::WString => {
            let n = usize::try_from(le(c.take(2)?)).map_err(|_| "length")?;
            Ok(Decoded::Text(
                String::from_utf8_lossy(c.take(n)?).into_owned(),
            ))
        }
        FieldType::BString => {
            let n = usize::try_from(le(c.take(1)?)).map_err(|_| "length")?;
            Ok(Decoded::Text(
                String::from_utf8_lossy(c.take(n)?).into_owned(),
            ))
        }
        FieldType::Opaque { size } => {
            let at = c.pos;
            let len = match size {
                Some(n) => c.take(usize::try_from(*n).map_err(|_| "size")?)?.len(),
                None => c.rest().len(),
            };
            Ok(Decoded::Opaque { at, len })
        }
        FieldType::Empty => Ok(Decoded::Struct(Vec::new())),
        FieldType::Ref { ty, .. } => decode_in(ty, c, ctx, siblings, trace),
        FieldType::Union {
            decider: Decider::EditorIdPrefix,
            variants,
        } => match ctx
            .edid_first
            .and_then(|b| select_variant(variants, i64::from(b), None))
        {
            Some(v) => decode_in(&v.ty, c, ctx, siblings, trace),
            None => {
                let at = c.pos;
                let len = c.rest().len();
                Ok(Decoded::Opaque { at, len })
            }
        },
        FieldType::Union {
            decider: Decider::Field { name, secondary },
            variants,
        } => {
            // A struct field decided by earlier siblings: the value is a one-member struct
            // named after the variant (`Value1.Float`); a key no variant names is opaque.
            let int = |field: &str| {
                siblings
                    .iter()
                    .find(|(k, _)| k == field)
                    .and_then(|(_, v)| match v {
                        Decoded::Int(i) => i64::try_from(*i).ok(),
                        _ => None,
                    })
            };
            let key = int(name).ok_or_else(|| format!("union decider `{name}` not decoded"))?;
            let second = match secondary {
                Some(s) => Some(int(s).ok_or_else(|| format!("union decider `{s}` not decoded"))?),
                None => None,
            };
            match select_variant(variants, key, second) {
                Some(v) => {
                    trace.enter(PathPart::Member(&v.name));
                    let value = decode_in(&v.ty, c, ctx, siblings, trace);
                    trace.leave();
                    Ok(Decoded::Struct(vec![(v.name.clone(), value?)]))
                }
                None => {
                    let size = ty
                        .fixed_size(Some(ctx.form_version))
                        .and_then(|s| usize::try_from(s).ok())
                        .ok_or("a field union without a fixed size")?;
                    let at = c.pos;
                    c.take(size)?;
                    Ok(Decoded::Opaque { at, len: size })
                }
            }
        }
        FieldType::Array { element, count } => {
            let n = match count {
                ArrayCount::Fixed(n) => Some(usize::try_from(*n).map_err(|_| "count")?),
                ArrayCount::Rest => None,
                ArrayCount::From(name) => {
                    let v = siblings
                        .iter()
                        .find(|(k, _)| k == name)
                        .map(|(_, v)| v)
                        .ok_or_else(|| format!("count field `{name}` not decoded"))?;
                    match v {
                        Decoded::Int(i) => Some(usize::try_from(*i).map_err(|_| "count")?),
                        _ => return Err(format!("count field `{name}` is not an integer")),
                    }
                }
            };
            let mut out = Vec::new();
            loop {
                match n {
                    Some(n) if out.len() >= n => break,
                    None if c.remaining() == 0 => break,
                    _ => {}
                }
                if out.len() >= MAX_ELEMENTS || out.len() > c.data.len() {
                    return Err("too many array elements".into());
                }
                let before = c.pos;
                trace.enter(PathPart::Index(out.len()));
                let v = decode_in(element, c, ctx, &[], trace);
                trace.leave();
                out.push(v?);
                if n.is_none() && c.pos == before {
                    break;
                }
            }
            Ok(Decoded::Array(out))
        }
        FieldType::Struct { fields } => {
            let mut out: Vec<(String, Decoded)> = Vec::new();
            for f in fields {
                if !f.present_in(Some(ctx.form_version)) {
                    continue;
                }
                if f.if_remaining && c.remaining() == 0 {
                    continue;
                }
                trace.enter(PathPart::Member(&f.name));
                let v = decode_in(&f.ty, c, ctx, &out, trace);
                trace.leave();
                out.push((f.name.clone(), v?));
            }
            Ok(Decoded::Struct(out))
        }
    }
}

/// Decode a whole subrecord payload; every byte must be accounted for.
pub fn decode(ty: &FieldType, data: &[u8], ctx: &Ctx) -> Result<Decoded, String> {
    decode_traced(ty, data, ctx).map(|(v, _)| v)
}

/// As [`decode`], also returning the payload offset of every [`Decoded::Form`] value, in
/// the order a depth-first walk of the result meets them (struct fields in order, array
/// elements in order).
pub fn decode_traced(
    ty: &FieldType,
    data: &[u8],
    ctx: &Ctx,
) -> Result<(Decoded, Vec<usize>), String> {
    let mut c = Cursor { data, pos: 0 };
    let mut trace = Trace::default();
    let v = decode_in(ty, &mut c, ctx, &[], &mut trace)?;
    if c.remaining() != 0 {
        return Err(format!("layout covers {} of {} bytes", c.pos, data.len()));
    }
    Ok((v, trace.forms))
}

/// Every lstring in a subrecord payload laid out as `ty`, wherever it sits (the whole
/// payload, a struct member at any position, an array element, a union variant), in the
/// order a depth-first walk meets them. The payload is decoded as [`decode`] does, so a
/// localized lstring is 4 bytes and an inline one runs to the end of the payload (`ctx`'s
/// `localized`). `Err` when the payload does not decode.
pub fn lstring_spans(ty: &FieldType, data: &[u8], ctx: &Ctx) -> Result<Vec<LStringSpan>, String> {
    let mut c = Cursor { data, pos: 0 };
    let mut trace = Trace {
        track: true,
        ..Trace::default()
    };
    decode_in(ty, &mut c, ctx, &[], &mut trace)?;
    if c.remaining() != 0 {
        return Err(format!("layout covers {} of {} bytes", c.pos, data.len()));
    }
    Ok(trace.lstrings)
}

/// Where one subrecord of a record sits in its record type's definition, found by its
/// position among the record's subrecords (the generated decoder's own matching rules,
/// `generated::records::match_members`, over the same definition), not by its signature
/// alone: a signature can mean different members at different positions (PNDT has CNAM,
/// FNAM and GNAM twice with different layouts; the BFCB..BFCE component blocks of PCBN,
/// PKIN and GBFM hold NAM1..NAM5, VNAM, CNAM and ANAM that are not the top-level members
/// of the same signature).
#[derive(Debug, Clone, PartialEq)]
pub enum Placement<'s> {
    /// A subrecord member: its member path (`ObjectBounds.Obnd`) and definition.
    Member {
        /// Member path, group names included.
        path: String,
        /// The definition.
        def: &'s SubrecordDef,
    },
    /// Inside the body of an `any_until` member, which the schema keeps as raw bytes.
    Raw {
        /// The `any_until` member's path.
        path: String,
    },
    /// No member accepts the subrecord at its position.
    Unplaced,
}

/// Why a subrecord has no layout to decode with; its payload is opaque to the reader.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The subrecord is inside an `any_until` body: the schema keeps it raw there, whatever
    /// the same signature means elsewhere in the record.
    #[error(
        "{sub} (subrecord {index}) is inside `{path}`, which the {record} definition keeps as raw bytes"
    )]
    Raw {
        /// Record signature.
        record: String,
        /// Subrecord signature.
        sub: String,
        /// Index among the record's subrecords.
        index: usize,
        /// The `any_until` member's path.
        path: String,
    },
    /// The definition names the signature, but no member accepts the subrecord at its
    /// position: its layout is unknown.
    #[error(
        "{sub} (subrecord {index}) is not where the {record} definition places it; its layout is unknown"
    )]
    Unplaced {
        /// Record signature.
        record: String,
        /// Subrecord signature.
        sub: String,
        /// Index among the record's subrecords.
        index: usize,
    },
    /// The occurrences a path names sit at members with different layouts.
    #[error(
        "{sub} has more than one layout in {record} ({}); the path is ambiguous: name one occurrence, e.g. {sub}[0]",
        paths.join(", ")
    )]
    Ambiguous {
        /// Record signature.
        record: String,
        /// Subrecord signature.
        sub: String,
        /// The member paths of the different layouts.
        paths: Vec<String>,
    },
}

/// A subrecord's layout: its type, its member name and its member path.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout<'s> {
    /// The payload type.
    pub ty: &'s FieldType,
    /// The member's own name (a path may start with it: `KSIZ.Count`).
    pub name: &'s str,
    /// The member path, group names included.
    pub path: String,
}

/// A subrecord's layout, or why it has none ([`layout_of`]).
pub type LayoutResult<'s> = Result<Option<Layout<'s>>, LayoutError>;

/// Most nested groups the matcher descends (the generated decoder's limit).
const MAX_GROUP_DEPTH: usize = 16;

fn accepts(member: &Member, sig: Option<Signature>, depth: usize) -> bool {
    match member {
        Member::Subrecord(s) => Some(s.signature) == sig,
        Member::AnyUntil(a) => Some(a.until) != sig,
        Member::Group(g) => {
            depth < MAX_GROUP_DEPTH && starts(&g.members, sig, depth.saturating_add(1))
        }
    }
}

fn starts(members: &[Member], sig: Option<Signature>, depth: usize) -> bool {
    for member in members {
        if accepts(member, sig, depth) {
            return true;
        }
        let required = match member {
            Member::Subrecord(s) => s.cardinality == Cardinality::Required,
            Member::Group(g) => g.cardinality == Cardinality::Required,
            Member::AnyUntil(_) => false,
        };
        if required {
            return false;
        }
    }
    false
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}.{name}")
    }
}

/// A member a started group instance lacks although the group's definition requires it
/// ([`group_gaps`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingMember {
    /// The member path of the subrecord to add: the missing member's own, or for a missing
    /// nested group the first subrecord that starts it (its first required lead).
    pub path: String,
    /// That subrecord's signature.
    pub signature: [u8; 4],
}

/// A group instance in a record (started by one of its leads) that lacks a member its
/// definition requires: the game reads the group from its lead, and a required member it
/// lacks is read from whatever follows, or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupGap {
    /// The group's member path (`MapMarker`).
    pub path: String,
    /// Index of the instance's first subrecord among the record's subrecords.
    pub start: usize,
    /// One past the index of its last subrecord.
    pub end: usize,
    /// The required members it lacks, in definition order.
    pub missing: Vec<MissingMember>,
}

/// The subrecord that starts a group of `members` at `prefix`: its first required
/// subrecord reached through its leads (a nested group's included), else its first
/// subrecord. `None` for a group with no subrecord member.
fn starter(members: &[Member], prefix: &str, depth: usize) -> Option<MissingMember> {
    if depth > MAX_GROUP_DEPTH {
        return None;
    }
    let mut first = None;
    for m in members {
        let path = join(prefix, m.name());
        let (found, required) = match m {
            Member::Subrecord(s) => (
                Some(MissingMember {
                    path,
                    signature: s.signature.as_bytes(),
                }),
                s.cardinality == Cardinality::Required,
            ),
            Member::Group(g) => (
                starter(&g.members, &path, depth.saturating_add(1)),
                g.cardinality == Cardinality::Required,
            ),
            Member::AnyUntil(_) => (None, false),
        };
        if required && found.is_some() {
            return found;
        }
        if first.is_none() {
            first = found;
        }
    }
    first
}

/// The subrecord signatures being placed, and where each one went.
struct Matcher<'s, 'a> {
    sigs: &'a [Option<Signature>],
    out: Vec<Placement<'s>>,
    /// Group instances lacking a required member, as the matcher closes them.
    gaps: Vec<GroupGap>,
}

impl<'s> Matcher<'s, '_> {
    /// One step (the generated `match_one`, over the schema model): places the subrecord at
    /// `pos`, and for a group the run it starts. Returns the index among `members` of the
    /// member it matched, `None` when none did.
    fn one(
        &mut self,
        pos: &mut usize,
        members: &'s [Member],
        cursor: &mut usize,
        depth: usize,
        prefix: &str,
    ) -> Option<usize> {
        let sig = self.sigs.get(*pos).copied()?;
        let (index, member) = members
            .iter()
            .enumerate()
            .skip(*cursor)
            .find(|(_, m)| accepts(m, sig, depth))?;
        *cursor = if member.cardinality() == Cardinality::Repeated {
            index
        } else {
            index.saturating_add(1)
        };
        let path = join(prefix, member.name());
        let placement = match member {
            Member::Group(g) => {
                let start = *pos;
                let mut inner = 0;
                let mut hit: Vec<usize> = Vec::new();
                while let Some(i) =
                    self.one(pos, &g.members, &mut inner, depth.saturating_add(1), &path)
                {
                    hit.push(i);
                }
                if *pos == start {
                    return None;
                }
                let missing: Vec<MissingMember> = g
                    .members
                    .iter()
                    .enumerate()
                    .filter(|(i, m)| m.cardinality() == Cardinality::Required && !hit.contains(i))
                    .filter_map(|(_, m)| match m {
                        Member::Subrecord(s) => Some(MissingMember {
                            path: join(&path, &s.name),
                            signature: s.signature.as_bytes(),
                        }),
                        Member::Group(inner) => starter(
                            &inner.members,
                            &join(&path, &inner.name),
                            depth.saturating_add(1),
                        ),
                        Member::AnyUntil(_) => None,
                    })
                    .collect();
                if !missing.is_empty() {
                    self.gaps.push(GroupGap {
                        path,
                        start,
                        end: *pos,
                        missing,
                    });
                }
                return Some(index);
            }
            Member::Subrecord(def) => Placement::Member { path, def },
            Member::AnyUntil(_) => Placement::Raw { path },
        };
        if let Some(slot) = self.out.get_mut(*pos) {
            *slot = placement;
        }
        *pos = pos.saturating_add(1);
        Some(index)
    }
}

/// Places every subrecord of a record of type `record` (its subrecord signatures, in
/// order) in the definition, as the generated decoder does (`match_members`, or
/// `match_unordered` for a record type marked `unordered`). `None` when the schema does
/// not define the record type.
pub fn placements<'s>(
    schema: &'s Schema,
    record: [u8; 4],
    subs: &[[u8; 4]],
) -> Option<Vec<Placement<'s>>> {
    matched(schema, record, subs).map(|(out, _)| out)
}

/// Every group instance in a record of type `record` (its subrecord signatures, in order)
/// that lacks a member its definition requires, as [`placements`] places the subrecords:
/// a group started by one of its leads (a REFR `XMRK`) whose required members (`FNAM`,
/// `FULL`, `TNAM`, `VNAM` of `MapMarker`) are not all there. A missing required group
/// nested in one is named by the subrecord that would start it. `None` when the schema
/// does not define the record type.
pub fn group_gaps(schema: &Schema, record: [u8; 4], subs: &[[u8; 4]]) -> Option<Vec<GroupGap>> {
    matched(schema, record, subs).map(|(_, gaps)| gaps)
}

/// The matcher's run over a record: where each subrecord went, and the incomplete group
/// instances.
fn matched<'s>(
    schema: &'s Schema,
    record: [u8; 4],
    subs: &[[u8; 4]],
) -> Option<(Vec<Placement<'s>>, Vec<GroupGap>)> {
    let def = schema.records.get(&Signature::from_bytes(record)?)?;
    let sigs: Vec<Option<Signature>> = subs.iter().map(|s| Signature::from_bytes(*s)).collect();
    let mut m = Matcher {
        sigs: &sigs,
        out: vec![Placement::Unplaced; sigs.len()],
        gaps: Vec::new(),
    };
    let (mut pos, mut cursor) = (0, 0);
    while pos < sigs.len() {
        let before = pos;
        let mut placed = m.one(&mut pos, &def.members, &mut cursor, 0, "").is_some();
        // An unordered record (CELL, REFR, ACHR): when no member from the cursor on accepts
        // the subrecord, match from the first member again (the generated
        // `match_unordered`).
        if !placed && def.unordered {
            let mut wrapped = 0;
            placed = m.one(&mut pos, &def.members, &mut wrapped, 0, "").is_some();
            if placed {
                cursor = wrapped;
            }
        }
        if !placed || pos == before {
            pos = before.saturating_add(1);
        }
    }
    Some((m.out, m.gaps))
}

/// Whether the definition of `record` names subrecord signature `sub` anywhere.
fn names(schema: &Schema, record: [u8; 4], sub: [u8; 4]) -> bool {
    let (Some(rs), Some(ss)) = (Signature::from_bytes(record), Signature::from_bytes(sub)) else {
        return false;
    };
    schema
        .records
        .get(&rs)
        .is_some_and(|def| def.leaf_subrecords().iter().any(|(_, s)| s.signature == ss))
}

/// The layout of one placed subrecord (`placement` from [`placements`]; `None` when the
/// record type has no definition): `Ok(None)` when the schema knows nothing about it (no
/// record definition, or a signature the definition never names), an error when it sits
/// where the schema keeps raw bytes or where no member accepts it.
pub fn layout_of<'s>(
    schema: &'s Schema,
    record: [u8; 4],
    sub: [u8; 4],
    index: usize,
    placement: Option<&Placement<'s>>,
) -> Result<Option<Layout<'s>>, LayoutError> {
    let text = |b: [u8; 4]| String::from_utf8_lossy(&b).into_owned();
    match placement {
        None => Ok(None),
        Some(Placement::Member { path, def }) => Ok(Some(Layout {
            ty: &def.ty,
            name: def.name.as_str(),
            path: path.clone(),
        })),
        Some(Placement::Raw { path }) => Err(LayoutError::Raw {
            record: text(record),
            sub: text(sub),
            index,
            path: path.clone(),
        }),
        Some(Placement::Unplaced) if !names(schema, record, sub) => Ok(None),
        Some(Placement::Unplaced) => Err(LayoutError::Unplaced {
            record: text(record),
            sub: text(sub),
            index,
        }),
    }
}

/// The layout of subrecord `index` of a record of type `record` whose subrecord
/// signatures are `subs`, resolved by its position ([`placements`], [`layout_of`]).
pub fn layout_at<'s>(
    schema: &'s Schema,
    record: [u8; 4],
    subs: &[[u8; 4]],
    index: usize,
) -> Result<Option<Layout<'s>>, LayoutError> {
    let Some(sub) = subs.get(index).copied() else {
        return Ok(None);
    };
    let placed = placements(schema, record, subs);
    let placement = placed.as_ref().and_then(|p| p.get(index));
    layout_of(schema, record, sub, index, placement)
}

/// The layout a signature path (`CNAM`, `CNAM[1]`) names, and the subrecords it selects.
/// `indexes` are the positions of the occurrences the path names (all of one signature).
/// Without an explicit occurrence (`explicit` false), occurrences inside raw `any_until`
/// bodies are not the member the path names and are left out. `Ok(None)` when the schema
/// knows nothing about the signature; [`LayoutError::Ambiguous`] when the occurrences sit
/// at members with different layouts.
pub fn path_layout<'s>(
    schema: &'s Schema,
    record: [u8; 4],
    subs: &[[u8; 4]],
    indexes: &[usize],
    explicit: bool,
) -> Result<Option<(Layout<'s>, Vec<usize>)>, LayoutError> {
    let placed = placements(schema, record, subs);
    let mut layouts: Vec<Layout<'s>> = Vec::new();
    let mut chosen: Vec<usize> = Vec::new();
    let mut first_raw: Option<LayoutError> = None;
    let mut sub = [0u8; 4];
    for &index in indexes {
        let Some(s) = subs.get(index).copied() else {
            continue;
        };
        sub = s;
        let placement = placed.as_ref().and_then(|p| p.get(index));
        match layout_of(schema, record, s, index, placement) {
            Ok(None) => return Ok(None),
            Ok(Some(l)) => {
                if !layouts.iter().any(|k| k.ty == l.ty) {
                    layouts.push(l);
                }
                chosen.push(index);
            }
            Err(e @ LayoutError::Raw { .. }) if !explicit => {
                first_raw.get_or_insert(e);
            }
            Err(e) => return Err(e),
        }
    }
    if layouts.len() > 1 {
        return Err(LayoutError::Ambiguous {
            record: String::from_utf8_lossy(&record).into_owned(),
            sub: String::from_utf8_lossy(&sub).into_owned(),
            paths: layouts.into_iter().map(|l| l.path).collect(),
        });
    }
    match (layouts.pop(), first_raw) {
        (Some(l), _) => Ok(Some((l, chosen))),
        (None, Some(e)) => Err(e),
        (None, None) => Ok(None),
    }
}

/// Walk `members` (lower-case) into `value`; arrays fan out. Returns the leaf values or why
/// the path is unknown.
pub fn select(value: &Decoded, members: &[String], label: &str) -> Result<Vec<Decoded>, String> {
    let mut current = vec![value.clone()];
    for m in members {
        let mut next = Vec::new();
        for v in current {
            collect(v, m, label, &mut next)?;
        }
        current = next;
    }
    // Arrays at the leaf fan out into their elements.
    let mut out = Vec::new();
    for v in current {
        match v {
            Decoded::Array(items) => out.extend(items),
            other => out.push(other),
        }
    }
    Ok(out)
}

fn collect(v: Decoded, member: &str, label: &str, out: &mut Vec<Decoded>) -> Result<(), String> {
    match v {
        Decoded::Struct(fields) => {
            let names: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
            match fields
                .into_iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(member))
            {
                Some((_, v)) => {
                    out.push(v);
                    Ok(())
                }
                None => Err(format!(
                    "{label} has no member `{member}` (members: {})",
                    names.join(", ")
                )),
            }
        }
        Decoded::Array(items) => {
            for i in items {
                collect(i, member, label, out)?;
            }
            Ok(())
        }
        Decoded::Opaque { .. } => Err(format!(
            "{label}: `{member}` is inside bytes the schema keeps opaque"
        )),
        _ => Err(format!(
            "{label}: `{member}` is not a member of a scalar value"
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx {
            form_version: 582,
            localized: true,
            edid_first: Some(b'f'),
        }
    }

    #[test]
    fn builtin_gmst_union_and_keywords() {
        let schema = kitkraft_schema::builtin().unwrap();
        // GMST DATA is a union chosen by the EditorID's first letter: `f` is an f32.
        let layout = layout_at(schema, *b"GMST", &[*b"EDID", *b"DATA"], 1)
            .unwrap()
            .unwrap();
        assert_eq!((layout.name, layout.path.as_str()), ("Value", "Value"));
        let ty = layout.ty;
        let v = decode(ty, &2.5f32.to_le_bytes(), &ctx()).unwrap();
        assert_eq!(select(&v, &[], "GMST.DATA").unwrap(), [Decoded::Float(2.5)]);
        // An unknown prefix keeps the payload opaque.
        let other = Ctx {
            edid_first: Some(b'#'),
            ..ctx()
        };
        assert_eq!(
            decode(ty, &[1, 2, 3, 4], &other).unwrap(),
            Decoded::Opaque { at: 0, len: 4 }
        );
        // A record type without a definition, and a subrecord the definition lacks.
        assert!(layout_at(schema, *b"QQQQ", &[*b"DATA"], 0)
            .unwrap()
            .is_none());
        assert!(layout_at(schema, *b"GMST", &[*b"EDID", *b"ZZZZ"], 1)
            .unwrap()
            .is_none());
        // Out of range is no layout, never a panic.
        assert!(layout_at(schema, *b"GMST", &[*b"EDID"], 5)
            .unwrap()
            .is_none());
    }

    /// Lane B round 4: an object mod property's FunctionType, Value1 and Value2 are
    /// struct-field unions decided by its ValueType (and Value1 by its Property for enum
    /// values); each decodes as a one-member struct named after its variant, and a value
    /// type no variant names is opaque.
    #[test]
    fn omod_property_values_follow_the_value_type() {
        let schema = kitkraft_schema::builtin().unwrap();
        let kitkraft_schema::CommonBody::Type(ty) = &schema.common["ObjectModProperty"].body else {
            panic!("type");
        };
        let entry = |vt: u8, prop: &[u8; 4], v1: [u8; 4], v2: [u8; 4]| {
            let mut out = vec![vt, 0, 0, 0, 1, 0, 0, 0];
            out.extend_from_slice(prop);
            out.extend_from_slice(&v1);
            out.extend_from_slice(&v2);
            out.extend_from_slice(&0.5f32.to_le_bytes());
            out
        };
        let pick = |data: &[u8], path: &[&str]| {
            let v = decode(ty, data, &ctx()).unwrap();
            let members: Vec<String> = path.iter().map(|s| s.to_string()).collect();
            select(&v, &members, "OMOD.DATA")
        };
        let float = entry(1, b"WSPS", 1.5f32.to_le_bytes(), 2.0f32.to_le_bytes());
        assert_eq!(
            pick(&float, &["Value1", "Float"]).unwrap(),
            [Decoded::Float(1.5)]
        );
        assert_eq!(
            pick(&float, &["value2", "float"]).unwrap(),
            [Decoded::Float(2.0)]
        );
        assert_eq!(
            pick(&float, &["FunctionType", "Numeric"]).unwrap(),
            [Decoded::Int(1)]
        );
        // The wrong variant is not a member of this entry.
        let err = pick(&float, &["Value1", "Int"]).unwrap_err();
        assert!(err.contains("members: Float"), "{err}");
        let form = entry(4, b"WKWD", 0x0002_1234u32.to_le_bytes(), 5i32.to_le_bytes());
        assert_eq!(
            pick(&form, &["Value1", "FormId"]).unwrap(),
            [Decoded::Form(0x0002_1234)]
        );
        assert_eq!(pick(&form, &["Value2", "Int"]).unwrap(), [Decoded::Int(5)]);
        let level = entry(5, b"WSLV", 2u32.to_le_bytes(), [0; 4]);
        assert_eq!(
            pick(&level, &["Value1", "SoundLevel"]).unwrap(),
            [Decoded::Int(2)]
        );
        // Value type 9: no variant; the bytes are opaque, and the entry still decodes.
        let odd = entry(9, b"WSPS", [1, 2, 3, 4], [5, 6, 7, 8]);
        assert_eq!(
            decode(ty, &odd, &ctx())
                .map(|v| select(&v, &["value1".into()], "X").unwrap())
                .unwrap(),
            [Decoded::Opaque { at: 12, len: 4 }]
        );
    }

    /// Planet schema lane request to lane D: a VNAM inside a PKIN base form component
    /// (the 1-byte VNAM of `Starfield.esm PKIN:0000B065`) is not the top-level `Version`.
    /// Since lane D types the placement-properties component (2026-10-15) it is that
    /// component's `SavePlacement`; a component layout the block does not type stays raw.
    #[test]
    fn pkin_vnam_inside_a_component_is_its_own_member() {
        let schema = kitkraft_schema::builtin().unwrap();
        let subs = [
            *b"EDID", *b"BFCB", *b"VNAM", *b"BFCE", *b"CNAM", *b"VNAM", *b"FNAM",
        ];
        let placed = placements(schema, *b"PKIN", &subs).unwrap();
        assert!(
            matches!(&placed[2], Placement::Member { path, .. }
                if path == "BfcbBlock.PlanetContentProperties.SavePlacement"),
            "{placed:?}"
        );
        let inner = layout_at(schema, *b"PKIN", &subs, 2).unwrap().unwrap();
        assert_eq!(inner.name, "SavePlacement");
        let top = layout_at(schema, *b"PKIN", &subs, 5).unwrap().unwrap();
        assert_eq!(top.name, "Version");
        // A path naming both occurrences is ambiguous: name one.
        assert!(matches!(
            path_layout(schema, *b"PKIN", &subs, &[2, 5], false),
            Err(LayoutError::Ambiguous { .. })
        ));
        let (layout, chosen) = path_layout(schema, *b"PKIN", &subs, &[5], true)
            .unwrap()
            .unwrap();
        assert_eq!((layout.name, chosen), ("Version", vec![5]));
        // A component whose data starts with a signature no typed layout starts with is
        // kept raw, whatever follows; a path naming the raw occurrence explicitly is an
        // error, not a guess.
        let raw = [
            *b"EDID", *b"BFCB", *b"ATCP", *b"VNAM", *b"BFCE", *b"CNAM", *b"VNAM",
        ];
        let placed = placements(schema, *b"PKIN", &raw).unwrap();
        assert!(matches!(&placed[3], Placement::Raw { .. }), "{placed:?}");
        let (layout, chosen) = path_layout(schema, *b"PKIN", &raw, &[3, 6], false)
            .unwrap()
            .unwrap();
        assert_eq!((layout.name, chosen), ("Version", vec![6]));
        assert!(matches!(
            path_layout(schema, *b"PKIN", &raw, &[3], true),
            Err(LayoutError::Raw { .. })
        ));
    }

    /// PNDT has CNAM, FNAM and GNAM at the top level and again inside the body group, with
    /// different layouts: each occurrence resolves by position, and a path naming all of
    /// them is ambiguous (typed, never a panic).
    #[test]
    fn unordered_records_place_members_anywhere() {
        // REFR is `unordered` (M4.BX1): XSCL and XLYR after DATA are still their members,
        // as the generated `match_unordered` places them.
        let schema = kitkraft_schema::builtin().unwrap();
        let subs = [*b"NAME", *b"DATA", *b"XSCL", *b"XLYR"];
        let placed = placements(schema, *b"REFR", &subs).unwrap();
        let paths: Vec<String> = placed
            .iter()
            .map(|p| match p {
                Placement::Member { path, .. } => path.clone(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(paths, ["Base", "Placement", "Scale", "Layer"]);
        let scale = layout_at(schema, *b"REFR", &subs, 2).unwrap().unwrap();
        assert_eq!(scale.path, "Scale");
    }

    /// Fable review of the wave 3 fix round, follow-up (b): a REFR `MapMarker` started by
    /// its lead `XMRK` with only `FNAM` lacks the required `FULL`, `TNAM` and `VNAM`; the
    /// whole group has no gap, and a record without the group has none either.
    #[test]
    fn a_started_group_missing_required_members_is_a_gap() {
        let schema = kitkraft_schema::builtin().unwrap();
        let partial = [*b"NAME", *b"DATA", *b"XMRK", *b"FNAM"];
        let gaps = group_gaps(schema, *b"REFR", &partial).unwrap();
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        let gap = gaps.first().unwrap();
        assert_eq!((gap.path.as_str(), gap.start, gap.end), ("MapMarker", 2, 4));
        let missing: Vec<(&str, [u8; 4])> = gap
            .missing
            .iter()
            .map(|m| (m.path.as_str(), m.signature))
            .collect();
        assert_eq!(
            missing,
            [
                ("MapMarker.Name", *b"FULL"),
                ("MapMarker.MarkerType", *b"TNAM"),
                ("MapMarker.Category", *b"VNAM"),
            ]
        );
        let whole = [
            *b"NAME", *b"DATA", *b"XMRK", *b"FNAM", *b"FULL", *b"TNAM", *b"VNAM",
        ];
        assert_eq!(group_gaps(schema, *b"REFR", &whole).unwrap(), []);
        assert_eq!(
            group_gaps(schema, *b"REFR", &[*b"NAME", *b"DATA"]).unwrap(),
            []
        );
        // The placements are the matcher's as before.
        assert!(placements(schema, *b"REFR", &partial)
            .unwrap()
            .iter()
            .all(|p| matches!(p, Placement::Member { .. })));
    }

    #[test]
    fn pndt_duplicated_signatures_resolve_by_position() {
        let schema = kitkraft_schema::builtin().unwrap();
        let subs = [
            *b"EDID", *b"BFCB", *b"FULL", *b"BFCE", *b"CNAM", *b"FNAM", *b"GNAM", *b"BDST",
            *b"ANAM", *b"CNAM", *b"ENAM", *b"FNAM", *b"GNAM", *b"KNAM", *b"BDED", *b"TEMP",
            *b"DENS", *b"PHLA",
        ];
        let placed = placements(schema, *b"PNDT", &subs).unwrap();
        for (i, p) in placed.iter().enumerate() {
            if i != 2 {
                assert!(matches!(p, Placement::Member { .. }), "{i}: {p:?}");
            }
        }
        for (top, inner) in [(4, 9), (5, 11), (6, 12)] {
            let a = layout_at(schema, *b"PNDT", &subs, top).unwrap().unwrap();
            let b = layout_at(schema, *b"PNDT", &subs, inner).unwrap().unwrap();
            assert!(!a.path.contains('.'), "{}", a.path);
            assert!(b.path.starts_with("Body."), "{}", b.path);
            assert_ne!(a.ty, b.ty, "{} vs {}", a.path, b.path);
            let both = path_layout(schema, *b"PNDT", &subs, &[top, inner], false);
            match both {
                Err(LayoutError::Ambiguous { paths, .. }) => {
                    assert_eq!(paths, [a.path.clone(), b.path.clone()]);
                }
                other => panic!("{other:?}"),
            }
            let (one, chosen) = path_layout(schema, *b"PNDT", &subs, &[inner], true)
                .unwrap()
                .unwrap();
            assert_eq!((one.path, chosen), (b.path, vec![inner]));
        }
        // A CNAM where no member accepts it is unknown, not decoded with either layout.
        let odd = [*b"EDID", *b"TEMP", *b"CNAM"];
        assert!(matches!(
            layout_at(schema, *b"PNDT", &odd, 2),
            Err(LayoutError::Unplaced { index: 2, .. })
        ));
    }

    #[test]
    fn structs_arrays_and_errors() {
        use kitkraft_schema::FieldDef;
        let prov = kitkraft_schema::builtin()
            .unwrap()
            .records
            .values()
            .next()
            .unwrap()
            .provenance
            .clone();
        let field = |name: &str, ty: FieldType| FieldDef {
            name: name.into(),
            ty,
            since: None,
            until: None,
            if_remaining: false,
            note: None,
            provenance: prov.clone(),
        };
        let ty = FieldType::Struct {
            fields: vec![
                field("Count", FieldType::Scalar(Scalar::U8)),
                field(
                    "Forms",
                    FieldType::Array {
                        element: Box::new(FieldType::FormId { targets: vec![] }),
                        count: ArrayCount::From("Count".into()),
                    },
                ),
                field("Damage", FieldType::Scalar(Scalar::I16)),
                field("Rest", FieldType::Opaque { size: None }),
            ],
        };
        let mut data = vec![2u8];
        data.extend(0x0001_0002u32.to_le_bytes());
        data.extend(0x0001_0003u32.to_le_bytes());
        data.extend((-5i16).to_le_bytes());
        data.extend([9, 9]);
        let v = decode(&ty, &data, &ctx()).unwrap();
        assert_eq!(
            select(&v, &["forms".into()], "X").unwrap(),
            [Decoded::Form(0x0001_0002), Decoded::Form(0x0001_0003)]
        );
        assert_eq!(
            select(&v, &["DAMAGE".into()], "X").unwrap(),
            [Decoded::Int(-5)]
        );
        assert!(select(&v, &["rest".into(), "x".into()], "X").is_err());
        assert!(select(&v, &["nope".into()], "X")
            .unwrap_err()
            .contains("Count, Forms"));
        // Too short and too long payloads are errors.
        assert!(decode(&ty, &data[..3], &ctx()).is_err());
        let fixed = FieldType::Scalar(Scalar::U16);
        assert!(decode(&fixed, &[1, 2, 3], &ctx()).is_err());
        // A hostile count does not allocate past the payload.
        let mut bad = vec![255u8];
        bad.extend([0u8; 8]);
        assert!(decode(&ty, &bad, &ctx()).is_err());
    }

    /// M3.E4: lstrings are found wherever the layout puts them, not only as the whole
    /// payload or a struct's last member.
    #[test]
    fn lstring_spans_inside_structs_and_arrays() {
        use kitkraft_schema::FieldDef;
        let prov = kitkraft_schema::builtin()
            .unwrap()
            .records
            .values()
            .next()
            .unwrap()
            .provenance
            .clone();
        let field = |name: &str, ty: FieldType| FieldDef {
            name: name.into(),
            ty,
            since: None,
            until: None,
            if_remaining: false,
            note: None,
            provenance: prov.clone(),
        };
        let entry = FieldType::Struct {
            fields: vec![
                field("Text", FieldType::LString),
                field("Value", FieldType::Scalar(Scalar::U16)),
            ],
        };
        let ty = FieldType::Struct {
            fields: vec![
                field("Name", FieldType::LString),
                field("Count", FieldType::Scalar(Scalar::U8)),
                field(
                    "Entries",
                    FieldType::Array {
                        element: Box::new(entry),
                        count: ArrayCount::From("Count".into()),
                    },
                ),
                field("Tail", FieldType::Scalar(Scalar::U32)),
            ],
        };
        let mut data = 7u32.to_le_bytes().to_vec();
        data.push(2);
        for id in [8u32, 9] {
            data.extend(id.to_le_bytes());
            data.extend(1u16.to_le_bytes());
        }
        data.extend(0u32.to_le_bytes());
        let spans = lstring_spans(&ty, &data, &ctx()).unwrap();
        let got: Vec<(&str, usize, usize)> = spans
            .iter()
            .map(|s| (s.member.as_str(), s.at, s.len))
            .collect();
        assert_eq!(
            got,
            [
                (".Name", 0, 4),
                (".Entries[0].Text", 5, 4),
                (".Entries[1].Text", 11, 4)
            ]
        );
        // The decode is unchanged by tracing; a short payload has no spans.
        assert!(decode(&ty, &data, &ctx()).is_ok());
        assert!(lstring_spans(&ty, &data[..6], &ctx()).is_err());
        // Inline (non-localized): a whole-payload lstring runs to the end.
        let inline = Ctx {
            localized: false,
            ..ctx()
        };
        let whole = lstring_spans(&FieldType::LString, b"Hi\0", &inline).unwrap();
        assert_eq!(
            whole,
            [LStringSpan {
                member: String::new(),
                at: 0,
                len: 3
            }]
        );
    }
}
