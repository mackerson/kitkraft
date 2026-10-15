//! Development aid: with `KITKRAFT_WORKBENCH_SHOTS=<dir>` set, the window fills each tab
//! from a startup `load` with a fixed tour of read-only requests, saves one image per tab
//! into `<dir>` and closes (how `docs/workbench.md` was checked). Nothing else in the
//! workbench writes a file, and this writes only the images, only when asked.

/// The environment variable naming the output directory.
pub const SHOTS_ENV: &str = "KITKRAFT_WORKBENCH_SHOTS";

/// A 32-bit top-down BMP of `width` x `height` RGBA pixels (row by row). `None` when the
/// sizes do not fit the format or `rgba` has the wrong length.
pub fn bmp(width: usize, height: usize, rgba: &[[u8; 4]]) -> Option<Vec<u8>> {
    const HEADER: usize = 54; // file header 14 + BITMAPINFOHEADER 40
    let pixels = width.checked_mul(height)?;
    if rgba.len() != pixels {
        return None;
    }
    let data_len = pixels.checked_mul(4)?;
    let file_len = u32::try_from(data_len.checked_add(HEADER)?).ok()?;
    let w = i32::try_from(width).ok()?;
    let h = i32::try_from(height).ok()?.checked_neg()?;
    let mut out = Vec::with_capacity(data_len.checked_add(HEADER)?);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_len.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(data_len).ok()?.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for [r, g, b, a] in rgba {
        out.extend_from_slice(&[*b, *g, *r, *a]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::bmp;

    #[test]
    fn header_and_pixel_order() {
        let img = bmp(2, 1, &[[1, 2, 3, 255], [4, 5, 6, 255]]).unwrap();
        assert_eq!(img.len(), 54 + 8);
        assert_eq!(&img[..2], b"BM");
        assert_eq!(u32::from_le_bytes(img[2..6].try_into().unwrap()), 62);
        assert_eq!(i32::from_le_bytes(img[22..26].try_into().unwrap()), -1);
        assert_eq!(&img[54..58], &[3, 2, 1, 255]);
        assert!(bmp(2, 2, &[[0; 4]]).is_none());
    }
}
