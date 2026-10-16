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
| M0.5 | Corpus test helper: `crates/esm/tests/common/mod.rs` reads `KITKRAFT_DATA_DIR` and `corpus/manifest.toml`, checks sizes, skips cleanly when unset | A    | M0.3       | `cargo test -p kitkraft-esm` passes with and without the env var; skip prints one line                                 |

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
| M1.11 | `execute(id, args: serde_json::Value) -> Result<Value, EngineError>` and the result envelope (`data`, `limitations`, `coverage`, `evidence`). The words -> (`id`, `args`) line parser used by `command_line` lives here in `kitkraft-engine`, not in a front end | M0.3        | Unknown id and schema-invalid args return errors; envelope shape matches `docs/mcp.md`          |
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
| M2.D5  | `doc`: per-plugin index (M2.A1), override table (every version of every form, in load order) and winning override; on-disk index cache under `%LOCALAPPDATA%\kitkraft\cache` keyed by path, size, mtime and index format version (never in Data or the profile) | M2.D4, M2.A1 | Default profile cold and warm times recorded in `docs/load-order.md`; warm load <= 3 s release; a changed size or mtime invalidates the entry (test); override lists are in load order and the winner is the last |
| M2.D6  | `doc`: conflict graph: forms with >= 2 overriding plugins, each overrider classed `identical-to-previous`, `differs` (which subrecords; typed fields where typed) or `deleted` | M2.D5 | A synthetic fixture per class; a Default profile run reports class counts with evidence offsets |
| M2.D7  | `doc`: localized strings: for localized plugins load the string tables from the overlay (loose files) or the plugin's archives (M2.G1); resolve lstrings with table and offset as evidence; `language` argument, default `en` | M2.D2, M2.G1, M2.G8, M2.B2 | FULL of a `Starfield.esm` WEAP picked by EditorID from the corpus resolves to non-empty text with the table offset as evidence; a missing table yields the string ID plus a limitation, never "absent" |
| M2.D8  | `load` command (`{profile}` or `{plugins, data_dir}`, plus `language`): fills the session; result lists plugins with index, flags, masters, missing masters, cache hits | M2.F1, M2.D5 | Through `execute` with JSON; `kit load --profile ...` prints a summary; the MCP `kitkraft://load-order` resource returns the same list |
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
| M2.E10 | Rename smoke test: `docs/smoke-rename.md` and an automated test (skipped without `KITKRAFT_DATA_DIR`) renaming the Old Earth Hunting Rifle to "Old Soviet Marksman Rifle" in a new plugin (steps below) | M2.E5, M2.E7, M2.E9, M2.D9, M2.D11 | The test builds the plugin in a temp dir, `validate` passes, `inspect` with the plugin appended to the load order shows the new FULL winning, `roundtrip --verify` is identical; the doc covers the owner's MO2 steps; the M2.O6 result is recorded in `plan/STATUS.md` |

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
| M2.F4 | `backend.rs`: tool call -> `engine::execute` on the server's session; envelope returned as structured content and as JSON text; error mapping per the `docs/mcp.md` table; resources `kitkraft://commands`, `kitkraft://load-order` | M2.F1, M2.F3 | A test per error kind; `load` then `inspect` in one session |
| M2.F5 | `kit mcp [--profile P]` launcher (not a `CommandSpec`, per AGENTS.md) | M2.F4 | `kit mcp` no longer exits 2; `--profile` preloads through `load`; `docs/mcp.md` exit-code table updated |
| M2.F6 | Smoke tests: an in-process stdio client in `crates/mcp/tests` (initialize -> tools/list -> tools/call `roundtrip` and `coverage` on a synthetic plugin); `apps/kit-cli/tests` spawns the real binary (`CARGO_BIN_EXE_kit`) and does the same | M2.F5 | Both run in `cargo xtask ci` without game data; with `KITKRAFT_DATA_DIR` a variant also calls `inspect_record` and `query_records` on official files |
| M2.F7 | Claude Code: `.mcp.json.example` at the repo root (release `kit.exe`, `mcp --profile <Default>`) and "Use from Claude Code" in `docs/mcp.md`; verify in a Claude Code session (or headless `claude -p`) that the tools list and that `inspect_record`, `query_records`, `execute roundtrip` and `coverage` return envelopes | M2.F6, M2.D9, M2.D11 | Transcript excerpt in `plan/STATUS.md`; no live `.mcp.json` committed |
| M2.F8 | Catalog closure: `Status::Deferred { reason, milestone }`; a test fails if any command is still `Planned` at M2 exit; `kit help` and `tools/list` show deferred reasons | - | Test in `cargo xtask ci`; at M2 exit every command is `Implemented` or `Deferred` with a reason |
| M2.F9 | `cargo xtask parity` reports typed-decoded and failed counts (M2.A3) and per-confidence counts (M2.B1) | M2.A3, M2.B1 | Regenerated `docs/record-parity.md` is idempotent and `--check`ed in `ci` as before |

### Lane G: archives + scripts + strings (`crates/ba2`, `crates/pex`, `crates/strings`)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M2.G0 | ADR 0006: sources for BA2, `.pex` and string-table facts: our scripts (ours); `uesp-skyrim` pages as hypotheses; official BA2s and scripts read in place as corpus (never committed); Microsoft's DDS documentation for DDS headers; `Archive2.exe` and `PapyrusCompiler.exe` run as black boxes only (never disassembled). It adds no source ADR 0005 forbids; one that would goes to the owner | - | ADR committed; `ATTRIBUTION.md` unchanged (no third-party bytes) |
| M2.G1 | `ba2` GNRL v2/v3 reader (port of `ba2x.py`): header, file records, name table, per-file streaming extraction; the v3 header field `ba2x.py` skips is inventoried from the official archives, and every codec found is supported (an MIT/Apache crate where needed) | M2.G0 | Synthetic fixtures; every official GNRL archive under `KITKRAFT_DATA_DIR` lists without error (versions, counts and codecs in `docs/corpus.md`); 100 seeded files per archive extract to their declared sizes; truncated and bit-flipped inputs give typed errors |
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
| M3.H4 | Oracle harness `cargo xtask oracle {spriggit,sf1edit} <plugin>`: tool paths from `KITKRAFT_SPRIGGIT` / `KITKRAFT_SF1EDIT`, tool version recorded. Spriggit: ESM -> YAML -> ESM in a temp dir, then our `diff` and `coverage` compare input and output (the YAML is never parsed or read). SF1Edit: record counts per type, FormIDs and EditorIDs as SF1Edit shows them, and its own error check on our outputs (gate G3); the capture method is chosen and documented in `docs/oracles.md` (its own export or dump features run as a program; a script we write for its scripting engine is written from its user docs, never from xEdit source, and stays outside the repo) | M3.O3 (SF1Edit part only) | `docs/oracles.md`: per tool, the build, the fixture set (the three small corpus files, the rename plugin, M3.O1 fixtures, the overhaul build) and a disagreement table; each disagreement recorded as evidence (ours, theirs, version skew), never as a corruption claim without our own byte evidence |
| M3.H5 | Rank CK editor features in `docs/ck-parity.md` (the ROADMAP M3 bullet): extend the checklist with what M3.O1 shows (menus, dialogs, save and export options, warnings on load), map each to a command or none, P1/P2/P3; viewport and world editing stay P3, deferred | M3.O1 | Every checklist row has evidence (a ledger id or session note); P1 rows without a command are proposed to the owner as M4 candidates |

#### Fixture matrix (M3.H1; dossier section 4)

Each fixture is a folder `C:\Modding\kitkraft\observe\<yyyy-mm-dd>-<id>\` (gitignored) with
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
| M3.O1 | **owner** (or an agent with desktop control the owner assigns): CK observation session per the script below (was M2.O4, CK part); two sittings are fine (F00..F06 and F13, then F07..F12 and F14). **Deferred by the owner 2026-10-14** until the developer workbench exists (M4.W2..W5), so the owner can A/B the CK against KitKraft's inspection surface; an agent with computer use may then drive the session under the M3.O1 rules | M3.H3, M3.A3, M3.A4, M3.E5, M3.H5, M3.D5 (F00) |
| M3.O2 | **owner**: MO2 and in-game session per the script below (was M2.O4, MO2 and game parts) | M3.D5; the inline-encoding gaps row (with F13) |
| M3.O3 | **owner**: download and install SF1Edit and note its build (was M2.O3; agents do not download executables). **Done 2026-10-14**: xEdit installed at `C:\Modding\tools\xEdit\`, Starfield mode `xSFEdit64.exe` (file version 4.1.5.0; older than the `dev-4.1.6` pin, so M3.B11 records the skew) | M3.B11, M3.H4 (SF1Edit part) |
| M3.O4 | **owner**, decision 3: pick the YouTube critique and fill `docs/overhaul/brief.md` (M3.E8) in your own words | M3.E9 |
| M3.O5 | **owner**: in-game acceptance of the overhaul build on a disposable save (steps in `docs/overhaul/build-1.md`); record a pass, or a failure with what was seen | M3 exit |
| M3.O6 | **owner**: decide ADR 0007: build the UI shell in M3 (M3.F5), or defer it to M4. **Decided 2026-10-14**: ADR 0007 **accepted with scope** (Amendment 1): a minimal read-only developer workbench after M4's inspection APIs stabilize (M4 lane W), the full authoring UI in M5; M3.F5 is not built in M3 | M3.F5 (not built), M4 lane W |

Still open from M2 and unchanged: M2.F7 (Claude Code transcript), M2.O6 (in-game rename;
M3.O5 reuses its procedure and should follow a pass), owner decisions 1, 4, 5 (Sim
Settlements 2: still "later") and 6.

Update 2026-10-14: M2.F7 and M2.O6 **passed** (`plan/STATUS.md`); M2 is fully closed.
Owner decision 1 **decided**: the name is **KitKraft**; identifiers are renamed to
`kitkraft-*`, `KITKRAFT_*`, `kitkraft://` (binary stays `kit`) as the last integration
step after the in-flight lanes land; until then they read `kitkraft`.

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
   the CK wrote alongside, into `C:\Modding\kitkraft\observe\<yyyy-mm-dd>-<id>\`. Leave
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

*Update 2026-10-14:* the owner deferred M3.O1 until M4's developer workbench exists
(M4.W2..W5). The overhaul path does not need it. What waits: every `ck-diff` and
`verified` confidence upgrade (M3.B12), the `ck` write style (M3.A3, M3.E5), CK fixtures
as container tests (M3.A4), the findings and feature ranking (M3.H3, M3.H5), F00 for
M3.D5, and with them the TES4 `SNAM`/`BNAM`/`CHGL` and F07 recompression questions. M3's
CK exit criteria are carried until that session; the rest of M3 can close without them.

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

## M4: coverage program

Goal: KitKraft covers the record families the owner's mod needs, can create new forms and
place them in the world, and every workstream of that mod has a first deliverable that
proves its path end to end, in game where the claim is about the game. The headline proof
is **the first POI placed by KitKraft and visited in game on a disposable save** (critical
path below). Concretely: the remaining schema family lanes and a schema integrator; cell
and world child-group placement, reference creation and a new-form allocator in the
builder; `new`, `clone`, `place`, `package`, `testplan`, `planet` and `papyrus.events`;
validation rules for cells; research documents for persistence and for the planet content
system; and a scoping pass, not an implementation, for UI, assets and audio.

This section extends the scope line at the top of this file to M4. M3 is not closed when
M4 starts: M3 tasks keep their IDs, owners and dependencies, and no M4 task waits on the
gun overhaul (M3.E9, M3.O5) or on the `ck` write style (M3.A3, M3.E5). Three schema lanes
are **already running in parallel and are not re-planned here**: lane B round 3 (M3.B1..B8:
WEAP AMMO PROJ AMDL OMOD COBJ ZOOM LVLI FLST INNR and the 105 corpus lstring signatures) and
the schema family lanes WORLD and PLANET (below). M4 tasks name what they consume from
them.

Markings as in M3: **owner** tasks need the owner (or, for the CK only, a desktop-control
agent the owner assigns); *(research)* tasks establish a fact before code relies on it,
recorded with its ADR 0005 source form and, where a claim is weighed, its evidence grade
(ADR 0005 Amendment 3 §3); *(experiment)* tasks may fail, and a recorded failure is a
result.

### The owner's mod scope

The owner's twelve workstreams, verbatim, with the owner's priorities. W numbers are used
below.

| W   | Workstream                                                  | Priority      |
| --- | ----------------------------------------------------------- | ------------- |
| W1  | Binary authoring and FormID/reference resolution            | P0            |
| W2  | Worldspaces, cells, placed references, and POI instances    | P0            |
| W3  | Workshop registration and construction                      | P0            |
| W4  | Ownership, claiming, and restoration                        | P0            |
| W5  | Save persistence, resets, and procedural locations          | P0            |
| W6  | Papyrus compilation and runtime event handling              | P0            |
| W7  | Planetary traits and unique resource extraction             | P1            |
| W8  | Manufacturing, storage, and economic loops                  | P1            |
| W9  | NPCs, crew, navigation, and settlement behavior             | P1            |
| W10 | Quests, factions, dialogue, and progression                 | P1            |
| W11 | UI, maps, visual assets, and audio                          | P1-P2         |
| W12 | Packaging, compatibility, automated testing, and deployment | P0 throughout |

The owner's discovery binds W2, W5 and W7: Starfield's plugins carry Planet Content Manager
records (`PCMT`, `PCBN`, `PCCN`) and leveled pack-ins (`LVLP`) alongside `LCTN`, `WRLD`,
`CELL`, `REFR`, `NAVM` and `QUST`, so the authoring surface must understand the planetary
content system, not only hand-placed cells. In the corpus (`docs/record-parity.md`):
`PCMT` 5 records, `PCBN` 379, `PCCN` 2,148, `LVLP` 52, `PKIN` 15,128, `PNDT` 1,786, `BIOM`
434, `LCTN` 7,316, `WRLD` 583, `CELL` 40,251 (32,230 compressed), `REFR` 6,719,725, `ACHR`
14,731, `NAVM` 92,893 (92,764 compressed). What each of these records means, and how a POI
reaches a planet through them, is not stated in this plan: layouts come from the pinned
xEdit files through the family lanes, link structure from the corpus (M4.D3), and
behaviour from CK and in-game observation.

### Binding constraints (owner, ADR 0004 "Owner approval 2026-10-14")

The six clarifications bind every M4 task, as in M3. What is new in M4 is that KitKraft
writes **new** records:

1. **Common-file helpers** are cited only for what a Starfield definition uses them for
   (every family lane).
2. **Further upstream helper files** need only a documented dependency
   (`docs/xedit-reference.md`, `docs/source-registry.md`); no policy round unless the
   license or source category differs.
3. **Confidence.** Every member KitKraft writes into a new or composed record (`new`,
   `clone` edits, `place`) must be `corpus` or better, or the command refuses unless
   `allow_reference` names it (the M3.E6 rule; ADR 0005 Amendment 3 §4). Upstream coverage
   never raises confidence.
4. **Lossless first.** Placing a reference or creating a form changes only the new
   records, the group sizes up their chain, the groups created for them, and the `TES4`
   counters the corpus rules name (`HEDR` count and next object ID, `INCC`); every other
   byte is re-emitted as read.
5. **FormID resolution.** The allocator works on raw, file-relative FormIDs; resolved
   identities stay lane D's. A new form in a small or medium plugin is refused until a CK
   observation (F11, F26) shows how the CK stores it: never guessed.
6. **Scope discipline.** No architecture restart: no new crate in M4 (the `.psc` reader
   lives in `crates/pex`, the new commands in `crates/engine`), no layer change, no
   licensing revisit. No lane waits on speculative compatibility: a task gated on an
   owner session ships what it can from the corpus and lists the rest as a limitation.
   The UI is built in M4 only if the owner's M3.O6 decision says so.

### Lanes (ownership)

Lanes A, D, E, F, G and H keep their M3 files plus what is listed here. Lane B splits into
**schema family lanes**, one agent each, each owning the definition files of its record
types; BX, the schema integrator, is a role of lane F because it writes generated output.
One agent per lane, each in its own worktree and `CARGO_TARGET_DIR`.

| Lane | Name                           | Owns |
| ---- | ------------------------------ | ---- |
| A    | container                      | `crates/esm` (not `src/generated/`) |
| BR   | schema round 3 (running)       | M3.B1..B12's files (M3, lane B) |
| BW   | schema family WORLD (running)  | `defs/` files for `WRLD CELL REFR ACHR NAVM LCTN LCRT` |
| BP   | schema family PLANET (running) | `defs/` files for `PCMT PCBN PCCN PNDT STDT BIOM LVLP PKIN GBFM SUNP SFBK SFPT TRAV` and the `RS*` types |
| BQ   | schema family QUEST            | `defs/` files for `QUST DIAL INFO SCEN PACK FACT SMBN SMQN SMEN CHAL`; first owner of the shared script-attachment and condition blocks |
| BC   | schema family ACTOR            | `defs/` files for `NPC_ LVLN RACE OTFT PERK SPEL MGEF CLAS AFFE` |
| BI   | schema family ITEMS/WORKSHOP   | `defs/` files for `ARMO MISC CONT FURN ACTI STAT MSTT TERM TMLM IDLE ANIO CPTH`; first owner of the shared object-bounds and model blocks. `COBJ`, `FLST` and `KYWD` stay lane BR's (M3.B5, M3.B7, M3.B6) |
| BX   | schema integrator (lane F)     | `crates/schema/defs/common/` (deduplication only), the `BUILTIN_DEFINITIONS` region of `crates/schema/src/lib.rs`, `crates/esm/src/generated/`, `docs/record-parity.md`; inventory definitions for record types no family owns (M4.BX2) |
| D    | doc + io                       | as M3, plus engine `handlers/planet.rs`, `docs/design/planet-content.md` |
| E    | set + validate + build         | as M3, plus engine `handlers/{new,clone,place,package,testplan}.rs`, `docs/recipes/`, `docs/packaging.md` |
| F    | mcp + surface                  | as M3, plus the BX role |
| G    | archives + scripts + strings   | as M3, plus `crates/pex/src/psc/`, `docs/research/asset-formats.md`, the ADR 0006 amendment |
| T    | in-game verification (proposed) | `crates/game` (new, L3), engine `handlers/game/`, the `probes.toml` half of `handlers/testplan.rs`, `docs/game-sessions.md`, `observe/` layout, `crates/game/profiles/` (added 2026-10-15, ADR 0010; starts after M4.O6 and M4.O7) |
| H    | evidence                       | as M3, plus `docs/design/persistence.md`, `docs/game-observations.md` |
| W    | developer workbench (planned)  | `crates/ui-egui`, `apps/kit`, the `kit ui` launcher lines in `apps/kit-cli`, the dependency list in ADR 0007 (added 2026-10-14; starts after M4.W0) |
| O    | owner                          | decisions, installs, CK, MO2 and game sessions, reviews |

**Family lane rules** (BQ, BC and BI here; BW and BP run under the same rules):

- Acceptance per record type is lane B's (M3.B1): every member has `source` (`xedit:` at
  the pin with a line range) and `confidence`; through the M2.A3 driver every corpus
  occurrence decodes with an empty `extra` and re-encodes identical on all 14 files, or
  the member is demoted to opaque with the conflict noted; members confirmed on every
  occurrence are `corpus` with a `corpus:` citation naming the test; union-decided members
  stay opaque until M3.B9 can express their decider; lstring members agree with the M3.B8
  corpus table; names from the CK wiki or the CK's UI labels, never an upstream
  placeholder; no Pascal code or comments copied (the reviewer checks the diff); the
  staged xEdit files are verified as in M3.B0 at the start of each session.
- **Generated output is not committed by family lanes**: not `crates/esm/src/generated/`,
  not the `BUILTIN_DEFINITIONS` region, not `docs/record-parity.md`. A family lane runs
  `cargo xtask codegen` in its worktree so its gate passes there, and restores those files
  before committing. Six lanes regenerating the same files would conflict on every merge;
  BX regenerates once per merge (M4.BX1). This is a stated deviation from AGENTS.md
  "Commits" for family lanes only: a family commit is gate-clean together with BX's merge.
- **Shared blocks**: a family lane may add a new file under `defs/common/` but never edits
  an existing one; it references the existing block instead. The first owner named in the
  table defines a shared block; the others use it once merged or keep the member opaque.
  Where two families defined the same block independently, BX merges them (M4.BX1).
- Large types record their full-corpus typed-check time in `docs/corpus.md` (`REFR` 6.7 M,
  `INFO` 162 k, `DIAL` 87 k, `NAVM` 93 k, nearly all compressed; every `NPC_` compressed).

### Format facts M4 relies on, and where they come from

Hypotheses until the named check confirms them. Nothing here is a Starfield record layout.

- Group types 0..9 and their names are the container facts in
  `crates/schema/defs/GRUP.toml` (`uesp-skyrim`, corpus-confirmed walk); type 10 is
  Starfield-only and unnamed (`docs/corpus.md`). The M2.E3 `ONAM` inventory lists records
  of group types 7 (INFO), 9 (REFR, ACHR, NAVM) and 10 (DIAL, SCEN), so type 10 holds
  dialogue and scenes at least in those files; M4.A1 inventories it fully.
- How interior cells are spread over blocks and sub-blocks, and how exterior cells are
  grouped by grid, is described on UESP's Skyrim page (level 1, container): a hypothesis
  until M4.A1 tests it on every cell of the 14 files.
- A plugin that adds children to a master's cell carries that cell (and its world) as an
  override, so the children group has a parent (M2.A5 notes; UESP "Groups"). Whether the
  CK writes the parent as an unchanged copy is F21's question.
- `ONAM` lists some overridden cell and topic children; what decides membership is open
  (M2.E3 "Writing rules"). New records are not overrides, so placing new references does
  not depend on it; editing vanilla references does (M4.A6).
- `INCC` equals the number of interior cells in all 14 files (`docs/corpus.md` "Writing
  rules"); a plugin that adds an interior cell updates it.
- `HEDR` next object ID is `0xFF001000` in 12 of 14 files; what the CK allocates for a new
  form is F06's question (M3.O1) and, for small and medium exports, F11's and F26's.
- The 5,034 `Scripts/Source` files in `Tools\ContentResources.zip` are read in place
  (M3.G1); they are Bethesda data and are never committed, quoted in docs or bundled.
- Console batch files (a text file of console commands the game runs on request) and the
  folder the game reads them from: grade D, confirmed or refuted in the first in-game
  session (M4.O2). KitKraft never writes into the game folder.
- MO2 installs a mod as a folder under `mods\` whose contents overlay `Data` (M2.D1,
  `docs/load-order.md` "Overlay").

### Workstream-to-capability matrix

For each workstream: the record types it needs (lane in brackets), what KitKraft provides
or will provide (schema, builder, validate, commands), what it depends on outside
KitKraft, what KitKraft **cannot** do, and the first deliverable that proves the path.
"Cannot" holds for all of M4 unless a task says otherwise. SFSE and CommonLibSF appear
only where a runtime hook might be needed; nothing in M4 depends on them (M4.O3).
*Amended 2026-10-14 (M4.O3 decided, ADR 0008):* runtime extensions are supported where
technically justified; no M4 task builds one, and a deliverable whose data path proves
insufficient writes a runtime-need record (ADR 0008 §5) and is carried, not dropped.

| W | Record types | KitKraft capabilities | External dependencies; what KitKraft cannot do | First deliverable (task) |
| - | ------------ | --------------------- | ---------------------------------------------- | ------------------------ |
| W1 | `TES4` (`HEDR` next object ID, masters, `ONAM`, `INCC`); the FormID members of every family | Schema: FormID members typed in every family, each checked by `formid.dangling` on the official files at merge (M4.BX1). Builder: new-form allocator (M4.A3), computed placement paths (M4.A2), `ONAM` for overridden children (M4.A6). Commands: `new`, `clone` (M4.E1), `refs` across families (M4.D1); `inspect`, `diff`, `conflicts` exist | CK fixtures F06, F11 (M3.O1) and F26 (M4.O1). Cannot: write a new form into a small or medium plugin before F11/F26 evidence (refused, never guessed) | M4.E1: a new `GLOB` and a clone of a vanilla `STAT` in a fresh plugin; FormIDs and `HEDR` next object ID equal the CK's F06 choice or the difference is a ledger entry; `validate` 0 errors |
| W2 | `WRLD CELL REFR ACHR NAVM LCTN LCRT` (BW); `PKIN LVLP` (BP); `LAYR`, `RFGP` (inventory, M4.BX2) | Schema: BW, BP. Builder: cell and world group structure (M4.A1, M4.A2), reference composition (M4.A5), NAVM passthrough (M4.A4). Validate: cell rules (M4.E4). Commands: `place` (M4.E2), build spec v3 (M4.E3), placement context in `inspect`/`query` (M4.D2), placed instances in `refs` (M4.D1) | CK for navmesh: **KitKraft cannot generate `NAVM`**; the owner generates it in the CK for our cell (F22) and `build` imports it unchanged (M4.A4). Cannot: edit terrain or preview a placement (no viewport): positions are numbers, taken from existing references or read in game by the owner | M4.E2 + M4.O2: the **first POI** (below) |
| W3 | `COBJ FLST KYWD` (BR); `STAT MSTT ACTI FURN CONT TERM` (BI); `WKMF` (inventory, M4.BX2) | Schema: BR, BI. Commands: `clone`, spec v3 `[[clone]]` and `[[new]]`. What registers an object in the outpost build menu is found from the vanilla records with `query` and `refs` (M4.E7, a recipe with its corpus evidence) | Cannot: create meshes, collision or snap points (they live in model files; clones reuse vanilla models); change outpost logic in the game's scripts (read through `papyrus.events`, never changed). SFSE only if M4.E7 finds the build menu cannot be extended by data (then a runtime-need record, ADR 0008 §5; M4.O3 decided) | M4.E7 + M4.O4: a cloned vanilla outpost object with its own recipe appears in the outpost build menu and builds on a disposable save |
| W4 | Ownership members of `CELL`, `REFR`, `LCTN` (BW); `FACT` (BQ); `NPC_` (BC); `QUST` and scripts for claim flows | Schema: whatever ownership members the pinned file defines, typed by BW. Builder: `ONAM` for edited vanilla references (M4.A6). Commands: `set` on a cell override, `query` by owner (M4.E8). Validate: owners resolve (`formid.dangling`) | Claiming and restoration are script and quest behaviour, observable only in game. Cannot: read runtime ownership from a save (KitKraft does not read saves) | M4.E8 + M4.O4: an override changes the owner of a vanilla interior cell (M4.H1 picks it) and the owner confirms in game that taking its items is no longer stealing; `query` lists cells by owning faction |
| W5 | Reference persistence (group types 8 and 9, BW); `LCTN LCRT` (BW); quest aliases (BQ); `PCMT PCBN PCCN LVLP PKIN` (BP) | Research: `docs/design/persistence.md` (M4.H3), `docs/design/planet-content.md` (M4.D3). Validate: `ref.persistence` (M4.E4, warning). Commands: `testplan` leave, save, reload and return steps (M4.E6); `planet` (M4.D3). Experiment: procedural placement (M4.E10) | Resets and procedural generation are runtime behaviour, known only from in-game observation (`docs/game-observations.md`, M4.H4) and level 4 sources. Cannot: read or write save files, or predict a reset. Plugins are never removed from a save that is not disposable. If M4.H3 and the testplan show state that vanilla persistence cannot keep, a save persistence hook is a runtime-need record (ADR 0008 §5, M5 at the earliest) | M4.H3 skeleton + M4.O2: the POI's persistence steps recorded in the game ledger |
| W6 | The script-attachment block on every scripted record (BQ, shared); quest fragments (BQ) | Commands: `papyrus.compile` (M2.G7, M3.G1), `papyrus.disasm` (M2.G6), `papyrus.events` (M4.G2), script attachment on `place`, `set` and `new` (M4.E9). Validate: `papyrus.missing-script`, `papyrus.property-unresolved` (M4.E9), `papyrus.denylisted-call` (exists) | Bethesda's `PapyrusCompiler.exe`, run as a black box, never `-release`. SFSE only if an event or function a deliverable needs is absent from the base sources: then a runtime event handler is a runtime-need record (ADR 0008 §5; M4.O3 decided); CommonLibSF is not read or linked until ADR 0008 §3's license review. Cannot: decompile, assemble or edit `.pex`; debug scripts at runtime; compile without Bethesda's compiler | M4.G2, then M4.E9 + M4.O4: a script compiled by KitKraft, attached to the POI's reference, runs on an event chosen from `papyrus.events` output and shows a message in game |
| W7 | `PNDT STDT BIOM PCMT PCBN PCCN SUNP` and `RS*` (BP); `IRES` (inventory, M4.BX2); extractor objects (BI) | Schema: BP. Commands: `planet` (M4.D3), `query` over planet and biome members | Cannot: change procedural terrain or biome generation outside records (any such data in archives is inventoried in M4.G3, not read) | M4.D3: `kit planet <PNDT>` lists one planet's biomes and resources with evidence; `docs/design/planet-content.md` |
| W8 | `COBJ FLST LVLI OMOD` (BR); `MISC CONT TERM` (BI); `IRES` (M4.BX2); `GMST GLOB` (typed); vendor data on `FACT` (BQ) | Spec v2 rules (exist) and v3 new records (M4.E3); `formid.dangling` on recipe components | Cannot: change economy behaviour the records do not express | M4.E7 + M4.O4: a new fabricator recipe for an existing item builds in game |
| W9 | `NPC_ LVLN RACE OTFT CLAS AFFE PERK SPEL MGEF` (BC); `PACK SMBN SMQN` (BQ); `ACHR NAVM` (BW) | Schema: BC. Commands: `place` an `ACHR`, `clone` an `NPC_`. Validate: `cell.actor-without-navmesh` (M4.E4). Builder: NAVM passthrough (M4.A4) | CK for navmesh (F22) and for face generation of new appearances. Cannot: generate navmesh, face data, voice, lip sync or animation | M4.E8 + M4.O4: a vanilla NPC placed into a vanilla cell that has navmesh stands and moves there in game |
| W10 | `QUST DIAL INFO SCEN PACK FACT SMBN SMQN SMEN CHAL` (BQ); `MESG` (M4.BX2) | Schema: BQ (stages, objectives and aliases only where the pinned file defines them and the corpus confirms). Builder: topic and type-10 children (M4.A1, M4.A2). Commands: `refs` from quests to references; fragments through M4.E9 | Cannot: create voiced dialogue (audio) or lip sync. A new quest is in reach only once aliases and stages type at `corpus` confidence | M4.BQ1 + M4.D1: `inspect` of a vanilla quest shows stages, objectives and aliases typed, and `refs` resolves their targets. A new quest with an alias on the POI's reference is a stretch, carried with the reason if aliases stay opaque |
| W11 | `LSCR MESG SOUN WWED ARTO TXST` and map-marker data (inventory, or BW); asset files in the archives | `ba2.list`, `ba2.extract`, `ba2.pack` (exist); `crates/nif` stays read-only metadata with no M4 work. **Scoping only** (M4.G3) | Cannot: create or edit meshes (`.nif`), materials (`.mat`), Wwise audio (`.wem`, `.bnk`) or interface files; textures only as DDS the user supplies, packed; no viewport | M4.G3: `docs/research/asset-formats.md` |
| W12 | `TES4`; everything | `package` (M4.E5, target-independent), `testplan` (M4.E6), archive naming (M4.G4), `validate`, `conflicts`, `roundtrip --verify`, the game ledger (M4.H4), `cargo xtask ci`; later `kit deploy --dev` into one isolated KitKraft-managed MO2 mod (M4.E11, ADR 0009) | MO2 (the owner installs the package; `deploy --dev` never touches profiles or other mods), `Archive2.exe` (black-box check), the game (the owner). Cannot: launch or drive the game; publish anything (owner decision 6; no Creations upload) | M4.E5 + M4.O2: the rename plugin (M2.O6) packaged and installed by the owner through MO2 from the package, with the hashes on disk equal to the manifest; then the POI package |

### Schema family lanes (lane B)

Running, not re-planned: **BR** (lane B round 3, M3.B1..B12), **BW** (WORLD) and **BP**
(PLANET). M4 tasks depend on them by name ("BW: `CELL` typed"). The tasks below start now,
in parallel, each in its own worktree; the staged xEdit files must be on disk (M3.O0).

| ID     | Task | Depends on | Acceptance |
| ------ | ---- | ---------- | ---------- |
| M4.BQ1 | QUEST family typed, in this order: `FACT`; `QUST`, with the script-attachment block and the condition block as new shared files (conditions cited at xEdit, never `/CTDA`); `PACK`; `DIAL`; `INFO`; `SCEN`; `SMBN`, `SMQN`, `SMEN`; `CHAL`. Inventory definitions first where a type has none | M3.O0 | Family lane rules, per type; the group type 7 and 10 contents agree with M4.A1's inventory; the script-attachment and condition blocks decode on every type that carries them in the corpus, or the member stays opaque on that type with the reason |
| M4.BC1 | ACTOR family typed, in this order: `NPC_`, `LVLN` (entry counts kept consistent per M3.B10), `OTFT`, `CLAS`, `AFFE`, `RACE`, `PERK`, `SPEL`, `MGEF` | M3.O0; M4.BQ1's shared blocks for members that use them | Family lane rules, per type; every `NPC_` (all compressed) is decoded for the check |
| M4.BI1 | ITEMS/WORKSHOP family typed, in this order: `STAT`, `MISC`, `CONT`, `ACTI`, `FURN`, `MSTT`, `TERM`, `TMLM`, `ARMO`, `IDLE`, `ANIO`, `CPTH`, with the object-bounds and model blocks as new shared files. `COBJ`, `FLST` and `KYWD` members that round 3 leaves opaque are added only when M4.E7 needs them, after BR's merge, as a Request to BR | M3.O0 | Family lane rules, per type |
| M4.BX1 | Schema integrator (lane F), **after each family merge** (BR, BW, BP, BQ, BC, BI, in the order they land): merge the family branch; resolve the `BUILTIN_DEFINITIONS` region; `cargo xtask codegen`; merge `defs/common/` duplicates (two blocks with the same layout become one, with both citations and every reference updated); `cargo xtask parity`; the M2.A3 typed check on all 14 files; `validate_corpus_official_files` | each family merge | `codegen --check` and `parity --check` clean; a structural duplicate check in `crates/schema/tests/defs.rs` fails on two `common/` blocks with the same layout; 0 typed conflicts on all 14 files; every new `formid.dangling` **error** on the official files is treated as a layout error in the merged family (member demoted, or the error explained) before the merge commit; per-family counts (typed records, typed subrecords, per confidence) and times in `plan/STATUS.md` |
| M4.BX2 | Inventory definitions (corpus order and cardinality, every payload opaque) for the record types the matrix names that no family owns: `DOOR`, `LIGH`, `IRES`, `MESG`, `LAYR`, `RFGP`, `PGRE`, `PHZD`, `WKMF`, `LSCR`, `SOUN`, `WWED`, `ARTO`, `TXST`. A type moves to a family lane when a first deliverable needs typed members | - | `coverage` names their subrecords; corpus re-encode identical; each type's later owner recorded in `plan/STATUS.md` when one is assigned |

### Lane A: container (`crates/esm`)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M4.A1 | *(research)* Cell and world group structure from the corpus: per group type 1..10, the parent group type, the label kind, the record types held and their counts; ordering rules (a record, then its children group; persistent and temporary children; block and sub-block order); the interior block and sub-block rule (UESP's Skyrim hypothesis) tested on every interior cell; the exterior grid rule tested on every exterior cell; where a world's persistent references live; type 10's parents and contents; the cell size implied by exterior references' positions against their cells' grid coordinates | interior part: -; exterior part and cell size: BW (`CELL` grid and `REFR` position typed) | `docs/corpus.md` "Cell and world groups": every rule stated with its counts, exceptions listed by file and FormID; fast tests on the small corpus files that contain cells, full runs `#[ignore]`; nothing assumed that a corpus test does not check |
| M4.A2 | Computed placement paths: from a cell (interior, or world plus grid) compute the path to the cell and to its persistent or temporary children group; `Plugin::upsert` and the streaming upsert create missing blocks, sub-blocks, world children and cell children in corpus order; `INCC` and the `HEDR` count follow the corpus rules | M4.A1; BW (`CELL` typed) | **Rebuild gate**: for every small corpus file with cells, and a seeded 1,000-cell sample of `Starfield.esm` with all their children, an empty plugin with the same `TES4` into which every record is upserted through computed paths (never copied ones) has the original group structure byte for byte, or each difference is listed and explained |
| M4.A3 | New-form allocator: object IDs above both the `HEDR` next object ID and the plugin's highest own object ID; `HEDR` next object ID advanced; own FormIDs file-relative (high byte = master count) for full-scale plugins; small and medium plugins refused until F11/F26 show the CK's form (then lane D's documented `FE`/`FD` storage); exhaustion per scale is an error | - (F06, F11, F26 raise it from corpus defaults) | Unit tests per rule; deterministic; once F06 is observed, the CK's FormID and next object ID for a new `GLOB` are reproduced or the difference is a ledger entry; until then every corpus-default choice is named in `limitations` |
| M4.A4 | NAVM passthrough: import `NAVM` records (and the groups they sit in) from a CK-saved plugin into a KitKraft-built one with identical bytes, compressed bodies untouched; refused when the two master lists are not prefix-compatible (FormIDs inside navmesh payloads are opaque and cannot be remapped) | M4.A2; M4.O1 (F22) for the fixture part | A synthetic test, and a test on F22 that skips with one line without `observe/`; imported records equal the CK's byte for byte; a master-order mismatch is refused with the reason |
| M4.A5 | Reference composition: `RefSpec {base, position, rotation, persistent, initially disabled, extra typed members}` -> a `REFR` (or an `ACHR` for an `NPC_` base) through BW's typed definition; persistence expressed however M4.A1 shows the corpus decides it (record flag, group, or both), consistently | BW (`REFR`, `ACHR` typed); M4.A3 | Composing a copy of a corpus reference from its decoded values reproduces its bytes, for a seeded sample of 1,000 references; units and ranges checked against the corpus (and F20/F21 when observed); a member below `corpus` confidence is refused unless allowed (clarification 3) |
| M4.A6 | *(research, then writer)* `ONAM` for overridden cell and topic children: the membership rule from the corpus inventory and F23; the writer lists overridden children per the rule | M4.O1 (F23) | Rule in `docs/corpus.md` "Writing rules" with its evidence; editing a vanilla reference writes `ONAM` per the rule, or is refused with the reason while the rule is open |

### Lane D: doc + io (`crates/doc`, `crates/io`)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M4.D1 | `refs` across the families (extends M3.D4): a reverse index over every typed FormID member of every merged family plus each reference's base, cached like the record index (M2.D5); placed instances (`REFR`, `ACHR`, with world and cell) grouped apart from other referrers; opaque payloads not scanned counted in `limitations` | M3.D4; BW; M4.BX1 | On the official load order `refs` of a vanilla `STAT` lists its placed references with cell and world; cold and warm times in `docs/load-order.md` (warm <= 10 s release on the Default profile); each family that merges adds its members with no change in lane D |
| M4.D2 | Placement context: `inspect` and `query` results for `CELL`, `REFR`, `ACHR` and `NAVM` give world, cell, grid, block and sub-block, and persistent or temporary, from the group path; `where` gains `@world`, `@cell`, `@persistent` | M4.A1 | A test through `execute` per field; on the official load order a known exterior reference reports its world and grid; `docs/mcp.md` updated |
| M4.D3 | `planet` command (read-only, alias `pcm`) and `docs/design/planet-content.md` *(research)*: from a `PNDT`, `STDT` or `PCMT`, the graph its typed FormID members reach (the Planet Content Manager tree, branch and content nodes and the records they name; planet to biomes and resources), conditions and opaque members shown as such; the doc gives corpus link statistics (which record types each member names, how often, how deep), what is known with its source, and the open questions for CK and in-game observation | BP; M4.D1 | Through `execute` with JSON; reachable over MCP; on the official load order every `PCMT` tree is walked with no unresolved link, or each is listed; the doc claims no behaviour without a source and a grade |
| M4.D4 | Runtime IDs and console names for new forms: the runtime ID of each form of a plugin appended to a profile (assumption `runtime-slots-by-scale` named), and the EditorID a console command can use | - | A test with a synthetic profile; `testplan` (M4.E6) uses it; the assumption is in the result's `limitations` |

### Lane E: set + validate + build (`crates/engine`)

| ID     | Task | Depends on | Acceptance |
| ------ | ---- | ---------- | ---------- |
| M4.E1  | `new` (`{type, edid, fields, into}`) and `clone` (`{form, edid, fields?, into}`): a new base form of a typed type, or a copy of a winning version with a new FormID (M4.A3) and EditorID; lstrings inlined (patches stay non-localized); FormID members copied as stored and checked by `formid.dangling`; refused for a source whose opaque members may hold FormIDs when its masters differ from the target's | M4.A3; the type's family merged (M4.BX1) | Through `execute` and the CLI; the W1 deliverable; an EditorID clash is an error; the new form is found by `inspect`, and by `refs` from its source |
| M4.E2  | `place` `{base, cell \| {world, position}, position, rotation, persistent?, edid?, into}`: a new `REFR` (or `ACHR`) composed by M4.A5 and placed by M4.A2; the parent `CELL` (and `WRLD`) carried as an unchanged copy of the winning version; for `{world, position}` the cell comes from the M4.A1 grid rule | M4.A2, M4.A5; M4.E4 for its acceptance | Places a reference into a vanilla interior cell and into a vanilla exterior cell of a fixed (not procedural) worldspace, each in a new plugin; `inspect` with the plugin appended shows the reference in its cell with M4.D2's context; `validate` passes; `roundtrip --verify` identical; the carried parents equal their winners byte for byte |
| M4.E3  | `build` spec v3: `[[new]]`, `[[clone]]`, `[[place]]`, `[[import]]` (NAVM passthrough from a CK plugin, M4.A4), with spec-local names so one entry can name a form another creates (`base = "new:poi_marker"`); v1 and v2 specs build as before | M4.E1, M4.E2; M4.A4 for `[[import]]` | Two builds of one spec are byte-identical; a test per entry kind and per error (unknown name, cycle, two writes to one field, a `reference`-only member without `allow_reference`); `plan` lists the new forms with their FormIDs |
| M4.E4  | Validate cell rules: `cell.missing-parent` (children whose `CELL` or `WRLD` neither the plugin nor its masters have: **error**); `ref.orphan` (a reference whose base does not resolve: **error**); `ref.outside-block` (a cell in the wrong block or sub-block: **error**; an exterior reference outside its cell's grid beyond the corpus tolerance: **warning**); `ref.persistence` (a reference another record names, such as an alias, a script property, a door link or a location, that sits in a temporary group: **warning**, grade D until observed); `cell.actor-without-navmesh` (**warning**); `cell.incc` (`INCC` against the interior cell count: **error**) | M4.A1; BW; M4.D1 for `ref.persistence` only (the other rules, which M4.E2's acceptance needs, do not wait for it) | A positive and a negative synthetic fixture per rule; `validate_corpus_official_files`: 0 errors, every warning kind listed with its reason in `plan/STATUS.md`; `docs/validate.md` rows with their sources |
| M4.E5  | `package` `{plugins, name, version?, out, ba2?, loose?, scripts?, zip?}`: an MO2-ready mod folder (plugins at the root, loose files and scripts at their Data paths, an optional archive through lane G's packer named per M4.G4), `kitkraft-manifest.json` (SHA-256 and size of every file, masters, the spec's hash, KitKraft version, the `validate` result), an optional `.zip` of the folder; refuses an `out` under the game Data dir, the MO2 profile, or MO2's `mods\` (M4.O5) | M4.G4 (archive name) | Deterministic (two runs, identical bytes); `Archive2.exe` opens the archive and lists the same files (recorded once); the manifest verifies after a copy; `docs/packaging.md` has the owner's install steps; the W12 deliverable |
| M4.E6  | `testplan` `{plugin \| spec, profile, out}`: console batch files (go to each new reference, add each new item, `help` on each EditorID) with M4.D4's runtime IDs, and `checklist.md` for a disposable save: setup, steps, the expected observation per step, persistence steps (leave, save, reload, return), and how to report into `docs/game-observations.md` | M4.D4; M4.H4 (ledger format) | Deterministic; the batch-file format and folder are marked grade D until M4.O2 confirms them; nothing written outside `out` |
| M4.E7  | Recipes for W3 and W8 (`docs/recipes/workshop.md`, `docs/recipes/fabricator.md`): from vanilla outpost objects and recipes, what registers a buildable object and a fabricator recipe (`query` and `refs` evidence; no meaning claimed without it), each as a v3 spec with its testplan | BI; BR (`COBJ`, `FLST`, `KYWD`); M4.E3 | Each spec builds, validates and roundtrips; every step cites its evidence; feeds M4.O4 |
| M4.E8  | Recipes for W4 and W9 (`docs/recipes/ownership.md`, `docs/recipes/actors.md`): a cell owner override; an `ACHR` of a vanilla NPC in a vanilla cell with navmesh; `query` by owner | BW; BQ (`FACT`); BC; M4.E2 | As M4.E7 |
| M4.E9  | Script attachment: `place`, `set` and `new` take `scripts = [{name, properties}]`, written through BQ's typed script-attachment block, property values as formrefs or spec names; rules `papyrus.missing-script` (no `.pex` for an attached script in the plugin's archives, loose files or the load order's archives: **error**) and `papyrus.property-unresolved` (**error**); recipe `docs/recipes/scripts.md` with a script compiled by `papyrus.compile` | BQ (script-attachment block at `corpus`); M4.G2; M4.E2 | Through `execute`; a synthetic fixture per rule; attaching a script to a corpus copy and reading it back gives the same block; feeds M4.O4 |
| M4.E10 | *(experiment)* Procedural placement: a test plugin that attaches the POI's pack-in to the planet content system the way M4.D3 shows vanilla content attached, with a testplan for a fresh landing | M4.D3; M4.E3 | A result recorded in `docs/game-observations.md` from M4.O4, either way; on no critical path; carried to M5 with the evidence if M4.D3 finds no data path |
| M4.E11 | *(later; added 2026-10-14, ADR 0009)* `deploy` (CLI `kit deploy --dev`), `deploy.rollback`, `deploy.status`: a `package` folder into `<mo2>\mods\KitKraft Dev - <plugin>\` only, `dev: true` and `mo2` required on every call; path validation (MO2 instance check in `crates/io`, a Request to lane D), ownership marker and `kitkraft-deploy.json` with hashes, per-file temp + rename, backups and rollback, journal recovery, diagnostics; never touches profiles, `plugins.txt`, `loadorder.txt`, `modlist.txt`, `overwrite\` or any other mod | M4.E5 | Tests on a synthetic MO2 instance in a temp dir only: first deploy, redeploy, rollback, an interrupted deploy recovered, a same-named unmarked folder refused, a junction in the path refused, a hand-edited deployed file reported and not overwritten, profile files byte-identical before and after; through `execute` and the CLI; `destructiveHint` over MCP; on no critical path |

#### First POI (critical-path deliverable)

1. `kit load --profile "C:\Modding\MO2-Starfield\profiles\Default"`.
2. M4.H1 names the target with its reasons: an exterior cell of a fixed (hand-authored)
   worldspace a disposable save can reach, a vanilla pack-in (`PKIN`) or `STAT` base, and a
   position near an existing reference. Rung 0 uses a vanilla interior cell instead.
3. `kit build poi.toml --out <staging>\KC_FirstPOI.esm`: one `[[place]]` of a persistent
   reference with an EditorID (spec v3). A pack-in is placed by its FormID, so `PKIN` needs
   no typed layout for this step: BP is off the critical path.
4. `kit validate` (cell rules included); `kit inspect` of the new reference with the plugin
   appended (world, cell, grid); `kit roundtrip --verify`.
5. `kit package` -> MO2-ready folder and manifest; `kit testplan` -> batch files and the
   disposable-save checklist.
6. Owner (M4.O2): install through MO2, load a disposable save, go to the reference (batch
   file or console), screenshot it, leave the area, save, reload, return, and record each
   observation in `docs/game-observations.md`. The agent prepares everything up to this
   step; it cannot launch the game.

### Lane F: mcp + surface (`crates/mcp`, `apps/kit-cli`, `xtask`, engine core)

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M4.F1 | Surface for the M4 commands (`new`, `clone`, `place`, `package`, `testplan`, `planet`, `papyrus.events`): the `docs/mcp.md` command list, `destructiveHint` on every command that writes, `kitkraft://commands` and tool descriptions; no new MCP tool unless a command cannot be reached through `execute` and `command_line` | each command as it lands | The catalog closure test passes; every new command is reachable over MCP (a test per command through `execute`) |
| M4.F2 | The BX role (M4.BX1, M4.BX2) | - | As M4.BX1 and M4.BX2 |
| M4.F3 | UI shell: M3.F5 runs in M4 only if the owner's M3.O6 decision defers it to M4 and the owner confirms; scope as ADR 0007 | M3.O6 | As M3.F5 |

### Lane G: archives + scripts + strings

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M4.G0 | ADR 0006 amendment: `.psc` declaration facts come from the official sources in `ContentResources.zip`, read in place (level 1, corpus), and from the compiler's black-box behaviour (level 2); `ck-wiki:` (level 4) for language semantics, in our own words; the Papyrus Index site (`docs/source-registry.md`, PAPYRUS) only as a cross-check of signatures, its registry status updated; no other parser or compiler is read | - | Amendment committed; registry row updated |
| M4.G1 | `.psc` declaration reader in `crates/pex` (`psc` module): script header (name, parent, flags), functions and events (parameters with types and defaults, return type, native and global flags), properties, structs, custom events; bodies skipped; what it does not understand is reported, never a panic | M4.G0 | All 5,034 sources read from the zip without an error, or each failure listed with file and line; synthetic fixtures per construct; hostile input gives typed errors; no source text in the repo |
| M4.G2 | `papyrus.events` `{sources_zip?, class?, name?, kind = event \| function \| all, inherited = true}`: signatures with script, line, flags and the parent chain resolved; source comments only with `with_comments: true`, read from the user's install | M4.G1 | Through `execute`; counts per kind in `plan/STATUS.md`; a test that the events `docs/design/persistence.md` cites exist on their scripts (names taken from the command's output, not from memory); reachable over MCP |
| M4.G3 | *(scoping)* Asset format inventory: from `ba2.list` over the 90 official archives, counts and total sizes per extension and per archive (`.nif`, `.mat`, `.dds`, `.wem`, `.bnk`, interface files, and every other extension found); per format, what KitKraft does today (list, extract, pack), whether a permitted source describes it (ADR 0006), and which workstream would need to author it | - | `docs/research/asset-formats.md`; no implementation and no format reading beyond `ba2.list`; W11's deliverable |
| M4.G4 | *(research)* Archive names a plugin loads: the naming pattern of the official archives against their plugins (corpus) and the suffixes a mod's archives may use; the in-game check is an M4.O2 step | - | Rule in `docs/corpus.md` "Archives" with counts; `package` (M4.E5) names archives by it |

### Lane H: evidence

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M4.H1 | M4 fixture matrix (below) and session scripts, appended to `docs/ck-observations.md` (created by M3.H1; not on `main` at 438b7a4): every placeholder resolved with FormID, EditorID and the reason (cells, bases, the ownership cell, the POI target and the rung 0 cell) | M3.H1 | Every placeholder of the M4.O1, M4.O2 and M4.O4 scripts resolved |
| M4.H2 | Findings for F20..F28 as ledger entries, each filed as a Request to its lane (A: group structure, allocator, `ONAM`, NAVM; BW and BQ: confidence; E: the persistence rule) | M4.O1, M3.H2 | Each fixture's first question answered, or marked not observable with the reason |
| M4.H3 | *(research)* `docs/design/persistence.md`, a skeleton marked research: persistent references and what makes a reference persistent; quest aliases and what they keep alive; load and cell-attach events and resets (event names from `papyrus.events`); cell and location resets; procedural locations (what M4.D3 shows; what is regenerated per landing); gotchas. Each claim with its source, its grade (ADR 0005 Am. 3) and the testplan step that would confirm it | - (M4.G2 for event names) | Every claim has a source and a grade; grade D claims are phrased as questions with a test; no SFSE or CommonLibSF source; no prose copied |
| M4.H4 | `docs/game-observations.md`: a ledger for in-game observations (`game-<yyyymmdd>-<nn>`: game build, plugin hashes, profile, steps, expected, seen; screenshots stay in `observe\`); the format `testplan` reports into | - | Format committed; M4.O2's and M4.O4's results recorded in it |

#### Fixture matrix (M4.H1)

Folders and names as in M3 (`observe\<yyyy-mm-dd>-<id>\`, plugins named `KC_`).

| ID  | Fixture | Base | First question |
| --- | ------- | ---- | -------------- |
| F20 | New interior cell with one placed vanilla `STAT` | F01 | Group structure of a new interior cell; block and sub-block; `INCC`; persistent or temporary by default |
| F21 | One new reference in an existing exterior cell of a fixed worldspace | F01 | Shape of the carried `CELL` and `WRLD`; exterior block labels; `ONAM` |
| F22 | The CK generates navmesh for F20's cell (or for the M4 POI plugin loaded as the active file) | F20 | The `NAVM` records and groups M4.A4 imports |
| F23 | One vanilla reference moved | F01 | `ONAM` membership for an overridden temporary child |
| F24 | A new pack-in from a few vanilla objects, if the CK offers it | F01 | `PKIN` as the CK writes it |
| F25 | A quest with a reference alias on F20's reference | F20 | Script-attachment and alias blocks; whether the reference becomes persistent |
| F26 | F20 exported as a small and as a medium master | F20 | How the CK stores new own forms per scale (M4.A3) |
| F27 | The owner of one vanilla interior cell changed | F01 | The ownership members M4.E8 writes |
| F28 | A vanilla outpost object duplicated in the CK | F01 | The CK's clone choices (FormID, EditorID, what is copied) against `clone` |

### Lane T: in-game verification (planning only; added 2026-10-15, ADR 0010 proposed)

An agent-driven session controller that launches Starfield on `KitKraft Dev`, drives it
over the Console API plugin's loopback HTTP endpoint, checks machine-readable probes that
`testplan` derives from the plugin under test, and fills `docs/game-observations.md`
itself. Research and sources: `docs/research/in-game-agent-testing-2026-10-15.md`. Rules:
ADR 0010 (target confinement, text channel only, writes limited to `observe\` and the
ledger). It starts only after the owner rules (M4.O6) and installs the tools (M4.O7), is
on no critical path, changes no M4 exit criterion, and may close M4.O2 and M4.O4 rows the
owner has not run, the owner's sessions keeping the last word. The ladder V0..V4 is the
acceptance column.

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M4.T1 | *(research)* Session facts, each graded (ADR 0005 Am. 3) and recorded in `docs/game-observations.md` (entry `game-<date>-T1`) and `docs/load-order.md`: (a) the MO2 command line that launches a named executable on a named profile (`ModOrganizer.exe --help`, MO2 2.5.2, black box); (b) **V0**: Console API 2.17.0 answers `GetSFSEVersion` over HTTP on `Starfield.exe` 1.16.244 with SFSE 0.2.21; (c) `bAlwaysActive=1` keeps the game running unfocused; (d) the console `load <save>` command from the main menu and from the world; (e) where `bat <name>` looks for its file (closes the O2-04 question without the owner); (f) the runtime IDs the game prints for the dev load order against `runtime-slots-by-scale` and `masters-load-first` (closes M3.D5's in-game half); (g) Console Command Runner's `{Form:…}` replacement as a cross-check of (f); (h) the Papyrus log path and whether the profile INI, not `My Games`, controls it under MO2 | M4.O7 for (b)..(h); (a) needs neither the game nor M4.O7 and starts at once (M5 review fix round, 2026-10-15) | Every fact has an observation with its grade; (b) is V0: pass or the lane stops and reports which plugin failed |
| M4.T2 | `testplan` emits `probes.toml` beside `checklist.md`: one probe per checklist row (same row ids), each a console command or short batch, a matcher (`contains`, `regex`, `number within tolerance`, `papyrus-log contains`), and the expected value derived from the plugin's own records (`FULL` text for `help <name> 4`; `getpos` against the `REFR` position; `GetStage`; `IsDisabled`), plus the persistence protocol as `save`/`load`/`coc` steps; rows with no console or Papyrus getter are emitted with `matcher = "manual"` and the reason | M4.E6 (done) | Deterministic; tests on the rename, first POI, ownership, actors and overhaul plugins; every row of `docs/game-observations.md`'s templates O2 and O4 maps to a probe or a `manual` row |
| M4.T3 | Session controller: new L3 crate `crates/game` (ADR 0002 row in the same commit) and engine commands `game.session`, `game.probe`, `game.stop` (catalog: writing) and `game.status` (catalog: not writing): preflight refusals of ADR 0010 §1, launch through MO2, wait for the endpoint and the loaded save, run `probes.toml`, capture HTTP responses, the console output log, `Papyrus.0.log` and screenshots into `observe\<date>-T<nn>\`, append the ledger entry with every row filled (`pass`, `fail`, `not run (reason)`), terminate the game, verify profile hashes; `--dry-run` prints the plan and writes nothing; a per-game profile file (`crates/game/profiles/starfield.toml`) holds the executable name, port, log paths and commands | M4.T1, M4.T2, M4.O6 | Each refusal has a test on a fixture MO2 instance; the probe runner is tested against a local fake of the `/console` endpoint; the ledger writer is tested for the format; `cargo xtask layers` passes; no write outside `observe\`, the ledger and `KitKraftSession-*` saves (test asserts the profile hashes); `game.session` exposes start, poll and stop, so a long-lived front end (the window, MCP) follows a session through the catalog without holding its one engine worker for minutes: `game.session --start` returns a session id at once and the session runs on a thread the engine's `Session` owns, `game.status <id>` returns its state, the probe rows so far and the screenshots taken (file names and paths under `observe\<session>\`), and `game.stop <id>` (with `--dry-run`) ends it (terminate, ledger entry, profile hashes); the synchronous form (no `--start`) stays for the one-shot CLI; tests for start, status and stop against the fake endpoint (added 2026-10-16 for M6.G4) |
| M4.T4 | **V1**: the rename plugin unattended: deploy (ADR 0009), session, `help "Old Soviet Marksman Rifle" 4` and `player.additem` + inventory screenshot; compare with the owner's `game-20261014-01` | M4.T3 | Ledger entry with `pass` on the `help` probe; the first agent-closed row in `docs/game-observations.md` |
| M4.T5 | **V2**: the first POI, template O2 rows O2-04..O2-11 and the persistence protocol steps 1-4 by console `save`/`load`; the `help KitKraftFirstPOICrate 4` and `getpos` probes; screenshots for O2-07 | M4.T3, M4.E6, the POI package | Every O2 row filled by the controller, `fail`s filed as Requests; `PS-02`, `PS-09`, `PS-26` graded from the entry |
| M4.T6 | **V3**: overhaul build 2 and the recipes (templates O4, O5): every row with a console or Papyrus getter (`getav`, `GetWeaponDamage`-class getters where they exist, `CGF` on Cassiopeia functions, the scripts recipe's `Debug.Trace` line in the Papyrus log); rows without one listed as runtime-need candidates (ADR 0008 §5), not silently skipped | M4.T3, M3.E9, M4.E7..E9 | Ledger entries; a list of unprobeable rows with the getter that is missing |
| M4.T7 | **V4**: regression replay: `observe\replays\<plugin>.toml` (probes plus the session plan) rerun by `cargo xtask game-regress` (opt-in, skipped by `cargo xtask ci` unless `KITKRAFT_GAME_PROFILE` is set) after any merge that changes a recipe or the builder | M4.T5 | A replay of the rename and the POI passes twice in a row on the same base save; a deliberate break (wrong `FULL`) fails |
| M4.T8 | *(research)* Runtime-need record for a KitKraft probe plugin, written only if M4.T4..T6 leave rows that no console or Papyrus route answers; design per `skytest` (passive plugin, `commands.jsonl` / `trace.jsonl`); CommonLibSF stays unread until the ADR 0008 §3 review | M4.T6 | `docs/design/runtime-extensions.md` record, or a one-line "not needed" in `docs/gaps.md` |
| M4.T9 | Game profiles for other titles: stub `skyrimse.toml` and `fallout4.toml` naming the script extender, the console channel candidates (ConsoleUtil-class plugins, AutoTest) and the log paths, unverified and marked so; `docs/game-sessions.md` documents the profile format | M4.T3 | The controller refuses an unverified profile with a message naming what is missing; the doc exists |
| M4.T10 | **End to end from a brief** (owner, 2026-10-15: make it "maximally easy for someone to point an agent at this project and say 'I want a mod that does X'"): (a) `kit mod run <spec>` (engine `mod.run`, catalog: writing) chains `build` -> `validate` -> `roundtrip --verify` -> `package` -> `deploy --dev` -> `testplan` -> `game.session` -> one report (`out\<name>\report.md`: every stage's envelope, the ledger entry id, pass/fail per probe), stopping at the first failing stage with the stage's evidence; (b) `docs/playbooks/mod-from-brief.md`, the agent playbook: brief template (as `docs/overhaul/brief.md` does for the overhaul) -> which recipe or spec kind fits (`overhaul.toml`, `place`, the `docs/recipes/*.toml`) -> `kit mod run` -> read the report -> fix and rerun; a "fix loop" section says what a failing probe means for each recipe kind; (c) AGENTS.md "Start every session" points a mod-building session at the playbook; (d) MO2 convenience scripts under `scripts\` in the repo (PowerShell: launch the dev profile, tail the console log, open `observe\`), each a thin wrapper over `kit` commands so the CLI stays the one implementation | M4.T4 (V1 proves the chain), M4.T3 | A fresh agent session given only the playbook and the rename brief produces the rename plugin, installs it on the dev profile, runs the session and reports `pass` without an owner step (recorded as a transcript check like M2.F7); the same for the first POI brief once V2 passes |

### Lane W: developer workbench (planning only; added 2026-10-14, ADR 0007 Amendment 1)

A minimal, read-only egui workbench over `Engine::execute`: an observability and
development surface with no application logic of its own. It starts only after M4.W0, is
on no critical path, and interrupts no running lane. The full authoring UI is M6
(`ROADMAP.md` "M6+: editor", renumbered from M5 on 2026-10-15).

| ID    | Task | Depends on | Acceptance |
| ----- | ---- | ---------- | ---------- |
| M4.W0 | *(gate, lane F)* Inspection APIs stable: `data` shape tests for `inspect`, `query`, `diff`, `conflicts`, `refs`, `schema --deep`, `validate`, and `build` with `plan`; the catalog marks every writing command (the source of MCP's `destructiveHint`) | M4.D1, M4.D2, M4.E4, M4.F1 | A shape change fails a test; every catalog command is marked writing or not |
| M4.W1 | Shell model and launcher: `crates/ui-egui` (L5, engine only) and `apps/kit`; `kit ui` launcher; engine on a worker thread; only non-writing commands offered; dependency list and licenses in ADR 0007 before merge | M4.W0 | `cargo xtask layers` passes; a test drives a command through the shell model without a window; a writing command cannot be reached from the shell |
| M4.W2 | Record browser: `load`, `query` with `where`, `inspect` (override chain, `limitations`, evidence, M4.D2 placement context) | M4.W1 | A test through the shell model per view; no data computed outside `execute` |
| M4.W3 | Structural diff: `diff` and `conflicts` with `fields[]` paths, before and after, opaque byte ranges | M4.W1 | As M4.W2 |
| M4.W4 | Schema and provenance: `schema --deep` members with `source`, `evidence`, `confidence`; `reference`-only members marked | M4.W1 | As M4.W2 |
| M4.W5 | FormID and reference diagnostics: raw and resolved FormIDs side by side, an unavailable master's scale shown as unknown, `refs`, `formid.dangling` and missing masters from `validate` | M4.W1 | As M4.W2 |
| M4.W6 | Build validation: `validate` by rule, `build` with `plan: true`, `roundtrip --verify` | M4.W1 | As M4.W2; nothing written |

### Owner (lane O)

| ID    | Task | Gates |
| ----- | ---- | ----- |
| M4.O1 | **owner** (or a desktop-control agent the owner assigns): CK session for F20..F28 under the M3.O1 rules (MO2 executable #4, observe only, nothing into Data, no Creations features). F22 is the only way KitKraft content gets navmesh. **Deferred by the owner 2026-10-14**, with M3.O1, until the developer workbench exists (M4.W2..W5); an agent with computer use may then drive it | M4.H2; confidence for M4.A1..A6 and M4.E4; M4.A4's fixture part |
| M4.O2 | **owner**: in-game session 1 on a disposable save: install the rename package (W12), then the first POI (rung 0 interior, then the exterior POI) with its testplan; confirm or refute the batch-file mechanism and the archive naming (M4.G4); record everything in `docs/game-observations.md` | M4 exit (first POI) |
| M4.O3 | **owner decision**: whether SFSE-dependent features may be planned. Default **no**: KitKraft's output is plugins, archives and Papyrus; nothing it builds requires SFSE; CommonLibSF stays unread (GPL). A yes needs an ADR (distribution terms, Steam-only runtime) and lands in M5 at the earliest. **Decided 2026-10-14: default overturned** (ADR 0008): SFSE, native runtime extensions and analogous frameworks are supported where technically justified; core crates never depend on them; runtime components live behind `runtime/` (or an `RT` crate); CommonLibSF is not read or linked until its license exceptions are quoted and reviewed; SFSE binaries are never bundled; PC-first; first component M5 at the earliest | any task that finds a needed hook missing (M4.E7, M4.E9): it writes a runtime-need record (ADR 0008 §5) |
| M4.O4 | **owner**: in-game session 2 on a disposable save: the M4.E7, M4.E8 and M4.E9 recipes (outpost object, fabricator recipe, cell ownership, placed NPC, script event) and the M4.E10 experiment | M4 exit (P0 and P1 deliverables) |
| M4.O5 | **owner decision**: whether `package` may install into MO2's `mods\` directly. Default **no**: it writes a folder (and an optional zip) elsewhere and the owner installs it. **Decided 2026-10-14: default approved with an exception** (ADR 0009): `package` stays target-independent; a separate, explicitly invoked `kit deploy --dev` (M4.E11) writes one isolated `KitKraft Dev - <plugin>` mod with path validation, a hashed manifest, safe replacement, diagnostics and rollback, and never touches profiles, load orders or other mods | M4.E5, M4.E11 |
| M4.O6 | **owner decision** (added 2026-10-15): rule on ADR 0010, agent-driven game sessions on `KitKraft Dev`. **Decided 2026-10-15: approved in principle** (ADR 0010 Amendment 1, with conditions; lane T continues as M5 lane C). Default until decided: **no** (the agent cannot launch the game; every in-game row stays the owner's). A yes accepts the preflight refusals, the text-only channel, and the write limits of ADR 0010 §1-3, and answers: screenshots and desktop input injection in scope for M4.T5 or deferred; whether `crates/game` may add an HTTP client and a process crate (MIT/Apache-2.0) before the list is reviewed | lane T (M4.T1..T9); AGENTS.md "Agent-driven sessions" paragraph |
| M4.O7 | **owner** (after a yes on M4.O6): install into the `KitKraft Dev` profile only, through MO2, the run-only tools Console API and Web Application (Nexus 4280, v2.17.0), Console Command Runner Updated (Nexus 7318, v1.11.0) and, optionally, Console Output To File (Nexus 3142, v1.21); add to the profile's `StarfieldCustom.ini` `[General] bAlwaysActive=1` and `[Papyrus] bEnableLogging=1 bEnableTrace=1 bLoadDebugInformation=1`; set the profile's `StarfieldPrefs.ini` to windowed; create and name a disposable base save (`KitKraftBase`) on the dev profile in a quiet interior cell; record the versions in `docs/source-registry.md` rows CONSOLE-API, CCR-UPDATED, CONSOLE-OUTPUT-FILE. Agents do not download these (Nexus requires the owner's login) | M4.T1 (V0) |

Unchanged and still open: owner decision 5 (Sim Settlements 2: **later**; rung 3 needs
kinggath's consent before any of its assets or scripts are touched), decisions 1, 4 and 6,
every open M3 owner task (M3.O0..O6), M2.F7 and M2.O6. M4.O2 should follow a pass of M2.O6.

**Owner rulings 2026-10-14** (ADR 0007 Amendment 1, ADR 0008, ADR 0009), which win over
older M4 text: M2.F7, M2.O6 and M3.O3 are done; M3.O6, M4.O3 and M4.O5 are decided.
Binding constraint 6's "no new crate" and "UI only if M3.O6 says so" give way to lane W
(`crates/ui-egui`, `apps/kit`, both already in `LAYER_TABLE`); M4.F3 is superseded by lane
W. In "Not in M4": SFSE and CommonLibSF plugins are still not built in M4, but features are
no longer excluded for needing them (a runtime-need record carries them); writing into
MO2's `mods\` has the one exception M4.E11; UI implementation is lane W's read-only
workbench only. Ordering adds `W: M4.D1 + M4.D2 + M4.E4 + M4.F1 -> W0 -> W1 -> W2..W6` and
`E: E5 -> E11`, both off the critical paths.

### Ordering

```
Critical (first POI): BW -> BX1 -> A1 -> A2 -> A5 -> E2 -> E3 -> E6 -> O2
                             A3 -> A5      A1 -> E4 -> E2      G4 -> E5 -> O2
                                                               D4 + H4 -> E6
Session 2 (P0, P1):   BI + BR -> BX1 -> E7 ------------------\
                      BQ -> BX1 -> E9   (G0 -> G1 -> G2 -> E9) +-> O4
                      BC -> BX1 -> E8 ------------------------/
Planet:               BP -> BX1 -> D1 -> D3 -> E10 -> O4
Evidence (raises confidence, never blocks the POI): H1 -> O1 -> H2 -> A3 (F26), A4 (F22), A6 (F23), E4 (F25)

B:  BR, BW, BP (running)      BQ1, BC1, BI1 (start now, in parallel)      BX1 after each merge      BX2 any time
A:  A1 (interior now; exterior after BW) -> A2 -> A4 (+ O1)      A3 (now)      BW + A3 -> A5      O1 -> A6
D:  M3.D4 + BW -> D1 -> D3      A1 -> D2      D4 (now)
E:  A3 + family -> E1 -> E3      E5 (now; archive name after G4)      E7, E8, E9 as above
F:  F1 as commands land      F2 = BX      M3.O6 -> F3
G:  G0 -> G1 -> G2      G3, G4 (now)
H:  H1, H3, H4 (now)      O1 -> H2
T:  O6 -> O7 -> T1 (V0)      E6 -> T2      T1 + T2 -> T3 -> T4 (V1) -> T5 (V2) -> T7 (V4)      T3 + E7..E9 -> T6 (V3) -> T8      T3 -> T9      T4 -> T10 (brief to mod)
```

The long pole is lane BW: typing `CELL` and `REFR` (6.7 M references) at `corpus`
confidence gates the exterior half of M4.A1, the rebuild gate (M4.A2) and reference
composition (M4.A5). Nothing on the critical path waits for a CK session: the corpus
carries every rule there, and F20..F28 raise confidence and settle what the corpus cannot
show (the CK's allocation, `ONAM`, navmesh). `PKIN` needs no typed layout to be placed, so
BP is off the critical path; it gates the planet work (W5, W7). The second in-game session
collects the P0 and P1 recipes once their families merge.

*Update 2026-10-14:* M4.O1 (F20..F28) is deferred with M3.O1 until the workbench exists
(M4.W2..W5). The first POI still waits for no CK session. What waits: `ck-diff` confidence
for the M4.A1..A6 rules and M4.E4's cell rules (they ship at `corpus` confidence from the
corpus alone); the CK's own FormID choices for M4.A3 (F06; corpus defaults are named in
`limitations`) and new forms in small or medium plugins (F11, F26: refused until then);
M4.A4's fixture half (F22; KitKraft content has no navmesh until then, so M4.E8's placed
NPC uses a vanilla cell that has one); the `ONAM` rule for edited vanilla references
(M4.A6, F23: refused while open); `ref.persistence` stays grade D (F25); M4.H2.

### M4 is done when

- Families QUEST, ACTOR and ITEMS/WORKSHOP are typed per the family lane rules, with WORLD,
  PLANET and round 3 merged; M4.BX1 ran after every merge with 0 typed conflicts on all 14
  files; `docs/record-parity.md` shows the per-family counts; the M4.BX2 inventories exist.
- The builder places records under computed cell and world paths and passes the rebuild
  gate (M4.A2); the allocator and reference composition are tested; NAVM passthrough works
  on a synthetic plugin and, if F22 exists, on it.
- `new`, `clone`, `place`, `package`, `testplan`, `planet` and `papyrus.events` are
  `Implemented` and reachable over MCP; spec v3 builds deterministically; the cell rules
  run with 0 errors on the official files.
- **The first POI** is built, validated and packaged by KitKraft and visited in game on a
  disposable save by the owner (M4.O2), with the persistence steps recorded in
  `docs/game-observations.md`, as a pass or as a failure with a follow-up task.
- Every P0 workstream's first deliverable (W1..W6, W12) is done; every P1 deliverable
  (W7..W10) is done or carried with the reason; W11 has its scoping document. M4.O4's
  in-game results are recorded as passes or as failures with follow-ups.
- `docs/design/persistence.md` and `docs/design/planet-content.md` exist as research with
  sources and grades; `docs/research/asset-formats.md` exists.
- The owner's M4.O3 and M4.O5 decisions are recorded (the defaults stand until then).
- `cargo xtask ci` is green; the full corpus roundtrip is still identical (M1.6 release,
  `kit roundtrip --verify` and `--decode` on all 14 files); `cargo xtask parity`
  regenerated; `plan/STATUS.md` says M5.

## Not in M4

- Navmesh generation (the CK does it; KitKraft imports it), terrain and landscape editing,
  and any other data the CK computes that a fixture shows we cannot reproduce.
- Creating or editing meshes, materials, collision, snap points, textures (beyond packing
  DDS the user supplies), Wwise audio, voice, lip sync, face generation, animation and
  interface files; the render viewport.
- Reading or writing save files.
- Papyrus assembling, editing or decompiling; any compiler but Bethesda's.
- SFSE or CommonLibSF plugins, and features that need them (M4.O3; CommonLibSF is never
  read). Lane T **runs** third-party SFSE plugins as installed tools (ADR 0010); it builds
  none, and a KitKraft probe plugin needs a runtime-need record first (M4.T8).
- Procedural placement beyond the M4.E10 experiment.
- Small- or medium-scale output plugins with new forms before F11/F26 evidence; localized
  output plugins (no M4 deliverable needs them).
- Typed layouts for record types outside the families; the M4.BX2 types stay inventories
  until a deliverable assigns them to a family.
- Voiced dialogue; a new quest at all if aliases stay opaque (the W10 stretch).
- UI implementation, unless the owner's M3.O6 decision moves M3.F5 into M4 (M4.F3).
- Sim Settlements 2 (owner decision 5: later); publishing, Creations uploads, a git remote;
  writing into the game folder, the Data dir, the MO2 profile or MO2's `mods\` (M4.O5
  default; the ADR 0010 exception, once accepted, is the game's own session saves and
  logs while `kit game session` runs it on the dev profile).
- Launching the game at all, until M4.O6 is decided (ADR 0010 proposed); reading or
  writing save files stays out even then (sessions use the game's `save` and `load`).
- Oracles beyond SF1Edit and Spriggit; changing the xEdit pin; revisiting decided
  licensing.

## M5: CK-backed world authoring and automated runtime testing

Goal (owner, 2026-10-15, `docs/foothold/owner-decisions-2026-10-15.md`): KitKraft's world
authoring is checked against the Creation Kit, and its in-game claims are closed by
repeatable, isolated, automated game sessions that record save/reload results. Every
capability carries six separate statuses (parsed, typed-readable, typed-writable,
CK-differential-tested, game-runtime-tested, save/reload-verified); where each stands
today is `docs/foothold/gap-matrix.md`. Foothold is the benchmark, not the deliverable
(ADR 0011): `FTH-EXP-nn` tickets are experiments built from general capabilities, and no
Foothold narrative, balance, asset or content work is in M5.

This section extends the scope line at the top of this file to M5. M5 starts on
2026-10-15 while M4 closes: M4 tasks keep their ids, owners and exit criteria, and the
lane T tasks M4.T1..T10 become lane C below under their M4 ids. `ROADMAP.md`'s editor
milestone (the full authoring UI) is renumbered "M6+: editor"; it is not re-planned here.
Text written before 2026-10-15 that puts the authoring UI in M5 (ADR 0007, M3.O6, the M4
lane W note before this change) means M6.
The dependency graph is `docs/foothold/dependency-graph.md`; the sprint that starts now is
`docs/foothold/next-sprint.md`. Reporting follows AGENTS.md "Reporting".

### Binding constraints (owner rulings 2026-10-15)

1. **Schema presence is not feature support.** A cell of the gap matrix moves only with
   the test, fixture or ledger entry that proves it.
2. **Statuses stay apart.** Validation is `not-run`, `pass`, `fail` or `inconclusive`;
   missing observational evidence is `inconclusive`, never `pass` (ADR 0010 Amendment 1).
3. **CK work is owner-assisted until proven otherwise** (addendum item 5). No task assumes
   an unattended CK operation works; M5.A4 is the experiment that may change that.
4. **Fixtures prefer `Starfield.esm` as their only master**; no paid DLC is a mandatory
   dependency (ADR 0011 §3).
5. **No refactor, no new crate** except `crates/game` (ADR 0010 rule 5), no layer change,
   no licensing revisit. Prototypes go to `prototypes/` and use `kit` only (ADR 0011 §2).
6. **CK sources stay local** under `C:\Modding\tools` (ADR 0011 §4); facts from them are
   cited as `corpus:` without quoting.

### Lanes (ownership)

M5 lanes are lettered after the owner's priorities A to F. They replace the M4 lane
letters for M5 tickets only; the crate ownership below decides who edits what.

| Lane | Name | Owns |
| ---- | ---- | ---- |
| A | CK oracle | The `oracle ck` subcommand in `xtask` (`xtask/src/oracle.rs` and its dispatch line in `xtask/src/main.rs`); `docs/ck-observations.md` session scripts and ledger; `docs/ck-parity.md` |
| B | Papyrus | `crates/pex`; engine `handlers/papyrus*`; `docs/recipes/scripts*`; the CK rows of `docs/source-registry.md` |
| C | runtime (was M4 lane T) | As lane T: `crates/game` (L3), engine `handlers/game/`, the `probes.toml` half of `handlers/testplan.rs`, `docs/game-sessions.md`, `observe/` layout; plus the `inconclusive` change to `docs/game-observations.md`'s format |
| D | authoring | Engine `edit/`, `validate/`, `build/`, `handlers/{new,clone,place,set,build}*`; `docs/recipes/` (general recipes); for M5 only, the `defs/` files of `LCTN`, `TERM`, `TMLM`, `PKIN`, `FACT` (family lane rules; generated output through BX), and for M5.D1 the `parity` subcommand in `xtask` (`xtask/src/parity.rs`; `xtask/src/evidence.rs` stays lane A's). Lanes A and D share `xtask/src/main.rs` only; merge order in `docs/foothold/next-sprint.md` |
| E | persistence and workshop experiments | `prototypes/foothold/`; `docs/design/persistence.md`; the FTH-EXP rows of both ledgers |
| F | documentation | `docs/engine-limitations.md` (new), `docs/compatibility.md` (new), `docs/foothold/` (except the owner's verbatim files) |
| H | research | `docs/research/hostile-encounters.md` (new) |
| BX | schema integrator (role of the M4 lane F) | as M4.BX1, for every M5 `defs/` change |
| O | owner | decisions, installs, CK sessions, game sessions the controller cannot close |

### Lane A: CK oracle

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M5.A1 | CK session script for the golden fixtures, owner-assisted. **F29**: the first POI authored in the CK (base `Starfield.esm:1E4FBD`, cell `Starfield.esm:21966F`, the POI's position and rotation, persistent, EditorID `KitKraftFirstPOICrate`), saved with `Starfield.esm` as its only master. **F30**: the CK opens `KitKraftFirstPOI.esm` and saves it unchanged under a new name (the re-export half of Gate K0). Then F21, F20 and F06 from the existing matrices. Each with its `fixture.toml` (`cargo xtask observe new <id>`), the CK build (1.16.244.0), the masters' SHA-256 and steps a person, or a computer-use agent watched by the owner, can follow. **Not a prerequisite of M5.O1**: `plan/STATUS.md` "Owner next actions" row 12 already holds the F29 and F30 steps; A1 turns them into the ledger's session script and adds the later sittings | - | Appended to `docs/ck-observations.md`; F29's `kit_equivalent` is `docs/poi/first-poi.toml`; no step assumes unattended CK operation |
| M5.A2 | `cargo xtask oracle ck <fixture-dir>...`: per fixture `roundtrip --verify` and `--decode`, `validate`, `coverage`, the `kit_equivalent` build, byte runs and `kit diff` against it; each difference classed `identical`, `explained` (an allow-list entry that cites a ledger finding, such as record header stamps) or `unexplained`; non-zero exit on `unexplained`; report in the fixture folder and one summary line for the ledger. Builds on `cargo xtask observe` (M3.H2). A refinement of M5.A3, not its prerequisite: it automates the classing M5.A3 first does by hand | M3.H2 (done) | Tests on synthetic fixture folders (identical, explained, unexplained) with a fake `kit`; nothing written outside the fixture folder; `docs/oracles.md` section |
| M5.A3 | Golden run: F29 and F30 ingested with `cargo xtask observe` (M3.H2, done) and checked with the commands that exist today: `kit roundtrip --verify --decode`, `validate`, `coverage`, and `kit diff` of F29 against `KitKraftFirstPOI.esm` and of F30 against its input; every difference classed by hand in the ledger entry (`identical`, `explained` by a cited ledger finding such as record header stamps, or `unexplained`); rerun through M5.A2 when it lands, its report replacing the hand classing. Members the differential confirms raised to `ck-diff` confidence (a request to the family lane, merged by BX); the `docs/gaps.md` rows on header stamps and new-plugin group order closed or restated | M5.O1 (M5.A2 refines it afterwards) | F29 roundtrips byte-identical through KitKraft; F29 against `KitKraftFirstPOI.esm` and F30 against its input show no `unexplained` difference; the gap matrix's CK-diff cells for `TES4`, `GRUP`, `CELL`, `REFR` filled |
| M5.A4 | *(experiment)* Unattended CK: whether a CK command line or a computer-use agent saves F29 reproducibly with no person at the keyboard (black box only, ADR 0001) | M5.A3 | A result in `docs/ck-observations.md` either way; CK fixtures stay owner-assisted until it passes |
| M5.A5 | The remaining fixtures F20..F28 (M4.O1's matrix) through M5.A2 as the owner records them; M4.A1..A6's rules raised to `ck-diff` where they agree | M5.A1 (the sittings' script), M5.A2, M4.O1 (the owner's F20..F28 sittings, `plan/STATUS.md` "Owner next actions") | One ledger entry per fixture; M4.H2's findings filed |

### Lane B: Papyrus

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M5.B1 | Provenance of the local CK sources: check `C:\Modding\tools\ck-sources\` against `ContentResources.zip` (file list and per-file SHA-256 kept outside the repo; counts and the list's hash in the registry row); `papyrus.events` gives the same index from the folder and from the zip | - | Row CK-CONTENT-RESOURCES complete (who extracted, when, complete or not); a test, ignored without the install, compares the two indexes |
| M5.B2 | PEX differential: `kit papyrus compile` against the official compiler run as the CK runs it (the flags and imports the CK uses, observed black box), for `KitKraft:HelloOnLoad` and a seeded sample of 50 base-game sources; normalized semantic comparison through `papyrus disasm --json` (compile time, user and machine stripped; functions, states, properties, instructions and string tables compared); raw bytes reported separately | M5.B1 | `docs/research/pex-differential.md` with a result per script; every semantic difference explained (flags) or filed; no source text quoted |
| M5.B3 | The scripted activator in game: `docs/recipes/scripts.md` "In game" closed by the owner (M4.O4, row O4-07) or by lane C (M4.T6); then a variant that fires on `OnActivate` on a placed `ACTI` reference, the wording of Gate K0 | M4.O4 or M4.T6 | Ledger entry: loads, fires once per expected event, `Times` read from `VMAD` (two notifications); save/reload row filled |

### Lane C: runtime (M4 lane T)

M4.T1..T10 keep their ids, text and acceptance (M4, "Lane T"), with ADR 0010 Amendment 1
added to every one. One change of dependency: M4.T1's fact (a), the MO2 command line that
launches a named executable on a named profile, needs neither the game nor M4.O7 and starts
now (black box, writing nothing into `C:\Modding\MO2-Starfield`); (b)..(h) still wait on
M4.O7. New tickets:

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M5.C1 | Amendment 1 in the controller, delivered with M4.T3: backups and restore-only rollback of the profile's list files and INIs; the authorized-path list asserted by a test; session and per-probe timeouts; `inconclusive` with its rules; the reproducible-setup header in each ledger entry; `docs/game-observations.md`'s result column gains `inconclusive` | M4.T3 | Tests on a fixture MO2 instance: a changed `plugins.txt` is restored and the session marked `inconclusive`; a missing log or screenshot makes its row `inconclusive`; a write outside the list fails the test |
| M5.C2 | First unattended save/reload: M4.T5 cut to rows O2-06, O2-07 and O2-08 on the first POI (persistence protocol steps 1 to 4 by console `save KitKraftSession-<date>-<nn>` and `load`) | M4.T3, M5.C1, M4.O7 | The controller writes a ledger entry with O2-08 `pass`, `fail` or `inconclusive` and its evidence; compared with the owner's M4.O2 persistence row when that exists |

### Lane D: authoring (support depth, typed write, reference integrity)

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M5.D1 | Support-depth matrix generated: `cargo xtask parity` derives Parsed and Typed-readable per type from coverage, and reads Typed-writable, CK-diff, Game and Save/reload from a checked-in evidence table (`docs/support-evidence.toml`: type, status, citation), producing the family table of `docs/foothold/gap-matrix.md` | - | `parity --check` fails on a stale matrix; a test checks that every cited test name, file or ledger id exists |
| M5.D2 | `LCTN` typed write: `new` and `clone` of a location (parent, keywords, persistent-reference lists, marker data where typed); the 8 `reference` members raised to `corpus` or left refused with the reason; integrity: every FormID member resolves, and an added-persistent-reference entry names a persistent reference of the plugin or its masters (a new `validate` warning until a CK fixture or game row confirms it) | BX for the `defs/` change | Tests through `execute`; a corpus test: cloning a seeded sample of official locations re-encodes identically apart from FormID and EditorID; recipe `docs/recipes/location.md` |
| M5.D3 | `TERM` typed write: `clone` of a vanilla terminal and its `TMLM` menu, menu items naming new forms with `new:`; conditions copied as bytes; integrity of the menu and its targets | BX | As M5.D2; recipe `docs/recipes/terminal.md` |
| M5.D4 | `PKIN` typed write: `clone` of a pack-in with its content list; component bodies copied as stored; integrity of the references the typed members expose; the `VNAM` misread by signature lookup fixed or refused | BX | As M5.D2 |
| M5.D5 | `FACT` typed write: `new` faction with relations and ranks (the rank group raised from `reference` or refused); the ownership recipe gains a new-faction variant | BX | As M5.D2 |
| M5.D6 | Reference integrity across the four types: `refs` and `formid.dangling` follow `VMAD` object property values (the lane E request in STATUS) and count what they cannot see | M5.D2..D5 | Corpus run with newly visited FormIDs per type in STATUS; no new unexplained error on the official files |
| M5.D7 | Gate K0 mini-plugin, a general recipe (`docs/recipes/mini-site.toml`): a new `LCTN`, a placed `ACTI` reference with the M4.E9 script, a `TERM` clone and a `KYWD`, linked to each other, `Starfield.esm` only | M5.D2, M5.D3, M4.T2 (the probes) | `validate` 0/0 with no dangling reference; roundtrip identical; testplan and probes written; game and save/reload rows by lane C or the owner |

### Lane E: persistence and workshop experiments (Foothold)

The engineering surface's EXP-01..EXP-12 (section J) as `FTH-EXP-nn`. Every status is the
owner's vocabulary; all are `not-run` today unless the row says otherwise. Each produces
a prototype under `prototypes/foothold/` and ledger rows; a failure is a result and goes
to `docs/engine-limitations.md`. Game rows have two routes, as M5.B3 has: the unattended
controller (M5.C2) or an owner-assisted sitting from the ticket's testplan (M5.O4), so an
experiment can run before the automation lands; "M5.C2 or M5.O4" below means either.

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| FTH-EXP-01 | Authored-POI workshop (not-run): an authored exterior location with `LocTypeOutpost`, marker and beacon; place, power, move, reload | M5.D2, M5.D4, M5.D7, M5.A5 (a CK twin of the beacon setup), M5.C2 or M5.O4 | Ledger rows for place, move and reload, each with a status; the CK twin's differential |
| FTH-EXP-02 | Interior decorator (not-run): a connected interior with a decoration activator | FTH-EXP-01 | As FTH-EXP-01, interior |
| FTH-EXP-03 | Stable claim (not-run): scripted claim terminal, owned location; theft, trespass and ownership through travel and save. Inputs: the ownership recipe (O4-01, O4-02), PS-39 (O4-21) | M5.D3, M5.D5, M5.B3, M5.C2 or M5.O4 | Rows per scope (reference, cell, location) |
| FTH-EXP-04 | Restoration stages (not-run): enable and disable groups across three stages; PS-12 (O4-13); the initially-disabled flag is at `reference` confidence today | M5.C2 or M5.O4, M5.A5 | Stage state after reload per stage |
| FTH-EXP-05 | Site-dependent resource (not-run): a scripted producer that only its linked node feeds | M5.B3, M5.D7 | No production elsewhere, no duplicates on load |
| FTH-EXP-06 | Instance identity (not-run): two generated locations from one POI template; PS-32 (O4-20); builds on M4.E10 | M4.E10, M5.C2 or M5.O4 | A go/no-go for procedural claims with its evidence |
| FTH-EXP-07 | Reset protection (not-run): PS-23, PS-24, PS-26..PS-28 (O4-18, O4-19) and the long absence of O2-09 | M5.C2 or M5.O4 | Rows per reference kind after more than 168 in-game hours |
| FTH-EXP-08 | Crew and navigation (not-run): the actors recipe (O4-03, O4-04); navmesh only from the CK (F22) | M5.A5 (F22), M5.C2 or M5.O4 | Route and assignment rows |
| FTH-EXP-09 | Free Lanes and Terran Armada interoperability (not-run; addendum item 5, "Test interactions with Free Lanes and Terran Armada outpost changes where applicable"): `SFBGS00D.esm` shared outpost storage, and each outpost change of `SFBGS00D.esm` and `SFBGS050.esm` that M5.F5 finds; optional fixtures (ADR 0011 §3) | FTH-EXP-01, M5.F3, M5.F5 | Rows with and without the shared container; a row per M5.F5 change the experiment touches, or the reason it does not apply |
| FTH-EXP-10 | Master-aware roundtrip (partial: decode with full, medium and small masters passes on the corpus; new forms in small or medium plugins refused; `FE`/`FD` runtime IDs not-run) | M5.A5 (F11, F26), M5.C2 or M5.O4 | xEdit or CK and a game row agree on every identity |
| FTH-EXP-11 | Packaging, loose against BA2 (not-run in game; `package` writes both) | M5.C2 or M5.O4 | The same probes pass on both installs |
| FTH-EXP-12 | Update and uninstall (not-run) | FTH-EXP-03, M5.C2 or M5.O4 | Migration or a documented refusal; no silent loss |

### Lane F: documentation

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M5.F1 | Engine limitation register `docs/engine-limitations.md`: experiment, exact API or CK behaviour, game build, workaround tried, observed failure, hook rationale (ADR 0008 §5 records link to it); seeded from evidence only (navmesh is CK-only, component blocks raw, `CTDA` raw, new forms in small or medium plugins refused, no save-file reading) | - | Every row cites a test, a `docs/gaps.md` row or a ledger entry; nothing from the brief without evidence |
| M5.F2 | Keep `docs/foothold/gap-matrix.md` current after every M5 merge until M5.D1 generates it | each merge | The matrix names the commit it was checked at |
| M5.F3 | Installed-version and DLC compatibility matrix `docs/compatibility.md`: game, CK, SFSE, Papyrus compiler, xEdit; every official master with SHA-256 (`corpus/manifest.toml`), scale and masters, and the product that ships it (the owner's mapping checked against the Steam install manifests, read only) | - | The owner's DLC mapping confirmed or corrected with evidence; ADR 0011 §3 points to it |
| M5.F4 | Ledger transcription: the owner's M3.O5 and M4.O2 results into `docs/game-observations.md` as entries with the plugins' hashes; rows the owner did not report stay `not-run`. Lane F's first ticket: until it lands, the gap matrix writes those results `pass (owner report; ledger entry pending M5.F4)` | - | Two entries; the gap matrix's G-OVH and G-POI cite them, and its owner-report cells become `pass`, or `inconclusive` where a hash cannot be recovered |
| M5.F5 | Free Lanes and Terran Armada outpost changes, observed in the files (addendum item 5): with `kit conflicts`, `kit diff` and `kit query` (read only, the installed masters in place), every record `SFBGS050.esm` (Terran Armada) and `SFBGS00D.esm` (Free Lanes) overrides from or adds beside `Starfield.esm` in the outpost and workshop families (`COBJ` with workshop keywords, outpost `KYWD`s, the `STAT`/`MSTT`/`FURN`/`CONT`/`TERM`/`ACTI`/`GBFM` they build, `LCTN`s with `LocTypeOutpost`, outpost `FLST`s and `GLOB`s), each with FormID, EditorID, the change classed, and the command that shows it; products named per ADR 0011 §3's mapping until M5.F3 confirms it | - | A section of `docs/compatibility.md`; every row cites a reproducible command and the masters' SHA-256; FTH-EXP-09's rows chosen from it; nothing claimed about runtime behaviour |

### Lane H: research

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M5.H1 | *(research)* Hostile encounters before hooks (addendum item 3): from the corpus and our docs (`kit query`, `refs`, `papyrus events`), how vanilla starts, runs and ends hostile encounters at outposts and in the Starborn and Armillary content (story manager event nodes, quests and their script events, encounter zones, leveled actors, cooldown globals), and what prior-art attack managers do (their published documentation only; no GPL source read), covering the addendum's bullets "Combat behavior, settlement defenses, damage, and recovery." and "Compatibility with vanilla outpost assaults and current DLC behavior."; each fact sourced and graded (ADR 0005 Am. 3); then a proposal for one generic fixture and its probes, or a runtime-need record (ADR 0008 §5) if no data or Papyrus path exists | - | `docs/research/hostile-encounters.md`; no runtime hook proposed before it; no Foothold attack design |
| M5.H2 | *(backlog)* The generic hostile-encounter fixture M5.H1 proposes: configurable trigger, actors, faction, spawn location and cooldown; deterministic test trigger; "Combat behavior, settlement defenses, damage, and recovery."; cancel and cleanup; save/reload; attended and unattended; "Compatibility with vanilla outpost assaults and current DLC behavior." (addendum item 3, verbatim) | M5.H1, M5.C2, M5.D7 | Not scheduled in M5 |

### Owner (lane O)

| ID | Task | Gates |
| -- | ---- | ----- |
| M4.O7 | Unchanged (M4): install the Console API, Console Command Runner Updated and optionally Console Output To File into `KitKraft Dev` through MO2; the INI switches; the base save `KitKraftBase` (its name and SHA-256 recorded). Agents do not download or install them | M4.T1, so all of lane C |
| M5.O1 | Owner-assisted CK sitting 1: F29 (place one crate and save) and F30 (open `KitKraftFirstPOI.esm`, save a copy), screenshots, files into `observe\<date>-F29\` and `-F30\`; a computer-use agent may drive the CK while the owner watches. Can start now from `plan/STATUS.md` "Owner next actions" row 12; M5.A1's script refines later sittings | M5.A3, so path (a) below |
| M4.O1 | Unchanged (M4): the CK sittings for F20..F28; no longer deferred for M5 (STATUS "Owner next actions" row 15) | M5.A5, so FTH-EXP-01, -04, -08, -10 |
| M4.O4 | Unchanged (M4): in-game session 2, including the scripts recipe in game (row O4-07) (STATUS "Owner next actions" row 14) | M5.B3 (or M4.T6), M5.O3 |
| M5.O2 | Confirm `crates/game`'s dependency list at the M4.T3 merge (ADR 0010 Amendment 1 (c)); blocks only if a license is not MIT or Apache-2.0 | M4.T3 merge |
| M5.O3 | M4.O4 rows not closed by lane C (the scripted activator O4-07 included) and the M4.O2 persistence row stay the owner's; lane C may close them first | M5.B3, FTH-EXP rows |
| M5.O4 | Owner-assisted game sitting for an FTH-EXP ticket, on `KitKraft Dev` with a disposable save, from the ticket's `kit testplan` checklist; the alternative to M5.C2 for that ticket's game and save/reload rows, recorded in the ledger with the plugins' hashes | the FTH-EXP ticket it serves |

### The shortest validated path

From `main` at `fcb443a` to (a) a CK-authored golden fixture round-tripping through KitKraft
with a clean differential, and (b) one unattended game session recording a save/reload
result. Owner actions in bold.

(a) CK golden fixture:

1. **M5.O1: the owner-assisted CK sitting for F29 and F30** (CK fixture authoring is
   owner-assisted at first; addendum item 5). It can start now: `plan/STATUS.md` "Owner
   next actions" row 12 holds the steps. Off the path, in parallel: M5.A1 (the session
   script for the ledger and the later sittings) and M5.A2 (`cargo xtask oracle ck`).
2. M5.A3 with the tools that exist (`cargo xtask observe`, `kit roundtrip --verify
   --decode`, `validate`, `kit diff`): F29 roundtrips byte-identical and shows no
   `unexplained` difference against `KitKraftFirstPOI.esm`; F30 against its input
   likewise, each difference classed in the ledger entry. (a) is reached. M5.A2, when it
   lands, reruns the same fixtures and replaces the hand classing.

(b) Unattended save/reload:

1. **M4.O7: the owner installs the Nexus console plugins into `KitKraft Dev`, sets the INI
   switches and makes `KitKraftBase`** (agents cannot download from Nexus). In parallel:
   M4.T1 (a) (the MO2 command line, black box; it needs no M4.O7), M4.T2 (`probes.toml`,
   agent), and the parts of M4.T3 + M5.C1 that need no game (preflight, backups and
   rollback, probe runner against a fake endpoint, ledger writer).
2. M4.T1 (b)..(h): session facts and V0 (`GetSFSEVersion` over HTTP), after M4.O7.
3. M4.T3 + M5.C1 finished with M4.T1's launch facts; **M5.O2: the owner confirms the
   dependency list at the merge**.
4. M5.C2: the controller loads `KitKraftBase`, checks the first POI, saves
   `KitKraftSession-*`, reloads and records O2-08 as `pass`, `fail` or `inconclusive`.
   (b) is reached.

Neither path needs lane D, E or H. Gate K0's other two open items (the mini-plugin in game
and the scripted activator firing) follow from M5.D7 and M5.B3 once (b) exists.

### M5 is done when

- Paths (a) and (b) are reached, and M5.A5 has run every fixture the owner recorded.
- `LCTN`, `TERM`, `PKIN` and `FACT` are typed-writable with reference integrity (M5.D2..D6),
  and the K0 mini-plugin (M5.D7) has game and save/reload rows.
- Gate K0 is met: the scripted activator fires in game (M5.B3) and the CK re-export
  differential is clean (M5.A3).
- The PEX differential (M5.B2) is recorded; the CK sources' provenance is complete (M5.B1).
- Every FTH-EXP ticket has a status with evidence (`not-run` with its reason counts).
- `docs/engine-limitations.md`, `docs/compatibility.md` and a generated gap matrix exist;
  `docs/research/hostile-encounters.md` exists.
- `cargo xtask ci` is green; the full corpus roundtrip is still identical.

## Not in M5

- Foothold narrative, quests, balance, asset polish, release readiness, and the full
  Foothold attack system (ADR 0011).
- The full authoring UI (`ROADMAP.md` "M6+: editor"), navmesh generation, terrain, mesh
  and material authoring.
- Unattended CK operation as an assumption (M5.A4 decides it); desktop input injection
  into the game (ADR 0010 Amendment 1 (b)).
- Reading or writing save files (sessions use the game's own `save` and `load`).
- A paid DLC as a mandatory dependency; Sim Settlements 2 content.

## M6: Cockpit

Goal and exit criteria: `ROADMAP.md` "M6: Cockpit" (owner decisions 2026-10-16). In
short, the workbench becomes the human cockpit beside the agents: it looks deliberate,
starts fast, and runs the game loop (deploy, session, probes, screenshots, ledger) through
engine commands only. Agents stay the main authors through MCP. This section extends the
scope line at the top of this file to M6. Text written before 2026-10-16 that calls the
full authoring UI "M6" or "M6+: editor" now means M6 to M8.

Planned 2026-10-16 while M5 runs. M6 tickets that need nothing from M5 can start once
their M6 dependencies are met. The game-loop tickets wait on lane C, by ticket id.

### Decisions taken by the director 2026-10-16, pending owner

Open questions from the review of this plan, answered so the lanes can proceed. Each
stands until the owner overrules it (`plan/STATUS.md` row 16).

1. **`crates/nif` stays behind the engine.** The window gets bounds and geometry only
   from engine commands; `INTERNAL_ONLY` is unchanged and there is no L5 -> L0 edge. ADR
   0002 Amendment 1 records this and retires the L2 `render` slot: the viewport is L5
   view code in `crates/ui-egui/src/viewport/`.
2. **Geometry encoding (M7.S5):** `mesh.geometry` returns base64 little-endian `f32`
   (positions, normals) and `u32` (indices) buffers inside the JSON envelope. The engine
   produces them, so the CLI and MCP get identical results; the cost is about 1.33x the
   raw bytes. M7.R4 measures it as V4, and an ADR for another channel is written only if
   V4 fails.
3. **Cockpit write scope (ADR 0007 Amendment 3):** `deploy.rollback` is in, as a safety
   control (what the cockpit deploys, it can take back); `testplan` stays out (file
   output, not a safety control). `game.stop` joins with M4.T3's start, poll and stop
   split (M6.G4). **Confirmed by the owner 2026-10-16** with the acceptance of
   Amendment 3 (M6.O3).
4. **CK menu order** stays `unresearched` until CK interface sitting 1 (M7.O2). The
   order written in `docs/ck-parity.md` and `ROADMAP.md` is a recollection, not
   evidence; M7.M1 waits on M7.K3 and builds nothing on it before then.

### Binding constraints (owner, 2026-10-16)

1. **Keep egui.** Pure Rust, one binary. The license rule is unchanged: MIT, Apache, Zlib
   or BSD-style, plus the BSL-1.0 clipboard crates pending in STATUS row 7. A new crate is
   listed in ADR 0007 (Amendment 3's dependency table) before its merge. OFL fonts stay
   off unless the owner rules otherwise (M6.O2). No web stack.
2. **Everything through catalog commands.** `crates/ui-egui` names the engine only in
   `gate.rs` and depends on no other workspace crate (`INTERNAL_ONLY`). A view that needs
   data no command returns gets it from the lane that owns that command, as a request
   ticket (marked "engine" below).
3. **Writes:** only ADR 0007 Amendment 3's list (accepted by the owner 2026-10-16, M6.O3):
   `ledger`, `deploy --dev`, `deploy.rollback`, `game.session`, `game.probe`, `game.stop`.
   Each is previewed and confirmed per click and enforced by the gate test. M6.G1 (the
   mechanics, scope `[ledger]`) needed no ruling; M6.G3 to M6.G6 waited on M6.O3, which
   is now decided, and wait only on their other dependencies.
4. **No game launcher in the UI.** `game.session`, `game.status`, `game.probe` and
   `game.stop` are lane C's (M4.T3). Until they are `Implemented` in the catalog, their
   buttons are disabled with the ticket id. No process spawning, MO2 command line or
   Console API call in `crates/ui-egui`. The window follows a running session by polling
   `game.status <id>` through the catalog; it never grows a second engine worker.
5. **Measure first.** No performance ticket merges without before-and-after rows in
   `docs/design/workbench-performance.md` "Results".
6. No Foothold content, no new runtime hooks, and no layer change beyond ADR 0002
   Amendment 1 (2026-10-16: the viewport is L5 view code in `crates/ui-egui`, the L2
   `render` slot is retired, `crates/nif` stays behind the engine).

### Lanes (ownership)

All four lanes edit `crates/ui-egui` (and `apps/kit` for P6 and P7), so they share files.
M6.V1 goes first: it moves each tab's drawing out of `app.rs` (2,705 lines) into its
`panels/` file and adds an empty theme module, so the lanes after it touch different
files. "engine" rows belong to the lane that owns the engine code named, by M5's lane
letters: lane D for `conflicts` and the reverse-link index (M6.P3b, M6.P5b), lane C for
`game.*`. M6.P2b and M6.P6b touch **engine core** (`crates/engine/src/lib.rs`,
`apps/kit-cli`, `crates/mcp`): M4's lane F owned it, M5's table gives it no letter (M5's
lane F is documentation), and the director names its writer. Their writer works in the
engine, not in `ui-egui`.

| Lane | Name | Owns |
| ---- | ---- | ---- |
| V | visual system | `crates/ui-egui/src/theme.rs` (new), panel frames and tables, `docs/design/workbench-style.md` (new) |
| P | performance | `crates/ui-egui/src/{worker,envelope}.rs`, the perf trace, `apps/kit` (launch, window), `docs/design/workbench-performance.md`; engine requests P2b, P3b, P5b, P6b through their lanes |
| G | game loop in the UI | `crates/ui-egui/src/gate.rs` (with the Amendment 3 tests), `panels/{cockpit,ledger,coverage,review}.rs` |
| X | accessibility and first run (**DEFER**, 2026-10-16; see "Lane X") | keyboard and focus handling, scaling, AccessKit, `panels/{start,help}.rs`, the Help sections in `docs/workbench.md` |
| O | owner | decisions, sign-offs |

### Lane V: visual system

**Started 2026-10-16** (owner, in chat: "Approve Amendment 3 and ADR 0012, start lane V
mockups"). M6.V1 and M6.V2 landed; **M6.O1 was decided 2026-10-16: Workbench** (owner, in
chat, verbatim: "Workbench, themes look great only keep the mocks if there's a good reason."). M6.V3 and the rest of M6.V1 (the `app.rs` split)
landed in lane V round 2; M6.V4 onward are open.

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M6.V1 | Make room for parallel work, with no visible change: each tab's drawing moves from `app.rs` into its `panels/` file; a `theme.rs` holds today's colors (`WARN`, `BAD`, `GOOD`, `CODE`) and spacing as named tokens | - | `app.rs` holds the shell only (under 900 lines); the screenshot tour's images show the same content before and after; the gate tests are unchanged and pass |
| M6.V2 | Two static mockups, rendered by the workbench itself: two theme presets (for example a dense dark-first and a calmer light-first one; each with its type scale, spacing, palette and table density) applied to Start, Records, Diff, Validate and Ledger; screenshot tour images in light and dark at 100 percent, kept outside the repo; a one-page comparison in `docs/design/workbench-style.md` (draft) | M6.V1 | 2 presets x 5 tabs x 2 modes captured; the owner has them for M6.O1 |
| M6.V3 | The token system for the chosen direction: type scale, 4 px spacing scale, color tokens (surface, raised, text, muted, accent, border, selection, focus, and status good, warn, bad, unknown, inconclusive) for light and dark, table density (compact, comfortable); light, dark or system, saved in `ui.json` | M6.O1 | A test scans the crate and finds no `Color32` literal outside `theme.rs`; a unit test computes the contrast of every text and background pair (4.5:1 for text, 3:1 for borders and the focus ring) in both modes |
| M6.V4 | Status is never color-only: one status component (glyph, word, color) for pass, fail, warn, unknown, inconclusive, not-run, the confidence levels and resolved or unresolved FormIDs, used by every panel; our own glyphs drawn with egui's painter (no icon font unless M6.O2 allows one) | M6.V3 | A test finds no status text colored outside the component; the tour shows each status in light and dark |
| M6.V5 | Consistent panels: one panel frame (title, action bar, the envelope header from `docs/workbench.md` "Every result looks the same"), one table style (sticky header, zebra rows, numbers right-aligned, FormIDs monospace, column resize), and empty, busy and error states; all 11 tabs moved onto it, and M7.R1's viewport panel (toolbar, status line) when it has merged | M6.V3, M6.V4, M6.P4 (tables) | Every tab uses the frame and the table style (a review checklist in the PR); tour images for all tabs |
| M6.V6 | `docs/design/workbench-style.md`: tokens, components, spacing and density rules, do and don't, with tour images referenced (kept outside the repo) | M6.V5 | A doc-sync test: every token the doc names exists in `theme.rs` and every token in `theme.rs` is in the doc |

Lane V notes (2026-10-16, lane V, on the director's brief of the same day, which scoped V1
and V2 differently from the rows above):

- **M6.V1 as delivered:** the token set, not the `app.rs` split. `crates/ui-egui/src/theme/`
  (`tokens.rs` data, `mod.rs` with `apply(ctx, &Theme, Mode)`), three presets (`classic` =
  today's look and the default, `workbench`, `studio`), the status component and table
  style, today's `WARN`/`BAD`/`GOOD`/`CODE` moved into `classic`, the AA contrast test, and
  `docs/design/workbench-visual-system.md`. **Still open from the row:** moving each tab's
  drawing out of `app.rs` (now about 2,790 lines) into `panels/`; the theme is reached
  through `theme::*` helpers, so that move does not touch the tokens.
- **M6.V2 as delivered:** 2 presets x 3 screens (Start, Records with the inspector,
  Validate) x 2 modes, 12 PNGs committed under `docs/design/mockups/2026-10-16/` (3.6 MB,
  the director asked for them in the repo; removed after M6.O1, in the history at
  `48beef1`) with a one-page comparison README; Diff and
  Ledger were not captured. `kit ui --theme <preset>` and `ui.json` `theme` select a
  preset. The tour now writes PNG (`image`, already in the tree; ADR 0007 Amendment 3
  table).

- **M6.V1 rest and M6.V3, lane V round 2 (2026-10-16, after M6.O1):** each tab's drawing
  moved from `app.rs` (2,791 lines) into `crates/ui-egui/src/panels/<tab>.rs` behind one
  `fn ui(&mut self, ui, ctx)` per tab (the view-models stay above a `// ---- drawing`
  line in the same file; shared pieces in `panels/common.rs`); `app.rs` is the shell
  (851 lines). `workbench` is the default preset (system light or dark; `--theme` and
  `ui.json` override); the Start tab has a picker (preset; system, light or dark) that
  writes `ui.json` (the row's "saved in `ui.json`"). New tokens: `control_border` and
  `focus` (3:1 on every surface, tested), `status.unknown` (glyph: empty square),
  `type_scale.subheading`, `spacing.block`, and `Sizes` (field widths, scroll views in
  rows, Help cell widths, the Records split). Every panel takes colors, sizes and gaps
  from `theme`; the scan test `no_color_size_or_spacing_literal_outside_the_theme` covers
  every file under `src/` outside `theme/`. Layout: the session text moved from the top
  bar to a status line at the bottom; Records is two panes (types and query left,
  inspector right) with a draggable line whose share is kept in `ui.json`
  (`records_split`). The tour runs at 1400 x 900.

### Lane P: performance

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M6.P1 | **Measure first.** The `KITKRAFT_WORKBENCH_PERF=1` trace to stderr (first frame; each request's queued, started, finished, prepared and first-drawn times; envelope bytes; frame times per tab, p50, p95 and max), then the protocol of `docs/design/workbench-performance.md` on the release build and the dev profile, cold and warm | - | Every row S1 to U1 has a window number with the build's SHA-256 and commit; each target is marked confirmed or revised; P2 to P7 re-ordered by what the numbers show |
| M6.P2a | Progress in the window for long commands: the per-plugin progress P2b emits, as a progress line on the Load tab and the top bar; the window stays responsive (T3) | M6.P1, M6.P2b | L4 and T3 measured before and after; the first progress is under 0.5 s on a cold load |
| M6.P2b | *(engine core)* An optional progress sink on `Engine::execute`, used by `load` (one event per plugin indexed) and the reverse-link index build; `kit --progress` prints to stderr; MCP sends progress notifications when the client asks for them | - | Tests: events in order, none when no sink is given; the result envelopes are unchanged (shape tests) |
| M6.P3a | Index cache reuse in the window: no second `load` for the same profile and arguments; the start screen's last profile loads from the cache at start (when one was used before); cache hits shown on the Load tab | M6.P1 | L2 before and after; a test through the shell model checks that a repeated load request is answered from the session |
| M6.P3b | *(engine, lane D)* The reverse-link index (`refs --incoming`) kept on disk beside the index cache, invalidated by the same plugin stamps | M6.P1 | I4 cold under 21 s and warm under 1 s across process restarts; the cache tests extended |
| M6.P4 | Virtualized tables: every table that can pass 200 rows (load, query, links, diff, conflicts, findings, schema) draws only the visible rows (`ScrollArea::show_rows`, or `egui_extras` tables once listed in ADR 0007); paging kept | M6.P1 | T1 p95 under 16 ms on a 10,000-row table in release; a test on a canned 100,000-row envelope checks that the view-model indexes rows without copying them |
| M6.P5a | Envelopes prepared off the UI thread: the worker builds each panel's view-model (and formats the raw JSON lazily, by visible range) before handing it to the UI | M6.P1 | T3 and I2 (window) before and after; no frame over 50 ms while a 30 MB envelope arrives |
| M6.P5b | *(engine, lane D)* `conflicts` gets a `detail` argument (`summary` by default: classes and changed signatures; `fields` and `evidence` on request); the 50-row page of `SFBGS00D.esm` drops from 29.8 MB and 26.7 to 41.6 s (`docs/design/workbench-performance.md`) | M6.P1 | I2 under 3 s and 2 MB at `summary`; the shape test and `docs/shapes.md` updated; CLI and MCP get the same argument |
| M6.P6a | Release by default: `cargo xtask ui` builds release and starts the window; `bin\` and `dist\KitKraft` both carry `kit-ui.exe` beside `kit.exe` (`bin\kit.exe ui` exits 2 today: "kit-ui.exe is not next to kit"); a debug build says so in the title bar as well as on the console; measure whether `[profile.dev.package."*"] opt-level = 2` for the egui tree makes debug runs usable, and adopt it only if the CI time cost is under 10 percent | - | `kit ui` from `bin\` and `dist\KitKraft` starts a release window; xtask test; the opt-level decision recorded with numbers |
| M6.P6b | *(engine core)* `kit --version` and the window's About line name the commit and whether the tree was dirty (from an env var `cargo xtask dist` sets at build time; no `build.rs`) | - | `kit --version` prints `kit 0.0.0 (<commit>)`; the performance doc's "Build identity" uses it |
| M6.P7 | Window quirks: at start, if the viewport is minimized, restore it and take focus (`ViewportCommand`); size, position, maximized state and monitor saved in `ui.json` and clamped to the monitors present at the next start; the screenshot tour no longer waits on a minimized window | M6.V1 | S2 passes from a background shell (owner check in M6.O4); a unit test for the clamp; `ui.json` stays UI state only (ADR 0007 Amendment 3) |

### Lane G: game loop in the UI

Lane C (M5) owns every game command. These tickets show and confirm them, nothing more.

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M6.G1 | Gate mechanics of ADR 0007 Amendment 3, enabling no new write and needing no ruling: `PERMITTED_WRITE` becomes `PERMITTED_WRITES` (id, preview test, the one panel file allowed to confirm, why); `COCKPIT_SCOPE = ["ledger"]` in the test (Amendment 2's scope; it grows only with M6.G3 to M6.G5 after M6.O3); the source scan generalized per entry; `deploy.status` and `doctor` move from "unused read-only" to `ALLOWLIST` | - | The extended disjointness test passes with `PERMITTED_WRITES = [ledger]`; adding a non-scope id in a scratch test fails it; `docs/workbench.md` "What it can run" rewritten |
| M6.G2 | The Cockpit tab: the plugin under review (from `out\` or a path), its dev deployment (`deploy.status`), the dev profile, the session readiness (`game.status` when the catalog has it), and the buttons Deploy, Roll back, Launch and run probes, Stop, Re-run probe, Screenshot. Each button's state is read from the catalog: present and `Implemented` turns it on; otherwise it is disabled with the ticket id (M4.T3) | M6.G1, M6.V5 | A shell-model test with a catalog lacking `game.*` shows the buttons disabled with "M4.T3"; with a fake `Implemented` spec they turn on |
| M6.G3 | Deploy to dev and roll back: `deploy` and `deploy.rollback` with `dev: true`. The panel fills `mo2` and `profile` from the loaded dev profile (and, for rollback, `mod` and `to` from `deploy.status`) and keeps the confirm box closed when they differ from it; the gate checks only `dev: true`, the writing form and `profile` not `Default` (ADR 0007 Amendment 3, "What the gate checks"); the engine refuses `Default` on its own. Preview runs `dry_run: true` and lists every path; the confirm box shows that list; the confirm button builds the one confirmed request | M6.G2, M6.O3 (decided 2026-10-16) | First, the gate test extension Amendment 3 names: `COCKPIT_SCOPE` and `PERMITTED_WRITES` grow by `deploy` and `deploy.rollback` (ADR 0007 Amendment 3 accepted 2026-10-16), and the extended disjointness test and the per-entry source scan pass with them; gate tests per Amendment 3 for both ids; a shell-model test deploys to a synthetic MO2 instance, reads it back with `deploy.status`, rolls it back and reads it again; nothing written without the click; a profile mismatch keeps the confirm box closed |
| M6.G4 | Launch the dev profile and run probes: `game.session` with `start: true` (preview with its `dry_run`, confirm) returns a session id at once, with M6.P2b's progress events for its own steps; the window then follows the session by polling `game.status <id>` through the catalog on a timer, each poll a short read on the one engine worker (no second worker, no process of its own); Stop through `game.stop <id>` (preview, confirm); the result: every probe row with `pass`, `fail`, `inconclusive` or `not-run` and the reason | M6.G2, M6.O3, M6.P2b, **M4.T3** (start, status, stop), **M5.C1**, M4.T2 | Gate tests; a shell-model test against M4.T3's fake endpoint and fixture instance: start, at least two polls with another read served between them, stop; the V1 run (M4.T4) started from the window by the owner (M6.O4) |
| M6.G5 | Probe results and screenshots: rows beside the plugin's records (a click opens `inspect`); the screenshots `game.status <id>` lists (file names and paths, returned through the catalog) shown as images. The window opens only the image files at the paths `game.status` returned, as the Help tab reads the repo's docs (ADR 0007 Amendment 2: our own evidence files, not game data); it lists no folder of its own; image loading listed in ADR 0007 first. Re-run one probe and Screenshot through `game.probe` (confirmed) | M6.G4, M6.O3, **M4.T3** | Tests on a canned `game.status` envelope; a listed screenshot missing on disk is shown as `inconclusive`, never hidden |
| M6.G6 | The ledger prefilled from probe results: rows the controller left `inconclusive` or `manual` open in the Ledger form with the row id, FormIDs and the screenshot beside it; the owner confirms each row with the existing confirmed `ledger` request | M6.G5, M6.O3, **M5.C1** (`inconclusive` in the ledger format) | A test from a canned session result to the Ledger form's fields; each row still needs its own click |
| M6.G7 | The Coverage panel: `coverage --matrix` (read-only, already allowed), one row per record type with the six statuses, each cell linking to its evidence (test name, ledger entry, fixture) | M6.V4, **M5.D1** (the matrix; on `main` as `kit coverage --matrix`) | A view-model test on the engine's matrix shape; statuses shown with the status component |
| M6.G8 | Review an agent-built plugin: pick a plugin under `out\`; one fixed sequence of `validate`, `roundtrip --verify` and `conflicts --plugin` (against its masters); a summary with the six statuses filled from what ran (`parse`, `byte_roundtrip`) and the rest `not-run` | M6.V5 | Shell-model test on a fixture plugin; nothing written |

### Lane X: accessibility and first run (priority DEFER)

**Deferred 2026-10-16.** The owner's top pains were "look, feel, performance" and "game
loop integration"; navigation was not selected and accessibility was not named. Lane X
therefore leaves M6's exit path. Its tickets stay here with priority DEFER and may be
picked up as P2 once M6's exit criteria are met, or when the owner asks. No V, P or G
ticket depends on a lane X ticket: what they need already lives in their own tickets
(the focus-ring token in M6.V3, no color-only status in M6.V4, the window restored and
focused in M6.P7, `doctor` on `ALLOWLIST` in M6.G1), so nothing is re-homed. The check at
125 and 150 percent is an optional part of M6.O4. M7.M5 no longer depends on M6.X1.

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M6.X1 | Keyboard navigation: Ctrl+1..9 for tabs, Ctrl+F to the panel's filter, Enter runs the panel's command, Esc closes a confirm box (never confirms), arrow keys in tables, a visible focus ring (theme token); the catalog's `shortcut` honored where a command has one; the shortcuts listed in Help | M6.V3 | A shell-model test per binding; every interactive control reachable by Tab in the tour's order (checked on Start, Records, Ledger, Cockpit) |
| M6.X2 | Scaling: readable at 125 and 150 percent Windows scaling and at zoom 0.8 to 1.5 (Ctrl +, Ctrl -, Ctrl 0, saved in `ui.json`); no clipped labels or overlapping columns; the tour takes a `pixels_per_point` override so it captures each scale | M6.V5 | Tour images at 100, 125 and 150 percent for every tab, reviewed in M6.O4 |
| M6.X3 | Screen reader: eframe's `accesskit` feature on (platform adapter; the transitive dependency list and licenses in ADR 0007 before the merge); labels on every icon-only button; status components expose their word | M6.V4 | Narrator reads the Start tab, a Records row and the Ledger confirm box (owner check in M6.O4); `cargo xtask layers` passes |
| M6.X4 | First run: with no `ui.json` and no arguments, the Start tab shows a first-run card (what the window does, the read-only promise and the confirmed writes, the dev profile preselected when found, `Default` shown read-only and never preselected), `doctor` findings inline (read-only), remembered after the first load | M6.G1 (`doctor` allowed), M6.V5 | Shell-model tests for found, not found, and only `Default` present |
| M6.X5 | Help: "Using the workbench" (from `docs/workbench.md`), the shortcuts, and a "?" per panel opening its section | M6.X1 | The doc-sync test extended to the new sections |
| M6.X6 | The usability script for M6.O4: about 15 minutes of tasks (start, load the dev profile, find a weapon, diff a plugin, review an agent build, deploy (preview only unless the owner chooses), record a ledger row) at 125 and 150 percent, with what to note | M6.V5, M6.X2 | `docs/workbench.md` "Usability pass" section |

### Owner (lane O)

| ID | Task | Gates |
| -- | ---- | ----- |
| M6.O1 | Choose the visual direction from M6.V2's two mockups (or a mix, named per token group). **Decided 2026-10-16: Workbench** (owner, in chat, verbatim: "Workbench, themes look great only keep the mocks if there's a good reason."); `workbench` is the default preset, `studio` stays selectable, `classic` is marked legacy; the mockups were removed (no good reason to keep them: `docs/design/workbench-visual-system.md`, "Chosen direction") | M6.V3, so all of lane V after it; now unblocked |
| M6.O2 | Rule on fonts: keep the Windows system fonts (Segoe UI, Consolas; the default) or allow a compiled-in OFL font or an MIT/Apache icon font (each then listed in `ATTRIBUTION.md`). Row 7's BSL-1.0 ruling is still open beside it | M6.V4 icons (only if an icon font is wanted), M6.V3 type scale |
| M6.O3 | Accept, amend or reject ADR 0007 Amendment 3 (the cockpit write scope: `ledger`, `deploy --dev`, `deploy.rollback`, `game.session`, `game.probe`, `game.stop`). **Decided 2026-10-16: approved** as written (owner, in chat: "Approve Amendment 3 and ADR 0012, start lane V mockups"); ADR 0007 Amendment 3, "Owner ruling 2026-10-16" | M6.G3 to M6.G6 (not M6.G1, which enables no write); now unblocked |
| M6.O4 | Sign-off on the release build and `KitKraft Dev`: a walkthrough of the M6 exit criteria (`ROADMAP.md` "M6: Cockpit"), the S2 start check, and the V1 run from the window once M4.T3 exists (M6.G4). Optional, if the owner wants it: the same walkthrough at 125 and 150 percent Windows scaling (performance row U1) | M6 exit |
| M6.O5 | Confirm the dependency additions at their merges (`egui_extras` and image loaders, `glam` for M7.R1, which may merge during M6; the AccessKit adapter only if lane X's M6.X3 is picked up). Blocks only on a license outside the rule | M6.P4, M6.G5, M7.R1 merges |

### Dependencies on M5

- **Lane C (runtime; M4.T1 to T10, M5.C1, M5.C2):**
  - M6.G4 and M6.G5 need M4.T3 (`game.session` with `--start`, `game.status`,
    `game.probe`, `game.stop`) and M5.C1.
  - M6.G6 needs M5.C1 (`inconclusive` in the ledger format).
  - Real sessions need M4.O7 (the owner's installs).
  - M6's end-to-end exit row is M4.T4 (V1) started from the window.
  - Until then the buttons are disabled with the ticket id. Nothing in M6 starts a game.
- **Lane D:** M6.G7 reads M5.D1's matrix (`kit coverage --matrix`, landed). M6.P3b and
  M6.P5b are lane D engine changes, scheduled with lane D's M5 work so they do not
  collide with M5.D2 to M5.D7 in `handlers/`.
- **Engine core** (M4 lane F's area; no M5 letter, see "Lanes"): M6.P2b and M6.P6b.
- Nothing in M6 waits on lanes A, B, E or H, or on a CK sitting.

### Ordering

1. Start at once, in parallel: M6.V1 (first merge), M6.G1, M6.P1, M6.P6a, M6.P2b,
   M6.P6b; the owner can rule on M6.O2 now (M6.O3 was decided 2026-10-16).
2. After M6.V1: M6.V2 (then M6.O1), M6.P7, M6.P3a, M6.P4, M6.P5a, and **M7.R1**, the 3D
   placeholder (owner: "toss a 3D placeholder in there"), which merges read-only during
   M6 (M7 lane R).
3. After M6.O1: M6.V3, then M6.V4, then M6.V5 (which also frames M7.R1's panel when it
   has merged), then M6.V6.
4. After M6.G1: M6.G2 (with M6.V5), M6.G7 (with M6.V4), M6.G8 (with M6.V5). After
   M6.O3: M6.G3. M6.G4 to M6.G6 when lane C lands M4.T3 and M5.C1 (M6.G4 also needs
   M6.P2b).
5. M6.O4 last. Lane X (DEFER) only if picked up as P2.

Merge order: M6.V1 first. Then theme and token changes (lane V) before the panels that
use them. `gate.rs` changes (lane G) in their own commits. Engine requests through their
owning lane's review. 36 tickets (V 6, P 11 including 4 engine, G 8, X 6 deferred, O 5,
counted with the owner rows); 30 on M6's exit path.

### M6 is done when

The exit criteria of `ROADMAP.md` "M6: Cockpit", checked in `plan/STATUS.md` like M3's
and M4's.

## Not in M6

- Editing (`set`, `new`, `clone`, `place`, `build` output): M7, under ADR 0007 Amendment 4.
- The Creation Kit's layout, menus and dialogs: M7. The viewport is M7 lane R, but its
  placeholder M7.R1 may merge during M6, read-only.
- A game launcher, MO2 automation or Console API client in the UI; `testplan` (file
  output, ADR 0007 Amendment 3), `package` or `mod.run` as UI writes.
- Web front ends, new runtime hooks, Foothold content.

## M7: CK parity

Goal and exit criteria: `ROADMAP.md` "M7: CK parity". The Creation Kit's windows, menus,
dialogs, labels, workflows and keyboard shortcuts are reproduced from black-box
observation, so that a CK user is at home. Editing goes through the same catalog
commands, gated like M6. The Render Window is a 3D viewport, read-only, from its first
ticket, and that ticket (M7.R1, the owner's "3D placeholder") starts once M6.V1 lands and
may merge during M6. Exit criteria come from `docs/ck-parity.md` "Interface parity": one row per CK
window, menu item and dialog, with status `unresearched`, `observed`, `reproduced` or
`improved` and its observation source.

### Binding constraints

1. **Clean room for the interface** (ADR 0012, `plan/adr/0012-ck-interface-clean-room.md`,
   accepted by the owner 2026-10-16, M7.O1; AGENTS.md "Clean room" points to it):
   - Observe only: owner-assisted CK sittings, where a computer-use agent may drive the CK
     while the owner watches. Screenshots stay in `observe\` (gitignored).
   - Reproduce from the sittings: layout, labels, menu structure, field grouping, tab
     names, workflows and shortcuts. Describe each in our own words in
     `docs/ck-parity.md`.
   - Never copy CK icons, images, help text, resources or binaries.
   - Never read the CK's resource files to extract menus or strings.
   - The public CK wiki is a level-4 source for data facts (AGENTS.md "Clean room"), not
     interface observation: no interface row is filled from it.
   - All art is ours.
2. **Everything through catalog commands**, as in M6. The window gets the following only
   as engine command results, never by reading plugins or archives itself:
   - categories, cell contents, bounds and geometry (lane S tickets);
   - viewport geometry, through engine commands (M7.S4, M7.S5).

   `INTERNAL_ONLY` keeps `kitkraft-ui-egui` on `kitkraft-engine` alone. ADR 0002
   Amendment 1 (2026-10-16) makes the viewport L5 view code, retires the L2 `render`
   slot and keeps `crates/nif` behind the engine: there is no L5 -> L0 edge.
3. **Writes:** only ADR 0007 Amendment 4's list once the owner accepts it (M7.O4). Before
   that the dialogs are read-only. Each write has a preview and a confirm and is enforced
   by the gate test, as in M6.
4. **The viewport never writes** in M7. Materials, lighting and editing placements in the
   viewport are M8.
5. Out of scope unless an ADR says otherwise: geometry editing, navmesh generation or
   baking, terrain, materials. The CK menus that hold them are drawn, with the items
   disabled and the reason.

### Lanes (ownership)

| Lane | Name | Owns |
| ---- | ---- | ---- |
| K | CK interface observation | `docs/ck-parity.md` "Interface parity", the sitting scripts in `docs/ck-observations.md`, ADR 0012; shares `docs/ck-parity.md` with M5 lane A in small separate commits |
| M | menus and shell | the menu bar, window layout, Data dialog, Preferences, Warnings window, shortcuts (`crates/ui-egui`) |
| W | windows and dialogs | Object Window, Cell View, Use Info, the edit-dialog framework and layouts (`crates/ui-egui/src/dialogs/`, data files under `crates/ui-egui/dialogs/`) |
| E | editing | ADR 0007 Amendment 4, the write panels and their gate entries |
| R | render | the viewport panel (`crates/ui-egui/src/viewport/`), its camera and drawing; `docs/design/workbench-performance.md` "Viewport" |
| S | engine surface (requests to the owning lanes, by M5's letters) | `menu_path` and categories (engine core, as M6 "Lanes" defines it; category data through lane BX), `cell`, the `set` preview and `revert` (lane D), NIF bounds and geometry (lane B: `crates/nif`, `crates/ba2`, engine `handlers/mesh*`). M5's table gives `crates/nif` and `crates/ba2` to no lane; M7 assigns them to lane B, M5's format-crate lane (it owns `crates/pex`), as M4's lane G held `ba2`, `pex` and `strings` together |
| O | owner | sittings, rulings, sign-off |

### Lane K: CK interface observation

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M7.K1 | Draft ADR 0012, "CK interface parity, clean room" (proposed): what may be reproduced and from what (constraint 1), the evidence for each row (sitting id, screenshot names kept local, our own description), and how a reproduced element is checked (a tour image beside the observation). It also states that the public CK wiki (a level-4 source for data facts) is not interface observation, only the owner-assisted sittings are; and that M7.O5 settles the license of the NIF-format sources before M7.S4 starts | - | The ADR file, status Proposed; AGENTS.md "Clean room" gains a pointer only after M7.O1. **Done 2026-10-16**: the owner approved ADR 0012 before a draft file existed, so the docs lane wrote `plan/adr/0012-ck-interface-clean-room.md` from this description with status Accepted, and AGENTS.md "Clean room" gained the pointer |
| M7.K2 | Sitting scripts for the interface. Sitting 1: the main menu bar with every menu expanded, the Data dialog, Object Window, Cell View, Render Window camera controls (mouse and keys), Preferences, Warnings. Sitting 2: the edit dialogs of the first types (WEAP, AMMO, COBJ, LCTN, TERM, CONT, STAT, KYWD), every tab, group and field label. Combinable with the fixture sittings of STATUS rows 12 and 15 so the owner sits once | M7.K1 | `docs/ck-observations.md` gains the two scripts; each step names the `docs/ck-parity.md` rows it fills |
| M7.K3 | `docs/ck-parity.md` "Interface parity", seeded now with `unresearched` rows (done by the roadmap lane, 2026-10-16), then filled from sitting 1: one row per window, menu item and dialog, status `observed`, the sitting id and our own description; CK shortcuts and camera controls as rows; what it maps to (a catalog id, a UI element, or `gap:` with the reason) | M7.O2 | Every item sitting 1 saw has a row; no CK text beyond the labels themselves |
| M7.K4 | The same for sitting 2: per type, its tabs, groups and field labels, each field mapped to a schema member path (or `gap:` when the schema has no member) | M7.O3 | Rows for the eight types; unmapped fields listed in `docs/gaps.md` |
| M7.K5 | The parity check: a test (in `crates/ui-egui`, reading `docs/ck-parity.md` as the Help tab reads docs) that every `menu item` row maps to a catalog id that exists, or to `gap:`, and that every `reproduced` or `improved` row names the tour image or test that shows it | M7.M1, M7.K3 | The test fails on a row with an unknown id or no evidence |

### Lane M: menus and shell

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M7.M0 | Window model decision: CK-style floating or docked windows beside M6's cockpit tabs (a "CK layout" and a "Cockpit" workspace); egui's own `Window`s or a docking crate (`egui_dock`, listed in ADR 0007 if chosen); a short design note with the choice and why | M6.V5 | `docs/design/workbench-style.md` section; prototype screenshots of both workspaces |
| M7.M1 | The menu bar rendered from the catalog: menus from `menu_path`, in the CK's order (from K3; KitKraft-only menus such as Archive, Papyrus and Strings after the CK's), shortcuts shown; `Deferred` and `Planned` commands disabled with their reason; writing commands disabled with "not in the write scope" until Amendment 4 allows them; a menu item opens the panel or dialog for its command | M7.M0, M7.S1 | Shell-model test: every catalog command appears exactly once; the disabled reasons are shown; no menu item runs a command the gate would refuse |
| M7.M2 | The Data dialog over `load`: the plugin list with check boxes, "set as active file" (UI state: the target `into` for the M7 writes), a details pane (author, description, masters, dates; from `load` and `inspect` of `TES4`), layout and labels as observed | M7.M1, M7.K3 | Shell-model tests; `Default` still read-only; the active file is never an official master (refused in the UI and by the engine's output guard) |
| M7.M3 | Preferences: the UI preferences (theme, density, zoom, paths, shortcuts) laid out as the CK's dialog where the settings correspond; stored only in `ui.json` | M7.M1, M7.K3 | Rows in `docs/ck-parity.md` updated to `reproduced` or `improved` with tour images |
| M7.M4 | The Warnings window: `validate` findings and the `load` limitations as a CK-style list (severity, record, message), filter, double-click opens the record | M7.M1 | Shell-model test on canned findings |
| M7.M5 | Keyboard shortcuts as the CK binds them (from K3); clashes with M6's bindings resolved in a table in `docs/ck-parity.md`; unobserved shortcuts marked so | M7.K3 (M6.X1's bindings if lane X ran it; otherwise M7.M5 adds the binding table itself) | A test: every observed CK shortcut is bound or listed as a gap |

### Lane W: windows and dialogs

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M7.W1 | Object Window: a category tree (from M7.S2) and a virtualized list (EditorID, FormID, count, users), a filter box, double-click opens the edit dialog; read-only | M7.S2, M7.M1, M6.P4 | Shell-model test; T1 measured on the largest category |
| M7.W2 | Cell View: worldspace selector, the cell list (interior, and exterior with grid coordinates), the selected cell's references (from M7.S3); selection shared with the viewport (M7.R2) | M7.S3, M7.M1 | Shell-model test on canned `cell` envelopes |
| M7.W3 | The edit-dialog framework: a layout per record type as data (`crates/ui-egui/dialogs/<TYPE>.toml`: tabs, groups, labels, member paths, widget kind, the `docs/ck-parity.md` row it reproduces), rendered read-only from `inspect` and `schema --deep`; a type without a layout gets a generic one marked "unobserved" | M7.M0 | A test: every member path in every layout exists in the schema, and every layout cites a parity row |
| M7.W4 | Layouts for the eight types of sitting 2 | M7.W3, M7.K4 | The eight types open in their dialogs; parity rows `reproduced` with tour images |
| M7.W5 | The Use Info window (CK-style) over `refs --incoming` | M7.M1 | Shell-model test |

### Lane E: editing (ADR 0007 Amendment 4)

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M7.E1 | Draft ADR 0007 Amendment 4 (proposed): the editor's write scope. `set`, `new`, `clone`, `place`, `build` (output) and `revert` (M7.S7), only into the active file, never an official master, never under the game's Data folder or MO2 (the engine's output guard), each with a preview and a confirmed request built by one dialog file; the disjointness test's scope constant extended. Also decided there: whether edits collect in the dialog and are written on the CK's OK or Save, or one `set` per OK | M6.G1 | The amendment drafted; M7.O4 can rule |
| M7.E2 | Field editing: the dialog's typed fields become editable; OK runs the `set` preview (M7.S6's dry run) and then `validate` on the result (the validation preview); confirm writes | M7.E1, M7.O4, M7.W4, M7.S6 | Gate tests; a shell-model test edits a field of a fixture and reads it back with `inspect` |
| M7.E3 | New and Duplicate in the Object Window through `new` and `clone`, with the CK's labels | M7.E1, M7.O4, M7.W1 | As M7.E2 |
| M7.E4 | Placement: from the Object Window into the Cell View's cell through `place` (position typed or taken from the viewport's cursor, read-only), preview and confirm | M7.E1, M7.O4, M7.W2, M7.R2 | As M7.E2; the placed reference shows in Cell View and the viewport after the write |
| M7.E5 | Build spec editing: a form for `build` specs (v3) with `build plan: true` as the preview (first opened in the gate as `GUARDED` with `plan` required, M4.W6's note) and the confirmed build | M7.E1, M7.O4 | Gate tests for the plan-only and the confirmed forms |
| M7.E6 | Undo the last edit, as far as the CK's Edit menu offers it (observed): the inverse is computed by the engine from its diff (M7.S7's `revert`), previewed with its `dry_run` and confirmed like any write | M7.E2, M7.S7 | A test: edit, undo, `diff` against the original shows no change |

### Lane R: render (the 3D viewport)

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M7.R1 | A GL viewport panel through eframe's glow backend (an egui `PaintCallback` with `eframe::glow`; no new framework): grid, axes, an orbit and a fly camera whose controls follow the CK's as sitting 1 observed them (unobserved controls marked so in `docs/ck-parity.md`); `glam` for the math, its license recorded in ADR 0007 Amendment 3's table before the merge; chrome from the `theme.rs` tokens that exist when it merges, the themed frame applied later by M6.V5 (lane V) | M6.V1 only (it waits on neither M6.O1 nor the sittings: controls not yet observed are marked so) | Camera math unit-tested without a window; tour image; V1 measured; `cargo xtask layers` passes. May merge during M6, read-only (grid, axes and camera; no game data); the same commit removes `kitkraft-render` from xtask's `LAYER_TABLE`, the AGENTS.md layer table and `docs/architecture.md` (ADR 0002 Amendment 1) |
| M7.R2 | Placed references of the selected cell as oriented markers from the `REFR`'s position, rotation and scale (from M7.S3), colored by base record type with a shape per type as well (not color-only); click to select, synced with the Cell View list | M7.R1, M7.S3, M7.W2 | Picking unit-tested; V2 measured; selection sync tested in the shell model |
| M7.R3 | Bounding boxes from the base object's NIF (a bound stored in the file, or computed from vertices; which one, per M7.S4's sources) through the engine's `mesh.bounds`; no materials | M7.R2, M7.S4 | Boxes for a cell of the first POI; V3 measured; a base without a readable bound shows as a limitation, not as a box |
| M7.R4 | Wireframe, then flat-shaded meshes from the geometry `mesh.geometry` returns (loose and packed assets both, through the engine's archive access), under a loading budget; each skipped mesh is a limitation line | M7.R3, M7.S5 | V4 and V5 measured; a test on a canned geometry envelope; nothing read by the window itself |
| M7.R5 | Mesh loading performance: measure V1 to V5 first, then a geometry cache per base object, a per-frame upload budget, and level of detail or skipping for distant references | M7.R4 | `docs/design/workbench-performance.md` "Viewport" rows with before and after |

### Lane S: engine surface (owned by the lanes named)

| ID | Task | Depends on | Done when |
| -- | ---- | ---------- | --------- |
| M7.S1 | *(engine core)* `menu_path` of every catalog command aligned with the CK's menus where a parity row maps it (KitKraft-only commands keep their own menus); `docs/ck-parity.md` cited in the commit | M7.K3 | The catalog test that every command has a `menu_path` still passes; M7.K5 passes |
| M7.S2 | *(engine core; the category data through lane BX)* The Object Window category of every record type as data (from parity rows; unobserved types in "Other", marked so), returned by `schema` | M7.K3 | A test: every defined type has one category; the categories' order matches the parity rows |
| M7.S3 | *(lane D)* A read-only `cell` command: worldspaces, cells (interior and exterior with grid coordinates) and the references of one cell (base, EditorID, position, rotation, scale, flags), paged, with evidence like `query` | - | Shape test; CLI and MCP; tested on the first POI's cell and an exterior cell of `Starfield.esm` |
| M7.S4 | *(lane B, with M7.O5)* NIF sources and bounds: the source decision for Starfield NIF and geometry layout (graded per ADR 0005, recorded in `docs/source-registry.md`), then `kitkraft-nif` reads a model's bound, and an engine `mesh.bounds` command finds the base object's model and reads it from loose files or a BA2 | M7.O5 | Tests on corpus models (skipped without the game); unknown layouts reported as unknown |
| M7.S5 | *(lane B)* Geometry: `mesh.geometry` returns positions, indices and normals for a model (Starfield may keep geometry outside the `.nif`; S4's sources settle where), from loose files or BA2s, with a size budget and a limitation per skipped mesh. Encoding (director decision 2026-10-16): base64 little-endian `f32` (positions, normals) and `u32` (indices) buffers inside the JSON envelope, produced by the engine so the CLI and MCP get identical results (about 1.33x the raw bytes) | M7.S4 | Tests on corpus models; the envelope size and time measured as V4; only if V4 fails with this encoding, an ADR for an engine-produced side channel comes before any workaround |
| M7.S6 | *(lane D)* A `set` preview: `dry_run: true` returns the record before and after, the member paths changed and the `validate` findings on the result, and writes nothing (`set` has no `dry_run` today) | - | Shape test; CLI and MCP; a dry run leaves `into` byte-identical, or absent when it did not exist |
| M7.S7 | *(lane D)* An engine `revert` command (catalog: writing): the inverse of one edit to the active file, computed from `diff` against the file's state before the edit, with `dry_run: true` as its preview | M7.S6 | A test: `set`, then `revert`, then `diff` against the original shows no change; CLI and MCP |

### Owner (lane O)

| ID | Task | Gates |
| -- | ---- | ----- |
| M7.O1 | Rule on ADR 0012 (the interface clean-room rule). **Decided 2026-10-16: approved** (owner, in chat: "Approve Amendment 3 and ADR 0012, start lane V mockups"); `plan/adr/0012-ck-interface-clean-room.md` | M7.K2 onward; now unblocked |
| M7.O2 | CK interface sitting 1, owner-assisted (a computer-use agent may drive while the owner watches); can share a sitting with STATUS rows 12 and 15 | M7.K3, so lanes M, W, S1, S2 |
| M7.O3 | CK interface sitting 2 (the edit dialogs of the first eight types) | M7.K4, M7.W4 |
| M7.O4 | Accept, amend or reject ADR 0007 Amendment 4 (the editor write scope) | M7.E2 to M7.E6 |
| M7.O5 | Rule on the sources for Starfield NIF and geometry layout (which documentation or tools, under which license category; ADR 0004 and ADR 0006 apply by analogy) | M7.S4, so M7.R3 to M7.R5 |
| M7.O6 | A/B sign-off: the same tasks in the CK and in KitKraft (find a form, edit a field, duplicate, place, save), and the CK open and re-export of the result (the M7 exit row) | M7 exit |

### Dependencies

- On M6: the theme and panels (M6.V3, M6.V5), virtualized tables (M6.P4) and the gate
  mechanics (M6.G1). The game-runtime exit row runs through M6's cockpit.
- On M5: lane A's CK differential (M5.A2) for the `ck_reexport` exit row, and the
  owner-assisted sittings it already schedules.
- Lane R starts with M7.R1 as soon as M6.V1 lands and may merge it during M6; it waits
  on neither the sittings nor the visual direction (M6.O1).

40 tickets (K 5, M 6, W 5, E 6, R 5, S 7, O 6, counted with the owner rows).

### M7 is done when

The exit criteria of `ROADMAP.md` "M7: CK parity", read off `docs/ck-parity.md`
"Interface parity" and checked in `plan/STATUS.md`.

## M8 (sketch): Beyond the CK

Goals: `ROADMAP.md` "M8: Beyond the CK". Example tickets, not a plan. Each write needs a
further ADR 0007 amendment, and the agent panel needs an ADR for the MCP journal.

| ID | Task (example) |
| -- | -------------- |
| M8.1 | Search everywhere: one box over EditorIDs, FormIDs, names, text and script names, through `query` and a new engine search index, with results grouped by type |
| M8.2 | Inline validation and conflicts: every record view shows its `validate` findings and its override chain (`conflicts`) without leaving the dialog |
| M8.3 | An undo history panel computed from the engine's diff (every confirmed write in the session, each revertible with a preview) |
| M8.4 | The agent panel: what an MCP session ran and wrote, read from a journal the MCP server keeps (an ADR decides its place and format), with "open in the cockpit" per step |
| M8.5 | In-game probe results beside records: the last ledger and session rows for a form, shown in its dialog |
| M8.6 | Viewport materials, textures and lighting, after a source decision for Starfield materials |
| M8.7 | Snap-to-grid placement editing in the viewport through `place` (gated like M7.E4) |
| M8.8 | A multi-plugin load-order view: plugins as columns, forms as rows, winners and conflicts at a glance |
