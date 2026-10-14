//! Record definitions as data.
//!
//! Every Starfield record type (WEAP, NPC_, CELL, ...) is described in TOML under
//! `crates/schema/defs/`: its subrecords, their order and repeat rules, and their field
//! layouts. `cargo xtask codegen` turns those files into typed Rust records (ADR 0003).
//! Definitions record facts (signatures, types, sizes, offsets) sourced from the UESP wiki
//! and our own corpus; they never copy UESP prose.
//!
//! The file format is documented in `crates/schema/defs/README.md`. Use [`load_sources`]
//! or [`load_dir`] to load a set; both validate everything before returning a [`Schema`].
//!
//! This crate has no knowledge of the plugin container. `kitcraft-esm` depends on it, not
//! the other way round.
//!
//! LAYER: L0

mod load;
mod model;
pub mod names;

pub use load::{load_dir, load_sources, parse_definition, ErrorKind, SchemaError, MAX_DEPTH};
pub use model::{
    AnyUntilDef, ArrayCount, Cardinality, Definition, FieldDef, FieldType, GroupDef, HeaderDef,
    Member, NamedValue, RecordDef, Scalar, Schema, Signature, SubrecordDef, VersionedSize,
};

/// The checked-in definition files (`crates/schema/defs/*.toml`), embedded at build time.
/// A test keeps this list in sync with the directory.
pub const BUILTIN_DEFINITIONS: &[(&str, &str)] = &[
    ("GLOB.toml", include_str!("../defs/GLOB.toml")),
    ("GMST.toml", include_str!("../defs/GMST.toml")),
    ("GRUP.toml", include_str!("../defs/GRUP.toml")),
    ("KYWD.toml", include_str!("../defs/KYWD.toml")),
    ("TES4.toml", include_str!("../defs/TES4.toml")),
    ("WEAP.toml", include_str!("../defs/WEAP.toml")),
];

/// Loads and validates the embedded definition set.
pub fn builtin() -> Result<Schema, SchemaError> {
    load_sources(BUILTIN_DEFINITIONS.iter().copied())
}
