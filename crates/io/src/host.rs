//! Where the game, Mod Organizer and the modding tools are on this machine (`kit doctor`,
//! and the default Data dir and MO2 instance every command falls back to).
//!
//! Read-only, like the rest of the crate: files are only read or looked at, and the
//! Windows registry is read through `reg.exe query` (no write, no handle kept). Every
//! answer is a [`Found`]: the path and, in words, how it was found, so a report can say
//! where a value came from. What is looked at, in order:
//!
//! - **Game install**: `HKCU\Software\Valve\Steam` `SteamPath`, then the libraries listed
//!   in its `steamapps\libraryfolders.vdf` (the library that lists app 1716740 first;
//!   `appmanifest_1716740.acf` names the folder), then the common install folders
//!   ([`Host::system`]). A folder counts when it holds `Starfield.exe`.
//! - **MO2 instance**: `C:\Modding\MO2-Starfield` (the owner's portable instance), then
//!   MO2's own registry value `HKCU\Software\Mod Organizer Team\Mod Organizer`
//!   `CurrentInstance` (a global instance under `%LOCALAPPDATA%\ModOrganizer`), then every
//!   other global instance there. Only instances whose `gameName` is Starfield (or unset).
//! - **Tools**: an environment variable when the tool has one, the instance's
//!   `[customExecutables]` binaries, then the default tool folders.

use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::ini::Ini;
use crate::instance::INI_NAME;

/// Starfield's Steam app id.
pub const STARFIELD_APP_ID: &str = "1716740";

/// The owner's portable MO2 instance (AGENTS.md, "Windows specifics").
pub const DEFAULT_MO2: &str = r"C:\Modding\MO2-Starfield";

/// Steam's registry key and value.
pub const STEAM_KEY: &str = r"HKCU\Software\Valve\Steam";
/// See [`STEAM_KEY`].
pub const STEAM_VALUE: &str = "SteamPath";
/// MO2's registry key and value (the global instance it opens).
pub const MO2_KEY: &str = r"HKCU\Software\Mod Organizer Team\Mod Organizer";
/// See [`MO2_KEY`].
pub const MO2_VALUE: &str = "CurrentInstance";

/// A path and how it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The path.
    pub path: PathBuf,
    /// How it was found, in words (`Steam registry ... + libraryfolders.vdf`).
    pub how: String,
}

impl Found {
    fn new(path: PathBuf, how: impl Into<String>) -> Found {
        Found {
            path,
            how: how.into(),
        }
    }
}

/// Reads one registry value as text (`REG_SZ` / `REG_EXPAND_SZ` data as stored).
pub trait Registry {
    /// The data of `value` under `key` (`HKCU\...`), or `None` when absent or unreadable.
    fn read(&self, key: &str, value: &str) -> Option<String>;
}

/// The real registry through `reg.exe query` (Windows; `None` everywhere else).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRegistry;

impl Registry for SystemRegistry {
    fn read(&self, key: &str, value: &str) -> Option<String> {
        if !cfg!(windows) {
            return None;
        }
        let out = std::process::Command::new("reg")
            .args(["query", key, "/v", value])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        parse_reg_query(&String::from_utf8_lossy(&out.stdout), value)
    }
}

/// A registry with fixed values (tests, and callers that must not touch the real one).
#[derive(Debug, Clone, Default)]
pub struct FixedRegistry(pub Vec<(String, String, String)>);

impl Registry for FixedRegistry {
    fn read(&self, key: &str, value: &str) -> Option<String> {
        self.0
            .iter()
            .find(|(k, v, _)| k.eq_ignore_ascii_case(key) && v.eq_ignore_ascii_case(value))
            .map(|(_, _, d)| d.clone())
    }
}

/// The data of `value` in `reg query` output: the line `    <value>    REG_<kind>    <data>`.
pub fn parse_reg_query(output: &str, value: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let rest = line.trim_start().strip_prefix(value)?;
        let rest = rest.trim_start_matches(' ');
        let rest = rest.strip_prefix("REG_")?;
        let (_kind, data) = rest.split_once("    ")?;
        Some(data.trim_end().to_string())
    })
}

/// Environment lookup: a variable's value, or `None`.
pub type EnvLookup = Box<dyn Fn(&str) -> Option<OsString>>;

/// Where to look: the registry, the environment, and the fixed folders tried after them.
pub struct Host {
    /// The registry reader.
    pub registry: Box<dyn Registry>,
    /// Environment lookup (`std::env::var_os` for the real host).
    pub env: EnvLookup,
    /// Game install folders tried after Steam's registry and libraries.
    pub game_candidates: Vec<PathBuf>,
    /// MO2 instance roots tried before the registry and the global instances.
    pub mo2_candidates: Vec<PathBuf>,
    /// Folders searched for tools (xEdit) after the variables and MO2's executables.
    pub tool_dirs: Vec<PathBuf>,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("game_candidates", &self.game_candidates)
            .field("mo2_candidates", &self.mo2_candidates)
            .field("tool_dirs", &self.tool_dirs)
            .finish_non_exhaustive()
    }
}

/// `C:\...` text as a path, with `/` turned into `\` on Windows (Steam's registry value
/// and MO2's settings use forward slashes).
pub fn native_path(text: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(text.replace('/', "\\"))
    } else {
        PathBuf::from(text)
    }
}

impl Host {
    /// This machine: the real registry and environment, the common Steam and Xbox
    /// install folders, the owner's MO2 instance and `C:\Modding\tools\xEdit`.
    pub fn system() -> Host {
        Host {
            registry: Box::new(SystemRegistry),
            env: Box::new(|name| std::env::var_os(name)),
            game_candidates: [
                r"C:\Program Files (x86)\Steam\steamapps\common\Starfield",
                r"C:\Program Files\Steam\steamapps\common\Starfield",
                r"C:\SteamLibrary\steamapps\common\Starfield",
                r"D:\SteamLibrary\steamapps\common\Starfield",
                r"D:\Steam\steamapps\common\Starfield",
                r"E:\SteamLibrary\steamapps\common\Starfield",
                r"C:\XboxGames\Starfield\Content",
            ]
            .into_iter()
            .map(PathBuf::from)
            .collect(),
            mo2_candidates: vec![PathBuf::from(DEFAULT_MO2)],
            tool_dirs: vec![PathBuf::from(r"C:\Modding\tools\xEdit")],
        }
    }

    fn env_path(&self, name: &str) -> Option<PathBuf> {
        (self.env)(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    }

    /// Steam's folder from its registry value.
    pub fn steam_root(&self) -> Option<Found> {
        let raw = self.registry.read(STEAM_KEY, STEAM_VALUE)?;
        let raw = raw.trim();
        (!raw.is_empty()).then(|| {
            Found::new(
                native_path(raw),
                format!("Steam registry {STEAM_KEY} {STEAM_VALUE}"),
            )
        })
    }

    /// The game install: the first folder holding `Starfield.exe` among Steam's libraries
    /// and then [`Host::game_candidates`]; also every place looked at (for a fix line).
    pub fn game_dir(&self) -> (Option<Found>, Vec<PathBuf>) {
        let mut looked: Vec<PathBuf> = Vec::new();
        let mut candidates: Vec<Found> = Vec::new();
        if let Some(steam) = self.steam_root() {
            let vdf = steam.path.join("steamapps").join("libraryfolders.vdf");
            let libraries = std::fs::read(&vdf)
                .map(|b| parse_library_folders(&String::from_utf8_lossy(&b)))
                .unwrap_or_default();
            // Libraries that list the game first, then the others, then Steam's own folder.
            let mut ordered: Vec<(PathBuf, String)> = Vec::new();
            for (lib, _) in libraries.iter().filter(|(_, a)| *a) {
                ordered.push((
                    lib.clone(),
                    format!(
                        "{} + libraryfolders.vdf (lists app {STARFIELD_APP_ID})",
                        steam.how
                    ),
                ));
            }
            for (lib, _) in libraries.iter().filter(|(_, a)| !*a) {
                ordered.push((lib.clone(), format!("{} + libraryfolders.vdf", steam.how)));
            }
            ordered.push((steam.path.clone(), steam.how.clone()));
            for (lib, how) in ordered {
                let apps = lib.join("steamapps");
                let folder =
                    std::fs::read(apps.join(format!("appmanifest_{STARFIELD_APP_ID}.acf")))
                        .ok()
                        .and_then(|b| vdf_value(&String::from_utf8_lossy(&b), "installdir"))
                        .unwrap_or_else(|| "Starfield".to_string());
                candidates.push(Found::new(apps.join("common").join(folder), how));
            }
        }
        for c in &self.game_candidates {
            candidates.push(Found::new(c.clone(), "common install folder"));
        }
        for c in candidates {
            if looked.iter().any(|l| same_path(l, &c.path)) {
                continue;
            }
            looked.push(c.path.clone());
            if c.path.join("Starfield.exe").is_file() {
                return (Some(c), looked);
            }
        }
        (None, looked)
    }

    /// Starfield MO2 instances, in the order [`Host`] documents; the first is the default.
    pub fn mo2_instances(&self) -> Vec<Found> {
        let mut out: Vec<Found> = Vec::new();
        let push = |out: &mut Vec<Found>, path: PathBuf, how: String| {
            if !out.iter().any(|f| same_path(&f.path, &path)) && is_starfield_instance(&path) {
                out.push(Found::new(path, how));
            }
        };
        for c in &self.mo2_candidates {
            push(&mut out, c.clone(), "default location".to_string());
        }
        let global = self
            .env_path("LOCALAPPDATA")
            .map(|l| l.join("ModOrganizer"));
        if let (Some(name), Some(global)) = (self.registry.read(MO2_KEY, MO2_VALUE), &global) {
            let name = name.trim();
            if !name.is_empty() {
                push(
                    &mut out,
                    global.join(name),
                    format!("MO2 registry {MO2_KEY} {MO2_VALUE}"),
                );
            }
        }
        if let Some(global) = &global {
            let mut dirs: Vec<PathBuf> = std::fs::read_dir(global)
                .map(|rd| rd.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
            dirs.sort();
            for d in dirs {
                push(
                    &mut out,
                    d,
                    format!("MO2 global instance in {}", global.display()),
                );
            }
        }
        out
    }

    /// A tool by file name (`xSFEdit64.exe`): the variable `env` names (a file, or a file
    /// next to it), then the MO2 instance's `[customExecutables]` binaries (the file, or
    /// one next to them), then [`Host::tool_dirs`].
    pub fn find_tool(&self, file: &str, env: Option<&str>, mo2: Option<&Path>) -> Option<Found> {
        if let Some((var, p)) = env.and_then(|v| self.env_path(v).map(|p| (v, p))) {
            for c in [p.clone(), p.with_file_name(file)] {
                if is_named(&c, file) && c.is_file() {
                    return Some(Found::new(c, format!("environment variable {var}")));
                }
            }
        }
        if let Some(root) = mo2 {
            for binary in custom_executables(root) {
                for c in [binary.clone(), binary.with_file_name(file)] {
                    if is_named(&c, file) && c.is_file() {
                        return Some(Found::new(
                            c,
                            format!("MO2 executables in {}", root.join(INI_NAME).display()),
                        ));
                    }
                }
            }
        }
        self.tool_dirs.iter().find_map(|d| {
            let c = d.join(file);
            c.is_file()
                .then(|| Found::new(c, format!("default tool folder {}", d.display())))
        })
    }
}

fn is_named(path: &Path, file: &str) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(file))
}

/// Whether two paths name the same place (ASCII case and separators ignored on Windows).
pub fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let s = p.to_string_lossy().replace('/', "\\");
        let s = s.trim_end_matches('\\').to_string();
        if cfg!(windows) {
            s.to_ascii_lowercase()
        } else {
            s
        }
    };
    norm(a) == norm(b)
}

fn is_starfield_instance(root: &Path) -> bool {
    let ini = root.join(INI_NAME);
    let Ok(bytes) = std::fs::read(&ini) else {
        return false;
    };
    let ini = Ini::parse(&String::from_utf8_lossy(&bytes));
    ini.get("General", "gameName")
        .is_none_or(|g| g.trim().is_empty() || g.trim().eq_ignore_ascii_case("Starfield"))
}

/// The `[customExecutables]` binaries of the instance at `root`.
pub fn custom_executables(root: &Path) -> Vec<PathBuf> {
    let Ok(bytes) = std::fs::read(root.join(INI_NAME)) else {
        return Vec::new();
    };
    Ini::parse(&String::from_utf8_lossy(&bytes))
        .entries("customExecutables")
        .into_iter()
        .filter(|(k, _)| {
            k.rsplit(['\\', '/'])
                .next()
                .is_some_and(|last| last.eq_ignore_ascii_case("binary"))
        })
        .map(|(_, v)| native_path(v.trim()))
        .collect()
}

/// The quoted tokens of one VDF line, unescaped (`\\` -> `\`, `\"` -> `"`).
fn vdf_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut token = String::new();
        loop {
            match chars.next() {
                None | Some('"') => break,
                Some('\\') => match chars.next() {
                    Some(e @ ('\\' | '"')) => token.push(e),
                    Some(other) => {
                        token.push('\\');
                        token.push(other);
                    }
                    None => token.push('\\'),
                },
                Some(other) => token.push(other),
            }
        }
        out.push(token);
    }
    out
}

/// The first `"key" "value"` pair named `key` in VDF/ACF text.
pub fn vdf_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|line| match vdf_tokens(line).as_slice() {
            [k, v] if k.eq_ignore_ascii_case(key) => Some(v.clone()),
            _ => None,
        })
}

/// Steam's `libraryfolders.vdf`: each library's folder and whether its `apps` list names
/// Starfield ([`STARFIELD_APP_ID`]), in file order.
pub fn parse_library_folders(text: &str) -> Vec<(PathBuf, bool)> {
    let mut out: Vec<(PathBuf, bool)> = Vec::new();
    for line in text.lines() {
        match vdf_tokens(line).as_slice() {
            [k, v] if k.eq_ignore_ascii_case("path") => out.push((native_path(v), false)),
            [k, _] if k == STARFIELD_APP_ID => {
                if let Some(last) = out.last_mut() {
                    last.1 = true;
                }
            }
            _ => {}
        }
    }
    out
}

/// Bytes of the PE headers read at most (headers and the section table).
const PE_HEADER_CAP: u64 = 64 * 1024;
/// Bytes of a `.rsrc` section read at most.
const RSRC_CAP: u64 = 64 * 1024 * 1024;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([*s.first()?, *s.get(1)?]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([
        *s.first()?,
        *s.get(1)?,
        *s.get(2)?,
        *s.get(3)?,
    ]))
}

/// The file version of a Windows executable (`1.16.244.0`): `VS_FIXEDFILEINFO` in the
/// `.rsrc` section (signature `0xFEEF04BD`, then the struct version, then the version's
/// high and low words). `None` when the file is not a PE or has no version resource.
pub fn pe_file_version(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = Vec::new();
    f.by_ref().take(PE_HEADER_CAP).read_to_end(&mut head).ok()?;
    let (offset, size) = rsrc_section(&head)?;
    let size = u64::from(size).min(RSRC_CAP);
    f.seek(SeekFrom::Start(u64::from(offset))).ok()?;
    let mut rsrc = Vec::new();
    f.take(size).read_to_end(&mut rsrc).ok()?;
    fixed_file_version(&rsrc)
}

/// `.rsrc`'s file offset and size from PE headers.
fn rsrc_section(head: &[u8]) -> Option<(u32, u32)> {
    if head.get(0..2)? != b"MZ" {
        return None;
    }
    let pe = usize::try_from(u32_at(head, 0x3C)?).ok()?;
    if head.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let coff = pe.checked_add(4)?;
    let sections = u16_at(head, coff.checked_add(2)?)?;
    let optional = usize::from(u16_at(head, coff.checked_add(16)?)?);
    let mut at = coff.checked_add(20)?.checked_add(optional)?;
    for _ in 0..sections {
        let name = head.get(at..at.checked_add(8)?)?;
        if name.starts_with(b".rsrc") {
            let size = u32_at(head, at.checked_add(16)?)?;
            let offset = u32_at(head, at.checked_add(20)?)?;
            return Some((offset, size));
        }
        at = at.checked_add(40)?;
    }
    None
}

/// The version in the first `VS_FIXEDFILEINFO` (4-byte aligned) of `rsrc`.
fn fixed_file_version(rsrc: &[u8]) -> Option<String> {
    const SIGNATURE: u32 = 0xFEEF_04BD;
    let mut at = 0usize;
    while at.checked_add(16)? <= rsrc.len() {
        if u32_at(rsrc, at)? == SIGNATURE {
            let ms = u32_at(rsrc, at.checked_add(8)?)?;
            let ls = u32_at(rsrc, at.checked_add(12)?)?;
            return Some(format!(
                "{}.{}.{}.{}",
                ms >> 16,
                ms & 0xFFFF,
                ls >> 16,
                ls & 0xFFFF
            ));
        }
        at = at.checked_add(4)?;
    }
    None
}

/// Files and bytes under `dir` (not following links; at most `max_depth` levels and
/// `max_entries` entries). `(0, 0)` when it does not exist.
pub fn dir_size(dir: &Path, max_depth: usize, max_entries: usize) -> (u64, u64) {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut seen = 0usize;
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            seen = seen.saturating_add(1);
            if seen > max_entries {
                return (files, bytes);
            }
            let Ok(m) = e.path().symlink_metadata() else {
                continue;
            };
            if m.is_dir() && depth < max_depth {
                stack.push((e.path(), depth.saturating_add(1)));
            } else if m.is_file() {
                files = files.saturating_add(1);
                bytes = bytes.saturating_add(m.len());
            }
        }
    }
    (files, bytes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("kitkraft-io-host-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn host(registry: FixedRegistry, local: Option<&Path>) -> Host {
        let local = local.map(Path::to_path_buf);
        Host {
            registry: Box::new(registry),
            env: Box::new(move |name| {
                (name == "LOCALAPPDATA")
                    .then(|| local.clone().map(PathBuf::into_os_string))
                    .flatten()
            }),
            game_candidates: Vec::new(),
            mo2_candidates: Vec::new(),
            tool_dirs: Vec::new(),
        }
    }

    #[test]
    fn reg_query_output_is_parsed() {
        let out = "\r\nHKEY_CURRENT_USER\\Software\\Valve\\Steam\r\n    SteamPath    REG_SZ    c:/program files (x86)/steam\r\n\r\n";
        assert_eq!(
            parse_reg_query(out, "SteamPath").as_deref(),
            Some("c:/program files (x86)/steam")
        );
        let empty = "    CurrentInstance    REG_SZ    \r\n";
        assert_eq!(
            parse_reg_query(empty, "CurrentInstance").as_deref(),
            Some("")
        );
        assert_eq!(parse_reg_query(out, "Other"), None);
    }

    #[test]
    fn library_folders_and_the_app_list() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"228980\"\t\t\"1\"\n\t\t}\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"1716740\"\t\t\"123\"\n\t\t}\n\t}\n}\n";
        let libs = parse_library_folders(vdf);
        assert_eq!(libs.len(), 2);
        assert_eq!(libs[0].0, native_path("C:\\Program Files (x86)\\Steam"));
        assert!(!libs[0].1);
        assert_eq!(libs[1].0, native_path("D:\\SteamLibrary"));
        assert!(libs[1].1);
        assert_eq!(
            vdf_value(
                "\"AppState\"\n{\n\t\"installdir\"\t\t\"Starfield\"\n}",
                "installdir"
            )
            .as_deref(),
            Some("Starfield")
        );
    }

    #[test]
    fn game_found_through_steam_registry_and_library() {
        let root = temp("steam");
        let steam = root.join("Steam");
        let lib = root.join("Lib");
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps/common/Starfield Game")).unwrap();
        std::fs::write(
            lib.join("steamapps/common/Starfield Game/Starfield.exe"),
            b"MZ",
        )
        .unwrap();
        std::fs::write(
            lib.join("steamapps/appmanifest_1716740.acf"),
            "\"AppState\"\n{\n\t\"installdir\"\t\t\"Starfield Game\"\n}\n",
        )
        .unwrap();
        let esc = |p: &Path| p.display().to_string().replace('\\', "\\\\");
        std::fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\"0\"\n{{\n\"path\" \"{}\"\n}}\n\"1\"\n{{\n\"path\" \"{}\"\n\"apps\"\n{{\n\"1716740\" \"1\"\n}}\n}}\n}}\n",
                esc(&steam),
                esc(&lib)
            ),
        )
        .unwrap();
        let reg = FixedRegistry(vec![(
            STEAM_KEY.to_string(),
            STEAM_VALUE.to_string(),
            steam.display().to_string(),
        )]);
        let (found, looked) = host(reg, None).game_dir();
        let found = found.unwrap();
        assert!(same_path(
            &found.path,
            &lib.join("steamapps/common/Starfield Game")
        ));
        assert!(found.how.contains("libraryfolders.vdf"), "{}", found.how);
        assert!(found.how.contains("1716740"), "{}", found.how);
        assert_eq!(looked.len(), 1);
        // Without the registry nothing is found and the fixed candidates are listed.
        let mut h = host(FixedRegistry::default(), None);
        h.game_candidates = vec![root.join("Nowhere")];
        let (none, looked) = h.game_dir();
        assert!(none.is_none());
        assert_eq!(looked, [root.join("Nowhere")]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mo2_instances_default_registry_and_global() {
        let root = temp("mo2");
        let portable = root.join("Portable");
        let local = root.join("Local");
        let global = local.join("ModOrganizer");
        for (d, game) in [
            (portable.clone(), "Starfield"),
            (global.join("Current"), "Starfield"),
            (global.join("Another"), ""),
            (global.join("Skyrim"), "Skyrim Special Edition"),
        ] {
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(
                d.join(INI_NAME),
                format!("[General]\r\ngameName={game}\r\n"),
            )
            .unwrap();
        }
        let reg = FixedRegistry(vec![(
            MO2_KEY.to_string(),
            MO2_VALUE.to_string(),
            "Current".to_string(),
        )]);
        let mut h = host(reg, Some(&local));
        h.mo2_candidates = vec![portable.clone(), root.join("Missing")];
        let found = h.mo2_instances();
        let paths: Vec<&Path> = found.iter().map(|f| f.path.as_path()).collect();
        assert_eq!(
            paths,
            [
                portable.as_path(),
                global.join("Current").as_path(),
                global.join("Another").as_path()
            ]
        );
        assert_eq!(found[0].how, "default location");
        assert!(found[1].how.contains("CurrentInstance"), "{}", found[1].how);
        assert!(found[2].how.contains("global instance"), "{}", found[2].how);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tools_from_env_mo2_executables_and_folders() {
        let root = temp("tools");
        let xedit = root.join("x Edit");
        std::fs::create_dir_all(&xedit).unwrap();
        for f in ["xSFEdit64.exe", "xDump64.exe"] {
            std::fs::write(xedit.join(f), b"MZ").unwrap();
        }
        let mo2 = root.join("MO2");
        std::fs::create_dir_all(&mo2).unwrap();
        std::fs::write(
            mo2.join(INI_NAME),
            format!(
                "[customExecutables]\r\n1\\binary={}\r\n",
                xedit
                    .join("xSFEdit64.exe")
                    .display()
                    .to_string()
                    .replace('\\', "/")
            ),
        )
        .unwrap();
        let h = host(FixedRegistry::default(), None);
        let f = h.find_tool("xDump64.exe", None, Some(&mo2)).unwrap();
        assert!(same_path(&f.path, &xedit.join("xDump64.exe")));
        assert!(f.how.contains("MO2 executables"), "{}", f.how);
        assert!(h.find_tool("xDump64.exe", None, None).is_none());
        let mut h = h;
        h.tool_dirs = vec![xedit.clone()];
        let f = h.find_tool("xSFEdit64.exe", None, None).unwrap();
        assert!(f.how.contains("default tool folder"), "{}", f.how);
        let named = xedit.join("xSFEdit64.exe").into_os_string();
        h.env = Box::new(move |n| (n == "KITKRAFT_SF1EDIT").then(|| named.clone()));
        let f = h
            .find_tool("xDump64.exe", Some("KITKRAFT_SF1EDIT"), None)
            .unwrap();
        assert!(same_path(&f.path, &xedit.join("xDump64.exe")));
        assert_eq!(f.how, "environment variable KITKRAFT_SF1EDIT");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A minimal PE: DOS header, `PE\0\0`, one `.rsrc` section holding a
    /// `VS_FIXEDFILEINFO` for 1.16.244.0.
    #[test]
    fn pe_file_version_reads_the_fixed_file_info() {
        let mut b = vec![0u8; 0x300];
        b[0..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        b[0x40..0x44].copy_from_slice(b"PE\0\0");
        b[0x46..0x48].copy_from_slice(&1u16.to_le_bytes()); // one section
        b[0x54..0x56].copy_from_slice(&0u16.to_le_bytes()); // no optional header
        let sec = 0x58;
        b[sec..sec + 5].copy_from_slice(b".rsrc");
        b[sec + 16..sec + 20].copy_from_slice(&0x100u32.to_le_bytes()); // size
        b[sec + 20..sec + 24].copy_from_slice(&0x200u32.to_le_bytes()); // offset
        let at = 0x200 + 0x24;
        b[at..at + 4].copy_from_slice(&0xFEEF_04BDu32.to_le_bytes());
        b[at + 8..at + 12].copy_from_slice(&((1u32 << 16) | 16).to_le_bytes());
        b[at + 12..at + 16].copy_from_slice(&(244u32 << 16).to_le_bytes());
        let root = temp("pe");
        let exe = root.join("Game.exe");
        std::fs::write(&exe, &b).unwrap();
        assert_eq!(pe_file_version(&exe).as_deref(), Some("1.16.244.0"));
        // Not a PE, a truncated one, a missing file.
        std::fs::write(&exe, b"not a pe").unwrap();
        assert_eq!(pe_file_version(&exe), None);
        std::fs::write(&exe, &b[..0x50]).unwrap();
        assert_eq!(pe_file_version(&exe), None);
        assert_eq!(pe_file_version(&root.join("missing.exe")), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dir_size_counts_files() {
        let root = temp("size");
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("one"), [0u8; 10]).unwrap();
        std::fs::write(root.join("a/b/two"), [0u8; 5]).unwrap();
        assert_eq!(dir_size(&root, 8, 1000), (2, 15));
        assert_eq!(dir_size(&root, 0, 1000), (1, 10));
        assert_eq!(dir_size(&root.join("missing"), 8, 1000), (0, 0));
        let _ = std::fs::remove_dir_all(&root);
    }
}
