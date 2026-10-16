//! Validate panel (M4.W6, read-only part): `validate` findings by rule with their byte
//! offsets, and `roundtrip --verify` (never with `out`). `build --plan` is not offered:
//! the gate refuses `build` in every mode for now.

use serde_json::{json, Map, Value};

use crate::envelope::{list, num, strings, text, text_or_empty};
use crate::gate::Request;

/// The panel's inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidateForm {
    /// Plugin to check.
    pub plugin: String,
    /// Rule ids, separated by spaces or commas (empty: every rule).
    pub rules: String,
}

impl ValidateForm {
    /// The `validate` request.
    pub fn request(&self) -> Result<Request, String> {
        let plugin = self.plugin.trim();
        if plugin.is_empty() {
            return Err("give a plugin".to_string());
        }
        let mut args = Map::new();
        args.insert("plugin".into(), json!(plugin));
        let rules: Vec<&str> = self
            .rules
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|r| !r.is_empty())
            .collect();
        if !rules.is_empty() {
            args.insert("rules".into(), json!(rules));
        }
        Ok(Request::new("validate", Value::Object(args)))
    }

    /// The `roundtrip --verify` request for the same plugin.
    pub fn roundtrip_request(&self) -> Result<Request, String> {
        let plugin = self.plugin.trim();
        if plugin.is_empty() {
            return Err("give a plugin".to_string());
        }
        Ok(Request::new(
            "roundtrip",
            json!({"plugin": plugin, "verify": true}),
        ))
    }
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRow {
    /// Rule id.
    pub rule: String,
    /// `error` or `warning`.
    pub severity: String,
    /// `SIGN:FORMID` or `Script.Function`.
    pub record: String,
    /// Subrecord signature.
    pub subrecord: String,
    /// Absolute byte offset (the evidence).
    pub offset: Option<u64>,
    /// What is wrong.
    pub message: String,
}

/// Findings of one rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleGroup {
    /// Rule id.
    pub rule: String,
    /// Errors listed.
    pub errors: usize,
    /// Warnings listed.
    pub warnings: usize,
    /// The findings.
    pub findings: Vec<FindingRow>,
}

/// A `validate` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidateView {
    /// `data.summary`.
    pub summary_line: String,
    /// No error finding.
    pub passed: Option<bool>,
    /// Error count.
    pub errors: u64,
    /// Warning count.
    pub warnings: u64,
    /// Findings in total (the list stops at 1,000).
    pub findings_total: u64,
    /// Rules run, in order; each gets a group, empty ones too.
    pub groups: Vec<RuleGroup>,
    /// FormIDs checked / unresolved / subrecords unchecked.
    pub checked_line: String,
}

impl ValidateView {
    /// Reads a `validate` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let mut groups: Vec<RuleGroup> = strings(data, "rules")
            .into_iter()
            .map(|rule| RuleGroup {
                rule,
                errors: 0,
                warnings: 0,
                findings: Vec::new(),
            })
            .collect();
        for f in list(data, "findings") {
            let row = FindingRow {
                rule: text_or_empty(f, "rule"),
                severity: text_or_empty(f, "severity"),
                record: text_or_empty(f, "record"),
                subrecord: text_or_empty(f, "subrecord"),
                offset: num(f, "offset"),
                message: text_or_empty(f, "message"),
            };
            let at = match groups.iter().position(|g| g.rule == row.rule) {
                Some(i) => i,
                None => {
                    groups.push(RuleGroup {
                        rule: row.rule.clone(),
                        errors: 0,
                        warnings: 0,
                        findings: Vec::new(),
                    });
                    groups.len().saturating_sub(1)
                }
            };
            if let Some(g) = groups.get_mut(at) {
                match row.severity.as_str() {
                    "error" => g.errors = g.errors.saturating_add(1),
                    _ => g.warnings = g.warnings.saturating_add(1),
                }
                g.findings.push(row);
            }
        }
        let n = |k: &str| num(data, k).map_or_else(|| "?".to_string(), |n| n.to_string());
        Self {
            summary_line: text_or_empty(data, "summary"),
            passed: data.get("passed").and_then(Value::as_bool),
            errors: num(data, "errors").unwrap_or(0),
            warnings: num(data, "warnings").unwrap_or(0),
            findings_total: num(data, "findings_total").unwrap_or(0),
            groups,
            checked_line: format!(
                "FormIDs checked {}, unresolved {}, subrecords unchecked {}",
                n("checked_form_ids"),
                n("unresolved_form_ids"),
                n("unchecked_subrecords")
            ),
        }
    }
}

/// A `roundtrip --verify` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoundtripView {
    /// Byte-identical.
    pub passed: Option<bool>,
    /// `bytes 39210, records 57, groups 7`.
    pub counts: String,
    /// The first mismatch: `offset, expected, actual, path`.
    pub mismatch: Option<String>,
}

impl RoundtripView {
    /// Reads a `roundtrip` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let n = |k: &str| num(data, k).map_or_else(|| "?".to_string(), |n| n.to_string());
        let mismatch = data.get("mismatch").filter(|m| m.is_object()).map(|m| {
            let byte = |k: &str| {
                m.get(k)
                    .and_then(Value::as_u64)
                    .map_or_else(|| "end".to_string(), |b| format!("0x{b:02X}"))
            };
            format!(
                "at {}: expected {}, got {} ({})",
                n_of(m, "offset"),
                byte("expected"),
                byte("actual"),
                text(m, "path").unwrap_or_default()
            )
        });
        Self {
            passed: data.get("passed").and_then(Value::as_bool),
            counts: format!(
                "bytes {}, records {}, groups {}, compressed {}",
                n("bytes"),
                n("records"),
                n("groups"),
                n("compressed_records")
            ),
            mismatch,
        }
    }
}

fn n_of(v: &Value, k: &str) -> String {
    num(v, k).map_or_else(|| "?".to_string(), |n| n.to_string())
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, CollapsingHeader, TextEdit};

use crate::panels::common::{
    action_bar, envelope_block, findings_grid, panel_head, plugin_combo, Action, Slot, TabCtx,
};
use crate::theme::{self, Field, Status};

/// The Validate tab's state: the form, and the last `validate` and `roundtrip --verify`
/// results.
#[derive(Default)]
pub(crate) struct ValidateTab {
    pub form: ValidateForm,
    pub view: Option<ValidateView>,
    pub roundtrip: Option<RoundtripView>,
}

impl ValidateTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let plugins = ctx.plugins;
        let actions = &mut ctx.actions;
        panel_head(
            ui,
            "Validate",
            "The validate rules on one plugin (`validate`; kitkraft://validate-rules lists them), and a byte roundtrip that rewrites it in memory and compares (`roundtrip --verify`; nothing is written).",
        );
        action_bar(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("plugin");
                ui.add(
                    TextEdit::singleline(&mut self.form.plugin)
                        .desired_width(theme::width(ui, Field::Long)),
                );
                plugin_combo(ui, "val-plugin", &mut self.form.plugin, plugins, false);
                ui.label("rules (empty: all)");
                ui.add(
                    TextEdit::singleline(&mut self.form.rules)
                        .desired_width(theme::width(ui, Field::Medium)),
                );
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("validate"))
                    .clicked()
                {
                    match self.form.request() {
                        Ok(req) => actions.push(Action::Submit(Slot::Validate, req)),
                        Err(e) => *ctx.notice = Some(e),
                    }
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("roundtrip --verify"))
                    .clicked()
                {
                    match self.form.roundtrip_request() {
                        Ok(req) => actions.push(Action::Submit(Slot::Roundtrip, req)),
                        Err(e) => *ctx.notice = Some(e),
                    }
                }
                ui.weak("build --plan is not offered yet (the gate refuses build)");
            });
        });
        envelope_block(ui, ctx.results.get(&Slot::Validate.into()), |ui| {
            let Some(view) = &self.view else {
                return;
            };
            ui.label(format!(
                "{} error(s), {} warning(s), {} finding(s) in total; {}",
                view.errors, view.warnings, view.findings_total, view.checked_line
            ));
            for g in &view.groups {
                let status = if g.errors > 0 {
                    Status::Fail
                } else if g.warnings > 0 {
                    Status::Warn
                } else {
                    Status::Pass
                };
                // The header takes one styled text: the status component's plain-text
                // form (glyph character and word lead it; status is never color-only).
                let title = theme::status_header(
                    ui,
                    status,
                    &format!(
                        "{}: {} error(s), {} warning(s)",
                        g.rule, g.errors, g.warnings
                    ),
                );
                CollapsingHeader::new(title)
                    .id_salt(("rule", g.rule.as_str()))
                    .default_open(!g.findings.is_empty() && g.findings.len() < 50)
                    .show(ui, |ui| {
                        findings_grid(ui, &g.rule, &g.findings);
                    });
            }
        });
        envelope_block(ui, ctx.results.get(&Slot::Roundtrip.into()), |ui| {
            let Some(view) = &self.roundtrip else {
                return;
            };
            ui.label(&view.counts);
            if let Some(m) = &view.mismatch {
                theme::status(ui, Status::Fail, &format!("first mismatch {m}"));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_split_on_commas_and_spaces() {
        let form = ValidateForm {
            plugin: " Mod.esp ".into(),
            rules: "masters.missing, formid.dangling  cell.incc".into(),
        };
        assert_eq!(
            form.request().unwrap().args,
            json!({"plugin": "Mod.esp", "rules": ["masters.missing", "formid.dangling", "cell.incc"]})
        );
        assert_eq!(
            form.roundtrip_request().unwrap().args,
            json!({"plugin": "Mod.esp", "verify": true})
        );
        assert!(ValidateForm::default().request().is_err());
    }

    #[test]
    fn findings_group_by_rule_with_offsets() {
        let env = json!({"command": "validate", "data": {"passed": false, "errors": 1, "warnings": 1,
        "findings_total": 2, "summary": "Mod.esp: failed", "checked_form_ids": 9, "unresolved_form_ids": 0,
        "unchecked_subrecords": 3, "rules": ["masters.missing", "formid.dangling"],
        "findings": [
            {"rule": "formid.dangling", "severity": "error", "record": "GLOB:01000800", "subrecord": "NAME", "offset": 77, "message": "no slot"},
            {"rule": "formid.dangling", "severity": "warning", "record": "GLOB:00000801", "subrecord": null, "offset": 120, "message": "injected"}
        ]}});
        let v = ValidateView::from_envelope(&env);
        assert_eq!(v.passed, Some(false));
        assert_eq!(v.groups.len(), 2);
        assert!(v.groups[0].findings.is_empty());
        assert_eq!((v.groups[1].errors, v.groups[1].warnings), (1, 1));
        assert_eq!(v.groups[1].findings[1].offset, Some(120));
        assert_eq!(
            v.checked_line,
            "FormIDs checked 9, unresolved 0, subrecords unchecked 3"
        );
    }

    #[test]
    fn roundtrip_mismatch_line() {
        let env = json!({"command": "roundtrip", "data": {"passed": false, "bytes": 100, "records": 2, "groups": 1,
            "compressed_records": 0,
            "mismatch": {"offset": 64, "expected": 1, "actual": null, "path": "GRUP GLOB (type 0) @24 > GLOB 00000800 @48"}}});
        let v = RoundtripView::from_envelope(&env);
        assert_eq!(v.passed, Some(false));
        assert_eq!(
            v.mismatch.as_deref(),
            Some("at 64: expected 0x01, got end (GRUP GLOB (type 0) @24 > GLOB 00000800 @48)")
        );
        let ok = RoundtripView::from_envelope(&json!({"data": {"passed": true, "mismatch": null}}));
        assert_eq!(ok.mismatch, None);
    }
}
