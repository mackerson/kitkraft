//! Bethesda archive (`.ba2`) reading and writing: GNRL v2/v3 and DX10 textures.
//!
//! Port of `C:\Modding\scripts\ba2x.py` (our own code), extended with DX10 textures, the
//! v3 header, LZ4 and packing. Sources per ADR 0006: `ours: ba2x.py` for the header and
//! GNRL records, `corpus:` for everything else (inventory in `docs/corpus.md`,
//! "Archives"), `ms-docs:` for the DDS header.
//!
//! Reading never loads an archive whole: [`Archive::read`] parses the header, the records
//! and the name table, and [`Archive::extract_to`] streams one entry. Every offset and size
//! from the file is checked against the file length before it is used.
//!
//! LAYER: L0

pub mod dds;
mod error;
mod extract;
pub mod hash;
mod read;
mod write;

pub use error::{Error, Result};
pub use extract::{normalize, relative_path, LZ4_MAX_BLOB};
pub use read::{
    Archive, Body, Chunk, Codec, Entry, General, Header, Kind, Texture, CHUNK_RECORD_LEN, FILLER,
    GENERAL_RECORD_LEN, MAGIC, TEXTURE_RECORD_LEN,
};
pub use write::{chunk_mips, pack, PackOptions, PackReport, Packed, GENERAL_FLAGS, TILE_MODE};

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests;
