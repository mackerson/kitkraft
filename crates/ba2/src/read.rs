//! Reading the archive header, the file records and the name table.
//!
//! Layout (`ours: ba2x.py` for the header and GNRL records; `corpus:` for the rest, see
//! `docs/corpus.md`, "Archives"):
//!
//! | Offset | Size | Field |
//! | ------ | ---- | ----- |
//! | 0      | 4    | magic `BTDX` |
//! | 4      | 4    | version (corpus: 2 and 3) |
//! | 8      | 4    | type `GNRL` or `DX10` |
//! | 12     | 4    | file count |
//! | 16     | 8    | name table offset (0 = no names) |
//! | 24     | 8    | v2+: two `u32` (corpus: always 1, 0) |
//! | 32     | 4    | v3: compression method (corpus: 3, every chunk LZ4 block) |
//!
//! Then one record per file. GNRL records are 36 bytes: name hash, extension (4 bytes),
//! directory hash, flags (corpus: always `0x00100100`), data offset `u64`, packed size
//! (0 = stored), size, and a `0xBAADF00D` filler. DX10 records are 24 bytes: name hash,
//! extension, directory hash, a `u8` (corpus: 0), chunk count `u8`, chunk record length
//! `u16` (24), height `u16`, width `u16`, mip count `u8`, DXGI format `u8`, cube-map flag
//! `u8`, tile mode `u8` (corpus: 8); then per chunk 24 bytes: data offset `u64`, packed size
//! (0 = stored), size, first and last mip `u16`, filler. The name table is a `u16` length
//! plus the bytes of each name, in record order, at the end of the file.

use std::io::{BufReader, Read, Seek, SeekFrom};

use crate::error::{io, Error, Result};

/// Archive magic.
pub const MAGIC: [u8; 4] = *b"BTDX";
/// `0xBAADF00D`, the filler at the end of GNRL records and chunk records.
pub const FILLER: u32 = 0xBAAD_F00D;

/// Bytes in a GNRL file record.
pub const GENERAL_RECORD_LEN: u64 = 36;
/// Bytes in a DX10 texture record (without its chunks).
pub const TEXTURE_RECORD_LEN: u64 = 24;
/// Bytes in a DX10 chunk record.
pub const CHUNK_RECORD_LEN: u64 = 24;

/// The two archive kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `GNRL`: any file, stored or compressed whole.
    General,
    /// `DX10`: textures, stored as chunks of mips; a DDS header is synthesized on extraction.
    Textures,
}

impl Kind {
    /// The four-byte type tag.
    pub fn tag(self) -> [u8; 4] {
        match self {
            Self::General => *b"GNRL",
            Self::Textures => *b"DX10",
        }
    }

    /// Lower-case name used in command results (`gnrl`, `dx10`).
    pub fn name(self) -> &'static str {
        match self {
            Self::General => "gnrl",
            Self::Textures => "dx10",
        }
    }
}

/// Compression codec of an archive's packed data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// A zlib stream (versions 1 and 2; `ours: ba2x.py`, corpus).
    Zlib,
    /// An LZ4 block (version 3 with method 3; corpus).
    Lz4Block,
}

impl Codec {
    /// Name used in command results.
    pub fn name(self) -> &'static str {
        match self {
            Self::Zlib => "zlib",
            Self::Lz4Block => "lz4-block",
        }
    }
}

/// The archive header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Version field.
    pub version: u32,
    /// Type.
    pub kind: Kind,
    /// Number of files.
    pub file_count: u32,
    /// Offset of the name table; 0 when the archive has none.
    pub name_table_offset: u64,
    /// The two `u32` after the first 24 bytes (versions 2 and 3).
    pub extra: Option<[u32; 2]>,
    /// The compression method field (version 3).
    pub compression: Option<u32>,
}

impl Header {
    /// Header length in bytes: 24, 32 (v2) or 36 (v3).
    pub fn len(&self) -> u64 {
        match (self.extra.is_some(), self.compression.is_some()) {
            (false, _) => 24,
            (true, false) => 32,
            (true, true) => 36,
        }
    }

    /// Always false: a header has at least 24 bytes.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The codec of packed data, or an error for a v3 method the corpus does not show.
    pub fn codec(&self) -> Result<Codec> {
        match self.compression {
            None => Ok(Codec::Zlib),
            Some(3) => Ok(Codec::Lz4Block),
            Some(method) => Err(Error::UnsupportedCodec { method }),
        }
    }
}

/// One GNRL file's storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct General {
    /// Flags field (corpus: `0x00100100`).
    pub flags: u32,
    /// Data offset.
    pub offset: u64,
    /// Packed size; 0 when stored.
    pub packed_size: u32,
    /// Size of the file.
    pub size: u32,
    /// Filler (`0xBAADF00D`).
    pub filler: u32,
}

/// One chunk of a texture: a run of mips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    /// Data offset.
    pub offset: u64,
    /// Packed size; 0 when stored.
    pub packed_size: u32,
    /// Size of the chunk's data.
    pub size: u32,
    /// First mip in the chunk.
    pub mip_first: u16,
    /// Last mip in the chunk.
    pub mip_last: u16,
    /// Filler (`0xBAADF00D`).
    pub filler: u32,
    /// Offset of this chunk record.
    pub record_offset: u64,
}

/// One DX10 texture's description and chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    /// The `u8` after the directory hash (corpus: 0).
    pub unknown: u8,
    /// Chunk record length (24).
    pub chunk_record_len: u16,
    /// Height in pixels.
    pub height: u16,
    /// Width in pixels.
    pub width: u16,
    /// Mip count.
    pub mip_count: u8,
    /// DXGI format number.
    pub format: u8,
    /// Cube-map flag (corpus: 0 or 1).
    pub cubemap: u8,
    /// Tile mode (corpus: 8).
    pub tile_mode: u8,
    /// The chunks, in order.
    pub chunks: Vec<Chunk>,
}

/// What an entry's record describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A GNRL file.
    General(General),
    /// A DX10 texture.
    Texture(Texture),
}

/// One file in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Archive path as stored (bytes decoded one-to-one as Latin-1, as `ba2x.py` does).
    pub name: String,
    /// Name hash field.
    pub name_hash: u32,
    /// Extension field.
    pub ext: [u8; 4],
    /// Directory hash field.
    pub dir_hash: u32,
    /// Offset of the record.
    pub record_offset: u64,
    /// Storage.
    pub body: Body,
}

impl Entry {
    /// Bytes extraction produces: the file size, or for a texture the synthesized DDS
    /// header plus every chunk.
    pub fn extracted_len(&self) -> u64 {
        match &self.body {
            Body::General(g) => u64::from(g.size),
            Body::Texture(t) => t.chunks.iter().fold(crate::dds::header_len(t), |acc, c| {
                acc.saturating_add(u64::from(c.size))
            }),
        }
    }

    /// Bytes the data occupies in the archive.
    pub fn stored_len(&self) -> u64 {
        match &self.body {
            Body::General(g) => stored(g.packed_size, g.size),
            Body::Texture(t) => t.chunks.iter().fold(0u64, |acc, c| {
                acc.saturating_add(stored(c.packed_size, c.size))
            }),
        }
    }

    /// Whether any of the data is compressed.
    pub fn is_compressed(&self) -> bool {
        match &self.body {
            Body::General(g) => g.packed_size != 0,
            Body::Texture(t) => t.chunks.iter().any(|c| c.packed_size != 0),
        }
    }

    /// First data offset (for evidence), if the entry has data.
    pub fn data_offset(&self) -> Option<u64> {
        match &self.body {
            Body::General(g) => Some(g.offset),
            Body::Texture(t) => t.chunks.first().map(|c| c.offset),
        }
    }

    /// Whether the hash fields agree with the name (`hash` module rule).
    pub fn hashes_match_name(&self) -> bool {
        let bytes = latin1_bytes(&self.name);
        crate::hash::record_hashes(&bytes) == (self.name_hash, self.ext, self.dir_hash)
    }
}

/// Stored length of one blob.
pub(crate) fn stored(packed: u32, size: u32) -> u64 {
    if packed == 0 {
        u64::from(size)
    } else {
        u64::from(packed)
    }
}

/// A parsed archive: header and every entry. Data is read on demand ([`crate::extract`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    /// The header.
    pub header: Header,
    /// Every file, in record order.
    pub entries: Vec<Entry>,
    /// Length of the archive file.
    pub file_len: u64,
    /// False when the name table offset is 0 and names were synthesized from the hashes.
    pub has_names: bool,
}

impl Archive {
    /// Reads the header, records and name table. Data stays in the file.
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let file_len = reader.seek(SeekFrom::End(0)).map_err(io(0))?;
        reader.seek(SeekFrom::Start(0)).map_err(io(0))?;
        let mut src = Src {
            inner: BufReader::with_capacity(1 << 16, reader),
            pos: 0,
            len: file_len,
        };
        let header = read_header(&mut src)?;
        let count = u64::from(header.file_count);
        let min_record = match header.kind {
            Kind::General => GENERAL_RECORD_LEN,
            Kind::Textures => TEXTURE_RECORD_LEN,
        };
        let needed = count.saturating_mul(min_record);
        src.check(needed)?;

        // Capacity is capped by the bytes actually present (checked above).
        let mut entries = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for _ in 0..count {
            let record_offset = src.pos;
            let name_hash = src.u32()?;
            let ext = src.bytes::<4>()?;
            let dir_hash = src.u32()?;
            let body = match header.kind {
                Kind::General => Body::General(read_general(&mut src)?),
                Kind::Textures => Body::Texture(read_texture(&mut src, record_offset)?),
            };
            entries.push(Entry {
                name: String::new(),
                name_hash,
                ext,
                dir_hash,
                record_offset,
                body,
            });
        }
        let records_end = src.pos;
        for entry in &entries {
            check_data(entry, file_len)?;
        }

        let has_names = header.name_table_offset != 0;
        if has_names {
            if header.name_table_offset < records_end || header.name_table_offset > file_len {
                return Err(Error::OutOfRange {
                    what: "name table offset",
                    offset: 16,
                    value: header.name_table_offset,
                    file_len,
                });
            }
            src.seek(header.name_table_offset)?;
            for entry in &mut entries {
                let len = src.u16()?;
                let bytes = src.vec(u64::from(len))?;
                entry.name = latin1(&bytes);
            }
        } else {
            for entry in &mut entries {
                entry.name = synthetic_name(entry);
            }
        }
        Ok(Self {
            header,
            entries,
            file_len,
            has_names,
        })
    }

    /// Opens and reads an archive file. Returns the file too, for extraction.
    pub fn open(path: &std::path::Path) -> Result<(Self, std::fs::File)> {
        let mut file = std::fs::File::open(path).map_err(io(0))?;
        let archive = Self::read(&mut file)?;
        Ok((archive, file))
    }
}

fn read_header<R: Read + Seek>(src: &mut Src<R>) -> Result<Header> {
    src.check(24)?;
    let magic = src.bytes::<4>()?;
    if magic != MAGIC {
        return Err(Error::BadMagic { found: magic });
    }
    let version = src.u32()?;
    if !(1..=3).contains(&version) {
        return Err(Error::UnsupportedVersion { version });
    }
    let tag = src.bytes::<4>()?;
    let kind = if tag == Kind::General.tag() {
        Kind::General
    } else if tag == Kind::Textures.tag() {
        Kind::Textures
    } else {
        return Err(Error::UnknownKind { found: tag });
    };
    let file_count = src.u32()?;
    let name_table_offset = src.u64()?;
    let extra = if version >= 2 {
        Some([src.u32()?, src.u32()?])
    } else {
        None
    };
    let compression = if version == 3 { Some(src.u32()?) } else { None };
    Ok(Header {
        version,
        kind,
        file_count,
        name_table_offset,
        extra,
        compression,
    })
}

fn read_general<R: Read + Seek>(src: &mut Src<R>) -> Result<General> {
    Ok(General {
        flags: src.u32()?,
        offset: src.u64()?,
        packed_size: src.u32()?,
        size: src.u32()?,
        filler: src.u32()?,
    })
}

fn read_texture<R: Read + Seek>(src: &mut Src<R>, record_offset: u64) -> Result<Texture> {
    let unknown = src.u8()?;
    let chunk_count = src.u8()?;
    let chunk_record_len = src.u16()?;
    if u64::from(chunk_record_len) != CHUNK_RECORD_LEN && chunk_count != 0 {
        return Err(Error::ChunkHeaderLen {
            offset: record_offset,
            found: chunk_record_len,
        });
    }
    let height = src.u16()?;
    let width = src.u16()?;
    let mip_count = src.u8()?;
    let format = src.u8()?;
    let cubemap = src.u8()?;
    let tile_mode = src.u8()?;
    src.check(u64::from(chunk_count).saturating_mul(CHUNK_RECORD_LEN))?;
    let mut chunks = Vec::with_capacity(usize::from(chunk_count));
    for _ in 0..chunk_count {
        let chunk_offset = src.pos;
        chunks.push(Chunk {
            offset: src.u64()?,
            packed_size: src.u32()?,
            size: src.u32()?,
            mip_first: src.u16()?,
            mip_last: src.u16()?,
            filler: src.u32()?,
            record_offset: chunk_offset,
        });
    }
    Ok(Texture {
        unknown,
        chunk_record_len,
        height,
        width,
        mip_count,
        format,
        cubemap,
        tile_mode,
        chunks,
    })
}

/// Every data range lies inside the file.
fn check_data(entry: &Entry, file_len: u64) -> Result<()> {
    let ranges: Vec<(u64, u64, u64)> = match &entry.body {
        Body::General(g) => vec![(
            g.offset,
            stored(g.packed_size, g.size),
            entry.record_offset.saturating_add(12),
        )],
        Body::Texture(t) => t
            .chunks
            .iter()
            .map(|c| (c.offset, stored(c.packed_size, c.size), c.record_offset))
            .collect(),
    };
    for (offset, len, field) in ranges {
        let end = offset.saturating_add(len);
        if end > file_len {
            return Err(Error::OutOfRange {
                what: "file data",
                offset: field,
                value: end,
                file_len,
            });
        }
    }
    Ok(())
}

/// Name for an entry of an archive without a name table.
fn synthetic_name(entry: &Entry) -> String {
    let ext: String = entry
        .ext
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| char::from(b))
        .collect();
    format!(
        "{:08x}/{:08x}{}{}",
        entry.dir_hash,
        entry.name_hash,
        if ext.is_empty() { "" } else { "." },
        ext
    )
}

/// Bytes to `String`, one char per byte (Latin-1), as `ba2x.py` decodes names.
pub(crate) fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// The inverse of [`latin1`]; chars above U+00FF become `?` (never produced by `latin1`).
pub(crate) fn latin1_bytes(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

/// Bounded sequential reader that knows its offset and the file length.
struct Src<'r, R> {
    inner: BufReader<&'r mut R>,
    pos: u64,
    len: u64,
}

impl<R: Read + Seek> Src<'_, R> {
    fn available(&self) -> u64 {
        self.len.saturating_sub(self.pos)
    }

    fn check(&self, needed: u64) -> Result<()> {
        if needed > self.available() {
            return Err(Error::Truncated {
                offset: self.pos,
                needed,
                available: self.available(),
            });
        }
        Ok(())
    }

    fn seek(&mut self, pos: u64) -> Result<()> {
        self.inner.seek(SeekFrom::Start(pos)).map_err(io(pos))?;
        self.pos = pos;
        Ok(())
    }

    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.check(N as u64)?;
        let mut buf = [0u8; N];
        self.inner.read_exact(&mut buf).map_err(io(self.pos))?;
        self.pos = self.pos.saturating_add(N as u64);
        Ok(buf)
    }

    fn vec(&mut self, n: u64) -> Result<Vec<u8>> {
        self.check(n)?;
        let len = usize::try_from(n).map_err(|_| Error::Truncated {
            offset: self.pos,
            needed: n,
            available: self.available(),
        })?;
        let mut buf = vec![0u8; len];
        self.inner.read_exact(&mut buf).map_err(io(self.pos))?;
        self.pos = self.pos.saturating_add(n);
        Ok(buf)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(u8::from_le_bytes(self.bytes::<1>()?))
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes::<2>()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes::<4>()?))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes::<8>()?))
    }
}
