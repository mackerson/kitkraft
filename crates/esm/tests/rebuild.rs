//! M4.A2 rebuild gate (`docs/corpus.md`, "Cell and world groups", "Rebuild gate").
//!
//! Every record of an official ESM is upserted into an empty plugin with the same `TES4`
//! through a computed path (`cells::placement`, from the record, its flags and the world,
//! cell, quest or topic it belongs to; never the path it was read from), in file order and
//! in a seeded shuffled order. The result must equal the original byte for byte.
//!
//! - `corpus_rebuild_small` (fast): the five small files, whole, in file order and two
//!   shuffles.
//! - `corpus_rebuild_full` (ignored): every top-level group of all 14 files, one at a time
//!   (memory: one group and its rebuild). The two `BlueprintShips` files are compared after
//!   listed normalizations (empty cell children groups dropped, group stamps set to the
//!   style's), which the test prints.
//!
//! `cargo test -p kitcraft-esm --release --test rebuild -- --ignored --nocapture`

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hasher;
use std::path::Path;
use std::time::Instant;

use common::hostile::Rng;
use kitcraft_esm::cells::{
    self, group_type as gt, CellPlace, Grid, Parent, PlacementStyle, FLAG_PERSISTENT,
};
use kitcraft_esm::driver::visit_file;
use kitcraft_esm::index::{index_file, IndexOptions};
use kitcraft_esm::{
    DecodeLimits, Entry, Group, GroupHeader, ParseOptions, Plugin, Record, Signature, WriteOptions,
};

fn sig(s: &[u8; 4]) -> Signature {
    Signature::new(s)
}

fn is_blueprint(name: &str) -> bool {
    name.starts_with("BlueprintShips")
}

/// Each record of a top-level entry with the parent its path implies, in file order. The
/// parent names only the world, cell, quest or topic (the label of the enclosing children
/// group); everything else about the path (block and sub-block numbers and grids, group
/// types, persistent or temporary) is computed by `cells::placement`.
struct Placing<'a> {
    record: Record<'a>,
    parent: Parent,
    /// Group type the record was read from (for keeping the order of records the corpus
    /// gives no rule for).
    group_type: i32,
    /// Index of the group it was read from, among the entry's groups.
    group: usize,
}

fn collect<'a>(entry: &Entry<'a>, limits: &DecodeLimits) -> Vec<Placing<'a>> {
    // First pass: places of every cell.
    fn walk<'p, 'a>(
        g: &'p Group<'a>,
        stack: &mut Vec<GroupHeader>,
        out: &mut Vec<(Vec<GroupHeader>, &'p Record<'a>, usize)>,
        counter: &mut usize,
    ) {
        stack.push(g.header);
        let me = *counter;
        *counter += 1;
        for c in g.children() {
            match c {
                Entry::Group(sub) => walk(sub, stack, out, counter),
                Entry::Record(r) => out.push((stack.clone(), r, me)),
            }
        }
        stack.pop();
    }
    let Entry::Group(top) = entry else {
        return Vec::new();
    };
    let mut flat = Vec::new();
    let mut counter = 0;
    walk(top, &mut Vec::new(), &mut flat, &mut counter);
    let mut places: HashMap<u32, CellPlace> = HashMap::new();
    for (path, r, _) in &flat {
        if r.header.signature != sig(b"CELL") {
            continue;
        }
        let world = path
            .iter()
            .find(|h| h.group_type == gt::WORLD_CHILDREN)
            .map(|h| h.label_u32());
        let place = match world {
            None => CellPlace::INTERIOR,
            Some(world) if r.header.flags & FLAG_PERSISTENT != 0 => CellPlace::Persistent { world },
            Some(world) => {
                let subs = match r.subrecords() {
                    Some(s) => s.to_vec(),
                    None => r.decode(limits).unwrap().unwrap(),
                };
                let x = subs.iter().find(|s| s.signature() == sig(b"XCLC")).unwrap();
                CellPlace::Exterior {
                    world,
                    grid: Grid::from_xclc(x.data()).unwrap(),
                }
            }
        };
        places.insert(r.header.form_id, place);
    }
    flat.into_iter()
        .map(|(path, r, group)| {
            let last = path.last().unwrap();
            let label = last.label_u32();
            let parent = match last.group_type {
                gt::CELL_PERSISTENT | gt::CELL_TEMPORARY => Parent::Cell {
                    place: places[&label],
                    cell: label,
                },
                gt::QUEST_CHILDREN => Parent::Quest { quest: label },
                gt::TOPIC_CHILDREN => Parent::Topic {
                    quest: path[path.len() - 2].label_u32(),
                    topic: label,
                },
                _ if r.header.signature == sig(b"CELL") => {
                    Parent::CellAt(places[&r.header.form_id])
                }
                _ => Parent::None,
            };
            Placing {
                record: r.clone(),
                parent,
                group_type: last.group_type,
                group,
            }
        })
        .collect()
}

/// The order to insert in: file order, or a seeded shuffle that keeps (a) parents before
/// their children groups' records, and (b) file order where the corpus gives no rule (the
/// records of types 8 and 9 within each group, and the records of top-level groups other
/// than WRLD and QUST).
fn order(items: &[Placing<'_>], shuffle: Option<u64>) -> Vec<usize> {
    let Some(seed) = shuffle else {
        return (0..items.len()).collect();
    };
    let mut rng = Rng::new(seed);
    let mut unit_key: HashMap<usize, u64> = HashMap::new();
    let mut keys: Vec<(u8, u64, usize)> = Vec::with_capacity(items.len());
    for (i, p) in items.iter().enumerate() {
        let s = &p.record.header.signature.0;
        let phase = match s {
            b"WRLD" | b"QUST" => 0,
            b"CELL" | b"DIAL" => 1,
            _ => 2,
        };
        let random = rng.next();
        let unit = match p.group_type {
            gt::CELL_PERSISTENT | gt::CELL_TEMPORARY => *unit_key.entry(p.group).or_insert(random),
            gt::TOP if !matches!(s, b"WRLD" | b"QUST") => 0,
            _ => random,
        };
        keys.push((phase, unit, i));
    }
    keys.sort_unstable();
    keys.into_iter().map(|(_, _, i)| i).collect()
}

/// Hash of what `entry` writes.
fn written_hash(entry: &Entry<'_>) -> (u64, u64) {
    struct H(std::collections::hash_map::DefaultHasher, u64);
    impl std::io::Write for H {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.write(buf);
            self.1 += buf.len() as u64;
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut h = H(Default::default(), 0);
    entry.write_to(&mut h, 0, &WriteOptions::default()).unwrap();
    (h.0.finish(), h.1)
}

/// The first place two trees differ, as a path.
fn first_difference(a: &Entry<'_>, b: &Entry<'_>, path: &str) -> Option<String> {
    match (a, b) {
        (Entry::Group(x), Entry::Group(y)) => {
            let here = format!(
                "{path} > GRUP {} {:08X}",
                x.header.group_type,
                x.header.label_u32()
            );
            if x.header != y.header {
                return Some(format!("{here}: header {:?} vs {:?}", x.header, y.header));
            }
            for (i, (c, d)) in x.children().iter().zip(y.children()).enumerate() {
                if let Some(diff) = first_difference(c, d, &format!("{here} [{i}]")) {
                    return Some(diff);
                }
            }
            (x.children().len() != y.children().len()).then(|| {
                format!(
                    "{here}: {} children vs {}",
                    x.children().len(),
                    y.children().len()
                )
            })
        }
        (Entry::Record(x), Entry::Record(y)) => (x.header != y.header || x.body() != y.body())
            .then(|| {
                format!(
                    "{path} > {} {:08X} vs {} {:08X}",
                    x.header.signature, x.header.form_id, y.header.signature, y.header.form_id
                )
            }),
        _ => Some(format!("{path}: a group against a record")),
    }
}

/// Make `entry` what a writer of `style` can produce from its records alone, listing each
/// change: empty cell children groups are dropped and group stamps set to the style's.
fn normalize(entry: &mut Entry<'_>, style: &PlacementStyle, path: &str, changes: &mut Vec<String>) {
    let Entry::Group(g) = entry else { return };
    let here = format!(
        "{path} > GRUP {} {:08X}",
        g.header.group_type,
        g.header.label_u32()
    );
    let want = style.header(g.header.label, g.header.group_type);
    if want != g.header {
        changes.push(format!(
            "{here}: stamps ({}, {}, {}) where the style writes ({}, {}, {})",
            g.header.timestamp,
            g.header.version_control,
            g.header.unknown,
            want.timestamp,
            want.version_control,
            want.unknown
        ));
        g.header = want;
    }
    let mut dropped = Vec::new();
    let kids = g.children_mut();
    kids.retain(|c| match c {
        Entry::Group(x) if x.header.group_type == gt::CELL_CHILDREN && x.children().is_empty() => {
            dropped.push(format!(
                "{here} > GRUP 6 {:08X}: empty cell children group",
                x.header.label_u32()
            ));
            false
        }
        _ => true,
    });
    changes.extend(dropped);
    for c in kids.iter_mut() {
        normalize(c, style, &here, changes);
    }
}

/// Rebuild one top-level entry through computed paths. Returns the first difference
/// (`None` when identical) after [`normalize`] (when `normalized`), and the changes it
/// made.
fn rebuild_entry(
    tes4: &Record<'_>,
    entry: &Entry<'_>,
    style: &PlacementStyle,
    shuffle: Option<u64>,
    normalized: bool,
    placeholders: &mut BTreeSet<String>,
) -> (Option<String>, Vec<String>) {
    let Entry::Group(top) = entry else {
        return (None, Vec::new());
    };
    let limits = DecodeLimits::default();
    let items = collect(entry, &limits);
    let sequence = order(&items, shuffle);
    let mut plugin = Plugin::new(tes4.clone());
    let mut items: Vec<Option<Placing<'_>>> = items.into_iter().map(Some).collect();
    for i in sequence {
        let p = items[i].take().unwrap();
        let path = cells::placement(&p.record.header, &p.parent, style)
            .unwrap_or_else(|e| panic!("{}: {e}", p.record.header.signature));
        plugin.upsert_styled(&path, p.record, style).unwrap();
    }
    let mut rebuilt = None;
    for e in plugin.entries() {
        let Entry::Group(g) = e else { continue };
        if g.header.same_group(&top.header) {
            rebuilt = Some(e);
        } else if g.children().is_empty() {
            placeholders.insert(g.header.label_signature().to_string());
        } else {
            let other = format!(
                "records went into another top-level group {}",
                g.header.label_signature()
            );
            return (Some(other), Vec::new());
        }
    }
    if top.children().is_empty() {
        // An empty original group: a placeholder, created only by the records of other
        // groups (checked against `placeholders` by the caller).
        return (None, Vec::new());
    }
    let Some(rebuilt) = rebuilt else {
        return (Some("no group rebuilt".into()), Vec::new());
    };
    let mut changes = Vec::new();
    let mut original = entry.clone();
    if normalized {
        normalize(&mut original, style, "", &mut changes);
    }
    if written_hash(rebuilt) == written_hash(&original) {
        return (None, changes);
    }
    let diff = first_difference(&original, rebuilt, "")
        .unwrap_or_else(|| "bytes differ, trees equal".into());
    (Some(diff), changes)
}

fn style_for(name: &str, path: &Path) -> PlacementStyle {
    if !is_blueprint(name) {
        return PlacementStyle::OFFICIAL;
    }
    let index = index_file(path, &IndexOptions::default()).unwrap();
    PlacementStyle::blueprint(index.groups[0].header.timestamp)
}

/// Rebuild every top-level group of `path`; returns the differences that remain and the
/// listed normalizations (`BlueprintShips` files only).
fn rebuild_file(name: &str, path: &Path, shuffle: Option<u64>) -> (Vec<String>, Vec<String>) {
    let style = style_for(name, path);
    let mut tes4: Option<Record<'static>> = None;
    let mut diffs = Vec::new();
    let mut changes = Vec::new();
    let mut placeholders = BTreeSet::new();
    let mut empty_tops = BTreeSet::new();
    visit_file(path, &ParseOptions::default(), |entry| {
        match entry {
            Entry::Record(r) => tes4 = Some(r.clone().into_owned()),
            Entry::Group(g) => {
                if g.children().is_empty() {
                    empty_tops.insert(g.header.label_signature().to_string());
                }
                let tes4 = tes4.as_ref().unwrap();
                let blueprint = is_blueprint(name);
                let (diff, normalized) =
                    rebuild_entry(tes4, entry, &style, shuffle, blueprint, &mut placeholders);
                if let Some(d) = diff {
                    diffs.push(format!("{name}: top {}: {d}", g.header.label_signature()));
                }
                changes.extend(normalized.into_iter().map(|c| format!("{name}: {c}")));
            }
        }
        Ok(())
    })
    .unwrap();
    if placeholders != empty_tops {
        diffs.push(format!(
            "{name}: placeholder groups {placeholders:?}, file has {empty_tops:?}"
        ));
    }
    (diffs, changes)
}

/// Whole small files: the rebuilt plugin's bytes equal the file's.
#[test]
fn corpus_rebuild_small() {
    let Some(files) = common::files("cells", &common::SMALL) else {
        return;
    };
    for file in &files {
        let bytes = std::fs::read(&file.path).unwrap();
        let original = Plugin::parse(&bytes).unwrap();
        for shuffle in [None, Some(1), Some(2)] {
            let mut plugin = Plugin::new(original.header().clone());
            for entry in original.entries() {
                let items = collect(entry, &DecodeLimits::default());
                let picks = order(&items, shuffle);
                let mut items: Vec<Option<Placing<'_>>> = items.into_iter().map(Some).collect();
                for i in picks {
                    let p = items[i].take().unwrap();
                    let path =
                        cells::placement(&p.record.header, &p.parent, &PlacementStyle::OFFICIAL)
                            .unwrap();
                    plugin
                        .upsert_styled(&path, p.record, &PlacementStyle::OFFICIAL)
                        .unwrap();
                }
            }
            let rebuilt = plugin.to_bytes().unwrap();
            if rebuilt != bytes {
                let reparsed = Plugin::parse(&rebuilt).unwrap();
                let diff = original
                    .entries()
                    .iter()
                    .zip(reparsed.entries())
                    .find_map(|(a, b)| first_difference(a, b, ""));
                panic!("{} (shuffle {shuffle:?}) differs: {diff:?}", file.name);
            }
        }
        println!(
            "{}: rebuilt identical (file order and 2 shuffles)",
            file.name
        );
    }
}

#[test]
#[ignore]
fn corpus_rebuild_full() {
    let Some(files) = common::all_files("cells") else {
        return;
    };
    for file in &files {
        for shuffle in [None, Some(7)] {
            let started = Instant::now();
            let (diffs, changes) = rebuild_file(&file.name, &file.path, shuffle);
            println!(
                "{} (shuffle {shuffle:?}): {} differences, {} listed normalizations, {:.1} s",
                file.name,
                diffs.len(),
                changes.len(),
                started.elapsed().as_secs_f64()
            );
            if shuffle.is_none() {
                let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
                for c in &changes {
                    let kind = c.rsplit(": ").next().unwrap_or_default().to_string();
                    *kinds.entry(kind).or_default() += 1;
                }
                for (k, n) in kinds {
                    println!("    {n:>6} x {k}");
                }
                for c in changes.iter().filter(|c| c.contains("empty cell")) {
                    println!("    {c}");
                }
            }
            for d in diffs.iter().take(10) {
                println!("    {d}");
            }
            assert!(diffs.is_empty(), "{}: {diffs:?}", file.name);
        }
    }
}
