//! Which plugins a profile loads, in which order, from which files.
//!
//! Each rule is named so results can say why a plugin is active; the evidence and what is
//! still unverified is in `docs/load-order.md` ("Active plugins").

use std::path::PathBuf;

use kitcraft_doc::PluginSpec;

use crate::mo2::Profile;
use crate::overlay::{Overlay, Resolved};

/// Official masters the game loads without a `plugins.txt` entry: the eight Ortham's 2024
/// article lists, plus `SFBGS00D.esm`, `SFBGS047.esm` and `SFBGS050.esm` added by LOOT 0.29.0
/// (2026-04-11). Active only when the file exists. Assumption `hardcoded-official`.
pub const HARDCODED_OFFICIAL: [&str; 11] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "SFBGS003.esm",
    "SFBGS006.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS00D.esm",
    "SFBGS047.esm",
    "SFBGS050.esm",
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
                        "{name} is active but not in loadorder.txt; appended"
                    ));
                }
                add(name, false, &mut names);
            }
        }
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
            let name = kitcraft_doc::index::file_name_of(&path);
            specs.push(PluginSpec { name, path });
        } else {
            missing.push(path.display().to_string());
        }
    }
    (specs, missing)
}
