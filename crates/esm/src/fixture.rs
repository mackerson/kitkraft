//! Synthetic plugin bytes for tests, built by hand from the documented layout.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

/// A subrecord with a `u16` size.
pub fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(sig);
    out.extend_from_slice(&u16::try_from(data.len()).unwrap().to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// A subrecord whose size is carried by an `XXXX` prefix (its own size field is 0).
pub fn xxxx_sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"XXXX");
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    out.extend_from_slice(sig);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// A record with fixed, recognisable header fields: timestamp 0x1234, version control
/// 0x5678, form version 581, unknown 7.
pub fn record(sig: &[u8; 4], flags: u32, form_id: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(sig);
    out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&form_id.to_le_bytes());
    out.extend_from_slice(&0x1234u16.to_le_bytes());
    out.extend_from_slice(&0x5678u16.to_le_bytes());
    out.extend_from_slice(&581u16.to_le_bytes());
    out.extend_from_slice(&7u16.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A group with timestamp 0x2A39, version control 0x0102 and unknown 0xCCCCCCCC.
pub fn group(label: &[u8; 4], group_type: i32, contents: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"GRUP");
    out.extend_from_slice(&u32::try_from(contents.len() + 24).unwrap().to_le_bytes());
    out.extend_from_slice(label);
    out.extend_from_slice(&group_type.to_le_bytes());
    out.extend_from_slice(&0x2A39u16.to_le_bytes());
    out.extend_from_slice(&0x0102u16.to_le_bytes());
    out.extend_from_slice(&0xCCCC_CCCCu32.to_le_bytes());
    out.extend_from_slice(contents);
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// A zlib stream holding `data` in stored (uncompressed) deflate blocks, built by hand so
/// the parser can be tested independently of any compressor.
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut chunks = data.chunks(0xFFFF).peekable();
    if chunks.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(chunk) = chunks.next() {
        let last = chunks.peek().is_none();
        out.push(u8::from(last));
        let len = u16::try_from(chunk.len()).unwrap();
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// A compressed record: flag 0x00040000, body = `u32` decompressed size + zlib stream.
pub fn compressed_record_from_stream(
    sig: &[u8; 4],
    form_id: u32,
    decompressed_len: u32,
    stream: &[u8],
) -> Vec<u8> {
    let mut body = decompressed_len.to_le_bytes().to_vec();
    body.extend_from_slice(stream);
    record(sig, 0x0004_0000, form_id, &body)
}

/// A compressed record whose zlib stream uses stored blocks.
pub fn compressed_record_stored(sig: &[u8; 4], form_id: u32, fields: &[u8]) -> Vec<u8> {
    let len = u32::try_from(fields.len()).unwrap();
    compressed_record_from_stream(sig, form_id, len, &zlib_stored(fields))
}

/// The TES4 header used by [`plugin`]: ESM + localized flags, HEDR, CNAM, two masters
/// with DATA, and ONAM.
pub fn tes4() -> Vec<u8> {
    let mut hedr = Vec::new();
    hedr.extend_from_slice(&0.96f32.to_le_bytes());
    hedr.extend_from_slice(&42u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let mut onam = Vec::new();
    onam.extend_from_slice(&0x0000_0A01u32.to_le_bytes());
    onam.extend_from_slice(&0x0100_0B02u32.to_le_bytes());
    let mut body = sub(b"HEDR", &hedr);
    body.extend(sub(b"CNAM", b"kitkraft\0"));
    body.extend(sub(b"MAST", b"Starfield.esm\0"));
    body.extend(sub(b"DATA", &0u64.to_le_bytes()));
    body.extend(sub(b"MAST", b"Other.esm\0"));
    body.extend(sub(b"DATA", &0u64.to_le_bytes()));
    body.extend(sub(b"ONAM", &onam));
    record(b"TES4", 0x81, 0, &body)
}

/// A plugin: [`tes4`] followed by the given entries.
pub fn plugin(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut out = tes4();
    for entry in entries {
        out.extend_from_slice(entry);
    }
    out
}

/// A small but structurally complete plugin: a GMST top group, a CELL top group with
/// cell children (persistent and temporary), and a NAVM top group holding a record with an
/// `XXXX`-sized subrecord.
pub fn sample_plugin() -> Vec<u8> {
    let mut gmst = record(
        b"GMST",
        0,
        0x100,
        &[sub(b"EDID", b"fFoo\0"), sub(b"DATA", &1.5f32.to_le_bytes())].concat(),
    );
    gmst.extend(record(
        b"GMST",
        0,
        0x101,
        &[sub(b"EDID", b"iBar\0"), sub(b"DATA", &7u32.to_le_bytes())].concat(),
    ));

    let refr = |id: u32| {
        record(
            b"REFR",
            0x400,
            id,
            &[
                sub(b"NAME", &0x300u32.to_le_bytes()),
                sub(b"DATA", &[0u8; 24]),
            ]
            .concat(),
        )
    };
    let persistent = group(&0x200u32.to_le_bytes(), 8, &refr(0x201));
    let temporary = group(
        &0x200u32.to_le_bytes(),
        9,
        &[refr(0x202), refr(0x203)].concat(),
    );
    let children = group(
        &0x200u32.to_le_bytes(),
        6,
        &[persistent, temporary].concat(),
    );
    let cell = record(b"CELL", 0, 0x200, &sub(b"EDID", b"TestCell\0"));
    let cells = group(b"CELL", 0, &[cell, children].concat());

    let big: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
    let navm = record(
        b"NAVM",
        0,
        0x400,
        &[xxxx_sub(b"NVNM", &big), sub(b"EDID", b"n\0")].concat(),
    );
    let navms = group(b"NAVM", 0, &navm);

    plugin(&[group(b"GMST", 0, &gmst), cells, navms])
}
