//! `doctor` (CLI `kit doctor`): what KitKraft finds on this machine, read-only, one line
//! per item with the exact path, how it was found, and `OK` or a one-line fix.
//!
//! Detection is [`kitkraft_io::host`] (registry through `reg.exe query`, files only read);
//! the same results are every command's default Data dir and profile-name lookup
//! ([`crate::host`]: argument > environment > detected). `get` prints one detected path
//! alone (for scripts: the dist folder's `.cmd` files). Nothing is written.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::failed;
use crate::deploy::LIVE_PROFILE;
use crate::envelope::{Coverage, CoverageStatus, Outcome};
use crate::exec::EngineError;
use crate::host::{detect, mo2_from_env, Detected, DEV_PROFILE};
use crate::install::{
    command_of, kit_servers, profile_arg, CLAUDE_JSON, LEGACY_SERVERS, MCP_JSON, SERVER,
};
use kitkraft_io::host::{dir_size, native_path, pe_file_version, same_path, Found, Host};
use kitkraft_io::Instance;

const ID: &str = "doctor";

/// One line of the report.
#[derive(Debug, Clone)]
struct Check {
    id: &'static str,
    label: &'static str,
    /// What was found (a path, a version, a list).
    value: String,
    /// The path, when there is one.
    path: Option<PathBuf>,
    /// How it was found.
    how: Option<String>,
    /// `None`: OK. Otherwise the one-line fix.
    fix: Option<String>,
    /// Counted in `problems` (and `passed`) when it needs a fix.
    essential: bool,
}

impl Check {
    fn ok(id: &'static str, label: &'static str, value: impl Into<String>) -> Check {
        Check {
            id,
            label,
            value: value.into(),
            path: None,
            how: None,
            fix: None,
            essential: false,
        }
    }

    fn found(id: &'static str, label: &'static str, f: &Found) -> Check {
        Check {
            path: Some(f.path.clone()),
            how: Some(f.how.clone()),
            ..Check::ok(id, label, f.path.display().to_string())
        }
    }

    fn fix(mut self, fix: impl Into<String>) -> Check {
        self.fix = Some(fix.into());
        self
    }

    fn essential(mut self) -> Check {
        self.essential = true;
        self
    }

    fn line(&self) -> String {
        let how = self
            .how
            .as_deref()
            .map_or_else(String::new, |h| format!(" [{h}]"));
        let end = self
            .fix
            .as_deref()
            .map_or_else(|| "OK".to_string(), |f| format!("FIX: {f}"));
        format!("{:<22} {}{how}  {end}", self.label, self.value)
    }

    fn json(&self) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "value": self.value,
            "path": self.path.as_ref().map(|p| p.display().to_string()),
            "how": self.how,
            "ok": self.fix.is_none(),
            "fix": self.fix,
            "essential": self.essential,
        })
    }
}

/// The values `get` can print.
const GET: &[&str] = &[
    "game_dir",
    "data_dir",
    "mo2",
    "active_profile",
    "dev_profile",
    "cache_dir",
    "kit",
];

fn str_opt<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

/// The cache folder and where its location came from (as `kitkraft_doc::IndexCache`
/// picks it, without touching it).
fn cache_dir() -> Option<(PathBuf, &'static str)> {
    let named = |n: &str| std::env::var_os(n).filter(|v| !v.is_empty());
    if let Some(d) = named("KITKRAFT_CACHE_DIR") {
        return Some((PathBuf::from(d), "environment variable KITKRAFT_CACHE_DIR"));
    }
    if let Some(d) = named("KITCRAFT_CACHE_DIR") {
        return Some((
            PathBuf::from(d),
            "environment variable KITCRAFT_CACHE_DIR (deprecated)",
        ));
    }
    named("LOCALAPPDATA").map(|l| {
        (
            PathBuf::from(l).join("kitkraft").join("cache"),
            "default %LOCALAPPDATA%\\kitkraft\\cache",
        )
    })
}

/// The home folder holding `.claude.json`.
fn home(args: &Value) -> Option<PathBuf> {
    str_opt(args, "home").map(PathBuf::from).or_else(|| {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
    })
}

/// The project folder for `.mcp.json`: the argument, else the repository above this
/// program, else above the working directory.
fn project(args: &Value) -> Option<PathBuf> {
    if let Some(p) = str_opt(args, "project") {
        return Some(PathBuf::from(p));
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    exe_dir
        .and_then(|d| crate::install::find_project(&d))
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|d| crate::install::find_project(&d))
        })
}

fn sha(path: &Path) -> Option<String> {
    crate::deploy::sha256::file(path).ok().map(|(_, h)| h)
}

/// The doctor's report on this machine (or with the overrides in `args`).
fn checks(args: &Value, host: &Host, detected: &Detected) -> (Vec<Check>, Value) {
    let mut out: Vec<Check> = Vec::new();
    let mut paths = serde_json::Map::new();

    // This program.
    let exe = std::env::current_exe().ok();
    let exe_hash = exe.as_deref().and_then(sha);
    if let Some(e) = &exe {
        paths.insert("kit".into(), json!(e.display().to_string()));
        out.push(Check {
            path: Some(e.clone()),
            how: Some(format!(
                "this program, version {}, sha256 {}",
                env!("CARGO_PKG_VERSION"),
                exe_hash
                    .as_deref()
                    .and_then(|h| h.get(..12))
                    .unwrap_or("unknown")
            )),
            ..Check::ok("kit", "KitKraft (kit)", e.display().to_string())
        });
    }

    // Game install.
    let game = detected.game.clone();
    match &game {
        Some(g) => out.push(Check::found("game", "Game install", g).essential()),
        None => out.push(
            Check::ok("game", "Game install", "not found")
                .fix(format!(
                    "install Starfield through Steam, or pass --data-dir / set KITKRAFT_DATA_DIR (looked at {} places)",
                    detected.game_looked.len()
                ))
                .essential(),
        ),
    }
    if let Some(g) = &game {
        paths.insert("game_dir".into(), json!(g.path.display().to_string()));
    }

    // Data dir: argument > environment > detected.
    let data = super::data_dir_or(
        args,
        super::env_var_os(super::DATA_DIR_ENV),
        detected.data_dir.as_ref().map(|f| f.path.as_path()),
    );
    match &data {
        Some((d, from)) => {
            let how = match *from {
                "data_dir" => "the --data-dir argument".to_string(),
                super::DATA_DIR_ENV => format!("environment variable {}", super::DATA_DIR_ENV),
                _ => detected
                    .data_dir
                    .as_ref()
                    .map_or_else(|| "detected".to_string(), |f| f.how.clone()),
            };
            let mut c = Check {
                path: Some(d.clone()),
                how: Some(how),
                ..Check::ok("data_dir", "Data dir", d.display().to_string())
            }
            .essential();
            if !d.join("Starfield.esm").is_file() {
                c = c.fix("no Starfield.esm there; pass --data-dir with the game's Data folder");
            }
            paths.insert("data_dir".into(), json!(d.display().to_string()));
            out.push(c);
        }
        None => out.push(
            Check::ok("data_dir", "Data dir", "not found")
                .fix("pass --data-dir <game>\\Data (or set KITKRAFT_DATA_DIR)")
                .essential(),
        ),
    }

    // Game version.
    let game_version = game
        .as_ref()
        .and_then(|g| pe_file_version(&g.path.join("Starfield.exe")));
    match (&game, &game_version) {
        (Some(g), Some(v)) => out.push(Check {
            how: Some(format!(
                "file version of {}",
                g.path.join("Starfield.exe").display()
            )),
            ..Check::ok("game_version", "Game version", v.clone())
        }),
        (Some(_), None) => {
            out.push(Check::ok("game_version", "Game version", "unknown").fix(
                "Starfield.exe has no readable version resource; verify the game files in Steam",
            ))
        }
        (None, _) => out.push(
            Check::ok("game_version", "Game version", "unknown")
                .fix("find the game install first (above)"),
        ),
    }

    // MO2 instance: argument, else KITKRAFT_MO2_DIR, else detected (as `profiles`).
    let mo2: Option<Found> = str_opt(args, "mo2")
        .map(|m| Found {
            path: PathBuf::from(m),
            how: "the --mo2 argument".to_string(),
        })
        .or_else(|| mo2_from_env(&*host.env))
        .or_else(|| detected.mo2_root().cloned());
    let instance = mo2.as_ref().and_then(|m| Instance::read(&m.path).ok());
    match (&mo2, &instance) {
        (Some(m), Some(_)) => {
            let mut c = Check::found("mo2", "MO2 instance", m).essential();
            let others: Vec<String> = detected
                .mo2
                .iter()
                .filter(|o| !same_path(&o.path, &m.path))
                .map(|o| o.path.display().to_string())
                .collect();
            if !others.is_empty() {
                c.value = format!("{} (also found: {})", c.value, others.join(", "));
            }
            paths.insert("mo2".into(), json!(m.path.display().to_string()));
            out.push(c);
        }
        (Some(m), None) => out.push(
            Check::found("mo2", "MO2 instance", m)
                .fix("no ModOrganizer.ini there; pass --mo2 with the MO2 folder")
                .essential(),
        ),
        (None, _) => out.push(
            Check::ok("mo2", "MO2 instance", "not found")
                .fix(format!(
                    "install MO2 portable at {} or pass --mo2 <folder>",
                    kitkraft_io::host::DEFAULT_MO2
                ))
                .essential(),
        ),
    }

    // Profiles and the dev profile.
    if let Some(inst) = &instance {
        let names = inst.profiles().unwrap_or_default();
        let active = inst.selected_profile.clone();
        let listed: Vec<String> = names
            .iter()
            .map(|n| {
                if active.as_deref() == Some(n.as_str()) {
                    format!("{n} (active)")
                } else {
                    n.clone()
                }
            })
            .collect();
        let mut c = Check {
            path: Some(inst.profiles_dir.clone()),
            how: Some(format!(
                "folders with modlist.txt in {}; active = selected_profile in {}",
                inst.profiles_dir.display(),
                inst.ini_path().display()
            )),
            ..Check::ok("profiles", "MO2 profiles", listed.join(", "))
        }
        .essential();
        if names.is_empty() {
            c.value = "none".into();
            c = c.fix("create a profile in MO2");
        }
        out.push(c);
        if let Some(a) = active.as_ref().filter(|a| names.contains(a)) {
            paths.insert(
                "active_profile".into(),
                json!(inst.profile_dir(a).display().to_string()),
            );
        }
        let dev = names
            .iter()
            .find(|n| n.as_str() == DEV_PROFILE)
            .or_else(|| names.iter().find(|n| n.eq_ignore_ascii_case(DEV_PROFILE)));
        match dev {
            Some(d) => {
                let dir = inst.profile_dir(d);
                paths.insert("dev_profile".into(), json!(dir.display().to_string()));
                out.push(
                    Check {
                        path: Some(dir.clone()),
                        how: Some(format!("profile `{d}` (name matched ignoring case)")),
                        ..Check::ok("dev_profile", "KitKraft Dev profile", dir.display().to_string())
                    }
                    .essential(),
                );
            }
            None => out.push(
                Check::ok("dev_profile", "KitKraft Dev profile", "missing")
                    .fix(format!(
                        "in MO2, copy the Default profile to `{DEV_PROFILE}` with profile-specific saves (AGENTS.md)"
                    ))
                    .essential(),
            ),
        }
    } else {
        out.push(
            Check::ok("profiles", "MO2 profiles", "unknown")
                .fix("find the MO2 instance first (above)")
                .essential(),
        );
        out.push(
            Check::ok("dev_profile", "KitKraft Dev profile", "unknown")
                .fix("find the MO2 instance first (above)")
                .essential(),
        );
    }

    // Tools.
    let mo2_root = mo2.as_ref().map(|m| m.path.as_path());
    let xedit = host.find_tool("xSFEdit64.exe", Some("KITKRAFT_SF1EDIT"), mo2_root);
    out.push(match &xedit {
        Some(f) => Check::found("xedit", "xEdit (xSFEdit64)", f),
        None => Check::ok("xedit", "xEdit (xSFEdit64)", "not found").fix(
            "optional (oracle runs): unpack xEdit 4.1.5+ to C:\\Modding\\tools\\xEdit, or set KITKRAFT_SF1EDIT",
        ),
    });
    let xdump = host.find_tool("xDump64.exe", Some("KITKRAFT_SF1EDIT"), mo2_root);
    out.push(match &xdump {
        Some(f) => Check::found("xdump", "xDump64", f),
        None => Check::ok("xdump", "xDump64", "not found")
            .fix("optional (oracle runs): it ships with xEdit; put it next to xSFEdit64.exe"),
    });
    let game_tool = |rel: &[&str]| {
        game.as_ref().map(|g| {
            let p = rel.iter().fold(g.path.clone(), |p, c| p.join(c));
            (p, "in the game install folder".to_string())
        })
    };
    let compiler = super::env_var_os("KITKRAFT_PAPYRUS_COMPILER")
        .map(|p| {
            (
                PathBuf::from(p),
                "environment variable KITKRAFT_PAPYRUS_COMPILER".to_string(),
            )
        })
        .or_else(|| game_tool(&["Tools", "Papyrus Compiler", "PapyrusCompiler.exe"]));
    out.push(match compiler {
        Some((p, how)) if p.is_file() => Check {
            path: Some(p.clone()),
            how: Some(how),
            ..Check::ok(
                "papyrus_compiler",
                "Papyrus compiler",
                p.display().to_string(),
            )
        },
        _ => Check::ok("papyrus_compiler", "Papyrus compiler", "not found")
            .fix("install the Creation Kit from Steam (it adds Tools\\Papyrus Compiler)"),
    });
    out.push(
        match game_tool(&["Tools", "ContentResources.zip"]).filter(|(p, _)| p.is_file()) {
            Some((p, how)) => Check {
                path: Some(p.clone()),
                how: Some(how),
                ..Check::ok(
                    "content_resources",
                    "ContentResources.zip",
                    p.display().to_string(),
                )
            },
            None => Check::ok("content_resources", "ContentResources.zip", "not found").fix(
                "install the Creation Kit from Steam (Tools\\ContentResources.zip: script sources)",
            ),
        },
    );
    out.push(sfse_check(game.as_ref(), game_version.as_deref()));

    // Cache.
    match cache_dir() {
        Some((dir, how)) => {
            let (files, bytes) = dir_size(&dir, 8, 200_000);
            let mib = bytes / (1024 * 1024);
            paths.insert("cache_dir".into(), json!(dir.display().to_string()));
            out.push(Check {
                path: Some(dir.clone()),
                how: Some(how.to_string()),
                ..Check::ok(
                    "cache",
                    "Index cache",
                    format!("{} ({files} files, {mib} MiB)", dir.display()),
                )
            });
        }
        None => out.push(Check::ok("cache", "Index cache", "none").fix(
            "set LOCALAPPDATA or KITKRAFT_CACHE_DIR; without one every load indexes from the files",
        )),
    }

    // MCP registrations.
    let home = home(args);
    out.extend(mcp_checks(
        home.as_ref().map(|h| h.join(CLAUDE_JSON)),
        project(args).map(|p| p.join(MCP_JSON)),
        exe.as_deref(),
        exe_hash.as_deref(),
    ));
    (out, Value::Object(paths))
}

fn sfse_check(game: Option<&Found>, version: Option<&str>) -> Check {
    let Some(g) = game else {
        return Check::ok("sfse", "SFSE", "unknown").fix("find the game install first (above)");
    };
    let loader = g.path.join("sfse_loader.exe");
    if !loader.is_file() {
        return Check::ok("sfse", "SFSE", "not installed")
            .fix("optional (runtime plugins, ADR 0008): install SFSE for this game version into the game folder");
    }
    let dll = std::fs::read_dir(&g.path)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                    let v = n.strip_prefix("sfse_")?.strip_suffix(".dll")?.to_string();
                    v.chars()
                        .all(|c| c.is_ascii_digit() || c == '_')
                        .then(|| v.replace('_', "."))
                })
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    let mut c = Check {
        path: Some(loader.clone()),
        how: Some("sfse_loader.exe and sfse_<version>.dll in the game folder".to_string()),
        ..Check::ok(
            "sfse",
            "SFSE",
            format!("{} (for game {})", loader.display(), dll.join(", ")),
        )
    };
    let matches = version.is_none_or(|v| dll.iter().any(|d| v.starts_with(d.as_str())));
    if dll.is_empty() {
        c = c.fix("sfse_loader.exe without its sfse_<version>.dll; reinstall SFSE");
    } else if !matches {
        c = c.fix(format!(
            "SFSE is built for {} but the game is {}; update SFSE",
            dll.join(", "),
            version.unwrap_or("?")
        ));
    }
    c
}

/// One check per KitKraft MCP server in `~/.claude.json` (user scope) and the project
/// `.mcp.json`, or one saying none is registered.
fn mcp_checks(
    claude: Option<PathBuf>,
    project_mcp: Option<PathBuf>,
    exe: Option<&Path>,
    exe_hash: Option<&str>,
) -> Vec<Check> {
    let mut out = Vec::new();
    for (id, label, file) in [
        ("mcp_user", "MCP (user)", claude),
        ("mcp_project", "MCP (project)", project_mcp),
    ] {
        let Some(file) = file else {
            out.push(
                Check::ok(id, label, "no file to read")
                    .fix("run Install.cmd in the dist folder (kit install --apply)"),
            );
            continue;
        };
        let doc = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        let Some(doc) = doc else {
            out.push(Check {
                how: Some(format!("read {}", file.display())),
                ..Check::ok(id, label, format!("{} missing or not JSON", file.display()))
                    .fix("run Install.cmd in the dist folder (kit install --apply)")
            });
            continue;
        };
        let servers = kit_servers(&doc);
        if servers.is_empty() {
            out.push(Check {
                how: Some(format!("mcpServers in {}", file.display())),
                ..Check::ok(id, label, "no KitKraft server registered")
                    .fix("run Install.cmd in the dist folder (kit install --apply)")
            });
        }
        for (name, entry) in servers {
            let command = command_of(&entry).unwrap_or("").to_string();
            let path = native_path(&command);
            let exists = path.is_file();
            let same = exe.is_some_and(|e| same_path(e, &path))
                || (exists && exe_hash.is_some() && sha(&path).as_deref() == exe_hash);
            let state = match (exists, same) {
                (false, _) => "missing",
                (true, true) => "exists, matches this build",
                (true, false) => "exists, a different build",
            };
            let profile = entry.get("args").and_then(profile_arg);
            // A profile folder may be spelled with Windows separators (`D:\...\Default`):
            // both count as a separator (Windows parses them the same), else a
            // Windows-spelled path reads as one component and `Default` is missed.
            let live = profile.as_deref().is_some_and(|p| {
                let s = native_path(p).to_string_lossy().replace('\\', "/");
                Path::new(&s)
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(LIVE_PROFILE))
            });
            let profile_text = profile
                .as_deref()
                .map_or_else(|| "no --profile".to_string(), |p| format!("profile {p}"));
            let mut c = Check {
                path: Some(path.clone()),
                how: Some(format!("mcpServers.{name} in {}", file.display())),
                ..Check::ok(
                    id,
                    label,
                    format!("{name} -> {command} ({state}), {profile_text}"),
                )
            };
            let mut fixes: Vec<String> = Vec::new();
            if !exists {
                fixes.push(
                    "the command does not exist; run Install.cmd to point it at this build".into(),
                );
            } else if LEGACY_SERVERS.iter().any(|l| name.eq_ignore_ascii_case(l)) {
                fixes.push(format!(
                    "pre-rename name; run Install.cmd to register it as `{SERVER}`"
                ));
            } else if !same {
                fixes.push("another kit.exe; run Install.cmd to point it at this build (then restart Claude Code)".into());
            }
            if live {
                fixes.push(format!(
                    "live game profile; re-run Install.cmd to use {DEV_PROFILE}"
                ));
            }
            if !fixes.is_empty() {
                c = c.fix(fixes.join("; "));
            }
            out.push(c);
        }
    }
    out
}

/// The handler.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let host = Host::system();
    let detected = detect(&host);
    let (checks, paths) = checks(args, &host, &detected);
    if let Some(key) = str_opt(args, "get") {
        if !GET.contains(&key) {
            return Err(failed(
                ID,
                format!("`get` takes one of: {}", GET.join(", ")),
            ));
        }
        let value = paths.get(key).and_then(Value::as_str).ok_or_else(|| {
            failed(
                ID,
                format!("{key}: not detected; run `kit doctor` to see why"),
            )
        })?;
        return Ok(Outcome {
            data: json!({"get": key, "value": value, "summary": value}),
            coverage: coverage(),
            limitations: Vec::new(),
            evidence: Vec::new(),
        });
    }
    let problems = checks
        .iter()
        .filter(|c| c.essential && c.fix.is_some())
        .count();
    let mut summary: Vec<String> = checks.iter().map(Check::line).collect();
    summary.push(String::new());
    summary.push(if problems == 0 {
        "Everything KitKraft needs was found. Optional items marked FIX are only needed for what they name.".to_string()
    } else {
        format!("{problems} essential item(s) need a fix (lines marked FIX above).")
    });
    summary.push(
        "Defaults: --data-dir and profile names fall back to what is detected here (argument > environment > detected)."
            .to_string(),
    );
    Ok(Outcome {
        data: json!({
            "checks": checks.iter().map(Check::json).collect::<Vec<Value>>(),
            "detected": paths,
            "problems": problems,
            "passed": problems == 0,
            "summary": summary.join("\n"),
        }),
        coverage: coverage(),
        limitations: vec![
            "registry values are read through reg.exe; a path with characters outside the console code page may read wrong".to_string(),
        ],
        evidence: Vec::new(),
    })
}

fn coverage() -> Coverage {
    Coverage {
        status: CoverageStatus::Complete,
        decoded_subrecords: None,
        opaque_subrecords: None,
        not_examined: Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use kitkraft_io::host::FixedRegistry;

    fn temp(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("kitkraft-doctor-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// A synthetic machine: a game folder with Data, Tools and SFSE, an MO2 instance with
    /// Default (active) and a lower-case `Kitkraft Dev`, xEdit in a tool folder, and a
    /// home whose `.claude.json` registers a pre-rename server at a missing path.
    #[test]
    fn report_on_a_synthetic_machine() {
        let root = temp("synthetic");
        let game = root.join("Game (x86)");
        for d in ["Data", "Tools/Papyrus Compiler"] {
            std::fs::create_dir_all(game.join(d)).unwrap();
        }
        for f in [
            "Starfield.exe",
            "sfse_loader.exe",
            "sfse_1_16_244.dll",
            "Data/Starfield.esm",
            "Tools/Papyrus Compiler/PapyrusCompiler.exe",
            "Tools/ContentResources.zip",
        ] {
            std::fs::write(game.join(f), b"x").unwrap();
        }
        let mo2 = root.join("MO2");
        for p in ["Default", "Kitkraft Dev"] {
            std::fs::create_dir_all(mo2.join("profiles").join(p)).unwrap();
            std::fs::write(mo2.join("profiles").join(p).join("modlist.txt"), b"").unwrap();
        }
        std::fs::write(
            mo2.join("ModOrganizer.ini"),
            "[General]\r\ngameName=Starfield\r\nselected_profile=@ByteArray(Default)\r\n",
        )
        .unwrap();
        let tools = root.join("xEdit");
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::write(tools.join("xSFEdit64.exe"), b"x").unwrap();
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join(CLAUDE_JSON),
            serde_json::to_vec(&json!({"mcpServers": {"kitcraft": {
                "command": root.join("gone/kit.exe").display().to_string()
            }}}))
            .unwrap(),
        )
        .unwrap();
        let host = Host {
            registry: Box::new(FixedRegistry::default()),
            env: Box::new(|_| None),
            game_candidates: vec![game.clone()],
            mo2_candidates: vec![mo2.clone()],
            tool_dirs: vec![tools.clone()],
        };
        let detected = detect(&host);
        let args = json!({"home": home.display().to_string(), "project": root.join("noproject").display().to_string(), "data_dir": game.join("Data").display().to_string()});
        let (checks, paths) = checks(&args, &host, &detected);
        let by = |id: &str| checks.iter().find(|c| c.id == id).unwrap();
        assert!(by("game").fix.is_none());
        assert_eq!(
            by("data_dir").how.as_deref(),
            Some("the --data-dir argument")
        );
        assert!(by("data_dir").fix.is_none());
        assert!(by("mo2").fix.is_none());
        assert_eq!(by("profiles").value, "Default (active), Kitkraft Dev");
        assert!(by("dev_profile").fix.is_none());
        assert_eq!(
            paths["dev_profile"],
            json!(mo2
                .join("profiles")
                .join("Kitkraft Dev")
                .display()
                .to_string())
        );
        assert_eq!(
            paths["active_profile"],
            json!(mo2.join("profiles").join("Default").display().to_string())
        );
        assert!(by("xedit").fix.is_none());
        assert!(by("xdump").fix.is_some(), "no xDump64 in the tool folder");
        assert!(by("papyrus_compiler").fix.is_none());
        assert!(by("content_resources").fix.is_none());
        // Version unknown (the stand-in exe is not a PE): the SFSE check does not guess.
        assert!(by("game_version").fix.is_some());
        assert!(by("sfse").fix.is_none(), "{:?}", by("sfse").fix);
        assert!(by("sfse").value.contains("1.16.244"));
        let user = by("mcp_user");
        assert!(user.value.starts_with("kitcraft -> "), "{}", user.value);
        assert!(user.value.contains("missing"), "{}", user.value);
        assert!(user.fix.is_some());
        assert!(by("mcp_project").fix.is_some());
        // Every line ends with OK or a fix.
        for c in &checks {
            let l = c.line();
            assert!(l.ends_with("OK") || l.contains("FIX: "), "{l}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Each MCP line names the profile its registration preloads; `Default` (the live
    /// game) gets a FIX pointing at Install.cmd and KitKraft Dev.
    #[test]
    fn mcp_lines_name_the_profile_and_flag_default() {
        let root = temp("mcpprofile");
        let exe = root.join("dist").join("kit.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"MZ").unwrap();
        let command = exe.display().to_string().replace('\\', "/");
        let write = |name: &str, args: Value| {
            let file = root.join(name);
            std::fs::write(
                &file,
                serde_json::to_vec(&json!({"mcpServers": {"kitkraft": {
                    "type": "stdio", "command": command, "args": args, "env": {}
                }}}))
                .unwrap(),
            )
            .unwrap();
            file
        };
        let user = write(
            "claude.json",
            json!([
                "mcp",
                "--profile",
                "C:/Modding/MO2-Starfield/profiles/Default"
            ]),
        );
        let project = write(
            "mcp.json",
            json!([
                "mcp",
                "--profile",
                "C:/Modding/MO2-Starfield/profiles/Kitkraft Dev"
            ]),
        );
        let checks = mcp_checks(Some(user), Some(project), Some(&exe), None);
        assert_eq!(checks.len(), 2);
        let u = &checks[0];
        assert!(
            u.value.ends_with(
                "(exists, matches this build), profile C:/Modding/MO2-Starfield/profiles/Default"
            ),
            "{}",
            u.value
        );
        assert_eq!(
            u.fix.as_deref(),
            Some("live game profile; re-run Install.cmd to use KitKraft Dev")
        );
        assert!(u.line().contains("FIX: live game profile"), "{}", u.line());
        let p = &checks[1];
        assert!(
            p.value
                .ends_with("profile C:/Modding/MO2-Starfield/profiles/Kitkraft Dev"),
            "{}",
            p.value
        );
        assert!(p.fix.is_none(), "{:?}", p.fix);
        // No --profile is said so; a missing command and Default give both fixes.
        let bare = write("bare.json", json!(["mcp"]));
        let c = mcp_checks(Some(bare), None, Some(&exe), None);
        assert!(c[0].value.ends_with(", no --profile"), "{}", c[0].value);
        assert!(c[0].fix.is_none());
        std::fs::remove_file(&exe).unwrap();
        let gone = write(
            "gone.json",
            json!(["mcp", "--profile=D:\\MO2\\profiles\\default"]),
        );
        let c = mcp_checks(Some(gone), None, None, None);
        let fix = c[0].fix.as_deref().unwrap();
        assert!(fix.starts_with("the command does not exist"), "{fix}");
        assert!(
            fix.ends_with("re-run Install.cmd to use KitKraft Dev"),
            "{fix}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sfse_for_another_game_version_needs_a_fix() {
        let root = temp("sfse");
        std::fs::write(root.join("sfse_loader.exe"), b"x").unwrap();
        std::fs::write(root.join("sfse_1_15_222.dll"), b"x").unwrap();
        let g = Found {
            path: root.clone(),
            how: "test".into(),
        };
        let c = sfse_check(Some(&g), Some("1.16.244.0"));
        assert!(c.fix.unwrap().contains("update SFSE"));
        assert!(sfse_check(Some(&g), Some("1.15.222.0")).fix.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Through `execute` with JSON (AGENTS.md: a new command lands with a test that calls
    /// it that way). The report depends on the machine; its shape does not.
    #[test]
    fn doctor_through_execute() {
        let r = crate::execute("doctor", json!({"project": "nowhere"})).unwrap();
        let data = &r["data"];
        assert!(data["checks"].as_array().unwrap().len() >= 14);
        assert!(data["summary"].as_str().unwrap().contains("Game install"));
        assert!(data["passed"].is_boolean());
        let e = crate::execute("doctor", json!({"get": "nope"})).unwrap_err();
        assert!(e.to_string().contains("must be one of"), "{e}");
        let k = crate::execute("doctor", json!({"get": "kit"})).unwrap();
        assert!(k["data"]["value"]
            .as_str()
            .unwrap()
            .contains("kitkraft_engine"));
    }
}
