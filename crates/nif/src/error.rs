//! Typed errors, each with the file offset where reading stopped.

/// Why a model could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The file does not start with the version line every official model starts with.
    #[error(
        "not a NIF this crate reads: the first line is not the version line of the official models"
    )]
    NotNif,
    /// A field holds a value no official model has.
    #[error("{what} at offset {offset}: unsupported value {value}")]
    Unsupported {
        /// The field.
        what: &'static str,
        /// File offset of the field.
        offset: u64,
        /// The value found.
        value: u64,
    },
    /// The file ends inside a field.
    #[error("{what} at offset {offset}: the file ends first")]
    Truncated {
        /// The field.
        what: &'static str,
        /// File offset of the field.
        offset: u64,
    },
    /// An index or reference past the end of its table.
    #[error("{what} at offset {offset}: {value} is out of range")]
    OutOfRange {
        /// The field.
        what: &'static str,
        /// File offset of the field.
        offset: u64,
        /// The value found.
        value: u64,
    },
    /// Bytes after the footer.
    #[error("{len} bytes after the footer, at offset {offset}")]
    Trailing {
        /// File offset of the first extra byte.
        offset: u64,
        /// How many extra bytes.
        len: usize,
    },
}

/// Result with this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
