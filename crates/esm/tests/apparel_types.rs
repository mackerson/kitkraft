//! The two apparel definitions (ARMA ARMO) against the official ESMs, read in place
//! through the record index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every record of both types is read with [`read_indexed`] (compressed ones
//! decompressed), decoded through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints, per type, `apparel <SIG>: <n> records, <m> identical`; for ARMA, the
//! `FLLD` and `XFLG` subrecords by payload size and the `FLLD` count per record; and a
//! slot histogram: for each named biped slot, how many ARMO records set it in `BO64`.
//!
//! - `apparel_types_sample` runs in `cargo test`: every ARMA and ARMO in
//!   `SFBGS003.esm` and `SFBGS004.esm`.
//! - `apparel_types_full` (ignored) runs all 14 files, every record; run it in release:
//!   `cargo test -p kitkraft-esm --release --test apparel_types -- --ignored --nocapture`

// Test-only code: a panic fails the test.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    // check_record threads the per-type stats and the two slot histograms through.
    clippy::too_many_arguments
)]

mod common;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::{match_body, Record as Typed, RECORDS};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::FLAG_LOCALIZED;
use kitkraft_esm::DecodeLimits;
use kitkraft_schema::{builtin, CommonBody, FieldType};

const TYPES: [&[u8; 4]; 2] = [b"ARMA", b"ARMO"];

#[derive(Default)]
struct TypeStats {
    records: u64,
    identical: u64,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
    /// ARMA only: the model `FLLD` and `XFLG` subrecords, by signature, payload size
    /// and count.
    model_flags: BTreeMap<String, BTreeMap<usize, u64>>,
    /// ARMA only: how many `FLLD` each record has, by that count and the number of
    /// records with it.
    fld_per_record: BTreeMap<usize, u64>,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

/// `n` with a comma between every three digits (`1238` -> `1,238`).
fn comma(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.char_indices() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The named biped slots of the shared `BipedSlots` definition, by name and mask.
fn named_slots() -> Vec<(String, u64)> {
    let schema = builtin().unwrap();
    let CommonBody::Type(FieldType::Flags { bits, .. }) = &schema.common["BipedSlots"].body else {
        panic!("BipedSlots is not a flags type");
    };
    bits.iter()
        .map(|b| (b.name.clone(), b.value as u64))
        .collect()
}

fn check_record(
    name: &str,
    sig: [u8; 4],
    form_id: u32,
    subs: &[RawSub<'_>],
    ctx: Ctx,
    t: &mut TypeStats,
    slots: &[(String, u64)],
    armo_slots: &mut BTreeMap<String, u64>,
) {
    let Some(typed) = Typed::decode(sig, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for {}", key(sig));
    };
    if sig == *b"ARMA" {
        let mut fld = 0usize;
        for sub in subs {
            let k = key(sub.signature);
            if k == "FLLD" {
                fld += 1;
            }
            if k == "FLLD" || k == "XFLG" {
                *t.model_flags
                    .entry(k)
                    .or_default()
                    .entry(sub.data.len())
                    .or_insert(0) += 1;
            }
        }
        *t.fld_per_record.entry(fld).or_insert(0) += 1;
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
    // The slot histogram counts every decoded ARMO that has a BO64, not only the
    // ones that re-encode identically: the question is how many records set each
    // slot, and a decode is a decode.
    if sig == *b"ARMO" {
        if let Typed::Armo(armo) = &typed {
            if let Some(bo64) = &armo.biped_slots {
                for (slot, mask) in slots {
                    if bo64.0 & mask != 0 {
                        *armo_slots.entry(slot.clone()).or_insert(0) += 1;
                    }
                }
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

fn check_file(
    path: &Path,
    stats: &mut BTreeMap<String, TypeStats>,
    slots: &[(String, u64)],
    armo_slots: &mut BTreeMap<String, u64>,
) {
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
        let t = stats.entry(key(sig)).or_default();
        t.records += 1;
        let subs: Vec<RawSub<'_>> = record
            .subrecords()
            .expect("decoded")
            .iter()
            .map(RawSub::from)
            .collect();
        let ctx = Ctx::for_record(&record, localized);
        check_record(&name, sig, loc.form_id, &subs, ctx, t, slots, armo_slots);
    }
}

fn report(
    stats: &BTreeMap<String, TypeStats>,
    slots: &[(String, u64)],
    armo_slots: &BTreeMap<String, u64>,
) {
    for (ty, t) in stats {
        println!(
            "apparel {ty}: {} records, {} identical",
            comma(t.records),
            comma(t.identical)
        );
        if !t.unplaced.is_empty() || !t.failed.is_empty() {
            println!("   unplaced: {:?} failed: {:#?}", t.unplaced, t.failed);
        }
        for f in &t.failures {
            println!("   ! {f}");
        }
        for (sig, by_size) in &t.model_flags {
            let total: u64 = by_size.values().sum();
            let sizes: Vec<String> = by_size
                .iter()
                .map(|(n, c)| format!("{n} bytes x{}", comma(*c)))
                .collect();
            println!("   {sig}: {} ({})", comma(total), sizes.join(", "));
        }
        if !t.fld_per_record.is_empty() {
            let per: Vec<String> = t
                .fld_per_record
                .iter()
                .map(|(n, c)| format!("{n} x{}", comma(*c)))
                .collect();
            println!("   FLLD per record: {}", per.join(", "));
        }
    }
    println!("biped slots: ARMO records setting each named slot");
    for (slot, _) in slots {
        println!(
            "   {slot}: {}",
            comma(armo_slots.get(slot).copied().unwrap_or(0))
        );
    }
}

fn assert_clean(stats: &BTreeMap<String, TypeStats>) {
    for (ty, t) in stats {
        assert!(t.unplaced.is_empty(), "{ty}: unplaced {:?}", t.unplaced);
        assert!(t.failed.is_empty(), "{ty}: failed {:?}", t.failed);
        assert_eq!(t.identical, t.records, "{ty}: re-encode differs");
    }
}

#[test]
fn apparel_types_sample() {
    let Some(files) = common::files("apparel_types_sample", &["SFBGS003.esm", "SFBGS004.esm"])
    else {
        return;
    };
    let slots = named_slots();
    let mut stats = BTreeMap::new();
    let mut armo_slots = BTreeMap::new();
    for f in &files {
        check_file(&f.path, &mut stats, &slots, &mut armo_slots);
    }
    report(&stats, &slots, &armo_slots);
    assert_clean(&stats);
    for ty in TYPES {
        assert!(
            stats.contains_key(&key(*ty)),
            "no {} record in the sample files",
            key(*ty)
        );
    }
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn apparel_types_full() {
    let Some(files) = common::all_files("apparel_types_full") else {
        return;
    };
    let slots = named_slots();
    let mut stats = BTreeMap::new();
    let mut armo_slots = BTreeMap::new();
    for f in &files {
        check_file(&f.path, &mut stats, &slots, &mut armo_slots);
    }
    report(&stats, &slots, &armo_slots);
    assert_clean(&stats);
    for ty in TYPES {
        assert!(stats.contains_key(&key(*ty)), "no {} record", key(*ty));
    }
}
