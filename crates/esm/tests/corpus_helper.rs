//! M0.5: the corpus helper reads the manifest and resolves files, or skips cleanly.

mod common;

#[test]
fn manifest_lists_the_fourteen_official_esms() {
    let manifest = common::manifest();
    assert_eq!(manifest.len(), 14);
    for entry in &manifest {
        assert!(entry.name.ends_with(".esm"), "{}", entry.name);
        assert!(entry.size > 0, "{}", entry.name);
        assert_eq!(entry.sha256.len(), 64, "{}", entry.name);
    }
    for small in common::SMALL {
        assert!(manifest.iter().any(|e| e.name == small), "{small}");
    }
}

#[test]
fn small_files_resolve_or_skip() {
    let Some(files) = common::files("corpus_helper", &common::SMALL) else {
        return;
    };
    assert_eq!(files.len(), 3);
    for file in files {
        assert!(file.path.is_file(), "{}", file.path.display());
        assert!(file.size < 64 * 1024, "{} is not small", file.name);
    }
}
