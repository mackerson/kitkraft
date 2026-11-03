//! Synthetic models built to the layout of the study (no Bethesda bytes).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
#![allow(clippy::arithmetic_side_effects, clippy::panic)]

use crate::{Bound, Error, Nif, Transform, VERSION, VERSION_LINE};

const NULL: u32 = u32::MAX;

struct Model {
    types: Vec<&'static str>,
    blocks: Vec<(u16, Vec<u8>)>,
    strings: Vec<&'static str>,
    roots: Vec<u32>,
    /// The words after the string table: the `u32` (0 in every official model) and
    /// anything a test inserts after it.
    after_strings: Vec<u32>,
}

fn u32s(out: &mut Vec<u8>, v: &[u32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

fn f32s(out: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

impl Model {
    fn bytes(&self) -> Vec<u8> {
        let mut out = VERSION_LINE.to_vec();
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.push(1);
        u32s(&mut out, &[12, self.blocks.len() as u32, 175]);
        out.extend_from_slice(&[1, 0]);
        u32s(&mut out, &[3]);
        out.extend_from_slice(&[1, 0, 2, 7, 0]);
        out.extend_from_slice(&(self.types.len() as u16).to_le_bytes());
        for t in &self.types {
            u32s(&mut out, &[t.len() as u32]);
            out.extend_from_slice(t.as_bytes());
        }
        for (t, _) in &self.blocks {
            out.extend_from_slice(&t.to_le_bytes());
        }
        for (_, b) in &self.blocks {
            u32s(&mut out, &[b.len() as u32]);
        }
        let longest = self.strings.iter().map(|s| s.len()).max().unwrap_or(0);
        u32s(&mut out, &[self.strings.len() as u32, longest as u32]);
        for s in &self.strings {
            u32s(&mut out, &[s.len() as u32]);
            out.extend_from_slice(s.as_bytes());
        }
        u32s(&mut out, &self.after_strings);
        for (_, b) in &self.blocks {
            out.extend_from_slice(b);
        }
        u32s(&mut out, &[self.roots.len() as u32]);
        u32s(&mut out, &self.roots);
        out
    }
}

fn prefix(out: &mut Vec<u8>, name: u32, t: &Transform) {
    u32s(out, &[name, 0, NULL, 0x0e]);
    f32s(out, &t.translation);
    for row in &t.rotation {
        f32s(out, row);
    }
    f32s(out, &[t.scale]);
    u32s(out, &[NULL]);
}

fn node(t: &Transform, children: &[u32]) -> (u16, Vec<u8>) {
    let mut b = Vec::new();
    prefix(&mut b, 0, t);
    u32s(&mut b, &[children.len() as u32]);
    u32s(&mut b, children);
    (0, b)
}

fn geometry(t: &Transform, sphere: [f32; 4], aabb: [f32; 6], skin: u32) -> (u16, Vec<u8>) {
    let mut b = Vec::new();
    prefix(&mut b, NULL, t);
    f32s(&mut b, &sphere);
    f32s(&mut b, &aabb);
    u32s(&mut b, &[skin, NULL, NULL]);
    // The rest of a geometry body (the mesh list) is not decoded here.
    b.extend_from_slice(&[1, 0, 0, 0, 0]);
    (1, b)
}

fn model(blocks: Vec<(u16, Vec<u8>)>) -> Model {
    Model {
        types: vec!["NiNode", "BSGeometry", "BSSkin::Instance"],
        blocks,
        strings: vec!["root"],
        roots: vec![0],
        after_strings: vec![0],
    }
}

const UNIT_BOX: [f32; 6] = [0.0, 0.0, 0.5, 0.5, 0.5, 0.5];
const UNIT_SPHERE: [f32; 4] = [0.0, 0.0, 0.5, 0.87];

fn moved(translation: [f32; 3], scale: f32) -> Transform {
    Transform {
        translation,
        scale,
        ..Transform::IDENTITY
    }
}

#[test]
fn one_node_one_geometry_gives_the_geometry_box() {
    let m = model(vec![
        node(&Transform::IDENTITY, &[1]),
        geometry(&Transform::IDENTITY, UNIT_SPHERE, UNIT_BOX, NULL),
    ]);
    let bytes = m.bytes();
    let nif = Nif::parse(&bytes).unwrap();
    assert_eq!(nif.blocks.len(), 2);
    assert_eq!(nif.type_name(1), Some("BSGeometry"));
    assert_eq!(nif.strings, vec!["root".to_string()]);
    let node = nif.node(0).unwrap().unwrap();
    assert_eq!(node.children, vec![Some(1)]);
    assert_eq!(node.trailing, 0);
    assert_eq!(node.prefix.name, Some(0));
    let g = nif.geometry(1).unwrap().unwrap();
    assert!(matches!(g.bound, Bound::Stored { .. }));
    assert_eq!(g.skin, None);
    let b = nif.model_bounds().unwrap();
    let aabb = b.aabb.unwrap();
    assert_eq!(aabb.min, [-0.5, -0.5, 0.0]);
    assert_eq!(aabb.max, [0.5, 0.5, 1.0]);
    assert_eq!(b.counts(), [1, 0, 0, 0]);
    assert_eq!(b.unreached_geometries, 0);
    // Geometry body offset + prefix (72 bytes with no extra data) = the bound's offset.
    assert_eq!(g.bound_offset, nif.blocks[1].offset + 72);
}

#[test]
fn transforms_apply_child_first_then_parent() {
    // Node: scale 2, moved +10 x. Geometry: rotated 90 degrees about z (x -> y), moved +1 x.
    let quarter = Transform {
        translation: [1.0, 0.0, 0.0],
        rotation: [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        scale: 1.0,
    };
    let m = model(vec![
        node(&moved([10.0, 0.0, 0.0], 2.0), &[1]),
        geometry(
            &quarter,
            [1.0, 0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            NULL,
        ),
    ]);
    let bytes = m.bytes();
    let aabb = Nif::parse(&bytes)
        .unwrap()
        .model_bounds()
        .unwrap()
        .aabb
        .unwrap();
    // Box x in [0, 2] -> rotated: y in [0, 2] -> +1 x: x = 1 -> scale 2 and +10 x: x = 12,
    // y in [0, 4].
    assert_eq!(aabb.min, [12.0, 0.0, 0.0]);
    assert_eq!(aabb.max, [12.0, 4.0, 0.0]);
}

#[test]
fn sphere_only_and_not_stored_bounds() {
    let max = f32::MAX;
    let m = model(vec![
        node(&Transform::IDENTITY, &[1, 2]),
        geometry(&Transform::IDENTITY, [0.0, 0.0, 0.0, 2.0], [max; 6], NULL),
        geometry(&Transform::IDENTITY, [0.0; 4], [max; 6], 3),
        (2, vec![0; 4]),
    ]);
    let bytes = m.bytes();
    let nif = Nif::parse(&bytes).unwrap();
    assert!(matches!(
        nif.geometry(1).unwrap().unwrap().bound,
        Bound::SphereOnly(_)
    ));
    let skinned = nif.geometry(2).unwrap().unwrap();
    assert_eq!(skinned.bound, Bound::NotStored);
    assert_eq!(skinned.skin, Some(3));
    let b = nif.model_bounds().unwrap();
    assert_eq!(b.counts(), [0, 1, 1, 0]);
    let aabb = b.aabb.unwrap();
    assert_eq!(aabb.min, [-2.0; 3]);
    assert_eq!(aabb.max, [2.0; 3]);
}

#[test]
fn a_model_with_only_unbounded_geometry_has_no_box() {
    let m = model(vec![
        node(&Transform::IDENTITY, &[1]),
        geometry(&Transform::IDENTITY, [0.0; 4], [f32::MAX; 6], NULL),
    ]);
    let bytes = m.bytes();
    let b = Nif::parse(&bytes).unwrap().model_bounds().unwrap();
    assert_eq!(b.aabb, None);
    assert_eq!(b.counts(), [0, 0, 1, 0]);
}

#[test]
fn values_outside_the_three_patterns_are_unrecognised_not_a_box() {
    let m = model(vec![
        node(&Transform::IDENTITY, &[1, 2]),
        geometry(
            &Transform::IDENTITY,
            UNIT_SPHERE,
            [0.0, 0.0, 0.0, -1.0, 1.0, 1.0],
            NULL,
        ),
        geometry(
            &Transform::IDENTITY,
            [f32::NAN, 0.0, 0.0, 1.0],
            UNIT_BOX,
            NULL,
        ),
    ]);
    let bytes = m.bytes();
    let b = Nif::parse(&bytes).unwrap().model_bounds().unwrap();
    assert_eq!(b.counts(), [0, 0, 0, 2]);
    assert_eq!(b.aabb, None);
}

#[test]
fn unreached_geometry_and_revisits_are_counted() {
    let m = model(vec![
        // The root lists itself and block 1 twice; block 2 is reached by nothing.
        node(&Transform::IDENTITY, &[0, 1, 1, NULL]),
        geometry(&Transform::IDENTITY, UNIT_SPHERE, UNIT_BOX, NULL),
        geometry(&Transform::IDENTITY, UNIT_SPHERE, UNIT_BOX, NULL),
    ]);
    let bytes = m.bytes();
    let nif = Nif::parse(&bytes).unwrap();
    assert_eq!(nif.node(0).unwrap().unwrap().children.last(), Some(&None));
    let b = nif.model_bounds().unwrap();
    assert_eq!(b.geometries.len(), 1);
    assert_eq!(b.unreached_geometries, 1);
    assert_eq!(b.revisits, 2);
}

#[test]
fn a_child_past_the_last_block_is_refused() {
    let m = model(vec![
        node(&Transform::IDENTITY, &[7]),
        geometry(&Transform::IDENTITY, UNIT_SPHERE, UNIT_BOX, NULL),
    ]);
    let bytes = m.bytes();
    let nif = Nif::parse(&bytes).unwrap();
    assert!(matches!(
        nif.node(0),
        Err(Error::OutOfRange {
            what: "child reference",
            value: 7,
            ..
        })
    ));
    assert!(nif.model_bounds().is_err());
}

#[test]
fn every_truncation_is_an_error_not_a_panic() {
    let m = model(vec![
        node(&Transform::IDENTITY, &[1]),
        geometry(&Transform::IDENTITY, UNIT_SPHERE, UNIT_BOX, NULL),
    ]);
    let bytes = m.bytes();
    for len in 0..bytes.len() {
        assert!(
            Nif::parse(&bytes[..len]).is_err(),
            "prefix of {len} bytes parsed"
        );
    }
}

#[test]
fn a_byte_after_the_footer_is_refused() {
    let mut bytes = model(vec![node(&Transform::IDENTITY, &[])]).bytes();
    bytes.push(0);
    assert!(matches!(
        Nif::parse(&bytes),
        Err(Error::Trailing { len: 1, .. })
    ));
}

#[test]
fn another_version_line_or_number_is_refused() {
    let mut bytes = model(vec![node(&Transform::IDENTITY, &[])]).bytes();
    let mut other = bytes.clone();
    other[30] = b'1';
    assert_eq!(Nif::parse(&other), Err(Error::NotNif));
    bytes[VERSION_LINE.len()] = 8;
    assert!(matches!(
        Nif::parse(&bytes),
        Err(Error::Unsupported {
            what: "version",
            offset: 39,
            ..
        })
    ));
}

#[test]
fn huge_declared_counts_fail_without_allocating_them() {
    let mut bytes = VERSION_LINE.to_vec();
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.push(1);
    u32s(&mut bytes, &[12, u32::MAX, 175]);
    bytes.extend_from_slice(&[0]);
    u32s(&mut bytes, &[3]);
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    assert!(matches!(Nif::parse(&bytes), Err(Error::Truncated { .. })));
}

#[test]
fn a_block_type_index_past_the_table_is_refused() {
    let mut m = model(vec![node(&Transform::IDENTITY, &[])]);
    m.blocks[0].0 = 9;
    assert!(matches!(
        Nif::parse(&m.bytes()),
        Err(Error::OutOfRange {
            what: "block type index",
            ..
        })
    ));
}

#[test]
fn a_root_past_the_last_block_is_refused() {
    let mut m = model(vec![node(&Transform::IDENTITY, &[])]);
    m.roots = vec![1];
    assert!(matches!(
        Nif::parse(&m.bytes()),
        Err(Error::OutOfRange { what: "root", .. })
    ));
}

#[test]
fn a_block_of_another_type_is_neither_node_nor_geometry() {
    let m = model(vec![node(&Transform::IDENTITY, &[1]), (2, vec![0; 4])]);
    let bytes = m.bytes();
    let nif = Nif::parse(&bytes).unwrap();
    assert_eq!(nif.node(1).unwrap(), None);
    assert_eq!(nif.geometry(1).unwrap(), None);
    assert_eq!(nif.geometry(0).unwrap(), None);
    assert_eq!(nif.model_bounds().unwrap().aabb, None);
}

#[test]
fn a_nonzero_u32_after_the_string_table_is_refused() {
    // Review H1: the study shows only that this u32 is 0; any other value is a layout no
    // official model has, refused with its offset (the reviewer's probe: 1, then DEADBEEF).
    let mut m = model(vec![node(&Transform::IDENTITY, &[])]);
    m.after_strings = vec![1, 0xDEAD_BEEF];
    assert!(matches!(
        Nif::parse(&m.bytes()),
        Err(Error::Unsupported {
            what: "u32 after the string table",
            value: 1,
            ..
        })
    ));
}

#[test]
fn a_footer_other_than_one_then_a_block_is_refused() {
    // Review H1 (root count): only `1, 0` occurs in the official footers.
    let mut m = model(vec![node(&Transform::IDENTITY, &[])]);
    m.roots = vec![0, 0];
    assert!(matches!(
        Nif::parse(&m.bytes()),
        Err(Error::Unsupported {
            what: "footer u32",
            value: 2,
            ..
        })
    ));
}

#[test]
fn a_set_value_where_every_official_block_has_none_is_refused() {
    // The u32 after the extra-data list is 0xFFFFFFFF in every official block (B2).
    let mut b = Vec::new();
    prefix(&mut b, 0, &Transform::IDENTITY);
    b[8..12].copy_from_slice(&0u32.to_le_bytes());
    u32s(&mut b, &[0]);
    let m = model(vec![(0, b)]);
    let bytes = m.bytes();
    let nif = Nif::parse(&bytes).unwrap();
    assert!(matches!(
        nif.node(0),
        Err(Error::Unsupported {
            what: "u32 after the extra data",
            value: 0,
            ..
        })
    ));
}

#[test]
fn a_box_beyond_f32_is_no_box_not_infinite_numbers() {
    // Review L5: finite but huge transforms made f32 corners infinite, written as null.
    let m = model(vec![
        node(&moved([0.0; 3], 1.0e30), &[1]),
        geometry(&moved([0.0; 3], 1.0e30), UNIT_SPHERE, UNIT_BOX, NULL),
    ]);
    let bytes = m.bytes();
    let b = Nif::parse(&bytes).unwrap().model_bounds().unwrap();
    assert_eq!(b.aabb, None);
    assert!(b.box_overflow);
    assert_eq!(b.counts(), [1, 0, 0, 0]);
}

#[test]
fn another_value_at_offsets_43_and_44_is_refused() {
    // Review L1 of M7.R3: the study says every single-value field is refused; the u8 at
    // 43 (always 1) and the u32 at 44 (always 12) were kept but not refused.
    let good = model(vec![node(&Transform::IDENTITY, &[])]).bytes();
    let mut at43 = good.clone();
    at43[43] = 2;
    assert!(matches!(
        Nif::parse(&at43),
        Err(Error::Unsupported {
            what: "u8 at offset 43",
            offset: 43,
            value: 2
        })
    ));
    let mut at44 = good;
    at44[44] = 13;
    assert!(matches!(
        Nif::parse(&at44),
        Err(Error::Unsupported {
            what: "u32 at offset 44",
            offset: 44,
            value: 13
        })
    ));
}
