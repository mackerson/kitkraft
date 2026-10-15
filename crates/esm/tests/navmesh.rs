//! M4.A4 NAVM passthrough on the Creation Kit fixture F22 (`docs/ck-observations.md`): the
//! navmesh the CK generated is imported into a copy of the same plugin without navmesh, and
//! every imported record must equal the CK's byte for byte. The fixture lives in
//! `observe\<date>-F22\` (gitignored, never committed); without it the test prints one skip
//! line and passes. Synthetic cases are unit tests in `src/navmesh.rs`.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use kitkraft_esm::navmesh::import_navmeshes;
use kitkraft_esm::navmesh::NavmeshOptions;
use kitkraft_esm::{Entry, Plugin, Record, WriteOptions};

/// The F22 fixture folder, if one exists.
fn fixture_dir() -> Option<PathBuf> {
    let observe = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../observe");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&observe)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with("-F22"))
        })
        .collect();
    dirs.sort();
    dirs.pop()
}

fn navmeshes<'p, 'a>(entries: &'p [Entry<'a>], out: &mut Vec<&'p Record<'a>>) {
    for e in entries {
        match e {
            Entry::Group(g) => navmeshes(g.children(), out),
            Entry::Record(r) if r.header.signature.0 == *b"NAVM" => out.push(r),
            Entry::Record(_) => {}
        }
    }
}

fn strip(entries: &mut Vec<Entry<'_>>) {
    entries.retain(|e| !matches!(e, Entry::Record(r) if r.header.signature.0 == *b"NAVM"));
    for e in entries.iter_mut() {
        if let Entry::Group(g) = e {
            strip(g.children_mut());
        }
    }
}

fn bytes_of(record: &Record<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    Entry::Record(record.clone())
        .write_to(&mut out, 0, &WriteOptions::default())
        .unwrap();
    out
}

#[test]
fn ck_navmesh_imports_unchanged() {
    let Some(dir) = fixture_dir() else {
        println!("skip ck_navmesh_imports_unchanged: no observe\\<date>-F22 fixture (M4.O1)");
        return;
    };
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        if !matches!(ext.as_deref(), Some("esm" | "esp" | "esl")) {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let source = Plugin::parse(&bytes).unwrap();
        let mut expected = Vec::new();
        navmeshes(source.entries(), &mut expected);
        if expected.is_empty() {
            continue;
        }
        let mut target = Plugin::parse(&bytes).unwrap().into_owned();
        strip(target.entries_mut());
        let report = import_navmeshes(&source, &mut target, &NavmeshOptions::default()).unwrap();
        assert_eq!(report.inserted, expected.len(), "{}", path.display());
        let written = target.to_bytes().unwrap();
        let reparsed = Plugin::parse(&written).unwrap();
        let mut got = Vec::new();
        navmeshes(reparsed.entries(), &mut got);
        assert_eq!(got.len(), expected.len());
        for want in &expected {
            let found = got
                .iter()
                .find(|r| r.header.form_id == want.header.form_id)
                .unwrap_or_else(|| panic!("NAVM {:08X} missing", want.header.form_id));
            assert_eq!(
                bytes_of(found),
                bytes_of(want),
                "NAVM {:08X}",
                want.header.form_id
            );
        }
        checked += expected.len();
        println!(
            "{}: {} NAVM imported unchanged",
            path.display(),
            expected.len()
        );
    }
    println!("F22 ({}): {checked} NAVM checked", dir.display());
}
