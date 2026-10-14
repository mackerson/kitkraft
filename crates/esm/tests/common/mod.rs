//! Shared helpers for corpus tests.
//!
//! The official ESMs are read in place from `KITCRAFT_DATA_DIR` and never copied into the
//! repository (see `docs/corpus.md`). `corpus/manifest.toml` pins each file by size; a size
//! that disagrees with the manifest is reported as "corpus changed", not as a roundtrip
//! failure. When the variable is unset or names a missing directory, [`files`] prints one
//! skip line and returns `None` so the test passes on machines without the game.

// Test-only code: a panic here fails the test, it is not a crash in shipped code. Clippy's
// `allow-*-in-tests` covers `#[test]` functions but not helpers in an integration-test module.
#![allow(dead_code, clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

/// Environment variable naming the Starfield `Data` directory.
pub const DATA_DIR_VAR: &str = "KITCRAFT_DATA_DIR";

/// The three small files used by the fast tests.
pub const SMALL: [&str; 3] = ["OldMars.esm", "SFBGS007.esm", "Constellation.esm"];

/// One entry of `corpus/manifest.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

/// A corpus file that exists in the data directory with the size the manifest pins.
#[derive(Debug, Clone)]
pub struct CorpusFile {
    pub name: String,
    pub size: u64,
    pub path: PathBuf,
}

/// Path of `corpus/manifest.toml` in this repository.
pub fn manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("manifest.toml")
}

/// Parse `corpus/manifest.toml`. Panics (failing the test) when it is unreadable.
pub fn manifest() -> Vec<ManifestEntry> {
    let path = manifest_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let table: toml::Table = text
        .parse()
        .unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()));
    let files = table
        .get("file")
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("{}: no [[file]] entries", path.display()));
    files
        .iter()
        .map(|f| {
            let name = f.get("name").and_then(toml::Value::as_str);
            let size = f.get("size").and_then(toml::Value::as_integer);
            let sha = f.get("sha256").and_then(toml::Value::as_str);
            match (name, size, sha) {
                (Some(name), Some(size), Some(sha)) => ManifestEntry {
                    name: name.to_string(),
                    size: u64::try_from(size).unwrap(),
                    sha256: sha.to_string(),
                },
                _ => panic!("{}: malformed [[file]] entry {f:?}", path.display()),
            }
        })
        .collect()
}

/// The data directory, or `None` (after printing one skip line) when it is unavailable.
pub fn data_dir(test: &str) -> Option<PathBuf> {
    match std::env::var_os(DATA_DIR_VAR) {
        None => {
            println!("skip {test}: {DATA_DIR_VAR} is not set (see docs/corpus.md)");
            None
        }
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if dir.is_dir() {
                Some(dir)
            } else {
                println!(
                    "skip {test}: {DATA_DIR_VAR}={} is not a directory (see docs/corpus.md)",
                    dir.display()
                );
                None
            }
        }
    }
}

/// Resolve `names` against the manifest and the data directory.
///
/// Returns `None` (after one skip line) when the data directory is unavailable. Panics with
/// "corpus changed" when a file is missing or its size disagrees with the manifest, and when
/// a name is not in the manifest at all (a test bug).
pub fn files(test: &str, names: &[&str]) -> Option<Vec<CorpusFile>> {
    let dir = data_dir(test)?;
    let manifest = manifest();
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let entry = manifest
            .iter()
            .find(|e| e.name == *name)
            .unwrap_or_else(|| panic!("{name} is not listed in corpus/manifest.toml"));
        let path = dir.join(&entry.name);
        let size = match std::fs::metadata(&path) {
            Ok(meta) => meta.len(),
            Err(e) => panic!(
                "corpus changed: {} is missing ({e}); update corpus/manifest.toml",
                path.display()
            ),
        };
        assert!(
            size == entry.size,
            "corpus changed: {} is {size} bytes, manifest says {}; update corpus/manifest.toml",
            entry.name,
            entry.size
        );
        out.push(CorpusFile {
            name: entry.name.clone(),
            size,
            path,
        });
    }
    Some(out)
}

/// Every manifest file, resolved as in [`files`].
pub fn all_files(test: &str) -> Option<Vec<CorpusFile>> {
    let names: Vec<String> = manifest().into_iter().map(|e| e.name).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    files(test, &refs)
}
