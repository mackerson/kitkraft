//! Script attachment (M4.E9, moved here from `kitkraft_engine::edit::vmad` for M5.D6): the
//! scripts list of a `VMAD` subrecord, decoded and encoded losslessly, and the object
//! values it holds with their offsets ([`object_refs`]), which `links`, `backrefs` and the
//! engine's `validate` follow. The attach operation stays in the engine.
//!
//! The schema types `VMAD` as the shared `ScriptAttachment` block (lane BQ,
//! `crates/schema/defs/common/ScriptAttachment.toml`): `Version` (i16), `ObjectFormat`
//! (i16), `ScriptCount` (u16) and `Data`, kept raw because each property's value layout is
//! chosen by its type byte (a sibling-decided union over nested structures the format
//! cannot express). This module reads and writes `Data`'s script entries; whatever
//! follows them (the fragment block of `QUST`, `INFO`, `PACK`, `SCEN`, `PERK`) stays raw
//! bytes, unchanged.
//!
//! Layout (`xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L3232-L3411`:
//! the property type enum, script flags, the object union, the struct members, the
//! property list, the script entry and the VMAD struct; `#L1796-L1821` the value decider;
//! object formats by `wbScriptObjFormatDecider`, `wbDefinitionsCommon.pas@9fb01688...#L6145-L6148`):
//!
//! - script: name (u16 length + bytes), flags (u8: 0 local, 1 inherited, 2 removed, 3
//!   inherited and removed), property count (u16), properties;
//! - property (and struct member): name (u16 length + bytes), type (u8), flags (u8: 1
//!   edited, 3 removed), value by type: 0 none and 6 variable (no bytes), 1 object, 2
//!   string (u16 length + bytes), 3 i32, 4 f32, 5 bool (u8), 7 struct (u32 member count,
//!   members), 11..15 arrays (u32 count) of object, string, i32, f32, bool, 16 an array of
//!   variables (a u32 count only), 17 an array (u32 count) of structs;
//! - object: object format 2 is unused (u16), alias (i16), FormID (u32); format 1 is
//!   FormID, alias, unused.
//!
//! Corpus check (`tests/vmad_corpus.rs`, release, ignored): every `VMAD` of the 14
//! official files decodes and re-encodes byte-identically through this module.

/// The subrecord signature.
pub const VMAD: [u8; 4] = *b"VMAD";

/// Deepest struct nesting followed (official scripts nest structs a level or two).
const MAX_DEPTH: usize = 16;

/// An object value as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Object {
    /// The FormID, in the plugin's FormID space.
    pub form_id: u32,
    /// The quest alias index (-1: none).
    pub alias: i16,
    /// The unused two bytes, as stored.
    pub unused: u16,
}

/// A property value, by type byte.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Type 0: no value.
    None,
    /// Type 1.
    Object(Object),
    /// Type 2: bytes as stored.
    String(Vec<u8>),
    /// Type 3.
    Int(i32),
    /// Type 4: the bits as stored.
    Float(u32),
    /// Type 5: the byte as stored.
    Bool(u8),
    /// Type 6: a variable, no bytes.
    Variable,
    /// Type 7: members.
    Struct(Vec<Property>),
    /// Type 11.
    Objects(Vec<Object>),
    /// Type 12.
    Strings(Vec<Vec<u8>>),
    /// Type 13.
    Ints(Vec<i32>),
    /// Type 14.
    Floats(Vec<u32>),
    /// Type 15.
    Bools(Vec<u8>),
    /// Type 16: the element count only.
    Variables(u32),
    /// Type 17.
    Structs(Vec<Vec<Property>>),
}

/// A property or struct member.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// The name as stored.
    pub name: Vec<u8>,
    /// The type byte.
    pub ty: u8,
    /// The flags byte.
    pub flags: u8,
    /// The value.
    pub value: Value,
}

/// A script entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// The script name as stored (`Namespace:Name`).
    pub name: Vec<u8>,
    /// The flags byte.
    pub flags: u8,
    /// Its properties.
    pub properties: Vec<Property>,
}

impl Script {
    /// The name as text.
    pub fn name_text(&self) -> String {
        String::from_utf8_lossy(&self.name).into_owned()
    }
}

/// A `VMAD` payload.
#[derive(Debug, Clone, PartialEq)]
pub struct Vmad {
    /// `Version`.
    pub version: i16,
    /// `ObjectFormat`.
    pub object_format: i16,
    /// The scripts.
    pub scripts: Vec<Script>,
    /// Bytes after the scripts (a fragment block), unchanged.
    pub rest: Vec<u8>,
}

impl Vmad {
    /// An empty attachment in the format every official record uses (version 6, object
    /// format 2: `ScriptAttachment.toml`'s observed values).
    pub fn new() -> Vmad {
        Vmad {
            version: 6,
            object_format: 2,
            scripts: Vec::new(),
            rest: Vec::new(),
        }
    }
}

impl Default for Vmad {
    fn default() -> Self {
        Vmad::new()
    }
}

struct Cursor<'d> {
    data: &'d [u8],
    pos: usize,
    /// Payload offset of each object's FormID, in decode order.
    marks: Vec<usize>,
}

impl<'d> Cursor<'d> {
    fn take(&mut self, n: usize) -> Result<&'d [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| format!("size overflow at {}", self.pos))?;
        let s = self.data.get(self.pos..end).ok_or_else(|| {
            format!(
                "VMAD ends at {} (needed {n} bytes at {})",
                self.data.len(),
                self.pos
            )
        })?;
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        self.take(1)?
            .first()
            .copied()
            .ok_or_else(|| "short".to_string())
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes(b.try_into().map_err(|_| "u16")?))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes(b.try_into().map_err(|_| "u32")?))
    }
    fn text(&mut self) -> Result<Vec<u8>, String> {
        let n = self.u16()?;
        Ok(self.take(usize::from(n))?.to_vec())
    }
    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    /// A count of elements at least `min` bytes each: refused when the payload cannot
    /// hold them (no allocation from a hostile count).
    fn count(&mut self, min: usize) -> Result<usize, String> {
        let n = usize::try_from(self.u32()?).map_err(|_| "count")?;
        if n.saturating_mul(min.max(1)) > self.remaining() && min > 0 {
            return Err(format!(
                "count {n} at {} exceeds the {} bytes left",
                self.pos.saturating_sub(4),
                self.remaining()
            ));
        }
        Ok(n)
    }
}

fn object(c: &mut Cursor<'_>, format: i16) -> Result<Object, String> {
    if format == 1 {
        c.marks.push(c.pos);
        let form_id = c.u32()?;
        let alias = c.u16()? as i16;
        let unused = c.u16()?;
        Ok(Object {
            form_id,
            alias,
            unused,
        })
    } else {
        let unused = c.u16()?;
        let alias = c.u16()? as i16;
        c.marks.push(c.pos);
        let form_id = c.u32()?;
        Ok(Object {
            form_id,
            alias,
            unused,
        })
    }
}

fn members(c: &mut Cursor<'_>, format: i16, depth: usize) -> Result<Vec<Property>, String> {
    let n = c.count(4)?;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(property(c, format, depth)?);
    }
    Ok(out)
}

fn value(c: &mut Cursor<'_>, ty: u8, format: i16, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err(format!("structs nested deeper than {MAX_DEPTH}"));
    }
    let next = depth.saturating_add(1);
    Ok(match ty {
        0 => Value::None,
        1 => Value::Object(object(c, format)?),
        2 => Value::String(c.text()?),
        3 => Value::Int(c.u32()? as i32),
        4 => Value::Float(c.u32()?),
        5 => Value::Bool(c.u8()?),
        6 => Value::Variable,
        7 => Value::Struct(members(c, format, next)?),
        11 => {
            let n = c.count(8)?;
            let mut v = Vec::new();
            for _ in 0..n {
                v.push(object(c, format)?);
            }
            Value::Objects(v)
        }
        12 => {
            let n = c.count(2)?;
            let mut v = Vec::new();
            for _ in 0..n {
                v.push(c.text()?);
            }
            Value::Strings(v)
        }
        13 | 14 => {
            let n = c.count(4)?;
            let mut v = Vec::new();
            for _ in 0..n {
                v.push(c.u32()?);
            }
            if ty == 13 {
                Value::Ints(v.into_iter().map(|x| x as i32).collect())
            } else {
                Value::Floats(v)
            }
        }
        15 => {
            let n = c.count(1)?;
            Value::Bools(c.take(n)?.to_vec())
        }
        16 => Value::Variables(c.u32()?),
        17 => {
            let n = c.count(4)?;
            let mut v = Vec::new();
            for _ in 0..n {
                v.push(members(c, format, next)?);
            }
            Value::Structs(v)
        }
        other => return Err(format!("unknown property type {other} at {}", c.pos)),
    })
}

fn property(c: &mut Cursor<'_>, format: i16, depth: usize) -> Result<Property, String> {
    let name = c.text()?;
    let ty = c.u8()?;
    let flags = c.u8()?;
    let value = value(c, ty, format, depth)?;
    Ok(Property {
        name,
        ty,
        flags,
        value,
    })
}

/// Decodes a `VMAD` payload: the header, every script entry, and the rest as bytes.
pub fn decode(data: &[u8]) -> Result<Vmad, String> {
    decode_marked(data).map(|(v, _)| v)
}

/// One object value of a `VMAD` payload: what [`objects`] lists, with where its FormID is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRef {
    /// `Script.Property`, `Script.Property[2]`, `Script.Property.Member` (as [`objects`]).
    pub path: String,
    /// The object as stored.
    pub object: Object,
    /// Offset of the FormID's four bytes in the payload.
    pub at: usize,
}

/// What [`object_refs`] read of one payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectScan {
    /// Every object value of the script entries, in payload order.
    pub objects: Vec<ObjectRef>,
    /// Bytes after the script entries (the fragment block of `QUST`, `INFO`, `PACK`,
    /// `SCEN`, `PERK`: on `QUST` it holds alias scripts with properties of their own),
    /// which are not read for objects.
    pub unread_tail: usize,
}

/// The object values (FormIDs) of a `VMAD` payload's script entries, with their payload
/// offsets; an error when the payload does not decode ([`decode`]).
pub fn object_refs(data: &[u8]) -> Result<ObjectScan, String> {
    let (v, marks) = decode_marked(data)?;
    let listed = objects(&v);
    if listed.len() != marks.len() {
        return Err(format!(
            "{} object values decoded but {} listed (structs nested deeper than {MAX_DEPTH})",
            marks.len(),
            listed.len()
        ));
    }
    Ok(ObjectScan {
        objects: listed
            .into_iter()
            .zip(marks)
            .map(|((path, object), at)| ObjectRef { path, object, at })
            .collect(),
        unread_tail: v.rest.len(),
    })
}

/// [`decode`], and the payload offset of every object's FormID in decode order (the order
/// [`objects`] lists them).
fn decode_marked(data: &[u8]) -> Result<(Vmad, Vec<usize>), String> {
    let mut c = Cursor {
        data,
        pos: 0,
        marks: Vec::new(),
    };
    let version = c.u16()? as i16;
    let object_format = c.u16()? as i16;
    let count = c.u16()?;
    let mut scripts = Vec::new();
    for _ in 0..count {
        let name = c.text()?;
        let flags = c.u8()?;
        let n = c.u16()?;
        let mut properties = Vec::new();
        for _ in 0..n {
            properties.push(property(&mut c, object_format, 0)?);
        }
        scripts.push(Script {
            name,
            flags,
            properties,
        });
    }
    let rest = data.get(c.pos..).unwrap_or_default().to_vec();
    Ok((
        Vmad {
            version,
            object_format,
            scripts,
            rest,
        },
        c.marks,
    ))
}

fn put_text(out: &mut Vec<u8>, text: &[u8]) -> Result<(), String> {
    let n = u16::try_from(text.len())
        .map_err(|_| format!("a name or string of {} bytes (at most 65,535)", text.len()))?;
    out.extend(n.to_le_bytes());
    out.extend_from_slice(text);
    Ok(())
}

fn put_count(out: &mut Vec<u8>, n: usize) -> Result<(), String> {
    let n = u32::try_from(n).map_err(|_| "too many elements")?;
    out.extend(n.to_le_bytes());
    Ok(())
}

fn put_object(out: &mut Vec<u8>, o: &Object, format: i16) {
    if format == 1 {
        out.extend(o.form_id.to_le_bytes());
        out.extend(o.alias.to_le_bytes());
        out.extend(o.unused.to_le_bytes());
    } else {
        out.extend(o.unused.to_le_bytes());
        out.extend(o.alias.to_le_bytes());
        out.extend(o.form_id.to_le_bytes());
    }
}

fn put_members(out: &mut Vec<u8>, list: &[Property], format: i16) -> Result<(), String> {
    put_count(out, list.len())?;
    for p in list {
        put_property(out, p, format)?;
    }
    Ok(())
}

fn put_property(out: &mut Vec<u8>, p: &Property, format: i16) -> Result<(), String> {
    put_text(out, &p.name)?;
    out.push(p.ty);
    out.push(p.flags);
    match &p.value {
        Value::None | Value::Variable => {}
        Value::Object(o) => put_object(out, o, format),
        Value::String(s) => put_text(out, s)?,
        Value::Int(i) => out.extend(i.to_le_bytes()),
        Value::Float(f) => out.extend(f.to_le_bytes()),
        Value::Bool(b) => out.push(*b),
        Value::Struct(m) => put_members(out, m, format)?,
        Value::Objects(v) => {
            put_count(out, v.len())?;
            for o in v {
                put_object(out, o, format);
            }
        }
        Value::Strings(v) => {
            put_count(out, v.len())?;
            for s in v {
                put_text(out, s)?;
            }
        }
        Value::Ints(v) => {
            put_count(out, v.len())?;
            v.iter().for_each(|x| out.extend(x.to_le_bytes()));
        }
        Value::Floats(v) => {
            put_count(out, v.len())?;
            v.iter().for_each(|x| out.extend(x.to_le_bytes()));
        }
        Value::Bools(v) => {
            put_count(out, v.len())?;
            out.extend_from_slice(v);
        }
        Value::Variables(n) => out.extend(n.to_le_bytes()),
        Value::Structs(v) => {
            put_count(out, v.len())?;
            for m in v {
                put_members(out, m, format)?;
            }
        }
    }
    Ok(())
}

/// Encodes a `VMAD` payload (the inverse of [`decode`]).
pub fn encode(v: &Vmad) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.extend(v.version.to_le_bytes());
    out.extend(v.object_format.to_le_bytes());
    let count = u16::try_from(v.scripts.len()).map_err(|_| "more than 65,535 scripts")?;
    out.extend(count.to_le_bytes());
    for s in &v.scripts {
        put_text(&mut out, &s.name)?;
        out.push(s.flags);
        let n = u16::try_from(s.properties.len()).map_err(|_| "more than 65,535 properties")?;
        out.extend(n.to_le_bytes());
        for p in &s.properties {
            put_property(&mut out, p, v.object_format)?;
        }
    }
    out.extend_from_slice(&v.rest);
    Ok(out)
}

/// Every object FormID in the scripts, with the property path that holds it
/// (`Script.Property`, `Script.Property[2]`, `Script.Property.Member`).
pub fn objects(v: &Vmad) -> Vec<(String, Object)> {
    fn walk(at: &str, p: &Property, out: &mut Vec<(String, Object)>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let here = format!("{at}.{}", String::from_utf8_lossy(&p.name));
        match &p.value {
            Value::Object(o) => out.push((here, *o)),
            Value::Objects(v) => {
                for (i, o) in v.iter().enumerate() {
                    out.push((format!("{here}[{i}]"), *o));
                }
            }
            Value::Struct(m) => m
                .iter()
                .for_each(|q| walk(&here, q, out, depth.saturating_add(1))),
            Value::Structs(v) => {
                for (i, m) in v.iter().enumerate() {
                    let at = format!("{here}[{i}]");
                    m.iter()
                        .for_each(|q| walk(&at, q, out, depth.saturating_add(1)));
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &v.scripts {
        let name = s.name_text();
        for p in &s.properties {
            walk(&name, p, &mut out, 0);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn object(form_id: u32) -> Object {
        Object {
            form_id,
            alias: -1,
            unused: 0,
        }
    }

    fn prop(name: &str, ty: u8, value: Value) -> Property {
        Property {
            name: name.as_bytes().to_vec(),
            ty,
            flags: 1,
            value,
        }
    }

    #[test]
    fn object_refs_point_at_each_form_id() {
        for format in [1i16, 2] {
            let v = Vmad {
                version: 6,
                object_format: format,
                scripts: vec![Script {
                    name: b"KK:Test".to_vec(),
                    flags: 0,
                    properties: vec![
                        prop("Count", 3, Value::Int(7)),
                        prop("Target", 1, Value::Object(object(0x0100_0800))),
                        prop(
                            "List",
                            11,
                            Value::Objects(vec![object(0x0000_0014), object(0x0200_0801)]),
                        ),
                        prop(
                            "Pair",
                            7,
                            Value::Struct(vec![prop("Ref", 1, Value::Object(object(0x0100_0802)))]),
                        ),
                    ],
                }],
                rest: vec![1, 2, 3],
            };
            let bytes = encode(&v).unwrap();
            let scan = object_refs(&bytes).unwrap();
            let paths: Vec<&str> = scan.objects.iter().map(|o| o.path.as_str()).collect();
            assert_eq!(
                paths,
                [
                    "KK:Test.Target",
                    "KK:Test.List[0]",
                    "KK:Test.List[1]",
                    "KK:Test.Pair.Ref"
                ]
            );
            for o in &scan.objects {
                let at = bytes.get(o.at..o.at + 4).unwrap();
                assert_eq!(u32::from_le_bytes(at.try_into().unwrap()), o.object.form_id);
            }
            assert_eq!(scan.unread_tail, 3);
        }
        assert!(object_refs(&[6, 0, 2, 0, 1, 0]).is_err());
    }
}
