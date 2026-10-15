//! Which plugins a profile loads, in which order, from which files.
//!
//! Each rule is named so results can say why a plugin is active; the evidence and what is
//! still unverified is in `docs/load-order.md` ("Active plugins").
//!
//! Order (assumption `hardcoded-official-slots`, `docs/load-order.md` "Order"; the order
//! within each scale is the one the owner's saves record, game 1.16.244.0): the official
//! plugins first, in [`OFFICIAL_ORDER`] (official ones it does not name after it, in the
//! ccc's order, then `loadorder.txt`'s); then every other plugin in `loadorder.txt` order.
//! MO2's `loadorder.txt` can list unmanaged DLC after managed plugins that name them as
//! masters, and the ccc lists `SFBGS006` before `SFBGS003`; the game follows neither.
//!
//! The blueprint (TES4 flag 0x800) plugins take the last full *runtime slots*, after every
//! user plugin (`kitkraft_doc::form::runtime_slots`). They stay with the official plugins
//! here: ten user plugins on the owner's `Default` profile name `BlueprintShips-Starfield`
//! as a master, and which version of a form the game uses when such a plugin overrides a
//! blueprint record is not observed; a master before its dependents is the safe reading.

use std::path::PathBuf;

use kitkraft_doc::PluginSpec;

use crate::mo2::Profile;
use crate::overlay::{Overlay, Resolved};

/// Official masters the game loads without a `plugins.txt` entry: the eight Ortham's 2024
/// article lists, plus `SFBGS00D.esm`, `SFBGS047.esm` and `SFBGS050.esm` added by LOOT 0.29.0
/// (2026-04-11), plus `ShatteredSpace.esm` and `SFBGS004.esm`: the owner's `KitKraft Dev`
/// saves list both (and `SFBGS00D`, `SFBGS047`, `SFBGS050`) while that profile's
/// `plugins.txt` lists them without `*`. Active only when the file exists. Assumption
/// `hardcoded-official`.
pub const HARDCODED_OFFICIAL: [&str; 13] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "ShatteredSpace.esm",
    "BlueprintShips-Starfield.esm",
    "SFBGS003.esm",
    "SFBGS004.esm",
    "SFBGS006.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS00D.esm",
    "SFBGS047.esm",
    "SFBGS050.esm",
];

/// The game's order of the official plugins (assumption `hardcoded-official-slots`). Within
/// each scale it is the order the owner's saves record (game 1.16.244.0, 2026-10-15): full
/// `Starfield`, `ShatteredSpace`, `SFBGS00D`, `SFBGS050`; small `Constellation`, `OldMars`,
/// `SFBGS004`, `SFBGS007`, `SFBGS008`, `SFBGS047`; medium `SFBGS003`, `SFBGS006`; the
/// blueprint plugins `BlueprintShips-Starfield`, `BlueprintShips-SFBGS050` (whose runtime
/// slots come after every other full plugin's, user plugins included). A save lists each
/// scale apart, so how the scales interleave is **not observed**; it decides only which of
/// two official plugins of different scales wins a form both override. So the list keeps
/// the previous order wherever the saves do not contradict it: the `Starfield.ccc` order
/// (with `SFBGS004` moved before `SFBGS007` and `SFBGS003` before `SFBGS006`, as the saves
/// require), then the other official plugins in the order the owner's `Default`
/// `loadorder.txt` lists them.
pub const OFFICIAL_ORDER: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "SFBGS004.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS003.esm",
    "SFBGS006.esm",
    "ShatteredSpace.esm",
    "SFBGS00D.esm",
    "SFBGS050.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS047.esm",
];

/// Prefix of blueprint companion plugins (`BlueprintShips-<base>`).
pub const BLUEPRINT_PREFIX: &str = "BlueprintShips-";

/// Why a plugin is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Active {
    /// Listed in the profile's `Starfield.ccc`.
    Ccc,
    /// In [`HARDCODED_OFFICIAL`].
    Hardcoded,
    /// `*` line in `plugins.txt`.
    PluginsTxt,
    /// Provided by a `*Unmanaged: <stem>` modlist entry (assumption `mo2-unmanaged-active`).
    Unmanaged(String),
    /// `BlueprintShips-<base>` whose base plugin is active (LOOT 0.29.0/0.29.1).
    BlueprintCompanion(String),
}

impl Active {
    /// Text for results.
    pub fn label(&self) -> String {
        match self {
            Active::Ccc => "Starfield.ccc".into(),
            Active::Hardcoded => "hardcoded-official".into(),
            Active::PluginsTxt => "plugins.txt".into(),
            Active::Unmanaged(m) => format!("mo2-unmanaged:{m}"),
            Active::BlueprintCompanion(b) => format!("blueprint-companion-of:{b}"),
        }
    }
}

/// One plugin the profile knows about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedPlugin {
    /// File name.
    pub name: String,
    /// Why it is active, `None` when inactive.
    pub active: Option<Active>,
    /// The file, when found in the overlay or the game Data dir.
    pub file: Option<Resolved>,
    /// Whether `loadorder.txt` placed it (otherwise appended).
    pub from_loadorder: bool,
}

/// The ordered plugin list.
#[derive(Debug, Clone, Default)]
pub struct LoadPlan {
    /// Every known plugin, in load order.
    pub plugins: Vec<PlannedPlugin>,
    /// Problems found.
    pub warnings: Vec<String>,
    /// Plugins placed by the assumption `hardcoded-official-slots` that have no
    /// `Starfield.ccc` entry: active official plugins without a `plugins.txt` entry
    /// (hardcoded, MO2-unmanaged, and blueprint companions of those), in the official block.
    pub hardcoded_slots: Vec<String>,
    /// Of those, the ones `loadorder.txt` lists after a plugin that now loads after them.
    pub moved: Vec<String>,
}

/// Sort groups: official plugins, then the user's.
const GROUP_OFFICIAL: u8 = 0;
const GROUP_USER: u8 = 1;

/// Whether an active plugin is official: in `Starfield.ccc`, hardcoded, MO2-unmanaged, or a
/// blueprint companion of one of those.
fn official(active: Option<&Active>, slotted: &[String]) -> bool {
    match active {
        Some(Active::Ccc | Active::Hardcoded | Active::Unmanaged(_)) => true,
        Some(Active::BlueprintCompanion(base)) => {
            slotted.iter().any(|s| s.eq_ignore_ascii_case(base))
        }
        _ => false,
    }
}

impl LoadPlan {
    /// Build the plan.
    pub fn new(profile: &Profile, overlay: &Overlay) -> LoadPlan {
        let mut plan = LoadPlan::default();
        let exists = |name: &str| overlay.resolve(name).is_some();

        // Activity, by rule (first rule that applies is reported).
        let mut active: Vec<(String, Active)> = Vec::new();
        let mark = |name: &str, why: Active, active: &mut Vec<(String, Active)>| {
            if !active.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
                active.push((name.to_string(), why));
            }
        };
        for name in profile.ccc.iter().flatten() {
            mark(name, Active::Ccc, &mut active);
        }
        for name in HARDCODED_OFFICIAL {
            if exists(name) {
                mark(name, Active::Hardcoded, &mut active);
            }
        }
        for line in profile.plugins_txt.iter().filter(|l| l.enabled) {
            mark(&line.name, Active::PluginsTxt, &mut active);
        }
        for m in profile.unmanaged() {
            let Some(stem) = m.unmanaged_stem() else {
                continue;
            };
            let found = ["esm", "esp", "esl"]
                .iter()
                .map(|ext| format!("{stem}.{ext}"))
                .find(|n| exists(n));
            match found {
                Some(name) => mark(&name, Active::Unmanaged(m.name.clone()), &mut active),
                None => plan.warnings.push(format!(
                    "modlist.txt:{}: unmanaged entry `{}` has no plugin {stem}.esm/.esp/.esl",
                    m.line, m.name
                )),
            }
        }
        // Blueprint companions of active plugins (one pass; companions have no companions).
        let bases: Vec<String> = active.iter().map(|(n, _)| n.clone()).collect();
        for base in bases {
            if base.len() >= BLUEPRINT_PREFIX.len()
                && base
                    .get(..BLUEPRINT_PREFIX.len())
                    .is_some_and(|p| p.eq_ignore_ascii_case(BLUEPRINT_PREFIX))
            {
                continue;
            }
            let companion = format!("{BLUEPRINT_PREFIX}{base}");
            if exists(&companion) {
                mark(&companion, Active::BlueprintCompanion(base), &mut active);
            }
        }

        // Order: loadorder.txt, then anything active it does not list.
        let mut names: Vec<(String, bool)> = Vec::new();
        let add = |name: &str, from: bool, names: &mut Vec<(String, bool)>| {
            if !names.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
                names.push((name.to_string(), from));
            }
        };
        for name in profile.loadorder.iter().flatten() {
            add(name, true, &mut names);
        }
        for line in &profile.plugins_txt {
            if !line.enabled {
                add(&line.name, false, &mut names);
            }
        }
        for (name, _) in &active {
            if !names.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
                if profile.loadorder.is_some() {
                    plan.warnings.push(format!(
                        "{name} is active but not in loadorder.txt; placed last in its group"
                    ));
                }
                add(name, false, &mut names);
            }
        }

        // Assumption `hardcoded-official-slots` (docs/load-order.md): the official plugins
        // in OFFICIAL_ORDER (unknown official ones after them: ccc order, then
        // loadorder.txt's), then the rest in loadorder.txt's order, then the blueprint full
        // plugins (OFFICIAL_ORDER among themselves, then loadorder.txt's).
        let why_of = |name: &str| {
            active
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, w)| w)
        };
        let direct: Vec<String> = active
            .iter()
            .filter(|(_, w)| matches!(w, Active::Ccc | Active::Hardcoded | Active::Unmanaged(_)))
            .map(|(n, _)| n.clone())
            .collect();
        let ccc_pos = |name: &str| {
            profile
                .ccc
                .iter()
                .flatten()
                .position(|c| c.eq_ignore_ascii_case(name))
                .unwrap_or(usize::MAX)
        };
        let official_pos = |name: &str| {
            OFFICIAL_ORDER
                .iter()
                .position(|c| c.eq_ignore_ascii_case(name))
                .unwrap_or(usize::MAX)
        };
        // (group, official position, ccc position, loadorder.txt position, name, listed)
        type Key = (u8, usize, usize, usize, String, bool);
        let mut keyed: Vec<Key> = names
            .into_iter()
            .enumerate()
            .map(|(i, (name, from))| {
                let group = if official(why_of(&name), &direct) {
                    GROUP_OFFICIAL
                } else {
                    GROUP_USER
                };
                let (o, c) = if group == GROUP_USER {
                    (usize::MAX, usize::MAX)
                } else {
                    (official_pos(&name), ccc_pos(&name))
                };
                (group, o, c, i, name, from)
            })
            .collect();
        let first_user = keyed
            .iter()
            .filter(|k| k.0 == GROUP_USER && k.5 && why_of(&k.4).is_some())
            .map(|k| k.3)
            .min();
        keyed.sort_by_key(|k| (k.0, k.1, k.2, k.3));
        for k in &keyed {
            if k.0 == GROUP_OFFICIAL && why_of(&k.4) != Some(&Active::Ccc) {
                plan.hardcoded_slots.push(k.4.clone());
                if k.5 && first_user.is_some_and(|f| k.3 > f) {
                    plan.moved.push(k.4.clone());
                }
            }
        }
        let names: Vec<(String, bool)> = keyed.into_iter().map(|k| (k.4, k.5)).collect();
        for (name, from_loadorder) in names {
            let why = active
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&name))
                .map(|(_, w)| w.clone());
            let file = overlay.resolve(&name);
            if why.is_some() && file.is_none() {
                plan.warnings
                    .push(format!("{name} is active but no file provides it"));
            }
            plan.plugins.push(PlannedPlugin {
                name,
                active: why,
                file,
                from_loadorder,
            });
        }
        plan
    }

    /// Active plugins with a file, in load order, ready for `LoadOrder::load`.
    pub fn specs(&self) -> Vec<PluginSpec> {
        self.plugins
            .iter()
            .filter(|p| p.active.is_some())
            .filter_map(|p| {
                Some(PluginSpec {
                    name: p.name.clone(),
                    path: p.file.as_ref()?.path.clone(),
                })
            })
            .collect()
    }

    /// Active plugins without a file.
    pub fn missing(&self) -> Vec<&str> {
        self.plugins
            .iter()
            .filter(|p| p.active.is_some() && p.file.is_none())
            .map(|p| p.name.as_str())
            .collect()
    }

    /// The planned entry for `name`.
    pub fn get(&self, name: &str) -> Option<&PlannedPlugin> {
        self.plugins
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }
}

/// Plugins given explicitly: names resolved against `data_dir` unless they are paths.
pub fn explicit(
    plugins: &[String],
    data_dir: Option<&std::path::Path>,
) -> (Vec<PluginSpec>, Vec<String>) {
    let mut specs = Vec::new();
    let mut missing = Vec::new();
    for p in plugins {
        let as_path = PathBuf::from(p);
        let path = if as_path.is_absolute() || as_path.components().count() > 1 {
            as_path
        } else {
            match data_dir {
                Some(d) => d.join(p),
                None => as_path,
            }
        };
        if path.is_file() {
            let name = kitkraft_doc::index::file_name_of(&path);
            specs.push(PluginSpec { name, path });
        } else {
            missing.push(path.display().to_string());
        }
    }
    (specs, missing)
}
