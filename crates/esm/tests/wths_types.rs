//! The WTHS definition (WeatherSettings) against the official ESMs, read in place
//! through the record index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every WTHS record is read with [`read_indexed`] (compressed ones decompressed), decoded
//! through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`);
//! - the `Starfield.esm` record `0002139D` (`WeatherUniqueRedMile_PartlyCloudy_C5_Unique`)
//!   carries the pinned values: `ReflectionParent` is `FormId(0x0002B56D)`,
//!   `Reflection` is absent and `ReflectionDiff` is present.
//!
//! It prints `wths WTHS: <n> records, <m> identical` (plain integers); per signature, how
//! many records have it, and the payload size range of each signature.
//!
//! - `wths_types_full` (ignored) runs all 14 files, every record; run it in release:
//!   `cargo test -p kitkraft-esm --release --test wths_types -- --ignored --nocapture`

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
use kitkraft_esm::generated::records::{match_body, FormId, Record as Typed, RECORDS};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::FLAG_LOCALIZED;
use kitkraft_esm::DecodeLimits;

const SIG: [u8; 4] = *b"WTHS";

#[derive(Default)]
struct Stats {
    records: u64,
    identical: u64,
    /// How many records carry at least one subrecord of each signature.
    sigs: BTreeMap<String, u64>,
    /// Per signature, the payload size range (min, max) in bytes, over all occurrences.
    sizes: BTreeMap<String, (usize, usize)>,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
    /// The `Starfield.esm` record `0002139D` was seen and its values match the pin.
    value_checked: bool,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn check_record(name: &str, form_id: u32, subs: &[RawSub<'_>], ctx: Ctx, t: &mut Stats) {
    let Some(typed) = Typed::decode(SIG, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for WTHS");
    };
    if name == "Starfield.esm" && form_id == 0x0002139D {
        let Typed::Wths(r) = &typed else {
            panic!("0002139D: not a WTHS");
        };
        assert!(r.reflection.is_none(), "0002139D: Reflection is present");
        assert!(
            r.reflection_diff.is_some(),
            "0002139D: ReflectionDiff is absent"
        );
        assert_eq!(
            r.reflection_parent.as_ref().map(|s| s.value),
            Some(FormId(0x0002B56D)),
            "0002139D: ReflectionParent differs"
        );
        t.value_checked = true;
    }
    let mut seen = BTreeSet::new();
    for sub in subs {
        let k = key(sub.signature);
        if seen.insert(k.clone()) {
            *t.sigs.entry(k.clone()).or_insert(0) += 1;
        }
        let len = sub.data.len();
        match t.sizes.entry(k) {
            Entry::Occupied(mut e) => {
                e.get_mut().0 = e.get().0.min(len);
                e.get_mut().1 = e.get().1.max(len);
            }
            Entry::Vacant(e) => {
                e.insert((len, len));
            }
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
        "wths WTHS: {} records, {} identical",
        t.records, t.identical
    );
    if !t.unplaced.is_empty() || !t.failed.is_empty() {
        println!("   unplaced: {:?} failed: {:#?}", t.unplaced, t.failed);
    }
    for f in &t.failures {
        println!("   ! {f}");
    }
    // Per signature: how many records have it.
    for (sig, n) in &t.sigs {
        println!("wths WTHS {sig}: {n} records");
    }
    for (sig, (lo, hi)) in &t.sizes {
        if lo == hi {
            println!("   {sig} payload: {lo} bytes");
        } else {
            println!("   {sig} payload: {lo}..{hi} bytes");
        }
    }
}

fn assert_clean(t: &Stats) {
    assert!(t.unplaced.is_empty(), "unplaced {:?}", t.unplaced);
    assert!(t.failed.is_empty(), "failed {:?}", t.failed);
    assert_eq!(t.identical, t.records, "re-encode differs");
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn wths_types_full() {
    let Some(files) = common::all_files("wths_types_full") else {
        return;
    };
    let mut t = Stats::default();
    for f in &files {
        check_file(&f.path, &mut t);
    }
    report(&t);
    assert!(t.records > 0, "no WTHS record in the official ESMs");
    assert_clean(&t);
    assert!(t.value_checked, "0002139D not seen");
    println!("wths value check: ok");
}
