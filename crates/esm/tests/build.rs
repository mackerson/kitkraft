//! M2.A5 plugin builder and streaming upsert on the official ESMs (docs/corpus.md).
//!
//! - `corpus_top_group_order`: every file's top-level groups appear in
//!   `build::TOP_GROUP_ORDER` order, and `Starfield.esm`'s are that table without the five
//!   types only other files have.
//! - `corpus_small_replace_one_record`: replacing one record of each small file while
//!   streaming the rest changes only that record and the sizes of the groups above it.
//! - `corpus_small_records_into_a_new_plugin`: records copied from small files into a new
//!   plugin keep their bytes, and the plugin parses and roundtrips identically.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::io::{Cursor, Read, Seek, SeekFrom};

use kitcraft_esm::build::{upsert_reader, HeaderSpec, Placed, Upsert, TOP_GROUP_ORDER};
use kitcraft_esm::driver::{roundtrip_bytes, RewriteOptions, RoundtripOptions};
use kitcraft_esm::index::{index_bytes, read_indexed, IndexOptions, PluginIndex, RecordLoc};
use kitcraft_esm::{DecodeLimits, Plugin, Record, Signature};

/// Top-level group labels of the file at `path`, from their headers alone.
fn top_labels(path: &std::path::Path) -> Vec<[u8; 4]> {
    let mut f = std::fs::File::open(path).unwrap();
    let len = f.metadata().unwrap().len();
    let mut head = [0u8; 24];
    f.read_exact(&mut head).unwrap();
    let mut at = 24 + u64::from(u32::from_le_bytes(head[4..8].try_into().unwrap()));
    let mut labels = Vec::new();
    while at < len {
        f.seek(SeekFrom::Start(at)).unwrap();
        f.read_exact(&mut head).unwrap();
        assert_eq!(&head[..4], b"GRUP");
        assert_eq!(i32::from_le_bytes(head[12..16].try_into().unwrap()), 0);
        labels.push(head[8..12].try_into().unwrap());
        at += u64::from(u32::from_le_bytes(head[4..8].try_into().unwrap()));
    }
    labels
}

#[test]
fn corpus_top_group_order() {
    let Some(files) = common::all_files("corpus_top_group_order") else {
        return;
    };
    for file in &files {
        let labels = top_labels(&file.path);
        let ranks: Vec<usize> = labels
            .iter()
            .map(|l| {
                TOP_GROUP_ORDER
                    .iter()
                    .position(|s| s == l)
                    .unwrap_or_else(|| panic!("{}: {} not in the table", file.name, Signature(*l)))
            })
            .collect();
        assert!(
            ranks.windows(2).all(|w| w[0] < w[1]),
            "{}: top-level groups out of table order",
            file.name
        );
        if file.name == "Starfield.esm" {
            let others = [*b"PERS", *b"SCCO", *b"GPOF", *b"GPOG", *b"GWED"];
            let expected: Vec<[u8; 4]> = TOP_GROUP_ORDER
                .iter()
                .copied()
                .filter(|s| !others.contains(s))
                .collect();
            assert_eq!(labels, expected);
        }
    }
}

/// A record at the deepest group level of the index (first of them), and one directly in
/// a top-level group.
fn picks(index: &PluginIndex) -> Vec<&RecordLoc> {
    let depth = |l: &RecordLoc| index.group_path(l.group).len();
    let mut out = Vec::new();
    let deepest = index.records.iter().map(depth).max().unwrap_or(0);
    if let Some(l) = index.records.iter().find(|l| depth(l) == deepest) {
        out.push(l);
    }
    if let Some(l) = index
        .records
        .iter()
        .find(|l| depth(l) == 1 && !out.iter().any(|o| o.offset == l.offset))
    {
        out.push(l);
    }
    if let Some(l) = index.records.iter().find(|l| l.is_compressed()) {
        out.push(l);
    }
    out
}

/// Grow the record's first subrecord by `extra` bytes (decompressing if needed).
fn grown(mut record: Record<'static>, extra: usize) -> Record<'static> {
    record.decompress(&DecodeLimits::default()).unwrap();
    let first = &mut record.subrecords_mut().unwrap()[0];
    let mut data = first.data().to_vec();
    data.extend(std::iter::repeat_n(b'k', extra));
    first.set_data(data);
    record
}

#[test]
fn corpus_small_replace_one_record() {
    let Some(files) = common::files("corpus_small_replace_one_record", &common::SMALL) else {
        return;
    };
    let mut checked = 0;
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
        for loc in picks(&index) {
            let original = read_indexed(&mut Cursor::new(&bytes), loc, None).unwrap();
            let record = grown(original, 5);
            let path = index.group_headers(loc);
            let mut out = Cursor::new(Vec::new());
            let report = upsert_reader(
                Cursor::new(&bytes),
                bytes.len() as u64,
                &RewriteOptions::default(),
                &mut out,
                vec![Placed { path, record }],
                |_| Ok(()),
            )
            .unwrap();
            assert_eq!(report.outcomes, [Upsert::Replaced], "{}", file.name);
            assert_eq!(report.rewrite.hedr_fix, None);
            assert_eq!(report.rewrite.counts_read, report.rewrite.counts_written);
            let out = out.into_inner();

            // The new record's length, from its own header.
            let at = loc.offset as usize;
            let new_len = 24 + u32::from_le_bytes(out[at + 4..at + 8].try_into().unwrap()) as usize;
            let old_len = loc.len() as usize;
            let delta = new_len as i64 - old_len as i64;
            assert_eq!(out.len() as i64, bytes.len() as i64 + delta);
            if !loc.is_compressed() {
                assert_eq!(delta, 5);
            }
            // After the record: identical.
            assert_eq!(out[at + new_len..], bytes[at + old_len..], "{}", file.name);
            // Before it: identical except each enclosing group's size field.
            let mut expected = bytes[..at].to_vec();
            for g in index.group_path(loc.group) {
                let o = g.offset as usize + 4;
                let size = (i64::from(g.size) + delta) as u32;
                expected[o..o + 4].copy_from_slice(&size.to_le_bytes());
            }
            assert_eq!(out[..at], expected[..], "{}", file.name);
            // The record itself reads back as edited.
            let reparsed = index_bytes(&out, &IndexOptions::default()).unwrap();
            let back = read_record_back(&out, &reparsed, loc.form_id);
            let first = &back.subrecords().unwrap()[0];
            assert!(first.data().ends_with(b"kkkkk"), "{}", file.name);
            checked += 1;
        }
    }
    assert!(checked >= 10, "{checked}");
}

fn read_record_back(bytes: &[u8], index: &PluginIndex, form_id: u32) -> Record<'static> {
    let loc = index
        .records
        .iter()
        .find(|l| l.form_id == form_id && l.signature != Signature::TES4)
        .unwrap();
    read_indexed(&mut Cursor::new(bytes), loc, Some(&DecodeLimits::default())).unwrap()
}

#[test]
fn corpus_small_records_into_a_new_plugin() {
    let Some(files) = common::files("corpus_small_records_into_a_new_plugin", &common::SMALL)
    else {
        return;
    };
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
        let tes4 = kitcraft_esm::tes4::Tes4::new(&index.header);
        // The source's masters, then the source itself, as a patch would list them.
        let mut masters: Vec<Vec<u8>> = tes4.masters().into_iter().map(|m| m.name).collect();
        masters.push(file.name.as_bytes().to_vec());
        let mut plugin = Plugin::create(&HeaderSpec::new(masters.clone()));
        let mut copied = Vec::new();
        for loc in picks(&index) {
            let record = read_indexed(&mut Cursor::new(&bytes), loc, None).unwrap();
            let outcome = plugin.upsert(&index.group_headers(loc), record).unwrap();
            assert_eq!(outcome, Upsert::Inserted);
            copied.push(loc);
        }
        let out = plugin.to_bytes().unwrap();
        assert!(
            roundtrip_bytes(&out, &RoundtripOptions::with_decode())
                .unwrap()
                .identical(),
            "{}",
            file.name
        );
        let new_index = index_bytes(&out, &IndexOptions::default()).unwrap();
        assert_eq!(new_index.records.len(), copied.len() + 1);
        let parsed = Plugin::parse(&out).unwrap();
        let names: Vec<Vec<u8>> = parsed
            .tes4()
            .masters()
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, masters);
        assert_eq!(
            parsed.tes4().hedr().unwrap().unwrap().record_count as usize,
            copied.len() + 1
        );
        for loc in copied {
            // Copied records keep their bytes and their group path.
            let new = new_index
                .records
                .iter()
                .find(|l| l.form_id == loc.form_id && l.signature == loc.signature)
                .unwrap();
            let src = &bytes[loc.offset as usize..(loc.offset + loc.len()) as usize];
            let dst = &out[new.offset as usize..(new.offset + new.len()) as usize];
            assert_eq!(src, dst, "{}", file.name);
            let path = |i: &PluginIndex, l: &RecordLoc| -> Vec<(i32, [u8; 4])> {
                i.group_path(l.group)
                    .iter()
                    .map(|g| (g.header.group_type, g.header.label))
                    .collect()
            };
            assert_eq!(path(&index, loc), path(&new_index, new), "{}", file.name);
        }
    }
}
