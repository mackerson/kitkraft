//! Reading the small text files MO2 and the game keep: one entry per line.

use std::path::Path;

use crate::error::{IoError, Result};

/// Lines of a text file: a UTF-8 BOM stripped, `\r\n` or `\n` endings, invalid UTF-8
/// replaced, trailing whitespace trimmed. `Ok(None)` when the file does not exist.
pub fn read_lines(path: &Path) -> Result<Option<Vec<String>>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(IoError::io(path, e)),
    };
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let text = String::from_utf8_lossy(bytes);
    Ok(Some(
        text.lines().map(|l| l.trim_end().to_string()).collect(),
    ))
}

/// Lines that are neither empty nor `#` comments, with their 1-based line numbers.
pub fn entries(lines: &[String]) -> Vec<(usize, &str)> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|(i, l)| (i.saturating_add(1), l.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn bom_crlf_and_comments() {
        let dir = std::env::temp_dir().join(format!("kitkraft-io-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.txt");
        std::fs::write(&path, b"\xEF\xBB\xBF# c\r\n*A.esm  \r\n\r\nB.esm\xFF\n").unwrap();
        let lines = read_lines(&path).unwrap().unwrap();
        assert_eq!(lines, ["# c", "*A.esm", "", "B.esm\u{FFFD}"]);
        assert_eq!(entries(&lines), [(2, "*A.esm"), (4, "B.esm\u{FFFD}")]);
        assert_eq!(read_lines(&dir.join("none.txt")).unwrap(), None);
    }
}
