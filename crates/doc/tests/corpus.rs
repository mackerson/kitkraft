//! FormID facts on the official files (M2.D3 a/b, M2.D4). Reads `KITCRAFT_DATA_DIR` in
//! place; skips when it is unset.
//!
//! - Fast: the small official files resolve every record header FormID through masters
//!   whose scales come from their own headers.
//! - `#[ignore]` (release): all 14 files in load order; every override names a record that
//!   exists in its master, and the largest own object ID per scale.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitcraft_doc::index::read_header;
use kitcraft_doc::{
    GlobalForm, IndexCache, LoadOptions, LoadOrder, MasterMap, PluginIndex, PluginSpec, RawRef,
    Scale,
};

/// The 14 official files in the owner's `loadorder.txt` order (2026-10-14).
const OFFICIAL: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS006.esm",
    "SFBGS003.esm",
    "ShatteredSpace.esm",
    "SFBGS004.esm",
    "SFBGS00D.esm",
    "SFBGS050.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS047.esm",
];

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("KITCRAFT_DATA_DIR")?);
    if dir.is_dir() {
        Some(dir)
    } else {
        eprintln!(
            "skip: KITCRAFT_DATA_DIR {} is not a directory",
            dir.display()
        );
        None
    }
}

#[test]
fn small_official_files_resolve_every_header_formid() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITCRAFT_DATA_DIR unset");
        return;
    };
    for name in [
        "Constellation.esm",
        "OldMars.esm",
        "SFBGS007.esm",
        "SFBGS004.esm",
        "SFBGS047.esm",
    ] {
        let path = dir.join(name);
        let index = PluginIndex::build(&path).unwrap();
        let own = index.header.flags.scale().unwrap();
        // Masters' scales from their own headers; load-order indices are their positions
        // in the master list, the file itself comes after them.
        let masters: Vec<(u16, Scale)> = index
            .header
            .masters
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let h = read_header(&dir.join(m)).unwrap();
                (i as u16, h.flags.scale().unwrap())
            })
            .collect();
        let self_index = masters.len() as u16;
        let map = MasterMap::new(name, self_index, own, &masters);
        let mut own_records = 0;
        for r in &index.records {
            let g = map
                .resolve(r.form_id)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            if g.plugin == self_index {
                own_records += 1;
                assert!(g.object_id <= own.max_object_id(), "{name}: {g:?}");
                // The own records use the file's own class prefix.
                assert_eq!(RawRef::decode(r.form_id).class, own, "{name}");
            }
            assert_eq!(map.to_raw(g), Some(r.form_id), "{name}");
        }
        assert!(own_records > 0, "{name}");
    }
}

#[test]
#[ignore = "release: indexes all 14 official files (about 2.5 GB)"]
fn corpus_official_load_order_resolves_and_overrides_exist() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITCRAFT_DATA_DIR unset");
        return;
    };
    let specs: Vec<PluginSpec> = OFFICIAL
        .iter()
        .map(|n| PluginSpec {
            name: n.to_string(),
            path: dir.join(n),
        })
        .collect();
    let cache = IndexCache::default_location();
    let started = std::time::Instant::now();
    let lo = LoadOrder::load(specs, &LoadOptions { cache, threads: 0 }).unwrap();
    eprintln!(
        "loaded in {:?} (index {} ms, table {} ms), {} versions",
        started.elapsed(),
        lo.index_ms,
        lo.table_ms,
        lo.forms.len()
    );
    // M2.D4: every master of every official file resolves.
    for (i, p) in lo.plugins.iter().enumerate() {
        assert!(p.error.is_none(), "{}: {:?}", p.name, p.error);
        assert!(p.missing_masters().is_empty(), "{}", p.name);
        assert!(p.late_masters(i as u16).is_empty(), "{}", p.name);
        assert!(p.map.is_ok(), "{}: {:?}", p.name, p.map);
        assert_eq!(lo.forms.unresolved_count(i as u16), 0, "{}", p.name);
    }
    // M2.D3 (a): an override names a record its master defines.
    let mut overrides: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut orphans: Vec<String> = Vec::new();
    for run in lo.forms.runs() {
        let form = run[0].form();
        let base_present = run.iter().any(|v| v.plugin == form.plugin);
        for v in run.iter().filter(|v| v.plugin != form.plugin) {
            let e = overrides
                .entry(lo.plugins[v.plugin as usize].name.clone())
                .or_default();
            e.0 += 1;
            if !base_present {
                e.1 += 1;
                let r = lo.record(v.plugin, v.record).unwrap();
                let line = format!(
                    "{} {} {:08X} -> {}",
                    lo.plugins[v.plugin as usize].name,
                    r.signature(),
                    r.form_id,
                    lo.form_label(form)
                );
                eprintln!("no base record: {line}");
                orphans.push(line);
            }
        }
    }
    eprintln!("| Plugin | Overrides of master records | Without a base record |");
    for (name, (n, missing)) in &overrides {
        eprintln!("| {name} | {n} | {missing} |");
    }
    let dangling: u64 = overrides.values().map(|v| v.1).sum();
    // M2.D3 (b): largest own object ID per scale and flag.
    eprintln!("| Plugin | Flags | Scale | Own records | Largest own object ID |");
    for (i, p) in lo.plugins.iter().enumerate() {
        let flags = p.header().unwrap().flags;
        let own: Vec<u32> = p
            .records()
            .iter()
            .filter_map(|r| lo.resolve(i as u16, r.form_id).ok())
            .filter(|g| g.plugin == i as u16)
            .map(|g| g.object_id)
            .collect();
        let max = own.iter().max().copied().unwrap_or(0);
        let scale = p.scale().unwrap();
        eprintln!(
            "| {} | 0x{:X} | {} | {} | 0x{max:X} |",
            p.name,
            flags.0,
            scale.as_str(),
            own.len()
        );
        assert!(max <= scale.max_object_id(), "{}", p.name);
    }
    // Corpus fact (2026-10-14): 15 overrides have no base record, all SFBGS050.esm records
    // addressed to SFBGS00D.esm (5 REFR, 9 NAVM, 1 PROJ); every other override, including
    // SFBGS047's two through its second *full* master, names an existing record.
    assert_eq!(dangling, 15, "overrides without a base record");
    assert!(orphans
        .iter()
        .all(|o| o.starts_with("SFBGS050.esm ") && o.contains("-> SFBGS00D.esm:")));
    assert_eq!(overrides["SFBGS047.esm"], (2, 0));
    // The winner of a form overridden by a DLC is the later plugin.
    let some = lo
        .forms
        .runs()
        .find(|run| run.len() >= 2)
        .expect("an override");
    let winner = lo.forms.winner(GlobalForm::from_key(some[0].key)).unwrap();
    assert_eq!(winner.plugin, some.last().unwrap().plugin);
}
