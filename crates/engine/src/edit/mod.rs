//! Editing records into patch plugins (lane E: M2.E2, M2.E4).
//!
//! - [`fields`]: field paths (`FULL`, `DNAM.Speed`, `FULL[2]`) and typed values.
//! - [`apply_field`]: one edit on one record, through the `kitcraft-esm` edit API (only the
//!   edited subrecord changes; a compressed record is recompressed only if it changed).

pub mod fields;

use kitcraft_esm::{DecodeLimits, Record, Signature};
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
    /// A limitation to report (the lstring fallback), if any.
    pub note: Option<String>,
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
    let payload_err = |e: kitcraft_esm::Error| FieldError::new(text, "payload", e.to_string());
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
                "the record has {count} {} subrecord(s); adding subrecords is not supported",
                path.signature_str()
            ),
        ));
    };
    let payload = record
        .subrecords()
        .and_then(|subs| subs.get(index))
        .map(|s| s.data().to_vec())
        .unwrap_or_default();
    let layout = fields::layout_for(text, record.header.signature.0, path.signature)?;
    let raw = kitcraft_schema::FieldType::Opaque { size: None };
    let (ty, note) = match &layout {
        Some(layout) => (&layout.ty, layout.fallback.clone()),
        None if path.steps.is_empty() => (&raw, None),
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
    let at = fields::locate(text, ty, &payload, &path.steps, ctx.form_version)?;
    let end = at.start.saturating_add(at.len);
    let old = payload
        .get(at.start..end)
        .ok_or_else(|| FieldError::new(text, "payload", "value outside the payload"))?;
    let new = fields::encode_value(text, at.ty, old, op, ctx)?;
    let before = fields::display(at.ty, old, ctx.localized_source);
    let after = fields::display(at.ty, &new, ctx.localized_target);
    let mut next = payload.get(..at.start).unwrap_or_default().to_vec();
    next.extend_from_slice(&new);
    next.extend_from_slice(payload.get(end..).unwrap_or_default());
    let changed = record
        .set_subrecord(index, next, limits)
        .map_err(payload_err)?;
    Ok(FieldChange {
        path: path.text.clone(),
        subrecord_index: index,
        before,
        after,
        changed,
        note,
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use kitcraft_esm::{Body, RecordHeader, Subrecord};

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
