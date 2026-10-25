//! KK-007: `inspect --full` adds the decoded swap entries of the `LMSW` record's `REFL`
//! to the subrecord entry under `lmsw`, next to how `vmad` is attached.
//!
//! `#[ignore]`: loads Starfield.esm. Run
//! `cargo test -p kitkraft-engine --release --test refl_inspect -- --include-ignored --nocapture`
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
fn inspect_full_shows_lmsw_swap_entries() {
    let Some(dir) = std::env::var_os("KITKRAFT_DATA_DIR")
        .or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
        .map(PathBuf::from)
        .filter(|d| d.join("Starfield.esm").is_file())
    else {
        println!("skipped: KITKRAFT_DATA_DIR is unset or lacks Starfield.esm");
        return;
    };
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm"], "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    // M4: the LMSW of Starfield.esm:1991B1 (LMS_Clothes_Matteo_Hat) maps one key material
    // path to its override.
    let r = engine
        .execute(
            "inspect",
            json!({"form": "LMS_Clothes_Matteo_Hat", "full": true}),
        )
        .unwrap();
    let subs = &r["data"]["versions"][0]["subrecords"];
    let refl = subs
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == "REFL")
        .unwrap();
    assert_eq!(
        refl["lmsw"]["entries"],
        json!([{
            "key": "Clothes\\Outfit_Sanctum_Priest\\Outfit_Sanctum_Priest_Hat.mat",
            "override": "Clothes\\Outfit_Sanctum_Priest\\Outfit_Sanctum_Priest_Hat_Ajun.mat",
        }]),
        "{r}"
    );
    println!("refl-inspect: ok");
}
