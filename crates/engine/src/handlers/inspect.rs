//! `inspect`: one record, with its header fields and its subrecords as opaque hex.
//!
//! Two modes:
//!
//! - **Load order** (M2.D9), when `profile`/`plugins` are given or the session has a load
//!   order: `form` is a formref (`Starfield.esm:01A2B3`), a hex FormID read as stored in
//!   `plugin` (required for bare hex), `runtime:<hex>` (a runtime ID under the runtime-slot
//!   assumption), or an EditorID. The result lists every version in load order with the
//!   winner marked, each override classed against the version before it, and `FULL`/`DESC`
//!   decoded as lstrings (inline text, or the string ID resolved through the plugin's
//!   string tables, with the table and offset as evidence).
//! - **Single plugin** (kept from M1), when only `plugin` is given: below.
//!
//! Single-plugin lookup: when `form` is 1-8 hex digits (optionally `0x`-prefixed), the first record whose
//! header FormID equals it wins, and compressed records are not decompressed for the scan.
//! Otherwise, or when no FormID matches (a second pass), the first record whose `EDID`
//! equals `form` (ASCII case-insensitive) is returned; that pass decompresses compressed
//! records to read their `EDID`. The file is streamed one top-level group at a time and
//! each pass stops at its first match.

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use kitkraft_esm::driver::visit_file;
use kitkraft_esm::{
    DecodeLimits, Entry, Error, ParseOptions, Record, Signature, Subrecord, HEADER_LEN,
};
use serde_json::{json, Value};

use kitkraft_doc::compare::{spans, OverrideClass};
use kitkraft_doc::confidence::{reference_limitation, reference_members};
use kitkraft_doc::flags::RECORD_FLAG_DELETED;
use kitkraft_doc::lstring::is_lstring;
use kitkraft_doc::strings::StringTables;
use kitkraft_doc::typed::{self, Decoded};
use kitkraft_doc::{
    FormRef, GlobalForm, LoadOrder, LoadedRecord, MasterMap, RecordReader, Version,
};

use super::load::{acquire, has_runtime_note, lstring_json, runtime_note, schema_of, Loaded};
use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;
use crate::shapes::{self, kind};

const ID: &str = "inspect";

/// Payload bytes shown per subrecord.
pub(crate) const PAYLOAD_CAP: usize = 256;

/// Single-plugin mode lists raw payloads only; schema-typed values need the load order.
pub(crate) const TYPED_PENDING: &str = "single-plugin mode lists raw payloads; schema-typed \
     values (member, value) are shown in load-order mode (pass `plugins` or `profile`)";

/// No load order yet: the record is this plugin's version only.
pub(crate) const SINGLE_PLUGIN: &str =
    "single plugin: overrides in other plugins are not considered (pass `plugins` or `profile` for the load order)";

/// Stated for a compressed record.
pub(crate) const COMPRESSED_OFFSETS: &str = "record is compressed: subrecord offsets are \
     relative to its decompressed data; evidence covers the compressed body in the file";

/// Stated when a payload was cut.
pub(crate) const PAYLOAD_CUT: &str = "payload_hex shows at most the first 256 bytes (or \
     max_bytes) of each subrecord (see payload_truncated); pass full for every byte and the \
     decoded VMAD";

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

/// The handler: load-order mode when a load order is named or loaded, else single plugin.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let named = args.get("profile").is_some() || args.get("plugins").is_some();
    let single = !named && args.get("plugin").is_some();
    if single {
        let inputs = &[super::resolve::Input::file("plugin")];
        return run_single(&super::resolve::inputs(Some(session), ID, args, inputs)?);
    }
    let schema = schema_of(session);
    let loaded = acquire(session, args, ID)?;
    run_load_order(loaded, args, schema)
}

/// Single-plugin mode.
fn run_single(args: &Value) -> Result<Outcome, EngineError> {
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
    Ok(outcome(
        plugin,
        &file_name(path),
        form,
        matched_by,
        &hit,
        payload_cap(args),
    ))
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

fn outcome(
    plugin: &str,
    file: &str,
    form: &str,
    matched_by: &str,
    hit: &Hit,
    cap: usize,
) -> Outcome {
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
        let shown = data.get(..cap).unwrap_or(data);
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

/// Find the form `form` names in the load order, and how it was matched (`inspect`'s
/// lookup, shared by `refs` and `planet`; errors name `command`).
pub(super) fn find_form(
    command: &str,
    loaded: &Loaded,
    form: &str,
    plugin: Option<&str>,
) -> Result<(GlobalForm, &'static str), EngineError> {
    let lo = &loaded.lo;
    let present = |g: &GlobalForm| !lo.forms.versions(*g).is_empty();
    if let Some(f) = FormRef::parse(form) {
        let p = lo
            .find(&f.plugin)
            .ok_or_else(|| failed(command, format!("`{}` is not in the load order", f.plugin)))?;
        return Ok((
            GlobalForm {
                plugin: p,
                object_id: f.object_id,
            },
            "formref",
        ));
    }
    if let Some(text) = form
        .get(..RUNTIME_PREFIX.len())
        .filter(|p| p.eq_ignore_ascii_case(RUNTIME_PREFIX))
        .and_then(|_| form.get(RUNTIME_PREFIX.len()..))
    {
        let raw = parse_form_id(text).ok_or_else(|| {
            failed(
                command,
                format!("`{form}`: `{text}` is not a hex runtime ID"),
            )
        })?;
        return lo
            .from_runtime_id(raw)
            .filter(present)
            .map(|g| (g, "runtime_id"))
            .ok_or_else(|| {
                failed(
                    command,
                    format!("no form has runtime ID {raw:08X} (under `runtime-slots-by-scale`)"),
                )
            });
    }
    let bare_hex = parse_form_id(form);
    if let (Some(raw), Some(name)) = (bare_hex, plugin) {
        let p = lo
            .find(name)
            .ok_or_else(|| failed(command, format!("`{name}` is not in the load order")))?;
        let g = lo
            .resolve(p, raw)
            .map_err(|e| failed(command, e.to_string()))?;
        if present(&g) {
            return Ok((g, "form_id"));
        }
    }
    for (index, loc) in loaded.edid_hits(form) {
        if let Ok(g) = lo.resolve(index, loc.form_id) {
            return Ok((g, "editor_id"));
        }
    }
    if bare_hex.is_some() && plugin.is_none() {
        // A bare hex FormID means nothing without the file whose masters number it.
        return Err(failed(
            command,
            format!(
                "`{form}` is no EditorID; as a hex FormID it needs `plugin` (the plugin whose \
                 numbering it uses), or write `runtime:{form}` for a runtime ID"
            ),
        ));
    }
    Err(failed(
        command,
        format!(
            "no record with formref, FormID, runtime ID or EditorID `{form}` in the load order"
        ),
    ))
}

/// Prefix naming a runtime ID in `form` (load-order mode).
const RUNTIME_PREFIX: &str = "runtime:";

/// Where a version comes from.
struct VersionCtx<'a> {
    lo: &'a kitkraft_doc::LoadOrder,
    schema: Option<&'a kitkraft_schema::Schema>,
    plugin: u16,
    file: &'a str,
    localized: bool,
    edid_first: Option<u8>,
    /// A version read from a file outside the load order (`set`'s dry run, M7.S6): its
    /// FormIDs resolve through this master map, and its own forms are named by `file`.
    outside: Option<&'a MasterMap>,
    /// Payload bytes shown per subrecord (`max_bytes`; `full` shows every byte).
    cap: usize,
    /// `full`: also decode each `VMAD` (scripts, properties, object values) and the
    /// `LMSW` records' `REFL` (swap entries).
    full: bool,
}

/// The payload bytes `inspect` shows per subrecord: every byte with `full`, else
/// `max_bytes`, else [`PAYLOAD_CAP`].
fn payload_cap(args: &Value) -> usize {
    if args.get("full").and_then(Value::as_bool) == Some(true) {
        return usize::MAX;
    }
    args.get("max_bytes")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(PAYLOAD_CAP)
}

/// A decoded `VMAD` payload as JSON (`inspect --full`): every script with its properties,
/// object values as `formref`s with their alias, and the size of the fragment block that
/// follows the scripts (not decoded).
fn vmad_json(data: &[u8], vc: &VersionCtx<'_>) -> Value {
    use kitkraft_doc::vmad::{decode, Property, Value as V};
    fn object(o: &kitkraft_doc::vmad::Object, vc: &VersionCtx<'_>) -> Value {
        let form = match vc.outside {
            Some(map) => shapes::formref_in(vc.lo, map, vc.file, o.form_id),
            None => shapes::formref(vc.lo, vc.plugin, o.form_id),
        };
        json!({"object": form, "alias": o.alias})
    }
    fn float(bits: u32) -> Value {
        serde_json::Number::from_f64(f64::from(f32::from_bits(bits)))
            .map_or_else(|| json!(format!("0x{bits:08X}")), Value::Number)
    }
    fn text(b: &[u8]) -> Value {
        json!(String::from_utf8_lossy(b))
    }
    fn props(list: &[Property], vc: &VersionCtx<'_>, depth: usize) -> Value {
        Value::Array(list.iter().map(|p| prop(p, vc, depth)).collect())
    }
    fn prop(p: &Property, vc: &VersionCtx<'_>, depth: usize) -> Value {
        let next = depth.saturating_add(1);
        let value = if depth > 16 {
            Value::Null
        } else {
            match &p.value {
                V::None | V::Variable => Value::Null,
                V::Object(o) => object(o, vc),
                V::String(b) => text(b),
                V::Int(i) => json!(i),
                V::Float(f) => float(*f),
                V::Bool(b) => json!(*b != 0),
                V::Struct(m) => props(m, vc, next),
                V::Objects(v) => Value::Array(v.iter().map(|o| object(o, vc)).collect()),
                V::Strings(v) => Value::Array(v.iter().map(|b| text(b)).collect()),
                V::Ints(v) => json!(v),
                V::Floats(v) => Value::Array(v.iter().map(|f| float(*f)).collect()),
                V::Bools(v) => Value::Array(v.iter().map(|b| json!(*b != 0)).collect()),
                V::Variables(n) => json!({"variables": n}),
                V::Structs(v) => Value::Array(v.iter().map(|m| props(m, vc, next)).collect()),
            }
        };
        json!({"name": text(&p.name), "type": p.ty, "flags": p.flags, "value": value})
    }
    match decode(data) {
        Ok(v) => json!({
            "version": v.version,
            "object_format": v.object_format,
            "scripts": v.scripts.iter().map(|s| json!({
                "name": s.name_text(),
                "flags": s.flags,
                "properties": props(&s.properties, vc, 0),
            })).collect::<Vec<_>>(),
            "fragment_bytes": v.rest.len(),
        }),
        Err(e) => json!({"error": e}),
    }
}

/// The `REFL` payload of an `LMSW` record as JSON (`inspect --full`): its swap entries
/// with key and override material paths; `{"error": ...}` when the payload does not
/// decode or its lists do not parse.
fn lmsw_json(data: &[u8]) -> Value {
    use kitkraft_doc::refl::{self, SwapEntry};
    let r = match refl::decode(data) {
        Ok(r) => r,
        Err(e) => return json!({"error": e}),
    };
    let entries = match refl::lmsw_entries(&r) {
        Ok(v) => v,
        Err(e) => return json!({"error": e}),
    };
    let entries = entries
        .iter()
        .filter_map(|e| match e {
            SwapEntry::Swap {
                key, r#override, ..
            } => Some(json!({"key": key, "override": r#override})),
            SwapEntry::Ref(_) | SwapEntry::Property { .. } => None,
        })
        .collect::<Vec<_>>();
    json!({"entries": entries})
}

/// The `Addon` entries of an `ARMO` record as JSON (`inspect --full`): each `INDX`/`MODL`
/// pair names the `ARMA` addon, with the `ARMA`'s four model `FileName`s by slot. An
/// `INDX` with no `MODL` after it, and a `MODL` shorter than 4 bytes, are skipped; an
/// `ARMA` that does not resolve anywhere in the load order is `"missing"` with empty
/// `models`.
fn armo_addons_json(
    lo: &LoadOrder,
    reader: &mut RecordReader,
    armo_plugin: u16,
    rec: &LoadedRecord,
) -> Value {
    let subs = &rec.subs;
    let mut addons = Vec::new();
    for (i, sub) in subs.iter().enumerate() {
        if sub.signature().0 != *b"INDX" {
            continue;
        }
        // An `INDX` with no `MODL` after it is skipped explicitly.
        let Some(modl) = subs.get(i + 1) else {
            continue;
        };
        if modl.signature().0 != *b"MODL" {
            continue;
        }
        let Some(bytes) = sub.data().get(..2) else {
            continue;
        };
        // An `INDX` shorter than 2 bytes is malformed: skipped, like a short `MODL`.
        let index = match bytes {
            [hi, lo] => u16::from_le_bytes([*hi, *lo]),
            _ => continue,
        };
        let Some(bytes) = modl.data().get(..4) else {
            continue;
        };
        // A `MODL` shorter than 4 bytes is skipped explicitly.
        let raw = match bytes {
            [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
            _ => continue,
        };
        let entry = match lo.resolve(armo_plugin, raw) {
            Err(_) => armo_addon_missing(index),
            Ok(g) => match lo.forms.winner(g) {
                None => armo_addon_missing(index),
                Some(w) => match lo.record(w.plugin, w.record) {
                    None => armo_addon_missing(index),
                    Some(loc) => {
                        let formid = lo
                            .runtime_id(g)
                            .map(|id| format!("0x{id:08X}"))
                            .unwrap_or_default();
                        let editor_id = lo.edid(w.plugin, loc).unwrap_or("");
                        let plugin = lo
                            .plugin(w.plugin)
                            .map_or_else(String::new, |p| p.name.clone());
                        json!({
                            "index": index,
                            "arma": {
                                "formid": formid,
                                "editor_id": editor_id,
                                "plugin": plugin,
                            },
                            "models": armo_models_json(lo, reader, w.plugin, loc),
                        })
                    }
                },
            },
        };
        addons.push(entry);
    }
    json!(addons)
}

/// One `Addon` entry whose `ARMA` is not in the load order.
fn armo_addon_missing(index: u16) -> Value {
    json!({"index": index, "arma": "missing", "models": {}})
}

/// The four model `FileName`s of an `ARMA` by slot (`MOD2` male, `MOD3` female, `MOD4`
/// male first person, `MOD5` female first person); an absent subrecord is `""`.
fn armo_models_json(
    lo: &LoadOrder,
    reader: &mut RecordReader,
    plugin: u16,
    loc: &kitkraft_doc::RecordLoc,
) -> Value {
    let Ok(rec) = reader.read(lo, plugin, loc) else {
        return json!({});
    };
    let mut male = String::new();
    let mut female = String::new();
    let mut male_first_person = String::new();
    let mut female_first_person = String::new();
    for sub in &rec.subs {
        let sig = sub.signature().0;
        if sig == *b"MOD2" {
            male = kitkraft_doc::index::zstring(sub.data());
        } else if sig == *b"MOD3" {
            female = kitkraft_doc::index::zstring(sub.data());
        } else if sig == *b"MOD4" {
            male_first_person = kitkraft_doc::index::zstring(sub.data());
        } else if sig == *b"MOD5" {
            female_first_person = kitkraft_doc::index::zstring(sub.data());
        }
    }
    json!({
        "male": male,
        "female": female,
        "male_first_person": male_first_person,
        "female_first_person": female_first_person,
    })
}

/// Where a version handed to [`version_entry`] comes from.
pub(super) enum VersionAt<'a> {
    /// A version of the load order.
    LoadOrder(Version),
    /// A plugin outside the load order (a staged write): its file name, whether it is
    /// localized, and how its FormIDs resolve (`own_index` past the load order's plugins).
    Outside {
        file: &'a str,
        localized: bool,
        map: &'a MasterMap,
    },
}

/// One version of a record in the shape of `inspect`'s `versions[]` entries (M7.S6: the
/// `before` and `after` of a dry run). An outside version has `load_index: null`.
pub(super) fn version_entry(
    loaded: &mut Loaded,
    schema: Option<&kitkraft_schema::Schema>,
    rec: &LoadedRecord,
    at: &VersionAt<'_>,
    evidence: &mut Vec<Evidence>,
    notes: &mut Vec<String>,
) -> Value {
    let lo = &loaded.lo;
    let (file, localized, plugin, outside, load_index, basis) = match at {
        VersionAt::LoadOrder(v) => {
            let p = lo.plugin(v.plugin);
            (
                p.map_or_else(String::new, |p| p.name.clone()),
                p.and_then(|p| p.header())
                    .is_some_and(|h| h.flags.localized()),
                v.plugin,
                None,
                json!(v.plugin),
                v.basis.as_str(),
            )
        }
        VersionAt::Outside {
            file,
            localized,
            map,
        } => (
            (*file).to_string(),
            *localized,
            map.own_index,
            Some(*map),
            Value::Null,
            "direct",
        ),
    };
    let vc = VersionCtx {
        lo,
        schema,
        plugin,
        file: &file,
        localized,
        edid_first: kitkraft_doc::compare::edid_first(rec),
        outside,
        cap: PAYLOAD_CAP,
        full: false,
    };
    let mut reference = Vec::new();
    let (subs, _, _) = version_subrecords(
        &mut loaded.strings,
        &vc,
        rec,
        evidence,
        notes,
        &mut reference,
    );
    let h = &rec.record.header;
    json!({
        "plugin": file,
        "load_index": load_index,
        "winner": true,
        "deleted": h.flags & RECORD_FLAG_DELETED != 0,
        "resolved_by": basis,
        "raw_form_id": format!("{:08X}", h.form_id),
        "record": {
            "signature": h.signature.to_string(),
            "flags": format!("0x{:08X}", h.flags),
            "compressed": rec.compressed,
            "offset": rec.offset(),
            "data_size": rec.data_size(),
            "timestamp": h.timestamp,
            "version_control": h.version_control,
            "form_version": h.form_version,
            "unknown": h.unknown,
        },
        "subrecord_offsets": if rec.compressed { "decompressed" } else { "file" },
        "subrecords": subs,
    })
}

/// A decoded schema value as JSON, with a `kind` marker on every object that is not a
/// struct (M4.W0, `crate::shapes`): FormIDs as `formref`s resolved through the plugin's
/// masters, string IDs as `lstring`, opaque members as `opaque` byte ranges of the payload
/// (and of the file, when `payload_at`, the payload's absolute offset, is known).
fn decoded_json(v: &Decoded, vc: &VersionCtx<'_>, payload_at: Option<u64>) -> Value {
    match v {
        Decoded::Int(i) => i64::try_from(*i).map_or_else(|_| json!(i.to_string()), |x| json!(x)),
        Decoded::Float(f) => {
            serde_json::Number::from_f64(*f).map_or_else(|| json!(f.to_string()), Value::Number)
        }
        Decoded::Form(raw) => match vc.outside {
            Some(map) => shapes::formref_in(vc.lo, map, vc.file, *raw),
            None => shapes::formref(vc.lo, vc.plugin, *raw),
        },
        Decoded::Sig(s) => json!(String::from_utf8_lossy(s)),
        Decoded::Text(t) => json!(t),
        Decoded::LStringId(id) => {
            json!({"kind": kind::LSTRING, "string_id": format!("0x{id:08X}")})
        }
        Decoded::Struct(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), decoded_json(v, vc, payload_at)))
                .collect(),
        ),
        Decoded::Array(items) => Value::Array(
            items
                .iter()
                .map(|i| decoded_json(i, vc, payload_at))
                .collect(),
        ),
        Decoded::Opaque { at, len } => json!({"kind": kind::OPAQUE, "opaque": {
            "payload_offset": at,
            "length": len,
            "file_offset": payload_at.and_then(|p| p.checked_add(u64::try_from(*at).ok()?)),
        }}),
    }
}

/// Subrecords of one version as JSON, with evidence; lstrings decoded, and fields decoded
/// through the schema where it defines the subrecord. Returns the list, whether a payload
/// was cut, and how many subrecords decoded through the schema; the `reference`-confidence
/// members of decoded values are added to `reference` (M3.D3).
fn version_subrecords(
    strings: &mut StringTables,
    vc: &VersionCtx<'_>,
    rec: &LoadedRecord,
    evidence: &mut Vec<Evidence>,
    notes: &mut Vec<String>,
    reference: &mut Vec<String>,
) -> (Vec<Value>, bool, u64) {
    let (file, localized) = (vc.file, vc.localized);
    let schema = vc.schema;
    let ctx = typed::Ctx {
        form_version: rec.record.header.form_version,
        localized,
        edid_first: vc.edid_first,
    };
    let mut typed_count = 0u64;
    let record_id = rec.label();
    let body_offset = rec.offset().saturating_add(HEADER_LEN as u64);
    let mut cut = false;
    let mut list = Vec::with_capacity(rec.subs.len());
    // Each subrecord's layout comes from its position in the record, not its signature
    // alone (`kitkraft_doc::typed::placements`).
    let record_sig = rec.record.header.signature.0;
    let all_sigs: Vec<[u8; 4]> = rec.subs.iter().map(|s| s.signature().0).collect();
    let placed = schema.and_then(|sc| typed::placements(sc, record_sig, &all_sigs));
    for (index, (s, span)) in rec.subs.iter().zip(spans(rec)).enumerate() {
        let data = s.data();
        let sig = s.signature().to_string();
        let shown = data.get(..vc.cap).unwrap_or(data);
        let truncated = shown.len() < data.len();
        cut |= truncated;
        let header_len = s.encoded_len().saturating_sub(data.len() as u64);
        let (offset, data_offset) = match span.file_offset {
            Some(o) => (o.saturating_sub(header_len), o),
            None => (
                span.body_offset.saturating_sub(header_len),
                span.body_offset,
            ),
        };
        let mut entry = json!({
            "signature": sig,
            "size": data.len(),
            "oversized": s.has_oversize_prefix(),
            "offset": offset,
            "data_offset": data_offset,
            "payload_hex": hex(shown),
            "payload_truncated": truncated,
        });
        if vc.full && sig == "VMAD" {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("vmad".into(), vmad_json(data, vc));
            }
        }
        if vc.full && sig == "REFL" && record_sig == *b"LMSW" {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("lmsw".into(), lmsw_json(data));
            }
        }
        if is_lstring(&sig) {
            let resolved = lstring_json(
                strings,
                (file, &record_id, &sig),
                data,
                localized,
                evidence,
                notes,
            );
            if let (Some(obj), Value::Object(extra)) = (entry.as_object_mut(), resolved) {
                obj.extend(extra);
            }
        } else if sig == "EDID" {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("text".into(), json!(kitkraft_doc::index::zstring(data)));
            }
        }
        let def = schema.and_then(|sc| {
            let placement = placed.as_ref().and_then(|p| p.get(index));
            match typed::layout_of(sc, record_sig, s.signature().0, index, placement) {
                Ok(layout) => layout.map(|l| (l.ty, l.name)),
                Err(e @ typed::LayoutError::Unplaced { .. }) => {
                    notes.push(format!("{record_id} in {file}: {e}; shown as bytes"));
                    None
                }
                // Raw by definition (inside a component block): opaque, as the schema says.
                Err(_) => None,
            }
        });
        if let (Some((ty, name)), Some(obj)) = (def, entry.as_object_mut()) {
            obj.insert("member".into(), json!(name));
            match typed::decode(ty, data, &ctx) {
                Ok(Decoded::Opaque { .. }) => {}
                Ok(v) => {
                    typed_count = typed_count.saturating_add(1);
                    obj.insert("value".into(), decoded_json(&v, vc, span.file_offset));
                    if let Some(sc) = schema {
                        reference.extend(reference_members(
                            sc,
                            rec.record.header.signature.0,
                            s.signature().0,
                            None,
                        ));
                    }
                }
                Err(e) => {
                    notes.push(format!(
                        "{sig} of {record_id} in {file} does not decode through its schema layout ({e})"
                    ));
                }
            }
        }
        if span.file_offset.is_some() {
            evidence.push(Evidence {
                file: file.to_string(),
                offset: data_offset,
                length: data.len() as u64,
                record: Some(record_id.clone()),
                subrecord: Some(sig),
            });
        }
        list.push(entry);
    }
    if rec.compressed {
        evidence.push(Evidence {
            file: file.to_string(),
            offset: body_offset,
            length: rec.data_size(),
            record: Some(record_id),
            subrecord: None,
        });
    }
    (list, cut, typed_count)
}

fn class_json(class: &OverrideClass) -> Value {
    match class {
        OverrideClass::IdenticalToPrevious | OverrideClass::Deleted => {
            json!({"class": class.as_str()})
        }
        OverrideClass::Differs {
            flags,
            order,
            subrecords,
        } => json!({
            "class": class.as_str(),
            "flags_differ": flags,
            "order_differs": order,
            "subrecords": subrecords
                .iter()
                .map(|d| super::diff::sub_diff_json(d, false))
                .collect::<Vec<_>>(),
        }),
    }
}

/// Load-order mode.
fn run_load_order(
    loaded: &mut Loaded,
    args: &Value,
    schema: Option<&kitkraft_schema::Schema>,
) -> Result<Outcome, EngineError> {
    let form_arg = str_arg(ID, args, "form")?;
    let plugin = args.get("plugin").and_then(Value::as_str);
    let cap = payload_cap(args);
    let full = args.get("full").and_then(Value::as_bool) == Some(true);
    let lo = &loaded.lo;
    let (form, matched_by) = find_form(ID, loaded, form_arg, plugin)?;
    let versions: Vec<Version> = lo.forms.versions(form).to_vec();
    if versions.is_empty() {
        return Err(failed(
            ID,
            format!("{} has no version in the load order", lo.form_label(form)),
        ));
    }
    let mut reader = RecordReader::new();
    let mut records = Vec::with_capacity(versions.len());
    for v in &versions {
        let loc = lo
            .record(v.plugin, v.record)
            .copied()
            .ok_or_else(|| failed(ID, "version missing from the index"))?;
        records.push(
            reader
                .read(lo, v.plugin, &loc)
                .map_err(|e| failed(ID, e.to_string()))?,
        );
    }
    let mut evidence = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut cut_any = false;
    let mut compressed_any = false;
    let mut opaque = 0u64;
    let mut typed_total = 0u64;
    let mut reference: Vec<String> = Vec::new();
    let mut list = Vec::with_capacity(versions.len());
    let last = versions.len().saturating_sub(1);
    for (i, (v, rec)) in versions.iter().zip(&records).enumerate() {
        let Some(p) = lo.plugin(v.plugin) else {
            continue;
        };
        let localized = p.header().is_some_and(|h| h.flags.localized());
        evidence.push(Evidence {
            file: p.name.clone(),
            offset: rec.offset(),
            length: HEADER_LEN as u64,
            record: Some(rec.label()),
            subrecord: None,
        });
        let vc = VersionCtx {
            lo,
            schema,
            plugin: v.plugin,
            file: &p.name,
            localized,
            edid_first: lo
                .record(v.plugin, v.record)
                .and_then(|l| lo.edid(v.plugin, l))
                .and_then(|e| e.bytes().next()),
            outside: None,
            cap,
            full,
        };
        let (subs, cut, typed_here) = version_subrecords(
            &mut loaded.strings,
            &vc,
            rec,
            &mut evidence,
            &mut notes,
            &mut reference,
        );
        typed_total = typed_total.saturating_add(typed_here);
        cut_any |= cut;
        compressed_any |= rec.compressed;
        opaque = opaque.saturating_add(rec.subs.len() as u64);
        let previous = i
            .checked_sub(1)
            .and_then(|j| Some((versions.get(j)?, records.get(j)?)));
        let class = previous.map(|prev| {
            class_json(&kitkraft_doc::conflicts::classify_versions(
                lo,
                schema,
                prev,
                (v, rec),
            ))
        });
        let comparable = i
            .checked_sub(1)
            .and_then(|j| versions.get(j))
            .map(|prev| lo.encodings_compatible(prev.plugin, v.plugin));
        let h = &rec.record.header;
        let mut version = json!({
            "plugin": p.name,
            "load_index": v.plugin,
            "winner": i == last,
            "deleted": h.flags & RECORD_FLAG_DELETED != 0,
            "resolved_by": v.basis.as_str(),
            "raw_form_id": format!("{:08X}", h.form_id),
            "record": {
                "signature": h.signature.to_string(),
                "flags": format!("0x{:08X}", h.flags),
                "compressed": rec.compressed,
                "offset": rec.offset(),
                "data_size": rec.data_size(),
                "timestamp": h.timestamp,
                "version_control": h.version_control,
                "form_version": h.form_version,
                "unknown": h.unknown,
            },
            "subrecord_offsets": if rec.compressed { "decompressed" } else { "file" },
            "subrecords": subs,
            "vs_previous": class,
            "payloads_comparable": comparable,
        });
        if full && h.signature.0 == *b"ARMO" {
            if let Some(obj) = version.as_object_mut() {
                obj.insert(
                    "addons".into(),
                    armo_addons_json(lo, &mut reader, v.plugin, rec),
                );
            }
        }
        list.push(version);
    }
    let winner = versions.last().copied();
    let editor_id = winner
        .and_then(|w| Some((w, lo.record(w.plugin, w.record)?)))
        .and_then(|(w, loc)| lo.edid(w.plugin, loc))
        .map(str::to_string);
    let signature = records
        .last()
        .map(|r| r.record.header.signature.to_string())
        .unwrap_or_default();
    let runtime = lo.runtime_id(form);
    let label = lo.form_label(form);
    let winner_name = winner
        .and_then(|w| lo.plugin(w.plugin))
        .map(|p| p.name.clone());
    let winner_deleted = winner
        .and_then(|w| lo.record(w.plugin, w.record))
        .is_some_and(|l| l.flags & RECORD_FLAG_DELETED != 0);
    let assumed: Vec<String> = versions
        .iter()
        .filter(|v| v.basis.assumed())
        .filter_map(|v| {
            let p = lo.plugin(v.plugin)?;
            let raw = lo.record(v.plugin, v.record)?.form_id;
            Some(format!(
                "{} stores it as {raw:08X} ({})",
                p.name,
                v.basis.as_str()
            ))
        })
        .collect();
    let mut limitations = vec![
        "subrecords show `value` only where the schema defines a layout (lane B); every other payload is hex"
            .to_string(),
    ];
    limitations.extend(loaded.limitations.iter().cloned());
    if runtime.is_some() && !has_runtime_note(&limitations) {
        limitations.push(runtime_note(lo));
    }
    if compressed_any {
        limitations.push(COMPRESSED_OFFSETS.to_string());
    }
    if cut_any {
        limitations.push(PAYLOAD_CUT.to_string());
    }
    if versions.len() > 1 {
        limitations.push(format!(
            "vs_previous: {}; where payloads_comparable is false the two plugins encode FormIDs \
             differently, so a difference inside an opaque payload may be encoding only",
            super::diff::FIELD_LEVEL
        ));
    }
    if winner_deleted {
        limitations.push(
            "the winning version is deleted (record flag 0x20): the form is removed in game, \
             and that version's subrecords are not its data"
                .to_string(),
        );
    }
    if !assumed.is_empty() {
        limitations.push(format!(
            "version(s) resolved to this form by an assumption (docs/load-order.md): {}",
            assumed.join("; ")
        ));
    }
    reference.sort();
    reference.dedup();
    if let Some(l) = reference_limitation(&[(signature.clone(), reference)]) {
        limitations.push(l);
    }
    notes.sort();
    notes.dedup();
    limitations.extend(notes);
    let source = loaded.source_check(form);
    let summary = format!(
        "{label} {signature} {}: {} version(s), winner {}{}{}",
        editor_id.as_deref().unwrap_or("(no EditorID)"),
        versions.len(),
        winner_name.as_deref().unwrap_or("?"),
        if winner_deleted { " (deleted)" } else { "" },
        runtime.map_or_else(String::new, |r| format!(", runtime ID {r:08X} (assumed)")),
    );
    let data = json!({
        "mode": "load_order",
        "load_order": loaded.source,
        "language": loaded.language,
        "form": form_arg,
        "matched_by": matched_by,
        "global": label,
        "defining_plugin": lo.plugin(form.plugin).map(|p| p.name.clone()),
        "object_id": format!("{:06X}", form.object_id),
        "runtime_id": runtime.map(|r| format!("{r:08X}")),
        "runtime_id_basis": runtime.map(|_| match lo.runtime_basis {
            kitkraft_doc::RuntimeBasis::Inferred => "assumed:runtime-slots-by-scale",
            kitkraft_doc::RuntimeBasis::ConfirmedBySave(_) => "confirmed-by-save",
        }),
        "signature": signature,
        "editor_id": editor_id,
        "winner": winner_name,
        "winner_deleted": winner_deleted,
        "winner_source": {
            "safe": source.is_safe(),
            "blocking": source.blocking,
            "unrelated": source.unrelated,
        },
        "versions": list,
        "summary": summary,
    });
    Ok(Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: Some(typed_total),
            opaque_subrecords: Some(opaque.saturating_sub(typed_total)),
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
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
        assert_eq!(sigs, ["EDID", "FULL", "MODL", "FLLD", "DNAM"]);
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
        assert_eq!(subs[4]["size"], 300);
        assert_eq!(subs[4]["payload_hex"].as_str().unwrap().len(), 512);
        assert_eq!(subs[4]["payload_truncated"], true);

        assert_eq!(
            result["limitations"],
            json!([TYPED_PENDING, SINGLE_PLUGIN, PAYLOAD_CUT])
        );
        assert_eq!(result["coverage"]["status"], "partial");
        assert_eq!(result["coverage"]["opaque_subrecords"], 5);
        assert_eq!(result["coverage"]["decoded_subrecords"], 0);
        let evidence = result["evidence"].as_array().unwrap();
        assert_eq!(evidence.len(), 1 + 5);
        assert_eq!(
            evidence[0],
            json!({"kind": "evidence", "file": file, "offset": rec_off, "length": 24,
                   "record": "WEAP:0001A2B3", "subrecord": null})
        );
        assert_eq!(evidence[5]["subrecord"], "DNAM");
        assert_eq!(evidence[5]["length"], 300);
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

        // No plugin and no load order.
        let err = execute("inspect", json!({"form": "TestRifle"})).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("no load order"), "{err}");
        let err = execute("inspect", json!({"plugin": "x.esm"})).unwrap_err();
        assert_eq!(err.kind(), "invalid_args");
    }

    #[test]
    fn inspect_across_a_load_order_shows_every_version_and_the_winner() {
        use crate::handlers::load::tests::{fixture, plugins_args};
        let dir = fixture("inspect-lo");
        let mut args = plugins_args(&dir);
        args["form"] = json!("Base.esm:000800");
        let result = execute("inspect", args.clone()).unwrap();
        let data = &result["data"];
        assert_eq!(data["mode"], "load_order");
        assert_eq!(data["matched_by"], "formref");
        assert_eq!(data["global"], "Base.esm:000800");
        assert_eq!(data["editor_id"], "OldEarthHuntingRifle");
        assert_eq!(data["winner"], "Patch.esm");
        assert_eq!(data["runtime_id"], "00000800");
        let versions = data["versions"].as_array().unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0]["plugin"], "Base.esm");
        assert_eq!(versions[0]["winner"], false);
        assert_eq!(versions[1]["winner"], true);
        assert_eq!(
            versions[0]["subrecords"][1]["text"],
            "Old Earth Hunting Rifle"
        );
        assert_eq!(
            versions[1]["subrecords"][1]["text"],
            "Old Soviet Marksman Rifle"
        );
        assert_eq!(versions[1]["vs_previous"]["class"], "differs");
        assert_eq!(
            versions[1]["vs_previous"]["subrecords"],
            json!([{
                "signature": "FULL",
                "occurrence": 0,
                "change": "changed",
                "member": "Name",
                "fields": [{
                    "path": "FULL.Name",
                    "before": "Old Earth Hunting Rifle",
                    "after": "Old Soviet Marksman Rifle",
                }],
            }])
        );
        assert_eq!(versions[1]["payloads_comparable"], true);
        // Fields the schema defines carry their member name and decoded value.
        assert_eq!(versions[0]["subrecords"][0]["member"], "EditorId");
        assert_eq!(
            versions[0]["subrecords"][0]["value"],
            "OldEarthHuntingRifle"
        );
        assert!(result["coverage"]["decoded_subrecords"].as_u64().unwrap() >= 2);
        // Every payload has evidence pointing at its bytes.
        let patch = std::fs::read(dir.join("Patch.esm")).unwrap();
        let full = &versions[1]["subrecords"][1];
        let at = full["data_offset"].as_u64().unwrap() as usize;
        assert_eq!(&patch[at..at + 9], b"Old Sovie");
        let evidence = result["evidence"].as_array().unwrap();
        assert_eq!(evidence.len(), 2 * (1 + 3));
        assert!(evidence.iter().any(|e| e["file"] == "Patch.esm"
            && e["subrecord"] == "FULL"
            && e["offset"].as_u64() == Some(at as u64)));
        assert!(result["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("runtime-slots-by-scale")));

        // EditorID, runtime ID, and a raw FormID read in a named plugin.
        args["form"] = json!("pistol");
        let r = execute("inspect", args.clone()).unwrap();
        assert_eq!(r["data"]["matched_by"], "editor_id");
        assert_eq!(
            r["data"]["versions"][1]["vs_previous"]["class"],
            "identical-to-previous"
        );
        args["form"] = json!("runtime:00000801");
        let r = execute("inspect", args.clone()).unwrap();
        assert_eq!(r["data"]["matched_by"], "runtime_id");
        assert_eq!(r["data"]["global"], "Base.esm:000801");
        assert_eq!(r["data"]["versions"][0]["resolved_by"], "direct");
        assert_eq!(r["data"]["versions"][0]["deleted"], false);
        // Review round 2: bare hex without `plugin` is no longer read as a runtime ID.
        args["form"] = json!("00000801");
        let err = execute("inspect", args.clone()).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(
            err.to_string().contains("needs `plugin`") && err.to_string().contains("runtime:"),
            "{err}"
        );
        args["form"] = json!("runtime:zz");
        assert_eq!(
            execute("inspect", args.clone()).unwrap_err().kind(),
            "failed"
        );
        args["form"] = json!("RUNTIME:00000FFF");
        let err = execute("inspect", args.clone()).unwrap_err();
        assert!(err.to_string().contains("no form has runtime ID"), "{err}");
        args["form"] = json!("800");
        args["plugin"] = json!("Patch.esm");
        let r = execute("inspect", args.clone()).unwrap();
        assert_eq!(r["data"]["matched_by"], "form_id");
        assert_eq!(r["data"]["global"], "Base.esm:000800");
        args["form"] = json!("Nothing");
        assert_eq!(execute("inspect", args).unwrap_err().kind(), "failed");
    }

    #[test]
    fn inspect_uses_the_session_load_order() {
        use crate::handlers::load::tests::{fixture, plugins_args};
        let dir = fixture("inspect-session");
        let engine = crate::Engine::with_default_handlers();
        // Nothing loaded and no plugin: refused.
        let err = engine
            .execute("inspect", json!({"form": "Pistol"}))
            .unwrap_err();
        assert!(err.to_string().contains("no load order"), "{err}");
        engine.execute("load", plugins_args(&dir)).unwrap();
        let r = engine
            .execute("inspect", json!({"form": "Pistol"}))
            .unwrap();
        assert_eq!(r["data"]["winner"], "Patch.esm");
    }

    /// Review round 2: a raw FormID 0 is the null reference (never `Base.esm:000000`), and a
    /// deleted winner is flagged on the version, in the summary and in `query`.
    #[test]
    fn null_formids_and_deleted_winners() {
        use crate::handlers::load::tests::plugin_flagged;
        use crate::handlers::testdata::sub;
        let dir = std::env::temp_dir().join(format!(
            "kitkraft-engine-lo-null-deleted-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut kwda = 0u32.to_le_bytes().to_vec();
        kwda.extend(0x802u32.to_le_bytes());
        let weap = [
            sub(b"EDID", b"Gun\0"),
            sub(b"KSIZ", &2u32.to_le_bytes()),
            sub(b"KWDA", &kwda),
        ]
        .concat();
        std::fs::write(
            dir.join("Base.esm"),
            plugin_flagged(
                0x1,
                &[],
                &[
                    (b"WEAP", 0, 0x800, weap.clone()),
                    (b"KYWD", 0, 0x802, sub(b"EDID", b"Kw\0")),
                ],
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("Patch.esm"),
            plugin_flagged(0x1, &["Base.esm"], &[(b"WEAP", 0x20, 0x800, weap)]),
        )
        .unwrap();
        let mut args = json!({
            "plugins": ["Base.esm", "Patch.esm"],
            "data_dir": dir.display().to_string(),
            "no_cache": true,
            "form": "Base.esm:000800",
        });
        let r = execute("inspect", args.clone()).unwrap();
        let data = &r["data"];
        let kwda = data["versions"][0]["subrecords"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["signature"] == "KWDA")
            .unwrap()["value"]
            .clone();
        assert_eq!(
            kwda,
            json!([
                {"kind": "formref", "raw": "00000000", "form": null, "null": true},
                {"kind": "formref", "raw": "00000802", "form": "Base.esm:000802"}
            ])
        );
        assert_eq!(data["versions"][0]["deleted"], false);
        assert_eq!(data["versions"][1]["deleted"], true);
        assert_eq!(data["winner_deleted"], true);
        assert!(data["summary"]
            .as_str()
            .unwrap()
            .contains("Patch.esm (deleted)"));
        assert!(r["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("winning version is deleted")));
        // Object 0 is no formref.
        args["form"] = json!("Base.esm:000000");
        assert_eq!(
            execute("inspect", args.clone()).unwrap_err().kind(),
            "failed"
        );

        // query: `null` finds the null keyword in the base version; the deleted winner is
        // unknown (its payload is not evaluated), never silently matched or rejected.
        let mut q = args.clone();
        q.as_object_mut().unwrap().remove("form");
        q["type"] = json!("WEAP");
        q["where"] = json!("KWDA == null");
        q["scope"] = json!("all");
        let r = execute("query", q.clone()).unwrap();
        assert_eq!(r["data"]["matched"], 1);
        assert_eq!(r["data"]["results"][0]["plugin"], "Base.esm");
        assert_eq!(r["data"]["results"][0]["deleted"], false);
        assert_eq!(r["data"]["unknown"], 1);
        q["scope"] = json!("winner");
        let r = execute("query", q.clone()).unwrap();
        assert_eq!(r["data"]["matched"], 0);
        assert_eq!(r["data"]["unknown"], 1);
        assert!(r["data"]["unknown_records"][0]["reasons"][0]
            .as_str()
            .unwrap()
            .contains("deleted"));
        // Without `where`, the deleted winner is listed and flagged.
        q.as_object_mut().unwrap().remove("where");
        let r = execute("query", q).unwrap();
        assert_eq!(r["data"]["results"][0]["deleted"], true);
        assert!(r["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("deleted versions")));
    }

    /// KK-222: `inspect --full` of an `ARMO` attaches its `Addon` entries as an `addons`
    /// key on the version object, each with the resolved `ARMA` and its four model paths.
    #[test]
    fn an_armo_inspect_shows_addons_and_their_model_paths() {
        use crate::edit::patch_tests::dir;
        use crate::handlers::place_tests::world_master;

        let d = dir("armo-inspect");
        let data = world_master(&d);
        let plugin = d.join("KC_Armor.esp");
        // The addon's `ARMA` takes the appended plugin's first new form.
        let r = execute(
            "new",
            json!({
                "type": "ARMA", "edid": "KC_Addon",
                "fields": {
                    "MOD2": "meshes/armor/kc/male.nif",
                    "MOD3": "meshes/armor/kc/female.nif",
                    "MOD4": "meshes/armor/kc/male_1p.nif",
                    "MOD5": "meshes/armor/kc/female_1p.nif",
                },
                "masters": ["World.esm"],
                "into": plugin.display().to_string(),
                "plugins": ["World.esm"], "data_dir": data.display().to_string(), "no_cache": true,
            }),
        )
        .unwrap();
        let arma_form_id = r["data"]["new_forms"][0]["form_id"].as_str().unwrap();
        // The `ARMO` with one `Addon` pointing at it.
        execute(
            "new",
            json!({
                "type": "ARMO", "edid": "KC_Outfit",
                "fields": {"INDX[0]": 0, "MODL[0]": arma_form_id},
                "masters": ["World.esm"],
                "into": plugin.display().to_string(),
                "plugins": ["World.esm"], "data_dir": data.display().to_string(), "no_cache": true,
            }),
        )
        .unwrap();
        let plugins = json!(["World.esm", plugin.display().to_string()]);
        let r = execute(
            "inspect",
            json!({
                "form": "KC_Outfit", "plugins": plugins,
                "data_dir": data.display().to_string(), "no_cache": true, "full": true,
            }),
        )
        .unwrap();
        let version = &r["data"]["versions"][0];
        assert_eq!(version["record"]["signature"], "ARMO");
        let addons = version["addons"].as_array().unwrap();
        assert_eq!(addons.len(), 1);
        let addon = &addons[0];
        assert_eq!(addon["index"], 0);
        assert_eq!(addon["arma"]["formid"], format!("0x{arma_form_id}"));
        assert_eq!(addon["arma"]["editor_id"], "KC_Addon");
        assert_eq!(addon["arma"]["plugin"], "KC_Armor.esp");
        assert_eq!(
            addon["models"],
            json!({
                "male": "meshes/armor/kc/male.nif",
                "female": "meshes/armor/kc/female.nif",
                "male_first_person": "meshes/armor/kc/male_1p.nif",
                "female_first_person": "meshes/armor/kc/female_1p.nif",
            })
        );
        // Without `full` the key is not there: the same gate as `lmsw`.
        let r = execute(
            "inspect",
            json!({
                "form": "KC_Outfit", "plugins": plugins,
                "data_dir": data.display().to_string(), "no_cache": true,
            }),
        )
        .unwrap();
        assert!(r["data"]["versions"][0].get("addons").is_none());
        let _ = std::fs::remove_dir_all(&d);
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
