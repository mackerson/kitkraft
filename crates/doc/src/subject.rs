//! A version of a form in a load order, as a `where` [`Subject`].
//!
//! Index-only paths (`@form`, `@plugin`, `@edid`, `@flags`) never read the file; field paths
//! read the record once. What can be decoded today: `EDID` (zstring) and the lstrings
//! `FULL`/`DESC` of non-localized plugins. Everything else is unknown until typed layouts are
//! wired (M2.A3, M2.B2), and a localized lstring is unknown until string tables are loaded
//! (M2.D7).

use crate::flags::RECORD_FLAG_DELETED;
use crate::index::{zstring, RecordLoc};
use crate::load_order::LoadOrder;
use crate::lstring::{decode_lstring, is_lstring, unresolved, LString};
use crate::overrides::Version;
use crate::query::{FieldValue, Lookup, Path, Subject};
use crate::record::{LoadedRecord, RecordReader};
use crate::strings::StringTables;
use crate::typed::{self, Decoded};

/// Whether a schema type is a union (its variant differs per record).
fn is_union(ty: &kitcraft_schema::FieldType) -> bool {
    match ty {
        kitcraft_schema::FieldType::Union { .. } => true,
        kitcraft_schema::FieldType::Ref { ty, .. } => is_union(ty),
        _ => false,
    }
}

/// One version, evaluated lazily.
pub struct VersionSubject<'a> {
    lo: &'a LoadOrder,
    reader: &'a mut RecordReader,
    version: Version,
    loaded: Option<Result<LoadedRecord, String>>,
    strings: Option<&'a mut StringTables>,
    schema: Option<&'a kitcraft_schema::Schema>,
}

impl<'a> VersionSubject<'a> {
    /// Wrap a version.
    pub fn new(lo: &'a LoadOrder, reader: &'a mut RecordReader, version: Version) -> Self {
        VersionSubject {
            lo,
            reader,
            version,
            loaded: None,
            strings: None,
            schema: kitcraft_schema::builtin().ok(),
        }
    }

    /// Decode field paths through `schema` instead of the builtin definitions.
    pub fn with_schema(mut self, schema: Option<&'a kitcraft_schema::Schema>) -> Self {
        self.schema = schema;
        self
    }

    /// Resolve localized lstrings through `strings`.
    pub fn with_strings(mut self, strings: Option<&'a mut StringTables>) -> Self {
        self.strings = strings;
        self
    }

    /// The record, read on first use.
    pub fn record(&mut self) -> Result<&LoadedRecord, String> {
        if self.loaded.is_none() {
            let result = match self.lo.record(self.version.plugin, self.version.record) {
                Some(loc) => {
                    let loc = *loc;
                    self.reader
                        .read(self.lo, self.version.plugin, &loc)
                        .map_err(|e| format!("record could not be read: {e}"))
                }
                None => Err("record not in the index".to_string()),
            };
            self.loaded = Some(result);
        }
        match &self.loaded {
            Some(Ok(r)) => Ok(r),
            Some(Err(e)) => Err(e.clone()),
            None => Err("record not read".into()),
        }
    }

    fn plugin_name(&self) -> String {
        self.lo
            .plugin(self.version.plugin)
            .map_or_else(String::new, |p| p.name.clone())
    }

    /// Why the payload must not be evaluated: the record is deleted (flag 0x20), so its
    /// subrecords are not the form's data.
    fn deleted_reason(&self, loc: Option<&RecordLoc>) -> Option<String> {
        let loc = loc?;
        (loc.flags & RECORD_FLAG_DELETED != 0).then(|| {
            format!(
                "the version in {} is deleted (record flag 0x20): its payload is not evaluated",
                self.plugin_name()
            )
        })
    }

    fn localized(&self) -> bool {
        self.lo
            .plugin(self.version.plugin)
            .and_then(|p| p.header())
            .is_some_and(|h| h.flags.localized())
    }
}

impl VersionSubject<'_> {
    /// Values of a typed path over every occurrence, or why they are unknown.
    fn typed_values(
        &mut self,
        ty: &kitcraft_schema::FieldType,
        occurrences: &[Vec<u8>],
        ctx: &typed::Ctx,
        members: &[String],
        label: &str,
        plugin: &str,
    ) -> Lookup {
        let mut values = Vec::new();
        for data in occurrences {
            let decoded = match typed::decode(ty, data, ctx) {
                Ok(d) => d,
                Err(e) => {
                    return Lookup::Unknown(format!("{label}: payload does not decode ({e})"))
                }
            };
            let leaves = match typed::select(&decoded, members, label) {
                Ok(l) => l,
                Err(e) => return Lookup::Unknown(e),
            };
            for leaf in leaves {
                match leaf {
                    Decoded::Int(i) => match i64::try_from(i) {
                        Ok(v) => values.push(FieldValue::Int(v)),
                        Err(_) => values.push(FieldValue::Float(i as f64)),
                    },
                    Decoded::Float(f) => values.push(FieldValue::Float(f)),
                    Decoded::Text(t) => values.push(FieldValue::Text(t)),
                    Decoded::Sig(s) => {
                        values.push(FieldValue::Text(String::from_utf8_lossy(&s).into_owned()))
                    }
                    Decoded::LStringId(id) => match self.strings.as_deref_mut() {
                        Some(st) => match st.lookup(plugin, id) {
                            Ok(r) => values.push(FieldValue::Text(r.text)),
                            Err(why) => return Lookup::Unknown(why),
                        },
                        None => return Lookup::Unknown(unresolved(label, plugin, id)),
                    },
                    Decoded::Form(0) => values.push(FieldValue::Null),
                    Decoded::Form(raw) => match self.lo.resolve(self.version.plugin, raw) {
                        Ok(g) => values.push(FieldValue::Form {
                            plugin: self
                                .lo
                                .plugin(g.plugin)
                                .map_or_else(String::new, |p| p.name.clone()),
                            object_id: g.object_id,
                        }),
                        Err(e) => {
                            return Lookup::Unknown(format!("{label}: FormID {raw:08X}: {e}"))
                        }
                    },
                    Decoded::Struct(fields) => {
                        return Lookup::Unknown(format!(
                            "{label} is a struct: name a member ({})",
                            fields
                                .iter()
                                .map(|(k, _)| k.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    }
                    Decoded::Opaque { .. } => {
                        return Lookup::Unknown(format!(
                            "{label}: the schema keeps these bytes opaque"
                        ))
                    }
                    Decoded::Array(items) => {
                        if !items.is_empty() {
                            return Lookup::Unknown(format!("{label} is a nested array"));
                        }
                    }
                }
            }
        }
        if values.is_empty() {
            return Lookup::Absent;
        }
        Lookup::Values(values)
    }
}

impl Subject for VersionSubject<'_> {
    fn lookup(&mut self, path: &Path) -> Lookup {
        let loc = self
            .lo
            .record(self.version.plugin, self.version.record)
            .copied();
        match path {
            Path::Form => {
                let form = self.version.form();
                let plugin = self
                    .lo
                    .plugin(form.plugin)
                    .map_or_else(String::new, |p| p.name.clone());
                Lookup::Values(vec![FieldValue::Form {
                    plugin,
                    object_id: form.object_id,
                }])
            }
            Path::Plugin => Lookup::Values(vec![FieldValue::Text(self.plugin_name())]),
            Path::Edid => {
                match loc.and_then(|l| self.lo.edid(self.version.plugin, &l).map(str::to_string)) {
                    Some(e) => Lookup::Values(vec![FieldValue::Text(e)]),
                    None => Lookup::Absent,
                }
            }
            Path::Flags => match loc {
                Some(l) => Lookup::Values(vec![FieldValue::Int(i64::from(l.flags))]),
                None => Lookup::Unknown("@flags: record not in the index".into()),
            },
            Path::Field {
                signature,
                members,
                index,
            } => {
                let sig = String::from_utf8_lossy(signature).into_owned();
                if let Some(why) = self.deleted_reason(loc.as_ref()) {
                    return Lookup::Unknown(format!("{sig}: {why}"));
                }
                let localized = self.localized();
                let plugin = self.plugin_name();
                let schema = self.schema;
                let rec = match self.record() {
                    Ok(r) => r,
                    Err(e) => return Lookup::Unknown(e),
                };
                let (record_sig, form_version) = (
                    rec.record.header.signature.0,
                    rec.record.header.form_version,
                );
                // Positions of the occurrences the path names; each one's layout comes
                // from its position in the record (`typed::path_layout`).
                let all_sigs: Vec<[u8; 4]> = rec.subs.iter().map(|s| s.signature().0).collect();
                let mut positions: Vec<usize> = all_sigs
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| *s == signature)
                    .map(|(i, _)| i)
                    .collect();
                if let Some(i) = index {
                    positions = usize::try_from(*i)
                        .ok()
                        .and_then(|i| positions.get(i).copied())
                        .into_iter()
                        .collect();
                }
                if positions.is_empty() {
                    return Lookup::Absent;
                }
                let payload = |i: &usize| rec.subs.get(*i).map(|s| s.data().to_vec());
                let mut owned: Vec<Vec<u8>> = positions.iter().filter_map(payload).collect();
                let schema_type = schema.map(|s| {
                    typed::path_layout(s, record_sig, &all_sigs, &positions, index.is_some())
                        .map(|t| {
                            t.map(|(layout, chosen)| {
                                (layout.ty.clone(), layout.name.to_string(), chosen)
                            })
                        })
                        .map_err(|e| e.to_string())
                });
                if let Some(Ok(Some((_, _, chosen)))) = &schema_type {
                    owned = chosen.iter().filter_map(payload).collect();
                }
                let schema_type = schema_type.map(|r| r.map(|t| t.map(|(ty, name, _)| (ty, name))));
                match schema_type {
                    Some(Err(why)) => return Lookup::Unknown(why),
                    Some(Ok(Some((ty, name)))) => {
                        // `KSIZ.Count`: a path may start with the subrecord's own name.
                        let members: &[String] = match members.split_first() {
                            Some((first, rest)) if first.eq_ignore_ascii_case(&name) => rest,
                            _ => members,
                        };
                        let edid_first = loc
                            .and_then(|l| self.lo.edid(self.version.plugin, &l))
                            .and_then(|e| e.bytes().next());
                        let ctx = typed::Ctx {
                            form_version,
                            localized,
                            edid_first,
                        };
                        let label = if members.is_empty() {
                            sig.clone()
                        } else {
                            format!("{sig}.{}", members.join("."))
                        };
                        return match self.typed_values(&ty, &owned, &ctx, members, &label, &plugin)
                        {
                            Lookup::Values(v) if is_union(&ty) => Lookup::UnionValues(v),
                            other => other,
                        };
                    }
                    Some(Ok(None)) | None => {}
                }
                let occurrences: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
                if !members.is_empty() {
                    return Lookup::Unknown(format!(
                        "{sig}.{}: no typed layout for {sig} yet (M2.A3/M2.B2); its payload is opaque",
                        members.join(".")
                    ));
                }
                if sig == "EDID" {
                    return Lookup::Values(
                        occurrences
                            .iter()
                            .map(|d| FieldValue::Text(zstring(d)))
                            .collect(),
                    );
                }
                if is_lstring(&sig) {
                    let ids: Vec<LString> = occurrences
                        .iter()
                        .map(|d| decode_lstring(d, localized))
                        .collect();
                    let mut values = Vec::new();
                    for l in ids {
                        match l {
                            LString::Text(t) => values.push(FieldValue::Text(t)),
                            LString::Id(id) => match self.strings.as_deref_mut() {
                                Some(st) => match st.lookup(&plugin, id) {
                                    Ok(r) => values.push(FieldValue::Text(r.text)),
                                    Err(why) => return Lookup::Unknown(why),
                                },
                                None => return Lookup::Unknown(unresolved(&sig, &plugin, id)),
                            },
                            LString::Malformed { len } => {
                                return Lookup::Unknown(format!(
                                    "{sig} in {plugin} is {len} bytes; a localized string ID is 4"
                                ))
                            }
                        }
                    }
                    return Lookup::Values(values);
                }
                Lookup::Unknown(format!(
                    "{sig} has no typed definition yet (M2.A3/M2.B2); its payload is opaque"
                ))
            }
        }
    }

    fn has(&mut self, signature: [u8; 4]) -> Result<bool, String> {
        let loc = self
            .lo
            .record(self.version.plugin, self.version.record)
            .copied();
        if let Some(why) = self.deleted_reason(loc.as_ref()) {
            return Err(format!(
                "has {}: {why}",
                String::from_utf8_lossy(&signature)
            ));
        }
        let rec = self.record()?;
        Ok(rec.subs.iter().any(|s| s.signature().0 == signature))
    }
}
