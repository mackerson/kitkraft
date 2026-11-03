//! SECH (Sound Echo Marker) against the official ESMs, read in place through the record
//! index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every record of the type is read with [`read_indexed`] (compressed ones decompressed),
//! decoded through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints `sech SECH: <n> records, <m> identical` (plain integers) and, per
//! signature, how many records have it, how often it occurs and its payload sizes,
//! plus the two NNAMs by position (the record's description before the first echo
//! start marker, the echo's own description inside the echoes).
//!
//! `sech_types_full` (ignored) runs all 14 files, every record; run it in release:
//! `cargo test -p kitkraft-esm --release --test sech_types -- --ignored --nocapture`

// Test-only code: a panic fails the test.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used
)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::{match_body, Record as Typed, RECORDS};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::FLAG_LOCALIZED;
use kitkraft_esm::DecodeLimits;

const TYPE: [u8; 4] = *b"SECH";

/// How many records hold the signature, how often it occurs and its payload sizes.
#[derive(Default)]
struct SigStats {
    records: u64,
    times: u64,
    sizes: BTreeMap<usize, u64>,
}

/// How many records (or echoes) hold the NNAM, how often it occurs and its payload sizes.
#[derive(Default)]
struct NnamStats {
    holders: u64,
    times: u64,
    sizes: BTreeMap<usize, u64>,
}

#[derive(Default)]
struct TypeStats {
    records: u64,
    identical: u64,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
    sigs: BTreeMap<String, SigStats>,
    /// The NNAM before the first echo start marker: the record's own description.
    nnam_record: NnamStats,
    /// The NNAM inside the echoes: the echo's own description.
    nnam_echo: NnamStats,
    /// Echoes, one per ECHE.
    echoes: u64,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn check_record(
    name: &str,
    sig: [u8; 4],
    form_id: u32,
    subs: &[RawSub<'_>],
    ctx: Ctx,
    t: &mut TypeStats,
) {
    let Some(typed) = Typed::decode(sig, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for {}", key(sig));
    };
    // Per-signature stats over every subrecord of the record, echo members included.
    // The two NNAMs count by position: before the first echo start marker the NNAM is
    // the record's description, inside the echoes the echo's own.
    let mut seen = BTreeSet::new();
    let mut in_echo = false;
    let mut echo_nnam = false;
    let mut record_nnam = false;
    for sub in subs {
        let s = t.sigs.entry(key(sub.signature)).or_default();
        s.times += 1;
        *s.sizes.entry(sub.data.len()).or_insert(0) += 1;
        if seen.insert(key(sub.signature)) {
            s.records += 1;
        }
        if sub.signature == *b"ECHO" || sub.signature == *b"ECHD" {
            in_echo = true;
            echo_nnam = false;
        } else if sub.signature == *b"NNAM" {
            let n = if in_echo {
                &mut t.nnam_echo
            } else {
                &mut t.nnam_record
            };
            n.times += 1;
            *n.sizes.entry(sub.data.len()).or_insert(0) += 1;
            if in_echo {
                echo_nnam = true;
            } else {
                record_nnam = true;
            }
        } else if sub.signature == *b"ECHE" && in_echo {
            t.echoes += 1;
            if echo_nnam {
                t.nnam_echo.holders += 1;
            }
        }
    }
    if record_nnam {
        t.nnam_record.holders += 1;
    }
    let members = RECORDS
        .iter()
        .find(|(s, _)| *s == sig)
        .map(|(_, m)| *m)
        .unwrap();
    let (_, unmatched) = match_body(sig, subs, members);
    for i in &unmatched {
        let k = key(subs[*i].signature);
        *t.unplaced.entry(k.clone()).or_insert(0) += 1;
        if t.failures.len() < 20 {
            t.failures.push(format!(
                "{name} {form_id:08X}: {k} at {i} matches no member"
            ));
        }
    }
    for (i, sub) in typed.extra() {
        if !unmatched.contains(i) {
            let k = key(sub.signature);
            *t.failed.entry(k.clone()).or_insert(0) += 1;
            if t.failures.len() < 20 {
                t.failures.push(format!(
                    "{name} {form_id:08X}: {k} at {i} ({} bytes) does not decode",
                    sub.data.len()
                ));
            }
        }
    }
    let encoded = typed.encode();
    let same = encoded.len() == subs.len()
        && encoded.iter().zip(subs).all(|(e, r)| {
            e.signature == r.signature && e.data == r.data && e.oversized == r.oversized
        });
    if same {
        t.identical += 1;
    } else if t.failures.len() < 20 {
        t.failures
            .push(format!("{name} {form_id:08X}: re-encoding differs"));
    }
}

fn check_file(path: &Path, t: &mut TypeStats) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let index = index_file(path, &IndexOptions::default()).unwrap();
    let localized = index.header.header.flags & FLAG_LOCALIZED != 0;
    let limits = DecodeLimits::default();
    let mut input = BufReader::new(File::open(path).unwrap());
    for loc in &index.records {
        if loc.signature.bytes() != TYPE {
            continue;
        }
        let record = read_indexed(&mut input, loc, Some(&limits)).unwrap();
        t.records += 1;
        let subs: Vec<RawSub<'_>> = record
            .subrecords()
            .expect("decoded")
            .iter()
            .map(RawSub::from)
            .collect();
        let ctx = Ctx::for_record(&record, localized);
        check_record(&name, loc.signature.bytes(), loc.form_id, &subs, ctx, t);
    }
}

/// The payload sizes of a signature: one value, or a range.
fn sizes(sizes: &BTreeMap<usize, u64>) -> String {
    let lo = sizes.keys().next().unwrap();
    let hi = sizes.keys().next_back().unwrap();
    if lo == hi {
        format!("{lo} bytes")
    } else {
        format!("{lo} to {hi} bytes")
    }
}

/// The payload sizes of a signature: one value, a range, or none at all.
fn sizes_or_none(all: &BTreeMap<usize, u64>) -> String {
    if all.is_empty() {
        "no payload".into()
    } else {
        sizes(all)
    }
}

fn report(t: &TypeStats) {
    println!(
        "sech SECH: {} records, {} identical",
        t.records, t.identical
    );
    if !t.unplaced.is_empty() || !t.failed.is_empty() {
        println!("   unplaced: {:?} failed: {:#?}", t.unplaced, t.failed);
    }
    for f in &t.failures {
        println!("   ! {f}");
    }
    for (sig, s) in &t.sigs {
        println!(
            "   {sig}: {} records have it, {} times, {}",
            s.records,
            s.times,
            sizes(&s.sizes)
        );
    }
    println!(
        "   NNAM before the first echo: {} of {} records have it, {} times, {}",
        t.nnam_record.holders,
        t.records,
        t.nnam_record.times,
        sizes_or_none(&t.nnam_record.sizes)
    );
    println!(
        "   NNAM in the echoes: {} of {} echoes have it, {} times, {}",
        t.nnam_echo.holders,
        t.echoes,
        t.nnam_echo.times,
        sizes_or_none(&t.nnam_echo.sizes)
    );
}

fn assert_clean(t: &TypeStats) {
    assert!(t.unplaced.is_empty(), "unplaced {:?}", t.unplaced);
    assert!(t.failed.is_empty(), "failed {:?}", t.failed);
    assert_eq!(t.identical, t.records, "re-encode differs");
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn sech_types_full() {
    let Some(files) = common::all_files("sech_types_full") else {
        return;
    };
    let mut t = TypeStats::default();
    for f in &files {
        check_file(&f.path, &mut t);
    }
    report(&t);
    assert_clean(&t);
}
