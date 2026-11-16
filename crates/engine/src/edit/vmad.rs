//! Script attachment (M4.E9): the attach operation `place`, `set` and `new` use on a
//! record's `VMAD` subrecord.
//!
//! The payload's data types, its lossless decoder and encoder and the object walk live in
//! `kitkraft_doc::vmad` (moved there for M5.D6, so `refs` and `validate` read object
//! property values through the same code) and are re-exported here.

use kitkraft_esm::{DecodeLimits, Record, Signature, Subrecord};

pub use kitkraft_doc::vmad::{
    decode, decode_quest_fragments, encode, encode_quest_fragments, objects, AliasScripts, Object,
    Property, QuestFragment, QuestFragments, Script, Value, Vmad,
};

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
    /// Quest stage fragments this script holds (`QUST` only). When not empty the script is
    /// the quest's fragment script, written into the fragment block (see [`attach`]).
    pub stages: Vec<StageFragment>,
}

/// One quest stage fragment: the function of the fragment script that runs at a stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageFragment {
    /// The quest stage.
    pub stage: u32,
    /// The log entry index within the stage (0 for the first).
    pub index: u32,
    /// The function's name (`Fragment_Stage_0010_Item_00`).
    pub function: String,
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
    /// `added` (a new entry) or `updated` (an entry the record had); for a quest's fragment
    /// script, `fragments-added` or `fragments-updated`.
    pub outcome: &'static str,
    /// The properties set.
    pub properties: Vec<String>,
}

/// Attaches `scripts` to `record`: each script is added to its `VMAD` (created in the
/// schema's position when absent), or, when the record already has a script of that name
/// (ASCII case-insensitive), the given properties are set on it and the others kept. A
/// new property gets flags 1 (edited), as upstream's default; object values get alias -1
/// (none). `form` maps an object target to the FormID stored in the record's plugin.
///
/// A script with `stages` is a quest's fragment script: on a `QUST` only, it goes into the
/// `VMAD`'s fragment block ([`QuestFragments`], decoded from the record's block or new),
/// not its script list, with one stage fragment per stage (replacing a fragment of the
/// same stage and index in place, else appended in the order given: official quests keep
/// no particular order, 886 of 1,494 blocks are not in stage order) and the unknown byte
/// 1, as in every official quest. A quest holds one fragment script; naming another is an
/// error. A quest with scripts and no fragments gets no fragment block: so are the 209
/// official quests of that shape, and no official block is empty
/// (`tests/quest_vmad_corpus.rs`).
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
    let quest = record.header.signature == Signature(*b"QUST");
    for a in scripts.iter().filter(|a| !a.stages.is_empty()) {
        if !quest {
            return Err(format!(
                "{}: stage fragments attach only to a QUST, not a {}",
                a.name, record.header.signature
            ));
        }
        let mut props = Vec::new();
        for (name, v) in &a.properties {
            props.push(Property {
                name: name.as_bytes().to_vec(),
                ty: v.type_byte(),
                flags: 1,
                value: stored_value(v, form).map_err(|e| format!("{}.{name}: {e}", a.name))?,
            });
        }
        let mut q = if vmad.rest.is_empty() {
            QuestFragments::new()
        } else {
            decode_quest_fragments(&vmad.rest, vmad.object_format)
                .map_err(|e| format!("the quest's fragment block does not decode: {e}"))?
        };
        let outcome = match &mut q.script {
            Some(s) if !s.name.eq_ignore_ascii_case(a.name.as_bytes()) => {
                return Err(format!(
                    "{}: the quest's fragment script is `{}`; a quest holds one",
                    a.name,
                    s.name_text()
                ));
            }
            Some(s) => {
                for p in props {
                    match s
                        .properties
                        .iter_mut()
                        .find(|x| x.name.eq_ignore_ascii_case(&p.name))
                    {
                        Some(x) => *x = p,
                        None => s.properties.push(p),
                    }
                }
                "fragments-updated"
            }
            None => {
                q.script = Some(Script {
                    name: a.name.as_bytes().to_vec(),
                    flags: 0,
                    properties: props,
                });
                "fragments-added"
            }
        };
        let name = q
            .script
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_default();
        for st in &a.stages {
            let f = QuestFragment {
                stage: st.stage,
                stage_index: st.index,
                unknown: 1,
                script_name: name.clone(),
                fragment_name: st.function.as_bytes().to_vec(),
            };
            match q
                .fragments
                .iter_mut()
                .find(|x| x.stage == st.stage && x.stage_index == st.index)
            {
                Some(x) => *x = f,
                None => q.fragments.push(f),
            }
        }
        vmad.rest = encode_quest_fragments(&q, vmad.object_format)?;
        report.push(Attached {
            script: a.name.clone(),
            outcome,
            properties: a.properties.iter().map(|(n, _)| n.clone()).collect(),
        });
    }
    for a in scripts.iter().filter(|a| a.stages.is_empty()) {
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
            stages: Vec::new(),
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
            stages: Vec::new(),
        };
        let done = attach(&mut r, &[again], &map, &DecodeLimits::default()).unwrap();
        assert_eq!(done[0].outcome, "updated");
        let v = decode(r.subrecords().unwrap()[1].data()).unwrap();
        assert_eq!(v.scripts.len(), 1);
        assert_eq!(v.scripts[0].properties[0].value, Value::Int(4));
        assert_eq!(v.scripts[0].properties.len(), 2);
    }

    fn record(sig: &[u8; 4]) -> Record<'static> {
        Record::new(
            RecordHeader {
                signature: Signature(*sig),
                form_id: 0x800,
                form_version: 582,
                ..RecordHeader::default()
            },
            Body::Raw(vec![Subrecord::new(Signature(*b"EDID"), b"Q\0".to_vec())]),
        )
    }

    fn stage(stage: u32, index: u32, function: &str) -> StageFragment {
        StageFragment {
            stage,
            index,
            function: function.into(),
        }
    }

    fn fragment_names(r: &Record<'_>) -> Vec<String> {
        let v = decode(r.subrecords().unwrap()[1].data()).unwrap();
        decode_quest_fragments(&v.rest, 2)
            .unwrap()
            .fragments
            .iter()
            .map(|f| String::from_utf8_lossy(&f.fragment_name).into_owned())
            .collect()
    }

    #[test]
    fn attach_writes_a_quests_fragment_block() {
        let map = |_: &ObjectTarget| Ok(0x0100_0801);
        let limits = DecodeLimits::default();
        // A quest script alone: the script list, no block (as 209 official quests).
        let mut r = record(b"QUST");
        let main = Attach {
            name: "KK:Main".into(),
            properties: Vec::new(),
            stages: Vec::new(),
        };
        attach(&mut r, std::slice::from_ref(&main), &map, &limits).unwrap();
        let v = decode(r.subrecords().unwrap()[1].data()).unwrap();
        assert!(v.rest.is_empty());
        // A fragment script with two stages, kept in the order given.
        let frag = Attach {
            name: "Fragments:Quests:QF_Q".into(),
            properties: vec![(
                "Marker".into(),
                AttachValue::Object(ObjectTarget::Local("M".into())),
            )],
            stages: vec![stage(20, 0, "B"), stage(10, 0, "A")],
        };
        let done = attach(&mut r, std::slice::from_ref(&frag), &map, &limits).unwrap();
        assert_eq!(done[0].outcome, "fragments-added");
        let v = decode(r.subrecords().unwrap()[1].data()).unwrap();
        assert_eq!(v.scripts.len(), 1, "the fragment script is not in the list");
        let q = decode_quest_fragments(&v.rest, 2).unwrap();
        assert_eq!(q.script.as_ref().unwrap().name, b"Fragments:Quests:QF_Q");
        assert!(q.fragments.iter().all(|f| f.unknown == 1));
        assert_eq!(fragment_names(&r), ["B", "A"]);
        // Again (any case): stage 10 replaced, stage 30 added, the property kept.
        let again = Attach {
            name: "fragments:quests:qf_q".into(),
            properties: Vec::new(),
            stages: vec![stage(10, 0, "A2"), stage(30, 0, "C")],
        };
        let done = attach(&mut r, &[again], &map, &limits).unwrap();
        assert_eq!(done[0].outcome, "fragments-updated");
        assert_eq!(fragment_names(&r), ["B", "A2", "C"]);
        let v = decode(r.subrecords().unwrap()[1].data()).unwrap();
        let q = decode_quest_fragments(&v.rest, 2).unwrap();
        assert_eq!(q.script.as_ref().unwrap().properties.len(), 1);
        // Another fragment script on the same quest is refused.
        let other = Attach {
            name: "Fragments:Quests:QF_Other".into(),
            properties: Vec::new(),
            stages: vec![stage(5, 0, "X")],
        };
        assert!(attach(&mut r, &[other], &map, &limits)
            .unwrap_err()
            .contains("a quest holds one"));
        // Stages on anything but a quest are refused.
        let mut acti = record(b"ACTI");
        assert!(attach(&mut acti, &[frag], &map, &limits)
            .unwrap_err()
            .contains("only to a QUST"));
    }
}
