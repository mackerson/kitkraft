//! The validated, typed form of a definition set.
//!
//! Values of these types only come out of [`crate::load`]; every invariant listed on a
//! type has been checked by the validator by then.

use std::collections::BTreeMap;
use std::fmt;

/// A four-byte record or subrecord signature (`WEAP`, `EDID`, `NPC_`).
///
/// Invariant: four printable ASCII bytes (`0x20..=0x7E`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Signature(pub [u8; 4]);

impl Signature {
    /// Parses a signature from text. Returns `None` unless it is exactly four printable
    /// ASCII characters.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes: [u8; 4] = text.as_bytes().try_into().ok()?;
        if bytes.iter().all(|b| (0x20..=0x7E).contains(b)) {
            Some(Self(bytes))
        } else {
            None
        }
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

/// How many times a member may occur at its position.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayCount {
    /// Exactly this many elements.
    Fixed(u32),
    /// As many elements as fit in the remaining bytes (the array must be last).
    Rest,
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
    /// NUL-terminated string. Variable size; must be last in a struct.
    ZString,
    /// Localized string: a 4-byte string-table ID when the plugin's localized flag is set,
    /// a zstring otherwise. Variable size; must be last in a struct.
    LString,
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
}

impl FieldType {
    /// The size in bytes of this type for a record of `form_version`, or `None` when it
    /// is variable. Fields gated by `since`/`until` count only when `form_version` is in
    /// range; with `form_version == None` every gated field counts.
    pub fn fixed_size(&self, form_version: Option<u16>) -> Option<u32> {
        match self {
            Self::Scalar(s) => Some(s.size()),
            Self::FormId { .. } | Self::Sig => Some(4),
            Self::ZString | Self::LString => None,
            Self::Enum { base, .. } | Self::Flags { base, .. } => Some(base.size()),
            Self::Array { element, count } => match count {
                ArrayCount::Fixed(n) => element.fixed_size(form_version)?.checked_mul(*n),
                ArrayCount::Rest => None,
            },
            Self::Struct { fields } => {
                let mut total: u32 = 0;
                for field in fields {
                    if !field.present_in(form_version) {
                        continue;
                    }
                    total = total.checked_add(field.ty.fixed_size(form_version)?)?;
                }
                Some(total)
            }
            Self::Opaque { size } => *size,
            Self::Empty => Some(0),
        }
    }

    /// Whether any field inside this type is gated by a form version.
    pub fn is_versioned(&self) -> bool {
        match self {
            Self::Struct { fields } => fields
                .iter()
                .any(|f| f.since.is_some() || f.until.is_some() || f.ty.is_versioned()),
            Self::Array { element, .. } => element.is_versioned(),
            _ => false,
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
    pub note: Option<String>,
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
}

/// A run of consecutive subrecords that belong together (a master and its size, an
/// object-template entry, a component block).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupDef {
    pub name: String,
    pub cardinality: Cardinality,
    pub members: Vec<Member>,
    pub note: Option<String>,
}

/// Any number of subrecords of any signature, kept as raw bytes, up to (not including)
/// the first subrecord with signature `until`.
#[derive(Debug, Clone, PartialEq)]
pub struct AnyUntilDef {
    pub name: String,
    pub until: Signature,
    pub note: Option<String>,
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
    /// Where the facts come from: `UESP: <url>` or `corpus: <note>`.
    pub source: String,
    /// Further provenance notes.
    pub evidence: Vec<String>,
    /// Named record-header flag bits that apply to this record type.
    pub flags: Vec<NamedValue>,
    pub members: Vec<Member>,
    pub note: Option<String>,
}

/// A fixed binary header that is not made of subrecords (the `GRUP` header).
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderDef {
    pub signature: Signature,
    pub name: String,
    pub source: String,
    pub evidence: Vec<String>,
    /// Total header size in bytes; equals the sum of `fields`.
    pub size: u32,
    pub fields: Vec<FieldDef>,
    pub note: Option<String>,
}

/// One definition file.
#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    Record(RecordDef),
    Header(HeaderDef),
}

impl Definition {
    /// The signature the definition describes.
    pub fn signature(&self) -> Signature {
        match self {
            Self::Record(r) => r.signature,
            Self::Header(h) => h.signature,
        }
    }
}

/// A validated set of definitions, keyed by signature.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schema {
    pub definitions: BTreeMap<Signature, Definition>,
}

impl Schema {
    /// Record definitions in signature order.
    pub fn records(&self) -> impl Iterator<Item = &RecordDef> {
        self.definitions.values().filter_map(|d| match d {
            Definition::Record(r) => Some(r),
            Definition::Header(_) => None,
        })
    }

    /// Header definitions in signature order.
    pub fn headers(&self) -> impl Iterator<Item = &HeaderDef> {
        self.definitions.values().filter_map(|d| match d {
            Definition::Header(h) => Some(h),
            Definition::Record(_) => None,
        })
    }

    /// The record definition for `signature`, if any.
    pub fn record(&self, signature: Signature) -> Option<&RecordDef> {
        match self.definitions.get(&signature)? {
            Definition::Record(r) => Some(r),
            Definition::Header(_) => None,
        }
    }
}
