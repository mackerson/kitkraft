//! Typed errors. Every variant carries the absolute byte offset where the problem was found:
//! an input offset for read errors, an output offset for write errors.

use crate::Signature;

/// An error while reading or writing a plugin.
///
/// `offset` is absolute within the file (or within the buffer handed to the parser). Errors
/// found inside the decompressed body of a compressed record are wrapped in
/// [`Error::InCompressed`], whose `offset` is the record's file offset and whose inner error
/// counts offsets from the start of the decompressed data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A read needed more bytes than the enclosing buffer, record, or group holds.
    #[error("unexpected end of data at offset {offset}: {what} needs {needed} bytes, {available} available")]
    UnexpectedEof {
        offset: u64,
        what: &'static str,
        needed: u64,
        available: u64,
    },

    /// The first entry of a plugin is not a `TES4` record.
    #[error("not a plugin: expected a TES4 header record at offset {offset}, found {found}")]
    NotAPlugin { offset: u64, found: Signature },

    /// A group header declares a size smaller than the 24-byte header itself.
    #[error("group at offset {offset} declares size {size}, smaller than its 24-byte header")]
    GroupTooSmall { offset: u64, size: u32 },

    /// Groups are nested deeper than the parser allows.
    #[error("group at offset {offset} is nested deeper than {max} levels")]
    TooDeep { offset: u64, max: usize },

    /// An `XXXX` subrecord whose own size is not 4.
    #[error("XXXX subrecord at offset {offset} has size {size}; expected 4")]
    BadOversize { offset: u64, size: u16 },

    /// An `XXXX` subrecord with nothing after it in the record.
    #[error("XXXX subrecord at offset {offset} is not followed by a subrecord")]
    DanglingOversize { offset: u64 },

    /// A compressed record body shorter than its 4-byte decompressed-size prefix.
    #[error("compressed record at offset {offset} has a {len}-byte body; at least 4 needed")]
    CompressedTooShort { offset: u64, len: usize },

    /// A compressed record declares a decompressed size above the configured cap.
    #[error("compressed record at offset {offset} declares {declared} decompressed bytes, over the cap of {cap}")]
    DecompressedTooLarge {
        offset: u64,
        declared: u64,
        cap: u64,
    },

    /// The zlib stream of a compressed record is invalid.
    #[error("compressed record at offset {offset}: invalid zlib stream: {message}")]
    Inflate { offset: u64, message: String },

    /// The zlib stream inflated to a different size than the record declares.
    #[error("compressed record at offset {offset}: inflated to {actual} bytes, header declares {declared}")]
    DecompressedSizeMismatch {
        offset: u64,
        declared: u64,
        actual: u64,
    },

    /// A parse error inside the decompressed body of a compressed record.
    #[error("in compressed record at offset {offset}: {source}")]
    InCompressed {
        offset: u64,
        #[source]
        source: Box<Error>,
    },

    /// A decompressed body holds more subrecords than [`crate::DecodeLimits`] allows.
    #[error("compressed record at offset {offset} holds more than {max} subrecords")]
    TooManySubrecords { offset: u64, max: u64 },

    /// A modified node is too large for its size field. `offset` is the output offset.
    #[error(
        "{what} written at output offset {offset} is {len} bytes, too large for its size field"
    )]
    TooLarge {
        offset: u64,
        what: &'static str,
        len: u64,
    },

    /// Compression of a modified record failed. `offset` is the output offset.
    #[error("compressing the record written at output offset {offset}: {message}")]
    Deflate { offset: u64, message: String },

    /// A record's compressed flag disagrees with its body: the flag is set on a
    /// [`crate::Body::Raw`] body, or clear on a [`crate::Body::Compressed`] one. Writing it
    /// would produce a record no reader can parse. `offset` is the output offset.
    #[error("record written at output offset {offset}: compressed flag {} but the body is {}",
        if *flag_set { "set" } else { "clear" },
        if *flag_set { "uncompressed" } else { "compressed" })]
    FlagBodyMismatch { offset: u64, flag_set: bool },

    /// A subrecord built or changed in memory uses the signature `XXXX`, which the format
    /// reserves for size prefixes. `offset` is the output offset.
    #[error("subrecord written at output offset {offset} uses the reserved signature XXXX")]
    ReservedSignature { offset: u64 },

    /// [`crate::index::read_record_at`] found a group header where a record was expected.
    #[error("expected a record at offset {offset}, found {found}")]
    NotARecord { offset: u64, found: Signature },

    /// [`crate::index::read_indexed`] read a record header that disagrees with its index
    /// entry: the index is stale or does not belong to this file.
    #[error("record at offset {offset} does not match its index entry: {what} is {found:#x}, index says {expected:#x}")]
    IndexMismatch {
        offset: u64,
        what: &'static str,
        expected: u64,
        found: u64,
    },

    /// A `TES4` `HEDR` subrecord too short to hold the record count (12 bytes: version,
    /// count, next object ID). `offset` is the subrecord data's source offset, or 0 for one
    /// built in memory.
    #[error("HEDR at offset {offset} has {len} bytes; the record count needs 8")]
    BadHedr { offset: u64, len: u64 },

    /// A subrecord position past the end of a record's subrecords. `offset` is the record's
    /// source offset, or 0 for a record built in memory.
    #[error("record at offset {offset} has {len} subrecords; there is no subrecord {index}")]
    NoSuchSubrecord {
        offset: u64,
        index: usize,
        len: usize,
    },

    /// A record cannot be placed where it was asked to go (see [`crate::Plugin::upsert`]).
    /// `offset` is the record's source offset, or 0 for a record built in memory.
    #[error("cannot place the record at offset {offset}: {reason}")]
    Placement { offset: u64, reason: &'static str },

    /// A broken internal invariant: a bug in this crate, reported instead of a panic.
    #[error("internal error at offset {offset}: {message}")]
    Internal { offset: u64, message: &'static str },

    /// An I/O error while streaming a file.
    #[error("I/O error at offset {offset}: {source}")]
    Io {
        offset: u64,
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    /// The absolute offset this error points at.
    pub fn offset(&self) -> u64 {
        match self {
            Error::UnexpectedEof { offset, .. }
            | Error::NotAPlugin { offset, .. }
            | Error::GroupTooSmall { offset, .. }
            | Error::TooDeep { offset, .. }
            | Error::BadOversize { offset, .. }
            | Error::DanglingOversize { offset }
            | Error::CompressedTooShort { offset, .. }
            | Error::DecompressedTooLarge { offset, .. }
            | Error::Inflate { offset, .. }
            | Error::DecompressedSizeMismatch { offset, .. }
            | Error::InCompressed { offset, .. }
            | Error::TooManySubrecords { offset, .. }
            | Error::TooLarge { offset, .. }
            | Error::Deflate { offset, .. }
            | Error::FlagBodyMismatch { offset, .. }
            | Error::ReservedSignature { offset }
            | Error::NotARecord { offset, .. }
            | Error::IndexMismatch { offset, .. }
            | Error::BadHedr { offset, .. }
            | Error::NoSuchSubrecord { offset, .. }
            | Error::Placement { offset, .. }
            | Error::Internal { offset, .. }
            | Error::Io { offset, .. } => *offset,
        }
    }
}

/// Result alias for this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
