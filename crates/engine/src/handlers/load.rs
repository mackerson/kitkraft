//! `load` (M2.D8): resolve a load order from an MO2 profile or an explicit plugin list,
//! index every plugin (through the on-disk cache), and keep it in the session.
//!
//! The other load-order commands (`inspect`, `query`, `conflicts`) take the same
//! `profile` / `plugins` / `data_dir` arguments: given, they load first (and replace the
//! session's load order); absent, they use the session's. The CLI runs each command in a
//! fresh session, so `kit` users pass `--profile` to each; an MCP session loads once.

use std::path::{Path, PathBuf};
use std::time::Instant;

use kitcraft_doc::{CacheUse, IndexCache, LoadOptions, LoadOrder, PluginSpec, Scale};
use kitcraft_io::{plan_profile, Active, ProfileOptions, Provider};
use serde_json::{json, Value};

use super::{failed, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

const ID: &str = "load";

/// Stated whenever runtime IDs are shown.
pub(crate) const RUNTIME_ASSUMED: &str = "runtime slots and IDs follow the assumption \
     `runtime-slots-by-scale` (docs/load-order.md): not yet observed in game or in the CK";

/// Stated while string tables are not read.
pub(crate) const STRINGS_PENDING: &str = "string tables are not loaded yet (M2.D7 waits for \
     the BA2 and string-table readers): lstrings of localized plugins stay string IDs";

/// A loaded load order, kept in the session.
pub(crate) struct Loaded {
    /// The model.
    pub lo: LoadOrder,
    /// Where it came from: `profile:<dir>` or `plugins`.
    pub source: String,
    /// Requested string-table language.
    pub language: String,
    /// Limitations every command using this load order repeats.
    pub limitations: Vec<String>,
}

/// The arguments that select a load order.
pub(crate) const LOAD_ORDER_ARGS: [&str; 3] = ["profile", "plugins", "data_dir"];

fn has_load_args(args: &Value) -> bool {
    args.get("profile").is_some() || args.get("plugins").is_some()
}

/// The session's load order, loading it first when `args` name one.
pub(crate) fn acquire<'s>(
    session: &'s mut Session,
    args: &Value,
    command: &str,
) -> Result<&'s Loaded, EngineError> {
    if has_load_args(args) {
        let (loaded, listing, _) = load(args, command)?;
        session.set_load_order(Some(listing));
        session.insert(loaded);
    }
    session.get::<Loaded>().ok_or_else(|| {
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
        return Err(failed(ID, "pass `profile` (an MO2 profile dir) or `plugins`"));
    }
    let (loaded, listing, evidence) = load(args, ID)?;
    let mut limitations = loaded.limitations.clone();
    let failed_plugins: Vec<String> = loaded
        .lo
        .plugins
        .iter()
        .filter(|p| p.error.is_some())
        .map(|p| p.name.clone())
        .collect();
    if !failed_plugins.is_empty() {
        limitations.push(format!(
            "{} plugin(s) failed to load and have no records: {}",
            failed_plugins.len(),
            failed_plugins.join(", ")
        ));
    }
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

/// Load per `args`: the session value, its JSON listing, and evidence.
fn load(args: &Value, command: &str) -> Result<(Loaded, Value, Vec<Evidence>), EngineError> {
    let started = Instant::now();
    let language = args
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("en")
        .to_string();
    let data_dir = args.get("data_dir").and_then(Value::as_str).map(PathBuf::from);
    let mut limitations = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // name -> (why active, provider)
    let mut why: Vec<(String, String, String)> = Vec::new();
    let mut inactive: Vec<String> = Vec::new();
    let (specs, source, profile_name): (Vec<PluginSpec>, String, Option<String>) =
        match (args.get("profile"), args.get("plugins")) {
            (Some(_), Some(_)) => {
                return Err(failed(command, "pass `profile` or `plugins`, not both"));
            }
            (Some(_), None) => {
                let dir = str_arg(command, args, "profile")?;
                let pl = plan_profile(
                    Path::new(dir),
                    &ProfileOptions {
                        data_dir: data_dir.clone(),
                    },
                )
                .map_err(|e| failed(command, e.to_string()))?;
                warnings.extend(pl.profile.warnings.iter().cloned());
                warnings.extend(pl.plan.warnings.iter().cloned());
                let mut assumed: Vec<String> = Vec::new();
                for p in &pl.plan.plugins {
                    match &p.active {
                        Some(a) => {
                            if matches!(
                                a,
                                Active::Hardcoded | Active::Unmanaged(_) | Active::BlueprintCompanion(_)
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
                let missing = pl.plan.missing();
                if !missing.is_empty() {
                    limitations.push(format!(
                        "active plugins with no file (not loaded): {}",
                        missing.join(", ")
                    ));
                }
                let _ = Provider::GameData;
                (pl.plan.specs(), format!("profile:{dir}"), Some(pl.profile.name.clone()))
            }
            (None, Some(list)) => {
                let names: Vec<String> = list
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let (specs, missing) = kitcraft_io::plan::explicit(&names, data_dir.as_deref());
                if !missing.is_empty() {
                    return Err(failed(
                        command,
                        format!("plugin file(s) not found: {}", missing.join(", ")),
                    ));
                }
                for s in &specs {
                    why.push((s.name.clone(), "listed".into(), "path".into()));
                }
                (specs, "plugins".to_string(), None)
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
    let lo = LoadOrder::load(
        specs,
        &LoadOptions {
            cache,
            threads: 0,
        },
    )
    .map_err(|e| failed(command, e.to_string()))?;
    limitations.push(RUNTIME_ASSUMED.to_string());
    limitations.push(STRINGS_PENDING.to_string());
    for w in &warnings {
        limitations.push(format!("profile: {w}"));
    }

    let mut evidence = Vec::new();
    let mut plugins = Vec::new();
    let (mut hits, mut records) = (0usize, 0u64);
    let mut missing_masters_total = 0usize;
    for (i, p) in lo.plugins.iter().enumerate() {
        let index = u16::try_from(i).unwrap_or(u16::MAX);
        let header = p.header();
        if p.cache == CacheUse::Hit {
            hits = hits.saturating_add(1);
        }
        records = records.saturating_add(p.records().len() as u64);
        let missing = p.missing_masters();
        missing_masters_total = missing_masters_total.saturating_add(missing.len());
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
            "late_masters": p.late_masters(index),
            "resolvable": p.map.is_ok(),
            "resolve_error": p.map.as_ref().err().map(ToString::to_string),
            "records": p.records().len(),
            "unresolved_records": lo.forms.unresolved_count(index),
            "runtime_slot": p.runtime_slot.map(|s| s.label()),
            "cache": cache_label(p.cache),
            "error": p.error,
        }));
    }
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
            language,
            limitations: vec![RUNTIME_ASSUMED.to_string(), STRINGS_PENDING.to_string()],
        },
        listing,
        evidence,
    ))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]
pub(crate) mod tests {
    use super::*;
    use crate::handlers::testdata::{group, record, sub};
    use crate::Engine;

    /// A plugin: TES4 (flags, masters) and one top group per record type.
    pub(crate) fn plugin(flags: u32, masters: &[&str], records: &[(&[u8; 4], u32, Vec<u8>)]) -> Vec<u8> {
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
        for (s, _, _) in records {
            if !sigs.contains(s) {
                sigs.push(s);
            }
        }
        for s in sigs {
            let body: Vec<u8> = records
                .iter()
                .filter(|(x, _, _)| x == &s)
                .flat_map(|(x, id, b)| record(x, 0, *id, b))
                .collect();
            out.extend(group(s, 0, &body));
        }
        out
    }

    /// A temp dir with Base.esm (two weapons) and Patch.esm (overrides one, renames it).
    pub(crate) fn fixture(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kitcraft-engine-lo-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let weap = |edid: &str, full: &str| [sub(b"EDID", &[edid.as_bytes(), &[0]].concat()), sub(b"FULL", &[full.as_bytes(), &[0]].concat()), sub(b"DATA", &[1, 2, 3, 4])].concat();
        std::fs::write(
            dir.join("Base.esm"),
            plugin(
                0x1,
                &[],
                &[
                    (b"WEAP", 0x800, weap("OldEarthHuntingRifle", "Old Earth Hunting Rifle")),
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
                    (b"WEAP", 0x800, weap("OldEarthHuntingRifle", "Old Soviet Marksman Rifle")),
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
        assert_eq!(plugins[1]["masters"], json!([{"name": "Base.esm", "index": 0}]));
        assert_eq!(plugins[1]["missing_masters"], json!([]));
        assert_eq!(plugins[0]["scale"], "full");
        assert_eq!(plugins[0]["runtime_slot"], "00");
        assert_eq!(plugins[0]["records"], 3);
        assert_eq!(plugins[0]["cache"], "uncached");
        assert!(data["summary"].as_str().unwrap().contains("2 plugins loaded"));
        assert_eq!(result["evidence"].as_array().unwrap().len(), 2);
        let limitations = result["limitations"].as_array().unwrap();
        assert!(limitations.iter().any(|l| l.as_str().unwrap().contains("runtime-slots-by-scale")));
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
        let err = crate::execute(
            "load",
            json!({"profile": dir.display().to_string()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("not an MO2 profile"), "{err}");
        let err = crate::execute(
            "load",
            json!({"profile": "x", "plugins": []}),
        )
        .unwrap_err();
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
    }
}
