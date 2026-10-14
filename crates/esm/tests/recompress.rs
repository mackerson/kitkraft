//! Measurement, not a gate: how many compressed records `flate2` (default backend,
//! miniz_oxide) recompresses to the exact original bytes, per zlib level. Results go in
//! docs/gaps.md.
//!
//! `cargo test -p kitcraft-esm --release -- --ignored corpus_recompression --nocapture`

// Test-only code: panics fail the measurement, they are not shipped.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::collections::BTreeMap;
use std::sync::Mutex;

use kitcraft_esm::compress::{deflate, inflate};
use kitcraft_esm::driver::visit_file;
use kitcraft_esm::{Body, DecodeLimits, Entry, ParseOptions};

#[derive(Default)]
struct Tally {
    records: u64,
    per_level: [u64; 10],
    any_level: u64,
    headers: BTreeMap<[u8; 2], u64>,
}

impl Tally {
    fn add(&mut self, other: &Tally) {
        self.records += other.records;
        self.any_level += other.any_level;
        for (t, n) in self.per_level.iter_mut().zip(other.per_level) {
            *t += n;
        }
        for (h, n) in &other.headers {
            *self.headers.entry(*h).or_insert(0) += n;
        }
    }
}

fn collect<'e>(entry: &'e Entry<'_>, out: &mut Vec<(u64, &'e [u8])>) {
    match entry {
        Entry::Group(g) => g.children().iter().for_each(|c| collect(c, out)),
        Entry::Record(r) => {
            if let Body::Compressed { original, .. } = r.body() {
                out.push((r.offset().unwrap(), original));
            }
        }
    }
}

fn measure(offset: u64, original: &[u8], tally: &mut Tally) {
    let fields = inflate(original, offset, &DecodeLimits::default()).unwrap();
    tally.records += 1;
    *tally.headers.entry([original[4], original[5]]).or_insert(0) += 1;
    let mut any = false;
    for level in 0..=9u32 {
        let again = deflate(&fields, level, offset).unwrap();
        if again == original {
            tally.per_level[level as usize] += 1;
            any = true;
        }
    }
    if any {
        tally.any_level += 1;
    }
}

/// Measure one top-level entry's compressed records on all cores.
fn measure_entry(entry: &Entry<'_>, total: &mut Tally) {
    let mut records = Vec::new();
    collect(entry, &mut records);
    if records.is_empty() {
        return;
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = records.len().div_ceil(threads);
    let shared = Mutex::new(Tally::default());
    std::thread::scope(|s| {
        for part in records.chunks(chunk) {
            let shared = &shared;
            s.spawn(move || {
                let mut local = Tally::default();
                for (offset, original) in part {
                    measure(*offset, original, &mut local);
                }
                shared.lock().unwrap().add(&local);
            });
        }
    });
    total.add(&shared.into_inner().unwrap());
}

#[test]
#[ignore = "measurement over 2.5 GB of game data; run with --release -- --ignored"]
fn corpus_recompression_parity() {
    let Some(files) = common::all_files("corpus_recompression_parity") else {
        return;
    };
    let mut total = Tally::default();
    println!("| File | Compressed records | Identical at any level | L0 | L1 | L2 | L3 | L4 | L5 | L6 | L7 | L8 | L9 |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for file in files {
        let mut tally = Tally::default();
        visit_file(&file.path, &ParseOptions::default(), |entry| {
            measure_entry(entry, &mut tally);
            Ok(())
        })
        .unwrap();
        let levels: Vec<String> = tally.per_level.iter().map(u64::to_string).collect();
        println!(
            "| {} | {} | {} | {} |",
            file.name,
            tally.records,
            tally.any_level,
            levels.join(" | ")
        );
        total.add(&tally);
    }
    let levels: Vec<String> = total.per_level.iter().map(u64::to_string).collect();
    println!(
        "| total | {} | {} | {} |",
        total.records,
        total.any_level,
        levels.join(" | ")
    );
    println!("zlib header bytes (CMF FLG) -> records:");
    for (h, n) in &total.headers {
        println!("  {:02X} {:02X} -> {n}", h[0], h[1]);
    }
}
