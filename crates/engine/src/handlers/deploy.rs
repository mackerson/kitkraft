//! `deploy` (CLI `kit deploy --dev`), `deploy.rollback`, `deploy.status` (M4.E11,
//! ADR 0009), `deploy.enable` and `deploy.disable` (ADR 0009 Amendment 2): JSON arguments
//! in, the deployment report out. The work is in
//! [`crate::deploy`]; this file only maps arguments and errors.

use std::path::PathBuf;

use serde_json::Value;

use super::{failed, str_arg};
use crate::deploy::{
    self, Action, DeployError, DeployRequest, EnableRequest, Hooks, RollbackRequest, StatusRequest,
};
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

/// The processes whose presence refuses a profile edit: the Starfield game profile's
/// blocking executables (game, script extender, editor, MO2) and MO2's helpers.
fn blocking() -> Vec<String> {
    kitkraft_game::profile::GameProfile::named("starfield").map_or_else(
        |_| {
            [
                "Starfield.exe",
                "sfse_loader.exe",
                "CreationKit.exe",
                "ModOrganizer.exe",
            ]
            .map(str::to_string)
            .to_vec()
        },
        |g| {
            let mut v = g.executables.blocking();
            v.extend(g.executables.helpers.iter().cloned());
            v
        },
    )
}

/// `%LOCALAPPDATA%\kitkraft\profile-backups`, beside the index cache.
fn backup_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|d| !d.is_empty())
        .map(|d| PathBuf::from(d).join("kitkraft").join("profile-backups"))
}

fn enable_request(id: &str, action: Action, args: &Value) -> Result<EnableRequest, EngineError> {
    use kitkraft_game::host::Host;
    let mods = args
        .get("mods")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(EnableRequest {
        action,
        mo2: PathBuf::from(str_arg(id, args, "mo2")?),
        mods,
        profile: opt_str(args, "profile"),
        dry_run: flag(args, "dry_run"),
        running: kitkraft_game::host::SystemHost
            .running()
            .map_err(|e| e.to_string()),
        blocking: blocking(),
        backup_root: backup_root(),
    })
}

/// `deploy.enable`.
pub(super) fn enable(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "deploy.enable";
    require_dev(ID, args)?;
    let req = enable_request(ID, Action::Enable, args)?;
    deploy::enable(&req).map(outcome).map_err(|e| err(ID, &e))
}

/// `deploy.disable`.
pub(super) fn disable(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "deploy.disable";
    require_dev(ID, args)?;
    let req = enable_request(ID, Action::Disable, args)?;
    deploy::enable(&req).map(outcome).map_err(|e| err(ID, &e))
}
