//! Agent-driven game sessions on the dev profile (ADR 0010, Amendment 1; lane C, M4.T3 and
//! M5.C1): the host-side work of `kit game session`.
//!
//! - [`profile`]: per-game profiles (`profiles/<game>.toml`): executables, the MO2
//!   command line, the console endpoint, save commands, INI switches, logs, timeouts, and
//!   which of those facts still lack evidence.
//! - [`preflight`]: the refusals of ADR 0010 section 1, as a pure function of gathered
//!   facts (the engine reads MO2 through `kitkraft-io` and fills them in).
//! - [`console`]: the loopback HTTP console client (std TCP).
//! - [`host`]: processes and screenshots (std process and Windows' own tools).
//! - [`probes`]: the `probes.toml` format `kit testplan` writes, and the verdicts.
//! - [`authorized`], [`backup`]: the write guard, backups and restore-only rollback.
//! - [`ledger`]: the entry appended to `docs/game-observations.md`.
//! - [`controller`]: one session from backup to ledger, synchronous, followed through
//!   [`controller::Shared`].
//! - [`testing`]: a fake `/console` endpoint and a fake host, for tests here and in the
//!   engine.
//!
//! Clean room (ADR 0001, ADR 0008): no game offsets, signatures or framework headers; the
//! controller knows a port, paths and command strings. The third-party console plugins are
//! run-only tools, never read. Docs: `docs/game-sessions.md`.
//!
//! LAYER: L3

pub mod authorized;
pub mod backup;
pub mod console;
pub mod controller;
mod error;
pub mod host;
pub mod ledger;
pub mod preflight;
pub mod probes;
pub mod profile;
pub mod testing;

pub use error::{GameError, Result};
pub use profile::GameProfile;
