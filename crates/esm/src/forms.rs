//! New-form allocator (M4.A3): FormIDs for records a plugin adds as its own.
//!
//! FormIDs are raw and file-relative (`docs/load-order.md`, "FormIDs"): a full-scale
//! plugin's own records are stored in the full class with the high byte equal to its
//! number of **full** masters (the index counts only the file's masters of that class, so
//! a small or medium master does not move it) and a 24-bit object ID below it. The masters'
//! scales are not in the file: the caller reads them from each master's `TES4`
//! ([`crate::tes4::Scale::of_flags`]) and passes them in master-list order; a master whose
//! scale is unknown (not loaded) is refused, never guessed (ADR 0004, owner approval rule
//! 5). Identities across a load order stay lane D's; nothing here resolves a master.
//!
//! [`NewForms`] hands out object IDs above both the `HEDR` next object ID and the highest
//! object ID the plugin already uses for its own records, in order, one at a time, and
//! [`NewForms::write_hedr`] advances the `HEDR` next object ID past the last one handed out
//! (keeping its high byte, `0xFF` in the corpus). Use **one allocator per plugin per edit
//! session**: two allocators made from the same plugin hand out the same FormIDs (it is
//! not `Copy` so that a copy cannot do this by accident).
//!
//! Refused, never guessed (ADR 0004, owner approval rule 5):
//!
//! - a **small or medium** plugin (TES4 flag 0x100 or 0x400): how the Creation Kit stores a
//!   new form of such a plugin is the question of fixtures F11 and F26, not observed yet;
//! - a master whose scale is unknown, or a scale list that does not match the masters;
//! - a plugin with both scale bits, or with 253 or more full masters (its own high byte
//!   would collide with the `FD`/`FE` class markers);
//! - object IDs past `0xFFFFFF` (exhaustion).
//!
//! Corpus defaults that the Creation Kit fixture F06 (M3.O1) may change are listed in
//! [`LIMITATIONS`]; [`NewForms::limitations`] adds what applies to one plugin. Callers name
//! them in their results.

use crate::index::PluginIndex;
use crate::model::{Entry, Plugin, Record};
use crate::tes4::{Scale, Tes4};
use crate::{Error, Result};

/// Highest object ID of a full-scale plugin's own records.
pub const MAX_FULL_OBJECT_ID: u32 = 0x00FF_FFFF;

/// The object ID every official file but two and a new plugin ([`crate::build::HeaderSpec`])
/// start their `HEDR` next object ID at. Not known to be a reserved floor.
pub const USUAL_FIRST_OBJECT_ID: u32 = 0x0000_1000;

/// Choices the allocator makes from the corpus alone, until the Creation Kit is observed
/// (F06; F11 and F26 for small and medium plugins).
pub const LIMITATIONS: &[&str] = &[
    "new FormIDs start at the plugin's HEDR next object ID (0xFF001000 in a new plugin, the \
     value 12 of the 14 official files carry) or above its highest own object ID; no \
     reserved floor of object IDs is known (nothing says IDs below 0x1000 are reserved), \
     and what the Creation Kit allocates is not observed yet (fixture F06)",
    "the HEDR next object ID is advanced past the last new form, keeping its high byte \
     (0xFF in every official file); whether the Creation Kit does the same is F06's question",
    "object IDs freed by deleted records are not reused",
    "new forms in small or medium plugins are refused until fixtures F11 and F26 show how \
     the Creation Kit stores them",
];

/// Hands out own FormIDs for new records of one plugin. One allocator per plugin per edit
/// session (see the module documentation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewForms {
    /// High byte of the plugin's own FormIDs: its number of full masters.
    prefix: u32,
    /// The next object ID to hand out.
    next: u32,
    /// The `HEDR` next object ID as read.
    hedr_read: u32,
    /// Whether the plugin has the blueprint flag.
    blueprint: bool,
    /// Object IDs handed out so far.
    allocated: u32,
}

fn refused(reason: impl Into<String>) -> Error {
    Error::Refused {
        offset: 0,
        reason: reason.into(),
    }
}

/// The high byte of the own FormIDs of the full-scale plugin whose `TES4` is `header`,
/// given its masters' scales in master-list order: the number of full masters. Refused
/// for a small or medium plugin, a scale list that does not match the masters, a master
/// whose scale is unknown (`None`), and 253 or more full masters.
pub fn own_prefix(header: &Record<'_>, master_scales: &[Option<Scale>]) -> Result<u32> {
    let tes4 = Tes4::new(header);
    if tes4.scale() != Some(Scale::Full) {
        return Err(refused(
            "new forms in a small or medium plugin (TES4 flag 0x100 or 0x400) are refused \
             until the Creation Kit fixtures F11 and F26 show how it stores them",
        ));
    }
    let masters = tes4.masters();
    if masters.len() != master_scales.len() {
        return Err(refused(format!(
            "{} master scales given for a plugin with {} masters",
            master_scales.len(),
            masters.len()
        )));
    }
    let mut full: usize = 0;
    for (master, scale) in masters.iter().zip(master_scales) {
        match scale {
            Some(Scale::Full) => full = full.saturating_add(1),
            Some(Scale::Medium | Scale::Small) => {}
            None => {
                return Err(refused(format!(
                    "the scale of master {} is unknown (not loaded): the high byte of the \
                     plugin's own FormIDs counts only its full masters, and a master's \
                     scale is never guessed",
                    master.name_lossy()
                )))
            }
        }
    }
    u32::try_from(full)
        .ok()
        .filter(|m| *m < 0xFD)
        .ok_or_else(|| {
            refused(format!(
                "a plugin with {full} full masters has no full-class FormID prefix for its own \
                 records (the high bytes FD and FE mark the medium and small classes)"
            ))
        })
}

/// Whether the stored FormID `form_id` of a plugin whose masters have `master_scales` (in
/// master-list order) names a master's record: `Some(true)` when its class index names a
/// master, `Some(false)` when it names the plugin itself or nothing, `None` when that
/// depends on a master whose scale is unknown. The null FormID names nothing.
pub fn names_master(form_id: u32, master_scales: &[Option<Scale>]) -> Option<bool> {
    if form_id == 0 {
        return Some(false);
    }
    let (class, index) = match form_id >> 24 {
        0xFE => (Scale::Small, (form_id >> 12) & 0xFFF),
        0xFD => (Scale::Medium, (form_id >> 16) & 0xFF),
        high => (Scale::Full, high),
    };
    let mut seen: u32 = 0;
    for scale in master_scales {
        match scale {
            Some(s) if *s == class => {
                if seen == index {
                    return Some(true);
                }
                seen = seen.saturating_add(1);
            }
            Some(_) => {}
            None => return None,
        }
    }
    Some(false)
}

impl NewForms {
    /// The allocator for `header` (a `TES4` record) whose masters have `master_scales` (in
    /// master-list order; see [`own_prefix`]) and whose own records reach object ID
    /// `own_max` (`None` when it has none).
    pub fn new(
        header: &Record<'_>,
        master_scales: &[Option<Scale>],
        own_max: Option<u32>,
    ) -> Result<NewForms> {
        let prefix = own_prefix(header, master_scales)?;
        let tes4 = Tes4::new(header);
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
            hedr_read: hedr.next_object_id,
            blueprint: tes4.count_limitation().is_some(),
            allocated: 0,
        })
    }

    /// The allocator for an indexed plugin (own records: those stored with the prefix
    /// [`own_prefix`] gives).
    pub fn for_index(index: &PluginIndex, master_scales: &[Option<Scale>]) -> Result<NewForms> {
        let prefix = own_prefix(&index.header, master_scales)?;
        let own_max = index
            .records
            .iter()
            .skip(1)
            .filter(|r| r.form_id >> 24 == prefix)
            .map(|r| r.form_id & MAX_FULL_OBJECT_ID)
            .max();
        NewForms::new(&index.header, master_scales, own_max)
    }

    /// The allocator for a plugin in memory.
    pub fn for_plugin(plugin: &Plugin<'_>, master_scales: &[Option<Scale>]) -> Result<NewForms> {
        let prefix = own_prefix(plugin.header(), master_scales)?;
        let mut own_max: Option<u32> = None;
        let mut pending: Vec<&Entry<'_>> = plugin.entries().iter().collect();
        while let Some(entry) = pending.pop() {
            match entry {
                Entry::Group(g) => pending.extend(g.children()),
                Entry::Record(r) => {
                    if r.header.form_id >> 24 == prefix {
                        let id = r.header.form_id & MAX_FULL_OBJECT_ID;
                        own_max = Some(own_max.map_or(id, |m| m.max(id)));
                    }
                }
            }
        }
        NewForms::new(plugin.header(), master_scales, own_max)
    }

    /// The high byte of the FormIDs this allocator hands out.
    pub fn prefix(&self) -> u32 {
        self.prefix
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

    /// A new own FormID (stored form: the plugin's full-master count in the high byte).
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

    /// [`LIMITATIONS`] and what applies to this plugin: a `HEDR` next object ID below
    /// [`USUAL_FIRST_OBJECT_ID`], and the blueprint flag's `HEDR` count meaning.
    pub fn limitations(&self) -> Vec<String> {
        let mut out: Vec<String> = LIMITATIONS.iter().map(|s| s.to_string()).collect();
        let low = self.hedr_read & MAX_FULL_OBJECT_ID;
        if low < USUAL_FIRST_OBJECT_ID {
            out.push(format!(
                "the plugin's HEDR next object ID is {:08X}, below 0x1000 where the official \
                 files and a new plugin start; new FormIDs may land on object IDs the engine \
                 or the Creation Kit treat specially (not known)",
                self.hedr_read
            ));
        }
        if self.blueprint {
            out.push(crate::tes4::BLUEPRINT_COUNT_LIMITATION.to_string());
        }
        out
    }

    /// The `HEDR` next object ID after the forms handed out: the next object ID with the
    /// high byte the header had.
    pub fn hedr_next_object_id(&self) -> u32 {
        (self.hedr_read & !MAX_FULL_OBJECT_ID) | (self.next & MAX_FULL_OBJECT_ID)
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
    use super::{names_master, own_prefix, NewForms, MAX_FULL_OBJECT_ID};
    use crate::build::HeaderSpec;
    use crate::fixture::{self, group, record, sub};
    use crate::index::{index_bytes, IndexOptions};
    use crate::model::GroupHeader;
    use crate::tes4::Scale::{self, Full, Medium, Small};
    use crate::{Error, Plugin, Record, Signature};

    fn rec(sig: &[u8; 4], id: u32) -> Record<'static> {
        let bytes = record(sig, 0, id, &sub(b"EDID", b"x\0"));
        crate::parse::parse_record(&mut crate::Reader::new(&bytes, 0))
            .unwrap()
            .into_owned()
    }

    fn full(n: usize) -> Vec<Option<Scale>> {
        vec![Some(Full); n]
    }

    #[test]
    fn a_new_plugin_starts_at_its_hedr_and_advances_it() {
        let masters = vec![b"Starfield.esm".to_vec(), b"Other.esm".to_vec()];
        let mut plugin = Plugin::create(&HeaderSpec::new(masters));
        let mut forms = NewForms::for_plugin(&plugin, &full(2)).unwrap();
        assert_eq!(forms.peek().unwrap(), 0x0200_1000);
        assert_eq!(forms.allocate().unwrap(), 0x0200_1000);
        assert_eq!(forms.allocate().unwrap(), 0x0200_1001);
        assert_eq!(forms.allocated(), 2);
        assert_eq!(forms.hedr_next_object_id(), 0xFF00_1002);
        assert!(forms.write_hedr(plugin.header_mut()).unwrap());
        let hedr = plugin.tes4().hedr().unwrap().unwrap();
        assert_eq!(hedr.next_object_id, 0xFF00_1002);
        // Deterministic, and the next allocator continues after the header.
        let again = NewForms::for_plugin(&plugin, &full(2)).unwrap();
        assert_eq!(again.peek().unwrap(), 0x0200_1002);
        // Nothing allocated: the header is left alone.
        let none = NewForms::for_plugin(&plugin, &full(2)).unwrap();
        let before = plugin.header().clone();
        assert!(!none.write_hedr(plugin.header_mut()).unwrap());
        assert_eq!(plugin.header(), &before);
        // A new plugin's limitations are the corpus defaults only.
        assert_eq!(none.limitations().len(), super::LIMITATIONS.len());
    }

    /// Review blocker (M4 placement core): the own prefix counts only full masters. A
    /// patch with masters [Starfield.esm (full), Constellation.esm (small), SFBGS003.esm
    /// (medium)] stores its own records 01xxxxxx, not 03xxxxxx (as `SFBGS047.esm` stores
    /// its overrides of its second full master with full index 1).
    #[test]
    fn the_own_prefix_counts_only_full_masters() {
        let masters = vec![
            b"Starfield.esm".to_vec(),
            b"Constellation.esm".to_vec(),
            b"SFBGS003.esm".to_vec(),
        ];
        let scales = [Some(Full), Some(Small), Some(Medium)];
        let mut plugin = Plugin::create(&HeaderSpec::new(masters));
        let top = GroupHeader::top(Signature::new(b"GLOB"));
        // 03xxxxxx is not this plugin's (full index 3 names nothing); 01xxxxxx is.
        plugin.upsert(&[top], rec(b"GLOB", 0x0300_9000)).unwrap();
        plugin.upsert(&[top], rec(b"GLOB", 0x0100_2000)).unwrap();
        let mut forms = NewForms::for_plugin(&plugin, &scales).unwrap();
        assert_eq!(forms.prefix(), 1);
        assert_eq!(forms.allocate().unwrap(), 0x0100_2001);
        let bytes = plugin.to_bytes().unwrap();
        let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
        assert_eq!(
            NewForms::for_index(&index, &scales)
                .unwrap()
                .peek()
                .unwrap(),
            0x0100_2001
        );
        // An unknown master scale is refused, never guessed; so is a wrong-length list.
        for bad in [
            vec![Some(Full), None, Some(Medium)],
            vec![Some(Full), Some(Small)],
        ] {
            let err = NewForms::for_plugin(&plugin, &bad).unwrap_err();
            assert!(matches!(err, Error::Refused { .. }), "{err}");
            assert!(NewForms::for_index(&index, &bad).is_err());
        }
        assert!(own_prefix(plugin.header(), &[None, None, None])
            .unwrap_err()
            .to_string()
            .contains("Starfield.esm"));
    }

    #[test]
    fn names_master_follows_the_class_index() {
        let scales = [Some(Full), Some(Small), Some(Full), Some(Medium)];
        assert_eq!(names_master(0x0000_1234, &scales), Some(true));
        assert_eq!(names_master(0x0100_1234, &scales), Some(true)); // second full master
        assert_eq!(names_master(0x0200_1234, &scales), Some(false)); // the plugin itself
        assert_eq!(names_master(0xFE00_0123, &scales), Some(true));
        assert_eq!(names_master(0xFE00_1123, &scales), Some(false));
        assert_eq!(names_master(0xFD00_0123, &scales), Some(true));
        assert_eq!(names_master(0, &scales), Some(false));
        // An unknown scale decides only what comes after it.
        let unsure = [Some(Full), None];
        assert_eq!(names_master(0x0000_0001, &unsure), Some(true));
        assert_eq!(names_master(0x0100_0001, &unsure), None);
        assert_eq!(names_master(0xFE00_0001, &unsure), None);
    }

    #[test]
    fn own_records_above_the_hedr_push_the_start_up() {
        let mut plugin = Plugin::create(&HeaderSpec::new(vec![b"Starfield.esm".to_vec()]));
        let top = GroupHeader::top(Signature::new(b"GLOB"));
        // An own record above the HEDR value, and a master's record (prefix 00) above both.
        plugin.upsert(&[top], rec(b"GLOB", 0x0100_5000)).unwrap();
        plugin.upsert(&[top], rec(b"GLOB", 0x00FF_0000)).unwrap();
        let forms = NewForms::for_plugin(&plugin, &full(1)).unwrap();
        assert_eq!(forms.peek().unwrap(), 0x0100_5001);
        let bytes = plugin.to_bytes().unwrap();
        let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
        assert_eq!(NewForms::for_index(&index, &full(1)).unwrap(), forms);
    }

    #[test]
    fn scales_master_counts_and_exhaustion_are_refused() {
        for flags in [0x100, 0x400, 0x500, 0x181] {
            let mut spec = HeaderSpec::new(Vec::new());
            spec.flags = flags;
            let plugin = Plugin::create(&spec);
            assert!(
                matches!(
                    NewForms::for_plugin(&plugin, &[]),
                    Err(Error::Refused { .. })
                ),
                "{flags:#x}"
            );
        }
        // The blueprint flag alone is a full-scale plugin, whose HEDR count meaning differs.
        let mut spec = HeaderSpec::new(Vec::new());
        spec.flags = 0x881;
        let blueprint = NewForms::for_plugin(&Plugin::create(&spec), &[]).unwrap();
        assert!(blueprint
            .limitations()
            .iter()
            .any(|l| l.contains("blueprint flag")));
        let many: Vec<Vec<u8>> = (0..253).map(|i| format!("M{i}.esm").into_bytes()).collect();
        let plugin = Plugin::create(&HeaderSpec::new(many));
        assert!(matches!(
            NewForms::for_plugin(&plugin, &full(253)),
            Err(Error::Refused { .. })
        ));
        // 253 masters of which few are full have a prefix.
        let mut scales = vec![Some(Scale::Small); 253];
        scales[0] = Some(Full);
        assert_eq!(NewForms::for_plugin(&plugin, &scales).unwrap().prefix(), 1);
        let mut spec = HeaderSpec::new(Vec::new());
        spec.next_object_id = 0xFF00_0000 | (MAX_FULL_OBJECT_ID - 1);
        let mut forms = NewForms::for_plugin(&Plugin::create(&spec), &[]).unwrap();
        assert_eq!(forms.allocate().unwrap(), MAX_FULL_OBJECT_ID - 1);
        assert_eq!(forms.allocate().unwrap(), MAX_FULL_OBJECT_ID);
        assert!(matches!(forms.allocate(), Err(Error::Refused { .. })));
        assert!(forms.peek().is_err());
        // A low HEDR next object ID is allocated from, and said so.
        let mut spec = HeaderSpec::new(Vec::new());
        spec.next_object_id = 0xFF00_0800;
        let low = NewForms::for_plugin(&Plugin::create(&spec), &[]).unwrap();
        assert_eq!(low.peek().unwrap(), 0x0000_0800);
        assert!(low.limitations().iter().any(|l| l.contains("FF000800")));
        // A header without a HEDR.
        let bare = fixture::plugin(&[]);
        let mut plugin = Plugin::parse(&bare).unwrap().into_owned();
        plugin
            .header_mut()
            .subrecords_mut()
            .unwrap()
            .retain(|s| s.signature() != Signature::new(b"HEDR"));
        let n = plugin.tes4().masters().len();
        assert!(NewForms::for_plugin(&plugin, &full(n)).is_err());
    }

    #[test]
    fn the_sample_plugins_own_records_count() {
        // The fixture plugin has 2 masters; its records are stored 00000xxx (master 0), so
        // none is its own and its HEDR decides.
        let bytes = fixture::sample_plugin();
        let plugin = Plugin::parse(&bytes).unwrap();
        let forms = NewForms::for_plugin(&plugin, &full(2)).unwrap();
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
        let forms = NewForms::for_index(&index, &full(2)).unwrap();
        let expected = (hedr.next_object_id & MAX_FULL_OBJECT_ID).max(0x901);
        assert_eq!(forms.peek().unwrap(), (2 << 24) | expected);
    }
}
