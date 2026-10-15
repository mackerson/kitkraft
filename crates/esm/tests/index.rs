//! M2.A1 record index and M2.A2 random access on the official ESMs (docs/corpus.md).
//!
//! - `corpus_small_index` (fast): the five small files index with the same record counts,
//!   per signature, as the streaming parse; 1,000 seeded index entries per file read back
//!   by offset equal the streaming parse's records.
//! - `corpus_index_all_counts` (ignored): the same counts on all 14 files.
//! - `corpus_index_starfield` (ignored): indexes `Starfield.esm`, prints wall time and the
//!   peak working set, then checks 1,000 seeded entries against the streaming parse.
//!
//! `cargo test -p kitkraft-esm --release --test index -- --ignored --nocapture`

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Instant;

use common::hostile::Rng;
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions, PluginIndex};
use kitkraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature};

/// Record count per signature, from the streaming parse (`TES4` included).
fn parsed_counts(path: &Path) -> BTreeMap<Signature, u64> {
    fn walk(entry: &Entry<'_>, out: &mut BTreeMap<Signature, u64>) {
        match entry {
            Entry::Group(g) => g.children().iter().for_each(|c| walk(c, out)),
            Entry::Record(r) => *out.entry(r.header.signature).or_insert(0) += 1,
        }
    }
    let mut out = BTreeMap::new();
    visit_file(path, &ParseOptions::default(), |e| {
        walk(e, &mut out);
        Ok(())
    })
    .unwrap();
    out
}

fn index_counts(index: &PluginIndex) -> BTreeMap<Signature, u64> {
    let mut out = BTreeMap::new();
    for loc in &index.records {
        *out.entry(loc.signature).or_insert(0) += 1;
    }
    out
}

/// Pick `n` distinct seeded record positions (all of them when there are fewer).
fn sample(index: &PluginIndex, n: usize, seed: u64) -> BTreeSet<usize> {
    let total = index.records.len();
    if total <= n {
        return (0..total).collect();
    }
    let mut rng = Rng::new(seed);
    let mut picked = BTreeSet::new();
    while picked.len() < n {
        picked.insert((rng.next() % total as u64) as usize);
    }
    picked
}

/// Read the sampled entries by offset (decompressing compressed ones) and compare them
/// with the records the streaming parse produces at the same offsets.
fn check_random_access(path: &Path, index: &PluginIndex, picked: &BTreeSet<usize>) {
    let limits = DecodeLimits::default();
    let mut file = BufReader::new(File::open(path).unwrap());
    let mut read: HashMap<u64, Record<'static>> = HashMap::new();
    for &i in picked {
        let loc = &index.records[i];
        let record = read_indexed(&mut file, loc, Some(&limits)).unwrap();
        read.insert(loc.offset, record);
    }
    fn walk(
        entry: &Entry<'_>,
        read: &HashMap<u64, Record<'static>>,
        seen: &mut usize,
        limits: &DecodeLimits,
    ) {
        match entry {
            Entry::Group(g) => g
                .children()
                .iter()
                .for_each(|c| walk(c, read, seen, limits)),
            Entry::Record(r) => {
                let Some(got) = read.get(&r.offset().unwrap()) else {
                    return;
                };
                let mut expected = r.clone().into_owned();
                expected.decompress(limits).unwrap();
                assert_eq!(got, &expected, "record at {:?}", r.offset());
                *seen += 1;
            }
        }
    }
    let mut seen = 0;
    visit_file(path, &ParseOptions::default(), |e| {
        walk(e, &read, &mut seen, &limits);
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, picked.len());
}

#[test]
fn corpus_small_index() {
    let Some(files) = common::files("corpus_small_index", &common::SMALL) else {
        return;
    };
    for file in files {
        let index = index_file(&file.path, &IndexOptions::default()).unwrap();
        assert_eq!(index.file_len, file.size);
        assert_eq!(
            index_counts(&index),
            parsed_counts(&file.path),
            "{}",
            file.name
        );
        let picked = sample(&index, 1000, 0xA2 ^ file.size);
        check_random_access(&file.path, &index, &picked);
        let edids = index.records.iter().filter(|l| l.edid.is_some()).count();
        println!(
            "{}: {} records, {} groups, {edids} EditorIDs, {} read back",
            file.name,
            index.records.len(),
            index.groups.len(),
            picked.len()
        );
    }
}

#[test]
#[ignore = "indexes 2.5 GB of game data; run with --release -- --ignored"]
fn corpus_index_all_counts() {
    let Some(files) = common::all_files("corpus_index_all_counts") else {
        return;
    };
    println!("| File | Records | Groups | EditorIDs | Index time (s) |");
    println!("| --- | --- | --- | --- | --- |");
    for file in files {
        let start = Instant::now();
        let index = index_file(&file.path, &IndexOptions::default()).unwrap();
        let secs = start.elapsed().as_secs_f64();
        assert_eq!(
            index_counts(&index),
            parsed_counts(&file.path),
            "{}",
            file.name
        );
        println!(
            "| {} | {} | {} | {} | {secs:.2} |",
            file.name,
            index.records.len(),
            index.groups.len(),
            index.records.iter().filter(|l| l.edid.is_some()).count(),
        );
    }
}

#[test]
#[ignore = "indexes Starfield.esm (1.46 GB); run with --release -- --ignored"]
fn corpus_index_starfield() {
    let Some(files) = common::files("corpus_index_starfield", &["Starfield.esm"]) else {
        return;
    };
    let file = &files[0];
    let start = Instant::now();
    let index = index_file(&file.path, &IndexOptions::default()).unwrap();
    let secs = start.elapsed().as_secs_f64();
    let peak = common::peak_working_set();
    println!(
        "Starfield.esm: {} records, {} groups, {} EditorIDs, indexed in {secs:.2} s, \
         peak working set {}",
        index.records.len(),
        index.groups.len(),
        index.records.iter().filter(|l| l.edid.is_some()).count(),
        peak.map_or("unknown".to_string(), |p| format!(
            "{p} bytes ({:.0} MiB)",
            p as f64 / 1048576.0
        )),
    );
    assert_eq!(index.records.len(), 3_829_247);
    // M1.5 bound: 2x the largest top-level group plus 256 MiB (docs/corpus.md).
    if let Some(peak) = peak {
        assert!(peak < 1_903 * 1024 * 1024, "{peak}");
    }
    let start = Instant::now();
    let picked = sample(&index, 1000, 0xA2);
    check_random_access(&file.path, &index, &picked);
    println!(
        "1,000 seeded records read back and compared in {:.1} s",
        start.elapsed().as_secs_f64()
    );
}
