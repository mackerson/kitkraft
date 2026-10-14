//! FormID spaces for validation: what a stored FormID names, given the plugin's masters.
//!
//! A FormID's top bits index one *class* of the plugin's master list, and the index equal
//! to the class's master count, in the plugin's own class, is the plugin itself
//! (`plan/execution-plan.md`, "Format facts"). Full-scale FormIDs index the full-scale
//! masters; `FE` (small) and `FD` (medium) FormIDs index the small and the medium masters
//! (lane D's encoding, [`kitcraft_doc::RawRef`], confirmed on the corpus:
//! `docs/load-order.md`, "FormIDs"). A class index can be counted only when the scale of
//! every master before it is known, so a missing master before the index leaves the FormID
//! unresolved: an unknown scale is never guessed. A small or medium plugin's own record
//! stored in the full class (lane D's assumption `full-style-self`) is an assumption, so it
//! stays unresolved here too.

use kitcraft_doc::{RawRef, Scale};
use kitcraft_esm::tes4::{FLAG_MEDIUM, FLAG_SMALL};

/// Whether TES4 flags mark a small or medium plugin.
pub fn scaled(flags: u32) -> bool {
    flags & (FLAG_SMALL | FLAG_MEDIUM) != 0
}

/// The scale TES4 flags give (`None` when both scale bits are set: no scale is known).
pub fn scale_of(flags: u32) -> Option<Scale> {
    match (flags & FLAG_SMALL != 0, flags & FLAG_MEDIUM != 0) {
        (false, false) => Some(Scale::Full),
        (true, false) => Some(Scale::Small),
        (false, true) => Some(Scale::Medium),
        (true, true) => None,
    }
}

/// A plugin's master list with what is known about each master's scale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    /// The plugin's file name.
    pub name: String,
    /// The plugin's own TES4 flags, when its header was read.
    pub flags: Option<u32>,
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
        /// Object ID (24 bits full, 16 medium, 12 small).
        object_id: u32,
        /// Index in the plugin's master list (`masters.len()` for the plugin itself).
        master_index: usize,
    },
    /// The index names no master of its class and not the plugin itself: an error.
    Beyond {
        /// The class index read from the FormID.
        index: u32,
        /// How many masters of that class the plugin has.
        count: usize,
        /// The class (`full`, `medium`, `small`).
        class: &'static str,
    },
    /// Cannot be resolved without guessing (a master of unknown scale comes first, or a
    /// small/medium plugin's own record stored full-style).
    Unknown(&'static str),
}

impl Space {
    /// Whether the plugin itself is small or medium (known from its flags).
    pub fn scaled(&self) -> bool {
        self.flags.is_some_and(scaled)
    }

    /// The plugin's own scale (`None`: its header was not read, or both bits are set).
    fn own_scale(&self) -> Option<Scale> {
        self.flags.and_then(scale_of)
    }

    /// What `raw` names in this plugin.
    pub fn resolve(&self, raw: u32) -> Target {
        if raw == 0 {
            return Target::Null;
        }
        let r = RawRef::decode(raw);
        let mut count: u16 = 0;
        for (i, (name, flags)) in self.masters.iter().enumerate() {
            let Some(scale) = flags.and_then(scale_of) else {
                return Target::Unknown(
                    "a master of unknown scale (missing, or both scale bits set) precedes this index",
                );
            };
            if scale != r.class {
                continue;
            }
            if count == r.index {
                return Target::Form {
                    owner: name.clone(),
                    object_id: r.object_id,
                    master_index: i,
                };
            }
            count = count.saturating_add(1);
        }
        if count == r.index {
            match self.own_scale() {
                Some(own) if own == r.class => {
                    return Target::Form {
                        owner: self.name.clone(),
                        object_id: r.object_id,
                        master_index: self.masters.len(),
                    };
                }
                Some(_) if r.class == Scale::Full => {
                    return Target::Unknown(
                        "a small or medium plugin's own record stored full-style (lane D's assumption full-style-self; not guessed here)",
                    );
                }
                None => return Target::Unknown("the plugin's own scale is not known"),
                Some(_) => {}
            }
        }
        Target::Beyond {
            index: u32::from(r.index),
            count: usize::from(count),
            class: r.class.as_str(),
        }
    }

    /// The FormID this plugin stores for `owner`'s object `object_id`, when it can be said
    /// without guessing (the scale of every master up to the owner known, the object ID
    /// within the owner's scale).
    pub fn raw_for(&self, owner: &str, object_id: u32) -> Option<u32> {
        let mut counts = [0u16; 3];
        let slot = |s: Scale| match s {
            Scale::Full => 0usize,
            Scale::Medium => 1,
            Scale::Small => 2,
        };
        for (name, flags) in &self.masters {
            let scale = flags.and_then(scale_of)?;
            let n = counts.get_mut(slot(scale))?;
            if name.eq_ignore_ascii_case(owner) {
                return RawRef {
                    class: scale,
                    index: *n,
                    object_id,
                }
                .encode();
            }
            *n = n.checked_add(1)?;
        }
        if !owner.eq_ignore_ascii_case(&self.name) {
            return None;
        }
        let own = self.own_scale()?;
        RawRef {
            class: own,
            index: *counts.get(slot(own))?,
            object_id,
        }
        .encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: u32 = 0x81;
    const SMALL: u32 = 0x181;
    const MEDIUM: u32 = 0x481;

    fn space(masters: &[(&str, Option<u32>)], flags: u32) -> Space {
        Space {
            name: "Me.esp".into(),
            flags: Some(flags),
            masters: masters
                .iter()
                .map(|(n, f)| ((*n).to_string(), *f))
                .collect(),
        }
    }

    fn form(owner: &str, object_id: u32, master_index: usize) -> Target {
        Target::Form {
            owner: owner.into(),
            object_id,
            master_index,
        }
    }

    #[test]
    fn full_scale_indices_skip_small_masters() {
        // SFBGS047's shape: masters starfield (full), sfbgs004 (small), sfbgs00d (full).
        let s = space(
            &[
                ("Starfield.esm", Some(FULL)),
                ("SFBGS004.esm", Some(SMALL)),
                ("SFBGS00D.esm", Some(FULL)),
            ],
            0,
        );
        assert_eq!(s.resolve(0x0100_0ABC), form("SFBGS00D.esm", 0xABC, 2));
        assert_eq!(s.resolve(0x0200_0001), form("Me.esp", 1, 3));
        assert_eq!(
            s.resolve(0x0300_0001),
            Target::Beyond {
                index: 3,
                count: 2,
                class: "full"
            }
        );
        assert_eq!(s.resolve(0), Target::Null);
        assert_eq!(s.raw_for("sfbgs00d.esm", 5), Some(0x0100_0005));
        assert_eq!(s.raw_for("Me.esp", 5), Some(0x0200_0005));
    }

    /// M3.E3: `FE`/`FD` FormIDs resolve through lane D's class encoding instead of being
    /// counted as unresolved.
    #[test]
    fn small_and_medium_classes_resolve() {
        // Constellation-like (small), SFBGS006-like (medium) and a second small master.
        let s = space(
            &[
                ("Starfield.esm", Some(FULL)),
                ("Small0.esm", Some(SMALL)),
                ("Medium0.esm", Some(MEDIUM)),
                ("Small1.esm", Some(SMALL)),
            ],
            0,
        );
        assert_eq!(s.resolve(0xFE00_0ABC), form("Small0.esm", 0xABC, 1));
        assert_eq!(s.resolve(0xFE00_1ABC), form("Small1.esm", 0xABC, 3));
        assert_eq!(s.resolve(0xFD00_1234), form("Medium0.esm", 0x1234, 2));
        assert_eq!(
            s.resolve(0xFE00_2001),
            Target::Beyond {
                index: 2,
                count: 2,
                class: "small"
            }
        );
        assert_eq!(
            s.resolve(0xFD01_0001),
            Target::Beyond {
                index: 1,
                count: 1,
                class: "medium"
            }
        );
        assert_eq!(s.raw_for("small1.esm", 5), Some(0xFE00_1005));
        assert_eq!(s.raw_for("Medium0.esm", 5), Some(0xFD00_0005));
        // An object ID beyond the owner's scale has no raw form.
        assert_eq!(s.raw_for("Small0.esm", 0x1000), None);
        // A small plugin's own forms: FE with its own small index.
        let me = space(
            &[("Starfield.esm", Some(FULL)), ("S.esm", Some(SMALL))],
            SMALL,
        );
        assert_eq!(me.resolve(0xFE00_1007), form("Me.esp", 7, 2));
        assert_eq!(me.raw_for("Me.esp", 7), Some(0xFE00_1007));
        // ...and full-style-self is lane D's assumption: not guessed.
        assert!(matches!(me.resolve(0x0100_0007), Target::Unknown(_)));
    }

    #[test]
    fn unknown_scale_is_not_guessed() {
        let s = space(
            &[
                ("A.esm", Some(1)),
                ("Missing.esm", None),
                ("C.esm", Some(1)),
            ],
            0,
        );
        assert!(matches!(s.resolve(0x0000_0001), Target::Form { .. }));
        assert!(matches!(s.resolve(0x0100_0001), Target::Unknown(_)));
        assert!(matches!(s.resolve(0xFE00_0001), Target::Unknown(_)));
        assert_eq!(s.raw_for("C.esm", 1), None);
        let both = space(&[("A.esm", Some(0x581))], 0);
        assert!(matches!(both.resolve(0x0000_0001), Target::Unknown(_)));
        let unread = Space {
            name: "Me.esp".into(),
            flags: None,
            masters: vec![("A.esm".into(), Some(1))],
        };
        assert!(matches!(unread.resolve(0x0100_0001), Target::Unknown(_)));
    }
}
