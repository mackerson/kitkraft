//! The Papyrus log lines a placed reference's attached scripts write (M4.T6), for
//! `probes.toml` rows matched with `papyrus-log` (`kitkraft_game::probes::Matcher`).
//!
//! For each script in the reference's `VMAD`, the compiled script is looked for as a loose
//! `Scripts\<Namespace>\<Name>.pex` next to the plugin (where `build` and `package` put it,
//! `docs/recipes/scripts.md`). Its code is read with `kitkraft-pex`. Each
//! `callstatic Debug Trace` whose text is a string literal, or the backing variable of one
//! of the script's own `String` auto properties, gives one expected line. For a property,
//! the text is the value the reference's `VMAD` sets, else the property's default in the
//! script. Text built at run time (a concatenation, a local) is not predicted; such a call
//! is counted in `unpredicted`. Nothing here runs the game: whether and when the line
//! reaches `Papyrus.0.log` is the session's evidence (grade D until a session shows it).

use std::path::{Path, PathBuf};

use kitkraft_pex::{opcode_name, Function, Object, Pex, Value as PexValue};

use crate::edit::vmad::{self, Vmad};

/// Largest compiled script read (official scripts are far below it).
const MAX_PEX_BYTES: u64 = 4 * 1024 * 1024;

/// One script attached to a reference, and what it writes to the Papyrus log.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Attached {
    /// `Namespace:Name` as stored in `VMAD`.
    pub script: String,
    /// Where the compiled script was looked for (relative: `Scripts\A\B.pex`).
    pub looked_for: String,
    /// Whether it was found and read.
    pub found: bool,
    /// Why it could not be read, when found but unreadable.
    pub error: Option<String>,
    /// Each predicted line: (the text, where it comes from).
    pub traces: Vec<(String, String)>,
    /// `Debug.Trace` calls whose text is built at run time.
    pub unpredicted: usize,
}

/// `A:B` as `Scripts\A\B.pex` under `dir`.
fn loose(dir: &Path, script: &str) -> (PathBuf, String) {
    let parts: Vec<&str> = script.split([':', '\\', '/']).collect();
    let mut path = dir.join("Scripts");
    let mut rel = String::from("Scripts");
    for (i, part) in parts.iter().enumerate() {
        let name = if i.saturating_add(1) == parts.len() {
            format!("{part}.pex")
        } else {
            (*part).to_string()
        };
        path.push(&name);
        rel.push('\\');
        rel.push_str(&name);
    }
    (path, rel)
}

/// The scripts of a reference's `VMAD` payload (`vmad`), looked for next to the plugin in
/// `dir`. A payload that does not decode gives nothing (validate reports it).
pub(super) fn of_reference(dir: &Path, vmad: &[u8]) -> Vec<Attached> {
    let Ok(v) = vmad::decode(vmad) else {
        return Vec::new();
    };
    of_vmad(dir, &v)
}

fn of_vmad(dir: &Path, v: &Vmad) -> Vec<Attached> {
    v.scripts
        .iter()
        .map(|s| {
            let script = s.name_text();
            let (path, looked_for) = loose(dir, &script);
            let mut a = Attached {
                script: script.clone(),
                looked_for,
                found: false,
                error: None,
                traces: Vec::new(),
                unpredicted: 0,
            };
            if !path.is_file() {
                return a;
            }
            a.found = true;
            let pex = super::super::papyrus::read_capped(&path, MAX_PEX_BYTES)
                .map_err(|e| e.to_string())
                .and_then(|b| Pex::parse(&b).map_err(|e| e.to_string()));
            match pex {
                Ok(pex) => {
                    let set = |name: &str| {
                        s.properties.iter().find_map(|p| {
                            let n = String::from_utf8_lossy(&p.name);
                            match &p.value {
                                vmad::Value::String(b) if n.eq_ignore_ascii_case(name) => {
                                    Some(String::from_utf8_lossy(b).into_owned())
                                }
                                _ => None,
                            }
                        })
                    };
                    for o in &pex.objects {
                        traces_of(&pex, o, &set, &mut a);
                    }
                }
                Err(e) => a.error = Some(e),
            }
            a
        })
        .collect()
}

fn functions(o: &Object) -> impl Iterator<Item = &Function> {
    o.states
        .iter()
        .flat_map(|s| s.functions.iter().map(|f| &f.function))
}

/// Records `text` as a predicted line, or as unpredicted when it is empty (an empty
/// expectation would match any log).
fn push(a: &mut Attached, text: String, from: String) {
    if text.trim().is_empty() {
        a.unpredicted = a.unpredicted.saturating_add(1);
    } else {
        a.traces.push((text, from));
    }
}

/// The `Debug.Trace` lines of object `o`: `set` gives the `VMAD` value of a property.
fn traces_of(pex: &Pex, o: &Object, set: &dyn Fn(&str) -> Option<String>, a: &mut Attached) {
    // Backing variable -> (property name, the variable's default) for String auto properties.
    let autos: Vec<(&str, &str, Option<&str>)> = o
        .properties
        .iter()
        .filter(|p| pex.string(p.type_name).eq_ignore_ascii_case("String"))
        .filter_map(|p| {
            let var = pex.string(p.auto_var?);
            let default = o
                .variables
                .iter()
                .find(|v| pex.string(v.name) == var)
                .and_then(|v| match v.value {
                    PexValue::String(i) => Some(pex.string(i)),
                    _ => None,
                });
            Some((var, pex.string(p.name), default))
        })
        .collect();
    for f in functions(o) {
        for ins in &f.instructions {
            if opcode_name(ins.opcode) != Some("callstatic") {
                continue;
            }
            let ident = |n: usize| match ins.args.get(n) {
                Some(PexValue::Identifier(i)) => pex.string(*i),
                _ => "",
            };
            if !ident(0).eq_ignore_ascii_case("Debug") || !ident(1).eq_ignore_ascii_case("Trace") {
                continue;
            }
            let first = ins.varargs.as_ref().and_then(|v| v.first());
            match first {
                Some(PexValue::String(i)) => {
                    let from = format!("a Debug.Trace string literal in {}", a.script);
                    push(a, pex.string(*i).to_string(), from);
                }
                Some(PexValue::Identifier(i)) => {
                    let var = pex.string(*i);
                    match autos.iter().find(|(v, _, _)| *v == var) {
                        Some((_, prop, default)) => match (set(prop), default) {
                            (Some(text), _) => {
                                let from = format!(
                                    "Debug.Trace({prop}) in {}, {prop} as the reference's VMAD sets it",
                                    a.script
                                );
                                push(a, text, from);
                            }
                            (None, Some(d)) => {
                                let from = format!(
                                    "Debug.Trace({prop}) in {}, {prop}'s default in the script (the VMAD does not set it)",
                                    a.script
                                );
                                push(a, (*d).to_string(), from);
                            }
                            (None, None) => a.unpredicted = a.unpredicted.saturating_add(1),
                        },
                        None => a.unpredicted = a.unpredicted.saturating_add(1),
                    }
                }
                _ => a.unpredicted = a.unpredicted.saturating_add(1),
            }
        }
    }
}
