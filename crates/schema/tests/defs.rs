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
    assert_eq!(
        sigs,
        ["AMDL", "AMMO", "COBJ", "GLOB", "GMST", "KYWD", "OMOD", "PROJ", "TES4", "WEAP", "ZOOM"]
    );
    let headers: Vec<&str> = embedded.headers.keys().map(String::as_str).collect();
    assert_eq!(headers, ["GroupHeader", "RecordHeader"]);
    let common: Vec<&str> = embedded.common.keys().map(String::as_str).collect();
    assert_eq!(
        common,
        [
            "BfcbBlock",
            "Bool8",
            "Component",
            "Condition",
            "Destructible",
            "Keywords",
            "Model",
            "ObjectModProperty",
            "ObteBlock",
            "SoundReference",
            "Vec3"
        ]
    );
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

/// The record types still described by a corpus inventory (M2.B2) type only sourced
/// members: the universal subrecords, the layouts M2.B9 sourced, and the base form
/// component frame from the shared `BfcbBlock`; everything else stays opaque until lane B
/// round 3 transcribes the type.
#[test]
fn inventory_definitions_type_only_sourced_members() {
    let schema = builtin().unwrap();
    let expected: [(&str, &[&str]); 3] = [
        ("GMST", &["EditorId", "Value"]),
        (
            "GLOB",
            &[
                "EditorId",
                "BfcbBlock.ComponentType",
                "BfcbBlock.EndMarker",
                "Value",
            ],
        ),
        (
            "KYWD",
            &[
                "EditorId",
                "BfcbBlock.ComponentType",
                "BfcbBlock.EndMarker",
                "Name",
            ],
        ),
    ];
    for (s, typed) in expected {
        let rec = schema.record(sig(s)).unwrap();
        assert!(rec.provenance.source.starts_with("corpus: "), "{s}");
        assert_eq!(typed_leaves(rec), typed, "{s}");
    }
    // The component data inside a BFCB block is a union decided by the component type
    // and stays raw (docs/gaps.md).
    let bfcb = &schema.common["BfcbBlock"];
    let CommonBody::Group(members) = &bfcb.body else {
        panic!("BfcbBlock is a group");
    };
    assert!(
        members
            .iter()
            .any(|m| matches!(m, Member::AnyUntil(a) if a.name == "ComponentData")),
        "BfcbBlock keeps its component data raw"
    );
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
}

/// Every citation of a node, its own and inherited.
fn all_provenance(rec: &kitcraft_schema::RecordDef) -> Vec<(String, &Provenance)> {
    let mut provs = vec![(rec.signature.to_string(), &rec.provenance)];
    member_provenance(&rec.signature.to_string(), &rec.members, &mut provs);
    provs
}

/// Lane B round 3 (M3.B1..B7): record types transcribed from the pinned xEdit file. The
/// record cites its xEdit range, every top-level member cites its own lines, and every
/// member at `corpus` names the corpus check.
#[test]
fn round3_definitions_cite_xedit_per_member() {
    let schema = builtin().unwrap();
    for s in ROUND3 {
        let rec = schema.record(sig(s)).unwrap();
        assert!(
            rec.provenance
                .source
                .starts_with("xedit: wbDefinitionsSF1.pas@"),
            "{s}"
        );
        for m in &rec.members {
            let prov = match m {
                Member::Subrecord(x) => &x.provenance,
                Member::Group(g) => &g.provenance,
                Member::AnyUntil(a) => &a.provenance,
            };
            assert!(
                prov.source.starts_with("xedit: ") && prov.source != rec.provenance.source,
                "{s}.{}: a member cites its own xEdit lines",
                m.name()
            );
        }
        for (path, prov) in all_provenance(rec) {
            if prov.confidence == Confidence::Corpus {
                assert!(
                    prov.citations().any(|c| c.starts_with("corpus: ")),
                    "{path}"
                );
            }
        }
    }
    let weap = schema.record(sig("WEAP")).unwrap();
    // FLLD sits in the model block and in the first-person model block.
    let groups: Vec<&str> = weap
        .members
        .iter()
        .filter_map(|m| match m {
            Member::Group(g) => Some(g.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        groups,
        [
            "BfcbBlock",
            "Model",
            "Enchantment",
            "Destructible",
            "Keywords",
            "ObteBlock",
            "FirstPersonModel"
        ]
    );
    let known = schema.known_signatures(sig("WEAP")).unwrap();
    for s in [
        "BFCB", "BFCE", "FLLD", "OBTE", "OBTF", "OBTS", "STOP", "WTRM", "DAMA",
    ] {
        assert!(known.contains(&sig(s)), "{s}");
    }
    // Only payloads xEdit decides by a union (or does not describe) stay opaque.
    let opaque: Vec<String> = weap
        .leaf_subrecords()
        .into_iter()
        .filter(|(_, s)| s.ty.is_opaque())
        .map(|(p, _)| p)
        .collect();
    assert_eq!(opaque, ["Scripts", "FirstPersonModel.Mo4c"]);
}

/// The record types lane B round 3 has transcribed from the pinned xEdit file.
const ROUND3: &[&str] = &["WEAP", "AMMO", "PROJ", "AMDL", "OMOD", "COBJ", "ZOOM"];

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
