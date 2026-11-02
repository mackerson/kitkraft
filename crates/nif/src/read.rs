//! The file layout: header, block table, string table, block bodies, footer.
//!
//! Every fact here comes from our own study of the official models
//! (`docs/research/nif-bounds-2026-11-02.md`, claims H1 to H6), and every one was checked
//! on all 120,547 official `.nif` files (120,543 in the archives, 4 loose).

use crate::error::{Error, Result};

/// The first line of every official model (claim H1).
pub const VERSION_LINE: &[u8] = b"Gamebryo File Format, Version 20.2.0.7\n";

/// The `u32` after [`VERSION_LINE`]: its four bytes, little-endian, are 20, 2, 0, 7 (claim H2).
pub const VERSION: u32 = 0x1402_0007;

/// Value of a block reference that points nowhere (claim B2).
const NO_REF: u32 = u32::MAX;

/// The header fields before the block-type table (claim H2). Named by us; only
/// [`Header::version`] has a meaning we can show (it spells the version line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Offset 39: always [`VERSION`] in the official models.
    pub version: u32,
    /// Offset 43: always 1.
    pub flag: u8,
    /// Offset 44: always 12.
    pub tag: u32,
    /// Offset 52: 170, 172, 173, 174 or 175 in the official models.
    pub revision: u32,
    /// The first short string (a `u8` length, then the bytes).
    pub short_a: Vec<u8>,
    /// The `u32` after it: 3 in the archived models, 2 in the 4 loose ones.
    pub value: u32,
    /// The second short string.
    pub short_b: Vec<u8>,
    /// The third short string: 1 to 9 bytes, always ending in a 0 byte.
    pub short_c: Vec<u8>,
}

/// One block's place in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    /// Index into [`Nif::block_types`].
    pub type_index: u16,
    /// Offset of the block's first byte in the file.
    pub offset: usize,
    /// Length of the block in bytes, from the block-size table (claim H4).
    pub len: usize,
}

/// A parsed model: the tables, with block bodies left in place.
#[derive(Debug, Clone, PartialEq)]
pub struct Nif<'a> {
    bytes: &'a [u8],
    /// The header fields before the block-type table.
    pub header: Header,
    /// Block type names, in table order.
    pub block_types: Vec<String>,
    /// Every block, in file order.
    pub blocks: Vec<Block>,
    /// The string table, referred to by index from block bodies.
    pub strings: Vec<String>,
    /// Offset of the string-table count, for evidence.
    pub strings_offset: usize,
    /// The `u32` list after the string table: empty in every official model.
    pub groups: Vec<u32>,
    /// Root block indices from the footer: exactly one, block 0, in every official model.
    pub roots: Vec<u32>,
}

impl<'a> Nif<'a> {
    /// Reads the header, the tables and the footer, and checks that the blocks fill the
    /// bytes between them exactly (claim H5). Block bodies are not decoded here.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let mut c = Cur::new(bytes, 0);
        if bytes.get(..VERSION_LINE.len()) != Some(VERSION_LINE) {
            return Err(Error::NotNif);
        }
        c.skip(VERSION_LINE.len(), "version line")?;
        let version = c.u32("version")?;
        if version != VERSION {
            return Err(Error::Unsupported {
                what: "version",
                offset: 39,
                value: u64::from(version),
            });
        }
        let flag = c.u8("header flag")?;
        let tag = c.u32("header tag")?;
        let block_count = c.u32("block count")?;
        let revision = c.u32("header revision")?;
        let short_a = c.short_string("header short string a")?;
        let value = c.u32("header value")?;
        let short_b = c.short_string("header short string b")?;
        let short_c = c.short_string("header short string c")?;
        let header = Header {
            version,
            flag,
            tag,
            revision,
            short_a,
            value,
            short_b,
            short_c,
        };

        let type_count = c.u16("block type count")?;
        let mut block_types = Vec::with_capacity(c.cap(u64::from(type_count), 4));
        for _ in 0..type_count {
            block_types.push(c.long_string("block type name")?);
        }

        // Each block takes a u16 type index and a u32 size in the header.
        let count = c.cap(u64::from(block_count), 6);
        if count != usize::try_from(block_count).unwrap_or(usize::MAX) {
            return Err(Error::Truncated {
                what: "block tables",
                offset: c.offset(),
            });
        }
        let mut types = Vec::with_capacity(count);
        for _ in 0..count {
            let at = c.offset();
            let t = c.u16("block type index")?;
            if usize::from(t) >= block_types.len() {
                return Err(Error::OutOfRange {
                    what: "block type index",
                    offset: at,
                    value: u64::from(t),
                });
            }
            types.push(t);
        }
        let mut sizes = Vec::with_capacity(count);
        for _ in 0..count {
            sizes.push(c.u32("block size")?);
        }

        let strings_offset = c.pos;
        let string_count = c.u32("string count")?;
        let _max_len = c.u32("longest string length")?;
        let mut strings = Vec::with_capacity(c.cap(u64::from(string_count), 4));
        for _ in 0..string_count {
            strings.push(c.long_string("string")?);
        }
        let group_count = c.u32("group count")?;
        let mut groups = Vec::with_capacity(c.cap(u64::from(group_count), 4));
        for _ in 0..group_count {
            groups.push(c.u32("group")?);
        }

        let mut blocks = Vec::with_capacity(count);
        for (type_index, size) in types.into_iter().zip(sizes) {
            let offset = c.pos;
            let len = usize::try_from(size).map_err(|_| Error::Truncated {
                what: "block",
                offset: c.offset(),
            })?;
            c.skip(len, "block")?;
            blocks.push(Block {
                type_index,
                offset,
                len,
            });
        }

        let root_count = c.u32("root count")?;
        let mut roots = Vec::with_capacity(c.cap(u64::from(root_count), 4));
        for _ in 0..root_count {
            let at = c.offset();
            let r = c.u32("root")?;
            if usize::try_from(r).map_or(true, |r| r >= blocks.len()) {
                return Err(Error::OutOfRange {
                    what: "root",
                    offset: at,
                    value: u64::from(r),
                });
            }
            roots.push(r);
        }
        if c.pos != bytes.len() {
            return Err(Error::Trailing {
                offset: c.offset(),
                len: bytes.len().saturating_sub(c.pos),
            });
        }
        Ok(Self {
            bytes,
            header,
            block_types,
            blocks,
            strings,
            strings_offset,
            groups,
            roots,
        })
    }

    /// The whole file.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Type name of block `index`.
    pub fn type_name(&self, index: usize) -> Option<&str> {
        let block = self.blocks.get(index)?;
        self.block_types
            .get(usize::from(block.type_index))
            .map(String::as_str)
    }

    /// A cursor over block `index`'s bytes.
    pub(crate) fn cursor(&self, index: usize) -> Option<Cur<'a>> {
        let block = self.blocks.get(index)?;
        let end = block.offset.checked_add(block.len)?;
        let body = self.bytes.get(block.offset..end)?;
        Some(Cur::new(body, block.offset))
    }

    /// Decodes a block reference read at `offset`: `None` for the null reference, an
    /// error for an index past the last block.
    pub(crate) fn reference(
        &self,
        value: u32,
        offset: usize,
        what: &'static str,
    ) -> Result<Option<u32>> {
        if value == NO_REF {
            return Ok(None);
        }
        if usize::try_from(value).map_or(true, |v| v >= self.blocks.len()) {
            return Err(Error::OutOfRange {
                what,
                offset: offset as u64,
                value: u64::from(value),
            });
        }
        Ok(Some(value))
    }
}

/// A bounds-checked little-endian reader over a slice that starts at `base` in the file.
#[derive(Debug, Clone)]
pub(crate) struct Cur<'a> {
    buf: &'a [u8],
    pub(crate) pos: usize,
    base: usize,
}

impl<'a> Cur<'a> {
    pub(crate) fn new(buf: &'a [u8], base: usize) -> Self {
        Self { buf, pos: 0, base }
    }

    /// File offset of the next byte.
    pub(crate) fn offset(&self) -> u64 {
        self.base.saturating_add(self.pos) as u64
    }

    /// File offset of the next byte, as a `usize`.
    pub(crate) fn file_pos(&self) -> usize {
        self.base.saturating_add(self.pos)
    }

    /// Bytes left.
    pub(crate) fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// A capacity for `count` items of at least `min` bytes each, capped by the bytes left.
    pub(crate) fn cap(&self, count: u64, min: u64) -> usize {
        let fit = (self.remaining() as u64).checked_div(min).unwrap_or(0);
        usize::try_from(count.min(fit)).unwrap_or(0)
    }

    pub(crate) fn take(&mut self, n: usize, what: &'static str) -> Result<&'a [u8]> {
        let truncated = Error::Truncated {
            what,
            offset: self.offset(),
        };
        let end = self.pos.checked_add(n).ok_or(truncated.clone())?;
        let s = self.buf.get(self.pos..end).ok_or(truncated)?;
        self.pos = end;
        Ok(s)
    }

    pub(crate) fn skip(&mut self, n: usize, what: &'static str) -> Result<()> {
        self.take(n, what).map(|_| ())
    }

    fn array<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N]> {
        let s = self.take(N, what)?;
        let mut out = [0u8; N];
        out.copy_from_slice(s);
        Ok(out)
    }

    pub(crate) fn u8(&mut self, what: &'static str) -> Result<u8> {
        Ok(u8::from_le_bytes(self.array::<1>(what)?))
    }

    pub(crate) fn u16(&mut self, what: &'static str) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array::<2>(what)?))
    }

    pub(crate) fn u32(&mut self, what: &'static str) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array::<4>(what)?))
    }

    pub(crate) fn f32(&mut self, what: &'static str) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array::<4>(what)?))
    }

    pub(crate) fn f32s<const N: usize>(&mut self, what: &'static str) -> Result<[f32; N]> {
        let mut out = [0f32; N];
        for v in &mut out {
            *v = self.f32(what)?;
        }
        Ok(out)
    }

    /// A `u8` length, then that many bytes (claim H3).
    fn short_string(&mut self, what: &'static str) -> Result<Vec<u8>> {
        let n = self.u8(what)?;
        Ok(self.take(usize::from(n), what)?.to_vec())
    }

    /// A `u32` length, then that many bytes, kept as text (claims H3, H4).
    fn long_string(&mut self, what: &'static str) -> Result<String> {
        let n = self.u32(what)?;
        let n = usize::try_from(n).map_err(|_| Error::Truncated {
            what,
            offset: self.offset(),
        })?;
        Ok(String::from_utf8_lossy(self.take(n, what)?).into_owned())
    }
}
