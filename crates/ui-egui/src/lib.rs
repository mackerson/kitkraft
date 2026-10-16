//! The KitKraft developer workbench (ADR 0007 Amendment 1, plan M4 lane W): a read-only
//! egui window over the engine's `execute`, an observability surface with no logic of its own.
//!
//! - [`gate`]: the only holder of the engine; an allowlist of non-writing commands and
//!   argument guards (`roundtrip` only with `verify`, never with `out`).
//! - [`worker`]: the engine on its own thread, one command at a time.
//! - [`panels`]: one file per tab: its toolkit-free view-model (inputs to a request,
//!   envelope to rows), tested against canned envelopes without a window, and its
//!   drawing (`fn ui`).
//! - [`cmdline`]: the `kit` line and MCP call behind every result, so the window teaches
//!   the CLI.
//! - [`md`]: plain Markdown for the Help tab, which shows the owner's QA material from the
//!   repo's docs at runtime ([`panels::help`]).
//! - `prepare`: each reply's view-model and raw JSON lines, built on the worker thread
//!   (M6.P5a), so the frame that receives a big envelope only draws it.
//! - [`prefs`]: the per-user UI prefs file (`%LOCALAPPDATA%\kitkraft\ui.json`), the only
//!   file the workbench writes on its own; UI state, never game data.
//! - [`theme`]: the visual tokens (type, spacing, sizes, radii, density, light and dark
//!   palettes) and the one function that applies them to egui; `workbench` by default,
//!   other presets by `--theme`, `ui.json` or the Start tab's picker.
//! - [`perf`]: the `KITKRAFT_WORKBENCH_PERF=1` trace (stderr `perf:` lines) and the
//!   window bench of `docs/design/workbench-performance.md` (M6.P1).
//! - [`run`]: opens the window (eframe, OpenGL backend).
//! - [`viewport`]: the 3D viewport of the Render Window tab (M7 lane R), drawn with GL
//!   through an egui paint callback.
//! - [`menu`], [`windows`], [`dialogs`]: the CK-style shell (M7 lane M): the menu bar from
//!   the catalog, floating windows (Warnings, Use Info, any tab), and the read-only
//!   edit dialog with its per-type layouts (`layouts/<TYPE>.toml`).
//!
//! The crate depends on `kitkraft-engine` alone among workspace crates (xtask
//! `INTERNAL_ONLY`), so the UI toolkit can be swapped without touching anything below.
//!
//! LAYER: L5

mod app;
pub mod cmdline;
pub mod dialogs;
pub mod envelope;
pub mod gate;
pub mod md;
pub mod menu;
pub mod panels;
pub mod perf;
pub mod prefs;
mod prepare;
pub mod shots;
pub mod theme;
pub mod viewport;
pub mod windows;
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
    /// `--theme`: the visual preset and mode; `None` reads `theme` from `ui.json`, else
    /// the default (`workbench`, following the system's light or dark setting).
    pub theme: Option<theme::ThemeChoice>,
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
