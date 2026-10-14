//! Comparing two versions of a record: per-subrecord differences and override classes.
//!
//! Subrecords are matched by signature and occurrence (the n-th `KWDA` against the n-th
//! `KWDA`). Without a [`FieldCompare`] payloads are compared as bytes, so a FormID inside a
//! payload is compared as stored: when the two files encode their masters differently,
//! equal references can look different, and callers say so in a limitation. With one
//! (M3.D2), a subrecord the schema gives a layout is compared member by member, FormIDs
//! resolved by plugin name on each side ([`crate::fields`]); the rest stay byte comparisons.

use kitcraft_esm::{Subrecord, FLAG_COMPRESSED, HEADER_LEN};
use kitcraft_schema::Schema;

use crate::fields::{self, FieldChange, Side, Typed};
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
#[derive(Debug, Clone, PartialEq)]
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
    /// First differing byte inside the payload (for `Changed`; `None` when the bytes are
    /// equal and only what they mean differs, a FormID under another master order).
    pub first_difference: Option<u64>,
    /// The schema's member name, when the subrecord was compared through its layout.
    pub member: Option<String>,
    /// The members that differ, when compared through the layout; `None` when compared as
    /// bytes (no layout, an added or removed subrecord, or a payload that does not decode).
    pub fields: Option<Vec<FieldChange>>,
    /// Why a subrecord with a layout is reported as a byte difference: its payload does not
    /// decode, or the bytes differ where the layout compares nothing (after a string's
    /// terminator, or invalid UTF-8 that reads as the same text).
    pub as_bytes: Option<String>,
}

impl SubDiff {
    fn bytes(signature: &str, occurrence: usize, change: Change) -> SubDiff {
        SubDiff {
            signature: signature.to_string(),
            occurrence,
            change,
            a: None,
            b: None,
            first_difference: None,
            member: None,
            fields: None,
            as_bytes: None,
        }
    }
}

/// Field-level comparison of two versions (M3.D2): the schema, how each version reads its
/// payloads, and whether their stored FormIDs mean the same forms.
#[derive(Debug)]
pub struct FieldCompare<'a> {
    /// Layouts (`kitcraft_schema::builtin()` outside tests).
    pub schema: &'a Schema,
    /// The first version.
    pub a: Side<'a>,
    /// The second version.
    pub b: Side<'a>,
    /// Whether a stored FormID names the same form in both versions; then byte-identical
    /// payloads are equal without decoding them.
    pub same_encoding: bool,
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

/// One subrecord keyed by (signature, occurrence): its text and bytes signature, the
/// occurrence, its position and its payload.
type Keyed<'s> = (String, [u8; 4], usize, usize, &'s [u8]);

/// Key every subrecord by (signature, occurrence).
fn keyed<'s>(subs: &'s [Subrecord<'static>]) -> Vec<Keyed<'s>> {
    let mut seen: Vec<([u8; 4], usize)> = Vec::new();
    subs.iter()
        .enumerate()
        .map(|(i, s)| {
            let sig = s.signature().0;
            let n = match seen.iter_mut().find(|(k, _)| *k == sig) {
                Some((_, n)) => {
                    *n = n.saturating_add(1);
                    *n
                }
                None => {
                    seen.push((sig, 0));
                    0
                }
            };
            (s.signature().to_string(), sig, n, i, s.data())
        })
        .collect()
}

/// First differing byte of two payloads (the shorter length when one is a prefix).
fn first_difference(a: &[u8], b: &[u8]) -> u64 {
    a.iter()
        .zip(b.iter())
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len())) as u64
}

/// Per-subrecord differences from `a` to `b`, payloads compared as bytes, in `a`'s order,
/// then additions in `b`'s.
pub fn diff_subrecords(a: &LoadedRecord, b: &LoadedRecord) -> Vec<SubDiff> {
    diff_subrecords_with(a, b, None)
}

/// [`diff_subrecords`], comparing subrecords with a layout member by member when `fields`
/// is given (module docs).
pub fn diff_subrecords_with(
    a: &LoadedRecord,
    b: &LoadedRecord,
    fields: Option<&FieldCompare<'_>>,
) -> Vec<SubDiff> {
    let (ka, kb) = (keyed(&a.subs), keyed(&b.subs));
    let (sa, sb) = (spans(a), spans(b));
    let record = a.record.header.signature.0;
    let mut out = Vec::new();
    for (sig, raw_sig, n, i, data) in &ka {
        let Some((_, _, _, j, other)) = kb.iter().find(|(_, s, m, _, _)| s == raw_sig && m == n)
        else {
            out.push(SubDiff {
                a: sa.get(*i).copied(),
                ..SubDiff::bytes(sig, *n, Change::Removed)
            });
            continue;
        };
        let changed = SubDiff {
            a: sa.get(*i).copied(),
            b: sb.get(*j).copied(),
            first_difference: (other != data).then(|| first_difference(data, other)),
            ..SubDiff::bytes(sig, *n, Change::Changed)
        };
        let typed = match fields {
            Some(c) if !(c.same_encoding && other == data) => {
                fields::compare(c.schema, record, *raw_sig, *n, (data, &c.a), (other, &c.b))
            }
            _ => Typed::NoLayout,
        };
        match typed {
            Typed::Compared { member, changes } => {
                if !changes.is_empty() {
                    out.push(SubDiff {
                        member: Some(member),
                        fields: Some(changes),
                        ..changed
                    });
                } else if other != data && fields.is_some_and(|c| c.same_encoding) {
                    // Equal by value, different bytes, and no master order to explain it.
                    out.push(SubDiff {
                        member: Some(member),
                        fields: Some(Vec::new()),
                        as_bytes: Some(
                            "the bytes differ where the layout compares no value (after a \
                             string's terminator, or text that is not valid UTF-8)"
                                .to_string(),
                        ),
                        ..changed
                    });
                }
            }
            Typed::Undecodable(why) => {
                if other != data {
                    out.push(SubDiff {
                        as_bytes: Some(why),
                        ..changed
                    });
                }
            }
            Typed::NoLayout => {
                if other != data {
                    out.push(changed);
                }
            }
        }
    }
    for (sig, raw_sig, n, j, _) in &kb {
        if !ka.iter().any(|(_, s, m, _, _)| s == raw_sig && m == n) {
            out.push(SubDiff {
                b: sb.get(*j).copied(),
                ..SubDiff::bytes(sig, *n, Change::Added)
            });
        }
    }
    out
}

/// Signatures in order.
fn signatures(r: &LoadedRecord) -> Vec<[u8; 4]> {
    r.subs.iter().map(|s| s.signature().0).collect()
}

/// Whether two subrecord lists hold the same subrecords in a different order.
pub fn reordered(a: &LoadedRecord, b: &LoadedRecord) -> bool {
    diff_subrecords(a, b).is_empty() && signatures(a) != signatures(b)
}

/// How an override relates to the version before it in load order.
#[derive(Debug, Clone, PartialEq)]
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

/// Class of `current` against `previous`, payloads compared as bytes.
pub fn classify(previous: &LoadedRecord, current: &LoadedRecord) -> OverrideClass {
    classify_with(previous, current, None)
}

/// [`classify`], with typed subrecords compared member by member when `fields` is given:
/// payloads that differ only in how they encode the same forms are then identical.
pub fn classify_with(
    previous: &LoadedRecord,
    current: &LoadedRecord,
    fields: Option<&FieldCompare<'_>>,
) -> OverrideClass {
    if current.record.header.flags & RECORD_FLAG_DELETED != 0 {
        return OverrideClass::Deleted;
    }
    let mask = !FLAG_COMPRESSED;
    let flags = previous.record.header.flags & mask != current.record.header.flags & mask;
    let subrecords = diff_subrecords_with(previous, current, fields);
    let order = subrecords.is_empty() && signatures(previous) != signatures(current);
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

/// The first byte of a record's EditorID (the union decider), if it has one.
pub fn edid_first(rec: &LoadedRecord) -> Option<u8> {
    rec.subs
        .iter()
        .find(|s| s.signature().0 == *b"EDID")
        .and_then(|s| s.data().first().copied())
        .filter(|b| *b != 0)
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
