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

/// The VMAD of the record at `offset` in `path`.
fn vmad_at(path: &std::path::Path, offset: u64) -> Vec<u8> {
    let mut file = std::fs::File::open(path).unwrap();
    let record =
        kitkraft_esm::index::read_record_at(&mut file, offset, Some(&DecodeLimits::default()))
            .unwrap();
    let subs = record.subrecords().unwrap();
    subs.iter()
        .find(|s| s.signature() == Signature(*b"VMAD"))
        .unwrap()
        .data()
        .to_vec()
}

/// M4.E9: `set` attaches a base game script to a copy of a vanilla reference that already
/// has scripts: reading the block back gives the vanilla scripts byte for byte, then the
/// new one, then the vanilla fragment bytes; nothing else in the payload moves.
#[test]
#[ignore = "loads Starfield.esm; run in release"]
fn attaching_to_a_corpus_copy_keeps_the_block() {
    let Some(dir) = std::env::var_os("KITKRAFT_DATA_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
    else {
        println!("skipped: KITKRAFT_DATA_DIR is unset or not a directory");
        return;
    };
    let load =
        serde_json::json!({"plugins": ["Starfield.esm"], "data_dir": dir.display().to_string()});
    let mut q = load.clone();
    q["type"] = "REFR".into();
    q["where"] = "has VMAD".into();
    q["limit"] = 1.into();
    let found = kitkraft_engine::execute("query", q).unwrap();
    let hit = &found["data"]["results"][0];
    let form = hit["form"].as_str().unwrap().to_string();
    let offset = hit["offset"].as_u64().unwrap();
    let original = vmad_at(&dir.join("Starfield.esm"), offset);
    let before = vmad::decode(&original).unwrap();
    assert_eq!(vmad::encode(&before).unwrap(), original);

    let out_dir = std::env::temp_dir().join(format!("kitkraft-vmad-corpus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    let into = out_dir.join("Attach.esp");
    let mut set = load.clone();
    set["form"] = form.clone().into();
    set["into"] = into.display().to_string().into();
    set["fields"] = serde_json::json!({});
    set["scripts"] = serde_json::json!([{"name": "DefaultDisableHavokOnLoad"}]);
    let r = kitkraft_engine::execute("set", set).unwrap();
    assert_eq!(
        r["data"]["validate"]["passed"],
        serde_json::json!(true),
        "{:#}",
        r["data"]
    );
    let written = r["data"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["form"] == serde_json::json!(form))
        .unwrap()["offset"]
        .as_u64()
        .unwrap();
    let after = vmad::decode(&vmad_at(&into, written)).unwrap();
    let n = before.scripts.len();
    assert_eq!(after.scripts.get(..n).unwrap(), before.scripts.as_slice());
    assert_eq!(after.scripts.len(), n + 1);
    assert_eq!(after.scripts[n].name_text(), "DefaultDisableHavokOnLoad");
    assert_eq!(after.rest, before.rest);
    // The new block is the vanilla one with the script appended, re-encoded.
    let mut expected = before.clone();
    expected.scripts.push(after.scripts[n].clone());
    assert_eq!(vmad_at(&into, written), vmad::encode(&expected).unwrap());
    println!(
        "{form}: {} vanilla script(s) kept, {} fragment byte(s) kept, 1 attached",
        n,
        before.rest.len()
    );
    let _ = std::fs::remove_dir_all(&out_dir);
}
