//! M3.A5 structure-aware mutator, on stable Rust (cargo-fuzz needs nightly).
//!
//! [`Mutator::new`] walks the parsed tree of a valid plugin once and records where every
//! structural field sits: record and group sizes, record flags and form versions, group
//! labels and types, subrecord sizes, the three fields of every `XXXX` prefix (its own
//! size, the `u32` it carries, the size field of the subrecord after it), the
//! decompressed-size prefix of every compressed body and the `HEDR` record count.
//! [`Mutator::mutate`] then makes one
//! input from a seeded [`Rng`]: up to three field overwrites with values chosen around the
//! field's own (0, 1, header lengths, off by one, doubled, the type's maximum, its sign
//! bit, random), and sometimes one splice that keeps every enclosing size consistent so
//! the input gets past framing: a subrecord or entry duplicated or deleted, an entry
//! wrapped in 1 to 70 extra groups, or a compressed body inflated, changed inside (a size
//! field, a duplicated or dropped subrecord, junk at the end, a truncated or extended
//! stream) and deflated again with a declared size that is right or wrong, or replaced by
//! a subrecord bomb (one more empty subrecord than `DecodeLimits` accepts).
//!
//! Field kinds are chosen first and sites second, so the rare ones (`XXXX`, compressed
//! bodies) are hit as often as record sizes.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use std::sync::OnceLock;

use kitkraft_esm::compress::{deflate, inflate, DEFAULT_MAX_SUBRECORDS};
use kitkraft_esm::{Body, DecodeLimits, Entry, Plugin, Record, Signature};

use super::hostile::Rng;

/// Record flag: compressed body (UESP, record flags).
const FLAG_COMPRESSED: u32 = 0x0004_0000;
/// Record flag: deleted (UESP, record flags).
const FLAG_DELETED: u32 = 0x0000_0020;

/// A structural field of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    RecordSize,
    RecordFlags,
    FormVersion,
    GroupSize,
    GroupLabel,
    GroupType,
    SubrecordSize,
    /// The `XXXX` subrecord's own `u16` size (4 when valid).
    OversizeOwnSize,
    /// The `u32` an `XXXX` subrecord carries.
    OversizeValue,
    /// The `u16` size field of the subrecord an `XXXX` prefix sizes (0 as written).
    OversizeNextSize,
    /// The `u32` decompressed size at the start of a compressed body.
    DecompressedSize,
    /// The record count in the `TES4` `HEDR` (bytes 4..8 of its data).
    HedrCount,
}

impl Field {
    const ALL: [Field; 12] = [
        Field::HedrCount,
        Field::RecordSize,
        Field::RecordFlags,
        Field::FormVersion,
        Field::GroupSize,
        Field::GroupLabel,
        Field::GroupType,
        Field::SubrecordSize,
        Field::OversizeOwnSize,
        Field::OversizeValue,
        Field::OversizeNextSize,
        Field::DecompressedSize,
    ];

    fn width(self) -> usize {
        match self {
            Field::FormVersion
            | Field::SubrecordSize
            | Field::OversizeOwnSize
            | Field::OversizeNextSize => 2,
            _ => 4,
        }
    }
}

/// A run of bytes whose length is counted by the size fields at `ancestors` (record data
/// sizes and group sizes, all `u32` at header + 4).
#[derive(Debug, Clone)]
struct Span {
    start: usize,
    len: usize,
    /// Header offsets of every enclosing record and group, innermost last.
    ancestors: Vec<usize>,
    /// Start of the top-level entry holding it (0 for the `TES4` record).
    top: usize,
}

/// Where the mutations of one input start and what they were.
#[derive(Debug, Clone)]
pub struct Mutation {
    pub bytes: Vec<u8>,
    /// The first offset an error may point at: the start of the first top-level entry
    /// changed.
    pub lower: u64,
    pub description: String,
    /// The field kinds and splice touched, for the statistics.
    pub kinds: Vec<&'static str>,
}

/// Sites of one valid plugin.
pub struct Mutator {
    original: Vec<u8>,
    fields: Vec<(Field, Vec<(usize, usize)>)>,
    subrecords: Vec<Span>,
    entries: Vec<Span>,
    compressed: Vec<Span>,
}

impl Mutator {
    /// Index `bytes`, which must parse.
    pub fn new(bytes: &[u8]) -> Mutator {
        let plugin = Plugin::parse(bytes).unwrap();
        let mut m = Mutator {
            original: bytes.to_vec(),
            fields: Field::ALL.iter().map(|&f| (f, Vec::new())).collect(),
            subrecords: Vec::new(),
            entries: Vec::new(),
            compressed: Vec::new(),
        };
        m.record(plugin.header(), &[], 0);
        for entry in plugin.entries() {
            let top = offset(entry);
            m.entry(entry, &[], top);
        }
        m.fields.retain(|(_, sites)| !sites.is_empty());
        m
    }

    /// How many sites of each field kind were found.
    pub fn field_counts(&self) -> Vec<(Field, usize)> {
        self.fields.iter().map(|(f, s)| (*f, s.len())).collect()
    }

    fn add(&mut self, field: Field, at: usize, top: usize) {
        let slot = self.fields.iter_mut().find(|(f, _)| *f == field).unwrap();
        slot.1.push((at, top));
    }

    fn entry(&mut self, entry: &Entry<'_>, ancestors: &[usize], top: usize) {
        match entry {
            Entry::Group(g) => {
                let at = g.offset().unwrap() as usize;
                self.add(Field::GroupSize, at + 4, top);
                self.add(Field::GroupLabel, at + 8, top);
                self.add(Field::GroupType, at + 12, top);
                self.entries.push(Span {
                    start: at,
                    len: g.size_as_read().unwrap() as usize,
                    ancestors: ancestors.to_vec(),
                    top,
                });
                let mut inner = ancestors.to_vec();
                inner.push(at);
                for child in g.children() {
                    self.entry(child, &inner, top);
                }
            }
            Entry::Record(r) => {
                self.entries.push(Span {
                    start: r.offset().unwrap() as usize,
                    len: 24 + r.data_size_as_read().unwrap() as usize,
                    ancestors: ancestors.to_vec(),
                    top,
                });
                self.record(r, ancestors, top);
            }
        }
    }

    fn record(&mut self, r: &Record<'_>, ancestors: &[usize], top: usize) {
        let at = r.offset().unwrap() as usize;
        self.add(Field::RecordSize, at + 4, top);
        self.add(Field::RecordFlags, at + 8, top);
        self.add(Field::FormVersion, at + 20, top);
        let mut inner = ancestors.to_vec();
        inner.push(at);
        match r.body() {
            Body::Compressed { .. } => {
                self.add(Field::DecompressedSize, at + 24, top);
                self.compressed.push(Span {
                    start: at,
                    len: r.data_size_as_read().unwrap() as usize,
                    ancestors: ancestors.to_vec(),
                    top,
                });
            }
            Body::Raw(subs) => {
                for s in subs {
                    if r.header.signature == Signature::TES4
                        && s.signature() == Signature::new(b"HEDR")
                        && s.data().len() >= 8
                    {
                        let data = s.data_offset().unwrap() as usize;
                        self.add(Field::HedrCount, data + 4, top);
                    }
                    let so = s.offset().unwrap() as usize;
                    if s.has_oversize_prefix() {
                        self.add(Field::OversizeOwnSize, so + 4, top);
                        self.add(Field::OversizeValue, so + 6, top);
                        self.add(Field::OversizeNextSize, so + 14, top);
                    } else {
                        self.add(Field::SubrecordSize, so + 4, top);
                    }
                    self.subrecords.push(Span {
                        start: so,
                        len: s.encoded_len() as usize,
                        ancestors: inner.clone(),
                        top,
                    });
                }
            }
        }
    }

    /// One mutated input.
    pub fn mutate(&self, rng: &mut Rng) -> Mutation {
        let mut bytes = self.original.clone();
        let mut lower = u64::MAX;
        let mut parts = Vec::new();
        let mut kinds = Vec::new();
        // 0 to 3 field overwrites (weighted to 1), then a splice with probability 2/5; at
        // least one of the two.
        let overwrites = *rng.pick(&[0usize, 1, 1, 1, 1, 2, 2, 3]);
        let splice = overwrites == 0 || rng.below(5) < 2;
        for _ in 0..overwrites {
            let (field, sites) = rng.pick(&self.fields);
            let &(at, top) = rng.pick(sites);
            let desc = overwrite(&mut bytes, *field, at, rng);
            parts.push(format!("{field:?}@{at} {desc}"));
            kinds.push(field_name(*field));
            lower = lower.min(top as u64);
        }
        // Field overwrites never move bytes, so the splice's offsets (from the original)
        // still hold. A splice kind the file has no site for is drawn again.
        if splice {
            for _ in 0..16 {
                if let Some((kind, desc, top)) = self.splice(&mut bytes, rng) {
                    parts.push(desc);
                    kinds.push(kind);
                    lower = lower.min(top as u64);
                    break;
                }
            }
        }
        if lower == u64::MAX {
            lower = 0;
        }
        Mutation {
            bytes,
            lower,
            description: parts.join("; "),
            kinds,
        }
    }

    fn splice(&self, bytes: &mut Vec<u8>, rng: &mut Rng) -> Option<(&'static str, String, usize)> {
        let choice = rng.below(6);
        match choice {
            0 | 1 if !self.subrecords.is_empty() => {
                let s = rng.pick(&self.subrecords).clone();
                if choice == 0 {
                    let copy = bytes[s.start..s.start + s.len].to_vec();
                    insert(bytes, s.start + s.len, &copy, &s.ancestors);
                    Some(("dup-subrecord", format!("dup subrecord@{}", s.start), s.top))
                } else {
                    remove(bytes, s.start, s.len, &s.ancestors);
                    Some((
                        "drop-subrecord",
                        format!("drop subrecord@{}", s.start),
                        s.top,
                    ))
                }
            }
            2 if !self.entries.is_empty() => {
                let e = rng.pick(&self.entries).clone();
                if rng.below(2) == 0 {
                    let copy = bytes[e.start..e.start + e.len].to_vec();
                    insert(bytes, e.start + e.len, &copy, &e.ancestors);
                    Some(("dup-entry", format!("dup entry@{}", e.start), e.top))
                } else {
                    remove(bytes, e.start, e.len, &e.ancestors);
                    Some(("drop-entry", format!("drop entry@{}", e.start), e.top))
                }
            }
            3 if !self.entries.is_empty() => {
                let e = rng.pick(&self.entries).clone();
                let depth = *rng.pick(&[1usize, 1, 2, 3, 6, 15, 16, 17, 63, 64, 70]);
                let label = random_label(rng);
                let group_type = *rng.pick(&[0i32, 1, 2, 4, 6, 7, 8, 9, 10, -1]);
                let mut headers = Vec::with_capacity(depth * 24);
                for level in 0..depth {
                    let size = (e.len + 24 * (depth - level)) as u32;
                    headers.extend_from_slice(b"GRUP");
                    headers.extend_from_slice(&size.to_le_bytes());
                    headers.extend_from_slice(&label);
                    headers.extend_from_slice(&group_type.to_le_bytes());
                    headers.extend_from_slice(&[0u8; 8]);
                }
                insert(bytes, e.start, &headers, &e.ancestors);
                Some((
                    "wrap-groups",
                    format!("wrap entry@{} in {depth} groups type {group_type}", e.start),
                    e.top,
                ))
            }
            4 | 5 if !self.compressed.is_empty() => {
                let c = rng.pick(&self.compressed).clone();
                self.recompress(bytes, &c, rng)
                    .map(|desc| ("recompress", desc, c.top))
            }
            _ => None,
        }
    }

    /// Inflate the compressed record `c`, change its decompressed data, deflate it again
    /// and fix every enclosing size.
    fn recompress(&self, bytes: &mut Vec<u8>, c: &Span, rng: &mut Rng) -> Option<String> {
        let body_at = c.start + 24;
        let body = &self.original[body_at..body_at + c.len];
        let mut fields = inflate(body, c.start as u64, &DecodeLimits::default()).ok()?;
        let subs = inner_subrecords(&fields);
        let mut desc = format!("recompress@{}:", c.start);
        match rng.below(6) {
            0 if !subs.is_empty() => {
                let &(at, len, xxxx) = rng.pick(&subs);
                if xxxx {
                    let v = size_value(rng, u64::from(read_u32(&fields, at + 6)), 4);
                    fields[at + 6..at + 10].copy_from_slice(&(v as u32).to_le_bytes());
                    desc.push_str(&format!(" inner XXXX@{at}={v}"));
                } else {
                    let old = u64::from(u16::from_le_bytes([fields[at + 4], fields[at + 5]]));
                    let v = size_value(rng, old, 2);
                    fields[at + 4..at + 6].copy_from_slice(&(v as u16).to_le_bytes());
                    desc.push_str(&format!(" inner size@{at} {old}->{v} (len {len})"));
                }
            }
            1 if !subs.is_empty() => {
                let &(at, len, _) = rng.pick(&subs);
                let copy = fields[at..at + len].to_vec();
                fields.splice(at + len..at + len, copy);
                desc.push_str(&format!(" inner dup@{at}"));
            }
            2 if !subs.is_empty() => {
                let &(at, len, _) = rng.pick(&subs);
                fields.drain(at..at + len);
                desc.push_str(&format!(" inner drop@{at}"));
            }
            3 => {
                let n = 1 + rng.below(6) as usize;
                for _ in 0..n {
                    fields.push(rng.next() as u8);
                }
                desc.push_str(&format!(" inner junk +{n}"));
            }
            4 if rng.below(3) == 0 => {
                // A subrecord bomb: one more empty subrecord than the decode limit allows,
                // in a stream of a few KiB (deflated once per process).
                desc.push_str(" subrecord bomb");
                return Some(self.replace_body(bytes, c, bomb().clone(), desc));
            }
            _ => desc.push_str(" unchanged data"),
        }
        let mut new_body = deflate(&fields, *rng.pick(&[0u32, 1, 6, 9]), 0).unwrap();
        match rng.below(6) {
            0 => {
                let declared = u64::from(read_u32(&new_body, 0));
                let v = size_value(rng, declared, 4);
                new_body[0..4].copy_from_slice(&(v as u32).to_le_bytes());
                desc.push_str(&format!(", declared {declared}->{v}"));
            }
            1 if new_body.len() > 4 => {
                let keep = 4 + rng.below(new_body.len() as u64 - 4) as usize;
                new_body.truncate(keep);
                desc.push_str(&format!(", stream cut to {keep}"));
            }
            2 => {
                let n = 1 + rng.below(8) as usize;
                for _ in 0..n {
                    new_body.push(rng.next() as u8);
                }
                desc.push_str(&format!(", {n} trailing bytes"));
            }
            _ => {}
        }
        Some(self.replace_body(bytes, c, new_body, desc))
    }

    /// Replace the body of compressed record `c` with `new_body`, fixing every size.
    fn replace_body(
        &self,
        bytes: &mut Vec<u8>,
        c: &Span,
        new_body: Vec<u8>,
        desc: String,
    ) -> String {
        let mut ancestors = c.ancestors.clone();
        ancestors.push(c.start);
        remove(bytes, c.start + 24, c.len, &ancestors);
        insert(bytes, c.start + 24, &new_body, &ancestors);
        desc
    }
}

/// A compressed body (declared size + stream) holding `DEFAULT_MAX_SUBRECORDS + 1` empty
/// subrecords.
fn bomb() -> &'static Vec<u8> {
    static BOMB: OnceLock<Vec<u8>> = OnceLock::new();
    BOMB.get_or_init(|| {
        let fields: Vec<u8> = b"NAME\0\0"
            .iter()
            .copied()
            .cycle()
            .take(6 * (DEFAULT_MAX_SUBRECORDS as usize + 1))
            .collect();
        deflate(&fields, 9, 0).unwrap()
    })
}

fn offset(entry: &Entry<'_>) -> usize {
    match entry {
        Entry::Group(g) => g.offset().unwrap() as usize,
        Entry::Record(r) => r.offset().unwrap() as usize,
    }
}

/// The statistics name of `field`.
pub fn field_name(field: Field) -> &'static str {
    match field {
        Field::RecordSize => "record-size",
        Field::RecordFlags => "record-flags",
        Field::FormVersion => "form-version",
        Field::GroupSize => "group-size",
        Field::GroupLabel => "group-label",
        Field::GroupType => "group-type",
        Field::SubrecordSize => "subrecord-size",
        Field::OversizeOwnSize => "xxxx-own-size",
        Field::OversizeValue => "xxxx-value",
        Field::OversizeNextSize => "xxxx-next-size",
        Field::DecompressedSize => "decompressed-size",
        Field::HedrCount => "hedr-count",
    }
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// Subrecords of a decompressed body as `(offset, encoded length, has XXXX prefix)`, as
/// far as they frame.
fn inner_subrecords(fields: &[u8]) -> Vec<(usize, usize, bool)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 6 <= fields.len() {
        let size = usize::from(u16::from_le_bytes([fields[at + 4], fields[at + 5]]));
        let (len, xxxx) = if &fields[at..at + 4] == b"XXXX" && size == 4 && at + 16 <= fields.len()
        {
            (16 + read_u32(fields, at + 6) as usize, true)
        } else {
            (6 + size, false)
        };
        if at + len > fields.len() {
            break;
        }
        out.push((at, len, xxxx));
        at += len;
    }
    out
}

/// Add `delta` (two's complement) to the `u32` size field of every header in `ancestors`.
fn adjust(bytes: &mut [u8], ancestors: &[usize], delta: i64) {
    for &h in ancestors {
        let old = read_u32(bytes, h + 4);
        let new = (i64::from(old) + delta) as u32;
        bytes[h + 4..h + 8].copy_from_slice(&new.to_le_bytes());
    }
}

/// Insert `data` at `at`, growing the sizes of `ancestors` (whose headers all start at or
/// before `at`, so their offsets do not move).
fn insert(bytes: &mut Vec<u8>, at: usize, data: &[u8], ancestors: &[usize]) {
    bytes.splice(at..at, data.iter().copied());
    adjust(bytes, ancestors, data.len() as i64);
}

fn remove(bytes: &mut Vec<u8>, at: usize, len: usize, ancestors: &[usize]) {
    bytes.drain(at..at + len);
    adjust(bytes, ancestors, -(len as i64));
}

/// A value for a size field of `width` bytes, chosen around `orig`.
fn size_value(rng: &mut Rng, orig: u64, width: usize) -> u64 {
    let max = if width == 2 {
        u64::from(u16::MAX)
    } else {
        u64::from(u32::MAX)
    };
    let v = match rng.below(12) {
        0 => 0,
        1 => *rng.pick(&[1u64, 2, 3, 4, 5, 6, 7, 10, 16, 23, 24, 25, 28]),
        2 => orig.wrapping_add(1),
        3 => orig.wrapping_sub(1),
        4 => orig.wrapping_add(rng.below(129)).wrapping_sub(64),
        5 => max,
        6 => max / 2 + 1,
        7 => orig.wrapping_mul(2),
        8 => orig / 2,
        9 => rng.next(),
        10 => rng.below(4096),
        _ => *rng.pick(&[
            0xFFFF,
            0x1_0000,
            0x1_0001,
            64 * 1024 * 1024,
            64 * 1024 * 1024 + 1,
            0x7FFF_FFFF,
            0xFFFF_FFFE,
        ]),
    };
    v & max
}

fn random_label(rng: &mut Rng) -> [u8; 4] {
    match rng.below(3) {
        0 => *rng.pick(&[*b"GRUP", *b"XXXX", *b"TES4", *b"CELL", *b"WRLD", [0u8; 4]]),
        1 => (rng.next() as u32).to_le_bytes(),
        _ => (rng.below(0x1000) as u32).to_le_bytes(),
    }
}

/// Overwrite the field at `at`; returns what was written.
fn overwrite(bytes: &mut [u8], field: Field, at: usize, rng: &mut Rng) -> String {
    let width = field.width();
    let orig = if width == 2 {
        u64::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
    } else {
        u64::from(read_u32(bytes, at))
    };
    let v = match field {
        Field::RecordFlags => {
            let o = orig as u32;
            u64::from(match rng.below(7) {
                0 | 1 => o ^ FLAG_COMPRESSED,
                2 => o ^ FLAG_DELETED,
                3 => o ^ FLAG_COMPRESSED ^ FLAG_DELETED,
                4 => o ^ (1 << rng.below(32)),
                5 => 0,
                _ => u32::MAX,
            })
        }
        Field::FormVersion => match rng.below(6) {
            0 => 0,
            1 => 1,
            2 => orig.wrapping_add(1) & 0xFFFF,
            3 => orig.wrapping_sub(1) & 0xFFFF,
            4 => 0xFFFF,
            _ => rng.next() & 0xFFFF,
        },
        Field::GroupLabel => u64::from(u32::from_le_bytes(random_label(rng))),
        Field::GroupType => {
            let random = rng.next() as i32;
            let types = [
                -2i32,
                -1,
                0,
                1,
                2,
                3,
                4,
                5,
                6,
                7,
                8,
                9,
                10,
                11,
                12,
                13,
                i32::MIN,
                i32::MAX,
                random,
            ];
            u64::from(*rng.pick(&types) as u32)
        }
        Field::OversizeOwnSize => match rng.below(3) {
            0 => *rng.pick(&[0u64, 3, 5, 8, 0xFFFF]),
            _ => size_value(rng, orig, 2),
        },
        _ => size_value(rng, orig, width),
    };
    if width == 2 {
        bytes[at..at + 2].copy_from_slice(&(v as u16).to_le_bytes());
    } else {
        bytes[at..at + 4].copy_from_slice(&(v as u32).to_le_bytes());
    }
    format!("{orig:#x}->{v:#x}")
}
