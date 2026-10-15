//! M4.A1 cell and world group structure on the official ESMs (`docs/corpus.md`, "Cell and
//! world groups"). Every rule `kitkraft_esm::cells` states is checked on every group and
//! record; a rule that fails fails the test, and the few facts the corpus does not hold
//! everywhere are listed as exceptions with file and FormID.
//!
//! - `corpus_group_rules_small` (fast): the five small files (two of them have cells).
//! - `corpus_group_rules_full` (ignored): all 14, printing the counts `docs/corpus.md`
//!   quotes.
//!
//! `cargo test -p kitkraft-esm --release --test cells -- --ignored --nocapture`

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines
)]

mod common;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Instant;

use kitkraft_esm::cells::{
    self, group_type as gt, Grid, CELL_UNITS, FLAG_PERSISTENT, PLACEHOLDER_TYPES,
};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions, PluginIndex};
use kitkraft_esm::{DecodeLimits, Signature};

fn sig(s: &[u8; 4]) -> Signature {
    Signature::new(s)
}

fn is_blueprint(name: &str) -> bool {
    name.starts_with("BlueprintShips")
}

/// Counts and exceptions for one or more files.
#[derive(Default)]
struct Facts {
    /// Rule name -> occurrences checked (every one holds, or the test fails).
    held: BTreeMap<String, u64>,
    /// Rule name -> exceptions (file and FormID or label), for the rules the corpus does not
    /// hold everywhere.
    exceptions: BTreeMap<String, Vec<String>>,
    /// Group type -> record type -> count.
    contents: BTreeMap<i32, BTreeMap<String, u64>>,
    /// Group type -> parent kind -> count.
    parents: BTreeMap<i32, BTreeMap<String, u64>>,
}

impl Facts {
    fn held(&mut self, rule: &str) {
        *self.held.entry(rule.to_string()).or_default() += 1;
    }

    fn check(&mut self, rule: &str, ok: bool, what: impl FnOnce() -> String) {
        assert!(ok, "rule {rule} fails: {}", what());
        self.held(rule);
    }

    fn except(&mut self, rule: &str, what: String) {
        self.exceptions
            .entry(rule.to_string())
            .or_default()
            .push(what);
    }

    fn print(&self) {
        println!("Group type | parent | count");
        for (t, m) in &self.parents {
            for (p, c) in m {
                println!("  {t} | {p} | {c}");
            }
        }
        println!("Group type | record type | count");
        for (t, m) in &self.contents {
            for (s, c) in m {
                println!("  {t} | {s} | {c}");
            }
        }
        println!("Rules held:");
        for (r, c) in &self.held {
            println!("  {c:>10}  {r}");
        }
        for (r, list) in &self.exceptions {
            println!("Exceptions to {r}: {}", list.len());
            for e in list.iter().take(20) {
                println!("    {e}");
            }
        }
    }
}

/// A group's children in file order: groups and records, as indexes into the index.
#[derive(Clone, Copy, Debug)]
enum Kid {
    G(usize),
    R(usize),
}

fn children(index: &PluginIndex) -> (Vec<Vec<Kid>>, Vec<Kid>) {
    let mut kids: Vec<Vec<(u64, Kid)>> = vec![Vec::new(); index.groups.len()];
    let mut top: Vec<(u64, Kid)> = Vec::new();
    for (i, g) in index.groups.iter().enumerate() {
        match g.parent {
            Some(p) => kids[p as usize].push((g.offset, Kid::G(i))),
            None => top.push((g.offset, Kid::G(i))),
        }
    }
    for (i, r) in index.records.iter().enumerate().skip(1) {
        match r.group {
            Some(p) => kids[p as usize].push((r.offset, Kid::R(i))),
            None => top.push((r.offset, Kid::R(i))),
        }
    }
    let strip = |mut v: Vec<(u64, Kid)>| {
        v.sort_by_key(|(o, _)| *o);
        v.into_iter().map(|(_, k)| k).collect::<Vec<Kid>>()
    };
    (kids.into_iter().map(strip).collect(), strip(top))
}

/// Check every structural rule on one file's index; `grids` maps exterior and persistent
/// cell FormIDs to their `XCLC` grid.
fn check_structure(name: &str, index: &PluginIndex, grids: &HashMap<u32, Grid>, f: &mut Facts) {
    let (kids, top) = children(index);
    let blueprint = is_blueprint(name);
    let tname = |g: usize| -> String {
        let h = index.groups[g].header;
        if h.group_type == 0 {
            format!("0 {}", h.label_signature())
        } else {
            h.group_type.to_string()
        }
    };
    // Top level: TES4, then type 0 groups only.
    f.check(
        "top level holds type 0 groups only",
        top.iter()
            .all(|k| matches!(k, Kid::G(g) if index.groups[*g].header.group_type == 0)),
        || name.to_string(),
    );
    let nested_only: Vec<Signature> = cells::CELL_CHILD_TYPES
        .iter()
        .chain(&cells::QUEST_CHILD_TYPES)
        .copied()
        .chain([sig(b"INFO"), sig(b"CELL")])
        .collect();
    for (gi, g) in index.groups.iter().enumerate() {
        let t = g.header.group_type;
        let parent = g.parent.map(|p| p as usize);
        *f.parents
            .entry(t)
            .or_default()
            .entry(parent.map_or("file".into(), tname))
            .or_default() += 1;
        let expected_parent: &[&str] = match t {
            0 => &["file"],
            1 => &["0 WRLD"],
            2 => &["0 CELL"],
            3 => &["2"],
            4 => &["1"],
            5 => &["4"],
            6 => &["1", "3", "5"],
            7 => &["10"],
            8 | 9 => &["6"],
            10 => &["0 QUST"],
            _ => &[],
        };
        let pname = parent.map_or("file".into(), tname);
        f.check(
            &format!("type {t} parent"),
            expected_parent.contains(&pname.as_str()),
            || format!("{name} group @{} under {pname}", g.offset),
        );
        let my = &kids[gi];
        let recs: Vec<usize> = my
            .iter()
            .filter_map(|k| if let Kid::R(r) = k { Some(*r) } else { None })
            .collect();
        let subs: Vec<usize> = my
            .iter()
            .filter_map(|k| if let Kid::G(c) = k { Some(*c) } else { None })
            .collect();
        for &r in &recs {
            *f.contents
                .entry(t)
                .or_default()
                .entry(index.records[r].signature.to_string())
                .or_default() += 1;
        }
        let allowed: Option<&[&[u8; 4]]> = match t {
            1 | 3 | 5 => Some(&[b"CELL"]),
            2 | 4 | 6 => Some(&[]),
            7 => Some(&[b"INFO"]),
            8 => Some(&[b"REFR", b"ACHR", b"PGRE", b"PHZD"]),
            9 => Some(&[b"NAVM", b"REFR", b"ACHR", b"PGRE", b"PHZD"]),
            10 => Some(&[b"DLBR", b"DIAL", b"SCEN"]),
            _ => None,
        };
        match allowed {
            Some(a) => {
                for &r in &recs {
                    let s = index.records[r].signature;
                    f.check(
                        &format!("type {t} record types"),
                        a.iter().any(|x| **x == s.0),
                        || format!("{name} {s} {:08X}", index.records[r].form_id),
                    );
                }
            }
            None => {
                for &r in &recs {
                    let s = index.records[r].signature;
                    f.check(
                        "type 0 holds no nested-only type",
                        !nested_only.contains(&s) && s == g.header.label_signature(),
                        || format!("{name} {s} in top {}", g.header.label_signature()),
                    );
                }
            }
        }
        let allowed_groups: &[i32] = match t {
            0 => match &g.header.label {
                b"WRLD" => &[1],
                b"CELL" => &[2],
                b"QUST" => &[10],
                _ => &[],
            },
            1 => &[6, 4],
            2 => &[3],
            3 | 5 => &[6],
            4 => &[5],
            6 => &[8, 9],
            10 => &[7],
            _ => &[],
        };
        for &c in &subs {
            let ct = index.groups[c].header.group_type;
            f.check(
                &format!("type {t} subgroup types"),
                allowed_groups.contains(&ct),
                || format!("{name} type {ct} in {t} @{}", g.offset),
            );
        }
        // Labels and order.
        let label = g.header.label_u32();
        match t {
            1 | 6 | 7 | 10 => {
                let pkids: &Vec<Kid> = &kids[parent.unwrap()];
                let pos = pkids
                    .iter()
                    .position(|k| matches!(k, Kid::G(x) if *x == gi))
                    .unwrap();
                let prev = pos.checked_sub(1).map(|p| pkids[p]);
                let ok = matches!(prev, Some(Kid::R(r)) if index.records[r].form_id == label);
                let want = match t {
                    1 => b"WRLD",
                    6 => b"CELL",
                    7 => b"DIAL",
                    _ => b"QUST",
                };
                let ok =
                    ok && matches!(prev, Some(Kid::R(r)) if index.records[r].signature.0 == *want);
                f.check(
                    &format!("type {t} follows the record its label names"),
                    ok,
                    || format!("{name} label {label:08X}"),
                );
            }
            8 | 9 => {
                let p = index.groups[parent.unwrap()].header;
                f.check(
                    &format!("type {t} label = its cell children label"),
                    p.label == g.header.label,
                    || format!("{name} {label:08X}"),
                );
            }
            _ => {}
        }
        // Record order.
        let ascending = |ids: &[u32]| ids.windows(2).all(|w| w[0] < w[1]);
        let ids = |sigs: Option<&[u8; 4]>| -> Vec<u32> {
            recs.iter()
                .filter(|&&r| sigs.is_none_or(|s| index.records[r].signature.0 == *s))
                .map(|&r| index.records[r].form_id)
                .collect()
        };
        match t {
            3 | 5 | 7 => f.check(
                &format!("type {t} records ascend by FormID"),
                ascending(&ids(None)),
                || format!("{name} group @{}", g.offset),
            ),
            0 if g.header.label == *b"WRLD" || g.header.label == *b"QUST" => f.check(
                "top WRLD and QUST records ascend by FormID",
                ascending(&ids(None)),
                || name.to_string(),
            ),
            10 => {
                let class = |r: usize| match &index.records[r].signature.0 {
                    b"DLBR" => 0,
                    b"DIAL" => 1,
                    _ => 2,
                };
                let keys: Vec<(u8, u32)> = recs
                    .iter()
                    .map(|&r| (class(r), index.records[r].form_id))
                    .collect();
                f.check(
                    "type 10 records ascend by (DLBR, DIAL, SCEN), FormID",
                    keys.windows(2).all(|w| w[0] < w[1]),
                    || format!("{name} {label:08X}"),
                );
            }
            _ => {}
        }
        // Children groups of the same type ascend by label.
        if matches!(t, 0 | 1 | 2 | 4) {
            let mut by_type: BTreeMap<i32, Vec<u32>> = BTreeMap::new();
            for &c in &subs {
                let h = index.groups[c].header;
                if matches!(h.group_type, 2..=5) {
                    by_type.entry(h.group_type).or_default().push(h.label_u32());
                }
            }
            for (ct, labels) in by_type {
                f.check(
                    &format!("type {ct} groups ascend by label (u32)"),
                    ascending(&labels),
                    || format!("{name} in group @{}", g.offset),
                );
            }
        }
        match t {
            1 => {
                // [CELL [6]] then type 4 groups.
                let shape: Vec<String> = my
                    .iter()
                    .map(|k| match k {
                        Kid::R(r) => index.records[*r].signature.to_string(),
                        Kid::G(c) => index.groups[*c].header.group_type.to_string(),
                    })
                    .collect();
                let cells = recs.len();
                f.check(
                    "type 1: at most one CELL, first",
                    cells <= 1 && (cells == 0 || shape[0] == "CELL"),
                    || format!("{name} {shape:?}"),
                );
                if let Some(&r) = recs.first() {
                    f.check(
                        "persistent cell has flag 0x400",
                        index.records[r].flags & FLAG_PERSISTENT != 0,
                        || name.to_string(),
                    );
                    f.check(
                        "persistent cell has XCLC",
                        grids.contains_key(&index.records[r].form_id),
                        || format!("{name} {:08X}", index.records[r].form_id),
                    );
                }
                f.held("persistent cells");
            }
            2 => {
                f.check("type 2 label is a digit", label < 10, || {
                    format!("{name} {label}")
                });
            }
            3 => {
                let block = index.groups[parent.unwrap()].header.label_u32();
                for &r in &recs {
                    let id = index.records[r].form_id;
                    f.check(
                        "interior cell: block, sub-block = last two digits of the object ID",
                        cells::interior_block(id) == (block, label),
                        || format!("{name} {id:08X} in {block}/{label}"),
                    );
                    f.check(
                        "interior cell: no flag 0x400",
                        index.records[r].flags & FLAG_PERSISTENT == 0,
                        || name.to_string(),
                    );
                    f.check("interior cell: no XCLC", !grids.contains_key(&id), || {
                        format!("{name} {id:08X}")
                    });
                    if id >> 24 == 0xFE && (id & 0x00FF_F000) != 0 {
                        f.except(
                            "interior cells only the 12-bit FE object ID places",
                            format!("{name} {id:08X}"),
                        );
                    }
                }
            }
            5 => {
                let block = index.groups[parent.unwrap()].header;
                for &r in &recs {
                    let id = index.records[r].form_id;
                    let grid = grids.get(&id).copied();
                    f.check("exterior cell has XCLC", grid.is_some(), || {
                        format!("{name} {id:08X}")
                    });
                    let grid = grid.unwrap();
                    f.check(
                        "exterior cell: sub-block label = grid / 8",
                        cells::grid_label(grid.sub_block()).unwrap() == g.header.label,
                        || format!("{name} {id:08X} {grid:?}"),
                    );
                    f.check(
                        "exterior cell: block label = grid / 32",
                        cells::grid_label(grid.block()).unwrap() == block.label,
                        || format!("{name} {id:08X} {grid:?}"),
                    );
                    f.check(
                        "exterior cell: no flag 0x400",
                        index.records[r].flags & FLAG_PERSISTENT == 0,
                        || name.to_string(),
                    );
                }
            }
            6 => {
                let types: Vec<i32> = subs
                    .iter()
                    .map(|&c| index.groups[c].header.group_type)
                    .collect();
                let pt = index.groups[parent.unwrap()].header.group_type;
                if pt == 1 {
                    f.check(
                        "persistent cell children: type 8 only",
                        types == [8],
                        || format!("{name} {types:?}"),
                    );
                }
                if pt == 5 {
                    f.check("exterior cell children: type 9 only", types == [9], || {
                        format!("{name} {types:?}")
                    });
                }
                if types.len() == 2 {
                    let want = if blueprint { [8, 9] } else { [9, 8] };
                    f.check(
                        if blueprint {
                            "cell children order 8, 9 (BlueprintShips)"
                        } else {
                            "cell children order 9, 8 (official pipeline)"
                        },
                        types == want,
                        || format!("{name} {label:08X} {types:?}"),
                    );
                }
                if types.is_empty() {
                    f.except(
                        "cell children groups that are empty",
                        format!("{name} {label:08X}"),
                    );
                    assert!(blueprint, "{name}: empty cell children group {label:08X}");
                }
            }
            8 | 9 => {
                for &r in &recs {
                    let loc = &index.records[r];
                    let pers = loc.flags & FLAG_PERSISTENT != 0;
                    f.check(
                        &format!(
                            "type {t} records: flag 0x400 {}",
                            if t == 8 { "set" } else { "clear" }
                        ),
                        pers == (t == 8),
                        || format!("{name} {} {:08X}", loc.signature, loc.form_id),
                    );
                }
                if t == 9 {
                    let navm: Vec<bool> = recs
                        .iter()
                        .map(|&r| index.records[r].signature.0 == *b"NAVM")
                        .collect();
                    if navm.contains(&true) {
                        let first = navm
                            .iter()
                            .position(|n| !n)
                            .is_none_or(|p| navm[p..].iter().all(|n| !n));
                        if blueprint {
                            if !first {
                                f.except(
                                    "NAVM not first in type 9 (BlueprintShips)",
                                    format!("{name} {label:08X}"),
                                );
                            } else {
                                f.held("NAVM first in type 9 (BlueprintShips)");
                            }
                        } else {
                            f.check("NAVM first in type 9 (official pipeline)", first, || {
                                format!("{name} {label:08X}")
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Placeholder top-level groups: empty, and exactly the placeholder types the file has
    // nested records of (official pipeline); none in the BlueprintShips files.
    let empty_tops: BTreeSet<String> = index
        .groups
        .iter()
        .enumerate()
        .filter(|(i, g)| g.parent.is_none() && kids[*i].is_empty())
        .map(|(_, g)| g.header.label_signature().to_string())
        .collect();
    let has: BTreeSet<String> = PLACEHOLDER_TYPES
        .iter()
        .filter(|s| index.records.iter().any(|r| r.signature == **s))
        .map(|s| s.to_string())
        .collect();
    let want = if blueprint { BTreeSet::new() } else { has };
    f.check(
        "empty top-level groups = placeholder types with records",
        empty_tops == want,
        || format!("{name}: empty {empty_tops:?}, expected {want:?}"),
    );
}

/// `XCLC` grids of every cell that has one, and the position check of exterior temporary
/// references (`DATA` floats 0 and 1 against the cell's square at `CELL_UNITS`).
fn read_cells(path: &Path, index: &PluginIndex, f: &mut Facts, name: &str) -> HashMap<u32, Grid> {
    let limits = DecodeLimits::default();
    let mut file = BufReader::new(File::open(path).unwrap());
    let mut grids = HashMap::new();
    for loc in &index.records {
        if loc.signature != sig(b"CELL") {
            continue;
        }
        let rec = read_indexed(&mut file, loc, Some(&limits)).unwrap();
        let subs = rec.subrecords().unwrap();
        let xclc: Vec<_> = subs
            .iter()
            .filter(|s| s.signature() == sig(b"XCLC"))
            .collect();
        f.check("a CELL has at most one XCLC", xclc.len() <= 1, || {
            format!("{name} {:08X}", loc.form_id)
        });
        if let Some(x) = xclc.first() {
            grids.insert(loc.form_id, Grid::from_xclc(x.data()).unwrap());
        }
    }
    // Exterior temporary references.
    let exterior: HashMap<u32, Grid> = index
        .records
        .iter()
        .filter(|r| {
            r.group.is_some_and(|g| {
                index.groups[g as usize].header.group_type == gt::EXTERIOR_SUB_BLOCK
            })
        })
        .filter_map(|r| Some((r.form_id, *grids.get(&r.form_id)?)))
        .collect();
    let mut outside: Vec<(f64, String)> = Vec::new();
    for loc in &index.records {
        let Some(g) = loc.group else { continue };
        let h = index.groups[g as usize].header;
        if h.group_type != gt::CELL_TEMPORARY || loc.signature == sig(b"NAVM") {
            continue;
        }
        let Some(&grid) = exterior.get(&h.label_u32()) else {
            continue;
        };
        let rec = read_indexed(&mut file, loc, Some(&limits)).unwrap();
        let Some(data) = rec
            .subrecords()
            .unwrap()
            .iter()
            .find(|s| s.signature() == sig(b"DATA"))
        else {
            f.check(
                "exterior temporary references without DATA are deleted (flag 0x20)",
                loc.flags & 0x20 != 0,
                || format!("{name} {} {:08X}", loc.signature, loc.form_id),
            );
            continue;
        };
        let d = data.data();
        let x = f32::from_le_bytes(d[0..4].try_into().unwrap());
        let y = f32::from_le_bytes(d[4..8].try_into().unwrap());
        f.held("exterior temporary references with DATA");
        if Grid::of_position(x, y) == Some(grid) {
            f.held("exterior temporary references inside their cell");
        } else {
            let lo_x = f64::from(grid.x) * f64::from(CELL_UNITS);
            let lo_y = f64::from(grid.y) * f64::from(CELL_UNITS);
            let dx = (lo_x - f64::from(x))
                .max(f64::from(x) - (lo_x + f64::from(CELL_UNITS)))
                .max(0.0);
            let dy = (lo_y - f64::from(y))
                .max(f64::from(y) - (lo_y + f64::from(CELL_UNITS)))
                .max(0.0);
            outside.push((
                dx.max(dy),
                format!(
                    "{name} {} {:08X} at ({x}, {y}) in cell {grid:?}",
                    loc.signature, loc.form_id
                ),
            ));
        }
    }
    outside.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (d, what) in outside {
        let bucket = if d < 1.0 {
            "< 1"
        } else if d < 10.0 {
            "1-10"
        } else if d < 100.0 {
            "10-100"
        } else {
            ">= 100"
        };
        f.except(
            &format!("exterior temporary references outside their cell by {bucket} units"),
            format!("{what}: {d:.2}"),
        );
    }
    grids
}

fn rules(names: &[&str]) -> Option<Facts> {
    let files = common::files("cells", names)?;
    let mut facts = Facts::default();
    for file in &files {
        let started = Instant::now();
        let index = index_file(&file.path, &IndexOptions::default()).unwrap();
        let grids = read_cells(&file.path, &index, &mut facts, &file.name);
        check_structure(&file.name, &index, &grids, &mut facts);
        println!("{}: {:.2} s", file.name, started.elapsed().as_secs_f64());
    }
    Some(facts)
}

#[test]
fn corpus_group_rules_small() {
    let Some(facts) = rules(&common::SMALL) else {
        return;
    };
    // SFBGS004 and SFBGS047 have cells; SFBGS047 has a world.
    assert!(facts.held["type 3 records ascend by FormID"] >= 2);
    assert!(facts.held["exterior cell: sub-block label = grid / 8"] >= 1);
    assert!(facts.held["persistent cells"] >= 1);
}

#[test]
#[ignore]
fn corpus_group_rules_full() {
    let names: Vec<String> = common::manifest().into_iter().map(|e| e.name).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let Some(facts) = rules(&names) else {
        return;
    };
    facts.print();
    let inside = facts.held["exterior temporary references inside their cell"] as f64;
    let all = facts.held["exterior temporary references with DATA"] as f64;
    println!(
        "inside their cell: {inside} of {all} ({:.3} %)",
        100.0 * inside / all
    );
    assert!(inside / all > 0.99);
}
