//! `cargo xtask oracle papyrus`: Bethesda's Papyrus compiler run directly, compared with
//! `kit papyrus compile` (owner priority B, 2026-10-15).
//!
//! `cargo xtask oracle papyrus [<psc>...] [--sources <dir>] [--optimize] [--out <dir>] [--keep]`
//!
//! For each script (default: [`DEFAULT_SCRIPTS`], our own sources under
//! `docs/recipes/scripts/KitKraft/`), the official `PapyrusCompiler.exe` is run as a black
//! box (ADR 0006) the way a modder runs it by hand: the declared name, `-f=` the flags file
//! of the extracted sources, `-i=` the script's root then the extracted `Scripts\Source`,
//! `-o=` a temp folder, from an empty working directory, never `-release` or `-final`
//! ([`official_args`]). The same script goes through `kit papyrus compile <psc path>`, with
//! `--sources-zip` when `kit doctor` finds `Tools\ContentResources.zip` (the recipe's form),
//! else `--import <sources>`. Both `.pex` files are disassembled with `kit papyrus disasm
//! --format json` and compared:
//!
//! - **identical**: the bytes are equal;
//! - **equivalent**: equal after [`normalize`] (compile time, user and machine names, the
//!   source file name's case and folder, the debug block's modification time, every byte
//!   offset and object size, the order of the string table and of the debug tables); the
//!   report lists which of those fields differed;
//! - **different**: the first divergence after normalization, as a JSON path with both
//!   values and the byte offset of the nearest enclosing element in each file, plus the
//!   first differing byte.
//!
//! The run fails when a script does not compile on one side or comes out different.
//!
//! Tools: `KITKRAFT_PAPYRUS_COMPILER` names the compiler (else `kit doctor`'s detection);
//! `--sources` or `KITKRAFT_PAPYRUS_IMPORTS` the extracted `Scripts\Source` (else
//! [`DEFAULT_SOURCES`], the owner-approved extraction, `docs/source-registry.md`). The
//! extraction is untrusted data: nothing in it is run, it is only an import folder. Reports
//! go to `--out` (default `observe/oracle/papyrus/`, gitignored); `docs/oracles.md` records
//! the results in our own words.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::evidence::{arg, byte_diff, sha256_hex, table, KitBinary, Runner};

const COMPILER_VAR: &str = "KITKRAFT_PAPYRUS_COMPILER";
const SOURCES_VAR: &str = "KITKRAFT_PAPYRUS_IMPORTS";
/// The extraction of `Tools\ContentResources.zip` the owner approved (2026-10-15, item 2).
pub(crate) const DEFAULT_SOURCES: &str = r"C:\Modding\tools\ck-sources\Scripts\Source";
const FLAGS_FILE: &str = "Starfield_Papyrus_Flags.flg";
/// Our own oracle scripts, relative to the checkout.
pub(crate) const DEFAULT_SCRIPTS: [&str; 8] = [
    "docs/recipes/scripts/KitKraft/HelloOnLoad.psc",
    "docs/recipes/scripts/KitKraft/OracleProperties.psc",
    "docs/recipes/scripts/KitKraft/OracleEvents.psc",
    "docs/recipes/scripts/KitKraft/OracleNatives.psc",
    "docs/recipes/scripts/KitKraft/OracleStrings.psc",
    "docs/recipes/scripts/KitKraft/OracleFloats.psc",
    "docs/recipes/scripts/KitKraft/OracleArrays.psc",
    "docs/recipes/scripts/KitKraft/OracleControl.psc",
];
/// Keys whose values legitimately differ between two compiles of one source.
const VOLATILE_KEYS: [&str; 7] = [
    "compile_time",
    "user",
    "machine",
    "modification_time",
    "offset",
    "strings_offset",
    "size",
];
/// Most normalized differences listed per script.
const LIST_CAP: usize = 40;

/// `cargo xtask oracle papyrus ...`.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let mut scripts: Vec<PathBuf> = Vec::new();
    let mut sources: Option<PathBuf> = None;
    let mut optimize = false;
    let mut keep = false;
    let mut out = root.join("observe").join("oracle").join("papyrus");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--optimize" => optimize = true,
            "--keep" => keep = true,
            "--sources" => {
                sources = Some(PathBuf::from(
                    it.next().context("--sources needs a folder")?,
                ));
            }
            "--out" => out = PathBuf::from(it.next().context("--out needs a folder")?),
            other if other.starts_with("--") => bail!("oracle papyrus: unknown option `{other}`"),
            other => scripts.push(PathBuf::from(other)),
        }
    }
    if scripts.is_empty() {
        scripts = DEFAULT_SCRIPTS.iter().map(|s| root.join(s)).collect();
    }
    let sources = sources
        .or_else(|| crate::env_var_os(SOURCES_VAR).map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SOURCES));
    let flags = sources.join(FLAGS_FILE);
    if !flags.is_file() {
        bail!(
            "no {FLAGS_FILE} in `{}`: pass --sources (or set {SOURCES_VAR}) to an extraction of \
             Tools\\ContentResources.zip's Scripts\\Source",
            sources.display()
        );
    }

    let kit = KitBinary::from_env(root);
    let work = std::env::temp_dir().join(format!("kitkraft-oracle-papyrus-{}", std::process::id()));
    let _ = fs::remove_dir_all(&work);
    let cwd = work.join("cwd");
    fs::create_dir_all(&cwd).with_context(|| format!("creating {}", cwd.display()))?;
    let cache = work.join("cache");
    let doctor = kit.kit(&["doctor".to_string()], &cache)?;
    let check_path = |id: &str| -> Option<PathBuf> {
        doctor
            .data()
            .get("checks")?
            .as_array()?
            .iter()
            .find(|c| c.get("id").and_then(Value::as_str) == Some(id))
            .filter(|c| c.get("ok").and_then(Value::as_bool) == Some(true))
            .and_then(|c| c.get("path").and_then(Value::as_str))
            .map(PathBuf::from)
    };
    let compiler = crate::env_var_os(COMPILER_VAR)
        .map(PathBuf::from)
        .or_else(|| check_path("papyrus_compiler"))
        .filter(|p| p.is_file())
        .with_context(|| {
            format!("PapyrusCompiler.exe not found: set {COMPILER_VAR} or install the Creation Kit")
        })?;
    let zip = check_path("content_resources").filter(|p| p.is_file());
    // The kit cache is kept between runs only through KITKRAFT_CACHE_DIR.
    let kit_cache = crate::env_var_os("KITKRAFT_CACHE_DIR").map_or(cache, PathBuf::from);

    println!("compiler: {}", compiler.display());
    println!("sources:  {}", sources.display());
    let mut results = Vec::new();
    for psc in &scripts {
        let r = one_script(
            &kit,
            &Setup {
                compiler: &compiler,
                sources: &sources,
                flags: &flags,
                zip: zip.as_deref(),
                cwd: &cwd,
                work: &work,
                cache: &kit_cache,
                optimize,
            },
            psc,
        )
        .unwrap_or_else(|e| json!({"script": psc.display().to_string(), "verdict": "error", "error": format!("{e:#}")}));
        println!(
            "{:<28} {}",
            r.get("object").and_then(Value::as_str).unwrap_or("?"),
            r.get("verdict").and_then(Value::as_str).unwrap_or("?")
        );
        results.push(r);
    }

    let report = json!({
        "generated_by": "cargo xtask oracle papyrus",
        "compiler": compiler.display().to_string(),
        "compiler_sha256": fs::read(&compiler).map(|b| sha256_hex(&b)).unwrap_or_default(),
        "sources": sources.display().to_string(),
        "kit_sources": zip.as_ref().map_or_else(|| format!("--import {}", sources.display()), |z| format!("--sources-zip {}", z.display())),
        "optimize": optimize,
        "results": results,
    });
    fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    fs::write(out.join("report.md"), markdown(&report))?;
    println!("report: {}", out.join("report.md").display());
    if keep {
        println!("kept: {}", work.display());
    } else {
        let _ = fs::remove_dir_all(&work);
    }
    let count = |verdict: &str| {
        at(&report, "/results").as_array().map_or(0, |r| {
            r.iter().filter(|x| at(x, "/verdict") == verdict).count()
        })
    };
    let (errors, different) = (count("error"), count("different"));
    if errors > 0 || different > 0 {
        bail!(
            "{errors} script(s) did not compile on one side and {different} differ after \
             normalization; see the report"
        );
    }
    Ok(())
}

/// Fixed inputs of one run.
struct Setup<'a> {
    compiler: &'a Path,
    sources: &'a Path,
    flags: &'a Path,
    zip: Option<&'a Path>,
    cwd: &'a Path,
    work: &'a Path,
    cache: &'a Path,
    optimize: bool,
}

fn one_script(kit: &impl Runner, s: &Setup<'_>, psc: &Path) -> Result<Value> {
    let text = fs::read_to_string(psc).with_context(|| format!("reading {}", psc.display()))?;
    let object =
        script_name(&text).with_context(|| format!("no ScriptName in {}", psc.display()))?;
    let root = script_root(psc, &object)?;
    let parts: Vec<&str> = object.split(':').collect();
    let stem = object.replace(':', "_");
    let official_out = s.work.join(&stem).join("official");
    let kit_out = s.work.join(&stem).join("kit");
    fs::create_dir_all(&official_out)?;

    // Official, by hand.
    let args = official_args(
        &object,
        s.flags,
        &[&root, s.sources],
        &official_out,
        s.optimize,
    )?;
    let output = Command::new(s.compiler)
        .args(&args)
        .current_dir(s.cwd)
        .output()
        .with_context(|| format!("starting {}", s.compiler.display()))?;
    let official_log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pex_rel = parts
        .iter()
        .fold(PathBuf::new(), |p, part| p.join(part))
        .with_extension("pex");
    let official_pex = official_out.join(&pex_rel);
    if !output.status.success() || !official_pex.is_file() {
        bail!(
            "the official compiler failed on {object} (exit {:?}): {}",
            output.status.code(),
            tail(&official_log, 6)
        );
    }

    // KitKraft.
    let mut kargs = vec![
        "papyrus".to_string(),
        "compile".to_string(),
        arg(psc)?,
        arg(&kit_out)?,
    ];
    match s.zip {
        Some(z) => kargs.extend(["--sources-zip".to_string(), arg(z)?]),
        None => kargs.extend(["--import".to_string(), arg(s.sources)?]),
    }
    if s.optimize {
        kargs.push("--optimize".to_string());
    }
    let k = kit.kit(&kargs, s.cache)?;
    if k.data().get("passed") != Some(&Value::Bool(true)) {
        bail!(
            "kit papyrus compile failed on {object}: {}",
            k.error().unwrap_or_else(|| tail(
                &format!(
                    "{}{}",
                    at(k.data(), "/stdout").as_str().unwrap_or_default(),
                    at(k.data(), "/stderr").as_str().unwrap_or_default()
                ),
                6
            ))
        );
    }
    let kit_pex = kit_out.join(&pex_rel);

    let a = fs::read(&official_pex)?;
    let b = fs::read(&kit_pex)?;
    let bytes = byte_diff(&a, &b, 8);
    let disasm = |p: &Path| -> Result<Value> {
        let out = kit.kit(
            &[
                "papyrus".to_string(),
                "disasm".to_string(),
                arg(p)?,
                "--format".to_string(),
                "json".to_string(),
            ],
            s.cache,
        )?;
        out.data()
            .get("script")
            .cloned()
            .with_context(|| format!("kit papyrus disasm {}: {:?}", p.display(), out.error()))
    };
    let ja = disasm(&official_pex)?;
    let jb = disasm(&kit_pex)?;
    let cmp = compare(&ja, &jb);
    let verdict = if bytes.identical {
        "identical"
    } else if cmp.divergence.is_none() {
        "equivalent"
    } else {
        "different"
    };
    Ok(json!({
        "script": psc.display().to_string(),
        "object": object,
        "verdict": verdict,
        "official": {"args": args, "bytes": a.len(), "sha256": sha256_hex(&a)},
        "kit": {"args": k.data().get("args").cloned().unwrap_or(Value::Null), "bytes": b.len(), "sha256": sha256_hex(&b)},
        "first_byte_difference": bytes.first_difference,
        "differing_bytes": bytes.differing_bytes,
        "normalized_differences": cmp.normalized,
        "divergence": cmp.divergence,
        "functions": count(&ja, "instructions"),
    }))
}

/// The official compiler's arguments: never a release or final switch.
pub(crate) fn official_args(
    object: &str,
    flags: &Path,
    imports: &[&Path],
    out: &Path,
    optimize: bool,
) -> Result<Vec<String>> {
    let imports: Vec<String> = imports.iter().map(|p| arg(p)).collect::<Result<_>>()?;
    let mut args = vec![
        object.to_string(),
        format!("-f={}", arg(flags)?),
        format!("-i={}", imports.join(";")),
        format!("-o={}", arg(out)?),
    ];
    if optimize {
        args.push("-op".to_string());
    }
    if args.iter().any(|a| {
        let l = a.to_ascii_lowercase();
        (l.starts_with('-') || l.starts_with('/'))
            && matches!(
                l.trim_start_matches(['-', '/'])
                    .split(['=', ':'])
                    .next()
                    .unwrap_or_default(),
                "r" | "release" | "final"
            )
    }) {
        bail!("refusing a release or final switch");
    }
    Ok(args)
}

/// The name a `ScriptName` statement declares (keywords ignore case; comments skipped).
pub(crate) fn script_name(text: &str) -> Option<String> {
    let mut in_block = false;
    for line in text.lines() {
        let t = line.trim();
        if in_block {
            in_block = !t.contains("/;");
            continue;
        }
        if t.starts_with(";/") {
            in_block = !t.contains("/;");
            continue;
        }
        let mut words = t.split_whitespace();
        if words
            .next()
            .is_some_and(|w| w.eq_ignore_ascii_case("scriptname"))
        {
            return words.next().map(str::to_string);
        }
    }
    None
}

/// The import root of `psc`: one folder up per namespace of `object`.
fn script_root(psc: &Path, object: &str) -> Result<PathBuf> {
    let abs = std::path::absolute(psc)?;
    let mut root = abs.parent().map(Path::to_path_buf).unwrap_or_default();
    for _ in object.split(':').skip(1) {
        root = root.parent().map(Path::to_path_buf).unwrap_or_default();
    }
    Ok(root)
}

static NULL: Value = Value::Null;

/// The value at a JSON pointer, or `null`.
fn at<'v>(v: &'v Value, pointer: &str) -> &'v Value {
    v.pointer(pointer).unwrap_or(&NULL)
}

fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines
        .get(lines.len().saturating_sub(n)..)
        .unwrap_or_default()
        .join(" / ")
}

fn count(script: &Value, key: &str) -> usize {
    match script {
        Value::Object(m) => m
            .iter()
            .map(|(k, v)| {
                if k == key {
                    v.as_array().map_or(0, Vec::len)
                } else {
                    count(v, key)
                }
            })
            .fold(0usize, usize::saturating_add),
        Value::Array(a) => a
            .iter()
            .map(|v| count(v, key))
            .fold(0usize, usize::saturating_add),
        _ => 0,
    }
}

/// The result of comparing two disassemblies.
#[derive(Debug, Default)]
pub(crate) struct Comparison {
    /// Paths (indices collapsed to `[]`) that differ before normalization, when the
    /// normalized forms agree or before the divergence.
    pub normalized: Vec<String>,
    /// The first divergence after normalization.
    pub divergence: Option<Value>,
}

/// Compares two `kit papyrus disasm --format json` scripts.
pub(crate) fn compare(a: &Value, b: &Value) -> Comparison {
    // Volatile fields: differences between the two with the tables in one order.
    let mut sa = a.clone();
    let mut sb = b.clone();
    sort_sets(&mut sa);
    sort_sets(&mut sb);
    let mut raw = Vec::new();
    leaf_diffs(&sa, &sb, "", &mut raw);
    // Order: differences inside an unordered table before sorting.
    let mut unsorted = Vec::new();
    leaf_diffs(&strip(a), &strip(b), "", &mut unsorted);
    let tables: Vec<String> = UNORDERED
        .iter()
        .map(|p| {
            collapse(
                &p.trim_start_matches('/')
                    .replace("/*", "[]")
                    .replace('/', "."),
            )
        })
        .collect();
    for p in unsorted {
        let pattern = collapse(&p);
        if let Some(t) = tables
            .iter()
            .filter(|t| pattern.starts_with(t.as_str()))
            .max_by_key(|t| t.len())
        {
            raw.push(format!("{t} (order)"));
        }
    }
    let mut normalized: Vec<String> = Vec::new();
    for p in raw {
        let pattern = collapse(&p);
        if !normalized.contains(&pattern) && normalized.len() < LIST_CAP {
            normalized.push(pattern);
        }
    }
    let na = normalize(a);
    let nb = normalize(b);
    let divergence = first_diff(&na, &nb, "").map(|path| {
        json!({
            "path": path,
            "official": lookup(&na, &path).cloned().unwrap_or(Value::Null),
            "kit": lookup(&nb, &path).cloned().unwrap_or(Value::Null),
            "official_offset": nearest_offset(&sa, &path),
            "kit_offset": nearest_offset(&sb, &path),
        })
    });
    Comparison {
        normalized,
        divergence,
    }
}

/// Tables the official compiler writes in a different order on every run (observed
/// 2026-10-15: four compiles of one source, four orders of the user flags and properties):
/// they are looked up by name, so they compare as sets. Pointers relative to the script;
/// `*` is every element of an array.
const UNORDERED: [&str; 13] = [
    "/string_table",
    "/user_flags",
    "/debug/function_lines",
    "/debug/groups",
    "/debug/member_orders",
    "/objects/*/variables",
    "/objects/*/properties",
    "/objects/*/property_functions",
    "/objects/*/guards",
    "/objects/*/structs",
    "/objects/*/structs/*/members",
    "/objects/*/states",
    "/objects/*/states/*/functions",
];

/// Sorts the arrays [`UNORDERED`] names, deepest first, by their `name` then their text.
fn sort_sets(v: &mut Value) {
    for pointer in UNORDERED.iter().rev() {
        sort_at(
            v,
            pointer
                .trim_start_matches('/')
                .split('/')
                .collect::<Vec<_>>()
                .as_slice(),
        );
    }
}

fn sort_at(v: &mut Value, steps: &[&str]) {
    match steps.split_first() {
        None => {
            if let Value::Array(items) = v {
                // Keyed on the stripped element, so a sort with offsets kept and one
                // without them agree.
                items.sort_by_cached_key(|e| {
                    (
                        e.get("name").map(Value::to_string).unwrap_or_default(),
                        strip(e).to_string(),
                    )
                });
            }
        }
        Some((&"*", rest)) => {
            if let Value::Array(items) = v {
                for e in items {
                    sort_at(e, rest);
                }
            }
        }
        Some((key, rest)) => {
            if let Some(next) = v.get_mut(*key) {
                sort_at(next, rest);
            }
        }
    }
}

/// Drops [`VOLATILE_KEYS`], reduces `header.source` to its lower-case file name, and sorts
/// the [`UNORDERED`] tables.
pub(crate) fn normalize(script: &Value) -> Value {
    let mut v = strip(script);
    if let Some(src) = v.pointer_mut("/header/source") {
        let name = src
            .as_str()
            .unwrap_or_default()
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        *src = Value::String(name);
    }
    sort_sets(&mut v);
    v
}

/// Whether `m` is a float value (`{type: "float", value, text, bits}`, `kit papyrus disasm
/// --format json`): compared by its `bits`, never by `value` (a JSON number equates -0.0
/// with 0.0 and every NaN is null), so a float operand or default whose stored bits differ
/// is a divergence (Fable review of lane B, finding 5).
fn is_float_value(m: &serde_json::Map<String, Value>) -> bool {
    m.get("type").and_then(Value::as_str) == Some("float") && m.contains_key("bits")
}

fn strip(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let float = is_float_value(m);
            Value::Object(
                m.iter()
                    .filter(|(k, _)| !VOLATILE_KEYS.contains(&k.as_str()))
                    .filter(|(k, _)| !(float && k.as_str() == "value"))
                    .map(|(k, v)| (k.clone(), strip(v)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(strip).collect()),
        other => other.clone(),
    }
}

/// Every differing leaf path (`a.b[3].c`), keys of both sides.
fn leaf_diffs(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = join(path, k);
                match (x.get(k.as_str()), y.get(k.as_str())) {
                    (Some(l), Some(r)) => leaf_diffs(l, r, &p, out),
                    _ => out.push(p),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (l, r)) in x.iter().zip(y).enumerate() {
                leaf_diffs(l, r, &format!("{path}[{i}]"), out);
            }
        }
        _ if a != b => out.push(path.to_string()),
        _ => {}
    }
}

/// The first differing path, depth first in key order; an array of another length differs
/// at its first unequal element or at its end.
fn first_diff(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            keys.into_iter().find_map(|k| {
                let p = join(path, k);
                match (x.get(k.as_str()), y.get(k.as_str())) {
                    (Some(l), Some(r)) => first_diff(l, r, &p),
                    _ => Some(p),
                }
            })
        }
        (Value::Array(x), Value::Array(y)) => x
            .iter()
            .zip(y)
            .enumerate()
            .find_map(|(i, (l, r))| first_diff(l, r, &format!("{path}[{i}]")))
            .or_else(|| (x.len() != y.len()).then(|| format!("{path}[{}]", x.len().min(y.len())))),
        _ if a != b => Some(path.to_string()),
        _ => None,
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// `a.b[2].c` -> `a.b[].c`.
fn collapse(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut in_index = false;
    for c in path.chars() {
        match c {
            '[' => {
                in_index = true;
                out.push('[');
            }
            ']' => {
                in_index = false;
                out.push(']');
            }
            _ if in_index => {}
            _ => out.push(c),
        }
    }
    out
}

/// Splits `a.b[2].c` into steps.
fn steps(path: &str) -> Vec<Step> {
    let mut out = Vec::new();
    for part in path.split('.') {
        let mut rest = part;
        if let Some(i) = rest.find('[') {
            let (key, idx) = rest.split_at(i);
            if !key.is_empty() {
                out.push(Step::Key(key.to_string()));
            }
            rest = idx;
            for n in rest.split(['[', ']']).filter(|s| !s.is_empty()) {
                if let Ok(n) = n.parse() {
                    out.push(Step::Index(n));
                }
            }
        } else if !rest.is_empty() {
            out.push(Step::Key(rest.to_string()));
        }
    }
    out
}

enum Step {
    Key(String),
    Index(usize),
}

fn lookup<'v>(v: &'v Value, path: &str) -> Option<&'v Value> {
    steps(path).iter().try_fold(v, |cur, s| match s {
        Step::Key(k) => cur.get(k.as_str()),
        Step::Index(i) => cur.get(*i),
    })
}

/// The `offset` of the deepest element along `path` that has one (instructions, functions,
/// properties, variables, objects), else `strings_offset` for the string table.
fn nearest_offset(v: &Value, path: &str) -> Option<u64> {
    let mut cur = v;
    let mut best = if path.starts_with("string_table") {
        v.get("strings_offset").and_then(Value::as_u64)
    } else {
        None
    };
    for s in steps(path) {
        let next = match &s {
            Step::Key(k) => cur.get(k.as_str()),
            Step::Index(i) => cur.get(*i),
        };
        let Some(next) = next else { break };
        cur = next;
        if let Some(o) = cur.get("offset").and_then(Value::as_u64) {
            best = Some(o);
        }
    }
    best
}

fn markdown(report: &Value) -> String {
    let mut out = String::from("# Papyrus compiler oracle\n\n");
    let _ = writeln!(
        out,
        "Written by `cargo xtask oracle papyrus`. Compiler: `{}` (SHA-256 `{}`); sources: `{}`; \
         kit: `{}`; optimize: {}.\n",
        at(report, "/compiler").as_str().unwrap_or_default(),
        at(report, "/compiler_sha256").as_str().unwrap_or_default(),
        at(report, "/sources").as_str().unwrap_or_default(),
        at(report, "/kit_sources").as_str().unwrap_or_default(),
        at(report, "/optimize")
    );
    let empty = Vec::new();
    let rows: Vec<Vec<String>> = at(report, "/results")
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|r| {
            let detail = match at(r, "/verdict").as_str() {
                Some("different") => format!(
                    "{} (official {} @{}, kit {} @{}); first byte {}",
                    at(r, "/divergence/path").as_str().unwrap_or_default(),
                    at(r, "/divergence/official"),
                    at(r, "/divergence/official_offset"),
                    at(r, "/divergence/kit"),
                    at(r, "/divergence/kit_offset"),
                    at(r, "/first_byte_difference")
                ),
                Some("error") => at(r, "/error").as_str().unwrap_or_default().to_string(),
                _ => at(r, "/normalized_differences")
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default(),
            };
            vec![
                at(r, "/object").as_str().unwrap_or("?").to_string(),
                at(r, "/verdict").as_str().unwrap_or("?").to_string(),
                format!("{} / {}", at(r, "/official/bytes"), at(r, "/kit/bytes")),
                at(r, "/functions").to_string(),
                detail,
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "Script",
            "Verdict",
            "Bytes (official / kit)",
            "Instructions",
            "Normalized differences or first divergence",
        ],
        &rows,
    ));
    out
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn script(time: u64, strings: &[&str], op: &str) -> Value {
        json!({
            "header": {"compile_time": time, "source": "C:\\x\\Hello.psc", "user": "u", "machine": "m"},
            "strings_offset": 40,
            "string_table": strings,
            "debug": {"offset": 300 + time, "modification_time": time, "function_lines": [
                {"function": "b", "lines": [2]}, {"function": "a", "lines": [1]}]},
            "objects": [{"name": "Hello", "offset": 400 + time, "size": 10,
                "states": [{"name": "", "offset": 500, "functions": [{"name": "F", "offset": 600,
                    "instructions": [{"offset": 610 + time, "op": op, "args": []}]}]}]}],
        })
    }

    #[test]
    fn equivalent_when_only_volatile_fields_and_orders_differ() {
        let a = script(1, &["a", "b"], "iadd");
        let mut b = script(2, &["b", "a"], "iadd");
        b["header"]["source"] = json!("hello.PSC");
        b["debug"]["function_lines"] = json!([
            {"function": "a", "lines": [1]}, {"function": "b", "lines": [2]}]);
        let c = compare(&a, &b);
        assert!(c.divergence.is_none(), "{:?}", c.divergence);
        assert!(c.normalized.contains(&"header.compile_time".to_string()));
        assert!(c.normalized.contains(&"string_table (order)".to_string()));
        assert!(c
            .normalized
            .contains(&"debug.function_lines (order)".to_string()));
        assert!(c
            .normalized
            .contains(&"objects[].states[].functions[].instructions[].offset".to_string()));
    }

    #[test]
    fn different_reports_the_first_divergence_with_offsets() {
        let a = script(1, &["a"], "iadd");
        let b = script(1, &["a"], "isub");
        let c = compare(&a, &b);
        let d = c.divergence.unwrap();
        assert_eq!(
            d["path"],
            "objects[0].states[0].functions[0].instructions[0].op"
        );
        assert_eq!(d["official"], "iadd");
        assert_eq!(d["kit"], "isub");
        assert_eq!(d["official_offset"], 611);
        let extra = script(1, &["a", "z"], "iadd");
        let d = compare(&a, &extra).divergence.unwrap();
        assert_eq!(d["path"], "string_table[1]");
        assert_eq!(d["kit_offset"], 40);
    }

    /// Floats compare by their stored bits: -0.0 and 0.0, or two NaN payloads, are equal
    /// as JSON numbers (NaN is null) but differ in the .pex (Fable review, finding 5).
    #[test]
    fn floats_compare_by_their_bits() {
        let with_float = |value: Value, bits: &str| {
            let mut s = script(1, &["a"], "fadd");
            s["objects"][0]["states"][0]["functions"][0]["instructions"][0]["args"] =
                json!([{"type": "float", "value": value, "text": "0.0", "bits": bits}]);
            s
        };
        let zero = with_float(json!(0.0), "0x00000000");
        let minus_zero = with_float(json!(-0.0), "0x80000000");
        let d = compare(&zero, &minus_zero).divergence.unwrap();
        assert_eq!(
            d["path"],
            "objects[0].states[0].functions[0].instructions[0].args[0].bits"
        );
        assert_eq!(d["official"], "0x00000000");
        assert_eq!(d["kit"], "0x80000000");
        let nan = with_float(Value::Null, "0x7FC00000");
        let other_nan = with_float(Value::Null, "0x7FC00001");
        assert!(compare(&nan, &other_nan).divergence.is_some());
        assert!(compare(&nan, &nan.clone()).divergence.is_none());
        // Equal bits: no divergence and no `value` left to compare.
        assert!(compare(&zero, &zero.clone()).divergence.is_none());
        assert!(
            normalize(&zero)["objects"][0]["states"][0]["functions"][0]["instructions"][0]["args"]
                [0]
            .get("value")
            .is_none()
        );
    }

    #[test]
    fn script_names_and_roots() {
        let text = ";/ ScriptName Fake\n/;\n; comment\nscriptNAME KitKraft:Hello extends X\n";
        assert_eq!(script_name(text).as_deref(), Some("KitKraft:Hello"));
        assert_eq!(script_name("Int x\n"), None);
        // The source tree is absolute on both platforms: `C:\...` on Windows,
        // `/...` elsewhere, so `absolute()` leaves it untouched either way.
        let (psc, expect) = if cfg!(windows) {
            (
                PathBuf::from(r"C:\a\b\KitKraft\Hello.psc"),
                PathBuf::from(r"C:\a\b"),
            )
        } else {
            (
                PathBuf::from("/a/b/KitKraft/Hello.psc"),
                PathBuf::from("/a/b"),
            )
        };
        let root = script_root(&psc, "KitKraft:Hello").unwrap();
        assert_eq!(root, expect);
        assert_eq!(collapse("a[1].b[22]"), "a[].b[]");
    }

    #[test]
    fn official_arguments_never_ask_for_release() {
        let args = official_args(
            "Ns:S",
            Path::new(r"C:\s\f.flg"),
            &[Path::new(r"C:\r"), Path::new(r"C:\s")],
            Path::new(r"C:\o"),
            true,
        )
        .unwrap();
        assert_eq!(
            args,
            ["Ns:S", r"-f=C:\s\f.flg", r"-i=C:\r;C:\s", r"-o=C:\o", "-op"]
        );
        assert!(official_args("-release", Path::new("f"), &[], Path::new("o"), false).is_err());
    }
}
