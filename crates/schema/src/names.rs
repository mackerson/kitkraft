//! Name rules shared by the validator and codegen.

/// `[A-Z][A-Za-z0-9]*`.
pub fn is_pascal_case(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

/// `NumRecords` -> `num_records`, `FormID` -> `form_id`, `HEDRSize` -> `hedr_size`.
pub fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len().saturating_add(4));
    let mut prev: Option<char> = None;
    let mut iter = chars.iter().copied().peekable();
    while let Some(c) = iter.next() {
        if c.is_ascii_uppercase() {
            let next_lower = iter.peek().is_some_and(|n| n.is_ascii_lowercase());
            let boundary = match prev {
                None => false,
                Some(p) => {
                    p.is_ascii_lowercase()
                        || p.is_ascii_digit()
                        || (p.is_ascii_uppercase() && next_lower)
                }
            };
            if boundary {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
        prev = Some(c);
    }
    out
}

/// Rust keywords (strict and reserved, 2021 edition) that cannot be plain identifiers.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "abstract", "become", "box", "do", "final", "gen", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// A snake_case Rust identifier for `name`, with `_` appended when it is a keyword.
pub fn field_ident(name: &str) -> String {
    let mut ident = snake_case(name);
    if KEYWORDS.contains(&ident.as_str()) {
        ident.push('_');
    }
    ident
}

/// SCREAMING_SNAKE_CASE for constants.
pub fn const_ident(name: &str) -> String {
    snake_case(name).to_ascii_uppercase()
}

/// A PascalCase type name for a signature (`NPC_` -> `Npc`, `TES4` -> `Tes4`).
pub fn signature_type_name(signature: &str) -> String {
    let mut out = String::with_capacity(4);
    for (i, c) in signature
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .enumerate()
    {
        if i == 0 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    if out.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        out.insert(0, 'R');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_handles_acronyms_and_digits() {
        assert_eq!(snake_case("NumRecords"), "num_records");
        assert_eq!(snake_case("FormID"), "form_id");
        assert_eq!(snake_case("HEDRSize"), "hedr_size");
        assert_eq!(snake_case("Value2"), "value2");
        assert_eq!(snake_case("Type"), "type");
        assert_eq!(field_ident("Type"), "type_");
    }

    #[test]
    fn pascal_case_rules() {
        assert!(is_pascal_case("EditorId"));
        assert!(!is_pascal_case("editorId"));
        assert!(!is_pascal_case("Editor_Id"));
        assert!(!is_pascal_case(""));
    }

    #[test]
    fn signature_type_names() {
        assert_eq!(signature_type_name("NPC_"), "Npc");
        assert_eq!(signature_type_name("TES4"), "Tes4");
        assert_eq!(signature_type_name("____"), "R");
    }
}
