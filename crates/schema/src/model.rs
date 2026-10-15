//! The validated, typed form of a definition set.
//!
//! Values of these types only come out of [`crate::load`]; every invariant listed on a
//! type has been checked by the validator by then.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A four-byte record or subrecord signature (`WEAP`, `EDID`, `NPC_`).
///
/// Invariant: four printable ASCII bytes (`0x20..=0x7E`). The field is private so the
/// invariant holds for every value; build one with [`Signature::parse`] or
/// [`Signature::from_bytes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Signature([u8; 4]);

impl Signature {
    /// Parses a signature from text. Returns `None` unless it is exactly four printable
    /// ASCII characters.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes: [u8; 4] = text.as_bytes().try_into().ok()?;
        Self::from_bytes(bytes)
    }

    /// A signature from raw bytes (as read from a file). Returns `None` unless all four
    /// bytes are printable ASCII.
    pub fn from_bytes(bytes: [u8; 4]) -> Option<Self> {
        if bytes.iter().all(|b| (0x20..=0x7E).contains(b)) {
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// The four raw bytes.
    pub fn as_bytes(&self) -> [u8; 4] {
        self.0
    }

    /// The signature as text. Always valid ASCII by the type invariant.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("????")
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How many times a member may occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// Exactly once.
    Required,
    /// Zero or one time.
    Optional,
    /// Zero or more times, consecutively.
    Repeated,
}

impl Cardinality {
    /// The TOML spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Repeated => "repeated",
        }
    }
}

/// How far a fact has been checked (ADR 0005 Amendment 2, with the owner's semantics of
/// 2026-10-14). Ordered weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// Documented in an allowed source (today: xEdit at the pinned revision), not yet
    /// observed in a fixture.
    Reference,
    /// Observed in a local fixture: sizes, counts and order agree with every occurrence
    /// in the 14 official ESMs (or in our synthetic plugins).
    Corpus,
    /// Checked against the Creation Kit's own output (differential observation).
    CkDiff,
    /// Independently validated: both `corpus` and `ck-diff`.
    Verified,
}

impl Confidence {
    /// Every level, weakest first.
    pub const ALL: [Confidence; 4] = [Self::Reference, Self::Corpus, Self::CkDiff, Self::Verified];

    /// The TOML spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Corpus => "corpus",
            Self::CkDiff => "ck-diff",
            Self::Verified => "verified",
        }
    }

    /// Parses the TOML spelling.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == text)
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a definition, member or field comes from and how far it has been checked.
///
/// Every node of the model carries its *effective* provenance: a member or field without
/// its own `source`/`evidence`/`confidence` keys inherits its parent's (the record, header
/// or shared definition file, then the enclosing member). Members expanded from a shared
/// definition keep that definition's provenance, not the referencing record's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The primary source, in an ADR 0005 form (see [`crate::SOURCE_PREFIXES`]).
    pub source: String,
    /// Further citations, same forms.
    pub evidence: Vec<String>,
    pub confidence: Confidence,
}

impl Provenance {
    /// The source followed by the evidence.
    pub fn citations(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.source.as_str()).chain(self.evidence.iter().map(String::as_str))
    }
}

/// Fixed-size little-endian scalar types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl Scalar {
    /// All scalars, for parsing and tests.
    pub const ALL: [Scalar; 10] = [
        Self::U8,
        Self::I8,
        Self::U16,
        Self::I16,
        Self::U32,
        Self::I32,
        Self::U64,
        Self::I64,
        Self::F32,
        Self::F64,
    ];

    /// The TOML spelling, which is also the Rust type name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::I8 => "i8",
            Self::U16 => "u16",
            Self::I16 => "i16",
            Self::U32 => "u32",
            Self::I32 => "i32",
            Self::U64 => "u64",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    /// Size in bytes.
    pub fn size(self) -> u32 {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
        }
    }

    /// Whether this is an integer type (enums and flags need one).
    pub fn is_integer(self) -> bool {
        !matches!(self, Self::F32 | Self::F64)
    }

    /// Whether this is a signed type.
    pub fn is_signed(self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::F32 | Self::F64
        )
    }

    /// Whether `value` is representable in this integer type.
    pub fn fits(self, value: i128) -> bool {
        let (min, max): (i128, i128) = match self {
            Self::U8 => (0, u8::MAX.into()),
            Self::I8 => (i8::MIN.into(), i8::MAX.into()),
            Self::U16 => (0, u16::MAX.into()),
            Self::I16 => (i16::MIN.into(), i16::MAX.into()),
            Self::U32 => (0, u32::MAX.into()),
            Self::I32 => (i32::MIN.into(), i32::MAX.into()),
            Self::U64 => (0, u64::MAX.into()),
            Self::I64 => (i64::MIN.into(), i64::MAX.into()),
            Self::F32 | Self::F64 => return false,
        };
        (min..=max).contains(&value)
    }

    /// Parses the TOML spelling.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == text)
    }
}

/// Element count of a [`FieldType::Array`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayCount {
    /// Exactly this many elements.
    Fixed(u32),
    /// As many elements as fit in the remaining bytes (the array must be last).
    Rest,
    /// As many elements as the value of the named earlier sibling field (an ungated
    /// integer field of the same struct).
    From(String),
}

/// One named value of an enum or one named bit mask of a flags field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedValue {
    pub name: String,
    pub value: i128,
}

/// The layout of a subrecord payload or of one field inside it.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    /// A fixed-size number.
    Scalar(Scalar),
    /// A 32-bit FormID. `targets` lists the record types it may point at; empty means
    /// "not known yet", not "any".
    FormId { targets: Vec<Signature> },
    /// Four raw bytes holding a signature (`char[4]`).
    Sig,
    /// NUL-terminated string. Reads to the end of the payload; must be last in a struct.
    ZString,
    /// Localized string: a 4-byte string-table ID when the plugin's localized flag is set,
    /// a zstring otherwise. Reads to the end of the payload; must be last in a struct.
    LString,
    /// `u16` length, then that many bytes (no terminator).
    WString,
    /// `u8` length, then that many bytes (no terminator).
    BString,
    /// An integer whose known values have names. Unknown values are preserved.
    Enum {
        base: Scalar,
        values: Vec<NamedValue>,
    },
    /// An unsigned integer of named bit masks. Unknown bits are preserved.
    Flags { base: Scalar, bits: Vec<NamedValue> },
    /// A run of identical elements.
    Array {
        element: Box<FieldType>,
        count: ArrayCount,
    },
    /// Named fields in order.
    Struct { fields: Vec<FieldDef> },
    /// Bytes we do not decode. `None` means "the rest of the payload" (must be last).
    Opaque { size: Option<u32> },
    /// A zero-length marker subrecord.
    Empty,
    /// A shared type from `defs/common/` (`type = "ref:Name"`), resolved at load time.
    Ref { name: String, ty: Box<FieldType> },
    /// A payload whose layout is chosen by `decider` (a discriminated union): a whole
    /// subrecord decided by the record's EditorID prefix (a payload no variant claims
    /// stays raw in `extra`), or a struct field decided by earlier sibling fields (a
    /// decider value no variant names keeps the field's bytes as read).
    Union {
        decider: Decider,
        variants: Vec<UnionVariant>,
    },
}

/// What selects the variant of a [`FieldType::Union`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decider {
    /// The first byte of the record's first `EDID` payload (`decider = "edid-prefix"`).
    /// Whole subrecord members only.
    EditorIdPrefix,
    /// The stored value of an earlier sibling field of the same struct
    /// (`decider = "field:<Name>"`), optionally refined by a second earlier sibling
    /// (`secondary = "field:<Name>"`) for the variants that name a `secondary_when`
    /// value. Struct fields only. The sibling fields are ungated integers or enums of at
    /// most 32 bits.
    Field {
        /// The deciding sibling field.
        name: String,
        /// The refining sibling field, if any.
        secondary: Option<String>,
    },
}

impl Decider {
    /// The TOML spelling of the decider (`edid-prefix`, `field:ValueType`); the secondary
    /// field is not included (see [`Decider::secondary`]).
    pub fn spelling(&self) -> String {
        match self {
            Self::EditorIdPrefix => "edid-prefix".to_owned(),
            Self::Field { name, .. } => format!("field:{name}"),
        }
    }

    /// The secondary sibling field of a field decider, if any.
    pub fn secondary(&self) -> Option<&str> {
        match self {
            Self::EditorIdPrefix => None,
            Self::Field { secondary, .. } => secondary.as_deref(),
        }
    }
}

impl fmt::Display for Decider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.spelling())?;
        if let Some(secondary) = self.secondary() {
            write!(f, " then field:{secondary}")?;
        }
        Ok(())
    }
}

/// One layout of a [`FieldType::Union`].
#[derive(Debug, Clone, PartialEq)]
pub struct UnionVariant {
    /// The decider values that select this layout: for `edid-prefix` one ASCII letter or
    /// digit (as its byte value), compared exactly; for a field decider one or more
    /// values of the deciding field.
    pub when: Vec<i64>,
    /// With a secondary decider: the value the secondary field must also have for this
    /// variant (`secondary_when`). A variant without one applies to its `when` values
    /// whenever no variant with a matching `secondary_when` does.
    pub secondary_when: Option<i64>,
    /// PascalCase name, unique in the union (`Other` is reserved for the raw fallback).
    pub name: String,
    pub ty: FieldType,
    pub note: Option<String>,
}

impl UnionVariant {
    /// The EditorID prefix byte of an `edid-prefix` variant.
    pub fn prefix(&self) -> Option<u8> {
        match self.when.as_slice() {
            [one] => u8::try_from(*one).ok(),
            _ => None,
        }
    }
}

/// The variant of a union that decider value `key` (and, for a union with a secondary
/// decider, `secondary`) selects: a variant naming `key` and that `secondary` value
/// first, else a variant naming `key` without a secondary value. `None` when no variant
/// names `key` (the bytes stay raw).
pub fn select_variant(
    variants: &[UnionVariant],
    key: i64,
    secondary: Option<i64>,
) -> Option<&UnionVariant> {
    let names = |v: &&UnionVariant| v.when.contains(&key);
    variants
        .iter()
        .filter(names)
        .find(|v| v.secondary_when.is_some() && v.secondary_when == secondary)
        .or_else(|| {
            variants
                .iter()
                .filter(names)
                .find(|v| v.secondary_when.is_none())
        })
}

impl FieldType {
    /// The size in bytes of this type for a record of `form_version`, or `None` when it
    /// is variable. Fields gated by `since`/`until` count only when `form_version` is in
    /// range; with `form_version == None` every gated field counts.
    pub fn fixed_size(&self, form_version: Option<u16>) -> Option<u32> {
        match self {
            Self::Scalar(s) => Some(s.size()),
            Self::FormId { .. } | Self::Sig => Some(4),
            Self::ZString | Self::LString | Self::WString | Self::BString => None,
            Self::Enum { base, .. } | Self::Flags { base, .. } => Some(base.size()),
            Self::Array { element, count } => match count {
                ArrayCount::Fixed(n) => element.fixed_size(form_version)?.checked_mul(*n),
                ArrayCount::Rest | ArrayCount::From(_) => None,
            },
            Self::Struct { fields } => {
                let mut total: u32 = 0;
                for field in fields {
                    if !field.present_in(form_version) {
                        continue;
                    }
                    if field.if_remaining {
                        return None;
                    }
                    total = total.checked_add(field.ty.fixed_size(form_version)?)?;
                }
                Some(total)
            }
            Self::Opaque { size } => *size,
            Self::Empty => Some(0),
            Self::Ref { ty, .. } => ty.fixed_size(form_version),
            Self::Union { variants, .. } => {
                let mut sizes = variants.iter().map(|v| v.ty.fixed_size(form_version));
                let first = sizes.next()??;
                sizes.all(|s| s == Some(first)).then_some(first)
            }
        }
    }

    /// Whether reading this type consumes everything up to the end of the payload, so
    /// nothing can follow it. Variable types that carry their own length (`wstring`,
    /// `bstring`, `count_from` arrays) do not.
    pub fn reads_to_end(&self) -> bool {
        match self {
            Self::ZString | Self::LString => true,
            Self::Opaque { size } => size.is_none(),
            Self::Array { element, count } => {
                matches!(count, ArrayCount::Rest) || element.reads_to_end()
            }
            Self::Struct { fields } => fields
                .last()
                .is_some_and(|f| f.if_remaining || f.ty.reads_to_end()),
            Self::Ref { ty, .. } => ty.reads_to_end(),
            Self::Union { variants, .. } => variants.iter().any(|v| v.ty.reads_to_end()),
            Self::Scalar(_)
            | Self::FormId { .. }
            | Self::Sig
            | Self::WString
            | Self::BString
            | Self::Enum { .. }
            | Self::Flags { .. }
            | Self::Empty => false,
        }
    }

    /// Whether any field inside this type is gated by a form version.
    pub fn is_versioned(&self) -> bool {
        match self {
            Self::Struct { fields } => fields
                .iter()
                .any(|f| f.since.is_some() || f.until.is_some() || f.ty.is_versioned()),
            Self::Array { element, .. } => element.is_versioned(),
            Self::Ref { ty, .. } => ty.is_versioned(),
            _ => false,
        }
    }

    /// Every `since`/`until` bound used by fields inside this type.
    pub fn version_bounds(&self, out: &mut BTreeSet<(Option<u16>, Option<u16>)>) {
        match self {
            Self::Struct { fields } => {
                for f in fields {
                    if f.since.is_some() || f.until.is_some() {
                        out.insert((f.since, f.until));
                    }
                    f.ty.version_bounds(out);
                }
            }
            Self::Array { element, .. } => element.version_bounds(out),
            Self::Ref { ty, .. } => ty.version_bounds(out),
            _ => {}
        }
    }

    /// Whether this type is fully undecoded bytes.
    pub fn is_opaque(&self) -> bool {
        matches!(self, Self::Opaque { .. })
    }
}

/// A named field inside a struct.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    /// PascalCase name; part of the public query vocabulary (`WFIR.ShotsPerSecond`).
    pub name: String,
    pub ty: FieldType,
    /// Present only when the record's form version is at least this.
    pub since: Option<u16>,
    /// Present only when the record's form version is at most this.
    pub until: Option<u16>,
    /// Present only when bytes remain in the payload when the field is reached (a
    /// trailing field that some occurrences omit). Always the last field.
    pub if_remaining: bool,
    pub note: Option<String>,
    /// Effective provenance (own keys, else the enclosing member's or definition's).
    pub provenance: Provenance,
}

impl FieldDef {
    /// Whether this field is present for `form_version` (`None`: assume present).
    pub fn present_in(&self, form_version: Option<u16>) -> bool {
        match form_version {
            None => true,
            Some(v) => self.since.is_none_or(|s| v >= s) && self.until.is_none_or(|u| v <= u),
        }
    }
}

/// An expected payload size for a range of form versions (inclusive bounds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionedSize {
    pub min: Option<u16>,
    pub max: Option<u16>,
    pub size: u32,
}

/// One subrecord slot in a record or group.
#[derive(Debug, Clone, PartialEq)]
pub struct SubrecordDef {
    pub signature: Signature,
    /// PascalCase name, unique within its record or group.
    pub name: String,
    pub cardinality: Cardinality,
    pub ty: FieldType,
    /// Declared payload size, checked against the layout when the layout is fixed.
    pub size: Option<u32>,
    /// Declared sizes per form-version range, for layouts that change between versions.
    pub size_by_version: Vec<VersionedSize>,
    pub note: Option<String>,
    /// Effective provenance (own keys, else the enclosing group's or definition's).
    pub provenance: Provenance,
}

/// A run of consecutive subrecords that belong together (a master and its size, a
/// BFCB..BFCE block, an OBTE..STOP block).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupDef {
    pub name: String,
    pub cardinality: Cardinality,
    pub members: Vec<Member>,
    /// The `defs/common/` group this was expanded from (`group_ref`), if any. Codegen
    /// emits a shared group's type once, under this name.
    pub shared: Option<String>,
    pub note: Option<String>,
    /// Effective provenance of the group's place in its list (own keys, else inherited).
    /// The members of a shared group keep the shared definition's provenance.
    pub provenance: Provenance,
}

/// Any number of subrecords of any signature, kept as raw bytes, up to (not including)
/// the first subrecord with signature `until`.
#[derive(Debug, Clone, PartialEq)]
pub struct AnyUntilDef {
    pub name: String,
    pub until: Signature,
    pub note: Option<String>,
    /// Effective provenance (own keys, else inherited).
    pub provenance: Provenance,
}

/// An entry in a record's (or group's) ordered member list.
#[derive(Debug, Clone, PartialEq)]
pub enum Member {
    Subrecord(SubrecordDef),
    Group(GroupDef),
    AnyUntil(AnyUntilDef),
}

impl Member {
    /// The member's name.
    pub fn name(&self) -> &str {
        match self {
            Self::Subrecord(s) => &s.name,
            Self::Group(g) => &g.name,
            Self::AnyUntil(a) => &a.name,
        }
    }

    /// The member's cardinality. `AnyUntil` is always repeated.
    pub fn cardinality(&self) -> Cardinality {
        match self {
            Self::Subrecord(s) => s.cardinality,
            Self::Group(g) => g.cardinality,
            Self::AnyUntil(_) => Cardinality::Repeated,
        }
    }
}

/// A record type: its signature, provenance, header flags and ordered members.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordDef {
    pub signature: Signature,
    /// PascalCase name.
    pub name: String,
    /// Where the facts come from and how far they are checked; members inherit it.
    pub provenance: Provenance,
    /// Named record-header flag bits that apply to this record type.
    pub flags: Vec<NamedValue>,
    /// Members may occur in any order (`unordered = true`; upstream marks CELL, REFR and
    /// ACHR so): the decoder matches each subrecord from its current member on and, when
    /// none accepts it there, from the first member again; encoding keeps the on-disk
    /// order of the values it decoded.
    pub unordered: bool,
    pub members: Vec<Member>,
    pub note: Option<String>,
}

impl RecordDef {
    /// Every subrecord definition, groups (shared ones included) expanded, in definition
    /// order, with its member path (`Master.FileName`). `any_until` members are not
    /// subrecord definitions and are skipped.
    pub fn leaf_subrecords(&self) -> Vec<(String, &SubrecordDef)> {
        let mut out = Vec::new();
        collect_leaves(&self.members, "", &mut out);
        out
    }
}

fn collect_leaves<'a>(
    members: &'a [Member],
    prefix: &str,
    out: &mut Vec<(String, &'a SubrecordDef)>,
) {
    for member in members {
        let path = if prefix.is_empty() {
            member.name().to_owned()
        } else {
            format!("{prefix}.{}", member.name())
        };
        match member {
            Member::Subrecord(s) => out.push((path, s)),
            Member::Group(g) => collect_leaves(&g.members, &path, out),
            Member::AnyUntil(_) => {}
        }
    }
}

/// A fixed binary header that is not made of subrecords: the `GRUP` header (keyed by its
/// signature) or the record header (`RECORD.toml`, no signature: its first field holds
/// the record's own signature).
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderDef {
    /// The fixed signature that starts the header, if it has one.
    pub signature: Option<Signature>,
    /// PascalCase name; also the generated type name.
    pub name: String,
    /// Where the facts come from and how far they are checked; fields inherit it.
    pub provenance: Provenance,
    /// Total header size in bytes; equals the sum of `fields`.
    pub size: u32,
    pub fields: Vec<FieldDef>,
    pub note: Option<String>,
}

/// What a shared definition (`defs/common/<Name>.toml`) provides.
#[derive(Debug, Clone, PartialEq)]
pub enum CommonBody {
    /// A member list, used with `group_ref = "<Name>"`.
    Group(Vec<Member>),
    /// A field type, used with `type = "ref:<Name>"`.
    Type(FieldType),
}

/// A shared group or type, defined once and referenced from records.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonDef {
    /// PascalCase name; also the generated type name.
    pub name: String,
    /// Where the facts come from and how far they are checked; members and fields
    /// inherit it wherever the definition is expanded.
    pub provenance: Provenance,
    pub body: CommonBody,
    pub note: Option<String>,
}

/// One definition file.
#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    Record(RecordDef),
    Header(HeaderDef),
    Common(CommonDef),
}

/// A validated set of definitions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schema {
    /// Record definitions by signature.
    pub records: BTreeMap<Signature, RecordDef>,
    /// Fixed binary headers by name (`GroupHeader`, `RecordHeader`).
    pub headers: BTreeMap<String, HeaderDef>,
    /// Shared groups and types by name (`defs/common/`). References are already expanded
    /// into the records; these are kept for codegen and documentation.
    pub common: BTreeMap<String, CommonDef>,
}

impl Schema {
    /// Record definitions in signature order.
    pub fn records(&self) -> impl Iterator<Item = &RecordDef> {
        self.records.values()
    }

    /// Header definitions in name order.
    pub fn headers(&self) -> impl Iterator<Item = &HeaderDef> {
        self.headers.values()
    }

    /// Shared definitions in name order.
    pub fn common(&self) -> impl Iterator<Item = &CommonDef> {
        self.common.values()
    }

    /// The record definition for `signature`, if any.
    pub fn record(&self, signature: Signature) -> Option<&RecordDef> {
        self.records.get(&signature)
    }

    /// The header definition named `name`, if any.
    pub fn header(&self, name: &str) -> Option<&HeaderDef> {
        self.headers.get(name)
    }

    /// The header that starts with the fixed signature `signature` (`GRUP`), if any.
    pub fn header_by_signature(&self, signature: Signature) -> Option<&HeaderDef> {
        self.headers
            .values()
            .find(|h| h.signature == Some(signature))
    }

    /// Subrecord signatures the definition of record type `signature` names (in any
    /// member, group or `any_until` terminator). `None` when the record type has no
    /// definition. A subrecord outside this set is unknown to the schema; one inside it
    /// may still land in `extra` when it is out of place or does not decode.
    pub fn known_signatures(&self, signature: Signature) -> Option<BTreeSet<Signature>> {
        let record = self.record(signature)?;
        let mut out = BTreeSet::new();
        collect_signatures(&record.members, &mut out);
        Some(out)
    }
}

fn collect_signatures(members: &[Member], out: &mut BTreeSet<Signature>) {
    for member in members {
        match member {
            Member::Subrecord(s) => {
                out.insert(s.signature);
            }
            Member::Group(g) => collect_signatures(&g.members, out),
            Member::AnyUntil(a) => {
                out.insert(a.until);
            }
        }
    }
}
