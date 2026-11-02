//! `mesh.bounds` (M7.S4) and `mesh.geometry` through `execute` with JSON, on synthetic
//! models (built from the layout in `docs/research/nif-bounds-2026-11-02.md`, no game
//! bytes) placed loose or in archives packed by `kitkraft-ba2`, and a synthetic master
//! whose static names one of them in `MODL`. Neither command writes.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::{Path, PathBuf};

use kitkraft_esm::build::HeaderSpec;
use kitkraft_esm::cells::{self, Parent, PlacementStyle};
use kitkraft_esm::tes4::FLAG_MASTER;
use kitkraft_esm::Plugin;
use serde_json::{json, Value};

use super::mesh::{entry_path, rank, OBND_NOTE};
use crate::edit::patch_tests::dir;
use crate::handlers::place_tests::{rec, sub, zstr};
use crate::{execute, find_command, Engine, Status};

const NULL: u32 = u32::MAX;

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

/// The 72-byte block prefix (claim B2) with no extra data: name, placement `t`, scale 1.
fn prefix(out: &mut Vec<u8>, name: u32, t: [f32; 3]) {
    u32s(out, &[name, 0, NULL, 0x0e]);
    f32s(out, &t);
    f32s(out, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0]);
    u32s(out, &[NULL]);
}

/// A model: block 0 an `NiNode` moved by `node_t` holding `geoms`, each a `BSGeometry`
/// with a bound (sphere, box) and an optional skin (block types: `NiNode`, `BSGeometry`,
/// `BSSkin::Instance`; a skin points at a trailing `BSSkin::Instance` block).
fn nif(node_t: [f32; 3], geoms: &[([f32; 4], [f32; 6], bool)]) -> Vec<u8> {
    let types = ["NiNode", "BSGeometry", "BSSkin::Instance"];
    let n = geoms.len() as u32;
    let mut blocks: Vec<(u16, Vec<u8>)> = Vec::new();
    let mut node = Vec::new();
    prefix(&mut node, 0, node_t);
    u32s(&mut node, &[n]);
    u32s(&mut node, &(1..=n).collect::<Vec<_>>());
    blocks.push((0, node));
    let mut skins = 0;
    for (sphere, aabb, skinned) in geoms {
        let mut g = Vec::new();
        prefix(&mut g, NULL, [0.0; 3]);
        f32s(&mut g, sphere);
        f32s(&mut g, aabb);
        let skin = if *skinned {
            skins += 1;
            n + skins
        } else {
            NULL
        };
        u32s(&mut g, &[skin, NULL, NULL]);
        g.push(1);
        blocks.push((1, g));
    }
    for _ in 0..skins {
        blocks.push((2, vec![0; 4]));
    }
    let mut out = b"Gamebryo File Format, Version 20.2.0.7\n".to_vec();
    out.extend_from_slice(&0x1402_0007u32.to_le_bytes());
    out.push(1);
    u32s(&mut out, &[12, blocks.len() as u32, 175]);
    out.extend_from_slice(&[1, 0]);
    u32s(&mut out, &[3]);
    out.extend_from_slice(&[1, 0, 2, 7, 0]);
    out.extend_from_slice(&(types.len() as u16).to_le_bytes());
    for t in types {
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

const MAX: f32 = f32::MAX;
/// A 2 x 4 x 6 box centred at (1, 2, 3).
const BOXED: ([f32; 4], [f32; 6], bool) =
    ([1.0, 2.0, 3.0, 3.8], [1.0, 2.0, 3.0, 1.0, 2.0, 3.0], false);
const SKINNED: ([f32; 4], [f32; 6], bool) = ([0.0; 4], [MAX; 6], true);

/// Packs `files` (archive path, bytes) into `data/<name>` as an uncompressed GNRL archive.
fn archive(data: &Path, name: &str, files: &[(&str, Vec<u8>)]) {
    let inputs: Vec<(String, String)> = files
        .iter()
        .map(|(n, _)| (n.to_string(), n.to_string()))
        .collect();
    let mut out = std::io::Cursor::new(Vec::new());
    let options = kitkraft_ba2::PackOptions {
        compress: false,
        ..Default::default()
    };
    kitkraft_ba2::pack(
        &inputs,
        |n| Ok(files.iter().find(|(f, _)| *f == n).unwrap().1.clone()),
        &options,
        &mut out,
    )
    .unwrap();
    std::fs::write(data.join(name), out.into_inner()).unwrap();
}

/// A Data dir: `Starfield - Meshes01.ba2` (crate, skinned, empty), `Starfield -
/// MeshesPatch.ba2` and `SFBGS999 - Main.ba2` (crate again, moved), a loose
/// `test\loose.nif` also in Meshes01, a broken model, and `Mesh.esm` with STAT 0x800 naming the crate.
fn data_dir(name: &str) -> PathBuf {
    let data = dir(name).join("Data");
    std::fs::create_dir_all(data.join("Meshes")).unwrap();
    let crate_nif = nif([0.0; 3], &[BOXED, SKINNED]);
    archive(
        &data,
        "Starfield - Meshes01.ba2",
        &[
            ("meshes\\test\\crate.nif", crate_nif.clone()),
            ("meshes\\test\\actor.nif", nif([0.0; 3], &[SKINNED])),
            ("meshes\\test\\empty.nif", nif([0.0; 3], &[])),
            ("meshes\\test\\loose.nif", crate_nif.clone()),
            ("meshes\\test\\broken.nif", crate_nif[..80].to_vec()),
        ],
    );
    archive(
        &data,
        "Starfield - MeshesPatch.ba2",
        &[("meshes\\test\\crate.nif", nif([0.0, 0.0, 10.0], &[BOXED]))],
    );
    archive(
        &data,
        "SFBGS999 - Main.ba2",
        &[("meshes\\test\\crate.nif", nif([100.0, 0.0, 0.0], &[BOXED]))],
    );
    // Loose wins over the archives: moved +5 y.
    std::fs::create_dir_all(data.join("Meshes").join("test")).unwrap();
    std::fs::write(
        data.join("Meshes").join("test").join("loose.nif"),
        nif([0.0, 5.0, 0.0], &[BOXED]),
    )
    .unwrap();

    let mut spec = HeaderSpec::new(Vec::new());
    spec.flags = FLAG_MASTER;
    let mut plugin = Plugin::create(&spec);
    for (form, edid, modl) in [
        (0x800, "MB_Crate", Some("Test\\Crate.nif")),
        (0x801, "MB_NoModel", None),
    ] {
        let mut subs = vec![sub(b"EDID", &zstr(edid))];
        if let Some(m) = modl {
            subs.push(sub(b"MODL", &zstr(m)));
        }
        let record = rec(b"STAT", 0, form, subs);
        let style = PlacementStyle::OFFICIAL;
        let path = cells::placement(&record.header, &Parent::None, &style).unwrap();
        plugin.upsert_styled(&path, record, &style).unwrap();
    }
    std::fs::write(data.join("Mesh.esm"), plugin.to_bytes().unwrap()).unwrap();
    data
}

fn run(data: &Path, args: Value) -> Result<Value, crate::EngineError> {
    let mut args = args;
    args["data_dir"] = json!(data.display().to_string());
    execute("mesh.bounds", args)
}

fn has(env: &Value, text: &str) -> bool {
    env["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap_or_default().contains(text))
}

#[test]
fn a_plugins_own_archive_wins_and_every_holder_is_named() {
    let data = data_dir("mesh-archive");
    let env = run(&data, json!({"model": "Test\\Crate.nif"})).unwrap();
    let d = &env["data"];
    assert_eq!(d["source"]["kind"], "archive");
    assert_eq!(d["source"]["archive"], "SFBGS999 - Main.ba2");
    assert_eq!(d["also_in"].as_array().unwrap().len(), 3);
    // The plugin's copy: the box moved +100 x by its node.
    assert_eq!(
        d["bounds"],
        json!({"min": [100.0, 0.0, 0.0], "max": [102.0, 4.0, 6.0]})
    );
    assert_eq!(d["frame"], "model root");
    assert_eq!(d["geometries"]["stored"], 1);
    assert_eq!(env["coverage"]["status"], "complete");
    assert!(has(&env, "3 archives hold this model"));
    assert!(has(&env, OBND_NOTE));
    let ev = env["evidence"].as_array().unwrap();
    assert_eq!(ev.len(), 1);
    let entry = d["source"]["file"].as_str().unwrap();
    assert_eq!(kitkraft_ba2::normalize(entry), "meshes/test/crate.nif");
    assert_eq!(ev[0]["file"], format!("SFBGS999 - Main.ba2/{entry}"));
    assert_eq!(ev[0]["length"], 40);
    let spec = find_command("mesh.bounds").unwrap();
    assert!(!spec.writes);
    assert_eq!(spec.status, Status::Implemented);
}

#[test]
fn archive_precedence_ranks_plugin_then_patch_then_base() {
    assert!(rank("SFBGS003 - Main.ba2") > rank("Starfield - MeshesPatch.ba2"));
    assert!(rank("Starfield - MeshesPatch.ba2") > rank("Starfield - Meshes01.ba2"));
    assert_eq!(entry_path("Test\\Crate.nif"), "meshes/test/crate.nif");
    assert_eq!(
        entry_path("Meshes\\Test\\Crate.nif"),
        "meshes/test/crate.nif"
    );
    assert_eq!(entry_path("\\Test/Crate.nif"), "meshes/test/crate.nif");
}

#[test]
fn a_loose_model_wins_over_the_archives() {
    let data = data_dir("mesh-loose");
    let env = run(&data, json!({"model": "test\\loose.nif"})).unwrap();
    assert_eq!(env["data"]["source"]["kind"], "loose");
    assert_eq!(
        env["data"]["bounds"],
        json!({"min": [0.0, 5.0, 0.0], "max": [2.0, 9.0, 6.0]})
    );
    assert!(has(&env, "read the loose file; 1 archive(s) also hold"));
    let ev = env["evidence"].as_array().unwrap();
    assert!(ev[0]["file"].as_str().unwrap().ends_with("loose.nif"));
}

#[test]
fn skinned_geometry_is_left_out_and_said_so() {
    let data = data_dir("mesh-skinned");
    let env = run(&data, json!({"model": "test\\actor.nif"})).unwrap();
    assert_eq!(env["data"]["bounds"], Value::Null);
    assert_eq!(env["data"]["geometries"]["not_stored"], 1);
    assert_eq!(env["coverage"]["status"], "partial");
    assert!(has(
        &env,
        "no bound in the model: all 1 geometry block(s) are skinned"
    ));
    assert_eq!(env["evidence"], json!([]));
}

#[test]
fn a_model_without_geometry_has_no_box_and_says_why() {
    let data = data_dir("mesh-empty");
    let env = run(&data, json!({"model": "Test\\Empty.nif"})).unwrap();
    assert_eq!(env["data"]["bounds"], Value::Null);
    assert_eq!(env["data"]["geometries"]["total"], 0);
    assert!(has(&env, "no bound in the model: it has no geometry block"));
}

#[test]
fn a_base_objects_modl_names_the_model() {
    let data = data_dir("mesh-form");
    let engine = Engine::with_default_handlers();
    let d = data.display().to_string();
    let env = engine
        .execute(
            "mesh.bounds",
            json!({"form": "MB_Crate", "plugins": ["Mesh.esm"], "data_dir": d, "no_cache": true}),
        )
        .unwrap();
    assert_eq!(env["data"]["model"], "Test\\Crate.nif");
    assert_eq!(env["data"]["base_signature"], "STAT");
    assert_eq!(env["data"]["base_editor_id"], "MB_Crate");
    assert!(env["data"]["form"]
        .as_str()
        .unwrap()
        .starts_with("Mesh.esm:"));
    let ev = env["evidence"].as_array().unwrap();
    assert_eq!(ev[0]["file"], "Mesh.esm");
    assert_eq!(ev[0]["subrecord"], "MODL");
    assert_eq!(ev[0]["length"], 15);
    // A base without MODL is refused, named.
    let err = engine
        .execute("mesh.bounds", json!({"form": "MB_NoModel", "data_dir": d}))
        .unwrap_err();
    assert!(err.to_string().contains("has no MODL"), "{err}");
}

#[test]
fn missing_broken_and_ambiguous_requests_are_refused() {
    let data = data_dir("mesh-refused");
    let missing = run(&data, json!({"model": "Test\\Nowhere.nif"})).unwrap_err();
    assert!(missing
        .to_string()
        .contains("in neither the loose files nor the archives"));
    let broken = run(&data, json!({"model": "Test\\Broken.nif"})).unwrap_err();
    assert!(broken.to_string().contains("broken.nif"), "{broken}");
    let both = run(&data, json!({"model": "a.nif", "form": "MB_Crate"})).unwrap_err();
    assert!(both.to_string().contains("not both"));
    let neither = run(&data, json!({})).unwrap_err();
    assert!(neither.to_string().contains("pass `model`"));
}

#[test]
fn mesh_bounds_refuses_arguments_it_does_not_know() {
    let err = execute("mesh.bounds", json!({"out": "x.json"})).unwrap_err();
    assert_eq!(err.kind(), "invalid_args");
}

#[test]
fn mesh_geometry_is_deferred_to_m7_s5_and_refused() {
    let spec = find_command("mesh.geometry").unwrap();
    assert!(!spec.writes);
    assert!(
        matches!(spec.status, Status::Deferred { milestone, .. } if milestone == "M7.S5"),
        "{:?}",
        spec.status
    );
    let err = execute("mesh.geometry", json!({"model": "Meshes\\Test\\Crate.nif"})).unwrap_err();
    assert_eq!(err.kind(), "deferred");
}
