//! Errors of the document model.

use std::path::PathBuf;

/// Everything `kitcraft-doc` can fail with.
#[derive(Debug, thiserror::Error)]
pub enum DocError {
    /// The plugin could not be parsed (the message carries the byte offset).
    #[error("{file}: {source}")]
    Esm {
        /// File name.
        file: String,
        /// Container error, with its absolute offset.
        #[source]
        source: kitcraft_esm::Error,
    },
    /// A file could not be read or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// What happened.
        #[source]
        source: std::io::Error,
    },
    /// A record expected at an offset is not there (stale index, wrong file).
    #[error("{file}: no {expected} record at offset {offset} ({found})")]
    RecordMismatch {
        /// File name.
        file: String,
        /// Absolute offset.
        offset: u64,
        /// Expected `SIGN:FORMID`.
        expected: String,
        /// What was found instead.
        found: String,
    },
    /// A plugin name is not in the load order.
    #[error("plugin `{0}` is not in the load order")]
    UnknownPlugin(String),
    /// Bad input to a parser in this crate (`where` text, formref, cursor).
    #[error("{0}")]
    Invalid(String),
}

/// Shorthand.
pub type Result<T, E = DocError> = std::result::Result<T, E>;

impl DocError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> DocError {
        DocError::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn esm(file: impl Into<String>, source: kitcraft_esm::Error) -> DocError {
        DocError::Esm {
            file: file.into(),
            source,
        }
    }
}
