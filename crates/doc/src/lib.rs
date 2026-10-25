//! The document model: a load order of plugins seen as one resolved data set.
//!
//! Resolves file-relative FormIDs across masters, tracks every override of a record and
//! the winning one, builds the conflict graph, and resolves localized strings.
//!
//! Raw FormIDs (as serialized in one file) and resolved identities ([`GlobalForm`]) are
//! different types on purpose: resolving needs the master graph (`form` module docs), and
//! a missing master is an error, never a guess. Facts and their sources:
//! `docs/load-order.md`.
//!
//! LAYER: L1

pub mod backrefs;
pub mod cache;
pub mod compare;
pub mod confidence;
pub mod conflicts;
pub mod diff;
mod error;
pub mod fields;
pub mod flags;
pub mod form;
pub mod hash;
pub mod index;
pub mod links;
pub mod load_order;
pub mod lstring;
pub mod overrides;
pub mod query;
pub mod record;
pub mod refl;
pub mod search;
pub mod source;
pub mod strings;
pub mod subject;
#[cfg(test)]
pub(crate) mod testdata;
pub mod typed;
pub mod vmad;

pub use cache::{CacheUse, IndexCache};
pub use error::{DocError, Result};
pub use flags::{PluginFlags, Scale};
pub use form::{
    Basis, FormRef, GlobalForm, MasterMap, RawRef, ResolveError, RuntimeSlot, SlotInput,
};
pub use index::{HeaderInfo, PluginIndex, RecordLoc};
pub use load_order::{
    LoadOptions, LoadOrder, LoadedPlugin, MasterLink, OnPluginDone, PluginDone, PluginSpec,
    RuntimeBasis,
};
pub use overrides::{Duplicate, FormTable, Version};
pub use record::{LoadedRecord, RecordReader};
pub use source::SourceCheck;
