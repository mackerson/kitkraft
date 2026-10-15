//! M1.7 hostile-input harness, shared by the synthetic and corpus runs in `tests/hostile.rs`.
//!
//! [`check`] parses, decodes, writes and drives one input: nothing may panic, every failure
//! is a typed error whose offset lies inside the input, and anything that parses writes back
//! byte-identically. [`check_with`] adds a lower bound on error offsets (the first byte a
//! mutation can have changed, so an offset counted from the wrong base shows up as an error
//! before it) and the typed roundtrip (M3.A5).

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

    /// A value in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// One element of `items` (non-empty).
    pub fn pick<'t, T>(&mut self, items: &'t [T]) -> &'t T {
        &items[self.below(items.len() as u64) as usize]
    }
}

/// Extra checks for [`check_with`].
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckOptions {
    /// No error may point before this offset.
    pub lower: u64,
    /// Also run the typed roundtrip (schema decode and re-encode of every defined record).
    pub typed: bool,
}

/// What one input did, for the run statistics.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    /// `Plugin::parse` accepted it.
    pub parsed: bool,
    /// The variant name of the parse error, when it failed.
    pub error: Option<String>,
    /// The strict decode (`roundtrip --decode`) reported a compressed-body fault.
    pub decode_fault: bool,
    /// The variant name of the strict-decode roundtrip's error, when it failed.
    pub decode_error: Option<String>,
}

/// The variant name of `err` (`UnexpectedEof`, `InCompressed/UnexpectedEof`, ...).
pub fn variant(err: &Error) -> String {
    let debug = format!("{err:?}");
    let name: String = debug
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    match err {
        Error::InCompressed { source, .. } => format!("{name}/{}", variant(source)),
        _ => name,
    }
}

struct Bounds<'w> {
    lower: u64,
    len: usize,
    what: &'w str,
}

impl Bounds<'_> {
    fn check(&self, err: &Error) {
        let (what, len) = (self.what, self.len);
        assert!(
            err.offset() <= len as u64,
            "{what}: error offset {} past input length {len}: {err}",
            err.offset()
        );
        assert!(
            err.offset() >= self.lower,
            "{what}: error offset {} before the first mutated entry at {}: {err}",
            err.offset(),
            self.lower
        );
        assert!(
            !matches!(err, Error::Internal { .. }),
            "{what}: internal error (a bug): {err}"
        );
    }
}

fn decode_all(entry: &Entry<'_>, bounds: &Bounds<'_>) {
    match entry {
        Entry::Group(g) => g.children().iter().for_each(|c| decode_all(c, bounds)),
        Entry::Record(r) => {
            if let Body::Compressed { .. } = r.body() {
                if let Err(e) = r.decode(&DecodeLimits::default()) {
                    bounds.check(&e);
                }
            }
        }
    }
}

/// Parse, decode, write and drive `bytes`; panics only on a property violation.
pub fn check(bytes: &[u8], what: &str) {
    check_with(bytes, what, &CheckOptions::default());
}

/// [`check`] with the extra checks of `options`; returns what the input did.
pub fn check_with(bytes: &[u8], what: &str, options: &CheckOptions) -> Outcome {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let bounds = Bounds {
            lower: options.lower,
            len: bytes.len(),
            what,
        };
        let mut out = Outcome::default();
        match Plugin::parse(bytes) {
            Ok(plugin) => {
                out.parsed = true;
                assert_eq!(plugin.to_bytes().unwrap(), bytes, "{what}: rewrite differs");
                plugin.entries().iter().for_each(|e| decode_all(e, &bounds));
                // Lossless even for weird-but-parseable input: the plain container roundtrip
                // succeeds and matches byte for byte.
                match roundtrip_bytes(bytes, &RoundtripOptions::default()) {
                    Ok(plain) => assert!(plain.identical(), "{what}: plain mismatch {plain:?}"),
                    Err(e) => panic!("{what}: parses, but the plain roundtrip fails: {e}"),
                }
            }
            Err(e) => {
                bounds.check(&e);
                out.error = Some(variant(&e));
            }
        }
        if options.typed {
            let typed = RoundtripOptions {
                typed: true,
                ..RoundtripOptions::with_decode()
            };
            match roundtrip_bytes(bytes, &typed) {
                Ok(report) => assert!(
                    report
                        .mismatch
                        .as_ref()
                        .is_none_or(|m| m.path.contains(" > compressed body: ")),
                    "{what}: typed roundtrip mismatch {report:?}"
                ),
                Err(e) => bounds.check(&e),
            }
        }
        match roundtrip_bytes(bytes, &RoundtripOptions::with_decode()) {
            Ok(report) => match &report.mismatch {
                // Only a compressed body that does not decode may be a mismatch (M3.A2):
                // the bytes themselves still write back as read.
                Some(m) => {
                    out.decode_fault = true;
                    assert!(
                        m.path.contains(" > compressed body: ")
                            && m.expected.is_none()
                            && m.actual.is_none()
                            && m.offset <= bytes.len() as u64
                            && m.offset >= bounds.lower,
                        "{what}: driver mismatch {report:?}"
                    );
                    match roundtrip_bytes(bytes, &RoundtripOptions::default()) {
                        Ok(plain) => {
                            assert!(plain.identical(), "{what}: container mismatch {plain:?}")
                        }
                        Err(e) => bounds.check(&e),
                    }
                }
                None => assert!(report.identical(), "{what}: driver mismatch {report:?}"),
            },
            Err(e) => {
                bounds.check(&e);
                out.decode_error = Some(variant(&e));
            }
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
                        bounds.check(&e);
                    }
                }
            }
            Err(e) => bounds.check(&e),
        }
        if bytes.is_empty() {
            assert!(Plugin::parse(bytes).is_err(), "{what}: empty input parsed");
            assert!(
                roundtrip_bytes(bytes, &RoundtripOptions::default()).is_err(),
                "{what}: empty input roundtripped"
            );
        }
        out
    }));
    match outcome {
        Ok(out) => out,
        Err(panic) => {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_default();
            panic!("{what}: {msg}");
        }
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
