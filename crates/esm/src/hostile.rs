//! M1.7 on synthetic fixtures: truncations and seeded bit flips never panic, and every
//! failure is a typed error whose offset lies inside the input. The same checks run on the
//! small corpus files in `tests/hostile.rs`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::driver::{roundtrip_bytes, RoundtripOptions};
use crate::fixture::{self, group, record, sub};
use crate::{Body, DecodeLimits, Entry, Error, Plugin};

/// Deterministic xorshift64* generator.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn assert_in_bounds(err: &Error, len: usize, what: &str) {
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
                    assert_in_bounds(&e, len, what);
                }
            }
        }
    }
}

/// Parse, decode, write and drive `bytes`; panics only on a property violation.
pub(crate) fn check(bytes: &[u8], what: &str) {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        match Plugin::parse(bytes) {
            Ok(plugin) => {
                // Anything that parses writes back byte-identically.
                assert_eq!(plugin.to_bytes().unwrap(), bytes, "{what}: rewrite differs");
                plugin
                    .entries()
                    .iter()
                    .for_each(|e| decode_all(e, bytes.len(), what));
            }
            Err(e) => assert_in_bounds(&e, bytes.len(), what),
        }
        match roundtrip_bytes(bytes, &RoundtripOptions::with_decode()) {
            Ok(report) => assert!(report.identical(), "{what}: driver mismatch {report:?}"),
            Err(e) => assert_in_bounds(&e, bytes.len(), what),
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

/// Every offset where a record or group starts or ends.
pub(crate) fn boundaries(bytes: &[u8]) -> Vec<usize> {
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

/// A small fixture with every node kind, including a compressed record and XXXX.
fn small_fixture() -> Vec<u8> {
    let compressed = fixture::compressed_record_stored(b"LAND", 0x500, &sub(b"VHGT", &[7u8; 40]));
    let mut deflated = crate::compress::deflate(&sub(b"DATA", &[1u8; 64]), 6, 0).unwrap();
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
            let mut mutated = bytes.clone();
            mutated[(bit / 8) as usize] ^= 1 << (bit % 8);
            check(&mutated, &format!("{name} flip {i} at bit {bit}"));
        }
    }
}

#[test]
fn every_single_byte_flip_of_the_small_fixture_headers_is_handled() {
    // Exhaustive over the first 400 bytes (TES4 header and the first groups): every bit.
    let bytes = small_fixture();
    for byte in 0..bytes.len().min(400) {
        for bit in 0..8 {
            let mut mutated = bytes.clone();
            mutated[byte] ^= 1 << bit;
            check(&mutated, &format!("byte {byte} bit {bit}"));
        }
    }
}
