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
//! LAYER: L0
