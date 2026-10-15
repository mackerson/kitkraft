//! Test fixtures: tiny plugins and the plugin lists of the owner's `KitKraft Dev` saves.

#![allow(clippy::unwrap_used, clippy::cast_possible_truncation)]

/// A plugin with only its TES4 record (flags, `HEDR`): enough for a load order.
pub(crate) fn tiny_plugin(flags: u32) -> Vec<u8> {
    let mut hedr = b"HEDR".to_vec();
    hedr.extend_from_slice(&12u16.to_le_bytes());
    hedr.extend_from_slice(&0.96f32.to_le_bytes());
    hedr.extend_from_slice(&0u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let mut out = b"TES4".to_vec();
    out.extend_from_slice(&(hedr.len() as u32).to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&[0u8; 12]);
    out.extend(hedr);
    out
}

/// Flags of the `KitKraft Dev` plugins (the corpus table in `docs/load-order.md`;
/// the KitKraft plugins are full masters).
pub(crate) fn dev_flags(name: &str) -> u32 {
    match name {
        n if n.starts_with("BlueprintShips-") => 0x881,
        "Constellation.esm" | "OldMars.esm" | "SFBGS004.esm" | "SFBGS007.esm" | "SFBGS008.esm"
        | "SFBGS047.esm" => 0x181,
        "SFBGS003.esm" | "SFBGS006.esm" => 0x481,
        "KitKraftFirstPOI.esm" | "KitKraftGunOverhaul.esm" => 0x1,
        _ => 0x81,
    }
}

/// The full plugins of the `KitKraft Dev` saves with the POI (Autosave1 `000039`, Exitsave0
/// `000041`; `docs/poi/first-poi-investigation.md`), in slot order, and whether each is
/// official (flag 0 in the save).
pub(crate) const DEV_FULL: [(&str, bool); 7] = [
    ("Starfield.esm", true),
    ("ShatteredSpace.esm", true),
    ("SFBGS00D.esm", true),
    ("SFBGS050.esm", true),
    ("KitKraftFirstPOI.esm", false),
    ("BlueprintShips-Starfield.esm", true),
    ("BlueprintShips-SFBGS050.esm", true),
];
/// Their small plugins (`FE` slots).
pub(crate) const DEV_SMALL: [(&str, bool); 6] = [
    ("Constellation.esm", true),
    ("OldMars.esm", true),
    ("SFBGS004.esm", true),
    ("SFBGS007.esm", true),
    ("SFBGS008.esm", true),
    ("SFBGS047.esm", true),
];
/// Their medium plugins (`FD` slots).
pub(crate) const DEV_MEDIUM: [(&str, bool); 2] = [("SFBGS003.esm", true), ("SFBGS006.esm", true)];
