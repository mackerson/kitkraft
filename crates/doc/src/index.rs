//! Per-plugin record index: where every record is, without keeping the tree.
//!
//! Built from lane A's `kitkraft_esm::index` (M2.A1), reduced to what the load order needs
//! and stored compactly (EditorIDs in one arena) so the cache (`cache.rs`) can hold it.
//!
//! EditorIDs: the first `EDID` subrecord of a record without its trailing NUL, decoded as
//! UTF-8 with invalid bytes replaced (lane A reads compressed records under
//! `DecodeLimits`; a stream that fails to inflate is an error for the whole plugin, with its
//! offset).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use kitkraft_esm::index::{index_file, IndexOptions};
use kitkraft_esm::tes4::Tes4;
use kitkraft_esm::{Entry, ParseOptions, Reader, Record, Signature};

use crate::error::{DocError, Result};
use crate::flags::PluginFlags;

/// Marks "no EditorID" in [`RecordLoc::edid_start`].
const NO_EDID: u32 = u32::MAX;

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
    /// Compressed records whose EditorID could not be read (always 0 since the esm index
    /// fails the plugin instead; kept for the cache layout).
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
    /// Index every record of `path` through `kitkraft_esm::index` (lane A's M2.A1 index)
    /// and keep what the load order needs.
    pub fn build(path: &Path) -> Result<PluginIndex> {
        let (size, mtime_ns) = file_stamp(path)?;
        let file_name = file_name_of(path);
        let esm = index_file(path, &IndexOptions::default())
            .map_err(|e| DocError::esm(file_name.clone(), e))?;
        let mut index = PluginIndex {
            file_name,
            path: path.to_path_buf(),
            size,
            mtime_ns,
            header: header_info(&esm.header),
            records: Vec::with_capacity(esm.records.len()),
            edids: String::new(),
            edid_errors: 0,
        };
        // `records` starts with TES4, which the load order keeps as `header`.
        for r in esm.records.iter().filter(|r| r.offset != 0) {
            let edid = r.edid_lossy();
            let (edid_start, edid_len) = match edid {
                Some(text) => match (u32::try_from(index.edids.len()), u32::try_from(text.len())) {
                    (Ok(start), Ok(len)) => {
                        index.edids.push_str(&text);
                        (start, len)
                    }
                    _ => (NO_EDID, 0),
                },
                None => (NO_EDID, 0),
            };
            index.records.push(RecordLoc {
                signature: r.signature.0,
                form_id: r.form_id,
                flags: r.flags,
                offset: r.offset,
                data_size: r.data_size,
                edid_start,
                edid_len,
            });
        }
        Ok(index)
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
            kitkraft_esm::Error::NotAPlugin {
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
            kitkraft_esm::Error::UnexpectedEof {
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
    fn broken_compressed_body_is_a_typed_error() {
        let mut bad = testdata::rec(b"WEAP", 0x800, &[]);
        bad[8..12].copy_from_slice(&0x0004_0000u32.to_le_bytes());
        let body = [5u32.to_le_bytes().to_vec(), vec![1, 2, 3, 4, 5]].concat();
        bad[4..8].copy_from_slice(&(body.len() as u32).to_le_bytes());
        bad.truncate(24);
        bad.extend(body);
        let bytes = testdata::plugin_with(0x1, &[], &[bad]);
        let path = testdata::temp_file("index-badz.esm", &bytes);
        let err = PluginIndex::build(&path).unwrap_err();
        assert!(matches!(err, DocError::Esm { .. }), "{err}");
        assert!(err.to_string().contains("index-badz.esm"), "{err}");
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
