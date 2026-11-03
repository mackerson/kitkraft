//! `build` (M7.E5, pipeline E; ADR 0007 Amendment 4, model A): a build spec (TOML v1 to
//! v3) compiled into the active file, its plan first, then one confirmed build.
//!
//! [`plan_request`] is `build --plan`: every change with before and after, the new forms
//! and the validation result, writing nothing. The editor's confirm box shows it; its
//! one click sends [`build_request`], the same arguments without `plan`. `build` writes
//! one fresh plugin and the window never sends `overwrite`, so the active file must not
//! exist yet. This file is the only one that builds a confirmed `build` (the gate's
//! per-entry source scan holds it). The editor draws the spec form (`super::editor`);
//! the Build Spec window (`WindowKind::BuildSpec`) is where the shell will show it.

use serde_json::{json, Value};

use crate::gate::Request;

/// The plan of building the spec at `spec` (a TOML file) into `out`, the active file: it
/// writes nothing, and the gate passes it like a read.
#[allow(dead_code)] // the editor's build form sends it from the next commit (M7.E5)
pub(crate) fn plan_request(spec: &str, out: &str) -> Request {
    Request::new("build", json!({"spec": spec, "out": out, "plan": true}))
}

/// The confirmed build of a planned spec: the plan's arguments with `plan` dropped,
/// built once per confirm click. `None` when `plan` is not a `build` plan.
#[allow(dead_code)] // the confirm box sends it from the next commit (M7.E5)
pub(crate) fn build_request(plan: &Request) -> Option<Request> {
    if plan.id != "build" || plan.args.get("plan") != Some(&Value::Bool(true)) {
        return None;
    }
    let mut args = plan.args.clone();
    args.as_object_mut()?.remove("plan");
    Some(Request::confirmed("build", args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::check;

    #[test]
    fn the_build_is_the_plan_without_plan_and_confirmed() {
        let plan = plan_request(r"C:\work\spec.toml", r"C:\out\Patch.esp");
        assert!(!plan.is_confirmed());
        assert!(check(&plan).is_ok(), "the plan passes like a read");
        let build = build_request(&plan).unwrap();
        assert!(build.is_confirmed());
        assert_eq!(build.id, "build");
        assert_eq!(build.args.get("plan"), None);
        assert_eq!(build.args["out"], json!(r"C:\out\Patch.esp"));
        assert!(check(&build).is_ok());
        assert!(build_request(&build).is_none());
        assert!(build_request(&Request::new("set", json!({"plan": true}))).is_none());
    }
}
