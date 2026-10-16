//! Lane R round 2 on real data (`KITKRAFT_DATA_DIR`; skips when unset): `cell` with
//! `with_bounds: true` (M7.R3) on `Starfield.esm` cells and on the first POI's cell
//! (`KITKRAFT_OUT_DIR`, default `<workspace>/out`: `poi/KitKraftFirstPOI.esm`; skipped
//! when absent). The envelopes are checked against their documented shape.
//!
//! Run in release: `cargo test -p kitkraft-engine --release --test lane_r2_corpus --
//! --ignored --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use kitkraft_engine::shapes::check_envelope;
use kitkraft_engine::Engine;
use serde_json::{json, Value};

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

fn out_dir() -> PathBuf {
    std::env::var_os("KITKRAFT_OUT_DIR").map_or_else(
        || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../out")),
        PathBuf::from,
    )
}

fn corners(b: &Value) -> ([f64; 3], [f64; 3]) {
    let v = |k: &str| {
        let a = b[k].as_array().unwrap();
        [0, 1, 2].map(|i| a[i].as_f64().unwrap())
    };
    (v("min"), v("max"))
}

/// Checks a `with_bounds` envelope: its shape, a bounds box on most references whose
/// base is a static, one `OBND` evidence row per base with bounds.
fn check_bounds(env: &Value) {
    let problems = check_envelope(env);
    assert!(problems.is_empty(), "{problems:?}");
    let items = env["data"]["items"].as_array().unwrap();
    let statics: Vec<&Value> = items
        .iter()
        .filter(|i| i["base_signature"] == "STAT")
        .collect();
    let boxed = statics.iter().filter(|i| i["bounds"].is_object()).count();
    eprintln!(
        "  {}: {} of {} STAT references with bounds; {}",
        env["data"]["summary"],
        boxed,
        statics.len(),
        env["data"]["bounds"]
    );
    assert!(boxed * 10 >= statics.len() * 9, "most statics have an OBND");
    for i in items.iter().filter(|i| i["bounds"].is_object()) {
        let (lo, hi) = corners(&i["bounds"]);
        assert!(lo.iter().chain(&hi).all(|v| v.is_finite()), "{i}");
    }
    let with = env["data"]["bounds"]["bases_with_bounds"].as_u64().unwrap();
    let obnd = env["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["subrecord"] == "OBND")
        .count();
    assert_eq!(obnd as u64, with);
}

#[test]
#[ignore = "release: indexes Starfield.esm and builds its reverse index (cached after the first run)"]
fn starfield_cells_carry_their_bases_obnd() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm"], "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    // The interior where the first POI's crate lives, and an exterior cell by grid.
    let interior = engine
        .execute(
            "cell",
            json!({"cell": "CityNewAtlantisApartmentTEMPLATE", "with_bounds": true, "limit": 1000}),
        )
        .unwrap();
    check_bounds(&interior);
    let exterior = engine
        .execute(
            "cell",
            json!({"world": "NewAtlantis", "x": 2, "y": 1, "with_bounds": true, "limit": 1000}),
        )
        .unwrap();
    check_bounds(&exterior);
    assert_eq!(exterior["data"]["cell_editor_id"], "NAResidentialStart");
    // Without the flag the same page has no bounds and no OBND evidence.
    let plain = engine
        .execute(
            "cell",
            json!({"world": "NewAtlantis", "x": 2, "y": 1, "limit": 1000}),
        )
        .unwrap();
    assert!(plain["data"].get("bounds").is_none());
    assert_eq!(
        plain["evidence"].as_array().unwrap().len(),
        plain["data"]["items"].as_array().unwrap().len()
    );
}

#[test]
#[ignore = "release: indexes Starfield.esm; needs out/poi/KitKraftFirstPOI.esm"]
fn the_first_poi_crate_gets_its_base_obnd() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let poi = out_dir().join("poi").join("KitKraftFirstPOI.esm");
    if !poi.is_file() {
        eprintln!("skip: {} absent", poi.display());
        return;
    }
    let engine = Engine::with_default_handlers();
    let r = engine
        .execute(
            "cell",
            json!({
                "cell": "CityNewAtlantisApartmentTEMPLATE", "with_bounds": true,
                "plugins": ["Starfield.esm", poi.display().to_string()],
                "data_dir": dir.display().to_string(), "limit": 1000,
            }),
        )
        .unwrap();
    check_bounds(&r);
    let crate_ = r["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["editor_id"] == "KitKraftFirstPOICrate")
        .unwrap_or_else(|| panic!("{r:#}"));
    assert_eq!(crate_["base"]["form"], "Starfield.esm:1E4FBD");
    let (lo, hi) = corners(&crate_["bounds"]);
    eprintln!("  crate OBND {lo:?} .. {hi:?}");
    // A crate: wider than deep, taller than nothing, the origin at its floor.
    assert!(lo.iter().zip(&hi).all(|(a, b)| a <= b));
    assert!(hi[0] - lo[0] > 0.5 && hi[2] - lo[2] > 0.5 && lo[2].abs() < 0.01);
}
