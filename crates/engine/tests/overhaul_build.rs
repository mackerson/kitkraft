//! M3.E9: the gun overhaul spec (`docs/overhaul/overhaul.toml`) built against a fixture of
//! the real records. The test copies the records the spec reads (14 WEAP, 4 AMDL, 10 OMOD,
//! 3 AMMO, 2 LVLI) byte for byte out of the official `Starfield.esm` (read in place through
//! `KITKRAFT_DATA_DIR`; skipped when unset) into a small localized `Starfield.esm` in a temp
//! Data directory, with the English string tables extracted next to it, swaps the spec's
//! `[load_order]` (the owner's `KitKraft Dev` profile) for that directory, and checks the
//! plan line by line: every change with its brief row, rule, before and after. Then it
//! builds, validates, rebuilds (identical bytes) and round-trips the plugin. Nothing from the
//! game is committed; everything is written under the OS temp dir.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use kitkraft_engine::execute;
use kitkraft_esm::build::HeaderSpec;
use kitkraft_esm::tes4::{FLAG_LOCALIZED, FLAG_MASTER};
use kitkraft_esm::{Entry, GroupHeader, ParseOptions, Plugin, Record, Signature};
use serde_json::{json, Value};

fn data_dir() -> Option<PathBuf> {
    match std::env::var_os("KITKRAFT_DATA_DIR")
        .or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
        .map(PathBuf::from)
    {
        Some(dir) if dir.join("Starfield.esm").is_file() => Some(dir),
        _ => {
            println!(
                "skipped: KITKRAFT_DATA_DIR is unset or has no Starfield.esm (docs/corpus.md)"
            );
            None
        }
    }
}

/// The records the spec reads, by type and EditorID.
const RECORDS: &[(&str, &[&str])] = &[
    (
        "WEAP",
        &[
            "M1919",
            "XM2311",
            "RussianHuntingRifle",
            "Maelstrom",
            "Kraken",
            "UrbanEagle",
            "Lawgiver",
            "Tombstone",
            "Beowulf",
            "HardTarget",
            "DrumBeat",
            "Shotty",
            "Breach",
            "Microgun",
        ],
    ),
    (
        "AMDL",
        &[
            "Maelstrom_FA_AM",
            "Tombstone_FA_AM",
            "Drumbeat_SA_AM",
            "Drumbeat_FA_AM",
        ],
    ),
    (
        "OMOD",
        &[
            "mod_Grendel_Receiver_FullAuto_Standard",
            "mod_Beowulf_Receiver_FullAuto",
            "mod_Maelstrom_Receiver_FullAuto_Standard",
            "mod_Drumbeat_Receiver_FullAuto_Standard",
            "mod_AA99_Receiver_FullAuto_Standard",
            "mod_Kodama_Receiver_FullAuto_Standard",
            "mod_HardTarget_Mag_L",
            "mod_Shotty_Mag_L",
            "mod_Shotty_Mag_Extended",
            "mod_Shotty_Mag_XL",
        ],
    ),
    ("AMMO", &["Ammo777mm", "Ammo65mm", "Ammo11mm"]),
    ("LVLI", &["LLW_Ammo_Beowulf", "LLW_Ammo_Kraken"]),
];

/// The plan's change lines (the CLI text; `docs/overhaul/build-1-plan.md` has the same
/// lines from the owner's profile, where the rifle's name before is the smoke test's).
const CHANGES: &[&str] = &[
    "  B1   rule[0] WEAP Starfield.esm:24561C XM2311 WAM2.AmmoCapacity: 9 -> 7",
    "  B1   rule[0] WEAP Starfield.esm:24561C XM2311 WDMG.BaseDamage: 36 -> 30.6",
    "  B1   rule[0] WEAP Starfield.esm:26D96C M1919 WAM2.AmmoCapacity: 9 -> 7",
    "  B1   rule[0] WEAP Starfield.esm:26D96C M1919 WDMG.BaseDamage: 28 -> 23.8",
    "  B2   rule[1] WEAP Starfield.esm:21BBCD RussianHuntingRifle FULL: {\"string_id\":\"0x0002E469\"} -> \"Old Earth Marksman Rifle\"",
    "  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WAM2.AmmoCapacity: 40 -> 24",
    "  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WDMG.BaseDamage: 4 -> 5.4",
    "  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WFIR.AttackAnimationDuration: 1.3166667 -> 1.7555556",
    "  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WFIR.AttackDelaySeconds: 0.13166668 -> 0.17555557",
    "  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WFIR.ShotsPerSecond: 7.5949364 -> 5.6962023",
    "  B3   rule[3] AMDL Starfield.esm:1B01CA Maelstrom_FA_AM ANAM.RecoilDegreesPerShotX: 0.1 -> 0.15",
    "  B3   rule[3] AMDL Starfield.esm:1B01CA Maelstrom_FA_AM ANAM.RecoilDegreesPerShotY: 0.3 -> 0.45000002",
    "  B4   rule[4] WEAP Starfield.esm:21FEB4 Kraken WAM2.AmmoList: \"0012F0CE\" -> \"0000E841\"",
    "  B4   rule[4] WEAP Starfield.esm:21FEB4 Kraken WAM2.AmmoType: \"002B5590\" -> \"0004AD3E\"",
    "  B4   rule[4] WEAP Starfield.esm:21FEB4 Kraken WDMG.BaseDamage: 3 -> 3.6",
    "  B5   rule[5] WEAP Starfield.esm:26D96D UrbanEagle WFIR.AttackAnimationDuration: 0.4 -> 0.22857143",
    "  B5   rule[5] WEAP Starfield.esm:26D96D UrbanEagle WFIR.AttackDelaySeconds: 0.4 -> 0.22857143",
    "  B5   rule[5] WEAP Starfield.esm:26D96D UrbanEagle WFIR.ShotsPerSecond: 2.5 -> 4.375",
    "  B6   rule[6] WEAP Starfield.esm:02D7F4 Lawgiver WFIR.AttackAnimationDuration: 1 -> 0.64",
    "  B6   rule[6] WEAP Starfield.esm:02D7F4 Lawgiver WFIR.AttackDelaySeconds: 1 -> 0.64",
    "  B6   rule[6] WEAP Starfield.esm:02D7F4 Lawgiver WFIR.ShotsPerSecond: 1 -> 1.5625",
    "  B7   rule[7] WEAP Starfield.esm:02EB36 Tombstone WDMG.BaseDamage: 19 -> 23.75",
    "  B7   rule[8] AMDL Starfield.esm:11287A Tombstone_FA_AM ANAM.RecoilDegreesPerShotX: 0.175 -> 0.2625",
    "  B7   rule[8] AMDL Starfield.esm:11287A Tombstone_FA_AM ANAM.RecoilDegreesPerShotY: 0.375 -> 0.5625",
    "  B9   rule[9] WEAP Starfield.esm:04716C Beowulf WAM2.AmmoCapacity: 30 -> 20",
    "  B9   rule[9] WEAP Starfield.esm:04716C Beowulf WAM2.AmmoList: \"0000E841\" -> \"0012F0CE\"",
    "  B9   rule[9] WEAP Starfield.esm:04716C Beowulf WAM2.AmmoType: \"0004AD3E\" -> \"002B5590\"",
    "  B11  rule[10] WEAP Starfield.esm:18DE2C DrumBeat WAM2.AmmoCapacity: 30 -> 18",
    "  B11  rule[10] WEAP Starfield.esm:18DE2C DrumBeat WDMG.BaseDamage: 8 -> 12",
    "  B11  rule[10] WEAP Starfield.esm:18DE2C DrumBeat WFIR.AttackAnimationDuration: 1.25 -> 1.6666666",
    "  B11  rule[10] WEAP Starfield.esm:18DE2C DrumBeat WFIR.AttackDelaySeconds: 0.0625 -> 0.083333336",
    "  B11  rule[10] WEAP Starfield.esm:18DE2C DrumBeat WFIR.ShotsPerSecond: 16 -> 12",
    "  B11  rule[11] AMDL Starfield.esm:179DBD Drumbeat_SA_AM ANAM.RecoilDegreesPerShotX: 0.185 -> 0.296",
    "  B11  rule[11] AMDL Starfield.esm:179DBD Drumbeat_SA_AM ANAM.RecoilDegreesPerShotY: 0.32 -> 0.51199996",
    "  B11  rule[12] AMDL Starfield.esm:2BF29F Drumbeat_FA_AM ANAM.RecoilDegreesPerShotX: 0.15 -> 0.24000001",
    "  B11  rule[12] AMDL Starfield.esm:2BF29F Drumbeat_FA_AM ANAM.RecoilDegreesPerShotY: 0.4 -> 0.64",
    "  B12  rule[13] WEAP Starfield.esm:26D960 Shotty WAM2.AmmoCapacity: 12 -> 6",
    "  B13  rule[14] WEAP Starfield.esm:0547A3 Breach WFIR.AttackAnimationDuration: 1 -> 0.6666667",
    "  B13  rule[14] WEAP Starfield.esm:0547A3 Breach WFIR.AttackDelaySeconds: 1 -> 0.6666667",
    "  B13  rule[14] WEAP Starfield.esm:0547A3 Breach WFIR.ShotsPerSecond: 1 -> 1.5",
    "  B14  rule[15] WEAP Starfield.esm:0546CD Microgun WDMG.BaseDamage: 9 -> 13.5",
    "  B14  rule[15] WEAP Starfield.esm:0546CD Microgun WFIR.FireSeconds: 1 -> 0.1",
];

/// Every record of `sig` in `Starfield.esm` whose EditorID is one of `edids`, as owned
/// records read byte for byte at the offsets `query` reports.
fn copy_records(real: &Path, sig: &str, edids: &[&str]) -> Vec<Record<'static>> {
    let lo = json!({"plugins": ["Starfield.esm"], "data_dir": real.display().to_string()});
    let clause: Vec<String> = edids.iter().map(|e| format!("EDID == \"{e}\"")).collect();
    let mut args = lo.clone();
    args["type"] = json!(sig);
    args["where"] = json!(clause.join(" or "));
    args["limit"] = json!(100);
    let q = execute("query", args).unwrap();
    let results = q["data"]["results"].as_array().unwrap();
    let mut found: Vec<&str> = results
        .iter()
        .map(|r| r["editor_id"].as_str().unwrap())
        .collect();
    found.sort_unstable();
    let mut want = edids.to_vec();
    want.sort_unstable();
    assert_eq!(found, want, "{sig}: {:#}", q["data"]);
    let mut file = std::fs::File::open(real.join("Starfield.esm")).unwrap();
    results
        .iter()
        .map(|r| {
            assert_eq!(r["plugin"], json!("Starfield.esm"));
            let offset = r["offset"].as_u64().unwrap();
            let mut head = [0u8; 24];
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.read_exact(&mut head).unwrap();
            let size = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
            let mut bytes = vec![0u8; 24 + size];
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.read_exact(&mut bytes).unwrap();
            match Entry::parse_one(&bytes, offset, 1, &ParseOptions::default()).unwrap() {
                Entry::Record(record) => record.into_owned(),
                Entry::Group(_) => panic!("{sig} at {offset} is a group"),
            }
        })
        .collect()
}

#[test]
fn the_overhaul_spec_plans_and_builds_on_the_real_records() {
    let Some(real) = data_dir() else { return };
    let tmp = std::env::temp_dir().join(format!("kitkraft-overhaul-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let data = tmp.join("Data");
    std::fs::create_dir_all(&data).unwrap();

    // The fixture: a localized master named Starfield.esm holding the real records, and the
    // English string tables as loose files (Strings\starfield_en.*). The tables come first:
    // `ba2.extract` never writes into a folder that holds a game master.
    execute(
        "ba2.extract",
        json!({
            "archive": real.join("Starfield - Localization.ba2").display().to_string(),
            "out_dir": data.display().to_string(),
            "pattern": "^strings/starfield_en\\.",
        }),
    )
    .unwrap();
    let mut spec = HeaderSpec::new(Vec::new());
    spec.flags = FLAG_MASTER | FLAG_LOCALIZED;
    let mut plugin = Plugin::create(&spec);
    let mut count = 0;
    for (sig, edids) in RECORDS {
        for record in copy_records(&real, sig, edids) {
            let top = GroupHeader::top(Signature(sig.as_bytes().try_into().unwrap()));
            plugin.upsert(&[top], record).unwrap();
            count += 1;
        }
    }
    assert_eq!(count, 33);
    std::fs::write(data.join("Starfield.esm"), plugin.to_bytes().unwrap()).unwrap();

    // The committed spec, with its load order pointed at the fixture.
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/overhaul/overhaul.toml"
    ))
    .unwrap();
    let profile = "profile = 'C:\\Modding\\MO2-Starfield\\profiles\\KitKraft Dev'\n";
    assert!(text.contains(profile), "the spec's load order changed");
    let text = text.replace(
        profile,
        &format!(
            "plugins = [\"Starfield.esm\"]\ndata_dir = '{}'\nno_cache = true\n",
            data.display()
        ),
    );
    let spec_path = tmp.join("overhaul.toml");
    std::fs::write(&spec_path, text).unwrap();
    let out = tmp.join("out").join("KitKraftGunOverhaul.esm");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let run = |extra: Value| {
        let mut args = json!({
            "spec": spec_path.display().to_string(),
            "out": out.display().to_string(),
        });
        for (k, v) in extra.as_object().unwrap() {
            args[k] = v.clone();
        }
        execute("build", args)
    };

    // The plan: every change, its row and its values.
    let plan = run(json!({"plan": true})).unwrap();
    let d = &plan["data"];
    assert!(!out.exists(), "a plan writes nothing");
    assert_eq!(d["passed"], json!(true), "{d:#}");
    assert_eq!(d["masters"], json!(["Starfield.esm"]));
    assert_eq!(d["changed_fields"], json!(42));
    let summary = d["summary"].as_str().unwrap();
    let lines: Vec<&str> = summary
        .lines()
        .skip_while(|l| !l.starts_with("changes ("))
        .skip(1)
        .collect();
    assert_eq!(lines, CHANGES, "{summary}");
    // Every change traces to exactly one brief row (the traceability rule).
    for c in d["changes"].as_array().unwrap() {
        assert_eq!(c["brief"].as_array().unwrap().len(), 1, "{c:#}");
        assert_eq!(c["changed"], json!(true), "{c:#}");
    }
    // The classes that only list the open rows' targets match them all.
    let matched = |name: &str| {
        d["classes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["class"] == json!(name))
            .map(|c| c["matched"].clone())
            .unwrap()
    };
    assert_eq!(matched("b8_automatic_receivers"), json!(6));
    assert_eq!(matched("b10_hard_target_large_mag"), json!(1));
    assert_eq!(matched("b12_shotty_magazines"), json!(3));
    assert!(d["warnings"].as_array().unwrap().is_empty(), "{d:#}");

    // The build: 17 overrides, validated, identical twice, and round-trips.
    let built = run(json!({})).unwrap();
    assert_eq!(built["data"]["passed"], json!(true), "{:#}", built["data"]);
    assert_eq!(built["data"]["changed_fields"], json!(42));
    assert_eq!(built["data"]["records"].as_array().unwrap().len(), 17);
    let first = std::fs::read(&out).unwrap();
    run(json!({"overwrite": true})).unwrap();
    assert_eq!(first, std::fs::read(&out).unwrap(), "two builds differ");
    let rt = execute(
        "roundtrip",
        json!({"plugin": out.display().to_string(), "verify": true}),
    )
    .unwrap();
    assert_eq!(rt["data"]["identical"], json!(true), "{:#}", rt["data"]);
    let _ = std::fs::remove_dir_all(&tmp);
}
