//! FormID spaces for validation: what a stored FormID names, given the plugin's masters.
//!
//! A FormID's top byte indexes the plugin's master list, and the index equal to the
//! master count is the plugin itself (`plan/execution-plan.md`, "Format facts"). When a
//! master is small or medium (TES4 0x100 / 0x400), full-scale FormIDs index only the
//! full-scale masters and `FE`/`FD` FormIDs address the small and medium ones (lane D's
//! `docs/load-order.md`, from SFBGS047 on the corpus). Validation resolves full-scale
//! FormIDs over the full-scale masters it knows the scale of, and reports `FE`/`FD`
//! FormIDs as unchecked (their layout is lane D's): an unknown scale is never guessed.

use kitcraft_esm::tes4::{FLAG_MEDIUM, FLAG_SMALL};

/// Whether TES4 flags mark a small or medium plugin.
pub fn scaled(flags: u32) -> bool {
    flags & (FLAG_SMALL | FLAG_MEDIUM) != 0
}

/// A plugin's master list with what is known about each master's scale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    /// The plugin's file name.
    pub name: String,
    /// Whether the plugin itself is small or medium.
    pub scaled: bool,
    /// Masters in order: (file name, TES4 flags if the header was read).
    pub masters: Vec<(String, Option<u32>)>,
}

/// What a stored FormID names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// FormID 0: no form.
    Null,
    /// A form of the master (or the plugin itself, `owner == name`) with this object ID.
    Form {
        /// File name of the plugin that owns it.
        owner: String,
        /// Object ID (low 24 bits).
        object_id: u32,
        /// Index in the plugin's master list (`masters.len()` for the plugin itself).
        master_index: usize,
    },
    /// The index names no master: an error.
    Beyond {
        /// The index.
        index: u32,
        /// How many full-scale slots there are.
        full: usize,
    },
    /// Cannot be resolved without guessing (a master of unknown scale comes first, or a
    /// small/medium `FE`/`FD` FormID).
    Unknown(&'static str),
}

impl Space {
    /// What `raw` names in this plugin.
    pub fn resolve(&self, raw: u32) -> Target {
        if raw == 0 {
            return Target::Null;
        }
        let top = raw >> 24;
        if top == 0xFE || top == 0xFD {
            return Target::Unknown(
                "small or medium (FE/FD) FormIDs need lane D's scale-aware resolution",
            );
        }
        let mut slot: u32 = 0;
        for (i, (name, flags)) in self.masters.iter().enumerate() {
            match flags {
                None => {
                    return Target::Unknown(
                        "a master of unknown scale (missing) precedes this index",
                    );
                }
                Some(f) if scaled(*f) => continue,
                Some(_) => {}
            }
            if slot == top {
                return Target::Form {
                    owner: name.clone(),
                    object_id: raw & 0x00FF_FFFF,
                    master_index: i,
                };
            }
            slot = slot.saturating_add(1);
        }
        if slot == top {
            if self.scaled {
                return Target::Unknown("a small or medium plugin's own forms are FE/FD FormIDs");
            }
            return Target::Form {
                owner: self.name.clone(),
                object_id: raw & 0x00FF_FFFF,
                master_index: self.masters.len(),
            };
        }
        Target::Beyond {
            index: top,
            full: usize::try_from(slot).unwrap_or(usize::MAX),
        }
    }

    /// The FormID this plugin stores for `owner`'s object `object_id`, when it can be said
    /// without guessing (full-scale owner, every earlier master's scale known).
    pub fn raw_for(&self, owner: &str, object_id: u32) -> Option<u32> {
        let mut slot: u32 = 0;
        for (name, flags) in &self.masters {
            let f = (*flags)?;
            if scaled(f) {
                if name.eq_ignore_ascii_case(owner) {
                    return None;
                }
                continue;
            }
            if name.eq_ignore_ascii_case(owner) {
                return Some((slot << 24) | object_id);
            }
            slot = slot.checked_add(1)?;
        }
        (owner.eq_ignore_ascii_case(&self.name) && !self.scaled).then_some((slot << 24) | object_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(masters: &[(&str, Option<u32>)], scaled: bool) -> Space {
        Space {
            name: "Me.esp".into(),
            scaled,
            masters: masters
                .iter()
                .map(|(n, f)| ((*n).to_string(), *f))
                .collect(),
        }
    }

    #[test]
    fn full_scale_indices_skip_small_masters() {
        // SFBGS047's shape: masters starfield (full), sfbgs004 (small), sfbgs00d (full).
        let s = space(
            &[
                ("Starfield.esm", Some(0x81)),
                ("SFBGS004.esm", Some(0x181)),
                ("SFBGS00D.esm", Some(0x81)),
            ],
            false,
        );
        assert_eq!(
            s.resolve(0x0100_0ABC),
            Target::Form {
                owner: "SFBGS00D.esm".into(),
                object_id: 0xABC,
                master_index: 2
            }
        );
        assert_eq!(
            s.resolve(0x0200_0001),
            Target::Form {
                owner: "Me.esp".into(),
                object_id: 1,
                master_index: 3
            }
        );
        assert_eq!(s.resolve(0x0300_0001), Target::Beyond { index: 3, full: 2 });
        assert_eq!(s.resolve(0), Target::Null);
        assert!(matches!(s.resolve(0xFE00_1001), Target::Unknown(_)));
        assert_eq!(s.raw_for("sfbgs00d.esm", 5), Some(0x0100_0005));
        assert_eq!(s.raw_for("SFBGS004.esm", 5), None);
        assert_eq!(s.raw_for("Me.esp", 5), Some(0x0200_0005));
    }

    #[test]
    fn unknown_scale_is_not_guessed() {
        let s = space(
            &[
                ("A.esm", Some(1)),
                ("Missing.esm", None),
                ("C.esm", Some(1)),
            ],
            false,
        );
        assert!(matches!(s.resolve(0x0000_0001), Target::Form { .. }));
        assert!(matches!(s.resolve(0x0100_0001), Target::Unknown(_)));
        assert_eq!(s.raw_for("C.esm", 1), None);
        let small = space(&[("A.esm", Some(1))], true);
        assert!(matches!(small.resolve(0x0100_0001), Target::Unknown(_)));
    }
}
