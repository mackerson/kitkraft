# Known gaps

Check this before claiming something works. Remove a row only with the commit that closes it.

| Gap                                                               | Impact                                                   | Closes in        |
| ----------------------------------------------------------------- | -------------------------------------------------------- | ---------------- |
| Scaffold has never been compiled                                  | Any file may have compile or fmt errors                  | M0.2             |
| No command is implemented; catalog entries are all `Planned`      | `kit` prints help only                                   | M1, M2           |
| Compressed-record recompression fidelity unknown                  | Edited compressed records may not match CK output bytes  | M1 (measure), M3 |
| Starfield plugin flag variants (light/medium/update) not verified | Header flags may be preserved but misreported            | M1.2             |
| No schema definitions yet                                         | Every subrecord is opaque; coverage is zero              | M1.9             |
| Localized strings not loaded                                      | FULL/DESC on localized plugins show string IDs only      | M2               |
| No load-order resolution                                          | Cannot answer "which plugin wins"                        | M2               |
| MCP server not implemented                                        | Agents use the CLI only                                  | M2               |
| No fuzzing harness (cargo-fuzz needs nightly)                     | Hostile-input coverage limited to deterministic mutation | after M1         |
| xEdit policy undecided (ADR 0004)                                 | Ambiguous fields stay opaque                             | owner            |
| Project name is a placeholder                                     | Package names (`kitcraft-*`, `kit`) may be renamed       | owner            |
| No remote, no hosted CI                                           | The local `cargo xtask ci` run is the only gate          | owner            |
