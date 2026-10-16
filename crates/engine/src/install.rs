//! `kit install` / `kit uninstall`: put the dist folder (`cargo xtask dist`) on the user
//! PATH and register it as the `kitkraft` MCP server for Claude Code.
//!
//! Front-end setup, not a data action: like the launchers `kit mcp` and `kit ui` it has
//! no `CommandSpec`, so it is not reachable over MCP (a connected agent cannot rewrite the
//! user's PATH or its own registration). The CLI plans first and prints the plan; only
//! `--apply` changes anything.
//!
//! What it changes, and nothing else (docs/QUICKSTART.md, "Install"):
//!
//! 1. The user PATH (`HKCU\Environment` `Path`, its value kind kept): the dist folder is
//!    added once. The old value is printed and saved under `<dist>\backups\`. A dist
//!    folder an earlier install recorded at another place (the repository moved) is
//!    replaced. The change is broadcast so new windows see it.
//! 2. `~/.claude.json`: `mcpServers.kitkraft` points at the dist `kit.exe`; its `args`
//!    and `env` come from the earlier `kitkraft` (or pre-rename `kitcraft`) entry, minus a
//!    `*_DATA_DIR` variable that names the Data dir detection finds anyway; a `kitcraft`
//!    entry running a `kit.exe` is removed (and remembered). A backup copy is written next
//!    to the file first. Every other key is kept (key order is normalized).
//! 3. The project's `.mcp.json` (the repository holding `.mcp.json.example`, gitignored):
//!    the same `kitkraft` entry, with a backup when it existed.
//! 4. `<dist>\install-state.json`: what was there before, for `kit uninstall`.
//!
//! Running it again changes nothing. `kit uninstall` removes the PATH entry and the
//! entries that point into the dist folder, restores what install replaced, and deletes
//! the state file. The registry is reached through [`UserEnv`]: [`SystemEnv`] (PowerShell
//! and the .NET registry API, Windows only) or [`FileEnv`] (a JSON stand-in, for tests and
//! dry runs that must not read the real one).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use kitkraft_io::host::same_path;

/// The MCP server name this installs.
pub const SERVER: &str = "kitkraft";
/// Pre-rename server names that install replaces when they run a `kit.exe`.
pub const LEGACY_SERVERS: &[&str] = &["kitcraft"];
/// The state file in the dist folder.
pub const STATE_FILE: &str = "install-state.json";
/// Claude Code's user configuration file in the home folder.
pub const CLAUDE_JSON: &str = ".claude.json";
/// The project-scope MCP file.
pub const MCP_JSON: &str = ".mcp.json";

/// Why install or uninstall stopped.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// A file could not be read or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// What happened.
        #[source]
        source: std::io::Error,
    },
    /// A JSON file is not valid JSON (it is left alone).
    #[error("{}: not valid JSON ({message}); fix or move it, then run again", path.display())]
    Json {
        /// The file.
        path: PathBuf,
        /// The parser's message.
        message: String,
    },
    /// The user environment could not be read or written.
    #[error("user environment: {0}")]
    Env(String),
    /// A file changed between the plan and the apply step.
    #[error("{}: changed since the plan was made (Claude Code may have rewritten it); run again", .0.display())]
    Changed(PathBuf),
    /// The options make no sense (no `kit.exe` in the dist folder, ...).
    #[error("{0}")]
    Invalid(String),
    /// Claude Code is running and the plan rewrites `.claude.json` ([`refuse_while_running`]).
    #[error("{0}")]
    ClaudeRunning(String),
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> InstallError + '_ {
    move |source| InstallError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// A registry value's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvKind {
    /// `REG_SZ`.
    #[serde(rename = "REG_SZ")]
    String,
    /// `REG_EXPAND_SZ` (`%VAR%` expanded by Windows).
    #[serde(rename = "REG_EXPAND_SZ")]
    ExpandString,
}

impl EnvKind {
    fn label(self) -> &'static str {
        match self {
            EnvKind::String => "REG_SZ",
            EnvKind::ExpandString => "REG_EXPAND_SZ",
        }
    }
}

/// The user PATH as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvValue {
    /// Its kind (kept on write).
    pub kind: EnvKind,
    /// The text as stored (`%VAR%` not expanded).
    pub value: String,
}

/// Where the user PATH lives.
pub trait UserEnv {
    /// The stored value, or `None` when there is none.
    fn read_path(&self) -> Result<Option<EnvValue>, InstallError>;
    /// Replaces the stored value.
    fn write_path(&mut self, value: &EnvValue) -> Result<(), InstallError>;
    /// What it is, for the plan (`HKCU\Environment Path`).
    fn describe(&self) -> String;
}

/// A JSON file standing in for `HKCU\Environment`: `{"Path": {"kind": "REG_EXPAND_SZ",
/// "value": "..."}}` (`{}` or no file: no PATH). For tests and safe dry runs. Any other
/// file (another top level, or a key besides `Path`) is refused rather than read as "no
/// PATH": `--apply` replaces the whole file, so a wrong `--env-file` would be lost.
#[derive(Debug, Clone)]
pub struct FileEnv {
    /// The file.
    pub path: PathBuf,
}

impl UserEnv for FileEnv {
    fn read_path(&self) -> Result<Option<EnvValue>, InstallError> {
        let Some(v) = read_json(&self.path)? else {
            return Ok(None);
        };
        let refuse = |why: String| {
            InstallError::Json {
            path: self.path.clone(),
            message: format!(
                "{why}; an --env-file holds `{{}}` or `{{\"Path\": ...}}` only (--apply replaces it whole)"
            ),
        }
        };
        let Some(obj) = v.as_object() else {
            return Err(refuse("the top level is not an object".to_string()));
        };
        if let Some(other) = obj.keys().find(|k| k.as_str() != "Path") {
            return Err(refuse(format!("it has a key `{other}` besides `Path`")));
        }
        match v.get("Path") {
            None | Some(Value::Null) => Ok(None),
            Some(p) => {
                serde_json::from_value(p.clone())
                    .map(Some)
                    .map_err(|e| InstallError::Json {
                        path: self.path.clone(),
                        message: e.to_string(),
                    })
            }
        }
    }

    fn write_path(&mut self, value: &EnvValue) -> Result<(), InstallError> {
        let text = serde_json::to_string_pretty(&json!({ "Path": value }))
            .map_err(|e| InstallError::Env(e.to_string()))?;
        write_atomic(&self.path, text.as_bytes())
    }

    fn describe(&self) -> String {
        format!(
            "{} (a stand-in for HKCU\\Environment Path)",
            self.path.display()
        )
    }
}

/// The real `HKCU\Environment` `Path`, through PowerShell and the .NET registry API (the
/// value kind and unexpanded `%VAR%` text are kept); after a write, `WM_SETTINGCHANGE` is
/// broadcast so programs started afterwards see it. Windows only.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemEnv;

const PS_READ: &str = "$ErrorActionPreference='Stop'; \
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false; \
$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment'); \
if ($null -eq $k) { 'none'; exit 0 }; \
$v = $k.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames); \
if ($null -eq $v) { 'none'; exit 0 }; \
$k.GetValueKind('Path').ToString(); $v";

const PS_WRITE: &str = "$ErrorActionPreference='Stop'; \
$k = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment'); \
$kind = [Microsoft.Win32.RegistryValueKind]$env:KITKRAFT_INSTALL_KIND; \
$k.SetValue('Path', $env:KITKRAFT_INSTALL_VALUE, $kind); $k.Close(); \
try { Add-Type -Namespace KitKraft -Name Broadcast -MemberDefinition '[DllImport(\"user32.dll\", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, UIntPtr w, string l, uint f, uint t, out UIntPtr r);'; \
$r = [UIntPtr]::Zero; [void][KitKraft.Broadcast]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$r); 'broadcast' } \
catch { 'no-broadcast' }";

fn powershell(script: &str, env: &[(&str, &str)]) -> Result<String, InstallError> {
    if !cfg!(windows) {
        return Err(InstallError::Env(
            "the user PATH is edited on Windows only".to_string(),
        ));
    }
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ]);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| InstallError::Env(format!("could not start powershell.exe: {e}")))?;
    if !out.status.success() {
        return Err(InstallError::Env(format!(
            "powershell.exe failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

/// `PS_READ`'s output: `none`, or the kind on the first line and the value on the next.
fn parse_ps_read(out: &str) -> Result<Option<EnvValue>, InstallError> {
    let mut lines = out.lines();
    let kind = match lines.next().map(str::trim) {
        None | Some("none") => return Ok(None),
        Some("String") => EnvKind::String,
        Some("ExpandString") => EnvKind::ExpandString,
        Some(other) => {
            return Err(InstallError::Env(format!(
                "HKCU\\Environment Path has kind {other}; expected REG_SZ or REG_EXPAND_SZ"
            )))
        }
    };
    let value = lines
        .next()
        .unwrap_or("")
        .trim_end_matches('\r')
        .to_string();
    Ok(Some(EnvValue { kind, value }))
}

impl UserEnv for SystemEnv {
    fn read_path(&self) -> Result<Option<EnvValue>, InstallError> {
        parse_ps_read(&powershell(PS_READ, &[])?)
    }

    fn write_path(&mut self, value: &EnvValue) -> Result<(), InstallError> {
        let kind = match value.kind {
            EnvKind::String => "String",
            EnvKind::ExpandString => "ExpandString",
        };
        powershell(
            PS_WRITE,
            &[
                ("KITKRAFT_INSTALL_KIND", kind),
                ("KITKRAFT_INSTALL_VALUE", value.value.as_str()),
            ],
        )
        .map(|_| ())
    }

    fn describe(&self) -> String {
        "HKCU\\Environment Path (the user PATH)".to_string()
    }
}

/// Install or uninstall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `kit install`.
    Install,
    /// `kit uninstall`.
    Uninstall,
}

/// What to change.
#[derive(Debug, Clone)]
pub struct Options {
    /// The folder holding `kit.exe` (the dist folder).
    pub dist: PathBuf,
    /// The home folder holding `.claude.json`.
    pub home: PathBuf,
    /// The project folder that gets `.mcp.json`, if any.
    pub project: Option<PathBuf>,
    /// Edit the user PATH.
    pub path: bool,
    /// Edit the MCP registrations.
    pub mcp: bool,
    /// MCP `args` when no earlier entry gives them.
    pub default_args: Vec<String>,
    /// The Data dir detection finds: an MCP `env` entry naming it is dropped.
    pub detected_data_dir: Option<PathBuf>,
    /// Suffix of backup file names (unix seconds).
    pub stamp: String,
}

impl Options {
    /// Options for `dist` with this machine's defaults: home `%USERPROFILE%` (else
    /// `$HOME`), the project found above `dist` (a folder with `.mcp.json.example` and
    /// `Cargo.toml`), MCP args preloading the default MO2 instance's `Default` profile
    /// (read-only, as before; [`default_mcp_args`]), the detected Data dir, and the
    /// current time as the stamp. Both defaults honor `KITKRAFT_DETECT=0` and
    /// `KITKRAFT_MO2_DIR` as every command does (`crate::host`).
    pub fn detect(dist: PathBuf) -> Options {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .filter(|h| !h.is_empty())
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        let project = find_project(&dist);
        // The same defaults every command uses: `KITKRAFT_DETECT=0` turns detection off
        // here too (then only `KITKRAFT_MO2_DIR` names an instance).
        let detected = crate::host::detected();
        let mo2 = crate::host::default_mo2();
        let default_args = default_mcp_args(mo2.as_ref().map(|m| m.path.as_path()));
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            .to_string();
        Options {
            dist,
            home,
            project,
            path: true,
            mcp: true,
            default_args,
            detected_data_dir: detected.data_dir.as_ref().map(|f| f.path.clone()),
            stamp,
        }
    }
}

/// MCP `args` for a new entry: `mcp`, and `--profile <mo2>/profiles/Default` (read-only
/// preload, as before) when `mo2` has that profile.
pub fn default_mcp_args(mo2: Option<&Path>) -> Vec<String> {
    let mut args = vec!["mcp".to_string()];
    if let Some(m) = mo2 {
        let profile = m.join("profiles").join("Default");
        if profile.join("modlist.txt").is_file() {
            args.push("--profile".to_string());
            args.push(slashes(&profile));
        }
    }
    args
}

/// The repository above `start` (a folder holding `.mcp.json.example` and `Cargo.toml`).
pub fn find_project(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join(".mcp.json.example").is_file() && d.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
}

/// A path with forward slashes, as Claude Code's files spell them (and as the folder-move
/// script rewrites them).
pub fn slashes(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// One change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Set the user PATH.
    UserPath {
        /// The value before (printed and backed up).
        before: Option<EnvValue>,
        /// The value after.
        after: EnvValue,
        /// Where the old value is saved.
        backup: PathBuf,
    },
    /// Write a file (create or replace).
    Write {
        /// The file.
        path: PathBuf,
        /// Its bytes when the plan was made (`None`: it did not exist).
        before: Option<Vec<u8>>,
        /// The new bytes.
        after: Vec<u8>,
        /// Where the old file is copied first, if anywhere.
        backup: Option<PathBuf>,
    },
    /// Delete a file.
    Delete {
        /// The file.
        path: PathBuf,
        /// Its bytes when the plan was made.
        before: Vec<u8>,
        /// Where it is copied first, if anywhere.
        backup: Option<PathBuf>,
    },
}

/// One part of the plan: what it is about, what would change (lines), and the change
/// (`None`: nothing to do).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// What it is about (`User PATH`, `~/.claude.json`).
    pub what: String,
    /// Human-readable lines.
    pub lines: Vec<String>,
    /// The change, or `None` when it is already as wanted.
    pub change: Option<Change>,
}

/// The plan.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Install or uninstall.
    pub action: Action,
    /// The steps, in apply order.
    pub steps: Vec<Step>,
    /// Notes (things not done, and why).
    pub notes: Vec<String>,
}

impl Plan {
    /// Whether anything would change.
    pub fn changes_anything(&self) -> bool {
        self.steps.iter().any(|s| s.change.is_some())
    }

    /// The plan as text: each step, `unchanged` or what would change, then the notes.
    pub fn render(&self, applied: bool) -> String {
        let verb = match (self.action, applied) {
            (Action::Install, false) => "kit install would change",
            (Action::Install, true) => "kit install changed",
            (Action::Uninstall, false) => "kit uninstall would change",
            (Action::Uninstall, true) => "kit uninstall changed",
        };
        let mut out = format!("{verb}:\n");
        for s in &self.steps {
            let state = if s.change.is_some() {
                ""
            } else {
                "  (unchanged)"
            };
            out.push_str(&format!("- {}{state}\n", s.what));
            for l in &s.lines {
                out.push_str(&format!("    {l}\n"));
            }
        }
        for n in &self.notes {
            out.push_str(&format!("note: {n}\n"));
        }
        if !self.changes_anything() {
            out.push_str("Nothing to change.\n");
        }
        out
    }
}

/// What install recorded about the state before it (`<dist>\install-state.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// The dist folder at install time.
    pub dist: String,
    /// The PATH entry install added (`None`: it was there already).
    pub path_entry: Option<String>,
    /// The user PATH before the first install.
    pub path_before: Option<EnvValue>,
    /// `~/.claude.json` at install time.
    pub claude_json: Option<FileState>,
    /// The project `.mcp.json` at install time.
    pub mcp_json: Option<FileState>,
}

/// One MCP file before install changed it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FileState {
    /// The file.
    pub path: String,
    /// Whether it existed.
    pub existed: bool,
    /// Its `mcpServers.kitkraft` entry before, if any.
    pub kitkraft_before: Option<Value>,
    /// Legacy entries install removed, by name.
    pub removed: Map<String, Value>,
}

fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>, InstallError> {
    match std::fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_err(path)(e)),
    }
}

fn parse_json(path: &Path, bytes: &[u8]) -> Result<Value, InstallError> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(text).map_err(|e| InstallError::Json {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

fn read_json(path: &Path) -> Result<Option<Value>, InstallError> {
    read_bytes(path)?.map(|b| parse_json(path, &b)).transpose()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), InstallError> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    let tmp = path.with_extension(format!("kitkraft-tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(io_err(&tmp))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io_err(path)(e)
    })
}

fn pretty(v: &Value) -> Vec<u8> {
    let mut text = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
    text.push('\n');
    text.into_bytes()
}

/// The MCP servers in `v["mcpServers"]` that are KitKraft's: named `kitkraft` or a legacy
/// name, or running a program named `kit` / `kit.exe`. `(name, entry)`, in file order.
pub fn kit_servers(v: &Value) -> Vec<(String, Value)> {
    let Some(servers) = v.get("mcpServers").and_then(Value::as_object) else {
        return Vec::new();
    };
    servers
        .iter()
        .filter(|(name, entry)| {
            name.eq_ignore_ascii_case(SERVER)
                || LEGACY_SERVERS.iter().any(|l| name.eq_ignore_ascii_case(l))
                || runs_kit(entry)
        })
        .map(|(n, e)| (n.clone(), e.clone()))
        .collect()
}

/// The `command` of an MCP entry.
pub fn command_of(entry: &Value) -> Option<&str> {
    entry.get("command").and_then(Value::as_str)
}

fn runs_kit(entry: &Value) -> bool {
    command_of(entry)
        .and_then(|c| {
            Path::new(&c.replace('\\', "/"))
                .file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
        })
        .is_some_and(|n| n == "kit.exe" || n == "kit")
}

fn points_into(entry: &Value, dist: &Path) -> bool {
    command_of(entry).is_some_and(|c| {
        let p = kitkraft_io::host::native_path(c);
        p.parent().is_some_and(|d| same_path(d, dist))
    })
}

/// The `kitkraft` entry for `opts`, carrying `args`/`env` from `previous`.
fn server_entry(opts: &Options, previous: Option<&Value>) -> (Value, Vec<String>) {
    let mut notes = Vec::new();
    let args = previous
        .and_then(|p| p.get("args"))
        .filter(|a| a.is_array())
        .cloned()
        .unwrap_or_else(|| json!(opts.default_args));
    let mut env = previous
        .and_then(|p| p.get("env"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(detected) = &opts.detected_data_dir {
        let drop: Vec<String> = env
            .iter()
            .filter(|(k, v)| {
                k.ends_with("_DATA_DIR")
                    && v.as_str()
                        .is_some_and(|d| same_path(&kitkraft_io::host::native_path(d), detected))
            })
            .map(|(k, _)| k.clone())
            .collect();
        for k in drop {
            env.remove(&k);
            notes.push(format!(
                "env {k} dropped: it names the Data dir `kit doctor` detects anyway"
            ));
        }
    }
    let entry = json!({
        "type": "stdio",
        "command": slashes(&opts.dist.join(kit_exe())),
        "args": args,
        "env": Value::Object(env),
    });
    (entry, notes)
}

fn kit_exe() -> String {
    format!("kit{}", std::env::consts::EXE_SUFFIX)
}

/// Plans the change of one MCP file. Returns the step and what to record about it.
fn plan_mcp_file(
    opts: &Options,
    action: Action,
    what: &str,
    path: &Path,
    recorded: Option<&FileState>,
    fallback: Option<&Value>,
) -> Result<(Step, Option<FileState>, Option<Value>), InstallError> {
    let before = read_bytes(path)?;
    let mut doc = match &before {
        Some(b) => parse_json(path, b)?,
        None => json!({}),
    };
    if !doc.is_object() {
        return Err(InstallError::Json {
            path: path.to_path_buf(),
            message: "the top level is not an object".to_string(),
        });
    }
    let original = doc.clone();
    let mut lines = Vec::new();
    let mut record: Option<FileState> = None;
    let mut set_entry: Option<Value> = None;
    match action {
        Action::Install => {
            let servers = doc
                .as_object_mut()
                .map(|o| o.entry("mcpServers").or_insert_with(|| json!({})));
            let Some(servers) = servers.and_then(Value::as_object_mut) else {
                return Err(InstallError::Json {
                    path: path.to_path_buf(),
                    message: "`mcpServers` is not an object".to_string(),
                });
            };
            let current = servers.get(SERVER).cloned();
            let mut removed = Map::new();
            for legacy in LEGACY_SERVERS {
                let name = servers
                    .keys()
                    .find(|k| k.eq_ignore_ascii_case(legacy))
                    .cloned();
                if let Some(name) = name {
                    if servers.get(&name).is_some_and(runs_kit) {
                        if let Some(old) = servers.remove(&name) {
                            lines.push(format!(
                                "remove the pre-rename entry `{name}` (command {})",
                                command_of(&old).unwrap_or("?")
                            ));
                            removed.insert(name, old);
                        }
                    }
                }
            }
            // The earlier entry here, else a removed pre-rename one, else the entry the
            // user config got (so both files register the same server).
            let previous = current
                .as_ref()
                .or_else(|| removed.values().next())
                .or(fallback);
            let (entry, notes) = server_entry(opts, previous);
            lines.extend(notes);
            if current.as_ref() != Some(&entry) {
                lines.push(format!(
                    "set mcpServers.{SERVER}: command {}, args {}",
                    command_of(&entry).unwrap_or("?"),
                    entry.get("args").map_or_else(String::new, Value::to_string)
                ));
                if let Some(c) = &current {
                    lines.push(format!("  (was: command {})", command_of(c).unwrap_or("?")));
                }
            }
            set_entry = Some(entry.clone());
            servers.insert(SERVER.to_string(), entry);
            let mut state = recorded.cloned().unwrap_or_else(|| FileState {
                path: path.display().to_string(),
                existed: before.is_some(),
                kitkraft_before: current.clone(),
                removed: Map::new(),
            });
            for (k, v) in removed {
                state.removed.entry(k).or_insert(v);
            }
            record = Some(state);
        }
        Action::Uninstall => {
            if let Some(servers) = doc.get_mut("mcpServers").and_then(Value::as_object_mut) {
                if servers
                    .get(SERVER)
                    .is_some_and(|e| points_into(e, &opts.dist))
                {
                    servers.remove(SERVER);
                    lines.push(format!(
                        "remove mcpServers.{SERVER} (points into the dist folder)"
                    ));
                }
                if let Some(r) = recorded {
                    if let Some(prev) = &r.kitkraft_before {
                        if !servers.contains_key(SERVER) {
                            servers.insert(SERVER.to_string(), prev.clone());
                            lines.push(format!(
                                "restore the earlier mcpServers.{SERVER} (command {})",
                                command_of(prev).unwrap_or("?")
                            ));
                        }
                    }
                    for (name, entry) in &r.removed {
                        if !servers.contains_key(name) {
                            servers.insert(name.clone(), entry.clone());
                            lines.push(format!(
                                "restore mcpServers.{name} (command {})",
                                command_of(entry).unwrap_or("?")
                            ));
                        }
                    }
                }
            }
            let created = recorded.is_some_and(|r| !r.existed);
            let only_empty_servers = doc.as_object().is_some_and(|o| {
                o.len() == 1
                    && o.get("mcpServers")
                        .and_then(Value::as_object)
                        .is_some_and(Map::is_empty)
            });
            if created && only_empty_servers {
                if let Some(b) = before {
                    lines.push(format!("delete {} (install created it)", path.display()));
                    return Ok((
                        Step {
                            what: what.to_string(),
                            lines,
                            change: Some(Change::Delete {
                                path: path.to_path_buf(),
                                before: b,
                                backup: None,
                            }),
                        },
                        None,
                        None,
                    ));
                }
            }
        }
    }
    // Install creates a missing file; uninstall leaves a missing one missing.
    let create = before.is_none() && action == Action::Install;
    let change = (doc != original || create).then(|| {
        let backup = before.as_ref().map(|_| backup_path(path, &opts.stamp));
        if let Some(b) = &backup {
            lines.push(format!("backup copy first: {}", b.display()));
        }
        Change::Write {
            path: path.to_path_buf(),
            before: before.clone(),
            after: pretty(&doc),
            backup,
        }
    });
    if change.is_none() {
        lines.push(if before.is_none() {
            format!("{} does not exist; nothing to do", path.display())
        } else {
            format!("{} already as wanted", path.display())
        });
    }
    Ok((
        Step {
            what: what.to_string(),
            lines,
            change,
        },
        record,
        set_entry,
    ))
}

fn backup_path(path: &Path, stamp: &str) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!("{name}.kitkraft-backup-{stamp}"))
}

fn path_entries(value: &str) -> Vec<String> {
    value
        .split(';')
        .filter(|e| !e.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn entry_is(entry: &str, dir: &Path) -> bool {
    let e = entry.trim().trim_matches('"');
    same_path(Path::new(e), dir)
}

/// Plans `action` for `opts` against `env`.
pub fn plan(opts: &Options, env: &dyn UserEnv, action: Action) -> Result<Plan, InstallError> {
    if action == Action::Install && !opts.dist.join(kit_exe()).is_file() {
        return Err(InstallError::Invalid(format!(
            "{} has no {}; run `cargo xtask dist` and install from its KitKraft folder",
            opts.dist.display(),
            kit_exe()
        )));
    }
    let state_path = opts.dist.join(STATE_FILE);
    let recorded: Option<State> = read_json(&state_path)?
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| InstallError::Json {
            path: state_path.clone(),
            message: e.to_string(),
        })?;
    let mut steps = Vec::new();
    let mut notes = Vec::new();
    let mut state = recorded.clone().unwrap_or_default();
    state.dist = opts.dist.display().to_string();
    let dist_text = opts.dist.display().to_string();
    let target_dir = opts
        .dist
        .components()
        .any(|c| matches!(c.as_os_str().to_str(), Some("target" | ".targets")));
    if target_dir && action == Action::Install {
        notes.push(format!(
            "{dist_text} looks like a cargo target folder; install from the folder `cargo xtask dist` writes"
        ));
    }

    // 1. The user PATH.
    if opts.path {
        let before = env.read_path()?;
        let old_text = before.as_ref().map_or("", |b| b.value.as_str());
        let mut entries = path_entries(old_text);
        let stale = recorded
            .as_ref()
            .and_then(|r| r.path_entry.clone())
            .filter(|e| !entry_is(e, &opts.dist));
        let mut lines = Vec::new();
        let mut changed = false;
        if let Some(stale) = &stale {
            let n = entries.len();
            entries.retain(|e| !entry_is(e, Path::new(stale.trim())));
            if entries.len() != n {
                lines.push(format!("remove the earlier dist folder {stale}"));
                changed = true;
            }
        }
        match action {
            Action::Install => {
                if entries.iter().any(|e| entry_is(e, &opts.dist)) {
                    lines.push(format!("{dist_text} is on it already"));
                } else {
                    entries.push(dist_text.clone());
                    lines.push(format!("add {dist_text}"));
                    changed = true;
                    state.path_entry = Some(dist_text.clone());
                }
                if changed && state.path_before.is_none() {
                    state.path_before = before.clone();
                }
            }
            Action::Uninstall => {
                let n = entries.len();
                entries.retain(|e| !entry_is(e, &opts.dist));
                if entries.len() != n {
                    lines.push(format!("remove {dist_text}"));
                    changed = true;
                } else {
                    lines.push(format!("{dist_text} is not on it"));
                }
            }
        }
        let change = changed.then(|| {
            let after = EnvValue {
                kind: before.as_ref().map_or(EnvKind::ExpandString, |b| b.kind),
                value: entries.join(";"),
            };
            let backup = opts
                .dist
                .join("backups")
                .join(format!("user-path-{}.txt", opts.stamp));
            lines.push(format!(
                "old value ({}): {}",
                before.as_ref().map_or("absent", |b| b.kind.label()),
                if old_text.is_empty() {
                    "(empty)"
                } else {
                    old_text
                }
            ));
            lines.push(format!("old value saved to {}", backup.display()));
            Change::UserPath {
                before: before.clone(),
                after,
                backup,
            }
        });
        steps.push(Step {
            what: format!("User PATH: {}", env.describe()),
            lines,
            change,
        });
    } else {
        notes.push("the user PATH was not looked at (--no-path)".to_string());
    }

    // 2. and 3. The MCP registrations.
    if opts.mcp {
        let claude = opts.home.join(CLAUDE_JSON);
        let (step, rec, user_entry) = plan_mcp_file(
            opts,
            action,
            &format!("Claude Code user config: {}", claude.display()),
            &claude,
            recorded.as_ref().and_then(|r| r.claude_json.as_ref()),
            None,
        )?;
        steps.push(step);
        if action == Action::Install {
            state.claude_json = rec;
        }
        match &opts.project {
            Some(project) => {
                let mcp = project.join(MCP_JSON);
                let (step, rec, _) = plan_mcp_file(
                    opts,
                    action,
                    &format!("Project MCP file: {}", mcp.display()),
                    &mcp,
                    recorded.as_ref().and_then(|r| r.mcp_json.as_ref()),
                    user_entry.as_ref(),
                )?;
                steps.push(step);
                if action == Action::Install {
                    state.mcp_json = rec;
                }
            }
            None => notes.push(
                "no project folder found above the dist folder (one holding .mcp.json.example); .mcp.json not written (pass --project)"
                    .to_string(),
            ),
        }
    } else {
        notes.push("the MCP registrations were not looked at (--no-mcp)".to_string());
    }

    // 4. The state file.
    let state_before = read_bytes(&state_path)?;
    match action {
        Action::Install => {
            let any = steps.iter().any(|s| s.change.is_some());
            if any || state_before.is_none() {
                let bytes = pretty(
                    &serde_json::to_value(&state)
                        .map_err(|e| InstallError::Invalid(format!("state: {e}")))?,
                );
                let change =
                    (state_before.as_deref() != Some(bytes.as_slice())).then(|| Change::Write {
                        path: state_path.clone(),
                        before: state_before.clone(),
                        after: bytes,
                        backup: None,
                    });
                steps.push(Step {
                    what: format!("Install record: {}", state_path.display()),
                    lines: vec!["what was there before, for kit uninstall".to_string()],
                    change,
                });
            }
        }
        Action::Uninstall => {
            if let Some(b) = state_before {
                steps.push(Step {
                    what: format!("Install record: {}", state_path.display()),
                    lines: vec!["delete it".to_string()],
                    change: Some(Change::Delete {
                        path: state_path.clone(),
                        before: b,
                        backup: None,
                    }),
                });
            } else {
                notes.push(format!(
                    "no {STATE_FILE} in the dist folder: only entries pointing into it are removed"
                ));
            }
        }
    }
    Ok(Plan {
        action,
        steps,
        notes,
    })
}

/// Image names Claude Code runs as on Windows, as `tasklist` showed them on the dev box
/// (2026-10-15): the desktop app (`...\WindowsApps\Claude_<version>_x64__...\app\claude.exe`)
/// and the native CLI (`%APPDATA%\Claude\claude-code\<version>\...\claude.exe`) are both
/// `claude.exe` (ASCII case ignored). A CLI installed through npm runs as `node.exe` and
/// cannot be told apart by name, so it is not detected.
pub const CLAUDE_PROCESSES: &[&str] = &["claude.exe"];

/// The image names in `tasklist /fo csv /nh` output (the first field of each row).
pub fn parse_tasklist(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|line| {
            let first = line.trim().strip_prefix('"')?;
            let (name, _) = first.split_once('"')?;
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

/// This machine's running image names, through `tasklist` (read-only; nothing is
/// stopped). Windows only: elsewhere the list is empty.
pub fn system_processes() -> Result<Vec<String>, InstallError> {
    if !cfg!(windows) {
        return Ok(Vec::new());
    }
    let out = std::process::Command::new("tasklist")
        .args(["/fo", "csv", "/nh"])
        .output()
        .map_err(|e| {
            InstallError::Env(format!(
                "could not start tasklist to see whether Claude Code is running: {e}"
            ))
        })?;
    if !out.status.success() {
        return Err(InstallError::Env(format!(
            "tasklist failed ({}); close Claude Code and pass --force-while-running",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(parse_tasklist(&String::from_utf8_lossy(&out.stdout)))
}

/// The one-line refusal when `processes` include Claude Code, else `None`.
pub fn claude_running(processes: &[String]) -> Option<String> {
    let mut names: Vec<&str> = Vec::new();
    let mut count: usize = 0;
    for p in processes {
        if let Some(n) = CLAUDE_PROCESSES
            .iter()
            .find(|n| p.trim().eq_ignore_ascii_case(n))
        {
            count = count.saturating_add(1);
            if !names.contains(n) {
                names.push(n);
            }
        }
    }
    (count > 0).then(|| {
        format!(
            "Claude Code is running ({count} {} process(es)): it rewrites .claude.json from memory and would drop the {SERVER} entry; close the Claude app and every claude session, then run this again (--force-while-running skips this check)",
            names.join("/")
        )
    })
}

/// Whether `plan` writes or deletes the `.claude.json` in `opts.home`.
pub fn touches_claude_json(plan: &Plan, opts: &Options) -> bool {
    let claude = opts.home.join(CLAUDE_JSON);
    plan.steps.iter().any(|s| match &s.change {
        Some(Change::Write { path, .. } | Change::Delete { path, .. }) => same_path(path, &claude),
        _ => false,
    })
}

/// Stops `--apply` before anything changes when the plan rewrites `.claude.json` while
/// Claude Code runs (`processes`: the running image names, [`system_processes`]): it keeps
/// that file in memory and writes it back, dropping the new entry. Nothing is stopped.
pub fn refuse_while_running(
    plan: &Plan,
    opts: &Options,
    processes: &[String],
) -> Result<(), InstallError> {
    if !touches_claude_json(plan, opts) {
        return Ok(());
    }
    match claude_running(processes) {
        Some(message) => Err(InstallError::ClaudeRunning(message)),
        None => Ok(()),
    }
}

/// Applies `plan`: backups first, then each change in order. A file that changed since
/// the plan stops it ([`InstallError::Changed`]); the steps before it stay applied (each
/// is printed). Returns one line per change made.
pub fn apply(plan: &Plan, env: &mut dyn UserEnv) -> Result<Vec<String>, InstallError> {
    let mut done = Vec::new();
    for step in &plan.steps {
        let Some(change) = &step.change else {
            continue;
        };
        match change {
            Change::UserPath {
                before,
                after,
                backup,
            } => {
                if env.read_path()? != *before {
                    return Err(InstallError::Changed(PathBuf::from(env.describe())));
                }
                let old = before.as_ref().map_or_else(String::new, |b| {
                    format!("{}\r\n{}\r\n", b.kind.label(), b.value)
                });
                write_atomic(backup, old.as_bytes())?;
                env.write_path(after)?;
                done.push(format!(
                    "{}: done (old value in {})",
                    step.what,
                    backup.display()
                ));
            }
            Change::Write {
                path,
                before,
                after,
                backup,
            } => {
                if read_bytes(path)? != *before {
                    return Err(InstallError::Changed(path.clone()));
                }
                if let (Some(b), Some(old)) = (backup, before) {
                    std::fs::write(b, old).map_err(io_err(b))?;
                }
                write_atomic(path, after)?;
                done.push(format!("{}: written", step.what));
            }
            Change::Delete {
                path,
                before,
                backup,
            } => {
                if read_bytes(path)?.as_ref() != Some(before) {
                    return Err(InstallError::Changed(path.clone()));
                }
                if let Some(b) = backup {
                    std::fs::write(b, before).map_err(io_err(b))?;
                }
                std::fs::remove_file(path).map_err(io_err(path))?;
                done.push(format!("{}: deleted", step.what));
            }
        }
    }
    Ok(done)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
        opts: Options,
        env: FileEnv,
    }

    fn fixture(tag: &str) -> Fixture {
        let root =
            std::env::temp_dir().join(format!("kitkraft-install-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dist = root.join("repo with space").join("dist").join("KitKraft");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join(kit_exe()), b"MZ").unwrap();
        let project = root.join("repo with space");
        std::fs::write(project.join(".mcp.json.example"), b"{}").unwrap();
        std::fs::write(project.join("Cargo.toml"), b"").unwrap();
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let env = FileEnv {
            path: root.join("hkcu-environment.json"),
        };
        let opts = Options {
            dist,
            home,
            project: find_project(&root.join("repo with space").join("dist").join("KitKraft")),
            path: true,
            mcp: true,
            default_args: vec!["mcp".into()],
            detected_data_dir: Some(PathBuf::from(r"C:\Game\Data")),
            stamp: "1".into(),
        };
        Fixture { root, opts, env }
    }

    fn claude(f: &Fixture) -> Value {
        read_json(&f.opts.home.join(CLAUDE_JSON)).unwrap().unwrap()
    }

    #[test]
    fn install_is_idempotent_and_uninstall_reverses_it() {
        let mut f = fixture("cycle");
        let user_path = r"%USERPROFILE%\bin;C:\Windows";
        f.env
            .write_path(&EnvValue {
                kind: EnvKind::ExpandString,
                value: user_path.into(),
            })
            .unwrap();
        let original_claude = json!({
            "numStartups": 3,
            "projects": {"C:/x": {"mcpServers": {}}},
            "mcpServers": {
                "other": {"type": "stdio", "command": "C:/other.exe", "args": []},
                "kitcraft": {
                    "type": "stdio",
                    "command": "C:/Modding/kitcraft/bin/kit.exe",
                    "args": ["mcp", "--profile", "C:/MO2/profiles/Default"],
                    "env": {"KITCRAFT_DATA_DIR": "C:/Game/Data", "OTHER": "1"}
                }
            }
        });
        std::fs::write(
            f.opts.home.join(CLAUDE_JSON),
            serde_json::to_vec(&original_claude).unwrap(),
        )
        .unwrap();
        assert_eq!(
            f.opts.project.as_deref(),
            Some(f.root.join("repo with space").as_path())
        );

        let p = plan(&f.opts, &f.env, Action::Install).unwrap();
        let text = p.render(false);
        assert!(text.contains("add "), "{text}");
        assert!(text.contains(user_path), "the old PATH is printed: {text}");
        assert!(
            text.contains("remove the pre-rename entry `kitcraft`"),
            "{text}"
        );
        assert!(text.contains("KITCRAFT_DATA_DIR dropped"), "{text}");
        // A dry run changes nothing.
        assert_eq!(f.env.read_path().unwrap().unwrap().value, user_path);
        assert!(!f.opts.project.as_ref().unwrap().join(MCP_JSON).exists());
        let done = apply(&p, &mut f.env).unwrap();
        assert_eq!(done.len(), 4, "{done:?}");

        let path = f.env.read_path().unwrap().unwrap();
        assert_eq!(path.kind, EnvKind::ExpandString);
        assert_eq!(path.value, format!("{user_path};{}", f.opts.dist.display()));
        let c = claude(&f);
        assert!(c["mcpServers"].get("kitcraft").is_none());
        assert_eq!(c["mcpServers"]["other"]["command"], "C:/other.exe");
        assert_eq!(c["numStartups"], 3);
        let entry = &c["mcpServers"][SERVER];
        assert_eq!(entry["command"], slashes(&f.opts.dist.join(kit_exe())));
        assert_eq!(
            entry["args"],
            json!(["mcp", "--profile", "C:/MO2/profiles/Default"])
        );
        assert_eq!(entry["env"], json!({"OTHER": "1"}));
        let mcp = read_json(&f.opts.project.as_ref().unwrap().join(MCP_JSON))
            .unwrap()
            .unwrap();
        assert_eq!(mcp["mcpServers"][SERVER], *entry);
        assert!(f.opts.home.join(".claude.json.kitkraft-backup-1").is_file());
        assert!(f
            .opts
            .dist
            .join("backups")
            .join("user-path-1.txt")
            .is_file());

        // Again: nothing to change.
        let again = plan(&f.opts, &f.env, Action::Install).unwrap();
        assert!(!again.changes_anything(), "{}", again.render(false));
        assert!(again.render(false).contains("Nothing to change"));

        // Uninstall restores the PATH, the legacy entry, and removes what install made.
        let u = plan(&f.opts, &f.env, Action::Uninstall).unwrap();
        apply(&u, &mut f.env).unwrap();
        assert_eq!(f.env.read_path().unwrap().unwrap().value, user_path);
        let c = claude(&f);
        assert!(c["mcpServers"].get(SERVER).is_none());
        assert_eq!(
            c["mcpServers"]["kitcraft"],
            original_claude["mcpServers"]["kitcraft"]
        );
        assert_eq!(c, original_claude);
        assert!(!f.opts.project.as_ref().unwrap().join(MCP_JSON).exists());
        assert!(!f.opts.dist.join(STATE_FILE).exists());
        let again = plan(&f.opts, &f.env, Action::Uninstall).unwrap();
        assert!(!again.changes_anything(), "{}", again.render(false));
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn a_fresh_home_and_no_path_get_created() {
        let mut f = fixture("fresh");
        let p = plan(&f.opts, &f.env, Action::Install).unwrap();
        apply(&p, &mut f.env).unwrap();
        let path = f.env.read_path().unwrap().unwrap();
        assert_eq!(path.value, f.opts.dist.display().to_string());
        let c = claude(&f);
        assert_eq!(c["mcpServers"][SERVER]["args"], json!(["mcp"]));
        assert_eq!(c["mcpServers"][SERVER]["env"], json!({}));
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn a_moved_dist_folder_replaces_the_old_path_entry() {
        let mut f = fixture("moved");
        let p = plan(&f.opts, &f.env, Action::Install).unwrap();
        apply(&p, &mut f.env).unwrap();
        // The repository moves: the state file moves with it, the PATH still names the
        // old place.
        let old = f.opts.dist.clone();
        let new_repo = f.root.join("moved repo");
        std::fs::rename(f.root.join("repo with space"), &new_repo).unwrap();
        f.opts.dist = new_repo.join("dist").join("KitKraft");
        f.opts.project = find_project(&f.opts.dist);
        let p = plan(&f.opts, &f.env, Action::Install).unwrap();
        let text = p.render(false);
        assert!(text.contains("remove the earlier dist folder"), "{text}");
        apply(&p, &mut f.env).unwrap();
        let value = f.env.read_path().unwrap().unwrap().value;
        assert_eq!(value, f.opts.dist.display().to_string());
        assert!(!value.contains(&old.display().to_string()));
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn a_file_changed_after_the_plan_stops_apply() {
        let mut f = fixture("race");
        std::fs::write(f.opts.home.join(CLAUDE_JSON), b"{}").unwrap();
        let p = plan(&f.opts, &f.env, Action::Install).unwrap();
        std::fs::write(f.opts.home.join(CLAUDE_JSON), b"{\"x\":1}").unwrap();
        let err = apply(&p, &mut f.env).unwrap_err();
        assert!(matches!(err, InstallError::Changed(_)), "{err}");
        // Invalid JSON is refused at plan time and left alone.
        std::fs::write(f.opts.home.join(CLAUDE_JSON), b"{not json").unwrap();
        let err = plan(&f.opts, &f.env, Action::Install).unwrap_err();
        assert!(err.to_string().contains("not valid JSON"), "{err}");
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn no_kit_exe_in_dist_is_refused() {
        let f = fixture("nokit");
        std::fs::remove_file(f.opts.dist.join(kit_exe())).unwrap();
        let err = plan(&f.opts, &f.env, Action::Install).unwrap_err();
        assert!(err.to_string().contains("cargo xtask dist"), "{err}");
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn a_running_claude_code_stops_a_plan_that_rewrites_claude_json() {
        let mut f = fixture("running");
        let listed = parse_tasklist(
            "\"System Idle Process\",\"0\",\"Services\",\"0\",\"8 K\"\r\n\
             \"Claude.exe\",\"4120\",\"Console\",\"1\",\"123,456 K\"\r\n\
             \"claude.exe\",\"9000\",\"Console\",\"1\",\"80,000 K\"\r\n",
        );
        assert_eq!(listed, ["System Idle Process", "Claude.exe", "claude.exe"]);
        let quiet: Vec<String> = vec!["explorer.exe".into(), "claude-helper.exe".into()];
        assert_eq!(claude_running(&quiet), None);
        let message = claude_running(&listed).unwrap();
        assert!(message.contains("2 claude.exe process(es)"), "{message}");
        assert!(message.contains("--force-while-running"), "{message}");
        assert!(!message.contains('\n'), "one line: {message}");

        // The plan rewrites .claude.json: refused, and nothing was written.
        let p = plan(&f.opts, &f.env, Action::Install).unwrap();
        assert!(touches_claude_json(&p, &f.opts));
        let err = refuse_while_running(&p, &f.opts, &listed).unwrap_err();
        assert!(matches!(err, InstallError::ClaudeRunning(_)), "{err}");
        assert_eq!(err.to_string(), message);
        assert!(!f.opts.home.join(CLAUDE_JSON).exists());
        refuse_while_running(&p, &f.opts, &quiet).unwrap();

        // Once installed, a plan that leaves .claude.json alone is not stopped.
        apply(&p, &mut f.env).unwrap();
        let again = plan(&f.opts, &f.env, Action::Install).unwrap();
        assert!(!touches_claude_json(&again, &f.opts));
        refuse_while_running(&again, &f.opts, &listed).unwrap();
        // Uninstall rewrites it again: stopped.
        let u = plan(&f.opts, &f.env, Action::Uninstall).unwrap();
        assert!(refuse_while_running(&u, &f.opts, &listed).is_err());
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn default_mcp_args_preload_default_only_when_an_instance_has_it() {
        let f = fixture("args");
        assert_eq!(default_mcp_args(None), ["mcp"]);
        let mo2 = f.root.join("MO2");
        assert_eq!(default_mcp_args(Some(&mo2)), ["mcp"]);
        let profile = mo2.join("profiles").join("Default");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(profile.join("modlist.txt"), b"").unwrap();
        assert_eq!(
            default_mcp_args(Some(&mo2)),
            ["mcp".to_string(), "--profile".into(), slashes(&profile)]
        );
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn an_env_file_other_than_a_path_stand_in_is_refused() {
        let mut f = fixture("envfile");
        // No file, an empty file and `{}` read as no PATH.
        assert_eq!(f.env.read_path().unwrap(), None);
        std::fs::write(&f.env.path, b"").unwrap();
        assert_eq!(f.env.read_path().unwrap(), None);
        std::fs::write(&f.env.path, b"{}").unwrap();
        assert_eq!(f.env.read_path().unwrap(), None);
        // Any other JSON is refused at plan time and left as it was.
        let others: [&[u8]; 4] = [
            br#"{"Other": 1}"#,
            br#"{"Path": {"kind": "REG_SZ", "value": "C:\\x"}, "keep": true}"#,
            b"[]",
            br#""Path""#,
        ];
        for text in others {
            std::fs::write(&f.env.path, text).unwrap();
            let err = plan(&f.opts, &f.env, Action::Install).unwrap_err();
            assert!(err.to_string().contains("--env-file"), "{err}");
            assert_eq!(std::fs::read(&f.env.path).unwrap(), text);
        }
        // The stand-in proper still reads and writes.
        f.env
            .write_path(&EnvValue {
                kind: EnvKind::String,
                value: r"C:\x".into(),
            })
            .unwrap();
        assert_eq!(f.env.read_path().unwrap().unwrap().value, r"C:\x");
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn powershell_read_output_parses() {
        assert_eq!(parse_ps_read("none\r\n").unwrap(), None);
        assert_eq!(
            parse_ps_read("ExpandString\r\n%A%;C:\\x\r\n").unwrap(),
            Some(EnvValue {
                kind: EnvKind::ExpandString,
                value: "%A%;C:\\x".into()
            })
        );
        assert!(parse_ps_read("Binary\r\nx").is_err());
        let servers = kit_servers(&json!({"mcpServers": {
            "a": {"command": "D:\\x\\kit.exe"}, "b": {"command": "other.exe"}, "kitkraft": {}
        }}));
        let names: Vec<&str> = servers.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["a", "kitkraft"]);
    }
}
