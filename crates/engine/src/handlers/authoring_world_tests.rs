//! Typed write and reference integrity for REFR, ACHR and CELL (lane D round 4,
//! 2026-10-16; `docs/schema/support-matrix.md`, the typed-writable evidence of
//! `docs/schema/evidence.toml`), in the pattern of `authoring_tests.rs` on the synthetic
//! master of the `place` tests (`World.esm`: the static `KC_Crate` 800, the NPC `KC_Npc`
//! 801, the interior cell `TestInterior`, the world `TestWorld` with the exterior cell
//! `TestExterior` and its navmesh).
//!
//! - REFR and ACHR: `place` puts a new reference (of `KC_Crate` into `TestInterior`, or of
//!   `KC_Npc` into `TestExterior`, which has a navmesh, for an ACHR: an actor in a cell
//!   without one is a `cell.actor-without-navmesh` warning) in `New.esp` with `fields`
//!   giving every typed leaf member of the definition (whole payloads from the schema
//!   layout, then struct fields one by one through `set`'s paths; `NAME`, the base, stays
//!   the one `place` composes, and an ACHR keeps `place`'s `DATA`).
//! - CELL: `new` writes a new interior cell with every typed leaf member, which the writer
//!   puts in the interior block and sub-block its FormID gives; `build`'s `[[new]]` writes
//!   one too.
//!
//! Each written record is read back by `authoring_tests::check_written`: every member placed
//! where the definition puts it, nothing raw, identical re-encoding, the field values, and
//! `validate` with no finding of any rule (`layout.undecoded` included) and only the
//! whole-opaque members unchecked. Each `*_dangling_link_is_reported` test writes FormIDs
//! naming a form `World.esm` lacks into members reference integrity covers, and checks that
//! `validate`'s `formid.dangling` names each and `refs` lists each as `missing`.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::index::{index_bytes, IndexOptions};
use kitkraft_esm::Plugin;
use serde_json::{json, Map, Value};

use super::authoring_tests::{check_written, plan_in, validate_at, Plan};
use super::place_tests::world_master;
use crate::edit::patch_tests::dir;
use crate::execute;

/// The fixture's master.
const MASTER: &str = "World.esm";
/// A form `World.esm` does not have.
const MISSING: &str = "World.esm:000FFF";

/// A plan for `sig` over the world fixture, without `NAME` for a reference (its base is
/// the one `place` writes) and, for an ACHR, without `DATA` (its position is `place`'s,
/// inside the exterior cell's grid: a sample position would leave it, a
/// `ref.outside-block` warning).
fn world_plan(sig: &str, layout: &str, component: &str) -> Plan {
    let mut p = plan_in(MASTER, sig, layout, component);
    let mut drop = |s: &[u8; 4]| {
        let text = String::from_utf8_lossy(s).into_owned();
        p.fields
            .retain(|k, _| k != &text && !k.starts_with(&format!("{text}.")));
        p.edits.retain(|(e, _, _)| e != s);
    };
    if sig != "CELL" {
        drop(b"NAME");
    }
    if sig == "ACHR" {
        drop(b"DATA");
    }
    p
}

/// `place` of `base` in `d/New.esp` with EditorID `edid` and `fields`: a static into
/// `TestInterior`, an NPC into `TestExterior` (grid 1, -1); returns (Data dir, `New.esp`,
/// the result).
fn place_with(
    d: &Path,
    base: &str,
    edid: &str,
    fields: &Map<String, Value>,
) -> (PathBuf, PathBuf, Value) {
    let data = world_master(d);
    let new = d.join("New.esp");
    let mut args = json!({
        "base": base, "edid": edid, "fields": fields, "into": new.display().to_string(),
        "plugins": [MASTER], "data_dir": data.display().to_string(), "no_cache": true,
    });
    if base == "KC_Npc" {
        args["world"] = json!("TestWorld");
        args["position"] = json!([150.0, -50.0, 0.0]);
    } else {
        args["cell"] = json!("TestInterior");
        args["position"] = json!([10.0, 20.0, 30.0]);
    }
    let r = execute("place", args).unwrap_or_else(|e| panic!("place {base}: {e}"));
    // A form the master lacks is a warning only.
    assert_eq!(
        r["data"]["validate"]["errors"],
        json!(0),
        "{:#}",
        r["data"]["validate"]
    );
    (data, new, r)
}

/// `new` of a CELL with `fields` into `d/New.esp`; returns (Data dir, `New.esp`, result).
fn new_cell(d: &Path, edid: &str, fields: &Map<String, Value>) -> (PathBuf, PathBuf, Value) {
    let data = world_master(d);
    let into = d.join("New.esp");
    let r = execute(
        "new",
        json!({
            "type": "CELL", "edid": edid, "fields": fields, "masters": [MASTER],
            "into": into.display().to_string(), "plugins": [MASTER],
            "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap_or_else(|e| panic!("new CELL: {e}"));
    (data, into, r)
}

fn reference_writes_and_reads_back(sig: &str, base: &str, layout: &str, component: &str) {
    let d = dir(&format!("authoring-world-{}", sig.to_lowercase()));
    let p = world_plan(sig, layout, component);
    let (data, new, r) = place_with(&d, base, &format!("KC_New{sig}"), &p.fields);
    assert_eq!(r["data"]["placed"][0]["signature"], json!(sig));
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    check_written(sig, &new, &data, &p);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn refr_place_sets_every_typed_member_and_reads_back() {
    reference_writes_and_reads_back(
        "REFR",
        "KC_Crate",
        "ContactShadow",
        "BGSContactShadowComponent_Component",
    );
}

#[test]
fn achr_place_sets_every_typed_member_and_reads_back() {
    reference_writes_and_reads_back(
        "ACHR",
        "KC_Npc",
        "ContactShadow",
        "BGSContactShadowComponent_Component",
    );
}

#[test]
fn cell_new_sets_every_typed_member_and_reads_back() {
    let d = dir("authoring-world-cell");
    let p = world_plan(
        "CELL",
        "ContactShadow",
        "BGSContactShadowComponent_Component",
    );
    let (data, into, r) = new_cell(&d, "KC_NewCELL", &p.fields);
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    check_written("CELL", &into, &data, &p);
    // The cell sits in the interior block and sub-block its FormID gives (0x1000 = 4096:
    // block 6, sub-block 9), and INCC counts it.
    let bytes = std::fs::read(&into).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let cell = index
        .records
        .iter()
        .find(|r| r.signature.to_string() == "CELL")
        .unwrap();
    assert_eq!(cell.form_id, 0x0100_1000);
    let groups: Vec<(i32, u32)> = index
        .group_path(cell.group)
        .iter()
        .map(|g| (g.header.group_type, g.header.label_u32()))
        .collect();
    assert_eq!(groups[1..], [(2, 6), (3, 9)]);
    let parsed = Plugin::parse(&bytes).unwrap();
    let incc = parsed.header().subrecords().and_then(|subs| {
        subs.iter()
            .find(|s| s.signature().to_string() == "INCC")
            .map(|s| s.data().to_vec())
    });
    assert_eq!(incc, Some(1u32.to_le_bytes().to_vec()));
    let _ = std::fs::remove_dir_all(&d);
}

/// A new cell is interior only: DATA without the Interior flag is refused, nothing written.
#[test]
fn a_new_exterior_cell_is_refused() {
    let d = dir("authoring-world-cell-exterior");
    let data = world_master(&d);
    let into = d.join("New.esp");
    let err = execute(
        "new",
        json!({
            "type": "CELL", "edid": "KC_Outside", "fields": {"DATA": 0},
            "into": into.display().to_string(), "plugins": [MASTER],
            "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("new.cell-exterior"), "{err}");
    assert!(!into.exists());
    // A world stays refused.
    let err = execute(
        "new",
        json!({
            "type": "WRLD", "edid": "KC_World", "into": into.display().to_string(),
            "plugins": [MASTER], "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("new.not-top-level"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `build`'s `[[new]]` writes a new interior cell as `new` does.
#[test]
fn build_new_writes_an_interior_cell() {
    let d = dir("authoring-world-cell-build");
    let data = world_master(&d);
    let spec = d.join("cell.toml");
    std::fs::write(
        &spec,
        format!(
            "version = 3\n\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n\
[[new]]\nname = \"room\"\ntype = \"CELL\"\nedid = \"KC_BuiltRoom\"\nmasters = [\"World.esm\"]\n\
fields = {{ FULL = \"Built Room\", XCLW = 1.5, LTMP = \"World.esm:000800\" }}\n",
            data.display()
        ),
    )
    .unwrap();
    let out = d.join("Room.esm");
    let r = execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": out.display().to_string()}),
    )
    .unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let bytes = std::fs::read(&out).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let cell = index
        .records
        .iter()
        .find(|r| r.signature.to_string() == "CELL")
        .unwrap();
    let groups: Vec<(i32, u32)> = index
        .group_path(cell.group)
        .iter()
        .map(|g| (g.header.group_type, g.header.label_u32()))
        .collect();
    assert_eq!(groups.len(), 3, "{groups:?}");
    assert_eq!(groups[1].0, 2);
    assert_eq!(groups[2].0, 3);
    let v = validate_at(&out, &data);
    assert_eq!(v["findings"], json!([]), "{v:#}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `validate`'s `formid.dangling` names each of `expect` (member paths) in `written`, and
/// `refs` on `edid` lists at least as many missing links.
fn dangling_found(written: &Path, data: &Path, plugins: Value, edid: &str, expect: &[&str]) {
    let v = validate_at(written, data);
    let findings: Vec<&Value> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule"] == "formid.dangling")
        .collect();
    for want in expect {
        assert!(
            findings.iter().any(
                |f| f["subrecord"].as_str().is_some_and(|s| s.contains(want))
                    && f["message"].as_str().is_some_and(|m| m.contains("000FFF"))
            ),
            "{edid}: no formid.dangling for {want}: {findings:#?}"
        );
    }
    let refs = execute(
        "refs",
        json!({
            "form": edid, "plugins": plugins,
            "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap();
    let missing: Vec<&str> = refs["data"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["status"] == "missing")
        .filter_map(|e| e["path"].as_str())
        .collect();
    assert!(
        missing.len() >= expect.len(),
        "{edid}: refs lists {missing:?} as missing, expected {expect:?}"
    );
}

fn reference_dangling(sig: &str, base: &str) {
    let d = dir(&format!("authoring-world-dangling-{}", sig.to_lowercase()));
    let mut p = world_plan(sig, "ContactShadow", "BGSContactShadowComponent_Component");
    for k in ["XLCN", "XEZN", "XLYR"] {
        p.fields.insert(k.to_string(), json!(MISSING));
    }
    let edid = format!("KC_Dangling{sig}");
    let (data, new, _) = place_with(&d, base, &edid, &p.fields);
    let plugins = json!([MASTER, new.display().to_string()]);
    dangling_found(&new, &data, plugins, &edid, &["XLCN", "XEZN", "XLYR"]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn refr_dangling_link_is_reported() {
    reference_dangling("REFR", "KC_Crate");
}

#[test]
fn achr_dangling_link_is_reported() {
    reference_dangling("ACHR", "KC_Npc");
}

#[test]
fn cell_dangling_link_is_reported() {
    let d = dir("authoring-world-dangling-cell");
    let mut p = world_plan(
        "CELL",
        "ContactShadow",
        "BGSContactShadowComponent_Component",
    );
    for k in ["LTMP", "XLCN", "XCWT"] {
        p.fields.insert(k.to_string(), json!(MISSING));
    }
    let (data, into, r) = new_cell(&d, "KC_DanglingCELL", &p.fields);
    // A form the master lacks is a warning: written, and reported.
    assert_eq!(r["data"]["validate"]["errors"], json!(0));
    let plugins = json!([MASTER, into.display().to_string()]);
    dangling_found(
        &into,
        &data,
        plugins,
        "KC_DanglingCELL",
        &["LTMP", "XLCN", "XCWT"],
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// The Fable review of wave 3, finding 1: `place` with `fields` naming a group member
/// whose group lead the reference lacks (`XPDO` DecalRefs without the `XPDD` DecalData
/// that starts the ProjectedDecal group) is refused and writes nothing; with the lead in
/// the fields too, both are written (the lead first) and `validate` passes with no
/// `layout.unplaced` finding.
#[test]
fn place_refuses_a_group_member_without_its_lead() {
    let d = dir("authoring-world-group-lead");
    let data = world_master(&d);
    let new = d.join("New.esp");
    let args = |fields: Value| {
        json!({
            "base": "KC_Crate", "edid": "KC_Decal", "fields": fields,
            "into": new.display().to_string(), "cell": "TestInterior",
            "position": [10.0, 20.0, 30.0], "plugins": [MASTER],
            "data_dir": data.display().to_string(), "no_cache": true,
        })
    };
    let err = execute("place", args(json!({"XPDO": "hex:00000000"})))
        .unwrap_err()
        .to_string();
    assert!(err.contains("XPDO") && err.contains("XPDD"), "{err}");
    assert!(!new.exists(), "a refused place writes nothing");
    let decal = format!("hex:{}", "00".repeat(16));
    let r = execute(
        "place",
        args(json!({"XPDD": decal, "XPDO": "hex:00000000"})),
    )
    .unwrap_or_else(|e| panic!("place with XPDD and XPDO: {e}"));
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    let v = validate_at(&new, &data);
    assert_eq!(v["errors"], json!(0), "{v:#}");
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["rule"] != "layout.unplaced"),
        "{v:#}"
    );
    // The lead comes first, at its own slot.
    let bytes = std::fs::read(&new).unwrap();
    let at = |s: &[u8]| bytes.windows(4).position(|w| w == s).unwrap();
    assert!(at(b"XPDD") < at(b"XPDO"));
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review of the wave 3 fix round, follow-up (b): `place` with fields that start a
/// REFR `MapMarker` (its lead `XMRK`, and `FNAM`) writes the members the group requires
/// that the fields do not give (`FULL`, `TNAM`, `VNAM`) as zeros, at their slots, names
/// them in `limitations`, and `validate` passes with no `layout.group-incomplete`.
#[test]
fn place_fills_the_required_members_of_a_started_group() {
    let d = dir("authoring-world-group-fill");
    let data = world_master(&d);
    let new = d.join("New.esp");
    let r = execute(
        "place",
        json!({
            "base": "KC_Crate", "edid": "KC_Marker",
            "fields": {"FNAM": "hex:01000000", "XMRK": "hex:"},
            "into": new.display().to_string(), "cell": "TestInterior",
            "position": [10.0, 20.0, 30.0], "plugins": [MASTER],
            "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap_or_else(|e| panic!("place with XMRK and FNAM: {e}"));
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    let limits = r["limitations"].to_string();
    assert!(
        limits.contains("TNAM (MapMarker.MarkerType)") && limits.contains("VNAM"),
        "{limits}"
    );
    let v = validate_at(&new, &data);
    assert_eq!(v["errors"], json!(0), "{v:#}");
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["rule"] != "layout.group-incomplete" && f["rule"] != "layout.unplaced"),
        "{v:#}"
    );
    // The group in definition order: XMRK FNAM FULL TNAM VNAM, the fields' FNAM kept.
    let bytes = std::fs::read(&new).unwrap();
    let at = |s: &[u8]| bytes.windows(4).rposition(|w| w == s).unwrap();
    let order = [
        at(b"XMRK"),
        at(b"FNAM"),
        at(b"FULL"),
        at(b"TNAM"),
        at(b"VNAM"),
    ];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    let fnam = at(b"FNAM");
    assert_eq!(
        bytes.get(fnam + 4..fnam + 10),
        Some(&[4, 0, 1, 0, 0, 0][..])
    );
    let tnam = at(b"TNAM");
    assert_eq!(bytes.get(tnam + 4..tnam + 8), Some(&[2, 0, 0, 0][..]));
    let _ = std::fs::remove_dir_all(&d);
}
