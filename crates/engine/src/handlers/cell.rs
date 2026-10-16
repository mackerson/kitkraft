//! `cell` (M7.S3): the read-only data behind a Cell View, a page at a time. Four modes:
//!
//! - no `world`, `cell` or `interior`: the worldspaces (`WRLD`) of the load order, each with
//!   its EditorID and the number of its cells that hold references;
//! - `interior: true`: the interior cells (cells whose children group sits outside every
//!   world), each with its EditorID and reference count;
//! - `world`: that world's exterior cells, each with its grid (`XCLC` X and Y); with `x`
//!   and `y` as well, the references of the cell at that grid;
//! - `cell`: the references of one cell: placed record type, base form (with its type and
//!   EditorID), the reference's EditorID, position, rotation, scale, persistence and flags;
//!   with `with_bounds: true` (M7.R3), also the base form's object bounds (`OBND`, two
//!   corners in the base's own space, as stored), read once per distinct base.
//!
//! Cells and their references come from the placement ranges of the reverse link index
//! (`kitkraft_doc::backrefs`, built once per load order and cached on disk beside the
//! record index): every record inside a cell's persistent or temporary children group, in
//! every plugin, is a candidate, resolved to its form; the winning version is the one
//! shown, and a reference whose winning version sits in another cell (moved by an
//! override) is counted, not listed. Values are read through the schema (`REFR`/`ACHR`
//! `NAME`, `DATA` as `PositionRotation`, `XSCL`, `CELL` `XCLC`: their definitions cite
//! `wbDefinitionsSF1.pas` and the world-family corpus check), never from fixed offsets.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use kitkraft_doc::backrefs::Backrefs;
use kitkraft_doc::links::decode_record;
use kitkraft_doc::query::{decode_cursor, encode_cursor};
use kitkraft_doc::typed::Decoded;
use kitkraft_doc::{GlobalForm, LoadOrder, RecordReader, Version};
use kitkraft_esm::cells::group_type;
use kitkraft_esm::HEADER_LEN;
use kitkraft_schema::Schema;
use serde_json::{json, Value};

use super::failed;
use super::inspect::find_form;
use super::load::{acquire, schema_of, Loaded};
use super::refs::{backrefs, identity};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;
use crate::shapes;

const ID: &str = "cell";

/// Default page size.
const DEFAULT_LIMIT: usize = 100;

/// Largest page.
const MAX_LIMIT: usize = 1000;

/// Record flag "persistent" (UESP, Skyrim_Mod:Mod_File_Format, "Records"; 0x400).
const PERSISTENT: u32 = 0x0000_0400;

/// Record flag "deleted".
const DELETED: u32 = 0x0000_0020;

/// The records of one cell's children groups, per plugin.
#[derive(Default)]
struct CellRuns {
    world: Option<GlobalForm>,
    runs: Vec<(u16, Range<u32>, i32)>,
}

impl CellRuns {
    fn records(&self) -> u64 {
        self.runs
            .iter()
            .map(|(_, r, _)| u64::from(r.end.saturating_sub(r.start)))
            .sum()
    }
}

/// Every cell with children, keyed by its form, from the placement ranges of every plugin.
fn cells(lo: &LoadOrder, br: &Backrefs) -> (BTreeMap<u64, CellRuns>, u64) {
    let mut out: BTreeMap<u64, CellRuns> = BTreeMap::new();
    let mut unresolved = 0u64;
    for (i, pb) in br.plugins.iter().enumerate() {
        let (Some(pb), Ok(p)) = (pb, u16::try_from(i)) else {
            continue;
        };
        for (range, place) in pb.placements() {
            let Ok(cell) = lo.resolve(p, place.cell_raw) else {
                unresolved = unresolved.saturating_add(1);
                continue;
            };
            let world = place.world_raw.and_then(|w| lo.resolve(p, w).ok());
            let entry = out.entry(cell.key()).or_default();
            if entry.world.is_none() {
                entry.world = world;
            }
            entry.runs.push((p, range, place.group_type));
        }
    }
    (out, unresolved)
}

/// A member of a decoded struct.
fn member<'d>(d: &'d Decoded, name: &str) -> Option<&'d Decoded> {
    match d {
        Decoded::Struct(fields) => fields.iter().find(|(k, _)| k == name).map(|(_, v)| v),
        _ => None,
    }
}

fn number(d: Option<&Decoded>) -> Value {
    match d {
        Some(Decoded::Float(f)) => {
            serde_json::Number::from_f64(*f).map_or_else(|| json!(f.to_string()), Value::Number)
        }
        Some(Decoded::Int(i)) => i64::try_from(*i).map_or(Value::Null, |x| json!(x)),
        _ => Value::Null,
    }
}

fn vec3(d: Option<&Decoded>) -> Value {
    d.map_or(Value::Null, |v| {
        json!([
            number(member(v, "X")),
            number(member(v, "Y")),
            number(member(v, "Z"))
        ])
    })
}

/// The winning version of `form`, read and decoded through the schema: `(version, the
/// decoded subrecords by signature, flags, header offset, file name)`.
struct Read {
    version: Version,
    subs: Vec<([u8; 4], Decoded)>,
    /// Each decoded subrecord's data offset in the file (`None` inside a compressed
    /// record) and its size, in the order of `subs`.
    spans: Vec<([u8; 4], Option<u64>, u64)>,
    flags: u32,
    offset: u64,
    file: String,
    label: String,
}

fn read_winner(
    lo: &LoadOrder,
    schema: &Schema,
    reader: &mut RecordReader,
    form: GlobalForm,
) -> Result<Read, String> {
    let v = lo
        .forms
        .winner(form)
        .copied()
        .ok_or_else(|| format!("{} has no version", lo.form_label(form)))?;
    let loc = lo
        .record(v.plugin, v.record)
        .copied()
        .ok_or_else(|| "version missing from the index".to_string())?;
    let rec = reader.read(lo, v.plugin, &loc).map_err(|e| e.to_string())?;
    let edid_first = lo.edid(v.plugin, &loc).and_then(|e| e.bytes().next());
    let mut subs = Vec::new();
    let mut spans = Vec::new();
    for d in decode_record(lo, schema, v.plugin, &rec, edid_first) {
        let Ok(value) = d.value else { continue };
        let raw = rec.subs.get(d.index);
        spans.push((
            d.signature,
            raw.and_then(|s| s.data_offset()),
            raw.map_or(0, |s| s.data().len() as u64),
        ));
        subs.push((d.signature, value));
    }
    Ok(Read {
        version: v,
        subs,
        spans,
        flags: rec.record.header.flags,
        offset: rec.offset(),
        file: lo
            .plugin(v.plugin)
            .map(|p| p.name.clone())
            .unwrap_or_default(),
        label: rec.label(),
    })
}

impl Read {
    fn sub(&self, sig: &[u8; 4]) -> Option<&Decoded> {
        self.subs.iter().find(|(s, _)| s == sig).map(|(_, d)| d)
    }

    fn evidence(&self) -> Evidence {
        Evidence {
            file: self.file.clone(),
            offset: self.offset,
            length: HEADER_LEN as u64,
            record: Some(self.label.clone()),
            subrecord: None,
        }
    }

    /// Evidence for the subrecord `sig`: its data bytes, or the record header when the
    /// record is compressed (no file offsets inside it).
    fn sub_evidence(&self, sig: &[u8; 4]) -> Evidence {
        match self.spans.iter().find(|(s, _, _)| s == sig) {
            Some((_, Some(offset), len)) => Evidence {
                file: self.file.clone(),
                offset: *offset,
                length: *len,
                record: Some(self.label.clone()),
                subrecord: Some(String::from_utf8_lossy(sig).into_owned()),
            },
            _ => self.evidence(),
        }
    }
}

/// A corner of `OBND` (`Min` or `Max`, each a `Vec3`) as three finite numbers.
fn corner(obnd: &Decoded, name: &str) -> Option<[f64; 3]> {
    let v = member(obnd, name)?;
    let axis = |a: &str| match member(v, a) {
        Some(Decoded::Float(f)) if f.is_finite() => Some(*f),
        _ => None,
    };
    Some([axis("X")?, axis("Y")?, axis("Z")?])
}

/// What `with_bounds` found for one base form.
#[derive(Clone)]
enum BaseBounds {
    /// `{"min": [x, y, z], "max": [x, y, z]}` as stored.
    Found(Value),
    /// The base has no `OBND` (none stored, or a type the schema does not define).
    Missing,
    /// An `OBND` is stored, but the base type's definition keeps it opaque.
    Opaque,
    /// Stored, but a corner is not three finite numbers.
    NotFinite,
    /// The base's winning version could not be read.
    Unreadable,
}

/// `with_bounds` (M7.R3): the `OBND` of each distinct base, read once through the schema
/// (`ObjectBounds`: `Min` and `Max`, each a `Vec3` of floats; its definition cites
/// `wbDefinitionsCommon.pas` and the corpus check), with evidence at the subrecord.
#[derive(Default)]
struct BoundsReader {
    cache: BTreeMap<u64, BaseBounds>,
    evidence: Vec<Evidence>,
    /// References without bounds, by their base's signature.
    without: BTreeMap<String, u64>,
    /// Of those, the references whose base stores an `OBND` its definition keeps opaque.
    opaque: BTreeMap<String, u64>,
    not_finite: u64,
    unreadable: u64,
}

impl BoundsReader {
    fn of(
        &mut self,
        lo: &LoadOrder,
        schema: &Schema,
        reader: &mut RecordReader,
        base: GlobalForm,
    ) -> BaseBounds {
        if let Some(b) = self.cache.get(&base.key()) {
            return b.clone();
        }
        let b = match read_winner(lo, schema, reader, base) {
            Ok(r) => match r.sub(b"OBND") {
                Some(obnd @ Decoded::Struct(_)) => {
                    match (corner(obnd, "Min"), corner(obnd, "Max")) {
                        (Some(min), Some(max)) => {
                            self.evidence.push(r.sub_evidence(b"OBND"));
                            BaseBounds::Found(json!({"min": min, "max": max}))
                        }
                        _ => BaseBounds::NotFinite,
                    }
                }
                // Stored, but the type's definition keeps it opaque (no typed layout).
                Some(_) => BaseBounds::Opaque,
                None => BaseBounds::Missing,
            },
            Err(_) => BaseBounds::Unreadable,
        };
        self.cache.insert(base.key(), b.clone());
        b
    }

    /// The item's `bounds` value, counting the references left without one.
    fn value(&mut self, found: Option<BaseBounds>, base_sig: Option<&str>) -> Value {
        match found {
            Some(BaseBounds::Found(v)) => return v,
            Some(BaseBounds::NotFinite) => self.not_finite = self.not_finite.saturating_add(1),
            Some(BaseBounds::Unreadable) => self.unreadable = self.unreadable.saturating_add(1),
            Some(BaseBounds::Opaque) => {
                let e = self
                    .opaque
                    .entry(base_sig.unwrap_or("?").to_string())
                    .or_default();
                *e = e.saturating_add(1);
            }
            Some(BaseBounds::Missing) | None => {}
        }
        let e = self
            .without
            .entry(base_sig.unwrap_or("(no base)").to_string())
            .or_default();
        *e = e.saturating_add(1);
        Value::Null
    }

    fn data(&self) -> Value {
        let found = self
            .cache
            .values()
            .filter(|b| matches!(b, BaseBounds::Found(_)))
            .count();
        json!({
            "bases": self.cache.len(),
            "bases_with_bounds": found,
            "references_without": self.without.values().fold(0u64, |a, n| a.saturating_add(*n)),
            "without_by_base_signature": self.without,
            "opaque_by_base_signature": self.opaque,
        })
    }

    fn limitations(&self, out: &mut Vec<String>) {
        out.push("bounds are the base form's OBND (two corners in the base's own space, as stored, before the reference's rotation and scale); they come from the plugin, not from the model, which may differ (a bound read from the NIF is M7.R3's variant blocked on M7.O5)".to_string());
        if !self.without.is_empty() {
            let list: Vec<String> = self
                .without
                .iter()
                .map(|(s, n)| format!("{s} {n}"))
                .collect();
            out.push(format!(
                "references whose base has no OBND the schema decodes have bounds null; by base type: {}",
                list.join(", ")
            ));
        }
        if !self.opaque.is_empty() {
            let list: Vec<String> = self
                .opaque
                .iter()
                .map(|(s, n)| format!("{s} {n}"))
                .collect();
            out.push(format!(
                "of those, the base stores an OBND that its type's definition keeps opaque (not yet typed in crates/schema), so it is unknown, not absent: {}",
                list.join(", ")
            ));
        }
        if self.not_finite > 0 {
            out.push(format!(
                "{} reference(s): the base's OBND holds a value that is not a finite number; bounds null",
                self.not_finite
            ));
        }
        if self.unreadable > 0 {
            out.push(format!(
                "{} reference(s): the base's winning version could not be read; bounds null",
                self.unreadable
            ));
        }
    }
}

/// The grid of a cell (`XCLC` X and Y), when it has one.
fn grid(read: &Read) -> Option<(i64, i64)> {
    let g = read.sub(b"XCLC")?;
    let int = |d: Option<&Decoded>| match d {
        Some(Decoded::Int(i)) => i64::try_from(*i).ok(),
        _ => None,
    };
    Some((int(member(g, "X"))?, int(member(g, "Y"))?))
}

fn limit_arg(args: &Value) -> usize {
    args.get("limit")
        .and_then(Value::as_u64)
        .and_then(|l| usize::try_from(l).ok())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT)
}

/// The page `[start, end)` of `total` items, and the cursor after it.
fn page(
    args: &Value,
    fp: u64,
    total: usize,
) -> Result<(usize, usize, Option<String>), EngineError> {
    let start = match args.get("cursor").and_then(Value::as_str) {
        Some(c) => decode_cursor(c, fp).map_err(|e| failed(ID, e))?,
        None => 0,
    };
    let end = start.saturating_add(limit_arg(args)).min(total);
    let next = (end < total).then(|| encode_cursor(fp, end));
    Ok((start.min(end), end, next))
}

fn fingerprint(lo: &LoadOrder, mode: &str, key: u64) -> u64 {
    let mut h = kitkraft_doc::hash::Fnv::new();
    h.write_u64(lo.fingerprint());
    h.write(mode.as_bytes());
    h.write_u64(key);
    h.finish()
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let schema =
        schema_of(session).ok_or_else(|| failed(ID, "the builtin schema does not load"))?;
    let loaded = acquire(session, args, ID)?;
    let started = std::time::Instant::now();
    let plugin = args.get("plugin").and_then(Value::as_str);
    let br = backrefs(loaded, schema);
    let (cells, unresolved) = cells(&loaded.lo, br);
    let mut limitations = vec![
        "cells come from the placement ranges of the reverse link index (records inside a cell's persistent or temporary children groups, in every plugin): a cell with no child record in any plugin is not listed".to_string(),
    ];
    limitations.extend(loaded.limitations.iter().cloned());
    for (p, e) in br.errors.iter().take(10) {
        let name = loaded.lo.plugin(*p).map_or("?", |x| x.name.as_str());
        limitations.push(format!("{name}: its placement ranges could not be read ({e}); its cells and references are missing"));
    }
    if unresolved > 0 {
        limitations.push(format!(
            "{unresolved} cell children group(s) name a cell whose FormID does not resolve in this load order; not listed"
        ));
    }
    let mut out = if let Some(c) = args.get("cell").and_then(Value::as_str) {
        let (cell, _) = find_form(ID, loaded, c, plugin)?;
        references(loaded, schema, &cells, cell, args, &mut limitations)?
    } else if let Some(w) = args.get("world").and_then(Value::as_str) {
        let (world, _) = find_form(ID, loaded, w, plugin)?;
        let at = match (
            args.get("x").and_then(Value::as_i64),
            args.get("y").and_then(Value::as_i64),
        ) {
            (Some(x), Some(y)) => Some((x, y)),
            (None, None) => None,
            _ => return Err(failed(ID, "give both `x` and `y`, or neither")),
        };
        world_cells(loaded, schema, &cells, world, at, args, &mut limitations)?
    } else if args.get("interior").and_then(Value::as_bool) == Some(true) {
        interiors(loaded, &cells, args, &limitations)?
    } else {
        worlds(loaded, &cells, args, &limitations)?
    };
    if let Some(obj) = out.data.as_object_mut() {
        obj.insert(
            "elapsed_ms".into(),
            json!(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
        );
        obj.insert("index_ms".into(), json!(br.elapsed_ms));
    }
    Ok(out)
}

fn coverage() -> Coverage {
    Coverage {
        status: CoverageStatus::Partial,
        decoded_subrecords: None,
        opaque_subrecords: None,
        not_examined: Vec::new(),
    }
}

/// `WRLD` forms of the load order (each once, in form order).
fn worlds(
    loaded: &Loaded,
    cells: &BTreeMap<u64, CellRuns>,
    args: &Value,
    limitations: &[String],
) -> Result<Outcome, EngineError> {
    let lo = &loaded.lo;
    let mut found: BTreeSet<u64> = BTreeSet::new();
    for (i, p) in lo.plugins.iter().enumerate() {
        let Ok(pi) = u16::try_from(i) else { continue };
        for loc in p.records().iter().filter(|l| &l.signature == b"WRLD") {
            if let Ok(g) = lo.resolve(pi, loc.form_id) {
                found.insert(g.key());
            }
        }
    }
    let mut per_world: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
    for c in cells.values() {
        if let Some(w) = c.world {
            let e = per_world.entry(w.key()).or_default();
            e.0 = e.0.saturating_add(1);
            e.1 = e.1.saturating_add(c.records());
        }
    }
    let list: Vec<u64> = found.into_iter().collect();
    let (start, end, next) = page(args, fingerprint(lo, "worlds", 0), list.len())?;
    let mut evidence = Vec::new();
    let mut items = Vec::new();
    for key in list.get(start..end).unwrap_or(&[]) {
        let g = GlobalForm::from_key(*key);
        let (_, edid) = identity(lo, g);
        if let Some(w) = lo.forms.winner(g) {
            if let (Some(p), Some(loc)) = (lo.plugin(w.plugin), lo.record(w.plugin, w.record)) {
                evidence.push(Evidence {
                    file: p.name.clone(),
                    offset: loc.offset,
                    length: HEADER_LEN as u64,
                    record: Some(format!("WRLD:{:08X}", loc.form_id)),
                    subrecord: None,
                });
            }
        }
        let (n, refs) = per_world.get(key).copied().unwrap_or_default();
        items.push(json!({
            "form": lo.form_label(g),
            "editor_id": edid,
            "cells": n,
            "child_records": refs,
            "versions": lo.forms.versions(g).len(),
        }));
    }
    let interior = cells.values().filter(|c| c.world.is_none()).count();
    let summary = format!(
        "{} worldspace(s), {} exterior and {interior} interior cell(s) with references; page {start}..{end}",
        list.len(),
        cells.len().saturating_sub(interior)
    );
    Ok(Outcome {
        data: json!({
            "mode": "worlds", "total": list.len(), "items": items,
            "interior_cells": interior, "next_cursor": next, "summary": summary,
        }),
        coverage: coverage(),
        limitations: limitations.to_vec(),
        evidence,
    })
}

/// Interior cells: cells whose children sit outside every world.
fn interiors(
    loaded: &Loaded,
    cells: &BTreeMap<u64, CellRuns>,
    args: &Value,
    limitations: &[String],
) -> Result<Outcome, EngineError> {
    let lo = &loaded.lo;
    let list: Vec<(&u64, &CellRuns)> = cells.iter().filter(|(_, c)| c.world.is_none()).collect();
    let (start, end, next) = page(args, fingerprint(lo, "interior", 0), list.len())?;
    let mut items = Vec::new();
    for (key, c) in list.get(start..end).unwrap_or(&[]) {
        let g = GlobalForm::from_key(**key);
        let (sig, edid) = identity(lo, g);
        items.push(json!({
            "form": lo.form_label(g), "signature": sig, "editor_id": edid,
            "child_records": c.records(), "versions": lo.forms.versions(g).len(),
        }));
    }
    let summary = format!(
        "{} interior cell(s) with references; page {start}..{end}",
        list.len()
    );
    Ok(Outcome {
        data: json!({
            "mode": "interior", "total": list.len(), "items": items,
            "next_cursor": next, "summary": summary,
        }),
        coverage: coverage(),
        limitations: limitations.to_vec(),
        evidence: Vec::new(),
    })
}

/// The exterior cells of `world`, with their grids; or, with `at`, the references of the
/// cell at that grid.
fn world_cells(
    loaded: &Loaded,
    schema: &Schema,
    cells: &BTreeMap<u64, CellRuns>,
    world: GlobalForm,
    at: Option<(i64, i64)>,
    args: &Value,
    limitations: &mut Vec<String>,
) -> Result<Outcome, EngineError> {
    let lo = &loaded.lo;
    let list: Vec<(&u64, &CellRuns)> = cells
        .iter()
        .filter(|(_, c)| c.world == Some(world))
        .collect();
    let mut reader = RecordReader::new();
    if let Some((x, y)) = at {
        let mut hit = None;
        for (key, _) in &list {
            let g = GlobalForm::from_key(**key);
            if let Ok(r) = read_winner(lo, schema, &mut reader, g) {
                if grid(&r) == Some((x, y)) {
                    hit = Some(g);
                    break;
                }
            }
        }
        let cell = hit.ok_or_else(|| {
            failed(
                ID,
                format!(
                    "{} has no cell with references at grid {x}, {y}",
                    lo.form_label(world)
                ),
            )
        })?;
        return references(loaded, schema, cells, cell, args, limitations);
    }
    let (start, end, next) = page(args, fingerprint(lo, "world", world.key()), list.len())?;
    let mut items = Vec::new();
    let mut evidence = Vec::new();
    for (key, c) in list.get(start..end).unwrap_or(&[]) {
        let g = GlobalForm::from_key(**key);
        let (sig, edid) = identity(lo, g);
        let (x, y) = match read_winner(lo, schema, &mut reader, g) {
            Ok(r) => {
                evidence.push(r.evidence());
                grid(&r).map_or((Value::Null, Value::Null), |(x, y)| (json!(x), json!(y)))
            }
            Err(e) => {
                limitations.push(format!("{}: {e}", lo.form_label(g)));
                (Value::Null, Value::Null)
            }
        };
        items.push(json!({
            "form": lo.form_label(g), "signature": sig, "editor_id": edid,
            "x": x, "y": y, "child_records": c.records(),
            "versions": lo.forms.versions(g).len(),
        }));
    }
    let (_, wedid) = identity(lo, world);
    let summary = format!(
        "{} {}: {} exterior cell(s) with references; page {start}..{end}",
        lo.form_label(world),
        wedid.as_deref().unwrap_or("(no EditorID)"),
        list.len()
    );
    Ok(Outcome {
        data: json!({
            "mode": "world", "world": lo.form_label(world), "world_editor_id": wedid,
            "total": list.len(), "items": items, "next_cursor": next, "summary": summary,
        }),
        coverage: coverage(),
        limitations: limitations.clone(),
        evidence,
    })
}

/// The references of `cell`.
fn references(
    loaded: &Loaded,
    schema: &Schema,
    cells: &BTreeMap<u64, CellRuns>,
    cell: GlobalForm,
    args: &Value,
    limitations: &mut Vec<String>,
) -> Result<Outcome, EngineError> {
    let lo = &loaded.lo;
    let br = loaded.backrefs.get();
    let runs = cells.get(&cell.key());
    // Every record of the cell's children, resolved to its form.
    let mut forms: BTreeMap<u64, i32> = BTreeMap::new();
    for (p, range, gt) in runs.map(|c| c.runs.as_slice()).unwrap_or(&[]) {
        for idx in range.clone() {
            let Some(loc) = lo.record(*p, idx) else {
                continue;
            };
            if let Ok(g) = lo.resolve(*p, loc.form_id) {
                forms.entry(g.key()).or_insert(*gt);
            }
        }
    }
    // Keep the forms whose winning version is still in this cell.
    let mut moved = 0u64;
    let mut list: Vec<(u64, i32)> = Vec::new();
    for (key, gt) in forms {
        let g = GlobalForm::from_key(key);
        let Some(w) = lo.forms.winner(g) else {
            continue;
        };
        let place = br
            .and_then(|b| b.plugins.get(usize::from(w.plugin)))
            .and_then(Option::as_ref)
            .and_then(|pb| pb.placement(w.record));
        match place {
            Some(pl) if lo.resolve(w.plugin, pl.cell_raw).ok() == Some(cell) => {
                list.push((key, pl.group_type));
            }
            Some(_) => moved = moved.saturating_add(1),
            None => list.push((key, gt)),
        }
    }
    let (start, end, next) = page(args, fingerprint(lo, "cell", cell.key()), list.len())?;
    let with_bounds = args.get("with_bounds").and_then(Value::as_bool) == Some(true);
    let mut bounds = BoundsReader::default();
    let mut reader = RecordReader::new();
    let mut evidence = Vec::new();
    let mut items = Vec::new();
    for (key, gt) in list.get(start..end).unwrap_or(&[]) {
        let g = GlobalForm::from_key(*key);
        let r = match read_winner(lo, schema, &mut reader, g) {
            Ok(r) => r,
            Err(e) => {
                limitations.push(format!("{}: {e}", lo.form_label(g)));
                continue;
            }
        };
        evidence.push(r.evidence());
        let (sig, edid) = identity(lo, g);
        let base_raw = match r.sub(b"NAME") {
            Some(Decoded::Form(raw)) => Some(*raw),
            _ => None,
        };
        let base = base_raw.and_then(|raw| lo.resolve(r.version.plugin, raw).ok());
        let (base_sig, base_edid) = base.map_or((None, None), |b| identity(lo, b));
        let placement = r.sub(b"DATA");
        let mut item = json!({
            "form": lo.form_label(g),
            "signature": sig,
            "editor_id": edid,
            "base": base_raw.map(|raw| shapes::formref(lo, r.version.plugin, raw)),
            "base_signature": base_sig,
            "base_editor_id": base_edid,
            "position": vec3(placement.and_then(|p| member(p, "Position"))),
            "rotation": vec3(placement.and_then(|p| member(p, "Rotation"))),
            "scale": number(r.sub(b"XSCL")),
            "persistent": *gt == group_type::CELL_PERSISTENT || r.flags & PERSISTENT != 0,
            "group": if *gt == group_type::CELL_PERSISTENT { "persistent" } else { "temporary" },
            "flags": format!("0x{:08X}", r.flags),
            "deleted": r.flags & DELETED != 0,
            "plugin": r.file,
            "versions": lo.forms.versions(g).len(),
        });
        if with_bounds {
            let found = base.map(|b| bounds.of(lo, schema, &mut reader, b));
            let v = bounds.value(found, base_sig.as_deref());
            if let Some(obj) = item.as_object_mut() {
                obj.insert("bounds".into(), v);
            }
        }
        items.push(item);
    }
    if with_bounds {
        bounds.limitations(limitations);
    }
    if moved > 0 {
        limitations.push(format!(
            "{moved} reference(s) placed in this cell by an earlier plugin sit in another cell in their winning version (moved by an override): not listed"
        ));
    }
    limitations.push(
        "position and rotation are as stored in the winning version (DATA, PositionRotation: rotation in radians); scale is null when the reference has no XSCL (the game uses 1)".to_string(),
    );
    let (csig, cedid) = identity(lo, cell);
    let world = runs.and_then(|c| c.world);
    let summary = format!(
        "{} {}: {} reference(s){}; page {start}..{end}",
        lo.form_label(cell),
        cedid.as_deref().unwrap_or("(no EditorID)"),
        list.len(),
        world.map_or_else(String::new, |w| format!(" in {}", lo.form_label(w)))
    );
    evidence.append(&mut bounds.evidence);
    let mut data = json!({
        "mode": "cell",
        "cell": lo.form_label(cell),
        "cell_signature": csig,
        "cell_editor_id": cedid,
        "world": world.map(|w| lo.form_label(w)),
        "total": list.len(),
        "moved_out": moved,
        "items": items,
        "next_cursor": next,
        "summary": summary,
    });
    if let (true, Some(obj)) = (with_bounds, data.as_object_mut()) {
        obj.insert("bounds".into(), bounds.data());
    }
    Ok(Outcome {
        data,
        coverage: coverage(),
        limitations: limitations.clone(),
        evidence,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]
mod tests {
    //! `with_bounds` (M7.R3) through `execute` on a synthetic master: an interior cell
    //! with two references to a static that has an `OBND` (read once), one to an NPC
    //! without one, and the envelope checked against its documented shape.

    use std::path::{Path, PathBuf};

    use kitkraft_esm::build::HeaderSpec;
    use kitkraft_esm::cells::{self, CellPlace, Parent, PlacementStyle};
    use kitkraft_esm::tes4::FLAG_MASTER;
    use kitkraft_esm::{Plugin, Record};
    use serde_json::{json, Value};

    use crate::edit::patch_tests::dir;
    use crate::handlers::place_tests::{rec, sub, zstr};
    use crate::shapes::check_envelope;
    use crate::Engine;

    fn floats(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    /// `Bounds.esm`: STAT `KC_Box` (800, OBND -10 -20 0 .. 10 20 40), NPC_ `KC_Npc` (801,
    /// no OBND), interior CELL `BoundsRoom` (900) with REFRs 901 and 902 (the box, the
    /// second turned and scaled) and 903 (the NPC).
    fn fixture(dir: &Path) -> PathBuf {
        let data = dir.join("Data");
        std::fs::create_dir_all(&data).unwrap();
        let mut spec = HeaderSpec::new(Vec::new());
        spec.flags = FLAG_MASTER;
        let mut plugin = Plugin::create(&spec);
        let style = PlacementStyle::OFFICIAL;
        let mut put = |record: Record<'static>, parent: Parent| {
            let path = cells::placement(&record.header, &parent, &style).unwrap();
            plugin.upsert_styled(&path, record, &style).unwrap();
        };
        put(
            rec(
                b"STAT",
                0,
                0x800,
                vec![
                    sub(b"EDID", &zstr("KC_Box")),
                    sub(b"OBND", &floats(&[-10.0, -20.0, 0.0, 10.0, 20.0, 40.0])),
                ],
            ),
            Parent::None,
        );
        put(
            rec(b"NPC_", 0, 0x801, vec![sub(b"EDID", &zstr("KC_Npc"))]),
            Parent::None,
        );
        put(
            rec(b"CELL", 0, 0x900, vec![sub(b"EDID", &zstr("BoundsRoom"))]),
            Parent::CellAt(CellPlace::INTERIOR),
        );
        let refs: [(u32, u32, [f32; 6], Option<f32>); 3] = [
            (0x901, 0x800, [1.0, 2.0, 3.0, 0.0, 0.0, 0.0], None),
            (0x902, 0x800, [100.0, 0.0, 0.0, 0.0, 0.0, 1.5], Some(2.0)),
            (0x903, 0x801, [0.0, 50.0, 0.0, 0.0, 0.0, 0.0], None),
        ];
        for (form, base, placed, scale) in refs {
            let mut subs = vec![
                sub(b"NAME", &base.to_le_bytes()),
                sub(b"DATA", &floats(&placed)),
            ];
            if let Some(s) = scale {
                subs.push(sub(b"XSCL", &s.to_le_bytes()));
            }
            put(
                rec(b"REFR", 0, form, subs),
                Parent::Cell {
                    place: CellPlace::INTERIOR,
                    cell: 0x900,
                },
            );
        }
        std::fs::write(data.join("Bounds.esm"), plugin.to_bytes().unwrap()).unwrap();
        data
    }

    fn items(env: &Value) -> Vec<Value> {
        env["data"]["items"].as_array().unwrap().clone()
    }

    #[test]
    fn with_bounds_adds_each_base_obnd_once_with_its_evidence() {
        let data = fixture(&dir("cell-bounds"));
        let engine = Engine::with_default_handlers();
        let lo = json!({"plugins": ["Bounds.esm"], "data_dir": data.display().to_string(), "no_cache": true});
        engine.execute("load", lo).unwrap();

        // Without the flag nothing changes: no `bounds` anywhere.
        let plain = engine
            .execute("cell", json!({"cell": "BoundsRoom"}))
            .unwrap();
        assert!(
            check_envelope(&plain).is_empty(),
            "{:?}",
            check_envelope(&plain)
        );
        assert!(plain["data"].get("bounds").is_none());
        assert!(items(&plain).iter().all(|i| i.get("bounds").is_none()));

        let env = engine
            .execute("cell", json!({"cell": "BoundsRoom", "with_bounds": true}))
            .unwrap();
        let problems = check_envelope(&env);
        assert!(problems.is_empty(), "{problems:?}");
        let list = items(&env);
        assert_eq!(list.len(), 3);
        let by_form = |f: &str| {
            list.iter()
                .find(|i| i["form"] == json!(format!("Bounds.esm:000{f}")))
                .unwrap()
                .clone()
        };
        let want = json!({"min": [-10.0, -20.0, 0.0], "max": [10.0, 20.0, 40.0]});
        assert_eq!(by_form("901")["bounds"], want);
        assert_eq!(by_form("902")["bounds"], want);
        assert_eq!(by_form("902")["scale"], json!(2.0));
        assert_eq!(by_form("902")["rotation"], json!([0.0, 0.0, 1.5]));
        assert_eq!(by_form("903")["bounds"], Value::Null);
        assert_eq!(
            env["data"]["bounds"],
            json!({
                "bases": 2, "bases_with_bounds": 1, "references_without": 1,
                "without_by_base_signature": {"NPC_": 1},
                "opaque_by_base_signature": {},
            })
        );
        // One evidence row per reference, then the base's OBND once, at its data bytes.
        let ev = env["evidence"].as_array().unwrap();
        assert_eq!(ev.len(), 4);
        let obnd = &ev[3];
        assert_eq!(obnd["subrecord"], json!("OBND"));
        assert_eq!(obnd["length"], json!(24));
        assert_eq!(obnd["record"], json!("STAT:00000800"));
        let bytes = std::fs::read(data.join("Bounds.esm")).unwrap();
        let at = usize::try_from(obnd["offset"].as_u64().unwrap()).unwrap();
        assert_eq!(bytes[at..at + 4], (-10.0f32).to_le_bytes());
        let lim = env["limitations"].as_array().unwrap();
        assert!(lim
            .iter()
            .any(|l| l.as_str().unwrap().contains("by base type: NPC_ 1")));
    }
}
