//! Model files (KK-018, KK-223): `asset.missing-model`. A record of the validated plugin
//! (a new form or an override; never a master's record) names a model file in its `Model`
//! group's `FileName` member (MODL, the shared `Model` group) or, for ARMO and ARMA, in
//! the `FileName` of a world-, biped- or first-person model member (KK-223). The file
//! must be found loose next to the plugin (its folder, as a package lays out `Data`) or
//! in the game Data folder, or as an entry of one of the load-order archives (the
//! plugin's own `<stem> - *.ba2`, then every other plugin's). Official values are
//! relative to the top folder `meshes` (in the official DLC ESMs, every value resolves
//! as `meshes/<value>`); names compare ASCII case-insensitively, with `/` and backslash
//! alike. An empty `FileName` is not a finding. ARMO's and ARMA's own `MODL` is not a
//! model path (ARMO's: a 4-byte armor-addon FormID; ARMA's: a race list), and ARMA's
//! alt-skeleton zstrings (MOD6/MOD7) are not model files, so neither is checked.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::attached::{normalize, Sources};
use super::{Finding, Severity};

/// A model path collected in the walk, searched after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    /// `SIGN:FORMID`.
    pub record: String,
    /// The subrecord signature holding the `FileName`.
    pub subrecord: [u8; 4],
    /// The `FileName` payload's offset in the file, when known.
    pub offset: Option<u64>,
    /// The member path, as the schema spells it (`Model.FileName`,
    /// `FemaleBipedModel.FileName`, ...).
    pub member: String,
    /// The stored `FileName`, as written (without its NUL).
    pub path: String,
}

/// The members of `sig`'s definition that hold a model file: the shared `Model` group's
/// `FileName` (MODL), and for ARMO and ARMA the `FileName` of their world-, biped- and
/// first-person model members. A list of `(subrecord signature, member path)` in the
/// definition's order, cached per signature in `cache`. Signatures collide across the
/// two record types (`MOD2` is ARMO's `MaleWorldModel` and ARMA's `MaleBipedModel`), so
/// the member is resolved from the record type's definition, not from the signature
/// alone. ARMA's alt-skeleton zstrings (MOD6/MOD7) and the two records' own `MODL`
/// (FormID/race list) are not model files and are not listed.
pub fn model_file_members(
    sig: [u8; 4],
    cache: &mut HashMap<[u8; 4], Vec<([u8; 4], String)>>,
) -> &Vec<([u8; 4], String)> {
    cache
        .entry(sig)
        .or_insert_with(|| {
            const MEMBERS: [&str; 7] = [
                "Model.FileName",
                "MaleWorldModel.FileName",
                "FemaleWorldModel.FileName",
                "MaleBipedModel.FileName",
                "FemaleBipedModel.FileName",
                "MaleFirstPerson.FileName",
                "FemaleFirstPerson.FileName",
            ];
            let Some(record_sig) = kitkraft_schema::Signature::from_bytes(sig) else {
                return Vec::new();
            };
            let Some(modl) = kitkraft_schema::Signature::from_bytes(*b"MODL") else {
                return Vec::new();
            };
            kitkraft_schema::builtin()
                .ok()
                .and_then(|schema| schema.record(record_sig))
                .map(|def| {
                    def.leaf_subrecords()
                        .into_iter()
                        .filter(|(path, sub)| {
                            MEMBERS.contains(&path.as_str())
                                && (path != "Model.FileName" || sub.signature == modl)
                        })
                        .map(|(path, sub)| (sub.signature.as_bytes(), path))
                        .collect()
                })
                .unwrap_or_default()
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
            subrecord: if m.member == "Model.FileName" {
                Some("MODL".to_string())
            } else {
                // The signatures collide across ARMO/ARMA, so the member path
                // disambiguates (e.g. `MOD3 (FemaleBipedModel.FileName)`).
                Some(format!("{} ({})", String::from_utf8_lossy(&m.subrecord), m.member))
            },
            offset: m.offset,
            message: if m.member == "Model.FileName" {
                format!(
                    "model `{}` is named by the record but no such file is found (searched {})",
                    m.path,
                    sources.searched_in(&roots)
                )
            } else {
                format!(
                    "model `{}` is named by the record's `{}` but no such file is found (searched {})",
                    m.path,
                    m.member,
                    sources.searched_in(&roots)
                )
            },
        });
    }
}
