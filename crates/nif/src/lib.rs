//! Read-only NIF model metadata: the block structure of a Starfield `.nif` and the bounds
//! its geometry blocks store, and the `.mesh` geometry they name. No rendering.
//!
//! Every layout fact here comes from our own study of the official models, read in place
//! from the game's archives and `Data` folder, with each claim checked on every official
//! model: `docs/research/nif-bounds-2026-11-02.md` (M7.S4). No other description of the
//! format was read (`docs/source-registry.md`, "Sources for NIF and mesh layout").
//!
//! - [`Nif::parse`] reads the header, the block-type, block-size and string tables and the
//!   footer, and checks that the blocks fill the file exactly (claims H1 to H6).
//! - [`Nif::node`] and [`Nif::geometry`] decode the shared block prefix (placement), node
//!   children, and a geometry block's bound (claims B1 to B6).
//! - [`Nif::model_bounds`] carries every geometry bound up the node tree to the root and
//!   returns their union (claim C1), with each geometry's transform to the root.
//! - [`Nif::slots`] and [`Mesh::parse`] read the slots a geometry block names its meshes in
//!   and a `.mesh` up to positions, indices and (grade B) normals
//!   (`docs/research/mesh-layout-2026-11-02.md`, M7.S5).
//!
//! LAYER: L0

mod block;
mod bounds;
mod error;
mod mesh;
mod read;

pub use block::{
    Bound, CenteredBox, Geometry, Node, Prefix, Sphere, Transform, GEOMETRY_TYPE, NODE_TYPES,
};
pub use bounds::{Aabb, GeometryBound, ModelBounds};
pub use error::{Error, Result};
pub use mesh::{Mesh, MeshOffsets, Slot, MESH_VERSION};
pub use read::{Block, Header, Nif, VERSION, VERSION_LINE};

#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod tests;
