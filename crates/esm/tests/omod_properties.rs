//! Object mod property entries (`common/ObjectModProperty.toml`) against the official ESMs
//! (lane B round 4): every property of every OMOD `DATA` payload and of every object
//! template item (`OBTS`, `common/ObteBlock.toml`) decodes through the struct-field unions
//! decided by its `ValueType` (and, for enum values, its `Property`), and every record
//! holding them re-encodes byte for byte.
//!
//! The checks, per record of the types that hold these entries (OMOD, and the six record
//! types whose definitions use the object template block):
//!
//! - the record decodes with nothing left in `extra` and re-encodes subrecord by subrecord;
//! - every OMOD `DATA` and every `OBTS` payload decodes on its own through its generated
//!   type, and every property has a value type some variant names (no `Other`);
//! - the FormID visitor (`form_ids()`) yields one `ObjectModProperty.Value1` FormID per
//!   property of value type 4 or 6, so remapping and `formid.dangling` see them.
//!
//! The counts the full run prints are quoted in `common/ObjectModProperty.toml`.
//!
//! - `omod_properties_sample` runs in `cargo test` on SFBGS003.esm.
//! - `omod_properties_full` (ignored) runs all 14 files; run it in release:
//!   `cargo test -p kitkraft-esm --release --test omod_properties -- --ignored --nocapture`

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
use std::path::Path;

use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::{
    decode, ObjectModProperty, ObjectModPropertyFunctionType, ObjectModPropertyValue1,
    ObjectModPropertyValue2, ObteBlockCombinationTemplateItem, OmodData, Record as Typed,
};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::FLAG_LOCALIZED;
use kitkraft_esm::DecodeLimits;

/// OMOD, and the record types whose definitions use `common/ObteBlock.toml`.
const TYPES: [&[u8; 4]; 7] = [
    b"OMOD", b"ARMO", b"CONT", b"GBFM", b"NPC_", b"TERM", b"WEAP",
];

#[derive(Default)]
struct Stats {
    records: BTreeMap<String, u64>,
    omod_data: u64,
    obts: u64,
    /// Property entries by where they sit (`OMOD DATA`, `OBTS`).
    properties: BTreeMap<&'static str, u64>,
    by_value_type: BTreeMap<u8, u64>,
    function_type: BTreeMap<&'static str, u64>,
    value1: BTreeMap<&'static str, u64>,
    value2: BTreeMap<&'static str, u64>,
    /// Value type 5 entries by property (four characters when printable).
    enum_properties: BTreeMap<String, u64>,
    form_values: u64,
    form_values_nonzero: u64,
    visited_value1: u64,
    failures: Vec<String>,
}

impl Stats {
    fn fail(&mut self, message: String) {
        if self.failures.len() < 20 {
            self.failures.push(message);
        }
    }
}

fn property_name(p: u32) -> String {
    let b = p.to_le_bytes();
    if b.iter()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        String::from_utf8_lossy(&b).into_owned()
    } else {
        format!("{p:#x}")
    }
}

fn tally(p: &ObjectModProperty, at: &'static str, stats: &mut Stats) {
    *stats.properties.entry(at).or_default() += 1;
    let vt = p.value_type.to_raw();
    *stats.by_value_type.entry(vt).or_default() += 1;
    let ft = match &p.function_type {
        ObjectModPropertyFunctionType::Numeric(_) => "Numeric",
        ObjectModPropertyFunctionType::Bool(_) => "Bool",
        ObjectModPropertyFunctionType::Enum(_) => "Enum",
        ObjectModPropertyFunctionType::Form(_) => "Form",
        ObjectModPropertyFunctionType::Other(_) => "Other",
    };
    *stats.function_type.entry(ft).or_default() += 1;
    let v1 = match &p.value1 {
        ObjectModPropertyValue1::Int(_) => "Int",
        ObjectModPropertyValue1::Float(_) => "Float",
        ObjectModPropertyValue1::Bool(_) => "Bool",
        ObjectModPropertyValue1::String(_) => "String",
        ObjectModPropertyValue1::FormId(id) => {
            stats.form_values += 1;
            if id.0 != 0 {
                stats.form_values_nonzero += 1;
            }
            "FormId"
        }
        ObjectModPropertyValue1::Enum(_) => "Enum",
        ObjectModPropertyValue1::SoundLevel(_) => "SoundLevel",
        ObjectModPropertyValue1::Stagger(_) => "Stagger",
        ObjectModPropertyValue1::HitBehavior(_) => "HitBehavior",
        ObjectModPropertyValue1::Other(_) => "Other",
    };
    *stats.value1.entry(v1).or_default() += 1;
    let v2 = match &p.value2 {
        ObjectModPropertyValue2::Int(_) => "Int",
        ObjectModPropertyValue2::Float(_) => "Float",
        ObjectModPropertyValue2::Bool(_) => "Bool",
        ObjectModPropertyValue2::Unused(_) => "Unused",
        ObjectModPropertyValue2::Other(_) => "Other",
    };
    *stats.value2.entry(v2).or_default() += 1;
    if vt == 5 {
        *stats
            .enum_properties
            .entry(property_name(p.property))
            .or_default() += 1;
    }
}

fn check_file(path: &Path, stats: &mut Stats) {
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
        *stats
            .records
            .entry(String::from_utf8_lossy(&sig).into_owned())
            .or_default() += 1;
        let subs: Vec<RawSub<'_>> = record
            .subrecords()
            .expect("decoded")
            .iter()
            .map(RawSub::from)
            .collect();
        let ctx = Ctx::for_record(&record, localized);
        let what = format!("{name} {} {:08X}", loc.signature, loc.form_id);
        let typed = Typed::decode(sig, &subs, ctx).expect("a definition exists");
        if !typed.extra().is_empty() {
            let sigs: Vec<String> = typed
                .extra()
                .iter()
                .map(|(i, s)| format!("{}@{i}", String::from_utf8_lossy(&s.signature)))
                .collect();
            stats.fail(format!("{what}: not decoded: {}", sigs.join(", ")));
        }
        let encoded = typed.encode();
        let same = encoded.len() == subs.len()
            && encoded.iter().zip(&subs).all(|(e, r)| {
                e.signature == r.signature && e.data == r.data && e.oversized == r.oversized
            });
        if !same {
            stats.fail(format!("{what}: re-encoding differs"));
        }
        let mut form_properties = 0u64;
        for s in &subs {
            let properties = match &s.signature {
                b"DATA" if sig == *b"OMOD" => {
                    stats.omod_data += 1;
                    match decode::<OmodData>(s.data, "OMOD.DATA", ctx) {
                        Ok(d) => Some(("OMOD DATA", d.properties)),
                        Err(e) => {
                            stats.fail(format!("{what}: DATA: {e}"));
                            None
                        }
                    }
                }
                b"OBTS" => {
                    stats.obts += 1;
                    match decode::<ObteBlockCombinationTemplateItem>(s.data, "OBTS", ctx) {
                        Ok(d) => Some(("OBTS", d.properties)),
                        Err(e) => {
                            stats.fail(format!("{what}: OBTS: {e}"));
                            None
                        }
                    }
                }
                _ => None,
            };
            for (at, list) in properties.into_iter() {
                for p in &list {
                    tally(p, at, stats);
                    if matches!(p.value_type.to_raw(), 4 | 6) {
                        form_properties += 1;
                    }
                }
            }
        }
        let visited = typed
            .form_ids()
            .iter()
            .filter(|(path, _)| *path == "ObjectModProperty.Value1")
            .count() as u64;
        stats.visited_value1 += visited;
        if visited != form_properties {
            stats.fail(format!(
                "{what}: {form_properties} FormID properties, {visited} visited"
            ));
        }
    }
}

fn report(stats: &Stats) {
    println!("records: {:?}", stats.records);
    println!(
        "payloads: {} OMOD DATA, {} OBTS; property entries: {:?}",
        stats.omod_data, stats.obts, stats.properties
    );
    println!("by value type: {:?}", stats.by_value_type);
    println!("FunctionType variants: {:?}", stats.function_type);
    println!("Value1 variants: {:?}", stats.value1);
    println!("Value2 variants: {:?}", stats.value2);
    println!("value type 5 by property: {:?}", stats.enum_properties);
    println!(
        "FormID values: {} ({} non-zero); visited as ObjectModProperty.Value1: {}",
        stats.form_values, stats.form_values_nonzero, stats.visited_value1
    );
    for f in &stats.failures {
        println!("FAIL {f}");
    }
}

fn assert_clean(stats: &Stats) {
    assert!(stats.failures.is_empty(), "{:#?}", stats.failures);
    for (what, counts) in [
        ("FunctionType", &stats.function_type),
        ("Value1", &stats.value1),
        ("Value2", &stats.value2),
    ] {
        assert_eq!(
            counts.get("Other").copied().unwrap_or(0),
            0,
            "{what}: a value type no variant names"
        );
    }
    assert_eq!(stats.form_values, stats.visited_value1);
}

#[test]
fn omod_properties_sample() {
    let Some(files) = common::files("omod_properties_sample", &["SFBGS003.esm"]) else {
        return;
    };
    let mut stats = Stats::default();
    for f in &files {
        check_file(&f.path, &mut stats);
    }
    report(&stats);
    assert_clean(&stats);
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn omod_properties_full() {
    let Some(files) = common::all_files("omod_properties_full") else {
        return;
    };
    let mut stats = Stats::default();
    for f in &files {
        check_file(&f.path, &mut stats);
    }
    report(&stats);
    assert_clean(&stats);
    assert!(stats.omod_data > 3000, "{}", stats.omod_data);
    assert!(stats.obts > 0);
}
