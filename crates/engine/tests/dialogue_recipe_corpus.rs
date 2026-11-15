//! KK-279: the dialogue recipe (`docs/recipes/dialogue.toml`) built on the official
//! `Starfield.esm` (`KITKRAFT_DATA_DIR`; skips when unset), through one `Engine` and
//! `execute` only, and every claim of `docs/recipes/dialogue.md` checked: the four new
//! forms and their group paths, the topic's `QNAM`, `TIFL` and `TIFC`, the response texts
//! read back through `inspect`, `validate` with 0 errors, `roundtrip` identical, and a
//! second build byte-identical, and its SHA-256 (as `testplan` reports it) the one the
//! recipe records.
//!
//! Run in release:
//! `cargo test -p kitkraft-engine --release --test dialogue_recipe_corpus -- --include-ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::{Path, PathBuf};

use kitkraft_engine::Engine;
use kitkraft_esm::{Entry, Plugin};
use serde_json::json;

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)
}

/// The recipe's spec with the Data folder named in its `[load_order]`.
fn spec_for(dir: &Path, out: &Path) -> PathBuf {
    let text = std::fs::read_to_string(repo("docs/recipes/dialogue.toml")).unwrap();
    let anchor = "plugins = [\"Starfield.esm\"]\n";
    assert!(text.contains(anchor), "the spec's [load_order] changed");
    let text = text.replacen(
        anchor,
        &format!("{anchor}data_dir = '{}'\n", dir.display()),
        1,
    );
    let spec = out.join("dialogue.toml");
    std::fs::write(&spec, text).unwrap();
    spec
}

/// A record's group path, signature and FormID.
type Shape<'a> = (Vec<(i32, u32)>, &'a str, u32);

/// A record of the built plugin: group path, signature, FormID, subrecords.
type Row = (Vec<(i32, u32)>, String, u32, Vec<([u8; 4], Vec<u8>)>);

fn rows(bytes: &[u8]) -> Vec<Row> {
    fn walk(entries: &[Entry<'_>], path: &mut Vec<(i32, u32)>, out: &mut Vec<Row>) {
        for e in entries {
            match e {
                Entry::Record(r) => out.push((
                    path.clone(),
                    r.header.signature.to_string(),
                    r.header.form_id,
                    r.subrecords()
                        .unwrap()
                        .iter()
                        .map(|s| (s.signature().0, s.data().to_vec()))
                        .collect(),
                )),
                Entry::Group(g) => {
                    path.push((g.header.group_type, g.header.label_u32()));
                    walk(g.children(), path, out);
                    path.pop();
                }
            }
        }
    }
    let plugin = Plugin::parse(bytes).unwrap();
    let mut out = Vec::new();
    walk(plugin.entries(), &mut Vec::new(), &mut out);
    out
}

fn sub<'a>(row: &'a Row, sig: &[u8; 4]) -> &'a [u8] {
    row.3
        .iter()
        .find(|(s, _)| s == sig)
        .map(|(_, d)| d.as_slice())
        .unwrap()
}

/// The SHA-256 and size `docs/recipes/dialogue.md` records.
const SHA256: &str = "ab474168b7593a37ac0f34a4a7f0777a7467269a984cf2e4978b64a87a8178e2";

#[test]
#[ignore = "reads Starfield.esm; run in release"]
fn the_dialogue_recipe_builds_and_every_claim_holds() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("kk-dialogue-recipe");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let spec = spec_for(&dir, &base);
    let out = base.join("KitKraftRecipeDialogue.esp");
    let engine = Engine::with_default_handlers();
    let build = |overwrite: bool| {
        engine
            .execute(
                "build",
                json!({"spec": spec.display().to_string(), "out": out.display().to_string(),
                       "overwrite": overwrite}),
            )
            .unwrap_or_else(|e| panic!("build: {e}"))
    };
    let r = build(false);
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    assert_eq!(
        r["data"]["masters"],
        json!(["Starfield.esm"]),
        "{:#}",
        r["data"]
    );
    let first = std::fs::read(&out).unwrap();

    // The four forms, their group paths and the topic's links.
    let rows = rows(&first);
    let qust = (0, u32::from_le_bytes(*b"QUST"));
    let shape: Vec<Shape<'_>> = rows
        .iter()
        .map(|r| (r.0.clone(), r.1.as_str(), r.2))
        .collect();
    assert_eq!(
        shape,
        vec![
            (vec![qust], "QUST", 0x0100_1000),
            (vec![qust, (10, 0x0100_1000)], "DIAL", 0x0100_1001),
            (
                vec![qust, (10, 0x0100_1000), (7, 0x0100_1001)],
                "INFO",
                0x0100_1002
            ),
            (
                vec![qust, (10, 0x0100_1000), (7, 0x0100_1001)],
                "INFO",
                0x0100_1003
            ),
        ]
    );
    let topic = &rows[1];
    assert_eq!(sub(topic, b"QNAM"), 0x0100_1000u32.to_le_bytes());
    let tifl: Vec<u8> = [0x0100_1002u32.to_le_bytes(), 0x0100_1003u32.to_le_bytes()].concat();
    assert_eq!(sub(topic, b"TIFL"), tifl.as_slice());
    assert_eq!(sub(topic, b"TIFC"), 2u32.to_le_bytes());
    assert_eq!(sub(topic, b"SNAM"), b"CUST");
    assert_eq!(sub(&rows[2], b"NAM1"), b"Hello, traveller.\0");
    assert_eq!(sub(&rows[3], b"NAM1"), b"Safe travels.\0");

    // Read back through a load order with the plugin: the response text by EditorID.
    let i = engine
        .execute(
            "inspect",
            json!({"form": "KitKraftRecipeHello",
                   "plugins": ["Starfield.esm", out.display().to_string()],
                   "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    assert!(
        i["data"].to_string().contains("Hello, traveller."),
        "{:#}",
        i["data"]
    );

    // validate, roundtrip, a second build.
    let v = engine
        .execute(
            "validate",
            json!({"plugin": out.display().to_string(), "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    assert_eq!(v["data"]["errors"], json!(0), "{:#}", v["data"]);
    let rt = engine
        .execute(
            "roundtrip",
            json!({"plugin": out.display().to_string(), "verify": true}),
        )
        .unwrap();
    assert_eq!(rt["data"]["identical"], json!(true), "{:#}", rt["data"]);
    build(true);
    let second = std::fs::read(&out).unwrap();
    assert_eq!(first, second, "a second build differs");
    // The plugin's SHA-256 as `testplan` reports it (as the terminal recipe's test).
    let plan = base.join("test");
    let planned = engine
        .execute(
            "testplan",
            json!({"plugin": out.display().to_string(), "out": plan.display().to_string(),
                   "plugins": ["Starfield.esm"], "data_dir": dir.display().to_string(),
                   "spec": spec.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("testplan: {e}"));
    let sha = planned["data"]["sha256"].as_str().map(str::to_string);
    eprintln!(
        "dialogue recipe: {} bytes, sha256 {sha:?}, validate {} warning(s)",
        first.len(),
        v["data"]["warnings"]
    );
    assert_eq!(
        sha.as_deref(),
        Some(SHA256),
        "update docs/recipes/dialogue.md"
    );
    let _ = std::fs::remove_dir_all(&base);
}
