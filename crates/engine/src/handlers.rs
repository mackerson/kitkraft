//! The default handler table: which catalog commands are wired, and to what.
//!
//! To wire a command to a format crate, write its handler here (or in a submodule) and
//! add it to [`defaults`]. Flip the catalog `status` to `Implemented` only when the
//! handler does the real work; front ends treat a `Planned` command's result as a stub.

use serde_json::Value;

use crate::envelope::Outcome;
use crate::exec::{CommandHandler, EngineError};

/// Handlers registered by [`crate::Engine::with_default_handlers`].
pub(crate) fn defaults() -> Vec<(&'static str, Box<dyn CommandHandler>)> {
    vec![("roundtrip", Box::new(roundtrip))]
}

/// `roundtrip`: stub until the `kitcraft-esm` streaming driver (M1.5) lands (M1.12).
fn roundtrip(_args: &Value) -> Result<Outcome, EngineError> {
    Ok(Outcome::not_implemented())
}
