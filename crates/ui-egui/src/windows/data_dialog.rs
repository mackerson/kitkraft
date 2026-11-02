//! The Data dialog (M7.M2, pipeline W): choose the masters and the active file over
//! `load` and `inspect` of each plugin's `TES4`, as `docs/ck-parity.md` observes it; an
//! official master is never the active file. "Set as Active File" sends
//! `Action::SetActiveFile`, the file pipeline E's writes go into.
//!
//! Step 0 of sprint 2026-11-02 declared this file, its state in `Shell`, its replies
//! (`Pending::Data` in `shell.rs`, `Slot::DataLoad` and `Slot::DataDetails` in `app.rs`)
//! and its View-menu window, so W fills it without touching those seams. Every body is a
//! no-op until M7.M2.

use serde_json::Value;

use crate::gate::Request;
use crate::panels::common::Slot;

/// What a Data dialog request sent through the shell's tickets was for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataTicket {
    /// The load order the dialog lists.
    Load,
    /// One plugin's details (its `TES4`).
    Details,
}

/// The Data dialog's state (kept in `crate::windows::Shell`).
#[derive(Default)]
pub struct DataDialogState {}

impl DataDialogState {
    /// Takes the reply to a request sent with `Pending::Data(ticket)`.
    pub fn receive(&mut self, _ticket: &DataTicket, _result: Result<&Value, String>) {}

    /// Takes a successful reply that came back on `Slot::DataLoad` or
    /// `Slot::DataDetails`.
    pub(crate) fn receive_slot(&mut self, _slot: Slot, _request: &Request, _env: &Value) {}
}
