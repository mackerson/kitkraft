//! Synthetic fixtures: pack -> read -> extract roundtrips and hostile input.

use std::collections::BTreeMap;
use std::io::Cursor;

use crate::{dds, pack, Archive, Body, Error, Kind, PackOptions, Texture};

/// Deterministic bytes: a compressible run then pseudo-random noise.
fn bytes(seed: u32, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..len)
        .map(|i| {
            if i % 3 == 0 {
                (i / 64) as u8
            } else {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            }
        })
        .collect()
}

fn dds_file(format: u8, width: u16, height: u16, mips: u8, cubemap: u8, seed: u32) -> Vec<u8> {
    let texture = Texture {
        unknown: 0,
        chunk_record_len: 24,
        height,
        width,
        mip_count: mips,
        format,
        cubemap,
        tile_mode: 8,
        chunks: Vec::new(),
    };
    let faces = dds::faces(&texture);
    let len = dds::chain_len(
        format,
        u32::from(width),
        u32::from(height),
        u32::from(mips),
        faces,
    )
    .unwrap();
    let mut out = dds::header(&texture);
    out.extend(bytes(seed, len as usize));
    out
}

fn pack_mem(files: &BTreeMap<String, Vec<u8>>, options: &PackOptions) -> Vec<u8> {
    let inputs: Vec<(String, String)> = files.keys().map(|k| (k.clone(), k.clone())).collect();
    let mut out = Cursor::new(Vec::new());
    pack(
        &inputs,
        |key| Ok(files.get(key).cloned().unwrap()),
        options,
        &mut out,
    )
    .unwrap();
    out.into_inner()
}

fn roundtrip(files: &BTreeMap<String, Vec<u8>>, options: &PackOptions) -> Vec<u8> {
    let archive_bytes = pack_mem(files, options);
    let mut cursor = Cursor::new(archive_bytes.clone());
    let archive = Archive::read(&mut cursor).unwrap();
    assert_eq!(archive.header.version, options.version);
    assert_eq!(archive.header.kind, options.kind);
    assert_eq!(archive.entries.len(), files.len());
    for (i, entry) in archive.entries.iter().enumerate() {
        let original = files
            .iter()
            .find(|(k, _)| crate::normalize(k) == entry.name)
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("unexpected name {}", entry.name));
        assert!(entry.hashes_match_name(), "{}", entry.name);
        assert_eq!(
            entry.extracted_len(),
            original.len() as u64,
            "{}",
            entry.name
        );
        let out = archive.extract_vec(&mut cursor, i).unwrap();
        assert!(out == *original, "{} differs after extraction", entry.name);
    }
    archive_bytes
}

fn general_files() -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    files.insert("Scripts/Alpha.pex".to_string(), bytes(1, 5000));
    files.insert("strings\\mymod_en.strings".to_string(), bytes(2, 300));
    files.insert("meshes/x/noext".to_string(), vec![7; 2048]);
    files.insert("empty.txt".to_string(), Vec::new());
    files.insert("tiny.bin".to_string(), vec![1, 2, 3]);
    files
}

#[test]
fn general_v2_and_v3_roundtrip_and_are_deterministic() {
    for version in [2, 3] {
        for compress in [true, false] {
            let options = PackOptions {
                kind: Kind::General,
                version,
                compress,
            };
            let files = general_files();
            let a = roundtrip(&files, &options);
            let b = pack_mem(&files, &options);
            assert_eq!(a, b, "packing is deterministic");
            let archive = Archive::read(&mut Cursor::new(a)).unwrap();
            let names: Vec<&str> = archive.entries.iter().map(|e| e.name.as_str()).collect();
            assert_eq!(
                names,
                [
                    "empty.txt",
                    "meshes/x/noext",
                    "scripts/alpha.pex",
                    "strings/mymod_en.strings",
                    "tiny.bin"
                ]
            );
            let compressed = archive.entries.iter().filter(|e| e.is_compressed()).count();
            if compress {
                // The all-sevens file shrinks under both codecs; empty and 3-byte files
                // stay stored.
                assert!(compressed >= 1, "v{version}: {compressed} compressed");
                assert!(!archive.entries[0].is_compressed(), "empty file is stored");
            } else {
                assert_eq!(compressed, 0);
            }
            assert!(archive.has_names);
        }
    }
}

#[test]
fn textures_roundtrip_with_official_chunking() {
    let mut files = BTreeMap::new();
    files.insert(
        "textures/a_color.dds".into(),
        dds_file(72, 2048, 2048, 12, 0, 1),
    );
    files.insert(
        "textures/b_normal.dds".into(),
        dds_file(84, 1024, 512, 11, 0, 2),
    );
    files.insert(
        "textures/c_mask.dds".into(),
        dds_file(80, 256, 256, 9, 0, 3),
    );
    files.insert("textures/d_cube.dds".into(), dds_file(10, 64, 64, 7, 1, 4));
    files.insert("textures/e_flat.dds".into(), dds_file(28, 16, 256, 1, 0, 5));
    // 1024x256 has 512 x 512 pixels: its top mip gets its own chunk (area rule).
    files.insert(
        "textures/f_wide.dds".into(),
        dds_file(72, 1024, 256, 11, 0, 6),
    );
    for version in [2, 3] {
        let options = PackOptions {
            kind: Kind::Textures,
            version,
            compress: true,
        };
        let bytes = roundtrip(&files, &options);
        let archive = Archive::read(&mut Cursor::new(bytes)).unwrap();
        let chunks: Vec<Vec<(u16, u16)>> = archive
            .entries
            .iter()
            .map(|e| match &e.body {
                Body::Texture(t) => t.chunks.iter().map(|c| (c.mip_first, c.mip_last)).collect(),
                Body::General(_) => panic!("general entry in a DX10 archive"),
            })
            .collect();
        assert_eq!(
            chunks,
            [
                vec![(0, 0), (1, 1), (2, 2), (3, 11)],
                vec![(0, 0), (1, 10)],
                vec![(0, 8)],
                vec![(0, 6)],
                vec![(0, 0)],
                vec![(0, 0), (1, 10)],
            ]
        );
    }
}

/// Regression (review round 2): the split rule is by area, not by both edges.
#[test]
fn chunk_plan_is_area_based() {
    use crate::chunk_mips;
    assert_eq!(chunk_mips(1024, 256, 11, false), [(0, 0), (1, 10)]);
    assert_eq!(chunk_mips(256, 1024, 11, false), [(0, 0), (1, 10)]);
    assert_eq!(chunk_mips(2048, 128, 12, false), [(0, 0), (1, 11)]);
    assert_eq!(chunk_mips(1024, 128, 11, false), [(0, 10)]);
    assert_eq!(
        chunk_mips(4096, 4096, 13, false),
        [(0, 0), (1, 1), (2, 2), (3, 12)]
    );
    assert_eq!(chunk_mips(512, 512, 1, false), [(0, 0)]);
    assert_eq!(chunk_mips(512, 512, 2, false), [(0, 0), (1, 1)]);
    assert_eq!(chunk_mips(2048, 2048, 12, true), [(0, 11)]);
    // Saturating: the largest dimensions never overflow.
    assert_eq!(
        chunk_mips(u32::MAX, u32::MAX, 3, false),
        [(0, 0), (1, 1), (2, 2)]
    );
}

#[test]
fn pack_refuses_bad_inputs() {
    let mut out = Cursor::new(Vec::new());
    let options = PackOptions {
        kind: Kind::Textures,
        version: 2,
        compress: true,
    };
    let mut bad = dds_file(72, 64, 64, 7, 0, 1);
    bad.pop();
    let err = pack(
        &[("t.dds".into(), "t".into())],
        |_| Ok(bad.clone()),
        &options,
        &mut out,
    )
    .unwrap_err();
    assert!(err.to_string().contains("mip chain"), "{err}");

    let general = PackOptions::default();
    for name in ["../x", "a/../../b", "C:/abs"] {
        assert!(
            pack(
                &[(name.into(), "x".into())],
                |_| Ok(vec![1]),
                &general,
                &mut out
            )
            .is_err(),
            "{name}"
        );
    }
    let dup = pack(
        &[("A.txt".into(), "1".into()), ("a.TXT".into(), "2".into())],
        |_| Ok(vec![1]),
        &general,
        &mut out,
    );
    assert!(dup.is_err());
    let v4 = PackOptions {
        version: 4,
        ..PackOptions::default()
    };
    assert!(pack(&[], |_| Ok(vec![]), &v4, &mut out).is_err());
}

#[test]
fn header_errors_are_typed() {
    let good = pack_mem(&general_files(), &PackOptions::default());
    let read = |b: &[u8]| Archive::read(&mut Cursor::new(b.to_vec()));
    let mut bad = good.clone();
    bad[0] = b'X';
    assert!(matches!(read(&bad), Err(Error::BadMagic { .. })));
    let mut bad = good.clone();
    bad[4] = 9;
    assert!(matches!(
        read(&bad),
        Err(Error::UnsupportedVersion { version: 9 })
    ));
    let mut bad = good.clone();
    bad[8] = b'Z';
    assert!(matches!(read(&bad), Err(Error::UnknownKind { .. })));
    let mut bad = good.clone();
    bad[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        read(&bad),
        Err(Error::Truncated { offset: 32, .. })
    ));
    assert!(matches!(
        read(&good[..10]),
        Err(Error::Truncated { offset: 0, .. })
    ));
}

#[test]
fn every_truncation_is_an_error_not_a_panic() {
    for options in [
        PackOptions::default(),
        PackOptions {
            kind: Kind::Textures,
            version: 3,
            compress: true,
        },
    ] {
        let files = if options.kind == Kind::Textures {
            let mut f = BTreeMap::new();
            f.insert("t/a.dds".to_string(), dds_file(80, 64, 64, 7, 0, 9));
            f
        } else {
            general_files()
        };
        let good = pack_mem(&files, &options);
        for len in 0..good.len() {
            let mut cursor = Cursor::new(good[..len].to_vec());
            match Archive::read(&mut cursor) {
                Err(_) => {}
                Ok(archive) => {
                    // Only the name table is gone or short at this point: data is intact.
                    for i in 0..archive.entries.len() {
                        let _ = archive.extract_vec(&mut cursor, i);
                    }
                }
            }
        }
    }
}

#[test]
fn bit_flips_never_panic_and_bad_data_is_typed() {
    let good = pack_mem(&general_files(), &PackOptions::default());
    for i in 0..good.len() {
        for mask in [0x01u8, 0x80, 0xFF] {
            let mut bad = good.clone();
            bad[i] ^= mask;
            let mut cursor = Cursor::new(bad);
            if let Ok(archive) = Archive::read(&mut cursor) {
                for j in 0..archive.entries.len() {
                    if let Err(e) = archive.extract_vec(&mut cursor, j) {
                        assert!(
                            matches!(
                                e,
                                Error::Decompress { .. }
                                    | Error::SizeMismatch { .. }
                                    | Error::UnsupportedCodec { .. }
                            ),
                            "byte {i}: {e}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_stream_larger_than_its_record_is_refused() {
    let mut files = BTreeMap::new();
    files.insert("big.txt".to_string(), vec![0u8; 100_000]);
    let mut bytes = pack_mem(&files, &PackOptions::default());
    // Record at 32: name hash, ext, dir hash, flags, offset u64, packed, size.
    let size_at = 32 + 4 + 4 + 4 + 4 + 8 + 4;
    bytes[size_at..size_at + 4].copy_from_slice(&10u32.to_le_bytes());
    let mut cursor = Cursor::new(bytes);
    let archive = Archive::read(&mut cursor).unwrap();
    let err = archive.extract_vec(&mut cursor, 0).unwrap_err();
    assert!(
        matches!(
            err,
            Error::SizeMismatch {
                expected: 10,
                actual: 11,
                ..
            }
        ),
        "{err}"
    );
    assert!(matches!(
        archive.extract_vec(&mut cursor, 5),
        Err(Error::NoSuchEntry { index: 5, count: 1 })
    ));
}

#[test]
fn archives_without_names_get_hash_names() {
    let mut files = BTreeMap::new();
    files.insert("dir/file.txt".to_string(), vec![1, 2, 3]);
    let mut bytes = pack_mem(&files, &PackOptions::default());
    bytes[16..24].copy_from_slice(&0u64.to_le_bytes());
    let archive = Archive::read(&mut Cursor::new(bytes)).unwrap();
    assert!(!archive.has_names);
    let (name, _, dir) = crate::hash::record_hashes(b"dir/file.txt");
    assert_eq!(archive.entries[0].name, format!("{dir:08x}/{name:08x}.txt"));
}
