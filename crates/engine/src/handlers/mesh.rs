//! `mesh.bounds` (M7.S4, lane R): a model's bounds from its NIF, found from a base
//! object's model or given as a path, read from loose files or a BA2.
//!
//! Step 0 of sprint 2026-11-02 registered the command so the catalog, the handler table,
//! the workbench gate and the menu agree on it before the reader exists. Until M7.S4 lands
//! the handler reads nothing: `bounds` is `null` and a limitation says it is not built yet,
//! so a caller never mistakes the stub for a model without a bound. `mesh.geometry`
//! (M7.S5) stays `Deferred` in the catalog and has no handler.

use serde_json::{json, Value};

use crate::exec::EngineError;
use crate::{Coverage, Outcome, Session};

/// The limitation every `mesh.bounds` result carries until M7.S4 reads the NIF.
pub(crate) const NOT_BUILT: &str = "not built yet (M7.S4)";

/// `mesh.bounds`: `bounds: null` and [`NOT_BUILT`]; reads no file and leaves the session
/// alone.
pub(super) fn bounds(_session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let model = args.get("model").and_then(Value::as_str);
    let form = args.get("form").and_then(Value::as_str);
    Ok(Outcome {
        data: json!({
            "model": model,
            "form": form,
            "bounds": null,
            "summary": format!("mesh.bounds: {NOT_BUILT}; no model was read"),
        }),
        coverage: Coverage::unknown(),
        limitations: vec![format!(
            "mesh.bounds is {NOT_BUILT}: bounds is null because no NIF was read, not because the model has no bound"
        )],
        evidence: Vec::new(),
    })
}
