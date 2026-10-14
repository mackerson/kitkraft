//! `deploy` (CLI `kit deploy --dev`), `deploy.rollback`, `deploy.status` (M4.E11,
//! ADR 0009): JSON arguments in, the deployment report out. The work is in
//! [`crate::deploy`]; this file only maps arguments and errors.

use std::path::PathBuf;

use serde_json::Value;

use super::{failed, str_arg};
use crate::deploy::{self, DeployError, DeployRequest, Hooks, RollbackRequest, StatusRequest};
use crate::envelope::{Coverage, CoverageStatus, Outcome};
use crate::exec::EngineError;

fn opt_str(args: &Value, name: &str) -> Option<String> {
    args.get(name).and_then(Value::as_str).map(str::to_string)
}

fn flag(args: &Value, name: &str) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(false)
}

/// `dev: true` is required on every writing call (ADR 0009: opt-in per invocation). The
/// schema only admits `true`; this is the second lock.
fn require_dev(command: &str, args: &Value) -> Result<(), EngineError> {
    if flag(args, "dev") {
        Ok(())
    } else {
        Err(failed(
            command,
            "refused [dev-flag-required]: pass --dev (`dev: true`); development deployment is opt-in on every call",
        ))
    }
}

fn outcome(report: deploy::Report) -> Outcome {
    Outcome {
        data: report.data,
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations: report.limitations,
        evidence: Vec::new(),
    }
}

fn err(command: &str, e: &DeployError) -> EngineError {
    failed(command, e.to_string())
}

/// `deploy`.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "deploy";
    require_dev(ID, args)?;
    let req = DeployRequest {
        source: PathBuf::from(str_arg(ID, args, "source")?),
        mo2: PathBuf::from(str_arg(ID, args, "mo2")?),
        profile: opt_str(args, "profile"),
        dry_run: flag(args, "dry_run"),
        hooks: Hooks::default(),
    };
    deploy::deploy(&req).map(outcome).map_err(|e| err(ID, &e))
}

/// `deploy.rollback`.
pub(super) fn rollback(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "deploy.rollback";
    require_dev(ID, args)?;
    let req = RollbackRequest {
        mo2: PathBuf::from(str_arg(ID, args, "mo2")?),
        mod_arg: str_arg(ID, args, "mod")?.to_string(),
        to: opt_str(args, "to"),
        profile: opt_str(args, "profile"),
        dry_run: flag(args, "dry_run"),
        hooks: Hooks::default(),
    };
    deploy::rollback(&req).map(outcome).map_err(|e| err(ID, &e))
}

/// `deploy.status`.
pub(super) fn status(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "deploy.status";
    let req = StatusRequest {
        mo2: PathBuf::from(str_arg(ID, args, "mo2")?),
        mod_arg: opt_str(args, "mod"),
        profile: opt_str(args, "profile"),
    };
    deploy::status(&req).map(outcome).map_err(|e| err(ID, &e))
}
