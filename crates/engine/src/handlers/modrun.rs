//! `mod.run` (CLI `kit mod run <spec>`; lane T round 2, M4.T10): one brief's spec to one
//! report. It chains seven catalog commands on the caller's session, each through the
//! same `execute` path every front end uses, so it adds no format or editing logic:
//!
//! | # | Stage | Command | Writes |
//! | - | ----- | ------- | ------ |
//! | 1 | `build` | `build <spec> <dir>\<plugin> --overwrite` | the plugin |
//! | 2 | `validate` | `validate --plugin <plugin>` | nothing |
//! | 3 | `roundtrip` | `roundtrip <plugin> --verify` | nothing |
//! | 4 | `package` | `package <plugin> <dir>\package --overwrite` | the mod folder and zip |
//! | 5 | `deploy` | `deploy --dev <package> --mo2 <mo2>` | the `KitKraft Dev - <plugin>` mod (ADR 0009) |
//! | 6 | `testplan` | `testplan <plugin> <dir>\testplan` | the batch, checklist, `probes.toml` |
//! | 7 | `session` | `game.session --dry-run`, then `game.session` when preflight passes | `observe\<id>\`, one ledger entry (ADR 0010) |
//!
//! `<dir>` is `<out>\<name>\` (default `out\<name>\` under the working directory). The run
//! stops at the first stage that fails (an error, or `data.passed: false`) and writes
//! `<dir>\report.md` (every stage's envelope summary, the ledger entry id, a verdict per
//! probe), `<dir>\report.json` (this command's `data`) and each stage's full envelope as
//! `<dir>\stages\<nn>-<stage>.json`. A session refused by preflight (today: always, until
//! M4.T1 records the launch, console and save facts and M4.O7 installs the console tools)
//! is not a failure: the report names every refusal, every probe is `not-run`, and the
//! result is `partial`. `dry_run` runs `build --plan` only, lists every stage's arguments
//! and writes nothing; `until` stops after the named stage.
//!
//! `result` follows AGENTS.md "Reporting": `pass` when every stage and every probe passed,
//! `fail` when a stage or a probe failed, else `partial`. `data.passed` is `false` only on
//! `fail` (the CLI's exit code 1).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use kitkraft_game::probes::ProbeFile;
use serde_json::{json, Map, Value};

use super::{failed, str_arg};
use crate::edit::patch::guard_location;
use crate::envelope::{Coverage, Outcome};
use crate::exec::{execute_nested, EngineError};
use crate::session::Session;

const ID: &str = "mod.run";

/// The stages, in order (the `until` values).
pub(crate) const STAGES: [&str; 7] = [
    "build",
    "validate",
    "roundtrip",
    "package",
    "deploy",
    "testplan",
    "session",
];

/// What marks a folder as a mod run's own (`report.json`'s `kind`): a run reuses only
/// such a folder.
pub(crate) const KIND: &str = "kitkraft-mod-run";

/// The report file names in `<dir>`.
pub(crate) const REPORT_MD: &str = "report.md";
pub(crate) const REPORT_JSON: &str = "report.json";

/// Where the playbook's fix loop is (named in every failing report).
const PLAYBOOK: &str = "docs/playbooks/mod-from-brief.md";

/// The dev profile a session runs on when the call names none.
const DEV_PROFILE: &str = "KitKraft Dev";

/// One stage's outcome.
#[derive(Debug, Clone)]
struct Stage {
    name: &'static str,
    command: &'static str,
    /// `pass`, `fail`, `not-run`, `inconclusive`.
    status: &'static str,
    reason: String,
    args: Value,
    summary: String,
    limitations: Vec<String>,
    /// `<dir>\stages\<nn>-<stage>.json` (relative to `<dir>`), when written.
    envelope: Option<String>,
    error: Option<Value>,
    /// Facts the report shows (plugin SHA-256, deployment id, ledger entry, ...).
    facts: Map<String, Value>,
}

impl Stage {
    fn new(name: &'static str, command: &'static str, args: Value) -> Stage {
        Stage {
            name,
            command,
            status: "not-run",
            reason: String::new(),
            args,
            summary: String::new(),
            limitations: Vec::new(),
            envelope: None,
            error: None,
            facts: Map::new(),
        }
    }

    fn json(&self) -> Value {
        json!({
            "stage": self.name,
            "command": self.command,
            "status": self.status,
            "reason": (!self.reason.is_empty()).then_some(&self.reason),
            "args": self.args,
            "summary": self.summary,
            "limitations": self.limitations,
            "envelope": self.envelope,
            "error": self.error,
            "facts": self.facts,
        })
    }
}

/// One probe row's verdict in the report.
#[derive(Debug, Clone)]
struct Row {
    row: String,
    title: String,
    status: String,
    reason: String,
}

fn opt_str<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
    args.get(name).and_then(Value::as_str)
}

fn flag(args: &Value, name: &str) -> bool {
    args.get(name).and_then(Value::as_bool) == Some(true)
}

/// A run name: ASCII letters, digits, `.`, `-`, `_`; not only dots.
fn check_name(name: &str) -> Result<(), EngineError> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && !name.chars().all(|c| c == '.');
    if ok {
        Ok(())
    } else {
        Err(failed(
            ID,
            format!("name `{name}`: use 1 to 64 ASCII letters, digits, `.`, `-` or `_`"),
        ))
    }
}

/// The load-order arguments every reading stage gets: the call's own (`profile`,
/// `plugins`, `data_dir`, `no_cache`), else the spec's `[load_order]` table (`build` reads
/// that itself; `validate`, `package` and `testplan` need it passed).
fn load_order(args: &Value, spec_text: &str) -> Map<String, Value> {
    let mut lo = Map::new();
    for key in ["profile", "plugins", "data_dir", "no_cache"] {
        if let Some(v) = args.get(key) {
            lo.insert(key.to_string(), v.clone());
        }
    }
    if lo.contains_key("profile") || lo.contains_key("plugins") {
        return lo;
    }
    let table = toml::from_str::<toml::Value>(spec_text)
        .ok()
        .and_then(|v| v.get("load_order").cloned());
    if let Some(t) = table {
        if let Some(p) = t.get("profile").and_then(toml::Value::as_str) {
            lo.insert("profile".into(), json!(p));
        }
        if let Some(list) = t.get("plugins").and_then(toml::Value::as_array) {
            let names: Vec<Value> = list
                .iter()
                .filter_map(toml::Value::as_str)
                .map(|s| json!(s))
                .collect();
            lo.insert("plugins".into(), Value::Array(names));
        }
        if !lo.contains_key("data_dir") {
            if let Some(d) = t.get("data_dir").and_then(toml::Value::as_str) {
                lo.insert("data_dir".into(), json!(d));
            }
        }
        if !lo.contains_key("no_cache") {
            if let Some(b) = t.get("no_cache").and_then(toml::Value::as_bool) {
                lo.insert("no_cache".into(), json!(b));
            }
        }
    }
    lo
}

fn with(base: &Map<String, Value>, extra: Value) -> Value {
    let mut out = base.clone();
    if let Value::Object(m) = extra {
        out.extend(m);
    }
    Value::Object(out)
}

fn path_text(p: &Path) -> String {
    p.display().to_string()
}

/// The first line of a summary (the report's table cell), `|` escaped.
fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().replace('|', "\\|")
}

/// Runs one stage through `execute` and fills `stage` from its envelope or error.
/// Returns the envelope when the command ran.
fn run_stage(
    session: &mut Session,
    stage: &mut Stage,
    stages_dir: Option<&Path>,
    index: usize,
    suffix: &str,
) -> Option<Value> {
    let result = execute_nested(session, stage.command, stage.args.clone());
    match result {
        Ok(env) => {
            let data = env.get("data").cloned().unwrap_or(Value::Null);
            stage.summary = data
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_default();
            stage.limitations = env
                .get("limitations")
                .and_then(Value::as_array)
                .map(|l| {
                    l.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let negative = data.get("passed") == Some(&Value::Bool(false));
            stage.status = if negative { "fail" } else { "pass" };
            if negative {
                stage.reason = format!("`{}` reported passed: false", stage.command);
            }
            if let Some(dir) = stages_dir {
                let file = format!("{:02}-{}{suffix}.json", index, stage.name);
                let text = serde_json::to_string_pretty(&env).unwrap_or_default();
                if std::fs::create_dir_all(dir).is_ok()
                    && std::fs::write(dir.join(&file), text).is_ok()
                {
                    stage.envelope = Some(format!("stages/{file}"));
                }
            }
            Some(env)
        }
        Err(e) => {
            stage.status = "fail";
            stage.reason = format!("{} error: {e}", e.kind());
            stage.summary = e.to_string();
            stage.error = Some(json!({"kind": e.kind(), "message": e.to_string()}));
            None
        }
    }
}

/// The verdict object `{status, reason}` of a session report row, as labels.
fn verdict_parts(v: &Value) -> (String, String) {
    let status = v
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("inconclusive")
        .to_string();
    let reason = v
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    (status, reason)
}

/// `mod.run`.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let spec = PathBuf::from(str_arg(ID, args, "spec")?);
    let spec_bytes =
        std::fs::read(&spec).map_err(|e| failed(ID, format!("{}: {e}", spec.display())))?;
    let spec_text = String::from_utf8_lossy(&spec_bytes).into_owned();
    let spec_sha = crate::deploy::sha256::hex(&spec_bytes);
    let stem = spec
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = opt_str(args, "name").map_or(stem, str::to_string);
    check_name(&name)?;
    let plugin_name = opt_str(args, "plugin").map_or_else(|| format!("{name}.esm"), str::to_string);
    let plugin_ok = Path::new(&plugin_name)
        .file_name()
        .map(|n| n.to_string_lossy())
        == Some(plugin_name.as_str().into())
        && [".esm", ".esp"]
            .iter()
            .any(|x| plugin_name.to_ascii_lowercase().ends_with(x));
    if !plugin_ok {
        return Err(failed(
            ID,
            format!("plugin `{plugin_name}`: a file name ending in .esm or .esp, not a path"),
        ));
    }
    let cwd = std::env::current_dir().map_err(|e| failed(ID, e.to_string()))?;
    let out_root = opt_str(args, "out").map_or_else(|| cwd.join("out"), PathBuf::from);
    let dir = out_root.join(&name);
    let dry_run = flag(args, "dry_run");
    let until = opt_str(args, "until").unwrap_or("session");
    let until_at = STAGES.iter().position(|s| *s == until).unwrap_or(6);

    for file in [REPORT_MD, REPORT_JSON] {
        guard_location(&dir.join(file)).map_err(|e| failed(ID, e.to_string()))?;
    }
    // A run reuses only its own folder.
    if !dry_run && dir.exists() {
        let ours = std::fs::read_to_string(dir.join(REPORT_JSON))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| v.get("kind").and_then(Value::as_str) == Some(KIND));
        let empty = std::fs::read_dir(&dir).is_ok_and(|mut rd| rd.next().is_none());
        if !ours && !empty {
            return Err(failed(
                ID,
                format!(
                    "{} exists and is not a mod run folder (no {REPORT_JSON} of kind {KIND}); pick another --name or --out",
                    dir.display()
                ),
            ));
        }
    }

    let lo = load_order(args, &spec_text);
    let plugin = dir.join(&plugin_name);
    let package_out = dir.join("package");
    let package_name = Path::new(&plugin_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let package_dir = package_out.join(&package_name);
    let testplan_out = dir.join("testplan");
    let probes = testplan_out.join("probes.toml");
    let mo2 = opt_str(args, "mo2").map(str::to_string);
    let dev_profile = opt_str(args, "dev_profile").unwrap_or(DEV_PROFILE);

    // Each stage's arguments, built up front so the dry run can list them.
    let mut build_args = json!({
        "spec": path_text(&spec), "out": path_text(&plugin), "overwrite": true,
    });
    for key in ["profile", "plugins", "data_dir", "no_cache", "language"] {
        if let (Some(v), Some(m)) = (args.get(key), build_args.as_object_mut()) {
            m.insert(key.to_string(), v.clone());
        }
    }
    let mut package_extra = json!({
        "plugin": path_text(&plugin), "out": path_text(&package_out), "name": package_name,
        "spec": path_text(&spec), "overwrite": true,
    });
    for key in ["loose", "scripts", "ba2", "version"] {
        if let (Some(v), Some(m)) = (args.get(key), package_extra.as_object_mut()) {
            m.insert(key.to_string(), v.clone());
        }
    }
    let mut deploy_args = json!({
        "source": path_text(&package_dir), "dev": true, "profile": dev_profile,
    });
    if let (Some(m), Some(d)) = (deploy_args.as_object_mut(), &mo2) {
        m.insert("mo2".into(), json!(d));
    }
    if let (Some(m), Some(c)) = (deploy_args.as_object_mut(), args.get("category")) {
        m.insert("category".into(), c.clone());
    }
    let mut session_args = json!({
        "probes": path_text(&probes), "profile": dev_profile,
    });
    if let Some(m) = session_args.as_object_mut() {
        if let Some(d) = &mo2 {
            m.insert("mo2".into(), json!(d));
        }
        for key in ["game", "base_save", "base_save_sha256", "observe", "ledger"] {
            if let Some(v) = args.get(key) {
                m.insert(key.to_string(), v.clone());
            }
        }
    }
    let mut stages = vec![
        Stage::new("build", "build", build_args),
        Stage::new(
            "validate",
            "validate",
            with(&lo, json!({"plugin": path_text(&plugin)})),
        ),
        Stage::new(
            "roundtrip",
            "roundtrip",
            json!({"plugin": path_text(&plugin), "verify": true}),
        ),
        Stage::new("package", "package", with(&lo, package_extra)),
        Stage::new("deploy", "deploy", deploy_args),
        Stage::new(
            "testplan",
            "testplan",
            with(
                &lo,
                json!({"plugin": path_text(&plugin), "out": path_text(&testplan_out), "spec": path_text(&spec)}),
            ),
        ),
        Stage::new("session", "game.session", session_args),
    ];

    if dry_run {
        return Ok(dry(
            session,
            &mut stages,
            &name,
            &spec,
            &spec_sha,
            &dir,
            until_at,
        ));
    }

    let stages_dir = dir.join("stages");
    // A rerun starts the stage envelopes afresh (the folder is the run's own).
    let _ = std::fs::remove_dir_all(&stages_dir);
    std::fs::create_dir_all(&dir).map_err(|e| failed(ID, format!("{}: {e}", dir.display())))?;

    let mut session_info = json!({"ran": false});
    let mut stopped: Option<String> = None;
    let mut index = 0usize;
    for stage in &mut stages {
        index = index.saturating_add(1);
        if let Some(why) = &stopped {
            stage.reason.clone_from(why);
            continue;
        }
        let position = index.saturating_sub(1);
        if stage.name == "deploy" && mo2.is_none() {
            stage.status = "fail";
            stage.reason =
                "deploy needs `mo2` (the MO2 instance root); it is never guessed (ADR 0009)".into();
            stage.summary = stage.reason.clone();
            stopped = Some("not-run (stopped at the failing `deploy` stage)".into());
            continue;
        }
        if stage.name == "session" {
            session_info = run_session(session, stage, &stages_dir, index);
        } else {
            let env = run_stage(session, stage, Some(&stages_dir), index, "");
            if let Some(env) = env {
                collect_facts(stage, &env, &plugin);
            }
        }
        if stage.status == "fail" {
            stopped = Some(format!(
                "not-run (stopped at the failing `{}` stage)",
                stage.name
            ));
        } else if position >= until_at {
            stopped = Some(format!("not-run (--until {until})"));
        }
    }

    let rows = probe_rows(&probes, &stages, &session_info);
    let failed_stage = stages.iter().find(|s| s.status == "fail").map(|s| s.name);
    let any_probe_fail = rows.iter().any(|r| r.status == "fail");
    let all_pass = stages.iter().all(|s| s.status == "pass")
        && !rows.is_empty()
        && rows.iter().all(|r| r.status == "pass");
    let result = if failed_stage.is_some() || any_probe_fail {
        "fail"
    } else if all_pass {
        "pass"
    } else {
        "partial"
    };
    let ledger_entry = session_info
        .get("ledger_entry")
        .cloned()
        .unwrap_or(Value::Null);
    let next = next_steps(&stages, &session_info, result);
    let summary = format!(
        "mod run `{name}`: {result}; stages {}; probes {}; report {}",
        stages
            .iter()
            .map(|s| format!("{} {}", s.name, s.status))
            .collect::<Vec<_>>()
            .join(", "),
        probe_counts(&rows),
        path_text(&dir.join(REPORT_MD)),
    );
    let data = json!({
        "kind": KIND,
        "name": name,
        "spec": {"path": path_text(&spec), "sha256": spec_sha},
        "plugin": path_text(&plugin),
        "dir": path_text(&dir),
        "report": path_text(&dir.join(REPORT_MD)),
        "result": result,
        "passed": result != "fail",
        "dry_run": false,
        "until": until,
        "failed_stage": failed_stage,
        "stages": stages.iter().map(Stage::json).collect::<Vec<_>>(),
        "session": session_info,
        "ledger_entry": ledger_entry,
        "probes": rows.iter().map(|r| json!({"row": r.row, "title": r.title, "status": r.status, "reason": (!r.reason.is_empty()).then_some(&r.reason)})).collect::<Vec<_>>(),
        "next_steps": next,
        "summary": summary,
    });
    let md = report_md(&data, &stages, &rows);
    std::fs::write(dir.join(REPORT_MD), md)
        .map_err(|e| failed(ID, format!("{}: {e}", dir.join(REPORT_MD).display())))?;
    let text = serde_json::to_string_pretty(&data).map_err(EngineError::Serialize)?;
    std::fs::write(dir.join(REPORT_JSON), text)
        .map_err(|e| failed(ID, format!("{}: {e}", dir.join(REPORT_JSON).display())))?;
    let mut limitations = vec![
        "each stage's own limitations are in data.stages[].limitations and its envelope file; this command adds no format logic".to_string(),
    ];
    if session_info.get("ran") != Some(&Value::Bool(true)) {
        limitations.push("no game session ran: every probe is not-run, so nothing in game is claimed (AGENTS.md, Reporting)".into());
    }
    Ok(Outcome {
        data,
        coverage: Coverage::unknown(),
        limitations,
        evidence: Vec::new(),
    })
}

/// Facts the report surfaces from a stage's envelope (and, after `build`, the plugin's
/// size and SHA-256, which the ledger and the package manifest also carry).
fn collect_facts(stage: &mut Stage, env: &Value, plugin: &Path) {
    let data = env.get("data").unwrap_or(&Value::Null);
    if stage.name == "build" && stage.status == "pass" {
        if let Ok((size, sha)) = crate::deploy::sha256::file(plugin) {
            stage.facts.insert("sha256".into(), json!(sha));
            stage.facts.insert("size".into(), json!(size));
        }
    }
    let keys: &[&str] = match stage.name {
        "build" => &["new_forms", "changed_fields"],
        "validate" => &["errors", "warnings", "findings_total"],
        "roundtrip" => &["identical", "first_mismatch"],
        "package" => &["folder", "zip"],
        "deploy" => &["mod", "target", "deployment_id", "outcome", "profile"],
        "testplan" => &["probes"],
        _ => &[],
    };
    for k in keys {
        if let Some(v) = data.get(*k) {
            if !v.is_null() {
                stage.facts.insert((*k).to_string(), v.clone());
            }
        }
    }
    if stage.name == "deploy" {
        if let Some(n) = data.get("next_steps") {
            stage.facts.insert("next_steps".into(), n.clone());
        }
    }
}

/// The session stage: preflight by `game.session --dry-run`; when it would start, the
/// session itself (synchronously). Returns what the report says about it.
fn run_session(session: &mut Session, stage: &mut Stage, stages_dir: &Path, index: usize) -> Value {
    if stage.args.get("mo2").is_none() {
        stage.status = "fail";
        stage.reason = "game.session needs `mo2`".into();
        stage.summary = stage.reason.clone();
        return json!({"ran": false});
    }
    let live_args = stage.args.clone();
    let mut pre = Stage::new("session", "game.session", with_flag(&live_args, "dry_run"));
    let env = run_stage(session, &mut pre, Some(stages_dir), index, "-preflight");
    let Some(env) = env else {
        stage.status = "fail";
        stage.reason = format!("preflight: {}", pre.reason);
        stage.summary = pre.summary;
        stage.error = pre.error;
        return json!({"ran": false});
    };
    let data = env.get("data").cloned().unwrap_or(Value::Null);
    let refusals = data.get("refusals").cloned().unwrap_or(json!([]));
    let would_start = data.get("would_start") == Some(&Value::Bool(true));
    stage.limitations = pre.limitations.clone();
    if !would_start {
        let rules: Vec<String> = refusals
            .as_array()
            .map(|a| {
                let mut r: Vec<String> = a
                    .iter()
                    .filter_map(|x| x.get("rule").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect();
                r.sort();
                r.dedup();
                r
            })
            .unwrap_or_default();
        stage.status = "not-run";
        stage.reason = format!("refused by preflight: {}", rules.join(", "));
        stage.summary = data
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        stage.envelope = pre.envelope;
        return json!({
            "ran": false, "refused": true, "rules": rules, "refusals": refusals,
            "plan": data.get("plan").cloned().unwrap_or(Value::Null),
        });
    }
    let live = run_stage(session, stage, Some(stages_dir), index, "");
    let Some(env) = live else {
        return json!({"ran": false, "refused": false, "preflight": pre.envelope});
    };
    let report = env
        .get("data")
        .and_then(|d| d.get("report"))
        .cloned()
        .unwrap_or(Value::Null);
    let (status, reason) = verdict_parts(report.get("status").unwrap_or(&Value::Null));
    stage.status = match status.as_str() {
        "pass" => "pass",
        "fail" => "fail",
        "not-run" => "not-run",
        _ => "inconclusive",
    };
    stage.reason = reason;
    let ledger = report.get("ledger_entry").cloned().unwrap_or(Value::Null);
    stage.facts.insert("ledger_entry".into(), ledger.clone());
    json!({
        "ran": true, "refused": false,
        "session": report.get("session").cloned().unwrap_or(Value::Null),
        "status": status,
        "ledger_entry": ledger,
        "findings": report.get("findings").cloned().unwrap_or(json!([])),
        "rows": report.get("rows").cloned().unwrap_or(json!([])),
        "preflight": pre.envelope,
    })
}

fn with_flag(args: &Value, name: &str) -> Value {
    let mut a = args.clone();
    if let Some(m) = a.as_object_mut() {
        m.insert(name.to_string(), Value::Bool(true));
    }
    a
}

/// One row per probe of `probes.toml`: the session's verdict when it ran, else `not-run`
/// with why.
fn probe_rows(probes: &Path, stages: &[Stage], session: &Value) -> Vec<Row> {
    let Some(file) = std::fs::read(probes)
        .ok()
        .and_then(|b| ProbeFile::parse(&String::from_utf8_lossy(&b)).ok())
    else {
        return Vec::new();
    };
    let ran: Vec<&Value> = session
        .get("rows")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let why_not = || -> String {
        let s = stages.iter().find(|s| s.name == "session");
        match s {
            Some(s) if !s.reason.is_empty() => format!("session {}", s.reason),
            Some(s) => format!("session {}", s.status),
            None => "no session".into(),
        }
    };
    file.probes
        .iter()
        .map(|p| {
            let hit = ran
                .iter()
                .find(|r| r.get("row").and_then(Value::as_str) == Some(p.row.as_str()));
            let (status, reason) = match hit {
                Some(r) => verdict_parts(r.get("verdict").unwrap_or(&Value::Null)),
                None => ("not-run".to_string(), why_not()),
            };
            Row {
                row: p.row.clone(),
                title: p.title.clone(),
                status,
                reason,
            }
        })
        .collect()
}

fn probe_counts(rows: &[Row]) -> String {
    if rows.is_empty() {
        return "none (no probes.toml)".into();
    }
    let count = |s: &str| rows.iter().filter(|r| r.status == s).count();
    format!(
        "{} pass, {} fail, {} inconclusive, {} not-run",
        count("pass"),
        count("fail"),
        count("inconclusive"),
        count("not-run")
    )
}

/// What to do next, from the stages and the session.
fn next_steps(stages: &[Stage], session: &Value, result: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(s) = stages.iter().find(|s| s.status == "fail") {
        out.push(format!(
            "Fix the `{}` stage ({}) and rerun the same `kit mod run`; the fix loop per spec kind is in {PLAYBOOK}.",
            s.name, s.reason
        ));
        return out;
    }
    let rules: Vec<&str> = session
        .get("rules")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for rule in &rules {
        let step = match *rule {
            "unverified-profile" => "The game profile has no evidence yet for its launch, console and save facts: M4.T1 records them in docs/game-observations.md (needs M4.O7, the owner's console tools); until then every probe stays not-run.",
            "plugin-not-deployed" => "Enable the `KitKraft Dev - <plugin>` mod (and tick the plugin, last) in the KitKraft Dev profile in MO2: deploy never edits a profile (ADR 0009); the deploy stage's next_steps say how.",
            "console-plugin" => "The Console API is not an enabled mod of the dev profile (M4.O7, the owner's).",
            "base-save" => "Give --base-save-sha256 (the SHA-256 recorded for KitKraftBase at M4.O7).",
            "ini-switch" | "profile-settings" => "The dev profile lacks the INI switches or profile-specific INIs (M4.O7; docs/game-sessions.md, Preflight).",
            "running" => "Starfield, the CK or MO2 is running: the session never starts beside them; rerun when they are closed.",
            "selected-profile" => "Select the KitKraft Dev profile in MO2 (the session refuses any other).",
            _ => "See docs/game-sessions.md, Preflight, for this rule.",
        };
        out.push(format!("`{rule}`: {step}"));
    }
    if result == "partial" && rules.is_empty() && session.get("ran") != Some(&Value::Bool(true)) {
        out.push("Rerun without --until to reach the session stage.".into());
    }
    if session.get("ran") == Some(&Value::Bool(true)) && result != "pass" {
        out.push(format!(
            "A probe is not pass: read its row in the ledger entry and the observe folder; the fix loop is in {PLAYBOOK}."
        ));
    }
    out
}

static NULL: Value = Value::Null;

/// `v.key`, or `null`.
fn at<'v>(v: &'v Value, key: &str) -> &'v Value {
    v.get(key).unwrap_or(&NULL)
}

/// `report.md`.
fn report_md(data: &Value, stages: &[Stage], rows: &[Row]) -> String {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let mut md = String::new();
    let _ = writeln!(md, "# Mod run: {}\n", s(at(data, "name")));
    let _ = writeln!(
        md,
        "Written by `kit mod run` (`mod.run`, M4.T10). Every stage below is one catalog command; its full envelope is the file named in its row. How to read this report and what to do next: `{PLAYBOOK}`.\n"
    );
    let _ = writeln!(md, "| Field | Value |\n| ----- | ----- |");
    let _ = writeln!(md, "| Result | **{}** |", s(at(data, "result")));
    let _ = writeln!(
        md,
        "| Spec | `{}` (SHA-256 `{}`) |",
        s(at(at(data, "spec"), "path")),
        s(at(at(data, "spec"), "sha256"))
    );
    let _ = writeln!(md, "| Plugin | `{}` |", s(at(data, "plugin")));
    if let Some(b) = stages.iter().find(|x| x.name == "build") {
        if let Some(sha) = b.facts.get("sha256").and_then(Value::as_str) {
            let _ = writeln!(md, "| Plugin SHA-256 | `{sha}` |");
        }
    }
    let ledger = at(data, "ledger_entry")
        .as_str()
        .map_or_else(|| "none (no session ran)".to_string(), |l| format!("`{l}`"));
    let _ = writeln!(md, "| Ledger entry | {ledger} |");
    let _ = writeln!(md, "| Until | `{}` |", s(at(data, "until")));
    let _ = writeln!(md, "| Probes | {} |\n", probe_counts(rows));

    let _ = writeln!(md, "## Stages\n");
    let _ = writeln!(
        md,
        "| # | Stage | Command | Status | Summary | Envelope |\n| - | ----- | ------- | ------ | ------- | -------- |"
    );
    for (i, st) in stages.iter().enumerate() {
        let status = if st.reason.is_empty() {
            st.status.to_string()
        } else {
            format!("{} ({})", st.status, st.reason.replace('|', "\\|"))
        };
        let _ = writeln!(
            md,
            "| {} | {} | `{}` | {} | {} | {} |",
            i.saturating_add(1),
            st.name,
            st.command,
            status,
            first_line(&st.summary),
            st.envelope
                .as_deref()
                .map_or_else(|| "-".to_string(), |e| format!("`{e}`")),
        );
    }
    let _ = writeln!(md);
    for st in stages
        .iter()
        .filter(|x| x.status != "not-run" || x.name == "session")
    {
        if st.summary.is_empty() && st.limitations.is_empty() && st.facts.is_empty() {
            continue;
        }
        let _ = writeln!(md, "### {}\n", st.name);
        let args = serde_json::to_string(&st.args).unwrap_or_default();
        let _ = writeln!(md, "Arguments: `{args}`\n");
        if !st.summary.is_empty() {
            let _ = writeln!(md, "```text\n{}\n```\n", st.summary.trim_end());
        }
        for (k, v) in &st.facts {
            let text = match v {
                Value::String(t) => t.clone(),
                other => serde_json::to_string(other).unwrap_or_default(),
            };
            let _ = writeln!(md, "- {k}: `{text}`");
        }
        if !st.facts.is_empty() {
            let _ = writeln!(md);
        }
        if !st.limitations.is_empty() {
            let _ = writeln!(md, "Limitations:\n");
            for l in &st.limitations {
                let _ = writeln!(md, "- {l}");
            }
            let _ = writeln!(md);
        }
    }

    let session = at(data, "session");
    let _ = writeln!(md, "## Session\n");
    if at(session, "ran").as_bool() == Some(true) {
        let _ = writeln!(
            md,
            "Session `{}`: {}; ledger entry {}.\n",
            s(at(session, "session")),
            s(at(session, "status")),
            ledger
        );
        if let Some(f) = at(session, "findings").as_array().filter(|f| !f.is_empty()) {
            let _ = writeln!(md, "Findings:\n");
            for x in f {
                let _ = writeln!(md, "- {}", x.as_str().unwrap_or_default());
            }
            let _ = writeln!(md);
        }
    } else if at(session, "refused").as_bool() == Some(true) {
        let _ = writeln!(
            md,
            "Refused by preflight (`game.session --dry-run`); no game was launched and nothing was written by the session.\n"
        );
        let _ = writeln!(md, "| Rule | Reason |\n| ---- | ------ |");
        for r in at(session, "refusals").as_array().into_iter().flatten() {
            let _ = writeln!(
                md,
                "| `{}` | {} |",
                s(at(r, "rule")),
                s(at(r, "message")).replace('|', "\\|")
            );
        }
        let _ = writeln!(md);
    } else {
        let _ = writeln!(md, "Not run.\n");
    }

    let _ = writeln!(md, "## Probes\n");
    if rows.is_empty() {
        let _ = writeln!(md, "No `probes.toml` (the testplan stage did not run).\n");
    } else {
        let _ = writeln!(
            md,
            "| Row | Title | Status | Reason |\n| --- | ----- | ------ | ------ |"
        );
        for r in rows {
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} |",
                r.row,
                r.title.replace('|', "\\|"),
                r.status,
                r.reason.replace('|', "\\|")
            );
        }
        let _ = writeln!(md);
    }

    let _ = writeln!(md, "## Next steps\n");
    let next: Vec<&Value> = at(data, "next_steps")
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    if next.is_empty() {
        let _ = writeln!(md, "None: every stage and every probe passed.");
    }
    for n in next {
        let _ = writeln!(md, "- {}", n.as_str().unwrap_or_default());
    }
    md
}

/// `dry_run`: `build --plan` (writes nothing) and every stage's arguments.
fn dry(
    session: &mut Session,
    stages: &mut [Stage],
    name: &str,
    spec: &Path,
    spec_sha: &str,
    dir: &Path,
    until_at: usize,
) -> Outcome {
    let mut index = 0usize;
    for stage in stages.iter_mut() {
        index = index.saturating_add(1);
        if stage.name == "build" {
            let mut plan = stage.clone();
            plan.args = with_flag(&stage.args, "plan");
            run_stage(session, &mut plan, None, index, "");
            stage.status = plan.status;
            stage.reason = if plan.status == "pass" {
                "build --plan only (dry run)".into()
            } else {
                plan.reason
            };
            stage.summary = plan.summary;
            stage.limitations = plan.limitations;
            stage.error = plan.error;
        } else if index.saturating_sub(1) > until_at {
            stage.reason = "not-run (--until)".into();
        } else {
            stage.reason = "not-run (dry run)".into();
        }
    }
    let failed_build = stages.iter().any(|s| s.status == "fail");
    let summary = format!(
        "mod run `{name}` dry run: build --plan {}; would write {} (nothing written)",
        if failed_build { "FAILED" } else { "passed" },
        path_text(dir)
    );
    Outcome {
        data: json!({
            "kind": KIND,
            "name": name,
            "spec": {"path": path_text(spec), "sha256": spec_sha},
            "dir": path_text(dir),
            "dry_run": true,
            "result": if failed_build { "fail" } else { "dry-run" },
            "passed": !failed_build,
            "stages": stages.iter().map(Stage::json).collect::<Vec<_>>(),
            "summary": summary,
        }),
        coverage: Coverage::unknown(),
        limitations: vec![
            "dry run: only `build --plan` ran; no other stage ran and nothing was written".into(),
        ],
        evidence: Vec::new(),
    }
}
