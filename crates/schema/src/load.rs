//! TOML loader and validator for definition files.
//!
//! The on-disk format is documented in `crates/schema/defs/README.md`. Parsing happens in
//! two steps: serde reads a permissive "raw" shape (all keys optional, unknown keys
//! rejected), then [`convert_file`] checks every rule and builds the typed model. Errors
//! name the file and a path inside it (`member[3].field[1]`).
//!
//! Shared definitions (`defs/common/<Name>.toml`, `kind = "group"` or `"type"`) are
//! converted on first use and expanded into every record that references them, so the
//! model never holds an unresolved reference.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;

use crate::model::{
    AnyUntilDef, ArrayCount, Cardinality, CommonBody, CommonDef, Confidence, Decider, Definition,
    FieldDef, FieldType, GroupDef, HeaderDef, Member, NamedValue, Provenance, RecordDef, Scalar,
    Schema, Signature, SubrecordDef, UnionVariant, VersionedSize,
};
use crate::names::{is_pascal_case, snake_case};

/// Deepest allowed nesting of groups and struct fields.
pub const MAX_DEPTH: usize = 8;

/// The subdirectory of a definition directory that holds shared definitions.
pub const COMMON_DIR: &str = "common";

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
    /// Two files define the same signature or name.
    #[error("{file}: duplicate definition of {key} (already defined in {first})")]
    Duplicate {
        file: String,
        key: String,
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
    #[error("file name {file:?} does not match {expected}")]
    FileName { file: String, expected: String },
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
    #[error("unknown definition kind {0:?}: expected record, header, group or type")]
    UnknownKind(String),
    #[error("missing key `{0}`")]
    MissingKey(&'static str),
    #[error("key `{key}` is not valid for {context}")]
    UnexpectedKey { key: &'static str, context: String },
    #[error("declared size {declared} does not match the layout size {computed}")]
    SizeConflict { declared: u32, computed: u32 },
    #[error("declared size {declared} but the layout has a variable size")]
    SizeOfVariable { declared: u32 },
    #[error("field {0:?} reads to the end of the payload (or is `if_remaining`) and must be the last field")]
    VariableNotLast(String),
    #[error("array elements must not read to the end of the payload and must not be empty")]
    BadElement,
    #[error("array count must be a positive integer or \"rest\", got {0}")]
    BadCount(String),
    #[error("`count_from = {0:?}` must name an earlier, ungated integer field of the same struct")]
    BadCountFrom(String),
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
    #[error("size_by_version ranges {first:?} and {second:?} overlap")]
    OverlappingVersions {
        first: (Option<u16>, Option<u16>),
        second: (Option<u16>, Option<u16>),
    },
    #[error(
        "source must start with `uesp-skyrim:`, `uesp-starfield:`, `xedit:`, `corpus:`, `ck-diff:` or `ck-wiki:` (ADR 0005), got {0:?}"
    )]
    BadSource(String),
    #[error("source {0:?} is forbidden by ADR 0005")]
    ForbiddenSource(String),
    #[error(
        "xedit citation {citation:?}: {reason}; expected `xedit: <wbDefinitionsSF1.pas or wbDefinitionsCommon.pas>@<pinned 40-hex sha>#L<start>-L<end>` (ADR 0004)"
    )]
    BadXeditSource {
        citation: String,
        reason: &'static str,
    },
    #[error(
        "unknown confidence {0:?}: expected reference, container-reference, corpus, ck-diff or verified"
    )]
    UnknownConfidence(String),
    #[error("`confidence` is required {0} (ADR 0005 Amendment 2, M2.B1)")]
    MissingConfidence(&'static str),
    #[error("confidence {confidence:?} needs {needs} (ADR 0005 Amendments 2 and 5)")]
    UnsupportedConfidence {
        confidence: String,
        needs: &'static str,
    },
    #[error("a member sets none or several of `sig`, `group`, `group_ref`, `any_until`")]
    MemberKind,
    #[error("`any_until = \"{0}\"` must be followed in the same list by a {0} subrecord")]
    AnyUntilUnclosed(Signature),
    #[error("a group must start with a subrecord or a group, not `any_until`")]
    GroupStart,
    #[error("the list is empty")]
    Empty,
    #[error("nesting deeper than {0} levels")]
    TooDeep(usize),
    #[error("no shared definition named {0:?} (defs/common/)")]
    UnknownRef(String),
    #[error("shared definition {name:?} is not a {expected}")]
    RefKind {
        name: String,
        expected: &'static str,
    },
    #[error("shared definition {0:?} refers to itself")]
    RefCycle(String),
    #[error("unknown union decider {0:?}: expected edid-prefix or field:<SiblingName>")]
    UnknownDecider(String),
    #[error("union variant key `when = {0}` must be one ASCII letter or digit for `decider = \"edid-prefix\"`, and one integer or a non-empty list of integers for a field decider")]
    BadWhen(String),
    #[error("`type = \"union\"` with `decider = \"edid-prefix\"` is only valid on a whole subrecord member, and with `decider = \"field:...\"` only on a struct field; never on an array element or a shared type")]
    UnionPlacement,
    #[error("union decider field {0:?} must be an earlier, ungated, non-`if_remaining` sibling of at most 32 bits: an integer or an enum")]
    BadDeciderField(String),
    #[error("union variant {variant:?} has size {size:?}, but every variant of a field union must have the fixed size of the first ({first:?})")]
    VariantSize {
        variant: String,
        size: Option<u32>,
        first: Option<u32>,
    },
    #[error("`secondary_when = {0}` must be an integer or four printable ASCII characters, on a union that sets `secondary`, and fit the secondary field")]
    BadSecondaryWhen(String),
    #[error(
        "a union with `decider = \"edid-prefix\"` needs an EDID subrecord member in the record"
    )]
    UnionWithoutEditorId,
    #[error("an unordered record cannot have a top-level `any_until` member: it would accept every subrecord out of place")]
    UnorderedAnyUntil,
    #[error("in an unordered record, {signature} can start two top-level members ({first} and {second}): which one takes it would depend on match order")]
    UnorderedAmbiguous {
        signature: Signature,
        first: String,
        second: String,
    },
}

// ---------------------------------------------------------------------------------------
// Raw (serde) shapes. Every key is optional here; the converter decides what is required.

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    signature: Option<String>,
    name: String,
    kind: Option<String>,
    source: String,
    #[serde(default)]
    evidence: Vec<String>,
    confidence: Option<String>,
    note: Option<String>,
    flags: Option<Vec<(String, i64)>>,
    unordered: Option<bool>,
    size_by_version: Option<Vec<RawVersionedSize>>,
    size: Option<u32>,
    member: Option<Vec<RawMember>>,
    field: Option<Vec<RawField>>,
    // Type keys of a shared type (`kind = "type"`).
    #[serde(rename = "type")]
    ty: Option<String>,
    targets: Option<Vec<String>>,
    count: Option<RawCount>,
    element: Option<String>,
    base: Option<String>,
    values: Option<Vec<(String, i64)>>,
    bits: Option<Vec<(String, i64)>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMember {
    sig: Option<String>,
    group: Option<String>,
    group_ref: Option<String>,
    any_until: Option<String>,
    name: Option<String>,
    card: Option<String>,
    note: Option<String>,
    #[serde(rename = "type")]
    ty: Option<String>,
    targets: Option<Vec<String>>,
    count: Option<RawCount>,
    count_from: Option<String>,
    element: Option<String>,
    base: Option<String>,
    values: Option<Vec<(String, i64)>>,
    bits: Option<Vec<(String, i64)>>,
    size: Option<u32>,
    field: Option<Vec<RawField>>,
    size_by_version: Option<Vec<RawVersionedSize>>,
    member: Option<Vec<RawMember>>,
    source: Option<String>,
    evidence: Option<Vec<String>>,
    confidence: Option<String>,
    decider: Option<String>,
    secondary: Option<String>,
    variants: Option<Vec<RawVariant>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVariant {
    when: RawWhen,
    secondary_when: Option<RawKey>,
    name: String,
    #[serde(rename = "type")]
    ty: String,
    targets: Option<Vec<String>>,
    base: Option<String>,
    values: Option<Vec<(String, i64)>>,
    bits: Option<Vec<(String, i64)>>,
    size: Option<u32>,
    note: Option<String>,
}

/// A variant's `when`: one EditorID prefix letter (`"f"`), or one or more values of the
/// deciding field (`4`, `[4, 6]`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawWhen {
    Number(i64),
    List(Vec<i64>),
    Text(String),
}

impl RawWhen {
    fn describe(&self) -> String {
        match self {
            Self::Number(n) => n.to_string(),
            Self::List(list) => format!("{list:?}"),
            Self::Text(text) => text.clone(),
        }
    }
}

/// A `secondary_when` value: an integer, or four characters read as a little-endian `u32`
/// (a signature-like property name such as `"WSLV"`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawKey {
    Number(i64),
    Text(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawField {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    targets: Option<Vec<String>>,
    count: Option<RawCount>,
    count_from: Option<String>,
    element: Option<String>,
    base: Option<String>,
    values: Option<Vec<(String, i64)>>,
    bits: Option<Vec<(String, i64)>>,
    size: Option<u32>,
    field: Option<Vec<RawField>>,
    since: Option<u16>,
    until: Option<u16>,
    if_remaining: Option<bool>,
    note: Option<String>,
    source: Option<String>,
    evidence: Option<Vec<String>>,
    confidence: Option<String>,
    decider: Option<String>,
    secondary: Option<String>,
    variants: Option<Vec<RawVariant>>,
}

/// The provenance keys a member or field may set (all optional; see [`provenance`]).
struct ProvKeys<'a> {
    source: Option<&'a String>,
    evidence: Option<&'a Vec<String>>,
    confidence: Option<&'a str>,
}

impl RawMember {
    fn prov(&self) -> ProvKeys<'_> {
        ProvKeys {
            source: self.source.as_ref(),
            evidence: self.evidence.as_ref(),
            confidence: self.confidence.as_deref(),
        }
    }
}

impl RawField {
    fn prov(&self) -> ProvKeys<'_> {
        ProvKeys {
            source: self.source.as_ref(),
            evidence: self.evidence.as_ref(),
            confidence: self.confidence.as_deref(),
        }
    }
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

/// The type-describing keys shared by members, fields and shared types.
struct TypeSpec<'a> {
    ty: &'a str,
    targets: Option<&'a Vec<String>>,
    count: Option<&'a RawCount>,
    count_from: Option<&'a str>,
    element: Option<&'a str>,
    base: Option<&'a str>,
    values: Option<&'a Vec<(String, i64)>>,
    bits: Option<&'a Vec<(String, i64)>>,
    size: Option<u32>,
    field: Option<&'a Vec<RawField>>,
}

// ---------------------------------------------------------------------------------------
// Public entry points.

/// Parses and validates one definition file on its own. `file` is used in error messages
/// and, when it ends in `.toml`, must match the definition (`<signature>.toml`, or
/// `<Name>.toml` for a shared definition). A file that references a shared definition
/// fails here with [`ErrorKind::UnknownRef`]; load it with [`load_sources`] together with
/// the shared files.
pub fn parse_definition(file: &str, text: &str) -> Result<Definition, SchemaError> {
    let raw = parse_raw(file, text)?;
    let mut resolver = Resolver::default();
    let mut cx = Cx {
        file,
        path: Vec::new(),
        res: &mut resolver,
        container: false,
    };
    convert_file(&mut cx, raw)
}

/// Loads a set of `(file name, text)` pairs into one schema, rejecting duplicate
/// signatures and names across files. Shared definitions (`kind = "group"` or `"type"`)
/// may appear anywhere in the list.
pub fn load_sources<'a, I>(sources: I) -> Result<Schema, SchemaError>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut resolver = Resolver::default();
    let mut others = Vec::new();
    for (file, text) in sources {
        let raw = parse_raw(file, text)?;
        if matches!(raw.kind.as_deref(), Some("group" | "type")) {
            if let Some((first, _)) = resolver.pending.get(&raw.name) {
                return Err(SchemaError::Duplicate {
                    file: file.to_owned(),
                    key: raw.name.clone(),
                    first: first.clone(),
                });
            }
            resolver
                .pending
                .insert(raw.name.clone(), (file.to_owned(), raw));
        } else {
            others.push((file, raw));
        }
    }
    let mut schema = Schema::default();
    // Shared definitions first, in name order, so every one is validated even if unused.
    let names: Vec<String> = resolver.pending.keys().cloned().collect();
    for name in names {
        let mut cx = Cx {
            file: "(shared definitions)",
            path: Vec::new(),
            res: &mut resolver,
            container: false,
        };
        resolve(&mut cx, &name)?;
    }
    let mut origin: BTreeMap<String, String> = BTreeMap::new();
    let mut claim = |file: &str, key: String| -> Result<(), SchemaError> {
        if let Some(first) = origin.get(&key) {
            return Err(SchemaError::Duplicate {
                file: file.to_owned(),
                key,
                first: first.clone(),
            });
        }
        origin.insert(key, file.to_owned());
        Ok(())
    };
    for (file, raw) in others {
        let mut cx = Cx {
            file,
            path: Vec::new(),
            res: &mut resolver,
            container: false,
        };
        match convert_file(&mut cx, raw)? {
            Definition::Record(r) => {
                claim(file, format!("signature {}", r.signature))?;
                schema.records.insert(r.signature, r);
            }
            Definition::Header(h) => {
                if let Some(sig) = h.signature {
                    claim(file, format!("signature {sig}"))?;
                }
                claim(file, format!("header {}", h.name))?;
                schema.headers.insert(h.name.clone(), h);
            }
            Definition::Common(_) => {}
        }
    }
    schema.common = resolver.done;
    Ok(schema)
}

/// Loads every `*.toml` file in `dir` and in `dir/common/` (shared definitions), not
/// otherwise recursive, in file-name order. File names are passed on as `NAME.toml` and
/// `common/NAME.toml`.
pub fn load_dir(dir: &Path) -> Result<Schema, SchemaError> {
    let mut texts = Vec::new();
    read_toml_files(dir, "", &mut texts)?;
    let common = dir.join(COMMON_DIR);
    if common.is_dir() {
        read_toml_files(&common, "common/", &mut texts)?;
    }
    load_sources(texts.iter().map(|(n, t)| (n.as_str(), t.as_str())))
}

fn read_toml_files(
    dir: &Path,
    prefix: &str,
    out: &mut Vec<(String, String)>,
) -> Result<(), SchemaError> {
    let io = |path: &Path, source| SchemaError::Io {
        path: path.display().to_string(),
        source,
    };
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let path = entry.map_err(|e| io(dir, e))?.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "toml") {
            files.push(path);
        }
    }
    files.sort();
    for path in &files {
        let text = std::fs::read_to_string(path).map_err(|e| io(path, e))?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push((format!("{prefix}{name}"), text));
    }
    Ok(())
}

fn parse_raw(file: &str, text: &str) -> Result<RawFile, SchemaError> {
    toml::from_str(text).map_err(|e| {
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
    })
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
// Shared definitions.

/// Shared definitions: raw ones waiting to be converted, converted ones, and the chain
/// being converted right now (for cycle detection).
#[derive(Default)]
struct Resolver {
    pending: BTreeMap<String, (String, RawFile)>,
    done: BTreeMap<String, CommonDef>,
    active: Vec<String>,
}

/// The converted shared definition `name`, converting it first if needed.
fn resolve(cx: &mut Cx<'_, '_>, name: &str) -> Result<CommonDef, SchemaError> {
    if let Some(def) = cx.res.done.get(name) {
        return Ok(def.clone());
    }
    if cx.res.active.iter().any(|a| a == name) {
        return Err(cx.err(ErrorKind::RefCycle(name.to_owned())));
    }
    let Some((file, raw)) = cx.res.pending.remove(name) else {
        return Err(cx.err(ErrorKind::UnknownRef(name.to_owned())));
    };
    cx.res.active.push(name.to_owned());
    let converted = {
        let mut inner = Cx {
            file: &file,
            path: Vec::new(),
            res: &mut *cx.res,
            container: false,
        };
        convert_file(&mut inner, raw)
    };
    cx.res.active.pop();
    let Definition::Common(def) = converted? else {
        return Err(cx.err(ErrorKind::UnknownRef(name.to_owned())));
    };
    cx.res.done.insert(name.to_owned(), def.clone());
    Ok(def)
}

// ---------------------------------------------------------------------------------------
// Conversion and validation.

/// Conversion context: the file and the path inside it, for error messages, and the
/// shared definitions.
struct Cx<'f, 'r> {
    file: &'f str,
    path: Vec<String>,
    res: &'r mut Resolver,
    /// The file being converted is a container definition (`TES4`, or a header: the group
    /// header and the record header), the only place `container-reference` is accepted
    /// (ADR 0005 Amendment 5). Set by [`convert_file`].
    container: bool,
}

impl Cx<'_, '_> {
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

    /// The file stem, when the file name ends in `.toml`.
    fn stem(&self) -> Option<&str> {
        let stem = self.file.strip_suffix(".toml")?;
        Some(stem.rsplit(['/', '\\']).next().unwrap_or(stem))
    }

    fn check_stem(&self, expected: &str) -> Result<(), SchemaError> {
        match self.stem() {
            Some(stem) if stem != expected => Err(self.err(ErrorKind::FileName {
                file: self.file.to_owned(),
                expected: format!("{expected}.toml"),
            })),
            _ => Ok(()),
        }
    }
}

fn convert_file(cx: &mut Cx<'_, '_>, raw: RawFile) -> Result<Definition, SchemaError> {
    check_name(cx, &raw.name)?;
    cx.container = match raw.kind.as_deref().unwrap_or("record") {
        "header" => true,
        "record" => raw.signature.as_deref() == Some("TES4"),
        _ => false,
    };
    let prov = file_provenance(cx, &raw)?;
    let kind = raw.kind.as_deref().unwrap_or("record");
    if kind != "type" {
        reject_file_type_keys(cx, &raw, kind)?;
    }
    match kind {
        "record" => {
            let sig_text = raw
                .signature
                .as_deref()
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("signature")))?;
            let signature = signature(cx, sig_text)?;
            cx.check_stem(signature.as_str())?;
            reject(cx, raw.size.is_some(), "size", "a record definition")?;
            reject(cx, raw.field.is_some(), "field", "a record definition")?;
            let flags = match &raw.flags {
                Some(flags) => cx.scoped("flags".into(), |cx| {
                    named_values(cx, flags, Scalar::U32, true)
                })?,
                None => Vec::new(),
            };
            let members = raw.member.unwrap_or_default();
            let members = convert_members(cx, &members, "member", &prov)?;
            check_group_depth(cx, &members, 0)?;
            let leaves = {
                let mut out = Vec::new();
                leaf_types(&members, &mut out);
                out
            };
            let has_union = leaves
                .iter()
                .any(|(_, ty)| matches!(ty, FieldType::Union { .. }));
            let edid = Signature::parse("EDID");
            if has_union && !leaves.iter().any(|(sig, _)| Some(*sig) == edid) {
                return Err(cx.err(ErrorKind::UnionWithoutEditorId));
            }
            let unordered = raw.unordered.unwrap_or(false);
            if unordered && members.iter().any(|m| matches!(m, Member::AnyUntil(_))) {
                return Err(cx.err(ErrorKind::UnorderedAnyUntil));
            }
            if unordered {
                check_unordered_entries(cx, &members)?;
            }
            Ok(Definition::Record(RecordDef {
                signature,
                name: raw.name,
                provenance: prov,
                flags,
                unordered,
                members,
                note: raw.note,
            }))
        }
        "header" => {
            let signature = match raw.signature.as_deref() {
                Some(text) => {
                    let sig = signature(cx, text)?;
                    cx.check_stem(sig.as_str())?;
                    Some(sig)
                }
                None => {
                    // A header without a fixed signature (the record header) is named by
                    // an upper-case file stem such as `RECORD`.
                    if let Some(stem) = cx.stem() {
                        let upper = !stem.is_empty()
                            && stem
                                .chars()
                                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                        if !upper {
                            return Err(cx.err(ErrorKind::FileName {
                                file: cx.file.to_owned(),
                                expected: "an upper-case name such as RECORD.toml".into(),
                            }));
                        }
                    }
                    None
                }
            };
            reject(cx, raw.flags.is_some(), "flags", "a header definition")?;
            reject(
                cx,
                raw.unordered.is_some(),
                "unordered",
                "a header definition",
            )?;
            reject(cx, raw.member.is_some(), "member", "a header definition")?;
            let size = raw
                .size
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("size")))?;
            let fields = raw
                .field
                .as_ref()
                .ok_or_else(|| cx.err(ErrorKind::MissingKey("field")))?;
            let fields = convert_fields(cx, fields, &prov)?;
            let ty = FieldType::Struct { fields };
            check_size(cx, &ty, Some(size), &[])?;
            let FieldType::Struct { fields } = ty else {
                return Err(cx.err(ErrorKind::Empty));
            };
            Ok(Definition::Header(HeaderDef {
                signature,
                name: raw.name,
                provenance: prov,
                size,
                fields,
                note: raw.note,
            }))
        }
        "group" | "type" => {
            cx.check_stem(&raw.name)?;
            let context = format!("a shared {kind} definition");
            reject(cx, raw.signature.is_some(), "signature", &context)?;
            reject(cx, raw.flags.is_some(), "flags", &context)?;
            reject(cx, raw.unordered.is_some(), "unordered", &context)?;
            let body = if kind == "group" {
                reject(cx, raw.size.is_some(), "size", &context)?;
                reject(cx, raw.field.is_some(), "field", &context)?;
                let raw_members = raw
                    .member
                    .as_ref()
                    .ok_or_else(|| cx.err(ErrorKind::MissingKey("member")))?;
                let members = convert_members(cx, raw_members, "member", &prov)?;
                match members.first() {
                    None => return Err(cx.err(ErrorKind::Empty)),
                    Some(Member::AnyUntil(_)) => return Err(cx.err(ErrorKind::GroupStart)),
                    Some(_) => {}
                }
                check_group_depth(cx, &members, 0)?;
                CommonBody::Group(members)
            } else {
                reject(cx, raw.member.is_some(), "member", &context)?;
                let ty_text = raw
                    .ty
                    .as_deref()
                    .ok_or_else(|| cx.err(ErrorKind::MissingKey("type")))?;
                let spec = TypeSpec {
                    ty: ty_text,
                    targets: raw.targets.as_ref(),
                    count: raw.count.as_ref(),
                    count_from: None,
                    element: raw.element.as_deref(),
                    base: raw.base.as_deref(),
                    values: raw.values.as_ref(),
                    bits: raw.bits.as_ref(),
                    size: raw.size,
                    field: raw.field.as_ref(),
                };
                CommonBody::Type(convert_type(cx, &spec, false, &prov)?)
            };
            // A shared type whose layout changes between form versions states its size
            // per version range, checked like a member's `size_by_version`.
            let mut size_by_version = Vec::new();
            for (i, v) in raw.size_by_version.iter().flatten().enumerate() {
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
            if let CommonBody::Type(ty) = &body {
                check_size(cx, ty, None, &size_by_version)?;
            }
            Ok(Definition::Common(CommonDef {
                name: raw.name,
                provenance: prov,
                body,
                size_by_version,
                note: raw.note,
            }))
        }
        other => Err(cx.err(ErrorKind::UnknownKind(other.to_owned()))),
    }
}

fn reject_file_type_keys(cx: &Cx<'_, '_>, raw: &RawFile, kind: &str) -> Result<(), SchemaError> {
    let context = format!("a {kind} definition");
    for (present, key) in [
        (raw.ty.is_some(), "type"),
        (raw.targets.is_some(), "targets"),
        (raw.count.is_some(), "count"),
        (raw.element.is_some(), "element"),
        (raw.base.is_some(), "base"),
        (raw.values.is_some(), "values"),
        (raw.bits.is_some(), "bits"),
        (raw.size_by_version.is_some(), "size_by_version"),
    ] {
        reject(cx, present, key, &context)?;
    }
    Ok(())
}

/// The `source` key forms permitted by ADR 0005 (source hierarchy, Amendment 2).
pub const SOURCE_PREFIXES: &[&str] = &[
    "uesp-skyrim: ",
    "uesp-starfield: ",
    "xedit: ",
    "corpus: ",
    "ck-diff: ",
    "ck-wiki: ",
];

/// Pages a `uesp-starfield:` citation may not name. The CTDA page follows a stale xEdit
/// branch; conditions are cited from xEdit at the pinned revision instead (ADR 0004).
pub const FORBIDDEN_UESP_STARFIELD_PAGES: &[&str] = &["Mod_File_Format/CTDA"];

const XEDIT_PREFIX: &str = "xedit: ";

/// The upstream xEdit revision every `xedit:` citation must name (ADR 0004). Changing it
/// needs an ADR 0004 amendment and a re-check of every cited line range.
pub const XEDIT_REVISION: &str = "9fb016884bec138ea6c7b872cec831537d464c3e";

/// The xEdit files the owner decision covers (ADR 0004; `wbDefinitionsCommon.pas` added
/// by the owner on 2026-10-14 for the shared helpers the Starfield definitions invoke).
pub const XEDIT_FILES: &[&str] = &["wbDefinitionsSF1.pas", "wbDefinitionsCommon.pas"];

fn check_source(cx: &Cx<'_, '_>, text: &str) -> Result<(), SchemaError> {
    if !SOURCE_PREFIXES.iter().any(|p| text.starts_with(p)) {
        return Err(cx.err(ErrorKind::BadSource(text.to_owned())));
    }
    if text.starts_with("uesp-starfield: ")
        && FORBIDDEN_UESP_STARFIELD_PAGES
            .iter()
            .any(|page| text.contains(page))
    {
        return Err(cx.err(ErrorKind::ForbiddenSource(text.to_owned())));
    }
    if let Some(rest) = text.strip_prefix(XEDIT_PREFIX) {
        check_xedit_citation(rest).map_err(|reason| {
            cx.err(ErrorKind::BadXeditSource {
                citation: text.to_owned(),
                reason,
            })
        })?;
    }
    Ok(())
}

/// Checks `<file>@<sha>#L<start>-L<end>` (the part after `xedit: `).
fn check_xedit_citation(rest: &str) -> Result<(), &'static str> {
    let (file, rest) = rest.split_once('@').ok_or("missing `@<sha>`")?;
    if !XEDIT_FILES.contains(&file) {
        return Err("the file must be wbDefinitionsSF1.pas or wbDefinitionsCommon.pas");
    }
    let (sha, lines) = rest.split_once('#').ok_or("missing `#L<start>-L<end>`")?;
    let is_hex = sha.len() == 40
        && sha
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !is_hex {
        return Err("the revision must be a full 40-character lower-case hex SHA");
    }
    if sha != XEDIT_REVISION {
        return Err("the revision is not the one ADR 0004 pins");
    }
    let (start, end) = lines
        .strip_prefix('L')
        .and_then(|l| l.split_once("-L"))
        .ok_or("the line range must read `L<start>-L<end>`")?;
    let start = line_number(start)?;
    let end = line_number(end)?;
    if start > end {
        return Err("the line range starts after it ends");
    }
    Ok(())
}

fn line_number(text: &str) -> Result<u32, &'static str> {
    let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    match text.parse::<u32>() {
        Ok(n) if digits && n > 0 && !text.starts_with('0') => Ok(n),
        _ => Err("line numbers must be positive integers without leading zeros"),
    }
}

fn parse_confidence(cx: &Cx<'_, '_>, text: &str) -> Result<Confidence, SchemaError> {
    Confidence::parse(text).ok_or_else(|| cx.err(ErrorKind::UnknownConfidence(text.to_owned())))
}

/// The provenance of a definition file: `source` (required), `evidence`, and
/// `confidence`, which every file must state (M2.B1; ADR 0005 Amendment 2 requires it on
/// `xedit:` and `uesp-skyrim:` citations, and the project requires it everywhere so every
/// member has a level to report).
fn file_provenance(cx: &Cx<'_, '_>, raw: &RawFile) -> Result<Provenance, SchemaError> {
    check_source(cx, &raw.source)?;
    for note in &raw.evidence {
        check_source(cx, note)?;
    }
    let level = raw
        .confidence
        .as_deref()
        .ok_or_else(|| cx.err(ErrorKind::MissingConfidence("on every definition file")))?;
    let prov = Provenance {
        source: raw.source.clone(),
        evidence: raw.evidence.clone(),
        confidence: parse_confidence(cx, level)?,
    };
    check_level(cx, &prov)?;
    Ok(prov)
}

/// The effective provenance of a member or field. With its own `source`, the member
/// states its own citations (`source` plus its own `evidence`) and must state its own
/// `confidence`. Without one, it inherits `parent`'s source, adds its own `evidence` to
/// the inherited evidence, and may raise or lower the inherited `confidence`. Whenever a
/// key is set, the resulting level is checked against the resulting citations.
fn provenance(
    cx: &Cx<'_, '_>,
    parent: &Provenance,
    keys: &ProvKeys<'_>,
) -> Result<Provenance, SchemaError> {
    if keys.source.is_none() && keys.evidence.is_none() && keys.confidence.is_none() {
        return Ok(parent.clone());
    }
    for note in keys
        .source
        .into_iter()
        .chain(keys.evidence.into_iter().flatten())
    {
        check_source(cx, note)?;
    }
    let own_evidence = keys.evidence.cloned().unwrap_or_default();
    let prov = match keys.source {
        Some(source) => {
            let level = keys.confidence.ok_or_else(|| {
                cx.err(ErrorKind::MissingConfidence(
                    "on a member or field with its own `source`",
                ))
            })?;
            Provenance {
                source: source.clone(),
                evidence: own_evidence,
                confidence: parse_confidence(cx, level)?,
            }
        }
        None => {
            let mut evidence = parent.evidence.clone();
            evidence.extend(own_evidence);
            let confidence = match keys.confidence {
                Some(level) => parse_confidence(cx, level)?,
                None => parent.confidence,
            };
            Provenance {
                source: parent.source.clone(),
                evidence,
                confidence,
            }
        }
    };
    check_level(cx, &prov)?;
    Ok(prov)
}

/// The claimed level must be backed by matching citations (the mechanical minimum of ADR
/// 0004 "Constraints", kept by the owner approval of 2026-10-14), and a `uesp-skyrim:`
/// fact is never `reference` (ADR 0005 Amendment 2). `container-reference` needs a
/// `uesp-skyrim:` citation and a container definition (ADR 0005 Amendment 5).
fn check_level(cx: &Cx<'_, '_>, prov: &Provenance) -> Result<(), SchemaError> {
    let cites = |prefix: &str| prov.citations().any(|c| c.starts_with(prefix));
    let unsupported = |needs| {
        cx.err(ErrorKind::UnsupportedConfidence {
            confidence: prov.confidence.as_str().to_owned(),
            needs,
        })
    };
    match prov.confidence {
        Confidence::Reference if !cites(XEDIT_PREFIX) => Err(unsupported("an `xedit:` citation")),
        Confidence::Reference if cites("uesp-skyrim: ") => Err(unsupported(
            "no `uesp-skyrim:` citation (Skyrim facts need `corpus` or better, or \
             `container-reference` in a container definition)",
        )),
        Confidence::ContainerReference if !cites("uesp-skyrim: ") => {
            Err(unsupported("a `uesp-skyrim:` citation"))
        }
        Confidence::ContainerReference if !cx.container => Err(unsupported(
            "a container definition (TES4, the group header or the record header); a Skyrim \
             fact never describes a Starfield record body",
        )),
        Confidence::Corpus if !cites("corpus: ") => Err(unsupported("a `corpus:` citation")),
        Confidence::CkDiff if !cites("ck-diff: ") => Err(unsupported("a `ck-diff:` citation")),
        Confidence::Verified if !(cites("corpus: ") && cites("ck-diff: ")) => {
            Err(unsupported("both a `corpus:` and a `ck-diff:` citation"))
        }
        _ => Ok(()),
    }
}

fn reject(
    cx: &Cx<'_, '_>,
    present: bool,
    key: &'static str,
    context: &str,
) -> Result<(), SchemaError> {
    if present {
        Err(cx.err(ErrorKind::UnexpectedKey {
            key,
            context: context.to_owned(),
        }))
    } else {
        Ok(())
    }
}

fn signature(cx: &Cx<'_, '_>, text: &str) -> Result<Signature, SchemaError> {
    Signature::parse(text).ok_or_else(|| cx.err(ErrorKind::InvalidSignature(text.to_owned())))
}

fn check_name(cx: &Cx<'_, '_>, name: &str) -> Result<(), SchemaError> {
    if !is_pascal_case(name) {
        return Err(cx.err(ErrorKind::InvalidName(name.to_owned())));
    }
    Ok(())
}

/// Checks names in one list for validity and uniqueness (also after snake_case
/// conversion, since generated Rust uses snake_case field names).
fn check_unique_names<'n>(
    cx: &Cx<'_, '_>,
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

fn cardinality(cx: &Cx<'_, '_>, card: Option<&str>) -> Result<Cardinality, SchemaError> {
    match card.unwrap_or("required") {
        "required" => Ok(Cardinality::Required),
        "optional" => Ok(Cardinality::Optional),
        "repeated" => Ok(Cardinality::Repeated),
        other => Err(cx.err(ErrorKind::UnknownCardinality(other.to_owned()))),
    }
}

/// Every subrecord member's signature and type, groups expanded.
fn leaf_types<'m>(members: &'m [Member], out: &mut Vec<(Signature, &'m FieldType)>) {
    for member in members {
        match member {
            Member::Subrecord(s) => out.push((s.signature, &s.ty)),
            Member::Group(g) => leaf_types(&g.members, out),
            Member::AnyUntil(_) => {}
        }
    }
}

/// The signatures that can start `member`: its own for a subrecord; for a group, those of
/// its members up to and including the first required one (nested groups likewise).
fn entry_signatures(member: &Member, depth: usize, out: &mut BTreeSet<Signature>) {
    match member {
        Member::Subrecord(s) => {
            out.insert(s.signature);
        }
        Member::AnyUntil(_) => {}
        Member::Group(g) if depth <= MAX_DEPTH => {
            for inner in &g.members {
                entry_signatures(inner, depth.saturating_add(1), out);
                if inner.cardinality() == Cardinality::Required {
                    break;
                }
            }
        }
        Member::Group(_) => {}
    }
}

/// An unordered record matches a subrecord against every top-level member from the start
/// again when the current one does not take it, so a signature that two top-level members
/// can start (two subrecord members, or a subrecord and a group that may begin with it)
/// would make which member it lands in depend on the order the matcher happens to be in.
fn check_unordered_entries(cx: &Cx<'_, '_>, members: &[Member]) -> Result<(), SchemaError> {
    let mut seen: BTreeMap<Signature, &str> = BTreeMap::new();
    for member in members {
        let mut sigs = BTreeSet::new();
        entry_signatures(member, 0, &mut sigs);
        for sig in sigs {
            if let Some(first) = seen.insert(sig, member.name()) {
                return Err(cx.err(ErrorKind::UnorderedAmbiguous {
                    signature: sig,
                    first: first.to_owned(),
                    second: member.name().to_owned(),
                }));
            }
        }
    }
    Ok(())
}

/// Names reserved in member lists: generated record structs have an `extra` field, and
/// unordered ones a `disk_order` field.
const RESERVED_MEMBER_NAMES: &[&str] = &["Extra", "DiskOrder"];

/// Group nesting after shared groups are expanded stays within [`MAX_DEPTH`].
fn check_group_depth(cx: &Cx<'_, '_>, members: &[Member], depth: usize) -> Result<(), SchemaError> {
    if depth > MAX_DEPTH {
        return Err(cx.err(ErrorKind::TooDeep(MAX_DEPTH)));
    }
    for member in members {
        if let Member::Group(g) = member {
            check_group_depth(cx, &g.members, depth.saturating_add(1))?;
        }
    }
    Ok(())
}

fn convert_members(
    cx: &mut Cx<'_, '_>,
    raw: &[RawMember],
    list: &str,
    parent: &Provenance,
) -> Result<Vec<Member>, SchemaError> {
    if cx.depth() > MAX_DEPTH {
        return Err(cx.err(ErrorKind::TooDeep(MAX_DEPTH)));
    }
    let mut members = Vec::with_capacity(raw.len());
    for (i, m) in raw.iter().enumerate() {
        let member = cx.scoped(format!("{list}[{i}]"), |cx| convert_member(cx, m, parent))?;
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

fn convert_member(
    cx: &mut Cx<'_, '_>,
    m: &RawMember,
    parent: &Provenance,
) -> Result<Member, SchemaError> {
    let prov = provenance(cx, parent, &m.prov())?;
    let kinds = [
        m.sig.is_some(),
        m.group.is_some(),
        m.group_ref.is_some(),
        m.any_until.is_some(),
    ];
    if kinds.iter().filter(|k| **k).count() != 1 {
        return Err(cx.err(ErrorKind::MemberKind));
    }
    reject(
        cx,
        m.count_from.is_some(),
        "count_from",
        "a member (only struct fields have siblings)",
    )?;
    let is_union = m.sig.is_some() && m.ty.as_deref() == Some("union");
    if !is_union {
        let context = "a member that is not `type = \"union\"`";
        reject(cx, m.decider.is_some(), "decider", context)?;
        reject(cx, m.secondary.is_some(), "secondary", context)?;
        reject(cx, m.variants.is_some(), "variants", context)?;
    }
    if let Some(group) = &m.group {
        return convert_group(cx, group, m, prov).map(Member::Group);
    }
    if let Some(name) = &m.group_ref {
        return convert_group_ref(cx, name, m, prov).map(Member::Group);
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
            provenance: prov,
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
    let ty = if is_union {
        reject_type_keys(cx, m, "a union member (variants carry their own types)")?;
        convert_union(cx, m, &prov)?
    } else {
        let spec = TypeSpec {
            ty: ty_text,
            targets: m.targets.as_ref(),
            count: m.count.as_ref(),
            count_from: None,
            element: m.element.as_deref(),
            base: m.base.as_deref(),
            values: m.values.as_ref(),
            bits: m.bits.as_ref(),
            size: m.size,
            field: m.field.as_ref(),
        };
        convert_type(cx, &spec, true, &prov)?
    };
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
        provenance: prov,
    }))
}

/// The decider of a union: `edid-prefix`, or `field:<Name>` (plus an optional
/// `secondary = "field:<Name>"`).
fn parse_decider(
    cx: &Cx<'_, '_>,
    decider: Option<&str>,
    secondary: Option<&str>,
) -> Result<Decider, SchemaError> {
    let field_name = |text: &str| {
        text.strip_prefix("field:")
            .filter(|name| is_pascal_case(name))
            .map(str::to_owned)
    };
    match decider {
        Some("edid-prefix") => {
            reject(
                cx,
                secondary.is_some(),
                "secondary",
                "a union with `decider = \"edid-prefix\"`",
            )?;
            Ok(Decider::EditorIdPrefix)
        }
        Some(text) => {
            let name = field_name(text)
                .ok_or_else(|| cx.err(ErrorKind::UnknownDecider(text.to_owned())))?;
            let secondary = match secondary {
                Some(text) => Some(
                    field_name(text)
                        .filter(|s| *s != name)
                        .ok_or_else(|| cx.err(ErrorKind::BadDeciderField(text.to_owned())))?,
                ),
                None => None,
            };
            Ok(Decider::Field { name, secondary })
        }
        None => Err(cx.err(ErrorKind::MissingKey("decider"))),
    }
}

/// `type = "union"` on a subrecord member: `decider = "edid-prefix"` plus `variants`, each
/// a `when` letter, a name and a simple type (scalars, enums, flags, `formid` with
/// `targets`, strings, `sig`, fixed opaque). Payloads no variant claims stay raw at
/// decode time.
fn convert_union(
    cx: &mut Cx<'_, '_>,
    m: &RawMember,
    prov: &Provenance,
) -> Result<FieldType, SchemaError> {
    let decider = parse_decider(cx, m.decider.as_deref(), m.secondary.as_deref())?;
    if decider != Decider::EditorIdPrefix {
        return Err(cx.err(ErrorKind::UnionPlacement));
    }
    let raw = m
        .variants
        .as_ref()
        .ok_or_else(|| cx.err(ErrorKind::MissingKey("variants")))?;
    let variants = convert_variants(cx, raw, None, prov)?;
    Ok(FieldType::Union { decider, variants })
}

/// The integer type of a union decider field (`None` when the field cannot decide: not an
/// earlier, ungated integer or enum sibling of at most 32 bits).
fn decider_base(fields: &[FieldDef], name: &str) -> Option<Scalar> {
    let field = fields.iter().find(|f| f.name == name)?;
    if field.since.is_some() || field.until.is_some() || field.if_remaining {
        return None;
    }
    let mut ty = &field.ty;
    while let FieldType::Ref { ty: inner, .. } = ty {
        ty = inner;
    }
    let base = match ty {
        FieldType::Scalar(s) => *s,
        FieldType::Enum { base, .. } => *base,
        _ => return None,
    };
    (base.is_integer() && base.size() <= 4).then_some(base)
}

/// `type = "union"` on a struct field: `decider = "field:<Sibling>"` (and an optional
/// `secondary = "field:<Sibling>"`), both earlier siblings in `earlier`; every variant has
/// the same fixed size, so a decider value no variant names keeps the bytes as read.
fn convert_field_union(
    cx: &mut Cx<'_, '_>,
    f: &RawField,
    earlier: &[FieldDef],
    prov: &Provenance,
) -> Result<FieldType, SchemaError> {
    for (present, key) in [
        (f.targets.is_some(), "targets"),
        (f.count.is_some(), "count"),
        (f.count_from.is_some(), "count_from"),
        (f.element.is_some(), "element"),
        (f.base.is_some(), "base"),
        (f.values.is_some(), "values"),
        (f.bits.is_some(), "bits"),
        (f.size.is_some(), "size"),
        (f.field.is_some(), "field"),
        (f.since.is_some(), "since"),
        (f.until.is_some(), "until"),
        (f.if_remaining.is_some(), "if_remaining"),
    ] {
        reject(
            cx,
            present,
            key,
            "a union field (variants carry their own types; it is never gated)",
        )?;
    }
    let decider = parse_decider(cx, f.decider.as_deref(), f.secondary.as_deref())?;
    let Decider::Field { name, secondary } = &decider else {
        return Err(cx.err(ErrorKind::UnionPlacement));
    };
    let base = decider_base(earlier, name)
        .ok_or_else(|| cx.err(ErrorKind::BadDeciderField(name.clone())))?;
    let secondary_base = match secondary {
        Some(s) => Some(
            decider_base(earlier, s)
                .ok_or_else(|| cx.err(ErrorKind::BadDeciderField(s.clone())))?,
        ),
        None => None,
    };
    let raw = f
        .variants
        .as_ref()
        .ok_or_else(|| cx.err(ErrorKind::MissingKey("variants")))?;
    let variants = convert_variants(cx, raw, Some((base, secondary_base)), prov)?;
    // One fixed size for every variant: the raw fallback keeps exactly that many bytes.
    let first = variants.first().and_then(|v| v.ty.fixed_size(None));
    for v in &variants {
        let size = v.ty.fixed_size(None);
        if size.is_none() || size != first || size == Some(0) {
            return Err(cx.err(ErrorKind::VariantSize {
                variant: v.name.clone(),
                size,
                first,
            }));
        }
    }
    Ok(FieldType::Union { decider, variants })
}

/// The `when` values of one variant.
fn variant_when(
    cx: &Cx<'_, '_>,
    when: &RawWhen,
    bases: Option<(Scalar, Option<Scalar>)>,
) -> Result<Vec<i64>, SchemaError> {
    let bad = || cx.err(ErrorKind::BadWhen(when.describe()));
    match (when, bases) {
        (RawWhen::Text(text), None) => match text.as_bytes() {
            [b] if b.is_ascii_alphanumeric() => Ok(vec![i64::from(*b)]),
            _ => Err(bad()),
        },
        (RawWhen::Number(n), Some((base, _))) => {
            if !base.fits(i128::from(*n)) {
                return Err(cx.err(ErrorKind::ValueOutOfRange {
                    value: i128::from(*n),
                    base: base.as_str(),
                }));
            }
            Ok(vec![*n])
        }
        (RawWhen::List(list), Some((base, _))) if !list.is_empty() => {
            let mut out = Vec::with_capacity(list.len());
            for n in list {
                if !base.fits(i128::from(*n)) {
                    return Err(cx.err(ErrorKind::ValueOutOfRange {
                        value: i128::from(*n),
                        base: base.as_str(),
                    }));
                }
                if out.contains(n) {
                    return Err(cx.err(ErrorKind::DuplicateValue(i128::from(*n))));
                }
                out.push(*n);
            }
            Ok(out)
        }
        _ => Err(bad()),
    }
}

/// A variant's `secondary_when`: an integer or four printable characters (read as a
/// little-endian `u32`), fitting the secondary field.
fn variant_secondary(
    cx: &Cx<'_, '_>,
    raw: Option<&RawKey>,
    secondary_base: Option<Scalar>,
) -> Result<Option<i64>, SchemaError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let (value, text) = match raw {
        RawKey::Number(n) => (Some(*n), n.to_string()),
        RawKey::Text(t) => (
            Signature::parse(t).map(|s| i64::from(u32::from_le_bytes(s.as_bytes()))),
            format!("{t:?}"),
        ),
    };
    match (value, secondary_base) {
        (Some(v), Some(base)) if base.fits(i128::from(v)) => Ok(Some(v)),
        _ => Err(cx.err(ErrorKind::BadSecondaryWhen(text))),
    }
}

/// The variants of a union. `bases` is `None` for `edid-prefix` (letters), else the
/// decider field's base and the secondary field's base (if any).
fn convert_variants(
    cx: &mut Cx<'_, '_>,
    raw: &[RawVariant],
    bases: Option<(Scalar, Option<Scalar>)>,
    prov: &Provenance,
) -> Result<Vec<UnionVariant>, SchemaError> {
    if raw.is_empty() {
        return Err(cx.err(ErrorKind::Empty));
    }
    let secondary_base = bases.and_then(|(_, s)| s);
    let mut variants: Vec<UnionVariant> = Vec::with_capacity(raw.len());
    for (i, v) in raw.iter().enumerate() {
        let variant = cx.scoped(format!("variants[{i}]"), |cx| {
            let when = variant_when(cx, &v.when, bases)?;
            // Without a `secondary` decider there is no secondary base, so any
            // `secondary_when` is rejected here.
            let secondary_when = variant_secondary(cx, v.secondary_when.as_ref(), secondary_base)?;
            for value in &when {
                let taken = variants
                    .iter()
                    .any(|seen| seen.when.contains(value) && seen.secondary_when == secondary_when);
                if taken {
                    return Err(cx.err(ErrorKind::DuplicateValue(i128::from(*value))));
                }
            }
            if v.ty == "union" || v.ty == "struct" || v.ty == "array" {
                return Err(cx.err(ErrorKind::UnexpectedKey {
                    key: "type",
                    context: format!("a union variant (type {:?})", v.ty),
                }));
            }
            let spec = TypeSpec {
                ty: &v.ty,
                targets: v.targets.as_ref(),
                count: None,
                count_from: None,
                element: None,
                base: v.base.as_deref(),
                values: v.values.as_ref(),
                bits: v.bits.as_ref(),
                size: v.size,
                field: None,
            };
            let ty = convert_type(cx, &spec, false, prov)?;
            Ok(UnionVariant {
                when,
                secondary_when,
                name: v.name.clone(),
                ty,
                note: v.note.clone(),
            })
        })?;
        variants.push(variant);
    }
    // `Other` names the raw fallback of a field union (codegen), so it is reserved.
    check_unique_names(cx, variants.iter().map(|v| v.name.as_str()), &["Other"])?;
    Ok(variants)
}

fn reject_type_keys(cx: &Cx<'_, '_>, m: &RawMember, context: &str) -> Result<(), SchemaError> {
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

fn convert_group(
    cx: &mut Cx<'_, '_>,
    name: &str,
    m: &RawMember,
    prov: Provenance,
) -> Result<GroupDef, SchemaError> {
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
    let members = convert_members(cx, raw, "member", &prov)?;
    match members.first() {
        None => return Err(cx.err(ErrorKind::Empty)),
        Some(Member::AnyUntil(_)) => return Err(cx.err(ErrorKind::GroupStart)),
        Some(_) => {}
    }
    Ok(GroupDef {
        name: name.to_owned(),
        cardinality,
        members,
        shared: None,
        note: m.note.clone(),
        provenance: prov,
    })
}

/// `group_ref = "<Name>"`: a shared group, expanded here. `name` overrides the member
/// name (default: the shared definition's name).
fn convert_group_ref(
    cx: &mut Cx<'_, '_>,
    shared: &str,
    m: &RawMember,
    prov: Provenance,
) -> Result<GroupDef, SchemaError> {
    for (present, key) in [
        (m.ty.is_some(), "type"),
        (m.member.is_some(), "member"),
        (m.size.is_some(), "size"),
        (m.size_by_version.is_some(), "size_by_version"),
    ] {
        reject(cx, present, key, "a `group_ref` member")?;
    }
    reject_type_keys(cx, m, "a `group_ref` member")?;
    let cardinality = cardinality(cx, m.card.as_deref())?;
    let def = resolve(cx, shared)?;
    let CommonBody::Group(members) = def.body else {
        return Err(cx.err(ErrorKind::RefKind {
            name: shared.to_owned(),
            expected: "group",
        }));
    };
    Ok(GroupDef {
        name: m.name.clone().unwrap_or_else(|| shared.to_owned()),
        cardinality,
        members,
        shared: Some(def.name),
        note: m.note.clone(),
        provenance: prov,
    })
}

fn convert_fields(
    cx: &mut Cx<'_, '_>,
    raw: &[RawField],
    parent: &Provenance,
) -> Result<Vec<FieldDef>, SchemaError> {
    if cx.depth() > MAX_DEPTH {
        return Err(cx.err(ErrorKind::TooDeep(MAX_DEPTH)));
    }
    if raw.is_empty() {
        return Err(cx.err(ErrorKind::Empty));
    }
    let mut fields: Vec<FieldDef> = Vec::with_capacity(raw.len());
    for (i, f) in raw.iter().enumerate() {
        let field = cx.scoped(format!("field[{i}]"), |cx| {
            let prov = provenance(cx, parent, &f.prov())?;
            version_range(cx, f.since, f.until)?;
            if f.ty == "union" {
                let ty = convert_field_union(cx, f, &fields, &prov)?;
                return Ok(FieldDef {
                    name: f.name.clone(),
                    ty,
                    since: None,
                    until: None,
                    if_remaining: false,
                    note: f.note.clone(),
                    provenance: prov,
                });
            }
            for (present, key) in [
                (f.decider.is_some(), "decider"),
                (f.secondary.is_some(), "secondary"),
                (f.variants.is_some(), "variants"),
            ] {
                reject(cx, present, key, "a field that is not `type = \"union\"`")?;
            }
            let spec = TypeSpec {
                ty: &f.ty,
                targets: f.targets.as_ref(),
                count: f.count.as_ref(),
                count_from: f.count_from.as_deref(),
                element: f.element.as_deref(),
                base: f.base.as_deref(),
                values: f.values.as_ref(),
                bits: f.bits.as_ref(),
                size: f.size,
                field: f.field.as_ref(),
            };
            let ty = convert_type(cx, &spec, false, &prov)?;
            if let FieldType::Array {
                count: ArrayCount::From(source),
                ..
            } = &ty
            {
                let ok = fields.iter().any(|s| {
                    s.name == *source
                        && s.since.is_none()
                        && s.until.is_none()
                        && !s.if_remaining
                        && matches!(s.ty, FieldType::Scalar(sc) if sc.is_integer())
                });
                if !ok {
                    return Err(cx.err(ErrorKind::BadCountFrom(source.clone())));
                }
            }
            Ok(FieldDef {
                name: f.name.clone(),
                ty,
                since: f.since,
                until: f.until,
                if_remaining: f.if_remaining.unwrap_or(false),
                note: f.note.clone(),
                provenance: prov,
            })
        })?;
        fields.push(field);
    }
    check_unique_names(cx, fields.iter().map(|f| f.name.as_str()), &[])?;
    // A field that reads to the end of the payload, or that is present only when bytes
    // remain, leaves nothing for a later field: it must be last.
    let last = fields.len().saturating_sub(1);
    for (i, f) in fields.iter().enumerate() {
        if i != last && (f.if_remaining || f.ty.reads_to_end()) {
            return Err(cx.err(ErrorKind::VariableNotLast(f.name.clone())));
        }
    }
    Ok(fields)
}

fn version_range(cx: &Cx<'_, '_>, min: Option<u16>, max: Option<u16>) -> Result<(), SchemaError> {
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
    cx: &mut Cx<'_, '_>,
    spec: &TypeSpec<'_>,
    is_member: bool,
    prov: &Provenance,
) -> Result<FieldType, SchemaError> {
    let context = format!("type {:?}", spec.ty);
    let allow = |cx: &Cx<'_, '_>, allowed: &[&str]| -> Result<(), SchemaError> {
        for (present, key) in [
            (spec.targets.is_some(), "targets"),
            (spec.count.is_some(), "count"),
            (spec.count_from.is_some(), "count_from"),
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
    if let Some(name) = spec.ty.strip_prefix("ref:") {
        allow(cx, &[])?;
        let def = resolve(cx, name)?;
        let CommonBody::Type(ty) = def.body else {
            return Err(cx.err(ErrorKind::RefKind {
                name: name.to_owned(),
                expected: "type",
            }));
        };
        return Ok(FieldType::Ref {
            name: def.name,
            ty: Box::new(ty),
        });
    }
    if spec.ty == "union" {
        return Err(cx.err(ErrorKind::UnionPlacement));
    }
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
        "wstring" => {
            allow(cx, &[])?;
            FieldType::WString
        }
        "bstring" => {
            allow(cx, &[])?;
            FieldType::BString
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
                fields: convert_fields(cx, raw, prov)?,
            }
        }
        "array" => {
            allow(
                cx,
                &[
                    "count",
                    "count_from",
                    "element",
                    "targets",
                    "base",
                    "values",
                    "bits",
                    "field",
                    "size",
                ],
            )?;
            let count = match (spec.count, spec.count_from) {
                (Some(_), Some(_)) => {
                    return Err(cx.err(ErrorKind::UnexpectedKey {
                        key: "count_from",
                        context: "an array that also sets `count`".into(),
                    }))
                }
                (None, Some(source)) => ArrayCount::From(source.to_owned()),
                (None, None) => return Err(cx.err(ErrorKind::MissingKey("count"))),
                (Some(RawCount::Word(w)), None) if w == "rest" => ArrayCount::Rest,
                (Some(RawCount::Number(n)), None) => match u32::try_from(*n) {
                    Ok(n) if n > 0 => ArrayCount::Fixed(n),
                    _ => return Err(cx.err(ErrorKind::BadCount(n.to_string()))),
                },
                (Some(RawCount::Word(w)), None) => {
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
                count_from: None,
                element: None,
                base: spec.base,
                values: spec.values,
                bits: spec.bits,
                // On an array member, `size` is the payload size, not the element's.
                size: if is_member { None } else { spec.size },
                field: spec.field,
            };
            let element = cx.scoped("element".into(), |cx| {
                convert_type(cx, &element_spec, false, prov)
            })?;
            // Elements must be self-delimiting and consume at least one byte.
            if element.reads_to_end() || element.fixed_size(None) == Some(0) {
                return Err(cx.err(ErrorKind::BadElement));
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

fn targets(cx: &Cx<'_, '_>, raw: Option<&Vec<String>>) -> Result<Vec<Signature>, SchemaError> {
    let mut out = Vec::new();
    for t in raw.into_iter().flatten() {
        let sig = signature(cx, t)?;
        if !out.contains(&sig) {
            out.push(sig);
        }
    }
    Ok(out)
}

fn int_base(
    cx: &Cx<'_, '_>,
    base: Option<&str>,
    what: &'static str,
) -> Result<Scalar, SchemaError> {
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
    cx: &Cx<'_, '_>,
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
///
/// Per-version sizes are checked at every form version where the layout can change
/// inside the entry's range: the range bounds, and each gated field's `since`,
/// `since - 1`, `until` and `until + 1` clipped to the range. Between two of those
/// versions the layout is constant, so this covers the whole range. Ranges must not
/// overlap.
fn check_size(
    cx: &Cx<'_, '_>,
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
    let bounds_of = |e: &VersionedSize| (e.min.unwrap_or(0), e.max.unwrap_or(u16::MAX));
    for (i, first) in by_version.iter().enumerate() {
        let (alo, ahi) = bounds_of(first);
        for second in by_version.iter().skip(i.saturating_add(1)) {
            let (blo, bhi) = bounds_of(second);
            if alo <= bhi && blo <= ahi {
                return Err(cx.err(ErrorKind::OverlappingVersions {
                    first: (first.min, first.max),
                    second: (second.min, second.max),
                }));
            }
        }
    }
    let mut gates = BTreeSet::new();
    ty.version_bounds(&mut gates);
    for entry in by_version {
        let (lo, hi) = bounds_of(entry);
        let mut probes = BTreeSet::from([lo, hi]);
        for (since, until) in &gates {
            let candidates = [
                *since,
                since.and_then(|s| s.checked_sub(1)),
                *until,
                until.and_then(|u| u.checked_add(1)),
            ];
            for v in candidates.into_iter().flatten() {
                probes.insert(v.clamp(lo, hi));
            }
        }
        // A layout that is variable-size (opaque rest, strings) cannot be checked; the
        // entry is then a documented fact only.
        for probe in probes {
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
    }
    Ok(())
}
