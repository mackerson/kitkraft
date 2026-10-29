# Roundtrip corpus

The M1 gate is a byte-identical rewrite of every official Starfield ESM. Those files are
Bethesda's, so the corpus is **referenced in place and never copied into the repository**.

## Location

Tests and tools find the game data through `KITKRAFT_DATA_DIR`:

```powershell
$env:KITKRAFT_DATA_DIR = "C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data"
```

The commands use it too: it is the default `data_dir` of every command that takes one
(`docs/mcp.md`, "Load-order arguments"). When the variable is unset or the directory is
missing, corpus tests print one skip line and pass, so `cargo xtask ci` works on machines without the game. The data directory is read-only
to us: nothing is ever written there. Roundtrip output, when written at all, goes to a temp
directory or `corpus/out/` (gitignored).

### Environment

Every variable is spelled `KITKRAFT_*`: `KITKRAFT_DATA_DIR`, `KITKRAFT_CACHE_DIR` (index
cache, default `%LOCALAPPDATA%\kitkraft\cache`), `KITKRAFT_SF1EDIT`, `KITKRAFT_SPRIGGIT`,
`KITKRAFT_KIT`, `KITKRAFT_PAPYRUS_COMPILER`, `KITKRAFT_UPDATE_GOLDEN` and the test-only
ones named next to their tests.

**Detected defaults (Lane F, 2026-10-15).** Commands no longer need `KITKRAFT_DATA_DIR`:
with no `data_dir` argument and no variable, the Data dir is the one `kit doctor`
detects (Steam's registry and libraries, the common install folders, then the MO2
instance's `gamePath`; `crates/engine/src/host.rs`, `crates/io/src/host.rs`). Precedence:
**argument > environment variable > detected**. A bare `profile` name (`KitKraft Dev`)
is that profile of the default MO2 instance (`KITKRAFT_MO2_DIR`, else the detected one;
the same for `kit doctor`, `profiles` and `kit install`). `KITKRAFT_DETECT=0` (or `off`) turns the
detected defaults off; tests that need "no Data dir" set it. The **corpus tests** are
unchanged: they still read the official files only through `KITKRAFT_DATA_DIR` and skip
without it, so `cargo xtask ci` behaves the same on every machine.

**Deprecated (one release):** before the KitKraft rename (2026-10-14) these were spelled
`KITCRAFT_*`. The old names are still read when the new one is unset, so existing shells
keep working; the new name wins when both are set. Rename them in your profile; the
fallback is removed in the release after this one. The index cache moved from
`%LOCALAPPDATA%\kitcraft\cache` to `%LOCALAPPDATA%\kitkraft\cache` and is **not**
migrated: it is rebuilt on first use (entries are only a speed-up), and the old folder can
be deleted by hand.

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
edits one inside a compressed record). The bit-flip tests (M1.7) use the first three only;
the structure-aware mutation tests (M3.A5) use all five ("Hostile input" below).
The full run is `#[ignore]` and runs in release:

```powershell
cargo test -p kitkraft-esm --release -- --ignored corpus_full --nocapture
```

`cargo test -p kitkraft-esm --release -- --ignored` runs every ignored test: the full
roundtrip, the streaming-edit measurement, the TES4 table, the inventory, the recompression
measurement (about 100 s), the list of its misses (M3.A1) and an extended hostile-input
run.

## How it is used

The corpus is a specification source in its own right (`plan/adr/0005-spec-sources-per-fact.md`):
it is the only source for inventory facts (record types and counts, FormIDs, EditorIDs,
subrecord signatures and sizes, header flag values and form versions as they occur), and the
tiebreaker that confirms or rejects every container fact taken from UESP's Skyrim pages and
every layout inferred from CK observation.

1. **Roundtrip (M1):** parse each file, write it back, compare byte for byte. A mismatch is
   reported as file, absolute offset, and group/record path.
2. **Coverage (M1.14):** count record types and subrecord signatures across the corpus and
   compare with schema definitions; output is `docs/record-parity.md`. Its counts are
   reconciled by `corpus_counts_match_both_documents` (`crates/schema/tests/reconcile.rs`,
   in every gate when `KITKRAFT_DATA_DIR` is set, 2 s in debug): a header walk of its own, not
   `kitkraft_esm`'s parser, recounts records per type and per file and checks them
   against `docs/record-parity.md` and the gap matrix headline.
3. **Hostile input (M1.7, M3.A5):** truncations and seeded bit flips of the three tiny files,
   and structure-aware mutations of the five small ones, must yield typed errors, never
   panics ("Hostile input" below).
4. **Third-party tool output (restricted):** Spriggit, run as an external binary, can turn a
   file into YAML. Per ADR 0001 §4 that output may be used only for byte-level inventory
   facts: record counts, FormIDs, EditorIDs, subrecord signatures and sizes. Its field names
   and struct layouts are never used to author or check `crates/schema/defs/`, and it is not
   a roundtrip oracle (it lost data on 2 of 5 real mods; see `docs/gaps.md`). Its source is
   never read.

## Hostile input

All of it is in `crates/esm/tests/hostile.rs`, through one harness
(`tests/common/hostile.rs`). For every input: nothing panics; `Plugin::parse`,
`Record::decode` on every compressed record, the plain, strict-decode (`roundtrip
--decode`) and typed roundtrips, the index and every indexed read either succeed or return
a typed error whose offset lies in the input and is not `Internal`; anything that parses
writes back byte-identically through `to_bytes` and through the plain driver roundtrip
(lossless even for weird-but-parseable input); a strict-decode mismatch may only name a
compressed body; anything the index accepts parses with the same record count.

**Bit flips and truncations (M1.7):** every truncation and every bit of the first 400
bytes of a synthetic fixture, 1,000 seeded flips per fixture, and for the three tiny
files every record-boundary truncation plus 1,000 seeded flips (seed `0xC0FFEE`). The
`#[ignore]`d `corpus_hostile_extended` adds every byte truncation, 50,000 flips and 20,000
1-8 byte overwrites per tiny file.

**Structure-aware mutation (M3.A5, stable Rust; cargo-fuzz needs nightly).**
`tests/common/mutate.rs` walks the parsed tree of a valid file and records every
structural field: record data size, flags, form version; group size, label, type;
subrecord size; the three fields of an `XXXX` prefix (its own size, the `u32` it carries,
the next subrecord's size field); the decompressed-size prefix of each compressed body;
the `HEDR` record count. Each input gets 0 to 3 field overwrites (kind first, then site,
so rare kinds get equal weight; values around the original: 0, small and header-sized
values, off by one, halved, doubled, the type's maximum, its sign bit, the 64 MiB decode
cap, random; flags toggle compressed `0x40000` and deleted `0x20`) and, with probability
2/5 (always, when there is no overwrite), one splice that keeps every enclosing size
consistent so the input gets past framing: a subrecord or an entry duplicated or dropped,
an entry wrapped in 1 to 70 extra groups (past the depth limits of 16 and 64), or a
compressed body inflated, changed (an inner size or `XXXX` value, a subrecord duplicated or
dropped, junk appended), deflated again (levels 0, 1, 6, 9) with a right or wrong declared
size, a cut stream or trailing bytes, or replaced by a subrecord bomb (1,000,001 empty
subrecords, one over `DecodeLimits::max_subrecords`, in a few KiB of stream). The RNG is
the harness's xorshift64*, seeded per input with `0xA5A5_0000 ^ len`. A further check
catches offsets counted from the wrong base: no error may point before the first
top-level entry the mutation touched.

`cargo test` runs 400 mutations per synthetic fixture and 100 per small file (10 to 20 s
in debug). The full run, 2,000 per input (`KITKRAFT_MUTATIONS` overrides the count):

```powershell
cargo test -p kitkraft-esm --release --test hostile -- --ignored mutation --nocapture
```

| Input (2026-10-15, release, one thread) | Field sites | Parse | Strict-decode faults | Parse errors | Strict-decode errors after a parse | Time |
| --- | --: | --: | --: | --- | --- | --: |
| synthetic `small_fixture` (2 compressed, `XXXX`) | - | 993 | 240 | `UnexpectedEof` 756, `BadOversize` 167, `TooDeep` 60, `GroupTooSmall` 24 | `DecompressedTooLarge` 103, `TooManySubrecords` 6 | 0.9 s |
| synthetic `sample_plugin` (70,000-byte `XXXX`) | - | 926 | 0 | `UnexpectedEof` 768, `BadOversize` 179, `TooDeep` 92, `GroupTooSmall` 35 | `DecompressedTooLarge` 98 | 0.2 s |
| `OldMars.esm` | 841 | 1,052 | 0 | `UnexpectedEof` 817, `TooDeep` 91, `GroupTooSmall` 40 | `DecompressedTooLarge` 127 | 0.4 s |
| `SFBGS007.esm` | 1,044 | 1,089 | 0 | `UnexpectedEof` 772, `TooDeep` 91, `GroupTooSmall` 48 | `DecompressedTooLarge` 143 | 0.8 s |
| `Constellation.esm` | 1,082 | 1,078 | 0 | `UnexpectedEof` 790, `TooDeep` 89, `GroupTooSmall` 43 | `DecompressedTooLarge` 125 | 0.5 s |
| `SFBGS004.esm` (2 compressed) | 11,744 | 1,167 | 356 | `UnexpectedEof` 708, `TooDeep` 76, `GroupTooSmall` 49 | `DecompressedTooLarge` 173, `TooManySubrecords` 11 | 10.4 s |
| `SFBGS047.esm` (2 compressed) | 1,265 | 1,174 | 271 | `UnexpectedEof` 740, `TooDeep` 55, `GroupTooSmall` 31 | `DecompressedTooLarge` 183, `TooManySubrecords` 9 | 12.4 s |

Result: 14,000 mutated inputs, no panic, no `Internal` error, no error offset outside
the input or before the mutated entry, no lossy rewrite. Peak working set of the test
process 21 MiB (the subrecord bomb's 6 MiB inflation is the largest allocation); slowest
single input 125 ms (a bomb, inflated by each decode path). Whole run 25.8 s. No crash was
found, so no regression test was added; the M1.7 hardening (checked sizes, allocations
capped by remaining bytes, depth cap, `DecodeLimits`) held. `DecompressedTooLarge` after
a parse is mostly a raw record whose flags gained the compressed bit: its first subrecord
signature read as a size is far over the 64 MiB cap. The five corpus files have no `XXXX`
outside a compressed body (the mutator found no such field), so top-level `XXXX` fields
are covered by the synthetic fixtures and inner ones by the recompress splice.

The plan row's long run, `KITKRAFT_MUTATIONS=100000` (same seeds, 2026-10-15, release; its
first minutes shared the machine with a `cargo xtask ci`), was clean too: 700,000 inputs in
24.7 min, peak working set 23 MiB, slowest single input 353 ms (a bomb).

| Input | Parse | Strict-decode faults | Parse errors | Strict-decode errors after a parse | Time |
| --- | --: | --: | --- | --- | --: |
| synthetic `small_fixture` | 49,998 | 11,878 | `UnexpectedEof` 37,036, `BadOversize` 8,239, `TooDeep` 3,211, `GroupTooSmall` 1,507, `DanglingOversize` 9 | `DecompressedTooLarge` 5,305, `TooManySubrecords` 489 | 109 s |
| synthetic `sample_plugin` | 46,975 | 6 | `UnexpectedEof` 37,707, `BadOversize` 8,701, `TooDeep` 5,039, `GroupTooSmall` 1,568, `DanglingOversize` 10 | `DecompressedTooLarge` 4,434 | 8 s |
| `OldMars.esm` | 53,459 | 0 | `UnexpectedEof` 39,896, `TooDeep` 4,416, `GroupTooSmall` 2,229 | `DecompressedTooLarge` 6,390 | 25 s |
| `SFBGS007.esm` | 53,939 | 0 | `UnexpectedEof` 39,513, `TooDeep` 4,344, `GroupTooSmall` 2,204 | `DecompressedTooLarge` 6,471 | 55 s |
| `Constellation.esm` | 53,551 | 0 | `UnexpectedEof` 39,936, `TooDeep` 4,264, `GroupTooSmall` 2,249 | `DecompressedTooLarge` 6,386 | 29 s |
| `SFBGS004.esm` | 58,599 | 17,485 | `UnexpectedEof` 36,017, `TooDeep` 3,334, `GroupTooSmall` 2,050 | `DecompressedTooLarge` 8,129, `TooManySubrecords` 525 | 507 s |
| `SFBGS047.esm` | 58,694 | 14,318 | `UnexpectedEof` 36,492, `TooDeep` 2,906, `GroupTooSmall` 1,908 | `DecompressedTooLarge` 8,166, `TooManySubrecords` 554 | 748 s |

The 6 strict-decode faults of `sample_plugin`, which has no compressed record, are raw
records whose flags gained the compressed bit and whose first four bytes happen to read as
a size under the cap.

## Run log

Record each full run here: date, commit, files identical/total, wall time, peak memory.

| Date       | Commit  | Identical | Wall time | Peak memory | Notes |
| ---------- | ------- | --------- | --------- | ----------- | ----- |
| 2026-10-14 | 86f7b71 | 14/14     | 17.0 s    | 1,237 MiB   | First full run (lane A, before the main merge); every compressed record also decoded |
| 2026-10-14 | adaf097 | 14/14     | 14.7 s    | 1,238 MiB   | After merging main and switching flate2 to zlib-rs; same counts |
| 2026-10-14 | 087ab97 | 14/14     | 15.4 s    | 1,379 MiB   | Review round 2 (lane A); manifest SHA-256 of all 14 verified first (5.9 s, not in the wall time); same counts |
| 2026-10-14 | b27d0dc | 14/14     | 16.7 s    | 1,394 MiB   | M2 lane A; every record with a schema definition also checked through it (M2.A3, table below); same counts |
| 2026-10-14 | M3.A2   | 14/14     | 18.3 s    | 1,397 MiB   | Strict decode (`roundtrip --decode` in the driver): every compressed body inflated to exactly its declared size, its stream ending with end marker and checksum at the body's end (no trailing bytes in any of the 136,741), and its subrecords re-encoded to the inflated bytes; typed check on (definitions of 52e852a); same counts |
| 2026-10-14 | M4 lane A | 14/14   | 18.8 s    | 1,397 MiB   | After M4.A1..A6 and the main merge; the typed check now reads `Record::extra()` for every defined type (lane F's request): still 0 conflicts on 20,437 typed records; same counts |

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

`corpus_rewrite_edit_in_largest_group` (`cargo test -p kitkraft-esm --release --test corpus
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

`kitkraft_esm::index::index_file` walks group and record headers without building the
tree, reading each record body once (to find its first `EDID`; compressed records are
inflated only until that subrecord). `cargo test -p kitkraft-esm --release --test index --
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

ACTOR family (M4.BC1, 2026-10-14, release, definitions of the family branch with the
generated code regenerated): `corpus_full_roundtrip` reports 14/14 identical, 60,077 typed
records, 2,155,517 placed subrecords, 0 unchecked and 0 conflicts (26.8 s for the whole
test). Every one of the 8,447 NPC_ records is compressed; an out-of-tree pass that also
requires an empty `extra` decodes and re-encodes all of them in 5.4 s (all 14 files,
decompression with the default `DecodeLimits` included). Per-type counts are in
`docs/schema/actor-family.md`.

### Typed reader against the generated decoder, and lstring IDs (M3.D6)

`inspect`, `where` and `diff` decode payloads through `kitkraft_doc::typed` (an interpreter
over lane B's schema), not through the generated code behind `Plugin::typed`.
`crates/doc/tests/typed_corpus.rs` checks that the two read the same values: on every
record of a defined type, the FormIDs and string IDs the interpreter reads from the
subrecords the generated decoder placed equal those the generated record's visitors list
(`form_ids()`, `lstrings()`), and every such subrecord decodes. The same walk looks up every
lstring ID (FULL and DESC on every record type, and the schema's `lstring` members, which
adds the GMST `s` settings) through `kitkraft_doc::strings`, as `inspect` does (English,
2026-10-14, release, 24 s for all 14 files; the five small files run in the fast suite):

| File | Typed records | FormIDs compared | String IDs compared | lstrings | Resolved | ID 0 | Unresolved |
| ---- | ------------- | ---------------- | ------------------- | -------- | -------- | ---- | ---------- |
| Starfield.esm | 17,395 | 2,718 | 12,179 | 49,550 | 43,366 | 6,099 | 85 (ARMO FULL) |
| Constellation.esm | 19 | 0 | 12 | 12 | 5 | 7 | 0 |
| OldMars.esm | 12 | 0 | 9 | 9 | 3 | 6 | 0 |
| BlueprintShips-Starfield.esm | 0 | 0 | 0 | 1,126 | 1,126 | 0 | 0 |
| SFBGS007.esm | 4 | 0 | 0 | 39 | 29 | 10 | 0 |
| SFBGS008.esm | 1 | 0 | 0 | 79 | 79 | 0 | 0 |
| SFBGS006.esm | 10 | 0 | 8 | 374 | 366 | 8 | 0 |
| SFBGS003.esm | 203 | 27 | 80 | 2,481 | 1,422 | 1,059 | 0 |
| ShatteredSpace.esm | 1,052 | 325 | 776 | 3,377 | 2,944 | 433 | 0 |
| SFBGS004.esm | 69 | 8 | 21 | 66 | 62 | 4 | 0 |
| SFBGS00D.esm | 666 | 32 | 329 | 2,028 | 1,664 | 364 | 0 |
| SFBGS050.esm | 987 | 181 | 974 | 2,312 | 1,884 | 428 | 0 |
| BlueprintShips-SFBGS050.esm | 0 | 0 | 0 | 96 | 0 | 0 | 96 (30 GBFM, 66 LCTN FULL) |
| SFBGS047.esm | 5 | 9 | 3 | 16 | 11 | 5 | 0 |

20,423 typed records agree; no placed subrecord fails the interpreter, and none the
interpreter decodes was left raw by the generated matcher. Of 61,565 lstrings, 52,961
resolve, 8,423 hold ID 0 (no string; in no table), and 181 do not resolve:
85 ARMO FULL IDs of Starfield.esm are in none of its English tables (`ARMO:00003A77` FULL
`0x0003145A` is the first), and BlueprintShips-SFBGS050.esm is localized but ships no
string tables (no `BlueprintShips-SFBGS050 - *.ba2`, unlike BlueprintShips-Starfield); 30
of its 96 IDs exist in SFBGS050's tables, which may be coincidence (lookups never fall back
to another plugin's tables). The ignored test pins these 181.

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

### Recompression misses (M3.A1)

The 160 compressed records that zlib-rs level 9 does not recompress to their original bytes
(`docs/gaps.md`, "Recompression parity", which defines the classes and lists the
compressor settings tried). `cargo test -p kitkraft-esm --release --test recompress --
--ignored corpus_recompression_misses --nocapture` prints the first seven columns (43 s on
the dev box, 2026-10-14); the class and the token comparison come from the out-of-tree
study. Offsets are of the record header; "Stream" is the zlib stream after the `u32`
decompressed size; "First differing stream byte" counts from the start of that stream.

| File | Type | FormID | Offset | Inflated | Stream | First differing stream byte | Class | Where the LZ77 tokens differ |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- | --- |
| Starfield.esm | NPC_ | 0000697A | 21,972,552 | 3,858 | 1,468 | 1,461 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0000961A | 21,992,349 | 5,824 | 2,084 | 2,077 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00017076 | 22,133,340 | 5,857 | 1,997 | 1,990 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00017083 | 22,135,365 | 4,878 | 1,836 | 1,830 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00017085 | 22,137,229 | 4,915 | 1,643 | 1,636 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00017086 | 22,138,900 | 4,921 | 1,632 | 1,625 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0001708E | 22,140,560 | 3,992 | 1,471 | 1,464 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 000831B3 | 22,721,139 | 3,901 | 1,401 | 1,395 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 000937FC | 22,746,097 | 4,817 | 1,614 | 1,607 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 000937FF | 22,747,739 | 4,811 | 1,611 | 1,605 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0016B419 | 24,002,503 | 5,860 | 2,119 | 2,112 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0016B41F | 24,004,650 | 3,893 | 1,439 | 1,433 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00182E00 | 24,141,666 | 714 | 383 | 377 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00191D6A | 24,283,198 | 4,612 | 1,583 | 1,576 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00191D6B | 24,284,809 | 4,004 | 1,494 | 1,487 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00196BB2 | 24,314,613 | 1,379 | 667 | 4 | E | last token: match 31 at distance 33 (level 9: 32) |
| Starfield.esm | NPC_ | 001C3895 | 24,698,405 | 3,011 | 1,251 | 1,244 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D0452 | 24,777,274 | 5,783 | 2,001 | 1,995 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D0453 | 24,779,303 | 3,875 | 1,473 | 1,466 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D0454 | 24,780,804 | 4,429 | 1,526 | 1,519 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D0455 | 24,782,358 | 4,819 | 1,601 | 1,594 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D0457 | 24,783,987 | 4,829 | 1,643 | 1,637 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D0458 | 24,785,658 | 5,035 | 1,705 | 1,698 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D0459 | 24,787,391 | 4,285 | 1,530 | 1,523 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D045A | 24,788,949 | 6,079 | 2,053 | 2,047 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D045B | 24,791,030 | 6,206 | 2,106 | 2,099 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001D045C | 24,793,164 | 3,327 | 1,376 | 1,369 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001EA2BB | 25,063,331 | 723 | 384 | 378 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001EA2C3 | 25,066,577 | 784 | 404 | 397 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001F678B | 25,156,061 | 4,920 | 1,647 | 1,640 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 001F678C | 25,157,736 | 5,779 | 1,991 | 1,984 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00203BC7 | 25,202,977 | 5,880 | 2,034 | 2,027 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00203BC8 | 25,205,039 | 3,266 | 1,370 | 1,364 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00205DCF | 25,207,068 | 776 | 401 | 394 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021C67C | 25,313,015 | 3,336 | 1,305 | 1,298 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021C67D | 25,314,348 | 4,646 | 1,597 | 1,591 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021C6A5 | 25,316,384 | 5,788 | 2,020 | 2,013 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021C6A6 | 25,318,432 | 4,824 | 1,606 | 1,599 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021D3D5 | 25,333,096 | 3,336 | 1,301 | 1,295 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021D3D6 | 25,334,425 | 5,946 | 2,070 | 2,063 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021D3D7 | 25,336,523 | 3,880 | 1,477 | 1,471 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021D3D8 | 25,338,028 | 4,953 | 1,669 | 1,662 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021D3D9 | 25,339,725 | 5,010 | 1,701 | 1,695 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0021D3DA | 25,341,454 | 3,315 | 1,385 | 1,379 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0022BCA3 | 25,512,680 | 3,819 | 1,458 | 1,451 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0022BE94 | 25,525,484 | 6,052 | 2,067 | 2,060 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0022BEEF | 25,527,579 | 5,004 | 1,700 | 1,693 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00235D62 | 25,567,633 | 4,386 | 1,504 | 1,497 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0023CEC6 | 25,609,491 | 6,157 | 2,087 | 2,080 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CEC | 25,803,535 | 4,192 | 1,508 | 1,502 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CED | 25,805,071 | 4,000 | 1,449 | 1,442 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CEE | 25,806,548 | 4,264 | 1,641 | 1,634 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CEF | 25,808,217 | 3,816 | 1,482 | 1,475 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CF8 | 25,815,513 | 6,055 | 2,066 | 2,059 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CF9 | 25,817,607 | 4,961 | 1,680 | 1,673 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CFB | 25,819,828 | 4,046 | 1,515 | 1,508 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CFC | 25,821,371 | 6,199 | 2,122 | 2,115 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CFD | 25,823,521 | 5,021 | 1,722 | 1,715 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255CFE | 25,825,271 | 4,975 | 1,671 | 1,664 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255D01 | 25,826,970 | 5,944 | 2,082 | 2,075 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255D02 | 25,829,080 | 5,926 | 2,070 | 2,063 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255D03 | 25,831,178 | 4,917 | 1,865 | 1,859 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255D04 | 25,833,071 | 4,974 | 1,695 | 1,689 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255D05 | 25,834,794 | 4,655 | 1,601 | 1,594 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00255D06 | 25,836,423 | 3,315 | 1,391 | 1,385 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FC0 | 25,999,555 | 3,524 | 1,481 | 1,474 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FC3 | 26,001,064 | 3,730 | 1,439 | 1,433 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FC5 | 26,002,531 | 3,316 | 1,286 | 1,279 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FC6 | 26,003,845 | 4,201 | 1,532 | 1,525 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FC8 | 26,005,405 | 2,125 | 915 | 909 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FCA | 26,006,348 | 5,423 | 1,791 | 1,785 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FCE | 26,008,167 | 3,912 | 1,448 | 1,441 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FE1 | 26,009,643 | 4,927 | 1,875 | 1,868 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FE3 | 26,011,546 | 769 | 405 | 399 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FE5 | 26,011,979 | 729 | 381 | 374 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FE7 | 26,012,388 | 4,052 | 1,510 | 1,503 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FE9 | 26,013,926 | 6,058 | 2,063 | 2,056 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FEB | 26,016,017 | 5,870 | 2,049 | 2,043 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FED | 26,018,094 | 3,556 | 1,369 | 1,362 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FEF | 26,019,491 | 5,014 | 1,704 | 1,698 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00264FF0 | 26,021,223 | 3,315 | 1,386 | 1,379 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273F94 | 26,365,912 | 785 | 403 | 396 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273F97 | 26,366,343 | 5,865 | 2,041 | 2,035 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273F98 | 26,368,412 | 4,235 | 1,507 | 1,500 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273F99 | 26,369,947 | 3,879 | 1,471 | 1,465 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273F9A | 26,371,446 | 4,645 | 1,581 | 1,574 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273F9D | 26,375,825 | 6,026 | 2,011 | 2,005 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273F9E | 26,377,864 | 6,180 | 2,094 | 2,087 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273F9F | 26,379,986 | 4,916 | 1,855 | 1,849 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273FA0 | 26,381,869 | 781 | 403 | 396 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273FA1 | 26,382,300 | 4,403 | 1,501 | 1,494 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273FA2 | 26,383,829 | 4,803 | 1,630 | 1,623 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 00273FA3 | 26,385,487 | 4,913 | 1,659 | 1,652 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0028B91A | 26,490,727 | 5,912 | 2,075 | 2,068 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0028B91C | 26,492,830 | 4,456 | 1,546 | 1,539 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0028B91F | 26,494,404 | 6,076 | 2,074 | 2,068 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0028B920 | 26,496,506 | 5,032 | 1,717 | 1,710 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF30F | 26,857,011 | 736 | 375 | 369 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF312 | 26,857,734 | 645 | 349 | 343 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF314 | 26,858,469 | 645 | 349 | 343 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF316 | 26,858,846 | 643 | 347 | 341 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF317 | 26,859,221 | 660 | 363 | 356 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF318 | 26,859,612 | 645 | 349 | 343 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF319 | 26,859,989 | 645 | 345 | 338 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF31A | 26,860,362 | 660 | 358 | 352 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF31B | 26,860,748 | 643 | 343 | 336 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF31D | 26,861,119 | 645 | 345 | 338 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF31F | 26,861,846 | 645 | 345 | 338 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002BF322 | 26,862,536 | 736 | 379 | 373 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002CCF9E | 26,966,831 | 5,024 | 1,716 | 1,709 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002CCF9F | 26,968,575 | 6,195 | 2,114 | 2,107 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 002FCB50 | 27,569,524 | 5,852 | 2,100 | 2,093 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0038679A | 27,995,978 | 4,397 | 1,514 | 1,508 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0039EC80 | 28,119,164 | 3,863 | 1,470 | 1,463 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | NPC_ | 0039EC81 | 28,120,662 | 4,783 | 1,584 | 1,578 | E | last token: match 35 at distance 38 (level 9: 37) |
| Starfield.esm | CELL | 0002C786 | 139,142,869 | 325 | 229 | 223 | E | last token: match 8 at distance 189 (level 9: 188) |
| Starfield.esm | CELL | 0005B77A | 139,346,590 | 613 | 417 | 410 | E | last token: match 17 at distance 336 (level 9: 168) |
| Starfield.esm | CELL | 0015DEDF | 152,405,220 | 1,801 | 734 | 3 | E | last token: match 4 at distance 1606 (level 9: 32) |
| Starfield.esm | CELL | 000FBC17 | 179,000,960 | 605 | 412 | 405 | E | last token: match 28 at distance 517 (level 9: 516) |
| Starfield.esm | CELL | 00029A06 | 180,019,247 | 495 | 272 | 265 | E | last token: match 4 at distance 326 (level 9: 144) |
| Starfield.esm | CELL | 0027459B | 208,880,197 | 330 | 270 | 263 | E | last token: match 4 at distance 176 (level 9: 175) |
| Starfield.esm | CELL | 000073B8 | 231,299,986 | 334 | 205 | 198 | E | last token: match 4 at distance 205 (level 9: 68) |
| Starfield.esm | CELL | 0012C069 | 251,922,324 | 8,573 | 2,956 | 2,948 | E | last token: match 8 at distance 1168 (level 9: 32) |
| Starfield.esm | CELL | 001DBCA3 | 257,282,456 | 368 | 219 | 212 | E | last token: match 12 at distance 283 (level 9: 282) |
| Starfield.esm | CELL | 00093068 | 287,593,248 | 7,944 | 2,658 | 2,650 | E | last token: match 8 at distance 1040 (level 9: 48) |
| Starfield.esm | CELL | 000F71A8 | 294,395,852 | 9,020 | 3,030 | 3,022 | E | last token: match 8 at distance 2672 (level 9: 32) |
| Starfield.esm | CELL | 00004D90 | 296,983,501 | 400 | 282 | 275 | E | last token: match 12 at distance 293 (level 9: 292) |
| Starfield.esm | CELL | 00214663 | 313,012,349 | 4,411 | 1,576 | 1,568 | E | last token: match 8 at distance 3184 (level 9: 80) |
| Starfield.esm | CELL | 0005AE5D | 316,274,534 | 437 | 245 | 239 | E | last token: match 4 at distance 241 (level 9: 240) |
| Starfield.esm | NAVM | 00017F17 | 318,400,440 | 31,255 | 17,687 | 3 | N | level 8 reproduces it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 000267A8 | 318,418,155 | 29,097 | 16,284 | 4 | N | level 8 reproduces it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 000267A9 | 318,434,467 | 29,097 | 16,280 | 4 | N | level 8 reproduces it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 000267AA | 318,450,775 | 29,109 | 16,284 | 4 | N | level 8 reproduces it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 00029AA4 | 318,467,087 | 36,184 | 20,629 | 2 | N | level 8 reproduces it; first difference at byte 66 of the data |
| Starfield.esm | NAVM | 00103414 | 318,487,744 | 28,179 | 15,833 | 4 | N | level 8 reproduces it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 00110AD3 | 318,503,605 | 10,926 | 6,427 | 3 | N | levels 7 and 8 reproduce it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 0014FC69 | 318,510,060 | 28,337 | 14,909 | 3 | N | level 8 reproduces it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 0017FEC6 | 318,524,997 | 10,355 | 5,920 | 3 | N | levels 7 and 8 reproduce it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 001BA29E | 318,530,945 | 49,210 | 28,867 | 3 | N | level 8 reproduces it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 001BA29F | 318,559,840 | 103,552 | 60,647 | 4 | U | from byte 24 of the data; 4 deflate blocks (level 9: 3) |
| Starfield.esm | NAVM | 002F01CA | 318,620,515 | 55,538 | 29,121 | 2 | N | level 8 reproduces it; first difference at byte 14 of the data |
| Starfield.esm | NAVM | 003CFA13 | 318,649,664 | 68,557 | 37,566 | 2 | N | level 8 reproduces it; first difference at byte 24 of the data |
| Starfield.esm | CELL | 0037A56C | 340,554,679 | 440 | 327 | 320 | E | last token: match 16 at distance 349 (level 9: 348) |
| Starfield.esm | CELL | 000090AE | 340,688,671 | 21,763 | 7,500 | 7,492 | E | last token: match 11 at distance 3152 (level 9: 112) |
| Starfield.esm | CELL | 000F69A8 | 354,812,467 | 397 | 279 | 272 | E | last token: match 12 at distance 293 (level 9: 292) |
| Starfield.esm | CELL | 000994BA | 356,872,202 | 629 | 394 | 388 | E | last token: match 28 at distance 535 (level 9: 534) |
| Starfield.esm | CELL | 000090AF | 361,698,075 | 36,764 | 12,614 | 12,606 | E | last token: match 4 at distance 9088 (level 9: 16) |
| Starfield.esm | CELL | 000D131B | 368,853,066 | 282 | 198 | 192 | E | last token: match 4 at distance 93 (level 9: 92) |
| Starfield.esm | CELL | 000DBF69 | 369,638,232 | 355 | 243 | 236 | E | last token: match 8 at distance 207 (level 9: 206) |
| BlueprintShips-Starfield.esm | CELL | 010BD33D | 120,846,037 | 10,520 | 3,666 | 3,658 | E | last token: match 8 at distance 4816 (level 9: 128) |
| BlueprintShips-Starfield.esm | CELL | 010FFF0F | 164,353,359 | 748 | 386 | 379 | E | last token: match 4 at distance 538 (level 9: 212) |
| BlueprintShips-Starfield.esm | CELL | 01102CA7 | 204,982,254 | 1,608 | 631 | 91 | E | last token: match 4 at distance 640 (level 9: 20) |
| SFBGS003.esm | NPC_ | FD000829 | 100,733 | 4,577 | 1,486 | 1,480 | E | last token: match 35 at distance 38 (level 9: 37) |
| SFBGS003.esm | NPC_ | FD00505C | 116,328 | 5,324 | 1,773 | 1,766 | E | last token: match 35 at distance 38 (level 9: 37) |
| SFBGS003.esm | NPC_ | FD005BB7 | 122,614 | 4,567 | 1,478 | 1,471 | E | last token: match 35 at distance 38 (level 9: 37) |
| SFBGS006.esm | CELL | FD00C91D | 4,212,126 | 405 | 287 | 280 | E | last token: match 12 at distance 293 (level 9: 292) |
| SFBGS006.esm | CELL | FD00AB5D | 7,013,611 | 287 | 203 | 197 | E | last token: match 4 at distance 93 (level 9: 92) |
| SFBGS00D.esm | CELL | 01072FC4 | 8,010,102 | 549 | 373 | 366 | E | last token: match 24 at distance 461 (level 9: 460) |
| SFBGS00D.esm | CELL | 01068043 | 11,695,342 | 1,533 | 685 | 3 | E | last token: match 4 at distance 1350 (level 9: 16) |
| SFBGS050.esm | CELL | 0201B11B | 20,036,603 | 5,086 | 1,793 | 1,786 | E | last token: match 10 at distance 32 (level 9: 16) |


## TES4 header inventory (M1.2b)

From `corpus_tes4_table` (`cargo test -p kitkraft-esm --release --test corpus -- --ignored
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
  (meaning not documented on a permitted source; the corpus shows it holding the children
  of a quest). The parser treats group types opaquely; what each holds is in "Cell and
  world groups" below.
- Groups nest at most 6 deep (the parser's default cap is 16).
- The record header's last `u16` ("unknown" on UESP) is non-zero in nearly every record of
  12 files (3,817,455 of 3,829,247 in `Starfield.esm`) but in only 341 and 33 records of the
  two BlueprintShips files. The group header's `u32` "unknown" takes a single value per file except in the BlueprintShips
  files (5 and 4 distinct values). Both are preserved verbatim.

## Cell and world groups (M4.A1)

Group types 1 to 10 and where the records of cells, worlds and quests live, from every
group and record of the 14 files. Each rule below is asserted on every occurrence by
`corpus_group_rules_full` (`crates/esm/tests/cells.rs`, release, 27 s; the five small files
run in every gate as `corpus_group_rules_small`), and is what `kitkraft_esm::cells` computes
from. Type numbers and names: `uesp-skyrim: Skyrim_Mod:Mod_File_Format` ("Groups"), a
container source, confirmed here; type 10 has no name on a permitted source and is called
"quest children" after its contents. No record layout is used except two facts found by
searching the corpus (the `XCLC` grid and the `DATA` position, below); xEdit was not read
for any of this.

| Type | Name                       | Parent (count)                                 | Label                       | Holds (records over all 14 files)                                                   |
| ---- | -------------------------- | ---------------------------------------------- | --------------------------- | ----------------------------------------------------------------------------------- |
| 0    | top                        | file (695, of which 30 empty)                  | record type                 | that type's records; `CELL` holds type 2, `WRLD` type 1, `QUST` type 10              |
| 1    | world children             | top `WRLD` (581)                               | the `WRLD` FormID           | 549 persistent `CELL` with their type 6, then type 4 (2,299)                         |
| 2    | interior cell block        | top `CELL` (85)                                | block number 0..9           | type 3 (695)                                                                        |
| 3    | interior cell sub-block    | 2 (695)                                        | sub-block number 0..9       | 17,020 `CELL`, each followed by its type 6 (16,979)                                 |
| 4    | exterior cell block        | 1 (2,299)                                      | block grid                  | type 5 (2,461)                                                                      |
| 5    | exterior cell sub-block    | 4 (2,461)                                      | sub-block grid              | 22,682 `CELL`, each followed by its type 6 (4,734)                                  |
| 6    | cell children              | 1 (545), 3 (16,979), 5 (4,734)                 | the `CELL` FormID           | type 8 (3,757), type 9 (21,663)                                                     |
| 7    | topic children             | 10 (81,287)                                    | the `DIAL` FormID           | 162,028 `INFO`                                                                      |
| 8    | cell persistent children   | 6 (3,757)                                      | the `CELL` FormID           | `REFR` 115,119, `ACHR` 780, `PGRE` 53, `PHZD` 33                                    |
| 9    | cell temporary children    | 6 (21,663)                                     | the `CELL` FormID           | `REFR` 6,604,606, `NAVM` 92,893, `ACHR` 13,951, `PGRE` 2,546, `PHZD` 417           |
| 10   | quest children             | top `QUST` (1,808)                             | the `QUST` FormID           | `DIAL` 87,031 (each followed by its type 7), `SCEN` 9,673, `DLBR` 135               |

Rules (all 14 files, no exception unless one is listed):

- **Children groups follow their record.** Every group of type 1, 6, 7 and 10 directly
  follows the `WRLD`, `CELL`, `DIAL` or `QUST` record its label names (581, 22,258, 81,287,
  1,808). Types 8 and 9 carry their cell children group's label. No record of a type that
  lives in these groups (`CELL REFR ACHR PGRE PHZD NAVM DLBR DIAL SCEN INFO`) sits in a
  top-level group.
- **Persistence is the record flag 0x400 and the group together.** All 115,985 records in
  type 8 have flag 0x400; none of the 6,714,413 in type 9 does (no `NAVM` ever does). A
  world's persistent references live in its persistent cell: the one `CELL` directly in
  type 1, always its first child, always flag 0x400 (549 of 581 worlds have one; no other
  cell has the flag), whose children group holds type 8 only (545). Exterior cells (type 5)
  have type 9 only (4,734); interior cells have either or both.
- **Interior blocks.** Block = last decimal digit of the cell's object ID, sub-block = the
  digit before it, on all 17,020 interior cells. The object ID is the stored FormID's low
  24 bits, or 12 bits for a stored `FE` FormID and 16 for `FD` (`docs/load-order.md` scale
  classes): the master byte is not part of it (the full 32-bit reading fails on 4,989 cells,
  all overrides), and one cell decides the `FE` width: `SFBGS047.esm` `FE001016` is in block
  2, sub-block 2 (object `0x016` = 22, where 24 bits would give 4,118). Medium files' cells
  are all `FD00xxxx`, where 16 and 24 bits agree.
- **Exterior grid.** A cell at grid (x, y) is in sub-block (floor(x/8), floor(y/8)) and block
  (floor(sub x/4), floor(sub y/4)); a block or sub-block label holds y in its low 16 bits
  and x in its high 16 bits, each an `i16` (22,682 cells; 2,461 sub-blocks under the right
  block). The grid is the first two `i32` of the cell's `XCLC`, x first: found by trying
  every (subrecord, byte offset, order) reading of two `i32` in every exterior cell; only
  this one holds for all 22,682 (the swapped order holds for 11,083). Every exterior and
  persistent cell has one `XCLC`; no interior cell has one.
- **Cell size.** Of the 2,038,073 exterior temporary references with a `DATA`, 2,037,719
  (99.983 %) have `DATA` floats 0 and 1 inside their cell's square when a cell is 100
  units wide (found the same way: the first two floats of `DATA` against the grid, for
  widths 100 to 8,192; only 100 fits). The 354 outside: 118 by under 1 unit, 219 by 1 to 10,
  7 by 10 to 100, 10 by more (8 `ShatteredSpace.esm` `REFR` `010C6B30`..`010C6B39` by 216 to
  230; `Starfield.esm` `000DD340` and `003FF0EC` by 45,423 and 37,039). The other 5,738
  exterior temporary references have no `DATA` and are all deleted (flag 0x20).
- **Order.** Records ascend by stored FormID in types 3 (695 groups), 5 (2,461) and 7
  (81,287) and in the top-level `WRLD` and `QUST` groups; type 10 holds `DLBR`, then `DIAL`
  (with their type 7), then `SCEN`, each ascending (1,808). Sibling groups of types 2, 3, 4
  and 5 ascend by label read as a `u32` (so grid labels order by x, then y, as unsigned
  16-bit values). Records in types 8 and 9 follow no FormID order.
- **Where the two writers differ** (`cells::PlacementStyle`). The 12 files of the official
  build pipeline put type 9 before type 8 when a cell has both (2,364 cells), every `NAVM`
  first in its type 9 (3,017 groups), stamp every group 0, and keep an empty top-level
  group for each of `NAVM DIAL INFO DLBR SCEN` the file has records of (the 30 empty
  top-level groups, exactly). The two `BlueprintShips` files put type 8 first (815 cells),
  append `NAVM` after references (405 of their type 9 groups), keep no empty top-level
  group, stamp groups with one timestamp per file (13505, 13501) and an unknown field of 0
  (top level), 3 (types 2 to 6) or 1 (types 8, 9), except 52 groups (below), and leave 17
  cells with an empty children group (`BlueprintShips-Starfield.esm` 16, from
  `011BA314`; `BlueprintShips-SFBGS050.esm` `03022FE6`).

What the Creation Kit writes for a new plugin (which style, and the stamps) is F20's
question (M4.O1); until it is observed KitKraft writes the official pipeline's style.

**Cross-check with the WORLD family lane** (`docs/schema/world-family.md` on its branch,
`crates/esm/tests/world_family.rs`, derived independently): every group count of the table
above agrees, and so do the children-group rules (types 1 and 6 follow their record and
carry its FormID; types 8 and 9 sit in the type 6 group with the same label) and the
exterior labels (two `i16`, y in the low half, which that page writes "Y first"; sub-block
= grid div 8, block = grid div 32, 22,682 of 22,682); that lane takes the grid from the
pinned xEdit `CELL` definition (`XCLC`: X, Y), which agrees with the corpus search here.
Two findings are refined here: (1) the interior rule holds for 17,020 of 17,020 cells,
not 17,019, once the object ID is read at the width of its scale class (the one cell
the 24-bit reading misses, `SFBGS047.esm` `FE001016`, is a small file's own cell, whose
object ID is the low 12 bits); (2) the order of a cell's persistent and temporary groups,
which that page reports as "either order", is fixed per writer: all 2,364 cells with type
9 before type 8 are in the 12 files of the official pipeline and all 815 with type 8 first
are in the two `BlueprintShips` files, so each style has a default to write.

### Rebuild gate (M4.A2)

`corpus_rebuild_full` (`crates/esm/tests/rebuild.rs`, release, 116 s;
`corpus_rebuild_small` runs the five small files whole in every gate) rebuilds every
top-level group of all 14 files from an empty plugin with the same `TES4`: each record is
upserted (`Plugin::upsert_styled`) through the path `cells::placement` computes from the
record's header and its parent (the world, cell, quest or topic it belongs to), never the
path it was read from, once in file order and once in a seeded shuffled order (parents
first; records of types 8 and 9, and of top-level groups other than `WRLD` and `QUST`,
keep their relative order, which the corpus gives no rule for). Result, 2026-10-14:

| Files | Compared | Result |
| ----- | -------- | ------ |
| The 12 files of the official pipeline (`Starfield.esm` whole: no sample) | every top-level group, byte for byte, both orders | identical; the empty top-level groups the rebuild creates equal the file's |
| `BlueprintShips-Starfield.esm`, `BlueprintShips-SFBGS050.esm` (style `blueprint`) | the same, after the listed normalizations | identical after 69 listed differences: the 17 empty cell children groups above (no record puts them there), and 52 group stamps that are not the style's majority: 50 cell children groups with unknown 1 (28), 2 (19) or 7 (3), and the top-level `CELL` group of each file with 3 |

The `HEDR` record count of a rebuilt plugin follows the majority rule ("TES4 header
inventory"), so it equals the 12 files' and differs from the two `BlueprintShips` files';
the gate compares the groups, not `TES4`.

### Placed references (M4.A5)

`crates/esm/tests/refs.rs` (release, 63 s for the shape, 31 s for the sample): in all
6,719,725 `REFR` and 14,731 `ACHR` of the 14 files, `NAME` occurs once with 4 bytes; `DATA`
occurs once with 24 bytes, except in the 5,847 deleted `REFR` (flag 0x20), which have none;
`EDID`, when present, is the first subrecord; and `DATA` is the last subrecord in all but
3 `REFR` of `Starfield.esm` (`00182E1A`, `002119C0`, `0001E709`: an `MNAM` follows). Before
`NAME` the corpus has `EDID`, `VMAD`, `XALG` and component blocks (`BFCB`..`BFCE`); the
other members sit between `NAME` and `DATA`. In `Starfield.esm` every `NAME` is a record of
the file that is not a reference, cell, world or navmesh (2,323,009 `STAT`, 309,021 `PDCL`,
165,362 `MSTT`, ...), except `REFR` `00014893`, whose `NAME` `000001FB` is no record.
`kitkraft_esm::refs::compose_reference` writes this order; a seeded 1,000 references per
file (all of them in the files with fewer: 9,144 in all) decomposed into their base,
position, rotation, flags, EditorID, other subrecords and header fields, and composed
again, equal their bytes in the file.

## Archives (M2.G1-G3)

The 90 official `.ba2` archives (about 148.7 GB) are read in place, never copied (ADR 0006).
`cargo test -p kitkraft-ba2 --release --test corpus -- --ignored corpus_full --nocapture`
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

### Non-ASCII names in Archive2 (M3.G3, 2026-10-14)

`black-box: Archive2.exe 1.2.0.1`, run as a program only (`Archive2 <folder> -c=<out.ba2>
-r=<root> -f=General -compression=None`) on synthetic files in a temp folder, its archives
read back with `kitkraft-ba2` and extracted again with `Archive2 <archive> -e=<folder>`.
Inputs: one name per non-ASCII character class (`ä`, `é`, `ß`, `ÿ`, `€`, `œ`, NBSP, `Ü`,
`Ä`/`É`/`Œ`/`Ÿ`, `Ж`, `ж`, `日`, `İ`, `ı`), a non-ASCII directory (`dép`), a non-ASCII
extension (`.tëx`), and then every byte 0x80..0xFF of Windows-1252 at the end of a name
and in its middle (256 files). What it does:

- **Encoding.** Paths are lower-cased (Unicode: `Ä` -> `ä`, `Œ` -> `œ`, `Ÿ` -> `ÿ`) and
  stored in Windows-1252; it extracts them under the same names (`eur€.txt`). A
  character outside Windows-1252 (`Ж`, `日`, `İ`, `ı`) fails the whole run
  (`System.IO.IOException: Failed to read file format`); bytes the code page leaves
  undefined (0x81, 0x8D, 0x8F, 0x90, 0x9D) are stored as `?`.
- **Hashes.** Name hash, directory hash and the extension field use the corpus CRC rule
  over the stored bytes with every non-ASCII byte replaced: by 0 for 0x80..0xEF, 0xF6,
  0xF7, 0xFE and 0xFF, so the CRC (which stops at a 0, as over a C string) covers the
  name or directory only up to its first such byte, and the extension field holds a 0
  there (`.tëx` -> `t\0x\0`). Examples: `mixaéb` hashes as `mixa`, `kctest\dép` as
  `kctest\d`, `nb<NBSP>x` as `nb`.
- **Twelve bytes are not a rule.** 0xF0..0xF5 and 0xF8..0xFD (and their upper-case forms
  0xD0..0xD5, 0xD8..0xDD) are replaced by bytes that read as two little-endian 64-bit
  addresses (`18 5B AE AA FD 7F 00 00` and `10 99 62 AB FD 7F 00 00` for 0xF0..0xF7 and
  0xF8..0xFF in the first run). In a second run on the same boot the substitutes of
  0xF2, 0xF3, 0xFA and 0xFB (and 0xD2, 0xD3, 0xDA, 0xDB) changed (16 of 256 name hashes
  differed); the others were the same. Their hashes depend on the run.
- **Silent loss.** Names that agree up to their first non-ASCII byte collide: of
  `ucÄ`, `ucÉ`, `ucŒ`, `ucŸ` (all hashed as `uc`) Archive2 kept the first and dropped
  the other three without a message, although it printed "Adding" for each.

Consequences in `crates/ba2`: names are decoded as Windows-1252 (Latin-1 before, which
differs only in 0x80..0x9F: `eur\u{80}.txt` instead of `eur€.txt`); the reader's hash
check accepts Archive2's hashes for non-ASCII names (`hash::archive2_record_hashes`,
tests built from the values above) and never the run-dependent ones. `ba2.pack` keeps
refusing non-ASCII names: Archive2 itself cannot keep two such names apart, twelve bytes
have no reproducible hash, and how the game looks such names up is not observed (that
needs an in-game check of a loose-vs-archived file, not planned in M3).

### Archive names (M4.G4, 2026-10-14)

Which archives belong to which plugin, from the file names in `Data` and the archive lists
in the game's `Starfield.ini` (read in place; `corpus:`). How the game decides what to
load is runtime behaviour: everything below that goes beyond the names is a hypothesis
(grade C) until the M4.O2 in-game session checks it.

- **Pattern.** Every one of the 90 archives is named `<stem> - <suffix>.ba2` (one space,
  hyphen, one space). For 89 of them `<stem>` is the file name of an official plugin
  without its extension, as spelled in `Data` (`SFBGS00D - Main.ba2` for `SFBGS00D.esm`);
  the 90th, `CreationKit - Shaders.ba2`, has no plugin and is listed in `CreationKit.ini`
  only.
- **The base game is listed, not matched.** All 50 `Starfield - *.ba2` archives appear in
  `Starfield.ini` `[Archive]` lists (`SResourceArchiveList` 23, `sResourceIndexFileList`
  15, `sResourceStartUpArchiveList` 11, `SResourceArchiveMemoryCacheList` 2,
  `sResourceEnglishVoiceList` 3; one listed name, `Starfield - ShadersBeta.ba2`, has no
  file). So does `BlueprintShips-Starfield - Localization.ba2`, the only plugin-named
  archive in a list.
- **Plugin archives are matched by name.** The other 38 archives belong to 11 official
  plugins and appear in no ini list, so the game finds them from the plugin's name (the
  hypothesis M4.O2 checks with a mod archive): `Constellation` and `OldMars` (Localization,
  Textures), `SFBGS003`, `SFBGS004` and `SFBGS00D` (Main, Textures, Voices_de, Voices_en,
  Voices_es, Voices_fr, Voices_ja), `SFBGS006` and `SFBGS047` (Main, Textures),
  `SFBGS007` and `SFBGS008` (Main), `SFBGS050` (Main, Textures, Voices_en),
  `ShatteredSpace` (Main01, Main02, Textures, Voices_en). `BlueprintShips-SFBGS050.esm`
  has none.
- **Suffixes used by plugin archives:** `Main` (8 plugins; GNRL), `Main01`/`Main02`
  (`ShatteredSpace`, GNRL, split because `Main01` reached 4,294,957,291 bytes),
  `Textures` (9; always DX10), `Localization` (3; string tables only), `Voices_<lang>`
  (`de`, `en`, `es`, `fr`, `ja`; `.wem` only). Archive files stay below 2^32 bytes: the
  26 largest are 4.0 to 4.29 GB, the largest 4,294,957,291.
- **Rule for KitKraft (`package`).** A mod's general archive is `<plugin stem> - Main.ba2`
  (GNRL) and its texture archive `<plugin stem> - Textures.ba2` (DX10): the two suffixes
  every official plugin with that content uses, the stem spelled exactly as the plugin's
  file name. Other suffixes (`Voices_<lang>`, `Localization`, numbered splits) are not
  written in M4. Whether the game also loads other suffixes for a mod plugin is not
  claimed. M4.O2 confirms or refutes that a `<stem> - Main.ba2` next to a mod plugin is
  loaded (a script or a loose-vs-archived file in the POI package).

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
| **`ONAM` is not needed for an override of a top-level record.** In the 5 files with an `ONAM` (SFBGS003, SFBGS006, SFBGS00D, SFBGS050, ShatteredSpace) every listed FormID is a record of the same file in a group of type 7 (topic children: INFO), 9 (cell temporary children: REFR, ACHR, NAVM) or 10 (DIAL, SCEN); no top-level record is listed, although 10 files override 853 top-level master records between them (SFBGS003 58, SFBGS004 20, SFBGS006 15, SFBGS008 8, SFBGS00D 133, SFBGS047 1, SFBGS050 142, ShatteredSpace 86, BlueprintShips 357 and 33). Patches therefore write no `ONAM`. **Membership (M4.A6):** a record is listed exactly when it overrides a record its master has and sits in a group of type 7, 9 or 10: true for every record of the 14 files (7,137 listed; overrides in type 8, cells and top-level records never are; the 9 NAVM and 5 deleted REFR of SFBGS050 that are not listed override SFBGS00D objects SFBGS00D lacks). The order of the list is neither file order nor FormID order in SFBGS003, SFBGS006, SFBGS00D and SFBGS050 (it looks unordered); `cells::onam_forms` writes file order, a choice until the Creation Kit is observed (F23). `ONAM` sits right after the master list in every file (`Tes4::set_overridden_forms`). | `corpus:` `corpus_onam_inventory_full`; `corpus_onam_membership_full` (`crates/esm/tests/onam.rs`) |
| **Inline text encoding**: UTF-8, NUL-terminated (the string tables hold UTF-8, `crates/strings`). Whether the game reads inline FULL as UTF-8 or a code page is a `ck-diff` / in-game fact (M2.O4, M2.O6); ASCII names are unaffected. Every written or inlined lstring with a byte >= 0x80 is named in a limitation. A string set as `hex:` must be one NUL-terminated string. | `corpus:` string tables; open |

## Typed check times of large record types (M4 family lanes)

Full-corpus typed check (every official record of the type decoded through the generated
decoder, `extra` empty, re-encoded and compared), all 14 files in parallel, release build,
dev box, 2026-10-14. The out-of-tree checker of each family lane; see its family guide.

| Types | Records | Compressed | Time | Family guide |
| ----- | ------: | ---------: | ---: | ------------ |
| INFO | 162,028 | 0 | ~5 s | `docs/schema/quest-family.md` |
| DIAL | 87,031 | 0 | ~4 s | `docs/schema/quest-family.md` |
| all ten QUEST family types | 267,549 | 0 | ~6 s | `docs/schema/quest-family.md` |
