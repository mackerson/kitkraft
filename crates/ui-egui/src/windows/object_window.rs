//! The Object Window (M7.W1, pipeline W): the category tree from `schema` (M7.S2), a
//! virtual list of one category's records, a filter box, and a double-click that opens
//! the record's dialog; read-only. Pipeline E's `new`/`clone` hooks (M7.E3) sit in it.
//!
//! Step 0 of sprint 2026-11-02 declared this file, its state in `Shell`, its replies
//! (`Pending::Object` in `shell.rs`, `Slot::ObjectCategories` and `Slot::ObjectList` in
//! `app.rs`) and its View-menu window, so W fills it without touching those seams. Every
//! body is a no-op until M7.W1.

use eframe::egui;
use serde_json::Value;

use crate::gate::Request;
use crate::panels::common::{Action, Slot};

/// What an Object Window request sent through the shell's tickets was for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectTicket {
    /// The category tree.
    Categories,
    /// One category's records.
    List,
}

/// The Object Window's state (kept in `crate::windows::Shell`).
#[derive(Default)]
pub struct ObjectWindowState {}

impl ObjectWindowState {
    /// Takes the reply to a request sent with `Pending::Object(ticket)`.
    pub fn receive(&mut self, _ticket: &ObjectTicket, _result: Result<&Value, String>) {}

    /// Takes a successful reply that came back on `Slot::ObjectCategories` or
    /// `Slot::ObjectList`.
    pub(crate) fn receive_slot(&mut self, _slot: Slot, _request: &Request, _env: &Value) {}

    /// Draws the window's content; returns the actions it asks for. Pipeline E's
    /// `new`/`clone` controls (M7.E3) come from its one hook here. The `allow` goes when
    /// W draws the window from `shell.rs` (M7.W1).
    #[allow(dead_code)]
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, selected: Option<&str>) -> Vec<Action> {
        crate::dialogs::new::object_window_actions(ui, selected)
    }
}
