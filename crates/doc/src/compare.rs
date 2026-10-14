//! Comparing two versions of a record: per-subrecord differences and override classes.
//!
//! Subrecords are matched by signature and occurrence (the n-th `KWDA` against the n-th
//! `KWDA`). Payloads are compared as bytes: no typed layouts are wired yet (M2.A3), so a
//! FormID inside a payload is compared as stored. When the two files encode their masters
//! differently, equal references can look different; callers say so in a limitation.

use kitcraft_esm::{Subrecord, FLAG_COMPRESSED, HEADER_LEN};

use crate::flags::RECORD_FLAG_DELETED;
use crate::record::LoadedRecord;

/// Where a subrecord is: its offset in the file (uncompressed records) or in the inflated
/// body (compressed records), and its payload length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Absolute offset of the payload in the file, `None` inside a compressed body.
    pub file_offset: Option<u64>,
    /// Offset of the payload in the record body (inflated for a compressed record).
    pub body_offset: u64,
    /// Payload length.
    pub length: u64,
}

/// What happened to one subrecord occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Only in the second version.
    Added,
    /// Only in the first version.
    Removed,
    /// In both, with different payloads.
    Changed,
}

impl Change {
    /// Lower-case name.
    pub fn as_str(self) -> &'static str {
        match self {
            Change::Added => "added",
            Change::Removed => "removed",
            Change::Changed => "changed",
        }
    }
}

/// One subrecord difference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubDiff {
    /// Signature.
    pub signature: String,
    /// Occurrence index of that signature (0-based).
    pub occurrence: usize,
    /// Kind.
    pub change: Change,
    /// In the first version.
    pub a: Option<Span>,
    /// In the second version.
    pub b: Option<Span>,
    /// First differing byte inside the payload (for `Changed`).
    pub first_difference: Option<u64>,
}

/// Spans of every subrecord of a record, in order.
pub fn spans(rec: &LoadedRecord) -> Vec<Span> {
    let mut body_at = 0u64;
    rec.subs
        .iter()
        .map(|s| {
            let len = s.data().len() as u64;
            let header = s.encoded_len().saturating_sub(len);
            let span = Span {
                file_offset: if rec.compressed {
                    None
                } else {
                    s.data_offset()
                },
                body_offset: body_at.saturating_add(header),
                length: len,
            };
            body_at = body_at.saturating_add(s.encoded_len());
            span
        })
        .collect()
}

/// Key every subrecord by (signature, occurrence).
fn keyed<'s>(subs: &'s [Subrecord<'static>]) -> Vec<(String, usize, usize, &'s [u8])> {
    let mut seen: Vec<(String, usize)> = Vec::new();
    subs.iter()
        .enumerate()
        .map(|(i, s)| {
            let sig = s.signature().to_string();
            let n = match seen.iter_mut().find(|(k, _)| *k == sig) {
                Some((_, n)) => {
                    *n = n.saturating_add(1);
                    *n
                }
                None => {
                    seen.push((sig.clone(), 0));
                    0
                }
            };
            (sig, n, i, s.data())
        })
        .collect()
}

/// Per-subrecord differences from `a` to `b`, in `a`'s order, then additions in `b`'s.
pub fn diff_subrecords(a: &LoadedRecord, b: &LoadedRecord) -> Vec<SubDiff> {
    let (ka, kb) = (keyed(&a.subs), keyed(&b.subs));
    let (sa, sb) = (spans(a), spans(b));
    let mut out = Vec::new();
    for (sig, n, i, data) in &ka {
        match kb.iter().find(|(s, m, _, _)| s == sig && m == n) {
            None => out.push(SubDiff {
                signature: sig.clone(),
                occurrence: *n,
                change: Change::Removed,
                a: sa.get(*i).copied(),
                b: None,
                first_difference: None,
            }),
            Some((_, _, j, other)) if other != data => out.push(SubDiff {
                signature: sig.clone(),
                occurrence: *n,
                change: Change::Changed,
                a: sa.get(*i).copied(),
                b: sb.get(*j).copied(),
                first_difference: Some(
                    data.iter()
                        .zip(other.iter())
                        .position(|(x, y)| x != y)
                        .unwrap_or(data.len().min(other.len())) as u64,
                ),
            }),
            Some(_) => {}
        }
    }
    for (sig, n, j, _) in &kb {
        if !ka.iter().any(|(s, m, _, _)| s == sig && m == n) {
            out.push(SubDiff {
                signature: sig.clone(),
                occurrence: *n,
                change: Change::Added,
                a: None,
                b: sb.get(*j).copied(),
                first_difference: None,
            });
        }
    }
    out
}

/// Whether two subrecord lists hold the same subrecords in a different order.
pub fn reordered(a: &LoadedRecord, b: &LoadedRecord) -> bool {
    let sigs = |r: &LoadedRecord| -> Vec<String> {
        r.subs.iter().map(|s| s.signature().to_string()).collect()
    };
    diff_subrecords(a, b).is_empty() && sigs(a) != sigs(b)
}

/// How an override relates to the version before it in load order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideClass {
    /// Same flags (compressed bit aside) and same subrecords.
    IdenticalToPrevious,
    /// The deleted flag is set.
    Deleted,
    /// Something differs.
    Differs {
        /// Record flags differ (compressed bit aside).
        flags: bool,
        /// Same subrecords in another order.
        order: bool,
        /// Subrecord differences.
        subrecords: Vec<SubDiff>,
    },
}

impl OverrideClass {
    /// `identical-to-previous`, `deleted` or `differs`.
    pub fn as_str(&self) -> &'static str {
        match self {
            OverrideClass::IdenticalToPrevious => "identical-to-previous",
            OverrideClass::Deleted => "deleted",
            OverrideClass::Differs { .. } => "differs",
        }
    }
}

/// Class of `current` against `previous`.
pub fn classify(previous: &LoadedRecord, current: &LoadedRecord) -> OverrideClass {
    if current.record.header.flags & RECORD_FLAG_DELETED != 0 {
        return OverrideClass::Deleted;
    }
    let mask = !FLAG_COMPRESSED;
    let flags = previous.record.header.flags & mask != current.record.header.flags & mask;
    let subrecords = diff_subrecords(previous, current);
    let order = subrecords.is_empty() && reordered(previous, current);
    if !flags && !order && subrecords.is_empty() {
        OverrideClass::IdenticalToPrevious
    } else {
        OverrideClass::Differs {
            flags,
            order,
            subrecords,
        }
    }
}

/// Byte length of a record as stored (header plus body).
pub fn stored_len(rec: &LoadedRecord) -> u64 {
    rec.data_size().saturating_add(HEADER_LEN as u64)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::index::PluginIndex;
    use crate::record::read_record_file;
    use crate::testdata::{self, rec, rec_flags};

    fn load(records: Vec<Vec<u8>>) -> Vec<LoadedRecord> {
        let bytes = testdata::plugin_with(0x1, &[], &records);
        let path = testdata::temp_file("cmp.esm", &bytes);
        let index = PluginIndex::build(&path).unwrap();
        index
            .records
            .iter()
            .map(|r| read_record_file(&path, r).unwrap())
            .collect()
    }

    #[test]
    fn classes() {
        let base = vec![
            ("EDID", b"Gun\0".to_vec()),
            ("FULL", vec![1, 0, 0, 0]),
            ("KWDA", vec![1]),
            ("KWDA", vec![2]),
        ];
        let mut changed = base.clone();
        changed[1].1 = vec![2, 0, 0, 0];
        changed.remove(3);
        changed.push(("DNAM", vec![7]));
        let mut swapped = base.clone();
        swapped.swap(0, 1);
        let recs = load(vec![
            rec(b"WEAP", 1, &base),
            rec(b"WEAP", 1, &base),
            rec(b"WEAP", 1, &changed),
            rec_flags(b"WEAP", 0x20, 1, &[]),
            rec(b"WEAP", 1, &swapped),
            rec_flags(b"WEAP", 0x2, 1, &base),
            testdata::compressed(b"WEAP", 1, &base),
        ]);
        assert_eq!(
            classify(&recs[0], &recs[1]),
            OverrideClass::IdenticalToPrevious
        );
        assert_eq!(classify(&recs[0], &recs[3]), OverrideClass::Deleted);
        // Compressed and uncompressed with the same subrecords are identical.
        assert_eq!(
            classify(&recs[0], &recs[6]),
            OverrideClass::IdenticalToPrevious
        );
        let OverrideClass::Differs { subrecords, .. } = classify(&recs[1], &recs[2]) else {
            panic!()
        };
        let summary: Vec<(String, usize, &str)> = subrecords
            .iter()
            .map(|d| (d.signature.clone(), d.occurrence, d.change.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                ("FULL".to_string(), 0, "changed"),
                ("KWDA".to_string(), 1, "removed"),
                ("DNAM".to_string(), 0, "added")
            ]
        );
        let full = &subrecords[0];
        assert_eq!(full.first_difference, Some(0));
        let (a, b) = (full.a.unwrap(), full.b.unwrap());
        assert_eq!(a.length, 4);
        assert_eq!(a.body_offset, 6 + 4 + 6);
        assert_eq!(b.file_offset.unwrap() - recs[2].offset(), 24 + 6 + 4 + 6);
        assert!(matches!(
            classify(&recs[0], &recs[4]),
            OverrideClass::Differs { order: true, .. }
        ));
        assert!(matches!(
            classify(&recs[0], &recs[5]),
            OverrideClass::Differs { flags: true, .. }
        ));
    }
}
