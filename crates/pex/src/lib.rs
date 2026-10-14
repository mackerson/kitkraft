//! Compiled Papyrus (`.pex`, Starfield v3.12) reading and disassembly.
//!
//! Port of `C:\Modding\scripts\pexdis.py` (our own code; ADR 0006). [`Pex::parse`] builds a
//! structured model with byte offsets; [`disassemble`] prints it in `pexdis.py`'s text
//! format, line for line.
//!
//! Layout (`ours: pexdis.py`, checked on every script in the official archives, `corpus:`):
//! little-endian; header (magic `u32`, major `u8`, minor `u8`, game id `u16`, compile time
//! `u64`, source / user / machine names); the string table (`u16` count of `u16`-length
//! strings, decoded one byte per char as `pexdis.py` does); optional debug info (function
//! line tables, property groups, struct member orders); user flags; objects, each with
//! structs, variables, guards (v3.12), properties (auto, or get/set functions) and states
//! of named functions. Instructions are an opcode and a fixed number of values, plus a
//! counted list for the call and guard opcodes. Each object's `u32` size counts the bytes
//! from that size field to the object's end (`corpus:` every official script); a mismatch
//! is [`Error::ObjectSize`]. Only version 3.12 occurs in the corpus; the engine flags any
//! other version in `limitations[]`.
//!
//! One correction to `pexdis.py`: it reads an extra `u16` after each struct's name, so it
//! fails on the 133 official scripts that define structs. Without it all 5,708 official
//! scripts parse to their last byte (`corpus:`, `plan/STATUS.md`); structs follow the
//! corpus. `pexdis.py` prints nothing for structs, and neither does [`disassemble`].
//!
//! Assembly comes later; compilation wraps Bethesda's compiler (engine `papyrus.compile`)
//! and never passes `-release`.
//!
//! LAYER: L0

mod model;
mod parse;
mod text;

pub use model::{
    DebugFunction, DebugInfo, Function, Header, Instruction, Member, NamedFunction, Object, Pex,
    Property, PropertyGroup, State, Struct, StructOrder, UserFlag, Value, Variable,
};
pub use parse::Error;
pub use text::{disassemble, format_value, opcode_name, OPCODES};

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
mod tests;
