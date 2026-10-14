//! Corpus tests: the official archives under `KITCRAFT_DATA_DIR`, read in place (ADR 0006).
//! Without the variable they print one skip line and pass.
//!
//! - `corpus_small` (fast): the localization and small texture archives list, every hash
//!   agrees with its name, and every entry extracts to its declared size.
//! - `corpus_full` (`#[ignore]`, run in release): every archive lists; 100 seeded entries
//!   per archive extract to their declared sizes; every texture's chunks add up to its mip
//!   chain; prints the inventory recorded in `docs/corpus.md` ("Archives").
//!
//! ```powershell
//! cargo test -p kitcraft-ba2 --release --test corpus -- --ignored --nocapture
//! ```

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use kitcraft_ba2::{dds, Archive, Body, Kind};

fn data_dir() -> Option<PathBuf> {
    let Some(dir) = std::env::var_os("KITCRAFT_DATA_DIR") else {
        println!("skipped: KITCRAFT_DATA_DIR is not set");
        return None;
    };
    let dir = PathBuf::from(dir);
    if !dir.is_dir() {
        println!("skipped: KITCRAFT_DATA_DIR {} is missing", dir.display());
        return None;
    }
    Some(dir)
}

fn archives(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ba2")))
        .collect();
    out.sort();
    out
}

/// Counts bytes written.
struct Sink(u64);
impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// xorshift64 seeded from the file name, so the sample is stable per archive.
fn sample(name: &str, count: usize, take: usize) -> Vec<usize> {
    let mut x = name.bytes().fold(0x9E37_79B9_7F4A_7C15u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3)
    });
    let mut picked: Vec<usize> = (0..take.min(count))
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % count as u64) as usize
        })
        .collect();
    picked.sort_unstable();
    picked.dedup();
    picked
}

/// Checks every texture's chunk sizes add up to its mip chain (every face for a cube map)
/// and that its chunks cover the mips in order.
fn check_textures(archive: &Archive, name: &str) {
    for entry in &archive.entries {
        let Body::Texture(t) = &entry.body else {
            continue;
        };
        let total: u64 = t.chunks.iter().map(|c| u64::from(c.size)).sum();
        let want = dds::chain_len(
            t.format,
            u32::from(t.width),
            u32::from(t.height),
            u32::from(t.mip_count),
            dds::faces(t),
        );
        let Some(want) = want else {
            panic!("{name}: {}: DXGI format {} not sized", entry.name, t.format);
        };
        assert_eq!(
            total, want,
            "{name}: {}: chunks vs the mip chain of {}x{}, format {}, {} mips",
            entry.name, t.width, t.height, t.format, t.mip_count
        );
        // Chunks cover the mips in order without gaps.
        let mut next = 0u16;
        for c in &t.chunks {
            assert_eq!(c.mip_first, next, "{name}: {}", entry.name);
            assert!(c.mip_last >= c.mip_first, "{name}: {}", entry.name);
            next = c.mip_last + 1;
        }
    }
}

#[test]
fn corpus_small() {
    let Some(dir) = data_dir() else { return };
    let names = [
        "Starfield - Localization.ba2",
        "OldMars - Localization.ba2",
        "OldMars - Textures.ba2",
        "SFBGS047 - Textures.ba2",
        "SFBGS006 - Main.ba2",
    ];
    for name in names {
        let path = dir.join(name);
        if !path.is_file() {
            println!("skipped: {name} is missing");
            continue;
        }
        let (archive, mut file) = Archive::open(&path).unwrap();
        assert!(archive.has_names, "{name}");
        assert!(!archive.entries.is_empty(), "{name}");
        for (i, entry) in archive.entries.iter().enumerate() {
            assert!(entry.hashes_match_name(), "{name}: {}", entry.name);
            let mut sink = Sink(0);
            let n = archive.extract_to(&mut file, i, &mut sink).unwrap();
            assert_eq!(n, entry.extracted_len(), "{name}: {}", entry.name);
            assert_eq!(sink.0, n);
        }
        check_textures(&archive, name);
    }
}

/// Differential check against `Archive2.exe` run as a black box (ADR 0006): set
/// `KITCRAFT_A2_ARCHIVE` to an archive and `KITCRAFT_A2_DIR` to the folder
/// `Archive2.exe "<archive>" -extract="<dir>"` wrote; every entry must be byte-identical.
#[test]
#[ignore = "needs an Archive2.exe extraction; see the doc comment"]
fn archive2_differential() {
    let (Some(archive), Some(out)) = (
        std::env::var_os("KITCRAFT_A2_ARCHIVE"),
        std::env::var_os("KITCRAFT_A2_DIR"),
    ) else {
        println!("skipped: KITCRAFT_A2_ARCHIVE / KITCRAFT_A2_DIR not set");
        return;
    };
    let (archive, mut file) = Archive::open(Path::new(&archive)).unwrap();
    let out = PathBuf::from(out);
    let (mut same, mut header_only, mut data) = (0usize, Vec::new(), Vec::new());
    for (i, entry) in archive.entries.iter().enumerate() {
        let ours = archive.extract_vec(&mut file, i).unwrap();
        let path = out.join(kitcraft_ba2::relative_path(&entry.name).unwrap());
        let theirs = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if ours == theirs {
            same += 1;
            continue;
        }
        let skip = match &entry.body {
            Body::Texture(t) => dds::header_len(t) as usize,
            Body::General(_) => 0,
        };
        let body_same = ours.len() == theirs.len() && ours.get(skip..) == theirs.get(skip..);
        let first = ours.iter().zip(&theirs).position(|(a, b)| a != b);
        let note = format!("{} (first difference at byte {first:?})", entry.name);
        if body_same {
            header_only.push(note);
        } else {
            data.push(note);
        }
    }
    println!(
        "{same} identical, {} differ in the DDS header only, {} differ in data",
        header_only.len(),
        data.len()
    );
    for n in header_only.iter().chain(&data).take(20) {
        println!("  {n}");
    }
    assert!(header_only.is_empty() && data.is_empty());
}

#[test]
#[ignore = "reads every official archive (about 150 GB on disk); run in release"]
fn corpus_full() {
    let Some(dir) = data_dir() else { return };
    let mut kinds: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    let mut extras: BTreeMap<String, usize> = BTreeMap::new();
    let mut bad_hashes = Vec::new();
    let (mut total_entries, mut extracted, mut extracted_bytes) = (0u64, 0u64, 0u64);
    let mut packed_entries = 0u64;
    for path in archives(&dir) {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let (archive, mut file) = Archive::open(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        let h = &archive.header;
        let key = format!(
            "v{} {} codec {}",
            h.version,
            h.kind.name(),
            h.codec().map(|c| c.name()).unwrap_or("?")
        );
        let k = kinds.entry(key).or_default();
        k.0 += 1;
        k.1 += archive.entries.len() as u64;
        *extras
            .entry(format!(
                "extra {:?} compression {:?}",
                h.extra, h.compression
            ))
            .or_default() += 1;
        total_entries += archive.entries.len() as u64;
        for entry in &archive.entries {
            if !entry.hashes_match_name() {
                bad_hashes.push(format!("{name}: {}", entry.name));
            }
            if entry.is_compressed() {
                packed_entries += 1;
            }
            if let Body::General(g) = &entry.body {
                assert_eq!(
                    g.flags,
                    kitcraft_ba2::GENERAL_FLAGS,
                    "{name}: {}",
                    entry.name
                );
                assert_eq!(g.filler, kitcraft_ba2::FILLER, "{name}: {}", entry.name);
            }
            if let Body::Texture(t) = &entry.body {
                assert_eq!(
                    t.tile_mode,
                    kitcraft_ba2::TILE_MODE,
                    "{name}: {}",
                    entry.name
                );
            }
        }
        if h.kind == Kind::Textures {
            check_textures(&archive, &name);
        }
        for i in sample(&name, archive.entries.len(), 100) {
            let entry = &archive.entries[i];
            let mut sink = Sink(0);
            let n = archive
                .extract_to(&mut file, i, &mut sink)
                .unwrap_or_else(|e| panic!("{name}: {}: {e}", entry.name));
            assert_eq!(n, entry.extracted_len(), "{name}: {}", entry.name);
            extracted += 1;
            extracted_bytes += n;
        }
        println!(
            "{name}: v{} {} {} entries",
            h.version,
            h.kind.name(),
            archive.entries.len()
        );
    }
    println!("archives by version/kind/codec: {kinds:?}");
    println!("header extra fields: {extras:?}");
    println!(
        "entries {total_entries}, with packed data {packed_entries}; extracted {extracted} \
         ({extracted_bytes} bytes)"
    );
    println!("hash mismatches: {}", bad_hashes.len());
    assert!(bad_hashes.is_empty(), "{bad_hashes:#?}");
}
