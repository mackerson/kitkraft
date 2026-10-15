//! `schema` (M2.F9): what the builtin record schema (`crates/schema/defs`) says about a
//! record type: its members with their type, source citation and confidence (ADR 0004
//! values), so an agent can ask which fields exist and how far each is checked before it
//! queries or edits. Results stay small: top-level members by default; `deep` adds group
//! members, struct fields, enum and flag names and union variants; `member` picks one.
//!
//! No plugin is read: `coverage` says which of these subrecords a plugin actually has.

use std::collections::BTreeMap;

use kitkraft_schema::{
    ArrayCount, Confidence, FieldDef, FieldType, HeaderDef, Member, NamedValue, Provenance,
    RecordDef, Schema, Signature,
};
use serde_json::{json, Map, Value};

use super::failed;
use crate::envelope::{Coverage, CoverageStatus, Outcome};
use crate::exec::EngineError;

const ID: &str = "schema";

/// Stated on every result.
pub(crate) const DEFINITIONS_ONLY: &str = "definitions only: values come from the builtin \
     schema (crates/schema/defs) and their cited sources, not from a plugin; which of these \
     subrecords a plugin has, and how often, is `coverage`'s job";

/// Stated when `deep` is off.
pub(crate) const TOP_LEVEL_ONLY: &str = "top-level members only: pass `deep` (or `member`) \
     for group members, struct fields, enum and flag names and union variants";

/// The handler.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let schema = kitkraft_schema::builtin()
        .map_err(|e| failed(ID, format!("the builtin schema does not load: {e}")))?;
    let deep = args.get("deep").and_then(Value::as_bool) == Some(true);
    let member = args.get("member").and_then(Value::as_str);
    match args.get("type").and_then(Value::as_str) {
        None if member.is_some() => Err(failed(ID, "`member` needs `type`")),
        None => Ok(listing(schema)),
        Some(text) => {
            // Record signatures are upper case (`weap` means `WEAP`).
            let Some(signature) = Signature::parse(&text.to_ascii_uppercase()) else {
                return Err(failed(
                    ID,
                    format!("`{text}` is not a signature (four printable ASCII characters)"),
                ));
            };
            match (
                schema.record(signature),
                schema.header_by_signature(signature),
            ) {
                (Some(record), _) => one_record(record, member, deep),
                (None, Some(header)) => Ok(one_header(header, deep)),
                (None, None) => Ok(undefined(schema, signature)),
            }
        }
    }
}

/// Counts over a record's leaf subrecords.
#[derive(Default)]
struct Tally {
    subrecords: u64,
    typed: u64,
    opaque: u64,
    by_confidence: BTreeMap<Confidence, u64>,
}

impl Tally {
    fn of(record: &RecordDef) -> Self {
        let mut tally = Self::default();
        for (_, sub) in record.leaf_subrecords() {
            tally.subrecords = tally.subrecords.saturating_add(1);
            if is_opaque(&sub.ty) {
                tally.opaque = tally.opaque.saturating_add(1);
            } else {
                tally.typed = tally.typed.saturating_add(1);
            }
            let n = tally
                .by_confidence
                .entry(sub.provenance.confidence)
                .or_default();
            *n = n.saturating_add(1);
        }
        tally
    }

    fn add(&mut self, other: &Self) {
        self.subrecords = self.subrecords.saturating_add(other.subrecords);
        self.typed = self.typed.saturating_add(other.typed);
        self.opaque = self.opaque.saturating_add(other.opaque);
        for (c, n) in &other.by_confidence {
            let m = self.by_confidence.entry(*c).or_default();
            *m = m.saturating_add(*n);
        }
    }

    fn reference(&self) -> u64 {
        self.by_confidence
            .get(&Confidence::Reference)
            .copied()
            .unwrap_or(0)
    }

    /// `{reference, corpus, ck-diff, verified}`, every level present.
    fn confidence_json(&self) -> Value {
        let mut out = Map::new();
        for c in Confidence::ALL {
            let n = self.by_confidence.get(&c).copied().unwrap_or(0);
            out.insert(c.as_str().to_string(), json!(n));
        }
        Value::Object(out)
    }

    /// `corpus 30, reference 4` (levels with a count only).
    fn confidence_text(&self) -> String {
        let parts: Vec<String> = Confidence::ALL
            .iter()
            .filter_map(|c| {
                let n = self.by_confidence.get(c).copied().unwrap_or(0);
                (n > 0).then(|| format!("{} {n}", c.as_str()))
            })
            .collect();
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Whether a subrecord's payload is kept as raw bytes (a shared type that is opaque too).
fn is_opaque(ty: &FieldType) -> bool {
    match ty {
        FieldType::Opaque { .. } => true,
        FieldType::Ref { ty, .. } => is_opaque(ty),
        _ => false,
    }
}

/// A short type label: `u32`, `formid<KYWD>`, `[formid<KYWD>; rest]`, `opaque(24)`.
fn type_label(ty: &FieldType) -> String {
    match ty {
        FieldType::Scalar(s) => s.as_str().to_string(),
        FieldType::FormId { targets } if targets.is_empty() => "formid".to_string(),
        FieldType::FormId { targets } => {
            let names: Vec<&str> = targets.iter().map(Signature::as_str).collect();
            format!("formid<{}>", names.join("|"))
        }
        FieldType::Sig => "sig".to_string(),
        FieldType::ZString => "zstring".to_string(),
        FieldType::LString => "lstring".to_string(),
        FieldType::WString => "wstring".to_string(),
        FieldType::BString => "bstring".to_string(),
        FieldType::Enum { base, .. } => format!("enum<{}>", base.as_str()),
        FieldType::Flags { base, .. } => format!("flags<{}>", base.as_str()),
        FieldType::Array { element, count } => {
            let n = match count {
                ArrayCount::Fixed(n) => n.to_string(),
                ArrayCount::Rest => "rest".to_string(),
                ArrayCount::From(field) => format!("count from {field}"),
            };
            format!("[{}; {n}]", type_label(element))
        }
        FieldType::Struct { fields } => format!("struct({} fields)", fields.len()),
        FieldType::Opaque { size: Some(n) } => format!("opaque({n})"),
        FieldType::Opaque { size: None } => "opaque".to_string(),
        FieldType::Empty => "empty".to_string(),
        FieldType::Ref { name, ty } => format!("{name}: {}", type_label(ty)),
        FieldType::Union { decider, variants } => {
            format!("union by {} ({} variants)", decider, variants.len())
        }
    }
}

fn named(values: &[NamedValue]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|v| json!({"name": v.name, "value": v.value.to_string()}))
            .collect(),
    )
}

fn insert(obj: &mut Map<String, Value>, key: &str, value: Value) {
    obj.insert(key.to_string(), value);
}

/// `source` and `confidence`, and with `deep` the further `evidence` too. `parent` is the
/// enclosing node's provenance: an inherited source is not repeated in deep output.
fn provenance(obj: &mut Map<String, Value>, p: &Provenance, deep: bool) {
    insert(obj, "confidence", json!(p.confidence.as_str()));
    insert(obj, "source", json!(p.source));
    if deep && !p.evidence.is_empty() {
        insert(obj, "evidence", json!(p.evidence));
    }
}

/// The inner structure of a type, for `deep` output: fields, names, variants.
fn type_detail(obj: &mut Map<String, Value>, ty: &FieldType, parent: &Provenance) {
    match ty {
        FieldType::Struct { fields } => {
            let list: Vec<Value> = fields.iter().map(|f| field_json(f, parent)).collect();
            insert(obj, "fields", Value::Array(list));
        }
        FieldType::Enum { values, .. } => insert(obj, "values", named(values)),
        FieldType::Flags { bits, .. } => insert(obj, "bits", named(bits)),
        FieldType::Array { element, .. } => {
            let mut inner = Map::new();
            type_detail(&mut inner, element, parent);
            if !inner.is_empty() {
                insert(obj, "element", Value::Object(inner));
            }
        }
        FieldType::Ref { ty, .. } => type_detail(obj, ty, parent),
        FieldType::Union { decider, variants } => {
            let list: Vec<Value> = variants
                .iter()
                .map(|v| {
                    let mut out = Map::new();
                    let edid = *decider == kitkraft_schema::Decider::EditorIdPrefix;
                    match v.prefix().filter(|_| edid) {
                        // An EditorID prefix letter, as written in the definition.
                        Some(b) => {
                            insert(&mut out, "when", json!(char::from(b).to_string()));
                        }
                        _ => insert(&mut out, "when", json!(v.when)),
                    }
                    if let Some(s) = v.secondary_when {
                        insert(&mut out, "secondary_when", json!(s));
                    }
                    insert(&mut out, "name", json!(v.name));
                    insert(&mut out, "type", json!(type_label(&v.ty)));
                    if let Some(note) = &v.note {
                        insert(&mut out, "note", json!(note));
                    }
                    type_detail(&mut out, &v.ty, parent);
                    Value::Object(out)
                })
                .collect();
            insert(obj, "variants", Value::Array(list));
        }
        _ => {}
    }
}

fn field_json(field: &FieldDef, parent: &Provenance) -> Value {
    let mut out = Map::new();
    insert(&mut out, "name", json!(field.name));
    insert(&mut out, "type", json!(type_label(&field.ty)));
    if let Some(v) = field.since {
        insert(&mut out, "since_form_version", json!(v));
    }
    if let Some(v) = field.until {
        insert(&mut out, "until_form_version", json!(v));
    }
    if field.if_remaining {
        insert(&mut out, "if_remaining", json!(true));
    }
    insert(
        &mut out,
        "confidence",
        json!(field.provenance.confidence.as_str()),
    );
    if field.provenance != *parent {
        insert(&mut out, "source", json!(field.provenance.source));
        if !field.provenance.evidence.is_empty() {
            insert(&mut out, "evidence", json!(field.provenance.evidence));
        }
    }
    if let Some(note) = &field.note {
        insert(&mut out, "note", json!(note));
    }
    type_detail(&mut out, &field.ty, &field.provenance);
    Value::Object(out)
}

fn member_json(member: &Member, deep: bool) -> Value {
    let mut out = Map::new();
    match member {
        Member::Subrecord(s) => {
            insert(&mut out, "kind", json!("subrecord"));
            insert(&mut out, "signature", json!(s.signature.as_str()));
            insert(&mut out, "name", json!(s.name));
            insert(&mut out, "card", json!(s.cardinality.as_str()));
            insert(&mut out, "type", json!(type_label(&s.ty)));
            insert(&mut out, "typed", json!(!is_opaque(&s.ty)));
            let size = s.size.or_else(|| s.ty.fixed_size(None));
            insert(&mut out, "size", json!(size));
            provenance(&mut out, &s.provenance, deep);
            if deep {
                if let Some(note) = &s.note {
                    insert(&mut out, "note", json!(note));
                }
                type_detail(&mut out, &s.ty, &s.provenance);
            }
        }
        Member::Group(g) => {
            insert(&mut out, "kind", json!("group"));
            insert(&mut out, "name", json!(g.name));
            insert(&mut out, "card", json!(g.cardinality.as_str()));
            insert(&mut out, "shared", json!(g.shared));
            let signatures: Vec<String> = leaf_signatures(&g.members);
            insert(&mut out, "signatures", json!(signatures));
            provenance(&mut out, &g.provenance, deep);
            if deep {
                if let Some(note) = &g.note {
                    insert(&mut out, "note", json!(note));
                }
                let members: Vec<Value> = g.members.iter().map(|m| member_json(m, true)).collect();
                insert(&mut out, "members", Value::Array(members));
            }
        }
        Member::AnyUntil(a) => {
            insert(&mut out, "kind", json!("any_until"));
            insert(&mut out, "name", json!(a.name));
            insert(&mut out, "card", json!("repeated"));
            insert(&mut out, "until", json!(a.until.as_str()));
            provenance(&mut out, &a.provenance, deep);
            if deep {
                if let Some(note) = &a.note {
                    insert(&mut out, "note", json!(note));
                }
            }
        }
    }
    Value::Object(out)
}

fn leaf_signatures(members: &[Member]) -> Vec<String> {
    let mut out = Vec::new();
    for m in members {
        match m {
            Member::Subrecord(s) => out.push(s.signature.as_str().to_string()),
            Member::Group(g) => out.extend(leaf_signatures(&g.members)),
            Member::AnyUntil(_) => {}
        }
    }
    out
}

/// The member `wanted` (name or signature, ASCII case-insensitive): a top-level member,
/// else one inside a group.
fn find_member<'a>(members: &'a [Member], wanted: &str) -> Option<&'a Member> {
    let hit = |m: &Member| {
        m.name().eq_ignore_ascii_case(wanted)
            || matches!(m, Member::Subrecord(s) if s.signature.as_str().eq_ignore_ascii_case(wanted))
    };
    members.iter().find(|m| hit(m)).or_else(|| {
        members.iter().find_map(|m| match m {
            Member::Group(g) => find_member(&g.members, wanted),
            _ => None,
        })
    })
}

fn line(member: &Member) -> String {
    match member {
        Member::Subrecord(s) => format!(
            "  {:<4} {:<22} {:<34} {:<8} {}",
            s.signature.as_str(),
            s.name,
            type_label(&s.ty),
            s.cardinality.as_str(),
            s.provenance.confidence.as_str()
        ),
        Member::Group(g) => format!(
            "  {:<4} {:<22} {:<34} {:<8} {}",
            "",
            g.name,
            format!("group [{}]", leaf_signatures(&g.members).join(" ")),
            g.cardinality.as_str(),
            g.provenance.confidence.as_str()
        ),
        Member::AnyUntil(a) => format!(
            "  {:<4} {:<22} {:<34} {:<8} {}",
            "",
            a.name,
            format!("any until {}", a.until.as_str()),
            "repeated",
            a.provenance.confidence.as_str()
        ),
    }
}

fn outcome(data: Value, limitations: Vec<String>) -> Outcome {
    Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence: Vec::new(),
    }
}

/// Limitations that follow from what a result lists.
fn limitations_for(tally: &Tally, deep: bool) -> Vec<String> {
    let mut out = vec![DEFINITIONS_ONLY.to_string()];
    if tally.reference() > 0 {
        out.push(format!(
            "{} subrecord definition(s) have confidence `reference`: documented in an allowed \
             source, not yet checked against the corpus",
            tally.reference()
        ));
    }
    if tally.opaque > 0 {
        out.push(format!(
            "{} subrecord definition(s) are opaque: named, kept as raw bytes, no field layout yet",
            tally.opaque
        ));
    }
    if !deep {
        out.push(TOP_LEVEL_ONLY.to_string());
    }
    out
}

fn one_record(
    record: &RecordDef,
    member: Option<&str>,
    deep: bool,
) -> Result<Outcome, EngineError> {
    let tally = Tally::of(record);
    let (members, deep) = match member {
        Some(wanted) => {
            let found = find_member(&record.members, wanted).ok_or_else(|| {
                let names: Vec<String> = record
                    .leaf_subrecords()
                    .iter()
                    .map(|(path, s)| format!("{} ({path})", s.signature.as_str()))
                    .collect();
                failed(
                    ID,
                    format!(
                        "{} has no member `{wanted}`; members: {}",
                        record.signature.as_str(),
                        names.join(", ")
                    ),
                )
            })?;
            (vec![found], true)
        }
        None => (record.members.iter().collect(), deep),
    };
    let mut summary = format!(
        "{} ({}): {} subrecord definitions, {} typed, {} opaque; confidence: {}",
        record.signature.as_str(),
        record.name,
        tally.subrecords,
        tally.typed,
        tally.opaque,
        tally.confidence_text()
    );
    for m in &members {
        summary.push('\n');
        summary.push_str(&line(m));
    }
    let mut data = Map::new();
    insert(&mut data, "summary", json!(summary));
    insert(&mut data, "type", json!(record.signature.as_str()));
    insert(&mut data, "defined", json!(true));
    insert(&mut data, "kind", json!("record"));
    insert(&mut data, "name", json!(record.name));
    provenance(&mut data, &record.provenance, true);
    insert(&mut data, "note", json!(record.note));
    let flags: Vec<Value> = record
        .flags
        .iter()
        .map(|f| json!({"name": f.name, "mask": format!("0x{:08X}", f.value)}))
        .collect();
    insert(&mut data, "header_flags", Value::Array(flags));
    insert(&mut data, "deep", json!(deep));
    insert(&mut data, "member", json!(member));
    insert(&mut data, "subrecords", json!(tally.subrecords));
    insert(&mut data, "typed", json!(tally.typed));
    insert(&mut data, "opaque", json!(tally.opaque));
    insert(&mut data, "confidence_counts", tally.confidence_json());
    let list: Vec<Value> = members.iter().map(|m| member_json(m, deep)).collect();
    insert(&mut data, "members", Value::Array(list));
    Ok(outcome(Value::Object(data), limitations_for(&tally, deep)))
}

fn one_header(header: &HeaderDef, deep: bool) -> Outcome {
    let mut summary = format!(
        "{} ({}): fixed {}-byte header, {} fields; confidence: {}",
        header.signature.as_ref().map_or("", Signature::as_str),
        header.name,
        header.size,
        header.fields.len(),
        header.provenance.confidence.as_str()
    );
    for f in &header.fields {
        summary.push_str(&format!(
            "\n  {:<22} {:<34} {}",
            f.name,
            type_label(&f.ty),
            f.provenance.confidence.as_str()
        ));
    }
    let mut data = Map::new();
    insert(&mut data, "summary", json!(summary));
    insert(
        &mut data,
        "type",
        json!(header.signature.map(|s| s.as_str().to_string())),
    );
    insert(&mut data, "defined", json!(true));
    insert(&mut data, "kind", json!("header"));
    insert(&mut data, "name", json!(header.name));
    provenance(&mut data, &header.provenance, true);
    insert(&mut data, "size", json!(header.size));
    insert(&mut data, "deep", json!(deep));
    let fields: Vec<Value> = header
        .fields
        .iter()
        .map(|f| field_json(f, &header.provenance))
        .collect();
    insert(&mut data, "fields", Value::Array(fields));
    let mut limitations = vec![DEFINITIONS_ONLY.to_string()];
    if header.provenance.confidence == Confidence::Reference {
        limitations.push(
            "this header has confidence `reference`: not yet checked against the corpus".into(),
        );
    }
    outcome(Value::Object(data), limitations)
}

fn defined_types(schema: &Schema) -> Vec<String> {
    schema
        .records()
        .map(|r| r.signature.as_str().to_string())
        .collect()
}

fn undefined(schema: &Schema, signature: Signature) -> Outcome {
    let defined = defined_types(schema);
    let summary = format!(
        "{}: no definition; every subrecord of this type is unknown to the schema (defined: {})",
        signature.as_str(),
        defined.join(", ")
    );
    outcome(
        json!({
            "summary": summary,
            "type": signature.as_str(),
            "defined": false,
            "defined_types": defined,
        }),
        vec![
            DEFINITIONS_ONLY.to_string(),
            format!(
                "{} has no definition: its subrecords are unknown (not absent); `coverage` on a \
                 plugin lists the signatures it holds",
                signature.as_str()
            ),
        ],
    )
}

fn listing(schema: &Schema) -> Outcome {
    let mut total = Tally::default();
    let mut records = Vec::new();
    let mut lines = Vec::new();
    for record in schema.records() {
        let tally = Tally::of(record);
        lines.push(format!(
            "  {:<4} {:<22} {:>3} subrecords ({:>2} typed, {:>2} opaque)  {}",
            record.signature.as_str(),
            record.name,
            tally.subrecords,
            tally.typed,
            tally.opaque,
            tally.confidence_text()
        ));
        records.push(json!({
            "type": record.signature.as_str(),
            "name": record.name,
            "confidence": record.provenance.confidence.as_str(),
            "subrecords": tally.subrecords,
            "typed": tally.typed,
            "opaque": tally.opaque,
            "confidence_counts": tally.confidence_json(),
        }));
        total.add(&tally);
    }
    let headers: Vec<Value> = schema
        .headers()
        .map(|h| {
            json!({
                "type": h.signature.map(|s| s.as_str().to_string()),
                "name": h.name,
                "size": h.size,
                "confidence": h.provenance.confidence.as_str(),
            })
        })
        .collect();
    let common: Vec<Value> = schema
        .common()
        .map(|c| {
            json!({
                "name": c.name,
                "kind": match c.body {
                    kitkraft_schema::CommonBody::Group(_) => "group",
                    kitkraft_schema::CommonBody::Type(_) => "type",
                },
                "confidence": c.provenance.confidence.as_str(),
            })
        })
        .collect();
    let mut summary = format!(
        "{} record definitions: {} subrecord definitions, {} typed, {} opaque; confidence: {}",
        records.len(),
        total.subrecords,
        total.typed,
        total.opaque,
        total.confidence_text()
    );
    for l in lines {
        summary.push('\n');
        summary.push_str(&l);
    }
    let mut limitations = limitations_for(&total, true);
    limitations.push(
        "record types without a definition are not listed: their subrecords are unknown to the \
         schema"
            .to_string(),
    );
    outcome(
        json!({
            "summary": summary,
            "records": records,
            "headers": headers,
            "common": common,
            "subrecords": total.subrecords,
            "typed": total.typed,
            "opaque": total.opaque,
            "confidence_counts": total.confidence_json(),
        }),
        limitations,
    )
}

#[cfg(test)]
mod tests {
    use crate::execute;
    use serde_json::{json, Value};

    fn data(args: Value) -> Value {
        execute("schema", args).unwrap()["data"].clone()
    }

    fn member<'a>(data: &'a Value, name: &str) -> &'a Value {
        data["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == name)
            .unwrap_or_else(|| panic!("no member {name}: {data:#}"))
    }

    #[test]
    fn lists_every_record_definition_with_confidence_counts() {
        let result = execute("schema", json!({})).unwrap();
        let d = &result["data"];
        let types: Vec<&str> = d["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["type"].as_str().unwrap())
            .collect();
        assert!(
            types.contains(&"WEAP") && types.contains(&"GMST"),
            "{types:?}"
        );
        let counts = &d["confidence_counts"];
        let sum: u64 = ["reference", "corpus", "ck-diff", "verified"]
            .iter()
            .map(|c| counts[*c].as_u64().unwrap())
            .sum();
        assert_eq!(sum, d["subrecords"].as_u64().unwrap());
        assert_eq!(
            d["typed"].as_u64().unwrap() + d["opaque"].as_u64().unwrap(),
            sum
        );
        assert!(d["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["type"] == "GRUP"));
        assert_eq!(result["evidence"], json!([]));
        assert_eq!(result["coverage"]["status"], "complete");
    }

    #[test]
    fn a_record_lists_top_level_members_with_type_source_and_confidence() {
        let result = execute("schema", json!({"type": "weap"})).unwrap();
        let d = &result["data"];
        assert_eq!(d["type"], "WEAP");
        assert_eq!(d["name"], "Weapon");
        let edid = member(d, "EditorId");
        assert_eq!(edid["signature"], "EDID");
        assert_eq!(edid["type"], "zstring");
        assert_eq!(edid["typed"], true);
        assert_eq!(edid["confidence"], "corpus");
        assert!(edid["source"].as_str().unwrap().starts_with("xedit:"));
        assert!(edid.get("evidence").is_none(), "evidence only with deep");
        let keywords = member(d, "Keywords");
        assert_eq!(keywords["kind"], "group");
        assert_eq!(keywords["signatures"], json!(["KSIZ", "KWDA"]));
        assert!(
            keywords.get("members").is_none(),
            "group members only with deep"
        );
        // VMAD stays opaque in the WEAP definition (lane B round 3); OBND is typed.
        assert_eq!(member(d, "Scripts")["typed"], false);
        assert_eq!(member(d, "ObjectBounds")["typed"], true);
        let limitations = result["limitations"].to_string();
        assert!(
            limitations.contains("top-level members only"),
            "{limitations}"
        );
        assert!(limitations.contains("opaque"), "{limitations}");
        assert!(d["summary"].as_str().unwrap().contains("\n  EDID EditorId"));
    }

    #[test]
    fn deep_expands_groups_and_unions() {
        let d = data(json!({"type": "WEAP", "deep": true}));
        let keywords = member(&d, "Keywords");
        let inner = keywords["members"].as_array().unwrap();
        assert_eq!(inner[0]["signature"], "KSIZ");
        assert_eq!(inner[1]["type"], "[formid<KYWD>; rest]");
        let edid = member(&d, "EditorId");
        assert!(edid["evidence"].as_array().is_some_and(|e| !e.is_empty()));

        let gmst = data(json!({"type": "GMST", "member": "data"}));
        let members = gmst["members"].as_array().unwrap();
        assert_eq!(members.len(), 1);
        let variants = members[0]["variants"].as_array().unwrap();
        assert!(variants
            .iter()
            .any(|v| v["when"] == "f" && v["type"] == "f32"));
        // A member inside a group is found too.
        let kwda = data(json!({"type": "WEAP", "member": "KWDA"}));
        assert_eq!(kwda["members"][0]["name"], "Forms");
    }

    #[test]
    fn headers_unknown_types_and_bad_members() {
        let grup = data(json!({"type": "GRUP"}));
        assert_eq!(grup["kind"], "header");
        assert_eq!(grup["size"], 24);
        // LAND has no definition (REFR is typed by the world family since M4).
        let land = execute("schema", json!({"type": "LAND"})).unwrap();
        assert_eq!(land["data"]["defined"], false);
        assert!(land["limitations"]
            .to_string()
            .contains("unknown (not absent)"));
        let err = execute("schema", json!({"type": "WEAP", "member": "NOPE"})).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("EDID (EditorId)"), "{err}");
        assert_eq!(
            execute("schema", json!({"member": "EDID"}))
                .unwrap_err()
                .kind(),
            "failed"
        );
    }

    #[test]
    fn through_the_command_line() {
        let invocation = crate::parse_line("schema --type WEAP --deep").unwrap();
        let crate::Invocation::Run { command, args } = invocation else {
            panic!("not a run");
        };
        assert_eq!(command.id, "schema");
        assert_eq!(args, json!({"type": "WEAP", "deep": true}));
    }
}
