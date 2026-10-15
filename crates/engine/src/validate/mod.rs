//! `validate` (M2.E6 framework, M2.E7 and M2.E8 rules): findings about one plugin (and
//! its masters) or a set of compiled scripts.
//!
//! Every rule has a stable id and a default severity; a finding is `{rule, severity,
//! plugin, record, subrecord, offset, message}` and any `error` fails the check
//! (`data.passed: false`, CLI exit 1). Masters are found in the explicit `plugins` list,
//! next to the plugin, then in `data_dir`; their headers are read for flags and their
//! records indexed when a rule needs them. Rules and their sources: `docs/validate.md`.

pub mod cells;
#[cfg(test)]
mod cells_tests;
pub mod forms;
pub mod scripts;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kitkraft_esm::bridge::{has_definition, Ctx};
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::index::{index_file, read_record_at, IndexOptions};
use kitkraft_esm::tes4::{Tes4, FLAG_LOCALIZED};
use kitkraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature};
use kitkraft_schema::FieldType;
use serde::Serialize;

use crate::edit::fields::{self, Layout};
use crate::edit::lstrings;
use crate::edit::patch::has_opaque;
use crate::edit::source::file_name;
use forms::{Space, Target};

/// A layout cache key: record signature, subrecord signature, member path.
type LayoutKey = ([u8; 4], [u8; 4], Option<String>);

/// How bad a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Breaks the plugin or the game; fails the check.
    Error,
    /// Worth a look; does not fail the check.
    Warning,
}

/// What a rule reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Input {
    /// A plugin (`plugin`).
    Plugin,
    /// Compiled scripts (`scripts`).
    Scripts,
}

/// Record types whose overrides drop a name the previous version has in the official
/// files themselves (`corpus:` `validate_corpus_official_files`, 2026-10-14: 92 overrides in
/// SFBGS003, SFBGS008, SFBGS00D, SFBGS047, SFBGS050 and ShatteredSpace, all of these
/// types). `override.stripped-name` is a warning for them and an error for every other
/// type (the Resupply door is a DOOR).
pub const NAMES_STRIPPED_IN_CORPUS: [[u8; 4]; 5] =
    [*b"CELL", *b"LCTN", *b"NPC_", *b"QUST", *b"WRLD"];

/// One rule in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuleInfo {
    /// Stable id (`masters.missing`).
    pub id: &'static str,
    /// Severity of its main finding (some rules also emit warnings).
    pub severity: Severity,
    /// What it reads.
    pub input: Input,
    /// One line.
    pub summary: &'static str,
}

/// The rule registry, in run order.
pub const RULES: &[RuleInfo] = &[
    RuleInfo {
        id: "masters.missing",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "A master the plugin lists is not in the load order, next to the plugin, or in data_dir (default KITKRAFT_DATA_DIR)",
    },
    RuleInfo {
        id: "masters.order",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "A master is listed before one of its own masters",
    },
    RuleInfo {
        id: "formid.dangling",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "A FormID (record header or typed member) names no master slot, or a form of the plugin itself it lacks (error); a form its master lacks (warning)",
    },
    RuleInfo {
        id: "lstring.copied-id",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "A non-localized plugin holds a 4-byte string ID in an lstring (FULL, DESC, or one of the corpus lstring table): the game shows its bytes as text; a warning for 4-byte payloads of unknown layout in overrides of a localized master",
    },
    RuleInfo {
        id: "override.stripped-name",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "An override lacks the FULL or NTRM the previous version in load order has (the Resupply door: no name, no prompt); a warning on the types the official DLCs do it to",
    },
    RuleInfo {
        id: "cell.missing-parent",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "A cell child outside a cell's children, or a WRLD/CELL children group whose record neither the plugin nor its masters have (error); a master's record not carried before its children group (warning)",
    },
    RuleInfo {
        id: "ref.orphan",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "A REFR or ACHR whose NAME (base) resolves to no record of the plugin or its masters, or that has no NAME",
    },
    RuleInfo {
        id: "ref.outside-block",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "A CELL in the wrong block or sub-block for its object ID (interior) or XCLC grid (exterior) (error); an exterior reference whose position is outside its cell's grid (warning)",
    },
    RuleInfo {
        id: "ref.persistent-flag",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "Record flag 0x400 that disagrees with the group: set outside persistent children (type 8) or clear inside, on a NAVM, or on a CELL that is not a world's persistent cell",
    },
    RuleInfo {
        id: "ref.persistence",
        severity: Severity::Warning,
        input: Input::Plugin,
        summary: "A reference in temporary children that another record of the plugin names (a quest alias, a location, a script property, a door link): expected persistent (grade D)",
    },
    RuleInfo {
        id: "cell.actor-without-navmesh",
        severity: Severity::Warning,
        input: Input::Plugin,
        summary: "An ACHR in a cell that has no NAVM in the plugin or its masters",
    },
    RuleInfo {
        id: "cell.incc",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "TES4 INCC differs from the number of interior cells in the plugin (error); interior cells and no INCC (warning)",
    },
    RuleInfo {
        id: "navmesh.unchanged",
        severity: Severity::Warning,
        input: Input::Plugin,
        summary: "A NAVM override whose subrecords equal its master's version",
    },
    RuleInfo {
        id: "record.flags-body",
        severity: Severity::Error,
        input: Input::Plugin,
        summary: "A compressed body that does not decompress, lstring payloads that do not fit the localized flag, a HEDR count that fits neither corpus meaning (warning)",
    },
    RuleInfo {
        id: "papyrus.denylisted-call",
        severity: Severity::Warning,
        input: Input::Scripts,
        summary: "A compiled script calls a native on the denylist (god mode toggles)",
    },
];

/// The rule `id`, if registered.
pub fn rule(id: &str) -> Option<&'static RuleInfo> {
    RULES.iter().find(|r| r.id == id)
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Rule id.
    pub rule: &'static str,
    /// Severity.
    pub severity: Severity,
    /// File name of the plugin (or script) it is about.
    pub plugin: String,
    /// `SIGN:FORMID` (or the script function), when about one record.
    pub record: Option<String>,
    /// Subrecord signature (with its member path), when about one subrecord.
    pub subrecord: Option<String>,
    /// Absolute offset in the file, when known.
    pub offset: Option<u64>,
    /// What is wrong.
    pub message: String,
}

/// What to validate.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// The plugin.
    pub plugin: Option<PathBuf>,
    /// Compiled scripts: `.pex` files, directories of them, or `.ba2` archives.
    pub scripts: Vec<PathBuf>,
    /// Also scan the scripts in the plugin's own archives (`<stem> - *.ba2` next to it).
    pub plugin_archives: bool,
    /// Rules to run (`None`: every rule whose input was given).
    pub rules: Option<Vec<&'static str>>,
    /// Explicit load order, searched first for masters.
    pub plugins: Vec<PathBuf>,
    /// Directory searched for masters after the plugin's own.
    pub data_dir: Option<PathBuf>,
    /// Decompression bounds.
    pub limits: DecodeLimits,
    /// Record indexes already built (lane D's load order): lower-case file name ->
    /// every stored FormID with its record offset. Plugins not listed are indexed here.
    pub indexed: HashMap<String, Arc<HashMap<u32, u64>>>,
}

/// A master as found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MasterInfo {
    /// File name as listed.
    pub name: String,
    /// Where it was found.
    pub path: Option<String>,
    /// TES4 flags (`0x%08X`).
    pub flags: Option<String>,
    /// Whether its localized flag is set.
    pub localized: Option<bool>,
}

/// What [`validate`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Every finding, in file order per rule pass.
    pub findings: Vec<Finding>,
    /// Rules that ran.
    pub rules: Vec<&'static str>,
    /// Masters of the plugin.
    pub masters: Vec<MasterInfo>,
    /// Records examined (`TES4` included).
    pub records: u64,
    /// Groups examined.
    pub groups: u64,
    /// FormIDs checked against their masters.
    pub checked_form_ids: u64,
    /// FormIDs that could not be resolved without guessing (`FE`/`FD`, unknown scale).
    pub unresolved_form_ids: u64,
    /// FormIDs in typed members naming an engine-reserved form: an object ID below
    /// [`RESERVED_OBJECT_IDS`] in `Starfield.esm`'s index space that it does not define
    /// (see [`is_reserved`]). Not findings; counted here and per member path in
    /// [`Report::reserved_by_member`].
    pub reserved_form_ids: u64,
    /// [`Report::reserved_form_ids`] by record type and member path
    /// (`NPC_ PropertyValue.ActorValue`).
    pub reserved_by_member: BTreeMap<String, u64>,
    /// Subrecords whose FormIDs could not be checked (opaque or undecoded payloads).
    pub unchecked_subrecords: u64,
    /// Subrecords examined.
    pub subrecords: u64,
    /// Scripts examined.
    pub scripts: u64,
    /// The script inputs scanned (files, directories, archives), the plugin's archives
    /// included.
    pub script_inputs: Vec<PathBuf>,
    /// Limitations.
    pub limitations: Vec<String>,
}

impl Report {
    /// Findings of `severity`.
    pub fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// No errors.
    pub fn passed(&self) -> bool {
        self.count(Severity::Error) == 0
    }
}

/// A master plugin read for validation.
#[derive(Debug)]
struct MasterFile {
    name: String,
    path: Option<PathBuf>,
    flags: Option<u32>,
    masters: Vec<String>,
    /// Stored FormID -> record offset, once indexed.
    ids: Option<Arc<HashMap<u32, u64>>>,
}

/// Runs the rules `options` selects.
pub fn validate(options: &Options) -> Result<Report, String> {
    let mut report = Report::default();
    let selected = |id: &str| options.rules.as_ref().is_none_or(|r| r.contains(&id));
    if let Some(plugin) = &options.plugin {
        let rules: Vec<&'static str> = RULES
            .iter()
            .filter(|r| r.input == Input::Plugin && selected(r.id))
            .map(|r| r.id)
            .collect();
        validate_plugin(plugin, options, &rules, &mut report)?;
        report.rules.extend(rules);
    }
    let mut script_inputs = options.scripts.clone();
    if options.plugin_archives {
        if let Some(plugin) = &options.plugin {
            let found = scripts::plugin_archives(plugin);
            if found.is_empty() {
                report.limitations.push(format!(
                    "plugin_archives: no archive named after {} next to it",
                    file_name(plugin)
                ));
            }
            script_inputs.extend(found);
        }
    }
    if !script_inputs.is_empty() {
        let rules: Vec<&'static str> = RULES
            .iter()
            .filter(|r| r.input == Input::Scripts && selected(r.id))
            .map(|r| r.id)
            .collect();
        if !rules.is_empty() {
            scripts::denylisted_calls(&script_inputs, &mut report)?;
        }
        report.rules.extend(rules);
        report.script_inputs = script_inputs;
    }
    Ok(report)
}

/// Where `name` is: the explicit list, next to the plugin, then `data_dir`.
fn find_plugin(name: &str, near: &Path, options: &Options) -> Option<PathBuf> {
    if let Some(p) = options.plugins.iter().find(|p| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
    }) {
        return Some(p.clone());
    }
    let mut dirs: Vec<&Path> = Vec::new();
    if let Some(dir) = near.parent() {
        dirs.push(dir);
    }
    if let Some(dir) = &options.data_dir {
        dirs.push(dir);
    }
    for dir in dirs {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        // Case-insensitive match (master names differ in case in the corpus).
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(Result::ok) {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(name)
                {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

/// Every stored FormID of the plugin at `path` with its record offset (`TES4` excluded).
fn index_ids(path: &Path) -> Result<HashMap<u32, u64>, String> {
    let index = index_file(path, &IndexOptions::default())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(index
        .records
        .iter()
        .skip(1)
        .map(|r| (r.form_id, r.offset))
        .collect())
}

fn header_of(path: &Path) -> Result<(u32, Vec<String>), String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let header =
        read_record_at(&mut file, 0, None).map_err(|e| format!("{}: {e}", path.display()))?;
    let tes4 = Tes4::new(&header);
    Ok((
        tes4.flags(),
        tes4.masters().iter().map(|m| m.name_lossy()).collect(),
    ))
}

/// Context for one plugin's pass.
struct Pass<'a> {
    name: String,
    localized: bool,
    space: Space,
    /// Masters by lower-case name (the plugin's and theirs).
    files: BTreeMap<String, MasterFile>,
    /// The plugin's own stored FormIDs.
    own_ids: std::collections::HashSet<u32>,
    rules: &'a [&'static str],
    /// Layouts by (record, subrecord, member path at its position; `None` for no member).
    layouts: HashMap<LayoutKey, Option<Arc<Layout>>>,
    limits: DecodeLimits,
    /// What the cell rules collect.
    cells: cells::Facts,
}

impl Pass<'_> {
    fn on(&self, id: &str) -> bool {
        self.rules.contains(&id)
    }

    fn space_of(&self, file: &MasterFile) -> Space {
        Space {
            name: file.name.clone(),
            flags: file.flags,
            masters: file
                .masters
                .iter()
                .map(|m| {
                    let flags = self
                        .files
                        .get(&m.to_ascii_lowercase())
                        .and_then(|f| f.flags);
                    (m.clone(), flags)
                })
                .collect(),
        }
    }

    /// The layout of every subrecord of a record, each by its position
    /// (`fields::layout_at`), cached by the member it is placed in.
    fn layouts(&mut self, record: [u8; 4], subs: &[[u8; 4]]) -> Vec<Option<Arc<Layout>>> {
        let schema = kitkraft_schema::builtin().ok();
        let placed = schema.and_then(|s| kitkraft_doc::typed::placements(s, record, subs));
        let mut out = Vec::with_capacity(subs.len());
        for (index, sub) in subs.iter().enumerate() {
            let path = schema.and_then(|s| {
                let p = placed.as_ref().and_then(|p| p.get(index));
                kitkraft_doc::typed::layout_of(s, record, *sub, index, p)
                    .ok()
                    .flatten()
                    .map(|l| l.path)
            });
            let layout = self
                .layouts
                .entry((record, *sub, path))
                .or_insert_with(|| fields::layout_at(record, subs, index).map(Arc::new))
                .clone();
            out.push(layout);
        }
        out
    }

    /// Whether `owner` has the form `object_id` (`None`: cannot tell).
    fn master_has(&self, owner: &str, object_id: u32) -> Option<(&MasterFile, u64)> {
        let file = self.files.get(&owner.to_ascii_lowercase())?;
        let ids = file.ids.as_ref()?;
        let raw = self.space_of(file).raw_for(owner, object_id)?;
        ids.get(&raw).map(|offset| (file, *offset))
    }

    fn can_check(&self, owner: &str, object_id: u32) -> bool {
        self.files
            .get(&owner.to_ascii_lowercase())
            .filter(|f| f.ids.is_some())
            .is_some_and(|f| self.space_of(f).raw_for(owner, object_id).is_some())
    }
}

fn validate_plugin(
    path: &Path,
    options: &Options,
    rules: &[&'static str],
    report: &mut Report,
) -> Result<(), String> {
    let name = file_name(path);
    let (flags, masters) = header_of(path)?;
    let needs_index = rules.iter().any(|r| {
        matches!(
            *r,
            "formid.dangling"
                | "override.stripped-name"
                | "cell.missing-parent"
                | "ref.orphan"
                | "navmesh.unchanged"
        )
    });

    // Masters, and their masters, by name.
    let mut files: BTreeMap<String, MasterFile> = BTreeMap::new();
    let mut queue: Vec<String> = masters.clone();
    while let Some(m) = queue.pop() {
        let key = m.to_ascii_lowercase();
        if files.contains_key(&key) || key == name.to_ascii_lowercase() || files.len() > 255 {
            continue;
        }
        let found = find_plugin(&m, path, options);
        let (mflags, mmasters) = match &found {
            Some(p) => {
                let (f, ms) = header_of(p)?;
                (Some(f), ms)
            }
            None => (None, Vec::new()),
        };
        queue.extend(mmasters.iter().cloned());
        files.insert(
            key,
            MasterFile {
                name: m,
                path: found,
                flags: mflags,
                masters: mmasters,
                ids: None,
            },
        );
    }
    let direct: Vec<String> = masters.iter().map(|m| m.to_ascii_lowercase()).collect();
    if needs_index {
        for key in &direct {
            if let Some(file) = files.get_mut(key) {
                if let Some(ids) = options.indexed.get(key) {
                    file.ids = Some(Arc::clone(ids));
                } else if let Some(p) = &file.path {
                    file.ids = Some(Arc::new(index_ids(p)?));
                }
            }
        }
    }
    for m in &masters {
        let file = files.get(&m.to_ascii_lowercase());
        report.masters.push(MasterInfo {
            name: m.clone(),
            path: file
                .and_then(|f| f.path.as_ref())
                .map(|p| p.display().to_string()),
            flags: file.and_then(|f| f.flags).map(|f| format!("0x{f:08X}")),
            localized: file.and_then(|f| f.flags).map(|f| f & FLAG_LOCALIZED != 0),
        });
    }

    let finding = |rule: &'static str,
                   severity: Severity,
                   record: Option<String>,
                   subrecord: Option<String>,
                   offset: Option<u64>,
                   message: String| Finding {
        rule,
        severity,
        plugin: name.clone(),
        record,
        subrecord,
        offset,
        message,
    };

    // masters.missing, masters.order
    if rules.contains(&"masters.missing") {
        for m in &masters {
            if files
                .get(&m.to_ascii_lowercase())
                .is_none_or(|f| f.path.is_none())
            {
                report.findings.push(finding(
                    "masters.missing",
                    Severity::Error,
                    None,
                    Some("MAST".into()),
                    None,
                    format!("master {m} was not found (load order, next to the plugin, data_dir or KITKRAFT_DATA_DIR)"),
                ));
            }
        }
    }
    if rules.contains(&"masters.order") {
        for (i, m) in masters.iter().enumerate() {
            let Some(file) = files.get(&m.to_ascii_lowercase()) else {
                continue;
            };
            for dep in &file.masters {
                let at = masters.iter().position(|x| x.eq_ignore_ascii_case(dep));
                if at.is_some_and(|at| at > i) {
                    report.findings.push(finding(
                        "masters.order",
                        Severity::Error,
                        None,
                        Some("MAST".into()),
                        None,
                        format!("master {m} is listed before its own master {dep}"),
                    ));
                }
            }
        }
    }

    let space = Space {
        name: name.clone(),
        flags: Some(flags),
        masters: masters
            .iter()
            .map(|m| {
                let f = files.get(&m.to_ascii_lowercase()).and_then(|f| f.flags);
                (m.clone(), f)
            })
            .collect(),
    };
    let own_ids: std::collections::HashSet<u32> = if needs_index {
        index_ids(path)?.into_keys().collect()
    } else {
        std::collections::HashSet::new()
    };
    let mut pass = Pass {
        name: name.clone(),
        localized: flags & FLAG_LOCALIZED != 0,
        space,
        files,
        own_ids,
        rules,
        layouts: HashMap::new(),
        limits: options.limits,
        cells: cells::Facts::default(),
    };

    let mut hedr: Option<u32> = None;
    let mut findings: Vec<Finding> = Vec::new();
    let mut stats = (0u64, 0u64);
    let parse = ParseOptions::default();
    visit_file(path, &parse, |entry| {
        let mut groups = Vec::new();
        walk(
            entry,
            &mut groups,
            &mut pass,
            &mut findings,
            report,
            &mut hedr,
            &mut stats,
        );
        Ok(())
    })
    .map_err(|e| format!("{name}: {e}"))?;
    if cells::Facts::any(&pass) {
        cells::finish(&pass, path, &mut findings, &mut report.limitations);
    }
    report.records = stats.0;
    report.groups = stats.1;
    report.findings.extend(findings);

    if pass.on("record.flags-body") {
        if let Some(count) = hedr {
            let majority = u32::try_from(report.records).unwrap_or(u32::MAX);
            let alternate = u32::try_from(
                report
                    .records
                    .saturating_sub(1)
                    .saturating_add(report.groups),
            )
            .unwrap_or(u32::MAX);
            if count != majority && count != alternate {
                report.findings.push(Finding {
                    rule: "record.flags-body",
                    severity: Severity::Warning,
                    plugin: name.clone(),
                    record: Some("TES4:00000000".into()),
                    subrecord: Some("HEDR".into()),
                    offset: None,
                    message: format!(
                        "HEDR record count {count} is neither the records including TES4 ({majority}) nor records without TES4 plus groups ({alternate}) (docs/corpus.md, Writing rules)"
                    ),
                });
            }
        }
    }
    if pass.space.scaled() {
        report.limitations.push(
            "this plugin is small or medium: its own records stored full-style (lane D's assumption full-style-self) are counted as unresolved, not guessed".into(),
        );
    }
    Ok(())
}

fn walk(
    entry: &Entry<'_>,
    groups: &mut Vec<kitkraft_esm::GroupHeader>,
    pass: &mut Pass<'_>,
    findings: &mut Vec<Finding>,
    report: &mut Report,
    hedr: &mut Option<u32>,
    stats: &mut (u64, u64),
) {
    match entry {
        Entry::Group(g) => {
            stats.1 = stats.1.saturating_add(1);
            cells::on_group(pass, &g.header, g.offset(), findings);
            // Bounded: the parser caps nesting (kitkraft_esm::MAX_DEPTH_CAP).
            groups.push(g.header);
            for child in g.children() {
                walk(child, groups, pass, findings, report, hedr, stats);
            }
            groups.pop();
        }
        Entry::Record(r) => {
            stats.0 = stats.0.saturating_add(1);
            if r.header.signature == Signature::TES4 {
                let tes4 = Tes4::new(r);
                *hedr = tes4.hedr().and_then(Result::ok).map(|h| h.record_count);
                pass.cells.incc = r
                    .subrecords()
                    .and_then(|subs| subs.iter().find(|s| s.signature() == Signature(*b"INCC")))
                    .and_then(|s| <[u8; 4]>::try_from(s.data()).ok())
                    .map(u32::from_le_bytes);
                return;
            }
            check_record(r, groups, pass, findings, report);
        }
    }
}

fn check_record(
    r: &Record<'_>,
    groups: &[kitkraft_esm::GroupHeader],
    pass: &mut Pass<'_>,
    findings: &mut Vec<Finding>,
    report: &mut Report,
) {
    let id = format!("{}:{:08X}", r.header.signature, r.header.form_id);
    let at = r.offset();
    let plugin_name = pass.name.clone();
    let push = |findings: &mut Vec<Finding>,
                rule,
                severity,
                sub: Option<String>,
                offset: Option<u64>,
                message: String| {
        findings.push(Finding {
            rule,
            severity,
            plugin: plugin_name.clone(),
            record: Some(id.clone()),
            subrecord: sub,
            offset,
            message,
        });
    };
    let header = pass.space.resolve(r.header.form_id);

    // formid.dangling: the record header.
    if pass.on("formid.dangling") {
        check_form_id(
            pass,
            findings,
            report,
            &id,
            None,
            at,
            r.header.form_id,
            &header,
            true,
        );
    }

    // The subrecords (decompressed when needed).
    let decoded = match r.decode(&pass.limits) {
        Ok(d) => d,
        Err(e) => {
            if pass.on("record.flags-body") {
                push(
                    findings,
                    "record.flags-body",
                    Severity::Error,
                    None,
                    at,
                    format!("compressed body does not decompress: {e}"),
                );
            }
            return;
        }
    };
    let subs = match (&decoded, r.subrecords()) {
        (Some(d), _) => d.as_slice(),
        (None, Some(s)) => s,
        (None, None) => return,
    };
    report.subrecords = report.subrecords.saturating_add(subs.len() as u64);
    let compressed = r.header.is_compressed();
    let sub_offset =
        |s: &kitkraft_esm::Subrecord<'_>| if compressed { at } else { s.data_offset() };
    let sig = r.header.signature.0;
    let layouts = if pass.on("formid.dangling")
        || pass.on("record.flags-body")
        || pass.on("lstring.copied-id")
    {
        let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
        pass.layouts(sig, &sigs)
    } else {
        Vec::new()
    };

    if cells::Facts::any(pass) {
        cells::on_record(pass, groups, r, subs, &id, findings);
    }

    // formid.dangling: typed members (M2.B3 visitors), opaque subrecords counted; the
    // FormIDs records name, for ref.persistence.
    if pass.on("formid.dangling") || pass.on("ref.persistence") {
        let typed = if has_definition(r.header.signature) {
            let ctx = Ctx {
                form_version: r.header.form_version,
                localized: pass.localized,
            };
            let raw: Vec<kitkraft_esm::bridge::RawSub<'_>> = subs
                .iter()
                .map(kitkraft_esm::bridge::RawSub::from)
                .collect();
            kitkraft_esm::generated::records::Record::decode(sig, &raw, ctx)
        } else {
            None
        };
        match &typed {
            Some(typed) => {
                for (path, form) in typed.form_ids() {
                    cells::named_by(pass, &id, r.header.signature, path, form.0);
                    if !pass.on("formid.dangling") {
                        continue;
                    }
                    let target = pass.space.resolve(form.0);
                    check_form_id(
                        pass,
                        findings,
                        report,
                        &id,
                        Some(path.to_string()),
                        at,
                        form.0,
                        &target,
                        false,
                    );
                }
                let extra: Vec<usize> = typed.extra().iter().map(|(i, _)| *i).collect();
                let mut unchecked = extra.len() as u64;
                for i in 0..subs.len() {
                    if extra.contains(&i) {
                        continue;
                    }
                    let opaque = layouts
                        .get(i)
                        .and_then(Option::as_ref)
                        .is_none_or(|l| has_opaque(&l.ty));
                    if opaque {
                        unchecked = unchecked.saturating_add(1);
                    }
                }
                report.unchecked_subrecords = report.unchecked_subrecords.saturating_add(unchecked);
            }
            None => {
                report.unchecked_subrecords = report
                    .unchecked_subrecords
                    .saturating_add(subs.len() as u64);
            }
        }
    }

    // lstrings: shapes (record.flags-body) and copied IDs (lstring.copied-id).
    let master_localized = match &header {
        Target::Form { owner, .. } if !owner.eq_ignore_ascii_case(&pass.name) => pass
            .files
            .get(&owner.to_ascii_lowercase())
            .and_then(|f| f.flags)
            .map(|f| f & FLAG_LOCALIZED != 0),
        _ => None,
    };
    if pass.on("record.flags-body") || pass.on("lstring.copied-id") {
        let mut nth: BTreeMap<[u8; 4], usize> = BTreeMap::new();
        for (i, s) in subs.iter().enumerate() {
            let n = nth.entry(s.signature().0).or_insert(0);
            let occurrence = *n;
            *n = n.saturating_add(1);
            let layout = layouts.get(i).cloned().flatten();
            // A 4-byte payload nothing describes (no layout, or opaque bytes, and not
            // classed by the corpus lstring table) in an override of a localized master
            // may be a copied string ID: a warning (Fable review round 2, finding 2c).
            let unknown = layout.as_ref().is_none_or(|l| has_opaque(&l.ty));
            if unknown {
                let data = s.data();
                let maybe_id = data.len() == 4 && data.iter().any(|b| *b != 0);
                if maybe_id
                    && !pass.localized
                    && master_localized == Some(true)
                    && pass.on("lstring.copied-id")
                    && lstrings::class(sig, s.signature().0) == lstrings::Class::Unclassified
                {
                    let label = format!("{}[{occurrence}]", s.signature());
                    push(findings, "lstring.copied-id", Severity::Warning, Some(label), sub_offset(s), format!("{} is 4 bytes of unknown layout (0x{}) in an override of a localized master: if it is an lstring, it is a copied string ID the game shows as text. Neither the schema nor the corpus lstring table says (docs/validate.md)", s.signature(), fields::to_hex(data)));
                }
                continue;
            }
            let Some(layout) = layout else {
                continue;
            };
            for (member, start, len) in spans(&layout, s.data(), r.header.form_version) {
                let label = format!("{}[{occurrence}]{member}", s.signature());
                let bytes = s
                    .data()
                    .get(start..start.saturating_add(len))
                    .unwrap_or_default();
                let offset = sub_offset(s).map(|o| o.saturating_add(start as u64));
                if pass.localized {
                    if len != 4 && pass.on("record.flags-body") {
                        push(
                            findings,
                            "record.flags-body",
                            Severity::Error,
                            Some(label),
                            offset,
                            format!(
                                "localized plugin: lstring is {len} bytes, not a 4-byte string ID"
                            ),
                        );
                    }
                    continue;
                }
                if len == 4 && pass.on("lstring.copied-id") {
                    let id_value =
                        u32::from_le_bytes(<[u8; 4]>::try_from(bytes).unwrap_or_default());
                    let looks_like_text = fields::text_of(bytes)
                        .is_some_and(|t| t.chars().all(|c| !c.is_control()) && !t.is_empty());
                    if master_localized == Some(true) || !looks_like_text {
                        let why = if master_localized == Some(true) {
                            "it overrides a record of a localized master".to_string()
                        } else {
                            "the bytes are not text".to_string()
                        };
                        push(findings, "lstring.copied-id", Severity::Error, Some(label), offset, format!("localized string id copied into non-localized plugin: {} holds 0x{id_value:08X} ({why}); the game shows these bytes as the name. Write the text inline (kit set {}=\"...\")", s.signature(), s.signature()));
                    }
                    continue;
                }
                // One NUL-terminated string; its bytes need not be UTF-8 (a few official
                // table entries are not, and M3.E2 copies them as stored).
                let one_string = bytes
                    .strip_suffix(&[0])
                    .is_some_and(|body| !body.contains(&0));
                if !one_string && pass.on("record.flags-body") {
                    push(
                        findings,
                        "record.flags-body",
                        Severity::Error,
                        Some(label),
                        offset,
                        "non-localized plugin: lstring is not one NUL-terminated string".into(),
                    );
                }
            }
        }
    }

    // override.stripped-name
    if pass.on("override.stripped-name") {
        if let Target::Form {
            owner,
            object_id,
            master_index,
        } = &header
        {
            if *master_index < pass.space.masters.len() {
                for wanted in [*b"FULL", *b"NTRM"] {
                    if subs.iter().any(|s| s.signature().0 == wanted) {
                        continue;
                    }
                    if let Some((plugin, has)) =
                        previous_has(pass, owner, *object_id, *master_index, wanted)
                    {
                        if has {
                            let w = String::from_utf8_lossy(&wanted).into_owned();
                            if NAMES_STRIPPED_IN_CORPUS.contains(&r.header.signature.0) {
                                push(findings, "override.stripped-name", Severity::Warning, Some(w.clone()), at, format!("this override has no {w}, but the previous version in load order ({plugin}) has one; the official DLCs do this on {} overrides (docs/validate.md), so it is a warning here", r.header.signature));
                            } else {
                                push(findings, "override.stripped-name", Severity::Error, Some(w.clone()), at, format!("this override has no {w}, but the previous version in load order ({plugin}) has one; a stripped name removes the item's name and its activation prompt (the Resupply door)"));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The lstring spans of a subrecord laid out as `layout`.
fn spans(layout: &Layout, payload: &[u8], form_version: u16) -> Vec<(String, usize, usize)> {
    match &layout.ty {
        FieldType::LString => vec![(String::new(), 0, payload.len())],
        FieldType::Struct { fields: list } => {
            let last = list.iter().rev().find(|f| f.present_in(Some(form_version)));
            match last {
                Some(f) if matches!(f.ty, FieldType::LString) => {
                    let steps = [fields::Step::Member(f.name.clone())];
                    fields::locate("", &layout.ty, payload, &steps, form_version)
                        .map(|at| vec![(format!(".{}", f.name), at.start, at.len)])
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// The last of the plugin's masters, from the owner on, holding the form, and whether
/// its version has the subrecord `wanted`. `None` when no master's version can be read.
fn previous_has(
    pass: &Pass<'_>,
    owner: &str,
    object_id: u32,
    owner_index: usize,
    wanted: [u8; 4],
) -> Option<(String, bool)> {
    let mut last: Option<(&MasterFile, u64)> = None;
    for (name, _) in pass.space.masters.iter().skip(owner_index) {
        let Some(file) = pass.files.get(&name.to_ascii_lowercase()) else {
            continue;
        };
        let Some(ids) = file.ids.as_ref() else {
            continue;
        };
        let Some(raw) = pass.space_of(file).raw_for(owner, object_id) else {
            continue;
        };
        if let Some(offset) = ids.get(&raw) {
            last = Some((file, *offset));
        }
    }
    let (file, offset) = last?;
    let mut input = File::open(file.path.as_ref()?).ok()?;
    let record = read_record_at(&mut input, offset, Some(&pass.limits)).ok()?;
    let has = record
        .subrecords()
        .is_some_and(|subs| subs.iter().any(|s| s.signature().0 == wanted));
    Some((file.name.clone(), has))
}

/// The game master whose low object IDs are engine-reserved.
pub const RESERVED_MASTER: &str = "Starfield.esm";

/// Object IDs below this in [`RESERVED_MASTER`]'s index space that the master does not
/// define are engine-reserved default forms (the player reference 0x14, built-in actor
/// values 0x2D0..0x3AC, a story manager root node 0x5B, ...): `formid.dangling` counts a
/// typed member naming one as `reserved_form_ids` instead of reporting it. Corpus fact,
/// grade D until a CK observation confirms it (ADR 0005 Amendment 4, `docs/validate.md`
/// "Engine-reserved object IDs"): no official file defines these IDs, yet the 14 official
/// files name them 1,410 times from 13 record types (M4.BX1). Low IDs that
/// `Starfield.esm` does define are ordinary forms and are checked as usual.
pub const RESERVED_OBJECT_IDS: u32 = 0x800;

/// Whether `owner:object_id` is in the engine-reserved range ([`RESERVED_OBJECT_IDS`]);
/// `formid.dangling` asks only for forms the owner does not define.
pub fn is_reserved(owner: &str, object_id: u32) -> bool {
    object_id < RESERVED_OBJECT_IDS && owner.eq_ignore_ascii_case(RESERVED_MASTER)
}

#[allow(clippy::too_many_arguments)]
fn check_form_id(
    pass: &Pass<'_>,
    findings: &mut Vec<Finding>,
    report: &mut Report,
    record: &str,
    member: Option<String>,
    offset: Option<u64>,
    raw: u32,
    target: &Target,
    header: bool,
) {
    let what = match &member {
        Some(m) => format!("{m} = {raw:08X}"),
        None => format!("FormID {raw:08X}"),
    };
    let mut push = |severity, message: String| {
        findings.push(Finding {
            rule: "formid.dangling",
            severity,
            plugin: pass.name.clone(),
            record: Some(record.to_string()),
            subrecord: member.clone(),
            offset,
            message,
        });
    };
    match target {
        Target::Null => {}
        Target::Unknown(_) => {
            report.unresolved_form_ids = report.unresolved_form_ids.saturating_add(1);
        }
        Target::Beyond {
            index,
            count,
            class,
        } => {
            report.checked_form_ids = report.checked_form_ids.saturating_add(1);
            push(
                Severity::Error,
                format!("{what} names {class} master slot {index:02X}, but the plugin has {count} {class} master(s) and is not itself in that slot"),
            );
        }
        Target::Form {
            owner, object_id, ..
        } => {
            let reserved = !header && is_reserved(owner, *object_id);
            let note_reserved = |report: &mut Report| {
                report.reserved_form_ids = report.reserved_form_ids.saturating_add(1);
                // Keyed by record type and member path: a shared type's path
                // (`PropertyValue.ActorValue`) is the same in every record type.
                let key = format!(
                    "{} {}",
                    record.get(..4).unwrap_or(record),
                    member.as_deref().unwrap_or_default()
                );
                let count = report.reserved_by_member.entry(key).or_default();
                *count = count.saturating_add(1);
            };
            let own = owner.eq_ignore_ascii_case(&pass.name);
            if own {
                report.checked_form_ids = report.checked_form_ids.saturating_add(1);
                if !header && !pass.own_ids.contains(&raw) {
                    if reserved {
                        note_reserved(report);
                        return;
                    }
                    push(
                        Severity::Error,
                        format!("{what} names {owner}:{object_id:06X}, a form of this plugin that it does not have (a dangling self-reference: a record copied from a small or medium plugin, or a form that was removed)"),
                    );
                }
                return;
            }
            if !pass.can_check(owner, *object_id) {
                report.unresolved_form_ids = report.unresolved_form_ids.saturating_add(1);
                return;
            }
            report.checked_form_ids = report.checked_form_ids.saturating_add(1);
            if pass.master_has(owner, *object_id).is_none() {
                if reserved {
                    note_reserved(report);
                    return;
                }
                let message = if header {
                    format!("this record overrides {owner}:{object_id:06X}, which {owner} does not have (an injected record, or an override of a deleted form)")
                } else {
                    format!("{what} names {owner}:{object_id:06X}, which {owner} does not have")
                };
                push(Severity::Warning, message);
            }
        }
    }
}
