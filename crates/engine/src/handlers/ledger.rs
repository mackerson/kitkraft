//! `ledger` (lane W, ADR 0007 Amendment 2): one dated row in the in-game ledger,
//! `docs/game-observations.md`, and nowhere else.
//!
//! The row says which check it is (an overhaul check `C1`..`C14`, the first POI, the
//! persistence protocol, or a custom label), `pass`, `fail` or `note`, the FormIDs the
//! check looked at and what was seen. Rows of one day and profile go into one entry,
//! `### game-<yyyymmdd>-<nn>: workbench ledger, <date>, <profile>`, at the end of the
//! file: the entry is created when the file does not already end with it, and a row is
//! otherwise appended to its table. Both are appends; nothing already in the file is
//! rewritten.
//!
//! Without `confirm: true` it writes nothing and returns the text it would append (the
//! workbench's preview). The path must be `<checkout>/docs/game-observations.md`: a
//! regular file named so, in a folder named `docs`, next to the checkout's `AGENTS.md`,
//! starting with the ledger's title and holding its `## Ledger` section, reached through
//! no link and outside the game, the user Data folder and MO2. Every other path is
//! refused. The `Default` profile is refused as the profile a check ran on (AGENTS.md,
//! "Owner's live game is off limits").

use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::testplan::DEV_PROFILE;
use super::{failed, str_arg};
use crate::edit::patch::guard_location;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

const ID: &str = "ledger";

/// The ledger's file name.
pub(crate) const LEDGER_FILE: &str = "game-observations.md";
/// The folder it lives in.
const DOCS_DIR: &str = "docs";
/// The file that marks a KitKraft checkout, next to `docs`.
const CHECKOUT_MARKER: &str = "AGENTS.md";
/// The ledger's first line.
const TITLE: &str = "# Game observations";
/// The section entries go under.
const LEDGER_HEADING: &str = "## Ledger";
/// What a workbench entry's heading says after its id.
const ENTRY_TAG: &str = "workbench ledger";
/// The owner's live-game profile, refused as a check's profile.
const LIVE_PROFILE: &str = "Default";

/// Longest `note` and `label` (also in the schema).
const MAX_NOTE: usize = 2000;
const MAX_LABEL: usize = 200;

fn refused(why: impl std::fmt::Display) -> EngineError {
    failed(ID, format!("refused [ledger-path]: {why}"))
}

/// Why `path` is not the ledger, or `Ok` when it is. `path` is absolute.
pub(crate) fn check_ledger_path(path: &Path) -> Result<(), EngineError> {
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(refused(format!(
            "{} has `.` or `..` in it; name the ledger by a plain path",
            path.display()
        )));
    }
    let name_is = |p: Option<&Path>, want: &str| {
        p.and_then(Path::file_name)
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(want))
    };
    let docs = path.parent();
    if !name_is(Some(path), LEDGER_FILE) || !name_is(docs, DOCS_DIR) {
        return Err(refused(format!(
            "{} is not docs/{LEDGER_FILE}; the ledger is the only file this command writes",
            path.display()
        )));
    }
    let checkout = docs.and_then(Path::parent);
    if !checkout.is_some_and(|c| c.join(CHECKOUT_MARKER).is_file()) {
        return Err(refused(format!(
            "{} is not in a KitKraft checkout (no {CHECKOUT_MARKER} next to its docs folder)",
            path.display()
        )));
    }
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => {}
        Ok(_) => {
            return Err(refused(format!(
                "{} is a link or not a regular file",
                path.display()
            )))
        }
        Err(e) => return Err(refused(format!("{}: {e}", path.display()))),
    }
    guard_location(path).map_err(|e| refused(e.to_string()))
}

/// The day `secs` seconds after the Unix epoch falls on (UTC), as `(year, month, day)`.
fn civil_from_unix(secs: u64) -> Option<(u64, u64, u64)> {
    // Howard Hinnant's days-to-civil, for days on or after 1970-01-01.
    let days = secs.checked_div(86_400)?;
    let z = days.checked_add(719_468)?;
    let era = z.checked_div(146_097)?;
    let doe = z.checked_sub(era.checked_mul(146_097)?)?;
    let yoe = doe
        .checked_sub(doe.checked_div(1460)?)?
        .checked_add(doe.checked_div(36_524)?)?
        .checked_sub(doe.checked_div(146_096)?)?
        .checked_div(365)?;
    let doy = doe.checked_sub(
        yoe.checked_mul(365)?
            .checked_add(yoe.checked_div(4)?)?
            .checked_sub(yoe.checked_div(100)?)?,
    )?;
    let mp = doy.checked_mul(5)?.checked_add(2)?.checked_div(153)?;
    let day = doy
        .checked_sub(mp.checked_mul(153)?.checked_add(2)?.checked_div(5)?)?
        .checked_add(1)?;
    let month = if mp < 10 {
        mp.checked_add(3)?
    } else {
        mp.checked_sub(9)?
    };
    let year = yoe.checked_add(era.checked_mul(400)?)?;
    let year = if month <= 2 {
        year.checked_add(1)?
    } else {
        year
    };
    Some((year, month, day))
}

/// Today in UTC as `yyyy-mm-dd`.
fn today_utc() -> Option<String> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let (y, m, d) = civil_from_unix(secs)?;
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// `date` as `yyyymmdd` when it is a `yyyy-mm-dd` day, else `None`.
fn compact_date(date: &str) -> Option<String> {
    let mut parts = date.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || y.len() != 4
        || m.len() != 2
        || d.len() != 2
        || !date.chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return None;
    }
    let month: u8 = m.parse().ok()?;
    let day: u8 = d.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{y}{m}{d}"))
}

/// One table cell: on one line, with `|` escaped, control characters as spaces.
fn cell(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.trim().chars() {
        match c {
            '|' => out.push_str("\\|"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    if out.is_empty() {
        "-".to_string()
    } else {
        out
    }
}

/// A FormID, runtime ID or EditorID as given: letters, digits, `_ . : -` and spaces.
fn valid_form(f: &str) -> bool {
    !f.trim().is_empty()
        && f.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-' | ' '))
}

/// What the row checked, for its Step and Expected cells.
fn step_and_expected(row: &str, label: &str) -> (String, &'static str) {
    let step = if let Some(n) = row.strip_prefix('C').and_then(|n| n.parse::<u8>().ok()) {
        format!("Overhaul build 2 check {row} (`docs/overhaul/build-2.md`; template row O5-{n:02})")
    } else {
        match row {
            "POI" => "First POI (`docs/poi/first-poi.md`; template session O2)".to_string(),
            "persistence" => {
                "Persistence protocol (this file, \"Persistence protocol\")".to_string()
            }
            _ => format!("custom: {label}"),
        }
    };
    let step = if row != "custom" && !label.is_empty() {
        format!("{step}: {label}")
    } else {
        step
    };
    let expected = if row == "custom" {
        "-"
    } else {
        "as the template row"
    };
    (step, expected)
}

/// The id parts of a `### game-<yyyymmdd>-<nn>...` heading.
fn entry_id(line: &str) -> Option<(&str, u32)> {
    let rest = line.strip_prefix("### game-")?;
    let date = rest.get(..8)?;
    let nn = rest.get(9..11)?;
    if rest.get(8..9) != Some("-") || !date.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((date, nn.parse().ok()?))
}

/// What to append to `text` for `row_line`, and the entry it lands in.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Append {
    text: String,
    entry: String,
    new_entry: bool,
}

fn plan_append(
    text: &str,
    date: &str,
    compact: &str,
    profile: &str,
    row_line: &str,
) -> Result<Append, EngineError> {
    let heading_tail = format!(": {ENTRY_TAG}, {date}, `{profile}`");
    let lines: Vec<&str> = text.lines().collect();
    let last_heading = lines
        .iter()
        .enumerate()
        .rev()
        .find(|(_, l)| l.starts_with("## ") || l.starts_with("### "));
    // The file ends with today's workbench entry for this profile, its table last.
    let trimmed = text.trim_end_matches(['\n', '\r']);
    let ends_in_table = trimmed.lines().last().is_some_and(|l| l.starts_with('|'))
        && matches!(text.get(trimmed.len()..), Some("\n" | "\r\n"));
    if let Some((_, line)) = last_heading {
        if let Some((d, nn)) = entry_id(line) {
            if d == compact && line.ends_with(&heading_tail) && ends_in_table {
                return Ok(Append {
                    text: format!("{row_line}\n"),
                    entry: format!("game-{d}-{nn:02}"),
                    new_entry: false,
                });
            }
        }
    }
    let next = lines
        .iter()
        .filter_map(|l| entry_id(l))
        .filter(|(d, _)| *d == compact)
        .map(|(_, nn)| nn)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .filter(|n| *n <= 99)
        .ok_or_else(|| failed(ID, format!("{date} already has 99 ledger entries")))?;
    let entry = format!("game-{compact}-{next:02}");
    let sep = if text.ends_with("\n\n") {
        ""
    } else if text.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let body = format!(
        "{sep}### {entry}{heading_tail}\n\n\
         | Field | Content |\n\
         | ----- | ------- |\n\
         | Session | Rows recorded from the workbench's Ledger panel (`kit ledger`), one per confirmed click |\n\
         | Date | {date} |\n\
         | Game build | not recorded |\n\
         | MO2 profile | `{profile}` |\n\n\
         | Row | Step | Forms | Expected | Seen | Result |\n\
         | --- | ---- | ----- | -------- | ---- | ------ |\n\
         {row_line}\n"
    );
    Ok(Append {
        text: body,
        entry,
        new_entry: true,
    })
}

/// `ledger`.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let row = str_arg(ID, args, "row")?;
    let result = str_arg(ID, args, "result")?;
    let text_arg = |name: &str| {
        args.get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    };
    let note = text_arg("note");
    let label = text_arg("label");
    if note.chars().count() > MAX_NOTE || label.chars().count() > MAX_LABEL {
        return Err(failed(
            ID,
            format!("`note` is at most {MAX_NOTE} characters, `label` at most {MAX_LABEL}"),
        ));
    }
    if row == "custom" && label.is_empty() {
        return Err(failed(
            ID,
            "a `custom` row needs `label` (what was checked)",
        ));
    }
    if result == "note" && note.is_empty() {
        return Err(failed(ID, "a `note` row needs `note`"));
    }
    let profile = args
        .get("profile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .unwrap_or(DEV_PROFILE)
        .to_string();
    if profile.eq_ignore_ascii_case(LIVE_PROFILE) {
        return Err(failed(
            ID,
            format!(
                "refused [live-profile]: in-game checks run on `{DEV_PROFILE}`, never on `{LIVE_PROFILE}` (the owner's live game)"
            ),
        ));
    }
    if !valid_form(&profile) {
        return Err(failed(
            ID,
            format!("profile `{profile}`: letters, digits, `_ . : -` and spaces only"),
        ));
    }
    let forms: Vec<String> = args
        .get("forms")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .collect()
        })
        .unwrap_or_default();
    if let Some(bad) = forms.iter().find(|f| !valid_form(f)) {
        return Err(failed(
            ID,
            format!("form `{bad}`: give Plugin.esm:XXXXXX, a runtime ID or an EditorID (letters, digits, `_ . : -`)"),
        ));
    }
    let mut limitations = Vec::new();
    let date = match args.get("date").and_then(Value::as_str) {
        Some(d) => d.trim().to_string(),
        None => {
            limitations.push(
                "the date is today's in UTC (the engine knows no time zone); pass `date` for the local day"
                    .to_string(),
            );
            today_utc().ok_or_else(|| failed(ID, "the system clock is before 1970"))?
        }
    };
    let compact = compact_date(&date)
        .ok_or_else(|| failed(ID, format!("date `{date}` is not yyyy-mm-dd")))?;

    let file = args
        .get("file")
        .and_then(Value::as_str)
        .map_or_else(|| Path::new(DOCS_DIR).join(LEDGER_FILE), PathBuf::from);
    let file = if file.is_absolute() {
        file
    } else {
        std::env::current_dir()
            .map_err(|e| failed(ID, format!("working directory: {e}")))?
            .join(file)
    };
    check_ledger_path(&file)?;
    let before = std::fs::read_to_string(&file)
        .map_err(|e| failed(ID, format!("{}: {e}", file.display())))?;
    if !before.starts_with(TITLE) || !before.lines().any(|l| l.trim_end() == LEDGER_HEADING) {
        return Err(refused(format!(
            "{} does not start with `{TITLE}` or has no `{LEDGER_HEADING}` section",
            file.display()
        )));
    }

    let (step, expected) = step_and_expected(row, &label);
    let forms_cell = if forms.is_empty() {
        "-".to_string()
    } else {
        forms
            .iter()
            .map(|f| format!("`{}`", cell(f)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let row_line = format!(
        "| {} | {} | {} | {} | {} | {} |",
        cell(row),
        cell(&step),
        forms_cell,
        expected,
        cell(&note),
        cell(result)
    );
    let append = plan_append(&before, &date, &compact, &profile, &row_line)?;
    let offset = u64::try_from(before.len()).unwrap_or(u64::MAX);
    let length = u64::try_from(append.text.len()).unwrap_or(u64::MAX);
    let confirm = args
        .get("confirm")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut evidence = Vec::new();
    if confirm {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .map_err(|e| failed(ID, format!("{}: {e}", file.display())))?;
        f.write_all(append.text.as_bytes())
            .and_then(|()| f.flush())
            .map_err(|e| failed(ID, format!("{}: {e}", file.display())))?;
        evidence.push(Evidence {
            file: LEDGER_FILE.to_string(),
            offset,
            length,
            record: None,
            subrecord: None,
        });
    } else {
        limitations.push(
            "preview: nothing was written; pass `confirm: true` to append this text".to_string(),
        );
    }
    let what = format!("{row} {result}");
    let summary = if confirm {
        format!(
            "appended {what} to {}{} in {}",
            append.entry,
            if append.new_entry { " (new entry)" } else { "" },
            file.display()
        )
    } else {
        format!(
            "preview: would append {what} to {}{} in {} (nothing written)",
            append.entry,
            if append.new_entry { " (new entry)" } else { "" },
            file.display()
        )
    };
    Ok(Outcome {
        data: json!({
            "file": file.display().to_string(),
            "entry": append.entry,
            "new_entry": append.new_entry,
            "row": row_line,
            "text": append.text,
            "appended": confirm,
            "offset": offset,
            "length": length,
            "date": date,
            "profile": profile,
            "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    const LEDGER: &str = "# Game observations (in-game ledger, M4.H4)\n\nIntro.\n\n## Ledger\n\n### game-20261014-01: the rename plugin\n\n| Row | Step |\n| --- | ---- |\n| O2-01 | x |\n";

    /// A scratch checkout: `<dir>/AGENTS.md`, `<dir>/docs/game-observations.md`.
    fn checkout(tag: &str) -> (PathBuf, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("kitkraft-ledger-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "# AGENTS.md\n").unwrap();
        let file = dir.join("docs").join(LEDGER_FILE);
        std::fs::write(&file, LEDGER).unwrap();
        (dir, file)
    }

    fn args(file: &Path, extra: Value) -> Value {
        let mut a = json!({"row": "C3", "result": "pass", "date": "2026-10-16",
                           "file": file.display().to_string()});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        a
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_unix(0), Some((1970, 1, 1)));
        assert_eq!(civil_from_unix(951_782_400), Some((2000, 2, 29)));
        // 2026-10-15T23:59:59Z
        assert_eq!(civil_from_unix(1_792_108_799), Some((2026, 10, 15)));
        assert_eq!(compact_date("2026-10-16").as_deref(), Some("20261016"));
        for bad in [
            "2026-13-01",
            "2026-1-01",
            "26-10-16",
            "2026-10-16x",
            "2026/10/16",
        ] {
            assert_eq!(compact_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn preview_writes_nothing_and_confirm_appends_one_entry_then_rows() {
        let (dir, file) = checkout("append");
        let preview = crate::execute(
            "ledger",
            args(
                &file,
                json!({"forms": ["Starfield.esm:2984DF"], "note": "24 rounds | 5.6/s"}),
            ),
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), LEDGER);
        assert_eq!(preview["data"]["appended"], false);
        assert_eq!(preview["data"]["entry"], "game-20261016-01");
        assert!(preview["evidence"].as_array().unwrap().is_empty());

        let first = crate::execute(
            "ledger",
            args(
                &file,
                json!({"forms": ["Starfield.esm:2984DF"], "note": "24 rounds | 5.6/s", "confirm": true}),
            ),
        )
        .unwrap();
        let after = std::fs::read_to_string(&file).unwrap();
        assert!(after.starts_with(LEDGER), "nothing before is rewritten");
        assert_eq!(first["data"]["new_entry"], true);
        assert!(
            after.contains("### game-20261016-01: workbench ledger, 2026-10-16, `KitKraft Dev`")
        );
        assert!(after.contains(
            "| C3 | Overhaul build 2 check C3 (`docs/overhaul/build-2.md`; template row O5-03) | `Starfield.esm:2984DF` | as the template row | 24 rounds \\| 5.6/s | pass |\n"
        ), "{after}");
        assert_eq!(first["evidence"][0]["offset"], LEDGER.len());

        // A second row the same day goes into the same entry's table.
        let second = crate::execute(
            "ledger",
            args(
                &file,
                json!({"row": "POI", "result": "fail", "note": "crate missing", "confirm": true}),
            ),
        )
        .unwrap();
        assert_eq!(second["data"]["new_entry"], false);
        assert_eq!(second["data"]["entry"], "game-20261016-01");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.ends_with("| crate missing | fail |\n"), "{text}");
        assert_eq!(text.matches("### game-20261016").count(), 1);

        // Another day, or another profile, opens a new entry.
        let third = crate::execute(
            "ledger",
            args(&file, json!({"row": "custom", "label": "dropped pistol", "result": "note",
                               "note": "still there", "date": "2026-10-16", "profile": "KitKraft Dev 2", "confirm": true})),
        )
        .unwrap();
        assert_eq!(third["data"]["entry"], "game-20261016-02");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_other_path_is_refused() {
        let (dir, file) = checkout("paths");
        let other = dir.join("docs").join("workbench.md");
        std::fs::write(&other, LEDGER).unwrap();
        let outside = dir.join("game-observations.md");
        std::fs::write(&outside, LEDGER).unwrap();
        let no_marker = dir.join("sub").join("docs").join(LEDGER_FILE);
        std::fs::create_dir_all(no_marker.parent().unwrap()).unwrap();
        std::fs::write(&no_marker, LEDGER).unwrap();
        let dotted = dir.join("docs").join("..").join("docs").join(LEDGER_FILE);
        let missing_dir =
            std::env::temp_dir().join(format!("kitkraft-ledger-none-{}", std::process::id()));
        for bad in [
            other.clone(),
            outside.clone(),
            no_marker.clone(),
            dotted,
            missing_dir.join("docs").join(LEDGER_FILE),
        ] {
            let e = crate::execute("ledger", args(&bad, json!({"confirm": true}))).unwrap_err();
            assert!(
                e.to_string().contains("refused [ledger-path]"),
                "{}: {e}",
                bad.display()
            );
        }
        for f in [&other, &outside, &no_marker] {
            assert_eq!(std::fs::read_to_string(f).unwrap(), LEDGER);
        }
        // A file named right but without the ledger's title or section.
        std::fs::write(&file, "# Something else\n").unwrap();
        let e = crate::execute("ledger", args(&file, json!({"confirm": true}))).unwrap_err();
        assert!(e.to_string().contains("refused [ledger-path]"), "{e}");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "# Something else\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn arguments_are_checked_before_anything_is_read() {
        let (dir, file) = checkout("args");
        for (extra, needle) in [
            (json!({"profile": "Default"}), "live-profile"),
            (json!({"profile": " default "}), "live-profile"),
            (json!({"row": "custom"}), "label"),
            (json!({"result": "note"}), "needs `note`"),
            (json!({"forms": ["X|Y"]}), "form `X|Y`"),
            (json!({"date": "16.10.2026"}), "yyyy-mm-dd"),
        ] {
            let e = crate::execute("ledger", args(&file, extra.clone())).unwrap_err();
            assert!(e.to_string().contains(needle), "{extra}: {e}");
        }
        let e = crate::execute("ledger", args(&file, json!({"row": "C15"}))).unwrap_err();
        assert_eq!(e.kind(), "invalid_args");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), LEDGER);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_not_ending_in_the_entry_table_gets_a_new_entry() {
        let text = format!("{LEDGER}### game-20261016-01: workbench ledger, 2026-10-16, `KitKraft Dev`\n\n| Row |\n| --- |\n| C1 |\n\nA finding line.\n");
        let a = plan_append(&text, "2026-10-16", "20261016", "KitKraft Dev", "| C2 |").unwrap();
        assert!(a.new_entry);
        assert_eq!(a.entry, "game-20261016-02");
        let ok = format!("{LEDGER}### game-20261016-01: workbench ledger, 2026-10-16, `KitKraft Dev`\n\n| Row |\n| --- |\n| C1 |\n");
        let b = plan_append(&ok, "2026-10-16", "20261016", "KitKraft Dev", "| C2 |").unwrap();
        assert!(!b.new_entry);
        assert_eq!(b.text, "| C2 |\n");
    }
}
