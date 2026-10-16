//! What the record schema says about a member path, for the `build` spec v2 (M3.E6) and
//! `overhaul.scaffold` (M3.E7): whether the path names a member of the record type, how far
//! the member is checked (its `confidence`, the weakest along the path), and which members
//! are typed numbers.
//!
//! The confidence gate follows ADR 0004 ("Owner approval 2026-10-14", clarification 3) and
//! ADR 0005 Amendment 3 §4: `reference` means documented in an allowed source, not
//! validated, so a write that depends on a `reference`-only member is refused unless the
//! spec entry says `allow_reference = true`.

use kitkraft_schema::{Confidence, Decider, FieldType, RecordDef, Schema, Signature};

use crate::edit::fields::{FieldPath, Step, LSTRING_FALLBACK};
use crate::edit::lstrings;

fn deref(ty: &FieldType) -> &FieldType {
    match ty {
        FieldType::Ref { ty, .. } => deref(ty),
        other => other,
    }
}

fn sig_text(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

/// The record definition of `record`, if the schema has one.
pub fn definition(schema: &Schema, record: [u8; 4]) -> Option<&RecordDef> {
    schema.record(Signature::from_bytes(record)?)
}

/// What a member path resolves to in one record type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The weakest confidence along the path (`None`: the record type or the subrecord has
    /// no definition, and the path is a whole subrecord the patch writer handles as
    /// `hex:` or as the lstring fallback).
    pub confidence: Option<Confidence>,
    /// The member as the schema names it (`GLOB.Value`, `WEAP.Keywords.Forms`).
    pub label: String,
}

/// Checks that `path` names a member of record type `record`, and returns its confidence.
///
/// A record type the schema does not define is not checked (the patch writer decides).
/// For a defined type, a subrecord signature the definition does not list is an unknown
/// member, except `FULL`/`DESC` and the corpus lstring table (the patch writer's
/// fallbacks); each step must name a struct member (ASCII case-insensitive) or index an
/// array.
pub fn resolve(schema: &Schema, record: [u8; 4], path: &FieldPath) -> Result<Resolved, String> {
    let rtext = sig_text(record);
    let stext = path.signature_str();
    let Some(def) = definition(schema, record) else {
        return Ok(Resolved {
            confidence: None,
            label: format!("{rtext}.{stext}"),
        });
    };
    // By signature alone: the record's own member, before one inside a component layout.
    let leaves: Vec<(String, &kitkraft_schema::SubrecordDef)> =
        Signature::from_bytes(path.signature)
            .map(|sig| def.leaves_for(sig))
            .unwrap_or_default();
    let Some((name, first)) = leaves.first() else {
        let fallback = LSTRING_FALLBACK.contains(&&path.signature)
            || lstrings::class(record, path.signature) == lstrings::Class::LString;
        if fallback && path.steps.is_empty() {
            return Ok(Resolved {
                confidence: None,
                label: format!("{rtext}.{stext}"),
            });
        }
        let mut known: Vec<String> = def
            .leaf_subrecords()
            .iter()
            .map(|(_, s)| s.signature.as_str().to_string())
            .collect();
        known.dedup();
        return Err(format!(
            "unknown member: {rtext} has no subrecord {stext} in the schema (subrecords: {})",
            known.join(", ")
        ));
    };
    let mut confidence = leaves
        .iter()
        .map(|(_, s)| s.provenance.confidence)
        .min()
        .unwrap_or(first.provenance.confidence);
    let mut label = format!("{rtext}.{name}");
    let mut ty = deref(&first.ty);
    for step in &path.steps {
        match (step, ty) {
            (Step::Member(m), FieldType::Struct { fields }) => {
                let field = fields
                    .iter()
                    .find(|f| f.name.eq_ignore_ascii_case(m))
                    .ok_or_else(|| {
                        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
                        format!(
                            "unknown member: {label} has no member `{m}` (members: {})",
                            names.join(", ")
                        )
                    })?;
                confidence = confidence.min(field.provenance.confidence);
                label = format!("{label}.{}", field.name);
                ty = deref(&field.ty);
            }
            (Step::Index(_), FieldType::Array { element, .. }) => ty = deref(element),
            (
                Step::Member(m),
                FieldType::Union {
                    decider: Decider::Field { .. },
                    variants,
                },
            ) => {
                // A struct field union (`Value1.Float`): the step names a variant; which
                // variant an entry holds is checked against its decider when written.
                let variant = variants
                    .iter()
                    .find(|v| v.name.eq_ignore_ascii_case(m))
                    .ok_or_else(|| {
                        let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
                        format!(
                            "unknown member: {label} has no variant `{m}` (variants: {})",
                            names.join(", ")
                        )
                    })?;
                label = format!("{label}.{}", variant.name);
                ty = deref(&variant.ty);
            }
            (Step::Member(m), FieldType::Union { variants, .. }) => {
                // A member inside a union variant: the variant applies per record (its
                // EditorID), so the path is accepted when some variant has the member.
                let found = variants.iter().find_map(|v| match deref(&v.ty) {
                    FieldType::Struct { fields } => {
                        fields.iter().find(|f| f.name.eq_ignore_ascii_case(m))
                    }
                    _ => None,
                });
                let field = found.ok_or_else(|| {
                    format!("unknown member: no variant of {label} has a member `{m}`")
                })?;
                confidence = confidence.min(field.provenance.confidence);
                label = format!("{label}.{}", field.name);
                ty = deref(&field.ty);
            }
            (Step::Member(m), other) => {
                return Err(format!(
                    "unknown member: {label} is a {} with no members, so `{m}` names nothing",
                    crate::edit::fields::type_name(other)
                ))
            }
            (Step::Index(i), other) => {
                return Err(format!(
                    "unknown member: {label} is a {}, not an array, so [{i}] names nothing",
                    crate::edit::fields::type_name(other)
                ))
            }
        }
    }
    Ok(Resolved {
        confidence: Some(confidence),
        label,
    })
}

/// A typed numeric member, for `overhaul.scaffold`'s statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Numeric {
    /// The path in the `where` and field-path vocabulary: `FLTV`, `KSIZ`, `DNAM.Speed`.
    pub path: String,
    /// The schema's name for it (`GLOB.Value`).
    pub label: String,
    /// `int`, `float`, or `union` (a union whose variant is a number on some records).
    pub kind: &'static str,
    /// Its confidence (the weakest along the path).
    pub confidence: Confidence,
    /// Inside an array: a field path must index the element, so the scaffold suggests no
    /// rule for it.
    pub in_array: bool,
}

/// Every typed numeric member of record type `record` (integers and floats; not enums,
/// flags or FormIDs), one per subrecord signature, in definition order. The component
/// layouts of the base form component block are the shared block's, not the record's
/// members, and are left out.
pub fn numeric_members(schema: &Schema, record: [u8; 4]) -> Vec<Numeric> {
    let Some(def) = definition(schema, record) else {
        return Vec::new();
    };
    let rtext = sig_text(record);
    let mut out = Vec::new();
    let mut seen: Vec<[u8; 4]> = Vec::new();
    for (name, s) in def.leaf_subrecords() {
        if kitkraft_schema::in_component_layout(&name) {
            continue;
        }
        let sig = s.signature.as_bytes();
        if seen.contains(&sig) {
            continue;
        }
        seen.push(sig);
        let mut walk = Walk {
            out: &mut out,
            label: format!("{rtext}.{name}"),
            path: s.signature.as_str().to_string(),
            confidence: s.provenance.confidence,
            in_array: false,
        };
        walk.visit(&s.ty);
    }
    out
}

struct Walk<'o> {
    out: &'o mut Vec<Numeric>,
    label: String,
    path: String,
    confidence: Confidence,
    in_array: bool,
}

impl Walk<'_> {
    fn visit(&mut self, ty: &FieldType) {
        match deref(ty) {
            FieldType::Scalar(s) => self.out.push(Numeric {
                path: self.path.clone(),
                label: self.label.clone(),
                kind: if s.is_integer() { "int" } else { "float" },
                confidence: self.confidence,
                in_array: self.in_array,
            }),
            FieldType::Struct { fields } => {
                for f in fields {
                    let mut inner = Walk {
                        out: &mut *self.out,
                        label: format!("{}.{}", self.label, f.name),
                        path: format!("{}.{}", self.path, f.name),
                        confidence: self.confidence.min(f.provenance.confidence),
                        in_array: self.in_array,
                    };
                    inner.visit(&f.ty);
                }
            }
            FieldType::Array { element, .. } => {
                let was = self.in_array;
                self.in_array = true;
                self.visit(element);
                self.in_array = was;
            }
            FieldType::Union {
                decider: Decider::Field { .. },
                variants,
            } => {
                // A struct field union: each numeric variant is its own member path.
                for v in variants {
                    let mut inner = Walk {
                        out: &mut *self.out,
                        label: format!("{}.{}", self.label, v.name),
                        path: format!("{}.{}", self.path, v.name),
                        confidence: self.confidence,
                        in_array: self.in_array,
                    };
                    inner.visit(&v.ty);
                }
            }
            FieldType::Union { variants, .. }
                if variants
                    .iter()
                    .any(|v| matches!(deref(&v.ty), FieldType::Scalar(_))) =>
            {
                self.out.push(Numeric {
                    path: self.path.clone(),
                    label: self.label.clone(),
                    kind: "union",
                    confidence: self.confidence,
                    in_array: self.in_array,
                });
            }
            _ => {}
        }
    }
}

/// A schema with one `reference`-only member (`TEST.DNAM.Speed`) next to `corpus` ones:
/// the builtin schema has no `reference` member until lane B round 3 lands.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) fn reference_schema() -> Schema {
    const PIN: &str = "9fb016884bec138ea6c7b872cec831537d464c3e";
    let text = format!(
            "signature = \"TEST\"\nname = \"Test\"\nsource = \"corpus: synthetic\"\nconfidence = \"corpus\"\n\
             [[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\n\
             [[member]]\nsig = \"DNAM\"\nname = \"Data\"\ntype = \"struct\"\n\
             field = [\
               {{ name = \"Damage\", type = \"f32\" }},\
               {{ name = \"Speed\", type = \"f32\", source = \"xedit: wbDefinitionsSF1.pas@{PIN}#L10-L12\", confidence = \"reference\" }},\
               {{ name = \"Count\", type = \"u16\" }},\
             ]\n\
             [[member]]\nsig = \"KWDA\"\nname = \"Forms\"\ntype = \"array\"\nelement = \"formid\"\ncount = \"rest\"\n"
        );
    kitkraft_schema::load_sources([("TEST.toml", text.as_str())]).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::fields::parse_path;

    fn path(text: &str) -> FieldPath {
        parse_path(text).unwrap()
    }

    #[test]
    fn confidence_is_the_weakest_along_the_path() {
        let s = reference_schema();
        let r = resolve(&s, *b"TEST", &path("DNAM.Speed")).unwrap();
        assert_eq!(r.confidence, Some(Confidence::Reference));
        assert_eq!(r.label, "TEST.Data.Speed");
        let r = resolve(&s, *b"TEST", &path("DNAM.damage")).unwrap();
        assert_eq!(r.confidence, Some(Confidence::Corpus));
        // The whole subrecord contains the reference member, but its own node is corpus.
        let r = resolve(&s, *b"TEST", &path("DNAM")).unwrap();
        assert_eq!(r.confidence, Some(Confidence::Corpus));
    }

    /// Lane B round 4: OMOD property values are field unions; a build path names the
    /// variant (`DATA.Properties[0].Value1.Int`), and the numeric listing has one entry per
    /// numeric variant.
    #[test]
    fn omod_property_variants_resolve() {
        let builtin = kitkraft_schema::builtin().unwrap();
        let r = resolve(builtin, *b"OMOD", &path("DATA.Properties[0].Value1.Int")).unwrap();
        assert_eq!(r.label, "OMOD.Data.Properties.Value1.Int");
        assert_eq!(r.confidence, Some(Confidence::Corpus));
        let e = resolve(builtin, *b"OMOD", &path("DATA.Properties[0].Value1.Nope")).unwrap_err();
        assert!(
            e.contains("no variant `Nope`") && e.contains("SoundLevel"),
            "{e}"
        );
        let paths: Vec<String> = numeric_members(builtin, *b"OMOD")
            .into_iter()
            .map(|m| m.path)
            .collect();
        for p in [
            "DATA.Properties.Value1.Int",
            "DATA.Properties.Value1.Float",
            "DATA.Properties.Value2.Float",
        ] {
            assert!(paths.iter().any(|x| x == p), "{p}: {paths:?}");
        }
    }

    #[test]
    fn unknown_members_are_named() {
        let s = reference_schema();
        let e = resolve(&s, *b"TEST", &path("DNAM.Nope")).unwrap_err();
        assert!(
            e.contains("no member `Nope`") && e.contains("Damage, Speed, Count"),
            "{e}"
        );
        let e = resolve(&s, *b"TEST", &path("XNAM")).unwrap_err();
        assert!(e.contains("no subrecord XNAM"), "{e}");
        let e = resolve(&s, *b"TEST", &path("EDID.Text")).unwrap_err();
        assert!(e.contains("with no members"), "{e}");
        // FULL falls back to an lstring (the patch writer's rule), unchecked.
        assert_eq!(
            resolve(&s, *b"TEST", &path("FULL")).unwrap().confidence,
            None
        );
        // A type without a definition is not checked.
        assert_eq!(
            resolve(&s, *b"NPC_", &path("ZZZZ.A")).unwrap().confidence,
            None
        );
    }

    #[test]
    fn numeric_members_of_the_builtin_and_a_struct() {
        let s = reference_schema();
        let n = numeric_members(&s, *b"TEST");
        let paths: Vec<(&str, &str, bool)> = n
            .iter()
            .map(|m| {
                (
                    m.path.as_str(),
                    m.kind,
                    m.confidence == Confidence::Reference,
                )
            })
            .collect();
        assert_eq!(
            paths,
            [
                ("DNAM.Damage", "float", false),
                ("DNAM.Speed", "float", true),
                ("DNAM.Count", "int", false)
            ]
        );
        let builtin = kitkraft_schema::builtin().unwrap();
        let glob: Vec<String> = numeric_members(builtin, *b"GLOB")
            .into_iter()
            .map(|m| m.path)
            .collect();
        assert_eq!(glob, ["FLTV"]);
        // GMST XALG is a plain u64 since M3.B6 (lane B round 5).
        let gmst: Vec<(String, &str)> = numeric_members(builtin, *b"GMST")
            .into_iter()
            .map(|m| (m.path, m.kind))
            .collect();
        assert_eq!(
            gmst,
            [("XALG".to_string(), "int"), ("DATA".to_string(), "union")]
        );
        let weap: Vec<String> = numeric_members(builtin, *b"WEAP")
            .into_iter()
            .map(|m| m.path)
            .collect();
        assert!(weap.contains(&"KSIZ".to_string()), "{weap:?}");
    }
}
