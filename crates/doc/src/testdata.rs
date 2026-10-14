//! Synthetic plugins for tests, built from the container layout `kitcraft-esm` documents.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

pub fn sub(sig: &str, data: &[u8]) -> Vec<u8> {
    let mut out = sig.as_bytes().to_vec();
    out.extend_from_slice(&u16::try_from(data.len()).unwrap().to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn subs(fields: &[(&str, Vec<u8>)]) -> Vec<u8> {
    fields.iter().flat_map(|(s, d)| sub(s, d)).collect()
}

pub fn raw_record(sig: &[u8; 4], flags: u32, form_id: u32, body: &[u8]) -> Vec<u8> {
    let mut out = sig.to_vec();
    out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&form_id.to_le_bytes());
    out.extend_from_slice(&[0u8; 8]);
    out.extend_from_slice(body);
    out
}

/// An uncompressed record.
pub fn rec(sig: &[u8; 4], form_id: u32, fields: &[(&str, Vec<u8>)]) -> Vec<u8> {
    raw_record(sig, 0, form_id, &subs(fields))
}

/// A record with extra flags.
pub fn rec_flags(sig: &[u8; 4], flags: u32, form_id: u32, fields: &[(&str, Vec<u8>)]) -> Vec<u8> {
    raw_record(sig, flags, form_id, &subs(fields))
}

/// A compressed record.
pub fn compressed(sig: &[u8; 4], form_id: u32, fields: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let body = kitcraft_esm::compress::deflate(&subs(fields), 9, 0).unwrap();
    raw_record(sig, 0x0004_0000, form_id, &body)
}

pub fn group(label: &[u8; 4], contents: &[u8]) -> Vec<u8> {
    let mut out = b"GRUP".to_vec();
    out.extend_from_slice(&u32::try_from(contents.len() + 24).unwrap().to_le_bytes());
    out.extend_from_slice(label);
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&[0u8; 8]);
    out.extend_from_slice(contents);
    out
}

/// TES4 with HEDR and masters, then one top group per record signature (first appearance
/// order).
pub fn plugin_with(flags: u32, masters: &[&str], records: &[Vec<u8>]) -> Vec<u8> {
    let mut hedr = 0.96f32.to_le_bytes().to_vec();
    hedr.extend_from_slice(&(records.len() as u32 + 1).to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let mut tes4 = sub("HEDR", &hedr);
    for m in masters {
        let mut name = m.as_bytes().to_vec();
        name.push(0);
        tes4.extend(sub("MAST", &name));
        tes4.extend(sub("DATA", &0u64.to_le_bytes()));
    }
    let mut out = raw_record(b"TES4", flags, 0, &tes4);
    let mut order: Vec<[u8; 4]> = Vec::new();
    for r in records {
        let sig: [u8; 4] = r[..4].try_into().unwrap();
        if !order.contains(&sig) {
            order.push(sig);
        }
    }
    for sig in order {
        let body: Vec<u8> = records
            .iter()
            .filter(|r| r[..4] == sig)
            .flatten()
            .copied()
            .collect();
        out.extend(group(&sig, &body));
    }
    out
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temp directory for this test process.
pub fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("kitcraft-doc-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write `bytes` as `name` in a fresh temp dir.
pub fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    let path = temp_dir("f").join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}
