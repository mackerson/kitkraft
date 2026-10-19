//! `kitkraft-deploy.json` (the ownership marker and manifest of one deployment), the
//! journal of an operation in flight, and checking a deployed folder against its
//! manifest.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::instance::is_link;
use super::{io_error, refused, sha256, DeployError, Touched, MANIFEST, MARKER, MO2_META};

/// Largest manifest or journal read.
const MAX_JSON: u64 = 16_777_216;
/// Deepest folder nesting checked in a deployed folder.
const MAX_DEPTH: usize = 32;
/// Most files checked in a deployed folder.
const MAX_FILES: usize = super::MAX_PACKAGE_FILES;
/// `kind` of the journal.
pub(crate) const JOURNAL_KIND: &str = "kitkraft-dev-journal";

/// One deployed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ManifestFile {
    /// Path inside the mod folder, `/`-separated.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256, hex.
    pub sha256: String,
    /// Where it was copied from.
    pub source: String,
}

/// Where a deployment came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ManifestSource {
    /// The `source` argument, absolute.
    pub path: String,
    /// `plugin` or `folder`.
    pub kind: String,
    /// The package manifest, when the folder had one.
    pub package_manifest: Option<String>,
    /// Its SHA-256.
    pub package_manifest_sha256: Option<String>,
}

/// `kitkraft-deploy.json`: written last into the staged folder, so a folder that has it
/// is complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Manifest {
    /// Always [`MARKER`]: the ownership marker.
    pub kind: String,
    /// Layout version of this file (1).
    pub format: u32,
    /// The mod folder's name (`KitKraft Dev - <stem>`).
    #[serde(rename = "mod")]
    pub mod_name: String,
    /// The plugin that names the mod.
    pub plugin: String,
    /// This deployment.
    pub deployment_id: String,
    /// The deployment this one was deployed over: live when it was written (or, with no
    /// live folder, the one `.previous` held), if any. History, not a pointer: a later
    /// rollback or redeploy does not update it, so it need not name what `.previous`
    /// holds now. That is `.previous`'s own manifest id (`deploy.status`:
    /// `previous.manifest.deployment_id`), the one `deploy.rollback` restores.
    pub previous_deployment_id: Option<String>,
    /// UTC, RFC 3339.
    pub deployed_at: String,
    /// The same, as Unix seconds.
    pub deployed_at_unix: u64,
    /// The KitKraft build that wrote it.
    pub kitkraft_version: String,
    /// The source.
    pub source: ManifestSource,
    /// Every deployed file, sorted by path.
    pub files: Vec<ManifestFile>,
    /// The version written to MO2's `meta.ini` (ADR 0009 Amendment 3); absent in manifests
    /// written before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl Manifest {
    /// Whether this manifest marks the folder as the dev mod `mod_name`.
    pub(crate) fn owns(&self, mod_name: &str) -> bool {
        self.kind == MARKER && self.mod_name.eq_ignore_ascii_case(mod_name)
    }

    /// Summary for reports.
    pub(crate) fn summary(&self) -> Value {
        json!({
            "deployment_id": self.deployment_id,
            "previous_deployment_id": self.previous_deployment_id,
            "deployed_at": self.deployed_at,
            "plugin": self.plugin,
            "source": self.source.path,
            "files": self.files.len(),
            "bytes": self.files.iter().map(|f| f.size).fold(0u64, u64::saturating_add),
            "kitkraft_version": self.kitkraft_version,
        })
    }
}

/// An operation in flight: written before the first change, removed after the last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Journal {
    /// Always [`JOURNAL_KIND`].
    pub kind: String,
    /// The mod folder's name.
    #[serde(rename = "mod")]
    pub mod_name: String,
    /// `deploy` or `rollback`.
    pub operation: String,
    /// `in-progress` while the journal exists.
    pub state: String,
    /// UTC, RFC 3339.
    pub started_at: String,
    /// The deployment live after the commit.
    pub new_id: String,
    /// The deployment live before it.
    pub old_id: Option<String>,
    /// The deployment in `.previous` before it.
    pub older_id: Option<String>,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_JSON {
        return Err(format!("larger than {MAX_JSON} bytes"));
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// The manifest in `dir`: `None` when the file is absent, an error when it is unreadable.
pub(crate) fn read_manifest(dir: &Path) -> Result<Option<Manifest>, DeployError> {
    let path = dir.join(MANIFEST);
    if !path.is_file() {
        return Ok(None);
    }
    read_json(&path).map(Some).map_err(|m| {
        refused(
            "target-collision",
            &path,
            format!("unreadable deployment manifest: {m}"),
        )
    })
}

/// The journal at `path`, if any.
pub(crate) fn read_journal(path: &Path) -> Result<Option<Journal>, DeployError> {
    if !path.is_file() {
        return Ok(None);
    }
    read_json(path).map(Some).map_err(|m| {
        refused(
            "journal",
            path,
            format!("unreadable journal: {m}; move it aside after checking the folders by hand"),
        )
    })
}

/// Writes `bytes` to `path` through `<path>.tmp` + flush + rename.
pub(crate) fn write_atomic(
    path: &Path,
    bytes: &[u8],
    touched: &mut Vec<Touched>,
) -> Result<(), DeployError> {
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    {
        let mut f = fs::File::create(&tmp).map_err(|e| io_error(&tmp, "creating", &e))?;
        f.write_all(bytes)
            .and_then(|()| f.sync_all())
            .map_err(|e| io_error(&tmp, "writing", &e))?;
    }
    rename(&tmp, path)?;
    touched.push(Touched {
        action: "write",
        path: path.to_path_buf(),
        to: None,
    });
    Ok(())
}

/// Serializes `value` as pretty JSON with a trailing newline.
pub(crate) fn to_json<T: Serialize>(value: &T, path: &Path) -> Result<Vec<u8>, DeployError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| DeployError {
        rule: "io",
        path: Some(path.to_path_buf()),
        message: format!("serializing: {e}"),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// `fs::rename`, retried briefly when Windows reports the path as in use (an antivirus
/// scan of a file just written holds it for a moment).
pub(crate) fn rename(from: &Path, to: &Path) -> Result<(), DeployError> {
    let mut wait = 25u64;
    for _ in 0..6 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                std::thread::sleep(std::time::Duration::from_millis(wait));
                wait = wait.saturating_mul(2);
            }
            Err(e) => return Err(io_error(from, &format!("renaming to {}", to.display()), &e)),
        }
    }
    fs::rename(from, to).map_err(|e| {
        io_error(
            from,
            &format!(
                "renaming to {} (is the game, the CK or MO2 holding a file in it?)",
                to.display()
            ),
            &e,
        )
    })
}

/// Files under `dir` as `/`-separated relative paths; refuses links.
pub(crate) fn files_under(dir: &Path) -> Result<Vec<(String, PathBuf)>, DeployError> {
    let mut out = Vec::new();
    walk(dir, dir, 0, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), DeployError> {
    if depth > MAX_DEPTH {
        return Err(refused(
            "target-modified",
            dir,
            format!("folders nest deeper than {MAX_DEPTH}"),
        ));
    }
    let rd = fs::read_dir(dir).map_err(|e| io_error(dir, "listing", &e))?;
    for entry in rd.flatten() {
        let path = entry.path();
        if is_link(&path) {
            return Err(refused(
                "reparse-point",
                &path,
                "a link or junction inside a dev mod folder; remove it by hand",
            ));
        }
        let ft = entry
            .file_type()
            .map_err(|e| io_error(&path, "reading the type of", &e))?;
        if ft.is_dir() {
            walk(root, &path, depth.saturating_add(1), out)?;
        } else {
            if out.len() >= MAX_FILES {
                return Err(refused(
                    "target-modified",
                    root,
                    format!("has more than {MAX_FILES} files"),
                ));
            }
            let rel = path
                .strip_prefix(root)
                .map(|r| {
                    r.components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .unwrap_or_default();
            out.push((rel, path));
        }
    }
    Ok(())
}

/// How a deployed folder differs from its manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Integrity {
    /// Listed files whose size or hash changed (edited by hand).
    pub modified: Vec<String>,
    /// Listed files that are gone.
    pub missing: Vec<String>,
    /// Files no manifest lists (other than the manifest and MO2's `meta.ini`).
    pub unmanaged: Vec<String>,
    /// MO2's `meta.ini`, when present (carried over, never listed).
    pub meta_ini: bool,
}

impl Integrity {
    /// Nothing changed by hand.
    pub(crate) fn clean(&self) -> bool {
        self.modified.is_empty() && self.missing.is_empty() && self.unmanaged.is_empty()
    }

    /// As JSON.
    pub(crate) fn json(&self) -> Value {
        json!({
            "clean": self.clean(),
            "modified": self.modified,
            "missing": self.missing,
            "unmanaged": self.unmanaged,
            "meta_ini": self.meta_ini,
        })
    }

    /// One line naming what changed.
    pub(crate) fn describe(&self) -> String {
        let mut parts = Vec::new();
        for (what, list) in [
            ("modified", &self.modified),
            ("missing", &self.missing),
            ("unmanaged", &self.unmanaged),
        ] {
            if !list.is_empty() {
                parts.push(format!("{what}: {}", list.join(", ")));
            }
        }
        parts.join("; ")
    }
}

/// Compares `dir` with `manifest` (sizes, then SHA-256 of every listed file).
pub(crate) fn check(dir: &Path, manifest: &Manifest) -> Result<Integrity, DeployError> {
    let mut listed: BTreeMap<String, &ManifestFile> = manifest
        .files
        .iter()
        .map(|f| (f.path.to_lowercase(), f))
        .collect();
    let mut out = Integrity::default();
    for (rel, path) in files_under(dir)? {
        let key = rel.to_lowercase();
        if key == MANIFEST {
            continue;
        }
        if key == MO2_META {
            out.meta_ini = true;
            continue;
        }
        match listed.remove(&key) {
            None => out.unmanaged.push(rel),
            Some(f) => {
                let same_size = fs::metadata(&path).is_ok_and(|m| m.len() == f.size);
                let same = same_size
                    && sha256::file(&path)
                        .map_err(|e| io_error(&path, "hashing", &e))?
                        .1
                        == f.sha256;
                if !same {
                    out.modified.push(rel);
                }
            }
        }
    }
    out.missing = listed.into_values().map(|f| f.path.clone()).collect();
    Ok(out)
}
