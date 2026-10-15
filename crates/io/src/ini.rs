//! The few `ModOrganizer.ini` keys we read.
//!
//! Observed in the owner's instance (`docs/load-order.md`, "ModOrganizer.ini"): Qt-style
//! INI with `[General]`, `gamePath=@ByteArray(C:\\Program Files (x86)\\...)` (backslashes
//! doubled inside `@ByteArray(...)`), `selected_profile=@ByteArray(Default)`.

use std::collections::BTreeMap;

/// `section -> key -> value`, keys as written, values raw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ini {
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl Ini {
    /// Parse INI text. Lines outside a section go to section `""`. A leading UTF-8 byte
    /// order mark is dropped (an editor that saves `ModOrganizer.ini` with one would
    /// otherwise hide `[General]`, and with it `selected_profile`).
    pub fn parse(text: &str) -> Ini {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut ini = Ini::default();
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.to_string();
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                ini.sections
                    .entry(section.clone())
                    .or_default()
                    .insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        ini
    }

    /// A value, decoded (see [`decode_value`]).
    pub fn get(&self, section: &str, key: &str) -> Option<String> {
        self.sections
            .get(section)?
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| decode_value(v))
    }

    /// Every key of `section` (its name compared ignoring ASCII case) with its decoded
    /// value, in key order. MO2's `[customExecutables]` lists `1\binary=...`, `2\binary=...`.
    pub fn entries(&self, section: &str) -> Vec<(String, String)> {
        self.sections
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case(section))
            .flat_map(|(_, keys)| keys.iter())
            .map(|(k, v)| (k.clone(), decode_value(v)))
            .collect()
    }
}

/// `@ByteArray(x)` -> `x` with `\\` -> `\`; a value in double quotes loses them.
pub fn decode_value(raw: &str) -> String {
    if let Some(inner) = raw
        .strip_prefix("@ByteArray(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return inner.replace("\\\\", "\\");
    }
    raw.strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .map_or_else(|| raw.to_string(), |r| r.replace("\\\\", "\\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bytearray_paths() {
        let ini = Ini::parse(
            "[General]\r\ngameName=Starfield\r\ngamePath=@ByteArray(C:\\\\Program Files (x86)\\\\Steam\\\\steamapps\\\\common\\\\Starfield)\r\n[Settings]\r\nmod_directory=\"D:\\\\mods\"\r\n",
        );
        assert_eq!(
            ini.get("General", "gamepath").as_deref(),
            Some("C:\\Program Files (x86)\\Steam\\steamapps\\common\\Starfield")
        );
        assert_eq!(
            ini.get("Settings", "mod_directory").as_deref(),
            Some("D:\\mods")
        );
        assert_eq!(ini.get("General", "missing"), None);
    }

    #[test]
    fn entries_list_a_section_decoded() {
        let ini = Ini::parse(
            "[customExecutables]\r\n1\\binary=C:/Tools/xEdit/xSFEdit64.exe\r\n1\\title=xEdit\r\n2\\binary=@ByteArray(D:\\\\x\\\\y.exe)\r\n[General]\r\na=b\r\n",
        );
        let e = ini.entries("customexecutables");
        let pair = |k: &str, v: &str| (k.to_string(), v.to_string());
        assert_eq!(
            e,
            [
                pair("1\\binary", "C:/Tools/xEdit/xSFEdit64.exe"),
                pair("1\\title", "xEdit"),
                pair("2\\binary", "D:\\x\\y.exe"),
            ]
        );
        assert!(ini.entries("Missing").is_empty());
    }

    /// Fable review (deploy round): a UTF-8 BOM before `[General]` hid the section, so
    /// `selected_profile` read as absent and the `Default` gate failed open.
    #[test]
    fn a_utf8_bom_does_not_hide_the_first_section() {
        let bytes = b"\xef\xbb\xbf[General]\r\nselected_profile=@ByteArray(Default)\r\n";
        let ini = Ini::parse(&String::from_utf8_lossy(bytes));
        assert_eq!(
            ini.get("General", "selected_profile").as_deref(),
            Some("Default")
        );
    }
}
