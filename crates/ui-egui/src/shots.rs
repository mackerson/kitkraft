//! Development aid: with `KITKRAFT_WORKBENCH_SHOTS=<dir>` set, the window fills each tab
//! from a startup `load` with a fixed tour of read-only requests, saves one image per tab
//! into `<dir>` and closes (how `docs/workbench.md` was checked). Nothing else in the
//! workbench writes a file, and this writes only the images, only when asked.

/// The environment variable naming the output directory.
pub const SHOTS_ENV: &str = "KITKRAFT_WORKBENCH_SHOTS";

/// The tour's output directory from [`SHOTS_ENV`]'s value, or why it is refused: the
/// game's Data folder, the user Data folder (`My Games\Starfield\Data`), an MO2 instance
/// or profile (at any depth), or a path through a link, as the engine refuses for its
/// own outputs.
pub fn shots_dir(value: &std::ffi::OsStr) -> Result<std::path::PathBuf, String> {
    if value.is_empty() {
        return Err(format!("{SHOTS_ENV} is empty"));
    }
    let dir = std::path::PathBuf::from(value);
    crate::gate::guard_output_dir(&dir)
        .map_err(|why| format!("{SHOTS_ENV}: no screenshots written: {why}"))?;
    Ok(dir)
}

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
    use super::{bmp, shots_dir};

    #[test]
    fn the_tour_refuses_mo2_and_both_data_folders() {
        let base = std::env::temp_dir().join(format!("kitkraft-ui-shots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mo2 = base.join("MO2");
        std::fs::create_dir_all(mo2.join("profiles").join("KitKraft Dev")).unwrap();
        std::fs::write(mo2.join("ModOrganizer.ini"), b"[General]\n").unwrap();
        let game = base.join("Game").join("Data");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("Starfield.esm"), b"x").unwrap();
        let user = base.join("My Games").join("Starfield").join("Data");
        std::fs::create_dir_all(&user).unwrap();
        let fine = base.join("shots");

        let refused = [
            mo2.clone(),
            mo2.join("profiles").join("KitKraft Dev"),
            mo2.join("mods").join("new"),
            game.clone(),
            game.join("shots"),
            user.clone(),
        ]
        .map(|d| shots_dir(d.as_os_str()));
        let ok = shots_dir(fine.as_os_str());
        let empty = shots_dir(std::ffi::OsStr::new(""));
        let _ = std::fs::remove_dir_all(&base);
        for r in refused {
            assert!(r.is_err(), "{r:?}");
        }
        assert_eq!(ok.unwrap(), fine);
        assert!(empty.is_err());
    }

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
