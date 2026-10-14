//! `papyrus.denylisted-call` (M2.E8): calls in compiled scripts to natives on a denylist.
//!
//! The denylist is seeded from our own disassembly (`pexdis.py`, `kitcraft-pex`) of
//! `C:\Modding\scripts\noc_godmode_patch` (our god-mode patch for NPC Outfit Changer),
//! recorded in `docs/validate.md`: `Debug.SetGodMode` is the call that leaves the player
//! invulnerable when a script stops between turning it on and off. Calls are matched as
//! `callstatic <class> <function>` (operands 0 and 1) and `callmethod <function> <object>`
//! (operand 0, and the declared type of the object variable, operand 1, as the class), names
//! ASCII case-insensitive. A user script's own method that happens to share a denylisted
//! name (`MyQuest.SetGodMode`) is not a match.

use std::path::{Path, PathBuf};

use kitcraft_pex::{format_value, opcode_name, Function, Pex, Value};

use super::{Finding, Report, Severity};

/// `(class, function, why)` entries; the class is empty for a method call on any object.
pub const DENYLIST: &[(&str, &str, &str)] = &[(
    "debug",
    "setgodmode",
    "toggles god mode; if the script stops between turning it on and off (fast travel, a load), the player stays invulnerable in the save",
)];

/// Largest script read from an archive (the biggest official `.pex` is far below it).
const MAX_ARCHIVED_SCRIPT: u64 = 64 << 20;

/// Whether `path` names a `.ba2` archive.
pub fn is_archive(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ba2"))
}

/// The archives that belong to `plugin`: `<stem> - <anything>.ba2` (and `<stem>.ba2`) in
/// the plugin's directory, as the official files name them (`Starfield - Misc.ba2`,
/// `SFBGS003 - Main.ba2`), sorted.
pub fn plugin_archives(plugin: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(stem)) = (plugin.parent(), plugin.file_stem()) else {
        return Vec::new();
    };
    let stem = stem.to_string_lossy().to_ascii_lowercase();
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_archive(p))
        .filter(|p| {
            p.file_stem().is_some_and(|s| {
                let s = s.to_string_lossy().to_ascii_lowercase();
                s == stem || s.starts_with(&format!("{stem} - "))
            })
        })
        .collect();
    out.sort();
    out
}

/// Scans the `.pex` files at `paths`: loose files, directories searched recursively (at
/// most 16 levels and 100,000 files), and `.ba2` archives, whose `.pex` entries are read
/// in memory (lane G's `kitcraft-ba2`); a script in an archive is named
/// `<archive>/<path in archive>`. An archived script that does not parse is listed in
/// `report.limitations` rather than failing the run.
pub fn denylisted_calls(paths: &[PathBuf], report: &mut Report) -> Result<(), String> {
    let mut files = Vec::new();
    let mut archives = Vec::new();
    for path in paths {
        if path.is_file() && is_archive(path) {
            archives.push(path.clone());
        } else {
            collect(path, 0, &mut files)?;
        }
    }
    for archive in archives {
        scan_archive(&archive, report)?;
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

/// The `.pex` entries of one archive.
fn scan_archive(path: &Path, report: &mut Report) -> Result<(), String> {
    let (archive, mut file) =
        kitcraft_ba2::Archive::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let archive_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut failed: Vec<String> = Vec::new();
    for (index, entry) in archive.entries.iter().enumerate() {
        if !entry.name.to_ascii_lowercase().ends_with(".pex") {
            continue;
        }
        let label = format!("{archive_name}/{}", entry.name.replace('\\', "/"));
        let parsed = archive
            .extract_vec(&mut file, index, MAX_ARCHIVED_SCRIPT)
            .map_err(|e| e.to_string())
            .and_then(|bytes| Pex::parse(&bytes).map_err(|e| e.to_string()));
        match parsed {
            Ok(pex) => {
                report.scripts = report.scripts.saturating_add(1);
                scan(&pex, &label, &mut report.findings);
            }
            Err(e) => failed.push(format!("{label} ({e})")),
        }
    }
    if !failed.is_empty() {
        let shown: Vec<&String> = failed.iter().take(20).collect();
        report.limitations.push(format!(
            "{} script(s) in {archive_name} could not be read and were not scanned: {}",
            failed.len(),
            shown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
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

/// The declared type (lower case) of the variable `receiver` (lower case) that a
/// `callmethod` in `function` of `object` calls on: `self` is the object, else a parameter,
/// a local, or an object variable. Empty when it is none of them (never guessed).
fn receiver_type(
    pex: &Pex,
    object: &kitcraft_pex::Object,
    function: &Function,
    receiver: &str,
) -> String {
    if receiver == "self" {
        return pex.string(object.name).to_ascii_lowercase();
    }
    let named = |name: u16| pex.string(name).eq_ignore_ascii_case(receiver);
    function
        .params
        .iter()
        .chain(&function.locals)
        .find(|(name, _)| named(*name))
        .map(|(_, ty)| *ty)
        .or_else(|| {
            object
                .variables
                .iter()
                .find(|v| named(v.name))
                .map(|v| v.type_name)
        })
        .map(|ty| pex.string(ty).to_ascii_lowercase())
        .unwrap_or_default()
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
                    // The receiver's declared type: `self` is this object, else a
                    // parameter, local or object variable (empty when not found).
                    "callmethod" => (receiver_type(pex, object, function, &ident(1)), ident(0)),
                    _ => continue,
                };
                let Some((c, f, why)) = DENYLIST
                    .iter()
                    .find(|(c, f, _)| *f == name && (c.is_empty() || *c == class))
                else {
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
