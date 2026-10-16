//! `profiles` (lane W, the workbench's first-run screen): read-only. The MO2 instance (the
//! one given, else the default one), its profiles with the owner's live game marked, and
//! the Data folder a profile `load` would use.
//!
//! Where an instance is looked for, in order: the `instance` argument (only it, when
//! given); `KITKRAFT_MO2_DIR`; then the instances `kit doctor` detects, in its order
//! ([`kitkraft_io::host::Host::mo2_instances`]: `C:\Modding\MO2-Starfield`, MO2's
//! `CurrentInstance`, the other Starfield instances under `%LOCALAPPDATA%\ModOrganizer`).
//! One implementation ([`crate::host`]), so the start screen and `kit doctor` name the
//! same default instance; like `kit doctor`, it detects even when `KITKRAFT_DETECT=0`.
//! Every place is listed in `data.searched` with whether it held an instance. Nothing is
//! written, created or followed (`kitkraft_io::Instance`).

use std::ffi::OsString;
use std::path::PathBuf;

use kitkraft_io::host::{same_path, Host};
use kitkraft_io::Instance;
use serde_json::{json, Value};

use super::testplan::DEV_PROFILE;
use super::{env_var_os, DATA_DIR_ENV};
use crate::envelope::{Coverage, CoverageStatus, Outcome};
use crate::exec::EngineError;
use crate::host::mo2_from_env;

/// The owner's live-game profile (AGENTS.md): read-only input, never a test target.
const LIVE_PROFILE: &str = "Default";

/// One place an instance was looked for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    path: PathBuf,
    source: String,
}

/// The places to look, in order: `instance` alone when given; else `KITKRAFT_MO2_DIR`,
/// the instances `host` detects (`kit doctor`'s order), then `host`'s fixed candidates
/// detection skipped (listed so `searched` shows them).
fn candidates(instance: Option<&str>, host: &Host) -> Vec<Candidate> {
    if let Some(i) = instance {
        return vec![Candidate {
            path: PathBuf::from(i),
            source: "argument".to_string(),
        }];
    }
    let mut out: Vec<Candidate> = Vec::new();
    let mut push = |path: PathBuf, source: String| {
        if !out.iter().any(|c| same_path(&c.path, &path)) {
            out.push(Candidate { path, source });
        }
    };
    if let Some(f) = mo2_from_env(&*host.env) {
        push(f.path, f.how);
    }
    for f in host.mo2_instances() {
        push(f.path, f.how);
    }
    for c in &host.mo2_candidates {
        push(c.clone(), "default location".to_string());
    }
    out
}

/// The Data folder and where it came from: `data_dir`, the instance's `gamePath`, then
/// `KITKRAFT_DATA_DIR` (as `load` resolves it for a profile).
fn data_dir(
    arg: Option<&str>,
    instance: Option<&Instance>,
    env: Option<OsString>,
) -> Option<(PathBuf, &'static str)> {
    arg.map(|d| (PathBuf::from(d), "argument"))
        .or_else(|| {
            instance
                .and_then(Instance::data_dir)
                .map(|d| (d, "instance gamePath"))
        })
        .or_else(|| {
            env.filter(|e| !e.is_empty())
                .map(|e| (PathBuf::from(e), DATA_DIR_ENV))
        })
}

fn profile_rows(instance: &Instance) -> Result<Vec<Value>, String> {
    let names = instance.profiles().map_err(|e| e.to_string())?;
    Ok(names
        .iter()
        .map(|name| {
            let path = instance.profile_dir(name);
            json!({
                "name": name,
                "path": path.display().to_string(),
                "live": name.eq_ignore_ascii_case(LIVE_PROFILE),
                "dev": name.eq_ignore_ascii_case(DEV_PROFILE),
                "selected": instance.selected_profile.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(name)),
                "plugins_txt": path.join("plugins.txt").is_file(),
            })
        })
        .collect())
}

/// `profiles` with the host and the environment passed in (tests run it without the
/// process's).
fn report(args: &Value, host: &Host, data_env: Option<OsString>) -> Outcome {
    let arg = |n: &str| args.get(n).and_then(Value::as_str);
    let mut limitations = Vec::new();
    let mut searched = Vec::new();
    let mut found: Option<(Instance, String)> = None;
    for c in candidates(arg("instance"), host) {
        let read = if found.is_none() {
            Instance::read(&c.path).ok()
        } else {
            None
        };
        searched.push(json!({
            "path": c.path.display().to_string(),
            "source": c.source,
            "found": read.is_some() || (found.is_some() && c.path.join("ModOrganizer.ini").is_file()),
        }));
        if let Some(i) = read {
            found = Some((i, c.source));
        }
    }
    let instance = found.as_ref().map(|(i, _)| i);
    let (profiles, instance_json) = match &found {
        Some((i, by)) => {
            if !i
                .game_name
                .as_deref()
                .is_some_and(|g| g.eq_ignore_ascii_case("Starfield"))
            {
                limitations.push(format!(
                    "{}: gameName is {}, not Starfield",
                    i.ini_path().display(),
                    i.game_name.as_deref().unwrap_or("not set")
                ));
            }
            let rows = profile_rows(i).unwrap_or_else(|e| {
                limitations.push(format!("profiles not listed: {e}"));
                Vec::new()
            });
            (
                rows,
                json!({
                    "root": i.root.display().to_string(),
                    "found_by": by,
                    "game_name": i.game_name,
                    "game_dir": i.game_dir.as_ref().map(|d| d.display().to_string()),
                    "profiles_dir": i.profiles_dir.display().to_string(),
                    "mods_dir": i.mods_dir.display().to_string(),
                    "selected_profile": i.selected_profile,
                }),
            )
        }
        None => {
            limitations.push(
                "no MO2 instance found in the places searched; pass `instance` (the folder holding ModOrganizer.ini)"
                    .to_string(),
            );
            (Vec::new(), Value::Null)
        }
    };
    let data = data_dir(arg("data_dir"), instance, data_env);
    let data_json = match &data {
        Some((d, source)) => json!({
            "path": d.display().to_string(),
            "source": source,
            "exists": d.is_dir(),
            "starfield_esm": d.join("Starfield.esm").is_file(),
        }),
        None => {
            limitations.push(format!(
                "no Data folder: the instance has no gamePath and {DATA_DIR_ENV} is not set"
            ));
            json!({"path": null, "source": null, "exists": false, "starfield_esm": false})
        }
    };
    if profiles.iter().any(|p| p["live"] == Value::Bool(true)) {
        limitations.push(format!(
            "`{LIVE_PROFILE}` is the owner's live game: loading it only reads it; in-game checks run on `{DEV_PROFILE}`"
        ));
    }
    let summary = match &found {
        Some((i, by)) => format!(
            "MO2 instance {} ({by}): {} profile(s); Data {}",
            i.root.display(),
            profiles.len(),
            data.as_ref().map_or_else(
                || "unknown".to_string(),
                |(d, s)| format!("{} ({s})", d.display())
            )
        ),
        None => format!(
            "no MO2 instance found ({} place(s) searched)",
            searched.len()
        ),
    };
    Outcome {
        data: json!({
            "instance": instance_json,
            "searched": searched,
            "profiles": profiles,
            "data_dir": data_json,
            "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence: Vec::new(),
    }
}

/// `profiles`.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    Ok(report(args, &Host::system(), env_var_os(DATA_DIR_ENV)))
}

/// Whether `dir` holds an instance (for the tests' fixtures).
#[cfg(test)]
fn is_instance(dir: &std::path::Path) -> bool {
    Instance::read(dir).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixture(tag: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("kitkraft-profiles-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mo2 = base.join("MO2");
        for p in ["Default", "KitKraft Dev"] {
            let dir = mo2.join("profiles").join(p);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("modlist.txt"), "").unwrap();
            std::fs::write(dir.join("plugins.txt"), "").unwrap();
        }
        std::fs::create_dir_all(mo2.join("profiles").join("not-a-profile")).unwrap();
        let game = base.join("Game");
        std::fs::create_dir_all(game.join("Data")).unwrap();
        std::fs::write(game.join("Data").join("Starfield.esm"), b"x").unwrap();
        let game_path = game.display().to_string().replace('\\', "\\\\");
        std::fs::write(
            mo2.join("ModOrganizer.ini"),
            format!("[General]\ngameName=Starfield\ngamePath=@ByteArray({game_path})\nselected_profile=@ByteArray(Default)\n"),
        )
        .unwrap();
        base
    }

    #[test]
    fn an_instance_given_lists_its_profiles_with_the_live_one_marked() {
        let base = fixture("given");
        let mo2 = base.join("MO2");
        assert!(is_instance(&mo2));
        let out = report(
            &json!({"instance": mo2.display().to_string()}),
            &host(Some(PathBuf::from("Z:/ignored")), Vec::new(), None, None),
            None,
        );
        let d = &out.data;
        assert_eq!(d["instance"]["found_by"], "argument");
        assert_eq!(
            d["searched"].as_array().unwrap().len(),
            1,
            "the argument alone"
        );
        let names: Vec<&str> = d["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["Default", "KitKraft Dev"]);
        assert_eq!(d["profiles"][0]["live"], true);
        assert_eq!(d["profiles"][0]["selected"], true);
        assert_eq!(d["profiles"][1]["dev"], true);
        assert_eq!(d["profiles"][1]["live"], false);
        assert_eq!(d["data_dir"]["source"], "instance gamePath");
        assert_eq!(d["data_dir"]["starfield_esm"], true);
        assert!(out
            .limitations
            .iter()
            .any(|l| l.contains("owner's live game")));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A host with `KITKRAFT_MO2_DIR` = `env_mo2`, the fixed candidates `portable`, the
    /// `%LOCALAPPDATA%` `local` and MO2's `CurrentInstance` = `current`.
    fn host(
        env_mo2: Option<PathBuf>,
        portable: Vec<PathBuf>,
        local: Option<PathBuf>,
        current: Option<&str>,
    ) -> Host {
        let registry = kitkraft_io::host::FixedRegistry(
            current
                .map(|c| {
                    vec![(
                        kitkraft_io::host::MO2_KEY.to_string(),
                        kitkraft_io::host::MO2_VALUE.to_string(),
                        c.to_string(),
                    )]
                })
                .unwrap_or_default(),
        );
        Host {
            registry: Box::new(registry),
            env: Box::new(move |name| match name {
                crate::host::MO2_DIR_ENV => env_mo2.clone().map(PathBuf::into_os_string),
                "LOCALAPPDATA" => local.clone().map(PathBuf::into_os_string),
                _ => None,
            }),
            game_candidates: Vec::new(),
            mo2_candidates: portable,
            tool_dirs: Vec::new(),
        }
    }

    fn ini(dir: &Path, game: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("ModOrganizer.ini"),
            format!("[General]\ngameName={game}\n"),
        )
        .unwrap();
    }

    #[test]
    fn detection_is_kit_doctors_after_the_variable() {
        let base = fixture("detect");
        let mo2 = base.join("MO2");
        // Global instances: the registry's current one, another Starfield one, and one
        // of another game (skipped).
        let local = base.join("Local");
        let global = local.join("ModOrganizer");
        ini(&global.join("Current"), "Starfield");
        ini(&global.join("Another"), "Starfield");
        ini(&global.join("Skyrim"), "Skyrim Special Edition");
        let missing = base.join("Missing");
        let h = host(
            None,
            vec![mo2.clone(), missing.clone()],
            Some(local.clone()),
            Some("Current"),
        );
        let list = candidates(None, &h);
        let paths: Vec<&Path> = list.iter().map(|c| c.path.as_path()).collect();
        assert_eq!(
            paths,
            [
                mo2.as_path(),
                global.join("Current").as_path(),
                global.join("Another").as_path(),
                missing.as_path()
            ],
            "the portable instance first, as kit doctor"
        );
        // The start screen and kit doctor name the same default instance.
        let out = report(&json!({}), &h, None);
        let doctor = crate::host::detect(&h);
        assert_eq!(
            out.data["instance"]["root"],
            json!(doctor.mo2_root().unwrap().path.display().to_string())
        );
        assert_eq!(out.data["instance"]["found_by"], "default location");
        let found: Vec<bool> = out.data["searched"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["found"].as_bool().unwrap())
            .collect();
        assert_eq!(found, [true, true, true, false]);

        // KITKRAFT_MO2_DIR goes first.
        let h = host(
            Some(global.join("Another")),
            vec![mo2.clone()],
            Some(local),
            Some("Current"),
        );
        let out = report(&json!({}), &h, None);
        assert_eq!(
            out.data["instance"]["root"],
            json!(global.join("Another").display().to_string())
        );
        assert_eq!(
            out.data["instance"]["found_by"],
            format!("environment variable {}", crate::host::MO2_DIR_ENV)
        );
        assert_eq!(out.data["searched"].as_array().unwrap().len(), 3);

        // Nothing found: the places are listed and the data dir comes from the env.
        let none = report(
            &json!({"instance": base.join("nothing").display().to_string()}),
            &host(None, Vec::new(), None, None),
            Some(OsString::from(base.join("Game").join("Data"))),
        );
        assert_eq!(none.data["instance"], Value::Null);
        assert_eq!(none.data["data_dir"]["source"], DATA_DIR_ENV);
        assert!(none
            .limitations
            .iter()
            .any(|l| l.contains("no MO2 instance")));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn runs_through_execute_and_writes_nothing() {
        let base = fixture("execute");
        let mo2 = base.join("MO2");
        let before: Vec<_> = walk(&base);
        let env =
            crate::execute("profiles", json!({"instance": mo2.display().to_string()})).unwrap();
        assert_eq!(env["command"], "profiles");
        assert_eq!(env["data"]["profiles"].as_array().unwrap().len(), 2);
        assert_eq!(walk(&base), before);
        assert!(!crate::find_command("profiles").unwrap().writes);
        let _ = std::fs::remove_dir_all(&base);
    }

    fn walk(dir: &Path) -> Vec<(PathBuf, u64)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push((p.clone(), std::fs::metadata(&p).unwrap().len()));
                }
            }
        }
        out.sort();
        out
    }
}
