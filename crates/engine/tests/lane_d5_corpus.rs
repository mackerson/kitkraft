//! Lane D round 5 on real data (`KITKRAFT_DATA_DIR`; skips when unset), through one
//! `Engine` session and `execute` only: the five authoring gaps the prototype lanes hit
//! (`docs/recipes/mini-site.md`, `prototypes/foothold/*/README.md`), each from the wave 2
//! review's repro:
//!
//! 1. `set` adds a subrecord the record lacks at its schema slot: `CELL` `XLCN` (the
//!    review's `kit set Starfield.esm:127392 --fields '{"XLCN":...}' --dry-run`), `ACTI`
//!    `KWDA` (with its `KSIZ`), `LCTN` `PNAM`.
//! 2. Arrays take typed whole values: `[[new]] LCTN KWDA = ["Starfield.esm:0234F1"]`, a
//!    `FACT` `PRIS` of struct entries, a `WEAP`'s `DAMA`, `APPR` and `KWDA` (its `KSIZ`
//!    follows); a partial element in `hex:` is still refused.
//! 3. `[[clone]]` gains a subrecord its source lacks: `03CCF4` with `KSIZ`/`KWDA`, the
//!    template location `2A39CC` with `PNAM`.
//! 4. `new:` names a `[[place]]`: a location's `MNAM` and a `[[place]]`'s `XLKR` to a
//!    reference placed after it.
//! 5. `place --dry-run` previews the reference and its cell without writing.
//!
//! The built plugin passes `validate` with 0 errors and 0 warnings, round-trips
//! identically, and `refs` finds no dangling link from the new location. Everything is
//! written under the OS temp dir. Run in release:
//! `cargo test -p kitkraft-engine --release --test lane_d5_corpus -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::{Path, PathBuf};

use kitkraft_engine::Engine;
use serde_json::{json, Value};

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

/// `(signature, payload hex)` of every subrecord of an `inspect` version entry.
fn subs(version: &Value) -> Vec<(String, String)> {
    version["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["signature"].as_str().unwrap().to_string(),
                s["payload_hex"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

fn payload(list: &[(String, String)], sig: &str) -> String {
    list.iter()
        .find(|(s, _)| s == sig)
        .map(|(_, p)| p.clone())
        .unwrap_or_else(|| panic!("no {sig} in {list:?}"))
}

/// The signature right before `sig` in `list`.
fn before(list: &[(String, String)], sig: &str) -> String {
    let i = list.iter().position(|(s, _)| s == sig).unwrap();
    list[i - 1].0.clone()
}

/// A `set` dry run on the loaded `Starfield.esm`: its one `after` record.
fn set_dry(engine: &Engine, form: &str, fields: Value, into: &Path) -> Vec<(String, String)> {
    let r = engine
        .execute(
            "set",
            json!({"form": form, "fields": fields, "into": into.display().to_string(),
                   "dry_run": true}),
        )
        .unwrap_or_else(|e| panic!("set {form}: {e}"));
    let d = &r["data"];
    assert_eq!(d["would_write"], json!(true), "{form}: {:#}", d["validate"]);
    assert_eq!(
        d["validate"]["errors"],
        json!(0),
        "{form}: {:#}",
        d["validate"]
    );
    assert!(!into.exists(), "a dry run wrote {}", into.display());
    subs(&d["after"][0])
}

/// The last version of `form` in `Starfield.esm` + `plugin`.
fn built(engine: &Engine, data: &str, plugin: &Path, form: &str) -> Vec<(String, String)> {
    let r = engine
        .execute(
            "inspect",
            json!({"form": form, "plugins": ["Starfield.esm", plugin.display().to_string()],
                   "data_dir": data}),
        )
        .unwrap_or_else(|e| panic!("inspect {form}: {e}"));
    let versions = r["data"]["versions"].as_array().unwrap();
    subs(versions.last().unwrap())
}

const SPEC: &str = r#"version = 3

[load_order]
plugins = ["Starfield.esm"]
DATA_DIR

# Gap 3, the review's repro: a clone of ActivatorSwitch01 (no KWDA) gains KSIZ and KWDA.
[[clone]]
name = "switch"
form = "Starfield.esm:03CCF4"
edid = "KitKraftD5Switch"
fields = { KSIZ = 1, KWDA = "hex:F1340200" }

# Gap 3: OutpostOverlayTemplateLocation (no PNAM) cloned with a parent.
[[clone]]
name = "template"
form = "Starfield.esm:2A39CC"
edid = "KitKraftD5TemplateCopy"
fields = { PNAM = "Starfield.esm:01B38B" }

# Gap 2, the review's repro, and gap 4: typed KWDA, MNAM naming a [[place]].
[[new]]
name = "location"
type = "LCTN"
edid = "KitKraftD5Location"
fields = { KWDA = ["Starfield.esm:0234F1"], MNAM = "new:marker", PNAM = "Starfield.esm:01295A" }

# Gap 2: a whole PRIS of struct entries (CrimeFactionUC's first prison, 05BD93), one
# naming the reference this build places.
[[new]]
name = "faction"
type = "FACT"
edid = "KitKraftD5Faction"
fields = { PRIS = [{ PrisonDoorMarker = "Starfield.esm:2B5BF7", ShipLandingMarker = "Starfield.esm:023605", CargoContraband = "Starfield.esm:2CD25D", InventoryContraband = "Starfield.esm:2C031E", CompanionWaitMarker = "new:marker" }] }

# Gap 2 on WEAP (Tombstone): DAMA of struct entries, APPR of FormIDs, KWDA (17 -> 1;
# its KSIZ follows).
[[set]]
form = "Starfield.esm:02EB36"
fields = { DAMA = [{ Type = "Starfield.esm:208DCD", Amount = 7, CurveTable = "00000000" }], APPR = ["Starfield.esm:024004", "Starfield.esm:1E32C8"], KWDA = ["Starfield.esm:01FC18"] }

# Gap 1 and 4: the Crucible cell (1, -2), which has no XLCN, names the new location.
[[set]]
form = "Starfield.esm:127392"
fields = { XLCN = "new:location" }

# Gap 4: a [[place]] linking a reference placed after it.
[[place]]
name = "beacon"
base = "Starfield.esm:000034"
cell = "Starfield.esm:21966F"
position = [-19.0, -4.0, 4.63]
persistent = true
edid = "KitKraftD5Beacon"
fields = { "XLKR.Keyword" = "Starfield.esm:038C0B", "XLKR.Ref" = "new:marker" }

[[place]]
name = "marker"
base = "Starfield.esm:000034"
cell = "Starfield.esm:21966F"
position = [-21.0, -4.0, 4.63]
persistent = true
edid = "KitKraftD5Marker"
"#;

#[test]
#[ignore = "release: indexes Starfield.esm (cached after the first run)"]
fn the_review_repros_now_author() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let data = dir.display().to_string();
    let out = std::env::temp_dir().join(format!("kitkraft-lane-d5-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm"], "data_dir": data}),
        )
        .unwrap();
    let scratch = out.join("x.esp");

    // 1. set adds the subrecord at its schema slot (dry runs; nothing written).
    let cell = set_dry(
        &engine,
        "Starfield.esm:127392",
        json!({"XLCN": "Starfield.esm:299F69"}),
        &scratch,
    );
    assert_eq!(payload(&cell, "XLCN"), "699F2900");
    assert_eq!(before(&cell, "XLCN"), "XILS", "{cell:?}");
    let switch = set_dry(
        &engine,
        "Starfield.esm:03CCF4",
        json!({"KWDA": ["Starfield.esm:0234F1"]}),
        &scratch,
    );
    assert_eq!(payload(&switch, "KSIZ"), "01000000");
    assert_eq!(payload(&switch, "KWDA"), "F1340200");
    assert_eq!(before(&switch, "KWDA"), "KSIZ");
    assert_eq!(before(&switch, "KSIZ"), "FLLD", "{switch:?}");
    let template = set_dry(
        &engine,
        "Starfield.esm:2A39CC",
        json!({"PNAM": "Starfield.esm:01B38B"}),
        &scratch,
    );
    assert_eq!(payload(&template, "PNAM"), "8BB30100");
    // A partial element is still refused by the layout check.
    let err = engine
        .execute(
            "set",
            json!({"form": "Starfield.esm:03CCF4", "fields": {"KWDA": "hex:F134020000"},
                   "into": scratch.display().to_string(), "dry_run": true}),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("4-byte elements"), "{err}");

    // 5. place --dry-run.
    let r = engine
        .execute(
            "place",
            json!({"base": "Starfield.esm:000034", "cell": "Starfield.esm:21966F",
                   "position": [-19.0, -4.0, 4.63], "persistent": true,
                   "into": scratch.display().to_string(), "dry_run": true}),
        )
        .unwrap_or_else(|e| panic!("place dry run: {e}"));
    let d = &r["data"];
    assert_eq!(d["dry_run"], json!(true));
    assert_eq!(d["would_write"], json!(true), "{:#}", d["validate"]);
    assert_eq!(
        d["placed"][0]["cell"]["form"],
        json!("Starfield.esm:21966F")
    );
    assert_eq!(d["after"][0]["record"]["signature"], json!("REFR"));
    assert_eq!(d["carried"].as_array().unwrap().len(), 1);
    assert!(!scratch.exists());

    // 2, 3 and 4 in one build.
    let spec = out.join("d5.toml");
    std::fs::write(
        &spec,
        SPEC.replace("DATA_DIR", &format!("data_dir = '{}'", dir.display())),
    )
    .unwrap();
    let plugin = out.join("KitKraftLaneD5.esm");
    let r = engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": plugin.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("build: {e}"));
    let b = &r["data"];
    assert_eq!(b["passed"], json!(true), "{:#}", b["validate"]);
    eprintln!("{}", b["summary"].as_str().unwrap_or(""));
    // FormIDs: clones, then news, then places, from 0x1000.
    let form = |id: u32| format!("KitKraftLaneD5.esm:{id:06X}");
    let switch = built(&engine, &data, &plugin, &form(0x1000));
    assert_eq!(payload(&switch, "KSIZ"), "01000000");
    assert_eq!(payload(&switch, "KWDA"), "F1340200");
    let template = built(&engine, &data, &plugin, &form(0x1001));
    assert_eq!(payload(&template, "PNAM"), "8BB30100");
    let location = built(&engine, &data, &plugin, &form(0x1002));
    assert_eq!(payload(&location, "KSIZ"), "01000000");
    assert_eq!(payload(&location, "KWDA"), "F1340200");
    assert_eq!(payload(&location, "MNAM"), "05100001", "{location:?}");
    let faction = built(&engine, &data, &plugin, &form(0x1003));
    assert_eq!(
        payload(&faction, "PRIS"),
        "F75B2B00053602005DD22C001E032C0005100001"
    );
    let weapon = built(&engine, &data, &plugin, "Starfield.esm:02EB36");
    assert_eq!(payload(&weapon, "DAMA"), "CD8D20000700000000000000");
    assert_eq!(payload(&weapon, "APPR"), "04400200C8321E00");
    assert_eq!(payload(&weapon, "KSIZ"), "01000000");
    assert_eq!(payload(&weapon, "KWDA"), "18FC0100");
    let cell = built(&engine, &data, &plugin, "Starfield.esm:127392");
    assert_eq!(payload(&cell, "XLCN"), "02100001");
    let beacon = built(&engine, &data, &plugin, &form(0x1004));
    assert_eq!(payload(&beacon, "XLKR"), "0B8C030005100001");

    // validate 0/0, roundtrip identical, no dangling link from the new forms.
    let v = engine
        .execute("validate", json!({"plugin": plugin.display().to_string()}))
        .unwrap();
    assert_eq!(v["data"]["errors"], json!(0), "{:#}", v["data"]);
    assert_eq!(v["data"]["warnings"], json!(0), "{:#}", v["data"]);
    let rt = engine
        .execute(
            "roundtrip",
            json!({"plugin": plugin.display().to_string(), "verify": true}),
        )
        .unwrap();
    assert_eq!(rt["data"]["identical"], json!(true), "{:#}", rt["data"]);
    for id in [0x1000, 0x1001, 0x1002, 0x1003, 0x1004] {
        let r = engine
            .execute(
                "refs",
                json!({"form": form(id), "plugins": ["Starfield.esm", plugin.display().to_string()],
                       "data_dir": data}),
            )
            .unwrap();
        let by_status = r["data"]["counts"]["by_status"].as_object().unwrap();
        assert!(
            by_status.keys().all(|k| k == "resolved" || k == "null"),
            "{}: {by_status:?}",
            form(id)
        );
    }
    let _ = std::fs::remove_dir_all(&out);
}
