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
2026-10-14. A game update changes these; tests report a size or hash mismatch as "corpus
changed", not as a roundtrip failure, and the manifest is updated in its own commit. Every
corpus test checks the size; the fast roundtrip also hashes its five files, and the full
run hashes all 14 (5.9 s in release, one thread per file).

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

Total about 2.54 GB. Fast tests (`cargo test`) roundtrip the five smallest: `OldMars.esm`,
`SFBGS007.esm`, `Constellation.esm`, `SFBGS004.esm` and `SFBGS047.esm` (the last two bring 4
compressed records and one `XXXX` subrecord, so decompression and `XXXX` run on real data
in every gate; `corpus_small_compressed_and_xxxx` also decodes and touches every record and
edits one inside a compressed record). The mutation tests (M1.7) use the first three only.
The full run is `#[ignore]` and runs in release:

```powershell
cargo test -p kitcraft-esm --release -- --ignored corpus_full --nocapture
```

`cargo test -p kitcraft-esm --release -- --ignored` runs every ignored test: the full
roundtrip, the streaming-edit measurement, the TES4 table, the inventory, the recompression
measurement (about 100 s) and an extended hostile-input run.

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
3. **Hostile input (M1.7):** truncations and seeded bit flips of the three tiny files must
   yield errors, never panics.
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
| 2026-10-14 | 087ab97 | 14/14     | 15.4 s    | 1,379 MiB   | Review round 2 (lane A); manifest SHA-256 of all 14 verified first (5.9 s, not in the wall time); same counts |
| 2026-10-14 | b27d0dc | 14/14     | 16.7 s    | 1,394 MiB   | M2 lane A; every record with a schema definition also checked through it (M2.A3, table below); same counts |

Wall time is the test's own timer (`corpus_full_roundtrip`, release, one thread); peak
memory is the test process's `PeakWorkingSet64` (polled every 100 ms from PowerShell for
the first two rows, read by the test itself at the end since 087ab97). The M1.5 target is
2x the largest top-level group of `Starfield.esm` (the `WRLD` group, 863,580,497 bytes =
823.6 MiB) plus 256 MiB = 1,903 MiB; measured 1,379 MiB, about 1.7x the group. The driver
holds one top-level entry's bytes plus its parsed tree, which borrows subrecord data from
those bytes; output is compared as it is produced and never buffered. The rise from 1,238
MiB comes with `Subrecord` growing from 32 to 40 bytes (it now keeps its absolute source
offset, for absolute error offsets) and includes the 14 hashing threads' 1 MiB buffers.

### Streaming edit (`driver::rewrite_file`)

`corpus_rewrite_edit_in_largest_group` (`cargo test -p kitcraft-esm --release --test corpus
-- --ignored corpus_rewrite --nocapture`) streams `Starfield.esm` through `rewrite_file`,
grows one subrecord by one byte in the first record of the deepest group chain of `WRLD`
(depth 5), and writes to a byte counter.

| Date       | Commit  | Output              | Modified top-level entries | Wall time | Peak working set |
| ---------- | ------- | ------------------- | -------------------------- | --------- | ---------------- |
| 2026-10-14 | 087ab97 | input + 1 byte      | 1 (`WRLD`)                 | 2.6 s     | 1,293 MiB        |

The modified `WRLD` group (823.6 MiB) is not buffered: its size and those of the modified
groups inside it are computed first (unmodified children answer from the sizes they were
read with), its header is written, and its children stream to the sink. Peak memory is the
entry's bytes plus its parsed tree, as for the roundtrip; before this two-pass writer the
whole modified group was assembled in a second buffer before its header could be written.

### Record index (M2.A1) and random access (M2.A2)

`kitcraft_esm::index::index_file` walks group and record headers without building the
tree, reading each record body once (to find its first `EDID`; compressed records are
inflated only until that subrecord). `cargo test -p kitcraft-esm --release --test index --
--ignored --nocapture` (2026-10-14, lane A, warm file cache):

| File                         | Records   | Groups  | EditorIDs | Index time (s) |
| ---------------------------- | --------- | ------- | --------- | -------------- |
| Starfield.esm                | 3,829,247 | 101,341 | 204,930   | 3.33           |
| Constellation.esm            | 57        | 7       | 56        | 0.00           |
| OldMars.esm                  | 39        | 5       | 38        | 0.00           |
| BlueprintShips-Starfield.esm | 1,503,333 | 2,404   | 5,719     | 0.31           |
| BlueprintShips-SFBGS050.esm  | 80,008    | 283     | 217       | 0.02           |
| SFBGS003.esm                 | 65,476    | 1,437   | 3,375     | 0.05           |
| SFBGS004.esm                 | 892       | 58      | 310       | 0.00           |
| SFBGS006.esm                 | 60,922    | 1,177   | 1,121     | 0.01           |
| SFBGS007.esm                 | 63        | 12      | 62        | 0.00           |
| SFBGS008.esm                 | 182       | 45      | 169       | 0.01           |
| SFBGS00D.esm                 | 434,991   | 7,005   | 9,905     | 0.17           |
| SFBGS047.esm                 | 43        | 29      | 36        | 0.00           |
| SFBGS050.esm                 | 485,320   | 6,039   | 10,967    | 0.13           |
| ShatteredSpace.esm           | 1,029,529 | 17,747  | 28,441    | 1.80           |

Record counts (per signature, `TES4` included) equal the streaming parse's, which are the
counts `coverage` reports, on all 14 files. Indexing `Starfield.esm` alone peaks at a
288 MiB working set (the M1.5 bound is 1,903 MiB): the index holds one 56-byte
`RecordLoc` per record plus EditorIDs, and one record body at a time. `read_indexed` on
1,000 seeded
`Starfield.esm` entries (and on every record of the five small files) returns records equal
to the streaming parse's, compressed ones decompressed on both sides.

### Typed decode check (M2.A3)

`RoundtripOptions::typed` decodes every record whose type has a schema definition through
the generated code (`Ctx`: the record's form version and the `TES4` localized flag),
encodes it again and compares subrecord by subrecord; a payload a typed member cannot
decode, or a position where the encoding differs, is a conflict listed with its record,
subrecord index, signature and offset. The full run above (b27d0dc, definitions as of
`main` at 73a001d: TES4, GLOB, GMST, KYWD, WEAP) found none:

| File                         | Typed records | Placed subrecords | Records with conflicts |
| ---------------------------- | ------------- | ----------------- | ---------------------- |
| Starfield.esm                | 11,327        | 51,658            | 0                      |
| Constellation.esm            | 12            | 46                | 0                      |
| OldMars.esm                  | 7             | 31                | 0                      |
| BlueprintShips-Starfield.esm | 1             | 5                 | 0                      |
| BlueprintShips-SFBGS050.esm  | 1             | 7                 | 0                      |
| SFBGS003.esm                 | 169           | 608               | 0                      |
| SFBGS004.esm                 | 69            | 211               | 0                      |
| SFBGS006.esm                 | 3             | 12                | 0                      |
| SFBGS007.esm                 | 5             | 15                | 0                      |
| SFBGS008.esm                 | 2             | 9                 | 0                      |
| SFBGS00D.esm                 | 486           | 1,714             | 0                      |
| SFBGS047.esm                 | 5             | 53                | 0                      |
| SFBGS050.esm                 | 381           | 2,026             | 0                      |
| ShatteredSpace.esm           | 700           | 3,752             | 0                      |

13,168 records and 60,147 placed subrecords in all; every compressed one was decoded (0
unchecked). Most members are still opaque (`Bytes`), so this mostly exercises placement
and order; it becomes a real test of layouts as lane B types them (M2.B2 onwards).
`corpus_small_roundtrip` fails on any conflict in the five small files.

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

The largest decompressed record (3.4 MiB) is well under the 64 MiB default decode cap, and
cannot reach the default cap of 1,000,000 subrecords per decompressed record (3.4 MiB
holds at most about 590,000 six-byte subrecords).

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
  meaning UESP gives for Skyrim. The writer preserves it verbatim unless records or groups
  are added or removed, and then writes the 12-file meaning (`crates/esm/src/tes4.rs`;
  the BlueprintShips variant is a gap, `docs/gaps.md`).

## Container facts confirmed by the corpus

From `corpus_inventory` and the roundtrip above, every UESP (Skyrim page) container fact the
parser relies on holds for all 14 files: 24-byte record and group headers, group size
including its header, record data size excluding it, 6-byte subrecord headers, `XXXX`
carrying the next subrecord's size (all 362 `XXXX`-sized subrecords in uncompressed records
have a 0 size field; the rest of the 12,849 sit inside compressed records), and compressed bodies as a `u32` size plus a zlib stream.
Additional observations:

- `XXXX` occurs exactly when the payload does not fit the `u16` size field: of all 12,849
  `XXXX`-sized subrecords (12,487 inside compressed records, counted by decoding them),
  none has a payload of 0xFFFF bytes or less; the smallest is 65,536 bytes (in
  `Starfield.esm`, `SFBGS050.esm` and `ShatteredSpace.esm`). Per file (total / in compressed
  records): Starfield 10,592 / 10,361; BlueprintShips-Starfield 199 / 93;
  BlueprintShips-SFBGS050 1 / 0; SFBGS003 65 / 60; SFBGS008 21 / 19; SFBGS00D 167 / 161;
  SFBGS047 1 / 1; SFBGS050 99 / 91; ShatteredSpace 1,704 / 1,701; the other five have none.
  **Writer policy (M2.A4).** A subrecord that is not changed is written exactly as read,
  prefix included. A new or changed payload, whether set through the edit API
  (`Record::set_subrecord`, `Subrecord::set_data`) or encoded from a typed value
  (`Subrecord::from_owned`, `Record::set_typed`), gets an `XXXX` prefix exactly when it
  exceeds `u16::MAX` bytes, and then a 0 size field, the form every occurrence above has.
  The `oversized` flag of a typed value is ignored on write (review round 3: honouring it
  wrote `XXXX` before a payload shrunk below 64 KiB, a form the corpus never shows).
  `Subrecord::set_oversize_prefix(true)` still forces a prefix, for experiments only.
  Payloads over `u32::MAX` are an error (`TooLarge`).
- Group types 0 to 10 occur. UESP's Skyrim table stops at 9; type 10 is Starfield-only
  (meaning not documented on a permitted source). The parser treats group types opaquely.
- Groups nest at most 6 deep (the parser's default cap is 16).
- The record header's last `u16` ("unknown" on UESP) is non-zero in nearly every record of
  12 files (3,817,455 of 3,829,247 in `Starfield.esm`) but in only 341 and 33 records of the
  two BlueprintShips files. The group header's `u32` "unknown" takes a single value per file except in the BlueprintShips
  files (5 and 4 distinct values). Both are preserved verbatim.

## Archives (M2.G1-G3)

The 90 official `.ba2` archives (about 148.7 GB) are read in place, never copied (ADR 0006).
`cargo test -p kitcraft-ba2 --release --test corpus -- --ignored corpus_full --nocapture`
lists every archive, checks every record, extracts 100 seeded entries per archive (7,360
entries, 7.3 GB, 16 s on the dev box, 2026-10-14) to their declared sizes, and checks every
texture's chunks against its mip chain and against the chunk plan the packer computes. The
sample checks declared sizes only; byte identity with `Archive2.exe` is the manual
differential run on seven archives recorded in `plan/STATUS.md`. The fast `corpus_small` test lists and extracts
every entry of five small archives in every gate.

| Version | Type | Codec (header field) | Archives | Entries |
| ------- | ---- | -------------------- | -------- | ------- |
| 2       | GNRL | zlib                 | 65       | 1,627,505 |
| 2       | DX10 | zlib                 | 10       | 31,583  |
| 3       | DX10 | LZ4 block (method 3) | 15       | 104,818 |

1,763,906 entries in all, 980,871 with packed data. No GNRL archive is version 3 and no
archive is version 1. Facts the crate relies on (`corpus:` in `crates/ba2`):

- The 8 bytes after the 24-byte header are `u32` 1 and `u32` 0 in every archive; version 3
  adds a `u32` that is 3 in all 15, and every packed v3 chunk decodes as an LZ4 block to
  exactly its declared size. Packed v2 data is a zlib stream.
- Layout: header, every record, the data of every entry in record order with no gaps, then
  the name table, which ends the file. Names are lower-case with `/` separators.
- Name and directory hashes: CRC-32 (reflected polynomial `0xEDB88320`, initial value 0, no
  final XOR) of the lower-cased stem and of the directory with `\` separators; a name
  without `.` is hashed whole with a zero extension field. All 1,763,906 records agree.
- GNRL records: flags `0x00100100` and filler `0xBAADF00D` in every record; data is stored
  (packed size 0) or zlib-packed per archive (meshes, animations, LOD and planet data are
  packed; voices, strings, scripts and interface files are stored).
- DX10 records: the byte after the directory hash is 0, chunk records are 24 bytes, tile
  mode is 8, the cube-map byte is 0 or 1 (25 cube maps, all in `Starfield - Textures05`),
  height precedes width. Chunks cover the mips in order; some chunks are stored rather than
  packed. For every texture the chunk sizes add up to the mip chain of its DXGI format (six
  faces for a cube map). Formats seen: 10, 11, 28, 29, 31, 61, 71, 72, 77, 78, 80, 84, 87,
  95, 98, 99.
- DX10 chunk plan (all 136,401 textures, `check_textures`, 2026-10-14): a cube map is one
  chunk; otherwise each mip with at least 512 x 512 pixels by **area** (`width * height >=
  262,144`) that is not the last mip gets its own chunk while fewer than three exist, and
  the remaining mips share one last chunk. Reading the rule as "both edges at least 512"
  gets 421 textures wrong (1024x256 and the like); the area rule gets none wrong, and
  matches Archive2's packing of `Textures01`/`Textures05` (12,658 textures).
- Limits (2026-10-14): no record has size 0 (no empty files; `Archive2.exe` 1.2.0.1 skips
  zero-length inputs when packing, so `ba2.pack` does too); every name is ASCII (`pack`
  refuses others, `docs/gaps.md`) and passes `relative_path`'s Windows name checks; the
  largest GNRL file is 134,083,688 bytes, the largest DX10 chunk 33,554,592 bytes, the
  largest extraction 134,083,688 bytes. The crate's caps sit above these: 256 MiB per LZ4
  blob (`LZ4_MAX_BLOB`), and callers of `extract_vec` pass their own (64 MiB for `.pex`,
  256 MiB for string tables; `ba2.extract` refuses entries above `max_entry_bytes`,
  1 GiB by default).

Differential checks against `Archive2.exe` 1.2.0.1 (black box): see `plan/STATUS.md`.

## Writing rules (M2.E3)

What a patch plugin written by `set` and `build` follows, and where each rule comes from.
Corpus rules are asserted by `crates/engine/tests/writing_rules.rs` (fast tests in every
gate; `corpus_onam_inventory_full` is `#[ignore]`, release, 2.6 s). The `ck-diff` half
(what the Creation Kit writes for a new non-localized plugin, M2.O4) is not observed yet;
until it is, `set` and `build` name it in `limitations`.

| Rule | Source |
| ---- | ------ |
| **`HEDR` record count**: records including `TES4`, groups not counted (the meaning 12 of the 14 files use; the two BlueprintShips files count records excluding `TES4` plus groups). A new plugin gets this count; a streamed rewrite that adds a record recomputes it (`crates/esm/src/tes4.rs`, lane A). | `corpus:` TES4 header inventory above |
| **`HEDR` version 0.96** in all 14 files: a new plugin writes 0.96. | `corpus:` `corpus_new_plugin_header_facts` |
| **`HEDR` next object ID** `0xFF001000` in 12 of 14 files: a new plugin writes it. Patches add no new forms in M2, so nothing reads it; what the CK writes is a `ck-diff` question. | `corpus:` TES4 header inventory |
| **Form version** of a new `TES4`: 582, the newest in the corpus (`HeaderSpec::new`). A copied record keeps its own form version (581 for `Starfield.esm` records). | `corpus:` TES4 header inventory |
| **Master flag**: every official file is an `.esm` with flag 0x1. A patch written as `.esm` sets 0x1; as `.esp` it does not. `.esl` (and the small/medium bits 0x100/0x400) is refused until M2.D3 (c) has a source. | `corpus:` `corpus_new_plugin_header_facts` |
| **Localized flag**: every official file sets 0x80; a patch never does (M2: no string tables are written). Every lstring of a copied record is written as inline text: the edited value, or the source text looked up in the source plugin's string tables. Lstrings are the schema's, `FULL`/`DESC`, and the corpus lstring table's (`docs/validate.md`, "lstring signature table"); a non-zero 4-byte subrecord of unknown layout from a localized source refuses the copy unless `allow_untyped_lstrings`. | `corpus:` same test; plan "Not in M2"; `corpus:` `lstring_table_matches_the_corpus` |
| **`CNAM` (author) "DEFAULT"**, no `SNAM`: every official file has `CNAM`; the 12 built by the official pipeline say "Python" with `SNAM` "Script", the two BlueprintShips files (not from that pipeline) say "DEFAULT" with no `SNAM`. A new patch follows the BlueprintShips pattern. | `corpus:` `corpus_tes4_metadata` |
| **`INCC` = number of interior cells** (`CELL` records under the top-level `CELL` group), after the masters: true in all 14 files (0 in Constellation, OldMars, SFBGS007; 11,985 in `Starfield.esm`). A new patch writes it (0 unless it copies interior cells); an update adds the interior cells it inserts to an existing `INCC`. | `corpus:` `corpus_tes4_metadata`, `corpus_incc_is_interior_cells_full` |
| **Not written (stated deviation)**: `BNAM` ("Dev_Main", 12 of 14 files: the official branch name), `CHGL` (12 of 14, absent from both BlueprintShips files: a build changelist number), `SNAM` (above). They describe Bethesda's build, which a patch does not have; whether the CK writes them for a new plugin is M2.O4's (`CK_DIFF_PENDING` in `set`/`build` limitations). | `corpus:` `corpus_tes4_metadata` |
| **String ID 0** is in no `starfield_en` table (`.strings` lowest ID 22, `.dlstrings` 3607, `.ilstrings` 7): an lstring holding ID 0 has no text, and is written inline as the empty string. | `corpus:` `corpus_string_id_zero_has_no_text` (`starfield_en` in `Starfield - Localization.ba2`) |
| **`ONAM` is not needed for an override of a top-level record.** In the 5 files with an `ONAM` (SFBGS003, SFBGS006, SFBGS00D, SFBGS050, ShatteredSpace) every listed FormID is a record of the same file in a group of type 7 (topic children: INFO), 9 (cell temporary children: REFR, ACHR, NAVM) or 10 (DIAL, SCEN); no top-level record is listed, although 10 files override 853 top-level master records between them (SFBGS003 58, SFBGS004 20, SFBGS006 15, SFBGS008 8, SFBGS00D 133, SFBGS047 1, SFBGS050 142, ShatteredSpace 86, BlueprintShips 357 and 33). Patches therefore write no `ONAM`. Not every overridden temporary child is listed (SFBGS050 overrides 9 type-9 NAVM records it does not list), so what decides membership is open; a patch that overrides cell children would need it (not in M2). | `corpus:` `corpus_onam_inventory_full` |
| **Inline text encoding**: UTF-8, NUL-terminated (the string tables hold UTF-8, `crates/strings`). Whether the game reads inline FULL as UTF-8 or a code page is a `ck-diff` / in-game fact (M2.O4, M2.O6); ASCII names are unaffected. Every written or inlined lstring with a byte >= 0x80 is named in a limitation. A string set as `hex:` must be one NUL-terminated string. | `corpus:` string tables; open |
