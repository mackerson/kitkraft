//! M4.D1 on the official files (`KITKRAFT_DATA_DIR`; skips when unset), through `execute`:
//!
//! - `refs` of the Old Earth Hunting Rifle (`Starfield.esm:21BBCD`) follows its typed links
//!   two hops: its keywords, its ammunition (AMMO) and, from the ammunition, a projectile
//!   (PROJ), every value resolved or null, raw values kept.
//! - `refs --incoming` of a vanilla STAT on the 14 official files lists its placed
//!   references with cell and world (the reverse index, built or read from the cache).
//!
//! Both read `Starfield.esm` (1.46 GB): run in release.
//! `cargo test -p kitkraft-engine --release --test refs_corpus -- --ignored`

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

/// The 14 official files, in the order `docs/corpus.md` lists them.
const OFFICIAL: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "SFBGS003.esm",
    "SFBGS004.esm",
    "SFBGS006.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "ShatteredSpace.esm",
    "SFBGS00D.esm",
    "SFBGS047.esm",
    "SFBGS050.esm",
    "BlueprintShips-SFBGS050.esm",
];

fn edges(r: &Value) -> &Vec<Value> {
    r["data"]["edges"].as_array().unwrap()
}

#[test]
#[ignore = "release: indexes Starfield.esm (cached after the first run)"]
fn corpus_rifle_links_reach_ammo_and_projectile() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let engine = Engine::with_default_handlers();
    let r = engine
        .execute(
            "refs",
            json!({"form": "Starfield.esm:21BBCD", "depth": 2, "limit": 1000,
                   "plugins": ["Starfield.esm"], "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    let data = &r["data"];
    assert_eq!(data["editor_id"], "RussianHuntingRifle");
    assert_eq!(data["next_cursor"], Value::Null, "one page holds the graph");
    let list = edges(&r);
    let first: Vec<&Value> = list.iter().filter(|e| e["depth"] == 1).collect();
    // Hop 1: 111 typed FormID values (76 resolved, 35 null), 16 of them keywords.
    assert_eq!(first.len(), 111);
    assert_eq!(first.iter().filter(|e| e["status"] == "null").count(), 35);
    assert!(first
        .iter()
        .all(|e| e["status"] == "resolved" || e["status"] == "null"));
    let keywords: Vec<&&Value> = first
        .iter()
        .filter(|e| e["path"].as_str().unwrap().starts_with("KWDA.Forms["))
        .collect();
    assert_eq!(keywords.len(), 16);
    assert!(keywords.iter().all(|e| e["to_signature"] == "KYWD"));
    let ammo = first
        .iter()
        .find(|e| e["path"] == "WAM2.Ammunition.AmmoType")
        .unwrap();
    assert_eq!(ammo["to"], "Starfield.esm:2B559B");
    assert_eq!(ammo["to_signature"], "AMMO");
    assert_eq!(ammo["to_editor_id"], "Ammo9x39mm");
    assert_eq!(ammo["raw"], "002B559B");
    assert_eq!(ammo["targets"], json!(["AMMO"]));
    assert_eq!(ammo["target_allowed"], true);
    // Hop 2: the ammunition names a projectile.
    let projectile: Vec<&Value> = list
        .iter()
        .filter(|e| {
            e["depth"] == 2 && e["from"] == "Starfield.esm:2B559B" && e["to_signature"] == "PROJ"
        })
        .collect();
    assert!(!projectile.is_empty(), "{}", data["summary"]);
    // Evidence: one range per edge, four bytes each for uncompressed records.
    assert_eq!(r["evidence"].as_array().unwrap().len(), list.len());
    let summary = data["summary"].as_str().unwrap();
    assert!(summary.contains("WAM2.Ammunition.AmmoType -> Starfield.esm:2B559B AMMO Ammo9x39mm"));
}

#[test]
#[ignore = "release: builds the reverse index of the 14 official files (cached after the first run)"]
fn corpus_stat_lists_placed_references_with_cell_and_world() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": OFFICIAL, "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    let started = std::time::Instant::now();
    let r = engine
        .execute(
            "refs",
            json!({"form": "NA_Hard_RetWallASupport_03", "incoming": true}),
        )
        .unwrap();
    eprintln!(
        "refs --incoming: {} ms (index {} ms: {})",
        started.elapsed().as_millis(),
        r["data"]["index"]["elapsed_ms"],
        r["data"]["index"]
    );
    let data = &r["data"];
    assert_eq!(data["global"], "Starfield.esm:002AD0");
    assert_eq!(data["counts"]["placed"], 2, "{}", data["summary"]);
    for e in edges(&r) {
        assert_eq!(e["group"], "placed");
        assert_eq!(e["from_signature"], "REFR");
        assert_eq!(e["path"], "NAME.Base");
        assert_eq!(e["placed"]["world"]["form"], "Starfield.esm:01251B");
        assert!(e["placed"]["cell"]["form"].as_str().is_some());
    }
    // A keyword that weapons name: referrers other than placed ones come first.
    let r = engine
        .execute(
            "refs",
            json!({"form": "WeaponTypeBallistic", "incoming": true, "limit": 5, "types": ["WEAP"]}),
        )
        .unwrap();
    assert!(
        r["data"]["total"].as_u64().unwrap() > 50,
        "{}",
        r["data"]["summary"]
    );
    assert!(edges(&r)
        .iter()
        .all(|e| e["from_signature"] == "WEAP" && e["group"] == "referrer"));
    assert!(r["data"]["next_cursor"].is_string());
}
