//! Repository automation.
//!
//! - `cargo xtask ci`: the gate every commit passes (layers, fmt, clippy, test).
//! - `cargo xtask layers`: the layer check alone. Fast; compiles nothing.
//!
//! The layer check reads every workspace member's `Cargo.toml` and its `//! LAYER:` marker
//! (in `src/lib.rs` or `src/main.rs`) and fails on:
//! - a marker that is missing, malformed, or disagrees with [`LAYER_TABLE`];
//! - a package missing from [`LAYER_TABLE`];
//! - an upward dependency, or a same-layer dependency not in [`SAME_LAYER_ALLOWED`];
//! - any dependency on an app or tool package;
//! - `anyhow` in a library crate, or a UI toolkit below L5;
//! - a package without `[lints] workspace = true`.
//!
//! `[dependencies]`, `[build-dependencies]` and their `[target.*]` forms are checked.
//! `[dev-dependencies]` are exempt.
//!
//! LAYER: TOOL

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{anyhow, bail, Context, Result};

/// Canonical layer of every workspace package, including planned ones. Changing an entry
/// is an architecture change: it needs an ADR and the same edit to the table in AGENTS.md.
const LAYER_TABLE: &[(&str, Layer)] = &[
    ("kitcraft-schema", Layer::L(0)),
    ("kitcraft-esm", Layer::L(0)),
    ("kitcraft-ba2", Layer::L(0)),
    ("kitcraft-pex", Layer::L(0)),
    ("kitcraft-strings", Layer::L(0)),
    ("kitcraft-nif", Layer::L(0)),
    ("kitcraft-doc", Layer::L(1)),
    ("kitcraft-render", Layer::L(2)),
    ("kitcraft-io", Layer::L(3)),
    ("kitcraft-engine", Layer::L(4)),
    ("kitcraft-mcp", Layer::L(5)),
    ("kitcraft-ui-egui", Layer::L(5)),
    ("kit-cli", Layer::App),
    ("kit", Layer::App),
    ("xtask", Layer::Tool),
];

/// Same-layer edges that are allowed (`from`, `to`). Every entry is justified in
/// `plan/adr/0002-layering.md`.
const SAME_LAYER_ALLOWED: &[(&str, &str)] = &[("kitcraft-esm", "kitcraft-schema")];

/// External crates no library crate (L0..L5) may depend on.
const LIBRARY_BANNED: &[&str] = &["anyhow"];

/// UI toolkit crates nothing below L5 may depend on.
const UI_TOOLKIT: &[&str] = &["egui", "eframe", "egui-winit", "egui-wgpu", "winit"];

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
    manifest: PathBuf,
    layer: Layer,
    deps: Vec<String>,
    workspace_lints: bool,
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
    println!("  ci       layers, then cargo fmt --check, clippy -D warnings, test");
    println!("  layers   enforce the crate layer graph (see AGENTS.md)");
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask has no parent directory")
}

fn ci(root: &Path) -> Result<()> {
    println!("xtask ci: layers");
    layers(root)?;
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
            if is_library && LIBRARY_BANNED.contains(&dep.as_str()) {
                errors.push(format!(
                    "{at}: library crate `{}` depends on `{dep}`; use thiserror",
                    pkg.name
                ));
            }
            let below_ui = matches!(pkg.layer, Layer::L(n) if n < 5);
            if below_ui && UI_TOOLKIT.contains(&dep.as_str()) {
                errors.push(format!(
                    "{at}: `{}` ({}) depends on UI toolkit `{dep}`; only L5 and apps may",
                    pkg.name, pkg.layer
                ));
            }
        }
    }

    if !errors.is_empty() {
        for error in &errors {
            eprintln!("layers: {error}");
        }
        bail!("{} layer violation(s)", errors.len());
    }

    println!("layers: ok ({} packages)", packages.len());
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
mod tests {
    use super::{dependency_names, edge_violation, parse_layer, Layer, LAYER_TABLE};

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
        assert!(edge_violation("kitcraft-doc", l(1), "kitcraft-esm", l(0)).is_none());
        assert!(edge_violation("kitcraft-esm", l(0), "kitcraft-doc", l(1)).is_some());
        assert!(edge_violation("kitcraft-esm", l(0), "kitcraft-schema", l(0)).is_none());
        assert!(edge_violation("kitcraft-schema", l(0), "kitcraft-esm", l(0)).is_some());
        assert!(edge_violation("kitcraft-ba2", l(0), "kitcraft-pex", l(0)).is_some());
        assert!(edge_violation("kit-cli", Layer::App, "kitcraft-mcp", l(5)).is_none());
        assert!(edge_violation("kitcraft-mcp", l(5), "kit-cli", Layer::App).is_some());
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
}
