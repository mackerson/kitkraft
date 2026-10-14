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
use kitcraft_esm::{DecodeLimits, Entry, ParseOptions};

#[derive(Default)]
struct Inventory {
    form_versions: BTreeMap<u16, u64>,
    group_types: BTreeMap<i32, u64>,
    max_depth: usize,
    xxxx_targets: u64,
    xxxx_nonzero_size_field: u64,
    header_unknown_nonzero: u64,
    group_unknown_values: BTreeMap<u32, u64>,
    xxxx: Xxxx,
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
            // Every XXXX, including those inside compressed records.
            let decoded = r.decode(&DecodeLimits::default()).unwrap();
            let in_compressed = decoded.is_some();
            for s in decoded.as_deref().or(r.subrecords()).unwrap_or(&[]) {
                if !s.has_oversize_prefix() {
                    continue;
                }
                let x = &mut inv.xxxx;
                x.total += 1;
                if in_compressed {
                    x.in_compressed += 1;
                }
                let len = s.data().len() as u64;
                if len <= 0xFFFF {
                    x.small += 1;
                    *x.small_by_signature
                        .entry(s.signature().to_string())
                        .or_insert(0) += 1;
                }
                x.min_payload = Some(x.min_payload.map_or(len, |m| m.min(len)));
            }
        }
    }
}

#[derive(Default)]
struct Xxxx {
    total: u64,
    in_compressed: u64,
    /// Payload of 0xFFFF bytes or less: the prefix was not needed for the size.
    small: u64,
    small_by_signature: BTreeMap<String, u64>,
    min_payload: Option<u64>,
}

#[test]
#[ignore = "inventory over 2.5 GB of game data; run with --release -- --ignored"]
fn corpus_inventory() {
    let Some(files) = common::all_files("corpus_inventory") else {
        return;
    };
    println!("| File | Record form versions (count) | Group types (count) | Max group depth | XXXX (non-zero target size field) | Records with non-zero header `unknown` | Distinct group `unknown` values |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    let mut xxxx_rows = Vec::new();
    let mut total = Xxxx::default();
    for file in files {
        let mut inv = Inventory::default();
        visit_file(&file.path, &ParseOptions::default(), |entry| {
            walk(entry, 1, &mut inv);
            Ok(())
        })
        .unwrap();
        let x = &inv.xxxx;
        let by_sig: Vec<String> = x
            .small_by_signature
            .iter()
            .map(|(s, n)| format!("{s} ({n})"))
            .collect();
        xxxx_rows.push(format!(
            "| {} | {} | {} | {} | {} | {} |",
            file.name,
            x.total,
            x.in_compressed,
            x.small,
            x.min_payload.map_or("-".to_string(), |m| m.to_string()),
            by_sig.join(", ")
        ));
        total.total += x.total;
        total.in_compressed += x.in_compressed;
        total.small += x.small;
        for (s, n) in &x.small_by_signature {
            *total.small_by_signature.entry(s.clone()).or_insert(0) += n;
        }
        if let Some(m) = x.min_payload {
            total.min_payload = Some(total.min_payload.map_or(m, |t| t.min(m)));
        }
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
    println!();
    println!("| File | XXXX | In compressed records | Payload <= 0xFFFF | Smallest payload | Payload <= 0xFFFF by signature |");
    println!("| --- | --- | --- | --- | --- | --- |");
    for row in xxxx_rows {
        println!("{row}");
    }
    let by_sig: Vec<String> = total
        .small_by_signature
        .iter()
        .map(|(s, n)| format!("{s} ({n})"))
        .collect();
    println!(
        "| total | {} | {} | {} | {} | {} |",
        total.total,
        total.in_compressed,
        total.small,
        total.min_payload.map_or("-".to_string(), |m| m.to_string()),
        by_sig.join(", ")
    );
}
