//! Localized string tables: `.STRINGS`, `.DLSTRINGS`, `.ILSTRINGS`.
//!
//! Layout (`uesp-skyrim: https://en.uesp.net/wiki/Skyrim_Mod:String_Table_File_Format`,
//! confirmed on all 378 tables in the official archives, ADR 0006):
//!
//! | Offset | Size | Field |
//! | ------ | ---- | ----- |
//! | 0 | 4 | entry count `n` |
//! | 4 | 4 | data size |
//! | 8 | 8 x n | directory: string ID `u32`, offset `u32` relative to the data start |
//! | 8 + 8n | data size | data |
//!
//! In `.STRINGS` a string is NUL-terminated. In `.DLSTRINGS` and `.ILSTRINGS` it is a
//! `u32` length (counting the NUL) followed by the bytes and the NUL.
//!
//! `corpus:` in every official table the directory is sorted by ID with no duplicates,
//! identical strings are stored once and shared, the data holds the strings in the order
//! the directory first refers to them, and there are no gaps or trailing bytes.
//! [`Table::to_bytes`] writes exactly that layout, so every official table rewrites
//! byte-identically. Text is UTF-8 in the official files except a few entries of the
//! English tables (Windows-1252), so strings are kept as bytes; [`Entry::text`] shows a
//! non-UTF-8 entry as Windows-1252.
//!
//! [`Table::parse`] is strict: the header's data size must match the bytes after the
//! directory. [`ParseMode::Tolerant`] (M3.G2) also reads a table padded past its declared
//! data (no official table is), ignoring the padding and naming it ([`Padding`]), along
//! with any unreferenced bytes at the end of the declared data.
//!
//! LAYER: L0

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

/// Which of the three table kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// `.STRINGS`: NUL-terminated (names: FULL and the like).
    Strings,
    /// `.DLSTRINGS`: length-prefixed (descriptions, book text).
    DlStrings,
    /// `.ILSTRINGS`: length-prefixed (dialogue).
    IlStrings,
}

impl Kind {
    /// All three, in file-name order.
    pub const ALL: [Kind; 3] = [Kind::Strings, Kind::DlStrings, Kind::IlStrings];

    /// The kind a file name's extension names (ASCII case-insensitive).
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        Self::from_extension(ext)
    }

    /// The kind for an extension without the dot (`strings`, `DLSTRINGS`, ...).
    pub fn from_extension(ext: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.extension().eq_ignore_ascii_case(ext))
    }

    /// Lower-case extension without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Strings => "strings",
            Self::DlStrings => "dlstrings",
            Self::IlStrings => "ilstrings",
        }
    }

    /// Whether strings carry a `u32` length prefix.
    pub fn length_prefixed(self) -> bool {
        !matches!(self, Self::Strings)
    }
}

/// The table file name for a plugin and language: `starfield_en.strings` for
/// (`Starfield.esm`, `en`). Lower-case, as in the official archives; lookups should be
/// case-insensitive anyway.
pub fn table_name(plugin: &str, language: &str, kind: Kind) -> String {
    let stem = Path::new(plugin)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(plugin);
    format!(
        "{}_{}.{}",
        stem.to_ascii_lowercase(),
        language.to_ascii_lowercase(),
        kind.extension()
    )
}

/// Errors, each with the byte offset in the table where it was found.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The file ends inside a structure.
    #[error("truncated at offset {offset}: needed {needed} bytes, {available} available")]
    Truncated {
        /// Where the structure starts.
        offset: u64,
        /// Bytes needed.
        needed: u64,
        /// Bytes present.
        available: u64,
    },
    /// The header's data size disagrees with the bytes after the directory.
    #[error(
        "data size at offset 4 says {declared} bytes, the file has {actual} after the directory"
    )]
    DataSize {
        /// Header value.
        declared: u64,
        /// Bytes present.
        actual: u64,
    },
    /// A directory offset points outside the data.
    #[error("string {id:#010X} (directory entry at offset {entry}) points at data offset {offset}, past the end ({data_len})")]
    BadOffset {
        /// String ID.
        id: u32,
        /// Offset of the directory entry.
        entry: u64,
        /// The data-relative offset.
        offset: u64,
        /// Data length.
        data_len: u64,
    },
    /// A string runs past the end of the data, or lacks its NUL.
    #[error("string {id:#010X} at offset {offset}: {reason}")]
    BadString {
        /// String ID.
        id: u32,
        /// Absolute offset of the string (its length prefix for DL/IL tables).
        offset: u64,
        /// What is wrong.
        reason: &'static str,
    },
    /// Writing: a `.STRINGS` entry contains a NUL, or the table outgrows `u32` sizes.
    #[error("cannot write string {id:#010X}: {reason}")]
    Unwritable {
        /// String ID.
        id: u32,
        /// Why.
        reason: &'static str,
    },
    /// Merging tables of different kinds.
    #[error("cannot merge a {a:?} table with a {b:?} table")]
    KindMismatch {
        /// First kind.
        a: Kind,
        /// The other kind.
        b: Kind,
    },
    /// Reading a file failed.
    #[error("reading {path}: {message}")]
    Io {
        /// The file.
        path: String,
        /// The I/O error.
        message: String,
    },
}

/// How strictly [`Table::parse_with`] reads a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParseMode {
    /// The header's data size equals the bytes after the directory, as in every official
    /// table; anything else is [`Error::DataSize`]. What [`Table::parse`] does.
    #[default]
    Strict,
    /// Also accepts bytes after the declared data (a table padded, for example to an
    /// alignment, by another tool). Every padding run, and any unreferenced bytes at the
    /// end of the declared data, is reported in [`Parsed::padding`]. A file shorter than
    /// its declared data is still [`Error::DataSize`].
    Tolerant,
}

/// Where a padding run sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaddingKind {
    /// Inside the header's data size, after the end of the last string.
    DataTail,
    /// After the header's data size, to the end of the file.
    AfterData,
}

/// Bytes a [`ParseMode::Tolerant`] parse found outside every string, and ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Padding {
    /// Where it sits.
    pub kind: PaddingKind,
    /// Absolute offset in the table file.
    pub offset: u64,
    /// Length in bytes.
    pub len: u64,
    /// Whether every byte is zero.
    pub zeros: bool,
}

impl Padding {
    fn new(kind: PaddingKind, offset: u64, bytes: &[u8]) -> Self {
        Self {
            kind,
            offset,
            len: bytes.len() as u64,
            zeros: bytes.iter().all(|&b| b == 0),
        }
    }
}

impl std::fmt::Display for Padding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let where_ = match self.kind {
            PaddingKind::DataTail => "inside the declared data, after the last string",
            PaddingKind::AfterData => "after the header's data size",
        };
        let what = if self.zeros { "zero" } else { "non-zero" };
        write!(
            f,
            "{} bytes of {what} padding at offset {} ({where_})",
            self.len, self.offset
        )
    }
}

/// A table and what a [`ParseMode::Tolerant`] parse ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    /// The table.
    pub table: Table,
    /// Padding runs in file order; always empty in [`ParseMode::Strict`].
    pub padding: Vec<Padding>,
}

/// One string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// String ID (what a localized plugin's lstring subrecord holds).
    pub id: u32,
    /// The bytes, without the length prefix and the terminating NUL.
    pub bytes: Vec<u8>,
    /// Absolute offset of the string in the table it was read from (its length prefix
    /// for DL/IL), for evidence; `None` for entries not read from a file.
    pub offset: Option<u64>,
}

impl Entry {
    /// A new entry not read from a file.
    pub fn new(id: u32, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            id,
            bytes: bytes.into(),
            offset: None,
        }
    }

    /// The text: the bytes as UTF-8 when they are valid UTF-8, otherwise the whole entry
    /// decoded as Windows-1252, one char per byte. A few official entries are Windows-1252
    /// (`Starfield - Localization.ba2` dlstrings `0x000301B6` holds `0x92` for an
    /// apostrophe; SF1Edit oracle row X8, `docs/oracles.md`), and SF1Edit shows them that
    /// way. Display only: compare and copy by [`Entry::bytes`] ([`Entry::is_utf8`] says
    /// which reading applied).
    pub fn text(&self) -> Cow<'_, str> {
        match std::str::from_utf8(&self.bytes) {
            Ok(text) => Cow::Borrowed(text),
            Err(_) => Cow::Owned(decode_cp1252(&self.bytes)),
        }
    }

    /// Whether the bytes are valid UTF-8.
    pub fn is_utf8(&self) -> bool {
        std::str::from_utf8(&self.bytes).is_ok()
    }

    /// Bytes the string occupies in a table of `kind` (prefix and NUL included).
    pub fn stored_len(&self, kind: Kind) -> u64 {
        let base = (self.bytes.len() as u64).saturating_add(1);
        if kind.length_prefixed() {
            base.saturating_add(4)
        } else {
            base
        }
    }
}

/// A string table: entries in directory order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Which kind.
    pub kind: Kind,
    /// Entries in directory order.
    pub entries: Vec<Entry>,
    index: HashMap<u32, usize>,
}

impl Table {
    /// A table from entries (directory order as given). For a repeated ID, lookups find
    /// the first.
    pub fn new(kind: Kind, entries: Vec<Entry>) -> Self {
        let mut index = HashMap::with_capacity(entries.len());
        for (i, e) in entries.iter().enumerate() {
            index.entry(e.id).or_insert(i);
        }
        Self {
            kind,
            entries,
            index,
        }
    }

    /// Parses a table strictly: the header's data size must equal the bytes after the
    /// directory (the layout of every official table).
    pub fn parse(kind: Kind, bytes: &[u8]) -> Result<Self, Error> {
        Self::parse_with(kind, bytes, ParseMode::Strict).map(|p| p.table)
    }

    /// Parses a table in `mode`. [`ParseMode::Tolerant`] also accepts a file that runs past
    /// the header's data size, and reports that and any unreferenced bytes at the end of
    /// the data as [`Parsed::padding`]; strings must still lie inside the declared data.
    pub fn parse_with(kind: Kind, bytes: &[u8], mode: ParseMode) -> Result<Parsed, Error> {
        let len = bytes.len() as u64;
        let u32_at = |at: u64| -> Result<u32, Error> {
            usize::try_from(at)
                .ok()
                .and_then(|a| bytes.get(a..a.checked_add(4)?))
                .and_then(|b| <[u8; 4]>::try_from(b).ok())
                .map(u32::from_le_bytes)
                .ok_or(Error::Truncated {
                    offset: at,
                    needed: 4,
                    available: len.saturating_sub(at),
                })
        };
        let count = u64::from(u32_at(0)?);
        let declared = u64::from(u32_at(4)?);
        let dir_len = count.saturating_mul(8);
        let data_start = dir_len.saturating_add(8);
        if data_start > len {
            return Err(Error::Truncated {
                offset: 8,
                needed: dir_len,
                available: len.saturating_sub(8),
            });
        }
        let actual = len.saturating_sub(data_start);
        let mut padding = Vec::new();
        let data_len = match mode {
            ParseMode::Tolerant if declared < actual => {
                let offset = data_start.saturating_add(declared);
                padding.push(Padding::new(
                    PaddingKind::AfterData,
                    offset,
                    usize::try_from(offset)
                        .ok()
                        .and_then(|o| bytes.get(o..))
                        .unwrap_or_default(),
                ));
                declared
            }
            _ if declared != actual => {
                return Err(Error::DataSize { declared, actual });
            }
            _ => actual,
        };
        let data = usize::try_from(data_start)
            .ok()
            .zip(usize::try_from(data_len).ok())
            .and_then(|(s, n)| bytes.get(s..s.checked_add(n)?))
            .unwrap_or_default();
        // End of the furthest string, relative to the data (for the data-tail padding).
        let mut used = 0u64;

        // `count` <= (len - 8) / 8, so this allocation is bounded by the input size.
        let mut entries = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for i in 0..count {
            let entry_at = i.saturating_mul(8).saturating_add(8);
            let id = u32_at(entry_at)?;
            let rel = u64::from(u32_at(entry_at.saturating_add(4))?);
            if rel >= data_len {
                return Err(Error::BadOffset {
                    id,
                    entry: entry_at,
                    offset: rel,
                    data_len,
                });
            }
            let abs = data_start.saturating_add(rel);
            let bad = |reason| Error::BadString {
                id,
                offset: abs,
                reason,
            };
            let at = usize::try_from(rel).map_err(|_| bad("offset too large"))?;
            let rest = data.get(at..).unwrap_or_default();
            let text = if kind.length_prefixed() {
                let n = rest
                    .get(..4)
                    .and_then(|b| <[u8; 4]>::try_from(b).ok())
                    .map(u32::from_le_bytes)
                    .ok_or_else(|| bad("length prefix runs past the end"))?;
                let n = usize::try_from(n).map_err(|_| bad("length too large"))?;
                let body = rest
                    .get(4..)
                    .and_then(|r| r.get(..n))
                    .ok_or_else(|| bad("string runs past the end"))?;
                match body.split_last() {
                    Some((0, text)) => text,
                    _ => return Err(bad("length-prefixed string does not end in NUL")),
                }
            } else {
                let nul = rest
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or_else(|| bad("no NUL before the end of the data"))?;
                rest.get(..nul).unwrap_or_default()
            };
            let stored = (text.len() as u64)
                .saturating_add(1)
                .saturating_add(if kind.length_prefixed() { 4 } else { 0 });
            used = used.max(rel.saturating_add(stored));
            entries.push(Entry {
                id,
                bytes: text.to_vec(),
                offset: Some(abs),
            });
        }
        if mode == ParseMode::Tolerant && used < data_len {
            let start = usize::try_from(used).unwrap_or(usize::MAX);
            padding.insert(
                0,
                Padding::new(
                    PaddingKind::DataTail,
                    data_start.saturating_add(used),
                    data.get(start..).unwrap_or_default(),
                ),
            );
        }
        Ok(Parsed {
            table: Self::new(kind, entries),
            padding,
        })
    }

    /// Reads a table file; the kind comes from its extension.
    pub fn read_file(path: &Path) -> Result<Self, Error> {
        let kind = Kind::from_path(path).ok_or_else(|| Error::Io {
            path: path.display().to_string(),
            message: "extension is not .strings, .dlstrings or .ilstrings".into(),
        })?;
        let bytes = std::fs::read(path).map_err(|e| Error::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        Self::parse(kind, &bytes)
    }

    /// The entry for `id` (the first, if the ID repeats).
    pub fn get(&self, id: u32) -> Option<&Entry> {
        self.entries.get(*self.index.get(&id)?)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// IDs that occur more than once, in first-occurrence order.
    pub fn duplicate_ids(&self) -> Vec<u32> {
        let mut seen = HashMap::new();
        let mut dups = Vec::new();
        for e in &self.entries {
            let n = seen.entry(e.id).or_insert(0u32);
            *n = n.saturating_add(1);
            if *n == 2 {
                dups.push(e.id);
            }
        }
        dups
    }

    /// Serializes in directory order with identical strings stored once, in first-reference
    /// order: the layout of every official table.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let too_big = |id| Error::Unwritable {
            id,
            reason: "table larger than 4 GiB",
        };
        let count = u32::try_from(self.entries.len()).map_err(|_| too_big(0))?;
        let mut data: Vec<u8> = Vec::new();
        let mut placed: HashMap<&[u8], u32> = HashMap::new();
        let mut dir = Vec::with_capacity(self.entries.len().saturating_mul(8));
        for e in &self.entries {
            if !self.kind.length_prefixed() && e.bytes.contains(&0) {
                return Err(Error::Unwritable {
                    id: e.id,
                    reason: "a .STRINGS entry cannot contain NUL",
                });
            }
            let offset = match placed.get(e.bytes.as_slice()) {
                Some(&o) => o,
                None => {
                    let o = u32::try_from(data.len()).map_err(|_| too_big(e.id))?;
                    if self.kind.length_prefixed() {
                        let n = e.bytes.len().checked_add(1).ok_or_else(|| too_big(e.id))?;
                        let n = u32::try_from(n).map_err(|_| too_big(e.id))?;
                        data.extend_from_slice(&n.to_le_bytes());
                    }
                    data.extend_from_slice(&e.bytes);
                    data.push(0);
                    placed.insert(e.bytes.as_slice(), o);
                    o
                }
            };
            dir.extend_from_slice(&e.id.to_le_bytes());
            dir.extend_from_slice(&offset.to_le_bytes());
        }
        let data_len = u32::try_from(data.len()).map_err(|_| too_big(0))?;
        let mut out =
            Vec::with_capacity(8usize.saturating_add(dir.len()).saturating_add(data.len()));
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&data_len.to_le_bytes());
        out.extend_from_slice(&dir);
        out.extend_from_slice(&data);
        Ok(out)
    }
}

/// An ID two merged tables both define with different bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// String ID.
    pub id: u32,
    /// Index (into the merge inputs) of the table whose value was replaced.
    pub replaced: usize,
    /// Index of the table whose value won.
    pub winner: usize,
}

/// An ID repeated inside one input table. Not a conflict between inputs: within a table
/// the first occurrence wins, as [`Table::get`] does, and the repeat is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duplicate {
    /// String ID.
    pub id: u32,
    /// Index (into the merge inputs) of the table that repeats it.
    pub table: usize,
    /// Whether the dropped repeat has the same bytes as the first occurrence.
    pub same_bytes: bool,
}

/// What [`merge`] produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// The merged table, sorted by ID.
    pub table: Table,
    /// IDs defined differently by two inputs, in ID order (one per replacement).
    pub conflicts: Vec<Conflict>,
    /// IDs repeated inside one input, in input then directory order (one per repeat).
    pub duplicates: Vec<Duplicate>,
    /// IDs defined again by a later input with the same bytes.
    pub identical_repeats: u64,
}

/// Merges tables of one kind: a later table wins for an ID; within one table the first
/// occurrence of an ID wins (reported in [`Merged::duplicates`], apart from the
/// cross-table [`Merged::conflicts`]). The result is sorted by ID, so it is the same for
/// the same inputs.
pub fn merge(tables: &[Table]) -> Result<Merged, Error> {
    let kind = match tables.first() {
        Some(t) => t.kind,
        None => Kind::Strings,
    };
    let mut map: std::collections::BTreeMap<u32, (usize, &Entry)> = Default::default();
    let mut conflicts = Vec::new();
    let mut duplicates = Vec::new();
    let mut identical_repeats = 0u64;
    for (i, table) in tables.iter().enumerate() {
        if table.kind != kind {
            return Err(Error::KindMismatch {
                a: kind,
                b: table.kind,
            });
        }
        let mut first: HashMap<u32, &Entry> = HashMap::with_capacity(table.entries.len());
        for e in &table.entries {
            if let Some(kept) = first.get(&e.id) {
                duplicates.push(Duplicate {
                    id: e.id,
                    table: i,
                    same_bytes: kept.bytes == e.bytes,
                });
                continue;
            }
            first.insert(e.id, e);
            if let Some((prev, old)) = map.insert(e.id, (i, e)) {
                if old.bytes == e.bytes {
                    identical_repeats = identical_repeats.saturating_add(1);
                } else {
                    conflicts.push(Conflict {
                        id: e.id,
                        replaced: prev,
                        winner: i,
                    });
                }
            }
        }
    }
    conflicts.sort_by_key(|c| (c.id, c.winner));
    let entries = map
        .into_values()
        .map(|(_, e)| Entry::new(e.id, e.bytes.clone()))
        .collect();
    Ok(Merged {
        table: Table::new(kind, entries),
        conflicts,
        duplicates,
        identical_repeats,
    })
}

/// Windows-1252 code points of bytes 0x80..=0x9F; the five bytes the code page leaves
/// undefined (0x81, 0x8D, 0x8F, 0x90, 0x9D) map to the C1 control of the same value, so
/// the mapping stays one-to-one (as `kitkraft-ba2` reads archive names). Every other byte
/// is its own code point (as in Latin-1).
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

/// `bytes` decoded as Windows-1252, one char per byte.
fn decode_cp1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            b.checked_sub(0x80)
                .and_then(|i| CP1252_HIGH.get(usize::from(i)))
                .and_then(|&u| char::from_u32(u32::from(u)))
                .unwrap_or(char::from(b))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
mod tests;
