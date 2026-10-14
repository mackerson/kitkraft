//! Errors of the profile reader.

use std::path::PathBuf;

/// Everything `kitcraft-io` can fail with.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    /// A file or directory could not be read.
    #[error("{}: {source}", path.display())]
    Io {
        /// The path.
        path: PathBuf,
        /// What happened.
        #[source]
        source: std::io::Error,
    },
    /// The directory is not an MO2 profile (no `modlist.txt`).
    #[error("{}: not an MO2 profile (no modlist.txt)", .0.display())]
    NotAProfile(PathBuf),
    /// No `ModOrganizer.ini` two levels above the profile, or it has no `gamePath`.
    #[error("{0}")]
    Instance(String),
    /// The load-order model refused the plugin list.
    #[error(transparent)]
    Doc(#[from] kitcraft_doc::DocError),
}

/// Shorthand.
pub type Result<T, E = IoError> = std::result::Result<T, E>;

impl IoError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> IoError {
        IoError::Io {
            path: path.into(),
            source,
        }
    }
}
