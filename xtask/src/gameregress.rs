//! `cargo xtask game-regress` (M4.T7, pipeline X): replays the recorded game sessions
//! and fails when a probe that passed before no longer does.
//!
//! Step 0 of sprint 2026-11-02 declared the task and its dispatch in `main.rs` so X fills
//! this file without touching the shared one. Not built yet: it prints so and exits 0.
//! `cargo xtask ci` does not call it.

use std::path::Path;

use anyhow::Result;

/// What the task prints until M4.T7 builds it.
pub(crate) const NOT_BUILT: &str = "game-regress: not built (M4.T7)";

/// `cargo xtask game-regress [ARGS...]`.
pub(crate) fn run(_root: &Path, _args: &[String]) -> Result<()> {
    println!("{NOT_BUILT}");
    Ok(())
}
