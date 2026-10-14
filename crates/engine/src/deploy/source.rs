//! What a deployment copies: one plugin file, or a `package` folder (plugins at the root,
//! loose files and archives at their Data paths), hashed before anything is written.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::instance::{absolute, check_name, is_link};
use super::{io_error, refused, sha256, DeployError, MANIFEST, MO2_META, PACKAGE_MANIFEST};

/// Deepest folder nesting read from a package.
const MAX_DEPTH: usize = 32;
/// Most files read from a package.
const MAX_FILES: usize = 20_000;
/// Largest package manifest read.
const MAX_PACKAGE_MANIFEST: u64 = 16_777_216;

/// One file to deploy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFile {
    /// Path inside the mod folder, `/`-separated.
    pub rel: String,
    /// Where it is read from.
    pub abs: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256, hex.
    pub sha256: String,
}

/// A source read and hashed.
#[derive(Debug, Clone)]
pub(crate) struct Source {
    /// The `source` argument, absolute.
    pub root: PathBuf,
    /// `plugin` (one file) or `folder`.
    pub kind: &'static str,
    /// The plugin that names the mod (file name).
    pub plugin: String,
    /// Its file stem.
    pub stem: String,
    /// Every file to deploy, sorted by `rel`.
    pub files: Vec<SourceFile>,
    /// The package manifest and its SHA-256, when the folder has one.
    pub package_manifest: Option<(PathBuf, String)>,
    /// What was not checked.
    pub limitations: Vec<String>,
}

fn is_plugin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".esm", ".esp", ".esl"].iter().any(|e| lower.ends_with(e))
}

/// Reads `path` (a plugin file or a package folder), validates every name and hashes
/// every file.
pub(crate) fn read(path: &Path) -> Result<Source, DeployError> {
    let root = absolute(path)?;
    let meta = fs::symlink_metadata(&root)
        .map_err(|e| refused("source", &root, format!("cannot be read: {e}")))?;
    if meta.file_type().is_symlink() {
        return Err(refused(
            "reparse-point",
            &root,
            "the source is a symbolic link or junction; pass the real path",
        ));
    }
    if meta.is_file() {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !is_plugin(&name) {
            return Err(refused(
                "source",
                &root,
                "is not a plugin (.esm, .esp or .esl) or a package folder",
            ));
        }
        check_name(&name).map_err(|m| refused("plugin-name", &root, m))?;
        let (size, sha256) = sha256::file(&root).map_err(|e| io_error(&root, "hashing", &e))?;
        let stem = stem_of(&name);
        return Ok(Source {
            kind: "plugin",
            plugin: name.clone(),
            stem,
            files: vec![SourceFile {
                rel: name,
                abs: root.clone(),
                size,
                sha256,
            }],
            package_manifest: None,
            limitations: Vec::new(),
            root,
        });
    }
    let mut rels = Vec::new();
    walk(&root, &root, 0, &mut rels)?;
    rels.sort();
    let mut files = Vec::with_capacity(rels.len());
    let mut package_manifest = None;
    for (rel, abs) in rels {
        if rel.eq_ignore_ascii_case(PACKAGE_MANIFEST) {
            let (_, sha) = sha256::file(&abs).map_err(|e| io_error(&abs, "hashing", &e))?;
            package_manifest = Some((abs, sha));
            continue;
        }
        if rel.eq_ignore_ascii_case(MANIFEST) || rel.eq_ignore_ascii_case(MO2_META) {
            return Err(refused(
                "source-path",
                &abs,
                format!("`{rel}` at the package root is reserved (the deployment manifest and MO2's meta.ini)"),
            ));
        }
        let (size, sha256) = sha256::file(&abs).map_err(|e| io_error(&abs, "hashing", &e))?;
        files.push(SourceFile {
            rel,
            abs,
            size,
            sha256,
        });
    }
    let mut limitations = Vec::new();
    let listed_plugins = match &package_manifest {
        Some((path, _)) => verify_package(path, &files, &mut limitations)?,
        None => {
            limitations.push(format!(
                "{} has no {PACKAGE_MANIFEST}: the files were hashed as found, not checked against a package manifest",
                root.display()
            ));
            Vec::new()
        }
    };
    let roots: Vec<&SourceFile> = files
        .iter()
        .filter(|f| !f.rel.contains('/') && is_plugin(&f.rel))
        .collect();
    let plugin = match roots.as_slice() {
        [] => {
            return Err(refused(
                "source",
                &root,
                "has no plugin (.esm, .esp or .esl) at its root",
            ))
        }
        [one] => one.rel.clone(),
        several => listed_plugins
            .iter()
            .find(|p| several.iter().any(|f| f.rel.eq_ignore_ascii_case(p)))
            .cloned()
            .ok_or_else(|| {
                refused(
                    "source",
                    &root,
                    format!(
                        "has {} plugins at its root ({}) and no package manifest naming the first; deploy one plugin per dev mod",
                        several.len(),
                        several.iter().map(|f| f.rel.as_str()).collect::<Vec<_>>().join(", ")
                    ),
                )
            })?,
    };
    check_name(&plugin).map_err(|m| refused("plugin-name", &root.join(&plugin), m))?;
    Ok(Source {
        kind: "folder",
        stem: stem_of(&plugin),
        plugin,
        files,
        package_manifest,
        limitations,
        root,
    })
}

fn stem_of(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Files under `dir`, `/`-separated relative to `root`; every component must be a name
/// Windows accepts, and no entry may be a link.
fn walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), DeployError> {
    if depth > MAX_DEPTH {
        return Err(refused(
            "source-path",
            dir,
            format!("folders nest deeper than {MAX_DEPTH}"),
        ));
    }
    let rd = fs::read_dir(dir).map_err(|e| io_error(dir, "listing", &e))?;
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        check_name(&name).map_err(|m| refused("source-path", &path, m))?;
        if is_link(&path) {
            return Err(refused(
                "reparse-point",
                &path,
                "links and junctions are not deployed; copy the real file into the package",
            ));
        }
        let ft = entry
            .file_type()
            .map_err(|e| io_error(&path, "reading the type of", &e))?;
        if ft.is_dir() {
            walk(root, &path, depth.saturating_add(1), out)?;
        } else if ft.is_file() {
            if out.len() >= MAX_FILES {
                return Err(refused(
                    "source-path",
                    root,
                    format!("has more than {MAX_FILES} files"),
                ));
            }
            let rel = path
                .strip_prefix(root)
                .map_err(|_| refused("source-path", &path, "is outside the source folder"))?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((rel, path));
        }
    }
    Ok(())
}

/// Checks the files against the package manifest when its shape is recognized (an array
/// `files` of `{path, sha256, size?}`); returns its `plugins` list, if any (names, or
/// objects naming the plugin in `file` as `package` writes them, or in `name`). A manifest
/// of another shape is named in `limitations` and not used.
fn verify_package(
    path: &Path,
    files: &[SourceFile],
    limitations: &mut Vec<String>,
) -> Result<Vec<String>, DeployError> {
    let size = fs::metadata(path)
        .map_err(|e| io_error(path, "reading", &e))?
        .len();
    if size > MAX_PACKAGE_MANIFEST {
        return Err(refused(
            "package-manifest",
            path,
            format!("is larger than {MAX_PACKAGE_MANIFEST} bytes"),
        ));
    }
    let bytes = fs::read(path).map_err(|e| io_error(path, "reading", &e))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|e| refused("package-manifest", path, format!("is not JSON: {e}")))?;
    let plugins: Vec<String> = value
        .get("plugins")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    // `package` (M4.E5) writes `{"file": ..., "sha256": ..., ...}` entries.
                    p.as_str()
                        .or_else(|| p.get("file").and_then(Value::as_str))
                        .or_else(|| p.get("name").and_then(Value::as_str))
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    let Some(listed) = value.get("files").and_then(Value::as_array) else {
        limitations.push(format!(
            "{}: no `files` array (shape not recognized); the package was not verified against it",
            path.display()
        ));
        return Ok(plugins);
    };
    let mut expected: BTreeMap<String, (Option<u64>, String)> = BTreeMap::new();
    for entry in listed {
        let (Some(p), Some(sha)) = (
            entry.get("path").and_then(Value::as_str),
            entry.get("sha256").and_then(Value::as_str),
        ) else {
            limitations.push(format!(
                "{}: a `files` entry without `path` and `sha256` (shape not recognized); the package was not verified against it",
                path.display()
            ));
            return Ok(plugins);
        };
        expected.insert(
            p.replace('\\', "/").to_lowercase(),
            (
                entry.get("size").and_then(Value::as_u64),
                sha.to_ascii_lowercase(),
            ),
        );
    }
    for f in files {
        match expected.remove(&f.rel.to_lowercase()) {
            None => {
                return Err(refused(
                    "package-manifest",
                    &f.abs,
                    format!("`{}` is not listed in {PACKAGE_MANIFEST}", f.rel),
                ))
            }
            Some((size, sha)) => {
                if sha != f.sha256 || size.is_some_and(|s| s != f.size) {
                    return Err(refused(
                        "package-manifest",
                        &f.abs,
                        format!(
                            "does not match {PACKAGE_MANIFEST} (size {}, SHA-256 {}; listed {}, {sha})",
                            f.size,
                            f.sha256,
                            size.map_or_else(|| "?".to_string(), |s| s.to_string())
                        ),
                    ));
                }
            }
        }
    }
    if let Some(missing) = expected.keys().next() {
        return Err(refused(
            "package-manifest",
            path,
            format!("lists `{missing}`, which is not in the package"),
        ));
    }
    Ok(plugins)
}
