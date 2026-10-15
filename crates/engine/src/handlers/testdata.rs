//! A small synthetic plugin for handler tests, built byte by byte from the container
//! layout `kitkraft-esm` documents (24-byte record and group headers, 6-byte subrecord
//! headers, `XXXX` size prefix, `u32` size + zlib body for compressed records).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

/// Records in [`plugin`], including `TES4`.
pub(crate) const RECORDS: u64 = 4;
/// Groups in [`plugin`].
pub(crate) const GROUPS: u64 = 2;

/// FormID of the uncompressed weapon (EDID `TestRifle`).
pub(crate) const RIFLE: u32 = 0x0001_A2B3;
/// FormID of the compressed weapon (EDID `PackedGun`).
pub(crate) const PACKED: u32 = 0x0001_A2B4;

pub(crate) fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = sig.to_vec();
    out.extend_from_slice(&u16::try_from(data.len()).unwrap().to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// A subrecord whose size is carried by an `XXXX` prefix.
pub(crate) fn xxxx_sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = b"XXXX".to_vec();
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    out.extend_from_slice(sig);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// A record with timestamp 0x1234, version control 0x5678, form version 581, unknown 7.
pub(crate) fn record(sig: &[u8; 4], flags: u32, form_id: u32, body: &[u8]) -> Vec<u8> {
    let mut out = sig.to_vec();
    out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&form_id.to_le_bytes());
    out.extend_from_slice(&0x1234u16.to_le_bytes());
    out.extend_from_slice(&0x5678u16.to_le_bytes());
    out.extend_from_slice(&581u16.to_le_bytes());
    out.extend_from_slice(&7u16.to_le_bytes());
    out.extend_from_slice(body);
    out
}

pub(crate) fn group(label: &[u8; 4], group_type: i32, contents: &[u8]) -> Vec<u8> {
    let mut out = b"GRUP".to_vec();
    out.extend_from_slice(&u32::try_from(contents.len() + 24).unwrap().to_le_bytes());
    out.extend_from_slice(label);
    out.extend_from_slice(&group_type.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(contents);
    out
}

/// The `DNAM` payload of the rifle: 300 bytes, so inspect's 256-byte hex cap applies.
pub(crate) fn long_payload() -> Vec<u8> {
    (0..300u32).map(|i| (i % 251) as u8).collect()
}

/// The fields inside the compressed weapon.
pub(crate) fn packed_fields() -> Vec<u8> {
    let mut fields = sub(b"EDID", b"PackedGun\0");
    fields.extend(sub(b"DATA", &[1, 2, 3, 4]));
    fields
}

/// `TES4` + `GRUP GMST` (one GMST) + `GRUP WEAP` (an uncompressed and a compressed weapon).
pub(crate) fn plugin() -> Vec<u8> {
    let mut hedr = 1.0f32.to_le_bytes().to_vec();
    hedr.extend_from_slice(&3u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let mut tes4 = sub(b"HEDR", &hedr);
    tes4.extend(sub(b"CNAM", b"kit\0"));
    let mut out = record(b"TES4", 1, 0, &tes4);

    let mut gmst = sub(b"EDID", b"fTestValue\0");
    gmst.extend(sub(b"DATA", &2.5f32.to_le_bytes()));
    out.extend(group(b"GMST", 0, &record(b"GMST", 0, 0x100, &gmst)));

    let mut rifle = sub(b"EDID", b"TestRifle\0");
    rifle.extend(sub(b"FULL", b"Rifle\0"));
    rifle.extend(xxxx_sub(b"MODL", b"a\\b.nif\0"));
    rifle.extend(sub(b"DNAM", &long_payload()));
    let mut weapons = record(b"WEAP", 0, RIFLE, &rifle);
    let packed = kitkraft_esm::compress::deflate(&packed_fields(), 9, 0).unwrap();
    weapons.extend(record(b"WEAP", 0x0004_0000, PACKED, &packed));
    out.extend(group(b"WEAP", 0, &weapons));
    out
}

/// Writes `bytes` to a per-process temp file and returns its path.
pub(crate) fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("kitkraft-engine-{name}-{}.esm", std::process::id()));
    std::fs::write(&path, bytes).expect("write temp plugin");
    path
}

/// An empty per-process temp directory (removed first if a previous run left it).
pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kitkraft-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
