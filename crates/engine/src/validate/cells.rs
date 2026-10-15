//! Cell rules (M4.E4): where cells and their children sit, what references name, and what
//! the `TES4` counters say about them. Every structural fact is lane A's corpus rule
//! (`kitkraft_esm::cells`, `docs/corpus.md` "Cell and world groups"), asserted on every
//! group and record of the 14 official files; the persistence warning is a grade D
//! hypothesis until the game is observed. Rules and sources: `docs/validate.md`.
//!
//! The walk calls [`on_group`] for every group (with the groups above it), [`on_record`]
//! for every record of a cell child type, a `CELL` or anything that names a reference, and
//! [`finish`] once the plugin is read.

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
pub const RULES: [&str; 8] = [
    "cell.missing-parent",
    "ref.orphan",
    "ref.outside-block",
    "ref.persistent-flag",
    "ref.persistence",
    "cell.actor-without-navmesh",
    "cell.incc",
    "navmesh.unchanged",
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
        pass.cells
            .actors
            .entry(cell)
            .or_insert_with(|| id.to_string());
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
    if !pass.on("ref.outside-block") {
        if parent == Some(group_type::EXTERIOR_SUB_BLOCK) {
            if let Some(grid) = sub(subs, b"XCLC").and_then(|s| Grid::from_xclc(s.data())) {
                pass.cells.exterior.insert(form_id, grid);
            }
        }
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

/// The rules that need the whole plugin: `ref.persistence`, `cell.actor-without-navmesh`,
/// `cell.incc`.
pub(super) fn finish(pass: &Pass<'_>, plugin: &Path, findings: &mut Vec<Finding>) {
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
