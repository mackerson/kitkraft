//! The Warnings window (M7.M4): what loading the load order and validating a plugin
//! found, as one CK-style list (severity, source, record, message), with a filter and a
//! copy button; double-click a row to inspect its record. Every row comes from an
//! envelope the window already holds: the `load` view (missing and late masters, plugin
//! errors, active plugins with no file) with its limitations, and the Validate tab's
//! last `validate` findings. Severity is glyph, word and color (`theme::status`).

use crate::panels::load::LoadView;
use crate::panels::validate::ValidateView;
use crate::theme::Status;

/// How bad a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    /// A limitation the engine claimed: something it did not establish.
    Limitation,
}

impl Severity {
    /// The theme status that draws it (glyph, word and color).
    pub fn status(self) -> Status {
        match self {
            Severity::Error => Status::Fail,
            Severity::Warning => Status::Warn,
            Severity::Limitation => Status::Unknown,
        }
    }

    /// Its word.
    pub fn word(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Limitation => "limitation",
        }
    }
}

/// One line of the Warnings window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningRow {
    pub severity: Severity,
    /// `load` or `validate <rule>`.
    pub source: String,
    /// The plugin or record the row is about (a formref when it is a record).
    pub record: String,
    pub message: String,
    /// The record to inspect on double-click.
    pub inspect: Option<String>,
}

/// The rows from a `load` view, its limitations, and a `validate` view.
pub fn rows(
    load: Option<&LoadView>,
    load_limitations: &[String],
    validate: Option<&ValidateView>,
    validated: &str,
) -> Vec<WarningRow> {
    let mut out = Vec::new();
    let row = |severity, source: &str, record: &str, message: String| WarningRow {
        severity,
        source: source.to_string(),
        record: record.to_string(),
        message,
        inspect: None,
    };
    if let Some(l) = load {
        for p in &l.plugins {
            if let Some(e) = &p.error {
                out.push(row(
                    Severity::Error,
                    "load",
                    &p.name,
                    format!("did not load: {e}"),
                ));
            }
            for m in &p.missing_masters {
                out.push(row(
                    Severity::Error,
                    "load",
                    &p.name,
                    format!("master {m} is not in the load order"),
                ));
            }
            for m in &p.late_masters {
                out.push(row(
                    Severity::Warning,
                    "load",
                    &p.name,
                    format!("master {m} loads after it"),
                ));
            }
        }
        for m in &l.missing_active {
            out.push(row(
                Severity::Warning,
                "load",
                m,
                "active in the profile, but no file was found".to_string(),
            ));
        }
    }
    for lim in load_limitations {
        out.push(row(Severity::Limitation, "load", "", lim.clone()));
    }
    if let Some(v) = validate {
        let plugin = std::path::Path::new(validated)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(validated);
        for g in &v.groups {
            for f in &g.findings {
                let severity = if f.severity == "error" {
                    Severity::Error
                } else {
                    Severity::Warning
                };
                let mut message = f.message.clone();
                if !f.subrecord.is_empty() {
                    message = format!("{}: {message}", f.subrecord);
                }
                let looks_like_form = f.record.contains(':');
                out.push(WarningRow {
                    severity,
                    source: format!("validate {}", f.rule),
                    record: if f.record.is_empty() {
                        plugin.to_string()
                    } else {
                        f.record.clone()
                    },
                    message,
                    inspect: looks_like_form.then(|| f.record.clone()),
                });
            }
        }
    }
    out
}

/// The window's filter: text (in any column) and which severities show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub text: String,
    pub errors: bool,
    pub warnings: bool,
    pub limitations: bool,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            text: String::new(),
            errors: true,
            warnings: true,
            limitations: true,
        }
    }
}

impl Filter {
    /// Whether `row` passes.
    pub fn keeps(&self, row: &WarningRow) -> bool {
        let on = match row.severity {
            Severity::Error => self.errors,
            Severity::Warning => self.warnings,
            Severity::Limitation => self.limitations,
        };
        let needle = self.text.trim().to_lowercase();
        on && (needle.is_empty()
            || [&row.source, &row.record, &row.message]
                .iter()
                .any(|c| c.to_lowercase().contains(&needle)))
    }
}

/// The rows as tab-separated text with a header, for the clipboard.
pub fn copy_text<'a>(rows: impl Iterator<Item = &'a WarningRow>) -> String {
    let mut out = String::from("severity\tsource\trecord\tmessage\n");
    for r in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            r.severity.word(),
            r.source,
            r.record,
            r.message.replace(['\t', '\n'], " ")
        ));
    }
    out
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, ScrollArea, TextEdit};

use crate::panels::common::plugin_combo;
use crate::theme::{self, Field, View};

/// The window's own state.
#[derive(Default)]
pub struct WarningsState {
    pub filter: Filter,
    /// The plugin the "Validate" button checks (a path from the loaded plugins).
    pub plugin: String,
}

/// What the Warnings window asks the shell for.
pub enum WarningsAsk {
    /// Run `validate` on this plugin (through the Validate tab's request).
    Validate(String),
    Inspect(String),
}

impl WarningsState {
    /// Draws the window's content.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[WarningRow],
        busy: bool,
        plugins: &[(String, String)],
        validated: &str,
    ) -> Option<WarningsAsk> {
        let mut ask = None;
        ui.horizontal_wrapped(|ui| {
            ui.label("filter");
            ui.add(
                TextEdit::singleline(&mut self.filter.text)
                    .desired_width(theme::width(ui, Field::Short)),
            );
            ui.checkbox(&mut self.filter.errors, "errors");
            ui.checkbox(&mut self.filter.warnings, "warnings");
            ui.checkbox(&mut self.filter.limitations, "limitations");
        });
        ui.horizontal_wrapped(|ui| {
            if self.plugin.is_empty() {
                self.plugin = validated.to_string();
            }
            ui.label("validate");
            ui.add(
                TextEdit::singleline(&mut self.plugin).desired_width(theme::width(ui, Field::Long)),
            );
            plugin_combo(ui, "warnings-plugin", &mut self.plugin, plugins, false);
            if ui
                .add_enabled(
                    !busy && !self.plugin.trim().is_empty(),
                    egui::Button::new("Validate"),
                )
                .on_hover_text(
                    "runs `validate` on the plugin (the Validate tab shows the same result)",
                )
                .clicked()
            {
                ask = Some(WarningsAsk::Validate(self.plugin.clone()));
            }
        });
        let shown: Vec<&WarningRow> = rows.iter().filter(|r| self.filter.keeps(r)).collect();
        ui.horizontal(|ui| {
            let count = |s: Severity| rows.iter().filter(|r| r.severity == s).count();
            theme::status(
                ui,
                Status::Fail,
                &format!("{} errors", count(Severity::Error)),
            );
            theme::status(
                ui,
                Status::Warn,
                &format!("{} warnings", count(Severity::Warning)),
            );
            theme::status(
                ui,
                Status::Unknown,
                &format!("{} limitations", count(Severity::Limitation)),
            );
            ui.weak(format!("{} shown", shown.len()));
            if ui
                .small_button("copy")
                .on_hover_text("the shown rows, tab-separated")
                .clicked()
            {
                ui.ctx().copy_text(copy_text(shown.iter().copied()));
            }
        });
        if rows.is_empty() {
            theme::status_line(
                ui,
                Status::NotRun,
                "nothing to show: load a profile, then validate a plugin",
            );
            return ask;
        }
        ui.separator();
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
        ScrollArea::both()
            .id_salt("warnings-rows")
            .auto_shrink([false, false])
            .max_height(theme::view_height(ui, View::Tall))
            .show_rows(ui, row_h, shown.len(), |ui, range| {
                theme::table(ui, "warnings-grid").show(ui, |ui| {
                    for h in ["severity", "source", "record", "message"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for r in shown.get(range).unwrap_or_default() {
                        let status = theme::status(ui, r.severity.status(), r.severity.word());
                        ui.monospace(&r.source);
                        let rec = ui.add(
                            egui::Label::new(egui::RichText::new(&r.record).monospace())
                                .sense(egui::Sense::click()),
                        );
                        let msg = ui.add(egui::Label::new(&r.message).sense(egui::Sense::click()));
                        ui.end_row();
                        if let Some(form) = &r.inspect {
                            let hover = "double-click: inspect the record";
                            if [status, rec.on_hover_text(hover), msg.on_hover_text(hover)]
                                .iter()
                                .any(egui::Response::double_clicked)
                            {
                                ask = Some(WarningsAsk::Inspect(form.clone()));
                            }
                        }
                    }
                });
            });
        ask
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn load_view() -> LoadView {
        LoadView::from_envelope(&json!({"command": "load", "data": {
            "source": "profile:X", "plugins": [
                {"index": 0, "name": "Base.esm", "path": "D:/Data/Base.esm", "masters": [],
                 "missing_masters": [], "late_masters": []},
                {"index": 1, "name": "Patch.esp", "path": "D:/Data/Patch.esp", "masters": ["Gone.esm", "Late.esm"],
                 "missing_masters": ["Gone.esm"], "late_masters": ["Late.esm"]}],
            "inactive": [], "missing_active_plugins": ["Ghost.esp"]},
            "coverage": {"status": "complete"}, "limitations": [], "evidence": []}))
    }

    fn validate_view() -> ValidateView {
        ValidateView::from_envelope(&json!({"command": "validate", "data": {
            "rules": ["links", "masters"], "passed": false, "errors": 1, "warnings": 1,
            "findings": [
                {"rule": "links", "severity": "error", "record": "Patch.esp:000801",
                 "subrecord": "KWDA", "offset": 120, "message": "names a missing form"},
                {"rule": "masters", "severity": "warning", "record": "",
                 "message": "a master loads late"}]},
            "coverage": {"status": "complete"}, "limitations": [], "evidence": []}))
    }

    #[test]
    fn rows_come_from_load_problems_limitations_and_findings() {
        let lims = vec!["2 plugins were indexed from files: the cache missed".to_string()];
        let rows = rows(
            Some(&load_view()),
            &lims,
            Some(&validate_view()),
            "D:/Data/Patch.esp",
        );
        let words: Vec<(&str, &str, &str)> = rows
            .iter()
            .map(|r| (r.severity.word(), r.source.as_str(), r.record.as_str()))
            .collect();
        assert_eq!(
            words,
            vec![
                ("error", "load", "Patch.esp"),
                ("warning", "load", "Patch.esp"),
                ("warning", "load", "Ghost.esp"),
                ("limitation", "load", ""),
                ("error", "validate links", "Patch.esp:000801"),
                ("warning", "validate masters", "Patch.esp"),
            ]
        );
        assert_eq!(rows[4].inspect.as_deref(), Some("Patch.esp:000801"));
        assert_eq!(rows[4].message, "KWDA: names a missing form");
        assert!(rows[5].inspect.is_none());
        // Never color-only: each severity has its own status (glyph and word).
        let statuses: Vec<Status> = [Severity::Error, Severity::Warning, Severity::Limitation]
            .iter()
            .map(|s| s.status())
            .collect();
        assert_eq!(statuses, vec![Status::Fail, Status::Warn, Status::Unknown]);
    }

    #[test]
    fn the_filter_and_copy_follow_the_shown_rows() {
        let rows = rows(Some(&load_view()), &[], Some(&validate_view()), "Patch.esp");
        let mut f = Filter::default();
        assert_eq!(rows.iter().filter(|r| f.keeps(r)).count(), 5);
        f.warnings = false;
        assert_eq!(rows.iter().filter(|r| f.keeps(r)).count(), 2);
        f.text = "KWDA".into();
        let kept: Vec<&WarningRow> = rows.iter().filter(|r| f.keeps(r)).collect();
        assert_eq!(kept.len(), 1);
        let text = copy_text(kept.into_iter());
        assert_eq!(
            text,
            "severity\tsource\trecord\tmessage\nerror\tvalidate links\tPatch.esp:000801\tKWDA: names a missing form\n"
        );
    }
}
