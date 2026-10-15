//! Script attachment (M4.E9): the scripts list of a `VMAD` subrecord, decoded and encoded
//! losslessly, and the attach operation `place`, `set` and `new` use.
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

use kitkraft_esm::{DecodeLimits, Record, Signature, Subrecord};

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
    let mut c = Cursor { data, pos: 0 };
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
    Ok(Vmad {
        version,
        object_format,
        scripts,
        rest,
    })
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

/// A value to attach, typed by the script's declaration (see `handlers::scripts`).
#[derive(Debug, Clone, PartialEq)]
pub enum AttachValue {
    /// An object: a form of the load order, a form written in the same call (by
    /// EditorID), or none.
    Object(ObjectTarget),
    /// A string.
    String(String),
    /// An `Int`.
    Int(i32),
    /// A `Float`.
    Float(f32),
    /// A `Bool`.
    Bool(bool),
    /// An array: the element type byte (1..5) and the elements.
    Array(u8, Vec<AttachValue>),
}

/// What an object property points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectTarget {
    /// A form of the load order (plugin file name, object ID).
    Form(super::patch::FormName),
    /// A form the same call writes, by its EditorID.
    Local(String),
    /// No object (FormID 0).
    Null,
}

/// One script to attach with the property values to set.
#[derive(Debug, Clone, PartialEq)]
pub struct Attach {
    /// The script name (`Namespace:Name`).
    pub name: String,
    /// Properties as `(name, value)`, names as the declaration spells them.
    pub properties: Vec<(String, AttachValue)>,
}

impl AttachValue {
    /// The VMAD type byte.
    pub fn type_byte(&self) -> u8 {
        match self {
            AttachValue::Object(_) => 1,
            AttachValue::String(_) => 2,
            AttachValue::Int(_) => 3,
            AttachValue::Float(_) => 4,
            AttachValue::Bool(_) => 5,
            AttachValue::Array(t, _) => t.saturating_add(10),
        }
    }
}

fn stored_value(
    v: &AttachValue,
    form: &dyn Fn(&ObjectTarget) -> Result<u32, String>,
) -> Result<Value, String> {
    let obj = |t: &ObjectTarget| -> Result<Object, String> {
        Ok(Object {
            form_id: form(t)?,
            alias: -1,
            unused: 0,
        })
    };
    Ok(match v {
        AttachValue::Object(t) => Value::Object(obj(t)?),
        AttachValue::String(s) => Value::String(s.as_bytes().to_vec()),
        AttachValue::Int(i) => Value::Int(*i),
        AttachValue::Float(f) => Value::Float(f.to_bits()),
        AttachValue::Bool(b) => Value::Bool(u8::from(*b)),
        AttachValue::Array(t, items) => {
            let wrong = || format!("an array element is not of type {t}");
            match t {
                1 => Value::Objects(
                    items
                        .iter()
                        .map(|i| match i {
                            AttachValue::Object(o) => obj(o),
                            _ => Err(wrong()),
                        })
                        .collect::<Result<_, _>>()?,
                ),
                2 => Value::Strings(
                    items
                        .iter()
                        .map(|i| match i {
                            AttachValue::String(s) => Ok(s.as_bytes().to_vec()),
                            _ => Err(wrong()),
                        })
                        .collect::<Result<_, _>>()?,
                ),
                3 => Value::Ints(
                    items
                        .iter()
                        .map(|i| match i {
                            AttachValue::Int(x) => Ok(*x),
                            _ => Err(wrong()),
                        })
                        .collect::<Result<_, _>>()?,
                ),
                4 => Value::Floats(
                    items
                        .iter()
                        .map(|i| match i {
                            AttachValue::Float(x) => Ok(x.to_bits()),
                            _ => Err(wrong()),
                        })
                        .collect::<Result<_, _>>()?,
                ),
                5 => Value::Bools(
                    items
                        .iter()
                        .map(|i| match i {
                            AttachValue::Bool(x) => Ok(u8::from(*x)),
                            _ => Err(wrong()),
                        })
                        .collect::<Result<_, _>>()?,
                ),
                other => return Err(format!("arrays of type {other} cannot be attached")),
            }
        }
    })
}

/// Where a new `VMAD` goes in a record of type `record` whose subrecords are `subs`: after
/// the last subrecord the schema's member order puts before it (after `EDID` when the
/// record type has no definition naming `VMAD`).
fn vmad_position(record: [u8; 4], subs: &[[u8; 4]]) -> usize {
    let order: Vec<[u8; 4]> = kitkraft_schema::builtin()
        .ok()
        .and_then(|s| kitkraft_schema::Signature::from_bytes(record).and_then(|r| s.record(r)))
        .map(|d| {
            d.leaf_subrecords()
                .into_iter()
                .map(|(_, m)| m.signature.as_bytes())
                .collect()
        })
        .unwrap_or_default();
    let rank = |sig: &[u8; 4]| order.iter().position(|s| s == sig);
    match rank(b"VMAD") {
        Some(vmad) => subs
            .iter()
            .enumerate()
            .filter(|(_, s)| rank(s).is_some_and(|r| r < vmad))
            .map(|(i, _)| i.saturating_add(1))
            .max()
            .unwrap_or(0),
        None => subs
            .iter()
            .position(|s| s == b"EDID")
            .map_or(0, |i| i.saturating_add(1)),
    }
}

/// What [`attach`] did to one script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    /// The script.
    pub script: String,
    /// `added` (a new entry) or `updated` (an entry the record had).
    pub outcome: &'static str,
    /// The properties set.
    pub properties: Vec<String>,
}

/// Attaches `scripts` to `record`: each script is added to its `VMAD` (created in the
/// schema's position when absent), or, when the record already has a script of that name
/// (ASCII case-insensitive), the given properties are set on it and the others kept. A
/// new property gets flags 1 (edited), as upstream's default; object values get alias -1
/// (none). `form` maps an object target to the FormID stored in the record's plugin.
pub fn attach(
    record: &mut Record<'_>,
    scripts: &[Attach],
    form: &dyn Fn(&ObjectTarget) -> Result<u32, String>,
    limits: &DecodeLimits,
) -> Result<Vec<Attached>, String> {
    record.decompress(limits).map_err(|e| e.to_string())?;
    let sig = Signature(*b"VMAD");
    let index = record.find_subrecord(sig, 0);
    let mut vmad = match index {
        Some(i) => {
            let data = record
                .subrecords()
                .and_then(|s| s.get(i))
                .map(|s| s.data().to_vec())
                .unwrap_or_default();
            decode(&data).map_err(|e| format!("the record's VMAD does not decode: {e}"))?
        }
        None => Vmad::new(),
    };
    let mut report = Vec::new();
    for a in scripts {
        let mut props = Vec::new();
        for (name, v) in &a.properties {
            props.push(Property {
                name: name.as_bytes().to_vec(),
                ty: v.type_byte(),
                flags: 1,
                value: stored_value(v, form).map_err(|e| format!("{}.{name}: {e}", a.name))?,
            });
        }
        let names: Vec<String> = a.properties.iter().map(|(n, _)| n.clone()).collect();
        match vmad
            .scripts
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(a.name.as_bytes()))
        {
            Some(existing) => {
                for p in props {
                    match existing
                        .properties
                        .iter_mut()
                        .find(|q| q.name.eq_ignore_ascii_case(&p.name))
                    {
                        Some(q) => *q = p,
                        None => existing.properties.push(p),
                    }
                }
                report.push(Attached {
                    script: a.name.clone(),
                    outcome: "updated",
                    properties: names,
                });
            }
            None => {
                vmad.scripts.push(Script {
                    name: a.name.as_bytes().to_vec(),
                    flags: 0,
                    properties: props,
                });
                report.push(Attached {
                    script: a.name.clone(),
                    outcome: "added",
                    properties: names,
                });
            }
        }
    }
    let bytes = encode(&vmad)?;
    match index {
        Some(i) => {
            record
                .set_subrecord(i, bytes, limits)
                .map_err(|e| e.to_string())?;
        }
        None => {
            let sigs: Vec<[u8; 4]> = record
                .subrecords()
                .map(|s| s.iter().map(|x| x.signature().0).collect())
                .unwrap_or_default();
            let at = vmad_position(record.header.signature.0, &sigs);
            let subs = record
                .subrecords_mut()
                .ok_or("the record's subrecords cannot be edited")?;
            subs.insert(at.min(subs.len()), Subrecord::new(sig, bytes));
        }
    }
    Ok(report)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use kitkraft_esm::{Body, RecordHeader};

    fn prop(name: &str, ty: u8, value: Value) -> Property {
        Property {
            name: name.as_bytes().to_vec(),
            ty,
            flags: 1,
            value,
        }
    }

    #[test]
    fn every_type_roundtrips() {
        let o = Object {
            form_id: 0x0001_0203,
            alias: -1,
            unused: 0,
        };
        let v = Vmad {
            version: 6,
            object_format: 2,
            scripts: vec![Script {
                name: b"KK:Test".to_vec(),
                flags: 0,
                properties: vec![
                    prop("A", 0, Value::None),
                    prop("B", 1, Value::Object(o)),
                    prop("C", 2, Value::String(b"hi".to_vec())),
                    prop("D", 3, Value::Int(-7)),
                    prop("E", 4, Value::Float(1.5f32.to_bits())),
                    prop("F", 5, Value::Bool(1)),
                    prop("G", 6, Value::Variable),
                    prop("H", 7, Value::Struct(vec![prop("M", 3, Value::Int(2))])),
                    prop("I", 11, Value::Objects(vec![o, o])),
                    prop("J", 12, Value::Strings(vec![b"x".to_vec()])),
                    prop("K", 13, Value::Ints(vec![1, 2])),
                    prop("L", 14, Value::Floats(vec![0])),
                    prop("M", 15, Value::Bools(vec![0, 1])),
                    prop("N", 16, Value::Variables(0)),
                    prop(
                        "O",
                        17,
                        Value::Structs(vec![vec![prop("P", 5, Value::Bool(0))]]),
                    ),
                ],
            }],
            rest: vec![3, 0, 0],
        };
        let bytes = encode(&v).unwrap();
        assert_eq!(decode(&bytes).unwrap(), v);
        // Object format 2 stores unused, alias, FormID.
        let one = Vmad {
            scripts: vec![Script {
                name: b"S".to_vec(),
                flags: 0,
                properties: vec![prop("B", 1, Value::Object(o))],
            }],
            rest: Vec::new(),
            ..Vmad::new()
        };
        let b = encode(&one).unwrap();
        assert_eq!(&b[b.len() - 8..], [0, 0, 0xFF, 0xFF, 3, 2, 1, 0]);
        assert_eq!(objects(&one), [("S.B".to_string(), o)]);
    }

    #[test]
    fn hostile_payloads_are_errors() {
        // A struct member count far beyond the payload.
        let mut b = vec![6, 0, 2, 0, 1, 0, 1, 0, b'S', 0, 1, 0, 1, 0, b'P', 7, 1];
        b.extend(u32::MAX.to_le_bytes());
        assert!(decode(&b).is_err());
        // Truncated.
        assert!(decode(&[6, 0, 2]).is_err());
        // An unknown type.
        let bad = [6, 0, 2, 0, 1, 0, 1, 0, b'S', 0, 1, 0, 1, 0, b'P', 9, 1];
        assert!(decode(&bad)
            .unwrap_err()
            .contains("unknown property type 9"));
    }

    #[test]
    fn attach_adds_and_updates() {
        let mut r = Record::new(
            RecordHeader {
                signature: Signature(*b"ACTI"),
                form_id: 0x800,
                form_version: 582,
                ..RecordHeader::default()
            },
            Body::Raw(vec![
                Subrecord::new(Signature(*b"EDID"), b"X\0".to_vec()),
                Subrecord::new(Signature(*b"OBND"), vec![0; 12]),
            ]),
        );
        let map = |t: &ObjectTarget| match t {
            ObjectTarget::Null => Ok(0),
            _ => Ok(0x0100_0800),
        };
        let a = Attach {
            name: "KK:Test".into(),
            properties: vec![
                ("Count".into(), AttachValue::Int(3)),
                (
                    "Target".into(),
                    AttachValue::Object(ObjectTarget::Local("Y".into())),
                ),
            ],
        };
        let done = attach(&mut r, &[a], &map, &DecodeLimits::default()).unwrap();
        assert_eq!(done[0].outcome, "added");
        let subs = r.subrecords().unwrap();
        // VMAD goes after EDID, before OBND (ACTI's member order).
        assert_eq!(subs[1].signature(), Signature(*b"VMAD"));
        let v = decode(subs[1].data()).unwrap();
        assert_eq!(v.scripts[0].properties.len(), 2);
        assert_eq!(objects(&v)[0].1.form_id, 0x0100_0800);
        // Again with one property: the entry is updated, the other property kept.
        let again = Attach {
            name: "kk:test".into(),
            properties: vec![("Count".into(), AttachValue::Int(4))],
        };
        let done = attach(&mut r, &[again], &map, &DecodeLimits::default()).unwrap();
        assert_eq!(done[0].outcome, "updated");
        let v = decode(r.subrecords().unwrap()[1].data()).unwrap();
        assert_eq!(v.scripts.len(), 1);
        assert_eq!(v.scripts[0].properties[0].value, Value::Int(4));
        assert_eq!(v.scripts[0].properties.len(), 2);
    }
}
