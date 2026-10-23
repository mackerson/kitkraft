//! An MO2 instance, read-only: its `ModOrganizer.ini` and the folders it names
//! (ADR 0009's instance check, for `kit deploy --dev` and the profile reader).
//!
//! What is read (`docs/load-order.md`, "ModOrganizer.ini"): `[General] gamePath`,
//! `gameName` and `selected_profile` (Qt `@ByteArray(...)` values with `\\` for `\`), and
//! `[Settings] base_directory`, `mod_directory`, `profiles_directory`,
//! `overwrite_directory` and `download_directory` (`%BASE_DIR%` expanded). An empty value
//! counts as absent; a relative folder is taken against the instance root. The owner's
//! portable instance has no `[Settings]` folder keys, so the defaults (`mods`,
//! `profiles`, `overwrite`, `downloads` under the base directory) are what is observed;
//! the keys are honoured but **not observed**.
//!
//! Nothing here writes, creates or follows anything: folders are only looked at with
//! `symlink_metadata`, and symbolic links and junctions are reported, not resolved.

use std::path::{Path, PathBuf};

use crate::error::{IoError, Result};
use crate::ini::Ini;

/// The instance's settings file, at its root.
pub const INI_NAME: &str = "ModOrganizer.ini";

/// One folder of an instance and what is on disk there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderState {
    /// A plain folder.
    Folder,
    /// A symbolic link or junction (a name-surrogate reparse point on Windows).
    Link,
    /// Something that is not a folder.
    NotAFolder,
    /// Nothing there (or it cannot be looked at).
    Missing,
}

impl FolderState {
    /// What is at `path`, without following a link.
    pub fn of(path: &Path) -> FolderState {
        match std::fs::symlink_metadata(path) {
            Ok(m) if m.file_type().is_symlink() => FolderState::Link,
            Ok(m) if m.is_dir() => FolderState::Folder,
            Ok(_) => FolderState::NotAFolder,
            Err(_) => FolderState::Missing,
        }
    }

    /// Lower-case name for reports.
    pub fn label(self) -> &'static str {
        match self {
            FolderState::Folder => "folder",
            FolderState::Link => "link",
            FolderState::NotAFolder => "not-a-folder",
            FolderState::Missing => "missing",
        }
    }
}

/// A path spelled as a Windows absolute location: a drive letter (`X:` or `X:\...`) or a
/// UNC share (`\\share\...`). Windows' own `Path::is_absolute` already reports these, so
/// this only matters when the same settings are read on another platform, where the
/// spelling still names an absolute location and must not be joined against the root.
fn windows_absolute(p: &Path) -> bool {
    let b = p.as_os_str().as_encoded_bytes();
    let drive = b.first().is_some_and(|c| c.is_ascii_alphabetic()) && b.get(1) == Some(&b':');
    let unc = b.first() == Some(&b'\\') && b.get(1) == Some(&b'\\');
    drive || unc
}

/// An MO2 instance as its `ModOrganizer.ini` describes it. Every path is as the settings
/// spell it, made absolute against the root; none is canonicalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// The instance root (holds `ModOrganizer.ini`).
    pub root: PathBuf,
    /// `[Settings] base_directory`, default the root.
    pub base_dir: PathBuf,
    /// `[Settings] mod_directory`, default `<base>\mods`.
    pub mods_dir: PathBuf,
    /// `[Settings] profiles_directory`, default `<base>\profiles`.
    pub profiles_dir: PathBuf,
    /// `[Settings] overwrite_directory`, default `<base>\overwrite`.
    pub overwrite_dir: PathBuf,
    /// `[Settings] download_directory`, default `<base>\downloads`.
    pub downloads_dir: PathBuf,
    /// `[General] gamePath`, when set.
    pub game_dir: Option<PathBuf>,
    /// `[General] gameName`, when set.
    pub game_name: Option<String>,
    /// `[General] selected_profile` (MO2's active profile), decoded and trimmed; `None`
    /// when absent or empty. Callers that gate on it must treat `None` as unknown.
    pub selected_profile: Option<String>,
}

impl Instance {
    /// Reads the instance at `root`: `ModOrganizer.ini` must be a regular file there (not
    /// a link), else [`IoError::NotAnInstance`]. The folders it names are not required to
    /// exist ([`Instance::folders`] reports them).
    pub fn read(root: &Path) -> Result<Instance> {
        let ini_path = root.join(INI_NAME);
        if !std::fs::symlink_metadata(&ini_path).is_ok_and(|m| m.is_file()) {
            return Err(IoError::NotAnInstance(root.to_path_buf()));
        }
        let bytes = std::fs::read(&ini_path).map_err(|e| IoError::io(&ini_path, e))?;
        Ok(Instance::from_ini(
            root,
            Some(&Ini::parse(&String::from_utf8_lossy(&bytes))),
        ))
    }

    /// The instance at `root` described by `ini` (`None`: no settings file, every folder
    /// at its default).
    pub fn from_ini(root: &Path, ini: Option<&Ini>) -> Instance {
        let get = |section: &str, key: &str| {
            ini.and_then(|i| i.get(section, key))
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        // A path absolute on this platform — or spelled Windows-absolute
        // elsewhere (a drive letter or a UNC share) — stays as spelled; a
        // relative one is taken against `root`.
        let absolute = |p: PathBuf| {
            if p.is_absolute() || (!cfg!(windows) && windows_absolute(&p)) {
                p
            } else {
                root.join(p)
            }
        };
        let base_dir = get("Settings", "base_directory")
            .map_or_else(|| root.to_path_buf(), |v| absolute(PathBuf::from(v)));
        let base_text = base_dir.to_string_lossy().into_owned();
        let setting = |key: &str, default: &str| {
            get("Settings", key).map_or_else(
                || base_dir.join(default),
                |v| absolute(PathBuf::from(v.replace("%BASE_DIR%", &base_text))),
            )
        };
        Instance {
            root: root.to_path_buf(),
            mods_dir: setting("mod_directory", "mods"),
            profiles_dir: setting("profiles_directory", "profiles"),
            overwrite_dir: setting("overwrite_directory", "overwrite"),
            downloads_dir: setting("download_directory", "downloads"),
            game_dir: get("General", "gamePath").map(PathBuf::from),
            game_name: get("General", "gameName"),
            selected_profile: get("General", "selected_profile"),
            base_dir,
        }
    }

    /// The settings file's path.
    pub fn ini_path(&self) -> PathBuf {
        self.root.join(INI_NAME)
    }

    /// The folder of profile `name` (not checked).
    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(name)
    }

    /// The game's Data folder (`<gamePath>\Data`), when `gamePath` is set.
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.game_dir.as_ref().map(|g| g.join("Data"))
    }

    /// The profiles that exist: folders under `profiles_dir` holding a `modlist.txt`,
    /// sorted by name.
    pub fn profiles(&self) -> Result<Vec<String>> {
        let rd = match std::fs::read_dir(&self.profiles_dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(IoError::io(&self.profiles_dir, e)),
        };
        let mut out: Vec<String> = rd
            .flatten()
            .filter(|e| e.path().join("modlist.txt").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        out.sort();
        Ok(out)
    }

    /// The root and the four folders, with what is on disk at each:
    /// `("root" | "mods" | "profiles" | "overwrite" | "downloads", path, state)`.
    pub fn folders(&self) -> Vec<(&'static str, &Path, FolderState)> {
        [
            ("root", self.root.as_path()),
            ("mods", self.mods_dir.as_path()),
            ("profiles", self.profiles_dir.as_path()),
            ("overwrite", self.overwrite_dir.as_path()),
            ("downloads", self.downloads_dir.as_path()),
        ]
        .into_iter()
        .map(|(what, p)| (what, p, FolderState::of(p)))
        .collect()
    }

    /// The paths among the root, the four folders and their ancestors that are symbolic
    /// links or junctions, deduplicated, in the order met.
    pub fn reparse_points(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        for (_, path, _) in self.folders() {
            for p in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
                if FolderState::of(p) == FolderState::Link && !out.iter().any(|o| o == p) {
                    out.push(p.to_path_buf());
                }
            }
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("kitkraft-io-inst-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn defaults_and_bytearray_values() {
        let root = temp("defaults");
        std::fs::write(
            root.join(INI_NAME),
            "\u{feff}[General]\r\ngameName=Starfield\r\ngamePath=@ByteArray(C:\\\\Games\\\\Starfield)\r\nselected_profile=@ByteArray(KitKraft Dev)\r\n",
        )
        .unwrap();
        for d in ["mods", "profiles/KitKraft Dev", "profiles/NoList"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("profiles/KitKraft Dev/modlist.txt"), b"").unwrap();
        let i = Instance::read(&root).unwrap();
        assert_eq!(i.base_dir, root);
        assert_eq!(i.mods_dir, root.join("mods"));
        assert_eq!(i.profiles_dir, root.join("profiles"));
        assert_eq!(i.overwrite_dir, root.join("overwrite"));
        assert_eq!(i.downloads_dir, root.join("downloads"));
        assert_eq!(i.game_dir, Some(PathBuf::from("C:\\Games\\Starfield")));
        assert_eq!(i.game_name.as_deref(), Some("Starfield"));
        assert_eq!(i.selected_profile.as_deref(), Some("KitKraft Dev"));
        assert_eq!(i.profiles().unwrap(), ["KitKraft Dev"]);
        let states: Vec<(&str, FolderState)> =
            i.folders().iter().map(|(w, _, s)| (*w, *s)).collect();
        assert_eq!(
            states,
            [
                ("root", FolderState::Folder),
                ("mods", FolderState::Folder),
                ("profiles", FolderState::Folder),
                ("overwrite", FolderState::Missing),
                ("downloads", FolderState::Missing),
            ]
        );
        assert!(i.reparse_points().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn settings_keys_base_dir_and_empty_values() {
        let root = temp("settings");
        let ini = Ini::parse(
            "[General]\nselected_profile=@ByteArray()\ngamePath=\n[Settings]\nbase_directory=@ByteArray(D:\\\\MO2)\nmod_directory=@ByteArray(%BASE_DIR%/m)\nprofiles_directory=rel\noverwrite_directory=\ndownload_directory=\"E:\\\\dl\"\n",
        );
        let i = Instance::from_ini(&root, Some(&ini));
        assert_eq!(i.base_dir, PathBuf::from("D:\\MO2"));
        assert_eq!(i.mods_dir, PathBuf::from("D:\\MO2/m"));
        assert_eq!(i.profiles_dir, root.join("rel"));
        // Empty values are absent: the default under the base directory.
        assert_eq!(i.overwrite_dir, PathBuf::from("D:\\MO2").join("overwrite"));
        assert_eq!(i.downloads_dir, PathBuf::from("E:\\dl"));
        assert_eq!(i.selected_profile, None);
        assert_eq!(i.game_dir, None);
        assert_eq!(i.data_dir(), None);
        let none = Instance::from_ini(&root, None);
        assert_eq!(none.mods_dir, root.join("mods"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn not_an_instance() {
        let root = temp("none");
        assert!(matches!(
            Instance::read(&root),
            Err(IoError::NotAnInstance(_))
        ));
        // A folder named like the settings file is not one either.
        std::fs::create_dir_all(root.join(INI_NAME)).unwrap();
        assert!(matches!(
            Instance::read(&root),
            Err(IoError::NotAnInstance(_))
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reading_writes_nothing() {
        let root = temp("ro");
        std::fs::write(root.join(INI_NAME), "[General]\n").unwrap();
        let before = std::fs::read_dir(&root).unwrap().count();
        let i = Instance::read(&root).unwrap();
        let _ = (i.folders(), i.reparse_points(), i.profiles().unwrap());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), before);
        let _ = std::fs::remove_dir_all(&root);
    }
}
