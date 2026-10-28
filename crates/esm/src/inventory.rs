//! Corpus inventory of record types (M4.BX2): which top-level subrecords the records of a
//! type carry, how often, how large, and in which orders.
//!
//! [`TypeStats::add`] folds one record's subrecords into its type's statistics, and
//! [`scan_file`] visits every record of the wanted types in a plugin (compressed ones
//! decompressed). One scan feeds every consumer, so the inventory definitions cite the
//! numbers the check reads:
//!
//! - `tests/inventory_types.rs` prints [`TypeStats::report_lines`] and checks each record
//!   against its generated type;
//! - `examples/inventory_scan.rs` writes the statistics as TOML for `cargo xtask
//!   inventory-def` (which writes a definition from them) and `cargo xtask inventory-check`
//!   (which compares a definition's cited counts with them).
//!
//! A base form component block (`BFCB` .. `BFCE`) is one token ([`BLOCK_TOKEN`]) in the
//! orders: its contents are the component's data, kept raw by the shared `BfcbBlock` group.
//! The statistics are for top-level subrecords (the `BFCB` and `BFCE` markers included);
//! block contents are only listed ([`TypeStats::in_blocks`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::bridge::RawSub;
use crate::index::{index_file, read_indexed, IndexOptions, RecordLoc};
use crate::model::Record;
use crate::tes4::FLAG_LOCALIZED;
use crate::{DecodeLimits, Error, Result, Signature};

/// The token a whole `BFCB` .. `BFCE` component block stands as in [`TypeStats::orders`].
pub const BLOCK_TOKEN: &str = "{BFCB..BFCE}";

/// One top-level signature of a record type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SigStats {
    /// Records that have the signature at least once.
    pub records: u64,
    /// Occurrences in all records.
    pub occurrences: u64,
    /// Most occurrences in one record.
    pub max_per_record: u64,
    /// Most separate runs in one record (more than one: not consecutive).
    pub max_runs: u64,
    /// Smallest payload, in bytes.
    pub min_size: usize,
    /// Largest payload, in bytes.
    pub max_size: usize,
}

/// The inventory of one record type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeStats {
    /// Records seen.
    pub records: u64,
    /// Of those, records stored compressed.
    pub compressed: u64,
    /// Subrecords in all records, block contents included.
    pub subrecords: u64,
    /// Per top-level signature.
    pub sigs: BTreeMap<String, SigStats>,
    /// Subrecord order with runs collapsed (`A B+ C`) -> records.
    pub orders: BTreeMap<String, u64>,
    /// (a, b): a run of `a` comes before a run of `b` in some record.
    pub before: BTreeSet<(String, String)>,
    /// Signatures seen inside `BFCB` .. `BFCE` component blocks.
    pub in_blocks: BTreeSet<String>,
}

/// A signature as text (lossy for non-UTF-8 bytes).
pub fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

impl TypeStats {
    /// Folds one record's subrecords (after `XXXX` resolution) into the statistics.
    pub fn add(&mut self, subs: &[RawSub<'_>], compressed: bool) {
        self.records = self.records.saturating_add(1);
        if compressed {
            self.compressed = self.compressed.saturating_add(1);
        }
        let count = u64::try_from(subs.len()).unwrap_or(u64::MAX);
        self.subrecords = self.subrecords.saturating_add(count);

        // Tokens: top-level signatures, a component block as one.
        let mut tokens: Vec<String> = Vec::new();
        let mut top: Vec<&RawSub<'_>> = Vec::new();
        let mut in_block = false;
        for s in subs {
            let k = key(s.signature);
            if in_block {
                if k == "BFCE" {
                    in_block = false;
                    top.push(s);
                } else {
                    self.in_blocks.insert(k);
                }
                continue;
            }
            top.push(s);
            if k == "BFCB" {
                in_block = true;
                tokens.push(BLOCK_TOKEN.to_owned());
            } else {
                tokens.push(k);
            }
        }
        let mut runs: Vec<(String, u64)> = Vec::new();
        for k in tokens {
            match runs.last_mut() {
                Some((last, n)) if *last == k => *n = n.saturating_add(1),
                _ => runs.push((k, 1)),
            }
        }
        let order: Vec<String> = runs
            .iter()
            .map(|(k, n)| if *n > 1 { format!("{k}+") } else { k.clone() })
            .collect();
        let seen = self.orders.entry(order.join(" ")).or_insert(0);
        *seen = seen.saturating_add(1);
        for (i, (a, _)) in runs.iter().enumerate() {
            for (b, _) in runs.iter().skip(i.saturating_add(1)) {
                if a != b {
                    self.before.insert((a.clone(), b.clone()));
                }
            }
        }

        // Per top-level signature: occurrences and separate runs in this record.
        let mut per: BTreeMap<String, (u64, u64)> = BTreeMap::new();
        let mut previous: Option<[u8; 4]> = None;
        for s in &top {
            let e = per.entry(key(s.signature)).or_insert((0, 0));
            e.0 = e.0.saturating_add(1);
            if previous != Some(s.signature) {
                e.1 = e.1.saturating_add(1);
            }
            previous = Some(s.signature);
        }
        for s in &top {
            let st = self.sigs.entry(key(s.signature)).or_default();
            if st.occurrences == 0 {
                st.min_size = usize::MAX;
            }
            st.occurrences = st.occurrences.saturating_add(1);
            st.min_size = st.min_size.min(s.data.len());
            st.max_size = st.max_size.max(s.data.len());
        }
        for (k, (n, r)) in per {
            let st = self.sigs.entry(k).or_default();
            st.records = st.records.saturating_add(1);
            st.max_per_record = st.max_per_record.max(n);
            st.max_runs = st.max_runs.max(r);
        }
    }

    /// Pairs of tokens seen in both orders (`a < b`).
    pub fn conflicts(&self) -> Vec<(String, String)> {
        self.before
            .iter()
            .filter(|(a, b)| a < b && self.before.contains(&(b.clone(), a.clone())))
            .cloned()
            .collect()
    }

    /// The report lines the inventory test prints under a type's header line: each
    /// signature's counts and sizes, the block contents, the `orders` most common orders
    /// and the pairs seen in both orders.
    pub fn report_lines(&self, orders: usize) -> Vec<String> {
        let mut out = Vec::new();
        for (sig, s) in &self.sigs {
            out.push(format!(
                "   {sig}: in {} records, {} occurrences, <= {} per record, <= {} runs, size {}..{}",
                s.records, s.occurrences, s.max_per_record, s.max_runs, s.min_size, s.max_size
            ));
        }
        if !self.in_blocks.is_empty() {
            let inner: Vec<&str> = self.in_blocks.iter().map(String::as_str).collect();
            out.push(format!("   inside BFCB..BFCE: {}", inner.join(" ")));
        }
        let mut by_count: Vec<(&String, &u64)> = self.orders.iter().collect();
        by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (order, n) in by_count.iter().take(orders) {
            out.push(format!("   order x{n}: {order}"));
        }
        let conflicts: Vec<String> = self
            .conflicts()
            .iter()
            .map(|(a, b)| format!("{a}<->{b}"))
            .collect();
        if !conflicts.is_empty() {
            out.push(format!("   both orders seen: {}", conflicts.join(", ")));
        }
        out
    }
}

/// One record [`scan_file`] hands to its visitor.
pub struct Visit<'a> {
    /// Where the record is (signature, FormID, flags).
    pub loc: &'a RecordLoc,
    /// The record, decompressed.
    pub record: &'a Record<'static>,
    /// Its subrecords.
    pub subs: &'a [RawSub<'a>],
    /// The plugin's TES4 localized flag.
    pub localized: bool,
}

/// Calls `visit` for every record in the plugin at `path` whose type `wanted` accepts,
/// in file order, compressed records decompressed under [`DecodeLimits::default`].
pub fn scan_file(
    path: &Path,
    mut wanted: impl FnMut(Signature) -> bool,
    mut visit: impl FnMut(Visit<'_>),
) -> Result<()> {
    let index = index_file(path, &IndexOptions::default())?;
    let localized = index.header.header.flags & FLAG_LOCALIZED != 0;
    let limits = DecodeLimits::default();
    let file = File::open(path).map_err(|source| Error::Io { offset: 0, source })?;
    let mut input = BufReader::new(file);
    for loc in &index.records {
        if !wanted(loc.signature) {
            continue;
        }
        let record = read_indexed(&mut input, loc, Some(&limits))?;
        let subs: Vec<RawSub<'_>> = record
            .subrecords()
            .ok_or(Error::Internal {
                offset: loc.offset,
                message: "a compressed record was not decoded",
            })?
            .iter()
            .map(RawSub::from)
            .collect();
        visit(Visit {
            loc,
            record: &record,
            subs: &subs,
            localized,
        });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]
mod tests {
    use super::{TypeStats, BLOCK_TOKEN};
    use crate::bridge::RawSub;

    fn sub(sig: &[u8; 4], len: usize) -> RawSub<'static> {
        static BYTES: [u8; 16] = [0; 16];
        RawSub {
            signature: *sig,
            data: BYTES.get(..len).unwrap(),
            oversized: false,
        }
    }

    #[test]
    fn counts_runs_orders_and_blocks() {
        let mut t = TypeStats::default();
        t.add(
            &[
                sub(b"EDID", 5),
                sub(b"BFCB", 8),
                sub(b"FLCS", 4),
                sub(b"BFCE", 0),
                sub(b"LNAM", 4),
                sub(b"NNAM", 4),
                sub(b"LNAM", 6),
                sub(b"NNAM", 4),
                sub(b"DATA", 2),
                sub(b"DATA", 3),
            ],
            true,
        );
        t.add(&[sub(b"EDID", 9)], false);
        assert_eq!((t.records, t.compressed, t.subrecords), (2, 1, 11));
        let lnam = &t.sigs["LNAM"];
        assert_eq!(
            (
                lnam.records,
                lnam.occurrences,
                lnam.max_per_record,
                lnam.max_runs
            ),
            (1, 2, 2, 2)
        );
        assert_eq!((lnam.min_size, lnam.max_size), (4, 6));
        let data = &t.sigs["DATA"];
        assert_eq!((data.max_per_record, data.max_runs), (2, 1));
        assert_eq!(t.sigs["EDID"].records, 2);
        assert_eq!(t.sigs["BFCB"].occurrences, 1);
        assert!(!t.sigs.contains_key("FLCS"));
        assert!(t.in_blocks.contains("FLCS"));
        let first = format!("EDID {BLOCK_TOKEN} LNAM NNAM LNAM NNAM DATA+");
        assert_eq!(t.orders[&first], 1);
        assert_eq!(t.orders["EDID"], 1);
        assert_eq!(t.conflicts(), [("LNAM".to_owned(), "NNAM".to_owned())]);
        let lines = t.report_lines(5);
        assert!(lines.contains(&"   inside BFCB..BFCE: FLCS".to_owned()));
        assert!(lines.contains(&"   both orders seen: LNAM<->NNAM".to_owned()));
    }
}
