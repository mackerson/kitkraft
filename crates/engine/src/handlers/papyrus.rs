//! `papyrus.disasm` (M2.G6) over `kitcraft-pex`.
//!
//! The text is `pexdis.py`'s format line for line; `data.summary` carries it (without the
//! final newline) so `kit papyrus disasm x.pex` prints exactly what `pexdis.py x.pex` does.

pub(super) mod compile;
mod sources;

use std::fs;
use std::path::Path;

use kitcraft_ba2::Archive;
use kitcraft_pex::{disassemble, format_value, opcode_name, Function, Pex, Value};
use serde_json::{json, Value as Json};

use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

/// Largest script read (64 MiB; the official scripts are far smaller). Checked against the
/// loose file's length or the archive record's declared size before anything is read.
const MAX_SCRIPT_BYTES: u64 = 64 << 20;

/// `papyrus.disasm`.
pub(super) fn disasm(args: &Json) -> Result<Outcome, EngineError> {
    const ID: &str = "papyrus.disasm";
    let pex_arg = str_arg(ID, args, "pex")?;
    let archive = args.get("archive").and_then(Json::as_str);
    let format = args.get("format").and_then(Json::as_str).unwrap_or("text");
    let bytes = match archive {
        None => read_capped(Path::new(pex_arg), MAX_SCRIPT_BYTES)
            .map_err(|e| failed(ID, format!("reading `{pex_arg}`: {e}")))?,
        Some(a) => {
            let (archive_data, mut file) =
                Archive::open(Path::new(a)).map_err(|e| failed(ID, format!("{a}: {e}")))?;
            let index = archive_data
                .find(pex_arg)
                .ok_or_else(|| failed(ID, format!("`{pex_arg}` is not in {a}")))?;
            archive_data
                .extract_vec(&mut file, index, MAX_SCRIPT_BYTES)
                .map_err(|e| failed(ID, format!("{a}: {e}")))?
        }
    };
    let pex = Pex::parse(&bytes).map_err(|e| failed(ID, format!("{pex_arg}: {e}")))?;
    let text = disassemble(&pex);
    let functions: usize = pex.objects.iter().map(count_functions).sum();
    let instructions: usize = pex
        .objects
        .iter()
        .flat_map(all_functions)
        .map(|f| f.instructions.len())
        .sum();
    let structs: usize = pex.objects.iter().map(|o| o.structs.len()).sum();

    let mut data = json!({
        "pex": pex_arg,
        "archive": archive,
        "bytes": bytes.len(),
        "version": format!("{}.{}", pex.header.major, pex.header.minor),
        "game_id": pex.header.game_id,
        "source": pex.header.source,
        "objects": pex.objects.len(),
        "functions": functions,
        "instructions": instructions,
        "summary": text.strip_suffix('\n').unwrap_or(&text),
    });
    if let Some(obj) = data.as_object_mut() {
        if format != "json" {
            obj.insert("text".into(), Json::String(text.clone()));
        }
        if format != "text" {
            obj.insert("script".into(), script_json(&pex));
        }
    }
    let mut limitations = Vec::new();
    if (pex.header.major, pex.header.minor) != (3, 12) {
        limitations.push(format!(
            "version {}.{}: only 3.12 (every official Starfield script) is checked against \
             the corpus; other versions parse by the same layout (guards from 3.12 on) and \
             may be misread",
            pex.header.major, pex.header.minor
        ));
    }
    if structs > 0 {
        limitations.push(format!(
            "{structs} structs are in the model but, as in pexdis.py's format, not in the text"
        ));
    }
    if archive.is_some() {
        limitations.push(
            "the script was read from an archive; offsets are within the .pex file".to_string(),
        );
    }
    Ok(Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence: vec![Evidence {
            file: file_name(Path::new(&pex_arg.replace('\\', "/"))),
            offset: 0,
            length: bytes.len() as u64,
            record: None,
            subrecord: None,
        }],
    })
}

/// Reads a loose file, refusing one longer than `cap` bytes before reading it.
pub(super) fn read_capped(path: &Path, cap: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len > cap {
        return Err(std::io::Error::other(format!(
            "{len} bytes is above the {cap}-byte limit"
        )));
    }
    let mut out = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
    // `take` keeps a file that grows while it is read inside the cap too.
    file.take(cap).read_to_end(&mut out)?;
    Ok(out)
}

fn all_functions(o: &kitcraft_pex::Object) -> Vec<&Function> {
    let mut out: Vec<&Function> = Vec::new();
    for p in &o.properties {
        out.extend(p.getter.iter());
        out.extend(p.setter.iter());
    }
    for s in &o.states {
        out.extend(s.functions.iter().map(|f| &f.function));
    }
    out
}

fn count_functions(o: &kitcraft_pex::Object) -> usize {
    all_functions(o).len()
}

fn value_json(pex: &Pex, v: &Value) -> Json {
    let kind = match v {
        Value::None => "none",
        Value::Identifier(_) => "identifier",
        Value::String(_) => "string",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Bool(_) => "bool",
    };
    let value = match v {
        Value::None => Json::Null,
        Value::Identifier(i) | Value::String(i) => Json::String(pex.string(*i).to_string()),
        Value::Int(n) => json!(n),
        Value::Float(f) => json!(f),
        Value::Bool(b) => json!(*b != 0),
    };
    json!({"type": kind, "value": value, "text": format_value(pex, v)})
}

fn function_json(pex: &Pex, name: &str, f: &Function) -> Json {
    let s = |i: u16| pex.string(i).to_string();
    json!({
        "name": name,
        "offset": f.offset,
        "return_type": s(f.return_type),
        "doc": s(f.doc),
        "user_flags": f.user_flags,
        "flags": f.flags,
        "params": f.params.iter().map(|(n, t)| json!({"name": s(*n), "type": s(*t)})).collect::<Vec<_>>(),
        "locals": f.locals.iter().map(|(n, t)| json!({"name": s(*n), "type": s(*t)})).collect::<Vec<_>>(),
        "instructions": f.instructions.iter().map(|ins| json!({
            "offset": ins.offset,
            "opcode": ins.opcode,
            "op": opcode_name(ins.opcode),
            "args": ins.args.iter().map(|v| value_json(pex, v)).collect::<Vec<_>>(),
            "varargs": ins.varargs.as_ref().map(|vs| vs.iter().map(|v| value_json(pex, v)).collect::<Vec<_>>()),
        })).collect::<Vec<_>>(),
    })
}

fn script_json(pex: &Pex) -> Json {
    let s = |i: u16| pex.string(i).to_string();
    let h = &pex.header;
    let objects: Vec<Json> = pex
        .objects
        .iter()
        .map(|o| {
            let mut functions = Vec::new();
            for p in &o.properties {
                let name = s(p.name);
                if let Some(f) = &p.getter {
                    functions.push(function_json(pex, &format!("{name}.get"), f));
                }
                if let Some(f) = &p.setter {
                    functions.push(function_json(pex, &format!("{name}.set"), f));
                }
            }
            let states: Vec<Json> = o
                .states
                .iter()
                .map(|st| {
                    json!({
                        "name": s(st.name),
                        "offset": st.offset,
                        "functions": st.functions.iter()
                            .map(|f| function_json(pex, &s(f.name), &f.function))
                            .collect::<Vec<_>>(),
                    })
                })
                .collect();
            json!({
                "name": s(o.name),
                "offset": o.offset,
                "size": o.size,
                "parent": s(o.parent),
                "doc": s(o.doc),
                "const": o.const_flag != 0,
                "user_flags": o.user_flags,
                "auto_state": s(o.auto_state),
                "structs": o.structs.iter().map(|st| json!({
                    "name": s(st.name),
                    "offset": st.offset,
                    "members": st.members.iter().map(|m| json!({
                        "name": s(m.name),
                        "type": s(m.type_name),
                        "user_flags": m.user_flags,
                        "value": value_json(pex, &m.value),
                        "const": m.const_flag != 0,
                        "doc": s(m.doc),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "variables": o.variables.iter().map(|v| json!({
                    "name": s(v.name),
                    "type": s(v.type_name),
                    "offset": v.offset,
                    "user_flags": v.user_flags,
                    "value": value_json(pex, &v.value),
                    "const": v.const_flag != 0,
                })).collect::<Vec<_>>(),
                "guards": o.guards.iter().map(|g| s(*g)).collect::<Vec<_>>(),
                "properties": o.properties.iter().map(|p| json!({
                    "name": s(p.name),
                    "type": s(p.type_name),
                    "offset": p.offset,
                    "doc": s(p.doc),
                    "user_flags": p.user_flags,
                    "flags": p.flags,
                    "auto_var": p.auto_var.map(s),
                })).collect::<Vec<_>>(),
                "property_functions": functions,
                "states": states,
            })
        })
        .collect();
    json!({
        "header": {
            "magic": format!("0x{:08X}", h.magic),
            "major": h.major,
            "minor": h.minor,
            "game_id": h.game_id,
            "compile_time": h.compile_time,
            "source": h.source,
            "user": h.user,
            "machine": h.machine,
        },
        "strings": pex.strings.len(),
        "strings_offset": pex.strings_offset,
        "debug": pex.debug.as_ref().map(|d| json!({
            "offset": d.offset,
            "modification_time": d.modification_time,
            "functions": d.functions.len(),
            "property_groups": d.property_groups.len(),
            "struct_orders": d.struct_orders.len(),
        })),
        "user_flags": pex.user_flags.iter().map(|u| json!({"name": s(u.name), "bit": u.bit})).collect::<Vec<_>>(),
        "objects": objects,
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
pub(super) mod tests {
    use crate::execute;
    use serde_json::json;
    use std::fs;

    /// A minimal script: `Tiny extends ScriptObject` with `OnInit()` = `return None`.
    pub(crate) fn tiny_pex() -> Vec<u8> {
        let mut b = Vec::new();
        let ws = |b: &mut Vec<u8>, s: &str| {
            b.extend((s.len() as u16).to_le_bytes());
            b.extend(s.as_bytes());
        };
        b.extend(0xFA57_C0DEu32.to_le_bytes());
        b.extend([3u8, 12]);
        b.extend(2u16.to_le_bytes());
        b.extend(0u64.to_le_bytes());
        ws(&mut b, "Tiny.psc");
        ws(&mut b, "u");
        ws(&mut b, "m");
        let strings = ["", "Tiny", "ScriptObject", "OnInit", "None"];
        b.extend((strings.len() as u16).to_le_bytes());
        for s in strings {
            ws(&mut b, s);
        }
        b.push(0); // no debug info
        b.extend(0u16.to_le_bytes()); // user flags
        b.extend(1u16.to_le_bytes()); // objects
        let mut o = Vec::new();
        for v in [2u16, 0] {
            o.extend(v.to_le_bytes()); // parent, doc
        }
        o.push(0);
        o.extend(0u32.to_le_bytes());
        o.extend(0u16.to_le_bytes()); // auto state
        for _ in 0..4 {
            o.extend(0u16.to_le_bytes()); // structs, variables, guards, properties
        }
        o.extend(1u16.to_le_bytes()); // states
        o.extend(0u16.to_le_bytes()); // default state
        o.extend(1u16.to_le_bytes()); // one function
        o.extend(3u16.to_le_bytes()); // OnInit
        o.extend(4u16.to_le_bytes()); // returns None
        o.extend(0u16.to_le_bytes());
        o.extend(0u32.to_le_bytes());
        o.push(0);
        o.extend(0u16.to_le_bytes()); // params
        o.extend(0u16.to_le_bytes()); // locals
        o.extend(1u16.to_le_bytes()); // one instruction
        o.extend([26u8, 0]); // return None
        b.extend(1u16.to_le_bytes()); // object name
        b.extend((o.len() as u32 + 4).to_le_bytes());
        b.extend(o);
        b
    }

    #[test]
    fn disasm_through_execute_loose_and_in_archive() {
        let dir = std::env::temp_dir().join(format!("kitcraft-pex-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src").join("scripts")).unwrap();
        let pex = dir.join("src").join("scripts").join("tiny.pex");
        fs::write(&pex, tiny_pex()).unwrap();
        let want = "Tiny.psc v3.12 gid2\nOBJECT Tiny extends ScriptObject\n  FUNC OnInit()\n      0 return None\n";

        let r = execute("papyrus.disasm", json!({"pex": pex.to_str().unwrap()})).unwrap();
        assert_eq!(r["data"]["text"], want);
        assert_eq!(r["data"]["summary"], want.trim_end());
        assert_eq!(r["data"]["functions"], 1);
        assert!(r["data"].get("script").is_none());
        assert_eq!(r["limitations"].as_array().unwrap().len(), 0);
        // Regression (review round 2): a version other than 3.12 is flagged.
        let mut other = tiny_pex();
        other[5] = 13;
        fs::write(&pex, &other).unwrap();
        let r = execute("papyrus.disasm", json!({"pex": pex.to_str().unwrap()})).unwrap();
        assert!(
            r["limitations"][0]
                .as_str()
                .unwrap()
                .contains("version 3.13"),
            "{r}"
        );
        fs::write(&pex, tiny_pex()).unwrap();
        let j = execute(
            "papyrus.disasm",
            json!({"pex": pex.to_str().unwrap(), "format": "json"}),
        )
        .unwrap();
        assert!(j["data"].get("text").is_none());
        let f = &j["data"]["script"]["objects"][0]["states"][0]["functions"][0];
        assert_eq!(f["name"], "OnInit");
        assert_eq!(f["instructions"][0]["op"], "return");
        assert_eq!(f["instructions"][0]["args"][0]["type"], "none");

        let archive = dir.join("a.ba2");
        execute(
            "ba2.pack",
            json!({"dir": dir.join("src").to_str().unwrap(), "out": archive.to_str().unwrap()}),
        )
        .unwrap();
        let a = execute(
            "papyrus.disasm",
            json!({"pex": "Scripts\\Tiny.pex", "archive": archive.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(a["data"]["text"], want);

        fs::write(&pex, &tiny_pex()[..20]).unwrap();
        let err = execute("papyrus.disasm", json!({"pex": pex.to_str().unwrap()})).unwrap_err();
        assert!(err.to_string().contains("offset"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Regression (review round 2): a script whose record declares 512 MiB (a zlib bomb
    /// from a small archive) is refused by the 64 MiB cap before it is decoded; the same
    /// archive's string table is refused by the 256 MiB cap.
    #[test]
    fn oversized_entries_are_refused_before_decoding() {
        let dir = std::env::temp_dir().join(format!("kitcraft-pexcap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src").join("scripts")).unwrap();
        fs::write(
            dir.join("src").join("scripts").join("a.pex"),
            vec![7u8; 4000],
        )
        .unwrap();
        fs::create_dir_all(dir.join("src").join("strings")).unwrap();
        fs::write(
            dir.join("src").join("strings").join("x_en.strings"),
            vec![7u8; 4000],
        )
        .unwrap();
        let archive = dir.join("a.ba2");
        execute(
            "ba2.pack",
            json!({"dir": dir.join("src").to_str().unwrap(), "out": archive.to_str().unwrap()}),
        )
        .unwrap();
        // GNRL record at 32: hashes and ext (12), flags (4), offset (8), packed (4), size.
        let mut bytes = fs::read(&archive).unwrap();
        for size_at in [60, 60 + 36] {
            bytes[size_at..size_at + 4].copy_from_slice(&(512u32 << 20).to_le_bytes());
        }
        fs::write(&archive, &bytes).unwrap();
        let err = execute(
            "papyrus.disasm",
            json!({"pex": "scripts/a.pex", "archive": archive.to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("above the cap"), "{err}");
        let err = execute(
            "strings.dump",
            json!({"table": "strings/x_en.strings", "archive": archive.to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("above the cap"), "{err}");

        let small = dir.join("small.bin");
        fs::write(&small, b"abcd").unwrap();
        assert!(super::read_capped(&small, 3).is_err());
        assert_eq!(super::read_capped(&small, 4).unwrap(), b"abcd");
        fs::remove_dir_all(&dir).unwrap();
    }
}
