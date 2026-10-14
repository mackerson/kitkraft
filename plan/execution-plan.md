# Execution plan

Scope: M0 and M1 (done), and M2 (below). Later milestones are outlined in `ROADMAP.md` and get
broken down here when the previous one is close.

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

## M2: agent surface ("agents can rip and tear")

Goal: an agent can load a real load order, look anything up, change it into a patch plugin,
check the patch, and do all of that over MCP as well as the CLI. Concretely: load-order
resolution from an MO2 profile; `inspect`, `query`, `set`, `diff`, `validate` working across
that load order; `kit mcp`; the `ba2`, `pex` and `strings` ports; and the rename smoke test
(rung 1 of the step 2 ladder in `PLAN.md`).

Tasks marked **owner** need the owner (AGENTS.md "Owner decisions", or something only the
owner can do: install software, drive the game). Tasks marked *(research)* establish a fact
before code relies on it; the fact goes in a doc with its ADR 0005 source form, and the code
cites that doc.

### Lanes (ownership)

One agent per lane, each in its own worktree and `CARGO_TARGET_DIR`. Lanes D, E, F and G
replace M1's lane C; lane F inherits lane C's files.

| Lane | Name                         | Owns |
| ---- | ---------------------------- | ---- |
| A    | container                    | `crates/esm` (not `src/generated/`) |
| B    | schema                       | `crates/schema` (including `defs/`) |
| D    | doc + io                     | `crates/doc`, `crates/io`, `docs/load-order.md`; engine `handlers/{load,inspect,query,diff,conflicts}.rs` |
| E    | set + validate + build       | engine `src/edit/`, `src/validate/`, `src/build/`, `handlers/{set,validate,build}.rs`, the `FIELD=value` part of `src/cmdline.rs`; `docs/smoke-rename.md` |
| F    | mcp + surface                | `crates/mcp`, `apps/kit-cli`, `xtask`, engine core (`exec.rs`, `envelope.rs`, `args.rs`, `params.rs`, `cmdline.rs` except the shorthand, `Status`/`CommandSpec` and the session type), `docs/record-parity.md`, `.mcp.json.example`, the protocol parts of `docs/mcp.md` |
| G    | archives + scripts + strings | `crates/ba2`, `crates/pex`, `crates/strings`; engine `handlers/{ba2,papyrus,strings}.rs`; `plan/adr/0006-*` |
| O    | owner                        | decisions, installs, CK and game sessions, reviews |

Shared files, edited in small separate commits by the lane whose command it is: the command's
own `CommandSpec` entry in `crates/engine/src/lib.rs` and its line in `handlers.rs`
`defaults()`; the command's "Command results" subsection of `docs/mcp.md`; dependency lines in
`crates/engine/Cargo.toml` and the workspace `Cargo.toml`; rows of `docs/gaps.md` (removed by
the commit that closes them); sections of `docs/corpus.md`. Every new command lands per
AGENTS.md: CLI form, MCP reachability through `execute`, and a test that calls it through
`execute` with JSON.

### Format facts M2 relies on, and where they come from

Hypotheses until the named check confirms them. Nothing here is a Starfield field layout.

- File-relative FormIDs: high byte = index into the plugin's own master list, index == master
  count = the plugin itself (our own `C:\Modding\scripts\README.txt`; `uesp-skyrim`). Confirmed
  in M2.D3 on the corpus files that have masters (`SFBGS047`, `BlueprintShips-SFBGS050`).
- TES4 flag bits 0x100, 0x400 and 0x800 occur in the corpus (`docs/corpus.md`). What they mean
  (small/medium plugins) and how runtime `FE`/`FD` IDs are formed are `ck-diff` facts per ADR
  0005 and have no source yet (M2.D3).
- Localized plugins store a `u32` string ID in lstring subrecords (FULL, DESC), with the text
  in `Strings\<plugin>_<language>.{STRINGS,DLSTRINGS,ILSTRINGS}`; non-localized plugins store
  a NUL-terminated string inline (`uesp-skyrim`; FULL sizes checked on the corpus in M2.B2;
  the non-localized half, which the all-localized official corpus cannot show, is confirmed by
  M2.E3 and the in-game check M2.O6). `fix_resupply_door.py` is our own prior observation of
  the stripped-FULL bug.
- MO2 profile files (`modlist.txt`, `plugins.txt`, `loadorder.txt`, `lockedorder.txt`,
  `Starfield.ccc`, `archives.txt`) and the `mods\<name>\` overlay: from the files themselves
  and MO2 run as a black box. MO2 is GPL-3 software that handles Bethesda formats: **never
  read its source** (AGENTS.md "Clean room").
- BA2, `.pex`, string tables: `ba2x.py`, `pexdis.py` (ours), plus the sources M2.G0 lists.
  `ba2x.py` covers GNRL extraction only; DX10 and packing are new work on M2.G0 sources.

### Lane A: container (`crates/esm`)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M2.A0 | Round-2 hardening (in flight when this plan was written; its review findings are the scope) | - | Every finding closed with a regression test; M1.6 full corpus run identical; `cargo xtask ci` green |
| M2.A1 | Record index: stream a plugin into `RecordLoc {signature, form_id, flags, offset, data_size, group path, edid}` (EDID read from compressed records under `DecodeLimits`) without building the tree | M2.A0 | Record counts equal `coverage`'s on all 14 corpus files; `Starfield.esm` indexed in <= 15 s release with peak working set within the M1.5 bound; truncated and bit-flipped inputs give typed errors with offsets |
| M2.A2 | Random access: `read_record_at(file, offset) -> Record<'static>` (header checks, decompress on demand) | M2.A1 | For 1,000 seeded index entries per small corpus file and 1,000 in `Starfield.esm` the record equals the streaming parse's; a wrong offset is a typed error |
| M2.A3 | Typed decode wiring (lane B and lane C requests in `plan/STATUS.md`): `Record::decode_typed(ctx)` through the generated `Record::decode`, `Ctx` from TES4 (`form_version`, `localized`); driver option that decodes and re-encodes every record with a definition and compares | M2.A0 | On all 14 files every typed subrecord satisfies `encode(decode(x)) == x`, or each conflict is listed (record, subrecord, offset) and filed as a lane B request; counts exposed for `coverage` (decoded, failed) |
| M2.A4 | `XXXX` inventory (lane B request): count `XXXX` with payload <= 0xFFFF per file; record the writer policy for new and edited values | M2.A0 | Counts in `docs/corpus.md`; the `docs/gaps.md` row closed or restated with the numbers |
| M2.A5 | Plugin builder and streaming rewrite: a new plugin from scratch (TES4 with HEDR, MAST list, flags) with a record inserted into the right top-level group (created when missing, in the corpus's top-level group order); add or replace one record in an existing plugin while streaming the rest | M2.A1, M2.A2 | Builder output parses and roundtrips identical; replacing one record in a copy of a small corpus file changes only that record and the group sizes up its chain (byte diff confined to those ranges); group-order test against the `Starfield.esm` inventory |
| M2.A6 | Record edit API on owned records: set, insert, remove subrecords; compressed records recompressed (zlib-rs level 9) only when modified; `XXXX` kept per subrecord or chosen by size per M2.A4 | M2.A0 | A unit test per operation; an edited compressed corpus record decompresses back to the edited subrecords; unedited records still re-emit their original bytes |

### Lane B: schema (`crates/schema`)

M2.B1..B3 and M2.B9 have no gate and start now. M2.B4..B8 and M2.B10 resume after M2.O1; if
the owner keeps option A, those layouts come only from `ck-diff` observations (M2.O4), and
whatever is not observed carries to M3.

| ID     | Task | Depends on | Acceptance |
| ------ | ---- | ---------- | ---------- |
| M2.B1  | Provenance per member: `source` per field (ADR 0005 forms, plus any form M2.O1 approves) and `confidence` (ADR 0004 values: `reference` \| `corpus` \| `ck-diff` \| `verified`) | - | Loader rejects a missing or unknown value; `defs/README.md` documents it; existing defs migrated; `docs/record-parity.md` shows counts per confidence (M2.F9) |
| M2.B2  | Universal subrecords typed from `uesp-skyrim` facts confirmed on the corpus: EDID (zstring), FULL and DESC (lstring), KSIZ/KWDA (count + FormID array via `count_from`), on every M2 record type (inventory defs created for AMMO, PROJ, AMDL, OMOD, COBJ, ZOOM, LVLI, FLST, INNR) | M2.B1, M2.A3 | Through M2.A3 every corpus occurrence decodes and re-encodes identical; FULL is 4 bytes in every record of every (localized) official file; parity shows these `typed` |
| M2.B3  | Codegen visitors: `form_ids()` / `form_ids_mut()` over every FormID-typed member and `lstrings()` over every lstring member of a generated record | M2.B2 | Golden tests; a test record with FormIDs in nested and repeated members yields all of them, in order |
| M2.B4  | WEAP typed layout | M2.O1, M2.B1, M2.A3 | Every member has `source` and `confidence`; every corpus WEAP decodes and re-encodes identical; M2.B11 comparison on >= 20 WEAP records with every mismatch explained or the member demoted to opaque |
| M2.B5  | AMMO, PROJ typed | M2.B4 | As M2.B4, per type |
| M2.B6  | AMDL typed | M2.B4 | As M2.B4 |
| M2.B7  | OMOD typed | M2.B4, M2.O2 | As M2.B4 |
| M2.B8  | COBJ, ZOOM typed | M2.B4 | As M2.B4, per type |
| M2.B9  | GMST, GLOB, KYWD typed (value types from `uesp-skyrim` facts confirmed on the corpus, or the M2.O1 source) | M2.B2 | As M2.B4; a `query --type GMST` predicate on the value evaluates with no unknowns on the official files |
| M2.B10 | LVLI, FLST, INNR typed | M2.B8 | As M2.B4, per type |
| M2.B11 | SF1Edit differential harness: run SF1Edit as a program on the official files (never read its source), capture the values it shows for sampled records, compare with `kit inspect --json` | M2.O3, M2.D9 | Procedure and per-type comparison tables in `docs/schema-differential.md`; sample ids seeded and listed |

### Lane D: doc + io (`crates/doc`, `crates/io`)

| ID     | Task | Depends on | Acceptance |
| ------ | ---- | ---------- | ---------- |
| M2.D1  | *(research)* MO2 profile semantics from the files and MO2 as a black box: `modlist.txt` (`+`/`-`/`*` prefixes, `_separator` entries, priority direction), `plugins.txt` (`*` = enabled), `loadorder.txt`, `lockedorder.txt`, `Starfield.ccc`, `archives.txt`, `overwrite\`, the game dir from `ModOrganizer.ini` `gamePath` | - | `docs/load-order.md` states each rule with its observation (file and line, or an MO2 UI check); the mod priority direction is confirmed by one UI check (MO2's view of a file two mods ship) in an M2.O4 session |
| M2.D2  | `io`: read-only MO2 profile reader: enabled mods in priority order, plugin list with enabled flags, archive list, overlay resolution `relative path -> providing mod, overwrite, or game Data` (case-insensitive) | M2.D1 | Fixture profiles in temp dirs (disabled mods, separators, missing mod dir, case clashes, BOM/CRLF); on the real Default profile every enabled plugin resolves to an existing file, with counts computed from the profile files in the test, not hard-coded; profile files' size and mtime unchanged after the run |
| M2.D3  | *(research)* FormID facts: (a) file-relative resolution (above) confirmed on the corpus: every master-indexed FormID in corpus record headers names a record that exists in that master; (b) per flag bit 0x100/0x400/0x800 the largest object id used, from the corpus; (c) the meaning of those bits and the runtime `FE`/`FD` ID form, from a `ck-diff` or in-game console observation in an M2.O4 session | M2.A1 | (a) and (b) as corpus tests plus a `docs/load-order.md` section citing `corpus:`; (c) recorded with `ck-diff:`, or left open, in which case runtime IDs are shown only for plugins with none of those bits and the limitation says why |
| M2.D4  | `doc`: `LoadOrder` model: plugins in order with TES4 facts (masters, flags, localized), master resolution by case-insensitive name, missing and out-of-order masters, `GlobalForm {plugin, object_id}`, file-relative <-> global mapping both ways, runtime IDs where M2.D3 allows | M2.A1, M2.D3 (a) | Unit tests on synthetic load orders (missing master, master after dependent, names differing only in case); the 14 official files in `loadorder.txt` order resolve every master |
| M2.D5  | `doc`: per-plugin index (M2.A1), override table (every version of every form, in load order) and winning override; on-disk index cache under `%LOCALAPPDATA%\kitcraft\cache` keyed by path, size, mtime and index format version (never in Data or the profile) | M2.D4, M2.A1 | Default profile cold and warm times recorded in `docs/load-order.md`; warm load <= 3 s release; a changed size or mtime invalidates the entry (test); override lists are in load order and the winner is the last |
| M2.D6  | `doc`: conflict graph: forms with >= 2 overriding plugins, each overrider classed `identical-to-previous`, `differs` (which subrecords; typed fields where typed) or `deleted` | M2.D5 | A synthetic fixture per class; a Default profile run reports class counts with evidence offsets |
| M2.D7  | `doc`: localized strings: for localized plugins load the string tables from the overlay (loose files) or the plugin's archives (M2.G1); resolve lstrings with table and offset as evidence; `language` argument, default `en` | M2.D2, M2.G1, M2.G8, M2.B2 | FULL of a `Starfield.esm` WEAP picked by EditorID from the corpus resolves to non-empty text with the table offset as evidence; a missing table yields the string ID plus a limitation, never "absent" |
| M2.D8  | `load` command (`{profile}` or `{plugins, data_dir}`, plus `language`): fills the session; result lists plugins with index, flags, masters, missing masters, cache hits | M2.F1, M2.D5 | Through `execute` with JSON; `kit load --profile ...` prints a summary; the MCP `kitcraft://load-order` resource returns the same list |
| M2.D9  | `inspect` across the load order: `form` = `Plugin.esm:00ABCDEF`, EditorID, or runtime ID (where M2.D3 allows); `plugin` optional (single-plugin mode kept); data: winner, every override with plugin and offsets, typed fields (M2.A3) with evidence, opaque payloads as hex, lstrings resolved | M2.D5, M2.D7, M2.A3 | On the official load order a record a DLC overrides shows both versions and the winner; every value has an evidence entry; the `docs/gaps.md` "inspect is single-plugin" row closed |
| M2.D10 | `where` language in `crates/doc/src/query.rs` (grammar below): parser with column-numbered errors, three-valued evaluation, paging | M2.D5, M2.A3 | A unit test per rule and error; 10,000 seeded random inputs never panic; grammar and semantics documented in `docs/mcp.md` (`query`) |
| M2.D11 | `query` command `{type, where?, scope = winner \| all, limit, cursor}` over the session | M2.D10, M2.D7 | `kit query --type WEAP --where 'FULL ~ "Old Earth Hunting Rifle"'` on the official load order returns that weapon with evidence; result has `matched`, `unknown`, `scanned`; paging through a cursor returns the same set as one large page; <= 10 s warm, release |
| M2.D12 | `diff` command `{a, b, form?}`: FormIDs mapped through each plugin's own masters by name; added, removed and changed records; changed records diffed per subrecord, field-level where typed, byte ranges where opaque | M2.D4, M2.A3 | A corpus file vs a copy with one edited subrecord reports exactly that change with offsets on both sides; a file vs itself is empty; plugins with differently ordered masters match by name |
| M2.D13 | `conflicts` command (new catalog entry; port of `esmscan.py`) `{master?, plugin?, type?, only_conflicts = false, limit, cursor}`: records that plugins override from a master, with M2.D6 classes | M2.D6 | Same per-type counts as `esmscan.py` run read-only on the same files (checked once, recorded in `plan/STATUS.md`); a synthetic test through `execute`; reachable over MCP |

#### `where` grammar (M2.D10)

```
query   = or ;
or      = and { "or" and } ;
and     = not { "and" not } ;
not     = "not" not | atom ;
atom    = "(" or ")" | "has" SIG | path op value ;
path    = SIG { "." member } [ "[" index "]" ] | "@form" | "@plugin" | "@edid" | "@flags" ;
op      = "==" | "!=" | "<" | "<=" | ">" | ">=" | "~" | "!~" | "^=" ;
value   = number | hex | string | formref | "true" | "false" ;
formref = plugin-file-name ":" hex ;          (Starfield.esm:0001A2B3)
```

- Keywords and member names are ASCII case-insensitive; `SIG` is four characters; members are
  the schema's member names. `EDID ~ "x"` and `FULL ~ "x"` compare the decoded text (FULL
  resolved through the string tables).
- `~` contains, `!~` does not contain, `^=` starts with (all ASCII case-insensitive).
  Ordering operators need a typed numeric member. FormID members compare as global forms
  against a `formref`.
- A repeated subrecord or array matches if any element matches; `[n]` picks one occurrence.
- A subrecord the record does not have makes a comparison false and `has` false (absent is
  known). A path into an opaque or undefined member, a payload that fails to decode, or a
  missing string table makes it **unknown**. `and`/`or`/`not` follow three-valued (Kleene)
  logic; unknown records are not matched, are counted in `data.unknown` (first 20 listed),
  and are named in `limitations`.
- Results are ordered by load-order index, then FormID. `limit` defaults to 50, maximum
  1,000. `cursor` is opaque (load-order fingerprint + position); a cursor from another load
  order is a `failed` error.

### Lane E: set + validate + build (`crates/engine`)

| ID     | Task | Depends on | Acceptance |
| ------ | ---- | ---------- | ---------- |
| M2.E1  | `FIELD=value` shorthand in the shared line parser for object arguments: `kit set <form> FULL="Old Soviet Marksman Rifle" --into X.esp`, `SIG.member=1.5`, `SIG=hex:0A0B...` | - | `parse_line` tests; every shorthand form gives the same args as `--fields '{...}'`; the `docs/gaps.md` row closed |
| M2.E2  | Field paths and values: resolve `SIG`, `SIG.member...`, `SIG[n]` against the schema; parse and range-check typed values into the generated field types (integers by width, floats, enums by name, zstrings, FormIDs as `formref`); opaque subrecords accept only `hex:`; lstrings per M2.E3 | M2.B2, M2.A3 | A unit test per type; a bad path or value is an error naming the path and the rule |
| M2.E3  | *(research)* Writing rules: HEDR record count vs counted records and groups on all 14 files; which record types ONAM lists in the corpus and whether an override-only plugin needs ONAM; HEDR version and next object id for a new plugin; and, from an M2.O4 `ck-diff` session, what the CK writes for a new non-localized plugin (extension and master flag, FULL as inline text and its encoding) | M2.A1 | Rules in a "Writing rules" section of `docs/corpus.md` with `corpus:` / `ck-diff:` sources; corpus rules asserted by tests; any `ck-diff` part not yet observed is listed as a limitation of `set` and `build` |
| M2.E4  | Patch writer (`src/edit/`): copy the winning override as the base, apply edits, write through M2.A5. Patches are non-localized: every lstring of a copied localized record is written as inline text (M2.D7), or the write is refused. Masters: a new target gets the source plugin's masters in order, then the source; an existing target takes a record only when the master lists are prefix-compatible, otherwise FormIDs are remapped through M2.B3, and the write is refused when a record needing remapping has opaque subrecords. Never writes under the game Data dir, the MO2 profile dir, or over a plugin in the load order | M2.A5, M2.A6, M2.B3, M2.D5, M2.D7, M2.E2, M2.E3 (corpus part) | Tests: new target; a second record into the same target; remap; each refusal; output roundtrips identical; HEDR count follows the M2.E3 rule |
| M2.E5  | `set` command: the result names the target file and every record changed, with fields before and after and evidence in the output file; records of the target not edited stay byte-identical | M2.E4, M2.E1 | Through `execute` with JSON and through the shorthand; `Implemented` |
| M2.E6  | `validate` framework: rule registry; findings `{rule, severity, plugin, record, subrecord, offset, message}`; `data.passed` false on any error; args `{plugin?, scripts?, rules?}` (plugin or scripts required); masters resolved through the session or next to the plugin | M2.D4, M2.D5 | Through `execute`; CLI exit 1 on errors; an unknown rule id is `invalid_args` |
| M2.E7  | Rules: `masters.missing`, `masters.order`; `formid.dangling` (header FormIDs and typed FormID members via M2.B3; unchecked opaque subrecords counted in `coverage`); `lstring.copied-id` (a non-localized plugin whose lstring payload is a bare 4-byte ID, or equals the localized master's payload for the same form); `override.stripped-name` (an override lacks the FULL or NTRM the previous version in load order has: the Resupply door); `record.flags-body` (compressed flag vs a body that does not decompress or has the wrong size; localized flag vs lstring payload shapes; HEDR count vs M2.E3) | M2.E6, M2.B3, M2.E3 | A positive and a negative synthetic fixture per rule (the Resupply case rebuilt synthetically; no third-party data in the repo); zero errors on the 14 official files, any warning listed with its reason in `plan/STATUS.md` |
| M2.E8  | Rule `papyrus.denylisted-call`: calls in `.pex` files (loose or in archives) to natives on a denylist seeded from our own disassembly of `C:\Modding\scripts\noc_godmode_patch` (names recorded with that observation) | M2.E6, M2.G5, M2.G1 | Fires on a synthetic `.pex` built in-test; on the local noc patch (skipped when absent) the original and patched scripts differ in findings as expected |
| M2.E9  | `build` from a TOML spec (below): rules expand to sets over `query` matches in file order, explicit `[[set]]` entries apply after rules, two writes to one field are an error; always writes a fresh plugin; runs `validate` on the output and fails on errors | M2.E4, M2.E7, M2.D10 | Two builds of one spec are byte-identical; the rename spec builds a plugin byte-identical to the M2.E5 `set` output; spec errors name the TOML key |
| M2.E10 | Rename smoke test: `docs/smoke-rename.md` and an automated test (skipped without `KITCRAFT_DATA_DIR`) renaming the Old Earth Hunting Rifle to "Old Soviet Marksman Rifle" in a new plugin (steps below) | M2.E5, M2.E7, M2.E9, M2.D9, M2.D11 | The test builds the plugin in a temp dir, `validate` passes, `inspect` with the plugin appended to the load order shows the new FULL winning, `roundtrip --verify` is identical; the doc covers the owner's MO2 steps; the M2.O6 result is recorded in `plan/STATUS.md` |

#### `build` spec, M2 subset (M2.E9)

```toml
version = 1                         # spec format version; anything else is an error

[plugin]
localized = false                   # the only value M2 accepts; masters are computed (M2.E4)

[load_order]                        # optional; default: the session's load order
profile = 'C:\Modding\MO2-Starfield\profiles\Default'

[[set]]                             # one record, as `kit set`
form = "Starfield.esm:0001A2B3"     # formref or EditorID
fields = { FULL = "Old Soviet Marksman Rifle" }

[[rule]]                            # every record of one type matching a `where`
type = "WEAP"
where = 'EDID ~ "Rifle"'
fields = { "SIG.member" = { mul = 0.8 } }   # a value, or { mul = x } / { add = x } on typed numbers
```

The output path is the command's `out` argument. New records, leveled-list injection and
scripts are not in M2.

#### Rename smoke test (M2.E10)

1. `kit load --profile "C:\Modding\MO2-Starfield\profiles\Default"`.
2. `kit query --type WEAP --where 'FULL ~ "Old Earth Hunting Rifle"'`. If several records
   match, the doc records which one is the base weapon and why (EditorID; the others are
   variants).
3. `kit set <form> FULL="Old Soviet Marksman Rifle" --into <staging>\<name>`, with the
   extension and flags M2.E3 settles; the staging dir is outside the game Data dir and the MO2
   profile. `kit build rename.toml --out ...` produces the same bytes.
4. `kit validate <staging>\<name>` passes, `lstring.copied-id` in particular.
5. `kit inspect <form>` with the plugin appended: the new name wins; the result gives the
   runtime FormID for the console.
6. Owner (M2.O6): in MO2 create an empty mod, copy the plugin in, enable the mod and its
   plugin at the end of the load order, launch through MO2, and check the name in the
   inventory (`player.additem <runtime id> 1`). If the name shown differs, check whether an
   instance naming rule (INNR) or an OMOD changes it, and record what was seen. The agent
   cannot launch the game; it prepares everything up to this step.

### Lane F: mcp + surface (`crates/mcp`, `apps/kit-cli`, `xtask`, engine core)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M2.F1 | Session plumbing: `Engine` owns a `Session` (load order, caches) that handlers can read and change; stateless commands unaffected; the free `execute` keeps working with a fresh session | - | Existing tests pass; a test handler keeps state across two `execute` calls on one engine |
| M2.F2 | `server.rs`: JSON-RPC 2.0 over stdio, one message per line: `initialize` (protocol revision pinned in `docs/mcp.md`), `notifications/initialized`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/read`; malformed JSON -32700, unknown method -32601; bounded line length; never panics. Hand-written over `serde_json`, or an MIT/Apache MCP crate only if `cargo xtask layers` passes with it | - | A unit test per method; a malformed-input table (truncated, oversize, wrong types, wrong id types) all answered with errors |
| M2.F3 | `tools.rs`: tools `execute`, `command_line`, `inspect_record`, `query_records`, `diff_plugins`, `coverage`; input schemas derived from the catalog `args_schema` (`execute` enumerates the catalog ids), descriptions from `summary` | M2.F2 | Test: every tool schema matches its catalog entry; adding a catalog command changes `execute`'s enum with no edit in `crates/mcp` |
| M2.F4 | `backend.rs`: tool call -> `engine::execute` on the server's session; envelope returned as structured content and as JSON text; error mapping per the `docs/mcp.md` table; resources `kitcraft://commands`, `kitcraft://load-order` | M2.F1, M2.F3 | A test per error kind; `load` then `inspect` in one session |
| M2.F5 | `kit mcp [--profile P]` launcher (not a `CommandSpec`, per AGENTS.md) | M2.F4 | `kit mcp` no longer exits 2; `--profile` preloads through `load`; `docs/mcp.md` exit-code table updated |
| M2.F6 | Smoke tests: an in-process stdio client in `crates/mcp/tests` (initialize -> tools/list -> tools/call `roundtrip` and `coverage` on a synthetic plugin); `apps/kit-cli/tests` spawns the real binary (`CARGO_BIN_EXE_kit`) and does the same | M2.F5 | Both run in `cargo xtask ci` without game data; with `KITCRAFT_DATA_DIR` a variant also calls `inspect_record` and `query_records` on official files |
| M2.F7 | Claude Code: `.mcp.json.example` at the repo root (release `kit.exe`, `mcp --profile <Default>`) and "Use from Claude Code" in `docs/mcp.md`; verify in a Claude Code session (or headless `claude -p`) that the tools list and that `inspect_record`, `query_records`, `execute roundtrip` and `coverage` return envelopes | M2.F6, M2.D9, M2.D11 | Transcript excerpt in `plan/STATUS.md`; no live `.mcp.json` committed |
| M2.F8 | Catalog closure: `Status::Deferred { reason, milestone }`; a test fails if any command is still `Planned` at M2 exit; `kit help` and `tools/list` show deferred reasons | - | Test in `cargo xtask ci`; at M2 exit every command is `Implemented` or `Deferred` with a reason |
| M2.F9 | `cargo xtask parity` reports typed-decoded and failed counts (M2.A3) and per-confidence counts (M2.B1) | M2.A3, M2.B1 | Regenerated `docs/record-parity.md` is idempotent and `--check`ed in `ci` as before |

### Lane G: archives + scripts + strings (`crates/ba2`, `crates/pex`, `crates/strings`)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M2.G0 | ADR 0006: sources for BA2, `.pex` and string-table facts: our scripts (ours); `uesp-skyrim` pages as hypotheses; official BA2s and scripts read in place as corpus (never committed); Microsoft's DDS documentation for DDS headers; `Archive2.exe` and `PapyrusCompiler.exe` run as black boxes only (never disassembled). It adds no source ADR 0005 forbids; one that would goes to the owner | - | ADR committed; `ATTRIBUTION.md` unchanged (no third-party bytes) |
| M2.G1 | `ba2` GNRL v2/v3 reader (port of `ba2x.py`): header, file records, name table, per-file streaming extraction; the v3 header field `ba2x.py` skips is inventoried from the official archives, and every codec found is supported (an MIT/Apache crate where needed) | M2.G0 | Synthetic fixtures; every official GNRL archive under `KITCRAFT_DATA_DIR` lists without error (versions, counts and codecs in `docs/corpus.md`); 100 seeded files per archive extract to their declared sizes; truncated and bit-flipped inputs give typed errors |
| M2.G2 | DX10 reader: texture records and chunks, DDS header synthesis, extraction | M2.G1 | Every official DX10 archive lists; extracted DDS headers are valid and sizes match the mip chain; one texture compared byte for byte with `Archive2.exe`'s extraction (any difference documented) |
| M2.G3 | `ba2` writer: pack GNRL, and DX10 from DDS files | M2.G1, M2.G2 | pack -> list -> extract returns identical files; packing is deterministic; `Archive2.exe` opens our archive and lists the same files |
| M2.G4 | `ba2.list`, `ba2.extract`, `ba2.pack` commands (extract refuses an `out_dir` under the game Data dir) | M2.G3 | Through `execute`; `Implemented`; reachable over MCP |
| M2.G5 | `pex` reader and disassembler (port of `pexdis.py`): structured model (header, string table, debug info, user flags, objects, structs, variables, properties, states, functions, instructions) with offsets; text output in `pexdis.py`'s format, plus JSON | M2.G0 | Every `.pex` in the official archives (via M2.G1) disassembles; text equals `pexdis.py`'s on a seeded sample of 50 (checked once, recorded in `plan/STATUS.md`); hostile input gives typed errors |
| M2.G6 | `papyrus.disasm` command | M2.G5 | Through `execute`; `Implemented` |
| M2.G7 | `papyrus.compile`: wraps Bethesda's `PapyrusCompiler.exe` (path argument or detected under the game's tools dir); **never** `-release` (the argument builder cannot emit it and refuses it in `args`); flags file and imports; compiler output captured in the result; never writes into Data | - | Argument-builder tests; with the compiler present, a trivial script compiles in a temp dir and disassembles through M2.G5; without it, a `failed` error naming the paths looked at |
| M2.G8 | `strings`: `.STRINGS`, `.DLSTRINGS`, `.ILSTRINGS` reader and writer | M2.G0 | Synthetic fixtures per variant; the official `Starfield_en` tables (read through M2.G1) rewrite byte-identical; hostile input gives typed errors |
| M2.G9 | `strings.dump`, `strings.merge` (later table wins, conflicts reported, output deterministic) | M2.G8 | Through `execute`; `Implemented` |

### Owner (lane O)

| ID    | Task | Gates |
| ----- | ---- | ----- |
| M2.O1 | **owner**: approve the ADR 0004 decision record, the matching ADR 0005 amendment, and `docs/xedit-reference.md`. **Done 2026-10-14** (ADR 0004 "Owner approval 2026-10-14": both `wbDefinitionsSF1.pas` and `wbDefinitionsCommon.pas` at the same SHA) | M2.B4..B8, M2.B10 |
| M2.O2 | **owner**: whether OMOD is in scope (ADR 0005 Amendment 1, decision 5). **Done 2026-10-14**: in scope | M2.B7 |
| M2.O3 | **owner**: download and install SF1Edit (agents do not download executables) | M2.B11 |
| M2.O4 | **owner**: CK and MO2 observation sessions (run them, or let an agent drive the desktop): MO2 priority check (M2.D1), small/medium flags and runtime IDs (M2.D3 c), a new non-localized plugin as the CK writes it (M2.E3) | M2.D1 check, M2.D3 (c), M2.E3 `ck-diff` part |
| M2.O5 | **owner**: whether third-party plugins under `C:\Modding\MO2-Starfield\mods` may serve as read-only inventory evidence (an ADR 0005 amendment). Default: no; they are loaded as the user's load order, but no fact is sourced from them | facts M2.E3 cannot get from the official corpus or the CK |
| M2.O6 | **owner**: in-game check of the rename plugin (M2.E10 step 6) | M2 exit |

### Ordering

```
Critical: A0 -> A1 -> D4 -> D5 -> D7 -> E4 -> E5 -> E10 -> O6

Into E4:  A1 -> A2 -> A5          A0 -> A6          A0 -> A3 -> B2 -> E2
          B2 -> B3                A1 -> E3 (corpus part)
Into D4:  A1 -> D3 (a, b)         O4 -> D3 (c), D1 check, E3 (ck-diff part)
Into D7:  D1 -> D2                G0 -> G1 -> G8          B2
Into E10: D5 + A3 -> D10 -> D11   D5 + D7 + A3 -> D9      D4 + D5 -> E6 -> E7 -> E9
                                  E1 -> E5

D:  D5 -> D6 -> D13      D5 + F1 -> D8      D4 + A3 -> D12
E:  E6 + G5 -> E8
F:  F1, F2 -> F3 -> F4 -> F5 -> F6 -> F7 (+ D9, D11)      F8 any time      A3 + B1 -> F9
G:  G1 -> G2 -> G3 -> G4      G0 -> G5 -> G6      G7 (no deps)      G8 -> G9
B:  B1 -> B2 -> B9      O1 -> B4 -> B5, B6, B8 -> B10      O2 -> B7      O3 + D9 -> B11
A:  A0 -> A4
```

The critical path is fed by G0 -> G1 -> G8 (string tables into D7), A3 -> B2 -> E2 (typed
FULL into E4), A2 -> A5 (the builder into E4), and the M2.O4 session (E3's `ck-diff` part,
needed before O6). MCP (F1..F7) runs in parallel and joins only at F7. Lane B's gated tasks
are off the critical path.

### M2 is done when

- `kit mcp` is usable from Claude Code with at least `inspect_record`, `query_records`,
  `execute roundtrip` and `coverage` (M2.F7 transcript in `plan/STATUS.md`).
- The rename plugin is built through `set` and through `build` (identical bytes) and passes
  `validate` (M2.E10); the owner's in-game result (M2.O6) is recorded in `plan/STATUS.md`,
  as a pass or as a failure with a follow-up task.
- `kit load --profile <Default>` resolves every enabled plugin, and `inspect`, `query`,
  `diff` and `conflicts` work across that load order (M2.D8..D13).
- Every command in the catalog is `Implemented` or `Deferred` with a reason (M2.F8).
- `cargo xtask ci` is green.
- The full corpus roundtrip is still identical: M1.6 `--release -- --ignored` and
  `kit roundtrip --verify` on all 14 files; `cargo xtask parity` regenerated.
- Lane B: M2.B1..B3 and M2.B9 done; M2.B4..B8, B10 and B11 done, or carried to M3 with the
  M2.O1 decision recorded.
- `docs/gaps.md` states what M2 closed and what it did not; `plan/STATUS.md` says M3.

## Not in M2

- Creating new records (new FormIDs); patches only override existing records.
- Localized output plugins (writing our own string tables for a patch); small/medium flagged
  output plugins until M2.D3 (c) has a source.
- Undo/redo, and sessions that persist across CLI processes (the MCP server keeps one
  session; each `kit` invocation loads its own).
- Matching the CK's save choices (ONAM, compression, header fields) beyond the M2.E3 rules:
  M3.
- Papyrus assembling, editing, or decompiling to `.psc`; NIF and materials; UI; viewport.
- `celldoors.py`, `fix_resupply_door.py` and `pexgrep.py` as commands (they become `query`,
  `build` and `validate` recipes later).
- Typed layouts beyond the record types listed in lane B.
- The gun overhaul spec (rung 2; owner decision 3) and Sim Settlements 2 (rung 3; owner
  decision 5).
- Writing into the game Data dir or the MO2 profile, launching the game, a git remote.

## M3: CK parity checklist and the gun overhaul

Goal: the real Creation Kit's output becomes evidence we can diff against; our writer can
save a plugin the way the CK does wherever that has been observed; the weapon record types
are typed from the pinned xEdit reference; and rung 2 of the step 2 ladder (`PLAN.md`)
ships a first real gun overhaul built from a data-driven spec. Concretely: lane B round 3; a
CK differential observation program (fixture matrix, `docs/ck-observations.md` ledger,
`kit diff`-driven findings, a `ck` write style); external oracles (SF1Edit, Spriggit) run
as programs; the `build` spec v2 and `kit overhaul`; the hardening backlog from
`docs/gaps.md` and the `plan/STATUS.md` Requests; and an ADR 0007 proposal for the UI, as a
decision point only.

This section extends the scope line at the top of this file to M3. M2 is closed except two
owner checks that stay M2 tasks: the M2.F7 Claude Code transcript and the M2.O6 in-game
rename. Carried M2 tasks get M3 IDs: M2.B4..B8 and M2.B10 become M3.B1..B5 and M3.B7,
M2.B11 becomes M3.B11, M2.O3 becomes M3.O3, and M2.O4 splits into M3.O1 (CK) and M3.O2 (MO2
and in game). M2.O5 (third-party plugins as evidence) keeps its default, **no**.

Markings as in M2: **owner** tasks need the owner (AGENTS.md "Owner decisions", or
something only the owner can do: install software, drive the CK or the game, unless the
owner lets an agent with desktop control drive the CK); *(research)* tasks establish a fact
before code relies on it, recorded with its ADR 0005 source form.

### Binding constraints (owner, ADR 0004 "Owner approval 2026-10-14")

The six clarifications bind every M3 task. How each applies here:

1. **`wbDefinitionsCommon.pas`** is cited at the pinned SHA only for what a Starfield
   definition uses a helper for; no Skyrim-specific reading is imported (lane B round 3).
2. **Another upstream helper file** needs only a documented dependency
   (`docs/xedit-reference.md`, `docs/source-registry.md`) under the same rules; no new
   policy round unless its license or source category differs (then it is the owner's).
3. **Confidence semantics.** `reference` means documented in an allowed source, not
   understood. Upstream coverage never raises confidence; only our own corpus checks,
   `ck-diff` observations and independent validation do (M3.B12). A write that depends on
   a `reference`-only member is refused by default (M3.E6), per ADR 0005 Amendment 3 §4
   (claims that affect what we write are validated independently).
4. **Lossless first.** A typed member never makes a roundtrip lossy; whatever is not
   understood stays opaque. The `ck` write style changes only knobs a ledger entry backs;
   unedited records are re-emitted as read in every style.
5. **FormID resolution.** Raw stored FormIDs stay separate from master-aware resolved
   identities; the mixed-tier fixtures (F09, F10) test it; an unavailable master's scale is
   reported unknown, never guessed.
6. **Scope discipline.** No architecture restart, no licensing revisit without new
   evidence, no lane waiting on speculative compatibility. The UI is an ADR only (M3.F4)
   unless the owner opts in; Sim Settlements 2 is not in M3; only the oracles named here
   are run. A task gated on an owner session ships what it can without that session and
   lists the rest as a limitation.

### Lanes (ownership)

Lanes A, B, D, E, F and G keep their M2 files. Lane H is new and takes the observation and
oracle tooling out of lane F's `xtask`. One agent per lane, each in its own worktree and
`CARGO_TARGET_DIR`.

| Lane | Name                         | Owns |
| ---- | ---------------------------- | ---- |
| A    | container                    | `crates/esm` (not `src/generated/`) |
| B    | schema                       | `crates/schema` (including `defs/`), `docs/schema-differential.md` |
| D    | doc + io                     | `crates/doc`, `crates/io`, `docs/load-order.md`; engine `handlers/{load,inspect,query,diff,conflicts,refs}.rs` |
| E    | set + validate + build       | engine `src/edit/`, `src/validate/`, `src/build/`, `handlers/{set,validate,build,overhaul}.rs`; `docs/smoke-rename.md`, `docs/validate.md`, `docs/overhaul/` |
| F    | mcp + surface                | `crates/mcp`, `apps/kit-cli`, `xtask` (except lane H's modules), engine core as in M2, `docs/record-parity.md`, the protocol parts of `docs/mcp.md`, `plan/adr/0007-*` |
| G    | archives + scripts + strings | `crates/ba2`, `crates/pex`, `crates/strings`; engine `handlers/{ba2,papyrus,strings}.rs` |
| H    | evidence: CK + oracles       | `docs/ck-observations.md`, `docs/ck-parity.md`, `docs/oracles.md`, `xtask/src/observe.rs`, `xtask/src/oracle.rs`, the `observe/` layout (gitignored; nothing in it is ever committed) |
| O    | owner                        | decisions, installs, CK, MO2 and game sessions, reviews |

Shared files follow the M2 rules (small separate commits by the lane whose command it is);
the lines registering lane H's subcommands in `xtask/src/main.rs` are lane H's. Lane H's
findings reach other lanes as `plan/STATUS.md` Requests naming the ledger entry.

### Format facts M3 relies on, and where they come from

Hypotheses until the named check confirms them.

- The CK, launched through MO2 (executable #4), is expected to save into MO2's virtual Data
  folder, which lands in `C:\Modding\MO2-Starfield\overwrite\`. Where each file actually
  lands is recorded per fixture (M3.O1); nothing is ever moved into the game Data dir.
- The CK's working file is an `.esp`; shipping files are exported as `.esm` (LOOT-GUIDE in
  `docs/source-registry.md`, docs-only, grade B). Which export options the CK offers and
  which TES4 bits each sets is observed in F11.
- Medium flag `0x400` (pinned xEdit `TES4` flags; corpus: SFBGS003, SFBGS006) versus the
  stale `0x10000` (ADR 0005 Amendment 3 §5): F11 settles it for a CK export.
- TES4 metadata: the 12 pipeline-built official files carry `SNAM`, `BNAM` and `CHGL`; the
  two BlueprintShips files and our patches do not (`docs/corpus.md` "Writing rules"). What
  the CK writes for a new plugin is F01's and F02's first question.
- Recompression: 160 of 136,741 compressed corpus records (0.12%) do not recompress
  identically with zlib-rs at level 9 (148 at no level; `docs/gaps.md`). Whether the CK
  reproduces the original bytes, ours, or neither is F07's question.
- SF1Edit and Spriggit are run as programs only (`docs/source-registry.md`:
  external-oracle-only). Spriggit's YAML is never parsed or read for names: Spriggit is used
  only as an ESM -> YAML -> ESM round trip whose binary output our own tools compare.

### Lane A: container (`crates/esm`)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M3.A1 | *(research)* Recompression misses: list the 160 records (file, type, FormID, compressed and inflated sizes, first differing byte of the stream); try zlib-rs parameter variants out of tree (window bits, memory level, strategy) and classify each record; pick up to 5 that the CK can open in a dialog as F07 candidates for M3.H1 | - | Table in `docs/gaps.md` "Recompression parity" (per file, per type, per class); the candidate list filed as a Request to lane H; no change to the default writer without M3.H3 evidence |
| M3.A2 | `roundtrip --decode` in the driver: inflate every compressed body, check the declared size, re-emit; a corrupt but well-sized stream is a mismatch with offset and path | - | A synthetic corrupt-stream fixture reports a mismatch; all 14 files identical with `--decode` (time recorded in `docs/corpus.md`); closes the `docs/gaps.md` row together with M3.F1 |
| M3.A3 | Write styles: `WriteStyle::{Kit, Ck}` on `HeaderSpec` and the record writer. `Ck` knobs, each only once a ledger entry backs it: TES4 subrecords (`CNAM`, `SNAM`, `BNAM`, `CHGL`, `INCC`, `ONAM`), the `HEDR` count meaning and next object ID, record header fields the CK sets on overrides (timestamp, version control info, form version), compression per record type and compressor settings, the `XXXX` threshold. `ck_style_unobserved()` lists the knobs with no entry yet | M3.H3, M3.A1 | `Kit` output unchanged (the rename plugin keeps SHA-256 `9C0B28A6...4078EA27`); every `Ck` knob cites its observation id in code and in `docs/corpus.md` "Writing rules"; unedited records re-emit as read in both styles; a unit test per knob |
| M3.A4 | CK fixtures as container tests (gates G0, G1): every CK-saved plugin under `observe/` roundtrips byte-identical and survives the M1.7 truncation and bit-flip harness; each structural feature a fixture shows (mixed master tiers, `XXXX`, a CK-compressed record) gets a synthetic in-test builder so the gate covers it without CK bytes | M3.O1 (fixtures); the synthetic part none | The test skips with one line when `observe/` is absent; with the fixtures, 0 mismatches and 0 panics; synthetic builders committed, no CK-saved byte committed |
| M3.A5 | Structure-aware mutation on stable Rust (the `docs/gaps.md` fuzzing row): seeded mutations of size fields, `XXXX` sizes, decompressed-size prefixes, group nesting and subrecord counts, over synthetic fixtures and the three small corpus files; the long run is `#[ignore]` | - | 100,000 seeded mutations per small file in the long run, no panic, every failure typed with an offset; crashes found ship as regression tests; the gaps row restated (cargo-fuzz still needs nightly) |

### Lane B: schema (`crates/schema`), round 3

Round 3 is gated on the two pinned xEdit files being on disk (M3.O0). Order as in
`plan/STATUS.md`: weapons first, then the KYWD and GMST remainders, then LVLI, FLST, INNR,
with the lstring signatures alongside. Each typed layout lands with corpus acceptance; the
SF1Edit comparison (M3.B11) is no longer part of each type's acceptance (it was in M2.B4),
so round 3 does not wait on M3.O3. Confidence per ADR 0004: `reference` until the corpus
confirms sizes, counts and order on every occurrence (`corpus`); `ck-diff` or `verified`
only through M3.B12. Every task: no Pascal code or comments copied (the reviewer checks the
diff); names from the CK wiki or the CK's UI labels where they exist, never an upstream
placeholder label; union-decided members stay opaque until M3.B9 can express their decider.
The staged files are deleted when round 3 closes (ADR 0004) and re-staged for any later
round.

| ID     | Task | Depends on | Acceptance |
| ------ | ---- | ---------- | ---------- |
| M3.B0  | Verify the staged files before any read: size and git blob hash of `wbDefinitionsSF1.pas` (789,023 bytes, blob `ace733b5...`) and `wbDefinitionsCommon.pas` (338,167 bytes, blob `cfa80ba0...`) against `docs/xedit-reference.md`; record the path and the result in `plan/STATUS.md` | M3.O0 | Both match, or round 3 stops and the mismatch goes to the owner |
| M3.B1  | WEAP typed (was M2.B4) | M3.B0 | Every member has `source` (`xedit:` at the pin with a line range) and `confidence`; through the M2.A3 driver every corpus WEAP (452) decodes with an empty `extra` and re-encodes identical on all 14 files, or the member is demoted to opaque with the conflict noted; members confirmed on every occurrence are `corpus` with a `corpus:` citation naming the test; `kit schema --type WEAP --deep` lists every member; `docs/record-parity.md` regenerated |
| M3.B2  | AMMO, PROJ typed (was M2.B5; PROJ gets its corpus inventory definition first) | M3.B1 | As M3.B1, per type |
| M3.B3  | AMDL typed (was M2.B6) | M3.B1 | As M3.B1 |
| M3.B4  | OMOD typed (was M2.B7; in scope per M2.O2) | M3.B1 | As M3.B1; property arrays whose layout depends on another value stay opaque until M3.B9 |
| M3.B5  | COBJ, ZOOM typed (was M2.B8) | M3.B1 | As M3.B1, per type |
| M3.B6  | KYWD bodies (`CNAM`, `DNAM`, `TNAM`, `FNAM`, `DATA`, `ENAM`) and GMST `XALG` typed from the pinned file | M3.B0 | As M3.B1, per type; the `docs/gaps.md` row closed |
| M3.B7  | LVLI, FLST, INNR typed (was M2.B10; inventory definitions first) | M3.B5 | As M3.B1, per type; LVLI entry lists kept consistent with their counts per M3.B10 |
| M3.B8  | The 105 corpus lstring signatures (lane E's request: `edit::lstrings`, per record type) typed as `lstring` members, whole subrecord or struct tail as the pinned file lays them out; record types without a definition get an inventory definition (corpus order and cardinality, other members opaque) | M3.B0 | A test asserts the schema's set of lstring members equals the corpus table; `confidence = "corpus"` cited to `lstring_table_matches_the_corpus` (plus `xedit:` where the pinned file agrees; where it disagrees the corpus wins and the conflict is noted); every corpus occurrence decodes and re-encodes identical |
| M3.B9  | Union deciders beyond `edid-prefix`, only as a round 3 type needs them (a sibling field's value, the presence of another subrecord); each new decider kind documented in `defs/README.md` | M3.B4 (first user) | Golden and corpus tests per decider; the members that needed it stop being opaque; the `docs/gaps.md` unions row restated |
| M3.B10 | `count_from` consistency: generated encode writes the count from the array length (or the setter refuses a mismatch), so an edited array never carries a stale count | - | Golden tests; editing a `count_from` array through `Record::set_typed` writes a consistent payload; corpus re-encode still identical; the gaps row closed |
| M3.B11 | SF1Edit differential harness (was M2.B11): run SF1Edit as a program on the official files (never read its source), capture the values it shows for sampled records of each round 3 type, compare with `kit inspect --json` | M3.O3, M3.H4, M3.B1 | `docs/schema-differential.md`: SF1Edit build and its skew from the pin, procedure, seeded sample ids (>= 20 per type), per-type tables; every mismatch explained (our error, version skew, upstream defect) or the member demoted to opaque |
| M3.B12 | Confidence from evidence: for each ledger entry that confirms a member, add the `ck-diff:` citation and raise `confidence` to `ck-diff` (or `verified` with corpus agreement); a refuting entry demotes the member and notes the conflict | M3.H3 | Loader citation checks pass; `cargo xtask parity` shows the per-confidence counts; every promotion names its observation id |

### Lane D: doc + io (`crates/doc`, `crates/io`)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M3.D1 | Lane E's round 2 requests: `Loaded::missing_active_plugins` (a field, not a limitation string) and a "safe source" predicate per winning version; `strings::Resolved.bytes` (the exact table bytes next to the lossy `text`) | - | Unit tests; lane E no longer matches limitation text; non-UTF-8 official entries expose their bytes |
| M3.D2 | Field-level `diff` and `conflicts` where a record type is typed: changed members by path with before and after; FormIDs inside typed payloads mapped through each plugin's masters (M2.B3 visitors) before comparison; opaque members stay byte ranges | M3.B1 (weapon fields; the generic code can start earlier) | A corpus record vs a copy with one typed member changed reports that member's path; differently ordered master lists compare equal by name; the `docs/gaps.md` row restated per type |
| M3.D3 | `reference`-confidence values surface in `inspect` and `query` results as `limitations[]` entries naming the members (the open Policy request) | - | A test through `execute` on a definition with a `reference` member; `docs/mcp.md` updated |
| M3.D4 | Link following and references: `follow(form, member path)` resolving typed FormID members to winning versions (for `build`'s `via`, M3.E6), and a `refs` command (the CK's "Use Info", `docs/ck-parity.md` P1): every record whose header or typed FormID members name a form, with the count of opaque payloads not scanned in `limitations` | M3.B1, M3.B2 | Through `execute` with JSON; on the official load order `refs Starfield.esm:0021BBCD` lists the records that name the rifle in typed members, with evidence; reachable over MCP through `execute` |
| M3.D5 | Load-order assumptions against observation: `runtime-slots-by-scale`, `hardcoded-official-slots` and `full-style-self` checked against M3.O2's in-game IDs and the CK's `EditorDataFilesLoaded.txt` (F00); each kept with its evidence or replaced | M3.O2, M3.O1 | `docs/load-order.md` cites each observation; the `docs/gaps.md` rows on runtime slots, MO2 rules and `full-style-self` closed or restated |
| M3.D6 | Gaps hygiene for lane D rows: a corpus check that every lstring ID in the 14 files exists in its string tables (unresolved ones listed); confirm that `inspect` decodes through `Plugin::typed` and close the stale schema/inspect row | - | A corpus test (release, `#[ignore]` if slow) with counts in `docs/corpus.md`; both `docs/gaps.md` rows closed or restated |

### Lane E: set + validate + build (`crates/engine`)

| ID     | Task | Depends on | Acceptance |
| ------ | ---- | ---------- | ---------- |
| M3.E1  | Replace the blanket "load order unverified" refusal with M3.D1's fields: refuse only when the record's own winner or its masters are affected | M3.D1 | The Default profile builds the rename without `--plugins`; a synthetic load order with a missing active plugin unrelated to the record builds with a limitation; one related to it refuses |
| M3.E2  | Copy non-UTF-8 official strings exactly through `Resolved.bytes` instead of refusing them | M3.D1 | A test with a non-UTF-8 table entry: the inline bytes equal the table bytes; the refusal is gone |
| M3.E3  | `validate` hardening (the `docs/gaps.md` row): `.pex` inside a plugin's archives (lane G's `ba2` API), `formid.dangling` on `FE`/`FD` FormIDs through lane D's scale-aware resolution, opaque-FormID counts shrinking as round 3 types land | - | Synthetic fixtures per new case; `validate_corpus_official_files` still 0 errors (warnings listed with reasons in `plan/STATUS.md`) |
| M3.E4  | lstrings from the schema: once M3.B8 lands, `set`, `build` and `validate` take lstring members from lane B's definitions; `edit::lstrings` becomes a cross-check test; lstrings inside struct payloads are seen | M3.B8 | The corpus lstring test compares schema and table; the `docs/gaps.md` lstring row closed |
| M3.E5  | `style` argument on `set` and `build` (`kit` default, `ck`) over M3.A3; unobserved knobs listed in `limitations` | M3.A3 | For every CK edit fixture our commands can express (F02..F05, F07, F08, F13; `kit_equivalent` in its `fixture.toml`), `style: "ck"` output equals the CK's file byte for byte, or each differing range is explained in the ledger (checked by `cargo xtask observe` and by a test that skips without `observe/`); the default stays `kit` in M3 |
| M3.E6  | `build` spec v2 (below): named classes, record-type-wide rules over typed WEAP, AMMO, PROJ and AMDL members (OMOD, COBJ, ZOOM where typed), `via` link following, operators, `plan` (dry run) | M3.B1, M3.B2, M3.B3, M3.D4 | Two builds of one spec are byte-identical; v1 specs still build identically; a test per operator and per error (unknown member, two writes to one field, a `reference`-only member without `allow_reference`, a rule matching nothing under `strict`); `plan` lists every change with before and after and writes nothing |
| M3.E7  | `overhaul.scaffold` command (CLI `kit overhaul`, alias `overhaul`): from `{type, where?, group_by?, out}` emit a starter v2 spec: one class per group, match counts and the min/median/max of each typed numeric member as comments, every rule commented out | M3.E6 | Through `execute` with JSON; deterministic; refuses an existing `out` without `overwrite`; `build --plan` on a scaffold reports 0 changes; `kit overhaul --type WEAP` on the official load order in <= 10 s warm, release |
| M3.E8  | Design brief template `docs/overhaul/brief.md` for owner decision 3: the critique's source (title, channel, URL, date watched), one row per critique point (claim in the owner's words, timestamp, weapon classes, desired change, measurable target), out of scope, the in-game checklist; and the traceability rule (each spec class and rule names its brief row) | - | Template committed; no video text transcribed (points are written in the owner's words) |
| M3.E9  | First real build on the owner's profile: translate the filled brief into `docs/overhaul/overhaul.toml` (scaffolded by M3.E7, every rule naming its brief row) and build `out\<name>.esm` from the Default profile | M3.E6, M3.E7, M3.E1, M3.O4 | `validate` 0 errors; the `build --plan` report and per-class change counts in `docs/overhaul/build-1.md`; `roundtrip --verify` identical; overrides only (no new records, no leveled-list injection, no scripts), so the plugin can be dropped from a save; the M3.O5 steps written there |
| M3.E10 | Remapping with typed records: a weapon from a plugin whose master order differs joins an existing patch through the M2.B3 visitors now that WEAP is typed (the `docs/gaps.md` remap row) | M3.B1 | A synthetic test: remapped FormIDs resolve to the same global forms; still refused when any subrecord of the record is opaque |

#### `build` spec v2 (M3.E6)

```toml
version = 2                         # 1 is still accepted, with M2's meaning

[plugin]
localized = false                   # still the only value
style = "kit"                       # or "ck" (M3.E5)

[load_order]
profile = 'C:\Modding\MO2-Starfield\profiles\Default'

[class.ballistic_rifle]             # a named set: one record type and a `where`
type = "WEAP"
where = 'KWDA == Starfield.esm:<keyword> and not (EDID ~ "Test")'
brief = "B3"                        # the brief row this class serves (M3.E8)

[[rule]]
class = "ballistic_rifle"
fields = { "SIG.Member" = { mul = 0.8 } }

[[rule]]
class = "ballistic_rifle"
via = ["SIG.Member", "SIG.Member"]  # follow typed FormID members (e.g. weapon -> ammo -> projectile)
fields = { "SIG.Member" = { clamp = [0.0, 1.0] } }
allow_reference = false             # true allows writing a `reference`-only member (named in limitations)
```

- Member paths are lane B's schema names; the placeholders above stand for them (no layout
  is written in this plan).
- Operators: a value, `{ mul }`, `{ add }`, `{ min }`, `{ max }`, `{ clamp = [lo, hi] }` on
  typed numbers; only a value on other types.
- A record reached by several rules or classes (one ammo used by many weapons) is written
  once; two different writes to one field are an error naming both spec keys. Explicit
  `[[set]]` entries still apply after rules.
- A rule matching nothing is a warning, and an error with `strict = true`.
- `plan = true` (CLI `--plan`) returns the change list without writing.
- Still not in v2: new records, leveled-list injection, scripts.

### Lane F: mcp + surface (`crates/mcp`, `apps/kit-cli`, `xtask`, engine core)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M3.F1 | `roundtrip --decode` in the catalog, CLI and MCP; end-to-end exit 1 on a synthetic plugin with a corrupt compressed stream | M3.A2 | A spawned-binary test exits 1 with `data.passed: false`; both `docs/gaps.md` roundtrip rows closed |
| M3.F2 | `coverage` decodes through `Plugin::typed`: typed subrecords whose payload fails its layout are counted apart; `cargo xtask parity` adds typed-member and per-confidence counts per record type | - | The regenerated `docs/record-parity.md` is idempotent and `--check`ed; the "classify by signature only" gaps row closed |
| M3.F3 | Hygiene: stale `docs/gaps.md` rows (`PLANNED_IN_M2`, "MCP server not implemented"), answered `plan/STATUS.md` Requests struck through with where they were done, `docs/mcp.md` command list for `refs` and `overhaul.scaffold` | - | Rows and requests updated in small separate commits; no row names a closed task as open |
| M3.F4 | ADR 0007 (status: Proposed): an egui shell over the command catalog: `crates/ui-egui` (L5) and `apps/kit`; command palette and menus from `CommandSpec` (`label`, `menu_path`, `shortcut`, `aliases`); argument forms generated from `args_schema`; results from the envelope (`limitations`, `evidence`); no logic the CLI and MCP lack; layer bans unchanged; viewport out of scope | - | ADR committed with options (egui in M3, defer to M4, another toolkit), costs and a recommendation; no UI code; the owner's decision recorded by M3.O6 |
| M3.F5 | Only if the owner opts in (M3.O6): `kit ui` launcher and a minimal shell (palette, argument form, result view) over `execute` | M3.O6 | `cargo xtask layers` passes with the UI crates; every catalog command reachable from the palette; one test calls a command through the shell's path without a window |

### Lane G: archives + scripts + strings

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M3.G1 | `papyrus.compile` reads `Tools\ContentResources.zip` directly (an MIT/Apache zip crate; `cargo xtask layers` clean) | - | The M2.G7 scripts compile given only the zip path; the `docs/gaps.md` row closed |
| M3.G2 | Tolerant string-table parse mode for padded tables (the optional item from lane G round 2) | - | Synthetic padded tables parse with a limitation naming the padding; strict mode unchanged |
| M3.G3 | *(research, P3)* Archive2's hash rule for non-ASCII names from black-box Archive2 runs (`docs/gaps.md` row); `ba2.pack` keeps refusing until the rule is established | - | Observations in `docs/corpus.md` "Archives"; either the rule plus tests, or the row restated with what was tried |

### Lane H: evidence (CK observation and oracles)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M3.H1 | Fixture matrix and session scripts: create `docs/ck-observations.md` (ledger format per ADR 0005 §2 step 6, plus the matrix below); resolve the scripts' placeholders to concrete forms from the corpus (`kit inspect`, `kit query`): the rifle's ammo, one float GMST, one compressed NPC_, one record whose master version carries `XXXX` and that the CK edits through a dialog, M3.A1's candidates, one record each from `Constellation.esm`, `SFBGS004.esm` and `SFBGS006.esm`, and lane D's lists for M3.O2; a `fixture.toml` template | M3.A1 (F07 candidates; the rest can start now) | Every placeholder of the M3.O1 and M3.O2 scripts resolved with FormID, EditorID and why it was chosen; the template documented |
| M3.H2 | `cargo xtask observe <fixture-dir>`: SHA-256 of every file; `kit roundtrip --verify`, `coverage`, `validate`; `kit diff` and a byte-range diff against the fixture's base and against our equivalent (`kit_equivalent` in `fixture.toml`, run in both styles); writes `report.md` and `report.json` inside the fixture dir only | - | Tests on synthetic fixture dirs; rerunning is idempotent; nothing written outside the fixture dir |
| M3.H3 | Findings: run M3.H2 on every M3.O1 fixture and write one ledger entry per finding (id `ck-<yyyymmdd>-<nn>`: CK version, game build, dialog and field label, signature, offset, width, sentinels and bytes, base and changed SHA-256). First targets: (1) TES4 of a new `.esp`: `CNAM`, `SNAM`, `BNAM`, `CHGL`, `INCC`, `ONAM`, `HEDR` version, count meaning and next object ID; (2) compression choice per record type, and the CK's stream for F07's records vs the corpus's and ours (the 160 misses); (3) `XXXX`; (4) localized vs not (F02 vs F12) and inline text encoding (F13); (5) export flags and scale bits (F11; the `0x400` conflict entry); (6) FormID allocation (F06); (7) raw FormIDs under mixed master tiers (F09, F10) vs lane D's resolver (gate G5); group type 10 and the BlueprintShips `HEDR` meaning if a fixture shows them | M3.O1, M3.H2 | Each target answered with an entry id or marked not observable with the reason; each answer filed as a Request to its lane (A writer knobs, B confidence, D, E); the `docs/gaps.md` rows marked "M3 (CK observation)" closed or restated; the `docs/ck-parity.md` findings table filled |
| M3.H4 | Oracle harness `cargo xtask oracle {spriggit,sf1edit} <plugin>`: tool paths from `KITCRAFT_SPRIGGIT` / `KITCRAFT_SF1EDIT`, tool version recorded. Spriggit: ESM -> YAML -> ESM in a temp dir, then our `diff` and `coverage` compare input and output (the YAML is never parsed or read). SF1Edit: record counts per type, FormIDs and EditorIDs as SF1Edit shows them, and its own error check on our outputs (gate G3); the capture method is chosen and documented in `docs/oracles.md` (its own export or dump features run as a program; a script we write for its scripting engine is written from its user docs, never from xEdit source, and stays outside the repo) | M3.O3 (SF1Edit part only) | `docs/oracles.md`: per tool, the build, the fixture set (the three small corpus files, the rename plugin, M3.O1 fixtures, the overhaul build) and a disagreement table; each disagreement recorded as evidence (ours, theirs, version skew), never as a corruption claim without our own byte evidence |
| M3.H5 | Rank CK editor features in `docs/ck-parity.md` (the ROADMAP M3 bullet): extend the checklist with what M3.O1 shows (menus, dialogs, save and export options, warnings on load), map each to a command or none, P1/P2/P3; viewport and world editing stay P3, deferred | M3.O1 | Every checklist row has evidence (a ledger id or session note); P1 rows without a command are proposed to the owner as M4 candidates |

#### Fixture matrix (M3.H1; dossier section 4)

Each fixture is a folder `C:\Modding\kitcraft\observe\<yyyy-mm-dd>-<id>\` (gitignored) with
the CK-saved files, screenshots and a `fixture.toml`. Plugin names start with `KC_`. Gates
are the dossier's: G0 parse, G1 lossless passthrough, G2 narrow edit, G3 cross-tool, G4
editor/game, G5 dependency semantics.

| ID  | Fixture | Base | Gates | First question |
| --- | ------- | ---- | ----- | -------------- |
| F00 | Session metadata: CK version (title bar), game build (`Starfield.exe` Properties > Details), MO2 version, the CK's `EditorDataFilesLoaded.txt` | - | - | The load order as the CK sees it (lane D) |
| F01 | Empty plugin, master `Starfield.esm` only | - | G0, G1 | TES4 of a new `.esp` |
| F02 | Override of WEAP `Starfield.esm:0021BBCD` (`RussianHuntingRifle`), FULL = "Old Soviet Marksman Rifle" | - | G1, G2 | `kit diff` against our rename plugin (the M2.E3 `ck-diff` half) |
| F03 | WEAP: one numeric field changed with two sentinels (`F03a`, `F03b`), one checkbox toggled (`F03c`) | F02 | G2 | Field offset, width, encoding; whether anything else moves |
| F04 | The rifle's AMMO: one numeric field, two sentinels | F02 | G2 | As F03 |
| F05 | One float GMST, two sentinels | F01 | G2 | As F03 |
| F06 | One new GLOB | F01 | G1 | FormID allocation, `HEDR` next object ID |
| F07 | Override of one compressed NPC_ (one field changed) and of M3.A1's candidates (one trivial edit each) | F01 | G1, G2 | Compression choice and bytes |
| F08 | Override of the `XXXX` record (a field outside the large subrecord changed) | F01 | G1 | `XXXX` writing |
| F09 | Two masters: `Starfield.esm` + `SFBGS004.esm` (small), one override from each | - | G5 | Raw FormIDs, master list |
| F10 | Mixed tiers: `Starfield.esm` (full) + `Constellation.esm` (small) + `SFBGS006.esm` (medium), one override from each | - | G5 | Raw FormIDs per scale vs lane D |
| F11 | Exports of F06 (with its new record): full `.esm`, medium, small, as far as the CK offers each | F06 | G1, G5 | Flags, scale bits (`0x400`), own-FormID form after export |
| F12 | F02's edit saved as a localized plugin, if the CK offers it (with the string tables it writes) | F02 | G1 | lstring IDs and tables vs `crates/strings` |
| F13 | F02 with a non-ASCII name (`Fusil côté Ä`) | - | G2, G4 | Inline text encoding (the `docs/gaps.md` row) |
| F14 | The CK loads our rename plugin (later also the overhaul build) as a non-active plugin | - | G3, G4 | Warnings the CK reports on our output |

### Owner (lane O)

| ID    | Task | Gates |
| ----- | ---- | ----- |
| M3.O0 | **owner**: stage `Core/wbDefinitionsSF1.pas` and `Core/wbDefinitionsCommon.pas` at SHA `9fb016884bec138ea6c7b872cec831537d464c3e` outside the repo (suggested `C:\Modding\xedit-ref\9fb01688\`) and record the path in `plan/STATUS.md` (Blockers); or allow the lane B session to fetch them from raw.githubusercontent.com at that SHA | M3.B0 and all of lane B round 3 |
| M3.O1 | **owner** (or an agent with desktop control the owner assigns): CK observation session per the script below (was M2.O4, CK part); two sittings are fine (F00..F06 and F13, then F07..F12 and F14) | M3.H3, M3.A3, M3.A4, M3.E5, M3.H5, M3.D5 (F00) |
| M3.O2 | **owner**: MO2 and in-game session per the script below (was M2.O4, MO2 and game parts) | M3.D5; the inline-encoding gaps row (with F13) |
| M3.O3 | **owner**: download and install SF1Edit and note its build (was M2.O3; agents do not download executables) | M3.B11, M3.H4 (SF1Edit part) |
| M3.O4 | **owner**, decision 3: pick the YouTube critique and fill `docs/overhaul/brief.md` (M3.E8) in your own words | M3.E9 |
| M3.O5 | **owner**: in-game acceptance of the overhaul build on a disposable save (steps in `docs/overhaul/build-1.md`); record a pass, or a failure with what was seen | M3 exit |
| M3.O6 | **owner**: decide ADR 0007: build the UI shell in M3 (M3.F5), or defer it to M4 | M3.F5 |

Still open from M2 and unchanged: M2.F7 (Claude Code transcript), M2.O6 (in-game rename;
M3.O5 reuses its procedure and should follow a pass), owner decisions 1, 4, 5 (Sim
Settlements 2: still "later") and 6.

#### M3.O1: CK session script

Rules for whoever drives: launch the CK only through MO2 (executable #4); observe only, no
debugger, disassembler or memory tool; never use the CK's Creations, upload or Bethesda.net
login features; screenshots and saved files go only to `observe\`. Before starting, note
what is in `C:\Modding\MO2-Starfield\overwrite\` (ideally nothing).

For each fixture:

1. *File > Data*: tick only the masters the matrix names (F14: also our plugin), no active
   file unless the step says so; OK; wait for loading to finish. Screenshot the dialog.
2. Do the fixture's action. To edit a record: Object Window, filter by the EditorID or
   FormID M3.H1 resolved, open it, change only the named field to the sentinel M3.H1 chose,
   OK. For a new GLOB: Object Window > Global, right-click > New. For a GMST: the CK's
   game-settings editor. Screenshot the dialog before and after the edit. Where a menu path
   differs from these, record the one used.
3. *File > Save* as `KC_<id>.esp` (`KC_F03a.esp` etc. for variants). Start each variant
   from a fresh load of its base fixture, never from another variant. For F11 use the CK's
   own export to master; for F12 its localized save, if offered; record every option the
   dialog shows.
4. Find where the file landed (expected: `overwrite\`) and move it, with any string tables
   the CK wrote alongside, into `C:\Modding\kitcraft\observe\<yyyy-mm-dd>-<id>\`. Leave
   `overwrite\` as you found it. If a file appears under the game's `Data` folder instead,
   move it out too and note it.
5. Fill `fixture.toml` from the template: CK version, game build, masters ticked, actions
   as performed, sentinels, file names, where the file landed, every warning or message the
   CK showed (with a screenshot). If the CK refuses an action (for example saving an
   unchanged plugin), record the message and go on.

F00 is done once per session: copy `EditorDataFilesLoaded.txt` from the game folder into the
session's F00 folder (if the CK writes it) and fill the version fields. Then give lane H the
session's folder names; `cargo xtask observe` does the rest.

#### M3.O2: MO2 and in-game session script

Disposable save only. Screenshots to `observe\<yyyy-mm-dd>-O2\`; answers into
`plan/STATUS.md` (lane D turns them into `docs/load-order.md` entries).

1. MO2: for `StarUI Inventory` and its Free Lanes patch, which mod MO2 shows providing
   `Interface\inventorymenu.swf` (the Data tab or the conflicts view).
2. In game, console: click an object from `Constellation.esm` (M3.H1 names one) and note
   its ID (expected `FE000xxx`); the same for an object from a full plugin.
3. `help <EditorID> 4` for the `HG_ShipCargoHoldAtHome.esm` forms and for one form each of
   `ShatteredSpace`, `SFBGS004`, `SFBGS00D`, `SFBGS050`, `BlueprintShips-SFBGS050` and
   `SFBGS047` (M3.H1 lists them): do they exist, and which runtime IDs (slots) do they show?
4. If M2.O6 is still open, do it now; then enable F13's plugin the way M2.O6 does and read
   the non-ASCII name in the inventory (correct, garbled, or blank).

### Ordering

```
Critical (overhaul): O0 -> B0 -> B1 -> B2 -> D4 -> E6 -> E7 -> E9 -> O5
                                  B1 -> B3 -> E6           O4 -> E9      M2.O6 -> O5
Critical (CK):       A1 -> H1 -> O1 -> H3 -> A3 -> E5       H2 -> H3

B:  B0 -> B1 -> B2, B3, B4, B5 -> B7      B0 -> B6      B0 -> B8 -> E4      B4 -> B9
    B10 (any time)      O3 + H4 + B1 -> B11      H3 -> B12
A:  A1      A2 -> F1      O1 -> A4      A5 (any time)
D:  D1 -> E1, E2      B1 -> D2      D3      B1 + B2 -> D4      O1 + O2 -> D5      D6
E:  E3      B1 -> E10      E8 (now) -> O4
F:  F2, F3, F4 (any time)      O6 -> F5
G:  G1, G2, G3 (any time)
H:  H1, H2 (now)      O1 -> H5      H4 (Spriggit now; SF1Edit after O3)
```

The overhaul path is the long pole: lane B's transcription of WEAP, AMMO, PROJ and AMDL
(B1..B3) gates the spec v2 work, and nothing in lane B starts before M3.O0. The CK path is
gated by the owner session M3.O1; lane H prepares everything up to it (H1, H2), and lane A's
recompression study (A1) supplies the F07 candidates. The hardening tasks (A2, A5, B10, D1,
D3, D6, E1..E3, F1..F3, G1..G3) are off both paths and fill idle time.

### M3 is done when

- Lane B round 3: WEAP, AMMO, PROJ, AMDL, OMOD, COBJ, ZOOM, LVLI, FLST, INNR and the KYWD
  and GMST remainders are typed, every member sourced and given a confidence; the M2.A3
  driver reports 0 conflicts on all 14 files; the 105 corpus lstring signatures are schema
  members (M3.B8); the staged xEdit files are deleted and that is recorded.
- `docs/ck-observations.md` has entries for every M3.O1 fixture the CK could produce; each
  M3.H3 first target is answered or marked not observable with the reason; the TES4
  `SNAM`/`BNAM`/`CHGL` question and the 160 recompression misses are settled by an entry or
  restated with the evidence gathered; every CK fixture roundtrips identical (M3.A4).
- `set` and `build` accept `style: "ck"`, and for each expressible CK edit fixture the
  output equals the CK's bytes or every difference is explained in the ledger (M3.E5).
- `docs/ck-parity.md` ranks the CK features seen (M3.H5); the viewport is still deferred.
- The gun overhaul: spec v2 and `kit overhaul` are `Implemented`; the first build from the
  owner's brief is validated (M3.E9); the owner's in-game result (M3.O5) is recorded in
  `plan/STATUS.md`, as a pass or as a failure with a follow-up task.
- `docs/oracles.md` has the Spriggit run on the fixture set and, if M3.O3 is done, the
  SF1Edit run and M3.B11's tables; otherwise the SF1Edit parts are carried to M4 with the
  reason.
- Every hardening task is done or carried with a reason; `docs/gaps.md` says what M3
  closed.
- ADR 0007 is committed and the owner's M3.O6 decision recorded (with M3.F5 done if the
  owner opted in).
- `cargo xtask ci` is green; the full corpus roundtrip is still identical (M1.6 release,
  `kit roundtrip --verify` and `--decode` on all 14 files); `cargo xtask parity`
  regenerated; `plan/STATUS.md` says M4.

## Not in M3

- The render viewport, world, cell and navmesh editing, NIF and materials (M4+).
- UI implementation, unless the owner opts in through M3.O6.
- Creating new records (new FormIDs) in our writer; F06 and F11 only observe how the CK
  allocates them.
- Writing localized plugins or string tables for a patch, and writing small/medium-flagged
  output plugins: observed in M3 (F11, F12, M3.O2), written in M4 at the earliest.
- In the overhaul: leveled-list injection, scripts, new records, and fields outside the
  typed weapon records.
- Sim Settlements 2 (rung 3): owner decision 5 is still "later", and rung 3 needs
  kinggath's consent before any of its assets or scripts are touched.
- Oracles beyond SF1Edit and Spriggit (Mutagen, esplugin, libloadorder, LOOT as programs).
- Changing the xEdit pin, another upstream file with a different license or source
  category, or any revisit of decided licensing.
- Papyrus assembling, editing or decompiling; a git remote; publishing anything, including
  through the CK's Creations features.
