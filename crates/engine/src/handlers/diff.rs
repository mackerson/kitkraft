//! `diff` (M2.D12): record-level diff of two plugins, FormIDs matched through each file's
//! own masters by name (`kitkraft_doc::diff`).

use std::path::{Path, PathBuf};

use kitkraft_doc::compare::{OverrideClass, Span, SubDiff};
use kitkraft_doc::diff::{diff_plugins, Key};
use kitkraft_doc::fields::{FieldValue, FormValue};
use kitkraft_doc::FormRef;
use kitkraft_esm::HEADER_LEN;
use serde_json::{json, Value};

use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::shapes::kind;

const ID: &str = "diff";

/// Entries listed per category by default.
const DEFAULT_LIMIT: usize = 200;

fn span_json(s: Option<Span>) -> Value {
    s.map_or(
        Value::Null,
        |s| json!({"file_offset": s.file_offset, "body_offset": s.body_offset, "length": s.length}),
    )
}

/// A compared member value as JSON; opaque bytes as a range of the payload (and of the
/// file, when the payload's file offset is known).
fn field_value_json(v: Option<&FieldValue>, span: Option<Span>) -> Value {
    let Some(v) = v else { return Value::Null };
    match v {
        FieldValue::Int(i) => i64::try_from(*i).map_or_else(|_| json!(i.to_string()), |x| json!(x)),
        FieldValue::Float(f) => {
            serde_json::Number::from_f64(*f).map_or_else(|| json!(f.to_string()), Value::Number)
        }
        FieldValue::Form(FormValue::Null) => {
            json!({"kind": kind::FORMREF, "form": null, "null": true})
        }
        FieldValue::Form(FormValue::Form { label, .. }) => {
            json!({"kind": kind::FORMREF, "form": label})
        }
        FieldValue::Form(FormValue::Unresolved { raw, error }) => {
            json!({"kind": kind::FORMREF, "form": null, "raw": format!("{raw:08X}"), "error": error})
        }
        FieldValue::Sig(s) => json!(String::from_utf8_lossy(s)),
        FieldValue::Text(t) => json!(t),
        FieldValue::LStringId(id) => {
            json!({"kind": kind::LSTRING, "string_id": format!("0x{id:08X}")})
        }
        FieldValue::Opaque { offset, length } => json!({"kind": kind::OPAQUE, "opaque": {
            "payload_offset": offset,
            "length": length,
            "file_offset": span.and_then(|s| s.file_offset).map(|o| o.saturating_add(*offset)),
        }}),
        FieldValue::Empty => json!({}),
    }
}

/// One subrecord difference as JSON: signature, occurrence, change, and for a subrecord
/// compared through its layout the schema `member` and the `fields` that differ (path,
/// before, after); `as_bytes` says why a typed subrecord is reported as bytes.
pub(crate) fn sub_diff_json(s: &SubDiff, spans: bool) -> Value {
    let mut v = json!({
        "signature": s.signature,
        "occurrence": s.occurrence,
        "change": s.change.as_str(),
    });
    if let Some(obj) = v.as_object_mut() {
        if spans {
            obj.insert("a".into(), span_json(s.a));
            obj.insert("b".into(), span_json(s.b));
            obj.insert("first_difference".into(), json!(s.first_difference));
        }
        if let Some(m) = &s.member {
            obj.insert("member".into(), json!(m));
        }
        if let Some(fields) = &s.fields {
            obj.insert(
                "fields".into(),
                Value::Array(
                    fields
                        .iter()
                        .map(|f| {
                            json!({
                                "path": f.path,
                                "before": field_value_json(f.before.as_ref(), s.a),
                                "after": field_value_json(f.after.as_ref(), s.b),
                            })
                        })
                        .collect(),
                ),
            );
        }
        if let Some(why) = &s.as_bytes {
            obj.insert("as_bytes".into(), json!(why));
        }
    }
    v
}

/// Stated by `diff`, `conflicts` and `inspect`: how payloads are compared (M3.D2).
pub(crate) const FIELD_LEVEL: &str = "subrecords lane B's schema gives a layout are compared \
     member by member (`fields`: path, before, after), FormIDs resolved through each side's \
     masters by plugin name; members the schema keeps opaque are byte ranges, and subrecords \
     without a layout are compared as stored bytes";

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
    let own = |k: &Key| d.label(k);
    let mut evidence = Vec::new();
    let mut limitations = vec![FIELD_LEVEL.to_string()];
    if d.keyed_by == "raw" {
        limitations.push(
            "a master could not be read, so records are matched by raw FormID (both files list \
             the same masters in the same order and have the same own scale, so their encodings \
             agree)"
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
    if masters_differ || !d.same_encoding {
        limitations.push(format!(
            "the two files encode FormIDs differently (master lists or scales differ): typed \
             members compare by resolved form ({} record(s) differ in bytes only and count as \
             unchanged), but a byte difference inside an opaque payload may be encoding only",
            d.equal_by_value
        ));
    }
    if d.unresolved.0 > 0 || d.unresolved.1 > 0 {
        limitations.push(format!(
            "{} record(s) in a and {} in b have FormIDs that do not resolve and are not compared ({})",
            d.unresolved.0,
            d.unresolved.1,
            d.unresolved.2.clone().unwrap_or_default()
        ));
    }
    let listed = |v: &[(Key, kitkraft_doc::RecordLoc)],
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
            "subrecords": subs.iter().map(|s| sub_diff_json(s, true)).collect::<Vec<_>>(),
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
            "equal_by_value": d.equal_by_value,
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
            decoded_subrecords: None,
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
            json!({"added": 0, "removed": 0, "changed": 0, "unchanged": 3, "equal_by_value": 0})
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
        // Review round 2: plugin names keep the files' spelling, as in other commands.
        assert_eq!(data["changed"][0]["form"], "Base.esm:000800");
        assert_eq!(data["removed"][0]["form"], "Base.esm:000802");
        assert_eq!(data["changed"][0]["subrecords"][0]["signature"], "FULL");
        // M3.D2: FULL has a layout, so the member that changed is named with both values.
        assert_eq!(data["changed"][0]["subrecords"][0]["member"], "Name");
        assert_eq!(
            data["changed"][0]["subrecords"][0]["fields"],
            json!([{
                "path": "FULL.Name",
                "before": "Old Earth Hunting Rifle",
                "after": "Old Soviet Marksman Rifle",
            }])
        );
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
            json!({"added": 0, "removed": 0, "changed": 0, "unchanged": 1, "equal_by_value": 0})
        );
        // Raw fallback (a master missing, same masters and same own scale): said so.
        use crate::handlers::load::tests::plugin;
        let a = dir.join("RawA.esm");
        let b = dir.join("RawB.esm");
        std::fs::write(&a, plugin(0x1, &["Gone.esm"], &[(b"WEAP", 0x800, vec![])])).unwrap();
        std::fs::write(
            &b,
            plugin(
                0x1,
                &["Gone.esm"],
                &[(
                    b"WEAP",
                    0x800,
                    crate::handlers::testdata::sub(b"DATA", &[1]),
                )],
            ),
        )
        .unwrap();
        let raw = execute(
            "diff",
            json!({"a": a.display().to_string(), "b": b.display().to_string()}),
        )
        .unwrap();
        assert_eq!(raw["data"]["keyed_by"], "raw");
        assert_eq!(raw["data"]["changed"][0]["form"], "raw:00000800");
        assert!(raw["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("same own scale")));
        // M3.D2: the same keywords under another master order are equal by name.
        use crate::handlers::testdata::sub;
        let gun = |id: u32, kw: [u32; 2]| {
            (
                b"WEAP",
                id,
                [
                    sub(b"EDID", b"Gun\0"),
                    sub(b"KSIZ", &2u32.to_le_bytes()),
                    sub(
                        b"KWDA",
                        &[kw[0].to_le_bytes(), kw[1].to_le_bytes()].concat(),
                    ),
                ]
                .concat(),
            )
        };
        std::fs::write(dir.join("X.esm"), plugin(0x1, &[], &[])).unwrap();
        std::fs::write(dir.join("Z.esm"), plugin(0x1, &[], &[])).unwrap();
        let xa = dir.join("XZ.esm");
        let xb = dir.join("ZX.esm");
        std::fs::write(
            &xa,
            plugin(
                0x1,
                &["X.esm", "Z.esm"],
                &[gun(0x800, [0x900, 0x0100_0901])],
            ),
        )
        .unwrap();
        std::fs::write(
            &xb,
            plugin(
                0x1,
                &["Z.esm", "X.esm"],
                &[gun(0x0100_0800, [0x0100_0900, 0x901])],
            ),
        )
        .unwrap();
        let same = execute(
            "diff",
            json!({"a": xa.display().to_string(), "b": xb.display().to_string()}),
        )
        .unwrap();
        assert_eq!(
            same["data"]["counts"],
            json!({"added": 0, "removed": 0, "changed": 0, "unchanged": 1, "equal_by_value": 1})
        );
        assert!(same["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("encode FormIDs differently")));
        let (a, b) = (xa.display().to_string(), xb.display().to_string());
        let err = execute("diff", json!({"a": a, "b": b, "form": "nope"})).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert_eq!(
            execute("diff", json!({"a": "x"})).unwrap_err().kind(),
            "invalid_args"
        );
    }
}
