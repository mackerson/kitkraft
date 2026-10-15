//! Typed FormID links of one record version, and following them (M4.D1, extends M3.D4).
//!
//! A link is one FormID value in a member the schema types as `formid` (lane B's
//! definitions, read through [`crate::typed`]): its member path in the field-path grammar
//! of `set`, `where` and `diff` (`KWDA.Forms[2]`, `DNAM[1].Ammo`), the raw value as
//! stored, the record types the definition says it may name, where its four bytes are, and
//! what it resolves to in the load order. The raw value and the resolved identity are kept
//! apart (ADR 0004, owner clarification 5): a value that does not resolve (a master that is
//! not loaded, an index past the master list) is [`LinkStatus::Unresolvable`] with the
//! reason, never guessed.
//!
//! The values found are exactly the generated decoder's (`form_ids()` of
//! `kitkraft_esm::generated::records::Record`) over the same subrecords: both walk the
//! same definitions, and `tests/typed_corpus.rs` checks the two agree on every record of
//! every official file. This module walks the schema instead of the generated types to
//! get each value's full path (array indices included), its target types and its offset.
//!
//! Nothing is read from bytes the schema keeps raw: subrecords without a layout, subrecords
//! inside `any_until` bodies (component blocks), payloads that do not decode, and members
//! of type `opaque` are counted ([`RecordLinks::unexamined`], [`RecordLinks::opaque_members`])
//! so callers can say what was not scanned.

use kitkraft_schema::{select_variant, Decider, FieldType, Schema};

use crate::compare::spans;
use crate::form::{Basis, GlobalForm};
use crate::load_order::LoadOrder;
use crate::overrides::Version;
use crate::query::{FieldValue, Lookup, Path, Subject};
use crate::record::{LoadedRecord, RecordReader};
use crate::subject::VersionSubject;
use crate::typed::{self, Decoded};

/// The game master whose low object IDs are engine-reserved.
pub const RESERVED_MASTER: &str = "Starfield.esm";

/// Object IDs below this in [`RESERVED_MASTER`]'s index space that the master does not
/// define are engine-reserved default forms (the player reference 0x14, built-in actor
/// values 0x2D0..0x3AC, a story manager root node 0x5B, ...): `refs` reports a link to one
/// as [`LinkStatus::EngineReserved`], and `validate`'s `formid.dangling` counts it as
/// `reserved_form_ids` instead of reporting it. Corpus fact, grade D until a CK observation
/// confirms it (ADR 0005 Amendment 4, `docs/validate.md` "Engine-reserved object IDs"): no
/// official file defines these IDs, yet the 14 official files name them 1,410 times from 13
/// record types (M4.BX1). Low IDs that `Starfield.esm` does define are ordinary forms.
///
/// This is the one definition of the rule; `kitkraft_engine::validate` re-exports it.
pub const RESERVED_OBJECT_IDS: u32 = 0x800;

/// A FormID value that means "no form" in a typed member, like `00000000`: links and
/// `formid.dangling` do not resolve it (lane B round 4; upstream allows it where a member
/// may be unset, e.g. INFO TRDA's emotion keyword, which 53,844 response lines of
/// `Starfield.esm` hold).
pub const NONE_FORM_ID: u32 = 0xFFFF_FFFF;

/// What a link's raw value is in the load order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LinkStatus {
    /// Resolves to a form some plugin of the load order has a version of.
    Resolved,
    /// Resolves to a plugin and object ID, but no plugin of the load order has a version of
    /// that form (dangling), and it is not engine-reserved.
    Missing,
    /// Resolves to an object ID below [`RESERVED_OBJECT_IDS`] of [`RESERVED_MASTER`] that no
    /// plugin defines: an engine-reserved form.
    EngineReserved,
    /// `00000000`, the null reference.
    Null,
    /// `FFFFFFFF`, "none".
    NoneValue,
    /// Does not resolve: the reason is in [`Link::error`].
    Unresolvable,
}

impl LinkStatus {
    /// `resolved`, `missing`, `engine_reserved`, `null`, `none`, `unresolvable`.
    pub fn as_str(self) -> &'static str {
        match self {
            LinkStatus::Resolved => "resolved",
            LinkStatus::Missing => "missing",
            LinkStatus::EngineReserved => "engine_reserved",
            LinkStatus::Null => "null",
            LinkStatus::NoneValue => "none",
            LinkStatus::Unresolvable => "unresolvable",
        }
    }

    /// Whether the value names a form (resolved, missing or engine-reserved).
    pub fn names_form(self) -> bool {
        matches!(
            self,
            LinkStatus::Resolved | LinkStatus::Missing | LinkStatus::EngineReserved
        )
    }
}

/// One typed FormID value of a record version.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// Subrecord signature.
    pub subrecord: [u8; 4],
    /// Position among the record's subrecords.
    pub sub_index: usize,
    /// Position among the record's subrecords of this signature.
    pub occurrence: usize,
    /// Member path (`KWDA.Forms[2]`, `DNAM[1].Ammo`).
    pub path: String,
    /// Member names from the subrecord's member name down, without array indices
    /// (`["Forms"]`, `["Data", "PackIn"]`): what a `where` path names.
    pub chain: Vec<String>,
    /// The raw value as stored in the record.
    pub raw: u32,
    /// Record types the definition says it may name; empty means "not known yet".
    pub targets: Vec<[u8; 4]>,
    /// What the raw value is in the load order.
    pub status: LinkStatus,
    /// The resolved identity (resolved, missing and engine-reserved links).
    pub form: Option<GlobalForm>,
    /// How it resolved (an assumption is named, `docs/load-order.md`).
    pub basis: Option<Basis>,
    /// The record type of the target's winning version (resolved links).
    pub target_signature: Option<[u8; 4]>,
    /// Why an unresolvable value does not resolve.
    pub error: Option<String>,
    /// Absolute file offset of the four bytes, `None` inside a compressed record.
    pub file_offset: Option<u64>,
    /// Offset of the four bytes in the record body (inflated for a compressed record).
    pub body_offset: u64,
}

impl Link {
    /// Whether the target's record type is one the definition allows: `None` when the
    /// definition lists no targets or the target has no version.
    pub fn target_allowed(&self) -> Option<bool> {
        let sig = self.target_signature?;
        (!self.targets.is_empty()).then(|| self.targets.contains(&sig))
    }
}

/// Why a subrecord was not scanned for FormIDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Unexamined {
    /// The record type has no definition.
    NoDefinition,
    /// The definition does not name the subrecord.
    NoLayout,
    /// Inside an `any_until` body (a component block): raw by definition.
    Raw,
    /// The definition names the signature, but not at this position.
    Unplaced,
    /// The payload does not decode through its layout.
    Undecodable,
}

impl Unexamined {
    /// Short name.
    pub fn as_str(self) -> &'static str {
        match self {
            Unexamined::NoDefinition => "no_definition",
            Unexamined::NoLayout => "no_layout",
            Unexamined::Raw => "raw_component_block",
            Unexamined::Unplaced => "unplaced",
            Unexamined::Undecodable => "undecodable",
        }
    }
}

/// The links of one record version, and what was not scanned.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecordLinks {
    /// Typed FormID values, in subrecord order, then member order.
    pub links: Vec<Link>,
    /// Subrecords not scanned: `(signature, position, why)`.
    pub unexamined: Vec<([u8; 4], usize, Unexamined)>,
    /// Members of type `opaque` inside decoded subrecords (bytes not scanned).
    pub opaque_members: u64,
    /// The version is deleted (record flag 0x20): its payload is not the form's data, and
    /// nothing was scanned.
    pub deleted: bool,
}

/// Record flag 0x20.
const DELETED: u32 = 0x20;

/// One FormID leaf of a decoded payload.
struct Leaf {
    path: String,
    chain: Vec<String>,
    raw: u32,
    targets: Vec<[u8; 4]>,
}

/// Walks a decoded value with its type, collecting FormID leaves.
struct Walk<'c> {
    ctx: &'c typed::Ctx,
    leaves: Vec<Leaf>,
    opaque: u64,
}

impl Walk<'_> {
    fn walk(&mut self, ty: &FieldType, d: &Decoded, path: &str, chain: &mut Vec<String>) {
        match (ty, d) {
            (_, Decoded::Opaque { .. }) => self.opaque = self.opaque.saturating_add(1),
            (FieldType::Ref { ty, .. }, _) => self.walk(ty, d, path, chain),
            (FieldType::FormId { targets }, Decoded::Form(raw)) => self.leaves.push(Leaf {
                path: path.to_string(),
                chain: chain.clone(),
                raw: *raw,
                targets: targets.iter().map(|t| t.as_bytes()).collect(),
            }),
            (FieldType::Struct { fields }, Decoded::Struct(values)) => {
                for (name, v) in values {
                    if let Some(f) = fields.iter().find(|f| f.name == *name) {
                        chain.push(name.clone());
                        self.walk(&f.ty, v, &format!("{path}.{name}"), chain);
                        chain.pop();
                    }
                }
            }
            (FieldType::Array { element, .. }, Decoded::Array(items)) => {
                for (i, v) in items.iter().enumerate() {
                    self.walk(element, v, &format!("{path}[{i}]"), chain);
                }
            }
            (
                FieldType::Union {
                    decider: Decider::EditorIdPrefix,
                    variants,
                },
                _,
            ) => {
                if let Some(v) = self
                    .ctx
                    .edid_first
                    .and_then(|b| select_variant(variants, i64::from(b), None))
                {
                    self.walk(&v.ty, d, path, chain);
                }
            }
            (
                FieldType::Union {
                    decider: Decider::Field { .. },
                    variants,
                },
                Decoded::Struct(values),
            ) => {
                for (name, v) in values {
                    if let Some(variant) = variants.iter().find(|x| x.name == *name) {
                        chain.push(name.clone());
                        self.walk(&variant.ty, v, &format!("{path}.{name}"), chain);
                        chain.pop();
                    }
                }
            }
            _ => {}
        }
    }
}

/// Classify a raw value read from plugin `plugin`.
/// What [`classify`] says of a raw value: status, identity, basis, the target's record
/// type, and why it does not resolve.
type Classified = (
    LinkStatus,
    Option<GlobalForm>,
    Option<Basis>,
    Option<[u8; 4]>,
    Option<String>,
);

fn classify(lo: &LoadOrder, plugin: u16, raw: u32) -> Classified {
    if raw == 0 {
        return (LinkStatus::Null, None, None, None, None);
    }
    if raw == NONE_FORM_ID {
        return (LinkStatus::NoneValue, None, None, None, None);
    }
    let resolved = match lo.plugin(plugin).map(|p| &p.map) {
        Some(Ok(map)) => map.resolve_detailed(raw),
        Some(Err(e)) => Err(e.clone()),
        None => lo.resolve(plugin, raw).map(|g| (g, Basis::Direct)),
    };
    match resolved {
        Err(e) => (
            LinkStatus::Unresolvable,
            None,
            None,
            None,
            Some(e.to_string()),
        ),
        Ok((g, basis)) => match lo.forms.winner(g) {
            Some(w) => {
                let sig = lo.record(w.plugin, w.record).map(|l| l.signature);
                (LinkStatus::Resolved, Some(g), Some(basis), sig, None)
            }
            None if is_engine_reserved(lo, g) => {
                (LinkStatus::EngineReserved, Some(g), Some(basis), None, None)
            }
            None => (LinkStatus::Missing, Some(g), Some(basis), None, None),
        },
    }
}

/// Whether `form` is an engine-reserved form ([`RESERVED_OBJECT_IDS`]): callers ask only
/// for forms no plugin has a version of.
pub fn is_engine_reserved(lo: &LoadOrder, form: GlobalForm) -> bool {
    form.object_id < RESERVED_OBJECT_IDS
        && lo
            .plugin(form.plugin)
            .is_some_and(|p| p.name.eq_ignore_ascii_case(RESERVED_MASTER))
}

/// The typed FormID links of `rec`, a record of plugin `plugin`, read through `schema`.
/// `edid_first` is the first byte of the record's EditorID (a union decider).
pub fn record_links(
    lo: &LoadOrder,
    schema: &Schema,
    plugin: u16,
    rec: &LoadedRecord,
    edid_first: Option<u8>,
) -> RecordLinks {
    let mut out = RecordLinks::default();
    if rec.record.header.flags & DELETED != 0 {
        out.deleted = true;
        return out;
    }
    let localized = lo
        .plugin(plugin)
        .and_then(|p| p.header())
        .is_some_and(|h| h.flags.localized());
    let ctx = typed::Ctx {
        form_version: rec.record.header.form_version,
        localized,
        edid_first,
    };
    let record_sig = rec.record.header.signature.0;
    let sigs: Vec<[u8; 4]> = rec.subs.iter().map(|s| s.signature().0).collect();
    let Some(placed) = typed::placements(schema, record_sig, &sigs) else {
        out.unexamined.extend(
            sigs.iter()
                .enumerate()
                .map(|(i, s)| (*s, i, Unexamined::NoDefinition)),
        );
        return out;
    };
    let spans = spans(rec);
    let mut seen: Vec<([u8; 4], usize)> = Vec::new();
    for (index, s) in rec.subs.iter().enumerate() {
        let sig = s.signature().0;
        let occurrence = match seen.iter_mut().find(|(k, _)| *k == sig) {
            Some((_, n)) => {
                *n = n.saturating_add(1);
                *n
            }
            None => {
                seen.push((sig, 0));
                0
            }
        };
        let layout = match typed::layout_of(schema, record_sig, sig, index, placed.get(index)) {
            Ok(Some(l)) => l,
            Ok(None) => {
                out.unexamined.push((sig, index, Unexamined::NoLayout));
                continue;
            }
            Err(typed::LayoutError::Raw { .. }) => {
                out.unexamined.push((sig, index, Unexamined::Raw));
                continue;
            }
            Err(_) => {
                out.unexamined.push((sig, index, Unexamined::Unplaced));
                continue;
            }
        };
        let (decoded, offsets) = match typed::decode_traced(layout.ty, s.data(), &ctx) {
            Ok(d) => d,
            Err(_) => {
                out.unexamined.push((sig, index, Unexamined::Undecodable));
                continue;
            }
        };
        let text = String::from_utf8_lossy(&sig).into_owned();
        let base = if occurrence == 0 {
            format!("{text}.{}", layout.name)
        } else {
            format!("{text}[{occurrence}].{}", layout.name)
        };
        let mut walk = Walk {
            ctx: &ctx,
            leaves: Vec::new(),
            opaque: 0,
        };
        let mut chain = vec![layout.name.to_string()];
        walk.walk(layout.ty, &decoded, &base, &mut chain);
        out.opaque_members = out.opaque_members.saturating_add(walk.opaque);
        let span = spans.get(index);
        for (leaf, at) in walk.leaves.into_iter().zip(offsets) {
            let (status, form, basis, target_signature, error) = classify(lo, plugin, leaf.raw);
            let at = at as u64;
            out.links.push(Link {
                subrecord: sig,
                sub_index: index,
                occurrence,
                path: leaf.path,
                chain: leaf.chain,
                raw: leaf.raw,
                targets: leaf.targets,
                status,
                form,
                basis,
                target_signature,
                error,
                file_offset: span
                    .and_then(|s| s.file_offset)
                    .map(|o| o.saturating_add(at)),
                body_offset: span.map_or(0, |s| s.body_offset).saturating_add(at),
            });
        }
    }
    out
}

/// One subrecord decoded through the layout its position gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedSub<'s> {
    /// Position among the record's subrecords.
    pub index: usize,
    /// Signature.
    pub signature: [u8; 4],
    /// The layout (member name and member path, group names included: `Body.GalaxyData`).
    pub layout: typed::Layout<'s>,
    /// The value, or why the payload does not decode.
    pub value: Result<Decoded, String>,
}

/// Every subrecord of `rec` (a record of plugin `plugin`) that has a layout, decoded
/// through it; subrecords without one are left out ([`record_links`] counts them).
pub fn decode_record<'s>(
    lo: &LoadOrder,
    schema: &'s Schema,
    plugin: u16,
    rec: &LoadedRecord,
    edid_first: Option<u8>,
) -> Vec<DecodedSub<'s>> {
    let localized = lo
        .plugin(plugin)
        .and_then(|p| p.header())
        .is_some_and(|h| h.flags.localized());
    let ctx = typed::Ctx {
        form_version: rec.record.header.form_version,
        localized,
        edid_first,
    };
    let record_sig = rec.record.header.signature.0;
    let sigs: Vec<[u8; 4]> = rec.subs.iter().map(|s| s.signature().0).collect();
    let Some(placed) = typed::placements(schema, record_sig, &sigs) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, s) in rec.subs.iter().enumerate() {
        let sig = s.signature().0;
        if let Ok(Some(layout)) =
            typed::layout_of(schema, record_sig, sig, index, placed.get(index))
        {
            let value = typed::decode(layout.ty, s.data(), &ctx);
            out.push(DecodedSub {
                index,
                signature: sig,
                layout,
                value,
            });
        }
    }
    out
}

/// The links of a version, read through `reader`.
pub fn version_links(
    lo: &LoadOrder,
    reader: &mut RecordReader,
    schema: &Schema,
    version: Version,
) -> Result<(LoadedRecord, RecordLinks), String> {
    let loc = lo
        .record(version.plugin, version.record)
        .copied()
        .ok_or_else(|| "record not in the index".to_string())?;
    let rec = reader
        .read(lo, version.plugin, &loc)
        .map_err(|e| format!("record could not be read: {e}"))?;
    let edid_first = lo.edid(version.plugin, &loc).and_then(|e| e.bytes().next());
    let links = record_links(lo, schema, version.plugin, &rec, edid_first);
    Ok((rec, links))
}

/// Whether link `l` is what the `where` path `path` names: the same signature (and
/// occurrence, when the path picks one), and the path's members equal to the link's member
/// chain, with or without the subrecord's own member name first (`KSIZ.Count` = `KSIZ`).
pub fn path_names(path: &Path, l: &Link) -> bool {
    let Path::Field {
        signature,
        members,
        index,
    } = path
    else {
        return false;
    };
    if *signature != l.subrecord {
        return false;
    }
    if let Some(i) = index {
        if usize::try_from(*i).ok() != Some(l.occurrence) {
            return false;
        }
    }
    let same = |chain: &[String]| {
        chain.len() == members.len()
            && chain
                .iter()
                .zip(members)
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    };
    same(&l.chain) || l.chain.get(1..).is_some_and(same)
}

/// Why [`follow`] could not follow a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowError {
    /// The value is unknown (no version, opaque, undecodable, unresolvable): report it.
    Unknown(String),
    /// The path does not name FormIDs (a caller's error).
    NotForm(String),
}

/// Follow the typed FormID member `path` (a `where` field path: `KWDA`, `DNAM.Ammo`) of
/// `form`'s winning version: every link the path names, with its raw value and status.
/// A record without the member follows to nothing; a path that names something other than
/// FormIDs is [`FollowError::NotForm`]; a value that cannot be read, or a FormID that does
/// not resolve, is [`FollowError::Unknown`].
pub fn follow(
    lo: &LoadOrder,
    reader: &mut RecordReader,
    schema: &Schema,
    form: GlobalForm,
    path: &Path,
) -> Result<Vec<Link>, FollowError> {
    let Some(version) = lo.forms.winner(form).copied() else {
        return Err(FollowError::Unknown(format!(
            "{} has no version in the load order",
            lo.form_label(form)
        )));
    };
    let (_, links) = version_links(lo, reader, schema, version).map_err(FollowError::Unknown)?;
    if links.deleted {
        return Err(FollowError::Unknown(format!(
            "the winning version of {} is deleted (record flag 0x20): its payload is not evaluated",
            lo.form_label(form)
        )));
    }
    let named: Vec<Link> = links
        .links
        .into_iter()
        .filter(|l| path_names(path, l))
        .collect();
    if let Some(bad) = named.iter().find(|l| l.status == LinkStatus::Unresolvable) {
        return Err(FollowError::Unknown(format!(
            "{}: FormID {:08X}: {}",
            bad.path,
            bad.raw,
            bad.error.as_deref().unwrap_or("does not resolve")
        )));
    }
    if !named.is_empty() {
        return Ok(named);
    }
    // Nothing typed as a FormID at that path: say why, as `where` would.
    let mut subject = VersionSubject::new(lo, reader, version).with_schema(Some(schema));
    match subject.lookup(path) {
        Lookup::Absent => Ok(Vec::new()),
        Lookup::Unknown(why) => Err(FollowError::Unknown(why)),
        Lookup::Values(values) | Lookup::UnionValues(values) => {
            match values
                .iter()
                .find(|v| !matches!(v, FieldValue::Null | FieldValue::Form { .. }))
            {
                Some(other) => Err(FollowError::NotForm(format!(
                    "holds {other:?}, not a FormID: only typed FormID members can be followed"
                ))),
                None if values.iter().all(|v| matches!(v, FieldValue::Null)) => Ok(Vec::new()),
                None => Err(FollowError::Unknown(
                    "the path reads FormIDs the typed link walk did not place".to_string(),
                )),
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;
    use crate::load_order::{LoadOptions, PluginSpec};
    use crate::query::Query;
    use crate::testdata::{plugin_with, rec, temp_dir};

    /// Base.esm: a keyword, a weapon naming it twice, a missing form, the null FormID and
    /// a low object ID; Patch.esm (master Base.esm): a weapon naming Base's keyword and an
    /// index past its masters.
    fn order(tag: &str) -> LoadOrder {
        let dir = temp_dir(tag);
        let weap = |edid: &str, ids: &[u32]| -> Vec<(&'static str, Vec<u8>)> {
            let kwda: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
            vec![
                ("EDID", [edid.as_bytes(), &[0]].concat()),
                ("KSIZ", (ids.len() as u32).to_le_bytes().to_vec()),
                ("KWDA", kwda),
            ]
        };
        let base = plugin_with(
            0x1,
            &[],
            &[
                rec(b"KYWD", 0x800, &[("EDID", b"Kw\0".to_vec())]),
                rec(
                    b"WEAP",
                    0x801,
                    &weap("Rifle", &[0x800, 0x800, 0x999, 0, 0x14]),
                ),
            ],
        );
        let patch = plugin_with(
            0x1,
            &["Base.esm"],
            &[rec(
                b"WEAP",
                0x0100_0802,
                &weap("Pistol", &[0x800, 0x0500_0001]),
            )],
        );
        std::fs::write(dir.join("Base.esm"), base).unwrap();
        std::fs::write(dir.join("Patch.esm"), patch).unwrap();
        let specs = ["Base.esm", "Patch.esm"]
            .iter()
            .map(|n| PluginSpec {
                name: (*n).to_string(),
                path: dir.join(n),
            })
            .collect();
        LoadOrder::load(specs, &LoadOptions::default()).unwrap()
    }

    fn schema() -> &'static Schema {
        kitkraft_schema::builtin().unwrap()
    }

    #[test]
    fn links_keep_raw_values_and_mark_each_status() {
        let lo = order("status");
        let mut reader = RecordReader::new();
        let rifle = GlobalForm {
            plugin: 0,
            object_id: 0x801,
        };
        let v = *lo.forms.winner(rifle).unwrap();
        let (rec, links) = version_links(&lo, &mut reader, schema(), v).unwrap();
        let got: Vec<(&str, u32, &str)> = links
            .links
            .iter()
            .map(|l| (l.path.as_str(), l.raw, l.status.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("KWDA.Forms[0]", 0x800, "resolved"),
                ("KWDA.Forms[1]", 0x800, "resolved"),
                ("KWDA.Forms[2]", 0x999, "missing"),
                ("KWDA.Forms[3]", 0, "null"),
                ("KWDA.Forms[4]", 0x14, "missing"),
            ],
            "{links:?}"
        );
        let first = &links.links[0];
        assert_eq!(first.target_signature, Some(*b"KYWD"));
        assert_eq!(first.targets, vec![*b"KYWD"]);
        assert_eq!(first.target_allowed(), Some(true));
        assert_eq!(first.chain, ["Forms"]);
        // The offsets point at the stored bytes.
        let bytes = std::fs::read(lo.plugin(0).unwrap().path.clone()).unwrap();
        let at = links.links[2].file_offset.unwrap() as usize;
        assert_eq!(&bytes[at..at + 4], &0x999u32.to_le_bytes());
        assert!(!rec.compressed);
        // EDID and KSIZ hold no FormIDs and are not counted as unexamined.
        assert!(links.unexamined.is_empty(), "{:?}", links.unexamined);
    }

    #[test]
    fn unresolvable_values_keep_the_reason() {
        let lo = order("unresolvable");
        let mut reader = RecordReader::new();
        let pistol = GlobalForm {
            plugin: 1,
            object_id: 0x802,
        };
        let v = *lo.forms.winner(pistol).unwrap();
        let (_, links) = version_links(&lo, &mut reader, schema(), v).unwrap();
        assert_eq!(links.links[0].status, LinkStatus::Resolved);
        assert_eq!(
            links.links[0].form,
            Some(GlobalForm {
                plugin: 0,
                object_id: 0x800
            })
        );
        let bad = &links.links[1];
        assert_eq!(bad.status, LinkStatus::Unresolvable);
        assert_eq!(bad.raw, 0x0500_0001);
        assert!(bad.form.is_none());
        assert!(bad.error.as_deref().unwrap().contains("master"), "{bad:?}");
    }

    #[test]
    fn engine_reserved_needs_the_game_master() {
        let lo = order("reserved");
        // Base.esm is not Starfield.esm: 0x14 there is an ordinary missing form.
        assert!(!is_engine_reserved(
            &lo,
            GlobalForm {
                plugin: 0,
                object_id: 0x14
            }
        ));
    }

    #[test]
    fn follow_takes_where_paths() {
        let lo = order("follow");
        let mut reader = RecordReader::new();
        let rifle = GlobalForm {
            plugin: 0,
            object_id: 0x801,
        };
        let path = |text: &str| match Query::parse(&format!("{text} == 1")).unwrap().expr {
            crate::query::Expr::Cmp { path, .. } => path,
            other => panic!("{other:?}"),
        };
        let got = follow(&lo, &mut reader, schema(), rifle, &path("KWDA")).unwrap();
        assert_eq!(got.len(), 5);
        let same = follow(&lo, &mut reader, schema(), rifle, &path("KWDA.Forms")).unwrap();
        assert_eq!(same.len(), 5);
        assert_eq!(
            follow(&lo, &mut reader, schema(), rifle, &path("DNAM")).unwrap(),
            vec![]
        );
        assert!(matches!(
            follow(&lo, &mut reader, schema(), rifle, &path("EDID")),
            Err(FollowError::NotForm(_))
        ));
        let pistol = GlobalForm {
            plugin: 1,
            object_id: 0x802,
        };
        assert!(matches!(
            follow(&lo, &mut reader, schema(), pistol, &path("KWDA")),
            Err(FollowError::Unknown(_))
        ));
    }
}
