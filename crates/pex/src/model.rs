//! The structured model. String references are indices into [`Pex::strings`], checked when
//! parsed, so [`Pex::string`] never misses on a parsed script.

/// A parsed script.
#[derive(Debug, Clone, PartialEq)]
pub struct Pex {
    /// File header.
    pub header: Header,
    /// The string table.
    pub strings: Vec<String>,
    /// Offset of the string table's count.
    pub strings_offset: u64,
    /// Debug info, when present.
    pub debug: Option<DebugInfo>,
    /// User flag names and bit indices.
    pub user_flags: Vec<UserFlag>,
    /// Objects (scripts) in the file; one in practice.
    pub objects: Vec<Object>,
}

impl Pex {
    /// String `index`, or `""` when out of range (never for a parsed script).
    pub fn string(&self, index: u16) -> &str {
        self.strings
            .get(usize::from(index))
            .map_or("", String::as_str)
    }
}

/// The file header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Magic number.
    pub magic: u32,
    /// Major version (3 for Starfield).
    pub major: u8,
    /// Minor version (12 for Starfield).
    pub minor: u8,
    /// Game id.
    pub game_id: u16,
    /// Compile time (seconds since 1970).
    pub compile_time: u64,
    /// Source file name.
    pub source: String,
    /// User name.
    pub user: String,
    /// Machine name.
    pub machine: String,
}

/// Debug info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugInfo {
    /// Offset of the debug block (its flag byte).
    pub offset: u64,
    /// Modification time.
    pub modification_time: u64,
    /// Line tables per function.
    pub functions: Vec<DebugFunction>,
    /// Property groups.
    pub property_groups: Vec<PropertyGroup>,
    /// Struct member orders.
    pub struct_orders: Vec<StructOrder>,
}

/// Line numbers of one function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFunction {
    /// Object name (string index).
    pub object: u16,
    /// State name.
    pub state: u16,
    /// Function name.
    pub function: u16,
    /// Function kind (0 normal, 1 getter, 2 setter).
    pub kind: u8,
    /// Source line per instruction.
    pub lines: Vec<u16>,
}

/// A property group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyGroup {
    /// Object name.
    pub object: u16,
    /// Group name.
    pub name: u16,
    /// Documentation string.
    pub doc: u16,
    /// User flags.
    pub user_flags: u32,
    /// Property names.
    pub properties: Vec<u16>,
}

/// The declared order of a struct's members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructOrder {
    /// Object name.
    pub object: u16,
    /// Struct name.
    pub name: u16,
    /// Member names.
    pub members: Vec<u16>,
}

/// One user flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserFlag {
    /// Name.
    pub name: u16,
    /// Bit index.
    pub bit: u8,
}

/// A value: an instruction operand, a variable's initial value, a member default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    /// Type 0.
    None,
    /// Type 1: an identifier (string index).
    Identifier(u16),
    /// Type 2: a string literal (string index).
    String(u16),
    /// Type 3.
    Int(i32),
    /// Type 4.
    Float(f32),
    /// Type 5 (the raw byte; non-zero is true).
    Bool(u8),
}

/// One object (script).
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    /// Offset of the object's name.
    pub offset: u64,
    /// Name.
    pub name: u16,
    /// The size field (bytes from the size field itself to the end of the object, in every
    /// official script).
    pub size: u32,
    /// Parent class name (empty string for none).
    pub parent: u16,
    /// Documentation string.
    pub doc: u16,
    /// Const flag.
    pub const_flag: u8,
    /// User flags.
    pub user_flags: u32,
    /// Auto state name.
    pub auto_state: u16,
    /// Structs.
    pub structs: Vec<Struct>,
    /// Variables.
    pub variables: Vec<Variable>,
    /// Guards (v3.12 and later).
    pub guards: Vec<u16>,
    /// Properties.
    pub properties: Vec<Property>,
    /// States (the empty-named state holds the functions outside any state).
    pub states: Vec<State>,
}

/// A struct.
#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    /// Offset of the name.
    pub offset: u64,
    /// Name.
    pub name: u16,
    /// Members.
    pub members: Vec<Member>,
}

/// A struct member.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    /// Name.
    pub name: u16,
    /// Type name.
    pub type_name: u16,
    /// User flags.
    pub user_flags: u32,
    /// Default value.
    pub value: Value,
    /// Const flag.
    pub const_flag: u8,
    /// Documentation string.
    pub doc: u16,
}

/// A variable.
#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    /// Offset of the name.
    pub offset: u64,
    /// Name.
    pub name: u16,
    /// Type name.
    pub type_name: u16,
    /// User flags.
    pub user_flags: u32,
    /// Initial value.
    pub value: Value,
    /// Const flag.
    pub const_flag: u8,
}

/// A property.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Offset of the name.
    pub offset: u64,
    /// Name.
    pub name: u16,
    /// Type name.
    pub type_name: u16,
    /// Documentation string.
    pub doc: u16,
    /// User flags.
    pub user_flags: u32,
    /// Flags: 1 read, 2 write, 4 auto.
    pub flags: u8,
    /// Backing variable of an auto property.
    pub auto_var: Option<u16>,
    /// Getter (flag 1, not auto).
    pub getter: Option<Function>,
    /// Setter (flag 2, not auto).
    pub setter: Option<Function>,
}

/// A state.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// Offset of the name.
    pub offset: u64,
    /// Name (empty for the default state).
    pub name: u16,
    /// Functions.
    pub functions: Vec<NamedFunction>,
}

/// A function in a state.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedFunction {
    /// Name.
    pub name: u16,
    /// Body.
    pub function: Function,
}

/// A function body.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    /// Offset of the return type.
    pub offset: u64,
    /// Return type name.
    pub return_type: u16,
    /// Documentation string.
    pub doc: u16,
    /// User flags.
    pub user_flags: u32,
    /// Flags (global, native, ...).
    pub flags: u8,
    /// Parameters: (name, type).
    pub params: Vec<(u16, u16)>,
    /// Locals: (name, type).
    pub locals: Vec<(u16, u16)>,
    /// Code.
    pub instructions: Vec<Instruction>,
}

/// One instruction.
#[derive(Debug, Clone, PartialEq)]
pub struct Instruction {
    /// Offset of the opcode byte.
    pub offset: u64,
    /// Opcode.
    pub opcode: u8,
    /// Fixed operands.
    pub args: Vec<Value>,
    /// The counted operands of call and guard opcodes (their count is not stored here).
    pub varargs: Option<Vec<Value>>,
}
