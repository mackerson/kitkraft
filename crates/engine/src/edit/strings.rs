//! Localized text for the lstrings of copied records (M2.E4).
//!
//! Patches are non-localized (M2), so every lstring a copied record carries as a string ID
//! is written as inline text: the text comes from the string tables of the plugin the
//! record was copied from. [`PluginStrings`] finds those tables the way the official files
//! ship them (`docs/corpus.md` "Archives", lane G): loose
//! `<plugin dir>\Strings\<stem>_<language>.<ext>` first, else `strings/<stem>_<language>.<ext>`
//! inside an archive next to the plugin whose name starts with `<stem> - ` (`Starfield -
//! Localization.ba2`, `SFBGS003 - Main.ba2`). The three kinds (`.strings`, `.dlstrings`,
//! `.ilstrings`) are all searched; their ID sets are disjoint in `starfield_en` (lane G).
//!
//! This is the minimal lookup `set` and `build` need; the load-order-wide one (MO2
//! overlay, every archive a profile enables) is M2.D7's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use kitcraft_strings::{table_name, Kind, Table};

/// Text found for a string ID, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The string's bytes (no terminator), copied as stored.
    pub bytes: Vec<u8>,
    /// Where the table was read: `<archive>/<path in archive>` or a loose file path.
    pub table: String,
    /// Offset of the string in that table, when known.
    pub offset: Option<u64>,
}

/// Looks up the text of string IDs of localized plugins.
pub trait StringLookup {
    /// The text of string `id` of the plugin whose file name is `plugin`. `Ok(None)` when
    /// the plugin's tables are loaded but none has `id`; `Err` when no table could be read
    /// (the message says what was looked for).
    fn text(&self, plugin: &str, id: u32) -> Result<Option<Resolved>, String>;
}

/// A lookup that knows no strings (every lookup is an error).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoStrings;

impl StringLookup for NoStrings {
    fn text(&self, plugin: &str, id: u32) -> Result<Option<Resolved>, String> {
        Err(format!(
            "no string tables are available for {plugin} (string 0x{id:08X})"
        ))
    }
}

/// An in-memory lookup: `(plugin file name, id) -> text` (tests and callers that hold
/// the tables already).
#[derive(Debug, Clone, Default)]
pub struct MemoryStrings(pub BTreeMap<(String, u32), String>);

impl StringLookup for MemoryStrings {
    fn text(&self, plugin: &str, id: u32) -> Result<Option<Resolved>, String> {
        Ok(self
            .0
            .iter()
            .find(|((p, i), _)| *i == id && p.eq_ignore_ascii_case(plugin))
            .map(|(_, text)| Resolved {
                bytes: text.as_bytes().to_vec(),
                table: "memory".into(),
                offset: None,
            }))
    }
}

/// The tables of one plugin.
#[derive(Debug, Default)]
struct Loaded {
    tables: Vec<(String, Table)>,
    /// What was looked for, when nothing was found.
    searched: Vec<String>,
}

/// String tables of plugins on disk, loaded on first use per plugin (module docs).
#[derive(Debug)]
pub struct PluginStrings {
    language: String,
    /// Lower-case plugin file name -> its path.
    paths: BTreeMap<String, PathBuf>,
    loaded: Mutex<BTreeMap<String, Loaded>>,
}

impl PluginStrings {
    /// Tables in `language` (`en`) for the plugins at `paths`.
    pub fn new<'p>(language: &str, paths: impl IntoIterator<Item = &'p Path>) -> PluginStrings {
        let paths = paths
            .into_iter()
            .filter_map(|p| {
                let name = p.file_name()?.to_string_lossy().to_ascii_lowercase();
                Some((name, p.to_path_buf()))
            })
            .collect();
        PluginStrings {
            language: language.to_string(),
            paths,
            loaded: Mutex::new(BTreeMap::new()),
        }
    }

    fn load(&self, plugin: &Path) -> Loaded {
        let mut loaded = Loaded::default();
        let name = plugin
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dir = plugin.parent().unwrap_or_else(|| Path::new("."));
        let stem = Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let archives: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|entries| {
                let mut found: Vec<PathBuf> = entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| {
                        let file = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_ascii_lowercase())
                            .unwrap_or_default();
                        file.ends_with(".ba2") && file.starts_with(&format!("{stem} - "))
                    })
                    .collect();
                found.sort();
                found
            })
            .unwrap_or_default();
        for kind in Kind::ALL {
            let table = table_name(&name, &self.language, kind);
            let loose = dir.join("Strings").join(&table);
            if loose.is_file() {
                match Table::read_file(&loose) {
                    Ok(t) => {
                        loaded.tables.push((loose.display().to_string(), t));
                        continue;
                    }
                    Err(e) => loaded.searched.push(format!("{}: {e}", loose.display())),
                }
            } else {
                loaded.searched.push(loose.display().to_string());
            }
            let inner = format!("strings/{table}");
            for archive in &archives {
                let Ok((ba2, mut file)) = kitcraft_ba2::Archive::open(archive) else {
                    loaded
                        .searched
                        .push(format!("{} (unreadable)", archive.display()));
                    continue;
                };
                let Some(i) = ba2.find(&inner) else {
                    loaded
                        .searched
                        .push(format!("{}/{inner}", archive.display()));
                    continue;
                };
                let label = format!(
                    "{}/{inner}",
                    archive
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                );
                match ba2
                    .extract_vec(&mut file, i)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| Table::parse(kind, &bytes).map_err(|e| e.to_string()))
                {
                    Ok(t) => {
                        loaded.tables.push((label, t));
                        break;
                    }
                    Err(e) => loaded.searched.push(format!("{label}: {e}")),
                }
            }
        }
        loaded
    }
}

impl StringLookup for PluginStrings {
    fn text(&self, plugin: &str, id: u32) -> Result<Option<Resolved>, String> {
        let key = plugin.to_ascii_lowercase();
        let path = self
            .paths
            .get(&key)
            .ok_or_else(|| format!("{plugin} is not a plugin this command read"))?;
        let mut cache = self.loaded.lock().unwrap_or_else(PoisonError::into_inner);
        let loaded = cache.entry(key).or_insert_with(|| self.load(path));
        if loaded.tables.is_empty() {
            return Err(format!(
                "no {} string tables found for {plugin} (looked at: {})",
                self.language,
                loaded.searched.join("; ")
            ));
        }
        Ok(loaded.tables.iter().find_map(|(label, table)| {
            table.get(id).map(|entry| Resolved {
                bytes: entry.bytes.clone(),
                table: label.clone(),
                offset: entry.offset,
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loose_tables_next_to_the_plugin() {
        let dir = std::env::temp_dir().join(format!("kitcraft-e4-strings-{}", std::process::id()));
        let strings = dir.join("Strings");
        std::fs::create_dir_all(&strings).unwrap();
        let table = Table::new(
            Kind::Strings,
            vec![kitcraft_strings::Entry::new(7, "Rifle")],
        );
        std::fs::write(strings.join("my_en.strings"), table.to_bytes().unwrap()).unwrap();
        let plugin = dir.join("My.esm");
        let lookup = PluginStrings::new("en", [plugin.as_path()]);
        let got = lookup.text("my.ESM", 7).unwrap().unwrap();
        assert_eq!(got.bytes, b"Rifle");
        assert!(got.table.ends_with("my_en.strings"), "{}", got.table);
        assert_eq!(lookup.text("My.esm", 8).unwrap(), None);
        assert!(lookup.text("Other.esm", 7).is_err());
        let none = PluginStrings::new("de", [plugin.as_path()]);
        let err = none.text("My.esm", 7).unwrap_err();
        assert!(err.contains("my_de.strings"), "{err}");
        assert!(NoStrings.text("My.esm", 7).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
