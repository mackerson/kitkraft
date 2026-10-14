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
    /// Parse INI text. Lines outside a section go to section `""`.
    pub fn parse(text: &str) -> Ini {
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
}
