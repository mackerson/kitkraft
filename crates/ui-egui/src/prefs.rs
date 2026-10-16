//! Per-user workbench preferences: `%LOCALAPPDATA%\kitkraft\ui.json`.
//!
//! This file is the only thing the workbench writes on its own (the screenshot tour
//! writes images only when asked, and the ledger row is written by the engine's
//! `ledger` command, ADR 0007 Amendment 2). It is UI state, not game data: the MO2
//! instance and profile last loaded from the start screen, the checkout whose docs the
//! Help and Ledger tabs read, the visual theme (`theme`), the Records split
//! (`records_split`) and the CK-style windows' layout (`ck_windows`). It is written only
//! when a start-screen button, the Start tab's theme picker, the Help tab's "use this
//! checkout", the end of a drag of the Records inspector's edge, or a CK-style window
//! opened, closed, moved or resized asks for it, and never names or touches a plugin, a
//! profile's files or the game.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The folder under `%LOCALAPPDATA%`.
pub const PREFS_DIR: &str = "kitkraft";
/// The file in it.
pub const PREFS_FILE: &str = "ui.json";

/// What the workbench remembers between runs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prefs {
    /// MO2 instance root last used.
    pub instance: Option<String>,
    /// Profile folder last loaded from the start screen.
    pub profile: Option<String>,
    /// KitKraft checkout whose docs the Help and Ledger tabs read.
    pub repo_dir: Option<String>,
    /// The visual preset (`workbench`, `studio`, `classic`, each with an optional
    /// `-light` or `-dark`); `--theme` overrides it at start. The Start tab's picker
    /// writes it.
    pub theme: Option<String>,
    /// The Records inspector's share of the tab's width (0.2 to 0.8), written when a drag
    /// of the pane's edge ends.
    pub records_split: Option<f32>,
    /// The CK-style windows (M7.M0): open or closed, position and size, per window and
    /// per floating tab (`crate::windows`, which reads and writes the object).
    pub ck_windows: Option<Value>,
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
        let records_split = map
            .get("records_split")
            .and_then(Value::as_f64)
            .filter(|v| (0.2..=0.8).contains(v))
            .map(|v| v as f32);
        Self {
            instance: get("instance"),
            profile: get("profile"),
            repo_dir: get("repo_dir"),
            theme: get("theme"),
            records_split,
            ck_windows: map.get("ck_windows").filter(|v| v.is_object()).cloned(),
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
        if let Some(n) = self
            .records_split
            .and_then(|v| serde_json::Number::from_f64(f64::from(v)))
        {
            map.insert("records_split".into(), Value::Number(n));
        }
        if let Some(w) = &self.ck_windows {
            map.insert("ck_windows".into(), w.clone());
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
            records_split: Some(0.5),
            ck_windows: Some(serde_json::json!({"Warnings": {"open": true}})),
        };
        prefs.save(&path).unwrap();
        assert_eq!(Prefs::load(&path), prefs);
        assert!(!path.with_extension("json.tmp").exists());
        assert!(!std::fs::read_to_string(&path).unwrap().contains("repo_dir"));
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Prefs::load(&path), Prefs::default());
        // A share outside 0.2 to 0.8 is ignored.
        assert_eq!(
            Prefs::from_json(r#"{"records_split": 0.95}"#),
            Prefs::default()
        );
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
