//! What the viewport draws, as plain data: the ground grid, the world axes, the unit box
//! every marker is drawn from, the markers themselves and the scene colors. No GL here
//! (`super::gl` uploads and draws it), so the geometry is unit-tested.
//!
//! Units are the game's (`super::camera`, Z up). R1 has no game data: one placeholder
//! [`Marker`] stands at the origin. M7.R2 fills [`Marker`]s from the `REFR`s of the
//! selected cell (position, rotation, scale from the `cell` command), through the same
//! [`Marker::model`] and the same draw path.

use std::ops::{Add, Mul};

use glam::{EulerRot, Mat4, Quat, Vec3};

/// Grid spacing in game units.
pub const GRID_STEP: f32 = 256.0;
/// Every `GRID_MAJOR`th line is a major line (4,096 units apart).
pub const GRID_MAJOR: i32 = 16;
/// Grid lines run from `GRID_FIRST` to `GRID_LAST` steps (+-8,192 units).
pub const GRID_FIRST: i32 = -32;
/// See [`GRID_FIRST`].
pub const GRID_LAST: i32 = 32;
/// Half the grid's width in game units (`GRID_LAST` steps).
pub const GRID_EXTENT: f32 = 8_192.0;
/// Length of the +Z axis line.
pub const Z_AXIS: f32 = 1_024.0;

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
    /// Marker outlines (the fill is each marker's own color).
    pub edge: [f32; 4],
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

/// The ground grid on the Z = 0 plane: lines every [`GRID_STEP`] parallel to X and to Y,
/// major every [`GRID_MAJOR`] steps. The lines through the origin are left to the axes.
pub fn grid_lines(colors: &SceneColors) -> Vec<LineVertex> {
    let mut out = Vec::new();
    for i in GRID_FIRST..=GRID_LAST {
        if i == 0 {
            continue;
        }
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

/// The world axes from the origin: +X red, +Y green across the grid (their negative
/// halves dimmed), and +Z blue up to [`Z_AXIS`].
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
        let quad = [
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ];
        for idx in [0usize, 1, 2, 0, 2, 3] {
            if let Some(p) = quad.get(idx) {
                out.push(SolidVertex {
                    pos: p.to_array(),
                    normal: n,
                });
            }
        }
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

/// One oriented box in the scene: where it stands, how it is turned and scaled, its size
/// and its fill color. R1 draws [`Marker::placeholder`]; M7.R2 makes one per placed
/// reference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    /// Center, in game units.
    pub position: Vec3,
    /// Angles about X, Y and Z in radians, applied X first, then Y, then Z (the order the
    /// game uses for a `REFR`'s rotation is not established here; M7.R2 pins it from
    /// evidence before markers come from plugins).
    pub rotation: Vec3,
    /// Uniform scale (a `REFR`'s `XSCL`; 1 when absent).
    pub scale: f32,
    /// Half the box's size along its own axes before scaling, in game units.
    pub half_extents: Vec3,
    /// Fill color (RGBA, 0..1).
    pub color: [f32; 4],
}

impl Marker {
    /// R1's placeholder: a 384 x 192 x 128 box turned 30 degrees about Z, resting on the
    /// grid at the origin.
    pub fn placeholder() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 64.0),
            rotation: Vec3::new(0.0, 0.0, 30f32.to_radians()),
            scale: 1.0,
            half_extents: Vec3::new(192.0, 96.0, 64.0),
            color: [0.86, 0.60, 0.24, 1.0],
        }
    }

    /// The model matrix: the unit box (-1..1) to world space. Non-finite values fall back
    /// to identity parts, so a hostile value never reaches the GPU as NaN.
    pub fn model(&self) -> Mat4 {
        let finite = |v: Vec3, or: Vec3| if v.is_finite() { v } else { or };
        let scale = if self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        };
        let r = finite(self.rotation, Vec3::ZERO);
        let rotation = Quat::from_euler(EulerRot::ZYX, r.z, r.y, r.x);
        Mat4::from_scale_rotation_translation(
            finite(self.half_extents, Vec3::ONE).abs().mul(scale),
            rotation,
            finite(self.position, Vec3::ZERO),
        )
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn grid_is_flat_symmetric_and_marks_major_lines() {
        let g = grid_lines(&SceneColors::DARK);
        // 64 lines per direction (the origin lines are the axes'), two vertices each.
        assert_eq!(g.len(), 64 * 2 * 2);
        assert!(g.iter().all(|v| v.pos[2] == 0.0));
        assert!(g
            .iter()
            .all(|v| v.pos[0].abs() <= GRID_EXTENT && v.pos[1].abs() <= GRID_EXTENT));
        let majors: Vec<_> = g
            .iter()
            .filter(|v| v.color == SceneColors::DARK.grid_major)
            .collect();
        // +-4096 and +-8192, both directions, two vertices.
        assert_eq!(majors.len(), 4 * 2 * 2);
        assert!(majors
            .iter()
            .all(|v| v.pos[0] % 4096.0 == 0.0 && v.pos[1] % 4096.0 == 0.0));
        assert_eq!(GRID_LAST as f32 * GRID_STEP, GRID_EXTENT);
        assert_eq!(line_floats(&g).len(), g.len() * LINE_FLOATS);
        assert_eq!(LINE_LAYOUT.iter().sum::<i32>() as usize, LINE_FLOATS);
        assert_eq!(SOLID_LAYOUT.iter().sum::<i32>() as usize, SOLID_FLOATS);
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
    /// needs outward normals.
    #[test]
    fn box_faces_wind_outward() {
        let f = unit_box_faces();
        assert_eq!(f.len(), 36);
        assert_eq!(solid_floats(&f).len(), 36 * SOLID_FLOATS);
        for tri in f.chunks(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|v| Vec3::from(v.pos));
            let n = Vec3::from(tri[0].normal);
            assert!(tri.iter().all(|v| v.normal == tri[0].normal));
            let winding = (b - a).cross(c - a).normalize();
            assert!((winding - n).length() < 1e-6, "{tri:?}");
            let centroid = (a + b + c) / 3.0;
            assert!(centroid.dot(n) > 0.0);
            assert!([a, b, c].iter().all(|p| p.abs().max_element() == 1.0));
        }
    }

    #[test]
    fn box_edges_are_the_twelve_axis_edges() {
        let e = unit_box_edges();
        assert_eq!(e.len(), 24);
        let mut seen = std::collections::BTreeSet::new();
        for pair in e.chunks(2) {
            let (a, b) = (Vec3::from(pair[0]), Vec3::from(pair[1]));
            assert_eq!((b - a).length(), 2.0);
            let key: Vec<i32> = (a + b).to_array().iter().map(|c| *c as i32).collect();
            assert!(seen.insert(key), "duplicate edge {pair:?}");
        }
    }

    #[test]
    fn the_placeholder_rests_on_the_grid_turned_about_z() {
        let m = Marker::placeholder();
        let model = m.model();
        let corners: Vec<Vec3> = unit_box_edges()
            .iter()
            .map(|p| model.transform_point3(Vec3::from(*p)))
            .collect();
        let min_z = corners.iter().map(|p| p.z).fold(f32::MAX, f32::min);
        let max_z = corners.iter().map(|p| p.z).fold(f32::MIN, f32::max);
        assert!(min_z.abs() < 1e-3 && (max_z - 128.0).abs() < 1e-3);
        // The box's long axis (+X before turning) points 30 degrees from +X toward +Y.
        let long = model.transform_vector3(Vec3::X).normalize();
        assert!((long.y.atan2(long.x).to_degrees() - 30.0).abs() < 1e-3);
        assert!((model.transform_vector3(Vec3::X).length() - 192.0).abs() < 1e-3);
    }

    #[test]
    fn a_hostile_marker_never_yields_nan() {
        let m = Marker {
            position: Vec3::new(f32::NAN, 0.0, 0.0),
            rotation: Vec3::splat(f32::INFINITY),
            scale: f32::NAN,
            half_extents: Vec3::new(-2.0, f32::NAN, 1.0),
            color: [1.0; 4],
        };
        assert!(m.model().is_finite());
        let m = Marker {
            scale: 2.0,
            ..Marker::placeholder()
        };
        assert!((m.model().transform_vector3(Vec3::Z).length() - 128.0).abs() < 1e-3);
    }
}
