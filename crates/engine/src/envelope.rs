//! The result envelope every command returns (see `docs/mcp.md`, "Result envelope").

use serde::Serialize;
use serde_json::Value;

/// How much of the examined input a result decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverageStatus {
    /// Every byte of every examined record was decoded.
    Complete,
    /// Some bytes were kept opaque or not examined.
    Partial,
    /// The command cannot say (for example, it is a stub).
    Unknown,
}

/// The `coverage` member of the envelope.
///
/// Counts are `None` (serialized as `null`) when the command did not count them;
/// unknown is never reported as zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// Overall claim.
    pub status: CoverageStatus,
    /// Subrecords decoded through a schema definition.
    pub decoded_subrecords: Option<u64>,
    /// Subrecords kept as opaque bytes.
    pub opaque_subrecords: Option<u64>,
    /// Inputs (files, groups, record types) the command did not look at.
    pub not_examined: Vec<String>,
}

impl Coverage {
    /// Coverage of a command that cannot say what it examined.
    pub fn unknown() -> Self {
        Self {
            status: CoverageStatus::Unknown,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        }
    }
}

/// Where a value came from: one byte range in one plugin. Serialized with the
/// [`crate::shapes::kind::EVIDENCE`] marker first: `{kind, file, offset, length, record,
/// subrecord}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// File name as loaded (not a full path).
    pub file: String,
    /// Absolute byte offset in the file.
    pub offset: u64,
    /// Length in bytes.
    pub length: u64,
    /// `SIGN:FORMID`, when the range is inside a record.
    pub record: Option<String>,
    /// Subrecord signature, when the range is one subrecord.
    pub subrecord: Option<String>,
}

impl Serialize for Evidence {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Evidence", 6)?;
        s.serialize_field(crate::shapes::KIND, crate::shapes::kind::EVIDENCE)?;
        s.serialize_field("file", &self.file)?;
        s.serialize_field("offset", &self.offset)?;
        s.serialize_field("length", &self.length)?;
        s.serialize_field("record", &self.record)?;
        s.serialize_field("subrecord", &self.subrecord)?;
        s.end()
    }
}

/// What a command handler returns; [`crate::Engine::execute`] adds the command id and
/// serializes it as the envelope.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outcome {
    /// Command-specific payload.
    pub data: Value,
    /// What was decoded.
    pub coverage: Coverage,
    /// Known limitations; an empty list claims there are none.
    pub limitations: Vec<String>,
    /// Byte ranges backing every value in `data`.
    pub evidence: Vec<Evidence>,
}

impl Outcome {
    /// The outcome of a handler that is wired but does nothing yet.
    pub fn not_implemented() -> Self {
        Self {
            data: Value::Null,
            coverage: Coverage::unknown(),
            limitations: vec![NOT_IMPLEMENTED.to_string()],
            evidence: Vec::new(),
        }
    }
}

/// The limitation string a stub handler reports.
pub const NOT_IMPLEMENTED: &str = "not implemented";

/// The serialized envelope: `{command, data, coverage, limitations, evidence}`.
#[derive(Debug, Serialize)]
pub(crate) struct Envelope<'a> {
    pub command: &'a str,
    #[serde(flatten)]
    pub outcome: Outcome,
}
