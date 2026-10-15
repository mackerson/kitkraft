//! String tables of the official files through `kitkraft_doc::strings` (M3.D1). Reads
//! `KITKRAFT_DATA_DIR` in place; skips when it is unset.
//!
//! - Fast: every non-UTF-8 entry of the `starfield_en` tables resolves with its exact table
//!   bytes (`Resolved::bytes`), not only the lossy text.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use kitkraft_ba2::Archive;
use kitkraft_doc::strings::{DirSource, StringTables};
use kitkraft_strings::{table_name, Kind, Table};

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.is_dir().then_some(dir)
}

#[test]
fn non_utf8_official_strings_expose_their_bytes() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset or missing");
        return;
    };
    let archive_path = dir.join("Starfield - Localization.ba2");
    if !archive_path.is_file() {
        eprintln!("skip: {} is missing", archive_path.display());
        return;
    }
    let (archive, mut file) = Archive::open(&archive_path).unwrap();
    let mut tables = StringTables::new(
        "en",
        Box::new(DirSource {
            data_dir: dir.clone(),
        }),
    );
    let mut non_utf8 = 0usize;
    for kind in Kind::ALL {
        let rel = format!("strings/{}", table_name("Starfield.esm", "en", kind));
        let index = archive.find(&rel).unwrap();
        let bytes = archive.extract_vec(&mut file, index, 256 << 20).unwrap();
        let table = Table::parse(kind, &bytes).unwrap();
        for e in table.entries.iter().filter(|e| !e.is_utf8()) {
            // A repeated ID resolves to its first entry (lane G); skip later repeats.
            if table.get(e.id).map(|first| first.offset) != Some(e.offset) {
                continue;
            }
            let r = tables.lookup("Starfield.esm", e.id).unwrap();
            assert_eq!(r.bytes, e.bytes, "{rel} 0x{:08X}", e.id);
            assert!(!r.utf8);
            assert_eq!(r.offset, e.offset);
            non_utf8 += 1;
        }
    }
    eprintln!("starfield_en: {non_utf8} non-UTF-8 entries resolve with their exact bytes");
    assert!(
        non_utf8 > 0,
        "lane G reported non-UTF-8 entries in the English tables"
    );
}
