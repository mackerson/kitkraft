//! M2.E7 acceptance: every `validate` rule on the 14 official ESMs gives zero errors,
//! except the errors [`KNOWN_ERRORS`] lists as corpus facts, each with its evidence and
//! exact count (M4.BX1: a new official-file error is a layout error in the merged family
//! until it is explained there). Warnings are printed by kind for `plan/STATUS.md`. `#[ignore]`: it reads 2.5 GB; run
//! `cargo test -p kitcraft-engine --release --test validate_corpus -- --ignored --nocapture`
//! with `KITCRAFT_DATA_DIR` set.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitcraft_engine::execute;
use serde_json::json;

/// Errors the official files really have: real dangling FormIDs that more typed members
/// expose, not layout errors. Each entry is matched by plugin, rule, member path and stored
/// value, and must occur exactly `count` times, so a new error (or a fixed one) fails the
/// test instead of being hidden.
struct Known {
    plugin: &'static str,
    rule: &'static str,
    member: &'static str,
    value: u32,
    count: u64,
    /// Why this is a corpus fact.
    evidence: &'static str,
}

const PRPS_AV: &str = "PKIN PRPS (properties: actor value, value, curve; xedit \
    wbDefinitionsSF1.pas, M4 planet family) names actor values 0x33D (7 records) and 0x33E \
    (14), object IDs below 0x800 that no official file defines; ShatteredSpace.esm has 2 \
    more (warnings, since they name its master). Grade D hypothesis: engine-reserved IDs \
    below 0x800 (a CK check, plan/STATUS.md, planet lane request to lane E)";

const KNOWN_ERRORS: &[Known] = &[
    Known {
        plugin: "Starfield.esm",
        rule: "formid.dangling",
        member: "PKIN.PRPS.ActorValue",
        value: 0x0000_033D,
        count: 7,
        evidence: PRPS_AV,
    },
    Known {
        plugin: "Starfield.esm",
        rule: "formid.dangling",
        member: "PKIN.PRPS.ActorValue",
        value: 0x0000_033E,
        count: 14,
        evidence: PRPS_AV,
    },
    Known {
        plugin: "SFBGS050.esm",
        rule: "formid.dangling",
        member: "PNDT.EOVR.Worldspace",
        value: 0x0202_B2F4,
        count: 1,
        evidence: "PNDT:0005DE57's EOVR (edited worldspaces) entry names WRLD 0202B2F4, a \
            form of SFBGS050.esm itself that the file does not contain (planet lane, \
            docs/schema/planet-family.md)",
    },
];

/// The stored FormID a `formid.dangling` message names (`... = 0000033E names ...`).
fn named_value(message: &str) -> Option<u32> {
    let at = message.find(" = ")?;
    let hex = message.get(at.checked_add(3)?..at.checked_add(11)?)?;
    u32::from_str_radix(hex, 16).ok()
}

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
    // (plugin, rule, member, value) -> count of errors on the known list.
    let mut known: BTreeMap<(String, String, String, u32), u64> = BTreeMap::new();
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
        let mut unexplained = 0u64;
        for f in data["findings"].as_array().unwrap() {
            if f["severity"] != json!("error") {
                continue;
            }
            let rule = f["rule"].as_str().unwrap_or_default();
            let member = f["subrecord"].as_str().unwrap_or_default();
            let value = f["message"].as_str().and_then(named_value);
            let hit = KNOWN_ERRORS.iter().find(|k| {
                k.plugin == name && k.rule == rule && k.member == member && Some(k.value) == value
            });
            match hit {
                Some(k) => {
                    *known
                        .entry((k.plugin.into(), k.rule.into(), k.member.into(), k.value))
                        .or_default() += 1;
                }
                None => {
                    unexplained += 1;
                    println!("    unexplained error: {} {}", f["record"], f["message"]);
                }
            }
        }
        let listed = data["findings"].as_array().unwrap().len() as u64;
        assert_eq!(
            data["findings_total"],
            json!(listed),
            "{name}: findings are capped; errors cannot be matched"
        );
        if unexplained > 0 {
            failures.push(name);
        }
    }
    for k in KNOWN_ERRORS {
        let seen = known
            .get(&(k.plugin.into(), k.rule.into(), k.member.into(), k.value))
            .copied()
            .unwrap_or(0);
        println!(
            "known: {} {} {} = {:08X}: {seen} (expected {}): {}",
            k.plugin, k.rule, k.member, k.value, k.count, k.evidence
        );
        if seen != k.count {
            failures.push(k.plugin);
        }
    }
    assert!(
        failures.is_empty(),
        "unexplained or changed errors in {failures:?}"
    );
}
