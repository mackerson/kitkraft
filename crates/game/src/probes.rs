//! `probes.toml` (M4.T2): the machine-checkable half of a `kit testplan`, one probe per
//! checklist row, and the verdicts the controller reaches on them (ADR 0010 Amendment 1
//! condition 7).
//!
//! A probe is a row id (the checklist's), a title, the forms it is about, whether it wants
//! a screenshot, and an ordered list of steps. A step is one console command; a step with
//! a `matcher` is checked against that command's output (`contains`, `regex`, `number`
//! within `tolerance`) or against the Papyrus log (`papyrus-log`); a step without one is
//! setup. A row no console command can answer is `matcher = "manual"` with its `reason`.
//! `{save}` in a command is the session's own save name and `{base_save}` the base save's;
//! the controller fills both and refuses any other `save` or `load` target.
//!
//! Verdicts are exactly `not-run`, `pass`, `fail` and `inconclusive`, each but `pass`
//! with a reason. **Missing evidence is `inconclusive`, never `pass`:** no output to
//! match, no number in it, a log that was not written, a screenshot not taken.

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{GameError, Result};

/// The format version `kit testplan` writes.
pub const FORMAT: u32 = 1;

/// Longest console command accepted.
pub const MAX_COMMAND: usize = 1000;

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
}

/// How a step's output is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Matcher {
    /// The output contains `expected` (ASCII case ignored).
    Contains,
    /// The output matches the regular expression `expected`.
    Regex,
    /// The last number in the output is within `tolerance` of `expected`.
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
            let Some(seen) = last_number(text) else {
                return Verdict::Inconclusive(format!("no number in `{}`", clip(text, 160)));
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

/// The last decimal number in `text`.
pub fn last_number(text: &str) -> Option<f64> {
    let re = Regex::new(r"-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?").ok()?;
    re.find_iter(text)
        .filter_map(|m| m.as_str().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .last()
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
        let mut bad_regex = file;
        bad_regex.probes[0].steps[1].matcher = Some(Matcher::Regex);
        bad_regex.probes[0].steps[1].expected = Some("(".into());
        assert!(bad_regex.check().is_err());
    }

    #[test]
    fn numbers_and_clipping() {
        assert_eq!(last_number("GetPos >> 1 then -4.25e1"), Some(-42.5));
        assert_eq!(last_number("none"), None);
        assert_eq!(clip("a\nb", 10), "a b");
        assert_eq!(clip("abcdef", 3), "abc...");
    }
}
