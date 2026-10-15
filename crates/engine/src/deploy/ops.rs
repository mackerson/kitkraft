//! `deploy`, `deploy.rollback`, `deploy.status` and journal recovery.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::instance::{check_name, mod_arg, mod_name, path_units, Instance, MAX_PATH_UNITS};
use super::manifest::{
    check, files_under, read_journal, read_manifest, rename, to_json, write_atomic, Integrity,
    Journal, Manifest, ManifestFile, ManifestSource, JOURNAL_KIND,
};
use super::source::{self, Source, SourceFile};
use super::{
    io_error, now_unix, refused, rfc3339, sha256, DeployError, Hooks, Step, Touched, DEV_PROFILE,
    JOURNAL, LIVE_PROFILE, LOCK, MANIFEST, MARKER, MO2_META, PREFIX, PREVIOUS, RETIRED, STAGING,
};

/// Arguments of `deploy`.
#[derive(Debug, Clone)]
pub(crate) struct DeployRequest {
    /// A plugin file or a `package` folder.
    pub source: PathBuf,
    /// The MO2 instance root.
    pub mo2: PathBuf,
    /// The profile to check (never `Default`); diagnostics only.
    pub profile: Option<String>,
    /// Report what would change; write nothing.
    pub dry_run: bool,
    /// Test hooks.
    pub hooks: Hooks,
}

/// Arguments of `deploy.rollback`.
#[derive(Debug, Clone)]
pub(crate) struct RollbackRequest {
    /// The MO2 instance root.
    pub mo2: PathBuf,
    /// `KitKraft Dev - <stem>` or `<stem>`.
    pub mod_arg: String,
    /// The deployment id expected in `.previous`.
    pub to: Option<String>,
    /// The profile to check (never `Default`); diagnostics only.
    pub profile: Option<String>,
    /// Report what would change; write nothing.
    pub dry_run: bool,
    /// Test hooks.
    pub hooks: Hooks,
}

/// Arguments of `deploy.status`.
#[derive(Debug, Clone)]
pub(crate) struct StatusRequest {
    /// The MO2 instance root.
    pub mo2: PathBuf,
    /// Only this dev mod.
    pub mod_arg: Option<String>,
    /// The profile whose enabled state is read.
    pub profile: Option<String>,
}

/// What a command reports: `data` and `limitations`.
#[derive(Debug, Clone)]
pub(crate) struct Report {
    /// The payload (with a text `summary`).
    pub data: Value,
    /// What was not checked.
    pub limitations: Vec<String>,
}

/// The folders and journal of one dev mod.
#[derive(Debug, Clone)]
struct Slots {
    name: String,
    target: PathBuf,
    previous: PathBuf,
    staging: PathBuf,
    retired: PathBuf,
    journal: PathBuf,
    lock: PathBuf,
}

/// The lock a writing call (`deploy`, `deploy.rollback`) holds from before journal
/// recovery to its end: `<target>.kitkraft-lock`, created with `create_new`, so a second
/// call on the same dev mod is refused instead of racing the first. Removed on drop; a
/// process killed mid-run leaves it, and the refusal says how to clear it.
#[derive(Debug)]
struct Lock {
    path: PathBuf,
}

impl Lock {
    fn take(path: &Path) -> Result<Lock, DeployError> {
        match fs::File::create_new(path) {
            Ok(mut f) => {
                let note = format!(
                    "{{\"pid\": {}, \"started_at\": \"{}\"}}\n",
                    std::process::id(),
                    rfc3339(now_unix())
                );
                // The content is a note for a human; the file's existence is the lock.
                let _ = f.write_all(note.as_bytes());
                Ok(Lock {
                    path: path.to_path_buf(),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(refused(
                "locked",
                path,
                "another `kit deploy` or `kit deploy rollback` is running on this dev mod. If none is (a killed process leaves the lock behind), delete this file and run again; an interrupted operation's journal is recovered first",
            )),
            Err(e) => Err(io_error(path, "creating the lock", &e)),
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl Slots {
    fn new(inst: &Instance, name: &str) -> Result<Slots, DeployError> {
        let slot = |suffix: &str| -> Result<PathBuf, DeployError> {
            let n = format!("{name}{suffix}");
            let default = inst.child(&n)?;
            Ok(inst.find(&n)?.unwrap_or(default))
        };
        Ok(Slots {
            name: name.to_string(),
            target: slot("")?,
            previous: slot(PREVIOUS)?,
            staging: slot(STAGING)?,
            retired: slot(RETIRED)?,
            journal: slot(JOURNAL)?,
            lock: slot(LOCK)?,
        })
    }

    fn journal_tmp(&self) -> PathBuf {
        let mut n = self.journal.as_os_str().to_owned();
        n.push(".tmp");
        PathBuf::from(n)
    }
}

/// What occupies one slot.
#[derive(Debug, Clone)]
enum State {
    Absent,
    /// A folder whose manifest marks it as this dev mod.
    Ours(Box<Manifest>),
    /// A folder without a manifest.
    Bare,
    /// Anything else: a file, or a folder marked for another mod or unreadable.
    Foreign(String),
}

impl State {
    fn id(&self) -> Option<&str> {
        match self {
            State::Ours(m) => Some(m.deployment_id.as_str()),
            _ => None,
        }
    }

    fn exists(&self) -> bool {
        !matches!(self, State::Absent)
    }
}

fn state(path: &Path, name: &str) -> Result<State, DeployError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(State::Absent),
        Err(e) => return Err(io_error(path, "reading", &e)),
    };
    if meta.file_type().is_symlink() {
        return Err(refused(
            "reparse-point",
            path,
            "is a symbolic link or junction; KitKraft never writes through one",
        ));
    }
    if !meta.is_dir() {
        return Ok(State::Foreign("a file, not a folder".into()));
    }
    Ok(match read_manifest(path) {
        Ok(Some(m)) if m.owns(name) => State::Ours(Box::new(m)),
        Ok(Some(m)) => State::Foreign(format!(
            "its {MANIFEST} is not a `{MARKER}` marker for `{name}` (kind `{}`, mod `{}`)",
            m.kind, m.mod_name
        )),
        Ok(None) => State::Bare,
        Err(e) => State::Foreign(e.message),
    })
}

fn collision(path: &Path, why: &str) -> DeployError {
    refused(
        "target-collision",
        path,
        format!("exists and is not KitKraft-managed ({why}); KitKraft never writes into another mod. Rename or remove it in MO2 first"),
    )
}

fn previous_modified(path: &Path, i: &Integrity) -> DeployError {
    refused(
        "previous-modified",
        path,
        format!(
            "differs from its {MANIFEST} ({}); it holds an earlier deployment that this call would drop. Move the changes out (or delete the folder by hand if nothing in it is needed), then run again",
            i.describe()
        ),
    )
}

/// Refuses to drop a folder holding an earlier deployment (`.previous`, or `.retired`
/// while a deployment commits) unless it still matches its manifest (MO2's `meta.ini`
/// allowed): a file edited or added by hand there is never deleted.
fn check_droppable(path: &Path, s: &State) -> Result<(), DeployError> {
    match s {
        State::Absent => Ok(()),
        State::Ours(m) => {
            let i = check(path, m)?;
            if i.clean() {
                Ok(())
            } else {
                Err(previous_modified(path, &i))
            }
        }
        State::Bare => Err(collision(path, &format!("no {MANIFEST}"))),
        State::Foreign(why) => Err(collision(path, why)),
    }
}

fn remove_dir(path: &Path, touched: &mut Vec<Touched>) -> Result<(), DeployError> {
    // Refuses links anywhere inside before removing anything.
    files_under(path)?;
    fs::remove_dir_all(path).map_err(|e| io_error(path, "removing", &e))?;
    touched.push(Touched {
        action: "remove-dir",
        path: path.to_path_buf(),
        to: None,
    });
    Ok(())
}

fn remove_file(path: &Path, touched: &mut Vec<Touched>) -> Result<(), DeployError> {
    fs::remove_file(path).map_err(|e| io_error(path, "removing", &e))?;
    touched.push(Touched {
        action: "remove-file",
        path: path.to_path_buf(),
        to: None,
    });
    Ok(())
}

fn move_dir(from: &Path, to: &Path, touched: &mut Vec<Touched>) -> Result<(), DeployError> {
    rename(from, to)?;
    touched.push(Touched {
        action: "rename",
        path: from.to_path_buf(),
        to: Some(to.to_path_buf()),
    });
    Ok(())
}

/// Recovers an operation a journal says is in flight: finishes it when its new
/// deployment is live, otherwise puts the folders back as they were. Returns what it did.
fn recover(slots: &Slots, touched: &mut Vec<Touched>) -> Result<Option<Value>, DeployError> {
    let Some(j) = read_journal(&slots.journal)? else {
        let tmp = slots.journal_tmp();
        if tmp.is_file() {
            // Died while writing the journal: nothing else had changed yet.
            remove_file(&tmp, touched)?;
        }
        return Ok(None);
    };
    if j.kind != JOURNAL_KIND || !j.mod_name.eq_ignore_ascii_case(&slots.name) {
        return Err(refused(
            "journal",
            &slots.journal,
            format!("is not a journal for `{}`", slots.name),
        ));
    }
    let name = slots.name.as_str();
    let target = state(&slots.target, name)?;
    let previous = state(&slots.previous, name)?;
    let staging = state(&slots.staging, name)?;
    let retired = state(&slots.retired, name)?;
    for (path, s) in [
        (&slots.target, &target),
        (&slots.previous, &previous),
        (&slots.staging, &staging),
        (&slots.retired, &retired),
    ] {
        if let State::Foreign(why) = s {
            return Err(collision(path, why));
        }
    }
    let committed = target.id() == Some(j.new_id.as_str());
    let before = touched.len();
    match j.operation.as_str() {
        "deploy" => {
            if committed {
                if retired.exists() {
                    check_droppable(&slots.retired, &retired)?;
                    remove_dir(&slots.retired, touched)?;
                }
                if staging.exists() {
                    remove_dir(&slots.staging, touched)?;
                }
            } else {
                if staging.exists() {
                    remove_dir(&slots.staging, touched)?;
                }
                let mut previous_there = previous.exists();
                if !target.exists() && j.old_id.is_some() && previous.id() == j.old_id.as_deref() {
                    move_dir(&slots.previous, &slots.target, touched)?;
                    previous_there = false;
                }
                if !previous_there && retired.exists() {
                    move_dir(&slots.retired, &slots.previous, touched)?;
                }
            }
        }
        "rollback" => {
            if committed {
                if staging.exists() && !previous.exists() {
                    move_dir(&slots.staging, &slots.previous, touched)?;
                }
            } else if !target.exists() && staging.id() == j.old_id.as_deref() && staging.exists() {
                move_dir(&slots.staging, &slots.target, touched)?;
            }
        }
        other => {
            return Err(refused(
                "journal",
                &slots.journal,
                format!("unknown operation `{other}`"),
            ))
        }
    }
    // Our own temp names a crash mid-`write_atomic` leaves at a folder's root (rollback's
    // meta.ini copy into the live folder); left there, they would read as unmanaged files.
    for dir in [&slots.target, &slots.previous] {
        for own in [format!("{MO2_META}.tmp"), format!("{MANIFEST}.tmp")] {
            let tmp = dir.join(own);
            if fs::symlink_metadata(&tmp).is_ok_and(|m| m.is_file()) {
                remove_file(&tmp, touched)?;
            }
        }
    }
    remove_file(&slots.journal, touched)?;
    let steps: Vec<Value> = touched.iter().skip(before).map(Touched::json).collect();
    Ok(Some(json!({
        "journal": slots.journal.display().to_string(),
        "operation": j.operation,
        "started_at": j.started_at,
        "outcome": if committed { "completed" } else { "rolled back" },
        "live_deployment_id": if committed { Some(j.new_id) } else { j.old_id },
        "touched": steps,
    })))
}

/// Copies `file` to `dst` (a new file), hashing on the way, then reads `dst` back and
/// checks its SHA-256.
fn copy_verified(file: &SourceFile, dst: &Path) -> Result<(), DeployError> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| io_error(parent, "creating", &e))?;
    }
    let mut src = fs::File::open(&file.abs).map_err(|e| io_error(&file.abs, "opening", &e))?;
    let mut out = fs::File::create_new(dst).map_err(|e| io_error(dst, "creating", &e))?;
    let mut h = sha256::Sha256::default();
    let mut buf = vec![0u8; 1_048_576];
    loop {
        let n = src
            .read(&mut buf)
            .map_err(|e| io_error(&file.abs, "reading", &e))?;
        if n == 0 {
            break;
        }
        let chunk = buf.get(..n).unwrap_or_default();
        h.update(chunk);
        out.write_all(chunk)
            .map_err(|e| io_error(dst, "writing", &e))?;
    }
    out.sync_all().map_err(|e| io_error(dst, "flushing", &e))?;
    drop(out);
    if h.finish_hex() != file.sha256 {
        return Err(refused(
            "source-changed",
            &file.abs,
            "changed while it was being deployed; nothing was committed",
        ));
    }
    let (_, written) = sha256::file(dst).map_err(|e| io_error(dst, "verifying", &e))?;
    if written != file.sha256 {
        return Err(refused(
            "copy-verify",
            dst,
            "the copy's SHA-256 differs from the source's; nothing was committed",
        ));
    }
    Ok(())
}

/// The changes from the live deployment's files to the source's.
#[derive(Debug, Default)]
struct Diff {
    added: Vec<Value>,
    replaced: Vec<Value>,
    unchanged: Vec<Value>,
    removed: Vec<Value>,
}

impl Diff {
    fn new(files: &[SourceFile], old: Option<&Manifest>) -> Diff {
        let mut before: BTreeMap<String, &ManifestFile> = old
            .map(|m| m.files.iter().map(|f| (f.path.to_lowercase(), f)).collect())
            .unwrap_or_default();
        let mut d = Diff::default();
        for f in files {
            let entry = json!({"path": f.rel, "size": f.size, "sha256": f.sha256});
            match before.remove(&f.rel.to_lowercase()) {
                None => d.added.push(entry),
                Some(o) if o.sha256 == f.sha256 && o.size == f.size => d.unchanged.push(entry),
                Some(_) => d.replaced.push(entry),
            }
        }
        d.removed = before
            .into_values()
            .map(|f| json!({"path": f.path, "size": f.size, "sha256": f.sha256}))
            .collect();
        d
    }

    fn changed(&self) -> bool {
        !(self.added.is_empty() && self.replaced.is_empty() && self.removed.is_empty())
    }

    fn json(&self) -> Value {
        json!({
            "added": self.added,
            "replaced": self.replaced,
            "removed": self.removed,
            "unchanged": self.unchanged,
        })
    }
}

fn combine(mut refusals: Vec<DeployError>) -> DeployError {
    if refusals.len() == 1 {
        if let Some(only) = refusals.pop() {
            return only;
        }
    }
    let message = refusals
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    let first = refusals.into_iter().next();
    DeployError {
        rule: first.as_ref().map_or("refused", |r| r.rule),
        path: None,
        message,
    }
}

fn refusals_json(refusals: &[DeployError]) -> Value {
    Value::Array(
        refusals
            .iter()
            .map(|r| {
                json!({
                    "rule": r.rule,
                    "path": r.path.as_ref().map(|p| p.display().to_string()),
                    "message": r.message,
                })
            })
            .collect(),
    )
}

fn next_steps(name: &str, plugin: &str) -> Vec<String> {
    vec![
        format!("In MO2, switch to the `{DEV_PROFILE}` profile (never `{LIVE_PROFILE}`, the live game)."),
        "Refresh MO2 (F5) so it rereads mods\\ and its virtual file system serves the new files.".to_string(),
        format!("In the left pane, enable `{name}` in the `{DEV_PROFILE}` profile only (once; it stays enabled across redeploys)."),
        format!("In the Plugins tab, tick `{plugin}` and drag it to the bottom so it loads last."),
        format!("Leave `{name}{PREVIOUS}` disabled: it is the rollback copy (`kit deploy rollback --dev`)."),
        "Test on a disposable save only.".to_string(),
    ]
}

/// What `deploy` and `rollback` never write, for the report.
fn never_touched(inst: &Instance) -> Value {
    json!([
        inst.profiles.display().to_string(),
        inst.root.join("ModOrganizer.ini").display().to_string(),
        inst.root.join("overwrite").display().to_string(),
        inst.root.join("downloads").display().to_string(),
        format!("every other folder under {}", inst.mods.display()),
    ])
}

fn profile_section(
    inst: &Instance,
    profile: Option<&str>,
    name: &str,
    plugin: &str,
) -> (Value, Vec<String>) {
    match inst.diagnostic_profile(profile) {
        Some(p) => inst.profile_state(p, name, plugin),
        None => (
            Value::Null,
            vec!["no profile to read (none named, MO2's active one is missing or Default, and there is no KitKraft Dev profile): enabled state not read".into()],
        ),
    }
}

fn summary(head: &str, lines: &[String], touched: &[Touched], label: &str) -> String {
    let mut out = String::from(head);
    for l in lines {
        out.push('\n');
        out.push_str(l);
    }
    out.push_str(&format!("\n{label} ({}):", touched.len()));
    for t in touched {
        out.push_str("\n  ");
        out.push_str(&t.line());
    }
    out
}

fn deployment_id(unix: u64, seed: &str) -> String {
    let compact: String = rfc3339(unix)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let h = sha256::hex(format!("{seed}|{unix}|{nanos}|{}", std::process::id()).as_bytes());
    format!("{compact}-{}", h.get(..8).unwrap_or(&h))
}

/// The writes `deploy` makes, in order (the dry-run listing and the documentation of the
/// algorithm).
fn deploy_steps(
    slots: &Slots,
    files: &[SourceFile],
    meta_ini: bool,
    target_exists: bool,
    retire: bool,
) -> Vec<Touched> {
    let t = |action, path: &Path, to: Option<&Path>| Touched {
        action,
        path: path.to_path_buf(),
        to: to.map(Path::to_path_buf),
    };
    let mut steps = vec![
        t("write", &slots.journal, None),
        t("create-dir", &slots.staging, None),
    ];
    for f in files {
        steps.push(t("write", &slots.staging.join(&f.rel), None));
    }
    if meta_ini {
        steps.push(t("write", &slots.staging.join(MO2_META), None));
    }
    steps.push(t("write", &slots.staging.join(MANIFEST), None));
    if retire {
        steps.push(t("rename", &slots.previous, Some(&slots.retired)));
    }
    if target_exists {
        steps.push(t("rename", &slots.target, Some(&slots.previous)));
    }
    steps.push(t("rename", &slots.staging, Some(&slots.target)));
    if retire {
        steps.push(t("remove-dir", &slots.retired, None));
    }
    steps.push(t("remove-file", &slots.journal, None));
    steps
}

fn write_journal(
    slots: &Slots,
    j: &Journal,
    touched: &mut Vec<Touched>,
) -> Result<(), DeployError> {
    write_atomic(&slots.journal, &to_json(j, &slots.journal)?, touched)
}

/// Leftover `.staging` / `.retired` folders with no journal: removed when they are ours.
fn stale(slots: &Slots, refusals: &mut Vec<DeployError>) -> Result<Vec<PathBuf>, DeployError> {
    let mut out = Vec::new();
    for path in [&slots.staging, &slots.retired] {
        match state(path, &slots.name)? {
            State::Absent => {}
            State::Ours(m) if path == &slots.retired => {
                let i = check(path, &m)?;
                if i.clean() {
                    out.push(path.clone());
                } else {
                    refusals.push(previous_modified(path, &i));
                }
            }
            State::Ours(_) => out.push(path.clone()),
            State::Bare => refusals.push(collision(path, &format!("no {MANIFEST}"))),
            State::Foreign(why) => refusals.push(collision(path, &why)),
        }
    }
    Ok(out)
}

/// `deploy`.
pub(crate) fn deploy(req: &DeployRequest) -> Result<Report, DeployError> {
    let inst = Instance::resolve(&req.mo2)?;
    let source = source::read(&req.source)?;
    check_name(&source.stem)
        .map_err(|m| refused("plugin-name", &source.root.join(&source.plugin), m))?;
    let name = mod_name(&source.stem);
    let slots = Slots::new(&inst, &name)?;
    let mut refusals = inst.check_profiles(req.profile.as_deref());
    // The source must not be one of this mod's own folders (or contain them).
    if let Ok(src) = fs::canonicalize(&source.root) {
        for slot in [
            &slots.target,
            &slots.previous,
            &slots.staging,
            &slots.retired,
        ] {
            if let Ok(s) = fs::canonicalize(slot) {
                if src.starts_with(&s) || s.starts_with(&src) {
                    refusals.push(refused(
                        "location",
                        &source.root,
                        format!(
                            "overlaps the dev mod folder {}; deploy from the package output",
                            slot.display()
                        ),
                    ));
                }
            }
        }
    }
    // Every file lives under each slot in turn (`.previous` is the longest suffix), and
    // the longest internal name is the manifest's temp file `write_atomic` uses.
    let slot = [
        &slots.target,
        &slots.previous,
        &slots.staging,
        &slots.retired,
    ]
    .into_iter()
    .max_by_key(|p| path_units(p))
    .unwrap_or(&slots.previous);
    let mut longest: Vec<(PathBuf, &str)> = source
        .files
        .iter()
        .map(|f| (slot.join(&f.rel), "once deployed (in the rollback copy)"))
        .collect();
    longest.push((
        slot.join(format!("{MANIFEST}.tmp")),
        "for the deployment's own files; shorten the plugin name or the MO2 path",
    ));
    longest.push((
        slots.journal_tmp(),
        "for the deployment's own files; shorten the plugin name or the MO2 path",
    ));
    for (path, when) in longest {
        if path_units(&path) > MAX_PATH_UNITS {
            refusals.push(refused(
                "source-path",
                &path,
                format!("is longer than {MAX_PATH_UNITS} characters {when}"),
            ));
        }
    }
    if !refusals.is_empty() && !req.dry_run {
        return Err(combine(refusals));
    }

    // Held to the end of the call (dropped on every return); recovery runs only under it.
    let lock = if req.dry_run {
        None
    } else {
        Some(Lock::take(&slots.lock)?)
    };
    let mut touched = Vec::new();
    let pending = read_journal(&slots.journal)?;
    let recovered = if req.dry_run {
        None
    } else {
        recover(&slots, &mut touched)?
    };

    let target = state(&slots.target, &name)?;
    let previous = state(&slots.previous, &name)?;
    match &target {
        State::Bare => refusals.push(collision(&slots.target, &format!("no {MANIFEST}"))),
        State::Foreign(why) => refusals.push(collision(&slots.target, why)),
        _ => {}
    }
    match &previous {
        State::Bare => refusals.push(collision(&slots.previous, &format!("no {MANIFEST}"))),
        State::Foreign(why) => refusals.push(collision(&slots.previous, why)),
        _ => {}
    }
    let stale_dirs = if pending.is_some() && req.dry_run {
        Vec::new()
    } else {
        stale(&slots, &mut refusals)?
    };
    let old = match &target {
        State::Ours(m) => Some(m.as_ref().clone()),
        _ => None,
    };
    let mut integrity: Option<Integrity> = None;
    if let Some(m) = &old {
        let i = check(&slots.target, m)?;
        if !i.clean() {
            refusals.push(refused(
                "target-modified",
                &slots.target,
                format!(
                    "differs from its {MANIFEST} ({}); nothing is overwritten. Move the changes out, or roll back, then deploy again",
                    i.describe()
                ),
            ));
        }
        integrity = Some(i);
    }
    let meta_ini = integrity.as_ref().is_some_and(|i| i.meta_ini);
    let diff = Diff::new(&source.files, old.as_ref());
    let target_exists = target.exists();
    let retire = target_exists && previous.exists();
    let unchanged = old.is_some() && !diff.changed();
    // The rollback copy is dropped by this deployment: only when it is as recorded.
    let previous_integrity = match &previous {
        State::Ours(m) => Some(check(&slots.previous, m)?),
        _ => None,
    };
    if let Some(i) = &previous_integrity {
        if retire && !unchanged && !i.clean() {
            refusals.push(previous_modified(&slots.previous, i));
        }
    }
    let unix = now_unix();
    let previous_id = old
        .as_ref()
        .map(|m| m.deployment_id.clone())
        .or_else(|| previous.id().map(str::to_string));
    let (profile, mut warnings) =
        profile_section(&inst, req.profile.as_deref(), &name, &source.plugin);
    if let Some(p) = inst.selected_profile.as_deref() {
        if !p.eq_ignore_ascii_case(DEV_PROFILE) && !p.eq_ignore_ascii_case(LIVE_PROFILE) {
            warnings.push(format!(
                "MO2's active profile is `{p}`, not `{DEV_PROFILE}`; in-game checks run on `{DEV_PROFILE}`"
            ));
        }
    }
    let mut data = json!({
        "mod": name,
        "plugin": source.plugin,
        "target": slots.target.display().to_string(),
        "rollback_copy": slots.previous.display().to_string(),
        "mo2": inst.root.display().to_string(),
        "mods_dir": inst.mods.display().to_string(),
        "active_profile": inst.selected_profile,
        "source": {
            "path": source.root.display().to_string(),
            "kind": source.kind,
            "package_manifest": source.package_manifest.as_ref().map(|(p, sha)| json!({"path": p.display().to_string(), "sha256": sha})),
        },
        "previous_deployment_id": previous_id,
        "files": diff.json(),
        "live_integrity": integrity.as_ref().map(Integrity::json),
        "previous_integrity": previous_integrity.as_ref().map(Integrity::json),
        "carried_over": if meta_ini { vec![MO2_META] } else { Vec::new() },
        "recovered": recovered,
        "pending_journal": if req.dry_run { pending.as_ref().map(|j| json!({"path": slots.journal.display().to_string(), "operation": j.operation, "started_at": j.started_at})) } else { None },
        "lock": lock.as_ref().map(|l| l.path.display().to_string()),
        "refusals": refusals_json(&refusals),
        "never_touched": never_touched(&inst),
        "profile": profile,
        "warnings": warnings,
        "next_steps": next_steps(&name, &source.plugin),
        "dry_run": req.dry_run,
    });
    let counts = format!(
        "files: {} added, {} replaced, {} removed, {} unchanged",
        diff.added.len(),
        diff.replaced.len(),
        diff.removed.len(),
        diff.unchanged.len()
    );
    let mut lines = vec![format!("target: {}", slots.target.display()), counts];
    lines.extend(refusals.iter().map(|r| format!("refusal: {r}")));
    lines.extend(
        warnings_of(&data)
            .into_iter()
            .map(|w| format!("warning: {w}")),
    );

    if req.dry_run {
        let mut would = Vec::new();
        for s in &stale_dirs {
            would.push(Touched {
                action: "remove-dir",
                path: s.clone(),
                to: None,
            });
        }
        if !unchanged {
            would.extend(deploy_steps(
                &slots,
                &source.files,
                meta_ini,
                target_exists,
                retire,
            ));
        }
        let passed = refusals.is_empty();
        set(&mut data, "passed", json!(passed));
        set(
            &mut data,
            "outcome",
            json!(if !passed {
                "refused"
            } else if unchanged {
                "unchanged"
            } else {
                "plan"
            }),
        );
        set(&mut data, "touched", json!([]));
        set(
            &mut data,
            "would_touch",
            Value::Array(would.iter().map(Touched::json).collect()),
        );
        let head = format!(
            "deploy --dev --dry-run: {} `{}`",
            if passed { "would deploy" } else { "REFUSED" },
            name
        );
        set(
            &mut data,
            "summary",
            json!(summary(&head, &lines, &would, "would touch")),
        );
        return Ok(Report {
            data,
            limitations: source.limitations,
        });
    }
    if !refusals.is_empty() {
        return Err(combine(refusals));
    }
    if unchanged {
        set(&mut data, "passed", json!(true));
        set(&mut data, "outcome", json!("unchanged"));
        set(&mut data, "deployment_id", json!(previous_id));
        set(
            &mut data,
            "touched",
            Value::Array(touched.iter().map(Touched::json).collect()),
        );
        let head =
            format!("deploy --dev: `{name}` is already deployed with these files; nothing written");
        set(
            &mut data,
            "summary",
            json!(summary(&head, &lines, &touched, "touched")),
        );
        return Ok(Report {
            data,
            limitations: source.limitations,
        });
    }

    let seed: String = source.files.iter().map(|f| f.sha256.as_str()).collect();
    let id = deployment_id(unix, &seed);
    let manifest = Manifest {
        kind: MARKER.to_string(),
        format: 1,
        mod_name: name.clone(),
        plugin: source.plugin.clone(),
        deployment_id: id.clone(),
        previous_deployment_id: previous_id.clone(),
        deployed_at: rfc3339(unix),
        deployed_at_unix: unix,
        kitkraft_version: env!("CARGO_PKG_VERSION").to_string(),
        source: ManifestSource {
            path: source.root.display().to_string(),
            kind: source.kind.to_string(),
            package_manifest: source
                .package_manifest
                .as_ref()
                .map(|(p, _)| p.display().to_string()),
            package_manifest_sha256: source.package_manifest.as_ref().map(|(_, s)| s.clone()),
        },
        files: source
            .files
            .iter()
            .map(|f| ManifestFile {
                path: f.rel.clone(),
                size: f.size,
                sha256: f.sha256.clone(),
                source: f.abs.display().to_string(),
            })
            .collect(),
    };
    let journal = Journal {
        kind: JOURNAL_KIND.to_string(),
        mod_name: name.clone(),
        operation: "deploy".to_string(),
        state: "in-progress".to_string(),
        started_at: rfc3339(unix),
        new_id: id.clone(),
        old_id: old.as_ref().map(|m| m.deployment_id.clone()),
        older_id: previous.id().map(str::to_string),
    };
    let run = run_deploy(
        &slots,
        &source,
        &manifest,
        &journal,
        &stale_dirs,
        (meta_ini, target_exists, retire),
        req.hooks,
        &mut touched,
    );
    if let Err(e) = run {
        return Err(abort(e, &slots, &mut touched));
    }
    set(&mut data, "passed", json!(true));
    set(&mut data, "outcome", json!("deployed"));
    set(&mut data, "deployment_id", json!(id));
    set(
        &mut data,
        "touched",
        Value::Array(touched.iter().map(Touched::json).collect()),
    );
    let head = format!("deploy --dev: deployed `{name}` as {id}");
    lines.extend(
        next_steps(&name, &source.plugin)
            .into_iter()
            .enumerate()
            .map(|(i, s)| format!("next {}: {s}", i.saturating_add(1))),
    );
    set(
        &mut data,
        "summary",
        json!(summary(&head, &lines, &touched, "touched")),
    );
    Ok(Report {
        data,
        limitations: source.limitations,
    })
}

fn warnings_of(data: &Value) -> Vec<String> {
    data.get("warnings")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn set(data: &mut Value, key: &str, value: Value) {
    if let Some(obj) = data.as_object_mut() {
        obj.insert(key.to_string(), value);
    }
}

/// After a failure mid-operation: recover from the journal (automatic rollback) unless a
/// test interrupt simulated a dead process.
fn abort(e: DeployError, slots: &Slots, touched: &mut Vec<Touched>) -> DeployError {
    if e.rule == "interrupted" {
        return e;
    }
    let undo = match recover(slots, touched) {
        Ok(Some(r)) => format!(
            "rolled back automatically ({})",
            r.get("outcome")
                .and_then(Value::as_str)
                .unwrap_or("recovered")
        ),
        Ok(None) => "nothing had changed".to_string(),
        Err(r) => format!(
            "automatic rollback failed too: {r}; the journal {} stays for the next call",
            slots.journal.display()
        ),
    };
    let paths: Vec<String> = touched.iter().map(Touched::line).collect();
    DeployError {
        rule: e.rule,
        path: e.path,
        message: format!("{}; {undo}; paths touched: {}", e.message, paths.join(", ")),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_deploy(
    slots: &Slots,
    source: &Source,
    manifest: &Manifest,
    journal: &Journal,
    stale_dirs: &[PathBuf],
    (meta_ini, target_exists, retire): (bool, bool, bool),
    hooks: Hooks,
    touched: &mut Vec<Touched>,
) -> Result<(), DeployError> {
    for s in stale_dirs {
        remove_dir(s, touched)?;
    }
    write_journal(slots, journal, touched)?;
    fs::create_dir(&slots.staging).map_err(|e| io_error(&slots.staging, "creating", &e))?;
    touched.push(Touched {
        action: "create-dir",
        path: slots.staging.clone(),
        to: None,
    });
    for f in &source.files {
        let dst = slots.staging.join(&f.rel);
        copy_verified(f, &dst)?;
        touched.push(Touched {
            action: "write",
            path: dst,
            to: None,
        });
    }
    if meta_ini {
        let from = slots.target.join(MO2_META);
        let bytes = fs::read(&from).map_err(|e| io_error(&from, "reading", &e))?;
        write_atomic(&slots.staging.join(MO2_META), &bytes, touched)?;
    }
    let path = slots.staging.join(MANIFEST);
    write_atomic(&path, &to_json(manifest, &path)?, touched)?;
    hooks.check(Step::Staged)?;
    if retire {
        move_dir(&slots.previous, &slots.retired, touched)?;
        hooks.check(Step::Retired)?;
    }
    if target_exists {
        move_dir(&slots.target, &slots.previous, touched)?;
        hooks.check(Step::Moved)?;
    }
    move_dir(&slots.staging, &slots.target, touched)?;
    hooks.check(Step::Committed)?;
    if retire {
        remove_dir(&slots.retired, touched)?;
    }
    remove_file(&slots.journal, touched)
}

/// `deploy.rollback`.
pub(crate) fn rollback(req: &RollbackRequest) -> Result<Report, DeployError> {
    let inst = Instance::resolve(&req.mo2)?;
    let name = mod_arg(&req.mod_arg);
    let slots = Slots::new(&inst, &name)?;
    let mut refusals = inst.check_profiles(req.profile.as_deref());
    if !refusals.is_empty() && !req.dry_run {
        return Err(combine(refusals));
    }
    // Held to the end of the call; recovery runs only under it.
    let lock = if req.dry_run {
        None
    } else {
        Some(Lock::take(&slots.lock)?)
    };
    let mut touched = Vec::new();
    let pending = read_journal(&slots.journal)?;
    let recovered = if req.dry_run {
        None
    } else {
        recover(&slots, &mut touched)?
    };
    let target = state(&slots.target, &name)?;
    let previous = state(&slots.previous, &name)?;
    let (live, prev) = match (&target, &previous) {
        (State::Ours(t), State::Ours(p)) => (t.as_ref().clone(), p.as_ref().clone()),
        (State::Absent, _) => {
            return Err(refused(
                "nothing-to-roll-back",
                &slots.target,
                "no live deployment of this dev mod",
            ))
        }
        (State::Ours(_), State::Absent) => {
            return Err(refused(
                "nothing-to-roll-back",
                &slots.previous,
                "no previous deployment to restore",
            ))
        }
        (State::Ours(_), State::Bare) => {
            return Err(collision(&slots.previous, &format!("no {MANIFEST}")))
        }
        (State::Ours(_), State::Foreign(why)) => return Err(collision(&slots.previous, why)),
        (State::Bare, _) => return Err(collision(&slots.target, &format!("no {MANIFEST}"))),
        (State::Foreign(why), _) => return Err(collision(&slots.target, why)),
    };
    if let Some(to) = &req.to {
        if !to.eq_ignore_ascii_case(&prev.deployment_id) {
            refusals.push(refused(
                "rollback-target",
                &slots.previous,
                format!(
                    "holds deployment {}, not {to}; only the previous deployment is kept",
                    prev.deployment_id
                ),
            ));
        }
    }
    let live_check = check(&slots.target, &live)?;
    if !live_check.clean() {
        refusals.push(refused(
            "target-modified",
            &slots.target,
            format!(
                "differs from its {MANIFEST} ({}); nothing is overwritten",
                live_check.describe()
            ),
        ));
    }
    let prev_check = check(&slots.previous, &prev)?;
    if !prev_check.clean() {
        refusals.push(refused(
            "previous-modified",
            &slots.previous,
            format!(
                "differs from its {MANIFEST} ({}); it cannot be restored as recorded",
                prev_check.describe()
            ),
        ));
    }
    let stale_dirs = if pending.is_some() && req.dry_run {
        Vec::new()
    } else {
        stale(&slots, &mut refusals)?
    };
    let (profile, warnings) = profile_section(&inst, req.profile.as_deref(), &name, &prev.plugin);
    let t = |action, path: &Path, to: Option<&Path>| Touched {
        action,
        path: path.to_path_buf(),
        to: to.map(Path::to_path_buf),
    };
    let mut steps: Vec<Touched> = stale_dirs
        .iter()
        .map(|s| t("remove-dir", s, None))
        .collect();
    steps.push(t("write", &slots.journal, None));
    steps.push(t("rename", &slots.target, Some(&slots.staging)));
    steps.push(t("rename", &slots.previous, Some(&slots.target)));
    if live_check.meta_ini {
        steps.push(t("write", &slots.target.join(MO2_META), None));
    }
    steps.push(t("rename", &slots.staging, Some(&slots.previous)));
    steps.push(t("remove-file", &slots.journal, None));
    let mut data = json!({
        "mod": name,
        "target": slots.target.display().to_string(),
        "rollback_copy": slots.previous.display().to_string(),
        "mo2": inst.root.display().to_string(),
        "active_profile": inst.selected_profile,
        "restoring": prev.summary(),
        "replacing": live.summary(),
        "recovered": recovered,
        "pending_journal": if req.dry_run { pending.as_ref().map(|j| json!({"path": slots.journal.display().to_string(), "operation": j.operation})) } else { None },
        "lock": lock.as_ref().map(|l| l.path.display().to_string()),
        "refusals": refusals_json(&refusals),
        "never_touched": never_touched(&inst),
        "profile": profile,
        "warnings": warnings,
        "next_steps": [
            "Refresh MO2 (F5) so its virtual file system serves the restored files.",
            format!("`{}` keeps its place and enabled state in the `{DEV_PROFILE}` profile; nothing to change there.", name),
            "Run `kit deploy rollback --dev` again to undo this rollback.",
        ],
        "dry_run": req.dry_run,
    });
    let mut lines = vec![
        format!(
            "restoring {} (deployed {})",
            prev.deployment_id, prev.deployed_at
        ),
        format!(
            "replacing {} (deployed {}), kept as {}",
            live.deployment_id,
            live.deployed_at,
            slots.previous.display()
        ),
    ];
    lines.extend(refusals.iter().map(|r| format!("refusal: {r}")));
    if req.dry_run {
        let passed = refusals.is_empty();
        set(&mut data, "passed", json!(passed));
        set(
            &mut data,
            "outcome",
            json!(if passed { "plan" } else { "refused" }),
        );
        set(&mut data, "touched", json!([]));
        set(
            &mut data,
            "would_touch",
            Value::Array(steps.iter().map(Touched::json).collect()),
        );
        let head = format!(
            "deploy rollback --dev --dry-run: {} `{name}`",
            if passed { "would roll back" } else { "REFUSED" }
        );
        set(
            &mut data,
            "summary",
            json!(summary(&head, &lines, &steps, "would touch")),
        );
        return Ok(Report {
            data,
            limitations: Vec::new(),
        });
    }
    if !refusals.is_empty() {
        return Err(combine(refusals));
    }
    let journal = Journal {
        kind: JOURNAL_KIND.to_string(),
        mod_name: name.clone(),
        operation: "rollback".to_string(),
        state: "in-progress".to_string(),
        started_at: rfc3339(now_unix()),
        new_id: prev.deployment_id.clone(),
        old_id: Some(live.deployment_id.clone()),
        older_id: None,
    };
    let run = (|| -> Result<(), DeployError> {
        for s in &stale_dirs {
            remove_dir(s, &mut touched)?;
        }
        write_journal(&slots, &journal, &mut touched)?;
        move_dir(&slots.target, &slots.staging, &mut touched)?;
        req.hooks.check(Step::Moved)?;
        move_dir(&slots.previous, &slots.target, &mut touched)?;
        req.hooks.check(Step::Committed)?;
        // MO2's meta.ini follows the live folder: copied into the new live folder after
        // the swap, so `.previous` is never written before it is restored (a crash
        // mid-copy leaves `meta.ini.tmp` in the live folder; recovery removes it).
        if live_check.meta_ini {
            let from = slots.staging.join(MO2_META);
            let bytes = fs::read(&from).map_err(|e| io_error(&from, "reading", &e))?;
            write_atomic(&slots.target.join(MO2_META), &bytes, &mut touched)?;
        }
        move_dir(&slots.staging, &slots.previous, &mut touched)?;
        remove_file(&slots.journal, &mut touched)
    })();
    if let Err(e) = run {
        return Err(abort(e, &slots, &mut touched));
    }
    set(&mut data, "passed", json!(true));
    set(&mut data, "outcome", json!("rolled back"));
    set(&mut data, "deployment_id", json!(prev.deployment_id));
    set(
        &mut data,
        "touched",
        Value::Array(touched.iter().map(Touched::json).collect()),
    );
    let head = format!(
        "deploy rollback --dev: `{name}` is back to {}",
        prev.deployment_id
    );
    set(
        &mut data,
        "summary",
        json!(summary(&head, &lines, &touched, "touched")),
    );
    Ok(Report {
        data,
        limitations: Vec::new(),
    })
}

/// The dev mod folder a `mods\` entry belongs to, when its name starts with the prefix:
/// `(mod name, role)`.
fn classify(entry: &str) -> Option<(String, &'static str)> {
    let head = entry.get(..PREFIX.len())?;
    if !head.eq_ignore_ascii_case(PREFIX) {
        return None;
    }
    let lower = entry.to_lowercase();
    for (suffix, role) in [
        (format!("{JOURNAL}.tmp"), "journal-tmp"),
        (JOURNAL.to_string(), "journal"),
        (LOCK.to_string(), "lock"),
        (PREVIOUS.to_string(), "previous"),
        (STAGING.to_string(), "staging"),
        (RETIRED.to_string(), "retired"),
    ] {
        if lower.ends_with(&suffix) {
            let keep = entry.len().saturating_sub(suffix.len());
            return Some((format!("{PREFIX}{}", entry.get(PREFIX.len()..keep)?), role));
        }
    }
    Some((format!("{PREFIX}{}", entry.get(PREFIX.len()..)?), "live"))
}

/// `deploy.status` (read-only).
pub(crate) fn status(req: &StatusRequest) -> Result<Report, DeployError> {
    let inst = Instance::resolve(&req.mo2)?;
    let only = req.mod_arg.as_deref().map(mod_arg);
    let mut mods: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    for (entry, _) in inst.mod_entries()? {
        if let Some((name, role)) = classify(&entry) {
            if only.as_ref().is_none_or(|o| o.eq_ignore_ascii_case(&name)) {
                mods.entry(name).or_default().push(role);
            }
        }
    }
    if let Some(o) = &only {
        mods.entry(o.clone()).or_default();
    }
    let mut warnings = Vec::new();
    let active = inst.selected_profile.clone();
    match active.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) if p.eq_ignore_ascii_case(LIVE_PROFILE) => warnings.push(format!(
            "MO2's active profile is `{LIVE_PROFILE}` (the live game): deploy and rollback refuse until MO2 is switched to `{DEV_PROFILE}`"
        )),
        Some(_) => {}
        None => warnings.push(format!(
            "ModOrganizer.ini names no active profile: deploy and rollback refuse unless --profile names an existing profile other than `{LIVE_PROFILE}`"
        )),
    }
    if let Some(e) = req
        .profile
        .as_deref()
        .and_then(|p| inst.check_profile_arg(p))
    {
        warnings.push(format!("profile not read: {e}"));
    }
    let mut out = Vec::new();
    let mut lines = Vec::new();
    for (name, roles) in &mods {
        // One odd folder (a name its slots cannot be derived from, an unreadable entry)
        // is reported as that mod's error; the rest of the listing stands.
        let slots = match Slots::new(&inst, name) {
            Ok(s) => s,
            Err(e) => {
                lines.push(format!("{name}: error: {e}"));
                out.push(json!({
                    "mod": name,
                    "state": "error",
                    "error": e.to_string(),
                    "entries": roles,
                }));
                continue;
            }
        };
        let describe = |path: &Path, deep: bool| -> Value {
            match state(path, name) {
                Ok(State::Absent) => json!({"state": "absent"}),
                Ok(State::Bare) => {
                    json!({"state": "not-kitkraft", "reason": format!("no {MANIFEST}")})
                }
                Ok(State::Foreign(why)) => json!({"state": "not-kitkraft", "reason": why}),
                Ok(State::Ours(m)) => {
                    let integrity = if deep {
                        check(path, &m)
                            .map_or_else(|e| json!({"error": e.to_string()}), |i| i.json())
                    } else {
                        Value::Null
                    };
                    json!({"state": "managed", "manifest": m.summary(), "integrity": integrity})
                }
                Err(e) => json!({"state": "error", "error": e.to_string()}),
            }
        };
        let live = describe(&slots.target, true);
        let previous = describe(&slots.previous, false);
        let journal = read_journal(&slots.journal)
            .map(|j| j.map(|j| json!({"path": slots.journal.display().to_string(), "operation": j.operation, "started_at": j.started_at, "new_id": j.new_id, "old_id": j.old_id})))
            .unwrap_or_else(|e| Some(json!({"error": e.to_string()})));
        let leftovers: Vec<&str> = roles
            .iter()
            .copied()
            .filter(|r| matches!(*r, "staging" | "retired" | "journal-tmp"))
            .collect();
        let plugin = live
            .pointer("/manifest/plugin")
            .and_then(Value::as_str)
            .map(str::to_string);
        let profile = match (&plugin, inst.diagnostic_profile(req.profile.as_deref())) {
            (Some(plugin), Some(p)) => {
                let (v, w) = inst.profile_state(p, name, plugin);
                warnings.extend(w);
                v
            }
            _ => Value::Null,
        };
        lines.push(format!(
            "{name}: live {}{}, previous {}{}",
            live.get("state").and_then(Value::as_str).unwrap_or("?"),
            live.pointer("/manifest/deployment_id")
                .and_then(Value::as_str)
                .map_or_else(String::new, |i| format!(" ({i})")),
            previous.get("state").and_then(Value::as_str).unwrap_or("?"),
            previous
                .pointer("/manifest/deployment_id")
                .and_then(Value::as_str)
                .map_or_else(String::new, |i| format!(" ({i})")),
        ));
        if journal.is_some() {
            lines.push(format!(
                "  interrupted operation pending ({}); the next deploy or rollback recovers it",
                slots.journal.display()
            ));
        }
        let locked = fs::symlink_metadata(&slots.lock).is_ok();
        if locked {
            lines.push(format!(
                "  locked ({}): a deploy or rollback is running, or a killed one left the lock (delete it if none is running)",
                slots.lock.display()
            ));
        }
        out.push(json!({
            "mod": name,
            "state": "ok",
            "target": slots.target.display().to_string(),
            "live": live,
            "previous": previous,
            "pending_journal": journal,
            "lock": locked.then(|| slots.lock.display().to_string()),
            "leftovers": leftovers,
            "profile": profile,
        }));
    }
    let head = format!(
        "deploy status: {} KitKraft dev mod(s) in {}",
        out.len(),
        inst.mods.display()
    );
    lines.extend(warnings.iter().map(|w| format!("warning: {w}")));
    let text = summary(&head, &lines, &[], "touched");
    Ok(Report {
        data: json!({
            "mo2": inst.root.display().to_string(),
            "mods_dir": inst.mods.display().to_string(),
            "active_profile": active,
            "mods": out,
            "warnings": warnings,
            "touched": [],
            "summary": text,
        }),
        limitations: Vec::new(),
    })
}
