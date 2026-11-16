//! The quest fragment block of `VMAD` (`kitkraft_doc::vmad::QuestFragments`) against the
//! official files:
//!
//! 1. every `QUST` `VMAD` of the 14 official files that has the block decodes to it and
//!    re-encodes byte-identically, no byte left over; and the shapes `attach` relies on
//!    hold: no block is empty (a quest with scripts only has none), a fragment script comes
//!    with stage fragments and never sits in the script list, every fragment names that
//!    script and has the unknown byte 1 (their order is not the stage order in 886 blocks,
//!    so `attach` keeps the order given);
//! 2. `attach` (the path `kit build`, `new` and `set` use) rebuilds two official quest
//!    `VMAD`s byte for byte from their scripts, property values and stages: one with
//!    scripts and no block, one with a quest script, a fragment script and its stage
//!    fragments (each the one with the most properties among those `attach` can write).
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

use kitkraft_engine::edit::vmad::{self, AttachValue, ObjectTarget, StageFragment};
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
    failures: Vec<String>,
}

/// Whether `attach` writes exactly this script entry: flags 0 and every property a scalar
/// or an object (alias -1, unused 0) with flags 1.
fn attachable(s: &vmad::Script) -> bool {
    s.flags == 0
        && s.properties.iter().all(|p| {
            p.flags == 1
                && match &p.value {
                    vmad::Value::Object(o) => o.alias == -1 && o.unused == 0,
                    vmad::Value::String(s) => std::str::from_utf8(s).is_ok(),
                    vmad::Value::Int(_) | vmad::Value::Float(_) | vmad::Value::Bool(0 | 1) => true,
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
        if v.rest.is_empty() {
            stats.no_block += 1;
            if plain && !v.scripts.is_empty() {
                keep(
                    &mut stats.scripts_only,
                    Sample {
                        at,
                        vmad: s.data().to_vec(),
                        properties: main_props,
                    },
                );
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
                    Sample {
                        at,
                        vmad: s.data().to_vec(),
                        properties: main_props + fs.properties.len(),
                    },
                );
            }
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
fn attach_request(script: &vmad::Script, stages: Vec<StageFragment>) -> vmad::Attach {
    vmad::Attach {
        name: script.name_text(),
        properties: script
            .properties
            .iter()
            .map(|p| {
                let v = match &p.value {
                    vmad::Value::Object(o) if o.form_id == 0 => {
                        AttachValue::Object(ObjectTarget::Null)
                    }
                    vmad::Value::Object(o) => {
                        AttachValue::Object(ObjectTarget::Local(format!("{:08X}", o.form_id)))
                    }
                    vmad::Value::String(s) => {
                        AttachValue::String(String::from_utf8(s.clone()).unwrap())
                    }
                    vmad::Value::Int(i) => AttachValue::Int(*i),
                    vmad::Value::Float(f) => AttachValue::Float(f32::from_bits(*f)),
                    vmad::Value::Bool(b) => AttachValue::Bool(*b == 1),
                    other => panic!("not attachable: {other:?}"),
                };
                (String::from_utf8(p.name.clone()).unwrap(), v)
            })
            .collect(),
        stages,
    }
}

/// `attach` on a fresh quest with `sample`'s scripts (and fragment script with its
/// stages): the `VMAD` it writes.
fn rebuild(sample: &Sample) -> Vec<u8> {
    let official = vmad::decode(&sample.vmad).unwrap();
    let mut requests: Vec<vmad::Attach> = official
        .scripts
        .iter()
        .map(|s| attach_request(s, Vec::new()))
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
        requests.push(attach_request(q.script.as_ref().unwrap(), stages));
    }
    let mut record = Record::new(
        RecordHeader {
            signature: Signature(*b"QUST"),
            form_id: 0x800,
            form_version: 582,
            ..RecordHeader::default()
        },
        Body::Raw(vec![Subrecord::new(Signature(*b"EDID"), b"KK\0".to_vec())]),
    );
    let map = |t: &ObjectTarget| match t {
        ObjectTarget::Null => Ok(0),
        ObjectTarget::Local(id) => u32::from_str_radix(id, 16).map_err(|e| e.to_string()),
        ObjectTarget::Form(_) => Err("unused".to_string()),
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
    println!(
        "{}: {} script(s), {} fragment(s), {} properties, {} bytes; attach wrote {} bytes, identical: {}",
        sample.at,
        official.scripts.len(),
        vmad::decode_quest_fragments(&official.rest, 2).map_or(0, |q| q.fragments.len()),
        sample.properties,
        sample.vmad.len(),
        built.len(),
        built == sample.vmad
    );
    built
}

#[test]
#[ignore = "reads 2.5 GB of plugins; run in release"]
fn every_official_quest_fragment_block_roundtrips_and_attach_rebuilds_two() {
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
}
