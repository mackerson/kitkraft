//! The corpus inventory of some record types, as TOML on stdout (`kitkraft_esm::inventory`).
//!
//! `cargo run --release -p kitkraft-esm --example inventory_scan -- --types CURV,REGN
//! <plugin>...` scans every record of those types in the plugins, in the order given, and
//! prints one `[type.<SIG>]` table per type: `records`, `compressed`, `subrecords`,
//! `in_blocks`, `orders` (each `{ order, records }`) and `[type.<SIG>.sig.<SUB>]` with the
//! per-signature counts and sizes. `cargo xtask inventory-def` and `inventory-check` run
//! it on the 14 official ESMs (the same scan `tests/inventory_types.rs` prints); it is a
//! development tool, not part of the `kit` command catalog.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use kitkraft_esm::inventory::{key, scan_file, TypeStats};

fn int(n: u64) -> toml::Value {
    toml::Value::Integer(i64::try_from(n).unwrap_or(i64::MAX))
}

fn size(n: usize) -> toml::Value {
    toml::Value::Integer(i64::try_from(n).unwrap_or(i64::MAX))
}

fn render(stats: &BTreeMap<String, TypeStats>) -> Result<String, String> {
    let mut types = toml::Table::new();
    for (ty, t) in stats {
        let mut table = toml::Table::new();
        table.insert("records".into(), int(t.records));
        table.insert("compressed".into(), int(t.compressed));
        table.insert("subrecords".into(), int(t.subrecords));
        table.insert(
            "in_blocks".into(),
            toml::Value::Array(
                t.in_blocks
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect(),
            ),
        );
        let mut orders = Vec::new();
        for (order, n) in &t.orders {
            let mut o = toml::Table::new();
            o.insert("order".into(), toml::Value::String(order.clone()));
            o.insert("records".into(), int(*n));
            orders.push(toml::Value::Table(o));
        }
        table.insert("orders".into(), toml::Value::Array(orders));
        let mut sigs = toml::Table::new();
        for (sig, s) in &t.sigs {
            let mut e = toml::Table::new();
            e.insert("records".into(), int(s.records));
            e.insert("occurrences".into(), int(s.occurrences));
            e.insert("max_per_record".into(), int(s.max_per_record));
            e.insert("max_runs".into(), int(s.max_runs));
            e.insert("min_size".into(), size(s.min_size));
            e.insert("max_size".into(), size(s.max_size));
            sigs.insert(sig.clone(), toml::Value::Table(e));
        }
        table.insert("sig".into(), toml::Value::Table(sigs));
        types.insert(ty.clone(), toml::Value::Table(table));
    }
    let mut root = toml::Table::new();
    root.insert("type".into(), toml::Value::Table(types));
    toml::to_string(&root).map_err(|e| format!("rendering TOML: {e}"))
}

fn run() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let mut wanted: Vec<[u8; 4]> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--types" {
            let list = args.next().ok_or("--types needs a value")?;
            for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                let sig: [u8; 4] = name
                    .as_bytes()
                    .try_into()
                    .map_err(|_| format!("`{name}` is not a 4-character signature"))?;
                wanted.push(sig);
            }
        } else {
            files.push(PathBuf::from(arg));
        }
    }
    if wanted.is_empty() || files.is_empty() {
        return Err("usage: inventory_scan --types SIG[,SIG...] <plugin>...".into());
    }
    let mut stats: BTreeMap<String, TypeStats> = wanted
        .iter()
        .map(|s| (key(*s), TypeStats::default()))
        .collect();
    for path in &files {
        scan_file(
            path,
            |sig| wanted.contains(&sig.bytes()),
            |v| {
                stats
                    .entry(key(v.loc.signature.bytes()))
                    .or_default()
                    .add(v.subs, v.loc.is_compressed());
            },
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    render(&stats)
}

fn main() -> ExitCode {
    match run() {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("inventory_scan: {e}");
            ExitCode::FAILURE
        }
    }
}
