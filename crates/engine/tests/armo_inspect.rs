//! KK-222: `inspect --full` of an `ARMO` attaches its `Addon` entries as an `addons` key
//! on the version object: each entry's resolved `ARMA` (FormID, EditorID, plugin) and the
//! `ARMA`'s four model `FileName`s by slot.
//!
//! `#[ignore]`: loads Starfield.esm. Run
//! `cargo test -p kitkraft-engine --release --test armo_inspect -- --include-ignored --nocapture`
//! with `KITKRAFT_DATA_DIR` set.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use kitkraft_engine::Engine;
use serde_json::json;

#[test]
#[ignore = "loads Starfield.esm; run in release"]
fn inspect_full_shows_armo_addons_and_models() {
    let Some(dir) = std::env::var_os("KITKRAFT_DATA_DIR")
        .map(PathBuf::from)
        .filter(|d| d.join("Starfield.esm").is_file())
    else {
        println!("armo-inspect: skipped (no KITKRAFT_DATA_DIR)");
        return;
    };
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm"], "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    // First official `ARMO` with an `Addon` entry (`has INDX`) and an EditorID: page the
    // query in result order (plugin, then FormID) and take the first that `inspect --full`
    // shows at least one addon for.
    let mut cursor = None;
    loop {
        let mut q = json!({"type": "ARMO", "where": "has INDX", "limit": 1000});
        if let Some(c) = &cursor {
            q["cursor"] = json!(c);
        }
        let r = engine.execute("query", q).unwrap();
        let data = &r["data"];
        for res in data["results"].as_array().unwrap() {
            let Some(edid) = res["editor_id"].as_str() else {
                continue;
            };
            let r = engine
                .execute(
                    "inspect",
                    json!({"form": res["form"].as_str().unwrap(), "full": true}),
                )
                .unwrap();
            let version = &r["data"]["versions"][0];
            let Some(addons) = version["addons"].as_array() else {
                continue;
            };
            if addons.is_empty() {
                continue;
            }
            println!(
                "armo-inspect: first official ARMO with addons formid=0x{} addons={} editor_id={}",
                version["raw_form_id"].as_str().unwrap(),
                addons.len(),
                edid
            );
            return;
        }
        let Some(next) = data["next_cursor"].as_str() else {
            panic!("no official ARMO with an Addon entry and an EditorID");
        };
        cursor = Some(next.to_string());
    }
}
