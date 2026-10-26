//! KK-013: the terminal recipe (`docs/recipes/terminal.toml`) built on the official
//! `Starfield.esm` (`KITKRAFT_DATA_DIR`; skips when unset), through one `Engine` session
//! and `execute` only, and every claim of `docs/recipes/terminal.md` checked:
//!
//! - `build` writes four new forms (two `TMLM` clones, a `TERM` clone and a persistent
//!   `REFR`) into the cell `CityNewAtlantisApartmentTEMPLATE`, `Starfield.esm` the only
//!   master, validated;
//! - the links: the terminal's menu (`DNAM`), the menu item's submenu (`TNAM`), the
//!   reference's base (`NAME`), and the menu's sixteen copied conditions (`CTDA`), which
//!   still name the vanilla quest `UC01_ShipSimQuest`, and no outgoing link of any new
//!   form is dangling (`refs`);
//! - the edited texts and counts read back through `inspect`; `validate` 0 errors and 0
//!   warnings; `roundtrip --verify` identical; a second build is byte-identical, and its
//!   SHA-256 is the one `docs/recipes/terminal.md` records.
//!
//! Everything is written under the OS temp dir. Run in release:
//! `cargo test -p kitkraft-engine --release --test terminal_recipe_corpus -- --include-ignored --exact the_terminal_recipe_builds_and_every_claim_holds --nocapture`

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

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)
}

/// The recipe's spec with the Data folder named in its `[load_order]`.
fn spec_for(dir: &Path, into: &Path) -> PathBuf {
    let text = std::fs::read_to_string(repo("docs/recipes/terminal.toml")).unwrap();
    let anchor = "plugins = [\"Starfield.esm\"]\n";
    assert!(text.contains(anchor), "the spec's [load_order] changed");
    let text = text.replacen(
        anchor,
        &format!("{anchor}data_dir = '{}'\n", dir.display()),
        1,
    );
    let spec = into.join("terminal.toml");
    std::fs::write(&spec, text).unwrap();
    spec
}

/// The new forms the recipe claims, in FormID order: (object ID, type, EditorID).
const FORMS: [(u32, &str, &str); 4] = [
    (0x1000, "TMLM", "KitKraftRecipeTerminalMenu"),
    (0x1001, "TMLM", "KitKraftRecipeTerminalSubmenu"),
    (0x1002, "TERM", "KitKraftRecipeTerminal"),
    (0x1003, "REFR", "KitKraftRecipeTerminalRef"),
];

/// SHA-256 of the built plugin, as `docs/recipes/terminal.md` records it.
const SHA256: &str = "4d3bfba83f28254f046e33f47d4a6ae9d00402395c15c1ec8ffc04188c8645d1";

fn form(id: u32) -> String {
    format!("KitKraftRecipeTerminal.esm:{id:06X}")
}

/// `refs` of one form (outgoing, depth 1): the edges by path; every one resolves or is null.
fn links(engine: &Engine, plugins: &Value, data: &str, form: &str) -> Vec<Value> {
    let r = engine
        .execute(
            "refs",
            json!({"form": form, "plugins": plugins, "data_dir": data}),
        )
        .unwrap_or_else(|e| panic!("refs {form}: {e}"));
    let d = &r["data"];
    let by_status = d["counts"]["by_status"].as_object().unwrap();
    assert!(
        by_status.keys().all(|k| k == "resolved" || k == "null"),
        "{form}: a link that does not resolve: {by_status:?}\n{}",
        d["summary"]
    );
    d["edges"].as_array().unwrap().clone()
}

fn target(edges: &[Value], path: &str) -> Value {
    edges
        .iter()
        .find(|e| e["path"] == json!(path))
        .map(|e| e["to"].clone())
        .unwrap_or_else(|| panic!("no link {path} in {edges:#?}"))
}

/// The decoded value of the `n`-th subrecord of the winning version, in file order
/// (the same order the field path `SIG[n]` counts).
fn subrecord(data: &Value, sig: &str, n: usize) -> Value {
    data["versions"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|v| v["subrecords"].as_array().unwrap().iter())
        .filter(|s| s["signature"] == json!(sig))
        .nth(n)
        .cloned()
        .unwrap_or_else(|| panic!("no {n}-th {sig} subrecord in {data:#?}"))["value"]
        .clone()
}

#[test]
#[ignore = "release: indexes Starfield.esm"]
fn the_terminal_recipe_builds_and_every_claim_holds() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let data = dir.display().to_string();
    let out = std::env::temp_dir().join(format!("kitkraft-terminal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let engine = Engine::with_default_handlers();

    // Build.
    let spec = spec_for(&dir, &out);
    let plugin = out.join("KitKraftRecipeTerminal.esm");
    let built = engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": plugin.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("build: {e}"));
    let b = &built["data"];
    assert_eq!(b["passed"], json!(true), "{:#}", b["validate"]);
    assert_eq!(b["masters"], json!(["Starfield.esm"]));
    let mut written: Vec<(String, String, String)> = b["new_forms"]
        .as_array()
        .unwrap()
        .iter()
        .chain(b["placed"].as_array().unwrap())
        .map(|f| {
            (
                f["form_id"].as_str().unwrap().to_string(),
                f["signature"].as_str().unwrap().to_string(),
                f["editor_id"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    written.sort();
    let want: Vec<(String, String, String)> = FORMS
        .iter()
        .map(|(id, sig, edid)| (format!("0100{id:04X}"), sig.to_string(), edid.to_string()))
        .collect();
    assert_eq!(written, want);
    let placed = b["placed"].as_array().unwrap();
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0]["cell"]["form"], json!("Starfield.esm:21966F"));
    assert_eq!(placed[0]["persistent"], json!(true));
    let bytes = std::fs::read(&plugin).unwrap();
    eprintln!(
        "KitKraftRecipeTerminal.esm: {} bytes, {}",
        bytes.len(),
        b["summary"].as_str().unwrap_or("")
    );

    // Links: the claims of docs/recipes/terminal.md, "What the plugin links".
    let plugins = json!(["Starfield.esm", plugin.display().to_string()]);
    let terminal = links(&engine, &plugins, &data, &form(0x1002));
    assert_eq!(target(&terminal, "DNAM.TerminalMenu"), json!(form(0x1000)));
    let menu = links(&engine, &plugins, &data, &form(0x1000));
    assert_eq!(target(&menu, "TNAM.Submenu"), json!(form(0x1001)));
    // The sixteen conditions the menu clone carries: the schema has no field path into a
    // condition, so the clone copies them as bytes; their quest links survive.
    for i in 0..16 {
        let path = if i == 0 {
            "CTDA.Data.Parameter1.Quest".to_string()
        } else {
            format!("CTDA[{i}].Data.Parameter1.Quest")
        };
        assert_eq!(
            target(&menu, &path),
            json!("Starfield.esm:2BA648"),
            "condition {i} no longer names UC01_ShipSimQuest"
        );
    }
    let reference = links(&engine, &plugins, &data, &form(0x1003));
    assert_eq!(target(&reference, "NAME.Base"), json!(form(0x1002)));
    for (id, _, _) in FORMS {
        links(&engine, &plugins, &data, &form(id));
    }

    // The edited texts and counts, read back.
    let menu = engine
        .execute(
            "inspect",
            json!({"form": form(0x1000), "plugins": plugins, "data_dir": data, "full": true}),
        )
        .unwrap();
    let text = menu["data"].to_string();
    assert!(text.contains("KitKraft Recipe Menu"), "{text}");
    assert!(
        text.contains("This is the KitKraft recipe terminal menu."),
        "{text}"
    );
    assert!(text.contains("Open the Recipe Submenu"), "{text}");
    // The edited occurrences, pinned to their positions: the first body text and the
    // second item text (the submenu item's).
    assert_eq!(
        subrecord(&menu["data"], "BTXT", 0),
        json!("This is the KitKraft recipe terminal menu.")
    );
    assert_eq!(
        subrecord(&menu["data"], "ITXT", 1),
        json!("Open the Recipe Submenu")
    );
    let submenu = engine
        .execute(
            "inspect",
            json!({"form": form(0x1001), "plugins": plugins, "data_dir": data, "full": true}),
        )
        .unwrap();
    assert_eq!(subrecord(&submenu["data"], "TMVT", 0), json!(1));
    assert_eq!(subrecord(&submenu["data"], "BSIZ", 0), json!(1));
    assert!(
        submenu["data"]
            .to_string()
            .contains("This is the KitKraft recipe submenu."),
        "{}",
        submenu["data"]
    );
    let terminal = engine
        .execute(
            "inspect",
            json!({"form": form(0x1002), "plugins": plugins, "data_dir": data, "full": true}),
        )
        .unwrap();
    assert!(
        terminal["data"]
            .to_string()
            .contains("KitKraft Recipe Terminal"),
        "{}",
        terminal["data"]
    );

    // validate, roundtrip, a second build.
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
    let again = out.join("again").join("KitKraftRecipeTerminal.esm");
    std::fs::create_dir_all(again.parent().unwrap()).unwrap();
    engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": again.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("second build: {e}"));
    assert_eq!(std::fs::read(&again).unwrap(), bytes, "builds differ");

    // testplan: the three files, and the plugin's SHA-256 as the recipe records it.
    let plan = out.join("test");
    let planned = engine
        .execute(
            "testplan",
            json!({"plugin": plugin.display().to_string(), "out": plan.display().to_string(),
                   "plugins": ["Starfield.esm"], "data_dir": data,
                   "spec": spec.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("testplan: {e}"));
    for file in ["checklist.md", "kitkraft-test.txt", "probes.toml"] {
        assert!(plan.join(file).is_file(), "{file} missing");
    }
    assert_eq!(
        planned["data"]["sha256"],
        json!(SHA256),
        "the plugin changed: record the new hash in docs/recipes/terminal.md"
    );
    eprintln!("terminal-recipe: ok");
    let _ = std::fs::remove_dir_all(&out);
}
