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

#[cfg(test)]
mod tests {
    use super::{crc, record_hashes};

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
