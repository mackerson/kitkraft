//! Typed view of the `TES4` header record (UESP, Skyrim_Mod:Mod_File_Format/TES4, which
//! the Starfield page defers to).
//!
//! Only flags UESP documents are named here. Starfield's own plugin-size flags (the "small"
//! and "medium" master variants) are not on UESP; the bits observed in the corpus are
//! recorded in `docs/corpus.md` and exposed only through [`Tes4::flags`].

use crate::model::Record;
use crate::{Reader, Result, Signature};

/// TES4 flag: the file is a master regardless of its extension.
pub const FLAG_MASTER: u32 = 0x0000_0001;
/// TES4 flag: strings live in `.STRINGS`/`.DLSTRINGS`/`.ILSTRINGS` tables and lstring
/// subrecords hold string IDs instead of text.
pub const FLAG_LOCALIZED: u32 = 0x0000_0080;

/// `HEDR`: version, number of records and groups, next object ID.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hedr {
    pub version: f32,
    /// Number of records and groups, not counting the TES4 record itself.
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

    /// The `HEDR` subrecord, if present. Errors carry offsets relative to the subrecord.
    pub fn hedr(&self) -> Option<Result<Hedr>> {
        let sub = self.subrecords().iter().find(|s| s.signature() == HEDR)?;
        let mut r = Reader::new(sub.data(), 0);
        Some((|| {
            Ok(Hedr {
                version: r.f32()?,
                record_count: r.u32()?,
                next_object_id: r.u32()?,
            })
        })())
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

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::Tes4;
    use crate::{fixture, Plugin};

    #[test]
    fn reads_the_fixture_header() {
        let bytes = fixture::plugin(&[]);
        let plugin = Plugin::parse(&bytes).unwrap();
        let tes4 = Tes4::new(plugin.header());
        assert!(tes4.is_master());
        assert!(tes4.is_localized());
        assert_eq!(tes4.form_version(), 581);
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
}
