//! `place` (M7.E4, pipeline E): a reference of a base object placed into the Cell
//! View's cell at a typed position, previewed and confirmed per click.
//!
//! Step 0 of sprint 2026-11-02 declared this file and the Cell View's one call to
//! [`cell_view_bar`] (`windows/cell_view.rs`), whose answer the shell turns into
//! `Action::PlaceInCell`. The bar draws nothing until M7.E4.

use eframe::egui;

/// What the place bar asks for: place `base` into `cell` (formrefs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceAsk {
    pub base: String,
    pub cell: String,
}

/// The Cell View's place bar, drawn above the references of `cell`; `inspected` is the
/// shared selection (the base to place, once E decides how one is picked).
pub fn cell_view_bar(
    _ui: &mut egui::Ui,
    _cell: &str,
    _inspected: Option<&str>,
) -> Option<PlaceAsk> {
    None
}
