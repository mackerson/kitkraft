//! Synthetic `.mesh` files and geometry slots built to the layout of the M7.S5 study
//! (no Bethesda bytes).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
#![allow(clippy::arithmetic_side_effects, clippy::panic)]

use crate::{Error, Mesh, Nif, Transform, VERSION, VERSION_LINE};

fn u32s(out: &mut Vec<u8>, v: &[u32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

/// Parts of a synthetic mesh; `bytes()` lays them out as claims M1 to M6 say.
struct M {
    first: u32,
    indices: Vec<u16>,
    scale: f32,
    after_scale: u32,
    positions: Vec<[i16; 3]>,
    /// Counts of the five sections (each entry 4 bytes); `None` = the vertex count.
    counts: [Option<u32>; 5],
    normal: u32,
    m5: Option<u32>,
    lists: Vec<Vec<u16>>,
    m6: Option<u32>,
}

impl M {
    fn triangle() -> Self {
        Self {
            first: 2,
            indices: vec![0, 1, 2],
            scale: 2.0,
            after_scale: 0,
            positions: vec![[0, 0, 0], [32767, 0, 0], [0, 32767, 0]],
            counts: [None, Some(0), Some(0), None, None],
            // +z: fields x = y = 511.5 -> 512 (about 0), z = 1023 (1).
            normal: 512 | (512 << 10) | (1023 << 20),
            m5: None,
            lists: vec![],
            m6: Some(1),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let nv = self.positions.len() as u32;
        let mut out = Vec::new();
        u32s(&mut out, &[self.first, self.indices.len() as u32]);
        for i in &self.indices {
            out.extend_from_slice(&i.to_le_bytes());
        }
        out.extend_from_slice(&self.scale.to_le_bytes());
        u32s(&mut out, &[self.after_scale, nv]);
        for p in &self.positions {
            for v in p {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for (k, c) in self.counts.iter().enumerate() {
            let n = c.unwrap_or(nv);
            u32s(&mut out, &[n]);
            for _ in 0..n {
                u32s(&mut out, &[if k == 3 { self.normal } else { 7 }]);
            }
        }
        let m5 = self.m5.unwrap_or(nv * self.after_scale);
        u32s(&mut out, &[m5]);
        for _ in 0..m5 {
            u32s(&mut out, &[0xFFFF_0000]);
        }
        u32s(&mut out, &[self.lists.len() as u32]);
        for l in &self.lists {
            u32s(&mut out, &[l.len() as u32]);
            for v in l {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        if let Some(m) = self.m6 {
            u32s(&mut out, &[m]);
            out.extend(std::iter::repeat_n(0u8, 40 * m as usize));
            out.extend_from_slice(&[9, 9, 9, 9]);
        }
        out
    }
}

#[test]
fn a_triangle_reads_back_its_positions_indices_and_normals() {
    let mut m = M::triangle();
    m.after_scale = 1;
    m.lists = vec![vec![0, 1, 2]];
    let bytes = m.bytes();
    let mesh = Mesh::parse(&bytes).unwrap();
    assert_eq!(mesh.indices, vec![0, 1, 2]);
    assert_eq!(mesh.vertex_count(), 3);
    assert_eq!(
        mesh.positions(),
        vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]
    );
    let n = mesh.normals();
    assert!(n
        .iter()
        .all(|v| v[0].abs() < 0.01 && v[1].abs() < 0.01 && v[2] == 1.0));
    assert_eq!(mesh.offsets.indices, 8);
    assert_eq!(mesh.offsets.positions, 8 + 6 + 12);
}

#[test]
fn a_mesh_that_ends_after_its_lists_reads() {
    let mut m = M::triangle();
    m.m6 = None;
    assert!(Mesh::parse(&m.bytes()).is_ok());
}

#[test]
fn what_no_official_mesh_has_is_refused_with_its_offset() {
    let refused = |m: M| Mesh::parse(&m.bytes()).unwrap_err();
    let mut m = M::triangle();
    m.first = 3;
    assert!(matches!(
        refused(m),
        Error::Unsupported {
            what: "mesh first u32",
            offset: 0,
            value: 3
        }
    ));
    let mut m = M::triangle();
    m.indices = vec![0, 1, 3];
    assert!(matches!(
        refused(m),
        Error::OutOfRange {
            what: "index",
            offset: 12,
            value: 3
        }
    ));
    let mut m = M::triangle();
    m.indices = vec![0, 1];
    assert!(matches!(
        refused(m),
        Error::Unsupported {
            offset: 4,
            value: 2,
            ..
        }
    ));
    let mut m = M::triangle();
    m.counts[0] = Some(0);
    assert!(matches!(
        refused(m),
        Error::Unsupported {
            what: "section count",
            ..
        }
    ));
    let mut m = M::triangle();
    m.counts[1] = Some(2);
    assert!(matches!(
        refused(m),
        Error::Unsupported {
            what: "section count",
            ..
        }
    ));
    let mut m = M::triangle();
    m.m5 = Some(1);
    assert!(matches!(
        refused(m),
        Error::Unsupported {
            what: "M5 entry count",
            ..
        }
    ));
    let mut m = M::triangle();
    m.lists = vec![vec![5]];
    assert!(matches!(
        refused(m),
        Error::OutOfRange {
            what: "M5 list value",
            ..
        }
    ));
    let mut bytes = M::triangle().bytes();
    bytes.push(0);
    assert!(matches!(
        Mesh::parse(&bytes),
        Err(Error::Trailing { len: 1, .. })
    ));
}

#[test]
fn every_truncated_mesh_is_an_error_not_a_panic() {
    let bytes = M::triangle().bytes();
    // Ending right after the lists is a whole mesh (M6), the one prefix that reads.
    let without_m6 = bytes.len() - (4 + 40 + 4);
    for len in 0..bytes.len() {
        assert_eq!(
            Mesh::parse(&bytes[..len]).is_ok(),
            len == without_m6,
            "{len} bytes"
        );
    }
}

#[test]
fn huge_declared_counts_fail_without_allocating_them() {
    let mut bytes = Vec::new();
    u32s(&mut bytes, &[2, 3_000_000_000]);
    assert!(matches!(Mesh::parse(&bytes), Err(Error::Truncated { .. })));
}

// ---- slots

/// A model: an `NiNode` holding one `BSGeometry` whose four slots are `slots`
/// (`None` = absent; `Some((indices, vertices, third, path))`).
fn model(slots: &[Option<(u32, u32, u32, &str)>], trailing: &[u8]) -> Vec<u8> {
    let identity = Transform::IDENTITY;
    let prefix = |out: &mut Vec<u8>, name: u32| {
        u32s(out, &[name, 0, u32::MAX, 0x0e]);
        for v in identity.translation {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for row in identity.rotation {
            for v in row {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&identity.scale.to_le_bytes());
        u32s(out, &[u32::MAX]);
    };
    let mut node = Vec::new();
    prefix(&mut node, 0);
    u32s(&mut node, &[1, 1]);
    let mut geom = Vec::new();
    prefix(&mut geom, u32::MAX);
    for v in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5] {
        geom.extend_from_slice(&v.to_le_bytes());
    }
    u32s(&mut geom, &[u32::MAX, u32::MAX, u32::MAX]);
    for s in slots {
        match s {
            None => geom.push(0),
            Some((a, b, c, path)) => {
                geom.push(1);
                u32s(&mut geom, &[*a, *b, *c, path.len() as u32]);
                geom.extend_from_slice(path.as_bytes());
            }
        }
    }
    geom.extend_from_slice(trailing);
    let blocks = [(0u16, node), (1u16, geom)];
    let mut out = VERSION_LINE.to_vec();
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(1);
    u32s(&mut out, &[12, 2, 175]);
    out.extend_from_slice(&[1, 0]);
    u32s(&mut out, &[3]);
    out.extend_from_slice(&[1, 0, 2, 7, 0]);
    out.extend_from_slice(&2u16.to_le_bytes());
    for t in ["NiNode", "BSGeometry"] {
        u32s(&mut out, &[t.len() as u32]);
        out.extend_from_slice(t.as_bytes());
    }
    for (t, _) in &blocks {
        out.extend_from_slice(&t.to_le_bytes());
    }
    for (_, b) in &blocks {
        u32s(&mut out, &[b.len() as u32]);
    }
    u32s(&mut out, &[1, 4, 4]);
    out.extend_from_slice(b"root");
    u32s(&mut out, &[0]);
    for (_, b) in &blocks {
        out.extend_from_slice(b);
    }
    u32s(&mut out, &[1, 0]);
    out
}

#[test]
fn a_geometry_names_its_meshes_in_four_slots() {
    let bytes = model(
        &[
            Some((300, 104, 64, r"5eb2c642863e7a9bf5fd\b0f72db71f345718d55a")),
            Some((30, 10, 64, r"Geometries\aa\bb.mesh")),
            None,
            None,
        ],
        &[],
    );
    let nif = Nif::parse(&bytes).unwrap();
    assert_eq!(nif.slots(0).unwrap(), None);
    let slots = nif.slots(1).unwrap().unwrap();
    assert_eq!(slots.len(), 2);
    assert_eq!(
        (slots[0].slot, slots[0].index_count, slots[0].vertex_count),
        (0, 300, 104)
    );
    assert_eq!(
        slots[0].mesh_entry().as_deref(),
        Some("geometries/5eb2c642863e7a9bf5fd/b0f72db71f345718d55a.mesh")
    );
    assert_eq!(slots[1].slot, 1);
    assert_eq!(
        slots[1].mesh_entry().as_deref(),
        Some("geometries/aa/bb.mesh")
    );
    // The walk gives each geometry its transform to the root.
    let b = nif.model_bounds().unwrap();
    assert_eq!(b.geometries[0].to_root[0], [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn slots_the_study_did_not_see_are_refused() {
    let one = |slots: &[Option<(u32, u32, u32, &str)>], trailing: &[u8]| {
        let bytes = model(slots, trailing);
        let nif = Nif::parse(&bytes).unwrap();
        nif.slots(1)
    };
    let s = Some((3, 3, 64, "a\\b"));
    assert!(one(&[s, None, None, None], &[]).is_ok());
    assert!(matches!(
        one(&[Some((3, 3, 65, "a\\b")), None, None, None], &[]),
        Err(Error::Unsupported {
            what: "slot third u32",
            value: 65,
            ..
        })
    ));
    assert!(matches!(
        one(&[s, None, None, None], &[0]),
        Err(Error::Trailing { len: 1, .. })
    ));
    // An empty path (the loose primitives' inline geometry) names no mesh entry.
    let empty = one(&[Some((3, 3, 64, "")), None, None, None], &[])
        .unwrap()
        .unwrap();
    assert_eq!(empty[0].mesh_entry(), None);
}
