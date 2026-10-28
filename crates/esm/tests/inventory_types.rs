//! The M4.BX2 inventory definitions (the record types in `TYPES`) against the official ESMs,
//! read in place through the record index (see docs/corpus.md and
//! `crates/schema/defs/README.md`, "Inventory definitions").
//!
//! Every record of the types in `TYPES` is read with [`read_indexed`] (compressed ones
//! decompressed), decoded through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed: the inventory lists every
//!   signature the corpus shows, in corpus order);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload, `XXXX`).
//!
//! It prints, per type, the record count, each signature's record count, occurrences per
//! record and payload sizes, and the distinct subrecord orders (runs collapsed), which the
//! definitions cite.
//!
//! - `inventory_types_sample` runs in `cargo test`: every record of the types in `TYPES` in
//!   `SFBGS003.esm` and `SFBGS004.esm`.
//! - `inventory_types_full` (ignored) runs all 14 files, every record; run it in release:
//!   `cargo test -p kitkraft-esm --release --test inventory_types -- --ignored --nocapture`

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

#[rustfmt::skip]
const TYPES: &[&[u8; 4]] = &[
    b"AACT",
    b"AMBS",
    b"ARTO",
    b"ATMO",
    b"BMOD",
    b"CAMS",
    b"CLMT",
    b"COLL",
    b"CUR3",
    b"DOOR",
    b"FFKW",
    b"IMAD",
    b"IMGS",
    b"IPDS",
    b"IRES",
    b"KSSM",
    b"LAYR",
    b"LIGH",
    b"LMSW",
    b"LSCR",
    b"LTEX",
    b"LVLB",
    b"MATT",
    b"MESG",
    b"MOVT",
    b"MUST",
    b"PGRE",
    b"PHZD",
    b"PTST",
    b"REVB",
    b"RFGP",
    b"SFPC",
    b"SOUN",
    b"SPCH",
    b"TXST",
    b"VOLI",
    b"WKMF",
    b"WWED",
];

#[derive(Default)]
struct SigStats {
    /// Records that have the signature at least once.
    records: u64,
    occurrences: u64,
    /// Most occurrences in one record.
    max_per_record: u64,
    /// Most separate runs in one record (more than one: not consecutive).
    max_runs: u64,
    min_size: usize,
    max_size: usize,
}

#[derive(Default)]
struct TypeStats {
    records: u64,
    compressed: u64,
    subrecords: u64,
    sigs: BTreeMap<String, SigStats>,
    /// Subrecord order with runs collapsed (`A B+ C`) -> records.
    orders: BTreeMap<String, u64>,
    /// (a, b): a run of `a` comes before a run of `b` in some record.
    before: BTreeSet<(String, String)>,
    /// Signatures seen inside BFCB .. BFCE component blocks.
    in_blocks: BTreeSet<String>,
    defined: bool,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    reencode_mismatches: u64,
    failures: Vec<String>,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn inventory(subs: &[RawSub<'_>], t: &mut TypeStats) {
    // A base form component block (BFCB .. BFCE) is one token: its contents are the
    // component's data, kept raw by the shared `BfcbBlock` group.
    // Statistics below are for top-level subrecords (the BFCB and BFCE markers included);
    // block contents are only listed.
    let mut tokens: Vec<String> = Vec::new();
    let mut top: Vec<&RawSub<'_>> = Vec::new();
    let mut in_block = false;
    for s in subs {
        let k = key(s.signature);
        if in_block {
            if k == "BFCE" {
                in_block = false;
                top.push(s);
            } else {
                t.in_blocks.insert(k);
            }
            continue;
        }
        top.push(s);
        if k == "BFCB" {
            in_block = true;
            tokens.push("{BFCB..BFCE}".to_owned());
        } else {
            tokens.push(k);
        }
    }
    let mut runs: Vec<(String, u64)> = Vec::new();
    for k in tokens {
        match runs.last_mut() {
            Some((last, n)) if *last == k => *n += 1,
            _ => runs.push((k, 1)),
        }
    }
    let order: Vec<String> = runs
        .iter()
        .map(|(k, n)| if *n > 1 { format!("{k}+") } else { k.clone() })
        .collect();
    *t.orders.entry(order.join(" ")).or_insert(0) += 1;
    for (i, (a, _)) in runs.iter().enumerate() {
        for (b, _) in runs.iter().skip(i + 1) {
            if a != b {
                t.before.insert((a.clone(), b.clone()));
            }
        }
    }
    // Per top-level signature: occurrences and separate runs.
    let mut per: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut previous: Option<[u8; 4]> = None;
    for s in &top {
        let e = per.entry(key(s.signature)).or_insert((0, 0));
        e.0 += 1;
        if previous != Some(s.signature) {
            e.1 += 1;
        }
        previous = Some(s.signature);
    }
    for s in &top {
        let st = t.sigs.entry(key(s.signature)).or_default();
        if st.occurrences == 0 {
            st.min_size = usize::MAX;
        }
        st.occurrences += 1;
        st.min_size = st.min_size.min(s.data.len());
        st.max_size = st.max_size.max(s.data.len());
    }
    for (k, (n, r)) in per {
        let st = t.sigs.get_mut(&k).unwrap();
        st.records += 1;
        st.max_per_record = st.max_per_record.max(n);
        st.max_runs = st.max_runs.max(r);
    }
}

fn check_record(
    name: &str,
    sig: [u8; 4],
    form_id: u32,
    subs: &[RawSub<'_>],
    ctx: Ctx,
    t: &mut TypeStats,
) {
    t.subrecords += subs.len() as u64;
    inventory(subs, t);
    let Some(typed) = Typed::decode(sig, subs, ctx) else {
        return;
    };
    t.defined = true;
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
    if !same {
        t.reencode_mismatches += 1;
        if t.failures.len() < 20 {
            t.failures
                .push(format!("{name} {form_id:08X}: re-encoding differs"));
        }
    }
}

fn check_file(path: &Path, stats: &mut BTreeMap<String, TypeStats>) {
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
        if loc.is_compressed() {
            t.compressed += 1;
        }
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

fn report(stats: &BTreeMap<String, TypeStats>, orders: usize) {
    for (ty, t) in stats {
        println!(
            "== {ty}: {} records ({} compressed), {} subrecords, {} distinct orders, defined: {}",
            t.records,
            t.compressed,
            t.subrecords,
            t.orders.len(),
            t.defined
        );
        for (sig, s) in &t.sigs {
            println!(
                "   {sig}: in {} records, {} occurrences, <= {} per record, <= {} runs, size {}..{}",
                s.records, s.occurrences, s.max_per_record, s.max_runs, s.min_size, s.max_size
            );
        }
        if !t.in_blocks.is_empty() {
            let inner: Vec<&str> = t.in_blocks.iter().map(String::as_str).collect();
            println!("   inside BFCB..BFCE: {}", inner.join(" "));
        }
        let mut by_count: Vec<(&String, &u64)> = t.orders.iter().collect();
        by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (order, n) in by_count.iter().take(orders) {
            println!("   order x{n}: {order}");
        }
        let conflicts: Vec<String> = t
            .before
            .iter()
            .filter(|(a, b)| t.before.contains(&(b.clone(), a.clone())) && a < b)
            .map(|(a, b)| format!("{a}<->{b}"))
            .collect();
        if !conflicts.is_empty() {
            println!("   both orders seen: {}", conflicts.join(", "));
        }
        for (what, map) in [("unplaced", &t.unplaced), ("failed", &t.failed)] {
            if !map.is_empty() {
                println!("   {what}: {map:?}");
            }
        }
        if t.reencode_mismatches > 0 {
            println!("   re-encode mismatches: {}", t.reencode_mismatches);
        }
        for f in &t.failures {
            println!("   ! {f}");
        }
    }
}

fn assert_clean(stats: &BTreeMap<String, TypeStats>) {
    for (ty, t) in stats {
        assert!(t.defined, "{ty} has no definition");
        assert!(t.unplaced.is_empty(), "{ty}: unplaced {:?}", t.unplaced);
        assert!(t.failed.is_empty(), "{ty}: failed {:?}", t.failed);
        assert_eq!(t.reencode_mismatches, 0, "{ty}: re-encode differs");
    }
}

#[test]
fn inventory_types_sample() {
    let Some(files) = common::files("inventory_types_sample", &["SFBGS003.esm", "SFBGS004.esm"])
    else {
        return;
    };
    let mut stats = BTreeMap::new();
    for f in &files {
        check_file(&f.path, &mut stats);
    }
    report(&stats, 5);
    assert_clean(&stats);
    assert!(stats.values().map(|t| t.records).sum::<u64>() > 0);
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn inventory_types_full() {
    let Some(files) = common::all_files("inventory_types_full") else {
        return;
    };
    let mut stats = BTreeMap::new();
    for f in &files {
        check_file(&f.path, &mut stats);
    }
    report(&stats, 12);
    assert_clean(&stats);
    for ty in TYPES.iter().copied() {
        assert!(stats.contains_key(&key(*ty)), "no {} record", key(*ty));
    }
}
