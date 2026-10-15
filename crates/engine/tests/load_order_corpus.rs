//! M2.D7, M2.D9, M2.D11 on the official files (`KITKRAFT_DATA_DIR`, release, ignored):
//! the Old Earth Hunting Rifle's FULL resolves through `Starfield - Localization.ba2`, and
//! `query` finds the weapon by that name.
//!
//! `cargo test -p kitkraft-engine --release --test load_order_corpus -- --ignored`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
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

#[test]
#[ignore = "release: indexes Starfield.esm and ShatteredSpace.esm (cached after the first run)"]
fn official_lstrings_resolve_and_query_finds_the_rifle() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm", "ShatteredSpace.esm"],
                   "data_dir": dir.display().to_string()}),
        )
        .unwrap();

    // M2.D7 + M2.D9: FULL of the WEAP picked by EditorID, with the table offset as evidence.
    let r = engine
        .execute("inspect", json!({"form": "RussianHuntingRifle"}))
        .unwrap();
    let data = &r["data"];
    assert_eq!(data["global"], "Starfield.esm:21BBCD");
    let version = &data["versions"][0];
    let full = version["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == "FULL")
        .unwrap();
    assert_eq!(full["string_id"], "0x0002E469");
    assert_eq!(full["text"], "Old Earth Hunting Rifle");
    assert_eq!(
        full["string_table"],
        "Starfield - Localization.ba2:strings/starfield_en.strings"
    );
    let offset = full["string_offset"].as_u64().unwrap();
    let evidence = r["evidence"].as_array().unwrap();
    assert!(evidence.iter().any(|e| e["file"] == full["string_table"]
        && e["offset"].as_u64() == Some(offset)
        && e["subrecord"] == "FULL"));
    // The FULL payload itself is evidenced in the plugin (offset 21,546,047 per lane G).
    assert!(evidence
        .iter()
        .any(|e| e["file"] == "Starfield.esm" && e["subrecord"] == "FULL"));

    // M2.D11: the plan's acceptance query.
    let started = std::time::Instant::now();
    let r = engine
        .execute(
            "query",
            json!({"type": "WEAP", "where": "FULL ~ \"Old Earth Hunting Rifle\""}),
        )
        .unwrap();
    let elapsed = started.elapsed();
    let data = &r["data"];
    let forms: Vec<&Value> = data["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| &x["form"])
        .collect();
    assert!(forms.contains(&&json!("Starfield.esm:21BBCD")), "{data}");
    assert_eq!(data["unknown"], 0, "{data}");
    assert!(elapsed.as_secs() <= 10, "{elapsed:?}");
    eprintln!("{}  ({elapsed:?})", data["summary"]);

    // A missing table: the ID plus a limitation, never "absent".
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm"], "data_dir": dir.display().to_string(),
                   "language": "xx"}),
        )
        .unwrap();
    let r = engine
        .execute("inspect", json!({"form": "RussianHuntingRifle"}))
        .unwrap();
    let full = r["data"]["versions"][0]["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == "FULL")
        .unwrap()
        .clone();
    assert_eq!(full["string_id"], "0x0002E469");
    assert!(full.get("text").is_none(), "{full}");
    assert!(r["limitations"].as_array().unwrap().iter().any(|l| l
        .as_str()
        .unwrap()
        .contains("no xx string tables for Starfield.esm")));
}
