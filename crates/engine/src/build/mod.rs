//! The data-driven patch builder's library half: the spec format ([`spec`], versions 1
//! and 2), what the schema says about member paths ([`members`]: unknown members, the
//! `reference` confidence gate, typed numeric members), and the `overhaul.scaffold`
//! renderer ([`scaffold`]). The load-order half (matching classes, following `via`,
//! writing and validating the plugin, grouping records for a scaffold) is
//! `handlers/build.rs` and `handlers/overhaul.rs`.

pub mod members;
pub mod scaffold;
pub mod spec;
