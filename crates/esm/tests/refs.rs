//! M4.A5 reference composition on the official ESMs.
//!
//! - `corpus_reference_shape_*`: in every `REFR` and `ACHR`, `NAME` occurs once with 4
//!   bytes, `DATA` once with 24 bytes unless the record is deleted (then not at all),
//!   `EDID` is first whenever present, and `DATA` is last except where listed; in
//!   `Starfield.esm` (no masters) every `NAME` is the FormID of one of the file's records
//!   that is not itself a placed reference, cell, world or navmesh, except IDs below 0x800
//!   that name no record (one: `REFR` `00014893` -> `000001FB`).
//! - `corpus_compose_identical_*`: a seeded sample of 1,000 references per file (all of
//!   them in the small files), each decomposed into a `RefSpec` (base, position, rotation,
//!   flags, EditorID, the other subrecords as given, header stamps) and composed again,
//!   equals the record's bytes in the file.
//!
//! `cargo test -p kitcraft-esm --release --test refs -- --ignored --nocapture`

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

use common::hostile::Rng;
use kitcraft_esm::index::{index_file, read_indexed, IndexOptions, RecordLoc};
use kitcraft_esm::refs::{compose_reference, RecordStamps, RefSpec, FLAG_INITIALLY_DISABLED};
use kitcraft_esm::{DecodeLimits, Entry, Record, Signature, WriteOptions};

const ALLOW: &[&str] = &[
    "DATA.Position.Z",
    "DATA.Rotation",
    "flag.initially_disabled",
    "flag.raw",
];

fn is_ref(loc: &RecordLoc) -> bool {
    loc.signature.0 == *b"REFR" || loc.signature.0 == *b"ACHR"
}

fn shape(names: &[&str]) {
    let Some(files) = common::files("refs", names) else {
        return;
    };
    let limits = DecodeLimits::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut data_not_last: Vec<String> = Vec::new();
    for file in &files {
        let index = index_file(&file.path, &IndexOptions::default()).unwrap();
        let types: HashMap<u32, Signature> = index
            .records
            .iter()
            .map(|r| (r.form_id, r.signature))
            .collect();
        let check_bases = file.name == "Starfield.esm";
        let mut input = BufReader::new(File::open(&file.path).unwrap());
        for loc in index.records.iter().filter(|r| is_ref(r)) {
            let rec = read_indexed(&mut input, loc, Some(&limits)).unwrap();
            let subs = rec.subrecords().unwrap();
            let at = |sig: &[u8; 4]| -> Vec<usize> {
                subs.iter()
                    .enumerate()
                    .filter(|(_, s)| s.signature().0 == *sig)
                    .map(|(i, _)| i)
                    .collect()
            };
            let what = || format!("{} {} {:08X}", file.name, loc.signature, loc.form_id);
            let names = at(b"NAME");
            assert_eq!(names.len(), 1, "{}", what());
            assert_eq!(subs[names[0]].data().len(), 4, "{}", what());
            let data = at(b"DATA");
            let deleted = loc.flags & 0x20 != 0;
            assert_eq!(data.len(), usize::from(!deleted), "{}", what());
            if let Some(&d) = data.first() {
                assert_eq!(subs[d].data().len(), 24, "{}", what());
                assert!(d > names[0], "{}", what());
                if d + 1 != subs.len() {
                    data_not_last.push(what());
                }
            }
            for e in at(b"EDID") {
                assert_eq!(e, 0, "{}", what());
            }
            *counts
                .entry(format!("{} checked", loc.signature))
                .or_default() += 1;
            if check_bases {
                let base = u32::from_le_bytes(subs[names[0]].data().try_into().unwrap());
                let Some(ty) = types.get(&base) else {
                    // Not a record of the file: list the IDs (engine-defined forms below
                    // 0x800, if that is what they are, are not in any plugin).
                    *counts
                        .entry(format!("Starfield.esm NAME -> no record: {base:08X}"))
                        .or_default() += 1;
                    assert!(base < 0x800, "{}: base {base:08X}", what());
                    continue;
                };
                assert!(
                    !matches!(
                        &ty.0,
                        b"REFR" | b"ACHR" | b"CELL" | b"WRLD" | b"NAVM" | b"PGRE" | b"PHZD"
                    ),
                    "{}: base {base:08X} is a {ty}",
                    what()
                );
                *counts
                    .entry(format!("Starfield.esm NAME -> {ty}"))
                    .or_default() += 1;
            }
        }
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    for (i, (k, c)) in v.iter().enumerate() {
        if i < 15 || k.contains("no record") {
            println!("{c:>10}  {k}");
        }
    }
    println!("DATA not last: {} {data_not_last:?}", data_not_last.len());
    assert!(data_not_last.len() <= 3);
}

/// Split a corpus reference into the spec that composes it.
fn decompose(record: &Record<'_>) -> RefSpec {
    let subs = record.subrecords().unwrap();
    let pos = |sig: &[u8; 4]| subs.iter().position(|s| s.signature().0 == *sig).unwrap();
    let name = pos(b"NAME");
    let data = pos(b"DATA");
    let has_edid = subs.first().is_some_and(|s| s.signature().0 == *b"EDID");
    let start = usize::from(has_edid);
    let floats: Vec<f32> = subs[data]
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect();
    let h = record.header;
    let mut spec = RefSpec::new(
        h.signature,
        h.form_id,
        u32::from_le_bytes(subs[name].data().try_into().unwrap()),
        [floats[0], floats[1], floats[2]],
        [floats[3], floats[4], floats[5]],
    );
    spec.persistent = h.flags & 0x400 != 0;
    spec.initially_disabled = h.flags & FLAG_INITIALLY_DISABLED != 0;
    spec.raw_flags = h.flags & !(0x400 | FLAG_INITIALLY_DISABLED);
    spec.editor_id = has_edid.then(|| {
        let d = subs[0].data();
        d.strip_suffix(&[0]).unwrap_or(d).to_vec()
    });
    let own = |r: std::ops::Range<usize>| -> Vec<_> {
        subs[r].iter().map(|s| s.clone().into_owned()).collect()
    };
    spec.leading = own(start..name);
    spec.extra = own(name + 1..data);
    spec.trailing = own(data + 1..subs.len());
    spec.stamps = RecordStamps {
        form_version: h.form_version,
        timestamp: h.timestamp,
        version_control: h.version_control,
        unknown: h.unknown,
    };
    spec
}

fn compose_sample(names: &[&str], per_file: usize) {
    let Some(files) = common::files("refs", names) else {
        return;
    };
    for file in &files {
        let index = index_file(&file.path, &IndexOptions::default()).unwrap();
        let refs: Vec<&RecordLoc> = index
            .records
            .iter()
            .filter(|r| is_ref(r) && r.flags & 0x20 == 0)
            .collect();
        let mut picked: Vec<&RecordLoc> = if refs.len() <= per_file {
            refs.clone()
        } else {
            let mut rng = Rng::new(0x5EED_A5A5);
            let mut set = std::collections::BTreeSet::new();
            while set.len() < per_file {
                set.insert((rng.next() % refs.len() as u64) as usize);
            }
            set.into_iter().map(|i| refs[i]).collect()
        };
        picked.sort_by_key(|r| r.offset);
        let mut input = BufReader::new(File::open(&file.path).unwrap());
        for loc in &picked {
            let record = read_indexed(&mut input, loc, Some(&DecodeLimits::default())).unwrap();
            let spec = decompose(&record);
            let composed = compose_reference(&spec, ALLOW).unwrap();
            let mut got = Vec::new();
            Entry::Record(composed.record)
                .write_to(&mut got, 0, &WriteOptions::default())
                .unwrap();
            let mut want = vec![0u8; usize::try_from(loc.len()).unwrap()];
            input.seek(SeekFrom::Start(loc.offset)).unwrap();
            input.read_exact(&mut want).unwrap();
            assert_eq!(
                got, want,
                "{} {} {:08X}",
                file.name, loc.signature, loc.form_id
            );
        }
        println!(
            "{}: {} references composed identical",
            file.name,
            picked.len()
        );
    }
}

#[test]
fn corpus_reference_shape_small() {
    shape(&common::SMALL);
}

#[test]
#[ignore]
fn corpus_reference_shape_full() {
    let names: Vec<String> = common::manifest().into_iter().map(|e| e.name).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    shape(&names);
}

#[test]
fn corpus_compose_identical_small() {
    compose_sample(&common::SMALL, usize::MAX);
}

#[test]
#[ignore]
fn corpus_compose_identical_full() {
    let names: Vec<String> = common::manifest().into_iter().map(|e| e.name).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    compose_sample(&names, 1_000);
}
