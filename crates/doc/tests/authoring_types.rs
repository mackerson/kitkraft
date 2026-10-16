//! LCTN, TERM, PKIN and FACT through the typed path on the official files (lane D,
//! 2026-10-15; `KITKRAFT_DATA_DIR`, skips when unset).
//!
//! Every record of the four types is decoded through its generated definition and encoded
//! again. Checks: the generated decoder keeps no subrecord raw (`extra` is empty), no
//! subrecord sits in a raw `any_until` body (every base form component's data is one of the
//! typed layouts), and the encoding reproduces the record subrecord by subrecord. So the
//! records round-trip through the typed values, not the opaque passthrough.
//!
//! `authoring_types_small` runs in `cargo test` (the five small files);
//! `authoring_types_full` (ignored) runs all 14 and checks the record counts:
//! `cargo test -p kitkraft-doc --release --test authoring_types -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kitkraft_doc::typed::{self, Placement};
use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::generated::records::Record as Generated;
use kitkraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature, Subrecord};

const TYPES: [&str; 4] = ["LCTN", "TERM", "PKIN", "FACT"];

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

/// Records and subrecords per type.
type Tally = BTreeMap<String, (u64, u64)>;

fn check(plugin: &str, localized: bool, r: &Record<'_>, subs: &[Subrecord<'_>], t: &mut Tally) {
    let sig = r.header.signature.to_string();
    if !TYPES.contains(&sig.as_str()) {
        return;
    }
    let label = format!("{plugin} {sig}:{:08X}", r.header.form_id);
    let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
    let ctx = Ctx {
        form_version: r.header.form_version,
        localized,
    };
    let g = Generated::decode(r.header.signature.0, &raw, ctx).unwrap();
    assert!(g.extra().is_empty(), "{label}: kept raw {:?}", g.extra());
    let schema = kitkraft_schema::builtin().unwrap();
    let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
    let placed = typed::placements(schema, r.header.signature.0, &sigs).unwrap();
    for (i, p) in placed.iter().enumerate() {
        assert!(
            matches!(p, Placement::Member { .. }),
            "{label}: subrecord {i} ({}) is {p:?}",
            subs[i].signature()
        );
    }
    let encoded = g.encode();
    assert_eq!(encoded.len(), raw.len(), "{label}");
    for (i, (a, b)) in raw.iter().zip(&encoded).enumerate() {
        assert!(
            a.signature == b.signature && a.data == b.data.as_slice() && a.oversized == b.oversized,
            "{label}: subrecord {i} re-encodes differently"
        );
    }
    let e = t.entry(sig).or_default();
    e.0 += 1;
    e.1 += subs.len() as u64;
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
            check(plugin, localized, r, subs, t);
        }
    }
    Ok(())
}

fn scan(dir: &Path, names: &[&str]) -> Tally {
    let mut t = Tally::new();
    for name in names {
        let path = dir.join(name);
        let localized = kitkraft_doc::index::read_header(&path)
            .unwrap()
            .flags
            .localized();
        visit_file(&path, &ParseOptions::default(), |entry| {
            walk(name, localized, entry, &mut t)
        })
        .unwrap();
    }
    for (sig, (records, subs)) in &t {
        eprintln!("{sig}: {records} records, {subs} subrecords, all typed and identical");
    }
    t
}

#[test]
fn authoring_types_small() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    scan(&dir, &SMALL);
}

#[test]
#[ignore = "release: reads all 14 official files (about 2.5 GB)"]
fn authoring_types_full() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let t = scan(&dir, &ALL);
    let records: Vec<(&str, u64)> = t.iter().map(|(k, v)| (k.as_str(), v.0)).collect();
    assert_eq!(
        records,
        [
            ("FACT", 953),
            ("LCTN", 7_316),
            ("PKIN", 15_128),
            ("TERM", 501)
        ]
    );
}
