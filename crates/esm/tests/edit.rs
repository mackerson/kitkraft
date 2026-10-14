//! M2.A6 record edit API on real compressed records (docs/corpus.md): an edited compressed
//! record of `SFBGS004.esm` and `SFBGS047.esm` decompresses back to the edited subrecords,
//! and every other record keeps its bytes.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use kitcraft_esm::index::{index_bytes, IndexOptions};
use kitcraft_esm::{DecodeLimits, Entry, Plugin, Record, Signature, Subrecord};

fn first_compressed<'p, 'a>(entries: &'p mut [Entry<'a>]) -> Option<&'p mut Record<'a>> {
    for entry in entries {
        match entry {
            Entry::Group(g) => {
                if let Some(r) = first_compressed(g.children_mut()) {
                    return Some(r);
                }
            }
            Entry::Record(r) if r.header.is_compressed() => return Some(r),
            Entry::Record(_) => {}
        }
    }
    None
}

#[test]
fn corpus_edit_compressed_records() {
    let Some(files) = common::files(
        "corpus_edit_compressed_records",
        &["SFBGS004.esm", "SFBGS047.esm"],
    ) else {
        return;
    };
    let limits = DecodeLimits::default();
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let record = first_compressed(plugin.entries_mut()).unwrap();
        let (signature, form_id) = (record.header.signature, record.header.form_id);

        // Set, insert, remove.
        let n = record.edit_subrecords(&limits).unwrap().len();
        assert!(n >= 2, "{}", file.name);
        assert!(record
            .set_subrecord(0, &b"kitcraft edit\0"[..], &limits)
            .unwrap());
        record
            .insert_subrecord(
                1,
                Subrecord::new(Signature::new(b"ZZZZ"), vec![7u8; 9]),
                &limits,
            )
            .unwrap();
        let removed = record.remove_subrecord(n, &limits).unwrap();
        let expected: Vec<(Signature, Vec<u8>)> = record
            .subrecords()
            .unwrap()
            .iter()
            .map(|s| (s.signature(), s.data().to_vec()))
            .collect();
        assert_eq!(expected.len(), n);
        assert_ne!(removed.signature(), Signature::new(b"ZZZZ"));

        let out = plugin.to_bytes().unwrap();
        let before = index_bytes(&bytes, &IndexOptions::default()).unwrap();
        let after = index_bytes(&out, &IndexOptions::default()).unwrap();
        assert_eq!(before.records.len(), after.records.len());
        let mut edited = 0;
        for (a, b) in before.records.iter().zip(&after.records) {
            assert_eq!((a.signature, a.form_id), (b.signature, b.form_id));
            let src = &bytes[a.offset as usize..(a.offset + a.len()) as usize];
            let dst = &out[b.offset as usize..(b.offset + b.len()) as usize];
            let is_edited = a.signature == signature && a.form_id == form_id;
            let is_header = a.signature == Signature::TES4;
            if is_edited {
                edited += 1;
                assert!(b.is_compressed());
                let back = kitcraft_esm::index::read_indexed(
                    &mut std::io::Cursor::new(&out),
                    b,
                    Some(&limits),
                )
                .unwrap();
                let got: Vec<(Signature, Vec<u8>)> = back
                    .subrecords()
                    .unwrap()
                    .iter()
                    .map(|s| (s.signature(), s.data().to_vec()))
                    .collect();
                assert_eq!(got, expected, "{}", file.name);
            } else if !is_header {
                assert_eq!(src, dst, "{} {:08X}", file.name, a.form_id);
            }
        }
        assert_eq!(edited, 1);
        // Same number of records and groups: the header is written as read.
        assert_eq!(
            out[..before.records[0].len() as usize],
            bytes[..before.records[0].len() as usize]
        );
    }
}
