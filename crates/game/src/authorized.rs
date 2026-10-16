//! The authorized-path list (ADR 0010 Amendment 1 condition 3): every file the controller
//! writes goes through [`Authorized`], which refuses anything else and records what it
//! wrote, so a test can assert the list.
//!
//! Allowed: anything under the session's `observe\<yyyy-mm-dd>-T<nn>\` folder (outputs,
//! copied logs, screenshots, backups); one append to the ledger
//! (`docs/game-observations.md`); and, for rollback only, the profile files that were
//! backed up, restored byte for byte from the backup. The game's own writes (its logs,
//! `KitKraftSession-*` saves) are the game's; the controller only names the save.

use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::error::{GameError, Result};

/// The write guard of one session.
#[derive(Debug)]
pub struct Authorized {
    observe: PathBuf,
    ledger: Option<PathBuf>,
    rollback: Vec<PathBuf>,
    written: Mutex<Vec<PathBuf>>,
}

fn plain(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
}

impl Authorized {
    /// The list for a session: its observe folder, the ledger (if any), and the files
    /// rollback may restore.
    pub fn new(observe: &Path, ledger: Option<&Path>, rollback: &[PathBuf]) -> Authorized {
        Authorized {
            observe: observe.to_path_buf(),
            ledger: ledger.map(Path::to_path_buf),
            rollback: rollback.to_vec(),
            written: Mutex::new(Vec::new()),
        }
    }

    /// The session's observe folder.
    pub fn observe(&self) -> &Path {
        &self.observe
    }

    /// The list, for the plan and the report.
    pub fn describe(&self) -> Vec<String> {
        let mut v = vec![format!(
            "{}\\** (outputs, logs, screenshots, backups)",
            self.observe.display()
        )];
        if let Some(l) = &self.ledger {
            v.push(format!("{} (one appended entry)", l.display()));
        }
        for r in &self.rollback {
            v.push(format!("{} (rollback only, from its backup)", r.display()));
        }
        v
    }

    fn under_observe(&self, path: &Path) -> bool {
        plain(path) && plain(&self.observe) && path.starts_with(&self.observe)
    }

    fn check(&self, path: &Path, ledger: bool, rollback: bool) -> Result<()> {
        let ok = self.under_observe(path)
            || (ledger && plain(path) && self.ledger.as_deref() == Some(path))
            || (rollback && plain(path) && self.rollback.iter().any(|r| r == path));
        if ok {
            Ok(())
        } else {
            Err(GameError::Unauthorized(path.to_path_buf()))
        }
    }

    fn note(&self, path: &Path) {
        self.written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(path.to_path_buf());
    }

    /// Every path written so far.
    pub fn written(&self) -> Vec<PathBuf> {
        self.written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Checks and records a path under the observe folder that a helper (the screenshot
    /// tool) writes for the session.
    pub fn claim(&self, path: &Path) -> Result<()> {
        self.check(path, false, false)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| GameError::io(parent, e))?;
        }
        self.note(path);
        Ok(())
    }

    /// Creates a folder under the observe folder.
    pub fn create_dir_all(&self, dir: &Path) -> Result<()> {
        self.check(dir, false, false)?;
        std::fs::create_dir_all(dir).map_err(|e| GameError::io(dir, e))
    }

    /// Writes a file under the observe folder.
    pub fn write(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        self.check(path, false, false)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| GameError::io(parent, e))?;
        }
        std::fs::write(path, bytes).map_err(|e| GameError::io(path, e))?;
        self.note(path);
        Ok(())
    }

    /// Copies `from` (read anywhere) to `to` under the observe folder.
    pub fn copy_in(&self, from: &Path, to: &Path) -> Result<u64> {
        self.check(to, false, false)?;
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| GameError::io(parent, e))?;
        }
        let n = std::fs::copy(from, to).map_err(|e| GameError::io(from, e))?;
        self.note(to);
        Ok(n)
    }

    /// Appends `text` to the ledger.
    pub fn append_ledger(&self, path: &Path, text: &str) -> Result<()> {
        self.check(path, true, false)?;
        if self.under_observe(path) {
            return Err(GameError::Unauthorized(path.to_path_buf()));
        }
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|e| GameError::io(path, e))?;
        f.write_all(text.as_bytes())
            .map_err(|e| GameError::io(path, e))?;
        self.note(path);
        Ok(())
    }

    /// Restores a backed-up profile file byte for byte.
    pub fn restore(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        self.check(path, false, true)?;
        std::fs::write(path, bytes).map_err(|e| GameError::io(path, e))?;
        self.note(path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_paths_are_written() {
        let root = std::env::temp_dir().join(format!("kitkraft-game-auth-{}", std::process::id()));
        let observe = root.join("observe").join("2026-10-16-T01");
        let ledger = root.join("docs").join("game-observations.md");
        let plugins = root.join("profile").join("plugins.txt");
        std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        std::fs::create_dir_all(plugins.parent().unwrap()).unwrap();
        std::fs::write(&ledger, "# Game observations\n").unwrap();
        let a = Authorized::new(&observe, Some(&ledger), std::slice::from_ref(&plugins));
        a.write(&observe.join("report.json"), b"{}").unwrap();
        a.append_ledger(&ledger, "x\n").unwrap();
        a.restore(&plugins, b"*A.esm\n").unwrap();
        for bad in [
            root.join("profile").join("modlist.txt"),
            observe.join("..").join("escape.txt"),
            root.join("observe").join("2026-10-16-T02").join("x"),
        ] {
            assert!(
                matches!(a.write(&bad, b"x"), Err(GameError::Unauthorized(_))),
                "{}",
                bad.display()
            );
        }
        assert!(a.restore(&ledger, b"x").is_err());
        assert!(a.append_ledger(&plugins, "x").is_err());
        assert!(a.write(&ledger, b"x").is_err());
        assert_eq!(a.written().len(), 3);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
