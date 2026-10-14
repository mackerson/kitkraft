//! `papyrus.denylisted-call` (M2.E8): calls in compiled scripts to natives on a denylist.
//!
//! The denylist is seeded from our own disassembly (`pexdis.py`, `kitcraft-pex`) of
//! `C:\Modding\scripts\noc_godmode_patch` (our god-mode patch for NPC Outfit Changer),
//! recorded in `docs/validate.md`: `Debug.SetGodMode` is the call that leaves the player
//! invulnerable when a script stops between turning it on and off. Calls are matched as
//! `callstatic <class> <function>` (operands 0 and 1) and `callmethod <function> <object>`
//! (operand 0), names ASCII case-insensitive.

use std::path::{Path, PathBuf};

use kitcraft_pex::{format_value, opcode_name, Function, Pex, Value};

use super::{Finding, Report, Severity};

/// `(class, function, why)` entries; the class is empty for a method call on any object.
pub const DENYLIST: &[(&str, &str, &str)] = &[(
    "debug",
    "setgodmode",
    "toggles god mode; if the script stops between turning it on and off (fast travel, a load), the player stays invulnerable in the save",
)];

/// Scans the `.pex` files at `paths` (files, or directories searched recursively, at most
/// 16 levels and 100,000 files).
pub fn denylisted_calls(paths: &[PathBuf], report: &mut Report) -> Result<(), String> {
    let mut files = Vec::new();
    for path in paths {
        collect(path, 0, &mut files)?;
    }
    for file in files {
        let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let pex = Pex::parse(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        report.scripts = report.scripts.saturating_add(1);
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        scan(&pex, &name, &mut report.findings);
    }
    Ok(())
}

fn collect(path: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if out.len() >= 100_000 || depth > 16 {
        return Ok(());
    }
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                collect(&entry, depth.saturating_add(1), out)?;
            } else if entry
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pex"))
            {
                out.push(entry);
            }
        }
        Ok(())
    } else if path.is_file() {
        out.push(path.to_path_buf());
        Ok(())
    } else {
        Err(format!(
            "{}: no such script file or directory",
            path.display()
        ))
    }
}

/// Findings for one parsed script.
pub fn scan(pex: &Pex, file: &str, findings: &mut Vec<Finding>) {
    for object in &pex.objects {
        let object_name = pex.string(object.name);
        let mut functions: Vec<(String, &Function)> = Vec::new();
        for state in &object.states {
            for f in &state.functions {
                let state_name = pex.string(state.name);
                let label = if state_name.is_empty() {
                    format!("{object_name}.{}", pex.string(f.name))
                } else {
                    format!("{object_name}.{state_name}.{}", pex.string(f.name))
                };
                functions.push((label, &f.function));
            }
        }
        for p in &object.properties {
            for (kind, f) in [("get", &p.getter), ("set", &p.setter)] {
                if let Some(f) = f {
                    functions.push((format!("{object_name}.{}.{kind}", pex.string(p.name)), f));
                }
            }
        }
        for (label, function) in functions {
            for ins in &function.instructions {
                let op = opcode_name(ins.opcode).unwrap_or_default();
                let ident = |i: usize| match ins.args.get(i) {
                    Some(Value::Identifier(s)) => pex.string(*s).to_ascii_lowercase(),
                    _ => String::new(),
                };
                let (class, name) = match op {
                    "callstatic" => (ident(0), ident(1)),
                    "callmethod" => (String::new(), ident(0)),
                    _ => continue,
                };
                let Some((c, f, why)) = DENYLIST.iter().find(|(c, f, _)| {
                    *f == name && (c.is_empty() || *c == class || class.is_empty())
                }) else {
                    continue;
                };
                let args: Vec<String> = ins
                    .varargs
                    .iter()
                    .flatten()
                    .map(|v| format_value(pex, v))
                    .collect();
                findings.push(Finding {
                    rule: "papyrus.denylisted-call",
                    severity: Severity::Warning,
                    plugin: file.to_string(),
                    record: Some(label.clone()),
                    subrecord: None,
                    offset: Some(ins.offset),
                    message: format!("{op} {c}.{f}({}): {why}", args.join(", ")),
                });
            }
        }
    }
}
