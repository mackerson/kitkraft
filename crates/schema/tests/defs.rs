//! The checked-in definitions (M1.9) load cleanly and follow the provenance rules.

// Helpers outside `#[test]` fns panic to fail the calling test; this is test code.
#![allow(clippy::panic, clippy::unwrap_used, clippy::arithmetic_side_effects)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kitkraft_schema::{
    builtin, load_dir, Cardinality, CommonBody, Confidence, FieldDef, FieldType, Member,
    Provenance, RecordDef, Scalar, Signature, BUILTIN_DEFINITIONS, COMMON_DIR,
    FORBIDDEN_UESP_STARFIELD_PAGES,
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
/// is never below `corpus` (ADR 0005 Amendment 2, M2.B1), except the TES4 members no
/// official file shows, which are `container-reference` (Amendment 5).
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
    let mut container = Vec::new();
    for (path, p) in &provs {
        if p.confidence == Confidence::ContainerReference {
            container.push(path.as_str());
        } else if p.citations().any(|c| c.starts_with("uesp-skyrim: ")) {
            assert!(p.confidence >= Confidence::Corpus, "{path}");
        }
    }
    assert_eq!(container, ["TES4.Master.FileSize", "TES4.TagifyCount"]);
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
    // The expected lists derive from the directory (KK-090): a new definition file
    // needs no edit of this test, and a file the schema misses still fails it.
    let mut top = BTreeSet::new();
    toml_names(defs_dir(), "", &mut top);
    // The record signatures are the top-level stems, the two header files aside;
    // their headers are checked below.
    let record_sigs: Vec<String> = top
        .iter()
        .map(|n| n.trim_end_matches(".toml").to_owned())
        .filter(|s| s != "GRUP" && s != "RECORD")
        .collect();
    let sigs: Vec<String> = embedded
        .records
        .keys()
        .map(|s| s.as_str().to_owned())
        .collect();
    assert_eq!(sigs, record_sigs);
    let headers: Vec<&str> = embedded.headers.keys().map(String::as_str).collect();
    assert_eq!(headers, ["GroupHeader", "RecordHeader"]);
    let mut common_files = BTreeSet::new();
    toml_names(&defs_dir().join(COMMON_DIR), "", &mut common_files);
    let common_names: Vec<String> = common_files
        .iter()
        .map(|n| n.trim_end_matches(".toml").to_owned())
        .collect();
    let common: Vec<String> = embedded.common.keys().cloned().collect();
    assert_eq!(common, common_names);
}

/// The BO64 subrecord of the armor records is a `u64` of the first-person biped slots, and
/// the shared `BipedSlots` definition names the slots the owner's clothing mod relies on
/// (the bit index is the slot's position in the xEdit list).
#[test]
fn biped_slot_bits_are_named() {
    let schema = builtin().unwrap();
    for name in ["ARMO", "ARMA"] {
        let record = schema.record(sig(name)).unwrap();
        let FieldType::Flags { base, bits } = bo64_type(record) else {
            panic!("{name} BO64 is not a flags member")
        };
        assert_eq!(*base, Scalar::U64, "{name} BO64 base");
        for (wanted, value) in [
            ("HideHair", 1),
            ("Body", 1i128 << 3),
            ("Hat", 1i128 << 4),
            ("SsBody", 1i128 << 35),
        ] {
            assert!(
                bits.iter().any(|b| b.name == wanted && b.value == value),
                "{name} BO64: bit {value} is not named {wanted}"
            );
        }
    }
}

/// The record's `BO64` member type, following its shared-type reference.
fn bo64_type(record: &RecordDef) -> &FieldType {
    let ty = record
        .members
        .iter()
        .find_map(|m| match m {
            Member::Subrecord(s) if s.signature == sig("BO64") => Some(&s.ty),
            _ => None,
        })
        .unwrap();
    match ty {
        FieldType::Ref { ty, .. } => ty.as_ref(),
        other => other,
    }
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

/// Whether `path` lies inside one of the component layouts of the shared `BfcbBlock`
/// (`BfcbBlock.PlanetContentProperties.Unam`, lane D): sourced in `common/BfcbBlock.toml`,
/// not by the record that expands the block.
fn in_component_layout(path: &str) -> bool {
    path.strip_prefix("BfcbBlock.")
        .is_some_and(|rest| rest.contains('.'))
}

/// Member paths of the typed (non-opaque) leaf subrecords of a record, outside the shared
/// component layouts.
fn typed_leaves(rec: &kitkraft_schema::RecordDef) -> Vec<String> {
    rec.leaf_subrecords()
        .into_iter()
        .filter(|(p, s)| !s.ty.is_opaque() && !in_component_layout(p))
        .map(|(p, _)| p)
        .collect()
}

/// The record type still described by a corpus inventory (M2.B2), GLOB, types only sourced
/// members: the universal subrecords, the layout M2.B9 sourced, and the base form
/// component frame from the shared `BfcbBlock`. KYWD and GMST were inventories until M3.B6
/// (lane B round 5) transcribed them from the pinned xEdit file: every member upstream
/// gives a layout is typed; KYWD VMAD (script unions) and FNAM (no upstream layout) stay
/// opaque.
#[test]
fn inventory_definitions_type_only_sourced_members() {
    let schema = builtin().unwrap();
    let glob = schema.record(sig("GLOB")).unwrap();
    assert!(glob.provenance.source.starts_with("corpus: "));
    assert_eq!(
        typed_leaves(glob),
        [
            "EditorId",
            "BfcbBlock.ComponentType",
            "BfcbBlock.EndMarker",
            "Value",
        ]
    );
    let expected: [(&str, &[&str]); 2] = [
        ("GMST", &["EditorId", "XalgFlags", "Value"]),
        (
            "KYWD",
            &[
                "EditorId",
                "BfcbBlock.ComponentType",
                "BfcbBlock.EndMarker",
                "Color",
                "Notes",
                "KeywordType",
                "AttractionRule",
                "FlashLinkageName",
                "Name",
            ],
        ),
    ];
    for (s, typed) in expected {
        let rec = schema.record(sig(s)).unwrap();
        assert!(
            rec.provenance
                .citations()
                .any(|c| c.starts_with("xedit: wbDefinitionsSF1.pas@")),
            "{s}"
        );
        assert_eq!(typed_leaves(rec), typed, "{s}");
    }
    // The component data inside a BFCB block is a union decided by the component type;
    // the layouts not typed as groups of the block stay raw (docs/gaps.md).
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
fn all_provenance(rec: &kitkraft_schema::RecordDef) -> Vec<(String, &Provenance)> {
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
        .filter(|(p, s)| s.ty.is_opaque() && !in_component_layout(p))
        .map(|(p, _)| p)
        .collect();
    assert_eq!(opaque, ["Scripts", "FirstPersonModel.Mo4c"]);
}

/// The (record type, subrecord) pairs lane E's corpus lstring table classifies as lstrings
/// (`crates/engine/src/edit/lstrings.rs`, built and checked by
/// `lstring_table_matches_the_corpus`): 105 pairs over 58 record types, copied here so the
/// schema can be checked against them without depending on the engine.
const CORPUS_LSTRINGS: &[(&str, &str)] = &[
    ("ACTI", "ATTX"),
    ("ACTI", "FULL"),
    ("ALCH", "DESC"),
    ("ALCH", "DNAM"),
    ("ALCH", "FULL"),
    ("AMMO", "FULL"),
    ("AMMO", "ONAM"),
    ("AVIF", "ANAM"),
    ("AVIF", "FULL"),
    ("BIOM", "FULL"),
    ("BOOK", "DESC"),
    ("BOOK", "ENAM"),
    ("BOOK", "FNAM"),
    ("BOOK", "FULL"),
    ("BPTD", "BPTN"),
    ("CELL", "FULL"),
    ("CHAL", "DESC"),
    ("CHAL", "FULL"),
    ("CLAS", "FULL"),
    ("COBJ", "DESC"),
    ("CONT", "FULL"),
    ("DIAL", "FULL"),
    ("DMGT", "FULL"),
    ("DOOR", "FULL"),
    ("ENCH", "FULL"),
    ("EXPL", "FULL"),
    ("FACT", "FULL"),
    ("FLOR", "ATTX"),
    ("FLOR", "FULL"),
    ("FLST", "FULL"),
    ("FURN", "ATTX"),
    ("FURN", "FULL"),
    ("FXPD", "FULL"),
    ("GBFM", "FULL"),
    ("GBFM", "HULL"),
    ("GPOF", "DNAM"),
    ("GPOF", "NNAM"),
    ("GPOF", "RESN"),
    ("GPOF", "VOVS"),
    ("HAZD", "FULL"),
    ("HDPT", "FULL"),
    ("IDLE", "FULL"),
    ("INFO", "NAM1"),
    ("INFO", "RNAM"),
    ("INNR", "WNAM"),
    ("IRES", "FULL"),
    ("IRES", "NNAM"),
    ("KEYM", "FULL"),
    ("KYWD", "FULL"),
    ("LCTN", "FULL"),
    ("LSCR", "DESC"),
    ("LVLI", "ONAM"),
    ("MESG", "DESC"),
    ("MESG", "FULL"),
    ("MESG", "ITXT"),
    ("MESG", "NNAM"),
    ("MGEF", "DNAM"),
    ("MGEF", "FULL"),
    ("MISC", "FULL"),
    ("MISC", "NNAM"),
    ("MSTT", "FULL"),
    ("NPC_", "ATTX"),
    ("NPC_", "FULL"),
    ("NPC_", "LNAM"),
    ("NPC_", "SHRT"),
    ("OMOD", "DESC"),
    ("OMOD", "FULL"),
    ("PERK", "DESC"),
    ("PERK", "EPF2"),
    ("PERK", "FULL"),
    ("PKIN", "FULL"),
    ("PMFT", "FULL"),
    ("PNDT", "FULL"),
    ("PROJ", "FULL"),
    ("QUST", "CNAM"),
    ("QUST", "FULL"),
    ("QUST", "NNAM"),
    ("QUST", "QMDP"),
    ("QUST", "QMDS"),
    ("QUST", "QMDT"),
    ("QUST", "QMSU"),
    ("RACE", "DESC"),
    ("RACE", "FDSL"),
    ("RACE", "FMRN"),
    ("RACE", "FULL"),
    ("RACE", "SNAM"),
    ("REFR", "FULL"),
    ("REFR", "UNAM"),
    ("RSPJ", "DESC"),
    ("RSPJ", "FULL"),
    ("SPEL", "DESC"),
    ("SPEL", "FULL"),
    ("STAT", "FULL"),
    ("STDT", "FULL"),
    ("TERM", "FULL"),
    ("TMLM", "BTXT"),
    ("TMLM", "FULL"),
    ("TMLM", "INAM"),
    ("TMLM", "ISTX"),
    ("TMLM", "ITXT"),
    ("TMLM", "UNAM"),
    ("WATR", "FULL"),
    ("WEAP", "FULL"),
    ("WEAP", "WABB"),
    ("WRLD", "FULL"),
];

/// Record types with an lstring-only definition (M3.B8): EDID and their lstrings. A family
/// lane's full definition replaces one (M4.BX1: BIOM, GBFM, PKIN, PNDT, RSPJ and STDT from
/// the planet family; CELL, LCTN, REFR and WRLD from the world family; CHAL, DIAL, FACT,
/// INFO and QUST from the quest family; CLAS, MGEF, NPC_, PERK, RACE and SPEL from the
/// actor family; ACTI, CONT, FLOR, FURN, IDLE, MISC, MSTT, STAT, TERM and TMLM from the
/// items/workshop family). IRES, LSCR and MESG became M4.BX2 inventories
/// ([`BX2_INVENTORY`]).
#[rustfmt::skip]
const LSTRING_ONLY: &[&str] = &[
    "AVIF",
    "BPTD",
    "DMGT",
    "ENCH",
    "FXPD",
    "GPOF",
    "HDPT",
    "KEYM",
    "PMFT",
];

/// The M4.BX2 inventory definitions: corpus order and cardinality for record types no
/// family lane owns.
#[rustfmt::skip]
const BX2_INVENTORY: &[&str] = &[
    "AACT",
    "AAMD",
    "AAPD",
    "ADDN",
    "AMBS",
    "AOPF",
    "AOPS",
    "ARTO",
    "ATMO",
    "BMMO",
    "BMOD",
    "CAMS",
    "CLDF",
    "CLMT",
    "COLL",
    "CSTY",
    "CUR3",
    "DEBR",
    "DLBR",
    "DOBJ",
    "EFSH",
    "EFSQ",
    "EQUP",
    "FFKW",
    "FOGV",
    "FORC",
    "FSTP",
    "GBFT",
    "GCVR",
    "GPOG",
    "GRAS",
    "GWED",
    "IMAD",
    "IMGS",
    "IPDS",
    "IRES",
    "KSSM",
    "LAYR",
    "LENS",
    "LGDI",
    "LGTM",
    "LIGH",
    "LSCR",
    "LTEX",
    "LVLB",
    "MAAM",
    "MATT",
    "MESG",
    "MOVT",
    "MUSC",
    "MUST",
    "NAVI",
    "NOCM",
    "OSWP",
    "OVIS",
    "PDCL",
    "PERS",
    "PGRE",
    "PHZD",
    "PSDC",
    "PTST",
    "REVB",
    "RFGP",
    "SCCO",
    "SDLT",
    "SECH",
    "SFPC",
    "SFTR",
    "SOUN",
    "SPCH",
    "SPGD",
    "STAG",
    "STBH",
    "STMP",
    "STND",
    "TRNS",
    "TXST",
    "VOLI",
    "WBAR",
    "WKMF",
    "WTHR",
    "WTHS",
    "WWED",
];

/// Inventory definitions cited to the corpus alone (no xEdit citation; KK-008 and the
/// tickets whose brief says "only `corpus:` citations"). They follow the same rules as
/// [`BX2_INVENTORY`] otherwise. `cargo xtask add-def` puts a new inventory in one list or
/// the other by whether the file cites xEdit.
#[rustfmt::skip]
const INVENTORY_CORPUS_ONLY: &[&str] = &[
    "AVMD",
    "CURV",
    "DFOB",
    "IPCT",
    "LMSW",
    "REGN",
];

/// The M4.BX2 inventories whose `OBND` is typed as `ObjectBounds` (fix round W3, the lane R
/// request: the Render Window draws their references as bounds boxes).
const BX2_TYPED_BOUNDS: &[&str] = &["LIGH"];

/// M4.BX2: an inventory types only EDID, the corpus table's lstrings, the shared keyword
/// list, the base form component frame and, for [`BX2_TYPED_BOUNDS`], `OBND`; every other
/// member is opaque, cited to the
/// corpus scan at `corpus` confidence (with the pinned xEdit line where upstream lists it).
/// The [`INVENTORY_CORPUS_ONLY`] ones cite no xEdit line at all.
#[test]
fn bx2_inventories_type_only_edid_lstrings_and_shared_blocks() {
    let schema = builtin().unwrap();
    let all = BX2_INVENTORY.iter().chain(INVENTORY_CORPUS_ONLY);
    for record in all {
        let rec = schema.record(sig(record)).unwrap();
        assert!(
            rec.provenance
                .source
                .starts_with("corpus: inventory_types_full"),
            "{record}"
        );
        // The record's own citations (shared groups keep theirs, xEdit or not).
        let cites_xedit = rec.provenance.citations().any(|c| c.starts_with("xedit: "));
        assert_eq!(
            cites_xedit,
            BX2_INVENTORY.contains(record),
            "{record}: an inventory citing xEdit belongs in BX2_INVENTORY, one citing the corpus \
             alone in INVENTORY_CORPUS_ONLY (cargo xtask add-def)"
        );
        if BX2_INVENTORY.contains(record) {
            assert!(
                rec.provenance
                    .citations()
                    .any(|c| c.starts_with("xedit: wbDefinitionsSF1.pas@")),
                "{record} cites its xEdit definition"
            );
        }
        let lstrings: BTreeSet<String> = CORPUS_LSTRINGS
            .iter()
            .filter(|(r, _)| r == record)
            .map(|(_, s)| (*s).to_owned())
            .collect();
        assert_eq!(lstring_signatures(rec), lstrings, "{record}");
        for (path, sub) in rec.leaf_subrecords() {
            let shared = path.starts_with("Keywords.") || path.starts_with("BfcbBlock.");
            let edid = sub.signature == sig("EDID");
            let lstring = sub.ty == FieldType::LString;
            // Fix round W3 (lane R request): LIGH types OBND with the shared
            // ObjectBounds layout the other types use.
            let bounds = BX2_TYPED_BOUNDS.contains(record)
                && sub.signature == sig("OBND")
                && matches!(&sub.ty, FieldType::Ref { name, .. } if name == "ObjectBounds");
            assert!(
                shared || edid || lstring || bounds || sub.ty.is_opaque(),
                "{record}.{path} is typed"
            );
            if !shared {
                assert_eq!(
                    sub.provenance.confidence,
                    kitkraft_schema::Confidence::Corpus,
                    "{record}.{path}"
                );
                assert!(
                    sub.provenance
                        .source
                        .starts_with("corpus: inventory_types_full"),
                    "{record}.{path}"
                );
            }
        }
    }
}

/// Corpus-table lstrings that official records carry only inside a base form component
/// (BFCB..BFCE): the component data is a union by component type and stays raw in a full
/// definition (`BfcbBlock`), so the schema cannot type them by position; tools treat them
/// as lstrings by the FULL/DESC fact or the corpus table (engine `edit::lstrings`). The
/// lstring-only stubs of lane B round 3 typed them wherever they stood; the planet family's
/// full definitions replaced those stubs (M4.BX1). xEdit: the full-name component
/// (wbDefinitionsSF1.pas L6611-L6614) and the hull-code component (L6219-L6222). CHAL
/// keeps every activity's FULL and DESC inside its one component block (quest family,
/// M4.BX1 QUEST merge; `docs/schema/quest-family.md`).
const COMPONENT_LSTRINGS: &[(&str, &str)] = &[
    ("CHAL", "DESC"),
    ("CHAL", "FULL"),
    ("GBFM", "HULL"),
    ("PNDT", "FULL"),
    ("STDT", "FULL"),
];

/// Signatures of the whole-subrecord lstring members of a record, at any depth.
fn lstring_signatures(rec: &kitkraft_schema::RecordDef) -> BTreeSet<String> {
    rec.leaf_subrecords()
        .into_iter()
        .filter(|(_, s)| s.ty == FieldType::LString)
        .map(|(_, s)| s.signature.to_string())
        .collect()
}

/// M3.B8: every lstring of lane E's corpus table is an lstring member of its record type's
/// definition (or, for [`COMPONENT_LSTRINGS`], inside the raw data of a base form
/// component the definition references), and the lstring-only definitions type nothing else
/// (EDID aside).
#[test]
fn lstring_members_cover_the_corpus_table() {
    let schema = builtin().unwrap();
    assert_eq!(CORPUS_LSTRINGS.len(), 105);
    let mut types = BTreeSet::new();
    for (record, sub) in CORPUS_LSTRINGS {
        types.insert(*record);
        let rec = schema
            .record(sig(record))
            .unwrap_or_else(|| panic!("{record} has no definition"));
        if COMPONENT_LSTRINGS.contains(&(*record, *sub)) {
            assert!(
                rec.members.iter().any(
                    |m| matches!(m, Member::Group(g) if g.shared.as_deref() == Some("BfcbBlock"))
                ),
                "{record} has base form components"
            );
            // Never typed as something else at a position of its own.
            assert!(
                rec.leaf_subrecords()
                    .iter()
                    .all(|(_, s)| s.signature != sig(sub) || s.ty == FieldType::LString),
                "{record}.{sub}"
            );
            continue;
        }
        assert!(
            lstring_signatures(rec).contains(*sub),
            "{record}.{sub} is not an lstring member"
        );
    }
    assert_eq!(types.len(), 58);
    for record in LSTRING_ONLY {
        let rec = schema.record(sig(record)).unwrap();
        let expected: BTreeSet<String> = CORPUS_LSTRINGS
            .iter()
            .filter(|(r, _)| r == record)
            .map(|(_, s)| (*s).to_owned())
            .collect();
        assert_eq!(lstring_signatures(rec), expected, "{record}");
        let typed: Vec<String> = typed_leaves(rec)
            .into_iter()
            .filter(|p| !p.starts_with("LocalizedStrings."))
            .collect();
        assert_eq!(typed, ["EditorId"], "{record}");
    }
}

/// The record types lane B round 3 has transcribed from the pinned xEdit file.
const ROUND3: &[&str] = &[
    "WEAP", "AMMO", "PROJ", "AMDL", "OMOD", "COBJ", "ZOOM", "LVLI", "FLST", "INNR",
];

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

/// A layout fingerprint of a field type: names, types and sizes, not provenance or notes.
/// A shared type stays a reference (`ref:Name`), so a block that uses `Vec3` and one that
/// spells the three floats out differ here; [`inline_shapes`] catches the second.
fn ty_shape(ty: &FieldType) -> String {
    match ty {
        FieldType::Scalar(s) => format!("{s:?}"),
        FieldType::FormId { targets } => {
            let t: Vec<String> = targets.iter().map(ToString::to_string).collect();
            format!("formid[{}]", t.join(","))
        }
        FieldType::Enum { base, values } => {
            let v: Vec<String> = values
                .iter()
                .map(|n| format!("{}={}", n.name, n.value))
                .collect();
            format!("enum {base:?} [{}]", v.join(","))
        }
        FieldType::Flags { base, bits } => {
            let v: Vec<String> = bits
                .iter()
                .map(|n| format!("{}={}", n.name, n.value))
                .collect();
            format!("flags {base:?} [{}]", v.join(","))
        }
        FieldType::Array { element, count } => format!("array {count:?} of {}", ty_shape(element)),
        FieldType::Struct { fields } => {
            let f: Vec<String> = fields
                .iter()
                .map(|f| {
                    format!(
                        "{}:{}{:?}{:?}{}",
                        f.name,
                        ty_shape(&f.ty),
                        f.since,
                        f.until,
                        f.if_remaining
                    )
                })
                .collect();
            format!("struct {{{}}}", f.join(";"))
        }
        FieldType::Opaque { size } => format!("opaque {size:?}"),
        FieldType::Ref { name, .. } => format!("ref:{name}"),
        FieldType::Union { variants, .. } => format!("union {}", variants.len()),
        other => format!("{other:?}"),
    }
}

fn members_shape(members: &[Member]) -> String {
    let m: Vec<String> = members
        .iter()
        .map(|m| match m {
            Member::Subrecord(s) => format!(
                "{} {} {:?} {:?} {}",
                s.signature,
                s.name,
                s.cardinality,
                s.size,
                ty_shape(&s.ty)
            ),
            Member::Group(g) => match &g.shared {
                Some(shared) => format!("group_ref {shared} {} {:?}", g.name, g.cardinality),
                None => format!(
                    "group {} {:?} ({})",
                    g.name,
                    g.cardinality,
                    members_shape(&g.members)
                ),
            },
            Member::AnyUntil(a) => format!("any_until {} {}", a.until, a.name),
        })
        .collect();
    m.join("; ")
}

/// Shapes of the struct and enum layouts written out inline (not through `ref:`) anywhere
/// below `ty`; `top` skips `ty` itself.
fn inline_shapes(ty: &FieldType, top: bool, out: &mut Vec<String>) {
    if !top && matches!(ty, FieldType::Struct { .. } | FieldType::Enum { .. }) {
        out.push(ty_shape(ty));
    }
    match ty {
        FieldType::Struct { fields } => {
            for f in fields {
                inline_shapes(&f.ty, false, out);
            }
        }
        FieldType::Array { element, .. } => inline_shapes(element, false, out),
        _ => {}
    }
}

fn inline_member_shapes(members: &[Member], out: &mut Vec<String>) {
    for m in members {
        match m {
            Member::Subrecord(s) => inline_shapes(&s.ty, false, out),
            Member::Group(g) if g.shared.is_none() => inline_member_shapes(&g.members, out),
            _ => {}
        }
    }
}

/// M4.BX1: no two shared definitions (`defs/common/`) have the same layout, and no shared
/// definition spells out inline a struct or enum another shared type already names (the
/// families each wrote some of these; the integrator folds them into one, with every
/// reference updated).
#[test]
fn common_blocks_are_not_duplicated() {
    let schema = builtin().unwrap();
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    let mut types: Vec<(String, &str)> = Vec::new();
    for c in schema.common() {
        let shape = match &c.body {
            CommonBody::Group(members) => format!("group {}", members_shape(members)),
            CommonBody::Type(ty) => {
                let s = ty_shape(ty);
                types.push((s.clone(), c.name.as_str()));
                s
            }
        };
        if let Some(other) = seen.insert(shape, c.name.as_str()) {
            panic!("common/{} and common/{other} have the same layout", c.name);
        }
    }
    for c in schema.common() {
        let mut inline = Vec::new();
        match &c.body {
            CommonBody::Group(members) => inline_member_shapes(members, &mut inline),
            CommonBody::Type(ty) => inline_shapes(ty, true, &mut inline),
        }
        for s in &inline {
            for (shape, name) in &types {
                assert!(
                    s != shape,
                    "common/{} spells out common/{name} inline; use ref:{name}",
                    c.name
                );
            }
        }
    }
    // Records reference the shared blocks the families had spelled out inline (M4.BX1:
    // LVLP and LVLN models, the NPC_ destructible block and container items, every object
    // template).
    for (record, block) in [
        ("LVLP", "Model"),
        ("LVLN", "Model"),
        ("NPC_", "Destructible"),
        ("NPC_", "ContainerItems"),
        ("NPC_", "ObteBlock"),
        ("ARMO", "ObteBlock"),
        ("CONT", "ObteBlock"),
        ("TERM", "ObteBlock"),
        ("GBFM", "ObteBlock"),
    ] {
        let rec = schema.record(sig(record)).unwrap();
        assert!(
            rec.members
                .iter()
                .any(|m| matches!(m, Member::Group(g) if g.shared.as_deref() == Some(block))),
            "{record} references common/{block}"
        );
    }
    for record in [
        "PKIN", "NPC_", "RACE", "CLAS", "STAT", "ACTI", "CONT", "FURN", "MSTT", "TERM",
    ] {
        let rec = schema.record(sig(record)).unwrap();
        let prps = rec
            .leaf_subrecords()
            .into_iter()
            .find(|(_, s)| s.signature == sig("PRPS"))
            .unwrap();
        assert!(
            matches!(&prps.1.ty, FieldType::Array { element, .. }
                if matches!(element.as_ref(), FieldType::Ref { name, .. } if name == "PropertyValue")),
            "{record}.PRPS uses common/PropertyValue"
        );
    }
    let alias = &schema.common["QuestReferenceAlias"];
    let CommonBody::Group(members) = &alias.body else {
        panic!("QuestReferenceAlias is a group");
    };
    assert!(members.iter().any(|m| matches!(m, Member::Group(g)
        if g.name == "Inventory" && g.shared.as_deref() == Some("ContainerItems"))));
    // The folds of M4.BX1 hold.
    for (name, field, target) in [
        ("PositionRotation", "Position", "Vec3"),
        ("PositionRotation", "Rotation", "Vec3"),
        ("ObjectBounds", "Min", "Vec3"),
        ("ObjectBounds", "Max", "Vec3"),
        ("OwnerData", "NoCrime", "Bool8"),
    ] {
        let CommonBody::Type(FieldType::Struct { fields }) = &schema.common[name].body else {
            panic!("{name} is a struct type");
        };
        let f = fields.iter().find(|f| f.name == field).unwrap();
        assert!(
            matches!(&f.ty, FieldType::Ref { name, .. } if name == target),
            "{name}.{field}"
        );
    }
}
