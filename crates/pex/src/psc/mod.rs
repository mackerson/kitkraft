//! Papyrus source (`.psc`) declaration reader (M4.G1).
//!
//! Reads what a script declares, not what it does: the header (name, parent, flags),
//! imports, functions and events (parameters with types and defaults, return type, flags,
//! the state they sit in), properties, structs and their members, script variables,
//! custom events and guards. Function, event and property bodies are skipped. A statement
//! the reader does not understand outside a body is reported in [`Script::diagnostics`]
//! with its line, never a panic; only a structural fault (an unterminated comment, string
//! or block, an `End...` without its opener) is an [`PscError`], with its line.
//!
//! Sources (ADR 0006 Amendment 1): `corpus:` the 5,033 `.psc` files under `Scripts/Source`
//! in `Tools\ContentResources.zip`, read in place (every construct accepted here occurs
//! there; counts in `plan/STATUS.md`), and `ck-wiki:` for the language rules, in our own
//! words. No other Papyrus parser or grammar was read.
//!
//! Declaration shapes (`corpus:`; keywords are case-insensitive, flags in any order):
//!
//! - `ScriptName <Name> [Extends <Parent>] <flags>`; script flags seen: `Native`, `Const`,
//!   `Hidden`, `Conditional`, `Default`, `DebugOnly`.
//! - `[<Type>] Function <Name>(<params>) <flags>` and `Event <Name>(<params>) <flags>`;
//!   an event may name another script's event (`Event ObjectReference.OnLoad(...)`, a
//!   remote event handler: 515 in the corpus). A function flagged `Native` has no body;
//!   every other function and every event ends at `EndFunction` / `EndEvent`. Function
//!   flags seen: `Native`, `Global`, `DebugOnly`, `BetaOnly`, `Protected`, `SelfOnly`,
//!   `Private`.
//! - A parameter is `<Type> <name> [= <literal>]`; a type is a name with an optional `[]`.
//! - `<Type> Property <Name> [= <literal>] <flags>`: with `Auto` or `AutoReadOnly` (or
//!   `Native`) it is one line; otherwise `Function Get()` / `Function Set(...)` follow up to
//!   `EndProperty`. Property flags seen: `Auto`, `AutoReadOnly`, `Const`, `Mandatory`,
//!   `Hidden`, `Conditional` and `RequiresGuard(<guard>)`, a flag with an argument.
//! - `Struct <Name>` ... `EndStruct`, members `<Type> <name> [= <literal>] <flags>`.
//! - `Group <Name> <flags>` ... `EndGroup` around properties; `[Auto] State <Name>` ...
//!   `EndState` around functions and events.
//! - `Import <Name>`, `CustomEvent <Name>`, `Guard <Name> <flags>`, and script variables
//!   `<Type> <name> [= <literal>] <flags>`.
//!
//! Comments are kept on the declarations they document (`doc`: the `{ }` comment that
//! follows a declaration; `comment`: the `;` or `;/ /;` comments right above it) so a caller can show
//! them on request; KitCraft never stores or commits them (ADR 0006 Amendment 1).

mod lex;

use lex::{lex, Comment, Line, Token, TokenKind};

/// Largest source accepted (the largest official source is 170,924 bytes).
pub const MAX_SOURCE_BYTES: usize = 16 << 20;
/// Deepest nesting of `Group`, `State`, `Struct`, `Property` and bodies.
pub const MAX_NESTING: usize = 16;

/// A structural fault; `line` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PscError {
    /// The source is larger than [`MAX_SOURCE_BYTES`].
    #[error("{len} bytes is above the {MAX_SOURCE_BYTES}-byte limit")]
    TooLarge {
        /// Its length.
        len: usize,
    },
    /// A `;/` comment with no `/;`.
    #[error("line {line}: `;/` comment is not closed")]
    UnterminatedComment {
        /// Where it starts.
        line: u32,
    },
    /// A `{` comment with no `}`.
    #[error("line {line}: `{{` comment is not closed")]
    UnterminatedDoc {
        /// Where it starts.
        line: u32,
    },
    /// A string with no closing quote on its line.
    #[error("line {line}: string is not closed")]
    UnterminatedString {
        /// Its line.
        line: u32,
    },
    /// No `ScriptName` statement.
    #[error("no ScriptName statement")]
    MissingScriptName,
    /// An `End...` with no open block of its kind.
    #[error("line {line}: `{keyword}` without its opening statement")]
    UnexpectedEnd {
        /// Its line.
        line: u32,
        /// The keyword as written.
        keyword: String,
    },
    /// A block still open at the end of the file, or closed by the wrong `End...`.
    #[error("line {line}: {kind} `{name}` is not closed (expected `{expected}`{found})")]
    Unclosed {
        /// Where the block starts.
        line: u32,
        /// `function`, `event`, `property`, `struct`, `group`, `state`.
        kind: &'static str,
        /// Its name.
        name: String,
        /// The keyword that would close it.
        expected: &'static str,
        /// ` before ... at line N`, or empty at the end of the file.
        found: String,
    },
    /// Blocks nested deeper than [`MAX_NESTING`].
    #[error("line {line}: blocks nested deeper than {MAX_NESTING}")]
    TooDeep {
        /// Where the limit was hit.
        line: u32,
    },
}

/// Where a declaration is: its first line, and the byte range of its statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// 1-based line of the statement's first token.
    pub line: u32,
    /// Byte offset of the first token.
    pub offset: u64,
    /// Bytes to the end of the statement's last token.
    pub len: u64,
}

/// A type: a name (`Int`, `ObjectReference`, `Ns:Script`) and whether it is an array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRef {
    /// As written.
    pub name: String,
    /// `[]` follows the name.
    pub array: bool,
}

impl std::fmt::Display for TypeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.array {
            write!(f, "{}[]", self.name)
        } else {
            f.write_str(&self.name)
        }
    }
}

/// A flag word, with its argument for `RequiresGuard(<guard>)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    /// As written.
    pub name: String,
    /// The text inside the parentheses, if any.
    pub arg: Option<String>,
}

impl std::fmt::Display for Flag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.arg {
            Some(a) => write!(f, "{}({a})", self.name),
            None => f.write_str(&self.name),
        }
    }
}

/// Whether `flags` has `name` (ASCII case-insensitive).
pub fn has_flag(flags: &[Flag], name: &str) -> bool {
    flags.iter().any(|f| f.name.eq_ignore_ascii_case(name))
}

/// A function or event parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Its type.
    pub ty: TypeRef,
    /// Its name.
    pub name: String,
    /// The default value as written (`-1`, `"text"`, `None`), if any.
    pub default: Option<String>,
}

/// Function or event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableKind {
    /// `Function`.
    Function,
    /// `Event`.
    Event,
}

impl CallableKind {
    /// `function` / `event`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Event => "event",
        }
    }
}

/// A function or event declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callable {
    /// Function or event.
    pub kind: CallableKind,
    /// Its name (for a remote event, the event's own name: `OnLoad`).
    pub name: String,
    /// For a remote event handler (`Event ObjectReference.OnLoad`), the script named.
    pub remote: Option<String>,
    /// Return type; `None` for an event or a function returning nothing.
    pub return_type: Option<TypeRef>,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// Flags as written, in order.
    pub flags: Vec<Flag>,
    /// The `State` it is declared in; `None` for the empty state.
    pub state: Option<String>,
    /// The declaration statement.
    pub span: Span,
    /// The `{ }` comment after the declaration.
    pub doc: Option<String>,
    /// The `;` (or `;/ /;`) comments directly above it.
    pub comment: Option<String>,
}

/// A property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Its type.
    pub ty: TypeRef,
    /// Its name.
    pub name: String,
    /// The initial value as written, if any.
    pub default: Option<String>,
    /// Flags as written.
    pub flags: Vec<Flag>,
    /// The `Group` it is in.
    pub group: Option<String>,
    /// Whether it has `Get`/`Set` functions (no `Auto`/`AutoReadOnly`).
    pub full: bool,
    /// The declaration statement.
    pub span: Span,
    /// The `{ }` comment after the declaration.
    pub doc: Option<String>,
}

/// A script variable or a struct member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// Its type.
    pub ty: TypeRef,
    /// Its name.
    pub name: String,
    /// The initial value as written, if any.
    pub default: Option<String>,
    /// Flags as written.
    pub flags: Vec<Flag>,
    /// The declaration statement.
    pub span: Span,
    /// The `{ }` comment after the declaration.
    pub doc: Option<String>,
}

/// A struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Struct {
    /// Its name.
    pub name: String,
    /// Members, in order.
    pub members: Vec<Variable>,
    /// The `Struct` statement.
    pub span: Span,
}

/// A named declaration with flags: `CustomEvent`, `Guard`, `Import`, `State`, `Group`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    /// Its name.
    pub name: String,
    /// Flags as written (`Auto` for an auto state).
    pub flags: Vec<Flag>,
    /// The statement.
    pub span: Span,
}

/// A statement the reader did not understand (outside any body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// 1-based line.
    pub line: u32,
    /// What was expected; names only the first word of the statement.
    pub message: String,
}

/// Everything a script declares.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Script {
    /// The name after `ScriptName`.
    pub name: String,
    /// The name after `Extends`.
    pub parent: Option<String>,
    /// Script flags as written.
    pub flags: Vec<Flag>,
    /// The `ScriptName` statement.
    pub span: Span,
    /// The `{ }` comment after `ScriptName`.
    pub doc: Option<String>,
    /// `Import` statements.
    pub imports: Vec<Named>,
    /// Functions, in source order (property `Get`/`Set` functions excluded).
    pub functions: Vec<Callable>,
    /// Events, in source order.
    pub events: Vec<Callable>,
    /// Properties, in source order.
    pub properties: Vec<Property>,
    /// Structs, in source order.
    pub structs: Vec<Struct>,
    /// Script variables.
    pub variables: Vec<Variable>,
    /// `CustomEvent` declarations.
    pub custom_events: Vec<Named>,
    /// `Guard` declarations.
    pub guards: Vec<Named>,
    /// `State` blocks (`Auto` in their flags for the auto state).
    pub states: Vec<Named>,
    /// `Group` blocks.
    pub groups: Vec<Named>,
    /// Statements not understood.
    pub diagnostics: Vec<Diagnostic>,
}

/// An open block.
#[derive(Debug)]
struct Block {
    kind: BlockKind,
    name: String,
    line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Function,
    Event,
    Property,
    Struct,
    Group,
    State,
}

impl BlockKind {
    fn name(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Event => "event",
            Self::Property => "property",
            Self::Struct => "struct",
            Self::Group => "group",
            Self::State => "state",
        }
    }

    fn end(self) -> &'static str {
        match self {
            Self::Function => "EndFunction",
            Self::Event => "EndEvent",
            Self::Property => "EndProperty",
            Self::Struct => "EndStruct",
            Self::Group => "EndGroup",
            Self::State => "EndState",
        }
    }

    fn from_end(word: &str) -> Option<Self> {
        Some(match word {
            "endfunction" => Self::Function,
            "endevent" => Self::Event,
            "endproperty" => Self::Property,
            "endstruct" => Self::Struct,
            "endgroup" => Self::Group,
            "endstate" => Self::State,
            _ => return None,
        })
    }
}

/// Words that start a statement and so are never a type name.
const KEYWORDS: [&str; 24] = [
    "scriptname",
    "extends",
    "import",
    "function",
    "event",
    "property",
    "struct",
    "group",
    "state",
    "auto",
    "customevent",
    "guard",
    "if",
    "elseif",
    "else",
    "endif",
    "while",
    "endwhile",
    "return",
    "new",
    "as",
    "is",
    "lockguard",
    "tryguard",
];

/// Reads the declarations of one source file. Bytes that are not UTF-8 are replaced
/// (every official source is UTF-8).
pub fn parse(src: &[u8]) -> Result<Script, PscError> {
    if src.len() > MAX_SOURCE_BYTES {
        return Err(PscError::TooLarge { len: src.len() });
    }
    let text = String::from_utf8_lossy(src);
    let lexed = lex(&text)?;
    let mut p = Parser {
        script: Script::default(),
        stack: Vec::new(),
        named: false,
        comments: &lexed.comments,
        src: &text,
    };
    let lines = &lexed.lines;
    for (i, line) in lines.iter().enumerate() {
        let next_line = lines.get(i.saturating_add(1)).map(|l| l.line);
        p.line(line, next_line)?;
    }
    if let Some(open) = p.stack.last() {
        return Err(PscError::Unclosed {
            line: open.line,
            kind: open.kind.name(),
            name: open.name.clone(),
            expected: open.kind.end(),
            found: String::new(),
        });
    }
    if !p.named {
        return Err(PscError::MissingScriptName);
    }
    Ok(p.script)
}

struct Parser<'c> {
    script: Script,
    stack: Vec<Block>,
    named: bool,
    comments: &'c [Comment],
    src: &'c str,
}

/// The words of a statement after a position, as a cursor.
struct Cursor<'t> {
    tokens: &'t [Token],
    at: usize,
}

impl<'t> Cursor<'t> {
    fn new(tokens: &'t [Token]) -> Self {
        Cursor { tokens, at: 0 }
    }

    fn peek(&self) -> Option<&'t Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<&'t Token> {
        let t = self.tokens.get(self.at);
        if t.is_some() {
            self.at = self.at.saturating_add(1);
        }
        t
    }

    fn word(&mut self) -> Option<String> {
        match self.peek().map(|t| &t.kind) {
            Some(TokenKind::Word(w)) => {
                let w = w.clone();
                self.at = self.at.saturating_add(1);
                Some(w)
            }
            _ => None,
        }
    }

    fn eat_punct(&mut self, c: char) -> bool {
        if self.peek().is_some_and(|t| t.is_punct(c)) {
            self.at = self.at.saturating_add(1);
            true
        } else {
            false
        }
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.peek().is_some_and(|t| t.is_word(w)) {
            self.at = self.at.saturating_add(1);
            true
        } else {
            false
        }
    }

    fn done(&self) -> bool {
        self.at >= self.tokens.len()
    }

    /// A type: a word that is not a keyword, with an optional `[]`.
    fn ty(&mut self) -> Option<TypeRef> {
        let start = self.at;
        let name = self.word()?;
        if KEYWORDS.contains(&name.to_ascii_lowercase().as_str()) {
            self.at = start;
            return None;
        }
        let array = if self.peek().is_some_and(|t| t.is_punct('['))
            && self
                .tokens
                .get(self.at.saturating_add(1))
                .is_some_and(|t| t.is_punct(']'))
        {
            self.at = self.at.saturating_add(2);
            true
        } else {
            false
        };
        Some(TypeRef { name, array })
    }

    /// A literal after `=`: an optional sign and one token, as written in `src`.
    fn literal(&mut self, src_of: &dyn Fn(usize, usize) -> String) -> Option<String> {
        let first = self.next()?;
        let mut end = first.end;
        if first.is_punct('-') || first.is_punct('+') {
            let t = self.next()?;
            if t.kind != TokenKind::Number {
                return None;
            }
            end = t.end;
        } else if !matches!(
            first.kind,
            TokenKind::Number | TokenKind::Str | TokenKind::Word(_)
        ) {
            return None;
        }
        Some(src_of(first.start, end))
    }

    /// Flag words up to the end of the statement; `None` on anything else.
    fn flags(&mut self, src_of: &dyn Fn(usize, usize) -> String) -> Option<Vec<Flag>> {
        let mut out = Vec::new();
        while !self.done() {
            let name = self.word()?;
            let arg = if self.eat_punct('(') {
                let open = self.peek()?.start;
                let close = loop {
                    let t = self.next()?;
                    if t.is_punct(')') {
                        break t.start;
                    }
                };
                Some(src_of(open, close).trim().to_string())
            } else {
                None
            };
            out.push(Flag { name, arg });
        }
        Some(out)
    }
}

impl Parser<'_> {
    fn line(&mut self, line: &Line, next_line: Option<u32>) -> Result<(), PscError> {
        let Some(first) = line.tokens.first() else {
            return Ok(());
        };
        let first_word = first.word_lower().unwrap_or_default();
        let top = self.stack.last().map(|b| b.kind);

        // Inside a body only the matching end matters.
        if matches!(top, Some(BlockKind::Function | BlockKind::Event)) {
            if let Some(kind) = BlockKind::from_end(&first_word) {
                if Some(kind) == top {
                    self.stack.pop();
                } else if matches!(kind, BlockKind::Function | BlockKind::Event) {
                    return Err(self.unclosed(line.line, &first_word));
                }
            }
            return Ok(());
        }

        if let Some(kind) = BlockKind::from_end(&first_word) {
            return match self.stack.last() {
                Some(b) if b.kind == kind => {
                    self.stack.pop();
                    Ok(())
                }
                Some(_) => Err(self.unclosed(line.line, &first_word)),
                None => Err(PscError::UnexpectedEnd {
                    line: line.line,
                    keyword: word_text(first),
                }),
            };
        }

        let ctx = Ctx {
            span: span_of(line),
            doc: self.doc_for(line.line, next_line),
            comment: self.comment_above(line.line),
        };
        match top {
            Some(BlockKind::Struct) => self.struct_member(line, ctx),
            Some(BlockKind::Property) => self.in_property(line, &first_word),
            _ => self.statement(line, &first_word, ctx),
        }
    }

    fn unclosed(&self, line: u32, found: &str) -> PscError {
        match self.stack.last() {
            Some(open) => PscError::Unclosed {
                line: open.line,
                kind: open.kind.name(),
                name: open.name.clone(),
                expected: open.kind.end(),
                found: format!(" before `{found}` at line {line}"),
            },
            None => PscError::UnexpectedEnd {
                line,
                keyword: found.to_string(),
            },
        }
    }

    fn push(&mut self, kind: BlockKind, name: String, line: u32) -> Result<(), PscError> {
        if self.stack.len() >= MAX_NESTING {
            return Err(PscError::TooDeep { line });
        }
        self.stack.push(Block { kind, name, line });
        Ok(())
    }

    fn diag(&mut self, line: u32, message: String) {
        self.script.diagnostics.push(Diagnostic { line, message });
    }

    fn current(&self, kind: BlockKind) -> Option<String> {
        self.stack
            .iter()
            .rev()
            .find(|b| b.kind == kind)
            .map(|b| b.name.clone())
    }

    /// Inside `Property` ... `EndProperty`: `Get`/`Set` functions (bodies skipped).
    fn in_property(&mut self, line: &Line, first_word: &str) -> Result<(), PscError> {
        let mut c = Cursor::new(&line.tokens);
        let _ = c.ty();
        if c.eat_word("function") {
            let name = c.word().unwrap_or_default();
            let native = line.tokens.iter().any(|t| t.is_word("native"));
            if !native {
                self.push(BlockKind::Function, name, line.line)?;
            }
            return Ok(());
        }
        self.diag(
            line.line,
            format!("`{first_word}` inside a property: expected Function Get/Set or EndProperty"),
        );
        Ok(())
    }

    fn struct_member(&mut self, line: &Line, ctx: Ctx) -> Result<(), PscError> {
        match self.variable(line, ctx) {
            Some(v) => {
                if let Some(s) = self.script.structs.last_mut() {
                    s.members.push(v);
                }
            }
            None => {
                let w = line.tokens.first().map(word_text).unwrap_or_default();
                self.diag(
                    line.line,
                    format!("`{w}` inside a struct: expected `<Type> <name> [= value]`"),
                );
            }
        }
        Ok(())
    }

    fn statement(&mut self, line: &Line, first_word: &str, ctx: Ctx) -> Result<(), PscError> {
        let src = source_of(self.src);
        let mut c = Cursor::new(&line.tokens);
        match first_word {
            "scriptname" => {
                c.next();
                let Some(name) = c.word() else {
                    self.diag(line.line, "ScriptName without a name".into());
                    return Ok(());
                };
                let parent = if c.eat_word("extends") {
                    c.word()
                } else {
                    None
                };
                let flags = c.flags(&src).unwrap_or_else(|| {
                    self.script.diagnostics.push(Diagnostic {
                        line: line.line,
                        message: "ScriptName: unreadable flags".into(),
                    });
                    Vec::new()
                });
                if self.named {
                    self.diag(line.line, "a second ScriptName statement".into());
                    return Ok(());
                }
                self.named = true;
                self.script.name = name;
                self.script.parent = parent;
                self.script.flags = flags;
                self.script.span = ctx.span;
                self.script.doc = ctx.doc;
            }
            "import" | "customevent" | "guard" => {
                c.next();
                let Some(name) = c.word() else {
                    self.diag(line.line, format!("`{first_word}` without a name"));
                    return Ok(());
                };
                let flags = c.flags(&src).unwrap_or_default();
                let named = Named {
                    name,
                    flags,
                    span: ctx.span,
                };
                match first_word {
                    "import" => self.script.imports.push(named),
                    "customevent" => self.script.custom_events.push(named),
                    _ => self.script.guards.push(named),
                }
            }
            "struct" | "group" | "state" | "auto" => {
                c.next();
                let mut flags = Vec::new();
                let kind = match first_word {
                    "struct" => BlockKind::Struct,
                    "group" => BlockKind::Group,
                    "state" => BlockKind::State,
                    _ => {
                        if !c.eat_word("state") {
                            self.diag(line.line, "`auto` not followed by State".into());
                            return Ok(());
                        }
                        flags.push(Flag {
                            name: line.tokens.first().map(word_text).unwrap_or_default(),
                            arg: None,
                        });
                        BlockKind::State
                    }
                };
                let Some(name) = c.word() else {
                    self.diag(line.line, format!("`{first_word}` without a name"));
                    return Ok(());
                };
                flags.extend(c.flags(&src).unwrap_or_default());
                self.push(kind, name.clone(), line.line)?;
                let named = Named {
                    name: name.clone(),
                    flags,
                    span: ctx.span,
                };
                match kind {
                    BlockKind::Struct => self.script.structs.push(Struct {
                        name,
                        members: Vec::new(),
                        span: ctx.span,
                    }),
                    BlockKind::Group => self.script.groups.push(named),
                    _ => self.script.states.push(named),
                }
            }
            _ => self.typed_statement(line, first_word, ctx)?,
        }
        Ok(())
    }

    /// `[Type] Function`, `Event`, `Type Property`, or a variable.
    fn typed_statement(&mut self, line: &Line, first_word: &str, ctx: Ctx) -> Result<(), PscError> {
        let src = source_of(self.src);
        let mut c = Cursor::new(&line.tokens);
        let ty = c.ty();
        if c.eat_word("function") || (ty.is_none() && c.eat_word("event")) {
            let kind = if line
                .tokens
                .get(c.at.saturating_sub(1))
                .is_some_and(|t| t.is_word("event"))
            {
                CallableKind::Event
            } else {
                CallableKind::Function
            };
            return self.callable(line, kind, ty, c, ctx);
        }
        if let Some(ty) = ty {
            if c.eat_word("property") {
                let Some(name) = c.word() else {
                    self.diag(line.line, "Property without a name".into());
                    return Ok(());
                };
                let default = if c.eat_punct('=') {
                    c.literal(&src)
                } else {
                    None
                };
                let Some(flags) = c.flags(&src) else {
                    self.diag(line.line, format!("property `{name}`: unreadable flags"));
                    return Ok(());
                };
                let full = !["auto", "autoreadonly", "native"]
                    .iter()
                    .any(|f| has_flag(&flags, f));
                if full {
                    self.push(BlockKind::Property, name.clone(), line.line)?;
                }
                self.script.properties.push(Property {
                    ty,
                    name,
                    default,
                    flags,
                    group: self.current(BlockKind::Group),
                    full,
                    span: ctx.span,
                    doc: ctx.doc,
                });
                return Ok(());
            }
        }
        match self.variable(line, ctx) {
            Some(v) => self.script.variables.push(v),
            None => self.diag(
                line.line,
                format!("`{first_word}`: not a declaration this reader knows"),
            ),
        }
        Ok(())
    }

    fn callable(
        &mut self,
        line: &Line,
        kind: CallableKind,
        return_type: Option<TypeRef>,
        mut c: Cursor<'_>,
        ctx: Ctx,
    ) -> Result<(), PscError> {
        let src = source_of(self.src);
        let Some(first) = c.word() else {
            self.diag(line.line, format!("{} without a name", kind.as_str()));
            return Ok(());
        };
        let (remote, name) = if c.eat_punct('.') {
            match c.word() {
                Some(n) => (Some(first), n),
                None => {
                    self.diag(
                        line.line,
                        format!("{} `{first}.`: no event name", kind.as_str()),
                    );
                    return Ok(());
                }
            }
        } else {
            (None, first)
        };
        let params = match params(&mut c, &src) {
            Some(p) => p,
            None => {
                self.diag(
                    line.line,
                    format!("{} `{name}`: unreadable parameters", kind.as_str()),
                );
                // Still skip its body (unless native) so the rest of the file stays in step.
                if !line.tokens.iter().any(|t| t.is_word("native")) {
                    self.push(block_of(kind), name, line.line)?;
                }
                return Ok(());
            }
        };
        let flags = c.flags(&src).unwrap_or_else(|| {
            self.script.diagnostics.push(Diagnostic {
                line: line.line,
                message: format!("{} `{name}`: unreadable flags", kind.as_str()),
            });
            Vec::new()
        });
        let native = has_flag(&flags, "native");
        if !native {
            self.push(block_of(kind), name.clone(), line.line)?;
        }
        let callable = Callable {
            kind,
            name,
            remote,
            return_type,
            params,
            flags,
            state: self.current(BlockKind::State),
            span: ctx.span,
            doc: ctx.doc,
            comment: ctx.comment,
        };
        match kind {
            CallableKind::Function => self.script.functions.push(callable),
            CallableKind::Event => self.script.events.push(callable),
        }
        Ok(())
    }

    /// `<Type> <name> [= literal] <flags>`.
    fn variable(&self, line: &Line, ctx: Ctx) -> Option<Variable> {
        let src = source_of(self.src);
        let mut c = Cursor::new(&line.tokens);
        let ty = c.ty()?;
        let name = c.word()?;
        if KEYWORDS.contains(&name.to_ascii_lowercase().as_str()) {
            return None;
        }
        let default = if c.eat_punct('=') {
            Some(c.literal(&src)?)
        } else {
            None
        };
        let flags = c.flags(&src)?;
        Some(Variable {
            ty,
            name,
            default,
            flags,
            span: ctx.span,
            doc: ctx.doc,
        })
    }

    /// The `{ }` comment that starts on the statement's line or before the next statement.
    fn doc_for(&self, line: u32, next_line: Option<u32>) -> Option<String> {
        self.comments
            .iter()
            .find(|c| c.doc && c.line >= line && next_line.is_none_or(|n| c.line < n))
            .map(|c| c.text.clone())
    }

    /// The `;` comments on the lines directly above `line`, each alone on its line.
    fn comment_above(&self, line: u32) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        let mut want = line.checked_sub(1)?;
        for c in self.comments.iter().rev().filter(|c| c.line < line) {
            if c.doc || !c.own_line || c.end_line != want {
                if c.end_line < want {
                    break;
                }
                continue;
            }
            parts.push(&c.text);
            match c.line.checked_sub(1) {
                Some(w) => want = w,
                None => break,
            }
        }
        if parts.is_empty() {
            return None;
        }
        parts.reverse();
        Some(parts.join("\n"))
    }
}

/// What a declaration statement carries besides its tokens.
struct Ctx {
    span: Span,
    doc: Option<String>,
    comment: Option<String>,
}

fn block_of(kind: CallableKind) -> BlockKind {
    match kind {
        CallableKind::Function => BlockKind::Function,
        CallableKind::Event => BlockKind::Event,
    }
}

fn word_text(t: &Token) -> String {
    match &t.kind {
        TokenKind::Word(w) => w.clone(),
        TokenKind::Punct(c) => c.to_string(),
        TokenKind::Number => "number".into(),
        TokenKind::Str => "string".into(),
    }
}

fn span_of(line: &Line) -> Span {
    let (start, end) = line.span();
    Span {
        line: line.line,
        offset: start as u64,
        len: end.saturating_sub(start) as u64,
    }
}

/// The source text of a byte range, as written.
fn source_of(src: &str) -> impl Fn(usize, usize) -> String + '_ {
    move |start: usize, end: usize| src.get(start..end).unwrap_or_default().to_string()
}

/// `( [param {, param}] )`.
fn params(c: &mut Cursor<'_>, src: &dyn Fn(usize, usize) -> String) -> Option<Vec<Param>> {
    if !c.eat_punct('(') {
        return None;
    }
    let mut out = Vec::new();
    if c.eat_punct(')') {
        return Some(out);
    }
    loop {
        let ty = c.ty()?;
        let name = c.word()?;
        let default = if c.eat_punct('=') {
            Some(c.literal(src)?)
        } else {
            None
        };
        out.push(Param { ty, name, default });
        if c.eat_punct(')') {
            return Some(out);
        }
        if !c.eat_punct(',') {
            return None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
mod tests;
