//! The CSTY (CombatStyleForm) definition against the official ESMs, read in place
//! through the record index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every CSTY record is read with [`read_indexed`] (compressed ones decompressed),
//! decoded through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints, per type, `csty <SIG>: <n> records, <m> identical` and, per signature,
//! how many records have it, the occurrence count and the payload sizes (plain
//! integers, no separators).
//!
//! `csty_types_full` (ignored) runs all 14 files, every record; run it in release:
//! `cargo test -p kitkraft-esm --release --test csty_types -- --ignored --nocapture`

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

const SIG: [u8; 4] = *b"CSTY";

/// One subrecord signature: how many records carry it, how often it occurs, and the
/// smallest and largest payload seen, in bytes.
#[derive(Default, Clone, Copy)]
struct SigStats {
    records: u64,
    occurrences: u64,
    min: Option<usize>,
    max: Option<usize>,
}

#[derive(Default)]
struct TypeStats {
    records: u64,
    identical: u64,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
    per_sig: BTreeMap<String, SigStats>,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn check_record(name: &str, form_id: u32, subs: &[RawSub<'_>], ctx: Ctx, t: &mut TypeStats) {
    let Some(typed) = Typed::decode(SIG, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for {}", key(SIG));
    };
    let mut in_record = BTreeSet::new();
    for sub in subs {
        let k = key(sub.signature);
        let s = t.per_sig.entry(k.clone()).or_default();
        s.occurrences += 1;
        if in_record.insert(k) {
            s.records += 1;
        }
        s.min = Some(s.min.map_or(sub.data.len(), |m| m.min(sub.data.len())));
        s.max = Some(s.max.map_or(sub.data.len(), |m| m.max(sub.data.len())));
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
            t.failures
                .push(format!("{name} {form_id:08X}: {k} at {i} matches no member"));
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
        t.failures.push(format!("{name} {form_id:08X}: re-encoding differs"));
    }
}

fn check_file(path: &Path, stats: &mut TypeStats) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let index = index_file(path, &IndexOptions::default()).unwrap();
    let localized = index.header.header.flags & FLAG_LOCALIZED != 0;
    let limits = DecodeLimits::default();
    let mut input = BufReader::new(File::open(path).unwrap());
    for loc in &index.records {
        if loc.signature.bytes() != SIG {
            continue;
        }
        let record = read_indexed(&mut input, loc, Some(&limits)).unwrap();
        stats.records += 1;
        let subs: Vec<RawSub<'_>> = record
            .subrecords()
            .expect("decoded")
            .iter()
            .map(RawSub::from)
            .collect();
        let ctx = Ctx::for_record(&record, localized);
        check_record(&name, loc.form_id, &subs, ctx, stats);
    }
}

fn report(stats: &TypeStats) {
    println!("csty CSTY: {} records, {} identical", stats.records, stats.identical);
    if !stats.unplaced.is_empty() || !stats.failed.is_empty() {
        println!("   unplaced: {:?} failed: {:#?}", stats.unplaced, stats.failed);
    }
    for f in &stats.failures {
        println!("   ! {f}");
    }
    for (sig, s) in &stats.per_sig {
        let sizes = match (s.min, s.max) {
            (Some(a), Some(b)) if a == b => format!("{a} bytes"),
            (Some(a), Some(b)) => format!("{a}..{b} bytes"),
            _ => "0 bytes".to_string(),
        };
        println!(
            "   {sig}: {} records ({} occurrences), {sizes}",
            s.records, s.occurrences
        );
    }
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn csty_types_full() {
    let Some(files) = common::all_files("csty_types_full") else {
        return;
    };
    let mut stats = TypeStats::default();
    for f in &files {
        check_file(&f.path, &mut stats);
    }
    report(&stats);
    assert!(stats.unplaced.is_empty(), "unplaced: {:?}", stats.unplaced);
    assert!(stats.failed.is_empty(), "failed: {:?}", stats.failed);
    assert_eq!(stats.identical, stats.records, "re-encoding differs");
    assert!(stats.records > 0, "no CSTY record");
}
