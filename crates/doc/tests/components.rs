//! The component layouts the shared base form component block types (lane D, 2026-10-15;
//! `crates/schema/defs/common/BfcbBlock.toml`) on the official files (`KITKRAFT_DATA_DIR`;
//! skips when unset).
//!
//! For every record of a defined type that has BFCB..BFCE blocks, its subrecords are placed
//! in the definition (`kitkraft_doc::typed::placements`, the generated decoder's rules) and
//! decoded through the generated record. Checks:
//!
//! - every subrecord placed in one of the typed layouts sits in a block whose component name
//!   (the BFCB string) is one upstream gives that layout (the block's data is read by its
//!   first subrecord, as upstream's record union does; the names must agree);
//! - the generated decoder keeps none of them raw (`extra`), and the record re-encodes
//!   identically, subrecord by subrecord.
//!
//! `component_layouts_small` runs in `cargo test` (the five small files);
//! `component_layouts_full` (ignored) runs all 14 and checks the block counts
//! `BfcbBlock.toml` cites:
//! `cargo test -p kitkraft-doc --release --test components -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kitkraft_doc::typed::{self, Placement};
use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::driver::visit_file;
use kitkraft_esm::generated::records::Record as Generated;
use kitkraft_esm::{DecodeLimits, Entry, ParseOptions, Record, Signature, Subrecord};

const SMALL: [&str; 5] = [
    "Constellation.esm",
    "OldMars.esm",
    "SFBGS007.esm",
    "SFBGS004.esm",
    "SFBGS047.esm",
];

const ALL: [&str; 14] = [
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "BlueprintShips-Starfield.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS006.esm",
    "SFBGS003.esm",
    "ShatteredSpace.esm",
    "SFBGS004.esm",
    "SFBGS00D.esm",
    "SFBGS050.esm",
    "BlueprintShips-SFBGS050.esm",
    "SFBGS047.esm",
];

/// The component names upstream gives each typed layout
/// (`wbDefinitionsSF1.pas` L6055-L6828: the comment above each variant).
fn allowed(layout: &str) -> &'static [&'static str] {
    match layout {
        "AnimationGraph" => &["BGSAnimationGraph_Component"],
        "ContactShadow" => &["BGSContactShadowComponent_Component"],
        "PlanetContentProperties" => &["BGSPlanetContentManagerContentProperties_Component"],
        // Upstream lists five names on the REFL layout; the corpus has the same one-REFL
        // body under two more names (ReflectionProbes, SpacePhysics).
        "Reflection" => &[
            "BGSEffectSequenceComponent",
            "BGSLodOwner_Component",
            "BGSMaterialPropertyComponent",
            "LensFlareAttachmentFormComponent",
            "LightAttachmentFormComponent",
            "ReflectionProbes_Component",
            "BGSSpacePhysicsComponent",
        ],
        _ => &[],
    }
}

fn data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))?,
    );
    dir.join("Starfield.esm").is_file().then_some(dir)
}

#[derive(Debug, Default)]
struct Tally {
    /// Blocks per typed layout.
    blocks: BTreeMap<String, u64>,
    /// (layout, component name) -> blocks.
    names: BTreeMap<(String, String), u64>,
    /// Subrecords placed in a typed layout.
    placed: u64,
    /// Record types with typed component data.
    types: BTreeSet<String>,
}

fn check_record(
    plugin: &str,
    localized: bool,
    r: &Record<'_>,
    subs: &[Subrecord<'_>],
    t: &mut Tally,
) {
    if !subs
        .iter()
        .any(|s| s.signature() == Signature::new(b"BFCB"))
    {
        return;
    }
    let schema = kitkraft_schema::builtin().unwrap();
    let sig = r.header.signature.0;
    let label = format!("{plugin} {}:{:08X}", r.header.signature, r.header.form_id);
    let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
    let Some(placed) = typed::placements(schema, sig, &sigs) else {
        return;
    };
    let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
    let ctx = Ctx {
        form_version: r.header.form_version,
        localized,
    };
    let generated = Generated::decode(sig, &raw, ctx).unwrap();
    let extra: BTreeSet<usize> = generated.extra().iter().map(|(i, _)| *i).collect();
    let mut name = String::new();
    let mut block = 0usize;
    let mut counted: BTreeSet<(usize, String)> = BTreeSet::new();
    let mut any = false;
    for (i, (s, p)) in subs.iter().zip(&placed).enumerate() {
        if s.signature() == Signature::new(b"BFCB") {
            block = i;
            name = String::from_utf8_lossy(s.data())
                .trim_end_matches('\0')
                .to_string();
            continue;
        }
        let Placement::Member { path, .. } = p else {
            continue;
        };
        if !kitkraft_schema::in_component_layout(path) {
            continue;
        }
        let layout = path.split('.').nth(1).unwrap().to_string();
        assert!(
            allowed(&layout).contains(&name.as_str()),
            "{label}: subrecord {i} ({}) is read as the {layout} layout in a `{name}` block",
            s.signature()
        );
        assert!(
            !extra.contains(&i),
            "{label}: subrecord {i} ({}) placed in {path} but kept raw",
            s.signature()
        );
        any = true;
        t.placed += 1;
        if counted.insert((block, layout.clone())) {
            *t.blocks.entry(layout.clone()).or_default() += 1;
            *t.names.entry((layout, name.clone())).or_default() += 1;
        }
    }
    if any {
        t.types.insert(r.header.signature.to_string());
        let encoded = generated.encode();
        assert_eq!(encoded.len(), raw.len(), "{label}: subrecord count");
        for (i, (a, b)) in raw.iter().zip(&encoded).enumerate() {
            assert!(
                a.signature == b.signature
                    && a.data == b.data.as_slice()
                    && a.oversized == b.oversized,
                "{label}: subrecord {i} re-encodes differently"
            );
        }
    }
}

fn walk(
    plugin: &str,
    localized: bool,
    entry: &Entry<'_>,
    t: &mut Tally,
) -> kitkraft_esm::Result<()> {
    match entry {
        Entry::Group(g) => {
            for child in g.children() {
                walk(plugin, localized, child, t)?;
            }
        }
        Entry::Record(r) => {
            if r.header.signature == Signature::TES4 {
                return Ok(());
            }
            let decoded = r.decode(&DecodeLimits::default())?;
            let subs: &[Subrecord<'_>] = match (&decoded, r.subrecords()) {
                (Some(d), _) => d,
                (None, Some(s)) => s,
                (None, None) => &[],
            };
            check_record(plugin, localized, r, subs, t);
        }
    }
    Ok(())
}

fn scan(dir: &Path, names: &[&str]) -> Tally {
    let mut t = Tally::default();
    for name in names {
        let path = dir.join(name);
        let header = kitkraft_doc::index::read_header(&path).unwrap();
        let localized = header.flags.localized();
        visit_file(&path, &ParseOptions::default(), |entry| {
            walk(name, localized, entry, &mut t)
        })
        .unwrap();
    }
    eprintln!("typed component subrecords: {}", t.placed);
    for (layout, n) in &t.blocks {
        eprintln!("  {layout}: {n} blocks");
    }
    for ((layout, name), n) in &t.names {
        eprintln!("    {layout} / {name}: {n}");
    }
    eprintln!("  record types: {:?}", t.types);
    t
}

#[test]
fn component_layouts_small() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    scan(&dir, &SMALL);
}

#[test]
#[ignore = "release: reads all 14 official files (about 2.5 GB)"]
fn component_layouts_full() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let t = scan(&dir, &ALL);
    // The counts common/BfcbBlock.toml cites (2026-10-15).
    let blocks: Vec<(&str, u64)> = t.blocks.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(
        blocks,
        [
            ("AnimationGraph", 4_760),
            ("ContactShadow", 147_486),
            ("PlanetContentProperties", 1_272),
            ("Reflection", 8_501),
        ]
    );
}
