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
    // Twice: the same bytes, and `out` is replaced, not merged.
    build(&rename, &out).unwrap();
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
    let err = build(&s, &out).unwrap_err().to_string();
    assert!(err.contains("lstring.copied-id"), "{err}");
    assert_eq!(std::fs::read(&out).unwrap(), b"previous");
    let _ = std::fs::remove_dir_all(&d);
}
