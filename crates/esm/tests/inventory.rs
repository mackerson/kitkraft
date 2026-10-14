//! Corpus inventory facts recorded in docs/corpus.md (M1.2b and the container facts the
//! parser relies on). Prints tables; asserts nothing beyond parsing.
//!
//! `cargo test -p kitcraft-esm --release --test inventory -- --ignored --nocapture`

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::collections::BTreeMap;

use kitcraft_esm::driver::visit_file;
use kitcraft_esm::{Entry, ParseOptions};

#[derive(Default)]
struct Inventory {
    form_versions: BTreeMap<u16, u64>,
    group_types: BTreeMap<i32, u64>,
    max_depth: usize,
    xxxx_targets: u64,
    xxxx_nonzero_size_field: u64,
    header_unknown_nonzero: u64,
    group_unknown_values: BTreeMap<u32, u64>,
}

fn walk(entry: &Entry<'_>, depth: usize, inv: &mut Inventory) {
    match entry {
        Entry::Group(g) => {
            inv.max_depth = inv.max_depth.max(depth);
            *inv.group_types.entry(g.header.group_type).or_insert(0) += 1;
            *inv.group_unknown_values
                .entry(g.header.unknown)
                .or_insert(0) += 1;
            g.children().iter().for_each(|c| walk(c, depth + 1, inv));
        }
        Entry::Record(r) => {
            *inv.form_versions.entry(r.header.form_version).or_insert(0) += 1;
            if r.header.unknown != 0 {
                inv.header_unknown_nonzero += 1;
            }
            for s in r.subrecords().unwrap_or(&[]) {
                if s.has_oversize_prefix() {
                    inv.xxxx_targets += 1;
                    if s.size_field_as_read() != Some(0) {
                        inv.xxxx_nonzero_size_field += 1;
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "inventory over 2.5 GB of game data; run with --release -- --ignored"]
fn corpus_inventory() {
    let Some(files) = common::all_files("corpus_inventory") else {
        return;
    };
    println!("| File | Record form versions (count) | Group types (count) | Max group depth | XXXX (non-zero target size field) | Records with non-zero header `unknown` | Distinct group `unknown` values |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for file in files {
        let mut inv = Inventory::default();
        visit_file(&file.path, &ParseOptions::default(), |entry| {
            walk(entry, 1, &mut inv);
            Ok(())
        })
        .unwrap();
        let fv: Vec<String> = inv
            .form_versions
            .iter()
            .map(|(v, n)| format!("{v} ({n})"))
            .collect();
        let gt: Vec<String> = inv
            .group_types
            .iter()
            .map(|(t, n)| format!("{t} ({n})"))
            .collect();
        println!(
            "| {} | {} | {} | {} | {} ({}) | {} | {} |",
            file.name,
            fv.join(", "),
            gt.join(", "),
            inv.max_depth,
            inv.xxxx_targets,
            inv.xxxx_nonzero_size_field,
            inv.header_unknown_nonzero,
            inv.group_unknown_values.len(),
        );
    }
}
