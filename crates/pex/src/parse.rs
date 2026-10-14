//! Parsing. Every read is bounds-checked; every string index is checked against the
//! string table; counts are capped by the bytes left before anything is allocated.

use crate::model::{
    DebugFunction, DebugInfo, Function, Header, Instruction, Member, NamedFunction, Object, Pex,
    Property, PropertyGroup, State, Struct, StructOrder, UserFlag, Value, Variable,
};
use crate::text::OPCODES;

/// The magic number as a little-endian `u32` (`corpus:` every official script).
pub const MAGIC: u32 = 0xFA57_C0DE;

/// Parse errors, each with the byte offset where it was found.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The data ends inside a field.
    #[error("truncated at offset {offset}: needed {needed} bytes, {available} available")]
    Truncated {
        /// Field offset.
        offset: u64,
        /// Bytes needed.
        needed: u64,
        /// Bytes left.
        available: u64,
    },
    /// The first four bytes are not the little-endian magic.
    #[error("not a little-endian .pex: magic {found:#010X} at offset 0")]
    BadMagic {
        /// The value read.
        found: u32,
    },
    /// A string index past the string table.
    #[error("string index {index} at offset {offset} is past the string table ({count} strings)")]
    BadString {
        /// Field offset.
        offset: u64,
        /// The index.
        index: u16,
        /// Strings in the table.
        count: usize,
    },
    /// A value type byte other than 0..=5.
    #[error("bad value type {found} at offset {offset}")]
    BadValueType {
        /// Offset of the type byte.
        offset: u64,
        /// The byte.
        found: u8,
    },
    /// An opcode this disassembler does not know.
    #[error("unknown opcode {opcode} at offset {offset}")]
    UnknownOpcode {
        /// Offset of the opcode byte.
        offset: u64,
        /// The opcode.
        opcode: u8,
    },
    /// The operand count of a call or guard instruction is not a non-negative integer.
    #[error("operand count at offset {offset} is not a non-negative integer")]
    BadOperandCount {
        /// Offset of the count value.
        offset: u64,
    },
    /// An object's size field disagrees with the bytes its body occupies (counted from
    /// the size field itself to the end of the object, `corpus:` every official script).
    #[error("object at offset {offset} declares {declared} bytes from its size field, its body has {actual}")]
    ObjectSize {
        /// Offset of the object (its name index).
        offset: u64,
        /// The size field.
        declared: u32,
        /// Bytes actually read.
        actual: u64,
    },
    /// Bytes left after the last object.
    #[error("{count} unread bytes after the last object, at offset {offset}")]
    TrailingBytes {
        /// Where they start.
        offset: u64,
        /// How many.
        count: u64,
    },
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    strings: usize,
}

impl<'a> Reader<'a> {
    fn offset(&self) -> u64 {
        self.pos as u64
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n);
        match end.and_then(|end| self.data.get(self.pos..end)) {
            Some(bytes) => {
                self.pos = self.pos.saturating_add(n);
                Ok(bytes)
            }
            None => Err(Error::Truncated {
                offset: self.offset(),
                needed: n as u64,
                available: self.remaining() as u64,
            }),
        }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let bytes = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    /// A `u16`-length string, one char per byte (as `pexdis.py` decodes Latin-1).
    fn wstring(&mut self) -> Result<String, Error> {
        let n = self.u16()?;
        let bytes = self.take(usize::from(n))?;
        Ok(bytes.iter().map(|&b| char::from(b)).collect())
    }

    /// A string-table index, checked.
    fn index(&mut self) -> Result<u16, Error> {
        let offset = self.offset();
        let index = self.u16()?;
        if usize::from(index) >= self.strings {
            return Err(Error::BadString {
                offset,
                index,
                count: self.strings,
            });
        }
        Ok(index)
    }

    /// A count, and a vector with capacity capped by the bytes left (each element takes at
    /// least `min_len` bytes).
    fn count<T>(&mut self, min_len: usize) -> Result<(u16, Vec<T>), Error> {
        let n = self.u16()?;
        let cap = usize::from(n).min(self.remaining().checked_div(min_len).unwrap_or(0));
        Ok((n, Vec::with_capacity(cap)))
    }

    fn value(&mut self) -> Result<Value, Error> {
        let offset = self.offset();
        Ok(match self.u8()? {
            0 => Value::None,
            1 => Value::Identifier(self.index()?),
            2 => Value::String(self.index()?),
            3 => Value::Int(self.i32()?),
            4 => Value::Float(self.f32()?),
            5 => Value::Bool(self.u8()?),
            found => return Err(Error::BadValueType { offset, found }),
        })
    }

    fn pair(&mut self) -> Result<(u16, u16), Error> {
        Ok((self.index()?, self.index()?))
    }

    fn indices(&mut self) -> Result<Vec<u16>, Error> {
        let (n, mut out) = self.count(2)?;
        for _ in 0..n {
            out.push(self.index()?);
        }
        Ok(out)
    }
}

impl Pex {
    /// Parses a whole `.pex` file.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut r = Reader {
            data,
            pos: 0,
            strings: 0,
        };
        let magic = r.u32()?;
        if magic != MAGIC {
            return Err(Error::BadMagic { found: magic });
        }
        let header = Header {
            magic,
            major: r.u8()?,
            minor: r.u8()?,
            game_id: r.u16()?,
            compile_time: r.u64()?,
            source: r.wstring()?,
            user: r.wstring()?,
            machine: r.wstring()?,
        };
        let strings_offset = r.offset();
        let (n, mut strings) = r.count(2)?;
        for _ in 0..n {
            strings.push(r.wstring()?);
        }
        r.strings = strings.len();

        let debug_offset = r.offset();
        let debug = if r.u8()? != 0 {
            Some(read_debug(&mut r, debug_offset)?)
        } else {
            None
        };

        let (n, mut user_flags) = r.count(3)?;
        for _ in 0..n {
            user_flags.push(UserFlag {
                name: r.index()?,
                bit: r.u8()?,
            });
        }

        let guards = (header.major, header.minor) >= (3, 12);
        let (n, mut objects) = r.count(17)?;
        for _ in 0..n {
            objects.push(read_object(&mut r, guards)?);
        }
        if r.remaining() != 0 {
            return Err(Error::TrailingBytes {
                offset: r.offset(),
                count: r.remaining() as u64,
            });
        }
        Ok(Pex {
            header,
            strings,
            strings_offset,
            debug,
            user_flags,
            objects,
        })
    }
}

fn read_debug(r: &mut Reader<'_>, offset: u64) -> Result<DebugInfo, Error> {
    let modification_time = r.u64()?;
    let (n, mut functions) = r.count(9)?;
    for _ in 0..n {
        let object = r.index()?;
        let state = r.index()?;
        let function = r.index()?;
        let kind = r.u8()?;
        let (lines_n, mut lines) = r.count(2)?;
        for _ in 0..lines_n {
            lines.push(r.u16()?);
        }
        functions.push(DebugFunction {
            object,
            state,
            function,
            kind,
            lines,
        });
    }
    let (n, mut property_groups) = r.count(12)?;
    for _ in 0..n {
        property_groups.push(PropertyGroup {
            object: r.index()?,
            name: r.index()?,
            doc: r.index()?,
            user_flags: r.u32()?,
            properties: r.indices()?,
        });
    }
    let (n, mut struct_orders) = r.count(6)?;
    for _ in 0..n {
        struct_orders.push(StructOrder {
            object: r.index()?,
            name: r.index()?,
            members: r.indices()?,
        });
    }
    Ok(DebugInfo {
        offset,
        modification_time,
        functions,
        property_groups,
        struct_orders,
    })
}

fn read_object(r: &mut Reader<'_>, guards: bool) -> Result<Object, Error> {
    let offset = r.offset();
    let name = r.index()?;
    let size_at = r.offset();
    let size = r.u32()?;
    let parent = r.index()?;
    let doc = r.index()?;
    let const_flag = r.u8()?;
    let user_flags = r.u32()?;
    let auto_state = r.index()?;

    let (n, mut structs) = r.count(4)?;
    for _ in 0..n {
        let struct_offset = r.offset();
        let struct_name = r.index()?;
        let (m, mut members) = r.count(14)?;
        for _ in 0..m {
            members.push(Member {
                name: r.index()?,
                type_name: r.index()?,
                user_flags: r.u32()?,
                value: r.value()?,
                const_flag: r.u8()?,
                doc: r.index()?,
            });
        }
        structs.push(Struct {
            offset: struct_offset,
            name: struct_name,
            members,
        });
    }

    let (n, mut variables) = r.count(10)?;
    for _ in 0..n {
        variables.push(Variable {
            offset: r.offset(),
            name: r.index()?,
            type_name: r.index()?,
            user_flags: r.u32()?,
            value: r.value()?,
            const_flag: r.u8()?,
        });
    }

    let guard_names = if guards { r.indices()? } else { Vec::new() };

    let (n, mut properties) = r.count(11)?;
    for _ in 0..n {
        let prop_offset = r.offset();
        let prop_name = r.index()?;
        let type_name = r.index()?;
        let prop_doc = r.index()?;
        let prop_flags_user = r.u32()?;
        let flags = r.u8()?;
        let (auto_var, getter, setter) = if flags & 4 != 0 {
            (Some(r.index()?), None, None)
        } else {
            let getter = if flags & 1 != 0 {
                Some(read_function(r)?)
            } else {
                None
            };
            let setter = if flags & 2 != 0 {
                Some(read_function(r)?)
            } else {
                None
            };
            (None, getter, setter)
        };
        properties.push(Property {
            offset: prop_offset,
            name: prop_name,
            type_name,
            doc: prop_doc,
            user_flags: prop_flags_user,
            flags,
            auto_var,
            getter,
            setter,
        });
    }

    let (n, mut states) = r.count(4)?;
    for _ in 0..n {
        let state_offset = r.offset();
        let state_name = r.index()?;
        let (m, mut functions) = r.count(16)?;
        for _ in 0..m {
            let fn_name = r.index()?;
            functions.push(NamedFunction {
                name: fn_name,
                function: read_function(r)?,
            });
        }
        states.push(State {
            offset: state_offset,
            name: state_name,
            functions,
        });
    }

    let actual = r.offset().saturating_sub(size_at);
    if actual != u64::from(size) {
        return Err(Error::ObjectSize {
            offset,
            declared: size,
            actual,
        });
    }
    Ok(Object {
        offset,
        name,
        size,
        parent,
        doc,
        const_flag,
        user_flags,
        auto_state,
        structs,
        variables,
        guards: guard_names,
        properties,
        states,
    })
}

fn read_function(r: &mut Reader<'_>) -> Result<Function, Error> {
    let offset = r.offset();
    let return_type = r.index()?;
    let doc = r.index()?;
    let user_flags = r.u32()?;
    let flags = r.u8()?;
    let (n, mut params) = r.count(4)?;
    for _ in 0..n {
        params.push(r.pair()?);
    }
    let (n, mut locals) = r.count(4)?;
    for _ in 0..n {
        locals.push(r.pair()?);
    }
    let (n, mut instructions) = r.count(1)?;
    for _ in 0..n {
        let op_offset = r.offset();
        let opcode = r.u8()?;
        let (_, fixed, varargs) =
            OPCODES
                .get(usize::from(opcode))
                .copied()
                .ok_or(Error::UnknownOpcode {
                    offset: op_offset,
                    opcode,
                })?;
        let mut args = Vec::with_capacity(usize::from(fixed));
        for _ in 0..fixed {
            args.push(r.value()?);
        }
        let extra = if varargs {
            let count_offset = r.offset();
            let count = match r.value()? {
                Value::Int(n) => usize::try_from(n).map_err(|_| Error::BadOperandCount {
                    offset: count_offset,
                })?,
                _ => {
                    return Err(Error::BadOperandCount {
                        offset: count_offset,
                    })
                }
            };
            let mut extra = Vec::with_capacity(count.min(r.remaining()));
            for _ in 0..count {
                extra.push(r.value()?);
            }
            Some(extra)
        } else {
            None
        };
        instructions.push(Instruction {
            offset: op_offset,
            opcode,
            args,
            varargs: extra,
        });
    }
    Ok(Function {
        offset,
        return_type,
        doc,
        user_flags,
        flags,
        params,
        locals,
        instructions,
    })
}
