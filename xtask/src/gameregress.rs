//! `cargo xtask game-regress` (M4.T7, V4, pipeline X): replays recorded game sessions.
//!
//! A replay is `<replays>/<name>.toml` (default `observe/replays/`, gitignored; `--replays`
//! names another folder, such as `C:\Modding\kitkraft-observe\replays`). It holds how to
//! rebuild the plugin (a `kit` argument vector writing `{out}`), the `kit testplan`
//! arguments, the plugin's and the probe file's SHA-256 from the session that passed, and
//! that session's ledger id; beside it, `<name>.probes.toml` is that session's probe file.
//!
//! The desk half, built here, rebuilds each plugin into `out/game-regress/<name>/`, runs
//! `kit testplan` on it, and compares the plugin's bytes (SHA-256) and the probe file row by
//! row (title, matcher, steps) with the recorded ones. Any difference fails the replay: a
//! recipe or builder change moved what the passing session checked. A wrong `FULL`, for
//! example, changes both.
//!
//! The live half, the session run twice on the same base save (M4.T7's "passes twice in a
//! row"), is not built: it would run only when `KITKRAFT_GAME_PROFILE` names the dev profile,
//! and until it is built every replay's live part is reported `not-run`. `cargo xtask ci`
//! does not call this task.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

use crate::evidence::{sha256_hex, KitBinary, Runner};

/// The environment variable that opts into the live half (M4.T7).
pub(crate) const PROFILE_VAR: &str = "KITKRAFT_GAME_PROFILE";

/// The `kit` commands a replay's `build` may run: the plugin writers. Anything else (a game
/// session, a deploy into MO2) is refused when the replay is read (review M4).
const WRITERS: [&str; 5] = ["build", "set", "new", "clone", "place"];

/// The `kit testplan` options a replay may pass, each with its value.
const TESTPLAN_OPTIONS: [&str; 2] = ["--plugins", "--profile"];

/// One replay file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Replay {
    pub name: String,
    pub plugin: String,
    pub build: Vec<String>,
    pub testplan: Vec<String>,
    pub plugin_sha256: String,
    pub probes_sha256: String,
    pub session: String,
}

fn text(t: &toml::Table, key: &str, file: &Path) -> Result<String> {
    t.get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{}: `{key}` (text) is missing", file.display()))
}

fn list(t: &toml::Table, key: &str, file: &Path) -> Result<Vec<String>> {
    let arr = t
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| anyhow!("{}: `{key}` (a list of text) is missing", file.display()))?;
    arr.iter()
        .map(|v| {
            v.as_str().map(str::to_string).ok_or_else(|| {
                anyhow!("{}: `{key}` holds a value that is not text", file.display())
            })
        })
        .collect()
}

impl Replay {
    /// Parses a replay file; the replay's name is the file stem.
    pub(crate) fn parse(file: &Path, src: &str) -> Result<Replay> {
        let t: toml::Table = src
            .parse()
            .with_context(|| format!("{}: not TOML", file.display()))?;
        if t.get("format").and_then(toml::Value::as_integer) != Some(1) {
            bail!("{}: `format = 1` is required", file.display());
        }
        let name = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .ok_or_else(|| anyhow!("{}: no file name", file.display()))?;
        let build = list(&t, "build", file)?;
        if !build.first().is_some_and(|c| WRITERS.contains(&c.as_str())) {
            bail!(
                "{}: `build` must start with a plugin writer ({}), not {:?}",
                file.display(),
                WRITERS.join(", "),
                build.first()
            );
        }
        if !build.iter().any(|a| a.contains("{out}")) {
            bail!("{}: `build` must write `{{out}}`", file.display());
        }
        let plugin = text(&t, "plugin", file)?;
        if plugin.contains(['/', '\\']) || plugin.contains("..") || plugin.is_empty() {
            bail!("{}: `plugin` is a file name, not a path", file.display());
        }
        let testplan = list(&t, "testplan", file)?;
        let mut it = testplan.iter();
        while let Some(opt) = it.next() {
            let value = it.next();
            if !TESTPLAN_OPTIONS.contains(&opt.as_str())
                || value.is_none_or(|v| v.starts_with("--"))
            {
                bail!(
                    "{}: `testplan` takes only {} with a value each, not `{opt}`",
                    file.display(),
                    TESTPLAN_OPTIONS.join(" and ")
                );
            }
        }
        Ok(Replay {
            name,
            plugin,
            build,
            testplan,
            plugin_sha256: text(&t, "plugin_sha256", file)?.to_ascii_lowercase(),
            probes_sha256: text(&t, "probes_sha256", file)?.to_ascii_lowercase(),
            session: text(&t, "session", file)?,
        })
    }
}

/// What one replay found.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Outcome {
    pub name: String,
    /// Differences that fail the replay.
    pub failures: Vec<String>,
    /// Lines for the report (hashes, rows compared).
    pub notes: Vec<String>,
}

/// The probe rows of a probe file, `row -> {title, matcher, step}` (the header's notes and
/// `sha256` are covered by the plugin's own hash).
fn rows(src: &str, what: &str) -> Result<BTreeMap<String, Value>> {
    let t: toml::Table = src.parse().with_context(|| format!("{what}: not TOML"))?;
    let mut out = BTreeMap::new();
    let probes = t
        .get("probe")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for p in probes {
        let Some(row) = p.get("row").and_then(toml::Value::as_str) else {
            bail!("{what}: a probe without a row");
        };
        let keep = serde_json::json!({
            "title": p.get("title").and_then(toml::Value::as_str),
            "matcher": p.get("matcher").and_then(toml::Value::as_str),
            "step": serde_json::to_value(p.get("step")).unwrap_or(Value::Null),
        });
        out.insert(row.to_string(), keep);
    }
    Ok(out)
}

/// The rows that differ between the recorded probe file and the regenerated one.
pub(crate) fn compare_rows(recorded: &str, now: &str) -> Result<Vec<String>> {
    let a = rows(recorded, "the recorded probe file")?;
    let b = rows(now, "the regenerated probe file")?;
    let mut diffs = Vec::new();
    for (row, old) in &a {
        match b.get(row) {
            None => diffs.push(format!("{row}: gone from the regenerated plan")),
            Some(new) if new != old => diffs.push(format!(
                "{row}: changed ({} -> {})",
                old.get("title").and_then(Value::as_str).unwrap_or("-"),
                new.get("title").and_then(Value::as_str).unwrap_or("-")
            )),
            Some(_) => {}
        }
    }
    for row in b.keys().filter(|r| !a.contains_key(*r)) {
        diffs.push(format!("{row}: new in the regenerated plan"));
    }
    Ok(diffs)
}

/// The workspace root of a work folder `<root>/out/game-regress/<name>`.
fn root_of(work: &Path) -> &Path {
    work.ancestors().nth(3).unwrap_or(work)
}

/// `arg` made absolute when it names a file or folder under `root` (`kit` runs in its own
/// folder when `KITKRAFT_KIT` names it, so a spec path like `docs/poi/first-poi.toml` is
/// resolved here).
fn absolute(root: &Path, arg: String) -> String {
    let p = root.join(&arg);
    if !arg.starts_with('-') && Path::new(&arg).is_relative() && p.exists() {
        p.display().to_string()
    } else {
        arg
    }
}

/// The desk half of one replay: rebuild, testplan, compare.
pub(crate) fn replay_desk(
    r: &Replay,
    recorded_probes: &str,
    work: &Path,
    kit: &dyn Runner,
) -> Result<Outcome> {
    let mut o = Outcome {
        name: r.name.clone(),
        ..Outcome::default()
    };
    std::fs::create_dir_all(work).with_context(|| format!("{}", work.display()))?;
    let cache = work.join("cache");
    let plugin = work.join(&r.plugin);
    let out = plugin.display().to_string();
    let args: Vec<String> = r
        .build
        .iter()
        .map(|a| a.replace("{out}", &out))
        .map(|a| absolute(root_of(work), a))
        .collect();
    let built = kit.kit(&args, &cache)?;
    if let Some(e) = built.error() {
        o.failures.push(format!("rebuild failed: {e}"));
        return Ok(o);
    }
    let bytes = std::fs::read(&plugin).with_context(|| format!("{out}: not written"))?;
    let sha = sha256_hex(&bytes);
    if sha == r.plugin_sha256 {
        o.notes
            .push(format!("plugin {} {sha} as recorded", r.plugin));
    } else {
        o.failures.push(format!(
            "plugin {} changed: {sha}; the passing session ({}) had {}",
            r.plugin, r.session, r.plugin_sha256
        ));
    }
    let plan = work.join("plan");
    let mut targs = vec!["testplan".to_string(), out, plan.display().to_string()];
    targs.extend(r.testplan.iter().cloned());
    let tp = kit.kit(&targs, &cache)?;
    if let Some(e) = tp.error() {
        o.failures.push(format!("testplan failed: {e}"));
        return Ok(o);
    }
    let now = std::fs::read_to_string(plan.join("probes.toml"))
        .with_context(|| format!("{}: no probes.toml", plan.display()))?;
    let psha = sha256_hex(now.as_bytes());
    if psha == r.probes_sha256 {
        o.notes.push(format!("probes.toml {psha} as recorded"));
    } else {
        o.notes
            .push(format!("probes.toml {psha}, recorded {}", r.probes_sha256));
    }
    let diffs = compare_rows(recorded_probes, &now)?;
    if diffs.is_empty() {
        o.notes
            .push("every probe row as in the passing session".into());
    } else {
        o.failures
            .extend(diffs.into_iter().map(|d| format!("probe row {d}")));
    }
    Ok(o)
}

/// Every replay in `dir` with its recorded probe file, in name order.
fn read_all(dir: &Path) -> Result<Vec<(Replay, String)>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("{}: no replays folder", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|x| x == "toml")
                && !p.to_string_lossy().ends_with(".probes.toml")
        })
        .collect();
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(&f).with_context(|| format!("{}", f.display()))?;
        let r = Replay::parse(&f, &src)?;
        let probes = f.with_file_name(format!("{}.probes.toml", r.name));
        let recorded = std::fs::read_to_string(&probes)
            .with_context(|| format!("{}: the recorded probe file is missing", probes.display()))?;
        out.push((r, recorded));
    }
    Ok(out)
}

/// Every replay in `dir`, in name order; each works in `<root>/out/game-regress/<name>/`.
pub(crate) fn run_all(root: &Path, dir: &Path, kit: &dyn Runner) -> Result<Vec<Outcome>> {
    let mut out = Vec::new();
    for (r, recorded) in read_all(dir)? {
        let work = root.join("out").join("game-regress").join(&r.name);
        out.push(replay_desk(&r, &recorded, &work, kit)?);
    }
    Ok(out)
}

/// `cargo xtask game-regress [--replays <dir>] [--dry-run]`.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let mut replays = root.join("observe").join("replays");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--replays" => {
                replays = PathBuf::from(
                    it.next()
                        .ok_or_else(|| anyhow!("--replays needs a folder"))?,
                )
            }
            "--dry-run" => {}
            "--help" | "-h" => {
                println!("cargo xtask game-regress [--replays <dir>] [--dry-run]");
                println!(
                    "  Rebuilds each replay's plugin, reruns kit testplan, and fails on a changed"
                );
                println!(
                    "  plugin or probe row (the desk half of M4.T7). The live half (the session"
                );
                println!(
                    "  twice on the base save) needs {PROFILE_VAR} and is not built yet: not-run."
                );
                return Ok(());
            }
            other => bail!("game-regress: unknown argument `{other}`"),
        }
    }
    let kit = KitBinary::from_env(root);
    let outcomes = run_all(root, &replays, &kit)?;
    let failed = outcomes.iter().filter(|o| !o.failures.is_empty()).count();
    let live = std::env::var(PROFILE_VAR).ok();
    for o in &outcomes {
        let verdict = if o.failures.is_empty() {
            "pass"
        } else {
            "FAIL"
        };
        println!("{}: desk {verdict}", o.name);
        for n in &o.notes {
            println!("  {n}");
        }
        for f in &o.failures {
            println!("  FAIL {f}");
        }
        match &live {
            Some(p) => println!(
                "  live: not-run ({PROFILE_VAR}={p} is set, but the live replay is not built yet, M4.T7)"
            ),
            None => println!("  live: not-run ({PROFILE_VAR} is not set)"),
        }
    }
    println!(
        "game-regress: {} replay(s), {failed} failed at the desk",
        outcomes.len()
    );
    if failed > 0 {
        bail!("{failed} replay(s) changed what their passing session checked");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::evidence::KitOut;
    use std::cell::RefCell;

    /// A fake `kit`: `set` writes the FULL it is given as the plugin's bytes; `testplan`
    /// writes a probe file whose one row searches for that FULL.
    struct Fake {
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl Runner for Fake {
        fn kit(&self, args: &[String], _cache: &Path) -> Result<KitOut> {
            self.calls.borrow_mut().push(args.to_vec());
            let ok = KitOut {
                code: Some(0),
                json: serde_json::json!({"data": {}}),
            };
            match args.first().map(String::as_str) {
                Some("set") => {
                    std::fs::write(&args[3], args[2].as_bytes())?;
                    Ok(ok)
                }
                Some("testplan") => {
                    let full = std::fs::read_to_string(&args[1])?;
                    std::fs::create_dir_all(&args[2])?;
                    std::fs::write(Path::new(&args[2]).join("probes.toml"), probes(&full))?;
                    Ok(ok)
                }
                _ => Ok(KitOut {
                    code: Some(2),
                    json: serde_json::json!({"error": {"message": "unknown"}}),
                }),
            }
        }
    }

    fn probes(full: &str) -> String {
        format!(
            "format = 1\n\n[[probe]]\nrow = \"T-01\"\ntitle = 'help \"{full}\" 4'\n\n[[probe.step]]\ncommand = 'help \"{full}\" 4'\nmatcher = \"contains\"\nexpected = \"0021BBCD\"\n"
        )
    }

    /// A replay of the rename whose passing session had the right FULL; `full` is what the
    /// replay's build now writes.
    fn replay(dir: &Path, full: &str) {
        let good = "Old Soviet Marksman Rifle";
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("rename.toml"),
            format!(
                "format = 1\nplugin = \"Rename.esm\"\nbuild = [\"set\", \"Starfield.esm:0021BBCD\", \"{full}\", \"{{out}}\"]\n\
                 testplan = [\"--plugins\", \"Starfield.esm\"]\nplugin_sha256 = \"{}\"\nprobes_sha256 = \"{}\"\nsession = \"game-20261102-02\"\n",
                sha256_hex(good.as_bytes()),
                sha256_hex(probes(good).as_bytes())
            ),
        )
        .unwrap();
        std::fs::write(dir.join("rename.probes.toml"), probes(good)).unwrap();
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kk-game-regress-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn an_unchanged_replay_passes_at_the_desk() {
        let root = tmp("pass");
        replay(&root.join("replays"), "Old Soviet Marksman Rifle");
        let fake = Fake {
            calls: RefCell::new(Vec::new()),
        };
        let out = run_all(&root, &root.join("replays"), &fake).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].failures.is_empty(), "{:?}", out[0]);
        assert!(out[0].notes.iter().any(|n| n.contains("every probe row")));
        assert_eq!(fake.calls.borrow().len(), 2, "one rebuild, one testplan");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// M4.T7's deliberate break: a wrong FULL fails the replay (the plugin and its row).
    #[test]
    fn a_wrong_full_fails_the_replay() {
        let root = tmp("break");
        replay(&root.join("replays"), "Old Soviet Marksman Rifel");
        let fake = Fake {
            calls: RefCell::new(Vec::new()),
        };
        let out = run_all(&root, &root.join("replays"), &fake).unwrap();
        let f = &out[0].failures;
        assert!(
            f.iter().any(|x| x.contains("plugin Rename.esm changed")),
            "{f:?}"
        );
        assert!(
            f.iter().any(|x| x.contains("probe row T-01: changed")),
            "{f:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_replay_that_writes_nowhere_or_names_a_path_is_refused() {
        let p = Path::new("x.toml");
        let base =
            "format = 1\ntestplan = []\nplugin_sha256 = \"a\"\nprobes_sha256 = \"b\"\nsession = \"s\"\n";
        let e = Replay::parse(p, &format!("{base}plugin = \"P.esm\"\nbuild = [\"set\"]\n"))
            .unwrap_err();
        assert!(e.to_string().contains("{out}"), "{e}");
        let e = Replay::parse(
            p,
            &format!("{base}plugin = '..\\P.esm'\nbuild = [\"set\", \"{{out}}\"]\n"),
        )
        .unwrap_err();
        assert!(e.to_string().contains("file name"), "{e}");
        let e = Replay::parse(p, "format = 2\n").unwrap_err();
        assert!(e.to_string().contains("format = 1"), "{e}");
    }

    /// Review M4: a replay can only rebuild a plugin and plan it; a command that launches the
    /// game or writes into MO2, or another testplan argument, is refused.
    #[test]
    fn a_replay_runs_only_plugin_writers_and_plain_testplan_arguments() {
        let p = Path::new("x.toml");
        let base = "format = 1
plugin = \"P.esm\"
plugin_sha256 = \"a\"
probes_sha256 = \"b\"
session = \"s\"
";
        for build in [
            r#"["game", "session", "{out}"]"#,
            r#"["deploy", "{out}", 'C:\x', "--dev"]"#,
            r#"["deploy", "enable", "{out}", "--dev"]"#,
        ] {
            let e = Replay::parse(
                p,
                &format!(
                    "{base}testplan = []
build = {build}
"
                ),
            )
            .unwrap_err();
            assert!(e.to_string().contains("plugin writer"), "{build}: {e}");
        }
        let e = Replay::parse(
            p,
            &format!(
                "{base}build = [\"set\", \"{{out}}\"]
testplan = [\"--mo2\", 'C:\\x']
"
            ),
        )
        .unwrap_err();
        assert!(e.to_string().contains("testplan"), "{e}");
        assert!(Replay::parse(
            p,
            &format!(
                "{base}build = [\"build\", \"s.toml\", \"{{out}}\"]
testplan = [\"--profile\", \"P\", \"--plugins\", \"Starfield.esm\"]
"
            ),
        )
        .is_ok());
    }

    #[test]
    fn rows_that_go_or_appear_are_differences() {
        let a = probes("A");
        let b = "format = 1\n\n[[probe]]\nrow = \"T-02\"\ntitle = 'x'\n";
        let d = compare_rows(&a, b).unwrap();
        assert!(d.iter().any(|x| x.starts_with("T-01: gone")), "{d:?}");
        assert!(d.iter().any(|x| x.starts_with("T-02: new")), "{d:?}");
    }
}
