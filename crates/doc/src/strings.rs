//! String tables for localized plugins (M2.D7).
//!
//! A localized plugin's lstrings (`FULL`, `DESC`, ...) hold IDs into
//! `strings/<plugin stem>_<language>.{strings,dlstrings,ilstrings}` (plan format facts,
//! `uesp-skyrim`; names and placement per lane G's corpus survey in `plan/STATUS.md`:
//! base-game tables in `Starfield - Localization.ba2`, DLC and Creation tables in
//! `<Plugin> - Localization.ba2` or `<Plugin> - Main.ba2`). A table is looked up as a loose
//! file first (`Strings\<name>` through the caller's [`StringSource`], e.g. the MO2
//! overlay), then in the plugin's archives (`<stem> - *.ba2`, Localization archives first).
//! The three kinds hold disjoint IDs in the official tables, so an ID is looked up in every
//! table found and the kind it came from is reported.
//!
//! Tables are read once per plugin and kept. A missing table is reported with the places
//! looked at, never as an absent value.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use kitkraft_strings::{table_name, Kind, Table};

/// Where table files can come from.
pub trait StringSource: Send {
    /// A loose file at the Data-relative path `rel` (`Strings/starfield_en.strings`).
    fn loose(&self, rel: &str) -> Option<PathBuf>;
    /// Archives that may hold tables of the plugin with stem `stem`, in lookup order.
    fn archives(&self, stem: &str) -> Vec<PathBuf>;
}

/// A plain Data directory: loose files under it, archives at its root.
#[derive(Debug, Clone)]
pub struct DirSource {
    /// The Data dir.
    pub data_dir: PathBuf,
}

/// Find `rel` (`/`-separated) under `root`, ASCII case-insensitively.
pub fn find_ci(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut dir = root.to_path_buf();
    let parts: Vec<&str> = rel.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    let last = parts.len().checked_sub(1)?;
    for (i, part) in parts.iter().enumerate() {
        let found = std::fs::read_dir(&dir).ok()?.flatten().find_map(|e| {
            e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(part)
                .then(|| e.path())
        })?;
        if i == last {
            return found.is_file().then_some(found);
        }
        dir = found;
    }
    None
}

/// Archives in `dir` named `<stem> - *.ba2`, Localization archives first, then Main.
pub fn plugin_archives(dir: &Path, stem: &str) -> Vec<PathBuf> {
    let prefix = format!("{} - ", stem.to_ascii_lowercase());
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_ascii_lowercase())
                        .unwrap_or_default();
                    name.starts_with(&prefix) && name.ends_with(".ba2")
                })
                .collect()
        })
        .unwrap_or_default();
    sort_archives(&mut found);
    found
}

/// Localization archives first, then Main, then the rest, each by name.
pub fn sort_archives(archives: &mut [PathBuf]) {
    let rank = |p: &PathBuf| {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let r = if name.contains("localization") {
            0
        } else if name.contains(" - main") {
            1
        } else {
            2
        };
        (r, name)
    };
    archives.sort_by_key(rank);
}

impl StringSource for DirSource {
    fn loose(&self, rel: &str) -> Option<PathBuf> {
        find_ci(&self.data_dir, rel)
    }
    fn archives(&self, stem: &str) -> Vec<PathBuf> {
        plugin_archives(&self.data_dir, stem)
    }
}

/// A table read for one plugin.
#[derive(Debug, Clone)]
pub struct LoadedTable {
    /// Kind.
    pub kind: Kind,
    /// The strings.
    pub table: Table,
    /// For evidence: the loose file name, or `<archive>:<entry path>`.
    pub origin: String,
}

/// A resolved string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The text (lossy UTF-8: bytes that are not UTF-8 show as U+FFFD; see `bytes`).
    pub text: String,
    /// The string exactly as the table stores it (without the NUL terminator), for copying
    /// and comparing: a few official entries are not UTF-8 (cp1252-style bytes), and `text`
    /// cannot round-trip them (lane G, `plan/STATUS.md`).
    pub bytes: Vec<u8>,
    /// Whether `bytes` is valid UTF-8 (then `text` is exact).
    pub utf8: bool,
    /// Table origin (see [`LoadedTable::origin`]).
    pub origin: String,
    /// Kind of the table it came from.
    pub kind: &'static str,
    /// Offset of the string in the table (its length prefix for DL/IL tables).
    pub offset: Option<u64>,
    /// Bytes it occupies in the table.
    pub length: u64,
}

/// String tables of a load order, read on first use per plugin.
pub struct StringTables {
    language: String,
    source: Box<dyn StringSource>,
    plugins: HashMap<String, Result<Vec<LoadedTable>, String>>,
}

impl std::fmt::Debug for StringTables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StringTables")
            .field("language", &self.language)
            .field("plugins", &self.plugins.len())
            .finish()
    }
}

impl StringTables {
    /// Tables in `language`, found through `source`.
    pub fn new(language: &str, source: Box<dyn StringSource>) -> StringTables {
        StringTables {
            language: language.to_ascii_lowercase(),
            source,
            plugins: HashMap::new(),
        }
    }

    /// The language.
    pub fn language(&self) -> &str {
        &self.language
    }

    fn load(&self, plugin: &str) -> Result<Vec<LoadedTable>, String> {
        let stem = Path::new(plugin)
            .file_stem()
            .map_or_else(|| plugin.to_string(), |s| s.to_string_lossy().into_owned());
        let archives = self.source.archives(&stem);
        let mut out = Vec::new();
        let mut errors = Vec::new();
        let mut opened: Vec<(PathBuf, kitkraft_ba2::Archive, std::fs::File)> = Vec::new();
        let mut archives_tried = false;
        for kind in Kind::ALL {
            let name = table_name(plugin, &self.language, kind);
            let rel = format!("strings/{name}");
            if let Some(path) = self.source.loose(&rel) {
                match Table::read_file(&path) {
                    Ok(table) => {
                        out.push(LoadedTable {
                            kind,
                            table,
                            origin: format!("Strings\\{}", crate::index::file_name_of(&path)),
                        });
                        continue;
                    }
                    Err(e) => errors.push(format!("{}: {e}", path.display())),
                }
            }
            if !archives_tried {
                archives_tried = true;
                for a in &archives {
                    match kitkraft_ba2::Archive::open(a) {
                        Ok((archive, file)) => opened.push((a.clone(), archive, file)),
                        Err(e) => errors.push(format!("{}: {e}", a.display())),
                    }
                }
            }
            for (path, archive, file) in &mut opened {
                let Some(index) = archive.find(&rel) else {
                    continue;
                };
                let archive_name = crate::index::file_name_of(path);
                match archive
                    // 256 MiB: the cap `strings.dump` uses (lane G review round 2).
                    .extract_vec(file, index, 256 << 20)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| Table::parse(kind, &bytes).map_err(|e| e.to_string()))
                {
                    Ok(table) => {
                        out.push(LoadedTable {
                            kind,
                            table,
                            origin: format!("{archive_name}:{rel}"),
                        });
                        break;
                    }
                    Err(e) => errors.push(format!("{archive_name}:{rel}: {e}")),
                }
            }
        }
        if out.is_empty() {
            let mut why = format!(
                "no {} string tables for {plugin} (looked for loose Strings\\{}_{}.* and in {} archive(s){})",
                self.language,
                stem.to_ascii_lowercase(),
                self.language,
                archives.len(),
                if archives.is_empty() {
                    String::new()
                } else {
                    format!(
                        ": {}",
                        archives
                            .iter()
                            .map(|a| crate::index::file_name_of(a))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            );
            if !errors.is_empty() {
                why.push_str("; errors: ");
                why.push_str(&errors.join("; "));
            }
            return Err(why);
        }
        Ok(out)
    }

    /// The text of string `id` of `plugin`, or why it is unknown.
    pub fn lookup(&mut self, plugin: &str, id: u32) -> Result<Resolved, String> {
        let key = plugin.to_ascii_lowercase();
        if !self.plugins.contains_key(&key) {
            let loaded = self.load(plugin);
            self.plugins.insert(key.clone(), loaded);
        }
        let tables = match self.plugins.get(&key) {
            Some(Ok(t)) => t,
            Some(Err(e)) => return Err(e.clone()),
            None => return Err("string tables not loaded".into()),
        };
        for t in tables {
            if let Some(e) = t.table.get(id) {
                return Ok(Resolved {
                    text: e.text().into_owned(),
                    bytes: e.bytes.clone(),
                    utf8: e.is_utf8(),
                    origin: t.origin.clone(),
                    kind: t.kind.extension(),
                    offset: e.offset,
                    length: e.stored_len(t.kind),
                });
            }
        }
        Err(format!(
            "string ID 0x{id:08X} of {plugin} is in none of its {} tables ({})",
            self.language,
            tables
                .iter()
                .map(|t| t.origin.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::testdata;
    use kitkraft_strings::Entry;

    #[test]
    fn loose_tables_resolve_and_missing_ones_say_where_they_looked() {
        let dir = testdata::temp_dir("strings");
        std::fs::create_dir_all(dir.join("Strings")).unwrap();
        let table = Table::new(
            Kind::Strings,
            vec![Entry::new(0x10, "Old Earth Hunting Rifle")],
        );
        std::fs::write(
            dir.join("Strings").join("mod_en.STRINGS"),
            table.to_bytes().unwrap(),
        )
        .unwrap();
        let mut st = StringTables::new(
            "EN",
            Box::new(DirSource {
                data_dir: dir.clone(),
            }),
        );
        let r = st.lookup("Mod.esm", 0x10).unwrap();
        assert_eq!(r.text, "Old Earth Hunting Rifle");
        assert_eq!(r.kind, "strings");
        assert_eq!(r.origin, "Strings\\mod_en.STRINGS");
        assert!(r.offset.is_some());
        let miss = st.lookup("Mod.esm", 0x11).unwrap_err();
        assert!(miss.contains("0x00000011"), "{miss}");
        let none = st.lookup("Other.esm", 1).unwrap_err();
        assert!(none.contains("no en string tables for Other.esm"), "{none}");
        // A broken loose table is an error naming the file, not a panic.
        std::fs::write(dir.join("Strings").join("bad_en.strings"), b"\x05\0\0\0").unwrap();
        let bad = st.lookup("Bad.esm", 1).unwrap_err();
        assert!(bad.contains("bad_en.strings"), "{bad}");
    }

    /// M3.D1 (lane E round 2 request): a cp1252-style entry keeps its exact bytes next to
    /// the lossy text, so a copy can write the table's bytes unchanged.
    #[test]
    fn non_utf8_entries_expose_their_exact_bytes() {
        let dir = testdata::temp_dir("strings-bytes");
        std::fs::create_dir_all(dir.join("Strings")).unwrap();
        let cp1252 = b"Caf\xE9 \x93quoted\x94".to_vec();
        let table = Table::new(
            Kind::DlStrings,
            vec![Entry::new(7, cp1252.clone()), Entry::new(8, "plain")],
        );
        std::fs::write(
            dir.join("Strings").join("mod_en.dlstrings"),
            table.to_bytes().unwrap(),
        )
        .unwrap();
        let mut st = StringTables::new("en", Box::new(DirSource { data_dir: dir }));
        let r = st.lookup("Mod.esm", 7).unwrap();
        assert_eq!(r.bytes, cp1252);
        assert!(!r.utf8);
        assert!(r.text.contains('\u{FFFD}'));
        assert_eq!(r.length, cp1252.len() as u64 + 5);
        let plain = st.lookup("Mod.esm", 8).unwrap();
        assert_eq!((plain.bytes.as_slice(), plain.utf8), (&b"plain"[..], true));
    }

    #[test]
    fn archives_are_ordered_localization_first() {
        let mut v = vec![
            PathBuf::from("Starfield - Textures01.ba2"),
            PathBuf::from("Starfield - Main.ba2"),
            PathBuf::from("Starfield - Localization.ba2"),
        ];
        sort_archives(&mut v);
        assert_eq!(v[0], PathBuf::from("Starfield - Localization.ba2"));
        assert_eq!(v[1], PathBuf::from("Starfield - Main.ba2"));
    }
}
