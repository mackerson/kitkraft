# Base form component blocks: name-keyed decider or lead-signature groups

Design note for KK-281 (2026-11-15). It compares two ways of typing the data of the base form
component blocks (`BFCB` .. `BFCE`, `crates/schema/defs/common/BfcbBlock.toml`) and ends with
one recommendation. Option (a) is a schema-format change and is the owner's to approve:
its draft is [ADR 0015](../../plan/adr/0015-component-name-decider.md) (accepted by the owner on 2026-11-15, after first being
deferred the same day: built for the nine `DATA`/`DAT2` kinds from KK-293 on, while the
lead-signature groups below stay). Nothing here builds it.

Sources: the pinned xEdit file `wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e`
(the component list L5984-L6832, the layouts L6055-L6828, the two name deciders L1479-L1551),
read in place and described in our own words (ADR 0004); and a census of every block of the 14
official files (`docs/corpus.md` order, 2026-11-15, a throwaway walk over the same
`visit_file` + `typed::placements` path `crates/doc/tests/components.rs` uses; not committed).

## What a block is

A block is `BFCB` (the component's name, a NUL-terminated string), the component's data (a
run of subrecords), and an empty `BFCE`. Upstream reads the data as a record union over 42
layouts: for all but two of them the layout is chosen by the data's first subrecord. The two
exceptions are layouts whose only subrecord is `DATA` or `DAT2`: both are one subrecord whose
inner struct is chosen by the component name (L1479-L1517 for `DATA`, six names; L1519-L1551
for `DAT2`, three names), and a name neither decider lists keeps the bytes unread.

KitKraft today follows the record union for eight layouts: each is an optional group of the
block, matched by the decoder in member order, and a group is entered at any of its leading
members up to its first required one (`starts` in the generated matcher; `leads` in
`crates/engine/src/edit/scaffold.rs`). Everything else falls into `ComponentData`, the
`any_until = "BFCE"` member, and is kept as read.

## The corpus

349,154 blocks of 64 component names, in records of 70 defined types (every record type that
carries a block has a definition). By what upstream does with them:

| Class | Kinds | Blocks | Names (blocks) |
| ----- | ----: | -----: | -------------- |
| Empty in every official block (nothing to type: `BFCB` then `BFCE`) | 9 | 6,250 | PathingData 2,033, PapyrusScripts 2,030, MagicTargetForm 2,030, Scannable 126, Primitive 24, SkinForm 4, ShipManagement 1, VehicleManagement 1, WorkshopItemColor 1 |
| Reflection payloads (`REFL`, `PTCL`, `PCCC`: self-describing, opaque under ADR 0004) | 9 | 14,209 | LodOwner 5,784, EffectSequence 3,144, ParticleSystem 2,733, HoudiniData 1,825, ReflectionProbes 388, MaterialProperty 177, LensFlareAttachment 95, LightAttachment 58, SpacePhysics 5 |
| `DATA` / `DAT2` bodies: the inner struct is chosen by name | 9 | 1,197 | BlockEditorMetaData 585 (`DAT2`), BlockHeightAdjustment 255 (`DAT2`), OrbitedData 123, StarData 123, UniqueOverlayList 38, UniquePatternPlacementInfo 38, SurfaceTreePatternSwapInfo 33 (`DAT2`), BlockCellHeighGrid 1 (no decider entry), OverlayDesignatedPlacementInfo 1 |
| Ordinary subrecords, layout chosen by the first subrecord | 37 | 327,498 | the rest, below |

The 37 ordinary kinds, most frequent first, with the blocks that hold data (the others are
empty), the data's first subrecord in the corpus, and the upstream layout that reads them:

| Kind | Blocks | With data | Lead | Upstream layout | Typed |
| ---- | -----: | --------: | ---- | --------------- | ----- |
| ContactShadowComponent | 147,487 | 147,487 | `FLCS` | L6096-L6105 | yes |
| KeywordForm | 95,999 | 83,015 | `KSIZ` | Keywords, L6694-L6697 | KK-282 |
| Volumes | 23,159 | 23,159 | `VLMS` | L6636-L6677 | no (`VLMS` tail width chosen by the entry type) |
| ObjectPaletteDefaults | 6,226 | 6,105 | `OPDS` | L6170-L6173 | KK-283 |
| FullName | 5,775 | 5,695 | `FULL` | L6611-L6614 | KK-284 |
| AnimationGraph | 4,854 | 4,853 | `ANAM` (2 `BNAM`) | L6079-L6087 | yes |
| TESModel | 3,935 | 3,935 | `MODL` 2,067, `FLLD` 1,868 | PlanetModel L6619-L6635 / FLLD L6682-L6685 | KK-285 |
| PropertySheet | 3,876 | 2,064 | `PRPS` | L6199-L6202 | KK-286 |
| AttachParentArray | 3,321 | 2,396 | `APPR` | L6088-L6091 | KK-287 |
| ObjectWindowFilter | 3,062 | 3,062 | `INTV` | L6174-L6178 | KK-288 |
| DestructibleObject | 2,813 | 2,149 | `DEST` | L6118-L6121 | KK-289 |
| FormLinkData | 2,334 | 2,334 | `ITMC` | L6158-L6165 | 2026-11-16 |
| ExternalComponentDataSource | 2,030 | 2,030 | `EXDC` | L6140-L6157 | 2026-11-16 |
| LinkedVoiceType | 2,030 | 2,030 | `FCTF` | L6166-L6169 | 2026-11-16 |
| SpaceshipAIActor | 2,030 | 2,030 | `SAIA` | L6211-L6214 | 2026-11-16 |
| SpaceshipEquipment | 2,030 | 2,030 | `ESSF` | L6215-L6218 | 2026-11-16 |
| SpaceshipWeaponBindings | 2,030 | 2,030 | `SHWB` | L6223-L6230 | 2026-11-16 |
| Blueprint | 2,030 | 2,030 | `BUO4` | L6561-L6602 | 2026-11-16 |
| ForcedLocRefType | 2,022 | 14 | `FTYP` | L6686-L6689 | 2026-11-16 |
| TESContainer | 2,021 | 83 | `COCT` | Container Items, L6678-L6681 | 2026-11-16 (no `COED`) |
| TESPlanetModel | 1,825 | 1,825 | `MODL` | L6619-L6635 | KK-285 (`XMPM` opaque) |
| AddToInventoryOnDestroy | 1,724 | 1,702 | `INAM` | L6075-L6078 | 2026-11-16 |
| PlanetContentManagerContentProperties | 1,272 | 1,272 | `ZNAM` | L6179-L6198 | yes |
| StoredTraversals | 1,258 | 1,258 | `STRD` | L6235-L6258 | no (traversal FormID present by a flag bit; half floats) |
| SoundTag | 987 | 966 | `STCP` | L6207-L6210 | yes |
| WorldSpaceOverlay | 582 | 582 | `SNAM` | L6554-L6560 | 2026-11-16 |
| QualityUpgrade | 216 | 216 | `QUPA` | L6203-L6206 | yes |
| DisplayCase | 156 | 156 | `DCSD` | L6122-L6139 | yes |
| ImageSpaceModifiableForm | 142 | 50 | `MNAM` | L6615-L6618 | 2026-11-16 |
| SpaceshipHullCode | 134 | 134 | `HULL` | L6219-L6222 | 2026-11-16 |
| SpawnOnDestroy | 75 | 75 | `SODA` | L6231-L6234 | 2026-11-16 |
| CrowdComponent | 24 | 24 | `CDND` | L6106-L6117 | 2026-11-16 (after PropertySheet) |
| AdaptiveTriggerData | 19 | 19 | `WFIR` | L6060-L6074 | 2026-11-16 |
| ActivityTracker | 10 | 10 | `ATCP` | L6055-L6059 (helper L5645-L5672) | no (configuration strings carry FormIDs) |
| BodyPartInfo | 4 | 1 | `GNAM` | GNAM (unknown bytes), L6690-L6693 | 2026-11-16 (opaque) |
| CityMapsUsage | 4 | 4 | `MOD2` | L6092-L6095 | 2026-11-16 |
| VehicleConfig | 2 | 2 | `VCSB` | L6259-L6553 | no (two blocks; length-prefixed strings, WWise GUIDs) |

("Typed" as of 2026-11-15: KK-282..KK-289 took the nine most frequent kinds option (b)
reaches without a format change, Volumes aside; a date marks the kinds typed on 2026-11-16,
see the last update. Kind names drop the `BGS`/`TES` prefixes and `_Component` suffixes where
that stays unambiguous; the census prints the full strings.)

So the scoping claim that 55 kinds "need the name decider" (`docs/gaps.md`, EL-02 before this
note) is wrong: 9 kinds need it to tell their layouts apart; the other 46 non-reflection
kinds are either empty or chosen by a first subrecord no other layout leads with.

## Ambiguity under the lead-signature approach

A lead collides when two layouts can start with the same signature, so the decoder would read
one as the other. In the corpus:

- **`DATA` (6 names, 324 blocks) and `DAT2` (3 names, 873 blocks): real.** One subrecord,
  several inner layouts, the name decides. The lead-signature approach can at most type the
  subrecord as opaque.
- **`REFL` (7 names, 9,651 blocks): harmless.** One opaque layout under every name.
- **`MODL` (TESModel 2,067, TESPlanetModel 1,825): harmless.** Upstream reads both with the
  planet-model layout, whose members after `MODL` are all optional; a TESModel block is that
  layout without `XMPM` and the ring strings.
- **`FLLD` (TESModel 1,868 blocks hold only `FLLD`):** upstream's one-subrecord FLLD layout.
  In KitKraft's sequential matcher the planet-model group (first member optional) would also
  accept a leading `FLLD`, so the FLLD group must come before it in member order; then a
  `MODL`-led block still reaches the planet-model group and keeps its own `FLLD`.

Latent collisions (no official block shows them, but a plugin could):

- **`BNAM`**: AnimationGraph accepts any of its members first (2 official blocks start at
  `BNAM`), and WorldSpaceOverlay's third member is `BNAM`. A WorldSpaceOverlay block without
  `SNAM` and `PNAM` would be read as an animation graph.
- **`PRPS`**: CrowdComponent's population entries start with a `PRPS` of another layout
  (actor, value, curve) and its `CDND`/`CDNS` are optional upstream, so a Crowd group placed
  before PropertySheet (upstream order) would take every `PRPS`-led property sheet. Typing
  Crowd needs its lead made required or a later position: a choice upstream does not make.
- **`CITC`/`CTDA`**: the PlanetContentProperties group, all of whose members are optional,
  accepts a leading `CITC` or `CTDA`. Any later layout led by them (none upstream) would lose.

Upstream's own record union has the same latent cases; the lead-signature approach copies
its behaviour, it does not add new ones.

Layouts whose lead is unique but whose body needs other format work, whichever option is
taken: Volumes (`VLMS`: an array of entries whose tail width depends on the entry's type,
1, 2 or 3 floats; a field union must have one width today), StoredTraversals (nested
traversal arrays), VehicleConfig (length-prefixed strings, sound references, WWise GUIDs),
ActivityTracker (configuration strings that carry FormIDs), and the planet model's `XMPM`
(u16-counted arrays of u16-length strings).

## (a) A name-keyed decider

A new member kind for the block: a union of groups whose variant is chosen by the payload of
an earlier sibling subrecord (`BFCB`), each variant naming the component strings it reads,
with the raw `ComponentData` as the fallback for a name no variant lists or a body the
chosen variant does not match. The ADR draft has the shape.

- **Kinds it can type**: all 46 non-empty, non-reflection kinds by name, including the nine
  `DATA`/`DAT2` kinds (1,197 blocks) the other option cannot, and every name gets its own
  variant (TESModel apart from TESPlanetModel, each REFL name apart). The body work listed
  above is still separate.
- **Ambiguity**: none by construction for the official names. A block whose name says one
  layout and whose data is another (none in the corpus) needs a rule: raw fallback (lossless,
  unlike upstream, which reads such data by its first subrecord).
- **Byte-identical roundtrip**: kept if the fallback is raw and the name is never rewritten
  by the decoder. The new risk is a fallback path that drops or reorders subrecords; every
  official block would need the `components.rs` check per variant, as today.
- **Generated code**: the matcher (`match_members`, generated `records.rs`) matches by
  signature only. A name decider makes matching look at an earlier subrecord's payload:
  a new `MemberDesc` variant, a payload-aware `match_one`, a new `fill` arm, the golden
  seed, and `kitkraft_doc::typed::placements`, which takes signatures only today
  (`&[[u8; 4]]`) and would need payloads (or the block's names) passed in.
- **Edit paths**: `kitkraft_engine::edit::fields::layout_at`/`layouts` and
  `scaffold::record_order` decide a subrecord's layout from signatures; they would take the
  block's name instead, which is better for `new` (the component name given in `fields`
  selects the layout explicitly) but changes the signature of every caller, the coverage
  matrix walk, and `validate`'s placement.
- **Work size**: large. Schema model and loader, codegen and the matcher runtime, the doc
  placement API, engine layout resolution and scaffolding, their tests, the golden seed and
  parity; then every layout still has to be written as today. Several tickets, and an ADR
  because it changes the format (ADR 0003).

## (b) Continue lead-signature optional groups

Add one optional group per layout to `BfcbBlock.toml`, in upstream order except where the
sequential matcher needs an earlier position (the FLLD layout before the planet model),
each from the pinned file, each checked over every official block by `components.rs`.

- **Kinds it can type**: the 37 ordinary kinds whose lead is unique, as far as the format
  reaches their bodies. Without any format change: KeywordForm, ObjectPaletteDefaults,
  FullName, TESModel and TESPlanetModel (with `XMPM` opaque), PropertySheet,
  AttachParentArray, ObjectWindowFilter, DestructibleObject (KK-289, once the shared
  Destructible group's six `reference` members had corpus evidence), FormLinkData,
  ExternalComponentDataSource, LinkedVoiceType, SpaceshipAIActor, SpaceshipEquipment,
  SpaceshipWeaponBindings, Blueprint, ForcedLocRefType, TESContainer,
  AddToInventoryOnDestroy, WorldSpaceOverlay, ImageSpaceModifiableForm, SpaceshipHullCode,
  SpawnOnDestroy, AdaptiveTriggerData and CityMapsUsage: 127,423 blocks with data, on top
  of the 154,950 the six non-opaque layouts type today (and the 12,384 `REFL`/`PTCL`
  blocks held as opaque groups). Not the nine `DATA`/`DAT2` kinds (opaque at best), and not
  Crowd without a lead choice upstream does not make.
- **Ambiguity**: as listed above. The `components.rs` check keeps the corpus honest: a
  subrecord read into a layout under a name upstream does not give that layout fails it.
- **Byte-identical roundtrip**: unchanged mechanism (a group emits what it read in order;
  anything else stays in `ComponentData`). Each new group is checked the same way.
- **Generated code**: one struct per layout and one `fill`/`emit` arm in `BfcbBlock`; no
  runtime change. Every record type with a block grows by those members (the golden seed and
  `docs/record-parity.md` regenerate).
- **Edit paths**: unchanged. `record_order` already picks the layout the fields' signatures
  select (KK-220), `set` resolves positions with `layout_at`, and the typed-write helper
  (`authoring_tests.rs` `writes_and_reads_back`) takes a layout and a component name, so each
  layout gets a write test for the price of one call. A component signature the record also
  has at top level (`KSIZ`/`KWDA`, `FULL`, `MODL`, `PRPS`, `DEST`, ...) keeps its own slots
  first; only occurrences past them select the layout. One edit-path ambiguity showed up (KK-289): a signature that several
  layouts carry (`FLLD` in LightLayer, PlanetModel and Destructible's stages) selected the
  first of them in member order, even one it cannot start; since KK-289
  `record_order` prefers a layout the given signatures can start.
- **Work size**: small per layout (TOML, codegen, golden, parity, one corpus count, one test
  call); no ADR.

## Recommendation

**(b)**, now: it tells apart every layout except the `DATA`/`DAT2` ones, which hold 1,197 of
the 349,154 official blocks (0.34 %), it mirrors upstream's own record union, costs no format
or API change, and keeps each step independently checkable against the corpus (the bodies
that need other format work need it under (a) too). Revisit (a) when the
`DATA`/`DAT2` payloads (star and orbit data, block editor metadata, surface pattern data) are
needed by a feature, or when a mod is found whose block name and data disagree; ADR 0015
records the design for that day.

**Update (2026-11-15, KK-292..KK-295):** the owner accepted ADR 0015 the same day. The
`NamedLayout` subrecord union now types eight of the nine `DATA`/`DAT2` kinds (1,196 blocks;
`BGSBlockCellHeighGrid_Component` has no upstream decider entry and stays raw), and the
lead-signature groups above stay as they are.

**Update (2026-11-16):** nineteen more kinds are typed as lead-signature groups, each checked
over every official block by `component_layouts_full` (0 re-encode differences): the GBFM
ship parts (LinkedVoiceType, SpaceshipAIActor, SpaceshipEquipment, SpaceshipHullCode,
SpaceshipWeaponBindings), ExternalComponentDataSource, FormLinkData, Blueprint, HoudiniData
(its `PCCC` opaque, as `PTCL`), AddToInventoryOnDestroy, SpawnOnDestroy, WorldSpaceOverlay,
ImageSpaceModifiableForm, ForcedLocRefType, TESContainer (without `COED`: no official block
has one, and a `reference` member would lower every type with components), AdaptiveTriggerData,
CityMapsUsage, BodyPartInfo (its `GNAM` opaque, unknown upstream) and CrowdComponent, placed
after PropertySheet so it cannot take a `PRPS`-led property sheet (a crowd block that starts
with `PRPS`, none official, is read as a property sheet). 19,027 blocks; with them 44
layouts type or hold 50 of the 64 kinds and 297,803 of the 349,154 blocks (14,210 of them
opaque: `REFL`, `PTCL`, `PCCC`, `GNAM`); 26,921 are empty. 24,430 blocks in five kinds stay
raw: Volumes 23,159 (each `VLMS` entry ends in one, two or three floats chosen by its type,
and a field union must have one width), StoredTraversals 1,258 (each traversal's FormID is
present only when a flag bit of the same entry is clear, and its four half floats have no
type), ActivityTracker 10 (configuration strings that carry FormIDs), VehicleConfig 2 and
`BGSBlockCellHeighGrid_Component` 1 (no upstream decider entry). `components.rs` prints the
raw blocks per kind (the census) and, with `KITKRAFT_COMPONENT_SHAPES`, their signature runs.

