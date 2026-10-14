//! `set` through `execute` with JSON and through the `FIELD=value` shorthand (M2.E5), on a
//! synthetic load order with loose string tables.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitcraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitcraft_esm::tes4::{FLAG_LOCALIZED, FLAG_MASTER};
use kitcraft_esm::Plugin;
use kitcraft_strings::{Entry, Kind, Table};
use serde_json::{json, Value};

use crate::edit::patch_tests::{dir, weapon, write_plugin};
use crate::{execute, parse_line, Invocation};

/// `Data\Source.esm` (localized: OldRifle 000800, OtherRifle 000801), its loose English
/// table, and `Data\Override.esp` overriding OtherRifle with an inline name.
pub(crate) fn data(d: &Path) -> PathBuf {
    let data = d.join("Data");
    std::fs::create_dir_all(data.join("Strings")).unwrap();
    write_plugin(
        &data,
        "Source.esm",
        &[],
        FLAG_MASTER | FLAG_LOCALIZED,
        vec![
            weapon(0x0000_0800, "OldRifle", 0x0002_E469),
            weapon(0x0000_0801, "OtherRifle", 0x0002_E46A),
        ],
    );
    let table = Table::new(
        Kind::Strings,
        vec![
            Entry::new(0x0002_E469, "Old Earth Hunting Rifle"),
            Entry::new(0x0002_E46A, "Other Rifle"),
            Entry::new(0x0002_EDD8, "Simple"),
        ],
    );
    std::fs::write(
        data.join("Strings").join("source_en.strings"),
        table.to_bytes().unwrap(),
    )
    .unwrap();
    let mut other = weapon(0x0000_0801, "OtherRifle", 0);
    for s in other.subrecords_mut().unwrap() {
        if s.signature().to_string() == "FULL" {
            s.set_data(b"Overridden\0".to_vec());
        }
        if s.signature().to_string() == "DESC" {
            s.set_data(b"\0".to_vec());
        }
    }
    write_plugin(&data, "Override.esp", &["Source.esm"], 0, vec![other]);
    data
}

fn set(data: &Path, form: &str, fields: Value, into: &Path) -> Value {
    execute(
        "set",
        json!({
            "form": form, "fields": fields, "into": into.display().to_string(),
            "plugins": ["Source.esm", "Override.esp"], "data_dir": data.display().to_string(),
            "no_cache": true,
        }),
    )
    .unwrap()
}

#[test]
fn set_through_json_writes_an_inline_name() {
    let d = dir("set-json");
    let data = data(&d);
    let out = d.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let target = out.join("Patch.esm");
    let result = set(
        &data,
        "Source.esm:000800",
        json!({"FULL": "Old Soviet Marksman Rifle"}),
        &target,
    );
    let r = &result["data"];
    assert_eq!(r["created"], json!(true));
    assert_eq!(r["masters"], json!(["Source.esm"]));
    assert_eq!(r["flags"], json!("0x00000001"));
    let rec = &r["records"][0];
    assert_eq!(rec["editor_id"], json!("OldRifle"));
    assert_eq!(rec["fields"][0]["path"], json!("FULL"));
    assert_eq!(
        rec["fields"][0]["before"],
        json!({"string_id": "0x0002E469"})
    );
    assert_eq!(
        rec["fields"][0]["after"],
        json!("Old Soviet Marksman Rifle")
    );
    assert_eq!(rec["copied_from"]["plugin"], json!("Source.esm"));
    let inlined: Vec<&str> = rec["inlined_lstrings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["text"].as_str().unwrap())
        .collect();
    assert_eq!(inlined, ["", "Simple"]);
    // Evidence: the source record, the written record, the written FULL.
    let ev = result["evidence"].as_array().unwrap();
    assert_eq!(ev[0]["file"], json!("Source.esm"));
    assert_eq!(ev[1]["file"], json!("Patch.esm"));
    assert_eq!(ev[2]["subrecord"], json!("FULL"));
    let bytes = std::fs::read(&target).unwrap();
    let at = ev[2]["offset"].as_u64().unwrap() as usize;
    assert_eq!(&bytes[at..at + 26], b"Old Soviet Marksman Rifle\0");
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    assert!(!Plugin::parse(&bytes).unwrap().is_localized());
    // The patch validates against its master.
    let v = execute(
        "validate",
        json!({"plugin": target.display().to_string(), "data_dir": data.display().to_string()}),
    )
    .unwrap();
    assert_eq!(v["data"]["passed"], json!(true), "{:#}", v["data"]);
    assert_eq!(v["data"]["warnings"], json!(0));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_shorthand_gives_the_same_bytes() {
    let d = dir("set-short");
    let data = data(&d);
    let a = d.join("A.esp");
    let b = d.join("B.esp");
    set(
        &data,
        "OldRifle",
        json!({"FULL": "Old Soviet Marksman Rifle"}),
        &a,
    );
    let line = format!(
        r#"set OldRifle FULL="Old Soviet Marksman Rifle" --into "{}" --plugins Source.esm --plugins Override.esp --data-dir "{}" --no-cache"#,
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
fn the_winning_override_is_copied_and_other_records_stay() {
    let d = dir("set-winner");
    let data = data(&d);
    let target = d.join("Patch.esp");
    set(&data, "OldRifle", json!({"FULL": "A"}), &target);
    let before = std::fs::read(&target).unwrap();
    let result = set(&data, "OtherRifle", json!({"FULL": "B"}), &target);
    let r = &result["data"];
    assert_eq!(r["created"], json!(false));
    // OtherRifle wins in Override.esp, so the patch now needs it as a master.
    assert_eq!(r["masters"], json!(["Source.esm", "Override.esp"]));
    assert_eq!(
        r["records"][0]["copied_from"]["plugin"],
        json!("Override.esp")
    );
    assert_eq!(r["records"][0]["fields"][0]["before"], json!("Overridden"));
    assert!(result["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("master list was extended")));
    // The first record is byte-identical.
    let after = std::fs::read(&target).unwrap();
    let first = |bytes: &[u8]| {
        let p = Plugin::parse(bytes).unwrap();
        let kitcraft_esm::Entry::Group(g) = &p.entries()[0] else {
            panic!()
        };
        let kitcraft_esm::Entry::Record(r) = &g.children()[0] else {
            panic!()
        };
        let start = r.offset().unwrap() as usize;
        bytes[start..start + 24 + r.data_size_as_read().unwrap() as usize].to_vec()
    };
    assert_eq!(first(&before), first(&after));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn set_errors() {
    let d = dir("set-errors");
    let data = data(&d);
    let target = d.join("P.esp");
    let call = |fields: Value, into: &Path| {
        execute(
            "set",
            json!({
                "form": "OldRifle", "fields": fields, "into": into.display().to_string(),
                "plugins": ["Source.esm"], "data_dir": data.display().to_string(), "no_cache": true,
            }),
        )
    };
    let err = call(json!({"full": "x"}), &target).unwrap_err();
    assert_eq!(err.kind(), "invalid_args");
    assert!(err.to_string().contains("/fields/full"), "{err}");
    let err = call(json!({"FULL[7]": "x"}), &target).unwrap_err();
    assert_eq!(err.kind(), "invalid_args");
    assert!(err.to_string().contains("no-subrecord"), "{err}");
    let err = call(json!({"MODL": "x"}), &target).unwrap_err();
    assert!(err.to_string().contains("hex:"), "{err}");
    let err = call(json!({}), &target).unwrap_err();
    assert_eq!(err.kind(), "invalid_args");
    // Never over a load-order plugin or into the Data dir (it holds no Starfield.esm here,
    // but the plugin itself is protected).
    let err = call(json!({"FULL": "x"}), &data.join("Source.esm")).unwrap_err();
    assert_eq!(err.kind(), "failed");
    // No load order at all.
    let err = execute(
        "set",
        json!({"form": "OldRifle", "fields": {"FULL": "x"}, "into": target.display().to_string()}),
    )
    .unwrap_err();
    assert!(err.to_string().contains("no load order"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 4: a string set as `hex:` must be one NUL-terminated
/// string, and `set` runs the record rules on what it wrote (as `build` does): a record
/// that fails them leaves the target as it was.
#[test]
fn set_refuses_malformed_strings_and_records_that_fail_validate() {
    let d = dir("set-validate");
    let data = data(&d);
    let target = d.join("P.esp");
    let call = |fields: Value| {
        execute(
            "set",
            json!({
                "form": "OldRifle", "fields": fields, "into": target.display().to_string(),
                "plugins": ["Source.esm", "Override.esp"], "data_dir": data.display().to_string(),
                "no_cache": true,
            }),
        )
    };
    for bad in ["hex:", "hex:4142"] {
        let err = call(json!({ "FULL": bad })).unwrap_err();
        assert_eq!(err.kind(), "invalid_args");
        assert!(err.to_string().contains("NUL-terminated"), "{err}");
    }
    assert!(!target.exists());
    let good = call(json!({"FULL": "Good"})).unwrap();
    assert_eq!(good["data"]["validate"]["passed"], json!(true));
    let before = std::fs::read(&target).unwrap();
    // A string ID's bytes end in 00, so the shape passes; the record rules do not.
    let err = call(json!({"FULL": "hex:69E40200"}))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("fails validate") && err.contains("lstring.copied-id"),
        "{err}"
    );
    assert_eq!(std::fs::read(&target).unwrap(), before);
    let _ = std::fs::remove_dir_all(&d);
}

/// Until lane D's round 2: a load order with a plugin before its master (or missing one)
/// is refused as a source, and so is a deleted winning version.
#[test]
fn unverified_load_orders_and_deleted_records_are_refused() {
    let d = dir("set-unsafe");
    let data = data(&d);
    let target = d.join("P.esp");
    let run = |plugins: Value, form: &str| {
        execute(
            "set",
            json!({
                "form": form, "fields": {"FULL": "x"}, "into": target.display().to_string(),
                "plugins": plugins, "data_dir": data.display().to_string(), "no_cache": true,
            }),
        )
    };
    let err = run(json!(["Override.esp", "Source.esm"]), "OldRifle").unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("load order unverified") && text.contains("Override.esp loads before"),
        "{text}"
    );
    let err = run(json!(["Override.esp"]), "Override.esp:000801").unwrap_err();
    assert!(err.to_string().contains("misses master"), "{err}");
    // A deleted winner.
    let mut gone = weapon(0x0000_0800, "OldRifle", 0x0002_E469);
    gone.header.flags |= 0x20;
    write_plugin(&data, "Delete.esp", &["Source.esm"], 0, vec![gone]);
    let err = run(json!(["Source.esm", "Delete.esp"]), "Source.esm:000800").unwrap_err();
    assert!(err.to_string().contains("deleted"), "{err}");
    assert!(!target.exists());
    let _ = std::fs::remove_dir_all(&d);
}
