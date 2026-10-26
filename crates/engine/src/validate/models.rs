//! Model files (KK-018): `asset.missing-model`. A record of the validated plugin (a new
//! form or an override; never a master's record) names a model file in its `Model`
//! group's `FileName` member (MODL, the shared `Model` group). The file must be found
//! loose next to the plugin (its folder, as a package lays out `Data`) or in the game
//! Data folder, or as an entry of one of the load-order archives (the plugin's own
//! `<stem> - *.ba2`, then every other plugin's). Official MODL values are relative to
//! the top folder `meshes` (in the official DLC ESMs, every Model-group MODL value
//! resolves as `meshes/<value>`); names compare ASCII case-insensitively, with `/`
//! and backslash alike. An empty `FileName` is not a finding. The ARMO/ARMA world-model members are not the shared `Model` group (their
//! MODL is a FormID, not a path); a later ticket covers them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::attached::{normalize, Sources};
use super::{Finding, Severity};

/// A model path collected in the walk, searched after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    /// `SIGN:FORMID`.
    pub record: String,
    /// The `MODL` payload's offset in the file, when known.
    pub offset: Option<u64>,
    /// The stored `FileName`, as written (without its NUL).
    pub path: String,
}

/// Whether `sig`'s definition uses the shared `Model` group: a `Model.FileName` member
/// (MODL) holding the model path. Cached per signature in `cache`. ARMO and ARMA do
/// not use the group: their world-model members are FormIDs, not paths.
pub fn uses_model_group(sig: [u8; 4], cache: &mut HashMap<[u8; 4], bool>) -> bool {
    *cache.entry(sig).or_insert_with(|| {
        let Some(record_sig) = kitkraft_schema::Signature::from_bytes(sig) else {
            return false;
        };
        let Some(modl) = kitkraft_schema::Signature::from_bytes(*b"MODL") else {
            return false;
        };
        kitkraft_schema::builtin()
            .ok()
            .and_then(|schema| schema.record(record_sig))
            .is_some_and(|def| {
                def.leaf_subrecords()
                    .iter()
                    .any(|(path, sub)| path == "Model.FileName" && sub.signature == modl)
            })
    })
}

/// The zstring `FileName` value (the bytes before the first NUL), `None` when empty.
pub fn zstring(data: &[u8]) -> Option<String> {
    let end = match data.iter().position(|&b| b == 0) {
        Some(e) => e,
        None => data.len(),
    };
    let bytes = data.get(..end)?;
    if bytes.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// The path to search for: lower-case, `\` as `/`, under the top folder `meshes` unless
/// it already starts with it.
pub fn target_path(stored: &str) -> String {
    let n = normalize(stored);
    if n.starts_with("meshes/") {
        n
    } else {
        format!("meshes/{n}")
    }
}

/// The loose `meshes` folders to search: the plugin's folder (where a package lays out
/// `Data`), then the game Data folder.
fn loose_roots(home: Option<&Path>, data_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for base in [home.and_then(Path::parent), data_dir] {
        let Some(b) = base else {
            continue;
        };
        let root = b.join("meshes");
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

/// `rel` (inside a `meshes` folder) under `roots`, each path component matched ASCII
/// case-insensitively (loose files sit on a case-sensitive file system): the file, when
/// it is one.
fn loose_find(roots: &[PathBuf], rel: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    for root in roots {
        let mut cur = root.clone();
        let mut ok = true;
        for part in &parts {
            let Ok(rd) = std::fs::read_dir(&cur) else {
                ok = false;
                break;
            };
            let mut next: Option<PathBuf> = None;
            for e in rd {
                let Ok(e) = e else {
                    continue;
                };
                let name = e.file_name();
                let n = name.to_string_lossy();
                if n.eq_ignore_ascii_case(part) {
                    next = Some(cur.join(n.as_ref()));
                    break;
                }
            }
            let Some(n) = next else {
                ok = false;
                break;
            };
            cur = n;
        }
        if ok && cur.is_file() {
            return Some(cur);
        }
    }
    None
}

/// `asset.missing-model` over `list` (module docs), findings for `plugin`; `sources`
/// shares the load-order archive listing with the papyrus rules (their unreadable
/// archives are reported once, by the caller).
pub fn check(
    list: &[ModelRef],
    sources: &mut Sources,
    plugin: &str,
    home: Option<&Path>,
    data_dir: Option<&Path>,
    findings: &mut Vec<Finding>,
) {
    let roots = loose_roots(home, data_dir);
    let mut found: HashMap<String, bool> = HashMap::new();
    for m in list {
        let target = target_path(&m.path);
        let is_found = match found.get(&target) {
            Some(f) => *f,
            None => {
                // The roots are the `meshes` folders themselves; walk the rest.
                let rel = match target.strip_prefix("meshes/") {
                    Some(r) => r,
                    None => target.as_str(),
                };
                let f = loose_find(&roots, rel).is_some() || sources.entry_contains(&target);
                found.insert(target.clone(), f);
                f
            }
        };
        if is_found {
            continue;
        }
        findings.push(Finding {
            rule: "asset.missing-model",
            severity: Severity::Warning,
            plugin: plugin.to_string(),
            record: Some(m.record.clone()),
            subrecord: Some("MODL".to_string()),
            offset: m.offset,
            message: format!(
                "model `{}` is named by the record but no such file is found (searched {})",
                m.path,
                sources.searched_in(&roots)
            ),
        });
    }
}
