//! `load` (M2.D8): resolve a load order from an MO2 profile or an explicit plugin list,
//! index every plugin (through the on-disk cache), and keep it in the session.
//!
//! The other load-order commands (`inspect`, `query`, `conflicts`) take the same
//! `profile` / `plugins` / `data_dir` arguments: given, they load first (and replace the
//! session's load order); absent, they use the session's. The CLI runs each command in a
//! fresh session, so `kit` users pass `--profile` to each; an MCP session loads once.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use kitkraft_doc::index::RecordLoc;
use kitkraft_doc::strings::{DirSource, StringSource, StringTables};
use kitkraft_doc::{CacheUse, IndexCache, LoadOptions, LoadOrder, PluginSpec, Scale};
use kitkraft_io::{plan_profile, Active, ProfileOptions};
use serde_json::{json, Value};

use super::{failed, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

const ID: &str = "load";

/// Stated whenever runtime IDs are shown.
pub(crate) const RUNTIME_ASSUMED: &str = "runtime slots and IDs follow the assumption \
     `runtime-slots-by-scale` (docs/load-order.md): not yet observed in game or in the CK";

/// A loaded load order, kept in the session.
pub(crate) struct Loaded {
    /// The model.
    pub lo: LoadOrder,
    /// Where it came from: `profile:<dir>` or `plugins`.
    pub source: String,
    /// Requested string-table language.
    pub language: String,
    /// Limitations every command using this load order repeats: assumptions, ordering
    /// errors, plugins that failed to load or have no file, unresolved records.
    pub limitations: Vec<String>,
    /// Active plugins (per the profile) with no file: not in `lo`, not loaded by the game
    /// either. Always empty for an explicit `plugins` list (a missing file is an error).
    pub missing_active_plugins: Vec<String>,
    /// String tables of the load order's plugins, read on first use.
    pub strings: StringTables,
    /// Every EditorID of the load order, built on the first EditorID lookup and kept with
    /// the load order (one scan of the indexes per session, not one per lookup).
    pub edids: OnceLock<EdidIndex>,
    /// The index cache directory the load order was read through (`None` with
    /// `no_cache`); the reverse link index is cached there too.
    pub cache_dir: Option<PathBuf>,
    /// The reverse link index (`refs --incoming`, M4.D1), built or read from the cache on
    /// first use and kept with the load order.
    pub backrefs: OnceLock<kitkraft_doc::backrefs::Backrefs>,
}

/// The EditorIDs of a load order's indexes: `(hash of the ASCII-lowercased EditorID, plugin,
/// record)`, sorted, so a lookup is a binary search plus a check of the few entries with
/// that hash. Hits come in load order, then index order (what a scan would find first).
#[derive(Debug, Default)]
pub(crate) struct EdidIndex(Vec<(u64, u16, u32)>);

/// FNV-1a over the ASCII-lowercased bytes: EditorIDs compare ASCII case-insensitively.
fn edid_hash(edid: &str) -> u64 {
    edid.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b.to_ascii_lowercase())).wrapping_mul(0x0100_0000_01b3)
    })
}

impl EdidIndex {
    fn build(lo: &LoadOrder) -> EdidIndex {
        let mut entries = Vec::new();
        for (i, p) in lo.plugins.iter().enumerate() {
            let (Ok(plugin), Some(idx)) = (u16::try_from(i), p.index.as_deref()) else {
                continue;
            };
            for (r, loc) in idx.records.iter().enumerate() {
                if let (Some(e), Ok(r)) = (idx.edid(loc), u32::try_from(r)) {
                    entries.push((edid_hash(e), plugin, r));
                }
            }
        }
        entries.sort_unstable();
        EdidIndex(entries)
    }
}

impl Loaded {
    /// Whether the winning version of `form` is a safe source to copy
    /// (`kitkraft_doc::source`): problems that cannot touch that form are `unrelated`.
    pub fn source_check(&self, form: kitkraft_doc::GlobalForm) -> kitkraft_doc::SourceCheck {
        self.lo.source_check(form, &self.missing_active_plugins)
    }

    /// The records whose EditorID is `edid` (ASCII case-insensitive): `(plugin, record)` in
    /// load order, then index order.
    pub fn edid_hits(&self, edid: &str) -> Vec<(u16, &RecordLoc)> {
        let index = self.edids.get_or_init(|| EdidIndex::build(&self.lo));
        let h = edid_hash(edid);
        let start = index.0.partition_point(|(k, _, _)| *k < h);
        index
            .0
            .get(start..)
            .unwrap_or_default()
            .iter()
            .take_while(|(k, _, _)| *k == h)
            .filter_map(|(_, p, r)| {
                let loc = self.lo.record(*p, *r)?;
                self.lo
                    .edid(*p, loc)
                    .is_some_and(|e| e.eq_ignore_ascii_case(edid))
                    .then_some((*p, loc))
            })
            .collect()
    }
}

/// Test-only: definitions to decode with instead of the builtin ones (a `reference`
/// member, which no builtin definition has yet; M3.D3).
#[cfg(test)]
pub(crate) struct SchemaOverride(pub &'static kitkraft_schema::Schema);

/// The schema `inspect` and `query` decode with: the builtin definitions (tests may
/// install a [`SchemaOverride`] in the session).
pub(crate) fn schema_of(session: &Session) -> Option<&'static kitkraft_schema::Schema> {
    #[cfg(test)]
    if let Some(o) = session.get::<SchemaOverride>() {
        return Some(o.0);
    }
    let _ = session;
    kitkraft_schema::builtin().ok()
}

pub(crate) fn has_load_args(args: &Value) -> bool {
    args.get("profile").is_some() || args.get("plugins").is_some()
}

/// The session's load order, loading it first when `args` name one.
pub(crate) fn acquire<'s>(
    session: &'s mut Session,
    args: &Value,
    command: &str,
) -> Result<&'s mut Loaded, EngineError> {
    if has_load_args(args) {
        let (loaded, listing, _) = load(args, Some(&*session), command)?;
        session.set_load_order(Some(listing));
        session.insert(loaded);
    }
    session.get_mut::<Loaded>().ok_or_else(|| {
        failed(
            command,
            "no load order: pass --profile <MO2 profile dir> or --plugins <file>... \
             (or run `load` first in an MCP session)",
        )
    })
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    if !has_load_args(args) {
        return Err(failed(
            ID,
            "pass `profile` (an MO2 profile dir) or `plugins`",
        ));
    }
    let (loaded, listing, evidence) = load(args, Some(&*session), ID)?;
    let failed_plugins: Vec<String> = loaded
        .lo
        .plugins
        .iter()
        .filter(|p| p.error.is_some())
        .map(|p| p.name.clone())
        .collect();
    let limitations = loaded.limitations.clone();
    session.set_load_order(Some(listing.clone()));
    session.insert(loaded);
    Ok(Outcome {
        data: listing,
        coverage: Coverage {
            status: CoverageStatus::Partial,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: failed_plugins,
        },
        limitations,
        evidence,
    })
}

fn cache_label(c: CacheUse) -> &'static str {
    match c {
        CacheUse::Hit => "hit",
        CacheUse::Built => "built",
        CacheUse::Uncached => "uncached",
    }
}

fn scale_label(s: Option<Scale>) -> Value {
    s.map_or(Value::Null, |s| json!(s.as_str()))
}

/// Raw FormIDs listed per plugin for records resolved by an assumption.
const ASSUMED_LISTED: usize = 10;

/// Up to [`ASSUMED_LISTED`] records of plugin `index` resolved by an assumption, as
/// `{raw_form_id, form, basis}`.
fn assumed_examples(lo: &LoadOrder, index: u16) -> Vec<Value> {
    let Some(p) = lo.plugin(index) else {
        return Vec::new();
    };
    let Ok(map) = p.map.as_ref() else {
        return Vec::new();
    };
    p.records()
        .iter()
        .filter_map(|r| {
            let (g, basis) = map.resolve_detailed(r.form_id).ok()?;
            basis.assumed().then(|| {
                json!({
                    "raw_form_id": format!("{:08X}", r.form_id),
                    "form": lo.form_label(g),
                    "basis": basis.as_str(),
                })
            })
        })
        .take(ASSUMED_LISTED)
        .collect()
}

/// Load per `args`: the session value (whose `limitations` every command over it repeats),
/// its JSON listing, and evidence. A bare name in `plugins` resolves through
/// [`super::resolve`] (the current load order in `session`, the working directory, the
/// Data dir).
pub(crate) fn load(
    args: &Value,
    session: Option<&Session>,
    command: &str,
) -> Result<(Loaded, Value, Vec<Evidence>), EngineError> {
    let started = Instant::now();
    let language = args
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("en")
        .to_string();
    // An explicit `data_dir` wins; `KITKRAFT_DATA_DIR` is the default for a plugin list,
    // and for a profile only when its MO2 instance names no game (gamePath).
    let explicit_data_dir = args
        .get("data_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let data_dir = super::data_dir(args);
    let mut limitations = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // name -> (why active, provider)
    let mut why: Vec<(String, String, String)> = Vec::new();
    let mut inactive: Vec<String> = Vec::new();
    let mut missing_active: Vec<String> = Vec::new();
    type Picked = (
        Vec<PluginSpec>,
        String,
        Option<String>,
        Box<dyn StringSource>,
    );
    let (specs, source, profile_name, string_source): Picked =
        match (args.get("profile"), args.get("plugins")) {
            (Some(_), Some(_)) => {
                return Err(failed(command, "pass `profile` or `plugins`, not both"));
            }
            (Some(_), None) => {
                let dir = str_arg(command, args, "profile")?;
                let planned = plan_profile(
                    Path::new(dir),
                    &ProfileOptions {
                        data_dir: explicit_data_dir.clone(),
                    },
                );
                let pl = match (planned, &explicit_data_dir, &data_dir) {
                    (Ok(pl), _, _) => pl,
                    (Err(first), None, Some(env_dir)) => plan_profile(
                        Path::new(dir),
                        &ProfileOptions {
                            data_dir: Some(env_dir.clone()),
                        },
                    )
                    .inspect(|_| {
                        limitations.push(format!(
                            "the MO2 instance names no game Data dir; used {} = {}",
                            super::DATA_DIR_ENV,
                            env_dir.display()
                        ));
                    })
                    .map_err(|_| failed(command, first.to_string()))?,
                    (Err(e), _, _) => return Err(failed(command, e.to_string())),
                };
                warnings.extend(pl.profile.warnings.iter().cloned());
                warnings.extend(pl.plan.warnings.iter().cloned());
                let mut assumed: Vec<String> = Vec::new();
                for p in &pl.plan.plugins {
                    match &p.active {
                        Some(a) => {
                            if matches!(
                                a,
                                Active::Hardcoded
                                    | Active::Unmanaged(_)
                                    | Active::BlueprintCompanion(_)
                            ) {
                                assumed.push(format!("{} ({})", p.name, a.label()));
                            }
                            let provider = p
                                .file
                                .as_ref()
                                .map_or_else(|| "missing".to_string(), |f| f.provider.label());
                            why.push((p.name.clone(), a.label(), provider));
                        }
                        None => inactive.push(p.name.clone()),
                    }
                }
                if !assumed.is_empty() {
                    limitations.push(format!(
                        "active by assumption (docs/load-order.md, \"Active plugins\"): {}",
                        assumed.join(", ")
                    ));
                }
                if !pl.plan.hardcoded_slots.is_empty() {
                    let moved = if pl.plan.moved.is_empty() {
                        String::new()
                    } else {
                        format!("; loadorder.txt lists {} later", pl.plan.moved.join(", "))
                    };
                    limitations.push(format!(
                        "load order by the assumption `hardcoded-official-slots` \
                         (docs/load-order.md, \"Order\"; each scale's order as the owner's saves \
                         of game 1.16.244.0 record it, how scales interleave not observed): the \
                         official plugins first in the game's order, then the others in \
                         loadorder.txt's order; official plugins without a plugins.txt entry: \
                         {}{moved}",
                        pl.plan.hardcoded_slots.join(", ")
                    ));
                }
                let missing = pl.plan.missing();
                if !missing.is_empty() {
                    limitations.push(format!(
                        "active plugins with no file (not loaded): {}",
                        missing.join(", ")
                    ));
                }
                missing_active = missing.iter().map(ToString::to_string).collect();
                (
                    pl.plan.specs(),
                    format!("profile:{dir}"),
                    Some(pl.profile.name.clone()),
                    Box::new(pl.overlay),
                )
            }
            (None, Some(list)) => {
                let names: Vec<String> = list
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut specs = Vec::with_capacity(names.len());
                let mut missing = Vec::new();
                for (i, name) in names.iter().enumerate() {
                    let pointer = format!("/plugins/{i}");
                    let path = super::resolve::resolve(
                        session,
                        data_dir.as_deref(),
                        command,
                        &pointer,
                        name,
                    )?;
                    if path.is_file() {
                        let name = kitkraft_doc::index::file_name_of(&path);
                        specs.push(PluginSpec { name, path });
                    } else {
                        missing.push(path.display().to_string());
                    }
                }
                if !missing.is_empty() {
                    return Err(failed(
                        command,
                        format!("plugin file(s) not found: {}", missing.join(", ")),
                    ));
                }
                for s in &specs {
                    why.push((s.name.clone(), "listed".into(), "path".into()));
                }
                // String tables: the explicit Data dir, else where the first plugin is
                // (for bare names that is the default Data dir).
                let dir = explicit_data_dir
                    .clone()
                    .or_else(|| {
                        specs
                            .first()
                            .and_then(|s| s.path.parent().map(Path::to_path_buf))
                    })
                    .unwrap_or_default();
                (
                    specs,
                    "plugins".to_string(),
                    None,
                    Box::new(DirSource { data_dir: dir }),
                )
            }
            (None, None) => return Err(failed(command, "pass `profile` or `plugins`")),
        };
    let no_cache = args.get("no_cache").and_then(Value::as_bool) == Some(true);
    let cache = if no_cache {
        None
    } else {
        IndexCache::default_location()
    };
    let cache_dir = cache.as_ref().map(|c| c.dir().display().to_string());
    let lo = LoadOrder::load(specs, &LoadOptions { cache, threads: 0 })
        .map_err(|e| failed(command, e.to_string()))?;
    for w in &warnings {
        limitations.push(format!("profile: {w}"));
    }
    let blueprint: Vec<&str> = lo
        .plugins
        .iter()
        .filter(|p| {
            p.header().is_some_and(|h| h.flags.blueprint()) && p.scale() == Some(Scale::Full)
        })
        .map(|p| p.name.as_str())
        .collect();
    if !blueprint.is_empty() {
        limitations.push(format!(
            "blueprint (0x800) full plugins take the full runtime slots after every other \
             full plugin, whatever their place in the load order (`runtime-slots-by-scale`, \
             as the owner's saves record): {}",
            blueprint.join(", ")
        ));
    }

    let mut evidence = Vec::new();
    let mut plugins = Vec::new();
    let (mut hits, mut records) = (0usize, 0u64);
    let mut missing_masters_total = 0usize;
    let mut full_style: Vec<String> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut duplicated: Vec<String> = Vec::new();
    let mut ordering: Vec<String> = Vec::new();
    let mut missing_links: Vec<String> = Vec::new();
    let mut failed_plugins: Vec<String> = Vec::new();
    for (i, p) in lo.plugins.iter().enumerate() {
        let index = u16::try_from(i).unwrap_or(u16::MAX);
        let header = p.header();
        if p.cache == CacheUse::Hit {
            hits = hits.saturating_add(1);
        }
        records = records.saturating_add(p.records().len() as u64);
        let assumed_self = lo.forms.full_style_self_count(index);
        let masked = lo.forms.masked_count(index);
        if assumed_self > 0 {
            full_style.push(if masked > 0 {
                format!("{} ({assumed_self}, {masked} masked)", p.name)
            } else {
                format!("{} ({assumed_self})", p.name)
            });
        }
        let bad = lo.forms.unresolved_count(index);
        if bad > 0 {
            unresolved.push(format!("{} ({bad})", p.name));
        }
        let duplicates = lo.forms.duplicate_count(index);
        if duplicates > 0 {
            duplicated.push(format!("{} ({duplicates})", p.name));
        }
        let missing = p.missing_masters();
        missing_masters_total = missing_masters_total.saturating_add(missing.len());
        if !missing.is_empty() {
            missing_links.push(format!("{} needs {}", p.name, missing.join(", ")));
        }
        let late = p.late_masters(index);
        if !late.is_empty() {
            ordering.push(format!(
                "{} loads before its master(s) {}",
                p.name,
                late.join(", ")
            ));
        }
        if let Some(e) = &p.error {
            failed_plugins.push(format!("{} ({e})", p.name));
        }
        let (active, provider) = why
            .iter()
            .find(|(n, _, _)| n.eq_ignore_ascii_case(&p.name))
            .map_or((Value::Null, Value::Null), |(_, a, f)| (json!(a), json!(f)));
        if header.is_some() {
            evidence.push(Evidence {
                file: p.name.clone(),
                offset: 0,
                length: 24,
                record: Some("TES4:00000000".into()),
                subrecord: None,
            });
        }
        plugins.push(json!({
            "index": index,
            "name": p.name,
            "path": p.path.display().to_string(),
            "active": active,
            "provider": provider,
            "flags": header.map(|h| format!("0x{:08X}", h.flags.0)),
            "flag_names": header.map(|h| h.flags.names()),
            "scale": scale_label(p.scale()),
            "localized": header.map(|h| h.flags.localized()),
            "form_version": header.map(|h| h.form_version),
            "masters": p.masters.iter().map(|m| json!({"name": m.name, "index": m.plugin})).collect::<Vec<_>>(),
            "missing_masters": missing,
            "late_masters": late,
            "resolvable": p.map.is_ok(),
            "resolve_error": p.map.as_ref().err().map(ToString::to_string),
            "records": p.records().len(),
            "unresolved_records": bad,
            "full_style_self_records": assumed_self,
            "full_style_self_masked_records": masked,
            "assumed_records": if assumed_self > 0 { assumed_examples(&lo, index) } else { Vec::new() },
            "duplicate_records": duplicates,
            "runtime_slot": p.runtime_slot.map(|s| s.label()),
            "cache": cache_label(p.cache),
            "error": p.error,
        }));
    }
    // Every command over this load order repeats these (`Loaded::limitations`).
    let mut shared = Vec::new();
    if !ordering.is_empty() {
        shared.push(format!(
            "ordering error: {} master link(s) point later in the load order, so overrides and \
             winners involving them may be wrong: {}",
            ordering.len(),
            ordering.join("; ")
        ));
    }
    if !failed_plugins.is_empty() {
        shared.push(format!(
            "{} plugin(s) failed to load and have no records: {}",
            failed_plugins.len(),
            failed_plugins.join("; ")
        ));
    }
    if !missing_links.is_empty() {
        shared.push(format!(
            "masters not in the load order (FormIDs of these plugins do not resolve): {}",
            missing_links.join("; ")
        ));
    }
    shared.push(RUNTIME_ASSUMED.to_string());
    if !full_style.is_empty() {
        shared.push(format!(
            "own records of small/medium plugins stored in the full class resolve to the plugin \
             itself by the assumption `full-style-self`, object IDs masked to the plugin's scale \
             (docs/load-order.md); each version says so in `resolved_by`, and `load` lists \
             examples per plugin in `assumed_records`: {}",
            full_style.join(", ")
        ));
    }
    if !duplicated.is_empty() {
        shared.push(format!(
            "records of one plugin that resolve to the same form (a masked full-style ID or a \
             repeated FormID; both kept, the later record last): {}",
            duplicated.join(", ")
        ));
    }
    if !unresolved.is_empty() {
        shared.push(format!(
            "records whose FormID does not resolve are left out of overrides and queries: {}",
            unresolved.join(", ")
        ));
    }
    shared.extend(limitations);
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let summary = format!(
        "{}: {} plugins loaded, {} records ({} versions resolved), {} missing master link(s), index cache {}/{} hits, {} ms",
        profile_name.as_deref().unwrap_or("plugins"),
        lo.plugins.len(),
        records,
        lo.forms.len(),
        missing_masters_total,
        hits,
        lo.plugins.len(),
        elapsed_ms
    );
    let listing = json!({
        "loaded": true,
        "source": source,
        "language": language,
        "plugins": plugins,
        "inactive": inactive,
        "missing_active_plugins": missing_active,
        "records": records,
        "versions": lo.forms.len(),
        "cache_dir": cache_dir,
        "cache_hits": hits,
        "index_ms": lo.index_ms,
        "table_ms": lo.table_ms,
        "elapsed_ms": elapsed_ms,
        "summary": summary,
    });
    Ok((
        Loaded {
            lo,
            source,
            strings: StringTables::new(&language, string_source),
            language,
            limitations: shared,
            missing_active_plugins: missing_active,
            edids: OnceLock::new(),
            cache_dir: cache_dir.map(PathBuf::from),
            backrefs: OnceLock::new(),
        },
        listing,
        evidence,
    ))
}

/// An lstring payload as JSON: `{text}` for inline text; for a string ID `{string_id, text,
/// string_table, string_offset}` when the table has it (with evidence at the string in the
/// table), else `{string_id}` and the reason in `notes`.
/// `at` is (plugin, `SIGN:FORMID` of the record, subrecord signature).
pub(crate) fn lstring_json(
    strings: &mut StringTables,
    at: (&str, &str, &str),
    data: &[u8],
    localized: bool,
    evidence: &mut Vec<Evidence>,
    notes: &mut Vec<String>,
) -> Value {
    use kitkraft_doc::lstring::{decode_lstring, LString};
    let (plugin, record, signature) = at;
    match decode_lstring(data, localized) {
        LString::Text(t) => json!({"text": t}),
        LString::Malformed { len } => {
            notes.push(format!(
                "{signature} of {record} in {plugin} is {len} bytes; a localized string ID is 4"
            ));
            json!({"malformed_len": len})
        }
        LString::Id(id) => match strings.lookup(plugin, id) {
            Ok(r) => {
                if let Some(offset) = r.offset {
                    evidence.push(Evidence {
                        file: r.origin.clone(),
                        offset,
                        length: r.length,
                        record: Some(record.to_string()),
                        subrecord: Some(signature.to_string()),
                    });
                }
                json!({
                    "string_id": format!("0x{id:08X}"),
                    "text": r.text,
                    "string_table": r.origin,
                    "string_kind": r.kind,
                    "string_offset": r.offset,
                })
            }
            Err(why) => {
                notes.push(why);
                json!({"string_id": format!("0x{id:08X}")})
            }
        },
    }
}
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]
pub(crate) mod tests {
    use super::*;
    use crate::handlers::testdata::{group, record, sub};
    use crate::Engine;

    /// A plugin: TES4 (flags, masters) and one top group per record type.
    pub(crate) fn plugin(
        flags: u32,
        masters: &[&str],
        records: &[(&[u8; 4], u32, Vec<u8>)],
    ) -> Vec<u8> {
        let flagged: Vec<(&[u8; 4], u32, u32, Vec<u8>)> = records
            .iter()
            .map(|(s, id, b)| (*s, 0, *id, b.clone()))
            .collect();
        plugin_flagged(flags, masters, &flagged)
    }

    /// As [`plugin`], with record flags: `(signature, record flags, FormID, body)`.
    pub(crate) fn plugin_flagged(
        flags: u32,
        masters: &[&str],
        records: &[(&[u8; 4], u32, u32, Vec<u8>)],
    ) -> Vec<u8> {
        let mut hedr = 0.96f32.to_le_bytes().to_vec();
        hedr.extend_from_slice(&1u32.to_le_bytes());
        hedr.extend_from_slice(&0x800u32.to_le_bytes());
        let mut tes4 = sub(b"HEDR", &hedr);
        for m in masters {
            tes4.extend(sub(b"MAST", &[m.as_bytes(), &[0]].concat()));
            tes4.extend(sub(b"DATA", &0u64.to_le_bytes()));
        }
        let mut out = record(b"TES4", flags, 0, &tes4);
        let mut sigs: Vec<&[u8; 4]> = Vec::new();
        for (s, _, _, _) in records {
            if !sigs.contains(s) {
                sigs.push(s);
            }
        }
        for s in sigs {
            let body: Vec<u8> = records
                .iter()
                .filter(|(x, _, _, _)| x == &s)
                .flat_map(|(x, f, id, b)| record(x, *f, *id, b))
                .collect();
            out.extend(group(s, 0, &body));
        }
        out
    }

    /// A temp dir with Base.esm (two weapons) and Patch.esm (overrides one, renames it).
    pub(crate) fn fixture(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kitkraft-engine-lo-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let weap = |edid: &str, full: &str| {
            [
                sub(b"EDID", &[edid.as_bytes(), &[0]].concat()),
                sub(b"FULL", &[full.as_bytes(), &[0]].concat()),
                sub(b"DATA", &[1, 2, 3, 4]),
            ]
            .concat()
        };
        std::fs::write(
            dir.join("Base.esm"),
            plugin(
                0x1,
                &[],
                &[
                    (
                        b"WEAP",
                        0x800,
                        weap("OldEarthHuntingRifle", "Old Earth Hunting Rifle"),
                    ),
                    (b"WEAP", 0x801, weap("Pistol", "Pistol")),
                    (b"KYWD", 0x802, sub(b"EDID", b"Kw\0")),
                ],
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("Patch.esm"),
            plugin(
                0x1,
                &["Base.esm"],
                &[
                    (
                        b"WEAP",
                        0x800,
                        weap("OldEarthHuntingRifle", "Old Soviet Marksman Rifle"),
                    ),
                    (b"WEAP", 0x801, weap("Pistol", "Pistol")),
                ],
            ),
        )
        .unwrap();
        dir
    }

    pub(crate) fn plugins_args(dir: &Path) -> Value {
        json!({
            "plugins": ["Base.esm", "Patch.esm"],
            "data_dir": dir.display().to_string(),
            "no_cache": true,
        })
    }

    /// Fable review (lane E fix round, finding 7): EditorID lookups use one map per load
    /// order, built on the first lookup and kept with it in the session; hits come in
    /// load order, case-insensitively, and match what a scan finds.
    #[test]
    fn editor_ids_are_mapped_once_per_session() {
        let dir = fixture("edids");
        let mut session = Session::new();
        acquire(&mut session, &plugins_args(&dir), "load").unwrap();
        let loaded = session.get::<Loaded>().unwrap();
        assert!(loaded.edids.get().is_none(), "built lazily");
        let hits: Vec<(u16, u32)> = loaded
            .edid_hits("oldearthhuntingrifle")
            .into_iter()
            .map(|(p, loc)| (p, loc.form_id))
            .collect();
        assert_eq!(hits, [(0, 0x800), (1, 0x0000_0800)]);
        let built = loaded.edids.get().map(|i| i.0.len());
        assert_eq!(built, Some(5), "every record with an EditorID, once");
        assert_eq!(loaded.edid_hits("KW").len(), 1);
        assert!(loaded.edid_hits("Nope").is_empty());
        assert_eq!(loaded.edids.get().map(|i| i.0.len()), built, "not rebuilt");
        // The commands that look EditorIDs up use it.
        let found = super::super::set::find_form("set", loaded, "pistol", None).unwrap();
        assert_eq!(loaded.lo.form_label(found), "Base.esm:000801");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_lists_plugins_and_keeps_them_in_the_session() {
        let dir = fixture("load");
        let engine = Engine::with_default_handlers();
        let result = engine.execute("load", plugins_args(&dir)).unwrap();
        let data = &result["data"];
        assert_eq!(data["loaded"], true);
        let plugins = data["plugins"].as_array().unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[1]["name"], "Patch.esm");
        assert_eq!(
            plugins[1]["masters"],
            json!([{"name": "Base.esm", "index": 0}])
        );
        assert_eq!(plugins[1]["missing_masters"], json!([]));
        assert_eq!(plugins[0]["scale"], "full");
        assert_eq!(plugins[0]["runtime_slot"], "00");
        assert_eq!(plugins[0]["records"], 3);
        assert_eq!(plugins[0]["cache"], "uncached");
        assert!(data["summary"]
            .as_str()
            .unwrap()
            .contains("2 plugins loaded"));
        assert_eq!(result["evidence"].as_array().unwrap().len(), 2);
        let limitations = result["limitations"].as_array().unwrap();
        assert!(limitations
            .iter()
            .any(|l| l.as_str().unwrap().contains("runtime-slots-by-scale")));
        // The session holds it: the listing and the typed load order.
        let listing = engine.with_session(|s| s.load_order().cloned()).unwrap();
        assert_eq!(listing["plugins"].as_array().unwrap().len(), 2);
        assert!(engine.with_session(|s| s.get::<Loaded>().is_some()));
    }

    #[test]
    fn load_errors() {
        let dir = fixture("load-err");
        let err = crate::execute("load", json!({})).unwrap_err();
        assert_eq!(err.kind(), "failed");
        let err = crate::execute(
            "load",
            json!({"plugins": ["Nope.esm"], "data_dir": dir.display().to_string()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Nope.esm"), "{err}");
        let err =
            crate::execute("load", json!({"profile": dir.display().to_string()})).unwrap_err();
        assert!(err.to_string().contains("not an MO2 profile"), "{err}");
        let err = crate::execute("load", json!({"profile": "x", "plugins": []})).unwrap_err();
        assert!(err.to_string().contains("not both"), "{err}");
        let err = crate::execute("load", json!({"bogus": 1})).unwrap_err();
        assert_eq!(err.kind(), "invalid_args");
    }

    #[test]
    fn missing_master_is_reported_not_guessed() {
        let dir = fixture("load-missing");
        let result = crate::execute(
            "load",
            json!({"plugins": ["Patch.esm"], "data_dir": dir.display().to_string(), "no_cache": true}),
        )
        .unwrap();
        let p = &result["data"]["plugins"][0];
        assert_eq!(p["missing_masters"], json!(["Base.esm"]));
        assert_eq!(p["resolvable"], false);
        assert!(p["resolve_error"].as_str().unwrap().contains("Base.esm"));
        assert_eq!(p["unresolved_records"], 2);
        // Every command over the load order repeats the missing master.
        let q = json!({"plugins": ["Patch.esm"], "data_dir": dir.display().to_string(), "no_cache": true, "type": "WEAP"});
        let r = crate::execute("query", q).unwrap();
        assert!(has_limitation(&r, "masters not in the load order"), "{r}");
    }

    /// M3.D1: an active plugin with no file is a field of `Loaded` (and of `load`'s
    /// listing), not only limitation text; it does not make an unrelated winner unsafe.
    #[test]
    fn missing_active_plugins_are_a_field_and_winners_say_if_they_are_safe() {
        let dir = fixture("load-missing-active");
        let root = dir.join("MO2");
        for d in ["mods", "overwrite", "profiles/P", "Game/Data"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let game = root.join("Game").to_string_lossy().replace('\\', "\\\\");
        std::fs::write(
            root.join("ModOrganizer.ini"),
            format!("[General]\r\ngameName=Starfield\r\ngamePath=@ByteArray({game})\r\n"),
        )
        .unwrap();
        for name in ["Base.esm", "Patch.esm"] {
            std::fs::copy(dir.join(name), root.join("Game/Data").join(name)).unwrap();
        }
        std::fs::write(
            root.join("profiles/P/plugins.txt"),
            b"*Base.esm\r\n*Patch.esm\r\n*Gone.esm\r\n",
        )
        .unwrap();
        std::fs::write(root.join("profiles/P/modlist.txt"), b"").unwrap();
        let profile = root.join("profiles/P").display().to_string();
        let engine = Engine::with_default_handlers();
        let r = engine
            .execute("load", json!({"profile": profile, "no_cache": true}))
            .unwrap();
        assert_eq!(r["data"]["missing_active_plugins"], json!(["Gone.esm"]));
        let missing =
            engine.with_session(|s| s.get::<Loaded>().unwrap().missing_active_plugins.clone());
        assert_eq!(missing, ["Gone.esm"]);
        // Still stated as a limitation for readers of the envelope.
        assert!(has_limitation(&r, "active plugins with no file"), "{r}");
        let i = engine
            .execute("inspect", json!({"form": "Base.esm:000800"}))
            .unwrap();
        let source = &i["data"]["winner_source"];
        assert_eq!(source["safe"], true, "{source}");
        assert_eq!(source["blocking"], json!([]));
        assert!(source["unrelated"][0]
            .as_str()
            .unwrap()
            .contains("Gone.esm"));

        // A plugin that failed to load after the winner may override it: not safe.
        std::fs::write(dir.join("Bad.esm"), b"GRUPxxxx").unwrap();
        let mut args = plugins_args(&dir);
        args["plugins"] = json!(["Base.esm", "Patch.esm", "Bad.esm"]);
        args["form"] = json!("Base.esm:000800");
        let i = crate::execute("inspect", args.clone()).unwrap();
        let source = &i["data"]["winner_source"];
        assert_eq!(source["safe"], false);
        assert!(source["blocking"][0]
            .as_str()
            .unwrap()
            .contains("Bad.esm failed to load"));
        // Loading first, it cannot win: unrelated.
        args["plugins"] = json!(["Bad.esm", "Base.esm", "Patch.esm"]);
        let i = crate::execute("inspect", args).unwrap();
        assert_eq!(i["data"]["winner_source"]["safe"], true);
    }

    fn has_limitation(result: &Value, needle: &str) -> bool {
        result["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains(needle))
    }

    /// Review round 2: a command that loads implicitly (a fresh CLI session) reports failed
    /// plugins and ordering errors like `load` does.
    #[test]
    fn implicit_loads_keep_failed_plugins_and_ordering_errors() {
        let dir = fixture("load-implicit");
        std::fs::write(dir.join("Bad.esm"), b"GRUPxxxx").unwrap();
        let args = json!({
            "plugins": ["Patch.esm", "Base.esm", "Bad.esm"],
            "data_dir": dir.display().to_string(),
            "no_cache": true,
        });
        let mut q = args.clone();
        q["type"] = json!("WEAP");
        let r = crate::execute("query", q).unwrap();
        assert!(has_limitation(&r, "1 plugin(s) failed to load"), "{r}");
        assert!(has_limitation(&r, "Bad.esm"), "{r}");
        assert!(
            has_limitation(&r, "ordering error: 1 master link(s)")
                && has_limitation(&r, "Patch.esm loads before its master(s) Base.esm"),
            "{r}"
        );
        let mut i = args.clone();
        i["form"] = json!("Pistol");
        let r = crate::execute("inspect", i).unwrap();
        assert!(has_limitation(&r, "failed to load"), "{r}");
        assert!(has_limitation(&r, "ordering error"), "{r}");
        let r = crate::execute("conflicts", args.clone()).unwrap();
        assert!(has_limitation(&r, "failed to load"), "{r}");
        assert!(has_limitation(&r, "ordering error"), "{r}");
        let r = crate::execute("load", args).unwrap();
        assert!(has_limitation(&r, "failed to load"), "{r}");
        assert_eq!(r["coverage"]["not_examined"], json!(["Bad.esm"]));
        assert_eq!(r["data"]["plugins"][0]["late_masters"], json!(["Base.esm"]));
    }
}
