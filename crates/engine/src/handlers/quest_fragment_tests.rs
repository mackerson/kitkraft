//! Quest scripts and stage fragments through `build` (the self-reporting quest of
//! `docs/bench/verify-mod.md`): a `[[new]]` `QUST` gets a quest script in its `VMAD`'s
//! script list and a fragment script with stage fragments in its fragment block; the built
//! plugin roundtrips byte for byte and reads back as written. A quest with only a quest
//! script gets no fragment block, the form of 209 official quests
//! (`tests/quest_vmad_corpus.rs`).

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::index::{index_bytes, read_indexed, IndexOptions};
use kitkraft_esm::{DecodeLimits, Signature};
use serde_json::{json, Value};

use super::place_tests::world_master;
use super::scripts_tests::pex_with;
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
        pex_with(
            "Fragments:Quests:QF_KKQuest",
            "Quest",
            &[("Marker", "ObjectReference")],
        ),
    )
    .unwrap();
    (d, data)
}

fn spec(data: &Path, quest_scripts: &str) -> String {
    format!(
        "version = 3\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n\
[[new]]\nname = \"quest\"\ntype = \"QUST\"\nedid = \"KK_Quest\"\nmasters = [\"World.esm\"]\n\
scripts = {quest_scripts}\n",
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
stages = [{ stage = 10, function = \"Report\" }, { stage = 20 }] },\n]",
    );
    let r = build(&d, &text, "Quest.esp").unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    let scripts = r["data"]["scripts"].as_array().unwrap();
    let frag = scripts
        .iter()
        .find(|s| s["script"] == json!("Fragments:Quests:QF_KKQuest"))
        .unwrap();
    assert_eq!(
        frag["stages"][1]["function"],
        json!("Fragment_Stage_0020_Item_00")
    );
    assert!(r["limitations"].as_array().unwrap().iter().any(|l| l
        .as_str()
        .unwrap()
        .contains("stage fragment's function is not checked")));

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
    // The order given; the default function name for stage 20.
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
        [
            (
                10,
                0,
                "Fragments:Quests:QF_KKQuest".to_string(),
                "Report".to_string()
            ),
            (
                20,
                0,
                "Fragments:Quests:QF_KKQuest".to_string(),
                "Fragment_Stage_0020_Item_00".to_string()
            ),
        ]
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
