//! `build` through `execute` (M2.E9): spec parsing, rule expansion, identical bytes with
//! `set`, determinism, conflicts, and validation failures.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitcraft_esm::tes4::FLAG_MASTER;
use kitcraft_esm::{Body, Record, RecordHeader, Signature, Subrecord};
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
    assert!(fails("v.toml", "version = 3\n").contains("spec: version"));
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

// ---- spec v2 (M3.E6) ----

fn sub(sig: &[u8; 4], data: &[u8]) -> Subrecord<'static> {
    Subrecord::new(Signature(*sig), data.to_vec())
}

fn rec(sig: &[u8; 4], form_id: u32, subs: Vec<Subrecord<'static>>) -> Record<'static> {
    Record::new(
        RecordHeader {
            signature: Signature(*sig),
            form_id,
            form_version: 582,
            ..RecordHeader::default()
        },
        Body::Raw(subs),
    )
}

fn zs(text: &str) -> Vec<u8> {
    let mut b = text.as_bytes().to_vec();
    b.push(0);
    b
}

fn rifle(form_id: u32, edid: &str, keywords: &[u32]) -> Record<'static> {
    let mut kwda = Vec::new();
    for k in keywords {
        kwda.extend(k.to_le_bytes());
    }
    rec(
        b"WEAP",
        form_id,
        vec![
            sub(b"EDID", &zs(edid)),
            sub(b"FULL", &zs(edid)),
            sub(b"KSIZ", &(keywords.len() as u32).to_le_bytes()),
            sub(b"KWDA", &kwda),
            sub(b"DESC", &zs("")),
        ],
    )
}

/// `Data\Source.esm` (non-localized master): keywords 900/901, rifles 800..802 tagged with
/// them, globals A00..A02, game settings B00 (float) and B01 (int).
pub(crate) fn data_v2(d: &Path) -> PathBuf {
    let data = d.join("Data");
    std::fs::create_dir_all(&data).unwrap();
    let glob = |id: u32, edid: &str, v: f32| {
        rec(
            b"GLOB",
            id,
            vec![sub(b"EDID", &zs(edid)), sub(b"FLTV", &v.to_le_bytes())],
        )
    };
    let gmst = |id: u32, edid: &str, v: [u8; 4]| {
        rec(b"GMST", id, vec![sub(b"EDID", &zs(edid)), sub(b"DATA", &v)])
    };
    crate::edit::patch_tests::write_plugin(
        &data,
        "Source.esm",
        &[],
        FLAG_MASTER,
        vec![
            rec(b"KYWD", 0x900, vec![sub(b"EDID", &zs("KwBallistic"))]),
            rec(b"KYWD", 0x901, vec![sub(b"EDID", &zs("KwLaser"))]),
            rifle(0x800, "RifleA", &[0x900]),
            rifle(0x801, "RifleB", &[0x900, 0x901]),
            rifle(0x802, "LaserC", &[0x901]),
            glob(0xA00, "GlobA", 1.0),
            glob(0xA01, "GlobB", 10.0),
            glob(0xA02, "TestGlob", 5.0),
            gmst(0xB00, "fDamageMult", 2.0f32.to_le_bytes()),
            gmst(0xB01, "iMaxCount", 7i32.to_le_bytes()),
        ],
    );
    data
}

pub(crate) fn spec_v2(d: &Path, name: &str, data: &Path, body: &str) -> PathBuf {
    let text = format!(
        "version = 2\n\n[plugin]\nlocalized = false\nstyle = \"kit\"\n\n[load_order]\nplugins = [\"Source.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n{body}",
        data.display()
    );
    let path = d.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

pub(crate) fn plan(spec: &Path, out: &Path) -> Result<Value, crate::EngineError> {
    execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": out.display().to_string(), "plan": true}),
    )
}

/// `(editor_id, path, before, after)` of every change in a plan.
fn changes(result: &Value) -> Vec<(String, String, Value, Value)> {
    result["data"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["editor_id"].as_str().unwrap_or_default().to_string(),
                c["path"].as_str().unwrap().to_string(),
                c["before"].clone(),
                c["after"].clone(),
            )
        })
        .collect()
}

const OPERATORS_SPEC: &str =
    "[class.globals]\ntype = \"GLOB\"\nwhere = 'EDID ^= \"Glob\"'\nbrief = \"B1\"\n\n\
     [class.settings]\ntype = \"GMST\"\n\n\
     [[rule]]\nclass = \"globals\"\nbrief = \"B1\"\nfields = { FLTV = { clamp = [2.0, 8.0] } }\n\n\
     [[rule]]\nclass = \"settings\"\nfields = { DATA = { mul = 1.5 } }\n\n\
     [[set]]\nform = \"TestGlob\"\nfields = { FLTV = { add = 0.5 } }\n";

/// M3.E6: named classes, rules with operators, a set after the rules; `plan` lists every
/// change with before and after and writes nothing; two builds are byte-identical.
#[test]
fn v2_classes_operators_plan_and_identical_builds() {
    let d = dir("build-v2");
    let data = data_v2(&d);
    let s = spec_v2(&d, "ops.toml", &data, OPERATORS_SPEC);
    let out = d.join("Ops.esp");
    let result = plan(&s, &out).unwrap();
    assert!(!out.exists(), "a plan writes nothing");
    assert_eq!(result["data"]["plan"], json!(true));
    assert_eq!(result["data"]["passed"], json!(true), "{result:#}");
    assert_eq!(
        changes(&result),
        [
            ("GlobA".into(), "FLTV".into(), json!(1.0), json!(2.0)),
            ("GlobB".into(), "FLTV".into(), json!(10.0), json!(8.0)),
            ("fDamageMult".into(), "DATA".into(), json!(2.0), json!(3.0)),
            // 7 * 1.5 = 10.5, rounded half away from zero.
            ("iMaxCount".into(), "DATA".into(), json!(7), json!(11)),
            ("TestGlob".into(), "FLTV".into(), json!(5.0), json!(5.5)),
        ],
        "{result:#}"
    );
    assert_eq!(result["data"]["changed_fields"], json!(5));
    let classes = result["data"]["classes"].as_array().unwrap();
    assert_eq!(classes[0]["class"], json!("globals"));
    assert_eq!(classes[0]["matched"], json!(2));
    assert_eq!(classes[0]["brief"], json!("B1"));
    assert_eq!(classes[1]["matched"], json!(2));
    let rules = result["data"]["rules"].as_array().unwrap();
    assert_eq!(rules[0]["class"], json!("globals"));
    assert_eq!(rules[0]["fields_changed"], json!(2));
    assert_eq!(rules[1]["fields_written"], json!(2));
    assert!(result["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().starts_with("plan: nothing was written")));
    // The build writes what the plan listed, and twice gives the same bytes.
    let built = build(&s, &out).unwrap();
    assert_eq!(built["data"]["changed_fields"], json!(5));
    let again = d.join("Ops2.esp");
    build(&s, &again).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), std::fs::read(&again).unwrap());
    let _ = std::fs::remove_dir_all(&d);
}

/// M3.E6: `via` follows typed FormID members (weapon -> keyword); a record reached by
/// several rules and classes is written once when the writes agree, and two different
/// writes to one field are an error naming both keys.
#[test]
fn v2_via_writes_each_reached_record_once() {
    let d = dir("build-v2-via");
    let data = data_v2(&d);
    let body = |second: &str| {
        format!(
            "[class.ballistic]\ntype = \"WEAP\"\nwhere = 'KWDA == Source.esm:000900'\n\n\
             [class.lasers]\ntype = \"WEAP\"\nwhere = 'KWDA == Source.esm:000901'\n\n\
             [[rule]]\nclass = \"ballistic\"\nvia = [\"KWDA\"]\nfields = {{ EDID = \"KwTagged\" }}\n\n\
             [[rule]]\nclass = \"lasers\"\nvia = [\"KWDA\"]\nfields = {{ EDID = \"{second}\" }}\n"
        )
    };
    let s = spec_v2(&d, "via.toml", &data, &body("KwTagged"));
    let result = plan(&s, &d.join("Via.esp")).unwrap();
    // RifleA and RifleB reach 900 and 901; RifleB and LaserC reach them again.
    let got: Vec<(&str, &Value)> = result["data"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["form"].as_str().unwrap(), &c["before"]))
        .collect();
    assert_eq!(
        got,
        [
            ("Source.esm:000900", &json!("KwBallistic")),
            ("Source.esm:000901", &json!("KwLaser"))
        ],
        "{result:#}"
    );
    assert_eq!(
        result["data"]["changes"][0]["by"],
        json!(["rule[0]", "rule[1]"])
    );
    let rule0 = &result["data"]["rules"][0];
    assert_eq!(rule0["matched"], json!(2));
    assert_eq!(rule0["via"][0]["reached"], json!(2));
    assert_eq!(rule0["records"], json!(2));
    // A different value for the same field: an error naming both keys.
    let s = spec_v2(&d, "conflict.toml", &data, &body("KwOther"));
    let err = plan(&s, &d.join("Via.esp")).unwrap_err().to_string();
    assert!(
        err.contains("spec: rule[1].fields.EDID")
            && err.contains("already written by rule[0].fields.EDID with a different value"),
        "{err}"
    );
    // `via` through a member that is not a FormID.
    let s = spec_v2(
        &d,
        "notform.toml",
        &data,
        "[class.all]\ntype = \"WEAP\"\n[[rule]]\nclass = \"all\"\nvia = [\"EDID\"]\nfields = { EDID = \"x\" }\n",
    );
    let err = plan(&s, &d.join("Via.esp")).unwrap_err().to_string();
    assert!(
        err.contains("spec: rule[0].via[0]") && err.contains("not a FormID"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// M3.E6 errors: an unknown member, an operator on a non-number, a rule matching nothing
/// (a warning, an error under `strict`), and the v1 rule that any field written twice is an
/// error even when the writes agree.
#[test]
fn v2_errors_and_warnings() {
    let d = dir("build-v2-errors");
    let data = data_v2(&d);
    let out = d.join("E.esp");
    let fails = |name: &str, body: &str| {
        let s = spec_v2(&d, name, &data, body);
        plan(&s, &out).unwrap_err().to_string()
    };
    let globals = "[class.g]\ntype = \"GLOB\"\n";
    let err = fails(
        "member.toml",
        &format!("{globals}[[rule]]\nclass = \"g\"\nfields = {{ \"FLTV.Nope\" = 1 }}\n"),
    );
    assert!(
        err.contains("spec: rule[0].fields.FLTV.Nope: unknown member")
            && err.contains("record type GLOB"),
        "{err}"
    );
    let err = fails(
        "sub.toml",
        &format!("{globals}[[rule]]\nclass = \"g\"\nfields = {{ ZZZZ = \"hex:00\" }}\n"),
    );
    assert!(err.contains("no subrecord ZZZZ"), "{err}");
    let err = fails(
        "op.toml",
        &format!("{globals}[[rule]]\nclass = \"g\"\nfields = {{ EDID = {{ max = 2 }} }}\n"),
    );
    assert!(err.contains("apply to typed numbers"), "{err}");
    let nothing = "[class.none]\ntype = \"GLOB\"\nwhere = 'EDID == \"Missing\"'\n\
                   [[rule]]\nclass = \"none\"\nfields = { FLTV = 1 }\n\
                   [[set]]\nform = \"GlobA\"\nfields = { FLTV = 3 }\n";
    let s = spec_v2(&d, "nothing.toml", &data, nothing);
    let result = plan(&s, &out).unwrap();
    assert_eq!(
        result["data"]["warnings"],
        json!(["rule[0] matches nothing (class none)"])
    );
    let strict = spec_v2(&d, "strict.toml", &data, nothing);
    let text = std::fs::read_to_string(&strict).unwrap();
    std::fs::write(
        &strict,
        text.replacen("version = 2\n", "version = 2\nstrict = true\n", 1),
    )
    .unwrap();
    let err = plan(&strict, &out).unwrap_err().to_string();
    assert!(
        err.contains("spec: rule[0]: rule[0] matches nothing") && err.contains("strict"),
        "{err}"
    );
    // Without a plan, a spec that writes nothing is refused.
    let s = spec_v2(
        &d,
        "empty.toml",
        &data,
        "[class.none]\ntype = \"GLOB\"\nwhere = 'EDID == \"Missing\"'\n[[rule]]\nclass = \"none\"\nfields = { FLTV = 1 }\n",
    );
    let err = build(&s, &out).unwrap_err().to_string();
    assert!(err.contains("nothing to write"), "{err}");
    // A v2 rule and set agreeing on a write: one write. In v1 the same is an error.
    let agree = "[class.a]\ntype = \"GLOB\"\nwhere = 'EDID == \"GlobA\"'\n\
                 [[rule]]\nclass = \"a\"\nfields = { FLTV = 3 }\n\
                 [[set]]\nform = \"GlobA\"\nfields = { FLTV = 3 }\n";
    let s = spec_v2(&d, "agree.toml", &data, agree);
    let result = plan(&s, &out).unwrap();
    assert_eq!(
        result["data"]["changes"][0]["by"],
        json!(["rule[0]", "set[0]"])
    );
    let v1 = d.join("agree-v1.toml");
    std::fs::write(
        &v1,
        format!(
            "version = 1\n[load_order]\nplugins = [\"Source.esm\"]\ndata_dir = '{}'\nno_cache = true\n\
             [[rule]]\ntype = \"GLOB\"\nwhere = 'EDID == \"GlobA\"'\nfields = {{ FLTV = 3 }}\n\
             [[set]]\nform = \"GlobA\"\nfields = {{ FLTV = 3 }}\n",
            data.display()
        ),
    )
    .unwrap();
    let err = build(&v1, &out).unwrap_err().to_string();
    assert!(
        err.contains("already written by rule[0].fields.FLTV"),
        "{err}"
    );
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// M3.E6: a write to a `reference`-only member is refused without `allow_reference` and
/// named in `limitations` with it. The builtin schema has no `reference` member until lane
/// B round 3 lands, so this runs the gate `build` uses on a synthetic schema.
#[test]
fn reference_only_members_need_allow_reference() {
    let schema = crate::build::members::reference_schema();
    let edits = |path: &str| {
        vec![crate::edit::patch::Edit {
            path: crate::edit::fields::parse_path(path).unwrap(),
            op: crate::edit::fields::Op::Mul(0.8),
        }]
    };
    let mut cache = std::collections::HashMap::new();
    let mut limitations = Vec::new();
    let mut gate = |at: &str, path: &str, allow: bool, limitations: &mut Vec<String>| {
        super::build::gate(
            &schema,
            *b"TEST",
            at,
            &edits(path),
            allow,
            true,
            &mut cache,
            limitations,
        )
    };
    let err = gate("rule[0]", "DNAM.Speed", false, &mut limitations)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("spec: rule[0].fields.DNAM.Speed: TEST.Data.Speed is a reference-only member")
            && err.contains("allow_reference = true"),
        "{err}"
    );
    gate("rule[0]", "DNAM.Speed", true, &mut limitations).unwrap();
    assert_eq!(limitations.len(), 1);
    assert!(limitations[0].contains("TEST.Data.Speed, a reference-only member"));
    // A corpus member next to it needs nothing.
    gate("rule[1]", "DNAM.Damage", false, &mut limitations).unwrap();
    assert_eq!(limitations.len(), 1);
}
