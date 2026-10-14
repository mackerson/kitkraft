//! `build` (M2.E9): compile a TOML patch spec into one fresh plugin, then validate it.
//!
//! ```toml
//! version = 1                          # the only version
//! [plugin]
//! localized = false                    # the only value M2 accepts
//! [load_order]                         # optional; else the arguments or the session
//! profile = 'C:\Modding\MO2-Starfield\profiles\Default'   # or plugins = [...], data_dir
//! [[rule]]                             # every winning record of `type` matching `where`
//! type = "WEAP"
//! where = 'EDID ~ "Rifle"'
//! fields = { "DNAM.Speed" = { mul = 0.8 } }
//! [[set]]                              # one record, as `kit set`
//! form = "Starfield.esm:0021BBCD"
//! fields = { FULL = "Old Soviet Marksman Rifle" }
//! ```
//!
//! Rules expand to sets over their `query` matches (load order, then FormID), explicit
//! `[[set]]` entries apply after the rules, and a field written twice is an error naming
//! both TOML keys. The plugin is always written fresh (an existing `out` is replaced only
//! once the new one has passed `validate`); a build with validation errors writes nothing.
//! Spec errors name the TOML key (`set[0].fields.FULL`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kitcraft_doc::query::Query;
use kitcraft_doc::search::{run_page, PageRequest, Scope, MAX_LIMIT};
use kitcraft_doc::{GlobalForm, RecordReader};
use kitcraft_esm::DecodeLimits;
use serde_json::{json, Map, Value};

use super::load::{acquire, Loaded};
use super::set::{edits, find_form, outcome, winning_copy, write, Origin};
use super::{failed, str_arg};
use crate::edit::patch::Edit;
use crate::envelope::Outcome;
use crate::exec::EngineError;
use crate::session::Session;
use crate::validate::{self, Options, Severity};

const ID: &str = "build";

fn spec_error(key: &str, message: impl std::fmt::Display) -> EngineError {
    failed(ID, format!("spec: {key}: {message}"))
}

/// TOML -> JSON (for field values and load-order arguments).
fn to_json(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(*f).map_or(Value::Null, Value::Number)
        }
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(a) => Value::Array(a.iter().map(to_json).collect()),
        toml::Value::Table(t) => Value::Object(
            t.iter()
                .map(|(k, v)| (k.clone(), to_json(v)))
                .collect::<Map<String, Value>>(),
        ),
    }
}

fn only_keys(table: &toml::Table, key: &str, allowed: &[&str]) -> Result<(), EngineError> {
    for k in table.keys() {
        if !allowed.contains(&k.as_str()) {
            let at = if key.is_empty() {
                k.clone()
            } else {
                format!("{key}.{k}")
            };
            return Err(spec_error(
                &at,
                format!("unknown key; allowed: {}", allowed.join(", ")),
            ));
        }
    }
    Ok(())
}

fn string<'t>(table: &'t toml::Table, key: &str, at: &str) -> Result<&'t str, EngineError> {
    match table.get(key) {
        Some(toml::Value::String(s)) => Ok(s),
        Some(_) => Err(spec_error(&format!("{at}.{key}"), "must be a string")),
        None => Err(spec_error(&format!("{at}.{key}"), "is required")),
    }
}

/// The `fields` table of an entry as edits; errors name `<at>.fields.<path>`.
fn fields(table: &toml::Table, at: &str) -> Result<Vec<Edit>, EngineError> {
    let Some(toml::Value::Table(f)) = table.get("fields") else {
        return Err(spec_error(&format!("{at}.fields"), "is required (a table)"));
    };
    let json = match to_json(&toml::Value::Table(f.clone())) {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    edits(ID, "", &json).map_err(|e| match e {
        EngineError::InvalidArgs { violations, .. } => {
            let v = violations.into_iter().next();
            let key = v
                .as_ref()
                .map(|v| {
                    v.pointer
                        .trim_start_matches('/')
                        .replace("~1", "/")
                        .replace("~0", "~")
                })
                .unwrap_or_default();
            let message = v.map(|v| v.message).unwrap_or_default();
            spec_error(&format!("{at}.fields.{key}"), message)
        }
        other => other,
    })
}

/// A parsed spec.
struct Spec {
    load_order: Option<Value>,
    rules: Vec<(String, [u8; 4], Query, Vec<Edit>)>,
    sets: Vec<(String, String, Option<String>, Vec<Edit>)>,
}

fn parse_spec(text: &str) -> Result<Spec, EngineError> {
    let root: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| spec_error("(toml)", e.to_string().trim_end()))?;
    only_keys(
        &root,
        "",
        &["version", "plugin", "load_order", "set", "rule"],
    )?;
    match root.get("version") {
        Some(toml::Value::Integer(1)) => {}
        Some(_) => return Err(spec_error("version", "must be 1 (the only spec version)")),
        None => return Err(spec_error("version", "is required (version = 1)")),
    }
    if let Some(plugin) = root.get("plugin") {
        let toml::Value::Table(p) = plugin else {
            return Err(spec_error("plugin", "must be a table"));
        };
        only_keys(p, "plugin", &["localized"])?;
        match p.get("localized") {
            None | Some(toml::Value::Boolean(false)) => {}
            Some(_) => {
                return Err(spec_error(
                    "plugin.localized",
                    "must be false (M2 writes no string tables; masters are computed)",
                ))
            }
        }
    }
    let load_order = match root.get("load_order") {
        None => None,
        Some(toml::Value::Table(t)) => {
            only_keys(
                t,
                "load_order",
                &["profile", "plugins", "data_dir", "no_cache", "language"],
            )?;
            Some(to_json(&toml::Value::Table(t.clone())))
        }
        Some(_) => return Err(spec_error("load_order", "must be a table")),
    };
    let entries = |key: &str| -> Result<Vec<toml::Table>, EngineError> {
        match root.get(key) {
            None => Ok(Vec::new()),
            Some(toml::Value::Array(items)) => items
                .iter()
                .enumerate()
                .map(|(i, v)| match v {
                    toml::Value::Table(t) => Ok(t.clone()),
                    _ => Err(spec_error(
                        &format!("{key}[{i}]"),
                        "must be a table ([[{key}]])",
                    )),
                })
                .collect(),
            Some(_) => Err(spec_error(
                key,
                format!("must be an array of tables ([[{key}]])"),
            )),
        }
    };
    let mut rules = Vec::new();
    for (i, t) in entries("rule")?.iter().enumerate() {
        let at = format!("rule[{i}]");
        only_keys(t, &at, &["type", "where", "fields"])?;
        let ty = string(t, "type", &at)?;
        let sig: [u8; 4] = ty.to_ascii_uppercase().as_bytes().try_into().map_err(|_| {
            spec_error(
                &format!("{at}.type"),
                "must be a four-character record type",
            )
        })?;
        let query = Query::parse(string(t, "where", &at)?)
            .map_err(|e| spec_error(&format!("{at}.where"), e))?;
        rules.push((at.clone(), sig, query, fields(t, &at)?));
    }
    let mut sets = Vec::new();
    for (i, t) in entries("set")?.iter().enumerate() {
        let at = format!("set[{i}]");
        only_keys(t, &at, &["form", "plugin", "fields"])?;
        let form = string(t, "form", &at)?.to_string();
        let plugin = match t.get("plugin") {
            None => None,
            Some(_) => Some(string(t, "plugin", &at)?.to_string()),
        };
        sets.push((at.clone(), form, plugin, fields(t, &at)?));
    }
    if rules.is_empty() && sets.is_empty() {
        return Err(spec_error(
            "set",
            "the spec has no [[set]] or [[rule]] entries",
        ));
    }
    Ok(Spec {
        load_order,
        rules,
        sets,
    })
}

/// Every winning record of `sig` matching `query`, in result order.
fn matches(
    loaded: &mut Loaded,
    sig: [u8; 4],
    query: &Query,
    at: &str,
) -> Result<(Vec<GlobalForm>, u64), EngineError> {
    let mut reader = RecordReader::new();
    let mut out = Vec::new();
    let mut unknown = 0u64;
    let mut cursor: Option<String> = None;
    loop {
        let request = PageRequest {
            signature: sig,
            query: Some(query),
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

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let spec_path = PathBuf::from(str_arg(ID, args, "spec")?);
    let out = PathBuf::from(str_arg(ID, args, "out")?);
    let text = std::fs::read_to_string(&spec_path)
        .map_err(|e| failed(ID, format!("{}: {e}", spec_path.display())))?;
    let spec = parse_spec(&text)?;
    let loaded = match &spec.load_order {
        Some(lo_args) => acquire(session, lo_args, ID)?,
        None => acquire(session, args, ID)?,
    };
    let mut limitations = loaded.limitations.clone();

    // Expand: rules in order, then sets; one entry per form, edits merged.
    let mut order: Vec<GlobalForm> = Vec::new();
    let mut per_form: BTreeMap<u64, Vec<(String, Edit)>> = BTreeMap::new();
    let mut add =
        |form: GlobalForm, at: &str, list: &[Edit], label: String| -> Result<(), EngineError> {
            let entry = per_form.entry(form.key()).or_default();
            if entry.is_empty() {
                order.push(form);
            }
            for edit in list {
                let key = (edit.path.signature, edit.path.occurrence, &edit.path.steps);
                if let Some((first, _)) = entry
                    .iter()
                    .find(|(_, e)| (e.path.signature, e.path.occurrence, &e.path.steps) == key)
                {
                    return Err(spec_error(
                        &format!("{at}.fields.{}", edit.path.text),
                        format!(
                            "{} of {label} is already written by {first}",
                            edit.path.text
                        ),
                    ));
                }
                entry.push((format!("{at}.fields.{}", edit.path.text), edit.clone()));
            }
            Ok(())
        };
    let mut expanded = Vec::new();
    for (at, sig, query, list) in &spec.rules {
        let (forms, unknown) = matches(loaded, *sig, query, at)?;
        if unknown > 0 {
            limitations.push(format!(
                "{at}: {unknown} record(s) could not be decided by `where` and were not changed"
            ));
        }
        expanded.push(json!({"rule": at, "matched": forms.len(), "undecided": unknown}));
        for form in forms {
            add(form, at, list, loaded.lo.form_label(form))?;
        }
    }
    for (at, form, plugin, list) in &spec.sets {
        let g = find_form(ID, loaded, form, plugin.as_deref())
            .map_err(|e| spec_error(&format!("{at}.form"), e))?;
        add(g, at, list, loaded.lo.form_label(g))?;
    }
    if order.is_empty() {
        return Err(failed(
            ID,
            "the spec's rules matched no records and it has no sets; nothing to write",
        ));
    }

    // Write fresh into a temporary directory next to `out` (same file name), validate,
    // then move it over `out`.
    let limits = DecodeLimits::default();
    let mut copies = Vec::new();
    let mut origins: Vec<Origin> = Vec::new();
    for form in &order {
        let list: Vec<Edit> = per_form
            .get(&form.key())
            .map(|v| v.iter().map(|(_, e)| e.clone()).collect())
            .unwrap_or_default();
        let (copy, origin) = winning_copy(ID, loaded, *form, list, &limits)?;
        copies.push(copy);
        origins.push(origin);
    }
    let name = out
        .file_name()
        .ok_or_else(|| failed(ID, format!("{}: not a file path", out.display())))?;
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temp_dir = parent.join(format!(".kitcraft-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| failed(ID, format!("{}: {e}", temp_dir.display())))?;
    let temp = temp_dir.join(name);
    let cleanup = || {
        let _ = std::fs::remove_dir_all(&temp_dir);
    };
    let written = write(ID, loaded, &temp, copies);
    let mut report = match written {
        Ok(r) => r,
        Err(e) => {
            cleanup();
            return Err(e);
        }
    };
    let plugins: Vec<PathBuf> = loaded.lo.plugins.iter().map(|p| p.path.clone()).collect();
    let options = Options {
        plugin: Some(temp.clone()),
        plugins,
        ..Options::default()
    };
    let checked = validate::validate(&options);
    let checked = match checked {
        Ok(c) => c,
        Err(m) => {
            cleanup();
            return Err(failed(ID, format!("validating the output: {m}")));
        }
    };
    if !checked.passed() {
        cleanup();
        let errors: Vec<String> = checked
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .take(20)
            .map(|f| {
                format!(
                    "{} {}: {}",
                    f.rule,
                    f.record.as_deref().unwrap_or("-"),
                    f.message
                )
            })
            .collect();
        return Err(failed(
            ID,
            format!(
                "the built plugin fails validate, so nothing was written: {}",
                errors.join("; ")
            ),
        ));
    }
    if let Err(e) = std::fs::rename(&temp, &out) {
        cleanup();
        return Err(failed(
            ID,
            format!("moving the build to {}: {e}", out.display()),
        ));
    }
    cleanup();
    report.target = out.clone();
    let mut result = outcome(&report, &origins, limitations);
    if let Some(obj) = result.data.as_object_mut() {
        obj.insert("spec".into(), json!(spec_path.display().to_string()));
        obj.insert("rules".into(), Value::Array(expanded));
        obj.insert(
            "validate".into(),
            json!({
                "passed": checked.passed(),
                "errors": checked.count(Severity::Error),
                "warnings": checked.count(Severity::Warning),
                "findings": checked.findings,
                "rules": checked.rules,
            }),
        );
        obj.insert("passed".into(), json!(true));
    }
    Ok(result)
}
