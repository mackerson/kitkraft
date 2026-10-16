//! Lane D round 3 on real data (`KITKRAFT_DATA_DIR`; skips when unset), through one
//! `Engine` session:
//!
//! - M6.P3b: the reverse-link index stays with the loaded load order, so a second
//!   `refs --incoming` in the same session does not touch it again (and across processes
//!   it is read from the disk cache, `docs/design/workbench-performance.md` I4).
//! - M7.S3: `cell` on `Starfield.esm`: the worldspaces, the exterior cells of the first
//!   world with cells, and the references of one of them by its grid; and the first POI's
//!   cell with the crate it places (`KITKRAFT_OUT_DIR`, default `<workspace>/out`:
//!   `poi/KitKraftFirstPOI.esm`; skipped when absent).
//! - M7.S7: `revert` on copies of the built plugins in `out/` (the overhaul and the
//!   rename plugin): a dry run changes nothing, a removal then leaves the file without the
//!   record, `to: previous` restores the master's data.
//!
//! Run in release: `cargo test -p kitkraft-engine --release --test lane_d3_corpus --
//! --ignored --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use kitkraft_engine::Engine;
use serde_json::{json, Value};

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

/// The built plugins' folder: `KITKRAFT_OUT_DIR`, else `<workspace>/out`.
fn out_dir() -> PathBuf {
    std::env::var_os("KITKRAFT_OUT_DIR").map_or_else(
        || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../out")),
        PathBuf::from,
    )
}

fn ms(r: &Value) -> u64 {
    r["data"]["elapsed_ms"].as_u64().unwrap_or(u64::MAX)
}

#[test]
#[ignore = "release: indexes Starfield.esm and builds its reverse index (cached after the first run)"]
fn session_keeps_the_reverse_index_warm_and_cells_list_references() {
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
    let rifle = json!({"form": "Starfield.esm:02EB36", "incoming": true});
    let first = engine.execute("refs", rifle.clone()).unwrap();
    let second = engine.execute("refs", rifle).unwrap();
    eprintln!(
        "refs --incoming Tombstone: first {} ms (index {}), second {} ms (index {})",
        ms(&first),
        first["data"]["index"],
        ms(&second),
        second["data"]["index"]
    );
    assert_eq!(first["data"]["total"], second["data"]["total"]);
    assert!(ms(&second) < 1000, "second call {} ms", ms(&second));

    // cell: worlds, the first world with cells, a cell of it by grid.
    let worlds = engine.execute("cell", json!({"limit": 1000})).unwrap();
    eprintln!(
        "cell worlds: {} ms, {}",
        ms(&worlds),
        worlds["data"]["summary"]
    );
    let world = worlds["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["cells"].as_u64().unwrap_or(0) > 0)
        .expect("a world with cells");
    let cells = engine
        .execute("cell", json!({"world": world["form"], "limit": 50}))
        .unwrap();
    eprintln!(
        "cell world: {} ms, {}",
        ms(&cells),
        cells["data"]["summary"]
    );
    let cell = cells["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["x"].is_i64())
        .expect("an exterior cell with a grid");
    let refs = engine
        .execute(
            "cell",
            json!({"world": world["form"], "x": cell["x"], "y": cell["y"], "limit": 1000}),
        )
        .unwrap();
    eprintln!("cell refs: {} ms, {}", ms(&refs), refs["data"]["summary"]);
    assert_eq!(refs["data"]["cell"], cell["form"]);
    let items = refs["data"]["items"].as_array().unwrap();
    assert!(!items.is_empty());
    let placed = items.iter().filter(|i| i["position"].is_array()).count();
    assert!(placed > 0, "{refs:#}");
    assert!(items
        .iter()
        .all(|i| i["base"].is_object() || i["base"].is_null()));
    assert_eq!(refs["evidence"].as_array().unwrap().len(), items.len());
    let interior = engine
        .execute("cell", json!({"interior": true, "limit": 5}))
        .unwrap();
    assert!(interior["data"]["total"].as_u64().unwrap() > 100);
}

#[test]
#[ignore = "release: indexes Starfield.esm; needs out/poi/KitKraftFirstPOI.esm"]
fn first_poi_cell_lists_its_crate() {
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
                "cell": "CityNewAtlantisApartmentTEMPLATE",
                "plugins": ["Starfield.esm", poi.display().to_string()],
                "data_dir": dir.display().to_string(), "limit": 1000,
            }),
        )
        .unwrap();
    eprintln!("cell POI: {} ms, {}", ms(&r), r["data"]["summary"]);
    let crate_ = r["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["editor_id"] == "KitKraftFirstPOICrate")
        .unwrap_or_else(|| panic!("{r:#}"));
    assert_eq!(crate_["plugin"], "KitKraftFirstPOI.esm");
    assert_eq!(crate_["base"]["form"], "Starfield.esm:1E4FBD");
    assert_eq!(crate_["persistent"], true);
    assert!(crate_["position"].is_array());
}

#[test]
#[ignore = "release: indexes Starfield.esm; needs the built plugins in out/"]
fn revert_on_copies_of_the_built_plugins() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let out = out_dir();
    let tmp = std::env::temp_dir().join(format!("kitkraft-d3-revert-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm"], "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    let cases = [
        ("OldSovietMarksmanRifle.esm", "Starfield.esm:21BBCD"),
        ("overhaul/KitKraftGunOverhaul.esm", "Starfield.esm:24561C"),
    ];
    let mut ran = 0;
    for (rel, form) in cases {
        let src = out.join(rel);
        if !src.is_file() {
            eprintln!("skip: {} absent", src.display());
            continue;
        }
        let copy = tmp.join(src.file_name().unwrap());
        std::fs::copy(&src, &copy).unwrap();
        let bytes = std::fs::read(&copy).unwrap();
        let args = |extra: Value| {
            let mut a = json!({"plugin": copy.display().to_string(), "form": form});
            for (k, v) in extra.as_object().unwrap() {
                a[k] = v.clone();
            }
            a
        };
        let dry = engine
            .execute("revert", args(json!({"dry_run": true})))
            .unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), bytes, "dry run wrote");
        assert_eq!(dry["data"]["diff"]["counts"]["removed"], 1, "{dry:#}");
        let prev = engine
            .execute("revert", args(json!({"to": "previous"})))
            .unwrap();
        assert_eq!(prev["data"]["written"], true);
        assert_eq!(prev["data"]["diff"]["counts"]["changed"], 1, "{prev:#}");
        // The record now holds the master's data (lstrings inlined: a separate session,
        // so the copy does not join this one's load order).
        let conflicts = Engine::with_default_handlers()
            .execute(
                "inspect",
                json!({"form": form, "plugins": ["Starfield.esm", copy.display().to_string()], "data_dir": dir.display().to_string()}),
            )
            .unwrap();
        let versions = conflicts["data"]["versions"].as_array().unwrap();
        let class = &versions[1]["vs_previous"]["class"];
        eprintln!("{rel}: after `to: previous`, vs Starfield.esm: {class}");
        let removed = engine.execute("revert", args(json!({}))).unwrap();
        assert_eq!(removed["data"]["written"], true);
        assert_eq!(removed["data"]["diff"]["counts"]["removed"], 1);
        eprintln!("{rel}: {}", removed["data"]["summary"]);
        ran += 1;
    }
    let _ = std::fs::remove_dir_all(&tmp);
    eprintln!("revert ran on {ran} built plugin(s)");
}
