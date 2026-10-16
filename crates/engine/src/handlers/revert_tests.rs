//! `revert` (M7.S7) through `execute` on the synthetic load order of the `set` tests: a
//! `set` then a `revert` leaves the plugin with no difference from before the `set`
//! (checked with `diff`), a dry run changes nothing, `to: previous` copies the version
//! before the plugin, and a form the plugin defines is refused.

#![allow(clippy::arithmetic_side_effects)]

use std::path::Path;

use serde_json::{json, Value};

use super::set_tests::data;
use crate::edit::patch_tests::dir;
use crate::execute;

fn lo(data: &Path) -> Value {
    json!({
        "plugins": ["Source.esm", "Override.esp"], "data_dir": data.display().to_string(),
        "no_cache": true,
    })
}

fn with(base: &Value, extra: Value) -> Value {
    let mut a = base.clone();
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

fn counts(a: &Path, b: &Path, data: &Path) -> Value {
    execute(
        "diff",
        json!({"a": a.display().to_string(), "b": b.display().to_string(), "data_dir": data.display().to_string()}),
    )
    .unwrap()["data"]["counts"]
        .clone()
}

#[test]
fn set_then_revert_leaves_no_difference() {
    let d = dir("revert");
    let data = data(&d);
    let base = lo(&data);
    let patch = d.join("Patch.esm");
    let set = |form: &str, name: &str| {
        execute(
            "set",
            with(
                &base,
                json!({"form": form, "fields": {"FULL": name}, "into": patch.display().to_string()}),
            ),
        )
        .unwrap();
    };
    set("Source.esm:000800", "First");
    let original = d.join("Original.esm");
    std::fs::copy(&patch, &original).unwrap();
    let before = std::fs::read(&patch).unwrap();

    // A second edit, then its revert.
    set("Source.esm:000801", "Second");
    let after_set = std::fs::read(&patch).unwrap();
    assert_ne!(after_set, before);
    let revert = |extra: Value| {
        execute(
            "revert",
            with(
                &base,
                with(&json!({"plugin": patch.display().to_string()}), extra),
            ),
        )
    };

    // Dry run: the diff of the file before and after, and no write.
    let dry = revert(json!({"form": "Source.esm:000801", "dry_run": true})).unwrap();
    assert_eq!(std::fs::read(&patch).unwrap(), after_set);
    let dd = &dry["data"];
    assert_eq!(dd["dry_run"], json!(true));
    assert_eq!(dd["written"], json!(false));
    assert_eq!(dd["would_write"], json!(true), "{dd:#}");
    assert_eq!(dd["diff"]["counts"]["removed"], json!(1), "{dd:#}");
    assert_eq!(dd["raw_form_id"], json!("00000801"));
    assert!(dry["limitations"].as_array().unwrap().iter().any(|l| l
        .as_str()
        .unwrap()
        .starts_with("dry run: nothing was written")));

    // The revert: by EditorID; the file is the one before the second `set` again.
    let done = revert(json!({"form": "OtherRifle"})).unwrap();
    assert_eq!(done["data"]["written"], json!(true));
    assert_eq!(done["data"]["form"], json!("Source.esm:000801"));
    let c = counts(&original, &patch, &data);
    assert_eq!(
        (
            c["added"].clone(),
            c["removed"].clone(),
            c["changed"].clone()
        ),
        (json!(0), json!(0), json!(0)),
        "{c:#}"
    );

    // `to: previous`: the override of 000800 becomes a copy of the version before it.
    let prev = revert(json!({"form": "Source.esm:000800", "to": "previous"})).unwrap();
    assert_eq!(prev["data"]["to"], json!("previous"));
    assert_eq!(
        prev["data"]["diff"]["counts"]["changed"],
        json!(1),
        "{prev:#}"
    );
    let shown = execute(
        "inspect",
        json!({"form": "00000800", "plugin": patch.display().to_string()}),
    )
    .unwrap();
    let full = shown["data"]["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == "FULL")
        .unwrap()["payload_hex"]
        .as_str()
        .unwrap()
        .to_string();
    // "Old Earth Hunting Rifle\0", inlined from the string table.
    assert!(full.starts_with("4F6C64204561727468"), "{full}");

    // Refusals: a form the plugin defines, a form it does not have, a load-order plugin.
    execute(
        "new",
        with(
            &base,
            json!({"type": "glob", "edid": "KC_Own", "masters": ["Source.esm"], "into": patch.display().to_string()}),
        ),
    )
    .unwrap();
    let own = revert(json!({"form": "KC_Own"})).unwrap_err();
    assert!(own.to_string().contains("revert.own-form"), "{own}");
    let missing = revert(json!({"form": "Source.esm:000999"})).unwrap_err();
    assert!(
        missing.to_string().contains("revert.not-in-plugin"),
        "{missing}"
    );
    let protected = execute(
        "revert",
        with(
            &base,
            json!({"plugin": data.join("Override.esp").display().to_string(), "form": "OtherRifle"}),
        ),
    )
    .unwrap_err();
    assert_eq!(protected.kind(), "failed");
    let _ = std::fs::remove_dir_all(&d);
}
