//! Plugin builder and record placement (M2.A5).
//!
//! - [`HeaderSpec`] / [`Plugin::create`]: a new plugin with a `TES4` header (raw flags,
//!   `HEDR`, `CNAM`/`SNAM`, masters exactly as the caller lists them) and no records.
//! - [`Plugin::upsert`]: put one record into a plugin in memory, replacing the record with
//!   the same FormID in its group or adding it, creating missing groups on the way.
//! - [`upsert_reader`] / [`upsert_file_to_path`]: the same for any number of records while
//!   streaming the rest of an existing plugin, one top-level group at a time.
//!
//! # Placement
//!
//! A record goes where its [`GroupHeader`] path says: the top-level group first (type 0,
//! labelled with a record type), then each nested group, matched by label and group type.
//! The path is computed by [`crate::cells::placement`] (M4.A2) from the record and the
//! world, cell, quest or topic it belongs to, or taken from the plugin a record comes from
//! ([`crate::index::PluginIndex::group_headers`]).
//!
//! Missing groups and new records go where the corpus rules of [`crate::cells`] put them
//! (M4.A2; the rebuild gate in `tests/rebuild.rs` reproduces every top-level group of the
//! 14 official files this way): a top-level group in [`TOP_GROUP_ORDER`] order; a children
//! group (types 1, 6, 7, 10) right after the record its label names, which must be there
//! (refused otherwise: carry the `WRLD`/`CELL` override first,
//! [`crate::cells::carry_parents`]; only [`Plugin::upsert_before_owner`], for rebuilding
//! from records in any order, puts it at the end of its parent and moves it when the
//! record comes); blocks and sub-blocks (types 2 to 5) by label; a
//! cell's persistent and temporary children in the [`PlacementStyle`]'s order; records by
//! FormID where the corpus orders them (sub-blocks, topic and quest children, the
//! top-level `WRLD` and `QUST` groups), a world's persistent cell first, `NAVM` first in a
//! temporary group when the style says so, and appended everywhere else. A new group gets
//! the style's stamps, and a nested `NAVM`, `DIAL`, `INFO`, `DLBR` or `SCEN` also gets its
//! empty top-level group when the style keeps one ([`Plugin::upsert_styled`];
//! [`Plugin::upsert`] and the streaming upsert's default use the official pipeline's style,
//! [`RewriteOptions::placement`] another).
//!
//! Inside the innermost group a direct child record with the same FormID and signature is
//! replaced in place; a reference whose flag 0x400 changed leaves the cell's other
//! children group ([`Upsert::Moved`]). The path's innermost group must agree with the
//! record's flag 0x400 (types 8 and 9, a `CELL` directly in type 1), and a world keeps one
//! persistent cell. Records are copied as given: their FormIDs are stored values,
//! file-relative to the masters of the plugin they are written into, and nothing here
//! remaps them or infers anything about a master.
//!
//! The `HEDR` record count follows [`crate::tes4`]: recomputed (majority meaning) when
//! records are added, kept when one is only replaced.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::cells::{ChildOrder, PlacementStyle};
use crate::driver::{stream_rewrite, to_path, RewriteOptions, RewriteReport};
use crate::model::{Body, Entry, Group, GroupHeader, Plugin, Record, RecordHeader, Subrecord};
use crate::{Error, Reader, Result, Signature, HEADER_LEN};

/// Top-level group order of the official ESMs: every record type that has a top-level
/// group in any of the 14 files, in file order. `corpus:` the 176 top-level groups of
/// `Starfield.esm` in order, merged with the 5 types only other files have (`PERS` after
/// `RFGP` in both `BlueprintShips` files; `SCCO` between `KSSM` and `STAG` in
/// `ShatteredSpace.esm`; `GPOF`, `GPOG`, `GWED` after `AVMD` in `SFBGS007.esm`,
/// `SFBGS00D.esm` and `SFBGS050.esm`, placed last). Every file's own order is a
/// subsequence of this one (`tests/build.rs`). Where two types never share a file the
/// order between them is a choice, not an observation.
pub const TOP_GROUP_ORDER: &[[u8; 4]] = &[
    *b"GMST", *b"KYWD", *b"FFKW", *b"LCRT", *b"AACT", *b"TRNS", *b"TXST", *b"GLOB", *b"DMGT",
    *b"CLAS", *b"FACT", *b"AFFE", *b"HDPT", *b"RACE", *b"SOUN", *b"SECH", *b"ASPC", *b"AOPF",
    *b"MGEF", *b"LTEX", *b"PDCL", *b"ENCH", *b"SPEL", *b"ACTI", *b"CURV", *b"CUR3", *b"ARMO",
    *b"BOOK", *b"CONT", *b"DOOR", *b"LIGH", *b"MISC", *b"STAT", *b"SCOL", *b"PKIN", *b"MSTT",
    *b"GRAS", *b"FLOR", *b"FURN", *b"WEAP", *b"AMMO", *b"NPC_", *b"LVLN", *b"LVLP", *b"KEYM",
    *b"ALCH", *b"IDLM", *b"BMMO", *b"PROJ", *b"HAZD", *b"BNDS", *b"TERM", *b"LVLI", *b"GBFT",
    *b"GBFM", *b"LVLB", *b"WTHR", *b"WTHS", *b"CLMT", *b"SPGD", *b"REGN", *b"NAVI", *b"CELL",
    *b"WRLD", *b"NAVM", *b"DIAL", *b"INFO", *b"QUST", *b"IDLE", *b"PACK", *b"CSTY", *b"LSCR",
    *b"ANIO", *b"WATR", *b"EFSH", *b"EXPL", *b"DEBR", *b"IMGS", *b"IMAD", *b"FLST", *b"PERK",
    *b"BPTD", *b"ADDN", *b"AVIF", *b"CAMS", *b"CPTH", *b"VTYP", *b"MATT", *b"IPCT", *b"IPDS",
    *b"ARMA", *b"LCTN", *b"MESG", *b"DOBJ", *b"DFOB", *b"LGTM", *b"MUSC", *b"FSTP", *b"FSTS",
    *b"SMBN", *b"SMQN", *b"SMEN", *b"DLBR", *b"MUST", *b"EQUP", *b"SCEN", *b"OTFT", *b"ARTO",
    *b"MOVT", *b"COLL", *b"CLFM", *b"REVB", *b"RFGP", *b"PERS", *b"AMDL", *b"AAMD", *b"MAAM",
    *b"LAYR", *b"COBJ", *b"OMOD", *b"ZOOM", *b"INNR", *b"KSSM", *b"AORU", *b"SCCO", *b"STAG",
    *b"IRES", *b"BIOM", *b"NOCM", *b"LENS", *b"OVIS", *b"STND", *b"STMP", *b"GCVR", *b"MRPH",
    *b"TRAV", *b"RSGD", *b"OSWP", *b"ATMO", *b"LVSC", *b"SPCH", *b"AAPD", *b"VOLI", *b"SFBK",
    *b"SFPC", *b"SFPT", *b"SFTR", *b"PCMT", *b"BMOD", *b"STBH", *b"PNDT", *b"CNDF", *b"PCBN",
    *b"PCCN", *b"STDT", *b"WWED", *b"RSPJ", *b"AOPS", *b"AMBS", *b"WBAR", *b"PTST", *b"LMSW",
    *b"FORC", *b"TMLM", *b"EFSQ", *b"SDLT", *b"MTPT", *b"CLDF", *b"FOGV", *b"WKMF", *b"LGDI",
    *b"PSDC", *b"SUNP", *b"PMFT", *b"TODD", *b"AVMD", *b"CHAL", *b"FXPD", *b"GPOF", *b"GPOG",
    *b"GWED",
];

/// Position of record type `signature` in [`TOP_GROUP_ORDER`]; `None` for a type the
/// official files have no top-level group for (placed after every known one).
pub fn top_group_rank(signature: Signature) -> Option<usize> {
    TOP_GROUP_ORDER.iter().position(|s| *s == signature.0)
}

/// Rank of a top-level entry for ordering: known types by [`top_group_rank`], everything
/// else (unknown types, records at the top level) after them.
fn entry_rank(entry: &Entry<'_>) -> usize {
    match entry {
        Entry::Group(g) if g.header.group_type == 0 => {
            top_group_rank(g.header.label_signature()).unwrap_or(usize::MAX)
        }
        _ => usize::MAX,
    }
}

impl GroupHeader {
    /// The header of the top-level group (type 0) for record type `signature`, with zero
    /// stamps and unknown field.
    pub fn top(signature: Signature) -> GroupHeader {
        GroupHeader {
            label: signature.0,
            group_type: 0,
            ..GroupHeader::default()
        }
    }

    /// Whether `other` names the same group: same label and group type (stamps and the
    /// unknown field are not compared).
    pub fn same_group(&self, other: &GroupHeader) -> bool {
        self.label == other.label && self.group_type == other.group_type
    }
}

/// The `TES4` header of a new plugin. Every field is written as given.
///
/// [`HeaderSpec::new`] fills in values observed in the official ESMs (`docs/corpus.md`,
/// TES4 header inventory): `HEDR` version 0.96 and next object ID 0xFF001000 (12 of 14
/// files), form version 582 (the newest in the corpus), and flags 0 (neither master nor
/// localized). What the Creation Kit writes for a new plugin is M2.E3's to settle; change
/// the fields when it is.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderSpec {
    /// Raw `TES4` record flags (see the constants in [`crate::tes4`]).
    pub flags: u32,
    /// Form version in the `TES4` record header.
    pub form_version: u16,
    /// `HEDR` version.
    pub version: f32,
    /// `HEDR` next object ID.
    pub next_object_id: u32,
    /// `CNAM` (author), without its terminating NUL; omitted when `None`.
    pub author: Option<Vec<u8>>,
    /// `SNAM` (description), without its terminating NUL; omitted when `None`.
    pub description: Option<Vec<u8>>,
    /// Master file names in load order, without terminating NULs. Each is written as
    /// `MAST` followed by an 8-byte `DATA` of zero (every `MAST` in the official files is
    /// followed by an 8-byte `DATA`, `docs/corpus.md`).
    pub masters: Vec<Vec<u8>>,
}

impl HeaderSpec {
    /// A header for a non-localized, non-master plugin with these masters.
    pub fn new(masters: Vec<Vec<u8>>) -> HeaderSpec {
        HeaderSpec {
            flags: 0,
            form_version: 582,
            version: 0.96,
            next_object_id: 0xFF00_1000,
            author: None,
            description: None,
            masters,
        }
    }

    /// The `TES4` record. The `HEDR` record count is 1 (the header alone); the writer
    /// recomputes it once records are added.
    pub fn to_record(&self) -> Record<'static> {
        let mut hedr = Vec::with_capacity(12);
        hedr.extend_from_slice(&self.version.to_le_bytes());
        hedr.extend_from_slice(&1u32.to_le_bytes());
        hedr.extend_from_slice(&self.next_object_id.to_le_bytes());
        let mut subs = vec![Subrecord::new(Signature::new(b"HEDR"), hedr)];
        let zstring = |text: &[u8]| {
            let mut v = text.to_vec();
            v.push(0);
            v
        };
        if let Some(author) = &self.author {
            subs.push(Subrecord::new(Signature::new(b"CNAM"), zstring(author)));
        }
        if let Some(description) = &self.description {
            subs.push(Subrecord::new(
                Signature::new(b"SNAM"),
                zstring(description),
            ));
        }
        subs.extend(master_subrecords(&self.masters));
        Record::new(
            RecordHeader {
                signature: Signature::TES4,
                flags: self.flags,
                form_version: self.form_version,
                ..RecordHeader::default()
            },
            Body::Raw(subs),
        )
    }
}

/// `MAST` + 8-byte zero `DATA` for each name.
fn master_subrecords(masters: &[Vec<u8>]) -> Vec<Subrecord<'static>> {
    let mut subs = Vec::with_capacity(masters.len().saturating_mul(2));
    for name in masters {
        let mut data = name.clone();
        data.push(0);
        subs.push(Subrecord::new(Signature::new(b"MAST"), data));
        subs.push(Subrecord::new(Signature::new(b"DATA"), vec![0u8; 8]));
    }
    subs
}

/// Replace the master list of `record` (a `TES4` record) with `masters`, in order. The
/// `MAST`/`DATA` pairs go where the first `MAST` was, or else after `HEDR`, `CNAM` and
/// `SNAM` (UESP, Skyrim_Mod:Mod_File_Format/TES4, field order). A `DATA` right after a
/// removed `MAST` is removed with it. Returns `false` (and changes nothing) for a
/// compressed record that has not been decoded.
pub fn set_masters(record: &mut Record<'_>, masters: &[Vec<u8>]) -> bool {
    let Some(subs) = record.subrecords_mut() else {
        return false;
    };
    let mast = Signature::new(b"MAST");
    let data = Signature::new(b"DATA");
    let mut kept: Vec<Subrecord<'_>> = Vec::with_capacity(subs.len());
    let mut at: Option<usize> = None;
    let mut after_mast = false;
    for s in subs.drain(..) {
        if s.signature() == mast {
            at.get_or_insert(kept.len());
            after_mast = true;
            continue;
        }
        if s.signature() == data && after_mast {
            after_mast = false;
            continue;
        }
        after_mast = false;
        kept.push(s);
    }
    let at = at.unwrap_or_else(|| {
        let leading = [b"HEDR", b"CNAM", b"SNAM"].map(Signature::new);
        kept.iter()
            .take_while(|s| leading.contains(&s.signature()))
            .count()
    });
    let tail = kept.split_off(at.min(kept.len()));
    kept.extend(master_subrecords(masters));
    kept.extend(tail);
    *subs = kept;
    true
}

/// What [`Plugin::upsert`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upsert {
    /// A record with the same FormID and signature was replaced in place.
    Replaced,
    /// The record was added (and any missing group created).
    Inserted,
    /// A record with the same FormID was in the cell's other children group (persistent
    /// or temporary, under the same cell children group: its record flag 0x400 changed).
    /// It was taken out of there (with that group, when it was its last record) and the
    /// record put where its path says. A move between two cells is the caller's: this
    /// sees one cell children group only.
    Moved,
}

/// A record and where it goes (see the module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// Group headers from the top-level group down; the first must be type 0.
    pub path: Vec<GroupHeader>,
    pub record: Record<'static>,
}

impl Placed {
    /// A record of a type that lives directly in its top-level group.
    pub fn top_level(record: Record<'static>) -> Placed {
        Placed {
            path: vec![GroupHeader::top(record.header.signature)],
            record,
        }
    }
}

/// Refuse a path whose innermost group disagrees with the record's flags where the corpus
/// has no exception (`crate::cells`, "Where records live"): a record in persistent children
/// (type 8) carries record flag 0x400 and is no `NAVM`, a record in temporary children
/// (type 9) does not carry it, and a `CELL` directly in a world's children group (type 1)
/// is its persistent cell and carries it.
fn check_flags(path: &[GroupHeader], record: &Record<'_>) -> Result<()> {
    use crate::cells::group_type::*;
    use crate::cells::FLAG_PERSISTENT;
    let signature = record.header.signature;
    let persistent = record.header.flags & FLAG_PERSISTENT != 0;
    let reason = match path.last().map(|g| g.group_type) {
        Some(CELL_PERSISTENT) if signature.0 == *b"NAVM" => {
            "no NAVM is in persistent children (type 8) in the corpus"
        }
        Some(CELL_PERSISTENT) if !persistent => {
            "every record in persistent children (type 8) carries record flag 0x400 in the \
             corpus"
        }
        Some(CELL_TEMPORARY) if persistent => {
            "no record in temporary children (type 9) carries record flag 0x400 in the corpus"
        }
        Some(WORLD_CHILDREN) if signature.0 == *b"CELL" && !persistent => {
            "a CELL directly in a world's children group (type 1) is its persistent cell and \
             carries record flag 0x400 in the corpus"
        }
        _ => return Ok(()),
    };
    Err(Error::Placement {
        offset: record.offset().unwrap_or(0),
        reason,
    })
}

fn check_path(path: &[GroupHeader], record: &Record<'_>) -> Result<()> {
    check_flags(path, record)?;
    let offset = record.offset().unwrap_or(0);
    match path.first() {
        None => Err(Error::Placement {
            offset,
            reason: "the group path is empty",
        }),
        Some(top) if top.group_type != 0 => Err(Error::Placement {
            offset,
            reason: "the group path does not start with a top-level group (type 0)",
        }),
        Some(_) if path.iter().skip(1).any(|g| g.group_type == 0) => Err(Error::Placement {
            offset,
            reason: "a nested group in the path has type 0",
        }),
        Some(_) if path.len() > crate::MAX_DEPTH_CAP => Err(Error::TooDeep {
            offset,
            max: crate::MAX_DEPTH_CAP,
        }),
        Some(_) if record.header.signature == Signature::GRUP => Err(Error::Placement {
            offset,
            reason: "the record's signature is GRUP",
        }),
        Some(_) => Ok(()),
    }
}

/// Sort key of a record among the records of a group whose records ascend (see
/// [`records_ascend`]): quest children order `DLBR`, `DIAL`, `SCEN` first, then all by
/// stored FormID.
fn record_key(record: &Record<'_>) -> (u8, u32) {
    let class = match &record.header.signature.0 {
        b"DLBR" => 0,
        b"DIAL" => 1,
        b"SCEN" => 2,
        _ => 3,
    };
    (class, record.header.form_id)
}

/// Whether the records of a group with header `parent` ascend by [`record_key`] in the
/// corpus (`crate::cells`, "Order"): interior and exterior sub-blocks, topic and quest
/// children, and the top-level `WRLD` and `QUST` groups.
fn records_ascend(parent: &GroupHeader) -> bool {
    use crate::cells::group_type::*;
    match parent.group_type {
        TOP => parent.label == *b"WRLD" || parent.label == *b"QUST",
        INTERIOR_SUB_BLOCK | EXTERIOR_SUB_BLOCK | TOPIC_CHILDREN | QUEST_CHILDREN => true,
        _ => false,
    }
}

/// Where a new record goes among `children`, the children of the group `parent`.
fn record_position(
    children: &[Entry<'_>],
    parent: &GroupHeader,
    record: &Record<'_>,
    style: &PlacementStyle,
) -> usize {
    use crate::cells::group_type::*;
    let end = children.len();
    if records_ascend(parent) {
        let key = record_key(record);
        return children
            .iter()
            .position(|e| matches!(e, Entry::Record(r) if record_key(r) > key))
            .unwrap_or(end);
    }
    match parent.group_type {
        // The world's persistent cell comes before the exterior blocks.
        WORLD_CHILDREN => children
            .iter()
            .position(|e| matches!(e, Entry::Group(g) if g.header.group_type == EXTERIOR_BLOCK))
            .unwrap_or(end),
        CELL_TEMPORARY if style.navmesh_first && record.header.signature.0 == *b"NAVM" => children
            .iter()
            .position(|e| !matches!(e, Entry::Record(r) if r.header.signature.0 == *b"NAVM"))
            .unwrap_or(end),
        _ => end,
    }
}

/// Where a new group `header` goes among `children`.
fn group_position(
    children: &[Entry<'_>],
    parent: &GroupHeader,
    header: &GroupHeader,
    style: &PlacementStyle,
) -> usize {
    use crate::cells::group_type::*;
    let end = children.len();
    let first_block = || {
        children
            .iter()
            .position(|e| matches!(e, Entry::Group(g) if g.header.group_type == EXTERIOR_BLOCK))
            .unwrap_or(end)
    };
    match header.group_type {
        // Blocks and sub-blocks ascend by label read as a u32.
        INTERIOR_BLOCK | INTERIOR_SUB_BLOCK | EXTERIOR_BLOCK | EXTERIOR_SUB_BLOCK => children
            .iter()
            .position(|e| {
                matches!(e, Entry::Group(g) if g.header.group_type == header.group_type
                    && g.header.label_u32() > header.label_u32())
            })
            .unwrap_or(end),
        CELL_PERSISTENT | CELL_TEMPORARY => {
            let first = match style.cell_children {
                ChildOrder::TemporaryFirst => CELL_TEMPORARY,
                ChildOrder::PersistentFirst => CELL_PERSISTENT,
            };
            if header.group_type == first {
                0
            } else {
                end
            }
        }
        // Children groups follow the record their label names. Without that record, the
        // children of a world's persistent cell still come before the exterior blocks;
        // anything else goes to the end (the record, when it comes, is put before it).
        _ => {
            let owner = header.label_u32();
            match children
                .iter()
                .position(|e| matches!(e, Entry::Record(r) if r.header.form_id == owner))
            {
                Some(i) => i.saturating_add(1),
                None if parent.group_type == WORLD_CHILDREN => first_block(),
                None => end,
            }
        }
    }
}

/// Whether `group_type` is a children group type (1, 6, 7, 10): a group that belongs to
/// the record its label names.
fn is_children_type(group_type: i32) -> bool {
    use crate::cells::group_type::*;
    matches!(
        group_type,
        WORLD_CHILDREN | CELL_CHILDREN | TOPIC_CHILDREN | QUEST_CHILDREN
    )
}

/// Whether `group` is the children group (types 1, 6, 7, 10) of the record `form_id`.
fn is_children_of(group: &GroupHeader, form_id: u32) -> bool {
    is_children_type(group.group_type) && group.label_u32() == form_id
}

/// Whether a missing children group may be created before the record it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owners {
    /// Refused ([`NO_OWNER`]): the default.
    Required,
    /// Created at the end of its parent; the record, when it comes, moves it after itself
    /// ([`Plugin::upsert_before_owner`]).
    MayFollow,
}

const NO_OWNER: &str = "a new children group (type 1, 6, 7 or 10) needs the WRLD, CELL, DIAL \
                        or QUST record it belongs to in its parent group: carry the WRLD/CELL \
                        override first (cells::carry_parents)";

/// Refuse (under [`Owners::Required`]) placing `record` at `rest` below a top-level group
/// whose children are `top_children` (`None` when the group does not exist yet) when that
/// would create a children group whose owner record is not in the parent group. Read
/// only: nothing is created before the refusal.
fn check_owners(
    top_children: Option<&[Entry<'_>]>,
    rest: &[GroupHeader],
    record: &Record<'_>,
    owners: Owners,
) -> Result<()> {
    if owners == Owners::MayFollow {
        return Ok(());
    }
    let mut level = top_children;
    for next in rest {
        let found = level.and_then(|kids| {
            kids.iter().find_map(|e| match e {
                Entry::Group(g) if g.header.same_group(next) => Some(g.children()),
                _ => None,
            })
        });
        if found.is_none() && is_children_type(next.group_type) {
            let owner = next.label_u32();
            let present = level.is_some_and(|kids| {
                kids.iter()
                    .any(|e| matches!(e, Entry::Record(r) if r.header.form_id == owner))
            });
            if !present {
                return Err(Error::Placement {
                    offset: record.offset().unwrap_or(0),
                    reason: NO_OWNER,
                });
            }
        }
        level = found;
    }
    Ok(())
}

/// Take the record `form_id` out of the sibling of the persistent or temporary group
/// `next` (the other of types 8 and 9 with the same label) among `children`, the children
/// of a cell children group; drop that sibling when it is left empty. Whether one was
/// taken.
fn take_from_sibling(children: &mut Vec<Entry<'_>>, next: &GroupHeader, form_id: u32) -> bool {
    use crate::cells::group_type::*;
    let other = match next.group_type {
        CELL_PERSISTENT => CELL_TEMPORARY,
        CELL_TEMPORARY => CELL_PERSISTENT,
        _ => return false,
    };
    let Some(at) = children.iter().position(
        |e| matches!(e, Entry::Group(g) if g.header.group_type == other && g.header.label == next.label),
    ) else {
        return false;
    };
    let (taken, empty) = match children.get_mut(at) {
        Some(Entry::Group(g)) => {
            let kids = g.children_mut();
            let before = kids.len();
            kids.retain(|e| !matches!(e, Entry::Record(r) if r.header.form_id == form_id));
            (kids.len() != before, kids.is_empty())
        }
        _ => (false, false),
    };
    if taken && empty {
        children.remove(at);
    }
    taken
}

/// Place `record` under the groups `rest` inside `children`, the children of the group
/// `parent` the path has reached.
fn place<'a>(
    children: &mut Vec<Entry<'a>>,
    parent: &GroupHeader,
    rest: &[GroupHeader],
    record: Record<'a>,
    style: &PlacementStyle,
) -> Result<Upsert> {
    use crate::cells::group_type::*;
    let Some((next, deeper)) = rest.split_first() else {
        let form_id = record.header.form_id;
        if parent.group_type == WORLD_CHILDREN
            && record.header.signature.0 == *b"CELL"
            && children.iter().any(|e| {
                matches!(e, Entry::Record(r) if r.header.signature.0 == *b"CELL"
                    && r.header.form_id != form_id)
            })
        {
            return Err(Error::Placement {
                offset: record.offset().unwrap_or(0),
                reason: "a world has one persistent cell in the corpus, and this world's \
                         children group already holds another CELL",
            });
        }
        let same = children.iter().position(|e| match e {
            Entry::Record(r) => {
                r.header.form_id == form_id && r.header.signature == record.header.signature
            }
            Entry::Group(_) => false,
        });
        return Ok(match same.and_then(|i| children.get_mut(i)) {
            Some(slot) => {
                *slot = Entry::Record(record);
                Upsert::Replaced
            }
            None => {
                // A children group placed before its record moves to follow it.
                let orphan = children
                    .iter()
                    .position(
                        |e| matches!(e, Entry::Group(g) if is_children_of(&g.header, form_id)),
                    )
                    .map(|j| children.remove(j));
                let at = record_position(children, parent, &record, style);
                children.insert(at, Entry::Record(record));
                if let Some(group) = orphan {
                    children.insert(at.saturating_add(1), group);
                }
                Upsert::Inserted
            }
        });
    };
    // A record whose persistence changed leaves the cell's other children group.
    let moved = parent.group_type == CELL_CHILDREN
        && deeper.is_empty()
        && take_from_sibling(children, next, record.header.form_id);
    let found = children.iter().position(|e| match e {
        Entry::Group(g) => g.header.same_group(next),
        Entry::Record(_) => false,
    });
    let index = match found {
        Some(i) => i,
        None => {
            let at = group_position(children, parent, next, style);
            children.insert(at, Entry::Group(Group::new(*next)));
            at
        }
    };
    let outcome = match children.get_mut(index) {
        Some(Entry::Group(g)) => {
            let header = g.header;
            place(g.children_mut(), &header, deeper, record, style)?
        }
        _ => {
            return Err(Error::Internal {
                offset: 0,
                message: "a group found or inserted while placing a record is not there",
            })
        }
    };
    Ok(if moved { Upsert::Moved } else { outcome })
}

/// The empty top-level group a record placed at `path` needs under `style`, if any: the
/// official pipeline keeps an empty top-level group for each of
/// [`crate::cells::PLACEHOLDER_TYPES`] it has nested records of.
fn placeholder_for(
    path: &[GroupHeader],
    record: &Record<'_>,
    style: &PlacementStyle,
) -> Option<GroupHeader> {
    let signature = record.header.signature;
    (style.placeholder_groups
        && path.len() > 1
        && crate::cells::PLACEHOLDER_TYPES.contains(&signature))
    .then(|| style.header(signature.0, 0))
}

impl Plugin<'static> {
    /// A new plugin with the header `spec` describes and no records.
    pub fn create(spec: &HeaderSpec) -> Plugin<'static> {
        Plugin::new(spec.to_record())
    }
}

impl<'a> Plugin<'a> {
    /// Put `record` under the groups `path` (see the module documentation): replace the
    /// record with the same FormID and signature in the innermost group, or add it,
    /// creating missing groups. The top-level group is created in [`TOP_GROUP_ORDER`]
    /// order, before the first top-level entry ranked after it.
    pub fn upsert(&mut self, path: &[GroupHeader], record: Record<'a>) -> Result<Upsert> {
        self.upsert_styled(path, record, &PlacementStyle::OFFICIAL)
    }

    /// [`Plugin::upsert`] with the group order and stamps of `style` (see
    /// [`crate::cells`]): new groups and records go where the corpus rules put them, and
    /// a nested `NAVM`, `DIAL`, `INFO`, `DLBR` or `SCEN` also gets its empty top-level
    /// group when the style keeps one.
    ///
    /// Refused (`Error::Placement`), with nothing changed: a path whose innermost group
    /// disagrees with the record's flag 0x400 (types 8, 9, a `CELL` directly in type 1); a
    /// second persistent `CELL` in a world; and a path that would create a children group
    /// (types 1, 6, 7, 10) whose `WRLD`, `CELL`, `DIAL` or `QUST` record is not in its
    /// parent group: upsert that record first (for a master's cell,
    /// [`crate::cells::carry_parents`] gives the `WRLD` and `CELL` overrides). The path
    /// itself is otherwise taken as given: [`crate::cells::placement`] computes one that
    /// follows the corpus rules.
    pub fn upsert_styled(
        &mut self,
        path: &[GroupHeader],
        record: Record<'a>,
        style: &PlacementStyle,
    ) -> Result<Upsert> {
        self.upsert_with(path, record, style, Owners::Required)
    }

    /// [`Plugin::upsert_styled`] that may create a children group before the record it
    /// belongs to (at the end of its parent; the record, when upserted, moves the group
    /// after itself). **Only for rebuilding a plugin from all of its records in any order**
    /// (the rebuild gate's shuffles): a plugin written before every such record is there
    /// has a children group without its record, which the corpus never has.
    pub fn upsert_before_owner(
        &mut self,
        path: &[GroupHeader],
        record: Record<'a>,
        style: &PlacementStyle,
    ) -> Result<Upsert> {
        self.upsert_with(path, record, style, Owners::MayFollow)
    }

    fn upsert_with(
        &mut self,
        path: &[GroupHeader],
        record: Record<'a>,
        style: &PlacementStyle,
        owners: Owners,
    ) -> Result<Upsert> {
        check_path(path, &record)?;
        let Some((top, rest)) = path.split_first() else {
            return Err(Error::Internal {
                offset: 0,
                message: "an empty path passed the path check",
            });
        };
        let top_children = self.entries().iter().find_map(|e| match e {
            Entry::Group(g) if g.header.same_group(top) => Some(g.children()),
            _ => None,
        });
        check_owners(top_children, rest, &record, owners)?;
        if let Some(placeholder) = placeholder_for(path, &record, style) {
            self.top_group(&placeholder);
        }
        let index = self.top_group(top);
        match self.entries_mut().get_mut(index) {
            Some(Entry::Group(g)) => {
                let header = g.header;
                place(g.children_mut(), &header, rest, record, style)
            }
            _ => Err(Error::Internal {
                offset: 0,
                message: "top-level group vanished while placing a record",
            }),
        }
    }

    /// The index of the top-level group `top` names, created in [`TOP_GROUP_ORDER`] order
    /// when missing.
    fn top_group(&mut self, top: &GroupHeader) -> usize {
        let entries = self.entries_mut();
        let found = entries.iter().position(|e| match e {
            Entry::Group(g) => g.header.same_group(top),
            Entry::Record(_) => false,
        });
        match found {
            Some(i) => i,
            None => {
                let rank = top_group_rank(Signature(top.label)).unwrap_or(usize::MAX);
                let at = entries
                    .iter()
                    .position(|e| entry_rank(e) > rank)
                    .unwrap_or(entries.len());
                entries.insert(at, Entry::Group(Group::new(*top)));
                at
            }
        }
    }
}

/// Result of a streaming upsert.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertReport {
    /// The stream (sizes, counts, the `HEDR` fix).
    pub rewrite: RewriteReport,
    /// What happened to each record, in the order given.
    pub outcomes: Vec<Upsert>,
}

/// Labels of the top-level groups of a plugin, read from their headers alone. Leaves
/// `input` at its start.
fn top_level_labels<R: Read + Seek>(input: &mut R, len: u64) -> Result<Vec<GroupHeader>> {
    let mut labels = Vec::new();
    let mut offset: u64 = 0;
    while offset < len {
        input
            .seek(SeekFrom::Start(offset))
            .map_err(|source| Error::Io { offset, source })?;
        let mut head = [0u8; HEADER_LEN];
        input
            .read_exact(&mut head)
            .map_err(|source| Error::Io { offset, source })?;
        let mut r = Reader::new(&head, offset);
        let signature = r.signature()?;
        let size = u64::from(r.u32()?);
        let total = if signature == Signature::GRUP {
            labels.push(GroupHeader {
                label: r.array::<4>("group label")?,
                group_type: r.i32()?,
                ..GroupHeader::default()
            });
            size
        } else {
            size.saturating_add(HEADER_LEN as u64)
        };
        if total < HEADER_LEN as u64 {
            return Err(Error::GroupTooSmall {
                offset,
                size: u32::try_from(size).unwrap_or(u32::MAX),
            });
        }
        offset = offset.saturating_add(total);
    }
    input
        .seek(SeekFrom::Start(0))
        .map_err(|source| Error::Io { offset: 0, source })?;
    Ok(labels)
}

/// Stream the plugin in `input` (`len` bytes) to `sink`, placing each record of `placed`
/// as [`Plugin::upsert`] would and copying everything else. `header` may change the
/// `TES4` record first (for example [`set_masters`]). Top-level groups the plugin lacks
/// are created in [`TOP_GROUP_ORDER`] order; a record is replaced only when its FormID is
/// found in the group its path names.
///
/// Memory: one top-level group and the records being placed. The input is read twice:
/// once for its top-level group headers (seeking past each group), once to copy it.
pub fn upsert_reader<R: Read + Seek>(
    mut input: R,
    len: u64,
    options: &RewriteOptions,
    sink: &mut dyn Write,
    placed: Vec<Placed>,
    header: impl FnOnce(&mut Record<'_>) -> Result<()>,
) -> Result<UpsertReport> {
    for p in &placed {
        check_path(&p.path, &p.record)?;
    }
    let style = options.placement;
    let existing = top_level_labels(&mut input, len)?;
    let mut outcomes: Vec<Option<Upsert>> = vec![None; placed.len()];
    // Records whose top-level group the plugin has wait for it in the stream; the others
    // go into new top-level groups now, written before the first entry ranked after them.
    let mut pending: Vec<Option<Placed>> = Vec::with_capacity(placed.len());
    let mut created: Vec<(usize, Group<'static>)> = Vec::new();
    let create = |created: &mut Vec<(usize, Group<'static>)>, top: GroupHeader| -> usize {
        match created.iter().position(|(_, g)| g.header.same_group(&top)) {
            Some(i) => i,
            None => {
                let rank = top_group_rank(Signature(top.label)).unwrap_or(usize::MAX);
                created.push((rank, Group::new(top)));
                created.len().saturating_sub(1)
            }
        }
    };
    for (p, outcome) in placed.into_iter().zip(outcomes.iter_mut()) {
        let Some(top) = p.path.first().copied() else {
            pending.push(None);
            continue;
        };
        if let Some(placeholder) = placeholder_for(&p.path, &p.record, &style) {
            if !existing.iter().any(|g| g.same_group(&placeholder)) {
                create(&mut created, placeholder);
            }
        }
        if existing.iter().any(|g| g.same_group(&top)) {
            pending.push(Some(p));
            continue;
        }
        pending.push(None);
        let rest = p.path.get(1..).unwrap_or(&[]);
        let i = create(&mut created, top);
        if let Some((_, g)) = created.get_mut(i) {
            check_owners(Some(g.children()), rest, &p.record, Owners::Required)?;
            let header = g.header;
            *outcome = Some(place(g.children_mut(), &header, rest, p.record, &style)?);
        }
    }
    // Stable: groups of equal rank (unknown types) keep their first-use order.
    created.sort_by_key(|(rank, _)| *rank);
    let mut created = created.into_iter().peekable();

    let mut header = Some(header);
    let mut edit = |entry: &mut Entry<'_>| -> Result<()> {
        match entry {
            Entry::Record(r) if r.header.signature == Signature::TES4 => {
                if let Some(f) = header.take() {
                    f(r)?;
                }
            }
            Entry::Group(g) if g.header.group_type == 0 => {
                for (slot, outcome) in pending.iter_mut().zip(outcomes.iter_mut()) {
                    let matches = slot
                        .as_ref()
                        .and_then(|p| p.path.first())
                        .is_some_and(|top| g.header.same_group(top));
                    if !matches {
                        continue;
                    }
                    if let Some(p) = slot.take() {
                        let rest = p.path.get(1..).unwrap_or(&[]);
                        check_owners(Some(g.children()), rest, &p.record, Owners::Required)?;
                        let header = g.header;
                        *outcome = Some(place(g.children_mut(), &header, rest, p.record, &style)?);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    };
    let mut insert = |next: Option<&Entry<'_>>| -> Result<Vec<Entry<'static>>> {
        let limit = next.map(entry_rank);
        let mut out = Vec::new();
        while let Some((_, g)) = created.next_if(|(rank, _)| limit.is_none_or(|l| *rank < l)) {
            out.push(Entry::Group(g));
        }
        Ok(out)
    };
    let rewrite = stream_rewrite(input, len, options, sink, &mut edit, &mut insert)?;
    // Every existing top-level group is streamed, so every record has been placed.
    let outcomes = outcomes
        .into_iter()
        .map(|o| {
            o.ok_or(Error::Internal {
                offset: 0,
                message: "a record to place was never placed",
            })
        })
        .collect::<Result<Vec<Upsert>>>()?;
    Ok(UpsertReport { rewrite, outcomes })
}

/// [`upsert_reader`] from the plugin at `path` into a new file at `out` (through a
/// temporary file, renamed when complete), with the `HEDR` count patched when records
/// were added. `out` may not be `path` itself.
pub fn upsert_file_to_path(
    path: &Path,
    out: &Path,
    options: &RewriteOptions,
    placed: Vec<Placed>,
    header: impl FnOnce(&mut Record<'_>) -> Result<()>,
) -> Result<UpsertReport> {
    let mut outcomes = Vec::new();
    let rewrite = to_path(path, out, |input, len, sink| {
        let report = upsert_reader(input, len, options, sink, placed, header)?;
        outcomes = report.outcomes;
        Ok(report.rewrite)
    })?;
    Ok(UpsertReport { rewrite, outcomes })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use std::io::Cursor;

    use super::{
        set_masters, top_group_rank, upsert_reader, HeaderSpec, Placed, Upsert, TOP_GROUP_ORDER,
    };
    use crate::cells::PlacementStyle;
    use crate::driver::{roundtrip_bytes, RewriteOptions, RoundtripOptions};
    use crate::fixture::{self, group, record, sub};
    use crate::tes4::Tes4;
    use crate::{
        Body, Entry, Error, GroupHeader, Plugin, Record, RecordHeader, Signature, Subrecord,
    };

    fn rec(sig: &[u8; 4], form_id: u32, edid: &[u8]) -> Record<'static> {
        let mut data = edid.to_vec();
        data.push(0);
        Record::new(
            RecordHeader {
                signature: Signature::new(sig),
                form_id,
                form_version: 582,
                ..RecordHeader::default()
            },
            Body::Raw(vec![Subrecord::new(Signature::new(b"EDID"), data)]),
        )
    }

    fn top_labels(plugin: &Plugin<'_>) -> Vec<String> {
        plugin
            .entries()
            .iter()
            .map(|e| match e {
                Entry::Group(g) => g.header.label_signature().to_string(),
                Entry::Record(r) => r.header.signature.to_string(),
            })
            .collect()
    }

    #[test]
    fn the_order_table_has_no_duplicates() {
        let mut seen = std::collections::BTreeSet::new();
        assert!(TOP_GROUP_ORDER.iter().all(|s| seen.insert(*s)));
        assert_eq!(TOP_GROUP_ORDER.len(), 181);
        assert_eq!(top_group_rank(Signature::new(b"GMST")), Some(0));
        assert!(top_group_rank(Signature::new(b"WEAP")) < top_group_rank(Signature::new(b"COBJ")));
        assert_eq!(top_group_rank(Signature::new(b"ZZZZ")), None);
    }

    #[test]
    fn a_built_plugin_parses_and_roundtrips() {
        let mut spec = HeaderSpec::new(vec![b"Starfield.esm".to_vec(), b"Other.esm".to_vec()]);
        spec.author = Some(b"kitcraft".to_vec());
        let mut plugin = Plugin::create(&spec);
        // Inserted out of order; the groups come out in table order, unknown types last.
        for (sig, id) in [
            (b"COBJ", 0x10),
            (b"ZZZZ", 0x11),
            (b"WEAP", 0x12),
            (b"GMST", 0x13),
        ] {
            let r = rec(sig, id, b"x");
            assert_eq!(
                plugin
                    .upsert(&[GroupHeader::top(r.header.signature)], r)
                    .unwrap(),
                Upsert::Inserted
            );
        }
        let again = rec(b"WEAP", 0x14, b"y");
        plugin
            .upsert(&[GroupHeader::top(Signature::new(b"WEAP"))], again)
            .unwrap();
        assert_eq!(top_labels(&plugin), ["GMST", "WEAP", "COBJ", "ZZZZ"]);
        let bytes = plugin.to_bytes().unwrap();
        let parsed = Plugin::parse(&bytes).unwrap();
        assert!(roundtrip_bytes(&bytes, &RoundtripOptions::with_decode())
            .unwrap()
            .identical());
        assert_eq!(parsed.to_bytes().unwrap(), bytes);
        let tes4 = parsed.tes4();
        assert_eq!(tes4.flags(), 0);
        assert!(!tes4.is_localized());
        let hedr = tes4.hedr().unwrap().unwrap();
        assert_eq!((hedr.version, hedr.next_object_id), (0.96, 0xFF00_1000));
        // Five records plus TES4 (the majority meaning).
        assert_eq!(hedr.record_count, 6);
        let masters: Vec<String> = tes4.masters().iter().map(|m| m.name_lossy()).collect();
        assert_eq!(masters, ["Starfield.esm", "Other.esm"]);
        assert!(tes4.masters().iter().all(|m| m.data == Some(0)));
        let sigs: Vec<String> = parsed
            .header()
            .subrecords()
            .unwrap()
            .iter()
            .map(|s| s.signature().to_string())
            .collect();
        assert_eq!(sigs, ["HEDR", "CNAM", "MAST", "DATA", "MAST", "DATA"]);
        assert_eq!(parsed.header().header.form_version, 582);
    }

    #[test]
    fn replacing_keeps_position_and_nested_groups_follow_their_parent() {
        let bytes = fixture::sample_plugin();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        // Replace the second GMST.
        let mut r = rec(b"GMST", 0x101, b"iBaz");
        r.header.form_version = 581;
        assert_eq!(
            plugin
                .upsert(&[GroupHeader::top(Signature::new(b"GMST"))], r)
                .unwrap(),
            Upsert::Replaced
        );
        let reparsed_bytes = plugin.to_bytes().unwrap();
        let reparsed = Plugin::parse(&reparsed_bytes).unwrap();
        let Entry::Group(g) = &reparsed.entries()[0] else {
            panic!()
        };
        let Entry::Record(second) = &g.children()[1] else {
            panic!()
        };
        assert_eq!(second.subrecords().unwrap()[0].data(), b"iBaz\0");
        // HEDR count verbatim: nothing was added.
        assert_eq!(reparsed.tes4().hedr().unwrap().unwrap().record_count, 42);

        // A REFR under a new cell's children: the children group is created right after
        // the new CELL record, the temporary group inside it.
        let cell_top = GroupHeader::top(Signature::new(b"CELL"));
        let mut plugin = Plugin::parse(&bytes).unwrap();
        plugin
            .upsert(&[cell_top], rec(b"CELL", 0x300, b"New"))
            .unwrap();
        let children = GroupHeader {
            label: 0x300u32.to_le_bytes(),
            group_type: 6,
            ..GroupHeader::default()
        };
        let temporary = GroupHeader {
            group_type: 9,
            ..children
        };
        assert_eq!(
            plugin
                .upsert(&[cell_top, children, temporary], rec(b"REFR", 0x301, b"r"))
                .unwrap(),
            Upsert::Inserted
        );
        let out = plugin.to_bytes().unwrap();
        let reparsed = Plugin::parse(&out).unwrap();
        let Entry::Group(cells) = &reparsed.entries()[1] else {
            panic!()
        };
        let kinds: Vec<String> = cells
            .children()
            .iter()
            .map(|e| match e {
                Entry::Group(g) => format!("G{}:{:X}", g.header.group_type, g.header.label_u32()),
                Entry::Record(r) => format!("{}:{:X}", r.header.signature, r.header.form_id),
            })
            .collect();
        assert_eq!(kinds, ["CELL:200", "G6:200", "CELL:300", "G6:300"]);
        // Two records added: the HEDR count follows the majority meaning.
        assert_eq!(reparsed.tes4().hedr().unwrap().unwrap().record_count, 10);

        // A children group for a record that is not there is refused, with nothing
        // changed; with the rebuild opt-in it goes to the end.
        let orphan = GroupHeader {
            label: 0x999u32.to_le_bytes(),
            group_type: 6,
            ..GroupHeader::default()
        };
        let before = plugin.to_bytes().unwrap();
        let err = plugin
            .upsert(&[cell_top, orphan], rec(b"REFR", 0x302, b"o"))
            .unwrap_err();
        assert!(err.to_string().contains("carry the WRLD/CELL"), "{err}");
        assert_eq!(plugin.to_bytes().unwrap(), before);
        plugin
            .upsert_before_owner(
                &[cell_top, orphan],
                rec(b"REFR", 0x302, b"o"),
                &PlacementStyle::OFFICIAL,
            )
            .unwrap();
        let Entry::Group(cells) = &plugin.entries()[1] else {
            panic!()
        };
        let Some(Entry::Group(last)) = cells.children().last() else {
            panic!()
        };
        assert_eq!(last.header.label_u32(), 0x999);
    }

    #[test]
    fn bad_paths_are_errors() {
        let mut plugin = Plugin::create(&HeaderSpec::new(Vec::new()));
        let r = rec(b"WEAP", 1, b"w");
        assert!(matches!(
            plugin.upsert(&[], r.clone()),
            Err(Error::Placement { .. })
        ));
        let nested = GroupHeader {
            group_type: 6,
            ..GroupHeader::default()
        };
        assert!(matches!(
            plugin.upsert(&[nested], r.clone()),
            Err(Error::Placement { .. })
        ));
        let top = GroupHeader::top(Signature::new(b"WEAP"));
        assert!(matches!(
            plugin.upsert(&[top, top], r.clone()),
            Err(Error::Placement { .. })
        ));
        let deep = vec![nested; 70];
        let mut path = vec![top];
        path.extend(deep);
        assert!(matches!(
            plugin.upsert(&path, r.clone()),
            Err(Error::TooDeep { .. })
        ));
        let mut grup = r;
        grup.header.signature = Signature::GRUP;
        assert!(matches!(
            plugin.upsert(&[top], grup),
            Err(Error::Placement { .. })
        ));
        assert!(plugin.entries().is_empty());
    }

    #[test]
    fn set_masters_replaces_the_list_in_place() {
        let bytes = fixture::plugin(&[]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        assert!(set_masters(plugin.header_mut(), &[b"A.esm".to_vec()]));
        let sigs: Vec<String> = plugin
            .header()
            .subrecords()
            .unwrap()
            .iter()
            .map(|s| s.signature().to_string())
            .collect();
        assert_eq!(sigs, ["HEDR", "CNAM", "MAST", "DATA", "ONAM"]);
        assert_eq!(
            Tes4::new(plugin.header()).masters()[0].name_lossy(),
            "A.esm"
        );
        // No masters before: after HEDR/CNAM/SNAM.
        let mut header = HeaderSpec::new(Vec::new()).to_record();
        header
            .subrecords_mut()
            .unwrap()
            .push(Subrecord::new(Signature::new(b"ONAM"), vec![0u8; 4]));
        assert!(set_masters(
            &mut header,
            &[b"B.esm".to_vec(), b"C.esm".to_vec()]
        ));
        let sigs: Vec<String> = header
            .subrecords()
            .unwrap()
            .iter()
            .map(|s| s.signature().to_string())
            .collect();
        assert_eq!(sigs, ["HEDR", "MAST", "DATA", "MAST", "DATA", "ONAM"]);
        assert!(set_masters(&mut header, &[]));
        assert_eq!(header.subrecords().unwrap().len(), 2);
    }

    #[test]
    fn streaming_upsert_matches_the_in_memory_one() {
        let mut bytes = fixture::sample_plugin();
        bytes.extend(group(
            b"ZZZZ",
            0,
            &record(b"ZZZZ", 0, 0x700, &sub(b"EDID", b"z\0")),
        ));
        let cell_top = GroupHeader::top(Signature::new(b"CELL"));
        let children = GroupHeader {
            label: 0x200u32.to_le_bytes(),
            group_type: 6,
            ..GroupHeader::default()
        };
        let temporary = GroupHeader {
            group_type: 9,
            ..children
        };
        let placed = vec![
            Placed::top_level(rec(b"WEAP", 0x800, b"gun")),
            Placed {
                path: vec![cell_top, children, temporary],
                record: rec(b"REFR", 0x202, b"replaced"),
            },
            Placed::top_level(rec(b"AAAA", 0x801, b"unknown type")),
            Placed::top_level(rec(b"KYWD", 0x802, b"kw")),
            Placed::top_level(rec(b"GMST", 0x803, b"g")),
            Placed::top_level(rec(b"WEAP", 0x804, b"gun2")),
        ];
        let mut expected = Plugin::parse(&bytes).unwrap();
        let mut in_memory = Vec::new();
        for p in placed.clone() {
            in_memory.push(expected.upsert(&p.path, p.record).unwrap());
        }
        crate::tes4::Tes4::set_record_count(expected.header_mut(), 0).unwrap();
        let mut sink = Cursor::new(Vec::new());
        let report = upsert_reader(
            Cursor::new(&bytes),
            bytes.len() as u64,
            &RewriteOptions::default(),
            &mut sink,
            placed,
            |h| crate::tes4::Tes4::set_record_count(h, 0).map(drop),
        )
        .unwrap();
        assert_eq!(report.outcomes, in_memory);
        assert_eq!(report.outcomes[1], Upsert::Replaced);
        let fix = report.rewrite.hedr_fix.unwrap();
        fix.apply(&mut sink).unwrap();
        let out = sink.into_inner();
        assert_eq!(out, expected.to_bytes().unwrap());
        let parsed = Plugin::parse(&out).unwrap();
        assert_eq!(
            top_labels(&parsed),
            ["GMST", "KYWD", "WEAP", "CELL", "NAVM", "ZZZZ", "AAAA"]
        );
        assert_eq!(parsed.tes4().hedr().unwrap().unwrap().record_count, 14);
    }

    #[test]
    fn upsert_to_a_file_applies_the_hedr_count_and_new_masters() {
        let dir = std::env::temp_dir().join(format!("kitcraft-esm-upsert-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.esp");
        let output = dir.join("out.esp");
        let bytes = fixture::sample_plugin();
        std::fs::write(&input, &bytes).unwrap();
        let masters = vec![b"Starfield.esm".to_vec(), b"New.esm".to_vec()];
        let report = super::upsert_file_to_path(
            &input,
            &output,
            &RewriteOptions::default(),
            vec![Placed::top_level(rec(b"WEAP", 0x0100_0800, b"gun"))],
            |h| {
                set_masters(h, &masters);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(report.outcomes, [Upsert::Inserted]);
        let out = std::fs::read(&output).unwrap();
        let parsed = Plugin::parse(&out).unwrap();
        let names: Vec<String> = parsed
            .tes4()
            .masters()
            .iter()
            .map(|m| m.name_lossy())
            .collect();
        assert_eq!(names, ["Starfield.esm", "New.esm"]);
        // TES4, 2 GMST, CELL, 3 REFR, NAVM and the new WEAP.
        assert_eq!(parsed.tes4().hedr().unwrap().unwrap().record_count, 9);
        assert_eq!(top_labels(&parsed), ["GMST", "WEAP", "CELL", "NAVM"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// One line per entry, indented by depth: `G<type>:<label>` or `<sig>:<FormID>`.
    fn outline(entries: &[Entry<'_>], depth: usize, out: &mut Vec<String>) {
        for e in entries {
            match e {
                Entry::Group(g) => {
                    let label = if g.header.group_type == 0 {
                        g.header.label_signature().to_string()
                    } else {
                        format!("{:X}", g.header.label_u32())
                    };
                    out.push(format!(
                        "{}G{}:{label}",
                        " ".repeat(depth),
                        g.header.group_type
                    ));
                    outline(g.children(), depth + 1, out);
                }
                Entry::Record(r) => out.push(format!(
                    "{}{}:{:X}",
                    " ".repeat(depth),
                    r.header.signature,
                    r.header.form_id
                )),
            }
        }
    }

    /// M4.A2: new groups and records go where the corpus rules put them, whatever the order
    /// of the upserts, and the two styles differ where the corpus files do.
    #[test]
    fn computed_paths_build_corpus_order_from_any_insertion_order() {
        use crate::cells::{placement, CellPlace, Grid, Parent, PlacementStyle};
        let world = 0x30u32;
        let ext = CellPlace::Exterior {
            world,
            grid: Grid::new(-1, 0),
        };
        let pers = CellPlace::Persistent { world };
        let flagged = |sig: &[u8; 4], flags: u32, id: u32| {
            let mut r = rec(sig, id, b"x");
            r.header.flags = flags;
            r
        };
        let records = vec![
            (flagged(b"WRLD", 0, world), Parent::None),
            (flagged(b"QUST", 0, 0x40), Parent::None),
            (
                flagged(b"CELL", 0, 0x0000_0012),
                Parent::CellAt(CellPlace::INTERIOR),
            ),
            (
                flagged(b"CELL", 0, 0x0000_0002),
                Parent::CellAt(CellPlace::INTERIOR),
            ),
            (
                flagged(b"CELL", 0, 0x0000_0011),
                Parent::CellAt(CellPlace::INTERIOR),
            ),
            (flagged(b"CELL", 0, 0x0000_0050), Parent::CellAt(ext)),
            (flagged(b"CELL", 0x400, 0x0000_0051), Parent::CellAt(pers)),
            (flagged(b"DIAL", 0, 0x60), Parent::Quest { quest: 0x40 }),
            (flagged(b"SCEN", 0, 0x58), Parent::Quest { quest: 0x40 }),
            (flagged(b"DLBR", 0, 0x70), Parent::Quest { quest: 0x40 }),
            (flagged(b"DIAL", 0, 0x5F), Parent::Quest { quest: 0x40 }),
            (
                flagged(b"INFO", 0, 0x63),
                Parent::Topic {
                    quest: 0x40,
                    topic: 0x60,
                },
            ),
            (
                flagged(b"INFO", 0, 0x62),
                Parent::Topic {
                    quest: 0x40,
                    topic: 0x60,
                },
            ),
            (
                flagged(b"REFR", 0, 0x80),
                Parent::Cell {
                    place: CellPlace::INTERIOR,
                    cell: 0x12,
                },
            ),
            (
                flagged(b"REFR", 0x400, 0x81),
                Parent::Cell {
                    place: CellPlace::INTERIOR,
                    cell: 0x12,
                },
            ),
            (
                flagged(b"NAVM", 0, 0x82),
                Parent::Cell {
                    place: CellPlace::INTERIOR,
                    cell: 0x12,
                },
            ),
            (
                flagged(b"REFR", 0x400, 0x83),
                Parent::Cell {
                    place: pers,
                    cell: 0x51,
                },
            ),
            (
                flagged(b"REFR", 0, 0x84),
                Parent::Cell {
                    place: ext,
                    cell: 0x50,
                },
            ),
        ];
        // Mode 0: as listed; 1: parents first, the rest reversed; 2: all reversed (children
        // before the records their groups belong to).
        let refusals = std::cell::Cell::new(0usize);
        let build = |style: &PlacementStyle, mode: u8| {
            let mut plugin = Plugin::create(&HeaderSpec::new(Vec::new()));
            let mut items: Vec<_> = records.clone();
            if mode == 2 {
                items.reverse();
            }
            if mode == 1 {
                // Parents (WRLD, QUST, CELL, DIAL) stay first; the rest go in reverse.
                let (parents, mut rest): (Vec<_>, Vec<_>) =
                    items.into_iter().partition(|(r, _)| {
                        matches!(&r.header.signature.0, b"WRLD" | b"QUST" | b"CELL" | b"DIAL")
                    });
                let mut parents = parents;
                parents.reverse();
                parents.sort_by_key(|(r, _)| !matches!(&r.header.signature.0, b"WRLD" | b"QUST"));
                rest.reverse();
                items = parents.into_iter().chain(rest).collect();
            }
            for (r, parent) in items {
                let path = placement(&r.header, &parent, style).unwrap();
                if mode == 2 {
                    // Children before their records: only the rebuild opt-in allows it.
                    let mut probe = plugin.clone();
                    let strict = probe.upsert_styled(&path, r.clone(), style);
                    plugin.upsert_before_owner(&path, r, style).unwrap();
                    refusals.set(refusals.get() + usize::from(strict.is_err()));
                } else {
                    plugin.upsert_styled(&path, r, style).unwrap();
                }
            }
            let mut out = Vec::new();
            outline(plugin.entries(), 0, &mut out);
            out
        };
        let official = build(&PlacementStyle::OFFICIAL, 0);
        assert_eq!(official, build(&PlacementStyle::OFFICIAL, 1));
        assert_eq!(official, build(&PlacementStyle::OFFICIAL, 2));
        // Without the opt-in, the first record that needs each missing children group is
        // refused: under the exterior world, the persistent cell, the interior cell and the
        // quest (the later ones find the groups the opt-in created).
        assert_eq!(refusals.get(), 4);
        let expected = [
            // 0x2 = 2: block 2, sub-block 0; 0x11 = 17: 7, 1; 0x12 = 18: 8, 1.
            "G0:CELL",
            " G2:2",
            "  G3:0",
            "   CELL:2",
            " G2:7",
            "  G3:1",
            "   CELL:11",
            " G2:8",
            "  G3:1",
            "   CELL:12",
            "   G6:12",
            "    G9:12",
            "     NAVM:82",
            "     REFR:80",
            "    G8:12",
            "     REFR:81",
            "G0:WRLD",
            " WRLD:30",
            " G1:30",
            "  CELL:51",
            "  G6:51",
            "   G8:51",
            "    REFR:83",
            "  G4:FFFF0000",
            "   G5:FFFF0000",
            "    CELL:50",
            "    G6:50",
            "     G9:50",
            "      REFR:84",
            "G0:NAVM",
            "G0:DIAL",
            "G0:INFO",
            "G0:QUST",
            " QUST:40",
            " G10:40",
            "  DLBR:70",
            "  DIAL:5F",
            "  DIAL:60",
            "  G7:60",
            "   INFO:62",
            "   INFO:63",
            "  SCEN:58",
            "G0:DLBR",
            "G0:SCEN",
        ];
        // The reversed build inserted INFO 63 after 62 and NAVM after the references:
        // both land where the corpus order puts them.
        assert_eq!(official, expected);
        let blueprint = build(&PlacementStyle::blueprint(7), 0);
        assert!(!blueprint.iter().any(|l| l == "G0:NAVM" || l == "G0:DLBR"));
        let g6 = blueprint.iter().position(|l| l == "   G6:12").unwrap();
        assert_eq!(
            &blueprint[g6 + 1..g6 + 6],
            [
                "    G8:12",
                "     REFR:81",
                "    G9:12",
                "     REFR:80",
                "     NAVM:82"
            ]
        );
        // The streaming upsert places the same way, placeholders included.
        let base = Plugin::create(&HeaderSpec::new(Vec::new()))
            .to_bytes()
            .unwrap();
        let placed: Vec<Placed> = records
            .iter()
            .map(|(r, parent)| Placed {
                path: placement(&r.header, parent, &PlacementStyle::OFFICIAL).unwrap(),
                record: r.clone(),
            })
            .collect();
        let mut sink = Cursor::new(Vec::new());
        let report = upsert_reader(
            Cursor::new(&base),
            base.len() as u64,
            &RewriteOptions::default(),
            &mut sink,
            placed,
            |_| Ok(()),
        )
        .unwrap();
        report.rewrite.hedr_fix.unwrap().apply(&mut sink).unwrap();
        let streamed = Plugin::parse(sink.get_ref()).unwrap();
        let mut out = Vec::new();
        outline(streamed.entries(), 0, &mut out);
        assert_eq!(out, expected);
    }

    fn flagged(sig: &[u8; 4], flags: u32, id: u32) -> Record<'static> {
        let mut r = rec(sig, id, b"x");
        r.header.flags = flags;
        r
    }

    /// Review finding (M4 placement core): a patch that places a reference into a master's
    /// cell without carrying the cell got a children group with no CELL before it. Now
    /// refused, in memory and streaming, with nothing written.
    #[test]
    fn children_groups_need_their_record_in_memory_and_streaming() {
        use crate::cells::{placement, CellPlace, Grid, Parent};
        let style = PlacementStyle::OFFICIAL;
        let place = CellPlace::Exterior {
            world: 0x25,
            grid: Grid::new(1, -1),
        };
        let refr = flagged(b"REFR", 0, 0x0100_1000);
        let parent = Parent::Cell {
            place,
            cell: 0x7777,
        };
        let path = placement(&refr.header, &parent, &style).unwrap();
        let mut plugin = Plugin::create(&HeaderSpec::new(vec![b"Starfield.esm".to_vec()]));
        let empty = plugin.to_bytes().unwrap();
        let err = plugin
            .upsert_styled(&path, refr.clone(), &style)
            .unwrap_err();
        assert!(matches!(err, Error::Placement { .. }), "{err}");
        assert_eq!(plugin.to_bytes().unwrap(), empty);
        // Streaming: refused too, for a new top-level group and for an existing one.
        let stream = |base: &[u8], placed: Vec<Placed>| {
            upsert_reader(
                Cursor::new(base),
                base.len() as u64,
                &RewriteOptions::default(),
                &mut Cursor::new(Vec::new()),
                placed,
                |_| Ok(()),
            )
        };
        let one = || {
            vec![Placed {
                path: path.clone(),
                record: refr.clone(),
            }]
        };
        assert!(matches!(
            stream(&empty, one()),
            Err(Error::Placement { .. })
        ));
        let wrld = flagged(b"WRLD", 0, 0x25);
        let mut with_world = plugin.clone();
        with_world
            .upsert(&[GroupHeader::top(Signature::new(b"WRLD"))], wrld.clone())
            .unwrap();
        let with_world = with_world.to_bytes().unwrap();
        assert!(matches!(
            stream(&with_world, one()),
            Err(Error::Placement { .. })
        ));
        // Carried first (WRLD, then the CELL): accepted, in either form.
        let xclc = [1i32.to_le_bytes(), (-1i32).to_le_bytes()].concat();
        let mut cell = flagged(b"CELL", 0, 0x7777);
        cell.subrecords_mut()
            .unwrap()
            .push(Subrecord::new(Signature::new(b"XCLC"), xclc));
        let cell_path = placement(&cell.header, &Parent::CellAt(place), &style).unwrap();
        let carried = vec![
            Placed::top_level(wrld),
            Placed {
                path: cell_path,
                record: cell,
            },
        ];
        let mut all = carried.clone();
        all.extend(one());
        let mut sink = Cursor::new(Vec::new());
        let report = upsert_reader(
            Cursor::new(&empty),
            empty.len() as u64,
            &RewriteOptions::default(),
            &mut sink,
            all,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(report.outcomes, [Upsert::Inserted; 3]);
        for p in carried {
            plugin.upsert_styled(&p.path, p.record, &style).unwrap();
        }
        assert_eq!(
            plugin.upsert_styled(&path, refr, &style).unwrap(),
            Upsert::Inserted
        );
        if let Some(fix) = report.rewrite.hedr_fix {
            fix.apply(&mut sink).unwrap();
        }
        assert_eq!(sink.into_inner(), plugin.to_bytes().unwrap());
    }

    /// Review finding: toggling a reference's persistence put a second copy into the cell's
    /// other children group. Now the old copy leaves (with its group when it was the last)
    /// and the outcome says so.
    #[test]
    fn a_persistence_toggle_moves_the_record() {
        use crate::cells::{placement, CellPlace, Parent};
        let style = PlacementStyle::OFFICIAL;
        let mut plugin = Plugin::create(&HeaderSpec::new(Vec::new()));
        let cell = flagged(b"CELL", 0, 0x12);
        let at = Parent::CellAt(CellPlace::INTERIOR);
        let path = placement(&cell.header, &at, &style).unwrap();
        plugin.upsert_styled(&path, cell, &style).unwrap();
        let parent = Parent::Cell {
            place: CellPlace::INTERIOR,
            cell: 0x12,
        };
        let mut put = |r: Record<'static>| {
            let path = placement(&r.header, &parent, &style).unwrap();
            plugin.upsert_styled(&path, r, &style).unwrap()
        };
        assert_eq!(put(flagged(b"REFR", 0, 0x80)), Upsert::Inserted);
        assert_eq!(put(flagged(b"REFR", 0, 0x81)), Upsert::Inserted);
        // 0x80 becomes persistent: out of type 9 (which keeps 0x81), into a new type 8.
        assert_eq!(put(flagged(b"REFR", 0x400, 0x80)), Upsert::Moved);
        // 0x81 too: type 9 is left empty and goes.
        assert_eq!(put(flagged(b"REFR", 0x400, 0x81)), Upsert::Moved);
        assert_eq!(put(flagged(b"REFR", 0x400, 0x81)), Upsert::Replaced);
        // And back.
        assert_eq!(put(flagged(b"REFR", 0, 0x80)), Upsert::Moved);
        let mut out = Vec::new();
        outline(plugin.entries(), 0, &mut out);
        assert_eq!(
            out[4..],
            [
                "   G6:12",
                "    G9:12",
                "     REFR:80",
                "    G8:12",
                "     REFR:81"
            ]
        );
        let bytes = plugin.to_bytes().unwrap();
        let parsed = Plugin::parse(&bytes).unwrap();
        // CELL and two references, plus TES4.
        assert_eq!(parsed.tes4().hedr().unwrap().unwrap().record_count, 4);
    }

    /// Review nits: the path is checked against the record's flag 0x400, and a world has
    /// one persistent cell.
    #[test]
    fn paths_that_disagree_with_the_flags_are_refused() {
        use crate::cells::group_type as gt;
        let mut plugin = Plugin::create(&HeaderSpec::new(Vec::new()));
        let wrld_top = GroupHeader::top(Signature::new(b"WRLD"));
        let cell_top = GroupHeader::top(Signature::new(b"CELL"));
        let g = |group_type: i32, label: u32| GroupHeader {
            label: label.to_le_bytes(),
            group_type,
            ..GroupHeader::default()
        };
        plugin
            .upsert(&[cell_top], flagged(b"CELL", 0, 0x12))
            .unwrap();
        plugin
            .upsert(&[wrld_top], flagged(b"WRLD", 0, 0x30))
            .unwrap();
        let g8 = [
            cell_top,
            g(gt::CELL_CHILDREN, 0x12),
            g(gt::CELL_PERSISTENT, 0x12),
        ];
        let g9 = [
            cell_top,
            g(gt::CELL_CHILDREN, 0x12),
            g(gt::CELL_TEMPORARY, 0x12),
        ];
        let g1 = [wrld_top, g(gt::WORLD_CHILDREN, 0x30)];
        let before = plugin.to_bytes().unwrap();
        for (path, r) in [
            (&g8[..], flagged(b"REFR", 0, 1)),
            (&g8[..], flagged(b"NAVM", 0x400, 2)),
            (&g9[..], flagged(b"REFR", 0x400, 3)),
            (&g1[..], flagged(b"CELL", 0, 4)),
        ] {
            let err = plugin.upsert(path, r).unwrap_err();
            assert!(matches!(err, Error::Placement { .. }), "{err}");
        }
        assert_eq!(plugin.to_bytes().unwrap(), before);
        assert!(plugin.upsert(&g8, flagged(b"REFR", 0x400, 1)).is_ok());
        assert!(plugin.upsert(&g9, flagged(b"NAVM", 0, 2)).is_ok());
        assert!(plugin.upsert(&g1, flagged(b"CELL", 0x400, 5)).is_ok());
        // The same persistent cell again is a replacement; a second one is refused.
        let again = plugin.upsert(&g1, flagged(b"CELL", 0x400, 5)).unwrap();
        assert_eq!(again, Upsert::Replaced);
        let err = plugin.upsert(&g1, flagged(b"CELL", 0x400, 6)).unwrap_err();
        assert!(err.to_string().contains("one persistent cell"), "{err}");
    }
}
