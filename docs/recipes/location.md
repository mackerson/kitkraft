# Recipe: a location of your own (M5.D2)

Goal: the general recipe for authoring a location (`LCTN`) with KitKraft — a new one and a
clone of a vanilla one, with its parent, a location-type keyword, its map marker (`MNAM`)
and its persistent-reference list (`LCPR`), plus the cell's `XLCN` — all from one `build`
v3 spec with `Starfield.esm` as the only master. Spec: `docs/recipes/location.toml`. The
plugin owns three new forms and one cell override, so it is for a disposable save only:
never remove it from a save in progress (AGENTS.md, "Owner's live game").

It is general: to make another site's location, change the EditorIDs, the parent, the
keyword, the cell and the marker's position; the clone shows how a vanilla location is
reparented into your own hierarchy.

## What KitKraft knows, and from where

| Fact | Source |
| ---- | ------ |
| `LocTypePlayerHouse` is `KYWD` `Starfield.esm:2EF272`, the location-type keyword the official interior house locations carry: `CityAkilaCityPlayerHouseNiceLocation` `04B9E2` and `CityNeonTradeTowerBayuPenthouseLocation` `1F9FAB` both have `KWDA [2EF272]`; the mini-site's `LocTypeOutpost` `0234F1` is an outpost's type, not a fit for an interior site | `kit query KYWD --where 'EDID ~ "LocType"' --plugins Starfield.esm`; `kit inspect` of the two locations (2026-10-26) |
| `CityNewAtlantisLocation` is `LCTN` `Starfield.esm:01295A` ("New Atlantis"), this location's parent (`PNAM`) — the parent the mini-site's location and the lodge `CityNewAtlantisLodgeLocation` `27C8ED` both name | `kit inspect Starfield.esm:01295A`; `kit inspect Starfield.esm:27C8ED` |
| `CityNewAtlantisLodgeLocation` `27C8ED` is the one official LCTN with `MNAM`: it names its placed marker `NewAtlantisMapMarkerLodge` `0F93F1` (base `000010`); its `LCPR` entries spell an interior reference as the mini-site does — the cell, grid 32767 x 32767 and `FFFFFFFF` after it (its own cell `06BB29`) | `kit inspect Starfield.esm:27C8ED`; `kit inspect Starfield.esm:0F93F1` |
| `LCTN` is typed-writable through `new` (every typed member written and read back); an array such as `KWDA` or `LCPR` takes a typed list (lane D round 5: FormIDs, `new:` names, struct entries as tables) | `crates/engine/src/handlers/authoring_tests.rs` (`lctn_new_sets_every_typed_member_and_reads_back`); `crates/engine/tests/lane_d5_corpus.rs` |
| `MNAM` is a single `formid` naming a placed marker reference (`REFR` or `ACHR`); a `[[place]]`d reference may be named from `[[new]]` fields with `new:<name>` | `crates/schema/defs/LCTN.toml` (`WorldLocationMarkerRef`); the build's FormID allocation (below) |
| `CityNewAtlantisApartmentTEMPLATE` has no `XLCN`; a `[[set]]` adds a subrecord a record lacks at its schema slot (lane D round 5) | `kit inspect Starfield.esm:21966F`; `crates/schema/defs/CELL.toml` member order |
| `XMarkerHeading` is `REFR`-base `Starfield.esm:000034` (no name of its own), the marker base the first POI's cell already uses | `kit inspect Starfield.esm:000034` |
| `CityNewAtlantisApartmentTEMPLATE` (`CELL` `Starfield.esm:21966F`) is the first POI's cell; its floor is at z 4.63 near (-17, -4), and it has no navmesh; the marker goes west of the first POI's crate at (-25, -4, 4.63), continuing the mini-site's row of placements (x -19, -21, -23) | `docs/poi/first-poi.toml`; `docs/recipes/mini-site.md` |
| `OutpostOverlayTemplateLocation` is `LCTN` `Starfield.esm:2A39CC`, the only Starfield.esm location of type `LocTypeOutpost`: `KWDA [0234F1]`, `DATA` `LevelFlags` 16 (`UseParentsLockedLevel`) with owner none and levels 0, `RNAM` 0, `ANAM` 1.0, `TNAM` 60 — all kept by the `[[clone]]`; its `FULL` ("Outpost") is ours here | `kit inspect Starfield.esm:2A39CC`; `prototypes/foothold/exp01-authored-site/site.toml` |
| `build` gives `[[clone]]` entries their FormIDs first, then `[[new]]`, then `[[place]]`, each in spec order, from `0x001000`, all before anything is written, so a field may name any entry with `new:<name>`, a `[[place]]` too (lane D round 5) | the build's output (`LCTN :001000`, `LCTN :001001`, `REFR :001002`); `crates/engine/tests/mini_site_corpus.rs` pins the order |

## What the plugin links

```text
LCTN KitKraftRecipeLocation      --PNAM-->  LCTN CityNewAtlantisLocation (Starfield.esm:01295A)
                               --KWDA-->  KYWD LocTypePlayerHouse (Starfield.esm:2EF272)
                               --MNAM-->  REFR KitKraftRecipeMarker (new)
                               --LCPR-->  the marker below, with the cell
LCTN KitKraftRecipeOutpostLocation (clone of Starfield.esm:2A39CC)
                               --PNAM-->  LCTN KitKraftRecipeLocation (added to the clone)
CELL CityNewAtlantisApartmentTEMPLATE --XLCN--> LCTN KitKraftRecipeLocation (added)
REFR KitKraftRecipeMarker        (-25, -4, 4.63)  base XMarkerHeading (Starfield.esm:000034)
     persistent, in CityNewAtlantisApartmentTEMPLATE (overridden: XLCN only)
```

## Build, check

```text
kit build docs/recipes/location.toml out/recipes/KitKraftRecipeLocation.esm
kit validate --plugin out/recipes/KitKraftRecipeLocation.esm
kit roundtrip out/recipes/KitKraftRecipeLocation.esm --verify
kit refs KitKraftRecipeLocation --plugins Starfield.esm --plugins out/recipes/KitKraftRecipeLocation.esm
kit testplan out/recipes/KitKraftRecipeLocation.esm out/recipes/location-test --plugins Starfield.esm --spec docs/recipes/location.toml
```

2026-10-26 (release build): `KitKraftRecipeLocation.esm` 2,163 bytes, SHA-256
`43c2a244ff57743abf568535bcc3a87b4ab2d00b75b5f4154fa6313eaf3279e7`, one master
(`Starfield.esm`), new forms `:001000` to `:001002` (the cloned outpost LCTN, the new LCTN,
one persistent REFR) and the cell overridden for `XLCN`; `validate` 0 errors, 0 warnings;
`roundtrip --verify` identical (5 records, 6 groups, 1 compressed record); a second build is
byte-identical; `refs` of every new form and of the cell resolves every link. `testplan`
writes `checklist.md`, `kitkraft-test.txt` and `probes.toml` (8 probes).
`crates/engine/tests/location_recipe_corpus.rs` repeats all of it through `execute`
(release, `--ignored`).

## Members not written

The eight `LCTN` members at `reference` confidence in `crates/schema/defs/LCTN.toml` stay
unwritten until a CK fixture or the corpus raises them:

- `VMAD` (Scripts): xEdit builds it from script unions, so it stays raw; not in any
  official LCTN.
- `NAM0`: marked unused in xEdit; not in any official LCTN.
- `RCPR` (RemovedPersistentRefs), `RCUR` (RemovedUniqueBaseForms), `RCUN`
  (RemovedUniqueNpcs) and `RCSR` (RemovedSpecialRefs): the "Removed" reference lists; not
  in any official LCTN.
- `ACEC` (AddedWorldCells) and `RCEC` (RemovedWorldCells): one subrecord per worldspace;
  not in any official LCTN.

The other members the recipe leaves to the default are at `corpus` confidence, and this
site has no value for them: the "Added"/"Master" reference lists beyond `LCPR` (`ACPR`,
`ACUR`/`LCUR`, `ACUN`/`LCUN`, `ACSR`/`LCSR`, `ACID`/`LCID`, `ACEP`/`LCEP`), the properties
(`PRPS`), the ownership and encounter data (`DATA`), the music, faction, radius, fade and
timer members (`NAM1`, `FNAM`, `RNAM`, `ANAM`, `TNAM`), the colour (`CNAM`) and the star
and planet ids (`XNAM`, `YNAM`). The clone inherits the template's own `DATA`, `RNAM`,
`ANAM` and `TNAM` unchanged; the new location carries none of them. As in the mini-site,
the `LCPR` grid and tail (32767 x 32767, `FFFFFFFF`) are copied from the official interior
entries, not understood: what the game reads from them is not observed.

## In game (owner; KitKraft Dev, disposable save; not run)

1. Load `KitKraftRecipeLocation.esm` on the **KitKraft Dev** mod set; load a disposable
   save.
2. `coc CityNewAtlantisApartmentTEMPLATE`.
3. `player.moveto KitKraftRecipeMarker`: the marker stands west of the first POI's crate,
   continuing the mini-site's row at x -25.
4. `help "KitKraft Recipe" 4` lists the new location; the outpost location is its child.
   Record the probes' results in `docs/game-observations.md` with the hash above.

**Load one at a time.** This recipe overrides the same cell as the mini-site recipe
(`CityNewAtlantisApartmentTEMPLATE`), and each writes its own `XLCN` on it; with both
loaded, the later plugin in the load order wins the cell's location.
