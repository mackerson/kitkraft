//! `coverage` (M1.14): which record types and subrecords a plugin holds, against what the
//! schema (`kitcraft_schema::builtin()`) defines.
//!
//! The plugin is streamed one top-level entry at a time (`kitcraft_esm::driver::visit_file`).
//! Every subrecord occurrence is classified by its record type and signature:
//!
//! - `typed`: the record type's definition names the signature with a layout (any type
//!   other than `opaque`);
//! - `opaque`: the definition names it but keeps the payload as raw bytes;
//! - `unknown`: the record type has no definition, or its definition does not name the
//!   signature (subrecords inside an `any_until` run, such as a BFCB..BFCE body, count
//!   here: they are kept as raw subrecords).
//!
//! Classification is by signature only; payloads are not decoded against the layouts.
//! Compressed records are decompressed (bounded by the default `DecodeLimits`) unless
//! `decode` is `false`; a record that fails to decompress is listed in
//! `coverage.not_examined`, not fatal.

use std::collections::BTreeMap;
use std::path::Path;

use kitcraft_esm::driver::visit_file;
use kitcraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature, Subrecord, HEADER_LEN};
use kitcraft_schema::{FieldType, Schema};
use serde_json::{json, Value};

use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

const ID: &str = "coverage";

/// What the classification means, stated on every result.
pub(crate) const BY_SIGNATURE: &str = "subrecords are classified by signature against the \
     schema definitions (typed: defined with a layout; opaque: defined as raw bytes; unknown: \
     not named by a definition); payloads are not decoded or checked against the layouts, \
     because generated records are not wired into the reader yet";

/// Stated when compressed records were skipped.
pub(crate) const COMPRESSED_SKIPPED: &str = "decode is off: compressed records are counted \
     but their subrecords are not examined";

/// Decode failures listed one by one before the rest are summarized.
const MAX_LISTED_FAILURES: usize = 20;

/// How a subrecord signature relates to its record type's definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Class {
    Typed,
    Opaque,
    Unknown,
}

impl Class {
    fn as_str(self) -> &'static str {
        match self {
            Self::Typed => "typed",
            Self::Opaque => "opaque",
            Self::Unknown => "unknown",
        }
    }
}

/// The schema's view of one record type: its definition name and, per known subrecord
/// signature, the class and the member paths that use it.
#[derive(Debug, Default)]
struct TypeSchema {
    definition: Option<String>,
    known: BTreeMap<[u8; 4], (Class, Vec<String>)>,
}

impl TypeSchema {
    fn of(schema: Option<&Schema>, signature: Signature) -> TypeSchema {
        let Some(schema) = schema else {
            return TypeSchema::default();
        };
        let Some(sig) = kitcraft_schema::Signature::from_bytes(signature.bytes()) else {
            return TypeSchema::default();
        };
        let Some(record) = schema.record(sig) else {
            return TypeSchema::default();
        };
        let mut known: BTreeMap<[u8; 4], (Class, Vec<String>)> = BTreeMap::new();
        for (path, sub) in record.leaf_subrecords() {
            let class = if matches!(sub.ty, FieldType::Opaque { .. }) {
                Class::Opaque
            } else {
                Class::Typed
            };
            let entry = known
                .entry(sub.signature.as_bytes())
                .or_insert((class, Vec::new()));
            entry.0 = entry.0.min(class);
            entry.1.push(path);
        }
        // `any_until` terminators are known even when no member defines them.
        for sig in schema.known_signatures(sig).unwrap_or_default() {
            known
                .entry(sig.as_bytes())
                .or_insert((Class::Opaque, Vec::new()));
        }
        TypeSchema {
            definition: Some(record.name.clone()),
            known,
        }
    }

    fn class(&self, signature: [u8; 4]) -> Class {
        self.known.get(&signature).map_or(Class::Unknown, |k| k.0)
    }
}

#[derive(Debug)]
struct SubStats {
    count: u64,
    /// FormID of the first record holding this signature.
    first_form: u32,
}

#[derive(Debug)]
struct TypeStats {
    schema: TypeSchema,
    records: u64,
    compressed: u64,
    compressed_examined: u64,
    /// Offset, length and FormID of the first record of this type.
    first: (u64, u64, u32),
    subs: BTreeMap<[u8; 4], SubStats>,
}

#[derive(Debug, Default)]
struct Walk {
    types: BTreeMap<[u8; 4], TypeStats>,
    groups: u64,
    records: u64,
    compressed: u64,
    compressed_examined: u64,
    decode_failures: Vec<String>,
}

impl Walk {
    fn entry(&mut self, entry: &Entry<'_>, schema: Option<&Schema>, decode: Option<&DecodeLimits>) {
        match entry {
            Entry::Group(g) => {
                self.groups = self.groups.saturating_add(1);
                for child in g.children() {
                    self.entry(child, schema, decode);
                }
            }
            Entry::Record(r) => self.record(r, schema, decode),
        }
    }

    fn record(&mut self, r: &Record<'_>, schema: Option<&Schema>, decode: Option<&DecodeLimits>) {
        let h = &r.header;
        self.records = self.records.saturating_add(1);
        let offset = r.offset().unwrap_or(0);
        let length = r
            .data_size_as_read()
            .map_or(0, u64::from)
            .saturating_add(HEADER_LEN as u64);
        let stats = self
            .types
            .entry(h.signature.bytes())
            .or_insert_with(|| TypeStats {
                schema: TypeSchema::of(schema, h.signature),
                records: 0,
                compressed: 0,
                compressed_examined: 0,
                first: (offset, length, h.form_id),
                subs: BTreeMap::new(),
            });
        stats.records = stats.records.saturating_add(1);

        let decoded;
        let subs: &[Subrecord<'_>] = match r.subrecords() {
            Some(subs) => subs,
            None => {
                self.compressed = self.compressed.saturating_add(1);
                stats.compressed = stats.compressed.saturating_add(1);
                let Some(limits) = decode else {
                    return;
                };
                match r.decode(limits) {
                    Ok(Some(subs)) => {
                        self.compressed_examined = self.compressed_examined.saturating_add(1);
                        stats.compressed_examined = stats.compressed_examined.saturating_add(1);
                        decoded = subs;
                        &decoded
                    }
                    Ok(None) => &[],
                    Err(e) => {
                        self.decode_failures.push(format!(
                            "{} {:08X} @{offset}: compressed body not examined ({e})",
                            h.signature, h.form_id
                        ));
                        return;
                    }
                }
            }
        };
        for s in subs {
            let sub = stats.subs.entry(s.signature().bytes()).or_insert(SubStats {
                count: 0,
                first_form: h.form_id,
            });
            sub.count = sub.count.saturating_add(1);
        }
    }
}

/// Occurrence or distinct-signature counts per class.
#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    typed: u64,
    opaque: u64,
    unknown: u64,
}

impl Tally {
    fn add(&mut self, class: Class, n: u64) {
        let slot = match class {
            Class::Typed => &mut self.typed,
            Class::Opaque => &mut self.opaque,
            Class::Unknown => &mut self.unknown,
        };
        *slot = slot.saturating_add(n);
    }

    fn total(self) -> u64 {
        self.typed
            .saturating_add(self.opaque)
            .saturating_add(self.unknown)
    }

    fn json(self) -> Value {
        json!({"typed": self.typed, "opaque": self.opaque, "unknown": self.unknown})
    }
}

/// The handler.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let plugin = str_arg(ID, args, "plugin")?;
    let decode = args.get("decode").and_then(Value::as_bool).unwrap_or(true);
    let path = Path::new(plugin);
    let schema = kitcraft_schema::builtin()
        .map_err(|e| failed(ID, format!("the builtin schema does not load: {e}")))?;
    let bytes = std::fs::metadata(path)
        .map_err(|e| failed(ID, format!("{plugin}: {e}")))?
        .len();
    let limits = DecodeLimits::default();
    let mut walk = Walk::default();
    visit_file(path, &ParseOptions::default(), |entry| {
        walk.entry(entry, Some(schema), decode.then_some(&limits));
        Ok(())
    })
    .map_err(|e| failed(ID, format!("{plugin}: {e}")))?;
    Ok(outcome(plugin, &file_name(path), bytes, decode, &walk))
}

fn outcome(plugin: &str, file: &str, bytes: u64, decode: bool, walk: &Walk) -> Outcome {
    let mut types: Vec<(&[u8; 4], &TypeStats)> = walk.types.iter().collect();
    types.sort_by(|a, b| b.1.records.cmp(&a.1.records).then(a.0.cmp(b.0)));

    let mut occurrences = Tally::default();
    let mut list = Vec::with_capacity(types.len());
    let mut evidence = vec![Evidence {
        file: file.to_string(),
        offset: 0,
        length: bytes,
        record: None,
        subrecord: None,
    }];
    let mut defined = 0u64;
    for (sig, stats) in &types {
        let signature = Signature(**sig).to_string();
        let mut distinct = Tally::default();
        let mut type_occurrences = Tally::default();
        let mut subs = Vec::with_capacity(stats.subs.len());
        for (sub_sig, sub) in &stats.subs {
            let class = stats.schema.class(*sub_sig);
            distinct.add(class, 1);
            type_occurrences.add(class, sub.count);
            let members = stats.schema.known.get(sub_sig).map(|k| k.1.clone());
            subs.push(json!({
                "signature": Signature(*sub_sig).to_string(),
                "count": sub.count,
                "class": class.as_str(),
                "members": members.unwrap_or_default(),
                "first_record": format!("{signature}:{:08X}", sub.first_form),
            }));
        }
        occurrences.add(Class::Typed, type_occurrences.typed);
        occurrences.add(Class::Opaque, type_occurrences.opaque);
        occurrences.add(Class::Unknown, type_occurrences.unknown);
        if stats.schema.definition.is_some() {
            defined = defined.saturating_add(1);
        }
        let (offset, length, form) = stats.first;
        evidence.push(Evidence {
            file: file.to_string(),
            offset,
            length,
            record: Some(format!("{signature}:{form:08X}")),
            subrecord: None,
        });
        list.push(json!({
            "signature": signature,
            "definition": stats.schema.definition,
            "records": stats.records,
            "compressed": stats.compressed,
            "compressed_examined": stats.compressed_examined,
            "subrecords": type_occurrences.total(),
            "occurrences": type_occurrences.json(),
            "distinct": {
                "seen": distinct.total(),
                "typed": distinct.typed,
                "opaque": distinct.opaque,
                "unknown": distinct.unknown,
            },
            "first_offset": offset,
            "signatures": subs,
        }));
    }

    let mut limitations = vec![BY_SIGNATURE.to_string()];
    let mut not_examined = Vec::new();
    let skipped = walk.compressed.saturating_sub(walk.compressed_examined);
    if !decode && walk.compressed > 0 {
        limitations.push(COMPRESSED_SKIPPED.to_string());
        not_examined.push(format!(
            "{} compressed records (decode off)",
            walk.compressed
        ));
    }
    for failure in walk.decode_failures.iter().take(MAX_LISTED_FAILURES) {
        not_examined.push(failure.clone());
    }
    if let Some(rest) = walk.decode_failures.len().checked_sub(MAX_LISTED_FAILURES) {
        if rest > 0 {
            not_examined.push(format!("{rest} more compressed records failed to decode"));
        }
    }
    let complete = occurrences.opaque == 0 && occurrences.unknown == 0 && skipped == 0;
    let summary = format!(
        "{file}: {} records of {} types ({defined} defined in the schema); {} subrecords: \
         {} typed, {} opaque, {} unknown; {} compressed records, {} examined",
        walk.records,
        types.len(),
        occurrences.total(),
        occurrences.typed,
        occurrences.opaque,
        occurrences.unknown,
        walk.compressed,
        walk.compressed_examined,
    );
    let data = json!({
        "plugin": plugin,
        "file": file,
        "bytes": bytes,
        "decode": decode,
        "records": walk.records,
        "groups": walk.groups,
        "compressed_records": walk.compressed,
        "compressed_examined": walk.compressed_examined,
        "record_types_seen": types.len(),
        "record_types_defined": defined,
        "subrecords": occurrences.total(),
        "occurrences": occurrences.json(),
        "record_types": list,
        "summary": summary,
    });
    Outcome {
        data,
        coverage: Coverage {
            status: if complete {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: Some(occurrences.typed),
            opaque_subrecords: Some(occurrences.opaque.saturating_add(occurrences.unknown)),
            not_examined,
        },
        limitations,
        evidence,
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{BY_SIGNATURE, COMPRESSED_SKIPPED};
    use crate::handlers::testdata;
    use crate::{execute, CoverageStatus};
    use serde_json::{json, Value};

    fn find<'a>(list: &'a Value, signature: &str) -> &'a Value {
        list.as_array()
            .unwrap()
            .iter()
            .find(|t| t["signature"] == signature)
            .unwrap_or_else(|| panic!("{signature} missing from {list}"))
    }

    #[test]
    fn synthetic_plugin_through_execute() {
        let bytes = testdata::plugin();
        let path = testdata::write_temp("coverage", &bytes);
        let plugin = path.to_str().unwrap();
        let result = execute("coverage", json!({"plugin": plugin})).unwrap();
        assert_eq!(result["command"], "coverage");
        let data = &result["data"];
        assert_eq!(data["records"], testdata::RECORDS);
        assert_eq!(data["groups"], testdata::GROUPS);
        assert_eq!(data["decode"], true);
        assert_eq!(data["compressed_records"], 1);
        assert_eq!(data["compressed_examined"], 1);
        assert_eq!(data["record_types_seen"], 3);
        // TES4, GMST and WEAP all have definitions.
        assert_eq!(data["record_types_defined"], 3);

        // Sorted by record count: WEAP (2) first, then GMST and TES4 by signature.
        let types = &data["record_types"];
        let order: Vec<&str> = types
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["signature"].as_str().unwrap())
            .collect();
        assert_eq!(order, ["WEAP", "GMST", "TES4"]);

        // TES4: HEDR and CNAM have layouts in the definition.
        let tes4 = find(types, "TES4");
        assert_eq!(tes4["definition"], "PluginHeader");
        assert_eq!(
            tes4["distinct"],
            json!({"seen": 2, "typed": 2, "opaque": 0, "unknown": 0})
        );
        let hedr = find(&tes4["signatures"], "HEDR");
        assert_eq!(hedr["class"], "typed");
        assert_eq!(hedr["members"], json!(["Header"]));
        assert_eq!(hedr["first_record"], "TES4:00000000");

        // WEAP: EDID twice (once from the compressed record) and FULL typed (universal
        // subrecords, M2.B2), MODL opaque in the inventory definition; DNAM and DATA are
        // not in it.
        let weap = find(types, "WEAP");
        assert_eq!(weap["definition"], "Weapon");
        assert_eq!(weap["records"], 2);
        assert_eq!(
            (
                weap["compressed"].clone(),
                weap["compressed_examined"].clone()
            ),
            (json!(1), json!(1))
        );
        assert_eq!(find(&weap["signatures"], "EDID")["count"], 2);
        assert_eq!(find(&weap["signatures"], "EDID")["class"], "typed");
        assert_eq!(find(&weap["signatures"], "MODL")["class"], "opaque");
        let dnam = find(&weap["signatures"], "DNAM");
        assert_eq!(dnam["class"], "unknown");
        assert_eq!(dnam["members"], json!([]));
        assert_eq!(
            find(&weap["signatures"], "DATA")["first_record"],
            format!("WEAP:{:08X}", testdata::PACKED)
        );
        assert_eq!(
            weap["distinct"],
            json!({"seen": 5, "typed": 2, "opaque": 1, "unknown": 2})
        );
        assert_eq!(
            weap["occurrences"],
            json!({"typed": 3, "opaque": 1, "unknown": 2})
        );

        // Totals and the envelope agree with the per-type counts.
        assert_eq!(data["subrecords"], 10);
        assert_eq!(
            data["occurrences"],
            json!({"typed": 6, "opaque": 2, "unknown": 2})
        );
        assert_eq!(result["coverage"]["status"], "partial");
        assert_eq!(result["coverage"]["decoded_subrecords"], 6);
        assert_eq!(result["coverage"]["opaque_subrecords"], 4);
        assert_eq!(result["coverage"]["not_examined"], json!([]));
        assert_eq!(result["limitations"], json!([BY_SIGNATURE]));

        // Evidence: the whole file, then the first record of each type.
        let evidence = result["evidence"].as_array().unwrap();
        assert_eq!(evidence.len(), 4);
        assert_eq!(evidence[0]["offset"], 0);
        assert_eq!(evidence[0]["length"], bytes.len());
        assert_eq!(
            evidence[1]["record"],
            format!("WEAP:{:08X}", testdata::RIFLE)
        );
        assert_eq!(evidence[3]["record"], "TES4:00000000");
        assert_eq!(evidence[3]["offset"], 0);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn decode_off_skips_compressed_bodies_and_says_so() {
        let path = testdata::write_temp("coverage-nodecode", &testdata::plugin());
        let result = execute(
            "coverage",
            json!({"plugin": path.to_str().unwrap(), "decode": false}),
        )
        .unwrap();
        let data = &result["data"];
        assert_eq!(data["compressed_records"], 1);
        assert_eq!(data["compressed_examined"], 0);
        let weap = find(&data["record_types"], "WEAP");
        assert_eq!(find(&weap["signatures"], "EDID")["count"], 1);
        assert!(weap["signatures"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["signature"] != "DATA"));
        assert_eq!(
            result["coverage"]["not_examined"],
            json!(["1 compressed records (decode off)"])
        );
        assert_eq!(
            result["limitations"],
            json!([BY_SIGNATURE, COMPRESSED_SKIPPED])
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn corrupt_compressed_body_is_listed_not_fatal() {
        let mut bytes = testdata::plugin();
        // The compressed record is last; its zlib stream ends the file. Damage the stream
        // without changing any size field.
        let n = bytes.len();
        bytes[n - 6] ^= 0xFF;
        bytes[n - 5] ^= 0xFF;
        let path = testdata::write_temp("coverage-corrupt", &bytes);
        let result = execute("coverage", json!({"plugin": path.to_str().unwrap()})).unwrap();
        assert_eq!(result["data"]["compressed_examined"], 0);
        let not_examined = result["coverage"]["not_examined"].as_array().unwrap();
        assert_eq!(not_examined.len(), 1);
        let text = not_examined[0].as_str().unwrap();
        assert!(
            text.starts_with(&format!("WEAP {:08X} @", testdata::PACKED)),
            "{text}"
        );
        assert_eq!(
            result["coverage"]["status"],
            serde_json::to_value(CoverageStatus::Partial).unwrap()
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn unreadable_and_malformed_plugins_fail() {
        let missing = std::env::temp_dir().join("kitcraft-engine-coverage-missing.esm");
        let err = execute("coverage", json!({"plugin": missing.to_str().unwrap()})).unwrap_err();
        assert_eq!(err.kind(), "failed");

        let mut bytes = testdata::plugin();
        bytes.truncate(bytes.len() - 3);
        let path = testdata::write_temp("coverage-truncated", &bytes);
        let err = execute("coverage", json!({"plugin": path.to_str().unwrap()})).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("offset"), "{err}");
        std::fs::remove_file(&path).unwrap();
    }
}
