//! `coverage` (M1.14, M3.F2): which record types and subrecords a plugin holds, against
//! what the schema (`kitcraft_schema::builtin()`) defines.
//!
//! The plugin is streamed one top-level entry at a time (`kitcraft_esm::driver::visit_file`).
//! Every subrecord occurrence is classified by its record type and signature:
//!
//! - `typed`: the record type's definition names the signature with a layout (any type
//!   other than `opaque`) and this payload decoded through it;
//! - `failed`: the definition names the signature with a layout, but this payload did not
//!   decode through it: it does not fit the layout, or the definition does not accept the
//!   subrecord at its position (the record keeps it as raw bytes);
//! - `opaque`: the definition names it but keeps the payload as raw bytes;
//! - `unknown`: the record type has no definition, or its definition does not name the
//!   signature (subrecords inside an `any_until` run, such as a BFCB..BFCE body, count
//!   here: they are kept as raw subrecords).
//!
//! Every record of a defined type is decoded through its generated definition with the
//! plugin's context (the record's form version and the TES4 localized flag, as
//! `kitcraft_esm::Plugin::typed` does) and encoded again; a subrecord position the
//! encoding does not reproduce is a re-encode conflict (`reencode_conflicts`). The first
//! failures and conflicts are listed in `typed_failures` with their offsets. Decoded
//! occurrences are also counted by the `confidence` of the member that decoded them
//! (`decoded_by_confidence`; the weakest, when several members share the signature).
//!
//! Compressed records are decompressed (bounded by the default `DecodeLimits`) unless
//! `decode` is `false`; a record that fails to decompress is listed in
//! `coverage.not_examined`, not fatal.
//!
//! Size controls (the totals always cover the whole plugin): `signatures: false` leaves
//! out each type's per-signature list (`record_types[].signatures` is `null`), and
//! `top: N` lists only the N record types with the most records. `signatures` has no
//! schema default: absent means `true` here (the CLI), and the MCP server passes `false`
//! unless the caller sets it, so an MCP call returns a summary by default.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kitcraft_esm::bridge::{Ctx, RawSub};
use kitcraft_esm::driver::visit_file;
use kitcraft_esm::generated::records::{match_members, Record as TypedRecord, RECORDS};
use kitcraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature, Subrecord, HEADER_LEN};
use kitcraft_schema::{Confidence, FieldType, Schema};
use serde_json::{json, Value};

use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

const ID: &str = "coverage";

/// What the classification means, stated on every result.
pub(crate) const BY_LAYOUT: &str = "subrecords are classified against the schema \
     definitions, and records of a defined type are decoded through their layouts and \
     encoded again (typed: decoded through a layout; failed: named by a typed member but not \
     decoded, because the payload does not fit the layout or the definition does not accept \
     it at that position; opaque: defined as raw bytes; unknown: not named by a definition)";

/// Stated when compressed records were skipped.
pub(crate) const COMPRESSED_SKIPPED: &str = "decode is off: compressed records are counted \
     but their subrecords are not examined";

/// Decode failures listed one by one before the rest are summarized.
const MAX_LISTED_FAILURES: usize = 20;

/// Typed decode failures and re-encode conflicts listed in `typed_failures`.
const MAX_LISTED_TYPED: usize = 20;

/// What the result lists beyond the totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Listing {
    /// Include `record_types[].signatures`.
    signatures: bool,
    /// List at most this many record types (most records first); `None` lists all.
    top: Option<usize>,
}

/// How a subrecord signature relates to its record type's definition. An occurrence of a
/// `Typed` signature whose payload did not decode is counted as `failed` (module docs).
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

/// What a definition says about one subrecord signature.
#[derive(Debug)]
struct Known {
    /// The weakest class among the members using it (`Typed` first).
    class: Class,
    /// The member paths that use it.
    members: Vec<String>,
    /// The weakest confidence among those members; `None` for an `any_until` terminator
    /// that no member defines.
    confidence: Option<Confidence>,
}

/// The schema's view of one record type: its definition name and, per known subrecord
/// signature, what the definition says about it.
#[derive(Debug, Default)]
struct TypeSchema {
    definition: Option<String>,
    known: BTreeMap<[u8; 4], Known>,
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
        let mut known: BTreeMap<[u8; 4], Known> = BTreeMap::new();
        for (path, sub) in record.leaf_subrecords() {
            let class = if matches!(sub.ty, FieldType::Opaque { .. }) {
                Class::Opaque
            } else {
                Class::Typed
            };
            let confidence = sub.provenance.confidence;
            let entry = known.entry(sub.signature.as_bytes()).or_insert(Known {
                class,
                members: Vec::new(),
                confidence: Some(confidence),
            });
            entry.class = entry.class.min(class);
            entry.confidence = Some(entry.confidence.map_or(confidence, |c| c.min(confidence)));
            entry.members.push(path);
        }
        // `any_until` terminators are known even when no member defines them.
        for sig in schema.known_signatures(sig).unwrap_or_default() {
            known.entry(sig.as_bytes()).or_insert(Known {
                class: Class::Opaque,
                members: Vec::new(),
                confidence: None,
            });
        }
        TypeSchema {
            definition: Some(record.name.clone()),
            known,
        }
    }

    fn class(&self, signature: [u8; 4]) -> Class {
        self.known
            .get(&signature)
            .map_or(Class::Unknown, |k| k.class)
    }
}

#[derive(Debug)]
struct SubStats {
    count: u64,
    /// Occurrences of a typed signature whose payload did not decode.
    failed: u64,
    /// FormID of the first record holding this signature.
    first_form: u32,
}

#[derive(Debug)]
struct TypeStats {
    schema: TypeSchema,
    records: u64,
    compressed: u64,
    compressed_examined: u64,
    /// Records decoded through the type's definition.
    typed_records: u64,
    /// Subrecord positions where encoding a decoded record differs from the input.
    reencode_conflicts: u64,
    /// Offset, length and FormID of the first record of this type.
    first: (u64, u64, u32),
    subs: BTreeMap<[u8; 4], SubStats>,
}

/// One subrecord where a definition and the bytes disagree (listed in `typed_failures`).
#[derive(Debug)]
struct TypedFailure {
    record: String,
    signature: String,
    /// Position of the subrecord in its record (`XXXX` prefixes folded).
    index: usize,
    /// Absolute offset; `None` inside a compressed record.
    offset: Option<u64>,
    kind: &'static str,
}

impl TypedFailure {
    fn json(&self) -> Value {
        json!({
            "record": self.record,
            "signature": self.signature,
            "index": self.index,
            "offset": self.offset,
            "kind": self.kind,
        })
    }
}

#[derive(Debug, Default)]
struct Walk {
    types: BTreeMap<[u8; 4], TypeStats>,
    groups: u64,
    records: u64,
    compressed: u64,
    compressed_examined: u64,
    decode_failures: Vec<String>,
    /// The TES4 localized flag (lstring members decode by it).
    localized: bool,
    /// Typed decode failures and re-encode conflicts in the whole plugin.
    typed_failure_count: u64,
    /// The first [`MAX_LISTED_TYPED`] of them.
    typed_failures: Vec<TypedFailure>,
}

/// What decoding one record through its definition showed.
#[derive(Debug, Default)]
struct TypedOutcome {
    /// Positions the record kept raw: placed in a member whose layout the payload does not
    /// fit, or accepted by no member at that position.
    not_decoded: BTreeSet<usize>,
    /// Positions where the encoding differs from the input (or only one side has one).
    reencoded: Vec<usize>,
}

/// Decodes `subs` through the generated definition of `signature` (`None` without one)
/// and encodes the result again.
fn check_typed(signature: Signature, subs: &[Subrecord<'_>], ctx: Ctx) -> Option<TypedOutcome> {
    let members = RECORDS
        .iter()
        .find(|(sig, _)| *sig == signature.bytes())
        .map(|(_, m)| *m)?;
    let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
    let typed = TypedRecord::decode(signature.bytes(), &raw, ctx)?;
    let (_, unmatched) = match_members(&raw, members);
    let mut not_decoded: BTreeSet<usize> = typed.extra().iter().map(|(i, _)| *i).collect();
    not_decoded.extend(unmatched);
    let encoded = typed.encode();
    let len = raw.len().max(encoded.len());
    let reencoded = (0..len)
        .filter(|&i| match (raw.get(i), encoded.get(i)) {
            (Some(r), Some(e)) => {
                r.signature != e.signature
                    || r.data != e.data.as_slice()
                    || r.oversized != e.oversized
            }
            _ => true,
        })
        .collect();
    Some(TypedOutcome {
        not_decoded,
        reencoded,
    })
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

    fn typed_failure(&mut self, failure: TypedFailure) {
        self.typed_failure_count = self.typed_failure_count.saturating_add(1);
        if self.typed_failures.len() < MAX_LISTED_TYPED {
            self.typed_failures.push(failure);
        }
    }

    fn record(&mut self, r: &Record<'_>, schema: Option<&Schema>, decode: Option<&DecodeLimits>) {
        let h = &r.header;
        if self.records == 0 && h.signature.bytes() == *b"TES4" {
            self.localized = kitcraft_esm::tes4::Tes4::new(r).is_localized();
        }
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
                typed_records: 0,
                reencode_conflicts: 0,
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
        let typed = if stats.schema.definition.is_some() {
            check_typed(h.signature, subs, Ctx::for_record(r, self.localized))
        } else {
            None
        };
        if typed.is_some() {
            stats.typed_records = stats.typed_records.saturating_add(1);
        }
        let mut failures = Vec::new();
        for (index, s) in subs.iter().enumerate() {
            let signature = s.signature().bytes();
            let failed = stats.schema.class(signature) == Class::Typed
                && typed
                    .as_ref()
                    .is_some_and(|t| t.not_decoded.contains(&index));
            let sub = stats.subs.entry(signature).or_insert(SubStats {
                count: 0,
                failed: 0,
                first_form: h.form_id,
            });
            sub.count = sub.count.saturating_add(1);
            if failed {
                sub.failed = sub.failed.saturating_add(1);
                failures.push((index, "not_decoded"));
            }
        }
        if let Some(t) = &typed {
            let n = u64::try_from(t.reencoded.len()).unwrap_or(u64::MAX);
            stats.reencode_conflicts = stats.reencode_conflicts.saturating_add(n);
            failures.extend(t.reencoded.iter().map(|i| (*i, "reencoded")));
        }
        for (index, kind) in failures {
            let sub = subs.get(index);
            self.typed_failure(TypedFailure {
                record: format!("{}:{:08X}", h.signature, h.form_id),
                signature: sub.map_or_else(|| "-".to_string(), |s| s.signature().to_string()),
                index,
                offset: sub.and_then(Subrecord::offset),
                kind,
            });
        }
    }
}

/// Distinct-signature counts per class.
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
}

/// Occurrence counts: the typed occurrences that did not decode are counted apart.
#[derive(Debug, Default, Clone, Copy)]
struct Occurrences {
    typed: u64,
    failed: u64,
    opaque: u64,
    unknown: u64,
}

impl Occurrences {
    /// `n` occurrences of a signature of `class`, `failed` of which did not decode.
    fn add(&mut self, class: Class, n: u64, failed: u64) {
        match class {
            Class::Typed => {
                self.typed = self.typed.saturating_add(n.saturating_sub(failed));
                self.failed = self.failed.saturating_add(failed);
            }
            Class::Opaque => self.opaque = self.opaque.saturating_add(n),
            Class::Unknown => self.unknown = self.unknown.saturating_add(n),
        }
    }

    fn merge(&mut self, other: Occurrences) {
        self.typed = self.typed.saturating_add(other.typed);
        self.failed = self.failed.saturating_add(other.failed);
        self.opaque = self.opaque.saturating_add(other.opaque);
        self.unknown = self.unknown.saturating_add(other.unknown);
    }

    fn total(self) -> u64 {
        self.typed
            .saturating_add(self.failed)
            .saturating_add(self.opaque)
            .saturating_add(self.unknown)
    }

    fn json(self) -> Value {
        json!({
            "typed": self.typed,
            "failed": self.failed,
            "opaque": self.opaque,
            "unknown": self.unknown,
        })
    }
}

/// Decoded occurrences per confidence level, every level present, weakest first.
fn confidence_json(counts: &BTreeMap<Confidence, u64>) -> Value {
    let mut out = serde_json::Map::new();
    for c in Confidence::ALL {
        out.insert(
            c.as_str().to_string(),
            json!(counts.get(&c).copied().unwrap_or(0)),
        );
    }
    Value::Object(out)
}

fn add_confidence(into: &mut BTreeMap<Confidence, u64>, c: Confidence, n: u64) {
    let slot = into.entry(c).or_insert(0);
    *slot = slot.saturating_add(n);
}

/// The handler.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let plugin = str_arg(ID, args, "plugin")?;
    let decode = args.get("decode").and_then(Value::as_bool).unwrap_or(true);
    let listing = Listing {
        signatures: args
            .get("signatures")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        // The schema bounds `top` to a non-negative integer; a count past `usize` lists all.
        top: args
            .get("top")
            .and_then(Value::as_u64)
            .map(|n| usize::try_from(n).unwrap_or(usize::MAX)),
    };
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
    Ok(outcome(
        plugin,
        &file_name(path),
        bytes,
        decode,
        listing,
        &walk,
    ))
}

fn outcome(
    plugin: &str,
    file: &str,
    bytes: u64,
    decode: bool,
    listing: Listing,
    walk: &Walk,
) -> Outcome {
    let mut types: Vec<(&[u8; 4], &TypeStats)> = walk.types.iter().collect();
    types.sort_by(|a, b| b.1.records.cmp(&a.1.records).then(a.0.cmp(b.0)));

    let mut occurrences = Occurrences::default();
    let mut by_confidence: BTreeMap<Confidence, u64> = BTreeMap::new();
    let mut reencode_conflicts = 0u64;
    let mut typed_records = 0u64;
    let mut list = Vec::with_capacity(types.len());
    let mut evidence = vec![Evidence {
        file: file.to_string(),
        offset: 0,
        length: bytes,
        record: None,
        subrecord: None,
    }];
    let mut defined = 0u64;
    let listed = listing.top.unwrap_or(usize::MAX).min(types.len());
    for (index, (sig, stats)) in types.iter().enumerate() {
        let signature = Signature(**sig).to_string();
        let mut distinct = Tally::default();
        let mut type_occurrences = Occurrences::default();
        let mut type_confidence: BTreeMap<Confidence, u64> = BTreeMap::new();
        let mut subs = Vec::with_capacity(stats.subs.len());
        for (sub_sig, sub) in &stats.subs {
            let class = stats.schema.class(*sub_sig);
            let known = stats.schema.known.get(sub_sig);
            let confidence = known.and_then(|k| k.confidence);
            distinct.add(class, 1);
            type_occurrences.add(class, sub.count, sub.failed);
            if let (Class::Typed, Some(c)) = (class, confidence) {
                add_confidence(
                    &mut type_confidence,
                    c,
                    sub.count.saturating_sub(sub.failed),
                );
            }
            subs.push(json!({
                "signature": Signature(*sub_sig).to_string(),
                "count": sub.count,
                "class": class.as_str(),
                "failed": sub.failed,
                "confidence": confidence.map(Confidence::as_str),
                "members": known.map(|k| k.members.clone()).unwrap_or_default(),
                "first_record": format!("{signature}:{:08X}", sub.first_form),
            }));
        }
        occurrences.merge(type_occurrences);
        for (c, n) in &type_confidence {
            add_confidence(&mut by_confidence, *c, *n);
        }
        reencode_conflicts = reencode_conflicts.saturating_add(stats.reencode_conflicts);
        typed_records = typed_records.saturating_add(stats.typed_records);
        if stats.schema.definition.is_some() {
            defined = defined.saturating_add(1);
        }
        if index >= listed {
            // Counted in the totals above, not listed (nor its evidence).
            continue;
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
            "typed_records": stats.typed_records,
            "subrecords": type_occurrences.total(),
            "occurrences": type_occurrences.json(),
            "decoded_by_confidence": confidence_json(&type_confidence),
            "reencode_conflicts": stats.reencode_conflicts,
            "distinct": {
                "seen": distinct.total(),
                "typed": distinct.typed,
                "opaque": distinct.opaque,
                "unknown": distinct.unknown,
            },
            "first_offset": offset,
            "signatures": if listing.signatures { Value::Array(subs) } else { Value::Null },
        }));
    }

    let mut limitations = vec![BY_LAYOUT.to_string()];
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
    if walk.typed_failure_count > 0 {
        limitations.push(format!(
            "{} typed subrecords did not decode through their layout and {} subrecord \
             positions did not re-encode identically; typed_failures lists the first {}",
            occurrences.failed,
            reencode_conflicts,
            walk.typed_failures.len()
        ));
    }
    let omitted = types.len().saturating_sub(listed);
    if omitted > 0 {
        limitations.push(format!(
            "record_types lists the {listed} of {} record types with the most records (top); \
             the totals cover all of them",
            types.len()
        ));
    }
    let complete = occurrences.opaque == 0
        && occurrences.unknown == 0
        && occurrences.failed == 0
        && reencode_conflicts == 0
        && skipped == 0;
    let summary = format!(
        "{file}: {} records of {} types ({defined} defined in the schema, {typed_records} \
         records decoded through it); {} subrecords: {} typed, {} failed, {} opaque, {} \
         unknown; {reencode_conflicts} re-encode conflicts; {} compressed records, {} examined",
        walk.records,
        types.len(),
        occurrences.total(),
        occurrences.typed,
        occurrences.failed,
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
        "typed_records": typed_records,
        "subrecords": occurrences.total(),
        "occurrences": occurrences.json(),
        "decoded_by_confidence": confidence_json(&by_confidence),
        "reencode_conflicts": reencode_conflicts,
        "typed_failures": walk.typed_failures.iter().map(TypedFailure::json).collect::<Vec<_>>(),
        "signatures": listing.signatures,
        "top": listing.top,
        "record_types_listed": listed,
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
            opaque_subrecords: Some(
                occurrences
                    .opaque
                    .saturating_add(occurrences.unknown)
                    .saturating_add(occurrences.failed),
            ),
            not_examined,
        },
        limitations,
        evidence,
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{BY_LAYOUT, COMPRESSED_SKIPPED};
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
            json!({"typed": 3, "failed": 0, "opaque": 1, "unknown": 2})
        );

        // Totals and the envelope agree with the per-type counts.
        assert_eq!(data["subrecords"], 10);
        assert_eq!(
            data["occurrences"],
            json!({"typed": 7, "failed": 0, "opaque": 1, "unknown": 2})
        );
        assert_eq!(result["coverage"]["status"], "partial");
        assert_eq!(result["coverage"]["decoded_subrecords"], 7);
        assert_eq!(result["coverage"]["opaque_subrecords"], 3);
        assert_eq!(result["coverage"]["not_examined"], json!([]));
        assert_eq!(result["limitations"], json!([BY_LAYOUT]));

        // Every record of the three defined types decoded through its definition and
        // re-encoded identically; decoded occurrences by the decoding member's confidence
        // (the seed definitions are corpus-confirmed).
        assert_eq!(data["typed_records"], 4);
        assert_eq!(data["reencode_conflicts"], 0);
        assert_eq!(data["typed_failures"], json!([]));
        assert_eq!(weap["typed_records"], 2);
        assert_eq!(find(&weap["signatures"], "EDID")["confidence"], "corpus");
        assert_eq!(find(&weap["signatures"], "EDID")["failed"], 0);
        assert_eq!(dnam["confidence"], Value::Null);
        let by_confidence = &data["decoded_by_confidence"];
        let sum: u64 = ["reference", "corpus", "ck-diff", "verified"]
            .iter()
            .map(|c| by_confidence[c].as_u64().unwrap())
            .sum();
        assert_eq!(sum, 7, "{by_confidence}");
        assert_eq!(
            weap["decoded_by_confidence"]["corpus"]
                .as_u64()
                .unwrap()
                .saturating_add(weap["decoded_by_confidence"]["reference"].as_u64().unwrap()),
            3
        );

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
            json!([BY_LAYOUT, COMPRESSED_SKIPPED])
        );
        std::fs::remove_file(&path).unwrap();
    }

    /// `signatures: false` and `top` shrink the listing; the totals do not change.
    #[test]
    fn signatures_and_top_shrink_the_listing_not_the_totals() {
        let path = testdata::write_temp("coverage-summary", &testdata::plugin());
        let plugin = path.to_str().unwrap();
        let full = execute("coverage", json!({"plugin": plugin})).unwrap();
        assert_eq!(full["data"]["signatures"], true, "absent means true");
        assert_eq!(full["data"]["top"], Value::Null);
        assert_eq!(full["data"]["record_types_listed"], 3);
        assert!(full["data"]["record_types"][0]["signatures"].is_array());

        let summary = execute(
            "coverage",
            json!({"plugin": plugin, "signatures": false, "top": 1}),
        )
        .unwrap();
        let data = &summary["data"];
        for total in [
            "records",
            "subrecords",
            "occurrences",
            "record_types_seen",
            "record_types_defined",
            "summary",
        ] {
            assert_eq!(data[total], full["data"][total], "{total}");
        }
        assert_eq!(summary["coverage"], full["coverage"]);
        assert_eq!(data["signatures"], false);
        assert_eq!(data["top"], 1);
        assert_eq!(data["record_types_listed"], 1);
        let types = data["record_types"].as_array().unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0]["signature"], "WEAP");
        assert_eq!(types[0]["signatures"], Value::Null);
        assert_eq!(
            types[0]["occurrences"],
            full["data"]["record_types"][0]["occurrences"]
        );
        // Evidence: the whole file and the one listed type.
        assert_eq!(summary["evidence"].as_array().unwrap().len(), 2);
        assert_eq!(
            summary["limitations"],
            json!([
                BY_LAYOUT,
                "record_types lists the 1 of 3 record types with the most records (top); \
                 the totals cover all of them"
            ])
        );

        // top: 0 lists none; a top past the type count lists all without a limitation.
        let none = execute("coverage", json!({"plugin": plugin, "top": 0})).unwrap();
        assert_eq!(none["data"]["record_types"], json!([]));
        let all = execute("coverage", json!({"plugin": plugin, "top": 99})).unwrap();
        assert_eq!(all["data"]["record_types"], full["data"]["record_types"]);
        assert_eq!(all["limitations"], json!([BY_LAYOUT]));
        std::fs::remove_file(&path).unwrap();
    }

    /// A typed subrecord whose payload does not fit its layout is counted as `failed`, not
    /// `typed`, and listed with its offset (M3.F2).
    #[test]
    fn payload_that_does_not_fit_its_layout_is_counted_apart() {
        let mut hedr = 1.0f32.to_le_bytes().to_vec();
        hedr.extend_from_slice(&1u32.to_le_bytes());
        hedr.extend_from_slice(&0x800u32.to_le_bytes());
        let mut bytes = testdata::record(b"TES4", 0, 0, &testdata::sub(b"HEDR", &hedr));
        // GMST `f` values are an f32; three bytes do not fit.
        let mut gmst = testdata::sub(b"EDID", b"fBroken\0");
        gmst.extend(testdata::sub(b"DATA", &[1, 2, 3]));
        let group_start = bytes.len();
        bytes.extend(testdata::group(
            b"GMST",
            0,
            &testdata::record(b"GMST", 0, 0x200, &gmst),
        ));
        // Group header, record header, EDID subrecord (6 + 8), then DATA.
        let data_offset = group_start + 24 + 24 + 14;
        let path = testdata::write_temp("coverage-misfit", &bytes);
        let result = execute("coverage", json!({"plugin": path.to_str().unwrap()})).unwrap();
        let data = &result["data"];
        let gmst = find(&data["record_types"], "GMST");
        let sub = find(&gmst["signatures"], "DATA");
        assert_eq!(sub["class"], "typed");
        assert_eq!(sub["count"], 1);
        assert_eq!(sub["failed"], 1);
        assert_eq!(
            gmst["occurrences"],
            json!({"typed": 1, "failed": 1, "opaque": 0, "unknown": 0})
        );
        assert_eq!(
            data["typed_failures"],
            json!([{"record": "GMST:00000200", "signature": "DATA", "index": 1,
                    "offset": data_offset, "kind": "not_decoded"}])
        );
        // Kept raw, so the record still re-encodes identically.
        assert_eq!(data["reencode_conflicts"], 0);
        assert_eq!(result["coverage"]["decoded_subrecords"], 2);
        assert_eq!(result["coverage"]["opaque_subrecords"], 1);
        assert_eq!(result["coverage"]["status"], "partial");
        let limitations = result["limitations"].as_array().unwrap();
        assert_eq!(limitations.len(), 2, "{limitations:?}");
        assert!(
            limitations[1]
                .as_str()
                .unwrap()
                .starts_with("1 typed subrecords did not decode"),
            "{limitations:?}"
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
