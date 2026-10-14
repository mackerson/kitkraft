//! The virtual Data directory an MO2 profile presents: a relative path resolves to the
//! overwrite folder, else the highest-priority enabled mod that ships it, else the game's
//! Data dir. Matching is ASCII case-insensitive and `\` and `/` are the same.
//!
//! A `meta.ini` at a mod's root is MO2's bookkeeping, not game data, and is skipped
//! (assumption; `docs/load-order.md`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::mo2::Profile;

/// Deepest directory nesting walked inside a mod.
const MAX_DEPTH: usize = 32;

/// Who provides a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    /// MO2's overwrite folder.
    Overwrite,
    /// An enabled managed mod (its folder name).
    Mod(String),
    /// The game's Data dir.
    GameData,
}

impl Provider {
    /// Text for results: `overwrite`, `mod:<name>`, `data`.
    pub fn label(&self) -> String {
        match self {
            Provider::Overwrite => "overwrite".into(),
            Provider::Mod(name) => format!("mod:{name}"),
            Provider::GameData => "data".into(),
        }
    }
}

/// A resolved file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Real path on disk.
    pub path: PathBuf,
    /// Who provides it.
    pub provider: Provider,
}

/// How much of each mod to index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// Only files at the root (enough for plugins).
    Root,
    /// Every file.
    Full,
}

/// The overlay.
#[derive(Debug, Clone)]
pub struct Overlay {
    files: HashMap<String, Resolved>,
    data_dir: PathBuf,
    /// Overwrite and enabled mod folders, highest priority first.
    roots: Vec<(Provider, PathBuf)>,
    depth: Depth,
    /// Enabled mods whose folder is missing.
    pub missing_mod_dirs: Vec<String>,
}

/// `Textures\A.dds` -> `textures/a.dds`.
pub fn normalize(rel: &str) -> String {
    rel.replace('\\', "/")
        .trim_matches('/')
        .to_ascii_lowercase()
}

impl Overlay {
    /// Index the overwrite folder and every enabled mod, highest priority first.
    pub fn build(profile: &Profile, depth: Depth) -> Overlay {
        let mut overlay = Overlay {
            files: HashMap::new(),
            data_dir: profile.data_dir.clone(),
            roots: Vec::new(),
            depth,
            missing_mod_dirs: Vec::new(),
        };
        overlay.add_tree(&profile.overwrite_dir, &Provider::Overwrite, depth);
        overlay
            .roots
            .push((Provider::Overwrite, profile.overwrite_dir.clone()));
        for m in profile.enabled_mods() {
            let dir = profile.mods_dir.join(&m.name);
            if dir.is_dir() {
                overlay.add_tree(&dir, &Provider::Mod(m.name.clone()), depth);
                overlay.roots.push((Provider::Mod(m.name.clone()), dir));
            } else {
                overlay.missing_mod_dirs.push(m.name.clone());
            }
        }
        overlay
    }

    fn add_tree(&mut self, root: &Path, provider: &Provider, depth: Depth) {
        let mut stack: Vec<(PathBuf, String, usize)> = vec![(root.to_path_buf(), String::new(), 0)];
        while let Some((dir, prefix, level)) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in read.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let name = entry.file_name().to_string_lossy().into_owned();
                let rel = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}/{name}")
                };
                if kind.is_dir() {
                    if depth == Depth::Full && level < MAX_DEPTH {
                        stack.push((entry.path(), rel, level.saturating_add(1)));
                    }
                } else if kind.is_file() {
                    if level == 0
                        && *provider != Provider::Overwrite
                        && name.eq_ignore_ascii_case("meta.ini")
                    {
                        continue;
                    }
                    self.files
                        .entry(normalize(&rel))
                        .or_insert_with(|| Resolved {
                            path: entry.path(),
                            provider: provider.clone(),
                        });
                }
            }
        }
    }

    /// Resolve `rel` (relative to Data). With a root-only overlay, a path below the root
    /// is looked up in each provider's folder in priority order.
    pub fn resolve(&self, rel: &str) -> Option<Resolved> {
        let key = normalize(rel);
        if let Some(found) = self.files.get(&key) {
            return Some(found.clone());
        }
        if self.depth == Depth::Root && key.contains('/') {
            for (provider, root) in &self.roots {
                if let Some(path) = find_ci(root, &key) {
                    return Some(Resolved {
                        path,
                        provider: provider.clone(),
                    });
                }
            }
        }
        find_ci(&self.data_dir, &key).map(|path| Resolved {
            path,
            provider: Provider::GameData,
        })
    }

    /// Root-level `.ba2` files whose name starts with `<stem> - ` (case-insensitive), from
    /// the overlay and the game Data dir, one per name (the providing one), Localization
    /// archives first.
    pub fn archives_for(&self, stem: &str) -> Vec<PathBuf> {
        let prefix = format!("{} - ", stem.to_ascii_lowercase());
        let matches = |name: &str| name.starts_with(&prefix) && name.ends_with(".ba2");
        let mut names: Vec<String> = self
            .files
            .keys()
            .filter(|k| !k.contains('/') && matches(k))
            .cloned()
            .collect();
        if let Ok(read) = std::fs::read_dir(&self.data_dir) {
            for e in read.flatten() {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                if matches(&n) && !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        let mut paths: Vec<PathBuf> = names
            .iter()
            .filter_map(|n| self.resolve(n).map(|r| r.path))
            .collect();
        kitcraft_doc::strings::sort_archives(&mut paths);
        paths
    }

    /// Files indexed from mods and overwrite.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether no mod file was indexed.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Find `key` (normalized) under `root`, matching each component case-insensitively.
fn find_ci(root: &Path, key: &str) -> Option<PathBuf> {
    let mut dir = root.to_path_buf();
    let parts: Vec<&str> = key.split('/').filter(|p| !p.is_empty()).collect();
    let last = parts.len().checked_sub(1)?;
    for (i, part) in parts.iter().enumerate() {
        // List the directory so the match is case-insensitive on any file system and the
        // path keeps the case stored on disk.
        let found = std::fs::read_dir(&dir).ok()?.flatten().find_map(|e| {
            e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(part)
                .then(|| e.path())
        })?;
        let is_dir = found.is_dir();
        if i == last {
            return (!is_dir).then_some(found);
        }
        if !is_dir {
            return None;
        }
        dir = found;
    }
    None
}

impl kitcraft_doc::strings::StringSource for Overlay {
    fn loose(&self, rel: &str) -> Option<PathBuf> {
        self.resolve(rel).map(|r| r.path)
    }
    fn archives(&self, stem: &str) -> Vec<PathBuf> {
        self.archives_for(stem)
    }
}
