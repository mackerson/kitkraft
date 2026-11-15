//! Condition (`CTDA`) authoring through the write commands (KK-268,
//! `docs/design/conditions.md`).
//!
//! The fixture is the synthetic load order of the `set` tests (`Source.esm` with two
//! weapons) plus `Conds.esp`, written with `new`: a global (`KC_CondGlobal`,
//! `Conds.esp:001000`), a keyword (`KC_CondKeyword`, `:001001`) and a constructible object
//! without conditions (`KC_Recipe`, `:001002`).
//!
//! - `a_cloned_cobj_gets_conditions_that_read_back`: `clone` of the recipe adds two
//!   conditions member by member: a global compare (`UseGlobal`, the comparison value a
//!   `GLOB`, parameter 1 a global: `GetGlobalValue`; operator and flags written as the
//!   type byte's parts) and a FormID parameter (`GetIsID` of
//!   the rifle, run on the linked reference with a linked keyword), plus the two string
//!   parameters. Every typed member reads back as written, nothing is raw, the record
//!   re-encodes byte for byte and `validate` passes with no finding.
//! - `set_appends_a_condition_to_an_override`: `set` on the clone adds a third condition
//!   after the two it has and changes the second one's operator (a part of the type byte,
//!   written in place: its `Or` flag stays); the first is unchanged, and a gap is refused.
//! - `a_dangling_condition_link_is_reported`: the clone's conditions name a form the
//!   masters lack in the comparison value, parameter 1, the reference and parameter 3;
//!   `validate`'s `formid.dangling` names each and `refs` lists each as missing.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_doc::typed::{self, Decoded, Placement};
use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::Record as Generated;
use kitkraft_esm::{Plugin, Record};
use serde_json::{json, Value};

use super::set_tests::data;
use crate::edit::patch_tests::dir;
use crate::execute;

/// A form the masters lack.
const MISSING: &str = "Source.esm:000FFF";
/// The fixture's global, keyword and recipe, and the rifle (`Source.esm:000800`).
const GLOBAL: &str = "Conds.esp:001000";
const KEYWORD: &str = "Conds.esp:001001";
const RIFLE: &str = "Source.esm:000800";

/// Runs `command` with the fixture's load order (`Source.esm`, `Conds.esp`, then
/// `extra`) and Data dir.
fn call(command: &str, data: &Path, extra: &[&str], mut args: Value) -> Value {
    let mut plugins = vec!["Source.esm".to_string(), "Conds.esp".to_string()];
    plugins.extend(extra.iter().map(|p| p.to_string()));
    args["plugins"] = json!(plugins);
    args["data_dir"] = json!(data.display().to_string());
    args["no_cache"] = json!(true);
    execute(command, args).unwrap_or_else(|e| panic!("{command}: {e}"))
}

/// The fixture (module docs) in a fresh dir; returns the dir and its Data dir.
fn fixture(name: &str) -> (PathBuf, PathBuf) {
    let d = dir(&format!("conditions-{name}"));
    let data = data(&d);
    let conds = data.join("Conds.esp");
    for (ty, edid, fields) in [
        ("GLOB", "KC_CondGlobal", json!({"FLTV": 2.0})),
        ("KYWD", "KC_CondKeyword", json!({"TNAM": 0})),
        ("COBJ", "KC_Recipe", json!({"CNAM": RIFLE})),
    ] {
        execute(
            "new",
            json!({
                "type": ty, "edid": edid, "fields": fields, "masters": ["Source.esm"],
                "into": conds.display().to_string(),
                "plugins": ["Source.esm"], "data_dir": data.display().to_string(),
                "no_cache": true,
            }),
        )
        .unwrap_or_else(|e| panic!("new {ty}: {e}"));
    }
    (d, data)
}

/// The record with EditorID `edid` in `plugin`.
fn record_in(bytes: &[u8], edid: &str) -> Record<'static> {
    let plugin = Plugin::parse(bytes).unwrap();
    fn find(entries: &[kitkraft_esm::Entry<'_>], edid: &str) -> Option<Record<'static>> {
        entries.iter().find_map(|e| match e {
            kitkraft_esm::Entry::Record(r) => {
                let subs = r.subrecords()?;
                let first = subs.iter().find(|s| s.signature().0 == *b"EDID")?;
                (first.data().strip_suffix(&[0]) == Some(edid.as_bytes()))
                    .then(|| r.clone().into_owned())
            }
            kitkraft_esm::Entry::Group(g) => find(g.children(), edid),
        })
    }
    find(plugin.entries(), edid).unwrap_or_else(|| panic!("no record {edid}"))
}

/// Every `CTDA`, `CIS1` and `CIS2` of `record`, decoded, in order: (signature, value).
fn conditions(record: &Record<'_>) -> Vec<([u8; 4], Decoded)> {
    let schema = kitkraft_schema::builtin().unwrap();
    let subs = record.subrecords().unwrap();
    let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
    let placed = typed::placements(schema, record.header.signature.0, &sigs).unwrap();
    let ctx = typed::Ctx {
        form_version: record.header.form_version,
        localized: false,
        edid_first: Some(b'K'),
    };
    subs.iter()
        .zip(&placed)
        .filter(|(s, _)| matches!(&s.signature().0, b"CTDA" | b"CIS1" | b"CIS2"))
        .map(|(s, p)| {
            let Placement::Member { def, path } = p else {
                panic!("{} not placed: {p:?}", s.signature());
            };
            assert!(path.starts_with("Conditions."), "{path}");
            (
                s.signature().0,
                typed::decode(&def.ty, s.data(), &ctx).unwrap(),
            )
        })
        .collect()
}

/// The value at the member path `at` (`Parameter1.Global`) of a decoded `CTDA`.
fn member(ctda: &Decoded, at: &str) -> Decoded {
    let steps: Vec<String> = at.split('.').map(str::to_lowercase).collect();
    let got = typed::select(ctda, &steps, at).unwrap();
    match &got[..] {
        [one] => one.clone(),
        other => panic!("{at}: {other:?}"),
    }
}

/// The record re-encodes byte for byte through the generated decoder, nothing raw.
fn reencodes(record: &Record<'_>) {
    let subs = record.subrecords().unwrap();
    let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
    let generated = Generated::decode(
        record.header.signature.0,
        &raw,
        Ctx {
            form_version: record.header.form_version,
            localized: false,
        },
    )
    .unwrap();
    assert!(generated.extra().is_empty(), "{:?}", generated.extra());
    let encoded = generated.encode();
    assert_eq!(encoded.len(), raw.len());
    for (i, (a, b)) in raw.iter().zip(&encoded).enumerate() {
        assert!(
            a.signature == b.signature && a.data == b.data.as_slice(),
            "subrecord {i} re-encodes differently"
        );
    }
}

/// `validate` on `plugin`: passed, no finding.
fn clean(plugin: &Path, data: &Path) {
    let v = execute(
        "validate",
        json!({"plugin": plugin.display().to_string(), "data_dir": data.display().to_string()}),
    )
    .unwrap();
    assert_eq!(v["data"]["findings"], json!([]), "{:#}", v["data"]);
}

/// The two conditions and string parameters the clone test writes (sorted keys reach the
/// engine: `CTDA.ComparisonValue.Global` before `CTDA.Type`, `CTDA[1].Parameter3` before
/// `CTDA[1].RunOn`, so the writer must order deciders first).
fn two_conditions() -> Value {
    json!({
        // GetGlobalValue(KC_CondGlobal) >= KC_CondGlobal: the type byte's operator and
        // flags parts (KK-269), 3 << 5 | 0x04.
        "CTDA.Type.Operator": "GreaterThanOrEqualTo",
        "CTDA.Type.Flags": "UseGlobal",
        "CTDA.ComparisonValue.Global": GLOBAL,
        "CTDA.Function": 74,
        "CTDA.Parameter1.Global": GLOBAL,
        "CTDA.RunOn": "Subject",
        // GetIsID(rifle) == 1.0 on the linked reference by keyword, OR'ed with the next.
        "CTDA[1].Type": 0x01,
        "CTDA[1].ComparisonValue.Float": 1.0,
        "CTDA[1].Function": 72,
        "CTDA[1].Parameter1.InventoryObject": RIFLE,
        "CTDA[1].RunOn": "LinkedReference",
        "CTDA[1].Parameter3.LinkedKeyword": KEYWORD,
        "CIS1": "StringOne",
        "CIS2": "StringTwo",
    })
}

/// `clone` of `KC_Recipe` into `New.esp` with `fields`; returns the plugin path.
fn clone_recipe(d: &Path, data: &Path, edid: &str, fields: Value) -> PathBuf {
    let into = d.join("New.esp");
    let r = call(
        "clone",
        data,
        &[],
        json!({"form": "KC_Recipe", "edid": edid, "fields": fields,
               "into": into.display().to_string()}),
    );
    assert_eq!(r["data"]["validate"]["errors"], json!(0), "{:#}", r["data"]);
    into
}

#[test]
fn a_cloned_cobj_gets_conditions_that_read_back() {
    let (d, data) = fixture("clone");
    let into = clone_recipe(&d, &data, "KC_RecipeConditioned", two_conditions());
    let bytes = std::fs::read(&into).unwrap();
    let record = record_in(&bytes, "KC_RecipeConditioned");
    reencodes(&record);
    let conds = conditions(&record);
    let sigs: Vec<&[u8; 4]> = conds.iter().map(|(s, _)| s).collect();
    // The string parameters land after the last condition (their slot follows CTDA's).
    assert_eq!(sigs, [b"CTDA", b"CTDA", b"CIS1", b"CIS2"]);
    // New.esp's masters: Source.esm (0), Conds.esp (1).
    let global = Decoded::Form(0x0100_1000);
    let keyword = Decoded::Form(0x0100_1001);
    let first = &conds[0].1;
    for (at, want) in [
        ("Type.Operator", Decoded::Int(3)),
        ("Type.Flags", Decoded::Int(0x04)),
        ("ComparisonValue.Global", global.clone()),
        ("Function", Decoded::Int(74)),
        ("Parameter1.Global", global),
        ("RunOn", Decoded::Int(0)),
        ("Parameter3.Integer", Decoded::Int(0)),
    ] {
        assert_eq!(member(first, at), want, "CTDA[0].{at}");
    }
    let second = &conds[1].1;
    for (at, want) in [
        ("Type.Operator", Decoded::Int(0)),
        ("Type.Flags", Decoded::Int(0x01)),
        ("ComparisonValue.Float", Decoded::Float(1.0)),
        ("Function", Decoded::Int(72)),
        ("Parameter1.InventoryObject", Decoded::Form(0x0000_0800)),
        ("Parameter2.Unused", Decoded::Opaque { at: 16, len: 4 }),
        ("RunOn", Decoded::Int(4)),
        ("Reference.Unused", Decoded::Int(0)),
        ("Parameter3.LinkedKeyword", keyword),
    ] {
        assert_eq!(member(second, at), want, "CTDA[1].{at}");
    }
    assert_eq!(conds[2].1, Decoded::Text("StringOne".into()));
    assert_eq!(conds[3].1, Decoded::Text("StringTwo".into()));
    clean(&into, &data);
    // `inspect` finds the new form in the load order with the written plugin.
    let written = into.display().to_string();
    let r = call(
        "inspect",
        &data,
        &[written.as_str()],
        json!({"form": "KC_RecipeConditioned"}),
    );
    assert_eq!(r["command"], json!("inspect"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn set_appends_a_condition_to_an_override() {
    let (d, data) = fixture("append");
    let into = clone_recipe(&d, &data, "KC_RecipeConditioned", two_conditions());
    // New.esp is now in the load order; the override goes to Patch.esp.
    std::fs::rename(&into, data.join("New.esp")).unwrap();
    let before = record_in(
        &std::fs::read(data.join("New.esp")).unwrap(),
        "KC_RecipeConditioned",
    );
    let patch = d.join("Patch.esp");
    let r = call(
        "set",
        &data,
        &["New.esp"],
        json!({"form": "KC_RecipeConditioned", "into": patch.display().to_string(),
               "fields": {"CTDA[2].Function": "72", "CTDA[2].Parameter1.InventoryObject":
                          "Source.esm:000801", "CTDA[2].ComparisonValue.Float": 1.0,
                          // A part is written in place: CTDA[1] keeps its Or flag.
                          "CTDA[1].Type.Operator": "NotEqualTo"}}),
    );
    assert_eq!(r["data"]["validate"]["errors"], json!(0), "{:#}", r["data"]);
    let after = record_in(&std::fs::read(&patch).unwrap(), "KC_RecipeConditioned");
    reencodes(&after);
    let old = conditions(&before);
    let new = conditions(&after);
    let ctdas = |c: &[([u8; 4], Decoded)]| -> Vec<Decoded> {
        c.iter()
            .filter(|(s, _)| s == b"CTDA")
            .map(|(_, v)| v.clone())
            .collect()
    };
    let (old, new) = (ctdas(&old), ctdas(&new));
    assert_eq!(new.len(), 3);
    assert_eq!(new[0], old[0], "the first condition is unchanged");
    assert_eq!(member(&new[1], "Type.Operator"), Decoded::Int(1));
    assert_eq!(member(&new[1], "Type.Flags"), Decoded::Int(0x01));
    assert_eq!(member(&new[1], "Function"), member(&old[1], "Function"));
    assert_eq!(
        member(&new[2], "Parameter1.InventoryObject"),
        Decoded::Form(0x0000_0801)
    );
    clean(&patch, &data);
    // A gap (CTDA[4] on a record with three) is refused for a copied record.
    let err = execute(
        "set",
        json!({"form": "KC_RecipeConditioned", "into": d.join("Gap.esp").display().to_string(),
               "fields": {"CTDA[4].Function": 72},
               "plugins": ["Source.esm", "Conds.esp", "New.esp"],
               "data_dir": data.display().to_string(), "no_cache": true}),
    )
    .unwrap_err();
    assert!(err.to_string().contains("would be invented"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_dangling_condition_link_is_reported() {
    let (d, data) = fixture("dangling");
    let into = clone_recipe(
        &d,
        &data,
        "KC_RecipeDangling",
        json!({
            "CTDA.Type.Flags": "UseGlobal",
            "CTDA.ComparisonValue.Global": MISSING,
            "CTDA.Function": 72,
            "CTDA.Parameter1.InventoryObject": MISSING,
            "CTDA.RunOn": "Reference",
            "CTDA.Reference.Reference": MISSING,
            "CTDA[1].RunOn": "LinkedReference",
            "CTDA[1].Parameter3.LinkedKeyword": MISSING,
        }),
    );
    let v = execute(
        "validate",
        json!({"plugin": into.display().to_string(), "data_dir": data.display().to_string()}),
    )
    .unwrap();
    let findings: Vec<&Value> = v["data"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule"] == "formid.dangling")
        .collect();
    for want in ["ComparisonValue", "Parameter1", "Reference", "Parameter3"] {
        assert!(
            findings.iter().any(
                |f| f["subrecord"].as_str().is_some_and(|s| s.contains(want))
                    && f["message"].as_str().is_some_and(|m| m.contains("000FFF"))
            ),
            "no formid.dangling for {want}: {findings:#?}"
        );
    }
    let written = into.display().to_string();
    let refs = call(
        "refs",
        &data,
        &[written.as_str()],
        json!({"form": "KC_RecipeDangling"}),
    );
    let missing = refs["data"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["status"] == "missing")
        .count();
    assert!(missing >= 4, "{:#}", refs["data"]);
    let _ = std::fs::remove_dir_all(&d);
}
