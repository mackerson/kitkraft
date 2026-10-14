//! Writer: the [`Plugin`] tree back to bytes.
//!
//! Unmodified nodes are written with the sizes they were read with, and unmodified
//! compressed records re-emit their original bytes. Sizes are recomputed only for nodes
//! marked modified, which (see [`crate::model`]) are exactly the nodes on the path to an
//! edit.
//!
//! Writing is two passes over modified nodes and one over the rest. A modified group's size
//! is computed first ([`Entry::encoded_len`]: unmodified children answer from the sizes they
//! were read with, modified ones from their own children), then its header is written and
//! its children stream straight to the output. Nothing is buffered except the body of a
//! modified compressed record, which is serialized and compressed once per write and reused
//! by every size query above it.
//!
//! A modified compressed record whose subrecords still serialize to exactly the bytes they
//! were decoded from re-emits its original compressed bytes, so touching a record without
//! changing it never changes the file.

use std::collections::HashMap;
use std::io::Write;

use crate::model::{Body, Encoding, Entry, Group, Inflated, Plugin, Record, Subrecord, HEADER_LEN};
use crate::{Error, Result, Signature, FLAG_COMPRESSED};

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
    pub(crate) pos: u64,
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
    ///
    /// The `HEDR` record count is written as read unless records or groups were added or
    /// removed (see [`crate::tes4`]).
    pub fn write_to<W: Write + ?Sized>(&self, w: &mut W) -> Result<u64> {
        self.write_with(w, &WriteOptions::default())
    }

    /// Write the plugin with explicit options. Returns the number of bytes written.
    pub fn write_with<W: Write + ?Sized>(&self, w: &mut W, options: &WriteOptions) -> Result<u64> {
        let mut out = Out::new(w, 0);
        let mut writer = Writer::new(options);
        let header = self.header_for_write()?;
        writer.write_record(&header, &mut out)?;
        for entry in &self.entries {
            writer.write_entry(entry, &mut out)?;
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
    /// Returns the number of bytes written.
    pub fn write_to<W: Write + ?Sized>(
        &self,
        w: &mut W,
        pos: u64,
        options: &WriteOptions,
    ) -> Result<u64> {
        let mut out = Out::new(w, pos);
        Writer::new(options).write_entry(self, &mut out)?;
        Ok(out.pos.saturating_sub(pos))
    }

    /// Number of bytes [`Entry::write_to`] would write, header included. Unmodified nodes
    /// answer from the sizes they were read with; a modified compressed record is
    /// serialized and compressed to find out.
    pub fn encoded_len(&self, options: &WriteOptions) -> Result<u64> {
        Writer::new(options).entry_len(self, 0)
    }
}

/// The body to emit for a modified, decoded compressed record.
enum Packed {
    /// Its subrecords serialize to the bytes they were decoded from: emit `original`.
    Original,
    /// Recompressed bytes.
    Recompressed(Vec<u8>),
}

/// State of one write: options and memoized sizes of modified nodes, so each modified node
/// is measured (and each modified compressed record compressed) once, however deep it
/// sits. Nodes are keyed by address; the tree is borrowed immutably for the whole write.
struct Writer<'o> {
    options: &'o WriteOptions,
    group_lens: HashMap<usize, u64>,
    body_lens: HashMap<usize, u64>,
    packed: HashMap<usize, Packed>,
}

fn key<T>(node: &T) -> usize {
    std::ptr::from_ref(node).addr()
}

impl<'o> Writer<'o> {
    fn new(options: &'o WriteOptions) -> Writer<'o> {
        Writer {
            options,
            group_lens: HashMap::new(),
            body_lens: HashMap::new(),
            packed: HashMap::new(),
        }
    }

    /// Size of `entry` when written at output offset `pos`.
    fn entry_len(&mut self, entry: &Entry<'_>, pos: u64) -> Result<u64> {
        match entry {
            Entry::Group(g) => self.group_len(g, pos),
            Entry::Record(r) => self.record_len(r, pos),
        }
    }

    fn group_len(&mut self, g: &Group<'_>, pos: u64) -> Result<u64> {
        if let (false, Some(size)) = (g.is_modified(), g.size) {
            return Ok(u64::from(size));
        }
        if let Some(len) = self.group_lens.get(&key(g)) {
            return Ok(*len);
        }
        let mut total = HEADER_LEN as u64;
        for child in &g.children {
            let len = self.entry_len(child, pos.saturating_add(total))?;
            total = total.saturating_add(len);
        }
        self.group_lens.insert(key(g), total);
        Ok(total)
    }

    fn record_len(&mut self, r: &Record<'_>, pos: u64) -> Result<u64> {
        check_flag(r, pos)?;
        if let (false, Some(data_size)) = (r.is_modified(), r.data_size) {
            return Ok(u64::from(data_size).saturating_add(HEADER_LEN as u64));
        }
        let body = self.body_len(r, pos)?;
        Ok(body.saturating_add(HEADER_LEN as u64))
    }

    /// Data size of a modified record.
    fn body_len(&mut self, r: &Record<'_>, pos: u64) -> Result<u64> {
        if let Some(len) = self.body_lens.get(&key(r)) {
            return Ok(*len);
        }
        let len = match &r.body {
            Body::Raw(subs) => {
                let mut total: u64 = 0;
                for s in subs {
                    check_subrecord(s, pos)?;
                    total = total.saturating_add(s.encoded_len());
                }
                total
            }
            Body::Compressed {
                original,
                decoded: None,
                ..
            } => original.len() as u64,
            Body::Compressed {
                original,
                decoded: Some(subs),
                inflated,
            } => {
                let packed = self.pack(subs, inflated.as_ref(), pos)?;
                let len = match &packed {
                    Packed::Original => original.len(),
                    Packed::Recompressed(bytes) => bytes.len(),
                } as u64;
                self.packed.insert(key(r), packed);
                len
            }
        };
        self.body_lens.insert(key(r), len);
        Ok(len)
    }

    /// Serialize decoded subrecords; recompress unless they reproduce `inflated`.
    fn pack(
        &self,
        subs: &[Subrecord<'_>],
        inflated: Option<&Inflated>,
        pos: u64,
    ) -> Result<Packed> {
        let total = subs
            .iter()
            .fold(0u64, |acc, s| acc.saturating_add(s.encoded_len()));
        // Only a capacity hint: the size comes from the tree, not from input bytes.
        let mut fields = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
        {
            let mut inner = Out::new(&mut fields, pos);
            for s in subs {
                write_subrecord(s, &mut inner)?;
            }
        }
        if inflated.is_some_and(|i| i.bytes() == fields.as_slice()) {
            return Ok(Packed::Original);
        }
        crate::compress::deflate(&fields, self.options.compression_level, pos)
            .map(Packed::Recompressed)
    }

    fn write_entry<W: Write + ?Sized>(
        &mut self,
        entry: &Entry<'_>,
        out: &mut Out<'_, W>,
    ) -> Result<()> {
        match entry {
            Entry::Group(g) => self.write_group(g, out),
            Entry::Record(r) => self.write_record(r, out),
        }
    }

    fn write_group<W: Write + ?Sized>(
        &mut self,
        g: &Group<'_>,
        out: &mut Out<'_, W>,
    ) -> Result<()> {
        let size = match (g.is_modified(), g.size) {
            (false, Some(size)) => size,
            _ => {
                let len = self.group_len(g, out.pos)?;
                u32::try_from(len).map_err(|_| Error::TooLarge {
                    offset: out.pos,
                    what: "group",
                    len,
                })?
            }
        };
        group_header(g, size, out)?;
        for child in &g.children {
            self.write_entry(child, out)?;
        }
        Ok(())
    }

    fn write_record<W: Write + ?Sized>(
        &mut self,
        r: &Record<'_>,
        out: &mut Out<'_, W>,
    ) -> Result<()> {
        check_flag(r, out.pos)?;
        if let (false, Some(data_size)) = (r.is_modified(), r.data_size) {
            record_header(r, data_size, out)?;
            return match &r.body {
                Body::Raw(subs) => subs.iter().try_for_each(|s| write_subrecord(s, out)),
                Body::Compressed { original, .. } => out.put(original),
            };
        }
        let len = self.body_len(r, out.pos)?;
        let data_size = u32::try_from(len).map_err(|_| Error::TooLarge {
            offset: out.pos,
            what: "record data",
            len,
        })?;
        record_header(r, data_size, out)?;
        match &r.body {
            Body::Raw(subs) => subs.iter().try_for_each(|s| write_subrecord(s, out)),
            Body::Compressed {
                original,
                decoded: None,
                ..
            } => out.put(original),
            Body::Compressed {
                original,
                decoded: Some(_),
                ..
            } => match self.packed.get(&key(r)) {
                Some(Packed::Recompressed(bytes)) => out.put(bytes),
                Some(Packed::Original) | None => out.put(original),
            },
        }
    }
}

/// The compressed flag must match the body variant.
fn check_flag(r: &Record<'_>, pos: u64) -> Result<()> {
    let flag_set = r.header.flags & FLAG_COMPRESSED != 0;
    let compressed = matches!(r.body, Body::Compressed { .. });
    if flag_set == compressed {
        Ok(())
    } else {
        Err(Error::FlagBodyMismatch {
            offset: pos,
            flag_set,
        })
    }
}

/// A subrecord built or changed in memory may not be called `XXXX`.
fn check_subrecord(s: &Subrecord<'_>, pos: u64) -> Result<()> {
    match s.encoding {
        Encoding::Canonical { .. } if s.signature == Signature::XXXX => {
            Err(Error::ReservedSignature { offset: pos })
        }
        _ => Ok(()),
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

pub(crate) fn write_subrecord<W: Write + ?Sized>(
    s: &Subrecord<'_>,
    out: &mut Out<'_, W>,
) -> Result<()> {
    check_subrecord(s, out.pos)?;
    let data = s.data();
    let (size_field, oversize) = match s.encoding {
        Encoding::AsRead {
            size_field,
            oversize,
        } => (size_field, oversize),
        Encoding::Canonical { oversize } => match u16::try_from(data.len()) {
            Ok(size) if !oversize => (size, false),
            _ => (0, true),
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
    use crate::{
        Body, DecodeLimits, Error, Group, Plugin, Record, Signature, Subrecord, WriteOptions,
        FLAG_COMPRESSED,
    };

    fn group_at<'p, 'a>(entries: &'p mut [Entry<'a>], i: usize) -> &'p mut Group<'a> {
        match &mut entries[i] {
            Entry::Group(g) => g,
            Entry::Record(_) => panic!("expected group"),
        }
    }

    fn first_record<'p, 'a>(plugin: &'p mut Plugin<'a>) -> &'p mut Record<'a> {
        let g = group_at(plugin.entries_mut(), 0);
        match &mut g.children_mut()[0] {
            Entry::Record(r) => r,
            Entry::Group(_) => panic!("expected record"),
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

        // The same for a decoded compressed record. Its stream uses stored blocks, which no
        // compressor level reproduces, so recompressing would change the bytes.
        let fields = [
            sub(b"EDID", b"land\0"),
            fixture::xxxx_sub(b"VHGT", &[4u8; 90]),
        ]
        .concat();
        let rec = fixture::compressed_record_stored(b"LAND", 7, &fields);
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first_record(&mut plugin);
        r.decompress(&DecodeLimits::default()).unwrap();
        r.subrecords_mut().unwrap();
        r.body_mut();
        assert!(r.is_modified());
        assert_eq!(plugin.to_bytes().unwrap(), bytes);

        // Changed and changed back is unchanged too.
        let r = first_record(&mut plugin);
        let subs = r.subrecords_mut().unwrap();
        subs[0].set_data(&b"other\0"[..]);
        assert_ne!(plugin.to_bytes().unwrap(), bytes);
        let r = first_record(&mut plugin);
        r.subrecords_mut().unwrap()[0].set_data(&b"land\0"[..]);
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn compressed_flag_on_a_raw_body_is_an_error() {
        let bytes = fixture::sample_plugin();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let at = first_record(&mut plugin).offset().unwrap();
        // Not marked modified: the header is a plain field.
        first_record(&mut plugin).header.flags |= FLAG_COMPRESSED;
        match plugin.to_bytes() {
            Err(Error::FlagBodyMismatch {
                offset,
                flag_set: true,
            }) => assert_eq!(offset, at),
            other => panic!("{other:?}"),
        }
        // Marked modified: the same.
        first_record(&mut plugin).body_mut();
        assert!(matches!(
            plugin.to_bytes(),
            Err(Error::FlagBodyMismatch { flag_set: true, .. })
        ));
        first_record(&mut plugin).header.flags &= !FLAG_COMPRESSED;
        assert_eq!(plugin.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn swapping_a_body_variant_requires_the_matching_flag() {
        // Compressed -> Raw: the flag must be cleared.
        let fields = [sub(b"EDID", b"land\0"), sub(b"VHGT", &[4u8; 90])].concat();
        let rec = fixture::compressed_record_stored(b"LAND", 7, &fields);
        let bytes = fixture::plugin(&[group(b"LAND", 0, &rec)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let r = first_record(&mut plugin);
        let subs = r.decode(&DecodeLimits::default()).unwrap().unwrap();
        *r.body_mut() = Body::Raw(subs);
        assert!(matches!(
            plugin.to_bytes(),
            Err(Error::FlagBodyMismatch { flag_set: true, .. })
        ));
        first_record(&mut plugin).header.flags &= !FLAG_COMPRESSED;
        let out = plugin.to_bytes().unwrap();
        let mut reparsed = Plugin::parse(&out).unwrap();
        let r = first_record(&mut reparsed);
        assert!(!r.header.is_compressed());
        assert_eq!(r.subrecords().unwrap()[1].data(), &[4u8; 90][..]);

        // Raw -> Compressed: the flag must be set.
        let r = first_record(&mut reparsed);
        let original = crate::compress::deflate(&fields, 9, 0).unwrap();
        *r.body_mut() = Body::Compressed {
            original: original.into(),
            decoded: None,
            inflated: None,
        };
        assert!(matches!(
            reparsed.to_bytes(),
            Err(Error::FlagBodyMismatch {
                flag_set: false,
                ..
            })
        ));
        first_record(&mut reparsed).header.flags |= FLAG_COMPRESSED;
        let out = reparsed.to_bytes().unwrap();
        let again = Plugin::parse(&out).unwrap();
        let Entry::Group(g) = &again.entries()[0] else {
            panic!()
        };
        let Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        let decoded = r.decode(&DecodeLimits::default()).unwrap().unwrap();
        assert_eq!(decoded[0].data(), b"land\0");
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

        // The size pass agrees with what is written, for the edited group and the rest.
        let options = WriteOptions::default();
        for entry in plugin.entries() {
            let mut buf = Vec::new();
            let written = entry.write_to(&mut buf, 0, &options).unwrap();
            assert_eq!(entry.encoded_len(&options).unwrap(), written);
            assert_eq!(buf.len() as u64, written);
        }

        let reparsed = Plugin::parse(&out).unwrap();
        assert_eq!(reparsed.to_bytes().unwrap(), out);
    }

    #[test]
    fn edited_compressed_record_deep_in_groups_gets_consistent_sizes() {
        let fields = sub(b"EDID", b"deep\0");
        let land = fixture::compressed_record_stored(b"LAND", 0x10, &fields);
        let inner = group(&0x20u32.to_le_bytes(), 9, &land);
        let mid = group(&0x20u32.to_le_bytes(), 6, &inner);
        let bytes = fixture::plugin(&[group(b"CELL", 0, &mid)]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let top = group_at(plugin.entries_mut(), 0);
        let mid = group_at(top.children_mut(), 0);
        let inner = group_at(mid.children_mut(), 0);
        let Entry::Record(r) = &mut inner.children_mut()[0] else {
            panic!()
        };
        r.decompress(&DecodeLimits::default()).unwrap();
        r.subrecords_mut().unwrap()[0].set_data(&b"deeper and longer\0"[..]);
        let out = plugin.to_bytes().unwrap();
        // Every group size and the record size describe what follows them.
        let reparsed = Plugin::parse(&out).unwrap();
        assert_eq!(reparsed.to_bytes().unwrap(), out);
        let Entry::Group(top) = &reparsed.entries()[0] else {
            panic!()
        };
        let tes4_len = fixture::plugin(&[]).len();
        assert_eq!(top.size_as_read().unwrap() as usize, out.len() - tes4_len);
        assert!(out.len() > bytes.len());
        let Entry::Group(mid) = &top.children()[0] else {
            panic!()
        };
        let Entry::Group(inner) = &mid.children()[0] else {
            panic!()
        };
        let Entry::Record(r) = &inner.children()[0] else {
            panic!()
        };
        let decoded = r.decode(&DecodeLimits::default()).unwrap().unwrap();
        assert_eq!(decoded[0].data(), b"deeper and longer\0");
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

        // ... unless the prefix is forced.
        let navm = group_at(plugin.entries_mut(), 2);
        let Entry::Record(r) = &mut navm.children_mut()[0] else {
            panic!()
        };
        r.subrecords_mut().unwrap()[0].set_oversize_prefix(true);
        let out = plugin.to_bytes().unwrap();
        let mut expected = b"XXXX".to_vec();
        expected.extend_from_slice(&4u16.to_le_bytes());
        expected.extend_from_slice(&8u32.to_le_bytes());
        expected.extend_from_slice(b"NVNM\0\0");
        assert!(out.windows(expected.len()).any(|w| w == expected));
    }

    #[test]
    fn a_built_xxxx_subrecord_is_rejected() {
        let bytes = fixture::sample_plugin();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        first_record(&mut plugin)
            .subrecords_mut()
            .unwrap()
            .push(Subrecord::new(Signature::XXXX, &[1u8, 0, 0, 0][..]));
        assert!(matches!(
            plugin.to_bytes(),
            Err(Error::ReservedSignature { .. })
        ));
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
