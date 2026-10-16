//! Field paths and values (M2.E2).
//!
//! A field path names one subrecord of a record, or one member inside its payload:
//!
//! ```text
//! path  = SIG [ "[" n "]" ] { "." Member { "[" n "]" } }
//! ```
//!
//! `SIG` is the subrecord signature, `SIG[n]` its `n`-th occurrence in the record (0-based;
//! `SIG` alone is `SIG[0]`), `.Member` a field of the subrecord's struct layout (schema
//! member names, ASCII case-insensitive) and `[n]` an element of an array field.
//!
//! Layouts come from the schema (`kitkraft-schema`, the builtin definitions). Values are
//! JSON: a number, or a string parsed by the field's type:
//!
//! | Type            | Accepted                                                        |
//! | --------------- | --------------------------------------------------------------- |
//! | integers        | JSON integer, or decimal / `0x` hex text; range-checked by width |
//! | `f32` / `f64`   | JSON number or decimal text; finite, and in range for `f32`     |
//! | enum            | a value name (ASCII case-insensitive) or an integer             |
//! | flags           | an integer, or bit names joined by `\|`                          |
//! | FormID          | `Plugin.esm:00ABCDEF` (mapped to the target's master list), a raw hex FormID stored as given, or `null` (FormID 0, no form; shown as `null`) |
//! | `sig`           | four ASCII characters                                           |
//! | zstring         | text (no NUL), written NUL-terminated                           |
//! | lstring         | text, written as an inline zstring (patches are non-localized)  |
//! | wstring/bstring | text, written with its length prefix                            |
//! | opaque, structs, arrays | only `hex:0A0B...` raw bytes (or a member path)          |
//!
//! `hex:` is accepted for any type; for a fixed-size type the byte count must match, and a
//! zstring or (non-localized) lstring must still be one NUL-terminated string. An object
//! `{"mul": x}` or `{"add": x}` scales or offsets the current value of a typed number
//! (integers are rounded to the nearest value, ties away from zero, then range-checked; an
//! integral `add` is exact on 64-bit fields, while `mul` and a fractional `add` go through
//! `f64` and are refused beyond 2^53). `{"min": x}` raises a typed number to at least `x`,
//! `{"max": x}` lowers it to at most `x`, and `{"clamp": [lo, hi]}` does both (`build` spec
//! v2, M3.E6; on integers the bounds round inwards and the value stays exact). Non-finite
//! numbers (`inf`, `nan`, `1e400`) are `range` errors.
//!
//! `FULL` and `DESC` are lstrings (`uesp-skyrim` fact in `plan/execution-plan.md`, "Format
//! facts M2 relies on"; lane B's M2.B2 types them, with corpus evidence, on every record
//! type the schema defines). In a record type the schema does not define (`CELL`, `NPC_`,
//! ...) they are treated as lstrings anyway and [`Layout::fallback`] says so. Other
//! signatures the schema leaves untyped or opaque are lstrings when the corpus lstring
//! table says so ([`super::lstrings`]), again with a [`Layout::fallback`] note.

use kitkraft_schema::{ArrayCount, FieldDef, FieldType, NamedValue, Scalar};
use serde_json::{json, Value};

/// A field path or value error: names the path and the rule it broke.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{path}: {message} ({rule})")]
pub struct FieldError {
    /// The path as written.
    pub path: String,
    /// A stable rule id: `path-syntax`, `no-subrecord`, `no-member`, `layout`, `type`,
    /// `range`, `opaque`, `formref`, `payload`.
    pub rule: &'static str,
    /// What is wrong.
    pub message: String,
}

impl FieldError {
    pub(crate) fn new(path: &str, rule: &'static str, message: impl Into<String>) -> FieldError {
        FieldError {
            path: path.to_string(),
            rule,
            message: message.into(),
        }
    }
}

/// One step inside a subrecord payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A struct member, by schema name.
    Member(String),
    /// An array element.
    Index(usize),
}

/// A parsed field path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPath {
    /// The path as written.
    pub text: String,
    /// The subrecord signature.
    pub signature: [u8; 4],
    /// Which occurrence of the signature (0-based).
    pub occurrence: usize,
    /// Members and indices inside the payload.
    pub steps: Vec<Step>,
}

impl FieldPath {
    /// The signature as text.
    pub fn signature_str(&self) -> String {
        String::from_utf8_lossy(&self.signature).into_owned()
    }
}

/// Parses a field path (grammar in the module docs).
pub fn parse_path(text: &str) -> Result<FieldPath, FieldError> {
    let bad = |message: &str| FieldError::new(text, "path-syntax", message);
    if !crate::cmdline::is_field_path(text) {
        return Err(bad(
            "expected SIG, SIG[n], SIG.Member or SIG.Member[n] (SIG: four of A-Z, 0-9, _)",
        ));
    }
    let bytes = text.as_bytes();
    let signature: [u8; 4] = bytes
        .get(..4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| bad("missing signature"))?;
    let mut rest = text.get(4..).unwrap_or_default();
    let mut occurrence = 0usize;
    if let Some(after) = rest.strip_prefix('[') {
        let (n, tail) = index_prefix(after).ok_or_else(|| bad("bad occurrence index"))?;
        occurrence = n;
        rest = tail;
    }
    let mut steps = Vec::new();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let len = after
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            let name = after.get(..len).ok_or_else(|| bad("bad member name"))?;
            steps.push(Step::Member(name.to_string()));
            rest = after.get(len..).unwrap_or_default();
        } else if let Some(after) = rest.strip_prefix('[') {
            if steps.is_empty() {
                return Err(bad("a second occurrence index needs a member first"));
            }
            let (n, tail) = index_prefix(after).ok_or_else(|| bad("bad array index"))?;
            steps.push(Step::Index(n));
            rest = tail;
        } else {
            return Err(bad("unexpected character"));
        }
    }
    Ok(FieldPath {
        text: text.to_string(),
        signature,
        occurrence,
        steps,
    })
}

/// `123]rest` -> `(123, rest)`.
fn index_prefix(text: &str) -> Option<(usize, &str)> {
    let (digits, tail) = text.split_once(']')?;
    Some((digits.parse().ok()?, tail))
}

/// What an edit does to a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Replace it with this JSON value.
    Set(Value),
    /// Multiply a typed number.
    Mul(f64),
    /// Add to a typed number.
    Add(f64),
    /// Raise a typed number to at least this (the value's lower bound; `build` spec v2).
    Min(f64),
    /// Lower a typed number to at most this (the value's upper bound).
    Max(f64),
    /// Keep a typed number within `[lo, hi]` (`lo <= hi`).
    Clamp(f64, f64),
}

impl Op {
    /// Whether this op reads the current value (every op but a plain value).
    pub fn is_operator(&self) -> bool {
        !matches!(self, Op::Set(_))
    }

    /// The bounds of `min`, `max` and `clamp`.
    fn bounds(&self) -> Option<(Option<f64>, Option<f64>)> {
        match self {
            Op::Min(lo) => Some((Some(*lo), None)),
            Op::Max(hi) => Some((None, Some(*hi))),
            Op::Clamp(lo, hi) => Some((Some(*lo), Some(*hi))),
            _ => None,
        }
    }
}

/// The operator names an object value may use.
pub const OPERATORS: [&str; 5] = ["mul", "add", "min", "max", "clamp"];

/// Reads an op from its JSON form: `{"mul": x}`, `{"add": x}`, `{"min": x}` (at least x),
/// `{"max": x}` (at most x), `{"clamp": [lo, hi]}`, or any other value to set.
pub fn parse_op(path: &str, value: &Value) -> Result<Op, FieldError> {
    let Value::Object(map) = value else {
        return Ok(Op::Set(value.clone()));
    };
    let number = |key: &str, v: &Value| {
        v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| {
            FieldError::new(path, "type", format!("`{key}` takes a finite JSON number"))
        })
    };
    let mut entries = map.iter();
    let (Some((key, v)), None) = (entries.next(), entries.next()) else {
        return Err(FieldError::new(
            path,
            "type",
            "an object value must have exactly one operator: {\"mul\": x}, {\"add\": x}, {\"min\": x}, {\"max\": x} or {\"clamp\": [lo, hi]}",
        ));
    };
    match key.as_str() {
        "mul" => Ok(Op::Mul(number(key, v)?)),
        "add" => Ok(Op::Add(number(key, v)?)),
        "min" => Ok(Op::Min(number(key, v)?)),
        "max" => Ok(Op::Max(number(key, v)?)),
        "clamp" => {
            let pair = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| {
                FieldError::new(path, "type", "`clamp` takes [lo, hi]: two finite numbers")
            })?;
            let lo = number(key, pair.first().unwrap_or(&Value::Null))?;
            let hi = number(key, pair.get(1).unwrap_or(&Value::Null))?;
            if lo > hi {
                return Err(FieldError::new(
                    path,
                    "range",
                    format!("`clamp` [{lo}, {hi}]: lo is above hi"),
                ));
            }
            Ok(Op::Clamp(lo, hi))
        }
        other => Err(FieldError::new(
            path,
            "type",
            format!(
                "unknown operator `{other}`; an object value is one of {}",
                OPERATORS.join(", ")
            ),
        )),
    }
}

/// The layout of one subrecord, and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The payload type.
    pub ty: FieldType,
    /// Set when the type is the `FULL`/`DESC` lstring fallback rather than a schema
    /// definition (the text is the limitation to report).
    pub fallback: Option<String>,
}

/// Signatures that are lstrings even where the schema does not type them (module docs).
pub const LSTRING_FALLBACK: [&[u8; 4]; 2] = [b"FULL", b"DESC"];

/// The limitation reported when the fallback is used.
pub fn fallback_note(signature: &str) -> String {
    format!(
        "{signature} treated as an lstring (uesp-skyrim fact, plan/execution-plan.md): the \
         builtin schema does not define this record type, or keeps {signature} opaque in it"
    )
}

/// The layout of subrecord `index` of a record of type `record` whose subrecord
/// signatures are `subs`: the schema's for the member that subrecord is placed in, found by
/// its position (`kitkraft_doc::typed`, the generated decoder's matching rules), not by its
/// signature alone, since a signature can mean different members at different positions
/// (PNDT CNAM/FNAM/GNAM; NAM1..NAM5, VNAM, CNAM, ANAM inside the PCBN/PKIN/GBFM component
/// blocks). `None` when the schema knows nothing about it (no record definition, a
/// signature the definition never names, raw bytes inside a component block, or a
/// position no member accepts: its layout is unknown and the payload opaque) and it is
/// not an lstring by the fallback.
pub fn layout_at(record: [u8; 4], subs: &[[u8; 4]], index: usize) -> Option<Layout> {
    let sub = subs.get(index).copied()?;
    let schema_ty = kitkraft_schema::builtin().ok().and_then(|schema| {
        kitkraft_doc::typed::layout_at(schema, record, subs, index)
            .ok()
            .flatten()
            .map(|l| l.ty.clone())
    });
    finish(record, sub, schema_ty)
}

/// [`layout_at`] for every subrecord of a record, in order, with one placement pass.
pub fn layouts(record: [u8; 4], subs: &[[u8; 4]]) -> Vec<Option<Layout>> {
    let schema = kitkraft_schema::builtin().ok();
    let placed = schema.and_then(|s| kitkraft_doc::typed::placements(s, record, subs));
    subs.iter()
        .enumerate()
        .map(|(index, sub)| {
            let schema_ty = schema.and_then(|schema| {
                let p = placed.as_ref().and_then(|p| p.get(index));
                kitkraft_doc::typed::layout_of(schema, record, *sub, index, p)
                    .ok()
                    .flatten()
                    .map(|l| l.ty.clone())
            });
            finish(record, *sub, schema_ty)
        })
        .collect()
}

/// The layout from the schema's type at the subrecord's position, or the lstring fallback
/// (FULL/DESC by the uesp-skyrim fact, else the corpus table `super::lstrings`) where the
/// schema has nothing or only opaque bytes.
fn finish(record: [u8; 4], sub: [u8; 4], schema_ty: Option<FieldType>) -> Option<Layout> {
    let untyped = || {
        if LSTRING_FALLBACK.contains(&&sub) {
            Some(Layout {
                ty: FieldType::LString,
                fallback: Some(fallback_note(&String::from_utf8_lossy(&sub))),
            })
        } else if super::lstrings::class(record, sub) == super::lstrings::Class::LString {
            Some(Layout {
                ty: FieldType::LString,
                fallback: Some(super::lstrings::table_note(record, sub)),
            })
        } else {
            None
        }
    };
    match schema_ty {
        None => untyped(),
        Some(ty) if ty.is_opaque() => untyped().or(Some(Layout { ty, fallback: None })),
        Some(ty) => Some(Layout { ty, fallback: None }),
    }
}

/// The lstrings of a subrecord payload laid out as `ty`: `(member label, start, len)`,
/// wherever the schema places them (the whole payload, any struct member, array element
/// or union variant: M3.E4, `kitkraft_doc::typed::lstring_spans`). A whole-subrecord
/// lstring is the whole payload whatever its length (callers report a localized one that
/// is not 4 bytes); a payload that does not decode as `ty` has no spans.
pub fn lstring_spans(
    ty: &FieldType,
    payload: &[u8],
    ctx: &kitkraft_doc::typed::Ctx,
) -> Vec<(String, usize, usize)> {
    if matches!(ty, FieldType::LString) {
        return vec![(String::new(), 0, payload.len())];
    }
    kitkraft_doc::typed::lstring_spans(ty, payload, ctx)
        .map(|spans| spans.into_iter().map(|s| (s.member, s.at, s.len)).collect())
        .unwrap_or_default()
}

/// How values are encoded into the target plugin.
pub struct ValueCtx<'a> {
    /// Whether the target plugin is localized (patches never are in M2; an lstring set
    /// into a localized target is refused).
    pub localized_target: bool,
    /// Whether the plugin the payload was copied from is localized (decides how an
    /// lstring's old value is shown).
    pub localized_source: bool,
    /// The record's form version (decides which gated fields are present).
    pub form_version: u16,
    /// Maps a `formref` (plugin file name, object ID) to the FormID stored in the target.
    pub formref: &'a dyn Fn(&str, u32) -> Result<u32, String>,
}

impl std::fmt::Debug for ValueCtx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValueCtx")
            .field("localized_target", &self.localized_target)
            .field("localized_source", &self.localized_source)
            .field("form_version", &self.form_version)
            .finish_non_exhaustive()
    }
}

/// Where a path's value sits in a payload, and its type.
#[derive(Debug, Clone, PartialEq)]
pub struct Located<'t> {
    /// Byte offset in the payload.
    pub start: usize,
    /// Byte length.
    pub len: usize,
    /// The value's type.
    pub ty: &'t FieldType,
}

/// Finds the bytes `steps` name inside `payload`, laid out as `ty`.
pub fn locate<'t>(
    path: &str,
    ty: &'t FieldType,
    payload: &[u8],
    steps: &[Step],
    form_version: u16,
) -> Result<Located<'t>, FieldError> {
    let mut ty = deref(ty);
    let mut start = 0usize;
    let mut len = payload.len();
    // The integer members before the current one in its struct (union deciders).
    let mut siblings: Vec<(&str, i128)> = Vec::new();
    for step in steps {
        let window = payload
            .get(start..start.saturating_add(len))
            .ok_or_else(|| FieldError::new(path, "payload", "payload shorter than its layout"))?;
        match (step, ty) {
            (Step::Member(name), FieldType::Struct { fields }) => {
                let (off, flen, field, before) =
                    member_span(path, fields, window, name, form_version)?;
                start = start
                    .checked_add(off)
                    .ok_or_else(|| FieldError::new(path, "payload", "offset overflow"))?;
                len = flen;
                ty = deref(&field.ty);
                siblings = before;
            }
            (
                Step::Member(name),
                FieldType::Union {
                    decider:
                        kitkraft_schema::Decider::Field {
                            name: key,
                            secondary,
                        },
                    variants,
                },
            ) => {
                // A struct field union: the step names a variant, which must be the one the
                // record's own decider fields select (`Value1.Float` where ValueType is 1).
                let wanted = variants
                    .iter()
                    .find(|v| v.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| {
                        let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
                        FieldError::new(
                            path,
                            "no-member",
                            format!("no variant `{name}`; variants: {}", names.join(", ")),
                        )
                    })?;
                let value = |field: &str| {
                    siblings
                        .iter()
                        .rev()
                        .find(|(f, _)| f.eq_ignore_ascii_case(field))
                        .and_then(|(_, v)| i64::try_from(*v).ok())
                };
                let key_value = value(key).ok_or_else(|| {
                    FieldError::new(path, "layout", format!("union decider `{key}` not read"))
                })?;
                let second = secondary.as_deref().and_then(value);
                let chosen = kitkraft_schema::select_variant(variants, key_value, second);
                if chosen.map(|v| v.name.as_str()) != Some(wanted.name.as_str()) {
                    let here = chosen.map_or_else(
                        || "raw bytes (no variant names this value)".to_string(),
                        |v| format!("`{}`", v.name),
                    );
                    let mut deciders = format!("{key} = {key_value}");
                    if let (Some(s), Some(v)) = (secondary, second) {
                        deciders.push_str(&format!(", {s} = {v:#X}"));
                    }
                    return Err(FieldError::new(
                        path,
                        "layout",
                        format!(
                            "this entry's {deciders} selects {here}, not `{}`; set the decider first",
                            wanted.name
                        ),
                    ));
                }
                ty = deref(&wanted.ty);
            }
            (Step::Index(i), FieldType::Array { element, .. }) => {
                let size = element
                    .fixed_size(Some(form_version))
                    .and_then(|s| usize::try_from(s).ok())
                    .filter(|s| *s > 0)
                    .ok_or_else(|| {
                        FieldError::new(
                            path,
                            "layout",
                            "indexing needs an array of fixed-size elements",
                        )
                    })?;
                let off = i
                    .checked_mul(size)
                    .filter(|end| end.checked_add(size).is_some_and(|e| e <= window.len()))
                    .ok_or_else(|| {
                        FieldError::new(
                            path,
                            "no-member",
                            format!(
                                "index {i} is past the end of the array ({} elements)",
                                window.len().checked_div(size).unwrap_or(0)
                            ),
                        )
                    })?;
                start = start.saturating_add(off);
                len = size;
                ty = deref(element);
            }
            (Step::Member(name), _) => {
                return Err(FieldError::new(
                    path,
                    "no-member",
                    format!(
                        "`{name}`: the value here is not a struct ({})",
                        type_name(ty)
                    ),
                ))
            }
            (Step::Index(i), _) => {
                return Err(FieldError::new(
                    path,
                    "no-member",
                    format!("[{i}]: the value here is not an array ({})", type_name(ty)),
                ))
            }
        }
    }
    Ok(Located { start, len, ty })
}

/// The variant of a union layout that applies to a record whose `EDID` payload starts with
/// `edid_first` (`decider = "edid-prefix"`); `ty` itself for any other type. A union no
/// variant claims stays a union (handled as opaque).
pub fn select_variant(ty: &FieldType, edid_first: Option<u8>) -> &FieldType {
    match deref(ty) {
        FieldType::Union {
            decider: kitkraft_schema::Decider::EditorIdPrefix,
            variants,
        } => edid_first
            .and_then(|b| kitkraft_schema::select_variant(variants, i64::from(b), None))
            .map_or(ty, |v| deref(&v.ty)),
        other => other,
    }
}

fn deref(ty: &FieldType) -> &FieldType {
    match ty {
        FieldType::Ref { ty, .. } => deref(ty),
        other => other,
    }
}

/// The integer base of a field that can count an array or decide a union: an integer
/// scalar or an enum.
fn int_base(ty: &FieldType) -> Option<Scalar> {
    match deref(ty) {
        FieldType::Scalar(s) if s.is_integer() => Some(*s),
        FieldType::Enum { base, .. } => Some(*base),
        _ => None,
    }
}

/// Offset, length and definition of member `name` in a struct payload, and the integer
/// values of the members before it (array counts, union deciders).
type Span<'f> = (usize, usize, &'f FieldDef, Vec<(&'f str, i128)>);

fn member_span<'f>(
    path: &str,
    fields: &'f [FieldDef],
    payload: &[u8],
    name: &str,
    form_version: u16,
) -> Result<Span<'f>, FieldError> {
    let mut cursor = 0usize;
    let mut ints: Vec<(&str, i128)> = Vec::new();
    for field in fields {
        if !field.present_in(Some(form_version)) {
            if field.name.eq_ignore_ascii_case(name) {
                return Err(FieldError::new(
                    path,
                    "no-member",
                    format!(
                        "`{}` is not present at form version {form_version}",
                        field.name
                    ),
                ));
            }
            continue;
        }
        let rest = payload.get(cursor..).unwrap_or_default();
        if field.if_remaining && rest.is_empty() {
            if field.name.eq_ignore_ascii_case(name) {
                return Err(FieldError::new(
                    path,
                    "no-member",
                    format!(
                        "`{}` is absent from this payload (optional tail)",
                        field.name
                    ),
                ));
            }
            continue;
        }
        let len = field_len(path, &field.ty, rest, &ints, form_version)?;
        if field.name.eq_ignore_ascii_case(name) {
            return Ok((cursor, len, field, ints));
        }
        if let Some(s) = int_base(&field.ty) {
            if let Some(v) = rest.get(..len).and_then(|b| read_int(s, b)) {
                ints.push((field.name.as_str(), v));
            }
        }
        cursor = cursor
            .checked_add(len)
            .ok_or_else(|| FieldError::new(path, "payload", "offset overflow"))?;
    }
    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    Err(FieldError::new(
        path,
        "no-member",
        format!("no member `{name}`; members: {}", names.join(", ")),
    ))
}

/// Bytes a value of `ty` takes at the start of `rest`.
fn field_len(
    path: &str,
    ty: &FieldType,
    rest: &[u8],
    ints: &[(&str, i128)],
    form_version: u16,
) -> Result<usize, FieldError> {
    let short = || FieldError::new(path, "payload", "payload shorter than its layout");
    let len = match deref(ty) {
        t if t.reads_to_end() => rest.len(),
        FieldType::WString => {
            let n = rest.get(..2).ok_or_else(short)?;
            usize::from(u16::from_le_bytes([
                *n.first().ok_or_else(short)?,
                *n.get(1).ok_or_else(short)?,
            ]))
            .saturating_add(2)
        }
        FieldType::BString => usize::from(*rest.first().ok_or_else(short)?).saturating_add(1),
        FieldType::Array {
            element,
            count: ArrayCount::From(name),
        } => {
            let n = ints
                .iter()
                .rev()
                .find(|(field, _)| field.eq_ignore_ascii_case(name))
                .and_then(|(_, v)| usize::try_from(*v).ok())
                .ok_or_else(|| {
                    FieldError::new(path, "layout", format!("count field `{name}` not read"))
                })?;
            let size = element
                .fixed_size(Some(form_version))
                .and_then(|s| usize::try_from(s).ok())
                .ok_or_else(|| FieldError::new(path, "layout", "variable-size array elements"))?;
            n.checked_mul(size).ok_or_else(short)?
        }
        FieldType::Struct { fields } => {
            let mut total = 0usize;
            let mut inner: Vec<(&str, i128)> = Vec::new();
            for field in fields {
                if !field.present_in(Some(form_version)) {
                    continue;
                }
                let sub = rest.get(total..).unwrap_or_default();
                if field.if_remaining && sub.is_empty() {
                    continue;
                }
                let len = field_len(path, &field.ty, sub, &inner, form_version)?;
                if let Some(s) = int_base(&field.ty) {
                    if let Some(v) = sub.get(..len).and_then(|b| read_int(s, b)) {
                        inner.push((field.name.as_str(), v));
                    }
                }
                total = total.checked_add(len).ok_or_else(short)?;
            }
            total
        }
        other => other
            .fixed_size(Some(form_version))
            .and_then(|s| usize::try_from(s).ok())
            .ok_or_else(|| {
                FieldError::new(
                    path,
                    "layout",
                    format!("cannot size a {} here", type_name(other)),
                )
            })?,
    };
    if len > rest.len() {
        return Err(short());
    }
    Ok(len)
}

fn read_int(s: Scalar, b: &[u8]) -> Option<i128> {
    Some(match s {
        Scalar::U8 => i128::from(*b.first()?),
        Scalar::I8 => i128::from(i8::from_le_bytes(b.try_into().ok()?)),
        Scalar::U16 => i128::from(u16::from_le_bytes(b.try_into().ok()?)),
        Scalar::I16 => i128::from(i16::from_le_bytes(b.try_into().ok()?)),
        Scalar::U32 => i128::from(u32::from_le_bytes(b.try_into().ok()?)),
        Scalar::I32 => i128::from(i32::from_le_bytes(b.try_into().ok()?)),
        Scalar::U64 => i128::from(u64::from_le_bytes(b.try_into().ok()?)),
        Scalar::I64 => i128::from(i64::from_le_bytes(b.try_into().ok()?)),
        Scalar::F32 | Scalar::F64 => return None,
    })
}

fn read_float(s: Scalar, b: &[u8]) -> Option<f64> {
    match s {
        Scalar::F32 => Some(f64::from(f32::from_le_bytes(b.try_into().ok()?))),
        Scalar::F64 => Some(f64::from_le_bytes(b.try_into().ok()?)),
        other => read_int(other, b).map(|v| v as f64),
    }
}

/// A short name for a type in messages.
pub fn type_name(ty: &FieldType) -> String {
    match ty {
        FieldType::Scalar(s) => s.as_str().to_string(),
        FieldType::FormId { .. } => "formid".into(),
        FieldType::Sig => "sig".into(),
        FieldType::ZString => "zstring".into(),
        FieldType::LString => "lstring".into(),
        FieldType::WString => "wstring".into(),
        FieldType::BString => "bstring".into(),
        FieldType::Enum { base, .. } => format!("enum ({})", base.as_str()),
        FieldType::Flags { base, .. } => format!("flags ({})", base.as_str()),
        FieldType::Array { .. } => "array".into(),
        FieldType::Struct { .. } => "struct".into(),
        FieldType::Opaque { .. } => "opaque".into(),
        FieldType::Empty => "empty".into(),
        FieldType::Ref { name, .. } => format!("ref:{name}"),
        FieldType::Union { .. } => "union".into(),
    }
}

/// Encodes the new bytes for a value of type `ty` whose current bytes are `old`.
pub fn encode_value(
    path: &str,
    ty: &FieldType,
    old: &[u8],
    op: &Op,
    ctx: &ValueCtx<'_>,
) -> Result<Vec<u8>, FieldError> {
    let ty = deref(ty);
    let type_err = |message: String| FieldError::new(path, "type", message);
    if let Op::Set(Value::String(text)) = op {
        if let Some(hex) = text.strip_prefix("hex:") {
            let bytes = parse_hex(hex).ok_or_else(|| {
                type_err("hex: needs an even number of hex digits (spaces allowed)".into())
            })?;
            // A string set as raw bytes must still be one NUL-terminated string: the game
            // reads it up to the first NUL (an lstring of a non-localized plugin too).
            let text_type = matches!(ty, FieldType::ZString)
                || (matches!(ty, FieldType::LString) && !ctx.localized_target);
            if text_type && !is_one_zstring(&bytes) {
                return Err(type_err(format!(
                    "a {} set as hex: must be one NUL-terminated string (ending in 00, no other 00); got {} byte(s)",
                    type_name(ty),
                    bytes.len()
                )));
            }
            if let Some(size) = ty
                .fixed_size(Some(ctx.form_version))
                .and_then(|s| usize::try_from(s).ok())
            {
                if bytes.len() != size && !matches!(ty, FieldType::Opaque { size: None }) {
                    return Err(FieldError::new(
                        path,
                        "range",
                        format!(
                            "{} takes {size} bytes, hex: gave {}",
                            type_name(ty),
                            bytes.len()
                        ),
                    ));
                }
            }
            return Ok(bytes);
        }
    }
    match ty {
        FieldType::Scalar(s) if s.is_integer() => {
            let value = match op {
                Op::Set(v) => parse_int(v).ok_or_else(|| {
                    type_err(format!(
                        "{} takes an integer (decimal or 0x hex)",
                        s.as_str()
                    ))
                })?,
                _ => {
                    let current = read_int(*s, old).ok_or_else(|| {
                        FieldError::new(path, "payload", "current value unreadable")
                    })?;
                    int_op(path, current, op)?
                }
            };
            encode_int(*s, value).ok_or_else(|| {
                FieldError::new(
                    path,
                    "range",
                    format!("{value} does not fit {}", s.as_str()),
                )
            })
        }
        FieldType::Scalar(s) => {
            let value = match op {
                Op::Set(v) => parse_float(v).map_err(|e| match e {
                    FloatError::NotANumber => type_err(format!("{} takes a number", s.as_str())),
                    FloatError::NotFinite(text) => FieldError::new(
                        path,
                        "range",
                        format!("`{text}` is not a finite number (infinite, NaN, or beyond f64); {} takes a finite value", s.as_str()),
                    ),
                })?,
                _ => {
                    let current = read_float(*s, old).ok_or_else(|| {
                        FieldError::new(path, "payload", "current value unreadable")
                    })?;
                    match op {
                        Op::Mul(m) => current * m,
                        Op::Add(a) => current + a,
                        _ => match op.bounds() {
                            Some((lo, hi)) => {
                                let raised = lo.map_or(current, |lo| current.max(lo));
                                hi.map_or(raised, |hi| raised.min(hi))
                            }
                            None => current,
                        },
                    }
                }
            };
            if !value.is_finite() {
                return Err(FieldError::new(path, "range", "not a finite number"));
            }
            match s {
                Scalar::F32 => {
                    let narrow = value as f32;
                    if !narrow.is_finite() {
                        return Err(FieldError::new(
                            path,
                            "range",
                            format!("{value} does not fit f32"),
                        ));
                    }
                    Ok(narrow.to_le_bytes().to_vec())
                }
                _ => Ok(value.to_le_bytes().to_vec()),
            }
        }
        _ if !matches!(op, Op::Set(_)) => Err(type_err(format!(
            "operators ({}) apply to typed numbers, not to a {}; give a value",
            OPERATORS.join(", "),
            type_name(ty)
        ))),
        FieldType::Enum { base, values } => {
            let Op::Set(v) = op else {
                return Err(type_err("enum".into()));
            };
            let value = match v.as_str().and_then(|name| by_name(values, name)) {
                Some(value) => value,
                None => parse_int(v).ok_or_else(|| {
                    let names: Vec<&str> = values.iter().map(|n| n.name.as_str()).collect();
                    type_err(format!(
                        "enum takes one of {} or an integer",
                        names.join(", ")
                    ))
                })?,
            };
            encode_int(*base, value).ok_or_else(|| {
                FieldError::new(
                    path,
                    "range",
                    format!("{value} does not fit {}", base.as_str()),
                )
            })
        }
        FieldType::Flags { base, bits } => {
            let Op::Set(v) = op else {
                return Err(type_err("flags".into()));
            };
            let value = match parse_int(v) {
                Some(n) => n,
                None => {
                    let text = v
                        .as_str()
                        .ok_or_else(|| type_err("flags take an integer or names".into()))?;
                    let mut acc: i128 = 0;
                    for name in text.split('|').map(str::trim).filter(|n| !n.is_empty()) {
                        let bit = by_name(bits, name).ok_or_else(|| {
                            let names: Vec<&str> = bits.iter().map(|n| n.name.as_str()).collect();
                            type_err(format!(
                                "unknown flag `{name}`; flags: {}",
                                names.join(", ")
                            ))
                        })?;
                        acc |= bit;
                    }
                    acc
                }
            };
            encode_int(*base, value).ok_or_else(|| {
                FieldError::new(
                    path,
                    "range",
                    format!("{value} does not fit {}", base.as_str()),
                )
            })
        }
        FieldType::FormId { .. } => {
            let text = match op {
                Op::Set(Value::Null) => return Ok(0u32.to_le_bytes().to_vec()),
                Op::Set(Value::String(t)) => t.as_str(),
                _ => {
                    return Err(type_err(
                        "a FormID takes `Plugin.esm:00ABCDEF` or a raw hex FormID".into(),
                    ))
                }
            };
            let raw = parse_formid(path, text, ctx)?;
            Ok(raw.to_le_bytes().to_vec())
        }
        FieldType::Sig => {
            let text = text_value(path, op)?;
            let bytes = text.as_bytes();
            if bytes.len() != 4 || !bytes.iter().all(u8::is_ascii) {
                return Err(type_err("sig takes four ASCII characters".into()));
            }
            Ok(bytes.to_vec())
        }
        FieldType::ZString => zstring(path, text_value(path, op)?),
        FieldType::LString => {
            if ctx.localized_target {
                return Err(FieldError::new(
                    path,
                    "type",
                    "the target plugin is localized; writing string tables is not supported (M2 patches are non-localized)",
                ));
            }
            zstring(path, text_value(path, op)?)
        }
        FieldType::WString => {
            let text = text_value(path, op)?;
            let len = u16::try_from(text.len())
                .map_err(|_| FieldError::new(path, "range", "wstring longer than 65,535 bytes"))?;
            let mut out = len.to_le_bytes().to_vec();
            out.extend_from_slice(text.as_bytes());
            Ok(out)
        }
        FieldType::BString => {
            let text = text_value(path, op)?;
            let len = u8::try_from(text.len())
                .map_err(|_| FieldError::new(path, "range", "bstring longer than 255 bytes"))?;
            let mut out = vec![len];
            out.extend_from_slice(text.as_bytes());
            Ok(out)
        }
        FieldType::Empty => match op {
            Op::Set(Value::String(t)) if t.is_empty() => Ok(Vec::new()),
            _ => Err(type_err("an empty marker takes \"\"".into())),
        },
        FieldType::Opaque { .. } => Err(FieldError::new(
            path,
            "opaque",
            "this payload is opaque (no layout in the schema): set it as hex:0A0B...",
        )),
        FieldType::Union { .. } => Err(FieldError::new(
            path,
            "opaque",
            "no variant of this union applies to the record (its EditorID prefix); set it as hex:",
        )),
        FieldType::Struct { .. } | FieldType::Array { .. } | FieldType::Ref { .. } => {
            Err(FieldError::new(
                path,
                "type",
                format!(
                    "a {} is set member by member (`{path}.Member`) or whole as hex:",
                    type_name(ty)
                ),
            ))
        }
    }
}

fn text_value<'v>(path: &str, op: &'v Op) -> Result<&'v str, FieldError> {
    match op {
        Op::Set(Value::String(t)) => Ok(t),
        _ => Err(FieldError::new(path, "type", "takes a JSON string")),
    }
}

/// Largest integer magnitude `f64` holds exactly (2^53).
pub const EXACT_F64_INT: i128 = 1 << 53;

/// `{mul}` / `{add}` on an integer `current`. An integral `add` is exact in `i128` (64-bit
/// fields included); a fractional `add` and every `mul` go through `f64`, so they are
/// refused when the current value or the result is beyond 2^53, where `f64` drops bits.
///
/// `{min}`, `{max}` and `{clamp}` are exact: the bounds are rounded inwards to integers
/// (`min` 2.5 is at least 3) and the current value is kept when it is within them.
fn int_op(path: &str, current: i128, op: &Op) -> Result<i128, FieldError> {
    let range = |message: String| FieldError::new(path, "range", message);
    if let Some((lo, hi)) = op.bounds() {
        let whole = |b: f64| (b.abs() < 1.0e30).then_some(b as i128);
        let lo = match lo {
            Some(b) => {
                Some(whole(b.ceil()).ok_or_else(|| range(format!("bound {b} is out of range")))?)
            }
            None => None,
        };
        let hi = match hi {
            Some(b) => {
                Some(whole(b.floor()).ok_or_else(|| range(format!("bound {b} is out of range")))?)
            }
            None => None,
        };
        if let (Some(lo), Some(hi)) = (lo, hi) {
            if lo > hi {
                return Err(range(format!("no integer lies in [{lo}, {hi}]")));
            }
        }
        let raised = lo.map_or(current, |lo| current.max(lo));
        return Ok(hi.map_or(raised, |hi| raised.min(hi)));
    }
    if let Op::Add(a) = op {
        if a.fract() == 0.0 && a.abs() < 1.0e30 {
            return current
                .checked_add(*a as i128)
                .ok_or_else(|| range(format!("{current} + {a} overflows")));
        }
    }
    if current.abs() > EXACT_F64_INT {
        return Err(range(format!(
            "{current} is beyond 2^53, where mul and fractional add are inexact; set the value, or add an integer"
        )));
    }
    let next = match op {
        Op::Mul(m) => current as f64 * m,
        Op::Add(a) => current as f64 + a,
        _ => current as f64,
    };
    if !next.is_finite() || next.abs() > EXACT_F64_INT as f64 {
        return Err(range(format!(
            "the result of {op:?} on {current} is beyond 2^53 (exact integer range of f64)"
        )));
    }
    Ok(next.round() as i128)
}

/// Whether `bytes` is one NUL-terminated string: non-empty, ends in NUL, no other NUL.
pub fn is_one_zstring(bytes: &[u8]) -> bool {
    bytes
        .strip_suffix(&[0])
        .is_some_and(|body| !body.contains(&0))
}

/// Whether `ty` (behind any `ref:`) is an lstring.
pub fn is_lstring(ty: &FieldType) -> bool {
    matches!(deref(ty), FieldType::LString)
}

fn zstring(path: &str, text: &str) -> Result<Vec<u8>, FieldError> {
    if text.contains('\0') {
        return Err(FieldError::new(path, "type", "text may not contain NUL"));
    }
    let mut out = text.as_bytes().to_vec();
    out.push(0);
    Ok(out)
}

fn by_name(values: &[NamedValue], name: &str) -> Option<i128> {
    values
        .iter()
        .find(|v| v.name.eq_ignore_ascii_case(name))
        .map(|v| v.value)
}

/// An integer from a JSON integer or decimal / `0x` hex text.
pub fn parse_int(value: &Value) -> Option<i128> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .map(i128::from)
            .or_else(|| n.as_u64().map(i128::from)),
        Value::String(s) => {
            let s = s.trim();
            let (neg, digits) = match s.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, s),
            };
            let magnitude = match digits
                .strip_prefix("0x")
                .or_else(|| digits.strip_prefix("0X"))
            {
                Some(hex) if !hex.is_empty() => i128::from_str_radix(hex, 16).ok()?,
                Some(_) => return None,
                None if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                    digits.parse::<i128>().ok()?
                }
                None => return None,
            };
            if neg {
                magnitude.checked_neg()
            } else {
                Some(magnitude)
            }
        }
        _ => None,
    }
}

/// Why [`parse_float`] gave no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloatError {
    /// Not a number at all (a `type` error).
    NotANumber,
    /// A number, but infinite, NaN, or beyond `f64` (`1e400`): a `range` error. Holds the
    /// text as given.
    NotFinite(String),
}

/// A finite number from a JSON number or decimal text. `inf`, `infinity` and `nan` (any
/// case, signed) and decimals beyond `f64` (`1e400`) are numbers that are not finite.
pub fn parse_float(value: &Value) -> Result<f64, FloatError> {
    let v = match value {
        Value::Number(n) => n.as_f64().ok_or(FloatError::NotANumber)?,
        Value::String(s) => {
            let s = s.trim();
            let unsigned = s.trim_start_matches(['+', '-']).to_ascii_lowercase();
            if matches!(unsigned.as_str(), "inf" | "infinity" | "nan") {
                return Err(FloatError::NotFinite(s.to_string()));
            }
            // Only plain decimals; Rust would also parse other spellings.
            if s.is_empty()
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+' | b'e' | b'E'))
            {
                return Err(FloatError::NotANumber);
            }
            s.parse::<f64>().map_err(|_| FloatError::NotANumber)?
        }
        _ => return Err(FloatError::NotANumber),
    };
    if v.is_finite() {
        Ok(v)
    } else {
        Err(FloatError::NotFinite(value_text(value)))
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.trim().to_string(),
        other => other.to_string(),
    }
}

fn encode_int(s: Scalar, value: i128) -> Option<Vec<u8>> {
    if !s.fits(value) {
        return None;
    }
    Some(match s {
        Scalar::U8 => u8::try_from(value).ok()?.to_le_bytes().to_vec(),
        Scalar::I8 => i8::try_from(value).ok()?.to_le_bytes().to_vec(),
        Scalar::U16 => u16::try_from(value).ok()?.to_le_bytes().to_vec(),
        Scalar::I16 => i16::try_from(value).ok()?.to_le_bytes().to_vec(),
        Scalar::U32 => u32::try_from(value).ok()?.to_le_bytes().to_vec(),
        Scalar::I32 => i32::try_from(value).ok()?.to_le_bytes().to_vec(),
        Scalar::U64 => u64::try_from(value).ok()?.to_le_bytes().to_vec(),
        Scalar::I64 => i64::try_from(value).ok()?.to_le_bytes().to_vec(),
        Scalar::F32 | Scalar::F64 => return None,
    })
}

/// Raw bytes from hex digits (whitespace ignored).
pub fn parse_hex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[h, l]| {
            let hi = char::from(h).to_digit(16)?;
            let lo = char::from(l).to_digit(16)?;
            u8::try_from(hi.checked_mul(16)?.checked_add(lo)?).ok()
        })
        .collect()
}

/// A `formref` (`Plugin.esm:00ABCDEF`, object ID in the low 24 bits) split into the
/// plugin name and object ID; `None` when `text` is not one.
pub fn split_formref(text: &str) -> Option<(&str, u32)> {
    let (plugin, hex) = text.rsplit_once(':')?;
    if plugin.is_empty() || hex.is_empty() || hex.len() > 8 {
        return None;
    }
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let id = u32::from_str_radix(hex, 16).ok()?;
    Some((plugin, id))
}

fn parse_formid(path: &str, text: &str, ctx: &ValueCtx<'_>) -> Result<u32, FieldError> {
    if let Some((plugin, id)) = split_formref(text) {
        if id > 0x00FF_FFFF {
            return Err(FieldError::new(
                path,
                "formref",
                format!("`{text}`: a formref's object ID has at most 24 bits"),
            ));
        }
        if id == 0 {
            return Err(FieldError::new(
                path,
                "formref",
                format!("`{text}`: object ID 0 is no form; set the field to null for none"),
            ));
        }
        return (ctx.formref)(plugin, id).map_err(|m| FieldError::new(path, "formref", m));
    }
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    if digits.is_empty() || digits.len() > 8 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(FieldError::new(
            path,
            "type",
            format!("`{text}` is neither `Plugin.esm:00ABCDEF` nor a hex FormID"),
        ));
    }
    u32::from_str_radix(digits, 16)
        .map_err(|_| FieldError::new(path, "type", format!("`{text}` is not a FormID")))
}

/// A JSON view of a value of type `ty` stored as `bytes` (for before/after reports).
pub fn display(ty: &FieldType, bytes: &[u8], localized: bool) -> Value {
    let hex = || json!({ "hex": to_hex(bytes) });
    match deref(ty) {
        FieldType::Scalar(s) if s.is_integer() => read_int(*s, bytes).map_or_else(hex, |v| {
            i64::try_from(v).map_or_else(|_| json!(v.to_string()), |v| json!(v))
        }),
        FieldType::Scalar(s) => read_float(*s, bytes)
            .and_then(serde_json::Number::from_f64)
            .map_or_else(hex, Value::Number),
        FieldType::Enum { base, values } => read_int(*base, bytes).map_or_else(hex, |v| {
            values
                .iter()
                .find(|n| n.value == v)
                .map_or_else(|| json!(v.to_string()), |n| json!(n.name))
        }),
        FieldType::FormId { .. } => match <[u8; 4]>::try_from(bytes).map(u32::from_le_bytes) {
            Ok(0) => Value::Null,
            Ok(raw) => json!(format!("{raw:08X}")),
            Err(_) => hex(),
        },
        FieldType::ZString => text_of(bytes).map_or_else(hex, Value::String),
        FieldType::LString if localized => <[u8; 4]>::try_from(bytes).map_or_else(
            |_| hex(),
            |b| json!({ "string_id": format!("0x{:08X}", u32::from_le_bytes(b)) }),
        ),
        FieldType::LString => text_of(bytes).map_or_else(hex, Value::String),
        _ => hex(),
    }
}

/// A zstring payload's text, when it is exactly one NUL-terminated UTF-8 string.
pub fn text_of(bytes: &[u8]) -> Option<String> {
    let body = bytes.strip_suffix(&[0])?;
    if body.contains(&0) {
        return None;
    }
    String::from_utf8(body.to_vec()).ok()
}

/// Uppercase hex.
pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for b in bytes {
        let _ = write!(out, "{b:02X}");
    }
    out
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use kitkraft_schema::{Confidence, FieldDef, Provenance};

    fn ctx(localized_target: bool) -> ValueCtx<'static> {
        static MAP: fn(&str, u32) -> Result<u32, String> = |plugin, id| {
            if plugin.eq_ignore_ascii_case("Starfield.esm") {
                Ok(id)
            } else {
                Err(format!("{plugin} is not a master of the target"))
            }
        };
        ValueCtx {
            localized_target,
            localized_source: true,
            form_version: 582,
            formref: &MAP,
        }
    }

    fn set(ty: &FieldType, value: Value) -> Result<Vec<u8>, FieldError> {
        encode_value("X", ty, &[], &Op::Set(value), &ctx(false))
    }

    fn field(name: &str, ty: FieldType) -> FieldDef {
        FieldDef {
            name: name.into(),
            ty,
            since: None,
            until: None,
            if_remaining: false,
            note: None,
            provenance: Provenance {
                source: "corpus: test".into(),
                evidence: Vec::new(),
                confidence: Confidence::Corpus,
            },
        }
    }

    #[test]
    fn paths() {
        let p = parse_path("FULL").unwrap();
        assert_eq!((p.signature, p.occurrence, p.steps.len()), (*b"FULL", 0, 0));
        let p = parse_path("FULL[3]").unwrap();
        assert_eq!(p.occurrence, 3);
        let p = parse_path("DNAM[1].Inner.Values[2].X").unwrap();
        assert_eq!(p.occurrence, 1);
        assert_eq!(
            p.steps,
            [
                Step::Member("Inner".into()),
                Step::Member("Values".into()),
                Step::Index(2),
                Step::Member("X".into())
            ]
        );
        for bad in ["full", "FUL", "FULL.", "FULL[x]", "FULL[1][2]", "FULL.a b"] {
            assert_eq!(parse_path(bad).unwrap_err().rule, "path-syntax", "{bad}");
        }
    }

    /// Lane B round 4: an object mod property's Value1/Value2 are field unions decided by
    /// its ValueType; a path steps into the variant the entry's own value type selects
    /// (`DATA.Properties[1].Value1.Float`), and naming another variant is refused.
    #[test]
    fn omod_property_value_paths() {
        let layout = layout_at(*b"OMOD", &[*b"EDID", *b"DATA"], 1).unwrap();
        let entry = |vt: u8, prop: &[u8; 4], v1: [u8; 4], v2: [u8; 4]| {
            let mut out = vec![vt, 0, 0, 0, 0, 0, 0, 0];
            out.extend_from_slice(prop);
            out.extend_from_slice(&v1);
            out.extend_from_slice(&v2);
            out.extend_from_slice(&0f32.to_le_bytes());
            out
        };
        let mut data = Vec::new();
        data.extend(0u32.to_le_bytes()); // IncludeCount
        data.extend(2u32.to_le_bytes()); // PropertyCount
        data.extend([0, 0]); // Unknown1
        data.extend(0u32.to_le_bytes()); // NameLength
        data.extend([1, 0]); // MaxRank, LevelTierScaledOffset
        data.extend(0u32.to_le_bytes()); // AttachPoint
        data.extend(0u32.to_le_bytes()); // AttachParentSlotCount
        data.extend([0; 4]); // Unknown2
        let header = data.len();
        data.extend(entry(
            4,
            b"WKWD",
            0x0001_2345u32.to_le_bytes(),
            0i32.to_le_bytes(),
        ));
        data.extend(entry(
            1,
            b"WSPS",
            1.5f32.to_le_bytes(),
            0.25f32.to_le_bytes(),
        ));
        let at = |text: &str| {
            let p = parse_path(text).unwrap();
            locate(text, &layout.ty, &data, &p.steps, 582)
        };
        let float = at("DATA.Properties[1].Value1.Float").unwrap();
        assert_eq!((float.start, float.len), (header + 24 + 12, 4));
        assert_eq!(*float.ty, FieldType::Scalar(Scalar::F32));
        let old = &data[float.start..float.start + 4];
        let doubled = encode_value("X", float.ty, old, &Op::Mul(2.0), &ctx(false)).unwrap();
        assert_eq!(doubled, 3.0f32.to_le_bytes());
        let second = at("DATA.properties[1].value2.float").unwrap();
        assert_eq!(second.start, header + 24 + 16);
        let form = at("DATA.Properties[0].Value1.FormId").unwrap();
        assert!(matches!(form.ty, FieldType::FormId { .. }), "{:?}", form.ty);
        // Entry 0 has value type 4: its Value1 is a FormID, not a float.
        let wrong = at("DATA.Properties[0].Value1.Float").unwrap_err();
        assert_eq!(wrong.rule, "layout");
        assert!(wrong.message.contains("selects `FormId`"), "{wrong}");
        let none = at("DATA.Properties[0].Value1.Nope").unwrap_err();
        assert_eq!(none.rule, "no-member");
        assert!(none.message.contains("Int, Float"), "{none}");
        // The union itself is not a number.
        let whole = at("DATA.Properties[1].Value1").unwrap();
        assert!(encode_value("X", whole.ty, &[0; 4], &Op::Set(json!(1)), &ctx(false)).is_err());
    }

    #[test]
    fn integers_by_width() {
        let u8t = FieldType::Scalar(Scalar::U8);
        assert_eq!(set(&u8t, json!(255)).unwrap(), [255]);
        assert_eq!(set(&u8t, json!("0x10")).unwrap(), [16]);
        assert_eq!(set(&u8t, json!(256)).unwrap_err().rule, "range");
        assert_eq!(set(&u8t, json!(-1)).unwrap_err().rule, "range");
        assert_eq!(set(&u8t, json!("1.5")).unwrap_err().rule, "type");
        let i16t = FieldType::Scalar(Scalar::I16);
        assert_eq!(set(&i16t, json!("-2")).unwrap(), (-2i16).to_le_bytes());
        assert_eq!(set(&i16t, json!(40000)).unwrap_err().rule, "range");
        let u32t = FieldType::Scalar(Scalar::U32);
        assert_eq!(set(&u32t, json!("4294967295")).unwrap(), [255; 4]);
        let i64t = FieldType::Scalar(Scalar::I64);
        assert_eq!(set(&i64t, json!(i64::MIN)).unwrap(), i64::MIN.to_le_bytes());
        // mul/add on the current value, rounded.
        let old = 10u32.to_le_bytes();
        let got = encode_value("X", &u32t, &old, &Op::Mul(0.85), &ctx(false)).unwrap();
        assert_eq!(got, 9u32.to_le_bytes());
        let got = encode_value("X", &u32t, &old, &Op::Add(-11.0), &ctx(false));
        assert_eq!(got.unwrap_err().rule, "range");
    }

    /// `{add}` on 64-bit integers is exact (i128); `{mul}` and fractional `{add}` are
    /// refused beyond 2^53, where f64 drops bits (Fable review round 2, finding 9).
    #[test]
    fn sixty_four_bit_mul_and_add() {
        let i64t = FieldType::Scalar(Scalar::I64);
        let u64t = FieldType::Scalar(Scalar::U64);
        let big = (1i64 << 60) + 1;
        let op = |ty: &FieldType, old: &[u8], op: Op| encode_value("X", ty, old, &op, &ctx(false));
        // Through f64 this was 2^60 + 0: the low bit was lost.
        let got = op(&i64t, &big.to_le_bytes(), Op::Add(1.0)).unwrap();
        assert_eq!(got, (big + 1).to_le_bytes());
        let got = op(&u64t, &u64::MAX.to_le_bytes(), Op::Add(-1.0)).unwrap();
        assert_eq!(got, (u64::MAX - 1).to_le_bytes());
        assert_eq!(
            op(&u64t, &u64::MAX.to_le_bytes(), Op::Add(1.0))
                .unwrap_err()
                .rule,
            "range"
        );
        for refused in [Op::Mul(1.0), Op::Add(0.5)] {
            let err = op(&i64t, &big.to_le_bytes(), refused).unwrap_err();
            assert_eq!(err.rule, "range");
            assert!(err.message.contains("2^53"), "{err}");
        }
        // A result beyond 2^53 is refused too; within it, mul works.
        let near = 1i64 << 52;
        assert_eq!(
            op(&i64t, &near.to_le_bytes(), Op::Mul(4.0))
                .unwrap_err()
                .rule,
            "range"
        );
        let got = op(&i64t, &near.to_le_bytes(), Op::Mul(0.5)).unwrap();
        assert_eq!(got, (near / 2).to_le_bytes());
    }

    /// A zstring or (non-localized) lstring set as `hex:` must be one NUL-terminated
    /// string (Fable review round 2, finding 4).
    #[test]
    fn hex_strings_must_be_one_zstring() {
        for ty in [FieldType::ZString, FieldType::LString] {
            for bad in ["hex:", "hex:4142", "hex:41004200", "hex:00 00"] {
                let err = set(&ty, json!(bad)).unwrap_err();
                assert_eq!(err.rule, "type", "{bad}");
                assert!(err.message.contains("NUL-terminated"), "{err}");
            }
            assert_eq!(set(&ty, json!("hex:414200")).unwrap(), b"AB\0");
            assert_eq!(set(&ty, json!("hex:00")).unwrap(), b"\0");
        }
        assert!(is_one_zstring(b"x\0") && !is_one_zstring(b"") && !is_one_zstring(b"\0\0"));
    }

    #[test]
    fn floats() {
        let f32t = FieldType::Scalar(Scalar::F32);
        assert_eq!(set(&f32t, json!(1.5)).unwrap(), 1.5f32.to_le_bytes());
        assert_eq!(set(&f32t, json!("1.5")).unwrap(), 1.5f32.to_le_bytes());
        assert_eq!(set(&f32t, json!(1e300)).unwrap_err().rule, "range");
        // Non-finite spellings and decimals beyond f64 are range errors, not type errors
        // (Fable review round 2, finding 10); other text is a type error.
        let f64t = FieldType::Scalar(Scalar::F64);
        for text in ["inf", "-Infinity", "NaN", "1e400", "-1e400"] {
            let err = set(&f64t, json!(text)).unwrap_err();
            assert_eq!(err.rule, "range", "{text}");
            assert!(err.message.contains("not a finite number"), "{err}");
        }
        assert_eq!(set(&f64t, json!("abc")).unwrap_err().rule, "type");
        assert_eq!(set(&f64t, json!("")).unwrap_err().rule, "type");
        let f64t = FieldType::Scalar(Scalar::F64);
        assert_eq!(set(&f64t, json!(1e300)).unwrap(), 1e300f64.to_le_bytes());
        let old = 2.0f32.to_le_bytes();
        let got = encode_value("X", &f32t, &old, &Op::Mul(0.8), &ctx(false)).unwrap();
        assert_eq!(got, 1.6f32.to_le_bytes());
    }

    #[test]
    fn enums_and_flags() {
        let values = vec![
            NamedValue {
                name: "Pistol".into(),
                value: 1,
            },
            NamedValue {
                name: "Rifle".into(),
                value: 2,
            },
        ];
        let e = FieldType::Enum {
            base: Scalar::U8,
            values: values.clone(),
        };
        assert_eq!(set(&e, json!("rifle")).unwrap(), [2]);
        assert_eq!(set(&e, json!(7)).unwrap(), [7]);
        assert_eq!(set(&e, json!("Cannon")).unwrap_err().rule, "type");
        let f = FieldType::Flags {
            base: Scalar::U16,
            bits: values,
        };
        assert_eq!(set(&f, json!("Pistol|Rifle")).unwrap(), [3, 0]);
        assert_eq!(set(&f, json!("0x8000")).unwrap(), [0, 0x80]);
        assert_eq!(set(&f, json!("Nope")).unwrap_err().rule, "type");
    }

    #[test]
    fn strings_formids_and_hex() {
        assert_eq!(
            set(&FieldType::ZString, json!("Rifle")).unwrap(),
            b"Rifle\0"
        );
        assert_eq!(
            set(&FieldType::ZString, json!("a\0b")).unwrap_err().rule,
            "type"
        );
        assert_eq!(
            set(&FieldType::LString, json!("Old Soviet")).unwrap(),
            b"Old Soviet\0"
        );
        let refused = encode_value(
            "X",
            &FieldType::LString,
            &[],
            &Op::Set(json!("x")),
            &ctx(true),
        );
        assert_eq!(refused.unwrap_err().rule, "type");
        assert_eq!(
            set(&FieldType::WString, json!("ab")).unwrap(),
            [2, 0, b'a', b'b']
        );
        assert_eq!(
            set(&FieldType::BString, json!("ab")).unwrap(),
            [2, b'a', b'b']
        );
        assert_eq!(set(&FieldType::Sig, json!("WEAP")).unwrap(), b"WEAP");
        assert_eq!(set(&FieldType::Sig, json!("WEA")).unwrap_err().rule, "type");
        let form = FieldType::FormId { targets: vec![] };
        assert_eq!(
            set(&form, json!("Starfield.esm:0021BBCD")).unwrap(),
            0x0021_BBCDu32.to_le_bytes()
        );
        assert_eq!(
            set(&form, json!("0x0121BBCD")).unwrap(),
            0x0121_BBCDu32.to_le_bytes()
        );
        assert_eq!(
            set(&form, json!("Other.esm:000001")).unwrap_err().rule,
            "formref"
        );
        assert_eq!(
            set(&form, json!("Starfield.esm:FF000001"))
                .unwrap_err()
                .rule,
            "formref"
        );
        assert_eq!(set(&form, json!(5)).unwrap_err().rule, "type");
        // FormID 0 is null both ways, never a formref.
        assert_eq!(set(&form, json!(null)).unwrap(), [0; 4]);
        assert_eq!(
            set(&form, json!("Starfield.esm:000000")).unwrap_err().rule,
            "formref"
        );
        assert_eq!(display(&form, &[0; 4], false), json!(null));
        assert_eq!(display(&form, &[1, 0, 0, 1], false), json!("01000001"));
        let opaque = FieldType::Opaque { size: Some(2) };
        assert_eq!(set(&opaque, json!("hex:0A 0b")).unwrap(), [10, 11]);
        assert_eq!(set(&opaque, json!("hex:0A")).unwrap_err().rule, "range");
        assert_eq!(set(&opaque, json!("x")).unwrap_err().rule, "opaque");
        assert_eq!(
            set(&FieldType::Opaque { size: None }, json!("hex:")).unwrap(),
            b""
        );
        assert_eq!(set(&FieldType::Empty, json!("")).unwrap(), b"");
        assert_eq!(
            encode_value("X", &FieldType::ZString, &[], &Op::Mul(2.0), &ctx(false))
                .unwrap_err()
                .rule,
            "type"
        );
    }

    #[test]
    fn struct_members_and_arrays() {
        let ty = FieldType::Struct {
            fields: vec![
                field("Count", FieldType::Scalar(Scalar::U8)),
                field("Name", FieldType::BString),
                field(
                    "Items",
                    FieldType::Array {
                        element: Box::new(FieldType::Scalar(Scalar::U16)),
                        count: ArrayCount::From("Count".into()),
                    },
                ),
                field("Speed", FieldType::Scalar(Scalar::F32)),
                FieldDef {
                    since: Some(600),
                    ..field("Later", FieldType::Scalar(Scalar::U8))
                },
            ],
        };
        let mut payload = vec![2u8, 1, b'x', 7, 0, 9, 0];
        payload.extend_from_slice(&2.0f32.to_le_bytes());
        let steps = |p: &str| parse_path(p).unwrap().steps;
        let at = locate("X", &ty, &payload, &steps("DATA.Speed"), 582).unwrap();
        assert_eq!((at.start, at.len), (7, 4));
        let at = locate("X", &ty, &payload, &steps("DATA.items[1]"), 582).unwrap();
        assert_eq!((at.start, at.len), (5, 2));
        let err = locate("X", &ty, &payload, &steps("DATA.Items[2]"), 582).unwrap_err();
        assert_eq!(err.rule, "no-member");
        let err = locate("X", &ty, &payload, &steps("DATA.Later"), 582).unwrap_err();
        assert_eq!(err.rule, "no-member");
        let err = locate("X", &ty, &payload, &steps("DATA.Nope"), 582).unwrap_err();
        assert!(err.message.contains("Count, Name, Items, Speed"), "{err}");
        let err = locate("X", &ty, &payload, &steps("DATA.Speed.X"), 582).unwrap_err();
        assert_eq!(err.rule, "no-member");
        let err = set(&ty, json!("x")).unwrap_err();
        assert_eq!(err.rule, "type");
        // A payload shorter than the layout says.
        let err = locate("X", &ty, &payload[..8], &steps("DATA.Speed"), 582).unwrap_err();
        assert_eq!(err.rule, "payload");
    }

    #[test]
    fn layouts_and_fallback() {
        let at = |record: [u8; 4], sub: [u8; 4]| layout_at(record, &[*b"EDID", sub], 1);
        // WEAP FULL is opaque in the schema today, or an lstring once lane B types it.
        let layout = at(*b"WEAP", *b"FULL").unwrap();
        assert_eq!(layout.ty, FieldType::LString);
        // A record type without a definition still gets the lstring fallback.
        let layout = at(*b"ZZZZ", *b"FULL").unwrap();
        assert!(layout.fallback.is_some());
        assert_eq!(at(*b"ZZZZ", *b"XNAM"), None);
        // EDID is described by the WEAP definition.
        assert!(layout_at(*b"WEAP", &[*b"EDID"], 0).is_some());
        // Every lstring of the corpus table (finding 2b) outside a base form component is
        // typed by the schema since lane B round 3 (M3.B1, M3.B8): no fallback. WEAP has no
        // ATTX in the corpus.
        for (record, sub) in [(*b"ACTI", *b"ATTX"), (*b"WEAP", *b"WABB")] {
            let layout = at(record, sub).unwrap();
            assert_eq!(layout.ty, FieldType::LString);
            assert!(layout.fallback.is_none());
        }
        assert_eq!(at(*b"WEAP", *b"ATTX"), None);
        // An lstring inside a component block (PNDT's full-name component, GBFM's hull
        // code) is raw in the schema; the FULL/DESC fact or the corpus table still types it.
        let pndt = [*b"EDID", *b"BFCB", *b"FULL", *b"BFCE"];
        let full = layout_at(*b"PNDT", &pndt, 2).unwrap();
        assert_eq!(full.ty, FieldType::LString);
        assert!(full.fallback.is_some());
        let gbfm = [*b"EDID", *b"OBND", *b"ODTY", *b"BFCB", *b"HULL", *b"BFCE"];
        let hull = layout_at(*b"GBFM", &gbfm, 4).unwrap();
        assert_eq!(hull.ty, FieldType::LString);
        assert!(hull.fallback.unwrap().contains("corpus lstring table"));
        // Out of range: no layout, never a panic.
        assert_eq!(layout_at(*b"WEAP", &[*b"EDID"], 3), None);
    }

    /// Planet schema lane request to lane E: layouts resolve by position. PNDT's two CNAM,
    /// FNAM and GNAM each get their own member's layout (no "different layouts" error), and
    /// a VNAM inside a PKIN component block is not the top-level `Version`: the placement
    /// properties' `SavePlacement` since lane D types that component, and opaque inside a
    /// component layout the block keeps raw.
    #[test]
    fn layouts_resolve_by_position() {
        let pndt = [
            *b"EDID", *b"CNAM", *b"FNAM", *b"GNAM", *b"BDST", *b"ANAM", *b"CNAM", *b"ENAM",
            *b"FNAM", *b"GNAM", *b"KNAM", *b"BDED", *b"TEMP", *b"DENS", *b"PHLA",
        ];
        let all = layouts(*b"PNDT", &pndt);
        assert_eq!(all.len(), pndt.len());
        for (top, inner) in [(1, 6), (2, 8), (3, 9)] {
            let a = layout_at(*b"PNDT", &pndt, top).unwrap();
            let b = layout_at(*b"PNDT", &pndt, inner).unwrap();
            assert_ne!(a.ty, b.ty, "{top} vs {inner}");
            assert_eq!(all[top].as_ref(), Some(&a));
            assert_eq!(all[inner].as_ref(), Some(&b));
        }
        let pkin = [*b"EDID", *b"BFCB", *b"VNAM", *b"BFCE", *b"CNAM", *b"VNAM"];
        let save = layout_at(*b"PKIN", &pkin, 2).unwrap();
        let version = layout_at(*b"PKIN", &pkin, 5).unwrap();
        assert!(!version.ty.is_opaque());
        assert_ne!(save.ty, version.ty);
        let raw = [
            *b"EDID", *b"BFCB", *b"ATCP", *b"VNAM", *b"BFCE", *b"CNAM", *b"VNAM",
        ];
        assert_eq!(layout_at(*b"PKIN", &raw, 3), None);
    }

    /// M3.E6 operators: `min` raises to at least, `max` lowers to at most, `clamp` both;
    /// integer bounds round inwards and keep the current value exact.
    #[test]
    fn min_max_clamp_on_numbers() {
        assert_eq!(parse_op("X", &json!({"min": 1})).unwrap(), Op::Min(1.0));
        assert_eq!(parse_op("X", &json!({"max": 2.5})).unwrap(), Op::Max(2.5));
        assert_eq!(
            parse_op("X", &json!({"clamp": [0, 1]})).unwrap(),
            Op::Clamp(0.0, 1.0)
        );
        for bad in [
            json!({"clamp": [1]}),
            json!({"clamp": [2, 1]}),
            json!({"clamp": 3}),
            json!({"min": "x"}),
            json!({"pow": 2}),
        ] {
            assert!(parse_op("X", &bad).is_err(), "{bad}");
        }
        let f32t = FieldType::Scalar(Scalar::F32);
        let f = |v: f32, op: Op| {
            let b = encode_value("X", &f32t, &v.to_le_bytes(), &op, &ctx(false)).unwrap();
            f32::from_le_bytes(b.try_into().unwrap())
        };
        assert_eq!(f(0.5, Op::Min(1.0)), 1.0);
        assert_eq!(f(1.5, Op::Min(1.0)), 1.5);
        assert_eq!(f(3.0, Op::Max(2.0)), 2.0);
        assert_eq!(f(1.0, Op::Max(2.0)), 1.0);
        assert_eq!(f(-1.0, Op::Clamp(0.0, 1.0)), 0.0);
        assert_eq!(f(0.25, Op::Clamp(0.0, 1.0)), 0.25);
        assert_eq!(f(9.0, Op::Clamp(0.0, 1.0)), 1.0);
        let i32t = FieldType::Scalar(Scalar::I32);
        let i = |v: i32, op: Op| {
            encode_value("X", &i32t, &v.to_le_bytes(), &op, &ctx(false))
                .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
        };
        assert_eq!(i(1, Op::Min(2.5)).unwrap(), 3);
        assert_eq!(i(7, Op::Max(2.5)).unwrap(), 2);
        assert_eq!(i(5, Op::Clamp(-1.5, 9.9)).unwrap(), 5);
        assert_eq!(i(-5, Op::Clamp(-1.5, 9.9)).unwrap(), -1);
        assert_eq!(i(5, Op::Clamp(0.2, 0.8)).unwrap_err().rule, "range");
        assert_eq!(i(5, Op::Min(1e40)).unwrap_err().rule, "range");
        // 64-bit values stay exact (no f64 round trip).
        let u64t = FieldType::Scalar(Scalar::U64);
        let big = u64::MAX - 1;
        let got = encode_value("X", &u64t, &big.to_le_bytes(), &Op::Min(1.0), &ctx(false)).unwrap();
        assert_eq!(got, big.to_le_bytes());
        // Not a number: refused.
        let err =
            encode_value("X", &FieldType::ZString, b"a\0", &Op::Max(1.0), &ctx(false)).unwrap_err();
        assert_eq!(err.rule, "type");
        assert!(err.message.contains("apply to typed numbers"));
    }

    #[test]
    fn ops_and_display() {
        assert_eq!(parse_op("X", &json!({"mul": 0.8})).unwrap(), Op::Mul(0.8));
        assert_eq!(parse_op("X", &json!({"add": 2})).unwrap(), Op::Add(2.0));
        assert_eq!(parse_op("X", &json!(3)).unwrap(), Op::Set(json!(3)));
        assert_eq!(
            parse_op("X", &json!({"mul": 1, "add": 1}))
                .unwrap_err()
                .rule,
            "type"
        );
        assert_eq!(
            parse_op("X", &json!({"mul": "x"})).unwrap_err().rule,
            "type"
        );
        assert_eq!(
            display(&FieldType::LString, &[0x69, 0xE4, 2, 0], true),
            json!({"string_id": "0x0002E469"})
        );
        assert_eq!(
            display(&FieldType::LString, b"Rifle\0", false),
            json!("Rifle")
        );
        assert_eq!(
            display(
                &FieldType::Scalar(Scalar::F32),
                &1.5f32.to_le_bytes(),
                false
            ),
            json!(1.5)
        );
        assert_eq!(
            display(&FieldType::Opaque { size: None }, &[1, 2], false),
            json!({"hex": "0102"})
        );
        assert_eq!(text_of(b"a\0b\0"), None);
        assert_eq!(
            split_formref("Starfield.esm:0021BBCD"),
            Some(("Starfield.esm", 0x21BBCD))
        );
        assert_eq!(split_formref("0021BBCD"), None);
    }
}
