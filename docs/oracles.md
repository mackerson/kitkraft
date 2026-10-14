# External oracles

Third-party tools run as separate programs on our fixtures, so their output can be compared
with ours (dossier gate G3, ADR 0005 Amendment 2 §2a and Amendment 3 §4). The rules:

- **Run, never read.** Spriggit (GPL-3.0) and SF1Edit (MPL-2.0) are
  `external-oracle-only` in `docs/source-registry.md`: their source is never read, and
  Spriggit's YAML is never opened. Its field names and key structure are never used (ADR
  0001 §4, ADR 0005 §3). Neither tool is bundled; both are installed by the owner.
- **Our tools judge.** Every comparison is made with KitCraft's own commands (`kit diff`,
  `kit coverage`, `kit roundtrip --verify`) and the harness's byte and `TES4` diffs. What a
  tool reports is evidence about that tool.
- **A disagreement is evidence, not a verdict.** Each one is classed as *ours* (our reading
  or writing is wrong), *theirs* (the tool's output differs from its input, with our byte
  evidence), or *version skew* (the tool targets another build or revision). It is never
  called corruption without our own byte evidence, and "theirs" says what the bytes show,
  not why the tool does it.
- Only the oracles named in `plan/execution-plan.md` M3 are run: Spriggit and SF1Edit.

The harness is `cargo xtask oracle` (M3.H4, `xtask/src/oracle.rs`); its reports go to
`observe\oracle\` (gitignored) and are summarised here in our own words.

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
$env:KITCRAFT_SPRIGGIT = 'C:\Modding\tools\spriggit\spriggit.ps1'
$env:KITCRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
cargo xtask oracle spriggit --official --synthetic C:\Modding\kitcraft\out\OldSovietMarksmanRifle.esm
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

Not installed (owner task M3.O3; agents do not download executables), so the SF1Edit half
of M3.H4 and M3.B11 is open. `cargo xtask oracle sf1edit` stops with that message.

When it is installed:

1. Record the build (version and, if shown, the commit) and its skew from the pinned
   revision `9fb016884bec138ea6c7b872cec831537d464c3e` (`docs/xedit-reference.md`).
2. Choose the capture method and record it here: SF1Edit's own export or dump features run
   as a program, or a script for its scripting engine written from its user
   documentation only (never from xEdit source), kept outside the repo.
3. Capture record counts per type, FormIDs and EditorIDs as SF1Edit shows them, and its
   own error check on our outputs (the rename plugin, the `kit set` overrides above, the
   overhaul build): gate G3.
4. Add the build, the fixture set and a disagreement table here, in the format above.

Until then the SF1Edit parts are carried (to M4 if M3.O3 does not happen in M3), with this
section as the reason.
