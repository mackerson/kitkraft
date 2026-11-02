//! M7.S4 corpus test: every claim of `docs/research/nif-bounds-2026-11-02.md` on every
//! official model this test can reach.
//!
//! - The loose `.nif` files under `KITKRAFT_DATA_DIR` (the game's `Data` folder, read in
//!   place; 4 today, under `EditorFiles\Primitives`).
//! - Every `.nif` under `KITKRAFT_NIF_DIR`, when set: a folder of models extracted from the
//!   official archives (`kit ba2 extract`). `kitkraft-nif` is L0 and may not read archives
//!   itself; the engine's `mesh_corpus` test reads them in place through `kitkraft-ba2`.
//!
//! Skips with a message when neither is set. Nothing is written anywhere.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
#![allow(clippy::arithmetic_side_effects, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kitkraft_nif::{Bound, Nif, NODE_TYPES, VERSION};

/// Extra-data-like types a prefix's extra list may point at, besides `*ExtraData`.
const EXTRA_LIKE: [&str; 6] = [
    "BSXFlags",
    "BoneTranslations",
    "BSBound",
    "BSConnectPoint::Parents",
    "BSConnectPoint::Children",
    "SkinAttach",
];

fn nif_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            nif_files(&p, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("nif")) {
            out.push(p);
        }
    }
}

#[derive(Default)]
struct Tally {
    files: u64,
    blocks: u64,
    nodes: BTreeMap<String, u64>,
    geometry: [u64; 4],
    skinned: u64,
    null_children: u64,
    revisions: BTreeMap<u32, u64>,
    short_c_lens: BTreeMap<usize, u64>,
    with_box: u64,
    violations: Vec<String>,
}

impl Tally {
    fn fail(&mut self, path: &Path, what: String) {
        if self.violations.len() < 50 {
            self.violations.push(format!("{}: {what}", path.display()));
        } else if self.violations.len() == 50 {
            self.violations.push("... (more)".into());
        }
    }
}

fn det(r: &[[f32; 3]; 3]) -> f64 {
    let r = r.map(|row| row.map(f64::from));
    r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
        - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
}

fn check(path: &Path, bytes: &[u8], t: &mut Tally) {
    t.files += 1;
    // H1 to H6: the file parses and its blocks fill it exactly.
    let nif = match Nif::parse(bytes) {
        Ok(n) => n,
        Err(e) => return t.fail(path, format!("parse: {e}")),
    };
    let h = &nif.header;
    if h.version != VERSION || h.flag != 1 || h.tag != 12 {
        t.fail(
            path,
            format!("header constants {} {} {}", h.version, h.flag, h.tag),
        );
    }
    if ![170, 172, 173, 174, 175].contains(&h.revision) {
        t.fail(path, format!("revision {}", h.revision));
    }
    *t.revisions.entry(h.revision).or_default() += 1;
    *t.short_c_lens.entry(h.short_c.len()).or_default() += 1;
    if h.short_c.last() != Some(&0) {
        t.fail(path, format!("short string c {:?}", h.short_c));
    }
    if !nif.groups.is_empty() || nif.roots != [0] {
        t.fail(
            path,
            format!("groups {:?} roots {:?}", nif.groups, nif.roots),
        );
    }
    t.blocks += nif.blocks.len() as u64;

    let type_of = |r: u32| nif.type_name(r as usize).unwrap_or("?").to_string();
    for i in 0..nif.blocks.len() {
        let ty = nif.type_name(i).unwrap_or("?").to_string();
        let prefix = if NODE_TYPES.contains(&ty.as_str()) {
            // B3: node blocks.
            match nif.node(i) {
                Ok(Some(n)) => {
                    *t.nodes.entry(ty.clone()).or_default() += 1;
                    if ty == "NiNode" && n.trailing != 0 {
                        t.fail(
                            path,
                            format!("block {i} NiNode: {} bytes after children", n.trailing),
                        );
                    }
                    t.null_children += n.children.iter().filter(|c| c.is_none()).count() as u64;
                    n.prefix
                }
                Ok(None) => continue,
                Err(e) => {
                    t.fail(path, format!("block {i} {ty}: {e}"));
                    continue;
                }
            }
        } else if ty == "BSGeometry" {
            // B4 to B6: geometry blocks.
            match nif.geometry(i) {
                Ok(Some(g)) => {
                    let slot = match g.bound {
                        Bound::Stored { sphere, aabb } => {
                            let r = f64::from(sphere.radius);
                            let tol = 1e-3 * r.max(1.0);
                            let he = aabb.half_extent.map(f64::from);
                            let dc = sphere
                                .center
                                .iter()
                                .zip(aabb.center.iter())
                                .map(|(a, b)| (f64::from(*a) - f64::from(*b)).powi(2))
                                .sum::<f64>()
                                .sqrt();
                            let corner = he.iter().map(|v| v * v).sum::<f64>().sqrt();
                            let axis = he.iter().copied().fold(0.0, f64::max);
                            if dc > tol || axis > r + tol || r > corner + tol {
                                t.fail(
                                    path,
                                    format!(
                                        "block {i}: sphere {sphere:?} and box {aabb:?} disagree"
                                    ),
                                );
                            }
                            0
                        }
                        Bound::SphereOnly(_) => 1,
                        Bound::NotStored => 2,
                        Bound::Unrecognised => {
                            t.fail(path, format!("block {i}: unrecognised bound"));
                            3
                        }
                    };
                    t.geometry[slot] += 1;
                    // B5: no stored bound exactly when skinned, and the skin is a skin.
                    if (g.bound == Bound::NotStored) != g.skin.is_some() {
                        t.fail(
                            path,
                            format!("block {i}: bound {:?} with skin {:?}", g.bound, g.skin),
                        );
                    }
                    if let Some(s) = g.skin {
                        t.skinned += 1;
                        if type_of(s) != "BSSkin::Instance" {
                            t.fail(path, format!("block {i}: skin is a {}", type_of(s)));
                        }
                    }
                    // B6: the shader is always set; alpha is unset or an NiAlphaProperty.
                    match g.shader.map(type_of).as_deref() {
                        Some("BSLightingShaderProperty") | Some("BSEffectShaderProperty") => {}
                        other => t.fail(path, format!("block {i}: shader {other:?}")),
                    }
                    if let Some(a) = g.alpha {
                        if type_of(a) != "NiAlphaProperty" {
                            t.fail(path, format!("block {i}: alpha is a {}", type_of(a)));
                        }
                    }
                    let b = nif.blocks[i];
                    if bytes.get(g.bound_offset + 52) != Some(&1)
                        || g.bound_offset + 53 > b.offset + b.len
                    {
                        t.fail(
                            path,
                            format!("block {i}: byte after the references is not 1"),
                        );
                    }
                    g.prefix
                }
                Ok(None) => continue,
                Err(e) => {
                    t.fail(path, format!("block {i} BSGeometry: {e}"));
                    continue;
                }
            }
        } else {
            continue;
        };
        // B2: the shared prefix.
        if prefix.unused_ref.is_some() {
            t.fail(
                path,
                format!("block {i}: reference after the extra data is set"),
            );
        }
        for x in &prefix.extra_data {
            let xt = type_of(*x);
            if !xt.ends_with("ExtraData") && !EXTRA_LIKE.contains(&xt.as_str()) {
                t.fail(path, format!("block {i}: extra data is a {xt}"));
            }
        }
        if let Some(c) = prefix.collision {
            if type_of(c) != "bhkNPCollisionObject" {
                t.fail(path, format!("block {i}: collision is a {}", type_of(c)));
            }
        }
        let tr = prefix.transform;
        let all = tr
            .translation
            .iter()
            .chain(tr.rotation.iter().flatten())
            .chain(std::iter::once(&tr.scale));
        if !all.clone().all(|v| v.is_finite())
            || tr.scale <= 0.0
            || (det(&tr.rotation) - 1.0).abs() > 1e-2
        {
            t.fail(path, format!("block {i}: transform {tr:?}"));
        }
    }
    // C: every geometry is reached exactly once from the root.
    match nif.model_bounds() {
        Ok(b) => {
            if b.unreached_geometries != 0 || b.revisits != 0 {
                t.fail(
                    path,
                    format!(
                        "{} unreached geometries, {} revisits",
                        b.unreached_geometries, b.revisits
                    ),
                );
            }
            if b.aabb.is_some() {
                t.with_box += 1;
            }
        }
        Err(e) => t.fail(path, format!("model bounds: {e}")),
    }
}

#[test]
fn every_claim_holds_on_every_reachable_official_model() {
    let mut files = Vec::new();
    let data = std::env::var_os("KITKRAFT_DATA_DIR").map(PathBuf::from);
    if let Some(d) = &data {
        nif_files(d, &mut files);
    }
    let loose = files.len();
    if let Some(d) = std::env::var_os("KITKRAFT_NIF_DIR") {
        nif_files(Path::new(&d), &mut files);
    }
    if files.is_empty() {
        eprintln!("skipped: set KITKRAFT_DATA_DIR (loose models) or KITKRAFT_NIF_DIR (extracted archives)");
        return;
    }
    files.sort();
    let mut t = Tally::default();
    for p in &files {
        let bytes = std::fs::read(p).unwrap();
        check(p, &bytes, &mut t);
    }
    eprintln!(
        "M7.S4 corpus: {} models ({loose} loose), {} blocks; nodes {:?}; null children {}",
        t.files, t.blocks, t.nodes, t.null_children
    );
    eprintln!(
        "geometry bounds: stored {}, sphere only {}, not stored {} (skinned {}), unrecognised {}; models with a box {}; revisions {:?}; short string c lengths {:?}",
        t.geometry[0], t.geometry[1], t.geometry[2], t.skinned, t.geometry[3], t.with_box, t.revisions, t.short_c_lens
    );
    assert!(
        t.violations.is_empty(),
        "claims broken:\n{}",
        t.violations.join("\n")
    );
}
