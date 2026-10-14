//! Text output in `pexdis.py`'s format.
//!
//! ```text
//! <source> v<major>.<minor> gid<game id>
//! OBJECT <name> extends <parent>
//!   VAR <type> <name> = <value>
//!   PROP <type> <name>
//!   FUNC <name>(<param>, <param>)
//!       0 <opcode> <operand> <operand>
//! ```
//!
//! Property accessors print as `<prop>.get` / `<prop>.set`; functions of a named state as
//! `<state>.<function>`. Operands: `None`, identifiers bare, strings as Python's `repr`,
//! integers in decimal, floats as Python's `%g` of the value widened to `f64`, booleans as
//! `True`/`False`. The operand count of call and guard opcodes is not printed.

use std::fmt::Write as _;

use crate::model::{Function, Pex, Value};

/// Opcode table: name, fixed operand count, and whether a counted operand list follows
/// (`ours: pexdis.py`; every opcode in the official scripts is in it, `corpus:`).
pub const OPCODES: [(&str, u8, bool); 51] = [
    ("nop", 0, false),
    ("iadd", 3, false),
    ("fadd", 3, false),
    ("isub", 3, false),
    ("fsub", 3, false),
    ("imul", 3, false),
    ("fmul", 3, false),
    ("idiv", 3, false),
    ("fdiv", 3, false),
    ("imod", 3, false),
    ("not", 2, false),
    ("ineg", 2, false),
    ("fneg", 2, false),
    ("assign", 2, false),
    ("cast", 2, false),
    ("cmp_eq", 3, false),
    ("cmp_lt", 3, false),
    ("cmp_le", 3, false),
    ("cmp_gt", 3, false),
    ("cmp_ge", 3, false),
    ("jmp", 1, false),
    ("jmpt", 2, false),
    ("jmpf", 2, false),
    ("callmethod", 3, true),
    ("callparent", 2, true),
    ("callstatic", 3, true),
    ("return", 1, false),
    ("strcat", 3, false),
    ("propget", 3, false),
    ("propset", 3, false),
    ("array_create", 2, false),
    ("array_length", 2, false),
    ("array_get", 3, false),
    ("array_set", 3, false),
    ("array_find", 4, false),
    ("array_rfind", 4, false),
    ("is", 3, false),
    ("struct_create", 1, false),
    ("struct_get", 3, false),
    ("struct_set", 3, false),
    ("array_findstruct", 5, false),
    ("array_rfindstruct", 5, false),
    ("array_add", 3, false),
    ("array_insert", 3, false),
    ("array_removelast", 1, false),
    ("array_remove", 3, false),
    ("array_clear", 1, false),
    ("array_getallmatchingstructs", 6, false),
    ("lock_guards", 0, true),
    ("unlock_guards", 0, true),
    ("try_lock_guards", 1, true),
];

/// The mnemonic of an opcode, if known.
pub fn opcode_name(opcode: u8) -> Option<&'static str> {
    OPCODES.get(usize::from(opcode)).map(|(name, _, _)| *name)
}

/// One operand as `pexdis.py` prints it.
pub fn format_value(pex: &Pex, value: &Value) -> String {
    match value {
        Value::None => "None".to_string(),
        Value::Identifier(i) => pex.string(*i).to_string(),
        Value::String(i) => python_repr(pex.string(*i)),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => python_g(f64::from(*f)),
        Value::Bool(b) => if *b != 0 { "True" } else { "False" }.to_string(),
    }
}

/// The whole script, `pexdis.py`'s stdout byte for byte (UTF-8, trailing newline).
pub fn disassemble(pex: &Pex) -> String {
    let h = &pex.header;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} v{}.{} gid{}",
        h.source, h.major, h.minor, h.game_id
    );
    for object in &pex.objects {
        let _ = writeln!(
            out,
            "OBJECT {} extends {}",
            pex.string(object.name),
            pex.string(object.parent)
        );
        for v in &object.variables {
            let _ = writeln!(
                out,
                "  VAR {} {} = {}",
                pex.string(v.type_name),
                pex.string(v.name),
                format_value(pex, &v.value)
            );
        }
        for p in &object.properties {
            let name = pex.string(p.name);
            let _ = writeln!(out, "  PROP {} {}", pex.string(p.type_name), name);
            if let Some(f) = &p.getter {
                function(&mut out, pex, &format!("{name}.get"), f);
            }
            if let Some(f) = &p.setter {
                function(&mut out, pex, &format!("{name}.set"), f);
            }
        }
        for state in &object.states {
            let state_name = pex.string(state.name);
            for f in &state.functions {
                let name = if state_name.is_empty() {
                    pex.string(f.name).to_string()
                } else {
                    format!("{state_name}.{}", pex.string(f.name))
                };
                function(&mut out, pex, &name, &f.function);
            }
        }
    }
    out
}

fn function(out: &mut String, pex: &Pex, name: &str, f: &Function) {
    let params: Vec<&str> = f.params.iter().map(|(n, _)| pex.string(*n)).collect();
    let _ = writeln!(out, "  FUNC {name}({})", params.join(", "));
    for (i, ins) in f.instructions.iter().enumerate() {
        let operands: Vec<String> = ins
            .args
            .iter()
            .chain(ins.varargs.iter().flatten())
            .map(|v| format_value(pex, v))
            .collect();
        let _ = writeln!(
            out,
            "    {i:>3} {} {}",
            opcode_name(ins.opcode).unwrap_or("?"),
            operands.join(" ")
        );
    }
}

/// Python's `repr()` of a string whose chars are all <= U+00FF.
pub(crate) fn python_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len().saturating_add(2));
    out.push(quote);
    for c in s.chars() {
        let code = u32::from(c);
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            _ if code < 0x20 || (0x7F..=0xA0).contains(&code) || code == 0xAD => {
                let _ = write!(out, "\\x{code:02x}");
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Python's `'%g' % value`: six significant digits, trailing zeros removed, exponent form
/// below 1e-4 and from 1e6 up.
pub(crate) fn python_g(value: f64) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf" } else { "inf" }.into();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let sci = format!("{value:.5e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if (-4..6).contains(&exp) {
        let decimals = usize::try_from(5i32.saturating_sub(exp)).unwrap_or(0);
        strip_zeros(&format!("{value:.decimals$}"))
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip_zeros(mantissa), exp.unsigned_abs())
    }
}

fn strip_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}
