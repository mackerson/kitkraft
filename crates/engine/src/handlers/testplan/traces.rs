//! The Papyrus log lines a placed reference's attached scripts write (M4.T6), for
//! `probes.toml` rows matched with `papyrus-log` (`kitkraft_game::probes::Matcher`).
//!
//! For each script in the reference's `VMAD`, and in its base form's when the base is
//! scripted, the compiled script is looked for as a loose `Scripts\<Namespace>\<Name>.pex`
//! next to the plugin (where `build` and `package` put it, `docs/recipes/scripts.md`). Its
//! code is read with `kitkraft-pex`. A `callstatic Debug Trace` gives an expected line
//! only when it certainly runs when the test plan enters the cell:
//!
//! - its function is one of [`EVENTS`] (`OnLoad`, `OnInit`), the events entering the cell
//!   fires on a new reference;
//! - the function belongs to the state the script starts in (its auto state when that state
//!   defines the event, else the default state);
//! - the call comes before the function's first jump or return (the straight-line start of
//!   the handler, not inside an If, While or Else);
//! - its text is a string literal, or the backing variable of one of the script's `String`
//!   auto properties: the value the `VMAD` sets, else the property's default in the script.
//!
//! Every other Trace call is listed in `unpredicted` with the reason ("after a branch in
//! OnLoad", "in OnActivate", "in state Sleeping", "text built at run time"). Nothing here
//! runs the game: whether and when the line reaches `Papyrus.0.log` is the session's
//! evidence (grade D until a session shows it).

use std::path::{Path, PathBuf};

use kitkraft_pex::{opcode_name, Object, Pex, Value as PexValue};

use crate::edit::vmad::{self, Vmad};

/// Largest compiled script read (official scripts are far below it).
const MAX_PEX_BYTES: u64 = 4 * 1024 * 1024;

/// The events a test plan's rows make the game send a newly placed reference: entering its
/// cell initializes and loads it.
pub(super) const EVENTS: [&str; 2] = ["OnLoad", "OnInit"];

/// One script attached to a reference (or to its base), and what it writes to the Papyrus
/// log.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Attached {
    /// `Namespace:Name` as stored in `VMAD`.
    pub script: String,
    /// Whether the script is on the reference's base form, not on the reference.
    pub on_base: bool,
    /// Where the compiled script was looked for (relative: `Scripts\A\B.pex`); empty when
    /// the name is not a script name.
    pub looked_for: String,
    /// Whether it was found and read.
    pub found: bool,
    /// Why it could not be looked up or read.
    pub error: Option<String>,
    /// Each predicted line: (the text, where it comes from).
    pub traces: Vec<(String, String)>,
    /// Each `Debug.Trace` call not predicted, with the reason.
    pub unpredicted: Vec<String>,
}

/// `A:B` as `Scripts\A\B.pex` under `dir`, or `None` when a part of the name is empty, `.`
/// or `..`, or holds anything but ASCII letters, digits and `_` (a `VMAD` from the internet
/// may name anything; the lookup stays inside the plugin's folder).
pub(super) fn loose(dir: &Path, script: &str) -> Option<(PathBuf, String)> {
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
    Some((path, rel))
}

/// The scripts of a `VMAD` payload (`vmad`), looked for next to the plugin in `dir`;
/// `on_base` when the payload is the base form's. A payload that does not decode gives
/// nothing (validate reports it).
pub(super) fn of_vmad_bytes(dir: &Path, vmad: &[u8], on_base: bool) -> Vec<Attached> {
    let Ok(v) = vmad::decode(vmad) else {
        return Vec::new();
    };
    of_vmad(dir, &v, on_base)
}

fn of_vmad(dir: &Path, v: &Vmad, on_base: bool) -> Vec<Attached> {
    v.scripts
        .iter()
        .map(|s| {
            let script = s.name_text();
            let mut a = Attached {
                script: script.clone(),
                on_base,
                looked_for: String::new(),
                found: false,
                error: None,
                traces: Vec::new(),
                unpredicted: Vec::new(),
            };
            let Some((path, looked_for)) = loose(dir, &script) else {
                a.error = Some("not a script name (a part is empty, `.`, `..` or not a plain identifier), so no file is looked up".into());
                return a;
            };
            a.looked_for = looked_for;
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

/// Records `text` as a predicted line, or as unpredicted when it is empty (an empty
/// expectation would match any log).
fn push(a: &mut Attached, text: String, from: String) {
    if text.trim().is_empty() {
        a.unpredicted.push(format!("{from}: an empty text"));
    } else {
        a.traces.push((text, from));
    }
}

fn is_jump(op: u8) -> bool {
    matches!(opcode_name(op), Some("jmp" | "jmpf" | "jmpt" | "return"))
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
    let auto_state = pex.string(o.auto_state);
    // The state the script starts in, for an event: its auto state when that state defines
    // the event, else the default (empty-named) state.
    let starts_in = |event: &str| -> &str {
        let defines = |state: &str| {
            o.states.iter().any(|s| {
                pex.string(s.name).eq_ignore_ascii_case(state)
                    && s.functions
                        .iter()
                        .any(|f| pex.string(f.name).eq_ignore_ascii_case(event))
            })
        };
        if !auto_state.is_empty() && defines(auto_state) {
            auto_state
        } else {
            ""
        }
    };
    for state in &o.states {
        let sname = pex.string(state.name);
        for named in &state.functions {
            let event = pex.string(named.name);
            let mut branched = false;
            for ins in &named.function.instructions {
                if is_jump(ins.opcode) {
                    branched = true;
                    continue;
                }
                if opcode_name(ins.opcode) != Some("callstatic") {
                    continue;
                }
                let ident = |n: usize| match ins.args.get(n) {
                    Some(PexValue::Identifier(i)) => pex.string(*i),
                    _ => "",
                };
                if !ident(0).eq_ignore_ascii_case("Debug")
                    || !ident(1).eq_ignore_ascii_case("Trace")
                {
                    continue;
                }
                let at = if sname.is_empty() {
                    event.to_string()
                } else {
                    format!("{event} of state {sname}")
                };
                if !EVENTS.iter().any(|e| e.eq_ignore_ascii_case(event)) {
                    a.unpredicted.push(format!(
                        "a Trace in {at}: entering the cell sends {} only",
                        EVENTS.join(" and ")
                    ));
                    continue;
                }
                if !sname.eq_ignore_ascii_case(starts_in(event)) {
                    a.unpredicted.push(format!(
                        "a Trace in {event} of state {}: not the state the script starts in",
                        if sname.is_empty() { "(default)" } else { sname }
                    ));
                    continue;
                }
                if branched {
                    a.unpredicted.push(format!(
                        "a Trace after a branch in {at} (inside an If, While or Else, or after a return): it may not run"
                    ));
                    continue;
                }
                let on = if a.on_base {
                    format!("{} on the base form", a.script)
                } else {
                    a.script.clone()
                };
                match ins.varargs.as_ref().and_then(|v| v.first()) {
                    Some(PexValue::String(i)) => {
                        let from =
                            format!("a Debug.Trace string literal at the start of {at} in {on}");
                        push(a, pex.string(*i).to_string(), from);
                    }
                    Some(PexValue::Identifier(i)) => {
                        let var = pex.string(*i);
                        match autos.iter().find(|(v, _, _)| *v == var) {
                            Some((_, prop, default)) => match (set(prop), default) {
                                (Some(text), _) => {
                                    let from = format!(
                                        "Debug.Trace({prop}) at the start of {at} in {on}, {prop} as the VMAD sets it"
                                    );
                                    push(a, text, from);
                                }
                                (None, Some(d)) => {
                                    let from = format!(
                                        "Debug.Trace({prop}) at the start of {at} in {on}, {prop}'s default in the script (the VMAD does not set it)"
                                    );
                                    push(a, (*d).to_string(), from);
                                }
                                (None, None) => a.unpredicted.push(format!(
                                    "a Trace of {prop} in {at}: no value in the VMAD or the script"
                                )),
                            },
                            None => a
                                .unpredicted
                                .push(format!("a Trace in {at}: text built at run time")),
                        }
                    }
                    _ => a
                        .unpredicted
                        .push(format!("a Trace in {at}: text built at run time")),
                }
            }
        }
    }
}
