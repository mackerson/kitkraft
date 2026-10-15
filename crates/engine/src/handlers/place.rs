//! `place` (M4.E2): a new placed reference of a base form, at a position and rotation, in a
//! named cell or at a worldspace coordinate, written into a patch plugin.
//!
//! - The base is a formref, an EditorID or a hex FormID in `plugin`'s numbering. Its
//!   winning version decides the record: `ACHR` for an `NPC_`, `REFR` for anything else a
//!   reference can name. References, cells, worlds and navmeshes are refused as bases (no
//!   reference of the corpus names one: `kitkraft_esm::refs`), and so is `LVLN` until the
//!   corpus shows how a leveled actor is placed.
//! - The cell is `cell` (a `CELL` formref or EditorID), or `world` plus the position: the
//!   exterior cell of that world whose `XCLC` grid holds the position (lane A's grid rule,
//!   100 units a cell, `kitkraft_esm::cells`). A persistent reference in a world goes into
//!   the world's persistent cell, as every official file does. A new exterior cell is not
//!   created: a position with no cell is refused.
//! - The winning version of the cell (and of its world, for an exterior or persistent cell)
//!   is carried into the patch as an unchanged copy, so the new reference's children group
//!   has its parent (lane A's rule; `docs/corpus.md` "Cell and world groups"). Lstrings of a
//!   carried record from a localized plugin are written inline, as every patch does; the
//!   result says per carried record whether its bytes equal the winner's.
//! - The record is composed by `kitkraft_esm::refs::compose_reference` (members below
//!   `corpus` confidence refused unless named in `allow_reference`), gets a FormID from
//!   `kitkraft_esm::forms::NewForms` (full-scale targets only; the `HEDR` next object ID is
//!   advanced) and is placed by `kitkraft_esm::cells::placement`.
//! - An EditorID must be letters, digits and `_`, and must not exist in the load order or
//!   the target.
//!
//! As `set`: the write goes to a staged copy of `into`, `validate` runs on it, and `into`
//! is replaced only when the records written pass. Refusals are `failed` errors whose
//! message starts with a stable code, `place.<reason>:`; the argument-shaped ones
//! (`bad-vector`, `edid-invalid`, `cell-and-world`, `no-cell` when neither is given) are
//! `invalid_args` with a JSON pointer (`/position/1`, `/edid`) and the code as the rule,
//! as `set` reports a bad field. In a spec's `[[place]]` they stay `failed` (`build`
//! names the TOML key).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use kitkraft_doc::GlobalForm;
use kitkraft_esm::cells::{grid_label, group_type, Grid};
use kitkraft_esm::index::{index_file, read_record_at, IndexOptions};
use kitkraft_esm::{DecodeLimits, GroupHeader, Record, Signature};
use serde_json::{json, Value};

use super::load::{acquire, Loaded};
use super::set::{
    check, describe, extend_once, find_form, guard, install, outcome, safe_source, stage,
    validate_json, winning_copy, write_with, Origin,
};
use super::{failed, str_arg};
use crate::args::Violation;
use crate::edit::patch::{Copy, FormName, NewRef, PatchReport, RefPlace};
use crate::envelope::{Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;
use crate::validate::{self, Severity};

const ID: &str = "place";

/// Record types a placed reference never names (the corpus: `kitkraft_esm::refs`).
const NOT_PLACEABLE: [&[u8; 4]; 9] = [
    b"REFR", b"ACHR", b"PGRE", b"PHZD", b"CELL", b"WRLD", b"NAVM", b"LAND", b"TES4",
];

/// A refusal: `place.<code>: <message>`.
pub(crate) fn refuse(command: &str, code: &str, message: impl std::fmt::Display) -> EngineError {
    failed(command, format!("place.{code}: {message}"))
}

/// An argument-shaped refusal (`rule` is the stable `place.<code>`): for the `place` command
/// `invalid_args` at `/<field>` (`""`: the arguments object), as `set` reports a bad field;
/// for a spec entry (`build`) the `failed` `place.<code>: <at>: <message>` it always was.
fn refuse_arg(
    command: &str,
    at: &str,
    field: &str,
    rule: &'static str,
    message: String,
) -> EngineError {
    if command == ID {
        EngineError::InvalidArgs {
            command: ID.to_string(),
            violations: vec![Violation {
                pointer: if field.is_empty() {
                    String::new()
                } else {
                    format!("/{field}")
                },
                rule,
                message: format!("{rule}: {message}"),
            }],
        }
    } else {
        failed(command, format!("{rule}: {at}: {message}"))
    }
}

/// One placement, as `place` takes it or a spec's `[[place]]` entry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlaceRequest {
    /// `place` or `place[0]`.
    pub label: String,
    /// The base form.
    pub base: String,
    /// The plugin whose numbering hex FormIDs use.
    pub plugin: Option<String>,
    /// The cell.
    pub cell: Option<String>,
    /// The world (with the position deciding the cell).
    pub world: Option<String>,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub persistent: bool,
    pub initially_disabled: bool,
    pub edid: Option<String>,
    pub allow_reference: Vec<String>,
    /// A base written in the same call (a spec's `new:<name>`): its EditorID and type.
    pub base_local: Option<(String, [u8; 4])>,
}

impl PlaceRequest {
    /// A request with only a label (for EditorID checks).
    pub(crate) fn probe(label: &str) -> PlaceRequest {
        PlaceRequest {
            label: label.to_string(),
            base: String::new(),
            plugin: None,
            cell: None,
            world: None,
            position: [0.0; 3],
            rotation: [0.0; 3],
            persistent: false,
            initially_disabled: false,
            edid: None,
            allow_reference: Vec::new(),
            base_local: None,
        }
    }
}

/// A placement resolved against the load order, ready for the patch writer.
pub(crate) struct Planned {
    /// The carried cell (its world rides along as the copy's parent).
    pub carried: Copy,
    /// Where the carried cell came from.
    pub carried_origin: Origin,
    /// The new reference.
    pub new_ref: NewRef,
    /// `base`, `cell`, `world` for the report.
    pub info: Value,
    /// The base's and the cell's winning versions.
    pub evidence: Vec<Evidence>,
    /// Load-order problems that do not affect these forms.
    pub limitations: Vec<String>,
}

/// A 3-float argument (`position`, `rotation`); `None` when absent.
pub(crate) fn vec3(
    command: &str,
    value: Option<&Value>,
    what: &str,
) -> Result<Option<[f32; 3]>, EngineError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let bad = |field: String, message: String| {
        refuse_arg(command, what, &field, "place.bad-vector", message)
    };
    let items = value
        .as_array()
        .filter(|a| a.len() == 3)
        .ok_or_else(|| bad(what.to_string(), format!("{what} must be three numbers")))?;
    let mut out = [0f32; 3];
    for (i, (slot, item)) in out.iter_mut().zip(items).enumerate() {
        let v = item
            .as_f64()
            .ok_or_else(|| bad(format!("{what}/{i}"), format!("{what} must be numbers")))?;
        if !v.is_finite() || v.abs() > f64::from(f32::MAX) {
            return Err(bad(
                format!("{what}/{i}"),
                format!("{what} value {v} is not a finite 32-bit float"),
            ));
        }
        // In f32 range, checked just above.
        #[allow(clippy::cast_possible_truncation)]
        {
            *slot = v as f32;
        }
    }
    Ok(Some(out))
}

impl PlaceRequest {
    /// The request in `place`'s JSON arguments.
    pub(crate) fn from_args(command: &str, args: &Value) -> Result<PlaceRequest, EngineError> {
        let text = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
        let flag = |k: &str| args.get(k).and_then(Value::as_bool).unwrap_or(false);
        Ok(PlaceRequest {
            label: command.to_string(),
            base: str_arg(command, args, "base")?.to_string(),
            plugin: text("plugin"),
            cell: text("cell"),
            world: text("world"),
            position: vec3(command, args.get("position"), "position")?
                .ok_or_else(|| failed(command, "argument `position` is missing"))?,
            rotation: vec3(command, args.get("rotation"), "rotation")?.unwrap_or([0.0; 3]),
            persistent: flag("persistent"),
            initially_disabled: flag("initially_disabled"),
            edid: text("edid"),
            allow_reference: args
                .get("allow_reference")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            base_local: None,
        })
    }
}

/// Facts of a winning version: record type, EditorID, plugin (load-order index), offset
/// and length of the record.
type WinnerFacts = ([u8; 4], Option<String>, u16, u64, u64);

/// The record type of `form`'s winning version, with its EditorID and location.
fn winner_facts(loaded: &Loaded, form: GlobalForm) -> Option<WinnerFacts> {
    let v = loaded.lo.forms.versions(form).last()?;
    let loc = loaded.lo.record(v.plugin, v.record)?;
    Some((
        loc.signature,
        loaded.lo.edid(v.plugin, loc).map(str::to_string),
        v.plugin,
        loc.offset,
        u64::from(loc.data_size).saturating_add(24),
    ))
}

fn name_of(loaded: &Loaded, form: GlobalForm) -> FormName {
    FormName {
        plugin: loaded
            .lo
            .plugin(form.plugin)
            .map(|p| p.name.clone())
            .unwrap_or_default(),
        object_id: form.object_id,
    }
}

/// Whether `edid` is a usable EditorID: ASCII letters, digits and `_`, 1..=200 bytes.
pub(crate) fn valid_edid(edid: &str) -> bool {
    !edid.is_empty()
        && edid.len() <= 200
        && edid.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// The form of the load order that already has EditorID `edid` (case-insensitive).
pub(crate) fn edid_owner(loaded: &Loaded, edid: &str) -> Option<String> {
    let (index, loc) = loaded.edid_hits(edid).into_iter().next()?;
    Some(match loaded.lo.resolve(index, loc.form_id) {
        Ok(g) => loaded.lo.form_label(g),
        Err(_) => format!(
            "{}:{:08X}",
            loaded.lo.plugin(index).map_or("?", |p| p.name.as_str()),
            loc.form_id
        ),
    })
}

/// The EditorID's owner in the plugin at `path`, if it exists.
pub(crate) fn edid_in_file(path: &Path, edid: &str) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let index = index_file(path, &IndexOptions::default())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(index.records.iter().skip(1).find_map(|r| {
        r.edid_lossy()
            .filter(|e| e.eq_ignore_ascii_case(edid))
            .map(|_| format!("{}:{:08X}", r.signature, r.form_id))
    }))
}

/// Where a cell sits, from its group path in its plugin (lane A's `cells::cell_place_in`
/// over a path instead of an index): interior, a world's persistent cell, or an exterior
/// cell with the grid in its `XCLC`. World FormIDs are as stored in that plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placed {
    Interior,
    Persistent { world: u32 },
    Exterior { world: u32, grid: Grid },
}

fn place_from_path(
    path: &[GroupHeader],
    record: &Record<'_>,
    limits: &DecodeLimits,
) -> Option<Placed> {
    let world = path
        .iter()
        .find(|g| g.group_type == group_type::WORLD_CHILDREN)
        .map(GroupHeader::label_u32);
    let Some(world) = world else {
        return path
            .first()
            .filter(|top| top.label == *b"CELL")
            .map(|_| Placed::Interior);
    };
    if path
        .last()
        .is_some_and(|g| g.group_type == group_type::WORLD_CHILDREN)
    {
        return Some(Placed::Persistent { world });
    }
    let decoded = record.decode(limits).ok()?;
    let subs = match (&decoded, record.subrecords()) {
        (Some(d), _) => d.as_slice(),
        (None, Some(s)) => s,
        (None, None) => return None,
    };
    let grid = subs
        .iter()
        .find(|s| s.signature() == Signature::new(b"XCLC"))
        .and_then(|s| Grid::from_xclc(s.data()))?;
    Some(Placed::Exterior { world, grid })
}

/// Offsets of the records of type `want` directly inside the group the `steps` (group
/// type, label) lead to, from the top level of the plugin at `path`: group and record
/// headers only, siblings skipped by size (as `edit::source::placement_at`).
fn records_under(path: &Path, steps: &[(i32, [u8; 4])], want: [u8; 4]) -> Result<Vec<u64>, String> {
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let mut file = File::open(path).map_err(|e| err(&e))?;
    let len = file.metadata().map_err(|e| err(&e))?.len();
    let mut head = |at: u64| -> Result<[u8; 24], String> {
        let mut buf = [0u8; 24];
        file.seek(SeekFrom::Start(at)).map_err(|e| err(&e))?;
        file.read_exact(&mut buf).map_err(|e| err(&e))?;
        Ok(buf)
    };
    let field = |buf: &[u8; 24], at: usize| -> [u8; 4] {
        buf.get(at..at.saturating_add(4))
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .unwrap_or_default()
    };
    let tes4 = head(0)?;
    let mut at = u64::from(u32::from_le_bytes(field(&tes4, 4))).saturating_add(24);
    let mut end = len;
    let mut depth = 0usize;
    let mut out = Vec::new();
    while at < end {
        let buf = head(at)?;
        let sig = field(&buf, 0);
        let size = u64::from(u32::from_le_bytes(field(&buf, 4)));
        if &sig == b"GRUP" {
            let group_end = at.saturating_add(size);
            if size < 24 || group_end > end {
                return Err(err(&format!("group at {at} has a bad size {size}")));
            }
            let label = field(&buf, 8);
            let kind = i32::from_le_bytes(field(&buf, 12));
            if steps.get(depth) == Some(&(kind, label)) {
                at = at.saturating_add(24);
                end = group_end;
                depth = depth.saturating_add(1);
                continue;
            }
            at = group_end;
            continue;
        }
        if depth == steps.len() && sig == want {
            out.push(at);
        }
        at = at.saturating_add(size).saturating_add(24);
    }
    Ok(out)
}

/// The exterior cell of `world` whose grid is `grid`, from any plugin of the load order
/// (every version of a form names the same cell).
fn exterior_cell_at(
    loaded: &Loaded,
    world: GlobalForm,
    grid: Grid,
    limits: &DecodeLimits,
) -> Result<Option<GlobalForm>, EngineError> {
    let block = grid_label(grid.block()).map_err(|e| refuse(ID, "position-out-of-range", e))?;
    let sub = grid_label(grid.sub_block()).map_err(|e| refuse(ID, "position-out-of-range", e))?;
    for (i, p) in loaded.lo.plugins.iter().enumerate() {
        let index = u16::try_from(i).unwrap_or(u16::MAX);
        let Some(raw) = loaded.lo.to_raw(index, world) else {
            continue;
        };
        let steps = [
            (group_type::TOP, *b"WRLD"),
            (group_type::WORLD_CHILDREN, raw.to_le_bytes()),
            (group_type::EXTERIOR_BLOCK, block),
            (group_type::EXTERIOR_SUB_BLOCK, sub),
        ];
        let offsets = records_under(&p.path, &steps, *b"CELL").map_err(|m| failed(ID, m))?;
        let mut file =
            File::open(&p.path).map_err(|e| failed(ID, format!("{}: {e}", p.path.display())))?;
        for offset in offsets {
            let record = read_record_at(&mut file, offset, Some(limits))
                .map_err(|e| failed(ID, format!("{}: {e}", p.name)))?;
            let found = record.subrecords().and_then(|subs| {
                subs.iter()
                    .find(|s| s.signature() == Signature::new(b"XCLC"))
                    .and_then(|s| Grid::from_xclc(s.data()))
            });
            if found == Some(grid) {
                if let Ok(g) = loaded.lo.resolve(index, record.header.form_id) {
                    return Ok(Some(g));
                }
            }
        }
    }
    Ok(None)
}

/// The persistent cell of `world`: the `CELL` directly in its world children group, in
/// any plugin of the load order.
fn persistent_cell_of(
    loaded: &Loaded,
    world: GlobalForm,
) -> Result<Option<GlobalForm>, EngineError> {
    for (i, p) in loaded.lo.plugins.iter().enumerate() {
        let index = u16::try_from(i).unwrap_or(u16::MAX);
        let Some(raw) = loaded.lo.to_raw(index, world) else {
            continue;
        };
        let steps = [
            (group_type::TOP, *b"WRLD"),
            (group_type::WORLD_CHILDREN, raw.to_le_bytes()),
        ];
        let offsets = records_under(&p.path, &steps, *b"CELL").map_err(|m| failed(ID, m))?;
        if let Some(offset) = offsets.first() {
            let mut file = File::open(&p.path)
                .map_err(|e| failed(ID, format!("{}: {e}", p.path.display())))?;
            let header = read_record_at(&mut file, *offset, None)
                .map_err(|e| failed(ID, format!("{}: {e}", p.name)))?;
            if let Ok(g) = loaded.lo.resolve(index, header.header.form_id) {
                return Ok(Some(g));
            }
        }
    }
    Ok(None)
}

fn evidence_of(loaded: &Loaded, form: GlobalForm) -> Option<Evidence> {
    let (sig, _, plugin, offset, length) = winner_facts(loaded, form)?;
    Some(Evidence {
        file: loaded.lo.plugin(plugin)?.name.clone(),
        offset,
        length,
        record: Some(format!(
            "{}:{:08X}",
            String::from_utf8_lossy(&sig),
            loaded
                .lo
                .record(plugin, loaded.lo.forms.versions(form).last()?.record)?
                .form_id
        )),
        subrecord: None,
    })
}

fn form_json(loaded: &Loaded, form: GlobalForm) -> Value {
    let facts = winner_facts(loaded, form);
    json!({
        "form": loaded.lo.form_label(form),
        "signature": facts.as_ref().map(|f| String::from_utf8_lossy(&f.0).into_owned()),
        "editor_id": facts.as_ref().and_then(|f| f.1.clone()),
        "winner": facts.as_ref().and_then(|f| loaded.lo.plugin(f.2)).map(|p| p.name.clone()),
    })
}

/// Resolves `req` against the load order (see the module docs).
pub(crate) fn plan(
    command: &str,
    loaded: &Loaded,
    req: &PlaceRequest,
    limits: &DecodeLimits,
) -> Result<Planned, EngineError> {
    let at = &req.label;
    let mut limitations = Vec::new();
    // The arguments' shape first: nothing below is resolved for a request that is wrong.
    if let Some(edid) = &req.edid {
        if !valid_edid(edid) {
            return Err(refuse_arg(
                command,
                at,
                "edid",
                "place.edid-invalid",
                format!("EditorID `{edid}` must be 1 to 200 ASCII letters, digits or `_`"),
            ));
        }
    }
    match (&req.cell, &req.world) {
        (Some(_), Some(_)) => {
            return Err(refuse_arg(
                command,
                at,
                "world",
                "place.cell-and-world",
                "name the cell, or the world with a position, not both".to_string(),
            ))
        }
        (None, None) => {
            return Err(refuse_arg(
                command,
                at,
                "",
                "place.no-cell",
                "name the cell (`cell`) or the world (`world`, the position picks the cell)"
                    .to_string(),
            ))
        }
        _ => {}
    }
    // The base: a form of the load order, or one written in the same call.
    let base = match &req.base_local {
        Some(_) => None,
        None => {
            let base = find_form(command, loaded, &req.base, req.plugin.as_deref())
                .map_err(|e| refuse(command, "no-base", format!("{at}: {e}")))?;
            extend_once(&mut limitations, safe_source(command, loaded, base)?);
            Some(base)
        }
    };
    let base_sig = match (&req.base_local, base) {
        (Some((_, sig)), _) => *sig,
        (None, Some(base)) => {
            winner_facts(loaded, base)
                .ok_or_else(|| {
                    refuse(
                        command,
                        "no-base",
                        format!("{at}: {} has no version", loaded.lo.form_label(base)),
                    )
                })?
                .0
        }
        (None, None) => return Err(refuse(command, "no-base", format!("{at}: no base"))),
    };
    let base_label = match (&req.base_local, base) {
        (Some((edid, _)), _) => format!("new form {edid}"),
        (None, Some(b)) => loaded.lo.form_label(b),
        (None, None) => String::new(),
    };
    if NOT_PLACEABLE.iter().any(|s| **s == base_sig) {
        return Err(refuse(
            command,
            "base-not-placeable",
            format!(
                "{at}: {} is a {}; a placed reference names a base object, never a reference, cell, world or navmesh (corpus: every REFR and ACHR NAME)",
                base_label,
                String::from_utf8_lossy(&base_sig)
            ),
        ));
    }
    if &base_sig == b"LVLN" {
        return Err(refuse(
            command,
            "base-not-placeable",
            format!("{at}: a leveled actor (LVLN) base: whether it is placed as an ACHR is not checked against the corpus yet"),
        ));
    }
    let signature = if &base_sig == b"NPC_" {
        Signature::new(b"ACHR")
    } else {
        Signature::new(b"REFR")
    };

    // The cell.
    let cell = match (&req.cell, &req.world) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(refuse(command, "no-cell", format!("{at}: checked above")))
        }
        (Some(c), None) => find_form(command, loaded, c, req.plugin.as_deref())
            .map_err(|e| refuse(command, "no-cell", format!("{at}: {e}")))?,
        (None, Some(w)) => {
            let world = find_form(command, loaded, w, req.plugin.as_deref())
                .map_err(|e| refuse(command, "no-world", format!("{at}: {e}")))?;
            match winner_facts(loaded, world) {
                Some((sig, ..)) if &sig == b"WRLD" => {}
                _ => {
                    return Err(refuse(
                        command,
                        "not-a-world",
                        format!("{at}: {} is not a WRLD", loaded.lo.form_label(world)),
                    ))
                }
            }
            let grid = Grid::of_position(req.position[0], req.position[1]).ok_or_else(|| {
                refuse(
                    command,
                    "position-out-of-range",
                    format!("{at}: the position names no cell an i16 grid can hold"),
                )
            })?;
            if req.persistent {
                persistent_cell_of(loaded, world)?.ok_or_else(|| {
                    refuse(
                        command,
                        "no-cell",
                        format!(
                            "{at}: {} has no persistent cell in the load order",
                            loaded.lo.form_label(world)
                        ),
                    )
                })?
            } else {
                exterior_cell_at(loaded, world, grid, limits)?.ok_or_else(|| {
                    refuse(
                        command,
                        "no-exterior-cell",
                        format!(
                            "{at}: {} has no exterior cell at grid ({}, {}) in the load order; creating an exterior cell is not supported",
                            loaded.lo.form_label(world),
                            grid.x,
                            grid.y
                        ),
                    )
                })?
            }
        }
    };
    let cell_sig = winner_facts(loaded, cell).map(|f| f.0);
    if cell_sig != Some(*b"CELL") {
        return Err(refuse(
            command,
            "not-a-cell",
            format!("{at}: {} is not a CELL", loaded.lo.form_label(cell)),
        ));
    }
    extend_once(&mut limitations, safe_source(command, loaded, cell)?);
    let (mut copy, mut origin) = winning_copy(command, loaded, cell, Vec::new(), limits)?;
    let cell_plugin = loaded
        .lo
        .forms
        .versions(cell)
        .last()
        .map(|v| v.plugin)
        .unwrap_or_default();
    let placed = place_from_path(&copy.path, &copy.record, limits).ok_or_else(|| {
        refuse(
            command,
            "not-a-cell",
            format!(
                "{at}: {} is not in a cell group its plugin's layout explains",
                loaded.lo.form_label(cell)
            ),
        )
    })?;
    let world_of = |raw: u32| -> Result<GlobalForm, EngineError> {
        loaded
            .lo
            .resolve(cell_plugin, raw)
            .map_err(|e| failed(command, format!("{at}: the cell's world: {e}")))
    };
    // A persistent reference of a world lives in the world's persistent cell.
    let (cell, place) = match placed {
        Placed::Interior => (
            cell,
            RefPlace::Interior {
                cell: name_of(loaded, cell),
            },
        ),
        Placed::Persistent { world } => {
            if !req.persistent {
                return Err(refuse(
                    command,
                    "persistence",
                    format!("{at}: {} is a world's persistent cell, which holds persistent references only (corpus); pass persistent, or name an exterior cell", loaded.lo.form_label(cell)),
                ));
            }
            let world = world_of(world)?;
            (
                cell,
                RefPlace::Persistent {
                    world: name_of(loaded, world),
                    cell: name_of(loaded, cell),
                },
            )
        }
        Placed::Exterior { world, grid } => {
            let world = world_of(world)?;
            let at_grid = Grid::of_position(req.position[0], req.position[1]);
            if at_grid != Some(grid) {
                // 0.02 % of the corpus's exterior references sit outside their cell's square
                // (lane A), so this is reported, not refused (validate: ref.outside-block).
                limitations.push(format!(
                    "place.position-outside-cell: {at}: position ({}, {}) is in grid {:?}, but {} is at grid ({}, {}) (100 units a cell, lane A's grid rule); the reference is written into the named cell",
                    req.position[0],
                    req.position[1],
                    at_grid.map(|g| (g.x, g.y)),
                    loaded.lo.form_label(cell),
                    grid.x,
                    grid.y
                ));
            }
            if req.persistent {
                let pcell = persistent_cell_of(loaded, world)?.ok_or_else(|| {
                    refuse(
                        command,
                        "no-cell",
                        format!(
                            "{at}: {} has no persistent cell in the load order",
                            loaded.lo.form_label(world)
                        ),
                    )
                })?;
                extend_once(&mut limitations, safe_source(command, loaded, pcell)?);
                let (c, o) = winning_copy(command, loaded, pcell, Vec::new(), limits)?;
                copy = c;
                origin = o;
                limitations.push(format!(
                    "{at}: a persistent reference of a world goes into the world's persistent cell {} (corpus: exterior cells have no persistent children)",
                    loaded.lo.form_label(pcell)
                ));
                (
                    pcell,
                    RefPlace::Persistent {
                        world: name_of(loaded, world),
                        cell: name_of(loaded, pcell),
                    },
                )
            } else {
                (
                    cell,
                    RefPlace::Exterior {
                        world: name_of(loaded, world),
                        cell: name_of(loaded, cell),
                        grid,
                    },
                )
            }
        }
    };
    copy.carried = true;

    // The EditorID (its shape was checked first).
    if let Some(edid) = &req.edid {
        if let Some(owner) = edid_owner(loaded, edid) {
            return Err(refuse(
                command,
                "edid-clash",
                format!("{at}: EditorID `{edid}` is already used by {owner}"),
            ));
        }
    }

    let mut evidence = Vec::new();
    if let Some(base) = base {
        evidence.extend(evidence_of(loaded, base));
    }
    evidence.extend(evidence_of(loaded, cell));
    let world_json = place
        .world()
        .and_then(|w| {
            loaded.lo.find(&w.plugin).map(|p| GlobalForm {
                plugin: p,
                object_id: w.object_id,
            })
        })
        .map(|g| form_json(loaded, g));
    let kind = match &place {
        RefPlace::Interior { .. } => "interior",
        RefPlace::Exterior { .. } => "exterior",
        RefPlace::Persistent { .. } => "persistent",
    };
    let grid = match &place {
        RefPlace::Exterior { grid, .. } => json!([grid.x, grid.y]),
        _ => Value::Null,
    };
    let mut cell_json = form_json(loaded, cell);
    if let Some(obj) = cell_json.as_object_mut() {
        obj.insert("kind".into(), json!(kind));
        obj.insert("grid".into(), grid);
    }
    let info = json!({
        "entry": at,
        "base": match (&req.base_local, base) {
            (Some((edid, sig)), _) => json!({
                "form": req.base, "signature": String::from_utf8_lossy(sig),
                "editor_id": edid, "winner": null,
            }),
            (None, Some(b)) => form_json(loaded, b),
            (None, None) => Value::Null,
        },
        "cell": cell_json,
        "world": world_json,
    });
    Ok(Planned {
        carried: copy,
        carried_origin: origin,
        new_ref: NewRef {
            signature,
            base: base.map_or_else(
                || FormName {
                    plugin: String::new(),
                    object_id: 0,
                },
                |b| name_of(loaded, b),
            ),
            base_local: req.base_local.as_ref().map(|(e, _)| e.clone()),
            place,
            position: req.position,
            rotation: req.rotation,
            persistent: req.persistent,
            initially_disabled: req.initially_disabled,
            editor_id: req.edid.clone(),
            allow_reference: req.allow_reference.clone(),
            label: at.clone(),
        },
        info,
        evidence,
        limitations,
    })
}

/// The `placed` entries of a result: each new reference with its base, cell, members and
/// where it was written, in `report`'s new-form order (`infos` in the same order).
pub(crate) fn placed_json(report: &PatchReport, infos: &[(Value, &PlaceRequest)]) -> Vec<Value> {
    report
        .new_forms
        .iter()
        .zip(infos)
        .map(|(n, (info, req))| {
            let written = report.records.iter().find(|r| r.form_id == n.form_id && !r.parent);
            let mut entry = info.clone();
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("form".into(), json!(written.map(|w| w.formref.clone())));
                obj.insert("form_id".into(), json!(format!("{:08X}", n.form_id)));
                obj.insert("signature".into(), json!(written.map(|w| w.signature.clone())));
                obj.insert("editor_id".into(), json!(req.edid));
                obj.insert("stored_base".into(), json!(format!("{:08X}", n.base)));
                obj.insert("stored_cell".into(), json!(format!("{:08X}", n.cell)));
                obj.insert("stored_world".into(), json!(n.world.map(|w| format!("{w:08X}"))));
                obj.insert("position".into(), json!(req.position));
                obj.insert("rotation".into(), json!(req.rotation));
                obj.insert("persistent".into(), json!(req.persistent));
                obj.insert("initially_disabled".into(), json!(req.initially_disabled));
                obj.insert(
                    "group_path".into(),
                    Value::Array(
                        n.path
                            .iter()
                            .map(|(t, l)| {
                                let label = if *t == 0 {
                                    String::from_utf8_lossy(&l.to_le_bytes()).into_owned()
                                } else {
                                    format!("{l:08X}")
                                };
                                json!({"type": t, "label": label})
                            })
                            .collect(),
                    ),
                );
                obj.insert(
                    "members".into(),
                    Value::Array(
                        n.members
                            .iter()
                            .map(|m| json!({"member": m.member, "confidence": m.confidence.to_string(), "source": m.source}))
                            .collect(),
                    ),
                );
                obj.insert("offset".into(), json!(written.and_then(|w| w.offset)));
                obj.insert("length".into(), json!(written.and_then(|w| w.length)));
            }
            entry
        })
        .collect()
}

/// For each carried record: whether its bytes in `target` equal its winner's in the load
/// order (lstrings inlined from a localized plugin change them).
pub(crate) fn carried_json(loaded: &Loaded, report: &PatchReport, target: &Path) -> Vec<Value> {
    let read = |path: &Path, offset: u64, length: u64| -> Option<Vec<u8>> {
        let mut file = File::open(path).ok()?;
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut buf = vec![0u8; usize::try_from(length).ok()?];
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    };
    report
        .records
        .iter()
        .filter(|r| r.parent)
        .map(|r| {
            let owner = usize::try_from(r.form_id >> 24)
                .ok()
                .and_then(|i| report.masters.get(i));
            let winner = owner.and_then(|o| {
                let p = loaded.lo.find(o)?;
                let g = GlobalForm {
                    plugin: p,
                    object_id: r.form_id & 0x00FF_FFFF,
                };
                let (_, _, plugin, offset, length) = winner_facts(loaded, g)?;
                Some((loaded.lo.plugin(plugin)?.path.clone(), offset, length))
            });
            let identical = match (winner, r.offset, r.length) {
                (Some((path, offset, length)), Some(o), Some(l)) => {
                    let a = read(&path, offset, length);
                    let b = read(target, o, l);
                    Some(a.is_some() && a == b)
                }
                _ => None,
            };
            json!({
                "form": r.formref, "signature": r.signature, "editor_id": r.editor_id,
                "source": r.source, "outcome": r.outcome,
                "identical_to_winner": identical,
                "inlined_lstrings": r.inlined.len(),
            })
        })
        .collect()
}

/// Refuses an EditorID the target already has.
pub(crate) fn check_target_edids(
    command: &str,
    into: Option<&Path>,
    requests: &[&PlaceRequest],
) -> Result<(), EngineError> {
    for req in requests {
        let Some(into) = into else {
            break;
        };
        if let Some(edid) = &req.edid {
            if let Some(owner) = edid_in_file(into, edid).map_err(|m| failed(command, m))? {
                return Err(refuse(
                    command,
                    "edid-clash",
                    format!(
                        "{}: EditorID `{edid}` is already used by {owner} in {}",
                        req.label,
                        into.display()
                    ),
                ));
            }
        }
    }
    let mut seen: Vec<String> = Vec::new();
    for req in requests {
        if let Some(edid) = &req.edid {
            let key = edid.to_ascii_lowercase();
            if seen.contains(&key) {
                return Err(refuse(
                    command,
                    "edid-clash",
                    format!(
                        "{}: EditorID `{edid}` is given to two new references",
                        req.label
                    ),
                ));
            }
            seen.push(key);
        }
    }
    Ok(())
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let req = PlaceRequest::from_args(ID, args)?;
    let into = PathBuf::from(str_arg(ID, args, "into")?);
    let allow_untyped = args
        .get("allow_untyped_lstrings")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let loaded = acquire(session, args, ID)?;
    guard(ID, loaded, &into)?;
    let limits = DecodeLimits::default();
    let planned = plan(ID, loaded, &req, &limits)?;
    check_target_edids(ID, Some(&into), &[&req])?;
    let mut limitations = loaded.limitations.clone();
    extend_once(&mut limitations, planned.limitations.clone());

    let staged = stage(ID, &into, true)?;
    let mut report = write_with(
        ID,
        loaded,
        &staged.path,
        vec![planned.carried],
        vec![planned.new_ref],
        allow_untyped,
    )?;
    let checked = check(ID, loaded, &staged)?;
    let written: Vec<String> = report
        .records
        .iter()
        .map(|r| format!("{}:{:08X}", r.signature, r.form_id))
        .collect();
    let (blocking, other): (Vec<&validate::Finding>, Vec<&validate::Finding>) = checked
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .partition(|f| f.record.as_ref().is_none_or(|r| written.contains(r)));
    if !blocking.is_empty() {
        return Err(refuse(
            ID,
            "fails-validate",
            format!(
                "the written records fail validate, so {} was not changed: {}",
                into.display(),
                describe(&blocking)
            ),
        ));
    }
    if !other.is_empty() {
        limitations.push(format!(
            "{} has {} validate error(s) on records this call did not write: {}",
            into.display(),
            other.len(),
            describe(&other)
        ));
    }
    install(ID, &staged, &into)?;
    report.target = into.clone();
    let mut result = outcome(&report, &[], limitations);
    result.evidence.extend(planned.evidence);
    let placed = placed_json(&report, &[(planned.info, &req)]);
    let carried = carried_json(loaded, &report, &into);
    if let Some(obj) = result.data.as_object_mut() {
        obj.insert("placed".into(), Value::Array(placed));
        obj.insert("carried".into(), Value::Array(carried));
        obj.insert(
            "carried_from".into(),
            json!({
                "plugin": planned.carried_origin.plugin, "offset": planned.carried_origin.offset,
                "record": planned.carried_origin.label,
            }),
        );
        obj.insert(
            "next_object_id".into(),
            json!(report.next_object_id.map(|n| format!("{n:08X}"))),
        );
        obj.insert("validate".into(), validate_json(&checked));
    }
    Ok(result)
}
