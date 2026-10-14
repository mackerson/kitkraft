//! `build` through `execute` (M2.E9): spec parsing, rule expansion, identical bytes with
//! `set`, determinism, conflicts, and validation failures.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::set_tests::data;
use crate::edit::patch_tests::dir;
use crate::execute;

fn spec(d: &Path, name: &str, data: &Path, body: &str) -> PathBuf {
    let text = format!(
        "version = 1\n\n[plugin]\nlocalized = false\n\n[load_order]\nplugins = [\"Source.esm\", \"Override.esp\"]\ndata_dir = '{}'\nno_cache = true\n\n{body}",
        data.display()
    );
    let path = d.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn build(spec: &Path, out: &Path) -> Result<Value, crate::EngineError> {
    execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": out.display().to_string()}),
    )
}

fn rebuild(spec: &Path, out: &Path) -> Result<Value, crate::EngineError> {
    execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": out.display().to_string(), "overwrite": true}),
    )
}

const RENAME: &str =
    "[[set]]\nform = \"Source.esm:000800\"\nfields = { FULL = \"Old Soviet Marksman Rifle\" }\n";

#[test]
fn the_rename_spec_builds_the_bytes_set_writes() {
    let d = dir("build-rename");
    let data = data(&d);
    let by_set = d.join("Set.esm");
    execute(
        "set",
        json!({
            "form": "Source.esm:000800", "fields": {"FULL": "Old Soviet Marksman Rifle"},
            "into": by_set.display().to_string(), "plugins": ["Source.esm", "Override.esp"],
            "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap();
    let rename = spec(&d, "rename.toml", &data, RENAME);
    let out = d.join("out").join("Set.esm");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let result = build(&rename, &out).unwrap();
    assert_eq!(result["data"]["validate"]["passed"], json!(true));
    assert_eq!(result["data"]["passed"], json!(true));
    let built = std::fs::read(&out).unwrap();
    assert_eq!(built, std::fs::read(&by_set).unwrap());
    // Twice: refused without overwrite; with it, the same bytes (`out` is replaced, not
    // merged).
    let err = build(&rename, &out).unwrap_err().to_string();
    assert!(err.contains("pass overwrite"), "{err}");
    rebuild(&rename, &out).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), built);
    assert!(!out.parent().unwrap().read_dir().unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".kitcraft-build")));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn rules_expand_over_query_matches() {
    let d = dir("build-rule");
    let data = data(&d);
    let s = spec(
        &d,
        "rule.toml",
        &data,
        "[[rule]]\ntype = \"WEAP\"\nwhere = 'EDID ~ \"Rifle\"'\nfields = { FULL = \"Renamed\" }\n",
    );
    let out = d.join("Rule.esp");
    let result = build(&s, &out).unwrap();
    let data_out = &result["data"];
    assert_eq!(data_out["rules"][0]["matched"], json!(2));
    let records = data_out["records"].as_array().unwrap();
    let names: Vec<&str> = records
        .iter()
        .map(|r| r["editor_id"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["OldRifle", "OtherRifle"]);
    assert_eq!(records[1]["copied_from"]["plugin"], json!("Override.esp"));
    assert_eq!(data_out["masters"], json!(["Source.esm", "Override.esp"]));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn spec_errors_name_the_key() {
    let d = dir("build-errors");
    let data = data(&d);
    let out = d.join("E.esp");
    let fails = |name: &str, text: &str| {
        let path = d.join(name);
        std::fs::write(&path, text).unwrap();
        build(&path, &out).unwrap_err().to_string()
    };
    assert!(fails("v.toml", "version = 2\n").contains("spec: version"));
    assert!(fails("n.toml", "[[set]]\nform = \"x\"\n").contains("spec: version"));
    assert!(fails("k.toml", "version = 1\nfoo = 1\n").contains("spec: foo"));
    assert!(fails("l.toml", "version = 1\n[plugin]\nlocalized = true\n")
        .contains("spec: plugin.localized"));
    assert!(fails("t.toml", "version = 1\n[[set]]\nform = \n").contains("spec: (toml)"));
    let bad_field = spec(
        &d,
        "f.toml",
        &data,
        "[[set]]\nform = \"OldRifle\"\nfields = { full = \"x\" }\n",
    );
    let err = build(&bad_field, &out).unwrap_err().to_string();
    assert!(err.contains("spec: set[0].fields.full"), "{err}");
    let bad_where = spec(
        &d,
        "w.toml",
        &data,
        "[[rule]]\ntype = \"WEAP\"\nwhere = 'EDID ~~ \"x\"'\nfields = { FULL = \"x\" }\n",
    );
    let err = build(&bad_where, &out).unwrap_err().to_string();
    assert!(err.contains("spec: rule[0].where"), "{err}");
    let missing = spec(&d, "m.toml", &data, "[[set]]\nfields = { FULL = \"x\" }\n");
    let err = build(&missing, &out).unwrap_err().to_string();
    assert!(err.contains("spec: set[0].form"), "{err}");
    // A field written twice: by a rule and by a set.
    let twice = spec(
        &d,
        "c.toml",
        &data,
        &format!("[[rule]]\ntype = \"WEAP\"\nwhere = 'EDID ~ \"Old\"'\nfields = {{ FULL = \"a\" }}\n{RENAME}"),
    );
    let err = build(&twice, &out).unwrap_err().to_string();
    assert!(
        err.contains("spec: set[0].fields.FULL") && err.contains("rule[0].fields.FULL"),
        "{err}"
    );
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_build_that_fails_validate_writes_nothing() {
    let d = dir("build-invalid");
    let data = data(&d);
    let out = d.join("Bad.esp");
    std::fs::write(&out, b"previous").unwrap();
    // A raw 4-byte FULL on an override of a localized master: lstring.copied-id.
    let s = spec(
        &d,
        "bad.toml",
        &data,
        "[[set]]\nform = \"OldRifle\"\nfields = { FULL = \"hex:69E40200\" }\n",
    );
    let err = rebuild(&s, &out).unwrap_err().to_string();
    assert!(err.contains("lstring.copied-id"), "{err}");
    assert_eq!(std::fs::read(&out).unwrap(), b"previous");
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 1: `out` is guarded like any patch target before anything
/// is staged: never a load-order plugin (the guard used to see only the staged path), never
/// inside the game Data, user Data or MO2 directories; an existing `out` needs `overwrite`.
#[test]
fn out_is_guarded_before_anything_is_written() {
    let d = dir("build-guard");
    let data = data(&d);
    let rename = spec(&d, "rename.toml", &data, RENAME);
    let source = data.join("Source.esm");
    let before = std::fs::read(&source).unwrap();
    for refused in [build(&rename, &source), rebuild(&rename, &source)] {
        let err = refused.unwrap_err().to_string();
        assert!(err.contains("plugin of the load order"), "{err}");
    }
    assert_eq!(std::fs::read(&source).unwrap(), before);
    let game = d.join("Game").join("Data");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join("Starfield.esm"), b"x").unwrap();
    let err = rebuild(&rename, &game.join("P.esp"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("Data directory"), "{err}");
    let mo2 = d.join("MO2");
    std::fs::create_dir_all(mo2.join("overwrite")).unwrap();
    std::fs::write(mo2.join("ModOrganizer.ini"), b"x").unwrap();
    let err = rebuild(&rename, &mo2.join("overwrite").join("P.esp"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("Mod Organizer"), "{err}");
    assert!(!game.join("P.esp").exists() && !mo2.join("overwrite").join("P.esp").exists());
    let existing = d.join("Existing.esp");
    std::fs::write(&existing, b"mine").unwrap();
    let err = build(&rename, &existing).unwrap_err().to_string();
    assert!(err.contains("pass overwrite"), "{err}");
    assert_eq!(std::fs::read(&existing).unwrap(), b"mine");
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 8: a whole subrecord and a member of it, from two
/// entries, conflict; another occurrence does not.
#[test]
fn whole_and_member_writes_of_one_subrecord_conflict() {
    let d = dir("build-overlap");
    let data = data(&d);
    let out = d.join("O.esp");
    let s = spec(
        &d,
        "o.toml",
        &data,
        "[[rule]]\ntype = \"WEAP\"\nwhere = 'EDID ~ \"Old\"'\nfields = { MODL = \"hex:00\" }\n[[set]]\nform = \"OldRifle\"\nfields = { \"MODL.Path\" = \"x\" }\n",
    );
    let err = build(&s, &out).unwrap_err().to_string();
    assert!(
        err.contains("spec: set[0].fields.MODL.Path") && err.contains("rule[0].fields.MODL"),
        "{err}"
    );
    let s = spec(
        &d,
        "p.toml",
        &data,
        "[[rule]]\ntype = \"WEAP\"\nwhere = 'EDID ~ \"Old\"'\nfields = { FULL = \"a\" }\n[[set]]\nform = \"OldRifle\"\nfields = { \"FULL[1]\" = \"b\" }\n",
    );
    build(&s, &out).unwrap();
    assert!(out.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 10: TOML `inf`/`nan` (and `1e400` text) are range errors.
#[test]
fn non_finite_values_are_range_errors() {
    let d = dir("build-inf");
    let data = data(&d);
    let out = d.join("I.esp");
    for (fields, key) in [
        (
            "{ \"DNAM.Speed\" = { mul = inf } }",
            "set[0].fields.DNAM.Speed.mul",
        ),
        ("{ FULL = nan }", "set[0].fields.FULL"),
        ("{ FULL = -inf }", "set[0].fields.FULL"),
    ] {
        let s = spec(
            &d,
            "i.toml",
            &data,
            &format!("[[set]]\nform = \"OldRifle\"\nfields = {fields}\n"),
        );
        let err = build(&s, &out).unwrap_err().to_string();
        assert!(
            err.contains(&format!("spec: {key}: ")) && err.contains("not a finite number (range)"),
            "{err}"
        );
    }
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 11: builds stage in the OS temp dir (nothing is created
/// next to `out`), and stale staging directories, including the old `.kitcraft-build-<pid>`
/// layout next to `out`, are removed.
#[test]
fn stale_staging_directories_are_removed() {
    let d = dir("build-stale");
    let old = d.join(".kitcraft-build-4242");
    let other = d.join("keep-me");
    std::fs::create_dir_all(old.join("x")).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    super::set::remove_stale(&d, ".kitcraft-build-", std::time::Duration::ZERO);
    assert!(!old.exists() && other.exists());
    // A fresh one (younger than the age) is kept.
    std::fs::create_dir_all(&old).unwrap();
    super::set::remove_stale(&d, ".kitcraft-build-", std::time::Duration::from_secs(3600));
    assert!(old.exists());
    let _ = std::fs::remove_dir_all(&d);
}
