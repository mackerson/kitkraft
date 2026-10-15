//! Measurement, not a gate: how many compressed records `flate2` (with the workspace's
//! backend, zlib-rs) recompresses to the exact original bytes, per zlib level. Results go in
//! docs/gaps.md.
//!
//! `cargo test -p kitkraft-esm --release -- --ignored corpus_recompression --nocapture`

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

use kitkraft_esm::compress::{deflate, inflate};
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::{Body, DecodeLimits, Entry, ParseOptions};

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

/// One compressed record level 9 does not reproduce (M3.A1).
struct Miss {
    signature: String,
    form_id: u32,
    offset: u64,
    inflated: usize,
    stream: usize,
    /// First byte of the zlib stream (after the `u32` size) where level 9 differs.
    first_diff: usize,
    /// Other levels (0-8) that do reproduce it.
    levels: Vec<u32>,
}

fn collect_records<'e, 'a>(
    entry: &'e Entry<'a>,
    out: &mut Vec<(&'e kitkraft_esm::Record<'a>, &'e [u8])>,
) {
    match entry {
        Entry::Group(g) => g.children().iter().for_each(|c| collect_records(c, out)),
        Entry::Record(r) => {
            if let Body::Compressed { original, .. } = r.body() {
                out.push((r, original));
            }
        }
    }
}

fn find_misses(entry: &Entry<'_>) -> Vec<Miss> {
    let mut records = Vec::new();
    collect_records(entry, &mut records);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = records.len().div_ceil(threads).max(1);
    let found = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for part in records.chunks(chunk) {
            let found = &found;
            s.spawn(move || {
                let mut local = Vec::new();
                for (r, original) in part {
                    let offset = r.offset().unwrap();
                    let fields = inflate(original, offset, &DecodeLimits::default()).unwrap();
                    let again = deflate(&fields, 9, offset).unwrap();
                    if again[..] == original[..] {
                        continue;
                    }
                    let (ours, theirs) = (&again[4..], &original[4..]);
                    let first_diff = ours
                        .iter()
                        .zip(theirs)
                        .position(|(a, b)| a != b)
                        .unwrap_or(ours.len().min(theirs.len()));
                    let levels = (0..9)
                        .filter(|&l| deflate(&fields, l, offset).unwrap() == original[..])
                        .collect();
                    local.push(Miss {
                        signature: r.header.signature.to_string(),
                        form_id: r.header.form_id,
                        offset,
                        inflated: fields.len(),
                        stream: theirs.len(),
                        first_diff,
                        levels,
                    });
                }
                found.lock().unwrap().extend(local);
            });
        }
    });
    found.into_inner().unwrap()
}

/// M3.A1: list every compressed record that zlib-rs level 9 does not reproduce, in file
/// order, with the other levels that do (docs/corpus.md "Recompression misses").
///
/// `cargo test -p kitkraft-esm --release --test recompress -- --ignored corpus_recompression_misses --nocapture`
#[test]
#[ignore = "measurement over 2.5 GB of game data; run with --release -- --ignored"]
fn corpus_recompression_misses() {
    let Some(files) = common::all_files("corpus_recompression_misses") else {
        return;
    };
    println!(
        "| File | Type | FormID | Offset | Inflated | Stream | First differing stream byte | Other levels |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    let mut total = 0usize;
    for file in files {
        let mut misses = Vec::new();
        visit_file(&file.path, &ParseOptions::default(), |entry| {
            misses.extend(find_misses(entry));
            Ok(())
        })
        .unwrap();
        misses.sort_by_key(|m| m.offset);
        for m in &misses {
            let levels: Vec<String> = m.levels.iter().map(u32::to_string).collect();
            println!(
                "| {} | {} | {:08X} | {} | {} | {} | {} | {} |",
                file.name,
                m.signature,
                m.form_id,
                m.offset,
                m.inflated,
                m.stream,
                m.first_diff,
                if levels.is_empty() {
                    "-".to_string()
                } else {
                    levels.join(", ")
                }
            );
        }
        total += misses.len();
    }
    println!("{total} records differ at level 9");
}
