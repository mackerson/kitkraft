//! Four-character codes (`TES4`, `GRUP`, `WEAP`, `FULL`, ...).

use std::fmt;

/// A four-byte record, group, or subrecord signature, kept exactly as read.
///
/// Signatures are usually ASCII but input is hostile, so any four bytes are accepted;
/// [`fmt::Display`] escapes non-printable bytes as `\xNN`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Signature(pub [u8; 4]);

impl Signature {
    /// `TES4`: the plugin header record.
    pub const TES4: Signature = Signature(*b"TES4");
    /// `GRUP`: a group header.
    pub const GRUP: Signature = Signature(*b"GRUP");
    /// `XXXX`: carries the 32-bit size of the subrecord that follows it.
    pub const XXXX: Signature = Signature(*b"XXXX");

    /// Build a signature from four bytes, e.g. `Signature::new(b"WEAP")`.
    pub const fn new(bytes: &[u8; 4]) -> Signature {
        Signature(*bytes)
    }

    /// The raw bytes.
    pub const fn bytes(self) -> [u8; 4] {
        self.0
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            if b.is_ascii_graphic() || b == b' ' {
                write!(f, "{}", char::from(b))?;
            } else {
                write!(f, "\\x{b:02X}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signature({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::Signature;

    #[test]
    fn displays_ascii_and_escapes_the_rest() {
        assert_eq!(Signature::new(b"NPC_").to_string(), "NPC_");
        assert_eq!(Signature([b'A', 0, 0xFF, b'Z']).to_string(), "A\\x00\\xFFZ");
    }
}
