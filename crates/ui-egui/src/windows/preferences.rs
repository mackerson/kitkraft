//! Preferences (M7.M3, pipeline W): the settings `docs/ck-parity.md` observes, kept in
//! `ui.json` only (`crate::prefs`).
//!
//! Step 0 of sprint 2026-11-02 declared this file, its state in `Shell` and its
//! View-menu window, so W fills it without touching those seams. Empty until M7.M3.

/// The Preferences window's state (kept in `crate::windows::Shell`).
#[derive(Default)]
pub struct PreferencesState {}
