//! `place` (M4.E2) through `execute` with JSON on a synthetic master with an interior cell,
//! a world with a persistent and an exterior cell, a static, an NPC and a leveled actor.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_esm::build::HeaderSpec;
use kitkraft_esm::cells::{self, CellPlace, Grid, Parent, PlacementStyle, FLAG_PERSISTENT};
use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::index::{index_bytes, IndexOptions};
use kitkraft_esm::tes4::FLAG_MASTER;
use kitkraft_esm::{Body, Plugin, Record, RecordHeader, Signature, Subrecord};
use serde_json::{json, Value};

use crate::edit::patch_tests::dir;
use crate::{execute, parse_line, Invocation};

pub(crate) const WORLD: u32 = 0x0000_0700;
pub(crate) const PERSISTENT_CELL: u32 = 0x0000_0701;
pub(crate) const EXTERIOR_CELL: u32 = 0x0000_0702;
pub(crate) const INTERIOR_CELL: u32 = 0x0000_0900;
pub(crate) const NAVMESH: u32 = 0x0000_0710;

pub(crate) fn sub(sig: &[u8; 4], data: &[u8]) -> Subrecord<'static> {
    Subrecord::new(Signature(*sig), data.to_vec())
}

pub(crate) fn zstr(text: &str) -> Vec<u8> {
    let mut v = text.as_bytes().to_vec();
    v.push(0);
    v
}

pub(crate) fn rec(
    sig: &[u8; 4],
    flags: u32,
    form_id: u32,
    subs: Vec<Subrecord<'static>>,
) -> Record<'static> {
    Record::new(
        RecordHeader {
            signature: Signature(*sig),
            flags,
            form_id,
            timestamp: 0,
            version_control: 0,
            form_version: 582,
            unknown: 0,
        },
        Body::Raw(subs),
    )
}

pub(crate) fn data24(pos: [f32; 3]) -> Vec<u8> {
    let mut d = Vec::new();
    for v in pos.iter().chain(&[0.0f32, 0.0, 0.0]) {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d
}

/// `World.esm`: STAT `KC_Crate` (800), NPC_ `KC_Npc` (801), LVLN `KC_Lvl` (802); interior
/// CELL `TestInterior` (900) with one reference; WRLD `TestWorld` (700) with its persistent
/// cell (701, one persistent reference) and the exterior cell `TestExterior` (702) at grid
/// (1, -1).
pub(crate) fn world_master(dir: &Path) -> PathBuf {
    let data = dir.join("Data");
    std::fs::create_dir_all(&data).unwrap();
    let mut spec = HeaderSpec::new(Vec::new());
    spec.flags = FLAG_MASTER;
    let mut plugin = Plugin::create(&spec);
    // INCC: one interior cell, as every official file counts them.
    plugin
        .header_mut()
        .subrecords_mut()
        .unwrap()
        .push(sub(b"INCC", &1u32.to_le_bytes()));
    let style = PlacementStyle::OFFICIAL;
    let mut put = |record: Record<'static>, parent: Parent| {
        let path = cells::placement(&record.header, &parent, &style).unwrap();
        plugin.upsert_styled(&path, record, &style).unwrap();
    };
    put(
        rec(b"STAT", 0, 0x800, vec![sub(b"EDID", &zstr("KC_Crate"))]),
        Parent::None,
    );
    put(
        rec(b"NPC_", 0, 0x801, vec![sub(b"EDID", &zstr("KC_Npc"))]),
        Parent::None,
    );
    put(
        rec(b"LVLN", 0, 0x802, vec![sub(b"EDID", &zstr("KC_Lvl"))]),
        Parent::None,
    );
    put(
        rec(
            b"CELL",
            0,
            INTERIOR_CELL,
            vec![
                sub(b"EDID", &zstr("TestInterior")),
                sub(b"FULL", &zstr("Test Room")),
            ],
        ),
        Parent::CellAt(CellPlace::INTERIOR),
    );
    put(
        rec(
            b"REFR",
            0,
            0x901,
            vec![
                sub(b"NAME", &0x800u32.to_le_bytes()),
                sub(b"DATA", &data24([1.0, 2.0, 3.0])),
            ],
        ),
        Parent::Cell {
            place: CellPlace::INTERIOR,
            cell: INTERIOR_CELL,
        },
    );
    put(
        rec(b"WRLD", 0, WORLD, vec![sub(b"EDID", &zstr("TestWorld"))]),
        Parent::None,
    );
    let pcell = CellPlace::Persistent { world: WORLD };
    put(
        rec(
            b"CELL",
            FLAG_PERSISTENT,
            PERSISTENT_CELL,
            vec![sub(b"DATA", &[0, 0])],
        ),
        Parent::CellAt(pcell),
    );
    put(
        rec(
            b"REFR",
            FLAG_PERSISTENT,
            0x703,
            vec![
                sub(b"NAME", &0x800u32.to_le_bytes()),
                sub(b"DATA", &data24([0.0, 0.0, 0.0])),
            ],
        ),
        Parent::Cell {
            place: pcell,
            cell: PERSISTENT_CELL,
        },
    );
    let grid = Grid::new(1, -1);
    let xclc = [1i32.to_le_bytes(), (-1i32).to_le_bytes(), [0u8; 4]].concat();
    put(
        rec(
            b"CELL",
            0,
            EXTERIOR_CELL,
            vec![sub(b"EDID", &zstr("TestExterior")), sub(b"XCLC", &xclc)],
        ),
        Parent::CellAt(CellPlace::Exterior { world: WORLD, grid }),
    );
    // A navmesh in the exterior cell.
    put(
        rec(b"NAVM", 0, NAVMESH, vec![sub(b"NVNM", &[5u8; 12])]),
        Parent::Cell {
            place: CellPlace::Exterior { world: WORLD, grid },
            cell: EXTERIOR_CELL,
        },
    );
    let bytes = plugin.to_bytes().unwrap();
    std::fs::write(data.join("World.esm"), bytes).unwrap();
    data
}

fn args(data: &Path, into: &Path, extra: Value) -> Value {
    let mut a = json!({
        "base": "KC_Crate", "position": [10.0, 20.0, 30.0], "into": into.display().to_string(),
        "plugins": ["World.esm"], "data_dir": data.display().to_string(), "no_cache": true,
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

fn place(data: &Path, into: &Path, extra: Value) -> Result<Value, crate::EngineError> {
    execute("place", args(data, into, extra))
}

fn code(err: crate::EngineError) -> String {
    if err.kind() == "invalid_args" {
        return "invalid_args".into();
    }
    let text = err.to_string();
    text.split("place.")
        .nth(1)
        .and_then(|t| t.split(':').next())
        .unwrap_or(&text)
        .to_string()
}

#[test]
fn a_reference_lands_in_an_interior_cell_with_its_parent_carried() {
    let d = dir("place-interior");
    let data = world_master(&d);
    let into = d.join("KC_New.esm");
    let r = place(
        &data,
        &into,
        json!({"cell": "TestInterior", "edid": "KC_TestMarker", "rotation": [0.0, 0.0, 1.5]}),
    )
    .unwrap();
    let placed = &r["data"]["placed"][0];
    assert_eq!(placed["signature"], json!("REFR"));
    assert_eq!(placed["form_id"], json!("01001000"));
    assert_eq!(placed["form"], json!("KC_New.esm:00001000"));
    assert_eq!(placed["editor_id"], json!("KC_TestMarker"));
    assert_eq!(placed["base"]["form"], json!("World.esm:000800"));
    assert_eq!(placed["base"]["signature"], json!("STAT"));
    assert_eq!(placed["cell"]["kind"], json!("interior"));
    assert_eq!(placed["cell"]["editor_id"], json!("TestInterior"));
    assert_eq!(placed["stored_base"], json!("00000800"));
    // CELL top, block 4, sub-block 0 (0x900 = 2304), children, temporary.
    let path: Vec<i64> = placed["group_path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["type"].as_i64().unwrap())
        .collect();
    assert_eq!(path, [0, 2, 3, 6, 9]);
    assert!(placed["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["member"] == json!("NAME")));
    assert_eq!(r["data"]["masters"], json!(["World.esm"]));
    assert_eq!(r["data"]["next_object_id"], json!("FF001001"));
    let carried = &r["data"]["carried"][0];
    assert_eq!(carried["signature"], json!("CELL"));
    assert_eq!(carried["identical_to_winner"], json!(true));
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    // Evidence: the base and cell winners, and the written records.
    let files: Vec<&str> = r["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["file"].as_str().unwrap())
        .collect();
    assert!(
        files.contains(&"World.esm") && files.contains(&"KC_New.esm"),
        "{files:?}"
    );
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("fixture F06")));

    // The file: roundtrips, HEDR advanced, INCC 1, the reference in its cell's children.
    let bytes = std::fs::read(&into).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let parsed = Plugin::parse(&bytes).unwrap();
    let hedr = parsed.tes4().hedr().unwrap().unwrap();
    assert_eq!(hedr.next_object_id, 0xFF00_1001);
    assert_eq!(hedr.record_count, 3); // TES4, CELL, REFR
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let refr = index
        .records
        .iter()
        .find(|r| r.form_id == 0x0100_1000)
        .unwrap();
    let groups: Vec<(i32, u32)> = index
        .group_path(refr.group)
        .iter()
        .map(|g| (g.header.group_type, g.header.label_u32()))
        .collect();
    assert_eq!(groups[3..], [(6, INTERIOR_CELL), (9, INTERIOR_CELL)]);
    // inspect with the plugin appended finds it.
    let i = execute(
        "inspect",
        json!({"form": "KC_New.esm:001000", "plugins": ["World.esm", into.display().to_string()],
               "data_dir": data.display().to_string(), "no_cache": true}),
    )
    .unwrap();
    assert_eq!(i["data"]["signature"], json!("REFR"), "{:#}", i["data"]);
    // validate from the outside agrees.
    let v = execute(
        "validate",
        json!({"plugin": into.display().to_string(), "data_dir": data.display().to_string()}),
    )
    .unwrap();
    assert_eq!(v["data"]["passed"], json!(true), "{:#}", v["data"]);

    // A second reference into the same plugin: the next FormID; the cell it has is kept.
    let r2 = place(
        &data,
        &into,
        json!({"cell": "World.esm:000900", "persistent": true}),
    )
    .unwrap();
    assert_eq!(r2["data"]["placed"][0]["form_id"], json!("01001001"));
    assert_eq!(r2["data"]["carried"], json!([]));
    assert!(r2["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("kept as it is")));
    let path: Vec<i64> = r2["data"]["placed"][0]["group_path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["type"].as_i64().unwrap())
        .collect();
    assert_eq!(path.last(), Some(&8));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn world_positions_pick_the_exterior_or_the_persistent_cell() {
    let d = dir("place-world");
    let data = world_master(&d);
    let a = d.join("A.esm");
    let r = place(
        &data,
        &a,
        json!({"world": "TestWorld", "position": [150.0, -50.0, 5.0]}),
    )
    .unwrap();
    let placed = &r["data"]["placed"][0];
    assert_eq!(placed["cell"]["kind"], json!("exterior"));
    assert_eq!(placed["cell"]["grid"], json!([1, -1]));
    assert_eq!(placed["world"]["editor_id"], json!("TestWorld"));
    // Carried: the world and the cell.
    let carried: Vec<&str> = r["data"]["carried"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["signature"].as_str().unwrap())
        .collect();
    assert_eq!(carried, ["WRLD", "CELL"]);
    assert_eq!(r["data"]["validate"]["passed"], json!(true));
    // Persistent: the world's persistent cell, by world or by the exterior cell.
    for extra in [
        json!({"world": "TestWorld", "position": [150.0, -50.0, 5.0], "persistent": true}),
        json!({"cell": "TestExterior", "position": [150.0, -50.0, 5.0], "persistent": true}),
    ] {
        let b = d.join("B.esm");
        let _ = std::fs::remove_file(&b);
        let r = place(&data, &b, extra).unwrap();
        let placed = &r["data"]["placed"][0];
        assert_eq!(placed["cell"]["kind"], json!("persistent"));
        assert_eq!(placed["stored_cell"], json!("00000701"));
        let path: Vec<i64> = placed["group_path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["type"].as_i64().unwrap())
            .collect();
        assert_eq!(path, [0, 1, 6, 8]);
        let bytes = std::fs::read(&b).unwrap();
        assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
            .unwrap()
            .identical());
    }
    // An NPC base is an ACHR.
    let c = d.join("C.esm");
    let r = place(&data, &c, json!({"base": "KC_Npc", "cell": "TestInterior"})).unwrap();
    assert_eq!(r["data"]["placed"][0]["signature"], json!("ACHR"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn place_is_deterministic_and_the_cli_form_matches() {
    let d = dir("place-determ");
    let data = world_master(&d);
    let a = d.join("A.esm");
    let b = d.join("B.esm");
    place(&data, &a, json!({"cell": "TestInterior", "edid": "KC_M"})).unwrap();
    let line = format!(
        r#"place KC_Crate --position "[10.0, 20.0, 30.0]" --into "{}" --cell TestInterior --edid KC_M --plugins World.esm --data-dir "{}" --no-cache"#,
        b.display(),
        data.display()
    );
    let Invocation::Run { command, args } = parse_line(&line).unwrap() else {
        panic!()
    };
    execute(command.id, args).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn refusals_are_typed_and_write_nothing() {
    let d = dir("place-refusals");
    let data = world_master(&d);
    let into = d.join("R.esm");
    let cases = [
        (
            json!({"base": "TestInterior", "cell": "TestInterior"}),
            "base-not-placeable",
        ),
        (
            json!({"base": "KC_Lvl", "cell": "TestInterior"}),
            "base-not-placeable",
        ),
        (json!({"base": "Nope", "cell": "TestInterior"}), "no-base"),
        (json!({"cell": "KC_Crate"}), "not-a-cell"),
        (json!({"cell": "Nope"}), "no-cell"),
        (json!({}), "no-cell"),
        (
            json!({"cell": "TestInterior", "world": "TestWorld"}),
            "cell-and-world",
        ),
        (json!({"world": "KC_Crate"}), "not-a-world"),
        (
            json!({"world": "TestWorld", "position": [950.0, 950.0, 0.0]}),
            "no-exterior-cell",
        ),
        (json!({"cell": "World.esm:000701"}), "persistence"),
        (
            json!({"cell": "TestInterior", "edid": "KC_Crate"}),
            "edid-clash",
        ),
        (
            json!({"cell": "TestInterior", "edid": "bad id"}),
            "edid-invalid",
        ),
        (
            json!({"cell": "TestInterior", "edid": "bad\u{0}id"}),
            "edid-invalid",
        ),
        (
            json!({"cell": "TestInterior", "position": [1.0, 2.0]}),
            "invalid_args",
        ),
    ];
    for (extra, want) in cases {
        let err = place(&data, &into, extra.clone()).unwrap_err();
        assert_eq!(code(err), want, "{extra}");
        assert!(!into.exists(), "{extra}");
    }
    // A position outside the named exterior cell is written, with a limitation.
    let out = d.join("Out.esm");
    let r = place(&data, &out, json!({"cell": "TestExterior"})).unwrap();
    assert!(r["limitations"].as_array().unwrap().iter().any(|l| l
        .as_str()
        .unwrap()
        .starts_with("place.position-outside-cell")));
    // A member below corpus confidence without allow_reference.
    let err = place(
        &data,
        &into,
        json!({"cell": "TestInterior", "initially_disabled": true}),
    )
    .unwrap_err();
    assert!(err.to_string().contains("flag.initially_disabled"), "{err}");
    assert!(!into.exists());
    let ok = place(
        &data,
        &into,
        json!({"cell": "TestInterior", "initially_disabled": true,
               "allow_reference": ["flag.initially_disabled"], "edid": "KC_One"}),
    )
    .unwrap();
    assert!(ok["limitations"].as_array().unwrap().iter().any(|l| l
        .as_str()
        .unwrap()
        .contains("flag.initially_disabled written")));
    // The same EditorID again, now in the target.
    let err = place(
        &data,
        &into,
        json!({"cell": "TestInterior", "edid": "KC_One"}),
    )
    .unwrap_err();
    assert_eq!(code(err), "edid-clash");
    // Small targets and .esl are refused.
    let err = place(&data, &d.join("S.esl"), json!({"cell": "TestInterior"})).unwrap_err();
    assert!(err.to_string().contains(".esl"), "{err}");
    let small = d.join("Small.esp");
    let mut spec = HeaderSpec::new(vec![b"World.esm".to_vec()]);
    spec.flags = 0x100;
    std::fs::write(&small, Plugin::create(&spec).to_bytes().unwrap()).unwrap();
    let before = std::fs::read(&small).unwrap();
    let err = place(&data, &small, json!({"cell": "TestInterior"})).unwrap_err();
    assert!(err.to_string().contains("small or medium"), "{err}");
    assert_eq!(std::fs::read(&small).unwrap(), before);
    let _ = std::fs::remove_dir_all(&d);
}

/// Coordinator's round (lane A review): a small or medium master is refused before any
/// FormID is chosen, naming the master.
#[test]
fn a_small_master_refuses_new_forms() {
    let d = dir("place-small-master");
    let data = world_master(&d);
    let mut spec = HeaderSpec::new(Vec::new());
    spec.flags = FLAG_MASTER | 0x100;
    let mut small = Plugin::create(&spec);
    small
        .upsert(
            &[kitkraft_esm::GroupHeader::top(Signature(*b"STAT"))],
            rec(
                b"STAT",
                0,
                0x800,
                vec![sub(b"EDID", &zstr("KC_SmallThing"))],
            ),
        )
        .unwrap();
    std::fs::write(data.join("Small.esm"), small.to_bytes().unwrap()).unwrap();
    let into = d.join("P.esm");
    let mut a = args(
        &data,
        &into,
        json!({"base": "KC_SmallThing", "cell": "TestInterior"}),
    );
    a["plugins"] = json!(["World.esm", "Small.esm"]);
    let err = execute("place", a).unwrap_err();
    assert!(
        err.to_string().contains("place.scaled-master") && err.to_string().contains("Small.esm"),
        "{err}"
    );
    assert!(!into.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// Coordinator's round: when the target already has the cell, the reference goes under the
/// cell's observed group path there (here a block the formula does not predict), and the
/// target's cell is kept as it is.
#[test]
fn an_existing_cell_keeps_its_observed_path() {
    let d = dir("place-observed");
    let data = world_master(&d);
    let target = d.join("Third.esm");
    let mut spec = HeaderSpec::new(vec![b"World.esm".to_vec()]);
    spec.flags = FLAG_MASTER;
    let mut third = Plugin::create(&spec);
    let style = PlacementStyle::OFFICIAL;
    let path = vec![
        style.header(*b"CELL", 0),
        style.header(7u32.to_le_bytes(), 2),
        style.header(7u32.to_le_bytes(), 3),
    ];
    let cell = rec(
        b"CELL",
        0,
        INTERIOR_CELL,
        vec![
            sub(b"EDID", &zstr("TestInterior")),
            sub(b"FULL", &zstr("Moved")),
        ],
    );
    third.upsert(&path, cell).unwrap();
    std::fs::write(&target, third.to_bytes().unwrap()).unwrap();
    let r = place(&data, &target, json!({"cell": "TestInterior"})).unwrap();
    let labels: Vec<String> = r["data"]["placed"][0]["group_path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["label"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(labels[1..3], ["00000007", "00000007"], "{labels:?}");
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("kept as it is")));
    let bytes = std::fs::read(&target).unwrap();
    assert!(bytes.windows(6).any(|w| w == b"Moved\0"));
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let cells = index
        .records
        .iter()
        .filter(|r| r.signature == Signature(*b"CELL"))
        .count();
    assert_eq!(cells, 1, "the cell is not duplicated");
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review (lane E fix round, finding 4): the header stamps of a new reference (and of
/// a new form) are not called corpus defaults: 0 is rare or unobserved in the corpus for
/// them and their meaning is unknown (F20). The values are unchanged.
#[test]
fn header_stamps_are_not_claimed_as_corpus_defaults() {
    let d = dir("place-stamps");
    let data = world_master(&d);
    let into = d.join("S.esm");
    let r = place(&data, &into, json!({"cell": "TestInterior"})).unwrap();
    let text: Vec<&str> = r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert!(
        !text.iter().any(|l| l.contains("corpus default")),
        "{text:?}"
    );
    assert!(
        text.iter()
            .any(|l| l.contains("rare or unobserved in the corpus")
                && l.contains("meaning is unknown")
                && l.contains("F20")),
        "{text:?}"
    );
    let bytes = std::fs::read(&into).unwrap();
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let refr = index
        .records
        .iter()
        .find(|x| x.signature == Signature(*b"REFR"))
        .unwrap();
    let header = &bytes[refr.offset as usize..refr.offset as usize + 24];
    // Timestamp, version control, form version 582, unknown: as before.
    assert_eq!(header[16..24], [0, 0, 0, 0, 0x46, 0x02, 0, 0]);
    let r = execute(
        "new",
        json!({"type": "STAT", "edid": "KC_Stamped", "into": d.join("N.esm").display().to_string(),
               "masters": ["World.esm"], "plugins": ["World.esm"],
               "data_dir": data.display().to_string(), "no_cache": true}),
    )
    .unwrap();
    assert!(!r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("corpus default")));
    let _ = std::fs::remove_dir_all(&d);
}
