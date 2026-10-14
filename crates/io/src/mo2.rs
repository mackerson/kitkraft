//! An MO2 profile, read-only: `modlist.txt`, `plugins.txt`, `loadorder.txt`,
//! `lockedorder.txt`, `Starfield.ccc`, `archives.txt`, and the instance's
//! `ModOrganizer.ini`. Nothing here ever writes; files are opened for reading only.
//!
//! Semantics and their evidence: `docs/load-order.md` ("MO2 profile").

use std::path::{Path, PathBuf};

use crate::error::{IoError, Result};
use crate::ini::Ini;
use crate::text::{entries, read_lines};

/// The `modlist.txt` prefix of a mod line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModState {
    /// `+`: enabled.
    Enabled,
    /// `-`: disabled.
    Disabled,
    /// `*`: an entry MO2 does not manage (DLC and Creations shipped in the game's Data dir,
    /// written as `*Unmanaged: <plugin stem>`).
    Unmanaged,
}

/// One `modlist.txt` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry {
    /// Name as written (the folder under `mods\` for a managed mod).
    pub name: String,
    /// Prefix.
    pub state: ModState,
    /// Names ending in `_separator` are list separators, not mods.
    pub separator: bool,
    /// 1-based line number.
    pub line: usize,
}

impl ModEntry {
    /// For an unmanaged entry `Unmanaged: X`, the plugin stem `X`.
    pub fn unmanaged_stem(&self) -> Option<&str> {
        (self.state == ModState::Unmanaged)
            .then(|| self.name.strip_prefix("Unmanaged: ").unwrap_or(&self.name))
    }
}

/// A `plugins.txt` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLine {
    /// File name.
    pub name: String,
    /// `*` prefix.
    pub enabled: bool,
    /// 1-based line number.
    pub line: usize,
}

/// Overrides for [`Profile::read_with`].
#[derive(Debug, Clone, Default)]
pub struct ProfileOptions {
    /// Use this game Data dir instead of `ModOrganizer.ini`'s `gamePath` + `Data`.
    pub data_dir: Option<PathBuf>,
}

/// A profile and its instance's directories.
#[derive(Debug, Clone)]
pub struct Profile {
    /// The profile directory.
    pub dir: PathBuf,
    /// Its name (the directory name).
    pub name: String,
    /// The MO2 instance directory (holds `ModOrganizer.ini`).
    pub instance_dir: PathBuf,
    /// The game install dir from `gamePath`, when known.
    pub game_dir: Option<PathBuf>,
    /// The game's Data dir.
    pub data_dir: PathBuf,
    /// Where managed mods live.
    pub mods_dir: PathBuf,
    /// The overwrite folder.
    pub overwrite_dir: PathBuf,
    /// `modlist.txt` entries in file order. The first line is the *highest* priority.
    pub mods: Vec<ModEntry>,
    /// `plugins.txt` entries in file order (empty when the file is absent).
    pub plugins_txt: Vec<PluginLine>,
    /// `loadorder.txt` entries, `None` when the file is absent.
    pub loadorder: Option<Vec<String>>,
    /// Non-comment lines of `lockedorder.txt` (raw; not applied, see `docs/load-order.md`).
    pub locked: Vec<String>,
    /// `Starfield.ccc` entries, `None` when absent.
    pub ccc: Option<Vec<String>>,
    /// `archives.txt` entries, `None` when absent.
    pub archives: Option<Vec<String>>,
    /// Things that look wrong but do not stop reading.
    pub warnings: Vec<String>,
}

impl Profile {
    /// Read the profile at `dir` with default options.
    pub fn read(dir: &Path) -> Result<Profile> {
        Profile::read_with(dir, &ProfileOptions::default())
    }

    /// Read the profile at `dir`.
    pub fn read_with(dir: &Path, options: &ProfileOptions) -> Result<Profile> {
        let modlist = read_lines(&dir.join("modlist.txt"))?
            .ok_or_else(|| IoError::NotAProfile(dir.to_path_buf()))?;
        let mut warnings = Vec::new();
        let instance_dir = dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| IoError::Instance(format!("{}: no instance dir", dir.display())))?;
        let ini_path = instance_dir.join("ModOrganizer.ini");
        let ini = match std::fs::read(&ini_path) {
            Ok(b) => Some(Ini::parse(&String::from_utf8_lossy(&b))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(IoError::io(&ini_path, e)),
        };
        let game_dir = ini
            .as_ref()
            .and_then(|i| i.get("General", "gamePath"))
            .filter(|p| !p.is_empty())
            .map(PathBuf::from);
        let data_dir = match (&options.data_dir, &game_dir) {
            (Some(d), _) => d.clone(),
            (None, Some(g)) => g.join("Data"),
            (None, None) => {
                return Err(IoError::Instance(format!(
                    "{}: no gamePath in [General] (or no file); pass the game Data dir",
                    ini_path.display()
                )))
            }
        };
        let base = ini
            .as_ref()
            .and_then(|i| i.get("Settings", "base_directory"))
            .map_or_else(|| instance_dir.clone(), PathBuf::from);
        let setting = |key: &str, default: &str| -> PathBuf {
            match ini.as_ref().and_then(|i| i.get("Settings", key)) {
                Some(v) => PathBuf::from(v.replace("%BASE_DIR%", &base.to_string_lossy())),
                None => base.join(default),
            }
        };
        let mods_dir = setting("mod_directory", "mods");
        let overwrite_dir = setting("overwrite_directory", "overwrite");

        let mut mods = Vec::new();
        for (line, text) in entries(&modlist) {
            let (state, name) = match text.split_at_checked(1) {
                Some(("+", rest)) => (ModState::Enabled, rest),
                Some(("-", rest)) => (ModState::Disabled, rest),
                Some(("*", rest)) => (ModState::Unmanaged, rest),
                _ => {
                    warnings.push(format!("modlist.txt:{line}: no +/-/* prefix: `{text}`"));
                    continue;
                }
            };
            mods.push(ModEntry {
                name: name.to_string(),
                state,
                separator: name.ends_with("_separator"),
                line,
            });
        }

        let plugins_txt = read_lines(&dir.join("plugins.txt"))?
            .map(|lines| {
                entries(&lines)
                    .into_iter()
                    .map(|(line, text)| match text.strip_prefix('*') {
                        Some(name) => PluginLine {
                            name: name.trim().to_string(),
                            enabled: true,
                            line,
                        },
                        None => PluginLine {
                            name: text.trim().to_string(),
                            enabled: false,
                            line,
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        let list = |name: &str| -> Result<Option<Vec<String>>> {
            Ok(read_lines(&dir.join(name))?.map(|lines| {
                entries(&lines)
                    .into_iter()
                    .map(|(_, t)| t.trim().to_string())
                    .collect()
            }))
        };
        let loadorder = list("loadorder.txt")?;
        let locked = list("lockedorder.txt")?.unwrap_or_default();
        if !locked.is_empty() {
            warnings.push(format!(
                "lockedorder.txt has {} entries; locked positions are not applied (format not observed yet)",
                locked.len()
            ));
        }
        let ccc = list("Starfield.ccc")?;
        let archives = list("archives.txt")?;
        let name = dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        Ok(Profile {
            dir: dir.to_path_buf(),
            name,
            instance_dir,
            game_dir,
            data_dir,
            mods_dir,
            overwrite_dir,
            mods,
            plugins_txt,
            loadorder,
            locked,
            ccc,
            archives,
            warnings,
        })
    }

    /// Enabled managed mods (no separators, no unmanaged entries), highest priority first.
    pub fn enabled_mods(&self) -> Vec<&ModEntry> {
        self.mods
            .iter()
            .filter(|m| m.state == ModState::Enabled && !m.separator)
            .collect()
    }

    /// Unmanaged entries.
    pub fn unmanaged(&self) -> Vec<&ModEntry> {
        self.mods
            .iter()
            .filter(|m| m.state == ModState::Unmanaged)
            .collect()
    }

    /// The profile files this reader opens, for callers that check they stay untouched.
    pub fn files(&self) -> Vec<PathBuf> {
        [
            "modlist.txt",
            "plugins.txt",
            "loadorder.txt",
            "lockedorder.txt",
            "Starfield.ccc",
            "archives.txt",
        ]
        .iter()
        .map(|f| self.dir.join(f))
        .chain(std::iter::once(self.instance_dir.join("ModOrganizer.ini")))
        .collect()
    }
}
