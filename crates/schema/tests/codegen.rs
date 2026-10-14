//! Codegen golden tests (M1.10).
//!
//! `golden/seed.rs` is the output for the checked-in definitions and `golden/fixture.rs`
//! the output for the synthetic all-features fixture. To accept a deliberate change, run
//! with `KITCRAFT_UPDATE_GOLDEN=1` and review the diff. `tests/generated.rs` compiles both
//! files under the workspace lints and roundtrips data through them.

// Helpers outside `#[test]` fns panic/unwrap to fail the calling test; this is test code.
#![allow(clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use kitcraft_schema::{builtin, generate, load_sources, CodegenError};

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name)
}

fn check_golden(name: &str, generated: &str) {
    let path = golden_path(name);
    if std::env::var_os("KITCRAFT_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, generated).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; run with KITCRAFT_UPDATE_GOLDEN=1", path.display()));
    let expected = expected.replace("\r\n", "\n");
    assert!(
        expected == generated,
        "{name} is stale; rerun with KITCRAFT_UPDATE_GOLDEN=1 and review the diff"
    );
}

fn fixture_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ZZZZ.toml");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn seed_set_matches_golden() {
    let generated = generate(&builtin().unwrap()).unwrap();
    check_golden("seed.rs", &generated);
}

#[test]
fn fixture_matches_golden() {
    let text = fixture_source();
    let schema = load_sources([("ZZZZ.toml", text.as_str())]).unwrap();
    let generated = generate(&schema).unwrap();
    check_golden("fixture.rs", &generated);
}

#[test]
fn generation_is_deterministic() {
    let schema = builtin().unwrap();
    assert_eq!(generate(&schema).unwrap(), generate(&schema).unwrap());
}

#[test]
fn colliding_type_names_are_errors() {
    // A struct member named `Record` would produce `TestRecord`, the record type itself.
    let text = "signature = \"TEST\"\nname = \"Test\"\nsource = \"corpus: t\"\n\
                [[member]]\nsig = \"DATA\"\nname = \"Record\"\ntype = \"struct\"\n\
                field = [{ name = \"A\", type = \"u8\" }]\n";
    let schema = load_sources([("TEST.toml", text)]).unwrap();
    assert_eq!(
        generate(&schema),
        Err(CodegenError::DuplicateType("TestRecord".into()))
    );
}

/// The generated files must already be rustfmt-clean, because xtask checks them in under
/// `cargo fmt --check`. Skips when rustfmt is not installed.
#[test]
fn golden_files_are_rustfmt_clean() {
    let config = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rustfmt.toml");
    for name in ["seed.rs", "fixture.rs"] {
        let status = Command::new("rustfmt")
            .arg("--check")
            .arg("--edition")
            .arg("2021")
            .arg("--config-path")
            .arg(&config)
            .arg(golden_path(name))
            .status();
        match status {
            Ok(status) => assert!(status.success(), "{name} is not rustfmt-clean"),
            Err(e) => {
                eprintln!("skipping rustfmt check: {e}");
                return;
            }
        }
    }
}
