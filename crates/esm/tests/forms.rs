//! M4.A3 new-form allocator on the official ESMs: small and medium files are refused, and a
//! full-scale file's first new FormID is above every own object ID it has and above its
//! `HEDR` next object ID.
//!
//! `cargo test -p kitcraft-esm --release --test forms -- --ignored --nocapture`

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use kitcraft_esm::forms::{NewForms, MAX_FULL_OBJECT_ID};
use kitcraft_esm::index::{index_file, IndexOptions};
use kitcraft_esm::tes4::{Tes4, FLAG_MEDIUM, FLAG_SMALL};
use kitcraft_esm::Error;

fn check(names: &[&str]) {
    let Some(files) = common::files("forms", names) else {
        return;
    };
    for file in &files {
        let index = index_file(&file.path, &IndexOptions::default()).unwrap();
        let tes4 = Tes4::new(&index.header);
        let scaled = tes4.flags() & (FLAG_SMALL | FLAG_MEDIUM) != 0;
        let forms = NewForms::for_index(&index);
        if scaled {
            assert!(matches!(forms, Err(Error::Refused { .. })), "{}", file.name);
            println!("{}: refused (small or medium)", file.name);
            continue;
        }
        let forms = forms.unwrap();
        let first = forms.peek().unwrap();
        let prefix = u32::try_from(tes4.masters().len()).unwrap();
        assert_eq!(first >> 24, prefix, "{}", file.name);
        let hedr = tes4.hedr().unwrap().unwrap().next_object_id & MAX_FULL_OBJECT_ID;
        assert!(first & MAX_FULL_OBJECT_ID >= hedr, "{}", file.name);
        for r in index.records.iter().skip(1) {
            if r.form_id >> 24 == prefix {
                assert!(r.form_id < first, "{}: own {:08X}", file.name, r.form_id);
            }
        }
        println!(
            "{}: first new FormID {first:08X} (HEDR next {hedr:06X})",
            file.name
        );
    }
}

#[test]
fn corpus_small_files_are_refused() {
    check(&common::SMALL);
}

#[test]
#[ignore]
fn corpus_new_forms_full() {
    let names: Vec<String> = common::manifest().into_iter().map(|e| e.name).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    check(&names);
}
