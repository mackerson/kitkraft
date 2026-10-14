//! M2.D2 on the owner's real profile (read-only). Uses `KITCRAFT_MO2_PROFILE`, else
//! `C:\Modding\MO2-Starfield\profiles\Default`; skips when neither exists. Counts are computed
//! from the profile files here, never hard-coded.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::time::SystemTime;

use kitcraft_io::{plan_profile, Active, ProfileOptions, Provider};

fn profile_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("KITCRAFT_MO2_PROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Modding\MO2-Starfield\profiles\Default"));
    dir.join("modlist.txt").is_file().then_some(dir)
}

fn lines(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

fn stamp(path: &std::path::Path) -> Option<(u64, SystemTime)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.len(), m.modified().ok()?))
}

#[test]
fn default_profile_resolves_every_enabled_plugin() {
    let Some(dir) = profile_dir() else {
        eprintln!("skip: no MO2 profile");
        return;
    };
    let watched: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .chain(std::iter::once(dir.join("../../ModOrganizer.ini")))
        .collect();
    let before: Vec<_> = watched.iter().map(|p| stamp(p)).collect();

    let load = plan_profile(&dir, &ProfileOptions::default()).unwrap();
    let plan = &load.plan;

    // Expected counts, from the files.
    let star = lines(&dir.join("plugins.txt"))
        .iter()
        .filter(|l| l.starts_with('*'))
        .count();
    let ccc = lines(&dir.join("Starfield.ccc")).len();
    let loadorder = lines(&dir.join("loadorder.txt"));
    let modlist = lines(&dir.join("modlist.txt"));
    let unmanaged = modlist.iter().filter(|l| l.starts_with('*')).count();
    let enabled_mods = modlist
        .iter()
        .filter(|l| l.starts_with('+') && !l.ends_with("_separator"))
        .count();
    assert_eq!(load.profile.enabled_mods().len(), enabled_mods);

    let active: Vec<_> = plan.plugins.iter().filter(|p| p.active.is_some()).collect();
    let count = |f: &dyn Fn(&Active) -> bool| {
        active
            .iter()
            .filter(|p| p.active.as_ref().is_some_and(f))
            .count()
    };
    assert_eq!(count(&|a| *a == Active::PluginsTxt), star);
    assert_eq!(count(&|a| *a == Active::Ccc), ccc);
    eprintln!(
        "{} known plugins, {} active: ccc {}, hardcoded {}, plugins.txt {}, unmanaged {}, blueprint companions {}; loadorder.txt {} lines, {} enabled mods, {} unmanaged entries",
        plan.plugins.len(),
        active.len(),
        ccc,
        count(&|a| *a == Active::Hardcoded),
        star,
        count(&|a| matches!(a, Active::Unmanaged(_))),
        count(&|a| matches!(a, Active::BlueprintCompanion(_))),
        loadorder.len(),
        enabled_mods,
        unmanaged
    );
    for w in &plan.warnings {
        eprintln!("warning: {w}");
    }

    // Every active plugin has a file, and every file exists.
    assert_eq!(plan.missing(), Vec::<&str>::new());
    for p in &active {
        let file = p.file.as_ref().unwrap();
        assert!(file.path.is_file(), "{}", p.name);
        // Managed plugins come from their mods, official ones from the game Data dir.
        if p.active == Some(Active::PluginsTxt) {
            assert!(matches!(file.provider, Provider::Mod(_)), "{}", p.name);
        } else {
            assert_eq!(file.provider, Provider::GameData, "{}", p.name);
        }
    }
    // The active set in loadorder.txt order is exactly loadorder.txt (every entry is active
    // in this profile).
    let order: Vec<String> = active.iter().map(|p| p.name.to_ascii_lowercase()).collect();
    let expected: Vec<String> = loadorder.iter().map(|l| l.to_ascii_lowercase()).collect();
    assert_eq!(order, expected);
    assert!(plan.plugins.iter().all(|p| p.from_loadorder));

    // Nothing in the profile changed.
    let after: Vec<_> = watched.iter().map(|p| stamp(p)).collect();
    assert_eq!(before, after);
}
