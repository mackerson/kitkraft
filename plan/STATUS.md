# Status

Updated: 2026-10-14

## Current milestone: M2 (agent surface)

M0 (scaffold) and M1 (data parity) are done. Every "M1 is done when" criterion in
`plan/execution-plan.md` holds: M1.6 roundtrips all 14 corpus files in `--release` and M1.7
passes (lane A); `kit roundtrip --verify` reports identical on all 14 (M1.13, rechecked by
`cargo xtask parity`); `docs/record-parity.md` is generated (M1.14); `cargo xtask ci` is
green. M2 is broken down into lanes and tasks in `plan/execution-plan.md`; the lane summary
is under "Next".

## Toolchain (verified on the dev box)

- rustc 1.99.0 (b940084d7 2026-09-28)
- cargo 1.99.0 (5f94df478 2026-08-27)
- clippy 0.1.99 (b940084d7e 2026-09-28)
- rustfmt 1.10.0-stable (b940084d7e 2026-09-28)
- MSVC 14.37 linker

## Done

- `PLAN.md` brief written (owner).
- Repository skeleton: workspace manifest, lints, 10 library crates
  (`schema esm ba2 pex strings nif doc io engine mcp`), `apps/kit-cli`, `xtask` with a real
  layer check, agent docs, ADRs 0001-0004, docs templates.
- Engine `CommandSpec` type and the first-cut catalog (all entries `Planned`).
- `corpus/manifest.toml`: the 14 official ESMs on this machine with sizes and SHA-256.
- M0.1 toolchain installed (versions above).
- M0.2 first build: `cargo build --workspace` succeeds; `cargo run -p kit-cli -- help` prints
  the catalog.
- M0.3 `cargo fmt --all` applied; `cargo xtask ci` green (layers, fmt, clippy `-D warnings`,
  tests). Fixes needed: three `clippy::manual_contains` hits (engine `find_command`, xtask
  banned-dependency checks). `clippy.toml` keys are all recognised by clippy 0.1.99; kept.
- Layer check verified to fail on a deliberate upward edge (`ba2 -> doc`) and on an unlisted
  same-layer edge (`esm -> strings`); both reverted. (An edge that forms a cycle, such as
  `schema -> engine`, is rejected by cargo itself before xtask runs.)
- Corpus manifest re-verified 2026-10-14: size and SHA-256 of all 14 ESMs match.
- M0.4 first commit on `main`.
- M1.8 (lane B) definition format: TOML loader + validator in `kitcraft-schema`, format in
  `crates/schema/defs/README.md`; `source`/`evidence` must use the ADR 0005 forms.
- M1.9 (lane B, rescoped) seed definitions: TES4 and the GRUP header typed from
  `uesp-skyrim` pages and confirmed on the corpus; GMST, GLOB, KYWD, WEAP as corpus
  inventories (order, cardinality, BFCB..BFCE and OBTE..STOP blocks), every payload opaque;
  WEAP layout pending ADR 0005 (ck-diff). Corpus findings: TES4 MAST has no DATA in any
  official ESM; GRUP type 10 occurs (unnamed, preserved).
- M1.10 (lane B) codegen: `kitcraft_schema::generate(&Schema) -> String`; golden tests for the
  seed set and an all-features fixture; the goldens are rustfmt-clean and compile under the
  workspace lints in `crates/schema/tests/generated.rs`, with `encode(decode(x)) == x` on
  well-formed, malformed and random record bodies.
- Lane A (merged on `main` at 6fcfeaf): M0.5 corpus test helper, M1.1 bounded reader,
  M1.2 parse model, M1.2b TES4 header inventory (`docs/corpus.md`), M1.3 writer, M1.4
  compressed records, M1.5 streaming driver, M1.6 corpus roundtrip, M1.7 hostile input.
  All 14 corpus files roundtrip byte-identical; `Starfield.esm` in 8.77 s with a peak
  working set of 1,238 MiB; zlib-rs at level 9 recompresses 99.88% of compressed records
  byte-identically (`docs/gaps.md`).
- M1.11 (lane C) `execute(id, args)`, the result envelope, and the shared command-line
  parser.
- M1.12 (lane C) `roundtrip` wired to the esm driver, `Status::Implemented`; result carries
  `identical`, counts, `elapsed_ms`, and on mismatch `{offset, expected, actual, path}` with
  evidence at the differing byte; `--out` writes the rebuilt plugin.
- M1.13 (lane C) `kit roundtrip <plugin> [--verify] [--out path] [--json]`: exit 0
  identical, 1 `--verify` mismatch (`data.passed: false`), 2 error. 2026-10-14, release
  build: `kit roundtrip --verify` reports identical and exits 0 on all 14 corpus files
  (M1 exit criterion 2), e.g. `Constellation.esm` 39,210 bytes / 57 records, `Starfield.esm`
  3,829,247 records in 3.1 s with a warm file cache.
- Lane C: minimal `inspect <form> <plugin>` (FormID or EditorID; header fields, subrecords
  with offsets and hex payloads capped at 256 bytes; `typed decode pending schema wiring`).
- Lane B round 2 (review findings 1-13): generated records carry `XXXX` per subrecord
  (`RawSub/OwnedSub.oversized`, typed values as `Sub<T>`); `Ctx.localized` and
  `LString::{Id, Text}`; `size_by_version` probed at every gate boundary, overlaps
  rejected; format adds `if_remaining`, `count_from`, `wstring`, `bstring`, shared
  definitions in `defs/common/` (`group_ref`, `type = "ref:Name"`; BfcbBlock and ObteBlock
  deduplicated) and headers without a signature (`RECORD.toml`, the 24-byte record header,
  confirmed by a header walk of 8 official ESMs); codegen emits `enum Record` +
  `RECORDS`, `DecodeError: Display + Error`, `[T; N]` fixed arrays, reserved prelude names;
  `builtin()` cached; `leaf_subrecords`, `known_signatures`, `Signature::from_bytes`;
  `examples/generated.rs` compiles the goldens outside `cfg(test)`; ADR 0005 Amendment 1
  (`uesp-starfield:`, `/CTDA` forbidden, `/Raw_Data` as inventory cross-check).
- M1.15 (lane C) `cargo xtask codegen [--check]`: `builtin()` -> `generate()` -> rustfmt
  (workspace `rustfmt.toml`) -> `crates/esm/src/generated/{mod.rs,records.rs}`, DO NOT EDIT
  header with the defs hash (FNV-1a 64 over `BUILTIN_DEFINITIONS`); `--check` fails on a
  stale, missing or foreign file and runs in `cargo xtask ci` after `layers`. esm exposes it
  as `pub mod generated` (not wired into the reader). `BUILTIN_DEFINITIONS` has no generated
  region, so `crates/schema/tests/defs.rs` stays its guard (see Requests).
- M1.14 (lane C) `coverage` command (`Implemented`): per record type, record and compressed
  counts and every subrecord signature with its count, classed `typed` / `opaque` /
  `unknown` against the schema; compressed records decompressed by default (Starfield.esm
  8.3 s, 2.4 s with `--no-decode`, release). `cargo xtask parity` runs `kit coverage` and
  `kit roundtrip --verify` over the corpus and writes `docs/record-parity.md` (idempotent,
  checked): 186 record types, 7,490,102 records, 136,741/136,741 compressed records
  examined, 50,419,561 subrecords of which 77 typed, 59,660 opaque, 50,359,824 unknown.
- AGENTS.md clean-room section points to ADR 0005 Amendment 1 (starfieldwiki.net);
  `docs/gaps.md` stale scaffold row removed.
- Lane A round 2 (review findings 1-8, `crates/esm`): the writer rejects a compressed flag
  that disagrees with the body (`FlagBodyMismatch`); decoded compressed records keep their
  inflated bytes and re-emit the original stream when touched without a change;
  `DecodeLimits::max_subrecords` (1,000,000) with a count-then-parse decode into one shared
  buffer (an 8 KB bomb of 1.4 M empty subrecords fails before any is built); two-pass
  writer (`Entry::encoded_len`) that streams modified groups, and
  `driver::rewrite_file` (edit `Starfield.esm`'s `WRLD` in 2.6 s at a 1,293 MiB peak, no
  group buffered); `HEDR` count documented from the corpus, `Tes4::set_record_count`, kept
  verbatim unless records/groups are added or removed (then the 12-file meaning);
  `esm::bridge` (`RawSub`/`OwnedSub`/`Ctx` from the generated module, `Record::raw_subs`,
  `Subrecord::from_owned`, `Plugin::ctx`, `Record::typed`/`set_typed`); fast corpus tests add
  `SFBGS004.esm`/`SFBGS047.esm` (real compressed records and `XXXX` in every gate) and check
  manifest SHA-256; absolute subrecord offsets, reserved `XXXX` signature, depth cap 64, one
  hostile harness, `rust-version = "1.88"`. Full corpus run at 087ab97: 14/14 identical,
  15.4 s, peak 1,379 MiB. Corpus `XXXX` inventory: 0 of 12,849 have a payload <= 0xFFFF.
- Source policy (owner decision 2026-10-14): ADR 0004 **decided**: upstream xEdit
  `Core/wbDefinitionsSF1.pas` pinned at `9fb016884bec138ea6c7b872cec831537d464c3e` is the
  record-definition reference, source material only. `docs/xedit-reference.md` records the
  verification (default branch `dev-4.1.6`, why no tag is pinned, MPL-2.0, all 18 lane B
  record types defined, line spans, Pascal-to-TOML map). ADR 0005 Amendment 2 (four-level
  hierarchy, SF1Edit differential comparison, Skyrim container-only), ADR 0001 Amendment 1
  (clean room), AGENTS.md, PLAN.md, README, NOTICE, ATTRIBUTION updated. Schema loader:
  `xedit:` sources (pinned SHA + line range) and `confidence`
  (`reference`/`corpus`/`ck-diff`/`verified`; required for `xedit:` citations, optional
  elsewhere, always backed by matching citations). `*.pas` gitignored.
- **Owner approval 2026-10-14** (M2.O1, M2.O2, open decision 2b): the owner approved the
  ADR 0004 decision, `docs/xedit-reference.md`, and 2b: `Core/wbDefinitionsCommon.pas` may
  be cited at the same SHA under the same rules. Six clarifications recorded in ADR 0004
  ("Owner approval 2026-10-14"): the common file is authorized (no blind import of
  Skyrim-specific readings); further helper files need only a documented dependency;
  confidence semantics (`reference` allowed source, `corpus` local fixture, `ck-diff` CK
  output, `verified` independent validation); lossless first; raw vs master-aware resolved
  FormIDs, never guess an unavailable master's type; scope discipline. ADR 0004 rule text:
  the loader accepts both files, same SHA. OMOD is in scope (M2.O2). ADR 0005 Amendment 3
  adopts the owner's dossier (`docs/research/starfield-dossier-2026-10-14.md`) as an
  evidence backlog, not a scope change: `docs/source-registry.md` (every source with our
  status), evidence grades A-D alongside `confidence`, the general source rule, and the
  conflict-register entries that matter to us. `docs/xedit-reference.md` gains a
  `wbDefinitionsCommon.pas` section (blob `cfa80ba0...`, 338,167 bytes, 10,348 lines;
  conditions, script data and base form components turn out to be defined in the SF1 file
  itself; the common file supplies object bounds, keywords and callbacks).

- Lane F (M2.F1..F6, F8): `Engine` owns a `Session` (`SessionHandler`s registered in
  `handlers::session_defaults()`; typed slots; `Session::load_order` listing); stdio MCP
  server in `crates/mcp`, hand-written over `serde_json`, MCP 2026-07-28 plus the
  handshake revisions 2025-11-25..2024-11-05; tools `execute`, `command_line`,
  `inspect_record`, `query_records`, `diff_plugins`, `coverage`, `roundtrip` generated from
  the catalog; resources `kitcraft://commands`, `kitcraft://load-order`; argument and
  command errors as `isError` results, engine bugs `-32603` (`docs/mcp.md`); `kit mcp
  [--profile DIR]`; in-process pipe client test and a spawned-binary test (with
  `KITCRAFT_DATA_DIR`: `inspect_record` on `Constellation.esm` passes, `query_records`
  answers `not_implemented` until M2.D11); `Status::Deferred { reason, milestone }` and
  the `catalog_closure` test (every `Planned` command must be listed in `PLANNED_IN_M2`
  with its owning M2 task; nothing is deferred yet because every command has an M2 owner).

- Lane A, M2 (`crates/esm`): M2.A0 review round 3 follow-ups 1-10 (each with a
  regression test); M2.A1 record index (`Starfield.esm` in 3.3 s release, 288 MiB peak;
  counts equal the streaming parse on all 14 files); M2.A2 random access (1,000 seeded
  `Starfield.esm` entries and every small-file record equal the streaming parse); M2.A5
  plugin builder and streaming upsert (replacing one record of each small file changes
  only that record and its ancestors' size fields; top-level group order checked against
  all 14 files); M2.A6 record edit API (set, insert, remove; edited compressed corpus
  records decompress to the edits, every other record keeps its bytes); M2.A3 typed decode
  check in the driver (all 14 files: 13,168 typed records, 0 conflicts); M2.A4 `XXXX`
  writer policy in `docs/corpus.md`. TES4 flag constants named from the pinned xEdit
  definitions. Full release roundtrip at b27d0dc: 14/14 identical, 16.7 s, 1,394 MiB.

## In progress

- **M2.F7 (owner check pending).** `.mcp.json.example` and "Use from Claude Code" in
  `docs/mcp.md` are in; an agent cannot restart Claude Code, so the owner runs:
  1. `cargo build --release -p kit-cli`
  2. `claude mcp add --scope local kitcraft -- C:\Modding\kitcraft\target\release\kit.exe mcp`
     (or copy `.mcp.json.example` to `.mcp.json`, which is gitignored, and approve it).
  3. New Claude Code session: `/mcp` lists `kitcraft` connected, 7 tools.
  4. Ask it to call `mcp__kitcraft__coverage` on `<Data>\Constellation.esm`, then
     `mcp__kitcraft__execute` with `{command: "roundtrip", args: {plugin, verify: true}}`,
     then `mcp__kitcraft__inspect_record` on a form `coverage` names (`first_record`), then
     `mcp__kitcraft__query_records {type: "WEAP"}`; each returns an envelope
     (`query_records` returns a `not_implemented` tool error until M2.D11, and
     `--profile` preloading needs M2.D8).
  5. Paste a transcript excerpt here. Headless alternative: `claude -p "..." --mcp-config
     .mcp.json --allowedTools "mcp__kitcraft__*"`.
- **M2.F9** waits for M2.A3 and M2.B1 on `main` (neither is there at fdd254e).
- **Lane B round 3** (M2.B4..B8, M2.B10): gate lifted by the owner approval of
  2026-10-14; in progress in lane B's current worktree. OMOD (M2.B7) is in scope.

## Next

M2 lanes (from `PLAN.md` "M2 - agent surface" and the step 2 ladder). Owners are assigned
when M2 is broken down in `plan/execution-plan.md`.

| Lane       | Scope                                                                                                          | Where                                        |
| ---------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| doc        | Load order, FormID resolution across masters, overrides and conflicts ("which plugin wins")                    | `crates/doc`                                 |
| io         | MO2 profile reader: `plugins.txt`, mod list, Creations manifests                                               | `crates/io`                                  |
| mcp        | stdio MCP server over `execute()` (`execute`, `command_line`, ...); `kit mcp` launcher                         | `crates/mcp`, `apps/kit-cli`                 |
| ports      | `ba2` (`ba2x.py`), `pex` (`pexdis.py`), `strings` ported from `C:\Modding\scripts`                             | `crates/ba2`, `crates/pex`, `crates/strings` |
| validate   | Rules: missing masters, bad links, localized FULL copies, stripped FULL/NTRM, god-mode pex calls               | `crates/engine`                              |
| edit       | `set`, `query`, `diff` over load order and typed records (needs generated records wired in esm)                | `crates/engine`                              |
| smoke test | Rename "Old Earth Hunting Rifle" to "Old Soviet Marksman Rifle": `kit set` -> `kit validate` -> launch via MO2 | all                                          |
| container  | `roundtrip --decode`; typed edits through `Record::set_typed` (decode side wired: `Record::typed`)             | `crates/esm` (lane A)                        |
| schema     | Round 3 (in progress): typed layouts from the pinned xEdit files (see below)                                   | `crates/schema` (lane B)                     |

Lane B round 3 transcribes definitions from the pinned xEdit files (`xedit:` provenance,
`confidence = "reference"` until the corpus confirms) in this priority: **WEAP, AMMO, PROJ,
AMDL, OMOD, COBJ, ZOOM**; then **GMST, GLOB, KYWD** typed; then **LVLI, FLST, INNR**. Each
definition is checked against every corpus occurrence before it moves to
`confidence = "corpus"`; unions stay opaque.

## Blockers

- None.

## Open owner decisions

| #   | Decision                                | Default until decided             |
| --- | --------------------------------------- | --------------------------------- |
| 1   | Project name                            | `KitCraft` / `kitcraft-*` / `kit` |
| 2   | xEdit MPL source policy (ADR 0004)      | **decided 2026-10-14** (pinned xEdit SF1 definitions as reference) |
| 2a  | Review `docs/xedit-reference.md` and ADRs 0004/0005/0001 before lane B round 3 | **approved 2026-10-14** (M2.O1; lane B unpaused) |
| 2b  | Whether `wbDefinitionsCommon.pas` (helpers the SF1 file uses) may be cited too | **approved 2026-10-14**: yes, same SHA, same rules (ADR 0004 owner approval) |
| 3   | Video critique used as overhaul brief   | none; overhaul work is after M2   |
| 4   | License                                 | MIT OR Apache-2.0 (scaffolded)    |
| 5   | Contact kinggath about SS2 now or later | later                             |
| 6   | Git remote / publishing                 | local only                        |

### M2 owner tasks (lane O, `plan/execution-plan.md`)

| #     | Task                                                        | State                                  |
| ----- | ----------------------------------------------------------- | -------------------------------------- |
| M2.O1 | Approve ADR 0004, ADR 0005 Amendment 2, `docs/xedit-reference.md` | **done 2026-10-14** (with six clarifications) |
| M2.O2 | Whether OMOD is in scope                                    | **done 2026-10-14**: in scope (M2.B7)  |
| M2.O3 | Install SF1Edit (agents do not download executables)        | open; blocks M2.B11                    |
| M2.O4 | CK and MO2 observation sessions                             | open; blocks M2.D1 check, M2.D3 (c), M2.E3 `ck-diff` part |
| M2.O5 | Third-party mod plugins as inventory evidence               | open; default **no**                   |
| M2.O6 | In-game check of the rename plugin                          | open; at M2 exit                       |

### Divergence from the brief (for the owner)

The agents' docs diverge from `PLAN.md` in two places, now reconciled in `PLAN.md` ("AGENTS.md
wins") but still the owner's to confirm:

- **xEdit stance.** Resolved 2026-10-14: the owner decided ADR 0004 (pinned
  `wbDefinitionsSF1.pas` as the record-definition reference); `PLAN.md` and AGENTS.md agree.
- **No push.** The brief said "push to main"; AGENTS.md says commit to local `main`, no
  remote (owner decision 6).

## Requests (cross-lane)

Agents append here when they need a change in a crate they do not own.

- **Lane A -> lane B (answer to the RawSub/XXXX request; re-checked after lane B round 2).**
  `esm::bridge` re-exports the generated `RawSub`/`OwnedSub`/`Ctx` (checked against `main`
  at dcfe253, no local copies): `From<&Subrecord>` sets `oversized` from the `XXXX` prefix,
  `Subrecord::from_owned` writes `XXXX` when `oversized` is set or the payload exceeds
  `u16::MAX`, `Plugin::ctx(record)` passes the record's form version and TES4 `0x80`. The
  `XXXX` inventory is done: 0 of 12,849 have a payload <= 0xFFFF (smallest 65,536), so the
  `docs/gaps.md` row is closed. If the generated shapes change again, `bridge.rs` is the
  one file to adjust.
- **Lane A -> lane C.** The decode side of the typed wiring is in esm: `Plugin::typed(record)`
  / `Record::typed(ctx)` (`None` for a type without a definition or an undecoded compressed
  record: call `Record::decompress` first) and `Record::set_typed`. `coverage` and `inspect`
  can now decode payloads instead of classifying by signature. API changes from round 2
  that engine code may meet: `Body::Compressed` has a third field `inflated` (patterns need
  `..`), `DecodeLimits` has `max_subrecords` (build it with `..DecodeLimits::default()`),
  new `Error` variants (`FlagBodyMismatch`, `ReservedSignature`, `TooManySubrecords`), and
  `driver::rewrite_file` for streaming edits (`RewriteReport::hedr_fix` when records or
  groups are added or removed).
- ~~Lane B -> owner / AGENTS.md maintainer: point AGENTS.md at ADR 0005 Amendment 1.~~
  Done 2026-10-14: AGENTS.md "Clean room" now states the Amendment 2 hierarchy, with UESP's
  Starfield pages at level 4.
- **Policy -> lane B.** `confidence` is validated by the loader but not stored in the model
  (the policy change was limited to `load.rs`). Add it to `RecordDef`/`HeaderDef`/`CommonDef`
  so coverage can report it, and consider re-exporting the pinned revision for tools.
  Then add `confidence = "corpus"` to `TES4`, `GRUP`, `RECORD` (their `corpus:` evidence
  already claims it), make it required for `uesp-skyrim:` citations (ADR 0005 Amendment 2),
  and rerun `cargo xtask codegen` (the defs hash changes). Not done in the policy change
  because it would have touched `crates/esm/src/generated/`.
- **Policy -> lane C.** `coverage` / `cargo xtask parity` should show `confidence` next to
  the source kind once the model carries it; `confidence = "reference"` values should surface as
  a `limitations[]` entry in `inspect`/`query` results.
- **Policy -> lane B (owner approval 2026-10-14).** Accept `wbDefinitionsCommon.pas` in
  `xedit:` citations: add it to `XEDIT_FILES` in `crates/schema/src/load.rs` (same pinned
  SHA), update the error text, `crates/schema/tests/loader.rs` (a positive case for the
  common file, the existing negative cases for other files) and `crates/schema/tests/defs.rs`,
  and `crates/schema/defs/README.md`.
- **Policy -> AGENTS.md / plan maintainer.** AGENTS.md "Clean room" ("Other xEdit files are
  not covered yet") and owner decision 2 ("extending the reference beyond
  `wbDefinitionsSF1.pas`"), ADR 0001 Amendment 1 (only the SF1 file), and
  `plan/execution-plan.md` M2.O1/M2.O2 should reflect the approval (the common file at the
  same SHA; O1 and O2 done). Also: M2.B1 names confidence values
  `confirmed`/`corpus-consistent`/`hypothesis`, while ADR 0004 and the loader use
  `reference`/`corpus`/`ck-diff`/`verified`; the ADR wins.
- **Policy -> M2.** Install SF1Edit (matching the pin if possible, else record the build) for
  the differential comparison in ADR 0005 Amendment 2 §2a; never bundle it.
- **Lane B -> owner (default chosen).** Inventory definitions name members after their
  signature (`Edid`, `Wfir`, `Flld2`); renaming them once meanings are sourced (`ck-wiki`)
  is a breaking change for `kit query`. Default: accept the rename when it happens.
- **Lane F -> lanes D, E, G (session and catalog closure).** A handler that needs the
  load order is a `fn(&mut Session, &Value) -> Result<Outcome, EngineError>` added to
  `handlers::session_defaults()` (stateless handlers stay in `defaults()`). Keep your
  typed state in a slot (`Session::insert` / `get` / `get_mut`); `load` also sets
  `Session::set_load_order(Some(listing))`, which `kitcraft://load-order` serves as is.
  When you flip a command to `Implemented`, removing its `PLANNED_IN_M2` line in
  `crates/engine/src/lib.rs` is optional (a stale entry only prints); a command you will
  not finish in M2 becomes `Status::Deferred { reason, milestone: "M3" }`.
- **Lane C -> lane B.** `cargo xtask codegen` can own `BUILTIN_DEFINITIONS` once
  `crates/schema/src/lib.rs` marks a generated region for it (for example
  `// BEGIN generated by cargo xtask codegen` / `// END`); until then
  `tests/defs.rs` is the guard and xtask leaves the file alone.
- **Lane A -> lane D (M2.A1, M2.A2 landed).** `kitcraft_esm::index`:
  `index_file(&Path, &IndexOptions) -> Result<PluginIndex>` (also `index_bytes`,
  `index_reader`). `PluginIndex { file_len, header: Record<'static> /* TES4, parsed */,
  records: Vec<RecordLoc> /* file order, TES4 first */, groups: Vec<GroupLoc> }`;
  `RecordLoc { signature, form_id /* as stored */, flags, form_version, offset, data_size,
  group: Option<u32> /* innermost group, index into groups */, edid: Option<Box<[u8]>> }`;
  `GroupLoc { header: GroupHeader, offset, size, parent: Option<u32>, depth }`;
  `PluginIndex::group_path(Option<u32>)`, `group_headers(&RecordLoc)`. Masters and flags:
  `tes4::Tes4::new(&index.header)`. Random access: `read_record_at(&mut R: Read+Seek,
  offset, Option<&DecodeLimits>) -> Result<Record<'static>>` and `read_indexed(&mut R,
  &RecordLoc, ...)` (header checked against the entry: `Error::IndexMismatch`; a group
  offset is `Error::NotARecord`). `INDEX_FORMAT_VERSION` (1) for the M2.D5 cache key. The
  index is not serializable yet (no serde in esm); say if D5 wants a byte format from
  esm instead of its own.
- **Lane A -> lane E (M2.A5 landed; review follow-ups 1, 3, 8, 9 in).**
  `kitcraft_esm::build`: `Plugin::create(&HeaderSpec)`; `HeaderSpec::new(masters)` fills
  corpus-observed defaults (flags 0, form version 582, `HEDR` 0.96, next object ID
  0xFF001000) that M2.E3 should confirm or change; masters are written exactly as given
  (`MAST` + 8-byte zero `DATA`), nothing inferred. `Plugin::upsert(&[GroupHeader],
  Record) -> Result<Upsert /* Replaced | Inserted */>`: the path is
  `PluginIndex::group_headers(&loc)` of the source record; a record under cell or world
  children needs its parent CELL/WRLD record upserted first (a missing children group goes
  right after the record whose FormID is its label, else at the end). Streaming:
  `upsert_file_to_path(in, out, &RewriteOptions, Vec<Placed>, |tes4| ...)` (temp file +
  rename, `HEDR` patched, `set_masters` for the header edit), `upsert_reader` for a
  `Read + Seek` input. `driver::rewrite_file_to_path` applies the `HEDR` fix itself.
  `Record::typed(ctx) -> Result<TypedRecord, NotTyped>` (`NoDefinition` | `NotDecoded`);
  `Plugin::typed(&record, &DecodeLimits) -> Result<Option<TypedRecord>>` decodes
  compressed records on the fly. `Subrecord::from_owned` / `set_typed` choose `XXXX` by
  size; unchanged subrecords keep their bytes. FormIDs are copied as stored; remapping is
  E4's (via B3). M2.A6 edit API (decodes a compressed record on first edit, recompresses
  only if changed): `Record::find_subrecord(sig, nth) -> Option<usize>`,
  `set_subrecord(index, data, &DecodeLimits) -> Result<bool /* changed */>`,
  `insert_subrecord(index, Subrecord, &limits)`, `remove_subrecord(index, &limits)`,
  `edit_subrecords(&limits) -> Result<&mut Vec<Subrecord>>`; out-of-range is
  `Error::NoSuchSubrecord`.
- **Lane A -> lanes D, F (M2.A3 landed).** Typed decode check: `Record::check_typed(ctx)` /
  `bridge::check_subrecords(sig, &subs, ctx) -> Result<TypedCheck, NotTyped>`
  (`subrecords`, `placed`, `conflicts: Vec<TypedConflict { index, signature, offset,
  kind: NotDecoded | Reencoded }>`); `RoundtripOptions { typed: true, .. }` fills
  `Stats::{typed_records, typed_subrecords, typed_unchecked, typed_failed_records,
  typed_conflicts, typed_conflict_list}` for `coverage` and `cargo xtask parity` (M2.F9).
  On all 14 files today: 13,168 typed records, 0 conflicts (`docs/corpus.md`).
- **Lane A -> lane B.** Please generate an accessor for the raw subrecords a typed record
  kept: `generated::records::Record::extra(&self) -> &[(usize, OwnedSub)]`. Until then
  `esm::bridge::check_subrecords` matches the five variants generated today to find
  payloads that did not decode, and cannot see them for a new record type (it still
  catches re-encoding differences for every type).
