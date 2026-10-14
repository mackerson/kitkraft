//! The plugin tree: header record, groups, records, subrecords.
//!
//! Every header field is kept exactly as read, including ones whose meaning is unknown.
//! Data borrows from the input buffer where it can (`Cow::Borrowed`), so parsing a
//! top-level group costs little beyond the group's own bytes.
//!
//! Mutation goes through `*_mut` accessors, which mark the node modified. Because a child
//! is reachable mutably only through its parent's `*_mut` accessor, every ancestor of a
//! modified node is marked too, and the writer recomputes sizes exactly along those paths.
//! Unmodified nodes are written with the sizes they were read with.

use std::borrow::Cow;

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
}

impl<'a> Plugin<'a> {
    /// A plugin with the given header record and no entries.
    pub fn new(header: Record<'a>) -> Plugin<'a> {
        Plugin {
            header,
            entries: Vec::new(),
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

    /// Copy every borrowed byte so the plugin no longer borrows the input buffer.
    pub fn into_owned(self) -> Plugin<'static> {
        Plugin {
            header: self.header.into_owned(),
            entries: self.entries.into_iter().map(Entry::into_owned).collect(),
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
#[derive(Debug, Clone, PartialEq)]
pub enum Body<'a> {
    /// Uncompressed: a sequence of subrecords.
    Raw(Vec<Subrecord<'a>>),
    /// Compressed: the original bytes (the `u32` decompressed size and the zlib stream,
    /// exactly as read) and, once decoded, the subrecords they hold (owned data).
    ///
    /// The writer re-emits `original` unless the record is marked modified and `decoded`
    /// is present, in which case the decoded subrecords are recompressed.
    Compressed {
        original: Cow<'a, [u8]>,
        decoded: Option<Vec<Subrecord<'a>>>,
    },
}

/// A record: a header and a body.
#[derive(Debug, Clone, PartialEq)]
pub struct Record<'a> {
    /// Header fields; editing them never changes the record's size.
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
            Body::Compressed { original, decoded } => Body::Compressed {
                original: Cow::Owned(original.into_owned()),
                decoded: decoded.map(|subs| subs.into_iter().map(Subrecord::into_owned).collect()),
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
    /// Computed on write: an `XXXX` prefix only when the data does not fit a `u16`.
    Canonical,
}

/// A subrecord (UESP calls them fields): a signature and opaque data.
///
/// An `XXXX` prefix is not a separate subrecord here: it is folded into the subrecord whose
/// size it carries, and written back exactly as read.
#[derive(Debug, Clone, PartialEq)]
pub struct Subrecord<'a> {
    pub(crate) signature: Signature,
    pub(crate) data: Cow<'a, [u8]>,
    pub(crate) encoding: Encoding,
}

impl<'a> Subrecord<'a> {
    /// A new subrecord. Its size (and any `XXXX` prefix) is computed when written.
    pub fn new(signature: Signature, data: impl Into<Cow<'a, [u8]>>) -> Subrecord<'a> {
        Subrecord {
            signature,
            data: data.into(),
            encoding: Encoding::Canonical,
        }
    }

    /// The signature.
    pub fn signature(&self) -> Signature {
        self.signature
    }

    /// The data, excluding the header.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Replace the data; the size is recomputed when written.
    pub fn set_data(&mut self, data: impl Into<Cow<'a, [u8]>>) {
        self.data = data.into();
        self.encoding = Encoding::Canonical;
    }

    /// The `u16` size field as read, or `None` if the subrecord was built or changed in
    /// memory. It is 0 for a subrecord whose size came from an `XXXX` prefix.
    pub fn size_field_as_read(&self) -> Option<u16> {
        match self.encoding {
            Encoding::AsRead { size_field, .. } => Some(size_field),
            Encoding::Canonical => None,
        }
    }

    /// Whether the subrecord is written with an `XXXX` size prefix.
    pub fn has_oversize_prefix(&self) -> bool {
        match self.encoding {
            Encoding::AsRead { oversize, .. } => oversize,
            Encoding::Canonical => self.data.len() > usize::from(u16::MAX),
        }
    }

    /// Number of bytes this subrecord occupies when written (header, data, any prefix).
    pub fn encoded_len(&self) -> u64 {
        let prefix = if self.has_oversize_prefix() {
            (SUBRECORD_HEADER_LEN + 4) as u64
        } else {
            0
        };
        prefix + SUBRECORD_HEADER_LEN as u64 + self.data.len() as u64
    }

    /// Copy borrowed data.
    pub fn into_owned(self) -> Subrecord<'static> {
        Subrecord {
            signature: self.signature,
            data: Cow::Owned(self.data.into_owned()),
            encoding: self.encoding,
        }
    }
}
