//! M4.A6 (corpus half): which overridden records the official files list in `TES4` `ONAM`.
//!
//! In the five files with an `ONAM` (SFBGS003, SFBGS006, SFBGS00D, SFBGS050,
//! ShatteredSpace), a record is listed exactly when it is an override (its FormID belongs to
//! a master) of a record the master has, sitting in a group of type 7 (topic children:
//! `INFO`), 9 (cell temporary children: `REFR`, `ACHR`, `NAVM`) or 10 (quest children:
//! `DIAL`, `SCEN`); overrides in type 8 (persistent children), cells and top-level records
//! are never listed, nor own records, nor overrides of records the master lacks. The files
//! without an `ONAM` have no override such a rule would list. The list's order is neither
//! file order nor FormID order in four of the five (printed); `cells::onam_forms` finds the
//! same set from the parsed tree, and `Tes4::set_overridden_forms` puts the list back where
//! every file has it.
//!
//! `cargo test -p kitcraft-esm --release --test onam -- --ignored --nocapture`

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::collections::{BTreeMap, HashMap, HashSet};

use kitcraft_esm::cells;
use kitcraft_esm::index::{index_file, IndexOptions, PluginIndex};
use kitcraft_esm::tes4::Tes4;
use kitcraft_esm::{Entry, Plugin, WriteOptions};

/// Own stored FormIDs of a full-scale master, by object ID.
fn own_objects(index: &PluginIndex) -> HashSet<u32> {
    let prefix = u32::try_from(Tes4::new(&index.header).masters().len()).unwrap();
    index
        .records
        .iter()
        .skip(1)
        .filter(|r| r.form_id >> 24 == prefix)
        .map(|r| r.form_id & 0x00FF_FFFF)
        .collect()
}

#[test]
#[ignore]
fn corpus_onam_membership_full() {
    let Some(files) = common::all_files("onam") else {
        return;
    };
    let indexes: HashMap<String, PluginIndex> = files
        .iter()
        .map(|f| {
            (
                f.name.to_ascii_lowercase(),
                index_file(&f.path, &IndexOptions::default()).unwrap(),
            )
        })
        .collect();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for file in &files {
        let index = &indexes[&file.name.to_ascii_lowercase()];
        let tes4 = Tes4::new(&index.header);
        let masters: Vec<String> = tes4.masters().iter().map(|m| m.name_lossy()).collect();
        if masters.is_empty() {
            continue;
        }
        // Small and medium masters would change the FormID classes; the files with
        // overrides of children have full masters only.
        let master_objects: Vec<HashSet<u32>> = masters
            .iter()
            .map(|m| own_objects(&indexes[&m.to_ascii_lowercase()]))
            .collect();
        let onam = tes4.overridden_forms();
        let listed: HashSet<u32> = onam.iter().copied().collect();
        let mut expected: Vec<u32> = Vec::new();
        for r in index.records.iter().skip(1) {
            let hi = usize::try_from(r.form_id >> 24).unwrap();
            if hi >= masters.len() {
                assert!(
                    !listed.contains(&r.form_id),
                    "{} own {:08X} listed",
                    file.name,
                    r.form_id
                );
                continue;
            }
            let t = r
                .group
                .map_or(-1, |g| index.groups[g as usize].header.group_type);
            let in_master = master_objects[hi].contains(&(r.form_id & 0x00FF_FFFF));
            let rule = matches!(t, 7 | 9 | 10) && in_master;
            let is = listed.contains(&r.form_id);
            *counts
                .entry(format!(
                    "{}: {} in type {t}{}: listed {is}",
                    file.name,
                    r.signature,
                    if in_master {
                        ""
                    } else {
                        " (not in its master)"
                    }
                ))
                .or_default() += 1;
            assert_eq!(
                rule, is,
                "{} {} {:08X} type {t}",
                file.name, r.signature, r.form_id
            );
            if rule {
                expected.push(r.form_id);
            }
        }
        assert_eq!(
            listed.len(),
            onam.len(),
            "{}: ONAM repeats a FormID",
            file.name
        );
        assert_eq!(expected.len(), onam.len(), "{}", file.name);
        let file_order = expected == onam;
        let mut sorted = expected.clone();
        sorted.sort_unstable();
        let id_order = sorted == onam;
        println!(
            "{}: ONAM {} entries; file order {file_order}, ascending FormID {id_order}",
            file.name,
            onam.len()
        );
        if onam.is_empty() {
            continue;
        }
        // The writer side: `cells::onam_forms` finds the same set from the tree, and
        // `set_overridden_forms` with the file's own order puts ONAM back where it was.
        let bytes = std::fs::read(&file.path).unwrap();
        let plugin = Plugin::parse(&bytes).unwrap();
        let found = cells::onam_forms(&plugin, |id| {
            let hi = usize::try_from(id >> 24).unwrap();
            master_objects
                .get(hi)
                .is_some_and(|m| m.contains(&(id & 0x00FF_FFFF)))
        });
        assert_eq!(found, expected, "{}", file.name);
        let mut header = plugin.header().clone();
        assert!(Tes4::set_overridden_forms(&mut header, &[]));
        assert!(Tes4::new(&header).overridden_forms().is_empty());
        assert!(Tes4::set_overridden_forms(&mut header, &onam));
        let mut a = Vec::new();
        let mut b = Vec::new();
        Entry::Record(header)
            .write_to(&mut a, 0, &WriteOptions::default())
            .unwrap();
        Entry::Record(plugin.header().clone())
            .write_to(&mut b, 0, &WriteOptions::default())
            .unwrap();
        assert_eq!(a, b, "{}: ONAM position", file.name);
    }
    for (k, c) in &counts {
        println!("{c:>8}  {k}");
    }
}
