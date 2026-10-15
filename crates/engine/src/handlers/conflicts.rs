//! `conflicts` (M2.D13, port of our `esmscan.py`): records that plugins override from a
//! master, counted per plugin and record type as `esmscan.py` prints them, and listed a page
//! at a time with each override classed against the version before it (M2.D6).

use std::collections::BTreeMap;

use kitkraft_doc::compare::OverrideClass;
use kitkraft_doc::conflicts::{classify_run, overridden, signature_of, Filter};
use kitkraft_doc::hash::Fnv;
use kitkraft_doc::query::{decode_cursor, encode_cursor};
use kitkraft_doc::{RecordReader, Version};
use kitkraft_esm::HEADER_LEN;
use serde_json::{json, Value};

use super::failed;
use super::load::{acquire, Loaded};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

const ID: &str = "conflicts";

/// Default page size.
const DEFAULT_LIMIT: usize = 50;
/// Largest page size.
const MAX_LIMIT: usize = 1000;

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let loaded = acquire(session, args, ID)?;
    run_on(loaded, args)
}

fn sig_text(s: Option<[u8; 4]>) -> String {
    s.map_or_else(
        || "????".into(),
        |s| String::from_utf8_lossy(&s).into_owned(),
    )
}

fn run_on(loaded: &Loaded, args: &Value) -> Result<Outcome, EngineError> {
    let lo = &loaded.lo;
    let plugin_arg = |name: &str| -> Result<Option<u16>, EngineError> {
        match args.get(name).and_then(Value::as_str) {
            None => Ok(None),
            Some(p) => lo
                .find(p)
                .map(Some)
                .ok_or_else(|| failed(ID, format!("`{p}` is not in the load order"))),
        }
    };
    let master = plugin_arg("master")?;
    let plugin = plugin_arg("plugin")?;
    let signature = match args.get("type").and_then(Value::as_str) {
        None => None,
        Some(t) => Some(
            <[u8; 4]>::try_from(t.to_ascii_uppercase().as_bytes())
                .map_err(|_| failed(ID, format!("`{t}` is not a four-character record type")))?,
        ),
    };
    let filter = Filter {
        master,
        plugin,
        signature,
        only_conflicts: args.get("only_conflicts").and_then(Value::as_bool) == Some(true),
    };
    let count_classes = args.get("count_classes").and_then(Value::as_bool) == Some(true);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|l| usize::try_from(l).ok())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);

    // esmscan.py's counts: per (master, overriding plugin), per record type.
    let mut counts: BTreeMap<(u16, u16), BTreeMap<String, u64>> = BTreeMap::new();
    for run in lo.forms.runs() {
        let Some(first) = run.first() else { continue };
        let defining = first.form().plugin;
        if master.is_some_and(|m| m != defining) {
            continue;
        }
        for v in run.iter().filter(|v| v.plugin != defining) {
            if plugin.is_some_and(|p| p != v.plugin) {
                continue;
            }
            let sig = signature_of(lo, v);
            if signature.is_some() && sig != signature {
                continue;
            }
            let n = counts
                .entry((defining, v.plugin))
                .or_default()
                .entry(sig_text(sig))
                .or_insert(0);
            *n = n.saturating_add(1);
        }
    }
    let name = |i: u16| lo.plugin(i).map_or_else(String::new, |p| p.name.clone());
    let counts_json: Vec<Value> = counts
        .iter()
        .map(|((m, p), types)| {
            json!({
                "master": name(*m),
                "plugin": name(*p),
                "total": types.values().sum::<u64>(),
                "types": types,
            })
        })
        .collect();

    let schema = kitkraft_schema::builtin().ok();
    let forms = overridden(lo, &filter);
    let mut fp = Fnv::new();
    fp.write_u64(lo.fingerprint());
    fp.write(format!("{filter:?}").as_bytes());
    let fp = fp.finish();
    let start = match args.get("cursor").and_then(Value::as_str) {
        Some(c) => decode_cursor(c, fp).map_err(|e| failed(ID, e))?,
        None => 0,
    };
    let mut reader = RecordReader::new();
    let mut evidence = Vec::new();
    let mut items = Vec::new();
    let mut page_classes: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut incomparable = 0u64;
    let end = start.saturating_add(limit).min(forms.len());
    for run in forms.get(start..end).unwrap_or(&[]) {
        let c =
            classify_run(lo, &mut reader, run, schema).map_err(|e| failed(ID, e.to_string()))?;
        let base = run
            .iter()
            .find(|v| v.plugin == c.form.plugin)
            .or(run.first());
        let edid = base
            .and_then(|b| Some((b, lo.record(b.plugin, b.record)?)))
            .and_then(|(b, l)| lo.edid(b.plugin, l))
            .map(str::to_string);
        let mut overrides = Vec::new();
        for o in &c.overrides {
            let label = o
                .class
                .as_ref()
                .map_or("first-version", OverrideClass::as_str);
            let n = page_classes.entry(label).or_insert(0);
            *n = n.saturating_add(1);
            if !o.comparable {
                incomparable = incomparable.saturating_add(1);
            }
            push_evidence(loaded, &o.version, o.class.as_ref(), &mut evidence);
            overrides.push(json!({
                "plugin": name(o.version.plugin),
                "previous": o.previous.map(|p| name(p.plugin)),
                "class": label,
                "resolved_by": o.version.basis.as_str(),
                "comparable": o.comparable,
                "subrecords": match &o.class {
                    Some(OverrideClass::Differs { subrecords, .. }) => subrecords
                        .iter()
                        .map(|d| super::diff::sub_diff_json(d, false))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                },
                "flags_differ": matches!(o.class, Some(OverrideClass::Differs { flags: true, .. })),
            }));
        }
        items.push(json!({
            "form": lo.form_label(c.form),
            "signature": sig_text(base.and_then(|b| signature_of(lo, b))),
            "editor_id": edid,
            "base_present": c.base_present,
            "versions": run.iter().map(|v| name(v.plugin)).collect::<Vec<_>>(),
            "overrides": overrides,
        }));
    }
    let next_cursor = (end < forms.len()).then(|| encode_cursor(fp, end));

    let mut all_classes = Value::Null;
    if count_classes {
        let mut totals: BTreeMap<&'static str, u64> = BTreeMap::new();
        for run in &forms {
            let c = classify_run(lo, &mut reader, run, schema)
                .map_err(|e| failed(ID, e.to_string()))?;
            for o in &c.overrides {
                let label = o
                    .class
                    .as_ref()
                    .map_or("first-version", OverrideClass::as_str);
                let n = totals.entry(label).or_insert(0);
                *n = n.saturating_add(1);
            }
        }
        all_classes = json!(totals);
    }

    let mut limitations = vec![super::diff::FIELD_LEVEL.to_string()];
    limitations.extend(loaded.limitations.iter().cloned());
    if incomparable > 0 {
        limitations.push(format!(
            "{incomparable} override(s) on this page are not `comparable`: the two plugins encode \
             FormIDs differently, so a difference inside an opaque payload may be encoding only"
        ));
    }
    if !count_classes {
        limitations.push(
            "class_counts cover this page only; pass count_classes for every matching form"
                .to_string(),
        );
    }
    let total: u64 = counts.values().flat_map(|t| t.values()).sum();
    let summary = format!(
        "{} override record(s) of {} form(s){}{}; page {}..{} classed: {}",
        total,
        forms.len(),
        master.map_or_else(String::new, |m| format!(" from {}", name(m))),
        plugin.map_or_else(String::new, |p| format!(" by {}", name(p))),
        start,
        end,
        page_classes
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let data = json!({
        "master": master.map(name),
        "plugin": plugin.map(name),
        "type": signature.map(|s| sig_text(Some(s))),
        "only_conflicts": filter.only_conflicts,
        "counts": counts_json,
        "override_records": total,
        "forms": forms.len(),
        "items": items,
        "class_counts": page_classes,
        "class_counts_all": all_classes,
        "next_cursor": next_cursor,
        "summary": summary,
    });
    Ok(Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
}

/// The override's record header, and the payloads of the subrecords that differ.
fn push_evidence(
    loaded: &Loaded,
    v: &Version,
    class: Option<&OverrideClass>,
    evidence: &mut Vec<Evidence>,
) {
    let lo = &loaded.lo;
    let (Some(p), Some(loc)) = (lo.plugin(v.plugin), lo.record(v.plugin, v.record)) else {
        return;
    };
    let label = format!("{}:{:08X}", loc.signature(), loc.form_id);
    evidence.push(Evidence {
        file: p.name.clone(),
        offset: loc.offset,
        length: HEADER_LEN as u64,
        record: Some(label.clone()),
        subrecord: None,
    });
    if let Some(OverrideClass::Differs { subrecords, .. }) = class {
        for d in subrecords {
            if let Some(span) = d.b {
                evidence.push(Evidence {
                    file: p.name.clone(),
                    offset: span
                        .file_offset
                        .unwrap_or_else(|| loc.offset.saturating_add(HEADER_LEN as u64)),
                    length: if span.file_offset.is_some() {
                        span.length
                    } else {
                        u64::from(loc.data_size)
                    },
                    record: Some(label.clone()),
                    subrecord: span.file_offset.map(|_| d.signature.clone()),
                });
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]
mod tests {
    use crate::execute;
    use crate::handlers::load::tests::{fixture, plugins_args};
    use serde_json::json;

    #[test]
    fn conflicts_through_execute() {
        let dir = fixture("conflicts");
        let mut args = plugins_args(&dir);
        args["master"] = json!("Base.esm");
        let r = execute("conflicts", args.clone()).unwrap();
        let data = &r["data"];
        assert_eq!(
            data["counts"],
            json!([{"master": "Base.esm", "plugin": "Patch.esm", "total": 2, "types": {"WEAP": 2}}])
        );
        assert_eq!(data["forms"], 2);
        let items = data["items"].as_array().unwrap();
        assert_eq!(items[0]["form"], "Base.esm:000800");
        assert_eq!(items[0]["editor_id"], "OldEarthHuntingRifle");
        assert_eq!(items[0]["overrides"][0]["class"], "differs");
        assert_eq!(
            items[0]["overrides"][0]["subrecords"][0]["signature"],
            "FULL"
        );
        // M3.D2: the member that changed, with both values.
        assert_eq!(
            items[0]["overrides"][0]["subrecords"][0]["fields"][0],
            json!({"path": "FULL.Name", "before": "Old Earth Hunting Rifle", "after": "Old Soviet Marksman Rifle"})
        );
        assert_eq!(items[1]["overrides"][0]["class"], "identical-to-previous");
        assert_eq!(
            data["class_counts"],
            json!({"differs": 1, "identical-to-previous": 1})
        );
        let ev = r["evidence"].as_array().unwrap();
        assert!(ev
            .iter()
            .any(|e| e["subrecord"] == "FULL" && e["file"] == "Patch.esm"));
        assert_eq!(items[0]["overrides"][0]["resolved_by"], "direct");
        // Review round 2: the load order's limitations are repeated here too.
        assert!(r["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("runtime-slots-by-scale")));

        // Paging and the all-forms class count.
        args["limit"] = json!(1);
        args["count_classes"] = json!(true);
        let first = execute("conflicts", args.clone()).unwrap();
        assert_eq!(first["data"]["items"].as_array().unwrap().len(), 1);
        assert_eq!(
            first["data"]["class_counts_all"],
            json!({"differs": 1, "identical-to-previous": 1})
        );
        args["cursor"] = first["data"]["next_cursor"].clone();
        let second = execute("conflicts", args.clone()).unwrap();
        assert_eq!(second["data"]["items"][0]["form"], "Base.esm:000801");
        assert!(second["data"]["next_cursor"].is_null());

        // Filters: only conflicts (none: one overrider each), a type with no overrides.
        let mut only = plugins_args(&dir);
        only["only_conflicts"] = json!(true);
        assert_eq!(execute("conflicts", only).unwrap()["data"]["forms"], 0);
        let mut kywd = plugins_args(&dir);
        kywd["type"] = json!("kywd");
        let k = execute("conflicts", kywd).unwrap();
        assert_eq!(k["data"]["forms"], 0);
        assert_eq!(k["data"]["counts"], json!([]));
        let mut bad = plugins_args(&dir);
        bad["master"] = json!("Nope.esm");
        assert_eq!(execute("conflicts", bad).unwrap_err().kind(), "failed");
    }
}
