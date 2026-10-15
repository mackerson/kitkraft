# CK observations

The evidence ledger for Creation Kit differential observation (ADR 0005 §2, Amendment 2 §2)
and the M3 fixture matrix that feeds it (`plan/execution-plan.md`, M3, lane H). The CK is a
black box: an owner (or an agent with desktop control the owner assigns) opens plugins,
edits one thing through the UI, saves, and we diff the file it wrote against the base,
against the corpus and against our own writer. Nothing here is read from the CK's code or
memory.

Rules that bind every entry:

- **Clean room.** CreationKit.exe is launched through MO2 (executable #4) and observed only:
  no debugger, disassembler or memory tool. Its Creations, upload and Bethesda.net login
  features are never used.
- **Nothing from `observe/` is committed.** CK-saved plugins, string tables, screenshots
  and the generated reports stay in `C:\Modding\kitkraft\observe\` (gitignored). The ledger
  records offsets, widths, our own sentinel values, the bytes around them and SHA-256
  hashes, in our own words.
- **Our sentinels, not Bethesda data.** A ledger entry may quote bytes the CK wrote for our
  sentinel values and the header fields it sets; it never reproduces record payloads
  copied from the official files beyond the few bytes needed to locate a field.
- **One finding per entry**, filed to its lane as a `plan/STATUS.md` Request naming the
  entry id (M3.H3).

## In-game sessions (O2, O4, O5)

What the owner sees in the running game is not a CK observation and has its own ledger,
`docs/game-observations.md` (M4.H4): the entry format `kit testplan` reports into, a
persistence protocol (leave, wait, save, reload, return, long absence), and session
templates with every FormID and expected result: O2 (the rename, done 2026-10-14; the
rename and POI packages; the POI by `coc CityNewAtlantisApartmentTEMPLATE` then
`player.moveto KitKraftFirstPOICrate`), O4 (the ownership and actors recipes, and the
questions of `docs/design/persistence.md`), O5 (the overhaul's C1..C14). A CK fixture that
an in-game row needs (F25 for the alias questions) is named in that row.

## Ledger entry format

Id `ck-<yyyymmdd>-<nn>` (session date, two-digit sequence). One entry per finding:

| Field | Content |
| ----- | ------- |
| Id | `ck-20261020-01` |
| Fixture | Matrix id and folder (`F05`, `observe\2026-10-20-F05\`) |
| CK version / game build | From the fixture's `fixture.toml` (`ck_version`, `game_build`) |
| Question | Which M3.H3 target it answers (or which matrix "first question") |
| Record | Signature, FormID as `Plugin.esm:XXXXXX`, EditorID |
| Dialog and field | Dialog, tab and field label as the CK shows them |
| Location | Subrecord signature and occurrence, offset inside the payload, width; or the header field |
| Encoding | Type inferred from the sentinels (`f32`, `u32`, flag bit, ...) |
| Sentinels and bytes | Each sentinel and the bytes the CK wrote for it (e.g. `1234.5` -> `00 50 9A 44`) |
| Side effects | Everything else that changed (sizes, header fields, other subrecords, compression), or "none" |
| Files | SHA-256 of the base and of each changed file (from `report.md`) |
| Corpus cross-check | Whether the inferred layout fits every corpus occurrence (ADR 0005 §2 step 5) |
| Conclusion | The fact, its confidence effect (`ck-diff`, or a refuted member) |
| Filed | The Request it became (`plan/STATUS.md`, lane A/B/D/E) |

## Fixture matrix (M3.H1)

Each fixture is a folder `C:\Modding\kitkraft\observe\<yyyy-mm-dd>-<id>\` holding the
CK-saved files, screenshots and a `fixture.toml`. `cargo xtask observe new <id>` creates the
folder with a `fixture.toml` already filled with the forms and sentinels below. Plugin names
start with `KC_`. Gates (dossier section 4): G0 parse, G1 lossless passthrough, G2 narrow
edit, G3 cross-tool, G4 editor/game, G5 dependency semantics.

| ID | Fixture | Base | Masters ticked | Saved file(s) | Forms (resolved) | Sentinels | `kit_equivalent` | Gates | First question |
| -- | ------- | ---- | -------------- | ------------- | ---------------- | --------- | ---------------- | ----- | -------------- |
| F00 | Session metadata | - | - | `EditorDataFilesLoaded.txt` (copied) | - | - | - | - | The load order as the CK sees it (lane D) |
| F01 | Empty plugin | - | `Starfield.esm` | `KC_F01.esp` | none | - | none (no command writes an empty plugin) | G0, G1 | TES4 of a new `.esp` |
| F02 | Rifle renamed | - | `Starfield.esm` | `KC_F02.esp` | WEAP `Starfield.esm:0021BBCD` `RussianHuntingRifle` | Name = `Old Soviet Marksman Rifle` | `kit set` FULL (the M2 rename) | G1, G2 | `kit diff` against our rename plugin |
| F03 | Rifle numeric field x2, one checkbox | F02 | `Starfield.esm` | `KC_F03a.esp`, `KC_F03b.esp`, `KC_F03c.esp` | the rifle | float `1234.5` / `2345.25`, or integer `4321` / `1234`; one checkbox toggled | after M3.B1 (WEAP typed) | G2 | Field offset, width, encoding; what else moves |
| F04 | The rifle's ammo, numeric field x2 | F02 | `Starfield.esm` | `KC_F04a.esp`, `KC_F04b.esp` | AMMO `Starfield.esm:002B559B` `Ammo9x39mm` | as F03 | after M3.B2 (AMMO typed) | G2 | As F03 |
| F05 | Float GMST x2 | F01 | `Starfield.esm` | `KC_F05a.esp`, `KC_F05b.esp` | GMST `Starfield.esm:000ABEF6` `fJumpHeightMin` | `1234.5` / `2345.25` | `kit set` GMST `DATA` | G2 | As F03 |
| F06 | One new GLOB | F01 | `Starfield.esm` | `KC_F06.esp` | new GLOB `KC_F06_Global` (float) | value `1234.5` | none (no new records in M3) | G1 | FormID allocation, `HEDR` next object ID |
| F07 | Compressed NPC_ override (+ M3.A1 candidates) | F01 | `Starfield.esm` | `KC_F07.esp` | NPC_ `Starfield.esm:0000BD3B` `EAW_AudioSlateChar01_Lydia_NoAppearanceData`; M3.A1's candidates (pending) | one numeric field: as F03 | none yet (opaque payload) | G1, G2 | Compression choice and bytes |
| F08 | `XXXX` record override | F01 | `Starfield.esm` | `KC_F08.esp` | WRLD `Starfield.esm:0027CFA2` `akilacity` | one field outside `OFST`/`CLSZ`: as F03 | none yet (opaque payload) | G1 | `XXXX` writing |
| F09 | Two masters (full + small) | - | `Starfield.esm`, `SFBGS004.esm` | `KC_F09.esp` | GLOB `Starfield.esm:000FB76D` `CF05_EnsignTestGlobal`; GLOB `SFBGS004.esm:000EE4` `_SFBGS004` | `1234.5`; `2345.25` | none (`set` refuses small/medium sources) | G5 | Raw FormIDs, master list |
| F10 | Mixed tiers (full + small + medium) | - | `Starfield.esm`, `Constellation.esm`, `SFBGS006.esm` | `KC_F10.esp` | GLOB `Starfield.esm:000FB76D`; GLOB `Constellation.esm:000041` `ConstellationTestGlobal`; GLOB `SFBGS006.esm:000800` `SFBGS006_Global` | `1234.5`; `2345.25`; `3456.75` | none (as F09) | G5 | Raw FormIDs per scale vs lane D |
| F11 | Exports of F06 | F06 | `Starfield.esm` | `KC_F11_full.esm`, `KC_F11_medium.esm`, `KC_F11_small.esm` (as offered) | F06's GLOB | - | none (M4) | G1, G5 | Flags, scale bits (`0x400`), own-FormID form after export |
| F12 | F02 saved localized | F02 | `Starfield.esm` | `KC_F12.esp` + its string tables | the rifle | as F02 | none (M4) | G1 | lstring IDs and tables vs `crates/strings` |
| F13 | Non-ASCII name | - | `Starfield.esm` | `KC_F13.esp` | the rifle | Name = `Fusil côté Ä` | `kit set` FULL | G2, G4 | Inline text encoding |
| F14 | CK loads our plugin | - | `Starfield.esm`, `OldSovietMarksmanRifle.esm` (not active) | none needed | `out\OldSovietMarksmanRifle.esm` (2,221 bytes, SHA-256 `9C0B28A6...4078EA27`) | - | - | G3, G4 | Warnings the CK reports on our output |

Sentinel bytes (little-endian, so the byte diff can find them): `1234.5` = `00 50 9A 44`,
`2345.25` = `00 94 12 45`, `3456.75` = `00 0C 58 45` (`f32`); `4321` = `E1 10`,
`1234` = `D2 04` (low bytes of an integer of any width). `Fusil côté Ä` is
`46 75 73 69 6C 20 63 C3 B4 74 C3 A9 20 C3 84` in UTF-8 (what `kit set` writes) and
`... 63 F4 74 E9 20 C4` in Windows-1252, so F13's bytes say which one the CK writes.

### Resolved forms and why each was chosen

Every form was found with our own tools on the official files (KITKRAFT_DATA_DIR, read
only) on 2026-10-14; the commands are listed so the choice can be rechecked after a game
update.

| Role | Form | EditorID | Why | Evidence |
| ---- | ---- | -------- | --- | -------- |
| The rifle | WEAP `Starfield.esm:0021BBCD` | `RussianHuntingRifle` | The M2 smoke-test record: our rename plugin overrides it, so F02 compares directly with an existing `kit` output. Uncompressed, 1,999 bytes, form version 581 | `kit inspect Starfield.esm:0021BBCD --plugins Starfield.esm` |
| The rifle's ammo | AMMO `Starfield.esm:002B559B` | `Ammo9x39mm` (name "9x39mm") | The only AMMO FormID anywhere in the rifle's 64 subrecords: a scan of every 4-byte window of every payload (read in full from the file, not the 256-byte inspect preview) against the 150 AMMO FormIDs of `Starfield.esm` finds it once, at offset 0 of `WAM2`. The AMMO's own `NAM1` names a `Weapons\RussianHuntingRifle...` path, which agrees. Uncompressed, 421 bytes | `kit inspect`, `kit query AMMO --plugins Starfield.esm`, payload scan |
| A float GMST | GMST `Starfield.esm:000ABEF6` | `fJumpHeightMin` | A float setting (EditorID prefix `f`, 4-byte `DATA`, value 1.285875 = `8E 97 A4 3F`) with an obvious meaning, so the edit is easy to find in the game-settings editor; never activated in game | `kit query GMST --where 'EDID ~ "Jump"'`, `kit inspect` |
| A compressed NPC_ | NPC_ `Starfield.esm:0000BD3B` | `EAW_AudioSlateChar01_Lydia_NoAppearanceData` (name "Lydia") | Every named NPC_ in `Starfield.esm` is compressed; this is one of the smallest (261 bytes stored, 394 inflated) **without** a `TPLT` template, so the actor dialog edits its own values rather than inherited ones; an audio-slate actor with no appearance data keeps the record simple | `kit query NPC_ --where 'has FULL'` (all pages), header flags and an inflate of each body |
| The `XXXX` record | WRLD `Starfield.esm:0027CFA2` | `akilacity` | Records with an `XXXX`-prefixed subrecord in `Starfield.esm` are SFTR, NAVM, CELL, PNDT (all compressed), STDT, GBFM and WRLD (uncompressed). A worldspace has a CK dialog (World > World Spaces) and is uncompressed, so `XXXX` writing is seen without compression mixed in; `akilacity` carries `OFST` and `CLSZ` of 73,476 bytes each through `XXXX` (record 149,150 bytes), and its dialog fields live in the small subrecords | our own container walk of `Starfield.esm` (all records, inflated where compressed) |
| Full-master GLOB | GLOB `Starfield.esm:000FB76D` | `CF05_EnsignTestGlobal` | A float global (`FLTV` 0.0) whose name marks it as a test value; GLOB is the simplest dialog | `kit query GLOB --where 'EDID ~ "Test"'` |
| Small-master record (F09) | GLOB `SFBGS004.esm:000EE4` | `_SFBGS004` | `SFBGS004.esm` is small (raw FormIDs `FE...`); its only own GLOB; F09 is never activated in game | `kit query GLOB --plugins Starfield.esm --plugins SFBGS004.esm --scope all` |
| Small-master record (F10) | GLOB `Constellation.esm:000041` | `ConstellationTestGlobal` | Own record of a small master (`FE000041` as stored), a test global, float 0.0 | as above with `Constellation.esm` |
| Medium-master record (F10) | GLOB `SFBGS006.esm:000800` | `SFBGS006_Global` | Own record of the medium master (`FD000800` as stored), float 0.0 | as above with `SFBGS006.esm` |
| M3.A1's recompression candidates (F07) | pending | - | Up to 5 of the 160 records that do not recompress identically, chosen by lane A (M3.A1) so the CK can open each in a dialog. Lane A files them as a Request to lane H; they are added to F07's `fixture.toml` then | - |

## M3.O1: the CK session, resolved

The script in `plan/execution-plan.md` ("M3.O1: CK session script") with every placeholder
filled. Two sittings are fine: F00..F06 and F13, then F07..F12 and F14.

Before starting: note what is in `C:\Modding\MO2-Starfield\overwrite\` (ideally nothing).
For each fixture, `cargo xtask observe new <id>` (from `C:\Modding\kitkraft`) creates
`observe\<today>-<id>\fixture.toml`; fill it as you go. Screenshots go into the same folder.

| Step | F00 | F01 | F02 | F03 | F04 | F05 | F06 |
| ---- | --- | --- | --- | --- | --- | --- | --- |
| File > Data | - | `Starfield.esm` only, no active file | as F01 | `Starfield.esm`; `KC_F02.esp` active (a fresh copy per variant) | as F03 | as F01 | as F01 |
| Action | Copy `EditorDataFilesLoaded.txt`; note versions | none | Rifle: Name -> `Old Soviet Marksman Rifle` | Rifle: a: one numeric field -> 1st sentinel; b: same field -> 2nd; c: one checkbox | Ammo `Ammo9x39mm`: a/b: one numeric field, two sentinels | Game settings: `fJumpHeightMin` -> `1234.5` (a), `2345.25` (b) | Global > New: `KC_F06_Global`, float, `1234.5` |
| Save as | - | `KC_F01.esp` (record a refusal) | `KC_F02.esp` | `KC_F03a/b/c.esp` | `KC_F04a/b.esp` | `KC_F05a/b.esp` | `KC_F06.esp` |

| Step | F07 | F08 | F09 | F10 | F11 | F12 | F13 | F14 |
| ---- | --- | --- | --- | --- | --- | --- | --- | --- |
| File > Data | `Starfield.esm` | `Starfield.esm` | `Starfield.esm`, `SFBGS004.esm` | `Starfield.esm`, `Constellation.esm`, `SFBGS006.esm` | `Starfield.esm`; `KC_F06.esp` active | `Starfield.esm`, no active file | `Starfield.esm` | `Starfield.esm` + `OldSovietMarksmanRifle.esm` (not active) |
| Action | Actor `EAW_AudioSlateChar01_Lydia_NoAppearanceData`: one numeric field (record tab and label); one trivial edit per M3.A1 candidate | World Spaces > `akilacity`: one numeric field or checkbox outside the large data | `CF05_EnsignTestGlobal` -> `1234.5`; `_SFBGS004` -> `2345.25` | `CF05_EnsignTestGlobal` -> `1234.5`; `ConstellationTestGlobal` -> `2345.25`; `SFBGS006_Global` -> `3456.75` | The CK's export to master: full, then medium and small if offered (record every option) | F02's edit, saved localized if offered (keep the string tables and their folder) | Rifle: Name -> `Fusil côté Ä` | Load only; record every warning |
| Save as | `KC_F07.esp` | `KC_F08.esp` | `KC_F09.esp` | `KC_F10.esp` | `KC_F11_full.esm`, `KC_F11_medium.esm`, `KC_F11_small.esm` | `KC_F12.esp` | `KC_F13.esp` | - |

For F14, our plugin has to be visible to the CK: copy
`C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm` into a disposable MO2 mod (for example
`KitKraft observation (F14)`), enable it only for this fixture, and disable it afterwards.

After each save: find where the file landed (expected `overwrite\`), move it (with any
string tables) into the fixture folder, leave `overwrite\` as you found it, and fill
`landed_in`, `ck_version`, `game_build`, `messages`, `dialog`, `field` and `value_before`
in `fixture.toml`. A refused action is a finding: record the message and go on.

Then hand lane H the folder names. Each is processed with:

```powershell
$env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
cargo xtask observe observe\2026-10-20-F02   # writes report.md and report.json in that folder only
```

`KITKRAFT_KIT` may name a prebuilt `kit.exe`; otherwise the harness runs
`cargo run --release -p kit-cli`.

## M3.O2: the MO2 and in-game session, resolved

The script in `plan/execution-plan.md` ("M3.O2"), with the forms named. Disposable save;
screenshots to `observe\<yyyy-mm-dd>-O2\`; answers into `plan/STATUS.md`.

1. MO2: which mod MO2 shows providing `Interface\inventorymenu.swf`: `StarUI Inventory` or
   `StarUI Inventory - Free Lanes Compatibility Patch` (Data tab or conflicts view).
2. Console. `Constellation.esm` places no objects (its records are AVMD, KYWD, COBJ, OMOD,
   GLOB, LMSW, LVLI), so there is nothing of it to click. Use
   `help ConstellationTestGlobal 4` instead: the line shows its runtime ID (expected
   `FE000xxx`; KitKraft assumes `FE000041`). For a full plugin: `help RussianHuntingRifle 4`
   (KitKraft assumes `0021BBCD`).
3. `help <EditorID> 4` for each form below: does it exist, and which runtime ID does it show?
   KitKraft's assumption is the runtime ID `kit inspect <form> --profile
   C:\Modding\MO2-Starfield\profiles\Default` printed on 2026-10-14 (assumptions
   `runtime-slots-by-scale` and `hardcoded-official-slots`, `docs/load-order.md`).

   | Plugin | Form | EditorID | Stored FormID | KitKraft assumes |
   | ------ | ---- | -------- | ------------- | ---------------- |
   | `HG_ShipCargoHoldAtHome.esm` | TRNS | `HG_Outpost_ShipCargoHoldConsole` | `02000808` | unresolved (stored `02xxxxxx` with one master) |
   | `HG_ShipCargoHoldAtHome.esm` | ACTI | `HG_ShipCargoHoldConsole` | `02000807` | unresolved |
   | `HG_ShipCargoHoldAtHome.esm` | COBJ | `HG_co_Outpost_Misc_ShipCargoHoldConsole` | `02000806` | unresolved |
   | `HG_ShipCargoHoldAtHome.esm` | STMP | `HG_OutpostSnap_ShipCargoHoldConsole` | `0200080A` | unresolved |
   | `ShatteredSpace.esm` | GLOB `ShatteredSpace.esm:00222E` | `SFBGS001_MS04_AzadHellosBlocked` | `0100222E` | `0200222E` |
   | `SFBGS004.esm` | GLOB `SFBGS004.esm:000EE4` | `_SFBGS004` | `FE000EE4` | `FE004EE4` |
   | `SFBGS00D.esm` | GLOB `SFBGS00D.esm:000803` | `SFBGS00D` | `01000803` | `03000803` |
   | `SFBGS050.esm` | GLOB `SFBGS050.esm:00E0D1` | `SFTER_INC_APG01_CaptiveCount` | `0200E0D1` | `0400E0D1` |
   | `BlueprintShips-SFBGS050.esm` | LCTN `BlueprintShips-SFBGS050.esm:000872` | `BlueprintInteriorSFTER_TerranShip_Generic_TESTLocation` | `03000872` | `05000872` |
   | `SFBGS047.esm` | KYWD `SFBGS047.esm:000013` | `AnimFurn_SFBGS047_MoonJumper` | `FE001013` | `FE005013` |
   | `Constellation.esm` | GLOB `Constellation.esm:000041` | `ConstellationTestGlobal` | `FE000041` | `FE000041` |
   | `SFBGS006.esm` | GLOB `SFBGS006.esm:000800` | `SFBGS006_Global` | `FD000800` | `FD000800` |

   For the `BlueprintShips-SFBGS050.esm` location, also note the name `help` shows (in
   quotes after the EditorID): the plugin is localized but ships no string tables, and its
   `FULL` holds string ID `0x0002E9C4`, which is not in `sfbgs050_en.strings` either (none
   of its 66 LCTN name IDs is); a name, an empty name or a number tells lane D where the
   game looks (lane D's request from M3.D6).

   The `HG_ShipCargoHoldAtHome.esm` EditorIDs come from our own walk of that file (its
   records are left out of KitKraft's load order as unresolved); the plugin is in the
   owner's Default profile and is named here only for this check (M2.O5 stays **no** for
   third-party plugins as format evidence).
4. If M2.O6 is still open, do it now; then enable F13's `KC_F13.esp` the way M2.O6 enables
   the rename plugin and read the rifle's name in the inventory (correct, garbled, or blank).

## `fixture.toml`

`cargo xtask observe new <id>` writes this template with the fixture's values filled in;
`cargo xtask observe` reads it. Keys:

```toml
fixture = "F05"                 # matrix id
title = "One float GMST, two sentinels"
date = "2026-10-20"
ck_version = ""                 # CK title bar
game_build = ""                 # Starfield.exe Properties > Details
mo2_version = ""
masters = ["Starfield.esm"]     # ticked in File > Data
active_file = ""                # the active file set in File > Data, if any
landed_in = ""                  # where the CK saved (expected MO2 overwrite\)
base = '..\2026-10-20-F01\KC_F01.esp'   # the base fixture's plugin, relative to this folder
messages = []                   # every warning or message the CK showed (text)
screenshots = []                # file names in this folder
notes = ""

[[plugin]]                      # one per file the CK saved
file = "KC_F05a.esp"
form = "Starfield.esm:000ABEF6"
editor_id = "fJumpHeightMin"
action = "Game settings editor: fJumpHeightMin = 1234.5"
dialog = ""                     # dialog and tab as shown
field = ""                      # field label as shown
sentinel = "1234.5"
value_before = ""
# kit argument vectors that build our equivalent of this file; "{out}" is replaced by
# <fixture>/kit/<style>/<file>, and each is run in both write styles (kit, ck)
kit_equivalent = [
  ["set", "Starfield.esm:000ABEF6", "--fields", "{\"DATA\": 1234.5}", "{out}", "--plugins", "Starfield.esm"],
]

[[compare]]                     # extra pairs to diff (variants of one field)
a = "KC_F05a.esp"
b = "KC_F05b.esp"
```

What `cargo xtask observe <folder>` writes, inside the folder only: `report.json` and
`report.md` (SHA-256 of every file; per plugin the `TES4` header and subrecords,
`kit roundtrip --verify`, `kit coverage`, `kit validate`; per comparison a positional byte
diff, a `TES4` diff and `kit diff`: base vs each saved file, every `[[compare]]` pair, and
the CK's file vs ours in each style), `kit\<style>\` (our equivalents) and `kit\cache\`
(`kit`'s index cache). The style `ck` reports "unavailable" until `set` and `build` take a
`style` argument (M3.E5). No timings are recorded, so a rerun writes the same reports.

## Ledger

No entries yet: the first M3.O1 session has not happened (M3.H3 is gated on it).
