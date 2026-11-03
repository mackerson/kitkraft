//! Geometry: the slots a `BSGeometry` block names its meshes in, and the `.mesh` file up to
//! positions, triangle indices and normals (M7.S5).
//!
//! Every fact comes from our own corpus study, `docs/research/mesh-layout-2026-11-02.md`
//! (claims G1, G2, M1 to M6), checked on all 720,957 official meshes and the 120,543
//! archived models. Positions and indices are grade A. The normals (M4) are grade B: the
//! study shows them statistically (99.86% unit length, 98.8% facing their triangles'
//! winding), so [`Mesh::normals`] says so. Fields whose meaning the bytes do not show are
//! named by place and kept only as offsets.

use crate::block::GEOMETRY_TYPE;
use crate::error::{Error, Result};
use crate::read::{Cur, Nif};

/// The first `u32` of every official mesh (claim M1).
pub const MESH_VERSION: u32 = 2;

/// The third `u32` of every official slot (claim G1).
const SLOT_THIRD: u32 = 64;

/// One slot of a geometry block (claim G1): the counts of the mesh it names, and its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// Which of the four slots (0 is always present in the official models).
    pub slot: u8,
    /// The named mesh's index count (claim G2).
    pub index_count: u32,
    /// The named mesh's vertex count (claim G2).
    pub vertex_count: u32,
    /// The path as stored: `a\b` or `Geometries\a\b.mesh`.
    pub path: String,
    /// File offset of the slot's flag, for evidence.
    pub offset: usize,
}

impl Slot {
    /// The archive entry the path names (claim G2): `geometries/a/b.mesh`, lower case,
    /// `/` separated; `None` for an empty path (the 4 loose editor primitives keep their
    /// geometry inside the model, which the study does not decode).
    pub fn mesh_entry(&self) -> Option<String> {
        let n: String = self
            .path
            .chars()
            .map(|c| {
                if c == '\\' {
                    '/'
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
        if n.is_empty() {
            return None;
        }
        let n = if n.starts_with("geometries/") {
            n
        } else {
            format!("geometries/{n}")
        };
        Some(if n.ends_with(".mesh") {
            n
        } else {
            format!("{n}.mesh")
        })
    }
}

impl Nif<'_> {
    /// The slots of geometry block `index` (claim G1), or `None` when it is not a
    /// geometry block. The four slots follow the block's three references; each is a `u8`
    /// 0 or 1, and when 1, three `u32` and a `u32`-length path. The block must end right
    /// after the fourth.
    pub fn slots(&self, index: usize) -> Result<Option<Vec<Slot>>> {
        if self.type_name(index) != Some(GEOMETRY_TYPE) {
            return Ok(None);
        }
        let Some(g) = self.geometry(index)? else {
            return Ok(None);
        };
        let Some(mut c) = self.cursor(index) else {
            return Ok(None);
        };
        let start = g.bound_offset.saturating_sub(c.file_pos());
        c.skip(start.saturating_add(52), "geometry block before its slots")?;
        let mut out = Vec::new();
        for slot in 0..4u8 {
            let offset = c.file_pos();
            match c.u8("slot flag")? {
                0 => continue,
                1 => {}
                f => {
                    return Err(Error::Unsupported {
                        what: "slot flag",
                        offset: offset as u64,
                        value: u64::from(f),
                    })
                }
            }
            let index_count = c.u32("slot index count")?;
            let vertex_count = c.u32("slot vertex count")?;
            let at = c.offset();
            let third = c.u32("slot third u32")?;
            if third != SLOT_THIRD {
                return Err(Error::Unsupported {
                    what: "slot third u32",
                    offset: at,
                    value: u64::from(third),
                });
            }
            let len = c.u32("slot path length")?;
            let len = usize::try_from(len).map_err(|_| Error::Truncated {
                what: "slot path",
                offset: c.offset(),
            })?;
            let path = String::from_utf8_lossy(c.take(len, "slot path")?).into_owned();
            out.push(Slot {
                slot,
                index_count,
                vertex_count,
                path,
                offset,
            });
        }
        if c.remaining() != 0 {
            return Err(Error::Trailing {
                offset: c.offset(),
                len: c.remaining(),
            });
        }
        Ok(Some(out))
    }
}

/// A `.mesh` file read through claim M6 (the whole file must be accounted for).
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    /// Triangle indices, three per triangle, each below the vertex count (claim M1).
    pub indices: Vec<u16>,
    /// The `f32` the positions are scaled by (claim M2).
    pub scale: f32,
    /// The `u32` after the scale (claim M2; meaning not shown).
    pub u32_after_scale: u32,
    /// Positions as stored: three `i16` each (claim M2); see [`Mesh::positions`].
    pub quantized: Vec<[i16; 3]>,
    /// Section 4's words (claim M3); see [`Mesh::normals`].
    pub section_4: Vec<u32>,
    /// File offsets of the index list, the positions and section 4, for evidence.
    pub offsets: MeshOffsets,
}

/// Where the parts [`Mesh`] keeps start in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeshOffsets {
    /// The first index.
    pub indices: usize,
    /// The first position.
    pub positions: usize,
    /// The first word of section 4.
    pub section_4: usize,
}

impl Mesh {
    /// Reads a mesh. Refuses, with the offset, anything the study did not see in every
    /// official mesh: another first `u32`, a section count other than the vertex count
    /// (or 0 where allowed), an index or list value not below the vertex count, an index
    /// count that is not a multiple of 3, and bytes left at the end.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut c = Cur::new(bytes, 0);
        let version = c.u32("mesh first u32")?;
        if version != MESH_VERSION {
            return Err(Error::Unsupported {
                what: "mesh first u32",
                offset: 0,
                value: u64::from(version),
            });
        }
        let at = c.offset();
        let ni = c.u32("index count")?;
        if ni % 3 != 0 {
            return Err(Error::Unsupported {
                what: "index count (not a multiple of 3)",
                offset: at,
                value: u64::from(ni),
            });
        }
        let index_offset = c.file_pos();
        let mut indices = Vec::with_capacity(c.cap(u64::from(ni), 2));
        for _ in 0..ni {
            indices.push(c.u16("index")?);
        }
        let scale_at = c.offset();
        let scale = c.f32("scale")?;
        // Finite and positive in every official mesh (study M2; asserted by the study test).
        if !(scale.is_finite() && scale > 0.0) {
            return Err(Error::Unsupported {
                what: "scale",
                offset: scale_at,
                value: u64::from(scale.to_bits()),
            });
        }
        let u32_after_scale = c.u32("u32 after the scale")?;
        let nv = c.u32("vertex count")?;
        let positions_offset = c.file_pos();
        let mut quantized = Vec::with_capacity(c.cap(u64::from(nv), 6));
        for _ in 0..nv {
            let x = c.u16("position")?;
            let y = c.u16("position")?;
            let z = c.u16("position")?;
            quantized.push([x, y, z].map(|v| i16::from_le_bytes(v.to_le_bytes())));
        }
        for (k, i) in indices.iter().enumerate() {
            if u32::from(*i) >= nv {
                return Err(Error::OutOfRange {
                    what: "index",
                    offset: (index_offset as u64).saturating_add((k as u64).saturating_mul(2)),
                    value: u64::from(*i),
                });
            }
        }
        let mut section_4 = Vec::new();
        let mut section_4_offset = 0;
        for k in 1..=5u8 {
            let at = c.offset();
            let n = c.u32("section count")?;
            let ok = match k {
                1 | 4 | 5 => n == nv,
                _ => n == 0 || n == nv,
            };
            if !ok {
                return Err(Error::Unsupported {
                    what: "section count",
                    offset: at,
                    value: u64::from(n),
                });
            }
            if k == 4 {
                section_4_offset = c.file_pos();
                section_4.reserve(c.cap(u64::from(n), 4));
                for _ in 0..n {
                    section_4.push(c.u32("section 4")?);
                }
            } else {
                let len = usize::try_from(u64::from(n).saturating_mul(4)).unwrap_or(usize::MAX);
                c.skip(len, "section")?;
            }
        }
        // M5: vertex count x the u32 after the scale 4-byte entries, then u16 lists.
        let at = c.offset();
        let entries = c.u32("M5 entry count")?;
        if u64::from(entries) != u64::from(nv).saturating_mul(u64::from(u32_after_scale)) {
            return Err(Error::Unsupported {
                what: "M5 entry count",
                offset: at,
                value: u64::from(entries),
            });
        }
        let len = usize::try_from(u64::from(entries).saturating_mul(4)).unwrap_or(usize::MAX);
        c.skip(len, "M5 entries")?;
        let lists = c.u32("M5 list count")?;
        for _ in 0..lists {
            let n = c.u32("M5 list length")?;
            for _ in 0..n {
                let at = c.offset();
                let v = c.u16("M5 list value")?;
                if u32::from(v) >= nv {
                    return Err(Error::OutOfRange {
                        what: "M5 list value",
                        offset: at,
                        value: u64::from(v),
                    });
                }
            }
        }
        // M6: the end, or u32 m, m x 40 bytes and 4 closing bytes.
        if c.remaining() != 0 {
            let m = c.u32("M6 entry count")?;
            let len = usize::try_from(u64::from(m).saturating_mul(40)).unwrap_or(usize::MAX);
            c.skip(len, "M6 entries")?;
            c.skip(4, "M6 closing bytes")?;
        }
        if c.remaining() != 0 {
            return Err(Error::Trailing {
                offset: c.offset(),
                len: c.remaining(),
            });
        }
        Ok(Self {
            indices,
            scale,
            u32_after_scale,
            quantized,
            section_4,
            offsets: MeshOffsets {
                indices: index_offset,
                positions: positions_offset,
                section_4: section_4_offset,
            },
        })
    }

    /// Vertex count.
    pub fn vertex_count(&self) -> usize {
        self.quantized.len()
    }

    /// Positions in the geometry block's frame: each stored `i16` times
    /// `scale / 32767` (claim M2, grade A: the box of these equals the NIF's stored box
    /// on all 864,598 checked pairs).
    pub fn positions(&self) -> Vec<[f32; 3]> {
        let q = self.scale / 32767.0;
        self.quantized
            .iter()
            .map(|p| p.map(|v| f32::from(v) * q))
            .collect()
    }

    /// Normals, **grade B** (claim M4): section 4's three 10-bit unsigned fields, low
    /// bits first, each mapped from 0..1023 to -1..1. Statistical evidence only (99.86%
    /// unit length, 98.8% facing their triangles); not proven per vertex.
    pub fn normals(&self) -> Vec<[f32; 3]> {
        self.section_4
            .iter()
            .map(|w| [0u32, 10, 20].map(|s| ((w >> s) & 0x3FF) as f32 / 1023.0 * 2.0 - 1.0))
            .collect()
    }
}
