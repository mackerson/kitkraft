//! `validate` (M2.E6): the rule registry in [`crate::validate`] over one plugin and/or
//! compiled scripts. `data.passed` is false on any error finding (CLI exit 1).

use std::path::PathBuf;

use serde_json::{json, Value};

use super::failed;
use super::load::{acquire, Loaded};
use super::resolve::resolve;
use crate::args::Violation;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;
use crate::validate::{self, Options, Severity};

const ID: &str = "validate";

/// Findings listed in `data.findings` (all are counted).
pub(crate) const MAX_LISTED: usize = 1000;

fn strings(args: &Value, name: &str) -> Vec<String> {
    args.get(name)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn invalid(pointer: String, rule: &'static str, message: String) -> EngineError {
    EngineError::InvalidArgs {
        command: ID.to_string(),
        violations: vec![Violation {
            pointer,
            rule,
            message,
        }],
    }
}

/// Reads the arguments into [`Options`]; an unknown rule id is `invalid_args`.
pub(crate) fn options(args: &Value) -> Result<Options, EngineError> {
    let data_dir = super::data_dir(args);
    // `run` resolves bare names afterwards (`super::resolve`).
    let plugin = args
        .get("plugin")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let scripts: Vec<PathBuf> = strings(args, "scripts")
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let plugin_archives = args
        .get("plugin_archives")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if plugin_archives && plugin.is_none() {
        return Err(invalid(
            "/plugin_archives".into(),
            "required",
            "plugin_archives needs `plugin` (its archives are found next to it)".into(),
        ));
    }
    if plugin.is_none() && scripts.is_empty() {
        return Err(invalid(
            String::new(),
            "required",
            "give `plugin`, `scripts`, or both".into(),
        ));
    }
    let rules = match args.get("rules").and_then(Value::as_array) {
        None => None,
        Some(list) => {
            let mut ids = Vec::new();
            for (i, v) in list.iter().enumerate() {
                let text = v.as_str().unwrap_or_default();
                let rule = validate::rule(text).ok_or_else(|| {
                    let known: Vec<&str> = validate::RULES.iter().map(|r| r.id).collect();
                    invalid(
                        format!("/rules/{i}"),
                        "enum",
                        format!("unknown rule `{text}`; rules: {}", known.join(", ")),
                    )
                })?;
                ids.push(rule.id);
            }
            Some(ids)
        }
    };
    Ok(Options {
        plugin,
        scripts,
        plugin_archives,
        rules,
        plugins: strings(args, "plugins")
            .into_iter()
            .map(PathBuf::from)
            .collect(),
        data_dir,
        limits: kitcraft_esm::DecodeLimits::default(),
        indexed: std::collections::HashMap::new(),
    })
}

/// The load order's (cached) record indexes, for [`Options::indexed`].
pub(crate) fn indexes(
    lo: &kitcraft_doc::LoadOrder,
) -> std::collections::HashMap<String, std::sync::Arc<std::collections::HashMap<u32, u64>>> {
    lo.plugins
        .iter()
        .filter_map(|p| {
            let index = p.index.as_deref()?;
            let ids = index
                .records
                .iter()
                .map(|r| (r.form_id, r.offset))
                .collect();
            Some((p.name.to_ascii_lowercase(), std::sync::Arc::new(ids)))
        })
        .collect()
}

/// The handler. Masters come from the load order when one is named (`profile`,
/// `plugins`) or loaded in the session, else from next to the plugin and `data_dir`.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let mut options = options(args)?;
    // Bare names resolve against the session's load order as it was before this call.
    let find = |at: String, path: &PathBuf| match path.to_str() {
        Some(name) => resolve(Some(&*session), options.data_dir.as_deref(), ID, &at, name),
        None => Ok(path.clone()),
    };
    let plugin = match &options.plugin {
        Some(p) => Some(find("/plugin".into(), p)?),
        None => None,
    };
    let mut scripts = Vec::with_capacity(options.scripts.len());
    for (i, s) in options.scripts.iter().enumerate() {
        scripts.push(find(format!("/scripts/{i}"), s)?);
    }
    (options.plugin, options.scripts) = (plugin, scripts);
    let named = args.get("profile").is_some() || args.get("plugins").is_some();
    let mut limitations = Vec::new();
    if named || session.get::<Loaded>().is_some() {
        let loaded = acquire(session, args, ID)?;
        options.plugins = loaded.lo.plugins.iter().map(|p| p.path.clone()).collect();
        options.indexed = indexes(&loaded.lo);
        limitations.extend(loaded.limitations.iter().cloned());
    }
    let report = validate::validate(&options).map_err(|m| failed(ID, m))?;
    let mut out = outcome(&options, &report);
    for l in limitations {
        if !out.limitations.contains(&l) {
            out.limitations.push(l);
        }
    }
    Ok(out)
}

/// The envelope for a report (shared with `build`, which validates its output).
pub(crate) fn outcome(options: &Options, report: &validate::Report) -> Outcome {
    let errors = report.count(Severity::Error);
    let warnings = report.count(Severity::Warning);
    let passed = report.passed();
    let plugin = options.plugin.as_ref().map(|p| p.display().to_string());
    let target = options
        .plugin
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "scripts".into());
    let summary = format!(
        "{target}: {} ({errors} error(s), {warnings} warning(s); rules: {})",
        if passed { "passed" } else { "FAILED" },
        report.rules.join(", ")
    );
    let evidence: Vec<Evidence> = report
        .findings
        .iter()
        .take(MAX_LISTED)
        .filter_map(|f| {
            Some(Evidence {
                file: f.plugin.clone(),
                offset: f.offset?,
                length: 1,
                record: f.record.clone(),
                subrecord: f.subrecord.clone(),
            })
        })
        .collect();
    let mut limitations = report.limitations.clone();
    if report.findings.len() > MAX_LISTED {
        limitations.push(format!(
            "data.findings lists the first {MAX_LISTED} of {} findings",
            report.findings.len()
        ));
    }
    if report.unchecked_subrecords > 0 || report.unresolved_form_ids > 0 {
        limitations.push(format!(
            "formid.dangling checked {} FormIDs; {} subrecords are opaque or undecoded (FormIDs inside them unchecked) and {} FormIDs could not be resolved without guessing a master's scale",
            report.checked_form_ids, report.unchecked_subrecords, report.unresolved_form_ids
        ));
    }
    let complete = report.unchecked_subrecords == 0 && report.unresolved_form_ids == 0;
    Outcome {
        data: json!({
            "plugin": plugin,
            "scripts": report.script_inputs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "passed": passed,
            "errors": errors,
            "warnings": warnings,
            "rules": report.rules,
            "findings": report.findings.iter().take(MAX_LISTED).collect::<Vec<_>>(),
            "findings_total": report.findings.len(),
            "masters": report.masters,
            "records": report.records,
            "groups": report.groups,
            "subrecords": report.subrecords,
            "checked_form_ids": report.checked_form_ids,
            "unresolved_form_ids": report.unresolved_form_ids,
            "unchecked_subrecords": report.unchecked_subrecords,
            "scripts_checked": report.scripts,
            "summary": summary,
        }),
        coverage: Coverage {
            status: if complete {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: Some(
                report
                    .subrecords
                    .saturating_sub(report.unchecked_subrecords),
            ),
            opaque_subrecords: Some(report.unchecked_subrecords),
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    }
}
