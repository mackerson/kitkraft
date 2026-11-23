//! The EFSH definition (EffectShader) against the official ESMs, read in place
//! through the record index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every EFSH record is read with [`read_indexed`] (compressed ones decompressed), decoded
//! through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints `efsh EFSH: <n> records, <m> identical` (plain integers); per signature, how
//! many records have it, and the payload size range of each signature. The record
//! `Starfield.esm` FormID `0x0001914F` (EditorID `OreMineFXS`, as stored in that file) is
//! checked value for value; the test prints `efsh value check: ok`.
//!
//! - `efsh_types_full` (ignored) runs all 14 files, every record:
//!   `cargo test -p kitkraft-esm --release --test efsh_types -- --ignored --nocapture`

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

const SIG: [u8; 4] = *b"EFSH";

#[derive(Default)]
struct Stats {
    records: u64,
    identical: u64,
    /// How many records carry at least one subrecord of each signature.
    sigs: BTreeMap<String, u64>,
    /// Per signature, the payload size range (min, max) in bytes, over all occurrences.
    sizes: BTreeMap<String, (usize, usize)>,
    /// The value check on Starfield.esm 0001914F ran.
    value_checked: bool,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn check_record(name: &str, form_id: u32, subs: &[RawSub<'_>], ctx: Ctx, t: &mut Stats) {
    let Some(typed) = Typed::decode(SIG, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for EFSH");
    };
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
    // The record values, read from the generated typed fields.
    if name == "Starfield.esm" && form_id == 0x0001914F {
        let Typed::Efsh(r) = &typed else {
            panic!("0001914F: not a EFSH");
        };
        assert!(
            r.effect_sequence.is_none(),
            "0001914F: EffectSequence present"
        );
        assert!(r.empty.is_some(), "0001914F: Empty absent");
        let data = r.data.as_ref().expect("0001914F: no Data");
        assert_eq!(data.edge_fall_off.to_bits(), 0x3F4CCCCD);
        assert_eq!(data.edge_alpha_fade_in_time.to_bits(), 0x40000000);
        assert_eq!(data.edge_full_alpha_time.to_bits(), 0x3F99999A);
        assert_eq!(data.holes_stop_time.to_bits(), 0x41200000);
        assert_eq!(data.holes_start_value.to_bits(), 0x437F0000);
        assert_eq!(data.bone_depth, 4);
        assert_eq!(data.flags.0, 0x00000008);
        let file_name = r
            .model
            .as_ref()
            .and_then(|m| m.file_name.as_ref())
            .expect("0001914F: no model file name");
        assert!(file_name.0.ends_with(b"OreMineFXP.nif"));
        t.value_checked = true;
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
        "efsh EFSH: {} records, {} identical",
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
        println!("efsh EFSH {sig}: {n} records");
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
fn efsh_types_full() {
    let Some(files) = common::all_files("efsh_types_full") else {
        return;
    };
    let mut t = Stats::default();
    for f in &files {
        check_file(&f.path, &mut t);
    }
    report(&t);
    assert!(t.records > 0, "no EFSH record in the official ESMs");
    assert_clean(&t);
    assert!(t.value_checked, "0001914F not seen");
    println!("efsh value check: ok");
}
