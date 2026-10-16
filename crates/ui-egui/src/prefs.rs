//! Per-user workbench preferences: `%LOCALAPPDATA%\kitkraft\ui.json`.
//!
//! This file is the only thing the workbench writes on its own (the screenshot tour
//! writes images only when asked, and the ledger row is written by the engine's
//! `ledger` command, ADR 0007 Amendment 2). It is UI state, not game data: the MO2
//! instance and profile last loaded from the start screen, the checkout whose docs the
//! Help and Ledger tabs read, and the visual theme (`theme`, read but not changed). It is written only when a start-screen button or the
//! Help tab's "use this checkout" is clicked, and never names or touches a plugin, a
//! profile's files or the game.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The folder under `%LOCALAPPDATA%`.
pub const PREFS_DIR: &str = "kitkraft";
/// The file in it.
pub const PREFS_FILE: &str = "ui.json";

/// What the workbench remembers between runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prefs {
    /// MO2 instance root last used.
    pub instance: Option<String>,
    /// Profile folder last loaded from the start screen.
    pub profile: Option<String>,
    /// KitKraft checkout whose docs the Help and Ledger tabs read.
    pub repo_dir: Option<String>,
    /// The visual preset (`classic`, `workbench`, `studio`, each with an optional
    /// `-light` or `-dark`); `--theme` overrides it. The window reads it and keeps it on
    /// save; nothing in the window changes it yet (the owner picks a direction, M6.O1).
    pub theme: Option<String>,
}

/// `<local_app_data>\kitkraft\ui.json`, or `None` without `%LOCALAPPDATA%`.
pub fn prefs_path(local_app_data: Option<&OsStr>) -> Option<PathBuf> {
    local_app_data
        .filter(|l| !l.is_empty())
        .map(|l| Path::new(l).join(PREFS_DIR).join(PREFS_FILE))
}

/// The prefs file of this user (`%LOCALAPPDATA%`).
pub fn default_path() -> Option<PathBuf> {
    prefs_path(std::env::var_os("LOCALAPPDATA").as_deref())
}

impl Prefs {
    /// Reads `text`; anything unreadable is left at its default (a damaged file never
    /// stops the window from opening).
    pub fn from_json(text: &str) -> Self {
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else {
            return Self::default();
        };
        let get = |k: &str| {
            map.get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        Self {
            instance: get("instance"),
            profile: get("profile"),
            repo_dir: get("repo_dir"),
            theme: get("theme"),
        }
    }

    /// The file's text: a JSON object with the members that are set.
    pub fn to_json(&self) -> String {
        let mut map = Map::new();
        for (k, v) in [
            ("instance", &self.instance),
            ("profile", &self.profile),
            ("repo_dir", &self.repo_dir),
            ("theme", &self.theme),
        ] {
            if let Some(v) = v {
                map.insert(k.into(), Value::String(v.clone()));
            }
        }
        serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_else(|_| "{}".to_string())
    }

    /// The prefs at `path`, or the defaults when there are none.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|t| Self::from_json(&t))
            .unwrap_or_default()
    }

    /// Writes the prefs to `path` (its folder is created): a temp file next to it, then
    /// renamed over it, so a crash leaves the old file or the new one.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let dir = path
            .parent()
            .ok_or_else(|| format!("{}: no parent folder", path.display()))?;
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, self.to_json()).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_is_under_local_app_data() {
        let p = prefs_path(Some(OsStr::new(r"C:\Users\x\AppData\Local"))).unwrap();
        assert!(
            p.ends_with(Path::new("kitkraft").join("ui.json")),
            "{}",
            p.display()
        );
        assert!(prefs_path(None).is_none());
        assert!(prefs_path(Some(OsStr::new(""))).is_none());
    }

    #[test]
    fn round_trip_and_damaged_files_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("kitkraft-ui-prefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(PREFS_DIR).join(PREFS_FILE);
        assert_eq!(Prefs::load(&path), Prefs::default());
        let prefs = Prefs {
            instance: Some(r"C:\Modding\MO2-Starfield".into()),
            profile: Some(r"C:\Modding\MO2-Starfield\profiles\KitKraft Dev".into()),
            repo_dir: None,
            theme: Some("studio-dark".into()),
        };
        prefs.save(&path).unwrap();
        assert_eq!(Prefs::load(&path), prefs);
        assert!(!path.with_extension("json.tmp").exists());
        assert!(!std::fs::read_to_string(&path).unwrap().contains("repo_dir"));
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Prefs::load(&path), Prefs::default());
        assert_eq!(
            Prefs::from_json(r#"{"instance": 5, "profile": "  ", "repo_dir": "D:\\kk", "x": 1}"#),
            Prefs {
                repo_dir: Some(r"D:\kk".into()),
                ..Prefs::default()
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
