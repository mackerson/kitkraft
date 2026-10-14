//! The data-driven patch builder's library half: the spec format ([`spec`], versions 1
//! and 2) and what the schema says about member paths ([`members`]: unknown members, the
//! `reference` confidence gate, typed numeric members). The load-order half (matching
//! classes, following `via`, writing and validating the plugin) is `handlers/build.rs`.

pub mod members;
pub mod spec;
