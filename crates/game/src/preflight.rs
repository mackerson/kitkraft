//! Preflight (ADR 0010 section 1 and Amendment 1 conditions 1 and 2): every condition is a
//! refusal, never a warning, and each names its rule.
//!
//! [`check`] is pure: the caller (the engine's `game.session`, which reads MO2 through
//! `kitkraft-io`) gathers [`Facts`] and this decides. The rules:
//!
//! | Rule | Refused when |
//! | ---- | ------------ |
//! | `live-profile` | the profile named is `Default` (the owner's live game) |
//! | `profile-missing` | the profile folder does not exist in the named MO2 instance |
//! | `selected-profile` | MO2's `selected_profile` is absent, or is not the profile named |
//! | `running` | the game, its script extender, the editor or the mod manager runs |
//! | `profile-settings` | the profile's `settings.ini` lacks `LocalSettings=true` or `LocalSaves=true` |
//! | `ini-switch` | an INI switch of the game profile is missing or different |
//! | `console-plugin` | no enabled mod of the profile is the console plugin |
//! | `base-save` | the base save is missing, its SHA-256 was not given, or differs |
//! | `plugin-not-deployed` | the plugin under test is not a `deploy --dev` mod enabled in the profile |
//! | `unverified-profile` | the game profile has facts without evidence ([`crate::GameProfile::missing`]) |

use std::path::PathBuf;

use serde::Serialize;

use crate::host::is_running;
use crate::profile::GameProfile;

/// The owner's live-game profile: never a target (AGENTS.md).
pub const LIVE_PROFILE: &str = "Default";

/// What preflight looks at, gathered by the caller.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// The MO2 instance root (one instance per call).
    pub instance: PathBuf,
    /// The profile the call names.
    pub profile: String,
    /// Whether that profile's folder exists.
    pub profile_exists: bool,
    /// `ModOrganizer.ini` `[General] selected_profile`, decoded.
    pub selected_profile: Option<String>,
    /// Image names of running processes.
    pub running: Vec<String>,
    /// The profile's `settings.ini` `[General] LocalSettings` and `LocalSaves`.
    pub local_settings: Option<bool>,
    /// See `local_settings`.
    pub local_saves: Option<bool>,
    /// `(file, section, key) -> value` read from the profile's INIs; a missing entry is
    /// a missing key (or file).
    pub ini_values: Vec<(String, String, String, String)>,
    /// Names of the profile's enabled mods (`+` lines of `modlist.txt`).
    pub enabled_mods: Vec<String>,
    /// The plugin under test.
    pub plugin: Option<PluginFacts>,
    /// The base save.
    pub base_save: BaseSave,
}

/// The plugin under test and its dev deployment.
#[derive(Debug, Clone, Default)]
pub struct PluginFacts {
    /// File name (`KitKraftFirstPOI.esm`).
    pub name: String,
    /// The dev mod's folder name (`KitKraft Dev - KitKraftFirstPOI`).
    pub dev_mod: String,
    /// Whether the folder holds `deploy --dev`'s marker (`kitkraft-deploy.json`).
    pub marker: bool,
}

/// The base save.
#[derive(Debug, Clone, Default)]
pub struct BaseSave {
    /// Its name, without extension.
    pub name: String,
    /// Its path in the profile's saves folder.
    pub path: PathBuf,
    /// Its SHA-256 on disk, when it exists.
    pub sha256: Option<String>,
    /// The SHA-256 recorded at M4.O7, given by the call.
    pub expected_sha256: Option<String>,
}

/// One refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    /// The rule id.
    pub rule: &'static str,
    /// What is wrong, naming the path or process.
    pub message: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refused [{}]: {}", self.rule, self.message)
    }
}

/// Every refusal for `facts` under `game`; empty means the session may start.
pub fn check(game: &GameProfile, facts: &Facts) -> Vec<Refusal> {
    let mut out = Vec::new();
    let mut refuse = |rule: &'static str, message: String| out.push(Refusal { rule, message });
    let profile = facts.profile.trim();
    if profile.eq_ignore_ascii_case(LIVE_PROFILE) {
        refuse(
            "live-profile",
            format!(
                "`{LIVE_PROFILE}` is the owner's live game; sessions run only on the dev profile"
            ),
        );
    }
    if !facts.profile_exists {
        refuse(
            "profile-missing",
            format!(
                "profile `{profile}` does not exist in the MO2 instance {}",
                facts.instance.display()
            ),
        );
    }
    match facts.selected_profile.as_deref().map(str::trim) {
        None | Some("") => refuse(
            "selected-profile",
            "MO2's selected_profile is absent from ModOrganizer.ini; select the dev profile in MO2".into(),
        ),
        Some(sel) if sel.eq_ignore_ascii_case(LIVE_PROFILE) => refuse(
            "selected-profile",
            format!("MO2's selected profile is `{sel}` (the owner's live game); select `{profile}` in MO2 first"),
        ),
        Some(sel) if !sel.eq_ignore_ascii_case(profile) => refuse(
            "selected-profile",
            format!("MO2's selected profile is `{sel}`, not `{profile}`"),
        ),
        Some(_) => {}
    }
    for exe in game.executables.blocking() {
        if is_running(&facts.running, &exe) {
            refuse(
                "running",
                format!("{exe} is running (the owner may be playing or editing; one instance at a time)"),
            );
        }
    }
    if game.mo2.local_settings && facts.local_settings != Some(true) {
        refuse(
            "profile-settings",
            format!("the profile's settings.ini does not say LocalSettings=true: the game would read the INIs under My Games, not `{profile}`'s, so its switches are not in effect (owner, M4.O7)"),
        );
    }
    if game.mo2.local_saves && facts.local_saves != Some(true) {
        refuse(
            "profile-settings",
            "the profile's settings.ini does not say LocalSaves=true: saves would not stay in the dev profile".into(),
        );
    }
    for sw in &game.ini {
        let seen = facts.ini_values.iter().find(|(f, s, k, _)| {
            f.eq_ignore_ascii_case(&sw.file)
                && s.eq_ignore_ascii_case(&sw.section)
                && k.eq_ignore_ascii_case(&sw.key)
        });
        match seen {
            Some((_, _, _, v)) if v.trim().eq_ignore_ascii_case(sw.value.trim()) => {}
            Some((_, _, _, v)) => refuse(
                "ini-switch",
                format!(
                    "{} [{}] {}={} (needs {}; the owner sets it, M4.O7)",
                    sw.file,
                    sw.section,
                    sw.key,
                    v.trim(),
                    sw.value
                ),
            ),
            None => refuse(
                "ini-switch",
                format!(
                    "{} [{}] {} is not set (needs {}={}; the owner sets it, M4.O7)",
                    sw.file, sw.section, sw.key, sw.key, sw.value
                ),
            ),
        }
    }
    if !game.console.mod_names.is_empty() {
        let found = facts.enabled_mods.iter().any(|m| {
            let m = m.to_ascii_lowercase();
            game.console
                .mod_names
                .iter()
                .any(|n| m.contains(&n.to_ascii_lowercase()))
        });
        if !found {
            refuse(
                "console-plugin",
                format!(
                    "no enabled mod of `{profile}` is the console plugin ({}); the owner installs it (M4.O7)",
                    game.console.mod_names.join(", ")
                ),
            );
        }
    }
    let base = &facts.base_save;
    match (&base.sha256, &base.expected_sha256) {
        (None, _) => refuse(
            "base-save",
            format!("the base save `{}` is not at {}", base.name, base.path.display()),
        ),
        (Some(_), None) => refuse(
            "base-save",
            format!("the SHA-256 of the base save `{}` was not given (recorded at M4.O7); the controller loads only a save it can prove is the named one", base.name),
        ),
        (Some(have), Some(want)) if !have.eq_ignore_ascii_case(want.trim()) => refuse(
            "base-save",
            format!("the base save `{}` has SHA-256 {have}, not {want}", base.name),
        ),
        _ => {}
    }
    match &facts.plugin {
        None => refuse("plugin-not-deployed", "no plugin under test named".into()),
        Some(p) => {
            let enabled = facts
                .enabled_mods
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&p.dev_mod));
            if !p.marker {
                refuse(
                    "plugin-not-deployed",
                    format!("{} is not installed with `kit deploy --dev` (no `{}` mod with its marker); the controller installs nothing", p.name, p.dev_mod),
                );
            } else if !enabled {
                refuse(
                    "plugin-not-deployed",
                    format!(
                        "the dev mod `{}` is not enabled in `{profile}` (enable it in MO2)",
                        p.dev_mod
                    ),
                );
            }
        }
    }
    for u in game.missing() {
        refuse(
            "unverified-profile",
            format!(
                "{} profile: `{}` has no evidence yet ({})",
                game.game.id, u.fact, u.pending
            ),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn verified() -> GameProfile {
        let mut g = GameProfile::named("starfield").unwrap();
        g.launch.evidence = "test".into();
        g.console.evidence = "test".into();
        g.saves.evidence = "test".into();
        g
    }

    fn good(game: &GameProfile) -> Facts {
        Facts {
            instance: PathBuf::from(r"C:\MO2"),
            profile: "KitKraft Dev".into(),
            profile_exists: true,
            selected_profile: Some("Kitkraft Dev".into()),
            running: vec!["explorer.exe".into()],
            local_settings: Some(true),
            local_saves: Some(true),
            ini_values: game
                .ini
                .iter()
                .map(|s| {
                    (
                        s.file.clone(),
                        s.section.clone(),
                        s.key.clone(),
                        s.value.clone(),
                    )
                })
                .collect(),
            enabled_mods: vec!["Console API".into(), "KitKraft Dev - X".into()],
            plugin: Some(PluginFacts {
                name: "X.esm".into(),
                dev_mod: "KitKraft Dev - X".into(),
                marker: true,
            }),
            base_save: BaseSave {
                name: "KitKraftBase".into(),
                path: PathBuf::from("saves/KitKraftBase.sfs"),
                sha256: Some("ab".into()),
                expected_sha256: Some("AB".into()),
            },
        }
    }

    fn rules(game: &GameProfile, f: &Facts) -> Vec<&'static str> {
        check(game, f).into_iter().map(|r| r.rule).collect()
    }

    #[test]
    fn each_condition_is_its_own_refusal() {
        let g = verified();
        assert!(check(&g, &good(&g)).is_empty());
        type Case = (&'static str, Box<dyn Fn(&mut Facts)>);
        let cases: Vec<Case> = vec![
            (
                "live-profile",
                Box::new(|f| {
                    f.profile = "Default".into();
                    f.selected_profile = Some("Default".into());
                }),
            ),
            ("profile-missing", Box::new(|f| f.profile_exists = false)),
            ("selected-profile", Box::new(|f| f.selected_profile = None)),
            (
                "selected-profile",
                Box::new(|f| f.selected_profile = Some("Default".into())),
            ),
            (
                "selected-profile",
                Box::new(|f| f.selected_profile = Some("KitKraft Stack".into())),
            ),
            (
                "running",
                Box::new(|f| f.running.push("Starfield.exe".into())),
            ),
            (
                "running",
                Box::new(|f| f.running.push("creationkit.exe".into())),
            ),
            (
                "running",
                Box::new(|f| f.running.push("ModOrganizer.exe".into())),
            ),
            (
                "profile-settings",
                Box::new(|f| f.local_settings = Some(false)),
            ),
            ("profile-settings", Box::new(|f| f.local_saves = None)),
            (
                "ini-switch",
                Box::new(|f| {
                    f.ini_values.remove(0);
                }),
            ),
            ("ini-switch", Box::new(|f| f.ini_values[1].3 = "0".into())),
            (
                "console-plugin",
                Box::new(|f| f.enabled_mods.retain(|m| m != "Console API")),
            ),
            ("base-save", Box::new(|f| f.base_save.sha256 = None)),
            (
                "base-save",
                Box::new(|f| f.base_save.expected_sha256 = None),
            ),
            (
                "base-save",
                Box::new(|f| f.base_save.expected_sha256 = Some("cd".into())),
            ),
            ("plugin-not-deployed", Box::new(|f| f.plugin = None)),
            (
                "plugin-not-deployed",
                Box::new(|f| f.plugin.as_mut().unwrap().marker = false),
            ),
            (
                "plugin-not-deployed",
                Box::new(|f| f.enabled_mods.retain(|m| m != "KitKraft Dev - X")),
            ),
        ];
        for (rule, change) in cases {
            let mut f = good(&g);
            change(&mut f);
            let got = rules(&g, &f);
            assert!(got.contains(&rule), "{rule}: {got:?}");
            if rule != "live-profile" {
                assert!(got.iter().all(|r| *r == rule), "{rule}: {got:?}");
            }
        }
    }

    #[test]
    fn unverified_profiles_are_refused_naming_what_is_missing() {
        let sf = GameProfile::named("starfield").unwrap();
        let refusals = check(&sf, &good(&sf));
        assert_eq!(refusals.len(), 3);
        assert!(refusals.iter().all(|r| r.rule == "unverified-profile"));
        assert!(refusals[0].message.contains("`launch`"));
        assert!(refusals[0].message.contains("M4.T1 (a)"));
        let sky = GameProfile::named("skyrimse").unwrap();
        let msgs: Vec<String> = check(&sky, &good(&sky))
            .into_iter()
            .filter(|r| r.rule == "unverified-profile")
            .map(|r| r.message)
            .collect();
        assert!(msgs.iter().any(|m| m.contains("stub marked unverified")));
        assert!(msgs.iter().any(|m| m.contains("ConsoleUtil")));
    }
}
