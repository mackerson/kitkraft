//! `refs` (M4.D1, extends M3.D4): the typed FormID links of a form, outgoing (what its
//! winning version names) or incoming (which records name it: the CK's "Use Info"),
//! followed `depth` hops, a page at a time.
//!
//! Links come from `kitkraft_doc::links` (outgoing: every value of a member the schema types
//! as `formid`, with its member path, raw value, target types, offset and status) and
//! `kitkraft_doc::backrefs` (incoming: a reverse index over every plugin of the load order,
//! built from the generated decoder's `form_ids()` and cached beside the record index).
//! Raw values are kept beside resolved identities; a value that names nothing (`null`,
//! `none`), an engine-reserved form, a form no plugin has (`missing`) and a value that
//! does not resolve (`unresolvable`, with the reason) are marked as such, never guessed.
//! Incoming referrers that sit in a cell (placed references and other cell children) are
//! grouped after the other referrers, each with its cell, world and persistence.

use std::collections::{BTreeMap, HashSet, VecDeque};

use kitkraft_doc::backrefs::{BackrefCache, Backrefs, Obtained, Placement};
use kitkraft_doc::links::{version_links, Link, LinkStatus};
use kitkraft_doc::query::{decode_cursor, encode_cursor};
use kitkraft_doc::{GlobalForm, LoadOrder, RecordReader, Version};
use kitkraft_esm::HEADER_LEN;
use kitkraft_schema::Schema;
use serde_json::{json, Value};

use super::inspect::find_form;
use super::load::{acquire, schema_of, Loaded};
use super::{failed, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

const ID: &str = "refs";

/// Default page size.
const DEFAULT_LIMIT: usize = 100;

/// Largest page.
const MAX_LIMIT: usize = 1000;

/// Most forms a multi-hop walk expands.
pub(super) const MAX_NODES: usize = 5_000;

/// Most edges one call collects before paging.
const MAX_EDGES: usize = 500_000;

/// The record type and EditorID of `form`'s winning version.
pub(super) fn identity(lo: &LoadOrder, form: GlobalForm) -> (Option<String>, Option<String>) {
    let Some(w) = lo.forms.winner(form) else {
        return (None, None);
    };
    let Some(loc) = lo.record(w.plugin, w.record) else {
        return (None, None);
    };
    (
        Some(String::from_utf8_lossy(&loc.signature).into_owned()),
        lo.edid(w.plugin, loc).map(str::to_string),
    )
}

/// Text of a record type.
pub(super) fn sig_text(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

/// A link as JSON: path, raw value, status, the target (label, record type, EditorID),
/// the definition's target types and whether the target's type is one of them.
pub(super) fn link_json(lo: &LoadOrder, l: &Link) -> Value {
    let (to_sig, to_edid) = l.form.map_or((None, None), |g| identity(lo, g));
    json!({
        "path": l.path,
        "raw": format!("{:08X}", l.raw),
        "status": l.status.as_str(),
        "to": l.form.map(|g| lo.form_label(g)),
        "to_signature": to_sig,
        "to_editor_id": to_edid,
        "targets": l.targets.iter().map(|t| sig_text(*t)).collect::<Vec<_>>(),
        "target_allowed": l.target_allowed(),
        "resolved_by": l.basis.map(|b| b.as_str()),
        "error": l.error,
        "offset": l.file_offset,
        "body_offset": l.body_offset,
    })
}

/// Evidence for a link: its four bytes, or the record header inside a compressed record.
pub(super) fn link_evidence(file: &str, record: &str, rec_offset: u64, l: &Link) -> Evidence {
    match l.file_offset {
        Some(at) => Evidence {
            file: file.to_string(),
            offset: at,
            length: 4,
            record: Some(record.to_string()),
            subrecord: Some(sig_text(l.subrecord)),
        },
        None => Evidence {
            file: file.to_string(),
            offset: rec_offset,
            length: HEADER_LEN as u64,
            record: Some(record.to_string()),
            subrecord: None,
        },
    }
}

/// What a walk did not scan, and what it found, for `limitations`.
#[derive(Default)]
pub(super) struct Notes {
    pub unexamined: BTreeMap<(String, &'static str), u64>,
    pub opaque_members: u64,
    pub statuses: BTreeMap<&'static str, u64>,
    pub disallowed: u64,
    pub compressed: bool,
    pub deleted: Vec<String>,
    pub unreadable: Vec<String>,
}

impl Notes {
    pub(super) fn add_links(
        &mut self,
        record_sig: [u8; 4],
        links: &kitkraft_doc::links::RecordLinks,
    ) {
        for (sig, _, why) in &links.unexamined {
            let key = (
                format!("{}.{}", sig_text(record_sig), sig_text(*sig)),
                why.as_str(),
            );
            let n = self.unexamined.entry(key).or_default();
            *n = n.saturating_add(1);
        }
        self.opaque_members = self.opaque_members.saturating_add(links.opaque_members);
    }

    pub(super) fn add_link(&mut self, l: &Link) {
        let n = self.statuses.entry(l.status.as_str()).or_default();
        *n = n.saturating_add(1);
        if l.target_allowed() == Some(false) {
            self.disallowed = self.disallowed.saturating_add(1);
        }
    }

    pub(super) fn limitations(&self, out: &mut Vec<String>) {
        out.push(
            "links are the FormID values of members the schema types as `formid` (lane B's definitions); bytes the schema keeps raw are not read, so a link inside them is unknown, not absent".to_string(),
        );
        if !self.unexamined.is_empty() {
            let mut by_why: BTreeMap<&str, u64> = BTreeMap::new();
            for ((_, why), n) in &self.unexamined {
                let t = by_why.entry(why).or_default();
                *t = t.saturating_add(*n);
            }
            let examples: Vec<String> = self
                .unexamined
                .iter()
                .take(12)
                .map(|((at, why), n)| format!("{at} {why} x{n}"))
                .collect();
            out.push(format!(
                "subrecords not scanned for FormIDs: {} ({})",
                by_why
                    .iter()
                    .map(|(w, n)| format!("{w} {n}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                examples.join("; ")
            ));
        }
        if self.opaque_members > 0 {
            out.push(format!(
                "{} member(s) of type `opaque` inside decoded subrecords were not scanned (unions the format cannot express yet, unnamed bytes)",
                self.opaque_members
            ));
        }
        if self.statuses.contains_key("engine_reserved") {
            out.push("engine_reserved: an object ID below 0x800 of Starfield.esm that no plugin defines (player reference 0x14, built-in actor values, ...): a corpus fact, grade D until a CK observation (docs/validate.md)".to_string());
        }
        if self.statuses.contains_key("missing") {
            out.push("missing: the value resolves to a plugin and object ID, but no plugin of the load order has a version of that form".to_string());
        }
        if self.statuses.contains_key("unresolvable") {
            out.push("unresolvable: the raw value's master is not loaded or its index is past the master list; it is kept raw, never guessed (ADR 0004, owner clarification 5)".to_string());
        }
        if self.disallowed > 0 {
            out.push(format!(
                "{} link(s) name a record type their definition does not list (`target_allowed: false`); target lists are upstream's (reference), not checked by the corpus",
                self.disallowed
            ));
        }
        if self.compressed {
            out.push("a link inside a compressed record has no file offset: `body_offset` is its offset in the inflated body, and the evidence names the record header".to_string());
        }
        if !self.deleted.is_empty() {
            out.push(format!(
                "deleted versions (record flag 0x20) are not scanned: {}",
                self.deleted
                    .iter()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        for u in self.unreadable.iter().take(10) {
            out.push(u.clone());
        }
    }
}

/// The record types named by `types`.
fn types_arg(args: &Value) -> Result<Option<Vec<[u8; 4]>>, EngineError> {
    let Some(list) = args.get("types").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for t in list {
        let text = t.as_str().unwrap_or_default();
        let sig: [u8; 4] = text
            .to_ascii_uppercase()
            .as_bytes()
            .try_into()
            .map_err(|_| failed(ID, format!("types: `{text}` is not a record type")))?;
        out.push(sig);
    }
    Ok(Some(out))
}

/// The reverse index of the session's load order, built (or read from the cache) on
/// first use.
pub(super) fn backrefs<'l>(loaded: &'l Loaded, schema: &Schema) -> &'l Backrefs {
    loaded.backrefs.get_or_init(|| {
        let cache = loaded.cache_dir.as_ref().map(BackrefCache::new);
        Backrefs::build(&loaded.lo, schema, cache.as_ref(), 0)
    })
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
    if lo.forms.versions(form).is_empty() {
        return Err(failed(
            ID,
            format!("{} has no version in the load order", lo.form_label(form)),
        ));
    }
    let incoming = args.get("incoming").and_then(Value::as_bool) == Some(true);
    let depth = args
        .get("depth")
        .and_then(Value::as_u64)
        .and_then(|d| u32::try_from(d).ok())
        .unwrap_or(1)
        .max(1);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|l| usize::try_from(l).ok())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);
    let all_versions = args.get("scope").and_then(Value::as_str) == Some("all");
    let types = types_arg(args)?;
    let fp = {
        let mut h = kitkraft_doc::hash::Fnv::new();
        h.write_u64(lo.fingerprint());
        h.write_u64(form.key());
        h.write(&[u8::from(incoming), u8::from(all_versions)]);
        h.write(&depth.to_le_bytes());
        for t in types.iter().flatten() {
            h.write(t);
        }
        h.finish()
    };
    let start = match args.get("cursor").and_then(Value::as_str) {
        Some(c) => decode_cursor(c, fp).map_err(|e| failed(ID, e))?,
        None => 0,
    };
    let started = std::time::Instant::now();
    let (sig, edid) = identity(lo, form);
    let head = Head {
        form_arg,
        matched_by,
        form,
        sig,
        edid,
        depth,
        limit,
        start,
        fp,
        types,
    };
    let mut out = if incoming {
        incoming_page(loaded, schema, &head, all_versions)?
    } else {
        outgoing_page(loaded, schema, &head)?
    };
    if let Some(obj) = out.data.as_object_mut() {
        obj.insert(
            "elapsed_ms".into(),
            json!(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
        );
    }
    Ok(out)
}

/// What both directions share.
struct Head<'a> {
    form_arg: &'a str,
    matched_by: &'static str,
    form: GlobalForm,
    sig: Option<String>,
    edid: Option<String>,
    depth: u32,
    limit: usize,
    start: usize,
    fp: u64,
    types: Option<Vec<[u8; 4]>>,
}

/// One outgoing link and the version it was read from.
struct OutEdge {
    depth: u32,
    from: GlobalForm,
    version: Version,
    record: String,
    record_offset: u64,
    link: Link,
}

fn outgoing_page(
    loaded: &Loaded,
    schema: &Schema,
    head: &Head<'_>,
) -> Result<Outcome, EngineError> {
    let lo = &loaded.lo;
    let mut reader = RecordReader::new();
    let mut notes = Notes::default();
    let mut edges: Vec<OutEdge> = Vec::new();
    let mut visited: HashSet<u64> = HashSet::from([head.form.key()]);
    let mut queue: VecDeque<(GlobalForm, u32)> = VecDeque::from([(head.form, 0)]);
    let mut truncated = false;
    let mut nodes = 0usize;
    while let Some((f, d)) = queue.pop_front() {
        if d >= head.depth {
            continue;
        }
        let Some(v) = lo.forms.winner(f).copied() else {
            continue;
        };
        nodes = nodes.saturating_add(1);
        let (rec, links) = match version_links(lo, &mut reader, schema, v) {
            Ok(x) => x,
            Err(e) => {
                notes.unreadable.push(format!("{}: {e}", lo.form_label(f)));
                continue;
            }
        };
        let record_sig = rec.record.header.signature.0;
        if links.deleted {
            notes.deleted.push(lo.form_label(f));
        }
        notes.add_links(record_sig, &links);
        notes.compressed |= rec.compressed;
        let label = rec.label();
        for l in links.links {
            if let (Some(g), LinkStatus::Resolved) = (l.form, l.status) {
                let next = d.saturating_add(1);
                if next < head.depth && !visited.contains(&g.key()) {
                    if visited.len() >= MAX_NODES {
                        truncated = true;
                    } else {
                        visited.insert(g.key());
                        queue.push_back((g, next));
                    }
                }
            }
            if let Some(types) = &head.types {
                if !l.target_signature.is_some_and(|s| types.contains(&s)) {
                    continue;
                }
            }
            notes.add_link(&l);
            if edges.len() >= MAX_EDGES {
                truncated = true;
                break;
            }
            edges.push(OutEdge {
                depth: d.saturating_add(1),
                from: f,
                version: v,
                record: label.clone(),
                record_offset: rec.offset(),
                link: l,
            });
        }
    }
    let total = edges.len();
    let page: Vec<&OutEdge> = edges.iter().skip(head.start).take(head.limit).collect();
    let end = head.start.saturating_add(page.len());
    let next_cursor = (end < total).then(|| encode_cursor(head.fp, end));
    let mut evidence = Vec::new();
    let mut lines = Vec::new();
    let list: Vec<Value> = page
        .iter()
        .map(|e| {
            let file = lo
                .plugin(e.version.plugin)
                .map_or_else(String::new, |p| p.name.clone());
            evidence.push(link_evidence(&file, &e.record, e.record_offset, &e.link));
            let (from_sig, from_edid) = identity(lo, e.from);
            let mut v = link_json(lo, &e.link);
            if let Some(obj) = v.as_object_mut() {
                obj.insert("depth".into(), json!(e.depth));
                obj.insert("from".into(), json!(lo.form_label(e.from)));
                obj.insert("from_signature".into(), json!(from_sig));
                obj.insert("from_editor_id".into(), json!(from_edid));
                obj.insert("plugin".into(), json!(file));
            }
            lines.push(format!(
                "  {}{} -> {}",
                if e.depth > 1 {
                    format!("[{}] {} ", e.depth, lo.form_label(e.from))
                } else {
                    String::new()
                },
                e.link.path,
                target_text(lo, &e.link)
            ));
            v
        })
        .collect();
    let mut limitations = loaded.limitations.clone();
    notes.limitations(&mut limitations);
    if truncated {
        limitations.push(format!(
            "the walk stopped at {MAX_NODES} forms or {MAX_EDGES} links: lower `depth`"
        ));
    }
    let summary = format!(
        "{} {} {}: {} outgoing link(s), depth {} ({}){}\n{}",
        lo.form_label(head.form),
        head.sig.as_deref().unwrap_or("?"),
        head.edid.as_deref().unwrap_or("(no EditorID)"),
        total,
        head.depth,
        status_text(&notes.statuses),
        if next_cursor.is_some() {
            format!(", showing {}..{end}", head.start)
        } else {
            String::new()
        },
        lines.join("\n")
    );
    let data = json!({
        "form": head.form_arg,
        "matched_by": head.matched_by,
        "global": lo.form_label(head.form),
        "signature": head.sig,
        "editor_id": head.edid,
        "direction": "outgoing",
        "depth": head.depth,
        "forms_read": nodes,
        "total": total,
        "counts": {"by_status": notes.statuses},
        "edges": list,
        "next_cursor": next_cursor,
        "truncated": truncated,
        "summary": summary,
    });
    Ok(Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: Some(notes.unexamined.values().sum()),
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
}

/// `-> Starfield.esm:000123 AMMO Ammo_Foo`, or the raw value and its status.
pub(super) fn target_text(lo: &LoadOrder, l: &Link) -> String {
    match (l.status, l.form) {
        (LinkStatus::Resolved, Some(g)) => {
            let (sig, edid) = identity(lo, g);
            format!(
                "{} {} {}",
                lo.form_label(g),
                sig.as_deref().unwrap_or("?"),
                edid.as_deref().unwrap_or("")
            )
            .trim_end()
            .to_string()
        }
        (status, Some(g)) => format!("{} ({})", lo.form_label(g), status.as_str()),
        (status, None) => format!("{:08X} ({})", l.raw, status.as_str()),
    }
}

fn status_text(statuses: &BTreeMap<&'static str, u64>) -> String {
    if statuses.is_empty() {
        return "none".to_string();
    }
    statuses
        .iter()
        .map(|(k, v)| format!("{v} {k}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One incoming referrer version.
struct InHit {
    depth: u32,
    target: GlobalForm,
    plugin: u16,
    record: u32,
    placement: Option<Placement>,
}

/// A cell or world named by a stored FormID of `plugin`: label and EditorID.
fn place_json(lo: &LoadOrder, plugin: u16, raw: u32) -> Value {
    match lo.resolve(plugin, raw) {
        Ok(g) => {
            let (_, edid) = identity(lo, g);
            json!({"form": lo.form_label(g), "raw": format!("{raw:08X}"), "editor_id": edid})
        }
        Err(e) => json!({"form": null, "raw": format!("{raw:08X}"), "error": e.to_string()}),
    }
}

fn placement_json(lo: &LoadOrder, plugin: u16, p: &Placement) -> Value {
    json!({
        "cell": place_json(lo, plugin, p.cell_raw),
        "world": p.world_raw.map(|w| place_json(lo, plugin, w)),
        "persistent": p.group_type == kitkraft_esm::cells::group_type::CELL_PERSISTENT,
        "group_type": p.group_type,
    })
}

fn incoming_page(
    loaded: &Loaded,
    schema: &Schema,
    head: &Head<'_>,
    all_versions: bool,
) -> Result<Outcome, EngineError> {
    let lo = &loaded.lo;
    let index = backrefs(loaded, schema);
    let mut skipped: Vec<String> = Vec::new();
    let mut hits: Vec<InHit> = Vec::new();
    let mut visited: HashSet<u64> = HashSet::from([head.form.key()]);
    let mut frontier = vec![head.form];
    let mut truncated = false;
    for d in 0..head.depth {
        let mut next = Vec::new();
        for t in &frontier {
            let (found, skip) = index.referrers(lo, *t);
            for s in skip {
                if !skipped.contains(&s) {
                    skipped.push(s);
                }
            }
            for (p, r) in found {
                let Some(loc) = lo.record(p, r) else { continue };
                let g = lo.resolve(p, loc.form_id).ok();
                let winner = g
                    .and_then(|g| lo.forms.winner(g))
                    .is_some_and(|w| w.plugin == p && w.record == r);
                if !all_versions && !winner {
                    continue;
                }
                if let Some(g) = g {
                    if d.saturating_add(1) < head.depth && !visited.contains(&g.key()) {
                        if visited.len() >= MAX_NODES {
                            truncated = true;
                        } else {
                            visited.insert(g.key());
                            next.push(g);
                        }
                    }
                }
                if let Some(types) = &head.types {
                    if !types.contains(&loc.signature) {
                        continue;
                    }
                }
                if hits.len() >= MAX_EDGES {
                    truncated = true;
                    break;
                }
                let placement = index
                    .plugins
                    .get(usize::from(p))
                    .and_then(Option::as_ref)
                    .and_then(|b| b.placement(r));
                hits.push(InHit {
                    depth: d.saturating_add(1),
                    target: *t,
                    plugin: p,
                    record: r,
                    placement,
                });
            }
        }
        frontier = next;
    }
    // Other referrers first, then placed ones; each in load order, then index order.
    hits.sort_by_key(|h| (h.depth, h.placement.is_some(), h.plugin, h.record));
    let total = hits.len();
    let placed_total = hits.iter().filter(|h| h.placement.is_some()).count();
    let mut by_sig: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_cell: BTreeMap<(String, String), u64> = BTreeMap::new();
    for h in &hits {
        if let Some(loc) = lo.record(h.plugin, h.record) {
            let n = by_sig.entry(sig_text(loc.signature)).or_default();
            *n = n.saturating_add(1);
        }
        if let Some(p) = &h.placement {
            let label = |raw: u32| {
                lo.resolve(h.plugin, raw)
                    .map_or_else(|_| format!("{raw:08X}?"), |g| lo.form_label(g))
            };
            let key = (
                p.world_raw.map_or_else(|| "(interior)".to_string(), label),
                label(p.cell_raw),
            );
            let n = by_cell.entry(key).or_default();
            *n = n.saturating_add(1);
        }
    }
    let mut cells: Vec<((String, String), u64)> = by_cell.into_iter().collect();
    cells.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let cells_json: Vec<Value> = cells
        .iter()
        .take(50)
        .map(|((world, cell), n)| json!({"world": world, "cell": cell, "count": n}))
        .collect();

    let mut reader = RecordReader::new();
    let mut notes = Notes::default();
    let mut evidence = Vec::new();
    let mut lines = Vec::new();
    let mut edges = Vec::new();
    let page: Vec<&InHit> = hits.iter().skip(head.start).take(head.limit).collect();
    let end = head.start.saturating_add(page.len());
    let next_cursor = (end < total).then(|| encode_cursor(head.fp, end));
    for h in &page {
        let Some(loc) = lo.record(h.plugin, h.record).copied() else {
            continue;
        };
        let file = lo
            .plugin(h.plugin)
            .map_or_else(String::new, |p| p.name.clone());
        let g = lo.resolve(h.plugin, loc.form_id).ok();
        let winner = g
            .and_then(|g| lo.forms.winner(g))
            .is_some_and(|w| w.plugin == h.plugin && w.record == h.record);
        let version = Version {
            key: g.map_or(0, GlobalForm::key),
            plugin: h.plugin,
            record: h.record,
            basis: kitkraft_doc::Basis::Direct,
        };
        let (from_sig, from_edid) = (sig_text(loc.signature), lo.edid(h.plugin, &loc));
        let placed = h
            .placement
            .as_ref()
            .map(|p| placement_json(lo, h.plugin, p));
        let base = json!({
            "depth": h.depth,
            "group": if h.placement.is_some() { "placed" } else { "referrer" },
            "from": g.map(|g| lo.form_label(g)),
            "from_raw": format!("{:08X}", loc.form_id),
            "from_signature": from_sig,
            "from_editor_id": from_edid,
            "plugin": file,
            "winner": winner,
            "to": lo.form_label(h.target),
            "placed": placed,
        });
        let where_text = h.placement.as_ref().map_or_else(String::new, |p| {
            let named = |raw: u32| match lo.resolve(h.plugin, raw) {
                Ok(g) => match identity(lo, g).1 {
                    Some(e) => format!("{} {e}", lo.form_label(g)),
                    None => lo.form_label(g),
                },
                Err(_) => format!("{raw:08X} (unresolvable)"),
            };
            let cell = named(p.cell_raw);
            let world = p
                .world_raw
                .map_or_else(String::new, |w| format!(", world {}", named(w)));
            format!(
                " in cell {cell}{world}{}",
                if p.group_type == kitkraft_esm::cells::group_type::CELL_PERSISTENT {
                    " (persistent)"
                } else {
                    ""
                }
            )
        });
        let from_text = format!(
            "{} {} {}",
            g.map_or_else(
                || format!("{}:{:08X}", file, loc.form_id),
                |g| lo.form_label(g)
            ),
            from_sig,
            from_edid.unwrap_or("")
        );
        match version_links(lo, &mut reader, schema, version) {
            Ok((rec, links)) => {
                notes.compressed |= rec.compressed;
                let label = rec.label();
                let naming: Vec<&Link> = links
                    .links
                    .iter()
                    .filter(|l| l.status.names_form() && l.form == Some(h.target))
                    .collect();
                if naming.is_empty() {
                    notes.unreadable.push(format!(
                        "{label} in {file}: the reverse index lists it, but the schema walk places no member naming {} (a value the generated decoder reads where the schema walk does not)",
                        lo.form_label(h.target)
                    ));
                    edges.push(base.clone());
                }
                for l in naming {
                    notes.add_link(l);
                    evidence.push(link_evidence(&file, &label, rec.offset(), l));
                    let mut v = base.clone();
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("path".into(), json!(l.path));
                        obj.insert("raw".into(), json!(format!("{:08X}", l.raw)));
                        obj.insert("status".into(), json!(l.status.as_str()));
                        obj.insert("resolved_by".into(), json!(l.basis.map(|b| b.as_str())));
                        obj.insert("offset".into(), json!(l.file_offset));
                        obj.insert("body_offset".into(), json!(l.body_offset));
                    }
                    lines.push(format!(
                        "  {}{} via {}{where_text}",
                        if h.depth > 1 {
                            format!("[{}] ", h.depth)
                        } else {
                            String::new()
                        },
                        from_text.trim_end(),
                        l.path
                    ));
                    edges.push(v);
                }
            }
            Err(e) => {
                notes
                    .unreadable
                    .push(format!("{file} {:08X}: {e}", loc.form_id));
                edges.push(base);
            }
        }
    }
    let mut limitations = loaded.limitations.clone();
    limitations.push(
        "incoming links come from a reverse index of every plugin (the generated decoder's typed FormID values, cached beside the record index); records of types with no definition and subrecords kept raw are not searched (counts in `not_searched`)".to_string(),
    );
    let totals = index.totals();
    let mut no_def: Vec<(String, u64)> = totals
        .iter()
        .filter(|(_, s)| s.no_definition > 0)
        .map(|(sig, s)| (sig_text(*sig), s.no_definition))
        .collect();
    no_def.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let no_def_records: u64 = no_def.iter().map(|(_, n)| *n).sum();
    let raw_records: u64 = totals.values().map(|s| s.with_raw_subrecords).sum();
    if no_def_records > 0 {
        limitations.push(format!(
            "not searched: {no_def_records} record(s) of {} type(s) with no definition ({})",
            no_def.len(),
            no_def
                .iter()
                .take(15)
                .map(|(s, n)| format!("{s} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if raw_records > 0 {
        limitations.push(format!(
            "partly searched: {raw_records} record(s) have subrecords the decoder keeps raw (component blocks, unplaced or undecodable payloads)"
        ));
    }
    if placed_total > 0 {
        limitations.push("placement (cell, world, persistent) comes from the groups that hold the record (kitkraft_esm::cells: type 8 persistent, type 9 temporary; corpus facts)".to_string());
    }
    for s in &skipped {
        limitations.push(format!("not searched: {s}"));
    }
    for (p, e) in &index.errors {
        let name = lo.plugin(*p).map_or("?", |x| x.name.as_str());
        limitations.push(format!("{name}: reverse index failed: {e}"));
    }
    if truncated {
        limitations.push(format!(
            "the walk stopped at {MAX_NODES} forms or {MAX_EDGES} referrers: lower `depth`"
        ));
    }
    notes.limitations(&mut limitations);
    let obtained = |w: Obtained| index.obtained.iter().filter(|o| **o == Some(w)).count();
    let summary = format!(
        "{} {} {}: {} referrer(s) ({} placed), depth {}{}\n{}",
        lo.form_label(head.form),
        head.sig.as_deref().unwrap_or("?"),
        head.edid.as_deref().unwrap_or("(no EditorID)"),
        total,
        placed_total,
        head.depth,
        if next_cursor.is_some() {
            format!(", showing {}..{end}", head.start)
        } else {
            String::new()
        },
        lines.join("\n")
    );
    let data = json!({
        "form": head.form_arg,
        "matched_by": head.matched_by,
        "global": lo.form_label(head.form),
        "signature": head.sig,
        "editor_id": head.edid,
        "direction": "incoming",
        "depth": head.depth,
        "scope": if all_versions { "all" } else { "winner" },
        "total": total,
        "counts": {"referrers": total.saturating_sub(placed_total), "placed": placed_total, "by_signature": by_sig},
        "placed_by_cell": cells_json,
        "edges": edges,
        "next_cursor": next_cursor,
        "truncated": truncated,
        "index": {
            "plugins": index.plugins.iter().filter(|p| p.is_some()).count(),
            "cache_hits": obtained(Obtained::Hit),
            "built": obtained(Obtained::Built),
            "uncached": obtained(Obtained::Uncached),
            "elapsed_ms": index.elapsed_ms,
        },
        "not_searched": {
            "no_definition": no_def.iter().map(|(s, n)| json!({"type": s, "records": n})).collect::<Vec<_>>(),
            "records_with_raw_subrecords": raw_records,
        },
        "summary": summary,
    });
    Ok(Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: no_def
                .iter()
                .map(|(s, n)| format!("{s} ({n} records, no definition)"))
                .collect(),
        },
        limitations,
        evidence,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]
mod tests {
    use crate::handlers::load::tests::plugin;
    use crate::handlers::testdata::sub;
    use crate::Engine;
    use serde_json::{json, Value};
    use std::path::PathBuf;

    fn kwda(edid: &str, ids: &[u32]) -> Vec<u8> {
        let kw: Vec<u8> = ids.iter().flat_map(|i| i.to_le_bytes()).collect();
        [
            sub(b"EDID", &[edid.as_bytes(), &[0]].concat()),
            sub(b"KSIZ", &(ids.len() as u32).to_le_bytes()),
            sub(b"KWDA", &kw),
        ]
        .concat()
    }

    /// Base.esm: keyword 800, rifle 801 (keyword 800 twice, a missing 999, null, an index
    /// past the masters); Patch.esm (master Base.esm): a pistol naming the keyword and the
    /// rifle's keyword list overridden.
    fn fixture(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kitkraft-engine-refs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Base.esm"),
            plugin(
                0x1,
                &[],
                &[
                    (b"KYWD", 0x800, sub(b"EDID", b"Kw\0")),
                    (
                        b"WEAP",
                        0x801,
                        kwda("Rifle", &[0x800, 0x800, 0x999, 0, 0x0300_0001]),
                    ),
                    (b"KYWD", 0x803, sub(b"EDID", b"Other\0")),
                ],
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("Patch.esm"),
            plugin(
                0x1,
                &["Base.esm"],
                &[
                    (b"WEAP", 0x0100_0802, kwda("Pistol", &[0x800])),
                    (b"WEAP", 0x801, kwda("Rifle", &[0x803])),
                ],
            ),
        )
        .unwrap();
        dir
    }

    fn args(dir: &std::path::Path, extra: Value) -> Value {
        let mut a = json!({
            "plugins": ["Base.esm", "Patch.esm"],
            "data_dir": dir.display().to_string(),
            "no_cache": true,
        });
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        a
    }

    #[test]
    fn outgoing_links_mark_every_status() {
        let dir = fixture("out");
        let engine = Engine::with_default_handlers();
        // The winning rifle (Patch.esm) names Other.
        let r = engine
            .execute("refs", args(&dir, json!({"form": "Base.esm:000801"})))
            .unwrap();
        let edges = r["data"]["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 1, "{r}");
        assert_eq!(edges[0]["to"], "Base.esm:000803");
        assert_eq!(edges[0]["path"], "KWDA.Forms[0]");
        assert_eq!(edges[0]["status"], "resolved");
        assert_eq!(edges[0]["to_signature"], "KYWD");
        assert_eq!(edges[0]["to_editor_id"], "Other");
        assert_eq!(edges[0]["targets"], json!(["KYWD"]));
        assert_eq!(edges[0]["target_allowed"], true);
        assert_eq!(edges[0]["plugin"], "Patch.esm");
        // Evidence points at the four stored bytes.
        let ev = &r["evidence"][0];
        let bytes = std::fs::read(dir.join("Patch.esm")).unwrap();
        let at = ev["offset"].as_u64().unwrap() as usize;
        assert_eq!(&bytes[at..at + 4], &0x803u32.to_le_bytes());
        assert_eq!(ev["length"], 4);
        // Only Base.esm loaded: the rifle's own list, every status distinct, raw kept.
        let r = engine
            .execute(
                "refs",
                json!({"form": "Rifle", "plugins": ["Base.esm"], "data_dir": dir.display().to_string(), "no_cache": true}),
            )
            .unwrap();
        let statuses: Vec<(&str, &str)> = r["data"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["raw"].as_str().unwrap(), e["status"].as_str().unwrap()))
            .collect();
        assert_eq!(
            statuses,
            [
                ("00000800", "resolved"),
                ("00000800", "resolved"),
                ("00000999", "missing"),
                ("00000000", "null"),
                ("03000001", "unresolvable"),
            ]
        );
        assert!(r["data"]["edges"][4]["error"]
            .as_str()
            .unwrap()
            .contains("master"));
        assert!(r["data"]["edges"][4]["to"].is_null());
        let lim = r["limitations"].to_string();
        assert!(
            lim.contains("unresolvable:") && lim.contains("missing:"),
            "{lim}"
        );
        // Paging: two at a time, the cursor carries on, a foreign cursor is refused.
        let a = json!({"form": "Rifle", "plugins": ["Base.esm"], "data_dir": dir.display().to_string(), "no_cache": true, "limit": 2});
        let p1 = engine.execute("refs", a.clone()).unwrap();
        assert_eq!(p1["data"]["edges"].as_array().unwrap().len(), 2);
        let mut a2 = a.clone();
        a2["cursor"] = p1["data"]["next_cursor"].clone();
        let p2 = engine.execute("refs", a2.clone()).unwrap();
        assert_eq!(p2["data"]["edges"][0]["raw"], "00000999");
        a2["incoming"] = json!(true);
        assert!(engine.execute("refs", a2).is_err());
    }

    #[test]
    fn incoming_links_and_depth() {
        let dir = fixture("in");
        let engine = Engine::with_default_handlers();
        let r = engine
            .execute(
                "refs",
                args(&dir, json!({"form": "Base.esm:000800", "incoming": true})),
            )
            .unwrap();
        // The winning rifle no longer names Kw; the pistol does.
        let edges = r["data"]["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 1, "{r}");
        assert_eq!(edges[0]["from"], "Patch.esm:000802");
        assert_eq!(edges[0]["path"], "KWDA.Forms[0]");
        assert_eq!(edges[0]["winner"], true);
        assert_eq!(edges[0]["group"], "referrer");
        // Every version: Base.esm's rifle too, twice (two members).
        let r = engine
            .execute(
                "refs",
                args(
                    &dir,
                    json!({"form": "Base.esm:000800", "incoming": true, "scope": "all"}),
                ),
            )
            .unwrap();
        let froms: Vec<(&str, &str, bool)> = r["data"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e["from"].as_str().unwrap(),
                    e["path"].as_str().unwrap(),
                    e["winner"].as_bool().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            froms,
            [
                ("Base.esm:000801", "KWDA.Forms[0]", false),
                ("Base.esm:000801", "KWDA.Forms[1]", false),
                ("Patch.esm:000802", "KWDA.Forms[0]", true),
            ]
        );
        // Outgoing depth 2 from the pistol reaches nothing further (a keyword has no links).
        let r = engine
            .execute("refs", args(&dir, json!({"form": "Pistol", "depth": 2})))
            .unwrap();
        assert_eq!(r["data"]["total"], 1);
        assert_eq!(r["data"]["forms_read"], 2);
        // Through the command line.
        let line = format!(
            "refs Base.esm:000800 --incoming --plugins Base.esm --plugins Patch.esm --data-dir \"{}\" --no-cache",
            dir.display()
        );
        let words = crate::cmdline::split_words(&line).unwrap();
        match crate::cmdline::parse_words(&words).unwrap() {
            crate::Invocation::Run { command, args } => {
                assert_eq!(command.id, "refs");
                let r = engine.execute("refs", args).unwrap();
                assert_eq!(r["data"]["total"], 1);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn engine_reserved_rule_matches_validate() {
        assert_eq!(
            kitkraft_doc::links::RESERVED_OBJECT_IDS,
            crate::validate::RESERVED_OBJECT_IDS
        );
        assert_eq!(
            kitkraft_doc::links::RESERVED_MASTER,
            crate::validate::RESERVED_MASTER
        );
        assert_eq!(
            kitkraft_doc::links::NONE_FORM_ID,
            crate::validate::NONE_FORM_ID
        );
    }

    #[test]
    fn placed_references_are_grouped_with_their_cell() {
        use crate::handlers::testdata::{group, record};
        let dir = std::env::temp_dir().join(format!(
            "kitkraft-engine-refs-placed-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut bytes = plugin(
            0x1,
            &[],
            &[
                (b"STAT", 0x800, sub(b"EDID", b"Rock\0")),
                (b"WEAP", 0x801, kwda("Odd", &[])),
            ],
        );
        // A world 0x810 with its persistent cell 0x811 holding a persistent REFR of the
        // rock, and an interior cell 0x812 with a temporary REFR of it.
        let refr = |id: u32, flags: u32| {
            record(b"REFR", flags, id, &sub(b"NAME", &0x800u32.to_le_bytes()))
        };
        let grup = |label: u32, t: i32, body: &[u8]| {
            let mut out = b"GRUP".to_vec();
            out.extend_from_slice(&u32::try_from(body.len() + 24).unwrap().to_le_bytes());
            out.extend_from_slice(&label.to_le_bytes());
            out.extend_from_slice(&t.to_le_bytes());
            out.extend_from_slice(&[0u8; 8]);
            out.extend_from_slice(body);
            out
        };
        let world_children = grup(
            0x810,
            1,
            &[
                record(b"CELL", 0x400, 0x811, &sub(b"EDID", b"WorldCell\0")),
                grup(0x811, 6, &grup(0x811, 8, &refr(0x820, 0x400))),
            ]
            .concat(),
        );
        bytes.extend(group(
            b"WRLD",
            0,
            &[
                record(b"WRLD", 0, 0x810, &sub(b"EDID", b"World\0")),
                world_children,
            ]
            .concat(),
        ));
        let interior = grup(
            0,
            2,
            &grup(
                0,
                3,
                &[
                    record(b"CELL", 0, 0x812, &sub(b"EDID", b"Room\0")),
                    grup(0x812, 6, &grup(0x812, 9, &refr(0x821, 0))),
                ]
                .concat(),
            ),
        );
        bytes.extend(group(b"CELL", 0, &interior));
        std::fs::write(dir.join("Base.esm"), bytes).unwrap();
        let engine = Engine::with_default_handlers();
        let r = engine
            .execute(
                "refs",
                json!({"form": "Rock", "incoming": true, "plugins": ["Base.esm"], "data_dir": dir.display().to_string(), "no_cache": true}),
            )
            .unwrap();
        assert_eq!(r["data"]["counts"]["placed"], 2, "{r}");
        let edges = r["data"]["edges"].as_array().unwrap();
        assert_eq!(edges[0]["path"], "NAME.Base");
        assert_eq!(edges[0]["placed"]["cell"]["editor_id"], "WorldCell");
        assert_eq!(edges[0]["placed"]["world"]["editor_id"], "World");
        assert_eq!(edges[0]["placed"]["persistent"], true);
        assert_eq!(edges[1]["placed"]["cell"]["editor_id"], "Room");
        assert!(edges[1]["placed"]["world"].is_null());
        assert_eq!(edges[1]["placed"]["persistent"], false);
        assert_eq!(r["data"]["placed_by_cell"].as_array().unwrap().len(), 2);
        assert!(r["data"]["summary"]
            .as_str()
            .unwrap()
            .contains("in cell Base.esm:000811"));
    }
}
