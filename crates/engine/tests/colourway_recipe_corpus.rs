//! KK-020: the KitKraft colourway recipe (`docs/recipes/colourway.toml`) built on the
//! official `Starfield.esm` (`KITKRAFT_DATA_DIR`; skips when unset), through one `Engine`
//! session and `execute` only, and every claim of `docs/recipes/colourway.md` checked:
//!
//! - `build` writes two new forms with `Starfield.esm` the only master: an `LMSW` clone
//!   (`KitKraftRecipeColourwaySwap`) whose four material-swap entries keep the vanilla
//!   keys and point at files under `KitKraft/Colourway/`, and an `ARMO` clone
//!   (`KitKraftRecipeColourwayOutfit`) whose object-template property 14 names our swap;
//! - the build's `swap_overrides` report lists each entry's key, vanilla override and new
//!   override;
//! - `refs` from the outfit: every link resolves or is null, and property 14's FormID
//!   resolves to our swap;
//! - `inspect --full` of the swap lists the four vanilla keys, each with the new override;
//! - `validate` 0 errors and 0 warnings; `roundtrip --verify` identical; a second build is
//!   byte-identical, and its SHA-256 is the one `docs/recipes/colourway.md` records.
//!
//! Everything is written under the OS temp dir. Run in release:
//! `cargo test -p kitkraft-engine --release --test colourway_recipe_corpus -- --include-ignored --exact the_colourway_recipe_builds_and_every_claim_holds --nocapture`

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
    let text = std::fs::read_to_string(repo("docs/recipes/colourway.toml")).unwrap();
    let anchor = "plugins = [\"Starfield.esm\"]\n";
    assert!(text.contains(anchor), "the spec's [load_order] changed");
    let text = text.replacen(
        anchor,
        &format!("{anchor}data_dir = '{}'\n", dir.display()),
        1,
    );
    let spec = into.join("colourway.toml");
    std::fs::write(&spec, text).unwrap();
    spec
}

/// The two new forms the recipe claims, in FormID order: (object ID, type, EditorID).
const FORMS: [(u32, &str, &str); 2] = [
    (0x1000, "LMSW", "KitKraftRecipeColourwaySwap"),
    (0x1001, "ARMO", "KitKraftRecipeColourwayOutfit"),
];

/// The vanilla keys and overrides of `Starfield.esm:1F1DCA`, and the new override of each:
/// key (as stored), vanilla override (the report's `from`), new override (the report's
/// `to`).
const ENTRIES: [(&str, &str, &str); 4] = [
    (
        "Clothes\\Outfit_Mei_Devine\\Outfit_Mei_Devine_UpperBody.mat",
        "Clothes\\Outfit_Mei_Devine\\Outfit_Mei_Devine_NavyTan_UpperBody.mat",
        "KitKraft\\Colourway\\Outfit_Mei_Devine_NavyTan_UpperBody.mat",
    ),
    (
        "Clothes\\Outfit_Mei_Devine\\Outfit_Mei_Devine_LowerBody.mat",
        "Clothes\\Outfit_Mei_Devine\\Outfit_Mei_Devine_NavyTan_LowerBody.mat",
        "KitKraft\\Colourway\\Outfit_Mei_Devine_NavyTan_LowerBody.mat",
    ),
    (
        "Clothes\\Outfit_Mei_Devine\\Outfit_Mei_Devine_Tights.mat",
        "Clothes\\Outfit_Mei_Devine\\Outfit_Mei_Devine_NavyTan_Tights.mat",
        "KitKraft\\Colourway\\Outfit_Mei_Devine_NavyTan_Tights.mat",
    ),
    (
        "Clothes\\Outfit_Mei_Devine\\Outfit_Mei_Devine_Tights_M.mat",
        "Clothes\\Outfit_Mei_Devine\\Outfit_Mei_Devine_NavyTan_Tights_M.mat",
        "KitKraft\\Colourway\\Outfit_Mei_Devine_NavyTan_Tights_M.mat",
    ),
];

/// SHA-256 of the built plugin, as `docs/recipes/colourway.md` records it.
const SHA256: &str = "bd796e3f126df9eb64bce1040eeb5527a0308853573e158ce4ffa41f192ed7e0";

fn form(id: u32) -> String {
    format!("KitKraftColourway.esm:{id:06X}")
}

/// `refs` of one form (outgoing, depth 1): every edge resolves or is null; the edges by path.
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

fn target(edges: &[Value], path: &str) -> Value {
    edges
        .iter()
        .find(|e| e["path"] == json!(path))
        .map(|e| e["to"].clone())
        .unwrap_or_else(|| panic!("no link {path} in {edges:#?}"))
}

#[test]
#[ignore = "release: indexes Starfield.esm (cached after the first run)"]
fn the_colourway_recipe_builds_and_every_claim_holds() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let data = dir.display().to_string();
    let out = std::env::temp_dir().join(format!("kitkraft-colourway-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let engine = Engine::with_default_handlers();

    // Build.
    let spec = spec_for(&dir, &out);
    let plugin = out.join("KitKraftColourway.esm");
    let built = engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": plugin.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("build: {e}"));
    let b = &built["data"];
    assert_eq!(b["passed"], json!(true), "{:#}", b["validate"]);
    assert_eq!(b["masters"], json!(["Starfield.esm"]));
    let written: Vec<(String, String, String)> = b["new_forms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["form_id"].as_str().unwrap().to_string(),
                f["signature"].as_str().unwrap().to_string(),
                f["editor_id"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let want: Vec<(String, String, String)> = FORMS
        .iter()
        .map(|(id, sig, edid)| (format!("0100{id:04X}"), sig.to_string(), edid.to_string()))
        .collect();
    assert_eq!(written, want);
    let bytes = std::fs::read(&plugin).unwrap();
    eprintln!(
        "KitKraftColourway.esm: {} bytes, {}",
        bytes.len(),
        b["summary"].as_str().unwrap_or("")
    );

    // The swap report: one `[[clone]]` with `swap_overrides`, one report per entry, each the
    // entry's key, the vanilla override and the new one.
    let reports = b["swap_overrides"].as_array().unwrap();
    assert_eq!(reports.len(), 1, "{reports:#?}");
    let rows = reports[0].as_array().unwrap();
    assert_eq!(rows.len(), ENTRIES.len(), "{rows:#?}");
    for (row, (key, from, to)) in rows.iter().zip(ENTRIES.iter()) {
        assert_eq!(row["key"], json!(key), "{row:#?}");
        assert_eq!(row["from"], json!(from), "{row:#?}");
        assert_eq!(row["to"], json!(to), "{row:#?}");
    }

    // Links of the outfit: everything resolves or is null, and property 14 names our swap.
    let plugins = json!(["Starfield.esm", plugin.display().to_string()]);
    let outfit = links(&engine, &plugins, &data, &form(0x1001));
    assert_eq!(
        target(&outfit, "OBTS.TemplateItem.Properties[0].Value1.FormId"),
        json!(form(0x1000))
    );
    // The swap names no forms (its .mat paths are strings the schema keeps raw).
    assert!(links(&engine, &plugins, &data, &form(0x1000)).is_empty());

    // The swap's REFL: the four vanilla keys, each with the new override.
    let swap = engine
        .execute(
            "inspect",
            json!({"form": form(0x1000), "plugins": plugins, "data_dir": data, "full": true}),
        )
        .unwrap();
    let refl = swap["data"]["versions"][0]["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == json!("REFL"))
        .unwrap_or_else(|| panic!("no REFL in {:#}", swap["data"]));
    let entries = refl["lmsw"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), ENTRIES.len(), "{entries:#?}");
    for (row, (key, _, to)) in entries.iter().zip(ENTRIES.iter()) {
        assert_eq!(row["key"], json!(key), "{row:#?}");
        assert_eq!(row["override"], json!(to), "{row:#?}");
    }

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
    let again = out.join("again").join("KitKraftColourway.esm");
    std::fs::create_dir_all(again.parent().unwrap()).unwrap();
    engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": again.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("second build: {e}"));
    assert_eq!(std::fs::read(&again).unwrap(), bytes, "builds differ");

    // testplan: the plugin's SHA-256, as docs/recipes/colourway.md records it.
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
        "the plugin changed: record the new hash in docs/recipes/colourway.md"
    );
    eprintln!("colourway-recipe: ok");
    let _ = std::fs::remove_dir_all(&out);
}
