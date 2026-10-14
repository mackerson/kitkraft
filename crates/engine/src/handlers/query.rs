//! `query` (M2.D11): records of one type matching a `where` predicate, over the load order,
//! a page at a time. Grammar and semantics: `kitcraft_doc::query`, `docs/mcp.md`.

use kitcraft_doc::confidence::{reference_limitation, reference_members};
use kitcraft_doc::flags::RECORD_FLAG_DELETED;
use kitcraft_doc::query::Query;
use kitcraft_doc::search::{run_page_with, PageRequest, Scope, DEFAULT_LIMIT};
use kitcraft_doc::{RecordReader, Version};
use kitcraft_esm::HEADER_LEN;
use serde_json::{json, Value};

use super::failed;
use super::load::{acquire, lstring_json, schema_of, Loaded, RUNTIME_ASSUMED};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

const ID: &str = "query";

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let schema = schema_of(session);
    let loaded = acquire(session, args, ID)?;
    run_on(loaded, args, schema)
}

fn run_on(
    loaded: &mut Loaded,
    args: &Value,
    schema: Option<&kitcraft_schema::Schema>,
) -> Result<Outcome, EngineError> {
    let Loaded {
        lo,
        strings,
        limitations: shared,
        ..
    } = loaded;
    let lo = &*lo;
    let type_text = args.get("type").and_then(Value::as_str).unwrap_or_default();
    let signature: [u8; 4] = type_text
        .to_ascii_uppercase()
        .as_bytes()
        .try_into()
        .map_err(|_| {
            failed(
                ID,
                format!("`{type_text}` is not a four-character record type"),
            )
        })?;
    let where_text = args.get("where").and_then(Value::as_str);
    let query = match where_text {
        Some(t) => Some(Query::parse(t).map_err(|e| failed(ID, format!("where: {e}")))?),
        None => None,
    };
    let scope = match args.get("scope").and_then(Value::as_str) {
        Some("all") => Scope::All,
        _ => Scope::Winner,
    };
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|l| usize::try_from(l).ok())
        .unwrap_or(DEFAULT_LIMIT);
    let cursor = args.get("cursor").and_then(Value::as_str);
    let started = std::time::Instant::now();
    let mut reader = RecordReader::new();
    let request = PageRequest {
        signature,
        query: query.as_ref(),
        scope,
        limit,
        cursor,
    };
    let page = run_page_with(lo, &mut reader, &request, Some(&mut *strings), schema)
        .map_err(|e| failed(ID, e))?;

    let mut evidence = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut results = Vec::with_capacity(page.matched.len());
    for v in &page.matched {
        results.push(result_json(
            lo,
            strings,
            &mut reader,
            v,
            &mut evidence,
            &mut notes,
        ));
    }
    let unknown_records: Vec<Value> = page
        .unknown
        .iter()
        .map(|(v, reasons)| {
            json!({
                "form": lo.form_label(v.form()),
                "plugin": lo.plugin(v.plugin).map(|p| p.name.clone()),
                "reasons": reasons,
            })
        })
        .collect();
    let mut limitations = shared.clone();
    if !limitations.iter().any(|l| l == RUNTIME_ASSUMED) {
        limitations.push(RUNTIME_ASSUMED.to_string());
    }
    if page.unknown_count > 0 {
        limitations.push(format!(
            "{} record(s) on this page could not be decided and are not matched (see data.unknown_records)",
            page.unknown_count
        ));
    }
    let deleted = page
        .matched
        .iter()
        .filter(|v| {
            lo.record(v.plugin, v.record)
                .is_some_and(|l| l.flags & RECORD_FLAG_DELETED != 0)
        })
        .count();
    if deleted > 0 {
        limitations.push(format!(
            "{deleted} result(s) on this page are deleted versions (record flag 0x20, \
             `deleted: true`): `where` never evaluates their payload, only index paths \
             (@form, @plugin, @edid, @flags)"
        ));
    }
    // M3.D3: `where` paths that read `reference`-confidence members.
    let mut reference: Vec<String> = Vec::new();
    if let (Some(q), Some(sc)) = (query.as_ref(), schema) {
        for (sub, members) in q.field_paths() {
            reference.extend(reference_members(sc, signature, sub, Some(members)));
        }
    }
    reference.sort();
    reference.dedup();
    let type_name = String::from_utf8_lossy(&signature).into_owned();
    if let Some(l) = reference_limitation(&[(type_name, reference)]) {
        limitations.push(l);
    }
    limitations.extend(page.reasons.iter().take(20).cloned());
    notes.sort();
    notes.dedup();
    limitations.extend(notes.into_iter().take(20));
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let type_upper = String::from_utf8_lossy(&signature).into_owned();
    let summary =
        format!(
        "{type_upper}{}: {} matched, {} unknown, {} scanned of {} ({} scope){} in {elapsed_ms} ms",
        where_text.map_or_else(String::new, |w| format!(" where {w}")),
        page.matched.len(),
        page.unknown_count,
        page.scanned,
        page.candidates,
        scope.as_str(),
        if page.next_cursor.is_some() { ", more pages" } else { "" },
    );
    let data = json!({
        "type": type_upper,
        "where": where_text,
        "scope": scope.as_str(),
        "limit": limit.clamp(1, kitcraft_doc::search::MAX_LIMIT),
        "matched": page.matched.len(),
        "unknown": page.unknown_count,
        "scanned": page.scanned,
        "candidates": page.candidates,
        "next_cursor": page.next_cursor,
        "results": results,
        "unknown_records": unknown_records,
        "elapsed_ms": elapsed_ms,
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

/// One matched version: identity, EditorID, FULL (decoded when possible), evidence.
fn result_json(
    lo: &kitcraft_doc::LoadOrder,
    strings: &mut kitcraft_doc::strings::StringTables,
    reader: &mut RecordReader,
    v: &Version,
    evidence: &mut Vec<Evidence>,
    notes: &mut Vec<String>,
) -> Value {
    let form = v.form();
    let plugin = lo.plugin(v.plugin);
    let name = plugin.map_or_else(String::new, |p| p.name.clone());
    let loc = lo.record(v.plugin, v.record).copied();
    let edid = loc.and_then(|l| lo.edid(v.plugin, &l)).map(str::to_string);
    let label = loc.map(|l| format!("{}:{:08X}", l.signature(), l.form_id));
    if let Some(l) = &loc {
        evidence.push(Evidence {
            file: name.clone(),
            offset: l.offset,
            length: HEADER_LEN as u64,
            record: label.clone(),
            subrecord: None,
        });
    }
    let localized = plugin
        .and_then(|p| p.header())
        .is_some_and(|h| h.flags.localized());
    let mut full = Value::Null;
    if let Some(l) = &loc {
        if let Ok(rec) = reader.read(lo, v.plugin, l) {
            let spans = kitcraft_doc::compare::spans(&rec);
            if let Some((s, span)) = rec
                .subs
                .iter()
                .zip(spans)
                .find(|(s, _)| s.signature().0 == *b"FULL")
            {
                full = lstring_json(
                    strings,
                    (&name, label.as_deref().unwrap_or_default(), "FULL"),
                    s.data(),
                    localized,
                    evidence,
                    notes,
                );
                evidence.push(Evidence {
                    file: name.clone(),
                    offset: span
                        .file_offset
                        .unwrap_or_else(|| l.offset.saturating_add(HEADER_LEN as u64)),
                    length: if span.file_offset.is_some() {
                        span.length
                    } else {
                        u64::from(l.data_size)
                    },
                    record: label.clone(),
                    subrecord: span.file_offset.map(|_| "FULL".to_string()),
                });
            }
        }
    }
    json!({
        "form": lo.form_label(form),
        "signature": loc.map(|l| l.signature().to_string()),
        "editor_id": edid,
        "full": full,
        "plugin": name,
        "load_index": v.plugin,
        "raw_form_id": loc.map(|l| format!("{:08X}", l.form_id)),
        "offset": loc.map(|l| l.offset),
        "runtime_id": lo.runtime_id(form).map(|r| format!("{r:08X}")),
        "winner": lo.forms.winner(form) == Some(v),
        "deleted": loc.is_some_and(|l| l.flags & RECORD_FLAG_DELETED != 0),
        "resolved_by": v.basis.as_str(),
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]
mod tests {
    use crate::execute;
    use crate::handlers::load::tests::{fixture, plugins_args};
    use serde_json::json;

    #[test]
    fn query_through_execute() {
        let dir = fixture("query");
        let mut args = plugins_args(&dir);
        args["type"] = json!("WEAP");
        args["where"] = json!(r#"FULL ~ "Marksman""#);
        let r = execute("query", args.clone()).unwrap();
        let data = &r["data"];
        assert_eq!(data["matched"], 1);
        assert_eq!(data["scanned"], 2);
        assert_eq!(data["unknown"], 0);
        assert_eq!(data["results"][0]["form"], "Base.esm:000800");
        assert_eq!(data["results"][0]["plugin"], "Patch.esm");
        assert_eq!(
            data["results"][0]["full"]["text"],
            "Old Soviet Marksman Rifle"
        );
        assert_eq!(data["results"][0]["editor_id"], "OldEarthHuntingRifle");
        let ev = r["evidence"].as_array().unwrap();
        assert!(ev
            .iter()
            .any(|e| e["subrecord"] == "FULL" && e["file"] == "Patch.esm"));

        // Scope all sees the master's version too.
        args["where"] = json!(r#"EDID == "OldEarthHuntingRifle""#);
        args["scope"] = json!("all");
        let r = execute("query", args.clone()).unwrap();
        assert_eq!(r["data"]["matched"], 2);
        assert_eq!(r["data"]["results"][0]["winner"], false);

        // Paging: limit 1 then the cursor.
        args["limit"] = json!(1);
        let first = execute("query", args.clone()).unwrap();
        let cursor = first["data"]["next_cursor"].as_str().unwrap().to_string();
        args["cursor"] = json!(cursor);
        let second = execute("query", args.clone()).unwrap();
        assert_eq!(second["data"]["results"][0]["plugin"], "Patch.esm");
        // One candidate is left after the second match: a last, empty page ends it.
        args["cursor"] = second["data"]["next_cursor"].clone();
        let third = execute("query", args.clone()).unwrap();
        assert_eq!(third["data"]["matched"], 0);
        assert!(third["data"]["next_cursor"].is_null());

        // Errors: bad where (column), bad scope, foreign cursor.
        let mut bad = plugins_args(&dir);
        bad["type"] = json!("WEAP");
        bad["where"] = json!("FULL = 1");
        let err = execute("query", bad.clone()).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("column 6"), "{err}");
        bad["where"] = json!("FULL ~ \"x\"");
        bad["cursor"] = json!(first["data"]["next_cursor"].clone());
        assert_eq!(execute("query", bad.clone()).unwrap_err().kind(), "failed");
        bad["scope"] = json!("some");
        assert_eq!(execute("query", bad).unwrap_err().kind(), "invalid_args");
    }

    /// The builtin definitions with GLOB's `FLTV.Value` from the pinned xEdit file at
    /// `reference` confidence (no builtin member is `reference` yet).
    fn schema_with_reference_member() -> &'static kitcraft_schema::Schema {
        let owned: Vec<(String, String)> = kitcraft_schema::BUILTIN_DEFINITIONS
            .iter()
            .map(|(file, text)| {
                if *file != "GLOB.toml" {
                    return (file.to_string(), text.to_string());
                }
                let cut = text.find("[[member]]\nsig = \"FLTV\"").unwrap();
                let glob = format!(
                    "{}[[member]]\nsig = \"FLTV\"\nname = \"Value\"\ntype = \"f32\"\n\
                     source = \"xedit: wbDefinitionsSF1.pas@{}#L1-L2\"\nconfidence = \"reference\"\n",
                    &text[..cut],
                    kitcraft_schema::XEDIT_REVISION
                );
                (file.to_string(), glob)
            })
            .collect();
        let schema =
            kitcraft_schema::load_sources(owned.iter().map(|(f, t)| (f.as_str(), t.as_str())))
                .unwrap();
        Box::leak(Box::new(schema))
    }

    /// M3.D3: values read through a `reference`-confidence member are named in
    /// `limitations` by `inspect` and `query`; other paths are not.
    #[test]
    fn reference_members_surface_as_limitations() {
        use crate::handlers::load::tests::plugin;
        use crate::handlers::load::SchemaOverride;
        use crate::handlers::testdata::sub;
        let dir =
            std::env::temp_dir().join(format!("kitcraft-engine-reference-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let glob = [
            sub(b"EDID", b"Speed\0"),
            sub(b"FLTV", &2.5f32.to_le_bytes()),
        ]
        .concat();
        std::fs::write(
            dir.join("G.esm"),
            plugin(0x1, &[], &[(b"GLOB", 0x800, glob)]),
        )
        .unwrap();
        let load =
            json!({"plugins": ["G.esm"], "data_dir": dir.display().to_string(), "no_cache": true});
        let mentions = |r: &serde_json::Value| {
            r["limitations"].as_array().unwrap().iter().any(|l| {
                l.as_str().unwrap().contains("`reference` confidence")
                    && l.as_str().unwrap().contains("GLOB FLTV.Value")
            })
        };
        let engine = crate::Engine::with_default_handlers();
        engine.with_session(|s| s.insert(SchemaOverride(schema_with_reference_member())));
        engine.execute("load", load.clone()).unwrap();
        let r = engine.execute("inspect", json!({"form": "Speed"})).unwrap();
        assert_eq!(r["data"]["versions"][0]["subrecords"][1]["value"], 2.5);
        assert!(mentions(&r), "{}", r["limitations"]);
        let r = engine
            .execute("query", json!({"type": "GLOB", "where": "FLTV.Value > 1"}))
            .unwrap();
        assert_eq!(r["data"]["matched"], 1);
        assert!(mentions(&r), "{}", r["limitations"]);
        let r = engine
            .execute(
                "query",
                json!({"type": "GLOB", "where": "EDID == \"Speed\""}),
            )
            .unwrap();
        assert!(!mentions(&r), "{}", r["limitations"]);
        // The builtin definitions have no `reference` member: nothing to say.
        let mut args = load;
        args["form"] = json!("Speed");
        assert!(!mentions(&execute("inspect", args).unwrap()));
    }
}
