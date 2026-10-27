//! Fakes for the front ends' game tests: [`GameFakes`] stands up a fixture MO2 instance
//! under `std::env::temp_dir()`, a fake `/console` HTTP endpoint and a fake process host,
//! so the workbench and the MCP server can test their game buttons and tools through the
//! catalog against the same fakes the engine's own tests use (`kitkraft_game::testing`).
//! Nothing in the catalog reaches this module; it exists for tests alone.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kitkraft_game::host::Host;
use kitkraft_game::probes::{Matcher, Probe, ProbeFile, Step, FORMAT};
use kitkraft_game::testing::{mo2_fixture, FakeConsole, FakeHost, Fixture, FIXTURE_SAVE};
use kitkraft_game::GameProfile;
use serde_json::{json, Value};

use crate::Engine;

/// The process host sessions use; tests put a fake one in the session.
#[derive(Clone)]
pub(crate) struct GameHost(pub(crate) Arc<dyn Host>);

/// A fixture MO2 instance, a fake `/console` endpoint and a fake process host, everything
/// under one folder under `std::env::temp_dir()` that is removed when the fakes are
/// dropped.
#[derive(Debug)]
pub struct GameFakes {
    root: PathBuf,
    fixture: Fixture,
    /// The fake /console endpoint; never read after setup, the field keeps the
    /// server thread alive until the fakes drop.
    #[allow(dead_code)]
    console: FakeConsole,
    host: Arc<FakeHost>,
    probes: PathBuf,
    game: PathBuf,
}

impl GameFakes {
    /// Sets up the fakes; `tag` separates the folders of parallel tests.
    pub fn new(tag: &str) -> Result<GameFakes, String> {
        let root = std::env::temp_dir().join(format!(
            "kitkraft-engine-testing-{tag}-{}",
            std::process::id()
        ));
        let fixture = mo2_fixture(&root).map_err(|e| e.to_string())?;
        let console = FakeConsole::start().map_err(|e| format!("fake /console endpoint: {e}"))?;
        console.answer("GetSFSEVersion", "SFSE 0.2.21");
        console.answer("help KitKraftCrate", "REFR KitKraftCrate (04001000)");
        console.answer("getpos x", "GetPos >> -17.00");
        let host = Arc::new(FakeHost::new(
            &["explorer.exe"],
            &["ModOrganizer.exe", "sfse_loader.exe", "Starfield.exe"],
        ));
        let probes = probes_file(&fixture)?;
        let game = game_file(&fixture, &console)?;
        Ok(GameFakes {
            root,
            fixture,
            console,
            host,
            probes,
            game,
        })
    }

    /// Puts the fake host in the engine's session, so its `game.*` commands run on the
    /// fakes.
    pub fn install(&self, engine: &Engine) {
        let host = Arc::clone(&self.host);
        let host: Arc<dyn Host> = host;
        engine.with_session(|s| s.insert(GameHost(host)));
    }

    /// The arguments `game.session` takes on these fakes, with the object members of
    /// `extra` merged in (`start`, `dry_run`, ...).
    pub fn session_args(&self, extra: Value) -> Value {
        let mut a = json!({
            "mo2": self.fixture.instance.display().to_string(),
            "probes": self.probes.display().to_string(),
            "game": self.game.display().to_string(),
            "profile": "KitKraft Dev",
            "base_save_sha256": crate::deploy::sha256::hex(FIXTURE_SAVE),
            "observe": self.fixture.observe_root.display().to_string(),
            "ledger": self.fixture.ledger.display().to_string(),
        });
        if let (Some(extra), Some(args)) = (extra.as_object(), a.as_object_mut()) {
            for (k, v) in extra {
                args.insert(k.to_string(), v.clone());
            }
        }
        a
    }

    /// The folder holding everything (removed when the fakes are dropped).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The fixture MO2 instance root.
    pub fn instance(&self) -> &Path {
        &self.fixture.instance
    }

    /// The observe folder the sessions write into.
    pub fn observe(&self) -> &Path {
        &self.fixture.observe_root
    }

    /// The ledger the sessions append to.
    pub fn ledger(&self) -> &Path {
        &self.fixture.ledger
    }

    /// How many launches the fake host saw.
    pub fn launched(&self) -> usize {
        self.host.launched().len()
    }
}

impl Drop for GameFakes {
    fn drop(&mut self) {
        // The fake /console server stops when the console is dropped.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The probes file: the engine tests' two rows plus a manual one the controller reports
/// `inconclusive` with its screenshot.
fn probes_file(f: &Fixture) -> Result<PathBuf, String> {
    let step = |c: &str, m: Option<Matcher>, e: &str, t: Option<f64>| Step {
        command: c.into(),
        matcher: m,
        expected: (!e.is_empty()).then(|| e.into()),
        tolerance: t,
        why: None,
    };
    let p = ProbeFile {
        format: FORMAT,
        plugin: "X.esm".into(),
        sha256: crate::deploy::sha256::hex(b"TES4 fixture"),
        generated_by: "kitkraft-engine testing".into(),
        profile: "KitKraft Dev".into(),
        runtime_id_basis: "inferred".into(),
        notes: Vec::new(),
        probes: vec![
            Probe {
                row: "T-01".into(),
                title: "help".into(),
                forms: vec!["X.esm:001000".into()],
                screenshot: false,
                matcher: None,
                reason: None,
                steps: vec![step(
                    "help KitKraftCrate 4",
                    Some(Matcher::Contains),
                    "04001000",
                    None,
                )],
            },
            Probe {
                row: "T-02".into(),
                title: "getpos".into(),
                forms: vec![],
                screenshot: true,
                matcher: None,
                reason: None,
                steps: vec![
                    step("prid KitKraftCrate", None, "", None),
                    step("getpos x", Some(Matcher::Number), "-17", Some(0.5)),
                ],
            },
            Probe {
                row: "T-03".into(),
                title: "manual".into(),
                forms: vec![],
                screenshot: true,
                matcher: Some("manual".into()),
                reason: Some("no console command can answer this row".into()),
                steps: Vec::new(),
            },
        ],
    };
    let path = f.root.join("probes.toml");
    std::fs::write(&path, p.to_toml().map_err(|e| e.to_string())?)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

/// A Starfield game profile with evidence filled in, the fake endpoint's URL, short
/// timeouts and logs inside the fixture.
fn game_file(f: &Fixture, c: &FakeConsole) -> Result<PathBuf, String> {
    let mut g = GameProfile::named("starfield").map_err(|e| e.to_string())?;
    g.launch.evidence = "fixture".into();
    g.console.evidence = "fixture".into();
    g.saves.evidence = "fixture".into();
    g.console.url = c.url();
    g.timeouts.probe_secs = 3;
    g.timeouts.ready_secs = 3;
    g.timeouts.load_secs = 3;
    g.timeouts.exit_secs = 1;
    g.timeouts.session_secs = 60;
    for l in &mut g.logs {
        l.path = f.root.join(format!("{}.log", l.name)).display().to_string();
    }
    let path = f.root.join("test-game.toml");
    std::fs::write(&path, toml::to_string(&g).map_err(|e| e.to_string())?)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::arithmetic_side_effects)]

    use std::path::Path;
    use std::time::Duration;

    use serde_json::json;

    use super::GameFakes;
    use crate::Engine;

    #[test]
    fn fakes_run_a_started_session_through_the_catalog() {
        let fakes = GameFakes::new("started").unwrap();
        let engine = Engine::with_default_handlers();
        fakes.install(&engine);
        let r = engine
            .execute("game.session", fakes.session_args(json!({"start": true})))
            .unwrap();
        assert_eq!(r["data"]["started"], json!(true), "{r}");
        let id = r["data"]["session"].as_str().unwrap().to_string();
        let mut polls = 0;
        loop {
            let s = engine.execute("game.status", json!({"id": id})).unwrap();
            polls += 1;
            if s["data"]["phase"] == json!("idle") {
                let rows = s["data"]["rows"].as_array().unwrap();
                assert_eq!(rows.len(), 3, "{rows:?}");
                let verdict = |row: &str| {
                    rows.iter().find(|x| x["row"] == json!(row)).unwrap()["verdict"]["status"]
                        .clone()
                };
                assert_eq!(verdict("T-01"), json!("pass"));
                assert_eq!(verdict("T-02"), json!("pass"));
                assert_eq!(verdict("T-03"), json!("inconclusive"));
                let shots = s["data"]["screenshots"].as_array().unwrap();
                assert!(
                    shots
                        .iter()
                        .any(|x| Path::new(x["path"].as_str().unwrap()).is_file()),
                    "{shots:?}"
                );
                break;
            }
            assert!(polls < 400, "{s}");
            std::thread::sleep(Duration::from_millis(25));
        }
        let stopped = engine.execute("game.stop", json!({"id": id})).unwrap();
        assert!(
            stopped["data"]["report"]["ledger_entry"].as_str().is_some(),
            "{stopped}"
        );
        assert_eq!(fakes.launched(), 1);
    }

    #[test]
    fn fakes_refuse_the_live_profile_and_clean_up() {
        let fakes = GameFakes::new("refuse").unwrap();
        let engine = Engine::with_default_handlers();
        fakes.install(&engine);
        let dry = engine
            .execute(
                "game.session",
                fakes.session_args(json!({"profile": "Default", "dry_run": true})),
            )
            .unwrap();
        assert_eq!(dry["data"]["would_start"], json!(false), "{dry}");
        let refusals = dry["data"]["refusals"].as_array().unwrap();
        assert_eq!(
            refusals
                .iter()
                .filter(|x| x["rule"] == json!("live-profile"))
                .count(),
            1,
            "{refusals:?}"
        );
        let err = engine
            .execute(
                "game.session",
                fakes.session_args(json!({"profile": "Default"})),
            )
            .unwrap_err()
            .to_string();
        assert!(err.contains("refused [live-profile]"), "{err}");
        let root = fakes.root().to_path_buf();
        drop(fakes);
        assert!(!root.exists(), "{root:?}");
    }
}
