//! Tokens and logical lines of a `.psc` source.
//!
//! Language facts (`ck-wiki:` Papyrus language reference, in our own words; `corpus:` every
//! file under `Scripts/Source` in `Tools\ContentResources.zip`, 2026-10-14):
//!
//! - A statement ends at the end of its line. A backslash as the last non-blank character
//!   continues the statement on the next line (53 official files use it).
//! - `;` starts a comment to the end of the line; `;/` ... `/;` is a comment that may span
//!   lines (28 files); `{` ... `}` is a documentation comment, possibly over several lines,
//!   written after the declaration it documents (984 files). Braces have no other use.
//! - Strings are double-quoted with backslash escapes and stay on one line.
//! - Names are case-insensitive; a namespaced name joins its parts with `:`
//!   (`Fragments:Quests:QF_Name`). Every official source is valid UTF-8.

use super::PscError;

/// One token. `start..end` are byte offsets in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

/// What a token is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// A name or keyword, `:` joined (`Ns:Script`), as written.
    Word(String),
    /// A number literal as written (`10`, `1.5`, `0x1F`).
    Number,
    /// A string literal (quotes included in the span).
    Str,
    /// Any other single character (`(`, `)`, `[`, `]`, `,`, `=`, `.`, `-`, ...).
    Punct(char),
}

impl Token {
    /// The word, lower-cased, when this is a word.
    pub fn word_lower(&self) -> Option<String> {
        match &self.kind {
            TokenKind::Word(w) => Some(w.to_ascii_lowercase()),
            _ => None,
        }
    }

    /// Whether this is the word `w` (ASCII case-insensitive).
    pub fn is_word(&self, w: &str) -> bool {
        matches!(&self.kind, TokenKind::Word(x) if x.eq_ignore_ascii_case(w))
    }

    /// Whether this is the punctuation `c`.
    pub fn is_punct(&self, c: char) -> bool {
        self.kind == TokenKind::Punct(c)
    }
}

/// A statement: the tokens between two unescaped line ends.
#[derive(Debug, Clone)]
pub(crate) struct Line {
    /// 1-based line of the first token.
    pub line: u32,
    pub tokens: Vec<Token>,
}

impl Line {
    /// Byte range from the first token's start to the last token's end.
    pub fn span(&self) -> (usize, usize) {
        let start = self.tokens.first().map_or(0, |t| t.start);
        let end = self.tokens.last().map_or(start, |t| t.end);
        (start, end)
    }
}

/// A comment, kept for `with_comments` output only.
#[derive(Debug, Clone)]
pub(crate) struct Comment {
    /// 1-based line where it starts.
    pub line: u32,
    /// 1-based line where it ends.
    pub end_line: u32,
    /// The text without its delimiters, trimmed.
    pub text: String,
    /// `{ }` documentation comment (else `;` or `;/ /;`).
    pub doc: bool,
    /// Nothing but blanks precedes it on its line.
    pub own_line: bool,
}

/// The lexed source.
#[derive(Debug, Default)]
pub(crate) struct Lexed {
    pub lines: Vec<Line>,
    pub comments: Vec<Comment>,
}

fn is_word_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == ':'
}

/// Splits `src` into logical lines of tokens and the comments between them.
pub(crate) fn lex(src: &str) -> Result<Lexed, PscError> {
    let mut out = Lexed::default();
    let mut chars = src.char_indices().peekable();
    let mut line: u32 = 1;
    let mut current: Vec<Token> = Vec::new();
    let mut current_line: u32 = 1;
    // Whether only blanks (or comments) precede the cursor on this physical line.
    let mut line_blank = true;
    let bump = |line: &mut u32| *line = line.saturating_add(1);

    while let Some((at, c)) = chars.next() {
        match c {
            '\n' => {
                if !current.is_empty() {
                    out.lines.push(Line {
                        line: current_line,
                        tokens: std::mem::take(&mut current),
                    });
                }
                bump(&mut line);
                line_blank = true;
            }
            ' ' | '\t' | '\r' | '\u{feff}' => {}
            ';' => {
                let start_line = line;
                let own_line = line_blank && current.is_empty();
                if chars.peek().map(|&(_, n)| n) == Some('/') {
                    chars.next();
                    let body_start = at.saturating_add(2);
                    let mut prev = '\0';
                    let mut end = None;
                    for (i, n) in chars.by_ref() {
                        if n == '\n' {
                            bump(&mut line);
                        }
                        if prev == '/' && n == ';' {
                            end = Some(i.saturating_sub(1));
                            break;
                        }
                        prev = n;
                    }
                    let Some(end) = end else {
                        return Err(PscError::UnterminatedComment { line: start_line });
                    };
                    out.comments.push(Comment {
                        line: start_line,
                        end_line: line,
                        text: src
                            .get(body_start..end)
                            .unwrap_or_default()
                            .trim()
                            .to_string(),
                        doc: false,
                        own_line,
                    });
                } else {
                    let mut end = src.len();
                    while let Some(&(i, n)) = chars.peek() {
                        if n == '\n' {
                            end = i;
                            break;
                        }
                        chars.next();
                    }
                    out.comments.push(Comment {
                        line: start_line,
                        end_line: start_line,
                        text: src
                            .get(at.saturating_add(1)..end)
                            .unwrap_or_default()
                            .trim()
                            .to_string(),
                        doc: false,
                        own_line,
                    });
                }
            }
            '{' => {
                let start_line = line;
                let own_line = line_blank && current.is_empty();
                let mut end = None;
                for (i, n) in chars.by_ref() {
                    if n == '\n' {
                        bump(&mut line);
                    }
                    if n == '}' {
                        end = Some(i);
                        break;
                    }
                }
                let Some(end) = end else {
                    return Err(PscError::UnterminatedDoc { line: start_line });
                };
                out.comments.push(Comment {
                    line: start_line,
                    end_line: line,
                    text: src
                        .get(at.saturating_add(1)..end)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    doc: true,
                    own_line,
                });
            }
            '\\' => {
                // A continuation when only blanks (or a comment) follow on this line.
                let rest = src.get(at.saturating_add(1)..).unwrap_or_default();
                let tail = rest.split('\n').next().unwrap_or_default();
                let tail = tail.split(';').next().unwrap_or_default();
                if tail.trim().is_empty() {
                    // Skip to (and over) the line end, keeping any comment text out.
                    while let Some(&(_, n)) = chars.peek() {
                        chars.next();
                        if n == '\n' {
                            bump(&mut line);
                            break;
                        }
                    }
                } else {
                    push(
                        &mut current,
                        &mut current_line,
                        line,
                        at,
                        TokenKind::Punct('\\'),
                        1,
                    );
                    line_blank = false;
                }
            }
            '"' => {
                let mut end = None;
                let mut escaped = false;
                while let Some(&(i, n)) = chars.peek() {
                    if n == '\n' {
                        break;
                    }
                    chars.next();
                    if escaped {
                        escaped = false;
                    } else if n == '\\' {
                        escaped = true;
                    } else if n == '"' {
                        end = Some(i);
                        break;
                    }
                }
                let Some(end) = end else {
                    return Err(PscError::UnterminatedString { line });
                };
                push_range(
                    &mut current,
                    &mut current_line,
                    line,
                    at,
                    end.saturating_add(1),
                    TokenKind::Str,
                );
                line_blank = false;
            }
            c if is_word_start(c) => {
                let mut end = at.saturating_add(c.len_utf8());
                while let Some(&(i, n)) = chars.peek() {
                    if !is_word_char(n) {
                        break;
                    }
                    end = i.saturating_add(n.len_utf8());
                    chars.next();
                }
                let word = src.get(at..end).unwrap_or_default().to_string();
                push_range(
                    &mut current,
                    &mut current_line,
                    line,
                    at,
                    end,
                    TokenKind::Word(word),
                );
                line_blank = false;
            }
            c if c.is_ascii_digit() => {
                let mut end = at.saturating_add(1);
                while let Some(&(i, n)) = chars.peek() {
                    if !(n.is_ascii_alphanumeric() || n == '.' || n == '_') {
                        break;
                    }
                    end = i.saturating_add(1);
                    chars.next();
                }
                push_range(
                    &mut current,
                    &mut current_line,
                    line,
                    at,
                    end,
                    TokenKind::Number,
                );
                line_blank = false;
            }
            c => {
                push(
                    &mut current,
                    &mut current_line,
                    line,
                    at,
                    TokenKind::Punct(c),
                    c.len_utf8(),
                );
                line_blank = false;
            }
        }
    }
    if !current.is_empty() {
        out.lines.push(Line {
            line: current_line,
            tokens: current,
        });
    }
    Ok(out)
}

fn push(
    current: &mut Vec<Token>,
    current_line: &mut u32,
    line: u32,
    at: usize,
    kind: TokenKind,
    len: usize,
) {
    push_range(
        current,
        current_line,
        line,
        at,
        at.saturating_add(len),
        kind,
    );
}

fn push_range(
    current: &mut Vec<Token>,
    current_line: &mut u32,
    line: u32,
    start: usize,
    end: usize,
    kind: TokenKind,
) {
    if current.is_empty() {
        *current_line = line;
    }
    current.push(Token { kind, start, end });
}
