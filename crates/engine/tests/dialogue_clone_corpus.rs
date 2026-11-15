//! KK-278: `clone` of dialogue topics and responses on a sample of the official
//! `Starfield.esm` (`KITKRAFT_DATA_DIR`; skipped with a printed line when unset), in the
//! shape of `clone_sample_corpus.rs`.
//!
//! Each test walks Starfield.esm once for every record of the type with its parent (a
//! topic's quest is the label of its quest children group, type 10; a response's topic the
//! label of its topic children group, type 7), takes a deterministic sample spread evenly
//! over the FormID-sorted list (fixed stride), and clones each into one patch under its
//! own source parent (`parent`, `docs/design/dialogue.md`). The patch is read back and each
//! clone compared with its source subrecord by subrecord. A clone changes exactly: the
//! FormID (a new allocation), the `EDID` (a new EditorID; added first when the source has
//! none, as most topics and responses have none), every lstring span inlined from the
//! string tables (the patch is not localized), and for a topic the response list (`TIFL`
//! emptied, `TIFC` 0: a clone owns no responses). Every other subrecord is byte-identical,
//! in order. The group path is checked (a topic in its quest's type 10 group, a response in
//! its topic's type 7 group inside the quest's), and for responses every carried topic's
//! `TIFL` is its source list with the clones under it appended, in allocation order, and
//! `TIFC` its length. The patch must validate with 0 errors.
//!
//! The sample size is `KK_DIALOGUE_SAMPLE` (default 100).
//!
//! Run: `cargo test --release -p kitkraft-engine --test dialogue_clone_corpus -- --include-ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use kitkraft_doc::strings::{DirSource, StringTables};
use kitkraft_doc::typed::Ctx;
use kitkraft_engine::edit::{fields, patch};
use kitkraft_engine::Engine;
use kitkraft_esm::{DecodeLimits, Entry, Plugin, Record};
use serde_json::json;

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

fn out_dir(name: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sample_size() -> usize {
    std::env::var("KK_DIALOGUE_SAMPLE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100)
}

/// One parsed record, owned: signature, header, subrecords, group path `(type, label)`.
struct Rec {
    sig: [u8; 4],
    /// (flags, FormID, timestamp, version control, form version, unknown).
    hdr: (u32, u32, u16, u16, u16, u16),
    subs: Vec<([u8; 4], Vec<u8>)>,
    path: Vec<(i32, u32)>,
}

fn rec_of(r: &mut Record, path: &[(i32, u32)]) -> Rec {
    r.decompress(&DecodeLimits::default()).unwrap();
    let subs = r.subrecords().unwrap();
    Rec {
        sig: r.header.signature.bytes(),
        hdr: (
            r.header.flags,
            r.header.form_id,
            r.header.timestamp,
            r.header.version_control,
            r.header.form_version,
            r.header.unknown,
        ),
        subs: subs
            .iter()
            .map(|s| (s.signature().bytes(), s.data().to_vec()))
            .collect(),
        path: path.to_vec(),
    }
}

/// Every record of a type in `sigs`, with its group path.
fn walk(
    entries: &mut [Entry<'_>],
    sigs: &[[u8; 4]],
    path: &mut Vec<(i32, u32)>,
    out: &mut Vec<Rec>,
) {
    for e in entries.iter_mut() {
        match e {
            Entry::Record(r) => {
                if sigs.contains(&r.header.signature.bytes()) {
                    out.push(rec_of(r, path));
                }
            }
            Entry::Group(g) => {
                path.push((g.header.group_type, g.header.label_u32()));
                walk(g.children_mut(), sigs, path, out);
                path.pop();
            }
        }
    }
}

fn even_sample<T: Clone>(list: &[T], count: usize) -> Vec<T> {
    if list.len() <= count {
        return list.to_vec();
    }
    (0..count)
        .map(|i| list[i * list.len() / count].clone())
        .collect()
}

fn sub<'a>(subs: &'a [([u8; 4], Vec<u8>)], sig: &[u8; 4]) -> Option<&'a [u8]> {
    subs.iter()
        .find(|(s, _)| s == sig)
        .map(|(_, d)| d.as_slice())
}

fn ids(b: &[u8]) -> Vec<u32> {
    b.chunks(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn edid_str(subs: &[([u8; 4], Vec<u8>)]) -> Option<String> {
    let d = sub(subs, b"EDID")?;
    Some(String::from_utf8_lossy(d.strip_suffix(&[0u8]).unwrap_or(d)).into_owned())
}

/// The payload the writer gives a copied subrecord: each lstring span holds the string
/// tables' text plus a NUL (ID 0: the NUL only), as `clone_sample_corpus.rs` computes it.
fn expected_payload(
    layout: Option<&fields::Layout>,
    payload: &[u8],
    ctx: &Ctx,
    tables: &mut StringTables,
) -> Result<Vec<u8>, String> {
    let mut next = payload.to_vec();
    for (_name, start, len) in patch::lstring_spans(layout, payload, ctx).into_iter().rev() {
        if start + len > next.len() || len != 4 {
            return Err(format!("lstring span {start}+{len} unusable"));
        }
        let id = u32::from_le_bytes(next[start..start + 4].try_into().unwrap());
        let mut text = if id == 0 {
            Vec::new()
        } else {
            tables
                .lookup("Starfield.esm", id)
                .map_err(|e| format!("lstring {id:08X}: {e}"))?
                .bytes
        };
        text.push(0);
        next.splice(start..start + len, text);
    }
    Ok(next)
}

/// The differences a dialogue clone makes and no other (module docs); `Err` names the
/// first other one.
fn compare(
    src: &Rec,
    clone: &Rec,
    new_edid: &str,
    tables: &mut StringTables,
) -> Result<(), String> {
    let sid = src.hdr.1;
    if clone.sig != src.sig || clone.hdr.1 == sid {
        return Err(format!("{sid:08X}: record type or FormID not as expected"));
    }
    let (sf, _, st, sv, sfv, su) = src.hdr;
    let (cf, _, ct, cv, cfv, cu) = clone.hdr;
    if (cf, ct, cv, cfv, cu) != (sf, st, sv, sfv, su) {
        return Err(format!("{sid:08X}: record header differs"));
    }
    let mut want_edid = new_edid.as_bytes().to_vec();
    want_edid.push(0);
    // The source without its EDID, the clone's EDID first.
    let mut csubs = clone.subs.iter();
    match csubs.next() {
        Some((s, d)) if s == b"EDID" && *d == want_edid => {}
        other => {
            return Err(format!(
                "{sid:08X}: first subrecord {other:?}, not the new EDID"
            ))
        }
    }
    let ssubs: Vec<&([u8; 4], Vec<u8>)> = src.subs.iter().filter(|(s, _)| s != b"EDID").collect();
    let csubs: Vec<&([u8; 4], Vec<u8>)> = csubs.collect();
    if ssubs.len() != csubs.len() {
        return Err(format!(
            "{sid:08X}: {} subrecords besides EDID, the clone {}",
            ssubs.len(),
            csubs.len()
        ));
    }
    let sigs: Vec<[u8; 4]> = src.subs.iter().map(|(s, _)| *s).collect();
    let layouts = fields::layouts(src.sig, &sigs);
    let ctx = Ctx {
        form_version: src.hdr.4,
        localized: true,
        edid_first: sub(&src.subs, b"EDID").and_then(|d| d.first().copied()),
    };
    let offset = usize::from(src.subs.first().is_some_and(|(s, _)| s == b"EDID"));
    for (i, ((ssig, sdata), (csig, cdata))) in
        ssubs.iter().zip(&csubs).map(|(a, b)| (*a, *b)).enumerate()
    {
        let name = String::from_utf8_lossy(ssig).into_owned();
        if ssig != csig {
            return Err(format!(
                "{sid:08X}: subrecord {i} {name} -> {}",
                String::from_utf8_lossy(csig)
            ));
        }
        if src.sig == *b"DIAL" && ssig == b"TIFL" {
            if !cdata.is_empty() {
                return Err(format!("{sid:08X}: TIFL not emptied"));
            }
            continue;
        }
        if src.sig == *b"DIAL" && ssig == b"TIFC" {
            if cdata != &0u32.to_le_bytes() {
                return Err(format!("{sid:08X}: TIFC not 0"));
            }
            continue;
        }
        let layout = layouts.get(i + offset).and_then(Option::as_ref);
        let want = expected_payload(layout, sdata, &ctx, tables)
            .map_err(|e| format!("{sid:08X} {name}: {e}"))?;
        if cdata != &want {
            return Err(format!(
                "{sid:08X} {name}: differs (lengths {} vs {})",
                cdata.len(),
                want.len()
            ));
        }
    }
    Ok(())
}

/// The sample body for `sig` (`DIAL` or `INFO`).
fn sample_dialogue(sig: &str, prefix: &str) {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let data = dir.display().to_string();
    let sig4: [u8; 4] = sig.as_bytes().try_into().unwrap();
    let engine = Engine::with_default_handlers();
    let out = out_dir(&format!("kk-dialogue-clone-{sig}"));
    let into = out.join(format!("kk-dialogue-clone-{sig}.esp"));

    // 1. Every record of the type with its parent (the innermost group's label).
    let src_bytes = std::fs::read(dir.join("Starfield.esm")).unwrap();
    let mut src_plugin = Plugin::parse(&src_bytes).unwrap();
    let mut all: Vec<Rec> = Vec::new();
    let mut sigs = vec![sig4];
    if sig4 == *b"INFO" {
        sigs.push(*b"DIAL");
    }
    walk(src_plugin.entries_mut(), &sigs, &mut Vec::new(), &mut all);
    drop(src_plugin);
    let topics_by_id: HashMap<u32, usize> = all
        .iter()
        .enumerate()
        .filter(|(_, r)| r.sig == *b"DIAL")
        .map(|(i, r)| (r.hdr.1, i))
        .collect();
    let mut list: Vec<(u32, u32)> = all
        .iter()
        .filter(|r| r.sig == sig4)
        .map(|r| (r.hdr.1, r.path.last().unwrap().1))
        .collect();
    list.sort_unstable();
    let sample = even_sample(&list, sample_size());
    eprintln!(
        "dialogue-clone {sig}: Starfield.esm holds {} {sig} records; sampling {}",
        list.len(),
        sample.len()
    );

    // 2. Clone each under its own source parent.
    let mut by_edid: HashMap<String, (u32, u32)> = HashMap::new();
    let mut under: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (i, (id, parent)) in sample.iter().enumerate() {
        let edid = format!("{prefix}Clone_{i:03}");
        let r = engine.execute(
            "clone",
            json!({
                "form": format!("Starfield.esm:{:06X}", id & 0x00FF_FFFF),
                "parent": format!("Starfield.esm:{:06X}", parent & 0x00FF_FFFF),
                "edid": &edid,
                "into": into.display().to_string(),
                "plugins": ["Starfield.esm"],
                "data_dir": &data,
            }),
        );
        if let Err(e) = r {
            panic!("clone {id:08X} ({sig}) under {parent:08X} refused: {e}");
        }
        by_edid.insert(edid.clone(), (*id, *parent));
        under.entry(*parent).or_default().push(edid);
    }

    // 3. Read back and compare.
    let patch_bytes = std::fs::read(&into).unwrap();
    let mut patch_plugin = Plugin::parse(&patch_bytes).unwrap();
    let mut written: Vec<Rec> = Vec::new();
    walk(
        patch_plugin.entries_mut(),
        &[sig4, *b"DIAL"],
        &mut Vec::new(),
        &mut written,
    );
    let sources: HashMap<u32, &Rec> = all
        .iter()
        .filter(|r| r.sig == sig4)
        .map(|r| (r.hdr.1, r))
        .collect();
    let mut tables = StringTables::new(
        "en",
        Box::new(DirSource {
            data_dir: dir.clone(),
        }),
    );
    let mut identical = 0;
    let mut bad: Vec<String> = Vec::new();
    let mut new_ids: HashMap<String, u32> = HashMap::new();
    for c in written.iter().filter(|r| r.sig == sig4) {
        let Some(edid) = edid_str(&c.subs) else {
            continue;
        };
        let Some((src_id, parent)) = by_edid.get(&edid) else {
            continue;
        };
        new_ids.insert(edid.clone(), c.hdr.1);
        let src = sources[src_id];
        // The group path: the quest's type 10 group; a response also its topic's type 7.
        let quest = if sig4 == *b"DIAL" {
            *parent
        } else {
            topics_by_id
                .get(parent)
                .and_then(|i| all[*i].path.last())
                .map(|(_, q)| *q)
                .unwrap()
        };
        let mut want_path = vec![(0, u32::from_le_bytes(*b"QUST")), (10, quest)];
        if sig4 == *b"INFO" {
            want_path.push((7, *parent));
        }
        if c.path != want_path {
            bad.push(format!("{edid}: path {:?}, expected {want_path:?}", c.path));
            continue;
        }
        if sig4 == *b"DIAL" && sub(&c.subs, b"QNAM") != Some(&quest.to_le_bytes()[..]) {
            bad.push(format!("{edid}: QNAM is not the parent quest"));
            continue;
        }
        match compare(src, c, &edid, &mut tables) {
            Ok(()) => identical += 1,
            Err(e) => bad.push(e),
        }
    }
    assert_eq!(new_ids.len(), by_edid.len(), "clones read back");
    // 4. Each carried topic lists its new responses after its own, TIFC the count.
    let mut topics_checked = 0;
    if sig4 == *b"INFO" {
        for (topic, edids) in &under {
            let Some(t) = written
                .iter()
                .find(|r| r.sig == *b"DIAL" && r.hdr.1 == *topic)
            else {
                bad.push(format!("topic {topic:08X} not carried"));
                continue;
            };
            let source = &all[topics_by_id[topic]];
            let mut want = ids(sub(&source.subs, b"TIFL").unwrap_or_default());
            want.extend(edids.iter().map(|e| new_ids[e]));
            let got = ids(sub(&t.subs, b"TIFL").unwrap_or_default());
            let count = sub(&t.subs, b"TIFC").map(|b| u32::from_le_bytes(b.try_into().unwrap()));
            if got != want || count != Some(want.len() as u32) {
                bad.push(format!(
                    "topic {topic:08X}: TIFL {got:08X?} TIFC {count:?}, expected {want:08X?}"
                ));
            }
            topics_checked += 1;
        }
    }
    assert!(
        bad.is_empty(),
        "{sig}: {} difference(s):\n  {}",
        bad.len(),
        bad.join("\n  ")
    );

    // 5. The patch validates and roundtrips.
    let v = engine
        .execute(
            "validate",
            json!({ "plugin": into.display().to_string(), "data_dir": &data }),
        )
        .unwrap();
    assert_eq!(v["data"]["errors"], json!(0), "{:#}", v["data"]);
    let rt = kitkraft_esm::driver::roundtrip_bytes(
        &patch_bytes,
        &kitkraft_esm::driver::RoundtripOptions::default(),
    )
    .unwrap();
    assert!(rt.identical(), "the patch does not roundtrip");
    eprintln!(
        "dialogue-clone {sig}: {} cloned, {identical} identical, {} parents, {topics_checked} topics' TIFL checked, validate {} warning(s)",
        by_edid.len(),
        under.len(),
        v["data"]["warnings"]
    );
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
#[ignore = "reads Starfield.esm; slow"]
fn a_sample_of_official_topics_clones_under_their_quests() {
    sample_dialogue("DIAL", "Dial");
}

#[test]
#[ignore = "reads Starfield.esm; slow"]
fn a_sample_of_official_responses_clones_under_their_topics() {
    sample_dialogue("INFO", "Info");
}
