//! Streaming roundtrip driver.
//!
//! Reads a plugin one top-level entry at a time (the `TES4` record, then each top-level
//! group), parses it, writes it back, and compares the output with the bytes read. Only one
//! top-level entry is in memory at once, plus its parsed tree; the output is compared as it
//! is produced and never buffered (unless a caller passes a sink). The first mismatch is
//! reported as an absolute offset and a group/record/subrecord path.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::Path;

use crate::model::{Body, Entry, Group, Record, Subrecord, HEADER_LEN};
use crate::{DecodeLimits, Error, ParseOptions, Result, Signature, WriteOptions};

/// Roundtrip settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RoundtripOptions {
    pub parse: ParseOptions,
    /// When set, every compressed record is also decompressed and its subrecords parsed
    /// (and counted). The original compressed bytes are still what is written.
    pub decode: Option<DecodeLimits>,
    pub write: WriteOptions,
}

impl RoundtripOptions {
    /// Default limits, decoding every compressed record.
    pub fn with_decode() -> RoundtripOptions {
        RoundtripOptions {
            decode: Some(DecodeLimits::default()),
            ..RoundtripOptions::default()
        }
    }
}

/// Counts gathered while roundtripping.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// Top-level entries, including the `TES4` record.
    pub top_level_entries: u64,
    /// Groups at any depth.
    pub groups: u64,
    /// Records at any depth, including `TES4`.
    pub records: u64,
    /// Records with the compressed flag.
    pub compressed_records: u64,
    /// Compressed records that were decoded (only when decoding is enabled).
    pub decoded_records: u64,
    /// Subrecords in uncompressed records, plus those in decoded compressed records.
    pub subrecords: u64,
    /// Subrecords whose size came from an `XXXX` prefix.
    pub oversize_subrecords: u64,
    /// Size in bytes of the largest top-level entry (what the driver holds in memory).
    pub largest_top_level_entry: u64,
    /// Largest decompressed size of a decoded record.
    pub largest_decompressed: u64,
    /// Record count per signature.
    pub record_types: BTreeMap<Signature, u64>,
}

/// Where the output first differs from the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// Absolute offset of the first differing byte.
    pub offset: u64,
    /// Input byte there (`None` past the end of the input entry).
    pub expected: Option<u8>,
    /// Output byte there (`None` when the output ended early).
    pub actual: Option<u8>,
    /// Path of the innermost node containing the offset, e.g.
    /// `GRUP WRLD (type 0) @123 > WRLD 0000003C @147 > FULL @171`.
    pub path: String,
}

/// Result of a roundtrip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundtripReport {
    /// Input length in bytes.
    pub input_len: u64,
    /// Bytes produced before the comparison stopped.
    pub output_len: u64,
    /// The first mismatch, or `None` when the output is byte-identical.
    pub mismatch: Option<Mismatch>,
    pub stats: Stats,
}

impl RoundtripReport {
    /// Whether the rewrite was byte-identical.
    pub fn identical(&self) -> bool {
        self.mismatch.is_none() && self.input_len == self.output_len
    }
}

/// Roundtrip the file at `path`, streaming one top-level entry at a time.
pub fn roundtrip_file(path: &Path, options: &RoundtripOptions) -> Result<RoundtripReport> {
    roundtrip_file_to(path, options, None)
}

/// As [`roundtrip_file`], also writing the output to `sink`.
pub fn roundtrip_file_to(
    path: &Path,
    options: &RoundtripOptions,
    sink: Option<&mut dyn Write>,
) -> Result<RoundtripReport> {
    let file = File::open(path).map_err(|source| Error::Io { offset: 0, source })?;
    let len = file
        .metadata()
        .map_err(|source| Error::Io { offset: 0, source })?
        .len();
    roundtrip_reader(BufReader::with_capacity(1 << 20, file), len, options, sink)
}

/// Roundtrip a plugin held in memory, with the same logic as the file driver.
pub fn roundtrip_bytes(data: &[u8], options: &RoundtripOptions) -> Result<RoundtripReport> {
    roundtrip_reader(data, data.len() as u64, options, None)
}

/// Roundtrip `len` bytes from `input`.
pub fn roundtrip_reader<R: Read>(
    mut input: R,
    len: u64,
    options: &RoundtripOptions,
    mut sink: Option<&mut dyn Write>,
) -> Result<RoundtripReport> {
    let mut stats = Stats::default();
    let mut entries = TopLevelReader::new(&mut input, len);
    while let Some((offset, buf)) = entries.next_entry()? {
        let total = buf.len() as u64;
        let entry = Entry::parse_one(&buf, offset, 1, &options.parse)?;
        stats.top_level_entries = stats.top_level_entries.saturating_add(1);
        stats.largest_top_level_entry = stats.largest_top_level_entry.max(total);
        count_entry(&entry, &mut stats, options.decode.as_ref())?;

        let mut cmp = Compare {
            expected: &buf,
            pos: 0,
            diff: None,
            extra: match sink {
                Some(ref mut s) => Some(&mut **s),
                None => None,
            },
        };
        entry.write_to(&mut cmp, offset, &options.write)?;
        let written = cmp.pos as u64;
        let diff = cmp.diff.or_else(|| {
            (cmp.pos != buf.len()).then(|| Diff {
                at: cmp.pos.min(buf.len()),
                actual: None,
            })
        });
        if let Some(diff) = diff {
            let at = offset.saturating_add(diff.at as u64);
            return Ok(RoundtripReport {
                input_len: len,
                output_len: offset.saturating_add(written),
                mismatch: Some(Mismatch {
                    offset: at,
                    expected: buf.get(diff.at).copied(),
                    actual: diff.actual,
                    path: locate(&entry, at),
                }),
                stats,
            });
        }
    }
    Ok(RoundtripReport {
        input_len: len,
        output_len: entries.offset,
        mismatch: None,
        stats,
    })
}

/// Call `visit` with each top-level entry of the file at `path` (the `TES4` record first),
/// parsed from a buffer holding only that entry.
pub fn visit_file(
    path: &Path,
    options: &ParseOptions,
    mut visit: impl FnMut(&Entry<'_>) -> Result<()>,
) -> Result<()> {
    let file = File::open(path).map_err(|source| Error::Io { offset: 0, source })?;
    let len = file
        .metadata()
        .map_err(|source| Error::Io { offset: 0, source })?
        .len();
    let mut input = BufReader::with_capacity(1 << 20, file);
    let mut entries = TopLevelReader::new(&mut input, len);
    while let Some((offset, buf)) = entries.next_entry()? {
        let entry = Entry::parse_one(&buf, offset, 1, options)?;
        visit(&entry)?;
    }
    Ok(())
}

/// Reads a plugin one top-level entry at a time. Each entry's declared size is checked
/// against the bytes left in the input before anything is allocated for it.
struct TopLevelReader<R: Read> {
    input: R,
    len: u64,
    offset: u64,
}

impl<R: Read> TopLevelReader<R> {
    fn new(input: R, len: u64) -> TopLevelReader<R> {
        TopLevelReader {
            input,
            len,
            offset: 0,
        }
    }

    /// The next entry's absolute offset and bytes, or `None` at the end of the input.
    fn next_entry(&mut self) -> Result<Option<(u64, Vec<u8>)>> {
        let offset = self.offset;
        if offset >= self.len {
            return Ok(None);
        }
        let available = self.len.saturating_sub(offset);
        if available < HEADER_LEN as u64 {
            return Err(Error::UnexpectedEof {
                offset,
                what: "entry header",
                needed: HEADER_LEN as u64,
                available,
            });
        }
        let mut header = [0u8; HEADER_LEN];
        self.input
            .read_exact(&mut header)
            .map_err(|source| Error::Io { offset, source })?;
        let mut r = crate::Reader::new(&header, offset);
        let signature = r.signature()?;
        let size = u64::from(r.u32()?);
        if offset == 0 && signature != Signature::TES4 {
            return Err(Error::NotAPlugin {
                offset,
                found: signature,
            });
        }
        let (total, what) = if signature == Signature::GRUP {
            if size < HEADER_LEN as u64 {
                return Err(Error::GroupTooSmall {
                    offset,
                    size: u32::try_from(size).unwrap_or(u32::MAX),
                });
            }
            (size, "group")
        } else {
            (size.saturating_add(HEADER_LEN as u64), "record")
        };
        if total > available {
            return Err(Error::UnexpectedEof {
                offset,
                what,
                needed: total,
                available,
            });
        }
        let total_usize = usize::try_from(total).map_err(|_| Error::TooLarge {
            offset,
            what,
            len: total,
        })?;
        let mut buf = Vec::with_capacity(total_usize);
        buf.extend_from_slice(&header);
        (&mut self.input)
            .take(total.saturating_sub(HEADER_LEN as u64))
            .read_to_end(&mut buf)
            .map_err(|source| Error::Io { offset, source })?;
        if buf.len() != total_usize {
            return Err(Error::UnexpectedEof {
                offset,
                what,
                needed: total,
                available: buf.len() as u64,
            });
        }
        self.offset = offset.saturating_add(total);
        Ok(Some((offset, buf)))
    }
}

#[derive(Debug, Clone, Copy)]
struct Diff {
    at: usize,
    actual: Option<u8>,
}

/// A `Write` that compares against the expected bytes as output arrives.
struct Compare<'e, 's> {
    expected: &'e [u8],
    pos: usize,
    diff: Option<Diff>,
    extra: Option<&'s mut dyn Write>,
}

impl Write for Compare<'_, '_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(extra) = self.extra.as_deref_mut() {
            extra.write_all(buf)?;
        }
        if self.diff.is_none() {
            let rest = self.expected.get(self.pos..).unwrap_or(&[]);
            let n = buf.len().min(rest.len());
            if buf.get(..n) != rest.get(..n) {
                let i = buf.iter().zip(rest).position(|(a, b)| a != b).unwrap_or(n);
                self.diff = Some(Diff {
                    at: self.pos.saturating_add(i),
                    actual: buf.get(i).copied(),
                });
            } else if buf.len() > rest.len() {
                self.diff = Some(Diff {
                    at: self.pos.saturating_add(n),
                    actual: buf.get(n).copied(),
                });
            }
        }
        self.pos = self.pos.saturating_add(buf.len());
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.extra.as_deref_mut() {
            Some(extra) => extra.flush(),
            None => Ok(()),
        }
    }
}

fn count_entry(entry: &Entry<'_>, stats: &mut Stats, decode: Option<&DecodeLimits>) -> Result<()> {
    match entry {
        Entry::Group(g) => {
            stats.groups = stats.groups.saturating_add(1);
            for child in g.children() {
                count_entry(child, stats, decode)?;
            }
        }
        Entry::Record(r) => count_record(r, stats, decode)?,
    }
    Ok(())
}

fn count_subrecords(subs: &[Subrecord<'_>], stats: &mut Stats) {
    stats.subrecords = stats.subrecords.saturating_add(subs.len() as u64);
    let oversize = subs.iter().filter(|s| s.has_oversize_prefix()).count() as u64;
    stats.oversize_subrecords = stats.oversize_subrecords.saturating_add(oversize);
}

fn count_record(r: &Record<'_>, stats: &mut Stats, decode: Option<&DecodeLimits>) -> Result<()> {
    stats.records = stats.records.saturating_add(1);
    let n = stats.record_types.entry(r.header.signature).or_insert(0);
    *n = n.saturating_add(1);
    match r.body() {
        Body::Raw(subs) => count_subrecords(subs, stats),
        Body::Compressed { .. } => {
            stats.compressed_records = stats.compressed_records.saturating_add(1);
            if let Some(limits) = decode {
                if let Some(subs) = r.decode(limits)? {
                    stats.decoded_records = stats.decoded_records.saturating_add(1);
                    let size: u64 = subs.iter().map(Subrecord::encoded_len).sum();
                    stats.largest_decompressed = stats.largest_decompressed.max(size);
                    count_subrecords(&subs, stats);
                }
            }
        }
    }
    Ok(())
}

fn describe_group(g: &Group<'_>) -> String {
    let h = &g.header;
    let label = match h.group_type {
        0 => h.label_signature().to_string(),
        2 | 3 => format!("block {}", h.label_u32()),
        4 | 5 => {
            let [a, b, c, d] = h.label;
            let y = i16::from_le_bytes([a, b]);
            let x = i16::from_le_bytes([c, d]);
            format!("grid {y},{x}")
        }
        _ => format!("{:08X}", h.label_u32()),
    };
    format!("GRUP {label} (type {})", h.group_type)
}

/// Path of the innermost node of `entry` containing absolute offset `at`.
fn locate(entry: &Entry<'_>, at: u64) -> String {
    let mut path = String::new();
    let mut current = Some(entry);
    while let Some(node) = current.take() {
        if !path.is_empty() {
            path.push_str(" > ");
        }
        match node {
            Entry::Group(g) => {
                let _ = write!(path, "{} @{}", describe_group(g), g.offset().unwrap_or(0));
                current = g.children().iter().find(|c| contains(c, at));
            }
            Entry::Record(r) => {
                let start = r.offset().unwrap_or(0);
                let _ = write!(
                    path,
                    "{} {:08X} @{start}",
                    r.header.signature, r.header.form_id
                );
                if let Body::Raw(subs) = r.body() {
                    let mut sub_at = start.saturating_add(HEADER_LEN as u64);
                    for s in subs {
                        let end = sub_at.saturating_add(s.encoded_len());
                        if (sub_at..end).contains(&at) {
                            let _ = write!(path, " > {} @{sub_at}", s.signature());
                            break;
                        }
                        sub_at = end;
                    }
                }
            }
        }
    }
    path
}

fn contains(entry: &Entry<'_>, at: u64) -> bool {
    let (start, len) = match entry {
        Entry::Group(g) => (g.offset(), g.size_as_read().map(u64::from)),
        Entry::Record(r) => (
            r.offset(),
            r.data_size_as_read()
                .map(|s| u64::from(s).saturating_add(HEADER_LEN as u64)),
        ),
    };
    match (start, len) {
        (Some(start), Some(len)) => (start..start.saturating_add(len)).contains(&at),
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{locate, roundtrip_bytes, RoundtripOptions};
    use crate::fixture::{self, group, sub};
    use crate::{Entry, Error, ParseOptions, Plugin};

    #[test]
    fn sample_roundtrips_with_stats() {
        let rec = fixture::compressed_record_stored(b"LAND", 9, &sub(b"VHGT", &[1u8; 100]));
        let mut bytes = fixture::sample_plugin();
        bytes.extend(group(b"LAND", 0, &rec));
        let report = roundtrip_bytes(&bytes, &RoundtripOptions::with_decode()).unwrap();
        assert!(report.identical(), "{report:?}");
        let s = &report.stats;
        assert_eq!(s.top_level_entries, 5);
        assert_eq!(s.groups, 4 + 3);
        // TES4, 2 GMST, CELL, 3 REFR, NAVM, LAND.
        assert_eq!(s.records, 9);
        assert_eq!(s.compressed_records, 1);
        assert_eq!(s.decoded_records, 1);
        assert_eq!(s.oversize_subrecords, 1);
        assert_eq!(s.largest_decompressed, 106);
        assert_eq!(s.record_types[&crate::Signature::new(b"REFR")], 3);
    }

    #[test]
    fn mismatch_reports_offset_and_path() {
        let bytes = fixture::sample_plugin();
        let plugin = Plugin::parse(&bytes).unwrap();
        let Entry::Group(cells) = &plugin.entries()[1] else {
            panic!()
        };
        let Entry::Group(children) = &cells.children()[1] else {
            panic!()
        };
        let Entry::Group(temporary) = &children.children()[1] else {
            panic!()
        };
        let Entry::Record(refr) = &temporary.children()[1] else {
            panic!()
        };
        // Second subrecord (DATA) of the last REFR: header 24 + NAME (6 + 4).
        let data_at = refr.offset().unwrap() + 24 + 10;
        let path = locate(&Entry::Group(cells.clone()), data_at + 7);
        assert_eq!(
            path,
            format!(
                "GRUP CELL (type 0) @{} > GRUP 00000200 (type 6) @{} > GRUP 00000200 (type 9) @{} > REFR 00000203 @{} > DATA @{data_at}",
                cells.offset().unwrap(),
                children.offset().unwrap(),
                temporary.offset().unwrap(),
                refr.offset().unwrap()
            )
        );
    }

    #[test]
    fn truncated_input_is_an_error() {
        let bytes = fixture::sample_plugin();
        let cut = &bytes[..bytes.len() - 1];
        match roundtrip_bytes(cut, &RoundtripOptions::default()) {
            Err(Error::UnexpectedEof { what: "group", .. }) => {}
            other => panic!("{other:?}"),
        }
        let not_plugin = group(b"GMST", 0, &[]);
        assert!(matches!(
            roundtrip_bytes(&not_plugin, &RoundtripOptions::default()),
            Err(Error::NotAPlugin { offset: 0, .. })
        ));
        let deep = RoundtripOptions {
            parse: ParseOptions { max_depth: 2 },
            ..RoundtripOptions::default()
        };
        assert!(matches!(
            roundtrip_bytes(&bytes, &deep),
            Err(Error::TooDeep { max: 2, .. })
        ));
    }
}
