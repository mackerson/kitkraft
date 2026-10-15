//! M4.D3 on the official files (`KITKRAFT_DATA_DIR`; skips when unset), through `execute`:
//! every Planet Content Manager tree of `Starfield.esm` is walked from its upward links,
//! with its content node count (`docs/schema/planet-family.md`) and every link that does
//! not resolve listed; one planet reaches its star, biomes and the content nodes that
//! place its worldspaces. `corpus_planet_link_statistics` prints the link table of
//! `docs/design/planet-content.md`.
//!
//! `cargo test -p kitkraft-engine --release --test planet_corpus -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitkraft_doc::links::version_links;
use kitkraft_doc::search::{candidates, Scope};
use kitkraft_doc::{LoadOptions, LoadOrder, PluginSpec, RecordReader};
use kitkraft_engine::Engine;
use serde_json::json;

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

/// The five trees and their content nodes in `Starfield.esm`
/// (`docs/schema/planet-family.md`, "Trees").
const TREES: [(&str, u64); 5] = [
    ("PCM_BlockCreationRequest", 892),
    ("PCM_CellLoadRequest", 525),
    ("PCM_QuestRequest", 249),
    ("PCM_ScanPlanetRequest", 132),
    ("PCM_SpaceshipLandingRequest", 7),
];

#[test]
#[ignore = "release: indexes Starfield.esm (cached after the first run)"]
fn corpus_every_pcmt_tree_is_walked() {
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
    let mut unresolved_total = 0usize;
    for (tree, contents) in TREES {
        let r = engine
            .execute("planet", json!({"form": tree, "limit": 1}))
            .unwrap();
        let d = &r["data"];
        assert_eq!(d["kind"], "tree");
        eprintln!(
            "{tree}: {} ({} ms); default objects {}; unresolved {}; outside targets {}",
            d["counts"],
            d["elapsed_ms"],
            d["default_objects"].as_array().unwrap().len(),
            d["unresolved"],
            d["target_mismatches"].as_array().unwrap().len()
        );
        assert_eq!(d["counts"]["content_nodes"], contents, "{tree}");
        assert_eq!(d["default_objects"].as_array().unwrap().len(), 1, "{tree}");
        unresolved_total += d["unresolved"].as_array().unwrap().len();
    }
    // Every tree link resolves in Starfield.esm.
    assert_eq!(unresolved_total, 0);
}

#[test]
#[ignore = "release: indexes Starfield.esm (cached after the first run)"]
fn corpus_a_planet_reaches_its_star_biomes_and_placements() {
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
    // A content node of the quest tree that places a worldspace some planet lists.
    let r = engine
        .execute("planet", json!({"form": "PCM_QuestRequest", "limit": 5000}))
        .unwrap();
    let (node, planet) = r["data"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|n| {
            let p = n["content_detail"]["planets"].as_array()?.first()?;
            Some((
                n["form"].as_str()?.to_string(),
                p["form"].as_str()?.to_string(),
            ))
        })
        .expect("a worldspace content node on a planet");
    let r = engine.execute("planet", json!({"form": planet})).unwrap();
    let d = &r["data"];
    eprintln!("{}", d["summary"]);
    assert_eq!(d["kind"], "planet");
    assert!(d["planet"]["star"]["form"].is_string(), "{}", d["summary"]);
    assert!(!d["planet"]["worldspaces"].as_array().unwrap().is_empty());
    let placed = d["placements"].as_array().unwrap();
    let mine = placed
        .iter()
        .find(|p| p["node"] == node.as_str())
        .unwrap_or_else(|| panic!("{node} places {planet}: {}", d["summary"]));
    let chain = mine["chain"].as_array().unwrap();
    assert_eq!(chain.last().unwrap()["signature"], "PCMT");
    assert_eq!(mine["tree"], chain.last().unwrap()["form"]);
    // The star lists the planet back.
    let star = d["planet"]["star"]["form"].as_str().unwrap();
    let r = engine.execute("planet", json!({"form": star})).unwrap();
    assert!(r["data"]["planets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["planet"]["form"] == planet.as_str()));
}

/// Prints, per record type and member of the planet family, the record types its links
/// name and their statuses (the table in `docs/design/planet-content.md`).
#[test]
#[ignore = "release: reads every planet-family record of the 14 official files"]
fn corpus_planet_link_statistics() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let official = [
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
    let specs = official
        .iter()
        .map(|n| PluginSpec {
            name: (*n).to_string(),
            path: dir.join(n),
        })
        .collect();
    let lo = LoadOrder::load(
        specs,
        &LoadOptions {
            cache: kitkraft_doc::IndexCache::default_location(),
            threads: 0,
        },
    )
    .unwrap();
    let schema = kitkraft_schema::builtin().unwrap();
    let mut reader = RecordReader::new();
    // (type, member) -> (target or status) -> count
    let mut table: BTreeMap<(String, String), BTreeMap<String, u64>> = BTreeMap::new();
    let mut records: BTreeMap<String, u64> = BTreeMap::new();
    for sig in [
        *b"PCMT", *b"PCBN", *b"PCCN", *b"LVLP", *b"PKIN", *b"BIOM", *b"PNDT", *b"STDT",
    ] {
        let name = String::from_utf8_lossy(&sig).into_owned();
        for v in candidates(&lo, sig, Scope::Winner) {
            *records.entry(name.clone()).or_default() += 1;
            let (_, links) = version_links(&lo, &mut reader, schema, v).unwrap();
            for l in links.links {
                let member = l.chain.join(".");
                let what = match (l.status.as_str(), l.target_signature) {
                    ("resolved", Some(s)) => String::from_utf8_lossy(&s).into_owned(),
                    (s, _) => s.to_string(),
                };
                *table
                    .entry((
                        name.clone(),
                        format!("{}.{member}", String::from_utf8_lossy(&l.subrecord)),
                    ))
                    .or_default()
                    .entry(what)
                    .or_default() += 1;
            }
        }
    }
    eprintln!("winning records: {records:?}");
    eprintln!("| Type | Member | Names (count) |");
    eprintln!("| ---- | ------ | ------------- |");
    for ((t, m), counts) in &table {
        let list: Vec<String> = counts.iter().map(|(k, v)| format!("{k} {v}")).collect();
        eprintln!("| {t} | `{m}` | {} |", list.join(", "));
    }
}
