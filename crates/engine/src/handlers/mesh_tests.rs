//! `mesh.bounds` (M7.S4) and `mesh.geometry` through `execute` with JSON, on synthetic
//! models (built from the layout in `docs/research/nif-bounds-2026-11-02.md`, no game
//! bytes) placed loose or in archives packed by `kitkraft-ba2`, and a synthetic master
//! whose static names one of them in `MODL`. Neither command writes.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::chunks_exact_to_as_chunks
)]

use std::path::{Path, PathBuf};

use kitkraft_esm::build::HeaderSpec;
use kitkraft_esm::cells::{self, Parent, PlacementStyle};
use kitkraft_esm::tes4::FLAG_MASTER;
use kitkraft_esm::Plugin;
use serde_json::{json, Value};

use super::mesh::{base64, entry_path, geometry, rank, OBND_NOTE};
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
/// [`nif`] with `n` boxed geometry blocks whose first slots claim `indices` and
/// `vertices`.
fn nif_with_counts(node_t: [f32; 3], n: usize, indices: u32, vertices: u32) -> Vec<u8> {
    let mut out = nif(node_t, &vec![BOXED; n]);
    // Every slot of `nif` says (3, 3, 64): rewrite the counts in place.
    let pat = [3u8, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0];
    let mut i = 0;
    while let Some(p) = out[i..].windows(12).position(|w| w == pat) {
        let at = i + p;
        out[at..at + 4].copy_from_slice(&indices.to_le_bytes());
        out[at + 4..at + 8].copy_from_slice(&vertices.to_le_bytes());
        i = at + 12;
    }
    out
}

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
    for (k, (sphere, aabb, skinned)) in geoms.iter().enumerate() {
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
        // The four slots (mesh study G1): the first names `geometries/test/mesh<k>.mesh`,
        // a triangle (3 indices, 3 vertices); the other three are absent.
        let path = format!("test/mesh{k}");
        g.push(1);
        u32s(&mut g, &[3, 3, 64, path.len() as u32]);
        g.extend_from_slice(path.as_bytes());
        g.extend_from_slice(&[0, 0, 0]);
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
    assert!(has(&env, "ties by archive name"));
    assert!(has(&env, "whether or not its plugin is in a load order"));
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
    assert_eq!(
        entry_path("Test\\Crate.nif").unwrap(),
        "meshes/test/crate.nif"
    );
    assert_eq!(
        entry_path("Meshes\\Test\\Crate.nif").unwrap(),
        "meshes/test/crate.nif"
    );
    assert_eq!(
        entry_path("Test/Crate.nif").unwrap(),
        "meshes/test/crate.nif"
    );
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
    assert!(has(&env, "loose over archives is our rule"));
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

#[test]
fn a_model_path_leaving_meshes_is_refused_before_any_file_is_opened() {
    // Review M1: `..` let the reviewer read Data\Starfield.esm and C:\Windows\win.ini.
    let data = data_dir("mesh-escape");
    // A real file one level up: refused without being opened (no size, no parse error).
    std::fs::write(data.join("outside.nif"), nif([0.0; 3], &[BOXED])).unwrap();
    for bad in [
        r"..\outside.nif",
        r"Test\..\..\outside.nif",
        r"..\..\..\..\Windows\win.ini",
        r"C:\x.nif",
        r"C:x.nif",
        r"\\?\C:\x.nif",
        r"\Test\Crate.nif",
        "/Test/Crate.nif",
        r"Test\.\Crate.nif",
        r"Test\\Crate.nif",
        "",
    ] {
        let err = run(&data, json!({"model": bad})).unwrap_err();
        assert!(
            err.to_string().contains("not a path inside Meshes"),
            "{bad}: {err}"
        );
    }
}

// ---- mesh.geometry (M7.S5 code, ask A8): called directly, the catalog still says Deferred

/// A triangle mesh as the M7.S5 study lays it out (M1 to M6): corners (0,0,0), (s,0,0),
/// (0,s,0) at scale `s`, every normal +z.
fn triangle_mesh(scale: f32) -> Vec<u8> {
    let mut out = Vec::new();
    u32s(&mut out, &[2, 3]);
    for i in [0u16, 1, 2] {
        out.extend_from_slice(&i.to_le_bytes());
    }
    out.extend_from_slice(&scale.to_le_bytes());
    u32s(&mut out, &[0, 3]);
    for p in [[0i16, 0, 0], [32767, 0, 0], [0, 32767, 0]] {
        for v in p {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    let up: u32 = 512 | (512 << 10) | (1023 << 20);
    for (k, n) in [3u32, 0, 0, 3, 3].into_iter().enumerate() {
        u32s(&mut out, &[n]);
        for _ in 0..n {
            u32s(&mut out, &[if k == 3 { up } else { 0 }]);
        }
    }
    u32s(&mut out, &[0, 0]);
    out
}

fn f32s_of(b64: &str) -> Vec<f32> {
    decode64(b64)
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn u32s_of(b64: &str) -> Vec<u32> {
    decode64(b64)
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// A test-side base64 decoder (standard alphabet, `=` padding).
fn decode64(s: &str) -> Vec<u8> {
    let val = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => 0,
        }
    };
    let mut out = Vec::new();
    for q in s.as_bytes().chunks(4) {
        let pad = q.iter().filter(|c| **c == b'=').count();
        let n = q.iter().fold(0u32, |a, c| (a << 6) | val(*c));
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..3 - pad]);
    }
    out
}

/// A Data dir holding `Starfield - Meshes01.ba2` with a model of `geoms` triangles
/// (the node moved by `node_t`) and the meshes in `meshes`.
fn geometry_dir(name: &str, node_t: [f32; 3], geoms: usize, meshes: &[(&str, Vec<u8>)]) -> PathBuf {
    geometry_dir_with(name, node_t, &vec![BOXED; geoms], meshes)
}

/// [`geometry_dir`] with the geometry blocks' bounds given (boxed, skinned, ...).
fn geometry_dir_with(
    name: &str,
    node_t: [f32; 3],
    geoms: &[([f32; 4], [f32; 6], bool)],
    meshes: &[(&str, Vec<u8>)],
) -> PathBuf {
    let data = dir(name).join("Data");
    std::fs::create_dir_all(&data).unwrap();
    let model = nif(node_t, geoms);
    let mut files: Vec<(&str, Vec<u8>)> = vec![("meshes\\test\\tri.nif", model)];
    files.extend(meshes.iter().map(|(n, b)| (*n, b.clone())));
    archive(&data, "Starfield - Meshes01.ba2", &files);
    data
}

fn run_geometry(data: &Path, model: &str) -> Result<Value, crate::EngineError> {
    let mut session = crate::Session::default();
    let args = json!({"model": model, "data_dir": data.display().to_string()});
    geometry(&mut session, &args).map(|o| o.data)
}

#[test]
fn base64_matches_the_rfc_4648_examples() {
    let cases = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];
    for (raw, enc) in cases {
        assert_eq!(base64(raw.as_bytes()), enc);
        assert_eq!(decode64(enc), raw.as_bytes());
    }
}

#[test]
fn geometry_places_each_mesh_in_the_model_root_and_joins_the_indices() {
    let data = geometry_dir(
        "mesh-geometry",
        [10.0, 0.0, 0.0],
        2,
        &[
            ("geometries\\test\\mesh0.mesh", triangle_mesh(2.0)),
            ("geometries\\test\\mesh1.mesh", triangle_mesh(1.0)),
        ],
    );
    let d = run_geometry(&data, "Test\\Tri.nif").unwrap();
    assert_eq!(d["vertex_count"], 6);
    assert_eq!(d["index_count"], 6);
    assert_eq!(d["frame"], "model root");
    let p = f32s_of(d["positions"].as_str().unwrap());
    // The node's +10 x applies to both; the first mesh is scaled 2, the second 1.
    assert_eq!(&p[..9], &[10.0, 0.0, 0.0, 12.0, 0.0, 0.0, 10.0, 2.0, 0.0]);
    assert_eq!(&p[9..], &[10.0, 0.0, 0.0, 11.0, 0.0, 0.0, 10.0, 1.0, 0.0]);
    assert_eq!(u32s_of(d["indices"].as_str().unwrap()), [0, 1, 2, 3, 4, 5]);
    let n = f32s_of(d["normals"].as_str().unwrap());
    assert_eq!(n.len(), 18);
    for v in n.chunks_exact(3) {
        assert!(
            v[0].abs() < 0.01 && v[1].abs() < 0.01 && (v[2] - 1.0).abs() < 1e-6,
            "{v:?}"
        );
    }
    assert_eq!(d["raw_bytes"], 6 * 12 * 2 + 6 * 4);
    assert_eq!(d["meshes"].as_array().unwrap().len(), 2);
}

#[test]
fn geometry_skips_a_missing_or_broken_mesh_and_says_so() {
    let mut broken = triangle_mesh(1.0);
    broken.push(0);
    let data = geometry_dir(
        "mesh-geometry-skip",
        [0.0; 3],
        3,
        &[
            ("geometries\\test\\mesh0.mesh", triangle_mesh(1.0)),
            ("geometries\\test\\mesh1.mesh", broken),
        ],
    );
    let mut session = crate::Session::default();
    let args = json!({"model": "Test\\Tri.nif", "data_dir": data.display().to_string()});
    let out = geometry(&mut session, &args).unwrap();
    assert_eq!(out.data["vertex_count"], 3);
    let l = out.limitations.join("\n");
    assert!(
        l.contains("mesh1.mesh") && l.contains("M6 entry count"),
        "{l}"
    );
    assert!(l.contains("mesh2.mesh is in no archive"), "{l}");
    assert!(l.contains("2 geometry block(s) skipped"), "{l}");
    assert!(l.contains("normals are grade B"), "{l}");
}

#[test]
fn geometry_refuses_a_model_path_leaving_meshes_like_bounds() {
    let data = geometry_dir("mesh-geometry-escape", [0.0; 3], 1, &[]);
    let err = run_geometry(&data, "..\\Starfield.esm").unwrap_err();
    assert!(
        err.to_string().contains("not a path inside Meshes"),
        "{err}"
    );
    assert!(err.to_string().contains("mesh.geometry"), "{err}");
}

/// V4 (M7.S5; `docs/design/workbench-performance.md`), engine side: `mesh.geometry` for
/// every distinct model of a Starfield.esm cell's bases (the models `mesh.bounds` finds
/// from their MODL), in one session: time to the first mesh, time to all of them, and the
/// envelope size. Release only.
#[test]
#[ignore = "release: indexes Starfield.esm and reads every model of a cell"]
fn v4_mesh_geometry_for_the_models_of_a_cell() {
    let Some(data) = std::env::var_os("KITKRAFT_DATA_DIR").map(PathBuf::from) else {
        eprintln!("skipped: KITKRAFT_DATA_DIR is not set");
        return;
    };
    let d = data.display().to_string();
    let engine = Engine::with_default_handlers();
    engine
        .execute("load", json!({"plugins": ["Starfield.esm"], "data_dir": d}))
        .unwrap();
    let cell = engine
        .execute(
            "cell",
            json!({"world": "NewAtlantis", "x": 2, "y": 1, "limit": 1000}),
        )
        .unwrap();
    let mut models: Vec<String> = Vec::new();
    for item in cell["data"]["items"].as_array().unwrap() {
        let Some(base) = item["base"]["form"].as_str() else {
            continue;
        };
        if let Ok(env) = engine.execute("mesh.bounds", json!({"form": base, "data_dir": d})) {
            if let Some(m) = env["data"]["model"].as_str() {
                if !models.iter().any(|x| x == m) {
                    models.push(m.to_string());
                }
            }
        }
    }
    let mut session = crate::Session::default();
    let start = std::time::Instant::now();
    let mut first = None;
    let (mut vertices, mut triangles, mut raw, mut json_bytes, mut skipped, mut failed) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for m in &models {
        match geometry(&mut session, &json!({"model": m, "data_dir": d})) {
            Ok(out) => {
                first.get_or_insert(start.elapsed());
                vertices += out.data["vertex_count"].as_u64().unwrap_or(0);
                triangles += out.data["index_count"].as_u64().unwrap_or(0) / 3;
                raw += out.data["raw_bytes"].as_u64().unwrap_or(0);
                json_bytes += out.data.to_string().len() as u64;
                skipped += out
                    .limitations
                    .iter()
                    .filter(|l| l.ends_with(": skipped") || l.contains(" skipped: "))
                    .count() as u64;
            }
            Err(_) => failed += 1,
        }
    }
    let all = start.elapsed();
    eprintln!(
        "V4 NewAtlantis 2,1: {} models; first mesh {:?} (includes the archive and mesh index); all {all:?}; {vertices} vertices, {triangles} triangles; raw {raw} bytes, data JSON {json_bytes} bytes ({:.2}x raw); meshes skipped {skipped}; models refused {failed}",
        models.len(),
        first.unwrap_or_default(),
        json_bytes as f64 / raw.max(1) as f64
    );
    assert!(!models.is_empty());
}

#[test]
fn geometry_skips_skinned_blocks_whose_frame_the_study_does_not_show() {
    // Review M1 (M7.S5 code): a skinned block stores no box (NIF study B5), so the
    // mesh study's box check (M2) says nothing about the frame of its positions; it
    // is skipped with a limitation, never placed in "model root".
    let data = geometry_dir_with(
        "mesh-geometry-skinned",
        [5.0, 0.0, 0.0],
        &[BOXED, SKINNED],
        &[
            ("geometries\\test\\mesh0.mesh", triangle_mesh(1.0)),
            ("geometries\\test\\mesh1.mesh", triangle_mesh(1.0)),
        ],
    );
    let mut session = crate::Session::default();
    let args = json!({"model": "Test\\Tri.nif", "data_dir": data.display().to_string()});
    let out = geometry(&mut session, &args).unwrap();
    assert_eq!(out.data["vertex_count"], 3);
    assert_eq!(out.data["meshes"].as_array().unwrap().len(), 1);
    let l = out.limitations.join("\n");
    assert!(
        l.contains("block 2 is skinned or stores no recognised bound"),
        "{l}"
    );
    assert!(matches!(
        out.coverage.status,
        crate::envelope::CoverageStatus::Partial
    ));
}

/// A 3-vertex mesh whose index list repeats the triangle `n` times.
fn repeated_triangle_mesh(n: usize) -> Vec<u8> {
    let base = triangle_mesh(1.0);
    let mut out = Vec::new();
    u32s(&mut out, &[2, (3 * n) as u32]);
    for _ in 0..n {
        for i in [0u16, 1, 2] {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
    // The rest of the triangle mesh after its 3 indices (8 + 6 bytes).
    out.extend_from_slice(&base[14..]);
    out
}

#[test]
fn geometry_budgets_indices_as_well_as_vertices() {
    // Review M2 (M7.S5 code): 20 blocks naming a 3-vertex mesh of 300,000 indices made
    // a 32 MB reply with coverage Complete. With the index budget, the 21st block of
    // such a model is skipped and named, and coverage is Partial.
    let names: Vec<String> = (0..21)
        .map(|k| format!("geometries\\test\\mesh{k}.mesh"))
        .collect();
    let big = repeated_triangle_mesh(100_000);
    let meshes: Vec<(&str, Vec<u8>)> = names.iter().map(|n| (n.as_str(), big.clone())).collect();
    let data = dir("mesh-geometry-budget").join("Data");
    std::fs::create_dir_all(&data).unwrap();
    // Each slot says 300,000 indices and 3 vertices, as the mesh holds.
    let model = nif_with_counts([0.0; 3], 21, 300_000, 3);
    let mut files: Vec<(&str, Vec<u8>)> = vec![("meshes\\test\\tri.nif", model)];
    files.extend(meshes.iter().map(|(n, b)| (*n, b.clone())));
    archive(&data, "Starfield - Meshes01.ba2", &files);
    let mut session = crate::Session::default();
    let args = json!({"model": "Test\\Tri.nif", "data_dir": data.display().to_string()});
    let out = geometry(&mut session, &args).unwrap();
    assert_eq!(out.data["index_count"], 6_000_000);
    let l = out.limitations.join("\n");
    assert!(l.contains("block 21") && l.contains("index budget"), "{l}");
    assert!(matches!(
        out.coverage.status,
        crate::envelope::CoverageStatus::Partial
    ));
}

#[test]
fn geometry_skips_a_mesh_past_the_vertex_budget_and_one_whose_counts_disagree() {
    // Review L6: the vertex budget and the count check, each with its own line.
    let data = dir("mesh-geometry-counts").join("Data");
    std::fs::create_dir_all(&data).unwrap();
    // Block 1 claims 2,000,001 vertices; block 2 claims 4 vertices of a 3-vertex mesh.
    let mut model = nif_with_counts([0.0; 3], 2, 3, 2_000_001);
    let at = model
        .windows(12)
        .rposition(|w| w == [3, 0, 0, 0, 0x81, 0x84, 0x1e, 0, 64, 0, 0, 0])
        .unwrap();
    model[at + 4..at + 8].copy_from_slice(&4u32.to_le_bytes());
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("meshes\\test\\tri.nif", model),
        ("geometries\\test\\mesh0.mesh", triangle_mesh(1.0)),
        ("geometries\\test\\mesh1.mesh", triangle_mesh(1.0)),
    ];
    archive(&data, "Starfield - Meshes01.ba2", &files);
    let mut session = crate::Session::default();
    let args = json!({"model": "Test\\Tri.nif", "data_dir": data.display().to_string()});
    let out = geometry(&mut session, &args).unwrap();
    let l = out.limitations.join("\n");
    assert!(l.contains("vertex budget"), "{l}");
    assert!(l.contains("are not the mesh's"), "{l}");
    assert_eq!(out.data["vertex_count"], 0);
}

#[test]
fn geometry_names_unreached_blocks_and_stays_partial() {
    // Review L1 (M7.S5 code): block 2 is not reached from the root; it was dropped
    // silently with coverage Complete.
    let data = dir("mesh-geometry-unreached").join("Data");
    std::fs::create_dir_all(&data).unwrap();
    let mut model = nif([0.0; 3], &[BOXED, BOXED]);
    let at = model
        .windows(12)
        .position(|w| w == [2, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0])
        .unwrap();
    model[at + 8..at + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("meshes\\test\\tri.nif", model),
        ("geometries\\test\\mesh0.mesh", triangle_mesh(1.0)),
        ("geometries\\test\\mesh1.mesh", triangle_mesh(1.0)),
    ];
    archive(&data, "Starfield - Meshes01.ba2", &files);
    let mut session = crate::Session::default();
    let args = json!({"model": "Test\\Tri.nif", "data_dir": data.display().to_string()});
    let out = geometry(&mut session, &args).unwrap();
    assert_eq!(out.data["vertex_count"], 3);
    let l = out.limitations.join("\n");
    assert!(
        l.contains("1 geometry block(s) are not reached from the root"),
        "{l}"
    );
    assert!(matches!(
        out.coverage.status,
        crate::envelope::CoverageStatus::Partial
    ));
}

#[test]
fn geometry_skips_a_block_whose_slots_do_not_read_and_keeps_the_rest() {
    // Review L2 (M7.S5 code): one bad slot failed the whole reply.
    let data = dir("mesh-geometry-bad-slot").join("Data");
    std::fs::create_dir_all(&data).unwrap();
    let mut model = nif([0.0; 3], &[BOXED, BOXED]);
    let at = model
        .windows(12)
        .rposition(|w| w == [3, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0])
        .unwrap();
    model[at + 8] = 65;
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("meshes\\test\\tri.nif", model),
        ("geometries\\test\\mesh0.mesh", triangle_mesh(1.0)),
        ("geometries\\test\\mesh1.mesh", triangle_mesh(1.0)),
    ];
    archive(&data, "Starfield - Meshes01.ba2", &files);
    let mut session = crate::Session::default();
    let args = json!({"model": "Test\\Tri.nif", "data_dir": data.display().to_string()});
    let out = geometry(&mut session, &args).unwrap();
    assert_eq!(out.data["vertex_count"], 3);
    let l = out.limitations.join("\n");
    assert!(l.contains("block 2: its slots do not read"), "{l}");
    assert!(matches!(
        out.coverage.status,
        crate::envelope::CoverageStatus::Partial
    ));
}
