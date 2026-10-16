//! Per-user workbench preferences: `%LOCALAPPDATA%\kitkraft\ui.json`.
//!
//! This file is the only thing the workbench writes on its own (the screenshot tour
//! writes images only when asked, and the ledger row is written by the engine's
//! `ledger` command, ADR 0007 Amendment 2). It is UI state, not game data: the MO2
//! instance and profile last loaded from the start screen, the checkout whose docs the
//! Help and Ledger tabs read, the visual theme (`theme`), the Records split
//! (`records_split`) and the window's place (`window`: size, position, maximized, and
//! the monitor it was on; M6.P7). It is written only when a start-screen button, the
//! Start tab's theme picker, the Help tab's "use this checkout", the end of a drag of the
//! Records inspector's edge, or a move or resize of the window (once it has settled, and
//! at close) asks for it, and never names or touches a plugin, a profile's files or the
//! game.

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
    /// Where the window was when it last moved, resized or closed (M6.P7).
    pub window: Option<WindowPrefs>,
}

/// The window's place, in points (egui's logical pixels, so a DPI change does not
/// change them).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WindowPrefs {
    /// Inner size.
    pub width: f32,
    pub height: f32,
    /// Outer position (top-left of the frame), when known.
    pub x: Option<f32>,
    pub y: Option<f32>,
    /// Maximized.
    pub maximized: bool,
    /// Size of the monitor it was on, when known.
    pub monitor: Option<[f32; 2]>,
}

/// The smallest window the workbench restores, in points.
pub const MIN_WINDOW: [f32; 2] = [640.0, 400.0];

impl WindowPrefs {
    /// The place to restore on `monitor` (the size of the monitor the window opened on):
    /// the size no bigger than the monitor and no smaller than [`MIN_WINDOW`]; the
    /// position kept only when the monitor is the one it was saved on (same size), else
    /// dropped so the window is centred there. Without a monitor size, unchanged.
    pub fn clamp(&self, monitor: Option<[f32; 2]>) -> WindowPrefs {
        let Some([mw, mh]) = monitor.filter(|[w, h]| *w > 0.0 && *h > 0.0) else {
            return *self;
        };
        let fit = |v: f32, min: f32, max: f32| {
            if v.is_finite() {
                v.min(max).max(min.min(max))
            } else {
                min.min(max)
            }
        };
        let [min_w, min_h] = MIN_WINDOW;
        let same_monitor = self.monitor == Some([mw, mh]);
        let pos_ok = |p: Option<f32>| p.filter(|v| v.is_finite() && same_monitor);
        WindowPrefs {
            width: fit(self.width, min_w, mw),
            height: fit(self.height, min_h, mh),
            x: pos_ok(self.x),
            y: pos_ok(self.y),
            maximized: self.maximized,
            monitor: Some([mw, mh]),
        }
    }

    fn from_value(v: &Value) -> Option<Self> {
        let n = |k: &str| v.get(k).and_then(Value::as_f64).map(|f| f as f32);
        let monitor = v
            .get("monitor")
            .and_then(Value::as_array)
            .and_then(|a| Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32]));
        Some(Self {
            width: n("width").filter(|w| w.is_finite() && *w > 0.0)?,
            height: n("height").filter(|h| h.is_finite() && *h > 0.0)?,
            x: n("x").filter(|f| f.is_finite()),
            y: n("y").filter(|f| f.is_finite()),
            maximized: v.get("maximized").and_then(Value::as_bool).unwrap_or(false),
            monitor,
        })
    }

    fn to_value(self) -> Value {
        let num = |f: f32| {
            serde_json::Number::from_f64(f64::from(f))
                .map(Value::Number)
                .unwrap_or(Value::Null)
        };
        let mut map = Map::new();
        map.insert("width".into(), num(self.width));
        map.insert("height".into(), num(self.height));
        if let Some(x) = self.x {
            map.insert("x".into(), num(x));
        }
        if let Some(y) = self.y {
            map.insert("y".into(), num(y));
        }
        map.insert("maximized".into(), Value::Bool(self.maximized));
        if let Some([w, h]) = self.monitor {
            map.insert("monitor".into(), Value::Array(vec![num(w), num(h)]));
        }
        Value::Object(map)
    }
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
            window: map.get("window").and_then(WindowPrefs::from_value),
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
        if let Some(w) = self.window {
            map.insert("window".into(), w.to_value());
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
            window: Some(WindowPrefs {
                width: 1200.0,
                height: 800.0,
                x: Some(-1900.0),
                y: Some(40.0),
                maximized: false,
                monitor: Some([1920.0, 1080.0]),
            }),
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

    /// M6.P7: a saved place is clamped to the monitor the window opens on.
    #[test]
    fn the_window_is_clamped_to_the_monitor_present() {
        let saved = WindowPrefs {
            width: 2400.0,
            height: 300.0,
            x: Some(2000.0),
            y: Some(50.0),
            maximized: true,
            monitor: Some([2560.0, 1440.0]),
        };
        // Same monitor: position kept, size fitted.
        let same = saved.clamp(Some([2560.0, 1440.0]));
        assert_eq!((same.width, same.height), (2400.0, MIN_WINDOW[1]));
        assert_eq!((same.x, same.y), (Some(2000.0), Some(50.0)));
        assert!(same.maximized);
        // A smaller monitor: size cut to it, position dropped (centred by the window).
        let small = saved.clamp(Some([1280.0, 720.0]));
        assert_eq!((small.width, small.height), (1280.0, MIN_WINDOW[1]));
        assert_eq!((small.x, small.y), (None, None));
        assert_eq!(small.monitor, Some([1280.0, 720.0]));
        // A monitor smaller than the minimum: the monitor wins.
        let tiny = saved.clamp(Some([500.0, 300.0]));
        assert_eq!((tiny.width, tiny.height), (500.0, 300.0));
        // Unknown monitor or a bad size: unchanged, never NaN.
        assert_eq!(saved.clamp(None), saved);
        let nan = WindowPrefs {
            width: f32::NAN,
            ..saved
        }
        .clamp(Some([1920.0, 1080.0]));
        assert_eq!(nan.width, MIN_WINDOW[0]);
        // A damaged window entry is ignored; the rest of the file still reads.
        let p = Prefs::from_json(r#"{"theme": "studio", "window": {"width": "wide"}}"#);
        assert_eq!(p.window, None);
        assert_eq!(p.theme.as_deref(), Some("studio"));
    }
}
