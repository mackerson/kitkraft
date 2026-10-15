//! Bridge between the container and the generated record types ([`crate::generated`],
//! written by `cargo xtask codegen` from `crates/schema/defs/`).
//!
//! Generated records decode from `&[RawSub]` (subrecords after `XXXX` resolution) plus a
//! [`Ctx`], and encode to `Vec<OwnedSub>`. This module converts between those and the
//! container's [`Subrecord`], builds the [`Ctx`] for a record, and offers
//! [`Record::typed`] / [`Record::set_typed`] for whole records and
//! [`Record::check_typed`] to test a definition against real bytes (M2.A3).
//!
//! # `XXXX` prefixes
//!
//! `oversized` on a [`RawSub`] says whether the subrecord was read with an `XXXX` prefix.
//! On write the prefix is chosen by size alone, the policy the corpus supports (every one of
//! the 12,849 `XXXX`-sized subrecords in the official ESMs has a payload over `u16::MAX`,
//! `docs/corpus.md`): [`Subrecord::from_owned`] writes a prefix exactly when the payload
//! does not fit a `u16`, whatever `oversized` says. [`Record::set_owned_subs`] keeps a
//! subrecord whose signature and payload are unchanged exactly as it was read, so an
//! unchanged value never changes the file.

pub use crate::generated::records::{Ctx, OwnedSub, RawSub, Sig};

use crate::generated::records::{match_body, Record as TypedRecord, RECORDS};
use crate::model::{Plugin, Record, Subrecord};
use crate::{DecodeLimits, Result, Signature};

impl<'s> From<&'s Subrecord<'_>> for RawSub<'s> {
    fn from(sub: &'s Subrecord<'_>) -> RawSub<'s> {
        RawSub {
            signature: sub.signature().bytes(),
            data: sub.data(),
            oversized: sub.has_oversize_prefix(),
        }
    }
}

impl From<&Subrecord<'_>> for OwnedSub {
    fn from(sub: &Subrecord<'_>) -> OwnedSub {
        OwnedSub {
            signature: sub.signature().bytes(),
            data: sub.data().to_vec(),
            oversized: sub.has_oversize_prefix(),
        }
    }
}

impl Subrecord<'static> {
    /// A subrecord from an encoded value. Its size is computed when written, with an
    /// `XXXX` prefix exactly when the payload does not fit a `u16` (`sub.oversized` is
    /// not consulted; see the module documentation). The signature `XXXX` itself is
    /// rejected by the writer.
    pub fn from_owned(sub: OwnedSub) -> Subrecord<'static> {
        Subrecord::new(Signature(sub.signature), sub.data)
    }
}

/// Why a record has no typed form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotTyped {
    /// The schema has no definition for the record type.
    NoDefinition,
    /// The record is compressed and not decoded (see [`Record::decompress`], or use
    /// [`Plugin::typed`], which decodes on the fly).
    NotDecoded,
}

/// Whether the schema defines record type `signature`.
pub fn has_definition(signature: Signature) -> bool {
    RECORDS.iter().any(|(sig, _)| *sig == signature.bytes())
}

/// How a subrecord disagrees with its record's definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// The definition places the subrecord in a typed member, but its payload does not
    /// decode as that type (it is kept raw, so writing it back is still identical).
    NotDecoded,
    /// Encoding the decoded record does not reproduce the subrecord at this position.
    Reencoded,
}

/// One subrecord where a definition and the bytes disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypedConflict {
    /// Position of the subrecord in the record (`XXXX` prefixes folded).
    pub index: usize,
    /// Its signature (the input's; the encoding's when the input is shorter).
    pub signature: Signature,
    /// Absolute offset of the subrecord, or `None` inside a compressed record (positions
    /// there are relative to the decompressed body).
    pub offset: Option<u64>,
    pub kind: ConflictKind,
}

/// Result of [`Record::check_typed`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypedCheck {
    /// Subrecords in the record.
    pub subrecords: usize,
    /// Subrecords the definition places (in typed or opaque members).
    pub placed: usize,
    /// Every disagreement; empty when `encode(decode(x)) == x` holds subrecord by
    /// subrecord and every placed payload decodes.
    pub conflicts: Vec<TypedConflict>,
}

impl TypedCheck {
    /// Whether the definition reproduces the record exactly.
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }
}

impl<'a> Record<'a> {
    /// The subrecords as generated code reads them, or `None` for a compressed record that
    /// has not been decoded (see [`Record::decompress`]).
    pub fn raw_subs(&self) -> Option<Vec<RawSub<'_>>> {
        self.subrecords()
            .map(|subs| subs.iter().map(RawSub::from).collect())
    }

    /// Replace the subrecords with encoded values. A value whose signature and payload
    /// equal the subrecord already at its position keeps that subrecord as read (its
    /// `XXXX` prefix and offset included); any other value is written by size (see the
    /// module documentation). Returns `false` (and changes nothing) for a compressed record
    /// that has not been decoded. Marks the record modified.
    pub fn set_owned_subs(&mut self, subs: Vec<OwnedSub>) -> bool {
        match self.subrecords_mut() {
            Some(current) => {
                let next: Vec<Subrecord<'a>> = subs
                    .into_iter()
                    .enumerate()
                    .map(|(i, sub)| match current.get(i) {
                        Some(old)
                            if old.signature().bytes() == sub.signature
                                && old.data() == sub.data.as_slice() =>
                        {
                            old.clone()
                        }
                        _ => Subrecord::from_owned(sub),
                    })
                    .collect();
                *current = next;
                true
            }
            None => false,
        }
    }

    /// Decode this record through its generated definition. Subrecords the definition
    /// does not place, or whose payload does not decode, are kept raw inside the typed
    /// record, so [`Record::set_typed`] with an unchanged value writes the same bytes.
    pub fn typed(&self, ctx: Ctx) -> Result<TypedRecord, NotTyped> {
        if !has_definition(self.header.signature) {
            return Err(NotTyped::NoDefinition);
        }
        let subs = self.raw_subs().ok_or(NotTyped::NotDecoded)?;
        TypedRecord::decode(self.header.signature.bytes(), &subs, ctx).ok_or(NotTyped::NoDefinition)
    }

    /// Replace the subrecords with the encoding of `typed` (see
    /// [`Record::set_owned_subs`]). Returns `false` (and changes nothing) for a compressed
    /// record that has not been decoded. Marks the record modified; the header
    /// (signature, FormID, flags) is not touched.
    pub fn set_typed(&mut self, typed: &TypedRecord) -> bool {
        self.set_owned_subs(typed.encode())
    }

    /// Decode this record through its definition, encode it again and compare, subrecord
    /// by subrecord (M2.A3): every placed payload must decode, and `encode(decode(x))`
    /// must reproduce `x`. Errors as [`Record::typed`].
    pub fn check_typed(&self, ctx: Ctx) -> Result<TypedCheck, NotTyped> {
        if !has_definition(self.header.signature) {
            return Err(NotTyped::NoDefinition);
        }
        let subs = self.subrecords().ok_or(NotTyped::NotDecoded)?;
        check_subrecords(self.header.signature, subs, ctx)
    }
}

/// [`Record::check_typed`] over the subrecords of a record of type `signature` (for
/// example those [`Record::decode`] returned for a compressed record).
pub fn check_subrecords(
    signature: Signature,
    subs: &[Subrecord<'_>],
    ctx: Ctx,
) -> Result<TypedCheck, NotTyped> {
    let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
    let members = RECORDS
        .iter()
        .find(|(sig, _)| *sig == signature.bytes())
        .map(|(_, m)| *m)
        .ok_or(NotTyped::NoDefinition)?;
    let typed = TypedRecord::decode(signature.bytes(), &raw, ctx).ok_or(NotTyped::NoDefinition)?;
    let (_, unmatched) = match_body(signature.bytes(), &raw, members);
    let offset = |i: usize| subs.get(i).and_then(Subrecord::offset);
    let mut conflicts = Vec::new();
    // Every generated record type keeps the subrecords it could not decode in `extra`
    // (`Record::extra()`, generated for all types since M2.B3); those the definition
    // places are payloads that did not decode.
    let mut failed: Vec<usize> = typed
        .extra()
        .iter()
        .map(|(i, _)| *i)
        .filter(|i| !unmatched.contains(i))
        .collect();
    failed.sort_unstable();
    failed.dedup();
    for index in failed {
        if let Some(s) = subs.get(index) {
            conflicts.push(TypedConflict {
                index,
                signature: s.signature(),
                offset: offset(index),
                kind: ConflictKind::NotDecoded,
            });
        }
    }
    for index in reencode_mismatches(&raw, &typed.encode()) {
        let signature = raw
            .get(index)
            .map(|s| s.signature)
            .or_else(|| typed.encode().get(index).map(|s| s.signature))
            .unwrap_or_default();
        conflicts.push(TypedConflict {
            index,
            signature: Signature(signature),
            offset: offset(index),
            kind: ConflictKind::Reencoded,
        });
    }
    Ok(TypedCheck {
        subrecords: subs.len(),
        placed: subs.len().saturating_sub(unmatched.len()),
        conflicts,
    })
}

/// Positions where `encoded` differs from `raw` (signature, payload or `XXXX` flag),
/// including positions only one of them has.
fn reencode_mismatches(raw: &[RawSub<'_>], encoded: &[OwnedSub]) -> Vec<usize> {
    let len = raw.len().max(encoded.len());
    (0..len)
        .filter(|&i| match (raw.get(i), encoded.get(i)) {
            (Some(r), Some(e)) => {
                r.signature != e.signature
                    || r.data != e.data.as_slice()
                    || r.oversized != e.oversized
            }
            _ => true,
        })
        .collect()
}

impl Ctx {
    /// The context for `record` in a plugin whose TES4 localized flag is `localized`.
    pub fn for_record(record: &Record<'_>, localized: bool) -> Ctx {
        Ctx {
            form_version: record.header.form_version,
            localized,
        }
    }
}

impl Plugin<'_> {
    /// The context generated code needs to decode `record` of this plugin: the record's
    /// form version and this plugin's localized flag.
    pub fn ctx(&self, record: &Record<'_>) -> Ctx {
        Ctx::for_record(record, self.is_localized())
    }

    /// [`Record::typed`] with this plugin's context. A compressed record that is not
    /// decoded is decompressed on the fly under `limits` (the record is not changed).
    /// `Ok(None)` when the schema has no definition for the record type.
    pub fn typed(&self, record: &Record<'_>, limits: &DecodeLimits) -> Result<Option<TypedRecord>> {
        let ctx = self.ctx(record);
        match record.typed(ctx) {
            Ok(typed) => Ok(Some(typed)),
            Err(NotTyped::NoDefinition) => Ok(None),
            Err(NotTyped::NotDecoded) => {
                let Some(subs) = record.decode(limits)? else {
                    return Ok(None);
                };
                let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
                Ok(TypedRecord::decode(
                    record.header.signature.bytes(),
                    &raw,
                    ctx,
                ))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{reencode_mismatches, ConflictKind, Ctx, NotTyped, OwnedSub, RawSub};
    use crate::fixture::{self, group, record, sub, xxxx_sub};
    use crate::{DecodeLimits, Entry, Plugin, Subrecord};

    #[test]
    fn raw_subs_carry_the_xxxx_prefix_and_owned_subs_write_it_back() {
        let body = [
            sub(b"EDID", b"n\0"),
            xxxx_sub(b"NVNM", &[1u8; 12]),
            sub(b"DATA", &[2u8; 4]),
        ]
        .concat();
        let bytes = fixture::plugin(&[group(b"NAVM", 0, &record(b"NAVM", 0, 0x400, &body))]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let Entry::Group(g) = &plugin.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        let raw = r.raw_subs().unwrap();
        assert_eq!(
            raw[1],
            RawSub {
                signature: *b"NVNM",
                data: &[1u8; 12],
                oversized: true
            }
        );
        assert!(!raw[0].oversized && !raw[2].oversized);
        let ctx = plugin.ctx(r);
        assert_eq!(
            ctx,
            Ctx {
                form_version: 581,
                localized: true
            }
        );

        // Encoded values replace the subrecords; unchanged values keep their bytes, even
        // the XXXX prefix on a small payload that no new value would get.
        let owned: Vec<OwnedSub> = raw.iter().map(OwnedSub::from).collect();
        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
            panic!()
        };
        let Entry::Record(r) = &mut g.children_mut()[0] else {
            panic!()
        };
        assert!(r.set_owned_subs(owned));
        assert_eq!(plugin.to_bytes().unwrap(), bytes);

        let built = Subrecord::from_owned(OwnedSub {
            signature: *b"NVNM",
            data: vec![3u8; 2],
            oversized: true,
        });
        assert!(!built.has_oversize_prefix());
        assert_eq!(OwnedSub::from(&built).data, [3u8; 2]);
        let big = Subrecord::from_owned(OwnedSub {
            signature: *b"NVNM",
            data: vec![3u8; 70_000],
            oversized: false,
        });
        assert!(big.has_oversize_prefix());
    }

    /// Review round 3, finding 1: a sticky `oversized` wrote `XXXX` before a payload that
    /// had shrunk below 64 KiB, a form no permitted source shows.
    #[test]
    fn a_shrunk_xxxx_value_is_written_without_the_prefix() {
        let bytes = fixture::sample_plugin();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let Entry::Group(navm) = &mut plugin.entries_mut()[2] else {
            panic!()
        };
        let Entry::Record(r) = &mut navm.children_mut()[0] else {
            panic!()
        };
        let mut owned: Vec<OwnedSub> = r.raw_subs().unwrap().iter().map(OwnedSub::from).collect();
        assert!(owned[0].oversized);
        owned[0].data.truncate(10);
        assert!(r.set_owned_subs(owned));
        assert!(!r.subrecords().unwrap()[0].has_oversize_prefix());
        let out = plugin.to_bytes().unwrap();
        assert!(!out.windows(4).any(|w| w == b"XXXX"));
        let reparsed = Plugin::parse(&out).unwrap();
        let Entry::Group(navm) = &reparsed.entries()[2] else {
            panic!()
        };
        let Entry::Record(r) = &navm.children()[0] else {
            panic!()
        };
        assert_eq!(r.subrecords().unwrap()[0].data().len(), 10);
        assert_eq!(r.subrecords().unwrap()[0].size_field_as_read(), Some(10));
    }

    #[test]
    fn typed_records_decode_and_write_back_identically() {
        use crate::generated::records::Record as Typed;
        let limits = DecodeLimits::default();
        let bytes = fixture::sample_plugin();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let tes4 = plugin.typed(plugin.header(), &limits).unwrap().unwrap();
        let Typed::Tes4(t) = &tes4 else {
            panic!("{tes4:?}")
        };
        assert_eq!(t.master.len(), 2);
        let Entry::Group(g) = &plugin.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        let typed = plugin.typed(r, &limits).unwrap().unwrap();
        let Typed::Gmst(gmst) = &typed else {
            panic!("{typed:?}")
        };
        assert_eq!(gmst.editor_id.as_ref().unwrap().0, b"fFoo");
        // CELL has a definition since the world-family schema (undefined types are covered
        // by `undecoded_compressed_records_are_told_apart_and_decoded_on_the_fly`).
        let Entry::Group(cells) = &plugin.entries()[1] else {
            panic!()
        };
        let Entry::Record(cell) = &cells.children()[0] else {
            panic!()
        };
        let typed_cell = plugin.typed(cell, &limits).unwrap().unwrap();
        assert!(matches!(typed_cell, Typed::Cell(_)), "{typed_cell:?}");
        assert!(typed_cell.extra().is_empty());

        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
            panic!()
        };
        let Entry::Record(r) = &mut g.children_mut()[0] else {
            panic!()
        };
        assert!(r.set_typed(&typed));
        assert!(r.is_modified());
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
    }

    /// Review round 3, finding 9: "no definition" and "compressed, not decoded" were both
    /// `None`; `Plugin::typed` now decodes on the fly.
    #[test]
    fn undecoded_compressed_records_are_told_apart_and_decoded_on_the_fly() {
        let fields = [sub(b"EDID", b"kw\0"), sub(b"FULL", &[1, 0, 0, 0])].concat();
        let rec = fixture::compressed_record_stored(b"KYWD", 1, &fields);
        let land = fixture::compressed_record_stored(b"LAND", 2, &sub(b"EDID", b"x\0"));
        let bytes = fixture::plugin(&[group(b"KYWD", 0, &rec), group(b"LAND", 0, &land)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let limits = DecodeLimits::default();
        let Entry::Group(g) = &plugin.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        assert_eq!(r.typed(plugin.ctx(r)).unwrap_err(), NotTyped::NotDecoded);
        assert_eq!(
            r.check_typed(plugin.ctx(r)).unwrap_err(),
            NotTyped::NotDecoded
        );
        let typed = plugin.typed(r, &limits).unwrap().unwrap();
        assert_eq!(typed.encode().len(), 2);
        let Entry::Group(g) = &plugin.entries()[1] else {
            panic!()
        };
        let Entry::Record(land) = &g.children()[0] else {
            panic!()
        };
        assert_eq!(
            land.typed(Ctx::default()).unwrap_err(),
            NotTyped::NoDefinition
        );
        assert!(plugin.typed(land, &limits).unwrap().is_none());

        let Entry::Group(g) = &mut plugin.entries_mut()[1] else {
            panic!()
        };
        let Entry::Record(r) = &mut g.children_mut()[0] else {
            panic!()
        };
        assert!(r.raw_subs().is_none());
        assert!(!r.set_owned_subs(Vec::new()));
        r.decompress(&DecodeLimits::default()).unwrap();
        assert_eq!(r.raw_subs().unwrap()[0].data, b"x\0");
        assert_eq!(Ctx::for_record(r, false).form_version, 581);
    }

    #[test]
    fn check_typed_reports_payloads_that_do_not_decode() {
        // The fixture TES4 decodes cleanly.
        let bytes = fixture::sample_plugin();
        let plugin = Plugin::parse(&bytes).unwrap();
        let check = plugin
            .header()
            .check_typed(plugin.ctx(plugin.header()))
            .unwrap();
        assert!(check.is_clean(), "{check:?}");
        assert_eq!(check.subrecords, 7);
        assert_eq!(check.placed, 7);

        // A HEDR two bytes short, an unterminated CNAM and an unknown subrecord: the first
        // two are placed but do not decode; the third is not placed at all.
        let body = [
            sub(b"HEDR", &[0u8; 10]),
            sub(b"CNAM", b"abc"),
            sub(b"ZZZZ", b""),
        ]
        .concat();
        let bytes = record(b"TES4", 0, 0, &body);
        let plugin = Plugin::parse(&bytes).unwrap();
        let check = plugin.header().check_typed(Ctx::default()).unwrap();
        assert_eq!(check.subrecords, 3);
        assert_eq!(check.placed, 2);
        let found: Vec<(usize, String, Option<u64>, ConflictKind)> = check
            .conflicts
            .iter()
            .map(|c| (c.index, c.signature.to_string(), c.offset, c.kind))
            .collect();
        assert_eq!(
            found,
            [
                (0, "HEDR".into(), Some(24), ConflictKind::NotDecoded),
                (1, "CNAM".into(), Some(24 + 16), ConflictKind::NotDecoded),
            ]
        );
    }

    /// Lane F's M3.F2 request: `extra_of` matched five record types, so a payload that
    /// did not decode in any other type (AMMO here) went unreported.
    #[test]
    fn check_typed_reports_undecoded_payloads_of_every_defined_type() {
        use crate::generated::records::RECORDS;
        let body = [sub(b"EDID", b"unterminated"), sub(b"DATA", &[0u8; 4])].concat();
        for (sig, _) in RECORDS {
            if *sig == *b"TES4" {
                continue;
            }
            let bytes = fixture::plugin(&[group(sig, 0, &record(sig, 0, 0x800, &body))]);
            let plugin = Plugin::parse(&bytes).unwrap();
            let Entry::Group(g) = &plugin.entries()[0] else {
                panic!()
            };
            let Entry::Record(r) = &g.children()[0] else {
                panic!()
            };
            let check = r.check_typed(plugin.ctx(r)).unwrap();
            assert!(
                check
                    .conflicts
                    .iter()
                    .any(|c| c.index == 0 && c.kind == ConflictKind::NotDecoded),
                "{}: {check:?}",
                String::from_utf8_lossy(sig)
            );
        }
    }

    #[test]
    fn reencode_mismatches_cover_every_difference() {
        let raw = [
            RawSub {
                signature: *b"EDID",
                data: b"a\0",
                oversized: false,
            },
            RawSub {
                signature: *b"DATA",
                data: b"x",
                oversized: false,
            },
            RawSub {
                signature: *b"FULL",
                data: b"",
                oversized: true,
            },
        ];
        let mut encoded: Vec<OwnedSub> = raw.iter().map(OwnedSub::from).collect();
        assert!(reencode_mismatches(&raw, &encoded).is_empty());
        encoded[1].data = b"y".to_vec();
        encoded[2].oversized = false;
        encoded.push(encoded[0].clone());
        assert_eq!(reencode_mismatches(&raw, &encoded), [1, 2, 3]);
        encoded.truncate(1);
        assert_eq!(reencode_mismatches(&raw, &encoded), [1, 2]);
    }
}
