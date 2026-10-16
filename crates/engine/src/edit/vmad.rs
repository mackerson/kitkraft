//! Script attachment (M4.E9): the attach operation `place`, `set` and `new` use on a
//! record's `VMAD` subrecord.
//!
//! The payload's data types, its lossless decoder and encoder and the object walk live in
//! `kitkraft_doc::vmad` (moved there for M5.D6, so `refs` and `validate` read object
//! property values through the same code) and are re-exported here.

use kitkraft_esm::{DecodeLimits, Record, Signature, Subrecord};

pub use kitkraft_doc::vmad::{decode, encode, objects, Object, Property, Script, Value, Vmad};

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
