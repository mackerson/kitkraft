//! Bounded little-endian reader over a byte slice.

use crate::{Error, Result, Signature};

/// A cursor over `&[u8]` whose every read is bounds-checked.
///
/// `base` is the absolute offset of `data[0]` in the file, so errors report file offsets
/// even when the reader covers only one group or one record body.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    base: u64,
}

impl<'a> Reader<'a> {
    /// A reader over `data`, whose first byte sits at absolute offset `base`.
    pub fn new(data: &'a [u8], base: u64) -> Reader<'a> {
        Reader { data, pos: 0, base }
    }

    /// Absolute offset of the next byte to be read.
    pub fn offset(&self) -> u64 {
        // usize -> u64 never truncates on supported targets.
        self.base.saturating_add(self.pos as u64)
    }

    /// Bytes left to read.
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Whether every byte has been read.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Take the next `n` bytes, labelled `what` in the error if they are not there.
    pub fn take_labeled(&mut self, n: usize, what: &'static str) -> Result<&'a [u8]> {
        let available = self.remaining();
        let slice = self
            .pos
            .checked_add(n)
            .and_then(|end| self.data.get(self.pos..end));
        match slice {
            Some(slice) => {
                self.pos = self.pos.saturating_add(n);
                Ok(slice)
            }
            None => Err(Error::UnexpectedEof {
                offset: self.offset(),
                what,
                needed: n as u64,
                available: available as u64,
            }),
        }
    }

    /// Take the next `n` bytes.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        self.take_labeled(n, "data")
    }

    /// Take the next `N` bytes as an array.
    pub fn array<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N]> {
        let offset = self.offset();
        let slice = self.take_labeled(N, what)?;
        <[u8; N]>::try_from(slice).map_err(|_| Error::UnexpectedEof {
            offset,
            what,
            needed: N as u64,
            available: slice.len() as u64,
        })
    }

    /// Read a `u8`.
    pub fn u8(&mut self) -> Result<u8> {
        let [b] = self.array::<1>("u8")?;
        Ok(b)
    }

    /// Read a little-endian `u16`.
    pub fn u16(&mut self) -> Result<u16> {
        self.array::<2>("u16").map(u16::from_le_bytes)
    }

    /// Read a little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32> {
        self.array::<4>("u32").map(u32::from_le_bytes)
    }

    /// Read a little-endian `i32`.
    pub fn i32(&mut self) -> Result<i32> {
        self.array::<4>("i32").map(i32::from_le_bytes)
    }

    /// Read a little-endian `u64`.
    pub fn u64(&mut self) -> Result<u64> {
        self.array::<8>("u64").map(u64::from_le_bytes)
    }

    /// Read a little-endian `f32`.
    pub fn f32(&mut self) -> Result<f32> {
        self.array::<4>("f32").map(f32::from_le_bytes)
    }

    /// Read a four-byte signature.
    pub fn signature(&mut self) -> Result<Signature> {
        self.array::<4>("signature").map(Signature)
    }

    /// Look at the next signature without consuming it.
    pub fn peek_signature(&self) -> Result<Signature> {
        self.clone().signature()
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::Reader;
    use crate::{Error, Signature};

    fn eof(err: Error) -> (u64, &'static str, u64, u64) {
        match err {
            Error::UnexpectedEof {
                offset,
                what,
                needed,
                available,
            } => (offset, what, needed, available),
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn reads_little_endian_values_and_tracks_offsets() {
        let data = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, b'W', b'E', b'A', b'P', 0xFF, 0xFF, 0xFF,
            0xFF,
        ];
        let mut r = Reader::new(&data, 1000);
        assert_eq!(r.u8().unwrap(), 0x01);
        assert_eq!(r.offset(), 1001);
        assert_eq!(r.u16().unwrap(), 0x0302);
        assert_eq!(r.u32().unwrap(), 0x0706_0504);
        assert_eq!(r.peek_signature().unwrap(), Signature::new(b"WEAP"));
        assert_eq!(r.signature().unwrap(), Signature::new(b"WEAP"));
        assert_eq!(r.i32().unwrap(), -1);
        assert!(r.is_empty());
        assert_eq!(r.offset(), 1015);
    }

    #[test]
    fn every_read_past_the_end_is_an_error_at_the_right_offset() {
        let data = [0u8; 3];
        type Read = fn(&mut Reader<'_>) -> Result<(), Error>;
        let reads: [(&str, usize, Read); 9] = [
            ("u8", 1, |r| r.u8().map(drop)),
            ("u16", 2, |r| r.u16().map(drop)),
            ("u32", 4, |r| r.u32().map(drop)),
            ("i32", 4, |r| r.i32().map(drop)),
            ("u64", 8, |r| r.u64().map(drop)),
            ("f32", 4, |r| r.f32().map(drop)),
            ("signature", 4, |r| r.signature().map(drop)),
            ("signature", 4, |r| r.peek_signature().map(drop)),
            ("data", 5, |r| r.take(5).map(drop)),
        ];
        for start in 0..=3usize {
            for (what, width, read) in &reads {
                let mut r = Reader::new(&data, 500);
                r.take(start).unwrap();
                let left = 3 - start;
                let result = read(&mut r);
                if *width <= left {
                    assert!(result.is_ok(), "{what} at {start}");
                    continue;
                }
                let (offset, label, needed, available) = eof(result.unwrap_err());
                assert_eq!(offset, 500 + start as u64, "{what} at {start}");
                assert_eq!(label, *what);
                assert_eq!(needed, *width as u64);
                assert_eq!(available, left as u64);
                // A failed read consumes nothing.
                assert_eq!(r.offset(), 500 + start as u64);
            }
        }
    }

    #[test]
    fn huge_take_does_not_overflow() {
        let data = [0u8; 4];
        let mut r = Reader::new(&data, u64::MAX - 1);
        r.take(2).unwrap();
        assert_eq!(r.offset(), u64::MAX);
        let (_, _, needed, available) = eof(r.take(usize::MAX).unwrap_err());
        assert_eq!(needed, usize::MAX as u64);
        assert_eq!(available, 2);
    }

    #[test]
    fn labeled_take_names_the_field() {
        let mut r = Reader::new(&[], 7);
        let (offset, what, _, _) = eof(r.take_labeled(24, "record header").unwrap_err());
        assert_eq!((offset, what), (7, "record header"));
    }
}
