//! Parser: bytes to the [`Plugin`] tree.
//!
//! Layout (UESP, Skyrim_Mod:Mod_File_Format, which the Starfield page defers to):
//! - record header, 24 bytes: signature, `u32` data size (excluding the header), `u32`
//!   flags, `u32` FormID, `u16` timestamp, `u16` version control info, `u16` form version,
//!   `u16` unknown;
//! - group header, 24 bytes: `GRUP`, `u32` size (including the header), 4-byte label,
//!   `i32` group type, `u16` timestamp, `u16` version control info, `u32` unknown;
//! - subrecord header, 6 bytes: signature, `u16` size. After an `XXXX` subrecord (whose
//!   4-byte data is a `u32`), the next subrecord's data length is that `u32` and its own
//!   `u16` size field is 0.

use std::borrow::Cow;

use crate::model::{
    Body, Encoding, Entry, Group, GroupHeader, Plugin, Record, RecordHeader, Subrecord, HEADER_LEN,
};
use crate::{Error, Reader, Result, Signature};

/// Parser limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    /// Deepest group nesting accepted; a top-level group is depth 1. Official files nest
    /// at most 6 deep (world > block > sub-block > cell children > persistent/temporary).
    pub max_depth: usize,
}

impl Default for ParseOptions {
    fn default() -> ParseOptions {
        ParseOptions { max_depth: 16 }
    }
}

impl<'a> Plugin<'a> {
    /// Parse a whole plugin held in memory, with default limits.
    pub fn parse(data: &'a [u8]) -> Result<Plugin<'a>> {
        Plugin::parse_with(data, &ParseOptions::default())
    }

    /// Parse a whole plugin held in memory.
    pub fn parse_with(data: &'a [u8], options: &ParseOptions) -> Result<Plugin<'a>> {
        let mut r = Reader::new(data, 0);
        let header = parse_header_record(&mut r)?;
        let entries = parse_entries(&mut r, 1, options)?;
        Ok(Plugin { header, entries })
    }
}

impl<'a> Entry<'a> {
    /// Parse exactly one entry (a group or a record) filling `data`, whose first byte sits
    /// at absolute offset `base`. `depth` is 1 for a top-level entry.
    pub fn parse_one(
        data: &'a [u8],
        base: u64,
        depth: usize,
        options: &ParseOptions,
    ) -> Result<Entry<'a>> {
        let mut r = Reader::new(data, base);
        let entry = parse_entry(&mut r, depth, options)?;
        if !r.is_empty() {
            return Err(Error::UnexpectedEof {
                offset: r.offset(),
                what: "end of entry",
                needed: 0,
                available: r.remaining() as u64,
            });
        }
        Ok(entry)
    }
}

/// Parse the leading `TES4` record.
pub(crate) fn parse_header_record<'a>(r: &mut Reader<'a>) -> Result<Record<'a>> {
    let offset = r.offset();
    let found = r.peek_signature()?;
    if found != Signature::TES4 {
        return Err(Error::NotAPlugin { offset, found });
    }
    parse_record(r)
}

/// Parse entries until the reader is exhausted.
pub(crate) fn parse_entries<'a>(
    r: &mut Reader<'a>,
    depth: usize,
    options: &ParseOptions,
) -> Result<Vec<Entry<'a>>> {
    let mut entries = Vec::new();
    while !r.is_empty() {
        entries.push(parse_entry(r, depth, options)?);
    }
    Ok(entries)
}

fn parse_entry<'a>(r: &mut Reader<'a>, depth: usize, options: &ParseOptions) -> Result<Entry<'a>> {
    if r.peek_signature()? == Signature::GRUP {
        parse_group(r, depth, options).map(Entry::Group)
    } else {
        parse_record(r).map(Entry::Record)
    }
}

fn parse_group<'a>(r: &mut Reader<'a>, depth: usize, options: &ParseOptions) -> Result<Group<'a>> {
    let offset = r.offset();
    if depth > options.max_depth {
        return Err(Error::TooDeep {
            offset,
            max: options.max_depth,
        });
    }
    let _grup = r.signature()?;
    let size = r.u32()?;
    let header = GroupHeader {
        label: r.array::<4>("group label")?,
        group_type: r.i32()?,
        timestamp: r.u16()?,
        version_control: r.u16()?,
        unknown: r.u32()?,
    };
    let content_len = usize::try_from(size)
        .ok()
        .and_then(|s| s.checked_sub(HEADER_LEN))
        .ok_or(Error::GroupTooSmall { offset, size })?;
    let content_offset = r.offset();
    let content = r.take_labeled(content_len, "group contents")?;
    let mut inner = Reader::new(content, content_offset);
    let children = parse_entries(&mut inner, depth.saturating_add(1), options)?;
    Ok(Group {
        header,
        size: Some(size),
        offset: Some(offset),
        children,
        modified: false,
    })
}

pub(crate) fn parse_record<'a>(r: &mut Reader<'a>) -> Result<Record<'a>> {
    let offset = r.offset();
    let signature = r.signature()?;
    let data_size = r.u32()?;
    let header = RecordHeader {
        signature,
        flags: r.u32()?,
        form_id: r.u32()?,
        timestamp: r.u16()?,
        version_control: r.u16()?,
        form_version: r.u16()?,
        unknown: r.u16()?,
    };
    let data_offset = r.offset();
    let len = usize::try_from(data_size).map_err(|_| Error::UnexpectedEof {
        offset: data_offset,
        what: "record data",
        needed: u64::from(data_size),
        available: r.remaining() as u64,
    })?;
    let data = r.take_labeled(len, "record data")?;
    let body = if header.is_compressed() {
        Body::Compressed {
            original: Cow::Borrowed(data),
            decoded: None,
        }
    } else {
        Body::Raw(parse_subrecords(data, data_offset)?)
    };
    Ok(Record {
        header,
        data_size: Some(data_size),
        offset: Some(offset),
        body,
        modified: false,
    })
}

/// Parse a record body (`data`, starting at absolute offset `base`) into subrecords.
pub(crate) fn parse_subrecords(data: &[u8], base: u64) -> Result<Vec<Subrecord<'_>>> {
    let mut r = Reader::new(data, base);
    let mut subs = Vec::new();
    while !r.is_empty() {
        let offset = r.offset();
        let signature = r.signature()?;
        let size = r.u16()?;
        if signature == Signature::XXXX {
            if size != 4 {
                return Err(Error::BadOversize { offset, size });
            }
            let real_size = r.u32()?;
            if r.is_empty() {
                return Err(Error::DanglingOversize { offset });
            }
            let signature = r.signature()?;
            let size_field = r.u16()?;
            let len = usize::try_from(real_size).map_err(|_| Error::UnexpectedEof {
                offset: r.offset(),
                what: "oversize subrecord data",
                needed: u64::from(real_size),
                available: r.remaining() as u64,
            })?;
            let data = r.take_labeled(len, "oversize subrecord data")?;
            subs.push(Subrecord {
                signature,
                data: Cow::Borrowed(data),
                encoding: Encoding::AsRead {
                    size_field,
                    oversize: true,
                },
            });
        } else {
            let data = r.take_labeled(usize::from(size), "subrecord data")?;
            subs.push(Subrecord {
                signature,
                data: Cow::Borrowed(data),
                encoding: Encoding::AsRead {
                    size_field: size,
                    oversize: false,
                },
            });
        }
    }
    Ok(subs)
}

#[cfg(test)]
mod tests {
    use crate::fixture::{self, group, record, sub, xxxx_sub};
    use crate::model::{Body, Entry};
    use crate::{Error, ParseOptions, Plugin, Signature};

    #[test]
    fn sample_plugin_parses_to_the_expected_tree() {
        let bytes = fixture::sample_plugin();
        let plugin = Plugin::parse(&bytes).unwrap();

        let tes4 = plugin.header();
        assert_eq!(tes4.header.signature, Signature::TES4);
        assert_eq!(tes4.header.flags, 0x81);
        assert_eq!(tes4.offset(), Some(0));
        let subs = tes4.subrecords().unwrap();
        let sigs: Vec<String> = subs.iter().map(|s| s.signature().to_string()).collect();
        assert_eq!(
            sigs,
            ["HEDR", "CNAM", "MAST", "DATA", "MAST", "DATA", "ONAM"]
        );

        let entries = plugin.entries();
        assert_eq!(entries.len(), 3);
        let Entry::Group(gmst) = &entries[0] else {
            panic!("expected GMST group")
        };
        assert_eq!(gmst.header.label_signature(), Signature::new(b"GMST"));
        assert_eq!(gmst.header.group_type, 0);
        assert_eq!(gmst.children().len(), 2);
        let Entry::Record(first) = &gmst.children()[0] else {
            panic!("expected record")
        };
        assert_eq!(first.header.form_id, 0x0000_0100);
        assert_eq!(first.header.timestamp, 0x1234);
        assert_eq!(first.header.version_control, 0x5678);
        assert_eq!(first.header.form_version, 581);
        assert_eq!(first.header.unknown, 7);
        assert_eq!(first.offset(), Some(gmst.offset().unwrap() + 24));

        // CELL > cell record + cell children (6) > persistent (8) + temporary (9).
        let Entry::Group(cells) = &entries[1] else {
            panic!("expected CELL group")
        };
        assert_eq!(cells.children().len(), 2);
        let Entry::Group(children) = &cells.children()[1] else {
            panic!("expected cell children")
        };
        assert_eq!(children.header.group_type, 6);
        assert_eq!(children.header.label_u32(), 0x0000_0200);
        let types: Vec<i32> = children
            .children()
            .iter()
            .map(|e| match e {
                Entry::Group(g) => g.header.group_type,
                Entry::Record(_) => -1,
            })
            .collect();
        assert_eq!(types, [8, 9]);

        // The XXXX prefix is folded into the subrecord it sizes.
        let Entry::Group(navm) = &entries[2] else {
            panic!("expected NAVM group")
        };
        let Entry::Record(big) = &navm.children()[0] else {
            panic!("expected record")
        };
        let subs = big.subrecords().unwrap();
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].signature(), Signature::new(b"NVNM"));
        assert_eq!(subs[0].data().len(), 70_000);
        assert!(subs[0].has_oversize_prefix());
        assert_eq!(subs[0].size_field_as_read(), Some(0));
        assert_eq!(subs[1].signature(), Signature::new(b"EDID"));
        assert!(!subs[1].has_oversize_prefix());
    }

    #[test]
    fn compressed_records_are_kept_undecoded() {
        let body = sub(b"EDID", b"x\0");
        let rec = fixture::compressed_record_stored(b"LAND", 0x10, &body);
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let plugin = Plugin::parse(&bytes).unwrap();
        let Entry::Group(g) = &plugin.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        assert!(r.header.is_compressed());
        assert!(r.subrecords().is_none());
        let Body::Compressed { original, decoded } = r.body() else {
            panic!()
        };
        assert_eq!(original.len(), rec.len() - 24);
        assert!(decoded.is_none());
    }

    #[test]
    fn first_record_must_be_tes4() {
        let bytes = record(b"WEAP", 0, 1, &[]);
        match Plugin::parse(&bytes) {
            Err(Error::NotAPlugin { offset: 0, found }) => {
                assert_eq!(found, Signature::new(b"WEAP"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nesting_is_capped() {
        let mut inner = record(b"REFR", 0, 9, &sub(b"EDID", b"a\0"));
        for _ in 0..20 {
            inner = group(b"\0\0\0\0", 6, &inner);
        }
        let bytes = fixture::plugin(&[inner]);
        let options = ParseOptions { max_depth: 16 };
        match Plugin::parse_with(&bytes, &options) {
            Err(Error::TooDeep { offset, max: 16 }) => {
                // The 17th group header: TES4 record plus 16 group headers in.
                let tes4_len = fixture::plugin(&[]).len() as u64;
                assert_eq!(offset, tes4_len + 16 * 24);
            }
            other => panic!("{other:?}"),
        }
        let options = ParseOptions { max_depth: 20 };
        assert!(Plugin::parse_with(&bytes, &options).is_ok());
    }

    #[test]
    fn group_smaller_than_its_header_is_an_error() {
        let mut g = group(b"GMST", 0, &[]);
        g[4..8].copy_from_slice(&10u32.to_le_bytes());
        let bytes = fixture::plugin(&[g]);
        let at = fixture::plugin(&[]).len() as u64;
        match Plugin::parse(&bytes) {
            Err(Error::GroupTooSmall { offset, size: 10 }) => assert_eq!(offset, at),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn record_overrunning_its_group_is_an_error_at_the_data_offset() {
        let rec = record(b"GMST", 0, 1, &sub(b"EDID", b"abc\0"));
        let mut g = group(b"GMST", 0, &rec);
        // Shrink the group by 2 bytes so the record data no longer fits.
        let size = u32::from_le_bytes(g[4..8].try_into().unwrap()) - 2;
        g[4..8].copy_from_slice(&size.to_le_bytes());
        g.truncate(g.len() - 2);
        let bytes = fixture::plugin(&[g]);
        let at = fixture::plugin(&[]).len() as u64;
        match Plugin::parse(&bytes) {
            Err(Error::UnexpectedEof {
                offset,
                what: "record data",
                needed: 10,
                available: 8,
            }) => assert_eq!(offset, at + 24 + 24),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn xxxx_errors() {
        let mut bad = Vec::new();
        bad.extend_from_slice(b"XXXX");
        bad.extend_from_slice(&3u16.to_le_bytes());
        bad.extend_from_slice(&[1, 2, 3]);
        let bytes = fixture::plugin(&[group(b"NAVM", 0, &record(b"NAVM", 0, 1, &bad))]);
        assert!(matches!(
            Plugin::parse(&bytes),
            Err(Error::BadOversize { size: 3, .. })
        ));

        let mut dangling = Vec::new();
        dangling.extend_from_slice(b"XXXX");
        dangling.extend_from_slice(&4u16.to_le_bytes());
        dangling.extend_from_slice(&9u32.to_le_bytes());
        let bytes = fixture::plugin(&[group(b"NAVM", 0, &record(b"NAVM", 0, 1, &dangling))]);
        assert!(matches!(
            Plugin::parse(&bytes),
            Err(Error::DanglingOversize { .. })
        ));

        // XXXX declares more data than the record holds.
        let mut short = xxxx_sub(b"NVNM", &[0u8; 100]);
        short.truncate(short.len() - 1);
        let bytes = fixture::plugin(&[group(b"NAVM", 0, &record(b"NAVM", 0, 1, &short))]);
        assert!(matches!(
            Plugin::parse(&bytes),
            Err(Error::UnexpectedEof {
                what: "oversize subrecord data",
                needed: 100,
                available: 99,
                ..
            })
        ));
    }

    #[test]
    fn parse_one_requires_exactly_one_entry() {
        let rec = record(b"GMST", 0, 1, &sub(b"EDID", b"a\0"));
        let mut two = rec.clone();
        two.extend_from_slice(&rec);
        let opts = ParseOptions::default();
        assert!(Entry::parse_one(&rec, 100, 1, &opts).is_ok());
        match Entry::parse_one(&two, 100, 1, &opts) {
            Err(Error::UnexpectedEof { offset, .. }) => {
                assert_eq!(offset, 100 + rec.len() as u64);
            }
            other => panic!("{other:?}"),
        }
    }
}
