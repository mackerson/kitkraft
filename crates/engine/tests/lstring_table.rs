//! The lstring signature table (`kitkraft_engine::edit::lstrings`), rebuilt from the
//! corpus and compared with the table in the source (Fable review round 2, finding 2b).
//!
//! The pinned xEdit file is not available locally (`plan/STATUS.md`, Blockers), so the
//! table is built from the official ESMs (all localized) by the method in
//! `kitkraft_engine::edit::lstrings` and `docs/validate.md` ("lstring signature table").
//!
//! `#[ignore]`: reads 2.5 GB. Run
//! `cargo test -p kitkraft-engine --release --test lstring_table -- --ignored --nocapture`
//! with `KITKRAFT_DATA_DIR` set; on a mismatch it prints the table to paste.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use kitkraft_ba2::Archive;
use kitkraft_engine::edit::lstrings::{classify, is_anchor, Class, Counts, TABLE};
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::index::{index_file, IndexOptions};
use kitkraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature};
use kitkraft_strings::{table_name, Kind, Table};

const CORPUS: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS003.esm",
    "SFBGS004.esm",
    "SFBGS006.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS00D.esm",
    "SFBGS047.esm",
    "SFBGS050.esm",
    "ShatteredSpace.esm",
];

/// Every string ID in the plugin's English tables with the kind of its table (0
/// `.STRINGS`, 1 `.DLSTRINGS`, 2 `.ILSTRINGS`), from the plugin's archives
/// (`<stem> - *.ba2`, Localization first). `None` when no table was found. An ID in two
/// kinds keeps the first.
fn string_ids(dir: &Path, plugin: &str) -> Option<HashMap<u32, usize>> {
    let stem = Path::new(plugin).file_stem()?.to_str()?;
    let mut ids = HashMap::new();
    let mut found = 0;
    for (k, kind) in [Kind::Strings, Kind::DlStrings, Kind::IlStrings]
        .into_iter()
        .enumerate()
    {
        let want = table_name(plugin, "en", kind);
        for path in kitkraft_doc::strings::plugin_archives(dir, stem) {
            let (archive, mut file) = Archive::open(&path).unwrap();
            let Some(i) = archive.entries.iter().position(|e| {
                e.name
                    .rsplit(['/', '\\'])
                    .next()
                    .is_some_and(|n| n.eq_ignore_ascii_case(&want))
            }) else {
                continue;
            };
            let bytes = archive.extract_vec(&mut file, i, 256 << 20).unwrap();
            let table = Table::parse(kind, &bytes).unwrap();
            for e in &table.entries {
                ids.entry(e.id).or_insert(k);
            }
            found += 1;
            break;
        }
    }
    (found > 0).then_some(ids)
}

/// One file's subrecords per (record type, signature), before they are counted.
#[derive(Default)]
struct Seen {
    occurrences: u64,
    four_bytes: u64,
    nonzero: u64,
    values: HashSet<u32>,
}

type Key = ([u8; 4], [u8; 4]);

fn visit(record: &Record<'_>, seen: &mut HashMap<Key, Seen>) {
    if record.header.signature == Signature::TES4 {
        return;
    }
    let decoded = record.decode(&DecodeLimits::default()).unwrap();
    let subs = match (&decoded, record.subrecords()) {
        (Some(d), _) => d.as_slice(),
        (None, Some(s)) => s,
        (None, None) => return,
    };
    for s in subs {
        let c = seen
            .entry((record.header.signature.0, s.signature().0))
            .or_default();
        c.occurrences += 1;
        let Ok(bytes) = <[u8; 4]>::try_from(s.data()) else {
            continue;
        };
        c.four_bytes += 1;
        let v = u32::from_le_bytes(bytes);
        if v != 0 {
            c.nonzero += 1;
            c.values.insert(v);
        }
    }
}

fn walk(entry: &Entry<'_>, seen: &mut HashMap<Key, Seen>) {
    match entry {
        Entry::Group(g) => {
            for child in g.children() {
                walk(child, seen);
            }
        }
        Entry::Record(r) => visit(r, seen),
    }
}

/// One scanned file: its subrecords, its string IDs, and per key how many distinct values
/// are FormIDs stored in the file.
struct Scanned {
    seen: HashMap<Key, Seen>,
    strings: HashMap<u32, usize>,
    form_ids: HashMap<Key, u64>,
}

/// Totals over every file: distinct non-zero values, how many resolve and in which kind
/// of table, how many are FormIDs of their file, and how many are also a value of another
/// key that `anchor` names an lstring (in the same file).
fn count(files: &[Scanned], anchor: &dyn Fn(Key) -> bool) -> BTreeMap<Key, Counts> {
    let mut stats: BTreeMap<Key, Counts> = BTreeMap::new();
    for file in files {
        // Value -> (an anchor key holding it, whether a second anchor key holds it too),
        // independent of iteration order.
        let mut anchors: HashMap<u32, (Key, bool)> = HashMap::new();
        for (key, s) in &file.seen {
            if anchor(*key) {
                for v in &s.values {
                    anchors
                        .entry(*v)
                        .and_modify(|(k, shared)| *shared |= k != key)
                        .or_insert((*key, false));
                }
            }
        }
        for (key, s) in &file.seen {
            let c = stats.entry(*key).or_default();
            c.occurrences += s.occurrences;
            c.four_bytes += s.four_bytes;
            c.nonzero += s.nonzero;
            c.form_ids += file.form_ids.get(key).copied().unwrap_or(0);
            for v in &s.values {
                c.distinct += 1;
                if let Some(k) = file.strings.get(v) {
                    c.resolved += 1;
                    c.kinds[*k] += 1;
                }
                if anchors
                    .get(v)
                    .is_some_and(|(a, shared)| *shared || a != key)
                {
                    c.anchored += 1;
                }
            }
        }
    }
    stats
}

fn text(sig: [u8; 4]) -> String {
    String::from_utf8_lossy(&sig).into_owned()
}

#[test]
#[ignore = "reads 2.5 GB of plugins and their string tables; run in release"]
fn lstring_table_matches_the_corpus() {
    let Some(dir) = std::env::var_os("KITKRAFT_DATA_DIR")
        .or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
    else {
        println!("skipped: KITKRAFT_DATA_DIR is unset or not a directory");
        return;
    };
    let mut files: Vec<Scanned> = Vec::new();
    for name in CORPUS {
        let path = dir.join(name);
        let Some(strings) = string_ids(&dir, name) else {
            println!("{name}: no English string tables; not scanned");
            continue;
        };
        let index = index_file(&path, &IndexOptions::default()).unwrap();
        let forms: HashSet<u32> = index.records.iter().skip(1).map(|r| r.form_id).collect();
        let started = std::time::Instant::now();
        let mut seen: HashMap<Key, Seen> = HashMap::new();
        visit_file(&path, &ParseOptions::default(), |entry| {
            walk(entry, &mut seen);
            Ok(())
        })
        .unwrap();
        let form_ids = seen
            .iter()
            .map(|(k, s)| {
                (
                    *k,
                    s.values.iter().filter(|v| forms.contains(v)).count() as u64,
                )
            })
            .collect();
        println!(
            "{name}: {} string IDs, {} forms, {} ms",
            strings.len(),
            forms.len(),
            started.elapsed().as_millis()
        );
        files.push(Scanned {
            seen,
            strings,
            form_ids,
        });
    }
    // Round 1: FULL and DESC are the anchor lstrings. Round 2: every lstring round 1
    // found is an anchor too, so a FormID field whose values coincide with string IDs of
    // another lstring (dense ID ranges) is told apart from one that owns its IDs.
    let first = count(&files, &|(_, sub)| is_anchor(sub));
    let anchors: HashSet<Key> = first
        .iter()
        .filter(|((_, sub), c)| classify(*sub, c) == Class::LString)
        .map(|(k, _)| *k)
        .collect();
    let stats = count(&files, &|key| is_anchor(key.1) || anchors.contains(&key));

    // The scan as a table, and the per-signature evidence.
    let mut rebuilt: BTreeMap<[u8; 4], [Vec<String>; 3]> = BTreeMap::new();
    for ((record, sub), c) in &stats {
        let class = classify(*sub, c);
        let column = match class {
            Class::LString => 0,
            Class::NotLString => 1,
            Class::Unclassified => 2,
        };
        rebuilt.entry(*record).or_default()[column].push(text(*sub));
        if c.four_bytes == c.occurrences && c.distinct > 0 {
            println!(
                "  {}.{}: {:?} occurrences {}, distinct {}, resolve {} ({:.1}%) by kind {:?}, FormIDs {}, anchored {}",
                text(*record),
                text(*sub),
                class,
                c.occurrences,
                c.distinct,
                c.resolved,
                100.0 * c.resolved as f64 / c.distinct as f64,
                c.kinds,
                c.form_ids,
                c.anchored
            );
        }
    }
    let generated: Vec<String> = rebuilt
        .iter()
        .map(|(record, [l, n, u])| {
            format!(
                "    (\"{}\", \"{}\", \"{}\", \"{}\"),",
                text(*record),
                l.join(" "),
                n.join(" "),
                u.join(" ")
            )
        })
        .collect();
    let current: Vec<String> = TABLE
        .iter()
        .map(|(r, l, n, u)| format!("    (\"{r}\", \"{l}\", \"{n}\", \"{u}\"),"))
        .collect();
    if generated != current {
        println!("--- generated table ---");
        for line in &generated {
            println!("{line}");
        }
        println!("--- end ---");
        panic!("the lstring table differs from the corpus scan (printed above)");
    }
    let lstrings: usize = rebuilt.values().map(|[l, _, _]| l.len()).sum();
    println!(
        "{} record types, {lstrings} lstring signatures: the table matches the corpus",
        rebuilt.len()
    );
    // M3.E4: the schema against the same scan. The table now equals the scan, so
    // `compare` (table vs schema) is the corpus vs the schema: no conflict, and the table
    // is the fallback only inside blocks the schema keeps raw.
    let schema = kitkraft_schema::builtin().unwrap();
    let c = kitkraft_engine::edit::lstrings::compare(schema);
    println!(
        "schema: {} lstrings agree, {} from the table as fallback ({}), {} conflicts",
        c.agree.len(),
        c.fallback.len(),
        c.fallback.join(", "),
        c.conflicts.len()
    );
    assert_eq!(c.conflicts, Vec::<String>::new());
    assert_eq!(c.agree.len() + c.fallback.len(), lstrings);
}
