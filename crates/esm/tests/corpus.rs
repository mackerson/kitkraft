//! M1.6: roundtrip the official ESMs in place (see docs/corpus.md).
//!
//! - `corpus_small_roundtrip` runs in `cargo test` on the three small files.
//! - `corpus_full_roundtrip` (ignored) runs all 14 files; run it in release:
//!   `cargo test -p kitcraft-esm --release -- --ignored corpus_full`
//! - `corpus_tes4_table` (ignored) prints the TES4 header facts recorded in docs/corpus.md.

mod common;

use std::time::Instant;

use kitcraft_esm::driver::{roundtrip_file, RoundtripOptions, RoundtripReport};
use kitcraft_esm::tes4::Tes4;
use kitcraft_esm::{Entry, Plugin};

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
    let failures = run(&files);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
#[ignore = "reads 2.5 GB of game data; run with --release -- --ignored"]
fn corpus_full_roundtrip() {
    let Some(files) = common::all_files("corpus_full_roundtrip") else {
        return;
    };
    let start = Instant::now();
    let failures = run(&files);
    println!("total wall time: {:.1} s", start.elapsed().as_secs_f64());
    assert!(failures.is_empty(), "{failures:#?}");
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
