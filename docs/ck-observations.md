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
   **Answered 2026-11-01 (owner):** the patch provides it (6 winning conflicts over
   `StarUI Inventory`, 0 losing); recorded in `docs/load-order.md`.
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

## Session scripts (M5.A1)

Owner-assisted sittings follow a written script: per step what the owner does, what the
agent records (a screenshot, the CK's messages verbatim, hashes) and where the files go
(`observe\ck-<yyyy-mm-dd>\`, one fixture folder per fixture), then the ingestion commands
(`kit roundtrip --verify --decode`, `kit diff`, `cargo xtask observe`, `cargo xtask oracle
ck`). Every CK interface claim in a script is marked *(unobserved)* until a sitting
confirms it.

| Script | Fixtures | Sitting | State |
| ------ | -------- | ------- | ----- |
| `docs/ck-sessions/f29-f30.md` | F29 (the first POI's crate placed in the CK, `KitKraftFirstPOI-CK.esm`), F30 (`KitKraftFirstPOI.esm` re-saved as `KitKraftFirstPOI-CKRoundtrip.esm`) | M5.O1 (`plan/STATUS.md` row 12) | written 2026-10-16; F30's load half run 2026-11-01 by an agent (`ck-20261101-01`, no save); F29 approved once F30 is clean |

## Ledger

### ck-20261101-01: F30 first half, agent-run (load only; no save)

The first CK sitting, run unattended by an agent on 2026-11-01 with the owner's approval in
chat ("go ahead with 2 and 3"), launched through MO2 on `Kitkraft Dev` and driven through
the Windows UI Automation interface, window messages, keyboard and mouse input (no
debugger, memory access or resource reading). Record and screenshots in
`observe\ck-2026-11-01\` (`session.md`, `shots\`, `09-warnings.txt`); the CK's two settings
files were copied to `settings-before\` first. Interface observations from the same sitting
(the menu map, the Data dialog's layout) stayed in `observe\` until the owner ruled on ADR
0012 Amendment 2; he accepted it later on 2026-11-01, so they may now move
`docs/ck-parity.md` rows, in a change of their own.

Rules since 2026-11-01 (owner, "accept all", `plan/owner-decisions-2026-11-01.md` items 17,
18 and 21): an agent may run automated sittings (ADR 0012 Amendment 2); a CK save needs
the owner's OK per sitting (F30 allowed; F29 allowed once F30 comes back clean; F20..F28
one by one); static string scans of `CreationKit.exe` are not allowed, and the 2026-10-16
scan's entry (M5.A4) was deleted from this file.

| Field | Content |
| ----- | ------- |
| Id | `ck-20261101-01` |
| Fixture | F30 (`docs/ck-sessions/f29-f30.md`), first half |
| CK version / game build | CreationKit.exe 1.16.244.0; game 1.16.244.0 |
| Input | `KitKraftFirstPOI-CKRoundtrip.esm`: a byte copy of `out\poi\KitKraftFirstPOI.esm` (SHA-256 `13c22b48...57b5f1`, 1,821 bytes, roundtrips identical), deployed with `kit deploy --dev` as `KitKraft Dev - KitKraftFirstPOI-CKRoundtrip`, enabled for the sitting and disabled after it |
| Question | Does the CK load our plugin, and with which warnings (Gate K0, "CK can load") |
| Result | **Loaded.** Data dialog: the file listed as "Master File" with `Starfield.esm` as its one parent master; ticked; OK. The CK loaded `Starfield.esm` and our plugin in under 2 minutes (status bar "Done." by the check at 100 s, not yet at 40 s; working set about 16 GB) with no message box. Its Warnings window then held 1,590 rows (Time, Type, Form, Form ID, File, Last User, Current User, Last Touched, Text, Source File, Line Number); **none names our plugin or a KitKraft form**: every row is about vanilla forms (`MASTERFILE`, `FORMS` and the like) |
| Not done | Our plugin could not be made the active file: "Set as Active File" on the selected, ticked row changed nothing visible (status stayed "Master File"; no message). So nothing was saved and F30's second half (save unchanged, then `cargo xtask oracle ck`) is open. The File menu offers "Convert Active File to ..." and "Compact Active File Form IDs for ...", which suggests the CK expects a non-master active file; an ESP twin of the plugin, or the owner's guidance, is the next step |
| Side effects | The deployed copy is unchanged after the sitting (same SHA-256). The CK wrote `Backup\AutoSaveDEFAULT.bak` (126 bytes) into MO2's `overwrite\` and, as on every run, rewrote `CreationKitPrefs.ini` and `QtCreationKitSavedSettings.ini` in the game folder. It closed without a prompt |
| Driving notes | The Data dialog is a Win32 dialog whose list rows are not exposed through UI Automation: a row is selected by type-ahead, ticked by double-click (single click and Space did not tick it); a modal dialog opened through UI Automation `Invoke` blocks the call, so modal-opening controls are clicked instead. Qt menus expose their items under the menu bar item while open |

### ck-20261101-02: F30 second half, agent-run (the `.esp` twin, saved unchanged)

The owner approved the twin in chat on 2026-11-01 ("yes, try the .esp twin for F30"). The
sitting ran unattended through MO2 on `Kitkraft Dev`, driven the same way as
`ck-20261101-01`. Record in `observe\ck-2026-11-01\F30b\` (`session.md`, `shots\`,
`structure.txt`, the CK's saved file); fixture folder `observe\ck-2026-11-01\F30\` (the
master attempt moved to `F30-master-attempt\`); oracle output in `observe\ck-oracle\F30\`.

| Field | Content |
| ----- | ------- |
| Id | `ck-20261101-02` |
| Fixture | F30 (`docs/ck-sessions/f29-f30.md`), second half |
| CK version / game build | CreationKit.exe 1.16.244.0; game 1.16.244.0 |
| Input | `KitKraftFirstPOIEsp.esp`: the bytes of `out\poi\KitKraftFirstPOI.esm` with the TES4 master flag (`0x1`) cleared, nothing else changed (SHA-256 `ab620226...348083`, 1,821 bytes), deployed with `kit deploy --dev` as `KitKraft Dev - KitKraftFirstPOIEsp`, enabled for the sitting and disabled after it |
| Question | Does the CK save our plugin unchanged, and what does it rewrite (Gate K0, "CK can re-export") |
| Result | **Saved.** Data dialog: listed as "Plugin File", one parent master `Starfield.esm`; "Set as Active File" ticked it and changed its status to "Active File"; OK. Loaded in under 3 minutes; title bar `... - [KitKraftFirstPOIEsp.esp] <Small Master Capable- 4094 IDs remaining>`. File > Save Plugin wrote 1,808 bytes (SHA-256 `cd35126f...5e23f4`) with no prompt; File > Exit closed it with no prompt. Ctrl+S did not save (the window was not in front at the time; not retried) |
| Records | **Unchanged.** `kit diff`: 0 added, 0 removed, 0 changed, 2 unchanged. The CELL override's compressed payload (1,485 bytes) and the REFR `01001000` (header and body) are byte-identical. The CK's file roundtrips identical (`kit roundtrip --verify --decode`) |
| TES4 | `cargo xtask oracle ck`: fail, 4 unexplained, match 1/1. HEDR: version 0.96 kept; record count 3 -> 7 (the CK's count equals the two records plus the five groups); next object ID `FF001001` -> `00001007`. `DATA` after `MAST` (8 zero bytes) removed. `INCC` (4 bytes, value 1) removed. `BNAM` added: "Main" (the branch the title bar names). `CNAM` "DEFAULT" and `MAST` kept |
| Groups and record headers | Below the oracle's view (from `structure.txt`): every GRUP gets version control `0x3561` (ours 0); the cell-children group (type 6) gets 7 and the temporary group (type 8) gets 1 in the field after it (ours 0); records keep version control 0. The CELL's form version 581 -> 582 (REFR stays 582) |
| Side effects | The CK saved in place: MO2 wrote the change into the dev mod's own file, not into `overwrite\`, so `KitKraft Dev - KitKraftFirstPOIEsp` now differs from its deploy manifest (the dev mod is disabled; the CK's file is also copied to `F30\ck.esm`). The CK's two settings files were restored from `F30b\settings-before\` |
| Open | M5.A3 classes the four TES4 differences and the group fields by hand (which are CK bookkeeping, which our writer should copy). Whether this counts as F30 "clean" for F29 (sheet item 21) is the owner's call: the records came back unchanged, the header did not |

### ck-20261101-03: F29, agent-run (the first POI's crate placed in the CK)

The owner approved F29 in chat on 2026-11-01 ("yes, F30 counts as clean, go ahead with
F29") and gave the agent the screen ("take the screen"). The sitting ran unattended through
MO2 on `Kitkraft Dev`, driven as in `ck-20261101-01`, plus window moves (`MoveWindow` on a
floating window) and a mouse drag. Record in `observe\ck-2026-11-01\F29\` (`session.md`,
`shots\01..10`, `structure.txt`, the CK's three files); fixture folder the same; oracle
output in `observe\ck-oracle\F29\`.

| Field | Content |
| ----- | ------- |
| Id | `ck-20261101-03` |
| Fixture | F29 (`docs/ck-sessions/f29-f30.md`) |
| CK version / game build | CreationKit.exe 1.16.244.0; game 1.16.244.0; `Starfield.esm` SHA-256 `1dabed00...0da7` (matches the corpus) |
| Steps | Data: only `Starfield.esm` ticked, no active file; loaded in 125 s. View > Cell View (a toggle); World Space "Interior"; Cell Filter `CityNewAtlantisApartmentTEMPLATE`; double-click loaded it (tooltip `'CityNewAtlantisApartmentTEMPLATE' (0021966F)`). Object Window: category "*All" (about 15 s not responding while it fills), Text Filter `Crate_MilitarySupply_01`: one row, `001E4FBD`, STAT, count 459. Dragged into the Render Window: status bar `Crate_MilitarySupply_01 (01000807)`, count 460. Double-click opened the Reference dialog |
| Reference dialog | Fields: Reference Editor ID, Base Object, Layer, Location Override; tabs 3D Data, Extra, Reflected by, Linked Ref, Linked From, Enable Parent, Emittance, MultiBound, Keywords, Location Ref Type, NavMesh Import Option, Attach Ref, Comments, Scripts; 3D Data has Position X/Y/Z and Rotation X/Y/Z, each with a step box, Scale and Test Radius; 22 flag checkboxes below the tabs (Respawns and Allow Permanent Projected Decals ticked by default). **No "Persistent" checkbox on any tab.** Typed: EditorID `KitKraftFirstPOICrateCK`, position (-17, -4, 4.63), rotation Z 179.9087. OK raised "Rename Confirmation": "Are you sure you want to rename this object? This object is currently in use by 0 objects and 0 references." (Yes) |
| Save | File > Save Plugin opened "Select Target Plugin" (folder `Starfield\Data`, file name `*.esp`, type "TES Plugin Files (*.esp)"); saved as `KitKraftFirstPOI-CK.esp`, 1,810 bytes, into MO2's `overwrite\`. Then File > Convert Active File to ... offers Small Master, Medium Master, Full Master; Full Master asked "Convert the active file to a Master file?" (Yes) and reported "New Master file KitKraftFirstPOI-CK.esm created." It wrote `KitKraftFirstPOI-CK.esm` (1,820 bytes, SHA-256 `7e54e993...8d24b9`) next to the `.esp` and re-saved the `.esp` (2 bytes differ: HEDR's next object ID) |
| Result (`ck.esm` = the CK's `.esm`) | Roundtrips identical. `cargo xtask oracle ck`: fail, 4 unexplained, 1 ours-missing, match 0/1, as the fixture expected (the CK's reference is `000807`, ours `001000`). The CELL override's compressed payload (1,485 bytes) is byte-identical to ours. REFR: NAME `001E4FBD`; DATA position (-17, -4, 4.63) exact; rotation Z stored as 3.13999915 radians (the field takes degrees; ours stores 3.14) |
| Differences from ours | **Persistence:** the CK's reference has record flags 0 and sits in the temporary children group (type 9, field after the label 3); ours has the persistent flag `0x400` in the persistent group (type 8). TES4 of the CK's master: HEDR record count 7, next object ID `01000800` (its `.esp` before conversion: `00000809`); no `DATA` after `MAST`; `BNAM` "Main"; `INCC` value 1 (the CK's `.esp` has no `INCC`, so it appears with the master flag). Groups: version control `0x3561`, as in F30. CELL form version 582 (ours 581) |
| Side effects | `overwrite\` now holds `KitKraftFirstPOI-CK.esp`, `KitKraftFirstPOI-CK.esm` and `Backup\AutoSaveDEFAULT.bak` (1,810 bytes); they are left for the owner to remove (the agent writes nothing under MO2 outside `kit deploy`). The CK's settings files were restored from `F29\settings-before\` |
| Open | M5.A3 classes the differences. Two questions for our writer: whether a placed reference should default to temporary as the CK does (ours is persistent on purpose, `docs/poi/first-poi.toml`), and what `INCC` counts. The interface facts above (Cell View, Object Window, the Reference dialog) may move `docs/ck-parity.md` rows under ADR 0012 Amendment 2, in a change of their own |

### ck-20261101-04: M5.A3, the F29 and F30 differences classed by hand

Desk work on the two sittings above; no CK run. Each difference `cargo xtask oracle ck`
reported is matched to the corpus (`docs/corpus.md` "TES4 headers") or to the sitting that
shows it, and the matches went into each fixture's `fixture.toml` as `[[normalization]]`
entries citing `ck-20261101-02` (F30) or `ck-20261101-03` (F29). The differences below the
oracle's view (group headers, record form versions) are classed from `structure.txt`. The
two references of F29 were compared field by field with `kit inspect --full`, since the CK
gave its reference another FormID and the oracle cannot pair them by ID.

| Field | Content |
| ----- | ------- |
| Id | `ck-20261101-04` |
| Fixtures | F29 and F30 (`observe\ck-2026-11-01\F29\`, `F30\`); `fixture.toml` SHA-256 `7fa00740...b553cd4d` (F29) and `451a1e7e...a7bd49b1` (F30); the versions before the classing kept as `fixture.before-a3.toml` |
| Result | `cargo xtask oracle ck`: **F30 clean** (0 unexplained, 0 ours-missing, 4 explained; match 1/1). **F29 ours-missing** (0 unexplained, 1 ours-missing, 4 explained; no `match`, see REFR below). Both files of both fixtures roundtrip identical |
| TES4 `HEDR` record count | **Explained, convention.** The CK writes records below `TES4` plus groups (F30: 2 + 5 = 7), as the two BlueprintShips files do; ours writes every record including `TES4`, groups not counted, as the other 12 official files do (`docs/corpus.md`; `tes4::majority_record_count`). The CK loaded ours with no warning (`ck-20261101-01`, `-02`) and the game ran it (`game-20261101-04`). Not a defect |
| TES4 `HEDR` next object ID | **Explained, CK bookkeeping.** Ours writes `0xFF001000`, as 12 official files do; the CK writes its own counter: F30 `0x00001007`, F29 `.esp` `0x00000809`, F29 `.esm` `0x01000800` after File > Convert. Our reader masks the high byte (`forms::NewForms`) |
| TES4 `DATA` after `MAST` | **Explained, convention.** Every `MAST` of the official files is followed by an 8-byte zero `DATA`, and ours copies it (`build::HeaderSpec`); the CK's save writes `MAST` alone. The game and the CK read both |
| TES4 `INCC` | **Explained.** The CK writes `INCC` only on a master: F29's `.esp` has none, its `.esm` has value 1, as ours (one interior cell). F30's removal is an artifact of the twin, whose bytes are our master's with only the master flag cleared |
| TES4 `BNAM` | **Explained, CK bookkeeping.** The CK writes its branch name, "Main" (shown in its title bar); 12 of the 14 official files carry a 9-byte `BNAM`, ours none |
| Groups | **Explained, CK bookkeeping.** The CK stamps every group's version control `0x3561` (ours 0, as the official files: `docs/corpus.md` "Cell and world groups" "stamp every group 0") and writes the field after a cell-children group's label: type 6 gets 7, type 8 gets 1 (F30), type 9 gets 3 (F29); ours 0. The group path itself (CELL top, block 9, sub-block 9, cell, children) is the same as ours |
| CELL | **Identical payload.** The override's compressed payload (1,485 bytes) is byte-identical in both fixtures; the CK raised the record's form version 581 -> 582 (ours keeps the vanilla record's 581, since the override is a carried copy) |
| REFR (F30) | **Identical**, header and body (`kit diff`: 2 unchanged) |
| REFR (F29) | Ours `001000`, the CK's `000807` (its first ID in a small-master-capable `.esp`). Subrecords `EDID`, `NAME`, `DATA` in that order in both. `NAME` `001E4FBD` and the `DATA` position (-17, -4, 4.63) are byte-identical. **Explained, input:** `EDID` `KitKraftFirstPOICrateCK` was typed differently on purpose; rotation Z is 4 ULP apart (ours `C3F54840` = 3.14, the CK's `BFF54840` = 3.13999915) because the Reference dialog takes degrees and 179.9087 was typed. **Explained, authoring choice:** ours is persistent (flag `0x400`, group type 8), as `docs/poi/first-poi.toml` asks; the CK's is temporary (flags 0, group type 9) and its Reference dialog has no Persistent checkbox |
| Corpus cross-check | Every layout the CK wrote (`HEDR` 12 bytes, `CNAM`, `MAST`, `BNAM`, `INCC` 4 bytes; `REFR` `EDID`/`NAME`/`DATA` 24 bytes) fits the corpus definitions. All four CK files (F30's, F29's `.esp` before and after the conversion, its `.esm`) roundtrip identical and pass `kit validate` with 0 errors, `layout.undecoded` among the rules; one subrecord is opaque in each, as in ours. The three `.esp` files get one warning each, `cell.incc` (an interior cell and no `INCC`): the rule follows the official files, all masters, and so flags every CK-saved non-master plugin |
| Conclusion | No difference is unexplained, so the differential of Gate K0 ("the CK loads and re-exports the output") is clean. `CELL` and `REFR` move to `ck-differential-tested` **pass** (`docs/schema/evidence.toml`). Raising `REFR` `NAME` and `DATA` (and the `TES4` members the CK wrote) to `verified` confidence is a request to the schema lane, not done here |
| Open | (1) Should a placed reference default to temporary, as the CK does? Ours is persistent by recipe, and both load (owner question, `plan/STATUS.md`). (2) Our writer keeps the official files' header convention (count, next object ID, `MAST`+`DATA`); copying the CK's instead is a choice, not a fix. (3) `validate`'s `cell.incc` should not warn on a non-master plugin, since the CK writes `INCC` only on masters. (4) The oracle pairs records by FormID only; pairing by base, cell and position would let F29 keep a `match` (`docs/oracles.md` "Limits") |
| Filed | `plan/STATUS.md` row 12 and the M5 exit rows; `docs/foothold/gap-matrix.md` (TES4, GRUP, CELL, REFR CK-diff cells; Gate K0); `docs/gaps.md` (the new-plugin group and new-form numbering rows restated) |

### ck-20261102-01: M7.K3, the menu map of `ck-20261101-01` (desk entry; no CK run)

Desk work on 2026-11-02; no CK was launched. In `ck-20261101-01`, after the load, the agent
opened each of the CK's twelve top-level menus in turn, dumped its items through UI
Automation and took a screenshot while it was open. Those files stayed in `observe\` until
the owner accepted ADR 0012 Amendment 2. This entry files them under the Amendment's
decision 2: an item counts only when the dump lists it and the screenshot shows it. Only
labels are repeated here. The dump's automation ids and class names are left out, and it
holds no tooltip or help text. The `docs/ck-parity.md` rows built from it land in the
integrator's menu-sync commit, which also sets the bar's order (`CK_MENU_ORDER`). Those
rows are one per menu, one per item, and the order row reworded as observed.

| Field | Content |
| ----- | ------- |
| Id | `ck-20261102-01` |
| Sitting | `ck-20261101-01` (2026-11-01, agent-run, observation only), after F30's load: `Starfield.esm` and `KitKraftFirstPOI-CKRoundtrip.esm` loaded, no active file. CK 1.16.244.0 (title bar), game 1.16.244.0. The CK's settings files as kept in `observe\ck-2026-11-01\settings-before\` |
| Files | In `observe\ck-2026-11-01\`: `12-menus.txt`, the twelve menus' items (SHA-256 `d5e044a7...c8a9b3c9`), and `shots\12-menu-<menu>.png`, one per menu, taken with the menu open (File `d9c2aec8...a843ff72`, Edit `26778685...53c7684f`, View `06a1e8a9...134eff00`, Docks `495ce1b1...c5be74cc`, Galaxy `6eb810b8...ac64e156`, World `d4c6c1aa...d77649d8`, Terrain `7d75aa09...261b09af`, NavMesh `903acef0...ca9f9f3b`, Character `6a6e4309...a2a5bddb`, Gameplay `14efd232...e53d5655`, Audio `e6533ed7...2a776e18`, Help `ebaaeb7a...ef4a9b79`). For the order also `observe\2026-11-01-ck-uia-probe\uia-tree.json` (SHA-256 `635489c0...45076de4`), the menu bar with nothing loaded |
| Question | M7.K3 (menus): which top-level menus, in which order; each menu's items in order, its separators, and which items are disabled |
| Menu bar | Twelve menus, in this order: File, Edit, View, Docks, Galaxy, World, Terrain, NavMesh, Character, Gameplay, Audio, Help. The probe saw the same twelve in the same order with nothing loaded. The owner's recollection (2026-10-16) named eight of them in the same relative order; Docks, Galaxy, Terrain and Audio are the four it did not name |
| Items | 133, separators not counted: File 13, Edit 12, View 31, Docks 4, Galaxy 5, World 16, Terrain 8, NavMesh 8, Character 15, Gameplay 12, Audio 5, Help 4. Below, `/` marks a separator and *(off)* an item drawn disabled in this state |
| File | Data..., Save Plugin, Import/Export, Preferences..., Theme (experimental), Validate Loaded Data *(off)* / Compact Active File Form IDs for ..., Convert Active File to ... / Create Archive, Upload Plugin and Archive to Bethesda.net *(off)* / Login to Bethesda.net, Logout *(off)* / Exit |
| Edit | Undo, Redo / Cut Render, Copy Render, Paste Render, Paste in Place, Duplicate, Search & Replace... / Find Text, Find by Condition / Render Window Hotkeys, Picking Preferences |
| View | Toggle Render Features, Refresh Render Window / Object Windows, Render Windows, Packin / Toolbar, Cell View, Statusbar, Scene Info Window, Open Windows..., Preview Window, Preview Animations, Show/Hide, Reference Batch Action Window, Layers, Default Layer Manager *(off)*, Object Window Layouts / Material Editor, Material Palette, Material Swap Editor, AVMS / Current Cell Only *(off)* / Isometric, Top / Depth Biasing, Warnings, Inspector, Properties, Particle Editor, Volumes, Reflection Probe |
| Docks | Save Current Layout, Save Layout as..., Recent Layouts, Manage Layouts... |
| Galaxy | Galaxy View, Block Pattern Editor / Generate Locations, Gen Overlay Placement Data, Generate Biome Placements |
| World | World Spaces..., Cells... / Go to Location, World Testing And Reports, View Render Test Failures..., View BetaComment Data / Run Havok Sim, Animate Lighting and Effects / Create Local Maps, Validate Room/Portal Alignment, Align Tanget Space at NIF Intersection *(off; the label as the CK spells it)*, Generate LOS / Generate Max Height Data / Force Weather, Toggle Wind / Export MapMarker Data |
| Terrain | Landscape Editing, Landscape Cutting, Object Palette Editing (Qt), Heightmap Autopaint, Heightmap Import, Export Landscape... / Reimport All Blocks, Cell Terrain Data Viewer |
| NavMesh | Generate Current Cell, Generate Loaded Area, Generate WorldSpace / Cycle View Mode, Draw Cover, Draw Traversal / Other Options / Clean NavMesh Splines |
| Character | Filtered Dialogue... / Export Relationship Data / Export Dialogue..., Export Speech Challenge Scenes..., Export Dialogue For Specific Voices, Export Companion Affinity Event, List Neutral Emotion Dialogue, List Dialogue Needing Review, Quest Voice Asset Totals, Update NPC Body Tint Colors, Update FaceBones Model Availability, Export FaceGen Customization Textures *(off)* / Edit Player Dialogue, Quest Aliases, Generate Player Address Lip Files |
| Gameplay | Settings... / Papyrus Script Manager, Compile Papyrus Scripts..., Migrate Papyrus Property Data..., Export Papyrus Attach Data... / Animations..., Creature Creator, Camera Paths..., Default Objects..., Validate Forms, Recipe Manager, Workshop Editor |
| Audio | Animation Sound Tag Overrides..., Process Local Voice WAVs, Build Soundbank for Active File, Reload Wwise Data, Run FaceFX Compiler |
| Help | Contents, Snap Windows / Third-Party Software Licenses / About |
| Seen in the screenshots only (not counted) | The dump does not carry these, so under decision 2 they are not observed yet. A submenu arrow on 12 items: File > Import/Export, Theme (experimental), Compact Active File Form IDs for ..., Convert Active File to ...; View > Toggle Render Features, Object Windows, Render Windows, Packin; Docks > Recent Layouts; World > World Testing And Reports, Generate Max Height Data; NavMesh > Other Options. A check mark on View > Toolbar, Cell View, Statusbar, Depth Biasing and Warnings. A key shortcut beside 17 items. Below Docks' four items and a separator, the Docks screenshot shows eleven more entries (window names such as Warnings and Cell View) that the dump does not list under Docks. Sitting 1's rest (M7.O2: Docks, shortcuts) observes each of these |
| Not observed | The submenus' contents (none was opened); what any item does (none was invoked); the menus in other states (no data loaded beyond the probe's bar, an active file set) |
| Result | Every item has a row in the ask below, as `observed` with this sitting as its source. The rows are the order row, reworded as observed; the eight existing menu rows, `unresearched` to `observed`; four new menu rows (Docks, Galaxy, Terrain, Audio); and 133 `menu item` rows, each mapped to a catalog id or to `gap:` with its reason |
| Filed | The parity rows and `CK_MENU_ORDER` are an ask to the integrator's menu-sync seam commit (sprint 2026-11-02, report `x-M7.K3`). Then M7.M1 (the bar's items mapped) and M7.K5 (the test over the `menu item` rows) build on them |
