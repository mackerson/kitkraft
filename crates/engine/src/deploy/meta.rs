//! MO2's `meta.ini` in a dev mod (ADR 0009 Amendment 3): the version, category and comment
//! MO2 shows in its mod list.
//!
//! A dev mod without one gets a minimal file in MO2's own layout (observed in the owner's
//! instance, MO2 2.5.2, 2026-10-18: `[General]`, `key=value` lines, CRLF, `category="4,"`).
//! A dev mod that has one (MO2 writes keys of its own once it lists the mod) keeps every
//! line except `version=`, `comments=` and, when a category is given, `category=`, which
//! are replaced in place, or added right after `[General]` when missing.

use std::fs;
use std::path::Path;

use super::{refused, DeployError};

/// MO2's category list, beside `ModOrganizer.exe`: `id|name|nexus ids|parent` lines.
pub(crate) const CATEGORIES: &str = "categories.dat";

/// What a deployment says about itself in `meta.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetaFields {
    /// `version=`.
    pub version: String,
    /// `comments=`.
    pub comments: String,
    /// `category="<id>,"`; `None` leaves the key alone.
    pub category: Option<u32>,
}

/// The category id MO2 lists under `name` (ignoring case) in `<mo2>\categories.dat`.
pub(crate) fn category_id(mo2_root: &Path, name: &str) -> Result<u32, DeployError> {
    let path = mo2_root.join(CATEGORIES);
    let text = fs::read(&path).map_err(|e| {
        refused(
            "category",
            &path,
            format!("cannot be read ({e}), so category `{name}` cannot be looked up"),
        )
    })?;
    let text = String::from_utf8_lossy(&text);
    let mut known = Vec::new();
    for line in text.lines() {
        let mut parts = line.split('|');
        let (Some(id), Some(n)) = (parts.next(), parts.next()) else {
            continue;
        };
        if let Ok(id) = id.trim().parse::<u32>() {
            if n.trim().eq_ignore_ascii_case(name.trim()) {
                return Ok(id);
            }
            known.push(n.trim().to_string());
        }
    }
    Err(refused(
        "category",
        &path,
        format!("has no category `{name}`; MO2 knows: {}", known.join(", ")),
    ))
}

/// A value as MO2 writes it: plain when it holds no character an INI reader would split
/// on, else quoted with `\` and `"` escaped.
fn value(v: &str) -> String {
    if v.chars()
        .any(|c| matches!(c, ',' | ';' | '"' | '=' | '\\') || c.is_control())
    {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        v.to_string()
    }
}

fn lines_for(f: &MetaFields) -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("version", value(&f.version)),
        ("comments", value(&f.comments)),
    ];
    if let Some(c) = f.category {
        out.push(("category", format!("\"{c},\"")));
    }
    out
}

/// A new `meta.ini` for a dev mod MO2 has not described yet.
pub(crate) fn new_meta(f: &MetaFields) -> Vec<u8> {
    let mut lines = vec![
        "[General]".to_string(),
        "gameName=Starfield".to_string(),
        "modid=0".to_string(),
    ];
    lines.extend(lines_for(f).into_iter().map(|(k, v)| format!("{k}={v}")));
    lines.push("notes=".to_string());
    let mut out = lines.join("\r\n");
    out.push_str("\r\n");
    out.into_bytes()
}

/// `existing` with the fields' keys replaced in `[General]` (added after its header when
/// missing; a `[General]` section is added at the top when there is none). Every other
/// byte is kept, including the line ending style and a trailing newline.
pub(crate) fn update_meta(existing: &[u8], f: &MetaFields) -> Vec<u8> {
    let text = String::from_utf8_lossy(existing);
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let final_eol = text.ends_with(eol);
    let mut lines: Vec<String> = text.split(eol).map(str::to_string).collect();
    if final_eol {
        lines.pop();
    }
    let header = lines
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case("[General]"));
    let header = match header {
        Some(h) => h,
        None => {
            lines.insert(0, "[General]".to_string());
            0
        }
    };
    // The section runs to the next `[...]` line.
    let end = lines
        .iter()
        .enumerate()
        .skip(header.saturating_add(1))
        .find(|(_, l)| l.trim_start().starts_with('['))
        .map_or(lines.len(), |(i, _)| i);
    let mut insert_at = header.saturating_add(1);
    for (key, v) in lines_for(f) {
        let found = lines
            .iter()
            .enumerate()
            .take(end)
            .skip(header.saturating_add(1))
            .find(|(_, l)| {
                l.split_once('=')
                    .is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key))
            })
            .map(|(i, _)| i);
        let line = format!("{key}={v}");
        match found {
            Some(i) => {
                if let Some(slot) = lines.get_mut(i) {
                    *slot = line;
                }
            }
            None => {
                lines.insert(insert_at, line);
                insert_at = insert_at.saturating_add(1);
            }
        }
    }
    let mut out = lines.join(eol);
    if final_eol || existing.is_empty() {
        out.push_str(eol);
    }
    out.into_bytes()
}

/// `YYYY.M.D.HHMM` (UTC) from Unix seconds: the version a build without one gets.
pub(crate) fn date_version(unix: u64) -> String {
    let stamp = super::rfc3339(unix);
    // `2026-10-18T22:15:30Z`
    let num = |a: usize, b: usize| {
        stamp
            .get(a..b)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    };
    format!(
        "{}.{}.{}.{:02}{:02}",
        num(0, 4),
        num(5, 7),
        num(8, 10),
        num(11, 13),
        num(14, 16)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(category: Option<u32>) -> MetaFields {
        MetaFields {
            version: "1.0.0".into(),
            comments: "KitKraft dev build 20261019T005406Z-8427029f".into(),
            category,
        }
    }

    #[test]
    fn a_new_file_is_mo2_s_layout() {
        assert_eq!(
            String::from_utf8(new_meta(&fields(Some(4)))).unwrap(),
            "[General]\r\ngameName=Starfield\r\nmodid=0\r\nversion=1.0.0\r\ncomments=KitKraft dev build 20261019T005406Z-8427029f\r\ncategory=\"4,\"\r\nnotes=\r\n"
        );
    }

    #[test]
    fn an_update_replaces_only_our_keys_and_keeps_mo2_s() {
        let mo2 = b"[General]\r\ngameName=Starfield\r\nmodid=0\r\nversion=0.9\r\nnewestVersion=\r\ncategory=\"7,\"\r\ncomments=old\r\nnotes=mine\r\n[installedFiles]\r\nsize=0\r\n";
        let got = String::from_utf8(update_meta(mo2, &fields(Some(4)))).unwrap();
        assert_eq!(
            got,
            "[General]\r\ngameName=Starfield\r\nmodid=0\r\nversion=1.0.0\r\nnewestVersion=\r\ncategory=\"4,\"\r\ncomments=KitKraft dev build 20261019T005406Z-8427029f\r\nnotes=mine\r\n[installedFiles]\r\nsize=0\r\n"
        );
        // No category given: MO2's (or the owner's) choice stays.
        let kept = String::from_utf8(update_meta(mo2, &fields(None))).unwrap();
        assert!(kept.contains("category=\"7,\""));
    }

    #[test]
    fn missing_keys_and_sections_are_added() {
        let got = String::from_utf8(update_meta(
            b"[General]\ngameName=Starfield\n",
            &fields(None),
        ))
        .unwrap();
        assert_eq!(
            got,
            "[General]\nversion=1.0.0\ncomments=KitKraft dev build 20261019T005406Z-8427029f\ngameName=Starfield\n"
        );
        let got = String::from_utf8(update_meta(b"", &fields(None))).unwrap();
        assert!(got.starts_with("[General]\n"));
    }

    #[test]
    fn values_with_separators_are_quoted() {
        assert_eq!(value("a, b"), "\"a, b\"");
        assert_eq!(value("1.0.0"), "1.0.0");
    }

    #[test]
    fn categories_are_looked_up_by_name() {
        let dir = std::env::temp_dir().join(format!("kitkraft-cat-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(CATEGORIES),
            "1|Starfield|0\r\n4|Gameplay|0\r\n7|Ships|0\r\n",
        )
        .unwrap();
        assert_eq!(category_id(&dir, "gameplay").unwrap(), 4);
        assert_eq!(category_id(&dir, "Nope").unwrap_err().rule, "category");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn date_versions() {
        assert_eq!(date_version(1_792_368_446), "2026.10.19.0007");
    }
}
