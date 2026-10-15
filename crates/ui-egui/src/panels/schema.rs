//! Schema panel (M4.W4): what the builtin schema defines for a record type, through
//! `schema --deep`: members, fields, variants, each with its confidence, source and
//! evidence citations; `reference`-only members are marked unconfirmed.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::envelope::{list, short, strings, text, text_or_empty};
use crate::gate::Request;

/// The panel's inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaForm {
    /// Record type or header (empty: every definition).
    pub record_type: String,
    /// One member (name or signature), optional.
    pub member: String,
    /// Expand groups, fields and variants.
    pub deep: bool,
}

impl Default for SchemaForm {
    fn default() -> Self {
        Self {
            record_type: String::new(),
            member: String::new(),
            deep: true,
        }
    }
}

impl SchemaForm {
    /// The `schema` request.
    pub fn request(&self) -> Request {
        let mut args = Map::new();
        let ty = self.record_type.trim();
        if !ty.is_empty() {
            args.insert("type".into(), json!(ty));
            if !self.member.trim().is_empty() {
                args.insert("member".into(), json!(self.member.trim()));
            }
            if self.deep {
                args.insert("deep".into(), json!(true));
            }
        }
        Request::new("schema", Value::Object(args))
    }
}

/// One row of the member tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRow {
    /// Nesting depth (0: top level).
    pub depth: usize,
    /// `subrecord`, `group`, `field`, `variant`, `any_until`, ...
    pub kind: String,
    /// Subrecord signature, if any.
    pub signature: String,
    /// Member name.
    pub name: String,
    /// `required`, `optional`, `repeated`, ...
    pub card: String,
    /// Type label (`formid<KYWD>`, `opaque(24)`).
    pub ty: String,
    /// `reference`, `corpus`, `ck-diff`, `verified` (inherited from the parent when the
    /// definition does not repeat it).
    pub confidence: String,
    /// Source citation (inherited likewise).
    pub source: String,
    /// Evidence citations.
    pub evidence: Vec<String>,
    /// Definition note.
    pub note: String,
}

impl MemberRow {
    /// Documented upstream, not yet confirmed by the corpus or the CK.
    pub fn unconfirmed(&self) -> bool {
        self.confidence == "reference"
    }
}

/// A `schema` result: one definition's member tree, or the list of definitions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaView {
    /// Whether the type has a definition (`None`: the list of every definition).
    pub defined: Option<bool>,
    /// Definition name.
    pub name: String,
    /// Definition confidence.
    pub confidence: String,
    /// Definition source.
    pub source: String,
    /// `reference / corpus / ck-diff / verified` member counts.
    pub confidence_counts: String,
    /// The member tree, depth first.
    pub members: Vec<MemberRow>,
    /// Every definition (list mode): `(type, name, confidence)`.
    pub definitions: Vec<(String, String, String)>,
}

impl SchemaView {
    /// Reads a `schema` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let counts = data.get("confidence_counts").map_or_else(String::new, |c| {
            ["reference", "corpus", "ck-diff", "verified"]
                .iter()
                .map(|k| format!("{k} {}", c.get(*k).map_or_else(|| "?".to_string(), short)))
                .collect::<Vec<_>>()
                .join(", ")
        });
        let mut members = Vec::new();
        let top = if data.get("members").is_some() {
            list(data, "members").to_vec()
        } else if data.get("member").is_some_and(Value::is_object) {
            data.get("member").cloned().into_iter().collect()
        } else {
            Vec::new()
        };
        let confidence = text_or_empty(data, "confidence");
        let source = text_or_empty(data, "source");
        for m in &top {
            flatten(m, 0, &confidence, &source, &mut members);
        }
        let definitions = list(data, "records")
            .iter()
            .map(|r| {
                (
                    text_or_empty(r, "type"),
                    text_or_empty(r, "name"),
                    text_or_empty(r, "confidence"),
                )
            })
            .collect();
        Self {
            defined: data.get("defined").and_then(Value::as_bool),
            name: text_or_empty(data, "name"),
            confidence,
            source,
            confidence_counts: counts,
            members,
            definitions,
        }
    }
}

fn flatten(m: &Value, depth: usize, conf: &str, src: &str, out: &mut Vec<MemberRow>) {
    if depth > 32 {
        return;
    }
    let confidence = text(m, "confidence").unwrap_or_else(|| conf.to_string());
    let source = text(m, "source").unwrap_or_else(|| src.to_string());
    let kind = text(m, "kind").unwrap_or_else(|| {
        if m.get("when").is_some() {
            "variant".to_string()
        } else {
            "field".to_string()
        }
    });
    out.push(MemberRow {
        depth,
        kind,
        signature: text_or_empty(m, "signature"),
        name: text_or_empty(m, "name"),
        card: text_or_empty(m, "card"),
        ty: m.get("type").map_or_else(String::new, short),
        confidence: confidence.clone(),
        source: source.clone(),
        evidence: strings(m, "evidence"),
        note: text_or_empty(m, "note"),
    });
    let deeper = depth.saturating_add(1);
    for key in ["members", "fields", "variants"] {
        for child in list(m, key) {
            flatten(child, deeper, &confidence, &source, out);
        }
    }
}

/// Who names each signature in a `schema --deep` envelope: the first subrecord member
/// with that signature, at any depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// Member name.
    pub name: String,
    /// Its confidence.
    pub confidence: String,
    /// Its source citation.
    pub source: String,
}

/// Signature -> the member that names it.
pub fn provenance(env: &Value) -> BTreeMap<String, Provenance> {
    let view = SchemaView::from_envelope(env);
    let mut out = BTreeMap::new();
    for row in view.members.into_iter().filter(|r| !r.signature.is_empty()) {
        out.entry(row.signature).or_insert(Provenance {
            name: row.name,
            confidence: row.confidence,
            source: row.source,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_drops_member_and_deep_without_a_type() {
        let form = SchemaForm {
            record_type: "".into(),
            member: "KWDA".into(),
            deep: true,
        };
        assert_eq!(form.request().args, json!({}));
        let form = SchemaForm {
            record_type: "WEAP".into(),
            member: "".into(),
            deep: true,
        };
        assert_eq!(form.request().args, json!({"type": "WEAP", "deep": true}));
    }

    #[test]
    fn member_tree_inherits_confidence_and_marks_reference() {
        let env = json!({"command": "schema", "data": {
        "defined": true, "kind": "record", "name": "Test", "confidence": "corpus", "source": "corpus: test",
        "confidence_counts": {"reference": 1, "corpus": 2, "ck-diff": 0, "verified": 0},
        "members": [
            {"kind": "subrecord", "signature": "EDID", "name": "EditorId", "card": "optional", "type": "zstring",
             "confidence": "corpus", "source": "corpus: edid", "evidence": ["corpus: scan"]},
            {"kind": "subrecord", "signature": "DNAM", "name": "Data", "card": "required", "type": "struct",
             "confidence": "reference", "source": "xedit: test#L1-L2",
             "fields": [{"name": "Speed", "type": "f32"}]}
        ]}});
        let view = SchemaView::from_envelope(&env);
        assert_eq!(view.defined, Some(true));
        assert_eq!(
            view.confidence_counts,
            "reference 1, corpus 2, ck-diff 0, verified 0"
        );
        assert_eq!(view.members.len(), 3);
        assert!(!view.members[0].unconfirmed());
        assert!(view.members[1].unconfirmed());
        let speed = &view.members[2];
        assert_eq!(
            (speed.depth, speed.kind.as_str(), speed.name.as_str()),
            (1, "field", "Speed")
        );
        assert!(
            speed.unconfirmed(),
            "a field inherits its member's confidence"
        );
        let prov = provenance(&env);
        assert_eq!(prov["DNAM"].confidence, "reference");
        assert!(!prov.contains_key(""));
    }

    #[test]
    fn list_mode_and_undefined_types() {
        let view = SchemaView::from_envelope(&json!({"data": {"records": [
            {"type": "GLOB", "name": "Global", "confidence": "corpus"}]}}));
        assert_eq!(view.defined, None);
        assert_eq!(
            view.definitions,
            vec![("GLOB".into(), "Global".into(), "corpus".into())]
        );
        let view = SchemaView::from_envelope(
            &json!({"data": {"defined": false, "defined_types": ["GLOB"]}}),
        );
        assert_eq!(view.defined, Some(false));
        assert!(view.members.is_empty());
    }
}
