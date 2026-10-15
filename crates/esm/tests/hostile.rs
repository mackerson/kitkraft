//! M1.7: hostile input never panics. One harness (`common::hostile::check`) runs on
//!
//! - synthetic fixtures: every truncation, every bit of the first 400 bytes, and 1,000
//!   seeded bit flips;
//! - the three tiny corpus files (`common::TINY`): truncation at every record and group
//!   boundary and 1,000 seeded single-bit flips per file (seed 0xC0FFEE).
//!
//! Nothing may panic; every failure is a typed error whose offset lies inside the input;
//! anything that parses writes back byte-identically.
//!
//! M3.A5 adds structure-aware mutation (`common::mutate`): seeded changes to size fields,
//! `XXXX` values, decompressed-size prefixes, flags, form versions, group labels and types,
//! plus size-consistent splices (duplicated or dropped subrecords and entries, extra group
//! nesting, recompressed bodies), over both synthetic fixtures and the five small corpus
//! files (`common::SMALL`). `cargo test` runs a reduced count; the full run (2,000 per
//! input) is `cargo test -p kitkraft-esm --release --test hostile -- --ignored mutation`.
//! Every mutated input also goes through the typed roundtrip, and no error may point
//! before the first top-level entry the mutation touched.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

// The crate's own test fixtures (`src/fixture.rs`), so both runs share one builder.
#[allow(dead_code)]
#[path = "../src/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use common::hostile::{boundaries, check, check_with, flip, CheckOptions, Rng};
use common::mutate::{field_name, Mutator};
use fixture::{group, record, sub};

/// Mutations per input in `cargo test` (debug) and in the `--ignored --release` run.
const QUICK_SYNTHETIC: usize = 400;
const QUICK_CORPUS: usize = 100;
const FULL: usize = 2_000;

/// What one mutation run saw.
#[derive(Default)]
struct RunStats {
    inputs: usize,
    parsed: usize,
    decode_faults: usize,
    errors: BTreeMap<String, usize>,
    decode_errors: BTreeMap<String, usize>,
    kinds: BTreeMap<&'static str, usize>,
    slowest: (Duration, String),
    elapsed: Duration,
}

impl std::fmt::Display for RunStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} mutations in {:.2} s: {} parse, {} strict-decode faults; parse errors {:?}; \
             strict-decode errors after a parse {:?}; kinds {:?}; slowest {:.1} ms ({})",
            self.inputs,
            self.elapsed.as_secs_f64(),
            self.parsed,
            self.decode_faults,
            self.errors,
            self.decode_errors,
            self.kinds,
            self.slowest.0.as_secs_f64() * 1000.0,
            self.slowest.1
        )
    }
}

/// `count` seeded structure-aware mutations of `bytes`, each through the full harness.
fn mutation_run(name: &str, bytes: &[u8], count: usize, seed: u64) -> RunStats {
    let mutator = Mutator::new(bytes);
    let mut rng = Rng::new(seed);
    let mut stats = RunStats::default();
    let start = Instant::now();
    for i in 0..count {
        let m = mutator.mutate(&mut rng);
        let what = format!("{name} mutation {i} (seed {seed:#x}): {}", m.description);
        let one = Instant::now();
        let outcome = check_with(
            &m.bytes,
            &what,
            &CheckOptions {
                lower: m.lower,
                typed: true,
            },
        );
        let took = one.elapsed();
        if took > stats.slowest.0 {
            stats.slowest = (took, format!("{i}: {}", m.description));
        }
        stats.inputs += 1;
        stats.parsed += usize::from(outcome.parsed);
        stats.decode_faults += usize::from(outcome.decode_fault);
        if let Some(e) = outcome.error {
            *stats.errors.entry(e).or_default() += 1;
        }
        if let (true, Some(e)) = (outcome.parsed, outcome.decode_error) {
            *stats.decode_errors.entry(e).or_default() += 1;
        }
        for k in m.kinds {
            *stats.kinds.entry(k).or_default() += 1;
        }
    }
    stats.elapsed = start.elapsed();
    println!("{name}: {stats}");
    stats
}

fn synthetic_mutations(count: usize) {
    for (name, bytes) in [
        ("small fixture", small_fixture()),
        ("sample", fixture::sample_plugin()),
    ] {
        let stats = mutation_run(name, &bytes, count, 0xA5A5_0000 ^ bytes.len() as u64);
        // Every field kind the fixture has, and every splice, was exercised.
        let mut expected: Vec<&str> = Mutator::new(&bytes)
            .field_counts()
            .into_iter()
            .map(|(f, _)| field_name(f))
            .collect();
        expected.extend(["dup-subrecord", "drop-subrecord", "dup-entry", "drop-entry"]);
        expected.push("wrap-groups");
        for kind in expected {
            assert!(
                stats.kinds.contains_key(kind),
                "{name}: no {kind} in {:?}",
                stats.kinds
            );
        }
    }
}

/// Mutations per input in the full run: `KITKRAFT_MUTATIONS`, else [`FULL`].
fn full_count() -> usize {
    std::env::var("KITKRAFT_MUTATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(FULL)
}

fn corpus_mutations(test: &str, count: usize) {
    let Some(files) = common::files(test, &common::SMALL) else {
        return;
    };
    let start = Instant::now();
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        let sites: usize = Mutator::new(&bytes)
            .field_counts()
            .iter()
            .map(|(_, n)| n)
            .sum();
        println!("{}: {sites} field sites", file.name);
        mutation_run(&file.name, &bytes, count, 0xA5A5_0000 ^ bytes.len() as u64);
    }
    println!(
        "{test}: {} files in {:.1} s; peak working set {:?} bytes",
        common::SMALL.len(),
        start.elapsed().as_secs_f64(),
        common::peak_working_set()
    );
}

#[test]
fn structure_aware_mutations_of_synthetic_fixtures() {
    synthetic_mutations(QUICK_SYNTHETIC);
}

#[test]
fn structure_aware_mutations_of_small_corpus_files() {
    corpus_mutations(
        "structure_aware_mutations_of_small_corpus_files",
        QUICK_CORPUS,
    );
}

/// The full M3.A5 run: 2,000 mutations (or `KITKRAFT_MUTATIONS`) per synthetic fixture and
/// per small corpus file.
/// `cargo test -p kitkraft-esm --release --test hostile -- --ignored mutation --nocapture`
#[test]
#[ignore = "full structure-aware mutation run; seconds to minutes in release"]
fn structure_aware_mutation_full_run() {
    let count = full_count();
    synthetic_mutations(count);
    corpus_mutations("structure_aware_mutation_full_run", count);
}

/// A small fixture with every node kind, including two compressed records and XXXX.
fn small_fixture() -> Vec<u8> {
    let compressed = fixture::compressed_record_stored(b"LAND", 0x500, &sub(b"VHGT", &[7u8; 40]));
    let mut deflated = kitkraft_esm::compress::deflate(&sub(b"DATA", &[1u8; 64]), 6, 0).unwrap();
    let decl = u32::from_le_bytes(deflated[..4].try_into().unwrap());
    let stream = deflated.split_off(4);
    let compressed2 = fixture::compressed_record_from_stream(b"LAND", 0x501, decl, &stream);
    let xxxx = record(
        b"NAVM",
        0,
        0x600,
        &[fixture::xxxx_sub(b"NVNM", &[3u8; 20]), sub(b"EDID", b"n\0")].concat(),
    );
    let refr = record(b"REFR", 0x400, 0x201, &sub(b"NAME", &[0, 3, 0, 0]));
    let cell_children = group(
        &0x200u32.to_le_bytes(),
        6,
        &group(&0x200u32.to_le_bytes(), 9, &refr),
    );
    let cell = record(b"CELL", 0, 0x200, &sub(b"EDID", b"c\0"));
    fixture::plugin(&[
        group(
            b"GMST",
            0,
            &record(b"GMST", 0, 0x100, &sub(b"EDID", b"fX\0")),
        ),
        group(b"CELL", 0, &[cell, cell_children].concat()),
        group(b"LAND", 0, &[compressed, compressed2].concat()),
        group(b"NAVM", 0, &xxxx),
    ])
}

#[test]
fn every_truncation_of_a_small_fixture_is_handled() {
    let bytes = small_fixture();
    check(&bytes, "untouched");
    for len in 0..bytes.len() {
        check(&bytes[..len], &format!("truncated to {len}"));
    }
}

#[test]
fn truncation_at_every_record_boundary_of_the_sample() {
    let bytes = fixture::sample_plugin();
    for cut in boundaries(&bytes) {
        check(&bytes[..cut], &format!("sample truncated to {cut}"));
    }
}

#[test]
fn seeded_bit_flips_are_handled() {
    for (name, bytes) in [
        ("small fixture", small_fixture()),
        ("sample", fixture::sample_plugin()),
    ] {
        let mut rng = Rng::new(0xC0FFEE);
        let bits = bytes.len() as u64 * 8;
        for i in 0..1000 {
            let bit = rng.next() % bits;
            check(&flip(&bytes, bit), &format!("{name} flip {i} at bit {bit}"));
        }
    }
}

#[test]
fn every_single_byte_flip_of_the_small_fixture_headers_is_handled() {
    // Exhaustive over the first 400 bytes (TES4 header and the first groups): every bit.
    let bytes = small_fixture();
    for byte in 0..bytes.len().min(400) {
        for bit in 0..8 {
            check(
                &flip(&bytes, (byte * 8 + bit) as u64),
                &format!("byte {byte} bit {bit}"),
            );
        }
    }
}

#[test]
fn corpus_small_files_survive_truncation_and_bit_flips() {
    let Some(files) = common::files("corpus_small_files_survive_truncation", &common::TINY) else {
        return;
    };
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        check(&bytes, &file.name);
        let cuts = boundaries(&bytes);
        for &cut in &cuts {
            check(&bytes[..cut], &format!("{} truncated to {cut}", file.name));
        }
        let mut rng = Rng::new(0xC0FFEE);
        let bits = bytes.len() as u64 * 8;
        let mut parsed = 0;
        for i in 0..1000 {
            let bit = rng.next() % bits;
            let mutated = flip(&bytes, bit);
            if kitkraft_esm::Plugin::parse(&mutated).is_ok() {
                parsed += 1;
            }
            check(&mutated, &format!("{} flip {i} at bit {bit}", file.name));
        }
        println!(
            "{}: {} boundary truncations, 1000 bit flips ({parsed} still parse)",
            file.name,
            cuts.len()
        );
    }
}

/// A longer run for occasional use: every byte-offset truncation, 50,000 single-bit flips
/// and 20,000 random 1-8 byte overwrites per tiny file.
/// `cargo test -p kitkraft-esm --release --test hostile -- --ignored`
#[test]
#[ignore = "extended mutation run; minutes in release"]
fn corpus_hostile_extended() {
    let Some(files) = common::files("corpus_hostile_extended", &common::TINY) else {
        return;
    };
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        for cut in 0..bytes.len() {
            check(&bytes[..cut], &format!("{} truncated to {cut}", file.name));
        }
        let mut rng = Rng::new(0xC0FFEE ^ bytes.len() as u64);
        let bits = bytes.len() as u64 * 8;
        for i in 0..50_000 {
            let bit = rng.next() % bits;
            check(
                &flip(&bytes, bit),
                &format!("{} flip {i} at bit {bit}", file.name),
            );
        }
        for i in 0..20_000 {
            let at = (rng.next() % bytes.len() as u64) as usize;
            let n = 1 + (rng.next() % 8) as usize;
            let mut mutated = bytes.clone();
            for b in mutated.iter_mut().skip(at).take(n) {
                *b = rng.next() as u8;
            }
            check(
                &mutated,
                &format!("{} overwrite {i} at {at}+{n}", file.name),
            );
        }
        println!("{}: extended run clean", file.name);
    }
}
