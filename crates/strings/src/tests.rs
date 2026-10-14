//! Synthetic fixtures per variant, exact layouts, hostile input and merge.

use super::{
    merge, table_name, Duplicate, Entry, Error, Kind, Padding, PaddingKind, ParseMode, Table,
};

fn sample(kind: Kind) -> Table {
    Table::new(
        kind,
        vec![
            Entry::new(1, "Old Earth Hunting Rifle"),
            Entry::new(2, "Rifle"),
            Entry::new(5, "Old Earth Hunting Rifle"), // shared with 1
            Entry::new(7, ""),
            Entry::new(9, "Caf\u{e9} \u{65e5}\u{672c}"),
        ],
    )
}

#[test]
fn strings_layout_is_exact() {
    let bytes = sample(Kind::Strings).to_bytes().unwrap();
    let mut want = Vec::new();
    want.extend(5u32.to_le_bytes());
    let data: Vec<u8> = [
        &b"Old Earth Hunting Rifle\0"[..],
        b"Rifle\0",
        b"\0",
        "Caf\u{e9} \u{65e5}\u{672c}\0".as_bytes(),
    ]
    .concat();
    want.extend((data.len() as u32).to_le_bytes());
    for (id, off) in [(1u32, 0u32), (2, 24), (5, 0), (7, 30), (9, 31)] {
        want.extend(id.to_le_bytes());
        want.extend(off.to_le_bytes());
    }
    want.extend(&data);
    assert_eq!(bytes, want);
}

#[test]
fn length_prefixed_layout_is_exact() {
    let table = Table::new(
        Kind::DlStrings,
        vec![
            Entry::new(3, "ab"),
            Entry::new(4, "ab"),
            Entry::new(8, "c\0d"),
        ],
    );
    let bytes = table.to_bytes().unwrap();
    let mut want = Vec::new();
    want.extend(3u32.to_le_bytes());
    want.extend(15u32.to_le_bytes());
    for (id, off) in [(3u32, 0u32), (4, 0), (8, 7)] {
        want.extend(id.to_le_bytes());
        want.extend(off.to_le_bytes());
    }
    want.extend(3u32.to_le_bytes());
    want.extend(b"ab\0");
    want.extend(4u32.to_le_bytes());
    want.extend(b"c\0d\0");
    assert_eq!(bytes, want);
    let back = Table::parse(Kind::DlStrings, &bytes).unwrap();
    assert_eq!(back.get(8).unwrap().bytes, b"c\0d");
    assert_eq!(back.get(4).unwrap().offset, Some(8 + 24));
}

#[test]
fn every_variant_roundtrips() {
    for kind in Kind::ALL {
        let table = sample(kind);
        let bytes = table.to_bytes().unwrap();
        let back = Table::parse(kind, &bytes).unwrap();
        assert_eq!(back.len(), 5);
        for (a, b) in table.entries.iter().zip(&back.entries) {
            assert_eq!((a.id, &a.bytes), (b.id, &b.bytes));
        }
        assert_eq!(back.to_bytes().unwrap(), bytes, "{kind:?}");
        assert_eq!(back.get(9).unwrap().text(), "Caf\u{e9} \u{65e5}\u{672c}");
        assert!(back.get(9).unwrap().is_utf8());
        assert!(back.get(4).is_none());
        assert!(back.duplicate_ids().is_empty());
    }
    let empty = Table::new(Kind::Strings, Vec::new());
    assert_eq!(empty.to_bytes().unwrap(), [0u8; 8]);
    assert!(Table::parse(Kind::Strings, &[0u8; 8]).unwrap().is_empty());
}

#[test]
fn hostile_input_gives_typed_errors() {
    for kind in Kind::ALL {
        let good = sample(kind).to_bytes().unwrap();
        for len in 0..good.len() {
            assert!(
                Table::parse(kind, &good[..len]).is_err(),
                "{kind:?} len {len}"
            );
        }
        for i in 0..good.len() {
            for mask in [0x01u8, 0x80, 0xFF] {
                let mut bad = good.clone();
                bad[i] ^= mask;
                let _ = Table::parse(kind, &bad);
            }
        }
    }
    assert!(matches!(
        Table::parse(Kind::Strings, &[1, 0, 0, 0, 0, 0, 0, 0]),
        Err(Error::Truncated { offset: 8, .. })
    ));
    let mut bytes = sample(Kind::Strings).to_bytes().unwrap();
    bytes[4] ^= 1;
    assert!(matches!(
        Table::parse(Kind::Strings, &bytes),
        Err(Error::DataSize { .. })
    ));
    // Offset past the data.
    let mut bytes = sample(Kind::Strings).to_bytes().unwrap();
    bytes[12..16].copy_from_slice(&1000u32.to_le_bytes());
    assert!(matches!(
        Table::parse(Kind::Strings, &bytes),
        Err(Error::BadOffset {
            id: 1,
            entry: 8,
            ..
        })
    ));
    // A length prefix that runs past the end.
    let mut bytes = sample(Kind::IlStrings).to_bytes().unwrap();
    let data = 8 + 5 * 8;
    bytes[data..data + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Table::parse(Kind::IlStrings, &bytes),
        Err(Error::BadString { id: 1, .. })
    ));
    // No terminating NUL in a .STRINGS table.
    let mut bytes = Table::new(Kind::Strings, vec![Entry::new(1, "x")])
        .to_bytes()
        .unwrap();
    let last = bytes.len() - 1;
    bytes[last] = b'y';
    assert!(matches!(
        Table::parse(Kind::Strings, &bytes),
        Err(Error::BadString { .. })
    ));
    // A huge count is refused before allocating.
    let mut huge = vec![0xFF, 0xFF, 0xFF, 0x7F, 0, 0, 0, 0];
    huge.extend([0u8; 16]);
    assert!(Table::parse(Kind::Strings, &huge).is_err());
}

/// A table followed by padding the header does not count: strict refuses it, tolerant
/// reads the same entries and names the padding (M3.G2).
#[test]
fn padded_tables_parse_tolerantly_and_name_the_padding() {
    for kind in Kind::ALL {
        let good = sample(kind).to_bytes().unwrap();
        for pad in [&[0u8, 0, 0, 0][..], &[0xCD, 0xCD][..]] {
            let mut padded = good.clone();
            padded.extend_from_slice(pad);
            assert!(
                matches!(Table::parse(kind, &padded), Err(Error::DataSize { .. })),
                "{kind:?}: strict mode unchanged"
            );
            assert!(matches!(
                Table::parse_with(kind, &padded, ParseMode::Strict),
                Err(Error::DataSize { .. })
            ));
            let parsed = Table::parse_with(kind, &padded, ParseMode::Tolerant).unwrap();
            assert_eq!(parsed.table, Table::parse(kind, &good).unwrap(), "{kind:?}");
            assert_eq!(
                parsed.padding,
                [Padding {
                    kind: PaddingKind::AfterData,
                    offset: good.len() as u64,
                    len: pad.len() as u64,
                    zeros: pad.iter().all(|&b| b == 0),
                }]
            );
            let text = parsed.padding[0].to_string();
            assert!(
                text.starts_with(&format!("{} bytes of ", pad.len()))
                    && text.contains(&format!("at offset {}", good.len()))
                    && text.contains("after the header's data size"),
                "{text}"
            );
        }
        // An unpadded table has no padding in either mode.
        let plain = Table::parse_with(kind, &good, ParseMode::Tolerant).unwrap();
        assert!(plain.padding.is_empty());
        // Shorter than the declared data is still an error.
        let mut short = good.clone();
        short.pop();
        assert!(Table::parse_with(kind, &short, ParseMode::Tolerant).is_err());
    }
}

/// Padding counted by the header (unreferenced bytes after the last string) parses in
/// both modes, as before; tolerant names it.
#[test]
fn padding_inside_the_declared_data_is_named_in_tolerant_mode() {
    let good = sample(Kind::Strings).to_bytes().unwrap();
    let mut padded = good.clone();
    padded.extend_from_slice(&[0, 0, 0]);
    let declared = u32::from_le_bytes(padded[4..8].try_into().unwrap()) + 3;
    padded[4..8].copy_from_slice(&declared.to_le_bytes());
    let strict = Table::parse(Kind::Strings, &padded).unwrap();
    assert_eq!(strict, Table::parse(Kind::Strings, &good).unwrap());
    let parsed = Table::parse_with(Kind::Strings, &padded, ParseMode::Tolerant).unwrap();
    assert_eq!(parsed.table, strict);
    assert_eq!(
        parsed.padding,
        [Padding {
            kind: PaddingKind::DataTail,
            offset: good.len() as u64,
            len: 3,
            zeros: true,
        }]
    );
    // Both kinds at once, in file order.
    padded.extend_from_slice(&[7]);
    let parsed = Table::parse_with(Kind::Strings, &padded, ParseMode::Tolerant).unwrap();
    let kinds: Vec<PaddingKind> = parsed.padding.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, [PaddingKind::DataTail, PaddingKind::AfterData]);
    assert!(!parsed.padding[1].zeros);
}

#[test]
fn tolerant_mode_survives_hostile_input() {
    for kind in Kind::ALL {
        let mut good = sample(kind).to_bytes().unwrap();
        good.extend_from_slice(&[0, 0]);
        for len in 0..good.len() {
            let _ = Table::parse_with(kind, &good[..len], ParseMode::Tolerant);
        }
        for i in 0..good.len() {
            for mask in [0x01u8, 0x80, 0xFF] {
                let mut bad = good.clone();
                bad[i] ^= mask;
                let _ = Table::parse_with(kind, &bad, ParseMode::Tolerant);
            }
        }
    }
}

#[test]
fn nul_in_a_strings_entry_is_unwritable() {
    let t = Table::new(Kind::Strings, vec![Entry::new(1, "a\0b")]);
    assert!(matches!(t.to_bytes(), Err(Error::Unwritable { id: 1, .. })));
}

#[test]
fn merge_later_wins_sorted_and_reports_conflicts() {
    let a = Table::new(
        Kind::Strings,
        vec![
            Entry::new(5, "five"),
            Entry::new(1, "one"),
            Entry::new(3, "three"),
        ],
    );
    let b = Table::new(
        Kind::Strings,
        vec![
            Entry::new(3, "THREE"),
            Entry::new(1, "one"),
            Entry::new(9, "nine"),
        ],
    );
    let merged = merge(&[a.clone(), b.clone()]).unwrap();
    let got: Vec<(u32, String)> = merged
        .table
        .entries
        .iter()
        .map(|e| (e.id, e.text().into_owned()))
        .collect();
    assert_eq!(
        got,
        [
            (1, "one".into()),
            (3, "THREE".into()),
            (5, "five".into()),
            (9, "nine".into())
        ]
    );
    assert_eq!(merged.conflicts.len(), 1);
    assert_eq!(
        (
            merged.conflicts[0].id,
            merged.conflicts[0].replaced,
            merged.conflicts[0].winner
        ),
        (3, 0, 1)
    );
    assert_eq!(merged.identical_repeats, 1);
    // Deterministic: same inputs, same bytes.
    assert_eq!(
        merged.table.to_bytes().unwrap(),
        merge(&[a.clone(), b]).unwrap().table.to_bytes().unwrap()
    );
    let dl = Table::new(Kind::DlStrings, Vec::new());
    assert!(matches!(merge(&[a, dl]), Err(Error::KindMismatch { .. })));
}

/// Regression (review round 2): an ID repeated inside one input is a duplicate (first
/// wins, as `Table::get`), not a conflict between inputs.
#[test]
fn merge_reports_in_table_duplicates_apart_from_conflicts() {
    let a = Table::new(
        Kind::Strings,
        vec![
            Entry::new(1, "first"),
            Entry::new(1, "second"),
            Entry::new(2, "two"),
            Entry::new(2, "two"),
        ],
    );
    let b = Table::new(Kind::Strings, vec![Entry::new(3, "x"), Entry::new(1, "B")]);
    let merged = merge(&[a.clone(), b]).unwrap();
    assert_eq!(
        merged.duplicates,
        [
            Duplicate {
                id: 1,
                table: 0,
                same_bytes: false
            },
            Duplicate {
                id: 2,
                table: 0,
                same_bytes: true
            },
        ]
    );
    // The cross-table conflict replaces table 0's first occurrence, not its repeat.
    assert_eq!(merged.conflicts.len(), 1);
    assert_eq!(
        (merged.conflicts[0].id, merged.conflicts[0].replaced),
        (1, 0)
    );
    assert_eq!(merged.table.get(1).unwrap().text(), "B");
    // Alone, the table keeps its first occurrence, as `get` does.
    let alone = merge(std::slice::from_ref(&a)).unwrap();
    assert_eq!(alone.table.get(1).unwrap().text(), a.get(1).unwrap().text());
    assert!(alone.conflicts.is_empty());
    assert_eq!(alone.identical_repeats, 0);
}

#[test]
fn names_and_kinds() {
    assert_eq!(
        table_name("Starfield.esm", "EN", Kind::DlStrings),
        "starfield_en.dlstrings"
    );
    assert_eq!(Kind::from_extension("ILSTRINGS"), Some(Kind::IlStrings));
    assert_eq!(
        Kind::from_path(std::path::Path::new("x/Starfield_en.STRINGS")),
        Some(Kind::Strings)
    );
    assert_eq!(Kind::from_extension("txt"), None);
    let dup = Table::new(Kind::Strings, vec![Entry::new(1, "a"), Entry::new(1, "b")]);
    assert_eq!(dup.duplicate_ids(), [1]);
    assert_eq!(dup.get(1).unwrap().text(), "a");
}
