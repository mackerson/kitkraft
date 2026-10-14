//! Per-plugin record index: where every record is, without keeping the tree.
//!
//! A stand-in for lane A's M2.A1 index (same idea, built here over
//! [`kitcraft_esm::driver::visit_file`]); swap to the esm index once it lands on `main`.
//!
//! EditorIDs: the first `EDID` subrecord of a record, up to its NUL, decoded as UTF-8 with
//! invalid bytes replaced. For a compressed record only the first 4 KiB of the inflated
//! body is examined (EDID leads every record that has one in the corpus); a stream that fails
//! to inflate leaves the record without an EditorID and is counted in
//! [`PluginIndex::edid_errors`].

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use kitcraft_esm::driver::visit_file;
use kitcraft_esm::tes4::Tes4;
use kitcraft_esm::{Body, Entry, ParseOptions, Reader, Record, Signature};

use crate::error::{DocError, Result};
use crate::flags::PluginFlags;

/// Bytes of a compressed record's inflated body scanned for its EDID.
const EDID_SCAN: u64 = 4096;

/// Marks "no EditorID" in [`RecordLoc::edid_start`].
const NO_EDID: u32 = u32::MAX;

const EDID: Signature = Signature::new(b"EDID");

/// Where one record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordLoc {
    /// Record type.
    pub signature: [u8; 4],
    /// FormID as stored in this file (raw; resolve through the load order).
    pub form_id: u32,
    /// Record flags.
    pub flags: u32,
    /// Absolute offset of the record header.
    pub offset: u64,
    /// Data size field (excluding the 24-byte header).
    pub data_size: u32,
    pub(crate) edid_start: u32,
    pub(crate) edid_len: u32,
}

impl RecordLoc {
    /// The signature as text.
    pub fn signature(&self) -> Signature {
        Signature(self.signature)
    }
}

/// `HEDR` values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HedrInfo {
    /// Version.
    pub version: f32,
    /// Record count as stored.
    pub record_count: u32,
    /// Next object ID as stored.
    pub next_object_id: u32,
}

/// What the `TES4` record says.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderInfo {
    /// Raw flags.
    pub flags: PluginFlags,
    /// TES4 form version.
    pub form_version: u16,
    /// `HEDR`, when present and readable.
    pub hedr: Option<HedrInfo>,
    /// Master file names in order, as stored.
    pub masters: Vec<String>,
}

/// Every record of one plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginIndex {
    /// File name.
    pub file_name: String,
    /// Path the index was built from.
    pub path: PathBuf,
    /// File size when indexed.
    pub size: u64,
    /// Modification time when indexed (ns since the Unix epoch, 0 when unknown).
    pub mtime_ns: u64,
    /// Header facts.
    pub header: HeaderInfo,
    /// Records in file order, `TES4` excluded.
    pub records: Vec<RecordLoc>,
    pub(crate) edids: String,
    /// Compressed records whose EditorID could not be read.
    pub edid_errors: u64,
}

/// Size and modification time of `path`.
pub fn file_stamp(path: &Path) -> Result<(u64, u64)> {
    let meta = std::fs::metadata(path).map_err(|e| DocError::io(path, e))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_nanos()).ok())
        .unwrap_or(0);
    Ok((meta.len(), mtime))
}

/// The file name of `path` as text.
pub fn file_name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl PluginIndex {
    /// Stream `path` and index every record.
    pub fn build(path: &Path) -> Result<PluginIndex> {
        let (size, mtime_ns) = file_stamp(path)?;
        let file_name = file_name_of(path);
        let mut index = PluginIndex {
            file_name: file_name.clone(),
            path: path.to_path_buf(),
            size,
            mtime_ns,
            header: HeaderInfo {
                flags: PluginFlags(0),
                form_version: 0,
                hedr: None,
                masters: Vec::new(),
            },
            records: Vec::new(),
            edids: String::new(),
            edid_errors: 0,
        };
        let mut first = true;
        visit_file(path, &ParseOptions::default(), |entry| {
            if first {
                first = false;
                if let Entry::Record(r) = entry {
                    index.header = header_info(r);
                }
                return Ok(());
            }
            index.add_entry(entry);
            Ok(())
        })
        .map_err(|e| DocError::esm(file_name, e))?;
        Ok(index)
    }

    fn add_entry(&mut self, entry: &Entry<'_>) {
        // Iterative walk: nesting is already bounded by the parser.
        let mut stack: Vec<&Entry<'_>> = vec![entry];
        while let Some(e) = stack.pop() {
            match e {
                Entry::Group(g) => stack.extend(g.children().iter().rev()),
                Entry::Record(r) => self.add_record(r),
            }
        }
    }

    fn add_record(&mut self, r: &Record<'_>) {
        let edid = match edid_of(r) {
            Ok(e) => e,
            Err(()) => {
                self.edid_errors = self.edid_errors.saturating_add(1);
                None
            }
        };
        let (edid_start, edid_len) = match edid {
            Some(text) => match (u32::try_from(self.edids.len()), u32::try_from(text.len())) {
                (Ok(start), Ok(len)) => {
                    self.edids.push_str(&text);
                    (start, len)
                }
                _ => (NO_EDID, 0),
            },
            None => (NO_EDID, 0),
        };
        self.records.push(RecordLoc {
            signature: r.header.signature.0,
            form_id: r.header.form_id,
            flags: r.header.flags,
            offset: r.offset().unwrap_or(0),
            data_size: r.data_size_as_read().unwrap_or(0),
            edid_start,
            edid_len,
        });
    }

    /// The EditorID of `loc` (a record of this index).
    pub fn edid(&self, loc: &RecordLoc) -> Option<&str> {
        if loc.edid_start == NO_EDID {
            return None;
        }
        let start = usize::try_from(loc.edid_start).ok()?;
        let end = start.checked_add(usize::try_from(loc.edid_len).ok()?)?;
        self.edids.get(start..end)
    }

    /// Records with an EditorID.
    pub fn edid_count(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r.edid_start != NO_EDID)
            .count()
    }

    pub(crate) fn from_parts(
        file_name: String,
        path: PathBuf,
        stamp: (u64, u64),
        header: HeaderInfo,
        records: Vec<RecordLoc>,
        edids: String,
        edid_errors: u64,
    ) -> PluginIndex {
        PluginIndex {
            file_name,
            path,
            size: stamp.0,
            mtime_ns: stamp.1,
            header,
            records,
            edids,
            edid_errors,
        }
    }

    pub(crate) fn edid_arena(&self) -> &str {
        &self.edids
    }
}

/// Header facts of a `TES4` record.
pub fn header_info(r: &Record<'_>) -> HeaderInfo {
    let tes4 = Tes4::new(r);
    HeaderInfo {
        flags: PluginFlags(r.header.flags),
        form_version: r.header.form_version,
        hedr: tes4.hedr().and_then(|h| h.ok()).map(|h| HedrInfo {
            version: h.version,
            record_count: h.record_count,
            next_object_id: h.next_object_id,
        }),
        masters: tes4.masters().iter().map(|m| m.name_lossy()).collect(),
    }
}

/// Read only the `TES4` header of a plugin (masters and flags), without indexing it.
pub fn read_header(path: &Path) -> Result<HeaderInfo> {
    let file_name = file_name_of(path);
    let mut file = std::fs::File::open(path).map_err(|e| DocError::io(path, e))?;
    let mut head = [0u8; 24];
    file.read_exact(&mut head)
        .map_err(|e| DocError::io(path, e))?;
    let mut r = Reader::new(&head, 0);
    let sig = r.signature().map_err(|e| DocError::esm(&file_name, e))?;
    let size = r.u32().map_err(|e| DocError::esm(&file_name, e))?;
    if sig != Signature::TES4 {
        return Err(DocError::esm(
            &file_name,
            kitcraft_esm::Error::NotAPlugin {
                offset: 0,
                found: sig,
            },
        ));
    }
    let (file_len, _) = file_stamp(path)?;
    let total = u64::from(size).saturating_add(24);
    if total > file_len {
        return Err(DocError::esm(
            &file_name,
            kitcraft_esm::Error::UnexpectedEof {
                offset: 0,
                what: "record",
                needed: total,
                available: file_len,
            },
        ));
    }
    let mut buf = head.to_vec();
    (&mut file)
        .take(u64::from(size))
        .read_to_end(&mut buf)
        .map_err(|e| DocError::io(path, e))?;
    let entry = Entry::parse_one(&buf, 0, 1, &ParseOptions::default())
        .map_err(|e| DocError::esm(&file_name, e))?;
    match entry {
        Entry::Record(r) => Ok(header_info(&r)),
        Entry::Group(_) => Err(DocError::Invalid(format!(
            "{file_name}: first entry is not a record"
        ))),
    }
}

/// The record's EditorID, `Ok(None)` when it has none, `Err(())` when a compressed body could
/// not be inflated.
fn edid_of(r: &Record<'_>) -> std::result::Result<Option<String>, ()> {
    match r.body() {
        Body::Raw(subs) => Ok(subs
            .iter()
            .find(|s| s.signature() == EDID)
            .map(|s| zstring(s.data()))),
        Body::Compressed { original, .. } => {
            let stream = original.get(4..).ok_or(())?;
            let mut head = Vec::new();
            flate2::read::ZlibDecoder::new(stream)
                .take(EDID_SCAN)
                .read_to_end(&mut head)
                .map_err(|_| ())?;
            Ok(scan_edid(&head))
        }
    }
}

/// The first EDID among the whole subrecords at the start of `data`.
fn scan_edid(data: &[u8]) -> Option<String> {
    let mut r = Reader::new(data, 0);
    loop {
        let sig = r.signature().ok()?;
        let size = r.u16().ok()?;
        if sig == Signature::XXXX {
            return None;
        }
        let body = r.take(usize::from(size)).ok()?;
        if sig == EDID {
            return Some(zstring(body));
        }
    }
}

/// Bytes up to the first NUL, as lossy UTF-8.
pub fn zstring(data: &[u8]) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    String::from_utf8_lossy(data.get(..end).unwrap_or(data)).into_owned()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::testdata;

    #[test]
    fn index_lists_every_record_with_edids() {
        let bytes = testdata::plugin_with(
            0x81,
            &["Starfield.esm"],
            &[
                testdata::rec(b"WEAP", 0x0000_1234, &[("EDID", b"Rifle\0".to_vec())]),
                testdata::compressed(b"WEAP", 0x0100_0800, &[("EDID", b"Packed\0".to_vec())]),
                testdata::rec(b"GLOB", 0x0100_0801, &[("DATA", vec![1, 2])]),
            ],
        );
        let path = testdata::temp_file("index-basic.esm", &bytes);
        let index = PluginIndex::build(&path).unwrap();
        assert_eq!(index.header.masters, ["Starfield.esm"]);
        assert_eq!(index.header.flags.0, 0x81);
        assert_eq!(index.records.len(), 3);
        let edids: Vec<Option<&str>> = index.records.iter().map(|r| index.edid(r)).collect();
        assert_eq!(edids, [Some("Rifle"), Some("Packed"), None]);
        assert_eq!(index.records[1].form_id, 0x0100_0800);
        assert_eq!(&bytes[index.records[2].offset as usize..][..4], b"GLOB");
        assert_eq!(index.edid_errors, 0);
        assert_eq!(read_header(&path).unwrap(), index.header);
    }

    #[test]
    fn broken_compressed_body_counts_as_edid_error() {
        let mut bad = testdata::rec(b"WEAP", 0x800, &[]);
        bad[8..12].copy_from_slice(&0x0004_0000u32.to_le_bytes());
        let body = [5u32.to_le_bytes().to_vec(), vec![1, 2, 3, 4, 5]].concat();
        bad[4..8].copy_from_slice(&(body.len() as u32).to_le_bytes());
        bad.truncate(24);
        bad.extend(body);
        let bytes = testdata::plugin_with(0x1, &[], &[bad]);
        let path = testdata::temp_file("index-badz.esm", &bytes);
        let index = PluginIndex::build(&path).unwrap();
        assert_eq!(index.records.len(), 1);
        assert_eq!(index.edid_errors, 1);
    }

    #[test]
    fn truncated_plugin_is_a_typed_error_with_offset() {
        let bytes = testdata::plugin_with(
            0x1,
            &[],
            &[testdata::rec(b"WEAP", 1, &[("EDID", b"A\0".to_vec())])],
        );
        let path = testdata::temp_file("index-trunc.esm", &bytes[..bytes.len() - 3]);
        let err = PluginIndex::build(&path).unwrap_err();
        assert!(matches!(err, DocError::Esm { .. }), "{err}");
        assert!(err.to_string().contains("offset"), "{err}");
    }
}
