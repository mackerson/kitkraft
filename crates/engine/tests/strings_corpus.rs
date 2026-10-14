//! String tables in the official archives, read through `kitcraft-ba2` and rewritten by
//! `kitcraft-strings` (M2.G8). Lives here because the engine is the lowest crate that
//! depends on both. Skips without `KITCRAFT_DATA_DIR`.
//!
//! - `strings_en_rewrite_identical` (fast): the three `starfield_en` tables.
//! - `strings_all_rewrite_identical` (`#[ignore]`): every table in every official archive.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use kitcraft_ba2::Archive;
use kitcraft_strings::{Kind, Table};

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("KITCRAFT_DATA_DIR")?);
    dir.is_dir().then_some(dir)
}

/// Rewrites every table in `archive` whose name passes `keep`; returns how many.
fn check_archive(path: &std::path::Path, keep: impl Fn(&str) -> bool) -> usize {
    let (archive, mut file) = Archive::open(path).unwrap();
    let mut checked = 0;
    for (i, entry) in archive.entries.iter().enumerate() {
        let Some(kind) = entry
            .name
            .rsplit_once('.')
            .and_then(|(_, ext)| Kind::from_extension(ext))
        else {
            continue;
        };
        if !keep(&entry.name) {
            continue;
        }
        let bytes = archive.extract_vec(&mut file, i, 256 << 20).unwrap();
        let table = Table::parse(kind, &bytes)
            .unwrap_or_else(|e| panic!("{}: {}: {e}", path.display(), entry.name));
        assert!(
            table.duplicate_ids().is_empty(),
            "{}: duplicate IDs",
            entry.name
        );
        let ids: Vec<u32> = table.entries.iter().map(|e| e.id).collect();
        assert!(
            ids.windows(2).all(|w| w[0] < w[1]),
            "{}: IDs not sorted",
            entry.name
        );
        let again = table.to_bytes().unwrap();
        assert!(
            again == bytes,
            "{}: {} does not rewrite byte-identically",
            path.display(),
            entry.name
        );
        checked += 1;
    }
    checked
}

#[test]
fn strings_en_rewrite_identical() {
    let Some(dir) = data_dir() else {
        println!("skipped: KITCRAFT_DATA_DIR is not set or missing");
        return;
    };
    let path = dir.join("Starfield - Localization.ba2");
    if !path.is_file() {
        println!("skipped: {} is missing", path.display());
        return;
    }
    let n = check_archive(&path, |name| name.contains("starfield_en."));
    assert_eq!(n, 3);
}

#[test]
#[ignore = "reads every string table in every official archive; run in release"]
fn strings_all_rewrite_identical() {
    let Some(dir) = data_dir() else {
        println!("skipped: KITCRAFT_DATA_DIR is not set or missing");
        return;
    };
    let mut total = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ba2"))
        {
            continue;
        }
        let (archive, _) = Archive::open(&path).unwrap();
        if archive.header.kind != kitcraft_ba2::Kind::General {
            continue;
        }
        let n = check_archive(&path, |_| true);
        if n > 0 {
            println!("{}: {n} tables identical", path.display());
        }
        total += n;
    }
    println!("{total} tables rewrite byte-identically");
    assert!(total > 0);
}
