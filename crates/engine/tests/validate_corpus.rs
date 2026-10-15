//! M2.E7 acceptance: every `validate` rule on the 14 official ESMs gives zero errors,
//! except the errors [`KNOWN_ERRORS`] lists as corpus facts, each with its evidence and
//! exact count (M4.BX1: a new official-file error is a layout error in the merged family
//! until it is explained there). FormIDs naming engine-reserved forms (object IDs below
//! 0x800 in Starfield.esm's index space, `kitcraft_engine::validate::is_reserved`) are not
//! findings; their count per plugin and member path is pinned in [`RESERVED`] as the
//! regression expectation. Warnings are printed by kind for `plan/STATUS.md`. `#[ignore]`:
//! it reads 2.5 GB; run
//! `cargo test -p kitcraft-engine --release --test validate_corpus -- --ignored --nocapture`
//! with `KITCRAFT_DATA_DIR` set.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitcraft_engine::execute;
use serde_json::{json, Value};

/// Errors the official files really have: real dangling FormIDs that more typed members
/// expose, not layout errors. Each entry is matched by plugin, rule, member path and stored
/// value (`None`: any value of that member), and must occur exactly `count` times, so a
/// new error (or a fixed one) fails the test instead of being hidden.
struct Known {
    plugin: &'static str,
    rule: &'static str,
    member: &'static str,
    value: Option<u32>,
    count: u64,
    /// Why this is a corpus fact.
    evidence: &'static str,
}

/// A `formid.dangling` known error.
const fn dangling(
    plugin: &'static str,
    member: &'static str,
    value: Option<u32>,
    count: u64,
    evidence: &'static str,
) -> Known {
    Known {
        plugin,
        rule: "formid.dangling",
        member,
        value,
        count,
        evidence,
    }
}

const XPLK_INDEX: &str = "REFR XPLK (power links: reference, u32; xedit wbDefinitionsSF1.pas \
    L8844, world family, M4) in the DLC files stores its reference with a file index one \
    above the plugin's own (SFBGS00D 02 for 01, SFBGS050 03 for 02, ShatteredSpace 02 for \
    01); with the plugin's own index every target exists (kit inspect: SFBGS00D 0103065F, \
    SFBGS050 0202599E and 02028369 are REFR records next to the linking ones), and \
    Starfield.esm's XPLK all resolve. The layout fits (8 bytes, 0 typed conflicts); the \
    numbering is a corpus fact to check in the CK (plan/STATUS.md)";

const XBPS_SELF: &str = "CELL XBPS (ship snap links: parent ref, linked ref, two node \
    indexes; xedit wbDefinitionsSF1.pas L10695-L10704, world family, M4) in three \
    SFBGS050.esm cells (020A38DA, 02073D30, 02043C48) names SFBGS050 references the file \
    does not contain (nor BlueprintShips-SFBGS050.esm); every XBPS entry is 16 bytes with 0 \
    typed conflicts and Starfield.esm's resolve, so these are dangling links, not a layout \
    error";

const RDSA_STALE: &str = "NPC_:0018B5F4's RDSA entry (xedit wbDefinitionsSF1.pas, actor \
    family, M4.BC1) names SpecificReference 00577823, which Starfield.esm does not have; the \
    entry's flags do not enable the specific reference, so it is a stale value the game \
    ignores, not a layout error (actor lane request to lane H: does the CK clear it?)";

const KNOWN_ERRORS: &[Known] = &[
    dangling(
        "SFBGS050.esm",
        "PNDT.EOVR.Worldspace",
        Some(0x0202_B2F4),
        1,
        "PNDT:0005DE57's EOVR (edited worldspaces) entry names WRLD 0202B2F4, a \
            form of SFBGS050.esm itself that the file does not contain (planet lane, \
            docs/schema/planet-family.md)",
    ),
    dangling("SFBGS00D.esm", "REFR.XPLK.Ref", None, 4, XPLK_INDEX),
    dangling("SFBGS050.esm", "REFR.XPLK.Ref", None, 4, XPLK_INDEX),
    dangling("ShatteredSpace.esm", "REFR.XPLK.Ref", None, 30, XPLK_INDEX),
    dangling("SFBGS050.esm", "CELL.XBPS.ParentRef", None, 24, XBPS_SELF),
    dangling("SFBGS050.esm", "CELL.XBPS.LinkedRef", None, 23, XBPS_SELF),
    dangling(
        "Starfield.esm",
        "NPC_.RDSA.SpecificReference",
        Some(0x0057_7823),
        1,
        RDSA_STALE,
    ),
];

/// Engine-reserved FormIDs the official files name, per plugin and member path: the
/// regression expectation of the reserved-ID exception (`docs/validate.md`, "Engine-reserved
/// object IDs"; ADR 0005 Amendment 4). Before the exception these were `KNOWN_ERRORS` (in
/// Starfield.esm) and `formid.dangling` warnings (in the DLC files): the player reference
/// 0x14 in reference slots, actor values 0x2D0..0x3AC in PRPS and FLST lists, REFR NAME
/// 0x1FB, the story manager root 0x5B. A change in any count fails the test.
const RESERVED: &[(&str, &str, u64)] = &[
    ("Starfield.esm", "EnableParent.Ref", 6),
    ("Starfield.esm", "FLST.LNAM", 23),
    ("Starfield.esm", "LCTN.LCEP.Parent", 2),
    ("Starfield.esm", "LinkedReferences.XLKR.Ref", 1),
    ("Starfield.esm", "NPC_.PRPS.ActorValue", 139),
    ("Starfield.esm", "PKIN.PRPS.ActorValue", 21),
    ("Starfield.esm", "QuestReferenceAlias.ALFR", 2),
    ("Starfield.esm", "RACE.PRPS.ActorValue", 178),
    ("Starfield.esm", "REFR.NAME", 1),
    ("Starfield.esm", "REFR.XATR", 4),
    ("Starfield.esm", "SMEN.PNAM", 20),
    ("SFBGS003.esm", "NPC_.PRPS.ActorValue", 186),
    ("SFBGS003.esm", "RACE.PRPS.ActorValue", 7),
    ("SFBGS003.esm", "SMEN.PNAM", 1),
    ("SFBGS004.esm", "RACE.PRPS.ActorValue", 14),
    ("SFBGS00D.esm", "EnableParent.Ref", 1),
    ("SFBGS00D.esm", "LinkedReferences.XLKR.Ref", 1),
    ("SFBGS00D.esm", "NPC_.PRPS.ActorValue", 3),
    ("SFBGS00D.esm", "QuestReferenceAlias.ALFR", 1),
    ("SFBGS00D.esm", "RACE.PRPS.ActorValue", 3),
    ("SFBGS047.esm", "RACE.PRPS.ActorValue", 14),
    ("SFBGS050.esm", "EnableParent.Ref", 2),
    ("SFBGS050.esm", "LCTN.LCEP.Parent", 1),
    ("SFBGS050.esm", "NPC_.PRPS.ActorValue", 3),
    ("SFBGS050.esm", "RACE.PRPS.ActorValue", 31),
    ("ShatteredSpace.esm", "EnableParent.Ref", 1),
    ("ShatteredSpace.esm", "NPC_.PRPS.ActorValue", 36),
    ("ShatteredSpace.esm", "PKIN.PRPS.ActorValue", 2),
    ("ShatteredSpace.esm", "QuestReferenceAlias.ALFR", 1),
    ("ShatteredSpace.esm", "RACE.PRPS.ActorValue", 31),
    ("ShatteredSpace.esm", "REFR.XATR", 1),
    ("ShatteredSpace.esm", "SMEN.PNAM", 1),
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
    let mut known: BTreeMap<(String, String, String, Option<u32>), u64> = BTreeMap::new();
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
                k.plugin == name
                    && k.rule == rule
                    && k.member == member
                    && k.value.is_none_or(|v| Some(v) == value)
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
        let seen: BTreeMap<String, u64> = data["reserved_by_member"]
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0)))
                    .collect()
            })
            .unwrap_or_default();
        let expected: BTreeMap<String, u64> = RESERVED
            .iter()
            .filter(|(plugin, _, _)| *plugin == name)
            .map(|(_, member, count)| ((*member).to_owned(), *count))
            .collect();
        println!(
            "    reserved: {} {}",
            data["reserved_form_ids"],
            Value::from(serde_json::Map::from_iter(
                seen.iter().map(|(k, v)| (k.clone(), json!(v)))
            ))
        );
        if seen != expected {
            for (member, count) in &seen {
                println!("    reserved count: (\"{name}\", \"{member}\", {count}),");
            }
            println!("    expected: {expected:?}");
            failures.push(name);
        }
    }
    for k in KNOWN_ERRORS {
        let seen = known
            .get(&(k.plugin.into(), k.rule.into(), k.member.into(), k.value))
            .copied()
            .unwrap_or(0);
        let value = k
            .value
            .map_or_else(|| "any".to_owned(), |v| format!("{v:08X}"));
        println!(
            "known: {} {} {} = {value}: {seen} (expected {}): {}",
            k.plugin, k.rule, k.member, k.count, k.evidence
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
