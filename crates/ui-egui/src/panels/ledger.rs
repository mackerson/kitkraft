//! Ledger panel (ADR 0007 Amendment 2): the workbench's one write. It records an in-game
//! result as a row of `docs/game-observations.md` through the engine's `ledger` command:
//! which check (`C1`..`C14`, `POI`, `persistence`, `custom`), `pass`/`fail`/`note`, the
//! FormIDs and a note. **Preview** runs `ledger` without `confirm` (nothing is written);
//! **Append** asks once more, and only the confirm click builds the confirmed request
//! ([`LedgerForm::append_request`]) the gate lets write. The engine refuses any other
//! path and the `Default` profile. Rows a game session left `inconclusive` or `manual:`
//! fill the form through [`LedgerForm::from_probe_row`], so the owner only checks and
//! confirms (M6.G6).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::envelope::{list, text, text_or_empty};
use crate::gate::Request;
use crate::md::{parse, plain, Block, Inline};
use crate::panels::help::in_checkout;

/// The ledger in a checkout.
pub const LEDGER_DOC: &str = "docs/game-observations.md";
/// The profile in-game checks run on (AGENTS.md).
pub const DEV_PROFILE: &str = "KitKraft Dev";
/// The rows the engine accepts (its `row` enum).
pub const ROWS: &[&str] = &[
    "C1",
    "C2",
    "C3",
    "C4",
    "C5",
    "C6",
    "C7",
    "C8",
    "C9",
    "C10",
    "C11",
    "C12",
    "C13",
    "C14",
    "POI",
    "persistence",
    "custom",
];
/// The results (its `result` enum).
pub const RESULTS: &[&str] = &["pass", "fail", "note"];

/// The ledger file of the checkout at `root`.
pub fn ledger_path(root: &Path) -> PathBuf {
    in_checkout(root, LEDGER_DOC)
}

/// The panel's inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerForm {
    /// The ledger file (`<checkout>\docs\game-observations.md`).
    pub file: String,
    /// One of [`ROWS`].
    pub row: String,
    /// One of [`RESULTS`].
    pub result: String,
    /// FormIDs, separated by commas, semicolons or new lines.
    pub forms: String,
    /// What was seen.
    pub note: String,
    /// What was checked (`custom` rows; optional otherwise).
    pub label: String,
    /// The profile the check ran on.
    pub profile: String,
    /// `yyyy-mm-dd`, or empty for today (UTC, the engine's clock).
    pub date: String,
}

impl Default for LedgerForm {
    fn default() -> Self {
        Self {
            file: String::new(),
            row: "C1".to_string(),
            result: "pass".to_string(),
            forms: String::new(),
            note: String::new(),
            label: String::new(),
            profile: DEV_PROFILE.to_string(),
            date: String::new(),
        }
    }
}

impl LedgerForm {
    /// The FormIDs as a list.
    pub fn forms_list(&self) -> Vec<String> {
        self.forms
            .split([',', ';', '\n'])
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn args(&self, confirm: bool) -> Result<Value, String> {
        let file = self.file.trim();
        if file.is_empty() {
            return Err(format!(
                "no ledger file: the Help tab's checkout gives {LEDGER_DOC}"
            ));
        }
        let mut a = Map::new();
        a.insert("file".into(), json!(file));
        a.insert("row".into(), json!(self.row));
        a.insert("result".into(), json!(self.result));
        let forms = self.forms_list();
        if !forms.is_empty() {
            a.insert("forms".into(), json!(forms));
        }
        for (k, v) in [
            ("note", &self.note),
            ("label", &self.label),
            ("profile", &self.profile),
            ("date", &self.date),
        ] {
            if !v.trim().is_empty() {
                a.insert(k.into(), json!(v.trim()));
            }
        }
        if confirm {
            a.insert("confirm".into(), json!(true));
        }
        Ok(Value::Object(a))
    }

    /// `ledger` without `confirm`: the engine returns the row and writes nothing.
    pub fn preview_request(&self) -> Result<Request, String> {
        Ok(Request::new("ledger", self.args(false)?))
    }

    /// `ledger` with `confirm: true`, as a confirmed request. Called only by the confirm
    /// button's click, once per click.
    pub fn append_request(&self) -> Result<Request, String> {
        Ok(Request::confirmed("ledger", self.args(true)?))
    }

    /// A ledger form from a probe row the session controller could not decide (M6.G6):
    /// `Some` only for a verdict `inconclusive`, or `not-run` with a reason starting
    /// `manual:` (a manual row, including one that took a screenshot for the owner);
    /// `None` for `pass`, `fail` and other `not-run` rows, and for a row whose verdict
    /// is missing or not a string. The form is a `custom` row with result `note` (never
    /// `pass`: the owner picks pass or fail after looking), its label
    /// `<session> <row id>: <title>`, the row's forms one per line, and a note saying
    /// what was expected, what was seen, the verdict and its reason, and each of the
    /// row's screenshots by its listed path (a name not listed is named `not listed`).
    /// The note and label are cut at a character boundary to the engine's caps
    /// (2000 and 200 characters, ending in `...`).
    pub fn from_probe_row(
        session: &str,
        row: &Value,
        screenshots: &Value,
        file: &str,
    ) -> Option<Self> {
        let verdict = row.get("verdict")?;
        let status = verdict.get("status")?.as_str()?;
        let reason = text_or_empty(verdict, "reason");
        let undecided = match status {
            "inconclusive" => true,
            "not-run" => reason.starts_with("manual:"),
            _ => false,
        };
        if !undecided {
            return None;
        }
        let id = text_or_empty(row, "row");
        let title = text_or_empty(row, "title");
        let expected = text_or_empty(row, "expected");
        let seen = text_or_empty(row, "seen");
        let forms = list(row, "forms")
            .iter()
            .filter_map(Value::as_str)
            .filter(|f| !f.trim().is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let shots = list(row, "screenshots")
            .iter()
            .filter_map(Value::as_str)
            .filter(|n| !n.trim().is_empty())
            .map(|name| {
                let path = screenshots
                    .as_array()
                    .and_then(|a| {
                        a.iter()
                            .find(|e| e.get("file").and_then(Value::as_str) == Some(name))
                    })
                    .and_then(|e| e.get("path"))
                    .and_then(Value::as_str)
                    .unwrap_or("not listed");
                format!("{name}: {path}")
            })
            .collect::<Vec<_>>()
            .join("; ");
        let suffix = if reason.trim().is_empty() {
            String::new()
        } else {
            format!(" ({})", reason.trim())
        };
        let verdict_text = format!("{status}{suffix}");
        let note = format!(
            "expected: {}\nseen: {}\nverdict: {}\nscreenshots: {}",
            if expected.trim().is_empty() {
                "-"
            } else {
                expected.trim()
            },
            if seen.trim().is_empty() {
                "-"
            } else {
                seen.trim()
            },
            verdict_text,
            if shots.is_empty() {
                "none"
            } else {
                shots.as_str()
            },
        );
        Some(Self {
            file: file.to_string(),
            row: "custom".to_string(),
            result: "note".to_string(),
            forms,
            note: cut(&note, 2000),
            label: cut(&format!("{session} {id}: {title}"), 200),
            profile: DEV_PROFILE.to_string(),
            date: String::new(),
        })
    }
}

/// `s` cut at a character boundary to at most `max` characters; longer text ends in
/// `...` (the engine's `ledger` caps `note` at 2000 and `label` at 200).
fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = s.chars().take(max.saturating_sub(3)).collect::<String>();
    out.push_str("...");
    out
}

/// The forms for every undecided row of a `game.status` result, in order: `data.rows`
/// and then `data.reruns`, each through [`LedgerForm::from_probe_row`] (so only the
/// `inconclusive` and `manual:` rows come out).
pub fn probe_rows_to_record(status_env: &Value, file: &str) -> Vec<LedgerForm> {
    let d = status_env.get("data").unwrap_or(&Value::Null);
    let session = text_or_empty(d, "session");
    let shots = d.get("screenshots").unwrap_or(&Value::Null);
    let mut out = Vec::new();
    for key in ["rows", "reruns"] {
        for row in list(d, key) {
            if let Some(form) = LedgerForm::from_probe_row(&session, row, shots, file) {
                out.push(form);
            }
        }
    }
    out
}

/// The `ledger` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LedgerView {
    /// The file.
    pub file: String,
    /// `game-<yyyymmdd>-<nn>`.
    pub entry: String,
    /// The entry was (or would be) created.
    pub new_entry: bool,
    /// The row line.
    pub row: String,
    /// The full text appended (or that would be).
    pub text: String,
    /// It was written.
    pub appended: bool,
}

impl LedgerView {
    /// Reads a `ledger` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let d = env.get("data").unwrap_or(&Value::Null);
        let flag = |k: &str| d.get(k).and_then(Value::as_bool).unwrap_or(false);
        Self {
            file: text_or_empty(d, "file"),
            entry: text_or_empty(d, "entry"),
            new_entry: flag("new_entry"),
            row: text_or_empty(d, "row"),
            text: text(d, "text").unwrap_or_default(),
            appended: flag("appended"),
        }
    }
}

/// A row of the ledger's session templates, for the panel to show what the chosen check
/// expects and to fill in its FormIDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRow {
    /// `O5-03`.
    pub id: String,
    /// The template's Check or Step cell.
    pub check: String,
    /// The `Plugin.esm:XXXXXX` code spans of its Forms cell.
    pub forms: Vec<String>,
    /// Its Expected cell.
    pub expected: String,
}

/// The template row a ledger row stands for: `C<n>` is session O5's `O5-<nn>`, `POI` is
/// O2-06 (the crate). `None` for the others.
pub fn template_id(row: &str) -> Option<String> {
    if let Some(n) = row.strip_prefix('C').and_then(|n| n.parse::<u8>().ok()) {
        return Some(format!("O5-{n:02}"));
    }
    (row == "POI").then(|| "O2-06".to_string())
}

/// The first table row of `observations` (the ledger's text) whose first cell is
/// `template_id(row)`.
pub fn template_row(observations: &str, row: &str) -> Option<TemplateRow> {
    let id = template_id(row)?;
    for block in parse(observations) {
        let Block::Table { rows, .. } = block else {
            continue;
        };
        for r in rows {
            if r.first().map(|c| plain(c)) != Some(id.clone()) {
                continue;
            }
            let cell = |i: usize| r.get(i).map(|c| plain(c)).unwrap_or_default();
            let forms = r
                .get(2)
                .map(|c| {
                    c.iter()
                        .filter_map(|run| match run {
                            Inline::Code(s)
                                if [".esm:", ".esp:", ".esl:"]
                                    .iter()
                                    .any(|ext| s.to_ascii_lowercase().contains(ext)) =>
                            {
                                Some(s.clone())
                            }
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            return Some(TemplateRow {
                id,
                check: cell(1),
                forms,
                expected: cell(3),
            });
        }
    }
    None
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, ComboBox, RichText, TextEdit};

use crate::panels::common::{action_bar, envelope_block, panel_head, Action, Slot, TabCtx};
use crate::theme::{self, Field, Status};

/// The Ledger tab's state: the ledger's text as last read (its template rows), the form,
/// the last result, the row waiting for its confirm click, and the session row the form
/// was prefilled from (M6.G6).
#[derive(Default)]
pub(crate) struct LedgerTab {
    pub observations: Option<String>,
    pub form: LedgerForm,
    pub view: Option<LedgerView>,
    pub pending: Option<LedgerForm>,
    /// One line saying the form came from a session row (its `prefill` set it).
    pub prefill_source: Option<String>,
}

impl LedgerTab {
    /// Fills the form from a probe row (M6.G6): replaces the form, forgets any preview
    /// and any confirm box that was open, and remembers the session row it came from,
    /// shown as one line. Prefilling never writes and never confirms: the owner still
    /// clicks Preview, then Append, then the confirm button, and only that click builds
    /// the writing request (ADR 0007 Amendment 2). The Cockpit tab's button calls this;
    /// until that ticket it is used only by tests here.
    #[allow(dead_code)]
    pub(crate) fn prefill(&mut self, form: LedgerForm) {
        // The label is `<session> <row id>: <title>`; the first two words are the source.
        let source: String = form
            .label
            .split_whitespace()
            .take(2)
            .map(|w| w.strip_suffix(':').unwrap_or(w))
            .collect::<Vec<_>>()
            .join(" ");
        self.prefill_source =
            (!source.is_empty()).then(|| format!("prefilled from session row {source}"));
        self.form = form;
        self.view = None;
        self.pending = None;
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let actions = &mut ctx.actions;
        panel_head(
            ui,
            "Ledger",
            "Records one in-game result as a row of docs/game-observations.md through the engine's `ledger` command: the workbench's only write (ADR 0007 Amendment 2). Preview writes nothing; Append asks once more, and each confirm click appends one row. Checks run on KitKraft Dev with a disposable save; the engine refuses Default and any other file.",
        );
        if let Some(src) = &self.prefill_source {
            ui.weak(src);
        }
        let template = self
            .observations
            .as_deref()
            .and_then(|t| template_row(t, &self.form.row));
        let form = &mut self.form;
        let pending = &mut self.pending;
        let read = self.observations.is_some();
        let wide = theme::width(ui, Field::Wide);
        action_bar(ui, |ui| {
            theme::form_grid(ui, "ledger-form").show(ui, |ui| {
                ui.label("Ledger file");
                ui.add(TextEdit::singleline(&mut form.file).desired_width(wide));
                ui.end_row();
                ui.label("Row");
                ui.horizontal(|ui| {
                    ComboBox::from_id_salt("ledger-row")
                        .selected_text(form.row.clone())
                        .show_ui(ui, |ui| {
                            for r in ROWS {
                                ui.selectable_value(&mut form.row, (*r).to_string(), *r);
                            }
                        });
                    for r in RESULTS {
                        ui.radio_value(&mut form.result, (*r).to_string(), *r);
                    }
                });
                ui.end_row();
                ui.label(if form.row == "custom" {
                    "What was checked"
                } else {
                    "Label (optional)"
                });
                ui.add(TextEdit::singleline(&mut form.label).desired_width(wide));
                ui.end_row();
                ui.label("FormIDs");
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut form.forms)
                            .desired_width(theme::width(ui, Field::Long))
                            .hint_text("Starfield.esm:2984DF, ... (commas)"),
                    );
                    if let Some(t) = &template {
                        if ui
                            .add_enabled(!t.forms.is_empty(), egui::Button::new("from template"))
                            .on_hover_text(format!("the Forms of template row {}", t.id))
                            .clicked()
                        {
                            form.forms = t.forms.join(", ");
                        }
                    }
                });
                ui.end_row();
                ui.label("Seen (note)");
                ui.add(
                    TextEdit::multiline(&mut form.note)
                        .desired_rows(3)
                        .desired_width(wide),
                );
                ui.end_row();
                ui.label("Profile");
                ui.add(
                    TextEdit::singleline(&mut form.profile)
                        .desired_width(theme::width(ui, Field::Short)),
                );
                ui.end_row();
                ui.label("Date");
                ui.add(
                    TextEdit::singleline(&mut form.date)
                        .desired_width(theme::width(ui, Field::Medium))
                        .hint_text("yyyy-mm-dd (empty: today, UTC)"),
                );
                ui.end_row();
            });
            match &template {
                Some(t) => {
                    ui.label(format!("Template row {}: {}", t.id, t.check));
                    ui.weak(format!("expected: {}", t.expected));
                }
                None if !read => {
                    theme::status(
                        ui,
                        Status::Warn,
                        "the ledger was not read: no checkout found (see the Help tab)",
                    );
                }
                None => {
                    ui.weak("no template row for this check");
                }
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("Preview"))
                    .on_hover_text("`ledger` without confirm: shows the text, writes nothing")
                    .clicked()
                {
                    match form.preview_request() {
                        Ok(req) => actions.push(Action::Submit(Slot::Ledger, req)),
                        Err(e) => *ctx.notice = Some(e),
                    }
                }
                if ui
                    .add_enabled(
                        !busy && pending.is_none(),
                        egui::Button::new("Append to the ledger..."),
                    )
                    .clicked()
                {
                    *pending = Some(form.clone());
                }
            });
        });
        let mut cancel = false;
        if let Some(p) = &self.pending {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                theme::status_line(
                    ui,
                    Status::Warn,
                    &format!("Append this row to {}? This is the workbench's only write; one row per click.", p.file),
                );
                ui.monospace(format!(
                    "{} {}  forms: {}  profile: {}  date: {}",
                    p.row,
                    p.result,
                    if p.forms.trim().is_empty() { "-" } else { p.forms.trim() },
                    p.profile.trim(),
                    if p.date.trim().is_empty() { "today (UTC)" } else { p.date.trim() }
                ));
                if !p.label.trim().is_empty() {
                    ui.label(format!("label: {}", p.label.trim()));
                }
                ui.label(format!("seen: {}", if p.note.trim().is_empty() { "-" } else { p.note.trim() }));
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!busy, egui::Button::new(RichText::new("Confirm: append").strong()))
                        .clicked()
                    {
                        actions.push(Action::LedgerAppend(p.clone()));
                        cancel = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        }
        if cancel {
            self.pending = None;
        }
        envelope_block(ui, ctx.results.get(&Slot::Ledger.into()), |ui| {
            let Some(v) = &self.view else { return };
            if v.appended {
                theme::status(
                    ui,
                    Status::Pass,
                    &format!("appended to {} in {}", v.entry, v.file),
                );
            } else {
                theme::status(
                    ui,
                    Status::NotRun,
                    &format!(
                        "preview only (nothing written): it would go into {}{}",
                        v.entry,
                        if v.new_entry { " (a new entry)" } else { "" }
                    ),
                );
            }
            egui::Frame::group(ui.style()).show(ui, |ui| {
                for l in v.text.lines() {
                    ui.monospace(l);
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_is_unconfirmed_and_append_is_confirmed() {
        let form = LedgerForm {
            file: r"C:\kk\docs\game-observations.md".into(),
            row: "C3".into(),
            forms: "Starfield.esm:2984DF, Starfield.esm:1B01CA\n".into(),
            note: " 24 rounds ".into(),
            ..LedgerForm::default()
        };
        let p = form.preview_request().unwrap();
        assert!(!p.is_confirmed());
        assert_eq!(
            p.args,
            json!({"file": r"C:\kk\docs\game-observations.md", "row": "C3", "result": "pass",
                   "forms": ["Starfield.esm:2984DF", "Starfield.esm:1B01CA"], "note": "24 rounds",
                   "profile": "KitKraft Dev"})
        );
        crate::gate::check(&p).unwrap();
        let a = form.append_request().unwrap();
        assert!(a.is_confirmed());
        assert_eq!(a.args["confirm"], true);
        crate::gate::check(&a).unwrap();
        // The same arguments without the confirm click are refused.
        assert!(crate::gate::check(&Request::new("ledger", a.args)).is_err());
        assert!(LedgerForm::default().preview_request().is_err());
    }

    #[test]
    fn rows_match_the_engine_enum() {
        let spec = crate::gate::find_command("ledger").unwrap();
        let schema: Value = serde_json::from_str(spec.args_schema).unwrap();
        let rows: Vec<&str> = schema["properties"]["row"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(rows, ROWS);
        let results: Vec<&str> = schema["properties"]["result"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(results, RESULTS);
    }

    /// The repo's ledger has the templates the panel reads.
    #[test]
    fn template_rows_come_from_the_repo_ledger() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let text = std::fs::read_to_string(ledger_path(root)).unwrap();
        let c3 = template_row(&text, "C3").unwrap();
        assert_eq!(c3.id, "O5-03");
        assert!(c3.check.starts_with("C3:"), "{}", c3.check);
        assert_eq!(
            c3.forms,
            [
                "Starfield.esm:2984DF",
                "Starfield.esm:1B01CA",
                "Starfield.esm:077F13"
            ]
        );
        assert!(c3.expected.contains("24 rounds"));
        for n in 1..=14 {
            assert!(template_row(&text, &format!("C{n}")).is_some(), "C{n}");
        }
        let poi = template_row(&text, "POI").unwrap();
        assert_eq!(poi.forms, ["KitKraftFirstPOI.esm:001000"]);
        assert!(template_row(&text, "persistence").is_none());
        assert!(template_row(&text, "custom").is_none());
    }

    #[test]
    fn view_from_a_canned_envelope() {
        let v =
            LedgerView::from_envelope(&json!({"data": {"file": "f", "entry": "game-20261016-01",
            "new_entry": true, "row": "| C3 |", "text": "### x\n| C3 |\n", "appended": false}}));
        assert_eq!(v.entry, "game-20261016-01");
        assert!(v.new_entry && !v.appended);
        assert!(v.text.contains("| C3 |"));
    }

    /// Through the gate and the engine on a scratch checkout: the preview writes nothing,
    /// the confirmed request appends one row.
    #[test]
    fn preview_then_confirmed_append_on_a_scratch_checkout() {
        let dir = std::env::temp_dir().join(format!("kitkraft-ui-ledger-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "x").unwrap();
        let file = ledger_path(&dir);
        let before = "# Game observations\n\n## Ledger\n";
        std::fs::write(&file, before).unwrap();
        let form = LedgerForm {
            file: file.display().to_string(),
            row: "POI".into(),
            forms: "KitKraftFirstPOI.esm:001000".into(),
            date: "2026-10-16".into(),
            ..LedgerForm::default()
        };
        let engine = crate::gate::ReadOnlyEngine::new();
        let preview = engine.run(&form.preview_request().unwrap()).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        let pv = LedgerView::from_envelope(&preview);
        assert!(!pv.appended && pv.new_entry);
        let unconfirmed = Request::new("ledger", form.append_request().unwrap().args);
        assert!(engine.run(&unconfirmed).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        let done = engine.run(&form.append_request().unwrap()).unwrap();
        let after = std::fs::read_to_string(&file).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(LedgerView::from_envelope(&done).appended);
        assert_eq!(after, format!("{before}{}", pv.text));
        assert!(after.contains("| `KitKraftFirstPOI.esm:001000` |"));
    }

    /// M6.G6: a row the controller could not decide prefills the form for the owner.
    #[test]
    fn a_probe_row_left_inconclusive_prefills_the_form() {
        let file = r"C:\kk\docs\game-observations.md";
        let row = json!({
            "row": "T-03",
            "title": "the crate is on the shelf",
            "forms": ["X.esm:001000"],
            "commands": [],
            "expected": "the crate is on the shelf",
            "seen": "",
            "verdict": {"status": "inconclusive",
                        "reason": "manual: needs a look; screenshot T-03-1.png is for the owner"},
            "screenshots": ["T-03-1.png"]
        });
        let shots = json!([{"file": "T-03-1.png", "path": "observe/2026-10-25-T01/T-03-1.png"}]);
        let f = LedgerForm::from_probe_row("2026-10-25-T01", &row, &shots, file).unwrap();
        assert_eq!(f.row, "custom");
        assert_eq!(f.result, "note");
        assert_eq!(f.label, "2026-10-25-T01 T-03: the crate is on the shelf");
        assert_eq!(f.forms_list(), ["X.esm:001000"]);
        assert!(
            f.note.contains("expected: the crate is on the shelf"),
            "{}",
            f.note
        );
        assert!(f.note.contains("verdict: inconclusive"), "{}", f.note);
        assert!(
            f.note.contains("observe/2026-10-25-T01/T-03-1.png"),
            "{}",
            f.note
        );
        assert_eq!(f.profile, DEV_PROFILE);
        assert_eq!(f.file, file);
        // The note and label are cut at a character boundary to the engine's caps
        // (2000 and 200 characters), even for multi-byte text.
        let long = json!({
            "row": "T-11",
            "title": "é".repeat(200),
            "forms": [],
            "commands": [],
            "expected": "é".repeat(2100),
            "seen": "",
            "verdict": {"status": "inconclusive", "reason": ""},
            "screenshots": []
        });
        let l = LedgerForm::from_probe_row("2026-10-25-T01", &long, &shots, file).unwrap();
        assert_eq!(l.note.chars().count(), 2000);
        assert!(l.note.ends_with("..."));
        assert_eq!(l.label.chars().count(), 200);
        assert!(l.label.ends_with("..."));
        // A not-run row with a manual: reason is prefilled too.
        let manual = json!({
            "row": "T-07",
            "title": "the map shows the POI",
            "forms": [],
            "commands": [],
            "expected": "the map shows the POI",
            "seen": "",
            "verdict": {"status": "not-run", "reason": "manual: the map UI"},
            "screenshots": []
        });
        let m = LedgerForm::from_probe_row("2026-10-25-T01", &manual, &shots, file).unwrap();
        assert_eq!(m.result, "note");
        assert!(
            m.note.contains("verdict: not-run (manual: the map UI)"),
            "{}",
            m.note
        );
        // A screenshot name not listed in the status is named as not listed.
        let unlisted = json!({
            "row": "T-09",
            "title": "t",
            "forms": [],
            "commands": [],
            "expected": "",
            "seen": "",
            "verdict": {"status": "inconclusive", "reason": ""},
            "screenshots": ["T-09-1.png"]
        });
        let u = LedgerForm::from_probe_row("2026-10-25-T01", &unlisted, &shots, file).unwrap();
        assert!(u.note.contains("T-09-1.png: not listed"), "{}", u.note);
        // Missing members: no verdict means no form; missing text means empty text.
        assert!(LedgerForm::from_probe_row("s", &json!({"row": "T-01"}), &shots, file).is_none());
        assert!(LedgerForm::from_probe_row("s", &Value::Null, &shots, file).is_none());
        let no_title = json!({"row": "T-02", "verdict": {"status": "inconclusive", "reason": "x"}});
        assert_eq!(
            LedgerForm::from_probe_row("s", &no_title, &shots, file)
                .unwrap()
                .label,
            "s T-02: "
        );
    }

    /// M6.G6: decided rows are not prefilled; only the undecided ones come out, in order.
    #[test]
    fn pass_and_fail_rows_are_not_prefilled() {
        let file = "f.md";
        for (status, reason) in [
            ("pass", ""),
            ("fail", "the console said no"),
            ("not-run", "stopped by game.stop before row T-02"),
        ] {
            let row = json!({"row": "T-02", "title": "t", "forms": [], "commands": [],
                             "expected": "", "seen": "",
                             "verdict": {"status": status, "reason": reason},
                             "screenshots": []});
            assert!(
                LedgerForm::from_probe_row("2026-10-25-T01", &row, &json!([]), file).is_none(),
                "{status} {reason}"
            );
        }
        let status = json!({
            "data": {
                "session": "2026-10-25-T01",
                "rows": [
                    {"row": "T-01", "title": "a", "forms": [], "expected": "a", "seen": "a",
                     "verdict": {"status": "pass", "reason": ""}, "screenshots": []},
                    {"row": "T-02", "title": "b", "forms": ["X.esm:001000"], "expected": "b", "seen": "b",
                     "verdict": {"status": "fail", "reason": "no"}, "screenshots": []},
                    {"row": "T-03", "title": "c", "forms": [], "expected": "c", "seen": "",
                     "verdict": {"status": "inconclusive", "reason": "manual: needs a look"},
                     "screenshots": ["T-03-1.png"]},
                    {"row": "T-04", "title": "d", "forms": [], "expected": "d", "seen": "",
                     "verdict": {"status": "not-run", "reason": "stopped by game.stop"},
                     "screenshots": []},
                    {"row": "T-05", "title": "e", "forms": [], "expected": "e", "seen": "",
                     "verdict": {"status": "not-run", "reason": "manual: the map UI"},
                     "screenshots": []}
                ],
                "reruns": [
                    {"row": "T-06", "title": "f", "forms": [], "expected": "f", "seen": "f",
                     "verdict": {"status": "inconclusive", "reason": "no console reply"},
                     "screenshots": []}
                ],
                "screenshots": [
                    {"file": "T-03-1.png", "path": "observe/2026-10-25-T01/T-03-1.png"}
                ]
            }
        });
        let forms = probe_rows_to_record(&status, file);
        let labels: Vec<&str> = forms.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "2026-10-25-T01 T-03: c",
                "2026-10-25-T01 T-05: e",
                "2026-10-25-T01 T-06: f",
            ]
        );
        assert!(forms[0].note.contains("observe/2026-10-25-T01/T-03-1.png"));
    }

    /// M6.G6: a prefilled row still goes through preview, append and its own confirm.
    #[test]
    fn a_prefilled_row_still_needs_its_own_confirm() {
        let row = json!({
            "row": "T-03",
            "title": "the crate is on the shelf",
            "forms": ["X.esm:001000"],
            "commands": [],
            "expected": "the crate is on the shelf",
            "seen": "",
            "verdict": {"status": "inconclusive",
                        "reason": "manual: needs a look; screenshot T-03-1.png is for the owner"},
            "screenshots": ["T-03-1.png"]
        });
        let shots = json!([{"file": "T-03-1.png", "path": "observe/2026-10-25-T01/T-03-1.png"}]);
        let f = LedgerForm::from_probe_row(
            "2026-10-25-T01",
            &row,
            &shots,
            r"C:\kk\docs\game-observations.md",
        )
        .unwrap();
        let p = f.preview_request().unwrap();
        assert!(!p.is_confirmed());
        crate::gate::check(&p).unwrap();
        let a = f.append_request().unwrap();
        assert!(a.is_confirmed());
        crate::gate::check(&a).unwrap();
        // Prefilling forgets any preview and any confirm box that was open.
        let mut tab = LedgerTab {
            view: Some(LedgerView::default()),
            pending: Some(f.clone()),
            ..LedgerTab::default()
        };
        tab.prefill(f.clone());
        assert!(tab.view.is_none());
        assert!(tab.pending.is_none());
        assert_eq!(
            tab.prefill_source.as_deref(),
            Some("prefilled from session row 2026-10-25-T01 T-03")
        );
    }
}
