# Recipe: a terminal and its menus (KK-013)

Goal: a general recipe showing how to author a terminal (`TERM`) and its menus
(`TMLM`): a terminal whose menu (`DNAM`) is a clone of a vanilla terminal menu with
body texts and items, one of whose items is a submenu item (`TNAM`) that opens a second
cloned menu; every condition (`CTDA`) the clones carry is copied as stored. All from one
`build` v3 spec with `Starfield.esm` as the only master. Spec:
`docs/recipes/terminal.toml`. The plugin owns four new forms, so it is for a disposable
save only: never remove it from a save in progress (AGENTS.md, "Owner's live game").

It is general: to make another terminal, change the EditorIDs, the cell and the
position, swap the cloned vanilla forms for menus of the same shape, and set your own
`FULL`, body texts and item texts through field paths.

## What KitKraft knows, and from where

| Fact | Source |
| ---- | ------ |
| `UC01_SimShipTerminal_MainMenu` is `TMLM` `Starfield.esm:03D5D7`: `FULL` "Main Menu" (string ID `0x0000BC3D`), `TMVT` `Document`, 4 body texts (`BSIZ` 4), 3 items (`ISIZ` 3); item 2 is a submenu item (`ISET` Type `Submenu`, Flags `CreatesNewWindow`) and the record's only `TNAM`, stored as `UC01_SimShipTerminal_DiagnosticsTerminal` `03D5E7`; every item and three of the four body texts carry `CTDA` conditions | chosen from `kit query TMLM --where 'ISIZ >= 2 and has TNAM and has BTXT'` (25 matches; this one is a complete menu, so `FULL`, one `BTXT` and one `ITXT` can each be edited in place); `kit inspect Starfield.esm:03D5D7 --full` |
| The menu clone's ours: `FULL` "KitKraft Recipe Menu" (over the stored "Main Menu"), `BTXT[0]` our first body text, `ITXT[1]` (the record's second `ITXT`, the submenu item's text) "Open the Recipe Submenu", and `TNAM` (the record's only `TNAM`) `new:submenu` | the spec's comments; field paths in `crates/engine/src/edit/fields.rs` (`SIG[n]` counts occurrences record-wide) |
| `DefaultBlankTerminal_Genesis` is `TMLM` `Starfield.esm:227DA9`, the blank terminal menu: `FULL` "Terminal" (`0x0000BF98`), `TMVT` 0 (`Folder`), `BSIZ` 0, `ISIZ` 0; the mini-site recipe uses it as the same terminal's menu | `kit inspect Starfield.esm:227DA9`; `docs/recipes/mini-site.md` |
| The submenu clone's ours: the first body text (the source has none, so it is added at occurrence 0), `BSIZ` 1 — a stored count the schema does not derive from the `BodyText` group (it is not a `count_from`), so it is set with the text — and `TMVT` `Document` so the menu shows it | `crates/schema/defs/TMLM.toml` (`BSIZ` `u32`, note "number of BodyText items that follow"); a first build with only `BTXT` left `BSIZ` 0 beside one body text, so the spec sets it |
| `DefaultBlankComputer_ConsoleSitting_UsePackin` is `TERM` `Starfield.esm:00351B` ("Computer"): its menu link `DNAM` (`TerminalMenu`, `formid<TMLM>`) is the blank menu `227DA9`; it carries `VMAD` `TerminalScript` | `kit inspect Starfield.esm:00351B`; `crates/schema/defs/TERM.toml` |
| The terminal clone's ours: `FULL` "KitKraft Recipe Terminal" and the menu link `DNAM` `new:menu` (the menu clone above); the vanilla furniture keywords (`KWDA`, seven), the template (`FTMP`) and the `VMAD` stay as stored | the spec's comments; `kit refs KitKraftRecipeTerminal` on the built plugin |
| `CityNewAtlantisApartmentTEMPLATE` is `CELL` `Starfield.esm:21966F`, the first POI's cell; its floor is at z 4.63 near (-17, -4); no navmesh. The terminal is placed at (-25, -4, 4.63), west of the crate, next to the mini-site's terminal (-21, -4); persistent, EditorID `KitKraftRecipeTerminalRef` | `docs/poi/first-poi.toml`; `docs/recipes/actors.md`; `docs/recipes/mini-site.md` |
| `build` gives `[[clone]]` entries their FormIDs first, then `[[new]]`, then `[[place]]`, each in spec order, from `0x001000`, all before anything is written, so a field may name any entry with `new:<name>`, a `[[place]]` too; the FormIDs stored in the file carry the plugin's load-order slot as high byte (`01`, the only master is `Starfield.esm`), so the four forms are stored `01001000` to `01001003` though the build summary prints them `:00001000` to `:00001003` | the build's output; `kit inspect 01001002 --plugin <plugin>`; `crates/engine/tests/terminal_recipe_corpus.rs` pins it |
| The menu clone carries 16 `CTDA` conditions (body texts 3, 3, 2, 0; items 2, 3, 3), all comparison 0, `RunOn` `Subject`, `ComparisonValue` 0.0 or 1.0, `Function` 59 (Parameter1 Quest, Parameter2 QuestStage 500) or 629 (Parameter1 Quest, Parameter2 String), quest `UC01_ShipSimQuest` `Starfield.esm:2BA648`; a clone copies the `CTDA` bytes and the schema has no field path into them, so they are kept exactly as stored; `refs` of the built menu shows all 16 still naming the quest | `kit inspect Starfield.esm:03D5D7 --full`; `crates/schema/defs/common/Condition.toml`; `kit refs KitKraftRecipeTerminalMenu` on the built plugin |
| Texts set through a field, and texts a clone keeps, are stored as inline zstrings: a clone resolves its source's string IDs against the master's string table (item 0's `ITXT` `0x0000BC39` is stored as "Restart Current Tier"); `inspect` reads both back as text | `kit inspect 01001000 --plugin <plugin>` (payloads); `crates/engine/src/build/spec.rs` |

What is not known: the in-game display of the cloned menu (which body texts and items
pass the copied conditions), the function names behind `Function` 59 and 629 (the schema
keeps the function a number and the repo has no name table for the indices), and what
the game reads from `TMVT` on a menu whose source never showed body texts (the blank
source is `Folder`; the clone is set to `Document`).

## What the plugin links

```text
TERM KitKraftRecipeTerminal  --DNAM-->  TMLM KitKraftRecipeTerminalMenu (clone of UC01_SimShipTerminal_MainMenu)
TMLM KitKraftRecipeTerminalMenu  --TNAM (item 2)-->  TMLM KitKraftRecipeTerminalSubmenu (clone of DefaultBlankTerminal_Genesis)
                               --CTDA (16)-->  QUST UC01_ShipSimQuest (Starfield.esm:2BA648)
TERM KitKraftRecipeTerminal  --KWDA (7)-->  its vanilla furniture keywords, one null marker slot
                               --FTMP-->  FURN _Template_Terminal_Console01 (Starfield.esm:21B39A)
REFR KitKraftRecipeTerminalRef (-25, -4, 4.63)  base TERM above, persistent,
     in CityNewAtlantisApartmentTEMPLATE (Starfield.esm:21966F)
```

## Build, check

```text
kit build docs/recipes/terminal.toml out/recipes/KitKraftRecipeTerminal.esm
kit validate --plugin out/recipes/KitKraftRecipeTerminal.esm
kit roundtrip out/recipes/KitKraftRecipeTerminal.esm --verify
kit refs KitKraftRecipeTerminal --plugins Starfield.esm --plugins out/recipes/KitKraftRecipeTerminal.esm
kit testplan out/recipes/KitKraftRecipeTerminal.esm out/recipes/terminal-test --plugins Starfield.esm --spec docs/recipes/terminal.toml
```

2026-10-26 (release): `KitKraftRecipeTerminal.esm` 5,100 bytes, SHA-256
`4d3bfba83f28254f046e33f47d4a6ae9d00402395c15c1ec8ffc04188c8645d1`, one master
(`Starfield.esm`), new forms `:00001000` to `:00001003` (TMLM, TMLM, TERM, REFR; stored
`01001000` to `01001003`); `validate` 0 errors, 0 warnings (`formid.dangling` over 35
FormIDs, 3 subrecords unchecked because opaque); `roundtrip --verify` identical (6
records, 7 groups, 1 compressed); `refs` of every new form resolves every link (the
terminal's one `null` is a vanilla marker-keyword slot, FormID 0). Spec SHA-256
`7de4f9dd9f1ed6483a0057a4c2c73f8c21bf22c08653420f51ae32cd122749ad`. `testplan` writes
`checklist.md`, `kitkraft-test.txt` and `probes.toml`.
`crates/engine/tests/terminal_recipe_corpus.rs` repeats all of it through `execute`
(release, `--ignored`).

## In game (the owner; KitKraft Dev, disposable save)

Not run: these are the owner's steps.

1. Copy `out\recipes\KitKraftRecipeTerminal.esm` into the **KitKraft Dev** mod set
   (`kit package`, `kit deploy --dev`); load a disposable save.
2. `coc CityNewAtlantisApartmentTEMPLATE`; `player.moveto
   KitKraftRecipeTerminalRef` (or walk west of the crate; the terminal stands next to
   the mini-site's terminal).
3. Activate the terminal. **Expect:** a terminal named "KitKraft Recipe Terminal" whose
   menu is "KitKraft Recipe Menu"; our first body text leads the menu's body texts, and
   the item "Open the Recipe Submenu" — where its copied conditions allow — opens the
   submenu, a menu titled "Terminal" that shows "This is the KitKraft recipe submenu."
   The other body texts and items are the vanilla menu's, as stored.
4. Record the results in `docs/game-observations.md` with the hashes above.

## What could not be authored

Nothing was refused: every field the recipe wanted to set — the two `FULL`s,
`BTXT[0]`, `ITXT[1]`, `TNAM`, `TMVT`, `BSIZ`, `DNAM`, the reference's base, cell,
position, persistence and EditorID — was set through a field path. What stays as
stored:

- **The conditions.** The schema has no field path into a `CTDA` condition, so the 16
  conditions of the menu clone are copied byte-for-byte; their `Function` values are 59
  and 629 (Parameter1 Quest, Parameter2 QuestStage 500 or String), all against
  `UC01_ShipSimQuest` `2BA648`. The function names are not known: the schema keeps the
  function a number and the repo has no name table for the indices.
- **The rest of the cloned forms.** The other three body texts and their conditions, the
  other two items and their texts, the terminal's furniture keywords, its `FTMP`
  template and its `VMAD` (`TerminalScript`, opaque) are kept as stored from `03D5D7`
  and `00351B`. No new script is needed, so no Papyrus compile.
- Not opened in the CK (`ck_open`, `ck_reexport` not run) and not loaded in game.
