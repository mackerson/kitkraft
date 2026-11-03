//! M7.S4 on real data (`KITKRAFT_DATA_DIR`; skips when unset): `mesh.bounds` through
//! `execute` on official models, and every official model read in place from the archives
//! through `kitkraft-ba2` and `kitkraft-nif` (claims of
//! `docs/research/nif-bounds-2026-11-02.md`; the per-field checks are
//! `crates/nif/tests/corpus.rs`).
//!
//! The whole-corpus walk decodes about 1.5 GB, so it is ignored by default; run it in
//! release: `cargo test -p kitkraft-engine --release --test mesh_corpus -- --ignored
//! --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::chunks_exact_to_as_chunks
)]

use std::path::{Path, PathBuf};

use kitkraft_ba2::Archive;
use kitkraft_engine::Engine;
use kitkraft_nif::{Bound, Nif};
use serde_json::{json, Value};

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

fn bounds(engine: &Engine, data: &Path, model: &str) -> Value {
    engine
        .execute(
            "mesh.bounds",
            json!({"model": model, "data_dir": data.display().to_string()}),
        )
        .unwrap()
}

fn has(env: &Value, text: &str) -> bool {
    env["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap_or_default().contains(text))
}

fn near(v: &Value, want: [f64; 3]) -> bool {
    let a = v.as_array().unwrap();
    (0..3).all(|i| (a[i].as_f64().unwrap() - want[i]).abs() < 1e-3)
}

#[test]
fn mesh_bounds_reads_official_models_from_the_archives() {
    let Some(data) = data_dir() else {
        eprintln!("skipped: KITKRAFT_DATA_DIR is not set");
        return;
    };
    let engine = Engine::with_default_handlers();

    // The study's worked example: two flat decal geometries, 4 x 0.7, in Meshes01 only.
    let env = bounds(
        &engine,
        &data,
        "Architecture\\City\\NewAtlantis\\SetDressing\\NA_Addon_Ceiling_FloorDecals01.nif",
    );
    let d = &env["data"];
    assert_eq!(d["source"]["archive"], "Starfield - Meshes01.ba2", "{d}");
    assert_eq!(d["geometries"]["stored"], 2);
    assert!(
        near(&d["bounds"]["min"], [0.0, 0.0, 0.001]),
        "{}",
        d["bounds"]
    );
    assert!(
        near(&d["bounds"]["max"], [4.0, 0.7, 0.002]),
        "{}",
        d["bounds"]
    );
    assert_eq!(env["coverage"]["status"], "complete");
    // Evidence: block 2's bound at 0x268 of the extracted model (the study's offset).
    assert_eq!(env["evidence"][0]["offset"], 0x268);

    // Held by Meshes01 and MeshesPatch: the patch copy is read and both are named.
    let env = bounds(
        &engine,
        &data,
        "Architecture\\City\\NewAtlantis\\SetDressing\\NA_Addon_Ceiling_IlluminatedPanel01.nif",
    );
    assert_eq!(
        env["data"]["source"]["archive"],
        "Starfield - MeshesPatch.ba2"
    );
    assert!(has(&env, "2 archives hold this model"));
    assert!(env["data"]["bounds"].is_object());

    // Skinned only: no box, and the limitation says why.
    let env = bounds(
        &engine,
        &data,
        "Actors\\Character\\FaceGenData\\FaceGeom\\SFBGS003.esm\\00000E90.nif",
    );
    assert_eq!(env["data"]["bounds"], Value::Null);
    assert!(has(&env, "are skinned"), "{}", env["limitations"]);

    // No geometry block at all.
    let env = bounds(
        &engine,
        &data,
        "Terrain\\AkilaCity\\Objects\\AkilaCity.1.-1.-1.nif",
    );
    assert_eq!(env["data"]["bounds"], Value::Null);
    assert!(has(&env, "it has no geometry block"));

    // A loose official model (Data\Meshes has none; the editor primitives are elsewhere).
    let err = engine
        .execute(
            "mesh.bounds",
            json!({"model": "Test\\NotAModel.nif", "data_dir": data.display().to_string()}),
        )
        .unwrap_err();
    assert!(err.to_string().contains("in neither"), "{err}");
}

#[test]
#[ignore = "decodes every official model (about 1.5 GB); run in release"]
fn every_official_model_reads_in_place_from_the_archives() {
    let Some(data) = data_dir() else {
        eprintln!("skipped: KITKRAFT_DATA_DIR is not set");
        return;
    };
    let mut archives: Vec<PathBuf> = std::fs::read_dir(&data)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ba2")))
        .collect();
    archives.sort();
    let (mut models, mut with_box, mut no_geometry, mut skinned_only) = (0u64, 0u64, 0u64, 0u64);
    let mut states = [0u64; 4];
    let mut failures = Vec::new();
    for path in &archives {
        let (archive, mut file) = Archive::open(path).unwrap();
        for (i, entry) in archive.entries.iter().enumerate() {
            if !entry.name.to_ascii_lowercase().ends_with(".nif") {
                continue;
            }
            models += 1;
            let bytes = archive.extract_vec(&mut file, i, 64 << 20).unwrap();
            let result = Nif::parse(&bytes).and_then(|n| n.model_bounds());
            match result {
                Ok(b) => {
                    let c = b.counts();
                    for (s, n) in states.iter_mut().zip(c) {
                        *s += u64::from(n);
                    }
                    if b.unreached_geometries != 0 || b.revisits != 0 || c[3] != 0 {
                        failures.push(format!("{}: {b:?}", entry.name));
                    }
                    if b.aabb.is_some() {
                        with_box += 1;
                    } else if b.geometries.is_empty() {
                        no_geometry += 1;
                    } else if b.geometries.iter().all(|g| g.bound == Bound::NotStored) {
                        skinned_only += 1;
                    } else {
                        failures.push(format!("{}: no box but bounded geometry", entry.name));
                    }
                }
                Err(e) => failures.push(format!("{}: {e}", entry.name)),
            }
        }
    }
    eprintln!(
        "M7.S4 in place: {models} archived models; box {with_box}, no geometry {no_geometry}, skinned only {skinned_only}; geometry bounds stored {}, sphere only {}, not stored {}, unrecognised {}",
        states[0], states[1], states[2], states[3]
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(models, with_box + no_geometry + skinned_only);
}

/// V3 (M7.R3, NIF variant; `docs/design/workbench-performance.md`): the engine time of
/// the Render Window's `mesh.bounds` calls for a cell, one per distinct base: cold (the
/// session's first call builds the archive index) and warm (the same bases again).
#[test]
#[ignore = "release: indexes Starfield.esm; prints the V3 engine timings"]
fn v3_mesh_bounds_for_the_bases_of_a_cell() {
    let Some(data) = data_dir() else {
        eprintln!("skipped: KITKRAFT_DATA_DIR is not set");
        return;
    };
    let d = data.display().to_string();
    for (label, pick) in [
        (
            "CityNewAtlantisApartmentTEMPLATE",
            json!({"cell": "CityNewAtlantisApartmentTEMPLATE", "with_bounds": true, "limit": 1000}),
        ),
        (
            "NewAtlantis 2,1",
            json!({"world": "NewAtlantis", "x": 2, "y": 1, "with_bounds": true, "limit": 1000}),
        ),
    ] {
        let engine = Engine::with_default_handlers();
        engine
            .execute("load", json!({"plugins": ["Starfield.esm"], "data_dir": d}))
            .unwrap();
        let cell = engine.execute("cell", pick).unwrap();
        let items = cell["data"]["items"].as_array().unwrap();
        let mut bases: Vec<(String, Value)> = Vec::new();
        for i in items {
            if let Some(b) = i["base"]["form"].as_str() {
                if !bases.iter().any(|(f, _)| f == b) {
                    bases.push((b.to_string(), i["bounds"].clone()));
                }
            }
        }
        let mut times = Vec::new();
        let (mut boxed, mut unbounded, mut failed, mut agree) = (0, 0, 0, 0);
        let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
        for pass in 0..2 {
            let start = std::time::Instant::now();
            for (base, obnd) in &bases {
                match engine.execute("mesh.bounds", json!({"form": base, "data_dir": d})) {
                    Ok(env) if pass == 0 => {
                        let b = &env["data"]["bounds"];
                        if b.is_object() {
                            boxed += 1;
                            let close = |k: &str| {
                                (0..3).all(|a| {
                                    let (x, y) = (b[k][a].as_f64(), obnd[k][a].as_f64());
                                    matches!((x, y), (Some(x), Some(y)) if (x - y).abs() <= 0.01)
                                })
                            };
                            if close("min") && close("max") {
                                agree += 1;
                            }
                        } else {
                            unbounded += 1;
                        }
                    }
                    Ok(_) => {}
                    Err(e) if pass == 0 => {
                        failed += 1;
                        let m = e.to_string();
                        let why = if m.contains("has no MODL") {
                            "no MODL"
                        } else if m.contains("in neither") {
                            "model not found"
                        } else {
                            "other"
                        };
                        *reasons.entry(why.to_string()).or_default() += 1;
                    }
                    Err(_) => {}
                }
            }
            times.push(start.elapsed());
        }
        eprintln!(
            "V3 {label}: {} references, {} distinct bases; cold {:?}, warm {:?}; model box {boxed} (within 1 cm of OBND: {agree}), no model bound {unbounded}, refused {failed} {reasons:?}",
            items.len(),
            bases.len(),
            times[0],
            times[1]
        );
        assert_eq!(boxed + unbounded + failed, bases.len());
    }
}

// ---- M7.S5 study: the `.mesh` layout (`docs/research/mesh-layout-2026-11-02.md`)

/// One `BSGeometry` slot (study G1): its three `u32` and, for the first slot of a
/// geometry whose box is stored, that box (centre, half extents).
struct Slot {
    first: u32,
    second: u32,
    third: u32,
    stored_box: Option<([f32; 3], [f32; 3])>,
}

/// The archive entry a slot path names (G2): `Geometries\a\b.mesh`, or `a\b` for
/// `geometries\a\b.mesh`.
fn mesh_entry(path: &str) -> String {
    let n = kitkraft_ba2::normalize(path);
    let n = if n.starts_with("geometries/") {
        n
    } else {
        format!("geometries/{n}")
    };
    if n.ends_with(".mesh") {
        n
    } else {
        format!("{n}.mesh")
    }
}

fn u32_at(b: &[u8], p: usize) -> Option<u32> {
    b.get(p..p + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// The slots of every geometry block of a model (G1: after the three references, four
/// slots, each a `u8` 0 or 1, and when 1 three `u32` and a `u32`-length path), or
/// why the block's bytes do not fit.
fn slots_of(bytes: &[u8], out: &mut Vec<(String, Slot)>, g1: &mut G1Tally) -> Result<(), String> {
    let nif = Nif::parse(bytes).map_err(|e| e.to_string())?;
    for i in 0..nif.blocks.len() {
        // A geometry block that does not decode is a failure, never skipped (review L1).
        let g = match nif.geometry(i) {
            Ok(Some(g)) => g,
            Ok(None) => continue,
            Err(e) => return Err(format!("block {i}: {e}")),
        };
        g1.blocks += 1;
        // The crate's reader must read the same slots (M7.S5 code, ask A8).
        let read = nif
            .slots(i)
            .map_err(|e| format!("block {i}: kitkraft-nif slots: {e}"))?
            .unwrap_or_default();
        let first_out = out.len();
        let mut pattern = String::new();
        let block = nif.blocks[i];
        let end = block.offset + block.len;
        let mut p = g.bound_offset + 52;
        let stored_box = match g.bound {
            Bound::Stored { aabb, .. } => Some((aabb.center, aabb.half_extent)),
            _ => None,
        };
        for slot in 0..4 {
            let flag = *bytes.get(p).ok_or("the block ends in a slot flag")?;
            p += 1;
            pattern.push(if flag == 0 { '0' } else { '1' });
            match flag {
                0 => continue,
                1 => {}
                f => return Err(format!("block {i}: slot flag {f}")),
            }
            let first = u32_at(bytes, p).ok_or("short slot")?;
            let second = u32_at(bytes, p + 4).ok_or("short slot")?;
            let third = u32_at(bytes, p + 8).ok_or("short slot")?;
            let len = u32_at(bytes, p + 12).ok_or("short slot")? as usize;
            p += 16;
            let path = bytes.get(p..p + len).ok_or("short path")?;
            p += len;
            *g1.path_lengths.entry(len).or_default() += 1;
            out.push((
                mesh_entry(&String::from_utf8_lossy(path)),
                Slot {
                    first,
                    second,
                    third,
                    stored_box: if slot == 0 { stored_box } else { None },
                },
            ));
        }
        *g1.patterns.entry(pattern).or_default() += 1;
        let walked = out.get(first_out..).unwrap_or(&[]);
        let same = read.len() == walked.len()
            && read.iter().zip(walked).all(|(r, (k, w))| {
                r.index_count == w.first
                    && r.vertex_count == w.second
                    && r.mesh_entry().as_deref() == Some(k.as_str())
            });
        if !same {
            return Err(format!("block {i}: kitkraft-nif reads other slots"));
        }
        if p != end {
            return Err(format!(
                "block {i}: {} bytes after the four slots",
                end as i64 - p as i64
            ));
        }
    }
    Ok(())
}

/// G1's counts over every geometry block.
#[derive(Default, Debug)]
struct G1Tally {
    blocks: u64,
    patterns: std::collections::BTreeMap<String, u64>,
    path_lengths: std::collections::BTreeMap<usize, u64>,
}

/// A `.mesh` walked as claims M1 to M6 describe it.
struct Walked {
    indices: Vec<u16>,
    scale: f32,
    positions: Vec<[i16; 3]>,
    /// The five four-byte-per-vertex sections (M3); the fourth holds the normals (M4).
    sections: [Vec<u32>; 5],
    m5_entries: usize,
    lists: usize,
    /// `None` when the file ends after the extra lists (M6).
    forty_byte_entries: Option<usize>,
}

/// Walks a `.mesh`; `Err` names the first claim that does not hold.
fn walk_mesh(b: &[u8]) -> Result<Walked, String> {
    let mut p = 0usize;
    let rd = |p: &mut usize| -> Result<u32, String> {
        let v = u32_at(b, *p).ok_or(format!("ends at {p}"))?;
        *p += 4;
        Ok(v)
    };
    if rd(&mut p)? != 2 {
        return Err("M1: the first u32 is not 2".into());
    }
    let ni = rd(&mut p)? as usize;
    let raw = b.get(p..p + 2 * ni).ok_or("M1: indices past the end")?;
    let indices: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    p += 2 * ni;
    let scale = f32::from_bits(rd(&mut p)?);
    let u32_after_scale = rd(&mut p)? as usize;
    let nv = rd(&mut p)? as usize;
    let raw = b.get(p..p + 6 * nv).ok_or("M2: positions past the end")?;
    let positions: Vec<[i16; 3]> = raw
        .chunks_exact(6)
        .map(|c| {
            [
                i16::from_le_bytes([c[0], c[1]]),
                i16::from_le_bytes([c[2], c[3]]),
                i16::from_le_bytes([c[4], c[5]]),
            ]
        })
        .collect();
    p += 6 * nv;
    let mut sections: [Vec<u32>; 5] = Default::default();
    for (k, s) in sections.iter_mut().enumerate() {
        let n = rd(&mut p)? as usize;
        let allowed = match k {
            0 | 3 | 4 => n == nv,
            _ => n == 0 || n == nv,
        };
        if !allowed {
            return Err(format!(
                "M3: section {} has {n} entries for {nv} vertices",
                k + 1
            ));
        }
        let raw = b.get(p..p + 4 * n).ok_or("M3: a section past the end")?;
        *s = raw
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        p += 4 * n;
    }
    let m5_entries = rd(&mut p)? as usize;
    if m5_entries != nv * u32_after_scale {
        return Err("M5: the 4-byte entries are not vertices times the u32 after the scale".into());
    }
    p = p
        .checked_add(4 * m5_entries)
        .filter(|e| *e <= b.len())
        .ok_or("M5: entries past the end")?;
    let lists = rd(&mut p)? as usize;
    for _ in 0..lists {
        let c = rd(&mut p)? as usize;
        let raw = b.get(p..p + 2 * c).ok_or("M5: a list past the end")?;
        if raw
            .chunks_exact(2)
            .any(|x| usize::from(u16::from_le_bytes([x[0], x[1]])) >= nv)
        {
            return Err("M5: a list value is not below the vertex count".into());
        }
        p += 2 * c;
    }
    let forty_byte_entries = if p == b.len() {
        None
    } else {
        let m = rd(&mut p)? as usize;
        p = p
            .checked_add(40 * m)
            .filter(|e| *e <= b.len())
            .ok_or("M6: entries past the end")?;
        // M6: four more bytes close the file (their meaning is not shown).
        p = p
            .checked_add(4)
            .filter(|e| *e <= b.len())
            .ok_or("M6: the four closing bytes are missing")?;
        Some(m)
    };
    if p != b.len() {
        return Err(format!("{} bytes left at the end", b.len() - p));
    }
    Ok(Walked {
        indices,
        scale,
        positions,
        sections,
        m5_entries,
        lists,
        forty_byte_entries,
    })
}

/// A word of the fourth section as a vector (M4: three 10-bit unsigned fields, low bits
/// first, each mapped from 0..1023 to -1..1).
fn unpack10(w: u32) -> [f64; 3] {
    let f = |s: u32| f64::from((w >> s) & 0x3FF) / 1023.0 * 2.0 - 1.0;
    [f(0), f(10), f(20)]
}

#[derive(Default, Debug)]
struct MeshTally {
    meshes: u64,
    slots: u64,
    third_not_64: u64,
    counts_ok: u64,
    counts_bad: u64,
    box_checked: u64,
    box_ok: u64,
    normals: u64,
    unit: u64,
    agree: u64,
    compared: u64,
    /// Section 5 under the same reading, for the comparison M4 makes (grade B).
    s5_unit: u64,
    s5_agree: u64,
    indices: u64,
    index_out_of_range: u64,
    not_triples: u64,
    ends_after_lists: u64,
    with_lists: u64,
    with_m5_entries: u64,
    present: [u64; 5],
}

#[test]
#[ignore = "decodes every official .mesh (about 19 GB) and .nif; run in release"]
fn every_official_mesh_matches_the_layout_study() {
    let Some(data) = data_dir() else {
        eprintln!("skipped: KITKRAFT_DATA_DIR is not set");
        return;
    };
    let mut archives: Vec<PathBuf> = std::fs::read_dir(&data)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ba2")))
        .collect();
    archives.sort();
    let mut t = MeshTally::default();
    let mut failures: Vec<String> = Vec::new();

    // G1: the slots every geometry block names.
    let mut slots: std::collections::HashMap<String, Vec<Slot>> = Default::default();
    let mut g1 = G1Tally::default();
    for path in &archives {
        let (archive, mut file) = Archive::open(path).unwrap();
        for (i, e) in archive.entries.iter().enumerate() {
            if !e.name.to_ascii_lowercase().ends_with(".nif") {
                continue;
            }
            let bytes = archive.extract_vec(&mut file, i, 64 << 20).unwrap();
            let mut out = Vec::new();
            if let Err(why) = slots_of(&bytes, &mut out, &mut g1) {
                failures.push(format!("G1 {}: {why}", e.name));
            }
            for (k, s) in out {
                t.slots += 1;
                t.third_not_64 += u64::from(s.third != 64);
                slots.entry(k).or_default().push(s);
            }
        }
    }

    // M1 to M6 on every mesh, and G2/M2/M4 against the slots that name it.
    let mut named = std::collections::HashSet::new();
    let mut kinds: std::collections::BTreeMap<String, u64> = Default::default();
    for path in &archives {
        let (archive, mut file) = Archive::open(path).unwrap();
        for (i, e) in archive.entries.iter().enumerate() {
            let key = kitkraft_ba2::normalize(&e.name);
            if !key.ends_with(".mesh") {
                continue;
            }
            t.meshes += 1;
            if slots.contains_key(&key) {
                named.insert(key.clone());
            }
            let bytes = archive.extract_vec(&mut file, i, 1 << 30).unwrap();
            let w = match walk_mesh(&bytes) {
                Ok(w) => w,
                Err(why) => {
                    *kinds
                        .entry(why.split(':').next().unwrap_or("").to_string())
                        .or_default() += 1;
                    failures.push(format!("{}: {why}", e.name));
                    continue;
                }
            };
            // The crate's reader must read the same mesh (M7.S5 code, ask A8).
            match kitkraft_nif::Mesh::parse(&bytes) {
                Ok(m)
                    if m.indices == w.indices
                        && m.quantized == w.positions
                        && m.section_4 == w.sections[3]
                        && m.scale.to_bits() == w.scale.to_bits() => {}
                Ok(_) => failures.push(format!("{}: kitkraft-nif reads another mesh", e.name)),
                Err(why) => failures.push(format!("{}: kitkraft-nif: {why}", e.name)),
            }
            let nv = w.positions.len();
            t.indices += w.indices.len() as u64;
            t.index_out_of_range +=
                w.indices.iter().filter(|x| usize::from(**x) >= nv).count() as u64;
            t.not_triples += u64::from(w.indices.len() % 3 != 0);
            t.ends_after_lists += u64::from(w.forty_byte_entries.is_none());
            t.with_lists += u64::from(w.lists > 0);
            t.with_m5_entries += u64::from(w.m5_entries > 0);
            for (k, s) in w.sections.iter().enumerate() {
                t.present[k] += u64::from(!s.is_empty());
            }
            // M4: the fourth section's vectors are unit length and face the way the
            // triangles' winding does.
            let mut acc = vec![[0f64; 3]; nv];
            for tri in w.indices.chunks_exact(3) {
                let [a, b2, c] = [tri[0], tri[1], tri[2]].map(usize::from);
                if a >= nv || b2 >= nv || c >= nv {
                    continue;
                }
                let pa = w.positions[a].map(f64::from);
                let pb = w.positions[b2].map(f64::from);
                let pc = w.positions[c].map(f64::from);
                let u = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
                let v = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                for k in [a, b2, c] {
                    for d in 0..3 {
                        acc[k][d] += n[d];
                    }
                }
            }
            for (k, word) in w.sections[3].iter().enumerate() {
                let n = unpack10(*word);
                t.normals += 1;
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                t.unit += u64::from((len - 1.0).abs() < 0.02);
                let f = acc[k];
                if f.iter().any(|x| *x != 0.0) {
                    t.compared += 1;
                    t.agree += u64::from(f[0] * n[0] + f[1] * n[1] + f[2] * n[2] > 0.0);
                }
            }
            for (k, word) in w.sections[4].iter().enumerate() {
                let n = unpack10(*word);
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                t.s5_unit += u64::from((len - 1.0).abs() < 0.02);
                let f = acc[k];
                if f.iter().any(|x| *x != 0.0) {
                    t.s5_agree += u64::from(f[0] * n[0] + f[1] * n[1] + f[2] * n[2] > 0.0);
                }
            }
            // G2: every slot naming this mesh agrees on the counts; M2: a first slot's
            // stored box is the positions' box, dequantized by scale / 32767.
            if let Some(list) = slots.get(&key) {
                let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
                let q = f64::from(w.scale) / 32767.0;
                for p in &w.positions {
                    for d in 0..3 {
                        let v = f64::from(p[d]) * q;
                        lo[d] = lo[d].min(v);
                        hi[d] = hi[d].max(v);
                    }
                }
                for s in list {
                    if s.first as usize == w.indices.len() && s.second as usize == nv {
                        t.counts_ok += 1;
                    } else {
                        t.counts_bad += 1;
                    }
                    if let (Some((c, h)), true) = (s.stored_box, nv > 0) {
                        t.box_checked += 1;
                        let tol = 2.0 * q.abs() + 1e-4;
                        let fits = (0..3).all(|d| {
                            (f64::from(c[d] - h[d]) - lo[d]).abs() <= tol
                                && (f64::from(c[d] + h[d]) - hi[d]).abs() <= tol
                        });
                        t.box_ok += u64::from(fits);
                    }
                }
            }
        }
    }
    let unresolved = slots.keys().filter(|k| !named.contains(*k)).count();
    eprintln!(
        "M7.S5: {t:?}; distinct meshes named {}, unresolved {unresolved}; {} failures {kinds:?}",
        slots.len(),
        failures.len()
    );
    for f in failures.iter().take(20) {
        eprintln!("  {f}");
    }
    eprintln!("G1: {g1:?}");
    assert!(failures.is_empty());
    // G1 (study table): every archived geometry block, its slot patterns and path lengths.
    assert_eq!(g1.blocks, 484_820);
    let patterns: Vec<(&str, u64)> = g1.patterns.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(
        patterns,
        [
            ("1000", 263_524),
            ("1100", 59_304),
            ("1110", 48_812),
            ("1111", 113_180)
        ]
    );
    let lengths: Vec<(usize, u64)> = g1.path_lengths.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(lengths, [(41, 964_697), (57, 16_591)]);
    // The grade-A claims, at the counts the study states (review M1): the test goes red
    // when any of them breaks, not only when a walk fails.
    assert_eq!(t.meshes, 720_957, "M1-M6: every official mesh walked");
    assert_eq!(
        (t.slots, t.third_not_64),
        (981_288, 0),
        "G1: the third u32 is 64"
    );
    assert_eq!(
        (slots.len(), unresolved),
        (399_384, 0),
        "G2: every slot names a mesh"
    );
    assert_eq!(
        (t.counts_ok, t.counts_bad),
        (1_888_804, 0),
        "G2: the counts agree"
    );
    assert_eq!(
        (t.indices, t.index_out_of_range, t.not_triples),
        (1_676_933_949, 0, 0),
        "M1: indices below the vertex count, three per triangle"
    );
    assert_eq!(
        (t.box_checked, t.box_ok),
        (864_598, 864_598),
        "M2: the positions' box is the stored box"
    );
    assert_eq!(
        t.present,
        [720_957, 257_992, 343_029, 720_957, 720_957],
        "M3: the sections present"
    );
    assert_eq!(
        (t.with_m5_entries, t.with_lists, t.ends_after_lists),
        (17_544, 1_744, 6_470),
        "M5, M6"
    );
    // M4 (normals) is grade B: its counts (unit length, facing the winding) are printed
    // above and are not asserted; they are evidence, not a proof.
}
