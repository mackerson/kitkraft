//! `new` and `clone` from the Object Window (M7.E3, pipeline E): a new form of the
//! selected type, or a copy of the selected record (Edit > Duplicate), previewed and
//! confirmed per click.
//!
//! Step 0 of sprint 2026-11-02 declared this file and the Object Window's one call to
//! [`object_window_actions`] (`windows/object_window.rs`). Nothing is drawn until M7.E3.

use eframe::egui;

use crate::panels::common::Action;

/// The Object Window's `new`/`clone` controls for `selected` (the record or type the
/// window has selected); returns the actions they ask for (`Action::OpenEditor`).
pub(crate) fn object_window_actions(_ui: &mut egui::Ui, _selected: Option<&str>) -> Vec<Action> {
    Vec::new()
}
