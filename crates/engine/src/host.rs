//! What `kit doctor` detects, as the default every command falls back to.
//!
//! Precedence, for every command that takes `data_dir` (and for a profile given by name):
//! **argument > environment variable > detected**. The game Data dir is the `data_dir`
//! argument, else `KITKRAFT_DATA_DIR`, else the detected game install's `Data` folder
//! (Steam's registry and libraries, then the common install folders, then the MO2
//! instance's `gamePath`: [`kitkraft_io::host`]). A `profile` that is a bare name
//! (`KitKraft Dev`, no folder separator) and not a folder in the working directory is
//! the profile of that name in the detected MO2 instance.
//!
//! Detection runs once per process, read-only. `KITKRAFT_DETECT=0` (or `off`) turns the
//! detected defaults off (arguments and variables still apply); `kit doctor` and
//! `profiles` themselves always detect. The MO2 instance follows the same rule: an
//! argument, else `KITKRAFT_MO2_DIR`, else the first instance
//! [`kitkraft_io::host::Host::mo2_instances`] finds (one implementation for `kit doctor`,
//! `profiles`, profile names and `kit install`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use kitkraft_io::host::{Found, Host};
use kitkraft_io::Instance;

/// Turns the detected defaults off when `0` or `off`.
pub(crate) const DETECT_ENV: &str = "KITKRAFT_DETECT";

/// The owner's development profile (AGENTS.md, "Owner's live game is off limits").
pub(crate) const DEV_PROFILE: &str = "KitKraft Dev";

/// What detection found.
#[derive(Debug, Clone, Default)]
pub(crate) struct Detected {
    /// The game install (holds `Starfield.exe`).
    pub game: Option<Found>,
    /// Every game folder looked at, in order.
    pub game_looked: Vec<PathBuf>,
    /// The game's Data folder.
    pub data_dir: Option<Found>,
    /// The MO2 instances found; the first is the default.
    pub mo2: Vec<Found>,
}

impl Detected {
    /// The default MO2 instance.
    pub(crate) fn mo2_root(&self) -> Option<&Found> {
        self.mo2.first()
    }
}

/// Detects with `host` (read-only).
pub(crate) fn detect(host: &Host) -> Detected {
    let (game, game_looked) = host.game_dir();
    let mo2 = host.mo2_instances();
    let data_dir = game
        .as_ref()
        .map(|g| g.path.join("Data"))
        .filter(|d| d.is_dir())
        .map(|path| Found {
            path,
            how: "the Data folder of the game install".to_string(),
        })
        .or_else(|| {
            mo2.iter().find_map(|m| {
                let inst = Instance::read(&m.path).ok()?;
                let d = inst.data_dir()?;
                d.is_dir().then(|| Found {
                    path: d,
                    how: format!("gamePath in {}", inst.ini_path().display()),
                })
            })
        });
    Detected {
        game,
        game_looked,
        data_dir,
        mo2,
    }
}

fn detection_off() -> bool {
    std::env::var(DETECT_ENV).is_ok_and(|v| {
        let v = v.trim();
        v == "0" || v.eq_ignore_ascii_case("off") || v.eq_ignore_ascii_case("false")
    })
}

/// This machine's detection, once per process; empty when [`DETECT_ENV`] turns it off.
pub(crate) fn detected() -> &'static Detected {
    static DETECTED: OnceLock<Detected> = OnceLock::new();
    DETECTED.get_or_init(|| {
        if detection_off() {
            Detected::default()
        } else {
            detect(&Host::system())
        }
    })
}

/// The profile folder a `profile` argument names: as given when it is a folder or has a
/// folder separator; otherwise the profile of that name (ASCII case ignored, as Windows
/// does) in `mo2`'s profiles folder when one exists there; otherwise as given.
pub(crate) fn profile_dir_in(arg: &str, mo2: Option<&Path>) -> PathBuf {
    let given = PathBuf::from(arg);
    if given.is_dir() || arg.contains(['\\', '/']) || arg.trim().is_empty() {
        return given;
    }
    let Some(inst) = mo2.and_then(|m| Instance::read(m).ok()) else {
        return given;
    };
    let names = inst.profiles().unwrap_or_default();
    names
        .iter()
        .find(|n| n.as_str() == arg)
        .or_else(|| names.iter().find(|n| n.eq_ignore_ascii_case(arg)))
        .map_or(given, |n| inst.profile_dir(n))
}

/// [`profile_dir_in`] with the default MO2 instance ([`default_mo2`]).
pub(crate) fn profile_dir(arg: &str) -> PathBuf {
    profile_dir_in(arg, default_mo2().as_ref().map(|f| f.path.as_path()))
}

/// Names the MO2 instance root ahead of detection (argument > environment > detected).
pub(crate) const MO2_DIR_ENV: &str = "KITKRAFT_MO2_DIR";

/// The MO2 instance [`MO2_DIR_ENV`] names through `env` (unset or empty: none).
pub(crate) fn mo2_from_env(env: &dyn Fn(&str) -> Option<OsString>) -> Option<Found> {
    env(MO2_DIR_ENV).filter(|v| !v.is_empty()).map(|v| Found {
        path: PathBuf::from(v),
        how: format!("environment variable {MO2_DIR_ENV}"),
    })
}

/// The default MO2 instance every command uses (`kit doctor`, `profiles`, profile names,
/// `kit install`'s MCP arguments): [`MO2_DIR_ENV`], else the first detected instance
/// (none when [`DETECT_ENV`] turns detection off).
pub(crate) fn default_mo2() -> Option<Found> {
    mo2_from_env(&|n| std::env::var_os(n)).or_else(|| detected().mo2_root().cloned())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use kitkraft_io::host::FixedRegistry;

    fn temp(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("kitkraft-engine-host-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn host(game: &Path, mo2: &Path) -> Host {
        Host {
            registry: Box::new(FixedRegistry::default()),
            env: Box::new(|_| None),
            game_candidates: vec![game.to_path_buf()],
            mo2_candidates: vec![mo2.to_path_buf()],
            tool_dirs: Vec::new(),
        }
    }

    #[test]
    fn data_dir_from_the_game_else_mo2_game_path() {
        let root = temp("detect");
        let game = root.join("Game");
        let mo2 = root.join("MO2");
        std::fs::create_dir_all(game.join("Data")).unwrap();
        std::fs::create_dir_all(mo2.join("profiles/Kitkraft Dev")).unwrap();
        std::fs::write(mo2.join("profiles/Kitkraft Dev/modlist.txt"), b"").unwrap();
        let game_text = game.display().to_string().replace('\\', "\\\\");
        std::fs::write(
            mo2.join("ModOrganizer.ini"),
            format!("[General]\r\ngameName=Starfield\r\ngamePath=@ByteArray({game_text})\r\n"),
        )
        .unwrap();
        // No Starfield.exe: the game is not found, the Data dir comes from MO2's gamePath.
        let d = detect(&host(&game, &mo2));
        assert!(d.game.is_none());
        let data = d.data_dir.unwrap();
        assert_eq!(data.path, game.join("Data"));
        assert!(data.how.contains("gamePath"), "{}", data.how);
        std::fs::write(game.join("Starfield.exe"), b"MZ").unwrap();
        let d = detect(&host(&game, &mo2));
        assert_eq!(d.game.as_ref().unwrap().path, game);
        assert!(d.data_dir.as_ref().unwrap().how.contains("game install"));
        assert_eq!(d.mo2_root().unwrap().path, mo2);

        // A bare profile name resolves in the instance, ignoring case; paths stay.
        assert_eq!(
            profile_dir_in("KitKraft Dev", Some(&mo2)),
            mo2.join("profiles").join("Kitkraft Dev")
        );
        assert_eq!(profile_dir_in("Nope", Some(&mo2)), PathBuf::from("Nope"));
        assert_eq!(
            profile_dir_in(r"x\KitKraft Dev", Some(&mo2)),
            PathBuf::from(r"x\KitKraft Dev")
        );
        assert_eq!(
            profile_dir_in("KitKraft Dev", None),
            PathBuf::from("KitKraft Dev")
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
