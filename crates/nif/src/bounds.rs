//! A model's box: the union of its geometry bounds, carried up the node tree to the root's
//! frame (claim C1, `docs/research/nif-bounds-2026-11-02.md`).

use crate::block::{Bound, Transform};
use crate::error::Result;
use crate::read::Nif;

/// An axis-aligned box as two corners.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Smallest x, y, z.
    pub min: [f32; 3],
    /// Largest x, y, z.
    pub max: [f32; 3],
}

/// One geometry block's contribution, for evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryBound {
    /// Block index.
    pub block: u32,
    /// File offset of its 40 bound bytes.
    pub bound_offset: usize,
    /// What it holds.
    pub bound: Bound,
    /// The block's frame to the root's, row by row: `p' = m * p + t` as `[m | t]`
    /// (the block's own transform, then its ancestors'; claim C1).
    pub to_root: [[f64; 4]; 3],
}

/// The box of a whole model, in the root block's frame, and what went into it.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelBounds {
    /// The union of every reached geometry's box (or its sphere's box when only a sphere
    /// is stored); `None` when no reached geometry has a bound.
    pub aabb: Option<Aabb>,
    /// Every geometry block reached from the root, in visiting order.
    pub geometries: Vec<GeometryBound>,
    /// Geometry blocks not reached from any root (none in the official models).
    pub unreached_geometries: u32,
    /// Children reached a second time, ignored (none in the official models).
    pub revisits: u32,
    /// The union does not fit in `f32` (huge placements); `aabb` is then `None`.
    pub box_overflow: bool,
}

impl ModelBounds {
    /// How many reached geometries hold each kind of bound: stored, sphere only, not
    /// stored, unrecognised.
    pub fn counts(&self) -> [u32; 4] {
        let mut out = [0u32; 4];
        for g in &self.geometries {
            let slot = match g.bound {
                Bound::Stored { .. } => out.get_mut(0),
                Bound::SphereOnly(_) => out.get_mut(1),
                Bound::NotStored => out.get_mut(2),
                Bound::Unrecognised => out.get_mut(3),
            };
            if let Some(n) = slot {
                *n = n.saturating_add(1);
            }
        }
        out
    }
}

/// An affine map in `f64`: `p' = m * p + t`.
#[derive(Debug, Clone, Copy)]
struct Affine {
    m: [[f64; 3]; 3],
    t: [f64; 3],
}

impl Affine {
    const IDENTITY: Affine = Affine {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0; 3],
    };

    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let [r0, r1, r2] = self.m;
        let [tx, ty, tz] = self.t;
        [dot(r0, p) + tx, dot(r1, p) + ty, dot(r2, p) + tz]
    }

    /// `self` after `local`: a point in the child's frame, carried to `self`'s frame.
    fn then(&self, local: &Transform) -> Affine {
        let s = f64::from(local.scale);
        let r = local.rotation.map(|row| row.map(|v| f64::from(v) * s));
        let cols = [
            [r[0][0], r[1][0], r[2][0]],
            [r[0][1], r[1][1], r[2][1]],
            [r[0][2], r[1][2], r[2][2]],
        ];
        let m = self.m.map(|row| cols.map(|col| dot(row, col)));
        let t = self.apply(local.translation.map(f64::from));
        Affine { m, t }
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Nif<'_> {
    /// The model's box. Walks from the root through node children; each geometry's
    /// bound is in its own frame, so its own transform applies, then each ancestor's.
    /// Each block is visited at most once, so the walk is bounded by the block count.
    pub fn model_bounds(&self) -> Result<ModelBounds> {
        let mut visited = vec![false; self.blocks.len()];
        let mut stack: Vec<(usize, Affine)> = Vec::new();
        if let Ok(r) = usize::try_from(self.root) {
            stack.push((r, Affine::IDENTITY));
        }
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        let mut geometries = Vec::new();
        let mut revisits = 0u32;
        while let Some((index, parent)) = stack.pop() {
            match visited.get_mut(index) {
                Some(seen) if *seen => {
                    revisits = revisits.saturating_add(1);
                    continue;
                }
                Some(seen) => *seen = true,
                None => continue,
            }
            if let Some(node) = self.node(index)? {
                let here = parent.then(&node.prefix.transform);
                for child in node.children.iter().rev().flatten() {
                    if let Ok(child) = usize::try_from(*child) {
                        stack.push((child, here));
                    }
                }
            } else if let Some(g) = self.geometry(index)? {
                let here = parent.then(&g.prefix.transform);
                let boxed = match g.bound {
                    Bound::Stored { aabb, .. } => Some((aabb.center, aabb.half_extent)),
                    Bound::SphereOnly(s) => Some((s.center, [s.radius; 3])),
                    Bound::NotStored | Bound::Unrecognised => None,
                };
                if let Some((c, h)) = boxed {
                    for corner in 0..8u8 {
                        let pick = |axis: u8, i: usize| {
                            let c = f64::from(c.get(i).copied().unwrap_or(0.0));
                            let h = f64::from(h.get(i).copied().unwrap_or(0.0));
                            if corner & (1 << axis) == 0 {
                                c - h
                            } else {
                                c + h
                            }
                        };
                        let p = here.apply([pick(0, 0), pick(1, 1), pick(2, 2)]);
                        for ((l, u), v) in lo.iter_mut().zip(hi.iter_mut()).zip(p) {
                            *l = l.min(v);
                            *u = u.max(v);
                        }
                    }
                }
                let [m0, m1, m2] = here.m;
                let [t0, t1, t2] = here.t;
                geometries.push(GeometryBound {
                    block: u32::try_from(index).unwrap_or(u32::MAX),
                    bound_offset: g.bound_offset,
                    bound: g.bound,
                    to_root: [
                        [m0[0], m0[1], m0[2], t0],
                        [m1[0], m1[1], m1[2], t1],
                        [m2[0], m2[1], m2[2], t2],
                    ],
                });
            }
        }
        let unreached_geometries = (0..self.blocks.len())
            .filter(|i| {
                self.type_name(*i) == Some(crate::block::GEOMETRY_TYPE)
                    && !visited.get(*i).copied().unwrap_or(false)
            })
            .count();
        let mut box_overflow = false;
        let aabb = if lo.iter().zip(hi.iter()).all(|(l, u)| l <= u) {
            let (min, max) = (lo.map(|v| v as f32), hi.map(|v| v as f32));
            if min.iter().chain(max.iter()).all(|v| v.is_finite()) {
                Some(Aabb { min, max })
            } else {
                box_overflow = true;
                None
            }
        } else {
            None
        };
        Ok(ModelBounds {
            aabb,
            geometries,
            unreached_geometries: u32::try_from(unreached_geometries).unwrap_or(u32::MAX),
            revisits,
            box_overflow,
        })
    }
}
