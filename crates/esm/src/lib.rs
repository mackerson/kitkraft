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
//! Format source: UESP's Skyrim pages (`Skyrim_Mod:Mod_File_Format`,
//! `Skyrim_Mod:Mod_File_Format/TES4`) for the shared container, confirmed against our own
//! corpus; facts UESP does not state for Starfield are taken from the corpus and say so
//! where they are used (`plan/adr/0005-spec-sources-per-fact.md`).
//!
//! # Public API at a glance
//!
//! - Whole plugin in memory: [`Plugin::parse`], [`Plugin::to_bytes`] / [`Plugin::write_to`];
//!   the tree is [`Entry`] / [`Group`] / [`Record`] / [`Subrecord`], edited through the
//!   `*_mut` accessors (an edit marks the path to it modified; everything else is written
//!   back byte for byte).
//! - Streaming over a file, one top-level group at a time: [`driver::roundtrip_file`],
//!   [`driver::visit_file`], [`driver::rewrite_file`].
//! - Record index and random access (M2.A1, M2.A2):
//!   [`index::index_file`]`(path, &IndexOptions) -> Result<PluginIndex>` gives every record
//!   as a [`index::RecordLoc`] (signature, FormID, flags, form version, offset, data size,
//!   enclosing group, EditorID) and every group as a [`index::GroupLoc`], without building
//!   the tree; [`index::read_record_at`]`(&mut file, offset, decode) -> Result<Record<'static>>`
//!   and [`index::read_indexed`]`(&mut file, &RecordLoc, decode)` (which also checks the
//!   header against the entry) read one record back.
//! - Building and placing records (M2.A5): [`Plugin::create`]`(&HeaderSpec)` makes a new
//!   plugin ([`build::HeaderSpec`]: raw flags, `HEDR`, author, masters as given);
//!   [`Plugin::upsert`]`(&[GroupHeader], Record) -> Result<Upsert>` replaces the record
//!   with the same FormID in the group the path names, or adds it, creating missing
//!   groups (top-level ones in [`build::TOP_GROUP_ORDER`]);
//!   [`build::upsert_file_to_path`]`(in, out, &RewriteOptions, Vec<Placed>, header_edit)`
//!   does the same while streaming an existing plugin into a new file, `HEDR` count
//!   patched. [`build::set_masters`] rewrites a `TES4` master list.
//! - Cell, world and quest groups (M4.A1, M4.A2): [`cells`] states the corpus rules and
//!   computes placement paths: [`cells::placement`]`(&RecordHeader, &Parent, &PlacementStyle)
//!   -> Result<Vec<GroupHeader>>` for a record under a cell (interior, a world's persistent
//!   cell, or an exterior cell at a grid), a quest or a topic; [`Plugin::upsert_styled`]
//!   places it in corpus order.
//! - New forms and placed references (M4.A3..A5): [`forms::NewForms`] allocates own
//!   FormIDs of a full-scale plugin and advances `HEDR`; [`refs::compose_reference`]
//!   builds a `REFR`/`ACHR` from a [`refs::RefSpec`] with a confidence gate, and
//!   [`refs::ComposedRef::placed`] gives its computed path; [`navmesh::import_navmeshes`]
//!   copies a CK-saved plugin's navmesh unchanged.
//! - Editing one record (M2.A6), compressed or not (decoded on first edit under
//!   [`DecodeLimits`], recompressed only if its bytes changed):
//!   [`Record::find_subrecord`]`(sig, nth)`, [`Record::set_subrecord`]`(index, data,
//!   &limits) -> Result<bool>`, [`Record::insert_subrecord`]`(index, Subrecord, &limits)`,
//!   [`Record::remove_subrecord`]`(index, &limits)`, [`Record::edit_subrecords`]`(&limits)`.
//!   A changed payload gets an `XXXX` prefix exactly when it does not fit a `u16`; an
//!   untouched subrecord keeps its bytes.
//! - Header: [`tes4::Tes4`] (flags, `HEDR`, masters, `ONAM`), the flag constants, and
//!   [`Plugin::hedr_count_on_write`].
//! - Typed records: [`bridge`] connects records to the generated types in [`generated`].
//!
//! LAYER: L0

pub mod bridge;
pub mod build;
pub mod cells;
pub mod compress;
pub mod driver;
mod edit;
mod error;
pub mod forms;
/// Typed records generated from `crates/schema/defs/` by `cargo xtask codegen` (do not edit).
pub mod generated;
pub mod index;
mod model;
pub mod navmesh;
mod parse;
mod reader;
pub mod refs;
mod signature;
pub mod tes4;
mod write;

#[cfg(test)]
mod fixture;

pub use compress::DecodeLimits;
pub use error::{Error, Result};
pub use model::{
    Body, Counts, Entry, Group, GroupHeader, Inflated, Plugin, Record, RecordHeader, Subrecord,
    FLAG_COMPRESSED, HEADER_LEN, SUBRECORD_HEADER_LEN,
};
pub use parse::{ParseOptions, MAX_DEPTH_CAP};
pub use reader::Reader;
pub use signature::Signature;
pub use write::WriteOptions;
