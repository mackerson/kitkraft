//! Ledger panel (ADR 0007 Amendment 2): the workbench's one write. It records an in-game
//! result as a row of `docs/game-observations.md` through the engine's `ledger` command:
//! which check (`C1`..`C14`, `POI`, `persistence`, `custom`), `pass`/`fail`/`note`, the
//! FormIDs and a note. **Preview** runs `ledger` without `confirm` (nothing is written);
//! **Append** asks once more, and only the confirm click builds the confirmed request
//! ([`LedgerForm::append_request`]) the gate lets write. The engine refuses any other
//! path and the `Default` profile.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::envelope::{text, text_or_empty};
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
/// the last result, and the row waiting for its confirm click.
#[derive(Default)]
pub(crate) struct LedgerTab {
    pub observations: Option<String>,
    pub form: LedgerForm,
    pub view: Option<LedgerView>,
    pub pending: Option<LedgerForm>,
}

impl LedgerTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let actions = &mut ctx.actions;
        panel_head(
            ui,
            "Ledger",
            "Records one in-game result as a row of docs/game-observations.md through the engine's `ledger` command: the workbench's only write (ADR 0007 Amendment 2). Preview writes nothing; Append asks once more, and each confirm click appends one row. Checks run on KitKraft Dev with a disposable save; the engine refuses Default and any other file.",
        );
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
}
