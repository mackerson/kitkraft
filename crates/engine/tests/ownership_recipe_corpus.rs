//! KK-015 (M5.D5): the ownership recipe's new-faction variant
//! (`docs/recipes/ownership-faction.toml`) built on the official `Starfield.esm`
//! (`KITKRAFT_DATA_DIR`; skips when unset), through one `Engine` session and `execute`
//! only, and every claim of the "Variant" section of `docs/recipes/ownership.md`
//! checked:
//!
//! - `build` writes the one new form (a FACT, `KitKraftRecipeOwnerFaction`) and
//!   overrides the cell `CityNewAtlantisFreestarEmbassy` (`Starfield.esm:017307`) so its
//!   `XOWN.Owner` names that faction, `Starfield.esm` the only master, validated;
//! - `refs` on the cell's override shows `XOWN` naming the new faction (resolved, not
//!   dangling);
//! - `validate` 0 errors and 0 warnings; `roundtrip --verify` identical; a second build
//!   is byte-identical, and its SHA-256 is the one the recipe records.
//!
//! Everything is written under the OS temp dir. Run in release:
//! `cargo test -p kitkraft-engine --release --test ownership_recipe_corpus -- --include-ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::{Path, PathBuf};

use kitkraft_engine::Engine;
use serde_json::{json, Value};

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)
}

/// The recipe's spec with the Data folder named in its `[load_order]`.
fn spec_for(dir: &Path, into: &Path) -> PathBuf {
    let text = std::fs::read_to_string(repo("docs/recipes/ownership-faction.toml")).unwrap();
    let anchor = "plugins = [\"Starfield.esm\"]\n";
    assert!(text.contains(anchor), "the spec's [load_order] changed");
    let text = text.replacen(
        anchor,
        &format!("{anchor}data_dir = '{}'\n", dir.display()),
        1,
    );
    let spec = into.join("ownership-faction.toml");
    std::fs::write(&spec, text).unwrap();
    spec
}

/// SHA-256 of the built plugin, as the "Variant" section of
/// `docs/recipes/ownership.md` records it.
const SHA256: &str = "e3575858608ad4894af50984430879c41d2e2179ccf3358636c07899f1d09af4";

/// `refs` of one form (outgoing, depth 1): the edges by path; every one resolves or is null.
fn links(engine: &Engine, plugins: &Value, data: &str, form: &str) -> Vec<Value> {
    let r = engine
        .execute(
            "refs",
            json!({"form": form, "plugins": plugins, "data_dir": data}),
        )
        .unwrap_or_else(|e| panic!("refs {form}: {e}"));
    let d = &r["data"];
    let by_status = d["counts"]["by_status"].as_object().unwrap();
    assert!(
        by_status.keys().all(|k| k == "resolved" || k == "null"),
        "{form}: a link that does not resolve: {by_status:?}\n{}",
        d["summary"]
    );
    d["edges"].as_array().unwrap().clone()
}

#[test]
#[ignore = "release: indexes Starfield.esm"]
fn the_faction_ownership_recipe_builds_and_every_claim_holds() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let data = dir.display().to_string();
    let out = std::env::temp_dir()
        .join(format!("kitkraft-ownership-faction-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let engine = Engine::with_default_handlers();

    // Build.
    let spec = spec_for(&dir, &out);
    let plugin = out.join("KitKraftOwnershipFaction.esm");
    let built = engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": plugin.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("build: {e}"));
    let b = &built["data"];
    assert_eq!(b["passed"], json!(true), "{:#}", b["validate"]);
    assert_eq!(b["masters"], json!(["Starfield.esm"]));
    // The one new form: the faction, object 0x1000 of master 01.
    let new_forms = b["new_forms"].as_array().unwrap();
    assert_eq!(new_forms.len(), 1, "{:#}", b);
    assert_eq!(new_forms[0]["form_id"], json!("01001000"), "{:#}", b);
    assert_eq!(new_forms[0]["signature"], json!("FACT"), "{:#}", b);
    assert_eq!(new_forms[0]["editor_id"], json!("KitKraftRecipeOwnerFaction"), "{:#}", b);
    assert_eq!(b["placed"].as_array().unwrap().len(), 0, "{:#}", b);
    let bytes = std::fs::read(&plugin).unwrap();
    eprintln!(
        "KitKraftOwnershipFaction.esm: {} bytes, {}",
        bytes.len(),
        b["summary"].as_str().unwrap_or("")
    );

    // The cell's override: XOWN names the new faction, resolved, not dangling.
    // The path is XOWN.Owner.Owner: the subrecord, the CELL member name for it, and
    // the OwnerData field.
    let plugins = json!(["Starfield.esm", plugin.display().to_string()]);
    let cell = links(&engine, &plugins, &data, "Starfield.esm:017307");
    let edge = cell
        .iter()
        .find(|e| e["path"] == json!("XOWN.Owner.Owner"))
        .unwrap_or_else(|| panic!("no XOWN.Owner link in {cell:#?}"));
    assert_eq!(edge["status"], json!("resolved"), "{:#}", edge);
    assert_eq!(edge["to"], json!("KitKraftOwnershipFaction.esm:001000"), "{:#}", edge);
    assert_eq!(edge["to_signature"], json!("FACT"), "{:#}", edge);
    assert_eq!(edge["to_editor_id"], json!("KitKraftRecipeOwnerFaction"), "{:#}", edge);

    // validate, roundtrip, a second build.
    let v = engine
        .execute("validate", json!({"plugin": plugin.display().to_string()}))
        .unwrap();
    assert_eq!(v["data"]["errors"], json!(0), "{:#}", v["data"]);
    assert_eq!(v["data"]["warnings"], json!(0), "{:#}", v["data"]);
    let rt = engine
        .execute(
            "roundtrip",
            json!({"plugin": plugin.display().to_string(), "verify": true}),
        )
        .unwrap();
    assert_eq!(rt["data"]["identical"], json!(true), "{:#}", rt["data"]);
    let again = out.join("again").join("KitKraftOwnershipFaction.esm");
    std::fs::create_dir_all(again.parent().unwrap()).unwrap();
    engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": again.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("second build: {e}"));
    assert_eq!(std::fs::read(&again).unwrap(), bytes, "builds differ");

    // testplan: the plugin's SHA-256, as the "Variant" section of
    // docs/recipes/ownership.md records it.
    let plan = out.join("test");
    let planned = engine
        .execute(
            "testplan",
            json!({"plugin": plugin.display().to_string(), "out": plan.display().to_string(),
                   "plugins": ["Starfield.esm"], "data_dir": data,
                   "spec": spec.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("testplan: {e}"));
    assert_eq!(
        planned["data"]["sha256"],
        json!(SHA256),
        "the plugin changed: record the new hash in docs/recipes/ownership.md"
    );
    eprintln!("ownership-faction-recipe: ok");
    let _ = std::fs::remove_dir_all(&out);
}
