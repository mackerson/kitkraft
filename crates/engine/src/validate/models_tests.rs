//! `asset.missing-model` through `execute`: the model file missing, loose, and archived.

#![allow(clippy::arithmetic_side_effects)]

use kitkraft_esm::Record;

use super::tests::{check, found, pair, patch, record, sub};
use crate::edit::patch_tests::dir;

/// A STAT whose Model group names `model`.
fn stat(model: &[u8]) -> Record<'static> {
    record(
        b"STAT",
        0x0001_0000,
        vec![sub(b"EDID", b"TestStat\0"), sub(b"MODL", model)],
    )
}

#[test]
fn a_missing_model_file_is_a_warning() {
    let d = dir("validate-model-missing");
    let p = patch(
        &d,
        "MyMod.esp",
        &[],
        vec![stat(b"actors/creature/ghost.nif\0")],
    );
    let data = check(&p, &["asset.missing-model"]);
    assert_eq!(found(&data), vec![pair("asset.missing-model", "warning")]);
    let message = data["findings"][0]["message"].as_str().unwrap();
    assert!(message.contains("actors/creature/ghost.nif"), "{message}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_loose_model_file_next_to_the_plugin_is_found() {
    let d = dir("validate-model-loose");
    let p = patch(
        &d,
        "MyMod.esp",
        &[],
        vec![stat(b"Actors\\Creature\\Ghost.NIF\0")],
    );
    // Lower-case on disk: names compare ASCII case-insensitively, backslash as `/`.
    let dir = d.join("meshes/actors/creature");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ghost.nif"), b"fake nif").unwrap();
    let data = check(&p, &["asset.missing-model"]);
    assert_eq!(found(&data), Vec::<(String, String)>::new());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn an_archived_model_file_is_found() {
    let d = dir("validate-model-archive");
    let p = patch(
        &d,
        "MyMod.esp",
        &[],
        vec![stat(b"actors/creature/ghost.nif\0")],
    );
    let archive = d.join("MyMod - Main.ba2");
    let inputs = vec![(
        "meshes/actors/creature/ghost.nif".to_string(),
        "ghost.nif".to_string(),
    )];
    let mut out = std::fs::File::create(&archive).unwrap();
    kitkraft_ba2::pack(
        &inputs,
        |key| {
            if key == "ghost.nif" {
                Ok(b"fake nif".to_vec())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    key.to_string(),
                ))
            }
        },
        &kitkraft_ba2::PackOptions::default(),
        &mut out,
    )
    .unwrap();
    let data = check(&p, &["asset.missing-model"]);
    assert_eq!(found(&data), Vec::<(String, String)>::new());
    let _ = std::fs::remove_dir_all(&d);
}
