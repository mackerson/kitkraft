//! The document model: a load order of plugins seen as one resolved data set.
//!
//! Resolves file-relative FormIDs across masters, tracks every override of a record and
//! the winning one, builds the conflict graph, and resolves localized strings.
//!
//! LAYER: L1
