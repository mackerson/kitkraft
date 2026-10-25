//! Every `REFL` of the official files decodes through `kitkraft_doc::refl` and re-encodes
//! byte-identically, and every `LMSW` one yields its entries (swaps, material references
//! and named property values). `#[ignore]`: reads 2.5 GB. Run
//! `cargo test -p kitkraft-doc --release --test refl_corpus -- --include-ignored --nocapture`
//! with `KITKRAFT_DATA_DIR` set.
//!
//! One line per record type that carries a `REFL`:
//! `refl-corpus <TYPE>: <n> decoded, <m> identical, <e> entries`, where `<e>` counts
//! `lmsw_entries` results (0 for the other types). A `LMSW` `REFL` that fails to decode,
//! re-encodes differently, or errors in `lmsw_entries` fails the test; the 19 `LMSW`
//! payloads that hold only empty lists legitimately yield no entry. A `REFL` of another
//! type that cannot decode or re-encode is printed with its FormID and reason, and fails
//! the test too: all 29,897 official `REFL`s decode and re-encode, so one that does not is
//! a regression. The `LMSW` shape (swaps, references, properties, and the payloads
//! without swaps) is pinned to the counts the `refl` module docs give.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitkraft_doc::refl;
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

/// The official LMSW records: the count the ticket fixes and the corpus holds.
const LMSW_TOTAL: u64 = 17410;

/// Every `REFL` subrecord of the 14 official files (2026-10-25).
const REFL_TOTAL: u64 = 29897;

/// What the `LMSW` entries hold, for the module docs' counts.
#[derive(Default, Debug, PartialEq, Eq)]
struct LmswShape {
    swaps: u64,
    refs: u64,
    properties: u64,
    /// Payloads with no swap.
    without_swaps: u64,
    /// Payloads with no swap and no property, only material references.
    refs_only: u64,
    /// Payloads with no entry at all (only empty lists).
    empty: u64,
    /// Payloads with named property values.
    with_properties: u64,
}

#[derive(Default)]
struct Stats {
    /// Per record type: (decoded, identical, entries).
    per_type: BTreeMap<String, (u64, u64, u64)>,
    lmsw: LmswShape,
    /// LMSW failures: decode, re-encode and entry errors, with the FormID.
    failures: Vec<String>,
    /// Non-LMSW `REFL`s that cannot decode or re-encode: FormID and reason.
    notes: Vec<String>,
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
    let lmsw = record.header.signature == Signature(*b"LMSW");
    for s in subs.iter().filter(|s| s.signature() == Signature(*b"REFL")) {
        let at = format!(
            "{file} {}:{:08X}",
            record.header.signature, record.header.form_id
        );
        let r = match refl::decode(s.data()) {
            Ok(r) => r,
            Err(e) => {
                if lmsw {
                    stats.failures.push(format!("{at}: {e}"));
                } else {
                    stats.notes.push(format!("{at}: {e}"));
                }
                continue;
            }
        };
        let same = refl::encode(&r) == s.data();
        if !same {
            if lmsw {
                stats.failures.push(format!("{at}: re-encodes differently"));
            } else {
                stats.notes.push(format!("{at}: re-encodes differently"));
            }
        }
        let entries = if lmsw {
            match refl::lmsw_entries(&r) {
                Ok(v) => {
                    let count =
                        |f: fn(&refl::SwapEntry) -> bool| v.iter().filter(|e| f(e)).count() as u64;
                    let swaps = count(|e| matches!(e, refl::SwapEntry::Swap { .. }));
                    let refs = count(|e| matches!(e, refl::SwapEntry::Ref(_)));
                    let properties = count(|e| matches!(e, refl::SwapEntry::Property { .. }));
                    let l = &mut stats.lmsw;
                    l.swaps += swaps;
                    l.refs += refs;
                    l.properties += properties;
                    l.without_swaps += u64::from(swaps == 0);
                    l.refs_only += u64::from(swaps == 0 && properties == 0 && refs > 0);
                    l.empty += u64::from(v.is_empty());
                    l.with_properties += u64::from(properties > 0);
                    v.len() as u64
                }
                Err(e) => {
                    stats.failures.push(format!("{at}: lmsw_entries: {e}"));
                    0
                }
            }
        } else {
            0
        };
        let t = stats
            .per_type
            .entry(record.header.signature.to_string())
            .or_default();
        t.0 += 1;
        if same {
            t.1 += 1;
        }
        t.2 += entries;
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
fn every_official_refl_decodes_and_reencodes() {
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
    for (ty, (decoded, identical, entries)) in &stats.per_type {
        println!("refl-corpus {ty}: {decoded} decoded, {identical} identical, {entries} entries");
    }
    for f in stats.notes.iter().take(20) {
        println!("  {f}");
    }
    for f in stats.failures.iter().take(20) {
        println!("  {f}");
    }
    let lmsw = stats.per_type.get("LMSW").copied().unwrap_or((0, 0, 0));
    assert!(
        stats.failures.is_empty(),
        "{} LMSW REFLs fail to decode, re-encode or yield entries",
        stats.failures.len()
    );
    assert_eq!(
        lmsw.0, LMSW_TOTAL,
        "expected the {LMSW_TOTAL} official LMSW records to hold a REFL"
    );
    assert_eq!(lmsw.1, LMSW_TOTAL);
    assert!(lmsw.2 > 0);
    println!("refl-corpus LMSW shape: {:?}", stats.lmsw);
    // Every other type decodes and re-encodes too, so a failure there is a regression.
    assert!(
        stats.notes.is_empty(),
        "{} non-LMSW REFLs fail to decode or re-encode",
        stats.notes.len()
    );
    let total: u64 = stats.per_type.values().map(|t| t.0).sum();
    let identical: u64 = stats.per_type.values().map(|t| t.1).sum();
    assert_eq!((total, identical), (REFL_TOTAL, REFL_TOTAL));
    // The counts the module docs give.
    assert_eq!(
        stats.lmsw,
        LmswShape {
            swaps: 17643,
            refs: 24277,
            properties: 18,
            without_swaps: 2586,
            refs_only: 2567,
            empty: 19,
            with_properties: 18,
        }
    );
}
