//! Development deployment into one isolated, KitKraft-managed MO2 mod (ADR 0009, M4.E11).
//!
//! `deploy` copies a plugin (or a `package` folder: the plugin plus loose files and
//! archives) into `<mods>\KitKraft Dev - <plugin stem>\`, the only place KitKraft ever
//! writes inside a Mod Organizer instance. The folder carries `kitkraft-deploy.json` (the
//! ownership marker and manifest: hashes, source paths, time, the previous deployment's
//! id); a folder of that name without the marker is never opened for writing.
//!
//! Safe replacement: the new deployment is staged in `<target>.staging` (inside `mods\`,
//! so every rename stays on one volume), verified by hash, and swapped in by renaming
//! folders: `<target>.previous` -> `<target>.retired`, `<target>` -> `<target>.previous`,
//! `<target>.staging` -> `<target>` (the commit), then the retired copy is removed. A
//! journal (`<target>.kitkraft-journal.json`, a file beside the folders) is written before
//! the first change and removed after the last; a journal found at the start of the next
//! call is recovered first (committed: finish; not committed: put the old folders back).
//! `deploy.rollback` swaps `<target>.previous` back in the same way, so a rollback can be
//! rolled back.
//!
//! Never touched by `deploy`, `deploy.rollback` and `deploy.status`: MO2 profiles
//! (`modlist.txt`, `plugins.txt`, `loadorder.txt`, ...), `ModOrganizer.ini`, `overwrite\`,
//! `downloads\`, any other folder under `mods\`, the game. They refuse when MO2's active
//! profile, or the `profile` argument, is `Default` (the owner's live game, AGENTS.md), and
//! they never enable the mod. Enabling is the separate, explicit `deploy.enable` (ADR 0009
//! Amendment 2, [`enable`]): the dev profile only, MO2 and the game closed. Rules and
//! examples: `docs/deploy.md`.

mod enable;
#[cfg(test)]
mod enable_tests;
mod instance;
mod manifest;
mod ops;
pub(crate) mod sha256;
mod source;
#[cfg(test)]
mod tests;

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub(crate) use enable::{enable, Action, EnableRequest};
pub(crate) use ops::{
    deploy, rollback, status, DeployRequest, Report, RollbackRequest, StatusRequest,
};

/// Folder name prefix of every KitKraft dev mod.
pub(crate) const PREFIX: &str = "KitKraft Dev - ";
/// The ownership marker and manifest inside a dev mod.
pub(crate) const MANIFEST: &str = "kitkraft-deploy.json";
/// `kind` of [`MANIFEST`].
pub(crate) const MARKER: &str = "kitkraft-dev-mod";
/// The file MO2 writes into every mod folder it lists; carried over, never listed.
pub(crate) const MO2_META: &str = "meta.ini";
/// The package manifest `package` writes (M4.E5); verified, not deployed.
pub(crate) const PACKAGE_MANIFEST: &str = "kitkraft-manifest.json";
/// [`PACKAGE_MANIFEST`]'s pre-rename name, still read when the new one is absent (deprecated,
/// kept for one release after the KitKraft rename).
pub(crate) const OLD_PACKAGE_MANIFEST: &str = "kitcraft-manifest.json";
/// The owner's live-game profile: never a target (AGENTS.md).
pub(crate) const LIVE_PROFILE: &str = "Default";
/// The profile in-game checks run on.
pub(crate) const DEV_PROFILE: &str = "KitKraft Dev";

/// Suffix of the rollback copy beside the live folder.
pub(crate) const PREVIOUS: &str = ".previous";
/// Suffix of the folder a new deployment is staged in.
pub(crate) const STAGING: &str = ".staging";
/// Suffix of the replaced rollback copy while a deployment commits.
pub(crate) const RETIRED: &str = ".retired";
/// Suffix of the journal file beside the folders.
pub(crate) const JOURNAL: &str = ".kitkraft-journal.json";
/// Most files in one package and in one dev mod: `package` writes no more, and `deploy`
/// reads no more from a package or a deployed folder (one limit, so every package can be
/// deployed).
pub(crate) const MAX_PACKAGE_FILES: usize = 20_000;
/// Suffix of the lock file a writing call holds for its whole run.
pub(crate) const LOCK: &str = ".kitkraft-lock";

/// Why a deployment command stopped. Every refusal names its rule and the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeployError {
    /// Rule id (`docs/deploy.md`, "Refusals").
    pub rule: &'static str,
    /// The path the rule is about.
    pub path: Option<PathBuf>,
    /// What is wrong and what to do.
    pub message: String,
}

impl fmt::Display for DeployError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(p) => write!(
                f,
                "refused [{}] {}: {}",
                self.rule,
                p.display(),
                self.message
            ),
            None => write!(f, "refused [{}]: {}", self.rule, self.message),
        }
    }
}

/// A refusal by `rule` about `path`.
pub(crate) fn refused(rule: &'static str, path: &Path, message: impl Into<String>) -> DeployError {
    DeployError {
        rule,
        path: Some(path.to_path_buf()),
        message: message.into(),
    }
}

/// An I/O failure while doing `action` on `path`.
pub(crate) fn io_error(path: &Path, action: &str, e: &std::io::Error) -> DeployError {
    DeployError {
        rule: "io",
        path: Some(path.to_path_buf()),
        message: format!("{action}: {e}"),
    }
}

/// One change on disk, for the diagnostic listing of every path touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Touched {
    /// `create-dir`, `write`, `rename`, `remove-dir`, `remove-file`.
    pub action: &'static str,
    /// The path changed.
    pub path: PathBuf,
    /// The new name of a `rename`.
    pub to: Option<PathBuf>,
}

impl Touched {
    /// As JSON for `data.touched`.
    pub(crate) fn json(&self) -> Value {
        match &self.to {
            Some(to) => json!({
                "action": self.action,
                "path": self.path.display().to_string(),
                "to": to.display().to_string()
            }),
            None => json!({"action": self.action, "path": self.path.display().to_string()}),
        }
    }

    /// One line of the text summary.
    pub(crate) fn line(&self) -> String {
        match &self.to {
            Some(to) => format!(
                "{} {} -> {}",
                self.action,
                self.path.display(),
                to.display()
            ),
            None => format!("{} {}", self.action, self.path.display()),
        }
    }
}

/// Points where a test can stop a deployment as if the process died there (the journal
/// stays; nothing is rolled back).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// The new deployment is complete in `.staging`.
    Staged,
    /// `.previous` was renamed to `.retired`.
    Retired,
    /// The live folder was renamed to `.previous` (deploy) or `.staging` (rollback).
    Moved,
    /// The new folder is live; cleanup has not run.
    Committed,
}

/// Test hooks; production code passes the default (none).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Hooks {
    /// Stop as if the process died after this step.
    pub interrupt_after: Option<Step>,
}

impl Hooks {
    fn check(self, step: Step) -> Result<(), DeployError> {
        if self.interrupt_after == Some(step) {
            return Err(DeployError {
                rule: "interrupted",
                path: None,
                message: format!("test interrupt after {step:?}"),
            });
        }
        Ok(())
    }
}

/// UTC time as RFC 3339 (`2026-10-14T22:15:30Z`) from Unix seconds.
pub(crate) fn rfc3339(unix: u64) -> String {
    civil(unix).map_or_else(
        || format!("unix:{unix}"),
        |(y, m, d, hh, mm, ss)| format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"),
    )
}

/// `(year, month, day, hour, minute, second)` of Unix seconds (the days-to-civil
/// algorithm by H. Hinnant, public domain).
fn civil(unix: u64) -> Option<(i64, u64, u64, u64, u64, u64)> {
    let days = i64::try_from(unix.checked_div(86_400)?).ok()?;
    let secs = unix.checked_rem(86_400)?;
    let z = days.checked_add(719_468)?;
    let era = z.checked_div(146_097)?;
    let doe = z.checked_sub(era.checked_mul(146_097)?)?;
    let yoe = doe
        .checked_sub(doe.checked_div(1460)?)?
        .checked_add(doe.checked_div(36_524)?)?
        .checked_sub(doe.checked_div(146_096)?)?
        .checked_div(365)?;
    let y = yoe.checked_add(era.checked_mul(400)?)?;
    let doy = doe.checked_sub(
        yoe.checked_mul(365)?
            .checked_add(yoe.checked_div(4)?)?
            .checked_sub(yoe.checked_div(100)?)?,
    )?;
    let mp = doy.checked_mul(5)?.checked_add(2)?.checked_div(153)?;
    let d = doy
        .checked_sub(mp.checked_mul(153)?.checked_add(2)?.checked_div(5)?)?
        .checked_add(1)?;
    let m = if mp < 10 {
        mp.checked_add(3)?
    } else {
        mp.checked_sub(9)?
    };
    let y = if m <= 2 { y.checked_add(1)? } else { y };
    Some((
        y,
        u64::try_from(m).ok()?,
        u64::try_from(d).ok()?,
        secs.checked_div(3600)?,
        secs.checked_rem(3600)?.checked_div(60)?,
        secs.checked_rem(60)?,
    ))
}

/// Seconds since the Unix epoch, now.
pub(crate) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
