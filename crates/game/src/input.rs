//! The record of input an agent sent to the game window (ADR 0010 Amendment 2).
//!
//! Amendment 2 lets a session send desktop input (keystrokes, the mouse, a virtual
//! controller) to the game window on one condition: input sent by an agent is recorded in
//! the session's report, what was sent and when, so a pass that depended on it says so.
//! Nothing sends input yet; this is the record, kept on [`crate::controller::Shared`]
//! ([`crate::controller::Shared::record_input`]) and written to `report.json` as `inputs`.

use serde::{Deserialize, Serialize};

/// One input an agent sent to the game window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SentInput {
    /// Milliseconds since the session started.
    pub at_ms: u64,
    /// The probe row it served, if any (`T-01`).
    pub row: Option<String>,
    /// `key`, `mouse`, `controller` or `focus`.
    pub kind: String,
    /// What was sent, in words: `F1`, `click 640,360`.
    pub detail: String,
}
