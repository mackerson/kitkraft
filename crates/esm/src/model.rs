//! The plugin tree: header record, groups, records, subrecords.
//!
//! Every header field is kept exactly as read, including ones whose meaning is unknown.
//! Data borrows from the input buffer where it can, so parsing a top-level group costs
//! little beyond the group's own bytes. Subrecords decoded from a compressed record share
//! one buffer holding the decompressed body.
//!
//! Mutation goes through `*_mut` accessors, which mark the node modified. Because a child
//! is reachable mutably only through its parent's `*_mut` accessor, every ancestor of a
//! modified node is marked too, and the writer recomputes sizes exactly along those paths.
//! Unmodified nodes are written with the sizes they were read with.

use std::borrow::Cow;
use std::sync::Arc;

use crate::Signature;

/// Record flag: the record body is zlib-compressed (UESP, Skyrim_Mod:Mod_File_Format,
/// "Records"). The body is then a `u32` decompressed size followed by a zlib stream.
pub const FLAG_COMPRESSED: u32 = 0x0004_0000;

/// Size in bytes of a record header and of a group header.
pub const HEADER_LEN: usize = 24;

/// Size in bytes of a subrecord header.
pub const SUBRECORD_HEADER_LEN: usize = 6;

/// A whole plugin: the `TES4` header record and the entries that follow it.
#[derive(Debug, Clone, PartialEq)]
pub struct Plugin<'a> {
    pub(crate) header: Record<'a>,
    pub(crate) entries: Vec<Entry<'a>>,
    /// Records (excluding `TES4`) and groups in `entries` when parsed, or `None` for a
    /// plugin built in memory. The writer compares it with the current tree to decide
    /// whether the `HEDR` record count must be recomputed (see [`crate::tes4`]).
    pub(crate) counts_as_read: Option<Counts>,
}

impl<'a> Plugin<'a> {
    /// A plugin with the given header record and no entries.
    pub fn new(header: Record<'a>) -> Plugin<'a> {
        Plugin {
            header,
            entries: Vec::new(),
            counts_as_read: None,
        }
    }

    /// The `TES4` header record.
    pub fn header(&self) -> &Record<'a> {
        &self.header
    }

    /// Mutable access to the header record.
    pub fn header_mut(&mut self) -> &mut Record<'a> {
        &mut self.header
    }

    /// Top-level entries (normally groups of type 0), in file order.
    pub fn entries(&self) -> &[Entry<'a>] {
        &self.entries
    }

    /// Mutable access to the top-level entries.
    pub fn entries_mut(&mut self) -> &mut Vec<Entry<'a>> {
        &mut self.entries
    }

    /// Records (excluding `TES4`) and groups as parsed, or `None` for a plugin built in
    /// memory.
    pub fn counts_as_read(&self) -> Option<Counts> {
        self.counts_as_read
    }

    /// Copy every borrowed byte so the plugin no longer borrows the input buffer.
    pub fn into_owned(self) -> Plugin<'static> {
        Plugin {
            header: self.header.into_owned(),
            entries: self.entries.into_iter().map(Entry::into_owned).collect(),
            counts_as_read: self.counts_as_read,
        }
    }
}

/// Records and groups below the `TES4` record, at any depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    /// Records, not counting `TES4`.
    pub records: u64,
    /// Groups.
    pub groups: u64,
}

impl Counts {
    /// Count every record and group in `entries`, at any depth.
    pub fn of(entries: &[Entry<'_>]) -> Counts {
        let mut counts = Counts::default();
        for entry in entries {
            counts.add_entry(entry);
        }
        counts
    }

    /// Add `entry` and everything below it.
    pub fn add_entry(&mut self, entry: &Entry<'_>) {
        match entry {
            Entry::Record(_) => self.records = self.records.saturating_add(1),
            Entry::Group(g) => {
                self.groups = self.groups.saturating_add(1);
                for child in &g.children {
                    self.add_entry(child);
                }
            }
        }
    }
}

/// A group or a record, as found inside a group or at the top level.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry<'a> {
    Group(Group<'a>),
    Record(Record<'a>),
}

impl<'a> Entry<'a> {
    /// Absolute source offset, or `None` for a node built in memory.
    pub fn offset(&self) -> Option<u64> {
        match self {
            Entry::Group(g) => g.offset(),
            Entry::Record(r) => r.offset(),
        }
    }

    /// Whether this entry (or anything below it) was reached mutably or built in memory.
    pub fn is_modified(&self) -> bool {
        match self {
            Entry::Group(g) => g.is_modified(),
            Entry::Record(r) => r.is_modified(),
        }
    }

    /// Copy every borrowed byte.
    pub fn into_owned(self) -> Entry<'static> {
        match self {
            Entry::Group(g) => Entry::Group(g.into_owned()),
            Entry::Record(r) => Entry::Record(r.into_owned()),
        }
    }
}

/// Group header fields other than the signature and size (UESP, Skyrim_Mod:Mod_File_Format,
/// "Groups").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GroupHeader {
    /// Label; its meaning depends on `group_type` (record type, FormID, block number, grid).
    pub label: [u8; 4],
    /// Group type (0 = top, 1 = world children, ... 9 = cell temporary children).
    pub group_type: i32,
    /// Timestamp.
    pub timestamp: u16,
    /// Version control info.
    pub version_control: u16,
    /// Unknown 32-bit field; preserved verbatim.
    pub unknown: u32,
}

impl GroupHeader {
    /// The label read as a signature (meaningful for top groups, type 0).
    pub fn label_signature(&self) -> Signature {
        Signature(self.label)
    }

    /// The label read as a little-endian `u32` (FormID or block number).
    pub fn label_u32(&self) -> u32 {
        u32::from_le_bytes(self.label)
    }
}

/// A `GRUP`: a header and the records and groups it contains.
#[derive(Debug, Clone, PartialEq)]
pub struct Group<'a> {
    /// Header fields; editing them never changes the group's size.
    pub header: GroupHeader,
    pub(crate) size: Option<u32>,
    pub(crate) offset: Option<u64>,
    pub(crate) children: Vec<Entry<'a>>,
    pub(crate) modified: bool,
}

impl<'a> Group<'a> {
    /// A new, empty group. Its size is computed when written.
    pub fn new(header: GroupHeader) -> Group<'a> {
        Group {
            header,
            size: None,
            offset: None,
            children: Vec::new(),
            modified: true,
        }
    }

    /// Size field as read (including the 24-byte header), or `None` for a new group.
    pub fn size_as_read(&self) -> Option<u32> {
        self.size
    }

    /// Absolute source offset of the group header, or `None` for a new group.
    pub fn offset(&self) -> Option<u64> {
        self.offset
    }

    /// Records and subgroups, in order.
    pub fn children(&self) -> &[Entry<'a>] {
        &self.children
    }

    /// Mutable access to the children; marks the group modified.
    pub fn children_mut(&mut self) -> &mut Vec<Entry<'a>> {
        self.modified = true;
        &mut self.children
    }

    /// Whether the group was reached mutably or built in memory.
    pub fn is_modified(&self) -> bool {
        self.modified || self.size.is_none()
    }

    /// Copy every borrowed byte.
    pub fn into_owned(self) -> Group<'static> {
        Group {
            header: self.header,
            size: self.size,
            offset: self.offset,
            children: self.children.into_iter().map(Entry::into_owned).collect(),
            modified: self.modified,
        }
    }
}

/// Record header fields other than the data size (UESP, Skyrim_Mod:Mod_File_Format,
/// "Records").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecordHeader {
    /// Record type.
    pub signature: Signature,
    /// Record flags; see [`FLAG_COMPRESSED`] and the TES4 flags in [`crate::tes4`].
    pub flags: u32,
    /// FormID.
    pub form_id: u32,
    /// Timestamp.
    pub timestamp: u16,
    /// Version control info.
    pub version_control: u16,
    /// Internal form version. Starfield values are taken from the corpus (UESP lists only
    /// Skyrim and Fallout 4 values).
    pub form_version: u16,
    /// Unknown 16-bit field; preserved verbatim.
    pub unknown: u16,
}

impl RecordHeader {
    /// Whether the compressed flag is set.
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }
}

/// A record body.
///
/// The record header's [`FLAG_COMPRESSED`] bit must agree with the variant: the writer
/// returns [`crate::Error::FlagBodyMismatch`] for a `Raw` body under a set bit or a
/// `Compressed` body under a clear one.
#[derive(Debug, Clone, PartialEq)]
pub enum Body<'a> {
    /// Uncompressed: a sequence of subrecords.
    Raw(Vec<Subrecord<'a>>),
    /// Compressed: the original bytes (the `u32` decompressed size and the zlib stream,
    /// exactly as read) and, once decoded, the subrecords they hold.
    ///
    /// The writer re-emits `original` unless the record is marked modified and `decoded`
    /// is present. Then the decoded subrecords are serialized; if that reproduces
    /// `inflated` byte for byte, `original` is still re-emitted, otherwise the subrecords
    /// are recompressed.
    Compressed {
        original: Cow<'a, [u8]>,
        decoded: Option<Vec<Subrecord<'a>>>,
        /// The decompressed bytes of `original`, set by [`Record::decompress`]. Leave it
        /// `None` when building or replacing a body by hand: a value that does not belong
        /// to `original` makes the writer emit `original` for subrecords that serialize to
        /// that value.
        inflated: Option<Inflated>,
    },
}

/// The decompressed bytes of a compressed record body, shared with the subrecords decoded
/// from them (no copy per subrecord).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inflated(pub(crate) Arc<Vec<u8>>);

impl Inflated {
    /// The decompressed bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A record: a header and a body.
#[derive(Debug, Clone, PartialEq)]
pub struct Record<'a> {
    /// Header fields; editing them never changes the record's size. The compressed flag
    /// must match the body variant (see [`Body`]).
    pub header: RecordHeader,
    pub(crate) data_size: Option<u32>,
    pub(crate) offset: Option<u64>,
    pub(crate) body: Body<'a>,
    pub(crate) modified: bool,
}

impl<'a> Record<'a> {
    /// A new record with the given header and body. Its data size is computed when written.
    pub fn new(header: RecordHeader, body: Body<'a>) -> Record<'a> {
        Record {
            header,
            data_size: None,
            offset: None,
            body,
            modified: true,
        }
    }

    /// Data size field as read (excluding the header), or `None` for a new record.
    pub fn data_size_as_read(&self) -> Option<u32> {
        self.data_size
    }

    /// Absolute source offset of the record header, or `None` for a new record.
    pub fn offset(&self) -> Option<u64> {
        self.offset
    }

    /// The body.
    pub fn body(&self) -> &Body<'a> {
        &self.body
    }

    /// Mutable access to the body; marks the record modified.
    pub fn body_mut(&mut self) -> &mut Body<'a> {
        self.modified = true;
        &mut self.body
    }

    /// The subrecords, or `None` for a compressed record that has not been decoded.
    pub fn subrecords(&self) -> Option<&[Subrecord<'a>]> {
        match &self.body {
            Body::Raw(subs) => Some(subs),
            Body::Compressed {
                decoded: Some(subs),
                ..
            } => Some(subs),
            Body::Compressed { decoded: None, .. } => None,
        }
    }

    /// Mutable subrecords, or `None` for a compressed record that has not been decoded
    /// (see [`Record::decompress`]). Marks the record modified.
    pub fn subrecords_mut(&mut self) -> Option<&mut Vec<Subrecord<'a>>> {
        let subs = match &mut self.body {
            Body::Raw(subs) => subs,
            Body::Compressed {
                decoded: Some(subs),
                ..
            } => subs,
            Body::Compressed { decoded: None, .. } => return None,
        };
        self.modified = true;
        Some(subs)
    }

    /// Whether the record was reached mutably or built in memory.
    pub fn is_modified(&self) -> bool {
        self.modified || self.data_size.is_none()
    }

    /// Copy every borrowed byte.
    pub fn into_owned(self) -> Record<'static> {
        let body = match self.body {
            Body::Raw(subs) => Body::Raw(subs.into_iter().map(Subrecord::into_owned).collect()),
            Body::Compressed {
                original,
                decoded,
                inflated,
            } => Body::Compressed {
                original: Cow::Owned(original.into_owned()),
                decoded: decoded.map(|subs| subs.into_iter().map(Subrecord::into_owned).collect()),
                inflated,
            },
        };
        Record {
            header: self.header,
            data_size: self.data_size,
            offset: self.offset,
            body,
            modified: self.modified,
        }
    }
}

/// How a subrecord's size is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    /// Exactly as read: the `u16` size field, and whether an `XXXX` prefix carried the size.
    AsRead { size_field: u16, oversize: bool },
    /// Computed on write: an `XXXX` prefix when `oversize` is set or the data does not fit
    /// a `u16`.
    Canonical { oversize: bool },
}

/// Subrecord data: borrowed from the input, owned, or a range of a shared buffer (the
/// decompressed body of a compressed record, decoded once for all of its subrecords).
#[derive(Clone)]
pub(crate) enum Data<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
    Shared {
        buf: Arc<Vec<u8>>,
        start: u32,
        len: u32,
    },
}

impl Data<'_> {
    pub(crate) fn as_slice(&self) -> &[u8] {
        match self {
            Data::Borrowed(b) => b,
            Data::Owned(v) => v,
            Data::Shared { buf, start, len } => {
                let start = *start as usize;
                start
                    .checked_add(*len as usize)
                    .and_then(|end| buf.get(start..end))
                    .unwrap_or(&[])
            }
        }
    }

    fn into_owned(self) -> Data<'static> {
        match self {
            Data::Borrowed(b) => Data::Owned(b.to_vec()),
            Data::Owned(v) => Data::Owned(v),
            Data::Shared { buf, start, len } => Data::Shared { buf, start, len },
        }
    }
}

impl<'a> From<Cow<'a, [u8]>> for Data<'a> {
    fn from(cow: Cow<'a, [u8]>) -> Data<'a> {
        match cow {
            Cow::Borrowed(b) => Data::Borrowed(b),
            Cow::Owned(v) => Data::Owned(v),
        }
    }
}

impl PartialEq for Data<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl std::fmt::Debug for Data<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_slice(), f)
    }
}

/// Stored in [`Subrecord`] for "no source offset".
const NO_OFFSET: u64 = u64::MAX;

/// A subrecord (UESP calls them fields): a signature and opaque data.
///
/// An `XXXX` prefix is not a separate subrecord here: it is folded into the subrecord whose
/// size it carries, and written back exactly as read. A subrecord built or changed in
/// memory may not use the signature `XXXX` itself; the writer rejects it
/// ([`crate::Error::ReservedSignature`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Subrecord<'a> {
    pub(crate) signature: Signature,
    pub(crate) encoding: Encoding,
    pub(crate) data: Data<'a>,
    /// Absolute source offset, or [`NO_OFFSET`].
    pub(crate) offset: u64,
}

impl<'a> Subrecord<'a> {
    /// A new subrecord. Its size (and any `XXXX` prefix) is computed when written.
    pub fn new(signature: Signature, data: impl Into<Cow<'a, [u8]>>) -> Subrecord<'a> {
        Subrecord {
            signature,
            encoding: Encoding::Canonical { oversize: false },
            data: Data::from(data.into()),
            offset: NO_OFFSET,
        }
    }

    /// Force an `XXXX` size prefix even when the data fits a `u16` (`true`), or write one
    /// only when it does not fit (`false`). The size is recomputed when written.
    pub fn set_oversize_prefix(&mut self, oversize: bool) {
        self.encoding = Encoding::Canonical { oversize };
    }

    /// A subrecord as read; `offset` is `None` inside a decompressed body.
    pub(crate) fn read(
        signature: Signature,
        data: Data<'a>,
        encoding: Encoding,
        offset: Option<u64>,
    ) -> Subrecord<'a> {
        Subrecord {
            signature,
            encoding,
            data,
            offset: offset.unwrap_or(NO_OFFSET),
        }
    }

    /// The signature.
    pub fn signature(&self) -> Signature {
        self.signature
    }

    /// The data, excluding the header.
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// Replace the data; the size is recomputed when written, with an `XXXX` prefix only
    /// when the data does not fit a `u16` (see [`Subrecord::set_oversize_prefix`]).
    pub fn set_data(&mut self, data: impl Into<Cow<'a, [u8]>>) {
        self.data = Data::from(data.into());
        self.encoding = Encoding::Canonical { oversize: false };
    }

    /// Absolute source offset of the subrecord's first byte (its `XXXX` prefix, if it has
    /// one), or `None` for a subrecord built in memory or decoded from a compressed record
    /// (positions there are relative to the decompressed data; see [`crate::Error`]).
    pub fn offset(&self) -> Option<u64> {
        (self.offset != NO_OFFSET).then_some(self.offset)
    }

    /// Absolute source offset of the data, past the header and any `XXXX` prefix.
    pub fn data_offset(&self) -> Option<u64> {
        let header = match self.encoding {
            Encoding::AsRead { oversize: true, .. } => (2 * SUBRECORD_HEADER_LEN + 4) as u64,
            _ => SUBRECORD_HEADER_LEN as u64,
        };
        self.offset().map(|o| o.saturating_add(header))
    }

    /// The `u16` size field as read, or `None` if the subrecord was built or changed in
    /// memory. It is 0 for a subrecord whose size came from an `XXXX` prefix.
    pub fn size_field_as_read(&self) -> Option<u16> {
        match self.encoding {
            Encoding::AsRead { size_field, .. } => Some(size_field),
            Encoding::Canonical { .. } => None,
        }
    }

    /// Whether the subrecord is written with an `XXXX` size prefix.
    pub fn has_oversize_prefix(&self) -> bool {
        match self.encoding {
            Encoding::AsRead { oversize, .. } => oversize,
            Encoding::Canonical { oversize } => {
                oversize || self.data().len() > usize::from(u16::MAX)
            }
        }
    }

    /// Number of bytes this subrecord occupies when written (header, data, any prefix).
    pub fn encoded_len(&self) -> u64 {
        let prefix = if self.has_oversize_prefix() {
            (SUBRECORD_HEADER_LEN + 4) as u64
        } else {
            0
        };
        prefix
            .saturating_add(SUBRECORD_HEADER_LEN as u64)
            .saturating_add(self.data().len() as u64)
    }

    /// Copy borrowed data. Data shared with a decompressed body stays shared.
    pub fn into_owned(self) -> Subrecord<'static> {
        Subrecord {
            signature: self.signature,
            encoding: self.encoding,
            data: self.data.into_owned(),
            offset: self.offset,
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::Subrecord;

    #[test]
    fn subrecord_stays_compact() {
        // The tree of a large top-level group holds millions of these.
        assert!(std::mem::size_of::<Subrecord<'_>>() <= 40);
    }
}
