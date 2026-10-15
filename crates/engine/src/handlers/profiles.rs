//! `profiles` (lane W, the workbench's first-run screen): read-only. The MO2 instance (the
//! one given, else the first one found), its profiles with the owner's live game marked,
//! and the Data folder a profile `load` would use.
//!
//! Where an instance is looked for, in order: the `instance` argument (only it, when
//! given); `KITKRAFT_MO2_DIR`; each instance folder under `%LOCALAPPDATA%\ModOrganizer`
//! whose `gameName` is Starfield (MO2's global instances); `C:\Modding\MO2-Starfield`
//! (the dev box's portable instance, AGENTS.md). Every place is listed in
//! `data.searched` with whether it held an instance. Nothing is written, created or
//! followed (`kitkraft_io::Instance`).

use std::ffi::OsString;
use std::path::PathBuf;

use kitkraft_io::Instance;
use serde_json::{json, Value};

use super::testplan::DEV_PROFILE;
use super::{env_var_os, DATA_DIR_ENV};
use crate::envelope::{Coverage, CoverageStatus, Outcome};
use crate::exec::EngineError;

/// Names an MO2 instance root when `instance` is not given.
pub(crate) const MO2_DIR_ENV: &str = "KITKRAFT_MO2_DIR";

/// The owner's live-game profile (AGENTS.md): read-only input, never a test target.
const LIVE_PROFILE: &str = "Default";

/// The dev box's portable instance (AGENTS.md, "Windows specifics").
fn dev_box_instance() -> PathBuf {
    PathBuf::from(format!("C:{}", std::path::MAIN_SEPARATOR))
        .join("Modding")
        .join("MO2-Starfield")
}

/// One place an instance was looked for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    path: PathBuf,
    source: &'static str,
}

/// The places to look, in order. `instance` alone when given.
fn candidates(
    instance: Option<&str>,
    env: Option<OsString>,
    local_app_data: Option<PathBuf>,
) -> Vec<Candidate> {
    if let Some(i) = instance {
        return vec![Candidate {
            path: PathBuf::from(i),
            source: "argument",
        }];
    }
    let mut out = Vec::new();
    if let Some(e) = env.filter(|e| !e.is_empty()) {
        out.push(Candidate {
            path: PathBuf::from(e),
            source: MO2_DIR_ENV,
        });
    }
    if let Some(root) = local_app_data.map(|l| l.join("ModOrganizer")) {
        let mut global: Vec<PathBuf> = std::fs::read_dir(&root)
            .map(|rd| rd.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        global.sort();
        for path in global.into_iter().take(64) {
            let starfield = Instance::read(&path).is_ok_and(|i| {
                i.game_name
                    .as_deref()
                    .is_some_and(|g| g.eq_ignore_ascii_case("Starfield"))
            });
            if starfield {
                out.push(Candidate {
                    path,
                    source: "LOCALAPPDATA\\ModOrganizer",
                });
            }
        }
    }
    out.push(Candidate {
        path: dev_box_instance(),
        source: "dev box default",
    });
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

/// `profiles` with the environment passed in (tests run it without the process's).
fn report(
    args: &Value,
    mo2_env: Option<OsString>,
    local_app_data: Option<PathBuf>,
    data_env: Option<OsString>,
) -> Outcome {
    let arg = |n: &str| args.get(n).and_then(Value::as_str);
    let mut limitations = Vec::new();
    let mut searched = Vec::new();
    let mut found: Option<(Instance, &'static str)> = None;
    for c in candidates(arg("instance"), mo2_env, local_app_data) {
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
    Ok(report(
        args,
        env_var_os(MO2_DIR_ENV),
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
        env_var_os(DATA_DIR_ENV),
    ))
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
            Some(OsString::from("Z:/ignored")),
            None,
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

    #[test]
    fn detection_order_env_then_global_instances_then_the_dev_box() {
        let base = fixture("detect");
        let mo2 = base.join("MO2");
        // A global instance folder of another game is skipped.
        let local = base.join("Local");
        let other = local.join("ModOrganizer").join("Skyrim");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(
            other.join("ModOrganizer.ini"),
            "[General]\ngameName=Skyrim Special Edition\n",
        )
        .unwrap();
        let list = candidates(
            None,
            Some(mo2.clone().into_os_string()),
            Some(local.clone()),
        );
        let sources: Vec<&str> = list.iter().map(|c| c.source).collect();
        assert_eq!(sources, [MO2_DIR_ENV, "dev box default"]);

        let out = report(
            &json!({}),
            Some(mo2.clone().into_os_string()),
            Some(local),
            None,
        );
        assert_eq!(out.data["instance"]["found_by"], MO2_DIR_ENV);
        assert_eq!(out.data["searched"][0]["found"], true);

        // Nothing found: the places are listed and the data dir comes from the env.
        let none = report(
            &json!({"instance": base.join("nothing").display().to_string()}),
            None,
            None,
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
