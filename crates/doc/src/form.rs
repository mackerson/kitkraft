//! FormIDs: raw (as serialized in one file) versus resolved (master-aware) identities.
//!
//! A raw FormID means nothing on its own: its high bits index a *class* of the file's
//! masters, and which class a master belongs to depends on that master's scale, which only
//! the master's own header says. So resolution needs the master graph loaded
//! ([`MasterMap`]); a missing master is an error, never a guess.
//!
//! Encoding of a raw FormID in a file whose masters are `M0..Mn-1` (hypothesis from Ortham's
//! 2024 article, `docs/load-order.md` "FormIDs"; confirmed on the corpus by
//! `tests/corpus.rs`):
//!
//! | High byte | Class  | Index bits      | Object bits |
//! | --------- | ------ | --------------- | ----------- |
//! | `FE`      | small  | 12 (bits 12-23) | 12          |
//! | `FD`      | medium | 8 (bits 16-23)  | 16          |
//! | other     | full   | 8 (bits 24-31)  | 24          |
//!
//! The index counts the file's masters *of that class only*, in master-list order; the
//! index equal to that count, in the file's own class, is the file itself.

use crate::flags::Scale;

/// A resolved identity: the plugin that defines the form (its load-order index) and the
/// object ID within that plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlobalForm {
    /// Load-order index of the defining plugin.
    pub plugin: u16,
    /// Object ID (at most 24 bits; 16 for medium and 12 for small plugins).
    pub object_id: u32,
}

impl GlobalForm {
    /// Packed into one sortable key.
    pub fn key(self) -> u64 {
        (u64::from(self.plugin) << 32) | u64::from(self.object_id)
    }

    /// Inverse of [`GlobalForm::key`].
    pub fn from_key(key: u64) -> GlobalForm {
        GlobalForm {
            plugin: u16::try_from(key >> 32).unwrap_or(u16::MAX),
            object_id: u32::try_from(key & 0xFFFF_FFFF).unwrap_or(u32::MAX),
        }
    }
}

/// A raw FormID split into its class, class index and object ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawRef {
    /// Which master class the high bits address.
    pub class: Scale,
    /// Index into the file's masters of that class.
    pub index: u16,
    /// Object ID.
    pub object_id: u32,
}

impl RawRef {
    /// Split a raw FormID.
    pub fn decode(raw: u32) -> RawRef {
        let high = raw >> 24;
        match high {
            0xFE => RawRef {
                class: Scale::Small,
                index: u16::try_from((raw >> 12) & 0xFFF).unwrap_or(0),
                object_id: raw & 0xFFF,
            },
            0xFD => RawRef {
                class: Scale::Medium,
                index: u16::try_from((raw >> 16) & 0xFF).unwrap_or(0),
                object_id: raw & 0xFFFF,
            },
            _ => RawRef {
                class: Scale::Full,
                index: u16::try_from(high).unwrap_or(0),
                object_id: raw & 0x00FF_FFFF,
            },
        }
    }

    /// Join into a raw FormID, or `None` when the index or object ID does not fit the class
    /// (full indices stop at `FC`: `FD` and `FE` are the other classes' prefixes).
    pub fn encode(self) -> Option<u32> {
        if self.object_id > self.class.max_object_id() {
            return None;
        }
        let index = u32::from(self.index);
        match self.class {
            Scale::Full => (index <= 0xFC).then_some((index << 24) | self.object_id),
            Scale::Medium => {
                (index <= 0xFF).then_some(0xFD00_0000 | (index << 16) | self.object_id)
            }
            Scale::Small => {
                (index <= 0xFFF).then_some(0xFE00_0000 | (index << 12) | self.object_id)
            }
        }
    }
}

/// Why a raw FormID could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// A master of the file is not in the load order, so the classes of all its masters
    /// (and its own index) are unknown.
    #[error("master `{master}` of `{plugin}` is not loaded: its scale is unknown, so FormIDs of `{plugin}` cannot be resolved")]
    MissingMaster {
        /// The file whose FormID was resolved.
        plugin: String,
        /// The first missing master.
        master: String,
    },
    /// A master (or the file itself) sets both scale bits.
    #[error("`{plugin}` sets both the small and the medium flag; no scale is known for that")]
    BadScale {
        /// The plugin with the conflicting bits.
        plugin: String,
    },
    /// The index is past the masters of its class and is not the file itself.
    #[error("FormID {raw:08X} in `{plugin}` addresses {class} master #{index}, but the file has {count} {class} master(s)")]
    IndexOutOfRange {
        /// The file.
        plugin: String,
        /// Raw FormID.
        raw: u32,
        /// Class name.
        class: &'static str,
        /// Index read.
        index: u16,
        /// Masters of that class.
        count: usize,
    },
}

/// Per-file table turning raw FormIDs into [`GlobalForm`]s and back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterMap {
    /// File name (for errors).
    pub plugin_name: String,
    /// Load-order index of the file itself.
    pub own_index: u16,
    /// The file's own scale.
    pub own_scale: Scale,
    /// Load-order indices of the full masters, in master-list order.
    pub full: Vec<u16>,
    /// Load-order indices of the medium masters.
    pub medium: Vec<u16>,
    /// Load-order indices of the small masters.
    pub small: Vec<u16>,
}

impl MasterMap {
    /// Build from the masters' load-order indices and scales, in master-list order.
    pub fn new(
        plugin_name: &str,
        own_index: u16,
        own_scale: Scale,
        masters: &[(u16, Scale)],
    ) -> MasterMap {
        let mut map = MasterMap {
            plugin_name: plugin_name.to_string(),
            own_index,
            own_scale,
            full: Vec::new(),
            medium: Vec::new(),
            small: Vec::new(),
        };
        for &(index, scale) in masters {
            map.class_mut(scale).push(index);
        }
        map
    }

    fn class(&self, scale: Scale) -> &[u16] {
        match scale {
            Scale::Full => &self.full,
            Scale::Medium => &self.medium,
            Scale::Small => &self.small,
        }
    }

    fn class_mut(&mut self, scale: Scale) -> &mut Vec<u16> {
        match scale {
            Scale::Full => &mut self.full,
            Scale::Medium => &mut self.medium,
            Scale::Small => &mut self.small,
        }
    }

    /// Resolve a raw FormID read from this file.
    pub fn resolve(&self, raw: u32) -> Result<GlobalForm, ResolveError> {
        let r = RawRef::decode(raw);
        let list = self.class(r.class);
        let index = usize::from(r.index);
        if let Some(&plugin) = list.get(index) {
            return Ok(GlobalForm {
                plugin,
                object_id: r.object_id,
            });
        }
        if index == list.len() && r.class == self.own_scale {
            return Ok(GlobalForm {
                plugin: self.own_index,
                object_id: r.object_id,
            });
        }
        Err(ResolveError::IndexOutOfRange {
            plugin: self.plugin_name.clone(),
            raw,
            class: r.class.as_str(),
            index: r.index,
            count: list.len(),
        })
    }

    /// The raw FormID this file would store for `form`, or `None` when the defining plugin
    /// is neither this file nor one of its masters, or the object ID does not fit.
    pub fn to_raw(&self, form: GlobalForm) -> Option<u32> {
        if form.plugin == self.own_index {
            let index = u16::try_from(self.class(self.own_scale).len()).ok()?;
            return RawRef {
                class: self.own_scale,
                index,
                object_id: form.object_id,
            }
            .encode();
        }
        for class in [Scale::Full, Scale::Medium, Scale::Small] {
            if let Some(pos) = self.class(class).iter().position(|&p| p == form.plugin) {
                return RawRef {
                    class,
                    index: u16::try_from(pos).ok()?,
                    object_id: form.object_id,
                }
                .encode();
            }
        }
        None
    }
}

/// Runtime load-order slot of an active plugin (see `docs/load-order.md`, "Runtime IDs";
/// an assumption until observed in game or in the CK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSlot {
    /// `00`..`FC`.
    Full(u8),
    /// `FD` + `00`..`FF`.
    Medium(u8),
    /// `FE` + `000`..`FFF`.
    Small(u16),
}

impl RuntimeSlot {
    /// The runtime FormID of `object_id` in this slot, or `None` if it does not fit.
    pub fn runtime_id(self, object_id: u32) -> Option<u32> {
        let (class, index) = match self {
            RuntimeSlot::Full(i) => (Scale::Full, u16::from(i)),
            RuntimeSlot::Medium(i) => (Scale::Medium, u16::from(i)),
            RuntimeSlot::Small(i) => (Scale::Small, i),
        };
        RawRef {
            class,
            index,
            object_id,
        }
        .encode()
    }

    /// The slot as text: `05`, `FD:01`, `FE:00A`.
    pub fn label(self) -> String {
        match self {
            RuntimeSlot::Full(i) => format!("{i:02X}"),
            RuntimeSlot::Medium(i) => format!("FD:{i:02X}"),
            RuntimeSlot::Small(i) => format!("FE:{i:03X}"),
        }
    }
}

/// Assign runtime slots to active plugins in load order by scale. `None` for an inactive
/// plugin, one without a known scale, or past the end of its index space.
pub fn runtime_slots(plugins: &[(bool, Option<Scale>)]) -> Vec<Option<RuntimeSlot>> {
    let (mut full, mut medium, mut small) = (0u16, 0u16, 0u16);
    plugins
        .iter()
        .map(|&(active, scale)| {
            if !active {
                return None;
            }
            let slot = match scale? {
                Scale::Full => u8::try_from(full)
                    .ok()
                    .filter(|i| *i <= 0xFC)
                    .map(RuntimeSlot::Full),
                Scale::Medium => u8::try_from(medium).ok().map(RuntimeSlot::Medium),
                Scale::Small => (small <= 0xFFF).then_some(RuntimeSlot::Small(small)),
            };
            match scale? {
                Scale::Full => full = full.saturating_add(1),
                Scale::Medium => medium = medium.saturating_add(1),
                Scale::Small => small = small.saturating_add(1),
            }
            slot
        })
        .collect()
}

/// A FormID typed as `Plugin.esm:XXXXXX` (defining plugin and object ID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormRef {
    /// Plugin file name as written.
    pub plugin: String,
    /// Object ID.
    pub object_id: u32,
}

impl FormRef {
    /// Parse `Plugin.esm:0001A2B3`. The hex part is 1-8 digits; with 7 or 8 digits the
    /// digits above the low 24 bits must be zero (the plugin name says which file).
    pub fn parse(text: &str) -> Option<FormRef> {
        let (plugin, hex) = text.rsplit_once(':')?;
        let lower = plugin.to_ascii_lowercase();
        if plugin.is_empty()
            || !(lower.ends_with(".esm") || lower.ends_with(".esp") || lower.ends_with(".esl"))
        {
            return None;
        }
        let digits = hex
            .strip_prefix("0x")
            .or_else(|| hex.strip_prefix("0X"))
            .unwrap_or(hex);
        if digits.is_empty() || digits.len() > 8 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let value = u32::from_str_radix(digits, 16).ok()?;
        if value > 0x00FF_FFFF {
            return None;
        }
        Some(FormRef {
            plugin: plugin.to_string(),
            object_id: value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_decode_and_encode() {
        let small = RawRef::decode(0xFE00_1013);
        assert_eq!(
            small,
            RawRef {
                class: Scale::Small,
                index: 1,
                object_id: 0x013
            }
        );
        assert_eq!(small.encode(), Some(0xFE00_1013));
        let medium = RawRef::decode(0xFD01_0806);
        assert_eq!(
            (medium.class, medium.index, medium.object_id),
            (Scale::Medium, 1, 0x806)
        );
        assert_eq!(medium.encode(), Some(0xFD01_0806));
        let full = RawRef::decode(0x0200_1570);
        assert_eq!(
            (full.class, full.index, full.object_id),
            (Scale::Full, 2, 0x1570)
        );
        assert_eq!(full.encode(), Some(0x0200_1570));
        // Out of range.
        let bad = RawRef {
            class: Scale::Small,
            index: 0,
            object_id: 0x1000,
        };
        assert_eq!(bad.encode(), None);
        let bad = RawRef {
            class: Scale::Full,
            index: 0xFD,
            object_id: 1,
        };
        assert_eq!(bad.encode(), None);
    }

    /// The SFBGS047 shape: masters starfield (full), sfbgs004 (small), sfbgs00d (full);
    /// the file itself is small.
    fn sfbgs047_like() -> MasterMap {
        MasterMap::new(
            "SFBGS047.esm",
            7,
            Scale::Small,
            &[(0, Scale::Full), (3, Scale::Small), (5, Scale::Full)],
        )
    }

    #[test]
    fn resolve_by_class_index() {
        let map = sfbgs047_like();
        assert_eq!(
            map.resolve(0x0000_1234),
            Ok(GlobalForm {
                plugin: 0,
                object_id: 0x1234
            })
        );
        // Full index 1 is the second *full* master (sfbgs00d), not master-list entry 1.
        assert_eq!(map.resolve(0x0100_79E3).map(|g| g.plugin), Ok(5));
        assert_eq!(map.resolve(0xFE00_0805).map(|g| g.plugin), Ok(3));
        // Small index 1 == small master count, own class small: the file itself.
        assert_eq!(
            map.resolve(0xFE00_1013),
            Ok(GlobalForm {
                plugin: 7,
                object_id: 0x013
            })
        );
        // Full index 2 == full count, but the file is not full.
        assert!(matches!(
            map.resolve(0x0200_0001),
            Err(ResolveError::IndexOutOfRange { count: 2, .. })
        ));
        assert!(map.resolve(0xFD00_0001).is_err());
        assert!(map.resolve(0xFE00_2001).is_err());
    }

    #[test]
    fn to_raw_is_the_inverse() {
        let map = sfbgs047_like();
        for raw in [0x0000_1234u32, 0x0100_79E3, 0xFE00_0805, 0xFE00_1013] {
            let g = map.resolve(raw).unwrap();
            assert_eq!(map.to_raw(g), Some(raw));
        }
        assert_eq!(
            map.to_raw(GlobalForm {
                plugin: 9,
                object_id: 1
            }),
            None
        );
        // Own records of a small file cannot exceed 12 bits.
        assert_eq!(
            map.to_raw(GlobalForm {
                plugin: 7,
                object_id: 0x1000
            }),
            None
        );
    }

    #[test]
    fn runtime_slots_count_per_scale() {
        let slots = runtime_slots(&[
            (true, Some(Scale::Full)),
            (true, Some(Scale::Small)),
            (false, Some(Scale::Full)),
            (true, Some(Scale::Medium)),
            (true, Some(Scale::Full)),
            (true, Some(Scale::Small)),
            (true, None),
        ]);
        assert_eq!(
            slots,
            [
                Some(RuntimeSlot::Full(0)),
                Some(RuntimeSlot::Small(0)),
                None,
                Some(RuntimeSlot::Medium(0)),
                Some(RuntimeSlot::Full(1)),
                Some(RuntimeSlot::Small(1)),
                None
            ]
        );
        assert_eq!(RuntimeSlot::Small(1).runtime_id(0x806), Some(0xFE00_1806));
        assert_eq!(RuntimeSlot::Medium(1).runtime_id(0x1808), Some(0xFD01_1808));
        assert_eq!(RuntimeSlot::Full(5).runtime_id(0x1A2B3), Some(0x0501_A2B3));
        assert_eq!(RuntimeSlot::Small(0).runtime_id(0x1000), None);
        assert_eq!(RuntimeSlot::Small(0x00A).label(), "FE:00A");
    }

    #[test]
    fn formref_parse() {
        assert_eq!(
            FormRef::parse("Starfield.esm:0001A2B3"),
            Some(FormRef {
                plugin: "Starfield.esm".into(),
                object_id: 0x1A2B3
            })
        );
        assert_eq!(FormRef::parse("a.esp:0x10").unwrap().object_id, 0x10);
        assert_eq!(FormRef::parse("Starfield.esm:0101A2B3"), None);
        assert_eq!(FormRef::parse("Starfield:0001A2B3"), None);
        assert_eq!(FormRef::parse("Starfield.esm:"), None);
        assert_eq!(FormRef::parse("0001A2B3"), None);
        assert_eq!(
            GlobalForm::from_key(
                GlobalForm {
                    plugin: 3,
                    object_id: 9
                }
                .key()
            ),
            GlobalForm {
                plugin: 3,
                object_id: 9
            }
        );
    }
}
