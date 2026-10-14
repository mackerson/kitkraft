# Known gaps

Check this before claiming something works. Remove a row only with the commit that closes it.

| Gap                                                               | Impact                                                   | Closes in        |
| ----------------------------------------------------------------- | -------------------------------------------------------- | ---------------- |
| Scaffold has never been compiled                                  | Any file may have compile or fmt errors                  | M0.2             |
| No command is implemented; catalog entries are all `Planned`      | Every command exits 2; `roundtrip` returns a stub envelope (`limitations: ["not implemented"]`) | M1, M2 |
| CLI exit code 1 (negative result) is never produced yet           | Scripts cannot yet tell "ran, mismatch" from "error"     | M1.13            |
| `command_line` cannot express `set`'s `FIELD=value` shorthand      | `kit set` needs `--fields '{"FULL":"..."}'` (JSON)       | M2               |
| Compressed-record recompression fidelity unknown                  | Edited compressed records may not match CK output bytes  | M1 (measure), M3 |
| Starfield plugin flag variants (light/medium/update) not verified | Header flags may be preserved but misreported            | M1.2b            |
| No schema definitions yet                                         | Every subrecord is opaque; coverage is zero              | M1.9             |
| Localized strings not loaded                                      | FULL/DESC on localized plugins show string IDs only      | M2               |
| No load-order resolution                                          | Cannot answer "which plugin wins"                        | M2               |
| MCP server not implemented                                        | Agents use the CLI only                                  | M2               |
| No fuzzing harness (cargo-fuzz needs nightly)                     | Hostile-input coverage limited to deterministic mutation | after M1         |
| Spriggit round trip lost data on 2 of 5 real mods tested          | Not an oracle; inventory facts only (ADR 0001 §4)        | permanent        |
| xEdit policy undecided (ADR 0004)                                 | Ambiguous fields stay opaque                             | owner            |
| Project name is a placeholder                                     | Package names (`kitcraft-*`, `kit`) may be renamed       | owner            |
| No remote, no hosted CI                                           | The local `cargo xtask ci` run is the only gate          | owner            |

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
