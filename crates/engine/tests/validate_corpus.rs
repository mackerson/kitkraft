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

const PRPS_AV: &str = "PKIN PRPS (properties: actor value, value, curve; xedit \
    wbDefinitionsSF1.pas, M4 planet family) names actor values 0x33D (7 records) and 0x33E \
    (14), object IDs below 0x800 that no official file defines; ShatteredSpace.esm has 2 \
    more (warnings, since they name its master). Grade D hypothesis: engine-reserved IDs \
    below 0x800 (a CK check, plan/STATUS.md, planet lane request to lane E)";

const FLST_AV: &str = "FLST LNAM (one FormID per entry; xedit wbDefinitionsSF1.pas, lane B \
    round 3, M3.B7) in five actor-value form lists of Starfield.esm (ENV_imGui_4_ActorValues, \
    ENV_imGui_4B_ActorValuesBasic, ENV_imGui_6_Weather, \
    ENV_imGui_7B_Breathing_and_Suit_Integrity, SpaceshipPartSummedValues: 23 entries) names \
    object IDs 0x2D8..0x359 below 0x800 that no official file defines, the same class as the \
    PKIN PRPS actor values. Grade D hypothesis: engine-reserved actor values (a CK check, \
    plan/STATUS.md)";

/// One `FLST.LNAM` known error of Starfield.esm.
const fn flst_av(value: u32, count: u64) -> Known {
    dangling("Starfield.esm", "FLST.LNAM", Some(value), count, FLST_AV)
}

const PLAYER_REF: &str = "object ID 0x14 in a placed-reference slot (world family, M4: enable \
    parent, attach ref, linked ref, location enable parent) whose xEdit target list names \
    PLYR, the player reference (xedit wbDefinitionsSF1.pas L3892-L3899 and the \
    placed-reference target lists); no official file defines 0x14. Grade D hypothesis: the \
    engine's player reference (validate could resolve it, request to lane E)";

const LOW_BASE: &str = "REFR:00014893's NAME (base object) is 0x1FB, an object ID below \
    0x800 that no official file defines, the same class as the PKIN PRPS and FLST LNAM \
    actor values (world family, M4). Grade D hypothesis: an engine-reserved form";

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

const KNOWN_ERRORS: &[Known] = &[
    flst_av(0x0000_02D8, 2),
    flst_av(0x0000_02DB, 1),
    flst_av(0x0000_02E2, 3),
    flst_av(0x0000_02EE, 1),
    flst_av(0x0000_0313, 3),
    flst_av(0x0000_032E, 1),
    flst_av(0x0000_034F, 3),
    flst_av(0x0000_0350, 2),
    flst_av(0x0000_0351, 2),
    flst_av(0x0000_0352, 2),
    flst_av(0x0000_0355, 1),
    flst_av(0x0000_0359, 2),
    dangling(
        "Starfield.esm",
        "PKIN.PRPS.ActorValue",
        Some(0x0000_033D),
        7,
        PRPS_AV,
    ),
    dangling(
        "Starfield.esm",
        "PKIN.PRPS.ActorValue",
        Some(0x0000_033E),
        14,
        PRPS_AV,
    ),
    dangling(
        "SFBGS050.esm",
        "PNDT.EOVR.Worldspace",
        Some(0x0202_B2F4),
        1,
        "PNDT:0005DE57's EOVR (edited worldspaces) entry names WRLD 0202B2F4, a \
            form of SFBGS050.esm itself that the file does not contain (planet lane, \
            docs/schema/planet-family.md)",
    ),
    dangling(
        "Starfield.esm",
        "EnableParent.Ref",
        Some(0x14),
        6,
        PLAYER_REF,
    ),
    dangling("Starfield.esm", "REFR.XATR", Some(0x14), 4, PLAYER_REF),
    dangling(
        "Starfield.esm",
        "LinkedReferences.XLKR.Ref",
        Some(0x14),
        1,
        PLAYER_REF,
    ),
    dangling(
        "Starfield.esm",
        "LCTN.LCEP.Parent",
        Some(0x14),
        2,
        PLAYER_REF,
    ),
    dangling("Starfield.esm", "REFR.NAME", Some(0x1FB), 1, LOW_BASE),
    dangling("SFBGS00D.esm", "REFR.XPLK.Ref", None, 4, XPLK_INDEX),
    dangling("SFBGS050.esm", "REFR.XPLK.Ref", None, 4, XPLK_INDEX),
    dangling("ShatteredSpace.esm", "REFR.XPLK.Ref", None, 30, XPLK_INDEX),
    dangling("SFBGS050.esm", "CELL.XBPS.ParentRef", None, 24, XBPS_SELF),
    dangling("SFBGS050.esm", "CELL.XBPS.LinkedRef", None, 23, XBPS_SELF),
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
