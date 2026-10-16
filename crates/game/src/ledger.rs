//! The ledger entry a session appends to `docs/game-observations.md` (ADR 0010 section 4,
//! Amendment 1 conditions 5 and 7): the reproducible-setup header (game, script extender
//! and console tool versions, the base save's and every plugin's SHA-256, the probes'
//! hash), then one row per probe with what was sent, what was seen, the result
//! (`pass`, `fail`, `inconclusive` or `not-run`, each but `pass` with its reason) and the
//! screenshots, then the findings. The format is the ledger's own ("Entry format").

use serde::Serialize;

use crate::probes::{clip, Verdict};

/// One probe row as run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RowReport {
    /// The row id.
    pub row: String,
    /// What it checks.
    pub title: String,
    /// The forms it is about.
    pub forms: Vec<String>,
    /// The commands sent, in order.
    pub commands: Vec<String>,
    /// What was expected.
    pub expected: String,
    /// What came back.
    pub seen: String,
    /// The verdict.
    pub verdict: Verdict,
    /// Screenshot file names (under the observe folder).
    pub screenshots: Vec<String>,
}

/// A whole entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    /// `game-<yyyymmdd>-<nn>`.
    pub id: String,
    /// The session id (`2026-10-16-T01`).
    pub session: String,
    /// yyyy-mm-dd.
    pub date: String,
    /// The session's overall status.
    pub status: Verdict,
    /// `(what, version)`: game build, script extender, console tools; `unknown` allowed.
    pub versions: Vec<(String, String)>,
    /// The MO2 profile.
    pub profile: String,
    /// The MO2 instance.
    pub instance: String,
    /// The base save's name and SHA-256.
    pub base_save: (String, String),
    /// The session's own save name.
    pub session_save: String,
    /// `(plugin, SHA-256)`.
    pub plugins: Vec<(String, String)>,
    /// The probes file and its SHA-256.
    pub probes: (String, String),
    /// The observe folder.
    pub observe: String,
    /// The rows.
    pub rows: Vec<RowReport>,
    /// Findings (differences, refusals during the run, timeouts).
    pub findings: Vec<String>,
}

/// A table cell: no pipes or line breaks, at most 300 characters.
pub fn cell(text: &str) -> String {
    let t = clip(text, 300).replace('|', "\\|");
    if t.is_empty() {
        "-".into()
    } else {
        t
    }
}

/// `yyyy-mm-dd` -> `yyyymmdd`.
pub fn compact(date: &str) -> String {
    date.chars().filter(char::is_ascii_digit).collect()
}

/// The next free `game-<yyyymmdd>-<nn>` in `ledger` for `date` (yyyy-mm-dd).
pub fn next_id(ledger: &str, date: &str) -> String {
    let prefix = format!("### game-{}-", compact(date));
    let max = ledger
        .lines()
        .filter_map(|l| l.strip_prefix(&prefix))
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0);
    format!("game-{}-{:02}", compact(date), max.saturating_add(1))
}

/// The entry's Markdown, starting with a blank line (to append at the end of the file).
pub fn render(e: &Entry) -> String {
    let mut s = String::new();
    let plugins = e
        .plugins
        .iter()
        .map(|(n, h)| format!("`{n}` SHA-256 `{h}`"))
        .collect::<Vec<_>>()
        .join("; ");
    s.push_str(&format!(
        "\n### {}: agent session {} (`kit game session`), {}\n\n",
        e.id,
        e.session,
        e.plugins
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    s.push_str("| Field | Content |\n| ----- | ------- |\n");
    let mut field = |k: &str, v: String| s.push_str(&format!("| {k} | {} |\n", cell(&v)));
    field(
        "Session",
        format!(
            "`{}`, run by the session controller (ADR 0010, lane C); not an owner sitting",
            e.session
        ),
    );
    field("Date", e.date.clone());
    field("Status", e.status.text());
    for (what, version) in &e.versions {
        field(what, version.clone());
    }
    field(
        "MO2 profile",
        format!("`{}` (instance {})", e.profile, e.instance),
    );
    field(
        "Save",
        format!(
            "base `{}` SHA-256 `{}`; session save `{}`",
            e.base_save.0, e.base_save.1, e.session_save
        ),
    );
    field("Plugins", plugins);
    field(
        "Probes",
        format!("`{}` SHA-256 `{}`", e.probes.0, e.probes.1),
    );
    field("Screenshots", format!("`{}`", e.observe));
    s.push_str("\n| Row | Step | Forms | Expected | Seen | Result | Evidence |\n");
    s.push_str("| --- | ---- | ----- | -------- | ---- | ------ | -------- |\n");
    for r in &e.rows {
        let step = if r.commands.is_empty() {
            r.title.clone()
        } else {
            r.commands
                .iter()
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let result = match &r.verdict {
            Verdict::Pass => "**pass**".to_string(),
            v => v.text(),
        };
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            cell(&r.row),
            cell(&step),
            cell(&r.forms.join(", ")),
            cell(&r.expected),
            cell(&r.seen),
            cell(&result),
            cell(&r.screenshots.join(", "))
        ));
    }
    if !e.findings.is_empty() {
        s.push('\n');
        for f in &e.findings {
            s.push_str(&format!("Finding: {}\n", clip(f, 600)));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn entry() -> Entry {
        Entry {
            id: "game-20261016-01".into(),
            session: "2026-10-16-T01".into(),
            date: "2026-10-16".into(),
            status: Verdict::Inconclusive("plugins.txt changed; restored".into()),
            versions: vec![
                ("Game build".into(), "`Starfield.exe` 1.16.244.0".into()),
                ("Script extender".into(), "unknown".into()),
            ],
            profile: "Kitkraft Dev".into(),
            instance: r"C:\MO2".into(),
            base_save: ("KitKraftBase".into(), "ab".repeat(32)),
            session_save: "KitKraftSession-20261016-01".into(),
            plugins: vec![("X.esm".into(), "cd".repeat(32))],
            probes: ("probes.toml".into(), "ef".repeat(32)),
            observe: r"observe\2026-10-16-T01\".into(),
            rows: vec![
                RowReport {
                    row: "T-01".into(),
                    title: "help".into(),
                    forms: vec!["X.esm:000800".into()],
                    commands: vec!["help \"A|B\" 4".into()],
                    expected: "contains `00000800`".into(),
                    seen: "WEAP 00000800\nline two".into(),
                    verdict: Verdict::Pass,
                    screenshots: vec![],
                },
                RowReport {
                    row: "T-02".into(),
                    title: "manual look".into(),
                    forms: vec![],
                    commands: vec![],
                    expected: "the owner looks".into(),
                    seen: String::new(),
                    verdict: Verdict::NotRun("manual: needs a look".into()),
                    screenshots: vec!["T-02.png".into()],
                },
            ],
            findings: vec!["plugins.txt changed during the session; restored".into()],
        }
    }

    #[test]
    fn the_entry_follows_the_ledger_format() {
        let text = render(&entry());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "");
        assert!(lines[1].starts_with("### game-20261016-01: agent session 2026-10-16-T01"));
        assert!(text.contains("| Status | inconclusive (plugins.txt changed; restored) |"));
        assert!(text.contains("| Game build | `Starfield.exe` 1.16.244.0 |"));
        assert!(text.contains(&format!(
            "base `KitKraftBase` SHA-256 `{}`",
            "ab".repeat(32)
        )));
        assert!(text.contains("| Row | Step | Forms | Expected | Seen | Result | Evidence |"));
        assert!(text.contains("| T-01 | `help \"A\\|B\" 4` | X.esm:000800 | contains `00000800` | WEAP 00000800 line two | **pass** | - |"));
        assert!(text.contains("| T-02 | manual look | - | the owner looks | - | not-run (manual: needs a look) | T-02.png |"));
        assert!(text.contains("Finding: plugins.txt changed"));
        // Every table row has the same number of cells as its header.
        for l in lines.iter().filter(|l| l.starts_with("| T-")) {
            assert_eq!(l.matches(" | ").count(), 6, "{l}");
        }
    }

    #[test]
    fn ids_count_up_per_day() {
        let ledger = "## Ledger\n\n### game-20261014-01: x\n### game-20261016-01: a\n### game-20261016-03: b\n";
        assert_eq!(next_id(ledger, "2026-10-16"), "game-20261016-04");
        assert_eq!(next_id(ledger, "2026-10-17"), "game-20261017-01");
    }
}
