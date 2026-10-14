//! `diff` (M2.D12): record-level diff of two plugins, FormIDs matched through each file's
//! own masters by name (`kitcraft_doc::diff`).

use std::path::{Path, PathBuf};

use kitcraft_doc::compare::{OverrideClass, Span};
use kitcraft_doc::diff::{diff_plugins, Key};
use kitcraft_doc::FormRef;
use kitcraft_esm::HEADER_LEN;
use serde_json::{json, Value};

use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

const ID: &str = "diff";

/// Entries listed per category by default.
const DEFAULT_LIMIT: usize = 200;

fn span_json(s: Option<Span>) -> Value {
    s.map_or(
        Value::Null,
        |s| json!({"file_offset": s.file_offset, "body_offset": s.body_offset, "length": s.length}),
    )
}

/// The handler.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let a = str_arg(ID, args, "a")?;
    let b = str_arg(ID, args, "b")?;
    let (pa, pb) = (Path::new(a), Path::new(b));
    let (na, nb) = (file_name(pa), file_name(pb));
    let extra: Vec<PathBuf> = super::data_dir(args).into_iter().collect();
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|l| usize::try_from(l).ok())
        .unwrap_or(DEFAULT_LIMIT);
    let only = match args.get("form").and_then(Value::as_str) {
        None => None,
        Some(f) => Some(FormRef::parse(f).ok_or_else(|| {
            failed(
                ID,
                format!("form `{f}` is not a formref (Plugin.esm:XXXXXX)"),
            )
        })?),
    };
    let d = diff_plugins(pa, pb, &extra, only.as_ref()).map_err(|e| failed(ID, e.to_string()))?;
    let own = |k: &Key| k.label(&na);
    let mut evidence = Vec::new();
    let mut limitations = vec![
        "payloads are compared as stored bytes (not decoded through the schema), so a change \
         is reported per subrecord and byte range, not per field"
            .to_string(),
    ];
    if d.keyed_by == "raw" {
        limitations.push(
            "a master could not be read, so records are matched by raw FormID (both files list \
             the same masters in the same order)"
                .to_string(),
        );
    }
    let masters_differ = d.a.header.masters.len() != d.b.header.masters.len()
        || d.a
            .header
            .masters
            .iter()
            .zip(&d.b.header.masters)
            .any(|(x, y)| !x.eq_ignore_ascii_case(y));
    if masters_differ {
        limitations.push(
            "the two files list their masters differently: FormIDs inside payloads are encoded \
             per file, so a byte difference may be encoding only"
                .to_string(),
        );
    }
    if d.unresolved.0 > 0 || d.unresolved.1 > 0 {
        limitations.push(format!(
            "{} record(s) in a and {} in b have FormIDs that do not resolve and are not compared ({})",
            d.unresolved.0,
            d.unresolved.1,
            d.unresolved.2.clone().unwrap_or_default()
        ));
    }
    let listed = |v: &[(Key, kitcraft_doc::RecordLoc)],
                  file: &str,
                  ev: &mut Vec<Evidence>|
     -> Vec<Value> {
        v.iter()
            .take(limit)
            .map(|(k, loc)| {
                let label = format!("{}:{:08X}", loc.signature(), loc.form_id);
                ev.push(Evidence {
                    file: file.to_string(),
                    offset: loc.offset,
                    length: u64::from(loc.data_size).saturating_add(HEADER_LEN as u64),
                    record: Some(label),
                    subrecord: None,
                });
                json!({"form": own(k), "signature": loc.signature().to_string(), "raw_form_id": format!("{:08X}", loc.form_id), "offset": loc.offset})
            })
            .collect()
    };
    let added = listed(&d.added, &nb, &mut evidence);
    let removed = listed(&d.removed, &na, &mut evidence);
    let mut changed = Vec::new();
    for c in d.changed.iter().take(limit) {
        let (flags, order, subs) = match &c.class {
            OverrideClass::Differs {
                flags,
                order,
                subrecords,
            } => (*flags, *order, subrecords.clone()),
            OverrideClass::Deleted => (true, false, Vec::new()),
            OverrideClass::IdenticalToPrevious => (false, false, Vec::new()),
        };
        let (la, lb) = (c.a.record.label(), c.b.record.label());
        for (file, rec, label) in [(&na, &c.a.record, &la), (&nb, &c.b.record, &lb)] {
            evidence.push(Evidence {
                file: file.clone(),
                offset: rec.offset(),
                length: HEADER_LEN as u64,
                record: Some(label.clone()),
                subrecord: None,
            });
        }
        for s in &subs {
            for (file, span, label, rec) in
                [(&na, s.a, &la, &c.a.record), (&nb, s.b, &lb, &c.b.record)]
            {
                let Some(span) = span else { continue };
                evidence.push(match span.file_offset {
                    Some(o) => Evidence {
                        file: file.clone(),
                        offset: o,
                        length: span.length,
                        record: Some(label.clone()),
                        subrecord: Some(s.signature.clone()),
                    },
                    None => Evidence {
                        file: file.clone(),
                        offset: rec.offset().saturating_add(HEADER_LEN as u64),
                        length: rec.data_size(),
                        record: Some(label.clone()),
                        subrecord: None,
                    },
                });
            }
        }
        changed.push(json!({
            "form": own(&c.key),
            "signature": c.a.loc.signature().to_string(),
            "class": c.class.as_str(),
            "a": {"raw_form_id": format!("{:08X}", c.a.loc.form_id), "offset": c.a.loc.offset, "flags": format!("0x{:08X}", c.a.loc.flags), "compressed": c.a.record.compressed},
            "b": {"raw_form_id": format!("{:08X}", c.b.loc.form_id), "offset": c.b.loc.offset, "flags": format!("0x{:08X}", c.b.loc.flags), "compressed": c.b.record.compressed},
            "flags_differ": flags,
            "order_differs": order,
            "subrecords": subs.iter().map(|s| json!({
                "signature": s.signature,
                "occurrence": s.occurrence,
                "change": s.change.as_str(),
                "a": span_json(s.a),
                "b": span_json(s.b),
                "first_difference": s.first_difference,
            })).collect::<Vec<_>>(),
        }));
    }
    let truncated = d.added.len() > limit || d.removed.len() > limit || d.changed.len() > limit;
    if truncated {
        limitations.push(format!(
            "lists show at most {limit} entries each (counts are complete)"
        ));
    }
    if d.changed
        .iter()
        .any(|c| c.a.record.compressed || c.b.record.compressed)
    {
        limitations.push(
            "inside compressed records subrecord offsets are relative to the inflated body \
             (body_offset); evidence then covers the whole compressed body"
                .to_string(),
        );
    }
    let summary = format!(
        "{na} -> {nb}: {} added, {} removed, {} changed, {} unchanged (matched by {})",
        d.added.len(),
        d.removed.len(),
        d.changed.len(),
        d.unchanged,
        d.keyed_by
    );
    let data = json!({
        "a": a,
        "b": b,
        "form": args.get("form"),
        "keyed_by": d.keyed_by,
        "counts": {
            "added": d.added.len(),
            "removed": d.removed.len(),
            "changed": d.changed.len(),
            "unchanged": d.unchanged,
        },
        "added": added,
        "removed": removed,
        "changed": changed,
        "summary": summary,
    });
    Ok(Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: Some(0),
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]
mod tests {
    use crate::execute;
    use crate::handlers::load::tests::fixture;
    use serde_json::json;

    #[test]
    fn diff_through_execute() {
        let dir = fixture("diff");
        let base = dir.join("Base.esm");
        let patch = dir.join("Patch.esm");
        let same = execute(
            "diff",
            json!({"a": base.display().to_string(), "b": base.display().to_string()}),
        )
        .unwrap();
        assert_eq!(
            same["data"]["counts"],
            json!({"added": 0, "removed": 0, "changed": 0, "unchanged": 3})
        );
        // Base vs Patch: Patch's own numbering differs (it has a master), records match by name.
        let d = execute(
            "diff",
            json!({"a": base.display().to_string(), "b": patch.display().to_string()}),
        )
        .unwrap();
        let data = &d["data"];
        assert_eq!(data["keyed_by"], "name");
        assert_eq!(data["counts"]["changed"], 1);
        assert_eq!(data["counts"]["removed"], 1);
        assert_eq!(data["changed"][0]["form"], "base.esm:000800");
        assert_eq!(data["changed"][0]["subrecords"][0]["signature"], "FULL");
        let b_off = data["changed"][0]["subrecords"][0]["b"]["file_offset"]
            .as_u64()
            .unwrap() as usize;
        let bytes = std::fs::read(&patch).unwrap();
        assert_eq!(&bytes[b_off..b_off + 3], b"Old");
        assert!(d["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["file"] == "Patch.esm" && e["offset"].as_u64() == Some(b_off as u64)));
        // One form only.
        let one = execute(
            "diff",
            json!({"a": base.display().to_string(), "b": patch.display().to_string(), "form": "Base.esm:000801"}),
        )
        .unwrap();
        assert_eq!(
            one["data"]["counts"],
            json!({"added": 0, "removed": 0, "changed": 0, "unchanged": 1})
        );
        let err = execute("diff", json!({"a": "x", "b": "y", "form": "nope"})).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert_eq!(
            execute("diff", json!({"a": "x"})).unwrap_err().kind(),
            "invalid_args"
        );
    }
}
