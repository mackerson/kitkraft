# Free Lanes and Terran Armada: outpost and workshop changes in the files (M5.F5)

This note covers what `SFBGS00D.esm` (Free Lanes) and `SFBGS050.esm` (Terran Armada) add
beside `Starfield.esm`, or override in it, within the outpost and workshop families. It
answers the owner's addendum item 5, "Test interactions with Free Lanes and Terran Armada
outpost changes where applicable", for the data half. FTH-EXP-09 takes its rows from
here. The product names follow `docs/compatibility.md`: Terran Armada is confirmed from
Steam's depots; Free Lanes is confirmed from its own records.

**Scope.** These are records only. Nothing here says how the game behaves at runtime. Raw
payloads (`VMAD` scripts, component blocks, some condition parameters;
`docs/engine-limitations.md` EL-02, EL-03) are not looked into, so a link inside them is
unknown, not absent.

## Inputs and commands

- Masters, read in place (SHA-256 as in `docs/compatibility.md` and `corpus/manifest.toml`):
  `Starfield.esm` `1dabed00...0da7`, `SFBGS00D.esm` `35271d32...da95`, `SFBGS050.esm`
  `0bda63ae...eb9d`.
- Binary: `C:\Modding\kitcraft\bin\kit.exe` (`kit 0.0.0`, SHA-256 `4fa997f8...0672d`),
  commit `5d03d01`, 2026-10-16, with `KITKRAFT_DATA_DIR` set to the game's `Data`.
- Load order for every command: `--plugins Starfield.esm --plugins SFBGS00D.esm --plugins
  SFBGS050.esm`, written `<LO>` below. `SFBGS050.esm` masters `SFBGS00D.esm`, so it can
  also override Free Lanes' own records, and the counts include those.

| Key | Command (read only) |
| --- | ------------------- |
| Q-ALL | `kit --json query <TYPE> --scope all --where "@plugin == 'SFBGS00D.esm' or @plugin == 'SFBGS050.esm'" --limit 1000 <LO>`: every version the two files hold (`--limit 1000`: the default page is 50, and `COBJ` alone has 356; every type below fits one page, `next_cursor` null). A result whose form names its own plugin is an addition; one that names an earlier plugin is an override |
| Q-OP | as Q-ALL, `and (@edid ~ 'Outpost' or @edid ~ 'Workshop' or @edid ~ 'Workbench')` |
| C-T | `kit --json conflicts --plugin <SFBGS00D.esm or SFBGS050.esm> --type <TYPE> --count-classes --limit 1000 <LO>`: each override classed against the version before it |
| I | `kit --json inspect <form> <LO>`: `BNAM`, `CNAM`, `FNAM`, `JNAM`, `KWDA` of a version |
| D | `kit diff Starfield.esm SFBGS00D.esm --form <form>`: member-level difference of one override |
| R | `kit refs <form> --incoming <LO>`: records and placed references that name a form |

Every query below reported `unknown: 0`, meaning every candidate record was decided.
Rechecked 2026-10-16 (fix round W1) with the same binary and `--limit 1000` on Q-ALL and
Q-OP for every type in the table: every count below was reproduced unchanged.

## Counts per type (Q-ALL, C-T)

"Outpost-related" means: for `COBJ`, `BNAM` (the build menu category) is an
`OutpostCategory*` keyword (I on every one of the 356 `COBJ` versions); for `LCTN`, a
`KWDA` that holds `LocTypeOutpost` (`Starfield.esm:0234F1`); for every other type, Q-OP.

| Type | Free Lanes adds | Free Lanes overrides (classes, C-T) | Terran Armada adds | Terran Armada overrides (classes, C-T) | Outpost-related among them |
| ---- | --------------: | ----------------------------------- | -----------------: | -------------------------------------- | -------------------------- |
| `COBJ` | 82 | 12 (11 differ, 1 identical-to-previous) | 262 | 0 | Free Lanes: 6 added, 1 override (identical-to-previous). Terran Armada: 30 added |
| `KYWD` | 244 | 4 (differ: `FULL` of the legendary ranks) | 241 | 0 | 2 added (FL), 15 added (TA); no override |
| `GLOB` | 209 | 0 | 126 | 0 | 6 added (FL), 9 added (TA) |
| `FLST` | 65 | 2 (differ: weapon-mod lists) | 67 | 2 (1 differs, 1 identical-to-previous) | 2 added (FL), 18 added (TA); no override |
| `FURN` | 10 | 1 (differs: `KSIZ`, `KWDA`) | 39 | 0 | 0 (FL), 13 added (TA) |
| `ACTI` | 54 | 0 | 95 | 0 | 1 added (FL), 4 added (TA) |
| `CONT` | 49 | 0 | 37 | 0 | 1 added (FL), 6 added (TA) |
| `STAT` | 116 | 18 (differ: `KSIZ`/`KWDA` in 16, `OBND` in 6, a component block in 1, `REFL` in 1) | 244 | 11 (differ, 2 of them Free Lanes' own forms) | 0 by name. One added `STAT` is built from an outpost recipe (below) |
| `MSTT` | 52 | 1 (differs: a ship colour-swap keyword) | 109 | 0 | 0 |
| `TERM` | 24 | 0 | 31 | 0 | 0 by name. One added `TERM` is built from an outpost recipe (below) |
| `GBFM` | 22 | 0 | 147 | 1 (differs: `OBND`, `PRPS`) | 0 |
| `QUST` | 116 | 0 | 128 | 6 (differ: 4 companion quests, 1 dialogue quest, 1 Free Lanes quest) | 1 added (FL), 10 added (TA) |
| `LCTN` | 120 | 14 (differ) | 78 | 19 (differ, 3 of them Free Lanes' own) | **0** carry `LocTypeOutpost`. The 3 Terran Armada faction outposts are named so but lack the keyword (below) |

**No vanilla outpost record is changed in substance.** Neither file overrides an outpost
keyword, outpost global, outpost form list, outpost location, outpost quest or outpost
workbench of `Starfield.esm`. The one outpost recipe override is identical to the version
before it. Both DLCs extend the outpost system by adding records.

## Overrides that touch outpost or workshop records

| Form | EditorID | Type | By | Change (class) | Command |
| ---- | -------- | ---- | -- | -------------- | ------- |
| `Starfield.esm:039628` | `co_Outpost_Decor_Planters01` | COBJ | Free Lanes | identical-to-previous: no change (BNAM `OutpostCategoryDecorations`, CNAM FLST `OutpostDecorPlantersList`) | C-T COBJ; I |
| `Starfield.esm:1BF552` | `CouchModBStr_Med02EndL` | FURN | Free Lanes | differs: `KSIZ` 5 to 7; `KWDA` gains `SFBGS00D.esm:06590F` `SFBGS00D_CouchMod_Swap_Keyword01` and `Starfield.esm:31987B` `WorkshopItemCenterPlacementOnBounds` | C-T FURN; D; I |
| 16 `STAT`s by Free Lanes, 10 by Terran Armada (for example `Starfield.esm:036500` `OpmExtAddOnSmFenceCorIn01`, `Starfield.esm:042ADD` `OpmExtPodSmFrontWin01`) | - | STAT | both | differs in `KSIZ`/`KWDA`. A `STAT`'s keywords sit inside a component block, which `inspect` keeps raw, so the keywords added cannot be named (EL-02) | C-T STAT; I |

The other overrides are outside the outpost family: legendary weapon mods (`COBJ`,
`KYWD`, `FLST`), companion quests, city and system locations. Free Lanes, for example,
adds `LocTypeSettledSystem` to `Starfield.esm:061521` `SAlgorab`.

## Additions in the outpost family, by what they build

`COBJ`s whose `BNAM` is an outpost category, with the object each one creates (`CNAM`; I
on each). The `FNAM` filters are the same category unless noted.

| Plugin | COBJ | Category (`BNAM`) | Creates (`CNAM`) | Notes |
| ------ | ---- | ----------------- | ---------------- | ----- |
| Free Lanes | `SFBGS00D.esm:02AF79` `SFBGS00D_co_Outpost_Storage_SharedOutpostContainer` | `OutpostCategoryStorage` | CONT `SFBGS00D.esm:02AF77` "Shared Outpost Container" | `JNAM` (max build count) = GLOB `SFBGS00D.esm:02AF7A` `SFBGS00D_Outpost_BuildLimit_Container`. The CONT's `KWDA` holds `SFBGS00D.esm:04D338` `WorkshopStashContainerKeyword` (new) and `Starfield.esm:184462` `IsChestForStatTracking`. R: the keyword is named by this CONT only, and the CONT by this COBJ and by FLST `SFBGS00D.esm:02AF78` `SFBGS00D_ADDTOOutpostDecorContainerList_LargeStorage`. No `VMAD` on either record |
| Free Lanes | `SFBGS00D.esm:02C32D` `SFBGS00D_co_OutpostPI_BreathableGenAirlock01` | `OutpostCategoryStructures` | FLST `SFBGS00D.esm:02C32C` `SFBGS00D_Outpost_BreathableAirlockList` "Airlock" | - |
| Free Lanes | `SFBGS00D.esm:074699` `SFBGS00D_co_Outpost_Display_ColonyWarActionHeroes` | `OutpostCategoryDisplays` | FLST `SFBGS00D.esm:07469E` `SFBGS00D_ColonyWarActionHero_DisplayList` | - |
| Free Lanes | `SFBGS00D.esm:03869C` `co_ShipDecor_ShipUpgradeTerminal` | `OutpostCategoryDisplays` (FNAM `OutpostCategoryMiscellaneous`) | TERM `SFBGS00D.esm:03869D` `ShipUpgrades_Terminal_ShipDecoration_Wall` | - |
| Free Lanes | `SFBGS00D.esm:051485` `SFFL_MS03_co_Outpost_MilliewhalePet` | `OutpostCategoryMiscInterior` | NPC_ `SFBGS00D.esm:053CCE` `SFFL_MS03_OutpostMilliwhale` | Build-limit GLOB `SFBGS00D.esm:051484` `Outpost_BuildLimit_MilliwhalePet` exists (Q-OP GLOB) |
| Free Lanes | `SFBGS00D.esm:07D7BF` `SFFL_MS03_co_Outpost_MilliewhaleBowl` | `OutpostCategoryMiscInterior` | PKIN `SFBGS00D.esm:0921F1` `SFFL_OutpostPI_MilliwhaleBowl` | ACTI `SFBGS00D.esm:071536` "Milliwhale Food Bowl"; GLOB `:092165` `Outpost_BuildLimit_MilliwhaleBowl` |
| Terran Armada | 4: `SFBGS050.esm:01662B`, `:07DC18`, `:07DC19`, `:07DC1A` (`Y2_co_Outpost_Harvester_Solid_X-Tech*`) | `OutpostCategoryHarvesters` | CONT `:01662C`, `:07E613`, `:07E614`, `:07E615` "Extractor - X-Tech" (a plain one and FC, RI and UC variants) | The resource is Free Lanes' MISC `SFBGS00D.esm:033E3D` `Y2_X-Tech_Misc` (keyword `:033E3E` `Y2_X-Tech_Resource_Keyword`). Free Lanes adds GLOB `:089AD1` `OutpostResourceProductionIntervalMfgX-Tech` and an uncategorized recipe `:05F4DB` `Y2_co_Resource_X-Tech` (no `BNAM`) |
| Terran Armada | 12: `SFBGS050.esm:06AF7F`..`:06AF84` (Freestar), `:08374B`..`:08374F` and `:09D4A3` (Crimson Fleet, EditorIDs `SFBGS034_*`) | `OutpostCategoryCrafting` | FURN workbenches: Freestar `:021DF3`, `:067E52`..`:067E56`; Crimson Fleet `:083746`..`:08374A`, `:09D4A2` | No `FNAM` filter on these 12 |
| Terran Armada | 2: `:073D2F`, `:09D4A5` | `OutpostCategoryCrafting` | FLST `:073D2E` `FreestarCollective_OutpostWorkbenchList01`, `:09D4A4` `CrimsonFleet_OutpostWorkbenchList01` | The Crimson Fleet list's `FULL` reads "Freestar Collective Workbenches" (Q-OP FLST) |
| Terran Armada | `:083814` `co_Outpost_Structure_SFBGS037_ElevatedCabin`; `:083815` `..._ElevatedCabin_furniture` | `OutpostCategoryStructures`; `OutpostCategoryFurniture` | PKIN `:08387E` `SFBGS037_ElevatedCabin`; the second has **no `CNAM`** | 15 `SFBGS037_*` FLSTs (foundation, beds, chairs, couch, tables, dining table, cabinet, planters, plants, rugs, pictures, shelves, wall shelves, lights, stool) and a cooking station FURN `:083832` come with it |
| Terran Armada | `:083816`, `:083817`, `:083818` (`SFBGS037` wardrobe, coat hanger, TV) | Displays; Decorations | CONT `:083819`, STAT `:083C74`, PKIN `:083888` | - |
| Terran Armada | 7: `:09F758`..`:09F75D`, `:0A9358` (`co_SFBGS033_Outpost_Decor_CreatureEnclosure_*`) | `OutpostCategoryDisplays` | PKIN creature enclosures `:08ED06`..`:08ED0B`; FLST `:0A9357` | - |

The other added `COBJ`s use the weapon, armor, industrial or ship-building workbench
keywords. Free Lanes adds 53 weapon, 9 armor, 9 ship-building and 1 industrial recipe, plus
3 legendary-reroll recipes and the X-Tech recipe with no `BNAM`. Terran Armada adds 202
weapon, 27 ship-building and 3 armor recipes. They do not belong to the outpost build menu.

## Other outpost additions (Q-OP, R)

| Plugin | Records | What the files show |
| ------ | ------- | ------------------- |
| Free Lanes | QUST `SFBGS00D.esm:0502EC` `SFBGS00D_SharedOutpostContainer_Tutorial` "Container Tutorial"; GLOB `:0502EE` `SFBGS00D_OutpostPlaced_GV` | The shared container has a tutorial quest and a global, by their names |
| Free Lanes | KYWD `:03F3A2` `SFBGS00D_OutpostScienceKit_Swap_Keyword01`; GLOB `:083A37` `SFFL_OutpostMilliwhaleSkin` | Swap and skin data for outpost objects |
| Terran Armada | QUST `SFBGS050.esm:028235`, `:028236`, `:028237` `SFTER_MQOutpostUC`/`FC`/`RI` ("Quid Pro Quo", "Give and Take", "Favors for Favors"), their dialogue quests `:039E9A`..`:039E9C` and actor quests `:09E246`..`:09E248`; 9 GLOBs `SFTER_MQOutpost_Turret*` and `*ObjectivesComplete` | Three faction outposts are quest content. Each has its own worldspace and location: R on `:02B011` `SFTEROutpostFCLocation` finds WRLD `:02B007` `SFTEROutpostFCWorld`, and R on ACTI `:038AC8` `OutpostBeaconActivatorFC` finds one persistent `REFR` `:02B254` placed in that world. The three locations `:02B011`, `:02B061`, `:02B096` do **not** carry `LocTypeOutpost` (the `LCTN` query by keyword returns `Starfield.esm:2A39CC` `OutpostOverlayTemplateLocation` alone) |
| Terran Armada | QUST `SFBGS050.esm:069DFF` `SFTER_INC_SQ_OutpostAttackSpecial_TerranXTech` "Terran Outpost Attack - X-Tech Extractor"; KYWD `:069E00` `SFTER_SQ_OutpostEventAttackSpecial_TerranXTech` | An outpost attack tied to the X-Tech extractor. This is relevant to M5.H1 (hostile encounters) and addendum item 3 ("Compatibility with vanilla outpost assaults and current DLC behavior") |
| Terran Armada | ACTI `:03C751` `SFBGS050_OutpostScanBooster02_TerranTriangulationUnit` "Scan Booster - Advanced"; KYWDs `*_Swap_Keyword` for the beacon activators, the large hex hab, the scan booster and the Aldumite extractor; CONT `:03B6BB` "Extractor - Aldumite"; CONT `:08EA74` `SFTER_MQOutpostCrate` | Outpost object variants added by name. Whether each has a recipe was not checked beyond the `COBJ` table above, which lists none for the scan booster or the Aldumite extractor |

## Candidate rows for FTH-EXP-09

FTH-EXP-09 chooses from these; none is run here.

| # | Change | Why the experiment touches it | Records |
| - | ------ | ----------------------------- | ------- |
| 1 | Shared outpost container (Free Lanes) | The owner's inventory says to respect it and not reimplement global storage (`docs/foothold/engineering-surface.md`, its two shared-container items). Its rows run with and without the container | COBJ `SFBGS00D.esm:02AF79`, CONT `:02AF77`, KYWD `:04D338`, GLOB `:02AF7A`, FLST `:02AF78`, QUST `:0502EC` |
| 2 | Faction outposts without `LocTypeOutpost` (Terran Armada) | A claim or registration check that keys on `LocTypeOutpost` would miss them | LCTN `SFBGS050.esm:02B011`, `:02B061`, `:02B096`; ACTI `:038AC8`..`:038ACA` |
| 3 | X-Tech extractors and production interval | A resource-producer experiment (FTH-EXP-05) built on extractors meets a Free Lanes global and Terran Armada extractors | CONT `SFBGS050.esm:01662C` and variants; GLOB `SFBGS00D.esm:089AD1`; MISC `:033E3D` |
| 4 | Outpost attack on the X-Tech extractor | Interacts with any encounter fixture (M5.H2) | QUST `SFBGS050.esm:069DFF` |
| 5 | Workbenches added to the crafting category | A fixture that adds to `OutpostCategoryCrafting` shares the menu with 14 Terran Armada recipes | COBJ `SFBGS050.esm:06AF7F`..`:06AF84`, `:08374B`..`:08374F`, `:09D4A3` |
| - | The vanilla planter recipe override | Does not apply: identical to the previous version | `Starfield.esm:039628` |

## Limits of this note

- `VMAD` is not read, so the scripts that make the shared container shared are not
  examined here. Papyrus sources inside the DLC archives were not opened either.
- `STAT` keywords inside component blocks are not decoded (EL-02).
- Links inside raw payloads are not found by R (its `not_searched` and "partly searched"
  counts).
- Ship-building, weapon and armor additions are counted, not listed.
