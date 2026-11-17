//! Quest scripts and stage fragments through `build` (the self-reporting quest of
//! `docs/bench/verify-mod.md`): a `[[new]]` `QUST` gets a quest script in its `VMAD`'s
//! script list and a fragment script with stage fragments in its fragment block; the built
//! plugin roundtrips byte for byte and reads back as written. A quest with only a quest
//! script gets no fragment block, the form of 209 official quests
//! (`tests/quest_vmad_corpus.rs`). A script on a quest alias (declared by the same
//! `[[new]]`'s `fields`) goes into that alias's entry of the block, in the official entry
//! layout; one on an alias the quest does not declare is refused (`quest.alias-missing`).

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::index::{index_bytes, read_indexed, IndexOptions};
use kitkraft_esm::{DecodeLimits, Signature};
use serde_json::{json, Value};

use super::place_tests::world_master;
use super::scripts_tests::{pex_with, pex_with_functions};
use crate::edit::patch_tests::dir;
use crate::edit::vmad;
use crate::execute;

/// `World.esm` in `<d>/Data`; `KK:QuestReport extends Quest` (a `Count` property) and
/// `Fragments:Quests:QF_KKQuest extends Quest` (a `Marker` property) in `<d>/Scripts`.
fn setup(name: &str) -> (PathBuf, PathBuf) {
    let d = dir(name);
    let data = world_master(&d);
    let kk = d.join("Scripts").join("KK");
    std::fs::create_dir_all(&kk).unwrap();
    std::fs::write(
        kk.join("QuestReport.pex"),
        pex_with("KK:QuestReport", "Quest", &[("Count", "Int")]),
    )
    .unwrap();
    let frag = d.join("Scripts").join("Fragments").join("Quests");
    std::fs::create_dir_all(&frag).unwrap();
    std::fs::write(
        frag.join("QF_KKQuest.pex"),
        pex_with_functions(
            "Fragments:Quests:QF_KKQuest",
            "Quest",
            &[("Marker", "ObjectReference")],
            &[
                "Report",
                "Fragment_Stage_0010_Item_00",
                "Fragment_Stage_0020_Item_00",
            ],
        ),
    )
    .unwrap();
    (d, data)
}

/// Stage 10 with one log entry (`QSDT` opens it; `new` adds the required reward count and
/// end, `QSRD`): a stage fragment runs from a log entry, so `validate` refuses one without
/// (`quest.fragment-stage`). Only one stage: a second instance of the repeated `Stage`
/// group cannot be written through `fields` yet (`new.group-incomplete`).
const STAGES: &str = r#"{ "INDX.StageIndex" = 10, QSDT = 0 }"#;

fn spec(data: &Path, quest_scripts: &str) -> String {
    spec_with(data, STAGES, quest_scripts)
}

fn spec_with(data: &Path, fields: &str, quest_scripts: &str) -> String {
    format!(
        "version = 3\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n\
[[new]]\nname = \"quest\"\ntype = \"QUST\"\nedid = \"KK_Quest\"\nmasters = [\"World.esm\"]\n\
fields = {fields}\nscripts = {quest_scripts}\n",
        data.display()
    )
}

/// The record `form_id` of the plugin at `path`: its `VMAD` payload.
fn vmad_bytes(path: &Path, form_id: u32) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let loc = index.records.iter().find(|r| r.form_id == form_id).unwrap();
    let record = read_indexed(
        &mut std::io::Cursor::new(&bytes),
        loc,
        Some(&DecodeLimits::default()),
    )
    .unwrap();
    record
        .subrecords()
        .unwrap()
        .iter()
        .find(|s| s.signature() == Signature(*b"VMAD"))
        .unwrap()
        .data()
        .to_vec()
}

fn build(d: &Path, spec_text: &str, out: &str) -> Result<Value, String> {
    let spec = d.join(format!("{out}.toml"));
    std::fs::write(&spec, spec_text).unwrap();
    execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": d.join(out).display().to_string()}),
    )
    .map_err(|e| e.to_string())
}

fn new_form_id(r: &Value) -> u32 {
    let f = r["data"]["new_forms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["editor_id"] == json!("KK_Quest"))
        .unwrap_or_else(|| panic!("{:#}", r["data"]["new_forms"]));
    u32::from_str_radix(f["form_id"].as_str().unwrap(), 16).unwrap()
}

#[test]
fn a_quest_gets_a_quest_script_and_stage_fragments() {
    let (d, data) = setup("quest-fragments");
    let text = spec(
        &data,
        "[\n  { name = \"KK:QuestReport\", properties = { Count = 3 } },\n  \
{ name = \"Fragments:Quests:QF_KKQuest\", properties = { Marker = \"KC_Crate\" }, \
stages = [{ stage = 10 }] },\n]",
    );
    let r = build(&d, &text, "Quest.esp").unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let scripts = r["data"]["scripts"].as_array().unwrap();
    let frag = scripts
        .iter()
        .find(|s| s["script"] == json!("Fragments:Quests:QF_KKQuest"))
        .unwrap();
    assert_eq!(
        frag["stages"][0]["function"],
        json!("Fragment_Stage_0010_Item_00")
    );

    let out = d.join("Quest.esp");
    let bytes = std::fs::read(&out).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let raw = vmad_bytes(&out, new_form_id(&r));
    let v = vmad::decode(&raw).unwrap();
    // The quest script is in the script list; the fragment script is not.
    assert_eq!(v.scripts.len(), 1);
    assert_eq!(v.scripts[0].name_text(), "KK:QuestReport");
    let q = vmad::decode_quest_fragments(&v.rest, v.object_format).unwrap();
    assert_eq!(q.bind_version, 3);
    let fs = q.script.as_ref().unwrap();
    assert_eq!(fs.name_text(), "Fragments:Quests:QF_KKQuest");
    assert_eq!(fs.flags, 0);
    assert_eq!(fs.properties[0].name, b"Marker");
    assert!(matches!(fs.properties[0].value, vmad::Value::Object(o) if o.form_id != 0));
    // The default function name for stage 10, log entry 0.
    let stages: Vec<(u32, u32, String, String)> = q
        .fragments
        .iter()
        .map(|f| {
            (
                f.stage,
                f.stage_index,
                String::from_utf8_lossy(&f.script_name).into_owned(),
                String::from_utf8_lossy(&f.fragment_name).into_owned(),
            )
        })
        .collect();
    assert_eq!(
        stages,
        [(
            10,
            0,
            "Fragments:Quests:QF_KKQuest".to_string(),
            "Fragment_Stage_0010_Item_00".to_string()
        )]
    );
    assert!(q.fragments.iter().all(|f| f.unknown == 1));
    assert!(q.aliases.is_empty());
    // The whole payload re-encodes to the bytes written.
    let mut again = vmad::encode(&vmad::Vmad {
        rest: Vec::new(),
        ..v.clone()
    })
    .unwrap();
    again.extend(vmad::encode_quest_fragments(&q, v.object_format).unwrap());
    assert_eq!(again, raw);

    // The command's own roundtrip agrees.
    let rt = execute(
        "roundtrip",
        json!({"plugin": out.display().to_string(), "verify": true}),
    )
    .unwrap();
    assert_eq!(rt["data"]["identical"], json!(true), "{:#}", rt["data"]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_quest_script_alone_writes_no_fragment_block() {
    let (d, data) = setup("quest-script-only");
    let text = spec(
        &data,
        "[{ name = \"KK:QuestReport\", properties = { Count = 1 } }]",
    );
    let r = build(&d, &text, "Only.esp").unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let v = vmad::decode(&vmad_bytes(&d.join("Only.esp"), new_form_id(&r))).unwrap();
    assert_eq!(v.scripts.len(), 1);
    assert!(v.rest.is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn stage_fragments_are_refused_off_a_quest_and_malformed() {
    let (d, data) = setup("quest-fragments-bad");
    // On a placed reference: not a quest.
    let text = format!(
        "version = 3\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n\
[[place]]\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\nposition = [0, 0, 0]\n\
scripts = [{{ name = \"Fragments:Quests:QF_KKQuest\", stages = [{{ stage = 10 }}] }}]\n",
        data.display()
    );
    let e = build(&d, &text, "Ref.esp").unwrap_err();
    assert!(e.contains("stage fragments attach only to a QUST"), "{e}");
    // A stage out of range, a twice-named stage, an unknown key, an empty list.
    for (stages, why) in [
        ("[{ stage = 70000 }]", "must be an integer 0 to 65535"),
        (
            "[{ stage = 10 }, { stage = 10, index = 0 }]",
            "same stage and index twice",
        ),
        (
            "[{ stage = 10, item = 1 }]",
            "unknown key; allowed: stage, index, function",
        ),
        ("[]", "non-empty array"),
    ] {
        let text = spec(
            &data,
            &format!("[{{ name = \"Fragments:Quests:QF_KKQuest\", stages = {stages} }}]"),
        );
        let e = build(&d, &text, "Bad.esp").unwrap_err();
        assert!(e.contains(why), "{stages}: {e}");
    }
    // Two fragment scripts on one quest.
    let two = spec(
        &data,
        "[{ name = \"Fragments:Quests:QF_KKQuest\", stages = [{ stage = 10 }] },\n  \
{ name = \"KK:QuestReport\", stages = [{ stage = 20 }] }]",
    );
    let e = build(&d, &two, "Two.esp").unwrap_err();
    assert!(e.contains("a quest holds one"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `setup` plus `KK:AliasReport extends ReferenceAlias` (a `Count` and a `Marker`
/// property).
fn setup_alias(name: &str) -> (PathBuf, PathBuf) {
    let (d, data) = setup(name);
    std::fs::write(
        d.join("Scripts").join("KK").join("AliasReport.pex"),
        pex_with(
            "KK:AliasReport",
            "ReferenceAlias",
            &[("Count", "Int"), ("Marker", "ObjectReference")],
        ),
    )
    .unwrap();
    (d, data)
}

/// A spec whose `[[new]]` quest declares reference alias 0 `Target` through `fields`.
fn alias_spec(data: &Path, quest_scripts: &str) -> String {
    format!(
        "version = 3\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n\
[[new]]\nname = \"quest\"\ntype = \"QUST\"\nedid = \"KK_Quest\"\nmasters = [\"World.esm\"]\n\
fields = {{ ANAM = 1, ALST = 0, ALID = \"Target\", ALED = \"\" }}\n\
scripts = {quest_scripts}\n",
        data.display()
    )
}

#[test]
fn a_quest_alias_gets_a_script_with_properties() {
    let (d, data) = setup_alias("quest-alias-script");
    let text = alias_spec(
        &data,
        "[\n  { name = \"KK:QuestReport\", properties = { Count = 1 } },\n  \
{ name = \"KK:AliasReport\", alias = \"target\", properties = { Count = 7, Marker = \"KC_Crate\" } },\n]",
    );
    let r = build(&d, &text, "Alias.esp").unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let rep = r["data"]["scripts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["script"] == json!("KK:AliasReport"))
        .unwrap();
    assert_eq!(rep["alias"], json!("target"));
    assert!(r["limitations"].as_array().unwrap().iter().any(|l| l
        .as_str()
        .unwrap()
        .contains("not checked to extend ReferenceAlias")));

    let out = d.join("Alias.esp");
    let bytes = std::fs::read(&out).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let quest = new_form_id(&r);
    let raw = vmad_bytes(&out, quest);
    let v = vmad::decode(&raw).unwrap();
    // The quest script in the list; the alias script only in the alias entry.
    assert_eq!(v.scripts.len(), 1);
    assert_eq!(v.scripts[0].name_text(), "KK:QuestReport");
    let q = vmad::decode_quest_fragments(&v.rest, v.object_format).unwrap();
    assert!(q.script.is_none() && q.fragments.is_empty());
    assert_eq!(q.aliases.len(), 1);
    let e = &q.aliases[0];
    // The official entry: the quest's own FormID, alias 0, unused 0, version 6, format 2.
    assert_eq!(
        (e.object.form_id, e.object.alias, e.object.unused),
        (quest, 0, 0)
    );
    assert_eq!((e.version, e.object_format), (6, 2));
    assert_eq!(e.scripts.len(), 1);
    let s = &e.scripts[0];
    assert_eq!((s.name_text().as_str(), s.flags), ("KK:AliasReport", 0));
    assert_eq!(s.properties[0].value, vmad::Value::Int(7));
    assert!(
        matches!(s.properties[1].value, vmad::Value::Object(o) if o.form_id != 0 && o.alias == -1)
    );
    // The whole payload re-encodes to the bytes written.
    let mut again = vmad::encode(&vmad::Vmad {
        rest: Vec::new(),
        ..v.clone()
    })
    .unwrap();
    again.extend(vmad::encode_quest_fragments(&q, v.object_format).unwrap());
    assert_eq!(again, raw);
    let rt = execute(
        "roundtrip",
        json!({"plugin": out.display().to_string(), "verify": true}),
    )
    .unwrap();
    assert_eq!(rt["data"]["identical"], json!(true), "{:#}", rt["data"]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_script_on_an_undeclared_alias_is_refused() {
    let (d, data) = setup_alias("quest-alias-missing");
    for (alias, why) in [
        (
            "3",
            "quest.alias-missing: the quest has no alias 3 (its aliases: 0 `Target` (reference))",
        ),
        (
            "\"Nobody\"",
            "quest.alias-missing: the quest has no alias `Nobody`",
        ),
        ("-1", "an alias ID must be an integer 0 to 32767"),
        (
            "true",
            "must be an alias ID (an integer) or an alias name (a string)",
        ),
        (
            "0, stages = [{ stage = 10 }]",
            "takes `alias` or `stages`, not both",
        ),
    ] {
        let text = alias_spec(
            &data,
            &format!("[{{ name = \"KK:AliasReport\", alias = {alias} }}]"),
        );
        let e = build(&d, &text, "Missing.esp").unwrap_err();
        assert!(e.contains(why), "{alias}: {e}");
        assert!(!d.join("Missing.esp").exists());
    }
    // A quest with no aliases at all.
    let text = spec(&data, "[{ name = \"KK:AliasReport\", alias = 0 }]");
    let e = build(&d, &text, "None.esp").unwrap_err();
    assert!(e.contains("(its aliases: none)"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_quest_script_property_holds_one_of_its_aliases() {
    let (d, data) = setup_alias("quest-alias-value");
    // KK:QuestReport with a ReferenceAlias property and an array of them.
    std::fs::write(
        d.join("Scripts").join("KK").join("QuestReport.pex"),
        pex_with(
            "KK:QuestReport",
            "Quest",
            &[
                ("Count", "Int"),
                ("TargetAlias", "ReferenceAlias"),
                ("Others", "ReferenceAlias[]"),
            ],
        ),
    )
    .unwrap();
    let text = alias_spec(
        &data,
        "[{ name = \"KK:QuestReport\", properties = { TargetAlias = { alias = \"Target\" }, \
Others = [{ alias = 0 }, { quest = \"KC_Crate\", alias = 3 }] } }]",
    );
    let r = build(&d, &text, "Value.esp").unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let out = d.join("Value.esp");
    let bytes = std::fs::read(&out).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let quest = new_form_id(&r);
    let v = vmad::decode(&vmad_bytes(&out, quest)).unwrap();
    let objs: Vec<(String, bool, i16)> = vmad::objects(&v)
        .into_iter()
        .map(|(p, o)| (p, o.form_id == quest, o.alias))
        .collect();
    assert_eq!(
        objs,
        [
            // Properties in name order (the spec's table is read as a map).
            ("KK:QuestReport.Others[0]".to_string(), true, 0),
            ("KK:QuestReport.Others[1]".to_string(), false, 3),
            ("KK:QuestReport.TargetAlias".to_string(), true, 0),
        ]
    );
    // An alias the quest does not declare, and a malformed value, are refused.
    for (value, why) in [
        (
            "{ alias = 4 }",
            "quest.alias-missing: the quest has no alias 4 (its aliases: 0 `Target` (reference))",
        ),
        ("{ quest = \"KC_Crate\" }", "needs `alias`"),
        ("{ alias = 0, ref = 1 }", "unknown key `ref`"),
    ] {
        let text = alias_spec(
            &data,
            &format!("[{{ name = \"KK:QuestReport\", properties = {{ TargetAlias = {value} }} }}]"),
        );
        let e = build(&d, &text, "BadValue.esp").unwrap_err();
        assert!(e.contains(why), "{value}: {e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_stage_fragment_needs_its_stage_log_entry_and_function() {
    let (d, data) = setup("quest-fragment-checks");
    // A fragment script that extends nothing, so its compiled functions are the whole
    // chain: a missing function is known to be missing.
    std::fs::write(
        d.join("Scripts")
            .join("Fragments")
            .join("Quests")
            .join("QF_Bare.pex"),
        pex_with_functions(
            "Fragments:Quests:QF_Bare",
            "",
            &[],
            &["Fragment_Stage_0010_Item_00"],
        ),
    )
    .unwrap();
    let frag = |script: &str, stage: &str| {
        format!("[{{ name = \"Fragments:Quests:{script}\", stages = [{stage}] }}]")
    };
    for (fields, scripts, why) in [
        (
            STAGES,
            frag("QF_KKQuest", "{ stage = 20 }"),
            "quest.fragment-stage QUST:01001000: stage fragment `Fragment_Stage_0020_Item_00` (stage 20, log entry 0): the quest has no stage 20; the game does not run it",
        ),
        (
            r#"{ "INDX.StageIndex" = 10 }"#,
            frag("QF_KKQuest", "{ stage = 10 }"),
            "stage 10 has no log entry (a fragment runs from a log entry; give the stage one, QSDT)",
        ),
        (
            STAGES,
            frag("QF_KKQuest", "{ stage = 10, index = 1 }"),
            "stage 10 has 1 log entry (0 to 0), not 1",
        ),
        (
            STAGES,
            frag("QF_Bare", "{ stage = 10, function = \"Missing\" }"),
            "quest.fragment-function QUST:01001000: stage 10 log entry 0: fragment `Missing` is not a function of `Fragments:Quests:QF_Bare` or its parents",
        ),
    ] {
        let e = build(&d, &spec_with(&data, fields, &scripts), "Bad.esp").unwrap_err();
        assert!(e.contains("the built plugin fails validate, so nothing was written"), "{e}");
        assert!(e.contains(why), "{scripts}: {e}");
        assert!(!d.join("Bad.esp").exists());
    }
    // A function of a script whose parent is not found as a .pex cannot be told missing:
    // written, with a warning that it was not checked.
    let r = build(
        &d,
        &spec(
            &data,
            &frag("QF_KKQuest", "{ stage = 10, function = \"Missing\" }"),
        ),
        "Unchecked.esp",
    )
    .unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let text = r.to_string();
    assert!(
        text.contains("quest.fragment-function") && text.contains("was not checked"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// The subrecords of the record `form_id` of the plugin at `path`: signature and payload.
fn subrecords_of(path: &Path, form_id: u32) -> Vec<(String, Vec<u8>)> {
    let bytes = std::fs::read(path).unwrap();
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let loc = index.records.iter().find(|r| r.form_id == form_id).unwrap();
    let record = read_indexed(
        &mut std::io::Cursor::new(&bytes),
        loc,
        Some(&DecodeLimits::default()),
    )
    .unwrap();
    record
        .subrecords()
        .unwrap()
        .iter()
        .map(|s| (s.signature().to_string(), s.data().to_vec()))
        .collect()
}

#[test]
fn an_alias_takes_its_flags_from_fields() {
    let (d, data) = setup_alias("quest-alias-fnam");
    // FNAM is an objective's flags and a reference alias's: with ALST given and no QOBJ,
    // it is the alias's. Starfield.esm QUST 000D40ED's alias 1 (`BehaviorActor`) stores
    // MatchingRefActorsOnly as 00 00 04 00 (kit inspect 0D40ED --plugin Starfield.esm).
    let fields =
        r#"{ ANAM = 1, ALST = 0, ALID = "Target", FNAM = "MatchingRefActorsOnly", ALED = "" }"#;
    let r = build(
        &d,
        &spec_with(&data, fields, "[{ name = \"KK:AliasReport\", alias = 0 }]"),
        "Flags.esp",
    )
    .unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let out = d.join("Flags.esp");
    let subs = subrecords_of(&out, new_form_id(&r));
    let sigs: Vec<&str> = subs.iter().map(|(s, _)| s.as_str()).collect();
    let at = sigs.iter().position(|s| *s == "ALST").unwrap();
    assert_eq!(sigs[at..at + 3], ["ALST", "ALID", "FNAM"], "{sigs:?}");
    assert_eq!(subs[at + 2].1, [0x00, 0x00, 0x04, 0x00]);
    assert!(!sigs.contains(&"QOBJ"));
    assert!(
        roundtrip_bytes(&std::fs::read(&out).unwrap(), &RoundtripOptions::default())
            .unwrap()
            .identical()
    );
    let _ = std::fs::remove_dir_all(&d);
}
