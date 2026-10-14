//! Name and directory hashes of archive records.
//!
//! `corpus:` every record of the 90 official archives (inventory in `docs/corpus.md`,
//! "Archives") satisfies:
//!
//! - the hash is CRC-32 with the reflected polynomial `0xEDB88320`, initial value 0 and no
//!   final XOR, over the lower-cased bytes;
//! - the name hash covers the file name without its extension (the text before the last
//!   `.`; a name without `.` is hashed whole and its extension field is zero);
//! - the extension field is the first four bytes of the extension, zero-padded;
//! - the directory hash covers the directory part with `\` separators, although the stored
//!   names use `/`.

/// CRC table for the reflected polynomial.
const TABLE: [u32; 256] = table();

// Evaluated at compile time only (`TABLE` is a `const`): an out-of-bounds index or overflow
// here is a build error, never a runtime panic.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const fn table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 == 1 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

/// The archive CRC over ASCII-lower-cased `bytes`.
pub fn crc(bytes: &[u8]) -> u32 {
    let mut c = 0u32;
    for &b in bytes {
        let index = usize::from((c as u8) ^ b.to_ascii_lowercase());
        // `index` < 256 by construction; `get` keeps the no-indexing rule.
        c = TABLE.get(index).copied().unwrap_or(0) ^ (c >> 8);
    }
    c
}

/// `(name_hash, extension, dir_hash)` for an archive path (either separator).
pub fn record_hashes(path: &[u8]) -> (u32, [u8; 4], u32) {
    let split = path.iter().rposition(|&b| b == b'/' || b == b'\\');
    let (dir, base) = match split {
        Some(i) => (
            path.get(..i).unwrap_or_default(),
            path.get(i.saturating_add(1)..).unwrap_or_default(),
        ),
        None => (&[][..], path),
    };
    let (stem, ext) = match base.iter().rposition(|&b| b == b'.') {
        Some(i) => (
            base.get(..i).unwrap_or_default(),
            base.get(i.saturating_add(1)..).unwrap_or_default(),
        ),
        None => (base, &[][..]),
    };
    let mut ext4 = [0u8; 4];
    for (slot, b) in ext4.iter_mut().zip(ext) {
        *slot = b.to_ascii_lowercase();
    }
    let dir: Vec<u8> = dir
        .iter()
        .map(|&b| if b == b'/' { b'\\' } else { b })
        .collect();
    (crc(stem), ext4, crc(&dir))
}

/// A byte whose Archive2 hash contribution changes from run to run (see
/// [`archive2_record_hashes`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnstableByte {
    /// The byte (Windows-1252, lower case).
    pub byte: u8,
    /// Its position in the path.
    pub index: usize,
}

/// What `Archive2.exe` 1.2.0.1 hashes a byte of a (lower-cased, Windows-1252) path as:
/// ASCII lower-cased; `0` for most non-ASCII bytes; `None` for 0xF0..=0xF5 and
/// 0xF8..=0xFD, which it replaces by bytes that look like parts of two 64-bit memory
/// addresses (observed `18 5B AE AA FD 7F` and `10 99 62 AB FD 7F`), so not a rule.
fn archive2_byte(b: u8) -> Option<u8> {
    match b {
        0x00..=0x7F => Some(b.to_ascii_lowercase()),
        0xF0..=0xF5 | 0xF8..=0xFD => None,
        _ => Some(0),
    }
}

/// CRC of `segment` as Archive2 hashes it: bytes mapped by [`archive2_byte`], up to the
/// first byte that maps to 0. `offset` is the segment's position in the path.
fn archive2_crc(segment: &[u8], offset: usize) -> Result<u32, UnstableByte> {
    let mut mapped = Vec::with_capacity(segment.len());
    for (i, &b) in segment.iter().enumerate() {
        match archive2_byte(b) {
            Some(0) => break,
            Some(m) => mapped.push(m),
            None => {
                return Err(UnstableByte {
                    byte: b,
                    index: offset.saturating_add(i),
                })
            }
        }
    }
    Ok(crc(&mapped))
}

/// `(name_hash, extension, dir_hash)` as `black-box: Archive2.exe 1.2.0.1` computes them
/// for a stored path (lower case, Windows-1252, either separator; M3.G3, `docs/corpus.md`
/// "Archives"). For an ASCII path this equals [`record_hashes`]. A non-ASCII byte other
/// than 0xF0..=0xF5 and 0xF8..=0xFD ends the hashed part of its name or directory, so
/// `café` and `cafè` hash like `caf` (and Archive2 keeps only the first of such files);
/// in the extension field it is a zero byte. A path whose hash would depend on one of
/// those twelve bytes is `Err`: Archive2's value for it is not reproducible.
pub fn archive2_record_hashes(path: &[u8]) -> Result<(u32, [u8; 4], u32), UnstableByte> {
    let split = path.iter().rposition(|&b| b == b'/' || b == b'\\');
    let (dir, base, base_at) = match split {
        Some(i) => (
            path.get(..i).unwrap_or_default(),
            path.get(i.saturating_add(1)..).unwrap_or_default(),
            i.saturating_add(1),
        ),
        None => (&[][..], path, 0),
    };
    let (stem, ext, ext_at) = match base.iter().rposition(|&b| b == b'.') {
        Some(i) => (
            base.get(..i).unwrap_or_default(),
            base.get(i.saturating_add(1)..).unwrap_or_default(),
            base_at.saturating_add(i).saturating_add(1),
        ),
        None => (base, &[][..], base_at),
    };
    let mut ext4 = [0u8; 4];
    for (i, (slot, &b)) in ext4.iter_mut().zip(ext).enumerate() {
        *slot = archive2_byte(b).ok_or(UnstableByte {
            byte: b,
            index: ext_at.saturating_add(i),
        })?;
    }
    let dir: Vec<u8> = dir
        .iter()
        .map(|&b| if b == b'/' { b'\\' } else { b })
        .collect();
    Ok((archive2_crc(stem, base_at)?, ext4, archive2_crc(&dir, 0)?))
}

#[cfg(test)]
mod tests {
    use super::{archive2_record_hashes, crc, record_hashes, UnstableByte};

    /// `black-box: Archive2.exe 1.2.0.1` (2026-10-14, M3.G3): hash fields Archive2 wrote
    /// for synthetic files with non-ASCII names (Windows-1252 bytes as stored).
    #[test]
    fn archive2_observations() {
        let dir = 0x459A_5314; // crc("kctest")
        assert_eq!(crc(b"kctest"), dir);
        for (path, name_hash) in [
            (&b"kctest/a\xE4.txt"[..], 0x3AB5_51CEu32),
            (b"kctest/eur\x80.txt", 0xA25F_9468),
            (b"kctest/e\xE9.txt", 0x3DD8_95D7),
            (b"kctest/mixa\xE9b.txt", 0x5319_7D41),
            (b"kctest/nb\xA0x.txt", 0x58C2_465D),
            (b"kctest/oe\x9C.txt", 0xDFBD_E2BF),
            (b"kctest/s\xDF.txt", 0xC90C_2086),
            (b"kctest/uc\xE4.txt", 0x86F3_BD51),
            (b"kctest/y\xFF.txt", 0x29D9_C998),
        ] {
            assert_eq!(
                archive2_record_hashes(path),
                Ok((name_hash, *b"txt\0", dir)),
                "{}",
                String::from_utf8_lossy(path)
            );
        }
        // A non-ASCII directory byte ends the directory hash; a non-ASCII extension byte
        // is a zero in the extension field.
        assert_eq!(
            archive2_record_hashes(b"kctest/d\xE9p/plain.txt"),
            Ok((0xDF02_95D2, *b"txt\0", 0x801D_2598))
        );
        assert_eq!(crc(b"kctest\\d"), 0x801D_2598);
        assert_eq!(
            archive2_record_hashes(b"kctest/ext.t\xEBx"),
            Ok((0xFE92_4F10, *b"t\0x\0", dir))
        );
        // ASCII paths: the corpus rule.
        assert_eq!(
            archive2_record_hashes(b"strings/starfield_de.dlstrings"),
            Ok(record_hashes(b"strings/starfield_de.dlstrings"))
        );
    }

    /// The twelve bytes whose substitute is not a rule are refused, wherever they would
    /// be hashed; after an earlier non-ASCII byte they no longer matter.
    #[test]
    fn archive2_unstable_bytes() {
        for b in (0xF0u8..=0xF5).chain(0xF8..=0xFD) {
            let path = [b'd', b'/', b'x', b, b'.', b't'];
            assert_eq!(
                archive2_record_hashes(&path),
                Err(UnstableByte { byte: b, index: 3 })
            );
            let ext = [b'x', b'.', b'a', b];
            assert_eq!(
                archive2_record_hashes(&ext),
                Err(UnstableByte { byte: b, index: 3 })
            );
            let dir = [b'a', b, b'/', b'x'];
            assert!(archive2_record_hashes(&dir).is_err());
            let after = [b'x', 0xE9, b, b'.', b't'];
            assert_eq!(
                archive2_record_hashes(&after),
                Ok((crc(b"x"), *b"t\0\0\0", 0))
            );
        }
        for b in [0xF6u8, 0xF7, 0xFE, 0xFF, 0x80, 0xC0, 0xEF] {
            assert_eq!(
                archive2_record_hashes(&[b'x', b, b'.', b't']),
                Ok((crc(b"x"), *b"t\0\0\0", 0)),
                "{b:#04x}"
            );
        }
    }

    #[test]
    fn corpus_examples() {
        // corpus: `Starfield - Localization.ba2`, `strings/starfield_de.dlstrings`.
        assert_eq!(crc(b"starfield_de"), 0x1EE3_606A);
        assert_eq!(crc(b"strings"), 0x29F6_B58B);
        assert_eq!(
            record_hashes(b"strings/starfield_de.dlstrings"),
            (0x1EE3_606A, *b"dlst", 0x29F6_B58B)
        );
        assert_eq!(
            record_hashes(b"Strings\\StarField_DE.DLSTRINGS"),
            (0x1EE3_606A, *b"dlst", 0x29F6_B58B)
        );
    }

    #[test]
    fn names_without_extension_or_directory() {
        let (name, ext, dir) = record_hashes(b"materials/weapons/bigbang/decals");
        assert_eq!(name, crc(b"decals"));
        assert_eq!(ext, [0; 4]);
        assert_eq!(dir, crc(b"materials\\weapons\\bigbang"));
        assert_eq!(record_hashes(b"a.b.nif"), (crc(b"a.b"), *b"nif\0", 0));
        assert_eq!(record_hashes(b""), (0, [0; 4], 0));
    }
}
