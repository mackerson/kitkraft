//! The `where` language of `query` (M2.D10): parser with column-numbered errors,
//! three-valued (Kleene) evaluation, and paging cursors.
//!
//! ```text
//! query   = or ;
//! or      = and { "or" and } ;
//! and     = not { "and" not } ;
//! not     = "not" not | atom ;
//! atom    = "(" or ")" | "has" SIG | path op value ;
//! path    = SIG { "." member } [ "[" index "]" ] | "@form" | "@plugin" | "@edid" | "@flags" ;
//! op      = "==" | "!=" | "<" | "<=" | ">" | ">=" | "~" | "!~" | "^=" ;
//! value   = number | hex | string | formref | "true" | "false" ;
//! formref = plugin-file-name ":" hex ;          (Starfield.esm:0001A2B3)
//! ```
//!
//! Semantics are documented in `docs/mcp.md` (`query`). In short: keywords and member
//! names are ASCII case-insensitive; text comparisons are ASCII case-insensitive; a
//! repeated subrecord matches if any occurrence matches; an absent subrecord makes a
//! comparison false; a value that cannot be decoded makes it unknown.

use std::fmt;

use crate::form::FormRef;

/// Longest `where` text accepted.
pub const MAX_QUERY_LEN: usize = 4096;
/// Deepest nesting of parentheses and `not`.
pub const MAX_DEPTH: usize = 64;

/// A parse error at a 1-based character column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    /// 1-based column of the offending character.
    pub column: usize,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "column {}: {}", self.column, self.message)
    }
}

impl std::error::Error for QueryError {}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `~` contains
    Contains,
    /// `!~` does not contain
    NotContains,
    /// `^=` starts with
    StartsWith,
}

impl Op {
    fn is_ordering(self) -> bool {
        matches!(self, Op::Lt | Op::Le | Op::Gt | Op::Ge)
    }
    fn is_text(self) -> bool {
        matches!(self, Op::Contains | Op::NotContains | Op::StartsWith)
    }
}

/// A literal.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Decimal number.
    Number(f64),
    /// `0x...`
    Hex(u64),
    /// `"..."`
    Text(String),
    /// `Plugin.esm:XXXXXX`
    Form(FormRef),
    /// `true` / `false`
    Bool(bool),
}

/// A field path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Path {
    /// `SIG.member...[n]`.
    Field {
        /// Subrecord signature (as written, upper-cased).
        signature: [u8; 4],
        /// Member names, lower-cased.
        members: Vec<String>,
        /// Occurrence picked with `[n]`.
        index: Option<u32>,
    },
    /// `@form`: the form's resolved identity.
    Form,
    /// `@plugin`: the plugin holding this version.
    Plugin,
    /// `@edid`: the EditorID.
    Edid,
    /// `@flags`: record flags.
    Flags,
}

/// The parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Any true.
    Or(Vec<Expr>),
    /// All true.
    And(Vec<Expr>),
    /// Negation.
    Not(Box<Expr>),
    /// `has SIG`.
    Has([u8; 4]),
    /// `path op value`.
    Cmp {
        /// Left side.
        path: Path,
        /// Operator.
        op: Op,
        /// Right side.
        value: Literal,
    },
}

/// Three-valued truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tri {
    /// Known true.
    True,
    /// Known false.
    False,
    /// Cannot be decided.
    Unknown,
}

impl Tri {
    fn from_bool(b: bool) -> Tri {
        if b {
            Tri::True
        } else {
            Tri::False
        }
    }
    fn not(self) -> Tri {
        match self {
            Tri::True => Tri::False,
            Tri::False => Tri::True,
            Tri::Unknown => Tri::Unknown,
        }
    }
}

/// A decoded field value.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// Text (zstring, resolved lstring).
    Text(String),
    /// Integer.
    Int(i64),
    /// Float.
    Float(f64),
    /// A resolved FormID: defining plugin name and object ID.
    Form {
        /// Defining plugin.
        plugin: String,
        /// Object ID.
        object_id: u32,
    },
    /// Boolean.
    Bool(bool),
}

/// What a path yields on one record.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup {
    /// The record does not have it (known).
    Absent,
    /// It cannot be decoded; the reason goes to `limitations`.
    Unknown(String),
    /// One value per occurrence.
    Values(Vec<FieldValue>),
}

/// The record a query is evaluated against.
pub trait Subject {
    /// Lookup of a path (`Field`, `Form`, `Plugin`, `Edid` or `Flags`).
    fn lookup(&mut self, path: &Path) -> Lookup;
    /// Whether the record has a subrecord, or why that is unknown.
    fn has(&mut self, signature: [u8; 4]) -> Result<bool, String>;
}

/// A parsed query.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// The expression.
    pub expr: Expr,
    /// The text it was parsed from.
    pub text: String,
}

impl Query {
    /// Parse `where` text.
    pub fn parse(text: &str) -> Result<Query, QueryError> {
        if text.chars().count() > MAX_QUERY_LEN {
            return Err(QueryError {
                column: MAX_QUERY_LEN,
                message: format!("query longer than {MAX_QUERY_LEN} characters"),
            });
        }
        let tokens = lex(text)?;
        let mut p = Parser {
            tokens,
            pos: 0,
            end_column: text.chars().count().saturating_add(1),
        };
        let expr = p.or(0)?;
        if let Some(t) = p.peek() {
            return Err(QueryError {
                column: t.column,
                message: format!("unexpected {}", t.kind.describe()),
            });
        }
        Ok(Query {
            expr,
            text: text.to_string(),
        })
    }

    /// Whether evaluation needs the record's subrecords (anything but `@` paths).
    pub fn needs_body(&self) -> bool {
        fn walk(e: &Expr) -> bool {
            match e {
                Expr::Or(v) | Expr::And(v) => v.iter().any(walk),
                Expr::Not(e) => walk(e),
                Expr::Has(_) => true,
                Expr::Cmp { path, .. } => matches!(path, Path::Field { .. }),
            }
        }
        walk(&self.expr)
    }

    /// Evaluate on `subject`; reasons for unknown parts are appended to `unknown`.
    pub fn eval(&self, subject: &mut dyn Subject, unknown: &mut Vec<String>) -> Tri {
        eval(&self.expr, subject, unknown)
    }
}

fn eval(e: &Expr, s: &mut dyn Subject, unknown: &mut Vec<String>) -> Tri {
    match e {
        Expr::Or(items) => {
            let mut out = Tri::False;
            for item in items {
                match eval(item, s, unknown) {
                    Tri::True => return Tri::True,
                    Tri::Unknown => out = Tri::Unknown,
                    Tri::False => {}
                }
            }
            out
        }
        Expr::And(items) => {
            let mut out = Tri::True;
            for item in items {
                match eval(item, s, unknown) {
                    Tri::False => return Tri::False,
                    Tri::Unknown => out = Tri::Unknown,
                    Tri::True => {}
                }
            }
            out
        }
        Expr::Not(inner) => eval(inner, s, unknown).not(),
        Expr::Has(sig) => match s.has(*sig) {
            Ok(b) => Tri::from_bool(b),
            Err(reason) => {
                unknown.push(reason);
                Tri::Unknown
            }
        },
        Expr::Cmp { path, op, value } => match s.lookup(path) {
            Lookup::Absent => Tri::False,
            Lookup::Unknown(reason) => {
                unknown.push(reason);
                Tri::Unknown
            }
            Lookup::Values(values) => {
                let mut out = Tri::False;
                for v in &values {
                    match compare(v, *op, value) {
                        Ok(true) => return Tri::True,
                        Ok(false) => {}
                        Err(reason) => {
                            unknown.push(reason);
                            out = Tri::Unknown;
                        }
                    }
                }
                out
            }
        },
    }
}

fn ci_contains(hay: &str, needle: &str) -> bool {
    hay.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn compare(v: &FieldValue, op: Op, lit: &Literal) -> Result<bool, String> {
    match (v, lit) {
        (FieldValue::Text(t), Literal::Text(x)) => Ok(match op {
            Op::Eq => t.eq_ignore_ascii_case(x),
            Op::Ne => !t.eq_ignore_ascii_case(x),
            Op::Contains => ci_contains(t, x),
            Op::NotContains => !ci_contains(t, x),
            Op::StartsWith => t.get(..x.len()).is_some_and(|p| p.eq_ignore_ascii_case(x)),
            Op::Lt | Op::Le | Op::Gt | Op::Ge => false,
        }),
        (FieldValue::Form { plugin, object_id }, Literal::Form(f)) => {
            let same = plugin.eq_ignore_ascii_case(&f.plugin) && *object_id == f.object_id;
            match op {
                Op::Eq => Ok(same),
                Op::Ne => Ok(!same),
                _ => Err("FormIDs compare only with == and !=".into()),
            }
        }
        (FieldValue::Form { plugin, object_id }, Literal::Text(x)) => match FormRef::parse(x) {
            Some(f) => compare(
                &FieldValue::Form {
                    plugin: plugin.clone(),
                    object_id: *object_id,
                },
                op,
                &Literal::Form(f),
            ),
            None => Err(format!("`{x}` is not a formref (Plugin.esm:XXXXXX)")),
        },
        (FieldValue::Bool(b), Literal::Bool(x)) => match op {
            Op::Eq => Ok(b == x),
            Op::Ne => Ok(b != x),
            _ => Err("booleans compare only with == and !=".into()),
        },
        (FieldValue::Int(_) | FieldValue::Float(_), Literal::Number(_) | Literal::Hex(_)) => {
            let a = match v {
                FieldValue::Int(i) => *i as f64,
                FieldValue::Float(f) => *f,
                _ => 0.0,
            };
            let b = match lit {
                Literal::Number(n) => *n,
                Literal::Hex(h) => *h as f64,
                _ => 0.0,
            };
            match op {
                Op::Eq => Ok(a == b),
                Op::Ne => Ok(a != b),
                Op::Lt => Ok(a < b),
                Op::Le => Ok(a <= b),
                Op::Gt => Ok(a > b),
                Op::Ge => Ok(a >= b),
                _ => Err("~, !~ and ^= need text".into()),
            }
        }
        // A value of another kind (a union variant, a text field compared with a number):
        // known not equal, not ordered, not containing.
        _ => Ok(matches!(op, Op::Ne | Op::NotContains)),
    }
}

// ---- lexer ----

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Ident(String),
    At(String),
    Str(String),
    Num(f64),
    Hex(u64),
    FormRef(FormRef),
    Op(Op),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Dot,
}

impl Kind {
    fn describe(&self) -> String {
        match self {
            Kind::Ident(s) => format!("`{s}`"),
            Kind::At(s) => format!("`@{s}`"),
            Kind::Str(_) => "a string".into(),
            Kind::Num(_) | Kind::Hex(_) => "a number".into(),
            Kind::FormRef(_) => "a formref".into(),
            Kind::Op(_) => "an operator".into(),
            Kind::LParen => "`(`".into(),
            Kind::RParen => "`)`".into(),
            Kind::LBracket => "`[`".into(),
            Kind::RBracket => "`]`".into(),
            Kind::Dot => "`.`".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: Kind,
    column: usize,
}

fn err(column: usize, message: impl Into<String>) -> QueryError {
    QueryError {
        column,
        message: message.into(),
    }
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Characters allowed in an unquoted plugin file name of a formref.
fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+' | '\'')
}

fn lex(text: &str) -> Result<Vec<Token>, QueryError> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let at = |i: usize| chars.get(i).copied();
    while let Some(c) = at(i) {
        let column = i.saturating_add(1);
        let next = at(i.saturating_add(1));
        if c.is_whitespace() {
            i = i.saturating_add(1);
            continue;
        }
        let single = match c {
            '(' => Some(Kind::LParen),
            ')' => Some(Kind::RParen),
            '[' => Some(Kind::LBracket),
            ']' => Some(Kind::RBracket),
            '.' if !next.is_some_and(|n| n.is_ascii_digit()) => Some(Kind::Dot),
            '~' => Some(Kind::Op(Op::Contains)),
            _ => None,
        };
        if let Some(kind) = single {
            out.push(Token { kind, column });
            i = i.saturating_add(1);
            continue;
        }
        let two: String = [Some(c), next].iter().flatten().collect();
        let op2 = match two.as_str() {
            "==" => Some(Op::Eq),
            "!=" => Some(Op::Ne),
            "<=" => Some(Op::Le),
            ">=" => Some(Op::Ge),
            "!~" => Some(Op::NotContains),
            "^=" => Some(Op::StartsWith),
            _ => None,
        };
        if let Some(op) = op2 {
            out.push(Token {
                kind: Kind::Op(op),
                column,
            });
            i = i.saturating_add(2);
            continue;
        }
        match c {
            '<' | '>' => {
                let op = if c == '<' { Op::Lt } else { Op::Gt };
                out.push(Token {
                    kind: Kind::Op(op),
                    column,
                });
                i = i.saturating_add(1);
            }
            '=' => return Err(err(column, "`=` is not an operator; use `==`")),
            '!' => return Err(err(column, "`!` must be followed by `=` or `~`")),
            '"' | '\'' => {
                let quote = c;
                let mut s = String::new();
                let mut j = i.saturating_add(1);
                loop {
                    match at(j) {
                        None => return Err(err(column, "unterminated string")),
                        Some('\\') => match at(j.saturating_add(1)) {
                            Some(e @ ('\\' | '"' | '\'')) => {
                                s.push(e);
                                j = j.saturating_add(2);
                            }
                            _ => {
                                s.push('\\');
                                j = j.saturating_add(1);
                            }
                        },
                        Some(ch) if ch == quote => break,
                        Some(ch) => {
                            s.push(ch);
                            j = j.saturating_add(1);
                        }
                    }
                }
                out.push(Token {
                    kind: Kind::Str(s),
                    column,
                });
                i = j.saturating_add(1);
            }
            '@' => {
                let mut j = i.saturating_add(1);
                while at(j).is_some_and(is_word) {
                    j = j.saturating_add(1);
                }
                let word: String = chars
                    .get(i.saturating_add(1)..j)
                    .unwrap_or(&[])
                    .iter()
                    .collect();
                if word.is_empty() {
                    return Err(err(
                        column,
                        "`@` must be followed by form, plugin, edid or flags",
                    ));
                }
                out.push(Token {
                    kind: Kind::At(word.to_ascii_lowercase()),
                    column,
                });
                i = j;
            }
            _ if c.is_ascii_digit() || c == '-' || c == '.' => {
                let mut j = i.saturating_add(1);
                while at(j).is_some_and(|d| is_name(d) || d == ':') {
                    j = j.saturating_add(1);
                }
                let word: String = chars.get(i..j).unwrap_or(&[]).iter().collect();
                out.push(Token {
                    kind: number_or_formref(&word, column)?,
                    column,
                });
                i = j;
            }
            _ if is_word(c) => {
                let mut j = i.saturating_add(1);
                while at(j).is_some_and(is_word) {
                    j = j.saturating_add(1);
                }
                // A plugin file name followed by `:hex` is a formref.
                let mut k = j;
                while at(k).is_some_and(is_name) {
                    k = k.saturating_add(1);
                }
                if at(k) == Some(':') && k > j {
                    let mut m = k.saturating_add(1);
                    while at(m).is_some_and(|d| d.is_ascii_alphanumeric()) {
                        m = m.saturating_add(1);
                    }
                    let word: String = chars.get(i..m).unwrap_or(&[]).iter().collect();
                    let f = FormRef::parse(&word).ok_or_else(|| {
                        err(
                            column,
                            format!("`{word}` is not a formref (Plugin.esm:XXXXXX)"),
                        )
                    })?;
                    out.push(Token {
                        kind: Kind::FormRef(f),
                        column,
                    });
                    i = m;
                    continue;
                }
                let word: String = chars.get(i..j).unwrap_or(&[]).iter().collect();
                out.push(Token {
                    kind: Kind::Ident(word),
                    column,
                });
                i = j;
            }
            _ => return Err(err(column, format!("unexpected character `{c}`"))),
        }
    }
    Ok(out)
}

fn number_or_formref(word: &str, column: usize) -> Result<Kind, QueryError> {
    if word.contains(':') {
        return FormRef::parse(word).map(Kind::FormRef).ok_or_else(|| {
            err(
                column,
                format!("`{word}` is not a formref (Plugin.esm:XXXXXX)"),
            )
        });
    }
    if let Some(hex) = word.strip_prefix("0x").or_else(|| word.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16)
            .map(Kind::Hex)
            .map_err(|_| err(column, format!("`{word}` is not a hex number")));
    }
    word.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .map(Kind::Num)
        .ok_or_else(|| err(column, format!("`{word}` is not a number")))
}

// ---- parser ----

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    end_column: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos = self.pos.saturating_add(1);
        }
        t
    }

    fn column(&self) -> usize {
        self.peek().map_or(self.end_column, |t| t.column)
    }

    fn keyword(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Token { kind: Kind::Ident(w), .. }) if w.eq_ignore_ascii_case(kw))
    }

    fn deeper(&self, depth: usize) -> Result<usize, QueryError> {
        if depth >= MAX_DEPTH {
            return Err(err(
                self.column(),
                format!("nesting deeper than {MAX_DEPTH}"),
            ));
        }
        Ok(depth.saturating_add(1))
    }

    fn or(&mut self, depth: usize) -> Result<Expr, QueryError> {
        let depth = self.deeper(depth)?;
        let mut items = vec![self.and(depth)?];
        while self.keyword("or") {
            self.bump();
            items.push(self.and(depth)?);
        }
        Ok(if items.len() == 1 {
            items.pop().unwrap_or(Expr::Or(Vec::new()))
        } else {
            Expr::Or(items)
        })
    }

    fn and(&mut self, depth: usize) -> Result<Expr, QueryError> {
        let mut items = vec![self.not(depth)?];
        while self.keyword("and") {
            self.bump();
            items.push(self.not(depth)?);
        }
        Ok(if items.len() == 1 {
            items.pop().unwrap_or(Expr::And(Vec::new()))
        } else {
            Expr::And(items)
        })
    }

    fn not(&mut self, depth: usize) -> Result<Expr, QueryError> {
        if self.keyword("not") {
            self.bump();
            let depth = self.deeper(depth)?;
            return Ok(Expr::Not(Box::new(self.not(depth)?)));
        }
        self.atom(depth)
    }

    fn atom(&mut self, depth: usize) -> Result<Expr, QueryError> {
        let column = self.column();
        if matches!(
            self.peek(),
            Some(Token {
                kind: Kind::LParen,
                ..
            })
        ) {
            self.bump();
            let inner = self.or(depth)?;
            match self.bump() {
                Some(Token {
                    kind: Kind::RParen, ..
                }) => return Ok(inner),
                Some(t) => {
                    return Err(err(
                        t.column,
                        format!("expected `)`, found {}", t.kind.describe()),
                    ))
                }
                None => return Err(err(self.end_column, "expected `)`")),
            }
        }
        if self.keyword("has") {
            self.bump();
            let column = self.column();
            return match self.bump() {
                Some(Token {
                    kind: Kind::Ident(w),
                    ..
                }) => signature(&w, column).map(Expr::Has),
                _ => Err(err(column, "`has` needs a four-character signature")),
            };
        }
        let path = self.path()?;
        let op_column = self.column();
        let op = match self.bump() {
            Some(Token {
                kind: Kind::Op(op), ..
            }) => op,
            Some(t) => {
                return Err(err(
                    t.column,
                    format!("expected an operator, found {}", t.kind.describe()),
                ))
            }
            None => return Err(err(op_column, "expected an operator")),
        };
        let value_column = self.column();
        let value = match self.bump() {
            Some(Token { kind, .. }) => match kind {
                Kind::Str(s) => Literal::Text(s),
                Kind::Num(n) => Literal::Number(n),
                Kind::Hex(h) => Literal::Hex(h),
                Kind::FormRef(f) => Literal::Form(f),
                Kind::Ident(w) if w.eq_ignore_ascii_case("true") => Literal::Bool(true),
                Kind::Ident(w) if w.eq_ignore_ascii_case("false") => Literal::Bool(false),
                other => {
                    return Err(err(
                        value_column,
                        format!("expected a value, found {}", other.describe()),
                    ))
                }
            },
            None => return Err(err(value_column, "expected a value")),
        };
        check(&path, op, &value, op_column, value_column)?;
        let _ = column;
        Ok(Expr::Cmp { path, op, value })
    }

    fn path(&mut self) -> Result<Path, QueryError> {
        let column = self.column();
        match self.bump() {
            Some(Token {
                kind: Kind::At(w), ..
            }) => match w.as_str() {
                "form" => Ok(Path::Form),
                "plugin" => Ok(Path::Plugin),
                "edid" => Ok(Path::Edid),
                "flags" => Ok(Path::Flags),
                _ => Err(err(
                    column,
                    format!("unknown `@{w}`: use @form, @plugin, @edid or @flags"),
                )),
            },
            Some(Token {
                kind: Kind::Ident(w),
                ..
            }) => {
                let sig = signature(&w, column)?;
                let mut members = Vec::new();
                while matches!(
                    self.peek(),
                    Some(Token {
                        kind: Kind::Dot,
                        ..
                    })
                ) {
                    self.bump();
                    let column = self.column();
                    match self.bump() {
                        Some(Token {
                            kind: Kind::Ident(m),
                            ..
                        }) => members.push(m.to_ascii_lowercase()),
                        _ => return Err(err(column, "expected a member name after `.`")),
                    }
                }
                let mut index = None;
                if matches!(
                    self.peek(),
                    Some(Token {
                        kind: Kind::LBracket,
                        ..
                    })
                ) {
                    self.bump();
                    let column = self.column();
                    index = match self.bump() {
                        Some(Token {
                            kind: Kind::Num(n), ..
                        }) if n >= 0.0 && n.fract() == 0.0 && n <= f64::from(u32::MAX) => {
                            Some(n as u32)
                        }
                        _ => return Err(err(column, "expected a non-negative index")),
                    };
                    let column = self.column();
                    if !matches!(
                        self.bump(),
                        Some(Token {
                            kind: Kind::RBracket,
                            ..
                        })
                    ) {
                        return Err(err(column, "expected `]`"));
                    }
                }
                Ok(Path::Field {
                    signature: sig,
                    members,
                    index,
                })
            }
            Some(t) => Err(err(
                t.column,
                format!("expected a field path, found {}", t.kind.describe()),
            )),
            None => Err(err(column, "expected a field path")),
        }
    }
}

fn signature(word: &str, column: usize) -> Result<[u8; 4], QueryError> {
    let upper = word.to_ascii_uppercase();
    upper.as_bytes().try_into().map_err(|_| {
        err(
            column,
            format!("`{word}` is not a four-character signature"),
        )
    })
}

/// Static type rules for the `@` paths and operator/value combinations.
fn check(
    path: &Path,
    op: Op,
    value: &Literal,
    op_col: usize,
    value_col: usize,
) -> Result<(), QueryError> {
    let text_value = matches!(value, Literal::Text(_));
    if op.is_text() && !text_value {
        return Err(err(value_col, "~, !~ and ^= need a string value"));
    }
    if op.is_ordering() && !matches!(value, Literal::Number(_) | Literal::Hex(_)) {
        return Err(err(value_col, "ordering operators need a number"));
    }
    match path {
        Path::Form => {
            let formish = matches!(value, Literal::Form(_))
                || matches!(value, Literal::Text(t) if FormRef::parse(t).is_some());
            if !matches!(op, Op::Eq | Op::Ne) {
                return Err(err(op_col, "@form compares only with == and !="));
            }
            if !formish {
                return Err(err(value_col, "@form needs a formref (Plugin.esm:XXXXXX)"));
            }
        }
        Path::Plugin | Path::Edid => {
            if !text_value {
                return Err(err(value_col, "@plugin and @edid need a string value"));
            }
            if op.is_ordering() {
                return Err(err(op_col, "@plugin and @edid do not support ordering"));
            }
        }
        Path::Flags => {
            if !matches!(value, Literal::Number(_) | Literal::Hex(_)) {
                return Err(err(value_col, "@flags needs a number"));
            }
        }
        Path::Field { .. } => {}
    }
    Ok(())
}

// ---- paging ----

/// Encode a cursor: `q1.<fingerprint hex>.<position>`.
pub fn encode_cursor(fingerprint: u64, position: usize) -> String {
    format!("q1.{fingerprint:016x}.{position}")
}

/// Decode a cursor made for `fingerprint`; another load order's (or query's) cursor is an
/// error.
pub fn decode_cursor(cursor: &str, fingerprint: u64) -> Result<usize, String> {
    let mut parts = cursor.split('.');
    let (Some("q1"), Some(fp), Some(pos), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(format!("`{cursor}` is not a query cursor"));
    };
    let fp =
        u64::from_str_radix(fp, 16).map_err(|_| format!("`{cursor}` is not a query cursor"))?;
    if fp != fingerprint {
        return Err("the cursor belongs to another load order or query".into());
    }
    pos.parse::<usize>()
        .map_err(|_| format!("`{cursor}` is not a query cursor"))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    struct Fake {
        edid: Option<String>,
        full: Lookup,
        kwda: Vec<i64>,
        flags: u32,
    }

    impl Subject for Fake {
        fn lookup(&mut self, path: &Path) -> Lookup {
            match path {
                Path::Edid => self.edid.clone().map_or(Lookup::Absent, |e| {
                    Lookup::Values(vec![FieldValue::Text(e)])
                }),
                Path::Plugin => Lookup::Values(vec![FieldValue::Text("Starfield.esm".into())]),
                Path::Flags => Lookup::Values(vec![FieldValue::Int(i64::from(self.flags))]),
                Path::Form => Lookup::Values(vec![FieldValue::Form {
                    plugin: "Starfield.esm".into(),
                    object_id: 0x1A2B3,
                }]),
                Path::Field {
                    signature, index, ..
                } => match signature {
                    b"FULL" => self.full.clone(),
                    b"KWDA" => {
                        let vals: Vec<FieldValue> =
                            self.kwda.iter().map(|v| FieldValue::Int(*v)).collect();
                        match index {
                            Some(i) => vals
                                .get(*i as usize)
                                .cloned()
                                .map_or(Lookup::Absent, |v| Lookup::Values(vec![v])),
                            None if vals.is_empty() => Lookup::Absent,
                            None => Lookup::Values(vals),
                        }
                    }
                    b"EDID" => self.edid.clone().map_or(Lookup::Absent, |e| {
                        Lookup::Values(vec![FieldValue::Text(e)])
                    }),
                    _ => {
                        Lookup::Unknown(format!("{} is opaque", String::from_utf8_lossy(signature)))
                    }
                },
            }
        }
        fn has(&mut self, signature: [u8; 4]) -> Result<bool, String> {
            match &signature {
                b"EDID" => Ok(self.edid.is_some()),
                b"KWDA" => Ok(!self.kwda.is_empty()),
                b"FULL" => Ok(!matches!(self.full, Lookup::Absent)),
                _ => Ok(false),
            }
        }
    }

    fn fake() -> Fake {
        Fake {
            edid: Some("OldEarthHuntingRifle".into()),
            full: Lookup::Unknown("FULL: string tables not loaded".into()),
            kwda: vec![3, 7],
            flags: 0x20,
        }
    }

    fn run(text: &str) -> (Tri, Vec<String>) {
        let q = Query::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
        let mut unknown = Vec::new();
        let t = q.eval(&mut fake(), &mut unknown);
        (t, unknown)
    }

    #[test]
    fn comparisons_and_kleene_logic() {
        assert_eq!(run(r#"EDID ~ "hunting""#).0, Tri::True);
        assert_eq!(run(r#"edid ^= "OLDEARTH""#).0, Tri::True);
        assert_eq!(run(r#"EDID !~ "rifle""#).0, Tri::False);
        assert_eq!(run(r#"EDID == 'oldearthhuntingrifle'"#).0, Tri::True);
        assert_eq!(run(r#"@edid != "x""#).0, Tri::True);
        assert_eq!(run("KWDA == 7").0, Tri::True);
        assert_eq!(run("KWDA[0] == 7").0, Tri::False);
        assert_eq!(run("KWDA[5] == 7").0, Tri::False);
        assert_eq!(run("KWDA > 5 and KWDA < 4").0, Tri::True);
        assert_eq!(run("@flags >= 0x20").0, Tri::True);
        assert_eq!(run("@form == Starfield.esm:0001A2B3").0, Tri::True);
        assert_eq!(run(r#"@form != "starfield.esm:1A2B4""#).0, Tri::True);
        assert_eq!(run(r#"@plugin == "STARFIELD.ESM""#).0, Tri::True);
        assert_eq!(run("has KWDA and not has DNAM").0, Tri::True);
        // An opaque subrecord is unknown; a missing one is known false.
        assert_eq!(run("DNAM == 1").0, Tri::Unknown);
        assert_eq!(run("KWDA[9] == 1").0, Tri::False);
        assert_eq!(run(r#"has DNAM"#).0, Tri::False);
        // Unknown propagates by Kleene rules.
        let (t, why) = run(r#"FULL ~ "Rifle""#);
        assert_eq!(t, Tri::Unknown);
        assert_eq!(why, ["FULL: string tables not loaded"]);
        assert_eq!(run(r#"FULL ~ "x" or EDID ~ "rifle""#).0, Tri::True);
        assert_eq!(run(r#"FULL ~ "x" and EDID ~ "nope""#).0, Tri::False);
        assert_eq!(run(r#"FULL ~ "x" and EDID ~ "rifle""#).0, Tri::Unknown);
        assert_eq!(run(r#"not FULL ~ "x""#).0, Tri::Unknown);
        assert_eq!(run(r#"not (EDID ~ "rifle" or FULL ~ "x")"#).0, Tri::False);
        // A value of another kind is known not to match (text is not ordered against 5).
        assert_eq!(run(r#"EDID > 5"#).0, Tri::False);
        assert_eq!(run(r#"KWDA ~ "x""#).0, Tri::False);
        assert_eq!(run(r#"KWDA !~ "x""#).0, Tri::True);
    }

    #[test]
    fn parse_shapes() {
        let q = Query::parse("WDMG.damage.value[2] >= 1.5").unwrap();
        assert_eq!(
            q.expr,
            Expr::Cmp {
                path: Path::Field {
                    signature: *b"WDMG",
                    members: vec!["damage".into(), "value".into()],
                    index: Some(2),
                },
                op: Op::Ge,
                value: Literal::Number(1.5),
            }
        );
        assert!(!Query::parse("@edid ~ \"a\"").unwrap().needs_body());
        assert!(Query::parse("@edid ~ \"a\" or has FULL")
            .unwrap()
            .needs_body());
        let q = Query::parse("a.esm:1 == 1");
        assert!(q.is_err());
        let q = Query::parse("@form == 'Sit To Add Ship.esm:000800'").unwrap();
        assert!(matches!(
            q.expr,
            Expr::Cmp {
                value: Literal::Text(_),
                ..
            }
        ));
        let q = Query::parse("@form == Ship-Mods_v2.esm:0x800").unwrap();
        assert!(matches!(
            q.expr,
            Expr::Cmp {
                value: Literal::Form(_),
                ..
            }
        ));
    }

    #[test]
    fn errors_have_columns() {
        let cases: &[(&str, usize, &str)] = &[
            ("", 1, "expected a field path"),
            ("EDID", 5, "expected an operator"),
            ("EDID ==", 8, "expected a value"),
            ("EDID = \"x\"", 6, "`=` is not an operator"),
            ("EDID ~ 5", 8, "need a string value"),
            ("EDID < \"x\"", 8, "ordering operators need a number"),
            ("EDIDX == 1", 1, "four-character signature"),
            ("@nope == 1", 1, "unknown `@nope`"),
            ("@form ~ \"x\"", 7, "compares only with"),
            ("@form == 5", 10, "@form needs a formref"),
            ("@form < 5", 7, "compares only with"),
            ("@flags == \"x\"", 11, "@flags needs a number"),
            ("(EDID == 1", 11, "expected `)`"),
            ("EDID == \"x", 9, "unterminated string"),
            ("EDID == 1 EDID", 11, "unexpected `EDID`"),
            ("has", 4, "`has` needs"),
            ("KWDA[-1] == 1", 6, "non-negative index"),
            ("KWDA[1 == 1", 8, "expected `]`"),
            ("EDID == 0xZZ", 9, "not a hex number"),
            ("EDID == x.esm:ZZ", 9, "not a formref"),
            ("EDID $ 1", 6, "unexpected character"),
            ("KWDA. == 1", 7, "member name"),
        ];
        for (text, column, needle) in cases {
            let e = Query::parse(text).unwrap_err();
            assert_eq!(e.column, *column, "{text}: {e}");
            assert!(e.message.contains(needle), "{text}: {e}");
        }
        let deep = format!("{}EDID == 1{}", "(".repeat(100), ")".repeat(100));
        assert!(Query::parse(&deep).unwrap_err().message.contains("nesting"));
        let nots = format!("{}EDID == 1", "not ".repeat(100));
        assert!(Query::parse(&nots).is_err());
        assert!(Query::parse(&"a".repeat(MAX_QUERY_LEN + 1)).is_err());
    }

    #[test]
    fn cursors() {
        let c = encode_cursor(0xABC, 50);
        assert_eq!(decode_cursor(&c, 0xABC), Ok(50));
        assert!(decode_cursor(&c, 0xABD).is_err());
        assert!(decode_cursor("q1.zz.1", 1).is_err());
        assert!(decode_cursor("x", 1).is_err());
        assert!(decode_cursor("q1.1.2.3", 1).is_err());
    }

    /// 10,000 seeded random inputs never panic (M2.D10).
    #[test]
    fn random_inputs_never_panic() {
        const ALPHABET: &[&str] = &[
            "EDID",
            "FULL",
            "KWDA",
            "@form",
            "@plugin",
            "@edid",
            "@flags",
            "@x",
            "has",
            "not",
            "and",
            "or",
            "(",
            ")",
            "[",
            "]",
            ".",
            "0",
            "1.5",
            "-3",
            "0x1F",
            "\"a\"",
            "'b'",
            "\"",
            "==",
            "!=",
            "<",
            "<=",
            ">",
            ">=",
            "~",
            "!~",
            "^=",
            "=",
            "!",
            "true",
            "false",
            "Starfield.esm:0001A2B3",
            "x.esm:",
            ":",
            "member",
            "é",
            "\\",
            " ",
            "$",
            "@",
        ];
        let mut state: u64 = 0x1234_5678_9ABC_DEF0;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..10_000 {
            let n = (next() % 12) as usize;
            let mut text = String::new();
            for _ in 0..n {
                text.push_str(ALPHABET[(next() % ALPHABET.len() as u64) as usize]);
                if next() % 2 == 0 {
                    text.push(' ');
                }
            }
            if let Ok(q) = Query::parse(&text) {
                let mut unknown = Vec::new();
                let _ = q.eval(&mut fake(), &mut unknown);
            }
        }
    }
}
