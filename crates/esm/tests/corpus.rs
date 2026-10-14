//! M1.6: roundtrip the official ESMs in place (see docs/corpus.md).
//!
//! - `corpus_small_roundtrip` and `corpus_small_compressed_and_xxxx` run in `cargo test`
//!   on the five small files (SHA-256 checked).
//! - `corpus_full_roundtrip` (ignored) runs all 14 files (SHA-256 checked); run it in
//!   release: `cargo test -p kitcraft-esm --release -- --ignored corpus_full`
//! - `corpus_rewrite_edit_in_largest_group` (ignored) measures the streaming edit path.
//! - `corpus_tes4_table` (ignored) prints the TES4 header facts recorded in docs/corpus.md.

// Test-only helpers: a panic fails the test.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use std::time::Instant;

use kitcraft_esm::driver::{
    rewrite_file, roundtrip_file, RewriteOptions, RoundtripOptions, RoundtripReport,
};
use kitcraft_esm::tes4::Tes4;
use kitcraft_esm::{DecodeLimits, Entry, Plugin, Record};

fn run(files: &[common::CorpusFile]) -> Vec<String> {
    let mut failures = Vec::new();
    println!(
        "| File | Identical | Time (s) | Records | Compressed | Subrecords | XXXX | Groups | Largest top-level entry | Largest decompressed |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for file in files {
        let start = Instant::now();
        let result = roundtrip_file(&file.path, &RoundtripOptions::with_decode());
        let secs = start.elapsed().as_secs_f64();
        match result {
            Ok(report) => {
                print_row(&file.name, &report, secs);
                if let Some(m) = &report.mismatch {
                    failures.push(format!(
                        "{}: first mismatch at offset {} (expected {:?}, wrote {:?}) in {}",
                        file.name, m.offset, m.expected, m.actual, m.path
                    ));
                } else if !report.identical() {
                    failures.push(format!(
                        "{}: wrote {} of {} bytes",
                        file.name, report.output_len, report.input_len
                    ));
                }
            }
            Err(e) => {
                println!("| {} | error | {secs:.2} | | | | | | | |", file.name);
                failures.push(format!("{}: {e} (offset {})", file.name, e.offset()));
            }
        }
    }
    failures
}

fn print_row(name: &str, report: &RoundtripReport, secs: f64) {
    let s = &report.stats;
    println!(
        "| {name} | {} | {secs:.2} | {} | {} | {} | {} | {} | {} | {} |",
        if report.identical() { "yes" } else { "no" },
        s.records,
        s.compressed_records,
        s.subrecords,
        s.oversize_subrecords,
        s.groups,
        s.largest_top_level_entry,
        s.largest_decompressed,
    );
}

#[test]
fn corpus_small_roundtrip() {
    let Some(files) = common::files("corpus_small_roundtrip", &common::SMALL) else {
        return;
    };
    common::verify_hashes(&files);
    let failures = run(&files);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Count compressed records and `XXXX` subrecords (decoded ones included) while decoding
/// every compressed record in place and reaching every record mutably without a change.
fn decode_and_touch(entry: &mut Entry<'_>, compressed: &mut u64, xxxx: &mut u64) {
    match entry {
        Entry::Group(g) => g
            .children_mut()
            .iter_mut()
            .for_each(|c| decode_and_touch(c, compressed, xxxx)),
        Entry::Record(r) => {
            if r.header.is_compressed() {
                *compressed += 1;
                r.decompress(&DecodeLimits::default()).unwrap();
            }
            let subs = r.subrecords_mut().unwrap();
            *xxxx += subs.iter().filter(|s| s.has_oversize_prefix()).count() as u64;
        }
    }
}

/// Real compressed records and `XXXX` in `cargo test`: decoded and touched they still
/// write back byte-identically; an edit inside one survives a reparse.
#[test]
fn corpus_small_compressed_and_xxxx() {
    let Some(files) = common::files("corpus_small_compressed_and_xxxx", &common::SMALL) else {
        return;
    };
    let mut seen = Vec::new();
    for file in files {
        let bytes = std::fs::read(&file.path).unwrap();
        let mut plugin = Plugin::parse(&bytes).unwrap();
        let (mut compressed, mut xxxx) = (0, 0);
        plugin
            .entries_mut()
            .iter_mut()
            .for_each(|e| decode_and_touch(e, &mut compressed, &mut xxxx));
        assert_eq!(plugin.to_bytes().unwrap(), bytes, "{}", file.name);
        seen.push((file.name.clone(), compressed, xxxx));
    }
    assert!(
        seen.contains(&("SFBGS047.esm".to_string(), 2, 1)),
        "{seen:?}"
    );
    assert!(
        seen.iter()
            .any(|(name, c, _)| name == "SFBGS004.esm" && *c == 2),
        "{seen:?}"
    );

    // Edit the first subrecord of the first compressed record of SFBGS047.
    let path = common::files("corpus_small_compressed_and_xxxx", &["SFBGS047.esm"])
        .unwrap()
        .remove(0)
        .path;
    let bytes = std::fs::read(path).unwrap();
    let mut plugin = Plugin::parse(&bytes).unwrap();
    let target = edit_first_compressed(plugin.entries_mut()).unwrap();
    let out = plugin.to_bytes().unwrap();
    assert_ne!(out, bytes);
    let reparsed = Plugin::parse(&out).unwrap();
    let found = find_record(reparsed.entries(), target).unwrap();
    let decoded = found.decode(&DecodeLimits::default()).unwrap().unwrap();
    assert_eq!(decoded[0].data(), b"kitcraft edit\0");
}

fn edit_first_compressed(entries: &mut [Entry<'_>]) -> Option<u32> {
    for entry in entries {
        match entry {
            Entry::Group(g) => {
                if let Some(id) = edit_first_compressed(g.children_mut()) {
                    return Some(id);
                }
            }
            Entry::Record(r) if r.header.is_compressed() => {
                r.decompress(&DecodeLimits::default()).unwrap();
                r.subrecords_mut().unwrap()[0].set_data(&b"kitcraft edit\0"[..]);
                return Some(r.header.form_id);
            }
            Entry::Record(_) => {}
        }
    }
    None
}

fn find_record<'p, 'a>(entries: &'p [Entry<'a>], form_id: u32) -> Option<&'p Record<'a>> {
    entries.iter().find_map(|e| match e {
        Entry::Group(g) => find_record(g.children(), form_id),
        Entry::Record(r) => (r.header.form_id == form_id).then_some(r),
    })
}

#[test]
#[ignore = "reads 2.5 GB of game data; run with --release -- --ignored"]
fn corpus_full_roundtrip() {
    let Some(files) = common::all_files("corpus_full_roundtrip") else {
        return;
    };
    let start = Instant::now();
    common::verify_hashes(&files);
    println!(
        "SHA-256 of {} files verified in {:.1} s",
        files.len(),
        start.elapsed().as_secs_f64()
    );
    let start = Instant::now();
    let failures = run(&files);
    println!("total wall time: {:.1} s", start.elapsed().as_secs_f64());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Peak working set of this process (Windows, via PowerShell), in bytes.
fn peak_working_set() -> Option<u64> {
    let script = format!("(Get-Process -Id {}).PeakWorkingSet64", std::process::id());
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Descend to the first record in the deepest-first chain of last children and grow its
/// first subrecord by one byte.
fn edit_deep(entry: &mut Entry<'_>, depth: usize) -> Option<usize> {
    match entry {
        Entry::Record(r) => {
            let subs = r.subrecords_mut()?;
            let first = subs.first_mut()?;
            let mut data = first.data().to_vec();
            data.push(0);
            first.set_data(data);
            Some(depth)
        }
        Entry::Group(g) => {
            let children = g.children_mut();
            let groups = children
                .iter()
                .filter(|c| matches!(c, Entry::Group(_)))
                .count();
            if groups > 0 {
                let last = children
                    .iter_mut()
                    .rev()
                    .find(|c| matches!(c, Entry::Group(_)))?;
                if let Some(d) = edit_deep(last, depth + 1) {
                    return Some(d);
                }
            }
            children.iter_mut().find_map(|c| match c {
                Entry::Record(_) => edit_deep(c, depth + 1),
                Entry::Group(_) => None,
            })
        }
    }
}

/// M1.5 follow-up: edit one subrecord deep inside `Starfield.esm`'s largest top-level
/// group (`WRLD`, 823.6 MiB) through the streaming edit path, writing to a byte counter.
/// Prints wall time and the process's peak working set for docs/corpus.md.
/// `cargo test -p kitcraft-esm --release --test corpus -- --ignored corpus_rewrite --nocapture`
#[test]
#[ignore = "streams Starfield.esm (1.46 GB); run with --release -- --ignored"]
fn corpus_rewrite_edit_in_largest_group() {
    let Some(files) = common::files("corpus_rewrite_edit_in_largest_group", &["Starfield.esm"])
    else {
        return;
    };
    let file = &files[0];
    struct Count(u64);
    impl std::io::Write for Count {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len() as u64;
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Count(0);
    let mut edited_depth = None;
    let start = Instant::now();
    let report = rewrite_file(&file.path, &RewriteOptions::default(), &mut sink, |entry| {
        if let Entry::Group(g) = entry {
            if g.header.label == *b"WRLD" && g.header.group_type == 0 {
                edited_depth = edit_deep(entry, 1);
            }
        }
        Ok(())
    })
    .unwrap();
    let secs = start.elapsed().as_secs_f64();
    let peak = peak_working_set();
    println!(
        "edited one subrecord at depth {edited_depth:?} of WRLD: wrote {} bytes (input {}), \
         {} modified top-level entries, {secs:.1} s, peak working set {}",
        report.output_len,
        report.input_len,
        report.modified_entries,
        peak.map_or("unknown".to_string(), |p| format!(
            "{p} bytes ({:.0} MiB)",
            p as f64 / 1048576.0
        )),
    );
    assert!(edited_depth.is_some());
    assert_eq!(report.output_len, report.input_len + 1);
    assert_eq!(sink.0, report.output_len);
    assert_eq!(report.modified_entries, 1);
    assert_eq!(report.hedr_fix, None);
}

#[test]
#[ignore = "prints the TES4 table for docs/corpus.md"]
fn corpus_tes4_table() {
    let Some(files) = common::all_files("corpus_tes4_table") else {
        return;
    };
    println!("| File | Flags | Form version | HEDR version | HEDR count | HEDR next ID | Masters | Other TES4 subrecords |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for file in files {
        // The TES4 record is the first entry; read just enough of the file for it.
        let mut head = vec![0u8; 24];
        let mut f = std::fs::File::open(&file.path).unwrap();
        std::io::Read::read_exact(&mut f, &mut head).unwrap();
        let size = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
        head.resize(24 + size, 0);
        std::io::Read::read_exact(&mut f, &mut head[24..]).unwrap();
        let Entry::Record(record) = Entry::parse_one(&head, 0, 1, &Default::default()).unwrap()
        else {
            panic!("{}: first entry is not a record", file.name)
        };
        // Sanity: the same record parses as a plugin header.
        let plugin = Plugin::parse(&head).unwrap();
        assert_eq!(plugin.header(), &record);
        let tes4 = Tes4::new(&record);
        let hedr = tes4.hedr().unwrap().unwrap();
        let masters: Vec<String> = tes4.masters().iter().map(|m| m.name_lossy()).collect();
        let mut others: Vec<String> = Vec::new();
        for s in record.subrecords().unwrap() {
            let sig = s.signature().to_string();
            if !matches!(sig.as_str(), "HEDR" | "MAST" | "DATA") {
                let label = format!("{sig} ({} B)", s.data().len());
                if !others.contains(&label) {
                    others.push(label);
                }
            }
        }
        println!(
            "| {} | 0x{:08X} | {} | {} | {} | 0x{:X} | {}: {} | {} |",
            file.name,
            tes4.flags(),
            tes4.form_version(),
            hedr.version,
            hedr.record_count,
            hedr.next_object_id,
            masters.len(),
            masters.join(", "),
            others.join(", "),
        );
    }
}
