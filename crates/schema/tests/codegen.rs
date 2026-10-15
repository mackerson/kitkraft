//! Codegen golden tests (M1.10).
//!
//! `golden/seed.rs` is the output for the checked-in definitions and `golden/fixture.rs`
//! the output for the synthetic fixtures in `tests/fixtures/` (every format feature). To
//! accept a deliberate change, run with `KITKRAFT_UPDATE_GOLDEN=1` and review the diff.
//! `tests/generated.rs` exercises both files; `examples/generated.rs` compiles them as a
//! non-test target, where the workspace lints apply without the test allowances.

// Helpers outside `#[test]` fns panic/unwrap to fail the calling test; this is test code.
#![allow(clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use kitkraft_schema::{builtin, generate, load_dir, load_sources, CodegenError};

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name)
}

fn rustfmt_config() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rustfmt.toml")
}

/// `source` formatted by rustfmt with the workspace `rustfmt.toml`, the way `cargo xtask
/// codegen` formats what it writes: the generator leaves line breaking to rustfmt, so the
/// goldens hold the formatted output. rustfmt is a required toolchain component
/// (`rust-toolchain.toml`); `RUSTFMT` overrides the binary as in xtask.
fn rustfmt(source: &str) -> String {
    use std::io::Write as _;
    use std::process::Stdio;
    let binary = std::env::var("RUSTFMT").unwrap_or_else(|_| "rustfmt".to_string());
    let mut child = Command::new(&binary)
        .args(["--edition", "2021", "--emit", "stdout", "--config-path"])
        .arg(rustfmt_config())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            panic!("could not start `{binary}` (rustup component add rustfmt): {e}")
        });
    let mut stdin = child.stdin.take().unwrap();
    let input = source.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap().unwrap();
    assert!(
        output.status.success(),
        "rustfmt failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Compares the rustfmt-formatted `generated` with the golden (or rewrites the golden).
fn check_golden(name: &str, generated: &str) {
    let generated = rustfmt(generated);
    let generated = generated.as_str();
    let path = golden_path(name);
    if std::env::var_os("KITKRAFT_UPDATE_GOLDEN")
        .or_else(|| std::env::var_os("KITCRAFT_UPDATE_GOLDEN"))
        .is_some()
    {
        std::fs::write(&path, generated).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; run with KITKRAFT_UPDATE_GOLDEN=1", path.display()));
    let expected = expected.replace("\r\n", "\n");
    assert!(
        expected == generated,
        "{name} is stale; rerun with KITKRAFT_UPDATE_GOLDEN=1 and review the diff"
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

/// The committed goldens are rustfmt output, and formatting them again changes nothing
/// (rustfmt is idempotent on what `cargo xtask codegen` writes, so `cargo fmt --check`
/// accepts the generated module).
#[test]
fn golden_files_are_rustfmt_clean() {
    for name in ["seed.rs", "fixture.rs"] {
        let golden = std::fs::read_to_string(golden_path(name))
            .unwrap()
            .replace("\r\n", "\n");
        assert!(rustfmt(&golden) == golden, "{name} is not rustfmt-clean");
    }
}

/// The generator does not lay out long lines itself; rustfmt does (lane B round 5: the
/// quest and actor lanes hit the old width mimicry with long member paths and a
/// one-member shared group). A definition with such names still yields a golden-stable,
/// rustfmt-clean module.
#[test]
fn long_names_and_single_member_groups_are_left_to_rustfmt() {
    let shared = "name = \"OnlyOneMemberInThisSharedGroupWithAVeryLongName\"\nkind = \"group\"\n\
                  source = \"corpus: t\"\nconfidence = \"corpus\"\n\
                  [[member]]\nsig = \"XNAM\"\nname = \"AnExtraordinarilyLongMemberNameForTesting\"\ntype = \"u32\"\n";
    let record = "signature = \"LONG\"\nname = \"LongNames\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
                  [[member]]\ngroup_ref = \"OnlyOneMemberInThisSharedGroupWithAVeryLongName\"\ncard = \"repeated\"\n\
                  [[member]]\nsig = \"DATA\"\nname = \"DataWithAnUnusuallyLongStructNameHere\"\ntype = \"struct\"\n\
                  field = [{ name = \"AVeryLongFieldNameThatPushesTheCallPastTheWidth\", type = \"f32\" }, \
                  { name = \"AnotherQuiteLongFieldNameForTheSameReason\", type = \"formid\" }]\n";
    let schema = load_sources([
        (
            "common/OnlyOneMemberInThisSharedGroupWithAVeryLongName.toml",
            shared,
        ),
        ("LONG.toml", record),
    ])
    .unwrap();
    let formatted = rustfmt(&generate(&schema).unwrap());
    assert_eq!(rustfmt(&formatted), formatted);
}
