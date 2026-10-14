//! The checked-in definitions (M1.9) load cleanly and follow the provenance rules.

// Helpers outside `#[test]` fns panic to fail the calling test; this is test code.
#![allow(clippy::panic, clippy::unwrap_used)]

use std::path::Path;

use kitcraft_schema::{
    builtin, load_dir, Cardinality, Definition, FieldType, Member, Signature, BUILTIN_DEFINITIONS,
};

fn defs_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/defs"))
}

fn sig(s: &str) -> Signature {
    Signature::parse(s).unwrap()
}

fn all_opaque(members: &[Member]) -> bool {
    members.iter().all(|m| match m {
        Member::Subrecord(s) => s.ty.is_opaque(),
        Member::Group(g) => all_opaque(&g.members),
        Member::AnyUntil(_) => true,
    })
}

#[test]
fn builtin_set_loads_and_matches_the_directory() {
    let embedded = builtin().unwrap();
    let on_disk = load_dir(defs_dir()).unwrap();
    assert_eq!(embedded, on_disk);
    let sigs: Vec<&str> = embedded.definitions.keys().map(|s| s.as_str()).collect();
    assert_eq!(sigs, ["GLOB", "GMST", "GRUP", "KYWD", "TES4", "WEAP"]);
}

#[test]
fn every_file_names_its_source() {
    for (file, text) in BUILTIN_DEFINITIONS {
        assert!(
            text.contains("https://en.uesp.net/wiki/") || text.contains("corpus:"),
            "{file} must name a UESP page or a corpus scan"
        );
        assert!(
            text.starts_with('#'),
            "{file} must open with a source comment"
        );
    }
    for def in builtin().unwrap().definitions.values() {
        let source = match def {
            Definition::Record(r) => &r.source,
            Definition::Header(h) => &h.source,
        };
        assert!(
            source.starts_with("uesp-skyrim: https://en.uesp.net/wiki/Skyrim_Mod:")
                || source.starts_with("corpus: "),
            "{source}"
        );
    }
}

#[test]
fn inventory_definitions_are_fully_opaque() {
    let schema = builtin().unwrap();
    for s in ["GMST", "GLOB", "KYWD", "WEAP"] {
        let rec = schema.record(sig(s)).unwrap();
        assert!(rec.source.starts_with("corpus: "), "{s}");
        assert!(all_opaque(&rec.members), "{s} must stay opaque");
    }
    let weap = schema.record(sig("WEAP")).unwrap();
    assert_eq!(weap.note.as_deref(), Some("layout pending ADR 0005"));
    // The two FLLD slots and the required core of the corpus inventory.
    let flld = weap
        .members
        .iter()
        .filter(|m| matches!(m, Member::Subrecord(s) if s.signature == sig("FLLD")))
        .count();
    assert_eq!(flld, 2);
    assert_eq!(weap.members.len(), 47);
}

#[test]
fn tes4_container_layout() {
    let schema = builtin().unwrap();
    let tes4 = schema.record(sig("TES4")).unwrap();
    let names: Vec<&str> = tes4.members.iter().map(Member::name).collect();
    assert_eq!(
        names,
        [
            "Header",
            "Author",
            "Description",
            "Master",
            "OverriddenForms",
            "TagifyCount",
            "Tnam",
            "Bnam",
            "Incc",
            "Chgl"
        ]
    );
    let Member::Subrecord(hedr) = &tes4.members[0] else {
        panic!("HEDR is a subrecord");
    };
    assert_eq!(hedr.ty.fixed_size(None), Some(12));
    let Member::Group(master) = &tes4.members[3] else {
        panic!("Master is a group");
    };
    assert_eq!(master.cardinality, Cardinality::Repeated);
    let localized = tes4.flags.iter().find(|f| f.name == "Localized").unwrap();
    assert_eq!(localized.value, 0x80);

    let Some(Definition::Header(grup)) = schema.definitions.get(&sig("GRUP")) else {
        panic!("GRUP is a header");
    };
    assert_eq!(grup.size, 24);
    let group_type = grup.fields.iter().find(|f| f.name == "GroupType").unwrap();
    assert!(matches!(&group_type.ty, FieldType::Enum { values, .. } if values.len() == 10));
}
