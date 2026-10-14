//! A version of a form in a load order, as a `where` [`Subject`].
//!
//! Index-only paths (`@form`, `@plugin`, `@edid`, `@flags`) never read the file; field paths
//! read the record once. What can be decoded today: `EDID` (zstring) and the lstrings
//! `FULL`/`DESC` of non-localized plugins. Everything else is unknown until typed layouts are
//! wired (M2.A3, M2.B2), and a localized lstring is unknown until string tables are loaded
//! (M2.D7).

use crate::index::zstring;
use crate::load_order::LoadOrder;
use crate::lstring::{decode_lstring, is_lstring, unresolved, LString};
use crate::overrides::Version;
use crate::query::{FieldValue, Lookup, Path, Subject};
use crate::record::{LoadedRecord, RecordReader};

/// One version, evaluated lazily.
pub struct VersionSubject<'a> {
    lo: &'a LoadOrder,
    reader: &'a mut RecordReader,
    version: Version,
    loaded: Option<Result<LoadedRecord, String>>,
}

impl<'a> VersionSubject<'a> {
    /// Wrap a version.
    pub fn new(lo: &'a LoadOrder, reader: &'a mut RecordReader, version: Version) -> Self {
        VersionSubject {
            lo,
            reader,
            version,
            loaded: None,
        }
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

    fn localized(&self) -> bool {
        self.lo
            .plugin(self.version.plugin)
            .and_then(|p| p.header())
            .is_some_and(|h| h.flags.localized())
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
            Path::Flags => {
                Lookup::Values(vec![FieldValue::Int(i64::from(loc.map_or(0, |l| l.flags)))])
            }
            Path::Field {
                signature,
                members,
                index,
            } => {
                let sig = String::from_utf8_lossy(signature).into_owned();
                let localized = self.localized();
                let plugin = self.plugin_name();
                let rec = match self.record() {
                    Ok(r) => r,
                    Err(e) => return Lookup::Unknown(e),
                };
                let mut occurrences: Vec<&[u8]> = rec
                    .subs
                    .iter()
                    .filter(|s| s.signature().0 == *signature)
                    .map(|s| s.data())
                    .collect();
                if let Some(i) = index {
                    occurrences = usize::try_from(*i)
                        .ok()
                        .and_then(|i| occurrences.get(i).copied())
                        .into_iter()
                        .collect();
                }
                if occurrences.is_empty() {
                    return Lookup::Absent;
                }
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
                    let mut values = Vec::new();
                    for d in occurrences {
                        match decode_lstring(d, localized) {
                            LString::Text(t) => values.push(FieldValue::Text(t)),
                            LString::Id(id) => {
                                return Lookup::Unknown(unresolved(&sig, &plugin, id))
                            }
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
        let rec = self.record()?;
        Ok(rec.subs.iter().any(|s| s.signature().0 == signature))
    }
}
