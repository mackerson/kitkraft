//! The block bodies this crate decodes: the shared prefix of node and geometry blocks, the
//! children list of node blocks, and the bound of `BSGeometry` blocks.
//!
//! Layouts from our own corpus study (`docs/research/nif-bounds-2026-11-02.md`, claims B1
//! to B6), each checked on every such block of the 120,547 official models.

use crate::error::{Error, Result};
use crate::read::{Cur, Nif};

/// The block types whose body is the shared prefix followed by a children list (claim B3).
/// `NiNode` ends right after the list; the others carry more bytes after it.
pub const NODE_TYPES: [&str; 8] = [
    "NiNode",
    "BSFaceGenNiNode",
    "BSWeakReferenceNode",
    "NiBillboardNode",
    "NiSwitchNode",
    "BSBlastNode",
    "BSOrderedNode",
    "BSValueNode",
];

/// The block type that carries a bound (claim B4).
pub const GEOMETRY_TYPE: &str = "BSGeometry";

/// `f32::MAX`, the value every box component holds when no box is stored (claim B5).
const NO_BOX: f32 = f32::MAX;

/// Placement of a block in its parent's frame: `p' = scale * rotation * p + translation`,
/// with `rotation` stored row by row (claims B2 and C1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Three `f32`.
    pub translation: [f32; 3],
    /// Nine `f32`, row by row.
    pub rotation: [[f32; 3]; 3],
    /// One `f32`; positive in every official block.
    pub scale: f32,
}

impl Transform {
    /// The transform that changes nothing.
    pub const IDENTITY: Transform = Transform {
        translation: [0.0; 3],
        rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        scale: 1.0,
    };
}

/// The prefix shared by node and geometry blocks (claim B2).
#[derive(Debug, Clone, PartialEq)]
pub struct Prefix {
    /// Index into [`Nif::strings`], or none.
    pub name: Option<u32>,
    /// References to the block's extra-data blocks.
    pub extra_data: Vec<u32>,
    /// The `u32` after the extra-data list, 0xFFFFFFFF in every official block; any other
    /// value is refused.
    pub after_extra: u32,
    /// The next `u32`, not interpreted (values such as 0x0000000E and 0x2000000F).
    pub opaque: u32,
    /// Placement in the parent's frame.
    pub transform: Transform,
    /// Reference to the block's collision object, or none.
    pub collision: Option<u32>,
}

/// A node block: the prefix and its children (claim B3).
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The shared prefix.
    pub prefix: Prefix,
    /// Child references; a null entry is kept as `None`.
    pub children: Vec<Option<u32>>,
    /// Bytes after the children list (0 for every official `NiNode`).
    pub trailing: usize,
}

/// A sphere: centre and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    /// Centre.
    pub center: [f32; 3],
    /// Radius.
    pub radius: f32,
}

/// An axis-aligned box as centre and half extents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CenteredBox {
    /// Centre.
    pub center: [f32; 3],
    /// Half the size along each axis.
    pub half_extent: [f32; 3],
}

/// What a geometry block's bound fields hold (claim B5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bound {
    /// A sphere and a box, in the block's own frame.
    Stored {
        /// The sphere.
        sphere: Sphere,
        /// The box.
        aabb: CenteredBox,
    },
    /// A sphere; the six box values are all `f32::MAX`.
    SphereOnly(Sphere),
    /// No bound: a zero sphere and the six box values all `f32::MAX` (every official
    /// skinned geometry, and only those).
    NotStored,
    /// Values outside the three patterns above (no official geometry has them).
    Unrecognised,
}

/// A geometry block: the prefix, the bound and the references after it (claims B4, B6).
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    /// The shared prefix.
    pub prefix: Prefix,
    /// The bound.
    pub bound: Bound,
    /// File offset of the 40 bound bytes, for evidence.
    pub bound_offset: usize,
    /// Reference to a `BSSkin::Instance`, or none.
    pub skin: Option<u32>,
    /// Reference to the shader property block, or none.
    pub shader: Option<u32>,
    /// Reference to an `NiAlphaProperty`, or none.
    pub alpha: Option<u32>,
}

impl Nif<'_> {
    fn checked_ref(&self, c: &mut Cur<'_>, what: &'static str) -> Result<Option<u32>> {
        let at = c.file_pos();
        let v = c.u32(what)?;
        self.reference(v, at, what)
    }

    fn prefix(&self, c: &mut Cur<'_>) -> Result<Prefix> {
        let name_at = c.offset();
        let name = c.u32("name")?;
        let name = if name == u32::MAX {
            None
        } else if usize::try_from(name).map_or(true, |n| n >= self.strings.len()) {
            return Err(Error::OutOfRange {
                what: "name string index",
                offset: name_at,
                value: u64::from(name),
            });
        } else {
            Some(name)
        };
        let count = c.u32("extra data count")?;
        let mut extra_data = Vec::with_capacity(c.cap(u64::from(count), 4));
        for _ in 0..count {
            let at = c.file_pos();
            let v = c.u32("extra data reference")?;
            match self.reference(v, at, "extra data reference")? {
                Some(r) => extra_data.push(r),
                None => {
                    return Err(Error::OutOfRange {
                        what: "extra data reference",
                        offset: at as u64,
                        value: u64::from(v),
                    })
                }
            }
        }
        let after_at = c.offset();
        let after_extra = c.u32("u32 after the extra data")?;
        if after_extra != u32::MAX {
            return Err(Error::Unsupported {
                what: "u32 after the extra data",
                offset: after_at,
                value: u64::from(after_extra),
            });
        }
        let opaque = c.u32("u32 after that")?;
        let translation = c.f32s::<3>("translation")?;
        let r = c.f32s::<9>("rotation")?;
        let scale = c.f32("scale")?;
        let collision = self.checked_ref(c, "collision reference")?;
        let [r0, r1, r2, r3, r4, r5, r6, r7, r8] = r;
        Ok(Prefix {
            name,
            extra_data,
            after_extra,
            opaque,
            transform: Transform {
                translation,
                rotation: [[r0, r1, r2], [r3, r4, r5], [r6, r7, r8]],
                scale,
            },
            collision,
        })
    }

    /// Block `index` as a node, or `None` when its type is not one of [`NODE_TYPES`].
    pub fn node(&self, index: usize) -> Result<Option<Node>> {
        let Some(t) = self.type_name(index) else {
            return Ok(None);
        };
        if !NODE_TYPES.contains(&t) {
            return Ok(None);
        }
        let Some(mut c) = self.cursor(index) else {
            return Ok(None);
        };
        let prefix = self.prefix(&mut c)?;
        let count = c.u32("children count")?;
        let mut children = Vec::with_capacity(c.cap(u64::from(count), 4));
        for _ in 0..count {
            children.push(self.checked_ref(&mut c, "child reference")?);
        }
        Ok(Some(Node {
            prefix,
            children,
            trailing: c.remaining(),
        }))
    }

    /// Block `index` as a geometry block, or `None` when it is not a [`GEOMETRY_TYPE`].
    pub fn geometry(&self, index: usize) -> Result<Option<Geometry>> {
        if self.type_name(index) != Some(GEOMETRY_TYPE) {
            return Ok(None);
        }
        let Some(mut c) = self.cursor(index) else {
            return Ok(None);
        };
        let prefix = self.prefix(&mut c)?;
        let bound_offset = c.file_pos();
        let [cx, cy, cz, radius] = c.f32s::<4>("bounding sphere")?;
        let [bx, by, bz, hx, hy, hz] = c.f32s::<6>("bounding box")?;
        let sphere = Sphere {
            center: [cx, cy, cz],
            radius,
        };
        let no_box = [bx, by, bz, hx, hy, hz].iter().all(|v| *v == NO_BOX);
        let finite = [cx, cy, cz, radius].iter().all(|v| v.is_finite());
        let bound = if no_box {
            if [cx, cy, cz, radius].iter().all(|v| *v == 0.0) {
                Bound::NotStored
            } else if finite && radius >= 0.0 {
                Bound::SphereOnly(sphere)
            } else {
                Bound::Unrecognised
            }
        } else if finite
            && radius >= 0.0
            && [bx, by, bz, hx, hy, hz].iter().all(|v| v.is_finite())
            && [hx, hy, hz].iter().all(|v| *v >= 0.0)
        {
            Bound::Stored {
                sphere,
                aabb: CenteredBox {
                    center: [bx, by, bz],
                    half_extent: [hx, hy, hz],
                },
            }
        } else {
            Bound::Unrecognised
        };
        let skin = self.checked_ref(&mut c, "skin reference")?;
        let shader = self.checked_ref(&mut c, "shader reference")?;
        let alpha = self.checked_ref(&mut c, "alpha reference")?;
        Ok(Some(Geometry {
            prefix,
            bound,
            bound_offset,
            skin,
            shader,
            alpha,
        }))
    }
}
