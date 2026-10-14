//! New-form allocator (M4.A3): FormIDs for records a plugin adds as its own.
//!
//! FormIDs are raw and file-relative (`docs/load-order.md`, "FormIDs"): a full-scale
//! plugin's own records are stored with the high byte equal to its number of masters and a
//! 24-bit object ID below it. [`NewForms`] hands out object IDs above both the `HEDR` next
//! object ID and the highest object ID the plugin already uses for its own records, in
//! order, one at a time, and [`NewForms::write_hedr`] advances the `HEDR` next object ID past
//! the last one handed out (keeping its high byte, `0xFF` in the corpus). Identities across
//! a load order stay lane D's; nothing here resolves a master.
//!
//! Refused, never guessed (ADR 0004, owner approval rule 5):
//!
//! - a **small or medium** plugin (TES4 flag 0x100 or 0x400): how the Creation Kit stores a
//!   new form of such a plugin is the question of fixtures F11 and F26, not observed yet;
//! - a plugin with both scale bits, or with 253 or more masters (its own high byte would
//!   collide with the `FD`/`FE` class markers);
//! - object IDs past `0xFFFFFF` (exhaustion).
//!
//! Corpus defaults that the Creation Kit fixture F06 (M3.O1) may change are listed in
//! [`LIMITATIONS`]; callers name them in their results.

use crate::index::PluginIndex;
use crate::model::{Entry, Plugin, Record};
use crate::tes4::{Tes4, FLAG_MEDIUM, FLAG_SMALL};
use crate::{Error, Result};

/// Highest object ID of a full-scale plugin's own records.
pub const MAX_FULL_OBJECT_ID: u32 = 0x00FF_FFFF;

/// Choices the allocator makes from the corpus alone, until the Creation Kit is observed
/// (F06; F11 and F26 for small and medium plugins).
pub const LIMITATIONS: &[&str] = &[
    "new FormIDs start at the plugin's HEDR next object ID (0xFF001000 in a new plugin, the \
     value 12 of the 14 official files carry) or above its highest own object ID; what the \
     Creation Kit allocates is not observed yet (fixture F06)",
    "the HEDR next object ID is advanced past the last new form, keeping its high byte \
     (0xFF in every official file); whether the Creation Kit does the same is F06's question",
    "object IDs freed by deleted records are not reused",
    "new forms in small or medium plugins are refused until fixtures F11 and F26 show how \
     the Creation Kit stores them",
];

/// Hands out own FormIDs for new records of one plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewForms {
    /// High byte of the plugin's own FormIDs: its number of masters.
    prefix: u32,
    /// The next object ID to hand out.
    next: u32,
    /// High byte of the `HEDR` next object ID as read.
    hedr_high: u32,
    /// Object IDs handed out so far.
    allocated: u32,
}

fn refused(reason: impl Into<String>) -> Error {
    Error::Refused {
        offset: 0,
        reason: reason.into(),
    }
}

impl NewForms {
    /// The allocator for `header` (a `TES4` record) whose plugin's own records reach object
    /// ID `own_max` (`None` when it has none).
    pub fn new(header: &Record<'_>, own_max: Option<u32>) -> Result<NewForms> {
        let tes4 = Tes4::new(header);
        let flags = tes4.flags();
        if flags & FLAG_SMALL != 0 || flags & FLAG_MEDIUM != 0 {
            return Err(refused(
                "new forms in a small or medium plugin (TES4 flag 0x100 or 0x400) are refused \
                 until the Creation Kit fixtures F11 and F26 show how it stores them",
            ));
        }
        let masters = tes4.masters().len();
        let prefix = u32::try_from(masters)
            .ok()
            .filter(|m| *m < 0xFD)
            .ok_or_else(|| {
                refused(format!(
                    "a plugin with {masters} masters has no full-class FormID prefix for its \
                     own records (the high bytes FD and FE mark the medium and small classes)"
                ))
            })?;
        let hedr = match tes4.hedr() {
            Some(Ok(h)) => h,
            Some(Err(e)) => return Err(e),
            None => return Err(refused("the TES4 record has no HEDR")),
        };
        let from_hedr = hedr.next_object_id & MAX_FULL_OBJECT_ID;
        let after_own = own_max.map_or(0, |m| m.saturating_add(1));
        Ok(NewForms {
            prefix,
            next: from_hedr.max(after_own).max(1),
            hedr_high: hedr.next_object_id & !MAX_FULL_OBJECT_ID,
            allocated: 0,
        })
    }

    /// The allocator for an indexed plugin.
    pub fn for_index(index: &PluginIndex) -> Result<NewForms> {
        let prefix = Tes4::new(&index.header).masters().len();
        let own_max = index
            .records
            .iter()
            .skip(1)
            .filter(|r| usize::try_from(r.form_id >> 24).ok() == Some(prefix))
            .map(|r| r.form_id & MAX_FULL_OBJECT_ID)
            .max();
        NewForms::new(&index.header, own_max)
    }

    /// The allocator for a plugin in memory.
    pub fn for_plugin(plugin: &Plugin<'_>) -> Result<NewForms> {
        let prefix = plugin.tes4().masters().len();
        let mut own_max: Option<u32> = None;
        let mut pending: Vec<&Entry<'_>> = plugin.entries().iter().collect();
        while let Some(entry) = pending.pop() {
            match entry {
                Entry::Group(g) => pending.extend(g.children()),
                Entry::Record(r) => {
                    if usize::try_from(r.header.form_id >> 24).ok() == Some(prefix) {
                        let id = r.header.form_id & MAX_FULL_OBJECT_ID;
                        own_max = Some(own_max.map_or(id, |m| m.max(id)));
                    }
                }
            }
        }
        NewForms::new(plugin.header(), own_max)
    }

    /// The FormID [`NewForms::allocate`] returns next, or an error when the object IDs are
    /// exhausted.
    pub fn peek(&self) -> Result<u32> {
        if self.next > MAX_FULL_OBJECT_ID {
            return Err(refused(
                "object IDs are exhausted: a full-scale plugin's own object IDs end at 0xFFFFFF",
            ));
        }
        Ok((self.prefix << 24) | self.next)
    }

    /// A new own FormID (stored form: the plugin's master count in the high byte).
    pub fn allocate(&mut self) -> Result<u32> {
        let id = self.peek()?;
        self.next = self.next.saturating_add(1);
        self.allocated = self.allocated.saturating_add(1);
        Ok(id)
    }

    /// How many FormIDs were handed out.
    pub fn allocated(&self) -> u32 {
        self.allocated
    }

    /// The `HEDR` next object ID after the forms handed out: the next object ID with the
    /// high byte the header had.
    pub fn hedr_next_object_id(&self) -> u32 {
        self.hedr_high | (self.next & MAX_FULL_OBJECT_ID)
    }

    /// Write [`NewForms::hedr_next_object_id`] into `header` (the plugin's `TES4`). Leaves
    /// it untouched (and returns `Ok(false)`) when nothing was allocated.
    pub fn write_hedr(&self, header: &mut Record<'_>) -> Result<bool> {
        if self.allocated == 0 {
            return Ok(false);
        }
        Tes4::set_next_object_id(header, self.hedr_next_object_id())
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{NewForms, MAX_FULL_OBJECT_ID};
    use crate::build::HeaderSpec;
    use crate::fixture::{self, group, record, sub};
    use crate::index::{index_bytes, IndexOptions};
    use crate::model::GroupHeader;
    use crate::tes4::Tes4;
    use crate::{Error, Plugin, Record, Signature};

    fn rec(sig: &[u8; 4], id: u32) -> Record<'static> {
        let bytes = record(sig, 0, id, &sub(b"EDID", b"x\0"));
        crate::parse::parse_record(&mut crate::Reader::new(&bytes, 0))
            .unwrap()
            .into_owned()
    }

    #[test]
    fn a_new_plugin_starts_at_its_hedr_and_advances_it() {
        let masters = vec![b"Starfield.esm".to_vec(), b"Other.esm".to_vec()];
        let mut plugin = Plugin::create(&HeaderSpec::new(masters));
        let mut forms = NewForms::for_plugin(&plugin).unwrap();
        assert_eq!(forms.peek().unwrap(), 0x0200_1000);
        assert_eq!(forms.allocate().unwrap(), 0x0200_1000);
        assert_eq!(forms.allocate().unwrap(), 0x0200_1001);
        assert_eq!(forms.allocated(), 2);
        assert_eq!(forms.hedr_next_object_id(), 0xFF00_1002);
        assert!(forms.write_hedr(plugin.header_mut()).unwrap());
        let hedr = plugin.tes4().hedr().unwrap().unwrap();
        assert_eq!(hedr.next_object_id, 0xFF00_1002);
        // Deterministic, and the next allocator continues after the header.
        let again = NewForms::for_plugin(&plugin).unwrap();
        assert_eq!(again.peek().unwrap(), 0x0200_1002);
        // Nothing allocated: the header is left alone.
        let none = NewForms::for_plugin(&plugin).unwrap();
        let before = plugin.header().clone();
        assert!(!none.write_hedr(plugin.header_mut()).unwrap());
        assert_eq!(plugin.header(), &before);
    }

    #[test]
    fn own_records_above_the_hedr_push_the_start_up() {
        let mut plugin = Plugin::create(&HeaderSpec::new(vec![b"Starfield.esm".to_vec()]));
        let top = GroupHeader::top(Signature::new(b"GLOB"));
        // An own record above the HEDR value, and a master's record (prefix 00) above both.
        plugin.upsert(&[top], rec(b"GLOB", 0x0100_5000)).unwrap();
        plugin.upsert(&[top], rec(b"GLOB", 0x00FF_0000)).unwrap();
        let forms = NewForms::for_plugin(&plugin).unwrap();
        assert_eq!(forms.peek().unwrap(), 0x0100_5001);
        let bytes = plugin.to_bytes().unwrap();
        let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
        assert_eq!(NewForms::for_index(&index).unwrap(), forms);
    }

    #[test]
    fn scales_master_counts_and_exhaustion_are_refused() {
        for flags in [0x100, 0x400, 0x500, 0x181] {
            let mut spec = HeaderSpec::new(Vec::new());
            spec.flags = flags;
            let plugin = Plugin::create(&spec);
            assert!(
                matches!(NewForms::for_plugin(&plugin), Err(Error::Refused { .. })),
                "{flags:#x}"
            );
        }
        // The blueprint flag alone is a full-scale plugin.
        let mut spec = HeaderSpec::new(Vec::new());
        spec.flags = 0x881;
        assert!(NewForms::for_plugin(&Plugin::create(&spec)).is_ok());
        let many: Vec<Vec<u8>> = (0..253).map(|i| format!("M{i}.esm").into_bytes()).collect();
        let plugin = Plugin::create(&HeaderSpec::new(many));
        assert!(matches!(
            NewForms::for_plugin(&plugin),
            Err(Error::Refused { .. })
        ));
        let mut spec = HeaderSpec::new(Vec::new());
        spec.next_object_id = 0xFF00_0000 | (MAX_FULL_OBJECT_ID - 1);
        let mut forms = NewForms::for_plugin(&Plugin::create(&spec)).unwrap();
        assert_eq!(forms.allocate().unwrap(), MAX_FULL_OBJECT_ID - 1);
        assert_eq!(forms.allocate().unwrap(), MAX_FULL_OBJECT_ID);
        assert!(matches!(forms.allocate(), Err(Error::Refused { .. })));
        assert!(forms.peek().is_err());
        // A header without a HEDR.
        let bare = fixture::plugin(&[]);
        let mut plugin = Plugin::parse(&bare).unwrap().into_owned();
        plugin
            .header_mut()
            .subrecords_mut()
            .unwrap()
            .retain(|s| s.signature() != Signature::new(b"HEDR"));
        assert!(NewForms::for_plugin(&plugin).is_err());
    }

    #[test]
    fn the_sample_plugins_own_records_count() {
        // The fixture plugin has 2 masters; its records are stored 00000xxx (master 0), so
        // none is its own and its HEDR decides.
        let bytes = fixture::sample_plugin();
        let plugin = Plugin::parse(&bytes).unwrap();
        let forms = NewForms::for_plugin(&plugin).unwrap();
        let hedr = plugin.tes4().hedr().unwrap().unwrap();
        assert_eq!(
            forms.peek().unwrap(),
            (2 << 24) | (hedr.next_object_id & MAX_FULL_OBJECT_ID)
        );
        let extra = group(
            b"GLOB",
            0,
            &record(b"GLOB", 0, 0x0200_0900, &sub(b"EDID", b"g\0")),
        );
        let mut more = bytes.clone();
        more.extend(extra);
        let index = index_bytes(&more, &IndexOptions::default()).unwrap();
        let forms = NewForms::for_index(&index).unwrap();
        let expected = (hedr.next_object_id & MAX_FULL_OBJECT_ID).max(0x901);
        assert_eq!(forms.peek().unwrap(), (2 << 24) | expected);
        let _ = Tes4::new(plugin.header());
    }
}
