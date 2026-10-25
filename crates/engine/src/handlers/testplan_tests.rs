//! `testplan` (M4.E6) through `execute` on plugins `place` wrote over the synthetic world
//! master of the `place` tests.

#![allow(clippy::arithmetic_side_effects)]

use std::path::Path;

use serde_json::{json, Value};

use super::place_tests::world_master;
use crate::edit::patch_tests::dir;
use crate::execute;

fn place(data: &Path, into: &Path, extra: Value) {
    let mut a = json!({
        "base": "KC_Crate", "position": [10.0, 20.0, 30.0], "into": into.display().to_string(),
        "plugins": ["World.esm"], "data_dir": data.display().to_string(), "no_cache": true,
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    execute("place", a).unwrap();
}

fn testplan(
    data: &Path,
    plugin: &Path,
    out: &Path,
    plugins: Value,
) -> Result<Value, crate::EngineError> {
    execute(
        "testplan",
        json!({
            "plugin": plugin.display().to_string(), "out": out.display().to_string(),
            "plugins": plugins, "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
}

#[test]
fn a_placed_reference_gets_coc_moveto_and_prid() {
    let d = dir("testplan-poi");
    let data = world_master(&d);
    let plugin = d.join("KC_Test.esm");
    place(
        &data,
        &plugin,
        json!({"cell": "TestInterior", "edid": "KC_TestMarker", "persistent": true}),
    );
    place(
        &data,
        &plugin,
        json!({"base": "KC_Npc", "world": "TestWorld", "position": [150.0, -50.0, 0.0]}),
    );
    let out = d.join("plan");
    let r = testplan(&data, &plugin, &out, json!(["World.esm"])).unwrap();
    let batch = std::fs::read_to_string(out.join("kitkraft-test.txt")).unwrap();
    // The marker has an EditorID: named by it, no slot involved. The NPC has none: World.esm
    // takes slot 00; the plugin, master-flagged, goes after it: slot 01 (predicted).
    assert_eq!(
        batch,
        "coc TestInterior\r\nplayer.moveto KC_TestMarker\r\nprid KC_TestMarker\r\n\
         player.moveto 01001001\r\nprid 01001001\r\n"
    );
    assert_eq!(
        r["data"]["runtime_id_basis"],
        json!("inferred:runtime-slots-by-scale+masters-load-first")
    );
    assert!(r["data"]["commands"][3]["why"]
        .as_str()
        .unwrap()
        .contains("by its runtime ID (inferred:runtime-slots-by-scale+masters-load-first)"));
    let data_json = &r["data"];
    assert_eq!(data_json["profile"], json!("KitKraft Dev"));
    assert_eq!(data_json["appended"], json!(true));
    assert_eq!(data_json["load_index"], json!(1));
    let forms = data_json["forms"].as_array().unwrap();
    assert_eq!(forms.len(), 2);
    assert_eq!(forms[0]["runtime_id"], json!("01001000"));
    assert_eq!(forms[0]["cell"]["editor_id"], json!("TestInterior"));
    assert_eq!(forms[0]["persistent"], json!(true));
    assert_eq!(forms[1]["signature"], json!("ACHR"));
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("runtime-slots-by-scale")));
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("grade D")));
    let checklist = std::fs::read_to_string(out.join("checklist.md")).unwrap();
    let sha = data_json["sha256"].as_str().unwrap();
    for needle in [
        "KitKraft Dev",
        "disposable save",
        sha,
        "Owns forms | yes",
        "`coc TestInterior`",
        "`player.moveto KC_TestMarker`",
        "click the object: KC_Crate",
        "reference ID `01001000` (inferred:runtime-slots-by-scale+masters-load-first)",
        "Also `prid 01001000`",
        "predicted by the assumptions `runtime-slots-by-scale` and `masters-load-first`",
        "KC_Crate (STAT, World.esm:000800)",
        "(10.0, 20.0, 30.0)",
        "docs/game-observations.md",
        "game-<yyyymmdd>-<nn>",
        "Never on `Default`",
    ] {
        assert!(
            checklist.contains(needle),
            "missing `{needle}` in:\n{checklist}"
        );
    }
    // Deterministic, and nothing else written.
    let again = d.join("plan2");
    testplan(&data, &plugin, &again, json!(["World.esm"])).unwrap();
    assert_eq!(
        std::fs::read(out.join("kitkraft-test.txt")).unwrap(),
        std::fs::read(again.join("kitkraft-test.txt")).unwrap()
    );
    assert_eq!(
        std::fs::read(out.join("checklist.md")).unwrap(),
        std::fs::read(again.join("checklist.md")).unwrap()
    );
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["checklist.md", "kitkraft-test.txt", "probes.toml"]);
    // Already in the load order: kept where it is.
    let r = testplan(
        &data,
        &plugin,
        &d.join("plan3"),
        json!(["World.esm", plugin.display().to_string()]),
    )
    .unwrap();
    assert_eq!(r["data"]["appended"], json!(false));
    assert_eq!(
        r["data"]["runtime_id_basis"],
        json!("inferred:runtime-slots-by-scale")
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// POI investigation follow-up: with a save whose lists name the load order (the plugin in
/// it), the numeric IDs are the save's and say so; the EditorID still leads.
#[test]
fn a_save_confirmed_order_gives_confirmed_ids() {
    let d = dir("testplan-save");
    let data = world_master(&d);
    let plugin = d.join("KC_Test.esm");
    place(
        &data,
        &plugin,
        json!({"cell": "TestInterior", "edid": "KC_TestMarker", "persistent": true}),
    );
    // The save puts the plugin first: slot 00, World.esm 01 (only a save can say so).
    let save = d.join("Exitsave0.sfs");
    std::fs::write(
        &save,
        super::testdata::save(&["KC_Test.esm", "World.esm"], &[], &[]),
    )
    .unwrap();
    let out = d.join("plan");
    let r = execute(
        "testplan",
        json!({
            "plugin": plugin.display().to_string(), "out": out.display().to_string(),
            "plugins": ["World.esm", plugin.display().to_string()],
            "data_dir": data.display().to_string(), "no_cache": true,
            "from_save": save.display().to_string(),
        }),
    )
    .unwrap();
    assert_eq!(r["data"]["runtime_id_basis"], json!("confirmed-by-save"));
    assert_eq!(r["data"]["forms"][0]["runtime_id"], json!("00001000"));
    assert_eq!(
        r["data"]["forms"][0]["runtime_id_basis"],
        json!("confirmed-by-save")
    );
    let batch = std::fs::read_to_string(out.join("kitkraft-test.txt")).unwrap();
    assert_eq!(
        batch,
        "coc TestInterior\r\nplayer.moveto KC_TestMarker\r\nprid KC_TestMarker\r\n"
    );
    let checklist = std::fs::read_to_string(out.join("checklist.md")).unwrap();
    for needle in [
        "| Load-order slot | 00: confirmed by the save Exitsave0.sfs (game 1.16.244.0)",
        "reference ID `00001000` (confirmed-by-save)",
    ] {
        assert!(
            checklist.contains(needle),
            "missing `{needle}` in:\n{checklist}"
        );
    }
    let limitations = r["limitations"].as_array().unwrap();
    assert!(limitations
        .iter()
        .any(|l| l.as_str().unwrap().contains("confirmed by the save")));
    assert!(!limitations
        .iter()
        .any(|l| l.as_str().unwrap().contains("are inferred")));
    let _ = std::fs::remove_dir_all(&d);
}

/// KK-010: a new `ARMO` is an outfit, not just an item: the batch adds and equips it, the
/// checklist says to equip and look at it, and the probes carry the equip check and a
/// manual look row, in step with the checklist's numbering.
#[test]
fn a_new_armor_is_added_equipped_and_looked_at() {
    let d = dir("testplan-armor-new");
    let data = world_master(&d);
    let plugin = d.join("KC_Outfit.esp");
    execute(
        "new",
        json!({
            "type": "ARMO", "edid": "KC_Outfit", "fields": {}, "masters": ["World.esm"],
            "into": plugin.display().to_string(),
            "plugins": ["World.esm"], "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap();
    let out = d.join("plan");
    let r = testplan(&data, &plugin, &out, json!(["World.esm"])).unwrap();
    // World.esm takes slot 00; the plugin is appended: slot 01, the first new form.
    assert_eq!(
        r["data"]["runtime_id_basis"],
        json!("inferred:runtime-slots-by-scale+masters-load-first")
    );
    assert_eq!(r["data"]["forms"][0]["form_id"], json!("01001000"));
    assert_eq!(r["data"]["forms"][0]["new"], json!(true));
    assert_eq!(r["data"]["forms"][0]["runtime_id"], json!("01001000"));
    let batch = std::fs::read_to_string(out.join("kitkraft-test.txt")).unwrap();
    assert_eq!(
        batch,
        "player.additem 01001000 1\r\nplayer.equipitem 01001000\r\n"
    );
    let checklist = std::fs::read_to_string(out.join("checklist.md")).unwrap();
    for needle in [
        "`player.additem 01001000 1`",
        "`player.equipitem 01001000`: equip it. **Expect:** the player wears it;",
        "Look at it: third person, `tfc` free camera, a screenshot into",
    ] {
        assert!(
            checklist.contains(needle),
            "missing `{needle}` in:\n{checklist}"
        );
    }
    let probes_text = std::fs::read_to_string(out.join("probes.toml")).unwrap();
    let probes = kitkraft_game::probes::ProbeFile::parse(&probes_text).unwrap();
    assert_eq!(probes.probes.len(), 5);
    for (i, p) in probes.probes.iter().enumerate() {
        assert_eq!(p.row, format!("T-{:02}", i + 1));
    }
    let add = &probes.probes[0];
    assert_eq!(add.steps[0].command, "player.additem 01001000 1");
    assert_eq!(add.steps[1].command, "player.getitemcount 01001000");
    let equip = &probes.probes[1];
    assert_eq!(equip.steps[0].command, "player.equipitem 01001000");
    assert_eq!(equip.steps[1].command, "player.getequipped 01001000");
    assert_eq!(
        equip.steps[1].matcher,
        Some(kitkraft_game::probes::Matcher::Regex)
    );
    assert_eq!(
        equip.steps[1].expected.as_deref(),
        Some(r">>\s*1(\.0+)?\s*$")
    );
    assert!(equip.steps[1].why.as_deref().unwrap().contains("grade D"));
    let look = &probes.probes[2];
    assert_eq!(look.matcher.as_deref(), Some("manual"));
    assert!(look
        .reason
        .as_deref()
        .unwrap()
        .contains("screenshot and a person"));
    assert_eq!(r["data"]["probes"]["rows"], json!(5));
    let _ = std::fs::remove_dir_all(&d);
}

/// KK-010: an override of an `ARMO` gets the same add, equip and look steps as a new one,
/// by its runtime ID, and no `help` step.
#[test]
fn an_armor_override_is_added_and_equipped() {
    let d = dir("testplan-armor-override");
    let data = world_master(&d);
    // The master owns the armor; `Mod.esp` overrides its name.
    let master = d.join("Data").join("Armor.esm");
    execute(
        "new",
        json!({
            "type": "ARMO", "edid": "KC_Outfit", "fields": {"FULL": "KC OUTFIT"},
            "masters": [],
            "into": master.display().to_string(),
            "plugins": ["World.esm"], "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap();
    let plugin = d.join("Mod.esp");
    execute(
        "set",
        json!({
            "form": "Armor.esm:00001000", "fields": {"FULL": "KC OUTFIT Red"},
            "into": plugin.display().to_string(),
            "plugins": ["Armor.esm"], "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap();
    let out = d.join("plan");
    let r = testplan(&data, &plugin, &out, json!(["Armor.esm"])).unwrap();
    // Armor.esm takes slot 00; `Mod.esp` is appended: slot 01. The override keeps the
    // master's stored ID, which resolves to the base owner (Armor.esm): its runtime ID
    // is the master's slot, 00001000.
    assert_eq!(
        r["data"]["runtime_id_basis"],
        json!("inferred:runtime-slots-by-scale+masters-load-first")
    );
    assert_eq!(r["data"]["forms"][0]["form_id"], json!("00001000"));
    assert_eq!(r["data"]["forms"][0]["new"], json!(false));
    assert_eq!(r["data"]["forms"][0]["editor_id"], json!("KC_Outfit"));
    assert_eq!(r["data"]["forms"][0]["runtime_id"], json!("00001000"));
    let batch = std::fs::read_to_string(out.join("kitkraft-test.txt")).unwrap();
    assert_eq!(
        batch,
        "player.additem 00001000 1\r\nplayer.equipitem 00001000\r\n"
    );
    let checklist = std::fs::read_to_string(out.join("checklist.md")).unwrap();
    for needle in [
        "`player.additem 00001000 1`",
        "`player.equipitem 00001000`: equip it. **Expect:** the player wears it;",
        "Look at it: third person, `tfc` free camera, a screenshot into",
    ] {
        assert!(
            checklist.contains(needle),
            "missing `{needle}` in:\n{checklist}"
        );
    }
    assert!(
        !checklist.contains("help "),
        "a `help` step for the override:\n{checklist}"
    );
    let probes_text = std::fs::read_to_string(out.join("probes.toml")).unwrap();
    let probes = kitkraft_game::probes::ProbeFile::parse(&probes_text).unwrap();
    assert_eq!(probes.probes.len(), 5);
    for (i, p) in probes.probes.iter().enumerate() {
        assert_eq!(p.row, format!("T-{:02}", i + 1));
    }
    for p in &probes.probes {
        for s in &p.steps {
            assert!(
                !s.command.starts_with("help "),
                "a `help` probe step: {}",
                s.command
            );
        }
    }
    let equip = &probes.probes[1];
    assert_eq!(equip.steps[0].command, "player.equipitem 00001000");
    assert_eq!(equip.steps[1].command, "player.getequipped 00001000");
    assert_eq!(probes.probes[2].matcher.as_deref(), Some("manual"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn testplan_refuses_mo2_and_missing_inputs() {
    let d = dir("testplan-guard");
    let data = world_master(&d);
    let plugin = d.join("KC_G.esm");
    place(&data, &plugin, json!({"cell": "TestInterior"}));
    let mo2 = d.join("MO2");
    std::fs::create_dir_all(&mo2).unwrap();
    std::fs::write(mo2.join("ModOrganizer.ini"), b"[General]\r\n").unwrap();
    let err = testplan(&data, &plugin, &mo2.join("plan"), json!(["World.esm"])).unwrap_err();
    assert!(err.to_string().contains("Mod Organizer"), "{err}");
    assert!(!mo2.join("plan").exists());
    let err = testplan(
        &data,
        &d.join("Nope.esm"),
        &d.join("p"),
        json!(["World.esm"]),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not a file"), "{err}");
    let err = execute(
        "testplan",
        json!({"plugin": plugin.display().to_string(), "out": d.join("p").display().to_string()}),
    )
    .unwrap_err();
    assert!(err.to_string().contains("no load order"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}
