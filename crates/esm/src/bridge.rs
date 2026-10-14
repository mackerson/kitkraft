//! Bridge between the container and the generated record types (`kitcraft-schema` codegen).
//!
//! Generated records decode from `&[RawSub]` (subrecords after `XXXX` resolution) plus a
//! `Ctx`, and encode to `Vec<OwnedSub>`. Those three types are emitted by
//! `kitcraft_schema::generate` as part of the generated module, which lands in this crate
//! at `src/generated/mod.rs` with M1.15 and does not exist yet. Until then this module
//! defines field-for-field copies of them (schema prelude as of `main` at 7510823, lane B
//! round 2). When the generated module lands, replace the three definitions below with
//! `pub use crate::generated::{Ctx, OwnedSub, RawSub};`; the conversions need no change.
//!
//! `oversized` is the container's `XXXX` prefix: set on a subrecord read with one (whatever
//! its size), and honoured on write. A payload longer than `u16::MAX` gets a prefix either
//! way.

use crate::model::{Encoding, Plugin, Record, Subrecord};
use crate::Signature;

/// A four-byte signature, as the generated code spells it.
pub type Sig = [u8; 4];

/// A subrecord as read from a record body (mirror of the generated `RawSub`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSub<'a> {
    pub signature: Sig,
    pub data: &'a [u8],
    /// The size came from a preceding `XXXX` subrecord.
    pub oversized: bool,
}

/// An owned subrecord (mirror of the generated `OwnedSub`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSub {
    pub signature: Sig,
    pub data: Vec<u8>,
    /// Write an `XXXX` size prefix even when the payload fits a `u16`.
    pub oversized: bool,
}

/// Facts from the container that change layouts (mirror of the generated `Ctx`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ctx {
    /// The record's form version (record header).
    pub form_version: u16,
    /// The plugin's TES4 localized flag (`0x80`).
    pub localized: bool,
}

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
    /// `XXXX` prefix when `oversized` is set or the payload does not fit a `u16`. The
    /// signature `XXXX` itself is rejected by the writer.
    pub fn from_owned(sub: OwnedSub) -> Subrecord<'static> {
        let mut out = Subrecord::new(Signature(sub.signature), sub.data);
        out.encoding = Encoding::Canonical {
            oversize: sub.oversized,
        };
        out
    }
}

impl<'a> Record<'a> {
    /// The subrecords as generated code reads them, or `None` for a compressed record that
    /// has not been decoded (see [`Record::decompress`]).
    pub fn raw_subs(&self) -> Option<Vec<RawSub<'_>>> {
        self.subrecords()
            .map(|subs| subs.iter().map(RawSub::from).collect())
    }

    /// Replace the subrecords with encoded values. Returns `false` (and changes nothing)
    /// for a compressed record that has not been decoded. Marks the record modified.
    pub fn set_owned_subs(&mut self, subs: Vec<OwnedSub>) -> bool {
        match self.subrecords_mut() {
            Some(current) => {
                *current = subs.into_iter().map(Subrecord::from_owned).collect();
                true
            }
            None => false,
        }
    }
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
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{Ctx, OwnedSub, RawSub};
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

        // Encoded values replace the subrecords; unchanged values give the same bytes.
        let owned: Vec<OwnedSub> = raw
            .iter()
            .map(|s| OwnedSub {
                signature: s.signature,
                data: s.data.to_vec(),
                oversized: s.oversized,
            })
            .collect();
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
        assert!(built.has_oversize_prefix());
        assert_eq!(OwnedSub::from(&built).data, [3u8; 2]);
    }

    #[test]
    fn undecoded_compressed_records_have_no_raw_subs() {
        let rec = fixture::compressed_record_stored(b"LAND", 1, &sub(b"EDID", b"x\0"));
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
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
}
