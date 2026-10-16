# FTH-EXP-01 prototype: authored exterior site with an outpost beacon

Serves **FTH-EXP-01** (authored-POI workshop; `plan/execution-plan.md`, Foothold
experiments; `docs/foothold/engineering-surface.md` B1, C1 and J). This is the
agent-executable half: build specs, built plugins, checks and probes. The game rows are
the owner's or lane C's (M5.C2 or the M5.O4 sitting) and are all `not-run`.

Arm A last built 2026-10-16 with the release `kit.exe` of lane D round 5 (from
`d35987a`), which authors the links the first build (at `97dc5ee`) could not; arm B is
unchanged and rebuilds to the same bytes. Game 1.16.244.0, `Starfield.esm` the only
master. Both plugins own new forms: disposable
saves on **KitKraft Dev** only, never removed from a save in progress (AGENTS.md).

The site's lifecycle state is the folder README's model (`../README.md`): this prototype
instantiates the authored site (unknown and discovered through its map marker, eligible by
authoring) and the workshop that makes it operational.

## Two arms

| Arm | Spec | Plugin | What it places |
| --- | ---- | ------ | -------------- |
| A | `site.toml` | `KitKraftFootholdSite.esm`, 3,977 bytes, SHA-256 `8aa846f1347db53db77077a6f041e997973f2a2229dd5c294be36af34d21b630` | a clone of the vanilla outpost template `LCTN` that names its map marker and lists its references, a clone of the vanilla beacon `ACTI`, three persistent references at the vanilla pack-in's offsets (the beacon linked to its centre marker, the centre marker, a map marker with its marker data linked to the centre marker), and the site's cell naming the location |
| B | `site-packin.toml` | `KitKraftFootholdSitePackin.esm`, 2,248 bytes, SHA-256 `f1e2a390571df78d32170f2e8d676790ee4cc67d8a255272b73db6982f318f3c` | a new `LCTN`, and one reference of vanilla's own beacon pack-in, which carries the links and marker values by reference |

Arm A authors the vanilla setup record by record; arm B asks whether reusing the vanilla
pack-in by reference is enough. The game rows decide between them (U2, U5).

## The site

`SettleCrucibleWorld` (`Starfield.esm:29B000`, "Crucible", Charybdis III), exterior cell
(1, -2) `Starfield.esm:127392`, at (155, -110, 8). Why there (`kit cell`, `kit inspect`,
`kit refs`, `--plugins Starfield.esm`):

- A fixed planet surface: a settlement worldspace (WRLD `XLCN` `SettleCrucibleLocation`
  `299F69`, whose `PNAM` is `SCharybdis_PCharybdisIII_Surface` `01B38B`), not one of the
  `Overlay*` or `OE*` worlds the planet content manager places.
- No outpost ban: of the settlement and city locations checked, Kreet, Gagarin, Eleos
  Retreat, Paradiso, Hopetown, New Homestead and Waggoner Farm carry the keyword
  `OutpostPreventPlacement` (`06902E`); `SettleCrucibleLocation` carries only
  `LocTypeSettlement` (`022611`). The other two without it were less quiet: Red Mile's
  world holds 22,418 references, and `VecteraWorldSpaceLocation` never resets (level
  flags 17).
- Quiet: the cell holds 5 references (a fern and four rocks at its north edge), no actor;
  the world's busy cells are (0, 0) and (0, 1) with 3,210 and 2,588.
- Near a landing area: `SettleCrucibleLocation`'s special reference with
  `ShipLandingMarkerRefType` (`009105`) is `Starfield.esm:12A0DD` in cell (1, -1), at
  (157.96, -57.40, 4.92), about 53 units north of the beacon.

## Evidence: the vanilla outpost records copied

| Record | What vanilla has | Used as |
| ------ | ---------------- | ------- |
| `LocTypeOutpost` `KYWD` `Starfield.esm:0234F1` | the outpost location type | the new location's only keyword |
| `OutpostOverlayTemplateLocation` `LCTN` `2A39CC` | the only location with `LocTypeOutpost` (`kit query LCTN --where 'KWDA == Starfield.esm:0234F1'`: 1 of 6,017): `FULL` "Outpost", `KWDA` [`0234F1`], `DATA` level flags 16 (`UseParentsLockedLevel`), no owner, levels 0, `RNAM` 0, `ANAM` 1.0, `TNAM` 60, no `PNAM` | arm A's `[[clone]]` (every value kept; the name is ours), gaining `PNAM`, `MNAM` and `LCPR`; arm B's `[[new]]` |
| `SettleCrucibleLocation` `LCTN` `299F69` | 32 `LCPR` entries naming `SettleCrucibleWorld` `29B000` with the grid of the cell the reference's position is in (`0C33F7` at (58.1, 132.3): Y 1, X 0; `299F65` at (104.8, 126.6): Y 1, X 1), each ending `FFFFFFFF` | the shape of arm A's three `LCPR` entries (cell (1, -2)) |
| `CityNewAtlantisLodgeLocation` `LCTN` `27C8ED` | the only official location with `MNAM`, naming a persistent map marker reference (`0F93F1`, base `000010`) | arm A's location `MNAM` naming its map marker |
| `OutpostOverlayTemplate` `WRLD` `2A39CB` | the template's only referrer (`XLCN`); 0 cells | not copied: new worlds are refused (U6) |
| `OutpostBeaconActivator` `ACTI` `2C60E9` | "Outpost Beacon", 19 keywords (`MustPersist`, `WorkshopKeyword`, 16 `OutpostCategory*` build-menu categories, `WorkshopNoStackingOn`), `PRPS` `OutpostBuildAreaRadius` (`063FC5`) **150** and `OutpostMainPackinItem` (`25E596`) 1, `FTYP` `OutpostBeacon_RefType` (`3E8651`), snap template `1D6187`, `VMAD` `outpostbeaconscript` (properties `OutpostSurveyBoost`, `OutpostSurveyBoostMessage`, `OutpostTutorial`, `SQ_Crew`, `SQ_Companions`, `SQ_Parent`, `LocTypeOutpost`) | arm A's `[[clone]]` (all kept, `FULL` "KitKraft Foothold Beacon"); arm B through the pack-in |
| `OutpostBeacon_Packin` `PKIN` `29A8C6` | `CNAM` cell `PackInOutpostBeaconPackinStorageCell` `29A8A6`, `FNAM` 6; no typed referrer | arm B's placed base |
| beacon reference `29A8C8` | persistent, at the pack-in origin, `XLKR` `WorkshopLinkCenter` (`038C0B`) to `29A8C7`, then an empty `XLKT` | arm A's beacon reference (persistent), with the same link and `XLKT` |
| `29A8C7` | persistent `XMarkerHeading` (`000034`) at (0, -1.5, 0), rotation z 4.712389, `XLRT` `28ADA0` | arm A's centre marker, `XLRT` copied |
| `OutpostMapMarker001` `295918` | persistent `MapMarker_LongRange` (`254244`) at (0, -0.0679, 0): `XMRK`, `FNAM` 51, `FULL` "Outpost", `TNAM` 12, `VNAM` 2, `UNAM` string ID 0, `VISI` 0, `XRDS` 150, `XLKR` (no keyword) to `29A8C7`, `XLKT` | arm A's map marker reference with the same data (`FULL` names the site; `UNAM` empty text, as the plugin is not localized) |
| the pack-in's other references | 4 `CollisionMarker`, an `NPCClipboard` furniture at (-1.5, 0, 0), an `OutpostGroupPackinDummy` | arm B only (N7) |
| `REFR` with a `PKIN` base in this world | 21 in `CrucibleExt03`, 9 in `CrucibleExt06`, all temporary | arm B's reference is temporary |

**Build radius note.** The radius is data on the beacon's base form: `PRPS`
`OutpostBuildAreaRadius` = 150 on `OutpostBeaconActivator`, kept by the clone. The
vanilla map marker's `XRDS` is also 150. Whether the build area is read from the base's
`PRPS`, from the reference, or from the location is unobserved (U2); probe W-01 reads it
with `getav` and W-03 tests it in build mode.

## Build and checks

```text
kit build prototypes/foothold/exp01-authored-site/site.toml out/prototypes/foothold/exp01/KitKraftFootholdSite.esm
kit validate --plugin out/prototypes/foothold/exp01/KitKraftFootholdSite.esm
kit roundtrip out/prototypes/foothold/exp01/KitKraftFootholdSite.esm --verify
kit refs KitKraftFootholdSite.esm:001000 --plugins Starfield.esm --plugins out/prototypes/foothold/exp01/KitKraftFootholdSite.esm
kit testplan out/prototypes/foothold/exp01/KitKraftFootholdSite.esm out/prototypes/foothold/exp01/testplan --plugins Starfield.esm --spec prototypes/foothold/exp01-authored-site/site.toml
```

Arm B: the same with `site-packin.toml`, `out/prototypes/foothold/exp01b/KitKraftFootholdSitePackin.esm`.

Results (2026-10-16; arm A rebuilt in lane D round 5, arm B as first built): `validate`
passed, 0 errors and 0 warnings on all 17 rules (`formid.dangling` over 128 FormIDs in A,
and the two papyrus rules included); `roundtrip --verify` identical (A: 9 records, 8
groups; B: 5 records, 7 groups); a second build of `site.toml` gives the same bytes.
`refs` of every new form resolves every link, none dangling (A: the beacon's 31 links,
among them the 7 `VMAD` object properties; the location's 9: keyword, parent, map marker,
and each `LCPR` entry's reference and world; the beacon reference's base, link keyword
and centre marker; the centre marker's base and `XLRT`; the map marker's base and centre marker; the
cell's `XLCN` names the location; the only `null`s are FormID 0 values: the location's
owner, the map marker's link keyword, two of the beacon's). New forms, A: `ACTI` `:001000`
`KitKraftFootholdBeacon`, `LCTN` `:001001` `KitKraftFootholdSiteLocation` (a clone of
`2A39CC`), `REFR` `:001002` `KitKraftFootholdBeaconRef`, `:001003`
`KitKraftFootholdBeaconCenter`, `:001004` `KitKraftFootholdMapMarker` (persistent, in the
world's persistent cell `28FECE`; the `WRLD` and that cell carried unchanged), and the
cell `127392` overridden for `XLCN` alone. B: `LCTN` `:001000`, `REFR` `:001001`
`KitKraftFootholdBeaconPackinRef` (temporary, in cell `127392`; the `WRLD` and the cell
carried unchanged). `testplan` wrote `checklist.md`, `kitkraft-test.txt` and `probes.toml`
(A: 16 rows; B: 7 rows) into `out/prototypes/foothold/exp01{,b}/testplan/`.
`site-probes.toml` here adds the workshop rows W-01 to W-07 (run after the generated
rows).

## Not authored (and why)

Lane D round 5 (M5.D8 to M5.D11, 2026-10-16) closed N1 to N6 and N8 of the first build:
arm A now writes the beacon's `XLKR` `WorkshopLinkCenter` to the centre marker (N1), the
map marker's data and link (N2), the centre marker's `XLRT` (N3, copied; whether the CK
writes it is U4), the cell's `XLCN` naming the location (N4), the location's `MNAM` and
`LCPR` naming the references (N5), its `KWDA` as a typed list (N6, kept from the clone),
and the location as a clone of the template gaining `PNAM` (N8). What remains:

| # | What vanilla has | Why arm A does not write it | Closes it |
| - | ---------------- | --------------------------- | --------- |
| N5b | location special references (`LCSR`/`ACSR`: a location reference type `LCRT` per reference) | which `LCRT` an outpost site's references take is unobserved (the template location has none, and the centre marker's `28ADA0` is obsolete by its EditorID) | the CK twin (U4) |
| N7 | the pack-in's collision markers, `NPCClipboard` and group dummy | not copied in arm A (no link data for them either) | arm B places them by reference |
| N9 | the map marker's subrecord order: vanilla's `295918` has `XRDS` after `VISI`; arm A writes it at its schema slot, before `XMRK` (where 224,559 of the corpus's `XRDS` are; 20,440 elsewhere, `REFR.toml`) | `build` adds a subrecord at its schema member order | nothing to close unless the game or the CK twin shows the order matters |

## Unobserved (each with the experiment that resolves it)

| # | Unknown | Resolved by |
| - | ------- | ----------- |
| U1 | Terrain height at (155, -110): `z` 8 is read off the rocks at y -101 to -103 (z 7.2 to 9.7); `kit` does not read `LAND` | W-01's screenshot in the M5.C2/M5.O4 session; the CK twin (M5.A5) |
| U2 | Whether an authored, never-player-placed beacon opens build mode, with or without the location link (N4, N5), and where the build radius is read | W-02, W-03 (game rows of FTH-EXP-01) |
| U3 | Whether `outpostbeaconscript` expects the player's own beacon placement (its properties name the outpost tutorial and crew quests); read it with `kit papyrus disasm`, never change it | W-02; a disassembly round (lane B) |
| U4 | What the CK writes when a modder sets up this site (links, location lists, cell `XLCN`, `ACPR`) | the CK twin of the beacon setup (M5.A5, an owner CK sitting) |
| U5 | Whether the game expands a plugin-placed pack-in reference into its contents (arm B) | arm B's generated rows and W-02 run on arm B |
| U6 | How a player outpost gets its location at run time (the overlay world `OutpostOverlayTemplate` names the template location) | a vanilla outpost observed in the same session (M5.O4) |
| U7 | Whether the site interferes with the Crucible settlement or a vanilla outpost nearby (C1, "double-register") | W-07; FTH-EXP-09 |
| U8 | Place, power, move and reload of built objects (the experiment's passing observation) | W-03 to W-06 |

## Report

```yaml
id: FTH-EXP-01
priority: EXP
owner: lane E
requires: [M5.D2, M5.D4, M5.D7, M5.A5, M5.C2]
output: prototypes/foothold/exp01-authored-site/{site.toml, site-packin.toml, site-probes.toml}; KitKraftFootholdSite.esm 8aa846f1347db53db77077a6f041e997973f2a2229dd5c294be36af34d21b630; KitKraftFootholdSitePackin.esm f1e2a390571df78d32170f2e8d676790ee4cc67d8a255272b73db6982f318f3c
source_provenance: [corpus:Starfield.esm LCTN 2A39CC, WRLD 2A39CB, ACTI 2C60E9, PKIN 29A8C6, CELL 29A8A6, REFR 29A8C7 29A8C8 295918, KYWD 0234F1 038C0B 06902E, LCTN 299F69 01B38B 27C8ED, CELL 127392, REFR 12A0DD 0F93F1 0C33F7 299F65]
validation:
  parse: pass            # kit validate + roundtrip, 2026-10-16: arm A at lane D round 5, arm B at 97dc5ee
  byte_roundtrip: pass   # kit roundtrip --verify: identical, both plugins
  ck_open: not-run
  ck_reexport: not-run
  game_runtime: not-run
  save_reload: not-run
limits: [N5b, N7, N9 above; terrain height assumed (U1)]
result: partial
```

`result` is read off the rows (AGENTS.md, "Reporting"): the two KitKraft rows passed, so
the ticket is `partial`, not `hypothesis`. The experiment itself (CK twin, game, save and
reload) is still a hypothesis: none of its rows has run.
