# First POI: "reference not found" (investigation, 2026-10-15)

The owner loaded `KitKraftFirstPOI.esm` (SHA-256 `13c22b48...`, the file in
`mods\Kitkraft POI dev\`, same hash) on the profile **Kitkraft Dev**. `coc
CityNewAtlantisApartmentTEMPLATE` worked; `player.moveto 02001000` and `prid 02001000`
reported the reference as not found. MO2 showed the plugin at index 02.

**Finding: the ID was wrong, not the plugin.** The game put the plugin in full slot **04**,
so the crate is `04001000`. `02001000` is `SFBGS00D.esm:001000`, a temporary `REFR` in an
exterior cell of that file's own world (cell `01000834`, group type 9, file offset
`0x1E2676F`). That cell was not loaded, so the console could not find it. Nothing in the
plugin is known to be wrong. It has the same shape as a working third-party plugin that does
the same thing (below).

## Evidence

**1. The game's own load order, from the owner's saves.** A Starfield `.sfs` is a `BCPS`
container of zlib chunks. Its decompressed header lists the plugins in three lists: full,
then small, then medium. The order within each list is read here as the runtime slot order
(the usual save layout). The console check below confirms it. Both modded saves of
Kitkraft Dev (`Autosave1_..._000039`, `Exitsave0_..._000041`; game 1.16.244.0) list:

| Scale | Game order (save) | KitKraft `load` (today) | MO2 |
| - | - | - | - |
| full | `00` Starfield, `01` ShatteredSpace, `02` SFBGS00D, `03` SFBGS050, **`04` KitKraftFirstPOI**, `05` BlueprintShips-Starfield, `06` BlueprintShips-SFBGS050 | `00` Starfield, `01` BlueprintShips-Starfield, `02` ShatteredSpace, `03` SFBGS00D, `04` SFBGS050, `05` BlueprintShips-SFBGS050, **`06`** KitKraftFirstPOI | **`02`** |
| small | Constellation, OldMars, SFBGS004, SFBGS007, SFBGS008, SFBGS047 | Constellation, OldMars, SFBGS007, SFBGS008, SFBGS004, SFBGS047 | |
| medium | SFBGS003, SFBGS006 | SFBGS006, SFBGS003 | |

The Default profile's `Save56_..._040127` (84 plugins) shows the same rules, with 41 full,
33 small and 10 medium. The official full plugins come first. Every user full plugin
follows, in `loadorder.txt` order: KitKraft's order is right for these, but each one is 2
slots too high. The two blueprint (0x800) plugins load **after every other full plugin**, at
`27` and `28`. Small and medium user plugins match KitKraft's slots. `ShatteredSpace`,
`SFBGS00D` and `SFBGS050` are loaded even though `plugins.txt` lists them without `*`. MO2
counts neither these three nor the blueprint rule, so its index 02 is wrong too. The
testplan's `28001000` in `first-poi.md` assumed the Default order with the plugin appended.
By the save's rules, the game would put it at `27` there: after `EasyLockpick_by_Kor.esp`
(`26`) and before the two blueprint plugins, which move to `28` and `29`.

**2. Plugin structure compared with working mods** (read-only walk of every `.esm` under
`mods\` below 60 MB, plus Stroud):

- **`TIG_Shipyards.esm`** (active on Default, full slot `1C` by the save) does exactly what
  we do. It adds a new persistent `REFR` to vanilla interior cell `0006BB29`. It has `TES4`
  flags `0x1` and one master (`Starfield.esm`). It carries the cell as a compressed override
  (`EDID FULL DATA XCLL LTMP XCLW XILS XOWN XLCN XCAS XCIM XCMO XEMP XTV2`), with the ref in
  type 8, flag 0x400, subrecords `EDID NAME DATA`, form version 582 and header unknown 0.
  Ours is the same in every one of these. Our differences are limited to CK bookkeeping:
  - our `TES4` has no `SNAM`/`TNAM`/`BNAM`;
  - our HEDR next object ID is `0xFF001001` (TIG has `0x01000800`);
  - our groups are stamped 0 (TIG's are `13xxx`/1).
  Nothing suggests the game reads any of these when it resolves a reference. Three working
  plugins also have group stamps of 0 and no `SNAM`/`TNAM`/`BNAM`: BASE, CulinaryGreenhouses
  and ConstellationDecorations.
- Every other mod that adds refs to a master's interior cell carries the cell override too,
  and uses temporary refs (type 9, flag 0, `NAME DATA`, sometimes `XSCL`/`XLYR`/`XPRM`/`XLKR`):
  Better_Living_II, and at least 12 more cells across other mods.
- No working mod's new ref has a subrecord that ours lacks and that all of them have.

**3. Starfield.esm interior references** (top-level `CELL` group):

- **Universal subrecords:** of 30,766 persistent (type 8) `REFR`s, only `NAME` and `DATA`
  appear in 100 %. `EDID` appears in 12.0 %, `XALG` in 4.8 % and `XLRT` in 4.2 %. Of
  1,934,939 temporary refs, again only `NAME` and `DATA` appear in 100 %.
- **The TEMPLATE cell:** two cells' EDIDs end in TEMPLATE. `CityNewAtlantisApartmentTEMPLATE`
  holds 432 refs: 431 in type 9 and one vanilla persistent ref in type 8 with flag 0x400.
  Persistent refs in this cell are therefore normal.

**4. TES4:** flags `0x1`, HEDR 0.96, count 3, `CNAM MAST DATA INCC`. TIG, Better Living, BASE,
DWN Paradiso and the other full plugins all use `0x1` (or `0x81` when localized). An `.esm`
is a master by its extension anyway. Nothing here needs to change.

## Variants (`out\poi\variants\`; each 0 errors on 16 rules, roundtrip identical)

| File | Spec | SHA-256 | Test |
| - | - | - | - |
| `KitKraftFirstPOI.esm` (rebuild, identical to the tested file) | `docs/poi/first-poi.toml` | `13c22b48...57b5f1` | on Kitkraft Dev as before: `coc CityNewAtlantisApartmentTEMPLATE`, `prid 04001000`, `player.moveto 04001000` |
| `KitKraftFirstPOITemp.esm` (temporary ref: type 9, flag 0, `NAME DATA`, no EDID) | `docs/poi/variants/first-poi-temporary.toml` | `7637c5d7...4fc3e` | only if the above fails: disable the first, enable this in its place, `coc CityNewAtlantisApartmentTEMPLATE`, look near the hallway wall piece at (-17, -6), click the crate in the console to read its ID |

To check any slot in game, open the console and click the object; the console shows its
reference ID. Without the plugin, `prid 04001000` should fail. With it, the high byte should
be the plugin's position among full plugins: count the officials, then the user plugins, and
count blueprint plugins last.
