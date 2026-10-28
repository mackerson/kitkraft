//! `cargo xtask codegen [--check]` (M1.15, ADR 0003): typed records from the schema.
//!
//! Loads the embedded definition set (`kitkraft_schema::builtin()`), runs
//! `kitkraft_schema::generate`, pipes every file it writes through `rustfmt` with the
//! workspace `rustfmt.toml` (the generator does not lay out lines itself; the schema's
//! golden tests format the same way), and writes [`OUT_DIR`]`/records.rs` plus a fixed
//! `mod.rs`. Both start with
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

/// A definition set: every file as [`kitkraft_schema::BUILTIN_DEFINITIONS`] names it
/// (`WEAP.toml`, `common/Model.toml`), with its contents, in that list's order, and the
/// schema loaded from them.
pub(crate) struct DefSet {
    pub(crate) files: Vec<(String, String)>,
    pub(crate) schema: kitkraft_schema::Schema,
}

impl DefSet {
    fn load(files: Vec<(String, String)>, what: &str) -> Result<DefSet> {
        let schema =
            kitkraft_schema::load_sources(files.iter().map(|(n, t)| (n.as_str(), t.as_str())))
                .map_err(|e| anyhow!("loading the {what} schema definitions: {e}"))?;
        Ok(DefSet { files, schema })
    }

    /// The set embedded in this xtask binary (`BUILTIN_DEFINITIONS`, as compiled).
    pub(crate) fn builtin() -> Result<DefSet> {
        let files = kitkraft_schema::BUILTIN_DEFINITIONS
            .iter()
            .map(|(n, t)| ((*n).to_string(), (*t).to_string()))
            .collect();
        DefSet::load(files, "builtin")
    }

    /// The set in `crates/schema/defs/` on disk, in the order `BUILTIN_DEFINITIONS` keeps
    /// (file names, top-level files then `common/`). `cargo xtask add-def` uses it: it
    /// changes that list, which this binary embedded before the change.
    pub(crate) fn on_disk(root: &Path) -> Result<DefSet> {
        let mut files = Vec::new();
        for name in definition_names(root)? {
            let path = root.join(DEFS_DIR).join(&name);
            let text =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            files.push((name, text));
        }
        DefSet::load(files, "on-disk")
    }

    /// Fingerprint of the set: every file name and its contents, in list order.
    pub(crate) fn hash(&self) -> String {
        let mut owned: Vec<Vec<u8>> = Vec::new();
        for (name, text) in &self.files {
            owned.push(name.as_bytes().to_vec());
            owned.push(vec![0]);
            owned.push(lf(text).into_bytes());
            owned.push(vec![0]);
        }
        let parts: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
        format!("fnv1a64:{:016x}", fnv1a64(&parts))
    }
}

/// The definition directory, relative to the workspace root.
pub(crate) const DEFS_DIR: &str = "crates/schema/defs";

/// The `*.toml` files of [`DEFS_DIR`] as `BUILTIN_DEFINITIONS` lists them: top-level names
/// sorted, then `common/<name>` sorted.
pub(crate) fn definition_names(root: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (sub, prefix) in [("", ""), ("common", "common/")] {
        let dir = root.join(DEFS_DIR).join(sub);
        let mut names = Vec::new();
        for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry
                .with_context(|| format!("reading {}", dir.display()))?
                .path();
            if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    names.push(format!("{prefix}{name}"));
                }
            }
        }
        names.sort();
        out.extend(names);
    }
    Ok(out)
}

/// The files `codegen` writes: (file name in [`OUT_DIR`], contents).
fn render(root: &Path, set: &DefSet) -> Result<Vec<(&'static str, String)>> {
    let code = kitkraft_schema::generate(&set.schema).context("generating typed records")?;
    let defs = set.files.len();
    let header = format!(
        "// DO NOT EDIT. Written by `cargo xtask codegen` from crates/schema/defs/ ({defs} files,\n\
         // kitkraft_schema::BUILTIN_DEFINITIONS); rerun it after changing a definition.\n\
         // Schema defs hash: {}\n\n",
        set.hash()
    );
    // Every written file goes through rustfmt, unconditionally: the generator leaves line
    // breaking to it.
    Ok(vec![
        ("mod.rs", rustfmt(root, MOD_RS)?),
        ("records.rs", rustfmt(root, &format!("{header}{code}"))?),
    ])
}

/// Formats `source` with the workspace `rustfmt.toml`.
pub(crate) fn rustfmt(root: &Path, source: &str) -> Result<String> {
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
    run_with(root, check, &DefSet::builtin()?)
}

/// [`run`] for the definition set `set`.
pub(crate) fn run_with(root: &Path, check: bool, set: &DefSet) -> Result<()> {
    let dir = root.join(OUT_DIR);
    let files = render(root, set)?;
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
