//! Record definitions as data.
//!
//! Every Starfield record type (WEAP, NPC_, CELL, ...) is described in TOML under
//! `crates/schema/defs/`: its subrecords, their order and repeat rules, and their field
//! layouts. `cargo xtask codegen` turns those files into typed Rust records (ADR 0003).
//! Definitions record facts (signatures, types, sizes, offsets) sourced from the UESP wiki
//! and our own corpus; they never copy UESP prose.
//!
//! This crate has no knowledge of the plugin container. `kitcraft-esm` depends on it, not
//! the other way round.
//!
//! LAYER: L0
