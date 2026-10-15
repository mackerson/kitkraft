# External oracles

Third-party tools run as separate programs on our fixtures, so their output can be compared
with ours (dossier gate G3, ADR 0005 Amendment 2 §2a and Amendment 3 §4). The rules:

- **Run, never read.** Spriggit (GPL-3.0) and SF1Edit (MPL-2.0) are
  `external-oracle-only` in `docs/source-registry.md`: their source is never read, and
  Spriggit's YAML is never opened. Its field names and key structure are never used (ADR
  0001 §4, ADR 0005 §3). Neither tool is bundled; both are installed by the owner.
- **Our tools judge.** Every comparison is made with KitKraft's own commands (`kit diff`,
  `kit coverage`, `kit roundtrip --verify`) and the harness's byte and `TES4` diffs. What a
  tool reports is evidence about that tool.
- **A disagreement is evidence, not a verdict.** Each one is classed as *ours* (our reading
  or writing is wrong), *theirs* (the tool's output differs from its input, with our byte
  evidence), or *version skew* (the tool targets another build or revision). It is never
  called corruption without our own byte evidence, and "theirs" says what the bytes show,
  not why the tool does it.
- Only the oracles named in `plan/execution-plan.md` M3 are run: Spriggit and SF1Edit.

The harness is `cargo xtask oracle` (M3.H4, `xtask/src/oracle.rs` for Spriggit,
`xtask/src/sf1edit.rs` for SF1Edit); its reports go to `observe\oracle\` (gitignored) and
are summarised here in our own words.

## Spriggit

### Build and method

| Item | Value |
| ---- | ----- |
| Tool | Spriggit CLI `0.41.0+Branch.main.Sha.5a68e919767f04ab06f2aa1a496eabd2da7dec37` (official release zip; `C:\Modding\tools\spriggit\README.md`) |
| Translation package | `Spriggit.Yaml.Starfield` 0.41.0 (a .NET 10 tool; run through the owner's wrapper `C:\Modding\tools\spriggit\spriggit.ps1`, which puts a portable .NET 10 SDK on `PATH`) |
| Masters | `-d` the game Data folder (read only) |
| Run | 2026-10-14, game files as in `corpus/manifest.toml` (recorded 2026-10-14) |
| Method | Each fixture is copied to a temp folder; `serialize` (plugin to YAML), `deserialize` (YAML to plugin); the YAML folder is deleted unread; input and output are compared with `kit` only |

```powershell
$env:KITKRAFT_SPRIGGIT = 'C:\Modding\tools\spriggit\spriggit.ps1'
$env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
cargo xtask oracle spriggit --official --synthetic C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm
```

### Fixture set and results

"Ours identical" is `kit roundtrip --verify` on the input; "Output parses" is `kit
coverage` and `kit roundtrip --verify` on Spriggit's output (both succeed and our roundtrip
of it is identical). Record counts per type are equal in every row.

| Fixture | Set | Bytes in -> out | Identical | Ours identical | Output parses | Records changed (`kit diff`) |
| ------- | --- | --------------- | --------- | -------------- | ------------- | ---------------------------- |
| `OldMars.esm` | official (small) | 24,358 -> 24,358 | no | yes | yes | 4 of 38 |
| `SFBGS007.esm` | official (small) | 27,312 -> 27,312 | no | yes | yes | 40 of 62 |
| `Constellation.esm` | official (small) | 39,210 -> 39,210 | no | yes | yes | 5 of 56 |
| `SFBGS004.esm` | official (small) | 254,945 -> 254,859 | no | yes | yes | 581 of 891 |
| `SFBGS047.esm` | official (small) | 472,022 -> 476,546 | no | yes | yes | 13 of 42 |
| `KC_S01_Empty.esp` | synthetic: `TES4` only, no masters | 66 -> 66 | no | yes | yes | - (no records) |
| `KC_S02_NewGlobal.esp` | synthetic: one new GLOB | 144 -> 144 | no | yes | yes | 0 of 1 |
| `KC_S03_CompressedGlobal.esp` | synthetic: the GLOB compressed (stored zlib blocks) | 159 -> 156 | no | yes | yes | 0 of 1 |
| `KC_S04_RifleRename.esm` | `kit set` rifle rename (same bytes as the M2 rename plugin) | 2,221 -> 2,524 | no | yes | yes | 1 of 1 |
| `KC_S05_GmstOverride.esp` | `kit set` GMST override | 179 -> 165 | no | yes | yes | 0 of 1 |
| `KC_S06_GlobOverride.esp` | `kit set` GLOB override | 186 -> 172 | no | yes | yes | 0 of 1 |
| `OldSovietMarksmanRifle.esm` | the M2 rename plugin (`out\`, SHA-256 `9C0B28A6...4078EA27`) | 2,221 -> 2,524 | no | yes | yes | 1 of 1 |
| M3.O1 fixtures | CK-saved plugins | pending (M3.O1 has not run; rerun with the fixture paths) | | | | |
| The overhaul build | `out\<name>.esm` | pending (M3.E9) | | | | |

The five official files are the five smallest in the corpus (the plan names "the three
small corpus files": Constellation, OldMars, SFBGS007; SFBGS004 and SFBGS047 were added
because they have more record types).

### Disagreements

Every input roundtrips identical through our container, and our tools read every Spriggit
output without an error. All but one are "theirs", stated by what the bytes show; S3 points
back at our writer.

| # | What changes from input to Spriggit output | Where (count) | Our byte evidence | Class | What it means for us |
| - | ------------------------------------------ | ------------- | ----------------- | ----- | -------------------- |
| S1 | 4-byte subrecords holding a non-zero value in a **localized** plugin are written as `00 00 00 00` | all five official files: 885 subrecords (OldMars 4, SFBGS007 204, Constellation 5, SFBGS004 618, SFBGS047 54), e.g. OMOD and WEAP `FULL`, COBJ and PERK `DESC`, INFO `NAM1` (512), RACE `FMRN` (40), GMST `DATA` of string settings (16) | `kit diff` spans; e.g. `OldMars.esm:000003` OMOD `FULL` `CA 26 03 00` -> `00 00 00 00`. All five files set the localized flag `0x80`, and `FULL`/`DESC` there are string IDs (`docs/validate.md`, lstring table); string ID 0 is in no table (`docs/corpus.md`) | theirs | Spriggit 0.41.0 output of a localized plugin has no names or descriptions; never a source for lstring values |
| S2 | `HEDR` record count and next object ID rewritten | every fixture; e.g. OldMars count 39 -> 43, next ID `0xFF001000` -> `0x0000002A`; our new GLOB plugin next ID `0xFF001000` -> `0x00000801`; `TES4`-only plugin count 1 -> 0 | `TES4` diff in the report | theirs | Two more `HEDR` readings to keep apart from the corpus's (`docs/gaps.md` `HEDR` row); the CK's own reading is F01/F06's question (`docs/ck-observations.md`) |
| S3 | `TES4` `DATA` after `MAST` (8 bytes) dropped | our four `kit set` plugins and the M2 rename plugin | `TES4` diff: `DATA[0]: removed`. Our own walk of the 14 official `TES4` records finds **no** `DATA` after any `MAST` (13 files have masters), as the `TES4` schema note says; `docs/corpus.md` ("Every MAST is followed by an 8-byte DATA") and our writer (`HEDR CNAM MAST DATA INCC`) disagree with the corpus | ours (open) | Spriggit's output matches the corpus here and ours does not. Filed to lane E (`plan/STATUS.md` Requests); what the CK writes is F01/F02's question |
| S4 | Records reordered inside their groups (same set of records, same bodies) | OldMars (KYWD, AVMD groups), SFBGS047 (REFR), the other official files | our own walk: sorted record lists equal, order differs; identical inflated bodies for every unchanged record | theirs | Positional byte diffs are noise here; `kit diff` (keyed by FormID) is the measure |
| S5 | Repeated subrecords of one record permuted (counts unchanged) | SFBGS004, SFBGS047 RACE: `ATKE`, `MPGM`, `MPGN`, `MTNM`, `ATKD` entries | `kit diff` changes per occurrence with equal subrecord counts per signature | theirs | Order of repeated subrecords is data we keep; Spriggit does not |
| S6 | GBFM: a second `FLKW` written as an extra `FLFM`; the following `ITMC` 2 -> 3 | SFBGS004 and SFBGS047 (one GBFM each) | subrecord counts `GBFM/FLFM` 2 -> 3, `GBFM/FLKW` 2 -> 1; `ITMC` `02` -> `03` | theirs | Same pattern the tool README found in a third-party plugin (Hatch-22); a content change |
| S7 | Some FE-prefixed FormIDs inside payloads change value | SFBGS004 (110 IPDS `PNAM`, 1 GBFM `FLFM`), SFBGS047 (2 IPDS `PNAM`, 1 GBFM `FLFM`): both small plugins | e.g. `SFBGS047.esm:000027` IPDS `PNAM` `D2 23 02 00 28 10 00 FE` -> `40 2F 01 00 FE CC 15 00` | theirs, or version skew | FormIDs of small plugins are a G5 question (lane D); Spriggit's output is not evidence for their encoding |
| S8 | Floats changed in the last bit; one `-0.0` written as `+0.0` | SFBGS004 LENS `LFSD` (21), MATT `CNAM` (1); SFBGS047 REFR `DATA` (1: bytes 16..19 `00 00 00 80` -> `00 00 00 00`) | e.g. `FC FA 7A 3F` -> `FB FA 7A 3F` | theirs | Not a source of float values |
| S9 | Other small payload changes | SFBGS007: 15 PERK `CTDA` (e.g. the first 4 bytes `00 55 4C 4C` -> zeros), QUST `ANAM` `05` -> `04`, 3 QUST `VMAD` | `kit diff` spans | theirs | - |
| S10 | Compressed records recompressed (same inflated bodies) | SFBGS047 (2 records, stored 419,730 -> 424,254 bytes); our stored-block GLOB (159 -> 156) | inflated bodies identical; `kit diff` reports the GLOB unchanged | theirs | Recompression parity is checked against the corpus and the CK (F07), not Spriggit |
| S11 | The rifle override gains five `FULL`/`OBTS` pairs and its `OBTE` changes | `KC_S04_RifleRename.esm`, `OldSovietMarksmanRifle.esm` | subrecord counts `WEAP/FULL` 8 -> 13, `WEAP/OBTS` 7 -> 12 | theirs | Spriggit fills structures our copy of the record keeps as stored; a G3 note, not a defect of ours |

What this settles for M3: Spriggit 0.41.0 cannot serve as a lossless store or as a value
oracle for Starfield plugins (S1, S6, S7, S8), which agrees with the `docs/gaps.md`
Spriggit row. It is useful as a cross-tool reader: it accepted every plugin our writer
produced (G3: no failures), our tools read every plugin it wrote, and S3 found a `TES4`
subrecord our writer adds that no official file has.

## SF1Edit

### Build and method

| Item | Value |
| ---- | ----- |
| Tool | xEdit **4.1.5f** x64 (file version 4.1.5.0; the GUI log names build `C0DCE642`), installed by the owner at `C:\Modding\tools\xEdit\` (M3.O3). The Starfield editor is `xSFEdit64.exe`; its console dumper `xDump64.exe` (banner `SF1Dump 4.1.5f x64`) is the same build |
| Skew from the pin | 4.1.5f is the 2024-04-28 release, 1,459 commits older than `dev-4.1.6` and older than the pinned `9fb01688...` (`docs/xedit-reference.md`). Every disagreement below is read with that in mind; the game data is the 2026 build (`corpus/manifest.toml`) |
| Capture method | xEdit's own dump feature run as a program: `xDump64.exe -sf1 -D:<scratch>\Data\ -I:<scratch>\Starfield.ini [-dg:SIG,...] <scratch>\Data\<plugin>` prints every record as an indented `Name: value` tree (the values the editor displays); `-check` prints the editor's error check instead. No xEdit source and no xEdit script were read or written |
| Isolation | The plugin, its masters (recursively), and `<name> - Localization.ba2` / `<name> - Main.ba2` are copied to a scratch `Data` under `%TEMP%`; the game's `Starfield.ini` is copied next to it for `-I:`. The game `Data` folder and the MO2 instance are only read. The dumper writes its stdout/stderr (redirected into the scratch folder) and one file outside it: it appends to `xDump64Exception.log` next to its exe whenever it meets an exception (in the 2026-10-14 runs: 7 access violations, none of which cost a dump its output, and X3's error). That log was deleted after the runs; delete it after a rerun too |
| Comparison | Our own tools judge: `kit query <TYPE> --scope all --where '@plugin == "<plugin>"'` lists our records, `kit inspect <form> --plugins <masters..> <plugin>` gives the members we decode (`value`) and the ones we keep opaque. Records are keyed by defining file and object ID (xEdit's load-order FormIDs are mapped back through the plugin's master list; a light plugin's own records show as `FE000xxx`). Per subrecord occurrence of each compared record: **agree**/**disagree** (a member we decode against xEdit's displayed values, converted by kind: text, number with 1e-6 tolerance, FormID, lstring by resolved text or by ID when xEdit has no table, integer 0/1 against a displayed boolean), **gap** (opaque to us, typed by xEdit), **both unknown**, **typed, xEdit unknown** (we decode it, xEdit lists it as unknown), plus what xEdit itself shows as unknown per `TYPE/SUB` |
| Run | 2026-10-14; small official files and synthetic set with every record compared (`--sample 0`, 1,294 s in all); `Starfield.esm` WEAP/AMMO/PROJ/COBJ/OMOD with 50 records per type, evenly spaced by FormID (`--sample 50 --no-check`, 344 s in all) |

```powershell
$env:KITKRAFT_SF1EDIT = 'C:\Modding\tools\xEdit\xSFEdit64.exe'   # its xDump64.exe is run
$env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
cargo xtask oracle sf1edit --official --synthetic C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm --sample 0
cargo xtask oracle sf1edit Starfield.esm --types WEAP,AMMO,PROJ,COBJ,OMOD --sample 50 --no-check
```

### Why not the editor's script mode (where it blocks)

The editor runs Pascal scripts (`-script:<file>.pas`), but not headless in 4.1.5f. Observed
with every path redirected to scratch (`-sf1 -D: -I: -P: -C: -T: -B: -R: -S:`), by reading
window titles and texts only (nothing was clicked):

- `-I:` takes the game ini **file** (`-I:<path>\Starfield.ini`); a folder gives `Fatal:
  Could not find ini` in the log and the program waits.
- With `-script:<name>.pas` the title is `SF1Script 4.1.5f x64` and the program stops at the
  **Module Selection** window, with `-autoload`, `-autoexit`, `-VeryQuickShowConflicts`, the
  plugin named as a positional argument, and `*Starfield.esm`/`*<plugin>` in the `-P:`
  plugins file alike. `-quickedit:<plugin>` is refused: `Script is incompatible with
  quickedit request!`.
- Without `-script`, `-autoload` is honoured (title `... [Auto Load]`), but a fresh settings
  file stops at **A message from the developer**; the setting that suppresses it is written
  only when that window is dismissed, and no switch for it is documented. Nothing was
  clicked, so script mode stays out of the harness. The editor also writes
  `<plugins file>.sf1viewsettings` next to the `-P:` file, and an `xSFEdit64Exception.log`
  next to the exe when it is killed (one appeared and was removed).

The dumper needs none of that: it loads the named plugin and its masters, prints, exits.
`Starfield.esm` loads in about 5 s (memory-mapped), so a dump of a small plugin takes 20-50
s and the five round 3 groups of `Starfield.esm` 39 s; no 20-minute cap was approached.

### Fixture set and results

Records: xEdit / ours, both with the plugin as the defining or overriding file (`TES4`
excluded). Classes count subrecord occurrences over every compared record. Check: findings
of `xDump -check`, grouped by message.

| Fixture | Records xEdit / ours | Only ours | Agree | Disagree | Gap | Both unknown | Typed, xEdit unknown | Check findings |
| ------- | -------------------- | --------- | ----- | -------- | --- | ------------ | -------------------- | -------------- |
| `OldMars.esm` | 38 / 38 | 0 | 21 | 0 | 639 | 42 | 0 | 38 (X2) |
| `SFBGS007.esm` | 42 / 62 | 20 (X1) | 8 | 0 | 351 | 42 | 0 | 0 |
| `Constellation.esm` | 56 / 56 | 0 | 33 | 0 | 791 | 59 | 0 | 56 (X2) |
| `SFBGS004.esm` | 891 / 891 | 0 | 129 | 0 | 7,891 | 898 | 0 | 1,516 (X2, X4, X5) |
| `SFBGS047.esm` | 0 / 42 (X3) | - | - | - | - | - | - | - |
| `KC_S01_Empty.esp` (synthetic) | 0 / 0 | 0 | - | - | - | - | - | 0 |
| `KC_S02_NewGlobal.esp` (synthetic) | 1 / 1 | 0 | 2 | 0 | 0 | 0 | 0 | 0 |
| `KC_S03_CompressedGlobal.esp` (synthetic) | 1 / 1 | 0 | 2 | 0 | 0 | 0 | 0 | 1 (X6) |
| `KC_S04_RifleRename.esm` (`kit set`) | 1 / 1 | 0 | 1 | 0 | 3 | 49 | 11 (X7) | 1 (X7) |
| `KC_S05_GmstOverride.esp` (`kit set`) | 1 / 1 | 0 | 2 | 0 | 0 | 0 | 0 | 0 |
| `KC_S06_GlobOverride.esp` (`kit set`) | 1 / 1 | 0 | 2 | 0 | 0 | 0 | 0 | 0 |
| `OldSovietMarksmanRifle.esm` (M2 rename plugin) | 1 / 1 | 0 | 1 | 0 | 3 | 49 | 11 (X7) | 1 (X7) |
| M3.O1 fixtures, the overhaul build | pending (rerun with their paths when they exist) | | | | | | | |

`Starfield.esm`, the round 3 types (records 50 of each compared):

| Type | Records xEdit / ours | Agree | Disagree | Gap | Both unknown | Typed, xEdit unknown |
| ---- | -------------------- | ----- | -------- | --- | ------------ | -------------------- |
| `WEAP` | 406 / 406 | 231 | 0 | 958 | 759 | 149 (X7) |
| `AMMO` | 150 / 150 | 234 | 0 | 366 | 150 | 0 |
| `PROJ` | 265 / 265 | 0 | 0 | 518 | 282 | 0 |
| `COBJ` | 3,009 / 3,009 | 99 | 1 (X8) | 449 | 102 | 0 |
| `OMOD` | 2,682 / 2,682 | 147 | 0 | 98 | 50 | 0 |

Every record both tools list agrees on signature and EditorID (0 mismatches in all
fixtures), and every FormID matches once xEdit's load-order numbering is mapped back.
Every member we decode today (`EDID`, `FULL`/`DESC` lstrings, `KSIZ`/`KWDA`, GLOB and GMST
values) agrees with xEdit's displayed value wherever xEdit decodes it, except X8.

### Disagreements

| # | What differs | Where (count) | Our byte evidence | Class | What it means for us |
| - | ------------ | ------------- | ----------------- | ----- | -------------------- |
| X1 | xEdit lists no `GPOG`/`GPOF` records (its log: `Error: unknown record type GPOF`) | `SFBGS007.esm`: 2 GPOG, 18 GPOF | `kit coverage` and `kit query` read all 20; the file roundtrips identical | version skew | Record types newer than 4.1.5f; nothing to compare there |
| X2 | `-check` warns on every own record of a light master: `internal file FormID is a HITME: FE0000xx (should be 010000xx)` | OldMars 38, Constellation 56, SFBGS004 871 | The stored FormIDs are `FE0000xx` (`kit inspect` `raw_form_id`, e.g. `OldMars.esm` KYWD `FE000001`) | version skew | The official light files store FE-prefixed FormIDs for their own records; 4.1.5f expects master-index numbering. Evidence for lane D's G5 question, not a defect of ours |
| X3 | The dumper stops: `ESL or Overlay flagged modules can't be masters of other modules in Starfield` (also with `-IgnoreESL`) | `SFBGS047.esm` (its master `SFBGS004.esm` is ESL-flagged) | The game ships it; `kit roundtrip --verify` is identical | version skew | 4.1.5f predates light masters; no xEdit view of this file |
| X4 | `-check`: `lstring ID [...] could not be resolved`; the dump shows `<Error: No strings file for lstring ID ...>` | `SFBGS004.esm` 616 | `kit` resolves them from `SFBGS004 - Main.ba2` (`strings/sfbgs004_en.strings`); with that archive in its `Data`, the dumper still reads only `<name> - Localization.ba2` | theirs | The harness compares those lstrings by ID (all agree) |
| X5 | `-check`: unknown enum values: condition function 966 (7), and 11 `SNAM` subtype names (`VPAW`, `VPEB`, ...) with their numeric subtypes | `SFBGS004.esm` 29 | - | version skew | Values newer than 4.1.5f's definitions |
| X6 | `-check`: `Record Flags -> <Unknown: 18 $12>` | our compressed GLOB | Bit 18 (`0x00040000`) is the container's compressed flag; the record inflates and roundtrips identical | theirs | 4.1.5f has no name for that flag on GLOB |
| X7 | After `Components` (a `BGSQualityUpgrade_Component` it cannot decode), xEdit lists the rest of the WEAP by bare signature (`QUPA`, `FULL`, `KSIZ`, ...), and `-check` reports `Missing required members: DESC, STOP` | 51 of 406 `Starfield.esm` WEAPs carry the component (the rifle `0021BBCD` among them), so also our rename plugin and `KC_S04` | xEdit shows the official record `Starfield.esm:21BBCD` the same way; our override roundtrips identical and its subrecord sequence is the official one | version skew | Not a defect in our writer; G3 is clean on everything else 4.1.5f can read |
| X8 | An lstring text: ours has U+FFFD where xEdit shows `’` | `Starfield.esm` COBJ `22D0FF` `DESC` (1 of 50 sampled) | The table entry holds byte `0x92` (Windows-1252 apostrophe), not UTF-8 | ours (fixed, lane B round 5) | `kitkraft_strings::Entry::text` now shows a non-UTF-8 entry as Windows-1252 (bytes unchanged in `Resolved.bytes`), as xEdit does; not rerun since |

### Rerun after the schema integration (M4.BX1, 2026-10-14)

The integrator reran the five small official files after merging the quest, actor and
items/workshop families (`cargo xtask oracle sf1edit --official --sample 0`, 14.5 min; the
dumper's `xDump64Exception.log` was deleted afterwards). Same build and method as above:

| Fixture | Records xEdit / ours | Agree | Disagree | Gap | Both unknown | Typed, xEdit unknown | Check findings |
| ------- | -------------------- | ----- | -------- | --- | ------------ | -------------------- | -------------- |
| `OldMars.esm` | 38 / 38 | 45 (was 21) | 10 | 605 (was 639) | 31 | 11 | 38 (X2) |
| `SFBGS007.esm` | 42 / 62 | 211 (was 8) | 74 | 74 (was 351) | 9 | 33 | 0 |
| `Constellation.esm` | 56 / 56 | 63 (was 33) | 11 | 750 (was 791) | 45 | 14 | 56 (X2) |
| `SFBGS004.esm` | 891 / 891 | 4,644 (was 129) | 2,580 | 796 (was 7,891) | 687 | 211 | 1,516 (X2, X4, X5) |
| `SFBGS047.esm` | - (X3) | - | - | - | - | - | - |

Gaps fell from 9,672 to 2,225 occurrences, mostly because INFO, DIAL, QUST, PERK, SPEL,
MGEF, RACE, NPC_, IDLE, MSTT and FURN are typed now. The new **disagreements** are, in
every example the report lists, display conversions the comparator does not make, not
different values: enum and flag fields (ours a number, xEdit a name or a flag list, e.g.
QUST `DNAM` Flags 273 = "Start Game Enabled, Starts Enabled, Run Once", INFO `INAM` 0 =
"Low", COBJ `LRNM` 3), structs holding an opaque or union field (CTDA, VMAD, ACBS Level,
TRDA Emotion), Wwise GUIDs (ours raw, xEdit `{...}`) and signature-valued fields (DIAL
`SNAM` `IDAT` = "Shared Info"). Teaching the comparator enum, flag and GUID display (lane H)
would turn them into agreements; until then they are not counted as evidence either way.
No compared record names an object ID below 0x800 in Starfield.esm's index space, so this
run says nothing about how SF1Edit shows engine-reserved forms (ADR 0005 Amendment 4).

### Coverage gaps xEdit exposes

A gap is a subrecord we keep opaque that xEdit shows as named, typed fields. Over the four
small official files there are 370 `TYPE/SUB` pairs with gaps (9,672 occurrences; most in
INFO 5,737, AVMD 1,323, RACE 628); record types without any definition of ours (AVMD,
PROJ, INFO, ...) show even `EDID` and `FULL` as gaps. For the round 3 types in
`Starfield.esm`:

| Type | Subrecords xEdit types that we keep opaque | Unknown to both | Typed by us, unknown to xEdit |
| ---- | ------------------------------------------ | --------------- | ----------------------------- |
| `WEAP` | `ANAM APPR BAMT BFCB BFCE BIDS BNAM DAMA ETYP INRD MO4S MOD4 MODL NNAM OBND OBTE OBTF OBTS PDSH PTT2 PUSH QNAM STOP WABB WAIM WAM2 WAUD WCHG WDMG WFIR WFLG WGEN WMEL WRLO WVAR WVIS` | `FLLD ODTY OPDS PTCL QUPA REFL WTRM WTUR` (and every subrecord after `Components` in X7's 48 records) | `FULL DESC KSIZ KWDA` in X7's records |
| `AMMO` | `DATA DNAM MODL NAM1 OBND ONAM PDSH PTT2 PUSH` | `FLLD ODTY` | - |
| `PROJ` | `ANAM BFCB BFCE BNAM DEST DSTD DSTF EDID FULL MODL NAM1 NAM3 OBND PRAS PRCS PROD VNAM` (no PROJ definition yet) | `DATA FLLD ODTY OPDS PTCL REFL XALG` | - |
| `COBJ` | `BFCB BFCE BNAM CNAM CTDA CUSH DATA FLTR FNAM FVPA LRNM NNAM PDSH PUSH RQPK SNAM` | `INTV RECF TNAM` | - |
| `OMOD` | `BFCB BFCE DATA FNAM MNAM MODL` | `FLLD` | - |

xEdit itself shows as unknown (any `Unknown` field, `Unknown N` enum value or raw bytes),
over all records of the type, in every occurrence: WEAP `FLLD ODTY QNAM WAIM WAM2 WAUD
WDMG WFIR WFLG WMEL WRLO WTRM WTUR WVIS`, AMMO `FLLD ODTY PTT2`, PROJ `DATA FLLD ODTY PROD
PTCL REFL XALG`, COBJ `FVPA INTV RECF RQPK TNAM` (and `CTDA` in 2,524 of 2,536), OMOD
`DATA FLLD`. These are where 4.1.5f is no reference: the pinned definitions
(`docs/xedit-reference.md`) and the corpus decide them (M3.B1..B7), and M3.B11 reruns this
comparison as members get typed.

What this settles for M3: SF1Edit 4.1.5f runs headless through its dumper; it reads every
official and kit-written plugin whose master structure it supports, agrees with every
member we decode except one lossy lstring of ours (X8), and its error check finds nothing
in our written plugins beyond X6 and X7, both traced to its age with our byte evidence.
It is too old to judge GPOG/GPOF, light masters or the newer WEAP components (X1, X3,
X7); a build at the pin would narrow those (owner decision 2: the pinned revision).
