//! The checked-in definitions (M1.9) load cleanly and follow the provenance rules.

// Helpers outside `#[test]` fns panic to fail the calling test; this is test code.
#![allow(clippy::panic, clippy::unwrap_used, clippy::arithmetic_side_effects)]

use std::collections::BTreeSet;
use std::path::Path;

use kitcraft_schema::{
    builtin, load_dir, Cardinality, CommonBody, Confidence, FieldDef, FieldType, Member,
    Provenance, Signature, BUILTIN_DEFINITIONS, COMMON_DIR, FORBIDDEN_UESP_STARFIELD_PAGES,
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

/// `*.toml` names in `dir`, prefixed as `load_dir` names them.
fn toml_names(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            out.insert(format!("{prefix}{name}"));
        }
    }
}

fn member_provenance<'a>(
    path: &str,
    members: &'a [Member],
    out: &mut Vec<(String, &'a Provenance)>,
) {
    for m in members {
        let here = format!("{path}.{}", m.name());
        match m {
            Member::Subrecord(s) => {
                out.push((here.clone(), &s.provenance));
                type_provenance(&here, &s.ty, out);
            }
            Member::Group(g) => {
                out.push((here.clone(), &g.provenance));
                member_provenance(&here, &g.members, out);
            }
            Member::AnyUntil(a) => out.push((here, &a.provenance)),
        }
    }
}

fn field_provenance<'a>(
    path: &str,
    fields: &'a [FieldDef],
    out: &mut Vec<(String, &'a Provenance)>,
) {
    for f in fields {
        let here = format!("{path}.{}", f.name);
        out.push((here.clone(), &f.provenance));
        type_provenance(&here, &f.ty, out);
    }
}

fn type_provenance<'a>(path: &str, ty: &'a FieldType, out: &mut Vec<(String, &'a Provenance)>) {
    match ty {
        FieldType::Struct { fields } => field_provenance(path, fields, out),
        FieldType::Array { element, .. } => type_provenance(path, element, out),
        FieldType::Ref { ty, .. } => type_provenance(path, ty, out),
        _ => {}
    }
}

/// Every definition, member and field has a confidence level, and a Skyrim container fact
/// is never below `corpus` (ADR 0005 Amendment 2, M2.B1).
#[test]
fn every_node_has_a_confidence_level() {
    let schema = builtin().unwrap();
    let mut provs: Vec<(String, &Provenance)> = Vec::new();
    for r in schema.records() {
        provs.push((r.signature.to_string(), &r.provenance));
        member_provenance(&r.signature.to_string(), &r.members, &mut provs);
    }
    for h in schema.headers() {
        provs.push((h.name.clone(), &h.provenance));
        field_provenance(&h.name, &h.fields, &mut provs);
    }
    for (path, p) in &provs {
        if p.citations().any(|c| c.starts_with("uesp-skyrim: ")) {
            assert!(p.confidence >= Confidence::Corpus, "{path}");
        }
    }
    assert_eq!(
        schema.record(sig("TES4")).unwrap().provenance.confidence,
        Confidence::Corpus
    );
    for name in ["GroupHeader", "RecordHeader"] {
        assert_eq!(
            schema.header(name).unwrap().provenance.confidence,
            Confidence::Corpus
        );
    }
}

/// Stands in for generating `BUILTIN_DEFINITIONS` until `cargo xtask codegen` (M1.15):
/// fails with the exact entries to add or remove when the list and the directory differ.
#[test]
fn builtin_list_matches_the_directory() {
    let mut on_disk = BTreeSet::new();
    toml_names(defs_dir(), "", &mut on_disk);
    toml_names(
        &defs_dir().join(COMMON_DIR),
        &format!("{COMMON_DIR}/"),
        &mut on_disk,
    );
    let listed: BTreeSet<String> = BUILTIN_DEFINITIONS
        .iter()
        .map(|(n, _)| (*n).to_owned())
        .collect();
    let missing: Vec<String> = on_disk
        .difference(&listed)
        .map(|n| format!("(\"{n}\", include_str!(\"../defs/{n}\")),"))
        .collect();
    let stale: Vec<&String> = listed.difference(&on_disk).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "BUILTIN_DEFINITIONS in src/lib.rs is out of date.\nadd:\n{}\nremove: {stale:?}",
        missing.join("\n")
    );
    assert_eq!(listed.len(), BUILTIN_DEFINITIONS.len(), "duplicate entries");
}

#[test]
fn builtin_set_loads_and_matches_the_directory() {
    let embedded = builtin().unwrap();
    let on_disk = load_dir(defs_dir()).unwrap();
    assert_eq!(*embedded, on_disk);
    // Cached: the same instance every time.
    assert!(std::ptr::eq(embedded, builtin().unwrap()));
    let sigs: Vec<&str> = embedded.records.keys().map(Signature::as_str).collect();
    assert_eq!(sigs, ["GLOB", "GMST", "KYWD", "TES4", "WEAP"]);
    let headers: Vec<&str> = embedded.headers.keys().map(String::as_str).collect();
    assert_eq!(headers, ["GroupHeader", "RecordHeader"]);
    let common: Vec<&str> = embedded.common.keys().map(String::as_str).collect();
    assert_eq!(common, ["BfcbBlock", "Keywords", "ObteBlock"]);
}

/// `BUILTIN_DEFINITIONS` has one marked region a tool may regenerate, holding every entry.
#[test]
fn builtin_list_has_one_generated_region() {
    let lib = include_str!("../src/lib.rs");
    let begin = "    // BEGIN generated by cargo xtask codegen: BUILTIN_DEFINITIONS\n";
    let end = "    // END generated by cargo xtask codegen: BUILTIN_DEFINITIONS\n";
    assert_eq!(lib.matches(begin).count(), 1);
    assert_eq!(lib.matches(end).count(), 1);
    let start = lib.find(begin).unwrap() + begin.len();
    let stop = lib.find(end).unwrap();
    let region = &lib[start..stop];
    assert_eq!(
        region.matches("include_str!(").count(),
        BUILTIN_DEFINITIONS.len()
    );
    for (name, _) in BUILTIN_DEFINITIONS {
        assert!(region.contains(&format!("\"{name}\"")), "{name}");
    }
}

/// Every `source` and `evidence` entry uses an ADR 0005 form whose citation has the shape
/// that form requires, and every file opens with a comment saying what each source
/// contributed.
#[test]
fn every_file_names_its_source() {
    let schema = builtin().unwrap();
    let mut provs: Vec<(String, &Provenance)> = Vec::new();
    for r in schema.records() {
        provs.push((r.signature.to_string(), &r.provenance));
        member_provenance(&r.signature.to_string(), &r.members, &mut provs);
    }
    for h in schema.headers() {
        provs.push((h.name.clone(), &h.provenance));
        field_provenance(&h.name, &h.fields, &mut provs);
    }
    for c in schema.common() {
        provs.push((c.name.clone(), &c.provenance));
        match &c.body {
            CommonBody::Group(members) => member_provenance(&c.name, members, &mut provs),
            CommonBody::Type(ty) => type_provenance(&c.name, ty, &mut provs),
        }
    }
    let citations: Vec<(String, &str)> = provs
        .iter()
        .flat_map(|(def, p)| p.citations().map(move |c| (def.clone(), c)))
        .collect();
    for (def, text) in citations {
        if let Some(page) = text.strip_prefix("uesp-skyrim: ") {
            assert!(
                page.starts_with("https://en.uesp.net/wiki/Skyrim_Mod:"),
                "{def}: {text}"
            );
        } else if let Some(page) = text.strip_prefix("uesp-starfield: ") {
            assert!(
                page.starts_with("https://starfieldwiki.net/wiki/Starfield_Mod:Mod_File_Format"),
                "{def}: {text}"
            );
            assert!(
                !FORBIDDEN_UESP_STARFIELD_PAGES
                    .iter()
                    .any(|p| page.contains(p)),
                "{def}: {text}"
            );
        } else if let Some(citation) = text.strip_prefix("xedit: ") {
            // The loader checks the pinned revision and the line range (ADR 0004).
            assert!(
                citation.starts_with("wbDefinitionsSF1.pas@")
                    || citation.starts_with("wbDefinitionsCommon.pas@"),
                "{def}: {text}"
            );
        } else if let Some(note) = text.strip_prefix("corpus: ") {
            assert!(note.len() > 10, "{def}: corpus note too vague: {text}");
        } else if let Some(id) = text.strip_prefix("ck-diff: ") {
            assert!(!id.trim().is_empty(), "{def}: {text}");
        } else if let Some(title) = text.strip_prefix("ck-wiki: ") {
            assert!(!title.trim().is_empty(), "{def}: {text}");
        } else {
            panic!("{def}: not an ADR 0005 form: {text}");
        }
    }
    for (file, text) in BUILTIN_DEFINITIONS {
        assert!(
            text.starts_with('#'),
            "{file} must open with a source comment"
        );
    }
}

/// Member paths of the typed (non-opaque) leaf subrecords of a record.
fn typed_leaves(rec: &kitcraft_schema::RecordDef) -> Vec<String> {
    rec.leaf_subrecords()
        .into_iter()
        .filter(|(_, s)| !s.ty.is_opaque())
        .map(|(p, _)| p)
        .collect()
}

/// Only sourced members are typed: the universal subrecords (M2.B2) and the layouts M2.B9
/// sourced; everything else in the inventory definitions stays opaque.
#[test]
fn inventory_definitions_type_only_sourced_members() {
    let schema = builtin().unwrap();
    let expected: [(&str, &[&str]); 4] = [
        ("GMST", &["EditorId"]),
        ("GLOB", &["EditorId"]),
        ("KYWD", &["EditorId", "Name"]),
        (
            "WEAP",
            &[
                "EditorId",
                "Name",
                "Keywords.Count",
                "Keywords.Forms",
                "Description",
                "ObteBlock.ObtsEntry.Name",
            ],
        ),
    ];
    for (s, typed) in expected {
        let rec = schema.record(sig(s)).unwrap();
        assert!(rec.provenance.source.starts_with("corpus: "), "{s}");
        assert_eq!(typed_leaves(rec), typed, "{s}");
    }
    let bfcb = &schema.common["BfcbBlock"];
    let CommonBody::Group(members) = &bfcb.body else {
        panic!("BfcbBlock is a group");
    };
    assert!(all_opaque(members), "BfcbBlock must stay opaque");
    let weap = schema.record(sig("WEAP")).unwrap();
    assert_eq!(weap.note.as_deref(), Some("layout pending ADR 0005"));
    // The two FLLD slots and the required core of the corpus inventory.
    let flld = weap
        .members
        .iter()
        .filter(|m| matches!(m, Member::Subrecord(s) if s.signature == sig("FLLD")))
        .count();
    assert_eq!(flld, 2);
    assert_eq!(weap.members.len(), 44);
    // Shared blocks are referenced, not copied.
    for s in ["GLOB", "KYWD", "WEAP"] {
        let rec = schema.record(sig(s)).unwrap();
        assert!(
            rec.members
                .iter()
                .any(|m| matches!(m, Member::Group(g) if g.shared.as_deref() == Some("BfcbBlock"))),
            "{s}"
        );
    }
    let known = schema.known_signatures(sig("WEAP")).unwrap();
    for s in ["BFCB", "BFCE", "OBTE", "OBTF", "OBTS", "STOP", "WTRM"] {
        assert!(known.contains(&sig(s)), "{s}");
    }
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
    let leaves: Vec<String> = tes4.leaf_subrecords().into_iter().map(|(p, _)| p).collect();
    assert!(leaves.contains(&"Master.FileName".to_owned()));

    let grup = schema.header_by_signature(sig("GRUP")).unwrap();
    assert_eq!(grup.size, 24);
    assert_eq!(grup.fields[0].name, "Signature");
    let group_type = grup.fields.iter().find(|f| f.name == "GroupType").unwrap();
    assert!(matches!(&group_type.ty, FieldType::Enum { values, .. } if values.len() == 10));

    let record = schema.header("RecordHeader").unwrap();
    assert_eq!((record.signature, record.size), (None, 24));
    let names: Vec<&str> = record.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Signature",
            "DataSize",
            "Flags",
            "FormId",
            "Timestamp",
            "VersionControl",
            "FormVersion",
            "Unknown"
        ]
    );
}
