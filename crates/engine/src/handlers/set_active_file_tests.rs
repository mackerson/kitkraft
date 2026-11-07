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

// ---------------------------------------------------------------------------------------
// Independent review of A6 (2026-11-06): adversarial cases the tests above do not cover.
// ---------------------------------------------------------------------------------------

/// Review M1: the engine exempts any last plugin the request calls the active file, so a
/// master-flagged plugin of another mod (outside Data, My Games and MO2), last in a
/// `plugins` order and named as a master by no loaded plugin, is written in place. The
/// workbench refuses a master-flagged active file (`windows/data_dialog.rs`), but the
/// engine, which the window's gate tests rely on as the final word, does not.
#[test]
fn a_master_flagged_last_plugin_is_refused_as_the_active_file() {
    let (d, data, _active) = scratch("review-master");
    let mods = d.join("mods").join("ThirdParty");
    std::fs::create_dir_all(&mods).unwrap();
    let third = crate::edit::patch_tests::write_plugin(
        &mods,
        "ThirdParty.esm",
        &["Source.esm"],
        kitkraft_esm::tes4::FLAG_MASTER,
        Vec::new(),
    );
    let plugins = vec!["Source.esm".to_string(), third.display().to_string()];
    let engine = loaded(&data, &plugins);
    let before = bytes(&third);
    let got = set_full(
        &engine,
        "Source.esm:000801",
        "Hijacked",
        &third,
        true,
        false,
    );
    assert!(
        got.is_err(),
        "the master-flagged ThirdParty.esm was written: {got:?}"
    );
    assert_eq!(bytes(&third), before, "ThirdParty.esm changed");
    let _ = std::fs::remove_dir_all(&d);
}

/// Review: path spellings. An earlier plugin spelled another way (case, `..`, forward
/// slashes, the `\\?\` prefix) is still refused with the flag; the active file spelled
/// another way gets the exemption it would get by its own spelling (no wider); a flag that
/// is not the JSON `true` is no flag.
#[test]
fn path_spellings_neither_widen_nor_dodge_the_exemption() {
    let (d, data, active) = scratch("review-spelling");
    let engine = loaded(&data, &order(&active));
    let over = data.join("Override.esp");
    let over_before = bytes(&over);
    let canon = std::fs::canonicalize(&over).unwrap();
    let canon_s = canon.display().to_string();
    let plain = canon_s.trim_start_matches(r"\\?\").to_string();
    let spellings = vec![
        PathBuf::from(plain.to_ascii_uppercase()),
        PathBuf::from(plain.replace('\\', "/")),
        data.join("..").join("Data").join("override.ESP"),
    ];
    for s in &spellings {
        let err = set_full(&engine, "Source.esm:000801", "X", s, true, false).unwrap_err();
        assert!(
            err.contains("only the last plugin") || err.contains("plugin of the load order"),
            "{}: {err}",
            s.display()
        );
    }
    // A `\\?\` spelling is refused as a spelling (unc-guard), before any load-order match.
    if cfg!(windows) {
        let s = PathBuf::from(format!(r"\\?\{plain}"));
        let err = set_full(&engine, "Source.esm:000801", "X", &s, true, false).unwrap_err();
        assert!(
            err.contains(r"verbatim (\\?\) path") && err.contains("local drive paths only"),
            "{}: {err}",
            s.display()
        );
    }
    assert_eq!(bytes(&over), over_before, "Override.esp changed");
    // Not a boolean true: refused as a plain load-order plugin (or as a bad argument).
    let active_before = bytes(&active);
    for flag in [json!("true"), json!(1), json!(null)] {
        let got = engine.execute(
            "set",
            json!({"form": "Source.esm:000801", "fields": {"FULL": "X"},
                   "into": active.display().to_string(), "active_file": flag}),
        );
        assert!(got.is_err(), "{flag}: {got:?}");
    }
    assert_eq!(bytes(&active), active_before, "a non-true flag wrote");
    // The active file by another spelling: exempt, written, and the session sees it.
    let other = PathBuf::from(
        std::fs::canonicalize(&active)
            .unwrap()
            .display()
            .to_string()
            .trim_start_matches(r"\\?\")
            .replace('\\', "/")
            .to_ascii_lowercase(),
    );
    set_full(&engine, "Source.esm:000801", "Spelled", &other, true, false).unwrap();
    assert!(winning_full(&engine, "Source.esm:000801").contains("Spelled"));
    assert_eq!(bytes(&over), over_before, "Override.esp changed");
    let _ = std::fs::remove_dir_all(&d);
}

/// Review: hard links. The active file is a hard link to an earlier plugin of the order,
/// or to a plugin inside a game Data folder: the write replaces the link (staged copy,
/// then rename), so the other name's bytes never change.
#[test]
fn a_hard_linked_active_file_never_changes_the_linked_plugin() {
    let (d, data, active) = scratch("review-hardlink");
    let over = data.join("Override.esp");
    let over_before = bytes(&over);
    let work = d.join("work");
    let linked = work.join("Linked.esp");
    std::fs::hard_link(&over, &linked).unwrap();
    let plugins = vec![
        "Source.esm".to_string(),
        "Override.esp".to_string(),
        linked.display().to_string(),
    ];
    let engine = loaded(&data, &plugins);
    set_full(
        &engine,
        "Source.esm:000800",
        "ViaLink",
        &linked,
        true,
        false,
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&bytes(&linked)).contains("ViaLink"));
    assert_eq!(
        bytes(&over),
        over_before,
        "Override.esp changed through a hard link"
    );
    // A hard link into a game Data folder (Starfield.esm beside it).
    let game = d.join("Game").join("Data");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join("Starfield.esm"), b"TES4").unwrap();
    let in_game = game.join("Patch.esp");
    std::fs::copy(&active, &in_game).unwrap();
    let game_before = bytes(&in_game);
    let linked2 = work.join("Linked2.esp");
    std::fs::hard_link(&in_game, &linked2).unwrap();
    let plugins = vec![
        "Source.esm".to_string(),
        "Override.esp".to_string(),
        linked2.display().to_string(),
    ];
    let engine = loaded(&data, &plugins);
    set_full(
        &engine,
        "Source.esm:000801",
        "ViaLink2",
        &linked2,
        true,
        false,
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&bytes(&linked2)).contains("ViaLink2"));
    assert_eq!(
        bytes(&in_game),
        game_before,
        "the Data plugin changed through a hard link"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Review: junctions. A load order whose last plugin is reached through a junction, or a
/// request naming the active file through one, is refused by `guard_location`; nothing is
/// written.
#[cfg(windows)]
#[test]
fn a_junction_to_the_active_file_is_refused() {
    let (d, data, active) = scratch("review-junction");
    let junction = d.join("work-j");
    let ok = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(d.join("work"))
        .output()
        .unwrap();
    assert!(ok.status.success(), "{ok:?}");
    let through = junction.join("Patch.esp");
    let before = bytes(&active);
    // Loaded through the junction.
    let mut plugins = order(&active);
    plugins.pop();
    plugins.push(through.display().to_string());
    let engine = loaded(&data, &plugins);
    let err = set_full(&engine, "Source.esm:000801", "X", &through, true, false).unwrap_err();
    assert!(err.contains("symbolic link or junction"), "{err}");
    // Loaded by its real path, named through the junction.
    let engine = loaded(&data, &order(&active));
    let err = set_full(&engine, "Source.esm:000801", "X", &through, true, false).unwrap_err();
    assert!(err.contains("symbolic link or junction"), "{err}");
    assert_eq!(bytes(&active), before);
    let _ = std::fs::remove_dir(&junction);
    let _ = std::fs::remove_dir_all(&d);
}

/// Review: two plugins of the same file name. A second `Patch.esp` earlier in the order
/// (another folder) holds a form of its own; with the flag, `in_place` matches copies by
/// file name, so the earlier file's record would be written into the active file as if it
/// were the active file's own (FormIDs as stored, no remap). It must be refused.
#[test]
fn a_same_named_earlier_plugin_is_not_taken_as_the_active_file() {
    let (d, data, active) = scratch("review-samename");
    let other_dir = d.join("other");
    std::fs::create_dir_all(&other_dir).unwrap();
    let other = other_dir.join("Patch.esp");
    // A form of its own in the other Patch.esp (not loaded while written).
    execute(
        "new",
        json!({"type": "GLOB", "edid": "KkOnlyInOther", "into": other.display().to_string(),
               "plugins": ["Source.esm"], "data_dir": data.display().to_string(),
               "no_cache": true}),
    )
    .unwrap();
    let plugins = vec![
        "Source.esm".to_string(),
        other.display().to_string(),
        active.display().to_string(),
    ];
    let engine = Engine::with_default_handlers();
    let load = engine.execute(
        "load",
        json!({"plugins": plugins, "data_dir": data.display().to_string(), "no_cache": true}),
    );
    assert!(
        load.is_ok(),
        "refused at load (the finding does not apply): {load:?}"
    );
    let before = bytes(&active);
    let got = engine.execute(
        "set",
        json!({"form": "KkOnlyInOther", "fields": {"FLTV": 2.0},
               "into": active.display().to_string(), "active_file": true}),
    );
    assert!(
        got.is_err(),
        "the other Patch.esp's own form was written as the active file's: {got:?}"
    );
    assert_eq!(bytes(&active), before);
    let _ = std::fs::remove_dir_all(&d);
}

/// Review: `revert` refuses a loaded active file cleanly (an error naming the load order,
/// the file byte identical), and `active_file` is not an argument it takes.
#[test]
fn revert_refuses_a_loaded_active_file_cleanly() {
    let (d, data, active) = scratch("review-revert");
    let engine = loaded(&data, &order(&active));
    let before = bytes(&active);
    let err = engine
        .execute(
            "revert",
            json!({"plugin": active.display().to_string(), "form": "Source.esm:000800"}),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("plugin of the load order"), "{err}");
    let err = engine
        .execute(
            "revert",
            json!({"plugin": active.display().to_string(), "form": "Source.esm:000800",
                   "active_file": true}),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("active_file"), "{err}");
    assert_eq!(bytes(&active), before);
    let _ = std::fs::remove_dir_all(&d);
}

/// Review: the reload after a write with the index cache on. Same-size in-place edits in
/// quick succession must each be seen by the next read and the next write. Needs
/// `KITKRAFT_CACHE_DIR` set to a scratch folder under the temp dir (an env var is
/// process-wide, so the test does not set it).
#[test]
#[ignore = "needs KITKRAFT_CACHE_DIR set to a scratch folder under the temp dir; run alone with --ignored"]
fn the_reload_with_the_index_cache_sees_each_write() {
    let cache = std::env::var_os("KITKRAFT_CACHE_DIR").expect("set KITKRAFT_CACHE_DIR");
    assert!(
        PathBuf::from(&cache).starts_with(std::env::temp_dir()),
        "KITKRAFT_CACHE_DIR must be under the temp dir"
    );
    let (d, data, active) = scratch("review-cache");
    let engine = Engine::with_default_handlers();
    for _ in 0..2 {
        let env = engine
            .execute(
                "load",
                json!({"plugins": order(&active), "data_dir": data.display().to_string()}),
            )
            .unwrap();
        assert!(env["data"]["cache_dir"].is_string(), "{:#}", env["data"]);
    }
    for name in ["AAAAA", "BBBBB", "CCCCC", "DDDDD"] {
        let size = bytes(&active).len();
        set_full(&engine, "Source.esm:000800", name, &active, true, false).unwrap();
        assert_eq!(bytes(&active).len(), size, "the size changed");
        let seen = winning_full(&engine, "Source.esm:000800");
        assert!(seen.contains(name), "{name}: {seen}");
        set_full(&engine, "Source.esm:000801", name, &active, true, false).unwrap();
        assert!(winning_full(&engine, "Source.esm:000801").contains(name));
        assert!(winning_full(&engine, "Source.esm:000800").contains(name));
    }
    let v = execute(
        "validate",
        json!({"plugin": active.display().to_string(), "data_dir": data.display().to_string()}),
    )
    .unwrap();
    assert_eq!(v["data"]["passed"], json!(true), "{:#}", v["data"]);
    let _ = std::fs::remove_dir_all(&d);
}

/// The admin-share spelling (`\\localhost\C$\...`) of a local file, or `None` when the
/// file is not on `C:` or the share is not reachable.
#[cfg(windows)]
fn admin_share(p: &Path) -> Option<PathBuf> {
    let canon = std::fs::canonicalize(p).ok()?.display().to_string();
    let plain = canon.trim_start_matches(r"\\?\").to_string();
    let rest = plain.strip_prefix(r"C:\")?;
    let unc = PathBuf::from(format!(r"\\localhost\C$\{rest}"));
    unc.is_file().then_some(unc)
}

/// Review: a UNC spelling (`\\localhost\C$\...`) of a plugin of the order, an earlier one
/// or the active file itself. Before the unc-guard merge neither the A6 check (`hits`) nor
/// `guard_target` saw a load-order plugin there (a UNC path resolves to `\\?\UNC\...`).
/// Now the output's network spelling is refused before any load-order match or exemption,
/// with or without the flag, for `set` and `new`. Skipped when the local admin share is not
/// reachable.
#[cfg(windows)]
#[test]
fn a_unc_spelling_of_a_loaded_plugin_is_refused() {
    let (d, data, active) = scratch("review-unc");
    let engine = loaded(&data, &order(&active));
    let over = data.join("Override.esp");
    let before = bytes(&over);
    let active_before = bytes(&active);
    let (Some(unc_over), Some(unc_active)) = (admin_share(&over), admin_share(&active)) else {
        let _ = std::fs::remove_dir_all(&d);
        return;
    };
    for unc in [&unc_over, &unc_active] {
        for flag in [false, true] {
            // OldRifle's winner is the active file, so the writer's own-master check does
            // not stand in for the guard here.
            let err =
                set_full(&engine, "Source.esm:000800", "ViaUnc", unc, flag, false).unwrap_err();
            assert!(
                err.contains("a network (UNC) path") && err.contains("local drive paths only"),
                "flag {flag}, set into {}: {err}",
                unc.display()
            );
            let mut args =
                json!({"type": "GLOB", "edid": "KkViaUnc", "into": unc.display().to_string()});
            if flag {
                args["active_file"] = json!(true);
            }
            let err = engine.execute("new", args).unwrap_err().to_string();
            assert!(
                err.contains("a network (UNC) path"),
                "flag {flag}, new into {}: {err}",
                unc.display()
            );
        }
    }
    assert_eq!(bytes(&over), before, "Override.esp changed");
    assert_eq!(bytes(&active), active_before, "the active file changed");
    let _ = std::fs::remove_dir_all(&d);
}

/// unc-guard follow-up: a load order with a plugin named through a network path is
/// refused before any exemption. The active file loaded through the admin share and
/// written by its drive spelling (a different identity key, so never the "last plugin"),
/// or by its share spelling, and an order whose earlier plugin is on the share: every
/// `set` with the flag is refused, and nothing changes. Skipped when the local admin share
/// is not reachable.
#[cfg(windows)]
#[test]
fn a_network_named_order_is_refused_before_any_exemption() {
    let (d, data, active) = scratch("unc-order");
    let over = data.join("Override.esp");
    let (Some(unc_over), Some(unc_active)) = (admin_share(&over), admin_share(&active)) else {
        let _ = std::fs::remove_dir_all(&d);
        return;
    };
    let before = bytes(&over);
    let active_before = bytes(&active);
    // The active file named through the share.
    let shared_active = vec![
        "Source.esm".to_string(),
        "Override.esp".to_string(),
        unc_active.display().to_string(),
    ];
    // An earlier plugin named through the share, the active file by its drive path.
    let shared_over = vec![
        "Source.esm".to_string(),
        unc_over.display().to_string(),
        active.display().to_string(),
    ];
    for plugins in [shared_active, shared_over] {
        let engine = loaded(&data, &plugins);
        let err = set_full(
            &engine,
            "Source.esm:000801",
            "ViaOrder",
            &active,
            true,
            false,
        )
        .unwrap_err();
        assert!(
            err.contains("through a network path") && err.contains("local drive path to write"),
            "{plugins:?}: {err}"
        );
    }
    assert_eq!(bytes(&over), before, "Override.esp changed");
    assert_eq!(bytes(&active), active_before, "the active file changed");
    let _ = std::fs::remove_dir_all(&d);
}

/// A6 review L3: `place` and `revert` refuse a loaded active file with a message that says
/// they do not yet write one (`set`, `new` and `clone` do); nothing is written.
#[test]
fn place_and_revert_say_they_do_not_yet_write_a_loaded_active_file() {
    let (d, data, active) = scratch("l3");
    let engine = loaded(&data, &order(&active));
    let before = bytes(&active);
    for (cmd, args) in [
        (
            "revert",
            json!({"plugin": active.display().to_string(), "form": "Source.esm:000800"}),
        ),
        (
            "place",
            json!({"base": "Source.esm:000800", "cell": "Source.esm:000801",
                   "position": [0.0, 0.0, 0.0], "into": active.display().to_string()}),
        ),
    ] {
        let err = engine.execute(cmd, args).unwrap_err().to_string();
        assert!(err.contains("plugin of the load order"), "{cmd}: {err}");
        assert!(
            err.contains("does not yet write to a loaded active file"),
            "{cmd}: {err}"
        );
    }
    assert_eq!(bytes(&active), before);
    let _ = std::fs::remove_dir_all(&d);
}

/// A6 review L2: the reload after a write keeps the load's `from_save`, so runtime IDs the
/// save confirmed stay confirmed.
#[test]
fn the_reload_keeps_from_save() {
    let (d, data, active) = scratch("l2");
    let save = d.join("Quicksave0.sfs");
    std::fs::write(
        &save,
        super::testdata::save(&["Source.esm", "Override.esp", "Patch.esp"], &[], &[]),
    )
    .unwrap();
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": order(&active), "data_dir": data.display().to_string(),
                   "no_cache": true, "from_save": save.display().to_string()}),
        )
        .unwrap();
    let basis = |engine: &Engine| {
        engine.with_session(|s| s.load_order().map(|l| l["runtime_basis"].clone()))
    };
    let first = basis(&engine);
    assert_eq!(first, Some(json!("confirmed-by-save")), "{first:?}");
    set_full(&engine, "Source.esm:000801", "Saved", &active, true, false).unwrap();
    assert_eq!(basis(&engine), first);
    let _ = std::fs::remove_dir_all(&d);
}

// ---------------------------------------------------------------------------------------
// Re-review of the fix round (58032c7d), 2026-11-06.
// ---------------------------------------------------------------------------------------

/// Re-review: a last plugin whose header cannot be read (garbage bytes) is not exempt;
/// nothing is written.
#[test]
fn an_unreadable_last_plugin_is_not_exempt() {
    let (d, data, _active) = scratch("rr-badheader");
    let bad = d.join("work").join("Bad.esp");
    std::fs::write(&bad, b"this is not a plugin at all, just bytes").unwrap();
    let mut plugins = vec!["Source.esm".to_string(), "Override.esp".to_string()];
    plugins.push(bad.display().to_string());
    let engine = Engine::with_default_handlers();
    let load = engine.execute(
        "load",
        json!({"plugins": plugins, "data_dir": data.display().to_string(), "no_cache": true}),
    );
    if load.is_ok() {
        let before = bytes(&bad);
        let err = set_full(&engine, "Source.esm:000801", "X", &bad, true, false).unwrap_err();
        assert!(err.contains("plugin of the load order"), "{err}");
        assert!(err.contains("header could not be read"), "{err}");
        assert_eq!(bytes(&bad), before);
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Sets bit 0 (master) of the TES4 record flags of the plugin at `p`, in place on disk.
fn flag_master_on_disk(p: &Path) {
    let mut b = bytes(p);
    assert_eq!(b.get(0..4), Some(&b"TES4"[..]));
    if let Some(f) = b.get_mut(8) {
        *f |= 1;
    }
    std::fs::write(p, b).unwrap();
}

/// Re-review: the active file is loaded without the master flag, then gains it on disk
/// (another tool) before the write. The exemption is decided on the loaded header, so the
/// write goes ahead into a file that is now master-flagged.
#[test]
fn a_master_flag_set_on_disk_after_the_load_is_refused() {
    let (d, data, active) = scratch("rr-flag-race");
    let engine = loaded(&data, &order(&active));
    flag_master_on_disk(&active);
    let before = bytes(&active);
    let got = set_full(
        &engine,
        "Source.esm:000801",
        "AfterFlag",
        &active,
        true,
        false,
    );
    assert!(
        got.is_err(),
        "a now master-flagged active file was written: {got:?}"
    );
    assert_eq!(bytes(&active), before);
    let _ = std::fs::remove_dir_all(&d);
}

/// Re-review: the active file is rewritten on disk by another writer after the load (a
/// form added in front of its WEAP group, so every later offset moves). The session's
/// in-place copy is read at the old offset; the write must either refuse or produce the
/// right record, never a mangled one.
#[test]
fn an_active_file_rewritten_after_the_load_is_never_mangled() {
    let (d, data, active) = scratch("rr-stale");
    let engine = loaded(&data, &order(&active));
    // Another writer (a one-shot call, the active file not loaded there) adds a GLOB.
    execute(
        "new",
        json!({"type": "GLOB", "edid": "KkExternal", "into": active.display().to_string(),
               "plugins": ["Source.esm", "Override.esp"], "data_dir": data.display().to_string(),
               "no_cache": true}),
    )
    .unwrap();
    let got = set_full(
        &engine,
        "Source.esm:000800",
        "AfterRewrite",
        &active,
        true,
        false,
    );
    if got.is_ok() {
        let text = String::from_utf8_lossy(&bytes(&active)).into_owned();
        assert!(
            text.contains("KkExternal"),
            "the other writer's form was lost"
        );
        assert!(text.contains("AfterRewrite"));
        let v = execute(
            "validate",
            json!({"plugin": active.display().to_string(), "data_dir": data.display().to_string()}),
        )
        .unwrap();
        assert_eq!(v["data"]["passed"], json!(true), "{:#}", v["data"]);
        let one = execute(
            "inspect",
            json!({"form": "Source.esm:000800", "plugins": order(&active),
                   "data_dir": data.display().to_string(), "no_cache": true}),
        )
        .unwrap();
        let s = one["data"].to_string();
        assert!(s.contains("AfterRewrite") && s.contains("OldRifle"), "{s}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Re-review: `set` cannot reach the active file's own TES4 header to give it the master
/// flag (no form addresses it), so the exemption cannot be turned against a later write.
#[test]
fn set_cannot_flag_the_active_files_own_header() {
    let (d, data, active) = scratch("rr-tes4");
    let engine = loaded(&data, &order(&active));
    for form in ["Patch.esp:000000", "00000000"] {
        let got = engine.execute(
            "set",
            json!({"form": form, "fields": {"FULL": "X"}, "plugin": "Patch.esp",
                   "into": active.display().to_string(), "active_file": true}),
        );
        assert!(got.is_err(), "{form}: {got:?}");
    }
    let b = bytes(&active);
    assert_eq!(
        b.get(8).map(|f| f & 1),
        Some(0),
        "the active file gained the master flag"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Re-review: same file name up to non-ASCII case (`Ä` / `ä`). The same-name check
/// compares ASCII case only, so the exemption stands; the in-place edit matches by path,
/// so the other file's form is not taken as the active file's own: it is either refused
/// or carried with the other file as a master, and the active file still validates.
#[test]
fn a_same_name_up_to_unicode_case_never_merges_the_two_files() {
    let d = dir("a6-rr-unicode");
    let data = data(&d);
    let work = d.join("work");
    std::fs::create_dir_all(work.join("meshes").join("weapons")).unwrap();
    std::fs::write(work.join("meshes").join("weapons").join("a.nif"), []).unwrap();
    let other_dir = d.join("other");
    std::fs::create_dir_all(&other_dir).unwrap();
    let active = work.join("Äpatch.esp");
    let other = other_dir.join("äpatch.esp");
    for (into, edid) in [(&other, "KkUniOther"), (&active, "KkUniActive")] {
        execute(
            "new",
            json!({"type": "GLOB", "edid": edid, "into": into.display().to_string(),
                   "plugins": ["Source.esm"], "data_dir": data.display().to_string(),
                   "no_cache": true}),
        )
        .unwrap();
    }
    let plugins = vec![
        "Source.esm".to_string(),
        other.display().to_string(),
        active.display().to_string(),
    ];
    let engine = loaded(&data, &plugins);
    let got = engine.execute(
        "set",
        json!({"form": "KkUniOther", "fields": {"FLTV": 3.0},
               "into": active.display().to_string(), "active_file": true}),
    );
    // The active file holds forms of its own, so adding the other file as a master is
    // refused; an Ok here would mean the record went in as the active file's own.
    assert!(got.is_err(), "accepted: {got:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Re-review: the same stale read, with a field both the intended WEAP and the record now
/// at its old offset have (EDID). The write must refuse (the file changed since the load),
/// not edit whatever record now sits at the old offset.
#[test]
fn a_stale_offset_never_edits_another_record() {
    let (d, data, active) = scratch("rr-stale-edid");
    let engine = loaded(&data, &order(&active));
    execute(
        "new",
        json!({"type": "GLOB", "edid": "KkExternal", "into": active.display().to_string(),
               "plugins": ["Source.esm", "Override.esp"], "data_dir": data.display().to_string(),
               "no_cache": true}),
    )
    .unwrap();
    let before = bytes(&active);
    let got = engine.execute(
        "set",
        json!({"form": "Source.esm:000800", "fields": {"EDID": "RenamedRifle"},
               "into": active.display().to_string(), "active_file": true}),
    );
    assert!(got.is_err(), "a stale read was written: {got:?}");
    assert_eq!(bytes(&active), before);
    let _ = std::fs::remove_dir_all(&d);
}
