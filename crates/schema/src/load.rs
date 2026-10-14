//! TOML loader and validator for definition files.
//!
//! The on-disk format is documented in `crates/schema/defs/README.md`. Parsing happens in
//! two steps: serde reads a permissive "raw" shape (all keys optional, unknown keys
//! rejected), then [`convert_file`] checks every rule and builds the typed model. Errors
//! name the file and a path inside it (`member[3].field[1]`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;

use crate::model::{
    AnyUntilDef, ArrayCount, Cardinality, Definition, FieldDef, FieldType, GroupDef, HeaderDef,
    Member, NamedValue, RecordDef, Scalar, Schema, Signature, SubrecordDef, VersionedSize,
};
use crate::names::{is_pascal_case, snake_case};

/// Deepest allowed nesting of groups and struct fields.
pub const MAX_DEPTH: usize = 8;

/// A definition set failed to load.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// The file is not valid TOML or does not have the expected shape.
    #[error("{file}:{line}:{column}: malformed definition: {message}")]
    Toml {
        file: String,
        line: usize,
        column: usize,
        message: String,
    },
    /// The file parsed but breaks a rule.
    #[error("{file}: {path}: {kind}")]
    Invalid {
        file: String,
        path: String,
        kind: ErrorKind,
    },
    /// Two files define the same signature.
    #[error("{file}: duplicate definition of {signature} (already defined in {first})")]
    Duplicate {
        file: String,
        signature: Signature,
        first: String,
    },
    /// A definition file or directory could not be read.
    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// The rule a definition breaks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorKind {
    #[error("invalid signature {0:?}: expected exactly four printable ASCII characters")]
    InvalidSignature(String),
    #[error("file name {file:?} does not match signature {signature}")]
    FileName { file: String, signature: Signature },
    #[error("invalid name {0:?}: expected PascalCase ASCII ([A-Z][A-Za-z0-9]*)")]
    InvalidName(String),
    #[error("name {0:?} is reserved")]
    ReservedName(String),
    #[error("duplicate name {0:?}")]
    DuplicateName(String),
    #[error("duplicate value {0}")]
    DuplicateValue(i128),
    #[error("unknown field type {0:?}")]
    UnknownType(String),
    #[error("unknown cardinality {0:?}: expected required, optional or repeated")]
    UnknownCardinality(String),
    #[error("unknown definition kind {0:?}: expected record or header")]
    UnknownKind(String),
    #[error("missing key `{0}`")]
    MissingKey(&'static str),
    #[error("key `{key}` is not valid for {context}")]
    UnexpectedKey { key: &'static str, context: String },
    #[error("declared size {declared} does not match the layout size {computed}")]
    SizeConflict { declared: u32, computed: u32 },
    #[error("declared size {declared} but the layout has a variable size")]
    SizeOfVariable { declared: u32 },
    #[error("variable-size field {0:?} must be the last field")]
    VariableNotLast(String),
    #[error("array elements must have a fixed, non-zero size")]
    BadElement,
    #[error("array count must be a positive integer or \"rest\", got {0}")]
    BadCount(String),
    #[error("{base} cannot hold {what}")]
    BadBase {
        base: &'static str,
        what: &'static str,
    },
    #[error("value {value} does not fit in {base}")]
    ValueOutOfRange { value: i128, base: &'static str },
    #[error("flag {0:?} must be a non-zero bit mask")]
    ZeroFlag(String),
    #[error("version range {min:?}..={max:?} is empty")]
    BadVersionRange { min: Option<u16>, max: Option<u16> },
    #[error("source must start with `UESP:` or `corpus:`, got {0:?}")]
    BadSource(String),
    #[error("a member sets none or several of `sig`, `group`, `any_until`")]
    MemberKind,
    #[error("`any_until = \"{0}\"` must be followed in the same list by a {0} subrecord")]
    AnyUntilUnclosed(Signature),
    #[error("a group must start with a subrecord or a group, not `any_until`")]
    GroupStart,
    #[error("the list is empty")]
    Empty,
    #[error("nesting deeper than {0} levels")]
    TooDeep(usize),
}

// ---------------------------------------------------------------------------------------
// Raw (serde) shapes. Every key is optional here; the converter decides what is required.

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    signature: String,
    name: String,
    kind: Option<String>,
    source: String,
    #[serde(default)]
    evidence: Vec<String>,
    note: Option<String>,
    flags: Option<Vec<(String, i64)>>,
    size: Option<u32>,
    member: Option<Vec<RawMember>>,
    field: Option<Vec<RawField>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMember {
    sig: Option<String>,
    group: Option<String>,
    any_until: Option<String>,
    name: Option<String>,
    card: Option<String>,
    note: Option<String>,
    #[serde(rename = "type")]
    ty: Option<String>,
    targets: Option<Vec<String>>,
    count: Option<RawCount>,
    element: Option<String>,
    base: Option<String>,
    values: Option<Vec<(String, i64)>>,
    bits: Option<Vec<(String, i64)>>,
    size: Option<u32>,
    field: Option<Vec<RawField>>,
    size_by_version: Option<Vec<RawVersionedSize>>,
    member: Option<Vec<RawMember>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawField {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    targets: Option<Vec<String>>,
    count: Option<RawCount>,
    element: Option<String>,
    base: Option<String>,
    values: Option<Vec<(String, i64)>>,
    bits: Option<Vec<(String, i64)>>,
    size: Option<u32>,
    field: Option<Vec<RawField>>,
    since: Option<u16>,
    until: Option<u16>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawCount {
    Number(i64),
    Word(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVersionedSize {
    min: Option<u16>,
    max: Option<u16>,
    size: u32,
}

/// The type-describing keys shared by members and fields.
struct TypeSpec<'a> {
    ty: &'a str,
    targets: Option<&'a Vec<String>>,
    count: Option<&'a RawCount>,
    element: Option<&'a str>,
    base: Option<&'a str>,
    values: Option<&'a Vec<(String, i64)>>,
    bits: Option<&'a Vec<(String, i64)>>,
    size: Option<u32>,
    field: Option<&'a Vec<RawField>>,
}

// ---------------------------------------------------------------------------------------
// Public entry points.

/// Parses and validates one definition file. `file` is used in error messages and, when
/// it ends in `.toml`, must be `<signature>.toml`.
pub fn parse_definition(file: &str, text: &str) -> Result<Definition, SchemaError> {
    let raw: RawFile = toml::from_str(text).map_err(|e| {
        let (line, column) = e
            .span()
            .map(|span| line_column(text, span.start))
            .unwrap_or((0, 0));
        SchemaError::Toml {
            file: file.to_owned(),
            line,
            column,
            message: e.message().to_owned(),
        }
    })?;
    let mut cx = Cx {
        file,
        path: Vec::new(),
    };
    convert_file(&mut cx, raw)
}

/// Loads a set of `(file name, text)` pairs into one schema, rejecting duplicate
/// signatures across files.
pub fn load_sources<'a, I>(sources: I) -> Result<Schema, SchemaError>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut schema = Schema::default();
    let mut origin: BTreeMap<Signature, String> = BTreeMap::new();
    for (file, text) in sources {
        let def = parse_definition(file, text)?;
        let signature = def.signature();
        if let Some(first) = origin.get(&signature) {
            return Err(SchemaError::Duplicate {
                file: file.to_owned(),
                signature,
                first: first.clone(),
            });
        }
        origin.insert(signature, file.to_owned());
        schema.definitions.insert(signature, def);
    }
    Ok(schema)
}

/// Loads every `*.toml` file in `dir` (not recursive), in file-name order.
pub fn load_dir(dir: &Path) -> Result<Schema, SchemaError> {
    let io = |path: &Path, source| SchemaError::Io {
        path: path.display().to_string(),
        source,
    };
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let path = entry.map_err(|e| io(dir, e))?.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            files.push(path);
        }
    }
    files.sort();
    let mut texts = Vec::with_capacity(files.len());
    for path in &files {
        let text = std::fs::read_to_string(path).map_err(|e| io(path, e))?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        texts.push((name, text));
    }
    load_sources(texts.iter().map(|(n, t)| (n.as_str(), t.as_str())))
}

fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.matches('\n').count().saturating_add(1);
    let column = before
        .rfind('\n')
        .map_or(before.len(), |nl| {
            before.len().saturating_sub(nl).saturating_sub(1)
        })
        .saturating_add(1);
    (line, column)
}

// ---------------------------------------------------------------------------------------
// Conversion and validation.

/// Conversion context: the file and the path inside it, for error messages.
struct Cx<'f> {
    file: &'f str,
    path: Vec<String>,
}

impl Cx<'_> {
    fn err(&self, kind: ErrorKind) -> SchemaError {
        SchemaError::Invalid {
            file: self.file.to_owned(),
            path: if self.path.is_empty() {
                "(file)".to_owned()
            } else {
                self.path.join(".")
            },
            kind,
        }
    }

    fn scoped<T>(
        &mut self,
        segment: String,
        f: impl FnOnce(&mut Self) -> Result<T, SchemaError>,
    ) -> Result<T, SchemaError> {
        self.path.push(segment);
        let out = f(self);
        self.path.pop();
        out
    }

    fn depth(&self) -> usize {
        self.path.len()
    }
}

fn convert_file(cx: &mut Cx<'_>, raw: RawFile) -> Result<Definition, SchemaError> {
    let signature = signature(cx, &raw.signature)?;
    if let Some(stem) = cx.file.strip_suffix(".toml") {
        let stem = stem.rsplit(['/', '\\']).next().unwrap_or(stem);
        if stem != signature.as_str() {
            return Err(cx.err(ErrorKind::FileName {
                file: cx.file.to_owned(),
                signature,
            }));
        }
    }
    check_name(cx, &raw.name)?;
    if !(raw.source.starts_with("UESP:") || raw.source.starts_with("corpus:")) {
        return Err(cx.err(ErrorKind::BadSource(raw.source)));
    }
    let kind = raw.kind.as_deref().unwrap_or("record");
    match kind {
        "record" => {
            reject(cx, raw.size.is_some(), "size", "a record definition")?;
            reject(cx, raw.field.is_some(), "field", "a record definition")?;
            let flags = match &raw.flags {
                Some(flags) => cx.scoped("flags".into(), |cx| {
                    named_values(cx, flags, Scalar::U32, true)
                })?,
                None => Vec::new(),
            };
            let members = raw.member.unwrap_or_default();
            let members = convert_members(cx, &members, "member")?;
            Ok(Definition::Record(RecordDef {
                signature,
                name: raw.name,
                source: raw.source,
                evidence: raw.evidence,
                flags,
                members,
                note: raw.note,
            }))
        }
        "header" => {
            reject(cx, raw.flags.is_some(), "flags", "a header definition")?;
            reject(cx, raw.member.is_some(), "member", "a header definition")?;
            let size = raw
                .size
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("size")))?;
            let fields = raw
                .field
                .as_ref()
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("field")))?;
            let fields = convert_fields(cx, fields)?;
            let ty = FieldType::Struct { fields };
            check_size(cx, &ty, Some(size), &[])?;
            let FieldType::Struct { fields } = ty else {
                return Err(cx.err(ErrorKind::Empty));
            };
            Ok(Definition::Header(HeaderDef {
                signature,
                name: raw.name,
                source: raw.source,
                evidence: raw.evidence,
                size,
                fields,
                note: raw.note,
            }))
        }
        other => Err(cx.err(ErrorKind::UnknownKind(other.to_owned()))),
    }
}

fn reject(cx: &Cx<'_>, present: bool, key: &'static str, context: &str) -> Result<(), SchemaError> {
    if present {
        Err(cx.err(ErrorKind::UnexpectedKey {
            key,
            context: context.to_owned(),
        }))
    } else {
        Ok(())
    }
}

fn signature(cx: &Cx<'_>, text: &str) -> Result<Signature, SchemaError> {
    Signature::parse(text).ok_or_else(|| cx.err(ErrorKind::InvalidSignature(text.to_owned())))
}

fn check_name(cx: &Cx<'_>, name: &str) -> Result<(), SchemaError> {
    if !is_pascal_case(name) {
        return Err(cx.err(ErrorKind::InvalidName(name.to_owned())));
    }
    Ok(())
}

/// Checks names in one list for validity and uniqueness (also after snake_case
/// conversion, since generated Rust uses snake_case field names).
fn check_unique_names<'n>(
    cx: &Cx<'_>,
    names: impl IntoIterator<Item = &'n str>,
    reserved: &[&str],
) -> Result<(), SchemaError> {
    let mut seen = BTreeSet::new();
    for name in names {
        check_name(cx, name)?;
        if reserved.contains(&name) {
            return Err(cx.err(ErrorKind::ReservedName(name.to_owned())));
        }
        if !seen.insert(snake_case(name)) {
            return Err(cx.err(ErrorKind::DuplicateName(name.to_owned())));
        }
    }
    Ok(())
}

fn cardinality(cx: &Cx<'_>, card: Option<&str>) -> Result<Cardinality, SchemaError> {
    match card.unwrap_or("required") {
        "required" => Ok(Cardinality::Required),
        "optional" => Ok(Cardinality::Optional),
        "repeated" => Ok(Cardinality::Repeated),
        other => Err(cx.err(ErrorKind::UnknownCardinality(other.to_owned()))),
    }
}

/// Names reserved in member lists: generated record structs have an `extra` field.
const RESERVED_MEMBER_NAMES: &[&str] = &["Extra"];

fn convert_members(
    cx: &mut Cx<'_>,
    raw: &[RawMember],
    list: &str,
) -> Result<Vec<Member>, SchemaError> {
    if cx.depth() > MAX_DEPTH {
        return Err(cx.err(ErrorKind::TooDeep(MAX_DEPTH)));
    }
    let mut members = Vec::with_capacity(raw.len());
    for (i, m) in raw.iter().enumerate() {
        let member = cx.scoped(format!("{list}[{i}]"), |cx| convert_member(cx, m))?;
        members.push(member);
    }
    check_unique_names(cx, members.iter().map(Member::name), RESERVED_MEMBER_NAMES)?;
    // Every any_until must be closed by a later subrecord with its signature.
    for (i, m) in members.iter().enumerate() {
        if let Member::AnyUntil(a) = m {
            let closed = members
                .iter()
                .skip(i.saturating_add(1))
                .any(|later| matches!(later, Member::Subrecord(s) if s.signature == a.until));
            if !closed {
                return Err(cx.err(ErrorKind::AnyUntilUnclosed(a.until)));
            }
        }
    }
    Ok(members)
}

fn convert_member(cx: &mut Cx<'_>, m: &RawMember) -> Result<Member, SchemaError> {
    let kinds = [m.sig.is_some(), m.group.is_some(), m.any_until.is_some()];
    if kinds.iter().filter(|k| **k).count() != 1 {
        return Err(cx.err(ErrorKind::MemberKind));
    }
    if let Some(group) = &m.group {
        return convert_group(cx, group, m).map(Member::Group);
    }
    if let Some(until) = &m.any_until {
        let until = signature(cx, until)?;
        for (present, key) in [
            (m.card.is_some(), "card"),
            (m.ty.is_some(), "type"),
            (m.member.is_some(), "member"),
            (m.size.is_some(), "size"),
            (m.size_by_version.is_some(), "size_by_version"),
        ] {
            reject(cx, present, key, "an `any_until` member")?;
        }
        reject_type_keys(cx, m, "an `any_until` member")?;
        let name = m
            .name
            .clone()
            .ok_or_else(|| cx.err(ErrorKind::MissingKey("name")))?;
        return Ok(Member::AnyUntil(AnyUntilDef {
            name,
            until,
            note: m.note.clone(),
        }));
    }
    let sig_text = m.sig.as_deref().unwrap_or_default();
    let sig = signature(cx, sig_text)?;
    reject(cx, m.member.is_some(), "member", "a subrecord member")?;
    let name = m
        .name
        .clone()
        .ok_or_else(|| cx.err(ErrorKind::MissingKey("name")))?;
    let cardinality = cardinality(cx, m.card.as_deref())?;
    let ty_text =
        m.ty.as_deref()
            .ok_or_else(|| cx.err(ErrorKind::MissingKey("type")))?;
    let spec = TypeSpec {
        ty: ty_text,
        targets: m.targets.as_ref(),
        count: m.count.as_ref(),
        element: m.element.as_deref(),
        base: m.base.as_deref(),
        values: m.values.as_ref(),
        bits: m.bits.as_ref(),
        size: m.size,
        field: m.field.as_ref(),
    };
    let ty = convert_type(cx, &spec, true)?;
    let mut size_by_version = Vec::new();
    for (i, v) in m.size_by_version.iter().flatten().enumerate() {
        cx.scoped(format!("size_by_version[{i}]"), |cx| {
            version_range(cx, v.min, v.max)?;
            size_by_version.push(VersionedSize {
                min: v.min,
                max: v.max,
                size: v.size,
            });
            Ok(())
        })?;
    }
    // For opaque payloads `size` is the opaque length itself, so it always agrees.
    check_size(cx, &ty, m.size, &size_by_version)?;
    Ok(Member::Subrecord(SubrecordDef {
        signature: sig,
        name,
        cardinality,
        ty,
        size: m.size,
        size_by_version,
        note: m.note.clone(),
    }))
}

fn reject_type_keys(cx: &Cx<'_>, m: &RawMember, context: &str) -> Result<(), SchemaError> {
    for (present, key) in [
        (m.targets.is_some(), "targets"),
        (m.count.is_some(), "count"),
        (m.element.is_some(), "element"),
        (m.base.is_some(), "base"),
        (m.values.is_some(), "values"),
        (m.bits.is_some(), "bits"),
        (m.field.is_some(), "field"),
    ] {
        reject(cx, present, key, context)?;
    }
    Ok(())
}

fn convert_group(cx: &mut Cx<'_>, name: &str, m: &RawMember) -> Result<GroupDef, SchemaError> {
    for (present, key) in [
        (m.name.is_some(), "name"),
        (m.ty.is_some(), "type"),
        (m.size.is_some(), "size"),
        (m.size_by_version.is_some(), "size_by_version"),
    ] {
        reject(
            cx,
            present,
            key,
            "a group member (the group value is its name)",
        )?;
    }
    reject_type_keys(cx, m, "a group member")?;
    check_name(cx, name)?;
    let cardinality = cardinality(cx, m.card.as_deref())?;
    let raw = m
        .member
        .as_ref()
        .ok_or_else(|| cx.err(ErrorKind::MissingKey("member")))?;
    let members = convert_members(cx, raw, "member")?;
    match members.first() {
        None => return Err(cx.err(ErrorKind::Empty)),
        Some(Member::AnyUntil(_)) => return Err(cx.err(ErrorKind::GroupStart)),
        Some(_) => {}
    }
    Ok(GroupDef {
        name: name.to_owned(),
        cardinality,
        members,
        note: m.note.clone(),
    })
}

fn convert_fields(cx: &mut Cx<'_>, raw: &[RawField]) -> Result<Vec<FieldDef>, SchemaError> {
    if cx.depth() > MAX_DEPTH {
        return Err(cx.err(ErrorKind::TooDeep(MAX_DEPTH)));
    }
    if raw.is_empty() {
        return Err(cx.err(ErrorKind::Empty));
    }
    let mut fields = Vec::with_capacity(raw.len());
    for (i, f) in raw.iter().enumerate() {
        let field = cx.scoped(format!("field[{i}]"), |cx| {
            version_range(cx, f.since, f.until)?;
            let spec = TypeSpec {
                ty: &f.ty,
                targets: f.targets.as_ref(),
                count: f.count.as_ref(),
                element: f.element.as_deref(),
                base: f.base.as_deref(),
                values: f.values.as_ref(),
                bits: f.bits.as_ref(),
                size: f.size,
                field: f.field.as_ref(),
            };
            let ty = convert_type(cx, &spec, false)?;
            Ok(FieldDef {
                name: f.name.clone(),
                ty,
                since: f.since,
                until: f.until,
                note: f.note.clone(),
            })
        })?;
        fields.push(field);
    }
    check_unique_names(cx, fields.iter().map(|f| f.name.as_str()), &[])?;
    // Variable-size fields can only be decoded as "the rest", so they must be last.
    let last = fields.len().saturating_sub(1);
    for (i, f) in fields.iter().enumerate() {
        if i != last && f.ty.fixed_size(None).is_none() {
            return Err(cx.err(ErrorKind::VariableNotLast(f.name.clone())));
        }
    }
    Ok(fields)
}

fn version_range(cx: &Cx<'_>, min: Option<u16>, max: Option<u16>) -> Result<(), SchemaError> {
    if let (Some(lo), Some(hi)) = (min, max) {
        if lo > hi {
            return Err(cx.err(ErrorKind::BadVersionRange { min, max }));
        }
    }
    Ok(())
}

/// Builds a field type from its keys. `is_member` is true for a whole subrecord payload,
/// where `size` on an opaque type is the payload size.
fn convert_type(
    cx: &mut Cx<'_>,
    spec: &TypeSpec<'_>,
    is_member: bool,
) -> Result<FieldType, SchemaError> {
    let context = format!("type {:?}", spec.ty);
    let allow = |cx: &Cx<'_>, allowed: &[&str]| -> Result<(), SchemaError> {
        for (present, key) in [
            (spec.targets.is_some(), "targets"),
            (spec.count.is_some(), "count"),
            (spec.element.is_some(), "element"),
            (spec.base.is_some(), "base"),
            (spec.values.is_some(), "values"),
            (spec.bits.is_some(), "bits"),
            (spec.size.is_some(), "size"),
            (spec.field.is_some(), "field"),
        ] {
            // A member's `size` is a declared payload size, valid for every type.
            let member_size = is_member && key == "size";
            if present && !member_size && !allowed.contains(&key) {
                return Err(cx.err(ErrorKind::UnexpectedKey {
                    key,
                    context: context.clone(),
                }));
            }
        }
        Ok(())
    };
    let ty = match spec.ty {
        "formid" => {
            allow(cx, &["targets"])?;
            FieldType::FormId {
                targets: targets(cx, spec.targets)?,
            }
        }
        "sig" => {
            allow(cx, &[])?;
            FieldType::Sig
        }
        "zstring" => {
            allow(cx, &[])?;
            FieldType::ZString
        }
        "lstring" => {
            allow(cx, &[])?;
            FieldType::LString
        }
        "empty" => {
            allow(cx, &[])?;
            FieldType::Empty
        }
        "opaque" => {
            allow(cx, &["size"])?;
            FieldType::Opaque { size: spec.size }
        }
        "enum" => {
            allow(cx, &["base", "values"])?;
            let base = int_base(cx, spec.base, "enum values")?;
            let values = spec
                .values
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("values")))?;
            let values = cx.scoped("values".into(), |cx| named_values(cx, values, base, false))?;
            FieldType::Enum { base, values }
        }
        "flags" => {
            allow(cx, &["base", "bits"])?;
            let base = int_base(cx, spec.base, "flag bits")?;
            if base.is_signed() {
                return Err(cx.err(ErrorKind::BadBase {
                    base: base.as_str(),
                    what: "flag bits (use an unsigned type)",
                }));
            }
            let bits = spec
                .bits
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("bits")))?;
            let bits = cx.scoped("bits".into(), |cx| named_values(cx, bits, base, true))?;
            FieldType::Flags { base, bits }
        }
        "struct" => {
            allow(cx, &["field"])?;
            let raw = spec
                .field
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("field")))?;
            FieldType::Struct {
                fields: convert_fields(cx, raw)?,
            }
        }
        "array" => {
            allow(
                cx,
                &[
                    "count", "element", "targets", "base", "values", "bits", "field", "size",
                ],
            )?;
            let count = match spec.count {
                None => return Err(cx.err(ErrorKind::MissingKey("count"))),
                Some(RawCount::Word(w)) if w == "rest" => ArrayCount::Rest,
                Some(RawCount::Number(n)) => match u32::try_from(*n) {
                    Ok(n) if n > 0 => ArrayCount::Fixed(n),
                    _ => return Err(cx.err(ErrorKind::BadCount(n.to_string()))),
                },
                Some(RawCount::Word(w)) => {
                    return Err(cx.err(ErrorKind::BadCount(format!("{w:?}"))))
                }
            };
            let element_ty = spec
                .element
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("element")))?;
            if element_ty == "array" {
                return Err(cx.err(ErrorKind::BadElement));
            }
            let element_spec = TypeSpec {
                ty: element_ty,
                targets: spec.targets,
                count: None,
                element: None,
                base: spec.base,
                values: spec.values,
                bits: spec.bits,
                // On an array member, `size` is the payload size, not the element's.
                size: if is_member { None } else { spec.size },
                field: spec.field,
            };
            let element = cx.scoped("element".into(), |cx| {
                convert_type(cx, &element_spec, false)
            })?;
            match element.fixed_size(None) {
                Some(n) if n > 0 => {}
                _ => return Err(cx.err(ErrorKind::BadElement)),
            }
            FieldType::Array {
                element: Box::new(element),
                count,
            }
        }
        other => match Scalar::parse(other) {
            Some(scalar) => {
                allow(cx, &[])?;
                FieldType::Scalar(scalar)
            }
            None => return Err(cx.err(ErrorKind::UnknownType(other.to_owned()))),
        },
    };
    Ok(ty)
}

fn targets(cx: &Cx<'_>, raw: Option<&Vec<String>>) -> Result<Vec<Signature>, SchemaError> {
    let mut out = Vec::new();
    for t in raw.into_iter().flatten() {
        let sig = signature(cx, t)?;
        if !out.contains(&sig) {
            out.push(sig);
        }
    }
    Ok(out)
}

fn int_base(cx: &Cx<'_>, base: Option<&str>, what: &'static str) -> Result<Scalar, SchemaError> {
    let text = base.ok_or_else(|| cx.err(ErrorKind::MissingKey("base")))?;
    let scalar = Scalar::parse(text).ok_or_else(|| cx.err(ErrorKind::UnknownType(text.into())))?;
    if !scalar.is_integer() {
        return Err(cx.err(ErrorKind::BadBase {
            base: scalar.as_str(),
            what,
        }));
    }
    Ok(scalar)
}

fn named_values(
    cx: &Cx<'_>,
    raw: &[(String, i64)],
    base: Scalar,
    is_flags: bool,
) -> Result<Vec<NamedValue>, SchemaError> {
    if raw.is_empty() {
        return Err(cx.err(ErrorKind::Empty));
    }
    let mut values = BTreeSet::new();
    let mut out = Vec::with_capacity(raw.len());
    for (name, value) in raw {
        let value = i128::from(*value);
        if is_flags && value == 0 {
            return Err(cx.err(ErrorKind::ZeroFlag(name.clone())));
        }
        if !base.fits(value) {
            return Err(cx.err(ErrorKind::ValueOutOfRange {
                value,
                base: base.as_str(),
            }));
        }
        if !values.insert(value) {
            return Err(cx.err(ErrorKind::DuplicateValue(value)));
        }
        out.push(NamedValue {
            name: name.clone(),
            value,
        });
    }
    check_unique_names(cx, out.iter().map(|v| v.name.as_str()), &["Other"])?;
    Ok(out)
}

/// Checks a declared size (and per-version sizes) against the layout.
fn check_size(
    cx: &Cx<'_>,
    ty: &FieldType,
    declared: Option<u32>,
    by_version: &[VersionedSize],
) -> Result<(), SchemaError> {
    let versioned = ty.is_versioned();
    if let Some(declared) = declared {
        if versioned {
            return Err(cx.err(ErrorKind::UnexpectedKey {
                key: "size",
                context: "a layout with version-gated fields (use size_by_version)".into(),
            }));
        }
        match ty.fixed_size(None) {
            None => return Err(cx.err(ErrorKind::SizeOfVariable { declared })),
            Some(computed) if computed != declared => {
                return Err(cx.err(ErrorKind::SizeConflict { declared, computed }));
            }
            Some(_) => {}
        }
    }
    for entry in by_version {
        // A layout that is variable-size (opaque rest, strings) cannot be checked; the
        // entry is then a documented fact only.
        let probes = [entry.min, entry.max];
        for probe in probes.into_iter().flatten() {
            let at = if versioned { Some(probe) } else { None };
            if let Some(computed) = ty.fixed_size(at) {
                if computed != entry.size {
                    return Err(cx.err(ErrorKind::SizeConflict {
                        declared: entry.size,
                        computed,
                    }));
                }
            }
        }
        if entry.min.is_none() && entry.max.is_none() {
            if let Some(computed) = ty.fixed_size(None) {
                if !versioned && computed != entry.size {
                    return Err(cx.err(ErrorKind::SizeConflict {
                        declared: entry.size,
                        computed,
                    }));
                }
            }
        }
    }
    Ok(())
}
