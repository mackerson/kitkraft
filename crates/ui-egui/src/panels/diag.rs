//! Diagnostics panel (M4.W5): resolve a FormID, EditorID or runtime ID with `inspect` and
//! show the raw stored FormID of every version beside the master-aware identity, each
//! plugin's scale (`unknown` when the load order could not read it), the FormIDs the
//! winner references (raw and resolved), then `validate`'s FormID and master rules on a
//! plugin. `refs` (M4.D1) is not on `main` yet; incoming references wait for it.

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
                {"signature": "KWDA", "member": "Keywords", "value": [{"raw": "01000801", "form": "Small.esm:000801"}, {"raw": "07000001", "form": null}]},
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
