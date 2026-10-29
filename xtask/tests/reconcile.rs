//! Reconciliation of the parity documents with the corpus, the schema and each other.
//!
//! `docs/record-parity.md` and the headline of `docs/foothold/gap-matrix.md` are written by
//! `cargo xtask parity`; `--check` only proves the generator reproduces its own output.
//! This test trusts neither the generator nor `kit coverage`: it reads the documents as
//! text and checks them against
//!
//! - the schema (`kitkraft_schema::BUILTIN_DEFINITIONS`, the files of
//!   `crates/schema/defs/`): the definition count, each row's definition name, the defs
//!   hash (recomputed here), the inventory list, the per-type confidence counts;
//! - each other: the totals of every table, the headline's counts against the parity
//!   totals (equal, or different by exactly the exclusion the headline states), the
//!   manifest hash (recomputed from `corpus/manifest.toml`);
//! - the hand-kept record-family rows of the gap matrix: every type a row names is a
//!   corpus type, each row's Typed-readable is one the generated data allows (see
//!   [`readable_violations`]), and the row tally the matrix states;
//! - the corpus (only with `KITKRAFT_DATA_DIR`, `docs/corpus.md`; one skip line without
//!   it): records per type and per file, recounted by a minimal header walk of its own
//!   ([`walk_file`]), against both documents.
//!
//! Failures name the document, the row (with its line) and the type. A campaign ticket
//! that adds a definition keeps this green by running `cargo xtask add-def` (which runs
//! `cargo xtask parity`), as `crates/schema/defs/README.md` ("Workflow") says.

// Test-only code: a panic fails the test. Clippy's `allow-*-in-tests` covers `#[test]`
// functions, not the helpers of an integration-test file (AGENTS.md, "Never panic").
#![allow(
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use kitkraft_schema::{Confidence, FieldType, Member, RecordDef, Schema};

const PARITY: &str = "docs/record-parity.md";
const GAP_MATRIX: &str = "docs/foothold/gap-matrix.md";
const MANIFEST: &str = "corpus/manifest.toml";
const RECORDS_RS: &str = "crates/esm/src/generated/records.rs";
const DEFS_DIR: &str = "crates/schema/defs";
const DATA_DIR_VAR: &str = "KITKRAFT_DATA_DIR";
/// What an inventory definition's `source` starts with (`cargo xtask inventory-def` writes
/// it; `crates/schema/tests/defs.rs` requires it of every inventory).
const INVENTORY_SOURCE: &str = "corpus: inventory_types_full";
/// What an lstring-only definition's `source` starts with (M3.B8).
const LSTRING_ONLY_SOURCE: &str = "corpus: lane E's corpus lstring table";
/// The shared groups an inventory types (`cargo xtask inventory-def`): the keyword list
/// and the base form component frame.
const INVENTORY_GROUPS: &[&str] = &["Keywords", "BfcbBlock"];

/// Backticked four-character names a record-family Type cell may hold that are not record
/// types, each for its reason. Anything else that is not a corpus type is a stale row.
const NOT_RECORD_TYPES: &[(&str, &str)] = &[
    ("GRUP", "the group header"),
    ("XXXX", "the oversize subrecord prefix"),
    ("VMAD", "the script subrecord (row `Script VMAD`)"),
    ("CTDA", "the condition subrecord (row `Conditions`)"),
];

/// The phrase by which the record-family section says what covers the types it has no
/// row for (its catch-all).
const CATCH_ALL: &str = "without a row here is covered by MX alone";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(rel: &str) -> String {
    let path = root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

// ---------------------------------------------------------------------------------------
// Markdown

/// The text of the `## <heading>` section, up to the next `## ` heading.
fn section<'a>(doc: &'a str, file: &str, heading: &str) -> &'a str {
    let marker = format!("\n## {heading}\n");
    let start = doc
        .find(&marker)
        .unwrap_or_else(|| panic!("{file}: no `## {heading}` section"))
        + 1;
    let rest = &doc[start..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |i| i + 3);
    &rest[..end]
}

/// One Markdown table row: its 1-based line in the file and its cells.
#[derive(Debug, Clone)]
struct Row {
    line: usize,
    cells: Vec<String>,
}

/// A table's cells, split on unescaped `|`, trimmed.
fn cells(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = line.trim().chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cell).trim().to_string()),
            _ => cell.push(c),
        }
    }
    // A row starts and ends with `|`: drop the empty piece before the first.
    if !out.is_empty() {
        out.remove(0);
    }
    out
}

/// The tables of `text` (which starts at 1-based line `first_line` of its file), each as
/// its header cells and body rows (the rule line dropped).
fn tables(text: &str, first_line: usize) -> Vec<(Vec<String>, Vec<Row>)> {
    let mut out: Vec<(Vec<String>, Vec<Row>)> = Vec::new();
    let mut current: Option<(Vec<String>, Vec<Row>)> = None;
    for (i, line) in text.lines().enumerate() {
        if line.starts_with('|') {
            let c = cells(line);
            match current.as_mut() {
                None => current = Some((c, Vec::new())),
                Some((_, rows)) => {
                    if !c.iter().all(|x| x.chars().all(|ch| ch == '-' || ch == ':')) {
                        rows.push(Row {
                            line: first_line + i,
                            cells: c,
                        });
                    }
                }
            }
        } else if let Some(t) = current.take() {
            out.push(t);
        }
    }
    out.extend(current);
    out
}

/// The 1-based line `text` (a slice of `doc`) starts at.
fn line_of(doc: &str, text: &str) -> usize {
    let offset = text.as_ptr() as usize - doc.as_ptr() as usize;
    doc[..offset].matches('\n').count() + 1
}

/// `1,234` -> 1234.
fn num(cell: &str, what: &str) -> u64 {
    cell.trim()
        .replace(',', "")
        .parse()
        .unwrap_or_else(|_| panic!("{what}: `{cell}` is not a number"))
}

/// The leading number of `text` (`186 record types` -> 186).
fn lead_num(text: &str, what: &str) -> u64 {
    let digits: String = text
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .collect();
    num(&digits, what)
}

/// The text after the first `marker` in `text`.
fn after<'a>(text: &'a str, marker: &str, what: &str) -> &'a str {
    let at = text
        .find(marker)
        .unwrap_or_else(|| panic!("{what}: no `{marker}`"));
    &text[at + marker.len()..]
}

/// The first backticked span of `text`.
fn first_tick(text: &str, what: &str) -> String {
    let rest = after(text, "`", what);
    let end = rest
        .find('`')
        .unwrap_or_else(|| panic!("{what}: unclosed backtick"));
    rest[..end].to_string()
}

/// Every backticked span of `text`.
fn ticks(text: &str) -> Vec<String> {
    text.split('`')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect()
}

/// A backticked four-character signature (`` `NPC_` `` -> `NPC_`).
fn is_sig(s: &str) -> bool {
    s.len() == 4
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

// ---------------------------------------------------------------------------------------
// The two documents

/// One row of the parity document's "Record types" table.
#[derive(Debug, Clone, Default)]
struct TypeRow {
    line: usize,
    schema: String,
    records: u64,
    opaque: u64,
    unknown: u64,
}

/// What the parity document says.
#[derive(Debug, Default)]
struct Parity {
    types: BTreeMap<String, TypeRow>,
    total_types: u64,
    total_defined: u64,
    total_records: u64,
    families_types: u64,
    families_definitions: u64,
    families_records: u64,
    corpus_files: u64,
    manifest_hash: String,
    definitions: u64,
    defs_hash: String,
    inventory: Vec<String>,
    /// Per defined corpus type: Schema, Failed, Re-encode.
    decoded: BTreeMap<String, (String, u64, u64)>,
    /// Per defined type: Schema, reference, container-reference.
    confidence: BTreeMap<String, (String, u64, u64)>,
    /// Per corpus file: records.
    files: Vec<(String, u64)>,
}

/// The column of `head` named `name`.
fn col(head: &[String], name: &str, what: &str) -> usize {
    head.iter()
        .position(|h| h == name)
        .unwrap_or_else(|| panic!("{what}: no `{name}` column in {head:?}"))
}

fn parity_doc() -> Parity {
    let doc = read(PARITY);
    let what = PARITY;

    // Families: the total row.
    let fam = section(&doc, what, "Families");
    let (head, rows) = tables(fam, line_of(&doc, fam))
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{what}: Families has no table"));
    let total = rows
        .iter()
        .find(|r| r.cells[0] == "**Total**")
        .unwrap_or_else(|| panic!("{what}: Families has no total row"));
    let families_types = num(&total.cells[col(&head, "Record types", what)], what);
    let families_definitions = num(&total.cells[col(&head, "Definitions", what)], what);
    let families_records = num(&total.cells[col(&head, "Records", what)], what);
    let inv_line = fam
        .lines()
        .find(|l| l.starts_with("Inventory definitions ("))
        .unwrap_or_else(|| panic!("{what}: Families names no inventory definitions"));
    let listed = after(after(inv_line, "*)", what), ": ", what).trim_end_matches('.');
    let inventory: Vec<String> = listed
        .split(", ")
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    // Corpus and method.
    let method = section(&doc, what, "Corpus and method");
    let corpus_files = lead_num(after(method, "- Corpus: the ", what), what);
    let manifest_hash = first_tick(after(method, "manifest hash", what), what);
    let schema_line = after(method, "- Schema: ", what);
    let definitions = lead_num(schema_line, what);
    let defs_hash = first_tick(after(schema_line, "defs hash", what), what);

    // Record types.
    let rt = section(&doc, what, "Record types");
    let (head, rows) = tables(rt, line_of(&doc, rt))
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{what}: Record types has no table"));
    let (c_schema, c_records, c_opaque, c_unknown) = (
        col(&head, "Schema", what),
        col(&head, "Records", what),
        col(&head, "Opaque", what),
        col(&head, "Unknown", what),
    );
    let mut types = BTreeMap::new();
    let mut totals = None;
    for r in &rows {
        let first = &r.cells[0];
        if let Some(n) = first.strip_prefix("**") {
            let defined = r.cells[c_schema].trim_end_matches(" defined");
            totals = Some((
                lead_num(n, what),
                num(defined, what),
                num(&r.cells[c_records], what),
            ));
            continue;
        }
        let sig = first.trim_matches('`').to_string();
        assert!(
            types
                .insert(
                    sig.clone(),
                    TypeRow {
                        line: r.line,
                        schema: r.cells[c_schema].clone(),
                        records: num(&r.cells[c_records], what),
                        opaque: num(&r.cells[c_opaque], what),
                        unknown: num(&r.cells[c_unknown], what),
                    },
                )
                .is_none(),
            "{what}:{}: `{sig}` has two rows",
            r.line
        );
    }
    let (total_types, total_defined, total_records) =
        totals.unwrap_or_else(|| panic!("{what}: Record types has no total row"));

    // Decoded through the definitions.
    let dec = section(&doc, what, "Decoded through the definitions");
    let (head, rows) = tables(dec, line_of(&doc, dec)).into_iter().next().unwrap();
    let (c_s, c_f, c_r) = (
        col(&head, "Schema", what),
        col(&head, "Failed", what),
        col(&head, "Re-encode", what),
    );
    let decoded = rows
        .iter()
        .filter(|r| r.cells[0] != "**Total**")
        .map(|r| {
            (
                r.cells[0].trim_matches('`').to_string(),
                (
                    r.cells[c_s].clone(),
                    num(&r.cells[c_f], what),
                    num(&r.cells[c_r], what),
                ),
            )
        })
        .collect();

    // Definitions by confidence.
    let conf = section(&doc, what, "Definitions by confidence");
    let (head, rows) = tables(conf, line_of(&doc, conf))
        .into_iter()
        .next()
        .unwrap();
    let (c_s, c_ref, c_cref) = (
        col(&head, "Schema", what),
        col(&head, "reference", what),
        col(&head, "container-reference", what),
    );
    let confidence = rows
        .iter()
        .filter(|r| r.cells[0] != "**Total**")
        .map(|r| {
            (
                r.cells[0].trim_matches('`').to_string(),
                (
                    r.cells[c_s].clone(),
                    num(&r.cells[c_ref], what),
                    num(&r.cells[c_cref], what),
                ),
            )
        })
        .collect();

    // Roundtrip.
    let rtp = section(&doc, what, "Roundtrip");
    let (head, rows) = tables(rtp, line_of(&doc, rtp)).into_iter().next().unwrap();
    let c_rec = col(&head, "Records", what);
    let files = rows
        .iter()
        .map(|r| (r.cells[0].clone(), num(&r.cells[c_rec], what)))
        .collect();

    Parity {
        types,
        total_types,
        total_defined,
        total_records,
        families_types,
        families_definitions,
        families_records,
        corpus_files,
        manifest_hash,
        definitions,
        defs_hash,
        inventory,
        decoded,
        confidence,
        files,
    }
}

/// What the gap matrix's generated headline says.
#[derive(Debug)]
struct Headline {
    /// MX rows, those with no corpus record among them.
    types: u64,
    empty: u64,
    records: u64,
    files: u64,
    /// Types MX leaves out, with the records the headline says they have.
    excluded: BTreeMap<String, u64>,
    /// The parity totals the headline quotes when something is excluded.
    with_them: Option<(u64, u64)>,
    manifest_hash: String,
    definitions: u64,
    defs_hash: String,
}

const HEAD_BEGIN: &str = "<!-- parity:headline:begin -->\n";
const HEAD_END: &str = "<!-- parity:headline:end -->\n";

fn headline(doc: &str) -> Headline {
    let what = &format!("{GAP_MATRIX} (headline)");
    let block = after(doc, HEAD_BEGIN, what);
    let block = &block[..block
        .find(HEAD_END)
        .unwrap_or_else(|| panic!("{what}: no end marker"))];
    let counts = after(block, "`kit coverage --matrix` (MX), ", what);
    let types = lead_num(counts, what);
    let empty = counts
        .lines()
        .next()
        .and_then(|l| l.split(" (").nth(1))
        .filter(|s| s.contains("defined with no corpus record"))
        .map_or(0, |s| lead_num(s, what));
    let records = lead_num(
        after(counts, "record types", what)
            .split(", ")
            .nth(1)
            .unwrap(),
        what,
    );
    let files = lead_num(after(counts, " records, ", what), what);
    let excl_line = after(block, "Excluded from MX: ", what)
        .lines()
        .next()
        .unwrap();
    let mut excluded = BTreeMap::new();
    if excl_line != "none." {
        for entry in excl_line.trim_end_matches('.').split("; ") {
            let sig = first_tick(entry, what);
            let n = lead_num(after(entry, "`, ", what), what);
            assert!(
                excluded.insert(sig.clone(), n).is_none(),
                "{what}: `{sig}` excluded twice"
            );
        }
    }
    let with_them = block.find("With them, ").map(|at| {
        let w = after(&block[at..], " counts ", what);
        (
            lead_num(w, what),
            lead_num(after(w, " record types and ", what), what),
        )
    });
    let inputs = after(block, "Generated by `cargo xtask parity`", what);
    Headline {
        types,
        empty,
        records,
        files,
        excluded,
        with_them,
        manifest_hash: first_tick(after(inputs, "manifest hash", what), what),
        definitions: lead_num(after(inputs, "the schema's ", what), what),
        defs_hash: first_tick(after(inputs, "defs hash", what), what),
    }
}

// ---------------------------------------------------------------------------------------
// Independent recomputation

fn fnv1a64(parts: &[&[u8]]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for &b in *part {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// The defs hash: every `BUILTIN_DEFINITIONS` file name and its LF text, NUL after each,
/// in list order (the fingerprint `crates/esm/src/generated/records.rs` names).
fn defs_hash() -> String {
    let mut owned: Vec<Vec<u8>> = Vec::new();
    for (name, text) in kitkraft_schema::BUILTIN_DEFINITIONS {
        owned.push(name.as_bytes().to_vec());
        owned.push(vec![0]);
        owned.push(text.replace("\r\n", "\n").into_bytes());
        owned.push(vec![0]);
    }
    let parts: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
    format!("fnv1a64:{:016x}", fnv1a64(&parts))
}

fn schema() -> &'static Schema {
    kitkraft_schema::builtin().expect("the builtin schema loads")
}

/// Whether `rec` is a minimal definition, the kind the matrix's Typed-readable calls `no`:
/// an inventory (`cargo xtask inventory-def`) or lstring-only one by its `source`, that
/// still types nothing beyond EDID, lstrings, the keyword list and the component frame
/// ([`beyond_minimal`]). An inventory a later round typed more of (DOOR's and LIGH's
/// `OBND`, fix round W3) is not minimal any more; neither is a definition transcribed from
/// xEdit that keeps its payloads opaque (NAVM).
fn minimal(rec: &RecordDef) -> bool {
    let source = rec.provenance.source.as_str();
    (source.starts_with(INVENTORY_SOURCE) || source.starts_with(LSTRING_ONLY_SOURCE))
        && beyond_minimal(rec).is_none()
}

/// The first member path of `rec` that types more than an inventory does (EDID, lstrings,
/// the [`INVENTORY_GROUPS`]), or `None`.
fn beyond_minimal(rec: &RecordDef) -> Option<String> {
    fn lstring(ty: &FieldType) -> bool {
        match ty {
            FieldType::LString => true,
            FieldType::Ref { ty, .. } => lstring(ty),
            _ => false,
        }
    }
    fn walk(members: &[Member], prefix: &str) -> Option<String> {
        for m in members {
            let path = if prefix.is_empty() {
                m.name().to_string()
            } else {
                format!("{prefix}.{}", m.name())
            };
            match m {
                Member::Subrecord(s) => {
                    let plain = s.ty.is_opaque()
                        || matches!(s.ty, FieldType::Empty)
                        || s.signature.as_str() == "EDID"
                        || lstring(&s.ty);
                    if !plain {
                        return Some(format!("{path} ({})", s.signature.as_str()));
                    }
                }
                Member::Group(g)
                    if g.shared
                        .as_deref()
                        .is_some_and(|n| INVENTORY_GROUPS.contains(&n)) => {}
                Member::Group(g) => {
                    if let Some(p) = walk(&g.members, &path) {
                        return Some(p);
                    }
                }
                Member::AnyUntil(_) => {}
            }
        }
        None
    }
    walk(&rec.members, "")
}

/// Subrecord definitions of `rec` (groups expanded) below `corpus` confidence:
/// (`reference`, `container-reference`).
fn below_corpus(rec: &RecordDef) -> (u64, u64) {
    let mut out = (0, 0);
    for (_, s) in rec.leaf_subrecords() {
        match s.provenance.confidence {
            Confidence::Reference => out.0 += 1,
            Confidence::ContainerReference => out.1 += 1,
            _ => {}
        }
    }
    out
}

fn record<'s>(schema: &'s Schema, sig: &str) -> Option<&'s RecordDef> {
    kitkraft_schema::Signature::parse(sig).and_then(|s| schema.record(s))
}

/// Collects failures and fails once, listing them all.
struct Failures(Vec<String>);

impl Failures {
    fn check(&mut self, ok: bool, msg: impl FnOnce() -> String) {
        if !ok {
            self.0.push(msg());
        }
    }
    fn finish(self, what: &str) {
        assert!(
            self.0.is_empty(),
            "{what}: {} failure(s):\n  {}",
            self.0.len(),
            self.0.join("\n  ")
        );
    }
}

// ---------------------------------------------------------------------------------------
// No corpus needed

/// The parity document against the schema: the definition count (three places), every
/// row's definition name, the defs hash, the inventory list and each type's confidence
/// counts. Stale after a definition change until `cargo xtask parity` (run by `add-def`).
#[test]
fn record_parity_matches_the_schema() {
    let p = parity_doc();
    let schema = schema();
    let mut f = Failures(Vec::new());
    let regen = "run `cargo xtask parity` (or `cargo xtask add-def <SIG>`, which runs it)";

    let defined: BTreeMap<String, &RecordDef> = schema
        .records()
        .map(|r| (r.signature.as_str().to_string(), r))
        .collect();
    // The files on disk are the ones BUILTIN_DEFINITIONS embeds.
    let on_disk: BTreeSet<String> = std::fs::read_dir(root().join(DEFS_DIR))
        .unwrap()
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            let sig = name.strip_suffix(".toml")?;
            is_sig(sig).then(|| sig.to_string())
        })
        .filter(|s| s != "GRUP")
        .collect();
    let keys: BTreeSet<String> = defined.keys().cloned().collect();
    f.check(on_disk == keys, || {
        format!(
            "{DEFS_DIR}: files without a builtin definition {:?}, builtin definitions without a \
             file {:?} (run `cargo xtask add-def`)",
            on_disk.difference(&keys).collect::<Vec<_>>(),
            keys.difference(&on_disk).collect::<Vec<_>>()
        )
    });

    let n = defined.len() as u64;
    for (place, count) in [
        (
            "\"Corpus and method\" (Schema: N record definitions)",
            p.definitions,
        ),
        ("Families total Definitions", p.families_definitions),
    ] {
        f.check(count == n, || {
            format!("{PARITY} {place} says {count}, the schema has {n}; {regen}")
        });
    }
    let hash = defs_hash();
    f.check(p.defs_hash == hash, || {
        format!(
            "{PARITY} names defs hash `{}`, BUILTIN_DEFINITIONS hashes to `{hash}`; {regen}",
            p.defs_hash
        )
    });
    let rs = read(RECORDS_RS);
    f.check(rs.contains(&format!("Schema defs hash: {hash}")), || {
        format!("{RECORDS_RS} does not name defs hash `{hash}` (run `cargo xtask codegen`)")
    });

    // Record types: each row's Schema is its definition's name, `-` without one.
    let mut with_def = 0;
    for (sig, row) in &p.types {
        let want = defined.get(sig).map_or("-", |r| r.name.as_str());
        if want != "-" {
            with_def += 1;
        }
        f.check(row.schema == want, || {
            format!(
                "{PARITY}:{}: `{sig}` Schema `{}`, the schema says `{want}`; {regen}",
                row.line, row.schema
            )
        });
    }
    f.check(p.total_defined == with_def, || {
        format!(
            "{PARITY} Record types total says {} defined, {with_def} rows name a definition",
            p.total_defined
        )
    });

    // Decoded: one row per defined corpus type, with its definition's name.
    let decoded_want: BTreeSet<&String> = p
        .types
        .keys()
        .filter(|s| defined.contains_key(*s))
        .collect();
    let decoded_have: BTreeSet<&String> = p.decoded.keys().collect();
    f.check(decoded_want == decoded_have, || {
        format!(
            "{PARITY} \"Decoded through the definitions\" rows {decoded_have:?}, the defined \
             corpus types are {decoded_want:?}"
        )
    });
    for (sig, (name, _, _)) in &p.decoded {
        let want = defined.get(sig).map_or("-", |r| r.name.as_str());
        f.check(name == want, || {
            format!("{PARITY} \"Decoded\": `{sig}` Schema `{name}`, the schema says `{want}`")
        });
    }

    // Definitions by confidence: one row per definition, reference counts as the schema's.
    let conf_have: BTreeSet<&String> = p.confidence.keys().collect();
    let conf_want: BTreeSet<&String> = defined.keys().collect();
    f.check(conf_have == conf_want, || {
        format!(
            "{PARITY} \"Definitions by confidence\" rows differ from the schema: missing {:?}, \
             extra {:?}; {regen}",
            conf_want.difference(&conf_have).collect::<Vec<_>>(),
            conf_have.difference(&conf_want).collect::<Vec<_>>()
        )
    });
    for (sig, (name, reference, container)) in &p.confidence {
        let Some(rec) = defined.get(sig) else {
            continue;
        };
        f.check(name == &rec.name, || {
            format!(
                "{PARITY} \"Definitions by confidence\": `{sig}` Schema `{name}`, the schema \
                 says `{}`",
                rec.name
            )
        });
        let (r, c) = below_corpus(rec);
        f.check((*reference, *container) == (r, c), || {
            format!(
                "{PARITY} \"Definitions by confidence\": `{sig}` reference {reference}, \
                 container-reference {container}; the schema has {r} and {c}; {regen}"
            )
        });
    }

    // The inventory list is the definitions an inventory source makes.
    let inv_want: Vec<String> = defined
        .iter()
        .filter(|(_, r)| r.provenance.source.starts_with(INVENTORY_SOURCE))
        .map(|(s, _)| s.clone())
        .collect();
    let mut inv_have = p.inventory.clone();
    inv_have.sort();
    f.check(inv_have == inv_want, || {
        format!(
            "{PARITY} lists the inventory definitions {inv_have:?}, the schema's are \
             {inv_want:?}; {regen}"
        )
    });
    f.finish("record-parity vs schema");
}

/// The parity document's own totals and the manifest: Record types, Families and
/// Roundtrip agree, and the manifest hash and file count are the manifest's.
#[test]
fn record_parity_totals_agree() {
    let p = parity_doc();
    let mut f = Failures(Vec::new());
    let rows = p.types.len() as u64;
    let records: u64 = p.types.values().map(|t| t.records).sum();
    f.check(p.total_types == rows, || {
        format!(
            "{PARITY} Record types total says {} types, the table has {rows} rows",
            p.total_types
        )
    });
    f.check(p.total_records == records, || {
        format!(
            "{PARITY} Record types total says {} records, its rows sum to {records}",
            p.total_records
        )
    });
    f.check(
        (p.families_types, p.families_records) == (rows, records),
        || {
            format!(
                "{PARITY} Families total says {} types and {} records, Record types {rows} and \
                 {records}",
                p.families_types, p.families_records
            )
        },
    );
    let file_records: u64 = p.files.iter().map(|(_, r)| r).sum();
    f.check(file_records == records, || {
        format!("{PARITY} Roundtrip rows sum to {file_records} records, Record types to {records}")
    });

    let manifest = read(MANIFEST);
    let hash = format!("fnv1a64:{:016x}", fnv1a64(&[manifest.as_bytes()]));
    f.check(p.manifest_hash == hash, || {
        format!(
            "{PARITY} names manifest hash `{}`, {MANIFEST} hashes to `{hash}`",
            p.manifest_hash
        )
    });
    let names = manifest_files()
        .into_iter()
        .map(|(n, _)| n)
        .collect::<Vec<_>>();
    f.check(p.corpus_files == names.len() as u64, || {
        format!(
            "{PARITY} says the corpus has {} files, {MANIFEST} lists {}",
            p.corpus_files,
            names.len()
        )
    });
    let listed: Vec<String> = p.files.iter().map(|(n, _)| n.clone()).collect();
    f.check(listed == names, || {
        format!("{PARITY} Roundtrip lists {listed:?}, {MANIFEST} {names:?}")
    });
    f.finish("record-parity totals");
}

/// The gap matrix's generated headline against the parity document: the same files and
/// inputs, and the same record types and records but for exactly the exclusion it states
/// (each excluded type's records as the parity document counts them). Every excluded type
/// has a hand-kept family row, since MX does not cover it.
#[test]
fn gap_matrix_headline_reconciles_with_record_parity() {
    let p = parity_doc();
    let doc = read(GAP_MATRIX);
    let h = headline(&doc);
    let mut f = Failures(Vec::new());
    let what = format!("{GAP_MATRIX} headline");

    f.check(h.files == p.corpus_files, || {
        format!(
            "{what}: {} official files, {PARITY} {}",
            h.files, p.corpus_files
        )
    });
    f.check(h.manifest_hash == p.manifest_hash, || {
        format!(
            "{what}: manifest hash `{}`, {PARITY} `{}`",
            h.manifest_hash, p.manifest_hash
        )
    });
    f.check(
        (h.definitions, h.defs_hash.as_str()) == (p.definitions, p.defs_hash.as_str()),
        || {
            format!(
                "{what}: {} definitions, defs hash `{}`; {PARITY} {} and `{}` (run `cargo xtask \
                 parity`)",
                h.definitions, h.defs_hash, p.definitions, p.defs_hash
            )
        },
    );
    let mut excluded_records = 0;
    for (sig, n) in &h.excluded {
        match p.types.get(sig) {
            None => f.0.push(format!(
                "{what}: excludes `{sig}`, which {PARITY} does not count"
            )),
            Some(t) => {
                excluded_records += t.records;
                f.check(t.records == *n, || {
                    format!(
                        "{what}: excludes `{sig}` with {n} records, {PARITY} counts {}",
                        t.records
                    )
                });
            }
        }
    }
    let corpus_types = h.types - h.empty;
    f.check(
        corpus_types + h.excluded.len() as u64 == p.total_types,
        || {
            format!(
                "{what}: {corpus_types} record types with records + {} excluded, {PARITY} counts \
                 {}",
                h.excluded.len(),
                p.total_types
            )
        },
    );
    f.check(h.records + excluded_records == p.total_records, || {
        format!(
            "{what}: {} records + {excluded_records} excluded, {PARITY} counts {}",
            h.records, p.total_records
        )
    });
    match h.with_them {
        Some(totals) => f.check(totals == (p.total_types, p.total_records), || {
            format!(
                "{what}: quotes {PARITY} as {totals:?}, it says ({}, {})",
                p.total_types, p.total_records
            )
        }),
        None => f.check(h.excluded.is_empty(), || {
            format!("{what}: excludes types but does not quote the parity totals")
        }),
    }
    let rows = family_rows(&doc);
    for sig in h.excluded.keys() {
        f.check(rows.iter().any(|r| r.types.contains(sig)), || {
            format!(
                "{what}: `{sig}` is excluded from MX and has no record-family row, so nothing \
                 covers it"
            )
        });
    }
    f.finish("gap-matrix headline vs record-parity");
}

/// One row of the record-family table.
#[derive(Debug)]
struct FamilyRow {
    line: usize,
    label: String,
    /// The record types its Type cell names (corpus types and not).
    types: Vec<String>,
    cells: Vec<String>,
}

/// The status a cell starts with (`partial (...)` -> `partial`).
fn status(cell: &str) -> &str {
    let end = cell.find([' ', ',', '(']).unwrap_or(cell.len());
    &cell[..end]
}

fn family_rows(doc: &str) -> Vec<FamilyRow> {
    let sec = section(doc, GAP_MATRIX, "Record families");
    let (head, rows) = tables(sec, line_of(doc, sec))
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{GAP_MATRIX}: Record families has no table"));
    assert_eq!(
        head.get(..7).map(<[String]>::to_vec),
        Some(
            [
                "Type",
                "Parsed",
                "Typed-readable",
                "Typed-writable",
                "CK-diff",
                "Game",
                "Save/reload"
            ]
            .map(String::from)
            .to_vec()
        ),
        "{GAP_MATRIX}: the record-family columns changed; update this test"
    );
    rows.into_iter()
        .map(|r| FamilyRow {
            line: r.line,
            label: r.cells[0].clone(),
            types: ticks(&r.cells[0])
                .into_iter()
                .filter(|t| is_sig(t))
                .collect(),
            cells: r.cells,
        })
        .collect()
}

/// Why `cell` (a family row's Typed-readable) contradicts the generated data for the
/// row's record `types`, if it does. The matrix's own definitions ("Statuses"):
///
/// - `no`: an inventory or lstring-only definition, or none ([`minimal`]). Contradicted
///   when a type's definition is more.
/// - `yes`: every member at `corpus` confidence or better, every occurrence decodes, no
///   re-encode conflict. Contradicted by a minimal or missing definition, a member below
///   `corpus` (the schema), or a failed decode or re-encode conflict (`docs/record-parity.md`).
/// - `partial`: a definition that types more than a minimal one, and something short of
///   `yes`: a member below `corpus`, an opaque or unknown subrecord signature seen, a failed
///   decode or a conflict. Contradicted by a minimal or missing definition (that is `no`),
///   or when no type of the row has anything short of `yes`.
///
/// A row naming several types applies `yes` and `no` to each, `partial` to the row.
fn readable_violations(cell: &str, types: &[&str], schema: &Schema, p: &Parity) -> Vec<String> {
    let mut out = Vec::new();
    let mut short = Vec::new();
    for t in types {
        let row = &p.types[*t];
        let (failed, conflicts) = p.decoded.get(*t).map_or((0, 0), |d| (d.1, d.2));
        let rec = record(schema, t);
        let is_minimal = rec.is_none_or(minimal);
        let (reference, container) = rec.map_or((0, 0), below_corpus);
        // What rules out `yes`, and what else (opaque, unknown) makes a row `partial`.
        let mut not_yes = Vec::new();
        if reference + container > 0 {
            not_yes.push(format!("{} members below `corpus`", reference + container));
        }
        if failed > 0 {
            not_yes.push(format!("{failed} failed decodes"));
        }
        if conflicts > 0 {
            not_yes.push(format!("{conflicts} re-encode conflicts"));
        }
        let short_of_yes = !not_yes.is_empty() || row.opaque > 0 || row.unknown > 0;
        let kind = match rec {
            None => "has no definition".to_string(),
            Some(_) if is_minimal => {
                "has a minimal (inventory or lstring-only) definition".to_string()
            }
            Some(r) => match beyond_minimal(r) {
                Some(path) => format!("has a definition typing {path}"),
                None => format!("has a full definition (source `{}`)", r.provenance.source),
            },
        };
        match status(cell) {
            "no" if !is_minimal => out.push(format!(
                "says `no`, but `{t}` {kind}; the matrix's `no` is an inventory or lstring-only \
                 definition, or none"
            )),
            "yes" | "partial" if is_minimal => out.push(format!(
                "says `{}`, but `{t}` {kind}, which the matrix calls `no`",
                status(cell)
            )),
            "yes" if !not_yes.is_empty() => {
                out.push(format!("says `yes`, but `{t}` has {}", not_yes.join(", ")))
            }
            _ => {}
        }
        if short_of_yes {
            short.push(*t);
        }
    }
    if status(cell) == "partial" && short.is_empty() && out.is_empty() {
        out.push(format!(
            "says `partial`, but {} has every member at `corpus`, no opaque or unknown \
             signature, no failed decode and no conflict: that is `yes`",
            types
                .iter()
                .map(|t| format!("`{t}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !matches!(status(cell), "yes" | "partial" | "no") {
        out.push(format!(
            "Typed-readable `{cell}` is not yes, partial or no for a record type"
        ));
    }
    out
}

/// The hand-kept record-family rows against the generated data: each names corpus types
/// only (or a listed non-record name), its Typed-readable does not contradict the schema
/// and `docs/record-parity.md` ([`readable_violations`]), the section names its
/// catch-all for the types it has no row for, and the row tally it states is the table's.
#[test]
fn gap_matrix_family_rows_agree_with_the_generated_data() {
    let p = parity_doc();
    let doc = read(GAP_MATRIX);
    let schema = schema();
    let rows = family_rows(&doc);
    let mut f = Failures(Vec::new());

    let sec = section(&doc, GAP_MATRIX, "Record families");
    let flat = sec.split_whitespace().collect::<Vec<_>>().join(" ");
    f.check(flat.contains(CATCH_ALL), || {
        format!(
            "{GAP_MATRIX} \"Record families\" no longer says what covers the types without a \
             row (the phrase \"{CATCH_ALL}\")"
        )
    });

    let mut named = BTreeSet::new();
    for row in &rows {
        let at = format!("{GAP_MATRIX}:{} row `{}`", row.line, row.label);
        let mut record_types = Vec::new();
        for t in &row.types {
            if p.types.contains_key(t) {
                record_types.push(t.as_str());
                named.insert(t.clone());
            } else if !NOT_RECORD_TYPES.iter().any(|(s, _)| s == t) {
                f.0.push(format!(
                    "{at}: names `{t}`, which is not a record type of the corpus ({PARITY})"
                ));
            }
        }
        if record_types.is_empty() {
            continue;
        }
        for v in readable_violations(&row.cells[2], &record_types, schema, &p) {
            f.0.push(format!("{at}: Typed-readable {v}"));
        }
    }
    println!(
        "gap matrix: {} family rows name {} of {} corpus record types; the other {} are \
         covered by MX alone",
        rows.len(),
        named.len(),
        p.types.len(),
        p.types.len() - named.len()
    );

    // The tally the matrix states under its headline.
    let tally_line = doc
        .lines()
        .find(|l| l.contains("rows of the record-family table below"))
        .unwrap_or_else(|| panic!("{GAP_MATRIX}: no row tally line"));
    let tally_at = doc.lines().position(|l| l == tally_line).unwrap() + 1;
    let mut paragraph = String::new();
    for l in doc.lines().skip(tally_at - 1) {
        if l.is_empty() {
            break;
        }
        paragraph.push_str(l);
        paragraph.push(' ');
    }
    let stated_rows = lead_num(after(&paragraph, "The ", "tally"), "tally");
    f.check(stated_rows == rows.len() as u64, || {
        format!(
            "{GAP_MATRIX}:{tally_at}: says {stated_rows} rows, the table has {}",
            rows.len()
        )
    });
    let columns = after(&paragraph, "own columns: ", "tally")
        .trim()
        .trim_end_matches('.');
    for part in columns.split("; ") {
        let (column, values) = part.split_once(' ').unwrap();
        let i = 1 + [
            "Parsed",
            "Typed-readable",
            "Typed-writable",
            "CK-diff",
            "Game",
            "Save/reload",
        ]
        .iter()
        .position(|c| *c == column)
        .unwrap_or_else(|| panic!("{GAP_MATRIX}:{tally_at}: unknown column `{column}`"));
        let mut actual: BTreeMap<&str, u64> = BTreeMap::new();
        for r in &rows {
            *actual.entry(status(&r.cells[i])).or_default() += 1;
        }
        let mut stated: BTreeMap<&str, u64> = BTreeMap::new();
        for v in values.split(", ") {
            let (value, n) = v.rsplit_once(' ').unwrap();
            stated.insert(value, num(n, "tally"));
        }
        f.check(stated == actual, || {
            format!("{GAP_MATRIX}:{tally_at}: {column} tally {stated:?}, the table has {actual:?}")
        });
    }
    f.finish("gap-matrix family rows");
}

// ---------------------------------------------------------------------------------------
// Corpus

/// `corpus/manifest.toml`'s files: name and size, in order.
fn manifest_files() -> Vec<(String, u64)> {
    let table: toml::Table = read(MANIFEST).parse().expect("the manifest parses");
    table["file"]
        .as_array()
        .expect("[[file]] entries")
        .iter()
        .map(|f| {
            (
                f["name"].as_str().unwrap().to_string(),
                u64::try_from(f["size"].as_integer().unwrap()).unwrap(),
            )
        })
        .collect()
}

/// Records per type in one plugin, by a minimal walk of its own: 24-byte record and group
/// headers (signature, then a `u32` size: the payload's for a record, the whole group's
/// for `GRUP`), stepping into groups and over record payloads, checking every record and
/// group ends inside its enclosing group and the walk ends exactly at the end of the file.
/// Nothing is decompressed or decoded: this is not `kitkraft_esm`'s parser.
fn walk_file(path: &Path) -> BTreeMap<String, u64> {
    const HEADER: u64 = 24;
    let file = File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let len = file.metadata().unwrap().len();
    let mut reader = BufReader::with_capacity(1 << 20, file);
    let mut counts: BTreeMap<[u8; 4], u64> = BTreeMap::new();
    let mut ends: Vec<u64> = Vec::new();
    let mut pos = 0u64;
    let mut head = [0u8; 24];
    while pos < len {
        while ends.last() == Some(&pos) {
            ends.pop();
        }
        reader
            .read_exact(&mut head)
            .unwrap_or_else(|e| panic!("{} @{pos}: {e}", path.display()));
        let sig: [u8; 4] = head[..4].try_into().unwrap();
        let size = u64::from(u32::from_le_bytes(head[4..8].try_into().unwrap()));
        // Where this record or group ends: inside its enclosing group, inside the file.
        let end = if &sig == b"GRUP" {
            assert!(
                size >= HEADER,
                "{} @{pos}: group size {size}",
                path.display()
            );
            pos + size
        } else {
            pos + HEADER + size
        };
        let outer = ends.last().copied().unwrap_or(len);
        assert!(
            end <= outer,
            "{} @{pos}: `{}` ends at {end}, past its group or the file ({outer})",
            path.display(),
            String::from_utf8_lossy(&sig)
        );
        let next = if &sig == b"GRUP" {
            ends.push(end);
            pos + HEADER
        } else {
            *counts.entry(sig).or_default() += 1;
            reader.seek_relative(i64::try_from(size).unwrap()).unwrap();
            end
        };
        pos = next;
    }
    ends.retain(|e| *e != len);
    assert!(
        pos == len && ends.is_empty(),
        "{}: the walk ends at {pos} of {len} with open groups {ends:?}",
        path.display()
    );
    counts
        .into_iter()
        .map(|(s, n)| (String::from_utf8_lossy(&s).into_owned(), n))
        .collect()
}

/// The corpus recounted (records per type and per file) against both documents: every
/// type's records in `docs/record-parity.md`, its totals and per-file records, and the
/// gap matrix headline's counts plus exactly the exclusion it states. Needs
/// `KITKRAFT_DATA_DIR` (read only); skips with one line without it.
#[test]
fn corpus_counts_match_both_documents() {
    let test = "corpus_counts_match_both_documents";
    let Some(dir) = std::env::var_os(DATA_DIR_VAR).map(PathBuf::from) else {
        println!("skip {test}: {DATA_DIR_VAR} is not set (see docs/corpus.md)");
        return;
    };
    if !dir.is_dir() {
        println!(
            "skip {test}: {DATA_DIR_VAR}={} is not a directory (see docs/corpus.md)",
            dir.display()
        );
        return;
    }
    let manifest = manifest_files();
    for (name, size) in &manifest {
        let path = dir.join(name);
        let actual = std::fs::metadata(&path)
            .unwrap_or_else(|e| {
                panic!(
                    "corpus changed: {} is missing ({e}); update {MANIFEST}",
                    path.display()
                )
            })
            .len();
        assert!(
            actual == *size,
            "corpus changed: {name} is {actual} bytes, {MANIFEST} says {size}"
        );
    }
    let start = std::time::Instant::now();
    let per_file: Vec<(String, BTreeMap<String, u64>)> = std::thread::scope(|s| {
        let handles: Vec<_> = manifest
            .iter()
            .map(|(name, _)| {
                let path = dir.join(name);
                let name = name.clone();
                s.spawn(move || (name, walk_file(&path)))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut walk: BTreeMap<String, u64> = BTreeMap::new();
    for (_, counts) in &per_file {
        for (t, n) in counts {
            *walk.entry(t.clone()).or_default() += n;
        }
    }
    let walked: u64 = walk.values().sum();
    println!(
        "{test}: {} files, {} record types, {walked} records walked in {:.1} s",
        per_file.len(),
        walk.len(),
        start.elapsed().as_secs_f64()
    );

    let p = parity_doc();
    let doc = read(GAP_MATRIX);
    let h = headline(&doc);
    let mut f = Failures(Vec::new());
    let all: BTreeSet<&String> = walk.keys().chain(p.types.keys()).collect();
    for t in all {
        let w = walk.get(t).copied().unwrap_or(0);
        match p.types.get(t) {
            Some(row) => f.check(row.records == w, || {
                format!(
                    "{PARITY}:{}: `{t}` {} records, the corpus has {w}",
                    row.line, row.records
                )
            }),
            None => f.0.push(format!(
                "{PARITY}: no row for `{t}`, which the corpus has {w} records of"
            )),
        }
    }
    f.check(
        (p.total_types, p.total_records) == (walk.len() as u64, walked),
        || {
            format!(
                "{PARITY}: {} record types and {} records, the corpus {} and {walked}",
                p.total_types,
                p.total_records,
                walk.len()
            )
        },
    );
    for (name, counts) in &per_file {
        let w: u64 = counts.values().sum();
        match p.files.iter().find(|(n, _)| n == name) {
            Some((_, r)) => f.check(*r == w, || {
                format!("{PARITY} Roundtrip: {name} {r} records, the file has {w}")
            }),
            None => f.0.push(format!("{PARITY} Roundtrip: no row for {name}")),
        }
    }

    // The headline: the corpus less exactly the stated exclusion.
    let what = format!("{GAP_MATRIX} headline");
    let mut excluded_records = 0;
    for (t, n) in &h.excluded {
        let w = walk.get(t).copied().unwrap_or(0);
        excluded_records += w;
        f.check(*n == w, || {
            format!("{what}: excludes `{t}` with {n} records, the corpus has {w}")
        });
    }
    f.check(
        h.types - h.empty + h.excluded.len() as u64 == walk.len() as u64,
        || {
            format!(
                "{what}: {} record types with records + {} excluded, the corpus has {}",
                h.types - h.empty,
                h.excluded.len(),
                walk.len()
            )
        },
    );
    f.check(h.records + excluded_records == walked, || {
        format!(
            "{what}: {} records + {excluded_records} excluded, the corpus has {walked}",
            h.records
        )
    });
    f.check(h.files == manifest.len() as u64, || {
        format!(
            "{what}: {} official files, {MANIFEST} lists {}",
            h.files,
            manifest.len()
        )
    });
    f.finish("corpus vs record-parity and the gap matrix headline");
}

// ---------------------------------------------------------------------------------------
// The checks themselves: each catches what it is for

/// A synthetic schema: `TFUL` typed at `corpus`, `TREF` with a `reference` member, `TINV`
/// an inventory (EDID and an opaque payload), `TOBN` an inventory a later round typed a
/// member of.
fn synthetic_schema() -> Schema {
    let xedit = "xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L1-L2";
    let full = |sig: &str, extra: &str| {
        format!(
            "signature = \"{sig}\"\nname = \"T{sig}\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
             [[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\n\
             [[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"u32\"\n{extra}"
        )
    };
    let inventory = |sig: &str, data: &str| {
        format!(
            "signature = \"{sig}\"\nname = \"T{sig}\"\nsource = \"{INVENTORY_SOURCE} (t)\"\n\
             confidence = \"corpus\"\n\
             [[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\n\
             [[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"{data}\"\n"
        )
    };
    let reference = format!(
        "[[member]]\nsig = \"XNAM\"\nname = \"Extra2\"\ntype = \"u32\"\ncard = \"optional\"\n\
         source = \"{xedit}\"\nconfidence = \"reference\"\n"
    );
    let files = [
        ("TFUL.toml".to_string(), full("TFUL", "")),
        ("TREF.toml".to_string(), full("TREF", &reference)),
        ("TINV.toml".to_string(), inventory("TINV", "opaque")),
        ("TOBN.toml".to_string(), inventory("TOBN", "u32")),
    ];
    kitkraft_schema::load_sources(files.iter().map(|(n, t)| (n.as_str(), t.as_str()))).unwrap()
}

/// [`readable_violations`] flags each contradiction the matrix's definitions name, and
/// passes the cells they allow.
#[test]
fn typed_readable_contradictions_are_caught() {
    let schema = synthetic_schema();
    let mut p = Parity::default();
    for (sig, opaque) in [
        ("TFUL", 0),
        ("TREF", 0),
        ("TINV", 1),
        ("TOBN", 0),
        ("TNON", 0),
    ] {
        p.types.insert(
            sig.to_string(),
            TypeRow {
                opaque,
                ..TypeRow::default()
            },
        );
    }
    let check =
        |cell: &str, types: &[&str], p: &Parity| readable_violations(cell, types, &schema, p);
    // Allowed.
    assert!(check("yes", &["TFUL"], &p).is_empty());
    assert!(check("partial (members)", &["TREF"], &p).is_empty());
    assert!(check("no", &["TINV"], &p).is_empty());
    assert!(check("no", &["TNON"], &p).is_empty());
    assert!(check("no", &["TINV", "TNON"], &p).is_empty());
    // An inventory with a typed member is not minimal: `partial` needs a shortfall.
    let v = check("partial", &["TOBN"], &p);
    assert!(v.len() == 1 && v[0].contains("that is `yes`"), "{v:?}");
    // Contradicted, each with the type and the reason.
    let v = check("yes", &["TINV"], &p);
    assert!(v[0].contains("`TINV`") && v[0].contains("minimal"), "{v:?}");
    let v = check("yes", &["TREF"], &p);
    assert!(
        v[0].contains("`TREF`") && v[0].contains("below `corpus`"),
        "{v:?}"
    );
    let v = check("no", &["TFUL"], &p);
    assert!(
        v[0].contains("`TFUL`") && v[0].contains("Data (DATA)"),
        "{v:?}"
    );
    let v = check("partial", &["TNON"], &p);
    assert!(v[0].contains("no definition"), "{v:?}");
    let v = check("partial", &["TFUL"], &p);
    assert!(v[0].contains("that is `yes`"), "{v:?}");
    let v = check("maybe", &["TFUL"], &p);
    assert!(
        v.iter().any(|x| x.contains("not yes, partial or no")),
        "{v:?}"
    );
    // A failed decode is short of `yes`.
    p.decoded.insert("TFUL".into(), ("TTFUL".into(), 1, 0));
    let v = check("yes", &["TFUL"], &p);
    assert!(v[0].contains("1 failed decodes"), "{v:?}");
}

/// The headline parser reads what the generator writes, with and without an exclusion.
#[test]
fn headline_is_read_with_and_without_an_exclusion() {
    let doc = format!(
        "x\n{HEAD_BEGIN}\n`kit coverage --matrix` (MX), 185 record types, 7,490,088 records, 14 \
         official files.\nExcluded from MX: `TES4`, 14 records (the plugin header, one per file, \
         whose row the record-family table keeps by hand).\nWith them, `docs/record-parity.md` \
         counts 186 record types and 7,490,102 records.\nGenerated by `cargo xtask parity` from \
         the files of `corpus/manifest.toml` (manifest hash `fnv1a64:1`), the schema's 135 \
         record definitions (defs hash `fnv1a64:2`) and `docs/schema/evidence.toml`.\n\n| t |\n\
         {HEAD_END}y\n"
    );
    let h = headline(&doc);
    assert_eq!(
        (h.types, h.empty, h.records, h.files),
        (185, 0, 7_490_088, 14)
    );
    assert_eq!(h.excluded, [("TES4".to_string(), 14)].into_iter().collect());
    assert_eq!(h.with_them, Some((186, 7_490_102)));
    assert_eq!(
        (
            h.manifest_hash.as_str(),
            h.definitions,
            h.defs_hash.as_str()
        ),
        ("fnv1a64:1", 135, "fnv1a64:2")
    );
    let excluded = "`TES4`, 14 records (the plugin header, one per file, whose row the \
                    record-family table keeps by hand).";
    let doc = doc
        .replace(
            "185 record types,",
            "187 record types (2 of them defined with no corpus record),",
        )
        .replace(excluded, "none.");
    let h = headline(&doc);
    assert_eq!((h.types, h.empty, h.records), (187, 2, 7_490_088));
    assert!(h.excluded.is_empty());
}

/// A scratch plugin under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str, bytes: &[u8]) -> Scratch {
        let path = std::env::temp_dir().join(format!(
            "kitkraft-reconcile-{tag}-{}.esm",
            std::process::id()
        ));
        std::fs::write(&path, bytes).unwrap();
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A 24-byte record or group header.
fn header(sig: &[u8; 4], size: u32) -> Vec<u8> {
    let mut out = sig.to_vec();
    out.extend(size.to_le_bytes());
    out.extend([0u8; 16]);
    out
}

/// The walk counts records inside nested groups and steps over payloads.
#[test]
fn the_walk_counts_records_in_nested_groups() {
    let mut bytes = header(b"TES4", 2);
    bytes.extend([1, 2]);
    let weap = |b: &mut Vec<u8>| {
        b.extend(header(b"WEAP", 3));
        b.extend([0, 0, 0]);
    };
    // A group of two WEAPs around a group of one STAT.
    let mut inner = header(b"GRUP", 48);
    inner.extend(header(b"STAT", 0));
    let mut outer = header(b"GRUP", 24 + 27 + 27 + u32::try_from(inner.len()).unwrap());
    weap(&mut outer);
    outer.extend(&inner);
    weap(&mut outer);
    bytes.extend(&outer);
    let f = Scratch::new("ok", &bytes);
    let want: BTreeMap<String, u64> = [("STAT", 1), ("TES4", 1), ("WEAP", 2)]
        .into_iter()
        .map(|(s, n)| (s.to_string(), n))
        .collect();
    assert_eq!(walk_file(&f.0), want);
}

/// A record that runs past its group fails the walk.
#[test]
#[should_panic(expected = "past its group")]
fn the_walk_fails_on_a_record_past_its_group() {
    let mut bytes = header(b"GRUP", 48);
    bytes.extend(header(b"WEAP", 4));
    bytes.extend([0u8; 4]);
    let f = Scratch::new("overrun", &bytes);
    walk_file(&f.0);
}
