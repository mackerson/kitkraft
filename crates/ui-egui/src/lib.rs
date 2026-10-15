//! The KitKraft developer workbench (ADR 0007 Amendment 1, plan M4 lane W): a read-only
//! egui window over the engine's `execute`, an observability surface with no logic of its own.
//!
//! - [`gate`]: the only holder of the engine; an allowlist of non-writing commands and
//!   argument guards (`roundtrip` only with `verify`, never with `out`).
//! - [`worker`]: the engine on its own thread, one command at a time.
//! - [`panels`]: toolkit-free view-models (inputs to a request, envelope to rows), tested
//!   against canned envelopes without a window.
//! - [`cmdline`]: the `kit` line and MCP call behind every result, so the window teaches
//!   the CLI.
//! - [`prefs`]: the per-user UI prefs file (`%LOCALAPPDATA%\kitkraft\ui.json`), the only
//!   file the workbench writes on its own; UI state, never game data.
//! - [`run`]: opens the window (eframe, OpenGL backend).
//!
//! The crate depends on `kitkraft-engine` alone among workspace crates (xtask
//! `INTERNAL_ONLY`), so the UI toolkit can be swapped without touching anything below.
//!
//! LAYER: L5

mod app;
pub mod cmdline;
pub mod envelope;
pub mod gate;
pub mod panels;
pub mod prefs;
pub mod shots;
pub mod worker;

/// What the launcher (`kit-ui`, `kit ui`) passes in: the same load-order arguments as
/// `kit mcp`. Given a profile or plugins, the window starts with a `load`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    /// MO2 profile directory (read-only).
    pub profile: Option<String>,
    /// Plugins in load order (used when no profile is given).
    pub plugins: Vec<String>,
    /// Game Data dir.
    pub data_dir: Option<String>,
}

/// Why the window could not run.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// No usable system font (build with the `default_fonts` feature).
    #[error("no font: {0}")]
    NoFont(String),
    /// eframe could not open or run the window.
    #[error("window: {0}")]
    Window(String),
}

/// Opens the workbench window and blocks until it is closed.
pub fn run(opts: LaunchOptions) -> Result<(), UiError> {
    app::run_window(opts)
}
