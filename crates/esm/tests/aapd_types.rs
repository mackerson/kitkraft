//! The AAPD definition (AimAssistPoseDataForm) against the official ESMs, read in place
//! through the record index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every AAPD record is read with [`read_indexed`] (compressed ones decompressed), decoded
//! through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints `aapd AAPD: <n> records, <m> identical` (plain integers); per signature, how
//! many records have it (the `BFCB` and `BFCE` base form component markers always); and,
//! per record, the `AAPS` count next to the number of bullet-bending pills the record
//! holds, with how often the count equals the pills.
//!
//! - `aapd_types_full` (ignored) runs all 14 files, every record; run it in release:
//!   `cargo test -p kitkraft-esm --release --test aapd_types -- --ignored --nocapture`

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

const SIG: [u8; 4] = *b"AAPD";

#[derive(Default)]
struct Stats {
    records: u64,
    identical: u64,
    /// How many records carry at least one subrecord of each signature.
    sigs: BTreeMap<String, u64>,
    /// Per record, the `AAPS` count next to the number of bullet-bending pills held:
    /// (count, pills) to how many records.
    pills: BTreeMap<(u32, usize), u64>,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn check_record(name: &str, form_id: u32, subs: &[RawSub<'_>], ctx: Ctx, t: &mut Stats) {
    let Some(typed) = Typed::decode(SIG, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for AAPD");
    };
    let mut seen = BTreeSet::new();
    for sub in subs {
        let k = key(sub.signature);
        if seen.insert(k.clone()) {
            *t.sigs.entry(k).or_insert(0) += 1;
        }
    }
    let members = RECORDS
        .iter()
        .find(|(s, _)| *s == SIG)
        .map(|(_, m)| *m)
        .unwrap();
    let (_, unmatched) = match_body(SIG, subs, members);
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
    // The (count, pills) table counts every decoded record, not only the ones that
    // re-encode identically: the question is how often AAPS matches the pills held.
    if let Typed::Aapd(aapd) = &typed {
        let count = aapd
            .bullet_bending_pill_count
            .as_ref()
            .map(|c| c.value)
            .unwrap_or(0);
        let pills = aapd.bullet_bending_pills.len();
        *t.pills.entry((count, pills)).or_insert(0) += 1;
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
        if sig != SIG {
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
        "aapd AAPD: {} records, {} identical",
        t.records, t.identical
    );
    if !t.unplaced.is_empty() || !t.failed.is_empty() {
        println!("   unplaced: {:?} failed: {:#?}", t.unplaced, t.failed);
    }
    for f in &t.failures {
        println!("   ! {f}");
    }
    // Per signature: how many records have it. The BFCB/BFCE markers always appear,
    // 0 when absent, since no other member of the shared block is named here.
    let mut table = t.sigs.clone();
    for sig in ["BFCB", "BFCE"] {
        table.entry(sig.to_string()).or_insert(0);
    }
    for (sig, n) in &table {
        println!("   {sig}: {n}");
    }
    for ((count, pills), n) in &t.pills {
        println!("   AAPS {count}, pills {pills}: {n} records");
    }
    let matched: u64 = t
        .pills
        .iter()
        .filter(|((count, pills), _)| *count == u32::try_from(*pills).unwrap_or(0))
        .map(|(_, n)| *n)
        .sum();
    println!(
        "   AAPS equals the pill count in {} of {} records",
        matched, t.records
    );
}

fn assert_clean(t: &Stats) {
    assert!(t.unplaced.is_empty(), "unplaced {:?}", t.unplaced);
    assert!(t.failed.is_empty(), "failed {:?}", t.failed);
    assert_eq!(t.identical, t.records, "re-encode differs");
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn aapd_types_full() {
    let Some(files) = common::all_files("aapd_types_full") else {
        return;
    };
    let mut t = Stats::default();
    for f in &files {
        check_file(&f.path, &mut t);
    }
    report(&t);
    assert!(t.records > 0, "no AAPD record in the official ESMs");
    assert_clean(&t);
}
