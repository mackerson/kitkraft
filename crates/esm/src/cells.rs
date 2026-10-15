//! Cell, world and quest group structure (M4.A1) and computed placement paths (M4.A2).
//!
//! Every rule here is a corpus fact: it holds for every group and record of the 14
//! official ESMs (`docs/corpus.md`, "Cell and world groups"; asserted by
//! `crates/esm/tests/cells.rs`). Group type numbers and names follow UESP's Skyrim page
//! (`uesp-skyrim: Skyrim_Mod:Mod_File_Format`, "Groups"), a container source; type 10 is
//! Starfield's and is named here after what the corpus shows it holding (the children of
//! a quest). Nothing here reads a record body except where a function says so.
//!
//! # Where records live
//!
//! | Group type | Parent | Label | Holds (corpus) |
//! | ---------- | ------ | ----- | -------------- |
//! | 0 top | file | record type | records of that type; `CELL` holds type 2, `WRLD` type 1, `QUST` type 10 |
//! | 1 world children | 0 `WRLD` | the `WRLD` FormID | the world's persistent `CELL` and its type 6, then type 4 |
//! | 2 interior block | 0 `CELL` | block number | type 3 |
//! | 3 interior sub-block | 2 | sub-block number | `CELL` records, each followed by its type 6 |
//! | 4 exterior block | 1 | block grid | type 5 |
//! | 5 exterior sub-block | 4 | sub-block grid | `CELL` records, each followed by its type 6 |
//! | 6 cell children | 1, 3, 5 | the `CELL` FormID | types 8 and 9 |
//! | 7 topic children | 10 | the `DIAL` FormID | `INFO` |
//! | 8 cell persistent children | 6 | the `CELL` FormID | `REFR` `ACHR` `PGRE` `PHZD`, record flag 0x400 set |
//! | 9 cell temporary children | 6 | the `CELL` FormID | `NAVM` `REFR` `ACHR` `PGRE` `PHZD`, flag 0x400 clear |
//! | 10 quest children | 0 `QUST` | the `QUST` FormID | `DLBR`, `DIAL` (each followed by its type 7), `SCEN` |
//!
//! A children group (types 1, 6, 7, 10) directly follows the record its label names. A
//! world's persistent references live in its persistent cell (the `CELL` directly under
//! type 1, the only cells with record flag 0x400), whose children group holds type 8 only;
//! exterior cells under type 5 never have a type 8.
//!
//! # Interior blocks
//!
//! An interior cell's block is the last decimal digit of its object ID and its sub-block
//! the digit before it ([`interior_block`]). The object ID is the FormID's low 24 bits for
//! the full class, 12 bits for a stored `FE` FormID and 16 bits for `FD` (the scale classes
//! of `docs/load-order.md`): the corpus confirms the 12-bit reading on the one interior
//! cell of a small file whose FE index is not 0 (`SFBGS047.esm` `FE001016`).
//!
//! # Exterior grid
//!
//! An exterior cell's grid `(x, y)` gives sub-block `(floor(x / 8), floor(y / 8))` and
//! block `(floor(sub_x / 4), floor(sub_y / 4))`. Block and sub-block labels hold `y` in the
//! low 16 bits and `x` in the high 16 bits, each an `i16` ([`grid_label`]). The cell's grid
//! is the first two `i32` of its `XCLC` payload, `x` first: of every (subrecord, byte
//! offset, order) reading of two `i32`, this is the only one that puts all 22,682 exterior
//! cells in their sub-blocks (a search of the corpus, not a definition; no interior cell
//! has an `XCLC`). A cell is [`CELL_UNITS`] wide: 2,037,719 of the 2,038,073 exterior
//! temporary references with a `DATA` (99.98 %) have `DATA` floats 0 and 1 inside their
//! cell's square at that size (the other 5,738 are deleted and have no `DATA`).
//!
//! # Order
//!
//! Inside a group, records of types 3, 5, 7 and 10 (and the top-level `WRLD` and `QUST`
//! groups) ascend by stored FormID, type 10 ordering `DLBR` before `DIAL` before `SCEN`;
//! sibling groups of types 2 to 5 ascend by label read as a `u32`; the persistent cell comes
//! first in type 1. Records in types 8 and 9 follow no order the corpus shows (the writer
//! appends), except that the official build pipeline puts every `NAVM` first in type 9.
//! Where the 12 files of the official pipeline and the two `BlueprintShips` files differ,
//! [`PlacementStyle`] names the choice.

use crate::index::{PluginIndex, RecordLoc};
use crate::model::{GroupHeader, Record, RecordHeader};
use crate::{DecodeLimits, Error, Result, Signature};

/// Group type numbers (UESP, Skyrim_Mod:Mod_File_Format, "Groups"; type 10 from the
/// corpus).
pub mod group_type {
    /// Top-level group, labelled with a record type.
    pub const TOP: i32 = 0;
    /// Children of a `WRLD` record.
    pub const WORLD_CHILDREN: i32 = 1;
    /// Interior cell block.
    pub const INTERIOR_BLOCK: i32 = 2;
    /// Interior cell sub-block.
    pub const INTERIOR_SUB_BLOCK: i32 = 3;
    /// Exterior cell block.
    pub const EXTERIOR_BLOCK: i32 = 4;
    /// Exterior cell sub-block.
    pub const EXTERIOR_SUB_BLOCK: i32 = 5;
    /// Children of a `CELL` record.
    pub const CELL_CHILDREN: i32 = 6;
    /// Children of a `DIAL` record.
    pub const TOPIC_CHILDREN: i32 = 7;
    /// Persistent children of a cell.
    pub const CELL_PERSISTENT: i32 = 8;
    /// Temporary children of a cell.
    pub const CELL_TEMPORARY: i32 = 9;
    /// Children of a `QUST` record (Starfield; no name on a permitted source).
    pub const QUEST_CHILDREN: i32 = 10;
}

/// Record flag 0x400: set on every record in a type 8 group and on every world's
/// persistent cell, clear on every record in a type 9 group and on every other cell
/// (corpus, all 14 files). UESP's Skyrim page calls it "Persistent".
pub const FLAG_PERSISTENT: u32 = 0x0000_0400;

/// Width of an exterior cell in position units (corpus: see the module documentation).
pub const CELL_UNITS: f32 = 100.0;

/// Cells per sub-block side, and sub-blocks per block side.
pub const SUB_BLOCK_CELLS: i32 = 8;
/// Sub-blocks per block side.
pub const BLOCK_SUB_BLOCKS: i32 = 4;

const CELL: Signature = Signature::new(b"CELL");
const WRLD: Signature = Signature::new(b"WRLD");
const QUST: Signature = Signature::new(b"QUST");
const DIAL: Signature = Signature::new(b"DIAL");
const DLBR: Signature = Signature::new(b"DLBR");
const SCEN: Signature = Signature::new(b"SCEN");
const INFO: Signature = Signature::new(b"INFO");
const NAVM: Signature = Signature::new(b"NAVM");
const XCLC: Signature = Signature::new(b"XCLC");

/// Record types the corpus has only inside cell children groups (types 8, 9).
pub const CELL_CHILD_TYPES: [Signature; 5] = [
    Signature::new(b"REFR"),
    Signature::new(b"ACHR"),
    Signature::new(b"PGRE"),
    Signature::new(b"PHZD"),
    NAVM,
];

/// Record types the corpus has only inside quest children groups (type 10).
pub const QUEST_CHILD_TYPES: [Signature; 3] = [DLBR, DIAL, SCEN];

/// Record types that live only in nested groups but still have an empty top-level group
/// in the files of the official pipeline that contain them (see [`PlacementStyle`]).
pub const PLACEHOLDER_TYPES: [Signature; 5] = [NAVM, DIAL, INFO, DLBR, SCEN];

/// An exterior cell's grid coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Grid {
    pub x: i32,
    pub y: i32,
}

impl Grid {
    pub const fn new(x: i32, y: i32) -> Grid {
        Grid { x, y }
    }

    /// The sub-block holding this cell: each coordinate divided by
    /// [`SUB_BLOCK_CELLS`], rounded down.
    pub fn sub_block(self) -> Grid {
        Grid {
            x: self.x.div_euclid(SUB_BLOCK_CELLS),
            y: self.y.div_euclid(SUB_BLOCK_CELLS),
        }
    }

    /// The block holding this cell: the sub-block divided by [`BLOCK_SUB_BLOCKS`],
    /// rounded down.
    pub fn block(self) -> Grid {
        let sub = self.sub_block();
        Grid {
            x: sub.x.div_euclid(BLOCK_SUB_BLOCKS),
            y: sub.y.div_euclid(BLOCK_SUB_BLOCKS),
        }
    }

    /// The cell holding position `(x, y)`: each coordinate divided by [`CELL_UNITS`],
    /// rounded down. `None` for a non-finite position or one whose cell does not fit an
    /// `i16` (no group label could name it).
    pub fn of_position(x: f32, y: f32) -> Option<Grid> {
        let cell = |v: f32| -> Option<i32> {
            if !v.is_finite() {
                return None;
            }
            let c = (f64::from(v) / f64::from(CELL_UNITS)).floor();
            if c < f64::from(i16::MIN) || c > f64::from(i16::MAX) {
                return None;
            }
            // In i16 range, checked just above.
            #[allow(clippy::cast_possible_truncation)]
            Some(c as i32)
        };
        Some(Grid {
            x: cell(x)?,
            y: cell(y)?,
        })
    }

    /// The grid an `XCLC` payload holds: its first two `i32`, `x` first (corpus: see the
    /// module documentation). `None` when the payload is shorter than 8 bytes.
    pub fn from_xclc(payload: &[u8]) -> Option<Grid> {
        let x = payload.get(0..4)?.try_into().ok().map(i32::from_le_bytes)?;
        let y = payload.get(4..8)?.try_into().ok().map(i32::from_le_bytes)?;
        Some(Grid { x, y })
    }
}

/// The label of an exterior block or sub-block group at `grid` (its block or sub-block
/// coordinates): `y` in the low 16 bits, `x` in the high 16 bits, each an `i16`.
pub fn grid_label(grid: Grid) -> Result<[u8; 4]> {
    let x = i16::try_from(grid.x).map_err(|_| out_of_range("grid x does not fit an i16"))?;
    let y = i16::try_from(grid.y).map_err(|_| out_of_range("grid y does not fit an i16"))?;
    let [y0, y1] = y.to_le_bytes();
    let [x0, x1] = x.to_le_bytes();
    Ok([y0, y1, x0, x1])
}

/// The grid an exterior block or sub-block label names (the inverse of [`grid_label`]).
pub fn label_grid(label: [u8; 4]) -> Grid {
    let [y0, y1, x0, x1] = label;
    Grid {
        x: i32::from(i16::from_le_bytes([x0, x1])),
        y: i32::from(i16::from_le_bytes([y0, y1])),
    }
}

fn out_of_range(reason: &'static str) -> Error {
    Error::Placement { offset: 0, reason }
}

/// The object ID part of a stored FormID, by its scale class (`docs/load-order.md`): the
/// low 12 bits for a stored `FE` FormID (small), 16 bits for `FD` (medium), else 24.
pub fn object_id(form_id: u32) -> u32 {
    match form_id >> 24 {
        0xFE => form_id & 0x0000_0FFF,
        0xFD => form_id & 0x0000_FFFF,
        _ => form_id & 0x00FF_FFFF,
    }
}

/// The interior block and sub-block of the cell with stored FormID `form_id`: the last
/// decimal digit of its object ID ([`object_id`]) and the digit before it.
pub fn interior_block(form_id: u32) -> (u32, u32) {
    let id = object_id(form_id);
    let block = id.checked_rem(10).unwrap_or(0);
    let sub = id
        .checked_div(10)
        .and_then(|d| d.checked_rem(10))
        .unwrap_or(0);
    (block, sub)
}

/// The interior block and sub-block labels (each read as a `u32`) of the group path that
/// holds an interior `CELL`, outermost first: `Some` when the path has both a type 2 and a
/// type 3 group.
pub fn interior_labels<'h>(path: impl IntoIterator<Item = &'h GroupHeader>) -> Option<(u32, u32)> {
    let mut block = None;
    let mut sub = None;
    for g in path {
        match g.group_type {
            group_type::INTERIOR_BLOCK => block = Some(g.label_u32()),
            group_type::INTERIOR_SUB_BLOCK => sub = Some(g.label_u32()),
            _ => {}
        }
    }
    Some((block?, sub?))
}

/// Where a cell sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellPlace {
    /// An interior cell: under the top-level `CELL` group, in the block and sub-block
    /// `labels` names (`(block, sub_block)`, each label read as a `u32`) when the cell
    /// exists in a plugin ([`cell_place_in`] reads them from the path that holds it, so a
    /// cell some other writer put elsewhere is still found), or by [`interior_block`] for a
    /// cell being created (`labels: None`, [`CellPlace::INTERIOR`]).
    Interior { labels: Option<(u32, u32)> },
    /// The persistent cell of world `world` (stored FormID): directly in its world
    /// children group. It holds the world's persistent references.
    Persistent { world: u32 },
    /// An exterior cell of world `world` at `grid`.
    Exterior { world: u32, grid: Grid },
}

impl CellPlace {
    /// A new interior cell: its block and sub-block computed by [`interior_block`].
    pub const INTERIOR: CellPlace = CellPlace::Interior { labels: None };

    /// The world, for an exterior or persistent cell.
    pub fn world(&self) -> Option<u32> {
        match self {
            CellPlace::Interior { .. } => None,
            CellPlace::Persistent { world } | CellPlace::Exterior { world, .. } => Some(*world),
        }
    }
}

/// Which children group of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Children {
    /// Type 8.
    Persistent,
    /// Type 9.
    Temporary,
}

/// The order of a cell's two children groups when it has both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChildOrder {
    /// Type 9 before type 8: every such cell of the 12 files of the official pipeline
    /// (2,364 cells).
    TemporaryFirst,
    /// Type 8 before type 9: every such cell of the two `BlueprintShips` files (815).
    PersistentFirst,
}

/// The three header fields of a group that carry no structure, as written for a new group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GroupStamps {
    pub timestamp: u16,
    pub version_control: u16,
    pub unknown: u32,
}

/// How groups are created and ordered where the corpus shows two writers doing it
/// differently: the 12 files of the official pipeline ([`PlacementStyle::OFFICIAL`]) and
/// the two `BlueprintShips` files ([`PlacementStyle::blueprint`]). What the Creation Kit
/// does for a new plugin is F20's question (M4.O1); until then KitCraft writes the
/// official style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlacementStyle {
    /// Order of a cell's persistent and temporary children groups.
    pub cell_children: ChildOrder,
    /// `NAVM` records first in a temporary children group (official: every one of 3,017
    /// groups with a `NAVM`; `BlueprintShips`: appended after the references).
    pub navmesh_first: bool,
    /// An empty top-level group for each of [`PLACEHOLDER_TYPES`] that the file has
    /// records of, in [`crate::build::TOP_GROUP_ORDER`] position (official pipeline only).
    pub placeholder_groups: bool,
    /// Header fields of a new group, by group type (index 0..=10; other types use zero).
    pub stamps: [GroupStamps; 11],
}

impl Default for PlacementStyle {
    fn default() -> PlacementStyle {
        PlacementStyle::OFFICIAL
    }
}

impl PlacementStyle {
    /// The 12 files of the official build pipeline: every group stamp 0.
    pub const OFFICIAL: PlacementStyle = PlacementStyle {
        cell_children: ChildOrder::TemporaryFirst,
        navmesh_first: true,
        placeholder_groups: true,
        stamps: [GroupStamps {
            timestamp: 0,
            version_control: 0,
            unknown: 0,
        }; 11],
    };

    /// The two `BlueprintShips` files: one timestamp per file (13505 and 13501), and the
    /// unknown field 0 on top-level groups, 3 on blocks, sub-blocks and cell children, 1 on
    /// persistent and temporary children (the majority value per type; `docs/corpus.md`
    /// lists the groups that differ).
    pub fn blueprint(timestamp: u16) -> PlacementStyle {
        let stamp = |unknown| GroupStamps {
            timestamp,
            version_control: 0,
            unknown,
        };
        let mut stamps = [stamp(0); 11];
        for t in [2usize, 3, 4, 5, 6] {
            if let Some(s) = stamps.get_mut(t) {
                *s = stamp(3);
            }
        }
        for t in [8usize, 9] {
            if let Some(s) = stamps.get_mut(t) {
                *s = stamp(1);
            }
        }
        PlacementStyle {
            cell_children: ChildOrder::PersistentFirst,
            navmesh_first: false,
            placeholder_groups: false,
            stamps,
        }
    }

    /// The stamps for a new group of `group_type`.
    pub fn stamps_for(&self, group_type: i32) -> GroupStamps {
        usize::try_from(group_type)
            .ok()
            .and_then(|t| self.stamps.get(t))
            .copied()
            .unwrap_or_default()
    }

    /// The style the writer of `index`'s plugin used, so an edit keeps it: from the first
    /// cell children group that holds both a persistent and a temporary group (temporary
    /// first: [`PlacementStyle::OFFICIAL`]; persistent first:
    /// [`PlacementStyle::blueprint`] with the first top-level group's timestamp). Without
    /// such a cell, a first top-level group with a timestamp other than 0 means
    /// `blueprint` (the official pipeline stamps every group 0), anything else `OFFICIAL`
    /// (which is also what a new plugin gets until F20 is observed).
    pub fn detect(index: &PluginIndex) -> PlacementStyle {
        let stamp = index
            .groups
            .iter()
            .find(|g| g.header.group_type == group_type::TOP)
            .map_or(0, |g| g.header.timestamp);
        let mut first: std::collections::HashMap<u32, i32> = std::collections::HashMap::new();
        for g in &index.groups {
            let t = g.header.group_type;
            if t != group_type::CELL_PERSISTENT && t != group_type::CELL_TEMPORARY {
                continue;
            }
            let Some(parent) = g.parent else { continue };
            match first.get(&parent) {
                None => {
                    first.insert(parent, t);
                }
                Some(seen) if *seen != t => {
                    return if *seen == group_type::CELL_TEMPORARY {
                        PlacementStyle::OFFICIAL
                    } else {
                        PlacementStyle::blueprint(stamp)
                    };
                }
                Some(_) => {}
            }
        }
        if stamp != 0 {
            PlacementStyle::blueprint(stamp)
        } else {
            PlacementStyle::OFFICIAL
        }
    }

    /// A group header of `group_type` with `label` and this style's stamps.
    pub fn header(&self, label: [u8; 4], group_type: i32) -> GroupHeader {
        let s = self.stamps_for(group_type);
        GroupHeader {
            label,
            group_type,
            timestamp: s.timestamp,
            version_control: s.version_control,
            unknown: s.unknown,
        }
    }
}

/// What a record hangs under, for [`placement`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Parent {
    /// Nothing: a record of a type that lives in its top-level group (`WRLD` and `QUST`
    /// included).
    None,
    /// The record is a `CELL`, placed as given.
    CellAt(CellPlace),
    /// A child of cell `cell` (stored FormID) placed as given: `REFR`, `ACHR`, `PGRE`,
    /// `PHZD` (persistent by record flag 0x400) or `NAVM` (always temporary).
    Cell { place: CellPlace, cell: u32 },
    /// A child of quest `quest`: `DLBR`, `DIAL`, `SCEN`.
    Quest { quest: u32 },
    /// An `INFO` of topic `topic`, itself a child of quest `quest`.
    Topic { quest: u32, topic: u32 },
}

fn refused(reason: &'static str) -> Error {
    Error::Placement { offset: 0, reason }
}

/// The groups that hold cell `cell` (stored FormID) at `place`, outermost first.
pub fn cell_path(place: &CellPlace, cell: u32, style: &PlacementStyle) -> Result<Vec<GroupHeader>> {
    use group_type::*;
    Ok(match *place {
        CellPlace::Interior { labels } => {
            let (block, sub) = labels.unwrap_or_else(|| interior_block(cell));
            vec![
                style.header(CELL.0, TOP),
                style.header(block.to_le_bytes(), INTERIOR_BLOCK),
                style.header(sub.to_le_bytes(), INTERIOR_SUB_BLOCK),
            ]
        }
        CellPlace::Persistent { world } => vec![
            style.header(WRLD.0, TOP),
            style.header(world.to_le_bytes(), WORLD_CHILDREN),
        ],
        CellPlace::Exterior { world, grid } => vec![
            style.header(WRLD.0, TOP),
            style.header(world.to_le_bytes(), WORLD_CHILDREN),
            style.header(grid_label(grid.block())?, EXTERIOR_BLOCK),
            style.header(grid_label(grid.sub_block())?, EXTERIOR_SUB_BLOCK),
        ],
    })
}

/// The groups that hold the `which` children of cell `cell` at `place`, outermost first.
/// A world's persistent cell has persistent children only, and an exterior cell temporary
/// children only (corpus); the other two combinations are refused: a persistent reference
/// of a world goes into its persistent cell.
pub fn cell_children_path(
    place: &CellPlace,
    cell: u32,
    which: Children,
    style: &PlacementStyle,
) -> Result<Vec<GroupHeader>> {
    match (place, which) {
        (CellPlace::Persistent { .. }, Children::Temporary) => {
            return Err(refused(
                "a world's persistent cell has no temporary children in the corpus",
            ))
        }
        (CellPlace::Exterior { .. }, Children::Persistent) => {
            return Err(refused(
                "exterior cells have no persistent children in the corpus: a world's \
                 persistent references live in its persistent cell",
            ))
        }
        _ => {}
    }
    let mut path = cell_path(place, cell, style)?;
    let label = cell.to_le_bytes();
    path.push(style.header(label, group_type::CELL_CHILDREN));
    path.push(style.header(
        label,
        match which {
            Children::Persistent => group_type::CELL_PERSISTENT,
            Children::Temporary => group_type::CELL_TEMPORARY,
        },
    ));
    Ok(path)
}

/// The groups the record with `header` (its signature, flags and stored FormID) goes into
/// under `parent`, outermost first: the computed placement path for
/// [`crate::Plugin::upsert`]. Refused (`Error::Placement`) where the corpus never shows the
/// combination: a cell child type outside a cell, a `NAVM` with flag 0x400, a `CELL` whose
/// flag 0x400 disagrees with its place, and so on.
pub fn placement(
    header: &RecordHeader,
    parent: &Parent,
    style: &PlacementStyle,
) -> Result<Vec<GroupHeader>> {
    use group_type::*;
    let signature = header.signature;
    let persistent = header.flags & FLAG_PERSISTENT != 0;
    let nested_only = CELL_CHILD_TYPES.contains(&signature)
        || QUEST_CHILD_TYPES.contains(&signature)
        || signature == INFO
        || signature == CELL;
    match *parent {
        Parent::None => {
            if nested_only {
                return Err(refused(
                    "this record type lives only in nested groups; name its parent",
                ));
            }
            Ok(vec![style.header(signature.0, TOP)])
        }
        Parent::CellAt(place) => {
            if signature != CELL {
                return Err(refused("only a CELL is placed as a cell"));
            }
            if persistent != matches!(place, CellPlace::Persistent { .. }) {
                return Err(refused(
                    "a CELL carries record flag 0x400 exactly when it is a world's \
                     persistent cell",
                ));
            }
            cell_path(&place, header.form_id, style)
        }
        Parent::Cell { place, cell } => {
            if !CELL_CHILD_TYPES.contains(&signature) {
                return Err(refused(
                    "cell children are REFR, ACHR, PGRE, PHZD and NAVM in the corpus",
                ));
            }
            if signature == NAVM && persistent {
                return Err(refused("no NAVM carries record flag 0x400 in the corpus"));
            }
            let which = if persistent {
                Children::Persistent
            } else {
                Children::Temporary
            };
            cell_children_path(&place, cell, which, style)
        }
        Parent::Quest { quest } => {
            if !QUEST_CHILD_TYPES.contains(&signature) {
                return Err(refused(
                    "quest children are DLBR, DIAL and SCEN in the corpus",
                ));
            }
            Ok(vec![
                style.header(QUST.0, TOP),
                style.header(quest.to_le_bytes(), QUEST_CHILDREN),
            ])
        }
        Parent::Topic { quest, topic } => {
            if signature != INFO {
                return Err(refused("topic children are INFO in the corpus"));
            }
            Ok(vec![
                style.header(QUST.0, TOP),
                style.header(quest.to_le_bytes(), QUEST_CHILDREN),
                style.header(topic.to_le_bytes(), TOPIC_CHILDREN),
            ])
        }
    }
}

/// Group types whose overridden records the official files list in `TES4` `ONAM`: topic
/// children (7), cell temporary children (9) and quest children (10). Overrides in
/// persistent children (8), cells and top-level records are never listed (M4.A6,
/// `tests/onam.rs`, every record of the 14 files).
pub const ONAM_GROUP_TYPES: [i32; 3] = [
    group_type::TOPIC_CHILDREN,
    group_type::CELL_TEMPORARY,
    group_type::QUEST_CHILDREN,
];

/// The FormIDs `plugin`'s `ONAM` lists by the corpus rule (M4.A6): every record in a group
/// of [`ONAM_GROUP_TYPES`] for which `overrides_master_record` is true (the caller decides,
/// from the load order, that the FormID is a master's and that the master has that
/// record: the official files list no override of a record its master lacks), in file
/// order. The official files' own order is not file order or FormID order (it looks
/// unordered), so the order written is a choice until the Creation Kit is observed (F23).
/// Write the list with [`crate::tes4::Tes4::set_overridden_forms`].
pub fn onam_forms(
    plugin: &crate::Plugin<'_>,
    mut overrides_master_record: impl FnMut(u32) -> bool,
) -> Vec<u32> {
    let mut out = Vec::new();
    walk_records(plugin.entries(), |path, r| {
        let listed = path
            .last()
            .is_some_and(|g| ONAM_GROUP_TYPES.contains(&g.group_type));
        if listed && overrides_master_record(r.header.form_id) {
            out.push(r.header.form_id);
        }
    });
    out
}

/// [`onam_forms`] for a plugin known by its index (a patch streamed from disk): the same
/// records, from the group each record of `index` sits in, in file order.
pub fn onam_forms_in(
    index: &PluginIndex,
    mut overrides_master_record: impl FnMut(u32) -> bool,
) -> Vec<u32> {
    index
        .records
        .iter()
        .filter(|r| {
            r.group
                .and_then(|g| index.groups.get(usize::try_from(g).ok()?))
                .is_some_and(|g| ONAM_GROUP_TYPES.contains(&g.header.group_type))
        })
        .map(|r| r.form_id)
        .filter(|id| overrides_master_record(*id))
        .collect()
}

/// Call `f` with every record of `entries` and the headers of the groups around it,
/// outermost first, in file order. Iterative: the depth is the tree's, with no recursion.
pub(crate) fn walk_records<'p, 'a>(
    entries: &'p [crate::Entry<'a>],
    mut f: impl FnMut(&[GroupHeader], &'p Record<'a>),
) {
    let mut path: Vec<GroupHeader> = Vec::new();
    let mut stack: Vec<std::slice::Iter<'p, crate::Entry<'a>>> = vec![entries.iter()];
    while let Some(level) = stack.last_mut() {
        match level.next() {
            Some(crate::Entry::Group(g)) => {
                path.push(g.header);
                stack.push(g.children().iter());
            }
            Some(crate::Entry::Record(r)) => f(&path, r),
            None => {
                stack.pop();
                path.pop();
            }
        }
    }
}

/// The persistent cell of world `world` (stored FormID) in `index`: the `CELL` directly in
/// the world's children group (corpus: at most one, always first, flag 0x400). A
/// persistent reference placed in that world goes into its persistent children.
pub fn persistent_cell(index: &PluginIndex, world: u32) -> Option<&RecordLoc> {
    index.records.iter().find(|r| {
        r.signature == CELL
            && r.group
                .and_then(|g| index.groups.get(usize::try_from(g).ok()?))
                .is_some_and(|g| {
                    g.header.group_type == group_type::WORLD_CHILDREN
                        && g.header.label_u32() == world
                })
    })
}

/// Where cell `loc` of `index` sits, from its group path and, for an exterior cell, the
/// grid in its `XCLC` (`record` is the cell read back, for example with
/// [`crate::index::read_indexed`]; a compressed one is decoded under `limits`). An
/// exterior cell without an `XCLC` is refused: every exterior cell of the corpus has one.
pub fn cell_place_in(
    index: &PluginIndex,
    loc: &RecordLoc,
    record: &Record<'_>,
    limits: &DecodeLimits,
) -> Result<CellPlace> {
    if loc.signature != CELL {
        return Err(refused("the record is not a CELL"));
    }
    let path = index.group_path(loc.group);
    let world = path
        .iter()
        .find(|g| g.header.group_type == group_type::WORLD_CHILDREN)
        .map(|g| g.header.label_u32());
    let Some(world) = world else {
        return match path.first() {
            Some(top) if top.header.label == CELL.0 => Ok(CellPlace::Interior {
                labels: interior_labels(path.iter().map(|g| &g.header)),
            }),
            _ => Err(refused("the CELL is not in a cell group")),
        };
    };
    if path
        .last()
        .is_some_and(|g| g.header.group_type == group_type::WORLD_CHILDREN)
    {
        return Ok(CellPlace::Persistent { world });
    }
    let decoded;
    let subs = match record.subrecords() {
        Some(subs) => subs,
        None => {
            decoded = record.decode(limits)?.unwrap_or_default();
            &decoded
        }
    };
    let grid = subs
        .iter()
        .find(|s| s.signature() == XCLC)
        .and_then(|s| Grid::from_xclc(s.data()))
        .ok_or_else(|| refused("an exterior cell without an XCLC grid"))?;
    Ok(CellPlace::Exterior { world, grid })
}

/// The records a plugin must hold before a child of cell `cell` (stored FormID) at `place`
/// can be placed in it ([`crate::Plugin::upsert_styled`] refuses a cell children group
/// whose `CELL` is not there, and a world children group whose `WRLD` is not): for an
/// exterior cell or a world's persistent cell the `WRLD`, then the `CELL`, each read from
/// `index` (the plugin that has them, usually the master) with `read` (for example
/// [`crate::index::read_indexed`] without decoding) and placed where `style` puts it, an
/// interior cell in the block and sub-block `index` has it in. Upsert them first, and
/// skip any the target already has (its override may carry edits).
///
/// Lossless: each record is the stored record (its bytes as read, a compressed body not
/// recompressed, FormIDs as stored). Stored FormIDs are file-relative, so the copies mean
/// the same records only in a target whose masters begin with `index`'s plugin's masters,
/// in order, followed by that plugin (a patch of it); remapping is not done here.
///
/// Refused (`Error::Placement`): the cell or its world is not in `index`, or `index` has
/// the cell at another place than `place` (interior block labels aside).
pub fn carry_parents(
    index: &PluginIndex,
    place: &CellPlace,
    cell: u32,
    style: &PlacementStyle,
    limits: &DecodeLimits,
    mut read: impl FnMut(&RecordLoc) -> Result<Record<'static>>,
) -> Result<Vec<crate::build::Placed>> {
    use crate::build::Placed;
    let loc = index
        .records
        .iter()
        .find(|r| r.signature == CELL && r.form_id == cell)
        .ok_or_else(|| refused("the cell to carry is not in the index"))?;
    let record = read(loc)?;
    let observed = cell_place_in(index, loc, &record, limits)?;
    let agrees = match (place, &observed) {
        (CellPlace::Interior { .. }, CellPlace::Interior { .. }) => true,
        (a, b) => a == b,
    };
    if !agrees {
        return Err(refused("the index has the cell to carry at another place"));
    }
    let mut out = Vec::with_capacity(2);
    if let Some(world) = observed.world() {
        let wloc = index
            .records
            .iter()
            .find(|r| r.signature == WRLD && r.form_id == world)
            .ok_or_else(|| refused("the world of the cell to carry is not in the index"))?;
        out.push(Placed {
            path: vec![style.header(WRLD.0, group_type::TOP)],
            record: read(wloc)?,
        });
    }
    out.push(Placed {
        path: cell_path(&observed, cell, style)?,
        record,
    });
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::fixture::{self, group, record, sub};
    use crate::index::{index_bytes, read_indexed, IndexOptions};

    fn header(sig: &[u8; 4], flags: u32, form_id: u32) -> RecordHeader {
        RecordHeader {
            signature: Signature::new(sig),
            flags,
            form_id,
            ..RecordHeader::default()
        }
    }

    fn shape(path: &[GroupHeader]) -> Vec<(i32, u32)> {
        path.iter().map(|g| (g.group_type, g.label_u32())).collect()
    }

    #[test]
    fn grid_math_rounds_down_and_labels_put_y_low() {
        let g = Grid::new(-1, 8);
        assert_eq!(g.sub_block(), Grid::new(-1, 1));
        assert_eq!(g.block(), Grid::new(-1, 0));
        assert_eq!(Grid::new(-33, 31).block(), Grid::new(-2, 0));
        assert_eq!(Grid::new(0, -8).sub_block(), Grid::new(0, -1));
        let label = grid_label(Grid::new(2, -1)).unwrap();
        assert_eq!(label, [0xFF, 0xFF, 0x02, 0x00]);
        assert_eq!(label_grid(label), Grid::new(2, -1));
        assert!(grid_label(Grid::new(40_000, 0)).is_err());
        assert_eq!(Grid::of_position(-0.5, 199.9), Some(Grid::new(-1, 1)));
        assert_eq!(Grid::of_position(f32::NAN, 0.0), None);
        assert_eq!(Grid::of_position(1e9, 0.0), None);
        let xclc = [3i32.to_le_bytes(), (-4i32).to_le_bytes()].concat();
        assert_eq!(Grid::from_xclc(&xclc), Some(Grid::new(3, -4)));
        assert_eq!(Grid::from_xclc(&xclc[..7]), None);
    }

    #[test]
    fn interior_blocks_use_the_object_id_of_the_scale_class() {
        assert_eq!(interior_block(0x0000_4D2B), (5, 5)); // 19755
        assert_eq!(interior_block(0x0100_4D2B), (5, 5)); // master byte ignored
        assert_eq!(interior_block(0xFE00_1016), (2, 2)); // 0x016 = 22 (SFBGS047)
        assert_eq!(interior_block(0xFD00_0123), (1, 9)); // 0x0123 = 291
        assert_eq!(object_id(0xFE12_3456), 0x456);
        assert_eq!(object_id(0xFD12_3456), 0x3456);
    }

    #[test]
    fn paths_follow_the_corpus_rules() {
        let style = PlacementStyle::OFFICIAL;
        let cell = header(b"CELL", 0, 0x0001_2345);
        let p = placement(&cell, &Parent::CellAt(CellPlace::INTERIOR), &style).unwrap();
        // 0x12345 = 74565: block 5, sub-block 6.
        let cell_top = u32::from_le_bytes(*b"CELL");
        assert_eq!(shape(&p), [(0, cell_top), (2, 5), (3, 6)]);
        let world = 0x0000_0025;
        let place = CellPlace::Exterior {
            world,
            grid: Grid::new(-1, 9),
        };
        let p = placement(&cell, &Parent::CellAt(place), &style).unwrap();
        let wrld = u32::from_le_bytes(*b"WRLD");
        assert_eq!(
            shape(&p),
            [(0, wrld), (1, world), (4, 0xFFFF_0000), (5, 0xFFFF_0001)]
        );
        let child = |place, cell| Parent::Cell { place, cell };
        let refr = header(b"REFR", 0, 0x0001_0001);
        let p = placement(&refr, &child(place, 0x0001_2345), &style).unwrap();
        assert_eq!(&shape(&p)[4..], [(6, 0x0001_2345), (9, 0x0001_2345)]);
        // Persistent references go into the world's persistent cell.
        let pers = header(b"REFR", FLAG_PERSISTENT, 0x0001_0002);
        assert!(placement(&pers, &child(place, 0x0001_2345), &style).is_err());
        let wcell = CellPlace::Persistent { world };
        let p = placement(&pers, &child(wcell, 0x0D62), &style).unwrap();
        assert_eq!(shape(&p), [(0, wrld), (1, world), (6, 0x0D62), (8, 0x0D62)]);
        assert!(placement(&refr, &child(wcell, 0x0D62), &style).is_err());
        // Quest and topic children.
        let dial = header(b"DIAL", 0, 0x0001_0003);
        let p = placement(&dial, &Parent::Quest { quest: 0x77 }, &style).unwrap();
        assert_eq!(shape(&p), [(0, u32::from_le_bytes(*b"QUST")), (10, 0x77)]);
        let info = header(b"INFO", 0, 0x0001_0004);
        let topic = Parent::Topic {
            quest: 0x77,
            topic: 0x0001_0003,
        };
        let p = placement(&info, &topic, &style).unwrap();
        let last = p.last().map(|g| (g.group_type, g.label_u32()));
        assert_eq!(last, Some((7, 0x0001_0003)));
        // Refusals.
        let top = Parent::None;
        assert!(placement(&refr, &top, &style).is_err());
        assert!(placement(&cell, &top, &style).is_err());
        assert!(placement(&info, &Parent::Quest { quest: 1 }, &style).is_err());
        let navm = header(b"NAVM", FLAG_PERSISTENT, 1);
        assert!(placement(&navm, &child(wcell, 1), &style).is_err());
        let pcell = header(b"CELL", FLAG_PERSISTENT, 1);
        assert!(placement(&pcell, &Parent::CellAt(CellPlace::INTERIOR), &style).is_err());
        assert!(placement(&cell, &Parent::CellAt(wcell), &style).is_err());
        assert!(placement(&pcell, &Parent::CellAt(wcell), &style).is_ok());
        let weap = header(b"WEAP", 0, 1);
        let p = placement(&weap, &top, &style).unwrap();
        assert_eq!(shape(&p), [(0, u32::from_le_bytes(*b"WEAP"))]);
        let far = CellPlace::Exterior {
            world,
            grid: Grid::new(1 << 20, 0),
        };
        assert!(placement(&cell, &Parent::CellAt(far), &style).is_err());
    }

    #[test]
    fn styles_stamp_new_groups() {
        let b = PlacementStyle::blueprint(13505);
        assert_eq!(b.header(*b"CELL", 0).unknown, 0);
        assert_eq!(b.header([0; 4], 3).unknown, 3);
        assert_eq!(b.header([0; 4], 9).unknown, 1);
        assert_eq!(b.header([0; 4], 9).timestamp, 13505);
        assert_eq!(b.stamps_for(-1), GroupStamps::default());
        assert_eq!(PlacementStyle::default(), PlacementStyle::OFFICIAL);
        assert_eq!(PlacementStyle::OFFICIAL.header([1; 4], 6).timestamp, 0);
    }

    #[test]
    fn cell_places_come_from_the_index_and_xclc() {
        let world = 0x0000_0700u32;
        let pcell = record(b"CELL", FLAG_PERSISTENT, 0x701, &sub(b"XCLC", &[0u8; 12]));
        let xclc = [5i32.to_le_bytes(), (-3i32).to_le_bytes(), [0u8; 4]].concat();
        let ecell = record(b"CELL", 0, 0x702, &sub(b"XCLC", &xclc));
        let label = grid_label(Grid::new(0, -1)).unwrap();
        let sub_block = group(&label, 5, &ecell);
        let block = group(&label, 4, &sub_block);
        let children = group(&world.to_le_bytes(), 1, &[pcell, block].concat());
        let wrld = [record(b"WRLD", 0, world, &sub(b"EDID", b"w\0")), children].concat();
        let mut bytes = fixture::sample_plugin();
        bytes.extend(group(b"WRLD", 0, &wrld));
        let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
        let limits = DecodeLimits::default();
        let mut file = std::io::Cursor::new(&bytes);
        let mut place = |form_id: u32| {
            let loc = index.records.iter().find(|r| r.form_id == form_id).unwrap();
            let rec = read_indexed(&mut file, loc, None).unwrap();
            cell_place_in(&index, loc, &rec, &limits)
        };
        assert_eq!(place(0x200).unwrap(), CellPlace::INTERIOR);
        assert_eq!(place(0x701).unwrap(), CellPlace::Persistent { world });
        let grid = Grid::new(5, -3);
        assert_eq!(place(0x702).unwrap(), CellPlace::Exterior { world, grid });
        assert!(place(0x100).is_err()); // a GMST
        let found = persistent_cell(&index, world).map(|r| r.form_id);
        assert_eq!(found, Some(0x701));
        assert!(persistent_cell(&index, 0x999).is_none());
    }

    /// An existing interior cell keeps the block and sub-block it was found in, even where
    /// the corpus rule would compute others (a third-party writer's choice); only a cell
    /// being created gets computed ones.
    #[test]
    fn existing_interior_cells_keep_their_observed_blocks() {
        // 0x705 = 1797: computed block 7, sub-block 9; the file has it in 3 / 4.
        let cell = record(b"CELL", 0, 0x705, &sub(b"EDID", b"c\0"));
        let sub_block = group(&4u32.to_le_bytes(), 3, &cell);
        let block = group(&3u32.to_le_bytes(), 2, &sub_block);
        let bytes = fixture::plugin(&[group(b"CELL", 0, &block)]);
        let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
        let loc = index.records.iter().find(|r| r.form_id == 0x705).unwrap();
        let rec = read_indexed(&mut std::io::Cursor::new(&bytes), loc, None).unwrap();
        let place = cell_place_in(&index, loc, &rec, &DecodeLimits::default()).unwrap();
        assert_eq!(
            place,
            CellPlace::Interior {
                labels: Some((3, 4))
            }
        );
        let style = PlacementStyle::OFFICIAL;
        let p = cell_children_path(&place, 0x705, Children::Temporary, &style).unwrap();
        let cell_top = u32::from_le_bytes(*b"CELL");
        assert_eq!(
            shape(&p),
            [(0, cell_top), (2, 3), (3, 4), (6, 0x705), (9, 0x705)]
        );
        let new = cell_children_path(&CellPlace::INTERIOR, 0x705, Children::Temporary, &style);
        assert_eq!(shape(&new.unwrap())[1..3], [(2, 7), (3, 9)]);
        assert_eq!(
            interior_labels(index.group_path(loc.group).iter().map(|g| &g.header)),
            Some((3, 4))
        );
    }

    /// Review nits: the style is read back from a plugin, and ONAM is found from an index
    /// as from the tree.
    #[test]
    fn style_detection_and_onam_from_an_index() {
        // The sample has a cell with type 8 before type 9: the BlueprintShips order.
        let sample = fixture::sample_plugin();
        let index = index_bytes(&sample, &IndexOptions::default()).unwrap();
        let stamp = index.groups[0].header.timestamp;
        assert_eq!(
            PlacementStyle::detect(&index),
            PlacementStyle::blueprint(stamp)
        );
        // Type 9 first: official. No cell with both and a stamp-free first group: official;
        // a stamped one: blueprint with that timestamp.
        let refr = |id: u32, flags: u32| record(b"REFR", flags, id, &sub(b"NAME", &[0; 4]));
        let label = 0x200u32.to_le_bytes();
        let both = group(
            &label,
            6,
            &[
                group(&label, 9, &refr(0x202, 0)),
                group(&label, 8, &refr(0x201, 0x400)),
            ]
            .concat(),
        );
        let cell = record(b"CELL", 0, 0x200, &sub(b"EDID", b"c\0"));
        let official = fixture::plugin(&[group(b"CELL", 0, &[cell.clone(), both].concat())]);
        let index = index_bytes(&official, &IndexOptions::default()).unwrap();
        assert_eq!(PlacementStyle::detect(&index), PlacementStyle::OFFICIAL);
        // (The fixture stamps groups 0x2A39; the timestamp is set by hand here.)
        let mut plain = fixture::plugin(&[group(b"CELL", 0, &cell)]);
        let top = plain.len() - (24 + cell.len());
        plain[top + 16..top + 18].copy_from_slice(&0u16.to_le_bytes());
        let index = index_bytes(&plain, &IndexOptions::default()).unwrap();
        assert_eq!(PlacementStyle::detect(&index), PlacementStyle::OFFICIAL);
        let mut stamped = plain.clone();
        stamped[top + 16..top + 18].copy_from_slice(&13505u16.to_le_bytes());
        let index = index_bytes(&stamped, &IndexOptions::default()).unwrap();
        assert_eq!(
            PlacementStyle::detect(&index),
            PlacementStyle::blueprint(13505)
        );
        // ONAM: records in types 7, 9 and 10 that the caller says override a master's.
        let index = index_bytes(&official, &IndexOptions::default()).unwrap();
        let plugin = crate::Plugin::parse(&official).unwrap();
        let pick = |id: u32| id != 0x999;
        assert_eq!(onam_forms_in(&index, pick), [0x202]);
        assert_eq!(onam_forms(&plugin, pick), [0x202]);
        assert!(onam_forms_in(&index, |_| false).is_empty());
    }
}
