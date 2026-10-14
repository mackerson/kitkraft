//! Packing files into a new archive.
//!
//! The layout follows the official archives (`corpus:`, `docs/corpus.md` "Archives"):
//! header, every record, the data of every file in record order with no padding, then the
//! name table. Names are stored lower-cased with `/` separators, as in every official
//! archive, and sorted, so the same inputs always give the same bytes. Refused: non-ASCII
//! names (every official name is ASCII and Archive2 hashes others by a rule not yet
//! established), two names with the same hash triple, and empty files (Archive2 skips
//! them; callers should too). GNRL records carry
//! flags `0x00100100`; DX10 records carry tile mode 8 and split mips into chunks the way
//! the official archives do: each mip with at least 512 x 512 pixels (`width * height >=
//! 262,144`, so a 1024x256 mip qualifies) gets its own chunk while fewer than three chunks
//! exist and it is not the last mip; the remaining mips share one last chunk (at most four
//! chunks). Cube maps are one chunk. `black-box: Archive2.exe 1.2.0.1` prints the 512 and
//! four-chunk defaults in its usage text; the area reading is `corpus:` (every official
//! texture's chunk plan recomputes from its width, height, mip count and cube flag,
//! `tests/corpus.rs` `check_textures`) and matches what Archive2 packs.
//!
//! Version 2 compresses with zlib, version 3 with LZ4 blocks (method 3). A blob is stored
//! when compression is off or does not make it smaller.

use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::dds;
use crate::error::{io, Error, Result};
use crate::extract::{normalize, relative_path};
use crate::hash::record_hashes;
use crate::read::{Codec, Kind, FILLER, MAGIC};

/// GNRL record flags in every official archive.
pub const GENERAL_FLAGS: u32 = 0x0010_0100;
/// DX10 tile mode in every official archive.
pub const TILE_MODE: u8 = 8;
/// Fewest pixels (512 x 512) of a mip that gets its own chunk.
const OWN_CHUNK_MIN_AREA: u64 = 512 * 512;
/// Most chunks per texture.
const MAX_CHUNKS: usize = 4;

/// How to pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackOptions {
    /// Archive kind.
    pub kind: Kind,
    /// 2 (zlib) or 3 (LZ4 block).
    pub version: u32,
    /// Compress blobs (kept only when smaller).
    pub compress: bool,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            kind: Kind::General,
            version: 2,
            compress: true,
        }
    }
}

/// One packed file, for the caller's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packed {
    /// Archive path as stored.
    pub name: String,
    /// Input size.
    pub size: u64,
    /// Bytes in the archive.
    pub stored: u64,
}

/// What [`pack`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackReport {
    /// Files in archive order.
    pub files: Vec<Packed>,
    /// Archive length.
    pub bytes: u64,
}

struct Plan {
    name: String,
    input: String,
    tex: Option<(dds::Parsed, Vec<(u32, u32)>)>,
}

/// Packs `inputs` (archive path, then anything `load` understands, usually a file path)
/// into `out`. `load` is called once per file for GNRL and twice for DX10 (the first pass
/// sizes the chunk records).
pub fn pack<W, F>(
    inputs: &[(String, String)],
    mut load: F,
    options: &PackOptions,
    out: &mut W,
) -> Result<PackReport>
where
    W: Write + Seek,
    F: FnMut(&str) -> std::io::Result<Vec<u8>>,
{
    let codec = match options.version {
        2 => Codec::Zlib,
        3 => Codec::Lz4Block,
        v => {
            return Err(Error::Pack {
                name: String::new(),
                reason: format!("version {v}: only 2 (zlib) and 3 (LZ4) are written"),
            })
        }
    };
    let pack_err = |name: &str, reason: String| Error::Pack {
        name: name.to_string(),
        reason,
    };
    let count = u32::try_from(inputs.len())
        .map_err(|_| pack_err("", format!("{} files is too many", inputs.len())))?;

    let mut plans: Vec<Plan> = Vec::with_capacity(inputs.len());
    for (name, input) in inputs {
        let stored = normalize(name);
        relative_path(&stored)?;
        if !stored.is_ascii() {
            // Archive2 hashes non-ASCII names differently from the CRC rule every official
            // (all-ASCII) name follows; refused until that rule is established.
            return Err(pack_err(
                name,
                "non-ASCII archive paths are not supported (their name hash is unknown)".into(),
            ));
        }
        if stored.len() > usize::from(u16::MAX) {
            return Err(pack_err(name, "name is longer than 65,535 bytes".into()));
        }
        plans.push(Plan {
            name: stored,
            input: input.clone(),
            tex: None,
        });
    }
    plans.sort_by(|a, b| a.name.cmp(&b.name));
    for pair in plans.windows(2) {
        if let [a, b] = pair {
            if a.name == b.name {
                return Err(pack_err(
                    &a.name,
                    "two inputs have this archive path".into(),
                ));
            }
        }
    }
    // The game finds a file by (name hash, extension, directory hash); two names with the
    // same triple (`a.dlstrings` and `a.dlstrongs` share the extension field `dlst`) would
    // shadow each other.
    let mut triples: HashMap<(u32, [u8; 4], u32), &str> = HashMap::with_capacity(plans.len());
    for plan in &plans {
        if let Some(other) = triples.insert(record_hashes(plan.name.as_bytes()), &plan.name) {
            return Err(pack_err(
                &plan.name,
                format!("`{other}` has the same name hash, extension field and directory hash"),
            ));
        }
    }

    let header_len: u64 = if options.version == 3 { 36 } else { 32 };
    let mut table_len = 0u64;
    for plan in &mut plans {
        let record = match options.kind {
            Kind::General => 36,
            Kind::Textures => {
                let bytes = load(&plan.input).map_err(io(0))?;
                let parsed = dds::parse(&bytes).map_err(|e| pack_err(&plan.name, e))?;
                let chunks = chunk_plan(&parsed, &bytes).map_err(|e| pack_err(&plan.name, e))?;
                let len = 24u64.saturating_add(24u64.saturating_mul(chunks.len() as u64));
                plan.tex = Some((parsed, chunks));
                len
            }
        };
        table_len = table_len.saturating_add(record);
    }

    let data_start = header_len.saturating_add(table_len);
    out.seek(SeekFrom::Start(data_start))
        .map_err(io(data_start))?;
    let mut pos = data_start;
    let mut records: Vec<u8> = Vec::new();
    let mut report = Vec::with_capacity(plans.len());
    for plan in &plans {
        let bytes = load(&plan.input).map_err(io(0))?;
        if bytes.is_empty() {
            // `black-box: Archive2.exe 1.2.0.1` skips zero-length files ("Skipped
            // zero-length file") and no official record has size 0; callers skip them.
            return Err(pack_err(&plan.name, "the file is empty".into()));
        }
        let (name_hash, ext, dir_hash) = record_hashes(plan.name.as_bytes());
        records.extend_from_slice(&name_hash.to_le_bytes());
        records.extend_from_slice(&ext);
        records.extend_from_slice(&dir_hash.to_le_bytes());
        let mut stored_total = 0u64;
        match &plan.tex {
            None => {
                let size = u32::try_from(bytes.len())
                    .map_err(|_| pack_err(&plan.name, "larger than 4 GiB".into()))?;
                let (data, packed) = encode(&bytes, codec, options.compress, &plan.name)?;
                out.write_all(&data).map_err(io(pos))?;
                records.extend_from_slice(&GENERAL_FLAGS.to_le_bytes());
                records.extend_from_slice(&pos.to_le_bytes());
                records.extend_from_slice(&packed.to_le_bytes());
                records.extend_from_slice(&size.to_le_bytes());
                records.extend_from_slice(&FILLER.to_le_bytes());
                stored_total = data.len() as u64;
                pos = pos.saturating_add(stored_total);
            }
            Some((parsed, chunks)) => {
                let fit = |v: u32, what: &str| {
                    u16::try_from(v).map_err(|_| pack_err(&plan.name, format!("{what} {v}")))
                };
                let chunk_count = u8::try_from(chunks.len()).unwrap_or(u8::MAX);
                records.push(0);
                records.push(chunk_count);
                records.extend_from_slice(&24u16.to_le_bytes());
                records.extend_from_slice(&fit(parsed.height, "height")?.to_le_bytes());
                records.extend_from_slice(&fit(parsed.width, "width")?.to_le_bytes());
                records.push(
                    u8::try_from(parsed.mips)
                        .map_err(|_| pack_err(&plan.name, "more than 255 mips".into()))?,
                );
                records.push(parsed.format);
                records.push(u8::from(parsed.cubemap));
                records.push(TILE_MODE);
                let pixels = bytes.get(parsed.data_offset..).unwrap_or_default();
                let mut at = 0usize;
                for (i, &(first, last)) in chunks.iter().enumerate() {
                    let len = chunk_len(parsed, first, last)
                        .ok_or_else(|| pack_err(&plan.name, format!("chunk {i} size")))?;
                    let end = at.saturating_add(len);
                    let raw = pixels
                        .get(at..end)
                        .ok_or_else(|| pack_err(&plan.name, "pixel data too short".into()))?;
                    at = end;
                    let size = u32::try_from(raw.len())
                        .map_err(|_| pack_err(&plan.name, "chunk larger than 4 GiB".into()))?;
                    let (data, packed) = encode(raw, codec, options.compress, &plan.name)?;
                    out.write_all(&data).map_err(io(pos))?;
                    records.extend_from_slice(&pos.to_le_bytes());
                    records.extend_from_slice(&packed.to_le_bytes());
                    records.extend_from_slice(&size.to_le_bytes());
                    records.extend_from_slice(&fit(first, "mip")?.to_le_bytes());
                    records.extend_from_slice(&fit(last, "mip")?.to_le_bytes());
                    records.extend_from_slice(&FILLER.to_le_bytes());
                    stored_total = stored_total.saturating_add(data.len() as u64);
                    pos = pos.saturating_add(data.len() as u64);
                }
            }
        }
        report.push(Packed {
            name: plan.name.clone(),
            size: bytes.len() as u64,
            stored: stored_total,
        });
    }

    let name_table = pos;
    for plan in &plans {
        let bytes = plan.name.as_bytes();
        let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
        out.write_all(&len.to_le_bytes()).map_err(io(pos))?;
        out.write_all(bytes).map_err(io(pos))?;
        pos = pos.saturating_add(2).saturating_add(bytes.len() as u64);
    }

    let mut header = Vec::with_capacity(36);
    header.extend_from_slice(&MAGIC);
    header.extend_from_slice(&options.version.to_le_bytes());
    header.extend_from_slice(&options.kind.tag());
    header.extend_from_slice(&count.to_le_bytes());
    header.extend_from_slice(&name_table.to_le_bytes());
    header.extend_from_slice(&1u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    if options.version == 3 {
        header.extend_from_slice(&3u32.to_le_bytes());
    }
    out.seek(SeekFrom::Start(0)).map_err(io(0))?;
    out.write_all(&header).map_err(io(0))?;
    out.write_all(&records).map_err(io(header_len))?;
    out.flush().map_err(io(pos))?;
    Ok(PackReport {
        files: report,
        bytes: pos,
    })
}

/// Compressed bytes and the packed-size field (0 when stored).
fn encode(raw: &[u8], codec: Codec, compress: bool, name: &str) -> Result<(Vec<u8>, u32)> {
    if !compress || raw.is_empty() {
        return Ok((raw.to_vec(), 0));
    }
    let packed = match codec {
        Codec::Zlib => {
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
            enc.write_all(raw).map_err(io(0))?;
            enc.finish().map_err(io(0))?
        }
        Codec::Lz4Block => lz4_flex::block::compress(raw),
    };
    if packed.len() >= raw.len() {
        return Ok((raw.to_vec(), 0));
    }
    let len = u32::try_from(packed.len()).map_err(|_| Error::Pack {
        name: name.to_string(),
        reason: "compressed data larger than 4 GiB".into(),
    })?;
    Ok((packed, len))
}

/// Mip ranges per chunk; checks the pixel data has exactly the mip chain's size.
fn chunk_plan(parsed: &dds::Parsed, bytes: &[u8]) -> std::result::Result<Vec<(u32, u32)>, String> {
    let faces = if parsed.cubemap { 6 } else { 1 };
    let want = dds::chain_len(
        parsed.format,
        parsed.width,
        parsed.height,
        parsed.mips,
        faces,
    )
    .ok_or("mip chain size overflows")?;
    let have = bytes.len().saturating_sub(parsed.data_offset) as u64;
    if have != want {
        return Err(format!(
            "pixel data is {have} bytes, the mip chain of {}x{} with {} mips needs {want}",
            parsed.width, parsed.height, parsed.mips
        ));
    }
    Ok(chunk_mips(
        parsed.width,
        parsed.height,
        parsed.mips,
        parsed.cubemap,
    ))
}

/// The official chunk plan for a texture: `(first mip, last mip)` per chunk (module doc).
pub fn chunk_mips(width: u32, height: u32, mips: u32, cubemap: bool) -> Vec<(u32, u32)> {
    let last = mips.max(1).saturating_sub(1);
    if cubemap {
        return vec![(0, last)];
    }
    let mut chunks = Vec::new();
    let (mut w, mut h) = (width.max(1), height.max(1));
    let mut mip = 0u32;
    while mip < last
        && chunks.len() < MAX_CHUNKS.saturating_sub(1)
        && u64::from(w).saturating_mul(u64::from(h)) >= OWN_CHUNK_MIN_AREA
    {
        chunks.push((mip, mip));
        mip = mip.saturating_add(1);
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    chunks.push((mip, last));
    chunks
}

/// Bytes of mips `first..=last` (one face).
fn chunk_len(parsed: &dds::Parsed, first: u32, last: u32) -> Option<usize> {
    let faces = if parsed.cubemap { 6u64 } else { 1 };
    let mut total = 0u64;
    for mip in first..=last {
        let w = parsed.width.checked_shr(mip).unwrap_or(0).max(1);
        let h = parsed.height.checked_shr(mip).unwrap_or(0).max(1);
        total = total.checked_add(dds::mip_len(parsed.format, w, h)?)?;
    }
    usize::try_from(total.checked_mul(faces)?).ok()
}
