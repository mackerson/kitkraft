//! The KSSM definition (SoundKeywordMapping) against the official ESMs, read in place
//! through the record index (see docs/corpus.md and `crates/schema/defs/README.md`).
//!
//! Every KSSM record is read with [`read_indexed`] (compressed ones decompressed), decoded
//! through its generated type and encoded again. The checks:
//!
//! - every subrecord is placed in a member (nothing left unnamed);
//! - every placed subrecord decodes (nothing placed fails);
//! - the encoding reproduces the record subrecord by subrecord (signature, payload,
//!   `XXXX`).
//!
//! It prints `kssm KSSM: <n> records, <m> identical` (plain integers); per signature, how
//! many records have it, the payload size range of each signature and its occurrences;
//! the keyword and reverb-entry totals with their per-record maxima, and how many records
//! carry the ReverbCount subrecord and the ReverbSounds group.
//!
//! `kssm_types_full` also pins the values of one record: Starfield.esm FormID
//! 0x00000B54 (EditorID SKMHandRifleOrionPCSemiAmplifier) must decode to the keywords,
//! count, classes and sound references the ticket names, read from the typed fields.
//!
//! - `kssm_types_full` (ignored) runs all 14 files, every record; run it in release:
//!   `cargo test -p kitkraft-esm --release --test kssm_types -- --ignored --nocapture`

// Test-only code: a panic fails the test.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used
)]

mod common;

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::{
    match_body, KssmReverbSoundsReverbClass, Record as Typed, RECORDS,
};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::FLAG_LOCALIZED;
use kitkraft_esm::DecodeLimits;

const SIG: [u8; 4] = *b"KSSM";

#[derive(Default)]
struct Stats {
    records: u64,
    identical: u64,
    /// How many records carry at least one subrecord of each signature.
    sigs: BTreeMap<String, u64>,
    /// Per signature, the total number of subrecord occurrences.
    occurrences: BTreeMap<String, u64>,
    /// Per signature, the payload size range (min, max) in bytes, over all occurrences.
    sizes: BTreeMap<String, (usize, usize)>,
    /// Keyword FormIDs (one per KNAM) over all records, and the most in one record.
    keywords_total: u64,
    keywords_max: usize,
    /// Reverb entries (one per RSMC/RSMH pair) over all records, and the most in one
    /// record.
    entries_total: u64,
    entries_max: usize,
    /// Records carrying the leading RSMC count, and records with a ReverbSounds group.
    with_reverb_count: u64,
    with_reverb_sounds: u64,
    /// True once the pinned record 0x00000B54 decoded and its values were checked.
    value_checked: bool,
    unplaced: BTreeMap<String, u64>,
    failed: BTreeMap<String, u64>,
    failures: Vec<String>,
}

fn key(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

fn check_record(name: &str, form_id: u32, subs: &[RawSub<'_>], ctx: Ctx, t: &mut Stats) {
    let Some(typed) = Typed::decode(SIG, subs, ctx) else {
        panic!("{name} {form_id:08X}: no definition for KSSM");
    };
    let Typed::Kssm(r) = &typed else {
        panic!("{name} {form_id:08X}: not a KSSM");
    };
    let mut seen = BTreeSet::new();
    for sub in subs {
        let k = key(sub.signature);
        if seen.insert(k.clone()) {
            *t.sigs.entry(k.clone()).or_insert(0) += 1;
        }
        *t.occurrences.entry(k.clone()).or_insert(0) += 1;
        let len = sub.data.len();
        match t.sizes.entry(k) {
            Entry::Occupied(mut e) => {
                e.get_mut().0 = e.get().0.min(len);
                e.get_mut().1 = e.get().1.max(len);
            }
            Entry::Vacant(e) => {
                e.insert((len, len));
            }
        }
    }
    t.keywords_total += r.keywords.len() as u64;
    t.keywords_max = t.keywords_max.max(r.keywords.len());
    t.entries_total += r.reverb_sounds.len() as u64;
    t.entries_max = t.entries_max.max(r.reverb_sounds.len());
    if r.reverb_count.is_some() {
        t.with_reverb_count += 1;
    }
    if !r.reverb_sounds.is_empty() {
        t.with_reverb_sounds += 1;
    }
    if name == "Starfield.esm" && form_id == 0x00000B54 {
        let keywords = &r.keywords;
        assert_eq!(keywords.len(), 3);
        assert_eq!(keywords[0].value.0, 0x0023D5FD);
        let count = r.reverb_count.as_ref().expect("ReverbCount");
        assert_eq!(count.value, 6);
        assert_eq!(r.reverb_sounds.len(), 6);
        let first_class = r.reverb_sounds[0]
            .reverb_class
            .as_ref()
            .expect("ReverbClass");
        assert_eq!(first_class.value, KssmReverbSoundsReverbClass::ClassE);
        assert_eq!(first_class.value.to_raw(), 5);
        let second_class = r.reverb_sounds[1]
            .reverb_class
            .as_ref()
            .expect("ReverbClass");
        assert_eq!(second_class.value, KssmReverbSoundsReverbClass::ClassD);
        assert_eq!(second_class.value.to_raw(), 4);
        let sound = r.sound.as_ref().expect("Sound");
        assert_eq!(sound.value.start_event[0], 0xC4);
        let first_sound = r.reverb_sounds[0].sound.as_ref().expect("Sound");
        assert_eq!(first_sound.value.start_event[0], 0x2F);
        t.value_checked = true;
    }
    let members = RECORDS
        .iter()
        .find(|(s, _)| *s == SIG)
        .map(|(_, m)| *m)
        .unwrap();
    let (_, unmatched) = match_body(SIG, subs, members);
    for i in &unmatched {
        let k = key(subs[*i].signature);
        *t.unplaced.entry(k.clone()).or_insert(0) += 1;
        if t.failures.len() < 20 {
            t.failures.push(format!(
                "{name} {form_id:08X}: {k} at {i} matches no member"
            ));
        }
    }
    for (i, sub) in typed.extra() {
        if !unmatched.contains(i) {
            let k = key(sub.signature);
            *t.failed.entry(k.clone()).or_insert(0) += 1;
            if t.failures.len() < 20 {
                t.failures.push(format!(
                    "{name} {form_id:08X}: {k} at {i} ({} bytes) does not decode",
                    sub.data.len()
                ));
            }
        }
    }
    let encoded = typed.encode();
    let same = encoded.len() == subs.len()
        && encoded.iter().zip(subs).all(|(e, r)| {
            e.signature == r.signature && e.data == r.data && e.oversized == r.oversized
        });
    if same {
        t.identical += 1;
    } else if t.failures.len() < 20 {
        t.failures
            .push(format!("{name} {form_id:08X}: re-encoding differs"));
    }
}

fn check_file(path: &Path, t: &mut Stats) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let index = index_file(path, &IndexOptions::default()).unwrap();
    let localized = index.header.header.flags & FLAG_LOCALIZED != 0;
    let limits = DecodeLimits::default();
    let mut input = BufReader::new(File::open(path).unwrap());
    for loc in &index.records {
        let sig = loc.signature.bytes();
        if sig != SIG {
            continue;
        }
        let record = read_indexed(&mut input, loc, Some(&limits)).unwrap();
        t.records += 1;
        let subs: Vec<RawSub<'_>> = record
            .subrecords()
            .expect("decoded")
            .iter()
            .map(RawSub::from)
            .collect();
        let ctx = Ctx::for_record(&record, localized);
        check_record(&name, loc.form_id, &subs, ctx, t);
    }
}

fn report(t: &Stats) {
    println!(
        "kssm KSSM: {} records, {} identical",
        t.records, t.identical
    );
    if !t.unplaced.is_empty() || !t.failed.is_empty() {
        println!("   unplaced: {:?} failed: {:#?}", t.unplaced, t.failed);
    }
    for f in &t.failures {
        println!("   ! {f}");
    }
    // Per signature: how many records have it.
    for (sig, n) in &t.sigs {
        println!("kssm KSSM {sig}: {n} records");
    }
    for (sig, (lo, hi)) in &t.sizes {
        if lo == hi {
            println!("   {sig} payload: {lo} bytes");
        } else {
            println!("   {sig} payload: {lo}..{hi} bytes");
        }
    }
    for (sig, n) in &t.occurrences {
        println!("   {sig} occurrences: {n}");
    }
    println!(
        "   Keywords: {} occurrences, up to {} per record",
        t.keywords_total, t.keywords_max
    );
    println!("   ReverbCount: {} records", t.with_reverb_count);
    println!(
        "   ReverbSounds: {} records, {} entries, up to {} per record",
        t.with_reverb_sounds, t.entries_total, t.entries_max
    );
}

fn assert_clean(t: &Stats) {
    assert!(t.unplaced.is_empty(), "unplaced {:?}", t.unplaced);
    assert!(t.failed.is_empty(), "failed {:?}", t.failed);
    assert_eq!(t.identical, t.records, "re-encode differs");
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn kssm_types_full() {
    let Some(files) = common::all_files("kssm_types_full") else {
        return;
    };
    let mut t = Stats::default();
    for f in &files {
        check_file(&f.path, &mut t);
    }
    report(&t);
    assert!(t.records > 0, "no KSSM record in the official ESMs");
    assert_clean(&t);
    assert!(t.value_checked, "0x00000B54 not seen");
    println!("kssm value check: ok");
}
