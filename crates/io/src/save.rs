//! Read-only Starfield save (`.sfs`) header reader: just enough to get the plugin lists
//! the game recorded, which are the game's own runtime slot assignment.
//!
//! Nothing here opens a file for writing. The layout below is a **corpus** fact read from
//! the owner's saves (copies, never the originals; game 1.16.244.0, 2026-10-15:
//! three modded and one vanilla save of the `KitKraft Dev` profile and `Save56` of
//! `Default`), recorded in `docs/load-order.md` ("Save headers"). No outside source.
//!
//! Container (`BCPS`, little-endian):
//!
//! | Offset | Size | Meaning |
//! | ------ | ---- | ------- |
//! | 0x00 | 4 | `BCPS` |
//! | 0x04 | 4 | container version: 1 in every save read |
//! | 0x08 | 8 | offset of the chunk size table (0x48 in every save) |
//! | 0x18 | 8 | offset of the first compressed chunk (the table ends there, padded with `padding\0`) |
//! | 0x20 | 8 | total uncompressed size |
//! | 0x30 | 4 | uncompressed chunk size (0x40000 in every save) |
//! | 0x38 | 8 | chunk alignment (16 in every save) |
//! | 0x44 | 4 | `ZIP `: every chunk is one zlib stream |
//! | table | 4 per chunk | compressed size of each chunk; count = ceil(total / chunk size) |
//!
//! Chunks follow one another from the data offset, each starting on the alignment.
//!
//! Decompressed data, from offset 0:
//!
//! | Field | Meaning |
//! | ----- | ------- |
//! | 12 bytes | `SFS_SAVEGAME` |
//! | u32 | size of the header that follows (skipped: player, location, play time) |
//! | that many bytes | the header (its first u32 is 27 in every save read) |
//! | u8 | 153 in every save read (read as a form version, not interpreted) |
//! | wstring x2 | the game version, twice (`1.16.244.0`) |
//! | u32 | size of the plugin block that follows |
//! | u8 + entries | full plugins, in runtime slot order |
//! | u16 + entries | small plugins, in `FE` slot order |
//! | u32 + entries | medium plugins, in `FD` slot order |
//! | rest of the block | 2 bytes (`01 00`) in every save read; not interpreted |
//!
//! A wstring is a u16 length and that many bytes (no terminator). An entry is a wstring
//! (the plugin file name), a u8 0 or 1, and after a 1: a u32 (0 in every save read), a
//! wstring (`18446744073709551615.undefined` in every save read) and a u8 (0). Official
//! plugins carry 0, every other plugin 1. The block size matches the parsed entries plus
//! the 2 trailing bytes in every save read; a block that ends early is an error.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use kitkraft_doc::{RuntimeSlot, Scale};

use crate::error::{IoError, Result};

/// The container magic.
pub const MAGIC: &[u8; 4] = b"BCPS";
/// The decompressed data's magic.
pub const SAVE_MAGIC: &[u8; 12] = b"SFS_SAVEGAME";
/// The only compression tag seen.
pub const ZIP: &[u8; 4] = b"ZIP ";

/// Largest container header read (chunk table included): 1 MiB is 262,144 chunks.
const MAX_CONTAINER_HEADER: u64 = 1 << 20;
/// Largest uncompressed chunk accepted.
const MAX_CHUNK: u32 = 16 << 20;
/// Most decompressed bytes read to reach the end of the plugin block.
const MAX_HEAD: usize = 64 << 20;

/// One plugin as the save lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePlugin {
    /// File name as stored.
    pub name: String,
    /// The flag byte after the name (0 for official plugins, 1 for the rest, in every save
    /// read).
    pub flag: u8,
    /// After flag 1: the u32 and the string that follow (not interpreted).
    pub extra: Option<(u32, String)>,
    /// Offset of the entry in the decompressed data.
    pub offset: u64,
}

/// What a save's header says about its plugins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveHeader {
    /// The save file.
    pub path: PathBuf,
    /// Its size and modification time (nanoseconds since the Unix epoch, 0 if unknown).
    pub size: u64,
    /// Modification time.
    pub mtime_ns: u64,
    /// Container version (0x04).
    pub container_version: u32,
    /// Total uncompressed size.
    pub uncompressed_size: u64,
    /// Chunks in the file.
    pub chunks: usize,
    /// Chunks decompressed to read the header.
    pub chunks_read: usize,
    /// The header's first u32.
    pub save_version: u32,
    /// The u8 after the header.
    pub form_version: u8,
    /// The game version string.
    pub game_version: String,
    /// The second version string.
    pub game_version_2: String,
    /// Offset of the plugin block (after its size) in the decompressed data.
    pub plugin_block_offset: u64,
    /// Its size.
    pub plugin_block_size: u32,
    /// Bytes of the block after the three lists (not interpreted).
    pub trailing: u32,
    /// Full plugins in slot order (`00`, `01`, ...).
    pub full: Vec<SavePlugin>,
    /// Small plugins in slot order (`FE000`, `FE001`, ...).
    pub small: Vec<SavePlugin>,
    /// Medium plugins in slot order (`FD00`, `FD01`, ...).
    pub medium: Vec<SavePlugin>,
}

impl SaveHeader {
    /// The runtime slot the save gives `name` (ASCII case-insensitive), with its scale.
    pub fn slot(&self, name: &str) -> Option<RuntimeSlot> {
        let pos = |list: &[SavePlugin]| list.iter().position(|p| p.name.eq_ignore_ascii_case(name));
        if let Some(i) = pos(&self.full) {
            return u8::try_from(i)
                .ok()
                .filter(|i| *i <= 0xFC)
                .map(RuntimeSlot::Full);
        }
        if let Some(i) = pos(&self.medium) {
            return u8::try_from(i).ok().map(RuntimeSlot::Medium);
        }
        pos(&self.small)
            .and_then(|i| u16::try_from(i).ok())
            .filter(|i| *i <= 0xFFF)
            .map(RuntimeSlot::Small)
    }

    /// The list of one scale.
    pub fn list(&self, scale: Scale) -> &[SavePlugin] {
        match scale {
            Scale::Full => &self.full,
            Scale::Medium => &self.medium,
            Scale::Small => &self.small,
        }
    }

    /// Every plugin with its scale, full then small then medium.
    pub fn plugins(&self) -> impl Iterator<Item = (Scale, &SavePlugin)> {
        self.full
            .iter()
            .map(|p| (Scale::Full, p))
            .chain(self.small.iter().map(|p| (Scale::Small, p)))
            .chain(self.medium.iter().map(|p| (Scale::Medium, p)))
    }
}

fn le_u32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at.checked_add(4)?)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}

fn le_u64(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at.checked_add(8)?)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .map(u64::from_le_bytes)
}

/// The container header.
#[derive(Debug, Clone)]
struct Container {
    version: u32,
    data_offset: u64,
    uncompressed: u64,
    chunk_size: u32,
    align: u64,
    sizes: Vec<u32>,
}

/// Decompressed bytes on demand: chunks are inflated as the parser needs them.
struct Stream {
    path: PathBuf,
    file: File,
    file_len: u64,
    c: Container,
    next_chunk: usize,
    next_offset: u64,
    buf: Vec<u8>,
    pos: usize,
}

impl Stream {
    fn err(&self, offset: u64, what: impl Into<String>) -> IoError {
        IoError::Save {
            path: self.path.clone(),
            offset,
            what: what.into(),
        }
    }

    fn here(&self) -> u64 {
        self.pos as u64
    }

    /// Inflate chunks until `n` bytes are available after the cursor.
    fn ensure(&mut self, n: usize, what: &str) -> Result<()> {
        let want = self
            .pos
            .checked_add(n)
            .ok_or_else(|| self.err(self.here(), format!("{what}: size overflow")))?;
        if want > MAX_HEAD {
            return Err(self.err(
                self.here(),
                format!("{what}: past the {MAX_HEAD}-byte limit for a save header"),
            ));
        }
        while self.buf.len() < want {
            let Some(&size) = self.c.sizes.get(self.next_chunk) else {
                return Err(self.err(
                    self.here(),
                    format!(
                        "{what}: needs {n} bytes, the save has {} decompressed bytes left",
                        self.buf.len().saturating_sub(self.pos)
                    ),
                ));
            };
            let at = self.next_offset;
            let end = at
                .checked_add(u64::from(size))
                .filter(|e| *e <= self.file_len);
            let Some(end) = end else {
                return Err(self.err(
                    at,
                    format!(
                        "chunk {} ({size} compressed bytes at file offset 0x{at:X}) runs past the end of the file ({} bytes)",
                        self.next_chunk, self.file_len
                    ),
                ));
            };
            self.file
                .seek(SeekFrom::Start(at))
                .map_err(|e| IoError::io(&self.path, e))?;
            let mut compressed = Vec::new();
            (&mut self.file)
                .take(u64::from(size))
                .read_to_end(&mut compressed)
                .map_err(|e| IoError::io(&self.path, e))?;
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .take(u64::from(self.c.chunk_size).saturating_add(1))
                .read_to_end(&mut out)
                .map_err(|e| {
                    self.err(
                        at,
                        format!("chunk {} (file offset 0x{at:X}): {e}", self.next_chunk),
                    )
                })?;
            if out.len() > self.c.chunk_size as usize {
                return Err(self.err(
                    at,
                    format!(
                        "chunk {} inflates past the chunk size {}",
                        self.next_chunk, self.c.chunk_size
                    ),
                ));
            }
            self.buf.extend_from_slice(&out);
            self.next_chunk = self.next_chunk.saturating_add(1);
            // The alignment is a power of two (checked with the container header).
            self.next_offset = end
                .checked_next_multiple_of(self.c.align.max(1))
                .ok_or_else(|| self.err(end, "chunk offset overflow"))?;
        }
        Ok(())
    }

    fn take(&mut self, n: usize, what: &str) -> Result<&[u8]> {
        self.ensure(n, what)?;
        let start = self.pos;
        let end = start.saturating_add(n);
        self.pos = end;
        self.buf.get(start..end).ok_or_else(|| IoError::Save {
            path: self.path.clone(),
            offset: start as u64,
            what: format!("{what}: short"),
        })
    }

    fn u8(&mut self, what: &str) -> Result<u8> {
        Ok(self.take(1, what)?.first().copied().unwrap_or(0))
    }

    fn u16(&mut self, what: &str) -> Result<u16> {
        let b = self.take(2, what)?;
        Ok(u16::from_le_bytes([
            b.first().copied().unwrap_or(0),
            b.get(1).copied().unwrap_or(0),
        ]))
    }

    fn u32(&mut self, what: &str) -> Result<u32> {
        let b = self.take(4, what)?;
        Ok(le_u32(b, 0).unwrap_or(0))
    }

    fn wstring(&mut self, what: &str) -> Result<String> {
        let len = self.u16(what)?;
        let b = self.take(usize::from(len), what)?;
        Ok(String::from_utf8_lossy(b).into_owned())
    }

    fn entry(&mut self) -> Result<SavePlugin> {
        let offset = self.here();
        let name = self.wstring("plugin name")?;
        if name.is_empty() {
            return Err(self.err(offset, "empty plugin name"));
        }
        let flag = self.u8("plugin flag")?;
        let extra = match flag {
            0 => None,
            1 => {
                let n = self.u32("plugin entry u32")?;
                let s = self.wstring("plugin entry string")?;
                let end = self.here();
                let z = self.u8("plugin entry end")?;
                if z != 0 {
                    return Err(self.err(
                        end,
                        format!("plugin `{name}`: entry ends with {z}, every save read has 0"),
                    ));
                }
                Some((n, s))
            }
            other => {
                return Err(self.err(
                    offset,
                    format!("plugin `{name}`: flag {other}; only 0 and 1 are known"),
                ))
            }
        };
        Ok(SavePlugin {
            name,
            flag,
            extra,
            offset,
        })
    }
}

fn container(path: &Path, file: &mut File, file_len: u64) -> Result<Container> {
    let bad = |offset: u64, what: String| IoError::Save {
        path: path.to_path_buf(),
        offset,
        what,
    };
    let mut head = vec![0u8; 0x48];
    file.read_exact(&mut head)
        .map_err(|_| bad(0, format!("{file_len} bytes: too short for a BCPS header")))?;
    if head.get(0..4) != Some(MAGIC.as_slice()) {
        return Err(bad(0, "not a BCPS container (magic)".into()));
    }
    let field = |at: usize| le_u64(&head, at).unwrap_or(0);
    let version = le_u32(&head, 4).unwrap_or(0);
    let table = field(0x08);
    let data_offset = field(0x18);
    let uncompressed = field(0x20);
    let chunk_size = le_u32(&head, 0x30).unwrap_or(0);
    let align = field(0x38);
    if head.get(0x44..0x48) != Some(ZIP.as_slice()) {
        return Err(bad(
            0x44,
            format!(
                "compression tag {:?}; only `ZIP ` is known",
                String::from_utf8_lossy(head.get(0x44..0x48).unwrap_or_default())
            ),
        ));
    }
    if chunk_size == 0 || chunk_size > MAX_CHUNK {
        return Err(bad(0x30, format!("chunk size {chunk_size}")));
    }
    if align == 0 || align > 4096 || !align.is_power_of_two() {
        return Err(bad(0x38, format!("chunk alignment {align}")));
    }
    let count = uncompressed.div_ceil(u64::from(chunk_size));
    let table_end = count
        .checked_mul(4)
        .and_then(|n| n.checked_add(table))
        .filter(|e| *e <= data_offset && data_offset <= MAX_CONTAINER_HEADER.min(file_len))
        .ok_or_else(|| {
            bad(
                0x08,
                format!(
                    "{count} chunks from table 0x{table:X} do not fit before the data at 0x{data_offset:X}"
                ),
            )
        })?;
    let mut rest = vec![0u8; usize::try_from(data_offset.saturating_sub(0x48)).unwrap_or(0)];
    file.read_exact(&mut rest)
        .map_err(|e| IoError::io(path, e))?;
    head.extend_from_slice(&rest);
    let mut sizes = Vec::new();
    let mut at = table;
    while at < table_end {
        let i = usize::try_from(at).map_err(|_| bad(at, "table offset".into()))?;
        let s = le_u32(&head, i).ok_or_else(|| bad(at, "chunk table".into()))?;
        sizes.push(s);
        at = at.saturating_add(4);
    }
    Ok(Container {
        version,
        data_offset,
        uncompressed,
        chunk_size,
        align,
        sizes,
    })
}

/// Read the plugin lists of the save at `path` (read-only; decompresses only the chunks
/// that hold the header, normally the first).
pub fn read_save_header(path: &Path) -> Result<SaveHeader> {
    let mut file = File::open(path).map_err(|e| IoError::io(path, e))?;
    let meta = file.metadata().map_err(|e| IoError::io(path, e))?;
    let size = meta.len();
    let mtime_ns = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_nanos()).ok())
        .unwrap_or(0);
    let c = container(path, &mut file, size)?;
    let mut s = Stream {
        path: path.to_path_buf(),
        file,
        file_len: size,
        next_chunk: 0,
        next_offset: c.data_offset,
        c,
        buf: Vec::new(),
        pos: 0,
    };
    let magic = s.take(SAVE_MAGIC.len(), "save magic")?;
    if magic != SAVE_MAGIC.as_slice() {
        return Err(s.err(0, "decompressed data does not start with SFS_SAVEGAME"));
    }
    let header_size = s.u32("header size")?;
    let header_at = s.here();
    let header_len = usize::try_from(header_size).unwrap_or(usize::MAX);
    if header_len < 4 {
        return Err(s.err(header_at, format!("header size {header_size}")));
    }
    let save_version = s.u32("save version")?;
    s.take(header_len.saturating_sub(4), "header")?;
    let form_version = s.u8("form version")?;
    let game_version = s.wstring("game version")?;
    let game_version_2 = s.wstring("second game version")?;
    let block_size = s.u32("plugin block size")?;
    let block_at = s.here();
    let block_end = block_at.saturating_add(u64::from(block_size));
    let full_count = s.u8("full plugin count")?;
    let mut full = Vec::with_capacity(usize::from(full_count));
    for _ in 0..full_count {
        full.push(s.entry()?);
    }
    let small_count = s.u16("small plugin count")?;
    let mut small = Vec::new();
    for _ in 0..small_count {
        small.push(s.entry()?);
    }
    let medium_at = s.here();
    let medium_count = s.u32("medium plugin count")?;
    if medium_count > 0x100 {
        return Err(s.err(
            medium_at,
            format!("{medium_count} medium plugins: at most 256 have slots"),
        ));
    }
    let mut medium = Vec::new();
    for _ in 0..medium_count {
        medium.push(s.entry()?);
    }
    let end = s.here();
    if end > block_end {
        return Err(s.err(
            end,
            format!(
                "the plugin lists end at 0x{end:X}, past the plugin block (0x{block_at:X} + {block_size})"
            ),
        ));
    }
    Ok(SaveHeader {
        path: path.to_path_buf(),
        size,
        mtime_ns,
        container_version: s.c.version,
        uncompressed_size: s.c.uncompressed,
        chunks: s.c.sizes.len(),
        chunks_read: s.next_chunk,
        save_version,
        form_version,
        game_version,
        game_version_2,
        plugin_block_offset: block_at,
        plugin_block_size: block_size,
        trailing: u32::try_from(block_end.saturating_sub(end)).unwrap_or(u32::MAX),
        full,
        small,
        medium,
    })
}

/// A save's plugin lists against a load order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveMatch {
    /// The save's slot for every plugin of the load order, in load order, when the save
    /// lists exactly the load order's plugins under the scales their headers give;
    /// otherwise `None` (the save describes another plugin set, and slots may shift).
    pub slots: Option<Vec<Option<RuntimeSlot>>>,
    /// Plugins of the load order the save does not list (`name (scale)`).
    pub only_in_load_order: Vec<String>,
    /// Plugins the save lists that are not in the load order (`name (scale)`).
    pub only_in_save: Vec<String>,
    /// Plugins the save lists under another scale than their header's, or that have no
    /// known scale (a failed load, both scale bits).
    pub scale_mismatch: Vec<String>,
    /// Plugins whose save slot differs from the inferred one (`name: inferred X, save Y`).
    pub differs_from_inferred: Vec<String>,
}

impl SaveMatch {
    /// Whether the save confirms every runtime slot of the load order.
    pub fn confirmed(&self) -> bool {
        self.slots.is_some()
    }
}

/// Compare `save`'s plugin lists with `lo` (names ASCII case-insensitive; the scale from
/// each plugin's header).
pub fn match_load_order(lo: &kitkraft_doc::LoadOrder, save: &SaveHeader) -> SaveMatch {
    let mut m = SaveMatch::default();
    let mut slots = Vec::with_capacity(lo.plugins.len());
    let mut used: Vec<(Scale, usize)> = Vec::new();
    for p in &lo.plugins {
        let found = [Scale::Full, Scale::Small, Scale::Medium]
            .into_iter()
            .find_map(|s| {
                save.list(s)
                    .iter()
                    .position(|e| e.name.eq_ignore_ascii_case(&p.name))
                    .map(|i| (s, i))
            });
        match (p.scale(), found) {
            (Some(scale), Some((s, i))) if s == scale => {
                used.push((s, i));
                let slot = save.slot(&p.name);
                if slot != p.runtime_slot {
                    let label =
                        |s: Option<RuntimeSlot>| s.map_or_else(|| "none".into(), |s| s.label());
                    m.differs_from_inferred.push(format!(
                        "{}: inferred {}, save {}",
                        p.name,
                        label(p.runtime_slot),
                        label(slot)
                    ));
                }
                slots.push(slot);
            }
            (scale, Some((s, i))) => {
                used.push((s, i));
                m.scale_mismatch.push(format!(
                    "{}: the save lists it as {}, its header says {}",
                    p.name,
                    s.as_str(),
                    scale.map_or("no known scale", Scale::as_str)
                ));
                slots.push(None);
            }
            (scale, None) => {
                m.only_in_load_order.push(format!(
                    "{} ({})",
                    p.name,
                    scale.map_or("no known scale", Scale::as_str)
                ));
                slots.push(None);
            }
        }
    }
    for (scale, e) in save.plugins() {
        let i = save
            .list(scale)
            .iter()
            .position(|x| std::ptr::eq(x, e))
            .unwrap_or(usize::MAX);
        if !used.contains(&(scale, i)) {
            m.only_in_save
                .push(format!("{} ({})", e.name, scale.as_str()));
        }
    }
    if m.only_in_load_order.is_empty() && m.only_in_save.is_empty() && m.scale_mismatch.is_empty() {
        m.slots = Some(slots);
    }
    m
}

/// Where the game keeps a profile's saves: `<profile>\saves` when the profile's
/// `settings.ini` has `[General] LocalSaves=true` (MO2's profile-specific saves; both
/// owner profiles say which in that key), else `Documents\My Games\Starfield\Saves` under
/// the user's home (`USERPROFILE` on Windows, `$HOME` elsewhere). A `sLocalSavePath` in
/// the game's INIs is not read.
pub fn saves_dir(profile_dir: &Path) -> Option<PathBuf> {
    let local = std::fs::read(profile_dir.join("settings.ini"))
        .ok()
        .map(|b| crate::ini::Ini::parse(&String::from_utf8_lossy(&b)))
        .and_then(|ini| ini.get("General", "LocalSaves"))
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
    if local {
        return Some(profile_dir.join("saves"));
    }
    let home = if cfg!(windows) {
        std::env::var_os("USERPROFILE")
    } else {
        std::env::var_os("HOME")
    };
    home.map(|home| {
        PathBuf::from(home)
            .join("Documents")
            .join("My Games")
            .join("Starfield")
            .join("Saves")
    })
}

/// The most recently modified `.sfs` file in `dir` (ties: the larger name).
pub fn newest_save(dir: &Path) -> Result<Option<PathBuf>> {
    let entries = std::fs::read_dir(dir).map_err(|e| IoError::io(dir, e))?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in entries.flatten() {
        let path = e.path();
        let is_sfs = path
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("sfs"));
        if !is_sfs {
            continue;
        }
        let Ok(meta) = e.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Ok(t) = meta.modified() else {
            continue;
        };
        let better = match &best {
            None => true,
            Some((bt, bp)) => t > *bt || (t == *bt && path > *bp),
        };
        if better {
            best = Some((t, path));
        }
    }
    Ok(best.map(|(_, p)| p))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]
pub(crate) mod tests {
    use super::*;
    use crate::testdata::{dev_flags, tiny_plugin, DEV_FULL, DEV_MEDIUM, DEV_SMALL};
    use std::io::Write;

    /// One plugin entry as a save stores it.
    fn entry(name: &str, official: bool) -> Vec<u8> {
        let mut out = (name.len() as u16).to_le_bytes().to_vec();
        out.extend_from_slice(name.as_bytes());
        if official {
            out.push(0);
        } else {
            out.push(1);
            out.extend_from_slice(&0u32.to_le_bytes());
            let s = "18446744073709551615.undefined";
            out.extend_from_slice(&(s.len() as u16).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
            out.push(0);
        }
        out
    }

    /// Decompressed save data with these lists (`(name, official)`), in the layout above.
    pub(crate) fn save_data(
        full: &[(&str, bool)],
        small: &[(&str, bool)],
        medium: &[(&str, bool)],
    ) -> Vec<u8> {
        let mut block = vec![full.len() as u8];
        for (n, o) in full {
            block.extend(entry(n, *o));
        }
        block.extend_from_slice(&(small.len() as u16).to_le_bytes());
        for (n, o) in small {
            block.extend(entry(n, *o));
        }
        block.extend_from_slice(&(medium.len() as u32).to_le_bytes());
        for (n, o) in medium {
            block.extend(entry(n, *o));
        }
        block.extend_from_slice(&[1, 0]);
        let mut out = SAVE_MAGIC.to_vec();
        let header = [27u32.to_le_bytes().to_vec(), vec![0xAB; 20]].concat();
        out.extend_from_slice(&(header.len() as u32).to_le_bytes());
        out.extend_from_slice(&header);
        out.push(153);
        for _ in 0..2 {
            out.extend_from_slice(&10u16.to_le_bytes());
            out.extend_from_slice(b"1.16.244.0");
        }
        out.extend_from_slice(&(block.len() as u32).to_le_bytes());
        out.extend(block);
        // The rest of a save: change forms and so on (not read).
        out.extend(std::iter::repeat_n(0x5Au8, 300));
        out
    }

    /// A BCPS container of `data` in chunks of `chunk` bytes, aligned to 16.
    pub(crate) fn bcps(data: &[u8], chunk: u32) -> Vec<u8> {
        let streams: Vec<Vec<u8>> = data
            .chunks(chunk as usize)
            .map(|c| {
                let mut z =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                z.write_all(c).unwrap();
                z.finish().unwrap()
            })
            .collect();
        let table = 0x48u64;
        let mut data_offset = table + 4 * streams.len() as u64 + 8;
        data_offset = data_offset.div_ceil(16) * 16;
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&table.to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes());
        out.extend_from_slice(&data_offset.to_le_bytes());
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        out.extend_from_slice(&0x4000_0000u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&chunk.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&16u64.to_le_bytes());
        out.extend_from_slice(&0xFDF5_1F03u32.to_le_bytes());
        out.extend_from_slice(ZIP);
        for s in &streams {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
        }
        out.extend_from_slice(b"padding\0");
        out.resize(data_offset as usize, 0);
        for s in &streams {
            out.extend_from_slice(s);
            let aligned = out.len().div_ceil(16) * 16;
            out.resize(aligned, 0);
        }
        out
    }

    #[test]
    fn a_save_confirms_only_the_plugin_set_it_lists() {
        use kitkraft_doc::{LoadOptions, LoadOrder, PluginSpec};
        let dir = temp("match");
        let names: Vec<&str> = DEV_FULL
            .iter()
            .chain(&DEV_SMALL)
            .chain(&DEV_MEDIUM)
            .map(|(n, _)| *n)
            .collect();
        for n in &names {
            std::fs::write(dir.join(n), tiny_plugin(dev_flags(n))).unwrap();
        }
        let spec = |n: &str| PluginSpec {
            name: n.into(),
            path: dir.join(n),
        };
        let save_path = dir.join("Autosave1.sfs");
        std::fs::write(
            &save_path,
            bcps(&save_data(&DEV_FULL, &DEV_SMALL, &DEV_MEDIUM), 0x40000),
        )
        .unwrap();
        let save = read_save_header(&save_path).unwrap();
        // The old order (blueprint plugins and SFBGS006 early): the save corrects it.
        let old = [
            "Starfield.esm",
            "Constellation.esm",
            "OldMars.esm",
            "BlueprintShips-Starfield.esm",
            "SFBGS007.esm",
            "SFBGS008.esm",
            "SFBGS006.esm",
            "SFBGS003.esm",
            "KitKraftFirstPOI.esm",
            "SFBGS004.esm",
            "SFBGS00D.esm",
            "SFBGS047.esm",
            "SFBGS050.esm",
            "BlueprintShips-SFBGS050.esm",
            "ShatteredSpace.esm",
        ];
        let mut lo = LoadOrder::load(
            old.iter().map(|n| spec(n)).collect(),
            &LoadOptions::default(),
        )
        .unwrap();
        let m = match_load_order(&lo, &save);
        assert!(m.confirmed(), "{m:?}");
        // In this order the POI comes before three official full plugins.
        assert!(
            m.differs_from_inferred
                .contains(&"KitKraftFirstPOI.esm: inferred 01, save 04".to_string()),
            "{m:?}"
        );
        assert!(
            m.differs_from_inferred
                .contains(&"SFBGS003.esm: inferred FD:01, save FD:00".to_string()),
            "{m:?}"
        );
        assert!(
            m.differs_from_inferred
                .contains(&"ShatteredSpace.esm: inferred 04, save 01".to_string()),
            "{m:?}"
        );
        lo.adopt_runtime_slots(m.slots.clone().unwrap(), "Autosave1.sfs".into())
            .unwrap();
        let poi = lo.find("KitKraftFirstPOI.esm").unwrap();
        assert_eq!(
            lo.plugin(poi).unwrap().runtime_slot,
            Some(RuntimeSlot::Full(4))
        );
        assert_eq!(lo.runtime_basis.as_str(), "confirmed-by-save");

        // A plugin the save does not list: nothing is confirmed.
        std::fs::write(dir.join("KitKraftGunOverhaul.esm"), tiny_plugin(0x1)).unwrap();
        let mut more: Vec<PluginSpec> = old.iter().map(|n| spec(n)).collect();
        more.insert(9, spec("KitKraftGunOverhaul.esm"));
        let lo = LoadOrder::load(more, &LoadOptions::default()).unwrap();
        let m = match_load_order(&lo, &save);
        assert!(!m.confirmed());
        assert_eq!(m.only_in_load_order, ["KitKraftGunOverhaul.esm (full)"]);
        // A save plugin the load order lacks, and a scale that disagrees.
        let fewer: Vec<PluginSpec> = old
            .iter()
            .filter(|n| **n != "SFBGS047.esm")
            .map(|n| spec(n))
            .collect();
        std::fs::write(dir.join("OldMars.esm"), tiny_plugin(0x81)).unwrap();
        let lo = LoadOrder::load(fewer, &LoadOptions::default()).unwrap();
        let m = match_load_order(&lo, &save);
        assert!(!m.confirmed());
        assert_eq!(m.only_in_save, ["SFBGS047.esm (small)"]);
        assert_eq!(
            m.scale_mismatch,
            ["OldMars.esm: the save lists it as small, its header says full"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    pub(crate) fn temp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kitkraft-io-save-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_the_plugin_lists_across_chunks() {
        let dir = temp("lists");
        let data = save_data(&DEV_FULL, &DEV_SMALL, &DEV_MEDIUM);
        // Chunks of 97 bytes: the lists span several chunks.
        let path = dir.join("Autosave1.sfs");
        std::fs::write(&path, bcps(&data, 97)).unwrap();
        let h = read_save_header(&path).unwrap();
        let names = |l: &[SavePlugin]| l.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&h.full), DEV_FULL.map(|(n, _)| n.to_string()));
        assert_eq!(names(&h.small), DEV_SMALL.map(|(n, _)| n.to_string()));
        assert_eq!(names(&h.medium), DEV_MEDIUM.map(|(n, _)| n.to_string()));
        assert_eq!(h.game_version, "1.16.244.0");
        assert_eq!((h.save_version, h.form_version), (27, 153));
        assert_eq!(h.trailing, 2);
        assert!(h.chunks_read > 1 && h.chunks_read < h.chunks, "{h:?}");
        assert_eq!(h.full[4].flag, 1);
        assert_eq!(
            h.full[4].extra,
            Some((0, "18446744073709551615.undefined".into()))
        );
        assert_eq!(h.slot("kitkraftfirstpoi.esm"), Some(RuntimeSlot::Full(4)));
        assert_eq!(h.slot("SFBGS004.esm"), Some(RuntimeSlot::Small(2)));
        assert_eq!(h.slot("SFBGS006.esm"), Some(RuntimeSlot::Medium(1)));
        assert_eq!(h.slot("Nope.esm"), None);
        // One big chunk reads the same lists from the first chunk alone.
        std::fs::write(&path, bcps(&data, 0x40000)).unwrap();
        let one = read_save_header(&path).unwrap();
        assert_eq!((one.chunks, one.chunks_read), (1, 1));
        assert_eq!(one.full, h.full);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hostile_saves_are_errors_not_panics() {
        let dir = temp("hostile");
        let data = save_data(&DEV_FULL, &DEV_SMALL, &DEV_MEDIUM);
        let good = bcps(&data, 4096);
        let path = dir.join("x.sfs");
        let check = |bytes: &[u8], needle: &str| {
            std::fs::write(&path, bytes).unwrap();
            let e = read_save_header(&path).unwrap_err().to_string();
            assert!(e.contains(needle), "{needle}: {e}");
        };
        check(b"BCPS", "too short");
        let mut b = good.clone();
        b[0] = b'X';
        check(&b, "magic");
        let mut b = good.clone();
        b[0x44..0x48].copy_from_slice(b"LZ4 ");
        check(&b, "compression tag");
        let mut b = good.clone();
        b[0x38] = 3;
        check(&b, "alignment");
        let mut b = good.clone();
        b[0x20..0x28].copy_from_slice(&u64::MAX.to_le_bytes());
        check(&b, "do not fit");
        // Truncated inside the compressed data.
        check(&good[..good.len() - 40], "past the end of the file");
        // A corrupt zlib stream.
        let mut z = good.clone();
        let data_at = u64::from_le_bytes(z[0x18..0x20].try_into().unwrap()) as usize;
        z[data_at] = 0;
        check(&z, "chunk 0");
        // An unknown entry flag.
        let mut d = data.clone();
        let name_at = d.windows(13).position(|w| w == b"Starfield.esm").unwrap();
        d[name_at + 13] = 7;
        check(&bcps(&d, 4096), "flag 7");
        // A block size smaller than the lists.
        let mut d = data.clone();
        let size_at = name_at - 2 - 1 - 4;
        d[size_at..size_at + 4].copy_from_slice(&10u32.to_le_bytes());
        check(&bcps(&d, 4096), "past the plugin block");
        // Not a save inside.
        check(&bcps(b"NOT_A_SAVE_AT_ALL", 4096), "SFS_SAVEGAME");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn newest_and_local_saves() {
        let dir = temp("newest");
        let profile = dir.join("P");
        std::fs::create_dir_all(profile.join("saves")).unwrap();
        std::fs::write(
            profile.join("settings.ini"),
            "[General]\r\nLocalSaves=true\r\n",
        )
        .unwrap();
        assert_eq!(saves_dir(&profile), Some(profile.join("saves")));
        let saves = profile.join("saves");
        assert_eq!(newest_save(&saves).unwrap(), None);
        std::fs::write(saves.join("Save1.sfs"), b"a").unwrap();
        std::fs::write(saves.join("notes.txt"), b"a").unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        File::options()
            .write(true)
            .open(saves.join("Save1.sfs"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        std::fs::write(saves.join("Autosave2.sfs"), b"b").unwrap();
        assert_eq!(
            newest_save(&saves).unwrap(),
            Some(saves.join("Autosave2.sfs"))
        );
        std::fs::write(
            profile.join("settings.ini"),
            "[General]\r\nLocalSaves=false\r\n",
        )
        .unwrap();
        let shared = saves_dir(&profile).unwrap();
        assert!(shared.ends_with("My Games/Starfield/Saves"), "{shared:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The owner's saves, read in place (read-only) when `KITKRAFT_SAVES` names a folder of
    /// `.sfs` files (copies; `docs/load-order.md`, "Save headers").
    #[test]
    #[ignore = "reads the owner's save copies; set KITKRAFT_SAVES"]
    fn owner_saves() {
        let Some(dir) = std::env::var_os("KITKRAFT_SAVES") else {
            eprintln!("KITKRAFT_SAVES unset: skipped");
            return;
        };
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "sfs"))
            .collect();
        paths.sort();
        for p in paths {
            let h = read_save_header(&p).unwrap();
            println!(
                "{}: v{} chunks {}/{} save {} form {} game {} / {} block 0x{:X}+{} trailing {} full {} small {} medium {}",
                p.file_name().unwrap().to_string_lossy(),
                h.container_version,
                h.chunks_read,
                h.chunks,
                h.save_version,
                h.form_version,
                h.game_version,
                h.game_version_2,
                h.plugin_block_offset,
                h.plugin_block_size,
                h.trailing,
                h.full.len(),
                h.small.len(),
                h.medium.len()
            );
            for (scale, e) in h.plugins() {
                println!(
                    "  {} {:>3} {} flag {} {:?}",
                    scale.as_str(),
                    h.list(scale).iter().position(|x| x == e).unwrap(),
                    e.name,
                    e.flag,
                    e.extra
                );
            }
            assert_eq!(h.trailing, 2);
        }
    }
}
