//! Shared helpers for lane H's evidence tasks (`cargo xtask observe`, `cargo xtask oracle`):
//! SHA-256, positional byte-range diffs, a `TES4` header summary, and running `kit`.
//!
//! The `TES4` summary reads only the shared container (24-byte record header, 6-byte
//! subrecord headers, the `XXXX` size prefix; `uesp-skyrim:` level 1, corpus-confirmed in
//! `crates/esm`) and the `HEDR` triple (`f32` version, `u32` record count, `u32` next object
//! ID; `docs/corpus.md` "Writing rules"). Everything else is shown as bytes. Every value
//! read from a file is bounds-checked: these files come from the CK, from Spriggit, or from
//! anywhere else, and are treated as hostile like any other plugin.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::{json, Value};

/// Environment variable naming a prebuilt `kit` binary (otherwise `cargo run --release`).
pub(crate) const KIT_VAR: &str = "KITCRAFT_KIT";

// ---------------------------------------------------------------------------------------
// SHA-256 (FIPS 180-4), no dependency.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

fn word(w: &[u32; 64], i: usize) -> u32 {
    w.get(i).copied().unwrap_or(0)
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (slot, bytes) in w.iter_mut().zip(block.as_chunks::<4>().0) {
        *slot = u32::from_be_bytes(*bytes);
    }
    for i in 16..64usize {
        let (w15, w2) = (word(&w, i.wrapping_sub(15)), word(&w, i.wrapping_sub(2)));
        let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ (w15 >> 3);
        let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ (w2 >> 10);
        let v = word(&w, i.wrapping_sub(16))
            .wrapping_add(s0)
            .wrapping_add(word(&w, i.wrapping_sub(7)))
            .wrapping_add(s1);
        if let Some(slot) = w.get_mut(i) {
            *slot = v;
        }
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for (k, wi) in K.iter().zip(w.iter()) {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(*k)
            .wrapping_add(*wi);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

/// Lower-case hex SHA-256 of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut state = H0;
    let (blocks, rest) = bytes.as_chunks::<64>();
    for block in blocks {
        compress(&mut state, block);
    }
    let mut tail = rest.to_vec();
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    let bits = (bytes.len() as u64).wrapping_mul(8);
    tail.extend_from_slice(&bits.to_be_bytes());
    for block in tail.as_chunks::<64>().0 {
        compress(&mut state, block);
    }
    state.iter().map(|w| format!("{w:08x}")).collect()
}

// ---------------------------------------------------------------------------------------
// Positional byte diff.

/// Positional comparison of two byte strings: the runs of differing bytes over the common
/// length, and how much longer one is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ByteDiff {
    pub size_a: u64,
    pub size_b: u64,
    pub identical: bool,
    pub first_difference: Option<u64>,
    /// Differing bytes over the common length (the size difference is not counted).
    pub differing_bytes: u64,
    /// Runs of differing bytes as `(offset, length)`, the first `cap` of them.
    pub ranges: Vec<(u64, u64)>,
    /// Every run, including those past `cap`.
    pub ranges_total: u64,
}

/// Compares `a` and `b` byte by byte; lists at most `cap` differing runs.
pub(crate) fn byte_diff(a: &[u8], b: &[u8], cap: usize) -> ByteDiff {
    let mut ranges = Vec::new();
    let mut total = 0u64;
    let mut differing = 0u64;
    let mut open: Option<(u64, u64)> = None;
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        let i = i as u64;
        if x == y {
            if let Some(run) = open.take() {
                total = total.saturating_add(1);
                if ranges.len() < cap {
                    ranges.push(run);
                }
            }
            continue;
        }
        differing = differing.saturating_add(1);
        open = Some(match open {
            Some((start, len)) => (start, len.saturating_add(1)),
            None => (i, 1),
        });
    }
    if let Some(run) = open {
        total = total.saturating_add(1);
        if ranges.len() < cap {
            ranges.push(run);
        }
    }
    let common = a.len().min(b.len()) as u64;
    let first_difference = ranges
        .first()
        .map(|r| r.0)
        .or((a.len() != b.len()).then_some(common));
    ByteDiff {
        size_a: a.len() as u64,
        size_b: b.len() as u64,
        identical: a == b,
        first_difference,
        differing_bytes: differing,
        ranges,
        ranges_total: total,
    }
}

impl ByteDiff {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "size_a": self.size_a,
            "size_b": self.size_b,
            "identical": self.identical,
            "first_difference": self.first_difference,
            "differing_bytes": self.differing_bytes,
            "ranges_total": self.ranges_total,
            "ranges": self.ranges.iter().map(|(o, l)| json!({"offset": o, "length": l})).collect::<Vec<_>>(),
        })
    }
}

// ---------------------------------------------------------------------------------------
// TES4 summary.

const RECORD_HEADER: usize = 24;
const SUB_HEADER: usize = 6;
/// Payload bytes shown per subrecord.
const HEX_CAP: usize = 64;

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let b = bytes.get(at..at.checked_add(2)?)?;
    let mut a = [0u8; 2];
    a.copy_from_slice(b);
    Some(u16::from_le_bytes(a))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    let mut a = [0u8; 4];
    a.copy_from_slice(b);
    Some(u32::from_le_bytes(a))
}

/// Upper-case hex of `bytes`.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// A NUL-terminated printable ASCII payload as text.
fn zstring(payload: &[u8]) -> Option<String> {
    let (last, body) = payload.split_last()?;
    if *last != 0 || body.iter().any(|b| !(0x20..0x7f).contains(b)) {
        return None;
    }
    Some(body.iter().map(|b| char::from(*b)).collect())
}

/// One subrecord of `TES4`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tes4Sub {
    pub signature: String,
    pub size: u64,
    /// Whether the size came from an `XXXX` prefix.
    pub oversized: bool,
    pub hex: String,
    pub truncated: bool,
    pub text: Option<String>,
}

/// The plugin header as stored.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tes4 {
    pub flags: u32,
    pub form_id: u32,
    pub timestamp: u16,
    pub version_control: u16,
    pub form_version: u16,
    pub unknown: u16,
    pub data_size: u32,
    pub subrecords: Vec<Tes4Sub>,
    /// `HEDR`: version, record count, next object ID.
    pub hedr: Option<(f32, u32, u32)>,
    pub masters: Vec<String>,
    /// Why the subrecord walk stopped early, if it did.
    pub problem: Option<String>,
}

/// Reads the `TES4` record at the start of `bytes`.
pub(crate) fn tes4(bytes: &[u8]) -> Result<Tes4, String> {
    if bytes.get(0..4) != Some(b"TES4".as_slice()) {
        return Err("the file does not start with a TES4 record".to_string());
    }
    let field = |at: usize| u32_at(bytes, at).ok_or("the TES4 header is truncated");
    let half = |at: usize| u16_at(bytes, at).ok_or("the TES4 header is truncated");
    let data_size = field(4)?;
    let flags = field(8)?;
    let form_id = field(12)?;
    let (timestamp, version_control, form_version, unknown) =
        (half(16)?, half(18)?, half(20)?, half(22)?);
    let mut out = Tes4 {
        flags,
        form_id,
        timestamp,
        version_control,
        form_version,
        unknown,
        data_size,
        subrecords: Vec::new(),
        hedr: None,
        masters: Vec::new(),
        problem: None,
    };
    if flags & 0x0004_0000 != 0 {
        out.problem = Some("TES4 has the compressed flag; subrecords not read".to_string());
        return Ok(out);
    }
    let end = usize::try_from(data_size)
        .ok()
        .and_then(|n| RECORD_HEADER.checked_add(n));
    let Some(body) = end.and_then(|end| bytes.get(RECORD_HEADER..end)) else {
        out.problem = Some(format!(
            "TES4 data size {data_size} runs past the end of the file ({} bytes)",
            bytes.len()
        ));
        return Ok(out);
    };
    let mut at = 0usize;
    let mut oversize: Option<u32> = None;
    while at < body.len() {
        let (Some(sig), Some(small)) = (
            at.checked_add(4).and_then(|e| body.get(at..e)),
            u16_at(body, at.saturating_add(4)),
        ) else {
            out.problem = Some(format!(
                "subrecord header truncated at TES4 body offset {at}"
            ));
            break;
        };
        let signature: String = sig.iter().map(|b| char::from(*b)).collect();
        let start = at.saturating_add(SUB_HEADER);
        let prefixed = oversize.take();
        let size = match prefixed {
            Some(big) => usize::try_from(big).unwrap_or(usize::MAX),
            None => usize::from(small),
        };
        let Some(payload) = start.checked_add(size).and_then(|e| body.get(start..e)) else {
            out.problem = Some(format!(
                "subrecord {signature} at TES4 body offset {at} runs past the record"
            ));
            break;
        };
        at = start.saturating_add(size);
        if signature == "XXXX" {
            oversize = u32_at(payload, 0);
            continue;
        }
        if signature == "HEDR" && payload.len() == 12 {
            if let (Some(v), Some(n), Some(next)) =
                (u32_at(payload, 0), u32_at(payload, 4), u32_at(payload, 8))
            {
                out.hedr = Some((f32::from_bits(v), n, next));
            }
        }
        let text = zstring(payload);
        if signature == "MAST" {
            out.masters
                .push(text.clone().unwrap_or_else(|| hex(payload)));
        }
        out.subrecords.push(Tes4Sub {
            signature,
            size: size as u64,
            oversized: prefixed.is_some(),
            hex: hex(payload.get(..HEX_CAP.min(payload.len())).unwrap_or(payload)),
            truncated: payload.len() > HEX_CAP,
            text,
        });
    }
    Ok(out)
}

impl Tes4 {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "flags": format!("0x{:08X}", self.flags),
            "form_id": format!("{:08X}", self.form_id),
            "timestamp": self.timestamp,
            "version_control": self.version_control,
            "form_version": self.form_version,
            "unknown": self.unknown,
            "data_size": self.data_size,
            "hedr": self.hedr.map(|(v, n, next)| json!({
                "version": format!("{v}"),
                "records": n,
                "next_object_id": format!("0x{next:08X}"),
            })),
            "masters": self.masters,
            "subrecords": self.subrecords.iter().map(|s| json!({
                "signature": s.signature,
                "size": s.size,
                "oversized": s.oversized,
                "hex": s.hex,
                "truncated": s.truncated,
                "text": s.text,
            })).collect::<Vec<_>>(),
            "problem": self.problem,
        })
    }
}

/// Differences between two `TES4` records, one line each: header fields, then subrecords
/// matched by signature and occurrence.
pub(crate) fn tes4_diff(a: &Tes4, b: &Tes4) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = |name: &str, x: String, y: String| {
        if x != y {
            out.push(format!("{name}: {x} -> {y}"));
        }
    };
    field(
        "flags",
        format!("0x{:08X}", a.flags),
        format!("0x{:08X}", b.flags),
    );
    field(
        "form version",
        a.form_version.to_string(),
        b.form_version.to_string(),
    );
    field(
        "timestamp",
        a.timestamp.to_string(),
        b.timestamp.to_string(),
    );
    field(
        "version control",
        a.version_control.to_string(),
        b.version_control.to_string(),
    );
    field("unknown", a.unknown.to_string(), b.unknown.to_string());
    let keyed = |t: &Tes4| {
        let mut seen: std::collections::BTreeMap<String, u32> = Default::default();
        t.subrecords
            .iter()
            .map(|s| {
                let n = seen.entry(s.signature.clone()).or_insert(0);
                let key = format!("{}[{}]", s.signature, n);
                *n = n.saturating_add(1);
                (key, s.clone())
            })
            .collect::<Vec<_>>()
    };
    let (ka, kb) = (keyed(a), keyed(b));
    let show = |s: &Tes4Sub| match &s.text {
        Some(t) => format!("{} bytes \"{t}\"", s.size),
        None => format!(
            "{} bytes {}{}",
            s.size,
            s.hex,
            if s.truncated { "..." } else { "" }
        ),
    };
    for (key, sa) in &ka {
        match kb.iter().find(|(k, _)| k == key) {
            None => out.push(format!("{key}: removed ({})", show(sa))),
            Some((_, sb)) if sb != sa => {
                out.push(format!("{key}: {} -> {}", show(sa), show(sb)));
            }
            Some(_) => {}
        }
    }
    for (key, sb) in &kb {
        if !ka.iter().any(|(k, _)| k == key) {
            out.push(format!("{key}: added ({})", show(sb)));
        }
    }
    let order = |k: &[(String, Tes4Sub)]| k.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
    let (oa, ob) = (order(&ka), order(&kb));
    let mut sa = oa.clone();
    let mut sb = ob.clone();
    sa.sort();
    sb.sort();
    if sa == sb && oa != ob {
        out.push(format!(
            "subrecord order: {} -> {}",
            oa.join(" "),
            ob.join(" ")
        ));
    }
    out
}

// ---------------------------------------------------------------------------------------
// Running kit.

/// The result of one `kit ... --json` run.
#[derive(Debug, Clone)]
pub(crate) struct KitOut {
    /// Process exit code (0 ok, 1 check failed, 2 error), if it exited normally.
    pub code: Option<i32>,
    /// The JSON envelope, or `{"error": {"message": ...}}` when there was none.
    pub json: Value,
}

impl KitOut {
    /// `data` of the envelope, or `Null`.
    pub(crate) fn data(&self) -> &Value {
        self.json.get("data").unwrap_or(&Value::Null)
    }

    /// The error message when the command did not produce a result.
    pub(crate) fn error(&self) -> Option<String> {
        self.json.get("error").map(|e| {
            e.get("message")
                .and_then(Value::as_str)
                .map_or_else(|| e.to_string(), str::to_string)
        })
    }
}

/// Something that runs `kit` with arguments and returns its JSON envelope. Tests use a fake.
pub(crate) trait Runner {
    /// Runs `kit <args> --json` with `KITCRAFT_CACHE_DIR` set to `cache`.
    fn kit(&self, args: &[String], cache: &Path) -> Result<KitOut>;
}

/// A real `kit`: the binary named by [`KIT_VAR`], or `cargo run --release -p kit-cli`.
pub(crate) struct KitBinary {
    program: PathBuf,
    prefix: Vec<String>,
    dir: PathBuf,
}

impl KitBinary {
    /// The binary at `path`.
    pub(crate) fn at(path: PathBuf) -> Self {
        let dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        Self {
            program: path,
            prefix: Vec::new(),
            dir,
        }
    }

    /// [`KIT_VAR`] if set, else `cargo run --quiet --release -p kit-cli --` in `root`.
    pub(crate) fn from_env(root: &Path) -> Self {
        if let Some(path) = std::env::var_os(KIT_VAR) {
            return Self::at(PathBuf::from(path));
        }
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        Self {
            program: PathBuf::from(cargo),
            prefix: ["run", "--quiet", "--release", "-p", "kit-cli", "--"]
                .map(str::to_string)
                .to_vec(),
            dir: root.to_path_buf(),
        }
    }
}

impl Runner for KitBinary {
    fn kit(&self, args: &[String], cache: &Path) -> Result<KitOut> {
        let output = Command::new(&self.program)
            .args(&self.prefix)
            .args(args)
            .arg("--json")
            .env("KITCRAFT_CACHE_DIR", cache)
            .current_dir(&self.dir)
            .output()
            .with_context(|| format!("could not start `{}`", self.program.display()))?;
        let json = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            json!({"error": {"message": format!(
                "kit printed no JSON (exit {:?}): {}{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            )}})
        });
        Ok(KitOut {
            code: output.status.code(),
            json,
        })
    }
}

/// Removes values that change between identical runs (timings) so reports are idempotent.
pub(crate) fn strip_volatile(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|k, _| k != "elapsed_ms" && k != "elapsed");
            for v in map.values_mut() {
                strip_volatile(v);
            }
        }
        Value::Array(items) => {
            for v in items {
                strip_volatile(v);
            }
        }
        _ => {}
    }
}

/// `path` as a UTF-8 string for a `kit` argument.
pub(crate) fn arg(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("path is not UTF-8: {}", path.display()))
}

/// A Markdown table.
pub(crate) fn table(head: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = format!("| {} |\n", head.join(" | "));
    out.push_str(&format!(
        "| {} |\n",
        head.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    ));
    for row in rows {
        let cells: Vec<String> = row.iter().map(|c| c.replace('|', "\\|")).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn sha256_standard_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million = vec![b'a'; 1_000_000];
        assert_eq!(
            sha256_hex(&million),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn byte_diff_runs_and_tail() {
        let a = b"abcdefgh";
        let b = b"abXdeYZhij";
        let d = byte_diff(a, b, 8);
        assert!(!d.identical);
        assert_eq!(d.ranges, vec![(2, 1), (5, 2)]);
        assert_eq!(d.ranges_total, 2);
        assert_eq!(d.differing_bytes, 3);
        assert_eq!(d.first_difference, Some(2));
        assert_eq!((d.size_a, d.size_b), (8, 10));

        let same = byte_diff(a, a, 8);
        assert!(same.identical);
        assert_eq!(same.first_difference, None);

        let longer = byte_diff(b"ab", b"abc", 8);
        assert_eq!(longer.first_difference, Some(2));
        assert!(longer.ranges.is_empty());

        let capped = byte_diff(b"a.a.a.a.", b"b.b.b.b.", 2);
        assert_eq!(capped.ranges.len(), 2);
        assert_eq!(capped.ranges_total, 4);
    }

    /// A subrecord.
    pub(crate) fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&(data.len() as u16).to_le_bytes());
        out.extend_from_slice(data);
        out
    }

    /// A `TES4` record with the given flags and subrecords (form version 582).
    pub(crate) fn tes4_record(flags: u32, body: &[u8]) -> Vec<u8> {
        let mut out = b"TES4".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&582u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    fn hedr(count: u32) -> Vec<u8> {
        let mut h = 0.96f32.to_le_bytes().to_vec();
        h.extend_from_slice(&count.to_le_bytes());
        h.extend_from_slice(&0xFF00_1000u32.to_le_bytes());
        sub(b"HEDR", &h)
    }

    #[test]
    fn tes4_summary_and_diff() {
        let mut body = hedr(1);
        body.extend(sub(b"CNAM", b"DEFAULT\0"));
        body.extend(sub(b"MAST", b"Starfield.esm\0"));
        body.extend(sub(b"DATA", &[0; 8]));
        let a = tes4(&tes4_record(0, &body)).unwrap();
        assert_eq!(a.hedr, Some((0.96, 1, 0xFF00_1000)));
        assert_eq!(a.masters, vec!["Starfield.esm".to_string()]);
        assert_eq!(a.subrecords.len(), 4);
        assert_eq!(a.subrecords[1].text.as_deref(), Some("DEFAULT"));
        assert!(a.problem.is_none());
        assert_eq!(a.to_json()["hedr"]["version"], "0.96");

        let mut body2 = hedr(2);
        body2.extend(sub(b"CNAM", b"Someone\0"));
        body2.extend(sub(b"MAST", b"Starfield.esm\0"));
        body2.extend(sub(b"DATA", &[0; 8]));
        body2.extend(sub(b"SNAM", b"Script\0"));
        let b = tes4(&tes4_record(1, &body2)).unwrap();
        let d = tes4_diff(&a, &b);
        assert!(d.contains(&"flags: 0x00000000 -> 0x00000001".to_string()));
        assert!(d.iter().any(|l| l.starts_with("HEDR[0]:")));
        assert!(d.contains(&"CNAM[0]: 8 bytes \"DEFAULT\" -> 8 bytes \"Someone\"".to_string()));
        assert!(d.contains(&"SNAM[0]: added (7 bytes \"Script\")".to_string()));
        assert!(tes4_diff(&a, &a).is_empty());
    }

    #[test]
    fn tes4_hostile_inputs_do_not_panic() {
        assert!(tes4(b"").is_err());
        assert!(tes4(b"TES4").is_err());
        assert!(tes4(b"GRUP\0\0\0\0").is_err());
        // Size past the end.
        let mut r = tes4_record(0, &hedr(1));
        r[4] = 0xFF;
        assert!(tes4(&r).unwrap().problem.is_some());
        // A subrecord whose size runs past the record.
        let mut bad = sub(b"CNAM", b"DEFAULT\0");
        bad[4] = 200;
        assert!(tes4(&tes4_record(0, &bad)).unwrap().problem.is_some());
        // An XXXX prefix with a huge size.
        let mut x = sub(b"XXXX", &u32::MAX.to_le_bytes());
        x.extend(sub(b"DATA", &[]));
        assert!(tes4(&tes4_record(0, &x)).unwrap().problem.is_some());
        // Every truncation of a valid header.
        let mut body = hedr(1);
        body.extend(sub(b"MAST", b"Starfield.esm\0"));
        let full = tes4_record(0, &body);
        for n in 0..full.len() {
            let _ = tes4(&full[..n]);
        }
    }

    #[test]
    fn strip_volatile_removes_timings() {
        let mut v = json!({"data": {"elapsed_ms": 3, "x": [{"elapsed_ms": 1, "y": 2}]}});
        strip_volatile(&mut v);
        assert_eq!(v, json!({"data": {"x": [{"y": 2}]}}));
    }
}
