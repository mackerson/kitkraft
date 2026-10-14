//! Random access: read one record at an indexed offset (a stand-in for lane A's M2.A2).

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use kitcraft_esm::{DecodeLimits, Entry, ParseOptions, Reader, Record, Signature, Subrecord};

use crate::error::{DocError, Result};
use crate::index::{file_name_of, RecordLoc};
use crate::load_order::LoadOrder;

/// A record read back from its file, with its subrecords (decoded when compressed).
#[derive(Debug, Clone)]
pub struct LoadedRecord {
    /// File name.
    pub file: String,
    /// The record.
    pub record: Record<'static>,
    /// Subrecords; for a compressed record these come from the inflated body and carry no
    /// file offsets.
    pub subs: Vec<Subrecord<'static>>,
    /// Whether the body was compressed.
    pub compressed: bool,
}

impl LoadedRecord {
    /// `SIGN:FORMID` (raw FormID as stored).
    pub fn label(&self) -> String {
        format!(
            "{}:{:08X}",
            self.record.header.signature, self.record.header.form_id
        )
    }

    /// Absolute offset of the record header.
    pub fn offset(&self) -> u64 {
        self.record.offset().unwrap_or(0)
    }

    /// Size of the record body as stored.
    pub fn data_size(&self) -> u64 {
        self.record.data_size_as_read().map_or(0, u64::from)
    }
}

/// Read the record at `offset` of an open file, checking it is `expected` when given.
pub fn read_record_at(
    file: &mut File,
    file_name: &str,
    offset: u64,
    expected: Option<(Signature, u32)>,
    limits: &DecodeLimits,
) -> Result<LoadedRecord> {
    let io = |e| DocError::Io {
        path: file_name.into(),
        source: e,
    };
    let len = file.metadata().map_err(io)?.len();
    file.seek(SeekFrom::Start(offset)).map_err(io)?;
    let mut head = [0u8; 24];
    if offset.saturating_add(24) > len {
        return Err(DocError::esm(
            file_name,
            kitcraft_esm::Error::UnexpectedEof {
                offset,
                what: "record header",
                needed: 24,
                available: len.saturating_sub(offset),
            },
        ));
    }
    file.read_exact(&mut head).map_err(io)?;
    let mut r = Reader::new(&head, offset);
    let signature = r.signature().map_err(|e| DocError::esm(file_name, e))?;
    let size = u64::from(r.u32().map_err(|e| DocError::esm(file_name, e))?);
    let _flags = r.u32().map_err(|e| DocError::esm(file_name, e))?;
    let form_id = r.u32().map_err(|e| DocError::esm(file_name, e))?;
    if signature == Signature::GRUP {
        return Err(DocError::RecordMismatch {
            file: file_name.to_string(),
            offset,
            expected: expected.map_or_else(|| "a".into(), |(s, f)| format!("{s}:{f:08X}")),
            found: "a group".into(),
        });
    }
    if let Some((sig, fid)) = expected {
        if sig != signature || fid != form_id {
            return Err(DocError::RecordMismatch {
                file: file_name.to_string(),
                offset,
                expected: format!("{sig}:{fid:08X}"),
                found: format!("{signature}:{form_id:08X}"),
            });
        }
    }
    let available = len.saturating_sub(offset).saturating_sub(24);
    if size > available {
        return Err(DocError::esm(
            file_name,
            kitcraft_esm::Error::UnexpectedEof {
                offset: offset.saturating_add(24),
                what: "record data",
                needed: size,
                available,
            },
        ));
    }
    let total = usize::try_from(size.saturating_add(24)).map_err(|_| {
        DocError::esm(
            file_name,
            kitcraft_esm::Error::TooLarge {
                offset,
                what: "record",
                len: size,
            },
        )
    })?;
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&head);
    file.take(size).read_to_end(&mut buf).map_err(io)?;
    if buf.len() != total {
        return Err(DocError::esm(
            file_name,
            kitcraft_esm::Error::UnexpectedEof {
                offset,
                what: "record data",
                needed: size,
                available: buf.len() as u64,
            },
        ));
    }
    let entry = Entry::parse_one(&buf, offset, 1, &ParseOptions::default())
        .map_err(|e| DocError::esm(file_name, e))?;
    let Entry::Record(record) = entry else {
        return Err(DocError::Invalid(format!(
            "{file_name}: no record at {offset}"
        )));
    };
    let record = record.into_owned();
    let compressed = record.header.is_compressed();
    let subs = if compressed {
        record
            .decode(limits)
            .map_err(|e| DocError::esm(file_name, e))?
            .unwrap_or_default()
    } else {
        record.subrecords().map(<[_]>::to_vec).unwrap_or_default()
    };
    Ok(LoadedRecord {
        file: file_name.to_string(),
        record,
        subs,
        compressed,
    })
}

/// Reads records of a load order, keeping one open handle per plugin.
#[derive(Debug, Default)]
pub struct RecordReader {
    files: HashMap<u16, File>,
    limits: DecodeLimits,
}

impl RecordReader {
    /// A reader with default decode limits.
    pub fn new() -> RecordReader {
        RecordReader::default()
    }

    /// Read the record `loc` of plugin `plugin`.
    pub fn read(&mut self, lo: &LoadOrder, plugin: u16, loc: &RecordLoc) -> Result<LoadedRecord> {
        let p = lo
            .plugin(plugin)
            .ok_or_else(|| DocError::Invalid(format!("no plugin #{plugin}")))?;
        let file = match self.files.entry(plugin) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(File::open(&p.path).map_err(|err| DocError::io(&p.path, err))?)
            }
        };
        read_record_at(
            file,
            &p.name,
            loc.offset,
            Some((loc.signature(), loc.form_id)),
            &self.limits,
        )
    }
}

/// Read one record of a file by path (opens the file).
pub fn read_record_file(path: &Path, loc: &RecordLoc) -> Result<LoadedRecord> {
    let mut file = File::open(path).map_err(|e| DocError::io(path, e))?;
    read_record_at(
        &mut file,
        &file_name_of(path),
        loc.offset,
        Some((loc.signature(), loc.form_id)),
        &DecodeLimits::default(),
    )
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::index::PluginIndex;
    use crate::testdata;

    #[test]
    fn reads_indexed_records_and_rejects_wrong_offsets() {
        let bytes = testdata::plugin_with(
            0x1,
            &[],
            &[
                testdata::rec(
                    b"WEAP",
                    0x800,
                    &[("EDID", b"A\0".to_vec()), ("DATA", vec![9])],
                ),
                testdata::compressed(b"WEAP", 0x801, &[("EDID", b"B\0".to_vec())]),
            ],
        );
        let path = testdata::temp_file("rec.esm", &bytes);
        let index = PluginIndex::build(&path).unwrap();
        let a = read_record_file(&path, &index.records[0]).unwrap();
        assert!(!a.compressed);
        assert_eq!(a.subs.len(), 2);
        assert_eq!(a.subs[1].data(), &[9]);
        assert!(a.subs[0].offset().is_some());
        let b = read_record_file(&path, &index.records[1]).unwrap();
        assert!(b.compressed);
        assert_eq!(b.subs[0].data(), b"B\0");

        let mut wrong = index.records[0];
        wrong.offset += 1;
        assert!(read_record_file(&path, &wrong).is_err());
        let mut wrong = index.records[0];
        wrong.form_id = 0x999;
        assert!(matches!(
            read_record_file(&path, &wrong),
            Err(DocError::RecordMismatch { .. })
        ));
        let mut past = index.records[0];
        past.offset = bytes.len() as u64 - 10;
        assert!(read_record_file(&path, &past).is_err());
        // The group header just before the first record.
        let mut group = index.records[0];
        group.offset -= 24;
        assert!(matches!(
            read_record_file(&path, &group),
            Err(DocError::RecordMismatch { .. })
        ));
    }
}
