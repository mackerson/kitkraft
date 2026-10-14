//! Typed field values for `where` paths, read through the schema definitions
//! (`kitcraft_schema::builtin()`, lane B), by interpreting a subrecord's [`FieldType`]
//! over its payload.
//!
//! This is a read-only decoder for queries: the generated records in `kitcraft-esm` stay the
//! write path. A payload whose layout does not account for every byte, a value under an
//! opaque member, or a union variant no definition claims is unknown, never a guess.

use kitcraft_schema::{ArrayCount, Decider, FieldType, Scalar, Schema, Signature};

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

fn decode_in(
    ty: &FieldType,
    c: &mut Cursor<'_>,
    ctx: &Ctx,
    siblings: &[(String, Decoded)],
) -> Result<Decoded, String> {
    match ty {
        FieldType::Scalar(s) => scalar(*s, c),
        FieldType::Enum { base, .. } | FieldType::Flags { base, .. } => scalar(*base, c),
        FieldType::FormId { .. } => Ok(Decoded::Form(le(c.take(4)?) as u32)),
        FieldType::Sig => {
            let b = c.take(4)?;
            Ok(Decoded::Sig(b.try_into().map_err(|_| "sig")?))
        }
        FieldType::ZString => Ok(Decoded::Text(zstring(c.rest()))),
        FieldType::LString => {
            if ctx.localized {
                Ok(Decoded::LStringId(le(c.take(4)?) as u32))
            } else {
                Ok(Decoded::Text(zstring(c.rest())))
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
        FieldType::Ref { ty, .. } => decode_in(ty, c, ctx, siblings),
        FieldType::Union { decider, variants } => {
            let Decider::EditorIdPrefix = decider;
            match variants.iter().find(|v| Some(v.when) == ctx.edid_first) {
                Some(v) => decode_in(&v.ty, c, ctx, siblings),
                None => {
                    let at = c.pos;
                    let len = c.rest().len();
                    Ok(Decoded::Opaque { at, len })
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
                out.push(decode_in(element, c, ctx, &[])?);
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
                let v = decode_in(&f.ty, c, ctx, &out)?;
                out.push((f.name.clone(), v));
            }
            Ok(Decoded::Struct(out))
        }
    }
}

/// Decode a whole subrecord payload; every byte must be accounted for.
pub fn decode(ty: &FieldType, data: &[u8], ctx: &Ctx) -> Result<Decoded, String> {
    let mut c = Cursor { data, pos: 0 };
    let v = decode_in(ty, &mut c, ctx, &[])?;
    if c.remaining() != 0 {
        return Err(format!("layout covers {} of {} bytes", c.pos, data.len()));
    }
    Ok(v)
}

/// The definition of subrecord `sub` in record type `record`, with the subrecord's member
/// name (a path may start with it: `KSIZ.Count`): `Ok(None)` when the record
/// type or the subrecord has no definition, `Err` when several definitions with different
/// layouts share the signature.
pub fn field_type(
    schema: &Schema,
    record: [u8; 4],
    sub: [u8; 4],
) -> Result<Option<(&FieldType, &str)>, String> {
    let (Some(rs), Some(ss)) = (Signature::from_bytes(record), Signature::from_bytes(sub)) else {
        return Ok(None);
    };
    let Some(def) = schema.records.get(&rs) else {
        return Ok(None);
    };
    let leaves = def.leaf_subrecords();
    let mut found: Option<(&FieldType, &str)> = None;
    for (_, s) in leaves {
        if s.signature == ss {
            match found {
                None => found = Some((&s.ty, s.name.as_str())),
                Some((t, _)) if *t == s.ty => {}
                Some(_) => {
                    return Err(format!(
                        "{ss} has more than one layout in {rs}; the path is ambiguous"
                    ))
                }
            }
        }
    }
    Ok(found)
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
        let schema = kitcraft_schema::builtin().unwrap();
        // GMST DATA is a union chosen by the EditorID's first letter: `f` is an f32.
        let (ty, _) = field_type(schema, *b"GMST", *b"DATA").unwrap().unwrap();
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
        assert!(field_type(schema, *b"QQQQ", *b"DATA").unwrap().is_none());
        assert!(field_type(schema, *b"GMST", *b"ZZZZ").unwrap().is_none());
    }

    #[test]
    fn structs_arrays_and_errors() {
        use kitcraft_schema::FieldDef;
        let prov = kitcraft_schema::builtin()
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
}
