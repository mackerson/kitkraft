//! Reference composition (M4.A5): a new placed reference (`REFR`, or `ACHR` for an NPC
//! base) from a base form, a position and a rotation.
//!
//! [`compose_reference`] builds the record from a [`RefSpec`]; [`ComposedRef::placed`]
//! computes where it goes ([`crate::cells::placement`]: a persistent reference into its
//! cell's persistent children, which for a world means the world's persistent cell) for
//! [`crate::Plugin::upsert_styled`] or the streaming upsert, which add the groups it needs
//! and update the `HEDR` record count. The FormID comes from [`crate::forms::NewForms`];
//! advancing the `HEDR` next object ID is the allocator's.
//!
//! # Layout and confidence
//!
//! The record is written on raw subrecords, in the order every reference of the corpus
//! has: `EDID` (when given) first, the `leading` subrecords, `NAME`, the `extra` ones,
//! `DATA`, the `trailing` ones (`tests/refs.rs`: in all 6,734,456 `REFR` and `ACHR` of the
//! 14 files `NAME` occurs once and `DATA` once unless the record is deleted, `EDID` is
//! first whenever present, and `DATA` is last but in 3). What each member means comes from
//! the schema when it defines `REFR`/`ACHR` (lane BW, from the pinned xEdit file: `NAME`
//! the base, `DATA` position X, Y, Z then rotation X, Y, Z), at the confidence it gives;
//! until then from the corpus alone:
//!
//! | Member | Without a schema definition |
//! | ------ | --------------------------- |
//! | `EDID` | `corpus` (a NUL-terminated string in every record type) |
//! | `NAME` | `corpus`: one 4-byte FormID; in `Starfield.esm` a record of the file that is not a reference, cell, world or navmesh, for all of its references but one (`00014893` names `000001FB`, which is no record) |
//! | `DATA.Position.X`, `DATA.Position.Y` | `corpus`: floats 0 and 1 put 99.98 % of exterior references in their cell (`crate::cells`) |
//! | `DATA.Position.Z`, `DATA.Rotation` | no source read by this lane: below `corpus` |
//! | `flag.persistent` (0x400) | `corpus`: set exactly on records in persistent children |
//! | `flag.initially_disabled` (0x800) | `uesp-skyrim` record flag only; the corpus cannot show its meaning: below `corpus` |
//! | `flag.raw` (any other header flag bit, as given) | below `corpus` |
//!
//! A member below `corpus` is refused unless the caller names it in `allow_reference`
//! (ADR 0005 Amendment 3 §4; the M3.E6 rule). [`ComposedRef::members`] lists every member
//! written with its confidence and source, for the caller's evidence.

use kitcraft_schema::{Confidence, FieldType, Member};

use crate::build::Placed;
use crate::cells::{self, CellPlace, PlacementStyle, FLAG_PERSISTENT};
use crate::model::{Body, Record, RecordHeader, Subrecord};
use crate::{Error, Result, Signature, FLAG_COMPRESSED};

/// Record flag 0x800, "initially disabled" on UESP's Skyrim page (a container source; the
/// corpus has it on 127,725 `REFR` and 2,956 `ACHR` but cannot show what it does).
pub const FLAG_INITIALLY_DISABLED: u32 = 0x0000_0800;

const EDID: Signature = Signature::new(b"EDID");
const NAME: Signature = Signature::new(b"NAME");
const DATA: Signature = Signature::new(b"DATA");

/// Header fields of a new record that carry no meaning KitCraft knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordStamps {
    pub form_version: u16,
    pub timestamp: u16,
    pub version_control: u16,
    pub unknown: u16,
}

impl Default for RecordStamps {
    /// Form version 582 (the newest in the corpus, as a new `TES4` gets), every other
    /// field 0. What the Creation Kit writes is fixture F20's question.
    fn default() -> RecordStamps {
        RecordStamps {
            form_version: 582,
            timestamp: 0,
            version_control: 0,
            unknown: 0,
        }
    }
}

/// What a new reference is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct RefSpec {
    /// `REFR`, or `ACHR` for an NPC base (the caller checks the base's type).
    pub signature: Signature,
    /// The reference's own FormID (stored form; from [`crate::forms::NewForms`]).
    pub form_id: u32,
    /// The base form, stored FormID in the target plugin's master space.
    pub base: u32,
    /// Position X, Y, Z.
    pub position: [f32; 3],
    /// Rotation X, Y, Z, as stored.
    pub rotation: [f32; 3],
    /// Persistent (record flag 0x400; placed in persistent children).
    pub persistent: bool,
    /// Initially disabled (record flag 0x800).
    pub initially_disabled: bool,
    /// Other record header flag bits, written as given (not compression).
    pub raw_flags: u32,
    /// EditorID, without its NUL.
    pub editor_id: Option<Vec<u8>>,
    /// Subrecords between `EDID` and `NAME` (scripts, component blocks), as encoded by the
    /// caller.
    pub leading: Vec<Subrecord<'static>>,
    /// Subrecords between `NAME` and `DATA` (scale, layer, links, ...).
    pub extra: Vec<Subrecord<'static>>,
    /// Subrecords after `DATA`.
    pub trailing: Vec<Subrecord<'static>>,
    pub stamps: RecordStamps,
}

impl RefSpec {
    /// A temporary, enabled reference with no EditorID or other members.
    pub fn new(
        signature: Signature,
        form_id: u32,
        base: u32,
        position: [f32; 3],
        rotation: [f32; 3],
    ) -> RefSpec {
        RefSpec {
            signature,
            form_id,
            base,
            position,
            rotation,
            persistent: false,
            initially_disabled: false,
            raw_flags: 0,
            editor_id: None,
            leading: Vec::new(),
            extra: Vec::new(),
            trailing: Vec::new(),
            stamps: RecordStamps::default(),
        }
    }
}

/// One member written into a composed reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberUse {
    /// `NAME`, `DATA.Position.Z`, `flag.persistent`, ...
    pub member: String,
    pub confidence: Confidence,
    /// Where the confidence comes from (a schema citation or this module's corpus test).
    pub source: String,
}

/// A composed reference.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposedRef {
    pub record: Record<'static>,
    /// Every member written, with its confidence.
    pub members: Vec<MemberUse>,
    /// What the caller should report: corpus defaults and allowed `reference` members.
    pub limitations: Vec<String>,
}

impl ComposedRef {
    /// The record with the path [`crate::cells::placement`] computes for it in cell `cell`
    /// at `place` (for a persistent reference in a world, `place` is the world's persistent
    /// cell: [`crate::cells::persistent_cell`]).
    pub fn placed(&self, place: CellPlace, cell: u32, style: &PlacementStyle) -> Result<Placed> {
        let path = cells::placement(
            &self.record.header,
            &cells::Parent::Cell { place, cell },
            style,
        )?;
        Ok(Placed {
            path,
            record: self.record.clone(),
        })
    }
}

const CORPUS_TEST: &str = "corpus: kitcraft-esm tests/refs.rs and tests/cells.rs";

/// The weakest confidence of a field type's fields (a struct's fields, recursively).
fn weakest(ty: &FieldType, own: Confidence) -> Confidence {
    match ty {
        FieldType::Struct { fields } => fields
            .iter()
            .map(|f| weakest(&f.ty, f.provenance.confidence))
            .fold(own, Confidence::min),
        _ => own,
    }
}

/// The schema's confidence and source for subrecord `member` of record type `signature`,
/// when the builtin schema defines it with a fixed payload of `size` bytes.
fn schema_member(
    signature: Signature,
    member: Signature,
    size: u32,
) -> Option<(Confidence, String)> {
    let schema = kitcraft_schema::builtin().ok()?;
    let record = schema.record(kitcraft_schema::Signature::from_bytes(signature.0)?)?;
    record.members.iter().find_map(|m| match m {
        Member::Subrecord(s) if s.signature.as_bytes() == member.0 => {
            (s.ty.fixed_size(None) == Some(size)).then(|| {
                (
                    weakest(&s.ty, s.provenance.confidence),
                    s.provenance.source.clone(),
                )
            })
        }
        _ => None,
    })
}

fn refused(reason: String) -> Error {
    Error::Refused { offset: 0, reason }
}

/// Compose the reference `spec` describes. Members below `corpus` confidence are refused
/// unless named in `allow_reference` (see the module documentation).
pub fn compose_reference(spec: &RefSpec, allow_reference: &[&str]) -> Result<ComposedRef> {
    if spec.signature.0 != *b"REFR" && spec.signature.0 != *b"ACHR" {
        return Err(refused(format!(
            "a placed reference is a REFR or an ACHR, not {}",
            spec.signature
        )));
    }
    let structural = FLAG_PERSISTENT | FLAG_INITIALLY_DISABLED | FLAG_COMPRESSED;
    if spec.raw_flags & structural != 0 {
        return Err(refused(
            "raw_flags may not carry 0x400 (persistent), 0x800 (initially disabled) or the \
             compressed flag; use the fields"
                .to_string(),
        ));
    }
    let mut members: Vec<MemberUse> = Vec::new();
    let mut add = |member: &str, confidence: Confidence, source: &str| {
        members.push(MemberUse {
            member: member.to_string(),
            confidence,
            source: source.to_string(),
        });
    };
    let corpus = Confidence::Corpus;
    let below = Confidence::Reference;
    if spec.editor_id.is_some() {
        add("EDID", corpus, CORPUS_TEST);
    }
    match schema_member(spec.signature, NAME, 4) {
        Some((c, source)) => add("NAME", c, &source),
        None => add("NAME", corpus, CORPUS_TEST),
    }
    match schema_member(spec.signature, DATA, 24) {
        Some((c, source)) => {
            for m in [
                "DATA.Position.X",
                "DATA.Position.Y",
                "DATA.Position.Z",
                "DATA.Rotation",
            ] {
                add(m, c, &source);
            }
        }
        None => {
            add("DATA.Position.X", corpus, CORPUS_TEST);
            add("DATA.Position.Y", corpus, CORPUS_TEST);
            add(
                "DATA.Position.Z",
                below,
                "no source read by lane A; the schema's REFR/ACHR DATA raises it",
            );
            add(
                "DATA.Rotation",
                below,
                "no source read by lane A; the schema's REFR/ACHR DATA raises it",
            );
        }
    }
    if spec.persistent {
        add("flag.persistent", corpus, CORPUS_TEST);
    }
    if spec.initially_disabled {
        add(
            "flag.initially_disabled",
            below,
            "uesp-skyrim: https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format (record flags)",
        );
    }
    if spec.raw_flags != 0 {
        add("flag.raw", below, "written as given");
    }
    let blocked: Vec<&str> = members
        .iter()
        .filter(|m| m.confidence < corpus && !allow_reference.contains(&m.member.as_str()))
        .map(|m| m.member.as_str())
        .collect();
    if !blocked.is_empty() {
        return Err(refused(format!(
            "members below corpus confidence: {} (name them in allow_reference to write \
             them anyway)",
            blocked.join(", ")
        )));
    }
    let mut limitations: Vec<String> = members
        .iter()
        .filter(|m| m.confidence < corpus)
        .map(|m| {
            format!(
                "{} written at {} confidence (allowed): {}",
                m.member, m.confidence, m.source
            )
        })
        .collect();
    if spec.stamps == RecordStamps::default() {
        limitations.push(
            "record header: form version 582 and timestamp, version control and unknown 0, \
             corpus defaults until the Creation Kit is observed (F20)"
                .to_string(),
        );
    }

    let mut subs: Vec<Subrecord<'static>> = Vec::new();
    if let Some(edid) = &spec.editor_id {
        let mut data = edid.clone();
        data.push(0);
        subs.push(Subrecord::new(EDID, data));
    }
    subs.extend(spec.leading.iter().cloned());
    subs.push(Subrecord::new(NAME, spec.base.to_le_bytes().to_vec()));
    subs.extend(spec.extra.iter().cloned());
    let mut data = Vec::with_capacity(24);
    for v in spec.position.iter().chain(&spec.rotation) {
        data.extend_from_slice(&v.to_le_bytes());
    }
    subs.push(Subrecord::new(DATA, data));
    subs.extend(spec.trailing.iter().cloned());
    let mut flags = spec.raw_flags;
    if spec.persistent {
        flags |= FLAG_PERSISTENT;
    }
    if spec.initially_disabled {
        flags |= FLAG_INITIALLY_DISABLED;
    }
    let record = Record::new(
        RecordHeader {
            signature: spec.signature,
            flags,
            form_id: spec.form_id,
            timestamp: spec.stamps.timestamp,
            version_control: spec.stamps.version_control,
            form_version: spec.stamps.form_version,
            unknown: spec.stamps.unknown,
        },
        Body::Raw(subs),
    );
    Ok(ComposedRef {
        record,
        members,
        limitations,
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{compose_reference, RecordStamps, RefSpec};
    use crate::build::HeaderSpec;
    use crate::cells::{CellPlace, Grid, PlacementStyle};
    use crate::forms::NewForms;
    use crate::{Entry, Error, Plugin, Signature, Subrecord};
    use kitcraft_schema::Confidence;

    fn spec() -> RefSpec {
        RefSpec::new(
            Signature::new(b"REFR"),
            0x0100_1000,
            0x0000_ABCD,
            [150.0, -20.0, 3.5],
            [0.0, 0.0, 1.5],
        )
    }

    const ALL: &[&str] = &["DATA.Position.Z", "DATA.Rotation"];

    #[test]
    fn composition_writes_corpus_order_and_refuses_unconfirmed_members() {
        // Without a schema REFR definition, Z and rotation are below corpus.
        let defined = crate::bridge::has_definition(Signature::new(b"REFR"));
        let refused = compose_reference(&spec(), &[]);
        if defined {
            assert!(refused.is_ok());
        } else {
            let err = refused.unwrap_err();
            assert!(matches!(err, Error::Refused { .. }));
            assert!(
                err.to_string().contains("DATA.Position.Z, DATA.Rotation"),
                "{err}"
            );
        }
        let mut s = spec();
        s.editor_id = Some(b"KC_Marker".to_vec());
        s.persistent = true;
        s.extra = vec![Subrecord::new(
            Signature::new(b"XSCL"),
            2.0f32.to_le_bytes().to_vec(),
        )];
        s.trailing = vec![Subrecord::new(Signature::new(b"MNAM"), vec![1])];
        let composed = compose_reference(&s, ALL).unwrap();
        let sigs: Vec<String> = composed
            .record
            .subrecords()
            .unwrap()
            .iter()
            .map(|s| s.signature().to_string())
            .collect();
        assert_eq!(sigs, ["EDID", "NAME", "XSCL", "DATA", "MNAM"]);
        let subs = composed.record.subrecords().unwrap();
        assert_eq!(subs[0].data(), b"KC_Marker\0");
        assert_eq!(subs[1].data(), 0x0000_ABCDu32.to_le_bytes());
        assert_eq!(&subs[3].data()[0..4], 150.0f32.to_le_bytes());
        assert_eq!(&subs[3].data()[20..24], 1.5f32.to_le_bytes());
        assert_eq!(composed.record.header.flags, 0x400);
        assert_eq!(composed.record.header.form_version, 582);
        assert!(composed
            .members
            .iter()
            .all(|m| m.confidence >= Confidence::Reference));
        if !defined {
            assert_eq!(composed.limitations.len(), 3, "{:?}", composed.limitations);
        }
        // Initially disabled and raw flags need their own allowance.
        let mut d = spec();
        d.initially_disabled = true;
        assert!(compose_reference(&d, ALL).is_err());
        let ok = compose_reference(
            &d,
            &[
                "DATA.Position.Z",
                "DATA.Rotation",
                "flag.initially_disabled",
            ],
        );
        assert_eq!(ok.unwrap().record.header.flags, 0x800);
        let mut r = spec();
        r.raw_flags = 0x400;
        assert!(compose_reference(&r, ALL).is_err());
        r.raw_flags = 0x0001_0000;
        assert!(compose_reference(&r, ALL).is_err());
        assert!(compose_reference(&r, &["DATA.Position.Z", "DATA.Rotation", "flag.raw"]).is_ok());
        let mut w = spec();
        w.signature = Signature::new(b"STAT");
        assert!(compose_reference(&w, ALL).is_err());
        let stamps = RecordStamps {
            form_version: 581,
            timestamp: 1,
            version_control: 2,
            unknown: 3,
        };
        let mut st = spec();
        st.stamps = stamps;
        let c = compose_reference(&st, ALL).unwrap();
        assert_eq!(
            (
                c.record.header.form_version,
                c.record.header.timestamp,
                c.record.header.unknown
            ),
            (581, 1, 3)
        );
    }

    #[test]
    fn a_composed_reference_lands_in_its_cell_with_the_hedr_updated() {
        let mut plugin = Plugin::create(&HeaderSpec::new(vec![b"Starfield.esm".to_vec()]));
        let mut forms = NewForms::for_plugin(&plugin).unwrap();
        let style = PlacementStyle::OFFICIAL;
        let world = 0x0000_0025;
        let place = CellPlace::Exterior {
            world,
            grid: Grid::of_position(150.0, -20.0).unwrap(),
        };
        let mut s = spec();
        s.form_id = forms.allocate().unwrap();
        let composed = compose_reference(&s, ALL).unwrap();
        let placed = composed.placed(place, 0x0000_7777, &style).unwrap();
        plugin
            .upsert_styled(&placed.path, placed.record, &style)
            .unwrap();
        // A persistent reference in an exterior cell is refused: it goes to the world's
        // persistent cell.
        let mut p = spec();
        p.persistent = true;
        p.form_id = forms.allocate().unwrap();
        let composed = compose_reference(&p, ALL).unwrap();
        assert!(composed.placed(place, 0x0000_7777, &style).is_err());
        let pcell = CellPlace::Persistent { world };
        let placed = composed.placed(pcell, 0x0000_0D62, &style).unwrap();
        plugin
            .upsert_styled(&placed.path, placed.record, &style)
            .unwrap();
        forms.write_hedr(plugin.header_mut()).unwrap();
        let bytes = plugin.to_bytes().unwrap();
        let parsed = Plugin::parse(&bytes).unwrap();
        let hedr = parsed.tes4().hedr().unwrap().unwrap();
        assert_eq!(hedr.record_count, 3);
        assert_eq!(hedr.next_object_id, 0xFF00_1002);
        let Entry::Group(wrld) = &parsed.entries()[0] else {
            panic!()
        };
        assert_eq!(wrld.header.label, *b"WRLD");
        // WRLD > G1 > [G6 (persistent cell) > G8 > REFR, G4 > G5 > G6 > G9 > REFR]
        let Entry::Group(g1) = &wrld.children()[0] else {
            panic!()
        };
        let kinds: Vec<i32> = g1
            .children()
            .iter()
            .map(|e| match e {
                Entry::Group(g) => g.header.group_type,
                Entry::Record(_) => -1,
            })
            .collect();
        assert_eq!(kinds, [6, 4]);
        assert!(crate::driver::roundtrip_bytes(
            &bytes,
            &crate::driver::RoundtripOptions::default()
        )
        .unwrap()
        .identical());
    }
}
