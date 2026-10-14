//! M3.D6 on the official files (`KITCRAFT_DATA_DIR`; skips when unset).
//!
//! - The reader `inspect`, `where` and `diff` decode with (`kitcraft_doc::typed`, an
//!   interpreter over lane B's schema) agrees with the generated decoder
//!   (`Plugin::typed` / `generated::records::Record::decode`): on every record of a defined
//!   type, the FormIDs and string IDs it reads from the subrecords the generated decoder
//!   placed are the ones the generated record's visitors list, and every such subrecord
//!   decodes.
//! - Every lstring ID (FULL and DESC on every record type, and every schema `lstring`
//!   member: the GMST `s` settings) is in its plugin's string tables, looked up through
//!   `kitcraft_doc::strings` exactly as `inspect` does; ID 0 is counted apart.
//!
//! Fast: the five small files. `#[ignore]` (release): all 14.
//! `cargo test -p kitcraft-doc --release --test typed_corpus -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unreachable
)]

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use kitcraft_doc::lstring::{decode_lstring, is_lstring, LString};
use kitcraft_doc::strings::{DirSource, StringTables};
use kitcraft_doc::typed::{self, Decoded};
use kitcraft_esm::bridge::{Ctx, RawSub};
use kitcraft_esm::driver::visit_file;
use kitcraft_esm::generated::records::{LString as GenLString, Record as Generated};
use kitcraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature, Subrecord};

const SMALL: [&str; 5] = [
    "Constellation.esm",
    "OldMars.esm",
    "SFBGS007.esm",
    "SFBGS004.esm",
    "SFBGS047.esm",
];

const ALL: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS006.esm",
    "SFBGS003.esm",
    "ShatteredSpace.esm",
    "SFBGS004.esm",
    "SFBGS00D.esm",
    "SFBGS050.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS047.esm",
];

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("KITCRAFT_DATA_DIR")?);
    dir.join("Starfield.esm").is_file().then_some(dir)
}

/// Counts for one file.
#[derive(Debug, Default)]
struct Tally {
    /// Records of a defined type cross-checked.
    typed_records: u64,
    /// FormIDs and string IDs compared.
    form_ids: u64,
    string_ids: u64,
    /// Subrecords the generated decoder kept raw although the interpreter decodes them
    /// (placement, not payload: an out-of-order or unexpected subrecord).
    extra_but_decodable: u64,
    /// lstring occurrences looked up, resolved, ID 0.
    lstrings: u64,
    resolved: u64,
    zero: u64,
    /// Unresolved: (record, sub) -> count, with the first examples.
    unresolved: BTreeMap<String, u64>,
    examples: Vec<String>,
    /// Of the unresolved IDs of a `BlueprintShips-<X>.esm`, those found in `<X>.esm`'s
    /// tables (a diagnostic only: lookups never fall back to another plugin's tables).
    in_parent_tables: u64,
}

fn leaves(d: &Decoded, forms: &mut Vec<u32>, ids: &mut Vec<u32>) {
    match d {
        Decoded::Form(f) => forms.push(*f),
        Decoded::LStringId(id) => ids.push(*id),
        Decoded::Struct(fields) => fields.iter().for_each(|(_, v)| leaves(v, forms, ids)),
        Decoded::Array(items) => items.iter().for_each(|v| leaves(v, forms, ids)),
        _ => {}
    }
}

fn check_record(
    plugin: &str,
    localized: bool,
    record: &Record<'_>,
    subs: &[Subrecord<'_>],
    strings: &mut StringTables,
    t: &mut Tally,
) {
    let schema = kitcraft_schema::builtin().unwrap();
    let sig = record.header.signature.0;
    let label = format!("{}:{:08X}", record.header.signature, record.header.form_id);
    let edid_first = subs
        .iter()
        .find(|s| s.signature() == Signature::new(b"EDID"))
        .and_then(|s| s.data().first().copied())
        .filter(|b| *b != 0);
    let ctx = typed::Ctx {
        form_version: record.header.form_version,
        localized,
        edid_first,
    };
    let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
    let generated = Generated::decode(
        sig,
        &raw,
        Ctx {
            form_version: record.header.form_version,
            localized,
        },
    );
    let extra: HashSet<usize> = generated
        .as_ref()
        .map(|g| g.extra().iter().map(|(i, _)| *i).collect())
        .unwrap_or_default();
    let (mut forms, mut ids) = (Vec::new(), Vec::new());
    let mut lstring_ids: Vec<(String, u32)> = Vec::new();
    // Layouts by position, as `inspect`, `where` and `diff` resolve them.
    let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
    let placed = typed::placements(schema, sig, &sigs);
    for (i, s) in subs.iter().enumerate() {
        let sub_sig = s.signature().0;
        let layout = typed::layout_of(
            schema,
            sig,
            sub_sig,
            i,
            placed.as_ref().and_then(|p| p.get(i)),
        );
        match layout {
            Err(e @ typed::LayoutError::Unplaced { .. }) => {
                // The generated decoder leaves what the matcher cannot place in `extra`.
                assert!(
                    extra.contains(&i),
                    "{plugin} {label}: {e}, but generated placed it"
                );
            }
            Err(typed::LayoutError::Ambiguous { .. }) => unreachable!("one subrecord, one layout"),
            Ok(Some(layout)) => {
                let ty = layout.ty;
                let decoded = typed::decode(ty, s.data(), &ctx);
                if extra.contains(&i) {
                    if decoded.is_ok() {
                        t.extra_but_decodable += 1;
                    }
                    continue;
                }
                let d = decoded.unwrap_or_else(|e| {
                    panic!(
                        "{plugin} {label} {}: generated decodes, typed does not: {e}",
                        s.signature()
                    )
                });
                let mut here = Vec::new();
                leaves(&d, &mut forms, &mut here);
                for id in &here {
                    lstring_ids.push((s.signature().to_string(), *id));
                }
                ids.extend(here);
            }
            // No layout, or raw inside a component block: FULL and DESC are lstrings on
            // every record type.
            Ok(None) | Err(typed::LayoutError::Raw { .. })
                if is_lstring(&s.signature().to_string()) =>
            {
                if let LString::Id(id) = decode_lstring(s.data(), localized) {
                    lstring_ids.push((s.signature().to_string(), id));
                }
            }
            Ok(None) | Err(typed::LayoutError::Raw { .. }) => {}
        }
    }
    if let Some(g) = &generated {
        t.typed_records += 1;
        let mut gen_forms: Vec<u32> = g.form_ids().iter().map(|(_, f)| f.0).collect();
        let mut gen_ids: Vec<u32> = g
            .lstrings()
            .iter()
            .filter_map(|(_, l)| match l {
                GenLString::Id(id) => Some(*id),
                GenLString::Text(_) => None,
            })
            .collect();
        forms.sort_unstable();
        gen_forms.sort_unstable();
        ids.sort_unstable();
        gen_ids.sort_unstable();
        assert_eq!(forms, gen_forms, "{plugin} {label}: FormIDs");
        assert_eq!(ids, gen_ids, "{plugin} {label}: string IDs");
        t.form_ids += forms.len() as u64;
        t.string_ids += ids.len() as u64;
    }
    for (sub, id) in lstring_ids {
        t.lstrings += 1;
        if id == 0 {
            t.zero += 1;
            continue;
        }
        match strings.lookup(plugin, id) {
            Ok(_) => t.resolved += 1,
            Err(why) => {
                let parent = plugin
                    .strip_prefix("BlueprintShips-")
                    .filter(|p| !p.eq_ignore_ascii_case("Starfield.esm"));
                if parent.is_some_and(|p| strings.lookup(p, id).is_ok()) {
                    t.in_parent_tables += 1;
                }
                *t.unresolved
                    .entry(format!("{}.{sub}", record.header.signature))
                    .or_default() += 1;
                if t.examples.len() < 10 {
                    t.examples.push(format!("{label} {sub} 0x{id:08X}: {why}"));
                }
            }
        }
    }
}

fn walk(
    plugin: &str,
    localized: bool,
    entry: &Entry<'_>,
    strings: &mut StringTables,
    t: &mut Tally,
) -> kitcraft_esm::Result<()> {
    match entry {
        Entry::Group(g) => {
            for child in g.children() {
                walk(plugin, localized, child, strings, t)?;
            }
        }
        Entry::Record(r) => {
            if r.header.signature == Signature::TES4 {
                return Ok(());
            }
            let decoded = r.decode(&DecodeLimits::default())?;
            let subs: &[Subrecord<'_>] = match (&decoded, r.subrecords()) {
                (Some(d), _) => d,
                (None, Some(s)) => s,
                (None, None) => &[],
            };
            check_record(plugin, localized, r, subs, strings, t);
        }
    }
    Ok(())
}

fn scan(dir: &Path, name: &str) -> Tally {
    let path = dir.join(name);
    let header = kitcraft_doc::index::read_header(&path).unwrap();
    let localized = header.flags.localized();
    let mut strings = StringTables::new(
        "en",
        Box::new(DirSource {
            data_dir: dir.to_path_buf(),
        }),
    );
    let mut t = Tally::default();
    visit_file(&path, &ParseOptions::default(), |entry| {
        walk(name, localized, entry, &mut strings, &mut t)
    })
    .unwrap();
    eprintln!(
        "| {name} | {} | {} | {} | {} | {} | {} | {} | {} |",
        t.typed_records,
        t.form_ids,
        t.string_ids,
        t.extra_but_decodable,
        t.lstrings,
        t.resolved,
        t.zero,
        t.unresolved.values().sum::<u64>()
    );
    for (k, n) in &t.unresolved {
        eprintln!("  unresolved {k}: {n}");
    }
    if t.in_parent_tables > 0 {
        eprintln!(
            "  of those, in the parent plugin's tables: {}",
            t.in_parent_tables
        );
    }
    for e in &t.examples {
        eprintln!("  e.g. {e}");
    }
    t
}

fn header() {
    eprintln!(
        "| File | Typed records | FormIDs compared | String IDs compared | Kept raw by placement | lstrings | Resolved | ID 0 | Unresolved |"
    );
}

#[test]
fn typed_reader_agrees_with_generated_and_lstrings_resolve_small_files() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITCRAFT_DATA_DIR unset");
        return;
    };
    header();
    for name in SMALL {
        let t = scan(&dir, name);
        assert!(t.typed_records > 0, "{name}");
        assert!(t.lstrings > 0, "{name}");
        assert_eq!(
            t.unresolved.values().sum::<u64>(),
            0,
            "{name}: {:?}",
            t.examples
        );
    }
}

#[test]
#[ignore = "release: reads all 14 official files (about 2.5 GB) and their string tables"]
fn typed_reader_agrees_with_generated_and_lstrings_resolve_all_files() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITCRAFT_DATA_DIR unset");
        return;
    };
    header();
    let started = std::time::Instant::now();
    let mut totals = Tally::default();
    for name in ALL {
        let t = scan(&dir, name);
        totals.typed_records += t.typed_records;
        totals.lstrings += t.lstrings;
        totals.resolved += t.resolved;
        totals.zero += t.zero;
        for (k, n) in t.unresolved {
            *totals.unresolved.entry(format!("{name} {k}")).or_default() += n;
        }
    }
    eprintln!(
        "all: {} typed records, {} lstrings, {} resolved, {} ID 0, {} unresolved, {:?}",
        totals.typed_records,
        totals.lstrings,
        totals.resolved,
        totals.zero,
        totals.unresolved.values().sum::<u64>(),
        started.elapsed()
    );
    // Corpus facts (2026-10-14, `docs/corpus.md`): 85 ARMO FULL IDs of Starfield.esm are in
    // none of its English tables, and BlueprintShips-SFBGS050.esm ships no string tables
    // at all (no `BlueprintShips-SFBGS050 - *.ba2`, no loose files). Anything else
    // unresolved is new.
    let expected: BTreeMap<String, u64> = [
        ("Starfield.esm ARMO.FULL", 85),
        ("BlueprintShips-SFBGS050.esm GBFM.FULL", 30),
        ("BlueprintShips-SFBGS050.esm LCTN.FULL", 66),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    assert_eq!(totals.unresolved, expected);
}
