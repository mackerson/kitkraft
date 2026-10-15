//! Plain Markdown for the Help tab: headings, paragraphs, list items, pipe tables and
//! code blocks, with bold, struck-out and code spans inside them. Enough for the repo's
//! own docs, nothing more: no links followed, no HTML, no nesting beyond list indent.
//! Toolkit-free so the parse is tested without a window; the window draws the blocks.

/// A run of text inside a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    /// Plain text.
    Text(String),
    /// `**bold**`.
    Strong(String),
    /// `~~struck out~~`.
    Struck(String),
    /// `` `code` ``: shown monospace and copyable.
    Code(String),
}

/// One block of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// `#`..`######`.
    Heading {
        /// 1..=6.
        level: u8,
        /// Its text.
        text: Vec<Inline>,
    },
    /// Lines of text up to a blank line or another block.
    Para(Vec<Inline>),
    /// `- item`, `* item` or `1. item`, with its continuation lines.
    Item {
        /// Indent level (two spaces each).
        depth: u8,
        /// `-` or `1.`.
        marker: String,
        /// Its text.
        text: Vec<Inline>,
    },
    /// A pipe table: header cells, then rows of cells.
    Table {
        /// The header row.
        header: Vec<Vec<Inline>>,
        /// The body rows.
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// A fenced code block, line by line.
    Code(Vec<String>),
}

/// Most lines one document is parsed from (the docs are far shorter).
const MAX_LINES: usize = 20_000;

/// The heading level and text of `line`, when it is a heading.
fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = line.get(hashes..)?.strip_prefix(' ')?;
    Some((u8::try_from(hashes).ok()?, rest.trim()))
}

/// The part of `text` under the first heading whose text starts with `title`: that
/// heading and everything up to the next heading of the same or a higher level.
pub fn section(text: &str, title: &str) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    let mut level = None;
    let mut fenced = false;
    for line in text.lines().take(MAX_LINES) {
        let fence = line.trim_start().starts_with("```");
        let h = if fenced { None } else { heading(line) };
        match level {
            None => {
                if let Some((l, t)) = h {
                    if t.starts_with(title) {
                        level = Some(l);
                        out.push(line);
                    }
                }
            }
            Some(at) => {
                if h.is_some_and(|(l, _)| l <= at) {
                    break;
                }
                out.push(line);
            }
        }
        if fence {
            fenced = !fenced;
        }
    }
    level.map(|_| out.join("\n"))
}

/// The inline runs of one line of text.
pub fn inlines(s: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut rest = s;
    let flush = |text: &mut String, out: &mut Vec<Inline>| {
        if !text.is_empty() {
            out.push(Inline::Text(std::mem::take(text)));
        }
    };
    while let Some(c) = rest.chars().next() {
        let after = rest.get(c.len_utf8()..).unwrap_or_default();
        if c == '`' {
            if let Some(end) = after.find('`') {
                flush(&mut text, &mut out);
                out.push(Inline::Code(
                    after.get(..end).unwrap_or_default().to_string(),
                ));
                rest = after.get(end.saturating_add(1)..).unwrap_or_default();
                continue;
            }
        }
        let mut matched = false;
        for (mark, make) in [
            ("**", Inline::Strong as fn(String) -> Inline),
            ("~~", Inline::Struck as fn(String) -> Inline),
        ] {
            if let Some(inner) = rest.strip_prefix(mark) {
                if let Some(end) = inner.find(mark) {
                    flush(&mut text, &mut out);
                    let body = inner.get(..end).unwrap_or_default().replace('`', "");
                    out.push(make(body));
                    rest = inner
                        .get(end.saturating_add(mark.len())..)
                        .unwrap_or_default();
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }
        // `[text](target)` shows its text.
        if c == '[' {
            if let Some(close) = after.find("](") {
                let tail = after.get(close.saturating_add(2)..).unwrap_or_default();
                if let Some(end) = tail.find(')') {
                    text.push_str(after.get(..close).unwrap_or_default());
                    rest = tail.get(end.saturating_add(1)..).unwrap_or_default();
                    continue;
                }
            }
        }
        if c == '\\' && after.starts_with(['|', '*', '`', '_', '[', ']']) {
            if let Some(next) = after.chars().next() {
                text.push(next);
                rest = after.get(next.len_utf8()..).unwrap_or_default();
                continue;
            }
        }
        text.push(c);
        rest = after;
    }
    flush(&mut text, &mut out);
    out
}

/// The cells of a table line: split on `|` outside code spans and not escaped.
fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut code = false;
    let mut prev = '\0';
    for c in t.chars() {
        match c {
            '`' => {
                code = !code;
                cur.push(c);
            }
            '|' if !code && prev != '\\' => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
        prev = c;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn is_separator(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|c| {
            !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')) && c.contains('-')
        })
}

/// A list item's depth, marker and text.
fn item(line: &str) -> Option<(u8, String, &str)> {
    let indent = line.len().saturating_sub(line.trim_start().len());
    let t = line.trim_start();
    let depth = u8::try_from(indent.checked_div(2).unwrap_or(0).min(6)).unwrap_or(6);
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        return Some((depth, "-".to_string(), rest));
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if (1..=3).contains(&digits) {
        let rest = t.get(digits..)?.strip_prefix(". ")?;
        return Some((depth, format!("{}.", t.get(..digits)?), rest));
    }
    None
}

/// The blocks of `text`.
pub fn parse(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().take(MAX_LINES).collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let line_at = |i: usize| lines.get(i).copied();
    while let Some(line) = line_at(i) {
        let next = i.saturating_add(1);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            i = next;
            continue;
        }
        if trimmed.starts_with("```") {
            let mut code = Vec::new();
            i = next;
            while let Some(l) = line_at(i) {
                i = i.saturating_add(1);
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push(l.to_string());
            }
            out.push(Block::Code(code));
            continue;
        }
        if let Some((level, t)) = heading(line) {
            out.push(Block::Heading {
                level,
                text: inlines(t),
            });
            i = next;
            continue;
        }
        if trimmed.starts_with('|') {
            let mut rows: Vec<Vec<String>> = Vec::new();
            while let Some(l) = line_at(i).filter(|l| l.trim().starts_with('|')) {
                rows.push(cells(l));
                i = i.saturating_add(1);
            }
            let mut rows = rows.into_iter();
            let header = rows.next().unwrap_or_default();
            let body: Vec<Vec<String>> = rows.filter(|r| !is_separator(r)).collect();
            out.push(Block::Table {
                header: header.iter().map(|c| inlines(c)).collect(),
                rows: body
                    .iter()
                    .map(|r| r.iter().map(|c| inlines(c)).collect())
                    .collect(),
            });
            continue;
        }
        if let Some((depth, marker, t)) = item(line) {
            let mut text = t.trim().to_string();
            i = next;
            // Indented lines that start no block of their own continue the item.
            while let Some(l) = line_at(i) {
                let lt = l.trim();
                if lt.is_empty()
                    || !l.starts_with(' ')
                    || item(l).is_some()
                    || lt.starts_with('|')
                    || lt.starts_with("```")
                {
                    break;
                }
                text.push(' ');
                text.push_str(lt);
                i = i.saturating_add(1);
            }
            out.push(Block::Item {
                depth,
                marker,
                text: inlines(&text),
            });
            continue;
        }
        let mut text = trimmed.to_string();
        i = next;
        while let Some(l) = line_at(i) {
            let lt = l.trim();
            if lt.is_empty()
                || heading(l).is_some()
                || item(l).is_some()
                || lt.starts_with('|')
                || lt.starts_with("```")
            {
                break;
            }
            text.push(' ');
            text.push_str(lt);
            i = i.saturating_add(1);
        }
        out.push(Block::Para(inlines(&text)));
    }
    out
}

/// The text of `runs` without markup (for searching a table).
pub fn plain(runs: &[Inline]) -> String {
    runs.iter()
        .map(|r| match r {
            Inline::Text(t) | Inline::Strong(t) | Inline::Struck(t) | Inline::Code(t) => t.as_str(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_runs() {
        assert_eq!(
            inlines("Run `player.additem 0026D96C 1` now, **Expect:** ~~old~~ [docs](x.md) a\\|b"),
            vec![
                Inline::Text("Run ".into()),
                Inline::Code("player.additem 0026D96C 1".into()),
                Inline::Text(" now, ".into()),
                Inline::Strong("Expect:".into()),
                Inline::Text(" ".into()),
                Inline::Struck("old".into()),
                Inline::Text(" docs a|b".into()),
            ]
        );
        // An unclosed mark is text.
        assert_eq!(
            inlines("a ` b ** c"),
            vec![Inline::Text("a ` b ** c".into())]
        );
    }

    #[test]
    fn sections_stop_at_the_next_heading_of_their_level_and_skip_fenced_hashes() {
        let doc = "# T\n\n## A\n\nx\n\n```\n# not a heading\n```\n\n### A.1\n\ny\n\n## B\n\nz\n";
        let a = section(doc, "A").unwrap();
        assert!(a.starts_with("## A") && a.contains("### A.1") && a.contains("# not a heading"));
        assert!(!a.contains("## B"));
        assert_eq!(section(doc, "B").unwrap(), "## B\n\nz");
        assert!(section(doc, "C").is_none());
    }

    #[test]
    fn blocks_of_a_doc() {
        let doc = "## In game\n\nSteps for the owner,\non two lines.\n\n1. Switch.\n   - `kit deploy --dev x`\n     (writes only mods)\n2. Add:\n\n   | Weapon | Console |\n   | :--- | :--- |\n   | Old Earth Pistol | `player.additem 0026D96C 1` |\n   | Pipe | `a|b` and c\\|d |\n\n```text\ncoc X\nprid Y\n```\n";
        let blocks = parse(doc);
        assert_eq!(
            blocks[0],
            Block::Heading {
                level: 2,
                text: vec![Inline::Text("In game".into())]
            }
        );
        assert_eq!(
            blocks[1],
            Block::Para(vec![Inline::Text(
                "Steps for the owner, on two lines.".into()
            )])
        );
        assert!(matches!(&blocks[2], Block::Item { depth: 0, marker, .. } if marker == "1."));
        match &blocks[3] {
            Block::Item { depth, text, .. } => {
                assert_eq!(*depth, 1);
                assert_eq!(text[0], Inline::Code("kit deploy --dev x".into()));
                assert_eq!(plain(text), "kit deploy --dev x (writes only mods)");
            }
            other => panic!("{other:?}"),
        }
        match &blocks[5] {
            Block::Table { header, rows } => {
                assert_eq!(header.len(), 2);
                assert_eq!(rows.len(), 2, "the separator row is dropped");
                assert_eq!(
                    rows[0][1],
                    vec![Inline::Code("player.additem 0026D96C 1".into())]
                );
                assert_eq!(plain(&rows[1][1]), "a|b and c|d");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            blocks[6],
            Block::Code(vec!["coc X".into(), "prid Y".into()])
        );
        assert_eq!(blocks.len(), 7);
    }
}
