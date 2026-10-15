//! `cargo xtask parity` (M1.14): writes `docs/record-parity.md` from the corpus.
//!
//! For every file in `corpus/manifest.toml` (read in place from `KITKRAFT_DATA_DIR`, size
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
const DATA_DIR_VAR: &str = "KITKRAFT_DATA_DIR";

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
    /// Records decoded through the definition.
    typed_records: u64,
    /// Typed occurrences that did not decode through their layout.
    failed: u64,
    /// Subrecord positions that did not re-encode identically.
    reencode_conflicts: u64,
    /// Decoded occurrences per confidence level.
    by_confidence: BTreeMap<String, u64>,
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

/// `cargo xtask parity [--check]`: with `check`, regenerate in memory and fail when the
/// committed document differs (line endings normalized) instead of writing it.
pub(crate) fn run(root: &Path, check: bool) -> Result<()> {
    let data_dir = crate::env_var_os(DATA_DIR_VAR)
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
            agg.typed_records = agg.typed_records.saturating_add(u(t, "typed_records"));
            agg.reencode_conflicts = agg
                .reencode_conflicts
                .saturating_add(u(t, "reencode_conflicts"));
            if let Some(occurrences) = t.get("occurrences") {
                agg.failed = agg.failed.saturating_add(u(occurrences, "failed"));
            }
            for c in CONFIDENCE {
                let n = t.get("decoded_by_confidence").map_or(0, |by| u(by, c));
                let slot = agg.by_confidence.entry(c.to_string()).or_insert(0);
                *slot = slot.saturating_add(n);
            }
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

    let builtin = kitkraft_schema::builtin()
        .map_err(|e| anyhow::anyhow!("loading the builtin schema definitions: {e}"))?;
    let defined = builtin.records.len();
    let found = occurrence_check(builtin, &types);
    if !found.unobserved.is_empty() {
        bail!(
            "{} member(s) claim `corpus` (or better) confidence, but their subrecord signature \
             occurs in no record of their type in the corpus (lower them to `reference`):\n  {}",
            found.unobserved.len(),
            found.unobserved.join("\n  ")
        );
    }
    if !found.observed_container.is_empty() {
        bail!(
            "{} `container-reference` member(s) occur in the corpus: validate each against \
             every occurrence and raise it to `corpus` (ADR 0005 Amendment 5):\n  {}",
            found.observed_container.len(),
            found.observed_container.join("\n  ")
        );
    }
    // Per-confidence counts of the definitions (M2.F9), from the same command agents use.
    let schema = kit(root, &["schema"])?;
    let schema = schema.get("data").context("schema result has no data")?;
    let signatures: Vec<String> = builtin
        .records()
        .map(|r| r.signature.as_str().to_string())
        .collect();
    let families = family_table(&signatures, &types);
    let doc = render(
        &manifest,
        defined,
        &types,
        &files,
        &not_examined,
        schema,
        &families,
    );
    let out = root.join(OUT);
    if check {
        let current = fs::read_to_string(&out).unwrap_or_default();
        if lf(&current) != doc {
            bail!("{OUT} is stale; run `cargo xtask parity` and commit the result");
        }
        println!("parity: {OUT} is up to date ({} record types)", types.len());
        return Ok(());
    }
    fs::write(&out, &doc).with_context(|| format!("writing {}", out.display()))?;
    println!("parity: wrote {OUT} ({} record types)", types.len());
    Ok(())
}

/// What [`occurrence_check`] found; both lists fail `cargo xtask parity`.
#[derive(Debug, Default)]
struct OccurrenceCheck {
    /// Members claiming `corpus` or better whose signature the corpus never shows.
    unobserved: Vec<String>,
    /// `container-reference` members whose signature the corpus shows.
    observed_container: Vec<String>,
}

/// The occurrence check, both directions (ADR 0005 Amendments 2 and 5).
///
/// - Members whose confidence says the corpus confirmed them (`corpus` or better) while no
///   record of their type in the corpus has their signature ("Seen" in the record types
///   table): a `corpus` claim needs an occurrence, so such a member must be `reference`.
///   `reference` and `container-reference` members make no corpus claim and are skipped.
/// - `container-reference` members (a UESP Skyrim container fact no Starfield file shows)
///   whose signature the corpus does show: the level is no longer true, and the member
///   must be validated and raised to `corpus`.
///
/// Signature-level: a signature seen anywhere in the record type counts for every member
/// that uses it. A shared group (`group_ref`) is checked at its place in the record (its
/// own confidence, and one of the signatures that can start it): its inner members'
/// confidence is about the shared layout, which the record types that have it confirm.
/// Record types the corpus does not have are skipped.
fn occurrence_check(
    schema: &kitkraft_schema::Schema,
    types: &BTreeMap<String, TypeAgg>,
) -> OccurrenceCheck {
    use kitkraft_schema::{Cardinality, Confidence, Member};
    fn walk(members: &[Member], prefix: &str, sig: &str, agg: &TypeAgg, out: &mut OccurrenceCheck) {
        let seen = |s: &str| agg.subs.get(s).is_some_and(|(n, _)| *n > 0);
        for member in members {
            let path = if prefix.is_empty() {
                member.name().to_string()
            } else {
                format!("{prefix}.{}", member.name())
            };
            match member {
                Member::Subrecord(sub) => {
                    let level = sub.provenance.confidence;
                    let line = format!("{sig}.{path} ({}, {level})", sub.signature);
                    let occurs = seen(sub.signature.as_str());
                    if level >= Confidence::Corpus && !occurs {
                        out.unobserved.push(line);
                    } else if level == Confidence::ContainerReference && occurs {
                        out.observed_container.push(line);
                    }
                }
                Member::Group(g) if g.shared.is_some() => {
                    // The signatures that can start the group: up to its first required one.
                    let mut starts = Vec::new();
                    for inner in &g.members {
                        if let Member::Subrecord(s) = inner {
                            starts.push(s.signature.as_str().to_string());
                        }
                        if inner.cardinality() == Cardinality::Required {
                            break;
                        }
                    }
                    let level = g.provenance.confidence;
                    let line = format!(
                        "{sig}.{path} (group starting {}, {level})",
                        starts.join("/")
                    );
                    let occurs = starts.iter().any(|s| seen(s));
                    if level >= Confidence::Corpus && !occurs {
                        out.unobserved.push(line);
                    } else if level == Confidence::ContainerReference && occurs {
                        out.observed_container.push(line);
                    }
                }
                Member::Group(g) => walk(&g.members, &path, sig, agg, out),
                Member::AnyUntil(_) => {}
            }
        }
    }
    let mut out = OccurrenceCheck::default();
    for record in schema.records() {
        let sig = record.signature.as_str();
        if let Some(agg) = types.get(sig) {
            walk(&record.members, "", sig, agg, &mut out);
        }
    }
    out
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

/// Confidence levels, weakest first (ADR 0004, ADR 0005 Amendment 5), as `kit schema`
/// names them (`kitkraft_schema::Confidence::ALL`; a test keeps the two in step).
const CONFIDENCE: [&str; 5] = [
    "reference",
    "container-reference",
    "corpus",
    "ck-diff",
    "verified",
];

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
    right.resize(head.len(), true);
    table(&head, &rows, &right)
}

/// The "Decoded through the definitions" table: one row per defined record type (in the
/// order given), then a total.
fn decoded_table(sorted: &[(&String, &TypeAgg)]) -> String {
    let mut total = TypeAgg::default();
    let mut rows = Vec::new();
    let row = |label: String, name: String, a: &TypeAgg| {
        let decoded = a
            .by_confidence
            .values()
            .fold(0u64, |sum, v| sum.saturating_add(*v));
        let mut cells = vec![
            label,
            name,
            n(a.typed_records),
            n(decoded),
            n(a.failed),
            n(a.reencode_conflicts),
        ];
        cells.extend(
            CONFIDENCE
                .iter()
                .map(|c| n(a.by_confidence.get(*c).copied().unwrap_or(0))),
        );
        cells
    };
    for (sig, agg) in sorted {
        let Some(definition) = &agg.definition else {
            continue;
        };
        total.typed_records = total.typed_records.saturating_add(agg.typed_records);
        total.failed = total.failed.saturating_add(agg.failed);
        total.reencode_conflicts = total
            .reencode_conflicts
            .saturating_add(agg.reencode_conflicts);
        for (c, v) in &agg.by_confidence {
            let slot = total.by_confidence.entry(c.clone()).or_insert(0);
            *slot = slot.saturating_add(*v);
        }
        rows.push(row(format!("`{sig}`"), definition.clone(), agg));
    }
    rows.push(row("**Total**".to_string(), String::new(), &total));
    let mut head = vec![
        "Record",
        "Schema",
        "Typed records",
        "Decoded",
        "Failed",
        "Re-encode",
    ];
    head.extend(CONFIDENCE);
    let mut right = vec![false, false];
    right.resize(head.len(), true);
    table(&head, &rows, &right)
}

/// The schema families, each with the record types its lane typed (`docs/schema/*-family.md`,
/// `crates/schema/defs/README.md` "Current set"). Every other record type, defined or not,
/// counts under [`OTHER_FAMILY`].
const FAMILIES: &[(&str, &[&str])] = &[
    (
        "weapons",
        &[
            "WEAP", "AMMO", "PROJ", "AMDL", "OMOD", "COBJ", "ZOOM", "LVLI", "FLST", "INNR",
        ],
    ),
    (
        "world",
        &["WRLD", "CELL", "REFR", "ACHR", "NAVM", "LCTN", "LCRT"],
    ),
    (
        "planet",
        &[
            "PCMT", "PCBN", "PCCN", "LVLP", "PKIN", "GBFM", "BIOM", "PNDT", "STDT", "SUNP", "SFBK",
            "SFPT", "RSGD", "RSPJ", "TRAV",
        ],
    ),
    (
        "quest",
        &[
            "FACT", "QUST", "PACK", "DIAL", "INFO", "SCEN", "SMBN", "SMQN", "SMEN", "CHAL",
        ],
    ),
    (
        "actor",
        &[
            "NPC_", "LVLN", "OTFT", "CLAS", "AFFE", "RACE", "PERK", "SPEL", "MGEF",
        ],
    ),
    (
        "items",
        &[
            "STAT", "MISC", "CONT", "ACTI", "FURN", "MSTT", "TERM", "TMLM", "ARMO", "IDLE", "ANIO",
            "CPTH",
        ],
    ),
];

/// The family of record types no family lane owns.
const OTHER_FAMILY: &str = "container/other";

/// The M4.BX2 inventory definitions (corpus order and cardinality, EDID and lstrings typed,
/// every other payload opaque): record types no family lane owns yet, so they count under
/// [`OTHER_FAMILY`] until a lane takes one (a test checks each has a definition and sits in
/// no family).
const INVENTORY: &[&str] = &[
    "DOOR", "LIGH", "IRES", "MESG", "LAYR", "RFGP", "PGRE", "PHZD", "WKMF", "LSCR", "SOUN", "WWED",
    "ARTO", "TXST",
];

/// The family a record type belongs to.
fn family_of(signature: &str) -> &'static str {
    FAMILIES
        .iter()
        .find(|(_, sigs)| sigs.contains(&signature))
        .map_or(OTHER_FAMILY, |(name, _)| name)
}

/// The "Families" table: per family, the record types the corpus shows, the record
/// definitions the schema has (`defined`: every defined signature), records, and subrecord
/// occurrences by class; then a total.
fn family_table(defined: &[String], types: &BTreeMap<String, TypeAgg>) -> String {
    #[derive(Default, Clone, Copy)]
    struct Sum {
        types: u64,
        definitions: u64,
        records: u64,
        typed: u64,
        opaque: u64,
        unknown: u64,
    }
    let names: Vec<&str> = FAMILIES
        .iter()
        .map(|(name, _)| *name)
        .chain([OTHER_FAMILY])
        .collect();
    let mut sums: BTreeMap<&str, Sum> = names.iter().map(|n| (*n, Sum::default())).collect();
    for sig in defined {
        if let Some(s) = sums.get_mut(family_of(sig)) {
            s.definitions = s.definitions.saturating_add(1);
        }
    }
    for (sig, agg) in types {
        let Some(s) = sums.get_mut(family_of(sig)) else {
            continue;
        };
        s.types = s.types.saturating_add(1);
        s.records = s.records.saturating_add(agg.records);
        for (count, class) in agg.subs.values() {
            let slot = match class.as_str() {
                "typed" => &mut s.typed,
                "opaque" => &mut s.opaque,
                "unknown" => &mut s.unknown,
                _ => continue,
            };
            *slot = slot.saturating_add(*count);
        }
    }
    let row = |label: String, s: Sum| {
        vec![
            label,
            n(s.types),
            n(s.definitions),
            n(s.records),
            n(s.typed),
            n(s.opaque),
            n(s.unknown),
        ]
    };
    let mut total = Sum::default();
    let mut rows = Vec::new();
    for name in &names {
        let s = sums.get(name).copied().unwrap_or_default();
        total.types = total.types.saturating_add(s.types);
        total.definitions = total.definitions.saturating_add(s.definitions);
        total.records = total.records.saturating_add(s.records);
        total.typed = total.typed.saturating_add(s.typed);
        total.opaque = total.opaque.saturating_add(s.opaque);
        total.unknown = total.unknown.saturating_add(s.unknown);
        rows.push(row((*name).to_string(), s));
    }
    rows.push(row("**Total**".to_string(), total));
    table(
        &[
            "Family",
            "Record types",
            "Definitions",
            "Records",
            "Typed",
            "Opaque",
            "Unknown",
        ],
        &rows,
        &[false, true, true, true, true, true, true],
    )
}

fn render(
    manifest: &Manifest,
    defined: usize,
    types: &BTreeMap<String, TypeAgg>,
    files: &[FileRow],
    not_examined: &[String],
    schema: &Value,
    families: &str,
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
    out.push_str("## Families\n\n");
    out.push_str(
        "Per schema family (the lanes that typed them; `docs/schema/*-family.md`), summed over\n\
         the family's record types from the tables below: **Record types** seen in the corpus,\n\
         **Definitions** in the schema (with or without corpus records), **Records**, and\n\
         subrecord occurrences the definitions type, keep **Opaque**, or do not name\n\
         (**Unknown**). *container/other* holds every other type: TES4, GMST, GLOB, KYWD, the\n\
         lstring-only definitions, the M4.BX2 inventory definitions and the types without a\n\
         definition.\n\n",
    );
    let _ = writeln!(
        out,
        "M4.BX2 inventory definitions (in *container/other*): {}.\n",
        INVENTORY.join(", ")
    );
    out.push_str(families);
    out.push_str("\n## Corpus and method\n\n");
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
        "- Classification is by subrecord signature against the record type's definition,\n  \
         and `coverage` decodes every record of a defined type through its generated\n  \
         definition and encodes it again (M3.F2): a typed subrecord whose payload does not\n  \
         decode is counted as failed, not typed (see \"Decoded through the definitions\").\n\n",
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
    let failed = sorted
        .iter()
        .fold(0u64, |sum, (_, a)| sum.saturating_add(a.failed));
    let _ = writeln!(
        out,
        "\nSubrecord occurrences: {} in total; {} typed ({} of them failed to decode), {} \
         opaque, {} unknown.",
        n(occ_total),
        n(occ("typed")),
        n(failed),
        n(occ("opaque")),
        n(occ("unknown"))
    );

    out.push_str("\n## Decoded through the definitions\n\n");
    out.push_str(
        "Per defined record type, from `kit coverage` (M3.F2): **Typed records** decoded\n\
         through the generated definition; **Decoded** typed subrecord occurrences whose\n\
         payload decoded, **Failed** those that did not (the payload does not fit the layout,\n\
         or the definition does not accept it at that position; kept raw); **Re-encode**\n\
         subrecord positions where encoding the decoded record did not reproduce the input.\n\
         The confidence columns split **Decoded** by the confidence of the member each\n\
         occurrence is placed in (by position, as the decoder places it; lane B round 4).\n\n",
    );
    out.push_str(&decoded_table(&sorted));

    out.push_str("\n## Definitions by confidence\n\n");
    out.push_str(
        "Subrecord definitions per record type (groups expanded) from `kit schema --json`\n\
         (M2.F9), by `confidence` (ADR 0004: `reference` documented in an allowed source,\n\
         `container-reference` a container fact from UESP's Skyrim pages that no official\n\
         file shows (ADR 0005 Amendment 5), `corpus` observed in the official files,\n\
         `ck-diff` checked against Creation Kit output, `verified` both). **Typed**\n\
         definitions give a layout; **Opaque** ones keep the payload as raw bytes. The\n\
         occurrence check fails when a `corpus` (or better) member never occurs, and when a\n\
         `container-reference` member does.\n\n",
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
    out.push_str(
        "\nBy design, REFR's FLCS, KSIZ, KWDA, VLMS and OPDS count as unknown: in REFR they\n\
         occur only inside the BFCB..BFCE base form component blocks (the component bodies the\n\
         definition keeps as raw subrecords, `common/BfcbBlock.toml`), not as REFR members.\n",
    );

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
    use super::{
        confidence_table, decoded_table, family_of, family_table, n, occurrence_check, table,
        TypeAgg, CONFIDENCE, FAMILIES, INVENTORY,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    /// Lane B round 4: a member that claims `corpus` must have its signature among the
    /// signatures the corpus shows in records of its type (COBJ FULL claimed `corpus` with
    /// no occurrence). `reference` members and types the corpus lacks are not checked.
    #[test]
    fn corpus_members_need_an_occurrence() {
        let schema = kitkraft_schema::load_sources([(
            "TEST.toml",
            "signature = \"TEST\"\nname = \"Test\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
             [[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\n\
             [[member]]\nsig = \"FULL\"\nname = \"Name\"\ntype = \"lstring\"\ncard = \"optional\"\n\
             [[member]]\nsig = \"XNAM\"\nname = \"Extra2\"\ntype = \"u32\"\ncard = \"optional\"\n\
             source = \"xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L1-L2\"\n\
             confidence = \"reference\"\n",
        )])
        .unwrap();
        let mut agg = TypeAgg::default();
        agg.subs.insert("EDID".into(), (3, "typed".into()));
        let types = [("TEST".to_string(), agg)].into_iter().collect();
        assert_eq!(
            occurrence_check(&schema, &types).unobserved,
            ["TEST.Name (FULL, corpus)"]
        );
        // A type the corpus does not have is not checked.
        assert!(occurrence_check(&schema, &Default::default())
            .unobserved
            .is_empty());

        // A shared group is checked at its place: one of its start signatures must occur;
        // its inner members' levels are about the shared layout.
        let block =
            "name = \"Block\"\nkind = \"group\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
                     [[member]]\nsig = \"BFCB\"\nname = \"Start\"\ntype = \"u32\"\n\
                     [[member]]\nsig = \"BFCE\"\nname = \"End\"\ntype = \"empty\"\n";
        let rec = "signature = \"TEST\"\nname = \"Test\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
                   [[member]]\ngroup_ref = \"Block\"\ncard = \"optional\"\n";
        let schema =
            kitkraft_schema::load_sources([("common/Block.toml", block), ("TEST.toml", rec)])
                .unwrap();
        let with = |sigs: &[&str]| {
            let mut agg = TypeAgg::default();
            for s in sigs {
                agg.subs.insert((*s).to_string(), (1, "typed".into()));
            }
            [("TEST".to_string(), agg)].into_iter().collect()
        };
        assert!(occurrence_check(&schema, &with(&["BFCB"]))
            .unobserved
            .is_empty());
        assert_eq!(
            occurrence_check(&schema, &with(&["EDID"])).unobserved,
            ["TEST.Block (group starting BFCB, corpus)"]
        );
    }

    /// ADR 0005 Amendment 5: the builtin TES4's `container-reference` members (`DATA`
    /// after `MAST`, `INTV`) pass while the corpus lacks them, with no list of names, and
    /// fail the check once a corpus file has one.
    #[test]
    fn container_reference_members_must_not_occur() {
        let schema = kitkraft_schema::builtin().unwrap();
        let tes4 = |sigs: &[&str]| {
            let mut agg = TypeAgg::default();
            for s in sigs {
                agg.subs.insert((*s).to_string(), (14, "typed".into()));
            }
            [("TES4".to_string(), agg)].into_iter().collect()
        };
        // Every signature the 14 official TES4 records show.
        let seen = [
            "HEDR", "CNAM", "SNAM", "MAST", "ONAM", "TNAM", "BNAM", "INCC", "CHGL",
        ];
        let found = occurrence_check(schema, &tes4(&seen));
        assert!(found.unobserved.is_empty(), "{:?}", found.unobserved);
        assert!(found.observed_container.is_empty());

        let mut with_intv = seen.to_vec();
        with_intv.push("INTV");
        let found = occurrence_check(schema, &tes4(&with_intv));
        assert_eq!(
            found.observed_container,
            ["TES4.TagifyCount (INTV, container-reference)"]
        );
        let mut with_data = seen.to_vec();
        with_data.push("DATA");
        assert_eq!(
            occurrence_check(schema, &tes4(&with_data)).observed_container,
            ["TES4.Master.FileSize (DATA, container-reference)"]
        );
    }

    /// Every family member is a builtin record definition, and no type is in two families,
    /// so the rollup cannot drift from the schema unnoticed.
    #[test]
    fn families_name_defined_types_once() {
        let schema = kitkraft_schema::builtin().unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for (family, sigs) in FAMILIES {
            for sig in *sigs {
                assert!(seen.insert(*sig), "{sig} is in two families");
                let parsed = kitkraft_schema::Signature::parse(sig).unwrap();
                assert!(
                    schema.record(parsed).is_some(),
                    "{family}: {sig} has no definition"
                );
            }
        }
        assert_eq!(family_of("WEAP"), "weapons");
        assert_eq!(family_of("TES4"), "container/other");
        // The M4.BX2 inventories have definitions and no family lane yet.
        for sig in INVENTORY {
            assert_eq!(family_of(sig), "container/other", "{sig}");
            let parsed = kitkraft_schema::Signature::parse(sig).unwrap();
            assert!(schema.record(parsed).is_some(), "{sig} has no definition");
        }
    }

    #[test]
    fn family_rows_sum_types_and_occurrences() {
        let agg = |records: u64, subs: &[(&str, u64, &str)]| TypeAgg {
            records,
            subs: subs
                .iter()
                .map(|(s, c, k)| ((*s).to_string(), (*c, (*k).to_string())))
                .collect(),
            ..TypeAgg::default()
        };
        let types: BTreeMap<String, TypeAgg> = [
            (
                "WEAP".to_string(),
                agg(10, &[("EDID", 10, "typed"), ("MO4C", 2, "opaque")]),
            ),
            ("AMMO".to_string(), agg(5, &[("EDID", 5, "typed")])),
            ("RFGP".to_string(), agg(7, &[("NNAM", 7, "unknown")])),
        ]
        .into_iter()
        .collect();
        let defined = ["WEAP", "AMMO", "PROJ", "TES4"].map(String::from);
        let t = family_table(&defined, &types);
        assert!(
            t.starts_with(
                "| Family          | Record types | Definitions | Records | Typed | Opaque | Unknown |"
            ),
            "{t}"
        );
        assert!(
            t.contains("| weapons         |            2 |           3 |      15 |    15 |      2 |       0 |"),
            "{t}"
        );
        assert!(
            t.contains("| container/other |            1 |           1 |       7 |     0 |      0 |       7 |"),
            "{t}"
        );
        assert!(
            t.contains("| **Total**       |            3 |           4 |      22 |    15 |      2 |       7 |"),
            "{t}"
        );
    }

    #[test]
    fn confidence_columns_follow_the_schema_levels() {
        let names: Vec<&str> = kitkraft_schema::Confidence::ALL
            .iter()
            .map(|c| c.as_str())
            .collect();
        assert_eq!(names, CONFIDENCE);
    }

    #[test]
    fn decoded_rows_skip_undefined_types_and_total() {
        let weap = TypeAgg {
            definition: Some("Weapon".into()),
            typed_records: 452,
            failed: 2,
            reencode_conflicts: 1,
            by_confidence: [("corpus".to_string(), 1000), ("reference".to_string(), 5)]
                .into_iter()
                .collect(),
            ..TypeAgg::default()
        };
        let refr = TypeAgg {
            records: 9,
            ..TypeAgg::default()
        };
        let (w, r) = ("WEAP".to_string(), "REFR".to_string());
        let t = decoded_table(&[(&r, &refr), (&w, &weap)]);
        assert!(!t.contains("REFR"), "{t}");
        assert!(
            t.starts_with("| Record    | Schema | Typed records | Decoded | Failed | Re-encode |"),
            "{t}"
        );
        assert!(
            t.contains("| `WEAP`    | Weapon |           452 |   1,005 |      2 |         1 |"),
            "{t}"
        );
        assert!(
            t.contains("| **Total** |        |           452 |   1,005 |"),
            "{t}"
        );
    }

    #[test]
    fn confidence_rows_and_total() {
        let counts = |r, c| {
            json!({"reference": r, "container-reference": 0, "corpus": c, "ck-diff": 0,
                   "verified": 0})
        };
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
                "| `WEAP`    | Weapon |              3 |     1 |      2 |         1 |                   0 |      2 |"
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
