//! Localized text for the lstrings of copied records (M2.E4).
//!
//! Patches are non-localized (M2), so every lstring a copied record carries as a string ID
//! is written as inline text: the text comes from the string tables of the plugin the
//! record was copied from, through lane D's load-order tables (M2.D7: the MO2 overlay or
//! Data dir's loose `Strings\`, then the plugin's archives) via [`TableStrings`].

use std::collections::BTreeMap;

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

/// The load order's string tables (lane D's M2.D7: the MO2 overlay, then the plugin's
/// archives), as a [`StringLookup`].
pub struct TableStrings<'t>(pub std::cell::RefCell<&'t mut kitcraft_doc::strings::StringTables>);

impl std::fmt::Debug for TableStrings<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TableStrings")
    }
}

impl StringLookup for TableStrings<'_> {
    fn text(&self, plugin: &str, id: u32) -> Result<Option<Resolved>, String> {
        let mut tables = self
            .0
            .try_borrow_mut()
            .map_err(|_| "string tables are busy".to_string())?;
        let found = tables.lookup(plugin, id)?;
        // Lane D's lookup gives lossy UTF-8 text; a few official strings are not UTF-8
        // (lane G), and copying the replacement character would change them. Refuse those
        // until the lookup gives the stored bytes.
        if found.text.contains('\u{FFFD}') {
            return Err(format!(
                "string 0x{id:08X} of {plugin} is not valid UTF-8, and the load order's string lookup gives only lossy text, so it cannot be copied byte for byte"
            ));
        }
        Ok(Some(Resolved {
            bytes: found.text.into_bytes(),
            table: found.origin,
            offset: found.offset,
        }))
    }
}
