//! Editing records into patch plugins (lane E: M2.E2, M2.E4).
//!
//! - [`fields`]: field paths (`FULL`, `DNAM.Speed`, `FULL[2]`) and typed values.
//! - [`apply_field`]: one edit on one record, through the `kitkraft-esm` edit API (only the
//!   edited subrecord changes; a compressed record is recompressed only if it changed).

pub mod fields;
pub mod lstrings;
pub mod patch;
#[cfg(test)]
pub(crate) mod patch_tests;
pub(crate) mod scaffold;
pub mod source;
pub mod strings;
pub mod vmad;

use kitkraft_esm::{DecodeLimits, Record, Signature};
use serde_json::{json, Value};

pub use fields::{parse_op, parse_path, FieldError, FieldPath, Op, Step, ValueCtx};

/// What one edit did.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChange {
    /// The path as written.
    pub path: String,
    /// Index of the edited subrecord in the record.
    pub subrecord_index: usize,
    /// The value before (JSON view: number, text, `{"string_id"}`, `{"hex"}`).
    pub before: Value,
    /// The value after.
    pub after: Value,
    /// Whether the bytes changed.
    pub changed: bool,
    /// Limitations to report: the lstring fallback, and inline text whose encoding the
    /// game has not been seen to read (bytes >= 0x80).
    pub notes: Vec<String>,
    /// Whether the value written is an lstring (whole subrecord or a struct's tail).
    pub lstring: bool,
}

impl FieldChange {
    /// The JSON shape `set` reports.
    pub fn to_json(&self) -> Value {
        json!({
            "path": self.path,
            "before": self.before,
            "after": self.after,
            "changed": self.changed,
        })
    }
}

/// Applies `op` at `path` to `record` (see [`fields`] for paths and values). The record
/// must already be in the target's FormID space; `ctx` maps formrefs into it.
pub fn apply_field(
    record: &mut Record<'_>,
    path: &FieldPath,
    op: &Op,
    ctx: &ValueCtx<'_>,
    limits: &DecodeLimits,
) -> Result<FieldChange, FieldError> {
    let text = path.text.as_str();
    let payload_err = |e: kitkraft_esm::Error| FieldError::new(text, "payload", e.to_string());
    record.decompress(limits).map_err(payload_err)?;
    let sig = Signature(path.signature);
    let Some(index) = record.find_subrecord(sig, path.occurrence) else {
        let count = record.subrecords().map_or(0, |subs| {
            subs.iter().filter(|s| s.signature() == sig).count()
        });
        return Err(FieldError::new(
            text,
            "no-subrecord",
            format!(
                "the record has {count} {} subrecord(s); the writer adds a missing one at its schema slot first (`scaffold::add_slots`)",
                path.signature_str()
            ),
        ));
    };
    let payload = record
        .subrecords()
        .and_then(|subs| subs.get(index))
        .map(|s| s.data().to_vec())
        .unwrap_or_default();
    let sigs: Vec<[u8; 4]> = record
        .subrecords()
        .map(|subs| subs.iter().map(|s| s.signature().0).collect())
        .unwrap_or_default();
    let layout = fields::layout_at(record.header.signature.0, &sigs, index);
    let raw = kitkraft_schema::FieldType::Opaque { size: None };
    let (ty, mut notes) = match &layout {
        Some(layout) => (&layout.ty, layout.fallback.clone().into_iter().collect()),
        None if path.steps.is_empty() => (&raw, Vec::new()),
        None => {
            return Err(FieldError::new(
                text,
                "opaque",
                format!(
                    "the schema has no layout for {} in {}; set the whole subrecord as hex:",
                    path.signature_str(),
                    record.header.signature
                ),
            ))
        }
    };
    let edid_first = record
        .find_subrecord(Signature(*b"EDID"), 0)
        .and_then(|i| record.subrecords()?.get(i)?.data().first().copied());
    let ty = fields::select_variant(ty, edid_first);
    let at = fields::locate(text, ty, &payload, &path.steps, ctx.form_version)?;
    let end = at.start.saturating_add(at.len);
    let old = payload
        .get(at.start..end)
        .ok_or_else(|| FieldError::new(text, "payload", "value outside the payload"))?;
    let new = fields::encode_value(text, at.ty, old, op, ctx)?;
    let lstring = fields::is_lstring(at.ty);
    if lstring && !ctx.localized_target {
        notes.extend(high_bytes_note(text, &new));
    }
    let before = fields::display(at.ty, old, ctx.localized_source);
    let after = fields::display(at.ty, &new, ctx.localized_target);
    let mut next = payload.get(..at.start).unwrap_or_default().to_vec();
    next.extend_from_slice(&new);
    next.extend_from_slice(payload.get(end..).unwrap_or_default());
    // A whole array value: its count follows it (lane D round 5). A `count_from` member
    // before it in the same struct is rewritten here; the count subrecord before a whole
    // array subrecord (KSIZ before KWDA) after the write below.
    let count = matches!(op, Op::Set(_))
        .then(|| fields::element_count(at.ty, op, &new, ctx.form_version))
        .flatten();
    if let Some(n) = count {
        fields::recount_sibling(
            text,
            ty,
            &payload,
            &path.steps,
            ctx.form_version,
            n,
            &mut next,
        )?;
    }
    // The edited payload must still decode as the schema's layout (Fable review of lane D,
    // finding 1): a whole subrecord given as hex: always, any other edit when the payload
    // decoded before it (an array whose length is not a multiple of its element size, a
    // count that disagrees with its array). `validate`'s `layout.undecoded` flags the same.
    if let Some(layout) = layout.as_ref().filter(|l| l.fallback.is_none()) {
        let dctx = kitkraft_doc::typed::Ctx {
            form_version: ctx.form_version,
            localized: ctx.localized_target,
            edid_first,
        };
        let sig_text = path.signature_str();
        let whole_value = (path.steps.is_empty()
            && matches!(op, Op::Set(Value::String(t)) if t.starts_with("hex:")))
            || matches!(op, Op::Set(Value::Array(_) | Value::Object(_)));
        let checked = !layout.ty.is_opaque()
            && (whole_value || kitkraft_doc::typed::decode(ty, &payload, &dctx).is_ok());
        if checked {
            if let Some(why) = fields::undecoded(&sig_text, ty, &next, &dctx) {
                return Err(FieldError::new(text, "range", why));
            }
        }
    }
    let mut changed = record
        .set_subrecord(index, next, limits)
        .map_err(payload_err)?;
    if let (Some(n), true) = (count, path.steps.is_empty()) {
        changed |= recount_subrecord(record, index, path, n, limits)?;
    }
    Ok(FieldChange {
        path: path.text.clone(),
        subrecord_index: index,
        before,
        after,
        changed,
        notes,
        lstring,
    })
}

/// After the whole array subrecord at `index` was written with `count` elements, writes
/// `count` into the count subrecord just before it (`KSIZ` before `KWDA`,
/// [`scaffold::count_slot`]) when the record has one there; a record that keeps its array
/// without that count (PCBN and PCCN `KWDA`) is left as it is. Returns whether bytes
/// changed.
fn recount_subrecord(
    record: &mut Record<'_>,
    index: usize,
    path: &FieldPath,
    count: usize,
    limits: &DecodeLimits,
) -> Result<bool, FieldError> {
    let Some(before) = index.checked_sub(1) else {
        return Ok(false);
    };
    let order = scaffold::member_order(record.header.signature.0);
    let Some((sig, ty)) = scaffold::count_slot(&order, path.signature) else {
        return Ok(false);
    };
    let there = record
        .subrecords()
        .and_then(|s| s.get(before))
        .map(|s| s.signature().0);
    if there != Some(sig) {
        return Ok(false);
    }
    let shown = String::from_utf8_lossy(&sig).into_owned();
    let bytes = fields::count_bytes(&ty, count).ok_or_else(|| {
        FieldError::new(
            &path.text,
            "range",
            format!("{count} elements do not fit the count subrecord {shown}"),
        )
    })?;
    record
        .set_subrecord(before, bytes, limits)
        .map_err(|e| FieldError::new(&path.text, "payload", e.to_string()))
}

/// The limitation for inline lstring text with bytes >= 0x80 at `at` (`FULL[0]`): the
/// patch writes UTF-8, and whether the game reads inline text as UTF-8 or a code page is
/// not observed yet (`docs/corpus.md`, Writing rules; M2.O4, M2.O6).
pub fn high_bytes_note(at: &str, text: &[u8]) -> Option<String> {
    text.iter().any(|b| *b >= 0x80).then(|| {
        format!(
            "{at}: inline lstring text has bytes >= 0x80 (written as UTF-8); how the game decodes inline text is unverified in game (M2.O4, M2.O6), so non-ASCII may show wrong"
        )
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use kitkraft_esm::{Body, RecordHeader, Subrecord};

    fn weapon() -> Record<'static> {
        Record::new(
            RecordHeader {
                signature: Signature(*b"WEAP"),
                form_id: 0x0021_BBCD,
                form_version: 582,
                ..RecordHeader::default()
            },
            Body::Raw(vec![
                Subrecord::new(Signature(*b"EDID"), b"Rifle\0".to_vec()),
                Subrecord::new(Signature(*b"FULL"), 0x0002_E469u32.to_le_bytes().to_vec()),
                Subrecord::new(Signature(*b"DNAM"), vec![1, 2, 3]),
                Subrecord::new(Signature(*b"FULL"), 0x0002_EDD8u32.to_le_bytes().to_vec()),
            ]),
        )
    }

    fn ctx() -> ValueCtx<'static> {
        static MAP: fn(&str, u32) -> Result<u32, String> = |_, id| Ok(id);
        ValueCtx {
            localized_target: false,
            localized_source: true,
            form_version: 582,
            formref: &MAP,
        }
    }

    #[test]
    fn set_full_inline_and_keep_the_rest() {
        let mut r = weapon();
        let path = parse_path("FULL").unwrap();
        let change = apply_field(
            &mut r,
            &path,
            &Op::Set(json!("Old Soviet Marksman Rifle")),
            &ctx(),
            &DecodeLimits::default(),
        )
        .unwrap();
        assert!(change.changed);
        assert_eq!(change.subrecord_index, 1);
        assert_eq!(change.before, json!({"string_id": "0x0002E469"}));
        assert_eq!(change.after, json!("Old Soviet Marksman Rifle"));
        let subs = r.subrecords().unwrap();
        assert_eq!(subs[1].data(), b"Old Soviet Marksman Rifle\0");
        assert_eq!(subs[2].data(), [1, 2, 3]);
        assert_eq!(subs[3].data(), 0x0002_EDD8u32.to_le_bytes());

        let path = parse_path("FULL[1]").unwrap();
        let change = apply_field(
            &mut r,
            &path,
            &Op::Set(json!("x")),
            &ctx(),
            &DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(change.subrecord_index, 3);
    }

    /// Inline lstring text with bytes >= 0x80 carries a limitation naming the subrecord
    /// (Fable review round 2, finding 6); ASCII does not. Both report an lstring write.
    #[test]
    fn non_ascii_lstrings_are_noted() {
        let mut r = weapon();
        let limits = DecodeLimits::default();
        let path = parse_path("FULL[1]").unwrap();
        let change = apply_field(
            &mut r,
            &path,
            &Op::Set(json!("Fusil Améliorée")),
            &ctx(),
            &limits,
        )
        .unwrap();
        assert!(change.lstring);
        assert_eq!(change.notes.len(), 1, "{:?}", change.notes);
        assert!(change.notes[0].starts_with("FULL[1]: ") && change.notes[0].contains("0x80"));
        let change = apply_field(&mut r, &path, &Op::Set(json!("Plain")), &ctx(), &limits).unwrap();
        assert!(change.lstring && change.notes.is_empty());
        // The same through hex:.
        let change = apply_field(
            &mut r,
            &path,
            &Op::Set(json!("hex:C3A900")),
            &ctx(),
            &limits,
        )
        .unwrap();
        assert_eq!(change.notes.len(), 1);
        // Not an lstring: no note, not an lstring write.
        let change = apply_field(
            &mut r,
            &parse_path("DNAM").unwrap(),
            &Op::Set(json!("hex:FF")),
            &ctx(),
            &limits,
        )
        .unwrap();
        assert!(!change.lstring && change.notes.is_empty());
    }

    /// GMST `DATA` is a union chosen by the EditorID's first letter (lane B, M2.B9).
    #[test]
    fn union_layout_by_editor_id() {
        let gmst = |edid: &[u8], data: Vec<u8>| {
            Record::new(
                RecordHeader {
                    signature: Signature(*b"GMST"),
                    form_version: 582,
                    ..RecordHeader::default()
                },
                Body::Raw(vec![
                    Subrecord::new(Signature(*b"EDID"), edid.to_vec()),
                    Subrecord::new(Signature(*b"DATA"), data),
                ]),
            )
        };
        let limits = DecodeLimits::default();
        let path = parse_path("DATA").unwrap();
        let mut r = gmst(b"fSpeed\0", 2.0f32.to_le_bytes().to_vec());
        let change = apply_field(&mut r, &path, &Op::Mul(1.5), &ctx(), &limits).unwrap();
        assert_eq!((change.before, change.after), (json!(2.0), json!(3.0)));
        let mut r = gmst(b"iCount\0", 7i32.to_le_bytes().to_vec());
        apply_field(&mut r, &path, &Op::Set(json!("-1")), &ctx(), &limits).unwrap();
        assert_eq!(r.subrecords().unwrap()[1].data(), (-1i32).to_le_bytes());
        // No variant for this prefix: opaque, hex only.
        let mut r = gmst(b"qOdd\0", vec![0; 4]);
        let err = apply_field(&mut r, &path, &Op::Set(json!(1)), &ctx(), &limits).unwrap_err();
        assert_eq!(err.rule, "opaque");
    }

    fn activator(subs: Vec<Subrecord<'static>>) -> Record<'static> {
        Record::new(
            RecordHeader {
                signature: Signature(*b"ACTI"),
                form_id: 0x0003_CCF4,
                form_version: 582,
                ..RecordHeader::default()
            },
            Body::Raw(subs),
        )
    }

    fn sigs(r: &Record<'_>) -> Vec<String> {
        r.subrecords()
            .unwrap()
            .iter()
            .map(|s| s.signature().to_string())
            .collect()
    }

    /// Lane D round 5 (gaps 1 to 3): a record that lacks `KWDA` gains it with its `KSIZ`
    /// at their schema slots; a whole typed array (or hex:) keeps `KSIZ` equal to its
    /// element count; a wrong length is still refused by the layout check.
    #[test]
    fn keywords_are_added_and_counted() {
        let limits = DecodeLimits::default();
        let edid = Subrecord::new(Signature(*b"EDID"), b"Switch\0".to_vec());
        let pnam = Subrecord::new(Signature(*b"PNAM"), vec![0; 4]);
        let mut r = activator(vec![edid.clone(), pnam.clone()]);
        let edit = |p: &str, v: Value| patch::Edit {
            path: parse_path(p).unwrap(),
            op: Op::Set(v),
        };
        let kwda = edit(
            "KWDA",
            json!(["Starfield.esm:0234F1", "Starfield.esm:000801"]),
        );
        scaffold::add_slots(&mut r, std::slice::from_ref(&kwda), false).unwrap();
        assert_eq!(sigs(&r), ["EDID", "KSIZ", "KWDA", "PNAM"]);
        let change = apply_field(&mut r, &kwda.path, &kwda.op, &ctx(), &limits).unwrap();
        assert!(change.changed);
        assert_eq!(change.subrecord_index, 2);
        let subs = r.subrecords().unwrap();
        assert_eq!(subs[1].data(), 2u32.to_le_bytes());
        assert_eq!(subs[2].data(), [0xF1, 0x34, 0x02, 0, 0x01, 0x08, 0, 0]);
        // hex: of whole elements recounts too; a partial element is refused as before.
        let path = parse_path("KWDA").unwrap();
        apply_field(
            &mut r,
            &path,
            &Op::Set(json!("hex:F1340200")),
            &ctx(),
            &limits,
        )
        .unwrap();
        assert_eq!(r.subrecords().unwrap()[1].data(), 1u32.to_le_bytes());
        let err = apply_field(
            &mut r,
            &path,
            &Op::Set(json!("hex:F134020000")),
            &ctx(),
            &limits,
        )
        .unwrap_err();
        assert_eq!(err.rule, "range");
        assert!(err.message.contains("4-byte elements"), "{err}");
        assert_eq!(r.subrecords().unwrap()[1].data(), 1u32.to_le_bytes());

        // A record that has nothing to add is not touched; a signature the type has no
        // slot for is refused naming the field; an occurrence past the next is refused
        // rather than inventing the ones between.
        let mut plain = activator(vec![edid.clone(), pnam.clone()]);
        scaffold::add_slots(&mut plain, &[edit("PNAM", json!("00000001"))], false).unwrap();
        assert_eq!(sigs(&plain), ["EDID", "PNAM"]);
        let err =
            scaffold::add_slots(&mut plain, &[edit("XLCN", json!("00000001"))], false).unwrap_err();
        assert_eq!((err.path.as_str(), err.rule), ("XLCN", "no-subrecord"));
        assert!(err.message.contains("no slot"), "{err}");
        let err = scaffold::add_slots(&mut plain, &[edit("PNAM[2]", json!("00000001"))], false)
            .unwrap_err();
        assert_eq!((err.path.as_str(), err.rule), ("PNAM[2]", "no-subrecord"));
        // KWDA already present without KSIZ (as PCBN keeps it): no KSIZ is added, and the
        // count is left alone.
        let mut bare = activator(vec![
            edid,
            Subrecord::new(Signature(*b"KWDA"), vec![1, 0, 0, 0]),
            pnam,
        ]);
        scaffold::add_slots(&mut bare, std::slice::from_ref(&kwda), false).unwrap();
        assert_eq!(sigs(&bare), ["EDID", "KWDA", "PNAM"]);
    }

    /// Fable review of the wave 3 fix round, follow-up (b): `set` (and `clone`, through
    /// the same `add_slots`) giving a REFR's `MapMarker` lead `XMRK` and `FNAM` adds the
    /// `FULL`, `TNAM` and `VNAM` the group requires, zero-filled, and returns them; a
    /// record that already has the whole group gains nothing more.
    #[test]
    fn a_started_group_gets_its_required_members() {
        let edit = |p: &str, v: Value| patch::Edit {
            path: parse_path(p).unwrap(),
            op: Op::Set(v),
        };
        let name = Subrecord::new(Signature(*b"NAME"), vec![0; 4]);
        let data = Subrecord::new(Signature(*b"DATA"), vec![0; 24]);
        let refr = |subs: Vec<Subrecord<'static>>| {
            Record::new(
                RecordHeader {
                    signature: Signature(*b"REFR"),
                    form_id: 0x0000_0801,
                    form_version: 582,
                    ..RecordHeader::default()
                },
                Body::Raw(subs),
            )
        };
        let mut r = refr(vec![name.clone(), data.clone()]);
        let edits = [
            edit("XMRK", json!("hex:")),
            edit("FNAM", json!("hex:01000000")),
        ];
        let added = scaffold::add_slots(&mut r, &edits, false).unwrap();
        assert_eq!(
            added,
            [
                "FULL (MapMarker.Name)",
                "TNAM (MapMarker.MarkerType)",
                "VNAM (MapMarker.Category)"
            ]
        );
        let got = sigs(&r);
        let at = |s: &str| got.iter().position(|g| g == s).unwrap();
        assert!(
            at("XMRK") < at("FNAM")
                && at("FNAM") < at("FULL")
                && at("FULL") < at("TNAM")
                && at("TNAM") < at("VNAM"),
            "{got:?}"
        );
        let subs = r.subrecords().unwrap();
        assert_eq!(subs[at("TNAM")].data(), [0, 0]);
        assert_eq!(subs[at("VNAM")].data(), [0, 0]);
        // The whole group already there: an edit of its FNAM adds nothing.
        let mut whole = r.clone();
        let before = sigs(&whole);
        let added =
            scaffold::add_slots(&mut whole, &[edit("FNAM", json!("hex:02000000"))], false).unwrap();
        assert!(added.is_empty());
        assert_eq!(sigs(&whole), before);
    }

    #[test]
    fn errors_name_the_path_and_rule() {
        let mut r = weapon();
        let limits = DecodeLimits::default();
        let err = apply_field(
            &mut r,
            &parse_path("FULL[2]").unwrap(),
            &Op::Set(json!("x")),
            &ctx(),
            &limits,
        )
        .unwrap_err();
        assert_eq!((err.path.as_str(), err.rule), ("FULL[2]", "no-subrecord"));
        assert!(err.to_string().starts_with("FULL[2]: "), "{err}");
        // DNAM has no layout: hex only, and no member paths.
        let err = apply_field(
            &mut r,
            &parse_path("DNAM").unwrap(),
            &Op::Set(json!("x")),
            &ctx(),
            &limits,
        )
        .unwrap_err();
        assert_eq!(err.rule, "opaque");
        let err = apply_field(
            &mut r,
            &parse_path("DNAM.Speed").unwrap(),
            &Op::Set(json!(1)),
            &ctx(),
            &limits,
        )
        .unwrap_err();
        assert_eq!(err.rule, "opaque");
        let change = apply_field(
            &mut r,
            &parse_path("DNAM").unwrap(),
            &Op::Set(json!("hex:0909")),
            &ctx(),
            &limits,
        )
        .unwrap();
        assert_eq!(change.after, json!({"hex": "0909"}));
        assert_eq!(r.subrecords().unwrap()[2].data(), [9, 9]);
        // Setting what is already there changes nothing.
        let change = apply_field(
            &mut r,
            &parse_path("DNAM").unwrap(),
            &Op::Set(json!("hex:0909")),
            &ctx(),
            &limits,
        )
        .unwrap();
        assert!(!change.changed);
    }
}
