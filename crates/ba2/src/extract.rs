//! Extracting one entry, streaming.
//!
//! GNRL data is stored or packed whole; DX10 data is a synthesized DDS header followed by
//! each chunk, stored or packed on its own. Every blob is checked against the size its
//! record states, and decoding stops one byte past it, so a hostile stream cannot expand
//! without bound. The declared sizes themselves are bounded by callers: [`Archive::extract_vec`]
//! takes a cap checked before anything is decoded (a 510 KB archive can declare 512 MiB of
//! zlib output), and one LZ4 blob may declare at most [`LZ4_MAX_BLOB`] bytes.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use flate2::read::ZlibDecoder;

use crate::error::{io, Error, Result};
use crate::read::{Archive, Body, Codec};

/// LZ4 cannot expand input by more than this factor (each 1-byte token can describe at
/// most 255 more literal or match bytes); larger declared sizes are refused before
/// allocating.
const LZ4_MAX_RATIO: u64 = 255;

/// Most bytes one LZ4 blob may declare (256 MiB). The LZ4 path decodes into a buffer of the
/// declared size, so this bounds that allocation whatever the packed size; the largest
/// official chunk is 33,554,592 bytes (`corpus:`, `docs/corpus.md` "Archives").
pub const LZ4_MAX_BLOB: u64 = 256 << 20;

/// One stored or packed run of bytes: a GNRL file or a DX10 chunk.
struct Blob<'a> {
    /// Archive path of the entry, for errors.
    name: &'a str,
    /// Data offset in the archive.
    offset: u64,
    /// Packed size; 0 when stored.
    packed: u32,
    /// Size after decoding.
    size: u32,
    /// Codec of packed data; `None` when stored.
    codec: Option<Codec>,
}

impl Archive {
    /// Writes entry `index` to `out` and returns the number of bytes written.
    pub fn extract_to<R: Read + Seek, W: Write>(
        &self,
        reader: &mut R,
        index: usize,
        out: &mut W,
    ) -> Result<u64> {
        let entry = self.entries.get(index).ok_or(Error::NoSuchEntry {
            index,
            count: self.entries.len(),
        })?;
        let codec = |packed: u32| -> Result<Option<Codec>> {
            if packed == 0 {
                Ok(None)
            } else {
                self.header.codec().map(Some)
            }
        };
        match &entry.body {
            Body::General(g) => {
                let blob = Blob {
                    name: &entry.name,
                    offset: g.offset,
                    packed: g.packed_size,
                    size: g.size,
                    codec: codec(g.packed_size)?,
                };
                blob.extract(reader, out, 0)
            }
            Body::Texture(t) => {
                let header = crate::dds::header(t);
                out.write_all(&header).map_err(io(0))?;
                let mut written = header.len() as u64;
                for chunk in &t.chunks {
                    let blob = Blob {
                        name: &entry.name,
                        offset: chunk.offset,
                        packed: chunk.packed_size,
                        size: chunk.size,
                        codec: codec(chunk.packed_size)?,
                    };
                    let n = blob.extract(reader, out, written)?;
                    written = written.saturating_add(n);
                }
                Ok(written)
            }
        }
    }

    /// Entry `index` in memory, refused with [`Error::TooLarge`] before anything is read
    /// when its declared extracted size is above `cap` bytes. Callers pick the cap for
    /// what they expect (a script, a string table); the buffer then grows to at most `cap`
    /// plus one byte, because decoding stops one byte past each blob's declared size.
    pub fn extract_vec<R: Read + Seek>(
        &self,
        reader: &mut R,
        index: usize,
        cap: u64,
    ) -> Result<Vec<u8>> {
        let entry = self.entries.get(index).ok_or(Error::NoSuchEntry {
            index,
            count: self.entries.len(),
        })?;
        let size = entry.extracted_len();
        if size > cap {
            return Err(Error::TooLarge {
                name: entry.name.clone(),
                offset: entry.record_offset,
                size,
                cap,
            });
        }
        let mut out = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        self.extract_to(reader, index, &mut out)?;
        Ok(out)
    }

    /// Index of the entry with this archive path (ASCII case-insensitive, either
    /// separator).
    pub fn find(&self, path: &str) -> Option<usize> {
        let want = normalize(path);
        self.entries.iter().position(|e| normalize(&e.name) == want)
    }
}

/// Lower-cases and turns `\` into `/`.
pub fn normalize(path: &str) -> String {
    path.chars()
        .map(|c| {
            if c == '\\' {
                '/'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

impl Blob<'_> {
    /// Decodes the blob into `out` (at output offset `out_offset`, for errors), verified to
    /// be exactly `size` bytes.
    fn extract<R: Read + Seek, W: Write>(
        &self,
        reader: &mut R,
        out: &mut W,
        out_offset: u64,
    ) -> Result<u64> {
        let Self {
            name,
            offset,
            packed,
            size,
            codec,
        } = *self;
        reader.seek(SeekFrom::Start(offset)).map_err(io(offset))?;
        let size = u64::from(size);
        let mismatch = |actual: u64| Error::SizeMismatch {
            name: name.to_string(),
            offset,
            expected: size,
            actual,
        };
        let decompress = |message: String| Error::Decompress {
            name: name.to_string(),
            offset,
            message,
        };
        let n = match codec {
            None => copy(&mut (&mut *reader).take(size), out, out_offset, |e| {
                Error::Io { offset, source: e }
            })?,
            Some(Codec::Zlib) => {
                let mut decoder = ZlibDecoder::new((&mut *reader).take(u64::from(packed)));
                copy(
                    &mut (&mut decoder).take(size.saturating_add(1)),
                    out,
                    out_offset,
                    |e| decompress(e.to_string()),
                )?
            }
            Some(Codec::Lz4Block) => lz4(reader, packed, size, out, out_offset, &decompress)?,
        };
        if n != size {
            return Err(mismatch(n));
        }
        Ok(n)
    }
}

/// One LZ4 block of `packed` bytes at the reader's position, decoded into a buffer of the
/// declared `size` (refused above [`LZ4_MAX_BLOB`] or 255 times `packed`).
fn lz4<R: Read, W: Write>(
    reader: &mut R,
    packed: u32,
    size: u64,
    out: &mut W,
    out_offset: u64,
    decompress: &dyn Fn(String) -> Error,
) -> Result<u64> {
    let packed = u64::from(packed);
    if size > LZ4_MAX_BLOB {
        return Err(decompress(format!(
            "declared size {size} is above the {LZ4_MAX_BLOB}-byte limit for one LZ4 blob"
        )));
    }
    if size > packed.saturating_mul(LZ4_MAX_RATIO).saturating_add(16) {
        return Err(decompress(format!(
            "declared size {size} is impossible for {packed} LZ4 bytes"
        )));
    }
    let mut src = Vec::new();
    reader
        .take(packed)
        .read_to_end(&mut src)
        .map_err(|e| decompress(e.to_string()))?;
    if src.len() as u64 != packed {
        return Err(decompress(format!(
            "{} of {packed} packed bytes present",
            src.len()
        )));
    }
    let len = usize::try_from(size).map_err(|_| decompress(format!("size {size}")))?;
    let mut dst = vec![0u8; len];
    let got =
        lz4_flex::block::decompress_into(&src, &mut dst).map_err(|e| decompress(e.to_string()))?;
    let data = dst.get(..got).unwrap_or_default();
    out.write_all(data).map_err(io(out_offset))?;
    Ok(got as u64)
}

/// Copies `src` to `out`; read errors go through `on_read`, write errors are I/O errors at
/// the output offset.
fn copy<R: Read, W: Write>(
    src: &mut R,
    out: &mut W,
    out_offset: u64,
    on_read: impl Fn(std::io::Error) -> Error,
) -> Result<u64> {
    let mut buf = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = match src.read(&mut buf) {
            Ok(0) => return Ok(total),
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(on_read(e)),
        };
        let chunk = buf.get(..n).unwrap_or_default();
        out.write_all(chunk)
            .map_err(io(out_offset.saturating_add(total)))?;
        total = total.saturating_add(n as u64);
    }
}

/// Windows device names: a path component whose stem (the text before its first `.`, with
/// trailing spaces dropped) is one of these, ASCII case-insensitively, opens the device
/// instead of a file. `ms-docs: Naming Files, Paths, and Namespaces`.
const RESERVED_STEMS: [&str; 30] = [
    "CON",
    "PRN",
    "AUX",
    "NUL",
    "COM0",
    "COM1",
    "COM2",
    "COM3",
    "COM4",
    "COM5",
    "COM6",
    "COM7",
    "COM8",
    "COM9",
    "COM\u{b9}",
    "COM\u{b2}",
    "COM\u{b3}",
    "LPT0",
    "LPT1",
    "LPT2",
    "LPT3",
    "LPT4",
    "LPT5",
    "LPT6",
    "LPT7",
    "LPT8",
    "LPT9",
    "LPT\u{b9}",
    "LPT\u{b2}",
    "LPT\u{b3}",
];

/// Why one path component cannot be a Windows file or directory name, if it cannot.
fn windows_refusal(part: &str) -> Option<&'static str> {
    if part.ends_with(['.', ' ']) {
        return Some("a component ends in `.` or a space (Windows drops them)");
    }
    if part
        .chars()
        .any(|c| c < ' ' || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Some("a component has a character Windows forbids in file names");
    }
    let stem = part.split('.').next().unwrap_or(part).trim_end_matches(' ');
    if RESERVED_STEMS.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Some("a component is a Windows device name (CON, NUL, COM1, ...)");
    }
    None
}

/// The relative file path an archive path extracts to. Refuses absolute paths, drive
/// prefixes, `..`, and empty names, so extraction never leaves the output directory, and
/// names Windows would alter or treat as devices (components ending in `.` or a space,
/// forbidden characters, `CON`, `NUL.txt`, `com1`, ...), so the path written is the path
/// planned on every platform.
pub fn relative_path(name: &str) -> Result<PathBuf> {
    let refuse = |reason| Error::UnsafePath {
        name: name.to_string(),
        reason,
    };
    if name.is_empty() {
        return Err(refuse("empty name"));
    }
    if name.contains(':') {
        return Err(refuse("contains `:`"));
    }
    let mut path = PathBuf::new();
    for part in name.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(refuse("contains `..`"));
        }
        if let Some(reason) = windows_refusal(part) {
            return Err(refuse(reason));
        }
        path.push(part);
    }
    if path.as_os_str().is_empty() {
        return Err(refuse("no file name"));
    }
    if Path::new(&path)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(refuse("not a plain relative path"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{lz4, normalize, relative_path, LZ4_MAX_BLOB};
    use crate::Error;
    use std::io::Cursor;
    use std::path::PathBuf;

    /// Regression (review round 2): an LZ4 blob declaring more than 256 MiB is refused
    /// before its 255x-packed buffer is allocated, even when the ratio would allow it.
    #[test]
    fn lz4_blob_cap() {
        let decompress = |message: String| Error::Decompress {
            name: "t.dds".into(),
            offset: 0,
            message,
        };
        let mut sink = Vec::new();
        let huge = LZ4_MAX_BLOB + 1;
        let err = lz4(
            &mut Cursor::new(vec![0u8; 16]),
            u32::MAX,
            huge,
            &mut sink,
            0,
            &decompress,
        )
        .unwrap_err();
        assert!(
            matches!(&err, Error::Decompress { message, .. } if message.contains("limit")),
            "{err}"
        );
        // Within the cap, the ratio check still applies.
        let err = lz4(
            &mut Cursor::new(vec![0u8; 16]),
            1,
            1 << 20,
            &mut sink,
            0,
            &decompress,
        )
        .unwrap_err();
        assert!(err.to_string().contains("impossible"), "{err}");
        // A short read is a typed error, not a panic.
        let err = lz4(
            &mut Cursor::new(vec![0u8; 4]),
            8,
            16,
            &mut sink,
            0,
            &decompress,
        )
        .unwrap_err();
        assert!(err.to_string().contains("4 of 8"), "{err}");
        assert!(sink.is_empty());
    }

    #[test]
    fn relative_paths_stay_inside() {
        assert_eq!(
            relative_path("scripts/form.pex").unwrap(),
            PathBuf::from("scripts").join("form.pex")
        );
        assert_eq!(
            relative_path("a\\\\b/./c").unwrap(),
            PathBuf::from("a").join("b").join("c")
        );
        for bad in ["", "../x", "a/../../x", "C:/x", "c:x", "/", "./"] {
            assert!(relative_path(bad).is_err(), "{bad}");
        }
        // A leading separator is dropped, not treated as the root.
        assert_eq!(relative_path("/a").unwrap(), PathBuf::from("a"));
    }

    /// Regression (review round 2): names Windows rewrites or maps to devices.
    #[test]
    fn windows_names_are_refused() {
        for bad in [
            "a./b.txt",
            "a /b.txt",
            "dir/file.",
            "dir/file ",
            "...",
            "con",
            "CON.txt",
            "x/Nul.tar.gz",
            "x/aux /y",
            "com1.dds",
            "LPT9",
            "lpt\u{b9}.txt",
            "COM0",
            "a/b?.txt",
            "a/<b>",
            "a\"b",
            "a|b",
            "a*b",
            "a\u{1}b",
        ] {
            let err = relative_path(bad).unwrap_err();
            assert!(matches!(err, Error::UnsafePath { .. }), "{bad}: {err}");
        }
        for good in [
            "console.txt",
            "connector/a.nif",
            "com10.txt",
            "lpt.txt",
            "nul_x.txt",
            "a.b/c",
            ".hidden",
            "con_.dds",
            "\u{e9}t\u{e9}.txt",
        ] {
            assert!(relative_path(good).is_ok(), "{good}");
        }
    }

    #[test]
    fn normalize_folds_case_and_separators() {
        assert_eq!(normalize("Scripts\\Form.PEX"), "scripts/form.pex");
    }
}
