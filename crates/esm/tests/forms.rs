//! M4.A3 new-form allocator on the official ESMs: small and medium files are refused, and a
//! full-scale file's first new FormID has the file's full-master count as its high byte and
//! is above every own object ID it has and above its `HEDR` next object ID. The masters'
//! scales come from their own `TES4` headers in the data directory.
//!
//! `cargo test -p kitcraft-esm --release --test forms -- --ignored --nocapture`

#![allow(clippy::unwrap_used, clippy::panic, clippy::arithmetic_side_effects)]

mod common;

use std::path::Path;

use kitcraft_esm::forms::{names_master, NewForms, MAX_FULL_OBJECT_ID};
use kitcraft_esm::index::{index_file, read_record_at, IndexOptions};
use kitcraft_esm::tes4::{Scale, Tes4, FLAG_MEDIUM, FLAG_SMALL};
use kitcraft_esm::Error;

/// The scale of each master of `header`, read from the master's own `TES4` in `dir`
/// (`None` when the file is missing: never guessed).
fn master_scales(dir: &Path, header: &kitcraft_esm::Record<'_>) -> Vec<Option<Scale>> {
    Tes4::new(header)
        .masters()
        .iter()
        .map(|m| {
            let mut file = std::fs::File::open(dir.join(m.name_lossy())).ok()?;
            let tes4 = read_record_at(&mut file, 0, None).ok()?;
            Tes4::new(&tes4).scale()
        })
        .collect()
}

fn check(names: &[&str]) {
    let Some(files) = common::files("forms", names) else {
        return;
    };
    for file in &files {
        let index = index_file(&file.path, &IndexOptions::default()).unwrap();
        let tes4 = Tes4::new(&index.header);
        let scaled = tes4.flags() & (FLAG_SMALL | FLAG_MEDIUM) != 0;
        let dir = file.path.parent().unwrap();
        let scales = master_scales(dir, &index.header);
        let forms = NewForms::for_index(&index, &scales);
        if scaled {
            assert!(matches!(forms, Err(Error::Refused { .. })), "{}", file.name);
            println!("{}: refused (small or medium)", file.name);
            continue;
        }
        let forms = forms.unwrap();
        let first = forms.peek().unwrap();
        let full = scales.iter().filter(|s| **s == Some(Scale::Full)).count();
        let prefix = u32::try_from(full).unwrap();
        assert_eq!(first >> 24, prefix, "{}", file.name);
        let hedr = tes4.hedr().unwrap().unwrap().next_object_id & MAX_FULL_OBJECT_ID;
        assert!(first & MAX_FULL_OBJECT_ID >= hedr, "{}", file.name);
        let mut overrides = 0u64;
        for r in index.records.iter().skip(1) {
            if r.form_id >> 24 == prefix {
                assert!(r.form_id < first, "{}: own {:08X}", file.name, r.form_id);
            }
            if names_master(r.form_id, &scales) == Some(true) {
                overrides += 1;
            }
        }
        println!(
            "{}: first new FormID {first:08X} (HEDR next {hedr:06X}; {} masters, {full} full; \
             {overrides} records name a master)",
            file.name,
            scales.len()
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
