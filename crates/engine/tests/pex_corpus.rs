//! Every `.pex` in the official archives, read through `kitkraft-ba2` and disassembled by
//! `kitkraft-pex` (M2.G5). Lives here because the engine is the lowest crate that depends
//! on both. Skips without `KITKRAFT_DATA_DIR`.
//!
//! - `pex_misc_parse` (fast): every script in `Starfield - Misc.ba2` parses to its end.
//! - `pex_all_against_pexdis` (`#[ignore]`): every script in every archive parses; with
//!   `KITKRAFT_PEXDIS_OUT` naming a folder of `pexdis.py` outputs (`<archive with spaces as
//!   _>\<path>.txt`, as written by running `pexdis.py` on an extraction of the same
//!   archives), the text must equal `pexdis.py`'s for every script it has output for.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]

use std::path::{Path, PathBuf};

use kitkraft_ba2::Archive;
use kitkraft_pex::{disassemble, Pex};

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.is_dir().then_some(dir)
}

struct Counts {
    parsed: usize,
    with_structs: usize,
    compared: usize,
    differing: Vec<String>,
}

fn check_archive(path: &Path, pexdis: Option<&Path>, counts: &mut Counts) {
    let (archive, mut file) = Archive::open(path).unwrap();
    let label = path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .replace(' ', "_");
    for (i, entry) in archive.entries.iter().enumerate() {
        if !entry.name.to_ascii_lowercase().ends_with(".pex") {
            continue;
        }
        let bytes = archive.extract_vec(&mut file, i, 64 << 20).unwrap();
        let pex = Pex::parse(&bytes).unwrap_or_else(|e| panic!("{label}: {}: {e}", entry.name));
        counts.parsed += 1;
        if pex.objects.iter().any(|o| !o.structs.is_empty()) {
            counts.with_structs += 1;
        }
        let text = disassemble(&pex);
        if let Some(dir) = pexdis {
            let rel = kitkraft_ba2::relative_path(&entry.name).unwrap();
            let mut expected = dir.join(&label).join(rel).into_os_string();
            expected.push(".txt");
            if let Ok(want) = std::fs::read_to_string(&expected) {
                counts.compared += 1;
                if want != text {
                    counts.differing.push(format!("{label}: {}", entry.name));
                }
            }
        }
    }
}

#[test]
fn pex_misc_parse() {
    let Some(dir) = data_dir() else {
        println!("skipped: KITKRAFT_DATA_DIR is not set or missing");
        return;
    };
    let path = dir.join("Starfield - Misc.ba2");
    if !path.is_file() {
        println!("skipped: {} is missing", path.display());
        return;
    }
    let mut counts = Counts {
        parsed: 0,
        with_structs: 0,
        compared: 0,
        differing: Vec::new(),
    };
    check_archive(&path, None, &mut counts);
    assert!(counts.parsed > 4000, "{}", counts.parsed);
}

#[test]
#[ignore = "every script in every official archive; run in release"]
fn pex_all_against_pexdis() {
    let Some(dir) = data_dir() else {
        println!("skipped: KITKRAFT_DATA_DIR is not set or missing");
        return;
    };
    let pexdis = std::env::var_os("KITKRAFT_PEXDIS_OUT")
        .or_else(|| std::env::var_os("KITCRAFT_PEXDIS_OUT"))
        .map(PathBuf::from);
    let mut counts = Counts {
        parsed: 0,
        with_structs: 0,
        compared: 0,
        differing: Vec::new(),
    };
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ba2")))
        .collect();
    paths.sort();
    for path in paths {
        let (archive, _) = Archive::open(&path).unwrap();
        if archive.header.kind != kitkraft_ba2::Kind::General {
            continue;
        }
        check_archive(&path, pexdis.as_deref(), &mut counts);
    }
    println!(
        "parsed {} scripts ({} with structs); compared {} with pexdis.py, {} differ",
        counts.parsed,
        counts.with_structs,
        counts.compared,
        counts.differing.len()
    );
    for d in counts.differing.iter().take(20) {
        println!("  differs: {d}");
    }
    assert!(counts.differing.is_empty());
}
