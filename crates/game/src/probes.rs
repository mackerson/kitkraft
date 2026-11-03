//! `probes.toml` (M4.T2): the machine-checkable half of a `kit testplan`, one probe per
//! checklist row, and the verdicts the controller reaches on them (ADR 0010 Amendment 1
//! condition 7).
//!
//! A probe is a row id (the checklist's), a title, the forms it is about, whether it wants
//! a screenshot, and an ordered list of steps. A step is one console command; a step with
//! a `matcher` is checked against that command's output (`contains`, `regex`, `number`
//! within `tolerance`) or against the Papyrus log (`papyrus-log`); a step without one is
//! setup. A step's `wait_secs` is a pause after its answer, before the next step: the game
//! answers `player.moveto`, `coc` and `load` at once and then moves, and a command sent
//! meanwhile answers with only its echo (game-20261101-T1). A row no console command can
//! answer is `matcher = "manual"` with its `reason`.
//!
//! The answer's shape (game-20261101-T1, Starfield 1.16.244 over the Console API): the
//! command's echo, `Command: <command>`, then the result lines, a getter's as
//! `GetPos: X >> -17.00` or `GetInCell >> 1.00`. So `number` reads the number after the
//! last `>>`, never a digit of the echoed command.
//! `{save}` in a command is the session's own save name and `{base_save}` the base save's;
//! the controller fills both and refuses any other `save` or `load` target.
//!
//! Verdicts are exactly `not-run`, `pass`, `fail` and `inconclusive`, each but `pass`
//! with a reason. **Missing evidence is `inconclusive`, never `pass`:** no output to
//! match, no number in it, a log that was not written, a screenshot not taken. **A console
//! failure line in any step's answer ([`console_failure`]) makes the row `fail`**, naming
//! the step and the line, whatever the later matchers say.

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{GameError, Result};

/// The format version `kit testplan` writes.
pub const FORMAT: u32 = 1;

/// Longest console command accepted.
pub const MAX_COMMAND: usize = 1000;

/// Longest pause a step may ask for (`wait_secs`); a row's deadline bounds it too.
pub const MAX_WAIT_SECS: u64 = 300;

/// A `probes.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeFile {
    /// [`FORMAT`].
    pub format: u32,
    /// The plugin's file name.
    pub plugin: String,
    /// Its SHA-256.
    pub sha256: String,
    /// The tool that wrote it.
    #[serde(default)]
    pub generated_by: String,
    /// The MO2 profile the probes run on.
    #[serde(default)]
    pub profile: String,
    /// Where the numeric runtime IDs come from (`confirmed-by-save`, `inferred:...`).
    #[serde(default)]
    pub runtime_id_basis: String,
    /// What the expectations rest on (grades, assumptions).
    #[serde(default)]
    pub notes: Vec<String>,
    /// The probes, in checklist order.
    #[serde(default, rename = "probe")]
    pub probes: Vec<Probe>,
}

/// One row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    /// The checklist row id (`T-03`).
    pub row: String,
    /// What the row checks.
    pub title: String,
    /// The forms (`Plugin.esm:XXXXXX`, EditorIDs, runtime IDs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forms: Vec<String>,
    /// A screenshot is part of the row's evidence.
    #[serde(default, skip_serializing_if = "is_false")]
    pub screenshot: bool,
    /// `manual` for a row no console command answers; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    /// Why a row is manual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Commands in order.
    #[serde(default, rename = "step", skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One console command and what its output must show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// The console command (one line). May be empty for a `papyrus-log` step.
    pub command: String,
    /// How the output is checked; none: a setup step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<Matcher>,
    /// The expected text, pattern or number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// For `number`: the largest accepted difference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
    /// Where the expectation comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// Seconds to pause after this step's answer, before the next step (at most
    /// [`MAX_WAIT_SECS`], and never past the row's deadline): a settle after a command
    /// the game answers before it has acted on it (`player.moveto`, `coc`, `load`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_secs: Option<u64>,
}

/// How a step's output is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Matcher {
    /// The output contains `expected` (ASCII case ignored).
    Contains,
    /// The output matches the regular expression `expected`.
    Regex,
    /// The number after the output's last `>>` is within `tolerance` of `expected`.
    Number,
    /// The Papyrus log written during the session contains `expected`.
    PapyrusLog,
}

impl Probe {
    /// Whether the row is manual.
    pub fn is_manual(&self) -> bool {
        self.matcher.as_deref() == Some("manual")
    }
}

impl ProbeFile {
    /// Parses and checks a `probes.toml`.
    pub fn parse(text: &str) -> Result<ProbeFile> {
        let file: ProbeFile = toml::from_str(text).map_err(|e| GameError::Probes(e.to_string()))?;
        file.check()?;
        Ok(file)
    }

    /// The TOML text (deterministic: field and row order as stored).
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string(self).map_err(|e| GameError::Probes(e.to_string()))
    }

    /// Checks rows, commands and matchers.
    pub fn check(&self) -> Result<()> {
        let bad = |row: &str, m: String| GameError::Probes(format!("row {row}: {m}"));
        if self.format != FORMAT {
            return Err(GameError::Probes(format!(
                "format {} (this build reads {FORMAT})",
                self.format
            )));
        }
        let mut seen = std::collections::BTreeSet::new();
        for p in &self.probes {
            if p.row.trim().is_empty() {
                return Err(GameError::Probes("a probe has an empty row id".into()));
            }
            if !seen.insert(p.row.as_str()) {
                return Err(bad(&p.row, "row id used twice".into()));
            }
            match p.matcher.as_deref() {
                None => {}
                Some("manual") => {
                    if p.reason.as_deref().is_none_or(|r| r.trim().is_empty()) {
                        return Err(bad(&p.row, "a manual row needs a reason".into()));
                    }
                }
                Some(other) => {
                    return Err(bad(
                        &p.row,
                        format!("row matcher `{other}` (only `manual` goes on a row; steps carry the others)"),
                    ))
                }
            }
            if !p.is_manual() && !p.steps.iter().any(|s| s.matcher.is_some()) {
                return Err(bad(
                    &p.row,
                    "no step has a matcher (or mark the row manual)".into(),
                ));
            }
            for s in &p.steps {
                check_step(s).map_err(|m| bad(&p.row, m))?;
            }
        }
        Ok(())
    }
}

fn check_step(s: &Step) -> std::result::Result<(), String> {
    if s.command.chars().any(|c| c == '\r' || c == '\n') {
        return Err(format!("command `{}` spans lines", clip(&s.command, 60)));
    }
    if s.command.len() > MAX_COMMAND {
        return Err(format!("command longer than {MAX_COMMAND} bytes"));
    }
    if s.command.trim().is_empty() && s.matcher != Some(Matcher::PapyrusLog) {
        return Err("empty command".into());
    }
    if s.wait_secs.is_some_and(|w| w > MAX_WAIT_SECS) {
        return Err(format!(
            "`{}`: wait_secs above {MAX_WAIT_SECS}",
            clip(&s.command, 60)
        ));
    }
    let Some(m) = s.matcher else {
        return Ok(());
    };
    let expected = s
        .expected
        .as_deref()
        .filter(|e| !e.is_empty())
        .ok_or_else(|| format!("`{}`: a matcher needs `expected`", clip(&s.command, 60)))?;
    match m {
        Matcher::Regex => {
            Regex::new(expected).map_err(|e| format!("regex `{expected}`: {e}"))?;
        }
        Matcher::Number => {
            let v: f64 = expected
                .trim()
                .parse()
                .map_err(|_| format!("number `{expected}`"))?;
            let t = s.tolerance.unwrap_or(0.0);
            if !v.is_finite() || !t.is_finite() || t < 0.0 {
                return Err(format!("number `{expected}` with tolerance {t}"));
            }
        }
        Matcher::Contains | Matcher::PapyrusLog => {}
    }
    Ok(())
}

/// A row's or a step's result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "reason", rename_all = "kebab-case")]
pub enum Verdict {
    /// Not run, with why.
    NotRun(String),
    /// Every check passed with evidence.
    Pass,
    /// A check saw something else.
    Fail(String),
    /// The evidence is missing or ambiguous.
    Inconclusive(String),
}

impl Verdict {
    /// `pass`, `fail`, `inconclusive` or `not-run`.
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::NotRun(_) => "not-run",
            Verdict::Pass => "pass",
            Verdict::Fail(_) => "fail",
            Verdict::Inconclusive(_) => "inconclusive",
        }
    }

    /// The reason, empty for `pass`.
    pub fn reason(&self) -> &str {
        match self {
            Verdict::Pass => "",
            Verdict::NotRun(r) | Verdict::Fail(r) | Verdict::Inconclusive(r) => r,
        }
    }

    /// `pass`, or `<label> (<reason>)`.
    pub fn text(&self) -> String {
        match self {
            Verdict::Pass => "pass".into(),
            other => format!("{} ({})", other.label(), other.reason()),
        }
    }

    /// One verdict for several: any `fail` wins, then `inconclusive`, then `not-run`;
    /// `pass` only when every part passed and there was at least one.
    pub fn combine(parts: &[Verdict]) -> Verdict {
        if let Some(f) = parts.iter().find(|v| matches!(v, Verdict::Fail(_))) {
            return f.clone();
        }
        if let Some(i) = parts.iter().find(|v| matches!(v, Verdict::Inconclusive(_))) {
            return i.clone();
        }
        if let Some(n) = parts.iter().find(|v| matches!(v, Verdict::NotRun(_))) {
            return n.clone();
        }
        if parts.is_empty() {
            return Verdict::Inconclusive("no check ran".into());
        }
        Verdict::Pass
    }
}

/// The verdict of one step on `output` (the command's answer; `None` when there was
/// none) and `papyrus` (the Papyrus log written during the session; `None` when there is
/// no log).
pub fn judge(step: &Step, output: Option<&str>, papyrus: Option<&str>) -> Verdict {
    let Some(m) = step.matcher else {
        return Verdict::Pass;
    };
    let expected = step.expected.as_deref().unwrap_or_default();
    let text = match m {
        Matcher::PapyrusLog => match papyrus {
            Some(t) => t,
            None => return Verdict::Inconclusive("the Papyrus log was not written".into()),
        },
        _ => match output {
            Some(t) if !t.trim().is_empty() => t,
            _ => return Verdict::Inconclusive("no console output to match".into()),
        },
    };
    match m {
        Matcher::Contains | Matcher::PapyrusLog => {
            if text
                .to_ascii_lowercase()
                .contains(&expected.to_ascii_lowercase())
            {
                Verdict::Pass
            } else {
                Verdict::Fail(format!("`{expected}` not in `{}`", clip(text, 160)))
            }
        }
        Matcher::Regex => match Regex::new(expected) {
            Ok(re) if re.is_match(text) => Verdict::Pass,
            Ok(_) => Verdict::Fail(format!("/{expected}/ does not match `{}`", clip(text, 160))),
            Err(e) => Verdict::Inconclusive(format!("bad regex: {e}")),
        },
        Matcher::Number => {
            let Some(seen) = answer_number(text) else {
                return Verdict::Inconclusive(format!(
                    "no number after `>>` in `{}`",
                    clip(text, 160)
                ));
            };
            let Ok(want) = expected.trim().parse::<f64>() else {
                return Verdict::Inconclusive(format!("bad expected number `{expected}`"));
            };
            let tol = step.tolerance.unwrap_or(0.0);
            if (seen - want).abs() <= tol {
                Verdict::Pass
            } else {
                Verdict::Fail(format!("{seen} is not within {tol} of {want}"))
            }
        }
    }
}

/// The console's failure lines (game-20261101-T1 rows i to l, 2026-11-01-T03): a form
/// the parameter does not resolve, a command that does not exist, a save that is not
/// there. ASCII case ignored.
const FAILURE_LINES: &str = concat!(
    r#"(?i)(item\s+'[^'\r\n]*'\s+not\s+found\s+for\s+parameter[^\r\n]*"#,
    r#"|script\s+command\s+"[^"\r\n]*"\s+not\s+found[^\r\n]*"#,
    r#"|save\s+file\s+not\s+found[^\r\n]*)"#
);

/// Whether `output` holds nothing but the console's echo of `command` (the command line, or
/// `Command: <command>`): no result to judge (2026-11-03-T02/T03: `help ...` and
/// `player.getincell` right after a `coc` answered so, under build load).
pub fn echo_only(command: &str, output: &str) -> bool {
    let command = command.trim();
    // An empty answer is no output at all, judged as such, not an echo.
    output.lines().any(|l| !l.trim().is_empty())
        && output.lines().map(str::trim).all(|line| {
            line.is_empty()
                || line.eq_ignore_ascii_case(command)
                || line
                    .strip_prefix("Command:")
                    .is_some_and(|rest| rest.trim().eq_ignore_ascii_case(command))
                || line.strip_prefix(command).is_some_and(|rest| {
                    let rest = rest.trim();
                    rest.is_empty()
                        || rest
                            .strip_prefix("Command:")
                            .is_some_and(|r| r.trim().eq_ignore_ascii_case(command))
                })
        })
}

/// The first console failure line in the answer `output` to `command`, trimmed: `Item
/// 'X' not found for parameter ObjectReferenceID.`, `Script command "x" not found.`,
/// `Save file not found.`. The command's own text (its echo and the `Command: <command>`
/// line) is taken out first, so a command that spells such words is never read as its
/// failure. A step whose answer has one failed, whatever its matcher says: a `load` that
/// found no save leaves the game where it was, and a later getter would pass on the old
/// state (2026-11-01-T03, row T-05).
pub fn console_failure(command: &str, output: &str) -> Option<String> {
    let re = Regex::new(FAILURE_LINES).ok()?;
    let command = command.trim();
    output.lines().find_map(|line| {
        // Where the command's text is in the line: a match starting there is the echo.
        let echoes: Vec<(usize, usize)> = if command.is_empty() {
            Vec::new()
        } else {
            line.match_indices(command)
                .filter_map(|(at, c)| Some((at, at.checked_add(c.len())?)))
                .collect()
        };
        let mut from = 0;
        while let Some(m) = re.find_at(line, from) {
            match echoes
                .iter()
                .find(|(a, b)| *a <= m.start() && m.start() < *b)
            {
                Some((_, end)) => from = *end,
                None => return Some(m.as_str().trim().to_string()),
            }
        }
        None
    })
}

/// The number a console getter answered: the one right after the last `>>` in `text`
/// (`GetPos: X >> -17.00`). An answer without `>>` (only the echo, an error) has none, so
/// a digit of the echoed command (`player.moveto 06001000`) is never read as the answer.
pub fn answer_number(text: &str) -> Option<f64> {
    let re = Regex::new(r">>\s*(-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?)").ok()?;
    re.captures_iter(text)
        .last()
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

/// `text` cut to `n` characters, newlines as spaces.
pub fn clip(text: &str, n: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let flat = flat.trim();
    if flat.chars().count() <= n {
        flat.to_string()
    } else {
        let mut s: String = flat.chars().take(n).collect();
        s.push_str("...");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(command: &str, m: Option<Matcher>, expected: &str, tol: Option<f64>) -> Step {
        Step {
            command: command.into(),
            matcher: m,
            expected: (!expected.is_empty()).then(|| expected.into()),
            tolerance: tol,
            why: None,
            wait_secs: None,
        }
    }

    #[test]
    fn matchers_pass_fail_and_missing_evidence_is_inconclusive() {
        let c = step("help X 4", Some(Matcher::Contains), "0021bbcd", None);
        assert_eq!(
            judge(&c, Some("WEAP: Rifle (0021BBCD)"), None),
            Verdict::Pass
        );
        assert_eq!(judge(&c, Some("nothing"), None).label(), "fail");
        assert_eq!(judge(&c, None, None).label(), "inconclusive");
        assert_eq!(judge(&c, Some("  \r\n"), None).label(), "inconclusive");

        let n = step("getpos x", Some(Matcher::Number), "-17.0", Some(0.5));
        assert_eq!(judge(&n, Some("GetPos: X >> -17.25"), None), Verdict::Pass);
        // The observed shape (game-20261101-T1): echo, `Command: <cmd>`, the result.
        assert_eq!(
            judge(
                &n,
                Some("getpos x\nCommand: getpos x\nGetPos: X >> -17.00\n"),
                None
            ),
            Verdict::Pass
        );
        // Only the echo (the game was still moving): no answer, whatever digits it has.
        let d = step(
            "player.getitemcount 06001000",
            Some(Matcher::Number),
            "6001000",
            None,
        );
        let echo = "player.getitemcount 06001000\nCommand: player.getitemcount 06001000";
        assert_eq!(judge(&d, Some(echo), None).label(), "inconclusive");
        assert_eq!(judge(&n, Some("GetPos: X >> 3.0"), None).label(), "fail");
        assert_eq!(
            judge(&n, Some("Script command not found"), None).label(),
            "inconclusive"
        );

        let r = step("getstage Q", Some(Matcher::Regex), r">>\s*\d+", None);
        assert_eq!(judge(&r, Some("GetStage >> 10"), None), Verdict::Pass);
        assert_eq!(judge(&r, Some("error"), None).label(), "fail");

        let p = step("", Some(Matcher::PapyrusLog), "KitKraft: hello", None);
        assert_eq!(judge(&p, None, Some("[x] KitKraft: hello")), Verdict::Pass);
        assert_eq!(judge(&p, None, Some("other")).label(), "fail");
        assert_eq!(judge(&p, Some("out"), None).label(), "inconclusive");

        let setup = step("coc X", None, "", None);
        assert_eq!(judge(&setup, None, None), Verdict::Pass);
    }

    #[test]
    fn combine_orders_fail_inconclusive_not_run_pass() {
        let f = Verdict::Fail("a".into());
        let i = Verdict::Inconclusive("b".into());
        let n = Verdict::NotRun("c".into());
        assert_eq!(Verdict::combine(&[Verdict::Pass, i.clone(), f.clone()]), f);
        assert_eq!(Verdict::combine(&[n.clone(), i.clone()]), i);
        assert_eq!(Verdict::combine(&[Verdict::Pass, n.clone()]), n);
        assert_eq!(Verdict::combine(&[Verdict::Pass]), Verdict::Pass);
        assert_eq!(Verdict::combine(&[]).label(), "inconclusive");
        assert_eq!(f.text(), "fail (a)");
        assert_eq!(
            serde_json::to_value(&i).unwrap(),
            serde_json::json!({"status": "inconclusive", "reason": "b"})
        );
    }

    #[test]
    fn probe_files_round_trip_and_are_checked() {
        let file = ProbeFile {
            format: FORMAT,
            plugin: "X.esm".into(),
            sha256: "00".into(),
            generated_by: "test".into(),
            profile: "KitKraft Dev".into(),
            runtime_id_basis: "inferred".into(),
            notes: vec!["n".into()],
            probes: vec![
                Probe {
                    row: "T-01".into(),
                    title: "t".into(),
                    forms: vec!["X.esm:000800".into()],
                    screenshot: true,
                    matcher: None,
                    reason: None,
                    steps: vec![
                        step("prid X", None, "", None),
                        step("getpos x", Some(Matcher::Number), "1.5", Some(0.5)),
                    ],
                },
                Probe {
                    row: "T-02".into(),
                    title: "m".into(),
                    forms: vec![],
                    screenshot: false,
                    matcher: Some("manual".into()),
                    reason: Some("needs a look".into()),
                    steps: vec![],
                },
            ],
        };
        let text = file.to_toml().unwrap();
        assert!(
            text.contains("[[probe]]") && text.contains("[[probe.step]]"),
            "{text}"
        );
        assert_eq!(ProbeFile::parse(&text).unwrap(), file);

        let mut dup = file.clone();
        dup.probes[1].row = "T-01".into();
        assert!(dup.check().is_err());
        let mut no_reason = file.clone();
        no_reason.probes[1].reason = None;
        assert!(no_reason.check().is_err());
        let mut two_lines = file.clone();
        two_lines.probes[0].steps[0].command = "prid X\nqqq".into();
        assert!(two_lines.check().is_err());
        let mut bad_number = file.clone();
        bad_number.probes[0].steps[1].expected = Some("x".into());
        assert!(bad_number.check().is_err());
        let mut no_matcher = file.clone();
        no_matcher.probes[0].steps[1].matcher = None;
        assert!(no_matcher.check().is_err());
        let mut long_wait = file.clone();
        long_wait.probes[0].steps[0].wait_secs = Some(MAX_WAIT_SECS + 1);
        assert!(long_wait.check().is_err());
        let mut waits = file.clone();
        waits.probes[0].steps[0].wait_secs = Some(10);
        let text = waits.to_toml().unwrap();
        assert!(text.contains("wait_secs = 10"), "{text}");
        assert_eq!(ProbeFile::parse(&text).unwrap(), waits);
        let mut bad_regex = file;
        bad_regex.probes[0].steps[1].matcher = Some(Matcher::Regex);
        bad_regex.probes[0].steps[1].expected = Some("(".into());
        assert!(bad_regex.check().is_err());
    }

    /// The echo-only answers of 2026-11-03-T02 and -T03, and answers that hold a result.
    #[test]
    fn an_answer_with_only_the_echo_is_told_apart() {
        assert!(echo_only(
            "help \"Old Soviet Marksman Rifle\" 4",
            "help \"Old Soviet Marksman Rifle\" 4"
        ));
        assert!(echo_only(
            "player.getincell 0006BB29",
            "player.getincell 0006BB29
Command: player.getincell 0006BB29
"
        ));
        assert!(echo_only(
            "player.getincell 0006BB29",
            "player.getincell 0006BB29 Command: player.getincell 0006BB29"
        ));
        assert!(!echo_only("getpos x", ""), "no output is not an echo");
        assert!(!echo_only(
            "player.getincell 0006BB29",
            "player.getincell 0006BB29
Command: player.getincell 0006BB29
GetInCell >> 1.00"
        ));
        assert!(!echo_only(
            "getpos x",
            "getpos x
GetPos: X >> -17.00"
        ));
        assert!(!echo_only(
            "help KitKraftCrate 4",
            "REFR: KitKraftCrate (04001000)"
        ));
    }

    #[test]
    fn console_failure_lines_are_found_and_the_echo_is_not() {
        // The observed shapes (game-20261101-T1 rows i, k, l; 2026-11-01-T03 row T-05).
        let load = "load KitKraftSession-20261101-03\nCommand: load KitKraftSession-20261101-03\nSave file not found.\n";
        assert_eq!(
            console_failure("load KitKraftSession-20261101-03", load).as_deref(),
            Some("Save file not found.")
        );
        let prid = "prid KitKraftFirstPOICrate\nCommand: prid KitKraftFirstPOICrate\nItem 'KitKraftFirstPOICrate' not found for parameter ObjectReferenceID.\n";
        assert_eq!(
            console_failure("prid KitKraftFirstPOICrate", prid).as_deref(),
            Some("Item 'KitKraftFirstPOICrate' not found for parameter ObjectReferenceID.")
        );
        let missing = "isdisabled\nCommand: isdisabled\nScript command \"isdisabled\" not found.";
        assert_eq!(
            console_failure("isdisabled", missing).as_deref(),
            Some("Script command \"isdisabled\" not found.")
        );
        // On one line (the clipped form) too.
        assert_eq!(
            console_failure("load S", "load S Command: load S Save file not found.").as_deref(),
            Some("Save file not found.")
        );
        // Answers without one.
        let ok =
            "player.getincell 0006BB29\nCommand: player.getincell 0006BB29\nGetInCell >> 1.00\n";
        assert_eq!(console_failure("player.getincell 0006BB29", ok), None);
        assert_eq!(console_failure("save S", "save S\nCommand: save S\n"), None);
        assert_eq!(console_failure("getpos x", ""), None);
        // A command that spells the words is not its own failure.
        let odd = "help save file not found\nCommand: help save file not found\n";
        assert_eq!(console_failure("help save file not found", odd), None);
        assert_eq!(
            console_failure(
                "help save file not found",
                "help save file not found Command: help save file not found Save file not found."
            )
            .as_deref(),
            Some("Save file not found.")
        );
    }

    #[test]
    fn numbers_and_clipping() {
        assert_eq!(answer_number("GetPos: X >> -17.00"), Some(-17.0));
        assert_eq!(answer_number("A >> 1\nGetDistance >> 9.48"), Some(9.48));
        assert_eq!(answer_number("GetInCell >> 1.00"), Some(1.0));
        assert_eq!(answer_number("player.moveto 06001000"), None);
        assert_eq!(
            answer_number("Script command \"isdisabled\" not found."),
            None
        );
        assert_eq!(answer_number("none"), None);
        assert_eq!(clip("a\nb", 10), "a b");
        assert_eq!(clip("abcdef", 3), "abc...");
    }
}
