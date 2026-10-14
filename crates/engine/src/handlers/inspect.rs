//! `inspect` (minimal): one record of one plugin, found by FormID or EditorID, with its
//! header fields and its subrecords as opaque hex. Typed fields arrive with schema wiring;
//! cross-plugin resolution (winning override) arrives with load order (M2).
//!
//! Lookup: when `form` is 1-8 hex digits (optionally `0x`-prefixed), the first record whose
//! header FormID equals it wins, and compressed records are not decompressed for the scan.
//! Otherwise, or when no FormID matches (a second pass), the first record whose `EDID`
//! equals `form` (ASCII case-insensitive) is returned; that pass decompresses compressed
//! records to read their `EDID`. The file is streamed one top-level group at a time and
//! each pass stops at its first match.

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use kitcraft_esm::driver::visit_file;
use kitcraft_esm::{
    DecodeLimits, Entry, Error, ParseOptions, Record, Signature, Subrecord, HEADER_LEN,
};
use serde_json::{json, Value};

use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

const ID: &str = "inspect";

/// Payload bytes shown per subrecord.
pub(crate) const PAYLOAD_CAP: usize = 256;

/// Every subrecord is opaque until the schema crate's typed records are wired in.
pub(crate) const TYPED_PENDING: &str = "typed decode pending schema wiring";

/// No load order yet: the record is this plugin's version only.
pub(crate) const SINGLE_PLUGIN: &str =
    "single plugin: overrides in other plugins are not considered (load order arrives in M2)";

/// Stated for a compressed record.
pub(crate) const COMPRESSED_OFFSETS: &str = "record is compressed: subrecord offsets are \
     relative to its decompressed data; evidence covers the compressed body in the file";

/// Stated when a payload was cut.
pub(crate) const PAYLOAD_CUT: &str =
    "payload_hex shows at most the first 256 bytes of each subrecord (see payload_truncated)";

/// Marks the deliberate early stop of `visit_file` once the record is found.
#[derive(Debug)]
struct Found;

impl std::fmt::Display for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("record found")
    }
}

impl std::error::Error for Found {}

fn stop() -> Error {
    Error::Io {
        offset: 0,
        source: io::Error::other(Found),
    }
}

fn is_stop(e: &Error) -> bool {
    matches!(e, Error::Io { source, .. }
        if source.get_ref().is_some_and(|inner| inner.is::<Found>()))
}

/// One enclosing group, outermost first.
#[derive(Debug, Clone)]
struct GroupInfo {
    offset: Option<u64>,
    group_type: i32,
    label: String,
}

/// A matched record, owned so it outlives the visited top-level buffer.
#[derive(Debug)]
struct Hit {
    record: Record<'static>,
    groups: Vec<GroupInfo>,
    /// Decoded subrecords of a compressed record.
    decoded: Option<Vec<Subrecord<'static>>>,
    editor_id: Option<String>,
}

#[derive(Debug, Default)]
struct Search {
    form_id: Option<u32>,
    editor_id: String,
    /// Compare EDIDs on this pass (costs a decompression per compressed record).
    check_editor_id: bool,
    by_form: Option<Hit>,
    by_editor_id: Option<Hit>,
}

/// `form` as a FormID: 1-8 hex digits, optional `0x`.
pub(crate) fn parse_form_id(form: &str) -> Option<u32> {
    let digits = form
        .strip_prefix("0x")
        .or_else(|| form.strip_prefix("0X"))
        .unwrap_or(form);
    if digits.is_empty() || digits.len() > 8 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

/// The handler.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let form = str_arg(ID, args, "form")?;
    let plugin = str_arg(ID, args, "plugin")?;
    let path = Path::new(plugin);
    let limits = DecodeLimits::default();
    let form_id = parse_form_id(form);
    let mut search = Search {
        form_id,
        editor_id: form.to_string(),
        check_editor_id: form_id.is_none(),
        ..Search::default()
    };
    scan(path, plugin, &mut search, &limits)?;
    if form_id.is_some() && search.by_form.is_none() {
        // A hex-looking query that is no FormID here may still be an EditorID.
        search.form_id = None;
        search.check_editor_id = true;
        scan(path, plugin, &mut search, &limits)?;
    }
    let (hit, matched_by) = match (search.by_form, search.by_editor_id) {
        (Some(hit), _) => (hit, "form_id"),
        (None, Some(hit)) => (hit, "editor_id"),
        (None, None) => {
            return Err(failed(
                ID,
                format!("{plugin}: no record with FormID or EditorID `{form}`"),
            ))
        }
    };
    Ok(outcome(plugin, &file_name(path), form, matched_by, &hit))
}

/// One pass over the file; stops early at the decisive match.
fn scan(
    path: &Path,
    plugin: &str,
    search: &mut Search,
    limits: &DecodeLimits,
) -> Result<(), EngineError> {
    let mut groups = Vec::new();
    let visited = visit_file(path, &ParseOptions::default(), |entry| {
        if walk(entry, &mut groups, search, limits)? {
            Err(stop())
        } else {
            Ok(())
        }
    });
    match visited {
        Ok(()) => Ok(()),
        Err(e) if is_stop(&e) => Ok(()),
        Err(e) => Err(failed(ID, format!("{plugin}: {e}"))),
    }
}

/// Depth-first search of one entry. `Ok(true)` means stop: the decisive match is found.
fn walk(
    entry: &Entry<'_>,
    groups: &mut Vec<GroupInfo>,
    search: &mut Search,
    limits: &DecodeLimits,
) -> Result<bool, Error> {
    match entry {
        Entry::Group(g) => {
            let h = &g.header;
            let label = if h.group_type == 0 {
                h.label_signature().to_string()
            } else {
                format!("{:08X}", h.label_u32())
            };
            groups.push(GroupInfo {
                offset: g.offset(),
                group_type: h.group_type,
                label,
            });
            for child in g.children() {
                if walk(child, groups, search, limits)? {
                    return Ok(true);
                }
            }
            groups.pop();
            Ok(false)
        }
        Entry::Record(r) => {
            let form_match = search.form_id == Some(r.header.form_id);
            let need_editor_id = search.check_editor_id && search.by_editor_id.is_none();
            if !form_match && !need_editor_id {
                return Ok(false);
            }
            let decoded = r.decode(limits)?;
            let subs: &[Subrecord<'_>] = match (&decoded, r.subrecords()) {
                (Some(d), _) => d,
                (None, Some(s)) => s,
                (None, None) => &[],
            };
            let editor_id = editor_id(subs);
            let hit = || Hit {
                record: r.clone().into_owned(),
                groups: groups.clone(),
                decoded: decoded.clone(),
                editor_id: editor_id.clone(),
            };
            if form_match {
                search.by_form = Some(hit());
                return Ok(true);
            }
            if editor_id
                .as_deref()
                .is_some_and(|id| id.eq_ignore_ascii_case(&search.editor_id))
            {
                search.by_editor_id = Some(hit());
                return Ok(true);
            }
            Ok(false)
        }
    }
}

/// The first `EDID`, up to its terminating NUL, if it is valid UTF-8.
fn editor_id(subs: &[Subrecord<'_>]) -> Option<String> {
    let edid = subs
        .iter()
        .find(|s| s.signature() == Signature::new(b"EDID"))?;
    let data = edid.data();
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    std::str::from_utf8(data.get(..end)?)
        .ok()
        .map(str::to_string)
}

fn hex(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().saturating_mul(2));
    for b in data {
        let _ = write!(out, "{b:02X}");
    }
    out
}

fn outcome(plugin: &str, file: &str, form: &str, matched_by: &str, hit: &Hit) -> Outcome {
    let r = &hit.record;
    let h = &r.header;
    let record_id = format!("{}:{:08X}", h.signature, h.form_id);
    let record_offset = r.offset().unwrap_or(0);
    let body_offset = record_offset.saturating_add(HEADER_LEN as u64);
    let compressed = hit.decoded.is_some();

    let mut limitations = vec![TYPED_PENDING.to_string(), SINGLE_PLUGIN.to_string()];
    let mut evidence = vec![Evidence {
        file: file.to_string(),
        offset: record_offset,
        length: HEADER_LEN as u64,
        record: Some(record_id.clone()),
        subrecord: None,
    }];

    let subs: &[Subrecord<'_>] = match (&hit.decoded, r.subrecords()) {
        (Some(d), _) => d,
        (None, Some(s)) => s,
        (None, None) => &[],
    };
    let mut at = if compressed { 0 } else { body_offset };
    let mut any_cut = false;
    let mut list = Vec::with_capacity(subs.len());
    for s in subs {
        let data = s.data();
        let size = data.len() as u64;
        let data_offset = at.saturating_add(s.encoded_len().saturating_sub(size));
        let shown = data.get(..PAYLOAD_CAP).unwrap_or(data);
        let truncated = shown.len() < data.len();
        any_cut |= truncated;
        list.push(json!({
            "signature": s.signature().to_string(),
            "size": size,
            "oversized": s.has_oversize_prefix(),
            "offset": at,
            "data_offset": data_offset,
            "payload_hex": hex(shown),
            "payload_truncated": truncated,
        }));
        if !compressed {
            evidence.push(Evidence {
                file: file.to_string(),
                offset: data_offset,
                length: size,
                record: Some(record_id.clone()),
                subrecord: Some(s.signature().to_string()),
            });
        }
        at = at.saturating_add(s.encoded_len());
    }
    if compressed {
        limitations.push(COMPRESSED_OFFSETS.to_string());
        evidence.push(Evidence {
            file: file.to_string(),
            offset: body_offset,
            length: r.data_size_as_read().map_or(0, u64::from),
            record: Some(record_id),
            subrecord: None,
        });
    }
    if any_cut {
        limitations.push(PAYLOAD_CUT.to_string());
    }

    let groups: Vec<Value> = hit
        .groups
        .iter()
        .map(|g| json!({"offset": g.offset, "type": g.group_type, "label": g.label}))
        .collect();
    let data = json!({
        "plugin": plugin,
        "form": form,
        "matched_by": matched_by,
        "record": {
            "signature": h.signature.to_string(),
            "form_id": format!("{:08X}", h.form_id),
            "editor_id": hit.editor_id,
            "flags": format!("0x{:08X}", h.flags),
            "compressed": compressed,
            "offset": record_offset,
            "data_size": r.data_size_as_read(),
            "timestamp": h.timestamp,
            "version_control": h.version_control,
            "form_version": h.form_version,
            "unknown": h.unknown,
            "groups": groups,
        },
        "subrecord_offsets": if compressed { "decompressed" } else { "file" },
        "subrecords": list,
    });
    Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: Some(0),
            opaque_subrecords: Some(subs.len() as u64),
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{parse_form_id, COMPRESSED_OFFSETS, PAYLOAD_CUT, SINGLE_PLUGIN, TYPED_PENDING};
    use crate::execute;
    use crate::handlers::testdata;
    use serde_json::json;

    #[test]
    fn form_ids_parse_as_hex() {
        assert_eq!(parse_form_id("0001A2B3"), Some(0x0001_A2B3));
        assert_eq!(parse_form_id("0x1a2b3"), Some(0x0001_A2B3));
        assert_eq!(parse_form_id("100"), Some(0x100));
        assert_eq!(parse_form_id("TestRifle"), None);
        assert_eq!(parse_form_id("123456789"), None);
        assert_eq!(parse_form_id(""), None);
        assert_eq!(parse_form_id("0x"), None);
    }

    #[test]
    fn inspect_by_form_id_lists_header_and_subrecords() {
        let bytes = testdata::plugin();
        let path = testdata::write_temp("inspect-form", &bytes);
        let plugin = path.to_str().unwrap();
        let file = path.file_name().unwrap().to_str().unwrap();
        let result = execute("inspect", json!({"form": "0001A2B3", "plugin": plugin})).unwrap();
        let data = &result["data"];
        assert_eq!(data["matched_by"], "form_id");
        let record = &data["record"];
        assert_eq!(record["signature"], "WEAP");
        assert_eq!(record["form_id"], "0001A2B3");
        assert_eq!(record["editor_id"], "TestRifle");
        assert_eq!(record["flags"], "0x00000000");
        assert_eq!(record["compressed"], false);
        assert_eq!(record["timestamp"], 0x1234);
        assert_eq!(record["version_control"], 0x5678);
        assert_eq!(record["form_version"], 581);
        assert_eq!(record["unknown"], 7);
        let weap_group = record["groups"][0].clone();
        assert_eq!(weap_group["label"], "WEAP");
        assert_eq!(weap_group["type"], 0);

        // Offsets are absolute and point at real bytes.
        let rec_off = record["offset"].as_u64().unwrap() as usize;
        assert_eq!(&bytes[rec_off..rec_off + 4], b"WEAP");
        assert_eq!(data["subrecord_offsets"], "file");
        let subs = data["subrecords"].as_array().unwrap();
        let sigs: Vec<&str> = subs
            .iter()
            .map(|s| s["signature"].as_str().unwrap())
            .collect();
        assert_eq!(sigs, ["EDID", "FULL", "MODL", "DNAM"]);
        assert_eq!(subs[0]["offset"].as_u64().unwrap() as usize, rec_off + 24);
        for s in subs {
            let at = s["offset"].as_u64().unwrap() as usize;
            let data_at = s["data_offset"].as_u64().unwrap() as usize;
            let size = s["size"].as_u64().unwrap() as usize;
            let sig = s["signature"].as_str().unwrap().as_bytes();
            if s["oversized"] == true {
                assert_eq!(&bytes[at..at + 4], b"XXXX");
                assert_eq!(&bytes[at + 10..at + 14], sig);
            } else {
                assert_eq!(&bytes[at..at + 4], sig);
            }
            let shown = hex::decode(s["payload_hex"].as_str().unwrap());
            assert_eq!(&bytes[data_at..data_at + shown.len()], &shown[..]);
            assert_eq!(s["payload_truncated"], shown.len() < size);
        }
        assert_eq!(subs[1]["payload_hex"], "5269666C6500");
        assert_eq!(subs[2]["oversized"], true);
        assert_eq!(subs[3]["size"], 300);
        assert_eq!(subs[3]["payload_hex"].as_str().unwrap().len(), 512);
        assert_eq!(subs[3]["payload_truncated"], true);

        assert_eq!(
            result["limitations"],
            json!([TYPED_PENDING, SINGLE_PLUGIN, PAYLOAD_CUT])
        );
        assert_eq!(result["coverage"]["status"], "partial");
        assert_eq!(result["coverage"]["opaque_subrecords"], 4);
        assert_eq!(result["coverage"]["decoded_subrecords"], 0);
        let evidence = result["evidence"].as_array().unwrap();
        assert_eq!(evidence.len(), 1 + 4);
        assert_eq!(
            evidence[0],
            json!({"file": file, "offset": rec_off, "length": 24,
                   "record": "WEAP:0001A2B3", "subrecord": null})
        );
        assert_eq!(evidence[4]["subrecord"], "DNAM");
        assert_eq!(evidence[4]["length"], 300);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn inspect_by_editor_id_decodes_compressed_records() {
        let path = testdata::write_temp("inspect-edid", &testdata::plugin());
        let plugin = path.to_str().unwrap();
        let result = execute("inspect", json!({"form": "packedgun", "plugin": plugin})).unwrap();
        let data = &result["data"];
        assert_eq!(data["matched_by"], "editor_id");
        assert_eq!(data["record"]["form_id"], "0001A2B4");
        assert_eq!(data["record"]["compressed"], true);
        assert_eq!(data["record"]["flags"], "0x00040000");
        assert_eq!(data["subrecord_offsets"], "decompressed");
        let expected = testdata::packed_fields();
        let subs = data["subrecords"].as_array().unwrap();
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0]["offset"], 0);
        assert_eq!(subs[1]["offset"], 16);
        assert_eq!(subs[1]["data_offset"], 22);
        assert_eq!(subs[1]["payload_hex"], "01020304");
        assert_eq!(&expected[22..26], &[1, 2, 3, 4]);
        assert_eq!(
            result["limitations"],
            json!([TYPED_PENDING, SINGLE_PLUGIN, COMPRESSED_OFFSETS])
        );
        assert_eq!(result["evidence"].as_array().unwrap().len(), 2);
        assert_eq!(result["evidence"][1]["subrecord"], serde_json::Value::Null);

        // EditorID lookup also works for an uncompressed record, and a hex-looking query
        // falls back to EditorID when no FormID matches.
        let result = execute("inspect", json!({"form": "fTestValue", "plugin": plugin})).unwrap();
        assert_eq!(result["data"]["record"]["signature"], "GMST");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_record_and_bad_plugin_are_failures() {
        let path = testdata::write_temp("inspect-missing", &testdata::plugin());
        let plugin = path.to_str().unwrap();
        let err = execute("inspect", json!({"form": "DEADBEEF", "plugin": plugin})).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("DEADBEEF"), "{err}");
        std::fs::remove_file(&path).unwrap();

        let mut bytes = testdata::plugin();
        bytes.truncate(40);
        let path = testdata::write_temp("inspect-truncated", &bytes);
        let err = execute(
            "inspect",
            json!({"form": "TestRifle", "plugin": path.to_str().unwrap()}),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("offset"), "{err}");
        std::fs::remove_file(&path).unwrap();

        let err = execute("inspect", json!({"form": "TestRifle"})).unwrap_err();
        assert_eq!(err.kind(), "invalid_args");
    }

    /// Minimal hex decoding for the assertions above.
    mod hex {
        pub fn decode(text: &str) -> Vec<u8> {
            (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                .collect()
        }
    }
}
