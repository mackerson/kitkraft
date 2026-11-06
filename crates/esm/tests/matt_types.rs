//! The MATT (MaterialType) definition against the official ESMs, read in place
//! through the record index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every MATT record is read with [`read_indexed`] (compressed ones decompressed),
//! decoded through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints `matt MATT: <n> records, <m> identical` (plain integers, no separators)
//! and, per subrecord signature, how many records have it and the payload sizes seen
//! - the facts the `corpus:` evidence lines in `crates/schema/defs/MATT.toml` quote -
//! plus how many records carry both an `HNAM` and an `ANAM`.
//!
//! `matt_types_full` (ignored) runs all 14 files, every record; run it in release:
//! `cargo test -p kitkraft-esm --release --test matt_types -- --ignored --nocapture`

// Test-only code: a panic fails the test.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used
)]

mod common;

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::{match_body, Record as Typed, RECORDS};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::FLAG_LOCALIZED;
use kitkraft_esm::DecodeLimits;

const SIG: [u8; 4] = *b"MATT";

#[derive(Default)]
struct TypeStats {
    records: u64,
    identical: u64,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
    /// Per subrecord signature: how many records have it, and the smallest and
    /// largest payload size seen in it.
    per_sig: BTreeMap<String, (u64, usize, usize)>,
    /// How many records carry both an `HNAM` and an `ANAM`.
    hnam_anam_together: u64,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn check_record(name: &str, form_id: u32, subs: &[RawSub<'_>], ctx: Ctx, t: &mut TypeStats) {
    let Some(typed) = Typed::decode(SIG, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for {}", key(SIG));
    };
    let mut seen = BTreeSet::new();
    for sub in subs {
        let len = sub.data.len();
        let is_first = seen.insert(sub.signature);
        match t.per_sig.entry(key(sub.signature)) {
            Entry::Occupied(mut o) => {
                let (count, min, max) = o.get_mut();
                if is_first {
                    *count += 1;
                }
                *min = (*min).min(len);
                *max = (*max).max(len);
            }
            Entry::Vacant(v) => {
                v.insert((1, len, len));
            }
        }
    }
    if seen.contains(&b"HNAM") && seen.contains(&b"ANAM") {
        t.hnam_anam_together += 1;
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

fn check_file(path: &Path, t: &mut TypeStats) {
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

fn report(t: &TypeStats) {
    println!("matt MATT: {} records, {} identical", t.records, t.identical);
    if !t.unplaced.is_empty() || !t.failed.is_empty() {
        println!("   unplaced: {:?} failed: {:#?}", t.unplaced, t.failed);
    }
    for f in &t.failures {
        println!("   ! {f}");
    }
    for (sig, (count, min, max)) in &t.per_sig {
        if *min == *max {
            println!("   {sig}: {count} records ({min} bytes)");
        } else {
            println!("   {sig}: {count} records ({min}..{max} bytes)");
        }
    }
    println!("   records with both HNAM and ANAM: {}", t.hnam_anam_together);
}

fn assert_clean(t: &TypeStats) {
    assert!(t.unplaced.is_empty(), "unplaced {:?}", t.unplaced);
    assert!(t.failed.is_empty(), "failed {:?}", t.failed);
    assert_eq!(t.identical, t.records, "re-encode differs");
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn matt_types_full() {
    let Some(files) = common::all_files("matt_types_full") else {
        return;
    };
    let mut t = TypeStats::default();
    for f in &files {
        check_file(&f.path, &mut t);
    }
    report(&t);
    assert_clean(&t);
    assert!(t.records > 0, "no MATT record");
}
