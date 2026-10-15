//! `new` and `clone` (M4.E1) through `execute` on the synthetic localized load order of
//! the `set` tests (`Source.esm` with two weapons and a string table, `Override.esp`).

#![allow(clippy::arithmetic_side_effects)]

use std::path::Path;

use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::index::{index_bytes, IndexOptions};
use serde_json::{json, Value};

use super::set_tests::data;
use crate::edit::patch_tests::dir;
use crate::execute;

fn call(
    command: &str,
    data: &Path,
    into: &Path,
    extra: Value,
) -> Result<Value, crate::EngineError> {
    let mut a = json!({
        "into": into.display().to_string(),
        "plugins": ["Source.esm", "Override.esp"], "data_dir": data.display().to_string(),
        "no_cache": true,
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    execute(command, a)
}

fn code(err: &crate::EngineError) -> String {
    let text = err.to_string();
    text.split(": ")
        .find(|p| p.starts_with("new.") || p.starts_with("clone."))
        .unwrap_or(&text)
        .to_string()
}

#[test]
fn a_new_global_gets_a_formid_its_editor_id_and_its_fields() {
    let d = dir("new-glob");
    let data = data(&d);
    let into = d.join("New.esp");
    let r = call(
        "new",
        &data,
        &into,
        json!({"type": "glob", "edid": "KC_TestGlobal", "fields": {"FLTV": 2.5}, "masters": ["Source.esm"]}),
    )
    .unwrap();
    let new = &r["data"]["new_forms"][0];
    assert_eq!(new["form_id"], json!("01001000"));
    assert_eq!(new["form"], json!("New.esp:00001000"));
    assert_eq!(new["signature"], json!("GLOB"));
    assert_eq!(new["editor_id"], json!("KC_TestGlobal"));
    assert_eq!(r["data"]["next_object_id"], json!("FF001001"));
    assert_eq!(r["data"]["masters"], json!(["Source.esm"]));
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    let bytes = std::fs::read(&into).unwrap();
    assert!(bytes.windows(14).any(|w| w == b"KC_TestGlobal\0"));
    assert!(bytes.windows(4).any(|w| w == 2.5f32.to_le_bytes()));
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    // The next one continues after it; deterministic bytes for the same calls.
    let r2 = call(
        "new",
        &data,
        &into,
        json!({"type": "GLOB", "edid": "KC_Second"}),
    )
    .unwrap();
    assert_eq!(r2["data"]["new_forms"][0]["form_id"], json!("01001001"));
    let again = d.join("Again.esp");
    call(
        "new",
        &data,
        &again,
        json!({"type": "glob", "edid": "KC_TestGlobal", "fields": {"FLTV": 2.5}, "masters": ["Source.esm"]}),
    )
    .unwrap();
    call(
        "new",
        &data,
        &again,
        json!({"type": "GLOB", "edid": "KC_Second"}),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(&into).unwrap(),
        std::fs::read(&again).unwrap()
    );
    // Without masters a new plugin has none, and its own prefix is 00.
    let bare = d.join("Bare.esp");
    let r = call(
        "new",
        &data,
        &bare,
        json!({"type": "GLOB", "edid": "KC_Bare"}),
    )
    .unwrap();
    assert_eq!(r["data"]["new_forms"][0]["form_id"], json!("00001000"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_clone_copies_the_winner_with_a_new_formid_and_editor_id() {
    let d = dir("clone-weap");
    let data = data(&d);
    let into = d.join("Clone.esp");
    let r = call(
        "clone",
        &data,
        &into,
        json!({"form": "OldRifle", "edid": "KC_RifleClone", "fields": {"FULL": "Clone Rifle"}}),
    )
    .unwrap();
    let new = &r["data"]["new_forms"][0];
    assert_eq!(new["form_id"], json!("01001000"));
    assert_eq!(new["editor_id"], json!("KC_RifleClone"));
    assert!(
        new["from"].as_str().unwrap().contains("Source.esm:000800"),
        "{new}"
    );
    assert_eq!(
        r["data"]["records"][0]["copied_from"]["plugin"],
        json!("Source.esm")
    );
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    let bytes = std::fs::read(&into).unwrap();
    assert!(bytes.windows(14).any(|w| w == b"KC_RifleClone\0"));
    assert!(bytes.windows(12).any(|w| w == b"Clone Rifle\0"));
    // The source's EditorID is gone; the template FULL was inlined from the table.
    assert!(!bytes.windows(9).any(|w| w == b"OldRifle\0"));
    assert!(bytes.windows(7).any(|w| w == b"Simple\0"));
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    assert!(index.records.iter().any(|r| r.form_id == 0x0100_1000));
    assert!(!index.records.iter().any(|r| r.form_id == 0x0000_0800));
    // inspect finds the clone with the patch appended.
    let i = execute(
        "inspect",
        json!({"form": "KC_RifleClone", "plugins": ["Source.esm", into.display().to_string()],
               "data_dir": data.display().to_string(), "no_cache": true}),
    )
    .unwrap();
    assert_eq!(
        i["data"]["global"],
        json!("Clone.esp:001000"),
        "{:#}",
        i["data"]
    );
    // A clone and a set of its source in one plugin are two records.
    let both = d.join("Both.esp");
    call(
        "set",
        &data,
        &both,
        json!({"form": "OldRifle", "fields": {"FULL": "Renamed"}}),
    )
    .unwrap();
    let r = call(
        "clone",
        &data,
        &both,
        json!({"form": "OldRifle", "edid": "KC_Two"}),
    );
    // The target has the override (no own records), so the clone joins it.
    assert_eq!(
        r.unwrap()["data"]["new_forms"][0]["form_id"],
        json!("01001000")
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn new_and_clone_refusals_are_typed() {
    let d = dir("newform-refusals");
    let data = data(&d);
    let into = d.join("R.esp");
    let cases = [
        (
            "new",
            json!({"type": "REFR", "edid": "KC_A"}),
            "new.not-top-level",
        ),
        (
            "new",
            json!({"type": "CELL", "edid": "KC_A"}),
            "new.not-top-level",
        ),
        (
            "new",
            json!({"type": "ZZZZ", "edid": "KC_A"}),
            "new.untyped",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "OldRifle"}),
            "new.edid-clash",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "bad id"}),
            "new.edid-invalid",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "KC_A", "masters": ["Nope.esm"]}),
            "new.no-master",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "KC_A", "fields": {"EDID": "x"}}),
            "new.edid-field",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "KC_A", "fields": {"FLTV[1]": 1.0}}),
            "new.occurrence",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "KC_A", "fields": {"ZZZZ": 1.0}}),
            "new.unknown-member",
        ),
        (
            "clone",
            json!({"form": "Nope", "edid": "KC_A"}),
            "clone.no-form",
        ),
        (
            "clone",
            json!({"form": "OldRifle", "edid": "OtherRifle"}),
            "clone.edid-clash",
        ),
    ];
    for (command, extra, want) in cases {
        let err = call(command, &data, &into, extra.clone()).unwrap_err();
        assert_eq!(code(&err), want, "{command} {extra}: {err}");
        assert!(!into.exists(), "{command} {extra}");
    }
    call(
        "new",
        &data,
        &into,
        json!({"type": "GLOB", "edid": "KC_One"}),
    )
    .unwrap();
    let err = call(
        "clone",
        &data,
        &into,
        json!({"form": "OldRifle", "edid": "KC_One"}),
    )
    .unwrap_err();
    assert_eq!(code(&err), "clone.edid-clash", "{err}");
    // A .esl target is refused before anything.
    let err = call(
        "new",
        &data,
        &d.join("S.esl"),
        json!({"type": "GLOB", "edid": "KC_B"}),
    )
    .unwrap_err();
    assert!(err.to_string().contains(".esl"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}
