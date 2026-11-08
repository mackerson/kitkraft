//! `set` (M2.E5): write field values on the winning version of a form into a patch plugin.
//!
//! The load order comes from `profile` / `plugins` / `data_dir` (loaded first, as for
//! `inspect`), or from the session. The form is a formref (`Starfield.esm:21BBCD`), an
//! EditorID, or a hex FormID in `plugin`'s space; its winning version is copied into `into`
//! through the patch writer (`crate::edit::patch`), with `fields` applied
//! (`crate::edit::fields`): `{"FULL": "Old Soviet Marksman Rifle"}`, or the CLI shorthand
//! `FULL="..."`. The write goes to a staged copy of `into` (OS temp dir), the plugin rules
//! of `validate` run on it as `build`'s do, and `into` is replaced only when the records
//! written pass (`docs/validate.md`).

use std::cell::RefCell;
use std::fs::File;
use std::path::{Path, PathBuf};

use kitkraft_doc::GlobalForm;
use kitkraft_esm::index::read_record_at;
use kitkraft_esm::DecodeLimits;
use serde_json::{json, Map, Value};

use super::load::{acquire, Loaded};
use super::{failed, str_arg};
use crate::args::Violation;
use crate::edit::fields::{parse_op, parse_path};
use crate::edit::patch::{
    guard_target, load_order_matches, write_patch_with, Copy, Edit, NewRef, PatchError,
    PatchOptions, PatchReport,
};
use crate::edit::source::{placement_at, SourcePlugin};
use crate::edit::strings::TableStrings;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;
use crate::validate::{self, Severity};

const ID: &str = "set";

/// Record flag "deleted" (UESP, Skyrim_Mod:Mod_File_Format, "Records").
const DELETED: u32 = 0x0000_0020;

/// The edits in a `fields` object; a bad path or value is `invalid_args` at
/// `/<arg>/<path>` (JSON pointer, `~` and `/` escaped).
pub(crate) fn edits(
    command: &str,
    pointer: &str,
    fields: &Map<String, Value>,
) -> Result<Vec<Edit>, EngineError> {
    let invalid = |key: &str, rule: &'static str, message: String| EngineError::InvalidArgs {
        command: command.to_string(),
        violations: vec![Violation {
            pointer: format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1")),
            rule,
            message,
        }],
    };
    if fields.is_empty() {
        return Err(invalid("", "minProperties", "no fields to set".into()));
    }
    fields
        .iter()
        .map(|(key, value)| {
            let path = parse_path(key).map_err(|e| invalid(key, e.rule, e.to_string()))?;
            let op = parse_op(key, value).map_err(|e| invalid(key, e.rule, e.to_string()))?;
            Ok(Edit { path, op })
        })
        .collect()
}

/// The form `form` names in the load order (formref, hex FormID in `plugin`, EditorID).
pub(crate) fn find_form(
    command: &str,
    loaded: &Loaded,
    form: &str,
    plugin: Option<&str>,
) -> Result<GlobalForm, EngineError> {
    let lo = &loaded.lo;
    if let Some(f) = kitkraft_doc::FormRef::parse(form) {
        let p = lo
            .find(&f.plugin)
            .ok_or_else(|| failed(command, format!("`{}` is not in the load order", f.plugin)))?;
        return Ok(GlobalForm {
            plugin: p,
            object_id: f.object_id,
        });
    }
    if let (Some(raw), Some(name)) = (super::inspect::parse_form_id(form), plugin) {
        let p = lo
            .find(name)
            .ok_or_else(|| failed(command, format!("`{name}` is not in the load order")))?;
        return lo
            .resolve(p, raw)
            .map_err(|e| failed(command, e.to_string()));
    }
    // The first record with the EditorID that resolves; otherwise why the ones found do not
    // (a missing master, named by lane D's resolver).
    let mut unresolved: Option<String> = None;
    for (index, loc) in loaded.edid_hits(form) {
        match lo.resolve(index, loc.form_id) {
            Ok(g) => return Ok(g),
            Err(e) => {
                let name = lo.plugin(index).map_or("?", |p| p.name.as_str());
                unresolved.get_or_insert_with(|| format!("{name}:{:08X} ({e})", loc.form_id));
            }
        }
    }
    Err(failed(
        command,
        match unresolved {
            Some(why) => format!(
                "`{form}` is the EditorID of {why}, whose FormID does not resolve in this load order"
            ),
            None => format!(
                "no record with formref or EditorID `{form}` in the load order (a hex FormID needs `plugin`)"
            ),
        },
    ))
}

/// Refuses to copy the winning version of `form` when lane D's source check (M3.D1,
/// `kitkraft_doc::source`) finds a problem that can change which version wins or what its
/// FormIDs mean: a plugin holding a version loads before its master, or a plugin after the
/// winner failed to load or has unresolved records that could be this form, or the winner
/// is missing or deleted. Problems elsewhere in the load order (an active plugin with no
/// file, an unrelated ordering error) are returned for `limitations` (M3.E1; this replaces
/// the blanket "load order unverified" refusal).
pub(crate) fn safe_source(
    command: &str,
    loaded: &Loaded,
    form: GlobalForm,
) -> Result<Vec<String>, EngineError> {
    let check = loaded.source_check(form);
    if !check.blocking.is_empty() {
        return Err(failed(
            command,
            format!(
                "the winning version of {} is not a safe source: {} (fix the load order, or name the plugins with `plugins`)",
                loaded.lo.form_label(form),
                check.blocking.join("; ")
            ),
        ));
    }
    Ok(check
        .unrelated
        .into_iter()
        // One line per problem, however many forms it leaves alone (`extend_once`).
        .map(|u| format!("load order problem that does not affect the forms written: {u}"))
        .collect())
}

/// Appends each of `more` to `limitations` unless it is already there.
pub(crate) fn extend_once(limitations: &mut Vec<String>, more: Vec<String>) {
    for l in more {
        if !limitations.contains(&l) {
            limitations.push(l);
        }
    }
}

/// Where the copied version came from, for the report.
#[derive(Debug, Clone)]
pub(crate) struct Origin {
    pub plugin: String,
    pub load_index: u16,
    pub offset: u64,
    pub length: u64,
    pub label: String,
    pub versions: usize,
}

/// A copy of the winning version of `form`, with `edits`, ready for the patch writer.
pub(crate) fn winning_copy(
    command: &str,
    loaded: &Loaded,
    form: GlobalForm,
    edits: Vec<Edit>,
    limits: &DecodeLimits,
) -> Result<(Copy, Origin), EngineError> {
    let lo = &loaded.lo;
    let versions = lo.forms.versions(form);
    let label = lo.form_label(form);
    let winner = versions
        .last()
        .ok_or_else(|| failed(command, format!("{label} has no version in the load order")))?;
    let plugin = lo
        .plugin(winner.plugin)
        .ok_or_else(|| failed(command, "winning plugin missing"))?;
    let header = plugin
        .header()
        .ok_or_else(|| failed(command, format!("{} has no readable header", plugin.name)))?;
    let loc = lo
        .record(winner.plugin, winner.record)
        .copied()
        .ok_or_else(|| failed(command, "winning record missing from the index"))?;
    let read = |offset: u64| {
        let mut file = File::open(&plugin.path)
            .map_err(|e| failed(command, format!("{}: {e}", plugin.path.display())))?;
        read_record_at(&mut file, offset, Some(limits))
            .map_err(|e| failed(command, format!("{}: {e}", plugin.name)))
    };
    let record = read(loc.offset)?;
    if record.header.flags & DELETED != 0 {
        return Err(failed(
            command,
            format!(
                "the winning version of {label} in {} is deleted (record flag 0x20); a patch does not edit a deleted record",
                plugin.name
            ),
        ));
    }
    let placement = placement_at(&plugin.path, loc.offset).map_err(|m| failed(command, m))?;
    let mut parents = Vec::new();
    for (depth, offset) in &placement.owners {
        let path = placement.path.get(..*depth).unwrap_or_default().to_vec();
        parents.push((read(*offset)?, path));
    }
    let source = SourcePlugin {
        name: plugin.name.clone(),
        path: plugin.path.clone(),
        masters: header.masters.clone(),
        flags: header.flags.0,
    };
    let origin = Origin {
        plugin: plugin.name.clone(),
        load_index: winner.plugin,
        offset: loc.offset,
        length: u64::from(loc.data_size).saturating_add(24),
        label: format!("{}:{:08X}", record.header.signature, record.header.form_id),
        versions: versions.len(),
    };
    Ok((
        Copy {
            source,
            record,
            path: placement.path,
            parents,
            edits,
            formref: label,
            carried: false,
            new_form: None,
            scripts: Vec::new(),
        },
        origin,
    ))
}

/// The load order's plugin files: never written.
pub(crate) fn protected(loaded: &Loaded) -> Vec<PathBuf> {
    loaded.lo.plugins.iter().map(|p| p.path.clone()).collect()
}

/// Refuses an output path the patch writer would refuse (`guard_target`: extension, a
/// load-order plugin, the game or user Data directory, an MO2 instance), before anything
/// is staged.
pub(crate) fn guard(command: &str, loaded: &Loaded, out: &Path) -> Result<(), EngineError> {
    let all = protected(loaded);
    // Matched by resolved identity, after the spelling and network-order refusals
    // (`load_order_matches`, as `guard_target` matches).
    let (_, hits) = load_order_matches(out, &all).map_err(|e| failed(command, e.to_string()))?;
    if !hits.is_empty() {
        // `place`, `build` and `revert` (A6 review, L3): only `set`, `new` and `clone`
        // write a loaded active file.
        return Err(failed(
            command,
            format!(
                "{} is a plugin of the load order: `{command}` does not yet write to a loaded active file (only `set`, `new` and `clone` do, with `active_file: true`); unload it, or write another file",
                out.display()
            ),
        ));
    }
    guard_target(out, &all).map_err(|e| failed(command, e.to_string()))
}

/// Whether the request names its output as the editor's active file (`active_file: true`;
/// `set`, `new` and `clone` only, ADR 0007 Amendment 4, owner decision 2026-11-05 item 7).
pub(crate) fn active_file_arg(args: &Value) -> bool {
    args.get("active_file").and_then(Value::as_bool) == Some(true)
}

/// An official master by name: one of the game's official order
/// (`kitkraft_io::plan::OFFICIAL_ORDER`), or any `Starfield.esm`, `Constellation.esm`,
/// `OldMars.esm`, `ShatteredSpace.esm`, `BlueprintShips-*.esm` or `SFBGS*.esm` (the rule
/// the workbench gate applies, ADR 0007 Amendment 4), compared without case.
pub(crate) fn official_master(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    let esm = n.ends_with(".esm");
    kitkraft_io::plan::OFFICIAL_ORDER
        .iter()
        .any(|o| o.eq_ignore_ascii_case(&n))
        || matches!(
            n.as_str(),
            "starfield.esm" | "constellation.esm" | "oldmars.esm" | "shatteredspace.esm"
        )
        || (esm && (n.starts_with("blueprintships-") || n.starts_with("sfbgs")))
}

/// [`guard`] for `set`, `new` and `clone` (A6, owner decision 2026-11-05 item 7). Without
/// `active` it is [`guard`]. With `active` (the request says `out` is the editor's active
/// file), `out` alone is taken off the protected list when it is a plugin of the load order
/// and all of these hold, so the editor can keep writing to the active file once it is
/// loaded:
///
/// 1. the load order came from an explicit `plugins` list, not an MO2 profile (a profile's
///    plugins live in Data or MO2, which `guard_location` refuses anyway; this holds even
///    where an MO2 keeps its mods elsewhere);
/// 2. `out` is in the load order once, as its **last** plugin (its own winner: no later
///    plugin hides the edit), and no loaded plugin names it as a master; "is" means the
///    same resolved identity as `guard_target` compares (`load_order_matches`: canonical as
///    far as it exists, `\\?\` stripped, case-insensitive), never the same spelling, and
///    `out`'s network, verbatim, device and stream spellings, and a load order with any
///    plugin named through a network path, are refused before any exemption; `out` must
///    also exist and canonicalize to exactly the last plugin's canonical path (a target
///    that does not fully resolve, such as a new `patch.esp` beside `Patch.esp` in a
///    case-sensitive folder, is refused, never exempted);
/// 3. `out` is not an official master by name ([`official_master`]), and no other loaded
///    plugin has its file name (A6 review, L1). The master flag does not matter: a
///    master-flagged file of the owner's own may be the active file (owner ruling
///    2026-11-07, item 3, which lifted the A6 review M1 refusal), as long as no loaded
///    plugin names it as a master (condition 2; by name without case, Unicode included, or
///    by a master name that resolves beside its plugin or in the Data dir to `out`'s
///    file; only a plain relative name, [`plain_master_name`], is resolved on disk, and an
///    earlier plugin naming any master by a path, a drive, a network or device spelling or
///    a reserved device name refuses the exemption unopened) and, when `out` is master-flagged, every plugin before it is master-flagged
///    too (the game loads masters first, `masters-load-first`; an unreadable header
///    counts as unflagged);
/// 4. its loaded header is readable (the plugin has an index), and the file on disk is
///    still the one loaded: same size, modification time and `TES4` flags as that header
///    (A6 re-review, RR-M1 and RR-L1; [`unchanged_on_disk`]), so a file that gained or lost
///    the master flag on disk since the load is refused until it is loaded again;
/// 5. `out` passes `guard_location` (never Data, the user Data folder, My Games, MO2, or
///    through a link), which `guard_target` still runs, unrelaxed.
///
/// Every other plugin of the load order stays protected. Returns the active file's path as
/// loaded when the exemption was used (the write is then in place: [`write_with`]'s
/// `in_place`, and the session's load order is read again after the write, [`reload`]).
pub(crate) fn guard_output(
    command: &str,
    loaded: &Loaded,
    out: &Path,
    active: bool,
) -> Result<Option<PathBuf>, EngineError> {
    let all = protected(loaded);
    if !active {
        guard_target(out, &all).map_err(|e| failed(command, e.to_string()))?;
        return Ok(None);
    }
    // The same comparison `guard_target` makes (unc-guard): `out`'s non-local spellings,
    // links and network resolution, and any load-order entry named through a network
    // path, are refused before any exemption; the entries `out` matches are those with its
    // resolved identity (`identity_key`), never a spelling comparison.
    let (_, hits) = load_order_matches(out, &all).map_err(|e| failed(command, e.to_string()))?;
    if hits.is_empty() {
        guard_target(out, &all).map_err(|e| failed(command, e.to_string()))?;
        return Ok(None);
    }
    let refuse = |why: String| {
        failed(
            command,
            format!(
                "{} is a plugin of the load order and cannot be written as the active file: {why}",
                out.display()
            ),
        )
    };
    if loaded.source != "plugins" {
        return Err(refuse(format!(
            "the load order was read from an MO2 profile ({}); the active file is exempt only in a load order given as `plugins`",
            loaded.source
        )));
    }
    let last = all.len().saturating_sub(1);
    if hits != [last] {
        return Err(refuse(
            "only the last plugin of the load order can be the active file (a later plugin would hide the edit, or depend on it)"
                .to_string(),
        ));
    }
    // The identity key keeps the caller's spelling (lowercased) for a part that does not
    // exist, so in a case-sensitive folder a new `patch.esp` keys like a loaded
    // `Patch.esp`. Exempt only a target that exists and fully resolves to the last
    // plugin's own file (exact canonical paths); anything else is left to `guard_target`,
    // which refuses it on the safe side (A6 review, unc-guard rebase item 2).
    let same_file = all.get(last).is_some_and(|p| {
        matches!(
            (std::fs::canonicalize(out), std::fs::canonicalize(p)),
            (Ok(a), Ok(b)) if a == b
        )
    });
    if !same_file {
        guard_target(out, &all).map_err(|e| failed(command, e.to_string()))?;
        return Ok(None);
    }
    let name = super::file_name(out);
    if official_master(&name) {
        return Err(refuse(format!("{name} is an official master")));
    }
    let plugin = loaded
        .lo
        .plugins
        .get(last)
        .ok_or_else(|| refuse("it is not in the load order".to_string()))?;
    // The loaded header is what `unchanged_on_disk` compares the file's `TES4` flags with;
    // without one there is nothing to compare, so the file is not exempt. Its master flag
    // is no refusal (owner ruling 2026-11-07, item 3).
    let Some(header) = plugin.header() else {
        return Err(refuse(format!(
            "its header could not be read ({})",
            plugin.error.as_deref().unwrap_or("no TES4")
        )));
    };
    // The game loads master-flagged plugins before the others (`masters-load-first`,
    // `handlers/testplan.rs`): after an unflagged plugin, a master-flagged active file
    // would load before it in the game, so it would not be its own winner, and could gain
    // it as a master that loads after it. A plugin whose header is unreadable counts as
    // unflagged.
    if header.flags.master() {
        if let Some(p) = loaded
            .lo
            .plugins
            .iter()
            .take(last)
            .find(|p| !p.header().is_some_and(|h| h.flags.master()))
        {
            return Err(refuse(format!(
                "{} has no master flag, and a master-flagged active file must follow only master-flagged plugins: the game loads masters first (masters-load-first)",
                p.name
            )));
        }
    }
    unchanged_on_disk(command, plugin)?;
    if let Some(p) = loaded
        .lo
        .plugins
        .iter()
        .take(last)
        .find(|p| p.name.eq_ignore_ascii_case(&name))
    {
        return Err(refuse(format!(
            "another plugin of the load order has the same file name ({})",
            p.path.display()
        )));
    }
    // A plugin listing it as a master (loaded before it: an ordering error) would see
    // its FormIDs change meaning. The name is compared without case (Unicode, not only
    // ASCII), and a master name that resolves beside the plugin naming it, or in the Data
    // dir, to `out`'s own file is the same master whatever its spelling (A6 fix round, L1:
    // NTFS folds non-ASCII case). The `MAST` string comes from the plugin's bytes, so the
    // file system is touched only for a plain relative name (A6 fix round 3): a path, a
    // drive, a network or device spelling or a reserved device name refuses the
    // exemption without being opened.
    let lower = name.to_lowercase();
    let canonical_out = std::fs::canonicalize(out).ok();
    for p in loaded.lo.plugins.iter().take(last) {
        for m in p.masters.iter().map(|m| m.name.as_str()) {
            if m.eq_ignore_ascii_case(&name) || m.to_lowercase() == lower {
                return Err(refuse(format!("{} names {name} as a master", p.name)));
            }
            let by_path = || {
                refuse(format!(
                    "{} names a master by a path ({m}); a master is named by its file name",
                    p.name
                ))
            };
            if !plain_master_name(m) {
                return Err(by_path());
            }
            let beside = p.path.parent().map(|d| d.join(m));
            let in_data = loaded.data_dir.as_ref().map(|d| d.join(m));
            for c in [beside, in_data].into_iter().flatten() {
                if crate::edit::patch::non_local_spelling(&c).is_some() {
                    return Err(by_path());
                }
                if canonical_out.is_some() && std::fs::canonicalize(&c).ok() == canonical_out {
                    return Err(refuse(format!("{} names {name} as a master", p.name)));
                }
            }
        }
    }
    let path = plugin.path.clone();
    let others: Vec<PathBuf> = all.into_iter().take(last).collect();
    // Every entry, `out`'s own included, was network-checked by `load_order_matches`
    // above; this repeats `guard_target`'s refusals for the other plugins only.
    guard_target(out, &others).map_err(|e| failed(command, e.to_string()))?;
    Ok(Some(path))
}

/// Whether a `MAST` name read from a plugin is a plain relative name that may be resolved
/// on disk ([`guard_output`]): not empty, no `:`, no prefix or root, every component a
/// normal name or `..`, and none a reserved device name (`CON`, `PRN`, `AUX`, `NUL`,
/// `CONIN$`, `CONOUT$`, `COM0`-`COM9`, `LPT0`-`LPT9`, `COM¹`-`COM³`, `LPT¹`-`LPT³`, with
/// or without an extension, any case). On the Windows 11 build tested none of these but
/// `NUL` maps to a device inside a directory (`std::path::absolute` turns only
/// `<dir>\NUL` into `\\.\NUL`); older Windows 10 builds mapped more, so the whole list is
/// refused on the safe side.
pub(crate) fn plain_master_name(m: &str) -> bool {
    use std::path::Component;
    let reserved = |c: &str| {
        let stem = c.split('.').next().unwrap_or(c).trim_end().to_uppercase();
        let numbered = stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|n| {
                let mut digits = n.chars();
                matches!(
                    (digits.next(), digits.next()),
                    (Some('0'..='9' | '¹' | '²' | '³'), None)
                )
            });
        numbered
            || matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
    };
    // A `\` is a separator only on Windows; elsewhere a name holding one is a Windows path.
    !m.is_empty()
        && !m.contains(':')
        && (cfg!(windows) || !m.contains('\\'))
        && Path::new(m).components().all(|c| match c {
            Component::Normal(n) => !reserved(&n.to_string_lossy()),
            Component::ParentDir => true,
            _ => false,
        })
}

/// Refuses the exemption when the active file on disk is not the file the session loaded
/// (A6 re-review, RR-M1 and RR-L1): its size or modification time differ from the index's,
/// or its `TES4` flags differ from the loaded header's (a master flag set or cleared since
/// the load included).
/// Another tool rewrote it, so the session's offsets may name other records: it must be
/// loaded again. The session's own writes re-read the load order ([`reload`]), so they
/// never trip this.
fn unchanged_on_disk(
    command: &str,
    plugin: &kitkraft_doc::LoadedPlugin,
) -> Result<(), EngineError> {
    use std::io::Read;
    let changed = |why: String| {
        failed(
            command,
            format!(
                "{command}.active-file-changed: {} changed on disk since it was loaded ({why}); load it again before writing to it",
                plugin.path.display()
            ),
        )
    };
    let index = plugin
        .index
        .as_deref()
        .ok_or_else(|| changed("it has no index".to_string()))?;
    let (size, mtime_ns) = kitkraft_doc::index::file_stamp(&plugin.path)
        .map_err(|e| changed(format!("it cannot be read: {e}")))?;
    if size != index.size {
        return Err(changed(format!(
            "{size} bytes now, {} when loaded",
            index.size
        )));
    }
    if index.mtime_ns != 0 && mtime_ns != index.mtime_ns {
        return Err(changed("its modification time differs".to_string()));
    }
    let mut head = [0u8; 12];
    File::open(&plugin.path)
        .and_then(|mut f| f.read_exact(&mut head))
        .map_err(|e| changed(format!("its header cannot be read: {e}")))?;
    let flags = head
        .get(8..12)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(u32::from_le_bytes);
    if head.get(0..4) != Some(b"TES4".as_slice()) || flags != Some(index.header.flags.0) {
        return Err(changed("its TES4 header differs".to_string()));
    }
    Ok(())
}

/// Reads the session's load order again after a write into its last plugin (the active
/// file, [`guard_output`]), so later commands do not read the old bytes through stale
/// offsets. The load order is the same `plugins` list (each by its path), language, index
/// cache choice, Data dir and `from_save`. When the reload fails the stale load order is dropped (later
/// commands then ask for a load) and the returned note says so.
pub(crate) fn reload(session: &mut Session, command: &str) -> Option<String> {
    let args = {
        let loaded = session.get::<Loaded>()?;
        let mut args = Map::new();
        let plugins: Vec<Value> = loaded
            .lo
            .plugins
            .iter()
            .map(|p| json!(p.path.display().to_string()))
            .collect();
        args.insert("plugins".into(), Value::Array(plugins));
        args.insert("language".into(), json!(loaded.language));
        args.insert("no_cache".into(), json!(loaded.cache_dir.is_none()));
        if let Some(d) = &loaded.data_dir {
            args.insert("data_dir".into(), json!(d.display().to_string()));
        }
        if let Some(save) = &loaded.from_save {
            args.insert("from_save".into(), json!(save));
        }
        Value::Object(args)
    };
    match super::load::load(&args, Some(&*session), command) {
        Ok((loaded, listing, _)) => {
            session.set_load_order(Some(listing));
            session.insert(loaded);
            None
        }
        Err(e) => {
            session.remove::<Loaded>();
            session.set_load_order(None);
            Some(format!(
                "the load order could not be read again after writing its last plugin ({e}); it was dropped from the session: load it again"
            ))
        }
    }
}

/// Writes `copies` into `into` with the load order's string tables, protected files and
/// plugin flags.
pub(crate) fn write(
    command: &str,
    loaded: &mut Loaded,
    into: &Path,
    copies: Vec<Copy>,
    allow_untyped_lstrings: bool,
) -> Result<PatchReport, EngineError> {
    write_with(
        command,
        loaded,
        into,
        copies,
        Vec::new(),
        allow_untyped_lstrings,
        None,
    )
}

/// [`write`] with new placed references (`place`, spec v3 `[[place]]`). `in_place` (only
/// after [`guard_output`] exempted the active file; its path as loaded): copies read from
/// that file, the active file's own versions, are edited in place.
pub(crate) fn write_with(
    command: &str,
    loaded: &mut Loaded,
    into: &Path,
    copies: Vec<Copy>,
    new_refs: Vec<NewRef>,
    allow_untyped_lstrings: bool,
    in_place: Option<&Path>,
) -> Result<PatchReport, EngineError> {
    let protected = protected(loaded);
    let plugin_flags: Vec<(String, u32)> = loaded
        .lo
        .plugins
        .iter()
        .filter_map(|p| Some((p.name.to_ascii_lowercase(), p.header()?.flags.0)))
        .collect();
    let strings = TableStrings(RefCell::new(&mut loaded.strings));
    let options = PatchOptions {
        strings: &strings,
        protected,
        plugin_flags,
        limits: DecodeLimits::default(),
        allow_untyped_lstrings,
        in_place: in_place.map(Path::to_path_buf),
    };
    write_patch_with(into, copies, new_refs, &options).map_err(|e| match e {
        PatchError::Field(f) => EngineError::InvalidArgs {
            command: command.to_string(),
            violations: vec![Violation {
                pointer: format!("/fields/{}", f.path.replace('~', "~0").replace('/', "~1")),
                rule: f.rule,
                message: f.to_string(),
            }],
        },
        PatchError::Refused(m) | PatchError::Io(m) => failed(command, m),
    })
}

/// A private directory under the OS temp dir where `set` and `build` write and validate a
/// plugin before it replaces its destination; removed on drop. Directories a crashed run
/// left behind (older than a day) are removed when a new one is made, and so are `build`'s
/// old `.kitkraft-build-<pid>` directories next to the destination (the layout before
/// 2026-10-14, older than an hour).
pub(crate) struct Staged {
    dir: PathBuf,
    /// The staged plugin (same file name as the destination).
    pub path: PathBuf,
    /// The destination (scripts the plugin attaches are looked up next to it).
    pub target: PathBuf,
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Prefix of staging directories in the OS temp dir.
const STAGE_PREFIX: &str = "kitkraft-stage-";

static STAGE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Removes directories in `dir` whose name starts with `prefix` and that were last
/// modified more than `age` ago (best effort).
pub(crate) fn remove_stale(dir: &Path, prefix: &str, age: std::time::Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let named = entry.file_name().to_string_lossy().starts_with(prefix);
        let old = entry
            .metadata()
            .ok()
            .filter(std::fs::Metadata::is_dir)
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|e| e > age);
        if named && old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Stages `out`: a fresh directory under the OS temp dir holding `out`'s file name, with a
/// copy of `out` when `copy_existing` and it exists.
pub(crate) fn stage(command: &str, out: &Path, copy_existing: bool) -> Result<Staged, EngineError> {
    let temp = std::env::temp_dir();
    remove_stale(
        &temp,
        STAGE_PREFIX,
        std::time::Duration::from_secs(24 * 3600),
    );
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    remove_stale(
        parent,
        ".kitkraft-build-",
        std::time::Duration::from_secs(3600),
    );
    let name = out
        .file_name()
        .ok_or_else(|| failed(command, format!("{}: not a file path", out.display())))?;
    let n = STAGE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = temp.join(format!("{STAGE_PREFIX}{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| failed(command, format!("{}: {e}", dir.display())))?;
    let staged = Staged {
        path: dir.join(name),
        dir,
        target: out.to_path_buf(),
    };
    if copy_existing && out.exists() {
        std::fs::copy(out, &staged.path)
            .map_err(|e| failed(command, format!("staging {}: {e}", out.display())))?;
    }
    Ok(staged)
}

/// Moves the staged plugin to `out`: copied next to `out` first, then renamed over it (the
/// staging dir may be on another volume; the rename stays on `out`'s).
pub(crate) fn install(command: &str, staged: &Staged, out: &Path) -> Result<(), EngineError> {
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = out.with_file_name(format!(".{name}.kitkraft-{}.tmp", std::process::id()));
    // A missing destination folder is created (`out` was guarded before staging).
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| failed(command, format!("{}: {e}", parent.display())))?;
    }
    std::fs::copy(&staged.path, &temp)
        .and_then(|_| std::fs::rename(&temp, out))
        .map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            failed(
                command,
                format!("moving the plugin to {}: {e}", out.display()),
            )
        })
}

/// Runs `validate` on the staged plugin against the load order.
pub(crate) fn check(
    command: &str,
    loaded: &Loaded,
    staged: &Staged,
) -> Result<validate::Report, EngineError> {
    let options = validate::Options {
        plugin: Some(staged.path.clone()),
        plugins: protected(loaded),
        indexed: super::validate::indexes(&loaded.lo),
        scripts_home: Some(staged.target.clone()),
        ..validate::Options::default()
    };
    validate::validate(&options).map_err(|m| failed(command, format!("validating the output: {m}")))
}

/// `{rule} {record}: {message}` of up to 20 findings.
pub(crate) fn describe(findings: &[&validate::Finding]) -> String {
    findings
        .iter()
        .take(20)
        .map(|f| {
            format!(
                "{} {}: {}",
                f.rule,
                f.record.as_deref().unwrap_or("-"),
                f.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The `validate` part of a `set` or `build` result.
pub(crate) fn validate_json(checked: &validate::Report) -> Value {
    json!({
        "passed": checked.passed(),
        "errors": checked.count(Severity::Error),
        "warnings": checked.count(Severity::Warning),
        "findings": checked.findings,
        "rules": checked.rules,
    })
}

/// The envelope for a written patch (shared with `build`).
pub(crate) fn outcome(
    report: &PatchReport,
    origins: &[Origin],
    mut limitations: Vec<String>,
) -> Outcome {
    let target = report
        .target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut evidence = Vec::new();
    let mut records = Vec::new();
    let mut origins = origins.iter();
    for r in &report.records {
        let mut json = r.to_json();
        if !r.parent {
            if let Some(o) = origins.next() {
                if let Some(obj) = json.as_object_mut() {
                    obj.insert(
                        "copied_from".into(),
                        json!({
                            "plugin": o.plugin, "load_index": o.load_index, "offset": o.offset,
                            "record": o.label, "versions_in_load_order": o.versions,
                        }),
                    );
                }
                evidence.push(Evidence {
                    file: o.plugin.clone(),
                    offset: o.offset,
                    length: o.length,
                    record: Some(o.label.clone()),
                    subrecord: None,
                });
            }
        }
        if let (Some(offset), Some(length)) = (r.offset, r.length) {
            evidence.push(Evidence {
                file: target.clone(),
                offset,
                length,
                record: Some(format!("{}:{:08X}", r.signature, r.form_id)),
                subrecord: None,
            });
        }
        for (sig, offset, length) in &r.edited_subrecords {
            if let Some(offset) = offset {
                evidence.push(Evidence {
                    file: target.clone(),
                    offset: *offset,
                    length: *length,
                    record: Some(format!("{}:{:08X}", r.signature, r.form_id)),
                    subrecord: Some(sig.clone()),
                });
            }
        }
        records.push(json);
    }
    let changed: Vec<String> = report
        .records
        .iter()
        .filter(|r| !r.parent)
        .map(|r| {
            let fields: Vec<String> = r
                .changes
                .iter()
                .map(|c| format!("{} {} -> {}", c.path, c.before, c.after))
                .collect();
            format!(
                "{} {} ({}): {}",
                r.signature,
                r.formref,
                r.editor_id.as_deref().unwrap_or("-"),
                if r.source.starts_with("new (") {
                    "new".to_string()
                } else if fields.is_empty() {
                    "copied".to_string()
                } else {
                    fields.join(", ")
                }
            )
        })
        .collect();
    let summary = format!(
        "{target}: {} ({} bytes, masters {}); {}",
        if report.created { "created" } else { "updated" },
        report.bytes,
        report.masters.join(", "),
        changed.join("; ")
    );
    for l in &report.limitations {
        if !limitations.contains(l) {
            limitations.push(l.clone());
        }
    }
    Outcome {
        data: json!({
            "target": report.target.display().to_string(),
            "file": target,
            "created": report.created,
            "masters": report.masters,
            "flags": format!("0x{:08X}", report.flags),
            "bytes": report.bytes,
            "records": records,
            "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    }
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let form = str_arg(ID, args, "form")?;
    let into = PathBuf::from(str_arg(ID, args, "into")?);
    let fields = args
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| failed(ID, "argument `fields` is missing"))?;
    // With scripts to attach, or a material swap, `fields` may be empty.
    let swaps = args.get("swap_overrides").filter(|s| !s.is_null());
    let edits = if fields.is_empty()
        && (args.get("scripts").is_some_and(|s| !s.is_null()) || swaps.is_some())
    {
        Vec::new()
    } else {
        edits(ID, "/fields", fields)?
    };
    let allow_untyped = args
        .get("allow_untyped_lstrings")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let dry_run = super::preview::requested(args);
    let schema = super::load::schema_of(session);
    let loaded = acquire(session, args, ID)?;
    let in_place = guard_output(ID, loaded, &into, active_file_arg(args))?;
    let limits = DecodeLimits::default();
    let g = find_form(ID, loaded, form, args.get("plugin").and_then(Value::as_str))?;
    let unrelated = safe_source(ID, loaded, g)?;
    // Scripts to attach (M4.E9): refused here when a script or property is unknown.
    let scripts = super::scripts::from_args(ID, args, loaded, &into)?;
    if edits.is_empty() && scripts.scripts.is_empty() && swaps.is_none() {
        return Err(failed(
            ID,
            "nothing to set: give fields, scripts or swap_overrides",
        ));
    }
    let (mut copy, origin) = winning_copy(ID, loaded, g, edits, &limits)?;
    // The material swap, as a whole-REFL edit of the LMSW's current REFL (KK-019).
    let swap_report = if let Some(swaps) = swaps {
        let (edit, report) = crate::edit::swaps::convert(ID, &copy.record, swaps, &copy.edits)?;
        copy.edits.push(edit);
        Some(report)
    } else {
        None
    };
    copy.scripts = scripts.scripts;
    let mut limitations = loaded.limitations.clone();
    extend_once(&mut limitations, unrelated);
    extend_once(&mut limitations, scripts.limitations);

    // Write into a staged copy of `into`, run the record rules on it (as `build` does),
    // and replace `into` only when the records this call wrote pass.
    let staged = stage(ID, &into, true)?;
    let mut report = write_with(
        ID,
        loaded,
        &staged.path,
        vec![copy],
        Vec::new(),
        allow_untyped,
        in_place.as_deref(),
    )?;
    let checked = check(ID, loaded, &staged)?;
    let written: Vec<String> = report
        .records
        .iter()
        .map(|r| format!("{}:{:08X}", r.signature, r.form_id))
        .collect();
    let (blocking, other): (Vec<&validate::Finding>, Vec<&validate::Finding>) = checked
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .partition(|f| f.record.as_ref().is_none_or(|r| written.contains(r)));
    if dry_run {
        // M7.S6: the same write and validation, reported without installing it.
        let mut notes = Vec::new();
        let mut before_evidence = Vec::new();
        let before = super::preview::before(schema, loaded, g, &mut before_evidence, &mut notes);
        let after = super::preview::after(schema, loaded, &staged, &report, &into, &mut notes);
        report.target = into.clone();
        let mut result = outcome(&report, &[origin], limitations);
        result.evidence.extend(before_evidence);
        super::preview::attach(
            &mut result,
            before,
            after,
            &report,
            &checked,
            &blocking,
            notes,
        );
        if let Some(obj) = result.data.as_object_mut() {
            if !scripts.reports.is_empty() {
                obj.insert("scripts".into(), Value::Array(scripts.reports));
            }
            if let Some(report) = swap_report {
                obj.insert("swap_overrides".into(), Value::Array(report));
            }
        }
        return Ok(result);
    }
    if !blocking.is_empty() {
        return Err(failed(
            ID,
            format!(
                "the written record fails validate, so {} was not changed: {}",
                into.display(),
                describe(&blocking)
            ),
        ));
    }
    if !other.is_empty() {
        limitations.push(format!(
            "{} has {} validate error(s) on records this call did not write: {}",
            into.display(),
            other.len(),
            describe(&other)
        ));
    }
    install(ID, &staged, &into)?;
    report.target = into.clone();
    let mut result = outcome(&report, &[origin], limitations);
    if let Some(obj) = result.data.as_object_mut() {
        obj.insert("validate".into(), validate_json(&checked));
        if !scripts.reports.is_empty() {
            obj.insert("scripts".into(), Value::Array(scripts.reports));
        }
        if let Some(report) = swap_report {
            obj.insert("swap_overrides".into(), Value::Array(report));
        }
    }
    // The active file is the load order's last plugin: read it again (A6).
    if in_place.is_some() {
        if let Some(note) = reload(session, ID) {
            result.limitations.push(note);
        }
    }
    Ok(result)
}
