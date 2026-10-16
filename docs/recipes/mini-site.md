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
| `LCTN` is typed-writable through `new` (every typed member written and read back); `KWDA` is an array, which a field value takes only as `hex:` (so a `new:` name cannot go inside it) | `crates/engine/src/handlers/authoring_tests.rs` (`lctn_new_sets_every_typed_member_and_reads_back`); `crates/engine/src/edit/fields.rs` (module docs, value table) |
| `DefaultBlankComputer_ConsoleSitting_UsePackin` (`TERM` `Starfield.esm:00351B`, "Computer") names its menu in `DNAM` (`TerminalMenu`, `formid<TMLM>`): `DefaultBlankTerminal_Genesis` (`TMLM` `Starfield.esm:227DA9`, "Terminal", no body text, no items); the terminal carries `VMAD` `TerminalScript` with no properties | `kit inspect Starfield.esm:00351B`; `kit inspect Starfield.esm:227DA9`; `kit schema --type TERM` |
| `SC_LD_CratesMedium02` (`PKIN` `Starfield.esm:0121EC`, no `VMAD`) names its content cell in `CNAM`: `PackInSCLDCratesMedium02StorageCell` (`Starfield.esm:01128F`); a clone keeps it | `kit query PKIN --where 'EDID ~ "Crate" and not has VMAD'`; `kit refs KitKraftMiniSitePackIn` on the built plugin |
| `FACT` is typed-writable through `new`; the required `DATA`, `CRVA` and `VENV` are written as zeros when not given | `authoring_tests.rs` (`fact_new_sets_every_typed_member_and_reads_back`); the build's limitations |
| `ActivatorSwitch01` (`ACTI` `Starfield.esm:03CCF4`) has a model and no `VMAD`; a `[[clone]]` takes a `scripts` list, checked against the compiled `.pex` like a `[[place]]` | the scripts recipe (`docs/recipes/scripts.md`); `crates/engine/src/handlers/scripts.rs` |
| `CityNewAtlantisApartmentTEMPLATE` (`CELL` `Starfield.esm:21966F`) is the first POI's cell; its floor is at z 4.63 near (-17, -4), and it has no navmesh | `docs/poi/first-poi.toml`; `docs/recipes/actors.md` |
| `build` gives `[[clone]]` entries their FormIDs first, then `[[new]]`, then `[[place]]`, each in spec order, from `0x001000` | the build's output (`KYWD :001000` ... `REFR :001009`); `crates/engine/tests/mini_site_corpus.rs` pins the order |

## What the plugin links

```text
LCTN KitKraftMiniSiteLocation  --KWDA-->  KYWD LocTypeOutpost (Starfield.esm:0234F1)
                               --KWDA-->  KYWD KitKraftLocTypeMiniSite (clone of LocTypeOutpost)
                               --PNAM-->  LCTN CityNewAtlantisLocation (Starfield.esm:01295A)
                               --FNAM-->  FACT KitKraftMiniSiteFaction (new)
TERM KitKraftMiniSiteTerminal  --DNAM-->  TMLM KitKraftMiniSiteMenu (clone)
ACTI KitKraftMiniSiteSwitch    --VMAD-->  KitKraft:HelloOnLoad (Text, Times = 1)
PKIN KitKraftMiniSitePackIn    --CNAM-->  CELL PackInSCLDCratesMedium02StorageCell (vanilla)
REFR KitKraftMiniSiteSwitchRef    (-19, -4, 4.63)  base ACTI above
REFR KitKraftMiniSiteTerminalRef  (-21, -4, 4.63)  base TERM above
REFR KitKraftMiniSitePackInRef    (-23, -4, 4.63)  base PKIN above
     all three persistent, in CityNewAtlantisApartmentTEMPLATE (carried unchanged)
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

2026-10-16 (release build at `97dc5ee` plus this recipe): `KitKraftMiniSite.esm` 4,927
bytes, SHA-256 `10f22dbcbbb17acbe684138a3502da2d017f4ccbe06ab00201e5667a76c36c26`, one
master (`Starfield.esm`), new forms `:001000` to `:001009` (KYWD, TMLM, TERM, PKIN, ACTI,
FACT, LCTN, three REFR); `validate` 0 errors, 0 warnings (the 17 plugin rules,
`formid.dangling` over 31 FormIDs, 5 subrecords unchecked because opaque); `roundtrip
--verify` identical (12 records, 12 groups); `refs` of every new form resolves every
link (the terminal's one `null` is the vanilla marker keyword slot, FormID 0). Spec
SHA-256 `c766467a83c5ca229c4dc647bea98b39bb38829d1e267724b48e7bfb7e0bed00`.
`HelloOnLoad.pex` compiled the same day: 911 bytes, SHA-256
`79ba706301dc8305c9d2e519b103f664e8176f23fa77126c3808c4d51879fb3f` (the compiler's output
differs from the scripts recipe's build of 2026-10-15; its disassembly is the same).
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

## Limits

- **No reference is in the location.** `[[set]]` cannot add `XLCN` to a cell that has
  none (`no-subrecord`: adding subrecords to an existing record is not supported), and a
  `[[new]]` field cannot name a `[[place]]` (`new:` names only `[[new]]` and `[[clone]]`),
  so the location's persistent-reference list (`LCPR`) and marker (`MNAM`) stay empty.
  The links run from the location outwards, not to the references.
- **`KWDA` of a new form is hex.** The site keyword's slot is its FormID as built
  (`01001000`), which depends on the clone staying first in the spec; the corpus test
  fails if it moves.
- **A clone cannot gain a subrecord** its source lacks (the switch has no `KWDA`, so it
  cannot carry the site keyword).
- The `FACT` is bare: no relations or ranks (the rank group stays at `reference`
  confidence, M5.D5), and its crime values are zeros.
- The menu has no items: menu items naming new forms are M5.D3's.
- Not opened in the CK (`ck_open`, `ck_reexport` not run) and not loaded in game.
