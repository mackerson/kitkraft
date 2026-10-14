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
    // The active set is exactly loadorder.txt (every entry is active in this profile), in
    // the `hardcoded-official-slots` order: Starfield.ccc's plugins in its order, then the
    // official plugins without a plugins.txt entry in loadorder.txt's relative order, then
    // the rest in loadorder.txt's order.
    let lower =
        |v: &[String]| -> Vec<String> { v.iter().map(|l| l.to_ascii_lowercase()).collect() };
    let ccc_lines = lower(&lines(&dir.join("Starfield.ccc")));
    let unmanaged_stems: Vec<String> = modlist
        .iter()
        .filter_map(|l| l.strip_prefix("*Unmanaged: "))
        .map(str::to_ascii_lowercase)
        .collect();
    let slotted = |name: &str| {
        let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
        kitcraft_io::plan::HARDCODED_OFFICIAL
            .iter()
            .any(|h| h.eq_ignore_ascii_case(name))
            || unmanaged_stems.iter().any(|u| u == stem)
    };
    let all = lower(&loadorder);
    let mut expected: Vec<String> = ccc_lines.clone();
    expected.extend(
        all.iter()
            .filter(|n| !ccc_lines.contains(n) && slotted(n))
            .cloned(),
    );
    expected.extend(
        all.iter()
            .filter(|n| !ccc_lines.contains(n) && !slotted(n))
            .cloned(),
    );
    let order: Vec<String> = active.iter().map(|p| p.name.to_ascii_lowercase()).collect();
    assert_eq!(order, expected);
    assert!(plan.plugins.iter().all(|p| p.from_loadorder));
    eprintln!(
        "hardcoded-official-slots: {:?}; moved from their loadorder.txt position: {:?}",
        plan.hardcoded_slots, plan.moved
    );

    // Nothing in the profile changed.
    let after: Vec<_> = watched.iter().map(|p| stamp(p)).collect();
    assert_eq!(before, after);
}

/// Review round 2, item 1: what `hardcoded-official-slots` changes on the owner's profile.
/// Loads the plan's order and the raw `loadorder.txt` order (same files) and counts forms
/// whose winning plugin differs, and masters loading after their dependents in each.
/// Release: `cargo test -p kitcraft-io --release --test default_profile -- --ignored
/// --nocapture` (uses the index cache).
#[test]
#[ignore = "release: indexes the whole profile twice (cached)"]
fn hardcoded_slots_effect_on_the_default_profile() {
    use std::collections::HashMap;

    use kitcraft_doc::{IndexCache, LoadOptions, LoadOrder, PluginSpec};

    let Some(dir) = profile_dir() else {
        eprintln!("skip: no MO2 profile");
        return;
    };
    let load = plan_profile(&dir, &ProfileOptions::default()).unwrap();
    let new_specs = load.plan.specs();
    let raw_order = lines(&dir.join("loadorder.txt"));
    let old_specs: Vec<PluginSpec> = raw_order
        .iter()
        .filter_map(|n| {
            new_specs
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(n))
                .cloned()
        })
        .collect();
    assert_eq!(old_specs.len(), new_specs.len());
    let options = LoadOptions {
        cache: IndexCache::default_location(),
        threads: 0,
    };
    // (defining plugin, object id) -> winning plugin, for every overridden form; and the
    // master links that point later in the order.
    let winners = |specs: Vec<PluginSpec>| -> (HashMap<(String, u32), String>, Vec<String>) {
        let lo = LoadOrder::load(specs, &options).unwrap();
        let name = |i: u16| lo.plugin(i).map(|p| p.name.to_ascii_lowercase()).unwrap();
        let mut out = HashMap::new();
        for run in lo.forms.runs() {
            if run.len() < 2 {
                continue;
            }
            let form = run[0].form();
            out.insert(
                (name(form.plugin), form.object_id),
                name(run[run.len() - 1].plugin),
            );
        }
        let late: Vec<String> = lo
            .plugins
            .iter()
            .enumerate()
            .flat_map(|(i, p)| {
                p.late_masters(i as u16)
                    .into_iter()
                    .map(|m| format!("{} before {m}", p.name))
                    .collect::<Vec<_>>()
            })
            .collect();
        (out, late)
    };
    let (old, old_late) = winners(old_specs);
    let (new, new_late) = winners(new_specs);
    let slotted: Vec<String> = load
        .plan
        .hardcoded_slots
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let mut changed: Vec<(String, String, String)> = Vec::new();
    for (k, w_old) in &old {
        if let Some(w_new) = new.get(k) {
            if w_new != w_old {
                changed.push((format!("{}:{:06X}", k.0, k.1), w_old.clone(), w_new.clone()));
            }
        }
    }
    changed.sort();
    let slotted_lost = changed
        .iter()
        .filter(|(_, w_old, _)| slotted.contains(w_old))
        .count();
    eprintln!(
        "loadorder.txt order: {} late master link(s) {:?}",
        old_late.len(),
        old_late
    );
    eprintln!(
        "hardcoded-official-slots order: {} late master link(s) {:?}",
        new_late.len(),
        new_late
    );
    eprintln!(
        "{} overridden forms change winner ({} where a slotted official plugin won before):",
        changed.len(),
        slotted_lost
    );
    for (form, w_old, w_new) in &changed {
        eprintln!("  {form}: {w_old} -> {w_new}");
    }
    assert!(new_late.len() <= old_late.len());
}
