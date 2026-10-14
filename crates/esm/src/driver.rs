//! Streaming drivers: roundtrip, visit, rewrite.
//!
//! Each reads a plugin one top-level entry at a time (the `TES4` record, then each
//! top-level group) and parses it; only one top-level entry is in memory at once, plus its
//! parsed tree.
//!
//! - [`roundtrip_file`] writes each entry back and compares the output with the bytes
//!   read as it is produced (never buffered). The first mismatch is reported as an
//!   absolute offset and a group/record/subrecord path.
//! - [`visit_file`] hands each entry to a closure.
//! - [`rewrite_file`] lets a closure edit each entry, then streams it to a sink;
//!   [`rewrite_file_to_path`] does the same into a new file and patches its `HEDR` count.
//!   A closure cannot add a top-level entry; [`crate::build::upsert_file_to_path`] adds
//!   records (creating missing top-level groups in order) while streaming the rest.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::model::{Body, Counts, Entry, Group, Record, Subrecord, HEADER_LEN};
use crate::{DecodeLimits, Error, ParseOptions, Result, Signature, WriteOptions};

/// Roundtrip settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RoundtripOptions {
    pub parse: ParseOptions,
    /// When set (`roundtrip --decode`, M3.A2), every compressed record is also checked
    /// strictly: its zlib stream must inflate, end with its end marker and checksum at the
    /// end of the body, and produce exactly the declared size; its subrecords must parse
    /// (they are counted) and serialize back to the decompressed bytes. A record that fails
    /// is a [`Mismatch`] at the fault's offset, with a path naming the record and the
    /// fault; a decode limit it exceeds is an error. The original compressed bytes are
    /// still what is written.
    pub decode: Option<DecodeLimits>,
    pub write: WriteOptions,
    /// When set, every record whose type has a schema definition is decoded through it,
    /// encoded again and compared, subrecord by subrecord
    /// ([`crate::bridge::check_subrecords`], M2.A3). Compressed records are checked only
    /// when `decode` is set too. Results are in [`Stats`] (`typed_*`).
    pub typed: bool,
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
    /// With [`RoundtripOptions::typed`]: records checked through their definition.
    pub typed_records: u64,
    /// With [`RoundtripOptions::typed`]: compressed records with a definition that were
    /// not checked because they were not decoded.
    pub typed_unchecked: u64,
    /// With [`RoundtripOptions::typed`]: subrecords the definitions placed in members.
    pub typed_subrecords: u64,
    /// With [`RoundtripOptions::typed`]: checked records with at least one conflict.
    pub typed_failed_records: u64,
    /// With [`RoundtripOptions::typed`]: conflicts found, in total.
    pub typed_conflicts: u64,
    /// The first [`MAX_LISTED_CONFLICTS`] conflicts, in file order.
    pub typed_conflict_list: Vec<RecordConflict>,
}

/// How many typed-decode conflicts [`Stats::typed_conflict_list`] keeps.
pub const MAX_LISTED_CONFLICTS: usize = 100;

/// A typed-decode conflict and the record it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordConflict {
    pub signature: Signature,
    pub form_id: u32,
    /// Absolute offset of the record.
    pub record_offset: u64,
    pub conflict: crate::bridge::TypedConflict,
}

/// Where the output first differs from the input, or (with [`RoundtripOptions::decode`])
/// the first compressed record body that does not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// Absolute offset of the first differing byte; for a body that does not decode, where
    /// inflating stopped (or the body's start, see the path).
    pub offset: u64,
    /// Input byte there (`None` past the end of the input entry, and for a body that does
    /// not decode).
    pub expected: Option<u8>,
    /// Output byte there (`None` when the output ended early, and for a body that does
    /// not decode).
    pub actual: Option<u8>,
    /// Path of the innermost node containing the offset, e.g.
    /// `GRUP WRLD (type 0) @123 > WRLD 0000003C @147 > FULL @171`. For a body that does
    /// not decode it ends with the fault:
    /// `... > NPC_ 0000697A @9000 > compressed body: invalid zlib stream: ...`.
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
    let mut localized = false;
    while let Some((offset, buf)) = entries.next_entry()? {
        let total = buf.len() as u64;
        let entry = Entry::parse_one(&buf, offset, 1, &options.parse)?;
        stats.top_level_entries = stats.top_level_entries.saturating_add(1);
        stats.largest_top_level_entry = stats.largest_top_level_entry.max(total);
        if let (0, Entry::Record(r)) = (offset, &entry) {
            localized = crate::tes4::Tes4::new(r).is_localized();
        }
        let count = Count {
            decode: options.decode.as_ref(),
            typed: options.typed.then_some(localized),
        };
        let mut fault = None;
        count_entry(&entry, &mut stats, &count, &mut fault)?;

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
        let byte_mismatch = diff.map(|diff| {
            let at = offset.saturating_add(diff.at as u64);
            Mismatch {
                offset: at,
                expected: buf.get(diff.at).copied(),
                actual: diff.actual,
                path: locate(&entry, at),
            }
        });
        let decode_mismatch =
            fault.map(|(record_offset, f)| fault_mismatch(&entry, record_offset, f));
        // Report whichever comes first in the file.
        let mismatch = match (byte_mismatch, decode_mismatch) {
            (Some(a), Some(b)) => Some(if b.offset < a.offset { b } else { a }),
            (a, b) => a.or(b),
        };
        if let Some(mismatch) = mismatch {
            return Ok(RoundtripReport {
                input_len: len,
                output_len: offset.saturating_add(written),
                mismatch: Some(mismatch),
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

/// Settings for [`rewrite_file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RewriteOptions {
    pub parse: ParseOptions,
    pub write: WriteOptions,
}

/// Where and what to patch when a streaming rewrite added or removed records or groups:
/// the `HEDR` record count was already written (it sits in the first entry), so the
/// caller writes `count` at output offset `offset` (see [`HedrFix::apply`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedrFix {
    /// Output offset of the `u32` record count inside `HEDR`.
    pub offset: u64,
    /// The count by the majority rule ([`crate::tes4::majority_record_count`]).
    pub count: u32,
}

impl HedrFix {
    /// Write the count into a seekable output (for example the file `rewrite_file` wrote).
    /// Leaves the position just after the count.
    pub fn apply<W: Write + Seek + ?Sized>(&self, w: &mut W) -> Result<()> {
        let io = |source| Error::Io {
            offset: self.offset,
            source,
        };
        w.seek(SeekFrom::Start(self.offset)).map_err(io)?;
        w.write_all(&self.count.to_le_bytes()).map_err(io)
    }
}

/// Result of [`rewrite_file`]. Check [`RewriteReport::hedr_fix`]: when it is set, the
/// output's `HEDR` count is stale until the fix is applied.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteReport {
    /// Input length in bytes.
    pub input_len: u64,
    /// Bytes written to the sink.
    pub output_len: u64,
    /// Top-level entries (`TES4` included) the edit left marked modified.
    pub modified_entries: u64,
    /// Records (excluding `TES4`) and groups before the edit.
    pub counts_read: Counts,
    /// Records (excluding `TES4`) and groups written.
    pub counts_written: Counts,
    /// Set when the counts differ and the `HEDR` record count written is not the majority
    /// count for what was written. [`rewrite_file_to_path`] has already applied it.
    pub hedr_fix: Option<HedrFix>,
    /// The counts differ but the header has no `HEDR` with room for a count, so the
    /// header was written as read and its count is stale (as
    /// [`crate::tes4::HedrCount::Unreadable`] for an in-memory write).
    pub hedr_unreadable: bool,
}

/// Stream the file at `path` to `sink`, letting `edit` change each top-level entry (the
/// `TES4` record first, then each top-level group) before it is written.
///
/// Memory: one top-level entry's bytes plus its parsed tree, as in [`roundtrip_file`].
/// Output is never buffered: a modified group's size is computed from its children
/// ([`Entry::encoded_len`]) before its header is written, and its children then stream
/// to `sink`.
///
/// The `HEDR` record count is written as read (the header precedes everything an edit
/// can add or remove); [`RewriteReport::hedr_fix`] says where to patch it afterwards.
pub fn rewrite_file(
    path: &Path,
    options: &RewriteOptions,
    sink: &mut dyn Write,
    edit: impl FnMut(&mut Entry<'_>) -> Result<()>,
) -> Result<RewriteReport> {
    let file = File::open(path).map_err(|source| Error::Io { offset: 0, source })?;
    let len = file
        .metadata()
        .map_err(|source| Error::Io { offset: 0, source })?
        .len();
    rewrite_reader(
        BufReader::with_capacity(1 << 20, file),
        len,
        options,
        sink,
        edit,
    )
}

/// As [`rewrite_file`], over `len` bytes from `input`.
pub fn rewrite_reader<R: Read>(
    input: R,
    len: u64,
    options: &RewriteOptions,
    sink: &mut dyn Write,
    mut edit: impl FnMut(&mut Entry<'_>) -> Result<()>,
) -> Result<RewriteReport> {
    stream_rewrite(
        input,
        len,
        options,
        sink,
        &mut edit,
        &mut |_| Ok(Vec::new()),
    )
}

/// [`rewrite_file`] into a new file at `out`, with the `HEDR` count patched when records
/// or groups were added or removed (the report's `hedr_fix` says what was applied).
///
/// The output is written to a temporary file next to `out` and renamed over it only when
/// the whole stream succeeded, so a failed rewrite leaves `out` as it was. `out` may not be
/// `path` itself.
pub fn rewrite_file_to_path(
    path: &Path,
    out: &Path,
    options: &RewriteOptions,
    mut edit: impl FnMut(&mut Entry<'_>) -> Result<()>,
) -> Result<RewriteReport> {
    to_path(path, out, |input, len, sink| {
        stream_rewrite(
            input,
            len,
            options,
            sink,
            &mut edit,
            &mut |_| Ok(Vec::new()),
        )
    })
}

/// Open `path`, stream it through `run` into a temporary file next to `out`, apply the
/// report's `HEDR` fix, and rename the result to `out`.
pub(crate) fn to_path(
    path: &Path,
    out: &Path,
    run: impl FnOnce(BufReader<File>, u64, &mut dyn Write) -> Result<RewriteReport>,
) -> Result<RewriteReport> {
    let io0 = |source| Error::Io { offset: 0, source };
    if out.exists()
        && std::fs::canonicalize(path).map_err(io0)? == std::fs::canonicalize(out).map_err(io0)?
    {
        return Err(io0(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the output path is the input file",
        )));
    }
    let file = File::open(path).map_err(io0)?;
    let len = file.metadata().map_err(io0)?.len();
    let mut tmp_name = out.as_os_str().to_owned();
    tmp_name.push(".kitcraft-tmp");
    let tmp = PathBuf::from(tmp_name);
    let result = (|| {
        let mut sink = BufWriter::with_capacity(1 << 20, File::create(&tmp).map_err(io0)?);
        let report = run(BufReader::with_capacity(1 << 20, file), len, &mut sink)?;
        if let Some(fix) = report.hedr_fix {
            fix.apply(&mut sink)?;
        }
        let file = sink.into_inner().map_err(|e| Error::Io {
            offset: report.output_len,
            source: e.into_error(),
        })?;
        file.sync_all().map_err(|source| Error::Io {
            offset: report.output_len,
            source,
        })?;
        Ok(report)
    })();
    match result {
        Ok(report) => {
            std::fs::rename(&tmp, out).map_err(io0)?;
            Ok(report)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// The streaming loop behind [`rewrite_reader`] and the build module's upsert: `edit`
/// changes each top-level entry; `insert` is called before each top-level entry after
/// `TES4` (with that entry, after its edit) and once at the end (with `None`), and the
/// entries it returns are written first.
pub(crate) fn stream_rewrite<R: Read>(
    mut input: R,
    len: u64,
    options: &RewriteOptions,
    sink: &mut dyn Write,
    edit: &mut dyn FnMut(&mut Entry<'_>) -> Result<()>,
    insert: &mut dyn FnMut(Option<&Entry<'_>>) -> Result<Vec<Entry<'static>>>,
) -> Result<RewriteReport> {
    let mut entries = TopLevelReader::new(&mut input, len);
    let mut pos: u64 = 0;
    let mut modified_entries: u64 = 0;
    let mut counts_read = Counts::default();
    let mut counts_written = Counts::default();
    let mut hedr: Option<(u64, u32)> = None;
    let write_new = |new: Vec<Entry<'static>>,
                     pos: &mut u64,
                     counts_written: &mut Counts,
                     modified_entries: &mut u64,
                     sink: &mut dyn Write|
     -> Result<()> {
        for entry in new {
            counts_written.add_entry(&entry);
            *modified_entries = modified_entries.saturating_add(1);
            let written = entry.write_to(sink, *pos, &options.write)?;
            *pos = pos.saturating_add(written);
        }
        Ok(())
    };
    while let Some((offset, buf)) = entries.next_entry()? {
        let mut entry = Entry::parse_one(&buf, offset, 1, &options.parse)?;
        let is_header = offset == 0;
        if !is_header {
            counts_read.add_entry(&entry);
        }
        edit(&mut entry)?;
        if is_header {
            if let Entry::Record(r) = &entry {
                hedr = hedr_count_at(r).map(|(at, count)| (pos.saturating_add(at), count));
            }
        } else {
            let new = insert(Some(&entry))?;
            write_new(
                new,
                &mut pos,
                &mut counts_written,
                &mut modified_entries,
                sink,
            )?;
            counts_written.add_entry(&entry);
        }
        if entry.is_modified() {
            modified_entries = modified_entries.saturating_add(1);
        }
        let written = entry.write_to(sink, pos, &options.write)?;
        pos = pos.saturating_add(written);
    }
    let new = insert(None)?;
    write_new(
        new,
        &mut pos,
        &mut counts_written,
        &mut modified_entries,
        sink,
    )?;
    let changed = counts_read != counts_written;
    let majority = crate::tes4::majority_record_count(counts_written);
    let (hedr_fix, hedr_unreadable) = match (changed, hedr, majority) {
        (false, _, _) => (None, false),
        (true, Some((offset, current)), Some(count)) => (
            (count != current).then_some(HedrFix { offset, count }),
            false,
        ),
        (true, _, _) => (None, true),
    };
    Ok(RewriteReport {
        input_len: len,
        output_len: pos,
        modified_entries,
        counts_read,
        counts_written,
        hedr_fix,
        hedr_unreadable,
    })
}

/// Offset of the `HEDR` record count inside `r` as written, and its value.
fn hedr_count_at(r: &Record<'_>) -> Option<(u64, u32)> {
    let hedr = crate::tes4::Tes4::new(r).hedr()?.ok()?;
    let mut at = HEADER_LEN as u64;
    for s in r.subrecords()? {
        if s.signature() == Signature::new(b"HEDR") {
            let header = s.encoded_len().saturating_sub(s.data().len() as u64);
            return Some((
                at.saturating_add(header).saturating_add(4),
                hedr.record_count,
            ));
        }
        at = at.saturating_add(s.encoded_len());
    }
    None
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
        if offset >= self.len && offset > 0 {
            return Ok(None);
        }
        // An empty input is not a plugin: it has no `TES4` header.
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

/// What [`count_entry`] does besides counting.
struct Count<'l> {
    decode: Option<&'l DecodeLimits>,
    /// `Some(localized)` to check typed records with that plugin flag.
    typed: Option<bool>,
}

/// The first compressed body in an entry that failed the strict decode: the record's
/// offset and the fault.
type FirstFault = Option<(u64, crate::compress::BodyFault)>;

fn count_entry(
    entry: &Entry<'_>,
    stats: &mut Stats,
    count: &Count<'_>,
    fault: &mut FirstFault,
) -> Result<()> {
    match entry {
        Entry::Group(g) => {
            stats.groups = stats.groups.saturating_add(1);
            for child in g.children() {
                count_entry(child, stats, count, fault)?;
            }
        }
        Entry::Record(r) => count_record(r, stats, count, fault)?,
    }
    Ok(())
}

/// The mismatch for a compressed body that failed the strict decode.
fn fault_mismatch(
    entry: &Entry<'_>,
    record_offset: u64,
    f: crate::compress::BodyFault,
) -> Mismatch {
    let mut path = locate(entry, record_offset);
    let _ = write!(path, " > compressed body: {}", f.reason);
    if let Some(at) = f.inflated_offset {
        let _ = write!(path, " (decompressed offset {at})");
    }
    Mismatch {
        offset: f.offset,
        expected: None,
        actual: None,
        path,
    }
}

/// Check one record's subrecords against its definition and add the result to `stats`.
fn check_typed(r: &Record<'_>, subs: &[Subrecord<'_>], localized: bool, stats: &mut Stats) {
    let ctx = crate::bridge::Ctx::for_record(r, localized);
    let Ok(check) = crate::bridge::check_subrecords(r.header.signature, subs, ctx) else {
        return;
    };
    stats.typed_records = stats.typed_records.saturating_add(1);
    stats.typed_subrecords = stats.typed_subrecords.saturating_add(check.placed as u64);
    if check.is_clean() {
        return;
    }
    stats.typed_failed_records = stats.typed_failed_records.saturating_add(1);
    stats.typed_conflicts = stats
        .typed_conflicts
        .saturating_add(check.conflicts.len() as u64);
    for conflict in check.conflicts {
        if stats.typed_conflict_list.len() >= MAX_LISTED_CONFLICTS {
            break;
        }
        stats.typed_conflict_list.push(RecordConflict {
            signature: r.header.signature,
            form_id: r.header.form_id,
            record_offset: r.offset().unwrap_or(0),
            conflict,
        });
    }
}

fn count_subrecords(subs: &[Subrecord<'_>], stats: &mut Stats) {
    stats.subrecords = stats.subrecords.saturating_add(subs.len() as u64);
    let oversize = subs.iter().filter(|s| s.has_oversize_prefix()).count() as u64;
    stats.oversize_subrecords = stats.oversize_subrecords.saturating_add(oversize);
}

fn count_record(
    r: &Record<'_>,
    stats: &mut Stats,
    count: &Count<'_>,
    fault: &mut FirstFault,
) -> Result<()> {
    stats.records = stats.records.saturating_add(1);
    let n = stats.record_types.entry(r.header.signature).or_insert(0);
    *n = n.saturating_add(1);
    let typed = count
        .typed
        .filter(|_| crate::bridge::has_definition(r.header.signature));
    match r.body() {
        Body::Raw(subs) => {
            count_subrecords(subs, stats);
            if let Some(localized) = typed {
                check_typed(r, subs, localized, stats);
            }
        }
        Body::Compressed { original, .. } => {
            stats.compressed_records = stats.compressed_records.saturating_add(1);
            let at = r.offset().unwrap_or(0);
            let decoded = match count.decode {
                Some(limits) => match crate::compress::check_body(original, at, limits)? {
                    crate::compress::BodyCheck::Decoded(subs) => Some(subs),
                    crate::compress::BodyCheck::Fault(f) => {
                        if fault.is_none() {
                            *fault = Some((at, f));
                        }
                        None
                    }
                },
                None => None,
            };
            match (decoded, typed) {
                (Some(subs), _) => {
                    stats.decoded_records = stats.decoded_records.saturating_add(1);
                    let size = subs
                        .iter()
                        .fold(0u64, |acc, s| acc.saturating_add(s.encoded_len()));
                    stats.largest_decompressed = stats.largest_decompressed.max(size);
                    count_subrecords(&subs, stats);
                    if let Some(localized) = typed {
                        check_typed(r, &subs, localized, stats);
                    }
                }
                (None, Some(_)) => {
                    stats.typed_unchecked = stats.typed_unchecked.saturating_add(1);
                }
                (None, None) => {}
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
    use super::{locate, rewrite_reader, roundtrip_bytes, RewriteOptions, RoundtripOptions};
    use crate::fixture::{self, group, sub};
    use crate::{Entry, Error, ParseOptions, Plugin};

    /// Grow the NAME of the first REFR under CELL > cell children > persistent.
    fn edit_refr(entry: &mut Entry<'_>) {
        let Entry::Group(cells) = entry else { return };
        if cells.header.label != *b"CELL" {
            return;
        }
        let Entry::Group(children) = &mut cells.children_mut()[1] else {
            panic!()
        };
        let Entry::Group(persistent) = &mut children.children_mut()[0] else {
            panic!()
        };
        let Entry::Record(refr) = &mut persistent.children_mut()[0] else {
            panic!()
        };
        refr.subrecords_mut().unwrap()[0].set_data(vec![7u8; 9]);
    }

    /// Drop the second GMST record.
    fn drop_gmst(entry: &mut Entry<'_>) {
        if let Entry::Group(g) = entry {
            if g.header.label == *b"GMST" {
                g.children_mut().pop();
            }
        }
    }

    #[test]
    fn rewrite_streams_an_edit_like_the_in_memory_writer() {
        let bytes = fixture::sample_plugin();
        let mut out = Vec::new();
        let report = rewrite_reader(
            &bytes[..],
            bytes.len() as u64,
            &RewriteOptions::default(),
            &mut out,
            |e| {
                edit_refr(e);
                Ok(())
            },
        )
        .unwrap();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        plugin.entries_mut().iter_mut().for_each(edit_refr);
        assert_eq!(out, plugin.to_bytes().unwrap());
        assert_eq!(out.len(), bytes.len() + 5);
        assert_eq!(report.output_len, out.len() as u64);
        assert_eq!(report.input_len, bytes.len() as u64);
        assert_eq!(report.modified_entries, 1);
        assert_eq!(report.counts_read, report.counts_written);
        assert_eq!(report.hedr_fix, None);
        assert!(!report.hedr_unreadable);
    }

    #[test]
    fn rewrite_that_removes_a_record_says_how_to_fix_hedr() {
        let bytes = fixture::sample_plugin();
        let mut out = std::io::Cursor::new(Vec::new());
        let report = rewrite_reader(
            &bytes[..],
            bytes.len() as u64,
            &RewriteOptions::default(),
            &mut out,
            |e| {
                drop_gmst(e);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            report.counts_read.records,
            report.counts_written.records + 1
        );
        let fix = report.hedr_fix.unwrap();
        // TES4, CELL, 3 REFR, NAVM and one GMST remain.
        assert_eq!(fix.count, 7);
        // The fixture TES4 starts with HEDR: record header, subrecord header, f32 version.
        assert_eq!(fix.offset, 24 + 6 + 4);
        fix.apply(&mut out).unwrap();
        let out = out.into_inner();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        plugin.entries_mut().iter_mut().for_each(drop_gmst);
        assert_eq!(out, plugin.to_bytes().unwrap());
        let reparsed = Plugin::parse(&out).unwrap();
        assert_eq!(reparsed.tes4().hedr().unwrap().unwrap().record_count, 7);
    }

    /// Review round 3, finding 3: the caller had to apply `hedr_fix` by hand.
    #[test]
    fn rewrite_to_path_patches_hedr_and_refuses_its_input() {
        let dir = std::env::temp_dir().join(format!("kitcraft-esm-rewrite-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.esm");
        let output = dir.join("out.esm");
        let bytes = fixture::sample_plugin();
        std::fs::write(&input, &bytes).unwrap();
        let report =
            super::rewrite_file_to_path(&input, &output, &RewriteOptions::default(), |e| {
                drop_gmst(e);
                Ok(())
            })
            .unwrap();
        assert_eq!(report.hedr_fix.map(|f| f.count), Some(7));
        let out = std::fs::read(&output).unwrap();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        plugin.entries_mut().iter_mut().for_each(drop_gmst);
        assert_eq!(out, plugin.to_bytes().unwrap());
        assert!(!dir.join("out.esm.kitcraft-tmp").exists());

        // The input is never its own output.
        let same =
            super::rewrite_file_to_path(&input, &input, &RewriteOptions::default(), |_| Ok(()));
        assert!(matches!(same, Err(Error::Io { .. })));
        assert_eq!(std::fs::read(&input).unwrap(), bytes);

        // A failing edit leaves an existing output untouched and no temporary file.
        let failed =
            super::rewrite_file_to_path(&input, &output, &RewriteOptions::default(), |e| match e {
                Entry::Group(_) => Err(Error::DanglingOversize { offset: 1 }),
                Entry::Record(_) => Ok(()),
            });
        assert!(failed.is_err());
        assert_eq!(std::fs::read(&output).unwrap(), out);
        assert!(!dir.join("out.esm.kitcraft-tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rewrite_errors_from_the_edit_stop_the_stream() {
        let bytes = fixture::sample_plugin();
        let mut out = Vec::new();
        let result = rewrite_reader(
            &bytes[..],
            bytes.len() as u64,
            &RewriteOptions::default(),
            &mut out,
            |e| match e {
                Entry::Group(_) => Err(Error::DanglingOversize { offset: 99 }),
                Entry::Record(_) => Ok(()),
            },
        );
        assert!(matches!(
            result,
            Err(Error::DanglingOversize { offset: 99 })
        ));
        // Only the TES4 record was written.
        assert_eq!(out.len(), fixture::plugin(&[]).len());
    }

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
    fn the_typed_option_checks_records_with_a_definition() {
        // GMST, KYWD and TES4 have definitions; CELL, REFR and NAVM do not. In `bad`
        // below, the TES4 HEDR (a typed member) is too short to decode.
        let kywd = fixture::compressed_record_stored(b"KYWD", 0x900, &sub(b"EDID", b"kw\0"));
        let mut bytes = fixture::sample_plugin();
        bytes.extend(group(b"KYWD", 0, &kywd));
        let options = RoundtripOptions {
            typed: true,
            ..RoundtripOptions::with_decode()
        };
        let report = roundtrip_bytes(&bytes, &options).unwrap();
        assert!(report.identical());
        let s = &report.stats;
        assert_eq!(s.typed_records, 4);
        assert_eq!(s.typed_unchecked, 0);
        assert_eq!(s.typed_failed_records, 0);
        assert!(s.typed_conflict_list.is_empty());
        // TES4 7 + GMST 2 + 2 + KYWD 1.
        assert_eq!(s.typed_subrecords, 12);

        // Without decoding, the compressed KYWD is not checked.
        let options = RoundtripOptions {
            typed: true,
            ..RoundtripOptions::default()
        };
        let s = roundtrip_bytes(&bytes, &options).unwrap().stats;
        assert_eq!((s.typed_records, s.typed_unchecked), (3, 1));

        // A short HEDR: one conflict, listed with its record and offset.
        let tes4 = crate::fixture::record(b"TES4", 0x81, 0, &sub(b"HEDR", &[0u8; 8]));
        let bad = [tes4, group(b"KYWD", 0, &kywd)].concat();
        let s = roundtrip_bytes(&bad, &options).unwrap().stats;
        assert_eq!(s.typed_failed_records, 1);
        assert_eq!(s.typed_conflicts, 1);
        let c = s.typed_conflict_list[0];
        assert_eq!(
            (c.signature, c.form_id, c.record_offset),
            (crate::Signature::TES4, 0, 0)
        );
        assert_eq!(c.conflict.offset, Some(24));
        assert_eq!(c.conflict.kind, crate::bridge::ConflictKind::NotDecoded);
    }

    /// A plugin whose last top-level group holds one compressed LAND record (FormID 0x51)
    /// with the given body (`u32` size + stream), and the record's offset.
    fn with_compressed_body(body: &[u8]) -> (Vec<u8>, u64) {
        let mut bytes = fixture::sample_plugin();
        let group_at = bytes.len() as u64;
        let rec = fixture::record(b"LAND", 0x0004_0000, 0x51, body);
        bytes.extend(group(b"LAND", 0, &rec));
        (bytes, group_at + 24)
    }

    fn land_fields() -> Vec<u8> {
        [sub(b"EDID", b"corrupt\0"), sub(b"VHGT", &[5u8; 300])].concat()
    }

    /// Roundtrip with and without `decode`: identical without (the bytes are re-emitted
    /// as read), a mismatch with. Returns that mismatch.
    fn decode_mismatch(bytes: &[u8]) -> super::Mismatch {
        let plain = roundtrip_bytes(bytes, &RoundtripOptions::default()).unwrap();
        assert!(plain.identical(), "{plain:?}");
        let report = roundtrip_bytes(bytes, &RoundtripOptions::with_decode()).unwrap();
        assert!(!report.identical());
        assert_eq!(report.output_len, bytes.len() as u64);
        let m = report.mismatch.unwrap();
        assert_eq!((m.expected, m.actual), (None, None));
        m
    }

    /// M3.A2: a corrupt stream whose declared size is right is a mismatch under `decode`,
    /// with the offset where inflating stopped and the record's path.
    #[test]
    fn decode_reports_a_corrupt_well_sized_stream() {
        let fields = land_fields();
        let mut body = crate::compress::deflate(&fields, 9, 0).unwrap();
        // Corrupt the Adler-32 trailer: every size still agrees.
        let last = body.len() - 1;
        body[last] ^= 0x01;
        let (bytes, at) = with_compressed_body(&body);
        let m = decode_mismatch(&bytes);
        let prefix = format!(
            "GRUP LAND (type 0) @{} > LAND 00000051 @{at} > compressed body: invalid zlib stream",
            at - 24
        );
        assert!(m.path.starts_with(&prefix), "{}", m.path);
        let stream_at = at + 24 + 4;
        let end = at + 24 + body.len() as u64;
        assert!((stream_at..=end).contains(&m.offset), "{}", m.offset);

        // A corrupt deflate block in the middle of the stream.
        let mut body = crate::compress::deflate(&fields, 9, 0).unwrap();
        body[8] ^= 0xFF;
        let (bytes, at) = with_compressed_body(&body);
        let m = decode_mismatch(&bytes);
        assert!(m.path.contains("LAND 00000051 @"), "{}", m.path);
        assert!(m.offset >= at + 28, "{}", m.offset);
    }

    /// Each kind of body defect `decode` finds, with where it points.
    #[test]
    fn decode_reports_each_body_fault() {
        let fields = land_fields();
        let good = crate::compress::deflate(&fields, 9, 0).unwrap();
        let declared = fields.len() as u32;

        // Declared size one byte short and one byte long.
        for wrong in [declared - 1, declared + 1] {
            let mut body = good.clone();
            body[..4].copy_from_slice(&wrong.to_le_bytes());
            let (bytes, at) = with_compressed_body(&body);
            let m = decode_mismatch(&bytes);
            assert!(
                m.path.contains("compressed body: the zlib stream inflates"),
                "{}",
                m.path
            );
            assert!((at + 24..=at + 24 + body.len() as u64).contains(&m.offset));
        }

        // Bytes after the end of the zlib stream.
        let mut body = good.clone();
        body.extend_from_slice(b"junk");
        let (bytes, at) = with_compressed_body(&body);
        let m = decode_mismatch(&bytes);
        assert!(
            m.path
                .ends_with("compressed body: 4 bytes follow the end of the zlib stream"),
            "{}",
            m.path
        );
        assert_eq!(m.offset, at + 24 + good.len() as u64);

        // A stream cut before its end marker (the record size shrinks with it).
        let (bytes, at) = with_compressed_body(&good[..good.len() - 6]);
        let m = decode_mismatch(&bytes);
        assert!(m.path.contains("compressed body: "), "{}", m.path);
        assert!(m.offset >= at + 28);

        // A body shorter than its size prefix.
        let (bytes, at) = with_compressed_body(&[1, 0]);
        let m = decode_mismatch(&bytes);
        assert!(
            m.path.contains("shorter than its 4-byte decompressed size"),
            "{}",
            m.path
        );
        assert_eq!(m.offset, at + 24);

        // A valid stream whose decompressed data is not subrecords.
        let junk = b"EDID\x09\x00ab";
        let mut body = (junk.len() as u32).to_le_bytes().to_vec();
        body.extend(fixture::zlib_stored(junk));
        let (bytes, at) = with_compressed_body(&body);
        let m = decode_mismatch(&bytes);
        assert!(
            m.path.contains("the decompressed data does not parse")
                && m.path.ends_with("(decompressed offset 6)"),
            "{}",
            m.path
        );
        assert_eq!(m.offset, at + 24);
    }

    /// Decode limits stay errors under `decode`: they are our caps, not defects.
    #[test]
    fn decode_limits_are_errors_not_mismatches() {
        let body = crate::compress::deflate(&land_fields(), 9, 0).unwrap();
        let (bytes, at) = with_compressed_body(&body);
        let options = RoundtripOptions {
            decode: Some(crate::DecodeLimits {
                max_decompressed: 10,
                ..crate::DecodeLimits::default()
            }),
            ..RoundtripOptions::default()
        };
        assert!(matches!(
            roundtrip_bytes(&bytes, &options),
            Err(Error::DecompressedTooLarge { offset, cap: 10, .. }) if offset == at
        ));
        let options = RoundtripOptions {
            decode: Some(crate::DecodeLimits {
                max_subrecords: 1,
                ..crate::DecodeLimits::default()
            }),
            ..RoundtripOptions::default()
        };
        assert!(matches!(
            roundtrip_bytes(&bytes, &options),
            Err(Error::TooManySubrecords { max: 1, .. })
        ));
        // Within the default limits it decodes and is identical.
        let report = roundtrip_bytes(&bytes, &RoundtripOptions::with_decode()).unwrap();
        assert!(report.identical(), "{report:?}");
        assert_eq!(report.stats.decoded_records, 1);
        let plain = roundtrip_bytes(&bytes, &RoundtripOptions::default()).unwrap();
        assert_eq!(report.stats.subrecords, plain.stats.subrecords + 2);
    }

    /// The first fault in file order is reported, and the roundtrip stops at its entry.
    #[test]
    fn decode_reports_the_first_fault() {
        let fields = land_fields();
        let mut bad = crate::compress::deflate(&fields, 9, 0).unwrap();
        let last = bad.len() - 1;
        bad[last] ^= 0x10;
        let first = fixture::record(b"LAND", 0x0004_0000, 0x60, &bad);
        let second = fixture::record(b"LAND", 0x0004_0000, 0x61, &[0, 0]);
        let mut bytes = fixture::sample_plugin();
        let at = bytes.len() as u64 + 24;
        bytes.extend(group(b"LAND", 0, &[first, second].concat()));
        let m = decode_mismatch(&bytes);
        assert!(
            m.path.contains(&format!("LAND 00000060 @{at}")),
            "{}",
            m.path
        );
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
        // Review round 3, finding 2: an empty input is an error, not an identical rewrite.
        assert!(matches!(
            roundtrip_bytes(&[], &RoundtripOptions::default()),
            Err(Error::UnexpectedEof {
                offset: 0,
                available: 0,
                ..
            })
        ));
        let mut out = Vec::new();
        assert!(matches!(
            rewrite_reader(&[][..], 0, &RewriteOptions::default(), &mut out, |_| Ok(())),
            Err(Error::UnexpectedEof { offset: 0, .. })
        ));
        assert!(out.is_empty());
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
