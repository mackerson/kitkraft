# Execution plan

Scope: M0 and M1. Later milestones are outlined in `ROADMAP.md` and get broken down here when
M1 is close.

## Lanes (ownership)

An agent owns one lane and edits only the crates in it. Cross-lane needs go to
`plan/STATUS.md` under "Requests".

| Lane | Name      | Owns                                                                                                                             |
| ---- | --------- | -------------------------------------------------------------------------------------------------------------------------------- |
| A    | container | `crates/esm`                                                                                                                     |
| B    | schema    | `crates/schema` (including `crates/schema/defs/`)                                                                                |
| C    | surface   | `crates/engine`, `apps/kit-cli`, `xtask`, `docs/record-parity.md`, `crates/esm/src/generated/` (written only by `xtask codegen`) |
| O    | owner     | toolchain install, reviews, commits that need the owner, decisions                                                               |

`crates/esm/src/generated/` is generated output: `cargo xtask codegen` (lane C) writes it from
lane B's definitions in `crates/schema/defs/`. Nobody hand-edits it, lane A included; a
change there is a change to a definition or to the codegen library.

`ba2`, `pex`, `strings`, `nif`, `doc`, `io`, `mcp` have no M1 work and no owner until M2.

## Format facts the M1 tasks rely on

Sources per `plan/adr/0005-spec-sources-per-fact.md`. UESP has no Starfield format page; the
facts below are the shared container from UESP's Skyrim pages. Treat each as a hypothesis
until our own hex dumps of the corpus confirm it, and record the ADR 0005 source in the
schema `source` key.

- Record header: 24 bytes, named as UESP names them: signature, `u32` data size, `u32`
  flags, `u32` FormID, `u16` timestamp, `u16` version control info, `u16` form version,
  `u16` unknown. Data size excludes the header.
- Group header: 24 bytes (`GRUP`, size including the header, label, group type, stamp
  fields). Groups nest.
- Subrecord header: 6 bytes (signature, `u16` size). An `XXXX` subrecord carries a `u32`
  size for the subrecord that follows, whose own `u16` size field is then 0.
- Compressed record flag `0x00040000`: data is a `u32` decompressed size followed by a zlib
  stream.
- TES4 flags: master (ESM) `0x1`, localized `0x80`. Starfield's light/medium/update flag
  bits are not documented anywhere we may read; M1.2b inventories them from the corpus.
- The TES4 header record carries masters (`MAST`), overridden forms (`ONAM`), and the
  localized flag; localized plugins store string IDs, not text, in FULL/DESC and similar.

Preserve every header field as read, including ones we do not understand. Unknown is not
absent.

## M0: scaffold

| ID   | Task                                                                                                                                              | Lane | Depends on | Acceptance                                                                                                             |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ---- | ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| M0.1 | **Done.** Install rustup stable with rustfmt and clippy                                                                                           | O    | -          | `cargo --version`, `cargo fmt --version`, `cargo clippy --version` work in PowerShell                                  |
| M0.2 | **Done.** First build of the unbuilt scaffold; fix compile errors in any crate (only task allowed to touch every crate)                           | C    | M0.1       | `cargo build --workspace` succeeds; `cargo run -p kit-cli -- help` prints the catalog                                  |
| M0.3 | **Done.** `cargo fmt --all`, then make the gate green                                                                                             | C    | M0.2       | `cargo xtask ci` passes; `cargo xtask layers` prints the layer table; a deliberate upward dep fails it (then reverted) |
| M0.4 | **Done.** Owner review and first commit                                                                                                           | O    | M0.3       | Commit on `main`; `plan/STATUS.md` moved to M1                                                                         |
| M0.5 | Corpus test helper: `crates/esm/tests/common/mod.rs` reads `KITCRAFT_DATA_DIR` and `corpus/manifest.toml`, checks sizes, skips cleanly when unset | A    | M0.3       | `cargo test -p kitcraft-esm` passes with and without the env var; skip prints one line                                 |

## M1: data parity

Goal: `kit roundtrip <esm> --verify` is byte-identical on all 14 corpus files
(`docs/corpus.md`). Subrecords we cannot decode are preserved as opaque bytes.

### Lane A: container (`crates/esm`)

| ID    | Task                                                                                                                                                                                        | Depends on | Acceptance                                                                                                                                                                                                     |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M1.1  | Bounded reader over `&[u8]`: `u8/u16/u32/signature/take(n)` returning `Result` with absolute offset; typed `thiserror` errors                                                               | M0.3       | Unit tests: every read past the end is an error with the right offset; no indexing in the crate                                                                                                                |
| M1.2  | Parse model: TES4 header record, groups (nested), records, subrecords incl. `XXXX`; all header fields kept verbatim; subrecords stored as raw bytes                                         | M1.1       | Synthetic fixtures built in-test parse to the expected tree; recursion depth capped                                                                                                                            |
| M1.2b | Header inventory: for all 14 corpus files dump TES4 record flags, form version, `HEDR` contents, and master count into a table in `docs/corpus.md`                                          | M1.2, M0.5 | Table covers all 14 files; distinct flag bits listed per file; closes the "light/medium/update not verified" row in `docs/gaps.md`                                                                             |
| M1.3  | Writer: serialize the model; sizes recomputed only for modified nodes, otherwise emitted as read                                                                                            | M1.2       | `write(parse(x)) == x` on synthetic fixtures; modify-one-subrecord test fixes sizes up the group chain                                                                                                         |
| M1.4  | Compressed records via `flate2` (add to workspace deps). Decompress on demand with a size cap (default 64 MiB, configurable); unmodified records re-emit the original compressed bytes      | M1.3       | Compressed synthetic fixture roundtrips; a declared size above the cap (default 64 MiB) is an error, not an allocation                                                                                         |
| M1.5  | Streaming file driver: one top-level group at a time from a `File`; compare against the source; first mismatch reported as file offset + group/record path                                  | M1.4       | Peak working set (PowerShell `(Get-Process -Id $pid).PeakWorkingSet64` of the test process) is below 2x the largest top-level group of `Starfield.esm` + 256 MiB; measured values recorded in `docs/corpus.md` |
| M1.6  | Corpus roundtrip tests: fast test on the three small files (Constellation, OldMars, SFBGS007); full-corpus test `#[ignore]`, run with `--release -- --ignored`                              | M1.5, M0.5 | All 14 files byte-identical; results (time, peak memory, record counts) recorded in `docs/corpus.md`                                                                                                           |
| M1.7  | Hostile input: deterministic truncation at every record boundary and 1,000 seeded single-bit flips per small file (seed `0xC0FFEE`), on synthetic fixtures and the three small corpus files | M1.3       | No panics; every failure is a typed error with an offset; any crash found ships as a regression test                                                                                                           |

Measure, do not gate: the fraction of compressed records that `flate2` recompresses to the
same bytes (per level). Record it in `docs/gaps.md`; it decides M3's "edit and save like the
CK" work.

### Lane B: schema (`crates/schema`)

| ID    | Task                                                                                                                     | Depends on | Acceptance                                                                                              |
| ----- | ------------------------------------------------------------------------------------------------------------------------ | ---------- | ------------------------------------------------------------------------------------------------------- |
| M1.8  | Definition format per ADR 0003: TOML loader + validator (duplicate signatures, unknown field types, size conflicts)      | M0.3       | Loader tests incl. malformed TOML returning errors; format documented in `crates/schema/defs/README.md` |
| M1.9  | Seed definitions: TES4, GMST, GLOB, KYWD, WEAP (subrecord list from the corpus; layouts only with an ADR 0005 source)    | M1.8       | Each file names its source per ADR 0005; no UESP or wiki prose copied; definitions load cleanly         |
| M1.10 | Codegen library: definitions -> Rust source for typed records (output written into `crates/esm/src/generated/` by xtask) | M1.9       | Golden test on the seed set; does not gate M1                                                           |

### Lane C: surface (`crates/engine`, `apps/kit-cli`, `xtask`)

| ID    | Task                                                                                                                                                                                                                                                             | Depends on  | Acceptance                                                                                      |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- | ----------------------------------------------------------------------------------------------- |
| M1.11 | `execute(id, args: serde_json::Value) -> Result<Value, EngineError>` and the result envelope (`data`, `limitations`, `coverage`, `evidence`). The words -> (`id`, `args`) line parser used by `command_line` lives here in `kitcraft-engine`, not in a front end | M0.3        | Unknown id and schema-invalid args return errors; envelope shape matches `docs/mcp.md`          |
| M1.12 | `roundtrip` command over the M1.5 driver; `Status::Implemented`                                                                                                                                                                                                  | M1.5, M1.11 | Called through `execute` with JSON in a test; mismatch result carries offset and record path    |
| M1.13 | `kit roundtrip <esm> [--verify] [--json]` in `apps/kit-cli` (clap); exit 0 identical, 1 mismatch, 2 error; argument parsing beyond clap reuses the engine line parser (M1.11), never a second one                                                                | M1.12       | Manual run on `Constellation.esm` prints identical; `--json` output equals the `execute` result |
| M1.14 | `coverage` command (record types and subrecords seen in a plugin vs schema-known) and `cargo xtask parity` that writes `docs/record-parity.md` from the corpus                                                                                                   | M1.6, M1.9  | Generated doc covers every record type in the corpus; rerunning is idempotent                   |
| M1.15 | `cargo xtask codegen [--check]`; `ci` runs `--check`                                                                                                                                                                                                             | M1.10       | CI fails when generated code is stale                                                           |

### Ordering

```
M0.1 -> M0.2 -> M0.3 (-> M0.4 done) -+-> A: M1.1 -> M1.2 -> M1.3 -+-> M1.4 -> M1.5 -> M1.6
                                     |               |            +-> M1.7
                                     |               +-> M1.2b (needs M0.5)
                                     +-> B: M1.8 -> M1.9 -> M1.10
                                     +-> C: M1.11 ------------------------> M1.12 -> M1.13
                                                                    M1.6 + M1.9 -> M1.14
                                                                           M1.10 -> M1.15
M0.3 -> M0.5 (lane A, before M1.6)
```

### M1 is done when

- M1.6 passes on all 14 corpus files in `--release`, and M1.7 passes.
- `kit roundtrip <file> --verify` (M1.13) reports identical for each corpus file.
- `docs/record-parity.md` is generated (M1.14).
- `cargo xtask ci` is green and `plan/STATUS.md` says M2.

## Not in M1

Load-order resolution (`doc`), MO2 profile reading (`io`), `ba2`/`pex`/`strings` ports, MCP
server, typed field editing, recompression parity with the CK. These start in M2; an idle
agent may prototype `ba2` or `pex` ports from `C:\Modding\scripts` only if the owner assigns
the lane.
