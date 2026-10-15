//! Field-level comparison of two versions of one subrecord (M3.D2).
//!
//! Where lane B's schema gives a subrecord a layout, both payloads are decoded through it
//! ([`crate::typed`]) and compared member by member: FormIDs are first mapped through each
//! side's own masters (by plugin *name*, through the caller's resolver), so two plugins that
//! order their masters differently compare equal when they name the same forms, and differ
//! when the same stored bytes name different forms. Members the schema keeps opaque are
//! compared as bytes and reported as byte ranges of the payload. A subrecord without a layout
//! is left to the byte comparison in [`crate::compare`].
//!
//! Paths follow the field-path grammar of `set` and `where`: the signature, `[n]` for an
//! occurrence after the first, then the subrecord's member name and the members and array
//! indices inside it (`KWDA.Forms[1]`, `DATA.Value`, `DNAM[1].Speed`).

use crate::typed::{self, Decoded};

/// A FormID value after resolution, as compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormValue {
    /// `00000000`, the null reference.
    Null,
    /// A resolved form: `key` is what compares (independent of master order and of the
    /// case of plugin names), `label` what is shown (`Starfield.esm:0021BBCD`).
    Form {
        /// Comparison key.
        key: String,
        /// Display label.
        label: String,
    },
    /// A FormID that does not resolve on its side; compared by the stored value only.
    Unresolved {
        /// As stored.
        raw: u32,
        /// Why it does not resolve.
        error: String,
    },
}

/// One compared value.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// Integer (enums and flags included).
    Int(i128),
    /// Float.
    Float(f64),
    /// FormID.
    Form(FormValue),
    /// Four signature bytes.
    Sig([u8; 4]),
    /// Text (inline strings).
    Text(String),
    /// Localized string ID.
    LStringId(u32),
    /// Bytes the schema keeps opaque: offset in the subrecord payload, and length.
    Opaque {
        /// Offset in the payload.
        offset: u64,
        /// Length.
        length: u64,
    },
    /// A struct with no members.
    Empty,
}

/// One member that differs. `None` on a side means the member is absent there (an array
/// element or a versioned field present on one side only).
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChange {
    /// Member path (module docs).
    pub path: String,
    /// The value in the first version.
    pub before: Option<FieldValue>,
    /// The value in the second version.
    pub after: Option<FieldValue>,
}

/// How one version's payloads are read.
pub struct Side<'a> {
    /// Decoding context (form version, localized flag, EditorID's first byte).
    pub ctx: typed::Ctx,
    /// Resolves a stored FormID through this version's plugin's masters.
    pub resolve: &'a dyn Fn(u32) -> FormValue,
}

impl std::fmt::Debug for Side<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Side").field("ctx", &self.ctx).finish()
    }
}

/// What a typed comparison of one subrecord found.
#[derive(Debug, Clone, PartialEq)]
pub enum Typed {
    /// The schema has no layout for the subrecord: compare bytes.
    NoLayout,
    /// The layout exists, but a payload does not decode through it (the reason): compare
    /// bytes.
    Undecodable(String),
    /// Decoded on both sides: the member name and the members that differ (empty when the
    /// versions are equal by value).
    Compared {
        /// The subrecord's member name in the schema.
        member: String,
        /// Differences.
        changes: Vec<FieldChange>,
    },
}

/// Compare occurrence `occurrence` of subrecord `sub` between two versions of a record
/// (`a` before, `b` after), each read through its own layout: `layouts` are the
/// subrecord's layouts in the two versions, resolved by its position in each
/// ([`typed::layout_of`]); a subrecord at members with different layouts in the two
/// versions, or with no layout, is compared as bytes.
pub fn compare(
    layouts: (&typed::LayoutResult<'_>, &typed::LayoutResult<'_>),
    sub: [u8; 4],
    occurrence: usize,
    a: (&[u8], &Side<'_>),
    b: (&[u8], &Side<'_>),
) -> Typed {
    let (ty, member) = match layouts {
        (Ok(Some(la)), Ok(Some(lb))) if la.ty == lb.ty => (la.ty, la.name),
        (Ok(Some(la)), Ok(Some(lb))) => {
            return Typed::Undecodable(format!(
                "{} is `{}` in the first version and `{}` in the second, with different layouts",
                String::from_utf8_lossy(&sub),
                la.path,
                lb.path
            ))
        }
        (Err(e), _) | (_, Err(e)) => return Typed::Undecodable(e.to_string()),
        _ => return Typed::NoLayout,
    };
    let (da, db) = match (
        typed::decode(ty, a.0, &a.1.ctx),
        typed::decode(ty, b.0, &b.1.ctx),
    ) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) | (_, Err(e)) => return Typed::Undecodable(e),
    };
    let sig = String::from_utf8_lossy(&sub).into_owned();
    let path = if occurrence == 0 {
        format!("{sig}.{member}")
    } else {
        format!("{sig}[{occurrence}].{member}")
    };
    let mut walker = Walker {
        a_bytes: a.0,
        b_bytes: b.0,
        a: a.1,
        b: b.1,
        out: Vec::new(),
    };
    walker.walk(&da, &db, &path);
    Typed::Compared {
        member: member.to_string(),
        changes: walker.out,
    }
}

/// Every leaf of `d` with its path, FormIDs resolved through `side`.
fn leaves(d: &Decoded, path: &str, side: &Side<'_>, out: &mut Vec<(String, FieldValue)>) {
    match d {
        Decoded::Struct(fields) if !fields.is_empty() => {
            for (k, v) in fields {
                leaves(v, &format!("{path}.{k}"), side, out);
            }
        }
        Decoded::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                leaves(v, &format!("{path}[{i}]"), side, out);
            }
        }
        other => out.push((path.to_string(), leaf(other, side))),
    }
}

/// A leaf as a compared value.
fn leaf(d: &Decoded, side: &Side<'_>) -> FieldValue {
    match d {
        Decoded::Int(i) => FieldValue::Int(*i),
        Decoded::Float(f) => FieldValue::Float(*f),
        Decoded::Form(0) => FieldValue::Form(FormValue::Null),
        Decoded::Form(raw) => FieldValue::Form((side.resolve)(*raw)),
        Decoded::Sig(s) => FieldValue::Sig(*s),
        Decoded::Text(t) => FieldValue::Text(t.clone()),
        Decoded::LStringId(id) => FieldValue::LStringId(*id),
        Decoded::Opaque { at, len } => FieldValue::Opaque {
            offset: *at as u64,
            length: *len as u64,
        },
        Decoded::Struct(_) | Decoded::Array(_) => FieldValue::Empty,
    }
}

struct Walker<'w, 'r> {
    a_bytes: &'w [u8],
    b_bytes: &'w [u8],
    a: &'w Side<'r>,
    b: &'w Side<'r>,
    out: Vec<FieldChange>,
}

impl Walker<'_, '_> {
    fn one_sided(&mut self, d: &Decoded, path: &str, first: bool) {
        let mut found = Vec::new();
        leaves(d, path, if first { self.a } else { self.b }, &mut found);
        for (p, v) in found {
            self.out.push(if first {
                FieldChange {
                    path: p,
                    before: Some(v),
                    after: None,
                }
            } else {
                FieldChange {
                    path: p,
                    before: None,
                    after: Some(v),
                }
            });
        }
    }

    fn walk(&mut self, a: &Decoded, b: &Decoded, path: &str) {
        match (a, b) {
            (Decoded::Struct(fa), Decoded::Struct(fb)) => {
                for (k, va) in fa {
                    let p = format!("{path}.{k}");
                    match fb.iter().find(|(kb, _)| kb == k) {
                        Some((_, vb)) => self.walk(va, vb, &p),
                        None => self.one_sided(va, &p, true),
                    }
                }
                for (k, vb) in fb {
                    if !fa.iter().any(|(ka, _)| ka == k) {
                        self.one_sided(vb, &format!("{path}.{k}"), false);
                    }
                }
            }
            (Decoded::Array(ia), Decoded::Array(ib)) => {
                for i in 0..ia.len().max(ib.len()) {
                    let p = format!("{path}[{i}]");
                    match (ia.get(i), ib.get(i)) {
                        (Some(x), Some(y)) => self.walk(x, y, &p),
                        (Some(x), None) => self.one_sided(x, &p, true),
                        (None, Some(y)) => self.one_sided(y, &p, false),
                        (None, None) => {}
                    }
                }
            }
            (Decoded::Opaque { at: x, len: n }, Decoded::Opaque { at: y, len: m }) => {
                let bytes = |data: &[u8], at: usize, len: usize| {
                    at.checked_add(len)
                        .and_then(|end| data.get(at..end))
                        .map(<[u8]>::to_vec)
                };
                if bytes(self.a_bytes, *x, *n) != bytes(self.b_bytes, *y, *m) {
                    self.out.push(FieldChange {
                        path: path.to_string(),
                        before: Some(leaf(a, self.a)),
                        after: Some(leaf(b, self.b)),
                    });
                }
            }
            (Decoded::Struct(_) | Decoded::Array(_), _)
            | (_, Decoded::Struct(_) | Decoded::Array(_)) => {
                // Different shapes (another union variant): every leaf on each side.
                self.one_sided(a, path, true);
                self.one_sided(b, path, false);
            }
            _ => {
                let (x, y) = (leaf(a, self.a), leaf(b, self.b));
                if !same(&x, &y) {
                    self.out.push(FieldChange {
                        path: path.to_string(),
                        before: Some(x),
                        after: Some(y),
                    });
                }
            }
        }
    }
}

/// Equality as compared: floats by their bits (so a stored NaN equals itself), FormIDs by
/// key (unresolved ones by their stored value).
pub fn same(a: &FieldValue, b: &FieldValue) -> bool {
    match (a, b) {
        (FieldValue::Float(x), FieldValue::Float(y)) => x.to_bits() == y.to_bits(),
        (
            FieldValue::Form(FormValue::Form { key: x, .. }),
            FieldValue::Form(FormValue::Form { key: y, .. }),
        ) => x == y,
        (
            FieldValue::Form(FormValue::Unresolved { raw: x, .. }),
            FieldValue::Form(FormValue::Unresolved { raw: y, .. }),
        ) => x == y,
        _ => a == b,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;
    use kitkraft_schema::Schema;

    /// The layout of `sub` placed after an EditorID (and `KSIZ` before a `KWDA`).
    fn at(schema: &'static Schema, record: [u8; 4], sub: [u8; 4]) -> typed::LayoutResult<'static> {
        let subs: Vec<[u8; 4]> = if &sub == b"KWDA" {
            vec![*b"EDID", *b"KSIZ", sub]
        } else {
            vec![*b"EDID", sub]
        };
        typed::layout_at(schema, record, &subs, subs.len() - 1)
    }

    fn ctx() -> typed::Ctx {
        typed::Ctx {
            form_version: 582,
            localized: false,
            edid_first: Some(b'f'),
        }
    }

    /// Side `a` numbers master `X.esm` as 00 and `Y.esm` as 01; side `b` the other way.
    fn resolver(order: [&'static str; 2]) -> impl Fn(u32) -> FormValue {
        move |raw: u32| match order.get((raw >> 24) as usize) {
            Some(name) => FormValue::Form {
                key: format!("{}:{:06X}", name.to_ascii_lowercase(), raw & 0xFF_FFFF),
                label: format!("{name}:{:06X}", raw & 0xFF_FFFF),
            },
            None => FormValue::Unresolved {
                raw,
                error: "past the masters".into(),
            },
        }
    }

    fn kwda(ids: &[u32]) -> Vec<u8> {
        ids.iter().flat_map(|i| i.to_le_bytes()).collect()
    }

    #[test]
    fn formids_compare_by_name_and_changes_have_paths() {
        let schema = kitkraft_schema::builtin().unwrap();
        let (ra, rb) = (resolver(["X.esm", "Y.esm"]), resolver(["Y.esm", "X.esm"]));
        let a = Side {
            ctx: ctx(),
            resolve: &ra,
        };
        let b = Side {
            ctx: ctx(),
            resolve: &rb,
        };
        // X:800, Y:801 in a's numbering == the same forms in b's numbering.
        let pa = kwda(&[0x0000_0800, 0x0100_0801]);
        let pb = kwda(&[0x0100_0800, 0x0000_0801]);
        let r = compare(
            (
                &at(schema, *b"WEAP", *b"KWDA"),
                &at(schema, *b"WEAP", *b"KWDA"),
            ),
            *b"KWDA",
            0,
            (&pa, &a),
            (&pb, &b),
        );
        assert_eq!(
            r,
            Typed::Compared {
                member: "Forms".into(),
                changes: vec![]
            }
        );
        // The same bytes on both sides name different forms.
        let r = compare(
            (
                &at(schema, *b"WEAP", *b"KWDA"),
                &at(schema, *b"WEAP", *b"KWDA"),
            ),
            *b"KWDA",
            0,
            (&pa, &a),
            (&pa, &b),
        );
        let Typed::Compared { changes, .. } = r else {
            panic!("{r:?}")
        };
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].path, "KWDA.Forms[0]");
        // One element changed, one appended; occurrence 1 shows in the path.
        let pc = kwda(&[0x0100_0800, 0x0100_0802, 0]);
        let r = compare(
            (
                &at(schema, *b"WEAP", *b"KWDA"),
                &at(schema, *b"WEAP", *b"KWDA"),
            ),
            *b"KWDA",
            1,
            (&pa, &a),
            (&pc, &b),
        );
        let Typed::Compared { changes, .. } = r else {
            panic!("{r:?}")
        };
        let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, ["KWDA[1].Forms[1]", "KWDA[1].Forms[2]"]);
        assert_eq!(
            changes[0].before,
            Some(FieldValue::Form(FormValue::Form {
                key: "y.esm:000801".into(),
                label: "Y.esm:000801".into()
            }))
        );
        assert_eq!(changes[1].before, None);
        assert_eq!(changes[1].after, Some(FieldValue::Form(FormValue::Null)));
    }

    #[test]
    fn opaque_members_are_byte_ranges_and_bad_payloads_fall_back() {
        let schema = kitkraft_schema::builtin().unwrap();
        let r = resolver(["X.esm", "Y.esm"]);
        let side = Side {
            ctx: typed::Ctx {
                edid_first: Some(b'#'),
                ..ctx()
            },
            resolve: &r,
        };
        // GMST DATA with an EditorID prefix no variant claims: opaque, compared as bytes.
        let r1 = compare(
            (
                &at(schema, *b"GMST", *b"DATA"),
                &at(schema, *b"GMST", *b"DATA"),
            ),
            *b"DATA",
            0,
            (&[1, 2, 3, 4], &side),
            (&[1, 2, 3, 5], &side),
        );
        let Typed::Compared { changes, .. } = r1 else {
            panic!("{r1:?}")
        };
        assert_eq!(
            changes[0].after,
            Some(FieldValue::Opaque {
                offset: 0,
                length: 4
            })
        );
        let same_bytes = compare(
            (
                &at(schema, *b"GMST", *b"DATA"),
                &at(schema, *b"GMST", *b"DATA"),
            ),
            *b"DATA",
            0,
            (&[1, 2, 3, 4], &side),
            (&[1, 2, 3, 4], &side),
        );
        assert!(matches!(same_bytes, Typed::Compared { changes, .. } if changes.is_empty()));
        // KSIZ is a u32: 3 bytes do not decode. A signature without a layout.
        assert!(matches!(
            compare(
                (
                    &at(schema, *b"WEAP", *b"KSIZ"),
                    &at(schema, *b"WEAP", *b"KSIZ")
                ),
                *b"KSIZ",
                0,
                (&[1, 0, 0], &side),
                (&[1, 0, 0, 0], &side)
            ),
            Typed::Undecodable(_)
        ));
        assert_eq!(
            compare(
                (
                    &at(schema, *b"WEAP", *b"ZZZZ"),
                    &at(schema, *b"WEAP", *b"ZZZZ")
                ),
                *b"ZZZZ",
                0,
                (&[1], &side),
                (&[2], &side)
            ),
            Typed::NoLayout
        );
        // Floats compare by bits.
        assert!(same(
            &FieldValue::Float(f64::NAN),
            &FieldValue::Float(f64::NAN)
        ));
        assert!(!same(&FieldValue::Float(0.0), &FieldValue::Float(-0.0)));
    }
}
