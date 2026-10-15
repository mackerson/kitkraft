//! `package` through `execute`, on synthetic plugins, loose files and scripts.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{sha256, MANIFEST};
use crate::execute;
use crate::handlers::papyrus::tests::tiny_pex;
use crate::handlers::testdata::{self, group, record, sub};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kitkraft-package-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A 4x4 R8G8B8A8 texture with a DX10 header (`ms-docs:` DDS_HEADER, DDS_HEADER_DXT10).
fn dds() -> Vec<u8> {
    let mut b = vec![0u8; 148];
    b[..4].copy_from_slice(b"DDS ");
    let put = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut b, 4, 124);
    put(&mut b, 8, 0x1007);
    put(&mut b, 12, 4);
    put(&mut b, 16, 4);
    put(&mut b, 28, 1);
    put(&mut b, 76, 32);
    put(&mut b, 80, 4);
    b[84..88].copy_from_slice(b"DX10");
    put(&mut b, 108, 0x1000);
    put(&mut b, 128, 28);
    put(&mut b, 132, 3);
    put(&mut b, 140, 1);
    b.extend((0..64u8).map(|i| i.wrapping_mul(7)));
    b
}

/// The plugin, a loose folder (a mesh and a texture) and a compiled script, under `dir`.
fn inputs(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let plugin = dir.join("src").join("KC_Test.esm");
    fs::create_dir_all(plugin.parent().unwrap()).unwrap();
    fs::write(&plugin, testdata::plugin()).unwrap();
    let loose = dir.join("loose");
    fs::create_dir_all(loose.join("Meshes").join("KC")).unwrap();
    fs::write(
        loose.join("Meshes").join("KC").join("Thing.nif"),
        b"not really a nif",
    )
    .unwrap();
    fs::create_dir_all(loose.join("Textures").join("KC")).unwrap();
    fs::write(loose.join("Textures").join("KC").join("Thing.dds"), dds()).unwrap();
    let pex = dir.join("src").join("tiny.pex");
    fs::write(&pex, tiny_pex()).unwrap();
    (plugin, loose, pex)
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// Every file under `root`, as sorted `(relative path, bytes)`.
fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// Checks every manifest entry against the files of `folder` (a package, or a copy of one).
fn verify(folder: &Path) -> Value {
    let manifest: Value =
        serde_json::from_slice(&fs::read(folder.join(MANIFEST)).unwrap()).unwrap();
    let files = tree(folder);
    let listed: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    let on_disk: Vec<&str> = files
        .iter()
        .map(|(rel, _)| rel.as_str())
        .filter(|r| *r != MANIFEST)
        .collect();
    assert_eq!(listed, on_disk);
    for f in manifest["files"].as_array().unwrap() {
        let (_, bytes) = files
            .iter()
            .find(|(r, _)| r == f["path"].as_str().unwrap())
            .unwrap();
        assert_eq!(f["size"], bytes.len() as u64);
        assert_eq!(f["sha256"], sha256::hex(bytes));
    }
    manifest
}

#[test]
fn loose_package_with_scripts_manifest_and_zip_is_deterministic() {
    let dir = temp("loose");
    let (plugin, loose, pex) = inputs(&dir);
    let spec = dir.join("src").join("poi.toml");
    fs::write(&spec, b"version = 3\n").unwrap();
    let run = |out: &Path| {
        execute(
            "package",
            json!({"plugin": s(&plugin), "out": s(out), "loose": s(&loose),
                   "scripts": [s(&pex)], "version": "1.0.0", "spec": s(&spec)}),
        )
        .unwrap()
    };
    let r = run(&dir.join("out1"));
    let folder = dir.join("out1").join("KC_Test");
    assert_eq!(r["data"]["folder"], s(&folder));
    let manifest = verify(&folder);
    let paths: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "KC_Test.esm",
            "Meshes/KC/Thing.nif",
            "Scripts/Tiny.pex",
            "Textures/KC/Thing.dds"
        ]
    );
    let p = &manifest["plugins"][0];
    assert_eq!(p["file"], "KC_Test.esm");
    assert_eq!(p["masters"], json!([]));
    assert_eq!(p["records"], testdata::RECORDS - 1);
    assert_eq!(p["record_types"], json!({"GMST": 1, "WEAP": 2}));
    assert_eq!(p["sha256"], sha256::hex(&testdata::plugin()));
    assert_eq!(manifest["kitkraft_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["spec"]["sha256"], sha256::hex(b"version = 3\n"));
    assert_eq!(manifest["spec"]["file"], "poi.toml");
    assert_eq!(manifest["validate"]["passed"], true);
    assert_eq!(manifest["archives"], json!([]));
    assert_eq!(manifest["version"], "1.0.0");
    // No absolute path or time in the manifest.
    let text = fs::read_to_string(folder.join(MANIFEST)).unwrap();
    assert!(!text.contains(s(&dir)) && !text.contains(&s(&dir).replace('\\', "\\\\")));

    // The zip holds the folder's files at its root, the manifest included.
    let zip_path = dir.join("out1").join("KC_Test-1.0.0.zip");
    assert_eq!(r["data"]["zip"]["path"], s(&zip_path));
    let mut zip = zip::ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
    let mut names = Vec::new();
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).unwrap();
        let name = e.name().unwrap().to_string();
        let mut bytes = Vec::new();
        e.read_to_end(&mut bytes).unwrap();
        assert_eq!(fs::read(folder.join(&name)).unwrap(), bytes, "{name}");
        names.push(name);
    }
    let mut want: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
    want.push(MANIFEST.into());
    want.sort();
    assert_eq!(names, want);

    // A second run elsewhere is byte-identical, folder and zip.
    run(&dir.join("out2"));
    assert_eq!(tree(&folder), tree(&dir.join("out2").join("KC_Test")));
    assert_eq!(
        fs::read(&zip_path).unwrap(),
        fs::read(dir.join("out2").join("KC_Test-1.0.0.zip")).unwrap()
    );

    // The manifest verifies after a copy (what the owner does when installing).
    let copy = dir.join("copy");
    for (rel, bytes) in tree(&folder) {
        let p = copy.join(&rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
    verify(&copy);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn ba2_package_names_archives_after_the_plugin() {
    let dir = temp("ba2");
    let (plugin, loose, pex) = inputs(&dir);
    let out = dir.join("out");
    let r = execute(
        "package",
        json!({"plugin": s(&plugin), "out": s(&out), "loose": s(&loose),
               "scripts": [s(&pex)], "ba2": true, "zip": false}),
    )
    .unwrap();
    assert_eq!(r["data"]["zip"], Value::Null);
    assert!(!out.join("KC_Test.zip").exists());
    let folder = out.join("KC_Test");
    let manifest = verify(&folder);
    let paths: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "KC_Test - Main.ba2",
            "KC_Test - Textures.ba2",
            "KC_Test.esm"
        ]
    );
    let archives = manifest["archives"].as_array().unwrap();
    assert_eq!(archives[0]["file"], "KC_Test - Main.ba2");
    assert_eq!(archives[0]["format"], "gnrl");
    let inner: Vec<&str> = archives[0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(inner, ["meshes/kc/thing.nif", "scripts/tiny.pex"]);
    // Fable review (deploy round), finding 11: the hash is the packed file's source, not
    // the archive entry's (compressed) bytes, and the field says so.
    assert_eq!(
        archives[0]["files"][1]["source_sha256"],
        sha256::hex(&tiny_pex())
    );
    assert!(archives[0]["files"][1].get("sha256").is_none());
    assert_eq!(archives[1]["format"], "dx10");
    // The archives read back through ba2.list.
    let listed = execute(
        "ba2.list",
        json!({"archive": s(&folder.join("KC_Test - Main.ba2"))}),
    )
    .unwrap();
    assert_eq!(listed["data"]["file_count"], 2);
    let listed = execute(
        "ba2.list",
        json!({"archive": s(&folder.join("KC_Test - Textures.ba2"))}),
    )
    .unwrap();
    assert_eq!(
        listed["data"]["entries"][0]["name"],
        "textures/kc/thing.dds"
    );
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("Archive names")));
    fs::remove_dir_all(&dir).unwrap();
}

/// A plugin that names a master no one has fails `validate`: refused, nothing written.
#[test]
fn failing_plugin_is_refused_before_anything_is_written() {
    let dir = temp("invalid");
    let mut hedr = 1.0f32.to_le_bytes().to_vec();
    hedr.extend_from_slice(&1u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let mut tes4 = sub(b"HEDR", &hedr);
    tes4.extend(sub(b"MAST", b"KC_Missing.esm\0"));
    tes4.extend(sub(b"DATA", &0u64.to_le_bytes()));
    let mut bytes = record(b"TES4", 1, 0, &tes4);
    let mut gmst = sub(b"EDID", b"fKcValue\0");
    gmst.extend(sub(b"DATA", &2.5f32.to_le_bytes()));
    bytes.extend(group(b"GMST", 0, &record(b"GMST", 0, 0x0100_0800, &gmst)));
    let plugin = dir.join("KC_Bad.esp");
    fs::write(&plugin, bytes).unwrap();
    let out = dir.join("out");
    let err = execute("package", json!({"plugin": s(&plugin), "out": s(&out)}))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("fails validate") && err.contains("KC_Missing.esm"),
        "{err}"
    );
    assert!(!out.exists(), "nothing may be written");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn out_and_replacement_guards() {
    let dir = temp("guards");
    let (plugin, loose, _) = inputs(&dir);
    let p = s(&plugin);

    // Inside a Mod Organizer instance (its mods folder included): refused.
    let mo2 = dir.join("MO2");
    fs::create_dir_all(mo2.join("mods")).unwrap();
    fs::write(mo2.join("ModOrganizer.ini"), b"[General]\n").unwrap();
    let err = execute("package", json!({"plugin": p, "out": s(&mo2.join("mods"))}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("Mod Organizer"), "{err}");
    assert!(fs::read_dir(mo2.join("mods")).unwrap().next().is_none());

    // A game Data folder: refused.
    let data = dir.join("Game").join("Data");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("Starfield.esm"), b"").unwrap();
    let err = execute("package", json!({"plugin": p, "out": s(&data)}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("Data"), "{err}");

    // An existing folder package did not make is never replaced.
    let out = dir.join("out");
    fs::create_dir_all(out.join("KC_Test")).unwrap();
    fs::write(out.join("KC_Test").join("keep.txt"), b"mine").unwrap();
    let err = execute(
        "package",
        json!({"plugin": p, "out": s(&out), "overwrite": true}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("only replaces folders it made"), "{err}");
    assert!(out.join("KC_Test").join("keep.txt").is_file());
    fs::remove_dir_all(out.join("KC_Test")).unwrap();

    // Its own folder: refused without overwrite, replaced with it.
    execute(
        "package",
        json!({"plugin": p, "out": s(&out), "name": "Mod"}),
    )
    .unwrap();
    let err = execute(
        "package",
        json!({"plugin": p, "out": s(&out), "name": "Mod"}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("exists"), "{err}");
    let r = execute(
        "package",
        json!({"plugin": p, "out": s(&out), "name": "Mod", "overwrite": true, "loose": s(&loose)}),
    )
    .unwrap();
    assert_eq!(r["data"]["files"], 4);
    verify(&out.join("Mod"));
    // Nothing but the folder and the zip is left in out (no partial folders).
    let mut left: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["Mod", "Mod.zip"]);

    // Bad names and inputs.
    for (args, needle) in [
        (
            json!({"plugin": p, "out": s(&out), "name": "a/b"}),
            "folder name",
        ),
        (
            json!({"plugin": p, "out": s(&out), "version": "1 0"}),
            "version",
        ),
        (
            json!({"plugin": s(&dir.join("none.esm")), "out": s(&out)}),
            "does not exist",
        ),
        (
            json!({"plugin": s(&loose.join("Meshes").join("KC").join("Thing.nif")), "out": s(&out)}),
            "not a .esm",
        ),
        (
            json!({"plugin": p, "out": s(&out), "scripts": [s(&plugin)]}),
            "not a readable .pex",
        ),
    ] {
        let err = execute("package", args).unwrap_err().to_string();
        assert!(err.contains(needle), "{needle}: {err}");
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// Fable review (deploy round), finding 2: an `out` through a junction into MO2's `mods\`,
/// followed by folders that do not exist yet, used to keep its unresolved spelling and
/// pass the guard. Refused; nothing lands in `mods\`.
#[test]
fn out_through_a_junction_is_refused() {
    let dir = temp("junction");
    let (plugin, _, _) = inputs(&dir);
    let Some((link, mods)) = crate::edit::patch_tests::junction_into_mods(&dir) else {
        return;
    };
    let out = link.join("new").join("deeper");
    let err = execute("package", json!({"plugin": s(&plugin), "out": s(&out)}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("junction"), "{err}");
    assert!(fs::read_dir(&mods).unwrap().next().is_none());
    fs::remove_dir(&link).unwrap();
    fs::remove_dir_all(&dir).unwrap();
}

/// Fable review (deploy round), finding 11: `package` allowed 100,000 files and
/// `deploy --dev` reads 20,000, so a package could be undeployable. One limit, 20,000.
#[test]
fn file_limit_matches_deploy() {
    assert_eq!(super::MAX_FILES, 20_000);
    assert_eq!(super::MAX_FILES, crate::deploy::MAX_PACKAGE_FILES);
    let dir = temp("limit");
    fs::write(dir.join("one.nif"), b"x").unwrap();
    let mut out: Vec<(Vec<String>, PathBuf)> = (0..super::MAX_FILES)
        .map(|i| (vec![format!("f{i}")], dir.join(format!("f{i}"))))
        .collect();
    let err = super::walk(&dir, &dir, 0, &mut out).unwrap_err();
    assert!(err.contains("more than 20000 files"), "{err}");
    fs::remove_dir_all(&dir).unwrap();
}

/// Fable review (deploy round), finding 11: `overwrite` removed the old folder before
/// renaming the new one in, so a file it could not delete left it half removed. The old
/// folder is now renamed aside first: when that fails, nothing has changed.
#[cfg(windows)]
#[test]
fn overwrite_never_leaves_a_half_removed_folder() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = temp("atomic");
    let (plugin, loose, _) = inputs(&dir);
    let out = dir.join("out");
    execute(
        "package",
        json!({"plugin": s(&plugin), "out": s(&out), "loose": s(&loose)}),
    )
    .unwrap();
    let folder = out.join("KC_Test");
    let before = tree(&folder);
    // Another program holds one file open without sharing (the CK, an archive tool).
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(folder.join("Meshes").join("KC").join("Thing.nif"))
        .unwrap();
    let err = execute(
        "package",
        json!({"plugin": s(&plugin), "out": s(&out), "overwrite": true}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("nothing was changed"), "{err}");
    drop(held);
    assert_eq!(tree(&folder), before, "the old folder is intact");
    let mut left: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["KC_Test", "KC_Test.zip"]);
    // Released, the overwrite replaces it and leaves no aside copy behind.
    let r = execute(
        "package",
        json!({"plugin": s(&plugin), "out": s(&out), "overwrite": true}),
    )
    .unwrap();
    assert_eq!(r["data"]["files"], 2);
    verify(&folder);
    let mut left: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["KC_Test", "KC_Test.zip"]);
    fs::remove_dir_all(&dir).unwrap();
}

/// The CLI form `kit package <plugin> <out> --ba2`.
#[test]
fn command_line_form() {
    let crate::Invocation::Run { command, args } =
        crate::parse_line("package KC_Test.esm out --ba2 --loose files").unwrap()
    else {
        panic!("expected Run");
    };
    assert_eq!(command.id, "package");
    assert_eq!(
        args,
        json!({"plugin": "KC_Test.esm", "out": "out", "ba2": true, "loose": "files"})
    );
}
