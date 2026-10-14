//! `cargo xtask parity` (M1.14): writes `docs/record-parity.md` from the corpus.
//!
//! For every file in `corpus/manifest.toml` (read in place from `KITCRAFT_DATA_DIR`, size
//! checked against the manifest), runs `kit coverage <file> --json` and
//! `kit roundtrip <file> --verify --json` through `cargo run --release -p kit-cli`, so the
//! numbers come from the same command path users and agents call. The per-file results are
//! merged per record type and rendered as Markdown. The output depends only on the corpus,
//! the schema and the code (no timestamps or timings), so rerunning is idempotent.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::codegen::{defs_hash, fnv1a64, lf};

/// The generated document, relative to the workspace root.
const OUT: &str = "docs/record-parity.md";
const MANIFEST: &str = "corpus/manifest.toml";
const DATA_DIR_VAR: &str = "KITCRAFT_DATA_DIR";

/// One corpus file as the manifest lists it.
struct CorpusFile {
    name: String,
    size: u64,
}

struct Manifest {
    recorded: String,
    hash: String,
    files: Vec<CorpusFile>,
}

fn read_manifest(root: &Path) -> Result<Manifest> {
    let path = root.join(MANIFEST);
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let table: toml::Table =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let recorded = table
        .get("recorded")
        .and_then(toml::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let mut files = Vec::new();
    for file in table
        .get("file")
        .and_then(toml::Value::as_array)
        .context("manifest has no [[file]] entries")?
    {
        let name = file
            .get("name")
            .and_then(toml::Value::as_str)
            .context("manifest [[file]] without name")?
            .to_string();
        let size = file
            .get("size")
            .and_then(toml::Value::as_integer)
            .and_then(|s| u64::try_from(s).ok())
            .with_context(|| format!("manifest entry {name} has no valid size"))?;
        files.push(CorpusFile { name, size });
    }
    let hash = format!("fnv1a64:{:016x}", fnv1a64(&[lf(&text).as_bytes()]));
    Ok(Manifest {
        recorded,
        hash,
        files,
    })
}

/// Runs `kit <words> --json` (release build) and returns the envelope. Exit 1 (a failed
/// check) still carries a result; exit 2 is an error.
fn kit(root: &Path, words: &[&str]) -> Result<Value> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(&cargo)
        .args(["run", "--quiet", "--release", "-p", "kit-cli", "--"])
        .args(words)
        .arg("--json")
        .current_dir(root)
        .output()
        .with_context(|| format!("could not start `{cargo}`"))?;
    let shown = words.join(" ");
    if !matches!(output.status.code(), Some(0 | 1)) {
        bail!(
            "`kit {shown}` failed ({}): {}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("`kit {shown} --json` did not print JSON"))
}

#[derive(Default)]
struct TypeAgg {
    definition: Option<String>,
    records: u64,
    compressed: u64,
    compressed_examined: u64,
    /// Subrecord signature -> (occurrences, class).
    subs: BTreeMap<String, (u64, String)>,
}

struct FileRow {
    name: String,
    bytes: u64,
    records: u64,
    compressed: u64,
    compressed_examined: u64,
    roundtrip: String,
}

fn u(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn s<'v>(value: &'v Value, key: &str) -> &'v str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// `cargo xtask parity`.
pub(crate) fn run(root: &Path) -> Result<()> {
    let data_dir = std::env::var_os(DATA_DIR_VAR)
        .map(PathBuf::from)
        .with_context(|| format!("set {DATA_DIR_VAR} to the Starfield Data directory"))?;
    if !data_dir.is_dir() {
        bail!("{DATA_DIR_VAR} ({}) is not a directory", data_dir.display());
    }
    let manifest = read_manifest(root)?;
    for file in &manifest.files {
        let path = data_dir.join(&file.name);
        let size = fs::metadata(&path)
            .with_context(|| format!("{}", path.display()))?
            .len();
        if size != file.size {
            bail!(
                "{}: {size} bytes, the manifest says {}; update {MANIFEST} first",
                file.name,
                file.size
            );
        }
    }

    let mut types: BTreeMap<String, TypeAgg> = BTreeMap::new();
    let mut files = Vec::new();
    let mut not_examined: Vec<String> = Vec::new();
    for file in &manifest.files {
        let path = data_dir.join(&file.name);
        let path_text = path.to_str().context("corpus path is not UTF-8")?;
        let start = Instant::now();
        let coverage = kit(root, &["coverage", path_text])?;
        let roundtrip = kit(root, &["roundtrip", path_text, "--verify"])?;
        println!(
            "parity: {} ({:.1} s)",
            file.name,
            start.elapsed().as_secs_f64()
        );
        let data = coverage
            .get("data")
            .context("coverage result has no data")?;
        if data.get("decode") != Some(&Value::Bool(true)) {
            bail!(
                "{}: coverage ran without decoding compressed records",
                file.name
            );
        }
        for item in coverage
            .pointer("/coverage/not_examined")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            not_examined.push(format!("{}: {}", file.name, item.as_str().unwrap_or("?")));
        }
        for t in data
            .get("record_types")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let agg = types.entry(s(t, "signature").to_string()).or_default();
            if agg.definition.is_none() {
                agg.definition = t
                    .get("definition")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            agg.records = agg.records.saturating_add(u(t, "records"));
            agg.compressed = agg.compressed.saturating_add(u(t, "compressed"));
            agg.compressed_examined = agg
                .compressed_examined
                .saturating_add(u(t, "compressed_examined"));
            for sub in t
                .get("signatures")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let entry = agg
                    .subs
                    .entry(s(sub, "signature").to_string())
                    .or_insert((0, s(sub, "class").to_string()));
                entry.0 = entry.0.saturating_add(u(sub, "count"));
            }
        }
        let rt = roundtrip
            .get("data")
            .context("roundtrip result has no data")?;
        let verdict = if rt.get("identical") == Some(&Value::Bool(true)) {
            "identical".to_string()
        } else {
            let offset = rt.pointer("/mismatch/offset").and_then(Value::as_u64);
            format!(
                "mismatch at {}",
                offset.map_or("?".into(), |o| o.to_string())
            )
        };
        files.push(FileRow {
            name: file.name.clone(),
            bytes: u(data, "bytes"),
            records: u(data, "records"),
            compressed: u(data, "compressed_records"),
            compressed_examined: u(data, "compressed_examined"),
            roundtrip: verdict,
        });
    }

    let defined = kitcraft_schema::builtin()
        .map(|s| s.records.len())
        .unwrap_or(0);
    // Per-confidence counts of the definitions (M2.F9), from the same command agents use.
    let schema = kit(root, &["schema"])?;
    let schema = schema.get("data").context("schema result has no data")?;
    let doc = render(&manifest, defined, &types, &files, &not_examined, schema);
    let out = root.join(OUT);
    fs::write(&out, &doc).with_context(|| format!("writing {}", out.display()))?;
    println!("parity: wrote {OUT} ({} record types)", types.len());
    Ok(())
}

/// A Markdown table with columns padded to their widest cell.
fn table(head: &[&str], rows: &[Vec<String>], right: &[bool]) -> String {
    let mut widths: Vec<usize> = head.iter().map(|h| h.len()).collect();
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.len());
        }
    }
    let line = |cells: Vec<String>| {
        let mut out = String::from("|");
        for ((cell, w), r) in cells.iter().zip(&widths).zip(right) {
            if *r {
                let _ = write!(out, " {cell:>w$} |");
            } else {
                let _ = write!(out, " {cell:<w$} |");
            }
        }
        out.push('\n');
        out
    };
    let mut out = line(head.iter().map(|h| (*h).to_string()).collect());
    let rule: Vec<String> = widths
        .iter()
        .zip(right)
        .map(|(w, r)| {
            let dashes = "-".repeat(w.saturating_sub(1));
            if *r {
                format!("{dashes}:")
            } else {
                format!("{dashes}-")
            }
        })
        .collect();
    out.push_str(&line(rule));
    for row in rows {
        out.push_str(&line(row.clone()));
    }
    out
}

/// `1234567` -> `1,234,567`.
fn n(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    let len = digits.len();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && len.saturating_sub(i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Confidence levels, weakest first (ADR 0004), as `kit schema` names them.
const CONFIDENCE: [&str; 4] = ["reference", "corpus", "ck-diff", "verified"];

/// The "Definitions by confidence" table from `kit schema --json`'s `data`.
fn confidence_table(schema: &Value) -> String {
    let row = |label: String, name: String, v: &Value| {
        let mut cells = vec![
            label,
            name,
            n(u(v, "subrecords")),
            n(u(v, "typed")),
            n(u(v, "opaque")),
        ];
        let counts = v.get("confidence_counts").unwrap_or(&Value::Null);
        cells.extend(CONFIDENCE.iter().map(|c| n(u(counts, c))));
        cells
    };
    let mut rows: Vec<Vec<String>> = schema
        .get("records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|r| row(format!("`{}`", s(r, "type")), s(r, "name").to_string(), r))
        .collect();
    rows.push(row("**Total**".to_string(), String::new(), schema));
    let mut head = vec!["Record", "Schema", "Subrecord defs", "Typed", "Opaque"];
    head.extend(CONFIDENCE);
    let mut right = vec![false, false];
    right.extend([true; 7]);
    table(&head, &rows, &right)
}

fn render(
    manifest: &Manifest,
    defined: usize,
    types: &BTreeMap<String, TypeAgg>,
    files: &[FileRow],
    not_examined: &[String],
    schema: &Value,
) -> String {
    let mut sorted: Vec<(&String, &TypeAgg)> = types.iter().collect();
    sorted.sort_by(|a, b| b.1.records.cmp(&a.1.records).then(a.0.cmp(b.0)));

    let mut rows = Vec::new();
    let (mut records, mut compressed, mut examined) = (0u64, 0u64, 0u64);
    let mut distinct = [0u64; 4];
    let mut occurrences = BTreeMap::<&str, u64>::new();
    let mut outside: Vec<String> = Vec::new();
    for (sig, agg) in &sorted {
        let mut class_count = BTreeMap::<&str, u64>::new();
        let mut unknown_here = Vec::new();
        for (sub, (count, class)) in &agg.subs {
            let c = class_count.entry(class.as_str()).or_insert(0);
            *c = c.saturating_add(1);
            let o = occurrences.entry(class.as_str()).or_insert(0);
            *o = o.saturating_add(*count);
            if class == "unknown" {
                unknown_here.push(format!("`{sub}` ({})", n(*count)));
            }
        }
        let get = |k: &str| class_count.get(k).copied().unwrap_or(0);
        let seen = agg.subs.len() as u64;
        records = records.saturating_add(agg.records);
        compressed = compressed.saturating_add(agg.compressed);
        examined = examined.saturating_add(agg.compressed_examined);
        for (slot, v) in
            distinct
                .iter_mut()
                .zip([seen, get("typed"), get("opaque"), get("unknown")])
        {
            *slot = slot.saturating_add(v);
        }
        if agg.definition.is_some() && !unknown_here.is_empty() {
            outside.push(format!("- `{sig}`: {}", unknown_here.join(", ")));
        }
        rows.push(vec![
            format!("`{sig}`"),
            agg.definition.clone().unwrap_or_else(|| "-".to_string()),
            n(agg.records),
            n(agg.compressed),
            n(seen),
            n(get("typed")),
            n(get("opaque")),
            n(get("unknown")),
        ]);
    }
    let with_def = sorted
        .iter()
        .filter(|(_, a)| a.definition.is_some())
        .count();
    rows.push(vec![
        format!("**{} types**", sorted.len()),
        format!("{with_def} defined"),
        n(records),
        n(compressed),
        n(distinct[0]),
        n(distinct[1]),
        n(distinct[2]),
        n(distinct[3]),
    ]);
    let occ = |k: &str| occurrences.get(k).copied().unwrap_or(0);
    let occ_total = occ("typed")
        .saturating_add(occ("opaque"))
        .saturating_add(occ("unknown"));

    let mut out = String::new();
    out.push_str("# Record parity\n\n");
    out.push_str(
        "**Generated by `cargo xtask parity`; do not edit by hand.** Rerun it after a change to\n\
         `crates/schema/defs/`, to the `coverage` command, or to the corpus.\n\n",
    );
    let _ = writeln!(
        out,
        "- Corpus: the {} files of `{MANIFEST}` (recorded {}, manifest hash\n  \
         `{}`), read in place from `{DATA_DIR_VAR}`; each file's\n  \
         size was checked against the manifest (the SHA-256 is not re-verified here).",
        manifest.files.len(),
        manifest.recorded,
        manifest.hash
    );
    let _ = writeln!(
        out,
        "- Schema: {defined} record definitions, defs hash `{}` (the hash named in\n  \
         `crates/esm/src/generated/records.rs`).",
        defs_hash()
    );
    let _ = writeln!(
        out,
        "- Counts come from `kit coverage <file> --json` (M1.14). Compressed records **were\n  \
         examined**: each was decompressed with the default `DecodeLimits` and its subrecords\n  \
         counted ({} of {} compressed records examined).",
        n(examined),
        n(compressed)
    );
    if not_examined.is_empty() {
        out.push_str("  None failed to decode.\n");
    } else {
        out.push_str("  Not examined:\n");
        for item in not_examined {
            let _ = writeln!(out, "  - {item}");
        }
    }
    out.push_str(
        "- Classification is by subrecord signature against the record type's definition;\n  \
         `coverage` does not decode payloads against the layouts (the typed decode and\n  \
         re-encode check over the corpus is lane A's M2.A3 driver test).\n\n",
    );
    out.push_str(
        "Columns:\n\n\
         - **Schema**: the definition's name, or `-` when the record type has no definition.\n\
         - **Records**: records of this type across the corpus; **Compressed**: how many of\n  \
         them are compressed.\n\
         - **Seen**: distinct subrecord signatures found in records of this type.\n\
         - **Typed** / **Opaque** / **Unknown**: of those, how many the definition gives a\n  \
         layout, keeps as raw bytes, or does not name (all of them when there is no\n  \
         definition). Subrecords inside an `any_until` run (a BFCB..BFCE body) are unknown.\n\n",
    );
    out.push_str("## Record types\n\n");
    out.push_str(&table(
        &[
            "Record",
            "Schema",
            "Records",
            "Compressed",
            "Seen",
            "Typed",
            "Opaque",
            "Unknown",
        ],
        &rows,
        &[false, false, true, true, true, true, true, true],
    ));
    let _ = writeln!(
        out,
        "\nSubrecord occurrences: {} in total; {} typed, {} opaque, {} unknown.",
        n(occ_total),
        n(occ("typed")),
        n(occ("opaque")),
        n(occ("unknown"))
    );

    out.push_str("\n## Definitions by confidence\n\n");
    out.push_str(
        "Subrecord definitions per record type (groups expanded) from `kit schema --json`\n\
         (M2.F9), by `confidence` (ADR 0004: `reference` documented in an allowed source,\n\
         `corpus` observed in the official files, `ck-diff` checked against Creation Kit\n\
         output, `verified` both). **Typed** definitions give a layout; **Opaque** ones keep\n\
         the payload as raw bytes.\n\n",
    );
    out.push_str(&confidence_table(schema));

    out.push_str("\n## Subrecords outside the definitions\n\n");
    if outside.is_empty() {
        out.push_str("Every subrecord seen in a defined record type is named by its definition.\n");
    } else {
        out.push_str(
            "Signatures seen in records of a defined type that its definition does not name\n\
             (occurrences in parentheses):\n\n",
        );
        for line in &outside {
            let _ = writeln!(out, "{line}");
        }
    }

    out.push_str("\n## Roundtrip\n\n");
    out.push_str(
        "`kit roundtrip <file> --verify` per corpus file: container-level byte-identical\n\
         rewrite (compressed bodies re-emitted, not recompressed; see `docs/gaps.md`).\n\n",
    );
    let file_rows: Vec<Vec<String>> = files
        .iter()
        .map(|f| {
            vec![
                f.name.clone(),
                n(f.bytes),
                n(f.records),
                format!("{} / {}", n(f.compressed_examined), n(f.compressed)),
                f.roundtrip.clone(),
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "File",
            "Bytes",
            "Records",
            "Compressed examined",
            "Roundtrip",
        ],
        &file_rows,
        &[false, true, true, true, false],
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::{confidence_table, n, table};
    use serde_json::json;

    #[test]
    fn confidence_rows_and_total() {
        let counts = |r, c| json!({"reference": r, "corpus": c, "ck-diff": 0, "verified": 0});
        let schema = json!({
            "records": [{"type": "WEAP", "name": "Weapon", "subrecords": 3, "typed": 1,
                         "opaque": 2, "confidence_counts": counts(1, 2)}],
            "subrecords": 3, "typed": 1, "opaque": 2, "confidence_counts": counts(1, 2),
        });
        let t = confidence_table(&schema);
        assert!(
            t.starts_with("| Record    | Schema | Subrecord defs | Typed | Opaque | reference |"),
            "{t}"
        );
        assert!(
            t.contains(
                "| `WEAP`    | Weapon |              3 |     1 |      2 |         1 |      2 |"
            ),
            "{t}"
        );
        assert!(t.contains("| **Total** |"), "{t}");
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(n(0), "0");
        assert_eq!(n(999), "999");
        assert_eq!(n(1000), "1,000");
        assert_eq!(n(3_829_247), "3,829,247");
    }

    #[test]
    fn tables_pad_and_align() {
        let t = table(
            &["A", "Count"],
            &[vec!["`WEAP`".into(), "12".into()]],
            &[false, true],
        );
        assert_eq!(
            t,
            "| A      | Count |\n| ------ | ----: |\n| `WEAP` |    12 |\n"
        );
    }
}
