//! `overhaul.scaffold` through `execute` (M3.E7) on the synthetic load order of the
//! `build` spec v2 tests.

use std::path::Path;

use serde_json::{json, Value};

use super::build_tests::{data_v2, plan};
use crate::edit::patch_tests::dir;
use crate::execute;

fn scaffold(data: &Path, extra: Value) -> Result<Value, crate::EngineError> {
    let mut args = json!({
        "plugins": ["Source.esm"], "data_dir": data.display().to_string(), "no_cache": true,
    });
    if let (Some(a), Some(e)) = (args.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            a.insert(k.clone(), v.clone());
        }
    }
    execute("overhaul.scaffold", args)
}

#[test]
fn a_scaffold_groups_by_keyword_and_plans_to_nothing() {
    let d = dir("overhaul-kwda");
    let data = data_v2(&d);
    let out = d.join("weapons.toml");
    let result = scaffold(
        &data,
        json!({"type": "WEAP", "group_by": "KWDA", "out": out.display().to_string()}),
    )
    .unwrap();
    let r = &result["data"];
    assert_eq!(r["records"], json!(3));
    let classes = r["classes"].as_array().unwrap();
    let names: Vec<(&str, u64)> = classes
        .iter()
        .map(|c| (c["class"].as_str().unwrap(), c["matched"].as_u64().unwrap()))
        .collect();
    assert_eq!(names, [("kwballistic", 2), ("kwlaser", 2)], "{r:#}");
    assert_eq!(classes[0]["where"], json!("KWDA == \"Source.esm:000900\""));
    assert_eq!(
        classes[0]["label"],
        json!("KWDA == Source.esm:000900 (KwBallistic)")
    );
    // WEAP's numeric members are typed since lane B round 3 (M3.B1), so KSIZ is no longer
    // the first statistic.
    let ksiz = classes[0]["stats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["member"] == "KSIZ")
        .unwrap();
    assert_eq!(ksiz["member"], json!("KSIZ"));
    assert_eq!(
        (&ksiz["min"], &ksiz["median"], &ksiz["max"]),
        (&json!(1.0), &json!(1.5), &json!(2.0))
    );
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(
        text.contains(
            "[class.kwballistic]\ntype = \"WEAP\"\nwhere = 'KWDA == \"Source.esm:000900\"'\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("# KSIZ: min 1, median 1.5, max 2 (2 record(s))"),
        "{text}"
    );
    assert!(
        text.contains("# [[rule]]\n# class = \"kwballistic\"\n"),
        "{text}"
    );
    assert!(
        !text.lines().any(|l| l == "[[rule]]"),
        "every rule is commented out"
    );
    // `build --plan` on the scaffold: 0 changes, and each class matches what it counted.
    let p = plan(&out, &d.join("Weapons.esp")).unwrap();
    assert_eq!(p["data"]["changes"], json!([]), "{p:#}");
    assert_eq!(p["data"]["classes"][0]["matched"], json!(2));
    assert_eq!(p["data"]["classes"][1]["matched"], json!(2));
    // Deterministic; an existing `out` needs `overwrite`; without `out`, the same text.
    let err = scaffold(
        &data,
        json!({"type": "WEAP", "group_by": "KWDA", "out": out.display().to_string()}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("pass overwrite"), "{err}");
    scaffold(
        &data,
        json!({"type": "WEAP", "group_by": "KWDA", "out": out.display().to_string(), "overwrite": true}),
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), text);
    let inline = scaffold(&data, json!({"type": "weap", "group_by": "KWDA"})).unwrap();
    assert_eq!(inline["data"]["spec_text"], json!(text));
    assert_eq!(inline["data"]["written"], json!(false));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_scaffold_without_groups_has_one_class_with_statistics() {
    let d = dir("overhaul-glob");
    let data = data_v2(&d);
    let result = scaffold(&data, json!({"type": "GLOB", "where": "EDID ^= \"Glob\""})).unwrap();
    let r = &result["data"];
    let classes = r["classes"].as_array().unwrap();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0]["class"], json!("glob"));
    assert_eq!(classes[0]["where"], json!("EDID ^= \"Glob\""));
    let fltv = &classes[0]["stats"][0];
    assert_eq!(
        (&fltv["min"], &fltv["median"], &fltv["max"]),
        (&json!(1.0), &json!(5.5), &json!(10.0))
    );
    let text = r["spec_text"].as_str().unwrap();
    assert!(
        text.contains("# fields = { \"FLTV\" = { mul = 1.0 } }"),
        "{text}"
    );
    // The plan of an inline scaffold: write it, then plan it.
    let spec = d.join("glob.toml");
    std::fs::write(&spec, text).unwrap();
    let p = plan(&spec, &d.join("G.esp")).unwrap();
    assert_eq!(p["data"]["classes"][0]["matched"], json!(2));
    assert_eq!(p["data"]["changed_fields"], json!(0));
    // Group by plugin, and a class limit: the rest is listed, not dropped silently.
    let by_plugin = scaffold(&data, json!({"type": "GMST", "group_by": "@plugin"})).unwrap();
    assert_eq!(
        by_plugin["data"]["classes"][0]["where"],
        json!("@plugin == \"Source.esm\"")
    );
    let limited = scaffold(
        &data,
        json!({"type": "GLOB", "group_by": "EDID", "max_groups": 1}),
    )
    .unwrap();
    assert_eq!(limited["data"]["classes"].as_array().unwrap().len(), 1);
    assert_eq!(limited["data"]["omitted"].as_array().unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn scaffold_argument_errors() {
    let d = dir("overhaul-errors");
    let data = data_v2(&d);
    let err = scaffold(
        &data,
        json!({"type": "WEAP", "out": d.join("x.esp").display().to_string()}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains(".toml"), "{err}");
    let err = scaffold(&data, json!({"type": "WEAP", "group_by": "KWDA =="}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("group_by"), "{err}");
    let err = execute("overhaul", json!({})).unwrap_err();
    assert_eq!(err.kind(), "invalid_args");
    let _ = std::fs::remove_dir_all(&d);
}
