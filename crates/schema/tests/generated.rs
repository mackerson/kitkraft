//! Exercises the golden codegen output: typed decoding of every format feature, and
//! `encode(decode(x)) == x` for well-formed, malformed and random input, `XXXX` flags
//! included. Lint compliance of the generated code itself is checked by
//! `examples/generated.rs`, which is not a test target.

#[allow(dead_code)]
mod fixture {
    include!("golden/fixture.rs");
}

#[allow(dead_code)]
mod seed {
    include!("golden/seed.rs");
}

use fixture::{
    Ctx, DecodeError, FormId, LString, OwnedSub, RawSub, Record, Sub, YyyyHeadKind, YyyyRecord,
    YyyySigned, ZString, ZzzzDataKind, ZzzzDataOptions, ZzzzRecord, RECORDS,
};

fn sub(signature: &[u8; 4], data: &[u8]) -> OwnedSub {
    OwnedSub {
        signature: *signature,
        data: data.to_vec(),
        oversized: false,
    }
}

fn big(signature: &[u8; 4], data: &[u8]) -> OwnedSub {
    OwnedSub {
        oversized: true,
        ..sub(signature, data)
    }
}

fn raw(subs: &[OwnedSub]) -> Vec<RawSub<'_>> {
    subs.iter()
        .map(|s| RawSub {
            signature: s.signature,
            data: &s.data,
            oversized: s.oversized,
        })
        .collect()
}

fn ctx(form_version: u16, localized: bool) -> Ctx {
    Ctx {
        form_version,
        localized,
    }
}

fn value<T: Clone>(slot: &Option<Sub<T>>) -> Option<T> {
    slot.as_ref().map(|s| s.value.clone())
}

fn data_payload(kind: i32) -> Vec<u8> {
    let mut d = Vec::new();
    d.push(7u8); // Count
    d.push(0xFE); // Delta = -2
    d.extend_from_slice(&0x1234u16.to_le_bytes());
    d.extend_from_slice(&(-5i16).to_le_bytes());
    d.extend_from_slice(&1.5f32.to_le_bytes());
    d.extend_from_slice(&0x0001_0F00u32.to_le_bytes());
    d.extend_from_slice(&kind.to_le_bytes());
    d.extend_from_slice(&0x8001u16.to_le_bytes());
    d.extend_from_slice(b"MODE");
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&2u16.to_le_bytes());
    d.extend_from_slice(&[9, 8, 7]);
    d.extend_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(d.len(), 39);
    d
}

fn well_formed() -> Vec<OwnedSub> {
    let mut vers = 3u32.to_le_bytes().to_vec();
    vers.extend_from_slice(&2.25f64.to_le_bytes());
    let mut points = Vec::new();
    for v in [1i32, -1, 2, -2] {
        points.extend_from_slice(&v.to_le_bytes());
    }
    let mut one_point = Vec::new();
    for v in [3i32, -3] {
        one_point.extend_from_slice(&v.to_le_bytes());
    }
    vec![
        sub(b"EDID", b"Thing\0"),
        sub(b"FULL", &[1, 0, 0, 0]),
        sub(b"DATA", &data_payload(-1)),
        sub(b"VERS", &vers),
        sub(b"KWDA", &[1, 0, 0, 0, 2, 0, 0, 1]),
        sub(b"ENAM", &[]),
        sub(b"ENTR", &points),
        sub(b"ENTR", &one_point),
        sub(b"BEGN", b"Kind\0"),
        sub(b"QQQQ", &[1, 2, 3]),
        sub(b"RRRR", &[]),
        sub(b"BEND", &[]),
        sub(b"TAIL", &[5, 0, 0, 0, 0xAA, 0xBB]),
        sub(b"BIG8", &(-9i64).to_le_bytes()),
        sub(b"SKIP", &[1, 2]),
        sub(b"SKIP", &[3, 4]),
    ]
}

fn roundtrip(subs: &[OwnedSub], ctx: Ctx) -> ZzzzRecord {
    let raws = raw(subs);
    let record = ZzzzRecord::decode(&raws, ctx);
    assert_eq!(record.encode(), subs, "roundtrip changed the record");
    record
}

fn roundtrip_unor(subs: &[OwnedSub], ctx: Ctx) -> fixture::UnorRecord {
    let raws = raw(subs);
    let record = fixture::UnorRecord::decode(&raws, ctx);
    assert_eq!(record.encode(), subs, "roundtrip changed the record");
    record
}

fn roundtrip_y(subs: &[OwnedSub], ctx: Ctx) -> YyyyRecord {
    let raws = raw(subs);
    let record = YyyyRecord::decode(&raws, ctx);
    assert_eq!(record.encode(), subs, "roundtrip changed the record");
    record
}

#[test]
fn fixture_decodes_every_type() {
    let rec = roundtrip(&well_formed(), ctx(100, true));
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(value(&rec.editor_id), Some(ZString(b"Thing".to_vec())));
    assert_eq!(value(&rec.name), Some(LString::Id(1)));
    let data = rec.data.as_ref().unwrap();
    assert_eq!(data.count, 7);
    assert_eq!(data.delta, -2);
    assert_eq!(data.small, 0x1234);
    assert_eq!(data.small_signed, -5);
    assert_eq!(data.speed, 1.5);
    assert_eq!(data.target, FormId(0x0001_0F00));
    assert_eq!(data.kind, ZzzzDataKind::Right);
    assert!(data
        .options
        .contains(ZzzzDataOptions::HIDDEN | ZzzzDataOptions::LOCKED));
    assert_eq!(&data.mode, b"MODE");
    assert_eq!(data.pair, [1, 2]);
    assert_eq!(data.blob, [9, 8, 7]);
    assert_eq!(data.wide, u64::MAX);
    let vers = rec.versioned.as_ref().unwrap();
    assert_eq!((vers.base, vers.added), (3, Some(2.25)));
    assert_eq!(
        rec.keywords.as_ref().unwrap().0,
        vec![FormId(1), FormId(0x0100_0002)]
    );
    assert_eq!(rec.entry.len(), 2);
    assert!(rec.entry[0].marker.is_some());
    assert!(rec.entry[1].marker.is_none());
    assert_eq!(rec.entry[0].points.as_ref().unwrap().0.len(), 2);
    assert_eq!(rec.block.len(), 1);
    assert_eq!(rec.block[0].body.len(), 2);
    assert_eq!(rec.tail.as_ref().unwrap().rest.0, vec![0xAA, 0xBB]);
    assert_eq!(value(&rec.big_value), Some(-9));
    assert_eq!(rec.skipped.len(), 2);
}

#[test]
fn lstring_follows_the_localized_flag() {
    let mut subs = well_formed();
    // Not localized: FULL is inline text.
    subs[1] = sub(b"FULL", b"Name\0");
    let rec = roundtrip(&subs, ctx(100, false));
    assert_eq!(
        value(&rec.name),
        Some(LString::Text(ZString(b"Name".to_vec())))
    );
    // The same text in a localized plugin is not a 4-byte ID: kept raw.
    let rec = roundtrip(&subs, ctx(100, true));
    assert!(rec.name.is_none());
    assert_eq!(rec.extra[0].0, 1);
    // An ID in a plugin that is not localized has no NUL terminator: kept raw.
    subs[1] = sub(b"FULL", &[1, 2, 3, 4]);
    let rec = roundtrip(&subs, ctx(100, false));
    assert!(rec.name.is_none());
    // Unterminated bytes stay raw in `extra` and still roundtrip.
    assert_eq!(rec.extra[0].1.data, vec![1, 2, 3, 4]);
}

#[test]
fn xxxx_flags_roundtrip_through_typed_members_extra_and_raw_lists() {
    let mut subs = well_formed();
    subs[0] = big(b"EDID", b"Thing\0"); // typed, small payload with XXXX
    subs[9] = big(b"QQQQ", &[1, 2, 3]); // inside an any_until body
    subs[14] = big(b"SKIP", &[1, 2]); // repeated typed member
    subs.push(big(b"UNKN", &[0; 3])); // unmatched: extra
    let rec = roundtrip(&subs, ctx(100, true));
    assert!(rec.editor_id.as_ref().unwrap().oversized);
    assert!(rec.skipped[0].oversized && !rec.skipped[1].oversized);
    assert!(rec.block[0].body[0].oversized);
    assert!(rec.extra.iter().any(|(_, s)| s.oversized));
    // Without the flag the same bytes come back without it.
    let plain: Vec<OwnedSub> = subs
        .iter()
        .map(|s| OwnedSub {
            oversized: false,
            ..s.clone()
        })
        .collect();
    let rec = roundtrip(&plain, ctx(100, true));
    assert!(!rec.editor_id.as_ref().unwrap().oversized);
}

#[test]
fn edited_values_encode() {
    let mut rec = roundtrip(&well_formed(), ctx(100, true));
    rec.editor_id = Some(Sub::from(ZString(b"Other".to_vec())));
    rec.big_value.as_mut().unwrap().value = 1;
    let out = rec.encode();
    assert_eq!(out[0], sub(b"EDID", b"Other\0"));
    assert_eq!(out[13], sub(b"BIG8", &1i64.to_le_bytes()));
}

#[test]
fn unknown_enum_values_are_preserved() {
    let mut subs = well_formed();
    subs[2] = sub(b"DATA", &data_payload(77));
    let rec = roundtrip(&subs, ctx(100, true));
    assert_eq!(rec.data.unwrap().value.kind, ZzzzDataKind::Other(77));
}

#[test]
fn version_gated_fields() {
    let mut subs = well_formed();
    subs[3] = sub(b"VERS", &3u32.to_le_bytes());
    let old = roundtrip(&subs, ctx(99, true));
    assert_eq!(old.versioned.unwrap().value.added, None);
    // The same 4-byte payload is too short for version 100: kept raw, still roundtrips.
    let new = roundtrip(&subs, ctx(100, true));
    assert!(new.versioned.is_none());
    assert_eq!(new.extra.len(), 1);
    assert_eq!(new.extra[0].0, 3);
}

#[test]
fn malformed_input_is_kept_raw_and_roundtrips() {
    let cases: Vec<Vec<OwnedSub>> = vec![
        vec![],
        vec![sub(b"EDID", b"no terminator")],
        vec![sub(b"DATA", &[0; 38])],
        vec![sub(b"SKIP", &[1, 2]), sub(b"EDID", b"late\0")],
        vec![
            sub(b"XXXX", &[1]),
            sub(b"EDID", b"a\0"),
            sub(b"EDID", b"b\0"),
        ],
        vec![sub(b"BEGN", b"k\0"), sub(b"EDID", b"swallowed\0")],
        vec![sub(b"ENTR", &[1, 2, 3])],
        vec![sub(b"FULL", &[]), sub(b"BEND", &[]), sub(b"OBTF", &[])],
    ];
    for subs in &cases {
        roundtrip(subs, ctx(100, true));
        roundtrip(subs, ctx(100, false));
    }
}

// ---------------------------------------------------------------------------------------
// YYYY: groups, whole-payload scalars, strings, gated arrays, count_from, shared defs.

fn list_payload(count: u8, points: &[(i16, i16)], tail: Option<u32>) -> Vec<u8> {
    let mut d = vec![count];
    for (x, y) in points {
        d.extend_from_slice(&x.to_le_bytes());
        d.extend_from_slice(&y.to_le_bytes());
    }
    d.extend_from_slice(&3u16.to_le_bytes());
    d.extend_from_slice(b"abc");
    d.push(2);
    d.extend_from_slice(b"xy");
    d.extend_from_slice(&[1, 2, 3, 4]);
    if let Some(tail) = tail {
        d.extend_from_slice(&tail.to_le_bytes());
    }
    d
}

fn yyyy_body() -> Vec<OwnedSub> {
    let mut gate = 7u16.to_le_bytes().to_vec();
    gate.extend_from_slice(&[1, 0, 2, 0]); // Pair (10..=19)
    gate.extend_from_slice(&[9, 9, 9, 9]); // Quad (10..)
    vec![
        sub(b"HNAM", &[2]),
        sub(b"HFLG", &[0x81]),
        sub(b"SCAL", &0.5f32.to_le_bytes()),
        sub(b"WIDE", &0x4000_0000_0000_0001u64.to_le_bytes()),
        sub(b"SIGN", &(-2i16).to_le_bytes()),
        sub(b"ONAM", b"\x05\0\0\0label\0"),
        sub(b"LNAM", b"\x06\0text\0"),
        sub(b"GATE", &gate),
        sub(b"GRST", &[1, 4, 0, 5, 0]),
        sub(b"PNTS", &[1, 0, 2, 0, 3, 0, 4, 0]),
        sub(b"LIST", &list_payload(2, &[(1, -1), (2, -2)], Some(42))),
        sub(b"REFP", &[0xFF, 0xFF, 1, 0]),
        sub(b"MRKB", &[]),
        sub(b"MRKD", &[1, 0, 0, 0]),
        sub(b"MRKB", &[]),
    ]
}

#[test]
fn yyyy_decodes_every_feature() {
    let rec = roundtrip_y(&yyyy_body(), ctx(15, false));
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    let head = rec.head.as_ref().unwrap();
    assert_eq!(value(&head.kind), Some(YyyyHeadKind::Second));
    let bits = head.bits.as_ref().unwrap();
    assert!(bits.contains(fixture::YyyyHeadBits::LOW | fixture::YyyyHeadBits::HIGH));
    assert_eq!(value(&rec.scale), Some(0.5));
    assert!(rec.mask.as_ref().unwrap().contains(fixture::YyyyMask::TOP));
    assert_eq!(rec.mask.as_ref().unwrap().0 & 1, 1);
    assert_eq!(value(&rec.signed), Some(YyyySigned::Negative));
    let opt = rec.opt.as_ref().unwrap();
    assert_eq!(opt.text.as_ref().unwrap().label, ZString(b"label".to_vec()));
    assert_eq!(
        opt.local.as_ref().unwrap().text,
        LString::Text(ZString(b"text".to_vec()))
    );
    let gate = rec.gated.as_ref().unwrap();
    assert_eq!(
        (gate.base, gate.pair, gate.quad),
        (7, Some([1, 2]), Some([9; 4]))
    );
    assert_eq!(rec.gated_rest.as_ref().unwrap().items, Some(vec![4, 5]));
    let points = &rec.points.as_ref().unwrap().0;
    assert_eq!((points[0].x, points[1].y), (1, 4));
    let list = rec.list.as_ref().unwrap();
    assert_eq!(list.count, 2);
    assert_eq!((list.items[1].x, list.items[1].y), (2, -2));
    assert_eq!(list.title.0, b"abc");
    assert_eq!(list.short.0, b"xy");
    assert_eq!((list.pairs[0].a, list.pairs[1].b), (1, 4));
    assert_eq!(list.tail, Some(42));
    assert_eq!(
        (
            rec.anchor.as_ref().unwrap().x,
            rec.anchor.as_ref().unwrap().y
        ),
        (-1, 1)
    );
    assert_eq!(rec.marker.len(), 2);
    assert_eq!(value(&rec.marker[0].value), Some(1));
    assert!(rec.marker[1].value.is_none());
}

#[test]
fn yyyy_version_gates_and_trailing_fields() {
    let mut subs = yyyy_body();
    // Version 25: Pair is gone, Quad stays: 6 bytes.
    subs[7] = sub(b"GATE", &[7, 0, 9, 9, 9, 9]);
    let rec = roundtrip_y(&subs, ctx(25, false));
    let gate = rec.gated.as_ref().unwrap();
    assert_eq!((gate.pair, gate.quad), (None, Some([9; 4])));
    // Version 3: only Base, and GRST has no rest array before version 5.
    subs[7] = sub(b"GATE", &[7, 0]);
    subs[8] = sub(b"GRST", &[1]);
    let rec = roundtrip_y(&subs, ctx(3, false));
    assert_eq!(rec.gated.as_ref().unwrap().quad, None);
    assert_eq!(rec.gated_rest.as_ref().unwrap().items, None);
    // if_remaining: absent tail decodes as None; a partial tail fails and stays raw.
    subs[10] = sub(b"LIST", &list_payload(1, &[(5, 6)], None));
    let rec = roundtrip_y(&subs, ctx(3, false));
    assert_eq!(rec.list.as_ref().unwrap().tail, None);
    let mut partial = list_payload(1, &[(5, 6)], None);
    partial.extend_from_slice(&[1, 2]);
    subs[10] = sub(b"LIST", &partial);
    let rec = roundtrip_y(&subs, ctx(3, false));
    assert!(rec.list.is_none());
    assert_eq!(rec.extra[0].0, 10);
}

#[test]
fn yyyy_count_from_and_length_prefixes_reject_bad_input() {
    let cases = [
        list_payload(3, &[(1, 1)], None), // count says 3, only 1 point (then misparse)
        list_payload(200, &[], None),     // hostile count
        vec![0, 9, 0, b'a'],              // wstring longer than the payload
        vec![0, 0, 0, 5, b'a'],           // bstring longer than the payload
    ];
    for payload in cases {
        let subs = vec![sub(b"HNAM", &[1]), sub(b"LIST", &payload)];
        let rec = roundtrip_y(&subs, ctx(3, false));
        assert!(rec.list.is_none(), "{payload:?}");
        assert_eq!(rec.extra.len(), 1);
    }
}

/// M3.B10: encode writes a `count_from` count from its array's length, so an edited array
/// (or a stale count) never yields an inconsistent payload, and an unedited one re-encodes
/// byte for byte.
#[test]
fn yyyy_count_from_is_written_from_the_array_length() {
    let decode = |subs: &[OwnedSub]| YyyyRecord::decode(&raw(subs), ctx(15, false));
    let list_of = |subs: &[OwnedSub]| subs.iter().find(|s| &s.signature == b"LIST").cloned();

    // Unedited: identical.
    let body = yyyy_body();
    assert_eq!(decode(&body).encode(), body);

    // Add an element without touching the count.
    let mut rec = decode(&body);
    let items = &mut rec.list.as_mut().unwrap().value.items;
    items.push(fixture::Point { x: 3, y: -3 });
    let encoded = rec.encode();
    assert_eq!(
        list_of(&encoded).unwrap().data,
        list_payload(3, &[(1, -1), (2, -2), (3, -3)], Some(42))
    );
    let again = decode(&encoded);
    assert!(again.extra.is_empty(), "{:?}", again.extra);
    assert_eq!(again.list.as_ref().unwrap().items.len(), 3);

    // A stale count is not written: the array decides.
    let mut rec = decode(&body);
    rec.list.as_mut().unwrap().value.count = 9;
    assert_eq!(rec.encode(), body);
    rec.list.as_mut().unwrap().value.items.clear();
    assert_eq!(
        list_of(&rec.encode()).unwrap().data,
        list_payload(0, &[], Some(42))
    );

    // More elements than the `u8` count holds: the count saturates and the array is cut
    // to it, so the payload still decodes.
    let mut rec = decode(&body);
    rec.list.as_mut().unwrap().value.items =
        (0..300).map(|i| fixture::Point { x: i, y: 0 }).collect();
    let encoded = rec.encode();
    assert_eq!(list_of(&encoded).unwrap().data.first(), Some(&255));
    let again = decode(&encoded);
    assert!(again.extra.is_empty(), "{:?}", again.extra);
    let list = again.list.as_ref().unwrap();
    assert_eq!(
        (list.count, list.items.len(), list.tail),
        (255, 255, Some(42))
    );
    assert_eq!(list.items.last().map(|p| p.x), Some(254));
}

/// M3.B10, a gated `count_from` array: present (form version 10+), its count follows the
/// array; absent (before 10), the count is written as stored.
#[test]
fn vvvv_gated_count_from_array() {
    let cntg = |count: u16, items: &[u8]| {
        let mut d = count.to_le_bytes().to_vec();
        d.push(0xAA);
        d.extend_from_slice(items);
        sub(b"CNTG", &d)
    };
    // Version 9: no array, the stored count (5) survives an encode.
    let old = vec![cntg(5, &[])];
    let rec = fixture::VvvvRecord::decode(&raw(&old), ctx(9, false));
    let gated = rec.gated_count.as_ref().unwrap();
    assert_eq!((gated.count, gated.items.as_ref()), (5, None));
    assert_eq!(rec.encode(), old);
    // Version 10: the array is there; an edit rewrites the count.
    let new = vec![cntg(2, &[7, 8])];
    let mut rec = fixture::VvvvRecord::decode(&raw(&new), ctx(10, false));
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(rec.encode(), new);
    rec.gated_count
        .as_mut()
        .unwrap()
        .value
        .items
        .as_mut()
        .unwrap()
        .push(9);
    assert_eq!(rec.encode(), vec![cntg(3, &[7, 8, 9])]);
}

#[test]
fn empty_record_keeps_everything_in_extra() {
    let subs = vec![sub(b"EDID", b"x\0"), big(b"DATA", &[1, 2])];
    let raws = raw(&subs);
    let rec = fixture::EmptRecord::decode(&raws, ctx(1, false));
    assert_eq!(rec.extra.len(), 2);
    assert_eq!(rec.encode(), subs);
}

#[test]
fn record_dispatch() {
    let subs = yyyy_body();
    let raws = raw(&subs);
    let rec = Record::decode(*b"YYYY", &raws, ctx(15, false)).unwrap();
    assert_eq!(rec.signature(), *b"YYYY");
    assert!(matches!(&rec, Record::Yyyy(y) if y.extra.is_empty()));
    assert_eq!(rec.encode(), subs);
    assert!(Record::decode(*b"NONE", &raws, ctx(15, false)).is_none());
    let sigs: Vec<[u8; 4]> = RECORDS.iter().map(|(sig, _)| *sig).collect();
    assert_eq!(
        sigs,
        [*b"EMPT", *b"UNOR", *b"UUUU", *b"VVVV", *b"YYYY", *b"ZZZZ"]
    );
    assert_eq!(fixture::UNORDERED, [*b"UNOR"]);
    assert!(rec.extra().is_empty());
}

#[test]
fn decode_errors_display_and_are_errors() {
    let err = DecodeError::Truncated {
        field: "ZZZZ.DATA.Count",
        offset: 3,
    };
    assert_eq!(err.to_string(), "ZZZZ.DATA.Count: payload ends at offset 3");
    let boxed: Box<dyn std::error::Error> = Box::new(err);
    assert!(boxed.to_string().contains("offset 3"));
    let result: Result<fixture::Point, DecodeError> =
        fixture::decode(&[1, 0, 2], "Point", ctx(0, false));
    assert!(matches!(result, Err(DecodeError::Truncated { .. })));
}

/// Deterministic pseudo-random bodies from a pool of known and unknown signatures with
/// random payload sizes: whatever the order, decoding then encoding is the identity.
#[test]
fn random_bodies_roundtrip() {
    let pool: [&[u8; 4]; 29] = [
        b"EDID", b"FULL", b"DATA", b"VERS", b"KWDA", b"ENAM", b"ENTR", b"BEGN", b"BEND", b"TAIL",
        b"BIG8", b"SKIP", b"QQQQ", b"ZZZZ", b"HNAM", b"HFLG", b"SCAL", b"WIDE", b"ONAM", b"LNAM",
        b"GATE", b"GRST", b"PNTS", b"LIST", b"MRKB", b"MRKD", b"NAME", b"PKEY", b"PVAL",
    ];
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    for _ in 0..3000 {
        let len = next() % 14;
        let mut subs = Vec::new();
        for _ in 0..len {
            let sig = pool[(next() % pool.len() as u64) as usize];
            let size = match next() % 5 {
                0 => 0,
                1 => 4,
                2 => 39,
                3 => 10,
                _ => (next() % 20) as usize,
            };
            let data: Vec<u8> = (0..size).map(|_| next() as u8).collect();
            let mut s = sub(sig, &data);
            s.oversized = next() % 7 == 0;
            subs.push(s);
        }
        let c = ctx((next() % 200) as u16, next() % 2 == 0);
        roundtrip(&subs, c);
        roundtrip_y(&subs, c);
        roundtrip_unor(&subs, c);
        let raws = raw(&subs);
        let empt = Record::decode(*b"EMPT", &raws, c).unwrap();
        assert_eq!(empt.encode(), subs);
    }
}

/// M4.BX1: an unordered record places every subrecord whatever its position (the ordered
/// matcher would leave the out-of-order ones raw), encodes the values back in on-disk
/// order, and after edits writes the remaining values in that order with added ones last.
#[test]
fn unordered_record_places_members_in_any_order() {
    let subs = vec![
        sub(b"NAME", &7u32.to_le_bytes()),
        sub(b"FULL", b"Hi\0"),
        sub(b"PKEY", &[1]),
        sub(b"PVAL", &[2, 0]),
        sub(b"EDID", b"Unor\0"),
        big(b"NAME", &8u32.to_le_bytes()),
        sub(b"DATA", &5u32.to_le_bytes()),
        sub(b"PKEY", &[3]),
    ];
    let raws = raw(&subs);
    let rec = roundtrip_unor(&subs, ctx(1, false));
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(rec.refs.len(), 2);
    assert!(rec.refs[1].oversized);
    assert_eq!(rec.pair.len(), 2);
    assert_eq!(rec.disk_order, [3, 4, 2, 0, 3, 1, 2]);
    let (_, ordered) = fixture::match_members(&raws, fixture::UnorRecord::MEMBERS);
    assert!(!ordered.is_empty());
    let (_, unmatched) = fixture::match_body(*b"UNOR", &raws, fixture::UnorRecord::MEMBERS);
    assert!(unmatched.is_empty());
    let typed = Record::decode(*b"UNOR", &raws, ctx(1, false)).unwrap();
    assert_eq!(typed.encode(), subs);
    // Edits: a removed value drops out, the others keep their order, an added one follows.
    let mut edited = rec.clone();
    edited.refs.remove(0);
    edited.refs.push(Sub::new(FormId(9)));
    edited.editor_id = None;
    let sigs: Vec<[u8; 4]> = edited.encode().iter().map(|s| s.signature).collect();
    assert_eq!(
        sigs,
        [*b"NAME", *b"FULL", *b"PKEY", *b"PVAL", *b"NAME", *b"DATA", *b"PKEY"]
    );
    // A value set on a fresh record (no recorded order) is written in definition order.
    let fresh = fixture::UnorRecord {
        name: Some(Sub::new(LString::Text(ZString(b"X".to_vec())))),
        editor_id: Some(Sub::new(ZString(b"E".to_vec()))),
        ..Default::default()
    };
    let sigs: Vec<[u8; 4]> = fresh.encode().iter().map(|s| s.signature).collect();
    assert_eq!(sigs, [*b"EDID", *b"FULL"]);
    // A second EDID (an optional member already filled) stays raw, in place: the first one
    // read fills the member.
    let mut twice = subs.clone();
    twice.insert(1, sub(b"EDID", b"Again\0"));
    let rec = roundtrip_unor(&twice, ctx(1, false));
    assert_eq!(rec.extra.len(), 1);
    assert_eq!(rec.extra[0].0, 5);
}

// ---------------------------------------------------------------------------------------
// The checked-in definitions.

fn seed_raw(subs: &[seed::OwnedSub]) -> Vec<seed::RawSub<'_>> {
    subs.iter()
        .map(|x| seed::RawSub {
            signature: x.signature,
            data: &x.data,
            oversized: x.oversized,
        })
        .collect()
}

fn s(sig: &[u8; 4], data: &[u8]) -> seed::OwnedSub {
    seed::OwnedSub {
        signature: *sig,
        data: data.to_vec(),
        oversized: false,
    }
}

#[test]
fn seed_tes4_and_headers() {
    use seed::{Ctx, FormId, GroupHeader, GroupHeaderGroupType, RecordHeader, Tes4Record};
    let mut hedr = 0.96f32.to_le_bytes().to_vec();
    hedr.extend_from_slice(&10u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let subs = vec![
        s(b"HEDR", &hedr),
        s(b"CNAM", b"author\0"),
        s(b"SNAM", b"\0"),
        s(b"MAST", b"Starfield.esm\0"),
        s(b"MAST", b"Other.esm\0"),
        s(b"DATA", &[0; 8]),
        s(b"ONAM", &[1, 0, 0, 0, 2, 0, 0, 0]),
        s(b"TNAM", &[1, 2, 3]),
        s(b"TNAM", &[4]),
        s(b"BNAM", &[0; 9]),
        s(b"INCC", &[5, 0, 0, 0]),
        s(b"CHGL", &[0; 4]),
    ];
    let raws = seed_raw(&subs);
    let rec = Tes4Record::decode(&raws, Ctx::default());
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(rec.header.as_ref().unwrap().record_count, 10);
    assert_eq!(rec.master.len(), 2);
    assert_eq!(
        rec.master[0].file_name.as_ref().unwrap().0,
        b"Starfield.esm"
    );
    assert!(rec.master[0].file_size.is_none());
    assert_eq!(rec.master[1].file_size.as_ref().map(|v| v.value), Some(0));
    assert_eq!(
        rec.overridden_forms.as_ref().unwrap().0,
        vec![FormId(1), FormId(2)]
    );
    assert_eq!(rec.tnam.len(), 2);
    assert_eq!(rec.incc.as_ref().map(|v| v.value), Some(5));
    assert_eq!(rec.encode(), subs);
    assert_eq!(Tes4Record::FLAG_LOCALIZED, 0x80);

    let mut grup = b"GRUP".to_vec();
    grup.extend_from_slice(&100u32.to_le_bytes());
    grup.extend_from_slice(b"WEAP");
    grup.extend_from_slice(&0i32.to_le_bytes());
    grup.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let header = GroupHeader::decode(&grup).unwrap();
    assert_eq!(&header.signature, b"GRUP");
    assert_eq!(header.group_type, GroupHeaderGroupType::Top);
    assert_eq!(&header.label, b"WEAP");
    assert_eq!(seed::encode(&header), grup);
    assert!(GroupHeader::decode(&grup[..23]).is_err());

    let mut rh = b"WEAP".to_vec();
    rh.extend_from_slice(&77u32.to_le_bytes());
    rh.extend_from_slice(&0x0004_0001u32.to_le_bytes());
    rh.extend_from_slice(&0x0001_2345u32.to_le_bytes());
    rh.extend_from_slice(&[1, 0, 2, 0]);
    rh.extend_from_slice(&582u16.to_le_bytes());
    rh.extend_from_slice(&[0, 0]);
    let header = RecordHeader::decode(&rh).unwrap();
    assert_eq!(&header.signature, b"WEAP");
    assert_eq!(header.data_size, 77);
    assert!(header.flags.contains(seed::RecordHeaderFlags::COMPRESSED));
    assert_eq!(header.form_id, FormId(0x0001_2345));
    assert_eq!(header.form_version, 582);
    assert_eq!(seed::encode(&header), rh);
    assert_eq!(RecordHeader::SIZE, 24);
}

#[test]
fn seed_weap_typed_shape() {
    use seed::{Ctx, WeapRecord};
    let sigs: [&[u8; 4]; 22] = [
        b"EDID", b"OBND", b"ODTY", b"BFCB", b"REFL", b"BFCE", b"FULL", b"FLLD", b"KSIZ", b"KWDA",
        b"DESC", b"OBTE", b"OBTF", b"FULL", b"OBTS", b"FULL", b"OBTS", b"STOP", b"NNAM", b"WFIR",
        b"FLLD", b"WTRM",
    ];
    // Well-formed payloads for a localized plugin (lane B round 3 layouts); the component
    // data inside the BFCB block is the reflection layout (its REFL kept as bytes).
    let kwda = [7u32.to_le_bytes(), 0x0100_0009u32.to_le_bytes()].concat();
    // A template item with no includes or properties and one keyword (22 bytes).
    let mut obts = vec![0u8; 15];
    obts.push(1);
    obts.extend_from_slice(&0x0100_0042u32.to_le_bytes());
    obts.extend_from_slice(&[0, 0]);
    let subs: Vec<seed::OwnedSub> = sigs
        .iter()
        .enumerate()
        .map(|(i, sig)| match *sig {
            b"EDID" => s(sig, b"Gun\0"),
            b"BFCB" => s(sig, b"T\0"),
            b"FULL" | b"DESC" => s(sig, &(0x0100_0000 + i as u32).to_le_bytes()),
            b"KSIZ" => s(sig, &2u32.to_le_bytes()),
            b"KWDA" => s(sig, &kwda),
            b"OBND" => s(sig, &[0; 24]),
            b"ODTY" | b"FLLD" | b"NNAM" => s(sig, &[i as u8, 0, 0, 0]),
            b"OBTE" => s(sig, &2u32.to_le_bytes()),
            b"OBTS" => s(sig, &obts),
            b"WFIR" => s(sig, &[0; 45]),
            b"WTRM" => s(sig, &[0; 17]),
            b"BFCE" | b"OBTF" | b"STOP" => s(sig, &[]),
            _ => s(sig, &vec![i as u8; i % 5]),
        })
        .collect();
    let raws = seed_raw(&subs);
    let localized = Ctx {
        form_version: 582,
        localized: true,
    };
    let rec = WeapRecord::decode(&raws, localized);
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(rec.bfcb_block.len(), 1);
    assert!(rec.bfcb_block[0].component_data.is_empty());
    assert!(rec.bfcb_block[0].reflection.is_some());
    assert_eq!(rec.bfcb_block[0].component_type.as_ref().unwrap().0, b"T");
    assert_eq!(rec.editor_id.as_ref().unwrap().0, b"Gun");
    assert_eq!(**rec.name.as_ref().unwrap(), seed::LString::Id(0x0100_0006));
    let keywords = rec.keywords.as_ref().unwrap();
    assert_eq!(**keywords.count.as_ref().unwrap(), 2);
    assert_eq!(keywords.forms.as_ref().unwrap().0.len(), 2);
    let ids: Vec<(&str, u32)> = rec.form_ids().into_iter().map(|(p, f)| (p, f.0)).collect();
    let kw: Vec<u32> = ids
        .iter()
        .filter(|(p, _)| *p == "Keywords.KWDA")
        .map(|(_, f)| *f)
        .collect();
    assert_eq!(kw, [7, 0x0100_0009]);
    assert!(ids.contains(&("WEAP.NNAM", 18)), "{ids:?}");
    // FULL, DESC and the two FULL entries of the object template.
    assert_eq!(rec.lstrings().len(), 4);
    // Without the localized flag a 4-byte FULL is no zstring: it stays raw, losslessly.
    let plain = WeapRecord::decode(&raws, Ctx::default());
    assert_eq!(plain.extra.len(), 4);
    assert_eq!(plain.encode(), subs);
    let block = rec.obte_block.as_ref().unwrap();
    assert_eq!(**block.combination_count.as_ref().unwrap(), 2);
    assert!(block.end_marker.is_some());
    assert_eq!(block.combination.len(), 2);
    assert!(block.combination[0].editor_only.is_some());
    assert!(block.combination[1].editor_only.is_none());
    let item = block.combination[0].template_item.as_ref().unwrap();
    assert_eq!(item.keywords.len(), 1);
    // FLLD sits in the model block and in the first-person model block.
    assert!(rec.model.as_ref().unwrap().light_layer.is_some());
    assert!(rec
        .first_person_model
        .as_ref()
        .unwrap()
        .light_layer
        .is_some());
    assert_eq!(rec.encode(), subs);

    // STOP on its own (no OBTE) still forms the block.
    let lone = vec![s(b"EDID", b"w\0"), s(b"STOP", &[])];
    let rec = WeapRecord::decode(&seed_raw(&lone), Ctx::default());
    let block = rec.obte_block.as_ref().unwrap();
    assert!(block.combination_count.is_none() && block.end_marker.is_some());
    assert_eq!(rec.encode(), lone);
}

#[test]
fn seed_glob_gmst_kywd_inventory_shapes() {
    use seed::{Ctx, GlobRecord, GmstRecord, KywdRecord};
    // GLOB "EDID BFCB QUPA BFCE FLTV" and "FLTV" shapes (QUPA: a component layout the
    // block keeps raw).
    let glob = vec![
        s(b"EDID", b"Gv\0"),
        s(b"BFCB", b"Type\0"),
        s(b"QUPA", &[1, 2]),
        s(b"BFCE", &[]),
        s(b"FLTV", &1.0f32.to_le_bytes()),
    ];
    let rec = GlobRecord::decode(&seed_raw(&glob), Ctx::default());
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(rec.editor_id.as_ref().unwrap().0, b"Gv");
    assert_eq!(rec.bfcb_block.len(), 1);
    assert_eq!(rec.bfcb_block[0].component_data.len(), 1);
    assert_eq!(**rec.value.as_ref().unwrap(), 1.0);
    assert_eq!(rec.encode(), glob);
    let bare = vec![s(b"FLTV", &[0; 4])];
    let rec = GlobRecord::decode(&seed_raw(&bare), Ctx::default());
    assert!(rec.editor_id.is_none() && rec.value.is_some() && rec.extra.is_empty());

    // GMST "EDID XALG DATA": the EditorID prefix picks the DATA layout (M2.B9).
    let localized = Ctx {
        form_version: 582,
        localized: true,
    };
    let gmst = vec![
        s(b"EDID", b"fX\0"),
        s(b"XALG", &[0; 8]),
        s(b"DATA", &[0, 0, 128, 63]),
    ];
    let rec = GmstRecord::decode(&seed_raw(&gmst), localized);
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(**rec.xalg_flags.as_ref().unwrap(), 0);
    assert_eq!(**rec.value.as_ref().unwrap(), seed::GmstValue::Float(1.0));
    assert_eq!(rec.encode(), gmst);
    for (edid, data, value) in [
        (&b"bOn\0"[..], 1u32.to_le_bytes(), seed::GmstValue::Bool(1)),
        (b"iCount\0", (-3i32).to_le_bytes(), seed::GmstValue::Int(-3)),
        (b"uSize\0", 9u32.to_le_bytes(), seed::GmstValue::Unsigned(9)),
        (
            b"sLabel\0",
            0x0003_3055u32.to_le_bytes(),
            seed::GmstValue::String(seed::LString::Id(0x0003_3055)),
        ),
    ] {
        let subs = vec![s(b"EDID", edid), s(b"DATA", &data)];
        let rec = GmstRecord::decode(&seed_raw(&subs), localized);
        assert_eq!(**rec.value.as_ref().unwrap(), value);
        assert_eq!(rec.value.as_ref().unwrap().prefix(), edid[0]);
        assert_eq!(rec.encode(), subs);
    }
    // A prefix no variant names keeps DATA raw, losslessly.
    let odd = vec![s(b"EDID", b"xOdd\0"), s(b"DATA", &[1, 2, 3, 4])];
    let rec = GmstRecord::decode(&seed_raw(&odd), localized);
    assert!(rec.value.is_none() && rec.extra.len() == 1);
    assert_eq!(rec.encode(), odd);

    // KYWD with a BFCB block, notes and FULL (M3.B6 layouts); a stray subrecord goes to
    // extra, and so does a DNAM that is not NUL-terminated text.
    let kywd = vec![
        s(b"EDID", b"Kw\0"),
        s(b"BFCB", b"T\0"),
        s(b"BFCE", &[]),
        s(b"CNAM", &[10, 20, 30, 255]),
        s(b"DNAM", b"note\0"),
        s(b"TNAM", &[29, 0, 0, 0]),
        s(b"FNAM", &[0; 4]),
        s(b"ENAM", b"Link\0"),
        s(b"FULL", &[1, 0, 0, 0]),
        s(b"QQQQ", &[]),
    ];
    let rec = KywdRecord::decode(&seed_raw(&kywd), Ctx::default());
    assert_eq!(rec.extra.len(), 1);
    assert_eq!(rec.extra[0].0, 9);
    assert!(rec.bfcb_block[0].component_data.is_empty());
    assert_eq!(rec.color.as_ref().unwrap().alpha, 255);
    assert_eq!(rec.notes.as_ref().unwrap().0, b"note");
    assert_eq!(**rec.keyword_type.as_ref().unwrap(), 29);
    assert_eq!(rec.flash_linkage_name.as_ref().unwrap().0, b"Link");
    assert!(rec.attraction_rule.is_none());
    assert_eq!(rec.encode(), kywd);
    let mut raw_notes = kywd.clone();
    raw_notes[4] = s(b"DNAM", &[1, 2, 3]);
    let rec = KywdRecord::decode(&seed_raw(&raw_notes), Ctx::default());
    assert!(rec.notes.is_none() && rec.extra.len() == 2);
    assert_eq!(rec.encode(), raw_notes);
}

/// M2.B3: `form_ids()` and `lstrings()` yield every typed FormID and lstring in definition
/// order, through struct arrays, repeated groups, shared types behind a `count_from` array
/// and version-gated fields; the `_mut` forms edit in place.
#[test]
fn visitors_yield_every_form_id_and_lstring_in_order() {
    fn id(v: u32) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }
    let weight = |w: u16| w.to_le_bytes().to_vec();
    let links = [id(0x0100_0001), weight(7), id(0x0100_0002), weight(8)].concat();
    let pairs = [
        vec![2u8],
        id(0x0000_0A01),
        vec![1],
        id(0x0000_0A02),
        vec![2],
    ]
    .concat();
    let last = [id(0x0200_0003), id(0x0200_0004), id(77)].concat();
    let subs = vec![
        sub(b"FULL", &id(11)),
        sub(b"LNKS", &links),
        sub(b"SREF", &id(0x0000_0B01)),
        sub(b"SDSC", &id(22)),
        sub(b"SPRS", &pairs),
        sub(b"SREF", &id(0x0000_0B02)),
        sub(b"LAST", &last),
    ];
    let mut rec = Record::decode(*b"VVVV", &raw(&subs), ctx(10, true)).unwrap();
    assert!(rec.extra().is_empty(), "{:?}", rec.extra());
    let ids: Vec<(&str, u32)> = rec.form_ids().into_iter().map(|(p, f)| (p, f.0)).collect();
    assert_eq!(
        ids,
        [
            ("VVVV.LNKS.Target", 0x0100_0001),
            ("VVVV.LNKS.Target", 0x0100_0002),
            ("VVVV.Slot.SREF", 0x0000_0B01),
            ("Link.Form", 0x0000_0A01),
            ("Link.Form", 0x0000_0A02),
            ("VVVV.Slot.SREF", 0x0000_0B02),
            ("VVVV.LAST.Owner", 0x0200_0003),
            ("VVVV.LAST.Extra", 0x0200_0004),
        ]
    );
    let strings: Vec<(&str, LString)> = rec
        .lstrings()
        .into_iter()
        .map(|(p, s)| (p, s.clone()))
        .collect();
    assert_eq!(
        strings,
        [
            ("VVVV.FULL", LString::Id(11)),
            ("VVVV.Slot.SDSC", LString::Id(22)),
            ("VVVV.LAST.Label", LString::Id(77)),
        ]
    );
    // A gated field that is absent is not visited.
    let mut old_subs = subs.clone();
    old_subs[6] = sub(b"LAST", &[id(0x0200_0003), id(77)].concat());
    let old = Record::decode(*b"VVVV", &raw(&old_subs), ctx(9, true)).unwrap();
    assert!(old.extra().is_empty());
    assert_eq!(old.form_ids().len(), 7);

    // Remap every master-0x01 FormID to master 0x03 and replace a string ID with text.
    for (_, form) in rec.form_ids_mut() {
        if form.0 >> 24 == 1 {
            *form = FormId(form.0 & 0x00FF_FFFF | 0x0300_0000);
        }
    }
    for (path, text) in rec.lstrings_mut() {
        if path == "VVVV.FULL" {
            *text = LString::Text(ZString(b"Named".to_vec()));
        }
    }
    let encoded = rec.encode();
    let new_links = [id(0x0300_0001), weight(7), id(0x0300_0002), weight(8)].concat();
    assert_eq!(encoded[0], sub(b"FULL", b"Named\0"));
    assert_eq!(encoded[1], sub(b"LNKS", &new_links));
    assert_eq!(encoded[2..], subs[2..]);

    // A record without FormID or lstring members gives empty lists.
    let empty = Record::decode(*b"EMPT", &[], ctx(1, true)).unwrap();
    assert!(empty.form_ids().is_empty() && empty.lstrings().is_empty());
}

/// M2.B9: `type = "union"` with `decider = "edid-prefix"`, once and repeated; FormID and
/// lstring variants are visited; without a decider the payload stays raw.
#[test]
fn unions_follow_the_editor_id_prefix() {
    use fixture::{UuuuItems, UuuuValue};
    let id = |v: u32| v.to_le_bytes().to_vec();
    let subs = vec![
        sub(b"EDID", b"rLink\0"),
        sub(b"DATA", &id(0x0100_0042)),
        sub(b"LIST", &id(7)),
        sub(b"LIST", &id(8)),
    ];
    let rec = Record::decode(*b"UUUU", &raw(&subs), ctx(1, true)).unwrap();
    assert!(rec.extra().is_empty(), "{:?}", rec.extra());
    let Record::Uuuu(u) = &rec else {
        panic!("UUUU");
    };
    assert_eq!(
        **u.value.as_ref().unwrap(),
        UuuuValue::Ref(FormId(0x0100_0042))
    );
    let items: Vec<UuuuItems> = u.items.iter().map(|s| s.value.clone()).collect();
    assert_eq!(
        items,
        [UuuuItems::Form(FormId(7)), UuuuItems::Form(FormId(8))]
    );
    let ids: Vec<(&str, u32)> = rec.form_ids().into_iter().map(|(p, f)| (p, f.0)).collect();
    assert_eq!(
        ids,
        [
            ("UUUU.DATA", 0x0100_0042),
            ("UUUU.LIST", 7),
            ("UUUU.LIST", 8)
        ]
    );
    assert_eq!(rec.encode(), subs);

    // A string variant is an lstring.
    let text = vec![sub(b"EDID", b"sName\0"), sub(b"DATA", &id(99))];
    let rec = Record::decode(*b"UUUU", &raw(&text), ctx(1, true)).unwrap();
    assert_eq!(rec.lstrings().len(), 1);
    assert!(rec.form_ids().is_empty());

    // The variant's own size rule still applies: a 1-byte `f` item, a 2-byte one fails.
    let small = vec![
        sub(b"EDID", b"fX\0"),
        sub(b"DATA", &1.5f32.to_le_bytes()),
        sub(b"LIST", &[3]),
        sub(b"LIST", &[3, 4]),
    ];
    let rec = Record::decode(*b"UUUU", &raw(&small), ctx(1, true)).unwrap();
    assert_eq!(rec.extra().len(), 1);
    assert_eq!(rec.extra()[0].0, 3);
    assert_eq!(rec.encode(), small);

    // No EDID: no layout, everything raw, nothing lost.
    let bare = vec![sub(b"DATA", &id(1)), sub(b"LIST", &id(2))];
    let rec = Record::decode(*b"UUUU", &raw(&bare), ctx(1, true)).unwrap();
    assert_eq!(rec.extra().len(), 2);
    assert_eq!(rec.encode(), bare);
    let err = fixture::decode::<UuuuValue>(&id(1), "UUUU.DATA", ctx(1, true)).unwrap_err();
    assert_eq!(err, DecodeError::Undecided { field: "UUUU.DATA" });
    assert!(err.to_string().contains("EditorID prefix"));
}

/// Lane B round 4: struct-field unions decided by an earlier sibling (`Kind`), refined by
/// a second one (`Prop`) for some values. Each array element picks its own variant; a
/// kind no variant names keeps the bytes as read (`Other`); FormID variants are visited;
/// every body re-encodes byte for byte.
#[test]
fn field_unions_follow_their_sibling() {
    use fixture::{
        UuuuPropsItem, UuuuPropsItemKind, UuuuPropsItemSecond, UuuuPropsItemValue,
        UuuuPropsItemValueLevel,
    };
    let entry = |kind: u8, prop: &[u8; 4], value: [u8; 4], second: [u8; 2]| {
        let mut out = vec![kind];
        out.extend_from_slice(prop);
        out.extend_from_slice(&value);
        out.extend_from_slice(&second);
        out
    };
    let mut payload = Vec::new();
    payload.extend(entry(0, b"ABCD", (-5i32).to_le_bytes(), 7i16.to_le_bytes()));
    payload.extend(entry(1, b"ABCD", 2.5f32.to_le_bytes(), [1, 2]));
    payload.extend(entry(6, b"ABCD", 0x0100_0042u32.to_le_bytes(), [3, 4]));
    payload.extend(entry(5, b"WSLV", 2u32.to_le_bytes(), [5, 6]));
    payload.extend(entry(5, b"WSTG", 3u32.to_le_bytes(), [7, 8]));
    payload.extend(entry(9, b"ABCD", [1, 2, 3, 4], [9, 9]));
    let subs = vec![sub(b"EDID", b"fX\0"), sub(b"PROP", &payload)];
    let rec = Record::decode(*b"UUUU", &raw(&subs), ctx(1, true)).unwrap();
    assert!(rec.extra().is_empty(), "{:?}", rec.extra());
    let Record::Uuuu(u) = &rec else {
        panic!("UUUU");
    };
    let items: &Vec<UuuuPropsItem> = &u.props.as_ref().unwrap().0;
    let values: Vec<UuuuPropsItemValue> = items.iter().map(|i| i.value.clone()).collect();
    assert_eq!(
        values,
        [
            UuuuPropsItemValue::Int(-5),
            UuuuPropsItemValue::Float(2.5),
            UuuuPropsItemValue::Ref(FormId(0x0100_0042)),
            UuuuPropsItemValue::Level(UuuuPropsItemValueLevel::Loud),
            UuuuPropsItemValue::Enum(3),
            UuuuPropsItemValue::Other([1, 2, 3, 4]),
        ]
    );
    assert_eq!(items[0].kind, UuuuPropsItemKind::Int);
    assert_eq!(items[0].second, UuuuPropsItemSecond::Int(7));
    assert_eq!(items[1].second, UuuuPropsItemSecond::Half([1, 2]));
    // Kind 6 is named by `Value` but not by `Second`; kind 9 by neither.
    assert_eq!(items[2].second, UuuuPropsItemSecond::Other([3, 4]));
    assert_eq!(items[5].kind, UuuuPropsItemKind::Other(9));
    assert_eq!(items[5].second, UuuuPropsItemSecond::Other([9, 9]));
    let ids: Vec<(&str, u32)> = rec.form_ids().into_iter().map(|(p, f)| (p, f.0)).collect();
    assert_eq!(ids, [("UUUU.PROP.Value", 0x0100_0042)]);
    assert_eq!(rec.encode(), subs);

    // Remapping through the visitor rewrites the FormID variant in place.
    let mut rec = rec;
    for (_, id) in rec.form_ids_mut() {
        id.0 = 0x0200_0042;
    }
    let encoded = rec.encode();
    assert_eq!(
        encoded[1].data.get(27..31),
        Some(&0x0200_0042u32.to_le_bytes()[..])
    );

    // A truncated entry fails the whole payload, which then stays raw.
    let short = vec![sub(b"PROP", &payload[..payload.len() - 1])];
    let rec = Record::decode(*b"UUUU", &raw(&short), ctx(1, true)).unwrap();
    assert_eq!(rec.extra().len(), 1);
    assert_eq!(rec.encode(), short);
    // Without its decider a field union keeps the bytes as read.
    let lone = fixture::decode::<UuuuPropsItemValue>(&[1, 2, 3, 4], "V", ctx(1, true)).unwrap();
    assert_eq!(lone, UuuuPropsItemValue::Other([1, 2, 3, 4]));
}
