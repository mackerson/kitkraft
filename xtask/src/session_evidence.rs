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
//!    are skipped, listed in a comment with the reason (rule 7); a type is never guessed.
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
//! 7. Rows that rule 1 cannot type (the bench verification mod's, `V-...`): the type is
//!    also read from the row id (a `-`-separated part that is a signature: `V-I-ARMO`,
//!    `V-S-I-MISC`), a reporter line (`KKV-P01 GLOB value=4242`, `KKV-H20 PKIN present`),
//!    `the <SIG> <name>` where the name is one of the row's forms (`the ACTI KKV10Acti is
//!    the base of ...`), and a trailing `(<SIG>)` (`after the reload: ... (ACTI)`). A
//!    signature is four characters of `A-Z`, `0-9`, `_`, starting with a letter. Every
//!    source that names a type must name the same one; none, or two that disagree, skips
//!    the row with that reason. A `(diagnostic)` row is skipped too.
//! 8. A row whose title says `presence only` (a `help` listing, a "present=True" reporter
//!    line) never counts as a pass: its `pass` is read as `inconclusive (presence only)`. A
//!    type whose rows are all inconclusive or not-run gets a comment
//!    `# <SIG>: no row passed (...); no entry`.
//! 9. `save-reload-verified` also holds for a type with a passing row whose title starts
//!    `after the reload` when the `persistence 1-3` row passed and no row of the type failed
//!    (the verification mod has no `persistence 4` row; its `V-S-*` rows check after the
//!    reload).

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
    /// The verdict's status; a presence-only pass is `inconclusive (presence only)` (rule 8).
    status: String,
    forms: Vec<String>,
    /// An `after the reload` row (rule 9).
    after_reload: bool,
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
    let mut skipped: Vec<String> = Vec::new();
    for v in rows {
        let title = v.get("title").and_then(Value::as_str).unwrap_or_default();
        let status = v
            .get("verdict")
            .and_then(|x| x.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let presence = title.to_ascii_lowercase().contains("presence only");
        let row = Row {
            id: v
                .get("row")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string(),
            status: if presence && status == "pass" {
                "inconclusive (presence only)".to_string()
            } else {
                status.to_string()
            },
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
            after_reload: title.starts_with("after the reload"),
        };
        if title.starts_with("persistence 1-3") {
            persist_13.push(row);
        } else if title.starts_with("persistence 4") {
            persist_4.push(row);
        } else if title.contains("(diagnostic)") {
            skipped.push(format!("{}: a diagnostic row", row.id));
        } else {
            match row_type(&row.id, title, &row.forms) {
                Ok(sig) => by_type.entry(sig).or_default().push(row),
                Err(why) => skipped.push(format!("{}: {why}", row.id)),
            }
        }
    }

    let persisted_13 = !persist_13.is_empty() && persist_13.iter().all(Row::passed);
    let persisted = persisted_13 && !persist_4.is_empty() && persist_4.iter().all(Row::passed);
    let ids_13: Vec<&str> = persist_13.iter().map(|r| r.id.as_str()).collect();
    let ids_4: Vec<&str> = persist_4.iter().map(|r| r.id.as_str()).collect();

    let mut out = format!(
        "# Proposed by cargo xtask session-evidence from session {session} ({ledger}); review\n\
         # each entry before adding it to docs/schema/evidence.toml.\n"
    );
    if !skipped.is_empty() {
        out.push_str("#\n# Rows with no record type, skipped (never guessed):\n");
        for line in &skipped {
            out.push_str(&format!("#   {line}\n"));
        }
    }
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
        let others = others(rows);
        let passed: Vec<&Row> = rows.iter().filter(|r| r.passed()).collect();
        if passed.is_empty() {
            out.push_str(&format!(
                "\n# {sig}: no row passed ({}); no entry\n",
                others.join("; ")
            ));
            continue;
        }
        let ids: Vec<&str> = passed.iter().map(|r| r.id.as_str()).collect();
        out.push_str(&entry(sig, RUNTIME, ledger, session, &ids, &others));
        // Rule 3: a passing row sharing a form with `persistence 4`, both persistence rows
        // passed.
        let via_4: Vec<&str> = passed
            .iter()
            .filter(|r| {
                persisted
                    && r.forms
                        .iter()
                        .any(|f| persist_4.iter().any(|p| p.forms.contains(f)))
            })
            .map(|r| r.id.as_str())
            .collect();
        // Rule 9: a passing `after the reload` row, `persistence 1-3` passed.
        let via_after: Vec<&str> = passed
            .iter()
            .filter(|r| persisted_13 && r.after_reload)
            .map(|r| r.id.as_str())
            .collect();
        let reload: Vec<&str> = if !via_4.is_empty() {
            [via_4, ids_13.clone(), ids_4.clone()].concat()
        } else if !via_after.is_empty() {
            [via_after, ids_13.clone()].concat()
        } else {
            Vec::new()
        };
        if !reload.is_empty() {
            out.push_str(&entry(sig, RELOAD, ledger, session, &reload, &others));
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

/// A record signature as the rules spell it: four characters of `A-Z`, `0-9` or `_`,
/// starting with a letter.
fn is_sig(word: &str) -> bool {
    word.len() == 4
        && word.starts_with(|c: char| c.is_ascii_uppercase())
        && word
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Rules 1 and 7: the row's record type from every source that names one (the backtick,
/// the row id, a reporter line, `the <SIG> <form>`, a trailing `(<SIG>)`); one type when
/// they all agree, otherwise why there is none.
fn row_type(id: &str, title: &str, forms: &[String]) -> Result<String, String> {
    let mut found: Vec<(String, &str)> = Vec::new();
    if let Some(sig) = record_type(title) {
        found.push((sig, "a backtick name"));
    }
    for seg in id.split('-').filter(|s| is_sig(s)) {
        found.push((seg.to_string(), "the row id"));
    }
    let words: Vec<&str> = title.split_whitespace().collect();
    for w in words.windows(3) {
        let [a, b, c] = w else { continue };
        // A reporter line: `KKV-P01 GLOB value=4242`.
        let tag = a.split_once('-').is_some_and(|(p, n)| {
            p.len() >= 2
                && p.chars().all(|c| c.is_ascii_uppercase())
                && n.chars().any(|c| c.is_ascii_digit())
                && n.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        });
        if tag && is_sig(b) {
            found.push((b.to_string(), "a reporter line"));
        }
        // `the ACTI KKV10Acti`, the name one of the row's forms.
        let name = c.trim_end_matches([':', ',', ';', ')', '.']);
        if a.trim_start_matches('(') == "the" && is_sig(b) && forms.iter().any(|f| f == name) {
            found.push((b.to_string(), "`the <SIG> <form>`"));
        }
    }
    if let Some(sig) = title
        .trim_end()
        .strip_suffix(')')
        .and_then(|t| t.rsplit_once('('))
        .map(|(_, s)| s)
        .filter(|s| is_sig(s))
    {
        found.push((sig.to_string(), "a trailing `(<SIG>)`"));
    }
    let mut sigs: Vec<&str> = found.iter().map(|(s, _)| s.as_str()).collect();
    sigs.sort_unstable();
    sigs.dedup();
    match sigs.as_slice() {
        [one] => Ok((*one).to_string()),
        [] => Err("no record type in its row id or title".to_string()),
        _ => Err(format!(
            "ambiguous record type ({})",
            found
                .iter()
                .map(|(s, from)| format!("{s} from {from}"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
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

    const VERIFY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/session-evidence/verify-report.json"
    ));

    /// Real verification-mod rows (a trimmed copy of game-20261116-04's, and V-H-PKIN of
    /// game-20261117-01): types from the row id, reporter lines, `the <SIG> <form>` and a
    /// trailing `(<SIG>)`; presence-only rows never pass; untyped rows are listed, skipped.
    #[test]
    fn verification_mod_rows_are_typed_from_their_ids_and_titles() {
        let report: Value = serde_json::from_str(VERIFY).unwrap();
        let out = propose(&report).unwrap();
        let list = entries(&out);
        let keys: Vec<(&str, &str)> = list
            .iter()
            .map(|t| (get(t, "type"), get(t, "status")))
            .collect();
        assert_eq!(
            keys,
            [
                ("ACTI", RUNTIME),
                ("ACTI", RELOAD),
                ("ARMO", RUNTIME),
                ("ARMO", RELOAD),
                ("GLOB", RUNTIME),
                ("LIGH", RUNTIME),
                ("LVLI", RUNTIME),
            ],
            "{out}"
        );
        let note = |sig: &str, status: &str| {
            list.iter()
                .find(|t| get(t, "type") == sig && get(t, "status") == status)
                .map(|t| get(t, "note").to_string())
                .unwrap()
        };
        assert!(
            note("ACTI", RUNTIME).starts_with("2026-11-16-T98: rows V-20, V-S-20 passed ("),
            "{out}"
        );
        assert!(
            note("ACTI", RELOAD).starts_with("2026-11-16-T98: rows V-S-20, V-S-01 passed ("),
            "{out}"
        );
        assert!(
            note("ARMO", RUNTIME).contains("rows V-I-ARMO, V-I-ARMO-EQ, V-S-I-ARMO passed"),
            "{out}"
        );
        assert!(
            note("GLOB", RUNTIME).contains("rows V-P01, V-C-GLOB passed"),
            "{out}"
        );
        // Presence only is never a pass.
        for sig in ["TMLM", "PKIN", "AVIF"] {
            assert!(
                out.contains(&format!("# {sig}: no row passed (")),
                "{sig}: {out}"
            );
        }
        assert!(
            out.contains("V-H-TMLM inconclusive (presence only)"),
            "{out}"
        );
        for skipped in [
            "#   V-01: no record type in its row id or title",
            "#   V-P00: no record type in its row id or title",
            "#   V-P98: a diagnostic row",
            "#   V-P99: no record type in its row id or title",
            "#   V-S-02: no record type in its row id or title",
        ] {
            assert!(out.contains(skipped), "{skipped}: {out}");
        }
    }

    #[test]
    fn a_row_whose_sources_disagree_is_skipped_not_guessed() {
        let forms = vec!["KKV10Acti".to_string()];
        let why = row_type("V-I-STAT", "the ACTI KKV10Acti is here", &forms).unwrap_err();
        assert!(why.starts_with("ambiguous record type"), "{why}");
        assert!(why.contains("STAT from the row id"), "{why}");
        // `the <SIG>` counts only when the next word is one of the row's forms.
        assert!(row_type("V-I-ARMO-EQ", "the ARMO is worn", &[]).is_ok());
        assert!(row_type("V-50", "the ARMO is worn", &[]).is_err());
        assert_eq!(
            row_type("V-P01", "Papyrus log: KKV-P01 GLOB value=4242 (GLOB)", &[]).as_deref(),
            Ok("GLOB")
        );
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
