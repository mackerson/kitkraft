//! `cargo xtask session-evidence <report.json>` (KK-361): the `docs/schema/evidence.toml`
//! entries a game session's passing rows justify, printed on stdout for a person to review
//! and paste. It never writes `evidence.toml`.
//!
//! The report is the controller's `report.json` (`kitkraft_game::controller::Report`, read
//! here as plain JSON): `session`, `ledger_entry` and `rows`, each row with `row`, `title`,
//! `forms` and `verdict` (`{"status": "pass"}`, or `fail`, `inconclusive`, `not-run` with a
//! `reason`).
//!
//! Rules:
//! 1. A row's record type is the first four-character token of `A-Z`, `0-9` or `_` that is
//!    followed by a space and a backtick in its title (`kit testplan` titles every per-form
//!    row ``... <SIG> `<name>` ...``). The persistence rows are those whose titles start
//!    `persistence 1-3` and `persistence 4`. Rows with no type that are not persistence rows
//!    are ignored.
//! 2. `game-runtime-tested` for a type: at least one row of that type passed and none
//!    failed. Inconclusive and not-run rows do not block; the note lists them.
//! 3. `save-reload-verified` for a type: both persistence rows passed, a passing row of that
//!    type shares a form (an EditorID or runtime ID string) with the `persistence 4` row,
//!    and no row of that type failed.
//! 4. A type with a failing row gets no entry, only a TOML comment:
//!    `# <SIG>: row T-nn failed; no entry`.
//! 5. A report with no `ledger_entry` has nothing to cite: nothing is printed on stdout and
//!    the task fails with "the report has no ledger entry: nothing to cite".
//! 6. Output, sorted by type then status: `[[entry]]` with `type`, `status`,
//!    `result = "pass"`, `evidence = ["ledger: <ledger_entry>"]` and `note = "<session>:
//!    rows T-01, T-04 passed[; T-03 inconclusive] (proposed by cargo xtask
//!    session-evidence; review before adding)"`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

const RUNTIME: &str = "game-runtime-tested";
const RELOAD: &str = "save-reload-verified";

/// The task: reads `args[0]` (a `report.json`; a relative path that does not exist from
/// the working directory is tried from the workspace root) and prints the proposed
/// entries. Stdout carries only the entries (no "workspace root" line), so it can be
/// pasted as it is; on an error it carries nothing.
pub fn run(args: &[String]) -> Result<()> {
    let Some(arg) = args.first() else {
        bail!("usage: cargo xtask session-evidence <report.json>");
    };
    let given = PathBuf::from(arg);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent();
    let path = match root {
        Some(root) if !given.is_file() && given.is_relative() => root.join(&given),
        _ => given,
    };
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let report: Value =
        serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))?;
    let out = propose(&report).map_err(|e| anyhow!("{}: {e}", path.display()))?;
    print!("{out}");
    Ok(())
}

/// One row of the report, as far as the rules read it.
struct Row {
    id: String,
    status: String,
    forms: Vec<String>,
}

impl Row {
    fn passed(&self) -> bool {
        self.status == "pass"
    }
}

/// The entries and comments `report` justifies, as TOML text.
pub fn propose(report: &Value) -> Result<String, String> {
    let ledger = report
        .get("ledger_entry")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("the report has no ledger entry: nothing to cite")?;
    let session = report
        .get("session")
        .and_then(Value::as_str)
        .unwrap_or("unknown session");
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("the report has no `rows` array")?;

    let mut by_type: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    let mut persist_13: Vec<Row> = Vec::new();
    let mut persist_4: Vec<Row> = Vec::new();
    for v in rows {
        let title = v.get("title").and_then(Value::as_str).unwrap_or_default();
        let row = Row {
            id: v
                .get("row")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string(),
            status: v
                .get("verdict")
                .and_then(|x| x.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            forms: v
                .get("forms")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        };
        if title.starts_with("persistence 1-3") {
            persist_13.push(row);
        } else if title.starts_with("persistence 4") {
            persist_4.push(row);
        } else if let Some(sig) = record_type(title) {
            by_type.entry(sig).or_default().push(row);
        }
    }

    let persisted = !persist_13.is_empty()
        && !persist_4.is_empty()
        && persist_13.iter().chain(&persist_4).all(Row::passed);
    let persist_ids: Vec<&str> = persist_13
        .iter()
        .chain(&persist_4)
        .map(|r| r.id.as_str())
        .collect();

    let mut out = format!(
        "# Proposed by cargo xtask session-evidence from session {session} ({ledger}); review\n\
         # each entry before adding it to docs/schema/evidence.toml.\n"
    );
    for (sig, rows) in &by_type {
        let failed: Vec<&str> = rows
            .iter()
            .filter(|r| r.status == "fail")
            .map(|r| r.id.as_str())
            .collect();
        if !failed.is_empty() {
            let noun = if failed.len() == 1 { "row" } else { "rows" };
            out.push_str(&format!(
                "\n# {sig}: {noun} {} failed; no entry\n",
                failed.join(", ")
            ));
            continue;
        }
        let passed: Vec<&Row> = rows.iter().filter(|r| r.passed()).collect();
        if passed.is_empty() {
            continue;
        }
        let others = others(rows);
        let ids: Vec<&str> = passed.iter().map(|r| r.id.as_str()).collect();
        out.push_str(&entry(sig, RUNTIME, ledger, session, &ids, &others));
        if persisted {
            let shared: Vec<&str> = passed
                .iter()
                .filter(|r| {
                    r.forms
                        .iter()
                        .any(|f| persist_4.iter().any(|p| p.forms.contains(f)))
                })
                .map(|r| r.id.as_str())
                .collect();
            if !shared.is_empty() {
                let mut ids = shared;
                ids.extend(persist_ids.iter().copied());
                out.push_str(&entry(sig, RELOAD, ledger, session, &ids, &others));
            }
        }
    }
    Ok(out)
}

/// The rows of a type that neither passed nor failed, as `T-03 inconclusive`.
fn others(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter(|r| !r.passed() && r.status != "fail")
        .map(|r| format!("{} {}", r.id, r.status))
        .collect()
}

fn entry(
    sig: &str,
    status: &str,
    ledger: &str,
    session: &str,
    passed: &[&str],
    others: &[String],
) -> String {
    let mut note = format!("{session}: rows {} passed", passed.join(", "));
    for o in others {
        note.push_str("; ");
        note.push_str(o);
    }
    note.push_str(" (proposed by cargo xtask session-evidence; review before adding)");
    format!(
        "\n[[entry]]\ntype = {}\nstatus = {}\nresult = \"pass\"\nevidence = [{}]\nnote = {}\n",
        quote(sig),
        quote(status),
        quote(&format!("ledger: {ledger}")),
        quote(&note)
    )
}

/// A TOML basic string.
fn quote(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// Rule 1: the first four-character token of `A-Z`, `0-9` or `_` followed by a space and a
/// backtick.
fn record_type(title: &str) -> Option<String> {
    let token = |c: char| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_';
    let chars: Vec<char> = title.chars().collect();
    chars.windows(6).enumerate().find_map(|(i, w)| {
        let (sig, tail) = w.split_at(4);
        let bounded = i
            .checked_sub(1)
            .and_then(|j| chars.get(j))
            .is_none_or(|c| !token(*c));
        (bounded && sig.iter().all(|c| token(*c)) && tail == [' ', '`'])
            .then(|| sig.iter().collect())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/session-evidence/report.json"
    ));

    fn sample() -> Value {
        serde_json::from_str(SAMPLE).unwrap()
    }

    fn entries(out: &str) -> Vec<toml::Table> {
        let doc: toml::Table = toml::from_str(out).unwrap();
        doc.get("entry")
            .and_then(toml::Value::as_array)
            .unwrap()
            .iter()
            .map(|v| v.as_table().unwrap().clone())
            .collect()
    }

    fn get<'a>(t: &'a toml::Table, k: &str) -> &'a str {
        t.get(k).and_then(toml::Value::as_str).unwrap()
    }

    #[test]
    fn the_sample_report_proposes_the_entries_its_passing_rows_justify() {
        let out = propose(&sample()).unwrap();
        let list = entries(&out);
        let keys: Vec<(&str, &str)> = list
            .iter()
            .map(|t| (get(t, "type"), get(t, "status")))
            .collect();
        assert_eq!(
            keys,
            [("REFR", RUNTIME), ("REFR", RELOAD), ("WEAP", RUNTIME)],
            "{out}"
        );
        for t in &list {
            assert_eq!(get(t, "result"), "pass", "{out}");
            let evidence = t.get("evidence").and_then(toml::Value::as_array).unwrap();
            assert_eq!(evidence.len(), 1, "{out}");
            assert_eq!(evidence[0].as_str(), Some("ledger: game-20261101-99"));
        }
        let refr = get(&list[0], "note");
        assert!(
            refr.starts_with("2026-11-01-T99: rows T-01, T-02, T-04 passed; T-03 inconclusive"),
            "{refr}"
        );
        assert!(out.contains("# TMLM: row T-06 failed; no entry"), "{out}");
        assert!(!list.iter().any(|t| get(t, "type") == "TMLM"), "{out}");
    }

    #[test]
    fn the_proposed_entries_parse_as_evidence_entries() {
        let out = propose(&sample()).unwrap();
        let list = entries(&out);
        assert_eq!(list.len(), 3, "{out}");
        for t in &list {
            for k in ["type", "status", "result", "evidence", "note"] {
                assert!(t.contains_key(k), "{k} missing in {t:?}");
            }
        }
    }

    #[test]
    fn a_report_without_a_ledger_entry_proposes_nothing() {
        let mut report = sample();
        report["ledger_entry"] = Value::Null;
        let err = propose(&report).unwrap_err();
        assert!(err.contains("no ledger entry"), "{err}");
    }

    #[test]
    fn the_record_type_is_the_four_character_token_before_a_backtick() {
        assert_eq!(
            record_type("help \"Old Rifle\" 4: WEAP `OldRifle`").as_deref(),
            Some("WEAP")
        );
        assert_eq!(record_type("NPC_ `KC_Guard` here").as_deref(), Some("NPC_"));
        assert_eq!(record_type("a XREFR `KC_Crate`"), None);
        assert_eq!(record_type("persistence 4: return"), None);
    }
}
