# Known gaps

Check this before claiming something works. Remove a row only with the commit that closes it.

| Gap                                                               | Impact                                                   | Closes in        |
| ----------------------------------------------------------------- | -------------------------------------------------------- | ---------------- |
| Every catalog command is `Implemented` (lane E closed `set`, `validate`, `build`); `PLANNED_IN_M2` still lists them | None for users; the stale list only prints | lane F (prune) |
| The `roundtrip` command compares at container level only. The driver's strict decode (`RoundtripOptions::decode`, M3.A2) inflates every compressed body, checks its declared size, its end marker and checksum and that nothing follows the stream, re-encodes its subrecords, and reports a failing body as a mismatch with offset and path; the catalog, CLI and MCP do not pass it yet | Through `kit roundtrip`, a compressed record whose zlib stream is corrupt but well-sized still roundtrips identical | M3.F1 (`roundtrip --decode` in the catalog) |
| CLI exit 1 is wired (`data.passed: false`) but no parseable plugin can produce a `roundtrip` mismatch today | The exit-1 path is unit-tested, not exercised end to end | M3.F1 (a corrupt compressed stream under `--decode`) |
| `inspect`, `query`, `diff` and `conflicts` decode fields only where lane B's schema gives a layout: today EDID, FULL, DESC and KSIZ/KWDA on the defined types (AMDL, AMMO, COBJ, GLOB, GMST, KYWD, OMOD, WEAP, ZOOM, TES4) and the GMST/GLOB values. There `diff`/`conflicts`/`inspect`'s `vs_previous` compare member by member with FormIDs resolved by plugin name (M3.D2); elsewhere payloads are compared as bytes | Most Starfield payloads stay hex and unknown to `where`; their diffs are per subrecord and byte range, and FormIDs inside them are compared as stored (`payloads_comparable` says when that is safe) | lane B round 3 (each newly typed member is compared by value with no change in lane D) |
| `set` and `build` refuse a load order with a failed plugin, a late or missing master, or an active plugin without a file ("load order unverified"), and a deleted winning version, until lane D's round 2 makes the winning override a safe source | Since lane D's round 2 (`hardcoded-official-slots`) the owner's Default profile has no master loading after its dependent, so it is no longer refused on that ground (it was: `stroudpremiumedition.esm` before two of its masters in `loadorder.txt` order); `--plugins` with an explicit list works. Lane D's part is in (M3.D1: `Loaded::missing_active_plugins`, `Loaded::source_check`, `Resolved::bytes`) | M3.E1, M3.E2 (lane E) |
| Patch FormIDs are copied raw: a record joins an existing patch only when the master lists are prefix-compatible (else remapped through the M2.B3 visitors, which is refused when any subrecord is opaque: nearly every Starfield record today) | Records from plugins with different master orders go to a new patch | M3.E10 (after M3.B1) |
| lstrings are found by schema type (whole subrecord, or the last field of a struct) and by the `FULL`/`DESC` fallback on undefined record types; lstrings elsewhere in a payload are not seen | Such an ID would be copied as stored into a non-localized patch; `validate` would not flag it either | M3.B8, M3.E4 |
| Inline text in patches is written as UTF-8; whether the game reads inline FULL as UTF-8 or a code page is not observed (M3.O2 with fixture F13, M2.O6) | Non-ASCII names may show wrong in game; ASCII names are unaffected | M3.O2 (F13) |
| Recompression is 99.88%, not 100%, identical (160 of 136,741 records differ at level 9): 12 NAVM were written at level 8, 147 NPC_/CELL differ only in the distance of their last match (not reproducible by any zlib-rs setting), 1 NAVM is unexplained (M3.A1, "Recompression parity" below) | An edited compressed record may differ from what the official tools or the CK would write; unedited ones, and decoded ones touched without a change, are always byte-identical | M3 (CK observation F07; M3.H3) |
| `HEDR` record count has two meanings in the corpus: records including `TES4` without groups (12 files), records excluding `TES4` plus groups (both `BlueprintShips` files) | Written verbatim unless records or groups are added or removed; then recomputed by the majority meaning, which may not be what the CK writes for a `BlueprintShips`-style plugin | M3.H3 (CK observation) |
| TES4 bits 0x100 (small), 0x400 (medium), 0x800 (blueprint) are named from the pinned xEdit file and their FormID encoding is corpus-confirmed (`docs/load-order.md`), but runtime slots (`FE`/`FD` IDs) are not observed in game or the CK | Runtime IDs in results are the assumption `runtime-slots-by-scale`; `esm::tes4` still names only 0x1 and 0x80 | M3.O1, M3.O2, M3.D5 |
| `formid.dangling` cannot see FormIDs inside opaque subrecords (counted as `unchecked_subrecords`), nor resolve a small or medium plugin's own records stored full-style (`full-style-self`, counted as unresolved). Since M3.E3 scripts inside archives are scanned and `FE`/`FD` FormIDs are checked | FormIDs in untyped payloads are not checked | lane B round 3 (typed layouts); M3.O2 (`full-style-self`) |
| Streaming `rewrite_file` writes `TES4` before it sees the edits | When an edit adds or removes records or groups, the `HEDR` count must be patched afterwards: `rewrite_file_to_path` does it; a caller streaming to its own sink applies `RewriteReport::hedr_fix` (`HedrFix::apply` on a seekable sink) | permanent (by design of the one-pass stream) |
| Group type 10 is not in UESP's table (Starfield-only)            | Preserved and walked; its label is shown as a FormID     | M3 (CK observation) |
| Schema has TES4, GRUP, the record header, the universal subrecords (EDID, FULL, DESC, KSIZ/KWDA) and the GMST and GLOB values; the rest of KYWD and WEAP are inventories. (`inspect`, `where` and `diff` decode every defined subrecord through `kitcraft_doc::typed`, which reads the same values as the generated decoder behind `Plugin::typed` on every record of the 14 files: M3.D6, `docs/corpus.md`) | Other Starfield-specific payloads are opaque; `inspect` shows hex | lane B round 3 (xEdit) |
| KYWD CNAM, DNAM, TNAM, FNAM, DATA, ENAM and GMST XALG have no source that M2.B9 could read (no UESP page for them was available, the pinned xEdit file not yet read) | Kept opaque; corpus facts for them are noted in the definitions | lane B round 3 (xEdit) |
| lstring IDs (FULL, DESC, GMST `s` values) checked against the string tables on all 14 files (M3.D6, `docs/corpus.md`): all resolve except 85 ARMO FULL IDs of Starfield.esm (in none of its English tables) and the 96 of BlueprintShips-SFBGS050.esm, which is localized but ships no string tables (30 of its IDs exist in SFBGS050's tables; whether the game reads a blueprint companion's strings there is not observed) | Those names show as unresolved strings with the reason, never guessed | in-game or CK observation (M3.O1/M3.O2) |
| `coverage` (and `docs/record-parity.md`) classify subrecords by signature only | A `typed` subrecord whose payload does not fit its layout still counts as typed | lane C (decode through `Plugin::typed`) |
| Discriminated unions: only `decider = "edid-prefix"` (layout chosen by the EditorID's first byte, M2.B9) can be expressed; a layout chosen by another field or subrecord cannot | Such payloads stay opaque | later format work |
| `count_from` arrays: the count field is written as stored | Editing the array without its count field writes an inconsistent payload | M2 (editing) |
| Group type 10 is not in UESP's table (Starfield-only)            | Preserved and walked; its label is shown as a FormID     | M3.H3 (CK observation) |
| Schema has TES4, GRUP, the record header, the universal subrecords (EDID, FULL, DESC, KSIZ/KWDA) and the GMST and GLOB values; the rest of KYWD and WEAP are inventories; `esm` decodes through them (`Record::typed`, `Plugin::typed`, `Record::set_typed`) but `inspect` does not use that yet | Other Starfield-specific payloads are opaque; `inspect` shows hex | lane B round 3 (M3.B1..B8); M3.D6 (`inspect` through `Plugin::typed`) |
| KYWD CNAM, DNAM, TNAM, FNAM, DATA, ENAM and GMST XALG have no source that M2.B9 could read (no UESP page for them was available, the pinned xEdit file not yet read) | Kept opaque; corpus facts for them are noted in the definitions | M3.B6 |
| lstring IDs (FULL, DESC, GMST `s` values) are typed by size and the localized flag only; no corpus check that each ID exists in the string tables | A wrong ID shows as an unresolved string, not as a decode error | M3.D6 (corpus check of every lstring ID) |
| Discriminated unions: only `decider = "edid-prefix"` (layout chosen by the EditorID's first byte, M2.B9) can be expressed; a layout chosen by another field or subrecord cannot | Such payloads stay opaque | M3.B9 (deciders as round 3 types need them) |
| `count_from` arrays: the count field is written as stored | Editing the array without its count field writes an inconsistent payload | M3.B10 |
| String ID 0 (14 official GMST string settings) is in no string table | Such values stay unknown in `query` rather than being read as empty strings | when a source says what ID 0 means |
| MO2 rules not yet checked in MO2 itself: mod priority direction (UI check), `lockedorder.txt` format (empty in the owner's profile), whether `loadorder.txt` lists inactive plugins, how unmanaged DLC plugins are activated, where blueprint masters load, where the game puts official plugins that have no `plugins.txt` entry (KitCraft: right after the `Starfield.ccc` plugins, assumption `hardcoded-official-slots`) | `docs/load-order.md` marks each as observed in files only or as an assumption | M3.O2, M3.D5 |
| Small/medium plugins that store their own records full-style resolve by the assumption `full-style-self` (object ID masked to the plugin's scale, per Ortham's example); FormIDs past every class (4 records of `HG_ShipCargoHoldAtHome.esm`) stay unresolved | Results name the plugins concerned and say per version how it resolved (`resolved_by`); unresolved records are left out of overrides and queries | M3.O2 (in-game check), M3.D5 |
| `ba2.pack` stores no DDS header; extraction synthesizes the one `Archive2.exe` writes | A texture packed with a different (valid) header extracts with the same pixels and a different header | permanent (the format has no header field) |
| `ba2.pack` refuses non-ASCII archive paths. Archive2's behaviour is now observed (M3.G3, `docs/corpus.md` "Non-ASCII names in Archive2"): Windows-1252 names, hashes that stop at the first non-ASCII byte (so such names collide and Archive2 silently keeps one), run-dependent hashes for twelve bytes, and no support for characters outside Windows-1252. How the game looks up such names is not observed | Files with accented or other non-ASCII names cannot be packed; archives Archive2 made with them are read, extracted under Archive2's names and hash-checked | an in-game lookup check (not planned in M3) |
| Archives cannot hold empty files: `black-box: Archive2.exe 1.2.0.1` skips them ("Skipped zero-length file") and no official record has size 0 | `ba2.pack` skips empty files and lists them in `limitations[]`; the `kitcraft-ba2` `pack` API refuses them | permanent (Archive2 and the corpus agree) |
| DDS header choices for DXGI formats absent from the official archives are unobserved | Those formats extract with the DX10 extension header, which Archive2 may not choose | when such a texture is observed |
| `.pex` assembly, editing and decompiling to `.psc` | Scripts can be disassembled and compiled from source only | after M3 (not in M3) |
| No fuzzing harness (cargo-fuzz needs nightly)                     | Hostile-input coverage limited to deterministic mutation | M3.A5 (stable-Rust structure-aware mutation) |
| `forbid(unsafe_code)` covers workspace crates only. zlib-rs (our flate2 backend) is pure Rust but uses `unsafe` internally: 469 source lines of zlib-rs 0.6.8 mention it | Decompression of hostile streams relies on that crate's own unsafe code being sound | permanent (dependency review) |
| Spriggit round trip lost data on 2 of 5 real mods tested          | Not an oracle; inventory facts only (ADR 0001 §4)        | permanent        |
| Typed Starfield record layouts (WEAP, AMMO, PROJ, AMDL, OMOD, COBJ, ZOOM, LVLI, FLST, INNR) not transcribed yet: ADR 0004 approved, but fetching the pinned xEdit files was not permitted in the 2026-10-14 lane B session | Record bodies beyond the universal subrecords stay opaque | M3.O0 (stage the files), then M3.B1..B7 |
| SF1Edit installed is xEdit 4.1.5f (2024), older than the pin; only its console dumper runs headless (`docs/oracles.md`) | The cross-check (`cargo xtask oracle sf1edit`) cannot judge GPOG/GPOF, light masters (SFBGS047) or newer WEAP components; its editor script mode needs a click | M3.B11 (rerun as members get typed); a build at the pin (owner) |
| Project name is a placeholder                                     | Package names (`kitcraft-*`, `kit`) may be renamed       | owner            |
| No remote, no hosted CI                                           | The local `cargo xtask ci` run is the only gate          | owner            |

## Recompression parity (measured 2026-10-14)

Question: if we decompress a record and compress it again, do we get the original bytes?
Unedited records never go through this (the writer re-emits their original compressed
bytes); it decides what an edited compressed record looks like. Measured over every
compressed record in the 14 official ESMs (136,741 records; `corpus_recompression_parity`).

All 136,741 streams start with zlib header `78 DA` (maximum-compression level class).

| Compressor                                        | Identical at any level | Level 7 | Level 8 | Level 9           | Levels 0-6 |
| ------------------------------------------------- | ---------------------- | ------- | ------- | ----------------- | ---------- |
| flate2 default backend (miniz_oxide)              | 3,366 (2.5%)           | 0       | 0       | 3,366             | 0          |
| C zlib 1.3 via libz-sys (out-of-tree measurement) | 117,664 (86.0%)        | 116,970 | 117,664 | 117,664           | 0          |
| zlib-rs (workspace backend since adaf097)         | 136,593 (99.9%)        | 2,310   | 2,320   | 136,581 (99.88%)  | 0          |

Per file with zlib-rs at level 9: 100% for BlueprintShips-SFBGS050, SFBGS004, SFBGS008,
SFBGS047 and ShatteredSpace; misses are 149 in Starfield.esm, 3 in BlueprintShips-Starfield,
3 in SFBGS003, 2 in SFBGS006, 2 in SFBGS00D, 1 in SFBGS050. The workspace now builds flate2
with only the zlib-rs backend and recompresses edited records at level 9.

### The 160 misses (M3.A1, 2026-10-14)

Every miss is listed in `docs/corpus.md` ("Recompression misses"), with its file, type,
FormID, offset, sizes, first differing stream byte and class. The listing is
`corpus_recompression_misses` (`crates/esm/tests/recompress.rs`); the classes come from an
out-of-tree study that decodes both streams to LZ77 tokens and searches compressor
settings (procedure below).

Per file and class (E, N, U as defined after the tables):

| File                         | Compressed records | Misses | E   | N  | U |
| ---------------------------- | ------------------ | ------ | --- | -- | - |
| Starfield.esm                | 91,149             | 149    | 136 | 12 | 1 |
| BlueprintShips-Starfield.esm | 1,218              | 3      | 3   | 0  | 0 |
| SFBGS003.esm                 | 1,034              | 3      | 3   | 0  | 0 |
| SFBGS006.esm                 | 360                | 2      | 2   | 0  | 0 |
| SFBGS00D.esm                 | 5,935              | 2      | 2   | 0  | 0 |
| SFBGS050.esm                 | 3,184              | 1      | 1   | 0  | 0 |
| The other 8 files            | 33,861             | 0      | 0   | 0  | 0 |
| Total                        | 136,741            | 160    | 147 | 12 | 1 |

Per record type (all files):

| Type | Compressed records | Misses | Rate  | E   | N  | U | Inflated size of the misses (min / median / max) |
| ---- | ------------------ | ------ | ----- | --- | -- | - | ------------------------------------------------ |
| NPC_ | 8,447              | 118    | 1.4%  | 118 | 0  | 0 | 643 / 4,511 / 6,206                              |
| CELL | 32,230             | 29     | 0.09% | 29  | 0  | 0 | 282 / 605 / 36,764                               |
| NAVM | 92,764             | 13     | 0.01% | 0   | 12 | 1 | 10,355 / 29,109 / 103,552                        |

No other record type has a miss. Every miss has zlib header `78 DA`, one stream that ends
exactly at the end of the record (no trailing bytes) and a correct Adler-32.

- **E, end-of-input match (147: 118 NPC_, 29 CELL, in six files).** The original stream
  and level 9's carry the same LZ77 tokens except the last one: a match that runs to the
  end of the record's data, of the same length, at a larger distance in the original (NPC_:
  length 35 at distance 38 where level 9 picks 37 in 117 of 118, the other 31 at 33 for
  32; CELL: lengths 4 to 28). The streams differ only in their last bytes, except in four
  where the changed distance code also changes the dynamic Huffman tables (from stream
  byte 3, 3, 4 and 91 on). In all 147, the data byte that follows the original's
  candidate is `00` and the one after level 9's is not. zlib-rs 0.6.8 stops at the first
  hash-chain candidate whose match reaches the end of the input; a compressor that keeps
  comparing past the end (where its buffer holds whatever it held before) picks a farther
  candidate whenever those bytes extend it.
- **N, level 8 (12 NAVM, Starfield.esm).** Reproduced exactly by zlib-rs at level 8 (two of
  them also at level 7), window 15, memory level 8 (most also 9, two also 7); the streams
  differ from level 9's near the start (data byte 14 in ten of them). All 12 sit in one
  group, the temporary children of interior cell `00000025` (`CELL > block 7 > sub-block
  3`), as does the U record.
- **U, unexplained (1 NAVM, `001BA29F`, 103,552 bytes, same group).** No setting tried
  reproduces it; it differs from data byte 24 on and has 4 deflate blocks (level 9: 3).

What was tried, out of tree (a scratch crate outside the workspace, zlib-rs 0.6.8 through
its `__internal-api` feature, the official files read in place; nothing of it is in the
repository or the build):

| Variant | Misses reproduced |
| ------- | ----------------- |
| Levels 1-9 x window bits 9-15 x memory level 1-9 x strategy default, filtered, Huffman-only, RLE, fixed (full stream) | the 12 N |
| The same grid, comparing everything after the 2-byte zlib header (a header written separately) | the 12 N |
| Levels 7-9 tuned: good 4-258 x lazy 16-258 x nice 32-258 x chain 256-65,535 (7 x 5 x 4 x 9 values) | the 12 N (chain <= 1,024) |
| Input fed in 512 B to 256 KiB pieces, no / partial / sync / full / block flush between pieces, levels 7-9 | the 12 N |
| One stream reused for every record of a file in file order (reset between records, window kept), level 9 | none (same as fresh) |
| C zlib 1.3.2 (libz-sys), levels 1-9 x memory level 1-9 x strategies 0-4, window 15 | none |
| miniz_oxide 0.9.1, levels 0-10 | none |
| zlib-rs patched to keep searching after a match reaches the end of the input (zero bytes past the input), levels 7-9, every record | 14 of the 148 that no stock level reproduces; at level 9 it matches 136,172 records instead of 136,581 |
| The same patched zlib-rs, one stream reused per file in file order (window kept), level 9 | 18 of the 147 E; 30 that stock level 9 reproduces now differ (136,569 identical) |

Conclusions: class N is a setting (level 8), probably a separate navmesh pass. Class E is
not a setting of any compressor tried; its last token tracks the byte after each candidate,
and changing what lies past the input changes which records match (zero fill: 14, a
window carried over in file order: 18). So its last token most likely depends on memory
past the end of the input (what the official tool's buffer held from earlier work), and a
compressor that sees only a record's own bytes is not expected to reproduce all of them.
Whether the CK's own writes show the same effect, and which bytes the CK writes for these
records, is F07's question (M3.H1 candidates in `plan/STATUS.md`). The default writer is
unchanged (zlib-rs, level 9) until M3.H3 says otherwise.

## Spriggit 0.41.0 is not a byte-exact Starfield roundtrip

Measured 2026-10-14 on copies of real mods (originals untouched), with Spriggit run only as
an external binary (ESM -> YAML -> ESM; no source read). Setup, commands and per-file
timings: `C:\Modding\tools\spriggit\README.md`.

| Plugin                       | Size in -> out       | What changed                                                                                                         |
| ---------------------------- | -------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Richer Merchants.esm         | 12,703 B -> 11,863 B | **Data loss.** Every leveled list with more than one entry kept only its first entry (15 entries lost). Spriggit's own post-serialize check passed anyway. |
| Ship Remote Control.esm      | 6,114 B -> 6,645 B   | **Data loss.** A terminal menu lost 2 of its 3 menu items.                                                           |
| CulinaryGreenhouses.esm      | 31,196 B -> 31,207 B | Cosmetic only: -0.0 written as +0.0, records recompressed, header/timestamp fields rewritten.                        |
| Hatch-22.esm                 | 174,311 B -> 175,246 B | Cosmetic (-0.0 vs +0.0, recompression, timestamps) plus one content change: in 19 GBFM records a second FLKW was written back as FLFM. The tool README also notes the TES4 ONAM was dropped. |
| ConstellationDecorations.esm | 4,158 B -> 4,158 B   | Byte-identical.                                                                                                      |

Conclusion: Spriggit cannot be our roundtrip oracle, and its YAML is not a safe step-2
shortcut for editing real plugins (it silently drops data and its own check does not
notice). Our own container (`crates/esm`, `kit roundtrip --verify`) is the roundtrip
reference; Spriggit output may be used only as a hint, and any plugin it writes must be
diffed against the input before use.
