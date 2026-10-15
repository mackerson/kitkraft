//! Bare input names through `execute` (M4.F1): every command that reads a plugin or
//! archive resolves `Constellation.esm` (or its archive) in `KITKRAFT_DATA_DIR`, and fails
//! with `not_found` naming the places searched when the Data dir does not have it. The
//! "resolves" half reads the official files in place (`docs/corpus.md`) and skips when
//! `KITKRAFT_DATA_DIR` is unset; the "not found" half points `data_dir` at an empty dir,
//! which wins over the environment, so it always runs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use serde_json::{json, Value};

use super::testdata::{self, temp_dir};
use crate::{execute, Engine, EngineError};

const PLUGIN: &str = "Constellation.esm";
const LOCALIZATION: &str = "Constellation - Localization.ba2";
const TABLE: &str = "strings/constellation_en.strings";
const SCRIPTS: &str = "Starfield - Misc.ba2";

/// The Data dir when `KITKRAFT_DATA_DIR` names one holding `Constellation.esm`.
fn data() -> Option<PathBuf> {
    let dir = super::env_var_os(super::DATA_DIR_ENV)
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)?;
    if dir.join(PLUGIN).is_file() {
        Some(dir)
    } else {
        println!("skipped: {PLUGIN} is not in KITKRAFT_DATA_DIR");
        None
    }
}

/// `args` with `data_dir` set to a fresh empty directory.
fn without(name: &str, mut args: Value) -> (Value, String) {
    let empty = temp_dir(&format!("resolve-empty-{name}"));
    args["data_dir"] = json!(empty.display().to_string());
    (args, empty.display().to_string())
}

/// `command` fails with `not_found` for `missing`, naming the empty Data dir it searched.
fn assert_not_found(command: &str, args: Value, missing: &str) {
    let (args, empty) = without(&command.replace('.', "-"), args);
    let err = execute(command, args).unwrap_err();
    let EngineError::NotFound { name, searched, .. } = &err else {
        panic!("{command}: expected not_found, got {err:?}");
    };
    assert_eq!(err.kind(), "not_found");
    assert_eq!(name, missing);
    assert_eq!(searched.len(), 3, "{searched:?}");
    let text = err.to_string();
    assert!(text.contains(missing), "{text}");
    assert!(text.contains("load order"), "{text}");
    assert!(text.contains("working directory"), "{text}");
    assert!(text.contains(&empty), "{text}");
}

#[test]
fn roundtrip_resolves_a_bare_name() {
    if data().is_some() {
        let r = execute("roundtrip", json!({"plugin": PLUGIN, "verify": true})).unwrap();
        assert_eq!(r["data"]["passed"], true);
    }
    assert_not_found("roundtrip", json!({"plugin": PLUGIN}), PLUGIN);
}

#[test]
fn coverage_resolves_a_bare_name() {
    if data().is_some() {
        let r = execute("coverage", json!({"plugin": PLUGIN, "top": 1})).unwrap();
        assert!(r["data"]["records"].as_u64().unwrap() > 0, "{r}");
    }
    assert_not_found("coverage", json!({"plugin": PLUGIN}), PLUGIN);
}

#[test]
fn diff_resolves_bare_names() {
    if data().is_some() {
        let r = execute("diff", json!({"a": PLUGIN, "b": PLUGIN})).unwrap();
        assert!(
            r["data"]["summary"].as_str().unwrap().contains("0 changed"),
            "{r}"
        );
    }
    assert_not_found("diff", json!({"a": PLUGIN, "b": PLUGIN}), PLUGIN);
}

#[test]
fn validate_resolves_a_bare_name() {
    let args = json!({"plugin": PLUGIN, "rules": ["masters.missing"]});
    if data().is_some() {
        let r = execute("validate", args.clone()).unwrap();
        assert_eq!(r["data"]["passed"], true);
    }
    assert_not_found("validate", args, PLUGIN);
}

#[test]
fn inspect_single_plugin_resolves_a_bare_name() {
    let args = json!({"plugin": PLUGIN, "form": "AVM_Equinox_Equinox_Digital_Deluxe"});
    if data().is_some() {
        let r = execute("inspect", args.clone()).unwrap();
        assert_eq!(r["data"]["matched_by"], "editor_id", "{r}");
    }
    assert_not_found("inspect", args, PLUGIN);
}

#[test]
fn conflicts_resolves_a_bare_name() {
    let args = json!({"plugins": [PLUGIN], "no_cache": true});
    if data().is_some() {
        execute("conflicts", args.clone()).unwrap();
    }
    assert_not_found("conflicts", args, PLUGIN);
}

/// `set` and `build` refuse a load order with a missing master; the refusal naming
/// `Starfield.esm` shows `Constellation.esm` itself was found and read.
fn refuses_for_the_master(command: &str, args: Value) {
    let err = execute(command, args).unwrap_err();
    assert_eq!(err.kind(), "failed", "{err}");
    assert!(err.to_string().contains("Starfield.esm"), "{err}");
}

#[test]
fn set_resolves_a_bare_name() {
    let out = temp_dir("resolve-set").join("Patch.esp");
    let args = json!({
        "plugins": [PLUGIN], "no_cache": true, "form": "AVM_Equinox_Equinox_Digital_Deluxe",
        "fields": {"EDID": "X"}, "into": out.display().to_string(),
    });
    if data().is_some() {
        refuses_for_the_master("set", args.clone());
    }
    assert_not_found("set", args, PLUGIN);
    assert!(!out.exists());
}

#[test]
fn build_resolves_a_bare_name() {
    let dir = temp_dir("resolve-build");
    let spec = dir.join("spec.toml");
    // M3.E1: load-order problems refuse the forms they touch, so the spec names one.
    std::fs::write(
        &spec,
        "version = 2\n\n[plugin]\nlocalized = false\n\n[[set]]\nform = \"AVM_Equinox_Equinox_Digital_Deluxe\"\nfields = { EDID = \"X\" }\n",
    )
    .unwrap();
    let out = dir.join("Out.esp");
    let args = json!({
        "spec": spec.display().to_string(), "out": out.display().to_string(),
        "plugins": [PLUGIN], "no_cache": true,
    });
    if data().is_some() {
        refuses_for_the_master("build", args.clone());
    }
    assert_not_found("build", args, PLUGIN);
    assert!(!out.exists());
}

#[test]
fn ba2_list_and_extract_resolve_a_bare_archive() {
    if data().is_some() {
        let r = execute("ba2.list", json!({"archive": LOCALIZATION})).unwrap();
        assert_eq!(r["data"]["matched"], 27, "{r}");
        let out = temp_dir("resolve-extract");
        let r = execute(
            "ba2.extract",
            json!({"archive": LOCALIZATION, "pattern": "_en\\.strings$",
                   "out_dir": out.display().to_string()}),
        )
        .unwrap();
        assert!(out.join(TABLE).is_file(), "{r}");
    }
    assert_not_found("ba2.list", json!({"archive": LOCALIZATION}), LOCALIZATION);
    let out = temp_dir("resolve-extract-missing");
    assert_not_found(
        "ba2.extract",
        json!({"archive": LOCALIZATION, "out_dir": out.display().to_string()}),
        LOCALIZATION,
    );
}

#[test]
fn strings_dump_resolves_a_bare_archive() {
    let args = json!({"archive": LOCALIZATION, "table": TABLE});
    if data().is_some() {
        let r = execute("strings.dump", args.clone()).unwrap();
        assert_eq!(r["data"]["matched"], 4, "{r}");
    }
    assert_not_found("strings.dump", args, LOCALIZATION);
}

/// The official tables live in archives, so `strings.merge` (loose tables only) is shown
/// on tables extracted from one into a `data_dir` of their own.
#[test]
fn strings_merge_resolves_bare_tables() {
    let tables = json!(["constellation_en.strings", "constellation_en.strings"]);
    if let Some(data) = data() {
        let dir = temp_dir("resolve-merge");
        let src = dir.join("tables");
        execute(
            "ba2.extract",
            json!({"archive": data.join(LOCALIZATION).display().to_string(),
                   "pattern": "_en\\.strings$", "out_dir": src.display().to_string()}),
        )
        .unwrap();
        let out = dir.join("merged.strings");
        let r = execute(
            "strings.merge",
            json!({"tables": tables, "out": out.display().to_string(),
                   "data_dir": src.join("strings").display().to_string()}),
        )
        .unwrap();
        assert!(out.is_file(), "{r}");
    }
    let out = temp_dir("resolve-merge-missing").join("merged.strings");
    assert_not_found(
        "strings.merge",
        json!({"tables": tables, "out": out.display().to_string()}),
        "constellation_en.strings",
    );
}

#[test]
fn papyrus_disasm_resolves_a_bare_archive() {
    let args = json!({"archive": SCRIPTS, "pex": "scripts/form.pex"});
    if data().is_some_and(|d| d.join(SCRIPTS).is_file()) {
        let r = execute("papyrus.disasm", args.clone()).unwrap();
        assert!(r["data"]["functions"].as_u64().unwrap() > 0, "{r}");
    }
    assert_not_found("papyrus.disasm", args, SCRIPTS);
}

/// Step 1: in a session with a load order, a bare name is that load order's plugin, even
/// when neither the working directory nor the Data dir has it.
#[test]
fn the_session_load_order_comes_first() {
    let dir = temp_dir("resolve-session");
    let plugin = dir.join("Session.esm");
    std::fs::write(&plugin, testdata::plugin()).unwrap();
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": [plugin.display().to_string()], "no_cache": true}),
        )
        .unwrap();
    let (args, _) = without("session", json!({"plugin": "session.esm", "verify": true}));
    let r = engine.execute("roundtrip", args).unwrap();
    assert_eq!(r["data"]["plugin"], plugin.display().to_string());
    assert_eq!(r["data"]["passed"], true);
    // Without the session it is not found.
    let (args, _) = without("session", json!({"plugin": "session.esm"}));
    assert_eq!(execute("roundtrip", args).unwrap_err().kind(), "not_found");
}

/// Outputs never resolve: a bare `out` is left as given, so the write guards see it.
#[test]
fn outputs_are_not_resolved() {
    let dir = temp_dir("resolve-outputs");
    std::fs::write(dir.join("Taken.esm"), b"x").unwrap();
    let args = super::resolve::inputs(
        None,
        "roundtrip",
        &json!({"plugin": "Taken.esm", "out": "Taken.esm", "data_dir": dir.display().to_string()}),
        &[super::resolve::Input::file("plugin")],
    )
    .unwrap();
    assert_eq!(args["plugin"], dir.join("Taken.esm").display().to_string());
    assert_eq!(args["out"], "Taken.esm");
}
