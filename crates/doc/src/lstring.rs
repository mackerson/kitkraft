//! Localized strings (M2.D7, partial).
//!
//! Hypothesis from the plan's format facts (`uesp-skyrim`, to be confirmed by M2.B2 for the
//! localized half and M2.E3/M2.O6 for the inline half): in a plugin with the localized flag,
//! `FULL` and `DESC` hold a `u32` string ID whose text lives in
//! `Strings\<plugin>_<language>.{STRINGS,DLSTRINGS,ILSTRINGS}`; otherwise they hold a
//! NUL-terminated string inline.
//!
//! String tables are not read yet: that needs the BA2 reader and the string-table reader of
//! lane G (M2.G1, M2.G8). Until then a localized lstring resolves to its ID and callers
//! report the missing table as a limitation, never as an absent value.

use crate::index::zstring;

/// Signatures treated as lstrings.
pub const LSTRING_SIGNATURES: [&str; 2] = ["FULL", "DESC"];

/// Whether `signature` is treated as an lstring.
pub fn is_lstring(signature: &str) -> bool {
    LSTRING_SIGNATURES.contains(&signature)
}

/// A decoded lstring payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LString {
    /// Inline text (non-localized plugin).
    Text(String),
    /// String ID (localized plugin); text needs the plugin's string tables.
    Id(u32),
    /// A localized payload that is not 4 bytes long.
    Malformed {
        /// Payload length.
        len: usize,
    },
}

/// Decode an lstring payload.
pub fn decode_lstring(data: &[u8], localized: bool) -> LString {
    if !localized {
        return LString::Text(zstring(data));
    }
    match <[u8; 4]>::try_from(data) {
        Ok(b) => LString::Id(u32::from_le_bytes(b)),
        Err(_) => LString::Malformed { len: data.len() },
    }
}

/// The limitation stated for an unresolved string ID.
pub fn unresolved(signature: &str, plugin: &str, id: u32) -> String {
    format!(
        "{signature} in {plugin} is localized string ID 0x{id:08X}; string tables are not loaded yet (M2.D7 waits for the BA2 and string-table readers, M2.G1/M2.G8)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode() {
        assert_eq!(
            decode_lstring(b"Rifle\0", false),
            LString::Text("Rifle".into())
        );
        assert_eq!(decode_lstring(&[1, 2, 0, 0], true), LString::Id(0x201));
        assert_eq!(
            decode_lstring(b"Rifle\0", true),
            LString::Malformed { len: 6 }
        );
        assert!(is_lstring("FULL") && !is_lstring("EDID"));
    }
}
