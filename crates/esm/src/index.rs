//! Record index (M2.A1) and random access (M2.A2).
//!
//! [`index_file`] streams a plugin and records where every record and group is, without
//! building the tree: one [`RecordLoc`] per record (signature, FormID, flags, offset, data
//! size, enclosing group, EditorID) and one [`GroupLoc`] per group. Memory is the index
//! itself plus one record's bytes at a time.
//!
//! [`read_record_at`] and [`read_indexed`] then read single records back by offset, with
//! their absolute offsets, optionally decompressed.
//!
//! The EditorID is the payload of the record's first `EDID` subrecord with one trailing
//! NUL removed if there is one (`EDID` is a zstring: UESP, Skyrim_Mod:Mod_File_Format,
//! "Data types"); other bytes are kept as they are. For a compressed
//! record the zlib stream is inflated only until that subrecord has been read, under
//! [`DecodeLimits`]; a compressed record without `EDID` is inflated to its end (streamed,
//! never held whole).
//!
//! Validation: every group and record must fit its parent and the file, groups nest at
//! most [`ParseOptions::effective_max_depth`] deep, and the subrecords of uncompressed
//! records must parse. A plugin that indexes also parses with [`crate::Plugin::parse`].
//! Compressed bodies are checked only as far as they are inflated.

use std::borrow::Cow;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::model::{GroupHeader, Record, HEADER_LEN, SUBRECORD_HEADER_LEN};
use crate::parse::{next_subrecord, parse_record};
use crate::{DecodeLimits, Error, ParseOptions, Reader, Result, Signature};

/// Version of the [`PluginIndex`] layout. Caches of serialized indexes (M2.D5) key on it;
/// it changes whenever a field is added or its meaning changes.
pub const INDEX_FORMAT_VERSION: u32 = 1;

const EDID: Signature = Signature::new(b"EDID");

/// Index settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexOptions {
    /// Nesting limit.
    pub parse: ParseOptions,
    /// Bounds on inflating compressed records to find their `EDID`.
    pub decode: DecodeLimits,
}

/// Where a group is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupLoc {
    /// Header fields (label, type, stamps, unknown).
    pub header: GroupHeader,
    /// Absolute offset of the group header.
    pub offset: u64,
    /// Size field: the group's length including its 24-byte header.
    pub size: u32,
    /// Index in [`PluginIndex::groups`] of the enclosing group; `None` at the top level.
    pub parent: Option<u32>,
    /// Nesting depth: 1 for a top-level group.
    pub depth: u16,
}

/// Where a record is, and the header facts needed to find it again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLoc {
    /// Record type.
    pub signature: Signature,
    /// FormID as stored (file-relative: the high byte indexes this plugin's masters).
    pub form_id: u32,
    /// Record flags as stored.
    pub flags: u32,
    /// The record's form version.
    pub form_version: u16,
    /// Absolute offset of the record header.
    pub offset: u64,
    /// Data size field (excluding the 24-byte header).
    pub data_size: u32,
    /// Index in [`PluginIndex::groups`] of the innermost enclosing group; `None` for the
    /// `TES4` record (and any other record at the top level).
    pub group: Option<u32>,
    /// The first `EDID` payload without its terminating NUL, if the record has one.
    pub edid: Option<Box<[u8]>>,
}

impl RecordLoc {
    /// Whether the compressed flag is set.
    pub fn is_compressed(&self) -> bool {
        self.flags & crate::FLAG_COMPRESSED != 0
    }

    /// Bytes the record occupies, header included.
    pub fn len(&self) -> u64 {
        u64::from(self.data_size).saturating_add(HEADER_LEN as u64)
    }

    /// Whether the record has no data (a 24-byte header only).
    pub fn is_empty(&self) -> bool {
        self.data_size == 0
    }

    /// The EditorID, lossily decoded.
    pub fn edid_lossy(&self) -> Option<Cow<'_, str>> {
        self.edid.as_deref().map(String::from_utf8_lossy)
    }
}

/// Every record and group of one plugin, in file order.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginIndex {
    /// Length of the indexed input in bytes.
    pub file_len: u64,
    /// The `TES4` header record, parsed (see [`crate::tes4::Tes4`] for masters and flags).
    pub header: Record<'static>,
    /// Every record in file order, `TES4` first.
    pub records: Vec<RecordLoc>,
    /// Every group in file order (a parent before its children).
    pub groups: Vec<GroupLoc>,
}

impl PluginIndex {
    /// The groups enclosing `group` (a [`RecordLoc::group`] or [`GroupLoc::parent`]),
    /// outermost first. Empty for `None` or an index out of range.
    pub fn group_path(&self, group: Option<u32>) -> Vec<&GroupLoc> {
        let mut path = Vec::new();
        let mut next = group;
        // Each step moves to a group earlier in the file, so this ends; the bound only
        // guards against a hand-built index with a cycle.
        while let Some(g) = next.and_then(|i| self.groups.get(usize::try_from(i).ok()?)) {
            if path.len() >= self.groups.len() {
                break;
            }
            path.push(g);
            next = g.parent;
        }
        path.reverse();
        path
    }

    /// The headers of the groups enclosing `record`, outermost first: the placement a
    /// copy of the record needs in another plugin ([`crate::Plugin::upsert`]).
    pub fn group_headers(&self, record: &RecordLoc) -> Vec<GroupHeader> {
        self.group_path(record.group)
            .into_iter()
            .map(|g| g.header)
            .collect()
    }
}

/// Index the plugin at `path`.
pub fn index_file(path: &Path, options: &IndexOptions) -> Result<PluginIndex> {
    let file = File::open(path).map_err(|source| Error::Io { offset: 0, source })?;
    let len = file
        .metadata()
        .map_err(|source| Error::Io { offset: 0, source })?
        .len();
    index_reader(BufReader::with_capacity(1 << 20, file), len, options)
}

/// Index a plugin held in memory.
pub fn index_bytes(data: &[u8], options: &IndexOptions) -> Result<PluginIndex> {
    index_reader(data, data.len() as u64, options)
}

/// Index `len` bytes read sequentially from `input`.
pub fn index_reader<R: Read>(input: R, len: u64, options: &IndexOptions) -> Result<PluginIndex> {
    let mut input = Input {
        inner: input,
        pos: 0,
    };
    let max_depth = options.parse.effective_max_depth();
    let mut buf: Vec<u8> = Vec::new();

    // TES4: parsed whole.
    let head = read_header(&mut input, len, "record header")?;
    let mut r = Reader::new(&head, 0);
    let found = r.signature()?;
    if found != Signature::TES4 {
        return Err(Error::NotAPlugin { offset: 0, found });
    }
    let data_size = r.u32()?;
    read_body(&mut input, len, 0, data_size, &mut buf)?;
    let mut whole = head.to_vec();
    whole.extend_from_slice(&buf);
    let header = parse_record(&mut Reader::new(&whole, 0))?.into_owned();
    let mut records = vec![RecordLoc {
        signature: header.header.signature,
        form_id: header.header.form_id,
        flags: header.header.flags,
        form_version: header.header.form_version,
        offset: 0,
        data_size,
        group: None,
        edid: header
            .subrecords()
            .and_then(|subs| subs.iter().find(|s| s.signature() == EDID))
            .map(|s| strip_nul(s.data())),
    }];
    let mut groups: Vec<GroupLoc> = Vec::new();
    // Open groups: index in `groups` and absolute end offset.
    let mut open: Vec<(u32, u64)> = Vec::new();

    loop {
        while open.last().is_some_and(|&(_, end)| end == input.pos) {
            open.pop();
        }
        if input.pos >= len {
            break;
        }
        let limit = open.last().map_or(len, |&(_, end)| end);
        let offset = input.pos;
        let available = limit.saturating_sub(offset);
        let head = read_header_within(&mut input, available, "entry header")?;
        let mut r = Reader::new(&head, offset);
        let signature = r.signature()?;
        let size = r.u32()?;
        let parent = open.last().map(|&(i, _)| i);
        if signature == Signature::GRUP {
            let header = GroupHeader {
                label: r.array::<4>("group label")?,
                group_type: r.i32()?,
                timestamp: r.u16()?,
                version_control: r.u16()?,
                unknown: r.u32()?,
            };
            if u64::from(size) < HEADER_LEN as u64 {
                return Err(Error::GroupTooSmall { offset, size });
            }
            let depth = open.len().saturating_add(1);
            if depth > max_depth {
                return Err(Error::TooDeep {
                    offset,
                    max: max_depth,
                });
            }
            if u64::from(size) > available {
                return Err(Error::UnexpectedEof {
                    offset: offset.saturating_add(HEADER_LEN as u64),
                    what: "group contents",
                    needed: u64::from(size).saturating_sub(HEADER_LEN as u64),
                    available: available.saturating_sub(HEADER_LEN as u64),
                });
            }
            let index = u32::try_from(groups.len()).map_err(|_| Error::TooLarge {
                offset,
                what: "group count",
                len: groups.len() as u64,
            })?;
            groups.push(GroupLoc {
                header,
                offset,
                size,
                parent,
                depth: u16::try_from(depth).unwrap_or(u16::MAX),
            });
            open.push((index, offset.saturating_add(u64::from(size))));
            continue;
        }
        let flags = r.u32()?;
        let form_id = r.u32()?;
        let _timestamp = r.u16()?;
        let _version_control = r.u16()?;
        let form_version = r.u16()?;
        read_body(&mut input, limit, offset, size, &mut buf)?;
        let data_offset = offset.saturating_add(HEADER_LEN as u64);
        let edid = if flags & crate::FLAG_COMPRESSED != 0 {
            compressed_edid(&buf, offset, &options.decode)?
        } else {
            raw_edid(&buf, data_offset)?
        };
        records.push(RecordLoc {
            signature,
            form_id,
            flags,
            form_version,
            offset,
            data_size: size,
            group: parent,
            edid,
        });
    }
    Ok(PluginIndex {
        file_len: len,
        header,
        records,
        groups,
    })
}

/// A sequential reader that knows its absolute position.
struct Input<R> {
    inner: R,
    pos: u64,
}

impl<R: Read> Input<R> {
    fn read_exact(&mut self, out: &mut [u8]) -> Result<()> {
        self.inner.read_exact(out).map_err(|source| Error::Io {
            offset: self.pos,
            source,
        })?;
        self.pos = self.pos.saturating_add(out.len() as u64);
        Ok(())
    }
}

/// Read a 24-byte header at the current position of an input of `len` bytes.
fn read_header<R: Read>(
    input: &mut Input<R>,
    len: u64,
    what: &'static str,
) -> Result<[u8; HEADER_LEN]> {
    let available = len.saturating_sub(input.pos);
    read_header_within(input, available, what)
}

fn read_header_within<R: Read>(
    input: &mut Input<R>,
    available: u64,
    what: &'static str,
) -> Result<[u8; HEADER_LEN]> {
    if available < HEADER_LEN as u64 {
        return Err(Error::UnexpectedEof {
            offset: input.pos,
            what,
            needed: HEADER_LEN as u64,
            available,
        });
    }
    let mut head = [0u8; HEADER_LEN];
    input.read_exact(&mut head)?;
    Ok(head)
}

/// Read a record body of `size` bytes into `buf`; the record (header at `offset`) must
/// end by `limit`.
fn read_body<R: Read>(
    input: &mut Input<R>,
    limit: u64,
    offset: u64,
    size: u32,
    buf: &mut Vec<u8>,
) -> Result<()> {
    let available = limit.saturating_sub(input.pos);
    if u64::from(size) > available {
        return Err(Error::UnexpectedEof {
            offset: offset.saturating_add(HEADER_LEN as u64),
            what: "record data",
            needed: u64::from(size),
            available,
        });
    }
    // `size` is at most the bytes left in the input, so this allocates no more than the
    // input holds.
    let n = usize::try_from(size).map_err(|_| Error::TooLarge {
        offset,
        what: "record data",
        len: u64::from(size),
    })?;
    buf.clear();
    buf.resize(n, 0);
    input.read_exact(buf)
}

fn strip_nul(data: &[u8]) -> Box<[u8]> {
    data.strip_suffix(&[0]).unwrap_or(data).into()
}

/// The first `EDID` of an uncompressed body; every subrecord is parsed (validated).
fn raw_edid(body: &[u8], base: u64) -> Result<Option<Box<[u8]>>> {
    let mut r = Reader::new(body, base);
    let mut edid = None;
    while !r.is_empty() {
        let at = next_subrecord(&mut r)?;
        if edid.is_none() && at.signature == EDID {
            edid = Some(strip_nul(at.data));
        }
    }
    Ok(edid)
}

/// Inflates a compressed body only as far as needed to read its first `EDID`.
struct Inflating<'b> {
    stream: flate2::read::ZlibDecoder<&'b [u8]>,
    /// Decompressed bytes consumed so far.
    pos: u64,
    declared: u64,
    record_offset: u64,
}

impl Inflating<'_> {
    /// Read `out.len()` decompressed bytes, `what` naming them in errors.
    fn read(&mut self, out: &mut [u8], what: &'static str) -> Result<()> {
        let needed = out.len() as u64;
        let available = self.declared.saturating_sub(self.pos);
        if needed > available {
            return Err(Error::InCompressed {
                offset: self.record_offset,
                source: Box::new(Error::UnexpectedEof {
                    offset: self.pos,
                    what,
                    needed,
                    available,
                }),
            });
        }
        self.stream
            .read_exact(out)
            .map_err(|e| self.stream_error(&e))?;
        self.pos = self.pos.saturating_add(needed);
        Ok(())
    }

    /// Skip `n` decompressed bytes.
    fn skip(&mut self, n: u64, what: &'static str) -> Result<()> {
        let available = self.declared.saturating_sub(self.pos);
        if n > available {
            return Err(Error::InCompressed {
                offset: self.record_offset,
                source: Box::new(Error::UnexpectedEof {
                    offset: self.pos,
                    what,
                    needed: n,
                    available,
                }),
            });
        }
        let copied = io::copy(&mut (&mut self.stream).take(n), &mut io::sink())
            .map_err(|e| self.stream_error(&e))?;
        self.pos = self.pos.saturating_add(copied);
        if copied != n {
            return Err(Error::DecompressedSizeMismatch {
                offset: self.record_offset,
                declared: self.declared,
                actual: self.pos,
            });
        }
        Ok(())
    }

    fn stream_error(&self, e: &io::Error) -> Error {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            // The stream ended before the size its record declares.
            Error::DecompressedSizeMismatch {
                offset: self.record_offset,
                declared: self.declared,
                actual: self.pos,
            }
        } else {
            Error::Inflate {
                offset: self.record_offset,
                message: e.to_string(),
            }
        }
    }
}

/// The first `EDID` of a compressed body (`u32` size + zlib stream), under `limits`.
fn compressed_edid(
    body: &[u8],
    record_offset: u64,
    limits: &DecodeLimits,
) -> Result<Option<Box<[u8]>>> {
    let mut r = Reader::new(body, record_offset.saturating_add(HEADER_LEN as u64));
    if body.len() < 4 {
        return Err(Error::CompressedTooShort {
            offset: record_offset,
            len: body.len(),
        });
    }
    let declared = u64::from(r.u32()?);
    if declared > limits.max_decompressed {
        return Err(Error::DecompressedTooLarge {
            offset: record_offset,
            declared,
            cap: limits.max_decompressed,
        });
    }
    let stream = r.take(r.remaining())?;
    let mut z = Inflating {
        stream: flate2::read::ZlibDecoder::new(stream),
        pos: 0,
        declared,
        record_offset,
    };
    let mut count: u64 = 0;
    while z.pos < declared {
        count = count.saturating_add(1);
        if count > limits.max_subrecords {
            return Err(Error::TooManySubrecords {
                offset: record_offset,
                max: limits.max_subrecords,
            });
        }
        let start = z.pos;
        let mut head = [0u8; SUBRECORD_HEADER_LEN];
        z.read(&mut head, "subrecord header")?;
        let mut hr = Reader::new(&head, start);
        let mut signature = hr.signature()?;
        let mut size = u64::from(hr.u16()?);
        if signature == Signature::XXXX {
            if size != 4 {
                return Err(Error::InCompressed {
                    offset: record_offset,
                    source: Box::new(Error::BadOversize {
                        offset: start,
                        size: u16::try_from(size).unwrap_or(u16::MAX),
                    }),
                });
            }
            let mut real = [0u8; 4];
            z.read(&mut real, "u32")?;
            if z.pos >= declared {
                return Err(Error::InCompressed {
                    offset: record_offset,
                    source: Box::new(Error::DanglingOversize { offset: start }),
                });
            }
            let mut next = [0u8; SUBRECORD_HEADER_LEN];
            z.read(&mut next, "subrecord header")?;
            signature = Reader::new(&next, 0).signature()?;
            size = u64::from(u32::from_le_bytes(real));
        }
        if signature == EDID {
            let what = "subrecord data";
            let available = declared.saturating_sub(z.pos);
            if size > available {
                return Err(Error::InCompressed {
                    offset: record_offset,
                    source: Box::new(Error::UnexpectedEof {
                        offset: z.pos,
                        what,
                        needed: size,
                        available,
                    }),
                });
            }
            // Bounded by the declared size, itself under `max_decompressed`.
            let mut data = vec![0u8; usize::try_from(size).unwrap_or(0)];
            z.read(&mut data, what)?;
            return Ok(Some(strip_nul(&data)));
        }
        z.skip(size, "subrecord data")?;
    }
    Ok(None)
}

/// Read the record whose header is at absolute `offset` of `input`, with absolute offsets.
/// With `decode`, a compressed record is decompressed ([`Record::decompress`]) so its
/// subrecords are available; without it, it stays compressed.
///
/// Errors: [`Error::NotARecord`] when `offset` holds a group header, `UnexpectedEof` when
/// the record would extend past the end of the input, and every error parsing (or
/// decoding) the record can give. An offset that is not the start of a record usually
/// fails one of these, but may also read a garbage record: use [`read_indexed`] to check
/// what is read against an index entry.
pub fn read_record_at<R: Read + Seek>(
    input: &mut R,
    offset: u64,
    decode: Option<&DecodeLimits>,
) -> Result<Record<'static>> {
    read_checked(input, offset, None, decode)
}

/// [`read_record_at`] for an index entry: the header read must match `loc` (signature,
/// FormID, flags, data size), or the result is [`Error::IndexMismatch`] before the body
/// is read.
pub fn read_indexed<R: Read + Seek>(
    input: &mut R,
    loc: &RecordLoc,
    decode: Option<&DecodeLimits>,
) -> Result<Record<'static>> {
    read_checked(input, loc.offset, Some(loc), decode)
}

fn read_checked<R: Read + Seek>(
    input: &mut R,
    offset: u64,
    expect: Option<&RecordLoc>,
    decode: Option<&DecodeLimits>,
) -> Result<Record<'static>> {
    let io_at = |offset| move |source| Error::Io { offset, source };
    let len = input.seek(SeekFrom::End(0)).map_err(io_at(offset))?;
    input.seek(SeekFrom::Start(offset)).map_err(io_at(offset))?;
    let mut src = Input {
        inner: &mut *input,
        pos: offset,
    };
    let head = read_header(&mut src, len, "record header")?;
    let mut r = Reader::new(&head, offset);
    let signature = r.signature()?;
    let data_size = r.u32()?;
    let flags = r.u32()?;
    let form_id = r.u32()?;
    if signature == Signature::GRUP {
        return Err(Error::NotARecord {
            offset,
            found: signature,
        });
    }
    if let Some(loc) = expect {
        let checks = [
            (
                "signature",
                u64::from(u32::from_le_bytes(loc.signature.0)),
                u64::from(u32::from_le_bytes(signature.0)),
            ),
            ("form id", u64::from(loc.form_id), u64::from(form_id)),
            ("flags", u64::from(loc.flags), u64::from(flags)),
            ("data size", u64::from(loc.data_size), u64::from(data_size)),
        ];
        if let Some((what, expected, found)) = checks.into_iter().find(|(_, e, f)| e != f) {
            return Err(Error::IndexMismatch {
                offset,
                what,
                expected,
                found,
            });
        }
    }
    let mut body = Vec::new();
    read_body(&mut src, len, offset, data_size, &mut body)?;
    let mut whole = Vec::with_capacity(body.len().saturating_add(HEADER_LEN));
    whole.extend_from_slice(&head);
    whole.extend_from_slice(&body);
    drop(body);
    let mut record = parse_record(&mut Reader::new(&whole, offset))?.into_owned();
    if let Some(limits) = decode {
        record.decompress(limits)?;
    }
    Ok(record)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use std::io::Cursor;

    use super::{index_bytes, read_indexed, read_record_at, IndexOptions, PluginIndex};
    use crate::fixture::{self, group, record, sub, xxxx_sub};
    use crate::{
        DecodeLimits, Entry, Error, ParseOptions, Plugin, Record, Signature, FLAG_COMPRESSED,
    };

    fn opts() -> IndexOptions {
        IndexOptions::default()
    }

    /// Every record of a parsed plugin, in file order, `TES4` first.
    fn records<'p, 'a>(plugin: &'p Plugin<'a>) -> Vec<&'p Record<'a>> {
        fn walk<'p, 'a>(entries: &'p [Entry<'a>], out: &mut Vec<&'p Record<'a>>) {
            for e in entries {
                match e {
                    Entry::Group(g) => walk(g.children(), out),
                    Entry::Record(r) => out.push(r),
                }
            }
        }
        let mut out = vec![plugin.header()];
        walk(plugin.entries(), &mut out);
        out
    }

    fn with_compressed() -> Vec<u8> {
        let mut bytes = fixture::sample_plugin();
        let fields = [sub(b"EDID", b"LandA\0"), sub(b"VHGT", &[1u8; 30])].concat();
        let a = fixture::compressed_record_stored(b"LAND", 0x900, &fields);
        let fields = [sub(b"VHGT", &[2u8; 30]), xxxx_sub(b"EDID", b"LateEdid\0")].concat();
        let b = fixture::compressed_record_stored(b"LAND", 0x901, &fields);
        let c = fixture::compressed_record_stored(b"LAND", 0x902, &sub(b"VHGT", &[3u8; 8]));
        bytes.extend(group(b"LAND", 0, &[a, b, c].concat()));
        bytes
    }

    #[test]
    fn index_matches_the_tree() {
        let bytes = with_compressed();
        let index = index_bytes(&bytes, &opts()).unwrap();
        let plugin = Plugin::parse(&bytes).unwrap();
        let recs = records(&plugin);
        assert_eq!(index.records.len(), recs.len());
        assert_eq!(index.file_len, bytes.len() as u64);
        assert_eq!(&index.header, plugin.header());
        for (loc, r) in index.records.iter().zip(&recs) {
            assert_eq!(loc.signature, r.header.signature);
            assert_eq!(loc.form_id, r.header.form_id);
            assert_eq!(loc.flags, r.header.flags);
            assert_eq!(loc.form_version, r.header.form_version);
            assert_eq!(Some(loc.offset), r.offset());
            assert_eq!(Some(loc.data_size), r.data_size_as_read());
            assert_eq!(loc.is_compressed(), r.header.is_compressed());
        }
        let edids: Vec<Option<String>> = index
            .records
            .iter()
            .map(|l| l.edid_lossy().map(|s| s.into_owned()))
            .collect();
        let some = |s: &str| Some(s.to_string());
        assert_eq!(
            edids,
            [
                None,
                some("fFoo"),
                some("iBar"),
                some("TestCell"),
                None,
                None,
                None,
                some("n"),
                some("LandA"),
                some("LateEdid"),
                None,
            ]
        );
        // Groups: GMST, CELL > children > persistent, temporary; NAVM; LAND.
        assert_eq!(index.groups.len(), 7);
        let depths: Vec<u16> = index.groups.iter().map(|g| g.depth).collect();
        assert_eq!(depths, [1, 1, 2, 3, 3, 1, 1]);
        // The last REFR sits in CELL > children (6) > temporary (9).
        let refr = &index.records[6];
        assert_eq!(refr.form_id, 0x203);
        let path = index.group_path(refr.group);
        let types: Vec<i32> = path.iter().map(|g| g.header.group_type).collect();
        assert_eq!(types, [0, 6, 9]);
        assert_eq!(path[0].header.label, *b"CELL");
        let headers = index.group_headers(refr);
        assert_eq!(headers.len(), 3);
        assert_eq!(headers[2].group_type, 9);
        assert!(index.group_path(None).is_empty());
        assert!(index.group_path(Some(99)).is_empty());
    }

    #[test]
    fn random_access_equals_the_streaming_parse() {
        let bytes = with_compressed();
        let index = index_bytes(&bytes, &opts()).unwrap();
        let plugin = Plugin::parse(&bytes).unwrap();
        let mut file = Cursor::new(&bytes);
        for (loc, expected) in index.records.iter().zip(records(&plugin)) {
            let got = read_indexed(&mut file, loc, None).unwrap();
            assert_eq!(&got, expected);
            let again = read_record_at(&mut file, loc.offset, None).unwrap();
            assert_eq!(got, again);
            // Decompressed on demand.
            let decoded = read_record_at(&mut file, loc.offset, Some(&DecodeLimits::default()));
            let decoded = decoded.unwrap();
            assert!(decoded.subrecords().is_some());
            if loc.is_compressed() {
                assert_eq!(
                    decoded.subrecords().unwrap(),
                    &expected.decode(&DecodeLimits::default()).unwrap().unwrap()[..]
                );
            }
        }
    }

    #[test]
    fn wrong_offsets_are_typed_errors() {
        let bytes = with_compressed();
        let index = index_bytes(&bytes, &opts()).unwrap();
        let mut file = Cursor::new(&bytes);
        // A group header.
        let g = index.groups[0].offset;
        assert!(matches!(
            read_record_at(&mut file, g, None),
            Err(Error::NotARecord { offset, .. }) if offset == g
        ));
        // Past the end.
        let end = bytes.len() as u64;
        assert!(matches!(
            read_record_at(&mut file, end - 10, None),
            Err(Error::UnexpectedEof {
                what: "record header",
                ..
            })
        ));
        assert!(matches!(
            read_record_at(&mut file, u64::MAX - 3, None),
            Err(Error::UnexpectedEof { .. })
        ));
        // An index entry whose offset points at another record.
        let mut loc = index.records[1].clone();
        loc.offset = index.records[2].offset;
        match read_indexed(&mut file, &loc, None) {
            Err(Error::IndexMismatch {
                offset,
                what: "form id",
                expected: 0x100,
                found: 0x101,
            }) => assert_eq!(offset, loc.offset),
            other => panic!("{other:?}"),
        }
        // Mid-record: whatever is read, it is not the indexed record.
        let mut loc = index.records[3].clone();
        loc.offset += 2;
        assert!(read_indexed(&mut file, &loc, None).is_err());
    }

    #[test]
    fn truncations_and_bad_sizes_are_errors_at_offsets() {
        let bytes = with_compressed();
        for cut in 0..bytes.len() {
            match index_bytes(&bytes[..cut], &opts()) {
                // Only cuts between top-level entries leave a plugin.
                Ok(index) => {
                    let plugin = Plugin::parse(&bytes[..cut]).unwrap();
                    assert_eq!(index.records.len(), records(&plugin).len(), "cut {cut}");
                    assert!(cut >= fixture::plugin(&[]).len());
                }
                Err(err) => assert!(err.offset() <= cut as u64, "cut {cut}: {err}"),
            }
        }
        // A record body overrunning its group.
        let rec = record(b"GMST", 0, 1, &sub(b"EDID", b"abc\0"));
        let mut g = group(b"GMST", 0, &rec);
        let size = u32::from_le_bytes(g[4..8].try_into().unwrap()) - 2;
        g[4..8].copy_from_slice(&size.to_le_bytes());
        g.extend_from_slice(&[0, 0]);
        let at = fixture::plugin(&[]).len() as u64;
        let bytes = fixture::plugin(&[g]);
        match index_bytes(&bytes, &opts()) {
            Err(Error::UnexpectedEof {
                offset,
                what: "record data",
                needed: 10,
                available: 8,
            }) => assert_eq!(offset, at + 48),
            other => panic!("{other:?}"),
        }
        // Not a plugin, too deep, group too small.
        let bytes = group(b"GMST", 0, &[]);
        assert!(matches!(
            index_bytes(&bytes, &opts()),
            Err(Error::NotAPlugin { offset: 0, .. })
        ));
        let mut inner = record(b"REFR", 0, 9, &sub(b"EDID", b"a\0"));
        for _ in 0..5 {
            inner = group(b"\0\0\0\0", 6, &inner);
        }
        let bytes = fixture::plugin(&[inner]);
        let shallow = IndexOptions {
            parse: ParseOptions { max_depth: 4 },
            ..IndexOptions::default()
        };
        assert!(matches!(
            index_bytes(&bytes, &shallow),
            Err(Error::TooDeep { max: 4, .. })
        ));
        assert_eq!(index_bytes(&bytes, &opts()).unwrap().groups.len(), 5);
        let mut g = group(b"GMST", 0, &[]);
        g[4..8].copy_from_slice(&3u32.to_le_bytes());
        assert!(matches!(
            index_bytes(&fixture::plugin(&[g]), &opts()),
            Err(Error::GroupTooSmall { size: 3, .. })
        ));
    }

    #[test]
    fn compressed_edid_errors_are_bounded_and_typed() {
        let land = |fields: &[u8]| {
            let rec = fixture::compressed_record_stored(b"LAND", 1, fields);
            fixture::plugin(&[group(b"LAND", 0, &rec)])
        };
        let at = fixture::plugin(&[]).len() as u64 + 24;
        // A subrecord overrunning the decompressed body.
        let bytes = land(b"VHGT\x09\x00ab");
        match index_bytes(&bytes, &opts()) {
            Err(Error::InCompressed { offset, source }) => {
                assert_eq!(offset, at);
                assert!(matches!(*source, Error::UnexpectedEof { offset: 6, .. }));
            }
            other => panic!("{other:?}"),
        }
        // Over the decompressed-size cap.
        let small = IndexOptions {
            decode: DecodeLimits {
                max_decompressed: 4,
                ..DecodeLimits::default()
            },
            ..IndexOptions::default()
        };
        assert!(matches!(
            index_bytes(&land(&sub(b"EDID", b"abc\0")), &small),
            Err(Error::DecompressedTooLarge {
                declared: 10,
                cap: 4,
                ..
            })
        ));
        // Too many subrecords before an EDID.
        let few = IndexOptions {
            decode: DecodeLimits {
                max_subrecords: 2,
                ..DecodeLimits::default()
            },
            ..IndexOptions::default()
        };
        let fields = [sub(b"AAAA", b""), sub(b"BBBB", b""), sub(b"EDID", b"x\0")].concat();
        assert!(matches!(
            index_bytes(&land(&fields), &few),
            Err(Error::TooManySubrecords { max: 2, .. })
        ));
        // ... but an EDID first stops the scan early.
        let fields = [sub(b"EDID", b"x\0"), sub(b"AAAA", b""), sub(b"BBBB", b"")].concat();
        let index = index_bytes(&land(&fields), &few).unwrap();
        assert_eq!(index.records[1].edid.as_deref(), Some(&b"x"[..]));
        // A corrupt stream; a stream shorter than declared; bad and dangling XXXX.
        let rec = fixture::compressed_record_from_stream(b"LAND", 1, 20, &[0xFF; 12]);
        assert!(matches!(
            index_bytes(&fixture::plugin(&[group(b"LAND", 0, &rec)]), &opts()),
            Err(Error::Inflate { .. })
        ));
        let rec = fixture::compressed_record_from_stream(
            b"LAND",
            1,
            40,
            &fixture::zlib_stored(&sub(b"VHGT", &[0u8; 10])),
        );
        assert!(matches!(
            index_bytes(&fixture::plugin(&[group(b"LAND", 0, &rec)]), &opts()),
            Err(Error::DecompressedSizeMismatch { declared: 40, .. })
        ));
        let bytes = land(b"XXXX\x03\x00abc");
        assert!(matches!(
            index_bytes(&bytes, &opts()),
            Err(Error::InCompressed { source, .. }) if matches!(*source, Error::BadOversize { size: 3, .. })
        ));
        let bytes = land(b"XXXX\x04\x00\x09\x00\x00\x00");
        assert!(matches!(
            index_bytes(&bytes, &opts()),
            Err(Error::InCompressed { source, .. }) if matches!(*source, Error::DanglingOversize { .. })
        ));
        assert!(matches!(
            index_bytes(
                &fixture::plugin(&[group(
                    b"LAND",
                    0,
                    &record(b"LAND", FLAG_COMPRESSED, 1, &[1, 2])
                )]),
                &opts()
            ),
            Err(Error::CompressedTooShort { len: 2, .. })
        ));
    }

    #[test]
    fn index_is_cheap_to_clone_and_compare() {
        let bytes = fixture::sample_plugin();
        let a: PluginIndex = index_bytes(&bytes, &opts()).unwrap();
        let b = a.clone();
        assert_eq!(a, b);
        assert_eq!(a.records[0].signature, Signature::TES4);
        assert!(!a.records[1].is_empty());
        assert_eq!(a.records[1].len(), 24 + 21);
    }
}
