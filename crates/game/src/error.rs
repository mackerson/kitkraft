//! Errors of the session controller.

use std::path::{Path, PathBuf};

/// Everything `kitkraft-game` can fail with.
#[derive(Debug, thiserror::Error)]
pub enum GameError {
    /// A file or folder could not be read or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The path.
        path: PathBuf,
        /// What happened.
        #[source]
        source: std::io::Error,
    },
    /// A game profile (`profiles/<game>.toml`) is malformed.
    #[error("game profile {name}: {message}")]
    Profile {
        /// The profile's name or path.
        name: String,
        /// What is wrong.
        message: String,
    },
    /// A `probes.toml` is malformed.
    #[error("probes: {0}")]
    Probes(String),
    /// A write outside the authorized paths of ADR 0010 Amendment 1 condition 3.
    #[error("refused [unauthorized-write]: {} is not on the session's authorized path list", .0.display())]
    Unauthorized(PathBuf),
    /// The console endpoint could not be used.
    #[error("console: {0}")]
    Console(String),
    /// A process operation failed.
    #[error("process: {0}")]
    Process(String),
}

impl GameError {
    /// An I/O error at `path`.
    pub fn io(path: &Path, source: std::io::Error) -> GameError {
        GameError::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// `Result` with [`GameError`].
pub type Result<T> = std::result::Result<T, GameError>;
