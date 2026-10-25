//! The cell rules (M4.E4) through `execute`: a positive and a negative synthetic fixture
//! per rule, on the synthetic world master of the `place` tests (an interior cell, a world
//! with a persistent cell and an exterior cell at grid (1, -1) holding a navmesh).

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_esm::build::HeaderSpec;
use kitkraft_esm::cells::{
    self, grid_label, CellPlace, Grid, Parent, PlacementStyle, FLAG_PERSISTENT,
};
use kitkraft_esm::tes4::FLAG_MASTER;
use kitkraft_esm::{GroupHeader, Plugin, Record, Signature, Subrecord};
use serde_json::{json, Value};

use crate::edit::patch_tests::dir;
use crate::execute;
use crate::handlers::place_tests::{
    data24, rec, sub, world_master, zstr, EXTERIOR_CELL, INTERIOR_CELL, NAVMESH, PERSISTENT_CELL,
    WORLD,
};

const RULES: [&str; 9] = super::cells::RULES;

/// A patch of `World.esm` in `data`, each record at its path; `INCC` as given.
/// Puts `record` under the groups `path`, creating them, with no placement rules.
fn force(
    entries: &mut Vec<kitkraft_esm::Entry<'static>>,
    path: &[GroupHeader],
    record: Record<'static>,
) {
    use kitkraft_esm::{Entry, Group};
    let Some((g, rest)) = path.split_first() else {
        entries.push(Entry::Record(record));
        return;
    };
    let i = match entries
        .iter()
        .position(|e| matches!(e, Entry::Group(x) if x.header.same_group(g)))
    {
        Some(i) => i,
        None => {
            entries.push(Entry::Group(Group::new(*g)));
            entries.len() - 1
        }
    };
    if let Entry::Group(x) = &mut entries[i] {
        force(x.children_mut(), rest, record);
    }
}

fn patch(
    data: &Path,
    name: &str,
    records: Vec<(Record<'static>, Vec<GroupHeader>)>,
    incc: Option<u32>,
) -> PathBuf {
    let mut header = HeaderSpec::new(vec![b"World.esm".to_vec()]).to_record();
    if let Some(n) = incc {
        header.subrecords_mut().unwrap().push(Subrecord::new(
            Signature(*b"INCC"),
            n.to_le_bytes().to_vec(),
        ));
    }
    let mut plugin = Plugin::new(header);
    for (record, path) in records {
        // The builder refuses the shapes the negative fixtures need (a children group
        // without its record, a flag against its group): those are put in by hand.
        if let Err(e) = plugin.upsert(&path, record.clone()) {
            assert!(matches!(e, kitkraft_esm::Error::Placement { .. }), "{e}");
            force(plugin.entries_mut(), &path, record);
        }
    }
    let file = data.join(name);
    std::fs::write(&file, plugin.to_bytes().unwrap()).unwrap();
    file
}

fn at(record: &Record<'_>, parent: Parent) -> Vec<GroupHeader> {
    cells::placement(&record.header, &parent, &PlacementStyle::OFFICIAL).unwrap()
}

fn interior() -> Parent {
    Parent::Cell {
        place: CellPlace::INTERIOR,
        cell: INTERIOR_CELL,
    }
}

fn exterior() -> CellPlace {
    CellPlace::Exterior {
        world: WORLD,
        grid: Grid::new(1, -1),
    }
}

/// The master's interior cell, carried (its EDID only: a copy that drops its FULL).
fn carried_cell() -> (Record<'static>, Vec<GroupHeader>) {
    let cell = rec(
        b"CELL",
        0,
        INTERIOR_CELL,
        vec![
            sub(b"EDID", &zstr("TestInterior")),
            sub(b"FULL", &zstr("Test Room")),
        ],
    );
    let path = at(&cell, Parent::CellAt(CellPlace::INTERIOR));
    (cell, path)
}

fn refr(form_id: u32, flags: u32, base: u32, pos: [f32; 3]) -> Record<'static> {
    rec(
        b"REFR",
        flags,
        form_id,
        vec![
            sub(b"NAME", &base.to_le_bytes()),
            sub(b"DATA", &data24(pos)),
        ],
    )
}

/// A location (`LCTN`) whose `ACPR` names `refs`: one 16-byte entry per FormID
/// (`Ref`, `WorldOrCell`, `GridY`, `GridX`, 4 bytes).
fn lctn_acpr(form_id: u32, refs: &[u32]) -> Record<'static> {
    let mut bytes = Vec::new();
    for &raw in refs {
        bytes.extend_from_slice(&raw.to_le_bytes());
        bytes.extend_from_slice(&INTERIOR_CELL.to_le_bytes());
        bytes.extend_from_slice(&0i16.to_le_bytes());
        bytes.extend_from_slice(&0i16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
    }
    rec(
        b"LCTN",
        0,
        form_id,
        vec![sub(b"EDID", &zstr("TestLocation")), sub(b"ACPR", &bytes)],
    )
}

fn check(plugin: &Path) -> Value {
    execute(
        "validate",
        json!({"plugin": plugin.display().to_string(), "rules": RULES}),
    )
    .unwrap()["data"]
        .clone()
}

/// `(rule, severity, record)` of every finding.
fn found(data: &Value) -> Vec<(String, String, String)> {
    data["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["rule"].as_str().unwrap().to_string(),
                f["severity"].as_str().unwrap().to_string(),
                f["record"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn one(rule: &str, severity: &str, record: &str) -> Vec<(String, String, String)> {
    vec![(rule.into(), severity.into(), record.into())]
}

#[test]
fn a_placed_reference_and_the_master_pass_every_cell_rule() {
    let d = dir("validate-cells-clean");
    let data = world_master(&d);
    let master = check(&data.join("World.esm"));
    assert_eq!(found(&master), [], "{master:#}");
    assert_eq!(master["rules"].as_array().unwrap().len(), 9);
    let (cell, path) = carried_cell();
    let ok = patch(
        &data,
        "Ok.esm",
        vec![
            (cell, path),
            (
                refr(0x0100_0800, 0, 0x800, [1.0, 2.0, 3.0]),
                at(&refr(0x0100_0800, 0, 0, [0.0; 3]), interior()),
            ),
        ],
        Some(1),
    );
    assert_eq!(found(&check(&ok)), []);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn missing_parents_and_orphans() {
    let d = dir("validate-cells-parent");
    let data = world_master(&d);
    let r = refr(0x0100_0800, 0, 0x800, [0.0; 3]);
    // The master's cell not carried: a warning; a cell nobody has: an error.
    let p = patch(
        &data,
        "NoCarry.esm",
        vec![(r.clone(), at(&r, interior()))],
        Some(0),
    );
    assert_eq!(
        found(&check(&p)),
        one("cell.missing-parent", "warning", "CELL:00000900")
    );
    let nobody = Parent::Cell {
        place: CellPlace::INTERIOR,
        cell: 0x0000_0999,
    };
    let p = patch(
        &data,
        "NoCell.esm",
        vec![(r.clone(), at(&r, nobody))],
        Some(0),
    );
    assert_eq!(
        found(&check(&p)),
        one("cell.missing-parent", "error", "CELL:00000999")
    );
    // A REFR in a top-level group.
    let p = patch(
        &data,
        "Top.esm",
        vec![(r.clone(), vec![GroupHeader::top(Signature(*b"REFR"))])],
        Some(0),
    );
    assert_eq!(
        found(&check(&p)),
        one("cell.missing-parent", "error", "REFR:01000800")
    );
    // ref.orphan: a base nobody has; no NAME at all.
    let (cell, path) = carried_cell();
    let orphan = refr(0x0100_0801, 0, 0x0000_0999, [0.0; 3]);
    let nameless = rec(
        b"REFR",
        0,
        0x0100_0802,
        vec![sub(b"DATA", &data24([0.0; 3]))],
    );
    let p = patch(
        &data,
        "Orphan.esm",
        vec![
            (cell, path),
            (orphan.clone(), at(&orphan, interior())),
            (nameless.clone(), at(&nameless, interior())),
        ],
        Some(1),
    );
    let mut got = found(&check(&p));
    got.sort();
    assert_eq!(
        got,
        [
            ("ref.orphan".into(), "error".into(), "REFR:01000801".into()),
            ("ref.orphan".into(), "error".into(), "REFR:01000802".into()),
        ]
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn blocks_grids_and_persistent_flags() {
    let d = dir("validate-cells-blocks");
    let data = world_master(&d);
    // An own interior cell (object ID 0x905 = 2309: block 9, sub-block 0) under block 3.
    let cell = rec(
        b"CELL",
        0,
        0x0100_0905,
        vec![sub(b"EDID", &zstr("KC_Room"))],
    );
    let wrong = vec![
        GroupHeader::top(Signature(*b"CELL")),
        PlacementStyle::OFFICIAL.header(3u32.to_le_bytes(), 2),
        PlacementStyle::OFFICIAL.header(0u32.to_le_bytes(), 3),
    ];
    let p = patch(&data, "Block.esm", vec![(cell.clone(), wrong)], Some(1));
    assert_eq!(
        found(&check(&p)),
        one("ref.outside-block", "error", "CELL:01000905")
    );
    let right = at(&cell, Parent::CellAt(CellPlace::INTERIOR));
    let p = patch(&data, "BlockOk.esm", vec![(cell, right)], Some(1));
    assert_eq!(found(&check(&p)), []);
    // An exterior cell whose XCLC says (9, 9) in the (1, -1) sub-block.
    let xclc = [9i32.to_le_bytes(), 9i32.to_le_bytes(), [0u8; 4]].concat();
    let ext = rec(b"CELL", 0, 0x0100_0906, vec![sub(b"XCLC", &xclc)]);
    let path = at(&ext, Parent::CellAt(exterior()));
    let wrld = rec(b"WRLD", 0, WORLD, vec![sub(b"EDID", &zstr("TestWorld"))]);
    let p = patch(
        &data,
        "Grid.esm",
        vec![
            (wrld.clone(), vec![GroupHeader::top(Signature(*b"WRLD"))]),
            (ext, path),
        ],
        Some(0),
    );
    assert_eq!(
        found(&check(&p)),
        one("ref.outside-block", "error", "CELL:01000906")
    );
    // An exterior reference outside its cell's square: a warning.
    let xcell = rec(
        b"CELL",
        0,
        EXTERIOR_CELL,
        vec![
            sub(b"EDID", &zstr("TestExterior")),
            sub(
                b"XCLC",
                &[1i32.to_le_bytes(), (-1i32).to_le_bytes(), [0u8; 4]].concat(),
            ),
        ],
    );
    let xpath = at(&xcell, Parent::CellAt(exterior()));
    let inside = refr(0x0100_0807, 0, 0x800, [150.0, -50.0, 0.0]);
    let outside = refr(0x0100_0808, 0, 0x800, [950.0, -50.0, 0.0]);
    let child = Parent::Cell {
        place: exterior(),
        cell: EXTERIOR_CELL,
    };
    let p = patch(
        &data,
        "Outside.esm",
        vec![
            (wrld.clone(), vec![GroupHeader::top(Signature(*b"WRLD"))]),
            (xcell, xpath),
            (inside.clone(), at(&inside, child)),
            (outside.clone(), at(&outside, child)),
        ],
        Some(0),
    );
    assert_eq!(
        found(&check(&p)),
        one("ref.outside-block", "warning", "REFR:01000808")
    );
    assert_eq!(grid_label(Grid::new(1, -1)).unwrap(), [0xFF, 0xFF, 1, 0]);
    // ref.persistent-flag: 0x400 in temporary children, and missing in persistent ones.
    let (cell, path) = carried_cell();
    let flagged = refr(0x0100_0809, FLAG_PERSISTENT, 0x800, [0.0; 3]);
    let temp_path = at(&refr(0x0100_0809, 0, 0, [0.0; 3]), interior());
    let unflagged = refr(0x0100_080A, 0, 0x800, [0.0; 3]);
    let pers_path = at(&refr(0x0100_080A, FLAG_PERSISTENT, 0, [0.0; 3]), interior());
    let p = patch(
        &data,
        "Flags.esm",
        vec![(cell, path), (flagged, temp_path), (unflagged, pers_path)],
        Some(1),
    );
    let mut got = found(&check(&p));
    got.sort();
    assert_eq!(
        got,
        [
            (
                "ref.persistent-flag".into(),
                "error".into(),
                "REFR:01000809".into()
            ),
            (
                "ref.persistent-flag".into(),
                "error".into(),
                "REFR:0100080A".into()
            ),
        ]
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn persistence_navmesh_and_incc() {
    let d = dir("validate-cells-misc");
    let data = world_master(&d);
    // ref.persistence: a door linked (XTEL) to a temporary reference.
    let (cell, path) = carried_cell();
    let mut door = refr(0x0100_0810, 0, 0x800, [0.0; 3]);
    door.subrecords_mut().unwrap().push(sub(
        b"XTEL",
        &[0x11, 0x08, 0x00, 0x01]
            .iter()
            .copied()
            .chain([0u8; 28])
            .collect::<Vec<_>>(),
    ));
    let target = refr(0x0100_0811, 0, 0x800, [0.0; 3]);
    let p = patch(
        &data,
        "Door.esm",
        vec![
            (cell.clone(), path.clone()),
            (door.clone(), at(&door, interior())),
            (target.clone(), at(&target, interior())),
        ],
        Some(1),
    );
    assert_eq!(
        found(&check(&p)),
        one("ref.persistence", "warning", "REFR:01000811")
    );
    let pers = refr(0x0100_0811, FLAG_PERSISTENT, 0x800, [0.0; 3]);
    let p = patch(
        &data,
        "DoorOk.esm",
        vec![
            (cell.clone(), path.clone()),
            (door.clone(), at(&door, interior())),
            (pers.clone(), at(&pers, interior())),
        ],
        Some(1),
    );
    assert_eq!(found(&check(&p)), []);
    // cell.actor-without-navmesh: an actor in the interior cell (no navmesh anywhere), and
    // one in the exterior cell (the master's navmesh).
    let achr = rec(
        b"ACHR",
        0,
        0x0100_0812,
        vec![
            sub(b"NAME", &0x801u32.to_le_bytes()),
            sub(b"DATA", &data24([0.0; 3])),
        ],
    );
    let p = patch(
        &data,
        "Actor.esm",
        vec![
            (cell.clone(), path.clone()),
            (achr.clone(), at(&achr, interior())),
        ],
        Some(1),
    );
    assert_eq!(
        found(&check(&p)),
        one("cell.actor-without-navmesh", "warning", "ACHR:01000812")
    );
    let xcell = rec(
        b"CELL",
        0,
        EXTERIOR_CELL,
        vec![
            sub(b"EDID", &zstr("TestExterior")),
            sub(
                b"XCLC",
                &[1i32.to_le_bytes(), (-1i32).to_le_bytes(), [0u8; 4]].concat(),
            ),
        ],
    );
    let wrld = rec(b"WRLD", 0, WORLD, vec![sub(b"EDID", &zstr("TestWorld"))]);
    let child = Parent::Cell {
        place: exterior(),
        cell: EXTERIOR_CELL,
    };
    let mut outdoor = achr.clone();
    outdoor.subrecords_mut().unwrap()[1] = sub(b"DATA", &data24([150.0, -50.0, 0.0]));
    let p = patch(
        &data,
        "ActorOk.esm",
        vec![
            (wrld.clone(), vec![GroupHeader::top(Signature(*b"WRLD"))]),
            (xcell.clone(), at(&xcell, Parent::CellAt(exterior()))),
            (outdoor.clone(), at(&outdoor, child)),
        ],
        Some(0),
    );
    assert_eq!(found(&check(&p)), []);
    // navmesh.unchanged: the master's navmesh copied as is (a warning), or changed (none).
    let same = rec(b"NAVM", 0, NAVMESH, vec![sub(b"NVNM", &[5u8; 12])]);
    let changed = rec(b"NAVM", 0, NAVMESH, vec![sub(b"NVNM", &[6u8; 12])]);
    for (nav, want) in [
        (same, one("navmesh.unchanged", "warning", "NAVM:00000710")),
        (changed, vec![]),
    ] {
        let p = patch(
            &data,
            "Nav.esm",
            vec![
                (wrld.clone(), vec![GroupHeader::top(Signature(*b"WRLD"))]),
                (xcell.clone(), at(&xcell, Parent::CellAt(exterior()))),
                (nav.clone(), at(&nav, child)),
            ],
            Some(0),
        );
        assert_eq!(found(&check(&p)), want);
    }
    // cell.incc: wrong (an error), absent with an interior cell (a warning).
    let p = patch(
        &data,
        "Incc.esm",
        vec![(cell.clone(), path.clone())],
        Some(2),
    );
    assert_eq!(
        found(&check(&p)),
        one("cell.incc", "error", "TES4:00000000")
    );
    let p = patch(&data, "NoIncc.esm", vec![(cell, path)], None);
    assert_eq!(
        found(&check(&p)),
        one("cell.incc", "warning", "TES4:00000000")
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review (lane E fix round, finding 3): an actor in a world's persistent cell is
/// checked against the exterior cell its position names: a master's with a navmesh, no
/// finding; a master's without one, a warning; a position no exterior cell holds, or a
/// cell of the plugin's own (not checked, as for the rule's other actors), a limitation.
#[test]
fn a_persistent_world_actor_is_checked_in_the_exterior_cell_it_stands_in() {
    let d = dir("validate-cells-world-actor");
    let data = world_master(&d);
    let pplace = CellPlace::Persistent { world: WORLD };
    let actor = |pos: [f32; 3], base: u32| {
        rec(
            b"ACHR",
            FLAG_PERSISTENT,
            0x0100_0830,
            vec![
                sub(b"NAME", &base.to_le_bytes()),
                sub(b"DATA", &data24(pos)),
            ],
        )
    };
    let world_parents = |world: u32, pcell: u32, place: CellPlace| {
        let wrld = rec(b"WRLD", 0, world, vec![]);
        let cell = rec(b"CELL", FLAG_PERSISTENT, pcell, vec![sub(b"DATA", &[0, 0])]);
        let path = at(&cell, Parent::CellAt(place));
        vec![
            (wrld, vec![GroupHeader::top(Signature(*b"WRLD"))]),
            (cell, path),
        ]
    };
    let in_pcell = Parent::Cell {
        place: pplace,
        cell: PERSISTENT_CELL,
    };
    let validate = |p: &Path| {
        execute(
            "validate",
            json!({"plugin": p.display().to_string(), "rules": RULES}),
        )
        .unwrap()
    };
    let run = |name: &str, extra: Vec<(Record<'static>, Vec<GroupHeader>)>| {
        let mut records = world_parents(WORLD, PERSISTENT_CELL, pplace);
        records.extend(extra);
        validate(&patch(&data, name, records, Some(0)))
    };
    let limited = |r: &Value, what: &str| {
        r["limitations"].as_array().unwrap().iter().any(|l| {
            let l = l.as_str().unwrap();
            l.starts_with("cell.actor-without-navmesh: 1 actor(s)") && l.contains(what)
        })
    };
    // At grid (1, -1): the master's exterior cell 702, which holds a navmesh. Before the
    // fix this was a warning on the persistent cell (the reviewer's false positive).
    let a = actor([150.0, -50.0, 0.0], 0x801);
    let r = run("WorldActor.esm", vec![(a.clone(), at(&a, in_pcell))]);
    assert_eq!(found(&r["data"]), [], "{:#}", r["data"]);
    assert!(!limited(&r, ""));
    // At grid (2, 2): the plugin's own exterior cell, without a navmesh: a limitation.
    let grid = Grid::new(2, 2);
    let own = rec(
        b"CELL",
        0,
        0x0100_0820,
        vec![sub(
            b"XCLC",
            &[2i32.to_le_bytes(), 2i32.to_le_bytes(), [0u8; 4]].concat(),
        )],
    );
    let a = actor([250.0, 250.0, 0.0], 0x801);
    let r = run(
        "WorldActorOwn.esm",
        vec![
            (
                own.clone(),
                at(
                    &own,
                    Parent::CellAt(CellPlace::Exterior { world: WORLD, grid }),
                ),
            ),
            (a.clone(), at(&a, in_pcell)),
        ],
    );
    assert_eq!(found(&r["data"]), []);
    assert!(
        limited(&r, "01000820 is this plugin's own"),
        "{:#}",
        r["limitations"]
    );
    // At grid (9, 9): no exterior cell anywhere: a limitation, no warning.
    let a = actor([950.0, 950.0, 0.0], 0x801);
    let r = run("WorldActorNowhere.esm", vec![(a.clone(), at(&a, in_pcell))]);
    assert_eq!(found(&r["data"]), []);
    assert!(
        limited(&r, "no exterior cell at grid (9, 9)"),
        "{:#}",
        r["limitations"]
    );
    // A master whose exterior cell at grid (3, 3) has no navmesh: a warning naming it.
    let mut spec = HeaderSpec::new(Vec::new());
    spec.flags = FLAG_MASTER;
    let mut land = Plugin::create(&spec);
    let style = PlacementStyle::OFFICIAL;
    let lworld = 0x0000_0900;
    let lplace = CellPlace::Persistent { world: lworld };
    let lgrid = Grid::new(3, 3);
    for (record, parent) in [
        (rec(b"NPC_", 0, 0x800, vec![]), Parent::None),
        (rec(b"WRLD", 0, lworld, vec![]), Parent::None),
        (
            rec(b"CELL", FLAG_PERSISTENT, 0x901, vec![sub(b"DATA", &[0, 0])]),
            Parent::CellAt(lplace),
        ),
        (
            rec(
                b"CELL",
                0,
                0x902,
                vec![sub(
                    b"XCLC",
                    &[3i32.to_le_bytes(), 3i32.to_le_bytes(), [0u8; 4]].concat(),
                )],
            ),
            Parent::CellAt(CellPlace::Exterior {
                world: lworld,
                grid: lgrid,
            }),
        ),
    ] {
        let path = cells::placement(&record.header, &parent, &style).unwrap();
        land.upsert_styled(&path, record, &style).unwrap();
    }
    std::fs::write(data.join("Land.esm"), land.to_bytes().unwrap()).unwrap();
    let mut plugin = Plugin::new(HeaderSpec::new(vec![b"Land.esm".to_vec()]).to_record());
    let a = actor([350.0, 350.0, 0.0], 0x800);
    let mut records = world_parents(lworld, 0x901, lplace);
    records.push((
        a.clone(),
        at(
            &a,
            Parent::Cell {
                place: lplace,
                cell: 0x901,
            },
        ),
    ));
    for (record, path) in records {
        plugin.upsert(&path, record).unwrap();
    }
    let p = data.join("LandActor.esm");
    std::fs::write(&p, plugin.to_bytes().unwrap()).unwrap();
    let r = validate(&p);
    assert_eq!(
        found(&r["data"]),
        one("cell.actor-without-navmesh", "warning", "ACHR:01000830"),
        "{:#}",
        r["data"]
    );
    assert!(r["data"]["findings"][0]["message"]
        .as_str()
        .unwrap()
        .contains("exterior cell 00000902 (grid (3, 3)"));
    let _ = std::fs::remove_dir_all(&d);
}

/// M5.D2: an `ACPR` (added persistent refs) entry of a location (`LCTN`) that names a
/// reference the plugin holds in temporary children (group type 9) is a warning; one
/// that names a persistent reference is clean; one that names a master's reference is
/// counted as a limitation.
#[test]
fn acpr_naming_a_temporary_reference_is_a_warning() {
    let d = dir("validate-cells-acpr-temp");
    let data = world_master(&d);
    let (cell, path) = carried_cell();
    let refr = refr(0x0100_0800, 0, 0x800, [1.0, 2.0, 3.0]);
    let lctn = lctn_acpr(0x0100_0840, &[0x0100_0800]);
    let p = patch(
        &data,
        "TempRef.esm",
        vec![
            (cell, path),
            (refr.clone(), at(&refr, interior())),
            (lctn, vec![GroupHeader::top(Signature(*b"LCTN"))]),
        ],
        Some(1),
    );
    let r = check(&p);
    assert_eq!(
        found(&r),
        one("location.added-ref-temporary", "warning", "LCTN:01000840"),
        "{:#}",
        r
    );
    assert!(r["findings"][0]["message"]
        .as_str()
        .unwrap()
        .contains("ACPR entry 0 names REFR:01000800"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn acpr_naming_a_persistent_reference_is_clean() {
    let d = dir("validate-cells-acpr-persistent");
    let data = world_master(&d);
    let (cell, path) = carried_cell();
    let refr = refr(0x0100_0800, FLAG_PERSISTENT, 0x800, [1.0, 2.0, 3.0]);
    let lctn = lctn_acpr(0x0100_0840, &[0x0100_0800]);
    let p = patch(
        &data,
        "PersistRef.esm",
        vec![
            (cell, path),
            (refr.clone(), at(&refr, interior())),
            (lctn, vec![GroupHeader::top(Signature(*b"LCTN"))]),
        ],
        Some(1),
    );
    assert_eq!(found(&check(&p)), []);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn acpr_naming_a_master_reference_is_a_limitation() {
    let d = dir("validate-cells-acpr-master");
    let data = world_master(&d);
    let (cell, path) = carried_cell();
    // 0x00000901 is the master's reference in its interior cell.
    let lctn = lctn_acpr(0x0100_0840, &[0x0000_0901]);
    let p = patch(
        &data,
        "MasterRef.esm",
        vec![
            (cell, path),
            (lctn, vec![GroupHeader::top(Signature(*b"LCTN"))]),
        ],
        Some(1),
    );
    let r = execute(
        "validate",
        json!({"plugin": p.display().to_string(), "rules": RULES}),
    )
    .unwrap();
    assert_eq!(found(&r["data"]), [], "{:#}", r["data"]);
    assert!(
        r["limitations"].as_array().unwrap().iter().any(|l| {
            l.as_str()
                .unwrap()
                .starts_with("location.added-ref-temporary: 1 ACPR entries name master references")
        }),
        "{:#}",
        r["limitations"]
    );
    let _ = std::fs::remove_dir_all(&d);
}
