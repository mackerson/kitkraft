//! `game.session`, `game.status`, `game.probe`, `game.stop` (lane C, M4.T3 and M5.C1;
//! ADR 0010 and its Amendment 1): agent-driven sessions on the dev profile.
//!
//! The work is `kitkraft-game`'s; this module gathers the facts preflight needs from the
//! MO2 instance through `kitkraft-io` (read only), builds the session plan, and runs it
//! inline (the one-shot CLI), or on a thread the engine's [`Session`] owns (`start`), so
//! a long-lived front end follows it with `game.status` on its one engine worker and ends
//! it with `game.stop`. Rules and examples: `docs/game-sessions.md`.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use kitkraft_game::console::HttpConsole;
use kitkraft_game::controller::{self, Deps, Plan, Report, Shared};
use kitkraft_game::host::{Host, SystemHost};
use kitkraft_game::preflight::{self, BaseSave, Facts, PluginFacts, Refusal};
use kitkraft_game::probes::ProbeFile;
use kitkraft_game::profile::fill;
use kitkraft_game::GameProfile;
use kitkraft_io::ini::Ini;
use kitkraft_io::Instance;
use serde_json::{json, Value};

use super::{failed, str_arg};
use crate::envelope::{Coverage, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

/// The dev profile a session runs on when the call names none.
const DEV_PROFILE: &str = "KitKraft Dev";

/// The process host sessions use; tests put a fake one in the session.
#[derive(Clone)]
pub(crate) struct GameHost(pub(crate) Arc<dyn Host>);

/// The sessions started with `start`, by id.
#[derive(Default)]
struct Sessions {
    map: BTreeMap<String, Running>,
}

struct Running {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<Report>>,
    plan: Value,
    probe_wait: Duration,
}

/// A session still running when its front end's engine goes away (the one-shot CLI exits
/// right after `--start`) is stopped properly, not abandoned: the game ended, the profile
/// verified, the ledger entry written.
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(t) = self.thread.take() {
            self.shared.request_stop();
            let _ = t.join();
        }
    }
}

fn host(session: &Session) -> Arc<dyn Host> {
    session.get::<GameHost>().map_or_else(
        || Arc::new(SystemHost) as Arc<dyn Host>,
        |h| Arc::clone(&h.0),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    crate::deploy::sha256::hex(bytes)
}

fn outcome(data: Value, limitations: Vec<String>) -> Outcome {
    Outcome {
        data,
        coverage: Coverage::unknown(),
        limitations,
        evidence: Vec::new(),
    }
}

fn bool_arg(args: &Value, name: &str) -> bool {
    args.get(name).and_then(Value::as_bool) == Some(true)
}

/// The user's Documents folder, for `{documents}` in log paths.
fn documents() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|h| PathBuf::from(h).join("Documents"))
}

fn read_ini(path: &Path) -> Option<Ini> {
    let bytes = std::fs::read(path).ok()?;
    Some(Ini::parse(&String::from_utf8_lossy(&bytes)))
}

/// The profile folder matching `name` ignoring case (MO2's `Kitkraft Dev` for
/// `KitKraft Dev`), else the folder as named.
fn profile_dir(instance: &Instance, name: &str) -> (String, PathBuf, bool) {
    let found = instance
        .profiles()
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.eq_ignore_ascii_case(name));
    match found {
        Some(p) => {
            let dir = instance.profile_dir(&p);
            (p, dir, true)
        }
        None => (name.to_string(), instance.profile_dir(name), false),
    }
}

fn enabled_mods(profile_dir: &Path) -> Vec<String> {
    kitkraft_io::text::read_lines(&profile_dir.join("modlist.txt"))
        .ok()
        .flatten()
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.trim().strip_prefix('+').map(str::to_string))
        .collect()
}

fn plugin_stem(name: &str) -> &str {
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
}

/// Everything a session needs, read and checked, before anything is written.
struct Prepared {
    plan: Plan,
    refusals: Vec<Refusal>,
    limitations: Vec<String>,
}

fn prepare(session: &Session, args: &Value, id: &str) -> Result<Prepared, EngineError> {
    let fail = |m: String| failed(id, m);
    let mo2 = PathBuf::from(str_arg(id, args, "mo2")?);
    let instance = Instance::read(&mo2).map_err(|e| fail(e.to_string()))?;
    let game_name = args
        .get("game")
        .and_then(Value::as_str)
        .unwrap_or("starfield");
    let game = GameProfile::named(game_name).map_err(|e| fail(e.to_string()))?;
    let probes_path = PathBuf::from(str_arg(id, args, "probes")?);
    let probes_bytes =
        std::fs::read(&probes_path).map_err(|e| fail(format!("{}: {e}", probes_path.display())))?;
    let probes = ProbeFile::parse(&String::from_utf8_lossy(&probes_bytes))
        .map_err(|e| fail(format!("{}: {e}", probes_path.display())))?;
    let probes_sha256 = sha256_hex(&probes_bytes);

    let wanted = args
        .get("profile")
        .and_then(Value::as_str)
        .unwrap_or(DEV_PROFILE);
    let (profile, profile_dir, profile_exists) = profile_dir(&instance, wanted);
    let host = host(session);
    let mut refusals_extra = Vec::new();
    let running = match host.running() {
        Ok(r) => r,
        Err(e) => {
            refusals_extra.push(Refusal {
                rule: "running",
                message: format!("could not list the running processes ({e}); the controller does not start blind"),
            });
            Vec::new()
        }
    };
    let settings = read_ini(&profile_dir.join("settings.ini"));
    let flag = |key: &str| {
        settings
            .as_ref()
            .and_then(|i| i.get("General", key))
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
    };
    let mut ini_values = Vec::new();
    for sw in &game.ini {
        if let Some(v) =
            read_ini(&profile_dir.join(&sw.file)).and_then(|i| i.get(&sw.section, &sw.key))
        {
            ini_values.push((sw.file.clone(), sw.section.clone(), sw.key.clone(), v));
        }
    }
    let mods = enabled_mods(&profile_dir);
    let stem = plugin_stem(&probes.plugin).to_string();
    let dev_mod = format!("{}{stem}", crate::deploy::PREFIX);
    let marker = std::fs::read_to_string(
        instance
            .mods_dir
            .join(&dev_mod)
            .join(crate::deploy::MANIFEST),
    )
    .is_ok_and(|t| t.contains(crate::deploy::MARKER));
    let base_name = args
        .get("base_save")
        .and_then(Value::as_str)
        .unwrap_or(&game.saves.base_default)
        .to_string();
    if base_name.is_empty()
        || base_name.contains(['/', '\\', ':'])
        || base_name.starts_with(&game.saves.session_prefix)
    {
        return Err(fail(format!(
            "base save `{base_name}`: a save name in the profile's saves folder, not a path and not a {}* save",
            game.saves.session_prefix
        )));
    }
    let saves_dir = profile_dir.join(&game.saves.dir);
    let base_path = saves_dir.join(format!("{base_name}.{}", game.saves.extension));
    let base_sha = std::fs::read(&base_path).ok().map(|b| sha256_hex(&b));
    let expected_sha = args
        .get("base_save_sha256")
        .and_then(Value::as_str)
        .map(str::to_string);
    let facts = Facts {
        instance: mo2.clone(),
        profile: profile.clone(),
        profile_exists,
        selected_profile: instance.selected_profile.clone(),
        running,
        local_settings: flag("LocalSettings"),
        local_saves: flag("LocalSaves"),
        ini_values,
        enabled_mods: mods.clone(),
        plugin: Some(PluginFacts {
            name: probes.plugin.clone(),
            dev_mod,
            marker,
        }),
        base_save: BaseSave {
            name: base_name.clone(),
            path: base_path.clone(),
            sha256: base_sha.clone(),
            expected_sha256: expected_sha,
        },
    };
    let mut refusals = preflight::check(&game, &facts);
    refusals.extend(refusals_extra);

    // Where the session writes: observe\<id>\ and the ledger.
    let cwd = std::env::current_dir().map_err(|e| fail(e.to_string()))?;
    let observe_root = args
        .get("observe")
        .and_then(Value::as_str)
        .map_or_else(|| cwd.join("observe"), PathBuf::from);
    if let Err(e) = crate::edit::patch::guard_location(&observe_root.join("x")) {
        refusals.push(Refusal {
            rule: "observe-path",
            message: e.to_string(),
        });
    }
    let ledger = args.get("ledger").and_then(Value::as_str).map_or_else(
        || cwd.join("docs").join(super::ledger::LEDGER_FILE),
        PathBuf::from,
    );
    if let Err(e) = super::ledger::check_ledger_path(&ledger) {
        refusals.push(Refusal {
            rule: "ledger-path",
            message: e.to_string(),
        });
    }
    let taken: Vec<String> = session
        .get::<Sessions>()
        .map(|s| s.map.keys().cloned().collect())
        .unwrap_or_default();
    let date = controller::today_utc();
    let (session_id, session_save) =
        controller::next_session(&observe_root, &date, &game.saves.session_prefix, &taken);

    // Backups: the list files, every INI of the profile, the base save.
    let mut backups: Vec<PathBuf> = [
        "modlist.txt",
        "plugins.txt",
        "loadorder.txt",
        "lockedorder.txt",
    ]
    .iter()
    .map(|n| profile_dir.join(n))
    .collect();
    let mut inis: Vec<PathBuf> = std::fs::read_dir(&profile_dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ini"))
                })
                .collect()
        })
        .unwrap_or_default();
    inis.sort();
    backups.extend(inis);
    backups.push(base_path);

    let docs = documents();
    let logs: Vec<(String, PathBuf)> = game
        .logs
        .iter()
        .filter_map(|l| {
            let path = if l.path.contains("{documents}") {
                let d = docs.as_ref()?.display().to_string();
                fill(&l.path, &[("documents", &d)])
            } else {
                l.path.clone()
            };
            Some((l.name.clone(), PathBuf::from(path)))
        })
        .collect();

    let mut versions = Vec::new();
    let exe_version = |file: &str| {
        instance
            .game_dir
            .as_ref()
            .and_then(|g| kitkraft_io::host::pe_file_version(&g.join(file)))
            .unwrap_or_else(|| "unknown".into())
    };
    versions.push((
        "Game build".to_string(),
        format!(
            "`{}` {}",
            game.executables.game,
            exe_version(&game.executables.game)
        ),
    ));
    versions.push((
        "Script extender".to_string(),
        format!(
            "`{}` {}",
            game.executables.script_extender,
            exe_version(&game.executables.script_extender)
        ),
    ));
    let mut tools = Vec::new();
    for m in &mods {
        let lower = m.to_ascii_lowercase();
        let console_tool = game
            .console
            .mod_names
            .iter()
            .chain(["console command runner".to_string()].iter())
            .any(|n| lower.contains(&n.to_ascii_lowercase()));
        if console_tool {
            let v = read_ini(&instance.mods_dir.join(m).join("meta.ini"))
                .and_then(|i| i.get("General", "version"))
                .unwrap_or_else(|| "unknown".into());
            tools.push(format!("{m} {v}"));
        }
    }
    versions.push((
        "Console tools".to_string(),
        if tools.is_empty() {
            "none enabled".into()
        } else {
            tools.join("; ")
        },
    ));

    let limitations: Vec<String> = game
        .unverified_logs()
        .into_iter()
        .map(|u| format!("log {} has no evidence yet ({}); a row that needs it is inconclusive when it is missing", u.fact, u.pending))
        .chain(std::iter::once(
            "what the console prints for each probe command is grade D until M4.T1 observes it".to_string(),
        ))
        .collect();
    let observe = observe_root.join(&session_id);
    let plan = Plan {
        session: session_id,
        date,
        timeouts: game.timeouts.clone(),
        mod_manager: mo2.join(&game.executables.mod_manager),
        instance: mo2,
        profile,
        observe,
        ledger: Some(ledger),
        base_save: base_name,
        base_save_sha256: base_sha.unwrap_or_default(),
        session_save,
        plugins: vec![(probes.plugin.clone(), probes.sha256.clone())],
        probes,
        probes_path,
        probes_sha256,
        backups,
        logs,
        versions,
        stay: bool_arg(args, "start"),
        poll: Duration::from_millis(250),
        profile_dir,
        game,
    };
    Ok(Prepared {
        plan,
        refusals,
        limitations,
    })
}

fn report_json(report: &Report) -> Value {
    serde_json::to_value(report).unwrap_or(Value::Null)
}

/// `game.session`.
pub(super) fn session(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "game.session";
    let prepared = prepare(session, args, ID)?;
    let Prepared {
        plan,
        refusals,
        limitations,
    } = prepared;
    if bool_arg(args, "dry_run") {
        return Ok(outcome(
            json!({
                "dry_run": true,
                "would_start": refusals.is_empty(),
                "refusals": refusals,
                "plan": plan.describe(),
                "summary": if refusals.is_empty() {
                    format!("session {} would start on `{}`; nothing written", plan.session, plan.profile)
                } else {
                    format!("session would be refused ({} reason(s)); nothing written", refusals.len())
                },
            }),
            limitations,
        ));
    }
    if !refusals.is_empty() {
        let text: Vec<String> = refusals.iter().map(ToString::to_string).collect();
        return Err(failed(ID, text.join("; ")));
    }
    let console = HttpConsole::new(&plan.game.console.url).map_err(|e| failed(ID, e))?;
    let deps = Deps {
        host: host(session),
        console: Box::new(console),
        digest: sha256_hex,
    };
    let shared = Shared::new(&plan.session);
    if bool_arg(args, "start") {
        let described = plan.describe();
        let id = plan.session.clone();
        let observe = plan.observe.display().to_string();
        let probe_wait = Duration::from_secs(plan.timeouts.probe_secs.saturating_add(30));
        let s2 = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name(format!("kitkraft-game-{id}"))
            .spawn(move || controller::run(&plan, deps, &s2))
            .map_err(|e| failed(ID, format!("could not start the session thread: {e}")))?;
        if session.get::<Sessions>().is_none() {
            session.insert(Sessions::default());
        }
        if let Some(s) = session.get_mut::<Sessions>() {
            s.map.insert(
                id.clone(),
                Running {
                    shared,
                    thread: Some(thread),
                    plan: described,
                    probe_wait,
                },
            );
        }
        return Ok(outcome(
            json!({
                "session": id, "started": true, "observe": observe,
                "summary": format!("session {id} started; follow it with `kit game status {id}`, end it with `kit game stop {id}`"),
            }),
            limitations,
        ));
    }
    let report = controller::run(&plan, deps, &shared);
    Ok(outcome(
        json!({
            "session": plan.session, "report": report_json(&report),
            "summary": format!("session {}: {} ({} rows); ledger entry {}", plan.session, report.status.text(), report.rows.len(), report.ledger_entry.as_deref().unwrap_or("not written")),
        }),
        limitations,
    ))
}

fn running<'s>(
    session: &'s mut Session,
    id: &str,
    command: &str,
) -> Result<&'s mut Running, EngineError> {
    session
        .get_mut::<Sessions>()
        .and_then(|s| s.map.get_mut(id))
        .ok_or_else(|| {
            failed(
                command,
                format!("no session `{id}` in this front end (sessions started with --start live as long as it does)"),
            )
        })
}

/// `game.status`.
pub(super) fn status(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "game.status";
    let id = str_arg(ID, args, "id")?.to_string();
    let r = running(session, &id, ID)?;
    let snap = r.shared.snapshot();
    let phase = serde_json::to_value(snap.phase).unwrap_or(Value::Null);
    Ok(outcome(
        json!({
            "session": id,
            "phase": phase,
            "current_row": snap.current_row,
            "rows": snap.rows,
            "reruns": snap.reruns,
            "screenshots": snap.screenshots.iter().map(|p| json!({
                "file": p.file_name().map(|n| n.to_string_lossy().into_owned()),
                "path": p.display().to_string(),
            })).collect::<Vec<_>>(),
            "events": snap.events,
            "elapsed_secs": snap.elapsed_secs,
            "report": snap.report,
        }),
        Vec::new(),
    ))
}

/// `game.probe`.
pub(super) fn probe(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "game.probe";
    let id = str_arg(ID, args, "id")?.to_string();
    let row = str_arg(ID, args, "row")?.to_string();
    let screenshot = bool_arg(args, "screenshot");
    let r = running(session, &id, ID)?;
    let result = r
        .shared
        .probe(&row, screenshot, r.probe_wait)
        .map_err(|e| failed(ID, e))?;
    Ok(outcome(
        json!({"session": id, "row": result, "summary": format!("{id} {row}: {}", result.verdict.text())}),
        Vec::new(),
    ))
}

/// `game.stop`.
pub(super) fn stop(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "game.stop";
    let id = str_arg(ID, args, "id")?.to_string();
    let r = running(session, &id, ID)?;
    if bool_arg(args, "dry_run") {
        let snap = r.shared.snapshot();
        return Ok(outcome(
            json!({
                "dry_run": true, "session": id,
                "phase": serde_json::to_value(snap.phase).unwrap_or(Value::Null),
                "would": [
                    format!("end {}", r.plan.get("ends").unwrap_or(&Value::Null)),
                    "copy the logs into the observe folder".to_string(),
                    format!(
                        "verify (and restore) {} backed-up profile files",
                        r.plan.get("backups").and_then(Value::as_array).map_or(0, Vec::len)
                    ),
                    format!("append one entry to {}", r.plan.get("ledger").unwrap_or(&Value::Null)),
                ],
                "remaining_rows_become": "not-run (stopped by game.stop before row X)",
            }),
            Vec::new(),
        ));
    }
    r.shared.request_stop();
    let report = match r.thread.take() {
        Some(t) => t
            .join()
            .map_err(|_| failed(ID, "the session thread failed"))?,
        None => r
            .shared
            .snapshot()
            .report
            .ok_or_else(|| failed(ID, "the session has no report"))?,
    };
    if let Some(s) = session.get_mut::<Sessions>() {
        s.map.remove(&id);
    }
    Ok(outcome(
        json!({
            "session": id, "report": report_json(&report),
            "summary": format!("session {id} stopped: {}; ledger entry {}", report.status.text(), report.ledger_entry.as_deref().unwrap_or("not written")),
        }),
        Vec::new(),
    ))
}
