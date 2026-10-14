//! `overhaul.scaffold` (M3.E7; CLI `kit overhaul`): a starter `build` spec v2 for one
//! record type, from the load order.
//!
//! Arguments `{type, where?, group_by?, out?, overwrite?, max_groups?}` plus the
//! load-order arguments. The winning records of `type` matching `where` are split into
//! groups by the value of `group_by` (a `where` path: `KWDA`, `@plugin`, `DNAM.Member`; a
//! record with several values, such as an array of keywords, joins one group per value;
//! without `group_by`, one group), and each group becomes a class whose `where` selects
//! exactly it, with its match count and the min/median/max of each typed numeric member of
//! the type (lane B's schema) as comments, and a commented-out rule. The largest
//! `max_groups` groups (default 40) are classes; the rest are listed in a comment. The
//! spec text is deterministic. With `out` (a `.toml` file, never inside the game or user
//! Data directory or MO2; an existing one only with `overwrite`) it is written there;
//! without, it is returned in `data.spec_text` (and as the summary, so
//! `kit overhaul WEAP > spec.toml` works).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use kitcraft_doc::query::{Expr, FieldValue, Lookup, Path, Query, Subject};
use kitcraft_doc::search::{run_page, PageRequest, Scope, MAX_LIMIT};
use kitcraft_doc::subject::VersionSubject;
use kitcraft_doc::{GlobalForm, RecordReader, Version};
use serde_json::{json, Value};

use super::load::{acquire, Loaded};
use super::{failed, str_arg};
use crate::build::members;
use crate::build::scaffold::{self, Group, Scaffold, Stat};
use crate::envelope::{Coverage, CoverageStatus, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

const ID: &str = "overhaul.scaffold";

/// Default number of classes.
const DEFAULT_GROUPS: usize = 40;

/// A `where` path (`KWDA`, `DNAM.Speed`, `@plugin`), parsed by lane D's grammar.
fn where_path(text: &str, what: &str) -> Result<Path, EngineError> {
    // `@plugin` and `@edid` compare only with text; every other path takes `null`.
    let parsed = Query::parse(&format!("{text} == null"))
        .or_else(|_| Query::parse(&format!("{text} == \"\"")))
        .map_err(|e| failed(ID, format!("{what} `{text}`: {e}")))?;
    match parsed.expr {
        Expr::Cmp { path, .. } => Ok(path),
        _ => Err(failed(ID, format!("{what} `{text}` is not a member path"))),
    }
}

/// A value as a `where` literal, or `None` when it has no exact literal form.
fn literal(value: &FieldValue) -> Option<String> {
    let text = |t: &str| format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\""));
    match value {
        FieldValue::Text(t) => Some(text(t)),
        FieldValue::Int(i) => Some(i.to_string()),
        FieldValue::Float(f) if f.is_finite() => Some(format!("{f}")),
        FieldValue::Float(_) => None,
        FieldValue::Form { plugin, object_id } => Some(text(&format!("{plugin}:{object_id:06X}"))),
        FieldValue::Bool(b) => Some(b.to_string()),
        FieldValue::Null => Some("null".into()),
    }
}

/// A TOML bare key from `raw`, unique among `used`.
fn class_name(raw: &str, used: &mut HashSet<String>) -> String {
    let mut s = String::new();
    for c in raw.chars() {
        let c = if c.is_ascii_alphanumeric() {
            c.to_ascii_lowercase()
        } else {
            '_'
        };
        if c == '_' && (s.is_empty() || s.ends_with('_')) {
            continue;
        }
        s.push(c);
    }
    let mut s: String = s.trim_end_matches('_').chars().take(48).collect();
    if s.is_empty() {
        s = "group".into();
    }
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s = format!("g_{s}");
    }
    let mut name = s.clone();
    let mut n = 2u32;
    while used.contains(&name) {
        name = format!("{s}_{n}");
        n = n.saturating_add(1);
    }
    used.insert(name.clone());
    name
}

/// The EditorID of `form`'s winning version.
fn editor_id(loaded: &Loaded, plugin: &str, object_id: u32) -> Option<String> {
    let index = loaded.lo.find(plugin)?;
    let form = GlobalForm {
        plugin: index,
        object_id,
    };
    let v = loaded.lo.forms.versions(form).last()?;
    let loc = loaded.lo.record(v.plugin, v.record)?;
    loaded.lo.edid(v.plugin, loc).map(str::to_string)
}

/// The values a lookup gives, as numbers (`None` when undecodable).
fn numbers(lookup: Lookup) -> Option<Vec<f64>> {
    match lookup {
        Lookup::Absent => Some(Vec::new()),
        Lookup::Unknown(_) => None,
        Lookup::Values(v) | Lookup::UnionValues(v) => Some(
            v.iter()
                .filter_map(|x| match x {
                    FieldValue::Int(i) => Some(*i as f64),
                    FieldValue::Float(f) => Some(*f),
                    _ => None,
                })
                .collect(),
        ),
    }
}

/// The `[load_order]` keys the scaffold repeats from the arguments.
fn load_order_keys(args: &Value) -> Vec<(String, toml::Value)> {
    let mut out = Vec::new();
    for key in ["profile", "plugins", "data_dir", "language", "no_cache"] {
        let value = match args.get(key) {
            Some(Value::String(s)) => toml::Value::String(s.clone()),
            Some(Value::Bool(b)) => toml::Value::Boolean(*b),
            Some(Value::Array(items)) => toml::Value::Array(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| toml::Value::String(s.to_string()))
                    .collect(),
            ),
            _ => continue,
        };
        out.push((key.to_string(), value));
    }
    out
}

struct Collected {
    label: String,
    literal: String,
    members: Vec<usize>,
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let ty = str_arg(ID, args, "type")?;
    let sig: [u8; 4] = ty
        .to_ascii_uppercase()
        .as_bytes()
        .try_into()
        .map_err(|_| failed(ID, "`type` must be a four-character record type"))?;
    let type_text = String::from_utf8_lossy(&sig).into_owned();
    let base_text = args
        .get("where")
        .and_then(Value::as_str)
        .map(str::to_string);
    let base = match &base_text {
        Some(t) => Some(Query::parse(t).map_err(|e| failed(ID, format!("where: {e}")))?),
        None => None,
    };
    let group_text = args
        .get("group_by")
        .and_then(Value::as_str)
        .map(str::to_string);
    let group_path = match &group_text {
        Some(t) => Some(where_path(t, "group_by")?),
        None => None,
    };
    let max_groups = args
        .get("max_groups")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(DEFAULT_GROUPS);
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let out = args.get("out").and_then(Value::as_str).map(PathBuf::from);
    if let Some(out) = &out {
        let toml_ext = out
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("toml"));
        if !toml_ext {
            return Err(failed(
                ID,
                format!("{}: the scaffold is a .toml spec", out.display()),
            ));
        }
        crate::edit::patch::guard_location(out).map_err(|e| failed(ID, e.to_string()))?;
        if out.exists() && !overwrite {
            return Err(failed(
                ID,
                format!("`{}` exists; pass overwrite to replace it", out.display()),
            ));
        }
    }
    let schema = kitcraft_schema::builtin()
        .map_err(|e| failed(ID, format!("the builtin schema does not load: {e}")))?;
    let numeric = members::numeric_members(schema, sig);
    let numeric_paths: Vec<Path> = numeric
        .iter()
        .map(|m| where_path(&m.path, "member"))
        .collect::<Result<_, _>>()?;
    let loaded = acquire(session, args, ID)?;
    let mut limitations = loaded.limitations.clone();

    // The winning records.
    let mut reader = RecordReader::new();
    let mut versions: Vec<Version> = Vec::new();
    let mut undecided = 0u64;
    let mut cursor: Option<String> = None;
    loop {
        let request = PageRequest {
            signature: sig,
            query: base.as_ref(),
            scope: Scope::Winner,
            limit: MAX_LIMIT,
            cursor: cursor.as_deref(),
        };
        let page = run_page(&loaded.lo, &mut reader, &request, Some(&mut loaded.strings))
            .map_err(|e| failed(ID, format!("where: {e}")))?;
        undecided = undecided.saturating_add(page.unknown_count);
        versions.extend(page.matched.iter().copied());
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    if undecided > 0 {
        limitations.push(format!(
            "{undecided} record(s) could not be decided by `where` and are in no class"
        ));
    }

    // Per record: its group values and its numbers.
    let mut groups: BTreeMap<String, Collected> = BTreeMap::new();
    let (mut ungrouped, mut group_unknown, mut unexpressible) = (0usize, 0usize, 0usize);
    let mut values: Vec<Vec<Option<Vec<f64>>>> = Vec::with_capacity(versions.len());
    for (i, v) in versions.iter().enumerate() {
        let mut subject = VersionSubject::new(&loaded.lo, &mut reader, *v)
            .with_strings(Some(&mut loaded.strings));
        let mut nums = Vec::with_capacity(numeric_paths.len());
        for p in &numeric_paths {
            nums.push(numbers(subject.lookup(p)));
        }
        values.push(nums);
        let Some(path) = &group_path else {
            continue;
        };
        let found = match subject.lookup(path) {
            Lookup::Absent => {
                ungrouped = ungrouped.saturating_add(1);
                continue;
            }
            Lookup::Unknown(_) => {
                group_unknown = group_unknown.saturating_add(1);
                continue;
            }
            Lookup::Values(v) | Lookup::UnionValues(v) => v,
        };
        let mut seen = HashSet::new();
        for value in found {
            let Some(lit) = literal(&value) else {
                unexpressible = unexpressible.saturating_add(1);
                continue;
            };
            if !seen.insert(lit.clone()) {
                continue;
            }
            let entry = groups.entry(lit.clone()).or_insert_with(|| Collected {
                label: String::new(),
                literal: lit,
                members: Vec::new(),
            });
            entry.members.push(i);
            if entry.label.is_empty() {
                entry.label = match &value {
                    FieldValue::Form { plugin, object_id } => {
                        let edid = editor_id(loaded, plugin, *object_id);
                        format!(
                            "{} == {plugin}:{object_id:06X}{}",
                            group_text.as_deref().unwrap_or_default(),
                            edid.map(|e| format!(" ({e})")).unwrap_or_default()
                        )
                    }
                    _ => format!(
                        "{} == {}",
                        group_text.as_deref().unwrap_or_default(),
                        entry.literal
                    ),
                };
            }
        }
    }
    let mut collected: Vec<Collected> = match &group_path {
        None => vec![Collected {
            label: format!(
                "every winning {type_text}{}",
                base_text
                    .as_ref()
                    .map(|w| format!(" matching {w}"))
                    .unwrap_or_default()
            ),
            literal: String::new(),
            members: (0..versions.len()).collect(),
        }],
        Some(_) => groups.into_values().collect(),
    };
    collected.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then_with(|| a.literal.cmp(&b.literal))
    });
    let omitted: Vec<(String, usize)> = collected
        .iter()
        .skip(max_groups)
        .map(|c| (c.label.clone(), c.members.len()))
        .collect();
    collected.truncate(max_groups);

    let mut used = HashSet::new();
    let mut classes = Vec::new();
    for c in &collected {
        let raw_name = match &group_path {
            None => type_text.clone(),
            Some(_) => {
                // The EditorID in the label when there is one, else the value.
                let edid = c
                    .label
                    .rsplit_once(" (")
                    .map(|(_, e)| e.trim_end_matches(')').to_string());
                edid.unwrap_or_else(|| c.literal.clone())
            }
        };
        let name = class_name(&raw_name, &mut used);
        let where_text = match (&group_text, &base_text) {
            (None, base) => base.clone(),
            (Some(g), None) => Some(format!("{g} == {}", c.literal)),
            (Some(g), Some(b)) => Some(format!("({b}) and {g} == {}", c.literal)),
        };
        let stats = numeric
            .iter()
            .enumerate()
            .map(|(k, m)| {
                let mut st = Stat {
                    member: m.clone(),
                    values: Vec::new(),
                    unknown: 0,
                    records: 0,
                };
                for &i in &c.members {
                    match values.get(i).and_then(|r| r.get(k)) {
                        Some(Some(list)) => {
                            if !list.is_empty() {
                                st.records = st.records.saturating_add(1);
                            }
                            st.values.extend(list.iter().copied());
                        }
                        Some(None) => st.unknown = st.unknown.saturating_add(1),
                        None => {}
                    }
                }
                st
            })
            .collect();
        classes.push(Group {
            name,
            where_text,
            label: c.label.clone(),
            count: c.members.len(),
            stats,
        });
    }
    let spec = Scaffold {
        record_type: type_text.clone(),
        base_where: base_text.clone(),
        group_by: group_text.clone(),
        load_order: load_order_keys(args),
        total: versions.len(),
        undecided,
        ungrouped,
        group_unknown,
        unexpressible,
        groups: classes,
        omitted,
        numeric: numeric.clone(),
    };
    let text = scaffold::render(&spec);
    if spec.load_order.is_empty() {
        limitations.push(
            "the load order came from the session, so the scaffold has no [load_order]: pass it to build".into(),
        );
    }
    if numeric.is_empty() {
        limitations.push(format!(
            "{type_text} has no typed numeric member in the schema yet (lane B round 3), so the scaffold has no statistics"
        ));
    }
    if let Some(out) = &out {
        if out.exists() && !overwrite {
            return Err(failed(
                ID,
                format!("`{}` exists; pass overwrite to replace it", out.display()),
            ));
        }
        let name = out
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let temp = out.with_file_name(format!(".{name}.kitcraft-{}.tmp", std::process::id()));
        std::fs::write(&temp, &text)
            .and_then(|()| std::fs::rename(&temp, out))
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                failed(ID, format!("{}: {e}", out.display()))
            })?;
    }
    let classes_json: Vec<Value> = spec
        .groups
        .iter()
        .map(|g| {
            json!({
                "class": g.name, "where": g.where_text, "matched": g.count, "label": g.label,
                "stats": g.stats.iter().map(|s| {
                    let sum = scaffold::summary(&s.values);
                    json!({
                        "member": s.member.path, "records": s.records, "undecodable": s.unknown,
                        "min": sum.map(|x| x.0), "median": sum.map(|x| x.1), "max": sum.map(|x| x.2),
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();
    let summary = match &out {
        Some(o) => format!(
            "{}: scaffold for {type_text} ({} records, {} classes, {} left out); every rule commented out",
            o.display(),
            spec.total,
            spec.groups.len(),
            spec.omitted.len()
        ),
        None => text.clone(),
    };
    Ok(Outcome {
        data: json!({
            "type": type_text, "where": base_text, "group_by": group_text,
            "out": out.as_ref().map(|o| o.display().to_string()),
            "written": out.is_some(), "bytes": text.len(),
            "records": spec.total, "undecided": undecided, "ungrouped": ungrouped,
            "group_undecodable": group_unknown, "unexpressible": unexpressible,
            "classes": classes_json,
            "omitted": spec.omitted.iter().map(|(l, n)| json!({"label": l, "matched": n})).collect::<Vec<_>>(),
            "numeric_members": numeric.iter().map(|m| json!({
                "path": m.path, "member": m.label, "kind": m.kind,
                "confidence": m.confidence.as_str(), "in_array": m.in_array,
            })).collect::<Vec<_>>(),
            "spec_text": if out.is_some() { Value::Null } else { Value::String(text.clone()) },
            "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence: Vec::new(),
    })
}
