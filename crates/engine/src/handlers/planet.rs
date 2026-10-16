//! `planet` (M4.D3, alias `pcm`): the Planet Content Manager chain of one record, from the
//! typed FormID links lane B's planet and world definitions give (`kitkraft_doc::links`)
//! and the structure `docs/schema/planet-family.md` and `docs/design/planet-content.md`
//! describe.
//!
//! - A tree (`PCMT`), branch (`PCBN`) or content node (`PCCN`): the tree under it, built
//!   from the nodes' upward links (`PRTN` parent, `PRVN` previous sibling; the downward
//!   `PCCB` lists are never written in an official file), each node with its selection
//!   fields, keywords, conditions as stored, and its content (`PCCC`: a worldspace, pack-in
//!   or leveled pack-in) with one level of detail; for a branch or content node also its
//!   ancestors up to the tree. For a tree, the default objects (`DFOB`, no definition yet)
//!   whose payload holds its FormID, matched by the stored bytes.
//! - A planet (`PNDT`): its worldspaces, biomes, surface tree and star (`STDT` whose system
//!   ID equals the planet's `StarSystemId`, a corpus fact, not a FormID link), and every
//!   content node that places one of its worldspaces, with the branch chain above it.
//! - A star (`STDT`): its planets, as above.
//! - A biome (`BIOM`): the planets that list it, its objects by type, and the conditions of
//!   tree nodes and leveled pack-ins whose stored parameter bytes equal one of its keywords.
//! - A location (`LCTN`): the worldspaces that name it, their planets and placing nodes, and
//!   the keyword condition candidates as for a biome.
//! - A worldspace (`WRLD`), pack-in (`PKIN`) or leveled pack-in (`LVLP`): the content nodes
//!   that place it (with their chains), and for pack-ins the leveled lists and biomes that
//!   name it.
//!
//! Conditions decide which planets, biomes, systems and locations a branch applies to, but
//! their parameters are unions the schema keeps as raw bytes (`common/Condition.toml`):
//! they are shown as stored and never evaluated. Every link keeps its raw value; one that
//! does not resolve is listed, never guessed.

use std::collections::{BTreeMap, HashMap, HashSet};

use kitkraft_doc::backrefs::raw_candidates;
use kitkraft_doc::links::{decode_record, version_links, Link, LinkStatus};
use kitkraft_doc::query::{decode_cursor, encode_cursor};
use kitkraft_doc::search::{candidates, Scope};
use kitkraft_doc::typed::Decoded;
use kitkraft_doc::{GlobalForm, LoadOrder, RecordReader, Version};
use kitkraft_esm::HEADER_LEN;
use kitkraft_schema::Schema;
use serde_json::{json, Value};

use super::inspect::find_form;
use super::load::{acquire, schema_of};
use super::refs::{identity, link_evidence, link_json, sig_text, target_text, Notes};
use super::{failed, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;
use crate::shapes::{self, kind};

const ID: &str = "planet";

/// Default page of tree nodes.
const DEFAULT_LIMIT: usize = 200;

/// Largest page.
const MAX_LIMIT: usize = 5000;

/// Deepest tree walked, and longest ancestor chain followed.
const MAX_DEPTH: usize = 32;

/// Most nodes one tree walk visits.
const MAX_NODES: usize = 20_000;

/// One decoded subrecord: signature, member path, value, payload.
type Field = ([u8; 4], String, Decoded, Vec<u8>);

/// One record read once: its winning version, links and decoded values.
struct Rec {
    sig: [u8; 4],
    /// Load-order index of the plugin holding the winning version.
    plugin: u16,
    edid: Option<String>,
    file: String,
    label: String,
    offset: u64,
    links: Vec<Link>,
    fields: Vec<Field>,
}

impl Rec {
    /// Links in subrecord `sig`.
    fn links_in(&self, sig: &[u8; 4]) -> impl Iterator<Item = &Link> {
        let sig = *sig;
        self.links.iter().filter(move |l| l.subrecord == sig)
    }

    /// The first resolved form named in subrecord `sig`.
    fn first_form(&self, sig: &[u8; 4]) -> Option<GlobalForm> {
        self.links_in(sig)
            .find(|l| l.status == LinkStatus::Resolved)
            .and_then(|l| l.form)
    }

    /// A decoded value of this record as JSON (`value_json`).
    fn json(&self, lo: &LoadOrder, d: &Decoded, payload: &[u8]) -> Value {
        value_json(lo, self.plugin, d, payload)
    }

    /// Decoded values of subrecord `sig`.
    fn values(&self, sig: &[u8; 4]) -> impl Iterator<Item = &Field> {
        let sig = *sig;
        self.fields.iter().filter(move |f| f.0 == sig)
    }

    /// The first value whose member path ends with `name`.
    fn value_at(&self, name: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|f| f.1 == name || f.1.ends_with(&format!(".{name}")))
    }

    fn has(&self, sig: &[u8; 4]) -> bool {
        self.fields.iter().any(|f| &f.0 == sig)
    }
}

/// The text at `key` of a JSON object, or "".
fn text<'v>(v: &'v Value, key: &str) -> &'v str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// A decoded value of a record of `plugin` as JSON, with the `kind` markers of
/// `crate::shapes`: FormIDs as `formref`s resolved through the plugin's masters, string IDs
/// as `lstring`, opaque bytes as `{kind: "opaque", opaque: <hex>}`.
fn value_json(lo: &LoadOrder, plugin: u16, d: &Decoded, payload: &[u8]) -> Value {
    match d {
        Decoded::Int(i) => i64::try_from(*i).map_or_else(|_| json!(i.to_string()), |x| json!(x)),
        Decoded::Float(f) => {
            serde_json::Number::from_f64(*f).map_or_else(|| json!(f.to_string()), Value::Number)
        }
        Decoded::Form(raw) => shapes::formref(lo, plugin, *raw),
        Decoded::Sig(s) => json!(String::from_utf8_lossy(s)),
        Decoded::Text(t) => json!(t),
        Decoded::LStringId(id) => {
            json!({"kind": kind::LSTRING, "string_id": format!("0x{id:08X}")})
        }
        Decoded::Struct(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), value_json(lo, plugin, v, payload)))
                .collect(),
        ),
        Decoded::Array(items) => Value::Array(
            items
                .iter()
                .map(|i| value_json(lo, plugin, i, payload))
                .collect(),
        ),
        Decoded::Opaque { at, len } => {
            let bytes = at
                .checked_add(*len)
                .and_then(|end| payload.get(*at..end))
                .unwrap_or_default();
            let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
            json!({"kind": kind::OPAQUE, "opaque": hex})
        }
    }
}

/// The opaque bytes of member `name` of a decoded struct, read as a little-endian `u32`.
fn opaque_u32(d: &Decoded, payload: &[u8], name: &str) -> Option<u32> {
    let Decoded::Struct(fields) = d else {
        return None;
    };
    let (_, v) = fields.iter().find(|(k, _)| k == name)?;
    raw_u32(v, payload, 0)
}

/// The four stored bytes of a condition field as a `u32`: kept raw, or read through the
/// union variant its decider selects (a one-member struct named after the variant; lane D
/// types the CTDA parameters since 2026-10-15).
fn raw_u32(v: &Decoded, payload: &[u8], depth: usize) -> Option<u32> {
    match v {
        Decoded::Opaque { at, len: 4 } => {
            let bytes: [u8; 4] = payload.get(*at..at.checked_add(4)?)?.try_into().ok()?;
            Some(u32::from_le_bytes(bytes))
        }
        Decoded::Form(raw) => Some(*raw),
        Decoded::Int(i) => u32::try_from(*i).ok().or_else(|| {
            i32::try_from(*i)
                .ok()
                .map(|x| u32::from_le_bytes(x.to_le_bytes()))
        }),
        Decoded::Struct(fs) if fs.len() == 1 && depth < 2 => {
            raw_u32(&fs.first()?.1, payload, depth.saturating_add(1))
        }
        _ => None,
    }
}

/// The records of one call, read once each.
struct Graph<'a> {
    lo: &'a LoadOrder,
    schema: &'a Schema,
    reader: RecordReader,
    recs: HashMap<u64, Option<Rec>>,
    notes: Notes,
    by_type: HashMap<[u8; 4], Vec<GlobalForm>>,
}

impl<'a> Graph<'a> {
    fn new(lo: &'a LoadOrder, schema: &'a Schema) -> Self {
        Graph {
            lo,
            schema,
            reader: RecordReader::new(),
            recs: HashMap::new(),
            notes: Notes::default(),
            by_type: HashMap::new(),
        }
    }

    /// The winning version of `form`, read and decoded on first use.
    fn get(&mut self, form: GlobalForm) -> Option<&Rec> {
        if !self.recs.contains_key(&form.key()) {
            let rec = self.read(form);
            self.recs.insert(form.key(), rec);
        }
        self.recs.get(&form.key()).and_then(Option::as_ref)
    }

    fn read(&mut self, form: GlobalForm) -> Option<Rec> {
        let lo = self.lo;
        let v: Version = *lo.forms.winner(form)?;
        let (rec, links) = match version_links(lo, &mut self.reader, self.schema, v) {
            Ok(x) => x,
            Err(e) => {
                self.notes
                    .unreadable
                    .push(format!("{}: {e}", lo.form_label(form)));
                return None;
            }
        };
        let sig = rec.record.header.signature.0;
        if links.deleted {
            self.notes.deleted.push(lo.form_label(form));
        }
        self.notes.add_links(sig, &links);
        self.notes.compressed |= rec.compressed;
        let loc = lo.record(v.plugin, v.record)?;
        let edid = lo.edid(v.plugin, loc).map(str::to_string);
        let edid_first = edid.as_ref().and_then(|e| e.bytes().next());
        let fields = if links.deleted {
            Vec::new()
        } else {
            decode_record(lo, self.schema, v.plugin, &rec, edid_first)
                .into_iter()
                .filter_map(|d| {
                    let payload = rec.subs.get(d.index)?.data().to_vec();
                    Some((d.signature, d.layout.path, d.value.ok()?, payload))
                })
                .collect()
        };
        Some(Rec {
            sig,
            plugin: v.plugin,
            edid,
            file: lo
                .plugin(v.plugin)
                .map_or_else(String::new, |p| p.name.clone()),
            label: rec.label(),
            offset: rec.offset(),
            links: links.links,
            fields,
        })
    }

    /// Every winning form of record type `sig`.
    fn winners(&mut self, sig: [u8; 4]) -> Vec<GlobalForm> {
        let lo = self.lo;
        self.by_type
            .entry(sig)
            .or_insert_with(|| {
                candidates(lo, sig, Scope::Winner)
                    .iter()
                    .map(Version::form)
                    .collect()
            })
            .clone()
    }

    /// `(form, links naming one of `targets`)` for every winner of `sig` with such links.
    fn naming(&mut self, sig: [u8; 4], targets: &HashSet<u64>) -> Vec<(GlobalForm, Vec<Link>)> {
        let mut out = Vec::new();
        for f in self.winners(sig) {
            let Some(rec) = self.get(f) else { continue };
            let found: Vec<Link> = rec
                .links
                .iter()
                .filter(|l| l.form.is_some_and(|g| targets.contains(&g.key())))
                .cloned()
                .collect();
            if !found.is_empty() {
                out.push((f, found));
            }
        }
        out
    }

    fn label(&self, form: GlobalForm) -> String {
        self.lo.form_label(form)
    }

    /// `{form, signature, editor_id}` of a form.
    fn brief(&mut self, form: GlobalForm) -> Value {
        let (sig, edid) = identity(self.lo, form);
        json!({"form": self.label(form), "signature": sig, "editor_id": edid})
    }
}

/// Parent and content indexes of the trees: children by parent, content nodes by what they
/// place.
#[derive(Default)]
struct Pcm {
    children: HashMap<u64, Vec<GlobalForm>>,
    placing: HashMap<u64, Vec<GlobalForm>>,
    nodes: usize,
}

fn pcm_index(g: &mut Graph<'_>) -> Pcm {
    let mut pcm = Pcm::default();
    for sig in [*b"PCBN", *b"PCCN"] {
        for f in g.winners(sig) {
            let Some(rec) = g.get(f) else { continue };
            pcm.nodes = pcm.nodes.saturating_add(1);
            if let Some(parent) = rec.first_form(b"PRTN") {
                pcm.children.entry(parent.key()).or_default().push(f);
            }
            for l in rec.links_in(b"PCCC") {
                if let Some(t) = l.form {
                    pcm.placing.entry(t.key()).or_default().push(f);
                }
            }
        }
    }
    pcm
}

/// `kids` in sibling order: each chain starts at a node without a previous sibling among
/// them and follows `PRVN` links; nodes no chain reaches (a cycle, a previous sibling
/// elsewhere) follow, marked.
fn order(g: &mut Graph<'_>, mut kids: Vec<GlobalForm>) -> Vec<(GlobalForm, &'static str)> {
    kids.sort();
    let members: HashSet<u64> = kids.iter().map(|k| k.key()).collect();
    let mut prev: HashMap<u64, Option<GlobalForm>> = HashMap::new();
    let mut next: HashMap<u64, Vec<GlobalForm>> = HashMap::new();
    for k in &kids {
        let p = g
            .get(*k)
            .and_then(|r| r.first_form(b"PRVN"))
            .filter(|p| members.contains(&p.key()));
        prev.insert(k.key(), p);
        if let Some(p) = p {
            next.entry(p.key()).or_default().push(*k);
        }
    }
    let mut out = Vec::new();
    let mut placed: HashSet<u64> = HashSet::new();
    for k in &kids {
        if prev.get(&k.key()).copied().flatten().is_some() {
            continue;
        }
        let mut at = Some(*k);
        let mut first = true;
        while let Some(c) = at {
            if !placed.insert(c.key()) {
                break;
            }
            out.push((c, if first { "first" } else { "chained" }));
            first = false;
            let after = next.get(&c.key()).cloned().unwrap_or_default();
            if after.len() > 1 {
                for extra in after.iter().skip(1) {
                    if placed.insert(extra.key()) {
                        out.push((*extra, "shared-previous"));
                    }
                }
            }
            at = after.first().copied();
        }
    }
    for k in kids {
        if placed.insert(k.key()) {
            out.push((k, "unchained"));
        }
    }
    out
}

/// One node of a walked tree.
struct Flat {
    form: GlobalForm,
    depth: usize,
    parent: Option<GlobalForm>,
    order: &'static str,
}

/// The tree under `root`, depth first, siblings in order.
fn walk(g: &mut Graph<'_>, pcm: &Pcm, root: GlobalForm, truncated: &mut bool) -> Vec<Flat> {
    let mut out = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    let mut stack = vec![Flat {
        form: root,
        depth: 0,
        parent: None,
        order: "root",
    }];
    while let Some(node) = stack.pop() {
        if !seen.insert(node.form.key()) {
            g.notes.unreadable.push(format!(
                "{} is reached twice: a cycle in the tree links",
                g.label(node.form)
            ));
            continue;
        }
        if out.len() >= MAX_NODES {
            *truncated = true;
            break;
        }
        let kids = pcm
            .children
            .get(&node.form.key())
            .cloned()
            .unwrap_or_default();
        if node.depth >= MAX_DEPTH && !kids.is_empty() {
            *truncated = true;
        } else {
            let ordered = order(g, kids);
            for (k, how) in ordered.into_iter().rev() {
                stack.push(Flat {
                    form: k,
                    depth: node.depth.saturating_add(1),
                    parent: Some(node.form),
                    order: how,
                });
            }
        }
        out.push(node);
    }
    out
}

/// The chain above `form` (parent first, the tree last).
fn ancestors(g: &mut Graph<'_>, form: GlobalForm) -> Vec<GlobalForm> {
    let mut out = Vec::new();
    let mut seen: HashSet<u64> = HashSet::from([form.key()]);
    let mut at = form;
    while out.len() < MAX_DEPTH {
        let Some(parent) = g.get(at).and_then(|r| r.first_form(b"PRTN")) else {
            break;
        };
        if !seen.insert(parent.key()) {
            break;
        }
        out.push(parent);
        at = parent;
    }
    out
}

/// The conditions of a record as stored (`CTDA` with its strings).
fn conditions_json(lo: &LoadOrder, rec: &Rec) -> Vec<Value> {
    rec.values(b"CTDA")
        .map(|(_, path, d, payload)| {
            let mut v = rec.json(lo, d, payload);
            if let Some(obj) = v.as_object_mut() {
                obj.insert("member".into(), json!(path));
            }
            v
        })
        .collect()
}

/// A tree node as JSON.
fn node_json(g: &mut Graph<'_>, n: &Flat, evidence: &mut Vec<Evidence>) -> Value {
    let lo = g.lo;
    let parent = n.parent.map(|p| lo.form_label(p));
    let Some(rec) = g.get(n.form) else {
        return json!({"form": lo.form_label(n.form), "depth": n.depth, "parent": parent, "readable": false});
    };
    evidence.push(Evidence {
        file: rec.file.clone(),
        offset: rec.offset,
        length: HEADER_LEN as u64,
        record: Some(rec.label.clone()),
        subrecord: None,
    });
    let link = |sig: &[u8; 4]| -> Value {
        rec.links_in(sig)
            .next()
            .map_or(Value::Null, |l| link_json(lo, l))
    };
    let field = |sig: &[u8; 4]| -> Value {
        rec.values(sig)
            .next()
            .map_or(Value::Null, |(_, _, d, p)| rec.json(lo, d, p))
    };
    for l in rec
        .links
        .iter()
        .filter(|l| matches!(&l.subrecord, b"PRTN" | b"PRVN" | b"PCCC"))
    {
        evidence.push(link_evidence(&rec.file, &rec.label, rec.offset, l));
    }
    let keywords: Vec<Value> = rec
        .links_in(b"KWDA")
        .map(|l| json!(target_text(lo, l)))
        .collect();
    json!({
        "form": lo.form_label(n.form),
        "signature": sig_text(rec.sig),
        "editor_id": rec.edid,
        "depth": n.depth,
        "parent": parent,
        "order": n.order,
        "node_type": field(b"NAM1"),
        "child_selection": field(b"NAM2"),
        "consume_request_on_failure": field(b"NAM5"),
        "count_curve": link(b"NAM3"),
        "distribution_curve": link(b"NAM4"),
        "parent_link": link(b"PRTN"),
        "previous": link(b"PRVN"),
        "content": link(b"PCCC"),
        "override_content_properties": rec.has(b"IOVR"),
        "keywords": keywords,
        "conditions": conditions_json(lo, rec),
    })
}

/// One level of detail on what a content node places.
fn content_detail(
    g: &mut Graph<'_>,
    planets: &mut Option<HashMap<u64, Vec<GlobalForm>>>,
    target: GlobalForm,
) -> Value {
    let lo = g.lo;
    let Some(rec) = g.get(target) else {
        return Value::Null;
    };
    let sig = rec.sig;
    match &sig {
        b"PKIN" => json!({"cell": rec.links_in(b"CNAM").next().map(|l| link_json(lo, l))}),
        b"LVLP" => {
            let entries: Vec<Value> = rec
                .links_in(b"LVLO")
                .map(|l| json!({"path": l.path, "pack_in": target_text(lo, l), "status": l.status.as_str()}))
                .collect();
            json!({"entries": entries})
        }
        b"WRLD" => {
            let location = rec.links_in(b"XLCN").next().map(|l| link_json(lo, l));
            let index = planets.get_or_insert_with(|| planet_index(g));
            let on: Vec<Value> = index
                .get(&target.key())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|p| g.brief(p))
                .collect();
            json!({"location": location, "planets": on})
        }
        _ => Value::Null,
    }
}

/// Planets by the worldspaces they list (master and edited).
fn planet_index(g: &mut Graph<'_>) -> HashMap<u64, Vec<GlobalForm>> {
    let mut out: HashMap<u64, Vec<GlobalForm>> = HashMap::new();
    for p in g.winners(*b"PNDT") {
        let Some(rec) = g.get(p) else { continue };
        let worlds: Vec<GlobalForm> = rec
            .links
            .iter()
            .filter(|l| matches!(&l.subrecord, b"CNAM" | b"EOVR"))
            .filter_map(|l| l.form)
            .collect();
        for w in worlds {
            let list = out.entry(w.key()).or_default();
            if !list.contains(&p) {
                list.push(p);
            }
        }
    }
    out
}

/// The content nodes that place one of `targets`, each with the chain above it.
fn placements(
    g: &mut Graph<'_>,
    pcm: &Pcm,
    targets: &[GlobalForm],
    evidence: &mut Vec<Evidence>,
) -> Vec<Value> {
    let mut out = Vec::new();
    for t in targets {
        let nodes = pcm.placing.get(&t.key()).cloned().unwrap_or_default();
        for n in nodes {
            let chain = ancestors(g, n);
            let lo = g.lo;
            let chain_json: Vec<Value> = chain
                .iter()
                .map(|a| {
                    let (sig, edid, conditions, selection) = match g.get(*a) {
                        Some(r) => (
                            Some(sig_text(r.sig)),
                            r.edid.clone(),
                            conditions_json(lo, r),
                            r.values(b"NAM2").next().map(|(_, _, d, p)| r.json(lo, d, p)),
                        ),
                        None => (None, None, Vec::new(), None),
                    };
                    json!({"form": lo.form_label(*a), "signature": sig, "editor_id": edid, "child_selection": selection, "conditions": conditions})
                })
                .collect();
            let (edid, link) = match g.get(n) {
                Some(r) => {
                    let l = r.links_in(b"PCCC").find(|l| l.form == Some(*t)).cloned();
                    if let Some(l) = &l {
                        evidence.push(link_evidence(&r.file, &r.label, r.offset, l));
                    }
                    (r.edid.clone(), l.map(|l| link_json(lo, &l)))
                }
                None => (None, None),
            };
            out.push(json!({
                "node": lo.form_label(n),
                "editor_id": edid,
                "content": link,
                "tree": chain.last().map(|r| lo.form_label(*r)),
                "chain": chain_json,
            }));
        }
    }
    out
}

/// Conditions of tree nodes and leveled pack-ins whose stored parameter bytes equal one of
/// `keywords` as that record's plugin would store it.
fn keyword_candidates(g: &mut Graph<'_>, keywords: &[GlobalForm]) -> Vec<Value> {
    let mut out = Vec::new();
    if keywords.is_empty() {
        return out;
    }
    let lo = g.lo;
    for sig in [*b"PCMT", *b"PCBN", *b"LVLP"] {
        for f in g.winners(sig) {
            let Some(v) = lo.forms.winner(f).copied() else {
                continue;
            };
            let Some(Ok(map)) = lo.plugin(v.plugin).map(|p| &p.map) else {
                continue;
            };
            let wanted: Vec<(u32, GlobalForm)> = keywords
                .iter()
                .flat_map(|k| raw_candidates(map, *k).into_iter().map(move |r| (r, *k)))
                .collect();
            let Some(rec) = g.get(f) else { continue };
            for (i, (_, path, d, payload)) in rec.values(b"CTDA").enumerate() {
                for param in ["Parameter1", "Parameter2", "Parameter3", "ComparisonValue"] {
                    let Some(raw) = opaque_u32(d, payload, param) else {
                        continue;
                    };
                    if let Some((_, k)) = wanted.iter().find(|(r, _)| *r == raw) {
                        let function = match d {
                            Decoded::Struct(fs) => fs
                                .iter()
                                .find(|(n, _)| n == "Function")
                                .map(|(_, v)| rec.json(lo, v, payload)),
                            _ => None,
                        };
                        out.push(json!({
                            "node": lo.form_label(f),
                            "signature": sig_text(rec.sig),
                            "editor_id": rec.edid,
                            "condition": i,
                            "member": path,
                            "function": function,
                            "parameter": param,
                            "raw": format!("{raw:08X}"),
                            "keyword": lo.form_label(*k),
                            "match": "raw-bytes",
                        }));
                    }
                }
            }
        }
    }
    out
}

/// The default objects (`DFOB`, no definition) whose payload holds `form` as their plugin
/// stores it, at a four-byte boundary of a subrecord other than `EDID`.
fn default_objects(
    g: &mut Graph<'_>,
    form: GlobalForm,
    evidence: &mut Vec<Evidence>,
) -> Vec<Value> {
    let lo = g.lo;
    let mut out = Vec::new();
    for f in g.winners(*b"DFOB") {
        let Some(v) = lo.forms.winner(f).copied() else {
            continue;
        };
        let Some(Ok(map)) = lo.plugin(v.plugin).map(|p| &p.map) else {
            continue;
        };
        let wanted = raw_candidates(map, form);
        let Some(loc) = lo.record(v.plugin, v.record).copied() else {
            continue;
        };
        let Ok(rec) = g.reader.read(lo, v.plugin, &loc) else {
            continue;
        };
        let spans = kitkraft_doc::compare::spans(&rec);
        for (s, span) in rec.subs.iter().zip(spans) {
            if s.signature().0 == *b"EDID" {
                continue;
            }
            for (i, bytes) in s.data().as_chunks::<4>().0.iter().enumerate() {
                if !wanted.contains(&u32::from_le_bytes(*bytes)) {
                    continue;
                }
                let at = (i as u64).saturating_mul(4);
                let file = lo
                    .plugin(v.plugin)
                    .map_or_else(String::new, |p| p.name.clone());
                if let Some(o) = span.file_offset {
                    evidence.push(Evidence {
                        file: file.clone(),
                        offset: o.saturating_add(at),
                        length: 4,
                        record: Some(rec.label()),
                        subrecord: Some(s.signature().to_string()),
                    });
                }
                out.push(json!({
                    "form": lo.form_label(f),
                    "editor_id": lo.edid(v.plugin, &loc),
                    "subrecord": s.signature().to_string(),
                    "payload_offset": at,
                    "offset": span.file_offset.map(|o| o.saturating_add(at)),
                    "match": "raw-bytes",
                }));
            }
        }
    }
    out
}

/// Every link of the tree's nodes that names nothing usable, and every link whose target's
/// type the definition does not list.
fn tree_problems(g: &mut Graph<'_>, nodes: &[Flat]) -> (Vec<Value>, Vec<Value>) {
    let lo = g.lo;
    let mut unresolved = Vec::new();
    let mut mismatched = Vec::new();
    for n in nodes {
        let Some(rec) = g.get(n.form) else { continue };
        for l in &rec.links {
            if matches!(
                l.status,
                LinkStatus::Missing | LinkStatus::Unresolvable | LinkStatus::EngineReserved
            ) {
                unresolved.push(json!({"node": lo.form_label(n.form), "editor_id": rec.edid, "path": l.path, "raw": format!("{:08X}", l.raw), "status": l.status.as_str(), "to": l.form.map(|f| lo.form_label(f)), "error": l.error}));
            }
            if l.target_allowed() == Some(false) {
                mismatched.push(json!({"node": lo.form_label(n.form), "editor_id": rec.edid, "path": l.path, "to": target_text(lo, l), "targets": l.targets.iter().map(|t| sig_text(*t)).collect::<Vec<_>>()}));
            }
        }
    }
    (unresolved, mismatched)
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let schema =
        schema_of(session).ok_or_else(|| failed(ID, "the builtin schema does not load"))?;
    let loaded = acquire(session, args, ID)?;
    let form_arg = str_arg(ID, args, "form")?;
    let plugin = args.get("plugin").and_then(Value::as_str);
    let (form, matched_by) = find_form(ID, loaded, form_arg, plugin)?;
    let lo = &loaded.lo;
    let (sig, edid) = identity(lo, form);
    let Some(sig) = sig else {
        return Err(failed(
            ID,
            format!("{} has no version in the load order", lo.form_label(form)),
        ));
    };
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|l| usize::try_from(l).ok())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);
    let fp = {
        let mut h = kitkraft_doc::hash::Fnv::new();
        h.write_u64(lo.fingerprint());
        h.write_u64(form.key());
        h.write(b"planet");
        h.finish()
    };
    let start = match args.get("cursor").and_then(Value::as_str) {
        Some(c) => decode_cursor(c, fp).map_err(|e| failed(ID, e))?,
        None => 0,
    };
    let started = std::time::Instant::now();
    let mut g = Graph::new(lo, schema);
    let mut evidence = Vec::new();
    let mut limitations = loaded.limitations.clone();
    let mut data = serde_json::Map::new();
    data.insert("form".into(), json!(form_arg));
    data.insert("matched_by".into(), json!(matched_by));
    data.insert("global".into(), json!(lo.form_label(form)));
    data.insert("signature".into(), json!(sig));
    data.insert("editor_id".into(), json!(edid));
    let mut lines: Vec<String> = Vec::new();
    let title = format!(
        "{} {sig} {}",
        lo.form_label(form),
        edid.as_deref().unwrap_or("(no EditorID)")
    );
    let pcm = pcm_index(&mut g);
    let mut planets_by_world: Option<HashMap<u64, Vec<GlobalForm>>> = None;
    let mut next_cursor: Option<String> = None;
    match sig.as_str() {
        "PCMT" | "PCBN" | "PCCN" => {
            data.insert("kind".into(), json!("tree"));
            let chain = ancestors(&mut g, form);
            let chain_json: Vec<Value> = chain
                .iter()
                .rev()
                .enumerate()
                .map(|(i, a)| {
                    let flat = Flat {
                        form: *a,
                        depth: i,
                        parent: None,
                        order: "ancestor",
                    };
                    node_json(&mut g, &flat, &mut evidence)
                })
                .collect();
            let root = chain.last().copied().unwrap_or(form);
            data.insert("tree".into(), json!(lo.form_label(root)));
            data.insert("ancestors".into(), Value::Array(chain_json));
            if sig == "PCMT" {
                let dfob = default_objects(&mut g, form, &mut evidence);
                if !dfob.is_empty() {
                    limitations.push("default_objects: DFOB has no definition yet; a default object is listed when one of its subrecords holds the tree's FormID, as its plugin stores it, at a four-byte boundary (stored bytes, not a typed member)".to_string());
                }
                data.insert("default_objects".into(), Value::Array(dfob));
            }
            let mut truncated = false;
            let nodes = walk(&mut g, &pcm, form, &mut truncated);
            let (unresolved, mismatched) = tree_problems(&mut g, &nodes);
            let mut by_content: BTreeMap<String, u64> = BTreeMap::new();
            let (mut branches, mut contents, mut max_depth) = (0u64, 0u64, 0usize);
            for n in &nodes {
                max_depth = max_depth.max(n.depth);
                let Some(rec) = g.get(n.form) else { continue };
                match &rec.sig {
                    b"PCBN" => branches = branches.saturating_add(1),
                    b"PCCN" => {
                        contents = contents.saturating_add(1);
                        let key = rec
                            .links_in(b"PCCC")
                            .next()
                            .map_or_else(|| "none".to_string(), |l| match (l.status, l.target_signature) {
                                (LinkStatus::Resolved, Some(s)) => sig_text(s),
                                (s, _) => s.as_str().to_string(),
                            });
                        let c = by_content.entry(key).or_default();
                        *c = c.saturating_add(1);
                    }
                    _ => {}
                }
            }
            let total = nodes.len();
            let page: Vec<&Flat> = nodes.iter().skip(start).take(limit).collect();
            let end = start.saturating_add(page.len());
            next_cursor = (end < total).then(|| encode_cursor(fp, end));
            let mut list = Vec::new();
            for n in page {
                let mut v = node_json(&mut g, n, &mut evidence);
                if let Some(t) = g.get(n.form).and_then(|r| r.first_form(b"PCCC")) {
                    let detail = content_detail(&mut g, &mut planets_by_world, t);
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("content_detail".into(), detail);
                    }
                }
                let (nsig, nedid) = identity(lo, n.form);
                let content = g
                    .get(n.form)
                    .and_then(|r| r.links_in(b"PCCC").next().cloned())
                    .map(|l| format!(" -> {}", target_text(lo, &l)))
                    .unwrap_or_default();
                lines.push(format!(
                    "{}{} {} {}{content}",
                    "  ".repeat(n.depth.saturating_add(1)),
                    lo.form_label(n.form),
                    nsig.as_deref().unwrap_or("?"),
                    nedid.as_deref().unwrap_or("")
                ));
                list.push(v);
            }
            if truncated {
                limitations.push(format!(
                    "the tree walk stopped at {MAX_NODES} nodes or {MAX_DEPTH} levels"
                ));
            }
            limitations.push("the tree is assembled from the nodes' upward links (PRTN parent, PRVN previous sibling): official files never write the downward PCCB lists (docs/schema/planet-family.md); `order` says how a node was placed among its siblings (first, chained, shared-previous, unchained)".to_string());
            data.insert(
                "counts".into(),
                json!({"nodes": total, "branches": branches, "content_nodes": contents, "max_depth": max_depth, "by_content": by_content}),
            );
            data.insert("unresolved".into(), json!(unresolved));
            data.insert("target_mismatches".into(), json!(mismatched));
            data.insert("nodes".into(), Value::Array(list));
            data.insert("total".into(), json!(total));
            lines.insert(
                0,
                format!(
                    "{title}: tree {} with {total} node(s) ({branches} branch, {contents} content; by content {}), {} unresolved link(s), {} outside their target types{}",
                    lo.form_label(root),
                    by_content
                        .iter()
                        .map(|(k, v)| format!("{k} {v}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    unresolved.len(),
                    mismatched.len(),
                    if next_cursor.is_some() {
                        format!(", showing {start}..{end}")
                    } else {
                        String::new()
                    }
                ),
            );
        }
        "PNDT" => {
            data.insert("kind".into(), json!("planet"));
            let planet = planet_json(&mut g, form, &mut evidence, &mut lines);
            let worlds = planet_worlds(&mut g, form);
            let placed = placements(&mut g, &pcm, &worlds, &mut evidence);
            lines.insert(
                0,
                format!(
                    "{title}: {} worldspace(s), placed by {} content node(s)",
                    worlds.len(),
                    placed.len()
                ),
            );
            for p in &placed {
                lines.push(format!(
                    "  placed by {} {} under {}",
                    text(p, "node"),
                    text(p, "editor_id"),
                    p.get("tree").and_then(Value::as_str).unwrap_or("(no tree)")
                ));
            }
            data.insert("planet".into(), planet);
            data.insert("placements".into(), Value::Array(placed));
            limitations.push(planet_routing_note());
        }
        "STDT" => {
            data.insert("kind".into(), json!("star"));
            let star = g.get(form).map(|r| {
                (
                    r.value_at("SystemId").map(|(_, _, d, p)| r.json(lo, d, p)),
                    r.links.iter().map(|l| link_json(lo, l)).collect::<Vec<_>>(),
                )
            });
            let (system, links) = star.unwrap_or((None, Vec::new()));
            let planets = star_planets(&mut g, system.as_ref());
            let mut list = Vec::new();
            for p in &planets {
                let worlds = planet_worlds(&mut g, *p);
                let placed = placements(&mut g, &pcm, &worlds, &mut evidence);
                let b = g.brief(*p);
                lines.push(format!(
                    "  planet {} {}: {} worldspace(s), {} placing node(s)",
                    lo.form_label(*p),
                    text(&b, "editor_id"),
                    worlds.len(),
                    placed.len()
                ));
                list.push(json!({"planet": b, "worldspaces": worlds.iter().map(|w| lo.form_label(*w)).collect::<Vec<_>>(), "placements": placed}));
            }
            lines.insert(0, format!("{title}: {} planet(s)", planets.len()));
            data.insert("star".into(), json!({"system_id": system, "links": links}));
            data.insert("planets".into(), Value::Array(list));
            limitations.push(star_note());
            limitations.push(planet_routing_note());
        }
        "BIOM" => {
            data.insert("kind".into(), json!("biome"));
            let targets: HashSet<u64> = HashSet::from([form.key()]);
            let users = g.naming(*b"PNDT", &targets);
            let planets: Vec<Value> = users
                .iter()
                .map(|(p, links)| {
                    json!({"planet": lo.form_label(*p), "editor_id": identity(lo, *p).1, "paths": links.iter().map(|l| l.path.clone()).collect::<Vec<_>>()})
                })
                .collect();
            let (summary, keywords) = biome_json(&mut g, form);
            let candidates = keyword_candidates(&mut g, &keywords);
            lines.insert(
                0,
                format!(
                    "{title}: listed by {} planet(s); {} keyword condition candidate(s)",
                    planets.len(),
                    candidates.len()
                ),
            );
            data.insert("biome".into(), summary);
            data.insert("planets".into(), Value::Array(planets));
            data.insert("keyword_conditions".into(), Value::Array(candidates));
            limitations.push(candidate_note());
            limitations.push(planet_routing_note());
        }
        "LCTN" => {
            data.insert("kind".into(), json!("location"));
            let targets: HashSet<u64> = HashSet::from([form.key()]);
            let worlds: Vec<GlobalForm> = g
                .naming(*b"WRLD", &targets)
                .into_iter()
                .filter(|(_, links)| links.iter().any(|l| l.subrecord == *b"XLCN"))
                .map(|(w, _)| w)
                .collect();
            let index = planets_by_world.get_or_insert_with(|| planet_index(&mut g)).clone();
            let mut planets: Vec<GlobalForm> = worlds
                .iter()
                .flat_map(|w| index.get(&w.key()).cloned().unwrap_or_default())
                .collect();
            planets.sort();
            planets.dedup();
            let placed = placements(&mut g, &pcm, &worlds, &mut evidence);
            let (keywords, links) = match g.get(form) {
                Some(r) => (
                    r.links_in(b"KWDA").filter_map(|l| l.form).collect::<Vec<_>>(),
                    r.links
                        .iter()
                        .filter(|l| matches!(&l.subrecord, b"PNAM" | b"KWDA" | b"NAM1"))
                        .map(|l| link_json(lo, l))
                        .collect::<Vec<_>>(),
                ),
                None => (Vec::new(), Vec::new()),
            };
            let candidates = keyword_candidates(&mut g, &keywords);
            lines.insert(
                0,
                format!(
                    "{title}: named by {} worldspace(s) (XLCN) on {} planet(s), placed by {} content node(s); {} keyword condition candidate(s)",
                    worlds.len(),
                    planets.len(),
                    placed.len(),
                    candidates.len()
                ),
            );
            let worlds_json: Vec<Value> = worlds.iter().map(|w| g.brief(*w)).collect();
            let planets_json: Vec<Value> = planets.iter().map(|p| g.brief(*p)).collect();
            data.insert("location".into(), json!({"links": links}));
            data.insert("worldspaces".into(), Value::Array(worlds_json));
            data.insert("planets".into(), Value::Array(planets_json));
            data.insert("placements".into(), Value::Array(placed));
            data.insert("keyword_conditions".into(), Value::Array(candidates));
            limitations.push(candidate_note());
            limitations.push(planet_routing_note());
        }
        "WRLD" | "PKIN" | "LVLP" => {
            data.insert(
                "kind".into(),
                json!(if sig == "WRLD" { "worldspace" } else { "pack-in" }),
            );
            let placed = placements(&mut g, &pcm, &[form], &mut evidence);
            let detail = content_detail(&mut g, &mut planets_by_world, form);
            let mut extra = 0usize;
            if sig != "WRLD" {
                let targets: HashSet<u64> = HashSet::from([form.key()]);
                let in_lists: Vec<Value> = g
                    .naming(*b"LVLP", &targets)
                    .into_iter()
                    .map(|(f, links)| json!({"leveled": lo.form_label(f), "editor_id": identity(lo, f).1, "paths": links.iter().map(|l| l.path.clone()).collect::<Vec<_>>()}))
                    .collect();
                let in_biomes: Vec<Value> = g
                    .naming(*b"BIOM", &targets)
                    .into_iter()
                    .map(|(f, links)| json!({"biome": lo.form_label(f), "editor_id": identity(lo, f).1, "objects": links.len()}))
                    .collect();
                extra = in_lists.len().saturating_add(in_biomes.len());
                data.insert("in_leveled".into(), Value::Array(in_lists));
                data.insert("in_biomes".into(), Value::Array(in_biomes));
            }
            lines.insert(
                0,
                format!(
                    "{title}: placed by {} content node(s){}",
                    placed.len(),
                    if sig == "WRLD" {
                        String::new()
                    } else {
                        format!(", named by {extra} leveled pack-in(s) or biome(s)")
                    }
                ),
            );
            for p in &placed {
                lines.push(format!(
                    "  placed by {} {} under {}",
                    text(p, "node"),
                    text(p, "editor_id"),
                    p.get("tree").and_then(Value::as_str).unwrap_or("(no tree)")
                ));
            }
            data.insert("detail".into(), detail);
            data.insert("placements".into(), Value::Array(placed));
            limitations.push(planet_routing_note());
        }
        other => {
            return Err(failed(
                ID,
                format!(
                    "{} is a {other}: `planet` takes a PCMT, PCBN, PCCN, PNDT, STDT, BIOM, LCTN, WRLD, PKIN or LVLP",
                    lo.form_label(form)
                ),
            ))
        }
    }
    limitations.push("conditions are shown as stored: the function index is a number (its table is not transcribed) and the parameters, comparison value and reference are raw bytes whose type the function selects (common/Condition.toml); nothing here evaluates them".to_string());
    g.notes.limitations(&mut limitations);
    data.insert("next_cursor".into(), json!(next_cursor));
    data.insert("records_read".into(), json!(g.recs.len()));
    data.insert("tree_nodes_indexed".into(), json!(pcm.nodes));
    data.insert(
        "elapsed_ms".into(),
        json!(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
    );
    data.insert("summary".into(), json!(lines.join("\n")));
    Ok(Outcome {
        data: Value::Object(data),
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: Some(g.notes.unexamined.values().sum()),
            not_examined: vec![
                "DFOB (no definition: default objects matched by stored bytes only)".to_string(),
                "base form component blocks (BFCB..BFCE, kept raw): planet keywords, names and the content placement properties".to_string(),
            ],
        },
        limitations,
        evidence,
    })
}

fn planet_routing_note() -> String {
    "routing: which planets, biomes, systems and locations a tree branch applies to is decided by its conditions (planetary body, biome, system and location keywords, per docs/schema/planet-family.md), which are not evaluated here; a planet's own keywords and name sit in its component blocks, which the schema keeps raw".to_string()
}

fn star_note() -> String {
    "a star's planets are the PNDT whose body GalaxyData.StarSystemId equals the star's DNAM SystemId: a corpus fact (all 1,786 official PNDT, docs/schema/planet-family.md), not a FormID link".to_string()
}

fn candidate_note() -> String {
    "keyword_conditions: a condition is listed when one of its raw parameters (or comparison value) equals one of the keywords as that record's plugin stores it; the parameter's type depends on the function, which is not typed, so these are candidates (`match: raw-bytes`), not evaluated routes".to_string()
}

/// The worldspaces a planet lists (master, then edited).
fn planet_worlds(g: &mut Graph<'_>, planet: GlobalForm) -> Vec<GlobalForm> {
    let mut out: Vec<GlobalForm> = Vec::new();
    if let Some(rec) = g.get(planet) {
        for l in rec
            .links
            .iter()
            .filter(|l| matches!(&l.subrecord, b"CNAM" | b"EOVR"))
        {
            if let Some(f) = l.form {
                if !out.contains(&f) {
                    out.push(f);
                }
            }
        }
    }
    out
}

/// The planets whose `StarSystemId` equals `system`.
fn star_planets(g: &mut Graph<'_>, system: Option<&Value>) -> Vec<GlobalForm> {
    let Some(system) = system.and_then(Value::as_i64) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for p in g.winners(*b"PNDT") {
        let id = g.get(p).and_then(|r| {
            r.value_at("GalaxyData").and_then(|(_, _, d, _)| match d {
                Decoded::Struct(fs) => {
                    fs.iter()
                        .find(|(n, _)| n == "StarSystemId")
                        .and_then(|(_, v)| match v {
                            Decoded::Int(i) => i64::try_from(*i).ok(),
                            _ => None,
                        })
                }
                _ => None,
            })
        });
        if id == Some(system) {
            out.push(p);
        }
    }
    out
}

/// A planet's links and values, grouped.
fn planet_json(
    g: &mut Graph<'_>,
    planet: GlobalForm,
    evidence: &mut Vec<Evidence>,
    lines: &mut Vec<String>,
) -> Value {
    let lo = g.lo;
    let Some(rec) = g.get(planet) else {
        return Value::Null;
    };
    let worldspaces: Vec<Value> = rec
        .links
        .iter()
        .filter(|l| matches!(&l.subrecord, b"CNAM" | b"EOVR"))
        .map(|l| link_json(lo, l))
        .collect();
    let biomes: Vec<Value> = rec
        .links_in(b"PPBD")
        .filter(|l| l.chain.get(1).is_some_and(|c| c == "Biome"))
        .map(|l| link_json(lo, l))
        .collect();
    let other: Vec<Value> = rec
        .links
        .iter()
        .filter(|l| !matches!(&l.subrecord, b"CNAM" | b"EOVR" | b"PPBD"))
        .map(|l| link_json(lo, l))
        .collect();
    let resources: usize = rec
        .links_in(b"PPBD")
        .filter(|l| {
            l.chain
                .last()
                .is_some_and(|c| c == "ResourceGeneration" || c == "Resource")
        })
        .count();
    for l in rec
        .links
        .iter()
        .filter(|l| matches!(&l.subrecord, b"CNAM" | b"EOVR" | b"PPBD"))
        .take(200)
    {
        evidence.push(link_evidence(&rec.file, &rec.label, rec.offset, l));
    }
    let galaxy = rec
        .value_at("GalaxyData")
        .map(|(_, _, d, p)| rec.json(lo, d, p));
    let system = galaxy.as_ref().and_then(|v| v.get("StarSystemId")).cloned();
    for b in &biomes {
        lines.push(format!(
            "  biome {} {}",
            text(b, "to"),
            text(b, "to_editor_id")
        ));
    }
    let star = system.as_ref().and_then(|s| {
        let id = s.as_i64()?;
        let stars = g.winners(*b"STDT");
        stars.into_iter().find(|st| {
            g.get(*st)
                .and_then(|r| r.value_at("SystemId").map(|(_, _, d, _)| matches!(d, Decoded::Int(i) if i64::try_from(*i).ok() == Some(id))))
                .unwrap_or(false)
        })
    });
    let star_json = star.map(|s| g.brief(s));
    if let Some(s) = &star_json {
        lines.push(format!(
            "  star {} {}",
            text(s, "form"),
            text(s, "editor_id")
        ));
    }
    json!({
        "worldspaces": worldspaces,
        "biomes": biomes,
        "resource_links": resources,
        "galaxy": galaxy,
        "star": star_json,
        "other_links": other,
    })
}

/// A biome's summary, and its keywords.
fn biome_json(g: &mut Graph<'_>, biome: GlobalForm) -> (Value, Vec<GlobalForm>) {
    let lo = g.lo;
    let Some(rec) = g.get(biome) else {
        return (Value::Null, Vec::new());
    };
    let mut objects: BTreeMap<String, u64> = BTreeMap::new();
    for l in rec.links_in(b"GNAM") {
        let key = match (l.status, l.target_signature) {
            (LinkStatus::Resolved, Some(s)) => sig_text(s),
            (s, _) => s.as_str().to_string(),
        };
        let n = objects.entry(key).or_default();
        *n = n.saturating_add(1);
    }
    let keywords: Vec<GlobalForm> = rec.links_in(b"KWDA").filter_map(|l| l.form).collect();
    let other: Vec<Value> = rec
        .links
        .iter()
        .filter(|l| !matches!(&l.subrecord, b"GNAM" | b"KWDA" | b"PNAM"))
        .map(|l| link_json(lo, l))
        .collect();
    (
        json!({
            "objects": objects,
            "flora": rec.links_in(b"PNAM").count(),
            "keywords": keywords.iter().map(|k| target_text_form(lo, *k)).collect::<Vec<_>>(),
            "links": other,
        }),
        keywords,
    )
}

fn target_text_form(lo: &LoadOrder, f: GlobalForm) -> String {
    let (sig, edid) = identity(lo, f);
    format!(
        "{} {} {}",
        lo.form_label(f),
        sig.as_deref().unwrap_or("?"),
        edid.as_deref().unwrap_or("")
    )
    .trim_end()
    .to_string()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]
pub(crate) mod tests {
    use crate::handlers::load::tests::plugin;
    use crate::handlers::testdata::sub;
    use crate::Engine;
    use serde_json::{json, Value};
    use std::path::PathBuf;

    fn edid(e: &str) -> Vec<u8> {
        sub(b"EDID", &[e.as_bytes(), &[0]].concat())
    }

    fn u32s(v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn kw(name: &str, ids: &[u32]) -> Vec<u8> {
        [
            edid(name),
            sub(b"KSIZ", &u32s(&[ids.len() as u32])),
            sub(b"KWDA", &u32s(ids)),
        ]
        .concat()
    }

    /// A tree TreeA -> BranchA -> (ContentWorld -> WorldA, ContentPack -> PackA,
    /// ContentBad -> an unresolvable FormID), chained by PRVN; WorldA names LocA (keyword
    /// KwLoc), PlanetA lists WorldA and BiomeA and sits in StarA's system 7; BranchA tests
    /// KwLoc's FormID in a condition parameter; a default object names the tree.
    pub(crate) fn fixture(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kitkraft-engine-planet-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cnam = Vec::new();
        cnam.extend_from_slice(&1.5f64.to_le_bytes());
        cnam.extend_from_slice(&(-2.0f64).to_le_bytes());
        cnam.extend_from_slice(&0x803u32.to_le_bytes());
        // Biome, chance, Data3, resource generation, then empty fauna, keyword, flora
        // (entry size 9) and Data11 lists.
        let mut ppbd = u32s(&[0x804]);
        ppbd.extend_from_slice(&50.0f32.to_le_bytes());
        ppbd.extend(u32s(&[0, 0, 0, 0, 0, 9, 0]));
        // Type, 3 unused, comparison value, function 560, 2 unused, parameter 1 = KwLoc,
        // parameter 2, run on, reference, parameter 3.
        let mut ctda = vec![0u8; 4];
        ctda.extend_from_slice(&1.0f32.to_le_bytes());
        ctda.extend_from_slice(&560u16.to_le_bytes());
        ctda.extend_from_slice(&[0, 0]);
        ctda.extend(u32s(&[0x801, 0, 0, 0, 0]));
        let records: Vec<(&[u8; 4], u32, Vec<u8>)> = vec![
            (b"KYWD", 0x800, edid("KwBiome")),
            (b"KYWD", 0x801, edid("KwLoc")),
            (b"LCTN", 0x802, kw("LocA", &[0x801])),
            (
                b"WRLD",
                0x803,
                [edid("WorldA"), sub(b"XLCN", &u32s(&[0x802]))].concat(),
            ),
            (b"BIOM", 0x804, kw("BiomeA", &[0x800])),
            (
                b"STDT",
                0x805,
                [edid("StarA"), sub(b"DNAM", &u32s(&[7]))].concat(),
            ),
            (
                b"PNDT",
                0x806,
                [
                    edid("PlanetA"),
                    sub(b"CNAM", &cnam),
                    sub(b"PPBD", &ppbd),
                    sub(b"BDST", &[]),
                    sub(b"GNAM", &u32s(&[7, 0, 1])),
                    sub(b"BDED", &[]),
                ]
                .concat(),
            ),
            (
                b"PKIN",
                0x807,
                [edid("PackA"), sub(b"CNAM", &u32s(&[0x8FF]))].concat(),
            ),
            (
                b"PCMT",
                0x808,
                [
                    edid("TreeA"),
                    sub(b"NAM1", &u32s(&[0])),
                    sub(b"NAM2", &u32s(&[1])),
                ]
                .concat(),
            ),
            (
                b"PCBN",
                0x809,
                [
                    edid("BranchA"),
                    sub(b"NAM2", &u32s(&[0])),
                    sub(b"CITC", &u32s(&[1])),
                    sub(b"CTDA", &ctda),
                    sub(b"PRTN", &u32s(&[0x808])),
                ]
                .concat(),
            ),
            (
                b"PCCN",
                0x80A,
                [
                    edid("ContentWorld"),
                    sub(b"PCCC", &u32s(&[0x803])),
                    sub(b"PRTN", &u32s(&[0x809])),
                ]
                .concat(),
            ),
            (
                b"PCCN",
                0x80B,
                [
                    edid("ContentPack"),
                    sub(b"PCCC", &u32s(&[0x807])),
                    sub(b"PRTN", &u32s(&[0x809])),
                    sub(b"PRVN", &u32s(&[0x80A])),
                ]
                .concat(),
            ),
            (
                b"PCCN",
                0x80C,
                [
                    edid("ContentBad"),
                    sub(b"PCCC", &u32s(&[0x0500_0001])),
                    sub(b"PRTN", &u32s(&[0x809])),
                    sub(b"PRVN", &u32s(&[0x80B])),
                ]
                .concat(),
            ),
            (
                b"DFOB",
                0x80D,
                [edid("DefaultTree"), sub(b"DATA", &u32s(&[0x808]))].concat(),
            ),
        ];
        std::fs::write(dir.join("Base.esm"), plugin(0x1, &[], &records)).unwrap();
        dir
    }

    fn run(engine: &Engine, dir: &std::path::Path, form: &str) -> Value {
        engine
            .execute(
                "planet",
                json!({"form": form, "plugins": ["Base.esm"], "data_dir": dir.display().to_string(), "no_cache": true}),
            )
            .unwrap()
    }

    fn forms(v: &Value, key: &str) -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x[key].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn a_tree_is_walked_from_its_upward_links() {
        let dir = fixture("tree");
        let engine = Engine::with_default_handlers();
        let r = run(&engine, &dir, "TreeA");
        let d = &r["data"];
        assert_eq!(d["kind"], "tree");
        let nodes = d["nodes"].as_array().unwrap();
        let order: Vec<(&str, i64, &str)> = nodes
            .iter()
            .map(|n| {
                (
                    n["editor_id"].as_str().unwrap(),
                    n["depth"].as_i64().unwrap(),
                    n["order"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            order,
            [
                ("TreeA", 0, "root"),
                ("BranchA", 1, "first"),
                ("ContentWorld", 2, "first"),
                ("ContentPack", 2, "chained"),
                ("ContentBad", 2, "chained"),
            ]
        );
        assert_eq!(
            d["counts"]["by_content"],
            json!({"PKIN": 1, "WRLD": 1, "unresolvable": 1})
        );
        // The one unresolved link is listed with its raw value and reason.
        let bad = d["unresolved"].as_array().unwrap();
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert_eq!(bad[0]["path"], "PCCC.Content");
        assert_eq!(bad[0]["raw"], "05000001");
        assert_eq!(bad[0]["status"], "unresolvable");
        // Conditions as stored: the function index, parameter 1 typed by the function (HasKeyword-style: a keyword).
        let cond = &nodes[1]["conditions"][0];
        assert_eq!(cond["Function"], 560);
        assert_eq!(
            cond["Parameter1"]["Keyword"]["form"], "Base.esm:000801",
            "{cond}"
        );
        // The worldspace content: its location and the planet that lists it.
        let world = &nodes[2];
        assert_eq!(world["content"]["to"], "Base.esm:000803");
        assert_eq!(world["content_detail"]["location"]["to"], "Base.esm:000802");
        assert_eq!(
            world["content_detail"]["planets"][0]["editor_id"],
            "PlanetA"
        );
        assert_eq!(nodes[3]["content_detail"]["cell"]["status"], "missing");
        // The default object, by its stored bytes.
        assert_eq!(forms(&d["default_objects"], "form"), ["Base.esm:00080D"]);
        assert!(r["limitations"].to_string().contains("raw bytes"));
        // A content node: its chain up to the tree, and the node itself.
        let r = run(&engine, &dir, "ContentPack");
        assert_eq!(r["data"]["tree"], "Base.esm:000808");
        assert_eq!(
            forms(&r["data"]["ancestors"], "editor_id"),
            ["TreeA", "BranchA"]
        );
        assert_eq!(r["data"]["nodes"].as_array().unwrap().len(), 1);
        // Paging, through the alias.
        let p = engine
            .execute(
                "pcm",
                json!({"form": "TreeA", "limit": 2, "plugins": ["Base.esm"], "data_dir": dir.display().to_string(), "no_cache": true}),
            )
            .unwrap();
        assert_eq!(p["data"]["nodes"].as_array().unwrap().len(), 2);
        assert!(p["data"]["next_cursor"].is_string());
    }

    #[test]
    fn planets_stars_biomes_and_locations_reach_their_placements() {
        let dir = fixture("planet");
        let engine = Engine::with_default_handlers();
        let r = run(&engine, &dir, "PlanetA");
        let d = &r["data"];
        assert_eq!(d["kind"], "planet");
        assert_eq!(d["planet"]["worldspaces"][0]["to"], "Base.esm:000803");
        assert_eq!(d["planet"]["biomes"][0]["to_editor_id"], "BiomeA");
        assert_eq!(d["planet"]["star"]["editor_id"], "StarA");
        let placed = d["placements"].as_array().unwrap();
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0]["editor_id"], "ContentWorld");
        assert_eq!(placed[0]["tree"], "Base.esm:000808");
        assert_eq!(
            forms(&placed[0]["chain"], "editor_id"),
            ["BranchA", "TreeA"]
        );

        let r = run(&engine, &dir, "StarA");
        assert_eq!(r["data"]["planets"][0]["planet"]["editor_id"], "PlanetA");
        assert_eq!(
            r["data"]["planets"][0]["placements"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let r = run(&engine, &dir, "BiomeA");
        assert_eq!(forms(&r["data"]["planets"], "planet"), ["Base.esm:000806"]);
        assert!(r["data"]["keyword_conditions"]
            .as_array()
            .unwrap()
            .is_empty());

        let r = run(&engine, &dir, "LocA");
        let d = &r["data"];
        assert_eq!(forms(&d["worldspaces"], "editor_id"), ["WorldA"]);
        assert_eq!(forms(&d["planets"], "editor_id"), ["PlanetA"]);
        assert_eq!(d["placements"].as_array().unwrap().len(), 1);
        let c = &d["keyword_conditions"][0];
        assert_eq!(c["editor_id"], "BranchA");
        assert_eq!(c["parameter"], "Parameter1");
        assert_eq!(c["keyword"], "Base.esm:000801");
        assert_eq!(c["match"], "raw-bytes");

        let r = run(&engine, &dir, "PackA");
        assert_eq!(r["data"]["placements"][0]["editor_id"], "ContentPack");

        // Not a planet-family record.
        let e = engine
            .execute(
                "planet",
                json!({"form": "KwLoc", "plugins": ["Base.esm"], "data_dir": dir.display().to_string(), "no_cache": true}),
            )
            .unwrap_err();
        assert!(e.to_string().contains("is a KYWD"), "{e}");
    }
}
