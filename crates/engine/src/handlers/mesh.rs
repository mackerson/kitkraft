//! `mesh.bounds` (M7.S4, lane R): a model's box from its NIF, found from a base object's
//! `MODL` or given as a path, read from the loose files or the archives of the Data dir.
//!
//! The box is what `kitkraft-nif` reads: the union of the bounds the model's geometry
//! blocks store, carried up the node tree to the model's root frame
//! (`docs/research/nif-bounds-2026-11-02.md`, claims B5 and C1). It is not the base's
//! `OBND` (`cell` with `with_bounds` reads that). `mesh.geometry` (M7.S5) stays `Deferred`
//! in the catalog and has no handler.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use kitkraft_ba2::Archive;
use kitkraft_doc::RecordReader;
use kitkraft_nif::{Bound, Nif};
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
}

impl MeshIndex {
    fn build(data_dir: &Path) -> Result<Self, EngineError> {
        let listing = std::fs::read_dir(data_dir).map_err(|e| {
            failed(
                ID,
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
/// names it (relative to `Meshes`) or one that already starts with `Meshes\`.
pub(crate) fn entry_path(model: &str) -> String {
    let n = kitkraft_ba2::normalize(model.trim_start_matches(['\\', '/']));
    if n.starts_with("meshes/") {
        n
    } else {
        format!("meshes/{n}")
    }
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
) -> Result<BaseModel, EngineError> {
    let loaded = acquire(session, args, ID)?;
    let plugin = args.get("plugin").and_then(Value::as_str);
    let (form, _) = find_form(ID, loaded, form_arg, plugin)?;
    let lo = &loaded.lo;
    let label = lo.form_label(form);
    let (signature, editor_id) = identity(lo, form);
    let v = lo
        .forms
        .winner(form)
        .copied()
        .ok_or_else(|| failed(ID, format!("{label} has no version in the load order")))?;
    let loc = lo
        .record(v.plugin, v.record)
        .copied()
        .ok_or_else(|| failed(ID, format!("{label}: version missing from the index")))?;
    let rec = RecordReader::new()
        .read(lo, v.plugin, &loc)
        .map_err(|e| failed(ID, format!("{label}: {e}")))?;
    let modl = rec
        .subs
        .iter()
        .find(|s| s.signature().bytes() == *b"MODL")
        .ok_or_else(|| {
            failed(
                ID,
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
        return Err(failed(ID, format!("{label}: its MODL is empty")));
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

/// `mesh.bounds`.
pub(super) fn bounds(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let model_arg = args.get("model").and_then(Value::as_str);
    let form_arg = args.get("form").and_then(Value::as_str);
    let mut evidence = Vec::new();
    let mut limitations = Vec::new();
    let base = match (model_arg, form_arg) {
        (Some(_), Some(_)) => return Err(failed(ID, "pass `model` or `form`, not both")),
        (None, None) => {
            return Err(failed(
                ID,
                "pass `model` (a path as a MODL names it) or `form` (a base object)",
            ))
        }
        (Some(_), None) => None,
        (None, Some(f)) => Some(model_of_form(session, args, f)?),
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
        .ok_or_else(|| failed(ID, "no Data dir: pass `data_dir` or set KITKRAFT_DATA_DIR"))?;
    let wanted = entry_path(&model);
    let rel: PathBuf = wanted.split('/').collect();
    let loose = data_dir.join(&rel);

    if session
        .get::<MeshIndex>()
        .is_none_or(|i| i.data_dir != data_dir)
    {
        let built = MeshIndex::build(&data_dir)?;
        session.insert(built);
    }
    let index = session
        .get::<MeshIndex>()
        .ok_or_else(|| failed(ID, "the archive index did not build"))?;
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
                ID,
                format!(
                    "{}: {len} bytes, above the {MAX_MODEL_BYTES}-byte cap",
                    loose.display()
                ),
            ));
        }
        let bytes =
            std::fs::read(&loose).map_err(|e| failed(ID, format!("{}: {e}", loose.display())))?;
        let shown = rel.display().to_string();
        let source = json!({"kind": "loose", "file": shown, "archive": null});
        if !also_in.is_empty() {
            limitations.push(format!(
                "read the loose file; {} archive(s) also hold this model ({})",
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
                ID,
                format!(
                    "model `{model}` ({wanted}) is in neither the loose files nor the archives of {}",
                    data_dir.display()
                ),
            ));
        };
        let Some((name, path, archive)) = index.archives.get(a) else {
            return Err(failed(ID, "the archive index is inconsistent"));
        };
        let mut f = File::open(path).map_err(|err| failed(ID, format!("{name}: {err}")))?;
        let bytes = archive
            .extract_vec(&mut f, e, MAX_MODEL_BYTES)
            .map_err(|err| failed(ID, format!("{name}: {err}")))?;
        let entry = archive
            .entries
            .get(e)
            .map_or_else(|| wanted.clone(), |x| x.name.clone());
        if also_in.len() > 1 {
            limitations.push(format!(
                "{} archives hold this model ({}); read {name}'s copy: a plugin's own archive over the base game's, a base Patch archive over the other base ones (the game's own archive order is not modelled)",
                also_in.len(),
                also_in.join(", ")
            ));
        }
        let source = json!({"kind": "archive", "file": entry, "archive": name});
        (bytes, format!("{name}/{entry}"), source)
    };

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
