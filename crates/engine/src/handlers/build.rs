//! `build` (M2.E9, spec v2 M3.E6): compile a TOML patch spec into one fresh plugin, then
//! validate it. The spec format (versions 1 and 2) is [`crate::build::spec`].
//!
//! Classes select the winning records of one type matching `where` (load order, then
//! FormID); a rule applies its `fields` to every record its class matches, or, with `via`,
//! to the records reached by following typed FormID members hop by hop (weapon -> keyword
//! today; weapon -> ammo -> projectile once lane B types them). Explicit `[[set]]` entries
//! apply after the rules. In a version 2 spec a record reached by several rules or classes
//! is written once: the same write twice is one write, and two different writes to one
//! field (the same path, or a whole subrecord and a member of it) are an error naming both
//! TOML keys; a version 1 spec keeps M2's rule (any field written twice is an error). A
//! rule that matches nothing is a warning (`limitations`), and an error with
//! `strict = true`. Version 2 checks every field path against the record type's schema
//! (`unknown member`), and every spec refuses a write to a `reference`-only member unless
//! the entry says `allow_reference = true` (then it is named in `limitations`).
//!
//! `out` is checked like any patch target (never a load-order plugin, the game or user
//! Data directory, or MO2) before anything is written. The plugin is always written fresh
//! in a staging directory under the OS temp dir, validated, and only then moved to `out`;
//! an existing `out` is replaced only with `overwrite: true`, and a build with validation
//! errors writes nothing. `plan: true` (CLI `--plan`) stops before the move: it returns the
//! change list (every field with before and after, and the spec keys that wrote it), the
//! classes and rules with their counts, and the validation result, and writes nothing to
//! `out`. Spec errors name the TOML key (`set[0].fields.FULL`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use kitcraft_doc::query::{FieldValue, Lookup, Path, Query, Subject};
use kitcraft_doc::search::{run_page, PageRequest, Scope, MAX_LIMIT};
use kitcraft_doc::subject::VersionSubject;
use kitcraft_doc::{GlobalForm, RecordReader};
use kitcraft_esm::DecodeLimits;
use kitcraft_schema::{Confidence, Schema};
use serde_json::{json, Value};

use super::load::{acquire, Loaded};
use super::set::{
    check, describe, find_form, guard, install, outcome, stage, validate_json, winning_copy, write,
    Origin,
};
use super::{failed, str_arg};
use crate::build::members;
use crate::build::spec::{self, Spec};
use crate::edit::fields::{FieldPath, Step};
use crate::edit::patch::Edit;
use crate::envelope::Outcome;
use crate::exec::EngineError;
use crate::session::Session;
use crate::validate::{self, Severity};

const ID: &str = "build";

fn spec_error(key: &str, message: impl std::fmt::Display) -> EngineError {
    failed(ID, format!("spec: {key}: {message}"))
}

/// Whether two field paths write overlapping bytes: the same subrecord occurrence, and one
/// path's steps a prefix of the other's (`DNAM` and `DNAM.Speed`, or the same member).
fn overlaps(a: &FieldPath, b: &FieldPath) -> bool {
    a.signature == b.signature
        && a.occurrence == b.occurrence
        && a.steps.iter().zip(&b.steps).all(|(x, y)| match (x, y) {
            (Step::Member(x), Step::Member(y)) => x.eq_ignore_ascii_case(y),
            (x, y) => x == y,
        })
}

/// Whether two overlapping paths name the same field (neither is a prefix of the other).
fn same_field(a: &FieldPath, b: &FieldPath) -> bool {
    overlaps(a, b) && a.steps.len() == b.steps.len()
}

/// One write planned on one record.
struct Written {
    /// `rule[0].fields.FULL`: the first spec key that wrote it.
    key: String,
    edit: Edit,
    /// Every entry (`rule[0]`, `set[1]`) that asked for this write.
    entries: Vec<String>,
}

/// The writes of a build, per record, in first-reached order.
struct Writes {
    /// Version 2: an identical write from another entry (or reached twice) is one write.
    dedupe: bool,
    order: Vec<GlobalForm>,
    per_form: BTreeMap<u64, Vec<Written>>,
}

impl Writes {
    fn add(
        &mut self,
        form: GlobalForm,
        at: &str,
        edits: &[Edit],
        label: &str,
    ) -> Result<(), EngineError> {
        let entry = self.per_form.entry(form.key()).or_default();
        if entry.is_empty() {
            self.order.push(form);
        }
        for edit in edits {
            let key = format!("{at}.fields.{}", edit.path.text);
            if let Some(w) = entry
                .iter_mut()
                .find(|w| overlaps(&w.edit.path, &edit.path))
            {
                if self.dedupe && same_field(&w.edit.path, &edit.path) && w.edit.op == edit.op {
                    if !w.entries.iter().any(|e| e == at) {
                        w.entries.push(at.to_string());
                    }
                    continue;
                }
                let what = if self.dedupe && same_field(&w.edit.path, &edit.path) {
                    "with a different value"
                } else {
                    ""
                };
                return Err(spec_error(
                    &key,
                    format!(
                        "{} of {label} is already written by {}{}{what}",
                        edit.path.text,
                        w.key,
                        if what.is_empty() { "" } else { " " }
                    ),
                ));
            }
            entry.push(Written {
                key,
                edit: edit.clone(),
                entries: vec![at.to_string()],
            });
        }
        Ok(())
    }
}

/// Every winning record of `sig` matching `query` (every winner when `None`), in result
/// order, and how many the query could not decide.
fn matches(
    loaded: &mut Loaded,
    sig: [u8; 4],
    query: Option<&Query>,
    at: &str,
) -> Result<(Vec<GlobalForm>, u64), EngineError> {
    let mut reader = RecordReader::new();
    let mut out = Vec::new();
    let mut unknown = 0u64;
    let mut cursor: Option<String> = None;
    loop {
        let request = PageRequest {
            signature: sig,
            query,
            scope: Scope::Winner,
            limit: MAX_LIMIT,
            cursor: cursor.as_deref(),
        };
        let page = run_page(&loaded.lo, &mut reader, &request, Some(&mut loaded.strings))
            .map_err(|e| spec_error(&format!("{at}.where"), e))?;
        unknown = unknown.saturating_add(page.unknown_count);
        out.extend(page.matched.iter().map(kitcraft_doc::Version::form));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    Ok((out, unknown))
}

/// Why a `via` hop could not follow a record.
enum Unfollowed {
    /// The value is unknown (opaque, undecodable, unresolvable FormID): reported.
    Unknown(String),
    /// The path does not name FormIDs: a spec error.
    NotForm(String),
}

/// The record type of `form`'s winning version.
fn signature_of(loaded: &Loaded, form: GlobalForm) -> Option<[u8; 4]> {
    let v = loaded.lo.forms.versions(form).last()?;
    loaded
        .lo
        .record(v.plugin, v.record)
        .map(|loc| loc.signature)
}

/// The forms the typed FormID member `path` of `form`'s winning version names (the null
/// FormID names nothing). Lane D's `follow` (M3.D4) will replace this once it lands.
fn follow(
    loaded: &Loaded,
    reader: &mut RecordReader,
    form: GlobalForm,
    path: &Path,
) -> Result<Vec<GlobalForm>, Unfollowed> {
    let Some(version) = loaded.lo.forms.versions(form).last().copied() else {
        return Err(Unfollowed::Unknown(format!(
            "{} has no version in the load order",
            loaded.lo.form_label(form)
        )));
    };
    let mut subject = VersionSubject::new(&loaded.lo, reader, version);
    let values = match subject.lookup(path) {
        Lookup::Absent => return Ok(Vec::new()),
        Lookup::Unknown(why) => return Err(Unfollowed::Unknown(why)),
        Lookup::Values(v) | Lookup::UnionValues(v) => v,
    };
    let mut out = Vec::new();
    for value in values {
        match value {
            FieldValue::Null => {}
            FieldValue::Form { plugin, object_id } => {
                let index = loaded.lo.find(&plugin).ok_or_else(|| {
                    Unfollowed::Unknown(format!("`{plugin}` is not in the load order"))
                })?;
                out.push(GlobalForm {
                    plugin: index,
                    object_id,
                });
            }
            other => {
                return Err(Unfollowed::NotForm(format!(
                    "holds {other:?}, not a FormID: `via` follows typed FormID members"
                )))
            }
        }
    }
    Ok(out)
}

/// The member checks of one entry's edits on a record of type `sig`: an unknown member (a
/// version 2 spec only), and a `reference`-only member without `allow_reference`.
#[allow(clippy::too_many_arguments)]
pub(super) fn gate(
    schema: &Schema,
    sig: [u8; 4],
    at: &str,
    edits: &[Edit],
    allow_reference: bool,
    check_members: bool,
    cache: &mut HashMap<([u8; 4], String), Result<members::Resolved, String>>,
    limitations: &mut Vec<String>,
) -> Result<(), EngineError> {
    let type_name = String::from_utf8_lossy(&sig).into_owned();
    for edit in edits {
        let key = format!("{at}.fields.{}", edit.path.text);
        let resolved = cache
            .entry((sig, edit.path.text.to_ascii_uppercase()))
            .or_insert_with(|| members::resolve(schema, sig, &edit.path));
        match resolved {
            Err(message) if check_members => {
                return Err(spec_error(
                    &key,
                    format!("{message} (record type {type_name})"),
                ))
            }
            Err(_) => {}
            Ok(r) if r.confidence == Some(Confidence::Reference) => {
                if !allow_reference {
                    return Err(spec_error(
                        &key,
                        format!(
                            "{} is a reference-only member (documented in the pinned xEdit definitions, not validated by the corpus or against the CK), so writing it is refused by default (ADR 0004 owner approval, clause 3; ADR 0005 Amendment 3 §4); set allow_reference = true on this entry to write it anyway",
                            r.label
                        ),
                    ));
                }
                let note = format!(
                    "{at}: writes {}, a reference-only member (allow_reference = true): its layout is documented, not validated",
                    r.label
                );
                if !limitations.contains(&note) {
                    limitations.push(note);
                }
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let spec_path = PathBuf::from(str_arg(ID, args, "spec")?);
    let out = PathBuf::from(str_arg(ID, args, "out")?);
    let flag = |key: &str| args.get(key).and_then(Value::as_bool).unwrap_or(false);
    let (overwrite, allow_untyped, plan) = (
        flag("overwrite"),
        flag("allow_untyped_lstrings"),
        flag("plan"),
    );
    let text = std::fs::read_to_string(&spec_path)
        .map_err(|e| failed(ID, format!("{}: {e}", spec_path.display())))?;
    let spec: Spec = spec::parse(&text).map_err(|e| failed(ID, e.to_string()))?;
    let loaded = match &spec.load_order {
        Some(lo_args) => acquire(session, lo_args, ID)?,
        None => acquire(session, args, ID)?,
    };
    // `out` is checked as the patch writer checks its target (never a load-order plugin,
    // the game or user Data directory, or MO2), and an existing `out` is replaced only
    // with `overwrite`, before anything is staged. A plan writes nothing to `out`.
    guard(ID, loaded, &out)?;
    if !plan && !overwrite && out.exists() {
        return Err(failed(
            ID,
            format!("`{}` exists; pass overwrite to replace it", out.display()),
        ));
    }
    let schema = kitcraft_schema::builtin()
        .map_err(|e| failed(ID, format!("the builtin schema does not load: {e}")))?;
    let v2 = spec.version >= 2;
    let mut limitations = loaded.limitations.clone();
    let mut warnings: Vec<String> = Vec::new();

    // Classes: winning records of one type matching `where`.
    let mut class_forms: Vec<Vec<GlobalForm>> = Vec::new();
    let mut classes_json = Vec::new();
    for class in &spec.classes {
        let (forms, unknown) = matches(loaded, class.signature, class.query.as_ref(), &class.key)?;
        if unknown > 0 {
            limitations.push(format!(
                "{}: {unknown} record(s) could not be decided by `where` and were not changed",
                class.key
            ));
        }
        classes_json.push(json!({
            "class": class.name, "type": String::from_utf8_lossy(&class.signature),
            "where": class.query.as_ref().map(|q| q.text.clone()), "brief": class.brief,
            "matched": forms.len(), "undecided": unknown,
        }));
        class_forms.push(forms);
    }

    // Rules, then sets.
    let mut writes = Writes {
        dedupe: v2,
        order: Vec::new(),
        per_form: BTreeMap::new(),
    };
    let mut cache = HashMap::new();
    let mut reader = RecordReader::new();
    let mut rules_json = Vec::new();
    for rule in &spec.rules {
        let class = spec
            .classes
            .get(rule.class)
            .ok_or_else(|| failed(ID, "rule names a missing class"))?;
        let matched = class_forms.get(rule.class).cloned().unwrap_or_default();
        let mut targets = matched.clone();
        let mut hops = Vec::new();
        for (j, hop) in rule.via.iter().enumerate() {
            let mut next = Vec::new();
            let mut seen = HashSet::new();
            let mut unknown: Vec<String> = Vec::new();
            for form in &targets {
                match follow(loaded, &mut reader, *form, &hop.path) {
                    Ok(list) => {
                        for g in list {
                            if seen.insert(g.key()) {
                                next.push(g);
                            }
                        }
                    }
                    Err(Unfollowed::Unknown(why)) => {
                        unknown.push(format!("{}: {why}", loaded.lo.form_label(*form)))
                    }
                    Err(Unfollowed::NotForm(why)) => {
                        return Err(spec_error(
                            &format!("{}.via[{j}]", rule.key),
                            format!("{} of {} {why}", hop.text, loaded.lo.form_label(*form)),
                        ))
                    }
                }
            }
            if !unknown.is_empty() {
                limitations.push(format!(
                    "{}: via {} could not follow {} record(s): {}",
                    rule.key,
                    hop.text,
                    unknown.len(),
                    unknown
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
            hops.push(json!({"via": hop.text, "from": targets.len(), "reached": next.len(), "unfollowed": unknown.len()}));
            targets = next;
        }
        if targets.is_empty() {
            let message = format!(
                "{} matches nothing (class {}{})",
                rule.key,
                class.name,
                if rule.via.is_empty() {
                    String::new()
                } else {
                    format!(", via {}", rule.via.len())
                }
            );
            if spec.strict {
                return Err(spec_error(&rule.key, format!("{message}; strict = true")));
            }
            if v2 {
                warnings.push(message.clone());
                limitations.push(format!("warning: {message}"));
            }
        }
        for form in &targets {
            let label = loaded.lo.form_label(*form);
            let sig = signature_of(loaded, *form)
                .ok_or_else(|| failed(ID, format!("{label} has no version in the load order")))?;
            gate(
                schema,
                sig,
                &rule.key,
                &rule.edits,
                rule.allow_reference,
                v2,
                &mut cache,
                &mut limitations,
            )?;
            writes.add(*form, &rule.key, &rule.edits, &label)?;
        }
        let mut entry = json!({
            "rule": rule.key, "matched": matched.len(), "undecided": 0,
        });
        if let Some(obj) = entry.as_object_mut() {
            let undecided = classes_json
                .get(rule.class)
                .and_then(|c| c.get("undecided"))
                .cloned()
                .unwrap_or(json!(0));
            obj.insert("undecided".into(), undecided);
            if v2 {
                obj.insert("class".into(), json!(class.name));
                obj.insert("brief".into(), json!(rule.brief));
                obj.insert("via".into(), Value::Array(hops));
                obj.insert("records".into(), json!(targets.len()));
            }
        }
        rules_json.push(entry);
    }
    for set in &spec.sets {
        let g = find_form(ID, loaded, &set.form, set.plugin.as_deref())
            .map_err(|e| spec_error(&format!("{}.form", set.key), e))?;
        let label = loaded.lo.form_label(g);
        if let Some(sig) = signature_of(loaded, g) {
            gate(
                schema,
                sig,
                &set.key,
                &set.edits,
                set.allow_reference,
                v2,
                &mut cache,
                &mut limitations,
            )?;
        }
        writes.add(g, &set.key, &set.edits, &label)?;
    }
    if writes.order.is_empty() && !plan {
        return Err(failed(
            ID,
            "the spec's rules matched no records and it has no sets; nothing to write",
        ));
    }

    // Write fresh into a staging directory (OS temp dir), validate, then move it to `out`
    // (or, for a plan, report and stop).
    let limits = DecodeLimits::default();
    let mut copies = Vec::new();
    let mut origins: Vec<Origin> = Vec::new();
    for form in &writes.order {
        let unrelated = super::set::safe_source(ID, loaded, *form)?;
        super::set::extend_once(&mut limitations, unrelated);
        let list: Vec<Edit> = writes
            .per_form
            .get(&form.key())
            .map(|v| v.iter().map(|w| w.edit.clone()).collect())
            .unwrap_or_default();
        let (copy, origin) = winning_copy(ID, loaded, *form, list, &limits)?;
        copies.push(copy);
        origins.push(origin);
    }
    if writes.order.is_empty() {
        // A plan of nothing (a fresh scaffold: every rule commented out).
        let mut result = Outcome {
            data: json!({
                "plan": true, "spec": spec_path.display().to_string(),
                "target": out.display().to_string(), "changes": [], "changed_fields": 0,
                "records": 0, "classes": classes_json, "rules": rules_json,
                "warnings": warnings, "passed": true,
                "summary": format!("plan: 0 changes ({} classes, {} rules)", spec.classes.len(), spec.rules.len()),
            }),
            coverage: crate::envelope::Coverage {
                status: crate::envelope::CoverageStatus::Partial,
                decoded_subrecords: None,
                opaque_subrecords: None,
                not_examined: Vec::new(),
            },
            limitations,
            evidence: Vec::new(),
        };
        result
            .limitations
            .push("plan: nothing was written".to_string());
        return Ok(result);
    }
    let staged = stage(ID, &out, false)?;
    let mut report = write(ID, loaded, &staged.path, copies, allow_untyped)?;
    let checked = check(ID, loaded, &staged)?;

    // The change list: every planned write with its before and after, and the entries
    // that asked for it.
    let mut changes = Vec::new();
    let mut changed_fields = 0u64;
    let mut per_entry: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let written = report.records.iter().filter(|r| !r.parent);
    for (form, record) in writes.order.iter().zip(written) {
        let list = writes.per_form.get(&form.key());
        for c in &record.changes {
            let entries: Vec<String> = list
                .and_then(|l| l.iter().find(|w| w.edit.path.text == c.path))
                .map(|w| w.entries.clone())
                .unwrap_or_default();
            if c.changed {
                changed_fields = changed_fields.saturating_add(1);
            }
            for e in &entries {
                let counts = per_entry.entry(e.clone()).or_insert((0, 0));
                counts.0 = counts.0.saturating_add(1);
                if c.changed {
                    counts.1 = counts.1.saturating_add(1);
                }
            }
            changes.push(json!({
                "form": record.formref, "signature": record.signature,
                "editor_id": record.editor_id, "path": c.path, "before": c.before,
                "after": c.after, "changed": c.changed, "by": entries,
            }));
        }
    }
    for entry in &mut rules_json {
        if let Some(obj) = entry.as_object_mut() {
            let key = obj
                .get("rule")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let (fields, changed) = per_entry.get(&key).copied().unwrap_or((0, 0));
            if v2 {
                obj.insert("fields_written".into(), json!(fields));
                obj.insert("fields_changed".into(), json!(changed));
            }
        }
    }

    if plan {
        report.target = out.clone();
        let mut result = outcome(&report, &origins, limitations);
        // The staged file is not the plugin `out` would be: drop its evidence ranges.
        result.evidence.retain(|e| {
            e.file
                != report
                    .target
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
        });
        result
            .limitations
            .push(format!("plan: nothing was written to {}", out.display()));
        if let Some(obj) = result.data.as_object_mut() {
            obj.insert("plan".into(), json!(true));
            obj.insert("spec".into(), json!(spec_path.display().to_string()));
            obj.insert("changes".into(), Value::Array(changes));
            obj.insert("changed_fields".into(), json!(changed_fields));
            obj.insert("classes".into(), Value::Array(classes_json));
            obj.insert("rules".into(), Value::Array(rules_json));
            obj.insert("warnings".into(), json!(warnings));
            obj.insert("validate".into(), validate_json(&checked));
            obj.insert("passed".into(), json!(checked.passed()));
            obj.insert(
                "summary".into(),
                json!(format!(
                    "plan: {changed_fields} field change(s) on {} record(s) for {} (nothing written); validate {}",
                    writes.order.len(),
                    out.display(),
                    if checked.passed() { "passed" } else { "FAILED" }
                )),
            );
        }
        return Ok(result);
    }
    if !checked.passed() {
        let errors: Vec<&validate::Finding> = checked
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect();
        return Err(failed(
            ID,
            format!(
                "the built plugin fails validate, so nothing was written: {}",
                describe(&errors)
            ),
        ));
    }
    if !overwrite && out.exists() {
        return Err(failed(
            ID,
            format!("`{}` exists; pass overwrite to replace it", out.display()),
        ));
    }
    install(ID, &staged, &out)?;
    report.target = out.clone();
    let mut result = outcome(&report, &origins, limitations);
    if let Some(obj) = result.data.as_object_mut() {
        obj.insert("spec".into(), json!(spec_path.display().to_string()));
        obj.insert("rules".into(), Value::Array(rules_json));
        if v2 {
            obj.insert("classes".into(), Value::Array(classes_json));
            obj.insert("changed_fields".into(), json!(changed_fields));
            obj.insert("warnings".into(), json!(warnings));
        }
        obj.insert("validate".into(), validate_json(&checked));
        obj.insert("passed".into(), json!(true));
    }
    Ok(result)
}
