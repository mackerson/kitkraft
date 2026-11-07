//! AVMD (AvmsData) against the official ESMs, read in place through the record index
//! (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every AVMD record is read with [`read_indexed`] (compressed ones decompressed),
//! decoded through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints `avmd AVMD: <n> records, <m> identical` (plain integers) and, per subrecord
//! signature, how many records have it.
//!
//! `avmd_types_full` (ignored) runs all 14 files, every record; run it in release:
//! `cargo test -p kitkraft-esm --release --test avmd_types -- --ignored --nocapture`

// Test-only code: a panic fails the test.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used
)]

mod common;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::{match_body, Record as Typed, RECORDS};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::FLAG_LOCALIZED;
use kitkraft_esm::DecodeLimits;

const SIG: &[u8; 4] = b"AVMD";

#[derive(Default)]
struct Stats {
    records: u64,
    identical: u64,
    /// Per subrecord signature: how many records have it (counted once per record).
    by_sig: BTreeMap<String, u64>,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn check_record(name: &str, form_id: u32, subs: &[RawSub<'_>], ctx: Ctx, t: &mut Stats) {
    let Some(typed) = Typed::decode(*SIG, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for AVMD");
    };
    let mut seen = BTreeSet::new();
    for sub in subs {
        let k = key(sub.signature);
        if seen.insert(k.clone()) {
            *t.by_sig.entry(k).or_insert(0) += 1;
        }
    }
    let members = RECORDS
        .iter()
        .find(|(s, _)| *s == *SIG)
        .map(|(_, m)| *m)
        .unwrap();
    let (_, unmatched) = match_body(*SIG, subs, members);
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

fn check_file(path: &Path, t: &mut Stats) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let index = index_file(path, &IndexOptions::default()).unwrap();
    let localized = index.header.header.flags & FLAG_LOCALIZED != 0;
    let limits = DecodeLimits::default();
    let mut input = BufReader::new(File::open(path).unwrap());
    for loc in &index.records {
        let sig = loc.signature.bytes();
        if sig != *SIG {
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
        check_record(&name, loc.form_id, &subs, ctx, t);
    }
}

fn report(t: &Stats) {
    println!(
        "avmd AVMD: {} records, {} identical",
        t.records, t.identical
    );
    for (sig, n) in &t.by_sig {
        println!("   {sig}: {n} records");
    }
    if !t.unplaced.is_empty() || !t.failed.is_empty() {
        println!("   unplaced: {:?} failed: {:#?}", t.unplaced, t.failed);
    }
    for f in &t.failures {
        println!("   ! {f}");
    }
}

fn assert_clean(t: &Stats) {
    assert!(t.unplaced.is_empty(), "AVMD: unplaced {:?}", t.unplaced);
    assert!(t.failed.is_empty(), "AVMD: failed {:?}", t.failed);
    assert_eq!(t.identical, t.records, "AVMD: re-encode differs");
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn avmd_types_full() {
    let Some(files) = common::all_files("avmd_types_full") else {
        return;
    };
    let mut t = Stats::default();
    for f in &files {
        check_file(&f.path, &mut t);
    }
    report(&t);
    assert_clean(&t);
    assert!(t.records > 0, "no AVMD record");
}
