//! M1.7 on the three small corpus files: truncation at every record and group boundary and
//! 1,000 seeded single-bit flips per file (seed 0xC0FFEE). Nothing may panic; every failure
//! is a typed error whose offset lies inside the input; anything that parses writes back
//! byte-identically.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};

use kitcraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitcraft_esm::{Body, DecodeLimits, Entry, Error, Plugin};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn in_bounds(err: &Error, len: usize, what: &str) {
    assert!(
        err.offset() <= len as u64,
        "{what}: error offset {} past input length {len}: {err}",
        err.offset()
    );
}

fn decode_all(entry: &Entry<'_>, len: usize, what: &str) {
    match entry {
        Entry::Group(g) => g.children().iter().for_each(|c| decode_all(c, len, what)),
        Entry::Record(r) => {
            if let Body::Compressed { .. } = r.body() {
                if let Err(e) = r.decode(&DecodeLimits::default()) {
                    in_bounds(&e, len, what);
                }
            }
        }
    }
}

fn check(bytes: &[u8], what: &str) {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        match Plugin::parse(bytes) {
            Ok(plugin) => {
                assert_eq!(plugin.to_bytes().unwrap(), bytes, "{what}: rewrite differs");
                plugin
                    .entries()
                    .iter()
                    .for_each(|e| decode_all(e, bytes.len(), what));
            }
            Err(e) => in_bounds(&e, bytes.len(), what),
        }
        match roundtrip_bytes(bytes, &RoundtripOptions::with_decode()) {
            Ok(report) => assert!(report.identical(), "{what}: driver mismatch {report:?}"),
            Err(e) => in_bounds(&e, bytes.len(), what),
        }
    }));
    if let Err(panic) = outcome {
        let msg = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_default();
        panic!("{what}: {msg}");
    }
}

fn boundaries(bytes: &[u8]) -> Vec<usize> {
    fn walk(entry: &Entry<'_>, out: &mut Vec<usize>) {
        let (start, len) = match entry {
            Entry::Group(g) => (g.offset().unwrap(), u64::from(g.size_as_read().unwrap())),
            Entry::Record(r) => (
                r.offset().unwrap(),
                u64::from(r.data_size_as_read().unwrap()) + 24,
            ),
        };
        out.push(start as usize);
        out.push((start + len) as usize);
        if let Entry::Group(g) = entry {
            g.children().iter().for_each(|c| walk(c, out));
        }
    }
    let plugin = Plugin::parse(bytes).unwrap();
    let mut out = vec![0];
    walk(&Entry::Record(plugin.header().clone()), &mut out);
    plugin.entries().iter().for_each(|e| walk(e, &mut out));
    out.sort_unstable();
    out.dedup();
    out
}

#[test]
fn corpus_small_files_survive_truncation_and_bit_flips() {
    let Some(files) = common::files("corpus_small_files_survive_truncation", &common::SMALL) else {
        return;
    };
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        check(&bytes, &file.name);
        let cuts = boundaries(&bytes);
        for &cut in &cuts {
            check(&bytes[..cut], &format!("{} truncated to {cut}", file.name));
        }
        let mut rng = Rng(0xC0FFEE);
        let bits = bytes.len() as u64 * 8;
        let mut parsed = 0;
        for i in 0..1000 {
            let bit = rng.next() % bits;
            let mut mutated = bytes.clone();
            mutated[(bit / 8) as usize] ^= 1 << (bit % 8);
            if Plugin::parse(&mutated).is_ok() {
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
/// and 20,000 random 1-8 byte overwrites per small file.
/// `cargo test -p kitcraft-esm --release --test hostile -- --ignored`
#[test]
#[ignore = "extended mutation run; minutes in release"]
fn corpus_hostile_extended() {
    let Some(files) = common::files("corpus_hostile_extended", &common::SMALL) else {
        return;
    };
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        for cut in 0..bytes.len() {
            check(&bytes[..cut], &format!("{} truncated to {cut}", file.name));
        }
        let mut rng = Rng(0xC0FFEE ^ bytes.len() as u64);
        let bits = bytes.len() as u64 * 8;
        for i in 0..50_000 {
            let bit = rng.next() % bits;
            let mut mutated = bytes.clone();
            mutated[(bit / 8) as usize] ^= 1 << (bit % 8);
            check(&mutated, &format!("{} flip {i} at bit {bit}", file.name));
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
