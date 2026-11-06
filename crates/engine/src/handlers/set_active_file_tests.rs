//! A6 (owner decision 2026-11-05 item 7; ADR 0007 Amendment 4): `set`, `new` and `clone`
//! may write into the editor's active file once it is loaded, when the request says so
//! (`active_file: true`) and the file is the last plugin of a load order given as
//! `plugins`, not an official master, and outside the game's Data folder, the user's My
//! Games folder and MO2. Every other load-order plugin stays refused, and so does every
//! request without the flag. Temp folders only.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::set_tests::data;
use crate::edit::patch_tests::dir;
use crate::{execute, Engine};

/// A scratch load order: `Data\Source.esm` and `Data\Override.esp` (no `Starfield.esm`
/// there), and the active file `work\Patch.esp`, written by a first `set` while it was not
/// loaded (OldRifle 000800 renamed). Returns (root, data, active file).
fn scratch(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let d = dir(&format!("a6-{tag}"));
    let data = data(&d);
    let work = d.join("work");
    // The copied WEAP names `Weapons\a.nif`: planted next to the patch (KK-018).
    let meshes = work.join("meshes").join("weapons");
    std::fs::create_dir_all(&meshes).unwrap();
    std::fs::write(meshes.join("a.nif"), []).unwrap();
    let active = work.join("Patch.esp");
    execute(
        "set",
        json!({
            "form": "Source.esm:000800", "fields": {"FULL": "First"},
            "into": active.display().to_string(),
            "plugins": ["Source.esm", "Override.esp"], "data_dir": data.display().to_string(),
            "no_cache": true,
        }),
    )
    .unwrap();
    (d, data, active)
}

/// An engine whose session holds `plugins` (bare names from `data`, or paths).
fn loaded(data: &Path, plugins: &[String]) -> Engine {
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": plugins, "data_dir": data.display().to_string(), "no_cache": true}),
        )
        .unwrap();
    engine
}

fn order(active: &Path) -> Vec<String> {
    vec![
        "Source.esm".into(),
        "Override.esp".into(),
        active.display().to_string(),
    ]
}

/// `set` FULL on `form` into `into`, on the engine's session.
fn set_full(
    engine: &Engine,
    form: &str,
    full: &str,
    into: &Path,
    active_file: bool,
    dry_run: bool,
) -> Result<Value, String> {
    let mut args =
        json!({"form": form, "fields": {"FULL": full}, "into": into.display().to_string()});
    if active_file {
        args["active_file"] = json!(true);
    }
    if dry_run {
        args["dry_run"] = json!(true);
    }
    engine.execute("set", args).map_err(|e| e.to_string())
}

/// The winning FULL of `form` in the session's load order, through `inspect`.
fn winning_full(engine: &Engine, form: &str) -> String {
    let env = engine.execute("inspect", json!({"form": form})).unwrap();
    env["data"].to_string()
}

fn bytes(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap()
}

/// Positive: the loaded active file, last in the order and outside Data, My Games and MO2,
/// takes a second write (a record it does not hold yet) and a third (the record it holds,
/// edited in place); the masters are untouched, and later reads see each write.
#[test]
fn a_loaded_active_file_takes_further_writes() {
    let (d, data, active) = scratch("positive");
    let engine = loaded(&data, &order(&active));
    let source = bytes(&data.join("Source.esm"));
    let over = bytes(&data.join("Override.esp"));
    let second = set_full(&engine, "Source.esm:000801", "Second", &active, true, false).unwrap();
    assert_eq!(second["data"]["created"], json!(false), "{second:#}");
    let third = set_full(&engine, "Source.esm:000800", "Third", &active, true, false).unwrap();
    assert_eq!(third["data"]["created"], json!(false), "{third:#}");
    // OtherRifle's winner was Override.esp, which the second write added as a master.
    assert_eq!(
        third["data"]["masters"],
        json!(["Source.esm", "Override.esp"]),
        "{third:#}"
    );
    assert_eq!(bytes(&data.join("Source.esm")), source);
    assert_eq!(bytes(&data.join("Override.esp")), over);
    // `new` and `clone` into the loaded active file, too.
    engine
        .execute(
            "new",
            json!({"type": "GLOB", "edid": "KkA6Global", "into": active.display().to_string(),
                   "active_file": true}),
        )
        .unwrap();
    engine
        .execute(
            "clone",
            json!({"form": "Source.esm:000800", "edid": "OldRifleA6Copy",
                   "into": active.display().to_string(), "active_file": true}),
        )
        .unwrap();
    // Read from the file alone: every write is there, and the file validates.
    let text = String::from_utf8_lossy(&bytes(&active)).into_owned();
    for want in ["Second", "Third", "KkA6Global", "OldRifleA6Copy"] {
        assert!(text.contains(want), "{want} missing from the active file");
    }
    assert!(
        !text.contains("First"),
        "the in-place edit kept the old FULL"
    );
    let v = execute(
        "validate",
        json!({"plugin": active.display().to_string(), "data_dir": data.display().to_string()}),
    )
    .unwrap();
    assert_eq!(v["data"]["passed"], json!(true), "{:#}", v["data"]);
    let _ = std::fs::remove_dir_all(&d);
}

/// Negative 1: a load-order plugin that is not the last is refused, flag or not, and the
/// flag never exempts another plugin than `into` (here a master, and a plugin before the
/// last); nothing is written.
#[test]
fn a_loaded_plugin_that_is_not_last_is_refused() {
    let (d, data, active) = scratch("not-last");
    let plugins = vec![
        "Source.esm".to_string(),
        active.display().to_string(),
        "Override.esp".to_string(),
    ];
    let engine = loaded(&data, &plugins);
    let before = bytes(&active);
    for flag in [true, false] {
        let err = set_full(&engine, "Source.esm:000801", "X", &active, flag, false).unwrap_err();
        assert!(err.contains("plugin of the load order"), "{err}");
        if flag {
            assert!(err.contains("only the last plugin"), "{err}");
        }
    }
    assert_eq!(bytes(&active), before);
    let source = data.join("Source.esm");
    let master = bytes(&source);
    let err = set_full(&engine, "Source.esm:000801", "X", &source, true, false).unwrap_err();
    assert!(err.contains("only the last plugin"), "{err}");
    assert_eq!(bytes(&source), master);
    // Last, but an earlier plugin names it as a master (an ordering error): refused.
    let dependent = crate::edit::patch_tests::write_plugin(
        &d.join("work"),
        "Dependent.esp",
        &["Source.esm", "Patch.esp"],
        0,
        Vec::new(),
    );
    let plugins = vec![
        "Source.esm".to_string(),
        dependent.display().to_string(),
        active.display().to_string(),
    ];
    let engine = loaded(&data, &plugins);
    let err = set_full(&engine, "Source.esm:000801", "X", &active, true, false).unwrap_err();
    assert!(err.contains("names Patch.esp as a master"), "{err}");
    assert_eq!(bytes(&active), before);
    let _ = std::fs::remove_dir_all(&d);
}

/// Negative 1b: without `active_file: true` the last plugin is refused as before: no
/// caller gains the exemption without asking for it.
#[test]
fn without_the_flag_the_last_plugin_is_still_refused() {
    let (d, data, active) = scratch("no-flag");
    let engine = loaded(&data, &order(&active));
    let before = bytes(&active);
    let err = set_full(&engine, "Source.esm:000801", "X", &active, false, false).unwrap_err();
    assert!(err.contains("a patch is always a separate file"), "{err}");
    for (ty, args) in [
        (
            "new",
            json!({"type": "GLOB", "edid": "KkNoFlag", "into": active.display().to_string()}),
        ),
        (
            "clone",
            json!({"form": "Source.esm:000800", "edid": "NoFlagCopy", "into": active.display().to_string()}),
        ),
    ] {
        let err = engine.execute(ty, args).unwrap_err().to_string();
        assert!(
            err.contains("a patch is always a separate file"),
            "{ty}: {err}"
        );
    }
    // `active_file: false` is the same as no flag.
    let err = engine
        .execute(
            "set",
            json!({"form": "Source.esm:000801", "fields": {"FULL": "X"},
                   "into": active.display().to_string(), "active_file": false}),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("a patch is always a separate file"), "{err}");
    assert_eq!(bytes(&active), before);
    let _ = std::fs::remove_dir_all(&d);
}

/// Negative 2: an official master by name is refused as the active file even when it is
/// the last plugin and lies outside Data and MO2.
#[test]
fn an_official_master_is_refused_even_when_last() {
    let (d, data, active) = scratch("official");
    for name in [
        "SFBGS0A6.esm",
        "ShatteredSpace.esm",
        "BlueprintShips-A6.esm",
    ] {
        let other = d.join(format!("official-{name}"));
        std::fs::create_dir_all(&other).unwrap();
        let official = other.join(name);
        std::fs::copy(&active, &official).unwrap();
        let mut plugins = order(&active);
        plugins.pop();
        plugins.push(official.display().to_string());
        let engine = loaded(&data, &plugins);
        let before = bytes(&official);
        let err = set_full(&engine, "Source.esm:000801", "X", &official, true, false).unwrap_err();
        assert!(err.contains("is an official master"), "{name}: {err}");
        assert_eq!(bytes(&official), before, "{name}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Negative 3: the last plugin under the game's Data folder, an MO2 instance or the user's
/// My Games folder is refused (`guard_location`, unrelaxed), flag or not.
#[test]
fn a_last_plugin_under_data_my_games_or_mo2_is_refused() {
    let (d, data, active) = scratch("location");
    let game = d.join("Game").join("Data");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join("Starfield.esm"), b"TES4").unwrap();
    let mo2 = d.join("MO2");
    std::fs::create_dir_all(mo2.join("mods").join("A6")).unwrap();
    std::fs::write(mo2.join("ModOrganizer.ini"), b"[General]\r\n").unwrap();
    let my_games = d.join("Documents").join("My Games").join("Starfield");
    std::fs::create_dir_all(&my_games).unwrap();
    for (folder, why) in [
        (game.clone(), "game's Data directory"),
        (mo2.join("mods").join("A6"), "Mod Organizer"),
        (my_games.clone(), "My Games"),
        (my_games.join("Data"), "user Data directory"),
    ] {
        std::fs::create_dir_all(&folder).unwrap();
        let there = folder.join("Patch.esp");
        std::fs::copy(&active, &there).unwrap();
        let mut plugins = order(&active);
        plugins.pop();
        plugins.push(there.display().to_string());
        let engine = loaded(&data, &plugins);
        let before = bytes(&there);
        for flag in [true, false] {
            let err = set_full(&engine, "Source.esm:000801", "X", &there, flag, false).unwrap_err();
            if flag {
                assert!(err.contains(why), "{}: {err}", folder.display());
            }
        }
        assert_eq!(bytes(&there), before, "{}", folder.display());
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Negative 4: a dry run into the loaded active file writes nothing: the file stays byte
/// identical, though the preview would write.
#[test]
fn a_dry_run_into_the_loaded_active_file_leaves_it_byte_identical() {
    let (d, data, active) = scratch("dry-run");
    let engine = loaded(&data, &order(&active));
    let before = bytes(&active);
    for form in ["Source.esm:000801", "Source.esm:000800"] {
        let env = set_full(&engine, form, "Previewed", &active, true, true).unwrap();
        assert_eq!(env["data"]["dry_run"], json!(true), "{env:#}");
        assert_eq!(env["data"]["would_write"], json!(true), "{env:#}");
        assert_eq!(bytes(&active), before, "{form}");
    }
    assert!(!winning_full(&engine, "Source.esm:000801").contains("Previewed"));
    let _ = std::fs::remove_dir_all(&d);
}

/// Negative 5 (staleness): after a confirmed write into the loaded active file, `inspect`
/// in the same session shows the new value (the session's load order was read again), and
/// a further write starts from it.
#[test]
fn after_a_write_the_session_reads_the_new_value() {
    let (d, data, active) = scratch("stale");
    let engine = loaded(&data, &order(&active));
    assert!(winning_full(&engine, "Source.esm:000800").contains("First"));
    set_full(
        &engine,
        "Source.esm:000800",
        "A much longer name than First",
        &active,
        true,
        false,
    )
    .unwrap();
    let seen = winning_full(&engine, "Source.esm:000800");
    assert!(seen.contains("A much longer name than First"), "{seen}");
    set_full(
        &engine,
        "Source.esm:000801",
        "Pistol two",
        &active,
        true,
        false,
    )
    .unwrap();
    let seen = winning_full(&engine, "Source.esm:000801");
    assert!(seen.contains("Pistol two"), "{seen}");
    let seen = winning_full(&engine, "Source.esm:000800");
    assert!(seen.contains("A much longer name than First"), "{seen}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Negative 6: a load order read from an MO2 profile (here one named `Default`) never
/// exempts its last plugin, even where that plugin's folder passes `guard_location` (an MO2
/// whose game folder holds no `Starfield.esm`); nothing is written.
#[test]
fn a_profile_load_never_exempts_its_last_plugin() {
    let (d, data, active) = scratch("profile");
    let last = data.join("Patch.esp");
    std::fs::copy(&active, &last).unwrap();
    let root = d.join("MO2");
    for sub in ["mods", "overwrite", "profiles/Default"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    let game = d.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        root.join("ModOrganizer.ini"),
        format!("[General]\r\ngameName=Starfield\r\ngamePath=@ByteArray({game})\r\n"),
    )
    .unwrap();
    std::fs::write(
        root.join("profiles/Default/plugins.txt"),
        b"*Source.esm\r\n*Override.esp\r\n*Patch.esp\r\n",
    )
    .unwrap();
    std::fs::write(root.join("profiles/Default/modlist.txt"), b"").unwrap();
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"profile": root.join("profiles/Default").display().to_string(), "no_cache": true}),
        )
        .unwrap();
    let before = bytes(&last);
    let err = set_full(&engine, "Source.esm:000801", "X", &last, true, false).unwrap_err();
    assert!(err.contains("MO2 profile"), "{err}");
    let err = set_full(&engine, "Source.esm:000801", "X", &last, true, true).unwrap_err();
    assert!(err.contains("MO2 profile"), "{err}");
    assert_eq!(bytes(&last), before);
    let _ = std::fs::remove_dir_all(&d);
}
