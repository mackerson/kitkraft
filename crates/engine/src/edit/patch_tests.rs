//! Tests of the patch writer (M2.E4) on synthetic plugins.

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitcraft_esm::build::HeaderSpec;
use kitcraft_esm::driver::{roundtrip_bytes, RoundtripOptions};
use kitcraft_esm::tes4::{FLAG_LOCALIZED, FLAG_MASTER};
use kitcraft_esm::{
    Body, DecodeLimits, Entry, GroupHeader, Plugin, Record, RecordHeader, Signature, Subrecord,
};
use serde_json::json;

use super::fields::{parse_path, Op};
use super::patch::{
    finish, flags_entry, marks_written, write_patch, Copy, Edit, PatchError, PatchOptions,
};
use super::source::{find_winner, index_plugins, read_copy, Indexed};
use super::strings::{MemoryStrings, NoStrings, StringLookup};

pub(crate) fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kitcraft-e4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sub(sig: &[u8; 4], data: &[u8]) -> Subrecord<'static> {
    Subrecord::new(Signature(*sig), data.to_vec())
}

/// A WEAP shaped like the Old Earth Hunting Rifle: a localized FULL, DESC = string 0, an
/// OBTE block with a template FULL, opaque subrecords in between.
pub(crate) fn weapon(form_id: u32, edid: &str, full: u32) -> Record<'static> {
    let mut edid = edid.as_bytes().to_vec();
    edid.push(0);
    Record::new(
        RecordHeader {
            signature: Signature(*b"WEAP"),
            flags: 0x4000_0000,
            form_id,
            timestamp: 0x1234,
            version_control: 0x5678,
            form_version: 581,
            unknown: 7,
        },
        Body::Raw(vec![
            sub(b"EDID", &edid),
            sub(b"FULL", &full.to_le_bytes()),
            sub(b"MODL", b"Weapons\\a.nif\0"),
            sub(b"DESC", &[0, 0, 0, 0]),
            sub(b"OBTE", &[1, 0, 0, 0]),
            sub(b"FULL", &0x0002_EDD8u32.to_le_bytes()),
            sub(b"OBTS", &[9; 22]),
            sub(b"STOP", &[]),
            sub(b"WDMG", &[1, 2, 3, 4, 5]),
        ]),
    )
}

pub(crate) fn write_plugin(
    dir: &Path,
    name: &str,
    masters: &[&str],
    flags: u32,
    records: Vec<Record<'static>>,
) -> PathBuf {
    let mut spec = HeaderSpec::new(masters.iter().map(|m| m.as_bytes().to_vec()).collect());
    spec.flags = flags;
    let mut plugin = Plugin::create(&spec);
    for r in records {
        let top = GroupHeader::top(r.header.signature);
        plugin.upsert(&[top], r).unwrap();
    }
    let path = dir.join(name);
    std::fs::write(&path, plugin.to_bytes().unwrap()).unwrap();
    path
}

pub(crate) fn strings() -> MemoryStrings {
    let mut m = MemoryStrings::default();
    m.0.insert(
        ("Source.esm".into(), 0x0002_E469),
        "Old Earth Hunting Rifle".into(),
    );
    m.0.insert(("Source.esm".into(), 0x0002_EDD8), "Simple".into());
    m.0.insert(("Source.esm".into(), 0x0002_E46A), "Other Rifle".into());
    m
}

fn copy_of(plugins: &[Indexed], form: &str, edits: Vec<Edit>) -> Copy {
    let winner = find_winner(plugins, form).unwrap();
    let p = &plugins[winner.plugin];
    let (record, path) = read_copy(
        &p.plugin.path,
        &p.index,
        winner.loc,
        &DecodeLimits::default(),
    )
    .unwrap();
    Copy {
        source: p.plugin.clone(),
        record,
        path,
        parents: Vec::new(),
        edits,
        formref: winner.form.formref(),
    }
}

fn rename(text: &str) -> Vec<Edit> {
    vec![Edit {
        path: parse_path("FULL").unwrap(),
        op: Op::Set(json!(text)),
    }]
}

fn options<'a>(lookup: &'a dyn StringLookup, plugins: &[Indexed]) -> PatchOptions<'a> {
    PatchOptions {
        strings: lookup,
        protected: plugins.iter().map(|p| p.plugin.path.clone()).collect(),
        plugin_flags: plugins.iter().map(|p| flags_entry(&p.plugin)).collect(),
        limits: DecodeLimits::default(),
        allow_untyped_lstrings: false,
    }
}

pub(crate) fn source(dir: &Path) -> Vec<Indexed> {
    let path = write_plugin(
        dir,
        "Source.esm",
        &[],
        FLAG_MASTER | FLAG_LOCALIZED,
        vec![
            weapon(0x0000_0800, "OldRifle", 0x0002_E469),
            weapon(0x0000_0801, "OtherRifle", 0x0002_E46A),
        ],
    );
    index_plugins(&[path]).unwrap()
}

fn first_record<'p>(plugin: &'p Plugin<'_>, i: usize) -> &'p Record<'p> {
    let Entry::Group(g) = &plugin.entries()[0] else {
        panic!()
    };
    let Entry::Record(r) = &g.children()[i] else {
        panic!()
    };
    r
}

#[test]
fn new_target_is_non_localized_and_lossless() {
    let d = dir("new");
    let plugins = source(&d);
    let lookup = strings();
    let target = d.join("out").join("Patch.esm");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let copy = copy_of(&plugins, "OldRifle", rename("Old Soviet Marksman Rifle"));
    let original = copy.record.clone();
    let report = write_patch(&target, vec![copy], &options(&lookup, &plugins)).unwrap();
    assert!(report.created);
    assert_eq!(report.masters, ["Source.esm"]);
    assert_eq!(report.flags, FLAG_MASTER);
    let r = &report.records[0];
    assert_eq!(
        (r.form_id, r.outcome, r.formref.as_str()),
        (0x800, "inserted", "Source.esm:00000800")
    );
    assert_eq!(r.changes[0].after, json!("Old Soviet Marksman Rifle"));
    let at: Vec<&str> = r.inlined.iter().map(|i| i.at.as_str()).collect();
    assert_eq!(at, ["DESC[0]", "FULL[1]"]);

    let bytes = std::fs::read(&target).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let plugin = Plugin::parse(&bytes).unwrap();
    let tes4 = plugin.tes4();
    assert!(!tes4.is_localized());
    assert!(tes4.is_master());
    assert_eq!(tes4.hedr().unwrap().unwrap().record_count, 2);
    assert!(tes4.overridden_forms().is_empty());
    let w = first_record(&plugin, 0);
    assert_eq!(w.header, original.header);
    let got: Vec<(String, Vec<u8>)> = w
        .subrecords()
        .unwrap()
        .iter()
        .map(|s| (s.signature().to_string(), s.data().to_vec()))
        .collect();
    let want: Vec<(String, Vec<u8>)> = original
        .subrecords()
        .unwrap()
        .iter()
        .map(|s| {
            let sig = s.signature().to_string();
            let data = match (sig.as_str(), s.data()) {
                ("FULL", d) if d == 0x0002_E469u32.to_le_bytes() => {
                    b"Old Soviet Marksman Rifle\0".to_vec()
                }
                ("FULL", _) => b"Simple\0".to_vec(),
                ("DESC", _) => b"\0".to_vec(),
                (_, d) => d.to_vec(),
            };
            (sig, data)
        })
        .collect();
    assert_eq!(got, want);
    // The evidence points at the new FULL in the file.
    let (sig, offset, len) = r.edited_subrecords[0].clone();
    assert_eq!(sig, "FULL");
    let offset = usize::try_from(offset.unwrap()).unwrap();
    assert_eq!(
        &bytes[offset..offset + len as usize],
        b"Old Soviet Marksman Rifle\0"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_second_record_into_the_same_target() {
    let d = dir("second");
    let plugins = source(&d);
    let lookup = strings();
    let target = d.join("Patch.esp");
    write_patch(
        &target,
        vec![copy_of(&plugins, "OldRifle", rename("A"))],
        &options(&lookup, &plugins),
    )
    .unwrap();
    let first = std::fs::read(&target).unwrap();
    let report = write_patch(
        &target,
        vec![copy_of(&plugins, "Source.esm:000801", rename("B"))],
        &options(&lookup, &plugins),
    )
    .unwrap();
    assert!(!report.created);
    assert_eq!(report.flags, 0);
    assert_eq!(report.records[0].outcome, "inserted");
    let bytes = std::fs::read(&target).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let plugin = Plugin::parse(&bytes).unwrap();
    assert_eq!(plugin.tes4().hedr().unwrap().unwrap().record_count, 3);
    // The first record's bytes are unchanged.
    let old = Plugin::parse(&first).unwrap();
    let span = |r: &Record<'_>| {
        let start = r.offset().unwrap() as usize;
        start..start + 24 + r.data_size_as_read().unwrap() as usize
    };
    assert_eq!(
        &first[span(first_record(&old, 0))],
        &bytes[span(first_record(&plugin, 0))]
    );
    // Writing it again replaces it in place.
    let report = write_patch(
        &target,
        vec![copy_of(&plugins, "OldRifle", rename("C"))],
        &options(&lookup, &plugins),
    )
    .unwrap();
    assert_eq!(report.records[0].outcome, "replaced");
    let again = Plugin::parse(&std::fs::read(&target).unwrap())
        .unwrap()
        .tes4()
        .hedr()
        .unwrap()
        .unwrap()
        .record_count;
    assert_eq!(again, 3);
    let _ = std::fs::remove_dir_all(&d);
}

fn typed(form_id: u32) -> Record<'static> {
    Record::new(
        RecordHeader {
            signature: Signature(*b"WEAP"),
            form_id,
            form_version: 582,
            ..RecordHeader::default()
        },
        Body::Raw(vec![
            sub(b"EDID", b"Typed\0"),
            sub(b"KSIZ", &2u32.to_le_bytes()),
            sub(b"KWDA", &[0x10, 0, 0, 0, 0x20, 0, 0, 1]),
        ]),
    )
}

#[test]
fn remap_through_typed_form_ids() {
    let d = dir("remap");
    let base = write_plugin(&d, "Base.esm", &[], FLAG_MASTER, vec![]);
    let other = write_plugin(&d, "Other.esm", &[], FLAG_MASTER, vec![]);
    // Source.esm: masters [Other.esm, Base.esm]; overrides Base.esm:000900 and lists a
    // keyword of each master.
    let src = write_plugin(
        &d,
        "Source.esm",
        &["Other.esm", "Base.esm"],
        FLAG_MASTER,
        vec![typed(0x0100_0900)],
    );
    let plugins = index_plugins(&[base, other, src]).unwrap();
    // The target already has masters [Base.esm, Other.esm] (the other order).
    let target = write_plugin(&d, "Patch.esp", &["Base.esm", "Other.esm"], 0, vec![]);
    let report = write_patch(
        &target,
        vec![copy_of(&plugins, "Base.esm:000900", vec![])],
        &options(&NoStrings, &plugins),
    )
    .unwrap();
    assert_eq!(report.masters, ["Base.esm", "Other.esm", "Source.esm"]);
    assert_eq!(report.records[0].form_id, 0x0000_0900);
    let bytes = std::fs::read(&target).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let plugin = Plugin::parse(&bytes).unwrap();
    // Other.esm:000010 was 00000010 in Source (index 0) and is 01000010 here; Base.esm
    // :000020 was 01000020 and is 00000020.
    assert_eq!(
        first_record(&plugin, 0).subrecords().unwrap()[2].data(),
        [0x10, 0, 0, 1, 0x20, 0, 0, 0]
    );

    // The same with an opaque subrecord is refused, and the target is left alone.
    let mut opaque = typed(0x0100_0901);
    opaque.subrecords_mut().unwrap().push(sub(b"WDMG", &[0; 4]));
    let src2 = write_plugin(
        &d,
        "Source2.esm",
        &["Other.esm", "Base.esm"],
        FLAG_MASTER,
        vec![opaque],
    );
    let plugins = index_plugins(&[d.join("Base.esm"), d.join("Other.esm"), src2]).unwrap();
    let target2 = write_plugin(&d, "Patch2.esp", &["Base.esm", "Other.esm"], 0, vec![]);
    let before = std::fs::read(&target2).unwrap();
    let err = write_patch(
        &target2,
        vec![copy_of(&plugins, "Base.esm:000901", vec![])],
        &options(&NoStrings, &plugins),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("WDMG subrecord is opaque"),
        "{err}"
    );
    assert_eq!(std::fs::read(&target2).unwrap(), before);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn refusals() {
    let d = dir("refuse");
    let plugins = source(&d);
    let lookup = strings();
    let opts = options(&lookup, &plugins);
    let go = |target: &Path, edits: Vec<Edit>| {
        write_patch(target, vec![copy_of(&plugins, "OldRifle", edits)], &opts)
    };
    // An unresolved string ID refuses, naming every one; ID 0 needs no table.
    let err = write_patch(
        &d.join("A.esp"),
        vec![copy_of(&plugins, "OldRifle", vec![])],
        &options(&NoStrings, &plugins),
    )
    .unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("FULL[0]") && text.contains("FULL[1]") && !text.contains("DESC"),
        "{text}"
    );
    assert!(!d.join("A.esp").exists());
    // ...but not one the edit replaces.
    let mut partial = MemoryStrings::default();
    partial
        .0
        .insert(("Source.esm".into(), 0x0002_EDD8), "Simple".into());
    write_patch(
        &d.join("B.esp"),
        vec![copy_of(&plugins, "OldRifle", rename("x"))],
        &options(&partial, &plugins),
    )
    .unwrap();
    // Extensions.
    let err = go(&d.join("C.esl"), rename("x")).unwrap_err().to_string();
    assert!(err.contains(".esl"), "{err}");
    assert!(go(&d.join("C.txt"), rename("x")).is_err());
    // Never over a load-order plugin, never in a Data or MO2 directory.
    let err = go(&plugins[0].plugin.path, rename("x"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("load order"), "{err}");
    let data = d.join("Data");
    std::fs::create_dir_all(data.join("sub")).unwrap();
    std::fs::write(data.join("Starfield.esm"), b"x").unwrap();
    let err = go(&data.join("sub").join("P.esp"), rename("x"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("Data directory"), "{err}");
    let user = d.join("My Games").join("Starfield").join("Data");
    std::fs::create_dir_all(&user).unwrap();
    let err = go(&user.join("P.esp"), rename("x"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("user Data directory"), "{err}");
    let mo2 = d.join("MO2");
    std::fs::create_dir_all(mo2.join("mods").join("New")).unwrap();
    std::fs::write(mo2.join("ModOrganizer.ini"), b"x").unwrap();
    let err = go(&mo2.join("mods").join("New").join("P.esp"), rename("x"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("Mod Organizer"), "{err}");
    // A localized target.
    let loc = write_plugin(&d, "Loc.esp", &["Source.esm"], FLAG_LOCALIZED, vec![]);
    let err = go(&loc, rename("x")).unwrap_err().to_string();
    assert!(err.contains("localized"), "{err}");
    // The same form twice in one write.
    let twice = write_patch(
        &d.join("T.esp"),
        vec![
            copy_of(&plugins, "OldRifle", rename("a")),
            copy_of(&plugins, "OldRifle", rename("b")),
        ],
        &opts,
    );
    assert!(twice.unwrap_err().to_string().contains("twice"));
    // A bad field is a Field error naming the path.
    let bad = vec![Edit {
        path: parse_path("FULL[5]").unwrap(),
        op: Op::Set(json!("x")),
    }];
    match go(&d.join("F.esp"), bad) {
        Err(PatchError::Field(e)) => {
            assert_eq!((e.path.as_str(), e.rule), ("FULL[5]", "no-subrecord"))
        }
        other => panic!("{other:?}"),
    }
    // A target with records of its own cannot gain masters.
    let own = write_plugin(&d, "Own.esp", &[], 0, vec![weapon(0x0000_0001, "Mine", 1)]);
    let err = go(&own, rename("x")).unwrap_err().to_string();
    assert!(err.contains("records of its own"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

fn rec(sig: &[u8; 4], form_id: u32, subs: Vec<Subrecord<'static>>) -> Record<'static> {
    Record::new(
        RecordHeader {
            signature: Signature(*sig),
            form_id,
            form_version: 582,
            ..RecordHeader::default()
        },
        Body::Raw(subs),
    )
}

/// Fable review round 2, finding 2a: lstrings the schema does not type. FLOR has no
/// definition: FULL is inlined by the fallback, ATTX by the corpus table. WEAP has no ATTX
/// in the corpus: a non-zero 4-byte ATTX from a localized source is refused, naming it,
/// unless `allow_untyped_lstrings`; a zero one is harmless and copied.
#[test]
fn untyped_lstrings_of_localized_sources() {
    let d = dir("untyped");
    let flor = rec(
        b"FLOR",
        0x0000_0802,
        vec![
            sub(b"EDID", b"Panel\0"),
            sub(b"FULL", &0x0002_E469u32.to_le_bytes()),
            sub(b"ATTX", &0x0002_EDD8u32.to_le_bytes()),
        ],
    );
    let mut rifle = weapon(0x0000_0800, "OldRifle", 0x0002_E469);
    rifle
        .subrecords_mut()
        .unwrap()
        .push(sub(b"ATTX", &0x0002_E46Au32.to_le_bytes()));
    let mut zero = weapon(0x0000_0801, "ZeroRifle", 0x0002_E469);
    zero.subrecords_mut().unwrap().push(sub(b"ATTX", &[0; 4]));
    let src = write_plugin(
        &d,
        "Source.esm",
        &[],
        FLAG_MASTER | FLAG_LOCALIZED,
        vec![rifle, zero, flor],
    );
    let plugins = index_plugins(&[src]).unwrap();
    let lookup = strings();
    let opts = options(&lookup, &plugins);

    let target = d.join("A.esp");
    let report = write_patch(
        &target,
        vec![copy_of(&plugins, "Source.esm:000802", vec![])],
        &opts,
    )
    .unwrap();
    let at: Vec<&str> = report.records[0]
        .inlined
        .iter()
        .map(|i| i.at.as_str())
        .collect();
    assert_eq!(at, ["FULL[0]", "ATTX[0]"]);
    assert!(report
        .limitations
        .iter()
        .any(|l| l.contains("FLOR.ATTX treated as an lstring by the corpus lstring table")));
    let bytes = std::fs::read(&target).unwrap();
    let plugin = Plugin::parse(&bytes).unwrap();
    let attx: Vec<&[u8]> = first_record(&plugin, 0)
        .subrecords()
        .unwrap()
        .iter()
        .filter(|s| s.signature() == Signature(*b"ATTX"))
        .map(|s| s.data())
        .collect();
    assert_eq!(attx, [b"Simple\0"]);

    let refused_target = d.join("W.esp");
    let err = write_patch(
        &refused_target,
        vec![copy_of(&plugins, "OldRifle", rename("x"))],
        &opts,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("ATTX[0]") && err.contains("allow_untyped_lstrings"),
        "{err}"
    );
    assert!(!refused_target.exists());

    let mut allowed = options(&lookup, &plugins);
    allowed.allow_untyped_lstrings = true;
    let report = write_patch(
        &refused_target,
        vec![copy_of(&plugins, "OldRifle", rename("x"))],
        &allowed,
    )
    .unwrap();
    assert!(
        report
            .limitations
            .iter()
            .any(|l| l.contains("ATTX[0] copied as stored under allow_untyped_lstrings")),
        "{:?}",
        report.limitations
    );

    // A zero ATTX is not a string ID worth refusing.
    write_patch(
        &d.join("Z.esp"),
        vec![copy_of(&plugins, "ZeroRifle", rename("x"))],
        &opts,
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 3: a copy from a small or medium plugin, or from one with
/// a small or medium master (or a master of unknown scale), is refused in pass 1.
#[test]
fn scaled_sources_are_refused() {
    let d = dir("scaled");
    let small = write_plugin(
        &d,
        "Small.esm",
        &[],
        FLAG_MASTER | kitcraft_esm::tes4::FLAG_SMALL,
        vec![],
    );
    let child = write_plugin(&d, "Child.esm", &["Small.esm"], FLAG_MASTER, vec![]);
    let plugins = index_plugins(&[small, child]).unwrap();
    let copy = |p: &Indexed| Copy {
        source: p.plugin.clone(),
        record: weapon(0x0000_0800, "Rifle", 0),
        path: vec![GroupHeader::top(Signature(*b"WEAP"))],
        parents: Vec::new(),
        edits: Vec::new(),
        formref: format!("{}:000800", p.plugin.name),
    };
    let target = d.join("P.esp");
    let opts = options(&NoStrings, &plugins);
    let err = write_patch(&target, vec![copy(&plugins[0])], &opts)
        .unwrap_err()
        .to_string();
    assert!(err.contains("Small.esm, a small or medium plugin"), "{err}");
    let err = write_patch(&target, vec![copy(&plugins[1])], &opts)
        .unwrap_err()
        .to_string();
    assert!(err.contains("small or medium master Small.esm"), "{err}");
    // The master's header was not read: its scale is unknown, never guessed.
    let mut unknown = options(&NoStrings, &plugins);
    unknown.plugin_flags.retain(|(n, _)| n != "small.esm");
    let err = write_patch(&target, vec![copy(&plugins[1])], &unknown)
        .unwrap_err()
        .to_string();
    assert!(err.contains("scale of Small.esm"), "{err}");
    assert!(!target.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 5: a new patch's TES4 has `CNAM` "DEFAULT" and `INCC`
/// (interior cells), as the two BlueprintShips files; no SNAM, BNAM or CHGL. `INCC` counts
/// interior cells, and grows when one is added to an existing patch.
#[test]
fn new_header_metadata_and_interior_cell_count() {
    let d = dir("tes4");
    let src = write_plugin(&d, "Cells.esm", &[], FLAG_MASTER, vec![]);
    let plugins = index_plugins(&[src]).unwrap();
    let cell_copy = |form_id: u32| {
        let block = |group_type: i32, label: u32| GroupHeader {
            label: label.to_le_bytes(),
            group_type,
            timestamp: 0,
            version_control: 0,
            unknown: 0,
        };
        Copy {
            source: plugins[0].plugin.clone(),
            record: rec(b"CELL", form_id, vec![sub(b"EDID", b"Room\0")]),
            path: vec![
                GroupHeader::top(Signature(*b"CELL")),
                block(2, 1),
                block(3, 1),
            ],
            parents: Vec::new(),
            edits: Vec::new(),
            formref: format!("Cells.esm:{form_id:06X}"),
        }
    };
    let header = |path: &Path| -> Vec<(String, Vec<u8>)> {
        let bytes = std::fs::read(path).unwrap();
        let plugin = Plugin::parse(&bytes).unwrap();
        plugin
            .header()
            .subrecords()
            .unwrap()
            .iter()
            .map(|s| (s.signature().to_string(), s.data().to_vec()))
            .collect()
    };
    let opts = options(&NoStrings, &plugins);
    let target = d.join("P.esp");
    write_patch(&target, vec![cell_copy(0x0000_0900)], &opts).unwrap();
    let subs = header(&target);
    let names: Vec<&str> = subs.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(names, ["HEDR", "CNAM", "MAST", "DATA", "INCC"]);
    assert_eq!(subs[1].1, b"DEFAULT\0");
    assert_eq!(subs[4].1, 1u32.to_le_bytes());
    write_patch(&target, vec![cell_copy(0x0000_0901)], &opts).unwrap();
    assert_eq!(header(&target)[4].1, 2u32.to_le_bytes());
    // Replacing a cell the patch has does not count it twice.
    write_patch(&target, vec![cell_copy(0x0000_0901)], &opts).unwrap();
    assert_eq!(header(&target)[4].1, 2u32.to_le_bytes());
    let bytes = std::fs::read(&target).unwrap();
    assert!(roundtrip_bytes(&bytes, &RoundtripOptions::default())
        .unwrap()
        .identical());
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 6: an lstring inlined from a table with bytes >= 0x80 is
/// named in a limitation (encoding unverified in game).
#[test]
fn non_ascii_inlined_text_is_a_limitation() {
    let d = dir("nonascii");
    let plugins = source(&d);
    let mut lookup = strings();
    lookup
        .0
        .insert(("Source.esm".into(), 0x0002_EDD8), "Fusée".into());
    let report = write_patch(
        &d.join("P.esp"),
        vec![copy_of(&plugins, "OldRifle", rename("Plain"))],
        &options(&lookup, &plugins),
    )
    .unwrap();
    let notes: Vec<&String> = report
        .limitations
        .iter()
        .filter(|l| l.contains("0x80"))
        .collect();
    assert_eq!(notes.len(), 1, "{:?}", report.limitations);
    assert!(notes[0].starts_with("FULL[1]: "), "{}", notes[0]);
    let _ = std::fs::remove_dir_all(&d);
}

/// Fable review round 2, finding 7: an edit of a struct's lstring tail (a member path)
/// marks the subrecord written, so the inliner does not read the new text as an ID; an
/// edit of another member does not.
#[test]
fn member_lstring_edits_mark_the_subrecord_written() {
    let edit = |path: &str| Edit {
        path: parse_path(path).unwrap(),
        op: Op::Set(json!("x")),
    };
    let change = |lstring: bool| crate::edit::FieldChange {
        path: String::new(),
        subrecord_index: 3,
        before: json!(null),
        after: json!(null),
        changed: true,
        notes: Vec::new(),
        lstring,
    };
    assert!(marks_written(&edit("DNAM.Name"), &change(true)));
    assert!(!marks_written(&edit("DNAM.Speed"), &change(false)));
    assert!(marks_written(&edit("DNAM"), &change(false)));
}

/// Fable review round 2, finding 12: the evidence pass runs on the staged file before the
/// rename; when it fails, the target is untouched and the staged file is removed.
#[test]
fn a_failed_evidence_pass_leaves_the_target_alone() {
    let d = dir("finish");
    let target = d.join("P.esp");
    let temp = d.join(".P.esp.tmp");
    std::fs::write(&target, b"previous").unwrap();
    std::fs::write(&temp, b"not a plugin").unwrap();
    let err = finish(&temp, &target, &mut [], &DecodeLimits::default());
    assert!(err.is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"previous");
    assert!(!temp.exists());
    let _ = std::fs::remove_dir_all(&d);
}
