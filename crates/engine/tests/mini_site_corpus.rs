//! M5.D7: the Gate K0 mini-plugin recipe (`docs/recipes/mini-site.toml`) built on the
//! official `Starfield.esm` (`KITKRAFT_DATA_DIR`; skips when unset), through one `Engine`
//! session and `execute` only, and every claim of `docs/recipes/mini-site.md` checked:
//!
//! - `papyrus.compile` compiles the scripts recipe's `KitKraft:HelloOnLoad` next to the
//!   plugin (skips when the game's Papyrus compiler is not installed);
//! - `build` writes ten new forms (KYWD, TMLM, TERM, PKIN and ACTI clones, a new FACT and
//!   LCTN, three persistent REFRs) into the cell `CityNewAtlantisApartmentTEMPLATE`, which
//!   it overrides to gain `XLCN` (lane D round 5), `Starfield.esm` the only master,
//!   validated;
//! - the links: the location's keywords (`LocTypeOutpost` and the clone, a typed list) and
//!   persistent references (`LCPR`, the three placed references), parent
//!   (`CityNewAtlantisLocation`) and faction; the cell's location; the switch's keyword
//!   (a subrecord its vanilla source lacks); the terminal's menu; each reference's base;
//!   the activator's script and its two property values; and no outgoing link of any new
//!   form is dangling (`refs`);
//! - `validate` 0 errors and 0 warnings; `roundtrip --verify` identical; a second build is
//!   byte-identical, and its SHA-256 is the one `docs/recipes/mini-site.md` records;
//! - `testplan` writes `checklist.md`, `kitkraft-test.txt` and `probes.toml` with a probe
//!   for every new form.
//!
//! Everything is written under the OS temp dir. Run in release:
//! `cargo test -p kitkraft-engine --release --test mini_site_corpus -- --ignored --nocapture`

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
    let text = std::fs::read_to_string(repo("docs/recipes/mini-site.toml")).unwrap();
    let anchor = "plugins = [\"Starfield.esm\"]\n";
    assert!(text.contains(anchor), "the spec's [load_order] changed");
    let text = text.replacen(
        anchor,
        &format!("{anchor}data_dir = '{}'\n", dir.display()),
        1,
    );
    let spec = into.join("mini-site.toml");
    std::fs::write(&spec, text).unwrap();
    spec
}

/// The new forms the recipe claims, in FormID order: (object ID, type, EditorID).
const FORMS: [(u32, &str, &str); 10] = [
    (0x1000, "KYWD", "KitKraftLocTypeMiniSite"),
    (0x1001, "TMLM", "KitKraftMiniSiteMenu"),
    (0x1002, "TERM", "KitKraftMiniSiteTerminal"),
    (0x1003, "PKIN", "KitKraftMiniSitePackIn"),
    (0x1004, "ACTI", "KitKraftMiniSiteSwitch"),
    (0x1005, "FACT", "KitKraftMiniSiteFaction"),
    (0x1006, "LCTN", "KitKraftMiniSiteLocation"),
    (0x1007, "REFR", "KitKraftMiniSiteSwitchRef"),
    (0x1008, "REFR", "KitKraftMiniSiteTerminalRef"),
    (0x1009, "REFR", "KitKraftMiniSitePackInRef"),
];

/// SHA-256 of the built plugin (lane D round 5), as `docs/recipes/mini-site.md` records it.
const SHA256: &str = "05aac03135515034e4efb8428d35bcaae3dfb9217d9a332141a802818d495d80";

fn form(id: u32) -> String {
    format!("KitKraftMiniSite.esm:{id:06X}")
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
    // `null` is a FormID 0 the source record holds (a vanilla terminal's marker keyword).
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

#[test]
#[ignore = "release: indexes Starfield.esm (cached after the first run); needs the game's Papyrus compiler"]
fn the_mini_site_recipe_builds_and_every_claim_holds() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let data = dir.display().to_string();
    let out = std::env::temp_dir().join(format!("kitkraft-mini-site-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let engine = Engine::with_default_handlers();

    // The script the activator names, compiled next to the plugin.
    let zip = dir.join("../Tools/ContentResources.zip");
    let compiled = engine.execute(
        "papyrus.compile",
        json!({
            "source": repo("docs/recipes/scripts/KitKraft/HelloOnLoad.psc").display().to_string(),
            "out_dir": out.join("Scripts").display().to_string(),
            "sources_zip": zip.display().to_string(),
        }),
    );
    match compiled {
        Ok(r) if r["data"]["passed"] == json!(true) => {}
        other => {
            eprintln!("skip: the Papyrus compiler did not run: {other:?}");
            let _ = std::fs::remove_dir_all(&out);
            return;
        }
    }

    // Build.
    let spec = spec_for(&dir, &out);
    let plugin = out.join("KitKraftMiniSite.esm");
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
    assert_eq!(placed.len(), 3);
    for p in placed {
        assert_eq!(p["cell"]["form"], json!("Starfield.esm:21966F"));
        assert_eq!(p["persistent"], json!(true));
    }
    let carried = b["carried"].as_array().unwrap();
    assert_eq!(carried.len(), 1, "{carried:#?}");
    let bytes = std::fs::read(&plugin).unwrap();
    eprintln!(
        "KitKraftMiniSite.esm: {} bytes, {}",
        bytes.len(),
        b["summary"].as_str().unwrap_or("")
    );

    // Links: the claims of docs/recipes/mini-site.md, "What the plugin links".
    let plugins = json!(["Starfield.esm", plugin.display().to_string()]);
    let location = links(&engine, &plugins, &data, &form(0x1006));
    assert_eq!(
        target(&location, "KWDA.Forms[0]"),
        json!("Starfield.esm:0234F1")
    );
    assert_eq!(target(&location, "KWDA.Forms[1]"), json!(form(0x1000)));
    assert_eq!(
        target(&location, "PNAM.ParentLocation"),
        json!("Starfield.esm:01295A")
    );
    assert_eq!(
        target(&location, "FNAM.UnreportedCrimeFaction"),
        json!(form(0x1005))
    );
    // Lane D round 5: the location lists its references, the cell names the location, and
    // the switch carries the site keyword.
    for (i, reference) in [0x1007, 0x1008, 0x1009].into_iter().enumerate() {
        assert_eq!(
            target(&location, &format!("LCPR.PersistentRefs[{i}].Ref")),
            json!(form(reference))
        );
        assert_eq!(
            target(&location, &format!("LCPR.PersistentRefs[{i}].WorldOrCell")),
            json!("Starfield.esm:21966F")
        );
    }
    let cell = links(&engine, &plugins, &data, "Starfield.esm:21966F");
    assert_eq!(target(&cell, "XLCN.Location"), json!(form(0x1006)));
    let switch = links(&engine, &plugins, &data, &form(0x1004));
    assert_eq!(target(&switch, "KWDA.Forms[0]"), json!(form(0x1000)));
    let terminal = links(&engine, &plugins, &data, &form(0x1002));
    assert_eq!(target(&terminal, "DNAM.TerminalMenu"), json!(form(0x1001)));
    for (reference, base) in [(0x1007, 0x1004), (0x1008, 0x1002), (0x1009, 0x1003)] {
        let edges = links(&engine, &plugins, &data, &form(reference));
        assert_eq!(target(&edges, "NAME.Base"), json!(form(base)));
    }
    for (id, _, _) in FORMS {
        links(&engine, &plugins, &data, &form(id));
    }

    // The activator's script and its property values.
    let switch = engine
        .execute(
            "inspect",
            json!({"form": form(0x1004), "plugins": plugins, "data_dir": data, "full": true}),
        )
        .unwrap();
    let text = switch["data"].to_string();
    assert!(text.contains("KitKraft:HelloOnLoad"), "{text}");
    assert!(text.contains("KitKraft: mini site switch loaded"), "{text}");

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
    let again = out.join("again").join("KitKraftMiniSite.esm");
    std::fs::create_dir_all(again.parent().unwrap()).unwrap();
    std::fs::create_dir_all(out.join("again").join("Scripts").join("KitKraft")).unwrap();
    std::fs::copy(
        out.join("Scripts/KitKraft/HelloOnLoad.pex"),
        out.join("again/Scripts/KitKraft/HelloOnLoad.pex"),
    )
    .unwrap();
    engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": again.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("second build: {e}"));
    assert_eq!(std::fs::read(&again).unwrap(), bytes, "builds differ");

    // testplan: the three files, a probe for every new form.
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
    // The plugin's SHA-256, as docs/recipes/mini-site.md records it.
    assert_eq!(
        planned["data"]["sha256"],
        json!(SHA256),
        "the plugin changed: record the new hash in docs/recipes/mini-site.md"
    );
    let probes: toml::Table =
        toml::from_str(&std::fs::read_to_string(plan.join("probes.toml")).unwrap()).unwrap();
    assert_eq!(probes["plugin"].as_str(), Some("KitKraftMiniSite.esm"));
    let rows = probes["probe"].as_array().unwrap();
    for (id, _, edid) in FORMS {
        let runtime = format!("0100{id:04X}");
        assert!(
            rows.iter().any(|p| p["forms"]
                .as_array()
                .is_some_and(|f| f.iter().any(|x| x.as_str() == Some(&runtime)))),
            "no probe for {edid} ({runtime})"
        );
    }
    eprintln!("probes.toml: {} probes", rows.len());
    let _ = std::fs::remove_dir_all(&out);
}
