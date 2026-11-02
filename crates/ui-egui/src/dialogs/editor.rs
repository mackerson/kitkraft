//! The editor (pipeline E; ADR 0007 Amendment 4): the M7 writes into the active file
//! only (`set` M7.E2, `revert` M7.E6, `build` M7.E5, `new`/`clone` M7.E3, `place`
//! M7.E4), each previewed, validated and confirmed per OK click through the gate.
//!
//! Step 0 of sprint 2026-11-02 declared this file and wired it into the shared seams so
//! E fills it without touching them: its state in `Shell` (`editor`), the per-frame call
//! to [`ui`] in `shell.rs`, its tickets (`Pending::Editor`, forwarded to
//! [`EditorState::receive`]), its slots (`Slot::EditorPreview`, `EditorWrite`,
//! `EditorValidate`, routed to [`EditorState::receive_slot`] in `app.rs`), and the
//! actions `OpenEditor` and `PlaceInCell` (applied through [`EditorState::open`] and
//! [`EditorState::place`]). Every body is a no-op: nothing here writes until E's gate
//! commits permit it.

use eframe::egui;
use serde_json::Value;

use crate::gate::Request;
use crate::panels::common::{Action, Slot};

/// What an editor request sent through the shell's tickets was for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorTicket {
    /// A write's preview (dry run).
    Preview,
    /// A write's confirmed form.
    Write,
    /// The `validate` of a preview.
    Validate,
}

/// The editor's state (kept in `crate::windows::Shell`).
#[derive(Default)]
pub struct EditorState {}

impl EditorState {
    /// `Action::OpenEditor`: start editing `form` with `command` (a catalog id).
    pub fn open(&mut self, _command: &str, _form: &str) {}

    /// `Action::PlaceInCell`: start placing `base` into `cell` (formrefs).
    pub fn place(&mut self, _base: &str, _cell: &str) {}

    /// Takes the reply to a request sent with `Pending::Editor(ticket)`.
    pub fn receive(&mut self, _ticket: &EditorTicket, _result: Result<&Value, String>) {}

    /// Takes a successful reply that came back on `Slot::EditorPreview`,
    /// `Slot::EditorWrite` or `Slot::EditorValidate`.
    pub(crate) fn receive_slot(&mut self, _slot: Slot, _request: &Request, _env: &Value) {}
}

/// The editor's per-frame hook, called by the shell after the CK-style windows: draws
/// what the editor has open and returns the actions it asks for (its requests go out as
/// `Action::Submit` on the editor slots). `active_file` is the Data dialog's choice, the
/// one file the writes may go into; `selected` the inspected record.
pub(crate) fn ui(
    _ctx: &egui::Context,
    _state: &mut EditorState,
    _busy: bool,
    _active_file: Option<&str>,
    _selected: Option<&str>,
) -> Vec<Action> {
    Vec::new()
}
