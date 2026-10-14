//! Compressed record bodies.
//!
//! A record with flag `0x00040000` stores a `u32` decompressed size followed by a zlib
//! stream (UESP, Skyrim_Mod:Mod_File_Format, "Records"). Decoding is on demand and bounded:
//! a declared size above [`DecodeLimits::max_decompressed`] is an error before anything is
//! allocated, and the inflater never produces more than the declared size plus one byte.

use std::io::{Read, Write};
use std::sync::Arc;

use crate::model::{Body, Data, Inflated, Record, Subrecord, HEADER_LEN};
use crate::parse::{count_subrecords, next_subrecord};
use crate::{Error, Reader, Result};

/// zlib level used when a modified compressed record is recompressed. Every compressed record
/// in the official ESMs carries zlib header `78 DA` (maximum compression); with the zlib-rs
/// backend, level 9 reproduces 99.9% of them byte for byte (docs/gaps.md).
pub const DEFAULT_LEVEL: u32 = 9;

/// Default cap on one record's decompressed size: 64 MiB.
pub const DEFAULT_MAX_DECOMPRESSED: u64 = 64 * 1024 * 1024;

/// Default cap on the number of subrecords in one decompressed record: 1,000,000. The
/// largest decompressed record in the official ESMs is 3.4 MiB, which holds at most
/// about 590,000 six-byte subrecords.
pub const DEFAULT_MAX_SUBRECORDS: u64 = 1_000_000;

/// Bounds on decompression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Largest decompressed size accepted for a single record.
    pub max_decompressed: u64,
    /// Most subrecords accepted in a single decompressed record. A small stream can
    /// inflate to megabytes of empty 6-byte subrecords; this bounds the tree built from it
    /// independently of `max_decompressed`.
    pub max_subrecords: u64,
}

impl Default for DecodeLimits {
    fn default() -> DecodeLimits {
        DecodeLimits {
            max_decompressed: DEFAULT_MAX_DECOMPRESSED,
            max_subrecords: DEFAULT_MAX_SUBRECORDS,
        }
    }
}

/// Inflate a compressed record body (`original`: `u32` size + zlib stream). `record_offset`
/// is the record's absolute offset, used in errors.
pub fn inflate(original: &[u8], record_offset: u64, limits: &DecodeLimits) -> Result<Vec<u8>> {
    if original.len() < 4 {
        return Err(Error::CompressedTooShort {
            offset: record_offset,
            len: original.len(),
        });
    }
    let mut r = Reader::new(original, record_offset.saturating_add(HEADER_LEN as u64));
    let declared = u64::from(r.u32()?);
    if declared > limits.max_decompressed {
        return Err(Error::DecompressedTooLarge {
            offset: record_offset,
            declared,
            cap: limits.max_decompressed,
        });
    }
    let stream = r.take(r.remaining())?;
    // Reserve at most 4 MiB up front; a hostile header cannot force a large allocation.
    let reserve = usize::try_from(declared.min(4 * 1024 * 1024)).unwrap_or(0);
    let mut out = Vec::with_capacity(reserve);
    flate2::read::ZlibDecoder::new(stream)
        .take(declared.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(|e| Error::Inflate {
            offset: record_offset,
            message: e.to_string(),
        })?;
    let actual = out.len() as u64;
    if actual != declared {
        return Err(Error::DecompressedSizeMismatch {
            offset: record_offset,
            declared,
            actual,
        });
    }
    Ok(out)
}

/// Compress `fields` into a record body: `u32` size + zlib stream at `level` (0-9).
pub fn deflate(fields: &[u8], level: u32, record_offset: u64) -> Result<Vec<u8>> {
    let len = fields.len() as u64;
    let declared = u32::try_from(len).map_err(|_| Error::TooLarge {
        offset: record_offset,
        what: "decompressed record data",
        len,
    })?;
    let mut out = declared.to_le_bytes().to_vec();
    let mut encoder = flate2::write::ZlibEncoder::new(out, flate2::Compression::new(level.min(9)));
    let fail = |e: std::io::Error| Error::Deflate {
        offset: record_offset,
        message: e.to_string(),
    };
    encoder.write_all(fields).map_err(fail)?;
    out = encoder.finish().map_err(fail)?;
    Ok(out)
}

/// Inflate a compressed body and parse it once into subrecords that share the inflated
/// buffer (one allocation for the data of every subrecord). The subrecords are counted
/// first, without allocating, so a body holding more than
/// [`DecodeLimits::max_subrecords`] fails before any subrecord is built.
fn decode_body(
    original: &[u8],
    record_offset: u64,
    limits: &DecodeLimits,
) -> Result<(Vec<Subrecord<'static>>, Inflated)> {
    let fields = inflate(original, record_offset, limits)?;
    let in_compressed = |e: Error| Error::InCompressed {
        offset: record_offset,
        source: Box::new(e),
    };
    let count = count_subrecords(&fields, limits.max_subrecords)
        .map_err(in_compressed)?
        .ok_or(Error::TooManySubrecords {
            offset: record_offset,
            max: limits.max_subrecords,
        })?;
    let buf = Arc::new(fields);
    let mut subs = Vec::with_capacity(count);
    let mut r = Reader::new(&buf, 0);
    while !r.is_empty() {
        let at = next_subrecord(&mut r).map_err(in_compressed)?;
        // `inflate` caps the buffer at a `u32` declared size, so both always fit.
        let too_large = || Error::TooLarge {
            offset: record_offset,
            what: "decompressed record data",
            len: buf.len() as u64,
        };
        let start = u32::try_from(at.data_offset).map_err(|_| too_large())?;
        let len = u32::try_from(at.data.len()).map_err(|_| too_large())?;
        subs.push(Subrecord::read(
            at.signature,
            Data::Shared {
                buf: Arc::clone(&buf),
                start,
                len,
            },
            at.encoding,
            None,
        ));
    }
    Ok((subs, Inflated(buf)))
}

impl<'a> Record<'a> {
    /// Decode a compressed record's subrecords without changing the record. Returns `None`
    /// for an uncompressed record. Errors inside the decompressed data are wrapped in
    /// [`Error::InCompressed`].
    pub fn decode(&self, limits: &DecodeLimits) -> Result<Option<Vec<Subrecord<'static>>>> {
        match &self.body {
            Body::Raw(_) => Ok(None),
            Body::Compressed { original, .. } => {
                decode_body(original, self.offset.unwrap_or(0), limits).map(|(subs, _)| Some(subs))
            }
        }
    }

    /// Decode a compressed record in place so [`Record::subrecords`] returns its
    /// subrecords. Does not mark the record modified: until it is reached through
    /// [`Record::subrecords_mut`] or [`Record::body_mut`], it is written with its original
    /// compressed bytes, and after that too unless its subrecords no longer serialize to
    /// the decompressed bytes.
    pub fn decompress(&mut self, limits: &DecodeLimits) -> Result<()> {
        let offset = self.offset.unwrap_or(0);
        if let Body::Compressed {
            original,
            decoded: decoded @ None,
            inflated,
        } = &mut self.body
        {
            let (subs, bytes) = decode_body(original, offset, limits)?;
            *decoded = Some(subs);
            *inflated = Some(bytes);
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{deflate, inflate, DecodeLimits};
    use crate::fixture::{self, group, sub};
    use crate::model::Entry;
    use crate::{Error, Plugin, Signature};

    fn first_record<'p, 'a>(plugin: &'p mut Plugin<'a>) -> &'p mut crate::Record<'a> {
        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
            panic!()
        };
        let Entry::Record(r) = &mut g.children_mut()[0] else {
            panic!()
        };
        r
    }

    #[test]
    fn compressed_fixture_roundtrips_and_decodes() {
        let fields = [sub(b"EDID", b"land\0"), sub(b"VHGT", &[3u8; 1000])].concat();
        for level in 0..=9 {
            let body = deflate(&fields, level, 0).unwrap();
            let stream = &body[4..];
            let rec =
                fixture::compressed_record_from_stream(b"LAND", 7, fields.len() as u32, stream);
            let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
            let mut plugin = Plugin::parse(&bytes).unwrap();
            assert_eq!(plugin.to_bytes().unwrap(), bytes);

            let r = first_record(&mut plugin);
            let decoded = r.decode(&DecodeLimits::default()).unwrap().unwrap();
            assert_eq!(decoded.len(), 2);
            assert_eq!(decoded[1].signature(), Signature::new(b"VHGT"));
            r.decompress(&DecodeLimits::default()).unwrap();
            assert_eq!(r.subrecords().unwrap().len(), 2);
            // Decoded but untouched: original bytes are re-emitted.
            assert_eq!(plugin.to_bytes().unwrap(), bytes);
        }
    }

    #[test]
    fn stored_block_stream_decodes() {
        // Over 64 KiB, so the stored stream has two blocks.
        let fields = fixture::xxxx_sub(b"DATA", &[0xAB; 70_000]);
        let rec = fixture::compressed_record_stored(b"NAVM", 1, &fields);
        let bytes = fixture::plugin(&[group(b"NAVM", 0, &rec)]);
        let plugin = Plugin::parse(&bytes).unwrap();
        let Entry::Group(g) = &plugin.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        let decoded = r.decode(&DecodeLimits::default()).unwrap().unwrap();
        assert_eq!(decoded[0].data(), &[0xAB; 70_000][..]);
        assert!(decoded[0].has_oversize_prefix());
    }

    #[test]
    fn modified_compressed_record_is_recompressed() {
        let fields = sub(b"EDID", b"before\0");
        let rec = fixture::compressed_record_stored(b"LAND", 7, &fields);
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first_record(&mut plugin);
        r.decompress(&DecodeLimits::default()).unwrap();
        r.subrecords_mut().unwrap()[0].set_data(&b"after, and longer\0"[..]);
        let out = plugin.to_bytes().unwrap();
        let reparsed = Plugin::parse(&out).unwrap();
        let Entry::Group(g) = &reparsed.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        assert!(r.header.is_compressed());
        let decoded = r.decode(&DecodeLimits::default()).unwrap().unwrap();
        assert_eq!(decoded[0].data(), b"after, and longer\0");
    }

    #[test]
    fn bomb_declared_size_is_an_error_not_an_allocation() {
        // Declares 4 GiB - 1 of output.
        let rec = fixture::compressed_record_from_stream(
            b"LAND",
            1,
            u32::MAX,
            &fixture::zlib_stored(b"x"),
        );
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let plugin = Plugin::parse(&bytes).unwrap();
        let Entry::Group(g) = &plugin.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        let at = r.offset().unwrap();
        match r.decode(&DecodeLimits::default()) {
            Err(Error::DecompressedTooLarge {
                offset,
                declared,
                cap,
            }) => {
                assert_eq!(offset, at);
                assert_eq!(declared, u64::from(u32::MAX));
                assert_eq!(cap, 64 * 1024 * 1024);
            }
            other => panic!("{other:?}"),
        }
        // A configurable cap.
        let small = DecodeLimits {
            max_decompressed: 0,
            ..DecodeLimits::default()
        };
        let rec = fixture::compressed_record_stored(b"LAND", 1, b"abc");
        assert!(matches!(
            inflate(&rec[24..], 0, &small),
            Err(Error::DecompressedTooLarge {
                declared: 3,
                cap: 0,
                ..
            })
        ));
    }

    #[test]
    fn subrecord_bomb_is_an_error_before_any_subrecord_is_built() {
        // 8 MiB of zeros deflates to about 8 KB and parses as 1,398,101 empty subrecords
        // with signature 00 00 00 00 (and 2 bytes left over that would be a parse error).
        let zeros = vec![0u8; 8 * 1024 * 1024];
        let body = deflate(&zeros, 9, 0).unwrap();
        assert!(body.len() < 16 * 1024, "{}", body.len());
        let rec =
            fixture::compressed_record_from_stream(b"LAND", 1, zeros.len() as u32, &body[4..]);
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first_record(&mut plugin);
        let at = r.offset().unwrap();
        let start = std::time::Instant::now();
        match r.decode(&DecodeLimits::default()) {
            Err(Error::TooManySubrecords { offset, max }) => {
                assert_eq!(offset, at);
                assert_eq!(max, 1_000_000);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            r.decompress(&DecodeLimits::default()),
            Err(Error::TooManySubrecords { .. })
        ));
        assert!(r.subrecords().is_none());
        // Generous even for a debug build; the old path built 1.4 M subrecords twice.
        assert!(start.elapsed().as_secs() < 10);

        // Under a lower cap a normal record fails the same way; at the exact count it
        // decodes.
        let fields = [sub(b"EDID", b"a\0"), sub(b"DATA", b"b")].concat();
        let rec = fixture::compressed_record_stored(b"LAND", 2, &fields);
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first_record(&mut plugin);
        let one = DecodeLimits {
            max_subrecords: 1,
            ..DecodeLimits::default()
        };
        assert!(matches!(
            r.decode(&one),
            Err(Error::TooManySubrecords { max: 1, .. })
        ));
        let two = DecodeLimits {
            max_subrecords: 2,
            ..DecodeLimits::default()
        };
        assert_eq!(r.decode(&two).unwrap().unwrap().len(), 2);
    }

    #[test]
    fn decoded_subrecords_share_one_buffer() {
        let fields = [sub(b"EDID", b"shared\0"), sub(b"DATA", &[9u8; 50])].concat();
        let rec = fixture::compressed_record_stored(b"LAND", 3, &fields);
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first_record(&mut plugin);
        r.decompress(&DecodeLimits::default()).unwrap();
        let crate::Body::Compressed {
            decoded: Some(subs),
            inflated: Some(inflated),
            ..
        } = r.body()
        else {
            panic!()
        };
        assert_eq!(inflated.bytes(), &fields[..]);
        // Each subrecord's data lies inside the inflated buffer.
        let range = inflated.bytes().as_ptr_range();
        for s in subs {
            assert!(range.contains(&s.data().as_ptr()));
            assert_eq!(s.offset(), None);
        }
        assert_eq!(subs[1].data(), &[9u8; 50][..]);
        // Owned copies keep sharing it.
        let owned = r.clone().into_owned();
        assert_eq!(owned.subrecords().unwrap()[0].data(), b"shared\0");
    }

    #[test]
    fn stream_longer_than_declared_is_bounded() {
        // A real 1 MiB stream that claims to inflate to 10 bytes.
        let big = vec![0u8; 1 << 20];
        let body = deflate(&big, 9, 0).unwrap();
        let mut lying = 10u32.to_le_bytes().to_vec();
        lying.extend_from_slice(&body[4..]);
        match inflate(&lying, 50, &DecodeLimits::default()) {
            Err(Error::DecompressedSizeMismatch {
                offset: 50,
                declared: 10,
                actual: 11,
            }) => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn short_and_corrupt_streams_are_errors() {
        assert!(matches!(
            inflate(&[1, 2], 9, &DecodeLimits::default()),
            Err(Error::CompressedTooShort { offset: 9, len: 2 })
        ));
        let mut body = 5u32.to_le_bytes().to_vec();
        body.extend_from_slice(&[0xFF; 16]);
        assert!(matches!(
            inflate(&body, 9, &DecodeLimits::default()),
            Err(Error::Inflate { offset: 9, .. })
        ));
        // Valid zlib, but the fields inside do not parse.
        let rec = fixture::compressed_record_stored(b"LAND", 1, b"EDID\x09\x00ab");
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let plugin = Plugin::parse(&bytes).unwrap();
        let Entry::Group(g) = &plugin.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        match r.decode(&DecodeLimits::default()) {
            Err(Error::InCompressed { offset, source }) => {
                assert_eq!(offset, r.offset().unwrap());
                assert!(matches!(*source, Error::UnexpectedEof { offset: 6, .. }));
            }
            other => panic!("{other:?}"),
        }
    }
}
