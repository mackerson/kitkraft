//! Development aid: with `KITKRAFT_WORKBENCH_SHOTS=<dir>` set, the window fills each tab
//! from a startup `load` with a fixed tour of read-only requests, saves one PNG per tab
//! into `<dir>` and closes (how `docs/workbench.md` was checked). Nothing else in the
//! workbench writes a file, and this writes only the images, only when asked.

/// The environment variable naming the output directory.
pub const SHOTS_ENV: &str = "KITKRAFT_WORKBENCH_SHOTS";

/// The window's inner size during the tour, in points (pixels at 100 percent scaling):
/// taller than the usual 1400 x 900 so the Records image holds the query table and the
/// selected record's inspector together, and still inside a 1920 x 1200 screen with the
/// taskbar.
pub const TOUR_SIZE: [f32; 2] = [1400.0, 1100.0];

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

/// A PNG of `width` x `height` RGBA pixels (row by row), written as 8-bit RGB (a window
/// is opaque) at the encoder's best compression, through the `image` crate eframe
/// already builds (ADR 0007 Amendment 3, dependency table). `None` when the sizes do not
/// fit or `rgba` has the wrong length.
pub fn png(width: usize, height: usize, rgba: &[[u8; 4]]) -> Option<Vec<u8>> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;
    let pixels = width.checked_mul(height)?;
    if rgba.len() != pixels {
        return None;
    }
    let rgb: Vec<u8> = rgba.iter().flat_map(|[r, g, b, _]| [*r, *g, *b]).collect();
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Best, FilterType::Adaptive)
        .write_image(
            &rgb,
            u32::try_from(width).ok()?,
            u32::try_from(height).ok()?,
            image::ExtendedColorType::Rgb8,
        )
        .ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{png, shots_dir};

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
    fn png_signature_size_and_pixels_read_back() {
        let img = png(2, 1, &[[1, 2, 3, 255], [4, 5, 6, 255]]).unwrap();
        assert_eq!(&img[..8], b"\x89PNG\r\n\x1a\n");
        // IHDR: width 2, height 1, 8 bits, color type 2 (RGB).
        assert_eq!(&img[12..16], b"IHDR");
        assert_eq!(u32::from_be_bytes(img[16..20].try_into().unwrap()), 2);
        assert_eq!(u32::from_be_bytes(img[20..24].try_into().unwrap()), 1);
        assert_eq!((img[24], img[25]), (8, 2));
        let back = image::load_from_memory_with_format(&img, image::ImageFormat::Png)
            .unwrap()
            .to_rgb8();
        assert_eq!(back.as_raw(), &[1, 2, 3, 4, 5, 6]);
        assert!(png(2, 2, &[[0; 4]]).is_none());
    }
}
