//! FNV-1a 64: cache keys, checksums, load-order fingerprints (not cryptographic).

/// Incremental FNV-1a 64.
#[derive(Debug, Clone, Copy)]
pub struct Fnv(u64);

impl Fnv {
    /// Fresh state.
    pub fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    /// Feed bytes.
    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /// Feed a `u64` (little-endian).
    pub fn write_u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes());
    }

    /// The hash.
    pub fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv {
    fn default() -> Fnv {
        Fnv::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_vector() {
        let mut h = super::Fnv::new();
        h.write(b"a");
        assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);
    }
}
