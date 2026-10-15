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

/// One zlib stream holding `data` in stored (uncompressed) deflate blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = data.chunks(0xFFFF).collect();
    for (i, b) in blocks.iter().enumerate() {
        out.push(u8::from(i + 1 == blocks.len()));
        let len = u16::try_from(b.len()).unwrap();
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

/// A Starfield save (`BCPS` container, one chunk) whose header lists these plugins, in the
/// layout `kitkraft_io::save` reads (`docs/load-order.md`, "Save headers"): full, small and
/// medium plugins, each list in slot order.
pub(crate) fn save(full: &[&str], small: &[&str], medium: &[&str]) -> Vec<u8> {
    let entry = |name: &&str| {
        let mut out = u16::try_from(name.len()).unwrap().to_le_bytes().to_vec();
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        out
    };
    let mut block = vec![u8::try_from(full.len()).unwrap()];
    block.extend(full.iter().flat_map(entry));
    block.extend_from_slice(&u16::try_from(small.len()).unwrap().to_le_bytes());
    block.extend(small.iter().flat_map(entry));
    block.extend_from_slice(&u32::try_from(medium.len()).unwrap().to_le_bytes());
    block.extend(medium.iter().flat_map(entry));
    block.extend_from_slice(&[1, 0]);
    let mut data = b"SFS_SAVEGAME".to_vec();
    data.extend_from_slice(&8u32.to_le_bytes());
    data.extend_from_slice(&27u32.to_le_bytes());
    data.extend_from_slice(&[0; 4]);
    data.push(153);
    for _ in 0..2 {
        data.extend_from_slice(&10u16.to_le_bytes());
        data.extend_from_slice(b"1.16.244.0");
    }
    data.extend_from_slice(&u32::try_from(block.len()).unwrap().to_le_bytes());
    data.extend(block);
    let z = zlib_stored(&data);
    let mut out = b"BCPS".to_vec();
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0x48u64.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&0x50u64.to_le_bytes());
    out.extend_from_slice(&(data.len() as u64).to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&0x40000u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&16u64.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(b"ZIP ");
    out.extend_from_slice(&u32::try_from(z.len()).unwrap().to_le_bytes());
    out.resize(0x50, 0);
    out.extend(z);
    out
}
