//! `mesh.bounds` and `mesh.geometry` through `execute` with JSON (step 0 of sprint
//! 2026-11-02, prepares M7.S4 and M7.S5): the bounds stub answers `bounds: null` with its
//! limitation and reads nothing; geometry is refused as deferred; neither writes.

use serde_json::json;

use super::mesh::NOT_BUILT;
use crate::{execute, find_command, Status};

#[test]
fn mesh_bounds_is_a_read_only_stub_that_says_it_is_not_built() {
    let env = execute("mesh.bounds", json!({"model": "Meshes\\Test\\Crate.nif"})).unwrap();
    assert_eq!(env["command"], "mesh.bounds");
    assert_eq!(env["data"]["bounds"], serde_json::Value::Null);
    assert_eq!(env["data"]["model"], "Meshes\\Test\\Crate.nif");
    assert_eq!(env["coverage"]["status"], "unknown");
    let limitations = env["limitations"].as_array().unwrap();
    assert!(
        limitations
            .iter()
            .any(|l| l.as_str().unwrap_or_default().contains(NOT_BUILT)),
        "{limitations:?}"
    );
    assert_eq!(env["evidence"], json!([]));
    let spec = find_command("mesh.bounds").unwrap();
    assert!(!spec.writes);
    assert_eq!(spec.status, Status::Implemented);
    assert!(spec.menu_path.starts_with("Tools/"), "{}", spec.menu_path);
}

#[test]
fn mesh_bounds_refuses_arguments_it_does_not_know() {
    let err = execute("mesh.bounds", json!({"out": "x.json"})).unwrap_err();
    assert_eq!(err.kind(), "invalid_args");
}

#[test]
fn mesh_geometry_is_deferred_to_m7_s5_and_refused() {
    let spec = find_command("mesh.geometry").unwrap();
    assert!(!spec.writes);
    assert!(
        matches!(spec.status, Status::Deferred { milestone, .. } if milestone == "M7.S5"),
        "{:?}",
        spec.status
    );
    let err = execute("mesh.geometry", json!({"model": "Meshes\\Test\\Crate.nif"})).unwrap_err();
    assert_eq!(err.kind(), "deferred");
}
