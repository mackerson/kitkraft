//! TES4 header flags and the plugin *scale*.
//!
//! Sources (`docs/load-order.md`, "Plugin flags and scale"):
//! - bit values: `xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e`, the
//!   `TES4` record's flag list (Master 0x001, Optimized 0x010, Localized 0x080, Small 0x100,
//!   Update 0x200, Medium 0x400, Blueprint 0x800), as relayed by the owner's dossier
//!   (`docs/research/starfield-dossier-2026-10-14.md` section 2);
//! - corpus: 0x100 and 0x400 each occur alone, never together (`docs/corpus.md`).
//!
//! Scale (full / medium / small) is one property; master, update and blueprint are separate
//! flags and are deliberately not folded into the same enum.

/// Master file.
pub const FLAG_MASTER: u32 = 0x0000_0001;
/// Optimized file.
pub const FLAG_OPTIMIZED: u32 = 0x0000_0010;
/// Localized: lstring subrecords hold string-table IDs.
pub const FLAG_LOCALIZED: u32 = 0x0000_0080;
/// Small scale (`FE` index space).
pub const FLAG_SMALL: u32 = 0x0000_0100;
/// Update plugin.
pub const FLAG_UPDATE: u32 = 0x0000_0200;
/// Medium scale (`FD` index space).
pub const FLAG_MEDIUM: u32 = 0x0000_0400;
/// Blueprint plugin.
pub const FLAG_BLUEPRINT: u32 = 0x0000_0800;

/// Record flag 0x20: the record is deleted (UESP, Skyrim_Mod:Mod_File_Format, "Records";
/// a container fact).
pub const RECORD_FLAG_DELETED: u32 = 0x0000_0020;

/// How a plugin's own records are numbered and which runtime index space it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scale {
    /// One byte of mod index, 24-bit object IDs.
    Full,
    /// `FD` + one byte of medium index, 16-bit object IDs.
    Medium,
    /// `FE` + 12 bits of small index, 12-bit object IDs.
    Small,
}

impl Scale {
    /// Lower-case name used in results.
    pub fn as_str(self) -> &'static str {
        match self {
            Scale::Full => "full",
            Scale::Medium => "medium",
            Scale::Small => "small",
        }
    }

    /// Largest object ID a record of this scale can carry.
    pub fn max_object_id(self) -> u32 {
        match self {
            Scale::Full => 0x00FF_FFFF,
            Scale::Medium => 0xFFFF,
            Scale::Small => 0x0FFF,
        }
    }
}

/// The raw TES4 flags with named accessors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PluginFlags(pub u32);

impl PluginFlags {
    /// Master bit.
    pub fn master(self) -> bool {
        self.0 & FLAG_MASTER != 0
    }
    /// Localized bit.
    pub fn localized(self) -> bool {
        self.0 & FLAG_LOCALIZED != 0
    }
    /// Update bit.
    pub fn update(self) -> bool {
        self.0 & FLAG_UPDATE != 0
    }
    /// Blueprint bit.
    pub fn blueprint(self) -> bool {
        self.0 & FLAG_BLUEPRINT != 0
    }

    /// The scale, or `None` when both the small and the medium bit are set (no meaning is
    /// known for that combination; resolution through such a plugin is refused).
    pub fn scale(self) -> Option<Scale> {
        match (self.0 & FLAG_SMALL != 0, self.0 & FLAG_MEDIUM != 0) {
            (false, false) => Some(Scale::Full),
            (true, false) => Some(Scale::Small),
            (false, true) => Some(Scale::Medium),
            (true, true) => None,
        }
    }

    /// Names of the set bits this module knows, plus `0x...` for the others.
    pub fn names(self) -> Vec<String> {
        const NAMED: [(u32, &str); 7] = [
            (FLAG_MASTER, "master"),
            (FLAG_OPTIMIZED, "optimized"),
            (FLAG_LOCALIZED, "localized"),
            (FLAG_SMALL, "small"),
            (FLAG_UPDATE, "update"),
            (FLAG_MEDIUM, "medium"),
            (FLAG_BLUEPRINT, "blueprint"),
        ];
        let mut out = Vec::new();
        let mut rest = self.0;
        for (bit, name) in NAMED {
            if self.0 & bit != 0 {
                out.push(name.to_string());
                rest &= !bit;
            }
        }
        for i in 0..32u32 {
            let bit = 1u32 << i;
            if rest & bit != 0 {
                out.push(format!("0x{bit:X}"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_from_flags() {
        assert_eq!(PluginFlags(0x81).scale(), Some(Scale::Full));
        assert_eq!(PluginFlags(0x181).scale(), Some(Scale::Small));
        assert_eq!(PluginFlags(0x481).scale(), Some(Scale::Medium));
        assert_eq!(PluginFlags(0x881).scale(), Some(Scale::Full));
        assert_eq!(PluginFlags(0x501).scale(), None);
        assert!(PluginFlags(0x881).blueprint());
        assert!(!PluginFlags(0x881).update());
        assert_eq!(
            PluginFlags(0x1_0281).names(),
            ["master", "localized", "update", "0x10000"]
        );
    }
}
