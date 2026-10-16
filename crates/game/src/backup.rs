//! Backups and restore-only rollback (ADR 0010 Amendment 1 condition 4).
//!
//! Before launch the controller copies the dev profile's list files, its INIs and the base
//! save into `observe\<session>\backup\` with their SHA-256. After the game exits it hashes
//! the originals again; a file that changed is restored byte for byte from its copy, the
//! difference is a finding, and the session is `inconclusive`. A file that did not exist
//! before and exists after is reported and left alone: rollback only restores, it never
//! deletes.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::authorized::Authorized;
use crate::error::{GameError, Result};

/// A SHA-256 of bytes as lowercase hex (the caller's implementation).
pub type Digest = fn(&[u8]) -> String;

/// One file backed up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Backup {
    /// The original.
    pub source: PathBuf,
    /// The copy under the observe folder (`None` when the original did not exist).
    pub copy: Option<PathBuf>,
    /// The original's SHA-256 before the session (`None`: it did not exist).
    pub sha256: Option<String>,
    /// Its size in bytes.
    pub size: u64,
}

/// A file that differs after the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// The original.
    pub path: PathBuf,
    /// SHA-256 before (`None`: absent).
    pub before: Option<String>,
    /// SHA-256 after (`None`: absent).
    pub after: Option<String>,
    /// Whether the backup was put back (and verified).
    pub restored: bool,
    /// What happened.
    pub note: String,
}

fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(GameError::io(path, e)),
    }
}

/// Copies each of `sources` into `dir` (under the observe folder) and hashes it.
pub fn take(
    sources: &[PathBuf],
    dir: &Path,
    guard: &Authorized,
    digest: Digest,
) -> Result<Vec<Backup>> {
    guard.create_dir_all(dir)?;
    let mut out = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let bytes = read(src)?;
        let name = src
            .file_name()
            .map_or_else(|| format!("file{i}"), |n| n.to_string_lossy().into_owned());
        match bytes {
            Some(b) => {
                let copy = dir.join(format!("{i:02}-{name}"));
                guard.write(&copy, &b)?;
                out.push(Backup {
                    source: src.clone(),
                    copy: Some(copy),
                    sha256: Some(digest(&b)),
                    size: u64::try_from(b.len()).unwrap_or(u64::MAX),
                });
            }
            None => out.push(Backup {
                source: src.clone(),
                copy: None,
                sha256: None,
                size: 0,
            }),
        }
    }
    Ok(out)
}

/// The backed-up files whose hash differs now.
pub fn verify(backups: &[Backup], digest: Digest) -> Result<Vec<Change>> {
    let mut out = Vec::new();
    for b in backups {
        let after = read(&b.source)?.map(|bytes| digest(&bytes));
        if after != b.sha256 {
            out.push(Change {
                path: b.source.clone(),
                before: b.sha256.clone(),
                after,
                restored: false,
                note: String::new(),
            });
        }
    }
    Ok(out)
}

/// Restores every changed file that existed before from its backup and checks the hash.
pub fn restore(backups: &[Backup], changes: &mut [Change], guard: &Authorized, digest: Digest) {
    for c in changes.iter_mut() {
        let Some(b) = backups.iter().find(|b| b.source == c.path) else {
            c.note = "no backup".into();
            continue;
        };
        let Some(copy) = &b.copy else {
            c.note =
                "did not exist before the session; left in place (rollback only restores)".into();
            continue;
        };
        let result = std::fs::read(copy)
            .map_err(|e| GameError::io(copy, e))
            .and_then(|bytes| {
                guard.restore(&c.path, &bytes)?;
                Ok(bytes)
            });
        match result {
            Ok(bytes) if Some(digest(&bytes)) == b.sha256 => {
                c.restored = true;
                c.note = format!("restored from {}", copy.display());
            }
            Ok(_) => c.note = "restored, but the backup's hash differs".into(),
            Err(e) => c.note = format!("not restored: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_digest(b: &[u8]) -> String {
        format!("{}:{}", b.len(), String::from_utf8_lossy(b))
    }

    #[test]
    fn a_changed_file_is_restored_and_a_new_one_left() {
        let root =
            std::env::temp_dir().join(format!("kitkraft-game-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let profile = root.join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        let plugins = profile.join("plugins.txt");
        let modlist = profile.join("modlist.txt");
        let locked = profile.join("lockedorder.txt");
        std::fs::write(&plugins, "*A.esm\n").unwrap();
        std::fs::write(&modlist, "+A\n").unwrap();
        let sources = vec![plugins.clone(), modlist.clone(), locked.clone()];
        let observe = root.join("observe").join("2026-10-16-T01");
        let guard = Authorized::new(&observe, None, &sources);
        let backups = take(&sources, &observe.join("backup"), &guard, fake_digest).unwrap();
        assert_eq!(backups.len(), 3);
        assert!(backups[2].copy.is_none());
        assert!(verify(&backups, fake_digest).unwrap().is_empty());

        std::fs::write(&plugins, "*A.esm\n*B.esm\n").unwrap();
        std::fs::write(&locked, "x\n").unwrap();
        let mut changes = verify(&backups, fake_digest).unwrap();
        assert_eq!(changes.len(), 2);
        restore(&backups, &mut changes, &guard, fake_digest);
        assert!(changes[0].restored, "{changes:?}");
        assert_eq!(std::fs::read_to_string(&plugins).unwrap(), "*A.esm\n");
        assert!(!changes[1].restored);
        assert!(changes[1].note.contains("only restores"));
        assert!(locked.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
