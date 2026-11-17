//! `new` and `clone` (M4.E1) through `execute` on the synthetic localized load order of
//! the `set` tests (`Source.esm` with two weapons and a string table, `Override.esp`).

#![allow(clippy::arithmetic_side_effects)]

use std::path::Path;

use kitkraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitkraft_esm::index::{index_bytes, IndexOptions};
use serde_json::{json, Value};

use super::set_tests::data;
use crate::edit::patch_tests::dir;
use crate::execute;

fn call(
    command: &str,
    data: &Path,
    into: &Path,
    extra: Value,
) -> Result<Value, crate::EngineError> {
    let mut a = json!({
        "into": into.display().to_string(),
        "plugins": ["Source.esm", "Override.esp"], "data_dir": data.display().to_string(),
        "no_cache": true,
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    execute(command, a)
}

fn code(err: &crate::EngineError) -> String {
    let text = err.to_string();
    text.split(": ")
        .find(|p| p.starts_with("new.") || p.starts_with("clone."))
        .unwrap_or(&text)
        .to_string()
}

#[test]
fn a_new_global_gets_a_formid_its_editor_id_and_its_fields() {
    let d = dir("new-glob");
    let data = data(&d);
    let into = d.join("New.esp");
    let r = call(
        "new",
        &data,
        &into,
        json!({"type": "glob", "edid": "KC_TestGlobal", "fields": {"FLTV": 2.5}, "masters": ["Source.esm"]}),
    )
    .unwrap();
    let new = &r["data"]["new_forms"][0];
    assert_eq!(new["form_id"], json!("01001000"));
    assert_eq!(new["form"], json!("New.esp:00001000"));
    assert_eq!(new["signature"], json!("GLOB"));
    assert_eq!(new["editor_id"], json!("KC_TestGlobal"));
    assert_eq!(r["data"]["next_object_id"], json!("FF001001"));
    assert_eq!(r["data"]["masters"], json!(["Source.esm"]));
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    let bytes = std::fs::read(&into).unwrap();
    assert!(bytes.windows(14).any(|w| w == b"KC_TestGlobal\0"));
    assert!(bytes.windows(4).any(|w| w == 2.5f32.to_le_bytes()));
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    // The next one continues after it; deterministic bytes for the same calls.
    let r2 = call(
        "new",
        &data,
        &into,
        json!({"type": "GLOB", "edid": "KC_Second"}),
    )
    .unwrap();
    assert_eq!(r2["data"]["new_forms"][0]["form_id"], json!("01001001"));
    let again = d.join("Again.esp");
    call(
        "new",
        &data,
        &again,
        json!({"type": "glob", "edid": "KC_TestGlobal", "fields": {"FLTV": 2.5}, "masters": ["Source.esm"]}),
    )
    .unwrap();
    call(
        "new",
        &data,
        &again,
        json!({"type": "GLOB", "edid": "KC_Second"}),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(&into).unwrap(),
        std::fs::read(&again).unwrap()
    );
    // Without masters a new plugin has none, and its own prefix is 00.
    let bare = d.join("Bare.esp");
    let r = call(
        "new",
        &data,
        &bare,
        json!({"type": "GLOB", "edid": "KC_Bare"}),
    )
    .unwrap();
    assert_eq!(r["data"]["new_forms"][0]["form_id"], json!("00001000"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_clone_copies_the_winner_with_a_new_formid_and_editor_id() {
    let d = dir("clone-weap");
    let data = data(&d);
    let into = d.join("Clone.esp");
    let r = call(
        "clone",
        &data,
        &into,
        json!({"form": "OldRifle", "edid": "KC_RifleClone", "fields": {"FULL": "Clone Rifle"}}),
    )
    .unwrap();
    let new = &r["data"]["new_forms"][0];
    assert_eq!(new["form_id"], json!("01001000"));
    assert_eq!(new["editor_id"], json!("KC_RifleClone"));
    assert!(
        new["from"].as_str().unwrap().contains("Source.esm:000800"),
        "{new}"
    );
    assert_eq!(
        r["data"]["records"][0]["copied_from"]["plugin"],
        json!("Source.esm")
    );
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]["validate"]
    );
    let bytes = std::fs::read(&into).unwrap();
    assert!(bytes.windows(14).any(|w| w == b"KC_RifleClone\0"));
    assert!(bytes.windows(12).any(|w| w == b"Clone Rifle\0"));
    // The source's EditorID is gone; the template FULL was inlined from the table.
    assert!(!bytes.windows(9).any(|w| w == b"OldRifle\0"));
    assert!(bytes.windows(7).any(|w| w == b"Simple\0"));
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    assert!(index.records.iter().any(|r| r.form_id == 0x0100_1000));
    assert!(!index.records.iter().any(|r| r.form_id == 0x0000_0800));
    // inspect finds the clone with the patch appended.
    let i = execute(
        "inspect",
        json!({"form": "KC_RifleClone", "plugins": ["Source.esm", into.display().to_string()],
               "data_dir": data.display().to_string(), "no_cache": true}),
    )
    .unwrap();
    assert_eq!(
        i["data"]["global"],
        json!("Clone.esp:001000"),
        "{:#}",
        i["data"]
    );
    // A clone and a set of its source in one plugin are two records.
    let both = d.join("Both.esp");
    call(
        "set",
        &data,
        &both,
        json!({"form": "OldRifle", "fields": {"FULL": "Renamed"}}),
    )
    .unwrap();
    let r = call(
        "clone",
        &data,
        &both,
        json!({"form": "OldRifle", "edid": "KC_Two"}),
    );
    // The target has the override (no own records), so the clone joins it.
    assert_eq!(
        r.unwrap()["data"]["new_forms"][0]["form_id"],
        json!("01001000")
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn new_and_clone_refusals_are_typed() {
    let d = dir("newform-refusals");
    let data = data(&d);
    let into = d.join("R.esp");
    let cases = [
        (
            "new",
            json!({"type": "REFR", "edid": "KC_A"}),
            "new.not-top-level",
        ),
        (
            "new",
            json!({"type": "INFO", "edid": "KC_A"}),
            "new.no-parent",
        ),
        (
            "new",
            json!({"type": "ZZZZ", "edid": "KC_A"}),
            "new.untyped",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "OldRifle"}),
            "new.edid-clash",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "bad id"}),
            "new.edid-invalid",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "KC_A", "masters": ["Nope.esm"]}),
            "new.no-master",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "KC_A", "fields": {"EDID": "x"}}),
            "new.edid-field",
        ),
        (
            "new",
            json!({"type": "KYWD", "edid": "KC_A", "fields": {"TNAM[1]": 1}}),
            "new.occurrence",
        ),
        (
            "new",
            json!({"type": "GLOB", "edid": "KC_A", "fields": {"ZZZZ": 1.0}}),
            "new.unknown-member",
        ),
        (
            "clone",
            json!({"form": "Nope", "edid": "KC_A"}),
            "clone.no-form",
        ),
        (
            "clone",
            json!({"form": "OldRifle", "edid": "OtherRifle"}),
            "clone.edid-clash",
        ),
    ];
    for (command, extra, want) in cases {
        let err = call(command, &data, &into, extra.clone()).unwrap_err();
        assert_eq!(code(&err), want, "{command} {extra}: {err}");
        assert!(!into.exists(), "{command} {extra}");
    }
    call(
        "new",
        &data,
        &into,
        json!({"type": "GLOB", "edid": "KC_One"}),
    )
    .unwrap();
    let err = call(
        "clone",
        &data,
        &into,
        json!({"form": "OldRifle", "edid": "KC_One"}),
    )
    .unwrap_err();
    assert_eq!(code(&err), "clone.edid-clash", "{err}");
    // A .esl target is refused before anything.
    let err = call(
        "new",
        &data,
        &d.join("S.esl"),
        json!({"type": "GLOB", "edid": "KC_B"}),
    )
    .unwrap_err();
    assert!(err.to_string().contains(".esl"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// The Creation Kit's defaults for required members of a new record (CK bench 2026-11-17,
/// class A of `docs/bench/ck-batch/triage-20261117.md`): each payload as the CK saved it in
/// ck-20261117-13 (WEAP 01001005, COBJ 01001008), -17 (INFO 01001006, emotion only), -14 (LIGH 0100100F, FURN 01001010, TERM
/// 01001013) and -16 (NPC_ 01001006). A size other than our placeholder's keeps the zeros.
#[test]
fn a_new_record_carries_the_ck_defaults_the_bench_observed() {
    use super::newform::ck_new_default;
    let ck: [(&[u8; 4], &[u8; 4], &str); 12] = [
        (b"WEAP", b"WFIR", "000000000000000000000000000000000000000000000000000000000000C3F5A83E00C3F5A83E000000000000"),
        (b"WEAP", b"WGEN", "0000000000000000000000000000803F0000000000000000"),
        (b"WEAP", b"WMEL", "00000000000000803F00000000"),
        (b"WEAP", b"WRLO", "000000000000803F0000"),
        (b"LIGH", b"DAT2", "FFFFFFFF0000000000000000000000000000803F0000B44200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008040"),
        (b"LIGH", b"LLLD", "0F000000"),
        (b"FURN", b"MNAM", "00000040"),
        (b"TERM", b"MNAM", "00000040"),
        (b"NPC_", b"NAM5", "FF00"),
        (b"NPC_", b"NAM6", "0000803F"),
        (b"COBJ", b"LRNM", "03"),
        (b"INFO", b"TRDA", "FFFFFFFF0000000000000000"),
    ];
    for (record, member, hex) in ck {
        let want = unhex(hex);
        assert_eq!(
            ck_new_default(*record, *member, vec![0; want.len()]),
            want,
            "{} {}",
            String::from_utf8_lossy(record),
            String::from_utf8_lossy(member)
        );
        // Another size (a schema change): our zeros stay.
        assert_eq!(
            ck_new_default(*record, *member, vec![0; want.len() + 1]),
            vec![0; want.len() + 1]
        );
    }
    // A member the CK leaves zero, or another type: zeros.
    assert_eq!(ck_new_default(*b"WEAP", *b"WDMG", vec![0; 4]), vec![0; 4]);
    assert_eq!(ck_new_default(*b"ACTI", *b"MNAM", vec![0; 4]), vec![0; 4]);
}

/// Through `new`: a new WEAP with its EditorID only is written with the CK's WGEN.
#[test]
fn a_new_weapon_is_written_with_the_ck_base_speed() {
    let d = dir("new-weap-ck-defaults");
    let data = data(&d);
    let into = d.join("NewWeap.esp");
    call(
        "new",
        &data,
        &into,
        json!({"type": "WEAP", "edid": "KC_NewWeap"}),
    )
    .unwrap();
    let bytes = std::fs::read(&into).unwrap();
    let mut want = b"WGEN".to_vec();
    want.extend_from_slice(&24u16.to_le_bytes());
    want.extend(unhex("0000000000000000000000000000803F0000000000000000"));
    assert!(
        bytes.windows(want.len()).any(|w| w == want.as_slice()),
        "WGEN with base speed 1.0"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Every new record of a type the CK bench saved after `new` wrote it with its EditorID
/// only, as the Creation Kit saved it: the subrecords after `EDID` of each record of
/// `ck.esp` in the bench folders of `kitkraft-observe\ck-2026-11-17` (sittings
/// ck-20261117-11 to -18 below; `docs/bench/ck-batch/triage-20261117.md`, class A).
/// `SIG:hex`, `0*<n>` for `n` zero bytes. What `new` writes otherwise is in
/// [`ck_saved_new_kept_apart`].
const CK_SAVED_NEW: &[(&str, &str, &str)] = &[
    // ck-20261117-15
    (
        "CELL",
        "KCBCELLNew",
        concat!(
            "FULL:4B4342204E65772043656C6C00 DATA:01000000 ",
            "XCLL:000000000000000000000000000000000000000000000000000000000000803F000000000",
            "000803F000000000000803F00000000000000009F0700000000000000401C46000000000000000",
            "00000803F0000803F0000803F0000803F0000803F000000000000000000000000 ",
            "LTMP:00000000 XCLW:FFFF7F7F XILS:0000803F",
        ),
    ),
    // ck-20261117-16
    (
        "NPC_",
        "KCB_NPC_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "ACBS:00000000000000000000000000000000 RNAM:7D340000 AIDT:00000000000000 STOP: ",
            "DATA: DNAM:0000000000000100 NAM5:FF00 NAM6:0000803F NAM4:0000803F ",
            "MWGT:000000000000000000000000 EDCT:00 STON:00 PRON:00",
        ),
    ),
    // ck-20261117-13
    (
        "WEAP",
        "KCB_WEAP_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 DESC:00 STOP: NNAM:00000000 WAIM:0*35 ",
            "WAM2:0000000000000000000000000000000000000000000000 WAUD:0*288 ",
            "WTUR:0000000000000000000000000000000000000000000000000000000000 ",
            "WCHG:0000000000000000000000000000 WDMG:0*62 ",
            "WFIR:000000000000000000000000000000000000000000000000000000000000C3F5A83E00C3F",
            "5A83E000000000000 WFLG:000000000000000000000000 ",
            "WGEN:0000000000000000000000000000803F0000000000000000 ",
            "WMEL:00000000000000803F00000000 QNAM:0*36 WRLO:000000000000803F0000 WVAR:0*49 ",
            "FLLD:01000000 WVIS:000000000000000000000000000000000000000000000000 ",
            "WTRM:0000000000000000000000000000000000",
        ),
    ),
    // ck-20261117-15
    (
        "LCTN",
        "KCB_LCTN_New",
        "DATA:0000000000001000 RNAM:00000000 ANAM:0000803F TNAM:00000000",
    ),
    // ck-20261117-12
    (
        "BOOK",
        "KCB_BOOK_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 DESC:00 DATA:0000000000000000 ",
            "DNAM:0000000000000000000000000000000000 CNAM:00",
        ),
    ),
    // ck-20261117-12
    (
        "ARMO",
        "KCB_ARMO_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 FLLD:01000000 RNAM:00000000 DESC:00 ",
            "DATA:000000000000000000000000 FNAM:0000000000000000 STOP:",
        ),
    ),
    // ck-20261117-12
    (
        "ARMA",
        "KCB_ARMA_New",
        concat!(
            "RNAM:00000000 DNAM:00000000000000000000000000 FLLD:01000000 FLLD:01000000 ",
            "FLLD:01000000 FLLD:01000000 FLLD:01000000 FLLD:01000000 MNAM:00000000 TNAM:00 ",
            "SNAM:00 VNAM:00 BSMP:00000000 BSMP:01000000",
        ),
    ),
    // ck-20261117-12
    (
        "MISC",
        "KCB_MISC_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 DATA:0000000000000000",
        ),
    ),
    // ck-20261117-12
    (
        "ALCH",
        "KCB_ALCH_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 DESC:00 DATA:00000000 ENIT:0*56 DNAM:00",
        ),
    ),
    // ck-20261117-12
    (
        "KEYM",
        "KCB_KEYM_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 DATA:0000000000000000",
        ),
    ),
    // ck-20261117-12
    (
        "LVLI",
        "KCB_LVLI_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "LVLD:00000000 LVLM:00 LVLF:0000 FLLD:01000000",
        ),
    ),
    // ck-20261117-16
    (
        "LVLN",
        "KCB_LVLN_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "LVLD:00000000 LVLM:00 LVLF:0000 FLLD:01000000",
        ),
    ),
    // ck-20261117-16
    (
        "LVLP",
        "KCB_LVLP_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "LVLD:00000000 LVLM:00 LVLF:0000 FLLD:01000000",
        ),
    ),
    // ck-20261117-16
    (
        "PACK",
        "KCB_PACK_New",
        concat!(
            "PKDT:000000000000000000000000 PSDT:000000000000000000000000 ",
            "PKCU:000000000000000000000000 XNAM:00 POBA: INAM:00000000 ",
            "PDTO:0000000000000000 POEA: INAM:00000000 PDTO:0000000000000000 POCA: ",
            "INAM:00000000 PDTO:0000000000000000",
        ),
    ),
    // ck-20261117-16
    (
        "PERK",
        "KCB_PERK_New",
        "DESC:00 DATA:00000000 PRRK: DESC:00 PRUC:02000000 PRRF:",
    ),
    // ck-20261117-14
    (
        "LIGH",
        "KCB_LIGH_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 ",
            "DAT2:FFFFFFFF0000000000000000000000000000803F0000B4420000000000000000000000000",
            "000000000000000000000000000000000000000000000000000000000000000000000000000804",
            "0 FLBD:000000000000000000000000000000000000000000000000 ",
            "FLRD:0000000000000000000000000000000000000000 FLGD:0*88 LLLD:0F000000 ",
            "FLAD:00000000000000000000000000000000 FVLD:00000000",
        ),
    ),
    // ck-20261117-14
    (
        "FURN",
        "KCB_FURN_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 PNAM:00000000 FNAM:0000 JNAM:0000 MNAM:00000040 GNAM:0000 ",
            "WBDT:00 STOP:",
        ),
    ),
    // ck-20261117-14
    (
        "TERM",
        "KCB_TERM_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 NAM1:00 ",
            "FLLD:01000000 PNAM:00000000 FNAM:0000 JNAM:0000 MNAM:00000040 GNAM:0000 ",
            "WBDT:00 STOP:",
        ),
    ),
    // ck-20261117-14
    (
        "ACTI",
        "KCB_ACTI_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 PNAM:00000000 FNAM:0000 JNAM:0000",
        ),
    ),
    // ck-20261117-14
    (
        "CONT",
        "KCB_CONT_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 DATA:00 STOP:",
        ),
    ),
    // ck-20261117-14
    (
        "DOOR",
        "KCB_DOOR_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 FNAM:00 DEVT:00000000 ANAM:00000000",
        ),
    ),
    // ck-20261117-14
    (
        "FLOR",
        "KCB_FLOR_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 PNAM:00000000 FNAM:0000 JNAM:0000 PFPC:00000000 STOP: ",
            "BNAM:00000000 FMAH:00000000 FMIH:00000000",
        ),
    ),
    // ck-20261117-14
    (
        "MSTT",
        "KCB_MSTT_New",
        concat!(
            "OBND:000000000000000000000000000000000000000000000000 ODTY:00000000 ",
            "FLLD:01000000 DATA:00",
        ),
    ),
];

/// Where `new` keeps writing something else than the CK saved, by type: the CK's item and
/// ours. CELL `FULL` is the bench recipe's name, not a default; NPC_ `RNAM` is the CK's
/// race `Starfield.esm:00347D`, a master's form; BOOK `CNAM` keeps the 4 zero bytes of its
/// layout and of every official BOOK (the CK narrows it, class B `BOOK CNAM
/// zero-narrowed`); ARMO and ARMA keep a zero `BO64` the CK drops (class B `BO64
/// zero-dropped`, compared apart).
fn ck_saved_new_kept_apart(sig: &str) -> &'static [(&'static str, &'static str)] {
    match sig {
        "CELL" => &[("FULL:4B4342204E65772043656C6C00", "")],
        "NPC_" => &[("RNAM:7D340000", "RNAM:00000000")],
        "BOOK" => &[("CNAM:00", "CNAM:00000000")],
        _ => &[],
    }
}

/// The subrecords of the one record of `sig` in `plugin` (uncompressed), as `SIG:hex`.
fn written_record(plugin: &[u8], sig: &[u8; 4]) -> Vec<String> {
    fn find<'a>(buf: &'a [u8], sig: &[u8; 4]) -> Option<&'a [u8]> {
        let mut off = 0;
        while off + 24 <= buf.len() {
            let size = u32::from_le_bytes(buf[off + 4..off + 8].try_into().unwrap()) as usize;
            if &buf[off..off + 4] == b"GRUP" {
                if let Some(r) = find(&buf[off + 24..off + size], sig) {
                    return Some(r);
                }
                off += size;
            } else {
                if &buf[off..off + 4] == sig {
                    return Some(&buf[off + 24..off + 24 + size]);
                }
                off += 24 + size;
            }
        }
        None
    }
    let body = find(plugin, sig).expect("the new record");
    let mut out = Vec::new();
    let mut o = 0;
    while o < body.len() {
        let s = String::from_utf8_lossy(&body[o..o + 4]).into_owned();
        let n = u16::from_le_bytes([body[o + 4], body[o + 5]]) as usize;
        let data = &body[o + 6..o + 6 + n];
        o += 6 + n;
        if s == "EDID" {
            continue;
        }
        let hex: String = data.iter().map(|b| format!("{b:02X}")).collect();
        out.push(if data.len() > 32 && data.iter().all(|b| *b == 0) {
            format!("{s}:0*{}", data.len())
        } else {
            format!("{s}:{hex}")
        });
    }
    out
}

#[test]
fn a_new_record_is_written_as_the_ck_saved_it() {
    let d = dir("new-ck-saved");
    let data = data(&d);
    let mut wrong = Vec::new();
    for (sig, edid, saved) in CK_SAVED_NEW {
        let into = d.join(format!("New{}.esp", sig.trim_end_matches('_')));
        let _ = std::fs::remove_file(&into);
        let edid = edid.replace('_', "");
        if let Err(e) = call("new", &data, &into, json!({"type": sig, "edid": edid})) {
            wrong.push(format!("{sig}: {e}"));
            continue;
        }
        let mut ours = written_record(
            &std::fs::read(&into).unwrap(),
            sig.as_bytes().try_into().unwrap(),
        );
        if matches!(*sig, "ARMO" | "ARMA") {
            let at = ours
                .iter()
                .position(|s| s.starts_with("BO64:"))
                .expect("BO64");
            assert_eq!(
                ours.remove(at),
                format!("BO64:{}", "0".repeat(ours_bo64(sig)))
            );
        }
        let mut want: Vec<String> = saved.split(' ').map(str::to_string).collect();
        for (ck, kept) in ck_saved_new_kept_apart(sig) {
            let at = want.iter().position(|s| s == ck).expect("the CK's item");
            if kept.is_empty() {
                want.remove(at);
            } else {
                want[at] = (*kept).to_string();
            }
        }
        if ours != want {
            wrong.push(format!(
                "{sig}:\n  ours {}\n  ck   {}",
                ours.join(" "),
                want.join(" ")
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let _ = std::fs::remove_dir_all(&d);
}
