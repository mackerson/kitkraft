//! Which schema members behind a value have `reference` confidence (M3.D3).
//!
//! `reference` means documented in an allowed source (xEdit at the pin), not yet confirmed
//! by our own evidence (ADR 0004, owner approval, clarification 3). A value decoded through
//! such a member is shown, but results name the member in `limitations` so nobody takes it
//! for a checked fact.

use kitcraft_schema::{Confidence, FieldType, Schema, Signature};

/// Member paths (`FLTV.Value`, `DNAM.Data.Speed`) with `reference` confidence that values
/// read from subrecord `sub` of a record of type `record` pass through: the subrecord's own
/// member, then every field inside it when `members` is `None` (a whole decoded payload),
/// else the fields along `members` (a `where` path, lower-case; a leading subrecord member
/// name is skipped as `where` does). Sorted, without duplicates.
pub fn reference_members(
    schema: &Schema,
    record: [u8; 4],
    sub: [u8; 4],
    members: Option<&[String]>,
) -> Vec<String> {
    let (Some(rs), Some(ss)) = (Signature::from_bytes(record), Signature::from_bytes(sub)) else {
        return Vec::new();
    };
    let Some(def) = schema.record(rs) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_, s) in def.leaf_subrecords() {
        if s.signature != ss {
            continue;
        }
        let base = format!("{}.{}", ss.as_str(), s.name);
        if s.provenance.confidence == Confidence::Reference {
            out.push(base.clone());
        }
        match members {
            None => all_fields(&s.ty, &base, &mut out, 0),
            Some(path) => {
                let path = match path.split_first() {
                    Some((first, rest)) if first.eq_ignore_ascii_case(&s.name) => rest,
                    _ => path,
                };
                along(&s.ty, path, &base, &mut out, 0);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Deepest type nesting followed (the schema loader bounds nesting too).
const MAX_DEPTH: usize = 64;

fn all_fields(ty: &FieldType, path: &str, out: &mut Vec<String>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let next = depth.saturating_add(1);
    match ty {
        FieldType::Struct { fields } => {
            for f in fields {
                let p = format!("{path}.{}", f.name);
                if f.provenance.confidence == Confidence::Reference {
                    out.push(p.clone());
                }
                all_fields(&f.ty, &p, out, next);
            }
        }
        FieldType::Array { element, .. } => all_fields(element, path, out, next),
        FieldType::Ref { ty, .. } => all_fields(ty, path, out, next),
        FieldType::Union { variants, .. } => {
            for v in variants {
                all_fields(&v.ty, path, out, next);
            }
        }
        _ => {}
    }
}

fn along(ty: &FieldType, members: &[String], path: &str, out: &mut Vec<String>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let next = depth.saturating_add(1);
    match ty {
        FieldType::Array { element, .. } => along(element, members, path, out, next),
        FieldType::Ref { ty, .. } => along(ty, members, path, out, next),
        FieldType::Union { variants, .. } => {
            for v in variants {
                along(&v.ty, members, path, out, next);
            }
        }
        FieldType::Struct { fields } => {
            let Some((first, rest)) = members.split_first() else {
                return;
            };
            if let Some(f) = fields.iter().find(|f| f.name.eq_ignore_ascii_case(first)) {
                let p = format!("{path}.{}", f.name);
                if f.provenance.confidence == Confidence::Reference {
                    out.push(p.clone());
                }
                along(&f.ty, rest, &p, out, next);
            }
        }
        _ => {}
    }
}

/// The limitation stating `members` (`(record type, member paths)`), or `None` when empty.
pub fn reference_limitation(members: &[(String, Vec<String>)]) -> Option<String> {
    let listed: Vec<String> = members
        .iter()
        .filter(|(_, m)| !m.is_empty())
        .map(|(record, m)| format!("{record} {}", m.join(", ")))
        .collect();
    (!listed.is_empty()).then(|| {
        format!(
            "values decoded through members with `reference` confidence (documented in the \
             pinned xEdit definitions, not yet confirmed by the corpus or the CK; ADR 0004): {}",
            listed.join("; ")
        )
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod tests {
    use super::*;

    /// The builtin definitions with GLOB's `FLTV.Value` sourced from the pinned xEdit file
    /// at `reference` confidence (no builtin member is `reference` yet).
    pub(crate) fn schema_with_reference_member() -> Schema {
        let sources = kitcraft_schema::BUILTIN_DEFINITIONS.iter().map(|(file, text)| {
            if *file != "GLOB.toml" {
                return (file.to_string(), text.to_string());
            }
            let cut = text.find("[[member]]\nsig = \"FLTV\"").unwrap();
            let glob = format!(
                "{}[[member]]\nsig = \"FLTV\"\nname = \"Value\"\ntype = \"f32\"\nsource = \"xedit: wbDefinitionsSF1.pas@{}#L1-L2\"\nconfidence = \"reference\"\n",
                &text[..cut],
                kitcraft_schema::XEDIT_REVISION
            );
            (file.to_string(), glob)
        });
        let owned: Vec<(String, String)> = sources.collect();
        kitcraft_schema::load_sources(owned.iter().map(|(f, t)| (f.as_str(), t.as_str()))).unwrap()
    }

    #[test]
    fn reference_members_are_found_whole_and_along_paths() {
        let schema = schema_with_reference_member();
        assert_eq!(
            reference_members(&schema, *b"GLOB", *b"FLTV", None),
            ["FLTV.Value"]
        );
        assert_eq!(
            reference_members(&schema, *b"GLOB", *b"FLTV", Some(&["value".into()])),
            ["FLTV.Value"]
        );
        assert!(reference_members(&schema, *b"GLOB", *b"EDID", None).is_empty());
        assert!(reference_members(&schema, *b"QQQQ", *b"FLTV", None).is_empty());
        let builtin = kitcraft_schema::builtin().unwrap();
        assert!(reference_members(builtin, *b"GLOB", *b"FLTV", None).is_empty());
        let text = reference_limitation(&[("GLOB".into(), vec!["FLTV.Value".into()])]).unwrap();
        assert!(text.contains("GLOB FLTV.Value"), "{text}");
        assert_eq!(reference_limitation(&[("GLOB".into(), vec![])]), None);
    }
}
