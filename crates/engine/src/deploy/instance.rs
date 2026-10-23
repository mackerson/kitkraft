//! The MO2 instance a deployment targets, resolved from its `ModOrganizer.ini`
//! (read-only), and the path rules every write is checked against.

use std::fs;
use std::path::{Path, PathBuf};

use kitkraft_io::{ModState, Profile, ProfileOptions};
use serde_json::{json, Value};

use super::{io_error, refused, DeployError, DEV_PROFILE, LIVE_PROFILE, PREFIX};
use crate::edit::patch::{guard_location, is_user_data_dir};

/// Longest full path (UTF-16 units) a deployed file may have: the classic Windows limit,
/// which MO2 and the game are not known to lift.
pub(crate) const MAX_PATH_UNITS: usize = 259;

/// An MO2 instance, as its `ModOrganizer.ini` describes it.
#[derive(Debug, Clone)]
pub(crate) struct Instance {
    /// The instance as its `ModOrganizer.ini` describes it (source of the profile list).
    pub io: kitkraft_io::Instance,
    /// The instance root (holds `ModOrganizer.ini`), absolute.
    pub root: PathBuf,
    /// `mods\` (`[Settings] mod_directory`, default `<base>\mods`), absolute.
    pub mods: PathBuf,
    /// `profiles\`.
    pub profiles: PathBuf,
    /// `[General] selected_profile`: MO2's active profile.
    pub selected_profile: Option<String>,
    /// `[General] gamePath`.
    pub game_dir: Option<PathBuf>,
    /// Directories a target must not be in or contain: `(what, canonical path)`.
    guarded: Vec<(&'static str, PathBuf)>,
}

/// Whether `path` itself is a symbolic link or junction (a name-surrogate reparse point;
/// Rust's `is_symlink` covers both on Windows).
pub(crate) fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// Refuses when `path` or any ancestor is a symbolic link or junction, so no write can be
/// redirected out of the folder it names. Components that do not exist yet are skipped.
pub(crate) fn refuse_links(path: &Path) -> Result<(), DeployError> {
    for p in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        if is_link(p) {
            return Err(refused(
                "reparse-point",
                p,
                "is a symbolic link or junction; deployment paths must be plain folders",
            ));
        }
    }
    Ok(())
}

/// `path` made absolute against the working directory (links not resolved).
pub(crate) fn absolute(path: &Path) -> Result<PathBuf, DeployError> {
    std::path::absolute(path).map_err(|e| io_error(path, "making the path absolute", &e))
}

fn canonical(path: &Path) -> Result<PathBuf, DeployError> {
    fs::canonicalize(path).map_err(|e| io_error(path, "resolving the path", &e))
}

/// Refuses a folder or file name Windows forbids, never rewriting it: empty, longer than
/// 255 characters, a character in `<>:"/\|?*` or a control character, a trailing dot or
/// space, or a reserved device name (`CON`, `NUL`, `COM1`, ...; with any extension).
pub(crate) fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("the name is empty".into());
    }
    if name.chars().count() > 255 {
        return Err(format!("`{name}` is longer than 255 characters"));
    }
    if let Some(c) = name.chars().find(|c| {
        matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control()
    }) {
        return Err(format!(
            "`{name}` contains {c:?}, which Windows forbids in a file name"
        ));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(format!(
            "`{name}` ends with a dot or a space, which Windows strips"
        ));
    }
    let base = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|p| {
            base.strip_prefix(p)
                .is_some_and(|n| n.len() == 1 && n.bytes().all(|b| (b'1'..=b'9').contains(&b)))
        });
    if reserved {
        return Err(format!("`{name}` is a reserved Windows device name"));
    }
    Ok(())
}

/// Length of `path` in UTF-16 units, as Windows counts it.
pub(crate) fn path_units(path: &Path) -> usize {
    path.to_string_lossy().encode_utf16().count()
}

impl Instance {
    /// Resolves and checks the instance at `mo2` (read-only): `ModOrganizer.ini` is a
    /// file, `mods\` and `profiles\` are folders, no component is a link or junction, and
    /// the instance is not inside the game's Data folder, the user Data folder or
    /// another MO2 instance (`set`'s and `build`'s `guard_location`).
    pub(crate) fn resolve(mo2: &Path) -> Result<Instance, DeployError> {
        let root = absolute(mo2)?;
        refuse_links(&root)?;
        // The settings, read once by `kitkraft-io`'s read-only instance reader.
        let mo2 = match kitkraft_io::Instance::read(&root) {
            Ok(i) => i,
            Err(kitkraft_io::IoError::NotAnInstance(_)) => {
                return Err(refused(
                    "mo2-instance",
                    &root,
                    "has no ModOrganizer.ini: not the root of an MO2 instance",
                ))
            }
            Err(e) => {
                return Err(refused(
                    "mo2-instance",
                    &root,
                    format!("ModOrganizer.ini could not be read: {e}"),
                ))
            }
        };
        let kitkraft_io::Instance {
            mods_dir: mods,
            profiles_dir: profiles,
            overwrite_dir: overwrite,
            downloads_dir: downloads,
            game_dir,
            selected_profile,
            ..
        } = &mo2;
        for (what, dir) in [("mods", mods), ("profiles", profiles)] {
            if !fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir()) {
                return Err(refused(
                    "mo2-instance",
                    dir,
                    format!("the instance's {what} folder does not exist"),
                ));
            }
        }
        refuse_links(mods)?;
        guard_location(&root).map_err(|e| refused("location", &root, e.to_string()))?;
        let mut guarded = vec![
            ("the MO2 profiles folder", canonical(profiles)?),
            ("the MO2 instance root", canonical(&root)?),
        ];
        for (what, dir) in [
            ("MO2's overwrite folder", overwrite),
            ("MO2's downloads folder", downloads),
        ] {
            if let Ok(c) = fs::canonicalize(dir) {
                guarded.push((what, c));
            }
        }
        if let Some(c) = game_dir
            .as_ref()
            .and_then(|g| fs::canonicalize(g.join("Data")).ok())
        {
            guarded.push(("the game's Data folder", c));
        }
        let (mods, profiles, selected_profile, game_dir) = (
            mods.clone(),
            profiles.clone(),
            selected_profile.clone(),
            game_dir.clone(),
        );
        let instance = Instance {
            io: mo2,
            root,
            mods,
            profiles,
            selected_profile,
            game_dir,
            guarded,
        };
        instance.check_mods_location()?;
        Ok(instance)
    }

    /// `mods\` must not be, or be inside, the game's Data folder or the user Data folder
    /// (`Documents\My Games\Starfield\Data`, `is_user_data_dir`, shared with `set` and
    /// `build`); it must not be the profiles, overwrite or downloads folder (MO2 lets
    /// `mod_directory` name any of them); and the guarded folders must not be inside it.
    fn check_mods_location(&self) -> Result<(), DeployError> {
        let mods = canonical(&self.mods)?;
        let spelled = absolute(&self.mods)?;
        for dir in mods.ancestors().chain(spelled.ancestors()) {
            if dir.join("Starfield.esm").is_file() {
                return Err(refused(
                    "location",
                    &self.mods,
                    format!("is inside the game's Data folder ({})", dir.display()),
                ));
            }
            if is_user_data_dir(dir) {
                return Err(refused(
                    "location",
                    &self.mods,
                    format!(
                        "is inside the game's user Data folder ({}), which the game also loads from",
                        dir.display()
                    ),
                ));
            }
        }
        for (what, g) in &self.guarded {
            if g == &mods && *what != "the MO2 instance root" {
                return Err(refused(
                    "location",
                    &self.mods,
                    format!(
                        "is {what} ({}); mod_directory must be a folder of its own",
                        g.display()
                    ),
                ));
            }
            if g != &mods && (mods.starts_with(g) && *what != "the MO2 instance root") {
                return Err(refused(
                    "location",
                    &self.mods,
                    format!("is inside {what} ({})", g.display()),
                ));
            }
            if g.starts_with(&mods) && g != &mods {
                return Err(refused(
                    "location",
                    &self.mods,
                    format!("contains {what} ({})", g.display()),
                ));
            }
        }
        Ok(())
    }

    /// The folder `name` under `mods\` (a direct child by construction), after checking
    /// that it is not, and does not contain, a guarded folder.
    pub(crate) fn child(&self, name: &str) -> Result<PathBuf, DeployError> {
        check_name(name).map_err(|m| refused("plugin-name", &self.mods.join(name), m))?;
        let path = self.mods.join(name);
        let canon = canonical(&self.mods)?.join(name);
        for (what, g) in &self.guarded {
            if g.starts_with(&canon) || (canon.starts_with(g) && *what != "the MO2 instance root") {
                return Err(refused(
                    "location",
                    &path,
                    format!("overlaps {what} ({})", g.display()),
                ));
            }
        }
        Ok(path)
    }

    /// The entries of `mods\` as `(name, path)`, sorted by name.
    pub(crate) fn mod_entries(&self) -> Result<Vec<(String, PathBuf)>, DeployError> {
        let rd = fs::read_dir(&self.mods).map_err(|e| io_error(&self.mods, "listing", &e))?;
        let mut out: Vec<(String, PathBuf)> = rd
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .collect();
        out.sort();
        Ok(out)
    }

    /// The entry of `mods\` named `name`, case-insensitively (Windows semantics on every
    /// platform).
    pub(crate) fn find(&self, name: &str) -> Result<Option<PathBuf>, DeployError> {
        let want = name.to_lowercase();
        Ok(self
            .mod_entries()?
            .into_iter()
            .find(|(n, _)| n.to_lowercase() == want)
            .map(|(_, p)| p))
    }

    /// The profile folder for `name`: the instance's profile entry matching it
    /// case-insensitively (MO2's `Kitkraft Dev` for `KitKraft Dev`), else the folder as
    /// named. On Windows the two spellings resolve to the same folder, so the lookup only
    /// changes the spelling on case-sensitive platforms.
    fn profile_dir(&self, name: &str) -> PathBuf {
        self.io
            .profile_named(name)
            .map(|p| self.io.profile_dir(&p))
            .unwrap_or_else(|| self.profiles.join(name))
    }

    /// The refusal for a `profile` argument, if any: `Default` (any case), a name Windows
    /// forbids, or not a profile folder of this instance (matched case-insensitively).
    pub(crate) fn check_profile_arg(&self, name: &str) -> Option<DeployError> {
        let dir = self.profiles.join(name);
        if name.trim().eq_ignore_ascii_case(LIVE_PROFILE) {
            Some(refused(
                "live-profile",
                &dir,
                format!(
                    "`{LIVE_PROFILE}` is the owner's live game and never a deployment target; use `{DEV_PROFILE}`"
                ),
            ))
        } else if let Err(m) = check_name(name) {
            Some(refused("profile-name", &dir, m))
        } else if !self.profile_dir(name).join("modlist.txt").is_file() {
            Some(refused(
                "profile-missing",
                &dir,
                "is not an MO2 profile of this instance (no modlist.txt)",
            ))
        } else {
            None
        }
    }

    /// The `profile` argument when [`Instance::check_profiles`] accepts it; diagnostics
    /// read nothing else the caller named.
    pub(crate) fn accepted_profile<'a>(&self, profile: Option<&'a str>) -> Option<&'a str> {
        profile.filter(|p| self.check_profile_arg(p).is_none())
    }

    /// MO2's active profile, trimmed, when `ModOrganizer.ini` names one.
    fn active_profile(&self) -> Option<&str> {
        self.selected_profile
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
    }

    /// Refusals for the profiles involved: MO2's active profile is `Default`; or it is
    /// unknown (`selected_profile` missing or empty) and `profile` does not name an
    /// existing profile other than `Default`; or the `profile` argument is refused
    /// ([`Instance::check_profile_arg`]). The gate fails closed: an INI it cannot read a
    /// profile from never counts as "not `Default`".
    pub(crate) fn check_profiles(&self, profile: Option<&str>) -> Vec<DeployError> {
        let mut out = Vec::new();
        let ini = self.root.join("ModOrganizer.ini");
        match self.active_profile() {
            Some(p) if p.eq_ignore_ascii_case(LIVE_PROFILE) => out.push(refused(
                "live-profile",
                &ini,
                format!(
                    "MO2's active profile is `{LIVE_PROFILE}`, the owner's live game; switch MO2 to the `{DEV_PROFILE}` profile first"
                ),
            )),
            Some(_) => {}
            None if self.accepted_profile(profile).is_some() => {}
            None => out.push(refused(
                "live-profile-unknown",
                &ini,
                format!(
                    "names no active profile ([General] selected_profile is missing or empty), so it may be `{LIVE_PROFILE}`; pass --profile \"{DEV_PROFILE}\" (an existing profile other than `{LIVE_PROFILE}`), or open MO2 on `{DEV_PROFILE}` and close it so it records the profile"
                ),
            )),
        }
        if let Some(e) = profile.and_then(|name| self.check_profile_arg(name)) {
            out.push(e);
        }
        out
    }

    /// The profile diagnostics are read for: the `profile` argument when it is accepted,
    /// else MO2's active profile, else `KitKraft Dev` when it exists. Never `Default`: its
    /// state is irrelevant, and a warning such as "not enabled in Default" must not read
    /// as advice.
    pub(crate) fn diagnostic_profile<'a>(&'a self, profile: Option<&'a str>) -> Option<&'a str> {
        let live = |p: &&str| p.trim().eq_ignore_ascii_case(LIVE_PROFILE);
        self.accepted_profile(profile)
            .or(self.active_profile().filter(|p| !live(p)))
            .or_else(|| {
                self.profile_dir(DEV_PROFILE)
                    .join("modlist.txt")
                    .is_file()
                    .then_some(DEV_PROFILE)
            })
    }

    /// Read-only: whether `mod_name` is enabled in `profile` and where `plugin` sits in
    /// its `plugins.txt` (through `kitkraft-io`'s profile reader), with warnings.
    pub(crate) fn profile_state(
        &self,
        profile: &str,
        mod_name: &str,
        plugin: &str,
    ) -> (Value, Vec<String>) {
        let dir = self.profile_dir(profile);
        let options = ProfileOptions {
            data_dir: Some(
                self.game_dir
                    .as_ref()
                    .map_or_else(|| self.root.clone(), |g| g.join("Data")),
            ),
        };
        let p = match Profile::read_with(&dir, &options) {
            Ok(p) => p,
            Err(e) => {
                return (
                    json!({"profile": profile, "read": false, "error": e.to_string()}),
                    vec![format!("profile `{profile}` could not be read: {e}")],
                )
            }
        };
        let mut warnings = Vec::new();
        let entry = p
            .mods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(mod_name));
        let enabled = entry.is_some_and(|m| m.state == ModState::Enabled);
        if !enabled {
            warnings.push(format!(
                "mod `{mod_name}` is {} in profile `{profile}`",
                if entry.is_some() {
                    "not enabled"
                } else {
                    "not listed yet"
                }
            ));
        }
        let position = p
            .plugins_txt
            .iter()
            .position(|l| l.name.eq_ignore_ascii_case(plugin));
        let line = position.and_then(|i| p.plugins_txt.get(i));
        let plugin_enabled = line.is_some_and(|l| l.enabled);
        let last = position.is_some_and(|i| i.saturating_add(1) == p.plugins_txt.len());
        if !plugin_enabled {
            warnings.push(format!(
                "plugin `{plugin}` is not active in profile `{profile}`'s plugins.txt"
            ));
        } else if !last {
            warnings.push(format!(
                "plugin `{plugin}` is not last in profile `{profile}`'s plugins.txt"
            ));
        }
        (
            json!({
                "profile": profile,
                "read": true,
                "mod_listed": entry.is_some(),
                "mod_enabled": enabled,
                "plugin_listed": line.is_some(),
                "plugin_enabled": plugin_enabled,
                "plugin_position": position.map(|i| i.saturating_add(1)),
                "plugins_listed": p.plugins_txt.len(),
                "plugin_last": last,
            }),
            warnings,
        )
    }
}

/// The dev mod's folder name for a plugin stem.
pub(crate) fn mod_name(stem: &str) -> String {
    format!("{PREFIX}{stem}")
}

/// A `mod` argument (`KitKraft Dev - X` or `X`) as the folder name.
pub(crate) fn mod_arg(arg: &str) -> String {
    let arg = arg.trim();
    if arg.len() >= PREFIX.len()
        && arg
            .get(..PREFIX.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(PREFIX))
    {
        format!("{PREFIX}{}", arg.get(PREFIX.len()..).unwrap_or_default())
    } else {
        mod_name(arg)
    }
}
