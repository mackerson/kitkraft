//! What the viewport draws, as plain data: the ground grid, the world axes, the marker
//! shapes, the markers themselves and the scene colors. No GL here (`super::gl` uploads
//! and draws it), so the geometry, the placement math and picking are unit-tested.
//!
//! Units are the game's (`super::camera`, Z up). Starfield stores positions in meters
//! (an exterior cell is 100 units wide: the corpus' exterior cells span 100 units, M7.R2),
//! so the grid is metric: lines every [`GRID_STEP`] grid unit, drawn scaled by a power of
//! ten that follows the camera ([`grid_step_for`]).
//!
//! A placed reference becomes one [`Marker`] (M7.R2): its `DATA` position and rotation and
//! its `XSCL` scale place either its base's object bounds (`OBND`, M7.R3) or, when the base
//! has none, a small shape that says its type. [`ref_rotation`] pins how the rotation
//! angles turn it, from corpus evidence.

use std::ops::{Add, Mul, Sub};

use glam::{EulerRot, Mat4, Quat, Vec3};

/// Grid spacing in grid units (the grid's model matrix scales it to world units).
pub const GRID_STEP: f32 = 1.0;
/// Every `GRID_MAJOR`th line is a major line.
pub const GRID_MAJOR: i32 = 10;
/// Grid lines run from `GRID_FIRST` to `GRID_LAST` steps.
pub const GRID_FIRST: i32 = -50;
/// See [`GRID_FIRST`].
pub const GRID_LAST: i32 = 50;
/// Half the grid's width in grid units (`GRID_LAST` steps).
pub const GRID_EXTENT: f32 = 50.0;
/// Length of the +Z axis line, in grid units.
pub const Z_AXIS: f32 = 10.0;

/// Half the size of the shape drawn for a reference whose base has no usable bounds, in
/// game units (a 0.5 m shape).
pub const UNIT_HALF: f32 = 0.25;
/// The smallest half extent a bounds box is drawn with (a flat decal's `OBND` has zero
/// thickness), in game units.
pub const MIN_HALF: f32 = 0.005;

/// One vertex of a line: position and RGBA color (0..1, the window's gamma space).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

/// One vertex of a lit triangle: position and outward face normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
}

/// Floats per [`LineVertex`] and per [`SolidVertex`], as uploaded.
pub const LINE_FLOATS: usize = 7;
/// See [`LINE_FLOATS`].
pub const SOLID_FLOATS: usize = 6;
/// Components per attribute, in location order (the shaders' `attributes`).
pub const LINE_LAYOUT: [i32; 2] = [3, 4];
/// See [`LINE_LAYOUT`].
pub const SOLID_LAYOUT: [i32; 2] = [3, 3];
/// Floats per marker instance: the model matrix (16, column-major) and the fill color.
pub const INSTANCE_FLOATS: usize = 20;

/// Flattens line vertices for upload: position then color.
pub fn line_floats(v: &[LineVertex]) -> Vec<f32> {
    v.iter()
        .flat_map(|v| v.pos.into_iter().chain(v.color))
        .collect()
}

/// Flattens solid vertices for upload: position then normal.
pub fn solid_floats(v: &[SolidVertex]) -> Vec<f32> {
    v.iter()
        .flat_map(|v| v.pos.into_iter().chain(v.normal))
        .collect()
}

/// The scene's colors (RGBA, 0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneColors {
    /// Sky straight up, at the horizon, and the ground tint below the horizon.
    pub zenith: [f32; 4],
    pub horizon: [f32; 4],
    pub ground: [f32; 4],
    pub grid_minor: [f32; 4],
    pub grid_major: [f32; 4],
    pub axis_x: [f32; 4],
    pub axis_y: [f32; 4],
    pub axis_z: [f32; 4],
    /// Marker outlines in the solid face mode (the fill is each marker's own color).
    pub edge: [f32; 4],
    /// The selected marker's outline.
    pub select: [f32; 4],
}

impl SceneColors {
    /// For a dark window.
    pub const DARK: Self = Self {
        zenith: [0.09, 0.11, 0.15, 1.0],
        horizon: [0.32, 0.35, 0.40, 1.0],
        ground: [0.12, 0.12, 0.13, 1.0],
        grid_minor: [0.62, 0.65, 0.70, 0.22],
        grid_major: [0.78, 0.81, 0.86, 0.55],
        axis_x: [0.93, 0.33, 0.30, 1.0],
        axis_y: [0.45, 0.82, 0.38, 1.0],
        axis_z: [0.38, 0.58, 0.98, 1.0],
        edge: [0.08, 0.06, 0.04, 1.0],
        select: [1.0, 1.0, 1.0, 1.0],
    };
    /// For a light window.
    pub const LIGHT: Self = Self {
        zenith: [0.58, 0.69, 0.82, 1.0],
        horizon: [0.88, 0.90, 0.92, 1.0],
        ground: [0.66, 0.66, 0.64, 1.0],
        grid_minor: [0.20, 0.22, 0.26, 0.22],
        grid_major: [0.12, 0.14, 0.18, 0.50],
        axis_x: [0.80, 0.16, 0.14, 1.0],
        axis_y: [0.16, 0.56, 0.14, 1.0],
        axis_z: [0.14, 0.32, 0.80, 1.0],
        edge: [0.10, 0.08, 0.05, 1.0],
        select: [0.0, 0.0, 0.0, 1.0],
    };

    /// The set for the window's mode.
    pub fn for_mode(dark: bool) -> Self {
        if dark {
            Self::DARK
        } else {
            Self::LIGHT
        }
    }
}

/// The ground grid on the Z = 0 plane, in grid units: lines every [`GRID_STEP`] parallel
/// to X and to Y, major every [`GRID_MAJOR`] steps. The lines through the origin are
/// drawn too (the world axes are separate and may sit elsewhere).
pub fn grid_lines(colors: &SceneColors) -> Vec<LineVertex> {
    let mut out = Vec::new();
    for i in GRID_FIRST..=GRID_LAST {
        let color = if i.checked_rem(GRID_MAJOR) == Some(0) {
            colors.grid_major
        } else {
            colors.grid_minor
        };
        let at = i as f32 * GRID_STEP;
        for (a, b) in [
            ([at, -GRID_EXTENT, 0.0], [at, GRID_EXTENT, 0.0]),
            ([-GRID_EXTENT, at, 0.0], [GRID_EXTENT, at, 0.0]),
        ] {
            out.push(LineVertex { pos: a, color });
            out.push(LineVertex { pos: b, color });
        }
    }
    out
}

/// The world axes from the origin, in grid units: +X red, +Y green across the grid (their
/// negative halves dimmed), and +Z blue up to [`Z_AXIS`].
pub fn axis_lines(colors: &SceneColors) -> Vec<LineVertex> {
    let dim = |c: [f32; 4]| [c[0], c[1], c[2], 0.35];
    let o = [0.0, 0.0, 0.0];
    let line = |a: [f32; 3], b: [f32; 3], color: [f32; 4]| {
        [LineVertex { pos: a, color }, LineVertex { pos: b, color }]
    };
    [
        line(o, [GRID_EXTENT, 0.0, 0.0], colors.axis_x),
        line(o, [-GRID_EXTENT, 0.0, 0.0], dim(colors.axis_x)),
        line(o, [0.0, GRID_EXTENT, 0.0], colors.axis_y),
        line(o, [0.0, -GRID_EXTENT, 0.0], dim(colors.axis_y)),
        line(o, [0.0, 0.0, Z_AXIS], colors.axis_z),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The grid step in world units for a camera `distance` from its target: a power of ten
/// about a tenth of the distance (1 m lines from 3 m to 30 m away), so some ten lines
/// span the view at any zoom.
pub fn grid_step_for(distance: f32) -> f32 {
    if !distance.is_finite() || distance <= 0.0 {
        return 1.0;
    }
    10f32.powf((distance / 10.0).log10().round().clamp(-3.0, 6.0))
}

/// The grid's model matrix: scaled by `step`, centered under `target` on a multiple of a
/// major line (so lines do not swim while panning), at height `floor`.
pub fn grid_model(step: f32, target: Vec3, floor: f32) -> Mat4 {
    let major = step * GRID_MAJOR as f32;
    let snap = |v: f32| {
        if v.is_finite() && major > 0.0 {
            (v / major).round() * major
        } else {
            0.0
        }
    };
    let floor = if floor.is_finite() { floor } else { 0.0 };
    Mat4::from_scale_rotation_translation(
        Vec3::splat(step),
        Quat::IDENTITY,
        Vec3::new(snap(target.x), snap(target.y), floor),
    )
}

/// The shape a marker is drawn with: the bounds box, or, for a reference without bounds,
/// one of three shapes, so the type shows by shape as well as by color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shape {
    Box,
    /// An octahedron.
    Diamond,
    /// A square pyramid, apex up.
    Pyramid,
}

impl Shape {
    /// Every shape, in draw order (the GL side keeps one instance buffer per shape).
    pub const ALL: [Shape; 3] = [Shape::Box, Shape::Diamond, Shape::Pyramid];

    /// Its index in [`Shape::ALL`].
    pub fn index(self) -> usize {
        match self {
            Shape::Box => 0,
            Shape::Diamond => 1,
            Shape::Pyramid => 2,
        }
    }

    /// Its triangles inside the -1..1 cube, counter-clockwise seen from outside.
    pub fn faces(self) -> Vec<SolidVertex> {
        match self {
            Shape::Box => unit_box_faces(),
            Shape::Diamond => diamond_faces(),
            Shape::Pyramid => pyramid_faces(),
        }
    }

    /// Its edges, as line vertex positions (pairs).
    pub fn edges(self) -> Vec<[f32; 3]> {
        match self {
            Shape::Box => unit_box_edges(),
            Shape::Diamond => {
                let axes = [Vec3::X, Vec3::Y, Vec3::Z];
                let mut out = Vec::with_capacity(24);
                for (i, a) in axes.iter().enumerate() {
                    for b in axes.iter().skip(i.saturating_add(1)) {
                        for (sa, sb) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                            out.push(a.mul(sa).to_array());
                            out.push(b.mul(sb).to_array());
                        }
                    }
                }
                out
            }
            Shape::Pyramid => {
                let base = pyramid_base();
                let apex = [0.0, 0.0, 1.0];
                let mut out = Vec::with_capacity(16);
                for (i, c) in base.iter().enumerate() {
                    let next = base.get(i.saturating_add(1) % 4).copied().unwrap_or(*c);
                    out.push(*c);
                    out.push(next);
                    out.push(*c);
                    out.push(apex);
                }
                out
            }
        }
    }
}

/// One triangle with its face normal from the winding.
fn tri(out: &mut Vec<SolidVertex>, a: Vec3, b: Vec3, c: Vec3) {
    let n = b.sub(a).cross(c.sub(a)).normalize_or(Vec3::Z).to_array();
    for p in [a, b, c] {
        out.push(SolidVertex {
            pos: p.to_array(),
            normal: n,
        });
    }
}

/// The faces of the box from -1 to +1 on each axis: 12 triangles, counter-clockwise seen
/// from outside (so back faces can be culled), each vertex with its face's normal.
pub fn unit_box_faces() -> Vec<SolidVertex> {
    // Each face: its normal and two in-plane axes u, v with u x v = normal.
    const FACES: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    let mut out = Vec::with_capacity(36);
    for (n, u, v) in FACES {
        let (n3, u3, v3) = (Vec3::from(n), Vec3::from(u), Vec3::from(v));
        let corner = |su: f32, sv: f32| n3.add(u3.mul(su)).add(v3.mul(sv));
        let (a, b, c, d) = (
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        );
        tri(&mut out, a, b, c);
        tri(&mut out, a, c, d);
    }
    out
}

/// The 12 edges of the same box, as line vertex positions.
pub fn unit_box_edges() -> Vec<[f32; 3]> {
    let mut out = Vec::with_capacity(24);
    for (a, b) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, -1.0), (-1.0, 1.0)] {
        out.push([-1.0, a, b]);
        out.push([1.0, a, b]);
        out.push([a, -1.0, b]);
        out.push([a, 1.0, b]);
        out.push([a, b, -1.0]);
        out.push([a, b, 1.0]);
    }
    out
}

/// The octahedron with its six vertices on the axes at distance 1: 8 triangles.
fn diamond_faces() -> Vec<SolidVertex> {
    let mut out = Vec::with_capacity(24);
    for sx in [1.0f32, -1.0] {
        for sy in [1.0f32, -1.0] {
            for sz in [1.0f32, -1.0] {
                let (a, b, c) = (Vec3::X.mul(sx), Vec3::Y.mul(sy), Vec3::Z.mul(sz));
                // X, Y, Z wind outward in the all-positive octant; an odd number of
                // mirrored axes flips the winding.
                if sx * sy * sz > 0.0 {
                    tri(&mut out, a, b, c);
                } else {
                    tri(&mut out, a, c, b);
                }
            }
        }
    }
    out
}

/// The pyramid's base corners at z = -1, counter-clockwise seen from above.
fn pyramid_base() -> [[f32; 3]; 4] {
    [
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, -1.0],
    ]
}

/// The square pyramid: base at z = -1, apex at (0, 0, 1); 6 triangles.
fn pyramid_faces() -> Vec<SolidVertex> {
    let base = pyramid_base().map(Vec3::from);
    let apex = Vec3::Z;
    let mut out = Vec::with_capacity(18);
    for (i, c) in base.iter().enumerate() {
        let next = base.get(i.saturating_add(1) % 4).copied().unwrap_or(*c);
        tri(&mut out, *c, next, apex);
    }
    let [a, b, c, d] = base;
    // The base faces down: clockwise seen from above.
    tri(&mut out, a, c, b);
    tri(&mut out, a, d, c);
    out
}

/// A placed reference's rotation (`DATA` `Rotation`, radians, as stored) as a quaternion.
///
/// The rotation is applied as `Rx(-x) * Ry(-y) * Rz(-z)` (the stored angles turn
/// clockwise, Z applied first to the object). Evidence (corpus, M7.R2, 2026-10-16): of
/// the four candidates (each angle's sign, and X-then-Z or Z-then-X), this one puts the
/// most `OBND` box corners of turned references on another reference's corner, in each
/// of three Starfield.esm cells: `CityNewAtlantisApartmentTEMPLATE` (524 of 1,904 corners
/// against 443 to 512), NewAtlantis 2,1 (698 of 3,432 against 596 to 674) and akilacity
/// 1,1 (561 of 5,528 against 418 to 486); among references turned about X or Y alone it
/// also scores highest (27 of 280 and 368 of 936). Modular pieces meet at their corners, so
/// the right convention lines them up. Not yet checked against the CK's Render Window.
pub fn ref_rotation(r: Vec3) -> Quat {
    let r = if r.is_finite() { r } else { Vec3::ZERO };
    Quat::from_euler(EulerRot::XYZ, -r.x, -r.y, -r.z)
}

/// One shape placed in the scene: where it stands, how it is turned and scaled, the box it
/// fills in its own space and its fill color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    /// The reference's position, in game units.
    pub position: Vec3,
    /// The reference's rotation as stored (radians; [`ref_rotation`]).
    pub rotation: Vec3,
    /// Uniform scale (a `REFR`'s `XSCL`; 1 when absent).
    pub scale: f32,
    /// The shape's center in the reference's own space, before scaling (the middle of
    /// the base's `OBND`; zero for a shape without bounds).
    pub offset: Vec3,
    /// Half the shape's size along its own axes before scaling, in game units.
    pub half_extents: Vec3,
    pub shape: Shape,
    /// Fill color (RGBA, 0..1).
    pub color: [f32; 4],
}

impl Marker {
    /// The placeholder shown while no cell is loaded: a 4 x 2 x 1.3 m box turned 30
    /// degrees about Z (counter-clockwise from above), resting on the grid at the origin.
    pub fn placeholder() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 0.65),
            rotation: Vec3::new(0.0, 0.0, (-30f32).to_radians()),
            scale: 1.0,
            offset: Vec3::ZERO,
            half_extents: Vec3::new(2.0, 1.0, 0.65),
            shape: Shape::Box,
            color: [0.86, 0.60, 0.24, 1.0],
        }
    }

    /// A reference drawn as its base's bounds box (`OBND` corners `min` and `max`, in the
    /// base's own space), or as `shape` at [`UNIT_HALF`] when the bounds are absent, not
    /// finite, or empty.
    pub fn placed(
        position: Vec3,
        rotation: Vec3,
        scale: Option<f32>,
        bounds: Option<(Vec3, Vec3)>,
        shape: Shape,
        color: [f32; 4],
    ) -> Self {
        let scale = scale.filter(|s| s.is_finite() && *s > 0.0).unwrap_or(1.0);
        let usable = bounds.filter(|(a, b)| {
            a.is_finite() && b.is_finite() && b.sub(*a).abs().max_element() > 1e-4
        });
        let (offset, half_extents, shape) = match usable {
            Some((a, b)) => (
                a.add(b).mul(0.5),
                b.sub(a).abs().mul(0.5).max(Vec3::splat(MIN_HALF)),
                Shape::Box,
            ),
            None => (Vec3::ZERO, Vec3::splat(UNIT_HALF), shape),
        };
        Self {
            position,
            rotation,
            scale,
            offset,
            half_extents,
            shape,
            color,
        }
    }

    /// The model matrix: the -1..1 shape to world space, `T(position) * R * S(scale) *
    /// T(offset) * S(half_extents)`. Non-finite values fall back to identity parts, so a
    /// hostile value never reaches the GPU as NaN.
    pub fn model(&self) -> Mat4 {
        let finite = |v: Vec3, or: Vec3| if v.is_finite() { v } else { or };
        let scale = if self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        };
        let placed = Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            ref_rotation(self.rotation),
            finite(self.position, Vec3::ZERO),
        );
        let local = Mat4::from_scale_rotation_translation(
            finite(self.half_extents, Vec3::ONE).abs(),
            Quat::IDENTITY,
            finite(self.offset, Vec3::ZERO),
        );
        placed.mul_mat4(&local)
    }

    /// The eight corners of the marker's box in world space.
    pub fn corners(&self) -> [Vec3; 8] {
        let m = self.model();
        let mut out = [Vec3::ZERO; 8];
        for (i, slot) in out.iter_mut().enumerate() {
            let s = |bit: usize| if i & bit == 0 { -1.0 } else { 1.0 };
            *slot = m.transform_point3(Vec3::new(s(1), s(2), s(4)));
        }
        out
    }

    /// Where the ray from `origin` along `dir` enters the marker's box (the box around
    /// its shape), as the ray parameter `t >= 0`; `None` when it misses, or when the box
    /// holds the origin (the camera inside a room's box does not pick the room).
    pub fn hit(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        let inv = self.model().inverse();
        if !inv.is_finite() {
            return None;
        }
        let o = inv.transform_point3(origin);
        let d = inv.transform_vector3(dir);
        let (mut near, mut far) = (f32::NEG_INFINITY, f32::INFINITY);
        for (oc, dc) in [(o.x, d.x), (o.y, d.y), (o.z, d.z)] {
            if dc.abs() < 1e-12 {
                if oc.abs() > 1.0 {
                    return None;
                }
                continue;
            }
            let (a, b) = ((-1.0 - oc) / dc, (1.0 - oc) / dc);
            near = near.max(a.min(b));
            far = far.min(a.max(b));
        }
        (near <= far && near >= 0.0 && near.is_finite()).then_some(near)
    }

    /// The box's volume in world units (picking prefers the smallest box under the
    /// cursor, so a small object inside a large one can be clicked).
    pub fn volume(&self) -> f32 {
        let m = self.model();
        let [a, b, c] = [Vec3::X, Vec3::Y, Vec3::Z].map(|v| m.transform_vector3(v).length());
        8.0 * a * b * c
    }

    /// The marker as uploaded: its model matrix (column-major) and its color.
    pub fn instance(&self) -> [f32; INSTANCE_FLOATS] {
        let mut out = [0.0; INSTANCE_FLOATS];
        for (slot, v) in out
            .iter_mut()
            .zip(self.model().to_cols_array().into_iter().chain(self.color))
        {
            *slot = v;
        }
        out
    }
}

/// The marker under a ray: of the boxes the ray enters, the smallest; the nearest among
/// boxes of equal volume.
pub fn pick(markers: &[Marker], origin: Vec3, dir: Vec3) -> Option<usize> {
    let mut best: Option<(usize, f32, f32)> = None;
    for (i, m) in markers.iter().enumerate() {
        let Some(t) = m.hit(origin, dir) else {
            continue;
        };
        let v = m.volume();
        let better = match best {
            None => true,
            Some((_, bv, bt)) => v < bv || (v == bv && t < bt),
        };
        if better {
            best = Some((i, v, t));
        }
    }
    best.map(|(i, _, _)| i)
}

/// The world box around every marker's corners; `None` when there are none.
pub fn bounds_of(markers: &[Marker]) -> Option<(Vec3, Vec3)> {
    let mut it = markers
        .iter()
        .flat_map(Marker::corners)
        .filter(|c| c.is_finite());
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), c| (lo.min(c), hi.max(c))))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn grid_is_flat_symmetric_and_marks_major_lines() {
        let g = grid_lines(&SceneColors::DARK);
        // 101 lines per direction, two vertices each.
        assert_eq!(g.len(), 101 * 2 * 2);
        assert!(g.iter().all(|v| v.pos[2] == 0.0));
        assert!(g
            .iter()
            .all(|v| v.pos[0].abs() <= GRID_EXTENT && v.pos[1].abs() <= GRID_EXTENT));
        let majors: Vec<_> = g
            .iter()
            .filter(|v| v.color == SceneColors::DARK.grid_major)
            .collect();
        // -50, -40, ..., 50: 11 lines per direction, two vertices.
        assert_eq!(majors.len(), 11 * 2 * 2);
        assert!(majors
            .iter()
            .all(|v| v.pos[0] % 10.0 == 0.0 && v.pos[1] % 10.0 == 0.0));
        assert_eq!(GRID_LAST as f32 * GRID_STEP, GRID_EXTENT);
        assert_eq!(line_floats(&g).len(), g.len() * LINE_FLOATS);
        assert_eq!(LINE_LAYOUT.iter().sum::<i32>() as usize, LINE_FLOATS);
        assert_eq!(SOLID_LAYOUT.iter().sum::<i32>() as usize, SOLID_FLOATS);
    }

    #[test]
    fn the_grid_follows_the_camera_in_powers_of_ten() {
        assert_eq!(grid_step_for(10.0), 1.0);
        assert_eq!(grid_step_for(29.0), 1.0);
        assert_eq!(grid_step_for(40.0), 10.0);
        assert_eq!(grid_step_for(2.0), 0.1);
        assert_eq!(grid_step_for(f32::NAN), 1.0);
        let m = grid_model(10.0, Vec3::new(240.0, 160.0, 7.0), 3.0);
        // Snapped to the 100-unit major lines, at the floor's height, scaled by the step.
        assert_eq!(m.transform_point3(Vec3::ZERO), Vec3::new(200.0, 200.0, 3.0));
        assert_eq!(m.transform_vector3(Vec3::X), Vec3::new(10.0, 0.0, 0.0));
        assert!(grid_model(1.0, Vec3::splat(f32::NAN), f32::NAN).is_finite());
    }

    #[test]
    fn axes_start_at_the_origin_along_their_own_axis() {
        let a = axis_lines(&SceneColors::DARK);
        assert_eq!(a.len(), 10);
        for pair in a.chunks(2) {
            assert_eq!(pair[0].pos, [0.0; 3]);
            let nonzero = pair[1].pos.iter().filter(|c| **c != 0.0).count();
            assert_eq!(nonzero, 1, "{pair:?}");
        }
        assert_eq!(a[1].pos, [GRID_EXTENT, 0.0, 0.0]);
        assert_eq!(a[1].color, SceneColors::DARK.axis_x);
        assert_eq!(a[9].pos, [0.0, 0.0, Z_AXIS]);
    }

    /// Back-face culling needs counter-clockwise faces seen from outside, and the light
    /// needs outward normals: for every shape.
    #[test]
    fn every_shape_winds_outward_inside_the_unit_cube() {
        for (shape, tris) in [(Shape::Box, 12), (Shape::Diamond, 8), (Shape::Pyramid, 6)] {
            let f = shape.faces();
            assert_eq!(f.len(), tris * 3, "{shape:?}");
            assert_eq!(solid_floats(&f).len(), f.len() * SOLID_FLOATS);
            for t in f.chunks(3) {
                let [a, b, c] = [t[0], t[1], t[2]].map(|v| Vec3::from(v.pos));
                let n = Vec3::from(t[0].normal);
                assert!(t.iter().all(|v| v.normal == t[0].normal));
                let winding = (b - a).cross(c - a).normalize();
                assert!((winding - n).length() < 1e-6, "{shape:?} {t:?}");
                assert!(((a + b + c) / 3.0).dot(n) > 0.0, "{shape:?} {t:?}");
                assert!([a, b, c].iter().all(|p| p.abs().max_element() <= 1.0));
            }
        }
        assert_eq!(Shape::ALL.map(Shape::index), [0, 1, 2]);
    }

    #[test]
    fn shape_edges_are_distinct_and_join_shape_vertices() {
        for (shape, n) in [(Shape::Box, 12), (Shape::Diamond, 12), (Shape::Pyramid, 8)] {
            let e = shape.edges();
            assert_eq!(e.len(), n * 2, "{shape:?}");
            let corners: Vec<Vec3> = shape.faces().iter().map(|v| Vec3::from(v.pos)).collect();
            let mut seen = std::collections::BTreeSet::new();
            for pair in e.chunks(2) {
                let (a, b) = (Vec3::from(pair[0]), Vec3::from(pair[1]));
                assert!(a != b);
                assert!(corners.contains(&a) && corners.contains(&b), "{shape:?}");
                let key = |v: Vec3| v.to_array().map(|c| (c * 10.0) as i32);
                let (ka, kb) = (key(a), key(b));
                assert!(seen.insert(ka.min(kb).into_iter().chain(ka.max(kb)).collect::<Vec<_>>()));
            }
        }
    }

    /// The pinned convention: angles turn clockwise, Z applied first.
    #[test]
    fn ref_rotation_turns_clockwise_z_first() {
        let q = ref_rotation(Vec3::new(0.0, 0.0, 90f32.to_radians()));
        // +X turns to -Y (clockwise seen from above).
        assert!((q * Vec3::X - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-6);
        // Rx(-x) * Ry(-y) * Rz(-z): Z first, then Y, then X.
        let (x, y, z) = (0.3f32, -0.7f32, 1.1f32);
        let want =
            Mat4::from_rotation_x(-x) * Mat4::from_rotation_y(-y) * Mat4::from_rotation_z(-z);
        let got = Mat4::from_quat(ref_rotation(Vec3::new(x, y, z)));
        assert!(got.abs_diff_eq(want, 1e-5));
        assert_eq!(ref_rotation(Vec3::splat(f32::NAN)), Quat::IDENTITY);
    }

    #[test]
    fn the_placeholder_rests_on_the_grid_turned_about_z() {
        let m = Marker::placeholder();
        let model = m.model();
        let c = m.corners();
        let min_z = c.iter().map(|p| p.z).fold(f32::MAX, f32::min);
        let max_z = c.iter().map(|p| p.z).fold(f32::MIN, f32::max);
        assert!(min_z.abs() < 1e-5 && (max_z - 1.3).abs() < 1e-5);
        // The long axis (+X before turning) points 30 degrees from +X toward +Y.
        let long = model.transform_vector3(Vec3::X).normalize();
        assert!((long.y.atan2(long.x).to_degrees() - 30.0).abs() < 1e-3);
        assert!((model.transform_vector3(Vec3::X).length() - 2.0).abs() < 1e-5);
    }

    /// An `OBND` box: offset from the reference's origin, turned and scaled with it.
    #[test]
    fn obnd_corners_follow_position_rotation_and_scale() {
        let min = Vec3::new(-1.0, 0.0, 0.0);
        let max = Vec3::new(3.0, 2.0, 1.0);
        let pos = Vec3::new(100.0, 50.0, 10.0);
        let m = Marker::placed(
            pos,
            Vec3::new(0.0, 0.0, 90f32.to_radians()),
            Some(2.0),
            Some((min, max)),
            Shape::Pyramid,
            [1.0; 4],
        );
        assert_eq!(
            m.shape,
            Shape::Box,
            "a bounds box whatever the type's shape"
        );
        assert_eq!(m.offset, Vec3::new(1.0, 1.0, 0.5));
        assert_eq!(m.half_extents, Vec3::new(2.0, 1.0, 0.5));
        // Each stored corner c lands at pos + scale * Rz(-90 deg) c: (x, y, z) -> (y, -x, z).
        let mut got: Vec<[i32; 3]> = m
            .corners()
            .iter()
            .map(|p| (*p - pos).to_array().map(|v| v.round() as i32))
            .collect();
        let mut want: Vec<[i32; 3]> = Vec::new();
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    want.push([(2.0 * y) as i32, (-2.0 * x) as i32, (2.0 * z) as i32]);
                }
            }
        }
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want);
        let (lo, hi) = bounds_of(&[m]).unwrap();
        assert_eq!(lo.round(), pos + Vec3::new(0.0, -6.0, 0.0));
        assert_eq!(hi.round(), pos + Vec3::new(4.0, 2.0, 2.0));
    }

    /// Turned about X and Y as well: the corners stay a rigid box around the position.
    #[test]
    fn obnd_corners_under_a_full_rotation_keep_their_distances() {
        let (min, max) = (Vec3::new(-0.5, -1.0, 0.0), Vec3::new(0.5, 1.0, 3.0));
        let r = Vec3::new(0.4, -1.2, 2.5);
        let m = Marker::placed(Vec3::ZERO, r, None, Some((min, max)), Shape::Box, [1.0; 4]);
        let q = ref_rotation(r);
        let mut got: Vec<Vec3> = m.corners().to_vec();
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    let w = q * Vec3::new(x, y, z);
                    let i = got.iter().position(|c| (*c - w).length() < 1e-5);
                    got.remove(i.unwrap_or_else(|| panic!("corner {w}")));
                }
            }
        }
        assert!(got.is_empty());
    }

    #[test]
    fn missing_or_empty_bounds_give_the_types_shape() {
        let p = Vec3::new(1.0, 2.0, 3.0);
        for bounds in [
            None,
            Some((Vec3::ZERO, Vec3::ZERO)),
            Some((Vec3::splat(f32::NAN), Vec3::ONE)),
        ] {
            let m = Marker::placed(
                p,
                Vec3::ZERO,
                Some(f32::NAN),
                bounds,
                Shape::Diamond,
                [1.0; 4],
            );
            assert_eq!(m.shape, Shape::Diamond);
            assert_eq!(m.half_extents, Vec3::splat(UNIT_HALF));
            assert_eq!(m.scale, 1.0);
            assert_eq!(m.model().transform_point3(Vec3::ZERO), p);
        }
        // A flat decal keeps a sliver of thickness.
        let flat = Marker::placed(
            p,
            Vec3::ZERO,
            None,
            Some((Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 0.0))),
            Shape::Pyramid,
            [1.0; 4],
        );
        assert_eq!(flat.shape, Shape::Box);
        assert_eq!(flat.half_extents.z, MIN_HALF);
    }

    #[test]
    fn a_hostile_marker_never_yields_nan() {
        let m = Marker {
            position: Vec3::new(f32::NAN, 0.0, 0.0),
            rotation: Vec3::splat(f32::INFINITY),
            scale: f32::NAN,
            offset: Vec3::splat(f32::NAN),
            half_extents: Vec3::new(-2.0, f32::NAN, 1.0),
            shape: Shape::Box,
            color: [1.0; 4],
        };
        assert!(m.model().is_finite());
        assert!(m.instance().iter().all(|v| v.is_finite()));
        assert_eq!(m.hit(Vec3::new(0.0, 0.0, 10.0), Vec3::NEG_Z), Some(9.0));
        let m = Marker {
            scale: 2.0,
            ..Marker::placeholder()
        };
        assert!((m.model().transform_vector3(Vec3::Z).length() - 1.3).abs() < 1e-5);
    }

    #[test]
    fn picking_takes_the_smallest_box_under_the_ray() {
        let at = |x: f32, half: f32| Marker {
            position: Vec3::new(x, 0.0, 0.0),
            half_extents: Vec3::splat(half),
            ..Marker::placeholder()
        };
        let big = at(0.0, 10.0);
        let small = at(0.0, 1.0);
        let aside = at(50.0, 1.0);
        let down = Vec3::NEG_Z;
        let eye = Vec3::new(0.0, 0.0, 100.0);
        assert!((big.hit(eye, down).unwrap() - 90.0).abs() < 1e-3);
        assert_eq!(pick(&[big, small, aside], eye, down), Some(1));
        assert_eq!(pick(&[big, aside], eye, down), Some(0));
        assert_eq!(
            pick(&[big, small], Vec3::new(5.0, 0.0, 100.0), down),
            Some(0)
        );
        assert_eq!(pick(&[aside], eye, down), None);
        // From inside a box, that box is not picked; one ahead is.
        assert_eq!(big.hit(Vec3::ZERO, down), None);
        assert_eq!(pick(&[big], Vec3::new(0.0, 0.0, 5.0), Vec3::X), None);
        // Behind the eye: no hit.
        assert_eq!(small.hit(eye, Vec3::Z), None);
        assert_eq!(bounds_of(&[]), None);
    }

    #[test]
    fn an_instance_is_the_model_then_the_color() {
        let m = Marker::placeholder();
        let i = m.instance();
        assert_eq!(i[..16], m.model().to_cols_array());
        assert_eq!(i[16..], m.color);
    }
}
