//! A base's triangles from the engine's `mesh.geometry` (M7.R4): decoded from the
//! envelope's base64 buffers, checked, and turned into the viewport's wireframe lines and
//! flat-shaded faces, placed by a reference's position, rotation and scale.
//!
//! The geometry comes only from that envelope (`docs/mcp.md`, `mesh.geometry`; R15
//! provenance): nothing here reads a NIF or a `.mesh`. Every buffer is treated as hostile:
//! its length is checked before anything is allocated, every index is checked against the
//! vertex count, and every position must be finite. The engine's normals (grade B) are not
//! used: a flat face's normal is its triangle's own, so a degenerate triangle has none and
//! is not drawn (counted in [`BaseMesh::degenerate`]).

use std::collections::BTreeSet;
use std::ops::{Div, Sub};

use glam::{Mat4, Vec3};
use serde_json::Value;

use super::scene::LineVertex;

/// Most triangles drawn as meshes in one scene (every reference together). A reference
/// past it is drawn as its box, with a limitation line.
pub const MESH_SCENE_TRIANGLES: usize = 500_000;

/// Most triangles placed per frame while a scene's meshes are built; the rest wait for
/// the next frames, and the GPU upload happens once, when the build is complete. The
/// budget is soft: it is checked after each reference is placed, so one base of up to
/// [`MESH_SCENE_TRIANGLES`] is placed whole in a single frame (review I3 of M7.R4).
pub const MESH_FRAME_TRIANGLES: usize = 100_000;

/// The engine's own caps (`handlers/mesh.rs`: `MAX_GEOMETRY_VERTICES`,
/// `MAX_GEOMETRY_INDICES`): a reply over them is not one the engine sends.
pub const MAX_VERTICES: usize = 2_000_000;
/// See [`MAX_VERTICES`].
pub const MAX_INDICES: usize = 6_000_000;

/// Floats per face vertex as uploaded: position, normal, color.
pub const FACE_FLOATS: usize = 10;
/// Components per face attribute, in location order (`shaders::MESH_FACES`).
pub const FACE_LAYOUT: [i32; 3] = [3, 3, 4];

/// One base's triangles in its model's root frame.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseMesh {
    /// The base form (`data.form`).
    pub form: String,
    pub positions: Vec<Vec3>,
    /// The drawable triangles (indices into `positions`).
    pub triangles: Vec<[u32; 3]>,
    /// Each edge once (sorted index pairs), for the wireframe.
    pub edges: Vec<[u32; 2]>,
    /// Triangles dropped as degenerate (no area, so no face normal).
    pub degenerate: usize,
    /// The engine's lines about blocks it skipped (`block <n>: ...`) or did not reach from
    /// the root (review L1 of M7.R4): the mesh is drawn
    /// in part, and each is shown as a limitation.
    pub skipped_blocks: Vec<String>,
}

/// Standard base64 (RFC 4648, `=` padding) to bytes, refusing anything longer than
/// `max_bytes` decoded, a character outside the alphabet, or bad padding.
pub fn decode_base64(text: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let s = text.as_bytes();
    if !s.len().is_multiple_of(4) {
        return Err(format!("base64 length {} is not a multiple of 4", s.len()));
    }
    let quads = s.len() / 4;
    let decoded_max = quads.checked_mul(3).ok_or("base64 too long")?;
    if decoded_max > max_bytes.saturating_add(2) {
        return Err(format!(
            "base64 of {decoded_max} bytes is over the {max_bytes}-byte limit"
        ));
    }
    let val = |c: u8| -> Option<u32> {
        Some(u32::from(match c {
            b'A'..=b'Z' => c.checked_sub(b'A')?,
            b'a'..=b'z' => c.checked_sub(b'a')?.checked_add(26)?,
            b'0'..=b'9' => c.checked_sub(b'0')?.checked_add(52)?,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        }))
    };
    let mut out = Vec::with_capacity(decoded_max);
    for (q, chunk) in s.as_chunks::<4>().0.iter().enumerate() {
        let last = q.checked_add(1) == Some(quads);
        let pad = chunk.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return Err(format!("base64 padding misplaced in quad {q}"));
        }
        let mut n: u32 = 0;
        for (i, c) in chunk.iter().enumerate() {
            let v = if i >= 4usize.saturating_sub(pad) {
                0
            } else {
                val(*c).ok_or_else(|| format!("base64 character {c:#04x} in quad {q}"))?
            };
            n = (n << 6) | v;
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(bytes.get(..3usize.saturating_sub(pad)).unwrap_or(&[]));
    }
    if out.len() > max_bytes {
        return Err(format!(
            "{} bytes decoded, over the {max_bytes}-byte limit",
            out.len()
        ));
    }
    Ok(out)
}

fn le_u32(b: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(<[u8; 4]>::try_from(b).ok()?))
}

fn le_f32(b: &[u8]) -> Option<f32> {
    Some(f32::from_le_bytes(<[u8; 4]>::try_from(b).ok()?))
}

impl BaseMesh {
    /// Reads a `mesh.geometry` envelope (`docs/mcp.md`). `max_triangles` caps what is kept
    /// (a base over it could never be drawn in one scene). `Err` says why the reply gives
    /// no mesh to draw; the caller shows it as a limitation and keeps the base's box.
    pub fn from_envelope(env: &Value, max_triangles: usize) -> Result<Self, String> {
        let data = env
            .get("data")
            .ok_or("not a mesh.geometry envelope: no data")?;
        let form = data
            .get("form")
            .and_then(Value::as_str)
            .ok_or("the reply names no base form")?
            .to_string();
        let text = |k: &str| {
            data.get(k)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("no `{k}` buffer"))
        };
        let vertex_count = data
            .get("vertex_count")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("no vertex_count")?;
        if vertex_count > MAX_VERTICES {
            return Err(format!(
                "{vertex_count} vertices, over the engine's {MAX_VERTICES}-vertex budget"
            ));
        }
        let pos_bytes = vertex_count
            .checked_mul(12)
            .ok_or("vertex count overflows")?;
        let raw = decode_base64(text("positions")?, pos_bytes)?;
        if raw.len() != pos_bytes {
            return Err(format!(
                "positions hold {} bytes, not {pos_bytes} for {vertex_count} vertices",
                raw.len()
            ));
        }
        let mut positions = Vec::with_capacity(vertex_count);
        for (i, p) in raw.as_chunks::<12>().0.iter().enumerate() {
            let c = |a: usize| p.get(a..a.saturating_add(4)).and_then(le_f32);
            let v = match (c(0), c(4), c(8)) {
                (Some(x), Some(y), Some(z)) => Vec3::new(x, y, z),
                _ => return Err(format!("vertex {i} does not read")),
            };
            if !v.is_finite() {
                return Err(format!("vertex {i} is not finite"));
            }
            positions.push(v);
        }
        let index_bytes = MAX_INDICES.saturating_mul(4);
        let raw = decode_base64(text("indices")?, index_bytes)?;
        if !raw.len().is_multiple_of(12) {
            return Err(format!(
                "indices hold {} bytes, not whole triangles (12 bytes each)",
                raw.len()
            ));
        }
        let count = raw.len() / 12;
        if count > max_triangles {
            return Err(format!(
                "{count} triangles, over the scene's mesh budget of {max_triangles}"
            ));
        }
        let mut triangles = Vec::with_capacity(count);
        let mut degenerate = 0usize;
        for (t, tri) in raw.as_chunks::<12>().0.iter().enumerate() {
            let i = |a: usize| tri.get(a..a.saturating_add(4)).and_then(le_u32);
            let (Some(a), Some(b), Some(c)) = (i(0), i(4), i(8)) else {
                return Err(format!("triangle {t} does not read"));
            };
            for v in [a, b, c] {
                if usize::try_from(v).map_or(true, |v| v >= vertex_count) {
                    return Err(format!("triangle {t} names vertex {v} of {vertex_count}"));
                }
            }
            let at = |v: u32| {
                usize::try_from(v)
                    .ok()
                    .and_then(|v| positions.get(v))
                    .copied()
            };
            match (at(a), at(b), at(c)) {
                (Some(pa), Some(pb), Some(pc)) if face_normal(pa, pb, pc).is_some() => {
                    triangles.push([a, b, c]);
                }
                _ => degenerate = degenerate.saturating_add(1),
            }
        }
        if triangles.is_empty() {
            return Err(if degenerate > 0 {
                format!("all {degenerate} triangles are degenerate")
            } else {
                "the reply holds no triangles".into()
            });
        }
        let mut edges = BTreeSet::new();
        for [a, b, c] in &triangles {
            for (x, y) in [(*a, *b), (*b, *c), (*c, *a)] {
                edges.insert([x.min(y), x.max(y)]);
            }
        }
        let skipped_blocks = env
            .get("limitations")
            .and_then(Value::as_array)
            .map(|l| {
                l.iter()
                    .filter_map(Value::as_str)
                    .filter(|s| {
                        s.starts_with("block ") || s.contains("are not reached from the root")
                    })
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            form,
            positions,
            triangles,
            edges: edges.into_iter().collect(),
            degenerate,
            skipped_blocks,
        })
    }

    fn at(&self, i: u32) -> Option<Vec3> {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.positions.get(i))
            .copied()
    }

    /// The wireframe: each edge once, placed by `placement`, in `color`.
    pub fn lines(&self, placement: &Mat4, color: [f32; 4], out: &mut Vec<LineVertex>) {
        for [a, b] in &self.edges {
            if let (Some(pa), Some(pb)) = (self.at(*a), self.at(*b)) {
                for p in [pa, pb] {
                    out.push(LineVertex {
                        pos: placement.transform_point3(p).to_array(),
                        color,
                    });
                }
            }
        }
    }

    /// The flat faces, placed by `placement`, as uploaded floats ([`FACE_FLOATS`] a
    /// vertex: position, the triangle's normal, `color`).
    pub fn faces(&self, placement: &Mat4, color: [f32; 4], out: &mut Vec<f32>) {
        for [a, b, c] in &self.triangles {
            let (Some(pa), Some(pb), Some(pc)) = (self.at(*a), self.at(*b), self.at(*c)) else {
                continue;
            };
            let [pa, pb, pc] = [pa, pb, pc].map(|p| placement.transform_point3(p));
            let Some(n) = face_normal(pa, pb, pc) else {
                continue;
            };
            for p in [pa, pb, pc] {
                out.extend(p.to_array());
                out.extend(n.to_array());
                out.extend(color);
            }
        }
    }
}

/// A triangle's unit normal (counter-clockwise front), or `None` when it has no area.
pub fn face_normal(a: Vec3, b: Vec3, c: Vec3) -> Option<Vec3> {
    let n = b.sub(a).cross(c.sub(a));
    let len = n.length();
    (len.is_finite() && len > 1e-12).then(|| n.div(len))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// Standard base64 of `bytes`, for canned envelopes.
    pub(crate) fn b64(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
            let n = (b(0) << 16) | (b(1) << 8) | b(2);
            let s = |k: u32| char::from(A[((n >> k) & 63) as usize]);
            out.push(s(18));
            out.push(s(12));
            out.push(if chunk.len() > 1 { s(6) } else { '=' });
            out.push(if chunk.len() > 2 { s(0) } else { '=' });
        }
        out
    }

    /// A `mesh.geometry` envelope for `form` with these vertices and triangles.
    pub(crate) fn geometry_env(form: &str, verts: &[[f32; 3]], tris: &[[u32; 3]]) -> Value {
        let pos: Vec<u8> = verts
            .iter()
            .flat_map(|v| v.iter().flat_map(|x| x.to_le_bytes()))
            .collect();
        let idx: Vec<u8> = tris
            .iter()
            .flat_map(|t| t.iter().flat_map(|x| x.to_le_bytes()))
            .collect();
        json!({"command": "mesh.geometry",
               "data": {"model": "Test\\Quad.nif", "form": form, "frame": "model root",
                        "vertex_count": verts.len(), "index_count": tris.len() * 3,
                        "positions": b64(&pos), "normals": b64(&vec![0u8; pos.len()]),
                        "indices": b64(&idx), "meshes": []},
               "limitations": ["block 2: Test/Skin.mesh skipped: it is skinned",
                               "normals are grade B"]})
    }

    /// A unit square in the XY plane, two counter-clockwise triangles.
    pub(crate) fn quad(form: &str) -> Value {
        geometry_env(
            form,
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        )
    }

    #[test]
    fn base64_round_trips_and_refuses_what_is_not_base64() {
        for n in 0..10usize {
            let bytes: Vec<u8> = (0..n as u8).map(|b| b.wrapping_mul(37)).collect();
            assert_eq!(decode_base64(&b64(&bytes), 64).unwrap(), bytes, "{n}");
        }
        assert!(decode_base64("abc", 64).is_err(), "length");
        assert!(decode_base64("ab*d", 64).is_err(), "alphabet");
        assert!(decode_base64("a===", 64).is_err(), "padding");
        assert!(decode_base64("ab==abcd", 64).is_err(), "padding inside");
        assert!(
            decode_base64(&b64(&[0u8; 30]), 12).is_err(),
            "over the limit"
        );
    }

    #[test]
    fn a_canned_quad_decodes_to_five_edges_and_two_flat_faces() {
        let m = BaseMesh::from_envelope(&quad("T.esm:000800"), 100).unwrap();
        assert_eq!(m.form, "T.esm:000800");
        assert_eq!(m.positions.len(), 4);
        assert_eq!(m.triangles.len(), 2);
        assert_eq!(m.edges.len(), 5, "the shared diagonal once");
        assert_eq!(m.degenerate, 0);
        assert_eq!(
            m.skipped_blocks,
            ["block 2: Test/Skin.mesh skipped: it is skinned"]
        );

        // Placed 10 units up and turned a quarter about Z.
        let place = Mat4::from_rotation_translation(
            glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 0.0, 10.0),
        );
        let mut lines = Vec::new();
        m.lines(&place, [1.0, 0.0, 0.0, 1.0], &mut lines);
        assert_eq!(lines.len(), 10);
        assert!(lines.iter().all(|v| (v.pos[2] - 10.0).abs() < 1e-5));
        let mut faces = Vec::new();
        m.faces(&place, [0.0, 1.0, 0.0, 1.0], &mut faces);
        assert_eq!(faces.len(), 2 * 3 * FACE_FLOATS);
        for v in faces.as_chunks::<FACE_FLOATS>().0 {
            assert!((v[5] - 1.0).abs() < 1e-5, "normal +Z: {v:?}");
            assert_eq!(&v[6..], &[0.0, 1.0, 0.0, 1.0]);
        }
    }

    #[test]
    fn a_bad_reply_is_refused_with_its_reason() {
        let v = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let reason = |env: Value| BaseMesh::from_envelope(&env, 100).unwrap_err();
        assert!(reason(geometry_env("F", &v, &[[0, 1, 3]])).contains("names vertex 3"));
        assert!(reason(geometry_env(
            "F",
            &[[f32::NAN, 0.0, 0.0], v[1], v[2]],
            &[[0, 1, 2]]
        ))
        .contains("not finite"));
        assert!(reason(geometry_env("F", &v, &[[0, 0, 1]])).contains("degenerate"));
        assert!(reason(geometry_env("F", &v, &[])).contains("no triangles"));
        let mut env = geometry_env("F", &v, &[[0, 1, 2]]);
        env["data"]["vertex_count"] = json!(4);
        assert!(reason(env).contains("not 48"));
        let mut env = geometry_env("F", &v, &[[0, 1, 2]]);
        env["data"]["indices"] = json!(b64(&[0u8; 8]));
        assert!(reason(env).contains("whole triangles"));
        let mut env = geometry_env("F", &v, &[[0, 1, 2]]);
        env["data"]["positions"] = json!("!!!!");
        assert!(reason(env).contains("base64"));
        let mut env = geometry_env("F", &v, &[[0, 1, 2]]);
        env["data"]["vertex_count"] = json!(MAX_VERTICES + 1);
        assert!(reason(env).contains("budget"));
        let quad = quad("F");
        assert!(BaseMesh::from_envelope(&quad, 1)
            .unwrap_err()
            .contains("mesh budget of 1"));
        assert!(reason(json!({"data": {}})).contains("base form"));
    }

    /// Review L1 of M7.R4: the engine's line about blocks not reached from the root means
    /// the mesh drawn is partial, so it is carried like a skipped block's line.
    #[test]
    fn unreached_blocks_are_a_limitation_of_the_mesh() {
        let mut env = quad("F");
        let unreached = "2 geometry block(s) are not reached from the root and 0 child reference(s) repeat (none in the official models): their meshes are not read";
        env["limitations"] = json!([unreached, "normals are grade B"]);
        let m = BaseMesh::from_envelope(&env, 100).unwrap();
        assert_eq!(m.skipped_blocks, [unreached]);
    }
}
