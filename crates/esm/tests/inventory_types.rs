//! The M4.BX2 inventory definitions (the record types in `TYPES`) against the official ESMs,
//! read in place through the record index (see docs/corpus.md and
//! `crates/schema/defs/README.md`, "Inventory definitions").
//!
//! Every record of the types in `TYPES` is read through the record index
//! (`kitkraft_esm::inventory::scan_file`, compressed ones decompressed), decoded through its
//! generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed: the inventory lists every
//!   signature the corpus shows, in corpus order);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload, `XXXX`).
//!
//! It prints, per type, the record count, each signature's record count, occurrences per
//! record and payload sizes, and the distinct subrecord orders (runs collapsed), which the
//! definitions cite. The statistics are `kitkraft_esm::inventory::TypeStats`, the same scan
//! `cargo xtask inventory-def` writes a definition from and `cargo xtask inventory-check`
//! compares a definition's cited counts with.
//!
//! `KK_INVENTORY_ONLY=CURV,REGN` limits both tests and their printout to those types (each
//! must be in `TYPES`); `cargo xtask add-def` adds a type to `TYPES`.
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

use std::collections::BTreeMap;
use std::path::Path;

use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::{match_body, Record as Typed, RECORDS};
use kitkraft_esm::inventory::{key, scan_file, TypeStats};

/// Limits the run and its printout to the listed types: `KK_INVENTORY_ONLY=CURV,REGN`.
const ONLY_VAR: &str = "KK_INVENTORY_ONLY";

#[rustfmt::skip]
const TYPES: &[&[u8; 4]] = &[
    b"AACT",
    b"AAMD",
    b"AAPD",
    b"ADDN",
    b"AMBS",
    b"AOPF",
    b"AOPS",
    b"ARTO",
    b"ATMO",
    b"BMMO",
    b"BMOD",
    b"CAMS",
    b"CLDF",
    b"CLMT",
    b"COLL",
    b"CSTY",
    b"CUR3",
    b"DEBR",
    b"DLBR",
    b"DOBJ",
    b"DOOR",
    b"EFSH",
    b"EQUP",
    b"FFKW",
    b"FOGV",
    b"FORC",
    b"FSTP",
    b"GBFT",
    b"GCVR",
    b"GPOG",
    b"GRAS",
    b"GWED",
    b"IMAD",
    b"IMGS",
    b"IPDS",
    b"IRES",
    b"KSSM",
    b"LAYR",
    b"LENS",
    b"LGDI",
    b"LGTM",
    b"LIGH",
    b"LMSW",
    b"LSCR",
    b"LTEX",
    b"LVLB",
    b"MAAM",
    b"MATT",
    b"MESG",
    b"MOVT",
    b"MUSC",
    b"MUST",
    b"NAVI",
    b"OSWP",
    b"PERS",
    b"PGRE",
    b"PHZD",
    b"PSDC",
    b"PTST",
    b"REVB",
    b"RFGP",
    b"SDLT",
    b"SECH",
    b"SFPC",
    b"SOUN",
    b"SPCH",
    b"STAG",
    b"STBH",
    b"STND",
    b"TXST",
    b"VOLI",
    b"WBAR",
    b"WKMF",
    b"WTHR",
    b"WTHS",
    b"WWED",
];

/// The inventory of one type (shared with `cargo xtask inventory-def` and
/// `inventory-check`, `kitkraft_esm::inventory`) and what checking its records found.
#[derive(Default)]
struct TypeCheck {
    stats: TypeStats,
    defined: bool,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    reencode_mismatches: u64,
    failures: Vec<String>,
}

/// `TYPES`, or the ones `KK_INVENTORY_ONLY` names (`CURV,REGN`; each must be in `TYPES`):
/// the run and its printout are limited to them.
fn selected() -> Vec<[u8; 4]> {
    let Ok(only) = std::env::var(ONLY_VAR) else {
        return TYPES.iter().map(|t| **t).collect();
    };
    let mut out = Vec::new();
    for name in only.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let sig: [u8; 4] = name
            .as_bytes()
            .try_into()
            .unwrap_or_else(|_| panic!("{ONLY_VAR}: `{name}` is not a 4-character signature"));
        assert!(
            TYPES.contains(&&sig),
            "{ONLY_VAR}: {name} is not in TYPES (add it with `cargo xtask add-def {name}`)"
        );
        out.push(sig);
    }
    assert!(!out.is_empty(), "{ONLY_VAR} names no type");
    out
}

fn check_record(
    name: &str,
    sig: [u8; 4],
    form_id: u32,
    subs: &[RawSub<'_>],
    ctx: Ctx,
    t: &mut TypeCheck,
) {
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

fn check_file(path: &Path, types: &[[u8; 4]], stats: &mut BTreeMap<String, TypeCheck>) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    scan_file(
        path,
        |sig| types.contains(&sig.bytes()),
        |v| {
            let sig = v.loc.signature.bytes();
            let t = stats.entry(key(sig)).or_default();
            t.stats.add(v.subs, v.loc.is_compressed());
            let ctx = Ctx::for_record(v.record, v.localized);
            check_record(&name, sig, v.loc.form_id, v.subs, ctx, t);
        },
    )
    .unwrap();
}

fn report(stats: &BTreeMap<String, TypeCheck>, orders: usize) {
    for (ty, t) in stats {
        let s = &t.stats;
        println!(
            "== {ty}: {} records ({} compressed), {} subrecords, {} distinct orders, defined: {}",
            s.records,
            s.compressed,
            s.subrecords,
            s.orders.len(),
            t.defined
        );
        for line in s.report_lines(orders) {
            println!("{line}");
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

fn assert_clean(stats: &BTreeMap<String, TypeCheck>) {
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
    let types = selected();
    let mut stats = BTreeMap::new();
    for f in &files {
        check_file(&f.path, &types, &mut stats);
    }
    report(&stats, 5);
    assert_clean(&stats);
    if std::env::var_os(ONLY_VAR).is_none() {
        assert!(stats.values().map(|t| t.stats.records).sum::<u64>() > 0);
    }
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn inventory_types_full() {
    let Some(files) = common::all_files("inventory_types_full") else {
        return;
    };
    let types = selected();
    let mut stats = BTreeMap::new();
    for f in &files {
        check_file(&f.path, &types, &mut stats);
    }
    report(&stats, 12);
    assert_clean(&stats);
    for ty in &types {
        assert!(stats.contains_key(&key(*ty)), "no {} record", key(*ty));
    }
}
