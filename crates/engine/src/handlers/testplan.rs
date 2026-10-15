//! `testplan` (M4.E6): an in-game test plan for a plugin, for the owner to run on the MO2
//! profile `KitKraft Dev` with a disposable save (AGENTS.md, "Owner's live game is off
//! limits").
//!
//! It writes two files into `out` and nothing anywhere else:
//!
//! - `kitkraft-test.txt`, a console batch file: for each new placed reference, `coc` to its
//!   interior cell (by the cell's EditorID), `player.moveto` and `prid` on the reference's
//!   runtime ID; for each new item, `player.additem`; for each override of a placed
//!   reference, `coc` and `prid`. The runtime IDs are computed for the plugin appended to
//!   the given load order (lane D's `LoadOrder::runtime_id`, assumption
//!   `runtime-slots-by-scale`; a master-flagged plugin goes after the last master-flagged
//!   plugin, assumption `masters-load-first`). That the game runs such a file with
//!   `bat <name>` from its own folder is grade D until M4.O2 confirms it; KitKraft never
//!   writes into the game folder: the owner copies the file there.
//! - `checklist.md`: setup on `KitKraft Dev` from a disposable save, one step per form with
//!   the expected observation, the persistence steps (leave, save, reload, return), what to
//!   record in the game ledger (`docs/game-observations.md`, `game-<yyyymmdd>-<nn>`), and
//!   the removal rule for a plugin that owns forms.
//!
//! Both are deterministic: no clock, sorted by FormID.

use std::fmt::Write as _;
use std::fs::File;
use std::path::{Path, PathBuf};

use kitkraft_doc::{GlobalForm, IndexCache, LoadOptions, LoadOrder, PluginSpec};
use kitkraft_esm::cells::group_type;
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::{Tes4, FLAG_MASTER};
use kitkraft_esm::{DecodeLimits, Signature};
use serde_json::{json, Value};

use super::load::{acquire, Loaded, RUNTIME_ASSUMED};
use super::{failed, str_arg};
use crate::edit::patch::guard_location;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

const ID: &str = "testplan";

/// The MO2 profile in-game checks run on (AGENTS.md); never `Default`.
pub const DEV_PROFILE: &str = "KitKraft Dev";

/// The batch file's name.
pub const BATCH: &str = "kitkraft-test.txt";

/// Base record types `player.additem` can give (inventory items).
const ITEMS: [&[u8; 4]; 10] = [
    b"WEAP", b"ARMO", b"MISC", b"ALCH", b"AMMO", b"BOOK", b"KEYM", b"AMDL", b"OMOD", b"LVLI",
];

const ASSUMED_ORDER: &str = "the plugin is appended to the given load order; a master-flagged plugin goes after the last master-flagged plugin (assumption `masters-load-first`, grade D until the KitKraft Dev load order is read back in game)";

const BATCH_GRADE_D: &str = "console batch file: that the game runs a text file of console commands with `bat <name>` from the folder holding Starfield.exe is grade D until M4.O2 confirms it; KitKraft does not write into the game folder, the owner copies the file there";

/// One form of the plugin the plan covers.
#[derive(Debug, Clone)]
struct Form {
    signature: String,
    stored: u32,
    new: bool,
    editor_id: Option<String>,
    runtime: Option<u32>,
    /// For a reference: base EditorID or formref, cell, position.
    base: Option<String>,
    cell: Option<CellInfo>,
    position: Option<[f32; 3]>,
    persistent: bool,
    offset: u64,
    length: u64,
}

#[derive(Debug, Clone)]
struct CellInfo {
    label: String,
    editor_id: Option<String>,
    interior: bool,
}

fn hex(id: u32) -> String {
    format!("{id:08X}")
}

/// The load order with `plugin` in it: as loaded when it is already there, else rebuilt
/// with it appended (after the last master-flagged plugin when it is master-flagged).
fn with_plugin(
    loaded: &Loaded,
    plugin: &Path,
    name: &str,
    master_flag: bool,
    no_cache: bool,
) -> Result<(LoadOrder, u16, bool), EngineError> {
    if let Some(i) = loaded.lo.find(name) {
        return Ok((
            LoadOrder::load(
                loaded
                    .lo
                    .plugins
                    .iter()
                    .map(|p| PluginSpec {
                        name: p.name.clone(),
                        path: p.path.clone(),
                    })
                    .collect(),
                &LoadOptions {
                    cache: if no_cache {
                        None
                    } else {
                        IndexCache::default_location()
                    },
                    threads: 0,
                },
            )
            .map_err(|e| failed(ID, e.to_string()))?,
            i,
            false,
        ));
    }
    let mut specs: Vec<PluginSpec> = loaded
        .lo
        .plugins
        .iter()
        .map(|p| PluginSpec {
            name: p.name.clone(),
            path: p.path.clone(),
        })
        .collect();
    let at = if master_flag {
        loaded
            .lo
            .plugins
            .iter()
            .rposition(|p| p.header().is_some_and(|h| h.flags.0 & FLAG_MASTER != 0))
            .map_or(0, |i| i.saturating_add(1))
    } else {
        specs.len()
    };
    specs.insert(
        at,
        PluginSpec {
            name: name.to_string(),
            path: plugin.to_path_buf(),
        },
    );
    let lo = LoadOrder::load(
        specs,
        &LoadOptions {
            cache: if no_cache {
                None
            } else {
                IndexCache::default_location()
            },
            threads: 0,
        },
    )
    .map_err(|e| failed(ID, e.to_string()))?;
    let index = u16::try_from(at).map_err(|_| failed(ID, "load order too long"))?;
    Ok((lo, index, true))
}

/// The EditorID of the winning version of `form`.
fn edid_of(lo: &LoadOrder, form: GlobalForm) -> Option<String> {
    let v = lo.forms.versions(form).last()?;
    let loc = lo.record(v.plugin, v.record)?;
    lo.edid(v.plugin, loc).map(str::to_string)
}

fn signature_of(lo: &LoadOrder, form: GlobalForm) -> Option<String> {
    let v = lo.forms.versions(form).last()?;
    lo.record(v.plugin, v.record)
        .map(|l| String::from_utf8_lossy(&l.signature).into_owned())
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let plugin = PathBuf::from(str_arg(ID, args, "plugin")?);
    let out = PathBuf::from(str_arg(ID, args, "out")?);
    let no_cache = args.get("no_cache").and_then(Value::as_bool) == Some(true);
    for file in [BATCH, "checklist.md"] {
        guard_location(&out.join(file)).map_err(|e| failed(ID, e.to_string()))?;
    }
    if !plugin.is_file() {
        return Err(failed(ID, format!("{}: not a file", plugin.display())));
    }
    let name = crate::edit::source::file_name(&plugin);
    let limits = DecodeLimits::default();
    let index = index_file(&plugin, &IndexOptions::default())
        .map_err(|e| failed(ID, format!("{}: {e}", plugin.display())))?;
    let tes4 = Tes4::new(&index.header);
    let masters: Vec<String> = tes4.masters().iter().map(|m| m.name_lossy()).collect();
    let master_flag = tes4.flags() & FLAG_MASTER != 0;
    let (size, sha) = crate::deploy::sha256::file(&plugin)
        .map_err(|e| failed(ID, format!("{}: {e}", plugin.display())))?;
    let spec_sha = match args.get("spec").and_then(Value::as_str) {
        Some(s) => Some(
            crate::deploy::sha256::file(Path::new(s))
                .map_err(|e| failed(ID, format!("{s}: {e}")))?
                .1,
        ),
        None => None,
    };

    let loaded = acquire(session, args, ID)?;
    let mut limitations = loaded.limitations.clone();
    let (lo, at, appended) = with_plugin(loaded, &plugin, &name, master_flag, no_cache)?;
    if appended {
        limitations.push(ASSUMED_ORDER.to_string());
    }
    if !limitations.iter().any(|l| l == RUNTIME_ASSUMED) {
        limitations.push(RUNTIME_ASSUMED.to_string());
    }
    limitations.push(BATCH_GRADE_D.to_string());
    let profile = args
        .get("profile")
        .and_then(Value::as_str)
        .map(str::to_string);
    if profile
        .as_deref()
        .and_then(|p| Path::new(p).file_name())
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("Default"))
    {
        limitations.push(format!(
            "the load order was read from the Default profile (read only); the steps run on the {DEV_PROFILE} profile, a copy of it, with this plugin enabled last: never on Default"
        ));
    }

    // The plugin's records.
    let own = masters.len();
    let mut file =
        File::open(&plugin).map_err(|e| failed(ID, format!("{}: {e}", plugin.display())))?;
    let mut forms: Vec<Form> = Vec::new();
    for loc in index.records.iter().skip(1) {
        let new = usize::try_from(loc.form_id >> 24).is_ok_and(|i| i == own);
        let global = lo.resolve(at, loc.form_id).ok();
        let sig = loc.signature;
        let is_ref = sig.0 == *b"REFR" || sig.0 == *b"ACHR";
        let path = index.group_path(loc.group);
        let carried = !new && matches!(&sig.0, b"CELL" | b"WRLD");
        if carried {
            continue;
        }
        let mut form = Form {
            signature: sig.to_string(),
            stored: loc.form_id,
            new,
            editor_id: loc.edid_lossy().map(|e| e.into_owned()),
            runtime: global.and_then(|g| lo.runtime_id(g)),
            base: None,
            cell: None,
            position: None,
            persistent: path
                .last()
                .is_some_and(|g| g.header.group_type == group_type::CELL_PERSISTENT),
            offset: loc.offset,
            length: loc.len(),
        };
        if is_ref {
            let record = read_indexed(&mut file, loc, Some(&limits))
                .map_err(|e| failed(ID, format!("{name}: {e}")))?;
            let subs = record.subrecords().unwrap_or_default();
            let raw_of = |sig: &[u8; 4]| {
                subs.iter()
                    .find(|s| s.signature() == Signature(*sig))
                    .and_then(|s| s.data().get(0..4))
                    .and_then(|b| <[u8; 4]>::try_from(b).ok())
                    .map(u32::from_le_bytes)
            };
            if let Some(base) = raw_of(b"NAME").and_then(|r| lo.resolve(at, r).ok()) {
                form.base = Some(match edid_of(&lo, base) {
                    Some(e) => format!(
                        "{e} ({}, {})",
                        signature_of(&lo, base).unwrap_or_default(),
                        lo.form_label(base)
                    ),
                    None => lo.form_label(base),
                });
            }
            if let Some(data) = subs
                .iter()
                .find(|s| s.signature() == Signature(*b"DATA"))
                .map(|s| s.data())
            {
                let f = |i: usize| {
                    data.get(i..i.saturating_add(4))
                        .and_then(|b| <[u8; 4]>::try_from(b).ok())
                        .map(f32::from_le_bytes)
                };
                if let (Some(x), Some(y), Some(z)) = (f(0), f(4), f(8)) {
                    form.position = Some([x, y, z]);
                }
            }
            let cell = path
                .iter()
                .rev()
                .find(|g| g.header.group_type == group_type::CELL_CHILDREN)
                .map(|g| g.header.label_u32());
            if let Some(cell) = cell.and_then(|c| lo.resolve(at, c).ok()) {
                form.cell = Some(CellInfo {
                    label: lo.form_label(cell),
                    editor_id: edid_of(&lo, cell),
                    interior: path.first().is_some_and(|g| g.header.label == *b"CELL"),
                });
            }
        }
        forms.push(form);
    }
    forms.sort_by_key(|f| (!f.new, f.stored));

    // The batch file.
    let mut batch = String::new();
    let mut commands: Vec<Value> = Vec::new();
    let mut last_cell: Option<String> = None;
    let mut push = |batch: &mut String, line: String, why: String| {
        batch.push_str(&line);
        batch.push_str("\r\n");
        commands.push(json!({"command": line, "why": why}));
    };
    for f in &forms {
        let Some(runtime) = f.runtime else {
            continue;
        };
        let what = format!(
            "{} {}{}",
            f.signature,
            hex(f.stored),
            f.editor_id
                .as_deref()
                .map(|e| format!(" ({e})"))
                .unwrap_or_default()
        );
        if f.signature == "REFR" || f.signature == "ACHR" {
            if let Some(cell) = &f.cell {
                if let (true, Some(edid)) = (cell.interior, &cell.editor_id) {
                    if last_cell.as_deref() != Some(edid) {
                        push(
                            &mut batch,
                            format!("coc {edid}"),
                            format!("go to {}, the cell of {what}", cell.label),
                        );
                        last_cell = Some(edid.clone());
                    }
                }
            }
            if f.new {
                push(
                    &mut batch,
                    format!("player.moveto {}", hex(runtime)),
                    format!("stand at {what}"),
                );
            }
            push(
                &mut batch,
                format!("prid {}", hex(runtime)),
                format!("select {what}"),
            );
        } else if f.new && ITEMS.iter().any(|s| s.as_slice() == f.signature.as_bytes()) {
            push(
                &mut batch,
                format!("player.additem {} 1", hex(runtime)),
                format!("add {what}"),
            );
        }
    }

    // The checklist.
    let checklist = checklist(
        &name,
        size,
        &sha,
        spec_sha.as_deref(),
        &masters,
        &forms,
        &lo,
        at,
    );
    std::fs::create_dir_all(&out).map_err(|e| failed(ID, format!("{}: {e}", out.display())))?;
    let batch_path = out.join(BATCH);
    let checklist_path = out.join("checklist.md");
    std::fs::write(&batch_path, batch.as_bytes())
        .map_err(|e| failed(ID, format!("{}: {e}", batch_path.display())))?;
    std::fs::write(&checklist_path, checklist.as_bytes())
        .map_err(|e| failed(ID, format!("{}: {e}", checklist_path.display())))?;

    let evidence: Vec<Evidence> = forms
        .iter()
        .map(|f| Evidence {
            file: name.clone(),
            offset: f.offset,
            length: f.length,
            record: Some(format!("{}:{}", f.signature, hex(f.stored))),
            subrecord: None,
        })
        .collect();
    let forms_json: Vec<Value> = forms
        .iter()
        .map(|f| {
            json!({
                "signature": f.signature, "form_id": hex(f.stored), "new": f.new,
                "editor_id": f.editor_id, "runtime_id": f.runtime.map(hex),
                "base": f.base, "cell": f.cell.as_ref().map(|c| json!({"form": c.label, "editor_id": c.editor_id, "interior": c.interior})),
                "position": f.position, "persistent": f.persistent,
            })
        })
        .collect();
    let summary = format!(
        "{}: {} command(s) for {} form(s) of {name} ({} new) -> {}; run on the {DEV_PROFILE} profile with a disposable save",
        BATCH,
        commands.len(),
        forms.len(),
        forms.iter().filter(|f| f.new).count(),
        out.display()
    );
    Ok(Outcome {
        data: json!({
            "plugin": plugin.display().to_string(), "sha256": sha, "size": size,
            "masters": masters, "profile": DEV_PROFILE,
            "load_order_source": profile.unwrap_or_else(|| "plugins".into()),
            "load_index": at, "appended": appended,
            "batch": batch_path.display().to_string(),
            "checklist": checklist_path.display().to_string(),
            "commands": commands, "forms": forms_json, "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
}

#[allow(clippy::too_many_arguments)]
fn checklist(
    name: &str,
    size: u64,
    sha: &str,
    spec_sha: Option<&str>,
    masters: &[String],
    forms: &[Form],
    lo: &LoadOrder,
    at: u16,
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# In-game test plan: {name}\n");
    let _ = writeln!(
        s,
        "Generated by `kit testplan`. Run it only on the MO2 profile **{DEV_PROFILE}** (a copy of\n`Default` with its own saves), from a **disposable save**. Never on `Default`.\n"
    );
    let _ = writeln!(s, "| | |\n| - | - |");
    let _ = writeln!(s, "| Plugin | `{name}` |");
    let _ = writeln!(s, "| Size | {size} bytes |");
    let _ = writeln!(s, "| SHA-256 | `{sha}` |");
    if let Some(h) = spec_sha {
        let _ = writeln!(s, "| Spec SHA-256 | `{h}` |");
    }
    let _ = writeln!(s, "| Masters | {} |", masters.join(", "));
    let slot = lo
        .plugin(at)
        .and_then(|p| p.runtime_slot)
        .map_or_else(|| "unknown".to_string(), |r| r.label());
    let _ = writeln!(
        s,
        "| Load-order slot | {slot} (assumed: `runtime-slots-by-scale`; the runtime IDs below change if it loads elsewhere) |"
    );
    let owns = forms.iter().any(|f| f.new);
    let _ = writeln!(
        s,
        "| Owns forms | {} |\n",
        if owns {
            "yes: never remove it from a save that is not disposable"
        } else {
            "no (overrides only)"
        }
    );
    let _ = writeln!(s, "## Setup\n");
    let _ = writeln!(s, "1. Install the plugin into MO2 as its own mod (`kit package`, then install the folder or zip;\n   or `kit deploy --dev`), and enable it in the **{DEV_PROFILE}** profile, after its masters.");
    let _ = writeln!(
        s,
        "2. Check the plugin's SHA-256 on disk equals the one above."
    );
    let _ = writeln!(s, "3. Copy `{BATCH}` into the folder that holds `Starfield.exe` (the game reads batch files\n   from there: grade D, confirm it in this session).");
    let _ = writeln!(s, "4. Start Starfield through MO2 with the **{DEV_PROFILE}** profile and load a disposable save\n   (or start a new game and save it as one).\n");
    let _ = writeln!(s, "## Steps\n");
    let _ = writeln!(s, "Run all at once with `bat kitkraft-test` in the console, or type the commands one by one.\n");
    let mut n = 1u32;
    for f in forms {
        let runtime = f.runtime.map_or_else(|| "(no runtime ID)".to_string(), hex);
        let edid = f.editor_id.as_deref().unwrap_or("-");
        if f.signature == "REFR" || f.signature == "ACHR" {
            let base = f.base.as_deref().unwrap_or("its base");
            let pos = f
                .position
                .map(|p| format!("({:.1}, {:.1}, {:.1})", p[0], p[1], p[2]))
                .unwrap_or_else(|| "(no position)".into());
            if let Some(cell) = &f.cell {
                match (&cell.editor_id, cell.interior) {
                    (Some(c), true) => {
                        let _ = writeln!(s, "{n}. `coc {c}` (cell {}).", cell.label);
                    }
                    _ => {
                        let _ = writeln!(s, "{n}. Go to cell {} (exterior; `player.moveto {runtime}` below takes you there).", cell.label);
                    }
                }
                n = n.saturating_add(1);
            }
            if f.new {
                let _ = writeln!(
                    s,
                    "{n}. `player.moveto {runtime}`: you stand at the new {} `{edid}`.",
                    f.signature
                );
                if f.persistent && f.editor_id.is_some() {
                    let _ = writeln!(
                        s,
                        "   If the console says the ID is unknown (the plugin took another slot), try\n   `player.moveto {edid}` and `prid {edid}`: a persistent reference with an EditorID\n   may be named directly (grade D); note the ID it shows."
                    );
                }
                n = n.saturating_add(1);
            }
            let _ = writeln!(
                s,
                "{n}. `prid {runtime}`, then `getpos x`, `getpos y`, `getpos z`.\n   **Expect:** {base} is visible at {pos}; `prid` selects it and the console shows its\n   reference ID `{runtime}`; the positions match. Screenshot it (into `observe\\`).",
            );
            n = n.saturating_add(1);
        } else if f.new && ITEMS.iter().any(|s| s.as_slice() == f.signature.as_bytes()) {
            let _ = writeln!(
                s,
                "{n}. `player.additem {runtime} 1`. **Expect:** `{edid}` in the inventory."
            );
            n = n.saturating_add(1);
        } else {
            let _ = writeln!(
                s,
                "{n}. `help {edid} 4` lists {} `{edid}` ({runtime}). **Expect:** it is found{}.",
                f.signature,
                if f.new {
                    " (a new form)"
                } else {
                    " with the changed values"
                }
            );
            n = n.saturating_add(1);
        }
    }
    let _ = writeln!(s, "\n## Persistence\n");
    let _ = writeln!(s, "{n}. Leave the area (`coc` to another cell, or fast travel), wait one in-game hour (`wait`),\n   save (a new disposable save), quit to the main menu, and load that save.");
    n = n.saturating_add(1);
    let _ = writeln!(s, "{n}. Return (`bat kitkraft-test` again). **Expect:** every object is where it was; note any that\n   vanished, moved or doubled (a temporary reference may reset when its cell resets: say how\n   long you were away).\n");
    let _ = writeln!(s, "## Report\n");
    let _ = writeln!(s, "Add an entry to `docs/game-observations.md`, id `game-<yyyymmdd>-<nn>`: game build, the\nplugin's SHA-256 above, profile **{DEV_PROFILE}**, each step with what was expected and what was\nseen, and the screenshots' file names in `observe\\`. A step that could not run (a batch file\nthe game did not read, a command it rejected) is a result: write it down.\n");
    let _ = writeln!(s, "## Afterwards\n");
    if owns {
        let _ = writeln!(s, "The plugin owns forms: keep it enabled for saves made with it, or delete those saves.\nDisable it in **{DEV_PROFILE}** only between disposable saves.");
    } else {
        let _ = writeln!(s, "The plugin only overrides: it can be disabled again.");
    }
    s
}
