//! M1.7 hostile-input harness, shared by the synthetic and corpus runs in `tests/hostile.rs`.
//!
//! [`check`] parses, decodes, writes and drives one input: nothing may panic, every failure
//! is a typed error whose offset lies inside the input, and anything that parses writes back
//! byte-identically.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

use std::panic::{catch_unwind, AssertUnwindSafe};

use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::index::{index_bytes, read_indexed, IndexOptions};
use kitkraft_esm::{Body, DecodeLimits, Entry, Error, Plugin};

/// Deterministic xorshift64* generator.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
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
pub fn check(bytes: &[u8], what: &str) {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        match Plugin::parse(bytes) {
            Ok(plugin) => {
                assert_eq!(plugin.to_bytes().unwrap(), bytes, "{what}: rewrite differs");
                plugin
                    .entries()
                    .iter()
                    .for_each(|e| decode_all(e, bytes.len(), what));
            }
            Err(e) => assert_in_bounds(&e, bytes.len(), what),
        }
        match roundtrip_bytes(bytes, &RoundtripOptions::with_decode()) {
            Ok(report) => match &report.mismatch {
                // Only a compressed body that does not decode may be a mismatch (M3.A2):
                // the bytes themselves still write back as read.
                Some(m) => {
                    assert!(
                        m.path.contains(" > compressed body: ")
                            && m.expected.is_none()
                            && m.actual.is_none()
                            && m.offset <= bytes.len() as u64,
                        "{what}: driver mismatch {report:?}"
                    );
                    match roundtrip_bytes(bytes, &RoundtripOptions::default()) {
                        Ok(plain) => {
                            assert!(plain.identical(), "{what}: container mismatch {plain:?}")
                        }
                        Err(e) => assert_in_bounds(&e, bytes.len(), what),
                    }
                }
                None => assert!(report.identical(), "{what}: driver mismatch {report:?}"),
            },
            Err(e) => assert_in_bounds(&e, bytes.len(), what),
        }
        // The index validates at least what the parser does, so anything it accepts
        // parses, with the same number of records, and every entry reads back.
        match index_bytes(bytes, &IndexOptions::default()) {
            Ok(index) => {
                let plugin = Plugin::parse(bytes)
                    .unwrap_or_else(|e| panic!("{what}: indexed but does not parse: {e}"));
                assert_eq!(index.records.len() as u64, count_records(&plugin), "{what}");
                let mut cursor = std::io::Cursor::new(bytes);
                for loc in &index.records {
                    if let Err(e) = read_indexed(&mut cursor, loc, Some(&DecodeLimits::default())) {
                        assert_in_bounds(&e, bytes.len(), what);
                    }
                }
            }
            Err(e) => assert_in_bounds(&e, bytes.len(), what),
        }
        if bytes.is_empty() {
            assert!(Plugin::parse(bytes).is_err(), "{what}: empty input parsed");
            assert!(
                roundtrip_bytes(bytes, &RoundtripOptions::default()).is_err(),
                "{what}: empty input roundtripped"
            );
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

fn count_records(plugin: &Plugin<'_>) -> u64 {
    fn walk(entry: &Entry<'_>) -> u64 {
        match entry {
            Entry::Group(g) => g.children().iter().map(walk).sum(),
            Entry::Record(_) => 1,
        }
    }
    1 + plugin.entries().iter().map(walk).sum::<u64>()
}

/// Every offset where a record or group starts or ends (the input must parse).
pub fn boundaries(bytes: &[u8]) -> Vec<usize> {
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

/// `bytes` with bit `bit` flipped.
pub fn flip(bytes: &[u8], bit: u64) -> Vec<u8> {
    let mut mutated = bytes.to_vec();
    mutated[(bit / 8) as usize] ^= 1 << (bit % 8);
    mutated
}
