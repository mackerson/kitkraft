//! M4.W0 shape tests: every inspection command, run through `execute` on synthetic
//! plugins in each of its modes, returns an envelope that conforms to its documented shape
//! (`crate::shapes`, `docs/shapes.md`), with every marker in place.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::shapes::{check_envelope, describe, kind, INSPECTION_COMMANDS, KIND};
use crate::Engine;

fn with(base: &Value, extra: Value) -> Value {
    let mut a = base.clone();
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

/// `(label, envelope)` for every inspection command in each of its modes.
fn envelopes(tag: &str) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let mut run = |engine: &Engine, label: &str, id: &str, args: Value| {
        let env = engine
            .execute(id, args.clone())
            .unwrap_or_else(|e| panic!("{label}: {id} {args}: {e}"));
        out.push((label.to_string(), env));
    };

    // Links fixture: Base.esm (keywords, a rifle naming them, a missing, a null and an
    // unresolvable FormID), Patch.esm (a pistol and the rifle's override).
    let dir = super::refs::tests::fixture(&format!("shapes-{tag}"));
    let file = |n: &str| dir.join(n).display().to_string();
    let lo = json!({"plugins": ["Base.esm", "Patch.esm"], "data_dir": dir.display().to_string(), "no_cache": true});
    let engine = Engine::with_default_handlers();
    run(&engine, "load", "load", lo.clone());
    run(
        &engine,
        "inspect load order",
        "inspect",
        json!({"form": "Base.esm:000801"}),
    );
    run(
        &engine,
        "query",
        "query",
        json!({"type": "WEAP", "where": "KWDA == Base.esm:000800", "scope": "all"}),
    );
    run(
        &engine,
        "query unknown",
        "query",
        json!({"type": "WEAP", "where": "KWDA.Forms > 3", "limit": 1}),
    );
    run(
        &engine,
        "conflicts",
        "conflicts",
        json!({"count_classes": true}),
    );
    run(
        &engine,
        "refs outgoing",
        "refs",
        json!({"form": "Base.esm:000801", "depth": 2}),
    );
    run(
        &engine,
        "refs incoming",
        "refs",
        json!({"form": "Base.esm:000800", "incoming": true, "scope": "all"}),
    );
    run(
        &engine,
        "validate with masters",
        "validate",
        json!({"plugin": file("Patch.esm")}),
    );
    run(
        &engine,
        "inspect base only",
        "inspect",
        with(&lo, json!({"form": "Rifle", "plugins": ["Base.esm"]})),
    );
    run(
        &engine,
        "inspect single plugin",
        "inspect",
        json!({"form": "00000801", "plugin": file("Base.esm")}),
    );
    run(
        &engine,
        "diff",
        "diff",
        json!({"a": file("Base.esm"), "b": file("Patch.esm")}),
    );
    run(
        &engine,
        "coverage",
        "coverage",
        json!({"plugin": file("Base.esm")}),
    );
    run(
        &engine,
        "coverage summary",
        "coverage",
        json!({"plugin": file("Patch.esm"), "signatures": false, "top": 1, "decode": false}),
    );
    run(
        &engine,
        "validate dangling",
        "validate",
        json!({"plugin": file("Base.esm"), "plugins": ["Base.esm"], "data_dir": dir.display().to_string(), "no_cache": true}),
    );
    run(&engine, "schema list", "schema", json!({}));
    run(&engine, "schema record", "schema", json!({"type": "WEAP"}));
    run(
        &engine,
        "schema deep",
        "schema",
        json!({"type": "PCBN", "deep": true}),
    );
    run(
        &engine,
        "schema member",
        "schema",
        json!({"type": "WEAP", "member": "KWDA"}),
    );
    run(&engine, "schema header", "schema", json!({"type": "GRUP"}));
    run(
        &engine,
        "schema undefined",
        "schema",
        json!({"type": "ZZZZ"}),
    );

    // Planet fixture: a tree with a branch and three content nodes, a world, location,
    // biome, planet, star and pack-in.
    let pdir = super::planet::tests::fixture(&format!("shapes-{tag}"));
    let plo =
        json!({"plugins": ["Base.esm"], "data_dir": pdir.display().to_string(), "no_cache": true});
    let engine = Engine::with_default_handlers();
    for (label, form) in [
        ("planet tree", "TreeA"),
        ("planet branch", "BranchA"),
        ("planet content", "ContentPack"),
        ("planet planet", "PlanetA"),
        ("planet star", "StarA"),
        ("planet biome", "BiomeA"),
        ("planet location", "LocA"),
        ("planet worldspace", "WorldA"),
        ("planet pack-in", "PackA"),
    ] {
        run(&engine, label, "planet", with(&plo, json!({"form": form})));
    }
    // A decoded value with opaque members (CTDA's unions).
    run(
        &engine,
        "inspect conditions",
        "inspect",
        json!({"form": "BranchA"}),
    );
    run(
        &engine,
        "refs incoming of a tree",
        "refs",
        json!({"form": "TreeA", "incoming": true}),
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&pdir);
    out
}

/// Every marker object in `v`: `(kind, object)`.
fn markers<'v>(v: &'v Value, out: &mut Vec<(&'v str, &'v Value)>) {
    match v {
        Value::Array(items) => items.iter().for_each(|i| markers(i, out)),
        Value::Object(m) => {
            if let Some(k) = m.get(KIND).and_then(Value::as_str) {
                if kind::ALL.contains(&k) {
                    out.push((k, v));
                }
            }
            m.values().for_each(|i| markers(i, out));
        }
        _ => {}
    }
}

#[test]
fn every_inspection_envelope_conforms_to_its_documented_shape() {
    let envs = envelopes("conform");
    let mut problems = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (label, env) in &envs {
        seen.insert(env["command"].as_str().unwrap());
        for p in check_envelope(env) {
            problems.push(format!("{label}: {p}"));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
    let all: BTreeSet<&str> = INSPECTION_COMMANDS.iter().copied().collect();
    assert_eq!(seen, all, "a command without a shape test");
    for id in INSPECTION_COMMANDS {
        let spec = crate::find_command(id).unwrap();
        assert!(
            !spec.writes,
            "{id} writes; inspection commands are read-only"
        );
    }
}

/// The values a front end formats carry their marker, at the positions that hold them.
#[test]
fn values_carry_explicit_markers() {
    let envs = envelopes("markers");
    let get = |label: &str| &envs.iter().find(|(l, _)| l == label).unwrap().1;
    let kinds = |env: &Value| {
        let mut found = Vec::new();
        markers(&env["data"], &mut found);
        found
            .into_iter()
            .map(|(k, _)| k.to_string())
            .collect::<BTreeSet<String>>()
    };

    // inspect: the rifle's keyword list holds a resolved, a missing, a null and an
    // unresolvable FormID, each a formref (raw beside the resolution).
    let base = get("inspect base only");
    let kwda = base["data"]["versions"][0]["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == "KWDA")
        .unwrap();
    let forms = kwda["value"].as_array().unwrap();
    assert!(forms.iter().all(|f| f[KIND] == kind::FORMREF), "{forms:?}");
    assert_eq!(forms[0]["form"], "Base.esm:000800");
    assert_eq!(forms[0]["raw"], "00000800");
    assert_eq!(forms[3]["null"], true);
    assert!(forms[4]["form"].is_null() && forms[4]["error"].is_string());
    // diff, conflicts and inspect's vs_previous compare FormIDs as formrefs.
    for label in ["diff", "conflicts", "inspect load order"] {
        assert!(kinds(get(label)).contains(kind::FORMREF), "{label}");
    }
    // Opaque members are byte ranges with the file offset.
    let ctda = get("inspect conditions")["data"]["versions"][0]["subrecords"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["signature"] == "CTDA")
        .unwrap();
    let p1 = &ctda["value"]["Unused1"];
    assert_eq!(p1[KIND], kind::OPAQUE, "{ctda}");
    assert_eq!(
        p1["opaque"]["file_offset"].as_u64().unwrap(),
        ctda["data_offset"].as_u64().unwrap() + p1["opaque"]["payload_offset"].as_u64().unwrap()
    );
    // refs: outgoing edges are links, incoming ones referrers; planet's links are links.
    let out = get("refs outgoing");
    assert!(out["data"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e[KIND] == kind::LINK));
    let inc = get("refs incoming");
    assert!(!inc["data"]["edges"].as_array().unwrap().is_empty());
    assert!(inc["data"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e[KIND] == kind::REFERRER));
    let tree = get("planet tree");
    assert!(kinds(tree).contains(kind::LINK));
    assert!(
        kinds(tree).contains(kind::OPAQUE),
        "conditions keep opaque unions"
    );
    // Every evidence entry is marked.
    for (label, env) in &envs {
        for e in env["evidence"].as_array().unwrap() {
            assert_eq!(e[KIND], kind::EVIDENCE, "{label}");
        }
    }
}

#[test]
fn the_checker_reports_unmarked_values_and_undocumented_members() {
    let env = |data: Value| {
        json!({"command": "diff", "data": data, "coverage": {"status": "complete",
            "decoded_subrecords": null, "opaque_subrecords": null, "not_examined": []},
            "limitations": [], "evidence": [{"file": "A.esm", "offset": 0, "length": 4,
            "record": null, "subrecord": null}],
            "build": {"version": "0.0.0", "commit": "unknown", "date": "unknown", "profile": "debug"}})
    };
    let field = |before: Value| {
        json!({"a": "A.esm", "b": "B.esm", "form": null, "keyed_by": "name",
            "counts": {"added": 0, "removed": 0, "changed": 1, "unchanged": 0, "equal_by_value": 0},
            "added": [], "removed": [], "summary": "x",
            "changed": [{"form": "A.esm:000800", "signature": "WEAP", "class": "differs",
                "a": {"raw_form_id": "00000800", "offset": 0, "flags": "0x0", "compressed": false},
                "b": {"raw_form_id": "00000800", "offset": 0, "flags": "0x0", "compressed": false},
                "flags_differ": false, "order_differs": false,
                "subrecords": [{"signature": "KWDA", "occurrence": 0, "change": "changed",
                    "a": null, "b": null, "first_difference": null,
                    "fields": [{"path": "KWDA.Forms[0]", "before": before, "after": null}]}]}]})
    };
    let marked = field(json!({"kind": "formref", "form": "A.esm:000801"}));
    let problems = check_envelope(&env(marked.clone()));
    // Only the unmarked evidence entry is wrong.
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert!(problems[0].contains("evidence[0]"), "{problems:?}");

    let unmarked = check_envelope(&env(field(
        json!({"form": "A.esm:000801", "raw": "00000801"}),
    )));
    assert!(
        unmarked
            .iter()
            .any(|p| p.contains("looks like a `formref`")),
        "{unmarked:#?}"
    );
    let struct_with_kind = check_envelope(&env(field(json!({"kind": "Rifle"}))));
    assert!(struct_with_kind
        .iter()
        .any(|p| p.contains("no value marker")));
    let mut extra = marked;
    extra["surprise"] = json!(1);
    extra.as_object_mut().unwrap().remove("keyed_by");
    let p = check_envelope(&env(extra));
    assert!(p
        .iter()
        .any(|p| p.contains("data.surprise: not in the documented shape")));
    assert!(p.iter().any(|p| p.contains("data.keyed_by: missing")));
}

/// `docs/shapes.md` is the rendered shapes; regenerate it with
/// `cargo test -p kitkraft-engine write_shape_docs -- --ignored`.
fn shape_docs() -> String {
    let mut text = String::from(SHAPES_HEAD);
    for id in INSPECTION_COMMANDS {
        text.push_str(&format!(
            "\n## `{id}`\n\n```text\n{}\n```\n",
            describe(id).unwrap()
        ));
    }
    text
}

const SHAPES_HEAD: &str = "# Data shapes of the inspection commands (M4.W0)\n\n\
Generated from `crates/engine/src/shapes.rs` by \
`cargo test -p kitkraft-engine write_shape_docs -- --ignored`; a test fails when this file \
and the code differ. The rules (markers, `?`, `by` cases) are in `docs/mcp.md`, \
\"Data shapes and markers\". One line per member path of `data`: `name?` is not always \
present, `[object]` is a list of the objects below it, `<formref>` and the other `<...>` \
are marker objects, `value` is a decoded schema value.\n";

fn shapes_md() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/shapes.md")
}

#[test]
fn docs_print_the_shapes_the_code_checks() {
    let want = shape_docs();
    let have = std::fs::read_to_string(shapes_md())
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        have == want,
        "docs/shapes.md differs from crate::shapes; run \
         `cargo test -p kitkraft-engine write_shape_docs -- --ignored`"
    );
}

#[test]
#[ignore = "writes docs/shapes.md from crate::shapes"]
fn write_shape_docs() {
    std::fs::write(shapes_md(), shape_docs()).unwrap();
}
