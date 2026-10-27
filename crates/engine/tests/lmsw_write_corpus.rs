//! KK-019: `swap_overrides` on `set` and `clone`, through one `Engine` session.
//!
//! The vanilla `LMSW` `Starfield.esm:1991B1` (`LMS_Clothes_Matteo_Hat`) is cloned with one
//! override into a temp plugin, and `set` with another override into a second plugin,
//! both with forward slashes in the paths (stored as backslashes, as the corpus does).
//! For each: `inspect --full` shows the new override under the old key, `validate` finds
//! 0 errors, and `roundtrip --verify` is identical; the result carries
//! `swap_overrides: [{key, from, to}]`. An unknown key is refused, naming it in its
//! stored form.
//!
//! `#[ignore]`: loads Starfield.esm. Run
//! `cargo test -p kitkraft-engine --release --test lmsw_write_corpus -- --include-ignored --nocapture`
//! with `KITKRAFT_DATA_DIR` set.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use kitkraft_engine::Engine;
use serde_json::json;

/// The swap entry's key material path, as given (forward slashes).
const KEY: &str = "Clothes/Outfit_Sanctum_Priest/Outfit_Sanctum_Priest_Hat.mat";
/// The key as stored in the corpus (backslashes), in the results and the inspection.
const KEY_ST: &str = "Clothes\\Outfit_Sanctum_Priest\\Outfit_Sanctum_Priest_Hat.mat";
/// The override it holds in the vanilla record.
const FROM: &str = "Clothes\\Outfit_Sanctum_Priest\\Outfit_Sanctum_Priest_Hat_Ajun.mat";
/// The two overrides the test writes, forward slashes as given, backslashes as stored.
const TO_CLONE: &str = "Textures/KitKraft/lmsw_write_clone_hat.mat";
const TO_CLONE_ST: &str = "Textures\\KitKraft\\lmsw_write_clone_hat.mat";
const TO_SET: &str = "Textures/KitKraft/lmsw_write_set_hat.mat";
const TO_SET_ST: &str = "Textures\\KitKraft\\lmsw_write_set_hat.mat";

/// `inspect --full` shows `to_st` under the old key; `validate` 0 errors; `roundtrip`
/// `--verify` identical.
fn check_written(engine: &Engine, data: &str, plugin: &Path, form: &str, to_st: &str) {
    let plugins = json!(["Starfield.esm", plugin.display().to_string()]);
    let insp = engine
        .execute(
            "inspect",
            json!({"form": form, "full": true, "plugins": plugins, "data_dir": data}),
        )
        .unwrap_or_else(|e| panic!("inspect {form}: {e}"));
    let versions = insp["data"]["versions"].as_array().unwrap();
    let winner = versions
        .iter()
        .find(|v| v["winner"] == json!(true))
        .unwrap();
    let subs = &winner["subrecords"];
    let refl = subs
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == "REFL")
        .unwrap();
    assert_eq!(
        refl["lmsw"]["entries"],
        json!([{ "key": KEY_ST, "override": to_st }]),
        "{insp}"
    );
    let v = engine
        .execute("validate", json!({"plugin": plugin.display().to_string()}))
        .unwrap_or_else(|e| panic!("validate: {e}"));
    assert_eq!(v["data"]["errors"], json!(0), "{:#}", v["data"]);
    let rt = engine
        .execute(
            "roundtrip",
            json!({"plugin": plugin.display().to_string(), "verify": true}),
        )
        .unwrap_or_else(|e| panic!("roundtrip: {e}"));
    assert_eq!(rt["data"]["identical"], json!(true), "{:#}", rt["data"]);
}

#[test]
#[ignore = "loads Starfield.esm; run in release"]
fn swap_overrides_clone_and_set_an_lmsw() {
    let Some(dir) = std::env::var_os("KITKRAFT_DATA_DIR")
        .or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
        .map(PathBuf::from)
        .filter(|d| d.join("Starfield.esm").is_file())
    else {
        println!("skip: KITKRAFT_DATA_DIR is unset or lacks Starfield.esm");
        return;
    };
    let data = dir.display().to_string();
    let out = std::env::temp_dir().join(format!("kitkraft-lmsw-write-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm"], "data_dir": data}),
        )
        .unwrap_or_else(|e| panic!("load: {e}"));

    // Clone 1991B1 with one override, forward slashes in both paths.
    let clone1 = out.join("KK_LMSW_Clone.esp");
    let r = engine
        .execute(
            "clone",
            json!({
                "form": "Starfield.esm:1991B1",
                "edid": "KK_LMSW_SwapTest",
                "into": clone1.display().to_string(),
                "swap_overrides": [{ "key": KEY, "override": TO_CLONE }],
            }),
        )
        .unwrap_or_else(|e| panic!("clone: {e}"));
    assert_eq!(
        r["data"]["swap_overrides"],
        json!([{ "key": KEY_ST, "from": FROM, "to": TO_CLONE_ST }]),
        "{r}"
    );
    let form = r["data"]["new_forms"][0]["form"]
        .as_str()
        .unwrap()
        .to_string();
    check_written(&engine, &data, &clone1, &form, TO_CLONE_ST);

    // Set the same form with another override, into a second plugin.
    let set1 = out.join("KK_LMSW_Set.esp");
    let r = engine
        .execute(
            "set",
            json!({
                "form": "Starfield.esm:1991B1",
                "fields": {},
                "into": set1.display().to_string(),
                "swap_overrides": [{ "key": KEY, "override": TO_SET }],
            }),
        )
        .unwrap_or_else(|e| panic!("set: {e}"));
    assert_eq!(
        r["data"]["swap_overrides"],
        json!([{ "key": KEY_ST, "from": FROM, "to": TO_SET_ST }]),
        "{r}"
    );
    check_written(&engine, &data, &set1, "Starfield.esm:1991B1", TO_SET_ST);

    // An unknown key is refused, naming it in its stored form; the file is never written.
    // A fresh target: `check_written`'s `inspect` put `set1` in the session's load order,
    // and a patch is never a load-order plugin.
    let bad1 = out.join("KK_LMSW_Bad.esp");
    let err = engine
        .execute(
            "set",
            json!({
                "form": "Starfield.esm:1991B1",
                "fields": {},
                "into": bad1.display().to_string(),
                "swap_overrides": [{ "key": "No/Such/key.mat", "override": TO_SET }],
            }),
        )
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("No\\Such\\key.mat"), "{msg}");
    assert!(!bad1.exists(), "a refused swap must write nothing");

    println!("lmsw-write: ok");
    let _ = std::fs::remove_dir_all(&out);
}
