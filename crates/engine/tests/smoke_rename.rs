//! M2.E10 rename smoke test (`docs/smoke-rename.md`): the Old Earth Hunting Rifle becomes
//! the "Old Soviet Marksman Rifle" in a new plugin, through `execute` exactly as `kit` and
//! MCP call it. Reads the official `Starfield.esm` in place through `KITCRAFT_DATA_DIR`
//! (skipped when unset); writes only to a temp directory (and lane D's index cache).

#![allow(clippy::arithmetic_side_effects)]

use std::path::PathBuf;

use kitcraft_engine::execute;
use serde_json::{json, Value};

fn data_dir() -> Option<PathBuf> {
    match std::env::var_os("KITCRAFT_DATA_DIR").map(PathBuf::from) {
        Some(dir) if dir.join("Starfield.esm").is_file() => Some(dir),
        _ => {
            println!(
                "skipped: KITCRAFT_DATA_DIR is unset or has no Starfield.esm (docs/corpus.md)"
            );
            None
        }
    }
}

const NAME: &str = "Old Soviet Marksman Rifle";

#[test]
fn rename_the_old_earth_hunting_rifle() {
    let Some(data) = data_dir() else { return };
    let data_text = data.display().to_string();
    let out = std::env::temp_dir().join(format!("kitcraft-smoke-rename-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let lo = json!({"plugins": ["Starfield.esm"], "data_dir": data_text});
    let with = |extra: Value| {
        let mut args = lo.clone();
        for (k, v) in extra.as_object().unwrap() {
            args[k] = v.clone();
        }
        args
    };

    // 1-2. Find the weapon by its (localized) name.
    let q = execute(
        "query",
        with(json!({"type": "WEAP", "where": "FULL ~ 'Old Earth Hunting Rifle'"})),
    )
    .unwrap();
    assert_eq!(q["data"]["matched"], json!(1), "{:#}", q["data"]);
    let hit = &q["data"]["results"][0];
    assert_eq!(hit["form"], json!("Starfield.esm:21BBCD"), "{hit:#}");
    assert_eq!(hit["editor_id"], json!("RussianHuntingRifle"));

    // 3. set, and build from a spec: the same bytes.
    let by_set = out.join("OldSovietMarksmanRifle.esm");
    let s = execute(
        "set",
        with(json!({
            "form": "Starfield.esm:0021BBCD",
            "fields": {"FULL": NAME},
            "into": by_set.display().to_string(),
        })),
    )
    .unwrap();
    let rec = &s["data"]["records"][0];
    assert_eq!(s["data"]["masters"], json!(["Starfield.esm"]));
    assert_eq!(s["data"]["flags"], json!("0x00000001"));
    assert_eq!(
        rec["fields"][0]["before"],
        json!({"string_id": "0x0002E469"})
    );
    assert_eq!(rec["fields"][0]["after"], json!(NAME));
    assert_eq!(rec["editor_id"], json!("RussianHuntingRifle"));
    // Every other lstring of the copy is inline text from starfield_en.
    assert_eq!(rec["inlined_lstrings"].as_array().unwrap().len(), 8);
    let spec = out.join("rename.toml");
    std::fs::write(
        &spec,
        format!(
            "version = 1\n[plugin]\nlocalized = false\n[load_order]\nplugins = [\"Starfield.esm\"]\ndata_dir = '{data_text}'\n[[set]]\nform = \"Starfield.esm:0021BBCD\"\nfields = {{ FULL = \"{NAME}\" }}\n"
        ),
    )
    .unwrap();
    let built_dir = out.join("built");
    std::fs::create_dir_all(&built_dir).unwrap();
    let by_build = built_dir.join("OldSovietMarksmanRifle.esm");
    execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": by_build.display().to_string()}),
    )
    .unwrap();
    let bytes = std::fs::read(&by_set).unwrap();
    assert_eq!(bytes, std::fs::read(&by_build).unwrap());

    // 4. validate passes, lstring.copied-id included.
    let v = execute(
        "validate",
        with(json!({"plugin": by_set.display().to_string()})),
    )
    .unwrap();
    assert_eq!(v["data"]["passed"], json!(true), "{:#}", v["data"]);
    assert_eq!(v["data"]["errors"], json!(0));
    assert!(v["data"]["rules"]
        .as_array()
        .unwrap()
        .contains(&json!("lstring.copied-id")));

    // 5. With the plugin appended, its name wins.
    let i = execute(
        "inspect",
        json!({
            "form": "Starfield.esm:0021BBCD",
            "plugins": ["Starfield.esm", by_set.display().to_string()],
            "data_dir": data_text,
        }),
    )
    .unwrap();
    let versions = i["data"]["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 2);
    let winner = versions.last().unwrap();
    assert_eq!(winner["winner"], json!(true));
    assert_eq!(winner["plugin"], json!("OldSovietMarksmanRifle.esm"));
    let full = winner["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == "FULL")
        .unwrap();
    assert_eq!(full["text"], json!(NAME));
    assert_eq!(i["data"]["runtime_id"], json!("0021BBCD"));

    // 6. roundtrip --verify is identical.
    let r = execute(
        "roundtrip",
        json!({"plugin": by_set.display().to_string(), "verify": true}),
    )
    .unwrap();
    assert_eq!(r["data"]["identical"], json!(true));

    // The Resupply mistake, for contrast: the master's string ID copied as is fails the
    // record rules `set` runs before it writes, so nothing is written.
    let wrong = out.join("CopiedId.esp");
    let err = execute(
        "set",
        with(json!({
            "form": "Starfield.esm:0021BBCD",
            "fields": {"FULL": "hex:69E40200"},
            "into": wrong.display().to_string(),
        })),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("lstring.copied-id") && err.contains("FULL"),
        "{err}"
    );
    assert!(!wrong.exists());

    let _ = std::fs::remove_dir_all(&out);
}
