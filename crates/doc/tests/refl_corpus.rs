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
//! type that cannot decode or re-encode is printed with its FormID and reason.

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

#[derive(Default)]
struct Stats {
    /// Per record type: (decoded, identical, entries).
    per_type: BTreeMap<String, (u64, u64, u64)>,
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
                Ok(v) => v.len() as u64,
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
}
