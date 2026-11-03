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
    clippy::arithmetic_side_effects
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
