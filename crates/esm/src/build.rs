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
//! [`crate::index::PluginIndex::group_headers`] gives the path of a record in the plugin
//! it comes from. A missing top-level group is created in the order of
//! [`TOP_GROUP_ORDER`]; a missing nested group whose label is the FormID of a record in
//! its parent (cell, world and topic children: UESP, Skyrim_Mod:Mod_File_Format, "Groups")
//! is created right after that record, any other at the end of its parent.
//!
//! Inside the innermost group a direct child record with the same FormID and signature is
//! replaced in place; otherwise the record is appended. Records are copied as given: their
//! FormIDs are stored values, file-relative to the masters of the plugin they are written
//! into, and nothing here remaps them or infers anything about a master.
//!
//! The `HEDR` record count follows [`crate::tes4`]: recomputed (majority meaning) when
//! records are added, kept when one is only replaced.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

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

fn check_path(path: &[GroupHeader], record: &Record<'_>) -> Result<()> {
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

/// Place `record` under the groups `rest` inside `children` (the children of the group
/// the path has reached).
fn place<'a>(children: &mut Vec<Entry<'a>>, rest: &[GroupHeader], record: Record<'a>) -> Upsert {
    let Some((next, deeper)) = rest.split_first() else {
        let same = children.iter().position(|e| match e {
            Entry::Record(r) => {
                r.header.form_id == record.header.form_id
                    && r.header.signature == record.header.signature
            }
            Entry::Group(_) => false,
        });
        return match same.and_then(|i| children.get_mut(i)) {
            Some(slot) => {
                *slot = Entry::Record(record);
                Upsert::Replaced
            }
            None => {
                children.push(Entry::Record(record));
                Upsert::Inserted
            }
        };
    };
    let found = children.iter().position(|e| match e {
        Entry::Group(g) => g.header.same_group(next),
        Entry::Record(_) => false,
    });
    let index = match found {
        Some(i) => i,
        None => {
            let parent = u32::from_le_bytes(next.label);
            let at = children
                .iter()
                .position(|e| matches!(e, Entry::Record(r) if r.header.form_id == parent))
                .map_or(children.len(), |i| i.saturating_add(1));
            children.insert(at, Entry::Group(Group::new(*next)));
            at
        }
    };
    match children.get_mut(index) {
        Some(Entry::Group(g)) => place(g.children_mut(), deeper, record),
        // `index` was found or inserted as a group just above.
        _ => Upsert::Inserted,
    }
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
        check_path(path, &record)?;
        let Some((top, rest)) = path.split_first() else {
            return Ok(Upsert::Inserted);
        };
        let entries = self.entries_mut();
        let found = entries.iter().position(|e| match e {
            Entry::Group(g) => g.header.same_group(top),
            Entry::Record(_) => false,
        });
        let index = match found {
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
        };
        match entries.get_mut(index) {
            Some(Entry::Group(g)) => Ok(place(g.children_mut(), rest, record)),
            _ => Err(Error::Internal {
                offset: 0,
                message: "top-level group vanished while placing a record",
            }),
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
    let existing = top_level_labels(&mut input, len)?;
    let mut outcomes: Vec<Option<Upsert>> = vec![None; placed.len()];
    // Records whose top-level group the plugin has wait for it in the stream; the others
    // go into new top-level groups now, written before the first entry ranked after them.
    let mut pending: Vec<Option<Placed>> = Vec::with_capacity(placed.len());
    let mut created: Vec<(usize, Group<'static>)> = Vec::new();
    for (p, outcome) in placed.into_iter().zip(outcomes.iter_mut()) {
        let Some(top) = p.path.first().copied() else {
            pending.push(None);
            continue;
        };
        if existing.iter().any(|g| g.same_group(&top)) {
            pending.push(Some(p));
            continue;
        }
        pending.push(None);
        let rest = p.path.get(1..).unwrap_or(&[]);
        let i = match created.iter().position(|(_, g)| g.header.same_group(&top)) {
            Some(i) => i,
            None => {
                let rank = top_group_rank(Signature(top.label)).unwrap_or(usize::MAX);
                created.push((rank, Group::new(top)));
                created.len().saturating_sub(1)
            }
        };
        if let Some((_, g)) = created.get_mut(i) {
            *outcome = Some(place(g.children_mut(), rest, p.record));
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
                        *outcome = Some(place(g.children_mut(), rest, p.record));
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

        // A children group for a record that is not there goes to the end.
        let orphan = GroupHeader {
            label: 0x999u32.to_le_bytes(),
            group_type: 6,
            ..GroupHeader::default()
        };
        plugin
            .upsert(&[cell_top, orphan], rec(b"REFR", 0x302, b"o"))
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
}
