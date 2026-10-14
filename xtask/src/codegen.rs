//! `cargo xtask codegen [--check]` (M1.15, ADR 0003): typed records from the schema.
//!
//! Loads the embedded definition set (`kitcraft_schema::builtin()`), runs
//! `kitcraft_schema::generate`, pipes the result through `rustfmt` with the workspace
//! `rustfmt.toml` (the generator only mirrors rustfmt's width rules for the constructs it
//! emits today), and writes [`OUT_DIR`]`/records.rs` plus a fixed `mod.rs`. Both start with
//! a DO NOT EDIT header; `records.rs` also names the hash of the definition set it came
//! from. `--check` writes nothing and fails when a file is missing, stale, or an unexpected
//! file sits in the directory; `cargo xtask ci` runs it.
//!
//! `BUILTIN_DEFINITIONS` (the `include_str!` list in `crates/schema/src/lib.rs`) is not
//! generated here: it has no generated region to own, so `crates/schema/tests/defs.rs`
//! stays its guard (it fails naming the entries to add or remove).

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};

/// Where the generated module lives, relative to the workspace root.
pub(crate) const OUT_DIR: &str = "crates/esm/src/generated";

const MOD_RS: &str = "\
// DO NOT EDIT. Written by `cargo xtask codegen`; `cargo xtask ci` fails when it is stale.

pub mod records;
";

/// FNV-1a, 64 bit: a stable content fingerprint (not a security hash).
pub(crate) fn fnv1a64(parts: &[&[u8]]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for part in parts {
        for &byte in *part {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}

/// Text with CRLF line endings folded to LF, so the hash does not depend on checkout
/// settings.
pub(crate) fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Fingerprint of the embedded definition set: every file name and its contents, in list
/// order.
pub(crate) fn defs_hash() -> String {
    let mut owned: Vec<Vec<u8>> = Vec::new();
    for (name, text) in kitcraft_schema::BUILTIN_DEFINITIONS {
        owned.push(name.as_bytes().to_vec());
        owned.push(vec![0]);
        owned.push(lf(text).into_bytes());
        owned.push(vec![0]);
    }
    let parts: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
    format!("fnv1a64:{:016x}", fnv1a64(&parts))
}

/// The files `codegen` writes: (file name in [`OUT_DIR`], contents).
fn render(root: &Path) -> Result<Vec<(&'static str, String)>> {
    let schema = kitcraft_schema::builtin()
        .map_err(|e| anyhow!("loading the builtin schema definitions: {e}"))?;
    let code = kitcraft_schema::generate(schema).context("generating typed records")?;
    let defs = kitcraft_schema::BUILTIN_DEFINITIONS.len();
    let header = format!(
        "// DO NOT EDIT. Written by `cargo xtask codegen` from crates/schema/defs/ ({defs} files,\n\
         // kitcraft_schema::BUILTIN_DEFINITIONS); rerun it after changing a definition.\n\
         // Schema defs hash: {}\n\n",
        defs_hash()
    );
    let records = rustfmt(root, &format!("{header}{code}"))?;
    Ok(vec![
        ("mod.rs", MOD_RS.to_string()),
        ("records.rs", records),
    ])
}

/// Formats `source` with the workspace `rustfmt.toml`.
fn rustfmt(root: &Path, source: &str) -> Result<String> {
    let rustfmt = std::env::var("RUSTFMT").unwrap_or_else(|_| "rustfmt".to_string());
    let config = root.join("rustfmt.toml");
    let mut child = Command::new(&rustfmt)
        .arg("--edition")
        .arg("2021")
        .arg("--config-path")
        .arg(&config)
        .arg("--emit")
        .arg("stdout")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("could not start `{rustfmt}`"))?;
    let mut stdin = child.stdin.take().context("rustfmt has no stdin")?;
    let input = source.to_string();
    // Write from a thread so a full stdout pipe cannot deadlock the two processes.
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output().context("waiting for rustfmt")?;
    writer
        .join()
        .map_err(|_| anyhow!("rustfmt input thread failed"))?
        .context("writing to rustfmt")?;
    if !output.status.success() {
        bail!(
            "rustfmt failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).context("rustfmt output is not UTF-8")
}

/// `cargo xtask codegen [--check]`.
pub(crate) fn run(root: &Path, check: bool) -> Result<()> {
    let dir = root.join(OUT_DIR);
    let files = render(root)?;
    if check {
        let mut stale = Vec::new();
        for (name, text) in &files {
            let path = dir.join(name);
            match fs::read_to_string(&path) {
                Ok(on_disk) if lf(&on_disk) == *text => {}
                Ok(_) => stale.push(format!("{OUT_DIR}/{name} is stale")),
                Err(_) => stale.push(format!("{OUT_DIR}/{name} is missing")),
            }
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries {
                let name = entry
                    .with_context(|| format!("reading {}", dir.display()))?
                    .file_name()
                    .to_string_lossy()
                    .into_owned();
                if !files.iter().any(|(n, _)| *n == name) {
                    stale.push(format!("{OUT_DIR}/{name} is not written by codegen"));
                }
            }
        }
        if !stale.is_empty() {
            for line in &stale {
                eprintln!("codegen: {line}");
            }
            bail!("generated code is out of date; run `cargo xtask codegen`");
        }
        println!("codegen: up to date ({OUT_DIR})");
        return Ok(());
    }
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    for (name, text) in &files {
        let path = dir.join(name);
        fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        println!("codegen: wrote {OUT_DIR}/{name} ({} bytes)", text.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{fnv1a64, lf};

    #[test]
    fn fnv1a64_matches_the_reference_vectors() {
        assert_eq!(fnv1a64(&[]), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(&[b"a"]), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(&[b"foo"]), 0xdcb2_7518_fed9_d577);
        assert_eq!(fnv1a64(&[b"fo", b"o"]), fnv1a64(&[b"foo"]));
    }

    #[test]
    fn line_endings_are_folded() {
        assert_eq!(lf("a\r\nb\n"), "a\nb\n");
    }
}
