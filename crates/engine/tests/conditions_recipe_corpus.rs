//! KK-272: the conditions recipe (`docs/recipes/conditions.toml`) built on the official
//! `Starfield.esm` (`KITKRAFT_DATA_DIR`; skips when unset), through one `Engine` session
//! and `execute` only, and every claim of `docs/recipes/conditions.md` checked:
//!
//! - `build` writes one new form (a `COBJ` clone of `co_mfg_Tier01_AdaptiveFrame`),
//!   `Starfield.esm` the only master, validated;
//! - the clone has one condition, written member by member, and it reads back through
//!   `inspect`: operator `EqualTo`, no flags, comparison value 1.0, function 874
//!   (`IsResearchComplete`), parameter 1 the research project `Weapon_Research_Ammo_01`
//!   (`Starfield.esm:2C0D72`), run on the subject, parameter 3 -1;
//! - its bytes equal the vanilla shell recipe's condition (`co_ammo_12gShotgunShell`)
//!   except the two unused bytes, which the vanilla record holds as non-zero leftovers;
//! - `refs` resolves the research project link; `validate` 0 errors and 0 warnings;
//!   `roundtrip --verify` identical; a second build is byte-identical, and its SHA-256 is
//!   the one `docs/recipes/conditions.md` records.
//!
//! Everything is written under the OS temp dir. Run in release:
//! `cargo test -p kitkraft-engine --release --test conditions_recipe_corpus -- --include-ignored --nocapture`

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
    let text = std::fs::read_to_string(repo("docs/recipes/conditions.toml")).unwrap();
    let anchor = "plugins = [\"Starfield.esm\"]\n";
    assert!(text.contains(anchor), "the spec's [load_order] changed");
    let text = text.replacen(
        anchor,
        &format!("{anchor}data_dir = '{}'\n", dir.display()),
        1,
    );
    let spec = into.join("conditions.toml");
    std::fs::write(&spec, text).unwrap();
    spec
}

/// SHA-256 of the built plugin, as `docs/recipes/conditions.md` records it.
const SHA256: &str = "a9a3c57eca99be2867678480b76abb90bede7bd772a393dfa9825dc988565f05";

/// The `CTDA` subrecords of the winning version `data` (an `inspect` result).
fn ctdas(data: &Value) -> Vec<Value> {
    data["versions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["winner"] == json!(true))
        .flat_map(|v| v["subrecords"].as_array().unwrap().iter())
        .filter(|s| s["signature"] == json!("CTDA"))
        .cloned()
        .collect()
}

#[test]
#[ignore = "release: indexes Starfield.esm"]
fn the_conditions_recipe_builds_and_every_claim_holds() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let data = dir.display().to_string();
    let out = std::env::temp_dir().join(format!("kitkraft-conditions-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let engine = Engine::with_default_handlers();

    // Build.
    let spec = spec_for(&dir, &out);
    let plugin = out.join("KitKraftConditions.esm");
    let built = engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": plugin.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("build: {e}"));
    let b = &built["data"];
    assert_eq!(b["passed"], json!(true), "{:#}", b["validate"]);
    assert_eq!(b["masters"], json!(["Starfield.esm"]));
    let new = b["new_forms"].as_array().unwrap();
    assert_eq!(new.len(), 1, "{new:#?}");
    assert_eq!(new[0]["signature"], json!("COBJ"));
    assert_eq!(
        new[0]["editor_id"],
        json!("KitKraft_co_ConditionedShotgunShells")
    );
    let form = "KitKraftConditions.esm:001000";
    let bytes = std::fs::read(&plugin).unwrap();

    // The condition, read back.
    let plugins = json!(["Starfield.esm", plugin.display().to_string()]);
    let ours = engine
        .execute(
            "inspect",
            json!({"form": form, "plugins": plugins, "data_dir": data, "full": true}),
        )
        .unwrap();
    let conds = ctdas(&ours["data"]);
    assert_eq!(conds.len(), 1, "{conds:#?}");
    let v = &conds[0]["value"];
    eprintln!("the recipe's condition: {v:#}");
    assert_eq!(v["Type"]["Operator"], json!(0));
    assert_eq!(v["Type"]["Flags"], json!(0));
    assert_eq!(v["ComparisonValue"]["Float"], json!(1.0));
    assert_eq!(v["Function"], json!(874));
    assert_eq!(
        v["Parameter1"]["ResearchProject"]["form"],
        json!("Starfield.esm:2C0D72")
    );
    assert_eq!(v["RunOn"], json!(0));
    assert_eq!(v["Parameter3"]["Integer"], json!(-1));
    // The vanilla shell recipe's condition, byte for byte but the unused bytes 10 and 11.
    let vanilla = engine
        .execute(
            "inspect",
            json!({"form": "Starfield.esm:2BBFAC", "plugins": ["Starfield.esm"],
                   "data_dir": data, "full": true}),
        )
        .unwrap();
    let theirs = ctdas(&vanilla["data"]);
    assert_eq!(theirs.len(), 1);
    let hex = |c: &Value| c["payload_hex"].as_str().unwrap().to_string();
    let (a, z) = (hex(&conds[0]), hex(&theirs[0]));
    eprintln!("ours    {a}\nvanilla {z}");
    assert_eq!(a.len(), 64);
    assert_eq!(&a[..20], &z[..20], "type, comparison value, function");
    assert_eq!(&a[20..24], "0000", "the unused bytes are written as zeros");
    assert_eq!(
        &a[24..],
        &z[24..],
        "parameters, run on, reference, parameter 3"
    );

    // The research project link resolves.
    let r = engine
        .execute(
            "refs",
            json!({"form": form, "plugins": plugins, "data_dir": data}),
        )
        .unwrap();
    let edges = r["data"]["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["to"] == json!("Starfield.esm:2C0D72") && e["status"] == json!("resolved")),
        "{edges:#?}"
    );

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
    let again = out.join("again").join("KitKraftConditions.esm");
    std::fs::create_dir_all(again.parent().unwrap()).unwrap();
    engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": again.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("second build: {e}"));
    assert_eq!(std::fs::read(&again).unwrap(), bytes, "builds differ");

    // testplan writes its files and names the plugin's SHA-256.
    let plan = out.join("test");
    let planned = engine
        .execute(
            "testplan",
            json!({"plugin": plugin.display().to_string(), "out": plan.display().to_string(),
                   "plugins": ["Starfield.esm"], "data_dir": data,
                   "spec": spec.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("testplan: {e}"));
    eprintln!(
        "KitKraftConditions.esm: {} bytes, SHA-256 {}",
        bytes.len(),
        planned["data"]["sha256"]
    );
    assert_eq!(
        planned["data"]["sha256"],
        json!(SHA256),
        "the plugin changed: record the new hash in docs/recipes/conditions.md"
    );
    let _ = std::fs::remove_dir_all(&out);
}
