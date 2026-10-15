//! M4.E9: every `VMAD` of the official files decodes and re-encodes byte-identically
//! through `kitkraft_engine::edit::vmad` (the script list of lane BQ's `ScriptAttachment`
//! block), and the script names and property shapes it reports are the ones the files
//! hold. Attaching a script re-encodes the whole payload, so this is the check that a
//! corpus copy with a script attached keeps every other byte.
//!
//! `#[ignore]`: reads 2.5 GB. Run
//! `cargo test -p kitkraft-engine --release --test vmad_corpus -- --ignored --nocapture`
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

use kitkraft_engine::edit::vmad;
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature};

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

#[derive(Default)]
struct Stats {
    /// VMADs per record type.
    per_type: BTreeMap<String, u64>,
    /// (version, object format) -> count.
    formats: BTreeMap<(i16, i16), u64>,
    /// Property type byte -> count.
    types: BTreeMap<u8, u64>,
    scripts: u64,
    with_rest: u64,
    /// Object values whose unused bytes are not zero.
    unused_nonzero: u64,
    objects: u64,
    failures: Vec<String>,
}

fn count_types(p: &vmad::Property, stats: &mut Stats) {
    *stats.types.entry(p.ty).or_default() += 1;
    match &p.value {
        vmad::Value::Struct(m) => m.iter().for_each(|q| count_types(q, stats)),
        vmad::Value::Structs(v) => v.iter().flatten().for_each(|q| count_types(q, stats)),
        _ => {}
    }
}

fn visit(record: &Record<'_>, file: &str, stats: &mut Stats) {
    if record.header.signature == Signature::TES4 {
        return;
    }
    let decoded = record.decode(&DecodeLimits::default()).unwrap();
    let subs = match (&decoded, record.subrecords()) {
        (Some(d), _) => d.as_slice(),
        (None, Some(s)) => s,
        (None, None) => return,
    };
    for s in subs.iter().filter(|s| s.signature() == Signature(*b"VMAD")) {
        *stats
            .per_type
            .entry(record.header.signature.to_string())
            .or_default() += 1;
        let at = format!(
            "{file} {} {:08X}",
            record.header.signature, record.header.form_id
        );
        let v = match vmad::decode(s.data()) {
            Ok(v) => v,
            Err(e) => {
                stats.failures.push(format!("{at}: {e}"));
                continue;
            }
        };
        match vmad::encode(&v) {
            Ok(bytes) if bytes == s.data() => {}
            Ok(_) => stats.failures.push(format!("{at}: re-encodes differently")),
            Err(e) => stats.failures.push(format!("{at}: {e}")),
        }
        *stats
            .formats
            .entry((v.version, v.object_format))
            .or_default() += 1;
        stats.scripts += v.scripts.len() as u64;
        if !v.rest.is_empty() {
            stats.with_rest += 1;
        }
        for s in &v.scripts {
            s.properties.iter().for_each(|p| count_types(p, stats));
        }
        for (_, o) in vmad::objects(&v) {
            stats.objects += 1;
            if o.unused != 0 {
                stats.unused_nonzero += 1;
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

#[test]
#[ignore = "reads 2.5 GB of plugins; run in release"]
fn every_official_vmad_roundtrips() {
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
    let total: u64 = stats.per_type.values().sum();
    println!("VMAD payloads: {total} by record type {:?}", stats.per_type);
    println!(
        "formats {:?}; scripts {}; with a fragment block {}; property types {:?}; objects {} ({} with non-zero unused bytes)",
        stats.formats,
        stats.scripts,
        stats.with_rest,
        stats.types,
        stats.objects,
        stats.unused_nonzero
    );
    for f in stats.failures.iter().take(20) {
        println!("  {f}");
    }
    assert!(
        stats.failures.is_empty(),
        "{} VMADs do not roundtrip",
        stats.failures.len()
    );
    assert!(total > 0);
}
