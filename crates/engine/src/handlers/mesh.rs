//! `mesh.bounds` (M7.S4, lane R): a model's box from its NIF, found from a base object's
//! `MODL` or given as a path, read from the loose files or the archives of the Data dir.
//!
//! The box is what `kitkraft-nif` reads: the union of the bounds the model's geometry
//! blocks store, carried up the node tree to the model's root frame
//! (`docs/research/nif-bounds-2026-11-02.md`, claims B5 and C1). It is not the base's
//! `OBND` (`cell` with `with_bounds` reads that). `mesh.geometry` (M7.S5) is [`geometry`];
//! the integrator's A8 seam commit flips the catalog from `Deferred` to `Implemented` and
//! adds its dispatch row.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use kitkraft_ba2::Archive;
use kitkraft_doc::RecordReader;
use kitkraft_nif::{Bound, Mesh, Nif};
use serde_json::{json, Value};

use super::inspect::find_form;
use super::load::acquire;
use super::refs::identity;
use super::{data_dir_source, failed};
use crate::envelope::{Coverage, CoverageStatus, Evidence};
use crate::exec::EngineError;
use crate::{Outcome, Session};

const ID: &str = "mesh.bounds";

/// Largest model read, in bytes. The largest official `.nif` is 3,466,437 bytes (study,
/// 2026-11-02); the cap stops a hostile archive entry from declaring gigabytes.
pub(crate) const MAX_MODEL_BYTES: u64 = 64 << 20;

/// At most this many geometry blocks get an evidence range each.
const MAX_EVIDENCE: usize = 1_000;

/// The study's cross-check of the model box against `OBND` (claim C1b), repeated on every
/// result so the box is never read as the base's `OBND`.
pub(crate) const OBND_NOTE: &str = "the box is the union of the bounds the model's geometry blocks store, in the model's root frame (docs/research/nif-bounds-2026-11-02.md, C1); it is not the base's OBND: over 33,684 official base objects it matched OBND within 1 cm for 23,261 (69%) and differed for the rest";

/// The `.nif` entries of the Data dir's archives, read once per session and Data dir.
struct MeshIndex {
    data_dir: PathBuf,
    /// Archives holding at least one `.nif`, in name order: (file name, path, archive).
    archives: Vec<(String, PathBuf, Archive)>,
    /// Normalized entry path -> (archive index, entry index), in archive name order.
    by_path: HashMap<String, Vec<(usize, usize)>>,
    /// Archives that did not open, with the reason.
    unreadable: Vec<String>,
    /// The `.mesh` entries, indexed on the first `mesh.geometry` call.
    meshes: Option<HashMap<String, Vec<(usize, usize)>>>,
}

impl MeshIndex {
    fn build(data_dir: &Path, id: &str) -> Result<Self, EngineError> {
        let listing = std::fs::read_dir(data_dir).map_err(|e| {
            failed(
                id,
                format!("cannot list the Data dir {}: {e}", data_dir.display()),
            )
        })?;
        let mut paths: Vec<PathBuf> = listing
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ba2")))
            .collect();
        paths.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
        let mut index = MeshIndex {
            data_dir: data_dir.to_path_buf(),
            archives: Vec::new(),
            by_path: HashMap::new(),
            unreadable: Vec::new(),
            meshes: None,
        };
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let archive = match Archive::open(&path) {
                Ok((a, _)) => a,
                Err(e) => {
                    index.unreadable.push(format!("{name}: {e}"));
                    continue;
                }
            };
            let models: Vec<(usize, String)> = archive
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| (i, kitkraft_ba2::normalize(&e.name)))
                .filter(|(_, n)| n.ends_with(".nif"))
                .collect();
            if models.is_empty() {
                continue;
            }
            let at = index.archives.len();
            for (i, n) in models {
                index.by_path.entry(n).or_default().push((at, i));
            }
            index.archives.push((name, path, archive));
        }
        Ok(index)
    }

    /// Indexes the `.mesh` entries of the kept archives, once (`mesh.geometry`). Every
    /// archive that holds a `.mesh` also holds a `.nif` in the official data, so the kept
    /// archives cover them; a `.mesh`-only archive would not be searched (a limitation).
    fn index_meshes(&mut self) {
        if self.meshes.is_some() {
            return;
        }
        let mut map: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        for (a, (_, _, archive)) in self.archives.iter().enumerate() {
            for (i, e) in archive.entries.iter().enumerate() {
                let n = kitkraft_ba2::normalize(&e.name);
                if n.ends_with(".mesh") {
                    map.entry(n).or_default().push((a, i));
                }
            }
        }
        self.meshes = Some(map);
    }

    /// The `.mesh` entry `entry` (normalized), from the archive [`rank`] prefers, with
    /// that archive's name; `None` when no indexed archive holds it or it does not read.
    fn read_mesh(&self, entry: &str, cap: u64) -> Option<(String, Vec<u8>)> {
        let holders = self.meshes.as_ref()?.get(entry)?;
        let (a, e) = holders
            .iter()
            .enumerate()
            .max_by_key(|(order, (a, _))| {
                let name = self.archives.get(*a).map_or("", |(n, _, _)| n.as_str());
                (rank(name), *order)
            })
            .map(|(_, h)| *h)?;
        let (name, path, archive) = self.archives.get(a)?;
        let mut f = File::open(path).ok()?;
        let bytes = archive.extract_vec(&mut f, e, cap).ok()?;
        Some((name.clone(), bytes))
    }
}

/// Which archive's copy is read when several hold a model: a plugin's own archive
/// (`<Plugin> - *.ba2`, not `Starfield - *`) over the base game's, a base `*Patch*`
/// archive over the other base ones, otherwise the last by name. The game's own archive
/// order is not modelled; the result names every archive that holds the model.
pub(crate) fn rank(archive: &str) -> u8 {
    let lower = archive.to_ascii_lowercase();
    if !lower.starts_with("starfield - ") {
        2
    } else if lower.contains("patch") {
        1
    } else {
        0
    }
}

/// `meshes/<model>`, normalized as archive entry names are, from a model path as a `MODL`
/// names it (relative to `Meshes`) or one that already starts with `Meshes\`. Refused
/// before any file is opened (review M1): an empty path, a leading separator, a drive or
/// device prefix (`:` or `?` in a segment), and empty, `.` or `..` segments, so the path
/// can only name a file under `Data\Meshes` or an archive entry under `meshes/`.
#[cfg(test)]
pub(crate) fn entry_path(model: &str) -> Result<String, EngineError> {
    entry_path_in(ID, model)
}

/// [`entry_path`], its errors naming command `id`.
fn entry_path_in(id: &str, model: &str) -> Result<String, EngineError> {
    let refuse = || {
        failed(
            id,
            format!(
                "model `{model}` is not a path inside Meshes (relative, no `.` or `..` segments, no drive)"
            ),
        )
    };
    let n = kitkraft_ba2::normalize(model);
    let bad = |s: &str| s.is_empty() || s == "." || s == ".." || s.contains([':', '?']);
    if n.split('/').any(bad) {
        return Err(refuse());
    }
    Ok(if n.starts_with("meshes/") {
        n
    } else {
        format!("meshes/{n}")
    })
}

/// What a base object names as its model.
struct BaseModel {
    model: String,
    form: String,
    signature: Option<String>,
    editor_id: Option<String>,
    evidence: Evidence,
}

/// The base's `MODL` text and its evidence.
fn model_of_form(
    session: &mut Session,
    args: &Value,
    form_arg: &str,
    id: &str,
) -> Result<BaseModel, EngineError> {
    let loaded = acquire(session, args, id)?;
    let plugin = args.get("plugin").and_then(Value::as_str);
    let (form, _) = find_form(id, loaded, form_arg, plugin)?;
    let lo = &loaded.lo;
    let label = lo.form_label(form);
    let (signature, editor_id) = identity(lo, form);
    let v = lo
        .forms
        .winner(form)
        .copied()
        .ok_or_else(|| failed(id, format!("{label} has no version in the load order")))?;
    let loc = lo
        .record(v.plugin, v.record)
        .copied()
        .ok_or_else(|| failed(id, format!("{label}: version missing from the index")))?;
    let rec = RecordReader::new()
        .read(lo, v.plugin, &loc)
        .map_err(|e| failed(id, format!("{label}: {e}")))?;
    let modl = rec
        .subs
        .iter()
        .find(|s| s.signature().bytes() == *b"MODL")
        .ok_or_else(|| {
            failed(
                id,
                format!(
                    "{label} ({}) has no MODL: no model to read",
                    signature.as_deref().unwrap_or("?")
                ),
            )
        })?;
    let model = String::from_utf8_lossy(modl.data())
        .trim_end_matches('\0')
        .to_string();
    if model.is_empty() {
        return Err(failed(id, format!("{label}: its MODL is empty")));
    }
    // A compressed record's subrecords carry no file offsets: point at the record then.
    let evidence = match modl.data_offset() {
        Some(offset) => Evidence {
            file: rec.file.clone(),
            offset,
            length: modl.data().len() as u64,
            record: Some(rec.label()),
            subrecord: Some("MODL".to_string()),
        },
        None => Evidence {
            file: rec.file.clone(),
            offset: loc.offset,
            length: kitkraft_esm::HEADER_LEN as u64,
            record: Some(rec.label()),
            subrecord: None,
        },
    };
    Ok(BaseModel {
        model,
        form: label,
        signature,
        editor_id,
        evidence,
    })
}

/// What `mesh.bounds` and `mesh.geometry` share: the model found, read, and named.
struct ModelRead {
    model: String,
    base: Option<BaseModel>,
    bytes: Vec<u8>,
    /// `<archive>/<entry>` or the loose path, for evidence.
    file: String,
    source: Value,
    also_in: Vec<String>,
    limitations: Vec<String>,
    evidence: Vec<Evidence>,
}

impl ModelRead {
    /// The base's form label, signature and EditorID, when a `form` was given.
    fn form(&self) -> Option<&str> {
        self.base.as_ref().map(|b| b.form.as_str())
    }
}

/// Finds the model a `model` path or a base `form` names and reads it from the loose
/// files or the archives (errors name command `id`).
fn read_model(session: &mut Session, args: &Value, id: &str) -> Result<ModelRead, EngineError> {
    let model_arg = args.get("model").and_then(Value::as_str);
    let form_arg = args.get("form").and_then(Value::as_str);
    let mut evidence = Vec::new();
    let mut limitations = Vec::new();
    let base = match (model_arg, form_arg) {
        (Some(_), Some(_)) => return Err(failed(id, "pass `model` or `form`, not both")),
        (None, None) => {
            return Err(failed(
                id,
                "pass `model` (a path as a MODL names it) or `form` (a base object)",
            ))
        }
        (Some(_), None) => None,
        (None, Some(f)) => Some(model_of_form(session, args, f, id)?),
    };
    let model = match (&base, model_arg) {
        (Some(b), _) => b.model.clone(),
        (None, Some(m)) => m.to_string(),
        (None, None) => String::new(),
    };
    if let Some(b) = &base {
        evidence.push(b.evidence.clone());
    }

    let (data_dir, _) = data_dir_source(args)
        .ok_or_else(|| failed(id, "no Data dir: pass `data_dir` or set KITKRAFT_DATA_DIR"))?;
    let wanted = entry_path_in(id, &model)?;
    let rel: PathBuf = wanted.split('/').collect();
    let loose = data_dir.join(&rel);

    if session
        .get::<MeshIndex>()
        .is_none_or(|i| i.data_dir != data_dir)
    {
        let built = MeshIndex::build(&data_dir, id)?;
        session.insert(built);
    }
    let index = session
        .get::<MeshIndex>()
        .ok_or_else(|| failed(id, "the archive index did not build"))?;
    if !index.unreadable.is_empty() {
        limitations.push(format!(
            "{} archive(s) of the Data dir did not open and were not searched: {}",
            index.unreadable.len(),
            index.unreadable.join("; ")
        ));
    }
    let holders: Vec<(usize, usize)> = index.by_path.get(&wanted).cloned().unwrap_or_default();
    let also_in: Vec<String> = holders
        .iter()
        .filter_map(|(a, _)| index.archives.get(*a).map(|(n, _, _)| n.clone()))
        .collect();

    let (bytes, file, source) = if loose.is_file() {
        let len = std::fs::metadata(&loose).map_or(u64::MAX, |m| m.len());
        if len > MAX_MODEL_BYTES {
            return Err(failed(
                id,
                format!(
                    "{}: {len} bytes, above the {MAX_MODEL_BYTES}-byte cap",
                    loose.display()
                ),
            ));
        }
        let bytes =
            std::fs::read(&loose).map_err(|e| failed(id, format!("{}: {e}", loose.display())))?;
        let shown = rel.display().to_string();
        let source = json!({"kind": "loose", "file": shown, "archive": null});
        if !also_in.is_empty() {
            limitations.push(format!(
                "read the loose file; {} archive(s) also hold this model ({}); loose over archives is our rule (the game's order is not modelled)",
                also_in.len(),
                also_in.join(", ")
            ));
        }
        (bytes, shown, source)
    } else {
        let pick = holders
            .iter()
            .enumerate()
            .max_by_key(|(order, (a, _))| {
                let name = index.archives.get(*a).map_or("", |(n, _, _)| n.as_str());
                (rank(name), *order)
            })
            .map(|(_, h)| *h);
        let Some((a, e)) = pick else {
            return Err(failed(
                id,
                format!(
                    "model `{model}` ({wanted}) is in neither the loose files nor the archives of {}",
                    data_dir.display()
                ),
            ));
        };
        let Some((name, path, archive)) = index.archives.get(a) else {
            return Err(failed(id, "the archive index is inconsistent"));
        };
        let mut f = File::open(path).map_err(|err| failed(id, format!("{name}: {err}")))?;
        let bytes = archive
            .extract_vec(&mut f, e, MAX_MODEL_BYTES)
            .map_err(|err| failed(id, format!("{name}: {err}")))?;
        let entry = archive
            .entries
            .get(e)
            .map_or_else(|| wanted.clone(), |x| x.name.clone());
        if also_in.len() > 1 {
            limitations.push(format!(
                "{} archives hold this model ({}); read {name}'s copy: a plugin's own archive over the base game's, a base Patch archive over the other base ones, ties by archive name (our rule: the game's own archive order is not modelled); every archive in the Data folder is searched, whether or not its plugin is in a load order",
                also_in.len(),
                also_in.join(", ")
            ));
        }
        let source = json!({"kind": "archive", "file": entry, "archive": name});
        (bytes, format!("{name}/{entry}"), source)
    };

    Ok(ModelRead {
        model,
        base,
        bytes,
        file,
        source,
        also_in,
        limitations,
        evidence,
    })
}

/// `mesh.bounds`.
pub(super) fn bounds(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let ModelRead {
        model,
        base,
        bytes,
        file,
        source,
        also_in,
        mut limitations,
        mut evidence,
    } = read_model(session, args, ID)?;
    let nif = Nif::parse(&bytes).map_err(|e| failed(ID, format!("{file}: {e}")))?;
    let mb = nif
        .model_bounds()
        .map_err(|e| failed(ID, format!("{file}: {e}")))?;
    let [stored, sphere_only, not_stored, unrecognised] = mb.counts();
    let total = mb.geometries.len();
    for g in mb.geometries.iter().take(MAX_EVIDENCE) {
        if matches!(g.bound, Bound::Stored { .. } | Bound::SphereOnly(_)) {
            evidence.push(Evidence {
                file: file.clone(),
                offset: g.bound_offset as u64,
                length: 40,
                record: None,
                subrecord: Some(format!("block {} BSGeometry bound", g.block)),
            });
        }
    }
    if total > MAX_EVIDENCE {
        limitations.push(format!(
            "evidence lists the first {MAX_EVIDENCE} of {total} geometry blocks"
        ));
    }
    if total == 0 {
        limitations.push("no bound in the model: it has no geometry block (study C1: 19,436 of the 120,547 official models)".to_string());
    } else if mb.aabb.is_none() && not_stored > 0 && unrecognised == 0 {
        limitations.push(format!(
            "no bound in the model: all {total} geometry block(s) are skinned, and skinned geometry stores none (study B5; 5,830 of the official models)"
        ));
    } else if not_stored > 0 {
        limitations.push(format!(
            "{not_stored} skinned geometry block(s) store no bound and are not in the box (study B5)"
        ));
    }
    if sphere_only > 0 {
        limitations.push(format!(
            "{sphere_only} geometry block(s) store only a sphere; each adds its sphere's enclosing box (study B5)"
        ));
    }
    if unrecognised > 0 {
        limitations.push(format!(
            "{unrecognised} geometry block(s) hold bound values outside the study's three patterns (no official model has them): left out of the box"
        ));
    }
    if mb.unreached_geometries > 0 || mb.revisits > 0 {
        limitations.push(format!(
            "{} geometry block(s) are not reached from the root and {} child reference(s) repeat (none in the official models): the box covers the reached ones only",
            mb.unreached_geometries, mb.revisits
        ));
    }
    if mb.box_overflow {
        limitations
            .push("the box overflows f32 (the placements are huge): no box is given".to_string());
    }
    limitations.push(OBND_NOTE.to_string());

    let bounds = mb.aabb.map(|b| json!({"min": b.min, "max": b.max}));
    let summary = match &mb.aabb {
        Some(b) => format!(
            "{model}: box {:?} to {:?} from {stored} stored and {sphere_only} sphere-only bound(s) of {total} geometry block(s)",
            b.min, b.max
        ),
        None => format!("{model}: no bound ({total} geometry block(s))"),
    };
    let complete = mb.aabb.is_some() && unrecognised == 0 && not_stored == 0;
    let (form, base_signature, base_editor_id) = match base {
        Some(b) => (Some(b.form), b.signature, b.editor_id),
        None => (None, None, None),
    };
    Ok(Outcome {
        data: json!({
            "model": model,
            "form": form,
            "base_signature": base_signature,
            "base_editor_id": base_editor_id,
            "source": source,
            "also_in": also_in,
            "bounds": bounds,
            "frame": "model root",
            "geometries": {
                "total": total,
                "stored": stored,
                "sphere_only": sphere_only,
                "not_stored": not_stored,
                "unrecognised": unrecognised,
                "unreached": mb.unreached_geometries,
            },
            "summary": summary,
        }),
        coverage: Coverage {
            status: if complete {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: vec!["geometry vertex data (.mesh files, M7.S5)".to_string()],
        },
        limitations,
        evidence,
    })
}

/// `mesh.geometry`'s id.
const GEOMETRY_ID: &str = "mesh.geometry";

/// Most vertices one `mesh.geometry` reply carries; a mesh that would pass it is skipped
/// with a limitation line (V5). The largest official model's first slots hold fewer.
pub(crate) const MAX_GEOMETRY_VERTICES: usize = 2_000_000;

/// Most indices one reply carries (review M2): with [`MAX_GEOMETRY_VERTICES`] this bounds
/// the raw buffers at 72 MB (24 bytes a vertex, 4 an index). The largest official model's
/// first slots hold 3,647,352 indices and 1,485,580 vertices (study test, 2026-11-02).
pub(crate) const MAX_GEOMETRY_INDICES: usize = 6_000_000;

/// Largest `.mesh` read, in bytes (the largest official one is under 64 MiB).
const MAX_MESH_BYTES: u64 = 256 << 20;

/// Standard base64 (RFC 4648 alphabet, `=` padding) of `bytes`.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let sym = |v: u32| char::from(ALPHABET.get((v & 63) as usize).copied().unwrap_or(b'A'));
    let mut out = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for chunk in bytes.chunks(3) {
        let b = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (b(0) << 16) | (b(1) << 8) | b(2);
        out.push(sym(n >> 18));
        out.push(sym(n >> 12));
        out.push(if chunk.len() > 1 { sym(n >> 6) } else { '=' });
        out.push(if chunk.len() > 2 { sym(n) } else { '=' });
    }
    out
}

/// Applies a 3x4 row-major affine map to a point.
fn place(m: &[[f64; 4]; 3], p: [f32; 3]) -> [f32; 3] {
    let p = p.map(f64::from);
    m.map(|r| (r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3]) as f32)
}

/// Turns a normal by a map's 3x3 part and renormalizes it (a zero vector stays zero).
fn turn(m: &[[f64; 4]; 3], n: [f32; 3]) -> [f32; 3] {
    let n = n.map(f64::from);
    let v = m.map(|r| r[0] * n[0] + r[1] * n[1] + r[2] * n[2]);
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 0.0 {
        v.map(|x| (x / len) as f32)
    } else {
        [0.0; 3]
    }
}

/// `mesh.geometry` (M7.S5): the model's triangles in its root frame, as base64
/// little-endian buffers (the director's geometry-encoding decision, `plan/execution-plan.md` "M6" decisions item 2 and the M7.S5 row; owner-confirmed 2026-11-01): `positions` and `normals` `f32` x, y, z
/// per vertex, `indices` `u32` per corner, three per triangle. Each reached geometry
/// block's first slot is read (claims G1 and G2 of
/// `docs/research/mesh-layout-2026-11-02.md`), placed by the block's own transform and
/// its ancestors' (claim C1 of the NIF study). Normals are grade B (claim M4).
///
/// Dispatched once the catalog entry is `Implemented` and `handlers.rs` has its row (the
/// integrator's A8 seam commit); `mesh_tests.rs` `mesh_geometry_through_the_catalog` holds
/// in both states.
#[allow(dead_code)] // Until A8's dispatch row: the seam commit applies r-A8-dead-code.patch.
pub(super) fn geometry(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let started = std::time::Instant::now();
    let r = read_model(session, args, GEOMETRY_ID)?;
    let form = r.form().map(str::to_string);
    let mut limitations = r.limitations;
    let mut evidence = r.evidence;
    let nif = Nif::parse(&r.bytes).map_err(|e| failed(GEOMETRY_ID, format!("{}: {e}", r.file)))?;
    let mb = nif
        .model_bounds()
        .map_err(|e| failed(GEOMETRY_ID, format!("{}: {e}", r.file)))?;
    let index = session
        .get_mut::<MeshIndex>()
        .ok_or_else(|| failed(GEOMETRY_ID, "the archive index did not build"))?;
    index.index_meshes();
    let mut positions: Vec<u8> = Vec::new();
    let mut normals: Vec<u8> = Vec::new();
    let mut indices: Vec<u8> = Vec::new();
    let mut vertices = 0usize;
    let mut index_total = 0usize;
    let mut parts = Vec::new();
    let mut skipped = 0usize;
    for g in &mb.geometries {
        let block = usize::try_from(g.block).unwrap_or(usize::MAX);
        // A skinned block stores no box (NIF study B5), so the mesh study's box check
        // (M2) does not show the frame of its positions: skipped, not guessed (review M1).
        if matches!(g.bound, Bound::NotStored | Bound::Unrecognised) {
            limitations.push(format!(
                "block {block} is skinned or stores no recognised bound (NIF study B5): the mesh study does not show the frame of its positions, so its mesh is skipped"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        }
        let slots = match nif.slots(block) {
            Ok(s) => s.unwrap_or_default(),
            Err(e) => {
                limitations.push(format!(
                    "block {block}: its slots do not read ({e}): skipped"
                ));
                skipped = skipped.saturating_add(1);
                continue;
            }
        };
        let Some(first) = slots.into_iter().find(|s| s.slot == 0) else {
            limitations.push(format!("block {block}: no first slot: no mesh read"));
            skipped = skipped.saturating_add(1);
            continue;
        };
        let Some(entry) = first.mesh_entry() else {
            limitations.push(format!(
                "block {block}: the first slot names no mesh file (geometry kept inside the model, not decoded)"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        };
        let nv = usize::try_from(first.vertex_count).unwrap_or(usize::MAX);
        let ni = usize::try_from(first.index_count).unwrap_or(usize::MAX);
        if index_total.saturating_add(ni) > MAX_GEOMETRY_INDICES {
            limitations.push(format!(
                "block {block}: {entry} skipped: its {ni} indices would pass the {MAX_GEOMETRY_INDICES}-index budget"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        }
        if vertices.saturating_add(nv) > MAX_GEOMETRY_VERTICES {
            limitations.push(format!(
                "block {block}: {entry} skipped: its {nv} vertices would pass the {MAX_GEOMETRY_VERTICES}-vertex budget"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        }
        let Some((name, bytes)) = index.read_mesh(&entry, MAX_MESH_BYTES) else {
            limitations.push(format!(
                "block {block}: {entry} is in no archive of the Data dir: skipped"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        };
        let mesh = match Mesh::parse(&bytes) {
            Ok(m) => m,
            Err(e) => {
                limitations.push(format!("block {block}: {name}/{entry}: {e}: skipped"));
                skipped = skipped.saturating_add(1);
                continue;
            }
        };
        if mesh.indices.len() != first.index_count as usize || mesh.vertex_count() != nv {
            limitations.push(format!(
                "block {block}: {entry}: the slot's counts ({}, {nv}) are not the mesh's ({}, {}): skipped",
                first.index_count,
                mesh.indices.len(),
                mesh.vertex_count()
            ));
            skipped = skipped.saturating_add(1);
            continue;
        }
        let base = u32::try_from(vertices).unwrap_or(u32::MAX);
        for p in mesh.positions() {
            for v in place(&g.to_root, p) {
                positions.extend_from_slice(&v.to_le_bytes());
            }
        }
        for n in mesh.normals() {
            for v in turn(&g.to_root, n) {
                normals.extend_from_slice(&v.to_le_bytes());
            }
        }
        for i in &mesh.indices {
            indices.extend_from_slice(&base.saturating_add(u32::from(*i)).to_le_bytes());
        }
        evidence.push(Evidence {
            file: format!("{name}/{entry}"),
            offset: mesh.offsets.positions as u64,
            length: (nv as u64).saturating_mul(6),
            record: None,
            subrecord: Some("positions".to_string()),
        });
        parts.push(json!({
            "block": g.block,
            "mesh": entry,
            "archive": name,
            "vertex_offset": vertices,
            "vertex_count": nv,
            "index_count": mesh.indices.len(),
        }));
        vertices = vertices.saturating_add(nv);
        index_total = index_total.saturating_add(mesh.indices.len());
    }
    if mb.geometries.is_empty() {
        limitations.push("the model has no geometry block: no triangles".to_string());
    }
    if mb.unreached_geometries > 0 || mb.revisits > 0 {
        limitations.push(format!(
            "{} geometry block(s) are not reached from the root and {} child reference(s) repeat (none in the official models): their meshes are not read",
            mb.unreached_geometries, mb.revisits
        ));
    }
    if skipped > 0 {
        limitations.push(format!(
            "{skipped} geometry block(s) skipped, each named above"
        ));
    }
    limitations.push("normals are grade B (docs/research/mesh-layout-2026-11-02.md, M4: 99.86% unit length and 98.8% facing their triangles over the official meshes, not proven per vertex)".to_string());
    limitations.push("only each geometry block's first slot is read; the other three slots name further meshes of the block whose use is not shown".to_string());
    let triangles = indices.len() / 12;
    let envelope_bytes = positions
        .len()
        .saturating_add(normals.len())
        .saturating_add(indices.len());
    let summary = format!(
        "{}: {vertices} vertices, {triangles} triangles from {} mesh(es), {skipped} skipped; {} ms",
        r.model,
        parts.len(),
        started.elapsed().as_millis()
    );
    Ok(Outcome {
        data: json!({
            "model": r.model,
            "form": form,
            "source": r.source,
            "frame": "model root",
            "encoding": "base64 little-endian: positions and normals f32 x, y, z per vertex; indices u32, three per triangle",
            "vertex_count": vertices,
            "index_count": indices.len() / 4,
            "raw_bytes": envelope_bytes,
            "meshes": parts,
            "positions": base64(&positions),
            "normals": base64(&normals),
            "indices": base64(&indices),
            "summary": summary,
        }),
        coverage: Coverage {
            status: if skipped == 0
                && !mb.geometries.is_empty()
                && mb.unreached_geometries == 0
                && mb.revisits == 0
            {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: vec!["the mesh sections and entries the study names by place".to_string()],
        },
        limitations,
        evidence,
    })
}
