//! DDS headers for DX10 texture records, and DXGI format sizes.
//!
//! Structure and constants: `ms-docs: DDS_HEADER structure`, `DDS_PIXELFORMAT`,
//! `DDS_HEADER_DXT10`, "Programming Guide for DDS" (legacy FourCCs and the D3D9 format
//! numbers used as FourCCs). Where the documentation allows several encodings of one
//! format, we follow what `black-box: Archive2.exe 1.2.0.1` writes when it extracts the
//! official archives (observed per DXGI format on `Starfield - Textures01/05.ba2`,
//! `SFBGS004 - Textures.ba2`, `OldMars - Textures.ba2`; `plan/STATUS.md`):
//!
//! - depth 1; caps `TEXTURE`, plus `MIPMAP | COMPLEX` with more than one mip and
//!   `COMPLEX` plus all six cube faces in caps2 for a cube map;
//! - block formats: linear size of the top mip; `DXT1` (71), `DXT5` (77), `BC4U` (80) and
//!   `BC5S` (84) as legacy FourCCs, every other block format through the DX10 extension;
//! - uncompressed: pitch; RGBA (28) and BGRA (87) as 32-bit masks, SNORM RGBA (31) as
//!   bump-du/dv masks, R8 (61) as 8-bit luminance, RGBA16 float (10) and unorm (11) as the
//!   numeric FourCCs 113 and 36; others through the DX10 extension.
//!
//! Formats absent from the official archives use the DX10 extension, which the
//! documentation allows for every DXGI format.
//!
//! Data layout after the header (`corpus:`): for a 2D texture, mips from largest to
//! smallest; for a cube map (`cubemap` = 1), six faces, each with its full mip chain.

use crate::read::Texture;

/// `DDS ` magic.
pub const MAGIC: [u8; 4] = *b"DDS ";
/// Size of `DDS_HEADER`.
const HEADER_SIZE: u32 = 124;
/// Size of `DDS_PIXELFORMAT`.
const PIXELFORMAT_SIZE: u32 = 32;

const DDSD_CAPS: u32 = 0x1;
const DDSD_HEIGHT: u32 = 0x2;
const DDSD_WIDTH: u32 = 0x4;
const DDSD_PITCH: u32 = 0x8;
const DDSD_PIXELFORMAT: u32 = 0x1000;
const DDSD_MIPMAPCOUNT: u32 = 0x2_0000;
const DDSD_LINEARSIZE: u32 = 0x8_0000;
const DDPF_FOURCC: u32 = 0x4;
const DDSCAPS_COMPLEX: u32 = 0x8;
const DDSCAPS_TEXTURE: u32 = 0x1000;
const DDSCAPS_MIPMAP: u32 = 0x40_0000;
const DDSCAPS2_CUBEMAP_ALLFACES: u32 = 0xFE00;
const DIMENSION_TEXTURE2D: u32 = 3;
const MISC_TEXTURECUBE: u32 = 0x4;

/// How a DXGI format stores pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// 4x4 blocks of this many bytes.
    Block(u32),
    /// This many bits per pixel.
    Bits(u32),
}

/// Storage layout of a DXGI format (`ms-docs: DXGI_FORMAT`), or `None` for a format this
/// crate cannot size (packed 4:2:2, 1-bit, video formats).
pub fn layout(format: u8) -> Option<Layout> {
    Some(match format {
        1..=4 => Layout::Bits(128),
        5..=8 => Layout::Bits(96),
        9..=22 => Layout::Bits(64),
        23..=47 | 67 | 87..=93 => Layout::Bits(32),
        48..=59 | 85 | 86 | 115 => Layout::Bits(16),
        60..=65 => Layout::Bits(8),
        70..=72 | 79..=81 => Layout::Block(8),
        73..=78 | 82..=84 | 94..=99 => Layout::Block(16),
        _ => return None,
    })
}

/// Bytes of one mip level, or `None` when the format is not sized.
pub fn mip_len(format: u8, width: u32, height: u32) -> Option<u64> {
    let w = u64::from(width.max(1));
    let h = u64::from(height.max(1));
    match layout(format)? {
        Layout::Block(bytes) => {
            let bw = w.div_ceil(4).max(1);
            let bh = h.div_ceil(4).max(1);
            bw.checked_mul(bh)?.checked_mul(u64::from(bytes))
        }
        Layout::Bits(bits) => w.checked_mul(u64::from(bits))?.div_ceil(8).checked_mul(h),
    }
}

/// Bytes of the whole texture (every face, every mip), or `None` when not sized.
pub fn chain_len(format: u8, width: u32, height: u32, mips: u32, faces: u32) -> Option<u64> {
    let mut total = 0u64;
    let (mut w, mut h) = (width, height);
    for _ in 0..mips.max(1) {
        total = total.checked_add(mip_len(format, w, h)?)?;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    total.checked_mul(u64::from(faces))
}

/// Number of faces: 6 for a cube map.
pub fn faces(texture: &Texture) -> u32 {
    if texture.cubemap != 0 {
        6
    } else {
        1
    }
}

const DDPF_ALPHAPIXELS: u32 = 0x1;
const DDPF_RGB: u32 = 0x40;
const DDPF_LUMINANCE: u32 = 0x2_0000;
const DDPF_BUMPDUDV: u32 = 0x8_0000;
const RGBA_MASKS: [u32; 4] = [0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000];
const BGRA_MASKS: [u32; 4] = [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000];

/// A `DDS_PIXELFORMAT` without the DX10 extension: flags, FourCC, bit count, masks.
type Legacy = (u32, [u8; 4], u32, [u32; 4]);

/// The legacy pixel format Archive2 writes for `format`, if it writes one.
fn legacy(format: u8) -> Option<Legacy> {
    let fourcc = |code: [u8; 4]| Some((DDPF_FOURCC, code, 0, [0; 4]));
    match format {
        10 => fourcc(113u32.to_le_bytes()),
        11 => fourcc(36u32.to_le_bytes()),
        71 => fourcc(*b"DXT1"),
        77 => fourcc(*b"DXT5"),
        80 => fourcc(*b"BC4U"),
        84 => fourcc(*b"BC5S"),
        28 => Some((DDPF_RGB | DDPF_ALPHAPIXELS, [0; 4], 32, RGBA_MASKS)),
        87 => Some((DDPF_RGB | DDPF_ALPHAPIXELS, [0; 4], 32, BGRA_MASKS)),
        31 => Some((DDPF_BUMPDUDV, [0; 4], 32, RGBA_MASKS)),
        61 => Some((DDPF_LUMINANCE, [0; 4], 8, [0xFF, 0, 0, 0])),
        _ => None,
    }
}

/// Length of the synthesized header: 128, or 148 with the DX10 extension.
pub fn header_len(texture: &Texture) -> u64 {
    if legacy(texture.format).is_some() {
        128
    } else {
        148
    }
}

/// The DDS header (magic included) for a texture record.
pub fn header(texture: &Texture) -> Vec<u8> {
    let width = u32::from(texture.width);
    let height = u32::from(texture.height);
    let blocky = matches!(layout(texture.format), Some(Layout::Block(_)));
    let pitch = match layout(texture.format) {
        Some(Layout::Block(_)) => mip_len(texture.format, width, height),
        Some(Layout::Bits(bits)) => u64::from(width)
            .checked_mul(u64::from(bits))
            .map(|b| b.div_ceil(8)),
        None => Some(0),
    }
    .and_then(|p| u32::try_from(p).ok())
    .unwrap_or(0);
    let flags = DDSD_CAPS
        | DDSD_HEIGHT
        | DDSD_WIDTH
        | DDSD_PIXELFORMAT
        | DDSD_MIPMAPCOUNT
        | if blocky { DDSD_LINEARSIZE } else { DDSD_PITCH };
    let cube = texture.cubemap != 0;
    let legacy = legacy(texture.format);

    let mut out = Vec::with_capacity(148);
    out.extend_from_slice(&MAGIC);
    for v in [
        HEADER_SIZE,
        flags,
        height,
        width,
        pitch,
        1, // depth
        u32::from(texture.mip_count),
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0u8; 44]); // dwReserved1[11]
    out.extend_from_slice(&PIXELFORMAT_SIZE.to_le_bytes());
    let (pf_flags, fourcc, bits, masks) = legacy.unwrap_or((DDPF_FOURCC, *b"DX10", 0, [0; 4]));
    out.extend_from_slice(&pf_flags.to_le_bytes());
    out.extend_from_slice(&fourcc);
    out.extend_from_slice(&bits.to_le_bytes());
    for m in masks {
        out.extend_from_slice(&m.to_le_bytes());
    }
    let mut caps = DDSCAPS_TEXTURE;
    if texture.mip_count > 1 {
        caps |= DDSCAPS_MIPMAP | DDSCAPS_COMPLEX;
    }
    if cube {
        caps |= DDSCAPS_COMPLEX;
    }
    let caps2 = if cube { DDSCAPS2_CUBEMAP_ALLFACES } else { 0 };
    for v in [caps, caps2, 0, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    if legacy.is_none() {
        let misc = if cube { MISC_TEXTURECUBE } else { 0 };
        for v in [
            u32::from(texture.format),
            DIMENSION_TEXTURE2D,
            misc,
            1, // array size
            0, // alpha mode
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// What [`parse`] reads from a DDS file for packing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parsed {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Mip count (at least 1).
    pub mips: u32,
    /// DXGI format.
    pub format: u8,
    /// Whether it is a cube map.
    pub cubemap: bool,
    /// Offset of the pixel data.
    pub data_offset: usize,
}

/// Reads a DDS header. Legacy FourCCs map to DXGI per `ms-docs: Programming Guide for DDS`.
pub fn parse(bytes: &[u8]) -> Result<Parsed, String> {
    let u32_at = |at: usize| -> Result<u32, String> {
        bytes
            .get(at..at.saturating_add(4))
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| format!("truncated DDS header at byte {at}"))
    };
    if bytes.get(..4) != Some(&MAGIC[..]) {
        return Err("missing `DDS ` magic".into());
    }
    if u32_at(4)? != HEADER_SIZE {
        return Err("DDS_HEADER size is not 124".into());
    }
    let height = u32_at(12)?;
    let width = u32_at(16)?;
    let mips = u32_at(28)?.max(1);
    let pf_flags = u32_at(80)?;
    let fourcc = bytes
        .get(84..88)
        .ok_or_else(|| "truncated DDS header".to_string())?;
    let caps2 = u32_at(112)?;
    let mut cubemap = caps2 & 0x200 != 0;
    let (format, data_offset) = if pf_flags & DDPF_FOURCC != 0 && fourcc == b"DX10" {
        let format = u32_at(128)?;
        let misc = u32_at(136)?;
        cubemap |= misc & MISC_TEXTURECUBE != 0;
        let format = u8::try_from(format).map_err(|_| format!("DXGI format {format}"))?;
        (format, 148)
    } else if pf_flags & DDPF_FOURCC != 0 {
        let format = match fourcc {
            b"DXT1" => 71,
            b"DXT2" | b"DXT3" => 74,
            b"DXT4" | b"DXT5" => 77,
            b"BC4U" | b"ATI1" => 80,
            b"BC4S" => 81,
            b"BC5U" | b"ATI2" => 83,
            b"BC5S" => 84,
            [113, 0, 0, 0] => 10,
            [36, 0, 0, 0] => 11,
            other => {
                return Err(format!(
                    "unsupported FourCC {:?}",
                    String::from_utf8_lossy(other)
                ))
            }
        };
        (format, 128)
    } else {
        // Uncompressed legacy pixel formats: the ones `header` writes.
        let bits = u32_at(88)?;
        let rmask = u32_at(92)?;
        let rgb = pf_flags & DDPF_RGB != 0;
        match (bits, rmask) {
            (32, 0x0000_00FF) if rgb => (28, 128),
            (32, 0x00FF_0000) if rgb => (87, 128),
            (32, 0x0000_00FF) if pf_flags & DDPF_BUMPDUDV != 0 => (31, 128),
            (8, 0xFF) if pf_flags & DDPF_LUMINANCE != 0 => (61, 128),
            _ => return Err(format!("unsupported pixel format ({bits} bits)")),
        }
    };
    if layout(format).is_none() {
        return Err(format!("DXGI format {format} cannot be sized"));
    }
    Ok(Parsed {
        width,
        height,
        mips,
        format,
        cubemap,
        data_offset,
    })
}

#[cfg(test)]
mod tests {
    use super::{chain_len, header, mip_len, parse};
    use crate::read::Texture;

    fn texture(format: u8, width: u16, height: u16, mips: u8, cubemap: u8) -> Texture {
        Texture {
            unknown: 0,
            chunk_record_len: 24,
            height,
            width,
            mip_count: mips,
            format,
            cubemap,
            tile_mode: 8,
            chunks: Vec::new(),
        }
    }

    #[test]
    fn mip_sizes_match_corpus_examples() {
        // corpus: Constellation - Textures.ba2, 1024x1024 BC4 with 11 mips in 3 chunks.
        assert_eq!(mip_len(80, 1024, 1024), Some(524_288));
        assert_eq!(chain_len(80, 1024, 1024, 11, 1), Some(699_064));
        // corpus: Starfield - Textures05.ba2, a 744x744 BC7 cube map with 10 mips.
        assert_eq!(chain_len(99, 744, 744, 10, 6), Some(4_437_600));
        // corpus: SFBGS004 - Textures.ba2, 3840x2160 BC7 with one mip.
        assert_eq!(chain_len(98, 3840, 2160, 1, 1), Some(8_294_400));
        assert_eq!(mip_len(28, 16, 256), Some(16_384));
        assert_eq!(mip_len(200, 4, 4), None);
    }

    #[test]
    fn header_shapes() {
        let legacy = header(&texture(80, 1024, 1024, 11, 0));
        assert_eq!(legacy.len(), 128);
        assert_eq!(legacy.get(84..88), Some(&b"BC4U"[..]));
        let dx10 = header(&texture(72, 512, 256, 10, 0));
        assert_eq!(dx10.len(), 148);
        let parsed = parse(&dx10).unwrap();
        assert_eq!(
            (parsed.width, parsed.height, parsed.mips, parsed.format),
            (512, 256, 10, 72)
        );
        assert!(!parsed.cubemap);
        let cube = parse(&header(&texture(10, 64, 64, 7, 1))).unwrap();
        assert!(cube.cubemap);
        assert_eq!(parse(&legacy).unwrap().format, 80);
        // Every format in the official archives parses back from the header we write.
        for format in [
            10u8, 11, 28, 29, 31, 61, 71, 72, 77, 78, 80, 84, 87, 95, 98, 99, 83, 2,
        ] {
            for (mips, cube) in [(1u8, 0u8), (5, 0), (1, 1), (5, 1)] {
                let h = header(&texture(format, 64, 32, mips, cube));
                assert_eq!(
                    h.len() as u64,
                    super::header_len(&texture(format, 64, 32, mips, cube))
                );
                let p = parse(&h).unwrap_or_else(|e| panic!("{format}: {e}"));
                assert_eq!(
                    (p.format, p.width, p.height, p.mips, p.cubemap),
                    (format, 64, 32, u32::from(mips), cube == 1),
                    "format {format}"
                );
            }
        }
        assert!(parse(b"DDS ").is_err());
        assert!(parse(b"nope").is_err());
    }
}
