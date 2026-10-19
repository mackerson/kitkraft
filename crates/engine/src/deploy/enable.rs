//! `deploy.enable` and `deploy.disable` (ADR 0009 Amendment 2): the one explicit way
//! KitKraft edits an MO2 profile. Only the dev profile `KitKraft Dev`, only while MO2, the
//! game, the script extender and the editor are closed, and only the lines of the mods the
//! call names.
//!
//! `enable` turns `-<mod>` into `+<mod>` in `modlist.txt` (or adds `+<mod>` under the
//! header comments, where MO2 itself lists a new mod), and moves each of the mod's plugins
//! to the end of `plugins.txt` as `*<plugin>` and to the end of `loadorder.txt`.
//! `disable` turns `+<mod>` into `-<mod>` and `*<plugin>` into `<plugin>` in place. Every
//! other byte of the three files stays as it was (line endings, header, BOM, final
//! newline); a line the editor does not understand refuses the call, it is never
//! rewritten.
//!
//! Before writing, the three files are copied with their SHA-256 into a backup folder
//! outside MO2 (`%LOCALAPPDATA%\kitkraft\profile-backups\<id>\`); each file is replaced by
//! temp file and rename, re-read and hash-checked; any failure puts every file back from
//! the backup. Nothing else is written: not `ModOrganizer.ini`, not the profile's INIs,
//! saves or `archives.txt`, not a mod folder.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::instance::{check_name, Instance};
use super::manifest::{read_manifest, write_atomic};
use super::ops::{combine, refusals_json, summary, Report};
use super::{
    io_error, now_unix, refused, rfc3339, sha256, DeployError, Touched, DEV_PROFILE, JOURNAL,
    LIVE_PROFILE, PREFIX, PREVIOUS, RETIRED, STAGING,
};

/// The profile files the commands may change, in the order they are written.
pub(crate) const EDITED: [&str; 3] = ["modlist.txt", "plugins.txt", "loadorder.txt"];

/// Plugin file extensions, lowercase.
const PLUGIN_EXTENSIONS: [&str; 3] = ["esm", "esp", "esl"];

/// What a call does to the named mods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    /// `deploy.enable`.
    Enable,
    /// `deploy.disable`.
    Disable,
}

impl Action {
    fn id(self) -> &'static str {
        match self {
            Action::Enable => "deploy.enable",
            Action::Disable => "deploy.disable",
        }
    }
}

/// Arguments of `deploy.enable` / `deploy.disable`.
#[derive(Debug, Clone)]
pub(crate) struct EnableRequest {
    /// Enable or disable.
    pub action: Action,
    /// The MO2 instance root.
    pub mo2: PathBuf,
    /// Folder names under `mods\` (`KitKraft Dev - <stem>` may be given as `<stem>`), in
    /// the order their plugins are put last.
    pub mods: Vec<String>,
    /// The profile; only `KitKraft Dev` (any case) is accepted. Default `KitKraft Dev`.
    pub profile: Option<String>,
    /// Report every line that would change; write nothing.
    pub dry_run: bool,
    /// The running processes (image names), or why they could not be listed. The handler
    /// passes the system's; tests pass a list.
    pub running: Result<Vec<String>, String>,
    /// Processes whose presence refuses the call (MO2, the game, SFSE, the CK, MO2's
    /// helpers).
    pub blocking: Vec<String>,
    /// Where backups go (`%LOCALAPPDATA%\kitkraft\profile-backups`); `None` refuses a
    /// writing call.
    pub backup_root: Option<PathBuf>,
}

/// One named mod, resolved.
#[derive(Debug, Clone)]
struct Target {
    /// The folder name as it is on disk.
    name: String,
    /// `kitkraft` (a marked dev mod) or `third-party`.
    kind: &'static str,
    /// Plugins at the folder's root (dev mod: the manifest's plugin).
    plugins: Vec<String>,
}

/// A text file split into lines, keeping what [`TextFile::render`] needs to give back the
/// same bytes for unchanged lines.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TextFile {
    bom: bool,
    eol: &'static str,
    final_eol: bool,
    lines: Vec<String>,
}

impl TextFile {
    fn parse(bytes: &[u8], path: &Path) -> Result<TextFile, DeployError> {
        let (bom, body) = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
            Some(rest) => (true, rest),
            None => (false, bytes),
        };
        let text = std::str::from_utf8(body).map_err(|_| {
            refused(
                "profile-format",
                path,
                "is not UTF-8 text; KitKraft never rewrites a file it cannot read exactly",
            )
        })?;
        let crlf = text.contains("\r\n");
        let lf_only = text.replace("\r\n", "").contains('\n');
        if crlf && lf_only {
            return Err(refused(
                "profile-format",
                path,
                "mixes CRLF and LF line endings; KitKraft never rewrites a file it cannot give back exactly",
            ));
        }
        let eol = if crlf { "\r\n" } else { "\n" };
        let final_eol = text.is_empty() || text.ends_with(eol);
        let mut lines: Vec<String> = text.split(eol).map(str::to_string).collect();
        if final_eol {
            lines.pop();
        }
        if lines.iter().any(|l| l.contains('\r')) {
            return Err(refused(
                "profile-format",
                path,
                "holds a bare carriage return; KitKraft never rewrites a file it cannot give back exactly",
            ));
        }
        Ok(TextFile {
            bom,
            eol,
            final_eol,
            lines,
        })
    }

    fn render(&self) -> Vec<u8> {
        let mut out = Vec::new();
        if self.bom {
            out.extend_from_slice(b"\xEF\xBB\xBF");
        }
        out.extend_from_slice(self.lines.join(self.eol).as_bytes());
        if self.final_eol && !self.lines.is_empty() {
            out.extend_from_slice(self.eol.as_bytes());
        }
        out
    }

    /// Lines that are not blank and not `#` comments, with their indices.
    fn entries(&self) -> impl Iterator<Item = (usize, &str)> {
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .map(|(i, l)| (i, l.as_str()))
    }

    /// Index after the leading header comments and blanks.
    fn after_header(&self) -> usize {
        self.lines
            .iter()
            .position(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .unwrap_or(self.lines.len())
    }
}

/// One line changed, for the report.
#[derive(Debug, Clone)]
struct Change {
    file: &'static str,
    action: &'static str,
    before: Option<String>,
    after: Option<String>,
}

impl Change {
    /// Whether this change edits the file (a move keeps the line but changes its place).
    fn edits(&self) -> bool {
        !self.action.starts_with("already") && self.action != "not listed"
    }

    fn json(&self) -> Value {
        json!({"file": self.file, "action": self.action, "before": self.before, "after": self.after})
    }

    fn line(&self) -> String {
        match (&self.before, &self.after) {
            (Some(b), Some(a)) if b == a => format!("{}: {} `{a}`", self.file, self.action),
            (Some(b), Some(a)) => format!("{}: {} `{b}` -> `{a}`", self.file, self.action),
            (None, Some(a)) => format!("{}: {} `{a}`", self.file, self.action),
            (Some(b), None) => format!("{}: {} `{b}`", self.file, self.action),
            (None, None) => format!("{}: {}", self.file, self.action),
        }
    }
}

/// `modlist.txt`: the line naming `name` (any case), or a refusal for a malformed or
/// duplicated list.
fn find_mod(f: &TextFile, name: &str, path: &Path) -> Result<Option<usize>, DeployError> {
    let mut found = None;
    for (i, l) in f.entries() {
        let rest = l.strip_prefix(['+', '-', '*']).ok_or_else(|| {
            refused(
                "profile-format",
                path,
                format!(
                    "line {} (`{l}`) has no +/-/* prefix; fix it in MO2 first",
                    i.saturating_add(1)
                ),
            )
        })?;
        if rest.trim().eq_ignore_ascii_case(name) {
            if found.is_some() {
                return Err(refused(
                    "profile-format",
                    path,
                    format!("lists `{name}` more than once; fix it in MO2 first"),
                ));
            }
            found = Some(i);
        }
    }
    Ok(found)
}

/// Line indices in `plugins.txt` / `loadorder.txt` naming `plugin` (any case, `*` ignored).
fn find_plugin(f: &TextFile, plugin: &str) -> Vec<usize> {
    f.entries()
        .filter(|(_, l)| {
            l.trim()
                .trim_start_matches('*')
                .trim()
                .eq_ignore_ascii_case(plugin)
        })
        .map(|(i, _)| i)
        .collect()
}

fn edit_modlist(
    f: &mut TextFile,
    t: &Target,
    action: Action,
    path: &Path,
    changes: &mut Vec<Change>,
) -> Result<(), DeployError> {
    let file = "modlist.txt";
    match (find_mod(f, &t.name, path)?, action) {
        (Some(i), _) if f.lines.get(i).is_some_and(|l| l.starts_with('*')) => {
            return Err(refused(
                "unmanaged-mod",
                path,
                format!(
                    "`{}` is an unmanaged entry (`*`); MO2 does not enable or disable it",
                    t.name
                ),
            ))
        }
        (Some(i), Action::Enable) => {
            let line = f.lines.get(i).cloned().unwrap_or_default();
            if let Some(rest) = line.strip_prefix('-') {
                let after = format!("+{rest}");
                changes.push(Change {
                    file,
                    action: "enable",
                    before: Some(line),
                    after: Some(after.clone()),
                });
                if let Some(slot) = f.lines.get_mut(i) {
                    *slot = after;
                }
            } else {
                changes.push(Change {
                    file,
                    action: "already enabled",
                    before: Some(line.clone()),
                    after: Some(line),
                });
            }
        }
        (Some(i), Action::Disable) => {
            let line = f.lines.get(i).cloned().unwrap_or_default();
            if let Some(rest) = line.strip_prefix('+') {
                let after = format!("-{rest}");
                changes.push(Change {
                    file,
                    action: "disable",
                    before: Some(line),
                    after: Some(after.clone()),
                });
                if let Some(slot) = f.lines.get_mut(i) {
                    *slot = after;
                }
            } else {
                changes.push(Change {
                    file,
                    action: "already disabled",
                    before: Some(line.clone()),
                    after: Some(line),
                });
            }
        }
        (None, Action::Enable) => {
            let after = format!("+{}", t.name);
            f.lines.insert(f.after_header(), after.clone());
            changes.push(Change {
                file,
                action: "add",
                before: None,
                after: Some(after),
            });
        }
        (None, Action::Disable) => {
            changes.push(Change {
                file,
                action: "not listed",
                before: None,
                after: None,
            });
        }
    }
    Ok(())
}

/// `plugins.txt` (`star` true) or `loadorder.txt`: on enable, every line naming the plugin
/// goes and one line is appended; on disable (`plugins.txt` only) `*` is dropped in place.
fn edit_plugins(
    f: &mut TextFile,
    file: &'static str,
    plugin: &str,
    star: bool,
    action: Action,
    changes: &mut Vec<Change>,
) {
    let found = find_plugin(f, plugin);
    match action {
        Action::Enable => {
            let after = if star {
                format!("*{plugin}")
            } else {
                plugin.to_string()
            };
            let last_entry = f.entries().last().map(|(i, _)| i);
            let already = found.len() == 1
                && found.first() == last_entry.as_ref()
                && found
                    .first()
                    .and_then(|i| f.lines.get(*i))
                    .is_some_and(|l| l.trim() == after);
            if already {
                changes.push(Change {
                    file,
                    action: "already last",
                    before: Some(after.clone()),
                    after: Some(after),
                });
                return;
            }
            let before = found.first().and_then(|i| f.lines.get(*i).cloned());
            for i in found.iter().rev() {
                f.lines.remove(*i);
            }
            // Appended after the last line (blank trailing lines stay where they are).
            let at = f
                .entries()
                .last()
                .map_or(f.lines.len(), |(i, _)| i.saturating_add(1));
            f.lines.insert(at, after.clone());
            changes.push(Change {
                file,
                action: if before.is_some() {
                    "move last"
                } else {
                    "add last"
                },
                before,
                after: Some(after),
            });
        }
        Action::Disable => {
            if !star {
                return;
            }
            for i in found {
                let line = f.lines.get(i).cloned().unwrap_or_default();
                if let Some(rest) = line.trim_start().strip_prefix('*') {
                    let after = rest.to_string();
                    changes.push(Change {
                        file,
                        action: "deactivate",
                        before: Some(line),
                        after: Some(after.clone()),
                    });
                    if let Some(slot) = f.lines.get_mut(i) {
                        *slot = after;
                    }
                }
            }
        }
    }
}

/// The profile folder for `name`, matched case-insensitively among the instance's
/// profiles (MO2's `Kitkraft Dev` for `KitKraft Dev`). `Default` and every profile other
/// than `KitKraft Dev` are refused by name.
fn resolve_profile(inst: &Instance, name: &str) -> Result<PathBuf, DeployError> {
    let spelled = inst.profiles.join(name);
    if name.trim().eq_ignore_ascii_case(LIVE_PROFILE) {
        return Err(refused(
            "live-profile",
            &spelled,
            format!("`{LIVE_PROFILE}` is the owner's live game; profile edits are refused there by name"),
        ));
    }
    if !name.trim().eq_ignore_ascii_case(DEV_PROFILE) {
        return Err(refused(
            "not-dev-profile",
            &spelled,
            format!("only `{DEV_PROFILE}` may be edited (ADR 0009 Amendment 2)"),
        ));
    }
    let found = fs::read_dir(&inst.profiles)
        .map_err(|e| io_error(&inst.profiles, "listing", &e))?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(DEV_PROFILE))
                && p.join("modlist.txt").is_file()
        });
    let dir = found.ok_or_else(|| {
        refused(
            "profile-missing",
            &spelled,
            "is not an MO2 profile of this instance (no modlist.txt)",
        )
    })?;
    super::instance::refuse_links(&dir)?;
    Ok(dir)
}

/// The named mod's folder and plugins; refusals for a missing folder, a rollback or
/// staging copy, or a `KitKraft Dev - ` folder without its marker.
fn resolve_mod(inst: &Instance, arg: &str) -> Result<Target, DeployError> {
    let arg = arg.trim();
    check_name(arg).map_err(|m| refused("mod-name", &inst.mods.join(arg), m))?;
    let lower = arg.to_lowercase();
    for suffix in [PREVIOUS, STAGING, RETIRED] {
        if lower.ends_with(suffix) {
            return Err(refused(
                "rollback-copy",
                &inst.mods.join(arg),
                format!("is a deployment's `{suffix}` copy, never enabled"),
            ));
        }
    }
    let path = match inst.find(arg)? {
        Some(p) => p,
        None => inst.find(&format!("{PREFIX}{arg}"))?.ok_or_else(|| {
            refused(
                "mod-missing",
                &inst.mods.join(arg),
                format!("no folder `{arg}` (or `{PREFIX}{arg}`) under mods\\"),
            )
        })?,
    };
    super::instance::refuse_links(&path)?;
    if !path.is_dir() {
        return Err(refused("mod-missing", &path, "is not a folder"));
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(arg)
        .to_string();
    if name.len() >= PREFIX.len()
        && name
            .get(..PREFIX.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(PREFIX))
    {
        let journal = inst.mods.join(format!("{name}{JOURNAL}"));
        if journal.is_file() {
            return Err(refused(
                "journal-pending",
                &journal,
                "an interrupted deploy or rollback is pending; run `kit deploy --dev` or `kit deploy rollback --dev` to recover it first",
            ));
        }
        return match read_manifest(&path)? {
            Some(m) if m.owns(&name) => Ok(Target { name, kind: "kitkraft", plugins: vec![m.plugin] }),
            _ => Err(refused(
                "not-kitkraft",
                &path,
                "is named like a KitKraft dev mod but carries no matching kitkraft-deploy.json marker",
            )),
        };
    }
    let mut plugins: Vec<String> = fs::read_dir(&path)
        .map_err(|e| io_error(&path, "listing", &e))?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| {
            Path::new(n)
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| PLUGIN_EXTENSIONS.contains(&x.to_ascii_lowercase().as_str()))
        })
        .collect();
    plugins.sort_by_key(|p| p.to_lowercase());
    Ok(Target {
        name,
        kind: "third-party",
        plugins,
    })
}

fn read_file(path: &Path) -> Result<Option<Vec<u8>>, DeployError> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(path, "reading", &e)),
    }
}

/// `deploy.enable` / `deploy.disable`.
pub(crate) fn enable(req: &EnableRequest) -> Result<Report, DeployError> {
    let id = req.action.id();
    let inst = Instance::resolve(&req.mo2)?;
    let mut refusals = Vec::new();

    // Nothing may hold the profile: MO2 rewrites its lists while it runs and on exit.
    match &req.running {
        Ok(list) => {
            for exe in &req.blocking {
                if list.iter().any(|r| r.eq_ignore_ascii_case(exe)) {
                    refusals.push(refused(
                        "running",
                        &inst.root,
                        format!("{exe} is running; close MO2 and the game first (MO2 rewrites the profile's lists while it runs and when it exits)"),
                    ));
                }
            }
        }
        Err(e) => refusals.push(refused(
            "running",
            &inst.root,
            format!("could not list the running processes ({e}); profile edits never run blind"),
        )),
    }
    if req.mods.is_empty() {
        refusals.push(DeployError {
            rule: "mods-required",
            path: None,
            message: "name at least one mod".into(),
        });
    }
    let profile_name = req
        .profile
        .clone()
        .unwrap_or_else(|| DEV_PROFILE.to_string());
    let profile_dir = match resolve_profile(&inst, &profile_name) {
        Ok(d) => Some(d),
        Err(e) => {
            refusals.push(e);
            None
        }
    };
    let mut targets = Vec::new();
    for m in &req.mods {
        match resolve_mod(&inst, m) {
            Ok(t)
                if !targets
                    .iter()
                    .any(|x: &Target| x.name.eq_ignore_ascii_case(&t.name)) =>
            {
                targets.push(t)
            }
            Ok(_) => {}
            Err(e) => refusals.push(e),
        }
    }
    if !req.dry_run && req.backup_root.is_none() {
        refusals.push(DeployError {
            rule: "backup-dir",
            path: None,
            message: "no backup folder (LOCALAPPDATA is not set); profile edits are never made without a backup".into(),
        });
    }

    // Read and edit in memory.
    let mut originals: Vec<(&'static str, PathBuf, Option<Vec<u8>>)> = Vec::new();
    let mut edited: Vec<(&'static str, PathBuf, Vec<u8>)> = Vec::new();
    let mut changes = Vec::new();
    let mut warnings = Vec::new();
    if let Some(dir) = &profile_dir {
        let locked = dir.join("lockedorder.txt");
        if let Some(b) = read_file(&locked)? {
            let f = TextFile::parse(&b, &locked)?;
            if f.entries().next().is_some() {
                refusals.push(refused(
                    "locked-order",
                    &locked,
                    "has locked plugins; moving plugins last could break the lock. Unlock them in MO2 first",
                ));
            }
        }
        let mut files = Vec::new();
        for name in EDITED {
            let path = dir.join(name);
            let bytes = read_file(&path)?;
            match &bytes {
                Some(b) => match TextFile::parse(b, &path) {
                    Ok(f) => files.push((name, path.clone(), Some(f))),
                    Err(e) => refusals.push(e),
                },
                None if name == "modlist.txt" => {
                    refusals.push(refused("profile-missing", &path, "is missing"))
                }
                None => {
                    warnings.push(format!(
                        "{name} does not exist in the profile; it is not created"
                    ));
                    files.push((name, path.clone(), None));
                }
            }
            originals.push((name, path, bytes));
        }
        // Edits are computed even when something else refused, so a dry run shows them;
        // nothing is written while any refusal stands.
        {
            for t in &targets {
                for (name, path, file) in &mut files {
                    let Some(f) = file.as_mut() else { continue };
                    match *name {
                        "modlist.txt" => {
                            if let Err(e) = edit_modlist(f, t, req.action, path, &mut changes) {
                                refusals.push(e);
                            }
                        }
                        "plugins.txt" => {
                            for p in &t.plugins {
                                edit_plugins(f, "plugins.txt", p, true, req.action, &mut changes);
                            }
                        }
                        _ => {
                            for p in &t.plugins {
                                edit_plugins(
                                    f,
                                    "loadorder.txt",
                                    p,
                                    false,
                                    req.action,
                                    &mut changes,
                                );
                            }
                        }
                    }
                }
                if t.plugins.is_empty() {
                    warnings.push(format!(
                        "`{}` has no plugin at its root; only modlist.txt changes",
                        t.name
                    ));
                }
            }
            for ((name, path, file), (_, _, orig)) in files.iter().zip(&originals) {
                if let (Some(f), Some(orig)) = (file, orig) {
                    let bytes = f.render();
                    if &bytes != orig {
                        edited.push((name, path.clone(), bytes));
                    }
                }
            }
        }
    }

    let mods_json: Vec<Value> = targets
        .iter()
        .map(|t| json!({"mod": t.name, "kind": t.kind, "plugins": t.plugins}))
        .collect();
    let mut lines: Vec<String> = changes.iter().map(Change::line).collect();
    lines.extend(warnings.iter().map(|w| format!("warning: {w}")));
    let base = |touched: &[Touched], backup: Option<&Path>, outcome: &str| {
        json!({
            "mo2": inst.root.display().to_string(),
            "profile": profile_name,
            "profile_dir": profile_dir.as_ref().map(|d| d.display().to_string()),
            "dry_run": req.dry_run,
            "mods": mods_json,
            "changes": changes.iter().map(Change::json).collect::<Vec<_>>(),
            "files_changed": edited.iter().map(|(n, _, _)| *n).collect::<Vec<_>>(),
            "backup": backup.map(|b| b.display().to_string()),
            "warnings": warnings,
            "refusals": refusals_json(&refusals),
            "touched": touched.iter().map(Touched::json).collect::<Vec<_>>(),
            "outcome": outcome,
            "passed": refusals.is_empty(),
            "never_touched": [
                inst.root.join("ModOrganizer.ini").display().to_string(),
                "every other file in the profile (INIs, saves, archives.txt, lockedorder.txt)".to_string(),
                inst.mods.display().to_string(),
            ],
            "next_steps": [
                format!("Open MO2 on `{DEV_PROFILE}` and check the mods and plugins read as this report says."),
                "Test on a disposable save only.".to_string(),
            ],
        })
    };

    if req.dry_run {
        let outcome = if !refusals.is_empty() {
            "refused"
        } else if edited.is_empty() {
            "unchanged"
        } else {
            "plan"
        };
        let head = format!(
            "{id} --dry-run: {} ({} line change(s) in {} file(s), profile `{profile_name}`)",
            if refusals.is_empty() {
                "would edit"
            } else {
                "REFUSED"
            },
            changes.iter().filter(|c| c.edits()).count(),
            edited.len()
        );
        let mut l = lines.clone();
        l.extend(refusals.iter().map(|r| format!("refused: {r}")));
        let mut data = base(&[], None, outcome);
        if let Some(o) = data.as_object_mut() {
            o.insert(
                "summary".into(),
                Value::String(summary(&head, &l, &[], "touched")),
            );
        }
        return Ok(Report {
            data,
            limitations: Vec::new(),
        });
    }
    if !refusals.is_empty() {
        return Err(combine(refusals));
    }
    let Some(dir) = profile_dir.clone() else {
        return Err(DeployError {
            rule: "profile-missing",
            path: None,
            message: "no profile".into(),
        });
    };
    if edited.is_empty() {
        let head = format!("{id}: nothing to change in profile `{profile_name}`");
        let mut data = base(&[], None, "unchanged");
        if let Some(o) = data.as_object_mut() {
            o.insert(
                "summary".into(),
                Value::String(summary(&head, &lines, &[], "touched")),
            );
        }
        return Ok(Report {
            data,
            limitations: Vec::new(),
        });
    }

    // Backup, then write, then verify; any failure restores every file.
    let root = req.backup_root.clone().unwrap_or_default();
    let backup_id = format!(
        "{}-{}",
        rfc3339(now_unix()).replace([':', '-'], ""),
        std::process::id()
    );
    let backup = root.join(&backup_id);
    let mut touched = Vec::new();
    fs::create_dir_all(&backup).map_err(|e| io_error(&backup, "creating the backup folder", &e))?;
    let mut record = Vec::new();
    for (name, path, bytes) in &originals {
        if let Some(b) = bytes {
            let copy = backup.join(name);
            fs::write(&copy, b).map_err(|e| io_error(&copy, "backing up", &e))?;
            let back = fs::read(&copy).map_err(|e| io_error(&copy, "verifying the backup", &e))?;
            if sha256::hex(&back) != sha256::hex(b) {
                return Err(refused(
                    "backup",
                    &copy,
                    "the backup copy does not match the original; nothing was changed",
                ));
            }
            record.push(
                json!({"file": name, "path": path.display().to_string(), "sha256": sha256::hex(b)}),
            );
        }
    }
    let note = json!({
        "kind": "kitkraft-profile-backup",
        "command": id,
        "profile_dir": dir.display().to_string(),
        "taken_at": rfc3339(now_unix()),
        "files": record,
        "mods": req.mods,
    });
    fs::write(
        backup.join("backup.json"),
        serde_json::to_vec_pretty(&note).unwrap_or_default(),
    )
    .map_err(|e| io_error(&backup, "writing backup.json", &e))?;

    let write_all = |touched: &mut Vec<Touched>| -> Result<(), DeployError> {
        for (_, path, bytes) in &edited {
            write_atomic(path, bytes, touched)?;
            let back = fs::read(path).map_err(|e| io_error(path, "verifying", &e))?;
            if sha256::hex(&back) != sha256::hex(bytes) {
                return Err(refused(
                    "verify",
                    path,
                    "the written file does not read back as written",
                ));
            }
        }
        Ok(())
    };
    if let Err(e) = write_all(&mut touched) {
        let mut restore_touched = Vec::new();
        for (_, path, bytes) in &originals {
            if let Some(b) = bytes {
                let _ = write_atomic(path, b, &mut restore_touched);
            }
        }
        return Err(DeployError {
            rule: e.rule,
            path: e.path,
            message: format!(
                "{}; every profile file was put back from {}",
                e.message,
                backup.display()
            ),
        });
    }
    let head = format!(
        "{id}: edited profile `{profile_name}` ({} line change(s) in {} file(s)); backup {}",
        changes.iter().filter(|c| c.edits()).count(),
        edited.len(),
        backup.display()
    );
    let mut data = base(&touched, Some(&backup), "edited");
    if let Some(o) = data.as_object_mut() {
        o.insert(
            "summary".into(),
            Value::String(summary(&head, &lines, &touched, "touched")),
        );
    }
    Ok(Report {
        data,
        limitations: Vec::new(),
    })
}
