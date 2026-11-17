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
    /// The quest alias this script is attached to (`QUST` only). When set the script goes
    /// into that alias's entry of the fragment block, not the script list (see [`attach`]).
    pub alias: Option<AliasTarget>,
}

/// A quest alias named by a script argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliasTarget {
    /// By alias ID (`ALST`, `ALLS` or `ALCS` of the quest).
    Id(u32),
    /// By alias name (`ALID`, ASCII case-insensitive).
    Name(String),
}

impl std::fmt::Display for AliasTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AliasTarget::Id(id) => write!(f, "{id}"),
            AliasTarget::Name(n) => write!(f, "`{n}`"),
        }
    }
}

/// One alias a quest record declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestAlias {
    /// The alias ID.
    pub id: u32,
    /// `reference` (`ALST`), `location` (`ALLS`) or `collection` (`ALCS`).
    pub kind: &'static str,
    /// Its name (`ALID`; a collection's is its template reference alias's), when present.
    pub name: Option<String>,
}

/// The aliases a quest's subrecords declare, in record order: each reference alias
/// (`ALST`), location alias (`ALLS`) and reference-collection alias (`ALCS` outside an
/// alias; inside a reference alias `ALCS` is its fill's collection, not an alias), with the
/// `ALID` that follows (`crates/schema/defs/QUST.toml`, "Aliases"). A collection's template
/// reference alias is not listed on its own; its `ALID` names the collection.
pub fn quest_aliases(subs: &[Subrecord<'_>]) -> Vec<QuestAlias> {
    let mut out: Vec<QuestAlias> = Vec::new();
    // Inside an alias (until its ALED); after a collection's ALCS (until its ALED).
    let mut inside = false;
    let mut collection = false;
    let mut named = false;
    for s in subs {
        let id = || {
            s.data()
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_le_bytes)
        };
        match &s.signature().0 {
            b"ALST" if collection => {
                inside = true;
            }
            b"ALST" | b"ALLS" => {
                if let Some(id) = id() {
                    out.push(QuestAlias {
                        id,
                        kind: if s.signature().0 == *b"ALST" {
                            "reference"
                        } else {
                            "location"
                        },
                        name: None,
                    });
                    named = false;
                }
                inside = true;
            }
            b"ALCS" if !inside => {
                if let Some(id) = id() {
                    out.push(QuestAlias {
                        id,
                        kind: "collection",
                        name: None,
                    });
                    named = false;
                }
                collection = true;
            }
            b"ALID" if inside && !named => {
                let text = s.data().split(|b| *b == 0).next().unwrap_or_default();
                if let Some(last) = out.last_mut() {
                    last.name = Some(String::from_utf8_lossy(text).into_owned());
                }
                named = true;
            }
            b"ALED" => {
                inside = false;
                collection = false;
            }
            _ => {}
        }
    }
    out
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

/// The quest fragment block of `vmad` ([`Vmad::rest`]), decoded, or a new empty one.
fn block(vmad: &Vmad) -> Result<QuestFragments, String> {
    if vmad.rest.is_empty() {
        Ok(QuestFragments::new())
    } else {
        decode_quest_fragments(&vmad.rest, vmad.object_format)
            .map_err(|e| format!("the quest's fragment block does not decode: {e}"))
    }
}

/// What [`attach`] did to one script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    /// The script.
    pub script: String,
    /// `added` (a new entry) or `updated` (an entry the record had); for a quest's fragment
    /// script, `fragments-added` or `fragments-updated`; for a script on a quest alias,
    /// `alias-added` or `alias-updated`.
    pub outcome: &'static str,
    /// The properties set.
    pub properties: Vec<String>,
    /// The alias ID, for a script on a quest alias.
    pub alias: Option<u32>,
}

/// The ID of the alias `target` names among `aliases`, or an error listing them.
fn alias_id(target: &AliasTarget, aliases: &[QuestAlias]) -> Result<u32, String> {
    let found = aliases.iter().find(|a| match target {
        AliasTarget::Id(id) => a.id == *id,
        AliasTarget::Name(n) => a.name.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(n)),
    });
    match found {
        Some(a) => Ok(a.id),
        None => {
            let listed: Vec<String> = aliases
                .iter()
                .map(|a| match &a.name {
                    Some(n) => format!("{} `{n}` ({})", a.id, a.kind),
                    None => format!("{} ({})", a.id, a.kind),
                })
                .collect();
            Err(format!(
                "quest.alias-missing: the quest has no alias {target} (its aliases: {})",
                if listed.is_empty() {
                    "none".to_string()
                } else {
                    listed.join(", ")
                }
            ))
        }
    }
}

/// Sets `props` on `s`: a property of the same name (ASCII case-insensitive) replaced, a
/// new one appended.
fn merge_properties(s: &mut Script, props: Vec<Property>) {
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
///
/// A script with an `alias` goes, on a `QUST` only, into that alias's entry of the fragment
/// block, never the script list. The alias must be one the record declares
/// ([`quest_aliases`], so a `fields` edit of the same call that adds it counts), else the
/// call is refused (`quest.alias-missing`). A new entry is written as every one of the
/// 4,404 official entries is: the alias as an object value holding the quest's own FormID,
/// the alias ID and 0 unused, then version 6 and object format 2; entries are appended in
/// the order given (official blocks keep no particular order: 312 of 1,050 are not in ID
/// order), and a script of the same name on the same alias has its properties updated. A
/// block holding only alias scripts has an empty fragment script name and no fragments, as
/// 26 official quests have.
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
    let props_of = |a: &Attach| -> Result<Vec<Property>, String> {
        a.properties
            .iter()
            .map(|(name, v)| {
                Ok(Property {
                    name: name.as_bytes().to_vec(),
                    ty: v.type_byte(),
                    flags: 1,
                    value: stored_value(v, form).map_err(|e| format!("{}.{name}: {e}", a.name))?,
                })
            })
            .collect()
    };
    for a in scripts {
        if let (Some(alias), false) = (&a.alias, a.stages.is_empty()) {
            return Err(format!(
                "{}: a script on alias {alias} takes no stages (stage fragments belong to the quest's fragment script)",
                a.name
            ));
        }
    }
    for a in scripts.iter().filter(|a| !a.stages.is_empty()) {
        if !quest {
            return Err(format!(
                "{}: stage fragments attach only to a QUST, not a {}",
                a.name, record.header.signature
            ));
        }
        let props = props_of(a)?;
        let mut q = block(&vmad)?;
        let outcome = match &mut q.script {
            Some(s) if !s.name.eq_ignore_ascii_case(a.name.as_bytes()) => {
                return Err(format!(
                    "{}: the quest's fragment script is `{}`; a quest holds one",
                    a.name,
                    s.name_text()
                ));
            }
            Some(s) => {
                merge_properties(s, props);
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
            alias: None,
        });
    }
    let aliased: Vec<(&Attach, &AliasTarget)> = scripts
        .iter()
        .filter_map(|a| a.alias.as_ref().map(|t| (a, t)))
        .collect();
    if !aliased.is_empty() {
        if !quest {
            let names: Vec<&str> = aliased.iter().map(|(a, _)| a.name.as_str()).collect();
            return Err(format!(
                "{}: alias scripts attach only to a QUST, not a {}",
                names.join(", "),
                record.header.signature
            ));
        }
        let declared = record.subrecords().map(quest_aliases).unwrap_or_default();
        let quest_form = record.header.form_id;
        let mut q = block(&vmad)?;
        for (a, target) in aliased {
            let id = alias_id(target, &declared).map_err(|e| format!("{}: {e}", a.name))?;
            let id16 = i16::try_from(id).map_err(|_| {
                format!(
                    "{}: alias ID {id} does not fit the 16-bit alias of a VMAD object value",
                    a.name
                )
            })?;
            let props = props_of(a)?;
            let at = match q.aliases.iter().position(|e| e.object.alias == id16) {
                Some(i) => i,
                None => {
                    q.aliases.push(AliasScripts {
                        object: Object {
                            form_id: quest_form,
                            alias: id16,
                            unused: 0,
                        },
                        version: 6,
                        object_format: 2,
                        scripts: Vec::new(),
                    });
                    q.aliases.len().saturating_sub(1)
                }
            };
            let entry = q
                .aliases
                .get_mut(at)
                .ok_or("the alias entry was not kept")?;
            let outcome = match entry
                .scripts
                .iter_mut()
                .find(|s| s.name.eq_ignore_ascii_case(a.name.as_bytes()))
            {
                Some(s) => {
                    merge_properties(s, props);
                    "alias-updated"
                }
                None => {
                    entry.scripts.push(Script {
                        name: a.name.as_bytes().to_vec(),
                        flags: 0,
                        properties: props,
                    });
                    "alias-added"
                }
            };
            report.push(Attached {
                script: a.name.clone(),
                outcome,
                properties: a.properties.iter().map(|(n, _)| n.clone()).collect(),
                alias: Some(id),
            });
        }
        vmad.rest = encode_quest_fragments(&q, vmad.object_format)?;
    }
    for a in scripts
        .iter()
        .filter(|a| a.stages.is_empty() && a.alias.is_none())
    {
        let props = props_of(a)?;
        let names: Vec<String> = a.properties.iter().map(|(n, _)| n.clone()).collect();
        let outcome = match vmad
            .scripts
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(a.name.as_bytes()))
        {
            Some(existing) => {
                merge_properties(existing, props);
                "updated"
            }
            None => {
                vmad.scripts.push(Script {
                    name: a.name.as_bytes().to_vec(),
                    flags: 0,
                    properties: props,
                });
                "added"
            }
        };
        report.push(Attached {
            script: a.name.clone(),
            outcome,
            properties: names,
            alias: None,
        });
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
            alias: None,
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
            alias: None,
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
            alias: None,
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
            alias: None,
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
            alias: None,
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
            alias: None,
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

    /// A quest with a reference alias (0, `Player`, whose fill names collection 1 through
    /// its own `ALCS`), a collection alias (1, named by its template `Crowd`) and a location
    /// alias (2, `Place`).
    fn quest_with_aliases() -> Record<'static> {
        let sub = |sig: &[u8; 4], data: &[u8]| Subrecord::new(Signature(*sig), data.to_vec());
        Record::new(
            RecordHeader {
                signature: Signature(*b"QUST"),
                form_id: 0x0100_0802,
                form_version: 582,
                ..RecordHeader::default()
            },
            Body::Raw(vec![
                sub(b"EDID", b"Q\0"),
                sub(b"ANAM", &3u32.to_le_bytes()),
                sub(b"ALST", &0u32.to_le_bytes()),
                sub(b"ALID", b"Player\0"),
                sub(b"ALCS", &1i32.to_le_bytes()),
                sub(b"ALED", b""),
                sub(b"ALCS", &1u32.to_le_bytes()),
                sub(b"ALMI", &[0]),
                sub(b"ALST", &1u32.to_le_bytes()),
                sub(b"ALID", b"Crowd\0"),
                sub(b"ALED", b""),
                sub(b"ALLS", &2u32.to_le_bytes()),
                sub(b"ALID", b"Place\0"),
                sub(b"ALED", b""),
            ]),
        )
    }

    fn on_alias(name: &str, alias: AliasTarget, properties: Vec<(String, AttachValue)>) -> Attach {
        Attach {
            name: name.into(),
            properties,
            stages: Vec::new(),
            alias: Some(alias),
        }
    }

    #[test]
    fn quest_aliases_lists_each_kind_once() {
        let r = quest_with_aliases();
        let got: Vec<(u32, &str, Option<String>)> = quest_aliases(r.subrecords().unwrap())
            .into_iter()
            .map(|a| (a.id, a.kind, a.name))
            .collect();
        assert_eq!(
            got,
            [
                (0, "reference", Some("Player".to_string())),
                (1, "collection", Some("Crowd".to_string())),
                (2, "location", Some("Place".to_string())),
            ]
        );
    }

    #[test]
    fn attach_writes_an_alias_script_in_the_official_layout() {
        let limits = DecodeLimits::default();
        let map = |_: &ObjectTarget| Ok(0x0100_0801);
        let mut r = quest_with_aliases();
        let a = on_alias(
            "KK:AliasScript",
            AliasTarget::Name("player".into()),
            vec![("Count".into(), AttachValue::Int(3))],
        );
        let done = attach(&mut r, &[a], &map, &limits).unwrap();
        assert_eq!((done[0].outcome, done[0].alias), ("alias-added", Some(0)));
        let subs = r.subrecords().unwrap();
        // VMAD after EDID (QUST member order).
        assert_eq!(subs[1].signature(), Signature(*b"VMAD"));
        let mut want: Vec<u8> = vec![6, 0, 2, 0, 0, 0]; // version 6, format 2, no scripts
        want.extend([3, 0, 0, 0, 0]); // bind data 3, no fragments, empty fragment script
        want.extend([1, 0]); // one alias entry
        want.extend([0, 0, 0, 0, 0x02, 0x08, 0x00, 0x01]); // unused, alias 0, the quest
        want.extend([6, 0, 2, 0, 1, 0]); // version 6, format 2, one script
        want.extend([14, 0]);
        want.extend(b"KK:AliasScript");
        want.extend([0, 1, 0, 5, 0]); // flags 0, one property, a name of 5 bytes
        want.extend(b"Count");
        want.extend([3, 1, 3, 0, 0, 0]); // Int, edited, 3
        assert_eq!(subs[1].data(), want.as_slice());

        // Another script on the same alias joins its entry; one on the collection (by ID)
        // and on the location alias start entries of their own, in the order given; the
        // first script again (any case) is updated in place.
        let more = [
            on_alias("KK:Second", AliasTarget::Id(0), Vec::new()),
            on_alias("KK:Crowd", AliasTarget::Id(1), Vec::new()),
            on_alias(
                "KK:Place",
                AliasTarget::Name("PLACE".into()),
                vec![(
                    "Target".into(),
                    AttachValue::Object(ObjectTarget::Local("M".into())),
                )],
            ),
            on_alias(
                "kk:aliasscript",
                AliasTarget::Id(0),
                vec![("Count".into(), AttachValue::Int(4))],
            ),
        ];
        let done = attach(&mut r, &more, &map, &limits).unwrap();
        let outcomes: Vec<(&str, Option<u32>)> =
            done.iter().map(|d| (d.outcome, d.alias)).collect();
        assert_eq!(
            outcomes,
            [
                ("alias-added", Some(0)),
                ("alias-added", Some(1)),
                ("alias-added", Some(2)),
                ("alias-updated", Some(0)),
            ]
        );
        let v = decode(r.subrecords().unwrap()[1].data()).unwrap();
        assert!(v.scripts.is_empty());
        let q = decode_quest_fragments(&v.rest, 2).unwrap();
        assert!(q.script.is_none() && q.fragments.is_empty());
        let entries: Vec<(i16, Vec<String>)> = q
            .aliases
            .iter()
            .map(|e| {
                (
                    e.object.alias,
                    e.scripts.iter().map(Script::name_text).collect(),
                )
            })
            .collect();
        assert_eq!(
            entries,
            [
                (
                    0,
                    vec!["KK:AliasScript".to_string(), "KK:Second".to_string()]
                ),
                (1, vec!["KK:Crowd".to_string()]),
                (2, vec!["KK:Place".to_string()]),
            ]
        );
        assert!(q.aliases.iter().all(|e| e.object.form_id == 0x0100_0802
            && e.object.unused == 0
            && (e.version, e.object_format) == (6, 2)));
        assert_eq!(q.aliases[0].scripts[0].properties[0].value, Value::Int(4));
        assert_eq!(
            q.aliases[2].scripts[0].properties[0].value,
            Value::Object(Object {
                form_id: 0x0100_0801,
                alias: -1,
                unused: 0
            })
        );

        // With a quest script and a fragment script too, each lands where it belongs.
        let rest = [
            Attach {
                name: "KK:Main".into(),
                properties: Vec::new(),
                stages: Vec::new(),
                alias: None,
            },
            Attach {
                name: "Fragments:Quests:QF_Q".into(),
                properties: Vec::new(),
                stages: vec![stage(10, 0, "A")],
                alias: None,
            },
        ];
        attach(&mut r, &rest, &map, &limits).unwrap();
        let v = decode(r.subrecords().unwrap()[1].data()).unwrap();
        assert_eq!(v.scripts.len(), 1);
        let q2 = decode_quest_fragments(&v.rest, 2).unwrap();
        assert_eq!(q2.fragments.len(), 1);
        assert_eq!(q2.aliases, q.aliases);
    }

    #[test]
    fn alias_scripts_are_refused_off_a_declared_alias() {
        let limits = DecodeLimits::default();
        let map = |_: &ObjectTarget| Ok(0);
        let one = |t: AliasTarget| [on_alias("KK:A", t, Vec::new())];
        let mut r = quest_with_aliases();
        let before = r.subrecords().unwrap().len();
        for (target, why) in [
            (
                AliasTarget::Id(7),
                "quest.alias-missing: the quest has no alias 7",
            ),
            (AliasTarget::Name("Nobody".into()), "no alias `Nobody`"),
        ] {
            let e = attach(&mut r, &one(target), &map, &limits).unwrap_err();
            assert!(e.contains(why), "{e}");
            assert!(
                e.contains("0 `Player` (reference), 1 `Crowd` (collection), 2 `Place` (location)"),
                "{e}"
            );
            assert_eq!(r.subrecords().unwrap().len(), before, "nothing written");
        }
        // A quest without aliases says so.
        let mut bare = record(b"QUST");
        let e = attach(&mut bare, &one(AliasTarget::Id(0)), &map, &limits).unwrap_err();
        assert!(e.contains("(its aliases: none)"), "{e}");
        // Not on a quest; not with stages.
        let mut acti = record(b"ACTI");
        let e = attach(&mut acti, &one(AliasTarget::Id(0)), &map, &limits).unwrap_err();
        assert!(e.contains("alias scripts attach only to a QUST"), "{e}");
        let mut both = on_alias("KK:A", AliasTarget::Id(0), Vec::new());
        both.stages = vec![stage(10, 0, "F")];
        let e = attach(&mut r, &[both], &map, &limits).unwrap_err();
        assert!(e.contains("takes no stages"), "{e}");
        // An alias ID beyond the 16-bit alias of an object value.
        let mut big = quest_with_aliases();
        if let Some(subs) = big.subrecords_mut() {
            subs[2] = Subrecord::new(Signature(*b"ALST"), 40_000u32.to_le_bytes().to_vec());
        }
        let e = attach(&mut big, &one(AliasTarget::Id(40_000)), &map, &limits).unwrap_err();
        assert!(e.contains("does not fit the 16-bit alias"), "{e}");
    }
}
