# Recipe: a mini site of linked new forms (Gate K0, M5.D7)

Goal: the smallest plugin that exercises every authoring path Gate K0 names at once: new
and cloned base forms of six types linked to each other, a script on a base form, and
placed references in an existing interior, all from one `build` v3 spec with
`Starfield.esm` as the only master. Spec: `docs/recipes/mini-site.toml`; script: the
scripts recipe's `docs/recipes/scripts/KitKraft/HelloOnLoad.psc` (ours). The plugin owns
ten new forms and a script, so it is for a disposable save only: never remove it from a
save in progress (AGENTS.md, "Owner's live game").

It is general: to make another site, change the EditorIDs, the cell and the positions,
and swap the cloned vanilla forms for others of the same type.

## What KitKraft knows, and from where

| Fact | Source |
| ---- | ------ |
| `LocTypeOutpost` is `KYWD` `Starfield.esm:0234F1` (colour white, type 0, `FNAM` 0); a clone keeps those three, so the site's own keyword is a location-type keyword of the same kind | `kit query KYWD --where 'EDID ~ "LocTypeOutpost"' --plugins Starfield.esm`; `kit inspect Starfield.esm:0234F1` |
| `CityNewAtlantisLocation` is `LCTN` `Starfield.esm:01295A` ("New Atlantis"), the site location's parent (`PNAM`) | `kit query LCTN --where 'EDID ~ "NewAtlantis"' --plugins Starfield.esm` |
| `LCTN` is typed-writable through `new` (every typed member written and read back); an array such as `KWDA` or `LCPR` takes a typed list (lane D round 5: FormIDs, `new:` names, struct entries as tables), and `KSIZ` is written from the list's length | `crates/engine/src/handlers/authoring_tests.rs` (`lctn_new_sets_every_typed_member_and_reads_back`); `crates/engine/src/edit/fields.rs` (module docs, value table; `typed_whole_arrays`); `crates/engine/tests/lane_d5_corpus.rs` |
| An `LCPR` entry of a reference in an interior cell names that cell, with grid 32767 x 32767 (`FF7F FF7F`) and `FFFFFFFF` after it: `DebugLocation` `001116`'s entries name the interior cell `WarehouseInteractiveObjects` `257DB4` so; a world reference's entry names the world and its cell's grid (`OEAF015Location` `00118E`: `OEAF015World` `09A9CB`, 0 x 0). In the entries of the first 60 `Starfield.esm` locations with `LCPR`, every one ends in `FFFFFFFF` and 181 have the 32767 grid | `kit query LCTN --where 'has LCPR' --plugins Starfield.esm --limit 60`, `kit inspect` of each (2026-10-16) |
| `CityNewAtlantisApartmentTEMPLATE` has no `XLCN`; `set` (and a `[[set]]`) adds a subrecord a record lacks at its schema slot (lane D round 5), here after `XILS` | `kit set Starfield.esm:21966F --fields '{"XLCN": ...}' --dry-run`; `crates/schema/defs/CELL.toml` member order |
| `ActivatorSwitch01` has no `KSIZ`/`KWDA`; a `[[clone]]`'s fields add them at their slots (after `FLLD`, before `PNAM`), `KSIZ` from the list | `crates/engine/tests/lane_d5_corpus.rs` (`the_review_repros_now_author`) |
| `DefaultBlankComputer_ConsoleSitting_UsePackin` (`TERM` `Starfield.esm:00351B`, "Computer") names its menu in `DNAM` (`TerminalMenu`, `formid<TMLM>`): `DefaultBlankTerminal_Genesis` (`TMLM` `Starfield.esm:227DA9`, "Terminal", no body text, no items); the terminal carries `VMAD` `TerminalScript` with no properties | `kit inspect Starfield.esm:00351B`; `kit inspect Starfield.esm:227DA9`; `kit schema --type TERM` |
| `SC_LD_CratesMedium02` (`PKIN` `Starfield.esm:0121EC`, no `VMAD`) names its content cell in `CNAM`: `PackInSCLDCratesMedium02StorageCell` (`Starfield.esm:01128F`); a clone keeps it | `kit query PKIN --where 'EDID ~ "Crate" and not has VMAD'`; `kit refs KitKraftMiniSitePackIn` on the built plugin |
| `FACT` is typed-writable through `new`; the required `DATA`, `CRVA` and `VENV` are written as zeros when not given | `authoring_tests.rs` (`fact_new_sets_every_typed_member_and_reads_back`); the build's limitations |
| `ActivatorSwitch01` (`ACTI` `Starfield.esm:03CCF4`) has a model and no `VMAD`; a `[[clone]]` takes a `scripts` list, checked against the compiled `.pex` like a `[[place]]` | the scripts recipe (`docs/recipes/scripts.md`); `crates/engine/src/handlers/scripts.rs` |
| `CityNewAtlantisApartmentTEMPLATE` (`CELL` `Starfield.esm:21966F`) is the first POI's cell; its floor is at z 4.63 near (-17, -4), and it has no navmesh | `docs/poi/first-poi.toml`; `docs/recipes/actors.md` |
| `build` gives `[[clone]]` entries their FormIDs first, then `[[new]]`, then `[[place]]`, each in spec order, from `0x001000`, all before anything is written, so a field may name any entry with `new:<name>`, a `[[place]]` too (lane D round 5) | the build's output (`KYWD :001000` ... `REFR :001009`); `crates/engine/tests/mini_site_corpus.rs` pins the order |

## What the plugin links

```text
LCTN KitKraftMiniSiteLocation  --KWDA-->  KYWD LocTypeOutpost (Starfield.esm:0234F1)
                               --KWDA-->  KYWD KitKraftLocTypeMiniSite (clone of LocTypeOutpost)
                               --PNAM-->  LCTN CityNewAtlantisLocation (Starfield.esm:01295A)
                               --FNAM-->  FACT KitKraftMiniSiteFaction (new)
                               --LCPR-->  the three REFR below, each with the cell
CELL CityNewAtlantisApartmentTEMPLATE --XLCN--> LCTN KitKraftMiniSiteLocation (added)
TERM KitKraftMiniSiteTerminal  --DNAM-->  TMLM KitKraftMiniSiteMenu (clone)
ACTI KitKraftMiniSiteSwitch    --VMAD-->  KitKraft:HelloOnLoad (Text, Times = 1)
                               --KWDA-->  KYWD KitKraftLocTypeMiniSite (added to the clone)
PKIN KitKraftMiniSitePackIn    --CNAM-->  CELL PackInSCLDCratesMedium02StorageCell (vanilla)
REFR KitKraftMiniSiteSwitchRef    (-19, -4, 4.63)  base ACTI above
REFR KitKraftMiniSiteTerminalRef  (-21, -4, 4.63)  base TERM above
REFR KitKraftMiniSitePackInRef    (-23, -4, 4.63)  base PKIN above
     all three persistent, in CityNewAtlantisApartmentTEMPLATE (overridden: XLCN only)
```

## Compile, build, check

```text
kit papyrus compile docs\recipes\scripts\KitKraft\HelloOnLoad.psc out\recipes\Scripts --sources-zip "<Starfield>\Tools\ContentResources.zip"
kit build docs/recipes/mini-site.toml out/recipes/KitKraftMiniSite.esm
kit validate --plugin out/recipes/KitKraftMiniSite.esm
kit roundtrip out/recipes/KitKraftMiniSite.esm --verify
kit refs KitKraftMiniSiteLocation --plugins Starfield.esm --plugins out/recipes/KitKraftMiniSite.esm
kit testplan out/recipes/KitKraftMiniSite.esm out/recipes/mini-site-test --plugins Starfield.esm --spec docs/recipes/mini-site.toml
```

2026-10-16, lane D round 5 (release build of the round's branch, from `d35987a`):
`KitKraftMiniSite.esm` 5,010 bytes, SHA-256
`05aac03135515034e4efb8428d35bcaae3dfb9217d9a332141a802818d495d80`, one master
(`Starfield.esm`), new forms `:001000` to `:001009` (KYWD, TMLM, TERM, PKIN, ACTI, FACT,
LCTN, three REFR) and the cell overridden for `XLCN`; `validate` 0 errors, 0 warnings (the
17 plugin rules, `formid.dangling` over 39 FormIDs, 6 subrecords unchecked because
opaque); `roundtrip --verify` identical (12 records, 12 groups); `refs` of every new form
and of the cell resolves every link (the terminal's one `null` is the vanilla marker
keyword slot, FormID 0). Spec SHA-256
`901195d5a6387cb51dce3cdbe4eaf1fda91e389fbcaaab3302415dc37da2278e`. `HelloOnLoad.pex`
compiled the same day: 911 bytes, SHA-256
`dee45aa42fe9b7a6daac5093181299f44791b959a96cdfc020a6528b73969bb4` (the compiler's output
differs from build to build; its disassembly is the same). The first build (M5.D7, at
`97dc5ee`: 4,927 bytes, SHA-256 `10f22dbc...76c36c26`) lacked the `LCPR`, `XLCN` and the
switch's `KWDA`, and wrote `KWDA` as hex.
`testplan` writes `checklist.md`, `kitkraft-test.txt` (7 commands) and `probes.toml` (21
probes: a `help` probe per new form, the cell, `getpos` per reference, and the persistence
protocol). `crates/engine/tests/mini_site_corpus.rs` repeats all of it through `execute`
(release, `--ignored`).

## In game (lane C or the owner; KitKraft Dev, disposable save)

`kit game session` runs `probes.toml` once lane C's controller may launch (M4.T1, M4.O7);
until then, by hand:

1. Package or copy `out\recipes\Scripts\KitKraft\HelloOnLoad.pex` with the plugin into the
   **KitKraft Dev** mod set (`kit package`, `kit deploy --dev`); load a disposable save.
2. `coc CityNewAtlantisApartmentTEMPLATE`. **Expect:** the notification "KitKraft: mini
   site switch loaded" once (the base form's script, `Times` = 1).
3. `player.moveto KitKraftMiniSiteSwitchRef`; the switch, the computer and the crates
   stand in a row at x -19, -21, -23. Activate the computer: an empty "KitKraft Mini Site
   Menu" terminal.
4. `help "KitKraft Mini Site" 4` lists the location; record the probes' results in
   `docs/game-observations.md` with the hashes above, then the save/reload rows.

## What could not be authored

Closed in lane D round 5 (M5.D8 to M5.D11): the cell's `XLCN`, the location's `LCPR`
naming the placed references, the location's `KWDA` as a typed list with `new:keyword`
inside it, and the switch's `KWDA` (a subrecord its vanilla source lacks). What remains:

- **No map marker.** The location's `MNAM` (`WorldLocationMarkerRef`) is authorable now
  (`MNAM = "new:<place>"`), but the one official location that has it,
  `CityNewAtlantisLodgeLocation` `27C8ED`, names a map marker reference (base
  `000010`), and an apartment interior has none; the exterior prototype
  (`prototypes/foothold/exp01-authored-site/`) writes it.
- **No special references** (`LCSR`, `ACSR`): each names a location reference type
  (`LCRT`) whose meaning for a site is unobserved.
- **The `LCPR` grid and tail are copied, not understood.** 32767 x 32767 and `FFFFFFFF`
  are what the official interior entries hold (see above); what the game reads from them
  is not observed.
- The `FACT` is bare: no relations or ranks (the rank group stays at `reference`
  confidence, M5.D5), and its crime values are zeros.
- The menu has no items: menu items naming new forms are M5.D3's.
- Not opened in the CK (`ck_open`, `ck_reexport` not run) and not loaded in game.
