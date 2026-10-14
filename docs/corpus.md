# Roundtrip corpus

The M1 gate is a byte-identical rewrite of every official Starfield ESM. Those files are
Bethesda's, so the corpus is **referenced in place and never copied into the repository**.

## Location

Tests and tools find the game data through `KITCRAFT_DATA_DIR`:

```powershell
$env:KITCRAFT_DATA_DIR = "C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data"
```

When the variable is unset or the directory is missing, corpus tests print one skip line and
pass, so `cargo xtask ci` works on machines without the game. The data directory is read-only
to us: nothing is ever written there. Roundtrip output, when written at all, goes to a temp
directory or `corpus/out/` (gitignored).

## Files

`corpus/manifest.toml` pins each file by size and SHA-256 as installed on the dev box on
2026-10-14. A game update changes these; tests report a hash mismatch as "corpus changed",
not as a roundtrip failure, and the manifest is updated in its own commit.

| File                         | Size (bytes)  | In PLAN.md M1 list        |
| ---------------------------- | ------------- | ------------------------- |
| Starfield.esm                | 1,457,098,709 | yes                       |
| Constellation.esm            | 39,210        | yes                       |
| OldMars.esm                  | 24,358        | yes                       |
| BlueprintShips-Starfield.esm | 303,656,303   | yes                       |
| BlueprintShips-SFBGS050.esm  | 16,136,720    | yes                       |
| SFBGS003.esm                 | 33,279,036    | yes                       |
| SFBGS004.esm                 | 254,945       | yes                       |
| SFBGS006.esm                 | 7,315,458     | yes                       |
| SFBGS007.esm                 | 27,312        | yes                       |
| SFBGS008.esm                 | 11,302,160    | yes                       |
| SFBGS00D.esm                 | 102,054,268   | yes                       |
| SFBGS047.esm                 | 472,022       | yes ("all SFBGS*.esm")    |
| SFBGS050.esm                 | 106,017,249   | yes ("all SFBGS*.esm")    |
| ShatteredSpace.esm           | 501,394,621   | M0 "every ESM under Data" |

Total about 2.54 GB. Fast tests use the three smallest (`OldMars.esm`, `SFBGS007.esm`,
`Constellation.esm`); the full run is `#[ignore]` and runs in release:

```powershell
cargo test -p kitcraft-esm --release -- --ignored corpus_full --nocapture
```

`cargo test -p kitcraft-esm --release -- --ignored` runs every ignored test: the full
roundtrip, the TES4 table, the inventory, the recompression measurement (about 100 s) and an
extended hostile-input run; about 140 s in all on the dev box.

## How it is used

The corpus is a specification source in its own right (`plan/adr/0005-spec-sources-per-fact.md`):
it is the only source for inventory facts (record types and counts, FormIDs, EditorIDs,
subrecord signatures and sizes, header flag values and form versions as they occur), and the
tiebreaker that confirms or rejects every container fact taken from UESP's Skyrim pages and
every layout inferred from CK observation.

1. **Roundtrip (M1):** parse each file, write it back, compare byte for byte. A mismatch is
   reported as file, absolute offset, and group/record path.
2. **Coverage (M1.14):** count record types and subrecord signatures across the corpus and
   compare with schema definitions; output is `docs/record-parity.md`.
3. **Hostile input (M1.7):** truncations and seeded bit flips of the small files must yield
   errors, never panics.
4. **Third-party tool output (restricted):** Spriggit, run as an external binary, can turn a
   file into YAML. Per ADR 0001 §4 that output may be used only for byte-level inventory
   facts: record counts, FormIDs, EditorIDs, subrecord signatures and sizes. Its field names
   and struct layouts are never used to author or check `crates/schema/defs/`, and it is not
   a roundtrip oracle (it lost data on 2 of 5 real mods; see `docs/gaps.md`). Its source is
   never read.

## Run log

Record each full run here: date, commit, files identical/total, wall time, peak memory.

| Date       | Commit  | Identical | Wall time | Peak memory | Notes |
| ---------- | ------- | --------- | --------- | ----------- | ----- |
| 2026-10-14 | 86f7b71 | 14/14     | 17.0 s    | 1,237 MiB   | First full run (lane A, before the main merge); every compressed record also decoded |
| 2026-10-14 | adaf097 | 14/14     | 14.7 s    | 1,238 MiB   | After merging main and switching flate2 to zlib-rs; same counts |

Wall time is the test's own timer (`corpus_full_roundtrip`, release, one thread); peak
memory is the test process's `PeakWorkingSet64`, polled every 100 ms from PowerShell. The
M1.5 target is 2x the largest top-level group of `Starfield.esm` (the `WRLD` group,
863,580,497 bytes = 823.6 MiB) plus 256 MiB = 1,903 MiB; measured 1,238 MiB, about 1.5x the
group. The driver holds one top-level entry's bytes plus its parsed tree, which borrows
subrecord data from those bytes; output is compared as it is produced and never buffered.

### Per-file results (adaf097)

Every compressed record was decompressed and its subrecords parsed (and counted), while
the writer re-emitted the original compressed bytes. Records include `TES4`; subrecords
include those inside compressed records; XXXX counts subrecords whose size came from an
`XXXX` prefix.

| File                         | Identical | Time (s) | Records   | Compressed | Subrecords | XXXX   | Groups  | Largest top-level entry (B) | Largest decompressed (B) |
| ---------------------------- | --------- | -------- | --------- | ---------- | ---------- | ------ | ------- | --------------------------- | ------------------------ |
| Starfield.esm                | yes       | 8.77     | 3,829,247 | 91,149     | 21,306,534 | 10,592 | 101,341 | 863,580,497                 | 3,540,951                |
| Constellation.esm            | yes       | 0.00     | 57        | 0          | 889        | 0      | 7       | 34,369                      | -                        |
| OldMars.esm                  | yes       | 0.00     | 39        | 0          | 708        | 0      | 5       | 22,198                      | -                        |
| BlueprintShips-Starfield.esm | yes       | 1.76     | 1,503,333 | 1,218      | 17,934,465 | 199    | 2,404   | 261,100,738                 | 242,190                  |
| BlueprintShips-SFBGS050.esm  | yes       | 0.08     | 80,008    | 103        | 930,705    | 1      | 283     | 13,914,790                  | 61,922                   |
| SFBGS003.esm                 | yes       | 0.17     | 65,476    | 1,034      | 445,606    | 65     | 1,437   | 22,735,239                  | 2,037,467                |
| SFBGS004.esm                 | yes       | 0.00     | 892       | 2          | 8,926      | 0      | 58      | 112,497                     | 448                      |
| SFBGS006.esm                 | yes       | 0.04     | 60,922    | 360        | 315,830    | 0      | 1,177   | 7,069,775                   | 35,005                   |
| SFBGS007.esm                 | yes       | 0.00     | 63        | 0          | 818        | 0      | 12      | 14,825                      | -                        |
| SFBGS008.esm                 | yes       | 0.06     | 182       | 23         | 1,652      | 21     | 45      | 10,119,060                  | 1,943,916                |
| SFBGS00D.esm                 | yes       | 0.57     | 434,991   | 5,935      | 2,315,716  | 167    | 7,005   | 67,110,960                  | 1,570,504                |
| SFBGS047.esm                 | yes       | 0.00     | 43        | 2          | 1,061      | 1      | 29      | 420,567                     | 650,321                  |
| SFBGS050.esm                 | yes       | 0.49     | 485,320   | 3,184      | 2,511,232  | 99     | 6,039   | 66,137,190                  | 2,746,586                |
| ShatteredSpace.esm           | yes       | 2.70     | 1,029,529 | 33,731     | 4,645,419  | 1,704  | 17,747  | 401,097,113                 | 1,623,608                |

The largest decompressed record (3.4 MiB) is well under the 64 MiB default decode cap.

## TES4 header inventory (M1.2b)

From `corpus_tes4_table` (`cargo test -p kitcraft-esm --release --test corpus -- --ignored
corpus_tes4 --nocapture`). Flags are the raw TES4 record flags.

| File                         | Flags      | Flag bits             | Form version | HEDR version | HEDR count | HEDR next object ID | Masters                                     | Other TES4 subrecords (bytes)                                    |
| ---------------------------- | ---------- | --------------------- | ------------ | ------------ | ---------- | ------------------- | ------------------------------------------- | ---------------------------------------------------------------- |
| Starfield.esm                | 0x00000081 | 0x1, 0x80             | 581          | 0.96         | 3,829,247  | 0xFF001000          | 0                                           | CNAM 7, SNAM 7, TNAM 1048, TNAM 30456, BNAM 9, INCC 4, CHGL 4    |
| Constellation.esm            | 0x00000181 | 0x1, 0x80, 0x100      | 552          | 0.96         | 57         | 0xFF001000          | 1: Starfield.esm                            | CNAM 7, SNAM 7, INCC 4, CHGL 4                                   |
| OldMars.esm                  | 0x00000181 | 0x1, 0x80, 0x100      | 552          | 0.96         | 39         | 0xFF001000          | 1: Starfield.esm                            | CNAM 7, SNAM 7, INCC 4, CHGL 4                                   |
| BlueprintShips-Starfield.esm | 0x00000881 | 0x1, 0x80, 0x800      | 581          | 0.96         | 1,505,736  | 0xFF0CB016          | 1: Starfield.esm                            | CNAM 8, BNAM 9, INCC 4                                           |
| BlueprintShips-SFBGS050.esm  | 0x00000881 | 0x1, 0x80, 0x800      | 582          | 0.96         | 80,290     | 0xFF000802          | 3: Starfield.esm, sfbgs00d.esm, SFBGS050.esm | CNAM 8, BNAM 9, INCC 4                                           |
| SFBGS003.esm                 | 0x00000481 | 0x1, 0x80, 0x400      | 582          | 0.96         | 65,476     | 0xFF001000          | 1: starfield.esm                            | CNAM 7, SNAM 7, ONAM 3624, TNAM 16, TNAM 552, BNAM 9, INCC 4, CHGL 4 |
| SFBGS004.esm                 | 0x00000181 | 0x1, 0x80, 0x100      | 581          | 0.96         | 892        | 0xFF001000          | 1: starfield.esm                            | CNAM 7, SNAM 7, TNAM 8, BNAM 9, INCC 4, CHGL 4                   |
| SFBGS006.esm                 | 0x00000481 | 0x1, 0x80, 0x400      | 581          | 0.96         | 60,922     | 0xFF001000          | 1: Starfield.esm                            | CNAM 7, SNAM 7, ONAM 48, BNAM 9, INCC 4, CHGL 4                  |
| SFBGS007.esm                 | 0x00000181 | 0x1, 0x80, 0x100      | 581          | 0.96         | 63         | 0xFF001000          | 1: Starfield.esm                            | CNAM 7, SNAM 7, BNAM 9, INCC 4, CHGL 4                           |
| SFBGS008.esm                 | 0x00000181 | 0x1, 0x80, 0x100      | 576          | 0.96         | 182        | 0xFF001000          | 1: starfield.esm                            | CNAM 7, SNAM 7, BNAM 9, INCC 4, CHGL 4                           |
| SFBGS00D.esm                 | 0x00000081 | 0x1, 0x80             | 582          | 0.96         | 434,991    | 0xFF001000          | 1: starfield.esm                            | CNAM 7, SNAM 7, ONAM 140, TNAM 2144, BNAM 9, INCC 4, CHGL 4      |
| SFBGS047.esm                 | 0x00000181 | 0x1, 0x80, 0x100      | 582          | 0.96         | 43         | 0xFF001000          | 3: starfield.esm, sfbgs004.esm, sfbgs00d.esm | CNAM 7, SNAM 7, BNAM 9, INCC 4, CHGL 4                           |
| SFBGS050.esm                 | 0x00000081 | 0x1, 0x80             | 582          | 0.96         | 485,320    | 0xFF001000          | 2: starfield.esm, sfbgs00d.esm              | CNAM 7, SNAM 7, ONAM 24724, TNAM 1460, BNAM 9, INCC 4, CHGL 4    |
| ShatteredSpace.esm           | 0x00000081 | 0x1, 0x80             | 582          | 0.96         | 1,029,529  | 0xFF001000          | 1: starfield.esm                            | CNAM 7, SNAM 7, ONAM 12, TNAM 8, TNAM 4096, BNAM 9, INCC 4, CHGL 4 |

Every MAST is followed by an 8-byte DATA. Observations (inventory only; what the bits mean
has no permitted source yet, see `docs/gaps.md`):

- Every file sets 0x1 (master) and 0x80 (localized), the two bits UESP documents.
- Three further bits occur, each alone: 0x100 (6 files, all small), 0x400 (SFBGS003,
  SFBGS006), 0x800 (both BlueprintShips files). 0x200, UESP's Skyrim light-master bit,
  never occurs.
- TES4 form versions: 552, 576, 581, 582. Record form versions across each whole file are
  uniform (552, 576, 581 or 582) except `Starfield.esm`, where 13 records carry older values
  (436, 447, 465, 476, 493, 505, 506, 522, 547) and the other 3,829,234 carry 581.
- HEDR count: equals the number of records including TES4 (groups not counted) in 12 files.
  In the two BlueprintShips files it equals records excluding TES4 plus groups, which is the
  meaning UESP gives for Skyrim. The field is preserved verbatim either way; nothing in the
  container depends on it.

## Container facts confirmed by the corpus

From `corpus_inventory` and the roundtrip above, every UESP (Skyrim page) container fact the
parser relies on holds for all 14 files: 24-byte record and group headers, group size
including its header, record data size excluding it, 6-byte subrecord headers, `XXXX`
carrying the next subrecord's size (all 362 `XXXX`-sized subrecords in uncompressed records
have a 0 size field; the rest of the 12,849 sit inside compressed records), and compressed bodies as a `u32` size plus a zlib stream.
Additional observations:

- Group types 0 to 10 occur. UESP's Skyrim table stops at 9; type 10 is Starfield-only
  (meaning not documented on a permitted source). The parser treats group types opaquely.
- Groups nest at most 6 deep (the parser's default cap is 16).
- The record header's last `u16` ("unknown" on UESP) is non-zero in nearly every record of
  12 files (3,817,455 of 3,829,247 in `Starfield.esm`) but in only 341 and 33 records of the
  two BlueprintShips files. The group header's `u32` "unknown" takes a single value per file except in the BlueprintShips
  files (5 and 4 distinct values). Both are preserved verbatim.
