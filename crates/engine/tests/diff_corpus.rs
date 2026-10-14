//! M3.D2 on an official file (`KITCRAFT_DATA_DIR`; skips when unset): `diff` of a corpus
//! plugin against a copy with one typed member changed (a keyword FormID in a `KWDA`)
//! reports that member's path with both forms, through `execute`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use kitcraft_doc::index::PluginIndex;
use kitcraft_doc::record::read_record_file;
use serde_json::json;

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("KITCRAFT_DATA_DIR")?);
    dir.join("SFBGS004.esm").is_file().then_some(dir)
}

#[test]
fn corpus_diff_names_the_changed_keyword() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITCRAFT_DATA_DIR unset");
        return;
    };
    let original = dir.join("SFBGS004.esm");
    let index = PluginIndex::build(&original).unwrap();
    // Every keyword FormID stored in an uncompressed KWDA, with the file offset of the first
    // such element of each record.
    let mut hits: Vec<(u64, u32, usize)> = Vec::new(); // (element offset, raw, element index)
    let mut seen: Vec<u32> = Vec::new();
    // Records of a type the schema defines (their KWDA has a layout), uncompressed.
    let schema = kitcraft_schema::builtin().unwrap();
    let defined = |sig: [u8; 4]| {
        kitcraft_schema::Signature::from_bytes(sig).is_some_and(|s| schema.record(s).is_some())
    };
    for r in index
        .records
        .iter()
        .filter(|r| r.flags & 0x0004_0000 == 0 && defined(r.signature))
    {
        let rec = read_record_file(&original, r).unwrap();
        let Some(kwda) = rec.subs.iter().find(|s| s.signature().0 == *b"KWDA") else {
            continue;
        };
        let data = kwda.data();
        for (i, chunk) in data.as_chunks::<4>().0.iter().enumerate() {
            let raw = u32::from_le_bytes(*chunk);
            if !seen.contains(&raw) {
                seen.push(raw);
            }
            if i == 0 && raw != 0 {
                hits.push((kwda.data_offset().unwrap(), raw, i));
            }
        }
    }
    let (at, before_raw, element) = hits[0];
    let after_raw = *seen
        .iter()
        .find(|r| **r != before_raw && **r != 0)
        .expect("a second keyword");
    let mut bytes = std::fs::read(&original).unwrap();
    bytes[at as usize..at as usize + 4].copy_from_slice(&after_raw.to_le_bytes());
    let tmp = std::env::temp_dir().join(format!("kitcraft-engine-diff-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let copy = tmp.join("SFBGS004 - edited.esm");
    std::fs::write(&copy, &bytes).unwrap();

    let r = kitcraft_engine::execute(
        "diff",
        json!({
            "a": original.display().to_string(),
            "b": copy.display().to_string(),
            "data_dir": dir.display().to_string(),
        }),
    )
    .unwrap();
    let data = &r["data"];
    assert_eq!(data["counts"]["changed"], 1, "{}", data["summary"]);
    assert_eq!(data["counts"]["added"], 0);
    assert_eq!(data["counts"]["removed"], 0);
    let subs = data["changed"][0]["subrecords"].as_array().unwrap();
    assert_eq!(subs.len(), 1);
    let s = &subs[0];
    assert_eq!(s["signature"], "KWDA");
    assert_eq!(s["member"], "Forms");
    let fields = s["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0]["path"], format!("KWDA.Forms[{element}]"));
    let before = fields[0]["before"]["form"].as_str().unwrap();
    let after = fields[0]["after"]["form"].as_str().unwrap();
    assert_ne!(before, after);
    // Both are formrefs naming a plugin of the load order (the file or its masters).
    for f in [before, after] {
        let (plugin, object) = f.split_once(':').unwrap();
        assert!(plugin.to_ascii_lowercase().ends_with(".esm"), "{f}");
        assert!(u32::from_str_radix(object, 16).is_ok(), "{f}");
    }
    eprintln!(
        "{}: {} {before} -> {after}",
        data["changed"][0]["form"], fields[0]["path"]
    );
    std::fs::remove_dir_all(&tmp).unwrap();
}
