//! M2.E7 acceptance: every `validate` rule on the 14 official ESMs gives zero errors.
//! Warnings are printed by kind for `plan/STATUS.md`. `#[ignore]`: it reads 2.5 GB; run
//! `cargo test -p kitcraft-engine --release --test validate_corpus -- --ignored --nocapture`
//! with `KITCRAFT_DATA_DIR` set.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitcraft_engine::execute;
use serde_json::json;

const CORPUS: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS003.esm",
    "SFBGS004.esm",
    "SFBGS006.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS00D.esm",
    "SFBGS047.esm",
    "SFBGS050.esm",
    "ShatteredSpace.esm",
];

#[test]
#[ignore = "validates 2.5 GB of plugins; run in release"]
fn validate_corpus_official_files() {
    let Some(dir) = std::env::var_os("KITCRAFT_DATA_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
    else {
        println!("skipped: KITCRAFT_DATA_DIR is unset or not a directory");
        return;
    };
    let mut failures = Vec::new();
    for name in CORPUS {
        let started = std::time::Instant::now();
        let result = execute(
            "validate",
            json!({"plugin": dir.join(name).display().to_string()}),
        )
        .unwrap();
        let data = &result["data"];
        let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
        let mut examples: BTreeMap<String, String> = BTreeMap::new();
        for f in data["findings"].as_array().unwrap() {
            let record_type = f["record"].as_str().and_then(|r| r.get(..4)).unwrap_or("-");
            let key = format!(
                "{} {} {record_type}",
                f["severity"].as_str().unwrap(),
                f["rule"].as_str().unwrap()
            );
            *kinds.entry(key.clone()).or_default() += 1;
            examples
                .entry(key)
                .or_insert_with(|| format!("{} {}", f["record"], f["message"]));
        }
        println!(
            "{name}: errors {}, warnings {}, findings {} (listed {:?}), checked FormIDs {}, unresolved {}, unchecked subrecords {}, {} ms",
            data["errors"],
            data["warnings"],
            data["findings_total"],
            kinds,
            data["checked_form_ids"],
            data["unresolved_form_ids"],
            data["unchecked_subrecords"],
            started.elapsed().as_millis()
        );
        for (kind, example) in &examples {
            println!("    {kind}: e.g. {example}");
        }
        if data["errors"] != json!(0) {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "errors in {failures:?}");
}
