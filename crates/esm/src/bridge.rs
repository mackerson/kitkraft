//! Bridge between the container and the generated record types ([`crate::generated`],
//! written by `cargo xtask codegen` from `crates/schema/defs/`).
//!
//! Generated records decode from `&[RawSub]` (subrecords after `XXXX` resolution) plus a
//! [`Ctx`], and encode to `Vec<OwnedSub>`. This module converts between those and the
//! container's [`Subrecord`], builds the [`Ctx`] for a record, and offers
//! [`Record::typed`] / [`Record::set_typed`] for whole records.
//!
//! `oversized` is the container's `XXXX` prefix: set on a subrecord read with one (whatever
//! its size), and honoured on write. A payload longer than `u16::MAX` gets a prefix either
//! way.

pub use crate::generated::records::{Ctx, OwnedSub, RawSub, Sig};

use crate::generated::records::Record as TypedRecord;
use crate::model::{Encoding, Plugin, Record, Subrecord};
use crate::Signature;

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

    /// Decode this record through its generated definition. `None` when the record type
    /// has no definition, or the record is compressed and not decoded. Subrecords the
    /// definition does not place are kept raw inside the typed record, so
    /// [`Record::set_typed`] with an unchanged value writes the same bytes.
    pub fn typed(&self, ctx: Ctx) -> Option<TypedRecord> {
        let subs = self.raw_subs()?;
        TypedRecord::decode(self.header.signature.bytes(), &subs, ctx)
    }

    /// Replace the subrecords with the encoding of `typed`. Returns `false` (and changes
    /// nothing) for a compressed record that has not been decoded. Marks the record
    /// modified; the header (signature, FormID, flags) is not touched.
    pub fn set_typed(&mut self, typed: &TypedRecord) -> bool {
        self.set_owned_subs(typed.encode())
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

    /// [`Record::typed`] with this plugin's context.
    pub fn typed(&self, record: &Record<'_>) -> Option<TypedRecord> {
        record.typed(self.ctx(record))
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
    fn typed_records_decode_and_write_back_identically() {
        use crate::generated::records::Record as Typed;
        let bytes = fixture::sample_plugin();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let tes4 = plugin.typed(plugin.header()).unwrap();
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
        let typed = plugin.typed(r).unwrap();
        let Typed::Gmst(gmst) = &typed else {
            panic!("{typed:?}")
        };
        assert_eq!(gmst.editor_id.as_ref().unwrap().0, b"fFoo");
        // Undefined types have no typed form.
        let Entry::Group(cells) = &plugin.entries()[1] else {
            panic!()
        };
        let Entry::Record(cell) = &cells.children()[0] else {
            panic!()
        };
        assert!(plugin.typed(cell).is_none());

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
