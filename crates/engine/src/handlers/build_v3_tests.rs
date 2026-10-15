//! `build` spec version 3 (M4.E3) through `execute`: `[[place]]` entries, spec-local
//! names, class rules over typed world records (with `ONAM`), `[[import]]` of a Creation
//! Kit plugin's navmesh, the plan, determinism and every spec error.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_esm::build::HeaderSpec;
use kitkraft_esm::cells::{self, CellPlace, Parent, PlacementStyle};
use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::index::{index_bytes, IndexOptions};
use kitkraft_esm::tes4::FLAG_MASTER;
use kitkraft_esm::{Body, Plugin, Record, RecordHeader, Signature, Subrecord};
use serde_json::{json, Value};

use super::place_tests::{world_master, INTERIOR_CELL};
use crate::edit::patch_tests::dir;
use crate::execute;

fn spec_v3(d: &Path, name: &str, data: &Path, body: &str) -> PathBuf {
    let text = format!(
        "version = 3\n\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n{body}",
        data.display()
    );
    let path = d.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn build(spec: &Path, out: &Path, extra: Value) -> Result<Value, crate::EngineError> {
    let mut args = json!({"spec": spec.display().to_string(), "out": out.display().to_string()});
    for (k, v) in extra.as_object().unwrap() {
        args[k] = v.clone();
    }
    execute("build", args)
}

const POI: &str = "[[place]]\nname = \"marker\"\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\n\
position = [10, 20.5, 30]\nrotation = [0.0, 0.0, 1.5]\npersistent = true\nedid = \"KC_PoiMarker\"\n\
brief = \"POI\"\n\n[[place]]\nbase = \"KC_Npc\"\nworld = \"TestWorld\"\nposition = [150.0, -50.0, 0.0]\n";

#[test]
fn places_build_deterministically_and_the_plan_lists_new_forms() {
    let d = dir("build-v3-place");
    let data = world_master(&d);
    let spec = spec_v3(&d, "poi.toml", &data, POI);
    let out = d.join("POI.esm");
    // The plan names the new forms and writes nothing.
    let p = build(&spec, &out, json!({"plan": true})).unwrap();
    assert_eq!(p["data"]["plan"], json!(true));
    let placed = p["data"]["placed"].as_array().unwrap();
    assert_eq!(placed.len(), 2);
    assert_eq!(placed[0]["form_id"], json!("01001000"));
    assert_eq!(placed[0]["entry"], json!("place[0]"));
    assert_eq!(placed[1]["form_id"], json!("01001001"));
    assert_eq!(placed[1]["signature"], json!("ACHR"));
    assert!(!out.exists());
    // The build, twice: identical bytes.
    let r = build(&spec, &out, json!({})).unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    assert_eq!(r["data"]["next_object_id"], json!("FF001002"));
    let first = std::fs::read(&out).unwrap();
    build(&spec, &out, json!({"overwrite": true})).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), first);
    assert!(roundtrip_bytes(&first, &RoundtripOptions::default())
        .unwrap()
        .identical());
    // Carried: the interior cell, the world and its exterior cell (once each).
    let parsed = Plugin::parse(&first).unwrap();
    assert_eq!(parsed.tes4().hedr().unwrap().unwrap().record_count, 6);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn class_rules_over_world_records_write_overrides_and_onam() {
    let d = dir("build-v3-rules");
    let data = world_master(&d);
    let body = "[class.crates]\ntype = \"REFR\"\nwhere = 'NAME == World.esm:000800'\n\n\
[[rule]]\nclass = \"crates\"\nfields = { \"DATA.Position.Z\" = { add = 100.0 } }\n";
    let spec = spec_v3(&d, "rules.toml", &data, body);
    let out = d.join("Rules.esm");
    let r = build(&spec, &out, json!({})).unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    // The interior reference (temporary) and the persistent one, each with its cell.
    let changed: Vec<&str> = r["data"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["parent"] == json!(false))
        .map(|x| x["form"].as_str().unwrap())
        .collect();
    assert_eq!(changed.len(), 2, "{changed:?}");
    let bytes = std::fs::read(&out).unwrap();
    let parsed = Plugin::parse(&bytes).unwrap();
    // ONAM lists the override in temporary children only (lane A's rule).
    assert_eq!(parsed.tes4().overridden_forms(), [0x0000_0901]);
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("ONAM lists the 1")));
    let _ = std::fs::remove_dir_all(&d);
}

fn ck_plugin(data: &Path) -> PathBuf {
    let mut spec = HeaderSpec::new(vec![b"World.esm".to_vec()]);
    spec.flags = FLAG_MASTER;
    let mut ck = Plugin::create(&spec);
    let style = PlacementStyle::OFFICIAL;
    let rec = |sig: &[u8; 4], id: u32, subs: Vec<Subrecord<'static>>| {
        Record::new(
            RecordHeader {
                signature: Signature(*sig),
                flags: 0,
                form_id: id,
                timestamp: 0,
                version_control: 0,
                form_version: 582,
                unknown: 0,
            },
            Body::Raw(subs),
        )
    };
    let cell = rec(
        b"CELL",
        INTERIOR_CELL,
        vec![Subrecord::new(
            Signature(*b"EDID"),
            b"TestInterior\0".to_vec(),
        )],
    );
    let path =
        cells::placement(&cell.header, &Parent::CellAt(CellPlace::INTERIOR), &style).unwrap();
    ck.upsert_styled(&path, cell, &style).unwrap();
    let navm = rec(
        b"NAVM",
        0x0100_0800,
        vec![Subrecord::new(Signature(*b"NVNM"), vec![7u8; 16])],
    );
    let parent = Parent::Cell {
        place: CellPlace::INTERIOR,
        cell: INTERIOR_CELL,
    };
    let path = cells::placement(&navm.header, &parent, &style).unwrap();
    ck.upsert_styled(&path, navm, &style).unwrap();
    let file = data.parent().unwrap().join("CK.esm");
    std::fs::write(&file, ck.to_bytes().unwrap()).unwrap();
    file
}

#[test]
fn imports_copy_a_ck_navmesh_unchanged() {
    let d = dir("build-v3-import");
    let data = world_master(&d);
    ck_plugin(&data);
    let body = format!("{POI}\n[[import]]\nfrom = \"CK.esm\"\n");
    let spec = spec_v3(&d, "import.toml", &data, &body);
    let out = d.join("Nav.esm");
    let r = build(&spec, &out, json!({})).unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let import = &r["data"]["imports"][0];
    assert_eq!(import["inserted"], json!(1));
    assert_eq!(import["cells"], json!(["00000900"]));
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.windows(16).any(|w| w == [7u8; 16]));
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    // Evidence offsets were located again after the import.
    let placed = &r["data"]["placed"][0];
    let at = usize::try_from(placed["offset"].as_u64().unwrap()).unwrap();
    assert_eq!(&bytes[at..at + 4], b"REFR");
    // A CK plugin with other masters is refused (its navmesh FormIDs cannot be remapped).
    let mut spec_other = HeaderSpec::new(vec![b"World.esm".to_vec(), b"Other.esm".to_vec()]);
    spec_other.flags = FLAG_MASTER;
    std::fs::write(
        d.join("Bad.esm"),
        Plugin::create(&spec_other).to_bytes().unwrap(),
    )
    .unwrap();
    let body = format!("{POI}\n[[import]]\nfrom = \"Bad.esm\"\n");
    let spec = spec_v3(&d, "bad.toml", &data, &body);
    let err = build(&spec, &d.join("Bad2.esm"), json!({})).unwrap_err();
    assert!(err.to_string().contains("spec: import[0].from"), "{err}");
    assert!(!d.join("Bad2.esm").exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn v3_spec_errors_name_the_key() {
    let d = dir("build-v3-errors");
    let data = world_master(&d);
    let fails = |name: &str, body: &str| -> String {
        let spec = spec_v3(&d, name, &data, body);
        build(&spec, &d.join("E.esm"), json!({}))
            .unwrap_err()
            .to_string()
    };
    let place = |extra: &str| {
        format!("[[place]]\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\nposition = [0, 0, 0]\n{extra}")
    };
    // Unknown name, a cycle, a placed reference used as a base, a duplicate name.
    assert!(fails(
        "a.toml",
        "[[place]]\nbase = \"new:nope\"\ncell = \"TestInterior\"\nposition = [0, 0, 0]\n"
    )
    .contains("spec: place[0].base: unknown name `nope`"));
    assert!(fails("b.toml", &place("name = \"a\"\nworld = \"new:a\"\n")).contains("a cycle"));
    let two = format!(
        "{}\n[[place]]\nbase = \"new:a\"\ncell = \"TestInterior\"\nposition = [0, 0, 0]\n",
        place("name = \"a\"")
    );
    assert!(fails("c.toml", &two).contains("not a placed reference"));
    let dup = format!("{}\n{}", place("name = \"a\""), place("name = \"A\""));
    assert!(fails("d.toml", &dup).contains("spec: place[1].name"));
    // Two places with one EditorID.
    let same = format!("{}\n{}", place("edid = \"KC_X\""), place("edid = \"KC_X\""));
    assert!(fails("e.toml", &same).contains("place.edid-clash"));
    // Positions.
    assert!(fails(
        "f.toml",
        "[[place]]\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\nposition = [0, 0]\n"
    )
    .contains("spec: place[0].position: must be three numbers"));
    assert!(fails(
        "g.toml",
        "[[place]]\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\nposition = [0, inf, 0]\n"
    )
    .contains("spec: place[0].position[1]"));
    assert!(fails(
        "h.toml",
        "[[place]]\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\n"
    )
    .contains("spec: place[0].position: is required"));
    // A member below corpus confidence needs allow_reference by name.
    let err = fails("i.toml", &place("initially_disabled = true"));
    assert!(err.contains("flag.initially_disabled"), "{err}");
    let ok = spec_v3(
        &d,
        "j.toml",
        &data,
        &place("initially_disabled = true\nallow_reference = [\"flag.initially_disabled\"]"),
    );
    assert!(build(&ok, &d.join("J.esm"), json!({})).is_ok());
    assert!(fails("k.toml", &place("allow_reference = true"))
        .contains("spec: place[0].allow_reference"));
    // Unknown keys; entries version 3 does not have yet; place refusals keep their code.
    assert!(fails("l.toml", &place("bogus = 1")).contains("spec: place[0].bogus"));
    assert!(fails("m.toml", "[[new]]\ntype = \"GLOB\"\n").contains("spec: new"));
    assert!(fails("n.toml", &place("world = \"TestWorld\"")).contains("place.cell-and-world"));
    // Two writes to one field still conflict in version 3.
    let body = "[class.crates]\ntype = \"REFR\"\nwhere = 'NAME == World.esm:000800'\n\n\
[[rule]]\nclass = \"crates\"\nfields = { \"DATA.Position.Z\" = 1.0 }\n\n\
[[rule]]\nclass = \"crates\"\nfields = { \"DATA.Position.Z\" = 2.0 }\n";
    assert!(fails("o.toml", body).contains("already written by rule[0]"));
    // [[place]] keys are refused in version 2.
    let v2 = d.join("v2.toml");
    std::fs::write(&v2, format!("version = 2\n{}", place(""))).unwrap();
    let err = build(&v2, &d.join("V2.esm"), json!({})).unwrap_err();
    assert!(err.to_string().contains("spec: place"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

const NEW_AND_CLONE: &str = "[[clone]]\nname = \"crate\"\nform = \"KC_Crate\"\nedid = \"KC_CrateClone\"\n\n\
[[new]]\nname = \"marker\"\ntype = \"STAT\"\nedid = \"KC_NewStatic\"\nmasters = [\"World.esm\"]\n\n\
[[place]]\nbase = \"new:crate\"\ncell = \"TestInterior\"\nposition = [1, 2, 3]\nedid = \"KC_CloneRef\"\n\n\
[[place]]\nbase = \"new:marker\"\ncell = \"TestInterior\"\nposition = [4, 5, 6]\n";

#[test]
fn new_and_clone_entries_feed_places() {
    let d = dir("build-v3-new");
    let data = world_master(&d);
    let spec = spec_v3(&d, "new.toml", &data, NEW_AND_CLONE);
    let out = d.join("New.esm");
    let p = build(&spec, &out, json!({"plan": true})).unwrap();
    let forms: Vec<(String, String)> = p["data"]["new_forms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["editor_id"].as_str().unwrap().to_string(),
                f["form_id"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        forms,
        [
            ("KC_CrateClone".to_string(), "01001000".to_string()),
            ("KC_NewStatic".to_string(), "01001001".to_string()),
        ]
    );
    let r = build(&spec, &out, json!({})).unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let placed = r["data"]["placed"].as_array().unwrap();
    assert_eq!(placed[0]["stored_base"], json!("01001000"));
    assert_eq!(placed[0]["form_id"], json!("01001002"));
    assert_eq!(placed[1]["stored_base"], json!("01001001"));
    assert_eq!(placed[0]["base"]["editor_id"], json!("KC_CrateClone"));
    let bytes = std::fs::read(&out).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    // Two builds, the same bytes.
    build(&spec, &out, json!({"overwrite": true})).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), bytes);
    // Name errors: a clone of its own name, a clone of a new form, a new cell.
    let fails = |name: &str, body: &str| -> String {
        let spec = spec_v3(&d, name, &data, body);
        build(&spec, &d.join("E.esm"), json!({}))
            .unwrap_err()
            .to_string()
    };
    assert!(fails(
        "c1.toml",
        "[[clone]]\nname = \"a\"\nform = \"new:a\"\nedid = \"KC_A\"\n"
    )
    .contains("a cycle"));
    assert!(fails(
        "c2.toml",
        "[[new]]\nname = \"b\"\ntype = \"STAT\"\nedid = \"KC_B\"\n[[clone]]\nform = \"new:b\"\nedid = \"KC_C\"\n"
    )
    .contains("written by new[0]"));
    assert!(fails(
        "c3.toml",
        "[[new]]\nname = \"b\"\ntype = \"STAT\"\nedid = \"KC_B\"\n[[place]]\nbase = \"KC_Crate\"\ncell = \"new:b\"\nposition = [0, 0, 0]\n"
    )
    .contains("must be a cell or world"));
    assert!(fails(
        "c4.toml",
        "[[new]]\ntype = \"STAT\"\nedid = \"KC_Same\"\n[[clone]]\nform = \"KC_Crate\"\nedid = \"KC_Same\"\n"
    )
    .contains("edid-clash"));
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review (lane E fix round, findings 1 and 8): a `[[set]]` on a reference of a cell
/// plus a `[[place]]` into that cell write the cell once (the set's parent copy stands),
/// and a `[[set]]` on the holder cell itself is reported in `carried[]` with
/// `identical_to_winner` (false: it is edited).
#[test]
fn a_set_in_a_cell_and_a_place_into_it_carry_the_cell_once() {
    let d = dir("build-v3-set-place");
    let data = world_master(&d);
    let place = "[[place]]\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\nposition = [1, 2, 3]\n";
    // The reviewer's case on the synthetic master: a set on the cell's REFR.
    let body = format!(
        "[[set]]\nform = \"World.esm:000901\"\nfields = {{ \"DATA.Position.Z\" = 9.5 }}\n\n{place}"
    );
    let spec = spec_v3(&d, "child.toml", &data, &body);
    let out = d.join("Child.esm");
    let r = build(&spec, &out, json!({})).unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let bytes = std::fs::read(&out).unwrap();
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let cells = index
        .records
        .iter()
        .filter(|x| x.signature == Signature(*b"CELL"))
        .count();
    assert_eq!(cells, 1, "the cell is written once");
    let carried = r["data"]["carried"].as_array().unwrap();
    assert_eq!(carried.len(), 1, "{carried:?}");
    assert_eq!(carried[0]["form"], json!("World.esm:00000900"));
    assert_eq!(carried[0]["edited"], json!(false));
    assert_eq!(carried[0]["identical_to_winner"], json!(true));
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("is carried once")));
    // A set on the holder cell: written once, edited, not identical to its winner.
    let body =
        format!("[[set]]\nform = \"TestInterior\"\nfields = {{ FULL = \"Renamed\" }}\n\n{place}");
    let spec = spec_v3(&d, "holder.toml", &data, &body);
    let out = d.join("Holder.esm");
    let r = build(&spec, &out, json!({})).unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let carried = r["data"]["carried"].as_array().unwrap();
    assert_eq!(carried.len(), 1, "{carried:?}");
    assert_eq!(carried[0]["signature"], json!("CELL"));
    assert_eq!(carried[0]["edited"], json!(true));
    assert_eq!(carried[0]["identical_to_winner"], json!(false));
    assert_eq!(carried[0]["versions_in_load_order"], json!(1));
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review (lane E fix round, finding 2): a version 3 plan's text lists the placed
/// references, new forms, imports and carried records, and counts them in its head line.
#[test]
fn the_plan_text_of_a_v3_spec_lists_places_new_forms_imports_and_carried() {
    let d = dir("build-v3-plan-text");
    let data = world_master(&d);
    let body = format!(
        "{POI}\n[[new]]\ntype = \"GLOB\"\nedid = \"KC_PlanGlobal\"\nmasters = [\"World.esm\"]\n"
    );
    let spec = spec_v3(&d, "plan.toml", &data, &body);
    let out = d.join("Plan.esm");
    let p = build(&spec, &out, json!({"plan": true})).unwrap();
    let text = p["data"]["summary"].as_str().unwrap();
    let head = text.lines().next().unwrap();
    assert!(
        head.contains(
            ", 2 placed reference(s), 1 new form(s), 0 import(s), 3 carried record(s) for"
        ),
        "{text}"
    );
    assert!(
        text.contains("\nplaced (2):\n  POI  place[0] REFR "),
        "{text}"
    );
    assert!(
        text.contains("KC_PoiMarker: base World.esm:000800 KC_Crate in interior World.esm:000900 TestInterior at (10, 20.5, 30), persistent"),
        "{text}"
    );
    assert!(text.contains("\nnew forms (1):\n  GLOB "), "{text}");
    assert!(text.contains("KC_PlanGlobal: new (new GLOB)"), "{text}");
    assert!(text.contains("\nimports (0):\n"), "{text}");
    assert!(
        text.contains("\ncarried: CELL World.esm:00000900 TestInterior, WRLD World.esm:00000700 TestWorld, CELL World.esm:00000702 TestExterior"),
        "{text}"
    );
    assert_eq!(p["data"]["carried"].as_array().unwrap().len(), 3);
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// M4.E7: a field value `new:<name>` names a form a `[[new]]` or `[[clone]]` of the same
/// build writes (a recipe's created object): the FormID member holds the new form's
/// FormID in the built plugin. A name no entry has, or a `[[place]]`'s, is a spec error.
#[test]
fn field_values_name_new_forms_of_the_build() {
    let d = dir("build-v3-local");
    let data = world_master(&d);
    let body = "[[clone]]\nname = \"crate\"\nform = \"KC_Crate\"\nedid = \"KC_CrateClone\"\n\n\
[[new]]\nname = \"recipe\"\ntype = \"COBJ\"\nedid = \"KC_Recipe\"\nmasters = [\"World.esm\"]\n\
fields = { CNAM = \"new:crate\" }\n";
    let spec = spec_v3(&d, "local.toml", &data, body);
    let out = d.join("Local.esm");
    let r = build(&spec, &out, json!({})).unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let records = r["data"]["records"].as_array().unwrap();
    let recipe = records
        .iter()
        .find(|x| x["editor_id"] == json!("KC_Recipe"))
        .unwrap();
    let cnam = recipe["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == json!("CNAM"))
        .unwrap();
    assert_eq!(cnam["after"], json!("01001000"), "{cnam:#}");
    let bytes = std::fs::read(&out).unwrap();
    // The COBJ's CNAM subrecord on disk: signature, size 4, the clone's FormID.
    let mut want = b"CNAM\x04\x00".to_vec();
    want.extend(0x0100_1000u32.to_le_bytes());
    assert!(bytes.windows(want.len()).any(|w| w == want));
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    // Unknown names and placed references are spec errors naming the key.
    let fails = |name: &str, body: &str| -> String {
        let spec = spec_v3(&d, name, &data, body);
        build(&spec, &d.join("E.esm"), json!({}))
            .unwrap_err()
            .to_string()
    };
    let e = fails(
        "l1.toml",
        "[[new]]\ntype = \"COBJ\"\nedid = \"KC_R\"\nmasters = [\"World.esm\"]\nfields = { CNAM = \"new:nope\" }\n",
    );
    assert!(
        e.contains("new[0].fields.CNAM") && e.contains("unknown name `nope`"),
        "{e}"
    );
    let e = fails(
        "l2.toml",
        "[[place]]\nname = \"spot\"\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\nposition = [0, 0, 0]\n\
[[new]]\ntype = \"COBJ\"\nedid = \"KC_R\"\nmasters = [\"World.esm\"]\nfields = { CNAM = \"new:spot\" }\n",
    );
    assert!(e.contains("is a [[place]]"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}
