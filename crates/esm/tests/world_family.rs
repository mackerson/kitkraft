//! The world-family definitions (WRLD, CELL, REFR, ACHR, NAVM, LCTN, LCRT) against the
//! official ESMs, read in place through the record index (see docs/corpus.md and
//! docs/schema/world-family.md).
//!
//! Every record of the seven types is read with [`read_indexed`] (compressed ones
//! decompressed), decoded through its generated type and encoded again. The checks:
//!
//! - every subrecord the definition places in a member decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload, `XXXX`);
//! - CELL, REFR and ACHR are `unordered` records (M4.BX1): every subrecord their definition
//!   names is placed wherever it stands; a signature no member names is counted per
//!   signature as unplaced and kept raw.
//!
//! The full run also checks the cell and worldspace group layout (types 1 to 9) that
//! docs/schema/world-family.md describes, and prints the counts that page quotes.
//!
//! - `world_family_sample` runs in `cargo test`: every record of the seven types in
//!   SFBGS003.esm (REFR sampled one in 20, NAVM one in 10).
//! - `world_family_full` (ignored) runs all 14 files, every record; run it in release:
//!   `cargo test -p kitcraft-esm --release --test world_family -- --ignored --nocapture`

// Test-only code: a panic fails the test.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used
)]

mod common;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use kitcraft_esm::bridge::{Ctx, RawSub};
use kitcraft_esm::generated::records::{match_body, Hit, MemberDesc, Record as Typed, RECORDS};
use kitcraft_esm::index::{index_file, read_indexed, IndexOptions, PluginIndex};
use kitcraft_esm::tes4::FLAG_LOCALIZED;
use kitcraft_esm::DecodeLimits;

const TYPES: [&[u8; 4]; 7] = [
    b"WRLD", b"CELL", b"REFR", b"ACHR", b"NAVM", b"LCTN", b"LCRT",
];

#[derive(Default)]
struct TypeStats {
    records: u64,
    compressed: u64,
    subrecords: u64,
    records_with_unplaced: u64,
    /// Placed in a typed or opaque subrecord member.
    placed: BTreeMap<String, u64>,
    /// Placed inside an any-until body (a BFCB..BFCE block), kept raw there.
    in_raw_body: BTreeMap<String, u64>,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    reencode_mismatches: u64,
    failures: Vec<String>,
}

#[derive(Default)]
struct Stats {
    types: BTreeMap<String, TypeStats>,
    group_types: BTreeMap<i32, u64>,
    /// (parent group type, child group type or record signature) -> count.
    children: BTreeMap<(i32, String), u64>,
    layout: BTreeMap<&'static str, (u64, u64)>,
    /// The first few exceptions to each layout rule.
    exceptions: BTreeMap<&'static str, Vec<String>>,
    /// Child group types of each cell children group (type 6), in file order -> count.
    cell_children: BTreeMap<String, u64>,
    file: String,
}

impl Stats {
    fn agree(&mut self, what: &'static str, ok: bool, example: impl FnOnce() -> String) {
        let e = self.layout.entry(what).or_insert((0, 0));
        e.1 += 1;
        if ok {
            e.0 += 1;
        } else {
            let list = self.exceptions.entry(what).or_default();
            if list.len() < 5 {
                list.push(format!("{}: {}", self.file, example()));
            }
        }
    }
}

/// Layout rules every official file follows; the full run fails if one is broken.
const STRICT_RULES: [&str; 5] = [
    "type 1 follows its WRLD (label = FormID)",
    "type 6 follows its CELL (label = FormID)",
    "types 8/9 sit in the type 6 group with the same label",
    "exterior subblock label = (Y div 8, X div 8) of XCLC",
    "exterior block label = (Y div 32, X div 32) of XCLC",
];

fn label_u32(label: [u8; 4]) -> u32 {
    u32::from_le_bytes(label)
}

fn label_grid(label: [u8; 4]) -> (i16, i16) {
    // (Y, X) as stored: two little-endian i16.
    (
        i16::from_le_bytes([label[0], label[1]]),
        i16::from_le_bytes([label[2], label[3]]),
    )
}

/// The input positions the matcher put in an any-until body (raw by definition).
fn collect_raw_bodies(hits: &[Hit], members: &[MemberDesc], out: &mut Vec<usize>) {
    for hit in hits {
        match hit {
            Hit::Sub { member, index } => {
                if matches!(members.get(*member), Some(MemberDesc::AnyUntil { .. })) {
                    out.push(*index);
                }
            }
            Hit::Group { member, hits } => {
                if let Some(MemberDesc::Group { members: inner, .. }) = members.get(*member) {
                    collect_raw_bodies(hits, inner, out);
                }
            }
        }
    }
}

/// Check one record; returns the CELL grid (X, Y) when the record is a CELL with XCLC.
fn check_record(
    name: &str,
    sig: [u8; 4],
    form_id: u32,
    subs: &[RawSub<'_>],
    ctx: Ctx,
    t: &mut TypeStats,
) -> Option<(i32, i32)> {
    let typed = Typed::decode(sig, subs, ctx).expect("a definition exists");
    let members = RECORDS
        .iter()
        .find(|(s, _)| *s == sig)
        .map(|(_, m)| *m)
        .unwrap();
    let (hits, unmatched) = match_body(sig, subs, members);
    t.subrecords += subs.len() as u64;
    if !unmatched.is_empty() {
        t.records_with_unplaced += 1;
    }
    let mut raw_bodies = Vec::new();
    collect_raw_bodies(&hits, members, &mut raw_bodies);
    for (i, s) in subs.iter().enumerate() {
        let key = String::from_utf8_lossy(&s.signature).into_owned();
        if unmatched.contains(&i) {
            *t.unplaced.entry(key).or_insert(0) += 1;
        } else if raw_bodies.contains(&i) {
            *t.in_raw_body.entry(key).or_insert(0) += 1;
        } else {
            *t.placed.entry(key).or_insert(0) += 1;
        }
    }
    for (i, sub) in typed.extra() {
        if !unmatched.contains(i) {
            let key = String::from_utf8_lossy(&sub.signature).into_owned();
            *t.failed.entry(key.clone()).or_insert(0) += 1;
            if t.failures.len() < 20 {
                t.failures.push(format!(
                    "{name} {form_id:08X}: {key} at {i} ({} bytes) does not decode",
                    sub.data.len()
                ));
            }
        }
    }
    let encoded = typed.encode();
    let same = encoded.len() == subs.len()
        && encoded.iter().zip(subs).all(|(e, r)| {
            e.signature == r.signature && e.data == r.data && e.oversized == r.oversized
        });
    if !same {
        t.reencode_mismatches += 1;
        if t.failures.len() < 20 {
            t.failures
                .push(format!("{name} {form_id:08X}: re-encoding differs"));
        }
    }
    match &typed {
        Typed::Cell(c) => c.grid.as_ref().map(|g| (g.x, g.y)),
        _ => None,
    }
}

/// `sample(signature, n)` says whether the n-th record of that type in the file is checked.
fn check_file(path: &Path, sample: &dyn Fn(&[u8; 4], u64) -> bool, stats: &mut Stats) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    stats.file.clone_from(&name);
    let index = index_file(path, &IndexOptions::default()).unwrap();
    let localized = index.header.header.flags & FLAG_LOCALIZED != 0;
    let limits = DecodeLimits::default();
    let mut input = BufReader::new(File::open(path).unwrap());
    let mut seen: BTreeMap<[u8; 4], u64> = BTreeMap::new();
    let mut grids: BTreeMap<u32, (i32, i32)> = BTreeMap::new();
    for loc in &index.records {
        let sig = loc.signature.bytes();
        if !TYPES.contains(&&sig) {
            continue;
        }
        let n = seen.entry(sig).or_insert(0);
        *n += 1;
        if !sample(&sig, *n - 1) {
            continue;
        }
        let record = read_indexed(&mut input, loc, Some(&limits)).unwrap();
        let t = stats
            .types
            .entry(String::from_utf8_lossy(&sig).into_owned())
            .or_default();
        t.records += 1;
        if loc.is_compressed() {
            t.compressed += 1;
        }
        let subs: Vec<RawSub<'_>> = record
            .subrecords()
            .expect("decoded")
            .iter()
            .map(RawSub::from)
            .collect();
        let ctx = Ctx::for_record(&record, localized);
        if let Some(grid) = check_record(&name, sig, loc.form_id, &subs, ctx, t) {
            grids.insert(loc.form_id, grid);
        }
    }
    layout(&index, &grids, stats);
}

/// A record directly in a group: offset, FormID, signature.
type RecordAt = (u64, u32, [u8; 4]);

/// Group-layout facts: counts per group type, what each group type holds, and the label
/// rules docs/schema/world-family.md states.
fn layout(index: &PluginIndex, grids: &BTreeMap<u32, (i32, i32)>, stats: &mut Stats) {
    for g in &index.groups {
        *stats.group_types.entry(g.header.group_type).or_insert(0) += 1;
        if let Some(p) = g.parent.and_then(|p| index.groups.get(p as usize)) {
            *stats
                .children
                .entry((p.header.group_type, format!("GRUP {}", g.header.group_type)))
                .or_insert(0) += 1;
        }
    }
    // Records directly in each group, in file order, for "the record just before".
    let mut by_parent: BTreeMap<Option<u32>, Vec<RecordAt>> = BTreeMap::new();
    for r in index.records.iter().skip(1) {
        by_parent
            .entry(r.group)
            .or_default()
            .push((r.offset, r.form_id, r.signature.bytes()));
        if let Some(p) = r.group.and_then(|p| index.groups.get(p as usize)) {
            *stats
                .children
                .entry((
                    p.header.group_type,
                    String::from_utf8_lossy(&r.signature.bytes()).into_owned(),
                ))
                .or_insert(0) += 1;
        }
    }
    let mut kids: BTreeMap<u32, Vec<i32>> = BTreeMap::new();
    for g in &index.groups {
        if let Some(p) = g.parent {
            kids.entry(p).or_default().push(g.header.group_type);
        }
    }
    for (gi, g) in index.groups.iter().enumerate() {
        let gt = g.header.group_type;
        let label = label_u32(g.header.label);
        let parent = g.parent.and_then(|p| index.groups.get(p as usize));
        match gt {
            1 | 6 => {
                // The record just before the group, in the same parent, is the WRLD/CELL
                // whose FormID is the label.
                let want: &[u8; 4] = if gt == 1 { b"WRLD" } else { b"CELL" };
                let prev = by_parent
                    .get(&g.parent)
                    .and_then(|v| v.iter().rev().find(|(off, _, _)| *off < g.offset).copied());
                let ok = prev.is_some_and(|(_, id, s)| &s == want && id == label);
                let what = if gt == 1 {
                    "type 1 follows its WRLD (label = FormID)"
                } else {
                    "type 6 follows its CELL (label = FormID)"
                };
                stats.agree(what, ok, || {
                    format!("group at {} label {label:08X}", g.offset)
                });
            }
            8 | 9 => {
                let ok = parent
                    .is_some_and(|p| p.header.group_type == 6 && p.header.label == g.header.label);
                stats.agree(
                    "types 8/9 sit in the type 6 group with the same label",
                    ok,
                    || format!("group at {}", g.offset),
                );
            }
            _ => {}
        }
        if gt == 6 {
            let order: Vec<String> = kids
                .get(&(gi as u32))
                .map(|k| k.iter().map(i32::to_string).collect())
                .unwrap_or_default();
            *stats.cell_children.entry(order.join(" ")).or_insert(0) += 1;
        }
    }
    // Block labels against the cells they hold.
    for r in index.records.iter().skip(1) {
        if r.signature.bytes() != *b"CELL" {
            continue;
        }
        let Some(p) = r.group.and_then(|p| index.groups.get(p as usize)) else {
            continue;
        };
        match p.header.group_type {
            3 => {
                let block = index.groups.get(p.parent.unwrap() as usize).unwrap();
                let object = r.form_id & 0x00FF_FFFF;
                let (sub, blk) = (label_u32(p.header.label), label_u32(block.header.label));
                let example = || format!("CELL {:08X} in block {blk} subblock {sub}", r.form_id);
                stats.agree(
                    "interior subblock label = (FormID & 0xFFFFFF) / 10 % 10",
                    sub == (object / 10) % 10,
                    example,
                );
                stats.agree(
                    "interior block label = (FormID & 0xFFFFFF) % 10",
                    blk == object % 10,
                    example,
                );
            }
            5 => {
                let Some(&(x, y)) = grids.get(&r.form_id) else {
                    continue;
                };
                let (sy, sx) = label_grid(p.header.label);
                let block = index.groups.get(p.parent.unwrap() as usize).unwrap();
                let (by, bx) = label_grid(block.header.label);
                let example = || format!("CELL {:08X} at ({x}, {y})", r.form_id);
                stats.agree(
                    "exterior subblock label = (Y div 8, X div 8) of XCLC",
                    i32::from(sy) == y.div_euclid(8) && i32::from(sx) == x.div_euclid(8),
                    example,
                );
                stats.agree(
                    "exterior block label = (Y div 32, X div 32) of XCLC",
                    i32::from(by) == y.div_euclid(32) && i32::from(bx) == x.div_euclid(32),
                    example,
                );
            }
            _ => {}
        }
    }
}

fn report(stats: &Stats, full: bool) {
    println!(
        "type  records  compressed  subrecords  placed  in-raw-body  unplaced  failed  reencode-diff  records-with-unplaced"
    );
    for (ty, t) in &stats.types {
        let placed: u64 = t.placed.values().sum();
        let raw: u64 = t.in_raw_body.values().sum();
        let unplaced: u64 = t.unplaced.values().sum();
        let failed: u64 = t.failed.values().sum();
        println!(
            "{ty}  {}  {}  {}  {placed}  {raw}  {unplaced}  {failed}  {}  {}",
            t.records, t.compressed, t.subrecords, t.reencode_mismatches, t.records_with_unplaced
        );
    }
    if !full {
        return;
    }
    for (ty, t) in &stats.types {
        for (s, n) in &t.placed {
            println!("placed {ty} {s} {n}");
        }
        for (s, n) in &t.in_raw_body {
            println!("rawbody {ty} {s} {n}");
        }
        for (s, n) in &t.unplaced {
            println!("unplaced {ty} {s} {n}");
        }
    }
    for (gt, n) in &stats.group_types {
        println!("group-type {gt} {n}");
    }
    for ((gt, child), n) in &stats.children {
        println!("group-child {gt} {child} {n}");
    }
    for (what, (ok, all)) in &stats.layout {
        println!("layout {ok}/{all}: {what}");
        for e in stats.exceptions.get(what).into_iter().flatten() {
            println!("  exception {e}");
        }
    }
    for (order, n) in &stats.cell_children {
        println!("cell-children [{order}] {n}");
    }
}

fn assert_clean(stats: &Stats) {
    let schema = kitcraft_schema::builtin().unwrap();
    let mut problems = Vec::new();
    for (ty, t) in &stats.types {
        // CELL, REFR and ACHR are `unordered` (M4.BX1): a subrecord their definition names
        // is placed wherever it stands; only signatures it does not name stay unplaced.
        let sig = kitcraft_schema::Signature::parse(ty).unwrap();
        let known = schema.known_signatures(sig).unwrap();
        let out_of_place: Vec<&String> = t
            .unplaced
            .keys()
            .filter(|s| kitcraft_schema::Signature::parse(s).is_some_and(|s| known.contains(&s)))
            .collect();
        if schema.record(sig).is_some_and(|r| r.unordered) {
            assert!(
                out_of_place.is_empty(),
                "{ty}: named signatures left unplaced: {out_of_place:?}"
            );
        }
        problems.extend(t.failures.iter().cloned());
        assert!(
            t.failed.is_empty() && t.reencode_mismatches == 0,
            "{ty}: {} placed subrecords failed to decode, {} records re-encoded differently:\n{}",
            t.failed.values().sum::<u64>(),
            t.reencode_mismatches,
            problems.join("\n")
        );
    }
}

#[test]
fn world_family_sample() {
    let Some(files) = common::files("world_family_sample", &["SFBGS003.esm"]) else {
        return;
    };
    let mut stats = Stats::default();
    let sample = |sig: &[u8; 4], n: u64| match sig {
        b"REFR" => n.is_multiple_of(20),
        b"NAVM" => n.is_multiple_of(10),
        _ => true,
    };
    for f in &files {
        check_file(&f.path, &sample, &mut stats);
    }
    report(&stats, false);
    assert_clean(&stats);
    for ty in TYPES {
        let key = String::from_utf8_lossy(ty).into_owned();
        assert!(
            stats.types.get(&key).is_some_and(|t| t.records > 0),
            "no {key} record checked"
        );
    }
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn world_family_full() {
    let Some(files) = common::all_files("world_family_full") else {
        return;
    };
    let mut stats = Stats::default();
    for f in &files {
        check_file(&f.path, &|_, _| true, &mut stats);
    }
    report(&stats, true);
    assert_clean(&stats);
    for what in STRICT_RULES {
        let (ok, all) = stats.layout.get(what).copied().unwrap_or_default();
        assert!(
            all > 0 && ok == all,
            "layout rule broken: {what} ({ok}/{all})"
        );
    }
}
