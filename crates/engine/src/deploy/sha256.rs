//! SHA-256 (FIPS 180-4), streaming, for the deployment manifest's file hashes. Written
//! here to keep the dependency list short (the workspace has no hashing crate); checked
//! against the FIPS 180-4 example vectors in the tests below.

use std::io::Read;
use std::path::Path;

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

const H0: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const BLOCK: usize = 64;

/// Streaming SHA-256 state.
#[derive(Debug, Clone)]
pub(crate) struct Sha256 {
    state: [u32; 8],
    buf: [u8; BLOCK],
    len: usize,
    total: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self {
            state: H0,
            buf: [0; BLOCK],
            len: 0,
            total: 0,
        }
    }
}

fn word(w: &[u32; 64], i: usize) -> u32 {
    w.get(i).copied().unwrap_or(0)
}

fn compress(state: &mut [u32; 8], block: &[u8; BLOCK]) {
    let mut w = [0u32; 64];
    for (slot, bytes) in w.iter_mut().zip(block.as_chunks::<4>().0) {
        *slot = u32::from_be_bytes(*bytes);
    }
    for i in 16usize..64 {
        let w15 = word(&w, i.wrapping_sub(15));
        let w2 = word(&w, i.wrapping_sub(2));
        let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ w15.wrapping_shr(3);
        let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ w2.wrapping_shr(10);
        let value = word(&w, i.wrapping_sub(16))
            .wrapping_add(s0)
            .wrapping_add(word(&w, i.wrapping_sub(7)))
            .wrapping_add(s1);
        if let Some(slot) = w.get_mut(i) {
            *slot = value;
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

impl Sha256 {
    /// Feeds bytes.
    pub(crate) fn update(&mut self, data: &[u8]) {
        self.total = self
            .total
            .wrapping_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
        let mut data = data;
        if self.len > 0 {
            let take = BLOCK.saturating_sub(self.len).min(data.len());
            let (head, rest) = data.split_at_checked(take).unwrap_or((data, &[]));
            if let Some(dst) = self.buf.get_mut(self.len..self.len.saturating_add(take)) {
                dst.copy_from_slice(head);
            }
            self.len = self.len.saturating_add(take);
            data = rest;
            if self.len < BLOCK {
                return;
            }
            let block = self.buf;
            compress(&mut self.state, &block);
            self.len = 0;
        }
        let (blocks, tail) = data.as_chunks::<BLOCK>();
        for block in blocks {
            compress(&mut self.state, block);
        }
        if let Some(dst) = self.buf.get_mut(..tail.len()) {
            dst.copy_from_slice(tail);
        }
        self.len = tail.len();
    }

    /// The digest as 64 lowercase hex digits.
    pub(crate) fn finish_hex(mut self) -> String {
        let bits = self.total.wrapping_mul(8);
        let pad = if self.len < 56 {
            56usize.saturating_sub(self.len)
        } else {
            120usize.saturating_sub(self.len)
        };
        let mut padding = [0u8; 72];
        if let Some(first) = padding.first_mut() {
            *first = 0x80;
        }
        let total = self.total;
        self.update(padding.get(..pad).unwrap_or_default());
        self.update(&bits.to_be_bytes());
        self.total = total;
        self.state
            .iter()
            .fold(String::with_capacity(64), |mut s, w| {
                for byte in w.to_be_bytes() {
                    s.push_str(&format!("{byte:02x}"));
                }
                s
            })
    }
}

/// SHA-256 of a byte slice, as hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut h = Sha256::default();
    h.update(bytes);
    h.finish_hex()
}

/// Size and SHA-256 of a file, streamed in 1 MiB reads.
pub(crate) fn file(path: &Path) -> std::io::Result<(u64, String)> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::default();
    let mut buf = vec![0u8; 1_048_576];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(buf.get(..n).unwrap_or_default());
        size = size.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
    }
    Ok((size, h.finish_hex()))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{hex, Sha256};

    #[test]
    fn fips_180_4_vectors() {
        assert_eq!(
            hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&million),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn split_updates_match_one_update() {
        let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        for split in [0usize, 1, 55, 56, 63, 64, 65, 127, 128, 999] {
            let (a, b) = data.split_at(split);
            let mut h = Sha256::default();
            h.update(a);
            h.update(b);
            assert_eq!(h.finish_hex(), hex(&data), "split at {split}");
        }
    }
}
