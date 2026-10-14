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
