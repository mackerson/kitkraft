//! Compiles the golden codegen output under the workspace lints and exercises it: typed
//! decoding, and `encode(decode(x)) == x` for well-formed, malformed and random input.

// No crate-level lint allows here: the included modules must pass the workspace lints as
// they are. Only dead code is allowed, since tests use a subset of the generated API.

#[allow(dead_code)]
mod fixture {
    include!("golden/fixture.rs");
}

#[allow(dead_code)]
mod seed {
    include!("golden/seed.rs");
}

use fixture::{Ctx, FormId, OwnedSub, RawSub, ZString, ZzzzDataKind, ZzzzDataOptions, ZzzzRecord};

fn sub(signature: &[u8; 4], data: &[u8]) -> OwnedSub {
    OwnedSub {
        signature: *signature,
        data: data.to_vec(),
    }
}

fn raw(subs: &[OwnedSub]) -> Vec<RawSub<'_>> {
    subs.iter()
        .map(|s| RawSub {
            signature: s.signature,
            data: &s.data,
        })
        .collect()
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

#[test]
fn fixture_decodes_every_type() {
    let rec = roundtrip(&well_formed(), Ctx { form_version: 100 });
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(rec.editor_id, Some(ZString(b"Thing".to_vec())));
    assert_eq!(rec.name.as_ref().unwrap().string_id(), Some(1));
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
    assert_eq!(data.pair, vec![1, 2]);
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
    assert_eq!(rec.big_value, Some(-9));
    assert_eq!(rec.skipped.len(), 2);
}

#[test]
fn unknown_enum_values_are_preserved() {
    let mut subs = well_formed();
    subs[2] = sub(b"DATA", &data_payload(77));
    let rec = roundtrip(&subs, Ctx { form_version: 100 });
    assert_eq!(rec.data.unwrap().kind, ZzzzDataKind::Other(77));
}

#[test]
fn version_gated_fields() {
    let mut subs = well_formed();
    subs[3] = sub(b"VERS", &3u32.to_le_bytes());
    let old = roundtrip(&subs, Ctx { form_version: 99 });
    assert_eq!(old.versioned.unwrap().added, None);
    // The same 4-byte payload is too short for version 100: kept raw, still roundtrips.
    let new = roundtrip(&subs, Ctx { form_version: 100 });
    assert!(new.versioned.is_none());
    assert_eq!(new.extra.len(), 1);
    assert_eq!(new.extra[0].0, 3);
}

#[test]
fn malformed_input_is_kept_raw_and_roundtrips() {
    let ctx = Ctx { form_version: 100 };
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
        roundtrip(subs, ctx);
    }
}

/// Deterministic pseudo-random bodies from a pool of known and unknown signatures with
/// random payload sizes: whatever the order, decoding then encoding is the identity.
#[test]
fn random_bodies_roundtrip() {
    let pool: [&[u8; 4]; 14] = [
        b"EDID", b"FULL", b"DATA", b"VERS", b"KWDA", b"ENAM", b"ENTR", b"BEGN", b"BEND", b"TAIL",
        b"BIG8", b"SKIP", b"QQQQ", b"ZZZZ",
    ];
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    for _ in 0..2000 {
        let len = next() % 12;
        let mut subs = Vec::new();
        for _ in 0..len {
            let sig = pool[(next() % pool.len() as u64) as usize];
            let size = match next() % 4 {
                0 => 0,
                1 => 4,
                2 => 39,
                _ => (next() % 20) as usize,
            };
            let data: Vec<u8> = (0..size).map(|_| next() as u8).collect();
            subs.push(sub(sig, &data));
        }
        let version = (next() % 200) as u16;
        roundtrip(
            &subs,
            Ctx {
                form_version: version,
            },
        );
    }
}

#[test]
fn seed_tes4_and_grup() {
    use seed::{Ctx, FormId, GrupHeader, GrupHeaderGroupType, OwnedSub, RawSub, Tes4Record};
    let s = |sig: &[u8; 4], data: &[u8]| OwnedSub {
        signature: *sig,
        data: data.to_vec(),
    };
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
    let raws: Vec<RawSub<'_>> = subs
        .iter()
        .map(|x| RawSub {
            signature: x.signature,
            data: &x.data,
        })
        .collect();
    let rec = Tes4Record::decode(&raws, Ctx::default());
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(rec.header.as_ref().unwrap().record_count, 10);
    assert_eq!(rec.master.len(), 2);
    assert_eq!(
        rec.master[0].file_name.as_ref().unwrap().0,
        b"Starfield.esm"
    );
    assert_eq!(rec.master[0].file_size, None);
    assert_eq!(rec.master[1].file_size, Some(0));
    assert_eq!(
        rec.overridden_forms.as_ref().unwrap().0,
        vec![FormId(1), FormId(2)]
    );
    assert_eq!(rec.tnam.len(), 2);
    assert_eq!(rec.incc, Some(5));
    assert_eq!(rec.encode(), subs);
    assert_eq!(Tes4Record::FLAG_LOCALIZED, 0x80);

    let mut grup = b"GRUP".to_vec();
    grup.extend_from_slice(&100u32.to_le_bytes());
    grup.extend_from_slice(b"WEAP");
    grup.extend_from_slice(&0i32.to_le_bytes());
    grup.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let header = GrupHeader::decode(&grup).unwrap();
    assert_eq!(header.group_type, GrupHeaderGroupType::Top);
    assert_eq!(&header.label, b"WEAP");
    assert_eq!(seed::encode(&header), grup);
    assert!(GrupHeader::decode(&grup[..23]).is_err());
}

#[test]
fn seed_weap_inventory_shape() {
    use seed::{Ctx, OwnedSub, RawSub, WeapRecord};
    let sigs: [&[u8; 4]; 22] = [
        b"EDID", b"OBND", b"ODTY", b"BFCB", b"REFL", b"BFCE", b"FULL", b"FLLD", b"KSIZ", b"KWDA",
        b"DESC", b"OBTE", b"OBTF", b"FULL", b"OBTS", b"FULL", b"OBTS", b"STOP", b"NNAM", b"WFIR",
        b"FLLD", b"WTRM",
    ];
    let subs: Vec<OwnedSub> = sigs
        .iter()
        .enumerate()
        .map(|(i, sig)| OwnedSub {
            signature: **sig,
            data: vec![i as u8; i % 5],
        })
        .collect();
    let raws: Vec<RawSub<'_>> = subs
        .iter()
        .map(|x| RawSub {
            signature: x.signature,
            data: &x.data,
        })
        .collect();
    let rec = WeapRecord::decode(&raws, Ctx::default());
    assert!(rec.extra.is_empty(), "{:?}", rec.extra);
    assert_eq!(rec.bfcb_block.len(), 1);
    assert_eq!(rec.bfcb_block[0].bfcb_body.len(), 1);
    assert!(rec.full.is_some());
    assert_eq!(rec.obts_entry.len(), 2);
    assert!(rec.obts_entry[0].obtf.is_some());
    assert!(rec.obts_entry[1].obtf.is_none());
    assert!(rec.flld.is_some() && rec.flld2.is_some());
    assert_eq!(rec.encode(), subs);
}
