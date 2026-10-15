//! `cargo xtask oracle sf1edit` (M3.H4, SF1Edit half; groundwork for M3.B11): xEdit's own
//! decoded view of a plugin, compared with ours.
//!
//! xEdit ships a console dumper next to its editor (`xDump64.exe`, the same build). Run in
//! Starfield mode it loads a plugin and its masters and prints every record as an indented
//! tree of `Name: value` lines, the way the editor displays them; `-dg:<SIG,...>` limits the
//! dump to some top groups and `-check` prints the editor's own error check instead. The
//! editor itself (`xSFEdit64.exe -script:...`) cannot run headless: in script mode it always
//! opens its module selection window (see `docs/oracles.md`). So the harness runs the dumper
//! as a program, on a scratch copy of the plugin and its masters (the game's `Data` folder is
//! only read), reads the text it prints, and compares it with `kit query` / `kit inspect`:
//!
//! - records: which FormIDs each side sees, signatures and EditorIDs;
//! - per subrecord occurrence of a sampled record: a member we decode (`value` in `kit
//!   inspect`) is compared with xEdit's displayed values after converting both to the same
//!   kind (text, number, lstring ID or text, FormID mapped to the defining file); a member we
//!   keep opaque is a coverage gap when xEdit shows named, typed fields for it, and "both
//!   unknown" when xEdit shows it as unknown too;
//! - what xEdit itself shows as unknown, per `TYPE/SUB`;
//! - with `--check` (default on), xEdit's own error check, findings grouped by message.
//!
//! Nothing of xEdit's source is read; its output is evidence about xEdit (docs/oracles.md).
//! Reports go to `--out` (default `observe/oracle/sf1edit/`, gitignored).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use kitkraft_schema::FieldType;
use serde_json::{json, Map, Value};

use crate::evidence::{arg, sha256_hex, table, tes4, KitBinary, KitOut, Runner};
use crate::oracle::{write_synthetic, Input, OFFICIAL_SMALL};

pub(crate) const SF1EDIT_VAR: &str = "KITKRAFT_SF1EDIT";
const DATA_DIR_VAR: &str = "KITKRAFT_DATA_DIR";
/// The console dumper xEdit ships next to its editor executables.
const DUMPER: &str = "xDump64.exe";
/// Default records compared in depth per record type (evenly spaced by FormID).
const DEFAULT_SAMPLE: usize = 25;
/// Disagreement and gap examples kept per `TYPE/SUB`.
const EXAMPLES: usize = 3;

// ---------------------------------------------------------------------------------------
// The dump: xEdit's text, parsed.

/// One displayed value under a subrecord: its path below the subrecord (`""` for the
/// subrecord line itself) and the value text, if the line has one.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct XLeaf {
    pub path: String,
    pub value: Option<String>,
}

/// One subrecord as xEdit shows it (`SIG - Name`), with its leaves in display order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct XNode {
    pub sig: String,
    /// 0 for the first `sig` in the record, 1 for the second, ...
    pub occurrence: usize,
    /// The name after `SIG - `.
    pub name: String,
    pub leaves: Vec<XLeaf>,
}

/// One record of the dump.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct XRecord {
    pub sig: String,
    /// The FormID as xEdit displays it (load-order numbering).
    pub form_id: u32,
    pub editor_id: Option<String>,
    pub nodes: Vec<XNode>,
}

fn indent_of(line: &str) -> usize {
    line.len()
        .saturating_sub(line.trim_start_matches(' ').len())
}

/// `SIG - Name` -> `(SIG, Name)`.
fn sig_node(name: &str) -> Option<(&str, &str)> {
    let sig = name.get(0..4)?;
    let rest = name.get(4..)?.strip_prefix(" - ")?;
    sig.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
        .then_some((sig, rest))
}

/// The name given to a subrecord xEdit lists by its bare signature: one it did not place in
/// its definition of the record (after an element it could not decode, its dump lists the
/// remaining subrecords that way).
pub(crate) const UNDECODED: &str = "Unknown (listed by signature only)";

fn is_bare_sig(name: &str) -> bool {
    name.len() == 4
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// The first `[XXXXXXXX]` (8 hex digits) in `text`.
pub(crate) fn bracket_form(text: &str) -> Option<u32> {
    let mut rest = text;
    while let Some(i) = rest.find('[') {
        let after = rest.get(i.saturating_add(1)..)?;
        if let Some(hex) = after.get(0..8) {
            if after.get(8..9) == Some("]") && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return u32::from_str_radix(hex, 16).ok();
            }
        }
        rest = after;
    }
    None
}

/// Splits `Name: value` (`Name [S]: value` for a struct) into name and value.
fn split_line(text: &str) -> (String, Option<String>) {
    let (name, value) = match text.find(": ") {
        Some(i) => (
            text.get(..i).unwrap_or(text),
            text.get(i.saturating_add(2)..).map(str::to_string),
        ),
        None => (text.strip_suffix(':').unwrap_or(text), None),
    };
    let name = name.strip_suffix(" [S]").unwrap_or(name);
    (name.to_string(), value)
}

struct Frame {
    depth: usize,
    node: Option<usize>,
    path: String,
    children: HashMap<String, usize>,
}

/// Builds one record from its lines: `(depth below the record, text)`; line 0 is the
/// `Record Header` line.
fn build_record(lines: &[(usize, &str)]) -> Option<XRecord> {
    let (_, head) = lines.first()?;
    let (_, head_value) = split_line(head);
    let head_value = head_value.unwrap_or_default();
    let form_id = bracket_form(head_value.split_once("] ").map_or("", |(_, r)| r)).unwrap_or(0);
    let mut sig = head_value
        .split_once("] ")
        .and_then(|(_, r)| r.get(0..4))
        .unwrap_or("????")
        .to_string();
    let editor_id = head_value
        .split_once('<')
        .and_then(|(_, r)| r.split_once('>'))
        .map(|(e, _)| e.to_string());
    let mut rec = XRecord {
        sig: String::new(),
        form_id,
        editor_id,
        nodes: Vec::new(),
    };
    let mut sig_count: HashMap<String, usize> = HashMap::new();
    let mut root_children: HashMap<String, usize> = HashMap::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut in_header = true;
    for (i, (depth, text)) in lines.iter().enumerate().skip(1) {
        let depth = *depth;
        if in_header {
            if depth > 0 {
                if let Some(s) = text.strip_prefix("Signature: ") {
                    sig = s.trim().to_string();
                }
                continue;
            }
            in_header = false;
        }
        let leaf = lines
            .get(i.saturating_add(1))
            .is_none_or(|(next, _)| *next <= depth);
        while stack.last().is_some_and(|f| f.depth >= depth) {
            stack.pop();
        }
        let (raw_name, value) = split_line(text);
        let children = match stack.last_mut() {
            Some(f) => &mut f.children,
            None => &mut root_children,
        };
        let seen = children.entry(raw_name.clone()).or_insert(0);
        let name = if *seen == 0 {
            raw_name.clone()
        } else {
            format!("{raw_name}#{seen}")
        };
        *seen = seen.saturating_add(1);
        let parent_node = stack.last().and_then(|f| f.node);
        let parent_path = stack.last().map(|f| f.path.clone()).unwrap_or_default();
        let bare = (depth == 0 && value.is_none() && is_bare_sig(&raw_name))
            .then_some((raw_name.as_str(), UNDECODED));
        let (node, path) = if let Some((s, rest)) = sig_node(&raw_name).or(bare) {
            let occ = sig_count.entry(s.to_string()).or_insert(0);
            rec.nodes.push(XNode {
                sig: s.to_string(),
                occurrence: *occ,
                name: rest.to_string(),
                leaves: Vec::new(),
            });
            *occ = occ.saturating_add(1);
            (rec.nodes.len().checked_sub(1), String::new())
        } else if parent_node.is_some() {
            let p = if parent_path.is_empty() {
                name.clone()
            } else {
                format!("{parent_path}\\{name}")
            };
            (parent_node, p)
        } else {
            (None, String::new())
        };
        if leaf {
            if let Some(n) = node.and_then(|n| rec.nodes.get_mut(n)) {
                n.leaves.push(XLeaf {
                    path: path.clone(),
                    value,
                });
            }
        }
        stack.push(Frame {
            depth,
            node,
            path,
            children: HashMap::new(),
        });
    }
    rec.sig = sig;
    Some(rec)
}

fn flush_record(cur: &mut Vec<(usize, &str)>, out: &mut Vec<XRecord>) {
    if !cur.is_empty() {
        out.extend(build_record(cur));
        cur.clear();
    }
}

/// Parses the text `xDump` printed: every record with its subrecords.
pub(crate) fn parse_dump(text: &str) -> Vec<XRecord> {
    let mut out = Vec::new();
    let mut current: Vec<(usize, &str)> = Vec::new();
    let mut base = 0usize;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let ind = indent_of(line);
        let body = line.get(ind..).unwrap_or_default();
        if body.starts_with("Record Header [S]:") || body.starts_with("Record Header:") {
            flush_record(&mut current, &mut out);
            base = ind;
            current.push((0, body));
        } else if body.starts_with("GRUP ")
            || body.starts_with("[Type]")
            || ind < base
            || current.is_empty()
        {
            flush_record(&mut current, &mut out);
        } else {
            current.push((ind.saturating_sub(base) / 2, body));
        }
    }
    flush_record(&mut current, &mut out);
    out
}

/// xEdit's `-check` output: findings grouped by message (FormIDs and numbers masked).
pub(crate) fn parse_check(text: &str) -> Value {
    let mut by_message: BTreeMap<String, u64> = BTreeMap::new();
    let mut examples: Vec<String> = Vec::new();
    let mut total = 0u64;
    for line in text.lines() {
        let Some((_, msg)) = line.split_once(" -> ") else {
            continue;
        };
        total = total.saturating_add(1);
        let masked: String = mask_hex(msg.trim());
        let n = by_message.entry(masked).or_insert(0);
        *n = n.saturating_add(1);
        if examples.len() < 5 {
            examples.push(line.trim().to_string());
        }
    }
    json!({"findings": total, "by_message": by_message, "examples": examples})
}

/// Replaces runs of hex digits of length >= 6 (FormIDs) and decimal numbers by `#`.
fn mask_hex(text: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let hexish = word.len() >= 6 && word.chars().all(|c| c.is_ascii_hexdigit());
        let num = !word.is_empty() && word.chars().all(|c| c.is_ascii_digit());
        if hexish || num {
            out.push('#');
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

// ---------------------------------------------------------------------------------------
// Keys: one record or FormID, as (defining file, object ID).

/// A FormID resolved to the file that defines it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum FormKey {
    Null,
    Key(String, u32),
}

/// How xEdit's load-order FormIDs map back to files for one dumped plugin: its masters in
/// order, then the plugin itself (a light plugin's own records show as `FE000xxx`).
#[derive(Debug, Clone)]
pub(crate) struct LoadMap {
    pub masters: Vec<String>,
    pub this: String,
}

impl LoadMap {
    pub(crate) fn xedit(&self, id: u32) -> FormKey {
        if id == 0 {
            return FormKey::Null;
        }
        let top = id >> 24;
        if top == 0xFE {
            return FormKey::Key(self.this.to_ascii_lowercase(), id & 0xFFF);
        }
        let file = usize::try_from(top)
            .ok()
            .and_then(|i| self.masters.get(i))
            .unwrap_or(&self.this);
        FormKey::Key(file.to_ascii_lowercase(), id & 0x00FF_FFFF)
    }
}

/// `Plugin.esm:XXXXXX` (ours) -> key.
pub(crate) fn our_key(form: &str) -> Option<FormKey> {
    let (file, obj) = form.rsplit_once(':')?;
    let obj = u32::from_str_radix(obj, 16).ok()?;
    Some(FormKey::Key(file.to_ascii_lowercase(), obj))
}

// ---------------------------------------------------------------------------------------
// Comparing values.

/// One scalar of a value we decoded.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Ours {
    Text(String),
    Num(f64),
    Bool(bool),
    LString(u32, Option<String>),
    Form(Option<FormKey>),
    /// An enum value (`key` names the field: `TYPE/SUB.Field...`), with our name for it.
    Enum {
        key: String,
        n: i128,
        name: Option<String>,
    },
    /// A flags value with our names of the bits it sets (unnamed bits are not listed).
    Flags {
        key: String,
        n: i128,
        names: Vec<String>,
    },
    /// A run of bytes (an array of `u8`): shown by xEdit as text, a GUID or hex bytes.
    Bytes(Vec<u8>),
    /// An integer of a typed field we give no names (`key` as for [`Ours::Enum`]): xEdit
    /// may show it as a name (a condition function, an operator), compared like an enum.
    Code {
        key: String,
        n: i128,
    },
    /// A field we keep opaque: it holds a place in the display order and matches anything.
    Any,
}

fn hex_u32(s: &str) -> Option<u32> {
    u32::from_str_radix(
        s.trim().trim_start_matches("0x").trim_start_matches("0X"),
        16,
    )
    .ok()
}

/// Flattens a `value` from `kit inspect` into scalars without its schema type; `ordered`
/// is cleared when an object (whose key order is not the display order) was crossed.
pub(crate) fn flatten(v: &Value, text: Option<&str>, out: &mut Vec<Ours>, ordered: &mut bool) {
    match v {
        Value::String(s) if s == "opaque" => out.push(Ours::Any),
        Value::String(s) => out.push(Ours::Text(s.clone())),
        Value::Number(n) => out.extend(n.as_f64().map(Ours::Num)),
        Value::Bool(b) => out.push(Ours::Bool(*b)),
        Value::Null => {}
        Value::Array(items) => {
            for i in items {
                flatten(i, None, out, ordered);
            }
        }
        Value::Object(map) => {
            if let Some(o) = scalar_object(map, text) {
                out.push(o);
            } else {
                *ordered = false;
                for i in map.values() {
                    flatten(i, None, out, ordered);
                }
            }
        }
    }
}

/// An lstring (`string_id`) or FormID (`raw`, `form`) object as one scalar.
fn scalar_object(map: &Map<String, Value>, text: Option<&str>) -> Option<Ours> {
    // M4.W0: `inspect` marks undecoded bytes `{"kind": "opaque", "opaque": {range}}` (it
    // was the string "opaque", which the callers still accept).
    if map.get("kind").and_then(Value::as_str) == Some("opaque") {
        return Some(Ours::Any);
    }
    if let Some(id) = map.get("string_id").and_then(Value::as_str) {
        return Some(Ours::LString(
            hex_u32(id).unwrap_or(u32::MAX),
            text.map(str::to_string),
        ));
    }
    let raw = map.get("raw").and_then(Value::as_str)?;
    let key = if hex_u32(raw) == Some(0) {
        Some(FormKey::Null)
    } else {
        map.get("form").and_then(Value::as_str).and_then(our_key)
    };
    Some(Ours::Form(key))
}

/// Flattens a `value` along its schema type, in definition order: enums and flags carry
/// their names, `u8` arrays become bytes, opaque fields keep their place as [`Ours::Any`].
/// Returns false (and pushes nothing) when the value's shape is not the type's; the caller
/// then falls back to [`flatten`].
pub(crate) fn typed_flatten(
    v: &Value,
    ty: &FieldType,
    key: &str,
    text: Option<&str>,
    out: &mut Vec<Ours>,
) -> bool {
    let mut local = Vec::new();
    let ok = typed_walk(v, ty, key, text, &mut local, 0);
    if ok {
        out.extend(local);
    }
    ok
}

fn typed_walk(
    v: &Value,
    ty: &FieldType,
    key: &str,
    text: Option<&str>,
    out: &mut Vec<Ours>,
    depth: usize,
) -> bool {
    if depth > 16 {
        return false;
    }
    let deeper = depth.saturating_add(1);
    match ty {
        FieldType::Opaque { .. } => {
            out.push(Ours::Any);
            true
        }
        FieldType::Empty => true,
        FieldType::Ref { ty, .. } => typed_walk(v, ty, key, text, out, deeper),
        FieldType::Enum { values, .. } => match v.as_i64() {
            Some(n) => {
                let n = i128::from(n);
                out.push(Ours::Enum {
                    key: key.to_string(),
                    n,
                    name: values.iter().find(|x| x.value == n).map(|x| x.name.clone()),
                });
                true
            }
            None => false,
        },
        FieldType::Flags { bits, .. } => match v.as_u64() {
            Some(n) => {
                let n = i128::from(n);
                out.push(Ours::Flags {
                    key: key.to_string(),
                    n,
                    names: bits
                        .iter()
                        .filter(|b| b.value != 0 && n & b.value == b.value)
                        .map(|b| b.name.clone())
                        .collect(),
                });
                true
            }
            None => false,
        },
        FieldType::Array { element, .. } => {
            let Some(items) = v.as_array() else {
                return false;
            };
            let byte_elem = matches!(
                element.as_ref(),
                FieldType::Scalar(kitkraft_schema::Scalar::U8 | kitkraft_schema::Scalar::I8)
            );
            if byte_elem {
                let bytes: Option<Vec<u8>> = items
                    .iter()
                    .map(|i| i.as_i64().and_then(|n| u8::try_from(n & 0xFF).ok()))
                    .collect();
                return match bytes {
                    Some(b) => {
                        // An empty run has nothing to compare (xEdit may not show it).
                        if !b.is_empty() {
                            out.push(Ours::Bytes(b));
                        }
                        true
                    }
                    None => false,
                };
            }
            items
                .iter()
                .all(|i| typed_walk(i, element, key, None, out, deeper))
        }
        FieldType::Struct { fields } => {
            let Some(map) = v.as_object() else {
                return false;
            };
            for f in fields {
                if let Some(fv) = map.get(&f.name) {
                    let k = format!("{key}.{}", f.name);
                    if !typed_walk(fv, &f.ty, &k, None, out, deeper) {
                        return false;
                    }
                }
            }
            true
        }
        FieldType::Union { variants, .. } => {
            // A decided union is shown as `{"<Variant>": value}`.
            let Some((name, inner)) = v.as_object().and_then(|m| {
                let mut it = m.iter();
                let first = it.next();
                it.next().is_none().then_some(first).flatten()
            }) else {
                return false;
            };
            match variants.iter().find(|x| x.name == *name) {
                Some(var) => {
                    typed_walk(inner, &var.ty, &format!("{key}.{name}"), text, out, deeper)
                }
                None => false,
            }
        }
        FieldType::Scalar(_)
        | FieldType::FormId { .. }
        | FieldType::Sig
        | FieldType::ZString
        | FieldType::LString
        | FieldType::WString
        | FieldType::BString => match v {
            Value::Object(map) => match scalar_object(map, text) {
                Some(o) => {
                    out.push(o);
                    true
                }
                None => false,
            },
            Value::Array(_) => false,
            Value::String(s) if s == "opaque" => {
                out.push(Ours::Any);
                true
            }
            Value::String(s) if matches!(ty, FieldType::Sig) => {
                out.push(Ours::Text(s.trim_end_matches('\0').to_string()));
                true
            }
            Value::Number(num) if matches!(ty, FieldType::Scalar(s) if s.is_integer()) => {
                match num.as_i64() {
                    Some(n) => out.push(Ours::Code {
                        key: key.to_string(),
                        n: i128::from(n),
                    }),
                    None => out.extend(num.as_f64().map(Ours::Num)),
                }
                true
            }
            other => {
                let mut ordered = true;
                flatten(other, text, out, &mut ordered);
                true
            }
        },
    }
}

fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    t.parse::<f64>().ok().or_else(|| {
        (t.len() == 8 && t.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(t, 16).ok().map(f64::from))
            .flatten()
    })
}

/// xEdit's raw-bytes display (`0A 1B 2C`), as bytes.
fn raw_bytes(v: &str) -> Option<Vec<u8>> {
    let parts: Vec<&str> = v.split(' ').collect();
    if parts.is_empty()
        || !parts
            .iter()
            .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return None;
    }
    parts
        .iter()
        .map(|p| u8::from_str_radix(p, 16).ok())
        .collect()
}

/// The text of a GUID the way xEdit shows it (`{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`):
/// the first three groups little-endian, the last two in byte order.
pub(crate) fn guid_text(b: &[u8]) -> Option<String> {
    let b: &[u8; 16] = b.try_into().ok()?;
    let [a0, a1, a2, a3, b0, b1, c0, c1, d @ ..] = *b;
    let mut s = format!("{{{a3:02X}{a2:02X}{a1:02X}{a0:02X}-{b1:02X}{b0:02X}-{c1:02X}{c0:02X}-");
    for (i, x) in d.iter().enumerate() {
        if i == 2 {
            s.push('-');
        }
        let _ = write!(s, "{x:02X}");
    }
    s.push('}');
    Some(s)
}

/// Letters and digits only, lower case: `Single or Binary` and `SingleOrBinary` meet.
fn norm(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// `<Unknown: X>` -> `X` (how xEdit shows a value it has no name for).
fn unwrap_unknown(v: &str) -> Option<&str> {
    v.trim()
        .strip_prefix("<Unknown: ")
        .and_then(|r| r.strip_suffix('>'))
}

/// An enum or flags value whose name we and xEdit spell differently: `(field, value,
/// xEdit's text)`. Agreement then rests on the pairing being consistent over the run.
pub(crate) type NamePair = (String, i128, String);

/// How one of our scalars compares with one displayed value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Hit {
    Yes,
    No,
    /// Same field, names spelled differently: agreement if consistent (see [`settle`]).
    Pending(NamePair),
}

fn named_hit(key: &str, n: i128, ours: &[String], v: &str, flags: bool) -> Hit {
    let t = v.trim();
    if bracket_form(t).is_some() {
        // A FormID is never the display of an enum, flags or code value.
        return Hit::No;
    }
    let unknown_n = t.strip_prefix("Unknown ").and_then(parse_num);
    if let Some(x) = parse_num(t)
        .or_else(|| unwrap_unknown(t).and_then(parse_num))
        .or(unknown_n)
    {
        #[allow(clippy::cast_precision_loss)]
        let same = (x - n as f64).abs() < 0.5;
        return if same { Hit::Yes } else { Hit::No };
    }
    if t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("false") {
        return if (n == 0) == t.eq_ignore_ascii_case("false") && (n == 0 || n == 1) {
            Hit::Yes
        } else {
            Hit::No
        };
    }
    if t.is_empty() {
        return if n == 0 { Hit::Yes } else { Hit::No };
    }
    let mut theirs: Vec<String> = if flags {
        t.split(", ").map(norm).collect()
    } else {
        vec![norm(t)]
    };
    let mut mine: Vec<String> = ours.iter().map(|s| norm(s)).collect();
    theirs.sort();
    mine.sort();
    if !mine.is_empty() && theirs == mine {
        Hit::Yes
    } else {
        Hit::Pending((key.to_string(), n, t.to_string()))
    }
}

/// Compares our scalar with xEdit's displayed `value`.
pub(crate) fn judge(ours: &Ours, value: Option<&str>, map: &LoadMap) -> Hit {
    // The dumper prints a fixed-size string with its terminating NUL.
    let v = value.unwrap_or("").trim_end_matches('\0');
    let yes = |b: bool| if b { Hit::Yes } else { Hit::No };
    // Raw bytes that are all zero equal any zero, null or empty value of ours.
    if let Some(bytes) = raw_bytes(v).filter(|_| v.contains(' ')) {
        let zero = bytes.iter().all(|b| *b == 0);
        match ours {
            Ours::Num(n) => {
                let mut le = [0u8; 8];
                for (d, s) in le.iter_mut().zip(&bytes) {
                    *d = *s;
                }
                #[allow(clippy::cast_precision_loss)]
                let int = u64::from_le_bytes(le) as f64;
                return yes(bytes.len() <= 8 && (int == *n || (zero && *n == 0.0)));
            }
            Ours::Form(k) => return yes(zero && *k == Some(FormKey::Null)),
            Ours::Bool(b) => return yes(zero != *b),
            Ours::LString(id, _) => return yes(zero && *id == 0),
            Ours::Enum { n, .. } | Ours::Flags { n, .. } => return yes(zero && *n == 0),
            Ours::Code { n, .. } => {
                let mut le = [0u8; 8];
                for (d, s) in le.iter_mut().zip(&bytes) {
                    *d = *s;
                }
                return yes(bytes.len() <= 8 && i128::from(u64::from_le_bytes(le)) == *n);
            }
            Ours::Bytes(b) => return yes(*b == bytes),
            Ours::Text(t) => return yes(zero && t.is_empty()),
            Ours::Any => return Hit::Yes,
        }
    }
    match ours {
        Ours::Any => Hit::Yes,
        Ours::Text(t) => {
            let shown = unwrap_unknown(v).unwrap_or(v);
            yes(same_text(t, shown))
        }
        Ours::Num(n) if parse_num(v).is_some() => yes(parse_num(v).is_some_and(|x| {
            let diff = (x - n).abs();
            diff <= 1e-6 || diff <= n.abs() * 1e-6
        })),
        Ours::Bool(b) => yes(v.eq_ignore_ascii_case(if *b { "true" } else { "false" })),
        // A flag we read as the integer 0 or 1 that xEdit displays as a boolean.
        Ours::Num(n) if v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("true") => {
            yes((*n == 0.0) == v.eq_ignore_ascii_case("false") && (*n == 0.0 || *n == 1.0))
        }
        // The float maximum is shown as "Default" (a sentinel value).
        Ours::Num(n) if v.trim() == "Default" => yes(*n >= f64::from(f32::MAX)),
        // A zero we read as a number where xEdit shows a null FormID: the same bytes.
        Ours::Num(n) => yes(*n == 0.0 && bracket_form(v) == Some(0)),
        Ours::LString(id, text) => {
            if let Some(rest) = v.split("lstring ID ").nth(1) {
                return yes(hex_u32(rest.get(0..8).unwrap_or(rest)) == Some(*id));
            }
            if v.is_empty() {
                return yes(*id == 0 || text.as_deref() == Some(""));
            }
            yes(text.as_deref().is_some_and(|t| same_text(t, v)))
        }
        Ours::Form(key) => {
            let Some(key) = key else { return Hit::No };
            yes(bracket_form(v).is_some_and(|id| map.xedit(id) == *key))
        }
        Ours::Enum { key, n, name } => named_hit(key, *n, name.as_slice(), v, false),
        Ours::Flags { key, n, names } => named_hit(key, *n, names, v, true),
        Ours::Code { key, n } => {
            #[allow(clippy::cast_precision_loss)]
            let as_num = judge(&Ours::Num(*n as f64), value, map);
            if as_num == Hit::Yes {
                Hit::Yes
            } else if bracket_form(v).is_some() {
                // xEdit shows a FormID where we read a plain integer: a real difference.
                Hit::No
            } else {
                named_hit(key, *n, &[], v, false)
            }
        }
        Ours::Bytes(b) => {
            if v.starts_with('{') {
                return yes(guid_text(b).is_some_and(|g| g.eq_ignore_ascii_case(v.trim())));
            }
            let text: String = b
                .iter()
                .take_while(|c| **c != 0)
                .map(|c| char::from(*c))
                .collect();
            yes(text == v || text.trim() == v.trim())
        }
    }
}

/// Text equality as the dump can show it: whitespace at the ends aside, and for a text of
/// several lines its first line (the dumper prints a value's later lines as lines of their
/// own, which the parser cannot tell from fields).
fn same_text(ours: &str, shown: &str) -> bool {
    ours == shown
        || ours.trim() == shown.trim()
        || (ours.contains('\n')
            && ours
                .lines()
                .next()
                .is_some_and(|first| first.trim() == shown.trim()))
}

/// Whether xEdit's displayed `value` is the same as our scalar (a differently spelled name
/// counts as the same here; [`settle`] checks it over the run).
#[cfg(test)]
pub(crate) fn same(ours: &Ours, value: Option<&str>, map: &LoadMap) -> bool {
    !matches!(judge(ours, value, map), Hit::No)
}

/// Whether xEdit shows this leaf as unknown: an `Unknown` name, an `Unknown N` enum value, or
/// raw bytes (`0A 1B ...`).
pub(crate) fn leaf_unknown(node: &XNode, leaf: &XLeaf) -> bool {
    let name = if leaf.path.is_empty() {
        node.name.as_str()
    } else {
        leaf.path.rsplit('\\').next().unwrap_or("")
    };
    let raw = leaf
        .value
        .as_deref()
        .is_some_and(|v| v.contains(' ') && raw_bytes(v).is_some());
    name.starts_with("Unknown")
        || name.starts_with("Unused")
        || leaf
            .value
            .as_deref()
            .is_some_and(|v| v.starts_with("Unknown "))
        || raw
}

fn node_unknown(node: &XNode) -> bool {
    !node.leaves.is_empty() && node.leaves.iter().all(|l| leaf_unknown(node, l))
}

/// The outcome of comparing one subrecord: agreement, with the name pairs it rests on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Verdict {
    pub agree: bool,
    pub pairs: Vec<NamePair>,
    /// Agreement on every value xEdit shows, with fields of ours xEdit does not show (its
    /// older layout has fewer): counted apart from full agreement.
    pub fewer: bool,
    /// Our values with no counterpart, and xEdit's values left over (for the examples).
    pub unmatched: Vec<String>,
    pub left: Vec<String>,
}

/// Pairs `ours` with `values` position by position (equal lengths).
fn zip_judge(ours: &[&Ours], values: &[Option<&str>], map: &LoadMap) -> Option<Vec<NamePair>> {
    if ours.len() != values.len() {
        return None;
    }
    let mut pairs = Vec::new();
    for (o, v) in ours.iter().zip(values) {
        match judge(o, *v, map) {
            Hit::Yes => {}
            Hit::No => return None,
            Hit::Pending(p) => pairs.push(p),
        }
    }
    Some(pairs)
}

/// The best order-preserving alignment of our values with xEdit's leaves.
#[derive(Debug, Default)]
struct Alignment {
    pairs: Vec<NamePair>,
    /// Our values left without a leaf.
    skipped: Vec<usize>,
    /// Leaves used by a match.
    used: Vec<bool>,
}

/// Aligns `ours` in order with `values` (xEdit may show fields we keep opaque or do not
/// have between them, and may lack fields we have): an exact match costs nothing, a name
/// pair 1, a value of ours left without a leaf 1,000 (so a real mismatch is never hidden by
/// pairing other values instead). `None` when the lists are too large to align.
fn align(ours: &[&Ours], values: &[Option<&str>], map: &LoadMap) -> Option<Alignment> {
    const INF: u32 = u32::MAX;
    const SKIP: u32 = 1000;
    let (n, m) = (ours.len(), values.len());
    let w = m.checked_add(1)?;
    let cells = n.checked_add(1)?.checked_mul(w)?;
    if cells > 4_000_000 {
        return None;
    }
    let at = |i: usize, j: usize| i.checked_mul(w).and_then(|x| x.checked_add(j));
    let mut hits: Vec<Hit> = Vec::with_capacity(n.checked_mul(m)?);
    for o in ours {
        for v in values {
            hits.push(judge(o, *v, map));
        }
    }
    let hit = |i: usize, j: usize| {
        i.checked_mul(m)
            .and_then(|x| x.checked_add(j))
            .and_then(|k| hits.get(k))
    };
    let get = |cost: &[u32], i: usize, j: usize| {
        at(i, j).and_then(|k| cost.get(k)).copied().unwrap_or(INF)
    };
    // cost(i, j): the cheapest alignment of ours[i..] with values[j..].
    let mut cost = vec![INF; cells];
    for i in (0..=n).rev() {
        for j in (0..=m).rev() {
            let best = if i == n {
                0
            } else {
                let skip_ours = get(&cost, i.saturating_add(1), j).saturating_add(SKIP);
                if j == m {
                    skip_ours
                } else {
                    let next = get(&cost, i.saturating_add(1), j.saturating_add(1));
                    let take = match hit(i, j) {
                        Some(Hit::Yes) => next,
                        Some(Hit::Pending(_)) => next.saturating_add(1),
                        _ => INF,
                    };
                    take.min(get(&cost, i, j.saturating_add(1))).min(skip_ours)
                }
            };
            if let Some(c) = at(i, j).and_then(|k| cost.get_mut(k)) {
                *c = best;
            }
        }
    }
    let mut out = Alignment {
        used: vec![false; m],
        ..Alignment::default()
    };
    let (mut i, mut j) = (0usize, 0usize);
    while i < n {
        let here = get(&cost, i, j);
        if j < m {
            let next = get(&cost, i.saturating_add(1), j.saturating_add(1));
            let taken = match hit(i, j) {
                Some(Hit::Yes) => next == here,
                Some(Hit::Pending(_)) => next != INF && next.saturating_add(1) == here,
                _ => false,
            };
            if taken {
                if let Some(Hit::Pending(p)) = hit(i, j) {
                    out.pairs.push(p.clone());
                }
                if let Some(u) = out.used.get_mut(j) {
                    *u = true;
                }
                i = i.saturating_add(1);
                j = j.saturating_add(1);
                continue;
            }
            if get(&cost, i, j.saturating_add(1)) == here {
                j = j.saturating_add(1);
                continue;
            }
        }
        out.skipped.push(i);
        i = i.saturating_add(1);
    }
    Some(out)
}

/// Values of ours an aligned pass left over that xEdit shows in another order (its older
/// layout moves some fields): first exact matches against the unused leaves, then, when as
/// many enum, flags or code values of ours remain as unused named leaves, those in order as
/// name pairs (checked for consistency by [`settle`]).
fn out_of_order(ours: &[&Ours], values: &[Option<&str>], map: &LoadMap, a: &mut Alignment) {
    let mut still = Vec::new();
    for i in std::mem::take(&mut a.skipped) {
        let Some(o) = ours.get(i) else { continue };
        let hit = values.iter().enumerate().position(|(j, v)| {
            !a.used.get(j).copied().unwrap_or(true) && judge(o, *v, map) == Hit::Yes
        });
        match hit.and_then(|j| a.used.get_mut(j)) {
            Some(u) => *u = true,
            None => still.push(i),
        }
    }
    let named = |o: &Ours| {
        matches!(
            o,
            Ours::Enum { .. } | Ours::Flags { .. } | Ours::Code { .. }
        )
    };
    let free: Vec<usize> = values
        .iter()
        .enumerate()
        .filter(|(j, v)| {
            !a.used.get(*j).copied().unwrap_or(true)
                && v.is_some_and(|t| !t.trim().is_empty() && parse_num(t).is_none())
        })
        .map(|(j, _)| j)
        .collect();
    let all_named = still.iter().all(|i| ours.get(*i).is_some_and(|o| named(o)));
    if all_named && !still.is_empty() && still.len() == free.len() {
        let mut pairs = Vec::new();
        for (i, j) in still.iter().zip(&free) {
            match ours
                .get(*i)
                .map(|o| judge(o, values.get(*j).copied().flatten(), map))
            {
                Some(Hit::Pending(p)) => pairs.push(p),
                Some(Hit::Yes) => {}
                _ => {
                    a.skipped = still;
                    return;
                }
            }
        }
        for j in &free {
            if let Some(u) = a.used.get_mut(*j) {
                *u = true;
            }
        }
        a.pairs.extend(pairs);
        return;
    }
    a.skipped = still;
}

/// The leaves no value of ours was matched with, that xEdit decodes and shows with a value
/// (a leaf xEdit calls unknown, or an empty list header, is not a value we failed to match).
fn known_left(node: &XNode, used: &[bool]) -> Vec<String> {
    node.leaves
        .iter()
        .zip(used)
        .filter(|(l, u)| {
            !**u && !leaf_unknown(node, l)
                && l.value.as_deref().is_some_and(|v| !v.trim().is_empty())
        })
        .map(|(l, _)| {
            let p = if l.path.is_empty() {
                &node.name
            } else {
                &l.path
            };
            format!("{p}={}", l.value.as_deref().unwrap_or(""))
        })
        .collect()
}

/// Compares our scalars with the node's displayed leaves: position by position when both
/// have the same count, else in order against a subsequence of the leaves (typed values
/// only), else as a multiset (exact matches first, then differently spelled enum and flags
/// names).
pub(crate) fn compare(ours: &[Ours], ordered: bool, node: &XNode, map: &LoadMap) -> Verdict {
    let agree = |pairs| Verdict {
        agree: true,
        pairs,
        fewer: false,
        unmatched: Vec::new(),
        left: Vec::new(),
    };
    let all: Vec<&Ours> = ours.iter().collect();
    let decoded: Vec<&Ours> = ours.iter().filter(|o| **o != Ours::Any).collect();
    if decoded.is_empty() {
        return agree(Vec::new());
    }
    let values: Vec<Option<&str>> = node.leaves.iter().map(|l| l.value.as_deref()).collect();
    if ordered {
        if let Some(p) = zip_judge(&all, &values, map) {
            return agree(p);
        }
        if let Some(mut a) = align(&decoded, &values, map) {
            if a.skipped.is_empty() {
                return agree(a.pairs);
            }
            out_of_order(&decoded, &values, map, &mut a);
            if a.skipped.is_empty() {
                return agree(a.pairs);
            }
            let left = known_left(node, &a.used);
            let unmatched = a
                .skipped
                .iter()
                .filter_map(|i| decoded.get(*i))
                .map(|o| format!("{o:?}"))
                .collect();
            return Verdict {
                agree: left.is_empty(),
                fewer: left.is_empty(),
                pairs: a.pairs,
                unmatched,
                left,
            };
        }
    }
    let mut used = vec![false; values.len()];
    let mut waiting = Vec::new();
    for o in &decoded {
        let hit = values.iter().enumerate().find(|(i, v)| {
            !used.get(*i).copied().unwrap_or(true) && judge(o, **v, map) == Hit::Yes
        });
        match hit.and_then(|(i, _)| used.get_mut(i)) {
            Some(slot) => *slot = true,
            None => waiting.push(*o),
        }
    }
    let mut pairs = Vec::new();
    let mut unmatched = Vec::new();
    for o in waiting {
        let hit = values.iter().enumerate().find_map(|(i, v)| {
            if used.get(i).copied().unwrap_or(true) {
                return None;
            }
            match judge(o, *v, map) {
                // Out of display order only an enum's or flags' name may pair.
                Hit::Pending(p) if matches!(o, Ours::Enum { .. } | Ours::Flags { .. }) => {
                    Some((i, p))
                }
                _ => None,
            }
        });
        match hit {
            Some((i, p)) => {
                if let Some(slot) = used.get_mut(i) {
                    *slot = true;
                }
                pairs.push(p);
            }
            None => unmatched.push(format!("{o:?}")),
        }
    }
    if unmatched.is_empty() {
        return agree(pairs);
    }
    let left = known_left(node, &used);
    Verdict {
        agree: left.is_empty(),
        fewer: left.is_empty(),
        pairs,
        unmatched,
        left,
    }
}

fn bump(map: &mut Map<String, Value>, key: &str) {
    let n = map.get(key).and_then(Value::as_u64).unwrap_or(0);
    map.insert(key.to_string(), json!(n.saturating_add(1)));
}

fn unbump(map: &mut Map<String, Value>, key: &str) {
    let n = map.get(key).and_then(Value::as_u64).unwrap_or(0);
    map.insert(key.to_string(), json!(n.saturating_sub(1)));
}

/// Every leaf of a node as `path=value` (the full view kept in disagreement examples).
fn all_leaves(node: &XNode) -> Vec<String> {
    node.leaves
        .iter()
        .take(200)
        .map(|l| {
            let p = if l.path.is_empty() {
                &node.name
            } else {
                &l.path
            };
            format!("{p}={}", l.value.as_deref().unwrap_or(""))
        })
        .collect()
}

fn leaf_text(node: &XNode) -> String {
    let parts: Vec<String> = node
        .leaves
        .iter()
        .take(6)
        .map(|l| {
            let p = if l.path.is_empty() {
                &node.name
            } else {
                &l.path
            };
            format!("{p}={}", l.value.as_deref().unwrap_or(""))
        })
        .collect();
    let mut s = parts.join("; ");
    if node.leaves.len() > 6 {
        s.push_str("; ...");
    }
    s.chars().take(300).collect()
}

/// The schema types of one record type's subrecords, by `(signature, member name)` as
/// `kit inspect` names them.
pub(crate) type Layouts<'s> = HashMap<(String, String), &'s FieldType>;

/// [`Layouts`] for record type `sig` from the schema.
pub(crate) fn layouts<'s>(schema: &'s kitkraft_schema::Schema, sig: &str) -> Layouts<'s> {
    let mut out = HashMap::new();
    let Some(rec) = kitkraft_schema::Signature::parse(sig).and_then(|s| schema.record(s)) else {
        return out;
    };
    for (_, sub) in rec.leaf_subrecords() {
        out.entry((sub.signature.as_str().to_string(), sub.name.clone()))
            .or_insert(&sub.ty);
    }
    out
}

/// A compared occurrence whose agreement rests on name pairs (checked by [`settle`]).
#[derive(Debug, Clone)]
pub(crate) struct PendingAgree {
    pub sig: String,
    pub form: String,
    pub pairs: Vec<NamePair>,
    pub xedit: String,
    /// The class it was counted in (`agree` or `agree_fewer_fields`).
    pub class: &'static str,
}

/// What a comparison of one record type gathers.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    pub members: BTreeMap<String, Map<String, Value>>,
    pub examples: BTreeMap<String, Vec<Value>>,
    pub pending: Vec<PendingAgree>,
}

/// Compares one of our records (`kit inspect` subrecords of the plugin's version) with
/// xEdit's. Adds counts per `SUB`, examples, and the agreements that rest on name pairs.
/// `strings` resolves a string ID that `kit inspect` gave without text.
pub(crate) fn compare_record(
    form: &str,
    subrecords: &[Value],
    x: &XRecord,
    map: &LoadMap,
    layouts: &Layouts<'_>,
    strings: &mut dyn FnMut(u32) -> Option<String>,
    tally: &mut Tally,
) {
    let mut occ: HashMap<String, usize> = HashMap::new();
    let mut matched_nodes = vec![false; x.nodes.len()];
    for sr in subrecords {
        let sig = sr
            .get("signature")
            .and_then(Value::as_str)
            .unwrap_or("????")
            .to_string();
        let o = occ.entry(sig.clone()).or_insert(0);
        let this_occ = *o;
        *o = o.saturating_add(1);
        let node_ix = x
            .nodes
            .iter()
            .position(|n| n.sig == sig && n.occurrence == this_occ);
        if let Some(slot) = node_ix.and_then(|i| matched_nodes.get_mut(i)) {
            *slot = true;
        }
        let node = node_ix.and_then(|i| x.nodes.get(i));
        let counts = tally.members.entry(sig.clone()).or_default();
        let typed = sr.get("value");
        let class = match (typed, node) {
            (Some(_), None) => "typed_not_shown",
            (Some(v), Some(n)) => {
                if node_unknown(n) {
                    "typed_xedit_unknown"
                } else {
                    let text = sr.get("text").and_then(Value::as_str);
                    let member = sr.get("member").and_then(Value::as_str).unwrap_or("");
                    let mut flat = Vec::new();
                    let mut ordered = true;
                    let key = format!("{}/{sig}", x.sig);
                    let ty = layouts.get(&(sig.clone(), member.to_string()));
                    if !ty.is_some_and(|t| typed_flatten(v, t, &key, text, &mut flat)) {
                        flatten(v, text, &mut flat, &mut ordered);
                    }
                    for o in &mut flat {
                        if let Ours::LString(id, t @ None) = o {
                            if *id != 0 {
                                *t = strings(*id);
                            }
                        }
                    }
                    let verdict = compare(&flat, ordered, n, map);
                    let class = if verdict.fewer {
                        "agree_fewer_fields"
                    } else {
                        "agree"
                    };
                    if verdict.agree {
                        if verdict.fewer {
                            let ex = tally.examples.entry(format!("fewer {sig}")).or_default();
                            if ex.len() < EXAMPLES {
                                ex.push(json!({"form": form, "unmatched": verdict.unmatched, "xedit": leaf_text(n)}));
                            }
                        }
                        if !verdict.pairs.is_empty() {
                            tally.pending.push(PendingAgree {
                                sig: sig.clone(),
                                form: form.to_string(),
                                pairs: verdict.pairs,
                                xedit: leaf_text(n),
                                class,
                            });
                        }
                        class
                    } else {
                        let ex = tally.examples.entry(format!("disagree {sig}")).or_default();
                        if ex.len() < EXAMPLES {
                            ex.push(json!({"form": form, "ours": v, "text": sr.get("text"), "xedit": leaf_text(n), "xedit_leaves": all_leaves(n), "unmatched": verdict.unmatched, "xedit_left": verdict.left}));
                        }
                        "disagree"
                    }
                }
            }
            (None, None) => "opaque_not_shown",
            (None, Some(n)) => {
                if node_unknown(n) {
                    "both_unknown"
                } else {
                    let ex = tally.examples.entry(format!("gap {sig}")).or_default();
                    if ex.len() < EXAMPLES {
                        ex.push(json!({"form": form, "xedit_name": n.name, "xedit": leaf_text(n)}));
                    }
                    "gap"
                }
            }
        };
        bump(counts, class);
    }
    for (n, seen) in x.nodes.iter().zip(&matched_nodes) {
        if !seen {
            bump(
                tally.members.entry(n.sig.clone()).or_default(),
                "xedit_only",
            );
        }
    }
}

/// Checks the agreements that rest on differently spelled names: over the run, each of our
/// values of a field must always show as the same xEdit text, and that text must come from
/// that value only. An occurrence with a pair that breaks this becomes a disagreement.
/// Returns, per field, our value -> xEdit's text (the spelling differences, for the report).
pub(crate) fn settle(tally: &mut Tally) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut by_value: BTreeMap<(String, i128), BTreeSet<String>> = BTreeMap::new();
    let mut by_text: BTreeMap<(String, String), BTreeSet<i128>> = BTreeMap::new();
    for p in &tally.pending {
        for (k, n, t) in &p.pairs {
            by_value
                .entry((k.clone(), *n))
                .or_default()
                .insert(t.clone());
            by_text
                .entry((k.clone(), t.clone()))
                .or_default()
                .insert(*n);
        }
    }
    let broken = |(k, n, t): &NamePair| {
        by_value.get(&(k.clone(), *n)).is_some_and(|s| s.len() > 1)
            || by_text
                .get(&(k.clone(), t.clone()))
                .is_some_and(|s| s.len() > 1)
    };
    for p in &tally.pending {
        if p.pairs.iter().any(broken) {
            let counts = tally.members.entry(p.sig.clone()).or_default();
            unbump(counts, p.class);
            bump(counts, "disagree");
            let ex = tally
                .examples
                .entry(format!("disagree {}", p.sig))
                .or_default();
            if ex.len() < EXAMPLES {
                let bad: Vec<String> = p
                    .pairs
                    .iter()
                    .filter(|x| broken(x))
                    .map(|(k, n, t)| format!("{k} = {n} shown as `{t}`"))
                    .collect();
                ex.push(
                    json!({"form": p.form, "ours": bad.join("; "), "text": null, "xedit": p.xedit}),
                );
            }
        }
    }
    let mut names: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for ((k, n), texts) in &by_value {
        let joined: Vec<&str> = texts.iter().map(String::as_str).collect();
        names
            .entry(k.clone())
            .or_default()
            .insert(n.to_string(), joined.join(" | "));
    }
    names
}

// ---------------------------------------------------------------------------------------
// Running xDump.

struct Dumper {
    exe: PathBuf,
    timeout: Duration,
}

struct DumpRun {
    exit: Option<i32>,
    seconds: u64,
    timed_out: bool,
    stdout: PathBuf,
    stderr: PathBuf,
    stderr_tail: String,
}

impl Dumper {
    fn from_env() -> Result<Self> {
        let given = crate::env_var_os(SF1EDIT_VAR).map(PathBuf::from).with_context(|| {
            format!(
                "set {SF1EDIT_VAR} to xSFEdit64.exe (xEdit 4.1.5+; its {DUMPER} is used) or to {DUMPER}"
            )
        })?;
        let is_dumper = given
            .file_name()
            .is_some_and(|n| n.to_string_lossy().to_ascii_lowercase().contains("dump"));
        let exe = if is_dumper {
            given
        } else {
            given
                .parent()
                .map(|d| d.join(DUMPER))
                .context("the SF1Edit path has no folder")?
        };
        if !exe.is_file() {
            bail!(
                "{} not found ({SF1EDIT_VAR} names the xEdit folder's xSFEdit64.exe or {DUMPER})",
                exe.display()
            );
        }
        Ok(Self {
            exe,
            timeout: Duration::from_secs(20 * 60),
        })
    }

    /// Runs the dumper with `args`, stdout to `out`, killing it after the time cap.
    fn run(&self, args: &[String], out: &Path) -> Result<DumpRun> {
        let err_path = out.with_extension("err.txt");
        let stdout = fs::File::create(out)?;
        let stderr = fs::File::create(&err_path)?;
        let start = Instant::now();
        let mut child = Command::new(&self.exe)
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .with_context(|| format!("could not start {}", self.exe.display()))?;
        let mut timed_out = false;
        let status = loop {
            if let Some(s) = child.try_wait()? {
                break Some(s);
            }
            if start.elapsed() > self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                timed_out = true;
                break None;
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        let err = read_text(&err_path);
        let lines = || err.lines().map(str::trim);
        let tail = lines()
            .rfind(|l| l.contains("Error"))
            .or_else(|| lines().rfind(|l| !l.is_empty()))
            .unwrap_or_default()
            .chars()
            .take(300)
            .collect();
        Ok(DumpRun {
            exit: status.and_then(|s| s.code()),
            seconds: start.elapsed().as_secs(),
            timed_out,
            stdout: out.to_path_buf(),
            stderr: err_path,
            stderr_tail: tail,
        })
    }
}

/// Windows-1252 bytes 0x80..=0x9F (the dumper writes its general string encoding, 1252
/// here: "Using general string encoding: 1252" in its log); 0 marks the five unused codes.
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
    0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

/// Decodes the dumper's output: UTF-8 when it is valid, else Windows-1252.
pub(crate) fn decode_text(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => e
            .into_bytes()
            .iter()
            .map(|b| match b {
                0x80..=0x9F => CP1252_HIGH
                    .get(usize::from(b & 0x1F))
                    .and_then(|c| char::from_u32(u32::from(*c)))
                    .filter(|c| *c != '\0')
                    .unwrap_or('\u{FFFD}'),
                _ => char::from(*b),
            })
            .collect(),
    }
}

fn read_text(path: &Path) -> String {
    fs::read(path).map(decode_text).unwrap_or_default()
}

/// The dumper's banner (`SF1Dump 4.1.5f x64`) from the head of its log (stderr).
fn banner(path: &Path) -> Option<String> {
    let f = fs::File::open(path).ok()?;
    BufReader::new(f)
        .lines()
        .take(5)
        .map_while(Result::ok)
        .map(|l| l.trim().to_string())
        .find(|l| l.contains("Dump") && l.chars().any(|c| c.is_ascii_digit()))
}

// ---------------------------------------------------------------------------------------
// Asking kit: one `kit mcp` session for the many `inspect` calls of a run.

/// A `kit mcp` process: one engine whose session keeps the load order loaded, so each
/// `inspect` after the first costs milliseconds instead of a process start and an index
/// read. Spoken to over stdio, one JSON-RPC message per line (`docs/mcp.md`).
struct McpSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpSession {
    fn start(kit: &KitBinary, cache: &Path) -> Result<Self> {
        let mut child = kit
            .command(&["mcp"])
            .env("KITKRAFT_CACHE_DIR", cache)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("could not start `kit mcp`")?;
        let stdin = child.stdin.take().context("kit mcp: no stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("kit mcp: no stdout")?);
        let mut s = Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        };
        s.request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "xtask-oracle", "version": "0"}}),
        )?;
        s.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?;
        Ok(s)
    }

    fn send(&mut self, msg: &Value) -> Result<()> {
        let mut line = serde_json::to_string(msg)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line)? == 0 {
                bail!("kit mcp closed its output");
            }
            let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            if msg.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(e) = msg.get("error") {
                bail!("kit mcp {method}: {e}");
            }
            return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// Runs catalog command `command` with `args` through the `execute` tool.
    fn execute(&mut self, command: &str, args: Value) -> Result<KitOut> {
        let r = self.request(
            "tools/call",
            json!({"name": "execute", "arguments": {"command": command, "args": args}}),
        )?;
        let structured = r.get("structuredContent").cloned().unwrap_or(Value::Null);
        let json = if r.get("isError").and_then(Value::as_bool) == Some(true) {
            match structured.get("error") {
                Some(_) => structured,
                None => json!({"error": {"message": structured.to_string()}}),
            }
        } else {
            structured
        };
        Ok(KitOut {
            code: Some(0),
            json,
        })
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// How the harness asks `kit inspect`: through one `kit mcp` session when it starts and the
/// load order loads, else one `kit inspect` process per record.
enum Inspector<'a> {
    Session(McpSession),
    Cli {
        kit: &'a dyn Runner,
        chain: Vec<String>,
        data_dir: String,
        cache: PathBuf,
    },
}

impl<'a> Inspector<'a> {
    fn start(
        kit: &'a dyn Runner,
        session_kit: Option<&KitBinary>,
        chain: &[String],
        data_dir: &str,
        cache: &Path,
    ) -> Self {
        let session = session_kit.and_then(|k| {
            let mut s = McpSession::start(k, cache).ok()?;
            let r = s
                .execute("load", json!({"plugins": chain, "data_dir": data_dir}))
                .ok()?;
            r.error().is_none().then_some(s)
        });
        match session {
            Some(s) => Self::Session(s),
            None => Self::Cli {
                kit,
                chain: chain.to_vec(),
                data_dir: data_dir.to_string(),
                cache: cache.to_path_buf(),
            },
        }
    }

    fn via(&self) -> &'static str {
        match self {
            Self::Session(_) => "kit mcp session",
            Self::Cli { .. } => "kit inspect per record",
        }
    }

    fn inspect(&mut self, form: &str) -> Result<KitOut> {
        match self {
            Self::Session(s) => s.execute("inspect", json!({"form": form})),
            Self::Cli {
                kit,
                chain,
                data_dir,
                cache,
            } => {
                let mut a = vec![
                    "inspect".to_string(),
                    form.to_string(),
                    "--data-dir".into(),
                    data_dir.clone(),
                ];
                for p in chain.iter() {
                    a.push("--plugins".into());
                    a.push(p.clone());
                }
                kit.kit(&a, cache)
            }
        }
    }

    /// Every entry of one string table (`strings.dump`), or none.
    fn table(&mut self, archive: &Path, table: &str) -> Vec<(u32, String)> {
        let Ok(archive) = arg(archive) else {
            return Vec::new();
        };
        let r = match self {
            Self::Session(s) => s.execute(
                "strings.dump",
                json!({"table": table, "archive": archive, "limit": 10_000_000}),
            ),
            Self::Cli { kit, cache, .. } => kit.kit(
                &[
                    "strings".into(),
                    "dump".into(),
                    table.to_string(),
                    "--archive".into(),
                    archive,
                    "--limit".into(),
                    "10000000".into(),
                ],
                cache,
            ),
        };
        let Ok(r) = r else { return Vec::new() };
        r.data()
            .get("entries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let id = e.get("id").and_then(Value::as_str).and_then(hex_u32)?;
                let text = e.get("text").and_then(Value::as_str)?;
                Some((id, text.to_string()))
            })
            .collect()
    }
}

/// The plugin's string tables (`<stem>_en` in its localization or main archive), read on
/// the first string ID `kit inspect` left without text (an lstring inside a struct).
struct StringLookup {
    archive: Option<PathBuf>,
    stem: String,
    map: Option<HashMap<u32, String>>,
}

impl StringLookup {
    fn new(archive: Option<PathBuf>, stem: &str) -> Self {
        Self {
            archive,
            stem: stem.to_ascii_lowercase(),
            map: None,
        }
    }

    fn get(&mut self, inspector: &mut Inspector<'_>, id: u32) -> Option<String> {
        if self.map.is_none() {
            let mut map = HashMap::new();
            if let Some(archive) = &self.archive {
                for kind in ["strings", "dlstrings", "ilstrings"] {
                    let table = format!("strings/{}_en.{kind}", self.stem);
                    for (k, v) in inspector.table(archive, &table) {
                        map.entry(k).or_insert(v);
                    }
                }
            }
            self.map = Some(map);
        }
        self.map.as_ref().and_then(|m| m.get(&id).cloned())
    }
}

// ---------------------------------------------------------------------------------------
// The run.

struct Opts {
    data_dir: PathBuf,
    work: PathBuf,
    types: Vec<String>,
    sample: usize,
    check: bool,
}

/// Copies `name` (from `from_dir`, else the game's Data) and, recursively, its masters and
/// its localization archive into the scratch `Data`; returns the plugin's masters.
fn stage(name: &str, from_dir: &Path, opts: &Opts, depth: usize) -> Result<Vec<String>> {
    if depth > 8 {
        bail!("master chain deeper than 8 at {name}");
    }
    let scratch = opts.work.join("Data");
    fs::create_dir_all(&scratch)?;
    let src = [from_dir.join(name), opts.data_dir.join(name)]
        .into_iter()
        .find(|p| p.is_file())
        .with_context(|| format!("{name} not found next to the plugin or in {DATA_DIR_VAR}"))?;
    let dst = scratch.join(name);
    let same = fs::metadata(&dst)
        .ok()
        .zip(fs::metadata(&src).ok())
        .is_some_and(|(a, b)| a.len() == b.len());
    if !same {
        fs::copy(&src, &dst).with_context(|| format!("copying {}", src.display()))?;
    }
    let stem = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    // The string tables: `<name> - Localization.ba2` (Starfield, Constellation, OldMars), else
    // `<name> - Main.ba2` (the SFBGS add-ons keep theirs there).
    for suffix in ["Localization", "Main"] {
        let ba2 = format!("{stem} - {suffix}.ba2");
        let found = [from_dir, opts.data_dir.as_path()]
            .into_iter()
            .map(|d| d.join(&ba2))
            .find(|p| p.is_file());
        if let Some(p) = found {
            if !scratch.join(&ba2).is_file() {
                fs::copy(&p, scratch.join(&ba2))?;
            }
            break;
        }
    }
    let head = read_head(&dst)?;
    let masters = tes4(&head).map(|t| t.masters).unwrap_or_default();
    for m in &masters {
        stage(m, from_dir, opts, depth.saturating_add(1))?;
    }
    Ok(masters)
}

/// The first 64 KiB of a file (enough for `TES4`).
fn read_head(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read as _;
    let mut f = fs::File::open(path)?;
    let mut buf = vec![0u8; 65_536];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

/// One of our records: key, form (`Plugin.esm:XXXXXX`), signature, EditorID.
type OurRecord = (FormKey, String, String, Option<String>);

/// Our records of `name` in the load order `chain`.
fn our_records(
    kit: &dyn Runner,
    chain: &[String],
    name: &str,
    types: &[String],
    opts: &Opts,
) -> Result<Vec<OurRecord>> {
    let cache = opts.work.join("cache");
    let scratch = arg(&opts.work.join("Data"))?;
    let mut out = Vec::new();
    for ty in types {
        let mut cursor: Option<String> = None;
        loop {
            let mut a = vec![
                "query".to_string(),
                ty.clone(),
                "--scope".into(),
                "all".into(),
                "--limit".into(),
                "1000".into(),
                "--where".into(),
                format!("@plugin == \"{name}\""),
                "--data-dir".into(),
                scratch.clone(),
            ];
            for p in chain {
                a.push("--plugins".into());
                a.push(p.clone());
            }
            if let Some(c) = &cursor {
                a.push("--cursor".into());
                a.push(c.clone());
            }
            let r = kit.kit(&a, &cache)?;
            if let Some(e) = r.error() {
                bail!("kit query {ty}: {e}");
            }
            for rec in r
                .data()
                .get("results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let plugin = rec.get("plugin").and_then(Value::as_str).unwrap_or("");
                if !plugin.eq_ignore_ascii_case(name) {
                    continue;
                }
                let form = rec.get("form").and_then(Value::as_str).unwrap_or("");
                if let Some(k) = our_key(form) {
                    out.push((
                        k,
                        form.to_string(),
                        rec.get("signature")
                            .and_then(Value::as_str)
                            .unwrap_or(ty)
                            .to_string(),
                        rec.get("editor_id")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    ));
                }
            }
            cursor = r
                .data()
                .get("next_cursor")
                .and_then(Value::as_str)
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
    }
    Ok(out)
}

/// Evenly spaced picks: up to `n` of `len` indices (all when `n` is 0 or `len <= n`).
pub(crate) fn sample_indices(len: usize, n: usize) -> Vec<usize> {
    if n == 0 || len <= n {
        return (0..len).collect();
    }
    (0..n)
        .filter_map(|i| i.checked_mul(len).and_then(|x| x.checked_div(n)))
        .collect()
}

/// Our record types in `name` (from `kit coverage`), without `TES4`.
fn our_types(kit: &dyn Runner, path: &Path, opts: &Opts) -> Result<Vec<String>> {
    let r = kit.kit(
        &["coverage".to_string(), arg(path)?],
        &opts.work.join("cache"),
    )?;
    if let Some(e) = r.error() {
        bail!("kit coverage: {e}");
    }
    Ok(r.data()
        .get("record_types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| t.get("signature").and_then(Value::as_str))
        .filter(|s| *s != "TES4")
        .map(str::to_string)
        .collect())
}

/// One fixture: stage, dump, check, compare.
fn one(
    dumper: &Dumper,
    kit: &dyn Runner,
    session_kit: Option<&KitBinary>,
    input: &Input,
    opts: &Opts,
) -> Result<Value> {
    let name = input
        .path
        .file_name()
        .context("fixture path has no file name")?
        .to_string_lossy()
        .into_owned();
    let from_dir = input
        .path
        .parent()
        .map_or_else(|| opts.data_dir.clone(), Path::to_path_buf);
    let bytes =
        fs::read(&input.path).with_context(|| format!("reading {}", input.path.display()))?;
    let masters = stage(&name, &from_dir, opts, 0)?;
    let scratch = opts.work.join("Data");
    let plugin = scratch.join(&name);
    let mut row = json!({
        "plugin": name,
        "set": input.set,
        "source": input.path.to_string_lossy(),
        "bytes": bytes.len(),
        "sha256": sha256_hex(&bytes),
        "masters": masters,
    });
    let mut base_args = vec!["-sf1".to_string(), format!("-D:{}\\", arg(&scratch)?)];
    let ini = opts.work.join("Starfield.ini");
    if ini.is_file() {
        base_args.push(format!("-I:{}", arg(&ini)?));
    }
    let types: Vec<String> = if opts.types.is_empty() {
        our_types(kit, &plugin, opts)?
    } else {
        opts.types.clone()
    };
    let mut dump_args = base_args.clone();
    if !opts.types.is_empty() {
        dump_args.push(format!("-dg:{}", opts.types.join(",")));
    }
    dump_args.push(arg(&plugin)?);
    println!("oracle: sf1edit dump {name}");
    let dump = dumper.run(&dump_args, &opts.work.join(format!("{name}.dump.txt")))?;
    let text = read_text(&dump.stdout);
    let tool = banner(&dump.stderr);
    let all = parse_dump(&text);
    drop(text);
    // xDump exits 0 even when it stops with "Unexpected Error": no records at all (not even
    // the TES4 header) is the failure signal.
    let ok = !all.is_empty();
    let set = |row: &mut Value, k: &str, v: Value| {
        if let Some(m) = row.as_object_mut() {
            m.insert(k.to_string(), v);
        }
    };
    set(
        &mut row,
        "dump",
        json!({
            "args": dump_args.iter().map(|a| if a.contains('\\') { "<path>" } else { a.as_str() }).collect::<Vec<_>>(),
            "tool": tool,
            "exit": dump.exit,
            "seconds": dump.seconds,
            "timed_out": dump.timed_out,
            "records": all.len(),
            "error": (!ok).then(|| dump.stderr_tail.clone()),
        }),
    );
    if opts.check {
        let mut check_args = base_args.clone();
        check_args.push("-check".into());
        check_args.push(arg(&plugin)?);
        println!("oracle: sf1edit check {name}");
        let ck = dumper.run(&check_args, &opts.work.join(format!("{name}.check.txt")))?;
        let ck_text = read_text(&ck.stdout);
        let mut v = parse_check(&ck_text);
        if let Some(m) = v.as_object_mut() {
            m.insert("exit".into(), json!(ck.exit));
            m.insert("seconds".into(), json!(ck.seconds));
            if ck.exit != Some(0) && ck_text.trim().is_empty() {
                m.insert("error".into(), json!(ck.stderr_tail));
            }
        }
        set(&mut row, "check", v);
    }
    if !ok {
        return Ok(row);
    }
    let map = LoadMap {
        masters: masters.clone(),
        this: name.clone(),
    };
    let records: Vec<&XRecord> = all.iter().filter(|r| r.sig != "TES4").collect();
    let mut chain = masters.clone();
    chain.push(name.clone());
    let ours = our_records(kit, &chain, &name, &types, opts)?;
    let mut xmap: BTreeMap<FormKey, &XRecord> = BTreeMap::new();
    for r in &records {
        xmap.insert(map.xedit(r.form_id), r);
    }
    let mut omap: BTreeMap<FormKey, (String, String, Option<String>)> = BTreeMap::new();
    for (k, form, sig, edid) in ours {
        omap.insert(k, (form, sig, edid));
    }
    let only_xedit: Vec<String> = xmap
        .iter()
        .filter(|(k, _)| !omap.contains_key(*k))
        .map(|(_, r)| {
            format!(
                "{} [{:08X}] {}",
                r.sig,
                r.form_id,
                r.editor_id.as_deref().unwrap_or("")
            )
        })
        .collect();
    let only_ours: Vec<String> = omap
        .iter()
        .filter(|(k, _)| !xmap.contains_key(*k))
        .map(|(_, (form, sig, edid))| format!("{sig} {form} {}", edid.as_deref().unwrap_or("")))
        .collect();
    let mut per_type: BTreeMap<String, Vec<(FormKey, String)>> = BTreeMap::new();
    let mut header_mismatch = Vec::new();
    for (k, (form, sig, edid)) in &omap {
        let Some(x) = xmap.get(k) else { continue };
        if x.sig != *sig || x.editor_id.as_deref().unwrap_or("") != edid.as_deref().unwrap_or("") {
            header_mismatch.push(format!(
                "{form}: ours {sig} <{}>, xEdit {} <{}>",
                edid.as_deref().unwrap_or(""),
                x.sig,
                x.editor_id.as_deref().unwrap_or("")
            ));
        }
        per_type
            .entry(sig.clone())
            .or_default()
            .push((k.clone(), form.clone()));
    }
    let mut type_rows = Map::new();
    let cache = opts.work.join("cache");
    let scratch_arg = arg(&scratch)?;
    let schema = kitkraft_schema::builtin().ok();
    let mut inspector = Inspector::start(kit, session_kit, &chain, &scratch_arg, &cache);
    let stem = Path::new(&name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let strings_archive = ["Localization", "Main"]
        .iter()
        .map(|s| scratch.join(format!("{stem} - {s}.ba2")))
        .find(|p| p.is_file());
    let mut strings = StringLookup::new(strings_archive, &stem);
    for ty in &types {
        let xcount = records.iter().filter(|r| r.sig == *ty).count();
        let ocount = omap.values().filter(|(_, s, _)| s == ty).count();
        let matched = per_type.get(ty).cloned().unwrap_or_default();
        let picks = sample_indices(matched.len(), opts.sample);
        let layouts = schema.map(|s| layouts(s, ty)).unwrap_or_default();
        let mut tally = Tally::default();
        let mut failures = Vec::new();
        println!(
            "oracle: sf1edit {name} {ty}: {} matched, comparing {}",
            matched.len(),
            picks.len()
        );
        for i in picks.iter().copied() {
            let Some((k, form)) = matched.get(i) else {
                continue;
            };
            let Some(x) = xmap.get(k) else { continue };
            let r = inspector.inspect(form)?;
            if let Some(e) = r.error() {
                failures.push(format!("{form}: {e}"));
                continue;
            }
            let version = r
                .data()
                .get("versions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|v| {
                    v.get("plugin")
                        .and_then(Value::as_str)
                        .is_some_and(|p| p.eq_ignore_ascii_case(&name))
                });
            let subs: Vec<Value> = version
                .and_then(|v| v.get("subrecords"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut lookup = |id: u32| strings.get(&mut inspector, id);
            compare_record(form, &subs, x, &map, &layouts, &mut lookup, &mut tally);
        }
        let names = settle(&mut tally);
        // What xEdit shows as unknown, over every record of the type.
        let mut unknown: BTreeMap<String, (u64, u64)> = BTreeMap::new();
        for r in records.iter().filter(|r| r.sig == *ty) {
            for n in &r.nodes {
                let e = unknown.entry(n.sig.clone()).or_insert((0, 0));
                e.1 = e.1.saturating_add(1);
                if n.leaves.iter().any(|l| leaf_unknown(n, l)) {
                    e.0 = e.0.saturating_add(1);
                }
            }
        }
        let unknown: Map<String, Value> = unknown
            .into_iter()
            .filter(|(_, (u, _))| *u > 0)
            .map(|(k, (u, t))| (k, json!({"unknown_nodes": u, "nodes": t})))
            .collect();
        let mut totals = Map::new();
        for m in tally.members.values() {
            for (k, v) in m {
                let n = totals.get(k).and_then(Value::as_u64).unwrap_or(0);
                totals.insert(k.clone(), json!(n.saturating_add(v.as_u64().unwrap_or(0))));
            }
        }
        type_rows.insert(
            ty.clone(),
            json!({
                "xedit_records": xcount,
                "our_records": ocount,
                "matched": matched.len(),
                "compared": picks.len(),
                "totals": totals,
                "members": tally.members,
                "examples": tally.examples,
                "name_pairs": names,
                "xedit_unknown": unknown,
                "inspect_failures": failures,
            }),
        );
    }
    set(&mut row, "inspect_via", json!(inspector.via()));
    set(&mut row, "xedit_records", json!(records.len()));
    set(&mut row, "our_records", json!(omap.len()));
    set(&mut row, "only_xedit", json!(only_xedit));
    set(&mut row, "only_ours", json!(only_ours));
    set(&mut row, "header_mismatch", json!(header_mismatch));
    set(&mut row, "types", Value::Object(type_rows));
    Ok(row)
}

/// `cargo xtask oracle sf1edit ...`.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let mut official = false;
    let mut synthetic = false;
    let mut keep = false;
    let mut check = true;
    let mut sample = DEFAULT_SAMPLE;
    let mut types: Vec<String> = Vec::new();
    let mut timeout_mins = 20u64;
    let mut out = root.join("observe").join("oracle").join("sf1edit");
    let mut given = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--official" => official = true,
            "--synthetic" => synthetic = true,
            "--keep" => keep = true,
            "--no-check" => check = false,
            "--sample" => {
                sample = it
                    .next()
                    .context("--sample needs a number (0 = all)")?
                    .parse()
                    .context("--sample needs a number")?;
            }
            "--types" => {
                types = it
                    .next()
                    .context("--types needs SIG,SIG,...")?
                    .split(',')
                    .map(|s| s.trim().to_ascii_uppercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "--timeout-mins" => {
                timeout_mins = it
                    .next()
                    .context("--timeout-mins needs a number")?
                    .parse()
                    .context("--timeout-mins needs a number")?;
            }
            "--out" => out = PathBuf::from(it.next().context("--out needs a directory")?),
            flag if flag.starts_with("--") => bail!("oracle sf1edit: unknown flag `{flag}`"),
            path => given.push(PathBuf::from(path)),
        }
    }
    if !official && !synthetic && given.is_empty() {
        bail!("oracle sf1edit: name plugins (a bare name is read from {DATA_DIR_VAR}), or pass --official and/or --synthetic");
    }
    let mut dumper = Dumper::from_env()?;
    dumper.timeout = Duration::from_secs(timeout_mins.saturating_mul(60));
    let data_dir = crate::env_var_os(DATA_DIR_VAR)
        .map(PathBuf::from)
        .with_context(|| format!("set {DATA_DIR_VAR} (masters are copied from there)"))?;
    let kit = KitBinary::from_env(root);
    let work = std::env::temp_dir().join(format!("kitkraft-oracle-sf1edit-{}", std::process::id()));
    fs::create_dir_all(work.join("Data"))?;
    if let Some(ini) = data_dir.parent().map(|p| p.join("Starfield.ini")) {
        if ini.is_file() {
            fs::copy(&ini, work.join("Starfield.ini"))?;
        }
    }
    let opts = Opts {
        data_dir: data_dir.clone(),
        work: work.clone(),
        types,
        sample,
        check,
    };
    let mut inputs: Vec<Input> = Vec::new();
    if official {
        for n in OFFICIAL_SMALL {
            inputs.push(Input {
                path: data_dir.join(n),
                set: "official",
            });
        }
    }
    if synthetic {
        inputs.extend(write_synthetic(
            &work.join("synthetic"),
            &kit,
            &work.join("cache"),
        )?);
    }
    for p in given {
        let path = if p.components().count() == 1 && !p.is_file() {
            data_dir.join(p)
        } else {
            p
        };
        inputs.push(Input { path, set: "given" });
    }
    let mut rows = Vec::new();
    for input in &inputs {
        match one(&dumper, &kit, Some(&kit), input, &opts) {
            Ok(r) => rows.push(r),
            Err(e) => rows.push(json!({
                "plugin": input.path.file_name().map(|n| n.to_string_lossy().into_owned()),
                "set": input.set,
                "error": format!("{e:#}"),
            })),
        }
    }
    let tool = rows
        .iter()
        .find_map(|r| r.pointer("/dump/tool").and_then(Value::as_str))
        .map(str::to_string);
    let report = json!({
        "generated_by": "cargo xtask oracle sf1edit (M3.H4)",
        "tool": {"dumper": DUMPER, "banner": tool},
        "method": "xEdit's console dumper on a scratch copy of the plugin and its masters (game Data only read); records keyed by defining file and object ID; sampled records compared member by member with kit inspect",
        "sample_per_type": opts.sample,
        "types_filter": opts.types,
        "fixtures": rows,
    });
    fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    let mut text = serde_json::to_string_pretty(&report)?;
    text.push('\n');
    fs::write(out.join("report.json"), text)?;
    fs::write(out.join("report.md"), render(&report))?;
    if !keep {
        let _ = fs::remove_dir_all(&work);
    }
    println!("oracle: {}", out.join("report.md").display());
    Ok(())
}

fn num(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

const CLASSES: [&str; 9] = [
    "agree",
    "agree_fewer_fields",
    "disagree",
    "typed_not_shown",
    "typed_xedit_unknown",
    "gap",
    "both_unknown",
    "opaque_not_shown",
    "xedit_only",
];

/// `report.md`.
pub(crate) fn render(r: &Value) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# SF1Edit (xEdit) comparison report\n");
    let _ = writeln!(
        md,
        "Written by `cargo xtask oracle sf1edit` (M3.H4). Tool: {} ({}). Sample per type: {} (0 = all). {}.\n",
        r.pointer("/tool/banner").and_then(Value::as_str).unwrap_or("-"),
        r.pointer("/tool/dumper").and_then(Value::as_str).unwrap_or("-"),
        r.get("sample_per_type").map_or("-".to_string(), Value::to_string),
        r.get("method").and_then(Value::as_str).unwrap_or("")
    );
    let _ = writeln!(
        md,
        "Classes per subrecord occurrence of a compared record: `agree`/`disagree` (a member we \
         decode, against xEdit's displayed values), `typed_not_shown` (we decode it, xEdit shows \
         no such subrecord), `typed_xedit_unknown` (we decode it, xEdit shows it as unknown), \
         `gap` (opaque to us, typed by xEdit), `both_unknown`, `opaque_not_shown`, and \
         `xedit_only` (an xEdit subrecord with no counterpart in our list).\n"
    );
    let fixtures: Vec<&Value> = r
        .get("fixtures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    let mut rows = Vec::new();
    for f in &fixtures {
        let mut tot = BTreeMap::new();
        for t in f
            .get("types")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|m| m.values())
        {
            for c in CLASSES {
                let e = tot.entry(c).or_insert(0u64);
                *e = e.saturating_add(t.get("totals").map_or(0, |x| num(x, c)));
            }
        }
        let g = |c: &str| tot.get(c).copied().unwrap_or(0).to_string();
        rows.push(vec![
            format!(
                "`{}`",
                f.get("plugin").and_then(Value::as_str).unwrap_or("?")
            ),
            f.get("set")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            f.pointer("/dump/seconds")
                .map_or("-".into(), Value::to_string),
            format!(
                "{} / {}",
                f.get("xedit_records").map_or("-".into(), Value::to_string),
                f.get("our_records").map_or("-".into(), Value::to_string)
            ),
            format!(
                "{} / {}",
                f.get("only_xedit")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len),
                f.get("only_ours")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len)
            ),
            f.get("header_mismatch")
                .and_then(Value::as_array)
                .map_or("-".into(), |a| a.len().to_string()),
            g("agree"),
            g("agree_fewer_fields"),
            g("disagree"),
            g("gap"),
            g("both_unknown"),
            g("typed_xedit_unknown"),
            g("typed_not_shown"),
            f.pointer("/check/findings")
                .map_or("-".into(), Value::to_string),
            f.get("error")
                .or_else(|| f.pointer("/dump/error"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(160)
                .collect(),
        ]);
    }
    md.push_str(&table(
        &[
            "Fixture",
            "Set",
            "Dump s",
            "Records xEdit / ours",
            "Only xEdit / only ours",
            "Sig/EDID differ",
            "Agree",
            "Agree, xEdit shows fewer fields",
            "Disagree",
            "Gap",
            "Both unknown",
            "Typed, xEdit unknown",
            "Typed, not shown",
            "Check findings",
            "Error",
        ],
        &rows,
    ));
    for f in fixtures {
        let _ = writeln!(
            md,
            "\n## `{}` ({})\n",
            f.get("plugin").and_then(Value::as_str).unwrap_or("?"),
            f.get("set").and_then(Value::as_str).unwrap_or("")
        );
        for key in ["only_xedit", "only_ours", "header_mismatch"] {
            let items: Vec<&str> = f
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            if !items.is_empty() {
                let shown: Vec<&str> = items.iter().take(10).copied().collect();
                let _ = writeln!(md, "- {key} ({}): {}", items.len(), shown.join("; "));
            }
        }
        if let Some(c) = f.get("check") {
            let _ = writeln!(md, "- xEdit check: {} finding(s)", num(c, "findings"));
            for (m, n) in c
                .get("by_message")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let _ = writeln!(md, "  - {n} x `{m}`");
            }
            if let Some(e) = c.get("error").and_then(Value::as_str) {
                let _ = writeln!(md, "  - check failed: {e}");
            }
        }
        let Some(types) = f.get("types").and_then(Value::as_object) else {
            continue;
        };
        let mut trows = Vec::new();
        for (ty, t) in types {
            let tot = t.get("totals").cloned().unwrap_or(Value::Null);
            let mut row = vec![
                format!("`{ty}`"),
                format!(
                    "{} / {} / {}",
                    num(t, "xedit_records"),
                    num(t, "our_records"),
                    num(t, "matched")
                ),
                num(t, "compared").to_string(),
            ];
            row.extend(CLASSES.iter().map(|c| num(&tot, c).to_string()));
            trows.push(row);
        }
        let mut head = vec!["Type", "xEdit / ours / matched", "Compared"];
        head.extend(CLASSES);
        md.push('\n');
        md.push_str(&table(&head, &trows));
        let mut lines = Vec::new();
        for (ty, t) in types {
            for (sub, m) in t
                .get("members")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let d = num(m, "disagree");
                let g = num(m, "gap");
                if d > 0 || g > 0 {
                    let ex: Vec<String> = ["disagree", "gap"]
                        .iter()
                        .flat_map(|k| {
                            t.pointer(&format!("/examples/{k} {sub}"))
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                                .take(1)
                                .map(|e| {
                                    e.get("xedit")
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_string()
                                })
                        })
                        .collect();
                    lines.push(format!(
                        "- `{ty}/{sub}`: disagree {d}, gap {g}; xEdit shows: {}",
                        ex.join(" | ")
                    ));
                }
            }
        }
        if !lines.is_empty() {
            let _ = writeln!(md, "\nDisagreements and coverage gaps by member:\n");
            for l in lines {
                let _ = writeln!(md, "{l}");
            }
        }
    }
    md
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    /// A dump of a synthetic plugin in the shape xDump prints (a `TES4`, a GLOB, a WEAP with
    /// keywords and a struct, and a light-plugin record).
    const DUMP: &str = "SF1Dump 4.1.5f x64\r
\r
Record Header [S]: [v582] NULL {ESM}\r
  Signature: TES4\r
  FormID: NULL - Null Reference [00000000]\r
HEDR - Header\r
  Version: 0.960000\r
  Number of Records: 3\r
Master Files [S]: <1 master file>\r
  Master File #0\r
    MAST - FileName: Starfield.esm\r
GRUP Top \"GLOB\"\r
  Record Header [S]: [v581] GLOB - Global [01000800] <KC_Global>\r
    Signature: GLOB\r
    Data Size: 26\r
  EDID - Editor ID: KC_Global\r
  FLTV - Value: 1234.500000\r
GRUP Top \"WEAP\"\r
  Record Header [S]: [v581] WEAP - Weapon [0021BBCD] <RussianHuntingRifle>\r
    Signature: WEAP\r
  EDID - Editor ID: RussianHuntingRifle\r
  OBND - Object Bounds [S]: (0, 0, 0), (1, 2, 3)\r
    Min [S]: (0, 0, 0)\r
      X: 0.000000\r
    Max [S]: (1, 2, 3)\r
      X: 1.000000\r
  FULL - Name: Old Soviet Marksman Rifle\r
  Keywords [S]: <2 keywords>\r
    KSIZ - Keyword Count: 2\r
    KWDA - Keywords [S]: <2 keywords>\r
      Keyword #0: KYWD - Keyword [00015EAD] <WeaponTypeRifle>\r
      Keyword #1: KYWD - Keyword [01000801] <KC_Kw>\r
  DESC - Description: <Error: No strings file for lstring ID 00000208>\r
  DNAM - Unknown: 00 1F 2A\r
  WAIM - Aim\r
    Unknown: 0\r
    Unknown: 1\r
  STOP - Marker\r
  QUPA\r
  [Type] AVM Data [S]: [Type] AVM Data\r
    Record Header [S]: [v552] KYWD - Keyword [FE000009] <AVM_Light>\r
      Signature: KYWD\r
    EDID - Editor ID: AVM_Light\r
    FNAM - Unknown: 00 00 00 00\r
";

    fn map() -> LoadMap {
        LoadMap {
            masters: vec!["Starfield.esm".into()],
            this: "KC_Test.esp".into(),
        }
    }

    #[test]
    fn parses_a_synthetic_dump() {
        let recs = parse_dump(DUMP);
        assert_eq!(recs.len(), 4, "{recs:#?}");
        let tes4 = &recs[0];
        assert_eq!(tes4.sig, "TES4");
        assert_eq!(tes4.nodes[0].sig, "HEDR");
        assert_eq!(tes4.nodes[0].leaves.len(), 2);
        assert_eq!(tes4.nodes[1].sig, "MAST");
        assert_eq!(
            tes4.nodes[1].leaves,
            vec![XLeaf {
                path: String::new(),
                value: Some("Starfield.esm".into())
            }]
        );
        let glob = &recs[1];
        assert_eq!((glob.sig.as_str(), glob.form_id), ("GLOB", 0x0100_0800));
        assert_eq!(glob.editor_id.as_deref(), Some("KC_Global"));
        assert_eq!(glob.nodes.len(), 2);
        let weap = &recs[2];
        assert_eq!(weap.sig, "WEAP");
        let sigs: Vec<&str> = weap.nodes.iter().map(|n| n.sig.as_str()).collect();
        assert_eq!(
            sigs,
            ["EDID", "OBND", "FULL", "KSIZ", "KWDA", "DESC", "DNAM", "WAIM", "STOP", "QUPA"]
        );
        let obnd = &weap.nodes[1];
        assert_eq!(obnd.leaves[0].path, "Min\\X");
        assert_eq!(obnd.leaves[1].path, "Max\\X");
        let kwda = &weap.nodes[4];
        assert_eq!(kwda.leaves.len(), 2);
        assert_eq!(kwda.leaves[1].path, "Keyword #1");
        // Repeated names get an index; a valueless leaf keeps None.
        assert_eq!(weap.nodes[7].leaves[1].path, "Unknown#1");
        assert_eq!(weap.nodes[8].leaves[0].value, None);
        // A subrecord listed by its bare signature (xEdit did not decode it) is unknown.
        assert_eq!(weap.nodes[9].name, UNDECODED);
        assert!(node_unknown(&weap.nodes[9]));
        // A record under a `[Type]` wrapper (KYWD in Starfield) is still found.
        let kw = &recs[3];
        assert_eq!((kw.sig.as_str(), kw.form_id), ("KYWD", 0xFE00_0009));
        assert_eq!(kw.nodes.len(), 2);
    }

    #[test]
    fn keys_map_load_order_ids_to_files() {
        let m = map();
        assert_eq!(
            m.xedit(0x0021_BBCD),
            FormKey::Key("starfield.esm".into(), 0x21BBCD)
        );
        assert_eq!(
            m.xedit(0x0100_0800),
            FormKey::Key("kc_test.esp".into(), 0x800)
        );
        assert_eq!(m.xedit(0xFE00_0009), FormKey::Key("kc_test.esp".into(), 9));
        assert_eq!(m.xedit(0), FormKey::Null);
        assert_eq!(
            our_key("Starfield.esm:21BBCD"),
            Some(FormKey::Key("starfield.esm".into(), 0x21BBCD))
        );
        assert_eq!(our_key("nonsense"), None);
    }

    #[test]
    fn compares_typed_members_and_classifies_opaque_ones() {
        let recs = parse_dump(DUMP);
        let weap = &recs[2];
        let subs = vec![
            json!({"signature": "EDID", "value": "RussianHuntingRifle"}),
            json!({"signature": "OBND"}),
            json!({"signature": "FULL", "value": {"string_id": "0x00000010"}, "text": "Old Soviet Marksman Rifle"}),
            json!({"signature": "KSIZ", "value": 3}),
            json!({"signature": "KWDA", "value": [
                {"form": "Starfield.esm:015EAD", "raw": "00015EAD"},
                {"form": "KC_Test.esp:000801", "raw": "01000801"}]}),
            json!({"signature": "DESC", "value": {"string_id": "0x00000208"}}),
            json!({"signature": "DNAM"}),
            json!({"signature": "WAIM"}),
            json!({"signature": "XXXX", "value": 1.5}),
            json!({"signature": "QUPA", "value": 7}),
        ];
        let mut tally = Tally::default();
        compare_record(
            "Starfield.esm:21BBCD",
            &subs,
            weap,
            &map(),
            &Layouts::new(),
            &mut |_| None,
            &mut tally,
        );
        let Tally {
            members, examples, ..
        } = tally;
        let c = |sig: &str, k: &str| members[sig].get(k).and_then(Value::as_u64).unwrap_or(0);
        assert_eq!(c("EDID", "agree"), 1);
        assert_eq!(c("FULL", "agree"), 1, "lstring compared by resolved text");
        assert_eq!(
            c("KSIZ", "disagree"),
            1,
            "3 keywords claimed, xEdit shows 2"
        );
        assert_eq!(c("KWDA", "agree"), 1, "FormIDs mapped through the masters");
        assert_eq!(
            c("DESC", "agree"),
            1,
            "lstring compared by ID when xEdit has no table"
        );
        assert_eq!(c("OBND", "gap"), 1);
        assert_eq!(c("DNAM", "both_unknown"), 1);
        assert_eq!(c("WAIM", "both_unknown"), 1);
        assert_eq!(c("XXXX", "typed_not_shown"), 1);
        assert_eq!(c("STOP", "xedit_only"), 1);
        assert_eq!(c("QUPA", "typed_xedit_unknown"), 1);
        assert!(examples.contains_key("disagree KSIZ"));
        assert!(examples["gap OBND"][0]["xedit"]
            .as_str()
            .unwrap()
            .contains("Min\\X=0.000000"));
        let report = json!({"fixtures": [{"plugin": "KC_Test.esp", "set": "synthetic",
            "types": {"WEAP": {"totals": {"agree": 4, "gap": 1}, "members": members, "examples": examples}}}]});
        let md = render(&report);
        assert!(md.contains("`WEAP/OBND`: disagree 0, gap 1"), "{md}");
    }

    #[test]
    fn value_conversion() {
        let m = map();
        assert!(same(&Ours::Num(0.96), Some("0.960000"), &m));
        assert!(same(
            &Ours::Num(f64::from(0xFF00_1000u32)),
            Some("FF001000"),
            &m
        ));
        assert!(!same(&Ours::Num(1.0), Some("Unknown 1"), &m));
        assert!(same(&Ours::Num(0.0), Some("False"), &m));
        assert!(!same(&Ours::Num(2.0), Some("True"), &m));
        assert!(same(&Ours::Bool(false), Some("False"), &m));
        assert!(same(
            &Ours::Form(Some(FormKey::Null)),
            Some("NULL - Null Reference [00000000]"),
            &m
        ));
        assert!(same(&Ours::LString(0, None), None, &m));
        assert!(!same(&Ours::Text("a".into()), Some("b"), &m));
        let mut flat = Vec::new();
        let mut ordered = true;
        flatten(
            &json!({"b": 2, "a": [1, {"string_id": "0x1"}]}),
            None,
            &mut flat,
            &mut ordered,
        );
        assert!(!ordered);
        assert_eq!(flat.len(), 3);
        // The M4.W0 opaque marker holds a place like the old "opaque" string.
        let mut marked = Vec::new();
        flatten(
            &json!([{"kind": "opaque", "opaque": {"payload_offset": 0, "length": 4, "file_offset": 9}},
                {"kind": "formref", "raw": "00000000", "form": null, "null": true}]),
            None,
            &mut marked,
            &mut ordered,
        );
        assert!(
            matches!(
                marked.as_slice(),
                [Ours::Any, Ours::Form(Some(FormKey::Null))]
            ),
            "{marked:?}"
        );
    }

    #[test]
    fn check_output_is_grouped_by_message() {
        let text = "    AVMD - AVM Data [FE000029] <X> -> Warning: internal file FormID is a HITME: FE000029 (should be 01000029 )\n    OMOD - Object Modification [FE000008] <Y> -> Warning: internal file FormID is a HITME: FE000008 (should be 01000008 )\n  Above errors were found in: GRUP Top \"OMOD\"\n";
        let v = parse_check(text);
        assert_eq!(v["findings"], 2);
        assert_eq!(
            v["by_message"]["Warning: internal file FormID is a HITME: # (should be # )"],
            2
        );
    }

    #[test]
    fn dumper_text_is_decoded_as_windows_1252_when_not_utf8() {
        assert_eq!(decode_text(b"plain".to_vec()), "plain");
        assert_eq!(decode_text("caf\u{e9}".as_bytes().to_vec()), "caf\u{e9}");
        assert_eq!(
            decode_text(vec![b'X', 0x99, 0xE9, 0x81]),
            "X\u{2122}\u{e9}\u{FFFD}"
        );
    }

    #[test]
    fn samples_are_evenly_spaced() {
        assert_eq!(sample_indices(3, 25), vec![0, 1, 2]);
        assert_eq!(sample_indices(10, 0).len(), 10);
        assert_eq!(sample_indices(100, 4), vec![0, 25, 50, 75]);
        assert_eq!(bracket_form("x [v581] [0021BBCD] y"), Some(0x21BBCD));
        assert_eq!(mask_hex("ID 00000208 at 12"), "ID # at #");
    }

    /// Two records in xEdit's shape for the display-conversion tests (M3.B11): a WEAP with
    /// an enum (`WGEN Type`), a GUID sound pair and an lstring inside a struct; an AMMO with
    /// flags and an opaque field xEdit shows as `Unused`.
    const DISPLAY_DUMP: &str = "GRUP Top \"WEAP\"
  Record Header [S]: [v581] WEAP - Weapon [000001F4] <Unarmed>
    Signature: WEAP
  WGEN - General [S]:
    Type: Unarmed
    Base weight: 0.000000
    Base value: 0
    Base Speed: 1.000000
    Attack Oxygen Cost: 40.000000
    Weapon Barrel: NULL - Null Reference [00000000]
  WMEL - Melee [S]:
    Unknown: 2
    Bash Damage: 14.000000
    Reach: 0.600000
    Stagger: Small
  PUSH - Pickup Sound [S]:
    Start: {69D5B163-1B78-4DC3-AB0E-AA1366F7E1D9}
    Stop: {00000000-0000-0000-0000-000000000000}
    Condition: NULL - Null Reference [00000000]
    Event Mapping: NULL - Null Reference [00000000]
  Record Header [S]: [v581] AMMO - Ammo [00003BD3] <AmmoX>
    Signature: AMMO
  DNAM - Data [S]:
    Projectile: PROJ - Projectile [00128A0B] <P>
    Flags: Non-Playable
    Unused: 00 00 00
    Damage: 0.000000
    Health: 0
  ONAM - Short Name: Missile
";

    fn schema() -> &'static kitkraft_schema::Schema {
        kitkraft_schema::builtin().unwrap()
    }

    fn run_one(
        rec: &XRecord,
        subs: &[Value],
        strings: &mut dyn FnMut(u32) -> Option<String>,
    ) -> Tally {
        let layouts = layouts(schema(), &rec.sig);
        let mut tally = Tally::default();
        compare_record(
            "Starfield.esm:0001F4",
            subs,
            rec,
            &map(),
            &layouts,
            strings,
            &mut tally,
        );
        tally
    }

    fn count(t: &Tally, sig: &str, k: &str) -> u64 {
        t.members[sig].get(k).and_then(Value::as_u64).unwrap_or(0)
    }

    #[test]
    fn enums_flags_guids_and_nested_lstrings_are_converted() {
        let recs = parse_dump(DISPLAY_DUMP);
        let weap = vec![
            json!({"signature": "WGEN", "member": "General", "value": {
                "AttackOxygenCost": 40.0, "BaseSpeed": 1.0, "BaseValue": 0, "BaseWeight": 0.0,
                "Type": 0, "WeaponBarrel": {"form": null, "null": true, "raw": "00000000"}}}),
            // Our names differ from xEdit's here (AttackSpeed vs Unknown, Stagger by value).
            json!({"signature": "WMEL", "member": "Melee", "value": {
                "AttackSpeed": 2, "BashDamage": 14.0, "Reach": 0.6000000238418579, "Stagger": 1}}),
            // The Wwise events are opaque to us: they hold their place and match anything.
            json!({"signature": "PUSH", "member": "PickupSound", "value": {
                "Condition": {"raw": "00000000"}, "StartEvent": "opaque", "StartForm": {"raw": "00000000"}, "StopEvent": "opaque"}}),
        ];
        let t = run_one(&recs[0], &weap, &mut |_| None);
        assert_eq!(
            count(&t, "WGEN", "agree"),
            1,
            "{:?} {:?}",
            t.examples,
            t.members
        );
        assert_eq!(count(&t, "WMEL", "agree"), 1, "{:?}", t.examples);
        assert_eq!(count(&t, "PUSH", "agree"), 1, "{:?}", t.examples);
        // Stagger 1 is "Small" upstream and in our schema alike, so no name pair is needed.
        assert!(t.pending.is_empty(), "{:?}", t.pending);

        let ammo = vec![
            json!({"signature": "DNAM", "member": "AmmoData", "value": {
                "Damage": 0.0, "Flags": 2, "Health": 0,
                "Projectile": {"form": "Starfield.esm:128A0B", "raw": "00128A0B"}, "Unused": "opaque"}}),
            json!({"signature": "ONAM", "member": "ShortName", "value": {"string_id": "0x0003136F"}}),
        ];
        let t = run_one(&recs[1], &ammo, &mut |id| {
            (id == 0x0003_136F).then(|| "Missile".to_string())
        });
        assert_eq!(count(&t, "DNAM", "agree"), 1, "{:?}", t.examples);
        assert_eq!(
            count(&t, "ONAM", "agree"),
            1,
            "nested lstring resolved through the tables"
        );
        // Without the table the lstring cannot be compared by text.
        let t = run_one(&recs[1], &ammo, &mut |_| None);
        assert_eq!(count(&t, "ONAM", "disagree"), 1);
    }

    #[test]
    fn display_conversions() {
        let m = map();
        let e = |n: i128, name: &str| Ours::Enum {
            key: "COBJ/LRNM".into(),
            n,
            name: Some(name.into()),
        };
        assert_eq!(judge(&e(0, "Unarmed"), Some("Unarmed"), &m), Hit::Yes);
        assert_eq!(
            judge(&e(3, "SingleOrBinary"), Some("Single or Binary"), &m),
            Hit::Yes
        );
        assert_eq!(judge(&e(5, "X"), Some("<Unknown: 5>"), &m), Hit::Yes);
        assert_eq!(judge(&e(5, "X"), Some("6"), &m), Hit::No);
        assert_eq!(
            judge(
                &e(3, "DefaultOrConditions"),
                Some("Known by default or when conditions are met"),
                &m
            ),
            Hit::Pending((
                "COBJ/LRNM".into(),
                3,
                "Known by default or when conditions are met".into()
            ))
        );
        let f = Ours::Flags {
            key: "PROJ/PROD.Flags".into(),
            n: 0x6,
            names: vec!["MuzzleFlash".into(), "Explosion".into()],
        };
        assert_eq!(judge(&f, Some("Explosion, Muzzle Flash"), &m), Hit::Yes);
        let empty = Ours::Flags {
            key: "k".into(),
            n: 0,
            names: vec![],
        };
        assert_eq!(judge(&empty, Some(""), &m), Hit::Yes);
        let bytes = vec![
            0x63, 0xB1, 0xD5, 0x69, 0x63, 0x1B, 0xC3, 0x4D, 0xAB, 0x0E, 0xAA, 0x13, 0x66, 0xF7,
            0xE1, 0xD9,
        ];
        assert_eq!(
            guid_text(&bytes).unwrap(),
            "{69D5B163-1B63-4DC3-AB0E-AA1366F7E1D9}"
        );
        assert_eq!(guid_text(&bytes[..15]), None);
        let guid = Ours::Bytes(bytes);
        assert_eq!(
            judge(&guid, Some("{69d5b163-1b63-4dc3-ab0e-aa1366f7e1d9}"), &m),
            Hit::Yes
        );
        let name = Ours::Bytes(b"TESObjectARMOR_InstanceData\0".to_vec());
        assert_eq!(
            judge(&name, Some("TESObjectARMOR_InstanceData"), &m),
            Hit::Yes
        );
        assert_eq!(
            judge(&Ours::Form(Some(FormKey::Null)), Some("00 00 00 00"), &m),
            Hit::Yes
        );
        assert_eq!(
            judge(
                &Ours::Num(0.0),
                Some("NULL - Null Reference [00000000]"),
                &m
            ),
            Hit::Yes
        );
        assert_eq!(
            judge(&Ours::Num(12.0), Some("12"), &m),
            Hit::Yes,
            "not read as a raw byte"
        );
        assert_eq!(
            judge(
                &Ours::Text("BGSQualityUpgrade_Component".into()),
                Some("<Unknown: BGSQualityUpgrade_Component>"),
                &m
            ),
            Hit::Yes
        );
        assert_eq!(judge(&Ours::Any, Some("whatever"), &m), Hit::Yes);
    }

    #[test]
    fn name_pairs_must_be_consistent_over_the_run() {
        let pend = |form: &str, n: i128, t: &str| PendingAgree {
            sig: "LRNM".into(),
            form: form.into(),
            pairs: vec![("COBJ/LRNM".into(), n, t.into())],
            xedit: String::new(),
            class: "agree",
        };
        let mut tally = Tally::default();
        tally
            .members
            .entry("LRNM".into())
            .or_default()
            .insert("agree".into(), json!(4));
        tally.pending = vec![
            pend("a", 3, "Known by default"),
            pend("b", 3, "Known by default"),
            pend("c", 1, "Scrapping it"),
            // Value 1 shown two ways: both occurrences of value 1 become disagreements.
            pend("d", 1, "Picked up"),
        ];
        let names = settle(&mut tally);
        assert_eq!(count(&tally, "LRNM", "agree"), 2);
        assert_eq!(count(&tally, "LRNM", "disagree"), 2);
        assert_eq!(names["COBJ/LRNM"]["3"], "Known by default");
        assert_eq!(names["COBJ/LRNM"]["1"], "Picked up | Scrapping it");
    }

    fn node(leaves: &[(&str, &str)]) -> XNode {
        XNode {
            sig: "TEST".into(),
            occurrence: 0,
            name: "Test".into(),
            leaves: leaves
                .iter()
                .map(|(p, v)| XLeaf {
                    path: (*p).into(),
                    value: Some((*v).into()),
                })
                .collect(),
        }
    }

    #[test]
    fn alignment_tolerates_older_layouts_but_not_value_differences() {
        let m = map();
        let code = |k: &str, n: i128| Ours::Code { key: k.into(), n };
        let form = Ours::Form(Some(FormKey::Key("starfield.esm".into(), 0x329ABD)));
        // A count xEdit does not show, and a FormID it shows in another place.
        let ours = vec![
            code("T/A.Count", 3),
            form.clone(),
            Ours::Any,
            Ours::Bytes(b"Name\0".to_vec()),
            Ours::Num(0.5),
        ];
        let n = node(&[
            ("Name", "Name"),
            ("Unknown", "00 00"),
            ("Attach Point", "KYWD - Keyword [00329ABD] <ap>"),
            ("Value", "0.500000"),
        ]);
        let v = compare(&ours, true, &n, &m);
        assert!(v.agree && v.fewer, "{v:?}");
        assert_eq!(v.unmatched.len(), 1, "only the count is ours alone");
        // A value that differs is never hidden by the alignment.
        let ours = vec![form, Ours::Num(0.75)];
        let v = compare(&ours, true, &n, &m);
        assert!(!v.agree, "{v:?}");
        assert!(
            v.left.contains(&"Value=0.500000".to_string()),
            "{:?}",
            v.left
        );
        // A field order that differs: both values still pair, the enum by name.
        let e = Ours::Enum {
            key: "T/B.Type".into(),
            n: 0,
            name: Some("Ball".into()),
        };
        let n2 = node(&[("Type", "Bullet"), ("Speed", "45.000000")]);
        let v = compare(&[Ours::Num(45.0), e], true, &n2, &m);
        assert!(v.agree && !v.fewer, "{v:?}");
        assert_eq!(v.pairs, vec![("T/B.Type".into(), 0, "Bullet".into())]);
        // xEdit's other spellings of numbers.
        assert_eq!(judge(&code("k", 13), Some("Unknown 13"), &m), Hit::Yes);
        assert_eq!(
            judge(&Ours::Num(f64::from(f32::MAX)), Some("Default"), &m),
            Hit::Yes
        );
        assert_eq!(
            judge(&code("k", 13), Some("KYWD - Keyword [000546E1] <K>"), &m),
            Hit::No
        );
    }
}
