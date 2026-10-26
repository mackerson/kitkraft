//! Attached scripts (M4.E9): where a script attached through `VMAD` comes from, and what
//! properties it declares. `papyrus.missing-script` and `papyrus.property-unresolved`
//! read it; `place`, `set` and `new` use it to refuse an unknown script or property and
//! to type each property value by its declaration.
//!
//! **Where a script is found** (the order the game's own lookup is assumed to follow,
//! grade D): loose `Scripts\<Namespace>\<Name>.pex` next to the plugin, then in the game
//! Data folder; then the `.pex` entries of the plugin's own archives (`<stem> - *.ba2`);
//! then those of every other plugin in the load order (the base game's scripts are in
//! `Starfield - Misc.ba2`, an archive of `Starfield.esm`). Names compare ASCII
//! case-insensitively; `A:B` is `Scripts/A/B.pex`.
//!
//! **What a script declares**: the property table of its compiled `.pex` (name and type
//! name, `kitkraft-pex`), and its parent's, up the `extends` chain. A script in the chain
//! that is not found as a `.pex` is read from the base game's sources, the index
//! `kit papyrus events` builds from `Tools\ContentResources.zip` (property members with
//! their types), when that zip is there. A property may be set on the script that
//! declares it or on any script extending it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::scripts::plugin_archives;

/// Largest `.pex` read (the biggest official script is far below it).
const MAX_SCRIPT: u64 = 64 << 20;
/// Longest `extends` chain followed.
const MAX_CHAIN: usize = 32;

/// A declared property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// Its name as declared.
    pub name: String,
    /// Its type name (`Int`, `ObjectReference`, `Float[]`, `MyQuest:Entry`).
    pub ty: String,
    /// The script that declares it.
    pub script: String,
}

/// A script's own declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Decl {
    name: String,
    parent: Option<String>,
    properties: Vec<(String, String)>,
}

/// Where a script was found, or read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The script name as the caller gave it.
    pub script: String,
    /// `<loose path>`, `<archive>/<entry>` or `papyrus events (<zip>)`.
    pub at: String,
}

/// The script lookup for one plugin and its load order (lazy: archives are listed on the
/// first lookup that needs them).
pub struct Sources {
    loose: Vec<PathBuf>,
    archives: Vec<PathBuf>,
    data_dir: Option<PathBuf>,
    /// Lower-case `scripts/a/b.pex` -> (archive index, entry index), once listed.
    index: Option<HashMap<String, (usize, usize)>>,
    /// Lower-case name of every load-order archive entry (not just `.pex`) -> (archive
    /// index, entry index), once listed (the model rules' listing).
    entries: Option<HashMap<String, (usize, usize)>>,
    /// Archives that could not be read, as `path: error`.
    pub unreadable: Vec<String>,
    decls: HashMap<String, Option<(Decl, String)>>,
    psc: Option<Option<HashMap<String, Decl>>>,
    psc_zip: Option<PathBuf>,
}

/// `A:B` as the relative path of its compiled script, lower-case with `/`.
pub fn pex_path(script: &str) -> String {
    format!(
        "scripts/{}.pex",
        script.replace([':', '\\'], "/").to_ascii_lowercase()
    )
}

/// `A:B` under a loose `Scripts` folder: `<dir>\A\B.pex` (Windows file names compare
/// without case).
fn loose_path(dir: &Path, script: &str) -> PathBuf {
    let mut path = dir.to_path_buf();
    let parts: Vec<&str> = script.split([':', '\\', '/']).collect();
    for (i, part) in parts.iter().enumerate() {
        if i.saturating_add(1) == parts.len() {
            path.push(format!("{part}.pex"));
        } else {
            path.push(part);
        }
    }
    path
}

pub(super) fn normalize(entry: &str) -> String {
    entry.replace('\\', "/").to_ascii_lowercase()
}

/// Record in `unreadable` an archive that could not be read (once; shared by both index
/// listings). A free function: called through one field while another is borrowed by
/// the loop.
fn note_unreadable(unreadable: &mut Vec<String>, msg: String) {
    if !unreadable.contains(&msg) {
        unreadable.push(msg);
    }
}

impl Sources {
    /// The lookup for `plugin` (loose scripts next to it and its archives) and the other
    /// plugins of its load order (`others`: their files), with the game `data_dir`.
    pub fn new(plugin: Option<&Path>, others: &[PathBuf], data_dir: Option<&Path>) -> Sources {
        let mut loose = Vec::new();
        let mut archives: Vec<PathBuf> = Vec::new();
        let add = |p: PathBuf, list: &mut Vec<PathBuf>| {
            if !list.contains(&p) {
                list.push(p);
            }
        };
        if let Some(p) = plugin {
            if let Some(dir) = p.parent() {
                add(dir.join("Scripts"), &mut loose);
            }
            for a in plugin_archives(p) {
                add(a, &mut archives);
            }
        }
        if let Some(d) = data_dir {
            add(d.join("Scripts"), &mut loose);
        }
        for o in others {
            for a in plugin_archives(o) {
                add(a, &mut archives);
            }
        }
        Sources {
            loose,
            archives,
            data_dir: data_dir.map(Path::to_path_buf),
            index: None,
            entries: None,
            unreadable: Vec::new(),
            decls: HashMap::new(),
            psc: None,
            psc_zip: None,
        }
    }

    /// Where this lookup searches (`Scripts` loose, then the load-order archives), for
    /// messages.
    pub fn searched(&self) -> String {
        self.searched_in(&self.loose)
    }

    /// Where a lookup over `loose` folders and the load-order archives searched, for
    /// messages (the model rules' listing).
    pub fn searched_in(&self, loose: &[PathBuf]) -> String {
        let mut parts: Vec<String> = loose.iter().map(|p| p.display().to_string()).collect();
        parts.push(format!(
            "{} archive(s) of the load order",
            self.archives.len()
        ));
        parts.join(", ")
    }

    fn archive_index(&mut self) -> &HashMap<String, (usize, usize)> {
        if self.index.is_none() {
            let mut map = HashMap::new();
            for (ai, path) in self.archives.iter().enumerate() {
                match kitkraft_ba2::Archive::open(path) {
                    Ok((archive, _)) => {
                        for (ei, e) in archive.entries.iter().enumerate() {
                            let n = normalize(&e.name);
                            if n.ends_with(".pex") {
                                map.entry(n).or_insert((ai, ei));
                            }
                        }
                    }
                    Err(e) => {
                        note_unreadable(&mut self.unreadable, format!("{}: {e}", path.display()))
                    }
                }
            }
            self.index = Some(map);
        }
        self.index.get_or_insert_with(HashMap::new)
    }

    /// The load-order archive entries (every kind, not just `.pex`), once listed.
    fn entry_index(&mut self) -> &HashMap<String, (usize, usize)> {
        if self.entries.is_none() {
            let mut map = HashMap::new();
            for (ai, path) in self.archives.iter().enumerate() {
                match kitkraft_ba2::Archive::open(path) {
                    Ok((archive, _)) => {
                        for (ei, e) in archive.entries.iter().enumerate() {
                            map.entry(normalize(&e.name)).or_insert((ai, ei));
                        }
                    }
                    Err(e) => {
                        note_unreadable(&mut self.unreadable, format!("{}: {e}", path.display()))
                    }
                }
            }
            self.entries = Some(map);
        }
        self.entries.get_or_insert_with(HashMap::new)
    }

    /// Whether a load-order archive has an entry with normalized name `n` (every kind,
    /// not just `.pex`).
    pub(super) fn entry_contains(&mut self, n: &str) -> bool {
        self.entry_index().contains_key(n)
    }

    /// The compiled script `script`: where it is and its bytes.
    fn read_pex(&mut self, script: &str) -> Result<Option<(String, Vec<u8>)>, String> {
        let rel = pex_path(script);
        for dir in &self.loose {
            let path = loose_path(dir, script);
            if path.is_file() {
                let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                return Ok(Some((path.display().to_string(), bytes)));
            }
        }
        let Some((ai, ei)) = self.archive_index().get(&rel).copied() else {
            return Ok(None);
        };
        let path = self.archives.get(ai).cloned().unwrap_or_default();
        let (archive, mut file) =
            kitkraft_ba2::Archive::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = archive
            .entries
            .get(ei)
            .map(|e| e.name.clone())
            .unwrap_or_default();
        let bytes = archive
            .extract_vec(&mut file, ei, MAX_SCRIPT)
            .map_err(|e| format!("{}/{name}: {e}", path.display()))?;
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Some((format!("{file_name}/{name}"), bytes)))
    }

    /// Where `script` is (a `.pex` of the sources), or `None`.
    pub fn locate(&mut self, script: &str) -> Option<Found> {
        let rel = pex_path(script);
        for dir in &self.loose {
            let path = loose_path(dir, script);
            if path.is_file() {
                return Some(Found {
                    script: script.to_string(),
                    at: path.display().to_string(),
                });
            }
        }
        let (ai, ei) = self.archive_index().get(&rel).copied()?;
        let archive = self.archives.get(ai)?;
        let name = kitkraft_ba2::Archive::open(archive)
            .ok()
            .and_then(|(a, _)| a.entries.get(ei).map(|e| e.name.clone()))
            .unwrap_or(rel);
        Some(Found {
            script: script.to_string(),
            at: format!(
                "{}/{name}",
                archive
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ),
        })
    }

    /// The base game's source declarations (`kit papyrus events`'s index), once.
    fn psc(&mut self) -> Option<&HashMap<String, Decl>> {
        if self.psc.is_none() {
            let built = crate::handlers::papyrus_declarations(self.data_dir.as_deref());
            self.psc_zip = built.as_ref().map(|(z, _)| z.clone());
            self.psc = Some(built.map(|(_, scripts)| {
                scripts
                    .into_iter()
                    .map(|(name, parent, properties)| {
                        (
                            name.to_ascii_lowercase(),
                            Decl {
                                name,
                                parent,
                                properties,
                            },
                        )
                    })
                    .collect()
            }));
        }
        self.psc.as_ref().and_then(Option::as_ref)
    }

    /// `script`'s own declaration and where it was read.
    fn decl(&mut self, script: &str) -> Result<Option<(Decl, String)>, String> {
        let key = script.to_ascii_lowercase();
        if let Some(d) = self.decls.get(&key) {
            return Ok(d.clone());
        }
        let found = match self.read_pex(script)? {
            Some((at, bytes)) => {
                let pex = kitkraft_pex::Pex::parse(&bytes).map_err(|e| format!("{at}: {e}"))?;
                let object = pex
                    .objects
                    .first()
                    .ok_or_else(|| format!("{at}: no object"))?;
                let parent = Some(pex.string(object.parent).to_string()).filter(|p| !p.is_empty());
                let properties = object
                    .properties
                    .iter()
                    .map(|p| {
                        (
                            pex.string(p.name).to_string(),
                            pex.string(p.type_name).to_string(),
                        )
                    })
                    .collect();
                Some((
                    Decl {
                        name: pex.string(object.name).to_string(),
                        parent,
                        properties,
                    },
                    at,
                ))
            }
            None => {
                let zip = self.psc_zip.clone();
                self.psc().and_then(|m| m.get(&key)).cloned().map(|d| {
                    let zip = self
                        .psc_zip
                        .clone()
                        .or(zip)
                        .map(|z| z.display().to_string())
                        .unwrap_or_default();
                    (d, format!("papyrus events ({zip})"))
                })
            }
        };
        self.decls.insert(key, found.clone());
        Ok(found)
    }

    /// Every property `script` can set (its own, then its parents'), or `None` when the
    /// script itself is not found. A parent that is not found ends the chain
    /// ([`Chain::missing_parent`]).
    pub fn properties(&mut self, script: &str) -> Result<Option<Chain>, String> {
        let Some((first, at)) = self.decl(script)? else {
            return Ok(None);
        };
        let mut out = Vec::new();
        let mut next = Some(first);
        let mut missing = None;
        let mut seen: Vec<String> = Vec::new();
        while let Some(d) = next.take() {
            if seen.len() >= MAX_CHAIN || seen.contains(&d.name.to_ascii_lowercase()) {
                break;
            }
            seen.push(d.name.to_ascii_lowercase());
            for (name, ty) in &d.properties {
                out.push(Declared {
                    name: name.clone(),
                    ty: ty.clone(),
                    script: d.name.clone(),
                });
            }
            if let Some(p) = &d.parent {
                match self.decl(p)? {
                    Some((pd, _)) => next = Some(pd),
                    None => missing = Some(p.clone()),
                }
            }
        }
        Ok(Some(Chain {
            declared: out,
            missing_parent: missing,
            at,
        }))
    }
}

/// What a script and its parents declare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    /// The properties, the script's own first.
    pub declared: Vec<Declared>,
    /// A parent that was not found (its properties are unknown).
    pub missing_parent: Option<String>,
    /// Where the script itself was read.
    pub at: String,
}

impl Chain {
    /// The declared property `name` (ASCII case-insensitive, as Papyrus names compare).
    pub fn find(&self, name: &str) -> Option<&Declared> {
        self.declared
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
    }
}

/// Script names, each with its property names.
pub type ScriptNames = Vec<(String, Vec<String>)>;

/// The scripts one record attaches, as the papyrus rules check them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    /// `SIGN:FORMID`.
    pub record: String,
    /// The VMAD payload's offset in the file, when known.
    pub offset: Option<u64>,
    /// Each script with its stored properties as `(name, type byte)`.
    pub scripts: Vec<(String, Vec<(String, u8)>)>,
    /// The master whose version of the record (the previous one in load order) the
    /// override was read against, and that version's scripts with their property names:
    /// what the override carries from there is reported as a warning, not an error.
    pub inherited: Option<(String, ScriptNames)>,
}

impl Attached {
    /// The scripts of a decoded VMAD.
    pub fn from_vmad(record: &str, offset: Option<u64>, v: &crate::edit::vmad::Vmad) -> Attached {
        Attached {
            record: record.to_string(),
            offset,
            scripts: v
                .scripts
                .iter()
                .map(|s| {
                    (
                        s.name_text(),
                        s.properties
                            .iter()
                            .map(|p| (String::from_utf8_lossy(&p.name).into_owned(), p.ty))
                            .collect(),
                    )
                })
                .collect(),
            inherited: None,
        }
    }

    /// Records what `plugin`'s earlier version of the record attaches (`earlier`).
    pub fn inherit(&mut self, plugin: &str, earlier: &crate::edit::vmad::Vmad) {
        self.inherited = Some((
            plugin.to_string(),
            earlier
                .scripts
                .iter()
                .map(|s| {
                    (
                        s.name_text(),
                        s.properties
                            .iter()
                            .map(|p| String::from_utf8_lossy(&p.name).into_owned())
                            .collect(),
                    )
                })
                .collect(),
        ));
    }

    /// The master `script` (and, with `property`, that property of it) comes from
    /// unchanged in name, if the earlier version attaches it.
    fn inherited_from(&self, script: &str, property: Option<&str>) -> Option<&str> {
        let (plugin, scripts) = self.inherited.as_ref()?;
        let (_, props) = scripts
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case(script))?;
        match property {
            None => Some(plugin),
            Some(p) => props
                .iter()
                .any(|q| q.eq_ignore_ascii_case(p))
                .then_some(plugin.as_str()),
        }
    }
}

/// Whether a stored type byte fits a declared one: equal; a declared `Var` takes any; an
/// unset value (type 0) fits anything; a struct (7, 17) fits a declared type this lookup
/// read as an object (1, 11), since a source declaration names a struct type the way it
/// names a script.
fn fits(stored: u8, declared: u8) -> bool {
    stored == declared
        || declared == 6
        || declared == 16
        || stored == 0
        || (stored == 7 && declared == 1)
        || (stored == 17 && declared == 11)
}

/// `papyrus.missing-script` and `papyrus.property-unresolved` over the scripts the
/// plugin's records attach (module docs), with findings for `plugin`.
#[allow(clippy::too_many_arguments)]
pub fn check(
    list: &[Attached],
    sources: &mut Sources,
    plugin: &str,
    missing_on: bool,
    properties_on: bool,
    findings: &mut Vec<super::Finding>,
    limitations: &mut Vec<String>,
) {
    let mut chains: HashMap<String, Option<Chain>> = HashMap::new();
    let mut unknown_parents: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for a in list {
        // What the previous version in load order already attaches is carried, not added:
        // a warning (the game logs the problem; the override did not cause it).
        let finding = |rule: &'static str, from: Option<&str>, message: String| {
            match from {
            None => super::Finding {
                rule,
                severity: super::Severity::Error,
                plugin: plugin.to_string(),
                record: Some(a.record.clone()),
                subrecord: Some("VMAD".to_string()),
                offset: a.offset,
                message,
            },
            Some(master) => super::Finding {
                rule,
                severity: super::Severity::Warning,
                plugin: plugin.to_string(),
                record: Some(a.record.clone()),
                subrecord: Some("VMAD".to_string()),
                offset: a.offset,
                message: format!("{message}; carried unchanged from {master}'s version of the record, so a warning"),
            },
        }
        };
        for (script, props) in &a.scripts {
            let key = script.to_ascii_lowercase();
            if !chains.contains_key(&key) {
                let chain = match sources.properties(script) {
                    Ok(c) => c,
                    Err(e) => {
                        if !errors.contains(&e) {
                            errors.push(e);
                        }
                        None
                    }
                };
                chains.insert(key.clone(), chain);
            }
            let Some(chain) = chains.get(&key).and_then(Option::as_ref) else {
                if missing_on {
                    findings.push(finding(
                        "papyrus.missing-script",
                        a.inherited_from(script, None),
                        format!(
                            "script `{script}` is attached but no {} is found (searched {})",
                            pex_path(script),
                            sources.searched()
                        ),
                    ));
                }
                continue;
            };
            if !properties_on {
                continue;
            }
            for (name, stored) in props {
                match chain.find(name) {
                    Some(d) => {
                        let declared = type_byte(&d.ty, &|t: &str| t.contains('#'));
                        if !fits(*stored, declared) {
                            findings.push(finding(
                                "papyrus.property-unresolved",
                                a.inherited_from(script, Some(name)),
                                format!(
                                    "{script}.{name}: stored as type {stored}, but `{}` declares it {} (type {declared})",
                                    d.script, d.ty
                                ),
                            ));
                        }
                    }
                    None => match &chain.missing_parent {
                        Some(p) => {
                            let note = format!("`{script}` extends `{p}`, which is not found: its properties ({script}.{name}, ...) are not checked");
                            if !unknown_parents.iter().any(|u| u.starts_with(&format!("`{script}`"))) {
                                unknown_parents.push(note);
                            }
                        }
                        None => findings.push(finding(
                            "papyrus.property-unresolved",
                            a.inherited_from(script, Some(name)),
                            format!(
                                "{script}.{name}: `{script}` and its parents declare no such property (read from {})",
                                chain.at
                            ),
                        )),
                    },
                }
            }
        }
    }
    for e in errors {
        limitations.push(format!("papyrus rules: a script could not be read: {e}"));
    }
    if !unknown_parents.is_empty() {
        let shown: Vec<String> = unknown_parents.iter().take(5).cloned().collect();
        limitations.push(format!(
            "papyrus.property-unresolved: {} script(s) extend a script that is not found; {}",
            unknown_parents.len(),
            shown.join("; ")
        ));
    }
    // Unreadable archives are reported once, by the caller (they are shared with the
    // model rules' listing).
}

/// The VMAD type byte a declared type takes, or why it cannot be set: `Int` 3, `Float` 4,
/// `Bool` 5, `String` 2, `Var` 6, a struct type 7 (`Script:Struct`, when `structs` names
/// it), any other type name an object (1); `[]` adds 10.
pub fn type_byte(ty: &str, is_struct: &dyn Fn(&str) -> bool) -> u8 {
    let (base, array) = match ty.strip_suffix("[]") {
        Some(b) => (b, 10u8),
        None => (ty, 0u8),
    };
    let t: u8 = match base.to_ascii_lowercase().as_str() {
        "int" => 3,
        "float" => 4,
        "bool" => 5,
        "string" => 2,
        "var" => 6,
        _ if is_struct(base) => 7,
        _ => 1,
    };
    t.saturating_add(array)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn names_and_types() {
        assert_eq!(pex_path("KK:Hello"), "scripts/kk/hello.pex");
        assert_eq!(pex_path("ObjectReference"), "scripts/objectreference.pex");
        let none = |_: &str| false;
        assert_eq!(type_byte("Int", &none), 3);
        assert_eq!(type_byte("float[]", &none), 14);
        assert_eq!(type_byte("ObjectReference", &none), 1);
        assert_eq!(type_byte("Message[]", &none), 11);
        assert_eq!(type_byte("Q:Entry", &|s: &str| s == "Q:Entry"), 7);
        assert_eq!(type_byte("Q:Entry[]", &|s: &str| s == "Q:Entry"), 17);
    }
}
