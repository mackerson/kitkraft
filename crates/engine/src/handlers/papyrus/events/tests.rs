//! `papyrus.events` through `execute`, on a synthetic zip (our own scripts) and, with
//! `KITKRAFT_DATA_DIR`, on the official `Tools\ContentResources.zip`.

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::index_for;
use crate::execute;
use crate::handlers::papyrus::sources::tests::{official_zip, zip_bytes};

const SCRIPT_OBJECT: &[u8] = b"ScriptName ScriptObject Native Hidden\n\
Function GotoState(String asNewState) Native\n\
Event OnInit()\nEndEvent\n";

const FORM: &[u8] = b"ScriptName Form extends ScriptObject Native Hidden\n\
Int Function GetFormID() Native\n";

const OBJECT_REFERENCE: &[u8] = b"ScriptName ObjectReference extends Form Native Hidden\n\
\n\
; Sent when the 3D is loaded.\n\
Event OnLoad()\n\
EndEvent\n\
{ Fired on load }\n\
Event OnActivate(ObjectReference akActionRef)\n\
EndEvent\n\
Function MoveTo(ObjectReference akTarget, Float afXOffset = 0.0, Bool abMatchRotation = True) Native\n";

const OUTPOST: &[u8] = b"ScriptName KC:OutpostBeacon extends ObjectReference Const\n\
CustomEvent OnBeaconLit\n\
Quest Property OwningQuest Auto Const Mandatory\n\
Struct Slot\n\
  Form Item\n\
  Int Count = 1\n\
EndStruct\n\
Event OnLoad()\n\
  RegisterForRemoteEvent(Self, \"OnActivate\")\n\
EndEvent\n\
Event ObjectReference.OnActivate(ObjectReference akSender, ObjectReference akActionRef)\n\
EndEvent\n\
Function Light()\n\
EndFunction\n";

const BROKEN: &[u8] = b"ScriptName Broken\nFunction F()\n";

fn synthetic(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "kitkraft-papyrus-events-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let zip = dir.join("ContentResources.zip");
    fs::write(
        &zip,
        zip_bytes(&[
            ("Scripts/Source/ScriptObject.psc", SCRIPT_OBJECT),
            ("Scripts/Source/Form.psc", FORM),
            ("Scripts/Source/ObjectReference.psc", OBJECT_REFERENCE),
            ("Scripts/Source/KC/OutpostBeacon.psc", OUTPOST),
            ("Scripts/Source/Broken.psc", BROKEN),
            (
                "Scripts/Source/Starfield_Papyrus_Flags.flg",
                b"Flag Hidden 0\n",
            ),
        ]),
    )
    .unwrap();
    (dir, zip)
}

fn events(zip: &std::path::Path, mut args: Value) -> Value {
    args["sources_zip"] = json!(zip.to_str().unwrap());
    args["no_cache"] = json!(true);
    execute("papyrus.events", args).unwrap()
}

fn rows(r: &Value) -> Vec<(String, String, String)> {
    r["data"]["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["kind"].as_str().unwrap().to_string(),
                m["script"].as_str().unwrap().to_string(),
                m["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn script_with_parents_and_evidence() {
    let (dir, zip) = synthetic("chain");
    let r = events(&zip, json!({"script": "kc:outpostbeacon", "kind": "event"}));
    let d = &r["data"];
    assert_eq!(d["script"]["name"], "KC:OutpostBeacon");
    assert_eq!(
        d["script"]["chain"],
        json!(["ObjectReference", "Form", "ScriptObject"])
    );
    assert_eq!(d["script"]["file"], "Scripts/Source/KC/OutpostBeacon.psc");
    assert_eq!(
        rows(&r),
        [
            ("event".into(), "KC:OutpostBeacon".into(), "OnLoad".into()),
            (
                "event".into(),
                "KC:OutpostBeacon".into(),
                "OnActivate".into()
            ),
            ("event".into(), "ObjectReference".into(), "OnLoad".into()),
            (
                "event".into(),
                "ObjectReference".into(),
                "OnActivate".into()
            ),
            ("event".into(), "ScriptObject".into(), "OnInit".into()),
        ]
    );
    let m = &d["matches"];
    // The handler overrides the parent's declaration; the remote one names its script.
    assert_eq!(m[0]["overrides"], "ObjectReference");
    assert_eq!(m[0]["inherited"], false);
    assert_eq!(m[1]["remote"], "ObjectReference");
    assert_eq!(
        m[1]["signature"],
        "Event ObjectReference.OnActivate(ObjectReference akSender, ObjectReference akActionRef)"
    );
    assert_eq!(m[2]["overrides"], Value::Null);
    assert_eq!(m[2]["inherited"], true);
    assert_eq!(m[2]["file"], "Scripts/Source/ObjectReference.psc");
    assert_eq!(m[2]["line"], 4);
    // Comments only on request.
    assert!(m[2].get("comment").is_none() && m[2].get("doc").is_none());
    // file:line evidence as byte ranges of the declaration.
    let ev = &r["evidence"][2];
    assert_eq!(ev["file"], "Scripts/Source/ObjectReference.psc");
    let at = ev["offset"].as_u64().unwrap() as usize;
    let len = ev["length"].as_u64().unwrap() as usize;
    assert_eq!(&OBJECT_REFERENCE[at..at + len], b"Event OnLoad()");
    assert_eq!(d["counts"]["event"], 5);
    assert_eq!(d["failures"], 1);
    assert_eq!(r["coverage"]["status"], "partial");
    assert!(r["coverage"]["not_examined"][0]
        .as_str()
        .unwrap()
        .starts_with("Scripts/Source/Broken.psc: line 2"));

    // Not inherited: only the script's own members.
    let own = events(
        &zip,
        json!({"script": "KC:OutpostBeacon", "inherited": false}),
    );
    let kinds: Vec<String> = rows(&own).into_iter().map(|(k, _, _)| k).collect();
    assert_eq!(
        kinds,
        [
            "custom_event",
            "property",
            "struct",
            "event",
            "event",
            "function"
        ]
    );
    let st = &own["data"]["matches"][2];
    assert_eq!(
        st["members"][1],
        json!({"type": "Int", "name": "Count", "default": "1"})
    );
    assert_eq!(own["data"]["matches"][1]["type"], "Quest");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn event_name_search_and_declarations_only() {
    let (dir, zip) = synthetic("search");
    // Every script that has OnLoad, then only where it is declared.
    let all = events(&zip, json!({"event": "onload"}));
    assert_eq!(all["data"]["total"], 2);
    let decl = events(&zip, json!({"event": "OnLoad", "declarations_only": true}));
    assert_eq!(
        rows(&decl),
        [("event".into(), "ObjectReference".into(), "OnLoad".into())]
    );
    // The remote handler is not a declaration either.
    let act = events(
        &zip,
        json!({"event": "OnActivate", "declarations_only": true}),
    );
    assert_eq!(act["data"]["total"], 1);

    // Search matches script names (all their members) and member names.
    let s = events(&zip, json!({"search": "OUTPOST", "kind": "function"}));
    assert_eq!(
        rows(&s),
        [("function".into(), "KC:OutpostBeacon".into(), "Light".into())]
    );
    let s = events(&zip, json!({"search": "move"}));
    assert_eq!(rows(&s)[0].2, "MoveTo");
    assert_eq!(
        s["data"]["matches"][0]["signature"],
        "Function MoveTo(ObjectReference akTarget, Float afXOffset = 0.0, Bool abMatchRotation = True) Native"
    );
    assert_eq!(s["data"]["matches"][0]["native"], true);

    // Paging.
    let p = events(&zip, json!({"kind": "all", "limit": 2, "start": 1}));
    assert_eq!(p["data"]["matches"].as_array().unwrap().len(), 2);
    assert_eq!(p["data"]["next_start"], 3);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn comments_only_with_comments() {
    let (dir, zip) = synthetic("comments");
    let r = events(
        &zip,
        json!({"script": "ObjectReference", "kind": "event", "inherited": false, "with_comments": true}),
    );
    let m = &r["data"]["matches"];
    assert_eq!(m[0]["comment"], "Sent when the 3D is loaded.");
    assert_eq!(m[0]["doc"], Value::Null);
    assert_eq!(m[1]["doc"], Value::Null);
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("not for redistribution")));
    fs::remove_dir_all(&dir).unwrap();
}

/// The CLI and MCP `command_line` spellings resolve to the same command and arguments.
#[test]
fn command_line_forms() {
    let crate::Invocation::Run { command, args } =
        crate::parse_line("papyrus events --event OnLoad --declarations-only").unwrap()
    else {
        panic!("expected Run");
    };
    assert_eq!(command.id, "papyrus.events");
    assert_eq!(args, json!({"event": "OnLoad", "declarations_only": true}));
    let crate::Invocation::Run { args, .. } =
        crate::parse_line("papyrus.events --script ObjectReference --kind function").unwrap()
    else {
        panic!("expected Run");
    };
    assert_eq!(
        args,
        json!({"script": "ObjectReference", "kind": "function"})
    );
}

#[test]
fn errors_are_clear() {
    let (dir, zip) = synthetic("errors");
    let z = zip.to_str().unwrap();
    let err = execute(
        "papyrus.events",
        json!({"sources_zip": z, "script": "Outpost", "no_cache": true}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("KC:OutpostBeacon"), "{err}");
    let err = execute(
        "papyrus.events",
        json!({"sources_zip": z, "event": "OnLoad", "kind": "function", "no_cache": true}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("contradicts"), "{err}");
    let err = execute(
        "papyrus.events",
        json!({"sources_zip": dir.join("missing.zip").to_str().unwrap()}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("does not exist"), "{err}");
    fs::remove_dir_all(&dir).unwrap();
}

/// The index is cached by the zip's central directory: the second call reads no source,
/// a changed zip gets a new entry, and the cache holds no comment text.
#[test]
fn index_cache_hits_and_misses() {
    let (dir, zip) = synthetic("cache");
    let cache = dir.join("cache");
    let (first, cached, _) = index_for(&zip, Some(&cache)).unwrap();
    assert!(!cached);
    let (second, cached, _) = index_for(&zip, Some(&cache)).unwrap();
    assert!(cached);
    assert_eq!(first, second);
    let entries: Vec<_> = fs::read_dir(cache.join("papyrus-index"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(entries.len(), 1);
    let text = fs::read_to_string(&entries[0]).unwrap();
    assert!(!text.contains("Sent when") && !text.contains("Fired on load"));

    fs::write(
        &zip,
        zip_bytes(&[("Scripts/Source/Form.psc", b"ScriptName Form\n")]),
    )
    .unwrap();
    let (third, cached, _) = index_for(&zip, Some(&cache)).unwrap();
    assert!(!cached);
    assert_eq!(third.scripts.len(), 1);
    // A corrupt cache entry is rebuilt, not an error.
    for e in fs::read_dir(cache.join("papyrus-index")).unwrap() {
        fs::write(e.unwrap().path(), b"{not json").unwrap();
    }
    let (_, cached, _) = index_for(&zip, Some(&cache)).unwrap();
    assert!(!cached);
    fs::remove_dir_all(&dir).unwrap();
}

/// M4.G2 on the official zip (skips without `KITKRAFT_DATA_DIR`): `ObjectReference`
/// declares `OnLoad`, its parents are found, a search finds outpost scripts, and the
/// counts per kind are printed for `plan/STATUS.md`.
#[test]
fn official_sources() {
    let Some(zip) = official_zip() else {
        return;
    };
    let r = events(
        &zip,
        json!({"script": "ObjectReference", "event": "OnLoad"}),
    );
    let d = &r["data"];
    assert_eq!(d["failures"], 0, "{r}");
    assert_eq!(r["coverage"]["status"], "complete");
    assert_eq!(d["files"], 5_033);
    let chain: Vec<&str> = d["script"]["chain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(chain.contains(&"Form"), "{chain:?}");
    let first = &d["matches"][0];
    assert_eq!(first["script"], "ObjectReference");
    assert_eq!(first["file"], "Scripts/Source/ObjectReference.psc");
    assert_eq!(first["overrides"], Value::Null);
    assert!(first["line"].as_u64().unwrap() > 0);

    let decl = events(&zip, json!({"event": "OnLoad", "declarations_only": true}));
    let outpost = events(&zip, json!({"search": "outpost", "limit": 5}));
    assert!(outpost["data"]["total"].as_u64().unwrap() > 0);
    println!(
        "official: {} scripts; counts {}; OnLoad declared in {:?}; ObjectReference chain {chain:?}; \
         `outpost` matches {}",
        d["scripts"],
        d["counts"],
        rows(&decl).iter().map(|r| r.1.clone()).collect::<Vec<_>>(),
        outpost["data"]["total"]
    );
}

/// Every `.psc` under `dir`, as `(path below dir with '/', bytes)`, in no particular order.
fn psc_tree(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("psc")) {
                let rel = p
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, fs::read(&p).unwrap()));
            }
        }
    }
    out
}

/// M5.B1: the owner's extraction of `Tools\ContentResources.zip`
/// (`C:\Modding\tools\ck-sources\Scripts\Source`, or `KITKRAFT_PAPYRUS_IMPORTS`; registry
/// row CK-CONTENT-RESOURCES) gives `papyrus.events` the same index as the zip read in
/// place: the same files, scripts (names, parents, flags, files, lines, offsets, members),
/// failures and diagnostics. Only the key differs (the zip's directory hash). Skipped when
/// either the zip or the folder is absent.
#[test]
fn extracted_folder_gives_the_same_index_as_the_zip() {
    let Some(zip) = official_zip() else {
        return;
    };
    let folder = crate::handlers::env_var_os("KITKRAFT_PAPYRUS_IMPORTS").map_or_else(
        || PathBuf::from(r"C:\Modding\tools\ck-sources\Scripts\Source"),
        PathBuf::from,
    );
    if !folder.is_dir() {
        println!("skipped: no extracted sources at {}", folder.display());
        return;
    }
    let (mut from_zip, _, _) = index_for(&zip, None).unwrap();
    let mut from_dir = super::build_from(&psc_tree(&folder), 0);
    for i in [&mut from_zip, &mut from_dir] {
        i.key = String::new();
        i.failures.sort();
        // The sort by name is stable, so ties keep the input order, which differs between
        // the zip's directory and a folder walk.
        i.scripts.sort_by(|a, b| {
            (a.name.to_ascii_lowercase(), &a.file).cmp(&(b.name.to_ascii_lowercase(), &b.file))
        });
    }
    assert_eq!(from_dir.files, 5_033);
    assert_eq!(from_dir.files, from_zip.files);
    assert_eq!(from_dir.failures, from_zip.failures);
    assert_eq!(from_dir.diagnostics, from_zip.diagnostics);
    assert_eq!(from_dir.scripts.len(), from_zip.scripts.len());
    for (a, b) in from_dir.scripts.iter().zip(&from_zip.scripts) {
        assert_eq!(a, b, "{} differs between the folder and the zip", a.file);
    }
    assert!(from_dir == from_zip);
    println!(
        "folder and zip: {} files, {} scripts, {} failures, {} diagnostics: the same index",
        from_dir.files,
        from_dir.scripts.len(),
        from_dir.failures.len(),
        from_dir.diagnostics
    );
}
