//! `kit walk`: the owner walk recorder's front-end setup (`docs/bench/owner-walk/README.md`).
//!
//! Not a catalog command: a walk waits for the owner's Enter at every station, and
//! `execute` never blocks on input (AGENTS.md, "Everything is a command"). So, like `kit
//! mcp` and `kit install`, it is a plain CLI subcommand, which also keeps it off MCP: no
//! agent can drive the owner's game through it. What it reads goes through the catalog
//! (`load` for the load order and runtime slots, `inspect` for each EditorID); the work is
//! `kitkraft_game::walk`.
//!
//! It reads the MO2 profile (refusing `Default` by name), the game profile's console URL
//! and Papyrus log path, the walk plugin and the plugins whose hashes the entry records. It
//! never launches or ends a program, never writes the profile, the game folder or a save,
//! and writes only its own folder under `observe`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kitkraft_game::walk::{WalkFile, WalkSetup};
use kitkraft_game::GameProfile;
use kitkraft_io::ini::Ini;
use kitkraft_io::Instance;
use serde_json::{json, Value};

/// What `kit walk` was given.
#[derive(Debug, Clone, Default)]
pub struct WalkOptions {
    /// The walk file (`docs/bench/owner-walk/stations.toml`).
    pub stations: PathBuf,
    /// The MO2 profile: a name of the detected (or `mo2`) instance, or its folder.
    pub profile: String,
    /// The MO2 instance root.
    pub mo2: Option<PathBuf>,
    /// The folder the walk's own folder is made in.
    pub observe: PathBuf,
    /// The walk plugin's runtime slot (two hex digits), instead of the inferred one.
    pub slot: Option<String>,
    /// The Papyrus log, instead of the game profile's.
    pub papyrus_log: Option<PathBuf>,
    /// The console URL, instead of the game profile's.
    pub url: Option<String>,
    /// The built walk plugin to read EditorIDs from, instead of the load order's copy
    /// (with `slot`, a dry run before the plugin is deployed).
    pub plugin: Option<PathBuf>,
}

/// A walk ready to run.
#[derive(Debug, Clone)]
pub struct PreparedWalk {
    /// The stations.
    pub file: WalkFile,
    /// Ids, header, paths and timings.
    pub setup: WalkSetup,
    /// The console endpoint.
    pub url: String,
    /// Things the owner should know before starting (a plugin not enabled, an inferred slot).
    pub notes: Vec<String>,
}

fn documents() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|h| PathBuf::from(h).join("Documents"))
}

fn read_ini(path: &Path) -> Option<Ini> {
    let bytes = std::fs::read(path).ok()?;
    Some(Ini::parse(&String::from_utf8_lossy(&bytes)))
}

/// `(yyyy-mm-dd, hhmmss)` in UTC now.
fn now_utc() -> (String, String) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let date = kitkraft_game::controller::civil(secs).map_or_else(
        || "1970-01-01".into(),
        |(y, m, d)| format!("{y:04}-{m:02}-{d:02}"),
    );
    let s = secs.checked_rem(86_400).unwrap_or(0);
    let (h, rest) = (
        s.checked_div(3600).unwrap_or(0),
        s.checked_rem(3600).unwrap_or(0),
    );
    let (m, sec) = (
        rest.checked_div(60).unwrap_or(0),
        rest.checked_rem(60).unwrap_or(0),
    );
    (date, format!("{h:02}{m:02}{sec:02}"))
}

/// The runtime ID of object `object` (six hex digits) of a plugin in `slot` (`0B`,
/// `FD:01`, `FE:00A`).
pub fn runtime_id(slot: &str, object: &str) -> Result<String, String> {
    let object = object.trim().to_ascii_uppercase();
    if object.len() != 6 || !object.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("object ID `{object}` is not six hex digits"));
    }
    let slot = slot.trim().to_ascii_uppercase();
    let hex = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit());
    match slot.split_once(':') {
        None if slot.len() == 2 && hex(&slot) => Ok(format!("{slot}{object}")),
        Some(("FD", i)) if i.len() == 2 && hex(i) => {
            Ok(format!("FD{i}{}", object.get(2..).unwrap_or_default()))
        }
        Some(("FE", i)) if i.len() == 3 && hex(i) => {
            Ok(format!("FE{i}{}", object.get(3..).unwrap_or_default()))
        }
        _ => Err(format!(
            "runtime slot `{slot}` (expected two hex digits, FD:nn or FE:nnn)"
        )),
    }
}

fn run(id: &str, args: Value) -> Result<Value, String> {
    crate::execute(id, args).map_err(|e| format!("`{id}`: {e}"))
}

/// Reads everything a walk needs; sends nothing and writes nothing.
pub fn prepare(o: &WalkOptions) -> Result<PreparedWalk, String> {
    if o.profile.trim().eq_ignore_ascii_case("default") {
        return Err("refused [live-profile]: the profile `Default` is the owner's live game; a walk runs on `Kitkraft Dev` only".into());
    }
    let text = std::fs::read(&o.stations).map_err(|e| format!("{}: {e}", o.stations.display()))?;
    let file = WalkFile::parse(&String::from_utf8_lossy(&text))
        .map_err(|e| format!("{}: {e}", o.stations.display()))?;
    let stations_sha = crate::deploy::sha256::hex(&text);
    let detected = crate::host::default_mo2();
    let mo2 = o
        .mo2
        .as_deref()
        .or(detected.as_ref().map(|f| f.path.as_path()));
    let profile_dir = crate::host::profile_dir_in(&o.profile, mo2);
    let folder = profile_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if folder.eq_ignore_ascii_case("default") {
        return Err(format!(
            "refused [live-profile]: {} is the owner's live game",
            profile_dir.display()
        ));
    }
    if !profile_dir.is_dir() {
        return Err(format!(
            "profile `{}` not found ({}); pass --mo2 <instance> or the profile folder",
            o.profile,
            profile_dir.display()
        ));
    }
    let instance = profile_dir
        .parent()
        .and_then(Path::parent)
        .and_then(|root| Instance::read(root).ok());
    let game = GameProfile::named("starfield").map_err(|e| e.to_string())?;
    let mut notes = Vec::new();

    // The load order: slots and paths (the catalog's `load`).
    let loaded = run(
        "load",
        json!({"profile": profile_dir.display().to_string()}),
    )?;
    let plugins = loaded
        .pointer("/data/plugins")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let find = |name: &str| {
        plugins.iter().find(|p| {
            p.get("name")
                .and_then(Value::as_str)
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
                && p.get("active").and_then(Value::as_bool) != Some(false)
        })
    };
    let walk_plugin = find(&file.plugin);
    let slot = match (&o.slot, walk_plugin) {
        (Some(s), _) => {
            notes.push(format!("runtime slot {s} given with --slot"));
            s.clone()
        }
        (None, Some(p)) => {
            let s = p
                .get("runtime_slot")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{} has no runtime slot in this load order", file.plugin))?
                .to_string();
            notes.push(format!(
                "runtime slot {s} of {} is {} (`runtime-slots-by-scale`); station W-00 checks it in game",
                file.plugin,
                p.get("runtime_basis").and_then(Value::as_str).unwrap_or("inferred")
            ));
            s
        }
        (None, None) if o.plugin.is_some() => {
            return Err(format!(
                "{} is not enabled, so its slot is unknown: pass --slot too",
                file.plugin
            ))
        }
        (None, None) => {
            return Err(format!(
                "{} is not an enabled plugin of {}: deploy and enable it first (README, \"Before you start\")",
                file.plugin,
                profile_dir.display()
            ))
        }
    };
    let plugin_path = o
        .plugin
        .clone()
        .or_else(|| {
            walk_plugin
                .and_then(|p| p.get("path").and_then(Value::as_str))
                .map(PathBuf::from)
        })
        .ok_or_else(|| format!("{}: no path in the load order", file.plugin))?;

    // Each EditorID's object ID, from the plugin itself (the catalog's `inspect`).
    let mut ids = BTreeMap::new();
    for edid in file.editor_ids() {
        let r = run(
            "inspect",
            json!({"form": edid, "plugin": plugin_path.display().to_string()}),
        )?;
        let form_id = r
            .pointer("/data/record/form_id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{edid}: not found in {}", plugin_path.display()))?;
        let object = form_id.get(2..).unwrap_or_default();
        ids.insert(edid, runtime_id(&slot, object)?);
    }

    // The entry's header.
    let mut header: Vec<(String, String)> = Vec::new();
    let exe = |f: &str| {
        instance
            .as_ref()
            .and_then(|i| i.game_dir.as_ref())
            .and_then(|g| kitkraft_io::host::pe_file_version(&g.join(f)))
            .unwrap_or_else(|| "unknown".into())
    };
    header.push((
        "Game build".into(),
        format!(
            "`{}` {}",
            game.executables.game,
            exe(&game.executables.game)
        ),
    ));
    let mods: Vec<String> = kitkraft_io::text::read_lines(&profile_dir.join("modlist.txt"))
        .ok()
        .flatten()
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.trim().strip_prefix('+').map(str::to_string))
        .collect();
    let tools: Vec<String> = mods
        .iter()
        .filter(|m| {
            let lower = m.to_ascii_lowercase();
            game.console
                .mod_names
                .iter()
                .any(|n| lower.contains(&n.to_ascii_lowercase()))
                || lower.contains("console command runner")
        })
        .map(|m| {
            let v = instance
                .as_ref()
                .and_then(|i| read_ini(&i.mods_dir.join(m).join("meta.ini")))
                .and_then(|i| i.get("General", "version"))
                .unwrap_or_else(|| "unknown".into());
            format!("{m} {v}")
        })
        .collect();
    header.push((
        "Console tools".into(),
        if tools.is_empty() {
            "none enabled".into()
        } else {
            tools.join("; ")
        },
    ));
    let url = o.url.clone().unwrap_or_else(|| game.console.url.clone());
    header.push(("Console endpoint".into(), url.clone()));
    header.push((
        "MO2 profile".into(),
        format!("`{folder}` ({})", profile_dir.display()),
    ));
    header.push((
        "Save".into(),
        "the owner's: kit walk sends no save or load (W-09 and W-10 use the owner's save `KitKraftWalk`)".into(),
    ));
    let mut hashes = Vec::new();
    for name in std::iter::once(&file.plugin).chain(&file.also_record) {
        match find(name) {
            Some(p) => {
                let path = p.get("path").and_then(Value::as_str).unwrap_or_default();
                let sha = std::fs::read(path)
                    .map(|b| crate::deploy::sha256::hex(&b))
                    .unwrap_or_else(|e| format!("unreadable ({e})"));
                hashes.push(format!(
                    "`{name}` SHA-256 `{sha}`, position {}, slot {}",
                    p.get("index").map(Value::to_string).unwrap_or_default(),
                    p.get("runtime_slot").and_then(Value::as_str).unwrap_or("?")
                ));
            }
            None => {
                hashes.push(format!("`{name}` not enabled"));
                notes.push(format!(
                    "{name} is not enabled in the profile: its stations will not show what they expect"
                ));
            }
        }
    }
    header.push(("Plugins".into(), hashes.join("; ")));
    header.push((
        "Stations".into(),
        format!("`{}` SHA-256 `{stations_sha}`", o.stations.display()),
    ));
    header.push((
        "Runtime IDs".into(),
        format!(
            "slot {slot} of `{}`: {}",
            file.plugin,
            ids.iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ));
    let papyrus = o.papyrus_log.clone().or_else(|| {
        let spec = game.logs.iter().find(|l| l.name.contains("papyrus"))?;
        let path = if spec.path.contains("{documents}") {
            let d = documents()?.display().to_string();
            kitkraft_game::profile::fill(&spec.path, &[("documents", &d)])
        } else {
            spec.path.clone()
        };
        Some(PathBuf::from(path))
    });
    header.push((
        "Papyrus log".into(),
        papyrus
            .as_ref()
            .map_or_else(|| "unknown".into(), |p| p.display().to_string()),
    ));

    let (date, hms) = now_utc();
    let session = format!("walk-{date}-{hms}");
    let observe = o.observe.join(&session);
    let mut setup = WalkSetup::live(&file, &session, &date, observe);
    setup.ledger_id = format!(
        "game-{}-walk{}",
        kitkraft_game::ledger::compact(&date),
        hms.get(..4).unwrap_or_default()
    );
    setup.ids = ids;
    setup.header = header;
    setup.papyrus_log = papyrus;
    setup.header.insert(0, ("Date".into(), date));
    Ok(PreparedWalk {
        file,
        setup,
        url,
        notes,
    })
}

impl PreparedWalk {
    /// What the walk will do, for `--dry-run` and the start of a walk: the header, the
    /// runtime IDs, the notes and the stations.
    pub fn describe(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "{}
",
            self.file.title
        ));
        for (k, v) in &self.setup.header {
            s.push_str(&format!(
                "  {k}: {v}
"
            ));
        }
        s.push_str(&format!(
            "  Writes only: {}
",
            self.setup.observe.display()
        ));
        for n in &self.notes {
            s.push_str(&format!(
                "  note: {n}
"
            ));
        }
        for st in &self.file.stations {
            s.push_str(&format!(
                "  {} {}: {}
",
                st.id,
                if st.covers.is_empty() {
                    String::new()
                } else {
                    format!("({})", st.covers.join(", "))
                },
                st.title
            ));
        }
        s
    }

    /// Runs the walk on the real console endpoint, the terminal (stdin, stdout) and the
    /// host's screen capture.
    pub fn run_live(&self) -> Result<kitkraft_game::walk::WalkReport, String> {
        use kitkraft_game::host::Host;
        let mut console = kitkraft_game::console::HttpConsole::new(&self.url)?;
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let mut out = std::io::stdout();
        let host = kitkraft_game::host::SystemHost;
        let mut shoot = |p: &Path| host.screenshot(p).map_err(|e| e.to_string());
        kitkraft_game::walk::run(
            &self.file,
            &self.setup,
            &mut console,
            &mut input,
            &mut out,
            &mut shoot,
        )
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_ids_follow_the_slot_scale() {
        assert_eq!(runtime_id("0b", "00100c").unwrap(), "0B00100C");
        assert_eq!(runtime_id("FD:01", "000801").unwrap(), "FD010801");
        assert_eq!(runtime_id("FE:00A", "000801").unwrap(), "FE00A801");
        assert!(runtime_id("B", "001000").is_err());
        assert!(runtime_id("0B", "1000").is_err());
    }

    #[test]
    fn the_live_profile_is_refused_by_name() {
        let o = WalkOptions {
            profile: "Default".into(),
            ..WalkOptions::default()
        };
        assert!(prepare(&o).unwrap_err().contains("live-profile"));
    }
}
