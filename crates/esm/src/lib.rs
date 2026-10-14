//! TES4 plugin container for Starfield `.esm` / `.esp` / `.esl` files.
//!
//! Reads and writes the byte-level structure: file header (TES4, masters, ONAM),
//! groups, records, subrecords (including `XXXX` oversize prefixes), zlib-compressed
//! records and the localized-strings flag. The M1 contract is a byte-identical
//! roundtrip of every official ESM; subrecords without a schema definition are kept as
//! opaque bytes and are never dropped.
//!
//! Input bytes are hostile: every length and offset is bounds-checked before use.
//!
//! Format source: the UESP wiki. Starfield's page defers to the Skyrim container layout
//! (`Skyrim_Mod:Mod_File_Format`, `Skyrim_Mod:Mod_File_Format/TES4`); facts UESP does not
//! state for Starfield are taken from our own corpus and say so where they are used.
//!
//! LAYER: L0

pub mod compress;
pub mod driver;
mod error;
mod model;
mod parse;
mod reader;
mod signature;
pub mod tes4;
mod write;

#[cfg(test)]
mod fixture;

pub use compress::DecodeLimits;
pub use error::{Error, Result};
pub use model::{
    Body, Entry, Group, GroupHeader, Plugin, Record, RecordHeader, Subrecord, FLAG_COMPRESSED,
    HEADER_LEN, SUBRECORD_HEADER_LEN,
};
pub use parse::ParseOptions;
pub use reader::Reader;
pub use signature::Signature;
pub use write::WriteOptions;
