//! The shared Destructible group (`crates/schema/defs/common/Destructible.toml`, the
//! destructible helper of `wbDefinitionsSF1.pas` L3829-L3890) on the official files
//! (`KITKRAFT_DATA_DIR`; skips when unset), wherever the schema places it: as a member of a
//! record type (ACTI, MSTT, PROJ and others) and inside a GBFM's destructible component
//! block (`BfcbBlock.Destructible`, KK-289).
//!
//! For every record with a subrecord placed in the group (`kitkraft_doc::typed::placements`,
//! the generated decoder's rules), the generated decoder keeps none of them raw (`extra`) and
//! the record re-encodes identically, subrecord by subrecord. The occurrences per member
//! path are the counts `Destructible.toml` cites for each member's `corpus` confidence.
//!
//! `destructible_members_small` runs in `cargo test` (the five small files);
//! `destructible_members_full` (ignored) runs all 14 and checks the counts:
//! `cargo test -p kitkraft-doc --release --test destructible -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kitkraft_doc::typed::{self, Placement};
use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::generated::records::Record as Generated;
use kitkraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature, Subrecord};

const SMALL: [&str; 5] = [
    "Constellation.esm",
    "OldMars.esm",
    "SFBGS007.esm",
    "SFBGS004.esm",
    "SFBGS047.esm",
];

const ALL: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS006.esm",
    "SFBGS003.esm",
    "ShatteredSpace.esm",
    "SFBGS004.esm",
    "SFBGS00D.esm",
    "SFBGS050.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS047.esm",
];

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

/// Whether `path` is a member of the Destructible group (a record's or a component's).
fn in_destructible(path: &str) -> bool {
    path.split('.').any(|seg| seg == "Destructible")
}

#[derive(Debug, Default)]
struct Tally {
    /// Occurrences per member path.
    paths: BTreeMap<String, u64>,
    /// Payload sizes per member path.
    sizes: BTreeMap<String, BTreeSet<usize>>,
    /// Record types holding the group.
    types: BTreeSet<String>,
    /// Records holding the group.
    records: u64,
}

fn check_record(
    plugin: &str,
    localized: bool,
    r: &Record<'_>,
    subs: &[Subrecord<'_>],
    t: &mut Tally,
) {
    let schema = kitkraft_schema::builtin().unwrap();
    let sig = r.header.signature.0;
    let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
    let Some(placed) = typed::placements(schema, sig, &sigs) else {
        return;
    };
    let ours: Vec<(usize, &str)> = placed
        .iter()
        .enumerate()
        .filter_map(|(i, p)| match p {
            Placement::Member { path, .. } if in_destructible(path) => Some((i, path.as_str())),
            _ => None,
        })
        .collect();
    if ours.is_empty() {
        return;
    }
    let label = format!("{plugin} {}:{:08X}", r.header.signature, r.header.form_id);
    let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
    let ctx = Ctx {
        form_version: r.header.form_version,
        localized,
    };
    let generated = Generated::decode(sig, &raw, ctx)
        .unwrap_or_else(|| panic!("{label}: the generated decoder does not take the record"));
    let extra: BTreeSet<usize> = generated.extra().iter().map(|(i, _)| *i).collect();
    for (i, path) in ours {
        assert!(
            !extra.contains(&i),
            "{label}: subrecord {i} ({}) placed in {path} but kept raw",
            subs[i].signature()
        );
        *t.paths.entry(path.to_string()).or_default() += 1;
        t.sizes
            .entry(path.to_string())
            .or_default()
            .insert(subs[i].data().len());
    }
    t.records += 1;
    t.types.insert(r.header.signature.to_string());
    let encoded = generated.encode();
    assert_eq!(encoded.len(), raw.len(), "{label}: subrecord count");
    for (i, (a, b)) in raw.iter().zip(&encoded).enumerate() {
        assert!(
            a.signature == b.signature && a.data == b.data.as_slice() && a.oversized == b.oversized,
            "{label}: subrecord {i} re-encodes differently"
        );
    }
}

fn walk(
    plugin: &str,
    localized: bool,
    entry: &Entry<'_>,
    t: &mut Tally,
) -> kitkraft_esm::Result<()> {
    match entry {
        Entry::Group(g) => {
            for child in g.children() {
                walk(plugin, localized, child, t)?;
            }
        }
        Entry::Record(r) => {
            if r.header.signature == Signature::TES4 {
                return Ok(());
            }
            let decoded = r.decode(&DecodeLimits::default())?;
            let subs: &[Subrecord<'_>] = match (&decoded, r.subrecords()) {
                (Some(d), _) => d,
                (None, Some(s)) => s,
                (None, None) => &[],
            };
            check_record(plugin, localized, r, subs, t);
        }
    }
    Ok(())
}

fn scan(dir: &Path, names: &[&str]) -> Tally {
    let mut t = Tally::default();
    for name in names {
        let path = dir.join(name);
        let header = kitkraft_doc::index::read_header(&path).unwrap();
        let localized = header.flags.localized();
        visit_file(&path, &ParseOptions::default(), |entry| {
            walk(name, localized, entry, &mut t)
        })
        .unwrap();
    }
    eprintln!("records holding the Destructible group: {}", t.records);
    for (path, n) in &t.paths {
        eprintln!("  {path}: {n} (sizes {:?})", t.sizes[path]);
    }
    eprintln!("  record types: {:?}", t.types);
    t
}

#[test]
fn destructible_members_small() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    scan(&dir, &SMALL);
}

#[test]
#[ignore = "release: reads all 14 official files (about 2.5 GB)"]
fn destructible_members_full() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let t = scan(&dir, &ALL);
    // The counts common/Destructible.toml cites (2026-11-15, KK-289).
    let paths: Vec<(&str, u64)> = t.paths.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(
        paths,
        [
            ("BfcbBlock.Destructible.Header", 2_149),
            ("BfcbBlock.Destructible.Stage.Data", 2_188),
            ("BfcbBlock.Destructible.Stage.EndMarker", 2_188),
            ("BfcbBlock.Destructible.Stage.ModelFileName", 51),
            ("BfcbBlock.Destructible.Stage.ModelLightLayer", 51),
            ("Destructible.Header", 498),
            ("Destructible.Resistances", 4),
            ("Destructible.SecondaryDamageList", 2),
            ("Destructible.Stage.AnimEvent", 72),
            ("Destructible.Stage.Data", 712),
            ("Destructible.Stage.EndMarker", 712),
            ("Destructible.Stage.ModelFileName", 262),
            ("Destructible.Stage.ModelLightLayer", 262),
            ("Destructible.Stage.ModelMaterialSwap", 74),
        ]
    );
    // DAMC is an array of 12-byte entries; DSDL, FLLD and DMDS are 4 bytes.
    assert!(t.sizes["Destructible.Resistances"]
        .iter()
        .all(|n| n % 12 == 0));
    for p in [
        "Destructible.SecondaryDamageList",
        "Destructible.Stage.ModelLightLayer",
        "Destructible.Stage.ModelMaterialSwap",
        "BfcbBlock.Destructible.Stage.ModelLightLayer",
    ] {
        assert_eq!(t.sizes[p], BTreeSet::from([4]), "{p}");
    }
    assert_eq!(t.records, 2_647);
}
