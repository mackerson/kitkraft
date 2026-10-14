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
| M2.B1  | Provenance per member: `source` per field (ADR 0005 forms, plus any form M2.O1 approves) and `confidence = "confirmed" \| "corpus-consistent" \| "hypothesis"` | - | Loader rejects a missing or unknown value; `defs/README.md` documents it; existing defs migrated; `docs/record-parity.md` shows counts per confidence (M2.F9) |
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
| M2.O1 | **owner**: approve the ADR 0004 decision record, the matching ADR 0005 amendment, and `docs/xedit-reference.md` (drafted on a worktree branch at `b121751`, not on `main`). Until then option A applies | M2.B4..B8, M2.B10 |
| M2.O2 | **owner**: whether OMOD is in scope (ADR 0005 Amendment 1, decision 5) | M2.B7 |
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
