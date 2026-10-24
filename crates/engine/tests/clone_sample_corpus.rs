//! KK-004: `clone` on a sample of the official `Starfield.esm` (`KITKRAFT_DATA_DIR`;
//! skipped with a printed line when unset).
//!
//! Each test lists every record of one type (the `query` command), takes a
//! deterministic 100-record sample spread evenly over the list (fixed stride,
//! no randomness), clones each into one patch plugin (the `clone` command,
//! new EditorID per clone), reads the patch back (`kitkraft_esm::Plugin`), and
//! compares each clone with its source subrecord by subrecord. Per the clone
//! handler (`src/handlers/newform.rs`) a clone changes exactly: the record's
//! FormID (a new allocation), the `EDID` (a new EditorID plus NUL), and,
//! because Starfield.esm is localized and the patch is not, every lstring
//! span inlined from the string tables (the language-en text, not the 4-byte
//! ID). Every other subrecord must be byte-identical, in the same order, same
//! count; every other record header field is copied as stored. A compressed
//! source record is recompressed on write, so the comparison is at the
//! decoded-subrecord level. The patch must validate with 0 errors.
//!
//! Run: `cargo test -p kitkraft-engine --test clone_sample_corpus -- --include-ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::HashMap;
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

/// The patch's home: a fresh dir under `CARGO_TARGET_TMPDIR` if set.
fn out_dir(name: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sig_name(s: [u8; 4]) -> String {
    String::from_utf8_lossy(&s).into_owned()
}

/// Every record of `sig` in Starfield.esm (paged `query`): (form label, FormID).
fn list_records(engine: &Engine, data: &str, sig: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = json!({
            "type": sig,
            "plugins": ["Starfield.esm"],
            "data_dir": data,
            "limit": 1000,
        });
        if let Some(c) = &cursor {
            args["cursor"] = json!(c);
        }
        let r = engine
            .execute("query", args)
            .unwrap_or_else(|e| panic!("query {sig}: {e}"));
        let d = &r["data"];
        for item in d["results"].as_array().unwrap() {
            let form = item["form"].as_str().unwrap().to_string();
            let id = u32::from_str_radix(item["raw_form_id"].as_str().unwrap(), 16).unwrap();
            out.push((form, id));
        }
        cursor = d["next_cursor"].as_str().map(str::to_string);
        if cursor.is_none() {
            break;
        }
    }
    out.sort_by_key(|(_, id)| *id);
    out
}

/// Up to `count` records spread evenly over the list (fixed stride; index
/// `i * len / count` never reaches `len`).
fn even_sample(list: &[(String, u32)], count: usize) -> Vec<(String, u32)> {
    if list.len() <= count {
        return list.to_vec();
    }
    (0..count)
        .map(|i| list[i * list.len() / count].clone())
        .collect()
}

/// What the comparison needs from one parsed record (owned copies; the parse
/// tree is dropped afterwards).
struct Rec {
    sig: [u8; 4],
    /// (flags, FormID, timestamp, version control, form version, unknown).
    hdr: (u32, u32, u16, u16, u16, u16),
    /// Subrecords in order: (signature, payload as stored).
    subs: Vec<([u8; 4], Vec<u8>)>,
}

fn rec_of(r: &mut Record) -> Rec {
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
    }
}

fn walk<'a>(entries: &mut [Entry<'a>], sig: [u8; 4], out: &mut Vec<Rec>) {
    for e in entries.iter_mut() {
        match e {
            Entry::Record(r) => {
                if r.header.signature.bytes() == sig {
                    out.push(rec_of(r));
                }
            }
            Entry::Group(g) => walk(g.children_mut(), sig, out),
        }
    }
}

fn edid_bytes(subs: &[([u8; 4], Vec<u8>)]) -> Option<&[u8]> {
    subs.iter()
        .find(|(s, _)| *s == *b"EDID")
        .map(|(_, d)| d.as_slice())
}

fn edid_str(subs: &[([u8; 4], Vec<u8>)]) -> Option<String> {
    let d = edid_bytes(subs)?;
    Some(String::from_utf8_lossy(d.strip_suffix(&[0u8]).unwrap_or(d)).into_owned())
}

fn first_diff(a: &[u8], b: &[u8]) -> usize {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()))
}

/// The writer's lstring inlining (`edit/patch.rs` `inline_lstrings`): each
/// lstring span of the subrecord (its schema layout, the source record's
/// context) holds the string tables' language-en text plus a NUL where the
/// source held the 4-byte ID; ID 0 becomes a single NUL. Spans are applied in
/// reverse so earlier offsets stay valid.
fn expected_payload(
    layout: Option<&fields::Layout>,
    payload: &[u8],
    ctx: &Ctx,
    tables: &mut StringTables,
) -> Result<Vec<u8>, String> {
    let mut next = payload.to_vec();
    for (_name, start, len) in patch::lstring_spans(layout, payload, ctx).into_iter().rev() {
        if start + len > next.len() {
            return Err("lstring span out of range".into());
        }
        let bytes = &next[start..start + len];
        if bytes.len() != 4 {
            return Err(format!("lstring is {len} bytes, not 4"));
        }
        let id = u32::from_le_bytes(bytes.try_into().unwrap());
        let text = if id == 0 {
            vec![0]
        } else {
            let r = tables
                .lookup("Starfield.esm", id)
                .map_err(|e| format!("lstring {id:08X}: {e}"))?;
            r.bytes
        };
        let mut w = text;
        w.push(0);
        next.splice(start..start + len, w);
    }
    Ok(next)
}

/// A difference a clone must not make: where it is, and what it says.
struct Mismatch {
    /// The source record's FormID (keys the exceptions list).
    form_id: u32,
    /// The subrecord it is in; `RECORD` for header/type-level differences.
    subrecord: String,
    detail: String,
}

/// Explained exceptions: (source FormID, subrecord, one-line reason). Only
/// differences matching this list count as `explained` in the printed line;
/// every other difference fails the test (TICKET.md item (c)). Each entry
/// gets a row in `docs/gaps.md`; the list stays short on purpose.
const EXCEPTIONS: &[(u32, &str, &str)] = &[];

/// Compare a clone with its source: `Ok` if the differences are exactly the
/// changes a clone makes (new FormID, new EDID, inlined lstrings); `Err`
/// names the first difference that is not one of them.
fn compare(
    src: &Rec,
    clone: &Rec,
    new_edid: &str,
    tables: &mut StringTables,
) -> Result<(), Mismatch> {
    let sid = src.hdr.1;
    if clone.sig != src.sig {
        return Err(Mismatch {
            form_id: sid,
            subrecord: "RECORD".into(),
            detail: format!(
                "record type {} -> {}",
                sig_name(src.sig),
                sig_name(clone.sig)
            ),
        });
    }
    // A clone changes the FormID (new allocation); every other header field is
    // copied as stored (newform.rs).
    let (sf, _, st, sv, sfv, su) = src.hdr;
    let (cf, cid, ct, cv, cfv, cu) = clone.hdr;
    if cid == sid {
        return Err(Mismatch {
            form_id: sid,
            subrecord: "RECORD".into(),
            detail: "the FormID was not changed".into(),
        });
    }
    if (cf, ct, cv, cfv, cu) != (sf, st, sv, sfv, su) {
        return Err(Mismatch {
            form_id: sid,
            subrecord: "RECORD".into(),
            detail: format!(
                "record header: (flags {sf:08X}, ts {st}, vc {sv:04X}, fv {sfv}, unk {su:04X}) -> (flags {cf:08X}, ts {ct}, vc {cv:04X}, fv {cfv}, unk {cu:04X})"
            ),
        });
    }
    if clone.subs.len() != src.subs.len() {
        return Err(Mismatch {
            form_id: sid,
            subrecord: "RECORD".into(),
            detail: format!(
                "subrecord count: {} -> {}",
                src.subs.len(),
                clone.subs.len()
            ),
        });
    }
    let sub_sigs: Vec<[u8; 4]> = src.subs.iter().map(|(s, _)| *s).collect();
    let layouts = fields::layouts(src.sig, &sub_sigs);
    let ctx = Ctx {
        form_version: src.hdr.4,
        localized: true,
        edid_first: edid_bytes(&src.subs).and_then(|d| d.first().copied()),
    };
    for (i, ((ssig, sdata), (csig, cdata))) in src.subs.iter().zip(&clone.subs).enumerate() {
        let name = sig_name(*ssig);
        if ssig != csig {
            return Err(Mismatch {
                form_id: sid,
                subrecord: name,
                detail: format!("type {ssig:?} -> {csig:?}"),
            });
        }
        if *ssig == *b"EDID" {
            let mut want = new_edid.as_bytes().to_vec();
            want.push(0);
            if cdata != &want {
                return Err(Mismatch {
                    form_id: sid,
                    subrecord: "EDID".into(),
                    detail: format!("expected {new_edid:?}, got {cdata:?}"),
                });
            }
            continue;
        }
        let want =
            expected_payload(layouts[i].as_ref(), sdata, &ctx, tables).map_err(|e| Mismatch {
                form_id: sid,
                subrecord: name.clone(),
                detail: format!("lstring inlining: {e}"),
            })?;
        if cdata != &want {
            let at = first_diff(cdata, &want);
            let byte = |b: Option<u8>| {
                b.map(|b| format!("{b:02X}"))
                    .unwrap_or_else(|| "<end>".into())
            };
            return Err(Mismatch {
                form_id: sid,
                subrecord: name,
                detail: format!(
                    "first diff at byte {} (clone {}, expected {}; lengths {} vs {})",
                    at,
                    byte(cdata.get(at).copied()),
                    byte(want.get(at).copied()),
                    cdata.len(),
                    want.len()
                ),
            });
        }
    }
    Ok(())
}

/// The sample-corpus body: query, even sample of 100, clone each into one
/// patch, compare subrecord by subrecord, validate, print the counts.
fn sample_clones(sig: &str, prefix: &str) {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let data = dir.display().to_string();
    let sig4 = sig.as_bytes().try_into().unwrap();
    let engine = Engine::with_default_handlers();
    let out = out_dir(&format!("kk-clone-{sig}"));
    let into = out.join(format!("kk-clone-{sig}.esp"));

    // 1. List every record of the type, sort by FormID, take an even sample.
    let all = list_records(&engine, &data, sig);
    let sample = even_sample(&all, 100);
    eprintln!(
        "clone-sample {sig}: Starfield.esm holds {} {sig} records; sampling {}",
        all.len(),
        sample.len()
    );

    // 2. Clone each into one patch, a new EditorID per clone. A refused clone
    // fails the test: a sample corpus must not silently shrink.
    let mut src_by_edid: HashMap<String, (String, u32)> = HashMap::new();
    for (i, (form, id)) in sample.iter().enumerate() {
        let edid = format!("{prefix}Clone_{i:03}");
        let r = engine.execute(
            "clone",
            json!({
                "form": form,
                "edid": &edid,
                "into": into.display().to_string(),
                "plugins": ["Starfield.esm"],
                "data_dir": &data,
            }),
        );
        match r {
            Ok(_) => {
                src_by_edid.insert(edid, (form.clone(), *id));
            }
            Err(e) => panic!("clone {form} ({sig}) refused: {e}"),
        }
    }

    // 3. Read the patch back and compare each clone with its source.
    let patch_bytes = std::fs::read(&into).unwrap();
    let mut patch =
        Plugin::parse(&patch_bytes).unwrap_or_else(|e| panic!("parse {}: {e}", into.display()));
    let mut clones: Vec<Rec> = Vec::new();
    walk(patch.entries_mut(), sig4, &mut clones);
    assert_eq!(clones.len(), src_by_edid.len(), "patch record count");

    let src_bytes = std::fs::read(dir.join("Starfield.esm")).unwrap();
    let mut src_plugin =
        Plugin::parse(&src_bytes).unwrap_or_else(|e| panic!("parse Starfield.esm: {e}"));
    let mut sources: Vec<Rec> = Vec::new();
    walk(src_plugin.entries_mut(), sig4, &mut sources);
    let sources_by_id: HashMap<u32, &Rec> = sources.iter().map(|r| (r.hdr.1, r)).collect();

    let mut tables = StringTables::new(
        "en",
        Box::new(DirSource {
            data_dir: dir.clone(),
        }),
    );
    let mut identical = 0;
    let mut explained = 0;
    let mut unexplained: Vec<String> = Vec::new();
    for c in &clones {
        let Some(new_edid) = edid_str(&c.subs) else {
            panic!("{}: the clone has no EDID subrecord", sig_name(c.sig));
        };
        let Some((_, src_id)) = src_by_edid.get(&new_edid) else {
            panic!("{new_edid}: unknown EditorID in the patch");
        };
        let Some(src) = sources_by_id.get(src_id) else {
            panic!("{src_id:08X}: source record not found in Starfield.esm");
        };
        match compare(src, c, &new_edid, &mut tables) {
            Ok(()) => identical += 1,
            Err(m) => {
                let known = EXCEPTIONS
                    .iter()
                    .find(|(id, sub, _)| *id == m.form_id && *sub == m.subrecord);
                match known {
                    Some((_, _, reason)) => {
                        explained += 1;
                        eprintln!(
                            "  explained: {src_id:08X} {}: {reason} — {}",
                            m.subrecord, m.detail
                        );
                    }
                    None => unexplained.push(format!("{src_id:08X} {}: {}", m.subrecord, m.detail)),
                }
            }
        }
    }
    assert!(
        unexplained.is_empty(),
        "{sig}: {} difference(s) not in the exceptions list:\n  {}",
        unexplained.len(),
        unexplained.join("\n  ")
    );

    // 4. The patch validates (the data dir lets it find the master).
    let v = engine
        .execute(
            "validate",
            json!({ "plugin": into.display().to_string(), "data_dir": &data }),
        )
        .unwrap_or_else(|e| panic!("validate: {e}"));
    assert_eq!(v["data"]["errors"], json!(0), "{:#}", v["data"]);

    // 5.
    eprintln!(
        "clone-sample {sig}: {} cloned, {} identical, {} explained",
        src_by_edid.len(),
        identical,
        explained
    );
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
#[ignore = "reads Starfield.esm; slow"]
fn a_sample_of_official_locations_clones_identically() {
    sample_clones("LCTN", "Lctn");
}

#[test]
#[ignore = "reads Starfield.esm; slow"]
fn a_sample_of_official_terminals_clones_identically() {
    sample_clones("TERM", "Term");
}
