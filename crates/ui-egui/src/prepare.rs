//! What the window shows for one reply, built on the engine's worker thread (M6.P5a):
//! the envelope's common view and raw JSON lines ([`Prepared`]) and the panel's
//! view-model ([`View`]). The shell ([`crate::app`]) hands [`prepare`] to the worker with
//! each request, so a big envelope (13 MB of `conflicts` on `SFBGS00D.esm`) is turned into
//! rows off the UI thread and the frame that receives it only stores and draws them.
//!
//! The view-models are the panels' own (`crate::panels`), built by the same
//! `from_envelope` functions their tests use; nothing here reads the envelope itself.

use serde_json::Value;

use crate::envelope::Prepared;
use crate::gate::{Request, RunError};
use crate::panels::common::Slot;
use crate::panels::coverage::MatrixView;
use crate::panels::diag::DiagView;
use crate::panels::diff::DiffView;
use crate::panels::ledger::LedgerView;
use crate::panels::links::LinksView;
use crate::panels::load::LoadView;
use crate::panels::planet::PlanetView;
use crate::panels::records::{
    types_from_coverage, types_from_schema, QueryView, RecordView, TypeRow,
};
use crate::panels::schema::SchemaView;
use crate::panels::start::StartView;
use crate::panels::validate::{RoundtripView, ValidateView};
use crate::worker::Prepare;

/// A panel's view-model for one reply. `None` for a failed request, and for the slots
/// whose reply is joined into a view the window already holds (`InspectSchema`,
/// `DiagValidate`: small, done on the UI thread).
#[derive(Debug)]
pub(crate) enum View {
    None,
    Start(StartView),
    Load(LoadView),
    Types(Vec<TypeRow>),
    Query(QueryView),
    Record(RecordView),
    Links(LinksView),
    Planet(PlanetView),
    Diff(DiffView),
    Diag(DiagView),
    Validate(ValidateView),
    Roundtrip(RoundtripView),
    Schema(SchemaView),
    Ledger(LedgerView),
    Coverage(MatrixView),
}

/// Everything the window needs from one reply.
#[derive(Debug)]
pub(crate) struct Built {
    pub prepared: Prepared,
    pub view: View,
}

/// Builds the [`Built`] for `slot` from a reply. `load` is the session's load view at
/// submit time (the Diagnostics view resolves plugin names through it).
pub(crate) fn build(
    slot: Slot,
    request: &Request,
    result: &Result<Value, RunError>,
    load: Option<&LoadView>,
) -> Built {
    let (prepared, env) = match result {
        Ok(env) => (Prepared::new(Some(env), None), Some(env)),
        Err(err) => {
            let json = match err {
                RunError::Failed { json, .. } => Some(json),
                RunError::Refused(_) => None,
            };
            (Prepared::new(None, json), None)
        }
    };
    let view = match env {
        None => View::None,
        Some(env) => match slot {
            Slot::Profiles => View::Start(StartView::from_envelope(env)),
            Slot::Load => View::Load(LoadView::from_envelope(env)),
            Slot::Types => View::Types(if request.id == "coverage" {
                types_from_coverage(env)
            } else {
                types_from_schema(env)
            }),
            Slot::Query => View::Query(QueryView::from_envelope(env)),
            Slot::Inspect => View::Record(RecordView::from_envelope(env)),
            Slot::Links => View::Links(LinksView::from_envelope(env)),
            Slot::Planet => View::Planet(PlanetView::from_envelope(env)),
            Slot::Diff => View::Diff(DiffView::from_envelope(env)),
            Slot::DiagInspect => View::Diag(DiagView::from_inspect(env, load)),
            Slot::Validate => View::Validate(ValidateView::from_envelope(env)),
            Slot::Roundtrip => View::Roundtrip(RoundtripView::from_envelope(env)),
            Slot::Schema => View::Schema(SchemaView::from_envelope(env)),
            Slot::Ledger => View::Ledger(LedgerView::from_envelope(env)),
            Slot::Coverage => View::Coverage(MatrixView::from_envelope(env)),
            // Joined by the shell into state it holds (the cockpit's review, the
            // inspector's schema, the diagnostics' validate).
            Slot::InspectSchema
            | Slot::DiagValidate
            | Slot::CockpitStatus
            | Slot::CockpitValidate
            | Slot::CockpitRoundtrip
            | Slot::CockpitMasters
            | Slot::CockpitDeploy
            | Slot::CockpitRollback => View::None,
        },
    };
    Built { prepared, view }
}

/// The [`Prepare`] the shell sends with a request for `slot`.
pub(crate) fn prepare(slot: Slot, load: Option<LoadView>) -> Prepare<Built> {
    Box::new(move |request, result| {
        let built = build(slot, request, result, load.as_ref());
        // The shell reads the envelope itself only where no view was built (a reply it
        // joins into state it holds); otherwise the envelope is freed here.
        if !matches!(built.view, View::None) {
            if let Ok(env) = result {
                drop(std::mem::take(env));
            }
        }
        built
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_reply_is_built_into_its_panel_view_and_raw_lines() {
        let env = json!({"command": "schema", "data": {"summary": "1 type", "types": []},
            "coverage": {"status": "complete"}, "limitations": [], "evidence": []});
        let b = build(
            Slot::Types,
            &Request::new("schema", json!({})),
            &Ok(env),
            None,
        );
        assert!(matches!(b.view, View::Types(_)));
        assert!(b.prepared.view.is_some());
        assert!(!b.prepared.raw.is_empty());

        let failed = build(
            Slot::Diff,
            &Request::new("conflicts", json!({})),
            &Err(RunError::Refused("no".into())),
            None,
        );
        assert!(matches!(failed.view, View::None));
        assert!(failed.prepared.view.is_none());
    }
}
