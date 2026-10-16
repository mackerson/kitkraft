//! `coverage` with `matrix: true` (lane D, 2026-10-15): the support-depth matrix, one row
//! per record type with six statuses kept apart (owner ruling 2026-10-15, "Acceptance
//! criteria"; `docs/schema/support-matrix.md`):
//!
//! - **parsed**: every instance in the examined plugins parses (header, subrecords, a
//!   compressed body that decompresses);
//! - **typed-readable**: the type has a definition, and every instance decodes through the
//!   generated definition with nothing left to the unknown-bytes fallback: no subrecord the
//!   definition does not name (`unknown`), none that fails its layout (`failed`), and every
//!   record re-encodes identically. The share of payload bytes a layout types is given
//!   beside it (opaque members and opaque fields type none);
//! - **typed-writable**, **CK-differential-tested**, **game-runtime-tested**,
//!   **save/reload-verified**: read from the evidence file (`docs/schema/evidence.toml`),
//!   each entry a status with pointers to its proof (a test, a doc section, a ledger
//!   entry). A type the file names nothing for is `not-run`: schema presence is never
//!   counted as support.
//!
//! The examined plugins are `plugin`, `plugins`, or (neither given) the official files
//! (`kitkraft_io::plan::OFFICIAL_ORDER`) found in the Data dir. `types` limits the rows,
//! and the walk, to those record types.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::coverage::{TypeTotals, Walk};
use super::failed;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

const ID: &str = "coverage";

/// The evidence file, relative to the repository root.
pub(crate) const EVIDENCE_FILE: &str = "docs/schema/evidence.toml";

/// The four statuses the evidence file records, in matrix order.
pub(crate) const EVIDENCE_STATUSES: [&str; 4] = [
    "typed-writable",
    "ck-differential-tested",
    "game-runtime-tested",
    "save-reload-verified",
];

/// The results an evidence entry may record (AGENTS.md, "Reporting").
pub(crate) const RESULTS: [&str; 3] = ["pass", "fail", "inconclusive"];

/// One entry of the evidence file.
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    /// Record type (`WEAP`).
    #[serde(rename = "type")]
    pub ty: String,
    /// One of [`EVIDENCE_STATUSES`].
    pub status: String,
    /// One of [`RESULTS`].
    pub result: String,
    /// Pointers to the proof: `test: <file>::<fn>`, `doc: <file>#<heading>`,
    /// `ledger: <entry id>`.
    pub evidence: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceFile {
    #[serde(default)]
    entry: Vec<Entry>,
}

/// Parses the evidence file's text and checks its vocabulary.
pub(crate) fn parse_evidence(text: &str) -> Result<Vec<Entry>, String> {
    let file: EvidenceFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for (i, e) in file.entry.iter().enumerate() {
        let at = format!("entry[{i}] ({} {})", e.ty, e.status);
        if e.ty.len() != 4 || !e.ty.bytes().all(|b| (0x20..=0x7E).contains(&b)) {
            return Err(format!(
                "{at}: `type` must be a four-character record signature"
            ));
        }
        if !EVIDENCE_STATUSES.contains(&e.status.as_str()) {
            return Err(format!(
                "{at}: `status` must be one of {}",
                EVIDENCE_STATUSES.join(", ")
            ));
        }
        if !RESULTS.contains(&e.result.as_str()) {
            return Err(format!(
                "{at}: `result` must be one of {}",
                RESULTS.join(", ")
            ));
        }
        if e.evidence.is_empty() {
            return Err(format!("{at}: `evidence` names no proof"));
        }
        if !seen.insert((e.ty.clone(), e.status.clone())) {
            return Err(format!("{at}: a second entry for the same type and status"));
        }
    }
    Ok(file.entry)
}

/// Whether evidence pointer `pointer` resolves under the repository root `root`: a test
/// (`test: <file>::<fn name>`: the file defines `fn <name>(`), a doc section
/// (`doc: <file>#<text>`: a Markdown heading, or a line starting with `**`, begins with
/// the text), or an in-game or CK ledger entry (`ledger: <id>`: a heading of
/// `docs/game-observations.md` or `docs/ck-observations.md` starts with the id).
pub(crate) fn resolves(root: &Path, pointer: &str) -> Result<(), String> {
    let read =
        |file: &str| std::fs::read_to_string(root.join(file)).map_err(|e| format!("{file}: {e}"));
    if let Some(rest) = pointer.strip_prefix("test: ") {
        let (file, name) = rest
            .split_once("::")
            .ok_or_else(|| format!("`{pointer}`: expected test: <file>::<fn>"))?;
        let text = read(file)?;
        let wanted = format!("fn {name}(");
        return if text.contains(&wanted) {
            Ok(())
        } else {
            Err(format!("{file} defines no `fn {name}`"))
        };
    }
    if let Some(rest) = pointer.strip_prefix("doc: ") {
        let (file, section) = rest
            .split_once('#')
            .ok_or_else(|| format!("`{pointer}`: expected doc: <file>#<section>"))?;
        let text = read(file)?;
        let found = text.lines().any(|l| {
            let heading = l.trim_start_matches('#');
            (l.starts_with('#') && heading.trim_start().starts_with(section))
                || l.strip_prefix("**").is_some_and(|b| b.starts_with(section))
        });
        return if found {
            Ok(())
        } else {
            Err(format!("{file} has no heading `{section}`"))
        };
    }
    if let Some(id) = pointer.strip_prefix("ledger: ") {
        for file in ["docs/game-observations.md", "docs/ck-observations.md"] {
            if let Ok(text) = read(file) {
                let found = text.lines().any(|l| {
                    l.starts_with('#') && l.trim_start_matches('#').trim_start().starts_with(id)
                });
                if found {
                    return Ok(());
                }
            }
        }
        return Err(format!("no ledger entry `{id}`"));
    }
    Err(format!(
        "`{pointer}`: a pointer is test: <file>::<fn>, doc: <file>#<section> or ledger: <id>"
    ))
}

/// The evidence file to read: the `evidence` argument, else `docs/schema/evidence.toml`
/// above the working directory, else in the checkout `kit` was built from.
fn evidence_path(args: &Value) -> Option<PathBuf> {
    if let Some(p) = args.get("evidence").and_then(Value::as_str) {
        return Some(PathBuf::from(p));
    }
    let cwd = std::env::current_dir().ok();
    let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    cwd.iter()
        .flat_map(|d| d.ancestors().map(Path::to_path_buf).collect::<Vec<_>>())
        .chain(std::iter::once(built))
        .map(|d| d.join(EVIDENCE_FILE))
        .find(|p| p.is_file())
}

/// The repository root of an evidence file at `<root>/docs/schema/evidence.toml`.
fn root_of(path: &Path) -> PathBuf {
    path.parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// The plugins a matrix examines.
fn plugins(args: &Value) -> Result<(Vec<PathBuf>, String), EngineError> {
    if let Some(p) = args.get("plugin").and_then(Value::as_str) {
        return Ok((vec![PathBuf::from(p)], "plugin".into()));
    }
    if let Some(list) = args.get("plugins").and_then(Value::as_array) {
        let paths: Vec<PathBuf> = list
            .iter()
            .filter_map(Value::as_str)
            .map(PathBuf::from)
            .collect();
        return Ok((paths, "plugins".into()));
    }
    let (dir, from) = super::data_dir_source(args).ok_or_else(|| {
        failed(
            ID,
            "no plugin given and no Data dir (data_dir, KITKRAFT_DATA_DIR or kit doctor's detection) to find the official files in",
        )
    })?;
    let found: Vec<PathBuf> = kitkraft_io::plan::OFFICIAL_ORDER
        .iter()
        .map(|n| dir.join(n))
        .filter(|p| p.is_file())
        .collect();
    if found.is_empty() {
        return Err(failed(
            ID,
            format!("no official plugin in {} ({from})", dir.display()),
        ));
    }
    Ok((
        found,
        format!("the official files in {} ({from})", dir.display()),
    ))
}

fn sig_text(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

/// The parsed status of one type.
fn parsed(t: &TypeTotals) -> &'static str {
    if t.decode_failed > 0 || t.compressed_examined < t.compressed {
        "fail"
    } else {
        "pass"
    }
}

/// The typed-readable status of one type and why.
fn typed_readable(t: &TypeTotals) -> (&'static str, String) {
    if t.definition.is_none() {
        return ("fail", "no definition".into());
    }
    if t.typed_records < t.records.saturating_sub(t.decode_failed) {
        return (
            "fail",
            "some records did not decode through the definition".into(),
        );
    }
    let mut why = Vec::new();
    if t.unknown > 0 {
        why.push(format!("{} unknown subrecords", t.unknown));
    }
    if t.failed > 0 {
        why.push(format!("{} failed subrecords", t.failed));
    }
    if t.reencode_conflicts > 0 {
        why.push(format!("{} re-encode conflicts", t.reencode_conflicts));
    }
    if why.is_empty() {
        ("pass", String::new())
    } else {
        ("fail", why.join(", "))
    }
}

/// Typed payload bytes as a percentage with one decimal, or `None` without payload.
fn percent(typed: u64, total: u64) -> Option<f64> {
    if total == 0 {
        return None;
    }
    // Byte counts of the corpus fit f64 exactly far beyond 2^32.
    let (t, n) = (typed as f64, total as f64);
    Some((t * 1000.0 / n).round() / 10.0)
}

/// Counts one more `result` under `status`.
fn bump(counts: &mut BTreeMap<String, BTreeMap<String, u64>>, status: &str, result: &str) {
    let n = counts
        .entry(status.to_string())
        .or_default()
        .entry(result.to_string())
        .or_default();
    *n = n.saturating_add(1);
}

/// The handler (`coverage` with `matrix: true`).
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let schema = kitkraft_schema::builtin()
        .map_err(|e| failed(ID, format!("the builtin schema does not load: {e}")))?;
    let filter: Option<BTreeSet<[u8; 4]>> = args.get("types").and_then(Value::as_array).map(|l| {
        l.iter()
            .filter_map(Value::as_str)
            .filter_map(|s| <[u8; 4]>::try_from(s.as_bytes()).ok())
            .collect()
    });
    let (paths, source) = plugins(args)?;
    let mut walk = Walk::for_matrix(filter.clone(), true);
    let mut files = Vec::new();
    let mut evidence_out = Vec::new();
    for p in &paths {
        walk.file(p, schema).map_err(|e| failed(ID, e))?;
        let bytes = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let name = super::file_name(p);
        files.push(json!({"path": p.display().to_string(), "file": name, "bytes": bytes}));
        evidence_out.push(Evidence {
            file: name,
            offset: 0,
            length: bytes,
            record: None,
            subrecord: None,
        });
    }
    let mut limitations = vec![
        "parsed and typed-readable are measured on the examined plugins only; the other four statuses come from the evidence file and are not-run where it names nothing".to_string(),
        "typed bytes: payload bytes a layout types (a decoded payload less its opaque fields); opaque members, unknown and failed subrecords and raw component data type none".to_string(),
    ];
    let mut not_examined = Vec::new();
    // Evidence: the file, its entries by (type, status), each pointer checked.
    let ev_path = evidence_path(args);
    let mut by: BTreeMap<(String, String), (Entry, Vec<Value>)> = BTreeMap::new();
    let mut evidence_file = Value::Null;
    match &ev_path {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| failed(ID, format!("{}: {e}", path.display())))?;
            let entries = parse_evidence(&text)
                .map_err(|e| failed(ID, format!("{}: {e}", path.display())))?;
            let root = root_of(path);
            for e in entries {
                let pointers: Vec<Value> = e
                    .evidence
                    .iter()
                    .map(|p| {
                        let check = resolves(&root, p);
                        json!({"pointer": p, "resolves": check.is_ok(), "error": check.err()})
                    })
                    .collect();
                by.insert((e.ty.clone(), e.status.clone()), (e, pointers));
            }
            evidence_file = json!(path.display().to_string());
        }
        None => {
            not_examined.push(format!("{EVIDENCE_FILE}: not found (pass `evidence`)"));
            limitations.push(format!(
                "no evidence file ({EVIDENCE_FILE}): typed-writable, CK-differential-tested, game-runtime-tested and save/reload-verified are reported unknown"
            ));
        }
    }
    // Rows: every type the walk saw, every defined type, every type the evidence names.
    let mut totals: BTreeMap<[u8; 4], TypeTotals> = walk
        .totals()
        .into_iter()
        .map(|t| (t.signature, t))
        .collect();
    let mut sigs: BTreeSet<[u8; 4]> = totals.keys().copied().collect();
    sigs.extend(schema.records().map(|r| r.signature.as_bytes()));
    sigs.extend(
        by.keys()
            .filter_map(|(t, _)| <[u8; 4]>::try_from(t.as_bytes()).ok()),
    );
    sigs.remove(b"TES4");
    if let Some(f) = &filter {
        sigs.retain(|s| f.contains(s));
    }
    let mut rows = Vec::new();
    let mut table = vec![format!(
        "{:<4}  {:>9}  {:<6}  {:<14}  {:<14}  {:<22}  {:<19}  {:<20}",
        "type",
        "records",
        "parsed",
        "typed-readable",
        "typed-writable",
        "CK-differential-tested",
        "game-runtime-tested",
        "save/reload-verified"
    )];
    let mut counts: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for sig in sigs {
        let text = sig_text(sig);
        let t = totals.remove(&sig).unwrap_or_else(|| TypeTotals {
            signature: sig,
            definition: kitkraft_schema::Signature::from_bytes(sig)
                .and_then(|s| schema.record(s))
                .map(|r| r.name.clone()),
            ..TypeTotals::default()
        });
        let (p_status, (r_status, r_why)) = if t.records == 0 {
            (
                "not-run",
                ("not-run", "no instance in the examined plugins".to_string()),
            )
        } else {
            (parsed(&t), typed_readable(&t))
        };
        let pct = percent(t.typed_bytes, t.payload_bytes);
        let mut row = json!({
            "type": text,
            "definition": t.definition,
            "records": t.records,
            "parsed": {
                "status": p_status,
                "compressed": t.compressed,
                "compressed_examined": t.compressed_examined,
                "decode_failed": t.decode_failed,
            },
            "typed_readable": {
                "status": r_status,
                "why": if r_why.is_empty() { Value::Null } else { json!(r_why) },
                "typed_records": t.typed_records,
                "typed_bytes": t.typed_bytes,
                "payload_bytes": t.payload_bytes,
                "typed_bytes_percent": pct,
                "occurrences": {"typed": t.typed, "failed": t.failed, "opaque": t.opaque, "unknown": t.unknown},
                "reencode_conflicts": t.reencode_conflicts,
            },
        });
        let mut cells = Vec::new();
        for status in EVIDENCE_STATUSES {
            let key = status.replace('-', "_");
            let (result, pointers, note) = match by.get(&(text.clone(), status.to_string())) {
                Some((e, ptrs)) => {
                    let all = ptrs.iter().all(|p| p["resolves"] == json!(true));
                    // A result whose evidence does not resolve is inconclusive, never pass.
                    let r = if all {
                        e.result.clone()
                    } else {
                        "inconclusive".into()
                    };
                    (r, Value::Array(ptrs.clone()), e.note.clone())
                }
                None if ev_path.is_none() => ("unknown".into(), json!([]), None),
                None => ("not-run".into(), json!([]), None),
            };
            bump(&mut counts, status, &result);
            cells.push(result.clone());
            if let Some(obj) = row.as_object_mut() {
                obj.insert(
                    key,
                    json!({"status": result, "evidence": pointers, "note": note}),
                );
            }
        }
        bump(&mut counts, "parsed", p_status);
        bump(&mut counts, "typed-readable", r_status);
        let readable = match pct {
            Some(p) if t.records > 0 => format!("{r_status} {p:.1}%"),
            _ => r_status.to_string(),
        };
        table.push(format!(
            "{:<4}  {:>9}  {:<6}  {:<14}  {:<14}  {:<22}  {:<19}  {:<20}",
            text,
            t.records,
            p_status,
            readable,
            cells.first().map_or("", String::as_str),
            cells.get(1).map_or("", String::as_str),
            cells.get(2).map_or("", String::as_str),
            cells.get(3).map_or("", String::as_str),
        ));
        rows.push(row);
    }
    let summary = format!(
        "Support-depth matrix over {source} ({} files); typed-readable shows the typed share of payload bytes.\n{}",
        files.len(),
        table.join("\n")
    );
    let data = json!({
        "matrix": true,
        "source": source,
        "files": files,
        "evidence_file": evidence_file,
        "statuses": ["parsed", "typed-readable", "typed-writable", "ck-differential-tested", "game-runtime-tested", "save-reload-verified"],
        "counts": counts,
        "rows": rows,
        "summary": summary,
    });
    Ok(Outcome {
        data,
        coverage: Coverage {
            status: if not_examined.is_empty() {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined,
        },
        limitations,
        evidence: evidence_out,
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{parse_evidence, resolves, EVIDENCE_FILE};
    use crate::execute;
    use crate::handlers::testdata;
    use serde_json::json;
    use std::path::Path;

    /// The repository root of this checkout.
    fn root() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
    }

    /// Every pointer of the checked-in evidence file resolves: the tests it names exist,
    /// the doc sections and ledger entries are there (lane D, 2026-10-15).
    #[test]
    fn every_evidence_pointer_resolves() {
        let text = std::fs::read_to_string(root().join(EVIDENCE_FILE)).unwrap();
        let entries = parse_evidence(&text).unwrap();
        assert!(!entries.is_empty());
        let mut bad = Vec::new();
        for e in &entries {
            for p in &e.evidence {
                if let Err(why) = resolves(root(), p) {
                    bad.push(format!("{} {}: {p}: {why}", e.ty, e.status));
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[test]
    fn evidence_vocabulary_is_checked() {
        let one = |ty: &str, status: &str, result: &str, ev: &str| {
            format!("[[entry]]\ntype = \"{ty}\"\nstatus = \"{status}\"\nresult = \"{result}\"\nevidence = [{ev}]\n")
        };
        let ok = one("WEAP", "game-runtime-tested", "pass", "\"ledger: x\"");
        assert_eq!(parse_evidence(&ok).unwrap().len(), 1);
        for bad in [
            one("WEAPON", "game-runtime-tested", "pass", "\"ledger: x\""),
            one("WEAP", "runtime", "pass", "\"ledger: x\""),
            one("WEAP", "game-runtime-tested", "passed", "\"ledger: x\""),
            one("WEAP", "game-runtime-tested", "pass", ""),
            format!("{ok}{ok}"),
        ] {
            assert!(parse_evidence(&bad).is_err(), "{bad}");
        }
        assert!(resolves(
            root(),
            "test: crates/engine/src/handlers/matrix.rs::every_evidence_pointer_resolves"
        )
        .is_ok());
        assert!(resolves(
            root(),
            "test: crates/engine/src/handlers/matrix.rs::no_such_test"
        )
        .is_err());
        assert!(resolves(root(), "doc: docs/schema/support-matrix.md#Statuses").is_ok());
        assert!(resolves(root(), "doc: docs/schema/support-matrix.md#No such heading").is_err());
        assert!(resolves(root(), "ledger: game-20261014-01").is_ok());
        assert!(resolves(root(), "ledger: game-19990101-01").is_err());
        assert!(resolves(root(), "url: x").is_err());
    }

    /// The matrix through `execute`, on the synthetic plugin and a synthetic evidence file
    /// (one pointer resolving, one not).
    #[test]
    fn matrix_rows_keep_six_statuses_apart() {
        let bytes = testdata::plugin();
        let path = testdata::write_temp("coverage-matrix", &bytes);
        let dir = testdata::temp_dir("coverage-matrix-evidence");
        std::fs::create_dir_all(dir.join("docs/schema")).unwrap();
        std::fs::write(
            dir.join("docs/game-observations.md"),
            "# Ledger\n\n### game-20990101-01: a test entry\n",
        )
        .unwrap();
        let ev = dir.join(EVIDENCE_FILE);
        std::fs::write(
            &ev,
            "[[entry]]\ntype = \"WEAP\"\nstatus = \"game-runtime-tested\"\nresult = \"pass\"\nevidence = [\"ledger: game-20990101-01\"]\n\
             [[entry]]\ntype = \"GMST\"\nstatus = \"typed-writable\"\nresult = \"pass\"\nevidence = [\"test: nowhere.rs::nothing\"]\n",
        )
        .unwrap();
        let r = execute(
            "coverage",
            json!({"matrix": true, "plugin": path.to_str().unwrap(), "evidence": ev.to_str().unwrap()}),
        )
        .unwrap();
        let rows = r["data"]["rows"].as_array().unwrap();
        let row = |t: &str| rows.iter().find(|r| r["type"] == t).unwrap().clone();
        let weap = row("WEAP");
        assert_eq!(weap["records"], 2);
        assert_eq!(weap["parsed"]["status"], "pass");
        assert_eq!(weap["game_runtime_tested"]["status"], "pass");
        assert_eq!(weap["save_reload_verified"]["status"], "not-run");
        assert_eq!(weap["ck_differential_tested"]["status"], "not-run");
        // A pass whose pointer does not resolve is inconclusive.
        assert_eq!(row("GMST")["typed_writable"]["status"], "inconclusive");
        // A defined type with no instance: not-run, not a pass.
        let lctn = row("LCTN");
        assert_eq!(lctn["records"], 0);
        assert_eq!(lctn["parsed"]["status"], "not-run");
        assert_eq!(lctn["typed_readable"]["status"], "not-run");
        let summary = r["data"]["summary"].as_str().unwrap();
        assert!(summary.contains("typed-writable"), "{summary}");
        // `types` limits the rows.
        let r = execute(
            "coverage",
            json!({"matrix": true, "plugin": path.to_str().unwrap(), "evidence": ev.to_str().unwrap(), "types": ["WEAP"]}),
        )
        .unwrap();
        assert_eq!(r["data"]["rows"].as_array().unwrap().len(), 1);
        // Without the matrix, a plugin is required.
        assert!(execute("coverage", json!({})).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
