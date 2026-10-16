//! The reader on Bethesda's compiler output for our own sources (papyrus oracle, owner
//! priority B, 2026-10-15; `docs/oracles.md`, "Papyrus compiler oracle").
//!
//! Each `.pex` under `tests/fixtures/` was compiled by `PapyrusCompiler.exe` 4.7.0.5 (not
//! release, not optimized) from the `.psc` of the same name beside it, which is ours (copied
//! from `docs/recipes/scripts/KitKraft/`). No Bethesda source or script is committed. The
//! oracle compares the official compiler with `kit papyrus compile` through this reader, so
//! these pin what the reader shows of that output: the instruction streams, property and
//! variable flags, user flags, property groups, structs and guards.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use kitkraft_pex::{disassemble, Pex, Value};

fn fixture(name: &str) -> Pex {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    Pex::parse(&std::fs::read(&path).unwrap()).unwrap()
}

fn names(p: &Pex, ids: &[u16]) -> Vec<String> {
    ids.iter().map(|i| p.string(*i).to_string()).collect()
}

#[test]
fn hello_on_load_disassembles_as_the_recipe_expects() {
    let p = fixture("HelloOnLoad.pex");
    assert_eq!(
        (p.header.major, p.header.minor, p.header.game_id),
        (3, 12, 4)
    );
    assert_eq!(p.header.source, "HelloOnLoad.psc");
    // The compiler suppresses the user and machine names outside release builds.
    assert_eq!(p.header.user, "<SUPPRESSED>");
    assert_eq!(p.header.machine, "<SUPPRESSED>");
    let text = disassemble(&p);
    let want = "\
HelloOnLoad.psc v3.12 gid4
OBJECT KitKraft:HelloOnLoad extends ObjectReference
  VAR String ::Text_var = 'KitKraft: hello from a script attached by kit'
  VAR Int ::Times_var = 1
  PROP String Text
  PROP Int Times
  FUNC OnLoad()
      0 assign I 0
      1 cmp_lt ::temp0 I ::Times_var
      2 jmpf ::temp0 5
      3 callstatic debug Notification ::nonevar ::Text_var
      4 iadd ::temp1 I 1
      5 assign I ::temp1
      6 jmp -5
";
    assert_eq!(text, want);
    let debug = p.debug.as_ref().unwrap();
    assert_eq!(debug.functions.len(), 1);
    // One source line per instruction.
    assert_eq!(debug.functions[0].lines.len(), 7);
}

#[test]
fn property_kinds_flags_and_groups() {
    let p = fixture("OracleProperties.pex");
    let o = &p.objects[0];
    assert_eq!(p.string(o.parent), "ObjectReference");
    let flag_bit = |name: &str| {
        p.user_flags
            .iter()
            .find(|u| p.string(u.name) == name)
            .map(|u| u.bit)
            .unwrap()
    };
    // The flags file's names, lower-cased, with their bits.
    assert_eq!(flag_bit("hidden"), 0);
    assert_eq!(flag_bit("conditional"), 1);
    assert_eq!(flag_bit("collapsedonref"), 3);
    assert_eq!(flag_bit("mandatory"), 5);
    let prop = |name: &str| {
        o.properties
            .iter()
            .find(|q| p.string(q.name) == name)
            .unwrap()
    };
    // Auto properties: read | write | auto, with a backing variable.
    for name in [
        "Count", "Scale", "Enabled", "Label", "Target", "Marker", "Hidden1",
    ] {
        assert_eq!(prop(name).flags, 7, "{name}");
        assert!(prop(name).auto_var.is_some(), "{name}");
    }
    // Mandatory and Hidden sit on the property; Conditional on its backing variable.
    assert_eq!(prop("Label").user_flags, 1 << 5);
    assert_eq!(prop("Hidden1").user_flags, 1);
    assert_eq!(prop("Enabled").user_flags, 0);
    let var = |name: &str| {
        o.variables
            .iter()
            .find(|v| p.string(v.name) == name)
            .unwrap()
    };
    assert_eq!(var("::Enabled_var").user_flags, 1 << 1);
    assert_eq!(var("::Count_var").value, Value::Int(3));
    assert_eq!(var("::Scale_var").value, Value::Float(1.5));
    assert_eq!(var("myLimit").value, Value::Int(10));
    // A full property: getter and setter (flags 3), a getter only (flags 1).
    let limit = prop("Limit");
    assert_eq!((limit.flags, limit.user_flags), (3, 1));
    assert!(limit.getter.is_some() && limit.setter.is_some() && limit.auto_var.is_none());
    let half = prop("Half");
    assert_eq!(half.flags, 1);
    assert!(half.getter.is_some() && half.setter.is_none());
    assert_eq!(
        p.string(prop("Count").doc),
        "An auto property with a default."
    );
    // Groups keep their declared member order; CollapsedOnRef is the group's flag.
    let groups = &p.debug.as_ref().unwrap().property_groups;
    let group = |name: &str| groups.iter().find(|g| p.string(g.name) == name).unwrap();
    assert_eq!(
        names(&p, &group("Settings").properties),
        ["Count", "Scale", "Enabled", "Label"]
    );
    assert_eq!(group("References").user_flags, 1 << 3);
    assert_eq!(
        names(&p, &group("").properties),
        ["Hidden1", "Limit", "Half"]
    );
}

#[test]
fn structs_guards_and_control_flow() {
    let p = fixture("OracleControl.pex");
    let o = &p.objects[0];
    assert_eq!(names(&p, &o.guards), ["CountGuard"]);
    let pair = &o.structs[0];
    assert_eq!(p.string(pair.name), "Pair");
    let first = pair
        .members
        .iter()
        .find(|m| p.string(m.name) == "First")
        .unwrap();
    assert_eq!(first.value, Value::Int(1));
    let orders = &p.debug.as_ref().unwrap().struct_orders;
    assert_eq!(names(&p, &orders[0].members), ["First", "Second"]);
    let text = disassemble(&p);
    for line in [
        "      0 cmp_lt ::temp0 n 0",
        "      6 ineg ::temp3 1",
        "     26 imod ::temp3 n 2",
        "     46 jmp -8",
        "     47 jmp -23",
        "      0 struct_create ::temp17",
        "      3 struct_set p First ::temp18",
        "      7 lock_guards CountGuard",
        "     10 unlock_guards CountGuard",
    ] {
        assert!(text.contains(line), "missing `{line}` in\n{text}");
    }
    let classify = o.states[0]
        .functions
        .iter()
        .find(|f| p.string(f.name) == "Classify")
        .unwrap();
    assert_eq!(classify.function.instructions.len(), 51);
}
