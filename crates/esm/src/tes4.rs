//! Typed view of the `TES4` header record (UESP, Skyrim_Mod:Mod_File_Format/TES4, which
//! the Starfield page defers to).
//!
//! # Header flags
//!
//! The flag bits are named after the pinned xEdit Starfield definitions
//! (`xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L9045-L9083`, the
//! TES4 record definition span listed in `docs/xedit-reference.md`; ADR 0004). Bits 0x1,
//! 0x80, 0x100, 0x400 and 0x800 occur in the official ESMs (confidence `corpus`,
//! `docs/corpus.md`); 0x10 and 0x200 never do (confidence `reference`). The names say what
//! the reference calls each bit, not what the engine does with it: how small, medium and
//! blueprint plugins form runtime FormIDs is a `ck-diff` fact with no source yet (M2.D3).
//! [`Tes4::flags`] keeps the raw `u32`; nothing here sets or clears a bit on its own.
//!
//! # The `HEDR` record count
//!
//! What the count counts is taken from the corpus, not from UESP (whose Skyrim meaning,
//! records plus groups without `TES4`, holds in only 2 of the 14 official files):
//!
//! - 12 files: every record including `TES4`, groups not counted ([`majority_record_count`]);
//! - `BlueprintShips-Starfield.esm` and `BlueprintShips-SFBGS050.esm`: records excluding
//!   `TES4`, plus groups.
//!
//! Policy: [`Plugin::write_to`] writes the count verbatim unless records or groups were
//! added or removed since the plugin was parsed (or the plugin was built in memory); then
//! it writes the majority meaning. A value set by hand is overwritten in that case.

use std::borrow::Cow;

use crate::model::{Counts, Plugin, Record};
use crate::{Error, Reader, Result, Signature};

/// TES4 flag "Master": the file is a master regardless of its extension (also UESP).
/// Confidence `corpus`: set in all 14 official ESMs.
pub const FLAG_MASTER: u32 = 0x0000_0001;
/// TES4 flag "Optimized" (xEdit). Confidence `reference`: in no official ESM.
pub const FLAG_OPTIMIZED: u32 = 0x0000_0010;
/// TES4 flag "Localized": strings live in `.STRINGS`/`.DLSTRINGS`/`.ILSTRINGS` tables and
/// lstring subrecords hold string IDs instead of text (also UESP). Confidence `corpus`: set
/// in all 14 official ESMs.
pub const FLAG_LOCALIZED: u32 = 0x0000_0080;
/// TES4 flag "Small" (xEdit). Confidence `corpus`: 6 official ESMs, all small.
pub const FLAG_SMALL: u32 = 0x0000_0100;
/// TES4 flag "Update" (xEdit). Confidence `reference`: in no official ESM (UESP gives the
/// same bit a different, Skyrim meaning).
pub const FLAG_UPDATE: u32 = 0x0000_0200;
/// TES4 flag "Medium" (xEdit). Confidence `corpus`: `SFBGS003.esm`, `SFBGS006.esm`.
pub const FLAG_MEDIUM: u32 = 0x0000_0400;
/// TES4 flag "Blueprint" (xEdit). Confidence `corpus`: both `BlueprintShips` files.
pub const FLAG_BLUEPRINT: u32 = 0x0000_0800;

/// `HEDR`: version, record count, next object ID.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hedr {
    pub version: f32,
    /// Record count. In 12 of the 14 official files it is the number of records including
    /// `TES4`, groups not counted; in the two `BlueprintShips` files it is the number of
    /// records excluding `TES4` plus the number of groups. See the module documentation.
    pub record_count: u32,
    pub next_object_id: u32,
}

/// One `MAST` entry and the `DATA` that follows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Master {
    /// File name bytes without the terminating NUL.
    pub name: Vec<u8>,
    /// The `u64` from the following `DATA` subrecord, if present.
    pub data: Option<u64>,
}

impl Master {
    /// The file name, lossily decoded.
    pub fn name_lossy(&self) -> String {
        String::from_utf8_lossy(&self.name).into_owned()
    }
}

/// A read-only view over a `TES4` record.
#[derive(Debug, Clone, Copy)]
pub struct Tes4<'r, 'a> {
    record: &'r Record<'a>,
}

const HEDR: Signature = Signature::new(b"HEDR");
const MAST: Signature = Signature::new(b"MAST");
const DATA: Signature = Signature::new(b"DATA");
const ONAM: Signature = Signature::new(b"ONAM");

/// Offset of the record count inside `HEDR` data (after the `f32` version).
const COUNT_AT: usize = 4;
/// Offset of the next object ID inside `HEDR` data.
const NEXT_ID_AT: usize = 8;

/// What [`Plugin::write_to`] does with the `HEDR` record count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedrCount {
    /// Written as stored: no record or group was added or removed, or the stored value
    /// already is the majority count.
    Verbatim,
    /// Records or groups were added or removed; this majority count is written.
    Recomputed(u32),
    /// Records or groups were added or removed, but the header has no `HEDR` with room for
    /// a count (or the count does not fit a `u32`): the header is written as stored and the
    /// count is stale.
    Unreadable,
}

/// The `HEDR` record count the majority of the official files use for `counts` (records
/// and groups below `TES4`): every record including `TES4`, groups not counted. `None` if
/// it does not fit a `u32`.
pub fn majority_record_count(counts: Counts) -> Option<u32> {
    u32::try_from(counts.records.saturating_add(1)).ok()
}

impl<'r, 'a> Tes4<'r, 'a> {
    /// View `record` as a TES4 header.
    pub fn new(record: &'r Record<'a>) -> Tes4<'r, 'a> {
        Tes4 { record }
    }

    /// Raw record flags.
    pub fn flags(&self) -> u32 {
        self.record.header.flags
    }

    /// Whether [`FLAG_MASTER`] is set.
    pub fn is_master(&self) -> bool {
        self.flags() & FLAG_MASTER != 0
    }

    /// Whether [`FLAG_LOCALIZED`] is set.
    pub fn is_localized(&self) -> bool {
        self.flags() & FLAG_LOCALIZED != 0
    }

    /// The record's form version.
    pub fn form_version(&self) -> u16 {
        self.record.header.form_version
    }

    fn subrecords(&self) -> &'r [crate::Subrecord<'a>] {
        self.record.subrecords().unwrap_or(&[])
    }

    /// The `HEDR` subrecord, if present. Errors carry absolute file offsets (relative to
    /// the subrecord data for a header built in memory).
    pub fn hedr(&self) -> Option<Result<Hedr>> {
        let sub = self.subrecords().iter().find(|s| s.signature() == HEDR)?;
        let mut r = Reader::new(sub.data(), sub.data_offset().unwrap_or(0));
        Some((|| {
            Ok(Hedr {
                version: r.f32()?,
                record_count: r.u32()?,
                next_object_id: r.u32()?,
            })
        })())
    }

    /// Set the `HEDR` record count of `record` (a `TES4` record), keeping every other byte
    /// of the subrecord. Returns `false` when there is no `HEDR` to update (none present,
    /// or the record is compressed and not decoded), and [`Error::BadHedr`] when the
    /// `HEDR` is too short to hold a count. Marks the record modified.
    pub fn set_record_count(record: &mut Record<'_>, count: u32) -> Result<bool> {
        Self::set_hedr_u32(record, COUNT_AT, count)
    }

    /// Set the `HEDR` next object ID of `record` (a `TES4` record), keeping every other
    /// byte; as [`Tes4::set_record_count`] otherwise (the `HEDR` must hold 12 bytes).
    pub fn set_next_object_id(record: &mut Record<'_>, value: u32) -> Result<bool> {
        Self::set_hedr_u32(record, NEXT_ID_AT, value)
    }

    /// Write `value` at byte `at` of the `HEDR` data.
    fn set_hedr_u32(record: &mut Record<'_>, at: usize, value: u32) -> Result<bool> {
        let Some(subs) = record.subrecords_mut() else {
            return Ok(false);
        };
        let Some(sub) = subs.iter_mut().find(|s| s.signature() == HEDR) else {
            return Ok(false);
        };
        let mut data = sub.data().to_vec();
        let len = data.len() as u64;
        let Some(slot) = data.get_mut(at..at.saturating_add(4)) else {
            return Err(Error::BadHedr {
                offset: sub.data_offset().unwrap_or(0),
                len,
            });
        };
        slot.copy_from_slice(&value.to_le_bytes());
        let prefix = sub.has_oversize_prefix();
        sub.set_data(data);
        if prefix {
            sub.set_oversize_prefix(true);
        }
        Ok(true)
    }

    /// Masters in order, each with the `DATA` value that follows it.
    pub fn masters(&self) -> Vec<Master> {
        let mut out: Vec<Master> = Vec::new();
        let mut expect_data = false;
        for sub in self.subrecords() {
            if sub.signature() == MAST {
                let name = sub.data();
                let name = name.strip_suffix(&[0]).unwrap_or(name);
                out.push(Master {
                    name: name.to_vec(),
                    data: None,
                });
                expect_data = true;
            } else if sub.signature() == DATA && expect_data {
                let value = Reader::new(sub.data(), 0).u64().ok();
                if let Some(last) = out.last_mut() {
                    last.data = value;
                }
                expect_data = false;
            } else {
                expect_data = false;
            }
        }
        out
    }

    /// FormIDs listed in `ONAM` (overridden forms). Trailing bytes short of a whole FormID
    /// are ignored.
    pub fn overridden_forms(&self) -> Vec<u32> {
        let Some(sub) = self.subrecords().iter().find(|s| s.signature() == ONAM) else {
            return Vec::new();
        };
        sub.data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect()
    }
}

impl<'a> Plugin<'a> {
    /// The header record viewed as `TES4`.
    pub fn tes4(&self) -> Tes4<'_, 'a> {
        Tes4::new(&self.header)
    }

    /// Whether the plugin is localized (TES4 flag [`FLAG_LOCALIZED`]): lstring subrecords
    /// of every record hold string-table IDs.
    pub fn is_localized(&self) -> bool {
        self.tes4().is_localized()
    }

    /// The `TES4` record's form version. Each record carries its own form version in its
    /// header ([`crate::RecordHeader::form_version`]), which is what decides its layout.
    pub fn form_version(&self) -> u16 {
        self.tes4().form_version()
    }

    /// What [`Plugin::write_to`] will do with the `HEDR` record count (see the module
    /// documentation). [`HedrCount::Unreadable`] is the one case where the count written
    /// is stale; the same case in a streaming rewrite is
    /// [`crate::driver::RewriteReport::hedr_unreadable`].
    pub fn hedr_count_on_write(&self) -> HedrCount {
        let now = Counts::of(&self.entries);
        if self.counts_as_read == Some(now) {
            return HedrCount::Verbatim;
        }
        let Some(count) = majority_record_count(now) else {
            return HedrCount::Unreadable;
        };
        match self.tes4().hedr() {
            Some(Ok(h)) if h.record_count == count => HedrCount::Verbatim,
            Some(Ok(_)) => HedrCount::Recomputed(count),
            Some(Err(_)) | None => HedrCount::Unreadable,
        }
    }

    /// The header record to write: as stored, unless records or groups were added or
    /// removed, in which case a copy with the majority `HEDR` record count. A header
    /// without a usable `HEDR` is written as stored ([`HedrCount::Unreadable`]).
    pub(crate) fn header_for_write(&self) -> Result<Cow<'_, Record<'a>>> {
        let HedrCount::Recomputed(count) = self.hedr_count_on_write() else {
            return Ok(Cow::Borrowed(&self.header));
        };
        let mut header = self.header.clone();
        match Tes4::set_record_count(&mut header, count) {
            Ok(true) => Ok(Cow::Owned(header)),
            Ok(false) | Err(Error::BadHedr { .. }) => Ok(Cow::Borrowed(&self.header)),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{majority_record_count, HedrCount, Tes4};
    use crate::fixture::{self, group, record, sub};
    use crate::model::{Counts, Entry};
    use crate::{Error, Plugin};

    fn hedr_count(bytes: &[u8]) -> u32 {
        let plugin = Plugin::parse(bytes).unwrap();
        plugin.tes4().hedr().unwrap().unwrap().record_count
    }

    #[test]
    fn reads_the_fixture_header() {
        let bytes = fixture::plugin(&[]);
        let plugin = Plugin::parse(&bytes).unwrap();
        let tes4 = Tes4::new(plugin.header());
        assert!(tes4.is_master());
        assert!(tes4.is_localized());
        assert!(plugin.is_localized());
        assert_eq!(tes4.form_version(), 581);
        assert_eq!(plugin.form_version(), 581);
        let hedr = tes4.hedr().unwrap().unwrap();
        assert_eq!(hedr.version, 0.96);
        assert_eq!(hedr.record_count, 42);
        assert_eq!(hedr.next_object_id, 0x800);
        let masters = tes4.masters();
        assert_eq!(masters.len(), 2);
        assert_eq!(masters[0].name_lossy(), "Starfield.esm");
        assert_eq!(masters[0].data, Some(0));
        assert_eq!(masters[1].name_lossy(), "Other.esm");
        assert_eq!(tes4.overridden_forms(), [0x0000_0A01, 0x0100_0B02]);
    }

    #[test]
    fn hedr_errors_carry_absolute_offsets() {
        // A TES4 whose HEDR holds only the version: the count read fails 4 bytes into it.
        let body = [sub(b"CNAM", b"x\0"), sub(b"HEDR", &0.96f32.to_le_bytes())].concat();
        let bytes = record(b"TES4", 0x81, 0, &body);
        let plugin = Plugin::parse(&bytes).unwrap();
        let hedr_data_at = 24 + 8 + 6;
        match plugin.tes4().hedr() {
            Some(Err(Error::UnexpectedEof { offset, .. })) => {
                assert_eq!(offset, hedr_data_at + 4);
            }
            other => panic!("{other:?}"),
        }
        let hedr = &plugin.header().subrecords().unwrap()[1];
        assert_eq!(hedr.offset(), Some(24 + 8));
        assert_eq!(hedr.data_offset(), Some(hedr_data_at));
        // Setting the count needs the field to be there (review round 3, finding 6: this
        // was an input-offset UnexpectedEof surfacing from a write call).
        let mut header = plugin.header().clone();
        assert!(matches!(
            Tes4::set_record_count(&mut header, 5),
            Err(Error::BadHedr { offset, len: 4 }) if offset == hedr_data_at
        ));
    }

    /// Review round 3, finding 3: a short HEDR made `to_bytes` fail once records were
    /// added, while the streaming rewrite stayed silent. Both now write the header as
    /// stored and say so.
    #[test]
    fn a_short_hedr_is_kept_and_reported_when_the_count_changes() {
        let body = [sub(b"HEDR", &0.96f32.to_le_bytes()), sub(b"CNAM", b"x\0")].concat();
        let rec = record(b"GMST", 0, 1, &sub(b"EDID", b"a\0"));
        let bytes = [record(b"TES4", 0, 0, &body), group(b"GMST", 0, &rec)].concat();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        assert_eq!(plugin.hedr_count_on_write(), HedrCount::Verbatim);
        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
            panic!()
        };
        g.children_mut().pop();
        assert_eq!(plugin.hedr_count_on_write(), HedrCount::Unreadable);
        let out = plugin.to_bytes().unwrap();
        assert_eq!(out[..24 + body.len()], bytes[..24 + body.len()]);

        let mut sink = std::io::Cursor::new(Vec::new());
        let report = crate::driver::rewrite_reader(
            &bytes[..],
            bytes.len() as u64,
            &Default::default(),
            &mut sink,
            |e| {
                if let Entry::Group(g) = e {
                    g.children_mut().pop();
                }
                Ok(())
            },
        )
        .unwrap();
        assert!(report.hedr_unreadable);
        assert_eq!(report.hedr_fix, None);
        assert_eq!(sink.into_inner(), out);
    }

    #[test]
    fn set_record_count_changes_only_the_count() {
        let bytes = fixture::plugin(&[]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        assert!(Tes4::set_record_count(plugin.header_mut(), 7).unwrap());
        let out = plugin.to_bytes().unwrap();
        assert_eq!(out.len(), bytes.len());
        assert_eq!(hedr_count(&out), 7);
        let diffs: Vec<usize> = (0..out.len()).filter(|&i| out[i] != bytes[i]).collect();
        assert!(
            diffs.iter().all(|&i| (24 + 6 + 4..24 + 6 + 8).contains(&i)),
            "{diffs:?}"
        );
    }

    #[test]
    fn record_count_is_preserved_unless_entries_are_added_or_removed() {
        // The fixture's HEDR says 42, matching neither convention: kept while the tree
        // keeps its shape, even when records are edited or the tree is touched.
        let rec = |id: u32| record(b"GMST", 0, id, &sub(b"EDID", b"a\0"));
        let bytes = fixture::plugin(&[group(b"GMST", 0, &[rec(1), rec(2)].concat())]);
        let mut plugin = Plugin::parse(&bytes).unwrap();
        assert_eq!(
            plugin.counts_as_read(),
            Some(Counts {
                records: 2,
                groups: 1
            })
        );
        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
            panic!()
        };
        let Entry::Record(r) = &mut g.children_mut()[0] else {
            panic!()
        };
        r.subrecords_mut().unwrap()[0].set_data(&b"changed\0"[..]);
        assert_eq!(hedr_count(&plugin.to_bytes().unwrap()), 42);

        // A record removed: recomputed by the majority rule (records + TES4, no groups).
        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
            panic!()
        };
        g.children_mut().pop();
        assert_eq!(plugin.hedr_count_on_write(), HedrCount::Recomputed(2));
        assert_eq!(hedr_count(&plugin.to_bytes().unwrap()), 2);

        // Put back by hand: the shape matches what was read, so verbatim again.
        let Entry::Group(g) = &mut plugin.entries_mut()[0] else {
            panic!()
        };
        let again = Plugin::parse(&bytes).unwrap();
        let Entry::Group(orig) = &again.entries()[0] else {
            panic!()
        };
        g.children_mut().push(orig.children()[1].clone());
        assert_eq!(hedr_count(&plugin.to_bytes().unwrap()), 42);

        // A group added.
        plugin
            .entries_mut()
            .push(Entry::Group(crate::Group::new(Default::default())));
        assert_eq!(hedr_count(&plugin.to_bytes().unwrap()), 3);
        assert_eq!(
            majority_record_count(Counts {
                records: 2,
                groups: 2
            }),
            Some(3)
        );
    }
}
