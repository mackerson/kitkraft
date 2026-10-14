# Known gaps

Check this before claiming something works. Remove a row only with the commit that closes it.

| Gap                                                               | Impact                                                   | Closes in        |
| ----------------------------------------------------------------- | -------------------------------------------------------- | ---------------- |
| Only `roundtrip`, `coverage`, a minimal `inspect` and the lane G commands (`ba2.*`, `papyrus.*`, `strings.*`) are implemented; every other catalog entry is `Planned` | Planned commands exit 2 with a stub or `not_implemented` error | M2 |
| `roundtrip` compares at container level only; compressed bodies are re-emitted, not decompressed | A compressed record whose zlib stream is corrupt but well-sized still roundtrips identical | M2 (`roundtrip --decode`) |
| CLI exit 1 is wired (`data.passed: false`) but no parseable plugin can produce a `roundtrip` mismatch today | The exit-1 path is unit-tested, not exercised end to end | when the writer gains a lossy path |
| `inspect` is single-plugin and untyped                            | No winning override; every payload is hex; FormIDs are file-relative as stored | M2 (load order), schema wiring |
| `command_line` cannot express `set`'s `FIELD=value` shorthand      | `kit set` needs `--fields '{"FULL":"..."}'` (JSON)       | M2               |
| Recompression is 99.88%, not 100%, identical (160 of 136,741 records differ at level 9) | An edited compressed record may differ from what the CK would write; unedited ones, and decoded ones touched without a change, are always byte-identical | M3 |
| `HEDR` record count has two meanings in the corpus: records including `TES4` without groups (12 files), records excluding `TES4` plus groups (both `BlueprintShips` files) | Written verbatim unless records or groups are added or removed; then recomputed by the majority meaning, which may not be what the CK writes for a `BlueprintShips`-style plugin | M3 (CK observation) |
| Streaming `rewrite_file` writes `TES4` before it sees the edits | When an edit adds or removes records or groups, the `HEDR` count must be patched afterwards: `rewrite_file_to_path` does it; a caller streaming to its own sink applies `RewriteReport::hedr_fix` (`HedrFix::apply` on a seekable sink) | permanent (by design of the one-pass stream) |
| Meaning of TES4 flag bits 0x100, 0x400, 0x800 has no permitted source | Bits are preserved and listed raw; `tes4` names only 0x1 and 0x80 | M3 (CK observation) |
| Group type 10 is not in UESP's table (Starfield-only)            | Preserved and walked; its label is shown as a FormID     | M3 (CK observation) |
| Schema has TES4, GRUP, the record header, the universal subrecords (EDID, FULL, DESC, KSIZ/KWDA) and the GMST and GLOB values; the rest of KYWD and WEAP are inventories; `esm` decodes through them (`Record::typed`, `Plugin::typed`, `Record::set_typed`) but `inspect` does not use that yet | Other Starfield-specific payloads are opaque; `inspect` shows hex | lane B round 3 (xEdit); lane D/F (`inspect` through `Plugin::typed`) |
| KYWD CNAM, DNAM, TNAM, FNAM, DATA, ENAM and GMST XALG have no source that M2.B9 could read (no UESP page for them was available, the pinned xEdit file not yet read) | Kept opaque; corpus facts for them are noted in the definitions | lane B round 3 (xEdit) |
| lstring IDs (FULL, DESC, GMST `s` values) are typed by size and the localized flag only; no corpus check that each ID exists in the string tables | A wrong ID shows as an unresolved string, not as a decode error | M2.D7 (string tables) |
| `coverage` (and `docs/record-parity.md`) classify subrecords by signature only | A `typed` subrecord whose payload does not fit its layout still counts as typed | lane C (decode through `Plugin::typed`) |
| Discriminated unions: only `decider = "edid-prefix"` (layout chosen by the EditorID's first byte, M2.B9) can be expressed; a layout chosen by another field or subrecord cannot | Such payloads stay opaque | later format work |
| `count_from` arrays: the count field is written as stored | Editing the array without its count field writes an inconsistent payload | M2 (editing) |
| Localized strings not loaded                                      | FULL/DESC on localized plugins show string IDs only      | M2               |
| No load-order resolution                                          | Cannot answer "which plugin wins"                        | M2               |
| MCP server not implemented                                        | Agents use the CLI only                                  | M2               |
| `ba2.pack` stores no DDS header; extraction synthesizes the one `Archive2.exe` writes | A texture packed with a different (valid) header extracts with the same pixels and a different header | permanent (the format has no header field) |
| DDS header choices for DXGI formats absent from the official archives are unobserved | Those formats extract with the DX10 extension header, which Archive2 may not choose | when such a texture is observed |
| `papyrus.compile` cannot read `Tools\ContentResources.zip` | The caller extracts `Scripts\Source` and passes it in `imports` | later (zip reader) |
| `.pex` assembly, editing and decompiling to `.psc` | Scripts can be disassembled and compiled from source only | after M2 |
| No fuzzing harness (cargo-fuzz needs nightly)                     | Hostile-input coverage limited to deterministic mutation | after M1         |
| `forbid(unsafe_code)` covers workspace crates only. zlib-rs (our flate2 backend) is pure Rust but uses `unsafe` internally: 469 source lines of zlib-rs 0.6.8 mention it | Decompression of hostile streams relies on that crate's own unsafe code being sound | permanent (dependency review) |
| Spriggit round trip lost data on 2 of 5 real mods tested          | Not an oracle; inventory facts only (ADR 0001 §4)        | permanent        |
| Typed Starfield record layouts (WEAP, AMMO, PROJ, AMDL, OMOD, COBJ, ZOOM, LVLI, FLST, INNR) not transcribed yet: ADR 0004 approved, but fetching the pinned xEdit files was not permitted in the 2026-10-14 lane B session | Record bodies beyond the universal subrecords stay opaque | lane B round 3, once the files may be fetched |
| SF1Edit not installed (differential comparison, ADR 0005 Am. 2 §2a) | No SF1Edit cross-check of decoded values               | M2               |
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
with only the zlib-rs backend and recompresses edited records at level 9. Why the remaining
0.1% differ (a different compressor setting, or records the official tools wrote another
way) is not known; deciding that needs CK observation (M3).

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
