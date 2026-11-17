//! The quest fragment block of `VMAD` (`kitkraft_doc::vmad::QuestFragments`) against the
//! official files:
//!
//! 1. every `QUST` `VMAD` of the 14 official files that has the block decodes to it and
//!    re-encodes byte-identically, no byte left over; and the shapes `attach` relies on
//!    hold: no block is empty (a quest with scripts only has none), a fragment script comes
//!    with stage fragments and never sits in the script list, every fragment names that
//!    script and has the unknown byte 1 (their order is not the stage order in 886 blocks,
//!    so `attach` keeps the order given);
//! 2. `attach` (the path `kit build`, `new` and `set` use) rebuilds four official quest
//!    `VMAD`s byte for byte from their scripts, property values, stages and alias scripts:
//!    one with scripts and no block, one with a quest script, a fragment script and its
//!    stage fragments, one holding alias scripts only, and one with a quest script, a
//!    fragment script and alias scripts (each the one with the most properties among those
//!    `attach` can write: scalars, objects, alias values of the quest itself or another
//!    quest, structs of these, and arrays of all of them; all on a copy of the quest's own subrecords, so `attach`
//!    finds each alias the way a build does);
//! 3. the alias entries `attach` writes by: every official entry holds the quest's own
//!    FormID, 0 unused bytes, version 6 and object format 2, and at least one script; which
//!    alias kinds carry scripts; how many entries name an alias the quest does not declare
//!    (`attach` refuses those, `quest.alias-missing`).
//!
//! `#[ignore]`: reads 2.5 GB. Run
//! `cargo test -p kitkraft-engine --release --test quest_vmad_corpus -- --ignored --nocapture`
//! with `KITKRAFT_DATA_DIR` set.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitkraft_engine::edit::vmad::{self, AliasTarget, AttachValue, ObjectTarget, StageFragment};
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::{
    Body, DecodeLimits, Entry, ParseOptions, Record, RecordHeader, Signature, Subrecord,
};

const CORPUS: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS003.esm",
    "SFBGS004.esm",
    "SFBGS006.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS00D.esm",
    "SFBGS047.esm",
    "SFBGS050.esm",
    "ShatteredSpace.esm",
];

/// An official quest `VMAD` that `attach` can write, and its property count.
struct Sample {
    at: String,
    vmad: Vec<u8>,
    properties: usize,
    /// The quest's FormID and its other subrecords (`attach` reads the aliases the record
    /// declares).
    form_id: u32,
    others: Vec<([u8; 4], Vec<u8>)>,
}

#[derive(Default)]
struct Stats {
    quests: u64,
    no_block: u64,
    bind_versions: BTreeMap<u8, u64>,
    /// (has scripts, has fragment script, has fragments, has aliases) of each block ->
    /// count.
    shapes: BTreeMap<(bool, bool, bool, bool), u64>,
    fragments: u64,
    alias_entries: u64,
    /// The unknown byte of each fragment -> count.
    unknown_bytes: BTreeMap<u8, u64>,
    /// Fragment scripts also in the script list; fragments naming another script; blocks
    /// whose fragments are not in stage and index order.
    listed: u64,
    foreign: u64,
    unsorted: u64,
    /// Scripts only, no block.
    scripts_only: Option<Sample>,
    /// A quest script, a fragment script and fragments, no aliases.
    with_fragments: Option<Sample>,
    /// Alias scripts only: no quest script, no fragment script.
    aliases_only: Option<Sample>,
    /// A quest script, a fragment script with fragments, and alias scripts.
    with_aliases: Option<Sample>,
    /// Alias entries by (holds the quest's FormID, unused, version, object format, has
    /// scripts) -> count.
    entry_shapes: BTreeMap<(bool, u16, i16, i16, bool), u64>,
    /// Alias entries by the kind of alias they name (`reference`, `location`,
    /// `collection`, `undeclared`) -> count, and where the undeclared ones are.
    entry_kinds: BTreeMap<&'static str, u64>,
    undeclared: Vec<String>,
    /// Blocks whose alias entries are not in alias ID order.
    entries_unsorted: u64,
    /// Every quest VMAD `attach` can write whole that holds a struct or struct array
    /// property: each is rebuilt.
    with_structs: Vec<Sample>,
    failures: Vec<String>,
}

/// Whether `attach` writes exactly this script entry of a script on `quest` (its FormID)
/// with the aliases `declared`: flags 0 and every property, with flags 1, a scalar, an
/// object (unused 0; no alias, an alias the quest declares, or an alias of another quest)
/// or an array of these.
fn attachable(s: &vmad::Script, quest: u32, declared: &[vmad::QuestAlias]) -> bool {
    let object = |o: &vmad::Object| {
        o.unused == 0
            && (o.alias == -1
                || (o.alias >= 0
                    && (o.form_id != quest
                        || declared
                            .iter()
                            .any(|d| i64::from(d.id) == i64::from(o.alias)))))
    };
    // A struct member: flags 1 and a scalar or an object.
    let member = |p: &vmad::Property| {
        p.flags == 1
            && match &p.value {
                vmad::Value::Object(o) => object(o),
                vmad::Value::String(s) => std::str::from_utf8(s).is_ok(),
                vmad::Value::Int(_) | vmad::Value::Float(_) | vmad::Value::Bool(0 | 1) => true,
                _ => false,
            }
    };
    s.flags == 0
        && s.properties.iter().all(|p| {
            p.flags == 1
                && match &p.value {
                    vmad::Value::Struct(m) => m.iter().all(member),
                    vmad::Value::Structs(v) => v.iter().flatten().all(member),
                    vmad::Value::Object(o) => object(o),
                    vmad::Value::Objects(v) => v.iter().all(object),
                    vmad::Value::String(s) => std::str::from_utf8(s).is_ok(),
                    vmad::Value::Strings(v) => v.iter().all(|s| std::str::from_utf8(s).is_ok()),
                    vmad::Value::Int(_) | vmad::Value::Float(_) | vmad::Value::Bool(0 | 1) => true,
                    vmad::Value::Ints(_) | vmad::Value::Floats(_) => true,
                    vmad::Value::Bools(v) => v.iter().all(|b| *b <= 1),
                    _ => false,
                }
        })
}

/// Keeps `candidate` in `slot` when it has more properties.
fn keep(slot: &mut Option<Sample>, candidate: Sample) {
    if slot
        .as_ref()
        .is_none_or(|old| old.properties < candidate.properties)
    {
        *slot = Some(candidate);
    }
}

fn visit(record: &Record<'_>, file: &str, stats: &mut Stats) {
    if record.header.signature != Signature(*b"QUST") {
        return;
    }
    let decoded = record.decode(&DecodeLimits::default()).unwrap();
    let subs = match (&decoded, record.subrecords()) {
        (Some(d), _) => d.as_slice(),
        (None, Some(s)) => s,
        (None, None) => return,
    };
    let declared = vmad::quest_aliases(subs);
    let attachable = |s: &vmad::Script| attachable(s, record.header.form_id, &declared);
    let sample = |at: &str, vmad: &[u8], properties: usize| Sample {
        at: at.to_string(),
        vmad: vmad.to_vec(),
        properties,
        form_id: record.header.form_id,
        others: subs
            .iter()
            .filter(|s| s.signature() != Signature(*b"VMAD"))
            .map(|s| (s.signature().0, s.data().to_vec()))
            .collect(),
    };
    for s in subs.iter().filter(|s| s.signature() == Signature(*b"VMAD")) {
        stats.quests += 1;
        let at = format!("{file} QUST {:08X}", record.header.form_id);
        let v = match vmad::decode(s.data()) {
            Ok(v) => v,
            Err(e) => {
                stats.failures.push(format!("{at}: {e}"));
                continue;
            }
        };
        let main_props: usize = v.scripts.iter().map(|s| s.properties.len()).sum();
        let plain = v.version == 6 && v.object_format == 2 && v.scripts.iter().all(attachable);
        let has_struct = |sc: &vmad::Script| {
            sc.properties
                .iter()
                .any(|p| matches!(p.value, vmad::Value::Struct(_) | vmad::Value::Structs(_)))
        };
        if v.rest.is_empty() && plain && v.scripts.iter().any(has_struct) {
            stats.with_structs.push(sample(&at, s.data(), main_props));
        }
        if v.rest.is_empty() {
            stats.no_block += 1;
            if plain && !v.scripts.is_empty() {
                keep(&mut stats.scripts_only, sample(&at, s.data(), main_props));
            }
            continue;
        }
        let q = match vmad::decode_quest_fragments(&v.rest, v.object_format) {
            Ok(q) => q,
            Err(e) => {
                stats.failures.push(format!("{at}: {e}"));
                continue;
            }
        };
        match vmad::encode_quest_fragments(&q, v.object_format) {
            Ok(b) if b == v.rest => {}
            Ok(_) => stats.failures.push(format!("{at}: re-encodes differently")),
            Err(e) => stats.failures.push(format!("{at}: {e}")),
        }
        *stats.bind_versions.entry(q.bind_version).or_default() += 1;
        *stats
            .shapes
            .entry((
                !v.scripts.is_empty(),
                q.script.is_some(),
                !q.fragments.is_empty(),
                !q.aliases.is_empty(),
            ))
            .or_default() += 1;
        stats.fragments += q.fragments.len() as u64;
        stats.alias_entries += q.aliases.len() as u64;
        if let Some(fs) = &q.script {
            stats.listed += u64::from(v.scripts.iter().any(|x| x.name == fs.name));
        }
        for f in &q.fragments {
            *stats.unknown_bytes.entry(f.unknown).or_default() += 1;
            if q.script.as_ref().is_none_or(|fs| fs.name != f.script_name) {
                stats.foreign += 1;
            }
        }
        if !q
            .fragments
            .windows(2)
            .all(|w| (w[0].stage, w[0].stage_index) < (w[1].stage, w[1].stage_index))
        {
            stats.unsorted += 1;
        }
        if let Some(fs) = &q.script {
            let names_ok = q.fragments.iter().all(|f| {
                f.unknown == 1
                    && f.script_name == fs.name
                    && std::str::from_utf8(&f.fragment_name).is_ok()
            });
            if plain && !v.scripts.is_empty() && q.aliases.is_empty() && attachable(fs) && names_ok
            {
                keep(
                    &mut stats.with_fragments,
                    sample(&at, s.data(), main_props + fs.properties.len()),
                );
            }
        }
        let is_declared = |alias: i16| {
            declared
                .iter()
                .find(|d| i64::from(d.id) == i64::from(alias))
        };
        for e in &q.aliases {
            *stats
                .entry_shapes
                .entry((
                    e.object.form_id == record.header.form_id,
                    e.object.unused,
                    e.version,
                    e.object_format,
                    !e.scripts.is_empty(),
                ))
                .or_default() += 1;
            let kind = is_declared(e.object.alias).map_or("undeclared", |d| d.kind);
            *stats.entry_kinds.entry(kind).or_default() += 1;
            if kind == "undeclared" {
                stats
                    .undeclared
                    .push(format!("{at} alias {}", e.object.alias));
            }
        }
        if !q
            .aliases
            .windows(2)
            .all(|w| w[0].object.alias < w[1].object.alias)
        {
            stats.entries_unsorted += 1;
        }
        // Alias samples: every alias script attachable, every entry in the shape attach
        // writes and naming a declared alias, no alias named twice.
        let alias_props: usize = q
            .aliases
            .iter()
            .flat_map(|e| &e.scripts)
            .map(|s| s.properties.len())
            .sum();
        let entries_ok = !q.aliases.is_empty()
            && q.aliases.iter().enumerate().all(|(i, e)| {
                e.object.form_id == record.header.form_id
                    && e.object.unused == 0
                    && (e.version, e.object_format) == (6, 2)
                    && is_declared(e.object.alias).is_some()
                    && e.scripts.iter().all(attachable)
                    && q.aliases
                        .iter()
                        .skip(i + 1)
                        .all(|f| f.object.alias != e.object.alias)
            });
        if plain && entries_ok {
            match &q.script {
                None if v.scripts.is_empty() => {
                    keep(&mut stats.aliases_only, sample(&at, s.data(), alias_props));
                }
                Some(fs)
                    if !v.scripts.is_empty()
                        && attachable(fs)
                        && q.fragments.iter().all(|f| {
                            f.unknown == 1
                                && f.script_name == fs.name
                                && std::str::from_utf8(&f.fragment_name).is_ok()
                        }) =>
                {
                    keep(
                        &mut stats.with_aliases,
                        sample(
                            &at,
                            s.data(),
                            main_props + fs.properties.len() + alias_props,
                        ),
                    );
                }
                _ => {}
            }
        }
        // Whole-VMAD struct samples: the fragment script (if any) and every alias entry in
        // the shapes attach writes.
        let fragments_ok = q.script.as_ref().is_none_or(|fs| {
            attachable(fs)
                && q.fragments.iter().all(|f| {
                    f.unknown == 1
                        && f.script_name == fs.name
                        && std::str::from_utf8(&f.fragment_name).is_ok()
                })
        });
        let structs = v.scripts.iter().any(has_struct)
            || q.script.iter().any(has_struct)
            || q.aliases.iter().flat_map(|e| &e.scripts).any(has_struct);
        if plain && fragments_ok && (q.aliases.is_empty() || entries_ok) && structs {
            stats
                .with_structs
                .push(sample(&at, s.data(), main_props + alias_props));
        }
    }
}

fn walk(entry: &Entry<'_>, file: &str, stats: &mut Stats) {
    match entry {
        Entry::Group(g) => {
            for child in g.children() {
                walk(child, file, stats);
            }
        }
        Entry::Record(r) => visit(r, file, stats),
    }
}

/// The attach request that writes `script`'s entry: object values become same-call
/// targets named by their FormID, which the mapping turns back into it.
fn attach_request(
    script: &vmad::Script,
    stages: Vec<StageFragment>,
    alias: Option<AliasTarget>,
    quest: u32,
) -> vmad::Attach {
    // An alias of the quest itself by ID; another quest's alias with that quest as a
    // same-call target.
    let object = |o: &vmad::Object| {
        AttachValue::Object(match (o.alias, o.form_id) {
            (-1, 0) => ObjectTarget::Null,
            (-1, f) => ObjectTarget::Local(format!("{f:08X}")),
            (a, f) => ObjectTarget::Alias {
                quest: (f != quest).then(|| Box::new(ObjectTarget::Local(format!("{f:08X}")))),
                alias: AliasTarget::Id(u32::try_from(a).unwrap()),
            },
        })
    };
    let string = |s: &Vec<u8>| AttachValue::String(String::from_utf8(s.clone()).unwrap());
    let scalar = |v: &vmad::Value| match v {
        vmad::Value::Object(o) => object(o),
        vmad::Value::String(s) => string(s),
        vmad::Value::Int(i) => AttachValue::Int(*i),
        vmad::Value::Float(f) => AttachValue::Float(f32::from_bits(*f)),
        vmad::Value::Bool(b) => AttachValue::Bool(*b == 1),
        other => panic!("not a struct member: {other:?}"),
    };
    let members = |m: &Vec<vmad::Property>| {
        AttachValue::Struct(
            m.iter()
                .map(|p| (String::from_utf8(p.name.clone()).unwrap(), scalar(&p.value)))
                .collect(),
        )
    };
    vmad::Attach {
        name: script.name_text(),
        properties: script
            .properties
            .iter()
            .map(|p| {
                let v = match &p.value {
                    vmad::Value::Object(o) => object(o),
                    vmad::Value::String(s) => string(s),
                    vmad::Value::Int(i) => AttachValue::Int(*i),
                    vmad::Value::Float(f) => AttachValue::Float(f32::from_bits(*f)),
                    vmad::Value::Bool(b) => AttachValue::Bool(*b == 1),
                    vmad::Value::Objects(v) => {
                        AttachValue::Array(1, v.iter().map(object).collect())
                    }
                    vmad::Value::Strings(v) => {
                        AttachValue::Array(2, v.iter().map(string).collect())
                    }
                    vmad::Value::Ints(v) => {
                        AttachValue::Array(3, v.iter().map(|i| AttachValue::Int(*i)).collect())
                    }
                    vmad::Value::Floats(v) => AttachValue::Array(
                        4,
                        v.iter()
                            .map(|f| AttachValue::Float(f32::from_bits(*f)))
                            .collect(),
                    ),
                    vmad::Value::Bools(v) => AttachValue::Array(
                        5,
                        v.iter().map(|b| AttachValue::Bool(*b == 1)).collect(),
                    ),
                    vmad::Value::Struct(m) => members(m),
                    vmad::Value::Structs(v) => {
                        AttachValue::Array(7, v.iter().map(members).collect())
                    }
                    other => panic!("not attachable: {other:?}"),
                };
                (String::from_utf8(p.name.clone()).unwrap(), v)
            })
            .collect(),
        stages,
        alias,
    }
}

/// `attach` on a fresh quest with `sample`'s scripts, fragment script with its stages and
/// alias scripts (the quest holding a copy of the official record's other subrecords, so
/// its aliases are declared): the `VMAD` it writes.
fn rebuild(sample: &Sample) -> Vec<u8> {
    let built = attach_all(sample);
    let official = vmad::decode(&sample.vmad).unwrap();
    let block = vmad::decode_quest_fragments(&official.rest, 2).ok();
    println!(
        "{}: {} script(s), {} fragment(s), {} alias entries ({} scripts), {} properties, {} bytes; attach wrote {} bytes, identical: {}",
        sample.at,
        official.scripts.len(),
        block.as_ref().map_or(0, |q| q.fragments.len()),
        block.as_ref().map_or(0, |q| q.aliases.len()),
        block
            .as_ref()
            .map_or(0, |q| q.aliases.iter().map(|e| e.scripts.len()).sum::<usize>()),
        sample.properties,
        sample.vmad.len(),
        built.len(),
        built == sample.vmad
    );
    built
}

/// [`rebuild`] without the line.
fn attach_all(sample: &Sample) -> Vec<u8> {
    let official = vmad::decode(&sample.vmad).unwrap();
    let mut requests: Vec<vmad::Attach> = official
        .scripts
        .iter()
        .map(|s| attach_request(s, Vec::new(), None, sample.form_id))
        .collect();
    if !official.rest.is_empty() {
        let q = vmad::decode_quest_fragments(&official.rest, 2).unwrap();
        let stages = q
            .fragments
            .iter()
            .map(|f| StageFragment {
                stage: f.stage,
                index: f.stage_index,
                function: String::from_utf8(f.fragment_name.clone()).unwrap(),
            })
            .collect();
        if let Some(fs) = &q.script {
            requests.push(attach_request(fs, stages, None, sample.form_id));
        }
        for e in &q.aliases {
            for s in &e.scripts {
                requests.push(attach_request(
                    s,
                    Vec::new(),
                    Some(AliasTarget::Id(u32::try_from(e.object.alias).unwrap())),
                    sample.form_id,
                ));
            }
        }
    }
    let mut record = Record::new(
        RecordHeader {
            signature: Signature(*b"QUST"),
            form_id: sample.form_id,
            form_version: 582,
            ..RecordHeader::default()
        },
        Body::Raw(
            sample
                .others
                .iter()
                .map(|(sig, data)| Subrecord::new(Signature(*sig), data.clone()))
                .collect(),
        ),
    );
    let map = |t: &ObjectTarget| match t {
        ObjectTarget::Null => Ok(0),
        ObjectTarget::Local(id) => u32::from_str_radix(id, 16).map_err(|e| e.to_string()),
        ObjectTarget::Form(_) | ObjectTarget::Alias { .. } => Err("unused".to_string()),
    };
    vmad::attach(&mut record, &requests, &map, &DecodeLimits::default()).unwrap();
    let built = record
        .subrecords()
        .unwrap()
        .iter()
        .find(|s| s.signature() == Signature(*b"VMAD"))
        .unwrap()
        .data()
        .to_vec();
    built
}

#[test]
#[ignore = "reads 2.5 GB of plugins; run in release"]
fn every_official_quest_fragment_block_roundtrips_and_attach_rebuilds_four() {
    let Some(dir) = std::env::var_os("KITKRAFT_DATA_DIR")
        .or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
    else {
        println!("skipped: KITKRAFT_DATA_DIR is unset or not a directory");
        return;
    };
    let mut stats = Stats::default();
    for name in CORPUS {
        let path = dir.join(name);
        visit_file(&path, &ParseOptions::default(), |entry| {
            walk(entry, name, &mut stats);
            Ok(())
        })
        .unwrap();
    }
    println!(
        "QUST VMADs {}: without a fragment block {}; blocks by (scripts, fragment script, \
         fragments, aliases) {:?}; bind data versions {:?}; stage fragments {} (unknown \
         bytes {:?}); alias entries {}; fragment scripts in the list {}, fragments naming \
         another script {}, blocks out of stage order {}",
        stats.quests,
        stats.no_block,
        stats.shapes,
        stats.bind_versions,
        stats.fragments,
        stats.unknown_bytes,
        stats.alias_entries,
        stats.listed,
        stats.foreign,
        stats.unsorted
    );
    for f in stats.failures.iter().take(20) {
        println!("  {f}");
    }
    assert!(
        stats.failures.is_empty(),
        "{} quest fragment blocks do not roundtrip",
        stats.failures.len()
    );
    assert!(stats.quests > 0);
    // The shapes `attach` writes by: no empty block; a fragment script always with
    // fragments and the reverse; never in the list; fragments name it, unknown byte 1;
    // bind data version 3.
    for (scripts, frag_script, frags, aliases) in stats.shapes.keys() {
        assert!(*frag_script || *aliases, "an empty block");
        assert_eq!(
            frag_script, frags,
            "a fragment script without fragments or the reverse"
        );
        let _ = scripts;
    }
    assert_eq!((stats.listed, stats.foreign), (0, 0));
    assert_eq!(stats.unknown_bytes.keys().copied().collect::<Vec<_>>(), [1]);
    assert_eq!(stats.bind_versions.keys().copied().collect::<Vec<_>>(), [3]);

    let only = stats
        .scripts_only
        .expect("an official quest with scripts and no block");
    assert_eq!(rebuild(&only), only.vmad, "{}", only.at);
    let frag = stats
        .with_fragments
        .expect("an official quest with a quest script and stage fragments");
    assert_eq!(rebuild(&frag), frag.vmad, "{}", frag.at);

    println!(
        "alias entries by (quest's FormID, unused, version, object format, has scripts) {:?}; \
         by alias kind {:?}; blocks whose entries are not in ID order {}; undeclared: {:?}",
        stats.entry_shapes, stats.entry_kinds, stats.entries_unsorted, stats.undeclared
    );
    // The one entry shape attach writes.
    assert_eq!(
        stats.entry_shapes.keys().copied().collect::<Vec<_>>(),
        [(true, 0, 6, 2, true)]
    );
    let only = stats
        .aliases_only
        .expect("an official quest holding alias scripts only");
    assert_eq!(rebuild(&only), only.vmad, "{}", only.at);
    let all = stats
        .with_aliases
        .expect("an official quest with a quest script, fragments and alias scripts");
    assert_eq!(rebuild(&all), all.vmad, "{}", all.at);

    // Struct and struct-array properties: every quest VMAD attach can write that holds one.
    let structs: usize = stats
        .with_structs
        .iter()
        .map(|sm| {
            let v = vmad::decode(&sm.vmad).unwrap();
            let mut n = 0usize;
            let mut count = |sc: &vmad::Script| {
                n += sc
                    .properties
                    .iter()
                    .map(|p| match &p.value {
                        vmad::Value::Struct(_) => 1,
                        vmad::Value::Structs(v) => v.len(),
                        _ => 0,
                    })
                    .sum::<usize>();
            };
            v.scripts.iter().for_each(&mut count);
            if let Ok(q) = vmad::decode_quest_fragments(&v.rest, 2) {
                q.script.iter().for_each(&mut count);
                q.aliases
                    .iter()
                    .flat_map(|e| &e.scripts)
                    .for_each(&mut count);
            }
            n
        })
        .sum();
    let differ: Vec<&str> = stats
        .with_structs
        .iter()
        .filter(|sm| attach_all(sm) != sm.vmad)
        .map(|sm| sm.at.as_str())
        .collect();
    println!(
        "quest VMADs with struct properties rebuilt: {} ({structs} struct values), differing: {differ:?}",
        stats.with_structs.len()
    );
    assert!(!stats.with_structs.is_empty());
    assert!(differ.is_empty(), "{differ:?}");
}
