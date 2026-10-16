//! Per-game profiles (ADR 0010 rule 6): what the controller needs to know about one game,
//! as data in `profiles/<game>.toml`, so the controller itself names no game.
//!
//! A profile holds the executable names, the MO2 executable and command line that starts
//! the game, the console endpoint and its readiness command, the save commands, the INI
//! switches the owner sets on the dev profile, the log files the game writes, and the
//! timeouts. Every fact that only an observation can settle carries `evidence` (a ledger
//! entry id such as `game-20261016-T1/a`) and, until it has one, `pending` (the ticket
//! that observes it). [`GameProfile::missing`] lists the facts a live session needs that
//! have no evidence; the controller refuses a live session while that list is not empty
//! (M4.T9), and `--dry-run` shows it. Nothing here is read from a game binary.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{GameError, Result};

/// The games with a profile in this crate.
pub const BUILTIN: &[&str] = &["starfield", "skyrimse", "fallout4"];

/// The text of a built-in profile.
pub fn builtin_text(id: &str) -> Option<&'static str> {
    match id {
        "starfield" => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/profiles/starfield.toml"
        ))),
        "skyrimse" => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/profiles/skyrimse.toml"
        ))),
        "fallout4" => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/profiles/fallout4.toml"
        ))),
        _ => None,
    }
}

/// One game's profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameProfile {
    /// Name and overall status.
    pub game: GameInfo,
    /// Executable file names.
    pub executables: Executables,
    /// How MO2 starts the game.
    pub launch: Launch,
    /// The console channel.
    pub console: ConsoleSpec,
    /// Saves and the commands that make and load them.
    pub saves: Saves,
    /// What the MO2 profile must have switched on.
    #[serde(default)]
    pub mo2: Mo2Requirements,
    /// INI switches the owner sets once on the dev profile (M4.O7); read, never written.
    #[serde(default, rename = "ini")]
    pub ini: Vec<IniSwitch>,
    /// Log files the game or its plugins write.
    #[serde(default, rename = "log")]
    pub logs: Vec<LogSpec>,
    /// Session budgets.
    #[serde(default)]
    pub timeouts: Timeouts,
}

/// `[game]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameInfo {
    /// Short id (`starfield`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// `verified`, `partial` or `unverified` (a stub naming candidates only).
    pub status: String,
    /// Free text.
    #[serde(default)]
    pub note: String,
}

/// `[executables]`: file names, never paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executables {
    /// The game (`Starfield.exe`).
    pub game: String,
    /// The script extender's loader (`sfse_loader.exe`).
    pub script_extender: String,
    /// The editor (`CreationKit.exe`); running, it blocks a session.
    #[serde(default)]
    pub editor: Option<String>,
    /// The mod manager (`ModOrganizer.exe`); running, it blocks a session.
    pub mod_manager: String,
    /// Helper processes the mod manager starts for the game, ended with it.
    #[serde(default)]
    pub helpers: Vec<String>,
}

impl Executables {
    /// The processes whose presence refuses a session (the owner may be playing).
    pub fn blocking(&self) -> Vec<String> {
        let mut v = vec![self.game.clone(), self.script_extender.clone()];
        if let Some(e) = &self.editor {
            v.push(e.clone());
        }
        v.push(self.mod_manager.clone());
        v
    }

    /// The processes the controller ends at the end of a session (it started them).
    pub fn ended(&self) -> Vec<String> {
        let mut v = vec![self.game.clone(), self.script_extender.clone()];
        v.extend(self.helpers.iter().cloned());
        v.push(self.mod_manager.clone());
        v
    }
}

/// `[launch]`: the mod manager's command line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Launch {
    /// The MO2 executable entry that starts the game (`SFSE`).
    pub mo2_executable: String,
    /// Arguments to the mod manager; `{profile}` and `{executable}` are filled in.
    pub args: Vec<String>,
    /// Ledger evidence for the command line; empty until observed.
    #[serde(default)]
    pub evidence: String,
    /// The ticket that observes it.
    #[serde(default)]
    pub pending: String,
}

/// `[console]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleSpec {
    /// The endpoint (`http://127.0.0.1:55555/console`); loopback only. Empty in a stub.
    pub url: String,
    /// A harmless command whose answer means the endpoint is up.
    #[serde(default)]
    pub ready_command: String,
    /// Text the readiness answer contains (empty: any answer).
    #[serde(default)]
    pub ready_expect: String,
    /// Names (or parts of names) of the MO2 mod that provides the endpoint; one of them
    /// must be an enabled mod of the dev profile.
    #[serde(default)]
    pub mod_names: Vec<String>,
    /// For a stub: the channels that could serve (named, not verified).
    #[serde(default)]
    pub candidates: Vec<String>,
    /// Ledger evidence that the endpoint answers; empty until observed.
    #[serde(default)]
    pub evidence: String,
    /// The ticket that observes it.
    #[serde(default)]
    pub pending: String,
}

/// `[saves]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saves {
    /// The saves folder, relative to the MO2 profile (profile-local saves).
    pub dir: String,
    /// The save file extension (`sfs`).
    pub extension: String,
    /// The console command that loads a save; `{save}` is the name.
    pub load_command: String,
    /// The console command that saves; `{save}` is the name.
    pub save_command: String,
    /// The base save loaded when the call names none.
    #[serde(default = "default_base")]
    pub base_default: String,
    /// The only names the controller saves to start with this.
    #[serde(default = "default_prefix")]
    pub session_prefix: String,
    /// Ledger evidence for the commands; empty until observed.
    #[serde(default)]
    pub evidence: String,
    /// The ticket that observes it.
    #[serde(default)]
    pub pending: String,
}

fn default_base() -> String {
    "KitKraftBase".into()
}

fn default_prefix() -> String {
    "KitKraftSession-".into()
}

/// `[mo2]`: profile options the session depends on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mo2Requirements {
    /// The profile's `settings.ini` must say `LocalSettings=true`: otherwise the game
    /// reads the INIs under `My Games`, not the profile's, and the switches are not seen.
    #[serde(default = "yes")]
    pub local_settings: bool,
    /// The profile's `settings.ini` must say `LocalSaves=true` (saves in the profile).
    #[serde(default = "yes")]
    pub local_saves: bool,
}

impl Default for Mo2Requirements {
    fn default() -> Self {
        Mo2Requirements {
            local_settings: true,
            local_saves: true,
        }
    }
}

fn yes() -> bool {
    true
}

/// `[[ini]]`: one required switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IniSwitch {
    /// The INI file in the MO2 profile (`StarfieldCustom.ini`).
    pub file: String,
    /// Section, without brackets.
    pub section: String,
    /// Key.
    pub key: String,
    /// Required value (compared ignoring ASCII case and surrounding blanks).
    pub value: String,
}

/// `[[log]]`: a file the game writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogSpec {
    /// Short name (`papyrus`, `console-history`).
    pub name: String,
    /// Path; `{documents}` is the user's Documents folder.
    pub path: String,
    /// Ledger evidence for the path; empty until observed.
    #[serde(default)]
    pub evidence: String,
    /// The ticket that observes it.
    #[serde(default)]
    pub pending: String,
}

/// `[timeouts]`, in seconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeouts {
    /// The whole session's wall-clock budget.
    #[serde(default = "t_session")]
    pub session_secs: u64,
    /// One probe row.
    #[serde(default = "t_probe")]
    pub probe_secs: u64,
    /// From launch to the endpoint's first answer.
    #[serde(default = "t_ready")]
    pub ready_secs: u64,
    /// From a `load` to the endpoint answering again.
    #[serde(default = "t_load")]
    pub load_secs: u64,
    /// From terminating to the processes being gone.
    #[serde(default = "t_exit")]
    pub exit_secs: u64,
}

fn t_session() -> u64 {
    1800
}
fn t_probe() -> u64 {
    60
}
fn t_ready() -> u64 {
    300
}
fn t_load() -> u64 {
    180
}
fn t_exit() -> u64 {
    60
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            session_secs: t_session(),
            probe_secs: t_probe(),
            ready_secs: t_ready(),
            load_secs: t_load(),
            exit_secs: t_exit(),
        }
    }
}

/// A fact a live session needs that has no evidence yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unverified {
    /// Which fact (`launch`, `console`, `saves`, `profile`).
    pub fact: String,
    /// Who observes it, or why it is missing.
    pub pending: String,
}

impl GameProfile {
    /// Parses a profile; `name` is used in errors.
    pub fn from_toml(name: &str, text: &str) -> Result<GameProfile> {
        let profile: GameProfile = toml::from_str(text).map_err(|e| GameError::Profile {
            name: name.to_string(),
            message: e.to_string(),
        })?;
        profile.check(name)?;
        Ok(profile)
    }

    /// A built-in profile by id, or a profile file by path.
    pub fn named(name_or_path: &str) -> Result<GameProfile> {
        if let Some(text) = builtin_text(name_or_path) {
            return GameProfile::from_toml(name_or_path, text);
        }
        let path = Path::new(name_or_path);
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
        {
            let text = std::fs::read_to_string(path).map_err(|e| GameError::io(path, e))?;
            return GameProfile::from_toml(name_or_path, &text);
        }
        Err(GameError::Profile {
            name: name_or_path.to_string(),
            message: format!(
                "not a built-in game ({}) and not a .toml file",
                BUILTIN.join(", ")
            ),
        })
    }

    fn check(&self, name: &str) -> Result<()> {
        let bad = |message: String| GameError::Profile {
            name: name.to_string(),
            message,
        };
        if !matches!(
            self.game.status.as_str(),
            "verified" | "partial" | "unverified"
        ) {
            return Err(bad(format!(
                "[game] status `{}` is not verified, partial or unverified",
                self.game.status
            )));
        }
        for exe in self.executables.blocking() {
            if exe.is_empty() || exe.contains(['/', '\\']) {
                return Err(bad(format!(
                    "executable `{exe}` must be a file name, not a path"
                )));
            }
        }
        if !self.console.url.is_empty() {
            crate::console::Endpoint::parse(&self.console.url)
                .map_err(|e| bad(format!("[console] url: {e}")))?;
        }
        if self.saves.session_prefix.len() < 4 {
            return Err(bad("[saves] session_prefix is too short".into()));
        }
        Ok(())
    }

    /// The facts a live session needs that have no evidence. Empty: the controller may
    /// launch (preflight still applies).
    pub fn missing(&self) -> Vec<Unverified> {
        let mut out = Vec::new();
        if self.game.status == "unverified" {
            out.push(Unverified {
                fact: "profile".into(),
                pending: format!(
                    "the {} profile is a stub marked unverified: {}",
                    self.game.name, self.game.note
                ),
            });
        }
        let mut need = |fact: &str, evidence: &str, pending: &str| {
            if evidence.trim().is_empty() {
                out.push(Unverified {
                    fact: fact.into(),
                    pending: if pending.is_empty() {
                        "no evidence recorded".into()
                    } else {
                        pending.into()
                    },
                });
            }
        };
        need("launch", &self.launch.evidence, &self.launch.pending);
        need("console", &self.console.evidence, &self.console.pending);
        need("saves", &self.saves.evidence, &self.saves.pending);
        if self.console.url.is_empty() {
            out.push(Unverified {
                fact: "console.url".into(),
                pending: format!(
                    "no console endpoint; candidates: {}",
                    if self.console.candidates.is_empty() {
                        "none named".to_string()
                    } else {
                        self.console.candidates.join("; ")
                    }
                ),
            });
        }
        out
    }

    /// Log specs without evidence (not blocking: a missing log makes its rows
    /// `inconclusive`).
    pub fn unverified_logs(&self) -> Vec<Unverified> {
        self.logs
            .iter()
            .filter(|l| l.evidence.trim().is_empty())
            .map(|l| Unverified {
                fact: format!("log.{}", l.name),
                pending: l.pending.clone(),
            })
            .collect()
    }

    /// The mod manager's arguments with `{profile}` and `{executable}` filled in.
    pub fn launch_args(&self, profile: &str) -> Vec<String> {
        self.launch
            .args
            .iter()
            .map(|a| {
                fill(
                    a,
                    &[
                        ("profile", profile),
                        ("executable", &self.launch.mo2_executable),
                    ],
                )
            })
            .collect()
    }
}

/// Replaces each `{key}` in `template` by its value.
pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut s = template.to_string();
    for (k, v) in values {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_parse_and_starfield_is_partial_until_m4_t1() {
        for id in BUILTIN {
            let p = GameProfile::named(id).unwrap();
            assert_eq!(&p.game.id, id);
        }
        let sf = GameProfile::named("starfield").unwrap();
        assert_eq!(sf.game.status, "partial");
        assert_eq!(sf.executables.game, "Starfield.exe");
        assert_eq!(sf.launch.mo2_executable, "SFSE");
        let missing: Vec<String> = sf.missing().into_iter().map(|u| u.fact).collect();
        assert_eq!(missing, ["launch", "console", "saves"]);
        assert!(sf.missing()[0].pending.contains("M4.T1 (a)"));
        assert_eq!(
            sf.launch_args("Kitkraft Dev"),
            ["-p", "Kitkraft Dev", "moshortcut://:SFSE"]
        );
    }

    #[test]
    fn stubs_are_refused_with_what_is_missing() {
        for id in ["skyrimse", "fallout4"] {
            let p = GameProfile::named(id).unwrap();
            assert_eq!(p.game.status, "unverified");
            let missing = p.missing();
            let facts: Vec<&str> = missing.iter().map(|u| u.fact.as_str()).collect();
            assert_eq!(
                facts,
                ["profile", "launch", "console", "saves", "console.url"],
                "{id}"
            );
            assert!(missing[4].pending.contains("ConsoleUtil"), "{id}");
        }
    }

    #[test]
    fn bad_profiles_are_errors() {
        let sf = builtin_text("starfield").unwrap();
        let remote = sf.replace(
            "http://127.0.0.1:55555/console",
            "http://10.0.0.5:55555/console",
        );
        assert!(GameProfile::from_toml("x", &remote)
            .unwrap_err()
            .to_string()
            .contains("loopback"));
        let path_exe = sf.replace("game = \"Starfield.exe\"", "game = \"C:\\\\Starfield.exe\"");
        assert!(GameProfile::from_toml("x", &path_exe).is_err());
        assert!(GameProfile::from_toml("x", "nonsense = 1").is_err());
        assert!(GameProfile::named("morrowind").is_err());
    }

    #[test]
    fn fill_replaces_keys() {
        assert_eq!(
            fill("load {save}", &[("save", "KitKraftBase")]),
            "load KitKraftBase"
        );
    }
}
