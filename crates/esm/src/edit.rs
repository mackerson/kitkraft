//! Record edit API (M2.A6): set, insert and remove subrecords of a record, compressed or
//! not.
//!
//! Every operation first makes the subrecords available: a compressed record is
//! decompressed under the caller's [`DecodeLimits`]. The writer then recompresses it
//! (zlib level [`crate::compress::DEFAULT_LEVEL`]) only when its subrecords no longer
//! serialize to the bytes they were decoded from; an unedited record, or one edited back
//! to what it was, re-emits its original bytes.
//!
//! `XXXX` policy (M2.A4, `docs/corpus.md`): a subrecord left alone keeps the encoding it
//! was read with; a new or changed payload gets an `XXXX` prefix exactly when it does not
//! fit the `u16` size field. Setting a payload to the bytes it already has changes
//! nothing.

use std::borrow::Cow;

use crate::model::{Record, Subrecord};
use crate::{DecodeLimits, Error, Result, Signature};

impl<'a> Record<'a> {
    /// The subrecords, for editing. A compressed record that is not decoded yet is
    /// decompressed first (errors as [`Record::decompress`]). Marks the record modified;
    /// that alone does not change what is written.
    pub fn edit_subrecords(&mut self, limits: &DecodeLimits) -> Result<&mut Vec<Subrecord<'a>>> {
        self.decompress(limits)?;
        let offset = self.offset().unwrap_or(0);
        self.subrecords_mut().ok_or(Error::Internal {
            offset,
            message: "a decompressed record has no subrecords",
        })
    }

    /// Position of the `nth` (0-based) subrecord with `signature`, or `None`. A compressed
    /// record must be decoded first (see [`Record::decompress`]); undecoded, it has none.
    pub fn find_subrecord(&self, signature: Signature, nth: usize) -> Option<usize> {
        self.subrecords()?
            .iter()
            .enumerate()
            .filter(|(_, s)| s.signature() == signature)
            .nth(nth)
            .map(|(i, _)| i)
    }

    /// Replace the payload of subrecord `index`. Returns whether the bytes changed: setting
    /// the payload it already has keeps the subrecord exactly as read.
    pub fn set_subrecord(
        &mut self,
        index: usize,
        data: impl Into<Cow<'a, [u8]>>,
        limits: &DecodeLimits,
    ) -> Result<bool> {
        let offset = self.offset().unwrap_or(0);
        let subs = self.edit_subrecords(limits)?;
        let len = subs.len();
        let sub = subs
            .get_mut(index)
            .ok_or(Error::NoSuchSubrecord { offset, index, len })?;
        let data = data.into();
        if sub.data() == data.as_ref() {
            return Ok(false);
        }
        sub.set_data(data);
        Ok(true)
    }

    /// Insert `sub` before subrecord `index` (`index` equal to the count appends).
    pub fn insert_subrecord(
        &mut self,
        index: usize,
        sub: Subrecord<'a>,
        limits: &DecodeLimits,
    ) -> Result<()> {
        let offset = self.offset().unwrap_or(0);
        let subs = self.edit_subrecords(limits)?;
        let len = subs.len();
        if index > len {
            return Err(Error::NoSuchSubrecord { offset, index, len });
        }
        subs.insert(index, sub);
        Ok(())
    }

    /// Remove and return subrecord `index`.
    pub fn remove_subrecord(
        &mut self,
        index: usize,
        limits: &DecodeLimits,
    ) -> Result<Subrecord<'a>> {
        let offset = self.offset().unwrap_or(0);
        let subs = self.edit_subrecords(limits)?;
        let len = subs.len();
        if index >= len {
            return Err(Error::NoSuchSubrecord { offset, index, len });
        }
        Ok(subs.remove(index))
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use crate::fixture::{self, group, record, sub, xxxx_sub};
    use crate::{DecodeLimits, Entry, Error, Plugin, Record, Signature, Subrecord};

    const EDID: Signature = Signature::new(b"EDID");
    const DATA: Signature = Signature::new(b"DATA");

    fn limits() -> DecodeLimits {
        DecodeLimits::default()
    }

    fn first<'p, 'a>(plugin: &'p mut Plugin<'a>) -> &'p mut Record<'a> {
        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
            panic!()
        };
        let Entry::Record(r) = &mut g.children_mut()[0] else {
            panic!()
        };
        r
    }

    fn reparsed_subs(bytes: &[u8]) -> Vec<(Signature, Vec<u8>, bool)> {
        let plugin = Plugin::parse(bytes).unwrap();
        let Entry::Group(g) = &plugin.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        let subs = match r.decode(&limits()).unwrap() {
            Some(decoded) => decoded,
            None => r
                .subrecords()
                .unwrap()
                .to_vec()
                .into_iter()
                .map(Subrecord::into_owned)
                .collect(),
        };
        subs.iter()
            .map(|s| (s.signature(), s.data().to_vec(), s.has_oversize_prefix()))
            .collect()
    }

    fn compressed(fields: &[u8]) -> Vec<u8> {
        let body = crate::compress::deflate(fields, 9, 0).unwrap();
        let rec = fixture::compressed_record_from_stream(
            b"LAND",
            7,
            u32::try_from(fields.len()).unwrap(),
            &body[4..],
        );
        fixture::plugin(&[group(b"LAND", 0, &rec)])
    }

    #[test]
    fn set_replaces_one_payload_and_leaves_the_rest() {
        let body = [
            sub(b"EDID", b"a\0"),
            sub(b"DATA", b"1234"),
            sub(b"DATA", b"5678"),
        ]
        .concat();
        let bytes = fixture::plugin(&[group(b"GMST", 0, &record(b"GMST", 0, 1, &body))]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first(&mut plugin);
        let at = r.find_subrecord(DATA, 1).unwrap();
        assert_eq!(at, 2);
        assert_eq!(r.find_subrecord(DATA, 2), None);
        assert!(!r.set_subrecord(at, &b"5678"[..], &limits()).unwrap());
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
        let r = first(&mut plugin);
        assert!(r.set_subrecord(at, &b"longer!"[..], &limits()).unwrap());
        let out = plugin.to_bytes().unwrap();
        assert_eq!(out.len(), bytes.len() + 3);
        let subs = reparsed_subs(&out);
        assert_eq!(subs[2].1, b"longer!");
        assert_eq!(subs[1].1, b"1234");
        assert!(matches!(
            first(&mut plugin).set_subrecord(3, &b""[..], &limits()),
            Err(Error::NoSuchSubrecord {
                index: 3,
                len: 3,
                ..
            })
        ));
    }

    #[test]
    fn insert_and_remove_shift_positions() {
        let body = [sub(b"EDID", b"a\0"), sub(b"DATA", b"1")].concat();
        let bytes = fixture::plugin(&[group(b"GMST", 0, &record(b"GMST", 0, 1, &body))]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first(&mut plugin);
        r.insert_subrecord(
            1,
            Subrecord::new(Signature::new(b"FULL"), &b"x\0"[..]),
            &limits(),
        )
        .unwrap();
        r.insert_subrecord(
            3,
            Subrecord::new(Signature::new(b"ZNAM"), &b""[..]),
            &limits(),
        )
        .unwrap();
        assert!(matches!(
            r.insert_subrecord(9, Subrecord::new(DATA, &b""[..]), &limits()),
            Err(Error::NoSuchSubrecord {
                index: 9,
                len: 4,
                ..
            })
        ));
        let out = plugin.to_bytes().unwrap();
        let sigs: Vec<String> = reparsed_subs(&out)
            .iter()
            .map(|s| s.0.to_string())
            .collect();
        assert_eq!(sigs, ["EDID", "FULL", "DATA", "ZNAM"]);

        let r = first(&mut plugin);
        let removed = r.remove_subrecord(1, &limits()).unwrap();
        assert_eq!(removed.signature(), Signature::new(b"FULL"));
        r.remove_subrecord(2, &limits()).unwrap();
        assert!(matches!(
            r.remove_subrecord(2, &limits()),
            Err(Error::NoSuchSubrecord {
                index: 2,
                len: 2,
                ..
            })
        ));
        // Back to the original subrecords: the original bytes.
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn compressed_records_are_decoded_on_edit_and_recompressed_only_when_changed() {
        let fields = [sub(b"EDID", b"land\0"), sub(b"VHGT", &[3u8; 200])].concat();
        let bytes = compressed(&fields);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first(&mut plugin);
        assert!(r.subrecords().is_none());
        assert_eq!(r.find_subrecord(EDID, 0), None);
        // An edit that changes nothing: original compressed bytes.
        assert!(!r.set_subrecord(0, &b"land\0"[..], &limits()).unwrap());
        assert_eq!(r.find_subrecord(EDID, 0), Some(0));
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
        // A real edit: recompressed at level 9, and it decompresses to the edit.
        let r = first(&mut plugin);
        r.set_subrecord(0, &b"edited\0"[..], &limits()).unwrap();
        r.insert_subrecord(2, Subrecord::new(DATA, vec![9u8; 4]), &limits())
            .unwrap();
        let out = plugin.to_bytes().unwrap();
        let reparsed = Plugin::parse(&out).unwrap();
        let Entry::Group(g) = &reparsed.entries()[0] else {
            panic!()
        };
        let Entry::Record(back) = &g.children()[0] else {
            panic!()
        };
        assert!(back.header.is_compressed());
        let crate::Body::Compressed { original, .. } = back.body() else {
            panic!()
        };
        assert_eq!(original[4..6], [0x78, 0xDA]);
        let subs = reparsed_subs(&out);
        assert_eq!(
            subs.iter().map(|s| s.1.clone()).collect::<Vec<_>>(),
            [b"edited\0".to_vec(), vec![3u8; 200], vec![9u8; 4]]
        );
        // A hostile body is an error, not a panic.
        let rec = fixture::compressed_record_stored(b"LAND", 1, b"EDID\x09\x00ab");
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        assert!(matches!(
            first(&mut plugin).set_subrecord(0, &b""[..], &limits()),
            Err(Error::InCompressed { .. })
        ));
    }

    #[test]
    fn xxxx_is_kept_when_untouched_and_chosen_by_size_when_set() {
        let body = [xxxx_sub(b"NVNM", &[1u8; 70_000]), sub(b"EDID", b"n\0")].concat();
        let bytes = fixture::plugin(&[group(b"NAVM", 0, &record(b"NAVM", 0, 1, &body))]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first(&mut plugin);
        // Editing a neighbour keeps the prefix.
        r.set_subrecord(1, &b"m\0"[..], &limits()).unwrap();
        assert!(reparsed_subs(&plugin.to_bytes().unwrap())[0].2);
        // A payload that fits a u16 loses it; one that does not keeps it.
        let r = first(&mut plugin);
        r.set_subrecord(0, vec![2u8; 100], &limits()).unwrap();
        assert!(!reparsed_subs(&plugin.to_bytes().unwrap())[0].2);
        let r = first(&mut plugin);
        r.set_subrecord(0, vec![2u8; 65_536], &limits()).unwrap();
        let subs = reparsed_subs(&plugin.to_bytes().unwrap());
        assert!(subs[0].2);
        assert_eq!(subs[0].1.len(), 65_536);
    }
}
