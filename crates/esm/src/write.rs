//! Writer: the [`Plugin`] tree back to bytes.
//!
//! Unmodified nodes are written with the sizes they were read with, and unmodified
//! compressed records re-emit their original bytes. Sizes are recomputed only for nodes
//! marked modified, which (see [`crate::model`]) are exactly the nodes on the path to an
//! edit. Unmodified subtrees stream straight to the output; a modified group or record is
//! assembled in a buffer first so its size is known before its header is written.

use std::io::Write;

use crate::model::{Body, Encoding, Entry, Group, Plugin, Record, Subrecord, HEADER_LEN};
use crate::{Error, Result, Signature};

/// Writer settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOptions {
    /// zlib level (0-9) for compressed records whose decoded subrecords were modified.
    pub compression_level: u32,
}

impl Default for WriteOptions {
    fn default() -> WriteOptions {
        WriteOptions {
            compression_level: crate::compress::DEFAULT_LEVEL,
        }
    }
}

/// An output sink that counts bytes so write errors can report an offset.
pub(crate) struct Out<'w, W: Write + ?Sized> {
    w: &'w mut W,
    pos: u64,
}

impl<'w, W: Write + ?Sized> Out<'w, W> {
    pub(crate) fn new(w: &'w mut W, pos: u64) -> Out<'w, W> {
        Out { w, pos }
    }

    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        self.w.write_all(bytes).map_err(|source| Error::Io {
            offset: self.pos,
            source,
        })?;
        self.pos = self.pos.saturating_add(bytes.len() as u64);
        Ok(())
    }
}

impl Plugin<'_> {
    /// Write the plugin. Returns the number of bytes written.
    pub fn write_to<W: Write + ?Sized>(&self, w: &mut W) -> Result<u64> {
        self.write_with(w, &WriteOptions::default())
    }

    /// Write the plugin with explicit options. Returns the number of bytes written.
    pub fn write_with<W: Write + ?Sized>(&self, w: &mut W, options: &WriteOptions) -> Result<u64> {
        let mut out = Out::new(w, 0);
        write_record(&self.header, &mut out, options)?;
        for entry in &self.entries {
            write_entry(entry, &mut out, options)?;
        }
        Ok(out.pos)
    }

    /// Serialize the plugin into a new buffer.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.write_to(&mut buf)?;
        Ok(buf)
    }
}

impl Entry<'_> {
    /// Write this entry; `pos` is the output offset of its first byte (used in errors).
    pub fn write_to<W: Write + ?Sized>(
        &self,
        w: &mut W,
        pos: u64,
        options: &WriteOptions,
    ) -> Result<u64> {
        let mut out = Out::new(w, pos);
        write_entry(self, &mut out, options)?;
        Ok(out.pos.saturating_sub(pos))
    }
}

pub(crate) fn write_entry<W: Write + ?Sized>(
    entry: &Entry<'_>,
    out: &mut Out<'_, W>,
    options: &WriteOptions,
) -> Result<()> {
    match entry {
        Entry::Group(g) => write_group(g, out, options),
        Entry::Record(r) => write_record(r, out, options),
    }
}

fn group_header<W: Write + ?Sized>(g: &Group<'_>, size: u32, out: &mut Out<'_, W>) -> Result<()> {
    out.put(&Signature::GRUP.0)?;
    out.put(&size.to_le_bytes())?;
    out.put(&g.header.label)?;
    out.put(&g.header.group_type.to_le_bytes())?;
    out.put(&g.header.timestamp.to_le_bytes())?;
    out.put(&g.header.version_control.to_le_bytes())?;
    out.put(&g.header.unknown.to_le_bytes())
}

fn record_header<W: Write + ?Sized>(
    r: &Record<'_>,
    data_size: u32,
    out: &mut Out<'_, W>,
) -> Result<()> {
    out.put(&r.header.signature.0)?;
    out.put(&data_size.to_le_bytes())?;
    out.put(&r.header.flags.to_le_bytes())?;
    out.put(&r.header.form_id.to_le_bytes())?;
    out.put(&r.header.timestamp.to_le_bytes())?;
    out.put(&r.header.version_control.to_le_bytes())?;
    out.put(&r.header.form_version.to_le_bytes())?;
    out.put(&r.header.unknown.to_le_bytes())
}

fn write_group<W: Write + ?Sized>(
    g: &Group<'_>,
    out: &mut Out<'_, W>,
    options: &WriteOptions,
) -> Result<()> {
    if let (false, Some(size)) = (g.is_modified(), g.size) {
        group_header(g, size, out)?;
        for child in &g.children {
            write_entry(child, out, options)?;
        }
        return Ok(());
    }
    let start = out.pos;
    let mut buf = Vec::new();
    {
        let mut inner = Out::new(&mut buf, start.saturating_add(HEADER_LEN as u64));
        for child in &g.children {
            write_entry(child, &mut inner, options)?;
        }
    }
    let total = (buf.len() as u64).saturating_add(HEADER_LEN as u64);
    let size = u32::try_from(total).map_err(|_| Error::TooLarge {
        offset: start,
        what: "group",
        len: total,
    })?;
    group_header(g, size, out)?;
    out.put(&buf)
}

fn write_record<W: Write + ?Sized>(
    r: &Record<'_>,
    out: &mut Out<'_, W>,
    options: &WriteOptions,
) -> Result<()> {
    if let (false, Some(data_size)) = (r.is_modified(), r.data_size) {
        record_header(r, data_size, out)?;
        return match &r.body {
            Body::Raw(subs) => {
                for sub in subs {
                    write_subrecord(sub, out)?;
                }
                Ok(())
            }
            Body::Compressed { original, .. } => out.put(original),
        };
    }
    let start = out.pos;
    let body_start = start.saturating_add(HEADER_LEN as u64);
    let mut buf = Vec::new();
    match &r.body {
        Body::Raw(subs) => {
            let mut inner = Out::new(&mut buf, body_start);
            for sub in subs {
                write_subrecord(sub, &mut inner)?;
            }
        }
        Body::Compressed {
            decoded: Some(subs),
            ..
        } => {
            let mut fields = Vec::new();
            {
                let mut inner = Out::new(&mut fields, 0);
                for sub in subs {
                    write_subrecord(sub, &mut inner)?;
                }
            }
            buf = crate::compress::deflate(&fields, options.compression_level, start)?;
        }
        Body::Compressed {
            original,
            decoded: None,
        } => buf.extend_from_slice(original),
    }
    let len = buf.len() as u64;
    let data_size = u32::try_from(len).map_err(|_| Error::TooLarge {
        offset: start,
        what: "record data",
        len,
    })?;
    record_header(r, data_size, out)?;
    out.put(&buf)
}

pub(crate) fn write_subrecord<W: Write + ?Sized>(
    s: &Subrecord<'_>,
    out: &mut Out<'_, W>,
) -> Result<()> {
    let data: &[u8] = &s.data;
    let (size_field, oversize) = match s.encoding {
        Encoding::AsRead {
            size_field,
            oversize,
        } => (size_field, oversize),
        Encoding::Canonical => match u16::try_from(data.len()) {
            Ok(size) => (size, false),
            Err(_) => (0, true),
        },
    };
    if oversize {
        let len = data.len() as u64;
        let real = u32::try_from(len).map_err(|_| Error::TooLarge {
            offset: out.pos,
            what: "subrecord data",
            len,
        })?;
        out.put(&Signature::XXXX.0)?;
        out.put(&4u16.to_le_bytes())?;
        out.put(&real.to_le_bytes())?;
    }
    out.put(&s.signature.0)?;
    out.put(&size_field.to_le_bytes())?;
    out.put(data)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use crate::fixture::{self, group, record, sub};
    use crate::model::{Entry, GroupHeader};
    use crate::{Group, Plugin, Signature, Subrecord};

    fn group_at<'p, 'a>(entries: &'p mut [Entry<'a>], i: usize) -> &'p mut Group<'a> {
        match &mut entries[i] {
            Entry::Group(g) => g,
            Entry::Record(_) => panic!("expected group"),
        }
    }

    fn size_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap())
    }

    #[test]
    fn unmodified_plugin_writes_back_identically() {
        let bytes = fixture::sample_plugin();
        let plugin = Plugin::parse(&bytes).unwrap();
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
        let owned = plugin.into_owned();
        assert_eq!(owned.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn compressed_record_reemits_original_bytes() {
        let rec = fixture::compressed_record_stored(b"LAND", 1, &sub(b"DATA", &[5u8; 300]));
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let plugin = Plugin::parse(&bytes).unwrap();
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn touching_without_changing_still_writes_identically() {
        let bytes = fixture::sample_plugin();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        for entry in plugin.entries_mut() {
            if let Entry::Group(g) = entry {
                for child in g.children_mut() {
                    if let Entry::Record(r) = child {
                        r.body_mut();
                    }
                }
            }
        }
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn modifying_one_subrecord_fixes_sizes_up_the_group_chain() {
        let bytes = fixture::sample_plugin();
        let original = Plugin::parse(&bytes).unwrap();
        let mut plugin = original.clone();

        // CELL top group > cell children (6) > persistent (8) > REFR > NAME.
        let cells = group_at(plugin.entries_mut(), 1);
        let children = group_at(cells.children_mut(), 1);
        let persistent = group_at(children.children_mut(), 0);
        let Entry::Record(refr) = &mut persistent.children_mut()[0] else {
            panic!()
        };
        let crate::Body::Raw(subs) = refr.body_mut() else {
            panic!()
        };
        subs[0].set_data(vec![1u8; 10]); // was 4 bytes: +6
        let out = plugin.to_bytes().unwrap();
        assert_eq!(out.len(), bytes.len() + 6);

        let Entry::Group(cells_before) = &original.entries()[1] else {
            panic!()
        };
        let cells_at = cells_before.offset().unwrap() as usize;
        let Entry::Group(children_before) = &cells_before.children()[1] else {
            panic!()
        };
        let children_at = children_before.offset().unwrap() as usize;
        let Entry::Group(pers_before) = &children_before.children()[0] else {
            panic!()
        };
        let pers_at = pers_before.offset().unwrap() as usize;
        let refr_at = pers_at + 24;

        assert_eq!(size_at(&out, cells_at), size_at(&bytes, cells_at) + 6);
        assert_eq!(size_at(&out, children_at), size_at(&bytes, children_at) + 6);
        assert_eq!(size_at(&out, pers_at), size_at(&bytes, pers_at) + 6);
        assert_eq!(size_at(&out, refr_at), size_at(&bytes, refr_at) + 6);
        // Everything before the CELL group is untouched; so is everything after it.
        assert_eq!(out[..cells_at], bytes[..cells_at]);
        let cells_end = cells_at + size_at(&bytes, cells_at) as usize;
        assert_eq!(out[cells_end + 6..], bytes[cells_end..]);

        let reparsed = Plugin::parse(&out).unwrap();
        assert_eq!(reparsed.to_bytes().unwrap(), out);
    }

    #[test]
    fn oversize_prefix_is_added_and_dropped_as_needed() {
        let small = record(b"STAT", 0, 1, &sub(b"EDID", b"s\0"));
        let bytes = fixture::plugin(&[group(b"STAT", 0, &small)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let g = group_at(plugin.entries_mut(), 0);
        let Entry::Record(r) = &mut g.children_mut()[0] else {
            panic!()
        };
        let crate::Body::Raw(subs) = r.body_mut() else {
            panic!()
        };
        subs[0].set_data(vec![9u8; 70_000]);
        assert!(subs[0].has_oversize_prefix());
        let out = plugin.to_bytes().unwrap();
        let reparsed = Plugin::parse(&out).unwrap();
        let Entry::Group(g) = &reparsed.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        let s = &r.subrecords().unwrap()[0];
        assert_eq!(s.data().len(), 70_000);
        assert_eq!(s.size_field_as_read(), Some(0));
        assert!(s.has_oversize_prefix());

        // The sample's XXXX subrecord, shrunk below 64 KiB, loses its prefix.
        let bytes = fixture::sample_plugin();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let navm = group_at(plugin.entries_mut(), 2);
        let Entry::Record(r) = &mut navm.children_mut()[0] else {
            panic!()
        };
        let crate::Body::Raw(subs) = r.body_mut() else {
            panic!()
        };
        subs[0].set_data(vec![1u8; 8]);
        let out = plugin.to_bytes().unwrap();
        assert_eq!(out.len(), bytes.len() - (70_000 - 8) - 10);
        assert!(!out.windows(4).any(|w| w == b"XXXX"));
    }

    #[test]
    fn new_nodes_get_computed_sizes() {
        let bytes = fixture::plugin(&[]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let mut g = Group::new(GroupHeader {
            label: *b"KYWD",
            ..GroupHeader::default()
        });
        let header = crate::RecordHeader {
            signature: Signature::new(b"KYWD"),
            form_id: 0x0100_0800,
            form_version: 581,
            ..crate::RecordHeader::default()
        };
        let body = crate::Body::Raw(vec![Subrecord::new(Signature::new(b"EDID"), &b"kw\0"[..])]);
        g.children_mut()
            .push(Entry::Record(crate::Record::new(header, body)));
        plugin.entries_mut().push(Entry::Group(g));
        let out = plugin.to_bytes().unwrap();
        let expected = fixture::group(
            b"KYWD",
            0,
            &[
                b"KYWD".as_slice(),
                &9u32.to_le_bytes(),
                &0u32.to_le_bytes(),
                &0x0100_0800u32.to_le_bytes(),
                &0u16.to_le_bytes(),
                &0u16.to_le_bytes(),
                &581u16.to_le_bytes(),
                &0u16.to_le_bytes(),
                &sub(b"EDID", b"kw\0"),
            ]
            .concat(),
        );
        // fixture::group uses non-zero stamps; compare everything but those 8 bytes.
        let tail = &out[bytes.len()..];
        assert_eq!(tail.len(), expected.len());
        assert_eq!(tail[..16], expected[..16]);
        assert_eq!(&tail[16..24], &[0u8; 8]);
        assert_eq!(tail[24..], expected[24..]);
    }
}
