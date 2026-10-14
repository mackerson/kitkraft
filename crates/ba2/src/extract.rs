//! Extracting one entry, streaming.
//!
//! GNRL data is stored or packed whole; DX10 data is a synthesized DDS header followed by
//! each chunk, stored or packed on its own. Every blob is checked against the size its
//! record states, and decoding stops one byte past it, so a hostile stream cannot expand
//! without bound.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use flate2::read::ZlibDecoder;

use crate::error::{io, Error, Result};
use crate::read::{Archive, Body, Codec};

/// LZ4 cannot expand input by more than this factor (each 1-byte token can describe at
/// most 255 more literal or match bytes); larger declared sizes are refused before
/// allocating.
const LZ4_MAX_RATIO: u64 = 255;

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
        match &entry.body {
            Body::General(g) => {
                let codec = if g.packed_size == 0 {
                    None
                } else {
                    Some(self.header.codec()?)
                };
                blob(
                    reader,
                    &entry.name,
                    g.offset,
                    g.packed_size,
                    g.size,
                    codec,
                    out,
                    0,
                )
            }
            Body::Texture(t) => {
                let header = crate::dds::header(t);
                out.write_all(&header).map_err(io(0))?;
                let mut written = header.len() as u64;
                for chunk in &t.chunks {
                    let codec = if chunk.packed_size == 0 {
                        None
                    } else {
                        Some(self.header.codec()?)
                    };
                    let n = blob(
                        reader,
                        &entry.name,
                        chunk.offset,
                        chunk.packed_size,
                        chunk.size,
                        codec,
                        out,
                        written,
                    )?;
                    written = written.saturating_add(n);
                }
                Ok(written)
            }
        }
    }

    /// Entry `index` in memory. The allocation is the record's declared extracted size,
    /// which [`Archive::read`] bounded by the file length for stored data; packed data is
    /// streamed into the buffer as it decodes.
    pub fn extract_vec<R: Read + Seek>(&self, reader: &mut R, index: usize) -> Result<Vec<u8>> {
        let mut out = Vec::new();
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

/// One stored or packed blob, verified to be exactly `size` bytes.
#[allow(clippy::too_many_arguments)]
fn blob<R: Read + Seek, W: Write>(
    reader: &mut R,
    name: &str,
    offset: u64,
    packed: u32,
    size: u32,
    codec: Option<Codec>,
    out: &mut W,
    out_offset: u64,
) -> Result<u64> {
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
        Some(Codec::Lz4Block) => {
            let packed = u64::from(packed);
            if size > packed.saturating_mul(LZ4_MAX_RATIO).saturating_add(16) {
                return Err(decompress(format!(
                    "declared size {size} is impossible for {packed} LZ4 bytes"
                )));
            }
            let mut src = Vec::new();
            (&mut *reader)
                .take(packed)
                .read_to_end(&mut src)
                .map_err(io(offset))?;
            if src.len() as u64 != packed {
                return Err(mismatch(src.len() as u64));
            }
            let len = usize::try_from(size).map_err(|_| mismatch(size))?;
            let mut dst = vec![0u8; len];
            let got = lz4_flex::block::decompress_into(&src, &mut dst)
                .map_err(|e| decompress(e.to_string()))?;
            let data = dst.get(..got).unwrap_or_default();
            out.write_all(data).map_err(io(out_offset))?;
            got as u64
        }
    };
    if n != size {
        return Err(mismatch(n));
    }
    Ok(n)
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

/// The relative file path an archive path extracts to. Refuses absolute paths, drive
/// prefixes, `..`, and empty names, so extraction never leaves the output directory.
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
    use super::{normalize, relative_path};
    use std::path::PathBuf;

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

    #[test]
    fn normalize_folds_case_and_separators() {
        assert_eq!(normalize("Scripts\\Form.PEX"), "scripts/form.pex");
    }
}
