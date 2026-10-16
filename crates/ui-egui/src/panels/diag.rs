//! Diagnostics panel (M4.W5): resolve a FormID, EditorID or runtime ID with `inspect` and
//! show the raw stored FormID of every version beside the master-aware identity, each
//! plugin's scale (`unknown` when the load order could not read it), the FormIDs the
//! winner references (raw and resolved), then `validate`'s FormID and master rules on a
//! plugin. Incoming references are the Links panel's "use info" (`refs --incoming`).

use serde_json::{json, Value};

use super::formids;
use super::load::LoadView;
use super::validate::{FindingRow, ValidateView};
use crate::envelope::{list, text, text_or_empty};
use crate::gate::Request;

/// Rules the diagnostics run (masters and FormIDs only).
pub const RULES: &[&str] = &["masters.missing", "masters.order", "formid.dangling"];

/// The `validate` request for the reference rules on `plugin`.
pub fn validate_request(plugin: &str) -> Result<Request, String> {
    let plugin = plugin.trim();
    if plugin.is_empty() {
        return Err("give the plugin to check".to_string());
    }
    Ok(Request::new(
        "validate",
        json!({"plugin": plugin, "rules": RULES}),
    ))
}

/// One version's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityRow {
    /// Plugin holding the version.
    pub plugin: String,
    /// Its scale in the loaded order (`unknown` when not read).
    pub scale: String,
    /// FormID as stored in the plugin (its own master numbering).
    pub raw_form_id: String,
    /// `direct` or the assumption that resolved it.
    pub resolved_by: String,
    /// Winner.
    pub winner: bool,
    /// Deleted.
    pub deleted: bool,
}

/// One FormID the winning version stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefRow {
    /// `SIG Member.Path`.
    pub at: String,
    /// As stored.
    pub raw: String,
    /// Formref, `NULL`, or `unresolved`.
    pub resolved: String,
}

/// The diagnostics of one form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagView {
    /// How the input matched (`formref`, `form_id`, `runtime_id`, `editor_id`).
    pub matched_by: String,
    /// Master-aware identity (`Plugin.esm:XXXXXX`).
    pub global: String,
    /// Defining plugin.
    pub defining_plugin: String,
    /// Object ID within it.
    pub object_id: String,
    /// Runtime ID and its basis.
    pub runtime_id: String,
    /// Winning plugin.
    pub winner: String,
    /// Every version.
    pub identities: Vec<IdentityRow>,
    /// FormIDs in the winning version.
    pub references: Vec<RefRow>,
    /// Masters and FormID findings from `validate`.
    pub findings: Vec<FindingRow>,
    /// `validate` summary line.
    pub validate_summary: Option<String>,
}

impl DiagView {
    /// Reads an `inspect` envelope; `load` gives each plugin's scale.
    pub fn from_inspect(env: &Value, load: Option<&LoadView>) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let scale = |p: &str| load.map_or_else(|| "unknown".to_string(), |l| l.scale_of(p));
        let versions = list(data, "versions");
        let identities = versions
            .iter()
            .map(|v| {
                let plugin = text_or_empty(v, "plugin");
                IdentityRow {
                    scale: scale(&plugin),
                    plugin,
                    raw_form_id: text_or_empty(v, "raw_form_id"),
                    resolved_by: text_or_empty(v, "resolved_by"),
                    winner: v.get("winner").and_then(Value::as_bool).unwrap_or(false),
                    deleted: v.get("deleted").and_then(Value::as_bool).unwrap_or(false),
                }
            })
            .collect();
        let mut references = Vec::new();
        let winner = versions
            .iter()
            .find(|v| v.get("winner") == Some(&Value::Bool(true)));
        for s in winner.map_or(&[][..], |w| list(w, "subrecords")) {
            if let Some(value) = s.get("value") {
                let sig = text_or_empty(s, "signature");
                let base = text(s, "member").unwrap_or_else(|| sig.clone());
                let mut found = Vec::new();
                formids(value, &base, &mut found);
                references.extend(found.into_iter().map(|(path, raw, resolved)| RefRow {
                    at: format!("{sig} {path}"),
                    raw,
                    resolved,
                }));
            }
        }
        let runtime_id = match (text(data, "runtime_id"), text(data, "runtime_id_basis")) {
            (Some(id), Some(basis)) => format!("{id} ({basis})"),
            (Some(id), None) => id,
            _ => "unknown".to_string(),
        };
        Self {
            matched_by: text_or_empty(data, "matched_by"),
            global: text_or_empty(data, "global"),
            defining_plugin: text_or_empty(data, "defining_plugin"),
            object_id: text_or_empty(data, "object_id"),
            runtime_id,
            winner: text_or_empty(data, "winner"),
            identities,
            references,
            findings: Vec::new(),
            validate_summary: None,
        }
    }

    /// Adds the master and FormID findings of a `validate` envelope.
    pub fn with_validate(&mut self, env: &Value) {
        let v = ValidateView::from_envelope(env);
        self.findings = v
            .groups
            .into_iter()
            .filter(|g| g.rule.starts_with("formid.") || g.rule.starts_with("masters."))
            .flat_map(|g| g.findings)
            .collect();
        self.validate_summary = Some(v.summary_line);
    }
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, ScrollArea, TextEdit};

use crate::panels::common::{
    envelope_block, findings_grid, link, record_actions, Action, Slot, TabCtx,
};
use crate::panels::records::inspect_request;
use crate::theme::{self, Field, Status, View};

/// The Diagnostics tab's state: the form to resolve, the plugin to validate, and the
/// view built from `inspect` and then `validate`.
#[derive(Default)]
pub(crate) struct DiagTab {
    pub form: String,
    pub plugin: String,
    pub validate_plugin: String,
    pub view: Option<DiagView>,
}

impl DiagTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let actions = &mut ctx.actions;
        ui.heading("FormID and reference diagnostics");
        ui.label("Raw stored FormIDs beside master-aware identities (`inspect`), then `validate`'s master and FormID rules. \"use info\" lists the records that name a form (`refs --incoming`, Links tab).");
        ui.horizontal_wrapped(|ui| {
            ui.label("form");
            ui.add(
                TextEdit::singleline(&mut self.form)
                    .desired_width(theme::width(ui, Field::Medium))
                    .hint_text("Plugin.esm:XXXXXX, EditorID, runtime:XXXXXXXX, hex"),
            );
            ui.label("plugin (for a hex FormID)");
            ui.add(
                TextEdit::singleline(&mut self.plugin)
                    .desired_width(theme::width(ui, Field::Short)),
            );
            if ui
                .add_enabled(!busy, egui::Button::new("Resolve"))
                .clicked()
            {
                self.validate_plugin.clear();
                match inspect_request(&self.form, Some(&self.plugin)) {
                    Ok(req) => actions.push(Action::Submit(Slot::DiagInspect, req)),
                    Err(e) => *ctx.notice = Some(e),
                }
            }
        });
        envelope_block(ui, ctx.results.get(&Slot::DiagInspect.into()), |ui| {
            let Some(view) = &self.view else { return };
            theme::table(ui, "diag-id").num_columns(2).show(ui, |ui| {
                for (k, v) in [
                    ("identity", &view.global),
                    ("matched by", &view.matched_by),
                    ("defining plugin", &view.defining_plugin),
                    ("object ID", &view.object_id),
                    ("runtime ID", &view.runtime_id),
                    ("winner", &view.winner),
                ] {
                    ui.strong(k);
                    ui.monospace(v);
                    ui.end_row();
                }
            });
            if !view.global.is_empty() {
                record_actions(ui, busy, &view.global, actions);
            }
            ui.label("Versions: the FormID each plugin stores, and how it resolved");
            theme::table(ui, "diag-versions").show(ui, |ui| {
                for h in ["plugin", "scale", "raw FormID", "resolved by", ""] {
                    ui.strong(h);
                }
                ui.end_row();
                for r in &view.identities {
                    ui.label(&r.plugin);
                    if r.scale == "unknown" {
                        theme::status(ui, Status::Unknown, "unknown");
                    } else {
                        ui.label(&r.scale);
                    }
                    ui.monospace(&r.raw_form_id);
                    if r.resolved_by == "direct" {
                        ui.label(&r.resolved_by);
                    } else {
                        theme::status(ui, Status::Warn, &r.resolved_by);
                    }
                    ui.label(match (r.winner, r.deleted) {
                        (true, true) => "winner, deleted",
                        (true, false) => "winner",
                        (false, true) => "deleted",
                        (false, false) => "",
                    });
                    ui.end_row();
                }
            });
            ui.label(format!(
                "FormIDs the winner stores ({}; typed members only)",
                view.references.len()
            ));
            ScrollArea::vertical()
                .id_salt("diag-refs")
                .max_height(theme::view_height(ui, View::Medium))
                .show(ui, |ui| {
                    theme::table(ui, "diag-refs-grid").show(ui, |ui| {
                        for h in ["at", "raw", "resolved", ""] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for r in &view.references {
                            ui.monospace(&r.at);
                            ui.monospace(&r.raw);
                            if r.resolved == "unresolved" {
                                theme::status(ui, Status::Fail, &r.resolved);
                                ui.label("");
                            } else if r.resolved == "NULL" {
                                ui.label(&r.resolved);
                                ui.label("");
                            } else {
                                if link(ui, !busy, &r.resolved).clicked() {
                                    actions.push(Action::Diagnose(r.resolved.clone()));
                                }
                                record_actions(ui, busy, &r.resolved, actions);
                            }
                            ui.end_row();
                        }
                    });
                });
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label("check references of plugin");
            ui.add(
                TextEdit::singleline(&mut self.validate_plugin)
                    .desired_width(theme::width(ui, Field::Long)),
            );
            if ui
                .add_enabled(!busy, egui::Button::new("validate masters + FormIDs"))
                .clicked()
            {
                match validate_request(&self.validate_plugin) {
                    Ok(req) => actions.push(Action::Submit(Slot::DiagValidate, req)),
                    Err(e) => *ctx.notice = Some(e),
                }
            }
        });
        envelope_block(ui, ctx.results.get(&Slot::DiagValidate.into()), |ui| {
            let Some(view) = &self.view else { return };
            if view.findings.is_empty() && view.validate_summary.is_some() {
                theme::status(ui, Status::Pass, "no master or FormID finding");
            }
            findings_grid(ui, "diag-findings", &view.findings);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_and_resolved_side_by_side_with_unknown_scale() {
        let inspect = json!({"command": "inspect", "data": {"mode": "load_order",
        "matched_by": "runtime_id", "global": "Base.esm:000800", "defining_plugin": "Base.esm",
        "object_id": "000800", "runtime_id": "00000800", "runtime_id_basis": "assumed:runtime-slots-by-scale",
        "winner": "Small.esm",
        "versions": [
            {"plugin": "Base.esm", "raw_form_id": "00000800", "resolved_by": "direct", "winner": false, "deleted": false, "subrecords": []},
            {"plugin": "Small.esm", "raw_form_id": "00000800", "resolved_by": "assumed:full-style-self", "winner": true, "deleted": false,
             "subrecords": [
                {"signature": "KWDA", "member": "Keywords", "value": [{"kind": "formref", "raw": "01000801", "form": "Small.esm:000801"}, {"kind": "formref", "raw": "07000001", "form": null}]},
                {"signature": "EDID", "value": "TestForm"}
             ]}
        ]}});
        let load = LoadView::from_envelope(&json!({"data": {"plugins": [
            {"index": 0, "name": "Base.esm", "scale": "full"}]}}));
        let view = DiagView::from_inspect(&inspect, Some(&load));
        assert_eq!(view.identities[0].scale, "full");
        assert_eq!(view.identities[1].scale, "unknown");
        assert_eq!(view.identities[1].resolved_by, "assumed:full-style-self");
        assert_eq!(view.runtime_id, "00000800 (assumed:runtime-slots-by-scale)");
        assert_eq!(
            view.references,
            vec![
                RefRow {
                    at: "KWDA Keywords[0]".into(),
                    raw: "01000801".into(),
                    resolved: "Small.esm:000801".into()
                },
                RefRow {
                    at: "KWDA Keywords[1]".into(),
                    raw: "07000001".into(),
                    resolved: "unresolved".into()
                },
            ]
        );
        let none = DiagView::from_inspect(&inspect, None);
        assert_eq!(none.identities[0].scale, "unknown");
    }

    #[test]
    fn only_master_and_formid_findings_are_kept() {
        let mut view = DiagView::default();
        view.with_validate(&json!({"command": "validate", "data": {"passed": false, "errors": 1, "warnings": 1,
            "summary": "Mod.esp: failed",
            "findings": [
                {"rule": "formid.dangling", "severity": "error", "record": "GLOB:01000800", "subrecord": "NAME", "offset": 77, "message": "no master slot 01"},
                {"rule": "lstring.copied-id", "severity": "warning", "record": "WEAP:00000801", "subrecord": "FULL", "offset": 90, "message": "x"}
            ]}}));
        assert_eq!(view.findings.len(), 1);
        assert_eq!(view.findings[0].offset, Some(77));
        assert_eq!(view.validate_summary.as_deref(), Some("Mod.esp: failed"));
        assert_eq!(
            validate_request("Mod.esp").unwrap().args,
            json!({"plugin": "Mod.esp", "rules": ["masters.missing", "masters.order", "formid.dangling"]})
        );
    }
}
