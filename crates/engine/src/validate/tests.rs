//! `validate` through `execute`: a positive and a negative synthetic fixture per rule
//! (M2.E6, M2.E7, M2.E8). No third-party data: the Resupply door is rebuilt from scratch.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitcraft_esm::tes4::{FLAG_LOCALIZED, FLAG_MASTER};
use kitcraft_esm::{Body, Record, RecordHeader, Signature, Subrecord};
use serde_json::{json, Value};

use crate::edit::patch_tests::{dir, weapon, write_plugin};
use crate::execute;

fn sub(sig: &[u8; 4], data: &[u8]) -> Subrecord<'static> {
    Subrecord::new(Signature(*sig), data.to_vec())
}

fn record(sig: &[u8; 4], form_id: u32, subs: Vec<Subrecord<'static>>) -> Record<'static> {
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

fn check(plugin: &Path, rules: &[&str]) -> Value {
    execute(
        "validate",
        json!({"plugin": plugin.display().to_string(), "rules": rules}),
    )
    .unwrap()["data"]
        .clone()
}

/// `(rule, severity)` of every finding.
fn found(data: &Value) -> Vec<(String, String)> {
    data["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["rule"].as_str().unwrap().to_string(),
                f["severity"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pair(rule: &str, severity: &str) -> (String, String) {
    (rule.to_string(), severity.to_string())
}

/// `Master.esm`: localized; a weapon with FULL, a door with FULL and NTRM, a keyword.
fn master(d: &Path) -> PathBuf {
    let door = record(
        b"DOOR",
        0x0000_0A00,
        vec![
            sub(b"EDID", b"StationDoor\0"),
            sub(b"FULL", &0x4025u32.to_le_bytes()),
            sub(b"NTRM", &[1, 0, 0, 0]),
        ],
    );
    let keyword = record(b"KYWD", 0x0000_0900, vec![sub(b"EDID", b"Kw\0")]);
    write_plugin(
        d,
        "Master.esm",
        &[],
        FLAG_MASTER | FLAG_LOCALIZED,
        vec![weapon(0x0000_0800, "Rifle", 0x0002_E469), door, keyword],
    )
}

fn patch(d: &Path, name: &str, masters: &[&str], records: Vec<Record<'static>>) -> PathBuf {
    write_plugin(d, name, masters, 0, records)
}

/// A clean override: inline FULL, every lstring text.
fn good_weapon() -> Record<'static> {
    let mut w = weapon(0x0000_0800, "Rifle", 0);
    for s in w.subrecords_mut().unwrap() {
        match s.signature().to_string().as_str() {
            "FULL" => s.set_data(b"Old Soviet Marksman Rifle\0".to_vec()),
            "DESC" => s.set_data(b"\0".to_vec()),
            _ => {}
        }
    }
    w
}

#[test]
fn a_clean_patch_passes_every_rule() {
    let d = dir("validate-clean");
    master(&d);
    let p = patch(&d, "Clean.esp", &["Master.esm"], vec![good_weapon()]);
    let result = execute("validate", json!({"plugin": p.display().to_string()})).unwrap();
    let data = &result["data"];
    assert_eq!(data["passed"], json!(true), "{data:#}");
    assert_eq!(found(data), Vec::<(String, String)>::new());
    assert_eq!(data["rules"].as_array().unwrap().len(), 6);
    assert_eq!(data["masters"][0]["localized"], json!(true));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn masters_missing_and_order() {
    let d = dir("validate-masters");
    master(&d);
    let a1 = write_plugin(&d, "A1.esm", &[], FLAG_MASTER, vec![]);
    write_plugin(&d, "A2.esm", &["A1.esm"], FLAG_MASTER, vec![]);
    let _ = a1;
    let bad = patch(&d, "Bad.esp", &["A2.esm", "A1.esm", "Nope.esm"], vec![]);
    let data = check(&bad, &["masters.missing", "masters.order"]);
    assert_eq!(
        found(&data),
        [
            pair("masters.missing", "error"),
            pair("masters.order", "error")
        ]
    );
    assert_eq!(data["passed"], json!(false));
    let good = patch(&d, "Good.esp", &["A1.esm", "A2.esm"], vec![]);
    assert_eq!(
        found(&check(&good, &["masters.missing", "masters.order"])),
        []
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn formid_dangling() {
    let d = dir("validate-formid");
    master(&d);
    let keywords = |kw: u32| {
        let mut w = good_weapon();
        // KSIZ/KWDA sit before DESC in the WEAP definition (as in the corpus).
        let subs = w.subrecords_mut().unwrap();
        let at = subs
            .iter()
            .position(|s| s.signature() == Signature(*b"DESC"))
            .unwrap();
        subs.insert(at, sub(b"KWDA", &kw.to_le_bytes()));
        subs.insert(at, sub(b"KSIZ", &1u32.to_le_bytes()));
        w
    };
    // Header FormID beyond the master list; a keyword the master lacks.
    let beyond = record(b"WEAP", 0x0500_0001, vec![sub(b"EDID", b"X\0")]);
    let bad = patch(
        &d,
        "Bad.esp",
        &["Master.esm"],
        vec![keywords(0x0000_0999), beyond],
    );
    let data = check(&bad, &["formid.dangling"]);
    let mut got = found(&data);
    got.sort();
    assert_eq!(
        got,
        [
            pair("formid.dangling", "error"),
            pair("formid.dangling", "warning")
        ]
    );
    assert_eq!(data["passed"], json!(false));
    assert!(data["checked_form_ids"].as_u64().unwrap() >= 3);
    let good = patch(&d, "Good.esp", &["Master.esm"], vec![keywords(0x0000_0900)]);
    let data = check(&good, &["formid.dangling"]);
    assert_eq!(found(&data), []);
    assert!(data["unchecked_subrecords"].as_u64().unwrap() > 0);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn lstring_copied_id() {
    let d = dir("validate-lstring");
    master(&d);
    // The Resupply mistake: the master's FULL string ID copied as is.
    let bad = patch(
        &d,
        "Bad.esp",
        &["Master.esm"],
        vec![weapon(0x0000_0800, "Rifle", 0x0002_E469)],
    );
    let data = check(&bad, &["lstring.copied-id"]);
    let findings = data["findings"].as_array().unwrap();
    // FULL[0], DESC[0] (ID 0) and the template FULL[1].
    assert_eq!(findings.len(), 3, "{data:#}");
    assert!(findings.iter().all(|f| f["message"]
        .as_str()
        .unwrap()
        .starts_with("localized string id copied into non-localized plugin")));
    assert_eq!(findings[0]["subrecord"], json!("FULL[0]"));
    assert!(findings[0]["offset"].as_u64().is_some());
    let good = patch(&d, "Good.esp", &["Master.esm"], vec![good_weapon()]);
    assert_eq!(found(&check(&good, &["lstring.copied-id"])), []);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn override_stripped_name_the_resupply_door() {
    let d = dir("validate-door");
    master(&d);
    let stripped = record(
        b"DOOR",
        0x0000_0A00,
        vec![sub(b"EDID", b"StationDoor\0"), sub(b"NTRM", &[1, 0, 0, 0])],
    );
    let bad = patch(&d, "Bad.esp", &["Master.esm"], vec![stripped]);
    let data = check(&bad, &["override.stripped-name"]);
    assert_eq!(found(&data), [pair("override.stripped-name", "error")]);
    assert_eq!(data["findings"][0]["subrecord"], json!("FULL"));
    let fixed = record(
        b"DOOR",
        0x0000_0A00,
        vec![
            sub(b"EDID", b"StationDoor\0"),
            sub(b"FULL", b"Door\0"),
            sub(b"NTRM", &[1, 0, 0, 0]),
        ],
    );
    let good = patch(&d, "Good.esp", &["Master.esm"], vec![fixed]);
    assert_eq!(found(&check(&good, &["override.stripped-name"])), []);
    // The official DLCs strip names of cells (and LCTN, NPC_, QUST, WRLD): a warning.
    let cell = |subs| record(b"CELL", 0x0000_0B00, subs);
    let m2 = write_plugin(
        &d,
        "Cells.esm",
        &[],
        FLAG_MASTER,
        vec![cell(vec![sub(b"EDID", b"Room\0"), sub(b"FULL", b"Room\0")])],
    );
    let _ = m2;
    let unnamed = patch(
        &d,
        "Cell.esp",
        &["Cells.esm"],
        vec![cell(vec![sub(b"EDID", b"Room\0")])],
    );
    let data = check(&unnamed, &["override.stripped-name"]);
    assert_eq!(found(&data), [pair("override.stripped-name", "warning")]);
    assert_eq!(data["passed"], json!(true));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn record_flags_body() {
    let d = dir("validate-body");
    master(&d);
    // A compressed record whose body is not zlib; a non-localized FULL with no NUL.
    let mut broken = record(b"WEAP", 0x0100_0001, vec![]);
    broken.header.flags = kitcraft_esm::FLAG_COMPRESSED;
    *broken.body_mut() = Body::Compressed {
        original: std::borrow::Cow::Owned(vec![8, 0, 0, 0, 1, 2, 3, 4]),
        decoded: None,
        inflated: None,
    };
    let unterminated = record(b"WEAP", 0x0100_0002, vec![sub(b"FULL", b"Rifle")]);
    let bad = patch(&d, "Bad.esp", &["Master.esm"], vec![broken, unterminated]);
    let data = check(&bad, &["record.flags-body"]);
    assert_eq!(
        found(&data),
        [
            pair("record.flags-body", "error"),
            pair("record.flags-body", "error")
        ]
    );
    // A localized plugin whose FULL is text, and a wrong HEDR count (warning).
    let loc = write_plugin(
        &d,
        "Loc.esm",
        &[],
        FLAG_MASTER | FLAG_LOCALIZED,
        vec![record(b"WEAP", 0x0000_0001, vec![sub(b"FULL", b"Rifle\0")])],
    );
    let mut bytes = std::fs::read(&loc).unwrap();
    // TES4 header (24) + HEDR header (6) + version (4): the record count.
    bytes[34..38].copy_from_slice(&99u32.to_le_bytes());
    std::fs::write(&loc, &bytes).unwrap();
    let data = check(&loc, &["record.flags-body"]);
    assert_eq!(
        found(&data),
        [
            pair("record.flags-body", "error"),
            pair("record.flags-body", "warning")
        ]
    );
    let good = patch(&d, "Good.esp", &["Master.esm"], vec![good_weapon()]);
    assert_eq!(found(&check(&good, &["record.flags-body"])), []);
    let _ = std::fs::remove_dir_all(&d);
}

/// Little-endian `.pex` bytes (layout: `kitcraft-pex`, `pexdis.py`): one object, one
/// function, one `callstatic <class> <function> ::nonevar <bool>`.
fn pex(class: &str, function: &str) -> Vec<u8> {
    pex_call(25, class, function, None)
}

/// As [`pex`], with opcode `op`: 25 `callstatic <a> <function>`, or 23
/// `callmethod <function> <a>` (`a` the receiver variable). `local` declares one local
/// variable `(name, type)` in the function.
fn pex_call(op: u8, a: &str, function: &str, local: Option<(&str, &str)>) -> Vec<u8> {
    let (local_name, local_type) = local.unwrap_or(("unused", "None"));
    let strings = [
        "",
        "Gm",
        "Form",
        a,
        function,
        "::nonevar",
        "OnInit",
        "None",
        local_name,
        local_type,
    ];
    let mut b: Vec<u8> = Vec::new();
    let ws = |b: &mut Vec<u8>, s: &str| {
        b.extend((s.len() as u16).to_le_bytes());
        b.extend(s.as_bytes());
    };
    b.extend(0xFA57_C0DEu32.to_le_bytes());
    b.extend([3, 12]);
    b.extend(2u16.to_le_bytes());
    b.extend(0u64.to_le_bytes());
    for s in ["Gm.psc", "user", "machine"] {
        ws(&mut b, s);
    }
    b.extend((strings.len() as u16).to_le_bytes());
    for s in strings {
        ws(&mut b, s);
    }
    b.push(0); // no debug info
    b.extend(0u16.to_le_bytes()); // no user flags
    b.extend(1u16.to_le_bytes()); // one object
    let mut o: Vec<u8> = Vec::new();
    o.extend(2u16.to_le_bytes()); // parent Form
    o.extend(0u16.to_le_bytes()); // doc
    o.push(0); // const
    o.extend(0u32.to_le_bytes()); // user flags
    o.extend(0u16.to_le_bytes()); // auto state
    for _ in 0..4 {
        o.extend(0u16.to_le_bytes()); // structs, variables, guards, properties
    }
    o.extend(1u16.to_le_bytes()); // one state
    o.extend(0u16.to_le_bytes()); // the default state
    o.extend(1u16.to_le_bytes()); // one function
    o.extend(6u16.to_le_bytes()); // OnInit
    o.extend(7u16.to_le_bytes()); // returns None
    o.extend(0u16.to_le_bytes()); // doc
    o.extend(0u32.to_le_bytes()); // user flags
    o.push(0); // flags
    o.extend(0u16.to_le_bytes()); // params
    if local.is_some() {
        o.extend(1u16.to_le_bytes()); // one local
        o.extend(8u16.to_le_bytes());
        o.extend(9u16.to_le_bytes());
    } else {
        o.extend(0u16.to_le_bytes()); // locals
    }
    o.extend(1u16.to_le_bytes()); // one instruction
    o.push(op);
    let operands: [u16; 3] = if op == 23 { [4, 3, 5] } else { [3, 4, 5] };
    for i in operands {
        o.push(1);
        o.extend(i.to_le_bytes());
    }
    o.push(3);
    o.extend(1i32.to_le_bytes()); // one vararg
    o.extend([5, 1]); // True
    b.extend(1u16.to_le_bytes()); // object name Gm
    b.extend((o.len() as u32 + 4).to_le_bytes());
    b.extend(o);
    b
}

#[test]
fn papyrus_denylisted_call() {
    let d = dir("validate-pex");
    std::fs::write(d.join("Bad.pex"), pex("Debug", "SetGodMode")).unwrap();
    std::fs::write(d.join("Good.pex"), pex("Debug", "Notification")).unwrap();
    let run = |path: PathBuf| {
        execute("validate", json!({"scripts": [path.display().to_string()]})).unwrap()["data"]
            .clone()
    };
    let data = run(d.join("Bad.pex"));
    assert_eq!(found(&data), [pair("papyrus.denylisted-call", "warning")]);
    assert_eq!(data["passed"], json!(true));
    assert_eq!(data["findings"][0]["record"], json!("Gm.OnInit"));
    assert!(data["findings"][0]["message"]
        .as_str()
        .unwrap()
        .contains("debug.setgodmode(True)"));
    assert_eq!(found(&run(d.join("Good.pex"))), []);
    // A directory is scanned recursively.
    assert_eq!(run(d.clone())["scripts_checked"], json!(2));
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 13: `callmethod` matches on the receiver's class too, so a
/// user script's own `SetGodMode` (on `self`, or on a variable of a user type) is not a
/// finding; a receiver of a denylisted class is.
#[test]
fn papyrus_callmethod_matches_the_class() {
    let d = dir("validate-callmethod");
    let run = |name: &str, bytes: Vec<u8>| {
        let path = d.join(name);
        std::fs::write(&path, bytes).unwrap();
        execute("validate", json!({"scripts": [path.display().to_string()]})).unwrap()["data"]
            .clone()
    };
    let own = run("Own.pex", pex_call(23, "self", "SetGodMode", None));
    assert_eq!(found(&own), []);
    let user = run(
        "User.pex",
        pex_call(23, "q", "SetGodMode", Some(("q", "MyQuestScript"))),
    );
    assert_eq!(found(&user), []);
    let unknown = run("Unknown.pex", pex_call(23, "nowhere", "SetGodMode", None));
    assert_eq!(found(&unknown), []);
    let debug = run(
        "Debug.pex",
        pex_call(23, "d", "SetGodMode", Some(("d", "Debug"))),
    );
    assert_eq!(found(&debug), [pair("papyrus.denylisted-call", "warning")]);
    let _ = std::fs::remove_dir_all(&d);
}

/// Our own god-mode patch (skipped when absent): the patched airlock script no longer
/// turns god mode on; the landing script still toggles it (now with a watchdog).
#[test]
fn papyrus_denylisted_call_on_the_local_noc_patch() {
    let dir = Path::new(r"C:\Modding\scripts\noc_godmode_patch\pex");
    if !dir.is_dir() {
        println!("skipped: {} not present", dir.display());
        return;
    }
    let count = |name: &str| {
        let data = execute(
            "validate",
            json!({"scripts": [dir.join(name).display().to_string()]}),
        )
        .unwrap()["data"]
            .clone();
        data["findings"].as_array().unwrap().len()
    };
    assert_eq!(count("SQ_Airlock_QuestScript.pex"), 0);
    assert_eq!(count("CS7_SpaceshipLandingQuestScript.pex"), 4);
}

#[test]
fn argument_errors() {
    let err = execute("validate", json!({})).unwrap_err();
    assert_eq!(err.kind(), "invalid_args");
    let err = execute(
        "validate",
        json!({"plugin": "x.esp", "rules": ["masters.missing", "nope"]}),
    )
    .unwrap_err();
    assert_eq!(err.kind(), "invalid_args");
    assert!(err.to_string().contains("/rules/1"), "{err}");
    let err = execute("validate", json!({"plugin": "no/such.esp"})).unwrap_err();
    assert_eq!(err.kind(), "failed");
}
