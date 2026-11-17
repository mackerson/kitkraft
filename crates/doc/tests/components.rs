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
//!   identically, subrecord by subrecord. A typed layout's subrecord the decoder cannot
//!   decode exactly (a variable-width union's unnamed type, a miscounted payload) falls
//!   back to raw: the census counts those blocks with the raw ones, by layout, and both
//!   tests require none among the official files.
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
        "DisplayCase" => &["BGSDisplayCase"],
        "PlanetContentProperties" => &["BGSPlanetContentManagerContentProperties_Component"],
        "QualityUpgrade" => &["BGSQualityUpgrade_Component"],
        "SoundTag" => &["BGSSoundTag_Component"],
        // Upstream's keyword layout carries no name comment; the corpus has it only here.
        "Keywords" => &["BGSKeywordForm_Component"],
        "Destructible" => &["BGSDestructibleObject_Component"],
        "ObjectWindowFilter" => &["BGSObjectWindowFilter_Component"],
        "AttachParentArray" => &["BGSAttachParentArray_Component"],
        "PropertySheet" => &["BGSPropertySheet_Component"],
        "LightLayer" => &["TESModel_Component"],
        "PlanetModel" => &["TESPlanetModel_Component", "TESModel_Component"],
        "FullName" => &["TESFullName_Component"],
        "ObjectPaletteDefaults" => &["BGSObjectPaletteDefaults_Component"],
        "LinkedVoiceType" => &["BGSLinkedVoiceType_Component"],
        "ExternalDataSource" => &["BGSExternalComponentDataSource_Component"],
        "FormLinkData" => &["BGSFormLinkData_Component"],
        "Blueprint" => &["Blueprint_Component"],
        "HoudiniData" => &["HoudiniData_Component"],
        "AdaptiveTrigger" => &["BGSAdaptiveTriggerData_Component"],
        "AddToInventoryOnDestroy" => &["BGSAddToInventoryOnDestroy_Component"],
        "CityMapsUsage" => &["BGSCityMapsUsage_Component"],
        "SpawnOnDestroy" => &["BGSSpawnOnDestroy_Component"],
        "WorldSpaceOverlay" => &["BGSWorldSpaceOverlay_Component"],
        "ImageSpaceModifiable" => &["TESImageSpaceModifiableForm_Component"],
        "ForcedLocRefType" => &["BGSForcedLocRefType_Component"],
        "Container" => &["TESContainer_Component"],
        "BodyPartInfo" => &["BGSBodyPartInfo_Component"],
        "Crowd" => &["BGSCrowdComponent_Component"],
        "Volumes" => &["Volumes_Component"],
        "StoredTraversals" => &["BGSStoredTraversals_Component"],
        "ActivityTracker" => &["BGSActivityTracker"],
        "SpaceshipAiActor" => &["BGSSpaceshipAIActor_Component"],
        "SpaceshipEquipment" => &["BGSSpaceshipEquipment_Component"],
        "SpaceshipHullCode" => &["BGSSpaceshipHullCode_Component"],
        "SpaceshipWeaponBindings" => &["BGSSpaceshipWeaponBindings_Component"],
        // The `NamedLayout` union's variants (ADR 0015): the names upstream's DATA and DAT2
        // deciders list (L1479-L1551).
        "StarData" => &["BGSStarDataComponent_Component"],
        "OrbitedData" => &["BGSOrbitedDataComponent_Component"],
        "UniqueOverlayList" => &["UniqueOverlayList_Component"],
        "UniquePatternPlacement" => &["UniquePatternPlacementInfo_Component"],
        "OverlayDesignatedPlacement" => &["BGSOverlayDesignatedPlacementInfo_Component"],
        "BlockHeightAdjustment" => &["BlockHeightAdjustment_Component"],
        "SurfaceTreePatternSwap" => &["SurfaceTreePatternSwapInfo_Component"],
        "BlockEditorMetaData" => &["BGSBlockEditorMetaData_Component"],
        "ParticleSystem" => &["ParticleSystem_Component"],
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
    /// Layout -> record type -> blocks.
    layout_types: BTreeMap<String, BTreeMap<String, u64>>,
    /// (layout, signature) -> (occurrences, smallest payload, largest payload).
    sizes: BTreeMap<(String, String), (u64, usize, usize)>,
    /// Component name -> blocks whose data stays raw (`ComponentData`).
    raw: BTreeMap<String, u64>,
    /// (component name, the raw data's signatures in order) -> blocks.
    raw_shapes: BTreeMap<(String, String), u64>,
    /// (layout, component name) -> blocks a typed layout's matcher took but whose
    /// subrecords the generated decoder could not decode exactly, so they stay raw (`extra`):
    /// a variable-width union's unnamed type, a truncated payload. Counted in `raw` too.
    fallback: BTreeMap<(String, String), u64>,
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
    let Some(placed) = typed::placements(schema, sig, subs) else {
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
    let mut fell_back: BTreeSet<usize> = BTreeSet::new();
    let mut any = false;
    let mut raw_shape: Vec<String> = Vec::new();
    for (i, (s, p)) in subs.iter().zip(&placed).enumerate() {
        if s.signature() == Signature::new(b"BFCB") {
            block = i;
            name = String::from_utf8_lossy(s.data())
                .trim_end_matches('\0')
                .to_string();
            raw_shape.clear();
            continue;
        }
        if s.signature() == Signature::new(b"BFCE") && !raw_shape.is_empty() {
            *t.raw.entry(name.clone()).or_default() += 1;
            *t.raw_shapes
                .entry((name.clone(), raw_shape.join(" ")))
                .or_default() += 1;
            raw_shape.clear();
            continue;
        }
        if let Placement::Raw { path } = p {
            if path.ends_with("ComponentData") {
                raw_shape.push(s.signature().to_string());
            }
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
        any = true;
        if extra.contains(&i) {
            // Placed in a typed layout but not decodable by it: kept raw, counted.
            eprintln!(
                "{label}: subrecord {i} ({}) placed in {path} but kept raw",
                s.signature()
            );
            if fell_back.insert(block) {
                *t.fallback.entry((layout, name.clone())).or_default() += 1;
                *t.raw.entry(name.clone()).or_default() += 1;
            }
            continue;
        }
        t.placed += 1;
        let len = s.data().len();
        let e = t
            .sizes
            .entry((layout.clone(), s.signature().to_string()))
            .or_insert((0, usize::MAX, 0));
        *e = (e.0 + 1, e.1.min(len), e.2.max(len));
        if counted.insert((block, layout.clone())) {
            *t.layout_types
                .entry(layout.clone())
                .or_default()
                .entry(r.header.signature.to_string())
                .or_default() += 1;
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
    for (layout, types) in &t.layout_types {
        eprintln!("  {layout} record types: {types:?}");
    }
    for ((layout, sig), (n, lo, hi)) in &t.sizes {
        eprintln!("  {layout} {sig}: {n} subrecords, {lo}..={hi} bytes");
    }
    let raw_total: u64 = t.raw.values().sum();
    let fallback_total: u64 = t.fallback.values().sum();
    eprintln!(
        "raw component blocks: {raw_total} in {} kinds ({fallback_total} of them typed layouts that fell back to raw)",
        t.raw.len()
    );
    for ((layout, name), n) in &t.fallback {
        eprintln!("  fallback {layout} / {name}: {n}");
    }
    let mut by_count: Vec<(&String, &u64)> = t.raw.iter().collect();
    by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (name, n) in by_count {
        eprintln!("  raw {name}: {n}");
    }
    if std::env::var_os("KITKRAFT_COMPONENT_SHAPES").is_some() {
        for ((name, shape), n) in &t.raw_shapes {
            eprintln!("    shape {name} [{shape}]: {n}");
        }
    }
    t
}

#[test]
fn component_layouts_small() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let t = scan(&dir, &SMALL);
    assert!(t.fallback.is_empty(), "fallbacks: {:?}", t.fallback);
}

#[test]
#[ignore = "release: reads all 14 official files (about 2.5 GB)"]
fn component_layouts_full() {
    let Some(dir) = data_dir() else {
        eprintln!("skip: KITKRAFT_DATA_DIR unset");
        return;
    };
    let t = scan(&dir, &ALL);
    // No official block falls back to raw from a typed layout.
    assert!(t.fallback.is_empty(), "fallbacks: {:?}", t.fallback);
    // The counts common/BfcbBlock.toml cites (2026-10-15; round 4 layouts 2026-10-16; the
    // layouts of KK-282 onward 2026-11-15). Recounted 2026-11-15: the record types typed
    // since round 4 carry 93 more animation graph, 1 more contact shadow, 1,188 more
    // particle system and 1,150 more reflection blocks; the old counts were stale. The
    // `NamedLayout` union's variants (ADR 0015, KK-294): 1,196 DATA/DAT2 blocks in eight
    // kinds. The GBFM ship-part layouts (2026-11-16): voice type, AI actor, equipment, hull
    // code, weapon bindings; then external data source, form links, blueprint and the
    // opaque Houdini data; then the small layouts (adaptive trigger, add to inventory and
    // spawn on destroy, city maps, world space overlay, image space, forced loc ref type,
    // container, body part info) and Crowd.
    let blocks: Vec<(&str, u64)> = t.blocks.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(
        blocks,
        [
            ("ActivityTracker", 10),
            ("AdaptiveTrigger", 19),
            ("AddToInventoryOnDestroy", 1_702),
            ("AnimationGraph", 4_853),
            ("AttachParentArray", 2_396),
            ("BlockEditorMetaData", 585),
            ("BlockHeightAdjustment", 255),
            ("Blueprint", 2_030),
            ("BodyPartInfo", 1),
            ("CityMapsUsage", 4),
            ("ContactShadow", 147_487),
            ("Container", 83),
            ("Crowd", 24),
            ("Destructible", 2_149),
            ("DisplayCase", 156),
            ("ExternalDataSource", 2_030),
            ("ForcedLocRefType", 14),
            ("FormLinkData", 2_334),
            ("FullName", 5_695),
            ("HoudiniData", 1_825),
            ("ImageSpaceModifiable", 50),
            ("Keywords", 83_015),
            ("LightLayer", 1_868),
            ("LinkedVoiceType", 2_030),
            ("ObjectPaletteDefaults", 6_105),
            ("ObjectWindowFilter", 3_062),
            ("OrbitedData", 123),
            ("OverlayDesignatedPlacement", 1),
            ("ParticleSystem", 2_733),
            ("PlanetContentProperties", 1_272),
            ("PlanetModel", 3_892),
            ("PropertySheet", 2_064),
            ("QualityUpgrade", 216),
            ("Reflection", 9_651),
            ("SoundTag", 966),
            ("SpaceshipAiActor", 2_030),
            ("SpaceshipEquipment", 2_030),
            ("SpaceshipHullCode", 134),
            ("SpaceshipWeaponBindings", 2_030),
            ("SpawnOnDestroy", 75),
            ("StarData", 123),
            ("StoredTraversals", 1_258),
            ("SurfaceTreePatternSwap", 33),
            ("UniqueOverlayList", 38),
            ("UniquePatternPlacement", 38),
            ("Volumes", 23_159),
            ("WorldSpaceOverlay", 582),
        ]
    );
}
