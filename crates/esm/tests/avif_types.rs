//! The AVIF (ActorValueInformation) definition against the official ESMs, read in place
//! through the record index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every AVIF record is read with [`read_indexed`] (compressed ones decompressed),
//! decoded through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints `avif AVIF: <n> records, <m> identical`, per signature how many records
//! have it (plain integers), and for `NAM0`-`NAM3`, `NLDT` and `AVFL` the payload sizes
//! and their occurrences per record.
//!
//! `avif_types_full` (ignored) runs all 14 files, every record; run it in release:
//! `cargo test -p kitkraft-esm --release --test avif_types -- --ignored --nocapture`

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
use kitkraft_esm::generated::records::{match_body, MemberDesc, Record as Typed, RECORDS};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::FLAG_LOCALIZED;
use kitkraft_esm::DecodeLimits;

const TYPES: [&[u8; 4]; 1] = [b"AVIF"];

/// The signatures whose payload sizes are reported, over their occurrences per record.
const SIZED: [&[u8; 4]; 6] = [b"AVFL", b"NLDT", b"NAM0", b"NAM1", b"NAM2", b"NAM3"];

#[derive(Default)]
struct Stats {
    records: u64,
    identical: u64,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
    /// Per signature: how many records have it, over every member signature
    /// (zero for the ones no record has).
    per_sig: BTreeMap<String, u64>,
    /// For [`SIZED`] signatures: payload size, then how many records have it at that size.
    sizes: BTreeMap<String, BTreeMap<usize, u64>>,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

/// Every signature a member of `members` can take, groups included.
fn member_sigs(members: &[MemberDesc]) -> Vec<[u8; 4]> {
    let mut out = Vec::new();
    for m in members {
        match m {
            MemberDesc::Sub { sig, .. } => out.push(*sig),
            MemberDesc::Group { members, .. } => out.extend(member_sigs(members)),
            MemberDesc::AnyUntil { .. } => {}
        }
    }
    out
}

fn check_record(
    name: &str,
    sig: [u8; 4],
    form_id: u32,
    subs: &[RawSub<'_>],
    ctx: Ctx,
    t: &mut Stats,
) {
    let Some(typed) = Typed::decode(sig, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for {}", key(sig));
    };
    let mut seen = BTreeSet::new();
    for sub in subs {
        seen.insert(key(sub.signature));
        if SIZED.contains(&&sub.signature) {
            let k = key(sub.signature);
            *t.sizes
                .entry(k)
                .or_default()
                .entry(sub.data.len())
                .or_insert(0) += 1;
        }
    }
    for k in seen {
        *t.per_sig.entry(k).or_insert(0) += 1;
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

fn check_file(path: &Path, t: &mut Stats) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let index = index_file(path, &IndexOptions::default()).unwrap();
    let localized = index.header.header.flags & FLAG_LOCALIZED != 0;
    let limits = DecodeLimits::default();
    let mut input = BufReader::new(File::open(path).unwrap());
    for loc in &index.records {
        let sig = loc.signature.bytes();
        if !TYPES.contains(&&sig) {
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
        check_record(&name, sig, loc.form_id, &subs, ctx, t);
    }
}

fn size_line(sizes: &BTreeMap<usize, u64>) -> String {
    sizes
        .iter()
        .map(|(size, n)| format!("{size} bytes in {n} records"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn report(t: &Stats) {
    println!(
        "avif AVIF: {} records, {} identical",
        t.records, t.identical
    );
    if !t.unplaced.is_empty() || !t.failed.is_empty() {
        println!("   unplaced: {:?} failed: {:#?}", t.unplaced, t.failed);
    }
    for f in &t.failures {
        println!("   ! {f}");
    }
    for (sig, n) in &t.per_sig {
        println!("   {sig}: {n}");
    }
    for s in SIZED {
        let sig = key(*s);
        if let Some(sizes) = t.sizes.get(&sig) {
            println!("   {sig}: {}", size_line(sizes));
        }
    }
}

fn assert_clean(t: &Stats) {
    assert!(t.unplaced.is_empty(), "AVIF: unplaced {:?}", t.unplaced);
    assert!(t.failed.is_empty(), "AVIF: failed {:?}", t.failed);
    assert_eq!(t.identical, t.records, "AVIF: re-encode differs");
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn avif_types_full() {
    let Some(files) = common::all_files("avif_types_full") else {
        return;
    };
    let mut t = Stats::default();
    for f in &files {
        check_file(&f.path, &mut t);
    }
    // Seed the per-signature map with every member signature, so a signature no
    // record has still prints (as zero).
    let members = RECORDS
        .iter()
        .find(|(s, _)| *s == *b"AVIF")
        .map(|(_, m)| *m)
        .unwrap();
    for sig in member_sigs(members) {
        t.per_sig.entry(key(sig)).or_insert(0);
    }
    report(&t);
    assert_clean(&t);
    assert!(t.records > 0, "no AVIF record");
}
