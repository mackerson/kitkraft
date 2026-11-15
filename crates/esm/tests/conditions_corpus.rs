//! Conditions (`CTDA`) of the official ESMs against the typed condition (KK-269, KK-270;
//! `docs/design/conditions.md`), read in place through the record index.
//!
//! Every 32-byte `CTDA` payload of every record of the 14 files is decoded through the
//! generated `ConditionData` and written back. The checks:
//!
//! - every payload re-encodes byte for byte (the type byte's operator and flags split and
//!   the named function keep every bit);
//! - every comparison operator is one the definition names (0 to 5);
//! - every function index is one the pinned xEdit table names; an index it does not name
//!   is printed with its count, and the list is pinned here (`UNNAMED`).
//!
//! It prints the operator and flag tallies and the number of distinct functions.
//! `conditions_corpus_full` (ignored) runs all 14 files; run it in release:
//! `cargo test -p kitkraft-esm --release --test conditions_corpus -- --ignored --nocapture`

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
use std::fs::File;
use std::io::BufReader;

use kitkraft_esm::bridge::Ctx;
use kitkraft_esm::generated::records::{ConditionData, ConditionDataFunction, Field, Reader};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::DecodeLimits;

/// Function indices of official conditions the pinned table does not name (none).
const UNNAMED: &[u16] = &[];

#[derive(Default)]
struct Tally {
    conditions: u64,
    identical: u64,
    operators: BTreeMap<u8, u64>,
    flags: BTreeMap<u8, u64>,
    functions: BTreeMap<u16, u64>,
    unnamed: BTreeMap<u16, u64>,
    failures: Vec<String>,
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn conditions_corpus_full() {
    let Some(files) = common::all_files("conditions_corpus_full") else {
        return;
    };
    let mut t = Tally::default();
    let limits = DecodeLimits::default();
    for f in &files {
        let name = f.path.file_name().unwrap().to_string_lossy().into_owned();
        let index = index_file(&f.path, &IndexOptions::default()).unwrap();
        let mut input = BufReader::new(File::open(&f.path).unwrap());
        for loc in &index.records {
            let record = read_indexed(&mut input, loc, Some(&limits)).unwrap();
            let Some(subs) = record.subrecords() else {
                continue;
            };
            let ctx = Ctx::for_record(&record, false);
            for s in subs.iter().filter(|s| s.signature().0 == *b"CTDA") {
                let data = s.data();
                t.conditions += 1;
                let mut r = Reader::new(data);
                let decoded = match ConditionData::read(&mut r, "CTDA", ctx) {
                    Ok(d) if r.is_empty() => d,
                    other => {
                        if t.failures.len() < 20 {
                            t.failures.push(format!(
                                "{name} {:08X}: CTDA ({} bytes) does not decode: {other:?}",
                                loc.form_id,
                                data.len()
                            ));
                        }
                        continue;
                    }
                };
                let mut out = Vec::new();
                decoded.write(&mut out);
                if out == data {
                    t.identical += 1;
                } else if t.failures.len() < 20 {
                    t.failures.push(format!(
                        "{name} {:08X}: CTDA re-encodes differently",
                        loc.form_id
                    ));
                }
                *t.operators.entry(decoded.type_.operator()).or_default() += 1;
                for bit in 0..5u8 {
                    if decoded.type_.flags() & (1 << bit) != 0 {
                        *t.flags.entry(1 << bit).or_default() += 1;
                    }
                }
                let index = decoded.function.to_raw();
                *t.functions.entry(index).or_default() += 1;
                if matches!(decoded.function, ConditionDataFunction::Other(_)) {
                    *t.unnamed.entry(index).or_default() += 1;
                }
            }
        }
    }
    println!(
        "conditions: {} CTDA, {} identical, {} distinct functions",
        t.conditions,
        t.identical,
        t.functions.len()
    );
    println!("   operators (top three bits): {:?}", t.operators);
    println!("   flags (low five bits, by mask): {:?}", t.flags);
    println!(
        "   function indices the table does not name: {:?}",
        t.unnamed
    );
    for f in &t.failures {
        println!("   ! {f}");
    }
    assert!(t.failures.is_empty(), "{:#?}", t.failures);
    assert_eq!(t.identical, t.conditions);
    assert!(
        t.operators.keys().all(|op| *op <= 5),
        "an operator without a name: {:?}",
        t.operators
    );
    let unnamed: Vec<u16> = t.unnamed.keys().copied().collect();
    assert_eq!(unnamed, UNNAMED, "function indices the table does not name");
}
