//! `cargo xtask oracle` (M3.H4): external tools run as oracles on our fixtures (lane H).
//!
//! `cargo xtask oracle spriggit [--official] [--synthetic] [<plugin>...] [--out <dir>] [--keep]`
//! runs a Spriggit round trip (plugin -> YAML -> plugin) on each fixture in a temp folder and
//! compares the input with Spriggit's output using our own tools only: SHA-256, a positional
//! byte diff, a `TES4` diff, `kit diff`, `kit roundtrip --verify` on the output, and an
//! inventory comparison from `kit coverage` on both (records per type, subrecord occurrences
//! per record type and signature). **The YAML is never opened**: Spriggit writes it into a
//! folder that is handed straight back to Spriggit and then deleted (ADR 0001 §4, ADR 0005
//! §3: Spriggit's field names and key structure are never used). Reports go to `--out`
//! (default `observe/oracle/spriggit/`, gitignored); `docs/oracles.md` records the results
//! in our own words.
//!
//! Fixture sets: `--official` the five smallest official files (read from
//! `KITKRAFT_DATA_DIR` and copied; the originals are never written), `--synthetic` the
//! plugins built here byte by byte plus overrides written by our own `kit set`, and any
//! plugin paths given.
//!
//! Tools: `KITKRAFT_SPRIGGIT` names `Spriggit.CLI.exe` or a PowerShell wrapper (`.ps1`, run
//! with `powershell -NoProfile -File`); the Starfield translation package version is
//! `KITKRAFT_SPRIGGIT_PACKAGE_VERSION`, else the CLI's own version. `cargo xtask oracle
//! sf1edit` is in [`crate::sf1edit`] (xEdit's console dumper; `KITKRAFT_SF1EDIT`).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::evidence::{
    arg, byte_diff, sha256_hex, strip_volatile, table, tes4, tes4_diff, KitBinary, KitOut, Runner,
};

const SPRIGGIT_VAR: &str = "KITKRAFT_SPRIGGIT";
const SPRIGGIT_PACKAGE_VAR: &str = "KITKRAFT_SPRIGGIT_PACKAGE_VERSION";
const DATA_DIR_VAR: &str = "KITKRAFT_DATA_DIR";
/// Spriggit's Starfield translation package.
const PACKAGE: &str = "Spriggit.Yaml.Starfield";
/// The five smallest official files (corpus/manifest.toml sizes).
pub(crate) const OFFICIAL_SMALL: [&str; 5] = [
    "OldMars.esm",
    "SFBGS007.esm",
    "Constellation.esm",
    "SFBGS004.esm",
    "SFBGS047.esm",
];
const RANGE_CAP: usize = 32;

/// `cargo xtask oracle ...`.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("spriggit") => {}
        Some("sf1edit") => {
            return crate::sf1edit::run(root, args.get(1..).unwrap_or_default());
        }
        Some("papyrus") => {
            return crate::papyrus::run(root, args.get(1..).unwrap_or_default());
        }
        // Lane A (M5.A2): ours against the CK's export of the same content.
        Some("ck") => {
            return crate::ckoracle::run(root, args.get(1..).unwrap_or_default());
        }
        _ => {
            println!(
                "cargo xtask oracle spriggit [--official] [--synthetic] [<plugin>...] [--out <dir>] [--keep]"
            );
            println!(
                "cargo xtask oracle sf1edit [--official] [--synthetic] [<plugin>...] [--types SIG,..] [--sample N] [--no-check] [--timeout-mins N] [--out <dir>] [--keep]"
            );
            println!(
                "cargo xtask oracle papyrus [<psc>...] [--sources <Scripts\\Source>] [--optimize] [--out <dir>] [--keep]"
            );
            println!("cargo xtask oracle ck <fixture-dir>... [--out <dir>] [--check]");
            println!();
            println!(
                "  {SPRIGGIT_VAR}: Spriggit.CLI.exe or a .ps1 wrapper; {DATA_DIR_VAR}: masters"
            );
            println!(
                "  {}: xEdit's xSFEdit64.exe (its xDump64.exe is run) or xDump64.exe",
                crate::sf1edit::SF1EDIT_VAR
            );
            return Ok(());
        }
    }
    let mut official = false;
    let mut synthetic = false;
    let mut keep = false;
    let mut out = root.join("observe").join("oracle").join("spriggit");
    let mut given = Vec::new();
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--official" => official = true,
            "--synthetic" => synthetic = true,
            "--keep" => keep = true,
            "--out" => out = PathBuf::from(it.next().context("--out needs a directory")?),
            flag if flag.starts_with("--") => bail!("oracle spriggit: unknown flag `{flag}`"),
            path => given.push(PathBuf::from(path)),
        }
    }
    if !official && !synthetic && given.is_empty() {
        bail!("oracle spriggit: name plugins, or pass --official and/or --synthetic");
    }
    let tool_path = crate::env_var_os(SPRIGGIT_VAR)
        .map(PathBuf::from)
        .with_context(|| format!("set {SPRIGGIT_VAR} to Spriggit.CLI.exe or its .ps1 wrapper"))?;
    let data_dir = crate::env_var_os(DATA_DIR_VAR)
        .map(PathBuf::from)
        .with_context(|| format!("set {DATA_DIR_VAR} (Spriggit looks masters up there)"))?;
    let tool = SpriggitCli { path: tool_path };
    let kit = KitBinary::from_env(root);
    let work = std::env::temp_dir().join(format!("kitkraft-oracle-{}", std::process::id()));
    let mut inputs: Vec<Input> = Vec::new();
    if official {
        for name in OFFICIAL_SMALL {
            inputs.push(Input {
                path: data_dir.join(name),
                set: "official",
            });
        }
    }
    if synthetic {
        let dir = work.join("synthetic");
        inputs.extend(write_synthetic(&dir, &kit, &work.join("cache"))?);
    }
    for p in given {
        inputs.push(Input {
            path: p,
            set: "given",
        });
    }
    let opts = Options {
        data_dir,
        work: work.clone(),
        keep,
    };
    let report = spriggit(&tool, &kit, &inputs, &opts)?;
    fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    let mut text = serde_json::to_string_pretty(&report)?;
    text.push('\n');
    fs::write(out.join("report.json"), text)?;
    fs::write(out.join("report.md"), render(&report))?;
    if !keep {
        let _ = fs::remove_dir_all(&work);
    }
    println!("oracle: {}", out.join("report.md").display());
    Ok(())
}

/// One fixture for the round trip.
pub(crate) struct Input {
    pub path: PathBuf,
    /// `official`, `synthetic`, `kit`, or `given`.
    pub set: &'static str,
}

pub(crate) struct Options {
    pub data_dir: PathBuf,
    pub work: PathBuf,
    pub keep: bool,
}

/// What an external program printed.
pub(crate) struct ToolOut {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs Spriggit with arguments. Tests use a fake.
pub(crate) trait Tool {
    fn run(&self, args: &[String]) -> Result<ToolOut>;
}

struct SpriggitCli {
    path: PathBuf,
}

impl Tool for SpriggitCli {
    fn run(&self, args: &[String]) -> Result<ToolOut> {
        let is_ps1 = self
            .path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ps1"));
        let mut cmd = if is_ps1 {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-NonInteractive", "-File"]);
            c.arg(&self.path);
            c
        } else {
            Command::new(&self.path)
        };
        let out = cmd
            .args(args)
            .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
            .output()
            .with_context(|| format!("could not start {}", self.path.display()))?;
        Ok(ToolOut {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// The tool's version line and the package version to pass with `-v`.
fn versions(tool: &dyn Tool) -> Result<(String, String)> {
    let out = tool.run(&["--version".to_string()])?;
    let all = format!("{}\n{}", out.stdout, out.stderr);
    let line = all
        .lines()
        .map(str::trim)
        .find(|l| l.to_ascii_lowercase().contains("version") || l.contains('+'))
        .unwrap_or_default()
        .to_string();
    let semver = line
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|w| w.split('.').count() == 3 && w.split('.').all(|p| !p.is_empty()))
        .unwrap_or_default()
        .to_string();
    let package = crate::env_var_os(SPRIGGIT_PACKAGE_VAR)
        .and_then(|v| v.into_string().ok())
        .unwrap_or(semver);
    if package.is_empty() {
        bail!("could not read Spriggit's version from `--version`; set {SPRIGGIT_PACKAGE_VAR}");
    }
    Ok((line, package))
}

/// The last non-empty line of a tool's output, shortened (for failures only).
fn last_line(out: &ToolOut) -> String {
    let all = format!("{}\n{}", out.stdout, out.stderr);
    let line = all
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or_default();
    line.chars().take(200).collect()
}

/// Inventory from a `kit coverage` result: records per type, and subrecord occurrences per
/// `TYPE/SIG`.
fn inventory(out: &KitOut) -> (BTreeMap<String, u64>, BTreeMap<String, u64>) {
    let mut types = BTreeMap::new();
    let mut subs = BTreeMap::new();
    for t in out
        .data()
        .get("record_types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let sig = t
            .get("signature")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let n = t.get("records").and_then(Value::as_u64).unwrap_or(0);
        types.insert(sig.clone(), n);
        for s in t
            .get("signatures")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let sub = s.get("signature").and_then(Value::as_str).unwrap_or("?");
            let count = s.get("count").and_then(Value::as_u64).unwrap_or(0);
            subs.insert(format!("{sig}/{sub}"), count);
        }
    }
    (types, subs)
}

/// Keys whose counts differ between `a` and `b`, as `{key, in, out}`.
fn count_diff(a: &BTreeMap<String, u64>, b: &BTreeMap<String, u64>) -> Vec<Value> {
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|k| {
            let (x, y) = (
                a.get(k).copied().unwrap_or(0),
                b.get(k).copied().unwrap_or(0),
            );
            (x != y).then(|| json!({"key": k, "in": x, "out": y}))
        })
        .collect()
}

/// `kit diff` reduced to counts and, per `TYPE/SIG change`, how many subrecords changed.
fn diff_summary(out: &KitOut) -> Value {
    if let Some(e) = out.error() {
        return json!({"error": e});
    }
    let d = out.data();
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut header_only = 0u64;
    for ch in d
        .get("changed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let rec = ch.get("signature").and_then(Value::as_str).unwrap_or("?");
        let subs: Vec<&Value> = ch
            .get("subrecords")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .collect();
        if subs.is_empty() {
            header_only = header_only.saturating_add(1);
        }
        for s in subs {
            let key = format!(
                "{rec}/{} {}",
                s.get("signature").and_then(Value::as_str).unwrap_or("?"),
                s.get("change").and_then(Value::as_str).unwrap_or("?")
            );
            let n = kinds.entry(key).or_insert(0);
            *n = n.saturating_add(1);
        }
        let flags = ch.get("flags_differ") == Some(&Value::Bool(true));
        let order = ch.get("order_differs") == Some(&Value::Bool(true));
        for (on, key) in [(flags, "flags differ"), (order, "subrecord order differs")] {
            if on {
                let n = kinds.entry(format!("{rec} {key}")).or_insert(0);
                *n = n.saturating_add(1);
            }
        }
    }
    let forms = |key: &str| -> Vec<String> {
        d.get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|r| {
                format!(
                    "{} {}",
                    r.get("signature").and_then(Value::as_str).unwrap_or("?"),
                    r.get("form").and_then(Value::as_str).unwrap_or("?")
                )
            })
            .collect()
    };
    let mut v = json!({
        "counts": d.get("counts"),
        "keyed_by": d.get("keyed_by"),
        "added": forms("added"),
        "removed": forms("removed"),
        "changes": kinds,
        "changed_listed": d.get("changed").and_then(Value::as_array).map_or(0, Vec::len),
        "limitations": out.json.get("limitations"),
    });
    strip_volatile(&mut v);
    v
}

/// One fixture: copy in, serialize, deserialize, delete the YAML, compare.
fn one(
    tool: &dyn Tool,
    kit: &dyn Runner,
    package: &str,
    input: &Input,
    index: usize,
    opts: &Options,
) -> Result<Value> {
    let name = input
        .path
        .file_name()
        .context("fixture path has no file name")?
        .to_string_lossy()
        .into_owned();
    let dir = opts.work.join(format!("{index:02}"));
    let (in_dir, yaml_dir, out_dir) = (dir.join("in"), dir.join("yaml"), dir.join("out"));
    let cache = opts.work.join("cache");
    for d in [&in_dir, &out_dir, &cache] {
        fs::create_dir_all(d).with_context(|| format!("creating {}", d.display()))?;
    }
    let bytes_in =
        fs::read(&input.path).with_context(|| format!("reading {}", input.path.display()))?;
    let in_path = in_dir.join(&name);
    fs::write(&in_path, &bytes_in)?;
    let out_path = out_dir.join(&name);
    let data = arg(&opts.data_dir)?;

    let ser = tool.run(&[
        "serialize".to_string(),
        "-i".into(),
        arg(&in_path)?,
        "-o".into(),
        arg(&yaml_dir)?,
        "-g".into(),
        "Starfield".into(),
        "-p".into(),
        PACKAGE.into(),
        "-v".into(),
        package.into(),
        "-d".into(),
        data.clone(),
    ])?;
    let de = if ser.code == Some(0) {
        Some(tool.run(&[
            "deserialize".to_string(),
            "-i".into(),
            arg(&yaml_dir)?,
            "-o".into(),
            arg(&out_path)?,
            "-d".into(),
            data,
        ])?)
    } else {
        None
    };
    // The YAML is never read: delete it before looking at anything else.
    if yaml_dir.exists() {
        fs::remove_dir_all(&yaml_dir)
            .with_context(|| format!("deleting {}", yaml_dir.display()))?;
    }
    let step = |o: &ToolOut| {
        json!({
            "exit": o.code,
            "message": (o.code != Some(0)).then(|| last_line(o)),
        })
    };
    let mut row = json!({
        "plugin": name,
        "set": input.set,
        "source": input.path.to_string_lossy(),
        "bytes_in": bytes_in.len(),
        "sha256_in": sha256_hex(&bytes_in),
        "serialize": step(&ser),
        "deserialize": de.as_ref().map(step),
    });
    let in_cov = kit.kit(&["coverage".to_string(), arg(&in_path)?], &cache)?;
    let in_rt = kit.kit(
        &["roundtrip".to_string(), arg(&in_path)?, "--verify".into()],
        &cache,
    )?;
    let ours_identical = in_rt
        .data()
        .get("identical")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut extra = serde_json::Map::new();
    extra.insert("ours_roundtrip_identical".into(), json!(ours_identical));
    if out_path.is_file() {
        let bytes_out = fs::read(&out_path)?;
        let bd = byte_diff(&bytes_in, &bytes_out, RANGE_CAP);
        let tes4_lines = match (tes4(&bytes_in), tes4(&bytes_out)) {
            (Ok(a), Ok(b)) => json!(tes4_diff(&a, &b)),
            (a, b) => json!([format!(
                "TES4 not comparable: {} / {}",
                a.err().unwrap_or_default(),
                b.err().unwrap_or_default()
            )]),
        };
        let out_cov = kit.kit(&["coverage".to_string(), arg(&out_path)?], &cache)?;
        let out_rt = kit.kit(
            &["roundtrip".to_string(), arg(&out_path)?, "--verify".into()],
            &cache,
        )?;
        let (ti, si) = inventory(&in_cov);
        let (to, so) = inventory(&out_cov);
        let diff = if bd.identical {
            json!({"skipped": "identical bytes"})
        } else {
            diff_summary(&kit.kit(
                &[
                    "diff".to_string(),
                    arg(&in_path)?,
                    arg(&out_path)?,
                    "--limit".into(),
                    "100000".into(),
                ],
                &cache,
            )?)
        };
        extra.insert("bytes_out".into(), json!(bytes_out.len()));
        extra.insert("sha256_out".into(), json!(sha256_hex(&bytes_out)));
        extra.insert("identical".into(), json!(bd.identical));
        extra.insert("bytes".into(), bd.to_json());
        extra.insert("tes4".into(), tes4_lines);
        extra.insert(
            "out_parses".into(),
            json!(out_rt.error().is_none() && out_cov.error().is_none()),
        );
        extra.insert(
            "out_roundtrip_identical".into(),
            json!(out_rt.data().get("identical").and_then(Value::as_bool)),
        );
        extra.insert("record_types".into(), json!(count_diff(&ti, &to)));
        extra.insert("subrecords".into(), json!(count_diff(&si, &so)));
        extra.insert("kit_diff".into(), diff);
    } else {
        extra.insert("bytes_out".into(), Value::Null);
        extra.insert("identical".into(), json!(false));
    }
    if let Some(map) = row.as_object_mut() {
        map.extend(extra);
    }
    if !opts.keep {
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(row)
}

/// Runs the Spriggit round trip on every input; returns the JSON report.
pub(crate) fn spriggit(
    tool: &dyn Tool,
    kit: &dyn Runner,
    inputs: &[Input],
    opts: &Options,
) -> Result<Value> {
    let (version_line, package) = versions(tool)?;
    let mut rows = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        println!("oracle: spriggit {}", input.path.display());
        rows.push(one(tool, kit, &package, input, i, opts)?);
    }
    Ok(json!({
        "generated_by": "cargo xtask oracle spriggit (M3.H4)",
        "tool": {"name": "Spriggit CLI", "version": version_line, "package": PACKAGE, "package_version": package},
        "method": "plugin -> YAML -> plugin in a temp folder; the YAML is deleted unread; input and output compared with kit only",
        "fixtures": rows,
    }))
}

fn cell(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => "-".to_string(),
        Some(other) => other.to_string(),
    }
}

fn kv_list(v: Option<&Value>) -> String {
    let items: Vec<String> = v
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|x| {
            format!(
                "`{}` {} -> {}",
                cell(x, "key"),
                cell(x, "in"),
                cell(x, "out")
            )
        })
        .collect();
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join(", ")
    }
}

/// `report.md`.
fn render(r: &Value) -> String {
    let mut md = String::new();
    let tool = r.get("tool").cloned().unwrap_or(Value::Null);
    let _ = writeln!(md, "# Spriggit round trip report\n");
    let _ = writeln!(
        md,
        "Written by `cargo xtask oracle spriggit` (M3.H4). Tool: {} (package `{}` {}). {}.\n",
        cell(&tool, "version"),
        cell(&tool, "package"),
        cell(&tool, "package_version"),
        cell(r, "method")
    );
    let fixtures: Vec<&Value> = r
        .get("fixtures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    let rows: Vec<Vec<String>> = fixtures
        .iter()
        .map(|f| {
            let counts = f
                .pointer("/kit_diff/counts")
                .cloned()
                .unwrap_or(Value::Null);
            vec![
                format!("`{}`", cell(f, "plugin")),
                cell(f, "set"),
                format!(
                    "{} / {}",
                    f.pointer("/serialize/exit")
                        .map_or("-".into(), Value::to_string),
                    f.pointer("/deserialize/exit")
                        .map_or("-".into(), Value::to_string)
                ),
                format!("{} -> {}", cell(f, "bytes_in"), cell(f, "bytes_out")),
                cell(f, "identical"),
                cell(f, "ours_roundtrip_identical"),
                cell(f, "out_parses"),
                format!(
                    "{} / {} / {}",
                    cell(&counts, "added"),
                    cell(&counts, "removed"),
                    cell(&counts, "changed")
                ),
                kv_list(f.get("record_types")),
            ]
        })
        .collect();
    md.push_str(&table(
        &[
            "Fixture",
            "Set",
            "Exit (ser / de)",
            "Bytes in -> out",
            "Identical",
            "Ours identical",
            "Output parses (kit)",
            "Records added / removed / changed",
            "Record counts that differ",
        ],
        &rows,
    ));
    for f in fixtures {
        let _ = writeln!(md, "\n## `{}` ({})\n", cell(f, "plugin"), cell(f, "set"));
        let _ = writeln!(
            md,
            "- SHA-256: `{}` -> `{}`",
            cell(f, "sha256_in"),
            cell(f, "sha256_out")
        );
        for step in ["serialize", "deserialize"] {
            if let Some(m) = f
                .get(step)
                .and_then(|s| s.get("message"))
                .and_then(Value::as_str)
            {
                let _ = writeln!(md, "- {step} failed: {m}");
            }
        }
        if let Some(b) = f.get("bytes") {
            let _ = writeln!(
                md,
                "- Bytes: first difference {}, {} differing bytes in {} run(s)",
                b.get("first_difference")
                    .and_then(Value::as_u64)
                    .map_or("-".to_string(), |o| format!("0x{o:X}")),
                cell(b, "differing_bytes"),
                cell(b, "ranges_total")
            );
        }
        for line in f
            .get("tes4")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let _ = writeln!(md, "- TES4 {}", line.as_str().unwrap_or_default());
        }
        let _ = writeln!(
            md,
            "- Subrecord counts that differ: {}",
            kv_list(f.get("subrecords"))
        );
        if let Some(kd) = f.get("kit_diff") {
            for key in ["added", "removed"] {
                let items: Vec<&str> = kd
                    .get(key)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                if !items.is_empty() {
                    let _ = writeln!(md, "- Records {key}: {}", items.join(", "));
                }
            }
            if let Some(changes) = kd.get("changes").and_then(Value::as_object) {
                if !changes.is_empty() {
                    let list: Vec<String> =
                        changes.iter().map(|(k, v)| format!("`{k}` x{v}")).collect();
                    let _ = writeln!(md, "- Changes in changed records: {}", list.join(", "));
                }
            }
            if let Some(e) = kd.get("error") {
                let _ = writeln!(md, "- `kit diff` error: {e}");
            }
        }
    }
    md
}

// ---------------------------------------------------------------------------------------
// Synthetic fixtures.
//
// Container framing as in `crates/esm` (24-byte record and group headers, 6-byte subrecord
// headers, `u32` inflated size + zlib for a compressed record); the `TES4` values a new
// plugin of ours carries (`HEDR` 0.96 / record count / `0xFF001000`, form version 582,
// `CNAM` "DEFAULT", `INCC` 0) are the `docs/corpus.md` "Writing rules". The GLOB shape
// (`EDID`, `FLTV`) is the one the corpus GLOBs chosen in `docs/ck-observations.md` carry.

fn put_sub(out: &mut Vec<u8>, sig: &[u8; 4], data: &[u8]) -> Result<()> {
    out.extend_from_slice(sig);
    out.extend_from_slice(&u16::try_from(data.len())?.to_le_bytes());
    out.extend_from_slice(data);
    Ok(())
}

fn put_header(
    out: &mut Vec<u8>,
    sig: &[u8; 4],
    size: usize,
    flags: u32,
    form_id: u32,
    form_version: u16,
) -> Result<()> {
    out.extend_from_slice(sig);
    out.extend_from_slice(&u32::try_from(size)?.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&form_id.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&form_version.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(())
}

fn tes4_bytes(records: u32) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut hedr = 0.96f32.to_le_bytes().to_vec();
    hedr.extend_from_slice(&records.to_le_bytes());
    hedr.extend_from_slice(&0xFF00_1000u32.to_le_bytes());
    put_sub(&mut body, b"HEDR", &hedr)?;
    put_sub(&mut body, b"CNAM", b"DEFAULT\0")?;
    put_sub(&mut body, b"INCC", &0u32.to_le_bytes())?;
    let mut out = Vec::new();
    put_header(&mut out, b"TES4", body.len(), 0, 0, 582)?;
    out.extend(body);
    Ok(out)
}

/// Adler-32 (RFC 1950).
fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = a.wrapping_add(u32::from(*byte)) % 65_521;
        b = b.wrapping_add(a) % 65_521;
    }
    (b << 16) | a
}

/// A zlib stream holding `data` in stored (uncompressed) deflate blocks (RFC 1950/1951).
pub(crate) fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut chunks = data.chunks(0xFFFF).peekable();
    if chunks.peek().is_none() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    while let Some(chunk) = chunks.next() {
        out.push(u8::from(chunks.peek().is_none()));
        let len = u16::try_from(chunk.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// A plugin with no masters and one new GLOB (FormID `0x000800`), optionally compressed.
fn glob_plugin(edid: &str, value: f32, compressed: bool) -> Result<Vec<u8>> {
    let mut fields = Vec::new();
    let mut z = edid.as_bytes().to_vec();
    z.push(0);
    put_sub(&mut fields, b"EDID", &z)?;
    put_sub(&mut fields, b"FLTV", &value.to_le_bytes())?;
    let (flags, body) = if compressed {
        let mut b = u32::try_from(fields.len())?.to_le_bytes().to_vec();
        b.extend(zlib_stored(&fields));
        (0x0004_0000u32, b)
    } else {
        (0, fields)
    };
    let mut rec = Vec::new();
    put_header(&mut rec, b"GLOB", body.len(), flags, 0x800, 581)?;
    rec.extend(body);
    let mut out = tes4_bytes(2)?;
    let group_size = rec.len().checked_add(24).context("group too large")?;
    out.extend_from_slice(b"GRUP");
    out.extend_from_slice(&u32::try_from(group_size)?.to_le_bytes());
    out.extend_from_slice(b"GLOB");
    out.extend_from_slice(&[0; 12]);
    out.extend(rec);
    Ok(out)
}

/// The hand-built synthetic plugins: `(file name, bytes)`.
pub(crate) fn synthetic_plugins() -> Result<Vec<(&'static str, Vec<u8>)>> {
    Ok(vec![
        ("KC_S01_Empty.esp", tes4_bytes(1)?),
        (
            "KC_S02_NewGlobal.esp",
            glob_plugin("KC_S02_Global", 1234.5, false)?,
        ),
        (
            "KC_S03_CompressedGlobal.esp",
            glob_plugin("KC_S03_Global", 2345.25, true)?,
        ),
    ])
}

/// Overrides our own writer produces (`kit set`), with `Starfield.esm` as master.
const KIT_MADE: &[(&str, &str, &str)] = &[
    (
        "KC_S04_RifleRename.esm",
        "Starfield.esm:0021BBCD",
        "{\"FULL\": \"Old Soviet Marksman Rifle\"}",
    ),
    (
        "KC_S05_GmstOverride.esp",
        "Starfield.esm:000ABEF6",
        "{\"DATA\": 1234.5}",
    ),
    (
        "KC_S06_GlobOverride.esp",
        "Starfield.esm:000FB76D",
        "{\"FLTV\": 1234.5}",
    ),
];

/// Writes the synthetic set into `dir` (hand-built ones, then `kit set` overrides).
pub(crate) fn write_synthetic(dir: &Path, kit: &dyn Runner, cache: &Path) -> Result<Vec<Input>> {
    fs::create_dir_all(dir)?;
    fs::create_dir_all(cache)?;
    let mut out = Vec::new();
    for (name, bytes) in synthetic_plugins()? {
        let p = dir.join(name);
        fs::write(&p, bytes)?;
        out.push(Input {
            path: p,
            set: "synthetic",
        });
    }
    for (name, form, fields) in KIT_MADE {
        let p = dir.join(name);
        let r = kit.kit(
            &[
                "set".to_string(),
                (*form).to_string(),
                "--fields".into(),
                (*fields).to_string(),
                arg(&p)?,
                "--plugins".into(),
                "Starfield.esm".into(),
            ],
            cache,
        )?;
        if let Some(e) = r.error() {
            bail!("kit set for {name} failed: {e}");
        }
        out.push(Input {
            path: p,
            set: "kit",
        });
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Temp(PathBuf);
    impl Temp {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "kitkraft-xtask-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A fake Spriggit: serialize copies the plugin into the YAML folder as an opaque file,
    /// deserialize writes it back with the last byte flipped (a "lossy" tool); it records
    /// whether the YAML folder still exists when asked for the version again.
    struct FakeSpriggit {
        calls: RefCell<Vec<Vec<String>>>,
    }

    fn flag(args: &[String], f: &str) -> PathBuf {
        let i = args.iter().position(|a| a == f).unwrap();
        PathBuf::from(&args[i + 1])
    }

    impl Tool for FakeSpriggit {
        fn run(&self, args: &[String]) -> Result<ToolOut> {
            self.calls.borrow_mut().push(args.to_vec());
            let ok = |s: &str| ToolOut {
                code: Some(0),
                stdout: s.to_string(),
                stderr: String::new(),
            };
            match args[0].as_str() {
                "--version" => Ok(ToolOut {
                    code: Some(1),
                    stdout: "Spriggit version 0.41.0+Branch.main.Sha.abc\n--version\n".into(),
                    stderr: String::new(),
                }),
                "serialize" => {
                    assert!(args.contains(&"Spriggit.Yaml.Starfield".to_string()));
                    assert!(args.contains(&"0.41.0".to_string()));
                    let input = flag(args, "-i");
                    if input.to_string_lossy().contains("Bad") {
                        return Ok(ToolOut {
                            code: Some(3),
                            stdout: String::new(),
                            stderr: "boom\nfailed to serialize\n".into(),
                        });
                    }
                    let yaml = flag(args, "-o");
                    fs::create_dir_all(&yaml).unwrap();
                    fs::copy(input, yaml.join("opaque.bin")).unwrap();
                    Ok(ok("done"))
                }
                "deserialize" => {
                    let yaml = flag(args, "-i");
                    let mut b = fs::read(yaml.join("opaque.bin")).unwrap();
                    *b.last_mut().unwrap() ^= 0xFF;
                    fs::write(flag(args, "-o"), b).unwrap();
                    Ok(ok("done"))
                }
                other => panic!("unexpected {other}"),
            }
        }
    }

    struct FakeKit;
    impl Runner for FakeKit {
        fn kit(&self, args: &[String], _cache: &Path) -> Result<KitOut> {
            let out = Path::new(&args[1])
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|n| n == "out");
            let data = match args[0].as_str() {
                "coverage" => json!({"record_types": [
                    {"signature": "TES4", "records": 1, "signatures": [{"signature": "HEDR", "count": 1}]},
                    {"signature": "GLOB", "records": 1, "signatures": [
                        {"signature": "EDID", "count": 1},
                        {"signature": "FLTV", "count": if out { 0 } else { 1 }}]}]}),
                "roundtrip" => json!({"identical": true, "elapsed_ms": 4}),
                "diff" => {
                    json!({"counts": {"added": 0, "removed": 0, "changed": 1, "unchanged": 0},
                    "keyed_by": "raw", "added": [], "removed": [],
                    "changed": [{"signature": "GLOB", "form": "x:000800",
                        "subrecords": [{"signature": "FLTV", "change": "changed"}]}]})
                }
                other => panic!("unexpected kit {other}"),
            };
            Ok(KitOut {
                code: Some(0),
                json: json!({"data": data, "limitations": []}),
            })
        }
    }

    #[test]
    fn harness_deletes_the_yaml_and_compares_with_our_tools() {
        let tmp = Temp::new("oracle");
        let src = tmp.0.join("src");
        fs::create_dir_all(&src).unwrap();
        let (name, bytes) = synthetic_plugins().unwrap().remove(1);
        fs::write(src.join(name), &bytes).unwrap();
        fs::write(src.join("Bad.esp"), &bytes).unwrap();
        let opts = Options {
            data_dir: tmp.0.join("data"),
            work: tmp.0.join("work"),
            keep: false,
        };
        let tool = FakeSpriggit {
            calls: RefCell::new(Vec::new()),
        };
        let inputs = vec![
            Input {
                path: src.join(name),
                set: "synthetic",
            },
            Input {
                path: src.join("Bad.esp"),
                set: "given",
            },
        ];
        let report = spriggit(&tool, &FakeKit, &inputs, &opts).unwrap();
        assert_eq!(report["tool"]["package_version"], "0.41.0");
        assert!(report["tool"]["version"]
            .as_str()
            .unwrap()
            .starts_with("Spriggit version 0.41.0"));
        let f = &report["fixtures"][0];
        assert_eq!(f["identical"], false);
        assert_eq!(f["bytes"]["differing_bytes"], 1);
        assert_eq!(f["bytes"]["first_difference"], bytes.len() - 1);
        assert_eq!(
            f["subrecords"],
            json!([{"key": "GLOB/FLTV", "in": 1, "out": 0}])
        );
        assert_eq!(f["record_types"], json!([]));
        assert_eq!(f["kit_diff"]["changes"]["GLOB/FLTV changed"], 1);
        assert_eq!(f["out_parses"], true);
        let bad = &report["fixtures"][1];
        assert_eq!(bad["serialize"]["exit"], 3);
        assert_eq!(bad["serialize"]["message"], "failed to serialize");
        assert!(bad["deserialize"].is_null());
        assert_eq!(bad["bytes_out"], Value::Null);
        // Nothing of the work folder is left: the YAML went first, then the rest.
        assert!(!opts.work.join("00").exists());
        assert!(!opts.work.join("01").exists());
        // The source plugins are untouched.
        assert_eq!(fs::read(src.join(name)).unwrap(), bytes);
        let md = render(&report);
        assert!(md.contains("`GLOB/FLTV` 1 -> 0"));
        assert!(md.contains("serialize failed: failed to serialize"));
    }

    #[test]
    fn versions_need_a_number() {
        struct NoVersion;
        impl Tool for NoVersion {
            fn run(&self, _: &[String]) -> Result<ToolOut> {
                Ok(ToolOut {
                    code: Some(0),
                    stdout: "hello".into(),
                    stderr: String::new(),
                })
            }
        }
        if crate::env_var_os(SPRIGGIT_PACKAGE_VAR).is_none() {
            assert!(versions(&NoVersion).is_err());
        }
    }

    #[test]
    fn synthetic_plugins_are_well_formed() {
        for (name, bytes) in synthetic_plugins().unwrap() {
            let t = tes4(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(t.problem.is_none(), "{name}");
            let records = if name.contains("Empty") { 1 } else { 2 };
            assert_eq!(t.hedr, Some((0.96, records, 0xFF00_1000)), "{name}");
            assert!(t.masters.is_empty());
        }
        // The stored zlib stream: header check bits, block framing, Adler-32.
        let z = zlib_stored(b"abc");
        assert_eq!((u16::from(z[0]) << 8 | u16::from(z[1])) % 31, 0);
        assert_eq!(&z[2..7], &[0x01, 3, 0, 0xFC, 0xFF]);
        assert_eq!(&z[7..10], b"abc");
        assert_eq!(&z[10..], &0x024D_0127u32.to_be_bytes());
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        let big = vec![7u8; 70_000];
        let zb = zlib_stored(&big);
        assert_eq!(zb.len(), 2 + 5 + 65_535 + 5 + (70_000 - 65_535) + 4);
        assert_eq!(zlib_stored(b"").len(), 2 + 5 + 4);
    }

    /// The real `kit` (when built) reads the synthetic plugins: identical roundtrip, the
    /// compressed record decoded.
    #[test]
    fn synthetic_plugins_roundtrip_with_the_real_kit() {
        let Some(kit) = crate::observe::tests::built_kit() else {
            eprintln!("skipped: no kit binary next to the test executable (build kit-cli)");
            return;
        };
        let tmp = Temp::new("oracle-real");
        let runner = KitBinary::at(kit);
        for (name, bytes) in synthetic_plugins().unwrap() {
            let p = tmp.0.join(name);
            fs::write(&p, bytes).unwrap();
            let rt = runner
                .kit(
                    &["roundtrip".into(), arg(&p).unwrap(), "--verify".into()],
                    &tmp.0.join("cache"),
                )
                .unwrap();
            assert_eq!(rt.data()["identical"], true, "{name}: {}", rt.json);
            let cov = runner
                .kit(&["coverage".into(), arg(&p).unwrap()], &tmp.0.join("cache"))
                .unwrap();
            assert_eq!(cov.error(), None, "{name}");
            let (types, subs) = inventory(&cov);
            if name.contains("Global") {
                assert_eq!(types.get("GLOB"), Some(&1), "{name}");
                assert_eq!(subs.get("GLOB/FLTV"), Some(&1), "{name}: {subs:?}");
            }
        }
    }
}
