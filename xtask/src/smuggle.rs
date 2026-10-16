//! Source-level checks for code that reaches outside its own package.
//!
//! A crate can obey the dependency rules in `Cargo.toml` and still compile another crate's
//! files into itself with `#[path = "../../other/src/x.rs"] mod x;` or `include!`. This scan
//! reads every `.rs` file under `src/`, `tests/`, `benches/` and `examples/` of a package and
//! fails on any `#[path = ...]` (also inside `cfg_attr`), `include!`, `include_str!` or
//! `include_bytes!` whose path resolves outside the package directory, or whose path it
//! cannot resolve. Accepted argument forms: a string literal (relative to the file's
//! directory, as rustc resolves it) or `concat!(env!("CARGO_MANIFEST_DIR"), "...")`.
//!
//! It is a lexical scan: comments and string contents are skipped, paths are normalized
//! without touching the filesystem.
//!
//! The same scan holds `unsafe` to the files in [`UNSAFE_ALLOWED`]: the workspace denies
//! the `unsafe_code` lint (`Cargo.toml`, `[workspace.lints.rust]`), and a file may only
//! allow it, or use the `unsafe` keyword, if it is listed there (ADR 0013).

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Ident(String),
    Punct(char),
    Str(String),
    Other,
}

#[derive(Debug, PartialEq, Eq)]
enum IncludeArg {
    /// A string literal, relative to the including file's directory.
    Literal(String),
    /// `concat!(env!("CARGO_MANIFEST_DIR"), ...)`, relative to the package directory.
    ManifestRelative(String),
    /// Anything else.
    Unresolved,
}

#[derive(Debug, PartialEq, Eq)]
enum SourceRef {
    PathAttr(String),
    Include { name: String, arg: IncludeArg },
}

/// The only source files, as (package, path inside the package with `/`), that may use
/// `unsafe` or name the `unsafe_code` lint. Each entry is justified in an ADR.
pub const UNSAFE_ALLOWED: &[(&str, &str)] = &[
    // ADR 0013: the GL calls of the workbench's 3D viewport (glow has no safe API).
    ("kitkraft-ui-egui", "src/viewport/gl.rs"),
];

/// Scans every Rust source file of the package `name` in `dir`; returns one message per
/// violation.
pub fn scan_package(dir: &Path, name: &str) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for sub in ["src", "tests", "benches", "examples"] {
        collect_rs(&dir.join(sub), &mut files)?;
    }
    files.sort();
    let mut errors = Vec::new();
    for (pkg, rel) in UNSAFE_ALLOWED {
        if *pkg == name && !dir.join(rel).is_file() {
            errors.push(format!(
                "{}: xtask UNSAFE_ALLOWED names `{rel}`, which does not exist; remove the entry",
                dir.display()
            ));
        }
    }
    for file in files {
        let text =
            fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        errors.extend(scan_source(&file, dir, &text));
        let rel = file
            .strip_prefix(dir)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if !UNSAFE_ALLOWED.contains(&(name, rel.as_str())) {
            if let Some(word) = unsafe_word(&lex(&text)) {
                errors.push(format!(
                    "{}: uses `{word}`; only the files in xtask UNSAFE_ALLOWED may (ADR 0013)",
                    file.display()
                ));
            }
        }
    }
    Ok(errors)
}

/// The first `unsafe` keyword or `unsafe_code` lint name in the code (not in comments or
/// strings).
fn unsafe_word(tokens: &[Tok]) -> Option<&'static str> {
    tokens.iter().find_map(|t| match t {
        Tok::Ident(i) if i == "unsafe" => Some("unsafe"),
        Tok::Ident(i) if i == "unsafe_code" => Some("unsafe_code"),
        _ => None,
    })
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry.with_context(|| format!("reading {}", dir.display()))?;
        let kind = entry
            .file_type()
            .with_context(|| format!("reading {}", entry.path().display()))?;
        let path = entry.path();
        if kind.is_dir() {
            collect_rs(&path, out)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Checks one source file. `file` is its path, `pkg_dir` the package directory.
fn scan_source(file: &Path, pkg_dir: &Path, text: &str) -> Vec<String> {
    let base = file.parent().unwrap_or(pkg_dir);
    let mut errors = Vec::new();
    for found in find_refs(&lex(text)) {
        let (shown, target) = match &found {
            SourceRef::PathAttr(path) => (format!("#[path = {path:?}]"), Some(base.join(path))),
            SourceRef::Include { name, arg } => match arg {
                IncludeArg::Literal(path) => (format!("{name}!({path:?})"), Some(base.join(path))),
                IncludeArg::ManifestRelative(path) => (
                    format!("{name}!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {path:?}))"),
                    Some(pkg_dir.join(path.trim_start_matches(['/', '\\']))),
                ),
                IncludeArg::Unresolved => (format!("{name}!"), None),
            },
        };
        match target {
            None => errors.push(format!(
                "{}: `{shown}` with a path xtask cannot resolve; use a string literal or \
                 concat!(env!(\"CARGO_MANIFEST_DIR\"), \"...\")",
                file.display()
            )),
            Some(target) if !inside(pkg_dir, &target) => errors.push(format!(
                "{}: `{shown}` resolves to {}, outside the package directory {}",
                file.display(),
                normalize(&target).unwrap_or(target).display(),
                pkg_dir.display()
            )),
            Some(_) => {}
        }
    }
    errors
}

/// Lexical normalization: drops `.`, applies `..`. `None` if `..` climbs above the root.
fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

fn inside(pkg_dir: &Path, target: &Path) -> bool {
    match (normalize(pkg_dir), normalize(target)) {
        (Some(dir), Some(target)) => target.starts_with(dir),
        _ => false,
    }
}

fn is_include(name: &str) -> bool {
    matches!(name, "include" | "include_str" | "include_bytes")
}

fn find_refs(tokens: &[Tok]) -> Vec<SourceRef> {
    let mut out = Vec::new();
    let mut rest = tokens;
    while let Some((first, tail)) = rest.split_first() {
        match (first, tail) {
            (Tok::Punct('#'), [Tok::Punct('!'), Tok::Punct('['), body @ ..])
            | (Tok::Punct('#'), [Tok::Punct('['), body @ ..]) => {
                for window in attribute_body(body).windows(3) {
                    if let [Tok::Ident(key), Tok::Punct('='), Tok::Str(path)] = window {
                        if key == "path" {
                            out.push(SourceRef::PathAttr(path.clone()));
                        }
                    }
                }
            }
            (Tok::Ident(name), [Tok::Punct('!'), Tok::Punct('(' | '[' | '{'), args @ ..])
                if is_include(name) =>
            {
                out.push(SourceRef::Include {
                    name: name.clone(),
                    arg: include_arg(args),
                });
            }
            _ => {}
        }
        rest = tail;
    }
    out
}

/// The tokens of an attribute up to its closing `]`.
fn attribute_body(body: &[Tok]) -> &[Tok] {
    let mut depth = 0usize;
    for (i, tok) in body.iter().enumerate() {
        match tok {
            Tok::Punct('[') => depth = depth.saturating_add(1),
            Tok::Punct(']') if depth == 0 => return body.get(..i).unwrap_or(body),
            Tok::Punct(']') => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    body
}

fn include_arg(args: &[Tok]) -> IncludeArg {
    match args {
        [Tok::Str(path), ..] => IncludeArg::Literal(path.clone()),
        [Tok::Ident(concat), Tok::Punct('!'), Tok::Punct('('), Tok::Ident(env), Tok::Punct('!'), Tok::Punct('('), Tok::Str(var), Tok::Punct(')'), rest @ ..]
            if concat == "concat" && env == "env" && var == "CARGO_MANIFEST_DIR" =>
        {
            let mut path = String::new();
            for tok in rest {
                match tok {
                    Tok::Punct(',') => {}
                    Tok::Str(part) => path.push_str(part),
                    Tok::Punct(')') => return IncludeArg::ManifestRelative(path),
                    _ => return IncludeArg::Unresolved,
                }
            }
            IncludeArg::Unresolved
        }
        _ => IncludeArg::Unresolved,
    }
}

struct Cursor {
    chars: Vec<char>,
    pos: usize,
}

impl Cursor {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos.saturating_add(ahead)).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek(0);
        if c.is_some() {
            self.pos = self.pos.saturating_add(1);
        }
        c
    }
}

/// A rough Rust tokenizer: enough to tell code from comments and string contents.
fn lex(text: &str) -> Vec<Tok> {
    let mut cur = Cursor {
        chars: text.chars().collect(),
        pos: 0,
    };
    let mut out = Vec::new();
    while let Some(c) = cur.peek(0) {
        if c.is_whitespace() {
            cur.bump();
        } else if c == '/' && cur.peek(1) == Some('/') {
            while cur.bump().is_some_and(|c| c != '\n') {}
        } else if c == '/' && cur.peek(1) == Some('*') {
            skip_block_comment(&mut cur);
        } else if c == '"' {
            cur.bump();
            out.push(Tok::Str(quoted(&mut cur)));
        } else if c == '\'' {
            skip_char_or_lifetime(&mut cur);
        } else if c.is_alphabetic() || c == '_' {
            let mut ident = String::new();
            while let Some(c) = cur.peek(0).filter(|c| c.is_alphanumeric() || *c == '_') {
                ident.push(c);
                cur.bump();
            }
            match (ident.as_str(), cur.peek(0)) {
                ("r" | "br" | "cr", Some('"' | '#')) => {
                    if let Some(s) = raw_string(&mut cur) {
                        out.push(Tok::Str(s));
                    }
                }
                ("b" | "c", Some('"')) => {
                    cur.bump();
                    out.push(Tok::Str(quoted(&mut cur)));
                }
                ("b", Some('\'')) => {}
                _ => out.push(Tok::Ident(ident)),
            }
        } else if c.is_ascii_digit() {
            while cur.peek(0).is_some_and(|c| c.is_alphanumeric() || c == '_') {
                cur.bump();
            }
            out.push(Tok::Other);
        } else {
            cur.bump();
            out.push(Tok::Punct(c));
        }
    }
    out
}

fn skip_block_comment(cur: &mut Cursor) {
    cur.bump();
    cur.bump();
    let mut depth = 1usize;
    while depth > 0 {
        match cur.bump() {
            None => return,
            Some('/') if cur.peek(0) == Some('*') => {
                cur.bump();
                depth = depth.saturating_add(1);
            }
            Some('*') if cur.peek(0) == Some('/') => {
                cur.bump();
                depth = depth.saturating_sub(1);
            }
            Some(_) => {}
        }
    }
}

/// Body of a `"..."` literal; the opening quote is already consumed.
fn quoted(cur: &mut Cursor) -> String {
    let mut s = String::new();
    while let Some(c) = cur.bump() {
        match c {
            '"' => break,
            '\\' => match cur.bump() {
                Some('n') => s.push('\n'),
                Some('t') => s.push('\t'),
                Some('r') => s.push('\r'),
                Some('0') => s.push('\0'),
                Some('\n') => {
                    while cur.peek(0).is_some_and(char::is_whitespace) {
                        cur.bump();
                    }
                }
                Some(other @ ('\\' | '"' | '\'')) => s.push(other),
                Some(other) => {
                    s.push('\\');
                    s.push(other);
                }
                None => break,
            },
            other => s.push(other),
        }
    }
    s
}

/// At the `#` or `"` after an `r` prefix: a raw string's body, or `None` for a raw
/// identifier (`r#name`), whose `#` is consumed so the name lexes next.
fn raw_string(cur: &mut Cursor) -> Option<String> {
    let mut hashes = 0usize;
    while cur.peek(hashes) == Some('#') {
        hashes = hashes.saturating_add(1);
    }
    if cur.peek(hashes) != Some('"') {
        cur.bump();
        return None;
    }
    for _ in 0..=hashes {
        cur.bump();
    }
    let mut s = String::new();
    while let Some(c) = cur.bump() {
        if c == '"' && (0..hashes).all(|k| cur.peek(k) == Some('#')) {
            for _ in 0..hashes {
                cur.bump();
            }
            break;
        }
        s.push(c);
    }
    Some(s)
}

/// At a `'`: skips a char literal (`'x'`, `'\n'`, `'\u{..}'`) or the quote of a lifetime.
fn skip_char_or_lifetime(cur: &mut Cursor) {
    if cur.peek(1) == Some('\\') {
        cur.bump();
        cur.bump();
        cur.bump();
        while cur.bump().is_some_and(|c| c != '\'' && c != '\n') {}
    } else if cur.peek(2) == Some('\'') {
        cur.bump();
        cur.bump();
        cur.bump();
    } else {
        cur.bump();
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{find_refs, lex, scan_package, scan_source, unsafe_word, IncludeArg, SourceRef};

    fn refs(text: &str) -> Vec<SourceRef> {
        find_refs(&lex(text))
    }

    fn pkg() -> PathBuf {
        std::env::temp_dir()
            .join("kitkraft-xtask-fixture")
            .join("crates")
            .join("esm")
    }

    fn errors(file_in_pkg: &str, text: &str) -> Vec<String> {
        let dir = pkg();
        scan_source(&dir.join(file_in_pkg), &dir, text)
    }

    #[test]
    fn comments_strings_and_chars_hide_lookalikes() {
        let text = r##"
            // include_str!("../../x")
            /* #[path = "../x.rs"] /* nested */ include!("y") */
            const A: &str = "#[path = \"../x.rs\"] include_bytes!(\"z\")";
            const B: &str = r#"include!("w") "quoted""#;
            const C: char = '"';
            fn f<'a>(x: &'a str) -> &'a str { x }
        "##;
        assert_eq!(refs(text), Vec::<SourceRef>::new());
    }

    #[test]
    fn finds_path_attributes_including_cfg_attr() {
        let text = r#"
            #[path = "../../doc/src/lib.rs"] mod a;
            #[cfg_attr(windows, path = "win.rs")] mod b;
            #[doc = "path"] mod c;
        "#;
        assert_eq!(
            refs(text),
            [
                SourceRef::PathAttr("../../doc/src/lib.rs".into()),
                SourceRef::PathAttr("win.rs".into()),
            ]
        );
    }

    #[test]
    fn classifies_include_arguments() {
        let text = r#"
            include!("a.rs");
            std::include_str!(r"b.txt");
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/defs/", "c.bin"));
            include_str!(concat!(env!("OUT_DIR"), "/gen.rs"));
            include_str![$path];
        "#;
        let args: Vec<IncludeArg> = refs(text)
            .into_iter()
            .map(|r| match r {
                SourceRef::Include { arg, .. } => arg,
                SourceRef::PathAttr(p) => panic!("unexpected path attr {p}"),
            })
            .collect();
        assert_eq!(
            args,
            [
                IncludeArg::Literal("a.rs".into()),
                IncludeArg::Literal("b.txt".into()),
                IncludeArg::ManifestRelative("/defs/c.bin".into()),
                IncludeArg::Unresolved,
                IncludeArg::Unresolved,
            ]
        );
    }

    #[test]
    fn paths_leaving_the_package_fail() {
        assert_eq!(
            errors("src/lib.rs", r#"#[path = "../../doc/src/x.rs"] mod x;"#).len(),
            1
        );
        assert_eq!(
            errors(
                "src/lib.rs",
                r#"const X: &str = include_str!("../../../secret");"#
            )
            .len(),
            1
        );
        assert_eq!(
            errors(
                "src/lib.rs",
                r#"const X: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../ba2/x"));"#
            )
            .len(),
            1
        );
        assert_eq!(
            errors(
                "src/lib.rs",
                r#"include!(concat!(env!("OUT_DIR"), "/x.rs"));"#
            )
            .len(),
            1
        );
        let absolute = std::env::temp_dir().join("elsewhere.rs");
        let text = format!("#[path = {:?}] mod x;", absolute.display().to_string());
        assert_eq!(errors("src/lib.rs", &text).len(), 1);
    }

    #[test]
    fn paths_inside_the_package_pass() {
        let text = r#"
            #[path = "platform/windows.rs"] mod windows;
            #[path = "../tests/common/mod.rs"] mod common;
            const D: &str = include_str!("../defs/WEAP.toml");
            const E: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/defs/TES4.toml"));
        "#;
        assert_eq!(errors("src/lib.rs", text), Vec::<String>::new());
        assert_eq!(
            errors("src/deep/mod.rs", r#"include!("../../Cargo.toml");"#),
            Vec::<String>::new()
        );
    }

    #[test]
    fn scans_a_package_directory() {
        let dir = std::env::temp_dir().join(format!("kitkraft-xtask-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src/nested")).unwrap();
        fs::create_dir_all(dir.join("tests")).unwrap();
        fs::write(dir.join("src/lib.rs"), "mod nested;\n").unwrap();
        fs::write(
            dir.join("src/nested/mod.rs"),
            "const OK: &str = include_str!(\"../lib.rs\");\n",
        )
        .unwrap();
        fs::write(
            dir.join("tests/smuggle.rs"),
            "#[path = \"../../other/src/lib.rs\"] mod other;\n",
        )
        .unwrap();
        fs::write(dir.join("notes.txt"), "include!(\"../../x\")").unwrap();
        let found = scan_package(&dir, "kitkraft-fixture").unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("smuggle.rs"), "{found:?}");
    }

    #[test]
    fn unsafe_is_found_in_code_only() {
        for bad in [
            "fn f() { unsafe { g() } }",
            "unsafe fn f() {}",
            "unsafe impl Send for X {}",
            "#![allow(unsafe_code)]",
            "#[cfg_attr(test, allow(dead_code, unsafe_code))] mod m;",
        ] {
            assert!(unsafe_word(&lex(bad)).is_some(), "{bad}");
        }
        for good in [
            "// unsafe { g() }",
            "/* #![allow(unsafe_code)] */",
            "const S: &str = \"unsafe\";",
            "fn unsafe_free() {} let not_unsafe = 1;",
        ] {
            assert!(unsafe_word(&lex(good)).is_none(), "{good}");
        }
    }

    /// Only the listed file of the listed package may use `unsafe`; the same file in
    /// another package may not, and a listed file that is gone is reported.
    #[test]
    fn unsafe_is_held_to_the_allowed_files() {
        let dir =
            std::env::temp_dir().join(format!("kitkraft-xtask-unsafe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src/viewport")).unwrap();
        let gl = "#![allow(unsafe_code)]\nfn f() { unsafe { g() } }\n";
        fs::write(dir.join("src/viewport/gl.rs"), gl).unwrap();
        fs::write(dir.join("src/lib.rs"), "mod viewport;\n").unwrap();
        assert_eq!(
            scan_package(&dir, "kitkraft-ui-egui").unwrap(),
            Vec::<String>::new()
        );
        let other = scan_package(&dir, "kitkraft-esm").unwrap();
        assert_eq!(other.len(), 1, "{other:?}");
        assert!(other[0].contains("gl.rs") && other[0].contains("UNSAFE_ALLOWED"));
        fs::write(dir.join("src/lib.rs"), "mod viewport;\nunsafe fn x() {}\n").unwrap();
        let lib = scan_package(&dir, "kitkraft-ui-egui").unwrap();
        assert_eq!(lib.len(), 1, "{lib:?}");
        assert!(lib[0].contains("lib.rs"), "{lib:?}");
        fs::remove_file(dir.join("src/viewport/gl.rs")).unwrap();
        let stale = scan_package(&dir, "kitkraft-ui-egui").unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(
            stale.iter().any(|e| e.contains("does not exist")),
            "{stale:?}"
        );
    }
}
