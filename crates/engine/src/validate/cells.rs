//! Cell rules (M4.E4): where cells and their children sit, what references name, and what
//! the `TES4` counters say about them. Every structural fact is lane A's corpus rule
//! (`kitkraft_esm::cells`, `docs/corpus.md` "Cell and world groups"), asserted on every
//! group and record of the 14 official files; the persistence warning is a grade D
//! hypothesis until the game is observed. Rules and sources: `docs/validate.md`.
//!
//! The walk calls [`on_group`] for every group (with the groups above it), [`on_record`]
//! for every record of a cell child type, a `CELL`, an `LCTN`, or anything that names a
//! reference, and [`finish`] once the plugin is read.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use kitkraft_esm::cells::{
    grid_label, group_type, interior_block, Grid, CELL_CHILD_TYPES, FLAG_PERSISTENT,
};
use kitkraft_esm::index::read_record_at;
use kitkraft_esm::{GroupHeader, Record, Signature, Subrecord};

use super::forms::Target;
use super::{Finding, Pass, Severity};

/// Record flag "deleted" (UESP, Skyrim_Mod:Mod_File_Format, "Records").
const DELETED: u32 = 0x0000_0020;

/// The cell rules, by id (the registry has their summaries).
pub const RULES: [&str; 9] = [
    "cell.missing-parent",
    "ref.orphan",
    "ref.outside-block",
    "ref.persistent-flag",
    "ref.persistence",
    "cell.actor-without-navmesh",
    "cell.incc",
    "navmesh.unchanged",
    "location.added-ref-temporary",
];

/// What the walk collects for the rules that need the whole plugin.
#[derive(Debug, Default)]
pub(super) struct Facts {
    /// `INCC` of the `TES4`, when present.
    pub incc: Option<u32>,
    /// Interior `CELL` records seen.
    pub interior_cells: u32,
    /// Exterior cells seen: stored FormID -> grid.
    exterior: HashMap<u32, Grid>,
    /// Exterior cells seen by (world, grid), stored FormIDs.
    exterior_at: HashMap<(u32, Grid), u32>,
    /// `ACHR`s in a world's persistent cell: (world, grid of the `DATA` position, `SIG:FORMID`).
    /// The persistent cell holds no navmesh; the actor stands in the exterior cell its
    /// position names.
    world_actors: Vec<(u32, Option<Grid>, String)>,
    /// References (`REFR`, `ACHR`) in temporary children: stored FormID -> `SIG:FORMID`.
    temporary: HashMap<u32, String>,
    /// FormIDs named by quests, scenes, packages and dialogue ([`NAMING_TYPES`]), or by a
    /// reference's door link (`XTEL`): stored FormID -> (referrer `SIG:FORMID`, member
    /// path), first referrer.
    named: HashMap<u32, (String, String)>,
    /// Cells holding an `ACHR`: stored cell FormID -> the first actor's `SIG:FORMID`.
    actors: BTreeMap<u32, String>,
    /// Cells with a `NAVM` in this plugin.
    navmeshed: HashSet<u32>,
    /// `ACPR` (added persistent refs) entries of `LCTN`s, for
    /// `location.added-ref-temporary`: (location `SIG:FORMID`, entry index, the entry's
    /// `Ref`, the offset of the entry in the file, when it can be told).
    added: Vec<(String, usize, u32, Option<u64>)>,
}

impl Facts {
    /// Whether any cell rule is on.
    pub(super) fn any(pass: &Pass<'_>) -> bool {
        RULES.iter().any(|r| pass.on(r))
    }
}

fn finding(
    pass: &Pass<'_>,
    rule: &'static str,
    severity: Severity,
    record: Option<String>,
    subrecord: Option<String>,
    offset: Option<u64>,
    message: String,
) -> Finding {
    Finding {
        rule,
        severity,
        plugin: pass.name.clone(),
        record,
        subrecord,
        offset,
        message,
    }
}

/// Whether the plugin or one of its masters has the record `raw` (stored FormID): `None`
/// when that cannot be told without guessing.
fn has_form(pass: &Pass<'_>, raw: u32) -> Option<bool> {
    match pass.space.resolve(raw) {
        Target::Null => Some(false),
        Target::Beyond { .. } => Some(false),
        Target::Unknown(_) => None,
        Target::Form {
            owner, object_id, ..
        } => {
            if owner.eq_ignore_ascii_case(&pass.name) {
                Some(pass.own_ids.contains(&raw))
            } else if pass.can_check(&owner, object_id) {
                Some(pass.master_has(&owner, object_id).is_some())
            } else {
                None
            }
        }
    }
}

/// `cell.missing-parent` for a world (type 1) or cell (type 6) children group: its record
/// must be in the plugin (every official file carries it right before the group); a
/// master's record not carried is a warning, a record nobody has an error.
pub(super) fn on_group(
    pass: &Pass<'_>,
    header: &GroupHeader,
    offset: Option<u64>,
    findings: &mut Vec<Finding>,
) {
    if !pass.on("cell.missing-parent") {
        return;
    }
    let kind = match header.group_type {
        group_type::WORLD_CHILDREN => "WRLD",
        group_type::CELL_CHILDREN => "CELL",
        _ => return,
    };
    let raw = header.label_u32();
    if pass.own_ids.contains(&raw) {
        return;
    }
    let what = format!("{kind}:{raw:08X}");
    match has_form(pass, raw) {
        Some(true) => findings.push(finding(
            pass,
            "cell.missing-parent",
            Severity::Warning,
            Some(what.clone()),
            None,
            offset,
            format!(
                "the children group of {what} has no {kind} record in this plugin; its master has one, but no official file has a children group without its record right before it (carry the {kind} as an override)"
            ),
        )),
        Some(false) => findings.push(finding(
            pass,
            "cell.missing-parent",
            Severity::Error,
            Some(what.clone()),
            None,
            offset,
            format!(
                "the children group of {what} names a {kind} that neither this plugin nor its masters have: its records have no {} to be in",
                if kind == "WRLD" { "world" } else { "cell" }
            ),
        )),
        None => {}
    }
}

fn sub<'s>(subs: &'s [Subrecord<'_>], sig: &[u8; 4]) -> Option<&'s Subrecord<'s>> {
    subs.iter().find(|s| s.signature() == Signature(*sig))
}

/// The cell children group a record sits in: `(cell stored FormID, group type 8 or 9)`.
fn children_of(path: &[GroupHeader]) -> Option<(u32, i32)> {
    let last = path.last()?;
    matches!(
        last.group_type,
        group_type::CELL_PERSISTENT | group_type::CELL_TEMPORARY
    )
    .then(|| (last.label_u32(), last.group_type))
}

/// The world whose persistent cell holds the children group at the end of `path` (world
/// children > cell children > persistent or temporary children), if it is one.
fn world_persistent_cell(path: &[GroupHeader]) -> Option<u32> {
    let n = path.len();
    let world = path.get(n.checked_sub(3)?)?;
    let cell = path.get(n.checked_sub(2)?)?;
    (world.group_type == group_type::WORLD_CHILDREN && cell.group_type == group_type::CELL_CHILDREN)
        .then(|| world.label_u32())
}

/// The x and y of a reference's `DATA` position.
fn position_xy(subs: &[Subrecord<'_>]) -> Option<(f32, f32)> {
    let data = sub(subs, b"DATA")?.data();
    let f = |i: usize| {
        data.get(i..i.saturating_add(4))
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(f32::from_le_bytes)
    };
    Some((f(0)?, f(4)?))
}

/// The record rules for one record (see the module docs). `subs` are its subrecords
/// (decompressed); `id` is `SIG:FORMID`.
pub(super) fn on_record(
    pass: &mut Pass<'_>,
    path: &[GroupHeader],
    r: &Record<'_>,
    subs: &[Subrecord<'_>],
    id: &str,
    findings: &mut Vec<Finding>,
) {
    let sig = r.header.signature;
    let at = r.offset();
    let persistent = r.header.flags & FLAG_PERSISTENT != 0;
    if sig == Signature(*b"CELL") {
        cell_record(pass, path, r, subs, id, findings);
        return;
    }
    // location.added-ref-temporary: the ACPR entries are collected now; whether they name
    // a temporary reference is decided in finish, when the walk has seen every cell child.
    if sig == Signature(*b"LCTN") {
        added_refs(pass, subs, at, id);
        return;
    }
    if !CELL_CHILD_TYPES.contains(&sig) {
        return;
    }
    let Some((cell, group)) = children_of(path) else {
        if pass.on("cell.missing-parent") {
            findings.push(finding(
                pass,
                "cell.missing-parent",
                Severity::Error,
                Some(id.to_string()),
                None,
                at,
                format!("a {sig} outside a cell's persistent or temporary children: the corpus has every {sig} there"),
            ));
        }
        return;
    };
    // ref.persistent-flag: flag 0x400 exactly in type 8; never on a NAVM.
    if pass.on("ref.persistent-flag") {
        let in_persistent = group == group_type::CELL_PERSISTENT;
        if persistent != in_persistent || (sig == Signature(*b"NAVM") && persistent) {
            findings.push(finding(
                pass,
                "ref.persistent-flag",
                Severity::Error,
                Some(id.to_string()),
                None,
                at,
                format!(
                    "record flag 0x400 is {} but the record is in the cell's {} children (group type {group}); the corpus sets it exactly on records in persistent children (type 8), never on a NAVM",
                    if persistent { "set" } else { "clear" },
                    if in_persistent { "persistent" } else { "temporary" }
                ),
            ));
        }
    }
    let deleted = r.header.flags & DELETED != 0;
    if sig == Signature(*b"NAVM") {
        pass.cells.navmeshed.insert(cell);
        if pass.on("navmesh.unchanged") {
            navmesh_unchanged(pass, r, subs, id, findings);
        }
        return;
    }
    if group == group_type::CELL_TEMPORARY
        && (sig == Signature(*b"REFR") || sig == Signature(*b"ACHR"))
    {
        pass.cells
            .temporary
            .insert(r.header.form_id, id.to_string());
    }
    if sig == Signature(*b"ACHR") && !deleted {
        match world_persistent_cell(path) {
            Some(world) => {
                let grid = position_xy(subs).and_then(|(x, y)| Grid::of_position(x, y));
                pass.cells.world_actors.push((world, grid, id.to_string()));
            }
            None => {
                pass.cells
                    .actors
                    .entry(cell)
                    .or_insert_with(|| id.to_string());
            }
        }
    }
    if sig == Signature(*b"REFR") || sig == Signature(*b"ACHR") {
        if let Some(xtel) = sub(subs, b"XTEL").and_then(|s| s.data().get(0..4)) {
            if let Ok(b) = <[u8; 4]>::try_from(xtel) {
                pass.cells
                    .named
                    .entry(u32::from_le_bytes(b))
                    .or_insert_with(|| (id.to_string(), "XTEL".to_string()));
            }
        }
    }
    // ref.orphan: the base must exist.
    if pass.on("ref.orphan")
        && !deleted
        && (sig == Signature(*b"REFR") || sig == Signature(*b"ACHR"))
    {
        match sub(subs, b"NAME").map(Subrecord::data) {
            None => findings.push(finding(
                pass,
                "ref.orphan",
                Severity::Error,
                Some(id.to_string()),
                Some("NAME".into()),
                at,
                format!("a {sig} without NAME: every reference of the corpus that is not deleted names its base"),
            )),
            Some(data) => {
                let raw = <[u8; 4]>::try_from(data).ok().map(u32::from_le_bytes);
                match raw.map(|raw| (raw, has_form(pass, raw))) {
                    Some((raw, Some(false))) => findings.push(finding(
                        pass,
                        "ref.orphan",
                        Severity::Error,
                        Some(id.to_string()),
                        Some("NAME".into()),
                        sub(subs, b"NAME").and_then(Subrecord::data_offset).or(at),
                        format!("the base {raw:08X} resolves to no record of this plugin or its masters: the game cannot place this reference"),
                    )),
                    None => findings.push(finding(
                        pass,
                        "ref.orphan",
                        Severity::Error,
                        Some(id.to_string()),
                        Some("NAME".into()),
                        at,
                        format!("NAME is {} bytes, not a FormID", data.len()),
                    )),
                    _ => {}
                }
            }
        }
    }
    // ref.outside-block (warning): an exterior reference outside its cell's square.
    if pass.on("ref.outside-block") {
        if let (Some(grid), Some(data)) = (
            pass.cells.exterior.get(&cell).copied(),
            sub(subs, b"DATA").map(Subrecord::data),
        ) {
            let f = |i: usize| {
                data.get(i..i.saturating_add(4))
                    .and_then(|b| <[u8; 4]>::try_from(b).ok())
                    .map(f32::from_le_bytes)
            };
            if let (Some(x), Some(y)) = (f(0), f(4)) {
                let at_grid = Grid::of_position(x, y);
                if at_grid != Some(grid) {
                    findings.push(finding(
                        pass,
                        "ref.outside-block",
                        Severity::Warning,
                        Some(id.to_string()),
                        Some("DATA".into()),
                        sub(subs, b"DATA").and_then(Subrecord::data_offset).or(at),
                        format!(
                            "position ({x}, {y}) is in grid {:?}, outside its cell's grid ({}, {}) (100 units a cell; 0.02 % of the corpus's exterior references are)",
                            at_grid.map(|g| (g.x, g.y)),
                            grid.x,
                            grid.y
                        ),
                    ));
                }
            }
        }
    }
}

fn cell_record(
    pass: &mut Pass<'_>,
    path: &[GroupHeader],
    r: &Record<'_>,
    subs: &[Subrecord<'_>],
    id: &str,
    findings: &mut Vec<Finding>,
) {
    let at = r.offset();
    let form_id = r.header.form_id;
    let persistent = r.header.flags & FLAG_PERSISTENT != 0;
    let parent = path.last().map(|g| g.group_type);
    let top_cell = path.first().is_some_and(|g| g.label == *b"CELL");
    if top_cell {
        pass.cells.interior_cells = pass.cells.interior_cells.saturating_add(1);
    }
    // ref.persistent-flag: flag 0x400 exactly on a world's persistent cell.
    let world_persistent = parent == Some(group_type::WORLD_CHILDREN);
    if pass.on("ref.persistent-flag") && persistent != world_persistent {
        findings.push(finding(
            pass,
            "ref.persistent-flag",
            Severity::Error,
            Some(id.to_string()),
            None,
            at,
            format!(
                "record flag 0x400 is {} on a CELL {}; the corpus sets it exactly on a world's persistent cell (the CELL directly in the world's children)",
                if persistent { "set" } else { "clear" },
                if world_persistent { "directly in a world's children" } else { "in a block" }
            ),
        ));
    }
    if parent == Some(group_type::EXTERIOR_SUB_BLOCK) {
        if let Some(grid) = sub(subs, b"XCLC").and_then(|s| Grid::from_xclc(s.data())) {
            if let Some(world) = path
                .iter()
                .find(|g| g.group_type == group_type::WORLD_CHILDREN)
            {
                pass.cells
                    .exterior_at
                    .entry((world.label_u32(), grid))
                    .or_insert(form_id);
            }
            if !pass.on("ref.outside-block") {
                pass.cells.exterior.insert(form_id, grid);
            }
        }
    }
    if !pass.on("ref.outside-block") {
        return;
    }
    let mut wrong: Option<String> = None;
    if top_cell {
        let (block, sub_block) = interior_block(form_id);
        let labels = (
            path.get(1)
                .filter(|g| g.group_type == group_type::INTERIOR_BLOCK),
            path.get(2)
                .filter(|g| g.group_type == group_type::INTERIOR_SUB_BLOCK),
        );
        match labels {
            (Some(b), Some(s)) if path.len() == 3 => {
                if b.label_u32() != block || s.label_u32() != sub_block {
                    wrong = Some(format!(
                        "interior cell in block {} sub-block {}, but its object ID puts it in block {block} sub-block {sub_block} (the last two decimal digits; corpus: all 17,020 official interior cells)",
                        b.label_u32(),
                        s.label_u32()
                    ));
                }
            }
            _ => {
                wrong = Some(
                    "interior cell not in a block and sub-block (types 2 and 3) of the top-level CELL group"
                        .into(),
                )
            }
        }
    } else if parent == Some(group_type::EXTERIOR_SUB_BLOCK) {
        match sub(subs, b"XCLC").and_then(|s| Grid::from_xclc(s.data())) {
            None => wrong = Some(
                "exterior cell without an XCLC grid (every exterior cell of the corpus has one)"
                    .into(),
            ),
            Some(grid) => {
                pass.cells.exterior.insert(form_id, grid);
                let want = (grid_label(grid.block()), grid_label(grid.sub_block()));
                let have = (
                    path.get(path.len().saturating_sub(2)).map(|g| g.label),
                    path.last().map(|g| g.label),
                );
                if let (Ok(block), Ok(sub_block)) = want {
                    if have != (Some(block), Some(sub_block)) {
                        wrong = Some(format!(
                            "exterior cell at grid ({}, {}) is not in block ({}, {}) sub-block ({}, {}) (sub-block = grid / 8, block = sub-block / 4; corpus: all 22,682 official exterior cells)",
                            grid.x,
                            grid.y,
                            grid.block().x,
                            grid.block().y,
                            grid.sub_block().x,
                            grid.sub_block().y
                        ));
                    }
                }
            }
        }
    } else if !world_persistent {
        wrong = Some("a CELL outside the top-level CELL group and a world's children".into());
    }
    if let Some(message) = wrong {
        // A small or medium plugin's own cells may be numbered by a class width the
        // corpus shows once (SFBGS047): a warning there.
        let severity = if pass.space.scaled() {
            Severity::Warning
        } else {
            Severity::Error
        };
        findings.push(finding(
            pass,
            "ref.outside-block",
            severity,
            Some(id.to_string()),
            None,
            at,
            message,
        ));
    }
}

/// `navmesh.unchanged`: a `NAVM` override whose subrecords equal its master's version.
fn navmesh_unchanged(
    pass: &Pass<'_>,
    r: &Record<'_>,
    subs: &[Subrecord<'_>],
    id: &str,
    findings: &mut Vec<Finding>,
) {
    let Target::Form {
        owner, object_id, ..
    } = pass.space.resolve(r.header.form_id)
    else {
        return;
    };
    if owner.eq_ignore_ascii_case(&pass.name) {
        return;
    }
    let Some((file, offset)) = pass.master_has(&owner, object_id) else {
        return;
    };
    let Some(path) = file.path.as_ref() else {
        return;
    };
    let Ok(mut input) = File::open(path) else {
        return;
    };
    let Ok(master) = read_record_at(&mut input, offset, Some(&pass.limits)) else {
        return;
    };
    let theirs: Vec<(Signature, &[u8])> = master
        .subrecords()
        .unwrap_or_default()
        .iter()
        .map(|s| (s.signature(), s.data()))
        .collect();
    let ours: Vec<(Signature, &[u8])> = subs.iter().map(|s| (s.signature(), s.data())).collect();
    if theirs == ours {
        findings.push(finding(
            pass,
            "navmesh.unchanged",
            Severity::Warning,
            Some(id.to_string()),
            None,
            r.offset(),
            format!(
                "this NAVM equals {}'s version: an unchanged navmesh override changes nothing in game, conflicts with any other plugin's navmesh for the cell, and must be listed in ONAM; drop it",
                file.name
            ),
        ));
    }
}

/// Record types whose FormIDs to a temporary reference `ref.persistence` reports: quests
/// (aliases), scenes, packages, dialogue and story manager nodes. Not `WRLD` (its `RNAM`
/// large-reference lists), `LCTN` (`LCSR`, `LCPR`) or anything else the official files use
/// to name temporary references: `Starfield.esm` alone has 48,538 such names, of which
/// the first thousand are `WRLD.RNAM` (435), `LCTN.LCSR` (398) and `LCTN.LCPR` (166)
/// (`corpus:` `validate_corpus_official_files`, 2026-10-14), so they are not evidence of a
/// problem.
const NAMING_TYPES: [&[u8; 4]; 9] = [
    b"QUST", b"SCEN", b"PACK", b"INFO", b"DIAL", b"DLBR", b"SMQN", b"SMBN", b"SMEN",
];

/// Collect what `record` names, for `ref.persistence` (see [`NAMING_TYPES`]).
pub(super) fn named_by(pass: &mut Pass<'_>, id: &str, sig: Signature, path: &str, raw: u32) {
    if !pass.on("ref.persistence") || !NAMING_TYPES.contains(&&sig.0) || raw == 0 {
        return;
    }
    pass.cells
        .named
        .entry(raw)
        .or_insert_with(|| (id.to_string(), path.to_string()));
}

/// The `ACPR` (added persistent refs) entries of a location, for
/// `location.added-ref-temporary`: each is 16 bytes (`Ref`, `WorldOrCell`, `GridY`,
/// `GridX`, 4 bytes); an entry that names no reference (`Ref` 0, "or none") is not
/// collected. A payload that does not decode is already `layout.undecoded`; its
/// undecodable tail is skipped here.
fn added_refs(pass: &mut Pass<'_>, subs: &[Subrecord<'_>], at: Option<u64>, id: &str) {
    if !pass.on("location.added-ref-temporary") {
        return;
    }
    for s in subs.iter().filter(|s| s.signature() == Signature(*b"ACPR")) {
        let base = s.data_offset().or(at);
        for (index, entry) in s.data().as_chunks::<16>().0.iter().enumerate() {
            let Some(bytes) = entry.get(0..4) else {
                continue;
            };
            let Some(raw) = <[u8; 4]>::try_from(bytes).ok().map(u32::from_le_bytes) else {
                continue;
            };
            if raw == 0 {
                continue;
            }
            let offset = base.and_then(|b| {
                let i = u64::try_from(index).ok()?;
                let o = i.checked_mul(16)?;
                b.checked_add(o)
            });
            pass.cells.added.push((id.to_string(), index, raw, offset));
        }
    }
}

/// The rules that need the whole plugin: `ref.persistence`, `location.added-ref-temporary`,
/// `cell.actor-without-navmesh`, `cell.incc`.
pub(super) fn finish(
    pass: &Pass<'_>,
    plugin: &Path,
    findings: &mut Vec<Finding>,
    limitations: &mut Vec<String>,
) {
    if pass.on("ref.persistence") {
        let mut hits: Vec<(&u32, &String, &(String, String))> = pass
            .cells
            .temporary
            .iter()
            .filter_map(|(raw, rid)| pass.cells.named.get(raw).map(|n| (raw, rid, n)))
            .collect();
        hits.sort_by_key(|(raw, _, _)| **raw);
        for (_, rid, (by, member)) in hits {
            findings.push(finding(
                pass,
                "ref.persistence",
                Severity::Warning,
                Some(rid.clone()),
                None,
                None,
                format!(
                    "{by} names this reference ({member}), but it is in temporary children: a reference another record names is expected to be persistent (grade D: unverified in game until M4.O2; docs/design/persistence.md)"
                ),
            ));
        }
    }
    if pass.on("location.added-ref-temporary") {
        // M5.D2: an added persistent reference that this plugin holds in temporary
        // children (group type 9) is expected to be persistent. Only references the
        // plugin holds are checked, as in ref.persistence; an entry naming a master's
        // reference is counted, the masters are not read.
        let mut master_named = 0usize;
        for (loc, index, raw, offset) in &pass.cells.added {
            let Target::Form { owner, .. } = pass.space.resolve(*raw) else {
                continue; // Null, beyond the load order, or unresolvable: no master reference.
            };
            if !owner.eq_ignore_ascii_case(&pass.name) {
                master_named = master_named.saturating_add(1);
                continue;
            }
            let Some(rid) = pass.cells.temporary.get(raw) else {
                continue; // Not held in temporary children (or not held at all).
            };
            findings.push(finding(
                pass,
                "location.added-ref-temporary",
                Severity::Warning,
                Some(loc.clone()),
                Some("ACPR".into()),
                *offset,
                format!(
                    "ACPR entry {index} names {rid}, which is in this plugin's temporary children (group type 9): an added persistent reference is expected to be persistent (grade D: unverified in game until a CK fixture confirms it; docs/design/persistence.md)"
                ),
            ));
        }
        if master_named > 0 {
            limitations.push(format!(
                "location.added-ref-temporary: {master_named} ACPR entries name master references; their persistence is not checked"
            ));
        }
    }
    if pass.on("cell.actor-without-navmesh") {
        let without: Vec<(u32, &String)> = pass
            .cells
            .actors
            .iter()
            .filter(|(cell, _)| !pass.cells.navmeshed.contains(cell))
            .map(|(cell, id)| (*cell, id))
            .collect();
        if !without.is_empty() {
            let in_masters =
                master_navmeshes(pass, &without.iter().map(|(c, _)| *c).collect::<Vec<_>>());
            for (cell, actor) in without {
                match in_masters.get(&cell) {
                    Some(true) => {}
                    Some(false) => findings.push(finding(
                        pass,
                        "cell.actor-without-navmesh",
                        Severity::Warning,
                        Some(actor.clone()),
                        None,
                        None,
                        format!("cell {cell:08X} has no NAVM in this plugin or its masters: an actor there cannot move (KitKraft cannot generate navmesh; generate it in the Creation Kit, fixture F22, and import it with [[import]])"),
                    )),
                    None => {}
                }
            }
        }
        world_actors(pass, findings, limitations);
    }
    if pass.on("cell.incc") {
        match pass.cells.incc {
            Some(incc) if incc != pass.cells.interior_cells => findings.push(finding(
                pass,
                "cell.incc",
                Severity::Error,
                Some("TES4:00000000".into()),
                Some("INCC".into()),
                None,
                format!(
                    "INCC is {incc}, but the plugin has {} interior cell(s) (corpus: INCC equals the interior cell count in all 14 official files)",
                    pass.cells.interior_cells
                ),
            )),
            None if pass.cells.interior_cells > 0 => findings.push(finding(
                pass,
                "cell.incc",
                Severity::Warning,
                Some("TES4:00000000".into()),
                Some("INCC".into()),
                None,
                format!(
                    "{} has {} interior cell(s) and no INCC (every official file has one)",
                    plugin.display(),
                    pass.cells.interior_cells
                ),
            )),
            _ => {}
        }
    }
}

/// `cell.actor-without-navmesh` for actors in a world's persistent cell: the exterior cell
/// of that world at the grid of the actor's position (the plugin's own, else a master's)
/// must have a navmesh. An actor whose cell cannot be found is a limitation, not a warning.
fn world_actors(pass: &Pass<'_>, findings: &mut Vec<Finding>, limitations: &mut Vec<String>) {
    let mut unchecked: Vec<String> = Vec::new();
    for (world, grid, actor) in &pass.cells.world_actors {
        let Some(grid) = grid else {
            unchecked.push(format!("{actor} (no usable DATA position)"));
            continue;
        };
        let cell = match pass.cells.exterior_at.get(&(*world, *grid)) {
            Some(c) => Some(*c),
            None => exterior_in_masters(pass, *world, *grid),
        };
        let Some(cell) = cell else {
            unchecked.push(format!(
                "{actor} (world {world:08X} has no exterior cell at grid ({}, {}) in this plugin or its masters)",
                grid.x, grid.y
            ));
            continue;
        };
        if pass.cells.navmeshed.contains(&cell) {
            continue;
        }
        // A cell of the plugin's own is not checked against masters, as for the rule's other
        // actors (an official file's own cells are never findings): named, not warned.
        let own = matches!(
            pass.space.resolve(cell),
            Target::Form { ref owner, .. } if owner.eq_ignore_ascii_case(&pass.name)
        );
        if own {
            unchecked.push(format!(
                "{actor} (its exterior cell {cell:08X} is this plugin's own, without a NAVM here)"
            ));
            continue;
        }
        match master_navmeshes(pass, &[cell]).get(&cell) {
            Some(true) => {}
            Some(false) => findings.push(finding(
                pass,
                "cell.actor-without-navmesh",
                Severity::Warning,
                Some(actor.clone()),
                None,
                None,
                format!(
                    "the actor is in world {world:08X}'s persistent cell and stands in its exterior cell {cell:08X} (grid ({}, {}), from its position), which has no NAVM in this plugin or its masters: it cannot move (KitKraft cannot generate navmesh; generate it in the Creation Kit, fixture F22, and import it with [[import]])",
                    grid.x, grid.y
                ),
            )),
            None => unchecked.push(format!(
                "{actor} (no master holding exterior cell {cell:08X} could be read)"
            )),
        }
    }
    if !unchecked.is_empty() {
        let more = unchecked.len().saturating_sub(5);
        limitations.push(format!(
            "cell.actor-without-navmesh: {} actor(s) in a world's persistent cell not checked: {}{}",
            unchecked.len(),
            unchecked
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
            if more > 0 {
                format!("; and {more} more")
            } else {
                String::new()
            }
        ));
    }
}

/// The exterior cell of `world` (stored FormID in the plugin) at `grid` in any master,
/// as a stored FormID of the plugin: group and record headers only, then the `XCLC` of
/// each cell in the grid's sub-block.
fn exterior_in_masters(pass: &Pass<'_>, world: u32, grid: Grid) -> Option<u32> {
    let Target::Form {
        owner, object_id, ..
    } = pass.space.resolve(world)
    else {
        return None;
    };
    let (block, sub_block) = (
        grid_label(grid.block()).ok()?,
        grid_label(grid.sub_block()).ok()?,
    );
    for (name, _) in &pass.space.masters {
        let Some(file) = pass.files.get(&name.to_ascii_lowercase()) else {
            continue;
        };
        let (Some(path), Some(raw_world)) = (
            file.path.as_ref(),
            pass.space_of(file).raw_for(&owner, object_id),
        ) else {
            continue;
        };
        let steps = [
            (group_type::TOP, *b"WRLD"),
            (group_type::WORLD_CHILDREN, raw_world.to_le_bytes()),
            (group_type::EXTERIOR_BLOCK, block),
            (group_type::EXTERIOR_SUB_BLOCK, sub_block),
        ];
        let Ok(offsets) = crate::edit::source::records_under(path, &steps, *b"CELL") else {
            continue;
        };
        let Ok(mut input) = File::open(path) else {
            continue;
        };
        for offset in offsets {
            let Ok(record) = read_record_at(&mut input, offset, Some(&pass.limits)) else {
                continue;
            };
            let at = record
                .subrecords()
                .and_then(|subs| sub(subs, b"XCLC").and_then(|s| Grid::from_xclc(s.data())));
            if at != Some(grid) {
                continue;
            }
            if let Target::Form {
                owner, object_id, ..
            } = pass.space_of(file).resolve(record.header.form_id)
            {
                if let Some(raw) = pass.space.raw_for(&owner, object_id) {
                    return Some(raw);
                }
            }
        }
    }
    None
}

/// For each cell (stored FormID in the plugin), whether a master holds a `NAVM` in its
/// temporary children: `None` when no master could be read for it.
fn master_navmeshes(pass: &Pass<'_>, cells: &[u32]) -> HashMap<u32, bool> {
    let mut out: HashMap<u32, bool> = HashMap::new();
    // Per master file: the raw FormIDs wanted there, back to the plugin's.
    let mut per_file: BTreeMap<String, HashMap<u32, u32>> = BTreeMap::new();
    for cell in cells {
        let Target::Form {
            owner, object_id, ..
        } = pass.space.resolve(*cell)
        else {
            continue;
        };
        for (name, _) in &pass.space.masters {
            let Some(file) = pass.files.get(&name.to_ascii_lowercase()) else {
                continue;
            };
            if let Some(raw) = pass.space_of(file).raw_for(&owner, object_id) {
                per_file
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .insert(raw, *cell);
            }
        }
    }
    for (key, wanted) in per_file {
        let Some(path) = pass.files.get(&key).and_then(|f| f.path.as_ref()) else {
            continue;
        };
        let Ok(found) = cells_with_navmesh(path, &wanted.keys().copied().collect()) else {
            continue;
        };
        for (raw, cell) in wanted {
            let has = found.contains(&raw);
            let slot = out.entry(cell).or_insert(false);
            *slot = *slot || has;
        }
    }
    out
}

/// The cells among `wanted` (stored FormIDs of the plugin at `path`) whose temporary
/// children hold a `NAVM`: group and record headers only, entering cell children groups
/// only for wanted cells.
fn cells_with_navmesh(path: &Path, wanted: &HashSet<u32>) -> Result<HashSet<u32>, String> {
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
    let mut found = HashSet::new();
    let tes4 = head(0)?;
    // (start, end, group type, label) of the groups to walk, depth first.
    let mut stack: Vec<(u64, u64, i32, u32)> = vec![(
        u64::from(u32::from_le_bytes(field(&tes4, 4))).saturating_add(24),
        len,
        -1,
        0,
    )];
    let mut steps = 0u64;
    while let Some((mut at, end, kind, label)) = stack.pop() {
        while at < end {
            steps = steps.saturating_add(1);
            if steps > 50_000_000 {
                return Err(err(&"too many entries"));
            }
            let buf = head(at)?;
            let sig = field(&buf, 0);
            let size = u64::from(u32::from_le_bytes(field(&buf, 4)));
            if &sig == b"GRUP" {
                let group_end = at.saturating_add(size);
                if size < 24 || group_end > end {
                    return Err(err(&format!("group at {at} has a bad size {size}")));
                }
                let glabel = field(&buf, 8);
                let gkind = i32::from_le_bytes(field(&buf, 12));
                let enter = match gkind {
                    group_type::TOP => glabel == *b"CELL" || glabel == *b"WRLD",
                    group_type::WORLD_CHILDREN
                    | group_type::INTERIOR_BLOCK
                    | group_type::INTERIOR_SUB_BLOCK
                    | group_type::EXTERIOR_BLOCK
                    | group_type::EXTERIOR_SUB_BLOCK => true,
                    group_type::CELL_CHILDREN => wanted.contains(&u32::from_le_bytes(glabel)),
                    group_type::CELL_TEMPORARY => true,
                    _ => false,
                };
                if enter && stack.len() < 16 {
                    stack.push((group_end, end, kind, label));
                    stack.push((
                        at.saturating_add(24),
                        group_end,
                        gkind,
                        u32::from_le_bytes(glabel),
                    ));
                    break;
                }
                at = group_end;
                continue;
            }
            if kind == group_type::CELL_TEMPORARY && &sig == b"NAVM" {
                found.insert(label);
            }
            at = at.saturating_add(size).saturating_add(24);
        }
    }
    Ok(found)
}
