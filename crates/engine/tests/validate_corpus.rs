//! M2.E7 acceptance: every `validate` rule on the 14 official ESMs gives zero errors,
//! except the errors [`KNOWN_ERRORS`] lists as corpus facts, each with its evidence and
//! exact count (M4.BX1: a new official-file error is a layout error in the merged family
//! until it is explained there). FormIDs naming engine-reserved forms (object IDs below
//! 0x800 in Starfield.esm's index space, `kitkraft_engine::validate::is_reserved`) are not
//! findings; their count per plugin and member path is pinned in [`RESERVED`] as the
//! regression expectation. Warnings are printed by kind for `plan/STATUS.md`. `#[ignore]`:
//! it reads 2.5 GB; run
//! `cargo test -p kitkraft-engine --release --test validate_corpus -- --ignored --nocapture`
//! with `KITKRAFT_DATA_DIR` set.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitkraft_engine::execute;
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
    numbering is a corpus fact to check in the CK (plan/STATUS.md). Since the unordered \
    record mode (M4.BX1) the XPLK that stood out of member order are typed too: \
    ShatteredSpace 30 -> 41, all 41 at index 02";

const XBPS_SELF: &str = "CELL XBPS (ship snap links: parent ref, linked ref, two node \
    indexes; xedit wbDefinitionsSF1.pas L10695-L10704, world family, M4) in SFBGS050.esm \
    cells (020A38DA, 02073D30, 02043C48 at the world merge) names SFBGS050 references the \
    file does not contain (nor BlueprintShips-SFBGS050.esm); every XBPS entry is 16 bytes \
    with 0 typed conflicts and Starfield.esm's resolve, so these are dangling links, not a \
    layout error. Since the unordered record mode (M4.BX1) the XBPS that stood out of \
    member order are typed too: SFBGS050 24/23 -> 119/107, and five \
    BlueprintShips-SFBGS050.esm cells (030DB654, 030DB67F, 030DB6AA, 030DB6D5, 030DB701) \
    name 20 of that file's own references (030DD18F..030DD1A2) it does not contain";

const LOW_BASE: &str = "REFR:00014893's NAME (base object) is 0x1FB, an object ID below \
    0x800 that no official file defines: an engine-reserved form (ADR 0005 Amendment 4), \
    which formid.dangling counts as reserved; ref.orphan (lane E, cell rules) still reports \
    the reference whose base no file defines";

const RDSA_STALE: &str = "NPC_:0018B5F4's RDSA entry (xedit wbDefinitionsSF1.pas, actor \
    family, M4.BC1) names SpecificReference 00577823, which Starfield.esm does not have; the \
    entry's flags do not enable the specific reference, so it is a stale value the game \
    ignores, not a layout error (actor lane request to lane H: does the CK clear it?)";

const STALE_SCRIPTS: &str = "VMAD (M4.E9, crates/engine/src/edit/vmad.rs; every official \
    VMAD re-encodes identically) of the file's own records attaches scripts that no archive \
    of the file or its masters holds as .pex (test and removed scripts: \
    JS_TestScript_DeleteThis, TestElectrifiedWaterTriggerScript, PlayAnimationOnLoadScript, \
    DestroyRefOnUnload, ...); the game logs and skips them. An override that carries such an \
    attachment unchanged gets a warning, not an error";

const STALE_PROPERTIES: &str = "VMAD (M4.E9) of the file's own records sets properties the \
    compiled script (its .pex in the file's or a master's archive) and its parents no \
    longer declare, e.g. RevertToOpenStateOnCellLoad.ShouldBeOpenOnReset, \
    companioncrimeresponsescript.COM_CombatResponseCivilianList, \
    SFBGS00D:SFBGS00DRandomEnableOnLoadScript.KeywordArray; the game logs and skips them. \
    An override that carries such a property unchanged gets a warning, not an error";

/// A papyrus rule's known errors in one official file (member `VMAD`, any value).
const fn papyrus(plugin: &'static str, rule: &'static str, count: u64) -> Known {
    Known {
        plugin,
        rule,
        member: "VMAD",
        value: None,
        count,
        evidence: if matches!(rule.as_bytes(), b"papyrus.missing-script") {
            STALE_SCRIPTS
        } else {
            STALE_PROPERTIES
        },
    }
}

const KNOWN_ERRORS: &[Known] = &[
    papyrus("Starfield.esm", "papyrus.missing-script", 14),
    papyrus("Starfield.esm", "papyrus.property-unresolved", 16),
    papyrus("BlueprintShips-Starfield.esm", "papyrus.missing-script", 1),
    papyrus("SFBGS00D.esm", "papyrus.missing-script", 2),
    papyrus("SFBGS00D.esm", "papyrus.property-unresolved", 25),
    papyrus("SFBGS050.esm", "papyrus.missing-script", 2),
    papyrus("SFBGS050.esm", "papyrus.property-unresolved", 7),
    papyrus("ShatteredSpace.esm", "papyrus.missing-script", 3),
    papyrus("ShatteredSpace.esm", "papyrus.property-unresolved", 13),
    dangling(
        "SFBGS050.esm",
        "PNDT.EOVR.Worldspace",
        Some(0x0202_B2F4),
        1,
        "PNDT:0005DE57's EOVR (edited worldspaces) entry names WRLD 0202B2F4, a \
            form of SFBGS050.esm itself that the file does not contain (planet lane, \
            docs/schema/planet-family.md)",
    ),
    Known {
        plugin: "Starfield.esm",
        rule: "ref.orphan",
        member: "NAME",
        value: None,
        count: 1,
        evidence: LOW_BASE,
    },
    dangling("SFBGS00D.esm", "REFR.XPLK.Ref", None, 4, XPLK_INDEX),
    dangling("SFBGS050.esm", "REFR.XPLK.Ref", None, 4, XPLK_INDEX),
    dangling("ShatteredSpace.esm", "REFR.XPLK.Ref", None, 41, XPLK_INDEX),
    dangling("SFBGS050.esm", "CELL.XBPS.ParentRef", None, 119, XBPS_SELF),
    dangling("SFBGS050.esm", "CELL.XBPS.LinkedRef", None, 107, XBPS_SELF),
    dangling(
        "BlueprintShips-SFBGS050.esm",
        "CELL.XBPS.ParentRef",
        None,
        10,
        XBPS_SELF,
    ),
    dangling(
        "BlueprintShips-SFBGS050.esm",
        "CELL.XBPS.LinkedRef",
        None,
        10,
        XBPS_SELF,
    ),
    dangling(
        "Starfield.esm",
        "NPC_.RDSA.SpecificReference",
        Some(0x0057_7823),
        1,
        RDSA_STALE,
    ),
];

/// Engine-reserved FormIDs the official files name, per plugin, record type and member path
/// (`data.reserved_by_member` keys): the regression expectation of the reserved-ID
/// exception (`docs/validate.md`, "Engine-reserved object IDs"; ADR 0005 Amendment 4). Before the exception these were `KNOWN_ERRORS` (in
/// Starfield.esm) and `formid.dangling` warnings (in the DLC files): the player reference
/// 0x14 in reference slots, actor values 0x2D0..0x3AC in PRPS and FLST lists, REFR NAME
/// 0x1FB, the story manager root 0x5B. A change in any count fails the test.
///
/// Lane B round 4 (the ADR 0005 A4 masking surface): each entry also pins the distinct
/// reserved object IDs the member names (compact hex runs), so a new ID behind the same
/// count fails too; every one must be in `validate::NAMED_RESERVED_OBJECT_IDS` or listed in
/// [`UNNAMED_RESERVED`].
const RESERVED: &[(&str, &str, u64, &str)] = &[
    (
        "Starfield.esm",
        "ACTI PropertyValue.ActorValue",
        128,
        "2D0,32E-330,338,3A2",
    ),
    (
        "Starfield.esm",
        "CONT PropertyValue.ActorValue",
        443,
        "330,33D,387,3A2,3AC",
    ),
    (
        "Starfield.esm",
        "FLST FLST.LNAM",
        23,
        "2D8,2DB,2E2,2EE,313,32E,34F-352,355,359",
    ),
    ("Starfield.esm", "FURN PropertyValue.ActorValue", 13, "33E"),
    ("Starfield.esm", "LCTN LCTN.LCEP.Parent", 2, "14"),
    (
        "Starfield.esm",
        "NPC_ PropertyValue.ActorValue",
        139,
        "32D,330,351,385,39E,3A2",
    ),
    (
        "Starfield.esm",
        "OMOD ObjectModProperty.Value1",
        3,
        "2E2,312",
    ),
    (
        "Starfield.esm",
        "PKIN PropertyValue.ActorValue",
        21,
        "33D-33E",
    ),
    ("Starfield.esm", "QUST QuestReferenceAlias.ALFR", 2, "14"),
    (
        "Starfield.esm",
        "RACE PropertyValue.ActorValue",
        178,
        "2D8-2D9,2E0,385,39E-3A1",
    ),
    ("Starfield.esm", "REFR EnableParent.Ref", 6, "14"),
    ("Starfield.esm", "REFR LinkedReferences.XLKR.Ref", 1, "14"),
    ("Starfield.esm", "REFR REFR.NAME", 1, "1FB"),
    ("Starfield.esm", "REFR REFR.XATR", 4, "14"),
    ("Starfield.esm", "SMEN SMEN.PNAM", 20, "5B"),
    (
        "Starfield.esm",
        "STAT PropertyValue.ActorValue",
        61,
        "33D-33E",
    ),
    ("Starfield.esm", "TERM PropertyValue.ActorValue", 1, "3A2"),
    (
        "SFBGS003.esm",
        "NPC_ PropertyValue.ActorValue",
        186,
        "39E-3A0",
    ),
    (
        "SFBGS003.esm",
        "RACE PropertyValue.ActorValue",
        7,
        "2D8-2D9,2E0,39E-3A1",
    ),
    ("SFBGS003.esm", "SMEN SMEN.PNAM", 1, "5B"),
    (
        "SFBGS004.esm",
        "RACE PropertyValue.ActorValue",
        14,
        "2D8-2D9,2E0,39E-3A1",
    ),
    ("SFBGS00D.esm", "FURN PropertyValue.ActorValue", 1, "33E"),
    (
        "SFBGS00D.esm",
        "NPC_ PropertyValue.ActorValue",
        3,
        "385,390",
    ),
    (
        "SFBGS00D.esm",
        "OMOD ObjectModProperty.Value1",
        2,
        "385,3B7",
    ),
    ("SFBGS00D.esm", "QUST QuestReferenceAlias.ALFR", 1, "14"),
    (
        "SFBGS00D.esm",
        "RACE PropertyValue.ActorValue",
        3,
        "2D8-2D9,2E0",
    ),
    ("SFBGS00D.esm", "REFR EnableParent.Ref", 1, "14"),
    ("SFBGS00D.esm", "REFR LinkedReferences.XLKR.Ref", 1, "14"),
    (
        "SFBGS047.esm",
        "RACE PropertyValue.ActorValue",
        14,
        "2D8-2D9,2E0,39E-3A1",
    ),
    (
        "SFBGS050.esm",
        "ACTI PropertyValue.ActorValue",
        8,
        "2D0,330,3A2",
    ),
    (
        "SFBGS050.esm",
        "CONT PropertyValue.ActorValue",
        12,
        "330,33D,3A2",
    ),
    ("SFBGS050.esm", "LCTN LCTN.LCEP.Parent", 1, "14"),
    (
        "SFBGS050.esm",
        "NPC_ PropertyValue.ActorValue",
        3,
        "32D,385",
    ),
    (
        "SFBGS050.esm",
        "RACE PropertyValue.ActorValue",
        31,
        "2D8-2D9,2E0,39E-3A1",
    ),
    ("SFBGS050.esm", "REFR EnableParent.Ref", 2, "14"),
    (
        "ShatteredSpace.esm",
        "ACTI PropertyValue.ActorValue",
        4,
        "2D0",
    ),
    (
        "ShatteredSpace.esm",
        "CONT PropertyValue.ActorValue",
        1,
        "33D",
    ),
    (
        "ShatteredSpace.esm",
        "NPC_ PropertyValue.ActorValue",
        36,
        "32D,385",
    ),
    (
        "ShatteredSpace.esm",
        "PKIN PropertyValue.ActorValue",
        2,
        "33E",
    ),
    (
        "ShatteredSpace.esm",
        "QUST QuestReferenceAlias.ALFR",
        1,
        "14",
    ),
    (
        "ShatteredSpace.esm",
        "RACE PropertyValue.ActorValue",
        31,
        "2D8-2D9,2E0,39E-3A1",
    ),
    ("ShatteredSpace.esm", "REFR EnableParent.Ref", 1, "14"),
    ("ShatteredSpace.esm", "REFR REFR.XATR", 1, "14"),
    ("ShatteredSpace.esm", "SMEN SMEN.PNAM", 1, "5B"),
];
/// Reserved IDs outside `validate::NAMED_RESERVED_OBJECT_IDS` that an official file names
/// (each is skipped by `formid.dangling` with a warning until a CK observation): lane B
/// round 4 typed OMOD property values, and SFBGS00D.esm OMOD 01026328 names 0x3B7 from a
/// FormID value (value type 4 or 6) that `Starfield.esm` does not define, just past the
/// 0x2D0..0x3AC actor value run the other files name.
const UNNAMED_RESERVED: &[(&str, &str, u32)] =
    &[("SFBGS00D.esm", "OMOD ObjectModProperty.Value1", 0x3B7)];

/// Sorted object IDs as compact hex runs: `14`, `2D0-2D2,2F1`.
fn compact(ids: &[u32]) -> String {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for id in sorted {
        match runs.last_mut() {
            Some((_, hi)) if hi.checked_add(1) == Some(id) => *hi = id,
            _ => runs.push((id, id)),
        }
    }
    runs.iter()
        .map(|(lo, hi)| {
            if lo == hi {
                format!("{lo:X}")
            } else {
                format!("{lo:X}-{hi:X}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

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
    let Some(dir) = std::env::var_os("KITKRAFT_DATA_DIR")
        .or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
    else {
        println!("skipped: KITKRAFT_DATA_DIR is unset or not a directory");
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
        let ids_of = |member: &str| -> Vec<u32> {
            data["reserved_ids_by_member"][member]
                .as_array()
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| u32::from_str_radix(id.as_str()?, 16).ok())
                        .collect()
                })
                .unwrap_or_default()
        };
        let seen: BTreeMap<String, (u64, String)> = data["reserved_by_member"]
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), (v.as_u64().unwrap_or(0), compact(&ids_of(k)))))
                    .collect()
            })
            .unwrap_or_default();
        let expected: BTreeMap<String, (u64, String)> = RESERVED
            .iter()
            .filter(|(plugin, _, _, _)| *plugin == name)
            .map(|(_, member, count, ids)| ((*member).to_owned(), (*count, (*ids).to_owned())))
            .collect();
        println!(
            "    reserved: {} {}",
            data["reserved_form_ids"],
            Value::from(serde_json::Map::from_iter(
                seen.iter()
                    .map(|(k, (n, ids))| (k.clone(), json!(format!("{n} [{ids}]"))))
            ))
        );
        if seen != expected {
            for (member, (count, ids)) in &seen {
                println!("    reserved count: (\"{name}\", \"{member}\", {count}, \"{ids}\"),");
            }
            println!("    expected: {expected:?}");
            failures.push(name);
        }
        // Every reserved ID the official files name is in the set the warning rule uses,
        // or one of the listed exceptions.
        let mut outside: Vec<(String, u32)> = Vec::new();
        for member in seen.keys() {
            for id in ids_of(member) {
                if !kitkraft_engine::validate::is_named_reserved(id) {
                    println!("    reserved ID {id:03X} in {member} is outside the named set");
                    outside.push((member.clone(), id));
                }
            }
        }
        let listed: Vec<(String, u32)> = UNNAMED_RESERVED
            .iter()
            .filter(|(plugin, _, _)| *plugin == name)
            .map(|(_, member, id)| ((*member).to_owned(), *id))
            .collect();
        if outside != listed {
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
