//! Codegen golden tests (M1.10).
//!
//! `golden/seed.rs` is the output for the checked-in definitions and `golden/fixture.rs`
//! the output for the synthetic fixtures in `tests/fixtures/` (every format feature). To
//! accept a deliberate change, run with `KITCRAFT_UPDATE_GOLDEN=1` and review the diff.
//! `tests/generated.rs` exercises both files; `examples/generated.rs` compiles them as a
//! non-test target, where the workspace lints apply without the test allowances.

// Helpers outside `#[test]` fns panic/unwrap to fail the calling test; this is test code.
#![allow(clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use kitcraft_schema::{builtin, generate, load_dir, load_sources, CodegenError};

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

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn seed_set_matches_golden() {
    let generated = generate(builtin().unwrap()).unwrap();
    check_golden("seed.rs", &generated);
}

#[test]
fn fixture_matches_golden() {
    let schema = load_dir(&fixtures_dir()).unwrap();
    let generated = generate(&schema).unwrap();
    check_golden("fixture.rs", &generated);
}

#[test]
fn generation_is_deterministic() {
    let schema = builtin().unwrap();
    assert_eq!(generate(schema).unwrap(), generate(schema).unwrap());
}

#[test]
fn shared_definitions_are_emitted_once() {
    let generated = generate(builtin().unwrap()).unwrap();
    assert_eq!(generated.matches("pub struct BfcbBlock {").count(), 1);
    assert_eq!(generated.matches("pub struct ObteBlock {").count(), 1);
    let fixture = generate(&load_dir(&fixtures_dir()).unwrap()).unwrap();
    assert_eq!(fixture.matches("pub struct Point {").count(), 1);
    assert_eq!(fixture.matches("pub struct Marker {").count(), 1);
}

#[test]
fn colliding_type_names_are_errors() {
    // A struct member named `Record` would produce `TestRecord`, the record type itself.
    let text =
        "signature = \"TEST\"\nname = \"Test\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
                [[member]]\nsig = \"DATA\"\nname = \"Record\"\ntype = \"struct\"\n\
                field = [{ name = \"A\", type = \"u8\" }]\n";
    let schema = load_sources([("TEST.toml", text)]).unwrap();
    assert_eq!(
        generate(&schema),
        Err(CodegenError::DuplicateType("TestRecord".into()))
    );
}

#[test]
fn prelude_names_are_reserved() {
    let text = "name = \"Sub\"\nkind = \"type\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\ntype = \"struct\"\n\
                field = [{ name = \"A\", type = \"u8\" }]\n";
    let schema = load_sources([("common/Sub.toml", text)]).unwrap();
    assert_eq!(generate(&schema), Err(CodegenError::Reserved("Sub".into())));
}

#[test]
fn schema_without_records_has_no_dispatch() {
    let text = "signature = \"HDR0\"\nname = \"Hdr\"\nkind = \"header\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
                size = 4\nfield = [{ name = \"Size\", type = \"u32\" }]\n";
    let schema = load_sources([("HDR0.toml", text)]).unwrap();
    let generated = generate(&schema).unwrap();
    assert!(!generated.contains("pub enum Record"));
    assert!(generated.contains("pub struct Hdr {"));
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
