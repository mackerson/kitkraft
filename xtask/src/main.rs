//! Repository automation.
//!
//! - `cargo xtask ci`: the gate every commit passes (layers, codegen --check, fmt, clippy,
//!   test).
//! - `cargo xtask layers`: the layer check alone. Compiles nothing; runs `cargo metadata`.
//! - `cargo xtask codegen [--check]`: typed records from the schema ([`codegen`]).
//! - `cargo xtask parity [--check]`: `docs/record-parity.md` from the corpus ([`parity`]).
//! - `cargo xtask inventory-def`, `inventory-check`: write an inventory definition from the
//!   corpus, check the counts one cites ([`inventory`]); `cargo xtask add-def`: register
//!   definitions in the shared lists and regenerate ([`adddef`]).
//! - `cargo xtask dist [--out DIR] [--sync-bin]`: the double-click distribution folder ([`dist`]).
//! - `cargo xtask observe`: CK observation fixtures to reports (lane H, [`observe`]).
//! - `cargo xtask oracle`: external tools (Spriggit, SF1Edit, the Papyrus compiler) run as
//!   oracles (lane H, [`oracle`], [`sf1edit`], [`papyrus`]); `oracle ck` compares our plugin
//!   with the Creation Kit's export of it (lane A, [`ckoracle`]).
//!
//! The layer check has three parts.
//!
//! 1. **Manifests** (fast, TOML only). Every workspace member's `Cargo.toml` and its
//!    `//! LAYER:` marker (in `src/lib.rs` or `src/main.rs`); fails on:
//!    - a marker that is missing, malformed, or disagrees with [`LAYER_TABLE`];
//!    - a package missing from [`LAYER_TABLE`];
//!    - an upward dependency, or a same-layer dependency not in [`SAME_LAYER_ALLOWED`];
//!    - any dependency on an app or tool package;
//!    - a direct dependency matching [`LIBRARY_BANNED`] in a library crate, or
//!      [`UI_TOOLKIT`] below L5 (name prefixes);
//!    - a package without `[lints] workspace = true`.
//!
//!    `[dependencies]`, `[build-dependencies]` and their `[target.*]` forms are read here.
//! 2. **Sources** ([`smuggle`]): `#[path]` and `include!`-family paths that leave the package;
//!    `unsafe` (or the `unsafe_code` lint) outside the files in `smuggle::UNSAFE_ALLOWED`.
//! 3. **Resolved graph** ([`graph`], via `cargo metadata`): transitive bans,
//!    dev-dependency rules, build scripts, and agreement between cargo's member list and
//!    part 1.
//!
//! LAYER: TOOL

mod adddef;
mod ckoracle;
mod codegen;
mod dist;
mod evidence;
mod graph;
mod inventory;
mod observe;
mod oracle;
mod papyrus;
mod parity;
mod sf1edit;
mod smuggle;
mod ui;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{anyhow, bail, Context, Result};

/// Canonical layer of every workspace package, including planned ones. Changing an entry
/// is an architecture change: it needs an ADR and the same edit to the table in AGENTS.md.
const LAYER_TABLE: &[(&str, Layer)] = &[
    ("kitkraft-schema", Layer::L(0)),
    ("kitkraft-esm", Layer::L(0)),
    ("kitkraft-ba2", Layer::L(0)),
    ("kitkraft-pex", Layer::L(0)),
    ("kitkraft-strings", Layer::L(0)),
    ("kitkraft-nif", Layer::L(0)),
    ("kitkraft-doc", Layer::L(1)),
    ("kitkraft-io", Layer::L(3)),
    ("kitkraft-game", Layer::L(3)),
    ("kitkraft-engine", Layer::L(4)),
    ("kitkraft-mcp", Layer::L(5)),
    ("kitkraft-ui-egui", Layer::L(5)),
    ("kit-cli", Layer::App),
    ("kit", Layer::App),
    ("xtask", Layer::Tool),
];

/// Same-layer edges that are allowed (`from`, `to`). Every entry is justified in
/// `plan/adr/0002-layering.md`.
const SAME_LAYER_ALLOWED: &[(&str, &str)] = &[("kitkraft-esm", "kitkraft-schema")];

/// Name prefixes of external crates no library crate (L0..L5) may depend on, directly or
/// transitively. Libraries use `thiserror`.
const LIBRARY_BANNED: &[&str] = &["anyhow", "eyre"];

/// Name prefixes of UI toolkit crates nothing below L5 may depend on, directly or
/// transitively (`egui` also covers `egui-winit`, `egui_extras`, ...).
const UI_TOOLKIT: &[&str] = &["egui", "eframe", "winit", "wgpu", "rfd", "iced", "slint"];

/// The entry of `prefixes` that `name` starts with, if any.
fn banned_prefix<'a>(name: &str, prefixes: &[&'a str]) -> Option<&'a str> {
    prefixes.iter().copied().find(|p| name.starts_with(p))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    L(u8),
    App,
    Tool,
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Layer::L(n) => write!(f, "L{n}"),
            Layer::App => f.write_str("APP"),
            Layer::Tool => f.write_str("TOOL"),
        }
    }
}

struct Package {
    name: String,
    dir: PathBuf,
    manifest: PathBuf,
    layer: Layer,
    deps: Vec<String>,
    workspace_lints: bool,
}

/// `name` (a `KITKRAFT_*` variable) from the environment, else its pre-rename `KITCRAFT_*`
/// spelling. Deprecated fallback kept for one release after the KitKraft rename
/// (`docs/corpus.md`, "Environment"); remove it in the release after.
pub(crate) fn env_var_os(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name).or_else(|| {
        let rest = name.strip_prefix("KITKRAFT_")?;
        std::env::var_os(format!("KITCRAFT_{rest}"))
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("xtask: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("ci") => ci(&workspace_root()?),
        Some("layers") => layers(&workspace_root()?),
        Some("codegen") => codegen::run(&workspace_root()?, check_flag("codegen")?),
        Some("parity") => parity::run(&workspace_root()?, check_flag("parity")?),
        Some("inventory-def") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            inventory::run_def(&workspace_root()?, &rest)
        }
        Some("inventory-check") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            inventory::run_check(&workspace_root()?, &rest)
        }
        Some("add-def") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            adddef::run(&workspace_root()?, &rest)
        }
        Some("observe") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            observe::run(&workspace_root()?, &rest)
        }
        Some("dist") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            dist::run(&workspace_root()?, &rest)
        }
        Some("ui") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            ui::run(&workspace_root()?, &rest)
        }
        Some("oracle") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            oracle::run(&workspace_root()?, &rest)
        }
        None | Some("help" | "-h" | "--help") => {
            print_help();
            Ok(())
        }
        Some(other) => Err(anyhow!("unknown task `{other}`; run `cargo xtask help`")),
    }
}

fn print_help() {
    println!("cargo xtask <task>");
    println!();
    println!("  ci       layers, codegen --check, cargo fmt --check, clippy -D warnings, test");
    println!("  layers   enforce the crate layer graph, sources and resolved deps (see AGENTS.md)");
    println!(
        "  codegen  write crates/esm/src/generated/ from the schema; --check fails when stale"
    );
    println!("  parity   run `kit coverage` over the corpus (KITKRAFT_DATA_DIR) and write");
    println!("           docs/record-parity.md; --check fails when it is stale");
    println!(
        "  inventory-def <SIG> --name <TypeName> [--ticket KK-NNN] [--evidence C]... [--force]"
    );
    println!(
        "           write crates/schema/defs/<SIG>.toml from a corpus scan (KITKRAFT_DATA_DIR):"
    );
    println!("           corpus order and cardinality, cited counts; TODO(review) where unsure");
    println!("  add-def <SIG>... [--no-parity]");
    println!("           register definitions (BUILTIN_DEFINITIONS, TYPES, the defs test lists,");
    println!("           WITHOUT_EDID), then codegen, the golden seed and parity; idempotent");
    println!("  inventory-check <SIG>...");
    println!("           fail when a definition cites a count the corpus scan does not show");
    println!("  observe  CK observation fixtures (observe/<date>-<id>/) to report.md/report.json;");
    println!("           `observe new <F-id>` writes a fixture.toml template (lane H)");
    println!(
        "  dist     release-build kit.exe and kit-ui.exe into <out>\\KitKraft\\ (default dist\\)"
    );
    println!("           with the double-click .cmd files and README.txt; [--out DIR]");
    println!("           [--sync-bin] (also refresh the main checkout's bin\\); running programs");
    println!("           are renamed aside to *.stale-<secs>, never stopped");
    println!("  ui       release-build kit and kit-ui, then open the window with `kit ui --dev`");
    println!("           (the KitKraft Dev profile) unless the arguments name --profile or");
    println!("           --plugins; other arguments pass to `kit ui`; --no-dev opens the picker");
    println!("  oracle   `oracle spriggit [--official] [--synthetic] [<plugin>...]`: Spriggit");
    println!("           round trips compared with kit (YAML never read; lane H);");
    println!("           `oracle sf1edit [--official] [<plugin>...] [--types ..]`: xEdit's");
    println!("           decoded view (its console dumper) compared with kit inspect;");
    println!("           `oracle papyrus [<psc>...] [--sources DIR] [--optimize]`: the official");
    println!("           Papyrus compiler vs kit papyrus compile, normalized disassembly diff;");
    println!("           `oracle ck <fixture-dir>... [--out DIR] [--check]`: kitkraft.esm vs the");
    println!("           CK's ck.esm, every difference classed (lane A, M5.A2)");
}

/// The optional `--check` argument of `task` (`cargo xtask <task> [--check]`).
fn check_flag(task: &str) -> Result<bool> {
    match std::env::args().nth(2).as_deref() {
        None => Ok(false),
        Some("--check") => Ok(true),
        Some(other) => bail!("{task}: unknown argument `{other}`; expected --check"),
    }
}

/// The checkout this run checks, resolved at run time and printed.
///
/// A target dir shared between worktrees can hold an xtask binary built from another
/// checkout; its compile-time `CARGO_MANIFEST_DIR` would then point the checks at the
/// wrong tree. The root comes from the runtime `CARGO_MANIFEST_DIR` (`cargo run`, so
/// `cargo xtask`, sets it to the xtask package being run), else from
/// `cargo locate-project --workspace` in the working directory; a binary whose embedded
/// root differs is refused as stale.
fn workspace_root() -> Result<PathBuf> {
    let embedded = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = std::env::var_os("CARGO_MANIFEST_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let cwd = std::env::current_dir().context("no working directory")?;
    let root = resolve_root(embedded, runtime.as_deref(), &cwd)?;
    println!("xtask: workspace root {}", root.display());
    Ok(root)
}

/// The workspace root for an xtask built from `embedded_xtask`, run as `runtime_xtask`
/// (the runtime `CARGO_MANIFEST_DIR`) or, without one, from `cwd`. Fails when the two
/// roots differ (a stale binary from another checkout).
fn resolve_root(
    embedded_xtask: &Path,
    runtime_xtask: Option<&Path>,
    cwd: &Path,
) -> Result<PathBuf> {
    let embedded_root = embedded_xtask
        .parent()
        .context("xtask has no parent directory")?;
    let runtime_root = match runtime_xtask {
        Some(dir) => dir
            .parent()
            .map(Path::to_path_buf)
            .context("CARGO_MANIFEST_DIR has no parent directory")?,
        None => locate_workspace(cwd)?,
    };
    if !same_dir(embedded_root, &runtime_root) {
        bail!(
            "stale xtask binary: built from {} but run for {}; rebuild it from this checkout \
             (`cargo xtask` does) or use a target dir of your own",
            embedded_root.display(),
            runtime_root.display()
        );
    }
    Ok(runtime_root)
}

/// The directory of the workspace manifest above `cwd` (`cargo locate-project`).
fn locate_workspace(cwd: &Path) -> Result<PathBuf> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(&cargo)
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .current_dir(cwd)
        .output()
        .with_context(|| format!("could not start `{cargo} locate-project`"))?;
    if !output.status.success() {
        bail!(
            "`cargo locate-project --workspace` failed in {}: {}",
            cwd.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let manifest = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("workspace manifest has no parent directory")
}

/// Whether `a` and `b` name the same directory (canonical when both exist).
fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn ci(root: &Path) -> Result<()> {
    println!("xtask ci: layers");
    layers(root)?;
    println!("xtask ci: codegen --check");
    codegen::run(root, true)?;
    cargo(root, &["fmt", "--all", "--", "--check"])?;
    cargo(
        root,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    cargo(root, &["test", "--workspace"])?;
    println!("xtask ci: all gates passed");
    Ok(())
}

fn cargo(root: &Path, args: &[&str]) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let shown = args.join(" ");
    println!("xtask ci: cargo {shown}");
    let status = Command::new(&cargo)
        .args(args)
        .current_dir(root)
        .status()
        .with_context(|| format!("could not start `{cargo}`"))?;
    if !status.success() {
        bail!("`cargo {shown}` failed ({status})");
    }
    Ok(())
}

fn layers(root: &Path) -> Result<()> {
    let packages = load_workspace(root)?;
    let by_name: BTreeMap<&str, &Package> = packages.iter().map(|p| (p.name.as_str(), p)).collect();
    let mut errors: Vec<String> = Vec::new();

    for pkg in &packages {
        let at = pkg.manifest.display();
        match LAYER_TABLE.iter().find(|(name, _)| *name == pkg.name) {
            None => errors.push(format!(
                "{at}: `{}` is not in xtask LAYER_TABLE; add it there and to AGENTS.md",
                pkg.name
            )),
            Some((_, expected)) if *expected != pkg.layer => errors.push(format!(
                "{at}: `{}` is marked {} but LAYER_TABLE says {expected}",
                pkg.name, pkg.layer
            )),
            Some(_) => {}
        }
        if !pkg.workspace_lints {
            errors.push(format!("{at}: missing `[lints] workspace = true`"));
        }
        for dep in &pkg.deps {
            if let Some(target) = by_name.get(dep.as_str()) {
                if let Some(why) = edge_violation(&pkg.name, pkg.layer, &target.name, target.layer)
                {
                    errors.push(format!("{at}: {why}"));
                }
                continue;
            }
            let is_library = matches!(pkg.layer, Layer::L(_));
            if is_library && banned_prefix(dep, LIBRARY_BANNED).is_some() {
                errors.push(format!(
                    "{at}: library crate `{}` depends on `{dep}`; use thiserror",
                    pkg.name
                ));
            }
            let below_ui = matches!(pkg.layer, Layer::L(n) if n < 5);
            if below_ui && banned_prefix(dep, UI_TOOLKIT).is_some() {
                errors.push(format!(
                    "{at}: `{}` ({}) depends on UI toolkit `{dep}`; only L5 and apps may",
                    pkg.name, pkg.layer
                ));
            }
        }
        errors.extend(smuggle::scan_package(&pkg.dir, &pkg.name)?);
    }

    let layer_of: BTreeMap<String, Layer> =
        packages.iter().map(|p| (p.name.clone(), p.layer)).collect();
    let resolved = graph::load(root)?;
    errors.extend(graph::check(
        &resolved,
        &layer_of,
        graph::DEV_ALLOWED,
        graph::BUILD_RS_ALLOWED,
    ));

    if !errors.is_empty() {
        for error in &errors {
            eprintln!("layers: {error}");
        }
        bail!("{} layer violation(s)", errors.len());
    }

    println!(
        "layers: ok ({} workspace packages, {} in the resolved graph, sources clean)",
        packages.len(),
        resolved.package_count()
    );
    for pkg in &packages {
        let internal: Vec<&str> = pkg
            .deps
            .iter()
            .map(String::as_str)
            .filter(|d| by_name.contains_key(d))
            .collect();
        let layer = pkg.layer.to_string();
        println!("  {layer:<5} {:<18} -> {}", pkg.name, internal.join(", "));
    }
    Ok(())
}

/// Why the dependency edge `from -> to` between two workspace packages is illegal, if it is.
fn edge_violation(from: &str, from_layer: Layer, to: &str, to_layer: Layer) -> Option<String> {
    match (from_layer, to_layer) {
        (_, Layer::App | Layer::Tool) => Some(format!(
            "{from} ({from_layer}) depends on {to} ({to_layer}); nothing may depend on apps or tools"
        )),
        (Layer::App | Layer::Tool, Layer::L(_)) => None,
        (Layer::L(a), Layer::L(b)) if b < a => None,
        (Layer::L(a), Layer::L(b)) if b == a => {
            let allowed = SAME_LAYER_ALLOWED
                .iter()
                .any(|(f, t)| *f == from && *t == to);
            if allowed {
                None
            } else {
                Some(format!(
                    "{from} -> {to}: same-layer ({from_layer}) dependency not in SAME_LAYER_ALLOWED"
                ))
            }
        }
        (Layer::L(_), Layer::L(_)) => Some(format!(
            "{from} ({from_layer}) -> {to} ({to_layer}): upward dependency"
        )),
    }
}

fn parse_layer(text: &str) -> Option<Layer> {
    match text.trim() {
        "APP" => Some(Layer::App),
        "TOOL" => Some(Layer::Tool),
        other => {
            let n: u8 = other.strip_prefix('L')?.parse().ok()?;
            (n <= 5).then_some(Layer::L(n))
        }
    }
}

fn read_layer(dir: &Path) -> Result<Layer> {
    for file in ["src/lib.rs", "src/main.rs"] {
        let path = dir.join(file);
        if !path.is_file() {
            continue;
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("//! LAYER:") {
                return parse_layer(rest).with_context(|| {
                    format!("{}: bad layer marker `{}`", path.display(), rest.trim())
                });
            }
        }
        bail!(
            "{}: missing `//! LAYER: <L0..L5|APP|TOOL>` marker",
            path.display()
        );
    }
    bail!("{}: no src/lib.rs or src/main.rs", dir.display())
}

fn read_toml(path: &Path) -> Result<toml::Table> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str::<toml::Table>(&text).with_context(|| format!("parsing {}", path.display()))
}

fn load_workspace(root: &Path) -> Result<Vec<Package>> {
    let root_manifest = read_toml(&root.join("Cargo.toml"))?;
    let workspace = root_manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .context("root Cargo.toml has no [workspace] table")?;
    let empty = toml::Table::new();
    let ws_deps = workspace
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .unwrap_or(&empty);
    let members = workspace
        .get("members")
        .and_then(toml::Value::as_array)
        .context("[workspace] has no members array")?;

    let mut dirs = Vec::new();
    for member in members {
        let pattern = member
            .as_str()
            .context("workspace member is not a string")?;
        if let Some(parent) = pattern.strip_suffix("/*") {
            dirs.extend(child_packages(&root.join(parent))?);
        } else if pattern.contains('*') {
            bail!("unsupported workspace member glob `{pattern}`; use `dir/*` or a plain path");
        } else {
            dirs.push(root.join(pattern));
        }
    }
    dirs.iter().map(|dir| load_package(dir, ws_deps)).collect()
}

fn child_packages(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        if path.join("Cargo.toml").is_file() {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn load_package(dir: &Path, ws_deps: &toml::Table) -> Result<Package> {
    let manifest_path = dir.join("Cargo.toml");
    let manifest = read_toml(&manifest_path)?;
    let name = manifest
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .with_context(|| format!("{}: no package.name", manifest_path.display()))?
        .to_string();
    let workspace_lints = manifest
        .get("lints")
        .and_then(|l| l.get("workspace"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    Ok(Package {
        name,
        dir: dir.to_path_buf(),
        layer: read_layer(dir)?,
        deps: dependency_names(&manifest, ws_deps),
        workspace_lints,
        manifest: manifest_path,
    })
}

/// Real package names of normal and build dependencies, including `[target.*]` tables.
/// Resolves `package = "..."` renames, also when inherited with `workspace = true`.
fn dependency_names(manifest: &toml::Table, ws_deps: &toml::Table) -> Vec<String> {
    const KINDS: [&str; 2] = ["dependencies", "build-dependencies"];
    let mut tables: Vec<&toml::Table> = Vec::new();
    for kind in KINDS {
        if let Some(table) = manifest.get(kind).and_then(toml::Value::as_table) {
            tables.push(table);
        }
    }
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            for kind in KINDS {
                if let Some(table) = target.get(kind).and_then(toml::Value::as_table) {
                    tables.push(table);
                }
            }
        }
    }

    let mut names = Vec::new();
    for table in tables {
        for (key, spec) in table {
            names.push(real_name(key, spec, ws_deps));
        }
    }
    names
}

fn real_name(key: &str, spec: &toml::Value, ws_deps: &toml::Table) -> String {
    if let Some(package) = spec.get("package").and_then(toml::Value::as_str) {
        return package.to_string();
    }
    let inherited = spec
        .get("workspace")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if inherited {
        if let Some(package) = ws_deps
            .get(key)
            .and_then(|d| d.get("package"))
            .and_then(toml::Value::as_str)
        {
            return package.to_string();
        }
    }
    key.to_string()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{
        banned_prefix, dependency_names, edge_violation, parse_layer, Layer, LAYER_TABLE,
        LIBRARY_BANNED, UI_TOOLKIT,
    };

    #[test]
    fn ban_lists_match_by_prefix() {
        assert_eq!(banned_prefix("egui-winit", UI_TOOLKIT), Some("egui"));
        assert_eq!(banned_prefix("wgpu-core", UI_TOOLKIT), Some("wgpu"));
        assert_eq!(banned_prefix("slint", UI_TOOLKIT), Some("slint"));
        assert_eq!(banned_prefix("serde", UI_TOOLKIT), None);
        assert_eq!(banned_prefix("eyre", LIBRARY_BANNED), Some("eyre"));
        assert_eq!(banned_prefix("thiserror", LIBRARY_BANNED), None);
    }

    #[test]
    fn parses_layer_markers() {
        assert_eq!(parse_layer(" L0"), Some(Layer::L(0)));
        assert_eq!(parse_layer("L5 "), Some(Layer::L(5)));
        assert_eq!(parse_layer("L6"), None);
        assert_eq!(parse_layer("l1"), None);
        assert_eq!(parse_layer("APP"), Some(Layer::App));
        assert_eq!(parse_layer("TOOL"), Some(Layer::Tool));
        assert_eq!(parse_layer(""), None);
    }

    #[test]
    fn edge_rules() {
        let l = Layer::L;
        assert!(edge_violation("kitkraft-doc", l(1), "kitkraft-esm", l(0)).is_none());
        assert!(edge_violation("kitkraft-esm", l(0), "kitkraft-doc", l(1)).is_some());
        assert!(edge_violation("kitkraft-esm", l(0), "kitkraft-schema", l(0)).is_none());
        assert!(edge_violation("kitkraft-schema", l(0), "kitkraft-esm", l(0)).is_some());
        assert!(edge_violation("kitkraft-ba2", l(0), "kitkraft-pex", l(0)).is_some());
        assert!(edge_violation("kit-cli", Layer::App, "kitkraft-mcp", l(5)).is_none());
        assert!(edge_violation("kitkraft-mcp", l(5), "kit-cli", Layer::App).is_some());
        assert!(edge_violation("kit-cli", Layer::App, "xtask", Layer::Tool).is_some());
    }

    #[test]
    fn layer_table_has_no_duplicates() {
        let mut names: Vec<&str> = LAYER_TABLE.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), LAYER_TABLE.len());
    }

    #[test]
    fn resolves_renamed_and_inherited_dependencies() {
        let ws: toml::Table =
            toml::from_str(r#"aliased = { path = "x", package = "real-name" }"#).unwrap();
        let manifest: toml::Table = toml::from_str(
            r#"
[dependencies]
aliased = { workspace = true }
plain = "1"
renamed = { version = "1", package = "other" }

[target.'cfg(windows)'.dependencies]
winonly = "1"

[dev-dependencies]
ignored = "1"
"#,
        )
        .unwrap();
        let mut names = dependency_names(&manifest, &ws);
        names.sort();
        assert_eq!(names, ["other", "plain", "real-name", "winonly"]);
    }

    /// A minimal workspace (root manifest, `xtask` package) in a scratch directory: a
    /// second checkout, as a worktree sharing the target dir would be.
    fn scratch_checkout(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kitkraft-xtask-root-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("xtask").join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"xtask\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("xtask").join("Cargo.toml"),
            "[package]\nname = \"xtask\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("xtask").join("src").join("main.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn the_root_comes_from_the_runtime_manifest_dir_not_the_binary() {
        let this = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = super::resolve_root(this, Some(this), this).unwrap();
        assert!(
            root.join("xtask").join("Cargo.toml").is_file(),
            "{}",
            root.display()
        );

        // A binary built here, run for another checkout: refused, naming both roots.
        let other = scratch_checkout("runtime");
        let err = super::resolve_root(this, Some(&other.join("xtask")), this)
            .unwrap_err()
            .to_string();
        let _ = std::fs::remove_dir_all(&other);
        assert!(err.contains("stale xtask binary"), "{err}");
        assert!(err.contains(&other.display().to_string()), "{err}");
    }

    #[test]
    fn without_a_manifest_dir_the_working_directory_locates_the_workspace() {
        let this = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let here = super::resolve_root(this, None, this).unwrap();
        assert!(super::same_dir(&here, this.parent().unwrap()));

        let other = scratch_checkout("cwd");
        let located = super::locate_workspace(&other.join("xtask").join("src"))
            .map(|root| super::same_dir(&root, &other));
        let refused = super::resolve_root(this, None, &other.join("xtask"));
        let _ = std::fs::remove_dir_all(&other);
        assert!(located.unwrap(), "locate-project found another root");
        let err = refused.unwrap_err().to_string();
        assert!(err.contains("stale xtask binary"), "{err}");
    }
}
