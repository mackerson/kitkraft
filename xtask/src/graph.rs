//! The `cargo metadata` half of `cargo xtask layers`.
//!
//! The TOML half reads each manifest's direct dependencies, which is fast but sees only what
//! is written there. This half asks cargo for what it actually resolves:
//!
//! - every library package's **transitive normal-dependency closure** is checked against the
//!   ban lists (UI toolkits below L5; `anyhow`/`eyre` in any library), by name prefix, so a
//!   banned crate cannot arrive through a helper crate or a rename;
//! - **dev-dependencies**: none may point at an app or tool package; none from L0..L4 may
//!   reach a UI toolkit; an internal dev-dependency that is not a legal normal edge (upward,
//!   or an unlisted same-layer edge) needs an entry in [`DEV_ALLOWED`];
//! - **build scripts**: any package with a build script fails unless listed in
//!   [`BUILD_RS_ALLOWED`];
//! - the workspace members cargo sees must be exactly the packages the TOML half loaded.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::{banned_prefix, edge_violation, Layer, LIBRARY_BANNED, UI_TOOLKIT};

/// Internal dev-dependency edges (`from`, `to`) allowed although the same edge would be
/// illegal as a normal dependency. Every entry needs a reason in `plan/adr/0002-layering.md`.
pub const DEV_ALLOWED: &[(&str, &str)] = &[];

/// Workspace packages restricted to a fixed set of workspace dependencies (normal or dev),
/// whatever their layers would allow. The workbench UI depends on the engine alone, so the
/// toolkit stays swappable and every action goes through `Engine::execute` (ADR 0007
/// Amendment 1; reason in `plan/adr/0002-layering.md`).
pub const INTERNAL_ONLY: &[(&str, &[&str])] = &[("kitkraft-ui-egui", &["kitkraft-engine"])];

/// Packages allowed to have a build script. Every entry needs an ADR: build scripts run
/// arbitrary code at build time and can generate or pull in source the layer check never sees.
pub const BUILD_RS_ALLOWED: &[&str] = &[];

/// What cargo resolved, reduced to what the layer rules need. Package ids are cargo's opaque
/// id strings.
#[derive(Debug, Default)]
pub struct Graph {
    /// Package id -> package name, for every package in the resolved graph.
    names: BTreeMap<String, String>,
    /// Workspace member ids.
    members: Vec<String>,
    /// Package id -> ids it depends on through a normal (not dev, not build) edge.
    normal: BTreeMap<String, BTreeSet<String>>,
    /// Workspace member id -> ids of its dev-dependencies.
    dev: BTreeMap<String, BTreeSet<String>>,
    /// Workspace member name -> build script paths cargo found for it.
    build_scripts: BTreeMap<String, Vec<String>>,
}

/// Runs `cargo metadata --no-deps` (workspace packages and their targets) and the full
/// `cargo metadata` (the resolved graph) in `root`.
pub fn load(root: &Path) -> Result<Graph> {
    let no_deps = metadata(root, &["--no-deps"])?;
    let full = metadata(root, &[])?;
    Graph::from_json(&no_deps, &full)
}

fn metadata(root: &Path, extra: &[&str]) -> Result<Value> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(&cargo)
        .args(["metadata", "--format-version", "1"])
        .args(extra)
        .current_dir(root)
        .output()
        .with_context(|| format!("could not start `{cargo} metadata`"))?;
    if !output.status.success() {
        bail!(
            "`cargo metadata {}` failed ({}): {}",
            extra.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("parsing `cargo metadata` output")
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .with_context(|| format!("cargo metadata: `{key}` is missing or not an array"))
}

fn string(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("cargo metadata: `{key}` is missing or not a string"))
}

impl Graph {
    /// Builds the graph from the two `cargo metadata` documents.
    pub fn from_json(no_deps: &Value, full: &Value) -> Result<Graph> {
        let mut graph = Graph::default();
        for pkg in array(full, "packages")? {
            graph.names.insert(string(pkg, "id")?, string(pkg, "name")?);
        }
        for member in array(full, "workspace_members")? {
            let id = member
                .as_str()
                .context("cargo metadata: workspace member id is not a string")?;
            graph.members.push(id.to_string());
        }
        for pkg in array(no_deps, "packages")? {
            let name = string(pkg, "name")?;
            for target in array(pkg, "targets")? {
                let is_build_script = target
                    .get("kind")
                    .and_then(Value::as_array)
                    .is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("custom-build")));
                if is_build_script {
                    graph
                        .build_scripts
                        .entry(name.clone())
                        .or_default()
                        .push(string(target, "src_path")?);
                }
            }
        }
        let resolve = full
            .get("resolve")
            .filter(|r| !r.is_null())
            .context("cargo metadata has no resolve graph")?;
        for node in array(resolve, "nodes")? {
            let id = string(node, "id")?;
            for dep in array(node, "deps")? {
                let pkg = string(dep, "pkg")?;
                for kind in array(dep, "dep_kinds")? {
                    let edges = match kind.get("kind").and_then(Value::as_str) {
                        None => &mut graph.normal,
                        Some("dev") => &mut graph.dev,
                        Some(_) => continue,
                    };
                    edges.entry(id.clone()).or_default().insert(pkg.clone());
                }
            }
        }
        Ok(graph)
    }

    /// Number of packages cargo resolved, workspace members included.
    pub fn package_count(&self) -> usize {
        self.names.len()
    }

    fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.names.get(id).map_or(id, String::as_str)
    }

    /// Every package reachable from `start` through normal edges, `start` included, with the
    /// package it was first reached from (breadth first, so chains are shortest).
    fn closure(&self, start: &str) -> BTreeMap<String, Option<String>> {
        let mut parent: BTreeMap<String, Option<String>> = BTreeMap::new();
        parent.insert(start.to_string(), None);
        let mut queue = VecDeque::from([start.to_string()]);
        while let Some(id) = queue.pop_front() {
            for next in self.normal.get(&id).into_iter().flatten() {
                if !parent.contains_key(next) {
                    parent.insert(next.clone(), Some(id.clone()));
                    queue.push_back(next.clone());
                }
            }
        }
        parent
    }

    /// `a -> b -> c` for the path `closure` recorded to `id`.
    fn chain(&self, parents: &BTreeMap<String, Option<String>>, id: &str) -> String {
        let mut names = vec![self.name(id).to_string()];
        let mut at = id;
        while let Some(Some(up)) = parents.get(at) {
            names.push(self.name(up).to_string());
            at = up;
        }
        names.reverse();
        names.join(" -> ")
    }
}

/// Checks the resolved graph. `layers` maps every package the TOML half loaded to its layer.
pub fn check(
    graph: &Graph,
    layers: &BTreeMap<String, Layer>,
    dev_allowed: &[(&str, &str)],
    build_allowed: &[&str],
) -> Vec<String> {
    check_with(graph, layers, dev_allowed, build_allowed, INTERNAL_ONLY)
}

/// [`check`] with the [`INTERNAL_ONLY`] table passed in (tests use their own).
pub fn check_with(
    graph: &Graph,
    layers: &BTreeMap<String, Layer>,
    dev_allowed: &[(&str, &str)],
    build_allowed: &[&str],
    internal_only: &[(&str, &[&str])],
) -> Vec<String> {
    let mut errors = Vec::new();

    let member_names: BTreeSet<&str> = graph.members.iter().map(|id| graph.name(id)).collect();
    for name in &member_names {
        if !layers.contains_key(*name) {
            errors.push(format!(
                "cargo sees workspace member `{name}` but the manifest scan did not load it"
            ));
        }
    }
    for name in layers.keys() {
        if !member_names.contains(name.as_str()) {
            errors.push(format!(
                "`{name}` was loaded from the manifests but cargo does not list it as a workspace member"
            ));
        }
    }

    for (name, scripts) in &graph.build_scripts {
        if !build_allowed.contains(&name.as_str()) {
            errors.push(format!(
                "`{name}` has a build script ({}); build scripts are not allowed (BUILD_RS_ALLOWED)",
                scripts.join(", ")
            ));
        }
    }

    for id in &graph.members {
        let name = graph.name(id);
        let Some(&layer) = layers.get(name) else {
            continue;
        };

        if let Layer::L(n) = layer {
            let parents = graph.closure(id);
            for reached in parents.keys().filter(|r| *r != id) {
                let dep = graph.name(reached);
                if let Some(prefix) = banned_prefix(dep, LIBRARY_BANNED) {
                    errors.push(format!(
                        "library `{name}` ({layer}) pulls `{dep}` (banned `{prefix}*`): {}",
                        graph.chain(&parents, reached)
                    ));
                }
                if n < 5 {
                    if let Some(prefix) = banned_prefix(dep, UI_TOOLKIT) {
                        errors.push(format!(
                            "`{name}` ({layer}) pulls UI toolkit `{dep}` (banned `{prefix}*` below L5): {}",
                            graph.chain(&parents, reached)
                        ));
                    }
                }
            }
        }

        if let Some((_, allowed)) = internal_only.iter().find(|(n, _)| *n == name) {
            let direct = graph.normal.get(id).into_iter().flatten();
            let dev = graph.dev.get(id).into_iter().flatten();
            let mut seen = BTreeSet::new();
            for dep_id in direct.chain(dev) {
                let dep = graph.name(dep_id);
                if layers.contains_key(dep) && !allowed.contains(&dep) && seen.insert(dep) {
                    errors.push(format!(
                        "`{name}` may depend only on {} among workspace crates (INTERNAL_ONLY), not `{dep}`",
                        allowed.join(", ")
                    ));
                }
            }
        }

        for dev_id in graph.dev.get(id).into_iter().flatten() {
            let dev = graph.name(dev_id);
            if let Some(&dev_layer) = layers.get(dev) {
                if let Some(why) = dev_edge_violation(name, layer, dev, dev_layer, dev_allowed) {
                    errors.push(why);
                }
            }
            if matches!(layer, Layer::L(n) if n < 5) {
                let parents = graph.closure(dev_id);
                for reached in parents.keys() {
                    let dep = graph.name(reached);
                    if let Some(prefix) = banned_prefix(dep, UI_TOOLKIT) {
                        errors.push(format!(
                            "`{name}` ({layer}) dev-depends on UI toolkit `{dep}` (banned `{prefix}*` below L5): {name} -(dev)-> {}",
                            graph.chain(&parents, reached)
                        ));
                    }
                }
            }
        }
    }
    errors
}

/// Why the dev-dependency edge `from -> to` between workspace packages is illegal, if it is.
fn dev_edge_violation(
    from: &str,
    from_layer: Layer,
    to: &str,
    to_layer: Layer,
    dev_allowed: &[(&str, &str)],
) -> Option<String> {
    if matches!(to_layer, Layer::App | Layer::Tool) {
        return Some(format!(
            "{from} ({from_layer}) dev-depends on {to} ({to_layer}); nothing may depend on apps or tools"
        ));
    }
    let why = edge_violation(from, from_layer, to, to_layer)?;
    if dev_allowed.iter().any(|(f, t)| *f == from && *t == to) {
        return None;
    }
    Some(format!("dev-dependency not in DEV_ALLOWED: {why}"))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{json, Value};

    use super::{check, check_with, Graph};
    use crate::Layer;

    /// Edge kinds in fixtures.
    const N: Option<&str> = None;
    const DEV: Option<&str> = Some("dev");
    const BUILD: Option<&str> = Some("build");

    /// Two fake `cargo metadata` documents. Ids are the package names. `members` are
    /// workspace members with their target kinds; `external` are other packages; `edges` are
    /// (from, to, kind).
    fn fixture(
        members: &[(&str, &[&str])],
        external: &[&str],
        edges: &[(&str, &str, Option<&str>)],
    ) -> Graph {
        let member_pkgs: Vec<Value> = members
            .iter()
            .map(|(name, kinds)| {
                let targets: Vec<Value> = kinds
                    .iter()
                    .map(|k| json!({ "kind": [k], "src_path": format!("{name}/{k}.rs") }))
                    .collect();
                json!({ "id": name, "name": name, "targets": targets })
            })
            .collect();
        let mut all_pkgs = member_pkgs.clone();
        all_pkgs.extend(
            external
                .iter()
                .map(|name| json!({ "id": name, "name": name, "targets": [] })),
        );
        let mut nodes: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
        for name in members
            .iter()
            .map(|(n, _)| *n)
            .chain(external.iter().copied())
        {
            nodes.entry(name).or_default();
        }
        for (from, to, kind) in edges {
            nodes
                .entry(*from)
                .or_default()
                .push(json!({ "pkg": to, "dep_kinds": [{ "kind": kind, "target": null }] }));
        }
        let nodes: Vec<Value> = nodes
            .into_iter()
            .map(|(id, deps)| json!({ "id": id, "deps": deps }))
            .collect();
        let ids: Vec<&str> = members.iter().map(|(n, _)| *n).collect();
        let no_deps = json!({ "packages": member_pkgs, "workspace_members": ids, "resolve": null });
        let full = json!({ "packages": all_pkgs, "workspace_members": ids, "resolve": { "nodes": nodes } });
        Graph::from_json(&no_deps, &full).unwrap()
    }

    fn layers(entries: &[(&str, Layer)]) -> BTreeMap<String, Layer> {
        entries.iter().map(|(n, l)| (n.to_string(), *l)).collect()
    }

    const LIB: &[&str] = &["lib"];

    #[test]
    fn transitive_ui_toolkit_below_l5_is_caught_with_its_chain() {
        let graph = fixture(
            &[("doc", LIB), ("mcp", LIB)],
            &["helper", "winit-core", "wgpu-hal"],
            &[
                ("doc", "helper", N),
                ("helper", "winit-core", N),
                ("mcp", "wgpu-hal", N),
            ],
        );
        let errors = check(
            &graph,
            &layers(&[("doc", Layer::L(1)), ("mcp", Layer::L(5))]),
            &[],
            &[],
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].contains("doc -> helper -> winit-core"),
            "{errors:?}"
        );
    }

    #[test]
    fn every_ui_prefix_is_banned_below_l5() {
        for ui in [
            "egui_extras",
            "eframe",
            "winit",
            "wgpu-core",
            "rfd",
            "iced_wgpu",
            "slint-macros",
        ] {
            let graph = fixture(&[("esm", LIB)], &[ui], &[("esm", ui, N)]);
            let errors = check(&graph, &layers(&[("esm", Layer::L(0))]), &[], &[]);
            assert_eq!(errors.len(), 1, "{ui}: {errors:?}");
        }
    }

    #[test]
    fn anyhow_and_eyre_are_banned_transitively_in_libraries_only() {
        let graph = fixture(
            &[("mcp", LIB), ("cli", &["bin"])],
            &["glue", "anyhow", "eyre"],
            &[
                ("mcp", "glue", N),
                ("glue", "eyre", N),
                ("cli", "anyhow", N),
            ],
        );
        let errors = check(
            &graph,
            &layers(&[("mcp", Layer::L(5)), ("cli", Layer::App)]),
            &[],
            &[],
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("mcp -> glue -> eyre"), "{errors:?}");
    }

    #[test]
    fn build_and_dev_edges_are_not_in_the_normal_closure() {
        let graph = fixture(
            &[("esm", LIB)],
            &["anyhow", "egui"],
            &[("esm", "anyhow", BUILD), ("esm", "anyhow", DEV)],
        );
        let errors = check(&graph, &layers(&[("esm", Layer::L(0))]), &[], &[]);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn dev_dependencies_on_apps_and_tools_are_forbidden() {
        let graph = fixture(
            &[("engine", LIB), ("cli", &["bin"]), ("xtask", &["bin"])],
            &[],
            &[("engine", "cli", DEV), ("cli", "xtask", DEV)],
        );
        let l = layers(&[
            ("engine", Layer::L(4)),
            ("cli", Layer::App),
            ("xtask", Layer::Tool),
        ]);
        let errors = check(&graph, &l, &[("engine", "cli")], &[]);
        assert_eq!(errors.len(), 2, "{errors:?}");
    }

    #[test]
    fn upward_dev_dependencies_need_dev_allowed() {
        let graph = fixture(
            &[("esm", LIB), ("doc", LIB), ("engine", LIB)],
            &[],
            &[("esm", "doc", DEV), ("engine", "esm", DEV)],
        );
        let l = layers(&[
            ("esm", Layer::L(0)),
            ("doc", Layer::L(1)),
            ("engine", Layer::L(4)),
        ]);
        let errors = check(&graph, &l, &[], &[]);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("DEV_ALLOWED"), "{errors:?}");
        assert!(check(&graph, &l, &[("esm", "doc")], &[]).is_empty());
    }

    #[test]
    fn dev_dependencies_on_ui_toolkits_are_forbidden_below_l5() {
        let graph = fixture(
            &[("io", LIB), ("ui", LIB)],
            &["test-helper", "eframe"],
            &[
                ("io", "test-helper", DEV),
                ("test-helper", "eframe", N),
                ("ui", "eframe", DEV),
            ],
        );
        let errors = check(
            &graph,
            &layers(&[("io", Layer::L(3)), ("ui", Layer::L(5))]),
            &[],
            &[],
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].contains("io -(dev)-> test-helper -> eframe"),
            "{errors:?}"
        );
    }

    #[test]
    fn build_scripts_need_an_allowlist_entry() {
        let graph = fixture(&[("esm", &["lib", "custom-build"])], &[], &[]);
        let l = layers(&[("esm", Layer::L(0))]);
        let errors = check(&graph, &l, &[], &[]);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("build script"), "{errors:?}");
        assert!(check(&graph, &l, &[], &["esm"]).is_empty());
    }

    #[test]
    fn workspace_members_must_match_the_manifest_scan() {
        let graph = fixture(&[("esm", LIB), ("stray", LIB)], &[], &[]);
        let errors = check(
            &graph,
            &layers(&[("esm", Layer::L(0)), ("ghost", Layer::L(0))]),
            &[],
            &[],
        );
        assert_eq!(errors.len(), 2, "{errors:?}");
    }

    #[test]
    fn internal_only_packages_reach_only_their_listed_workspace_crates() {
        let graph = fixture(
            &[("ui", LIB), ("engine", LIB), ("doc", LIB)],
            &["eframe"],
            &[
                ("ui", "engine", N),
                ("ui", "eframe", N),
                ("ui", "doc", DEV),
                ("engine", "doc", N),
            ],
        );
        let l = layers(&[
            ("ui", Layer::L(5)),
            ("engine", Layer::L(4)),
            ("doc", Layer::L(1)),
        ]);
        let table: &[(&str, &[&str])] = &[("ui", &["engine"])];
        let errors = check_with(&graph, &l, &[], &[], table);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("`doc`"), "{errors:?}");
        // Without the table the same graph is legal (L5 -> L1 is downward).
        assert!(check_with(&graph, &l, &[], &[], &[]).is_empty());
        // The real table names the workbench and only the engine.
        assert_eq!(
            super::INTERNAL_ONLY,
            &[("kitkraft-ui-egui", &["kitkraft-engine"][..])]
        );
    }
}
