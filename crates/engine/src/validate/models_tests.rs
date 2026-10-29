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

/// An ARMA whose FemaleBipedModel (MOD3) names `model`.
fn arma(model: &[u8]) -> Record<'static> {
    record(
        b"ARMA",
        0x0001_0000,
        vec![sub(b"EDID", b"TestArma\0"), sub(b"MOD3", model)],
    )
}

/// An ARMO whose MaleWorldModel (MOD2) names `model`.
fn armo(model: &[u8]) -> Record<'static> {
    record(
        b"ARMO",
        0x0001_0000,
        vec![sub(b"EDID", b"TestArmo\0"), sub(b"MOD2", model)],
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

#[test]
fn an_arma_with_a_missing_female_biped_model_is_a_warning() {
    let d = dir("validate-model-arma-missing");
    let p = patch(
        &d,
        "MyMod.esp",
        &[],
        vec![arma(b"actors/creature/ghost.nif\0")],
    );
    let data = check(&p, &["asset.missing-model"]);
    assert_eq!(found(&data), vec![pair("asset.missing-model", "warning")]);
    // The finding names the member path, not just the subrecord (MOD3).
    let subrecord = data["findings"][0]["subrecord"].as_str().unwrap();
    assert!(
        subrecord.contains("FemaleBipedModel.FileName"),
        "{subrecord}"
    );
    let message = data["findings"][0]["message"].as_str().unwrap();
    assert!(message.contains("actors/creature/ghost.nif"), "{message}");
    assert!(message.contains("FemaleBipedModel.FileName"), "{message}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn an_armo_with_a_world_model_found_loose_is_clean() {
    let d = dir("validate-model-armo-loose");
    let p = patch(
        &d,
        "MyMod.esp",
        &[],
        vec![armo(b"actors/armor/plate.nif\0")],
    );
    // Lower-case on disk: names compare ASCII case-insensitively.
    let dir = d.join("meshes/actors/armor");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plate.nif"), b"fake nif").unwrap();
    let data = check(&p, &["asset.missing-model"]);
    assert_eq!(found(&data), Vec::<(String, String)>::new());
    let _ = std::fs::remove_dir_all(&d);
}
