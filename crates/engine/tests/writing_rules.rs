//! M2.E3 writing rules, asserted on the official ESMs (`docs/corpus.md`, "Writing rules").
//!
//! Corpus tests read `KITKRAFT_DATA_DIR` in place and skip with a message when it is unset
//! or missing (`docs/corpus.md`). The fast ones run in every `cargo test`; the full ONAM
//! inventory is `#[ignore]` (release: `cargo test -p kitkraft-engine --release --test
//! writing_rules -- --ignored --nocapture`).

#![allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs::File;
use std::path::PathBuf;

use kitkraft_esm::index::{index_file, read_record_at, IndexOptions, PluginIndex};
use kitkraft_esm::tes4::{Tes4, FLAG_LOCALIZED, FLAG_MASTER};

/// The 14 official ESMs (`corpus/manifest.toml`).
const CORPUS: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS003.esm",
    "SFBGS004.esm",
    "SFBGS006.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS00D.esm",
    "SFBGS047.esm",
    "SFBGS050.esm",
    "ShatteredSpace.esm",
];

/// The files with an `ONAM` (`docs/corpus.md`, TES4 header inventory).
const WITH_ONAM: [&str; 5] = [
    "SFBGS006.esm",
    "SFBGS00D.esm",
    "ShatteredSpace.esm",
    "SFBGS003.esm",
    "SFBGS050.esm",
];

/// String ID 0 is in none of the `starfield_en` tables, so an lstring holding 0 has no
/// text: a patch writes it inline as the empty string.
#[test]
fn corpus_string_id_zero_has_no_text() {
    let Some(dir) = data_dir() else { return };
    let archive = dir.join("Starfield - Localization.ba2");
    let (ba2, mut file) = kitkraft_ba2::Archive::open(&archive).unwrap();
    for kind in kitkraft_strings::Kind::ALL {
        let name = kitkraft_strings::table_name("Starfield.esm", "en", kind);
        let i = ba2.find(&format!("strings/{name}")).unwrap();
        let table = kitkraft_strings::Table::parse(
            kind,
            &ba2.extract_vec(&mut file, i, 256 << 20).unwrap(),
        )
        .unwrap();
        assert!(!table.is_empty(), "{name}");
        assert!(table.get(0).is_none(), "{name} has string ID 0");
    }
}

fn data_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("KITKRAFT_DATA_DIR")
        .or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
        .map(PathBuf::from);
    match dir {
        Some(dir) if dir.is_dir() => Some(dir),
        _ => {
            println!("skipped: KITKRAFT_DATA_DIR is unset or not a directory (docs/corpus.md)");
            None
        }
    }
}

/// Every official file is a master-flagged, localized `.esm` with `HEDR` version 0.96: the
/// header a new plugin copies (master flag for `.esm`), and the flag it must not copy
/// (localized: patches are non-localized in M2).
#[test]
fn corpus_new_plugin_header_facts() {
    let Some(dir) = data_dir() else { return };
    for name in CORPUS {
        let path = dir.join(name);
        let mut file = File::open(&path).unwrap();
        let header = read_record_at(&mut file, 0, None).unwrap();
        let tes4 = Tes4::new(&header);
        assert!(name.ends_with(".esm"), "{name}");
        assert_ne!(tes4.flags() & FLAG_MASTER, 0, "{name}: master flag");
        assert_ne!(tes4.flags() & FLAG_LOCALIZED, 0, "{name}: localized flag");
        let hedr = tes4.hedr().unwrap().unwrap();
        assert_eq!(hedr.version, 0.96, "{name}");
    }
}

/// The `TES4` subrecords of a file, by signature, in order.
fn tes4_subrecords(path: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut file = File::open(path).unwrap();
    let header = read_record_at(&mut file, 0, None).unwrap();
    header
        .subrecords()
        .unwrap()
        .iter()
        .map(|s| (s.signature().to_string(), s.data().to_vec()))
        .collect()
}

/// Interior cells: `CELL` records under the top-level `CELL` group (exterior cells are
/// under `WRLD`).
fn interior_cells(index: &PluginIndex) -> u32 {
    let top_of = |mut g: Option<u32>| {
        let mut top = None;
        while let Some(i) = g {
            let group = index.groups.get(usize::try_from(i).ok()?)?;
            top = Some(group);
            g = group.parent;
        }
        top
    };
    let count = index
        .records
        .iter()
        .filter(|r| r.signature.to_string() == "CELL")
        .filter(|r| {
            top_of(r.group).is_some_and(|t| t.header.group_type == 0 && t.header.label == *b"CELL")
        })
        .count();
    u32::try_from(count).unwrap()
}

fn incc(subs: &[(String, Vec<u8>)]) -> u32 {
    let (_, data) = subs.iter().find(|(s, _)| s == "INCC").unwrap();
    u32::from_le_bytes(data.as_slice().try_into().unwrap())
}

/// Fable review round 2, finding 5: the `TES4` metadata a new patch writes or deliberately
/// leaves out (`docs/corpus.md`, Writing rules). Every file has `CNAM` and `INCC`; the 12
/// files of the official build pipeline carry `CNAM` "Python" and `SNAM` "Script", the two
/// BlueprintShips files `CNAM` "DEFAULT" and no `SNAM` or `CHGL`; `BNAM` ("Dev_Main") is in
/// 12 files, `CHGL` in the 12 pipeline files. `INCC` is the number of interior cells,
/// checked here on four small files (all 14: `corpus_incc_is_interior_cells_full`).
#[test]
fn corpus_tes4_metadata() {
    let Some(dir) = data_dir() else { return };
    let (mut bnam, mut chgl) = (0, 0);
    for name in CORPUS {
        let subs = tes4_subrecords(&dir.join(name));
        let get = |sig: &str| {
            subs.iter()
                .find(|(s, _)| s == sig)
                .map(|(_, d)| d.as_slice())
        };
        let blueprint = name.starts_with("BlueprintShips");
        let cnam: &[u8] = if blueprint { b"DEFAULT\0" } else { b"Python\0" };
        assert_eq!(get("CNAM"), Some(cnam), "{name}: CNAM");
        assert_eq!(get("SNAM").is_some(), !blueprint, "{name}: SNAM");
        assert_eq!(get("CHGL").is_some(), !blueprint, "{name}: CHGL");
        assert!(get("INCC").is_some_and(|d| d.len() == 4), "{name}: INCC");
        // INCC is the last subrecord when there is no CHGL, else just before CHGL.
        let last: Vec<&str> = subs.iter().rev().take(2).map(|(s, _)| s.as_str()).collect();
        assert!(
            last[0] == "INCC" || (last[0] == "CHGL" && last[1] == "INCC"),
            "{name}: {last:?}"
        );
        bnam += usize::from(get("BNAM") == Some(b"Dev_Main\0".as_slice()));
        chgl += usize::from(get("CHGL").is_some());
    }
    assert_eq!((bnam, chgl), (12, 12));
    for (name, cells) in [
        ("Constellation.esm", 0),
        ("SFBGS004.esm", 1),
        ("SFBGS008.esm", 4),
        ("SFBGS003.esm", 42),
    ] {
        let path = dir.join(name);
        let index = index_file(&path, &IndexOptions::default()).unwrap();
        assert_eq!(interior_cells(&index), cells, "{name}");
        assert_eq!(incc(&tes4_subrecords(&path)), cells, "{name}");
    }
}

/// Full: `INCC` equals the interior cell count in all 14 files.
#[test]
#[ignore = "indexes 2.5 GB of plugins; run in release"]
fn corpus_incc_is_interior_cells_full() {
    let Some(dir) = data_dir() else { return };
    for name in CORPUS {
        let path = dir.join(name);
        let index = index_file(&path, &IndexOptions::default()).unwrap();
        let cells = interior_cells(&index);
        assert_eq!(incc(&tes4_subrecords(&path)), cells, "{name}");
        println!("{name}: INCC {cells} = interior cells");
    }
}

/// What `ONAM` lists in one file: for each listed FormID, the record's signature and the
/// type of its innermost group; plus the top-level (group type 0) overrides of master
/// records the file has, which `ONAM` never lists.
struct OnamInventory {
    listed: BTreeMap<(String, i32), u64>,
    missing: Vec<u32>,
    top_level_overrides: u64,
    top_level_listed: u64,
}

fn onam_inventory(index: &PluginIndex) -> OnamInventory {
    let tes4 = Tes4::new(&index.header);
    let onam: std::collections::BTreeSet<u32> = tes4.overridden_forms().into_iter().collect();
    let masters = tes4.masters().len();
    let mut out = OnamInventory {
        listed: BTreeMap::new(),
        missing: Vec::new(),
        top_level_overrides: 0,
        top_level_listed: 0,
    };
    let mut seen = std::collections::BTreeSet::new();
    for loc in index.records.iter().skip(1) {
        let group_type = loc
            .group
            .and_then(|g| index.groups.get(usize::try_from(g).ok()?))
            .map_or(-1, |g| g.header.group_type);
        let listed = onam.contains(&loc.form_id);
        if listed {
            seen.insert(loc.form_id);
            *out.listed
                .entry((loc.signature.to_string(), group_type))
                .or_default() += 1;
        }
        // A full-scale override of a master record (the high byte indexes a master).
        let high = usize::try_from(loc.form_id >> 24).unwrap_or(usize::MAX);
        if group_type == 0 && high < masters {
            out.top_level_overrides += 1;
            if listed {
                out.top_level_listed += 1;
            }
        }
    }
    out.missing = onam.difference(&seen).copied().collect();
    out
}

fn check_onam(name: &str, index: &PluginIndex) -> OnamInventory {
    let inv = onam_inventory(index);
    assert!(
        inv.missing.is_empty(),
        "{name}: ONAM FormIDs without a record: {:08X?}",
        inv.missing
    );
    for (sig, group_type) in inv.listed.keys() {
        assert!(
            [7, 9, 10].contains(group_type),
            "{name}: ONAM lists a {sig} in a group of type {group_type}"
        );
    }
    assert_eq!(
        inv.top_level_listed, 0,
        "{name}: a top-level record in ONAM"
    );
    inv
}

/// Fast: `SFBGS006.esm` (7 MB) lists 12 temporary cell children and overrides top-level
/// records (FURN, PKIN, MSTT, NAVI) without listing them.
#[test]
fn corpus_onam_lists_child_records_only_fast() {
    let Some(dir) = data_dir() else { return };
    let index = index_file(&dir.join("SFBGS006.esm"), &IndexOptions::default()).unwrap();
    let inv = check_onam("SFBGS006.esm", &index);
    assert_eq!(inv.listed.values().sum::<u64>(), 12);
    assert!(inv.top_level_overrides > 0);
}

/// Full: every file with an ONAM, plus the override-only files without one.
#[test]
#[ignore = "indexes 0.9 GB of plugins; run in release"]
fn corpus_onam_inventory_full() {
    let Some(dir) = data_dir() else { return };
    // Starfield.esm has no masters, so nothing to override or list.
    for name in CORPUS.into_iter().filter(|n| *n != "Starfield.esm") {
        let index = index_file(&dir.join(name), &IndexOptions::default()).unwrap();
        let inv = check_onam(name, &index);
        assert_eq!(
            !inv.listed.is_empty(),
            WITH_ONAM.contains(&name),
            "{name}: ONAM presence"
        );
        println!(
            "{name}: ONAM {:?}; top-level overrides of master records {} (none listed)",
            inv.listed, inv.top_level_overrides
        );
    }
}
