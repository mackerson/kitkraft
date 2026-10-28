//! `cargo xtask inventory-check`: the KK-111 inventory counts, checked against the corpus.
//!
//! Runs the release inventory scan (the same command the KK-111 ticket accepts) and fails
//! when the scan does not pass, or when one of the ticket's types lacks its
//! `== <SIG>: <count> records` line with `defined: true`. The counts are the ticket's
//! corpus counts (TICKET.md), so a drift in the corpus or a definition that changes a
//! count fails here.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

/// The KK-111 inventory types and their record counts in the corpus (TICKET.md).
const EXPECTED: &[(&str, u64)] = &[
    ("DEBR", 3),
    ("OSWP", 3),
    ("WTHR", 3),
    ("PERS", 2),
    ("SDLT", 2),
    ("DOBJ", 1),
    ("GWED", 1),
    ("MAAM", 1),
];

/// The release inventory scan: the same command the KK-111 ticket accepts.
const SCAN: &[&str] = &[
    "test",
    "-p",
    "kitkraft-esm",
    "--release",
    "--test",
    "inventory_types",
    "--",
    "--include-ignored",
    "--exact",
    "inventory_types_full",
    "--nocapture",
];

/// The scan line a type must print: `== <SIG>: <count> records` ... `defined: true`.
fn find_defined_line<'a>(lines: &'a [String], sig: &str, count: u64) -> Option<&'a str> {
    let prefix = format!("== {sig}: {count} records");
    lines
        .iter()
        .map(String::as_str)
        .find(|line| line.starts_with(prefix.as_str()) && line.contains("defined: true"))
}

/// `cargo xtask inventory-check`: fail when the scan does not pass, or a ticketed count or
/// `defined: true` is missing from its output.
pub(crate) fn run(root: &Path) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(&cargo)
        .args(SCAN)
        .current_dir(root)
        .output()
        .with_context(|| format!("could not start `{cargo}`"))?;
    // The scan prints to the test's stdout, which cargo may route to stderr.
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        bail!("the inventory scan failed ({}):\n{combined}", output.status);
    }
    let lines: Vec<String> = combined.lines().map(str::to_string).collect();
    if !lines
        .iter()
        .any(|line| line.starts_with("test result: ok") && line.contains("1 passed"))
    {
        bail!("the inventory scan did not pass 1 test:\n{combined}");
    }
    for (sig, count) in EXPECTED {
        let line = find_defined_line(&lines, sig, *count)
            .with_context(|| format!("no `== {sig}: {count} records ... defined: true` line"))?;
        println!("inventory-check: {line}");
    }
    println!(
        "inventory-check: ok ({} types, counts and `defined: true` as ticketed)",
        EXPECTED.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::find_defined_line;

    /// The ticketed line is found; a different count or a missing `defined: true` is not.
    #[test]
    fn finds_only_the_ticketed_line() {
        let lines: Vec<String> = vec![
            "== DEBR: 3 records, 3 signatures, defined: true".into(),
            "== DEBR: 3 records, EDID (12..30)".into(),
            "== OSWP: 4 records, defined: true".into(),
        ];
        assert_eq!(
            find_defined_line(&lines, "DEBR", 3),
            Some("== DEBR: 3 records, 3 signatures, defined: true")
        );
        assert_eq!(find_defined_line(&lines, "OSWP", 3), None);
        assert_eq!(find_defined_line(&lines, "DEBR", 4), None);
    }
}
