//! `testplan` (M4.E6): an in-game test plan for a plugin, for the owner to run on the MO2
//! profile `KitKraft Dev` with a disposable save (AGENTS.md, "Owner's live game is off
//! limits").
//!
//! It writes three files into `out` and nothing anywhere else (the plan is run by the
//! owner, or by `kit game session` on `KitKraft Dev` once ADR 0010's launch facts are
//! observed):
//!
//! - `kitkraft-test.txt`, a console batch file: for each new placed reference, `coc` to its
//!   interior cell (by the cell's EditorID), `player.moveto` and `prid` on the reference's
//!   EditorID when it has one (no load-order slot involved; the POI investigation of
//!   2026-10-15), else on its runtime ID; for each new item, `player.additem`, and for a
//!   new or overridden `ARMO` a following `player.equipitem` (the outfit must be worn, not
//!   only owned); for each override of a placed reference, `coc` and `prid`. Runtime IDs
//!   come from the load order
//!   (lane D's `LoadOrder::runtime_id`): confirmed when `load --from-save` adopted a save's
//!   slots and the plugin is in that load order, else predicted by `runtime-slots-by-scale`
//!   (and, for a plugin appended to the given load order after the last master-flagged
//!   plugin, `masters-load-first`); every numeric ID says which. That the game runs such a
//!   file with `bat <name>` from its own folder is grade D until M4.O2 confirms it;
//!   KitKraft never writes into the game folder: the owner copies the file there.
//! - `checklist.md`: setup on `KitKraft Dev` from a disposable save, one step per form with
//!   the expected observation (by EditorID first, then clicking the object in the console
//!   to read its reference ID against the confirmed or predicted one), the persistence steps
//!   (leave, save, reload, return), what to record in the game ledger
//!   (`docs/game-observations.md`, `game-<yyyymmdd>-<nn>`), and the removal rule for a plugin
//!   that owns forms.
//! - `probes.toml` (lane C, M4.T2; [`probes`]): one probe per checklist step, with the
//!   step's number as its row (`T-<nn>`), each a console command or a short batch and a
//!   matcher whose expected value comes from the plugin's own records, or
//!   `matcher = "manual"` with the reason. `kit game session` runs it.
//!
//! All three are deterministic: no clock, sorted by FormID.

mod probes;

use std::fmt::Write as _;
use std::fs::File;
use std::path::{Path, PathBuf};

use kitkraft_doc::{GlobalForm, IndexCache, LoadOptions, LoadOrder, PluginSpec, RuntimeBasis};
use kitkraft_esm::cells::group_type;
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::{Tes4, FLAG_MASTER};
use kitkraft_esm::{DecodeLimits, Signature};
use serde_json::{json, Value};

use super::load::{acquire, runtime_note, Loaded, RUNTIME_NOTE};
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
    /// The plugin's own `FULL` text, when stored inline (lane C, for `probes.toml`).
    full: Option<String>,
    /// For a reference: the base's EditorID.
    base_edid: Option<String>,
    /// For a reference: the record's initially-disabled flag (0x800).
    disabled: bool,
}

#[derive(Debug, Clone)]
struct CellInfo {
    label: String,
    editor_id: Option<String>,
    interior: bool,
}

/// Where the plan's numeric runtime IDs come from.
#[derive(Debug, Clone)]
struct IdBasis {
    /// `confirmed-by-save` or `inferred:<assumptions>`.
    short: String,
    /// For the checklist: "confirmed by the save ..." or "predicted by ...".
    text: String,
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
    for file in [BATCH, "checklist.md", probes::FILE] {
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
    let (mut lo, at, appended) = with_plugin(loaded, &plugin, &name, master_flag, no_cache)?;
    // A plugin already in a save-confirmed load order keeps the save's slots; appended, the
    // save says nothing about it (and it may shift others): inferred.
    if let (false, RuntimeBasis::ConfirmedBySave(save)) = (appended, &loaded.lo.runtime_basis) {
        let slots = loaded.lo.plugins.iter().map(|p| p.runtime_slot).collect();
        lo.adopt_runtime_slots(slots, save.clone())
            .map_err(|e| failed(ID, e))?;
    }
    if appended {
        limitations.push(ASSUMED_ORDER.to_string());
    }
    limitations.retain(|l| !l.starts_with(RUNTIME_NOTE));
    limitations.push(runtime_note(&lo));
    limitations.push(BATCH_GRADE_D.to_string());
    let id_basis = match (&lo.runtime_basis, appended) {
        (RuntimeBasis::ConfirmedBySave(save), _) => IdBasis {
            short: "confirmed-by-save".into(),
            text: format!("confirmed by the save {save}"),
        },
        (RuntimeBasis::Inferred, false) => IdBasis {
            short: "inferred:runtime-slots-by-scale".into(),
            text: "predicted by the assumption `runtime-slots-by-scale` (docs/load-order.md); \
                   not confirmed by a save (`--from-save`)"
                .into(),
        },
        (RuntimeBasis::Inferred, true) => IdBasis {
            short: "inferred:runtime-slots-by-scale+masters-load-first".into(),
            text: "predicted by the assumptions `runtime-slots-by-scale` and \
                   `masters-load-first` (the plugin is not in the given load order, so it was \
                   appended); not confirmed by a save"
                .into(),
        },
    };
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
            full: None,
            base_edid: None,
            disabled: is_ref && loc.flags & probes::FLAG_INITIALLY_DISABLED != 0,
        };
        if !is_ref && !tes4.is_localized() {
            let record = read_indexed(&mut file, loc, Some(&limits))
                .map_err(|e| failed(ID, format!("{name}: {e}")))?;
            form.full = record
                .subrecords()
                .unwrap_or_default()
                .iter()
                .find(|s| s.signature() == Signature(*b"FULL"))
                .and_then(|s| probes::inline_text(s.data()));
        }
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
                form.base_edid = edid_of(&lo, base);
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
            // A reference with an EditorID is named by it: no load-order slot involved.
            // Otherwise its runtime ID, confirmed by a save or predicted.
            let (target, by) = match &f.editor_id {
                Some(e) => (e.clone(), "by its EditorID".to_string()),
                None => (
                    hex(runtime),
                    format!("by its runtime ID ({})", id_basis.short),
                ),
            };
            if f.new {
                push(
                    &mut batch,
                    format!("player.moveto {target}"),
                    format!("stand at {what}, {by}"),
                );
            }
            push(
                &mut batch,
                format!("prid {target}"),
                format!("select {what}, {by}"),
            );
        } else if (f.new && ITEMS.iter().any(|s| s.as_slice() == f.signature.as_bytes()))
            || f.signature == "ARMO"
        {
            push(
                &mut batch,
                format!("player.additem {} 1", hex(runtime)),
                format!("add {what}, by its runtime ID ({})", id_basis.short),
            );
            // Armor is an outfit: it must be worn, not only owned (new or overridden).
            if f.signature == "ARMO" {
                push(
                    &mut batch,
                    format!("player.equipitem {}", hex(runtime)),
                    format!("equip {what}, by its runtime ID ({})", id_basis.short),
                );
            }
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
        &id_basis,
    );
    std::fs::create_dir_all(&out).map_err(|e| failed(ID, format!("{}: {e}", out.display())))?;
    let batch_path = out.join(BATCH);
    let checklist_path = out.join("checklist.md");
    std::fs::write(&batch_path, batch.as_bytes())
        .map_err(|e| failed(ID, format!("{}: {e}", batch_path.display())))?;
    std::fs::write(&checklist_path, checklist.as_bytes())
        .map_err(|e| failed(ID, format!("{}: {e}", checklist_path.display())))?;
    // Lane C (M4.T2): the probes, one per checklist step.
    let probe_file = probes::build(&name, &sha, &forms, &id_basis);
    let probes_path = out.join(probes::FILE);
    let probes_text = probe_file
        .to_toml()
        .map_err(|e| failed(ID, e.to_string()))?;
    std::fs::write(&probes_path, probes_text.as_bytes())
        .map_err(|e| failed(ID, format!("{}: {e}", probes_path.display())))?;
    let manual = probe_file.probes.iter().filter(|p| p.is_manual()).count();

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
                "runtime_id_basis": f.runtime.map(|_| id_basis.short.clone()),
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
            "load_index": at, "appended": appended, "runtime_id_basis": id_basis.short,
            "batch": batch_path.display().to_string(),
            "checklist": checklist_path.display().to_string(),
            "probes": {
                "path": probes_path.display().to_string(),
                "rows": probe_file.probes.len(),
                "manual": manual,
            },
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
    id_basis: &IdBasis,
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
        "| Load-order slot | {slot}: {}; the numeric IDs below change if it loads elsewhere |",
        id_basis.text
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
    let _ = writeln!(
        s,
        "Step `n` is row `T-<nn>` of `{}` beside this file (the same checks, machine-checkable, for\n`kit game session`); the persistence steps below continue the numbering.\n",
        probes::FILE
    );
    let _ = writeln!(s, "Run all at once with `bat kitkraft-test` in the console, or type the commands one by one.\n");
    let _ = writeln!(
        s,
        "How to name an object: a reference with an EditorID is named by it (`player.moveto <EditorID>`,\n`prid <EditorID>`; that the console takes a persistent reference's EditorID is grade D until\nM4.O2). To read any object's ID, open the console and click the object: the console shows its\nreference ID, whose high byte is the plugin's runtime slot. The numeric IDs below are\n{}.\n",
        id_basis.text
    );
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
                        let _ = writeln!(
                            s,
                            "{n}. Go to cell {} (exterior; `player.moveto` below takes you there).",
                            cell.label
                        );
                    }
                }
                n = n.saturating_add(1);
            }
            // By EditorID first; the numeric ID is what clicking it should show.
            let target = f.editor_id.as_deref().unwrap_or(&runtime);
            if f.new {
                let _ = writeln!(
                    s,
                    "{n}. `player.moveto {target}`: you stand at the new {} `{edid}`.",
                    f.signature
                );
                n = n.saturating_add(1);
            }
            let _ = writeln!(
                s,
                "{n}. Open the console and click the object: {base} at {pos}.\n   **Expect:** the console shows the reference ID `{runtime}` ({}). Another ID means\n   another slot, not a missing object: write down the ID shown.",
                id_basis.short
            );
            n = n.saturating_add(1);
            let _ = writeln!(
                s,
                "{n}. `prid {target}`, then `getpos x`, `getpos y`, `getpos z`.\n   **Expect:** `prid` selects the same object; the positions match {pos}. Screenshot it\n   (into `observe\\`).",
            );
            if f.editor_id.is_some() {
                let _ = writeln!(
                    s,
                    "   Also `prid {runtime}` (the numeric ID, {}): it selects the same object.",
                    id_basis.short
                );
            }
            n = n.saturating_add(1);
        } else if (f.new && ITEMS.iter().any(|s| s.as_slice() == f.signature.as_bytes()))
            || f.signature == "ARMO"
        {
            let _ = writeln!(
                s,
                "{n}. `player.additem {runtime} 1` ({}). **Expect:** `{edid}` in the inventory.",
                id_basis.short
            );
            n = n.saturating_add(1);
            // Armor is an outfit: it must be worn and looked at, not only owned (new or
            // overridden).
            if f.signature == "ARMO" {
                let _ = writeln!(
                    s,
                    "{n}. `player.equipitem {runtime}`: equip it. **Expect:** the player wears it;\n   the slots it fills are free of other items."
                );
                n = n.saturating_add(1);
                let _ = writeln!(
                    s,
                    "{n}. Look at it: third person, `tfc` free camera, a screenshot into\n   `observe\\`."
                );
                n = n.saturating_add(1);
            }
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
