//! `probes.toml` (M4.T2) through `execute("testplan")` on the five plugin shapes the
//! ticket names, built synthetically: the rename (a WEAP override with an inline `FULL`),
//! the first POI (a new persistent reference with an EditorID in an interior cell), the
//! ownership recipe (a `CELL` override only), the actors recipe (a new `ACHR` in an
//! exterior), and the overhaul (several WEAP overrides). Plus the template mapping of
//! `docs/game-sessions.md` against `docs/game-observations.md`.

#![allow(clippy::arithmetic_side_effects)]

use std::path::Path;

use kitkraft_game::probes::{Matcher, ProbeFile};
use serde_json::{json, Value};

use crate::edit::patch_tests::dir;
use crate::execute;
use crate::handlers::place_tests::world_master;
use crate::handlers::set_tests::data as weapons_data;

fn testplan(data: &Path, plugin: &Path, out: &Path, plugins: Value) -> Value {
    execute(
        "testplan",
        json!({
            "plugin": plugin.display().to_string(), "out": out.display().to_string(),
            "plugins": plugins, "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap()
}

fn read(out: &Path) -> (ProbeFile, String) {
    let text = std::fs::read_to_string(out.join("probes.toml")).unwrap();
    (ProbeFile::parse(&text).unwrap(), text)
}

/// The checklist's step numbers, in order.
fn checklist_steps(out: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(out.join("checklist.md")).unwrap();
    let after = text.split("## Steps").nth(1).unwrap();
    let steps = after.split("## Report").next().unwrap();
    steps
        .lines()
        .filter_map(|l| {
            let (n, rest) = l.split_once(". ")?;
            (n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() && !rest.is_empty())
                .then(|| format!("T-{:02}", n.parse::<u32>().unwrap()))
        })
        .collect()
}

fn rows(p: &ProbeFile) -> Vec<String> {
    p.probes.iter().map(|x| x.row.clone()).collect()
}

fn commands(p: &ProbeFile, row: &str) -> Vec<String> {
    p.probes
        .iter()
        .find(|x| x.row == row)
        .unwrap()
        .steps
        .iter()
        .map(|s| s.command.clone())
        .collect()
}

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

fn set_full(data: &Path, form: &str, name: &str, into: &Path) {
    execute(
        "set",
        json!({
            "form": form, "fields": {"FULL": name}, "into": into.display().to_string(),
            "plugins": ["Source.esm"], "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap();
}

#[test]
fn the_first_poi_shape_gets_coc_moveto_help_getpos_and_persistence() {
    let d = dir("probes-poi");
    let data = world_master(&d);
    let plugin = d.join("KC_POI.esm");
    place(
        &data,
        &plugin,
        json!({"cell": "TestInterior", "edid": "KC_TestCrate", "persistent": true}),
    );
    let out = d.join("plan");
    let r = testplan(&data, &plugin, &out, json!(["World.esm"]));
    let (p, text) = read(&out);
    assert_eq!(
        rows(&p),
        checklist_steps(&out),
        "one probe per checklist step"
    );
    assert_eq!(rows(&p), ["T-01", "T-02", "T-03", "T-04", "T-05", "T-06"]);
    assert_eq!(r["data"]["probes"]["rows"], json!(6));
    assert_eq!(r["data"]["probes"]["manual"], json!(0));
    assert_eq!(p.sha256, r["data"]["sha256"].as_str().unwrap());
    assert_eq!(p.profile, "KitKraft Dev");
    // References and cells by runtime ID (game-20261101-T1: the console takes neither's
    // EditorID there); the EditorIDs stay in titles and forms.
    assert_eq!(
        commands(&p, "T-01"),
        ["coc TestInterior", "player.getincell 00000900"]
    );
    assert_eq!(
        p.probes[0].forms,
        ["World.esm:000900", "TestInterior", "00000900"]
    );
    assert_eq!(
        commands(&p, "T-02"),
        [
            "player.moveto 01001000",
            "prid 01001000",
            "getdistance player"
        ]
    );
    assert!(p.probes[1].title.contains("`KC_TestCrate`"));
    assert!(p.probes[1].forms.contains(&"KC_TestCrate".to_string()));
    // A wait after each move, none after a getter.
    let waits = |row: usize| -> Vec<Option<u64>> {
        p.probes[row].steps.iter().map(|s| s.wait_secs).collect()
    };
    assert_eq!(waits(0), [Some(10), None]);
    assert_eq!(waits(1), [Some(10), None, None]);
    // The click step: the runtime ID selects a reference of the plugin's base (by ID).
    assert_eq!(commands(&p, "T-03"), ["prid 01001000", "getisid 00000800"]);
    let t3 = &p.probes[2];
    assert_eq!(t3.steps[1].matcher, Some(Matcher::Regex));
    assert!(t3.title.contains("KC_Crate"), "{}", t3.title);
    // prid, getpos against DATA; screenshot; no isdisabled (not a Starfield command).
    let t4 = &p.probes[3];
    assert!(t4.screenshot);
    assert_eq!(
        commands(&p, "T-04"),
        ["prid 01001000", "getpos x", "getpos y", "getpos z"]
    );
    let expected: Vec<Option<&str>> = t4.steps.iter().map(|s| s.expected.as_deref()).collect();
    assert_eq!(expected[1..4], [Some("10"), Some("20"), Some("30")]);
    assert_eq!(t4.steps[1].tolerance, Some(0.5));
    assert!(!text.contains("isdisabled ="), "{text}");
    assert!(!text.contains("command = \"isdisabled\""), "{text}");
    // Persistence: leave, the session's own save, load it, then return and look again. The
    // synthetic load order has no CityNewAtlantisLodgeInt, so no getincell by its ID, and
    // the row says it does not prove the reload.
    assert_eq!(
        commands(&p, "T-05"),
        [
            "coc CityNewAtlantisLodgeInt",
            "save {save}",
            "load {save}",
            "player.getpos x"
        ]
    );
    // A wait after the save too (2026-11-01-T03: a load sent at once found no save).
    assert_eq!(waits(4), [Some(10), Some(10), Some(10), None]);
    assert!(p.probes[4].steps[3]
        .why
        .as_deref()
        .unwrap()
        .contains("does not prove the reload"));
    assert_eq!(
        commands(&p, "T-06"),
        [
            "coc TestInterior",
            "prid 01001000",
            "getpos x",
            "getpos y",
            "getpos z"
        ]
    );
    // No step names a reference or a cell by EditorID, except `coc` (grade D).
    for probe in &p.probes {
        for s in &probe.steps {
            if !s.command.starts_with("coc ") {
                assert!(
                    !s.command.contains("KC_TestCrate") && !s.command.contains("TestInterior"),
                    "{}",
                    s.command
                );
            }
        }
    }
    // The answer shape the matchers read: numbers after `>>`, the 1 on its own line.
    let getincell = &p.probes[0].steps[1];
    let re = regex::Regex::new(getincell.expected.as_deref().unwrap()).unwrap();
    assert!(re.is_match(
        "player.getincell 00000900\nCommand: player.getincell 00000900\nGetInCell >> 1.00\n"
    ));
    assert!(!re.is_match("player.getincell 00000900\nCommand: player.getincell 00000900"));
    assert!(!re.is_match("GetInCell >> 0.00"));
    assert!(p.notes.iter().any(|n| n.contains("game-20261101-T1")));
    // Deterministic.
    let again = d.join("plan2");
    testplan(&data, &plugin, &again, json!(["World.esm"]));
    assert_eq!(read(&again).1, text);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn the_actors_shape_uses_the_runtime_id_and_marks_what_needs_a_person() {
    let d = dir("probes-actor");
    let data = world_master(&d);
    let plugin = d.join("KC_Actor.esm");
    place(
        &data,
        &plugin,
        json!({"base": "KC_Npc", "world": "TestWorld", "position": [150.0, -50.0, 0.0]}),
    );
    let out = d.join("plan");
    testplan(&data, &plugin, &out, json!(["World.esm"]));
    let (p, _) = read(&out);
    assert_eq!(rows(&p), checklist_steps(&out));
    let manual: Vec<&str> = p
        .probes
        .iter()
        .filter(|x| x.is_manual())
        .map(|x| x.row.as_str())
        .collect();
    assert_eq!(manual, ["T-01"], "the exterior cell has no coc target");
    assert!(p.probes[0].reason.as_deref().unwrap().contains("exterior"));
    assert_eq!(commands(&p, "T-02")[0], "player.moveto 01001000");
    assert_eq!(commands(&p, "T-03"), ["prid 01001000", "getisid 00000801"]);
    assert_eq!(commands(&p, "T-04")[0], "prid 01001000");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn the_rename_and_overhaul_shapes_search_by_the_new_full() {
    let d = dir("probes-rename");
    let data = weapons_data(&d);
    let rename = d.join("Rename.esm");
    set_full(&data, "OldRifle", "Old Soviet Marksman Rifle", &rename);
    let out = d.join("plan");
    testplan(&data, &rename, &out, json!(["Source.esm"]));
    let (p, _) = read(&out);
    assert_eq!(rows(&p), checklist_steps(&out));
    assert_eq!(rows(&p), ["T-01", "T-02", "T-03"]);
    let t1 = &p.probes[0];
    assert_eq!(t1.steps[0].command, "help \"Old Soviet Marksman Rifle\" 4");
    assert_eq!(t1.steps[0].expected.as_deref(), Some("00000800"));
    assert!(
        p.probes[2].is_manual(),
        "no new reference to look for after the reload"
    );

    let overhaul = d.join("Overhaul.esm");
    set_full(&data, "OldRifle", "Old Earth Marksman Rifle", &overhaul);
    set_full(&data, "OtherRifle", "Other Rifle Mk2", &overhaul);
    let out2 = d.join("plan-overhaul");
    testplan(&data, &overhaul, &out2, json!(["Source.esm"]));
    let (p2, _) = read(&out2);
    assert_eq!(rows(&p2), checklist_steps(&out2));
    assert_eq!(
        [commands(&p2, "T-01"), commands(&p2, "T-02")],
        [
            vec!["help \"Old Earth Marksman Rifle\" 4".to_string()],
            vec!["help \"Other Rifle Mk2\" 4".to_string()]
        ]
    );
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn the_ownership_shape_has_only_the_persistence_rows() {
    let d = dir("probes-own");
    let data = world_master(&d);
    let plugin = d.join("KC_Own.esm");
    execute(
        "set",
        json!({
            "form": "TestInterior", "fields": {"FULL": "Owned Room"},
            "into": plugin.display().to_string(), "plugins": ["World.esm"],
            "data_dir": data.display().to_string(), "no_cache": true,
        }),
    )
    .unwrap();
    let out = d.join("plan");
    testplan(&data, &plugin, &out, json!(["World.esm"]));
    let (p, _) = read(&out);
    assert_eq!(rows(&p), checklist_steps(&out));
    assert_eq!(rows(&p), ["T-01", "T-02"]);
    assert!(!p.probes[0].is_manual());
    assert!(p.probes[1].is_manual());
    std::fs::remove_dir_all(&d).unwrap();
}

/// The persistence row proves the reload by the cell when both cells have runtime IDs: in
/// the other cell before the load, in the leave cell after it (2026-11-01-T03, row T-05).
#[test]
fn the_persistence_row_goes_elsewhere_between_the_save_and_the_load() {
    use super::super::{CellInfo, Form, IdBasis};
    let basis = IdBasis {
        short: "inferred".into(),
        text: "inferred".into(),
    };
    let crate_in = |cell: &str, runtime: u32| Form {
        signature: "REFR".into(),
        stored: 0x0100_1000,
        new: true,
        editor_id: Some("KC_TestCrate".into()),
        runtime: Some(0x0600_1000),
        base: None,
        cell: Some(CellInfo {
            label: format!("X.esm:{cell}"),
            editor_id: Some(cell.into()),
            interior: true,
            runtime: Some(runtime),
        }),
        position: Some([1.0, 2.0, 3.0]),
        persistent: true,
        offset: 0,
        length: 0,
        full: None,
        base_edid: None,
        base_runtime: None,
        disabled: false,
        scripts: Vec::new(),
    };
    let lodge = 0x0006_BB29;
    let apartment = 0x0021_966F;
    let row = |p: &ProbeFile| {
        p.probes
            .iter()
            .find(|x| x.title.starts_with("persistence 1-3"))
            .unwrap()
            .clone()
    };
    let cmds = |p: &kitkraft_game::probes::Probe| -> Vec<String> {
        p.steps.iter().map(|s| s.command.clone()).collect()
    };

    // The plugin's own interior cell is where the player goes.
    let forms = [crate_in("KC_Room", 0x0600_0900)];
    let p = super::build("X.esm", "00", &forms, &basis, Some(lodge), Some(apartment));
    let r = row(&p);
    assert_eq!(
        cmds(&r),
        [
            "coc CityNewAtlantisLodgeInt",
            "player.getincell 0006BB29",
            "save {save}",
            "coc KC_Room",
            "player.getincell 06000900",
            "load {save}",
            "player.getincell 0006BB29",
        ]
    );
    let waits: Vec<Option<u64>> = r.steps.iter().map(|s| s.wait_secs).collect();
    assert_eq!(
        waits,
        [Some(10), None, Some(10), Some(10), None, Some(10), None]
    );
    assert!(r.steps[6]
        .why
        .as_deref()
        .unwrap()
        .contains("proves the reload"));
    assert!(r.title.contains("go elsewhere"), "{}", r.title);
    assert!(p.check().is_ok());

    // No plugin cell (or the plugin's cell is the leave cell): CityNewAtlantisApartmentTEMPLATE.
    for forms in [vec![], vec![crate_in("CityNewAtlantisLodgeInt", lodge)]] {
        let p = super::build("X.esm", "00", &forms, &basis, Some(lodge), Some(apartment));
        let c = cmds(&row(&p));
        assert_eq!(c[3], "coc CityNewAtlantisApartmentTEMPLATE");
        assert_eq!(c[4], "player.getincell 0021966F");
    }

    // A leave cell but nowhere else to go: the old check, and it says it proves nothing.
    let p = super::build("X.esm", "00", &[], &basis, Some(lodge), None);
    let r = row(&p);
    assert_eq!(
        cmds(&r),
        [
            "coc CityNewAtlantisLodgeInt",
            "save {save}",
            "load {save}",
            "player.getincell 0006BB29",
        ]
    );
    assert!(r.steps[3]
        .why
        .as_deref()
        .unwrap()
        .contains("does not prove the reload"));
}

#[test]
fn inline_text_refuses_what_cannot_sit_in_a_console_quote() {
    use super::inline_text;
    assert_eq!(inline_text(b"Rifle\0"), Some("Rifle".into()));
    assert_eq!(inline_text(b"A \"quoted\" name\0"), None);
    assert_eq!(inline_text(b"two\nlines\0"), None);
    assert_eq!(inline_text(b"\0"), None);
    assert_eq!(inline_text(&[0xFF, 0xFE, 0]), None);
}

/// Every row of the ledger's templates O2 and O4 is mapped in `docs/game-sessions.md` to a
/// probe or a manual row (M4.T2 acceptance).
#[test]
fn every_template_row_maps_to_a_probe_or_a_manual_row() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let ledger = std::fs::read_to_string(root.join("docs").join("game-observations.md")).unwrap();
    let sessions = std::fs::read_to_string(root.join("docs").join("game-sessions.md")).unwrap();
    let template_rows: Vec<String> = ledger
        .split("## Template: session O5")
        .next()
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("| "))
        .filter_map(|l| l.split(' ').next())
        .filter(|id| id.starts_with("O2-") || id.starts_with("O4-"))
        .map(str::to_string)
        .collect();
    assert!(template_rows.len() >= 30, "{template_rows:?}");
    for id in &template_rows {
        let line = sessions
            .lines()
            .find(|l| l.starts_with(&format!("| {id} |")))
            .unwrap_or_else(|| panic!("{id} is not mapped in docs/game-sessions.md"));
        assert!(
            line.contains("| probe |") || line.contains("| manual |"),
            "{id}: {line}"
        );
    }
}

/// What one synthetic `Debug` call passes as its text.
enum Arg {
    /// The backing variable of the `Text` property.
    Prop,
    /// A string literal.
    Literal(&'static str),
    /// A local (text built at run time).
    Local,
}

/// A compiled script `KitKraft:Probe extends ObjectReference` with a `String` auto property
/// `Text` (default `default`) and an `OnLoad` making the given `callstatic Debug <fn>`
/// calls (the layout `kitkraft-pex` reads; see `scripts_tests::pex_with`).
fn pex_calling(default: &str, calls: &[(&str, Arg)]) -> Vec<u8> {
    let mut strings: Vec<String> = [
        "",
        "KitKraft:Probe",
        "ObjectReference",
        "Text",
        "String",
        "::Text_var",
        default,
        "OnLoad",
        "None",
        "Debug",
        "::nonevar",
        "loc",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let index = |s: &str, strings: &mut Vec<String>| -> u16 {
        match strings.iter().position(|x| x == s) {
            Some(i) => i as u16,
            None => {
                strings.push(s.to_string());
                (strings.len() - 1) as u16
            }
        }
    };
    let callstatic = kitkraft_pex::OPCODES
        .iter()
        .position(|(n, _, _)| *n == "callstatic")
        .unwrap() as u8;
    let ident = |o: &mut Vec<u8>, i: u16| {
        o.push(1);
        o.extend(i.to_le_bytes());
    };
    // The object body.
    let mut o = Vec::new();
    o.extend(2u16.to_le_bytes()); // parent
    o.extend(0u16.to_le_bytes()); // doc
    o.push(0);
    o.extend(0u32.to_le_bytes());
    o.extend(0u16.to_le_bytes()); // auto state
    o.extend(0u16.to_le_bytes()); // structs
    o.extend(1u16.to_le_bytes()); // variables: String ::Text_var = default
    o.extend(5u16.to_le_bytes());
    o.extend(4u16.to_le_bytes());
    o.extend(0u32.to_le_bytes());
    o.push(2);
    o.extend(6u16.to_le_bytes());
    o.push(0);
    o.extend(0u16.to_le_bytes()); // guards
    o.extend(1u16.to_le_bytes()); // properties: String Text, auto
    o.extend(3u16.to_le_bytes());
    o.extend(4u16.to_le_bytes());
    o.extend(0u16.to_le_bytes());
    o.extend(0u32.to_le_bytes());
    o.push(7);
    o.extend(5u16.to_le_bytes());
    o.extend(1u16.to_le_bytes()); // states: the default state
    o.extend(0u16.to_le_bytes());
    o.extend(1u16.to_le_bytes()); // one function: OnLoad
    o.extend(7u16.to_le_bytes());
    o.extend(8u16.to_le_bytes()); // returns None
    o.extend(0u16.to_le_bytes());
    o.extend(0u32.to_le_bytes());
    o.push(0);
    o.extend(0u16.to_le_bytes()); // params
    o.extend(1u16.to_le_bytes()); // locals: String loc
    o.extend(11u16.to_le_bytes());
    o.extend(4u16.to_le_bytes());
    o.extend((calls.len() as u16).to_le_bytes());
    for (function, arg) in calls {
        o.push(callstatic);
        ident(&mut o, 9);
        let f = index(function, &mut strings);
        ident(&mut o, f);
        ident(&mut o, 10);
        o.push(3);
        o.extend(1i32.to_le_bytes());
        match arg {
            Arg::Prop => ident(&mut o, 5),
            Arg::Local => ident(&mut o, 11),
            Arg::Literal(t) => {
                let i = index(t, &mut strings);
                o.push(2);
                o.extend(i.to_le_bytes());
            }
        }
    }
    let ws = |b: &mut Vec<u8>, s: &str| {
        b.extend((s.len() as u16).to_le_bytes());
        b.extend(s.as_bytes());
    };
    let mut b = Vec::new();
    b.extend(0xFA57_C0DEu32.to_le_bytes());
    b.extend([3u8, 12]);
    b.extend(2u16.to_le_bytes());
    b.extend(0u64.to_le_bytes());
    ws(&mut b, "Probe.psc");
    ws(&mut b, "u");
    ws(&mut b, "m");
    b.extend((strings.len() as u16).to_le_bytes());
    for s in &strings {
        ws(&mut b, s);
    }
    b.push(0); // no debug info
    b.extend(0u16.to_le_bytes()); // user flags
    b.extend(1u16.to_le_bytes()); // objects
    b.extend(1u16.to_le_bytes()); // object name
    b.extend((o.len() as u32 + 4).to_le_bytes());
    b.extend(o);
    b
}

/// A plugin placing a crate with `KitKraft:Probe` attached (properties `props`), with the
/// compiled script `pex` next to it (none: removed after the build); the probe file, the
/// checklist's steps and the probe file's text.
fn scripted(name: &str, pex: Option<Vec<u8>>, props: Value) -> (ProbeFile, Vec<String>, String) {
    let d = dir(name);
    let data = world_master(&d);
    let scripts = d.join("Scripts").join("KitKraft");
    std::fs::create_dir_all(&scripts).unwrap();
    let path = scripts.join("Probe.pex");
    // `place` checks the properties against the compiled script, so it is there while the
    // plugin is built.
    std::fs::write(&path, pex.clone().unwrap_or_else(|| pex_calling("d", &[]))).unwrap();
    let plugin = d.join("KC_Scripted.esm");
    place(
        &data,
        &plugin,
        json!({"cell": "TestInterior", "edid": "KC_ScriptedCrate",
               "scripts": [{"name": "KitKraft:Probe", "properties": props}]}),
    );
    if pex.is_none() {
        std::fs::remove_file(&path).unwrap();
    }
    let out = d.join("plan");
    testplan(&data, &plugin, &out, json!(["World.esm"]));
    let (p, text) = read(&out);
    let steps = checklist_steps(&out);
    (p, steps, text)
}

fn script_probe(p: &ProbeFile) -> kitkraft_game::probes::Probe {
    p.probes
        .iter()
        .find(|x| x.title.contains("KitKraft:Probe"))
        .unwrap()
        .clone()
}

/// M4.T6: a script that traces its `String` property gets a `papyrus-log` row expecting the
/// value the reference's VMAD sets; a literal is expected as written.
#[test]
fn a_traced_property_becomes_a_papyrus_log_probe() {
    let pex = pex_calling(
        "the default",
        &[
            ("Trace", Arg::Prop),
            ("Notification", Arg::Prop),
            ("Trace", Arg::Literal("KitKraft: literal line")),
        ],
    );
    let (p, steps, text) = scripted(
        "probes-script-trace",
        Some(pex),
        json!({"Text": "KitKraft: set by VMAD"}),
    );
    assert_eq!(rows(&p), steps, "one probe per checklist step");
    let s = script_probe(&p);
    assert!(!s.is_manual(), "{s:?}");
    assert_eq!(
        s.steps.len(),
        2,
        "two Trace calls; the Notification is no log line"
    );
    for step in &s.steps {
        assert_eq!(step.matcher, Some(Matcher::PapyrusLog));
        assert!(step.command.is_empty());
    }
    assert_eq!(
        s.steps[0].expected.as_deref(),
        Some("KitKraft: set by VMAD")
    );
    assert!(s.steps[0].why.as_deref().unwrap().contains("VMAD sets it"));
    assert_eq!(
        s.steps[1].expected.as_deref(),
        Some("KitKraft: literal line")
    );
    assert!(s.forms.iter().any(|f| f == "Scripts\\KitKraft\\Probe.pex"));
    // After the reference's cell, moveto, click and getpos rows.
    let at = p.probes.iter().position(|x| x.row == s.row).unwrap();
    assert_eq!(at, 4, "{:?}", rows(&p));
    // The file parses back for `kit game session`.
    assert!(ProbeFile::parse(&text).is_ok());
}

/// A property the VMAD does not set is traced with the script's default.
#[test]
fn an_unset_property_is_traced_with_the_scripts_default() {
    let pex = pex_calling("KitKraft: the default", &[("Trace", Arg::Prop)]);
    let (p, _, _) = scripted("probes-script-default", Some(pex), json!({}));
    let s = script_probe(&p);
    assert_eq!(
        s.steps[0].expected.as_deref(),
        Some("KitKraft: the default")
    );
    assert!(s.steps[0].why.as_deref().unwrap().contains("default"));
}

/// The rows that cannot be probed say why (the unprobeable list of M4.T6): no Trace call,
/// text built at run time, or no compiled script next to the plugin. None of them passes.
#[test]
fn a_script_with_nothing_to_match_is_a_manual_row_with_the_reason() {
    let quiet = pex_calling("x", &[("Notification", Arg::Prop)]);
    let (p, steps, _) = scripted("probes-script-quiet", Some(quiet), json!({}));
    assert_eq!(rows(&p), steps);
    let s = script_probe(&p);
    assert!(s.is_manual() && s.steps.is_empty(), "{s:?}");
    assert!(s.reason.as_deref().unwrap().contains("no Debug.Trace call"));

    let local = pex_calling("x", &[("Trace", Arg::Local)]);
    let (p, _, _) = scripted("probes-script-local", Some(local), json!({}));
    let s = script_probe(&p);
    assert!(s.is_manual());
    assert!(s.reason.as_deref().unwrap().contains("built at run time"));

    let (p, steps, _) = scripted("probes-script-missing", None, json!({}));
    assert_eq!(rows(&p), steps);
    let s = script_probe(&p);
    assert!(s.is_manual());
    assert!(s
        .reason
        .as_deref()
        .unwrap()
        .contains("not next to the plugin"));
}

/// A synthetic state: its name and its event handlers with their code.
type StateCode<'a> = (&'a str, Vec<(&'a str, Vec<Ins>)>);

/// One instruction of a synthetic event handler.
enum Ins {
    /// `callstatic Debug <function>(<arg>)`.
    Call(&'static str, Arg),
    /// `jmpf ::temp0 <offset>`: the start of an If or While.
    Jmpf,
}

/// A compiled script `KitKraft:Probe` like [`pex_calling`], with any states and event
/// handlers: `states` is (state name, [(event, code)]), `auto_state` the object's auto
/// state ("" for none).
fn pex_states(default: &str, auto_state: &str, states: &[StateCode]) -> Vec<u8> {
    let mut strings: Vec<String> = [
        "",
        "KitKraft:Probe",
        "ObjectReference",
        "Text",
        "String",
        "::Text_var",
        default,
        "None",
        "Debug",
        "::nonevar",
        "loc",
        "::temp0",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    fn index(s: &str, strings: &mut Vec<String>) -> u16 {
        match strings.iter().position(|x| x == s) {
            Some(i) => i as u16,
            None => {
                strings.push(s.to_string());
                (strings.len() - 1) as u16
            }
        }
    }
    let op = |name: &str| {
        kitkraft_pex::OPCODES
            .iter()
            .position(|(n, _, _)| *n == name)
            .unwrap() as u8
    };
    let ident = |o: &mut Vec<u8>, i: u16| {
        o.push(1);
        o.extend(i.to_le_bytes());
    };
    let auto = index(auto_state, &mut strings);
    let mut o = Vec::new();
    o.extend(2u16.to_le_bytes()); // parent
    o.extend(0u16.to_le_bytes()); // doc
    o.push(0);
    o.extend(0u32.to_le_bytes());
    o.extend(auto.to_le_bytes()); // auto state
    o.extend(0u16.to_le_bytes()); // structs
    o.extend(1u16.to_le_bytes()); // variables: String ::Text_var = default
    o.extend(5u16.to_le_bytes());
    o.extend(4u16.to_le_bytes());
    o.extend(0u32.to_le_bytes());
    o.push(2);
    o.extend(6u16.to_le_bytes());
    o.push(0);
    o.extend(0u16.to_le_bytes()); // guards
    o.extend(1u16.to_le_bytes()); // properties: String Text, auto
    o.extend(3u16.to_le_bytes());
    o.extend(4u16.to_le_bytes());
    o.extend(0u16.to_le_bytes());
    o.extend(0u32.to_le_bytes());
    o.push(7);
    o.extend(5u16.to_le_bytes());
    o.extend((states.len() as u16).to_le_bytes());
    for (state, functions) in states {
        let s = index(state, &mut strings);
        o.extend(s.to_le_bytes());
        o.extend((functions.len() as u16).to_le_bytes());
        for (event, code) in functions {
            let e = index(event, &mut strings);
            o.extend(e.to_le_bytes());
            o.extend(7u16.to_le_bytes()); // returns None
            o.extend(0u16.to_le_bytes());
            o.extend(0u32.to_le_bytes());
            o.push(0);
            o.extend(0u16.to_le_bytes()); // params
            o.extend(1u16.to_le_bytes()); // locals: String loc
            o.extend(10u16.to_le_bytes());
            o.extend(4u16.to_le_bytes());
            o.extend((code.len() as u16).to_le_bytes());
            for ins in code {
                match ins {
                    Ins::Jmpf => {
                        o.push(op("jmpf"));
                        ident(&mut o, 11);
                        o.push(3);
                        o.extend(2i32.to_le_bytes());
                    }
                    Ins::Call(function, arg) => {
                        o.push(op("callstatic"));
                        ident(&mut o, 8);
                        let f = index(function, &mut strings);
                        ident(&mut o, f);
                        ident(&mut o, 9);
                        o.push(3);
                        o.extend(1i32.to_le_bytes());
                        match arg {
                            Arg::Prop => ident(&mut o, 5),
                            Arg::Local => ident(&mut o, 10),
                            Arg::Literal(t) => {
                                let i = index(t, &mut strings);
                                o.push(2);
                                o.extend(i.to_le_bytes());
                            }
                        }
                    }
                }
            }
        }
    }
    let ws = |b: &mut Vec<u8>, s: &str| {
        b.extend((s.len() as u16).to_le_bytes());
        b.extend(s.as_bytes());
    };
    let mut b = Vec::new();
    b.extend(0xFA57_C0DEu32.to_le_bytes());
    b.extend([3u8, 12]);
    b.extend(2u16.to_le_bytes());
    b.extend(0u64.to_le_bytes());
    ws(&mut b, "Probe.psc");
    ws(&mut b, "u");
    ws(&mut b, "m");
    b.extend((strings.len() as u16).to_le_bytes());
    for s in &strings {
        ws(&mut b, s);
    }
    b.push(0); // no debug info
    b.extend(0u16.to_le_bytes()); // user flags
    b.extend(1u16.to_le_bytes()); // objects
    b.extend(1u16.to_le_bytes()); // object name
    b.extend((o.len() as u32 + 4).to_le_bytes());
    b.extend(o);
    b
}

/// Review H1, probe 1: a Trace after a branch in `OnLoad` (inside an If) may not run, so it
/// is no expected line; the row says why.
#[test]
fn a_trace_inside_a_branch_is_not_expected() {
    let pex = pex_states(
        "x",
        "",
        &[(
            "",
            vec![(
                "OnLoad",
                vec![
                    Ins::Jmpf,
                    Ins::Call("Trace", Arg::Literal("only when Gate is 1")),
                ],
            )],
        )],
    );
    let (p, steps, _) = scripted("probes-script-branch", Some(pex), json!({}));
    assert_eq!(rows(&p), steps);
    let s = script_probe(&p);
    assert!(s.is_manual() && s.steps.is_empty(), "{s:?}");
    assert!(
        s.reason.as_deref().unwrap().contains("after a branch"),
        "{s:?}"
    );
}

/// Review H1, probe 2: a Trace in another event (`OnActivate`) is not run by entering the
/// cell.
#[test]
fn a_trace_in_another_event_is_not_expected() {
    let pex = pex_states(
        "x",
        "",
        &[(
            "",
            vec![(
                "OnActivate",
                vec![Ins::Call("Trace", Arg::Literal("only on activate"))],
            )],
        )],
    );
    let (p, _, _) = scripted("probes-script-event", Some(pex), json!({}));
    let s = script_probe(&p);
    assert!(s.is_manual() && s.steps.is_empty(), "{s:?}");
    assert!(s.reason.as_deref().unwrap().contains("OnActivate"), "{s:?}");
}

/// Review H1, probe 3: a Trace in `OnLoad` of a state that is not the auto state does not
/// run; the auto state's `OnLoad` does.
#[test]
fn a_trace_in_another_state_is_not_expected() {
    let pex = pex_states(
        "x",
        "",
        &[
            (
                "",
                vec![(
                    "OnLoad",
                    vec![Ins::Call("Trace", Arg::Literal("auto state line"))],
                )],
            ),
            (
                "Sleeping",
                vec![(
                    "OnLoad",
                    vec![Ins::Call("Trace", Arg::Literal("only in state Sleeping"))],
                )],
            ),
        ],
    );
    let (p, _, _) = scripted("probes-script-state", Some(pex), json!({}));
    let s = script_probe(&p);
    assert!(!s.is_manual(), "{s:?}");
    let expected: Vec<_> = s
        .steps
        .iter()
        .map(|x| x.expected.clone().unwrap())
        .collect();
    assert_eq!(expected, ["auto state line"]);
    let why = s.steps[0].why.as_deref().unwrap();
    assert!(
        why.contains("state Sleeping"),
        "the unchecked call is named: {why}"
    );
    // An auto state of its own replaces the default state's handler.
    let pex = pex_states(
        "x",
        "Awake",
        &[
            (
                "",
                vec![(
                    "OnLoad",
                    vec![Ins::Call("Trace", Arg::Literal("default state line"))],
                )],
            ),
            (
                "Awake",
                vec![(
                    "OnLoad",
                    vec![Ins::Call("Trace", Arg::Literal("awake line"))],
                )],
            ),
        ],
    );
    let (p, _, _) = scripted("probes-script-auto", Some(pex), json!({}));
    let s = script_probe(&p);
    let expected: Vec<_> = s
        .steps
        .iter()
        .map(|x| x.expected.clone().unwrap())
        .collect();
    assert_eq!(expected, ["awake line"]);
}

/// Review M1: a script on the placed reference's base form (the mini-site's switch, a
/// cloned ACTI carrying HelloOnLoad) gets its row too, not silence.
#[test]
fn a_script_on_the_base_form_gets_its_row() {
    let d = dir("probes-script-base");
    let data = world_master(&d);
    let scripts = d.join("Scripts").join("KitKraft");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(
        scripts.join("Probe.pex"),
        pex_calling("x", &[("Trace", Arg::Prop)]),
    )
    .unwrap();
    let spec = d.join("site.toml");
    std::fs::write(
        &spec,
        format!(
            "version = 3\n\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n\
[[clone]]\nname = \"switch\"\nform = \"KC_Crate\"\nedid = \"KC_SiteSwitch\"\n\
scripts = [{{ name = \"KitKraft:Probe\", properties = {{ Text = \"KitKraft: site switch loaded\" }} }}]\n\n\
[[place]]\nbase = \"new:switch\"\ncell = \"TestInterior\"\nposition = [1, 2, 3]\nedid = \"KC_SiteSwitchRef\"\n",
            data.display()
        ),
    )
    .unwrap();
    let plugin = d.join("KC_Site.esm");
    execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": plugin.display().to_string()}),
    )
    .unwrap();
    let out = d.join("plan");
    testplan(&data, &plugin, &out, json!(["World.esm"]));
    let (p, _) = read(&out);
    assert_eq!(rows(&p), checklist_steps(&out));
    let s = script_probe(&p);
    assert!(s.title.contains("base"), "{}", s.title);
    assert_eq!(
        s.steps[0].expected.as_deref(),
        Some("KitKraft: site switch loaded")
    );
    assert!(s.steps[0].why.as_deref().unwrap().contains("base form"));
}
