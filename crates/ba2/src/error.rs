//! Typed errors. Every error from reading an archive names the byte offset it concerns.

/// Everything reading, extracting or packing an archive can fail with.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The underlying reader or writer failed.
    #[error("I/O error at offset {offset}: {source}")]
    Io {
        /// Archive offset being read (or output offset being written).
        offset: u64,
        /// The I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A structure runs past the end of the file.
    #[error("truncated at offset {offset}: needed {needed} bytes, {available} available")]
    Truncated {
        /// Where the structure starts.
        offset: u64,
        /// Bytes it needs.
        needed: u64,
        /// Bytes left in the file from `offset`.
        available: u64,
    },
    /// The first four bytes are not `BTDX`.
    #[error("not a BA2 archive: magic {found:02X?} at offset 0")]
    BadMagic {
        /// The bytes found.
        found: [u8; 4],
    },
    /// A version this crate does not read (1, 2 and 3 are read).
    #[error("unsupported BA2 version {version} at offset 4")]
    UnsupportedVersion {
        /// The version field.
        version: u32,
    },
    /// The type tag is neither `GNRL` nor `DX10`.
    #[error("unknown archive type {found:02X?} at offset 8")]
    UnknownKind {
        /// The bytes found.
        found: [u8; 4],
    },
    /// An offset or size from the archive points outside the file.
    #[error("{what} at offset {offset} is out of range: {value} (file is {file_len} bytes)")]
    OutOfRange {
        /// Which field.
        what: &'static str,
        /// Where the field is.
        offset: u64,
        /// Its value.
        value: u64,
        /// File length.
        file_len: u64,
    },
    /// A texture record whose chunk records are not 24 bytes.
    #[error("texture record at offset {offset} declares {found}-byte chunk records, expected 24")]
    ChunkHeaderLen {
        /// Record offset.
        offset: u64,
        /// The declared length.
        found: u16,
    },
    /// A v3 compression method other than the ones the corpus shows.
    #[error("unsupported compression method {method} (header offset 32)")]
    UnsupportedCodec {
        /// The header field.
        method: u32,
    },
    /// Compressed data does not decompress.
    #[error("decompressing `{name}` at offset {offset}: {message}")]
    Decompress {
        /// Archive path of the file.
        name: String,
        /// Offset of the compressed data.
        offset: u64,
        /// The codec's message.
        message: String,
    },
    /// Data decompresses (or is stored) at a size other than the record states.
    #[error("`{name}` at offset {offset} yields {actual} bytes, record says {expected}")]
    SizeMismatch {
        /// Archive path of the file.
        name: String,
        /// Offset of the data.
        offset: u64,
        /// Size from the record.
        expected: u64,
        /// Size produced (at most `expected + 1`: decoding stops there).
        actual: u64,
    },
    /// An entry index past the end.
    #[error("entry index {index} out of range ({count} entries)")]
    NoSuchEntry {
        /// Requested index.
        index: usize,
        /// Entries in the archive.
        count: usize,
    },
    /// An archive path that cannot be turned into a relative file path safely.
    #[error("unsafe archive path `{name}`: {reason}")]
    UnsafePath {
        /// The archive path.
        name: String,
        /// Why it was refused.
        reason: &'static str,
    },
    /// Packing refused an input.
    #[error("cannot pack `{name}`: {reason}")]
    Pack {
        /// Archive path (or input) concerned.
        name: String,
        /// Why.
        reason: String,
    },
}

/// Shorthand.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(offset: u64) -> impl FnOnce(std::io::Error) -> Error {
    move |source| Error::Io { offset, source }
}
