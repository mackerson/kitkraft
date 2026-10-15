# Planet content family: what the schema says, and what it does not

For mod authors who want to read or patch Starfield's procedural planet content with
KitCraft: the Planet Content Manager trees (PCMT, PCBN, PCCN), pack-ins (PKIN, LVLP),
generic base forms (GBFM), biomes (BIOM), planets and stars (PNDT, STDT, SUNP), terrain
blocks and patterns (SFBK, SFPT), resources (RSGD), research projects (RSPJ) and
traversals (TRAV). The definitions are in `crates/schema/defs/<SIG>.toml`; `kit schema
--type PNDT` prints any of them with sources and confidence.

Everything below was measured on the 14 official ESMs on 2026-10-14. Layouts come from the
pinned xEdit definitions (`wbDefinitionsSF1.pas` and `wbDefinitionsCommon.pas` at
`9fb01688`, ADR 0004); each definition cites its line ranges. "corpus" confidence means
every official record of the type decodes with every subrecord placed in a member, nothing
left over, and encodes back to the same bytes. Members that never occur in an official file
stay at `reference` confidence (documented upstream, not yet seen).

## Coverage

| Type | Records | Files | Typed leaf members | Opaque members | Never seen (`reference`) |
| ---- | ------: | ----: | -----------------: | -------------- | ------------------------ |
| PCMT | 5 | 1 | 10 of 11 | component block | component block, PCCB, KWDA, CIS1/CIS2 |
| PCBN | 379 | 4 | 14 of 15 | component block | PCCB, CIS1/CIS2 |
| PCCN | 2,148 | 4 | 6 of 7 | component block | - |
| LVLP | 52 | 2 | 24 of 26 | VMAD, component block | VMAD, PTT2, XALG, MOLM, MODC, XFLG, CIS1/CIS2 |
| PKIN | 15,128 | 8 | 20 of 22 | VMAD, component block | - |
| GBFM | 3,903 | 10 | 11 of 15 | VMAD, component block, NVNM | - |
| BIOM | 434 | 2 | 47 of 48 | BNAM (plus raw slots inside GNAM) | ZNAM |
| PNDT | 1,786 | 5 | 23 of 24 | component block (plus raw slots inside PPBD and HNAM) | XEMP |
| STDT | 124 | 2 | 10 of 13 | component block, ONAM, WED0 | ONAM, SNAM, HNAM, WED0 |
| SUNP | 53 | 2 | 2 of 4 | REFL, RDIF | - |
| SFBK | 1,877 | 4 | 17 of 18 | component block | - |
| SFPT | 264 | 2 | 8 of 9 | component block | BNAM, EFRM, EORI |
| RSGD | 32 | 1 | 3 of 4 | component block | component block |
| RSPJ | 82 | 1 | 15 of 16 | component block | component block, conditions |
| TRAV | 23 | 1 | 2 of 4 | VMAD, component block | VMAD, component block |

All 26,290 records decode and re-encode byte-identically (PNDT records are all compressed
and are checked after decompression). Two checks, both on all 14 files with the generated
decoder regenerated from these definitions: `kitcraft-esm`'s `corpus_full_roundtrip`
typed check (M2.A3, release: 0 conflicts, 0 unchecked records), and a stricter
out-of-tree pass that also requires every subrecord to be placed in a member and
`Record::extra()` to be empty for every record (the driver's own `extra` check covers only
five older types, a lane A request). PCCN occurs in the corpus (2,148 records, more than
PCBN's 379). No RSPG or RSSD records exist in any official file; RSPJ is the research
lab, not a planet resource record, and is included because it is defined alongside.

New shared definitions (`defs/common/`): `Condition` (CTDA plus CIS1/CIS2),
`ObjectBounds`, `ObjectPaletteDefaults`, `Transforms` (field names match the weapon
family's inline layouts so they can be merged), `ColorBgra`, `BiomeNoiseLayer`,
`ResourceBiomeTier`, `ResourceCellTier`.

## How procedural POI placement composes

**Trees.** The five PCMT records are the roots, one per kind of request their EditorIDs
name, and five default-object records (DFOB) in Starfield.esm point at them:

| Tree (PCMT EditorID) | Content nodes under it (Starfield.esm) |
| -------------------- | -------------------------------------: |
| `PCM_BlockCreationRequest` | 892 |
| `PCM_CellLoadRequest` | 525 |
| `PCM_QuestRequest` | 249 |
| `PCM_ScanPlanetRequest` | 132 |
| `PCM_SpaceshipLandingRequest` | 7 |

**Links run upwards.** A branch node (PCBN) names its parent in `ParentNode` (PRTN: a PCMT
or another PCBN) and its previous sibling in `PreviousNode` (PRVN). A content node (PCCN)
does the same under a PCBN. The downward child lists that upstream also defines (PCCB on
PCMT and PCBN) are never written in an official file, so a new node is attached by
pointing its PRTN at the parent and its PRVN at the last existing sibling (the first child
has no PRVN). Content nodes sit 2 to 6 levels below their root.

**Selection.** Each branch has a `ChildSelection` (`Random` or `Stacked`; a tree is always
`Stacked`), optional count and distribution curves (CURV), a consume-on-failure flag,
conditions and keywords. The conditions are where planets, biomes and locations come in.
The functions used on PCBN conditions in Starfield.esm are, by frequency: the planetary
body has a keyword (92), a global's value (72), distance from a location with a keyword
(55), whether a planet trait is known (54), the subject has a keyword (40), a condition
form (38), a random percentage (28), the biome has a keyword (14), the biome mask value at
the position (14), the system has a keyword (11), and whether a location is the current
one (11). Leveled pack-in entries test whether the location has a keyword (48). So a tree
is routed by body, biome, system and location keywords (LCTN and their keyword sets), not
by direct record links: no PCMT, PCBN or PCCN member names a BIOM or LCTN, and in
Starfield.esm no 4-byte BIOM or LCTN subrecord names a tree node.

**Content.** A content node's `Content` (PCCC) is what gets placed: in Starfield.esm a
worldspace 1,277 times, a pack-in 507 times, a leveled pack-in 12 times.
Five nodes name a placed reference instead, which upstream does not allow; their EditorIDs
mark them as duplicates or block-creation leftovers. Four nodes have no content at all.

**Placement properties.** Branches, content nodes, leveled pack-ins and pack-ins can carry
a `BGSPlanetContentManagerContentProperties_Component` block (284 PCBN, 108 PCCN, 10 LVLP,
448 PKIN). Its fields are, per upstream (`wbDefinitionsSF1.pas` L6180-L6198): do-all-before-repeating, global, per-system,
per-planet and per-planet-world limits on how often the content may be placed, whether the
placement is saved, a days-until-reset global, minimum density chance (value and global),
debug colour, per-request, spawn-group-limit and spawn-group-distance curves, whether
invalid biome markers are allowed, and its own CITC/CTDA conditions. A content node's
`IOVR` marker (93 nodes, always together with this block) makes the node's properties and
conditions replace those of the content it places. This block is kept as raw bytes today
(see "Opaque members").

## How pack-ins instantiate

- **PKIN** is a bundle of references stored in its own cell: `Cell` (CNAM) names that CELL
  in all 11,281 Starfield.esm pack-ins. Placing a pack-in instantiates that cell's
  references as a group; the record adds bounds, palette defaults (OPDS, up to two slots),
  snap template and behaviour, filter path, keywords, properties (`PRPS`: actor value,
  value, curve), a native terminal, material swaps and an optional display name. `Flags`
  (FNAM) bits 1..6 are named upstream (grouped workshop object, fixed, grouped, one motion,
  user-defined primitives, override dirtiness); bit 0 and a few high bits are set but
  unnamed. `Version` (VNAM) is 0 everywhere.
- **LVLP** is a leveled list over pack-ins: `Entry` groups of `LVLO` (level, pack-in or
  nested LVLP, count, chance none) each followed by its own conditions; list-level chance
  none, max count, flags (`EvalAsStack` 0x20 and `DoAllBeforeRepeating` 0x100 occur), an
  optional global and the stored entry count `LLCT`.
- **BIOM** scatters pack-ins without a tree: each `Mask` group (one of Base, Solid,
  FlatOuter, FlatInner, Talus, Flow, Path) lists `Objects` (GNAM; 50,172 in all), each a
  pack-in or leveled pack-in (46,584 and 3,271 in Starfield.esm) with a density and other
  values, plus the mask's footprint sizes.

Entering a new pack-in into a tree means: a PKIN (or LVLP), a PCCN whose PCCC names it and
whose PRTN names the branch, and the PRVN chain fixed up. Counts such as `LLCT`, `CITC`
and PPBD's element counts are stored, not derived: edit them together with their lists.

## GBFM: generic base forms are their components

A GBFM is almost entirely base form components (BFCB..BFCE blocks): 50,639 blocks in 3,903
records, 27 component names. The record's own members are bounds, dirtiness, palette
defaults, an optional filter, the template (`Template`, ANAM: a GBFT in all 3,141
Starfield.esm records), the instance-data names (STRV: `BGSMod_Template_Component` in
every record, `Spaceship_InstanceData` in 2,073), an object template (OBTE count 1 with one
OBTF/OBTS combination, 2,030 records), two STOP markers and, for 1,308 records, navmesh
geometry. Component use:

| Component | Records |
| --------- | ------: |
| BGSPropertySheet_Component | 3,876 |
| BGSKeywordForm_Component | 3,872 |
| TESFullName_Component | 3,866 |
| BGSAttachParentArray_Component | 3,321 |
| BGSDestructibleObject_Component | 2,813 |
| BGSFormLinkData_Component | 2,334 |
| TESModel_Component | 2,033 |
| BGSPathingData_Component | 2,031 |
| BGSAnimationGraph_Component | 2,030 |
| BGSExternalComponentDataSource_Component | 2,030 |
| BGSLinkedVoiceType_Component | 2,030 |
| BGSPapyrusScripts_Component | 2,030 |
| BGSSpaceshipAIActor_Component | 2,030 |
| BGSSpaceshipEquipment_Component | 2,030 |
| BGSSpaceshipWeaponBindings_Component | 2,030 |
| Blueprint_Component | 2,030 |
| TESMagicTargetForm_Component | 2,030 |
| BGSForcedLocRefType_Component | 2,022 |
| TESContainer_Component | 2,019 |
| BGSAddToInventoryOnDestroy_Component | 1,723 |
| BGSStoredTraversals_Component | 1,258 |
| BGSSoundTag_Component | 815 |
| BGSObjectWindowFilter_Component | 200 |
| BGSSpaceshipHullCode_Component | 134 |
| BGSCrowdComponent_Component | 24 |
| BGSPrimitive_Component | 24 |
| BGSSpawnOnDestroy_Component | 4 |

The 2,030 records that share eleven components are the ones with an object template; their
component names (spaceship AI, equipment, weapon bindings, blueprint) suggest ship
modules. A
GBFM's name, keywords, properties and model are inside these blocks, so they are not typed
members of GBFM today; `inspect` shows them as raw subrecords inside the block.

Components elsewhere in the family: PKIN uses BGSLodOwner (827), the placement properties
(448), BGSEffectSequence (105), BGSSpacePhysics (5), LensFlareAttachmentForm (4) and
BGSAnimationGraph (2); STDT and PNDT carry their star and planet data, name (FULL),
keywords and models in components (orbited data, star data, planet model, full name,
model, Houdini data); SFPT uses block-height adjustment and unique overlay/placement
components; SFBK uses block editor metadata (585).

## Planets, stars and terrain

- **PNDT** (planet, moon, belt, station): master and edited worldspaces with 64-bit
  latitude/longitude, the biomes (`Biomes`, PPBD: biome, chance, resource generation,
  fauna, keywords, flora with its resource and frequency), the surface tree (SFTR), and a
  body block between BDST and BDED with name, body type, space cell, orbital, orbited,
  galaxy (`StarSystemId` equals a star's STDT `SystemId` in all 1,786 records), catalogue, atmosphere and
  biome-noise data and player knowledge flags; then temperature, density, perihelion angle
  and the resource seed. Its display name is the FULL inside the TESFullName component.
- **STDT** (star): name, parsec position, `SystemId`, colour, sun preset (SUNP); the rest
  is in components.
- **SUNP**: either a full reflection blob (REFL) or a parent preset plus differences
  (RFDP + RDIF).
- **BIOM** also lists flora (FLOR), resource generation (RSGD), object and material
  swaps, climate, water, density multipliers and their globals, ambience, music, pattern
  style (PTST), colours, biome type, marker keywords, and per-layer terrain textures and
  noise.
- **SFBK** is one terrain tile; **SFPT** arranges tiles in 16 x 16 grids (stored as
  row-major arrays of 256 cells) with a rotation per cell; **RSGD** gives vein parameters
  per resource for seven rarity tiers on the surface and seven in cells.

## Opaque members, and why

| What | Where | Why |
| ---- | ----- | --- |
| Base form component blocks (BFCB..BFCE) | every type that has them | Upstream reads each block by its component name: a union decided by a string in another subrecord, which the format cannot express yet. Kept as raw subrecords (lossless). FULL, keywords, models, properties and the placement properties inside them are therefore not typed, and the FormID and lstring visitors do not see them |
| Condition comparison value, parameters 1-3, reference | `Condition` (CTDA) | Unions chosen by the type byte, the function index or Run On; 4 raw bytes each. Type, function index and Run On are typed |
| VMAD | LVLP, PKIN, GBFM, TRAV | Script property values are unions by property type |
| OBTS property values | GBFM | The item is typed through `ObteBlock` since M4.BX1; its property values are unions and stay raw |
| NVNM | GBFM | Large nested navmesh geometry; kept raw until navmesh work |
| REFL, RDIF | SUNP | Described by a helper in another upstream unit (`wbDefinitionsReflection.pas`) that is not an authorized source yet (ADR 0004 clarification 2) |
| ONAM, WED0 | STDT | Unknown upstream / a sound reference; never in an official STDT |
| BNAM; GNAM slots 1-4, 6-12, 14, 17-20 | BIOM | Unnamed upstream; BNAM is always `01 00 00 00` |
| PPBD `Data3`, `Data11`; HNAM `Data1`, `Data24`; FNAM `Data5` | PNDT | Unnamed upstream |

Not modelled at all (kept raw in `extra` if a plugin has it): a FULL name inside a GBFM
object-template combination. No official record has one, and leaving it out keeps the doc
layer's signature-based decoder from misreading the FULL inside GBFM name components.

## Things to know before editing

- **Same signature, two layouts.** PNDT has CNAM, FNAM and GNAM both at the top level and
  in the body block with different layouts; PCBN, PKIN and GBFM have signatures in their
  component blocks (NAM1..NAM5, VNAM, CNAM, ANAM) that mean something else at the top
  level. The generated decoder separates them by position. Tools that look a layout up by
  signature alone (`kit inspect`, `where` paths, `kit set` field paths) cannot yet; until
  they can, edit those members by raw payload (`hex:`) and check the result with `kit
  roundtrip --verify`.
- **Stored counts.** `CITC`, `LLCT`, `OBTE`, MOLM's count, PPBD's fauna/keyword/flora/
  trailing counts and the length fields of PNDT's strings are written as stored; change
  them with their lists.
- **PNDT strings** in DNAM, HNAM and KNAM are a 32-bit byte count (including the NUL; 0 for
  an empty string) followed by the bytes, exposed as `...Length` plus a byte array.
- **Raw FormIDs.** Every FormID member is the raw stored value; target types in the
  definitions are upstream's lists. In Starfield.esm they hold for PCCN/PCBN links, PKIN
  CNAM (CELL), BIOM's links, PNDT's surface tree, SFPT's style and worldspaces, SFBK's
  source block, STDT's sun preset and RSPJ's links; PCCC is the one exception above.
  `kit validate` checks them now that they are typed: Starfield.esm has 21 PKIN `PRPS`
  actor values naming AVIF 0x33D/0x33E, which no file defines, and SFBGS050 has one PNDT
  edited-worldspace entry naming a WRLD it does not contain.

## Open questions for a CK observation (lane H)

1. Placement properties: which CK fields map to ZNAM (unnamed upstream; maybe a manual
   days-until-reset value the CK does not save) and how the CK writes the block on a new
   PCCN, PCBN, PKIN or LVLP.
2. Tree links: does the CK ever write PCCB child lists, or only PRTN/PRVN, when a node is
   added, moved or reordered? How does it renumber PRVN chains?
3. PCCN without PCCC (4 records) and PCCC naming a REFR (5): how the CK shows and saves
   them.
4. PKIN's two OPDS slots: which one the CK edits, and where it writes a single OPDS.
5. Record-header flags named only upstream (PKIN Instanced/CalcLpis/InstancedStatic/
   PackInUseOnly/Obstacle, GBFM Unique, STDT StartsHidden/AlwaysDisplayName, LVLP
   ApplyLrtToAllButPivot): which CK checkboxes set them.
6. Unnamed values: BIOM BNAM and GNAM slots, PNDT PPBD `Data3`/`Data11`, HNAM
   `Data1`/`Data24`, orbited `Data5`, TRAV DNAM `Value4`, SFBK NAM1, PKIN FNAM bit 0.
7. RSPJ `NumberCreated` and `Tier` are 0 in every record: what the CK shows for them.
