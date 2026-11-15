//! Dialogue topics and responses under their parents (KK-275, KK-276;
//! `docs/design/dialogue.md`) through `execute`: `new` and `clone` of a `DIAL` with a
//! parent quest and of an `INFO` with a parent topic, into a patch whose quest is a
//! master's, an earlier write's or new in the same build; the group path, `QNAM`, the
//! topic's `TIFL`/`TIFC`, `ONAM`, validation, the byte roundtrip and the refusals.
//!
//! The fixture `Quest.esm` (a master of `Source.esm`, the `set` tests' load order) is
//! written with KitKraft itself: `new QUST`, then a `DIAL` under it and an `INFO` under
//! that, each with `active_file` so the second and third name forms of the file they go
//! into.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::{Entry, Plugin, Record};
use serde_json::{json, Value};

use super::set_tests::data;
use crate::edit::patch_tests::dir;
use crate::execute;

/// `execute(command)` with the load order `plugins` in `data`.
pub(super) fn call(
    command: &str,
    data: &Path,
    plugins: &[&str],
    into: &Path,
    extra: Value,
) -> Result<Value, crate::EngineError> {
    let mut a = json!({
        "into": into.display().to_string(),
        "plugins": plugins, "data_dir": data.display().to_string(), "no_cache": true,
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    execute(command, a)
}

pub(super) fn code(err: &crate::EngineError) -> String {
    let text = err.to_string();
    text.split(": ")
        .find(|p| p.starts_with("new.") || p.starts_with("clone."))
        .unwrap_or(&text)
        .to_string()
}

/// The data dir of the `set` tests plus `Quest.esm`: `KC_Quest` (QUST) with the topic
/// `KC_Topic` (DIAL) and its response `KC_Response` (INFO).
pub(super) fn dialogue_master(d: &Path) -> PathBuf {
    let data = data(d);
    let quest = data.join("Quest.esm");
    call(
        "new",
        &data,
        &["Source.esm"],
        &quest,
        json!({"type": "QUST", "edid": "KC_Quest", "masters": ["Source.esm"]}),
    )
    .unwrap();
    let own = |extra: Value| {
        let mut a = extra;
        a["active_file"] = json!(true);
        call(
            "new",
            &data,
            &["Source.esm", quest.to_str().unwrap()],
            &quest,
            a,
        )
        .unwrap()
    };
    own(json!({"type": "DIAL", "edid": "KC_Topic", "parent": "KC_Quest"}));
    own(json!({"type": "INFO", "edid": "KC_Response", "parent": "KC_Topic"}));
    data
}

/// Every record of the plugin with its group path `(type, label)`, in file order.
pub(super) fn records(bytes: &[u8]) -> Vec<Row> {
    fn walk(entries: &[Entry<'_>], path: &mut Vec<(i32, u32)>, out: &mut Vec<Row>) {
        for e in entries {
            match e {
                Entry::Record(r) => out.push((
                    path.clone(),
                    r.header.signature.to_string(),
                    r.header.form_id,
                    subs(r),
                )),
                Entry::Group(g) => {
                    path.push((g.header.group_type, g.header.label_u32()));
                    walk(g.children(), path, out);
                    path.pop();
                }
            }
        }
    }
    let plugin = Plugin::parse(bytes).unwrap();
    let mut out = Vec::new();
    walk(plugin.entries(), &mut Vec::new(), &mut out);
    out
}

fn subs(r: &Record<'_>) -> Vec<([u8; 4], Vec<u8>)> {
    r.subrecords()
        .unwrap()
        .iter()
        .map(|s| (s.signature().0, s.data().to_vec()))
        .collect()
}

pub(super) fn sub<'a>(subs: &'a [([u8; 4], Vec<u8>)], sig: &[u8; 4]) -> Option<&'a [u8]> {
    subs.iter()
        .find(|(s, _)| s == sig)
        .map(|(_, d)| d.as_slice())
}

pub(super) fn u32_of(b: Option<&[u8]>) -> u32 {
    u32::from_le_bytes(b.unwrap().try_into().unwrap())
}

pub(super) fn ids_of(b: Option<&[u8]>) -> Vec<u32> {
    b.unwrap()
        .chunks(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// The written plugin roundtrips byte for byte and validates with no error.
pub(super) fn sound(path: &Path, data: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let v = super::authoring_tests::validate_at(path, data);
    assert_eq!(v["errors"], json!(0), "{v:#}");
    bytes
}

/// `Quest.esm` FormIDs as stored there and in a patch whose masters are `Source.esm`,
/// `Quest.esm` (the quest's plugin is master 1 in both).
pub(super) const QUEST: u32 = 0x0100_1000;
pub(super) const TOPIC: u32 = 0x0100_1001;
pub(super) const RESPONSE: u32 = 0x0100_1002;

/// One record of a written plugin: its group path, signature, FormID and subrecords.
pub(super) type Row = (Vec<(i32, u32)>, String, u32, Vec<([u8; 4], Vec<u8>)>);

pub(super) fn find<'a>(rows: &'a [Row], sig: &str, id: u32) -> &'a Row {
    rows.iter()
        .find(|(_, s, f, _)| s == sig && *f == id)
        .unwrap_or_else(|| {
            let seen: Vec<_> = rows.iter().map(|r| (&r.0, &r.1, r.2)).collect();
            panic!("no {sig} {id:08X} in {seen:#?}")
        })
}

pub(super) const QUST_TOP: (i32, u32) = (0, u32::from_le_bytes(*b"QUST"));

/// The fixture itself: written by `new` with `active_file`, the topic in its quest's type
/// 10 group with `QNAM` the quest, the response in the topic's type 7 group, listed in
/// `TIFL` and counted in `TIFC`.
#[test]
fn the_dialogue_master_holds_a_quest_its_topic_and_a_response() {
    let d = dir("dialogue-master");
    let data = dialogue_master(&d);
    let bytes = sound(&data.join("Quest.esm"), &data);
    let rows = records(&bytes);
    assert_eq!(find(&rows, "QUST", QUEST).0, vec![QUST_TOP]);
    let topic = find(&rows, "DIAL", TOPIC);
    assert_eq!(topic.0, vec![QUST_TOP, (10, QUEST)]);
    assert_eq!(u32_of(sub(&topic.3, b"QNAM")), QUEST);
    assert_eq!(ids_of(sub(&topic.3, b"TIFL")), vec![RESPONSE]);
    assert_eq!(u32_of(sub(&topic.3, b"TIFC")), 1);
    let response = find(&rows, "INFO", RESPONSE);
    assert_eq!(response.0, vec![QUST_TOP, (10, QUEST), (7, TOPIC)]);
    // File order: the quest, then its children group with the topic, then the topic's.
    let order: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(order, ["QUST", "DIAL", "INFO"]);
    let _ = std::fs::remove_dir_all(&d);
}

/// KK-275: a new topic under a master's quest. The quest is carried as an unedited
/// override (the official overrides of a master's quest children show it so), the topic
/// goes into the type 10 group labelled with the quest's FormID with `QNAM` the quest and
/// no responses; the plugin validates and roundtrips. A second topic joins the same group,
/// by FormID, and the quest is not written twice.
#[test]
fn a_new_topic_goes_into_its_master_quests_children_group() {
    let d = dir("dialogue-new-topic");
    let data = dialogue_master(&d);
    let into = d.join("New.esp");
    let lo = ["Source.esm", "Quest.esm"];
    let r = call(
        "new",
        &data,
        &lo,
        &into,
        json!({"type": "DIAL", "edid": "KC_NewTopic", "parent": "Quest.esm:001000",
               "fields": {"PNAM": 50.0, "DATA.Category": 7, "SNAM": "CUST"}}),
    )
    .unwrap();
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    assert_eq!(r["data"]["new_forms"][0]["form_id"], json!("02001000"));
    assert_eq!(r["data"]["masters"], json!(["Source.esm", "Quest.esm"]));
    let notes = r["limitations"].to_string();
    assert!(notes.contains("carried into the patch"), "{notes}");
    let bytes = sound(&into, &data);
    let rows = records(&bytes);
    assert_eq!(find(&rows, "QUST", QUEST).0, vec![QUST_TOP]);
    let topic = find(&rows, "DIAL", 0x0200_1000);
    assert_eq!(topic.0, vec![QUST_TOP, (10, QUEST)]);
    assert_eq!(u32_of(sub(&topic.3, b"QNAM")), QUEST);
    assert_eq!(sub(&topic.3, b"TIFL"), Some(&[][..]));
    assert_eq!(u32_of(sub(&topic.3, b"TIFC")), 0);
    assert_eq!(sub(&topic.3, b"SNAM"), Some(&b"CUST"[..]));
    // Read back through the load order: the topic is a form of New.esp.
    let i = execute(
        "inspect",
        json!({"form": "KC_NewTopic",
               "plugins": ["Source.esm", "Quest.esm", into.display().to_string()],
               "data_dir": data.display().to_string(), "no_cache": true}),
    )
    .unwrap();
    assert!(i["data"].to_string().contains("DIAL"), "{:#}", i["data"]);
    // A second topic, into the patch as written: the quest stays once, both topics in one
    // children group, by FormID.
    call(
        "new",
        &data,
        &lo,
        &into,
        json!({"type": "DIAL", "edid": "KC_NewTopic2", "parent": "KC_Quest"}),
    )
    .unwrap();
    let bytes = sound(&into, &data);
    let rows = records(&bytes);
    assert_eq!(rows.iter().filter(|r| r.1 == "QUST").count(), 1);
    let topics: Vec<(Vec<(i32, u32)>, u32)> = rows
        .iter()
        .filter(|r| r.1 == "DIAL")
        .map(|r| (r.0.clone(), r.2))
        .collect();
    assert_eq!(
        topics,
        vec![
            (vec![QUST_TOP, (10, QUEST)], 0x0200_1000),
            (vec![QUST_TOP, (10, QUEST)], 0x0200_1001)
        ]
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// KK-275: a clone of a topic under a quest: a new FormID in the parent's children
/// group, `QNAM` the parent, no responses (`TIFL` emptied, `TIFC` 0), every other member
/// as stored. Without a parent the clone is refused, naming the source's quest.
#[test]
fn a_cloned_topic_goes_under_its_new_parent_without_responses() {
    let d = dir("dialogue-clone-topic");
    let data = dialogue_master(&d);
    let into = d.join("Clone.esp");
    let lo = ["Source.esm", "Quest.esm"];
    let err = call(
        "clone",
        &data,
        &lo,
        &into,
        json!({"form": "KC_Topic", "edid": "KC_TopicCopy"}),
    )
    .unwrap_err();
    assert_eq!(code(&err), "clone.no-parent", "{err}");
    assert!(err.to_string().contains("KC_Quest"), "{err}");
    assert!(!into.exists());
    let r = call(
        "clone",
        &data,
        &lo,
        &into,
        json!({"form": "KC_Topic", "edid": "KC_TopicCopy", "parent": "KC_Quest"}),
    )
    .unwrap();
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    assert!(
        r["limitations"].to_string().contains("TIFL emptied"),
        "{}",
        r["limitations"]
    );
    let bytes = sound(&into, &data);
    let rows = records(&bytes);
    let source = records(&std::fs::read(data.join("Quest.esm")).unwrap());
    let original = find(&source, "DIAL", TOPIC);
    let clone = find(&rows, "DIAL", 0x0200_1000);
    assert_eq!(clone.0, vec![QUST_TOP, (10, QUEST)]);
    assert_eq!(u32_of(sub(&clone.3, b"QNAM")), QUEST);
    assert_eq!(sub(&clone.3, b"TIFL"), Some(&[][..]));
    assert_eq!(u32_of(sub(&clone.3, b"TIFC")), 0);
    for (sig, payload) in &original.3 {
        if [b"EDID", b"TIFL", b"TIFC", b"QNAM"].contains(&sig) {
            continue;
        }
        assert_eq!(
            sub(&clone.3, sig),
            Some(payload.as_slice()),
            "{}",
            String::from_utf8_lossy(sig)
        );
    }
    // No response came along.
    assert!(rows.iter().all(|r| r.1 != "INFO"));
    let _ = std::fs::remove_dir_all(&d);
}

/// KK-275 refusals: a topic without a parent, with a parent that is missing or of the
/// wrong type, a parent on a top-level type, a QNAM naming another quest, a scene; nothing
/// is written.
#[test]
fn topic_parent_refusals_are_typed() {
    let d = dir("dialogue-topic-refusals");
    let data = dialogue_master(&d);
    let into = d.join("R.esp");
    let lo = ["Source.esm", "Quest.esm"];
    let cases = [
        (
            "new",
            json!({"type": "DIAL", "edid": "KC_A"}),
            "new.no-parent",
        ),
        (
            "new",
            json!({"type": "DIAL", "edid": "KC_A", "parent": "KC_Nope"}),
            "new.parent-missing",
        ),
        (
            "new",
            json!({"type": "DIAL", "edid": "KC_A", "parent": "Quest.esm:000FFF"}),
            "new.parent-missing",
        ),
        (
            "new",
            json!({"type": "DIAL", "edid": "KC_A", "parent": "OldRifle"}),
            "new.parent-type",
        ),
        (
            "new",
            json!({"type": "DIAL", "edid": "KC_A", "parent": "KC_Topic"}),
            "new.parent-type",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "KC_A", "parent": "KC_Quest"}),
            "new.parent-not-nested",
        ),
        (
            "new",
            json!({"type": "SCEN", "edid": "KC_A", "parent": "KC_Quest"}),
            "new.not-top-level",
        ),
        (
            "new",
            json!({"type": "DIAL", "edid": "KC_A", "parent": "KC_Quest",
                   "fields": {"QNAM": "Source.esm:000800"}}),
            "new.parent-mismatch",
        ),
        (
            "clone",
            json!({"form": "KC_Topic", "edid": "KC_A", "parent": "OldRifle"}),
            "clone.parent-type",
        ),
        (
            "clone",
            json!({"form": "KC_Topic", "edid": "KC_A", "parent": "KC_Nope"}),
            "clone.parent-missing",
        ),
        (
            "clone",
            json!({"form": "OldRifle", "edid": "KC_A", "parent": "KC_Quest"}),
            "clone.parent-not-nested",
        ),
    ];
    for (command, extra, want) in cases {
        let err = call(command, &data, &lo, &into, extra.clone()).unwrap_err();
        assert_eq!(code(&err), want, "{command} {extra}: {err}");
        assert!(!into.exists(), "{command} {extra}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The labels of the groups a copied record hangs in are FormIDs too: a copy whose
/// FormIDs move to other master indexes (the target's masters come in another order) has
/// its quest children group labelled with the moved quest's FormID. Before KK-275 the
/// label kept the source's index, so the writer found no owner for the group and refused.
#[test]
fn a_remapped_topic_keeps_its_children_group_under_its_quest() {
    let d = dir("dialogue-remap-labels");
    let data = dialogue_master(&d);
    crate::edit::patch_tests::write_plugin(
        &data,
        "Other.esm",
        &[],
        kitkraft_esm::tes4::FLAG_MASTER,
        Vec::new(),
    );
    let into = d.join("T.esp");
    crate::edit::patch_tests::write_plugin(&d, "T.esp", &["Other.esm"], 0, Vec::new());
    let r = call(
        "set",
        &data,
        &["Other.esm", "Source.esm", "Quest.esm"],
        &into,
        json!({"form": "KC_Topic", "fields": {"PNAM": 60.0}}),
    )
    .unwrap();
    assert_eq!(
        r["data"]["masters"],
        json!(["Other.esm", "Source.esm", "Quest.esm"])
    );
    let bytes = sound(&into, &data);
    let rows = records(&bytes);
    let quest = 0x0200_1000;
    assert_eq!(find(&rows, "QUST", quest).0, vec![QUST_TOP]);
    let topic = find(&rows, "DIAL", 0x0200_1001);
    assert_eq!(topic.0, vec![QUST_TOP, (10, quest)]);
    assert_eq!(u32_of(sub(&topic.3, b"QNAM")), quest);
    let _ = std::fs::remove_dir_all(&d);
}
