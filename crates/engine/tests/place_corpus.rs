//! M4.E2 on the official files (`KITKRAFT_DATA_DIR`, release, ignored): `place` of a
//! reference identical to an existing one reproduces its bytes. Lane A's composition test
//! (`crates/esm/tests/refs.rs`) extended through the command: references of
//! `Starfield.esm` that carry only `NAME` and `DATA`, in interior cells and in exterior
//! cells of a world, are placed again from their base, cell (or world and position),
//! position and rotation into a fresh plugin, and the written record's body equals the
//! original's byte for byte. The header's FormID is a new one and its stamps (form version,
//! timestamp, version control, unknown) are the composition defaults (`RecordStamps`), so
//! the header is compared field by field: signature, data size and flags equal.
//!
//! `cargo test -p kitkraft-engine --release --test place_corpus -- --ignored`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use kitkraft_engine::Engine;
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions, PluginIndex, RecordLoc};
use serde_json::json;

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

fn bytes_at(path: &Path, offset: u64, len: u64) -> Vec<u8> {
    let mut f = File::open(path).unwrap();
    f.seek(SeekFrom::Start(offset)).unwrap();
    let mut buf = vec![0u8; usize::try_from(len).unwrap()];
    f.read_exact(&mut buf).unwrap();
    buf
}

/// A reference to place again: `(loc, base, cell or world, interior)`.
struct Candidate<'a> {
    loc: &'a RecordLoc,
    base: u32,
    cell: u32,
    world: Option<u32>,
    position: [f32; 3],
    rotation: [f32; 3],
}

fn candidates<'a>(
    index: &'a PluginIndex,
    path: &Path,
    want_world: bool,
    n: usize,
) -> Vec<Candidate<'a>> {
    let mut file = File::open(path).unwrap();
    let mut out = Vec::new();
    let mut seen = 0usize;
    for loc in index.records.iter().filter(|r| {
        r.signature.0 == *b"REFR" && r.flags == 0 && r.form_id >> 24 == 0 && r.edid.is_none()
    }) {
        let groups = index.group_path(loc.group);
        let types: Vec<i32> = groups.iter().map(|g| g.header.group_type).collect();
        let world = if want_world {
            if types != [0, 1, 4, 5, 6, 9] {
                continue;
            }
            Some(groups[1].header.label_u32())
        } else {
            if types != [0, 2, 3, 6, 9] {
                continue;
            }
            None
        };
        seen += 1;
        // A spread sample: every 997th candidate.
        if seen % 997 != 1 {
            continue;
        }
        let record = read_indexed(&mut file, loc, None).unwrap();
        let Some(subs) = record.subrecords() else {
            continue;
        };
        let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
        if sigs != [*b"NAME", *b"DATA"] {
            continue;
        }
        let base = u32::from_le_bytes(subs[0].data().try_into().unwrap());
        if base >> 24 != 0 || subs[1].data().len() != 24 {
            continue;
        }
        let f = |i: usize| f32::from_le_bytes(subs[1].data()[i * 4..i * 4 + 4].try_into().unwrap());
        let position = [f(0), f(1), f(2)];
        let rotation = [f(3), f(4), f(5)];
        if !position.iter().chain(&rotation).all(|v| v.is_finite()) {
            continue;
        }
        if let Some(_w) = world {
            // Only references inside their cell's square (99.98 %): `world` picks the cell.
            let cx = (position[0] / 100.0).floor() as i32;
            let cy = (position[1] / 100.0).floor() as i32;
            let cell_label = groups[3].header.label; // the sub-block
            let sub_y = i16::from_le_bytes([cell_label[0], cell_label[1]]);
            let sub_x = i16::from_le_bytes([cell_label[2], cell_label[3]]);
            if cx.div_euclid(8) != i32::from(sub_x) || cy.div_euclid(8) != i32::from(sub_y) {
                continue;
            }
        }
        out.push(Candidate {
            loc,
            base,
            cell: groups[groups.len() - 1].header.label_u32(),
            world,
            position,
            rotation,
        });
        if out.len() == n {
            break;
        }
    }
    out
}

#[test]
#[ignore = "release: indexes Starfield.esm (cached after the first run)"]
fn placing_a_copy_of_a_corpus_reference_reproduces_its_body() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let esm = dir.join("Starfield.esm");
    let index = index_file(&esm, &IndexOptions::default()).unwrap();
    let engine = Engine::with_default_handlers();
    engine
        .execute(
            "load",
            json!({"plugins": ["Starfield.esm"], "data_dir": dir.display().to_string()}),
        )
        .unwrap();
    let out = std::env::temp_dir().join(format!("kitkraft-place-corpus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let mut checked = (0usize, 0usize);
    for want_world in [false, true] {
        let list = candidates(&index, &esm, want_world, 8);
        assert!(list.len() >= 4, "too few candidates (world: {want_world})");
        for (i, c) in list.iter().enumerate() {
            let into = out.join(format!("P{}_{i}.esm", u8::from(want_world)));
            let mut args = json!({
                "base": format!("Starfield.esm:{:06X}", c.base),
                "position": c.position, "rotation": c.rotation,
                "into": into.display().to_string(),
            });
            match c.world {
                Some(w) => args["world"] = json!(format!("Starfield.esm:{w:06X}")),
                None => args["cell"] = json!(format!("Starfield.esm:{:06X}", c.cell)),
            }
            let r = engine.execute("place", args.clone()).unwrap_or_else(|e| {
                panic!("place {args}: {e}");
            });
            let placed = &r["data"]["placed"][0];
            assert_eq!(
                placed["stored_cell"],
                json!(format!("{:08X}", c.cell)),
                "{args}"
            );
            assert_eq!(
                r["data"]["validate"]["passed"],
                json!(true),
                "{:#}",
                r["data"]["validate"]
            );
            let original = bytes_at(&esm, c.loc.offset, c.loc.len());
            let offset = placed["offset"].as_u64().unwrap();
            let length = placed["length"].as_u64().unwrap();
            let written = bytes_at(&into, offset, length);
            assert_eq!(written.len(), original.len(), "{args}");
            // Signature, data size and flags; then the body.
            assert_eq!(written[..12], original[..12], "{args}");
            assert_eq!(written[24..], original[24..], "{args}");
            if want_world {
                checked.1 += 1;
            } else {
                checked.0 += 1;
            }
        }
    }
    eprintln!(
        "placed again: {} interior and {} exterior references, bodies identical",
        checked.0, checked.1
    );
    let _ = std::fs::remove_dir_all(&out);
}

/// The Fable review's cases (lane E fix round) on the official files: (1) a `[[set]]` on
/// the apartment's reference `001B21E0` plus a `[[place]]` into its cell `0021966F` build
/// (the cell is written once, not refused as written twice); (3) an actor (`NPC_`
/// `00271133`) placed persistent in New Atlantis (`0001251B`) at (150, 350, 0) goes into
/// the world's persistent cell, and `cell.actor-without-navmesh` checks the exterior cell
/// its position names (`00012531`, which has a navmesh): no warning.
#[test]
#[ignore = "release: indexes Starfield.esm (cached after the first run)"]
fn the_reviewers_cases_on_the_official_files() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let out = std::env::temp_dir().join(format!("kitkraft-place-review-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let engine = Engine::with_default_handlers();
    // (1) set on a cell child + place into the cell.
    let spec = out.join("set-place.toml");
    std::fs::write(
        &spec,
        format!(
            "version = 3\n[load_order]\nplugins = [\"Starfield.esm\"]\ndata_dir = '{}'\n\n\
             [[set]]\nform = \"Starfield.esm:1B21E0\"\nfields = {{ \"DATA.Position.Z\" = 5.0 }}\n\n\
             [[place]]\nbase = \"Starfield.esm:1E4FBD\"\ncell = \"Starfield.esm:21966F\"\n\
             position = [-17.0, -4.0, 4.63]\npersistent = true\n",
            dir.display()
        ),
    )
    .unwrap();
    let built = out.join("SetPlace.esm");
    let r = engine
        .execute(
            "build",
            json!({"spec": spec.display().to_string(), "out": built.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("build: {e}"));
    assert_eq!(
        r["data"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    let index = index_file(&built, &IndexOptions::default()).unwrap();
    let cells: Vec<u32> = index
        .records
        .iter()
        .filter(|x| x.signature.0 == *b"CELL")
        .map(|x| x.form_id)
        .collect();
    assert_eq!(cells, [0x0021_966F]);
    let carried = r["data"]["carried"].as_array().unwrap();
    assert_eq!(carried.len(), 1, "{carried:?}");
    // (3) a persistent actor in a world.
    let into = out.join("Actor.esm");
    let r = engine
        .execute(
            "place",
            json!({"base": "Starfield.esm:271133", "world": "Starfield.esm:01251B",
                   "position": [150.0, 350.0, 0.0], "persistent": true,
                   "into": into.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("place: {e}"));
    assert_eq!(r["data"]["placed"][0]["signature"], json!("ACHR"));
    assert_eq!(r["data"]["placed"][0]["cell"]["kind"], json!("persistent"));
    let rules = r["data"]["validate"]["findings"].as_array().unwrap();
    assert!(
        !rules
            .iter()
            .any(|f| f["rule"] == json!("cell.actor-without-navmesh")),
        "{rules:?}"
    );
    let v = engine
        .execute(
            "validate",
            json!({"plugin": into.display().to_string(), "rules": ["cell.actor-without-navmesh"]}),
        )
        .unwrap();
    assert_eq!(v["data"]["findings"], json!([]), "{:#}", v["data"]);
    assert!(
        !v["limitations"].as_array().unwrap().iter().any(|l| l
            .as_str()
            .unwrap()
            .starts_with("cell.actor-without-navmesh")),
        "{:#}",
        v["limitations"]
    );
    let _ = std::fs::remove_dir_all(&out);
}
