# Items and workshop family: what the schema says, and what it does not

For mod authors who want to read or patch Starfield's placeable objects and items with
KitKraft, and above all who want to add something to the outpost build menu: statics
(STAT), moveable statics (MSTT), activators (ACTI), furniture (FURN), containers (CONT),
terminals and terminal menus (TERM, TMLM), misc items (MISC), armor (ARMO), idle
animations (IDLE), animated objects (ANIO) and camera paths (CPTH). The definitions are in
`crates/schema/defs/<SIG>.toml`; `kit schema --type FURN` prints any of them with sources
and confidence. Recipes (COBJ), form lists (FLST) and keywords (KYWD) belong to another
schema lane; this document reads them from the corpus only to explain the outpost menu.

Everything below was measured on the 14 official ESMs on 2026-10-14. Layouts come from the
pinned xEdit definitions (`wbDefinitionsSF1.pas` and `wbDefinitionsCommon.pas` at
`9fb01688`, ADR 0004); each definition cites its line ranges. "corpus" confidence means
every official record of the type decodes with every subrecord placed in a member, nothing
left over, and encodes back to the same bytes. Members that never occur in an official file
stay at `reference` confidence (documented upstream, not yet seen). Claims about what the
game does with the data are graded as in ADR 0005 Amendment 3: **A** is what the official
files contain (our own scan), **D** is a hypothesis to test in the CK or in game.

## Coverage

| Type | Records | Files | Opaque members | Never seen (`reference`) |
| ---- | ------: | ----: | -------------- | ------------------------ |
| STAT | 22,296 | 6 | VMAD, component blocks, NVNM | VMAD, DNAM leaf pair |
| MISC | 1,495 | 5 | VMAD, component blocks | SNTP, SNBH, CDIX |
| CONT | 866 | 5 | VMAD, component blocks, COED middle slot | object template, COED |
| ACTI | 1,940 | 5 | VMAD, component blocks, NVNM, CTDA unions | DEFL, RADR |
| FURN | 1,045 | 8 | VMAD, component blocks, NNAM | NTRM, WTFM, container items, NAM1 |
| MSTT | 2,134 | 8 | VMAD, component blocks | second PTT2 slot |
| TERM | 501 | 5 | VMAD, REFL (effect sequence component), WBDT | OPDS, SNBH, DEFL, XALG, destruction, NTRM, ALSH, ACSH, WTFM, ATTX, RADR, conditions, STRV, object template |
| TMLM | 974 | 5 | VMAD, CTDA unions, ISET/XLOC padding | - |
| ARMO | 1,168 | 5 | VMAD, component blocks, Wwise GUIDs | MO2S, MLM4, MO4S, destruction, ETYP, BIDS |
| IDLE | 1,302 | 5 | CTDA unions | - |
| ANIO | 66 | 2 | VMAD, component blocks | VMAD |
| CPTH | 152 | 3 | CTDA unions | - |

All 33,939 records decode and re-encode byte-identically, with every subrecord placed in a
member and `Record::extra()` empty for every record (out-of-tree check over the generated
decoder, all 14 files; no record of these types is compressed). Every other member is
typed: FormIDs carry their target types (raw, file-relative values; resolution is the doc
layer's), flags and enumerations carry the names the pinned file gives (placeholder names
upstream, such as "Unknown 3", are left unnamed and the bit is preserved), localized strings
agree with the corpus lstring table (`ATTX` on ACTI and FURN, `NNAM` on MISC, `INAM`,
`BTXT`, `ITXT`, `ISTX` and `UNAM` on TMLM, plus `FULL`/`DESC`).

New shared definitions (`defs/common/`): `ContainerItems` (COCT, CNTO, COED; NPC_ and the
quest alias inventory use it too since the integrator's ITEMS merge), `RadioReceiver`
(RADR), `FurnitureMarkerFlags` (MNAM), `FurnitureMarkerParameter` (one SNAM element) and
`FurnitureEntryPoint` (FNPR). The family also wrote its own model, destructible, sound
reference, object property, colour and object template blocks; at the ITEMS merge (M4.BX1)
the integrator folded them into the existing `Model` (MOLM became repeated: one STAT has
two), `Destructible`, `SoundReference`, `PropertyValue` (one PRPS element), `ColorRgb` and
`ObteBlock` (OBTE, combinations with a typed OBTS template item, STOP; ARMO, CONT and TERM
reference it, and their STOP is its end marker). `ObjectBounds`, `ObjectPaletteDefaults`,
`Transforms`, `Keywords`, `Condition` and `BfcbBlock` are reused from earlier lanes.

## The outpost build menu, from the official files

Starfield's outpost menu is data, and nearly all of it sits in records this lane or lane
BR types: the recipe (COBJ), the object it creates, and a handful of keywords and form
lists. What follows is grade A (what the files contain); how the game uses it is marked
where it is a hypothesis.

### Recipes are filed by their workbench keyword

Every COBJ names a workbench keyword in BNAM. 596 of the 3,518 official recipes use one of
the outpost category keywords, all of keyword type 9 ("recipe filter", KYWD TNAM):

| BNAM (category) | Recipes | | BNAM (category) | Recipes |
| --------------- | ------: | - | --------------- | ------: |
| OutpostCategoryHarvesters | 208 | | OutpostCategoryMiscellaneous | 13 |
| OutpostCategoryFurniture | 139 | | OutpostCategoryBuilders | 12 |
| OutpostCategoryDecorations | 88 | | OutpostCategoryPower | 11 |
| OutpostCategoryDisplays | 46 | | OutpostCategoryRobots | 8 |
| OutpostCategoryCrafting | 20 | | OutpostCategoryDefenses | 7 |
| OutpostCategoryStructures | 19 | | OutpostCategoryMiscInterior | 4 |
| OutpostCategoryStorage | 17 | | OutpostCategoryTestAssets | 3 |
| | | | OutpostCategoryQuest | 1 |

The other recipes belong to the weapon bench (1,412), the ship builder (806), the armor,
industrial, cooking and chem benches, or name no workbench (488, research and other
recipes). Facts shared by all 596 outpost recipes: TNAM (tier) is 0 and LRNM (learn
method) is 3; RECF is 0 in 587 (3 in 8, 2 in 1). 582 list component costs (FVPA); 345
have conditions; NNAM (amount produced) is 1 in 296 and 0 in 300. 224 carry an FNAM recipe
filter of one keyword, the same as BNAM in 217 (one of the others is
`OutpostCategoryBulldoze`). 54 cap the count per outpost with a global in JNAM
(`Outpost_BuildLimit_Mannequins` 23, `_Robots` 8, `_Turrets` 7, `_LandingPad` 4,
`_MemorialEasel` 4, `_CargoLinks` 2, and single ones for crew stations, the transfer
container and others).

### The category list and the beacon

- The form list `OutpostRecipeCategories` holds the 15 category keywords; the list
  `OutpostFollowCameraCategoryList` holds six of them plus two recipes.
- The outpost beacon activators (`OutpostBeaconActivator` and its faction variants FC, RI,
  UC; ACTI, Starfield.esm) carry the category keywords in their own keyword list;
  `OutpostPlayerHouseActivator` and `SFBGS006_SBShipDecorationMarker` carry some of them
  too. The beacon's list is
  `MustPersist`, `WorkshopKeyword`, all 15 categories, `OutpostCategoryPower_InteriorLights`
  (a sixteenth recipe-filter keyword no official recipe uses as BNAM) and
  `WorkshopNoStackingOn`; its properties (PRPS) are the actor values
  `OutpostBuildAreaRadius` and `OutpostMainPackinItem`. It also has a snap template
  (`OutpostSnap_OutpostBeacon`), a forced location reference type
  (`OutpostBeacon_RefType`) and a script.
- Hypothesis (grade D): the menu shows the categories the beacon's keywords name, ordered
  by `OutpostRecipeCategories`, and lists in each the recipes whose BNAM is that keyword.
  To be confirmed in game (M4.O4) before a new category is attempted.

### What a recipe creates

The 596 outpost recipes create: CONT 179 (harvesters, storage, display cases), FLST 145,
FURN 93, PKIN 66, STAT 63, NPC_ 38 (mannequins and robots among them), ACTI 5, MSTT 3, TERM 2,
AACT 1, and one recipe whose created object is null.

- **A form list as the created object** is a variant picker: the list's entries are the
  objects (or further recipes) the player can cycle through. `OutpostFurnitureBedList02_Beds`
  holds eight COBJ, six FURN and a PKIN; `OutpostDecorPosterList` nine COBJ and 18 PKIN;
  most decoration lists hold PKIN or STAT only. A COBJ inside a list is a nested recipe.
- **A pack-in (PKIN)** is used for anything multi-part: habs and hallways
  (`OutpostPI_OPK_OpiHabFourWall01`), reactors, sculptures, signs.
- **Foundation lists**: 33 lists named `OutpostFoundation*List` (Starfield.esm) each pair a
  foundation STAT with its `_Mid01` variant (harvesters, habs, builders, wind turbines). No record of the
  types this lane scanned names them; what selects a foundation is open (CK question 7).

### What the created objects have in common

Over the objects outpost recipes create directly (63 STAT, 3 MSTT, 93 FURN, 5 ACTI, 179
CONT, 2 TERM):

- **PTT2 (`Transforms`) is present on every one**, with the `WorkshopIcon` slot set to a
  TRNS record. PTT2 is otherwise optional (and on MISC it is present but its workshop slot is
  always null). Hypothesis (grade D): the workshop icon transform is what the build menu
  uses to frame the preview; a new buildable object should carry one (copy the vanilla
  object's).
- **Snap data**: SNTP (snap template, STMP) on 178 of 179 containers, all 5 activators and
  12 statics; SNBH (snap behaviour, STBH) on 170 containers. Harvesters snap to their
  foundations through these.
- **Keywords** (top-level KWDA, or for STAT the keyword component): `WorkshopNoStackingOn`
  (170 CONT, 18 FURN, 4 ACTI), `OutpostShowQuickMenu` (170 CONT, 4 ACTI),
  `WorkshopItemCenterPlacementOnBounds` (7 STAT, 4 FURN); harvesters add
  `OutpostCategoryHarvesters`, `ContainerTakeOnly`, `CrewFurniture_Geology` and a
  `ResourceTypeSolid`/`Gas`/`Liquid`; crafting furniture adds `AnimFurnWorkbench`,
  `WorkbenchGeneralKeyword` and the bench keyword (`WorkbenchWeaponKeyword`,
  `WorkbenchResearchKeyword`, ...).
- **Record flags**: `NavmeshOnlyCut` (0x10000000) is set on all 93 furniture and on 16 of
  the statics; `NonOccluder` and `PackInUseOnly` (0x10, 0x2000) on 43 furniture. No other
  flag is common.
- **Furniture**: `WorkbenchData.BenchType` (WBDT) is None in 75, Alchemy (cooking,
  industrial and pharmaceutical benches) in 9, Weapons, Armor and Research in 3 each; every
  one has markers (MNAM, SNAM) and a furniture template is common (FTMP, 33).
- **Containers**: every one has a native terminal (NTRM, a TMLM), properties (PRPS) and the
  STOP marker; 157 have a destruction block, 157 attach-parent slots (APPR).
- **Components**: harvesters and builders carry `BGSAnimationGraph_Component` (170 CONT, all
  5 ACTI); furniture `BGSEffectSequenceComponent` and `BGSLodOwner_Component` (64 each);
  statics the keyword component (one also a LOD owner). No component name is specific to outposts
  (`BGSWorkshopItemColor` occurs once, on a STAT that no outpost recipe creates).

### Leftovers that are not the outpost menu

- 61 keywords named `WorkshopRecipeFilter*` (Fallout 4's settlement menu tree:
  Building, Decor, Furniture, Resource, Vendor and their sub-filters) and the form lists
  `WorkshopMenu01Build02Metal`, `...Wood`, `...Concrete`, `...Fences` and
  `WorkshopMenu01Decor02WallDecorations` exist, but only one official recipe names one of
  these keywords (`WorkshopRecipeFilterResource02Power03Wire`). Do not build on them.
- `WKMF` is "Wwise Keyword Mapping" (sound selection by keyword for ship engines, cockpits,
  grav drives and creature animation), not a workshop record.
- 2,404 STAT carry `OutpostAllowBulldoze` in their keyword component: these are the
  environment objects the outpost bulldozer may remove, not buildable objects.

### What a new buildable object needs (to be confirmed in game)

This is the recipe M4.E7 turns into a spec and M4.O4 checks in game; every step is grade A
for "the vanilla records look like this" and grade D for "this is enough":

1. A base object: clone a vanilla buildable of the same kind (a harvester CONT, a FURN, a
   STAT) so that its model, bounds, snap template and behaviour, PTT2 transforms (with the
   workshop icon), keywords and record flags come along. Give it a new EditorID and name.
2. A COBJ: BNAM one of the 15 `OutpostCategory*` keywords; CNAM the new object; FVPA the
   resource cost; NNAM as the vanilla recipe it copies (0 and 1 are equally common); SNAM a
   menu priority; TNAM 0; LRNM 3; DATA a value; RECF 0;
   optionally the same keyword in FNAM, a JNAM build limit and conditions. Copying a
   vanilla outpost recipe and changing CNAM is the safe start.
3. For several variants under one menu entry, a FLST of the objects (or of their recipes)
   as the COBJ's created object.
4. Nothing registers the recipe anywhere else in the official data (no list of outpost
   recipes exists): a recipe is found by its BNAM keyword. If the in-game check shows the
   new recipe missing, the first suspects are the beacon's category keywords and anything
   the beacon's script filters on (read with `papyrus.events`, never changed).

Records this needs that are not typed yet: COBJ (lane BR, round 3), FLST and KYWD (lane BR),
PKIN (planet family, typed). Ownership of the outpost, power, cargo links and crew
assignments is script and keyword behaviour (`LinkOutpost*`, `CrewFurniture_*`,
`WorkshopStartPoweredOn`), not covered here.

## TERM (lane D, 2026-10-15)

Checked member by member against `xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L18155-L18249`: order,
widths, FormID targets, the FNAM and MNAM bits and the condition block match. Gaps filled:
the component data of every official terminal is typed now (the animation graph ANAM, BNAM,
CNAM strings, `#L6079-L6087`; the effect sequence's REFL kept as bytes, `#L6820-L6827`), and
condition parameters are typed through the shared `Condition` unions. Still opaque: VMAD
(fragmented script data, `wbVMADFragmentedPERK`) and WBDT (unused and zero-length
upstream, one zero byte in every official record). Every official TERM decodes with
nothing raw and re-encodes identically (`crates/doc/tests/authoring_types.rs`); typed
write and reference integrity (the terminal menu DNAM, condition FormIDs):
`crates/engine/src/handlers/authoring_tests.rs`. A TERM with an animation graph component
has two DNAM, the component's response file first: name the terminal menu `DNAM[1]` when
both are present.

Shared with the other types (lane D, 2026-10-15): the base form component block types four
component layouts (`common/BfcbBlock.toml`: animation graph `xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L6079-L6087`,
contact shadow `#L6096-L6105`, placement properties `#L6179-L6198`, reflection `#L6820-L6827`,
all `corpus`, `crates/doc/tests/components.rs`), and the condition's comparison value,
parameters and reference are unions (`common/Condition.toml`, `#L4750-L4840` with the
deciders `#L828-L857`, `#L859-L880` and `wbDefinitionsCommon.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L5738-L5779`, `corpus`
by `validate_corpus_official_files`). Support depth: `docs/schema/support-matrix.md`.

## Opaque members, and why

| What | Where | Why |
| ---- | ----- | --- |
| VMAD (script data) | every type that has it | Property values are unions by property type; TERM and TMLM use fragment variants. Kept raw. |
| Base form component blocks (BFCB..BFCE) | STAT, MISC, CONT, ACTI, FURN, MSTT, ARMO, ANIO | Each component has its own layout, chosen by the name in BFCB. Since lane D (2026-10-15) the animation graph, contact shadow, placement properties and reflection layouts are typed in the shared block (the reflection's REFL stays raw); the others are kept as raw subrecords. A STAT's keywords live here. |
| NVNM (navmesh geometry) | STAT, ACTI | Large nested geometry; kept raw. |
| CTDA parameters 1 and 2 of reference, actor and package parameters | ACTI, TMLM, IDLE, CPTH, TERM | Selected by the type byte's alias and packdata bits (`common/Condition.toml`); the comparison value, the other parameters and the reference are typed unions since lane D (2026-10-15). |
| COED middle slot | CONT (never seen) | A global FormID or a faction rank depending on the owner's type. |
| OBTS property values (object template item) | ARMO (CONT, TERM never) | The item is typed through `ObteBlock` since M4.BX1; since lane B round 4 each property's function type and values are struct-field unions by value type (`common/ObjectModProperty.toml`), typed. |
| NNAM (marker files) | FURN | Pairs of NUL-terminated strings, which an array element cannot hold. |
| WBDT | TERM | Declared unused and zero-length upstream; one zero byte in every official record. |
| ISET and XLOC padding | TMLM | Filler bytes with non-zero values (AB AB, AD DE); kept raw so they roundtrip. |
| Wwise GUIDs | sound slots, ARMO AVSG/AFSG | 16 raw bytes each; nothing in them needs typing. |

Also not typed: BO64 on ARMO is a plain `u64` (64 biped slot bits, names not transcribed);
XALG is a plain `u64` everywhere (its bit names are marked unverified upstream).

Members with positional names because upstream leaves them unnamed: STAT
`DirectionMaterial.Value2`, TMLM `Inam` (a localized string) and `MenuItem.Lock.Value6`.

## Open questions for the Creation Kit (lane H)

1. COBJ: which CK field writes BNAM and which FNAM for an outpost recipe, and does the build
   menu read the category from BNAM, FNAM or both (224 of 596 outpost recipes carry both)?
2. Is a PTT2 with a workshop-icon transform required for an object to appear or preview in
   the outpost menu? What does the CK write for a new object without one?
3. Which CK checkbox sets record flag 0x10000000 on outpost furniture (upstream: navmesh
   "only cut")?
4. Which CK fields set FURN FNAM bit 0x1, CONT DATA bit 0x8 (205 records), MNAM bit
   0x20000000, CPTH DATA bit 0x8, IDLE FNAM bit 0x10, and TERM's one-byte WBDT?
5. What the CK calls TMLM INAM, TMLM XLOC's last four bytes and STAT DNAM's second float
   (and why MaxAngle is 1.0, outside upstream's 30-120 range).
6. Does the CK write ARMO's FLLD and XFLG inside the world models as the corpus shows?
7. What references the `OutpostFoundation*List` form lists (foundation selection)?
8. In game (M4.O4): does a new recipe with a vanilla `OutpostCategory*` BNAM appear on
   every beacon, and does a new category keyword added to the beacon and to
   `OutpostRecipeCategories` create a new tab?
