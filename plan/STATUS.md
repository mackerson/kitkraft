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
  handshake revisions 2025-11-25 and 2025-06-18 (the batch-era 2025-03-26 and 2024-11-05
  negotiate to 2025-11-25; every served version is in `server/discover` and `-32022`;
  `ping`/`initialize` are legacy-only); tools `execute`, `command_line`,
  `inspect_record`, `query_records`, `diff_plugins`, `coverage`, `roundtrip` generated from
  the catalog; resources `kitcraft://commands`, `kitcraft://load-order`; argument and
  command errors as `isError` results, engine bugs `-32603` (`docs/mcp.md`); `kit mcp
  [--profile DIR]`; in-process pipe client test and a spawned-binary test (with
  `KITCRAFT_DATA_DIR`: `inspect_record` on `Constellation.esm` passes, `query_records`
  answers `not_implemented` until M2.D11); `Status::Deferred { reason, milestone }` and
  the `catalog_closure` test (every `Planned` command must be listed in `PLANNED_IN_M2`
  with its owning M2 task; nothing is deferred yet because every command has an M2 owner).
  Review round 2 (Fable, no blockers) fixed: `roundtrip --out` opens with `create_new`
  unless `overwrite: true`, and `destructiveHint` is `true` on the tools that can write;
  `coverage` gained `signatures` and `top`, and MCP defaults them to `false`/`20`
  (`backend::MCP_DEFAULTS`; `Starfield.esm`: 18 KB instead of 695 KB, text leads with
  `data.summary`); `execute` reports `unknown_command` before other violations and takes
  `args: null` as `{}`; a `not_implemented` `--profile` preload stays out of the
  instructions; `docs/mcp.md` states that paths are not confined to `KITCRAFT_DATA_DIR`.
  **Round 3 (2026-10-14):** M2.F8 closed: every catalog command is `Implemented`,
  `PLANNED_IN_M2` is gone and `catalog_closure` fails on any `Planned` command; partial
  commands say so in `limitations` (inspect single-plugin mode, diff/conflicts compare
  stored bytes). `data_dir` defaults to `KITCRAFT_DATA_DIR` on every command that takes it
  (a profile prefers MO2's `gamePath`); `validate` searches a standalone plugin's masters in
  the load order, next to the plugin, then the Data dir, and resolves a bare plugin name
  there (`kit validate --plugin out\X.esm` now passes with only the variable set). M2.F9:
  `schema` command (`kit schema --type WEAP [--member KWDA] [--deep]`, alias `defs`, MCP
  tool `record_schema`): members with type, source citation and confidence, top level by
  default; `cargo xtask parity` adds a per-confidence table from it. Resources:
  `kitcraft://validate-rules` (rules, severities, denylist, docs); `kitcraft://commands`
  gains `cli` and `tools`. The flaky `kit_mcp_on_an_official_file`: spawned servers use a
  private `KITCRAFT_CACHE_DIR` under the test target dir (no more sharing the user's index
  cache with other worktrees' gates), stderr is drained on a thread, and an EOF reports the
  child's exit status and stderr; 20 consecutive runs pass (the original crash did not
  reproduce under 4-way parallel cold-cache stress either). AGENTS.md, ADR 0001 Amendment
  1 and the plan name both xEdit files at one SHA; README has the M2 status and a quick
  start.

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

- Lane B, M2 (`crates/schema`): **M2.B1** provenance on every node: `Provenance { source,
  evidence, confidence }` on records, headers, shared definitions, members and fields
  (members and fields may set their own keys or inherit); `confidence` required on every
  file (so on every `uesp-skyrim:` citation too), ADR 0004 values; TES4/GRUP/RECORD say
  `corpus`; loader accepts `wbDefinitionsCommon.pas` at the pin; `XEDIT_REVISION` and
  `XEDIT_FILES` public. **M2.B2** universal subrecords typed with corpus evidence (scan of
  all 14 ESMs: 265,346 EDID, 44,077 FULL and 16,434 DESC all 4 bytes, 110,383 KSIZ/KWDA
  pairs consistent, 180,802 of 180,804 KWDA FormIDs resolve to KYWD): `EditorId`, `Name`,
  `Description`, shared group `Keywords { Count, Forms }`; new inventory definitions for
  AMMO, AMDL, COBJ, OMOD, ZOOM. **M2.B3** generated `form_ids()`/`form_ids_mut()`/
  `lstrings()`/`lstrings_mut()` with member paths on every record and on `Record`, plus
  `Record::extra()`. **M2.B9** GMST `DATA` as a union by EditorID prefix (new format
  construct `type = "union"`, `decider = "edid-prefix"`: b/f/i/s/u), GLOB `FLTV` as `f32`;
  KYWD bodies stay opaque (no readable source; `docs/gaps.md`). Corpus check through the
  generated records: every AMDL (126), AMMO (154), COBJ (3,518), GLOB (3,319), GMST
  (2,418), KYWD (6,965), OMOD (3,330), TES4 (14), WEAP (452) and ZOOM (141) record decodes
  with an empty `extra` and re-encodes identical. `BUILTIN_DEFINITIONS` has a marked
  generated region (answers lane C's request).

- Lane G, M2.G0..G9 (`crates/ba2`, `crates/pex`, `crates/strings`; engine
  `handlers/{ba2,papyrus,strings}.rs`), all checked 2026-10-14:
  - G0: ADR 0006 (sources for BA2, `.pex`, string tables); `docs/source-registry.md` rows.
  - G1-G3 `ba2`: GNRL/DX10 v2/v3 reader, zlib and LZ4-block codecs, DDS header synthesis,
    deterministic packer. All 90 official archives list (1,763,906 entries; every
    name/dir hash recomputes; 65 GNRL v2, 10 DX10 v2, 15 DX10 v3 LZ4); 100 seeded entries
    per archive extract to their **declared sizes** (7,360 entries, 7.3 GB, 16 s release;
    this sample checks sizes, not bytes); every texture's chunks equal its mip chain
    (`docs/corpus.md` "Archives"). Byte identity with Archive2 rests on the manual
    differential run below (seven archives), not on the sample.
    **Archive2.exe 1.2.0.1 differential (manual run):** our extraction is byte-identical
    to Archive2's for every entry of `Starfield - Textures01` (5,574) and `Textures05`
    (7,084, all 16 DXGI formats, 25 cube maps), `SFBGS004 - Textures` (18), `OldMars -
    Textures` (42), `Starfield - Localization` (27), `Starfield - Misc` (4,753),
    `SFBGS004 - Main` (739).
    Sample hash: `strings/starfield_en.strings` (1,542,583 bytes) SHA-256
    `E18CADD47270CC7C0F37A33BDE9FE3B4049468CF94558379DA36356BB144951E` from ours, from
    `ba2x.py`, and from Archive2. Archive2 opens our packed archives (Localization strings as
    GNRL v2 and v3, OldMars textures as DX10 v2 and v3) and extracts every file
    byte-identical to the input.
  - G4: `ba2.list`, `ba2.extract`, `ba2.pack` `Implemented` (refuse writes under a game
    Data dir).
  - G5 `pex`: all 5,708 official scripts (Misc, SFBGS00D/047/050 Main, ShatteredSpace
    Main01) parse to their last byte; **text equals `pexdis.py`'s on all 5,575 scripts
    `pexdis.py` can read** (0 differ). `pexdis.py` fails on the other 133: it reads an
    extra `u16` after each struct name; the corpus wins (ADR 0006), so `crates/pex` reads
    structs without it.
  - G6 `papyrus.disasm`, G7 `papyrus.compile` `Implemented`. PapyrusCompiler 4.7.0.5 run
    on Scripts/Source from `Tools\ContentResources.zip`: DefaultAliasOnActivate,
    DefaultCounterQuestA and the owner's noc patch scripts nocOutfitChangerQuest,
    nocOutfitChangerContainer, SQ_Airlock_QuestScript, CS7_SpaceshipLandingQuestScript
    compile through `kit papyrus compile` and `kit papyrus disasm` equals `pexdis.py` on
    each (6 scripts). Never `-release`.
  - G8 `strings`: all 378 official tables (13 archives, 9 languages) parse and rewrite
    byte-identically. G9 `strings.dump`, `strings.merge` `Implemented`. Known string:
    `strings/starfield_en.strings` ID `0x0002E469` = "Old Earth Hunting Rifle", the FULL
    (`69E40200` at offset 21,546,047) of `Starfield.esm` WEAP `0021BBCD`
    (`RussianHuntingRifle`).
  - Review round 2 (2026-10-14), each with a regression test: DX10 chunks split by mip
    **area** (`w * h >= 512 * 512`; recomputes all 136,401 official chunk plans, the
    edge reading gets 421 wrong); `extract_vec(.., cap)` refuses oversized entries before
    decoding (64 MiB `.pex`, 256 MiB tables, `ba2.extract` `max_entry_bytes` default
    1 GiB); one LZ4 blob at most 256 MiB; the user Data folder (`My Games\Starfield\Data`)
    is refused like the game's; extraction refuses Windows-hostile names (trailing `.` or
    space, forbidden characters, CON/NUL/COM1/...) and case-folding or file/directory
    collisions while planning, before the first write; `pack` refuses non-ASCII names
    (`docs/gaps.md`), hash-triple collisions and empty files (Archive2 skips them;
    `ba2.pack` skips and reports them), DDS headers with zero dimensions or too many mips;
    `ba2.pack` `overwrite` (create-new by default); `strings.merge` writes temp + rename
    and reports in-table duplicate IDs (`duplicates`, first wins) apart from cross-table
    conflicts; `.pex` object size fields are checked (all 5,708 official scripts agree)
    and versions other than 3.12 are flagged. Not done (optional): a tolerant parse mode
    for padded string tables.

- Lane D, M2.D1..D13 (`crates/doc`, `crates/io`, `docs/load-order.md`; engine
  `handlers/{load,inspect,query,diff,conflicts}.rs`), all checked 2026-10-14:
  - D1 `docs/load-order.md`: every MO2 profile rule with its evidence (file and line, or
    nolvus.net / STEP / Ortham / LOOT by URL). modlist.txt's first line is the highest
    priority (docs plus a file check: both Free Lanes compatibility patches ship the same
    files as their bases and are listed first). **Still open (M2.O4):** MO2's own UI view of
    one such file.
  - D2 `io`: read-only profile reader, overlay (overwrite > mods by priority > Data,
    case-insensitive), plugin plan naming why each plugin is active (`Starfield.ccc`,
    hardcoded official masters, `plugins.txt`, MO2 unmanaged entries, BlueprintShips
    companions). Default profile: 84 active plugins, every one an existing file (70 from
    mods, 14 from Data), order is `loadorder.txt`'s entries under `hardcoded-official-slots`
    (review round 2 below), profile files unchanged.
  - D3 (a, b) corpus: raw FormIDs address master *classes* by scale (`FE` small, `FD`
    medium, else full; the index counts that class only; the file itself is the next index
    of its own class), confirmed on SFBGS047 (masters full, small, full: its overrides of
    the second full master are `01xxxxxx`). All 14 files resolve every header FormID; 15
    SFBGS050 overrides address SFBGS00D records that do not exist; largest own object ID
    per scale fits 12/16/24 bits. (c) flag names from the pinned xEdit file via the owner's
    dossier; runtime slots are the assumption `runtime-slots-by-scale` (stated in every
    result that shows a runtime ID).
  - D4 `doc::LoadOrder` (raw vs `GlobalForm`, master maps, missing masters are typed
    errors, late masters reported); D5 index cache (`%LOCALAPPDATA%\kitcraft\cache`, path +
    size + mtime + both format versions + checksum) and override table; index and random
    access now through lane A's `esm::index` (M2.A1/A2). Default profile: 7,739,193
    records, cold 6.0 s, warm 0.68 s (`kit load`, release). 27 third-party small/medium
    plugins store own records full-style: resolved by the named assumption
    `full-style-self` (Ortham's documented example; no official file does it); 4 records
    of `HG_ShipCargoHoldAtHome.esm` stay unresolved.
  - D6 conflict classes; Default profile: 15,586 overridden forms, 15,893 overrides
    (8,206 deleted, 6,031 differs, 1,641 identical to previous, 15 without a base), 2.8 s.
  - D7 string tables through lane G's readers (loose via the overlay, else
    `<stem> - *.ba2`): `Starfield.esm` WEAP `21BBCD` FULL resolves to "Old Earth Hunting
    Rifle" with the table offset as evidence (release corpus test).
  - D8 `load`, D9 `inspect` (every version, winner, classes, lstrings, schema-typed
    values), D10 `where` language (typed member paths through lane B's schema, 10,000
    random inputs), D11 `query` (`kit query --type WEAP --where "FULL ~ 'Old Earth Hunting
    Rifle'"`: the rifle, 0 unknown, 0.2 s warm in-process), D12 `diff` (corpus file vs a
    copy with one flipped byte: exactly that subrecord, offsets on both sides), D13
    `conflicts`: **esmscan.py run read-only on the Default profile (master Starfield.esm,
    83 plugins) and `kit conflicts --master Starfield.esm` agree on all 59 plugins and
    every per-type count (8,234 override records).** All `Implemented`, tested through
    `execute`, reachable over MCP (`execute`, `inspect_record`, `query_records`,
    `diff_plugins`; `load` fills `kitcraft://load-order`).
  - **Review round 2 (2026-10-14), all 13 findings fixed with regression tests:**
    (1) order by the named assumption `hardcoded-official-slots`: `Starfield.ccc` plugins,
    then official plugins without a `plugins.txt` entry (`ShatteredSpace`, `SFBGS004`,
    `SFBGS00D`, `SFBGS050`, `BlueprintShips-SFBGS050`, `SFBGS047`, which MO2's
    `loadorder.txt` lists after managed plugins naming them as masters), then the rest.
    Default profile: late master links 2 -> 0; 15 forms change winner, all from a slotted
    official plugin to a later managed mod (the reviewer's 15). Remaining late masters are
    an ordering error in every result's limitations. M2.O4 stays open for the game's own
    order. (2) `full-style-self` IDs masked to the plugin's scale (`01012345` in a medium
    file = its `2345`), `resolved_by` per version, examples in `load`, same-plugin
    collisions reported; Default profile: 63,798 records of 27 plugins, none masked, no
    collisions. (3) raw `00000000` is null (`ResolveError::Null`, `{"null": true}`, `where`
    `null`); formrefs refuse object 0. (4)/(5) the load order's limitations (failed or
    missing plugins, missing masters, ordering errors, assumptions, profile warnings) are
    shared by `load`, `inspect`, `query` and `conflicts`, implicit loads included. (6)
    `ResolveError::MasterNotLoaded` / `PluginNotLoaded`. (7) `deleted` on versions, results
    and the inspect summary; `where` never evaluates a deleted payload (unknown with the
    reason). (8) kind mismatches are unknown except union variants; `&` bit test
    (`@flags & 0x20`). (9) `inspect` bare hex needs `plugin`; runtime IDs take `runtime:`.
    (10) cache staleness rule documented; mtime 0 never cached; a content sample (size +
    first/last 64 KiB) catches same-size-and-mtime edits; cache format 3. (11) `diff` keeps
    the files' plugin spelling; raw fallback names the own-scale condition. (12) `@flags`
    without an index entry is unknown. (13) runtime slots follow the scale bits only (update
    and blueprint flags do not change the class), doc and code agree; no profile plugin sets
    0x200. New Default-profile numbers (release): `kit load` cold 4.7 s / peak working set
    482 MB, warm 0.73 s / 376 MB (reviewer: 478 / 375 MB); conflicts 15,586 forms, 15,893
    overrides (8,206 deleted, 6,028 differs, 1,644 identical, 15 first versions), 2.7 s;
    esmscan counts unchanged (8,234 records from Starfield.esm by 59 plugins).
- Lane E, M2.E1..E10 (engine `src/edit/`, `src/validate/`, `handlers/{set,validate,build}.rs`,
  the `FIELD=value` shorthand; `docs/validate.md`, `docs/smoke-rename.md`), all checked
  2026-10-14:
  - E1 shorthand: `kit set <form> FULL="..." --into X.esp`, `SIG.member=1.5`,
    `SIG=hex:...` give exactly the `--fields` JSON (values always strings; the command
    parses them by type). E2 field paths `SIG`, `SIG[n]`, `SIG.Member[n]` against lane B's
    schema (unions by EditorID prefix), typed values range-checked, `hex:` for opaque,
    `{mul}/{add}`, FormID 0 is `null`; errors name the path and a rule id.
  - E3 writing rules in `docs/corpus.md` ("Writing rules"), asserted by
    `crates/engine/tests/writing_rules.rs`: master flag for `.esm`, `HEDR` 0.96 / next ID
    `0xFF001000` / form version 582, never localized, string ID 0 has no text, **no ONAM for
    top-level overrides** (ONAM lists only group-type 7/9/10 children; 853 top-level
    overrides in 10 official files are never listed). The `ck-diff` half (M2.O4) is a
    limitation of `set` and `build`.
  - E4 patch writer: lossless copies (only edited subrecords change), every lstring of a
    localized source inlined from lane D's string tables (ID 0 -> empty; unresolved or
    non-UTF-8 refuses), masters = source masters + source (prefix-compatible targets
    extended; otherwise remapped through the M2.B3 visitors, refused when opaque), the
    cell/world/topic parent rule, never a load-order plugin / game Data / user Data / MO2.
  - E5 `set` across the load order (session or `profile`/`plugins`): winning version,
    before/after per field, evidence in source and patch. Until lane D's round 2 it refuses
    an unverified load order (failed plugin, late or missing master, active plugin without
    a file) and a deleted winner; scaled sources are refused (round 2).
  - E6-E8 `validate` (session-aware; `{plugin?, scripts?, rules?}`): `masters.missing`,
    `masters.order`, `formid.dangling` (header + typed members, scale-aware full slots),
    `lstring.copied-id`, `override.stripped-name`, `record.flags-body`,
    `papyrus.denylisted-call` (`Debug.SetGodMode`, from our noc patch). On the 14 official
    files (`validate_corpus_official_files`, release, 76 s): **0 errors**; warnings: 92
    `override.stripped-name` on CELL/LCTN/NPC_/QUST/WRLD (the DLCs drop those names
    themselves, so those types warn instead of erroring) and 15 `formid.dangling` in
    SFBGS050 (overrides of SFBGS00D forms SFBGS00D lacks: injected records, the same 15
    lane D found).
  - E9 `build` from TOML (`[[rule]]` over `query` matches, `[[set]]`, conflicts named by
    key, fresh plugin validated before it replaces `out`).
  - E10 rename: `C:\Modding\kitcraft\out\OldSovietMarksmanRifle.esm` (2,197 bytes; `TES4`
    master flag, `HEDR` 2 records, master `Starfield.esm`; one `WEAP 0021BBCD`
    `RussianHuntingRifle` with FULL "Old Soviet Marksman Rifle" and its 8 other lstrings
    inline), identical from `kit set` and `kit build docs\smoke-rename.toml`; `validate`
    0 errors / 0 warnings; `roundtrip --verify` identical; with the plugin appended
    `inspect` shows the new name winning. Automated in `crates/engine/tests/smoke_rename.rs`
    (64 s debug). Owner steps: M2.O6 below and `docs/smoke-rename.md`.
- Lane E round 2 (Fable review, 13 findings, each with a regression test), 2026-10-14:
**M2.O6 result (2026-10-14):** PASS. Owner loaded `OldSovietMarksmanRifle.esm` (SHA-256 9c0b28a6...) through MO2 on a disposable save, `player.additem 0021BBCD 1`, and the item displayed as "Old Soviet Marksman Rifle". First in-game confirmation of the set/validate/build pipeline and of inline UTF-8 lstrings in a non-localized override of a localized master.

  `build` guards `out` like any patch target (a load-order plugin named as `out` was
  overwritten) and needs `overwrite` for an existing file; `set` stages, runs the plugin
  rules and writes only when its records pass; both stage under the OS temp dir (stale
  dirs removed). Corpus lstring table (`edit::lstrings`, `docs/validate.md`: the xEdit
  file is not on this machine, so the corpus method; 185 types, 105 lstring signatures,
  checked by `lstring_table_matches_the_corpus`): untyped lstrings (ATTX, ITXT, SHRT, ...)
  are inlined, a non-zero 4-byte subrecord of unknown layout from a localized source
  refuses the copy unless `allow_untyped_lstrings`, `validate` warns on them in overrides
  of localized masters. Small/medium sources (or masters) refused; own-form dangling
  FormIDs are errors. New `TES4`: `CNAM` "DEFAULT" + `INCC` (interior cells, true in all
  14 files); `SNAM`/`BNAM`/`CHGL` a stated deviation. Non-ASCII inline text, struct-tail
  lstring edits, whole-vs-member build conflicts, exact 64-bit `add` / 2^53 `mul`,
  `inf`/`nan`/`1e400` range errors, hex strings NUL-terminated, evidence before rename,
  `callmethod` class matching. `validate_corpus_official_files` unchanged (0 errors, 92 +
  15 warnings). The rename plugin was regenerated (TES4 only changed):
  **2,221 bytes, SHA-256 `9C0B28A6E9C4CDF4C7878CF7DD745A022CF6E0AE943D2AD3DA6C8F044078EA27`**.
## In progress

- **M2.F7 (owner check pending).** `.mcp.json.example` and "Use from Claude Code" in
  `docs/mcp.md` are in; an agent cannot restart Claude Code, so the owner runs:
  1. `cargo build --release -p kit-cli`
  2. `claude mcp add --scope local kitcraft -- C:\Modding\kitcraft\target\release\kit.exe mcp`
     (or copy `.mcp.json.example` to `.mcp.json`, which is gitignored, and approve it).
  3. New Claude Code session: `/mcp` lists `kitcraft` connected, 8 tools.
  4. Ask it to call `mcp__kitcraft__coverage` on `<Data>\Constellation.esm`, then
     `mcp__kitcraft__execute` with `{command: "roundtrip", args: {plugin, verify: true}}`,
     then `mcp__kitcraft__inspect_record` on a form `coverage` names (`first_record`), then
     `mcp__kitcraft__query_records {type: "WEAP"}` and `mcp__kitcraft__record_schema
     {type: "WEAP"}`; each returns an envelope (`query_records` needs a load order: pass
     `--profile` to `kit mcp`, which now preloads it).
  5. Paste a transcript excerpt here. Headless alternative: `claude -p "..." --mcp-config
     .mcp.json --allowedTools "mcp__kitcraft__*"`.
- **Lane B round 3 (M3, `crates/schema`).** M3.O0 done: the owner staged the pinned files
  outside the repo at `C:\Modding\tools\xedit-ref\` (`wbDefinitionsSF1.pas`,
  `wbDefinitionsCommon.pas`, `PIN.txt`). M3.B0 done 2026-10-14: sizes 789,023 and 338,167
  bytes, blob hashes `ace733b52e259e559aed0f4c71edf9e3432fc62b` and
  `cfa80ba05c176d7571e19895e835f8df735a06cf`, SHA-256 `9f01921a...952346d5` and
  `e616b654...682b7d26`, all as in `docs/xedit-reference.md`; the loader already accepts
  both files (M2.B1). Typed so far (each checked on every corpus record of its type: typed
  members decoded, nothing left in `extra`, zero re-encode differences, and the full-corpus
  typed check of `corpus_full_roundtrip` clean):
  - **M3.B1 WEAP**: all 452 records; every subrecord xEdit lists is typed except VMAD (script
    data, unions) and MO4C (unknown upstream); the BFCB component data stays raw (a union
    decided by the component type). New shared definitions: `Model`, `Destructible`,
    `SoundReference`, `ObjectModProperty`, `Vec3`, `Bool8`; `BfcbBlock` and `ObteBlock`
    typed (component type, end markers, object template items).
  - **M3.B2 AMMO, PROJ**: all 154 AMMO and 299 PROJ records. AMMO: every member typed
    except VMAD; DATA's weight is `if_remaining` (xEdit makes it optional). PROJ (no
    inventory before; the corpus check is its inventory): every member typed except VMAD and
    the BFCB component data; PROD's two particle define strings have a u32 length prefix,
    typed as a count field plus raw bytes (the format has no u32-prefixed string); the
    unused 0-byte DATA is an `empty` marker.
  - **M3.B3 AMDL**: all 126 records (EDID + 65-byte ANAM, 17 fields typed; the last one
    `if_remaining` as xEdit makes it optional).
  - **M3.B4 OMOD**: all 3,330 records (DATA 34..511 bytes): DATA typed with its include
    list and its property entries (shared `ObjectModProperty`, 24 bytes each: value type,
    function type and property as integers, Value1/Value2 opaque because they are xEdit
    unions and Value1 can be a FormID; M3.B9 can type them once a sibling-field decider
    exists). Name and the attach parent slot list carry u32 prefixes, typed as explicit
    count fields. VMAD, the unknown XFLG after the model block (never reached in the corpus:
    the model block's XFLG takes it) and the BFCB component data stay opaque.
  - **M3.B5 COBJ, ZOOM**: all 3,518 COBJ and 141 ZOOM records. COBJ: every member typed
    except VMAD, the BFCB component data and the condition payloads (new shared group
    `Condition`: CTDA opaque, a struct of xEdit unions; CIS1/CIS2 typed); components use the
    new shared type `Component`. ZOOM: EDID + the 35-byte ZNAM, fully typed.
  - **M3.B7 LVLI, FLST, INNR** (no inventories before; the corpus check is their
    inventory): all 6,543 LVLI (22,701 entries), 1,415 FLST and 8 INNR records. LVLI: every
    member typed except VMAD (one official record has one), the BFCB component data,
    condition payloads and the COED owner data (an xEdit union); the entry count LLCT is a
    separate subrecord, stored as read (M3.B10 covers `count_from` arrays only, so an edit
    that adds an entry must update LLCT itself; noted in the definition). FLST: LNAM list,
    conditional entries (INAM, CITC, conditions), ANAM. INNR: rulesets with their names,
    keywords, XNAM properties (the target byte is an xEdit union by UNAM, kept as an
    integer) and indexes.
  - **M3.B8 lstrings**: all 105 lstring signatures of lane E's corpus table are schema
    `lstring` members (`corpus`; the xEdit line cited for 103, CHAL DESC corpus only).
    The 49 record types without a definition got lstring-only definitions: EDID plus a
    repeated `LocalizedStrings` group of optional lstring members, so every occurrence is
    typed wherever it stands and every other subrecord stays raw in place. That is less
    than the plan's "inventory definition" (corpus order and cardinality of every member):
    a full inventory for types such as NPC_, QUST, REFR, INFO is left to later rounds.
    Corpus check: every lstring occurrence of those 49 types is visited (counts equal), 0
    conflicts, 0 re-encode differences, including 6.7 million REFR.

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
| schema     | Round 3 (blocked, see Blockers): typed layouts from the pinned xEdit files (see below)                         | `crates/schema` (lane B)                     |

Lane B round 3 transcribes definitions from the pinned xEdit files (`xedit:` provenance,
`confidence = "reference"` until the corpus confirms) in this priority: **WEAP, AMMO, PROJ,
AMDL, OMOD, COBJ, ZOOM**; then **GMST, GLOB, KYWD** typed; then **LVLI, FLST, INNR**. Each
definition is checked against every corpus occurrence before it moves to
`confidence = "corpus"`; unions stay opaque.

## Blockers

- None for lane B (the pinned xEdit files are staged, M3.O0).

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
| M2.O4 | CK and MO2 observation sessions                             | open; blocks M2.D1 check, M2.D3 (c), the `hardcoded-official-slots` check, M2.E3 `ck-diff` part |
| M2.O5 | Third-party mod plugins as inventory evidence               | open; default **no**                   |
| M2.O6 | In-game check of the rename plugin                          | **ready**: plugin built, steps below   |

#### M2.O6: in-game check of the rename plugin (owner)

The plugin is `C:\Modding\kitcraft\out\OldSovietMarksmanRifle.esm` (2,221 bytes,
SHA-256 `9C0B28A6...4078EA27`, regenerated 2026-10-14 after lane E round 2 added `CNAM`
and `INCC` to its `TES4`; full hash and details in `docs/smoke-rename.md`). It
renames `Starfield.esm:0021BBCD` (WEAP `RussianHuntingRifle`, "Old Earth Hunting Rifle")
to "Old Soviet Marksman Rifle". In the Default profile the rifle's winner is
`Starfield.esm` (no mod overrides it, checked 2026-10-14 with `kit inspect --profile`), so
the patch's only master is `Starfield.esm`. Use a disposable save.

1. MO2: right-click the mod list > *Create empty mod* > `Old Soviet Marksman Rifle
   (kitcraft smoke test)`.
2. Copy the plugin into `C:\Modding\MO2-Starfield\mods\Old Soviet Marksman Rifle
   (kitcraft smoke test)\`.
3. Enable the mod and its plugin; put the plugin last in the load order (it must load
   after the rifle's winner; if a mod that overrides `0021BBCD` is added later, rebuild
   the patch from that load order and keep the patch after it).
4. Launch Starfield through MO2, load the disposable save, console:
   `player.additem 0021BBCD 1`.
5. Read the name in the inventory and the weapon's inspect view; record it here (pass or
   fail, with a screenshot if possible).

Expected: "Old Soviet Marksman Rifle". The shown name is the base FULL plus the
instance naming rules of INNR `dn_CommonGun` (the rifle's `INRD`), which prefix names by
keyword ("Titanium", "Elemental", ...): a modded instance may read "Elemental Old Soviet
Marksman Rifle", which is still a pass. The old name means the plugin is inactive or not
last, or an OMOD/INNR string supplies the name (record which); garbage such as `%@` means a
string ID was read as text (a `lstring.copied-id` miss). Disable the mod afterwards; it adds
no forms and no scripts.

Result: _pending (owner)_.

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
- ~~**Policy -> lane B.** Store `confidence` in the model, `corpus` on TES4/GRUP/RECORD,
  required on `uesp-skyrim:`, re-export the pin.~~ Done in M2.B1 (lane B branch):
  `Provenance` on every node, `confidence` required on every file, `XEDIT_REVISION`.
- **Policy -> lane C.** `coverage` / `cargo xtask parity` should show `confidence` next to
  the source kind once the model carries it; `confidence = "reference"` values should surface as
  a `limitations[]` entry in `inspect`/`query` results.
- ~~**Policy -> lane B (owner approval 2026-10-14).** Accept `wbDefinitionsCommon.pas` in
  `xedit:` citations.~~ Done in M2.B1 (loader, tests, README).
- ~~**Policy -> AGENTS.md / plan maintainer.**~~ Done (lane F round 3): AGENTS.md, ADR 0001
  Amendment 1 and `plan/execution-plan.md` (M2.O1, M2.O2, M2.B1 confidence names) updated.
  Was: AGENTS.md "Clean room" ("Other xEdit files are
  not covered yet") and owner decision 2 ("extending the reference beyond
  `wbDefinitionsSF1.pas`"), ADR 0001 Amendment 1 (only the SF1 file), and
  `plan/execution-plan.md` M2.O1/M2.O2 should reflect the approval (the common file at the
  same SHA; O1 and O2 done). Also: M2.B1 names confidence values
  `confirmed`/`corpus-consistent`/`hypothesis`, while ADR 0004 and the loader use
  `reference`/`corpus`/`ck-diff`/`verified`; the ADR wins.
- **Policy -> lane C (partly done, lane F round 3).** `cargo xtask parity` now has a
  "Definitions by confidence" table (from `kit schema --json`), and `kit schema` /
  `record_schema` states `reference`-confidence definitions as a limitation; `inspect` and
  `query` results do not yet.
- **Policy -> M2.** Install SF1Edit (matching the pin if possible, else record the build) for
  the differential comparison in ADR 0005 Amendment 2 §2a; never bundle it.
- **Lane B -> owner (default chosen).** Inventory definitions name members after their
  signature (`Wfir`, `Flld2`); renaming them once meanings are sourced (`ck-wiki`)
  is a breaking change for `kit query`. Default: accept the rename when it happens. Done
  for the universal subrecords in M2.B2: `Edid` is now `EditorId`, `Full` `Name`, `Desc`
  `Description`, `Ksiz`/`Kwda` the group `Keywords { Count, Forms }`; GMST `Data` is
  `Value`, GLOB `Fltv` is `Value`.
- **Lane B -> lane A (FYI, done in lane B's branch).** The M2.B2 rename broke one line of
  the bridge test (`gmst.edid` was opaque bytes); lane B changed it to
  `gmst.editor_id ... == b"fFoo"` in `crates/esm/src/bridge.rs` so the gate stays green.
  Generated API additions you may use: `Record::extra()`, the four visitors, and union
  enums (`GmstValue`) decoded through `put_by_prefix` (a missing EDID keeps DATA raw).
- **Lane B -> lane F / coverage owner (FYI, done in lane B's branch).** The engine coverage
  test now counts EDID, FULL and GMST DATA as `typed` (expectations updated in
  `crates/engine/src/handlers/coverage.rs`). For M2.F9: per-member confidence is
  `SubrecordDef::provenance.confidence` (also on groups, fields, records); a union member
  is `FieldType::Union`, not opaque.
- **Lane D -> lane F.** (1) `inspect`'s `plugin` is no longer required, so it is no longer
  positional: `kit inspect <form> --plugin <file>`; the tests that used the positional form
  (engine `cmdline.rs`, kit-cli `exit_codes.rs`, mcp `tests/stdio.rs`) were updated. (2)
  `query` is implemented: tests that used it as the `Planned` example now use `build`
  (engine `lib.rs`, kit-cli `main.rs` and `exit_codes.rs`, mcp `server.rs`); mcp
  `backend.rs`'s null-args test now expects `load`'s `failed`; kit-cli `tests/mcp.rs`
  loads `Starfield.esm` + `Constellation.esm` for `query_records`. (3) `PLANNED_IN_M2`
  entries `load`, `query`, `diff` are stale. (4) M2.F7 step 4: `query_records` answers
  now, and `kit mcp --profile` preloads through `load`. (5) New catalog entry `conflicts`
  (alias `esmscan`); the MCP tool set may want a `conflicts` shortcut. **Lane F round 3:**
  (3) pruned (`PLANNED_IN_M2` is gone); (4) in the M2.F7 steps; (5) not added: `execute`
  reaches it, and the tool list stays small (8 tools with `record_schema`).
- **Lane D -> owner (M2.O4).** In one session: (a) MO2's left pane for `StarUI Inventory`
  vs its Free Lanes patch (which mod MO2 shows providing `Interface\inventorymenu.swf`);
  (b) in game, `help` or a console click on a Constellation.esm object should show an
  `FE000xxx` ID and a full plugin's object `<slot>xxxxxx` (`kit inspect <form> --profile
  ...` prints the runtime ID it assumes; `inspect` now needs `runtime:<hex>` for a runtime
  ID); (c) whether `HG_ShipCargoHoldAtHome.esm`'s records (stored `02xxxxxx` with one
  master) exist in game; (d) where the game loads `ShatteredSpace`, `SFBGS004`, `SFBGS00D`,
  `SFBGS050`, `BlueprintShips-SFBGS050` and `SFBGS047` (KitCraft now assumes right after
  the `Starfield.ccc` plugins, `hardcoded-official-slots`): the runtime IDs of their objects
  in game would show their slots. Lead: the CK's own `EditorDataFilesLoaded.txt` in the game
  dir lists each loaded file with its index and ID form (`001 - (FE000XXX) -
  'KZ_Mantis.esm'`); a CK session with the full profile would record the CK's order.
- **Lane D -> lanes E, F (review round 2 API changes).** A deleted version is
  `RecordLoc.flags & kitcraft_doc::flags::RECORD_FLAG_DELETED != 0` (`inspect` and `query`
  show `deleted`), for lane E's refusal of deleted sources. `Version` has `basis`
  (`kitcraft_doc::Basis`: `Direct`, `FullStyleSelf`, `FullStyleSelfMasked`);
  `MasterMap::resolve_detailed` returns it instead of a bool. `ResolveError` gained
  `MasterNotLoaded`, `PluginNotLoaded` and `Null` (raw `00000000` never resolves);
  `FormRef::parse` refuses object ID 0. `inspect` reads bare hex only with `plugin`; a
  runtime ID is `runtime:<hex>` (front ends and examples that pass bare hex runtime IDs
  need the prefix). Index cache format 3: old entries rebuild once. For lane E's
  `verify_load_order`: the Default profile now has no late master, and the "active plugins
  with no file" limitation is now in `Loaded::limitations` (it was only in `load`'s own
  result before, so the `NO_FILE` check could not see it); whether `set`/`build` can drop
  the "unverified" refusal is lane E's call.
- **Lane D -> lane B.** 14 official GMST string settings hold string ID 0, which is in no
  table; `query` leaves them unknown. A source for what ID 0 means would close it.
  `where` member names are the schema's PascalCase names, matched case-insensitively.
- **Lane E -> lane F (tests touched, catalog closure).** `set`, `validate` and `build`
  are `Implemented`, so no catalog command is `Planned` any more. Tests that used `build`
  as the Planned example were changed in lane E's commit 714bfe6: engine
  `command_without_handler_is_not_implemented` (an engine without the handler),
  `apps/kit-cli` `exit_code_follows_status_and_passed` (a Planned spec built in the test),
  `apps/kit-cli/tests/exit_codes.rs` `failing_command_exits_2_with_clear_message` (missing
  spec / plugin), `crates/mcp/src/server.rs` (build with a missing spec is `failed`).
  `PLANNED_IN_M2` still lists `set`, `build`, `validate` (stale; prune at will).
  `validate`'s `plugin` is optional, so it is a flag (`kit validate --plugin X.esp`), as
  lane D did for `inspect`. **Done (lane F round 3):** `PLANNED_IN_M2` removed; the
  closure test now fails on any `Planned` command.
- **Lane E -> lane D (round 2).** (1) `set`/`build` refuse a load order with failed
  plugins, late or missing masters, or active plugins without a file ("load order
  unverified"); the last is found by the text of `load`'s limitation ("active plugins with
  no file"). Please expose the missing active plugins as a field of `Loaded` and say when
  the winning override is a safe source again, so lane E can drop the blanket refusal. The
  owner's Default profile is refused today: `stroudpremiumedition.esm` loads before
  `sfbgs00d.esm` and `SFBGS047.esm`. (2) `strings::Resolved` has only lossy `text`; lane G
  notes some official strings are not UTF-8. `set` refuses such a string (a U+FFFD in the
  text) rather than change it; a `bytes` field would let it copy them exactly.
- **Lane E -> owner (M2.O4).** The `ck-diff` half of M2.E3 is open: what the CK writes for
  a new non-localized plugin (master flag for `.esm`, `HEDR` next object ID, inline text
  encoding: we write UTF-8). Until observed it is a limitation on every `set`/`build`.
- **Lane F -> lanes D, E, G (session and catalog closure).** A handler that needs the
  load order is a `fn(&mut Session, &Value) -> Result<Outcome, EngineError>` added to
  `handlers::session_defaults()` (stateless handlers stay in `defaults()`). Keep your
  typed state in a slot (`Session::insert` / `get` / `get_mut`); `load` also sets
  `Session::set_load_order(Some(listing))`, which `kitcraft://load-order` serves as is.
  When you flip a command to `Implemented`, removing its `PLANNED_IN_M2` line in
  `crates/engine/src/lib.rs` is optional (a stale entry only prints); a command you will
  not finish in M2 becomes `Status::Deferred { reason, milestone: "M3" }`.
- **Lane G -> lane D (M2.G8 landed; for M2.D7).** `kitcraft-strings` API:
  - `Kind::{Strings, DlStrings, IlStrings}`, `Kind::from_path(&Path)`,
    `Kind::from_extension(&str)`, `kind.extension()`;
    `table_name("Starfield.esm", "en", Kind::Strings) == "starfield_en.strings"`.
  - `Table::parse(kind, &bytes) -> Result<Table, Error>`, `Table::read_file(&Path)`;
    `table.get(id: u32) -> Option<&Entry>`; `Entry { id, bytes, offset: Option<u64> }`
    with `entry.text() -> Cow<str>` (lossy UTF-8), `entry.is_utf8()`,
    `entry.stored_len(kind)`; `offset` is the absolute offset in the table file (evidence).
    `Error` variants carry byte offsets.
  - Tables in archives: `kitcraft_ba2::Archive::open(&path) -> (Archive, File)`,
    `archive.find("strings/starfield_en.strings") -> Option<usize>` (case-insensitive,
    either separator), `archive.extract_vec(&mut file, index, cap)` (round 2: `cap` in
    bytes, e.g. `256 << 20`; above it `Error::TooLarge`). Corpus: base-game tables are
    in `Starfield - Localization.ba2`; the DLC and Creation tables are in
    `<Plugin> - Localization.ba2` or `<Plugin> - Main.ba2`; names are lower-case
    `strings/<plugin>_<lang>.<ext>`, languages `de en es fr it ja pl ptbr zhhans`. In
    `starfield_en` the ID sets of the three kinds are disjoint (FULL is in `.strings`).
- **Lane G -> lanes D, E (string tables, review round 2).** Compare and copy strings by
  `Entry::bytes`, not by the lossy `text()`: a few entries of the English tables are not
  UTF-8 (cp1252-style bytes), and `text()` replaces those bytes, so a round trip through
  it changes them. A table may repeat an ID: `Table::get` and `strings.merge` both use the
  **first** occurrence (merge reports the others in `duplicates`, apart from cross-table
  `conflicts`); use `get`, not a map built by later-wins insertion.
- **Lane G -> lane F (FYI, review round 2).** `ba2.extract` gained `max_entry_bytes` and
  `ba2.pack` gained `overwrite` in the catalog; the expected parameter list in
  `crates/engine/src/params.rs` `required_first_in_required_order` was updated to match.
  Also seen once in a full `cargo xtask ci` run after merging lane D (passed on rerun and
  in isolation): `apps/kit-cli/tests/mcp.rs` `kit_mcp_on_an_official_file` read EOF from
  the `kit mcp` child (it exited mid-session); worth a look if it recurs. **Lane F round
  3:** not reproduced (20 runs, and 4-way parallel cold-cache stress); the spawned servers
  now use a private index cache under the test target dir instead of the user's (shared
  with every other worktree's gate across cache format changes), stderr is drained, and an
  EOF fails with the child's exit status and stderr.
- **Lane G -> lane D (FYI, done in the lane G merge).** `Archive::extract_vec` takes a
  byte cap now; `crates/doc/src/strings.rs` passes 256 MiB (the `strings.dump` cap). An
  entry above it is `kitcraft_ba2::Error::TooLarge`, reported like any other table error.
- **Lane G -> lane E (for M2.E8).** `kitcraft_pex::Pex::parse(&bytes)`; calls are
  `Instruction`s with `opcode_name(op)` `callmethod` / `callstatic` / `callparent`, the
  function name in `args[0]` (`Value::Identifier`, resolve with `pex.string(i)`), the base
  object in `args[1]`; `.pex` inside archives via `kitcraft_ba2` as above, or
  `execute("papyrus.disasm", {pex, archive?, format: "json"})`.
- **Lane G -> lane F.** (1) Tests that used `ba2.list` as their example of a `Planned`
  command now use `query` (engine `lib.rs`, `apps/kit-cli` `main.rs` and
  `tests/exit_codes.rs`, `crates/mcp/src/server.rs`); they break again when `query`
  lands, so a spec built in the test may be better. (2) `papyrus.disasm` puts the
  multi-line disassembly in `data.summary` so `kit` prints `pexdis.py`'s output; if `kit`
  should prefer a `data.text` member instead, say so. (3) `PLANNED_IN_M2` still lists the
  seven lane G commands (now `Implemented`; prune at will). (4) The status paragraph at the
  top of `docs/mcp.md` still lists only `roundtrip`, `coverage` and `inspect`. **Lane F
  round 3:** (2) `data.summary` stays (`kit schema` does the same); (3) and (4) done.
- ~~**Lane C -> lane B.** Mark a generated region for `BUILTIN_DEFINITIONS`.~~ Done
  (lane B, M2.B2 commit): the entries sit between
  `    // BEGIN generated by cargo xtask codegen: BUILTIN_DEFINITIONS` and
  `    // END generated by cargo xtask codegen: BUILTIN_DEFINITIONS` in
  `crates/schema/src/lib.rs`, one rustfmt-formatted entry per file in file-name order
  (top level, then `common/`). xtask (lane F) may rewrite exactly those lines;
  `tests/defs.rs` checks the markers and stays the guard until it does.
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
- ~~**Lane A -> lane B.** Please generate an accessor for the raw subrecords a typed record
  kept: `generated::records::Record::extra(&self) -> &[(usize, OwnedSub)]`.~~ Done (the
  accessor exists since M2.B2). **Lane B -> lane A (FYI, done in lane B's M3.B1 commit):**
  `esm::bridge::extra_of` now returns `typed.extra()` for every record type instead of
  matching five variants, so the M2.A3 typed check (`corpus_full_roundtrip`) sees payloads
  that do not decode in every typed record type; a one-function change.
- **Lane B -> lanes D, E, F (FYI, done in lane B's M3.B1 commit).** Typing WEAP changed five
  engine test expectations, updated in the same commit: `edit::fields` (WEAP WABB is a
  schema lstring now, no table fallback), `edit::patch_tests` (the opaque subrecord in the
  remap refusal test is VMAD, inserted after EDID, instead of WDMG), `handlers::coverage`
  (MODL is typed; counts), `handlers::schema` (EDID cites `xedit:`; VMAD is the opaque
  member checked), `handlers::set_tests` (the `hex:` error now comes from the WDMG struct).
  Member names changed in the shared blocks: `BfcbBlock.Bfcb`/`BfcbBody`/`Bfce` are
  `ComponentType`/`ComponentData`/`EndMarker`; `ObteBlock.Obte`/`ObtsEntry`/`Obts`/`Stop`
  are `CombinationCount`/`Combination`/`TemplateItem`/`EndMarker`; WEAP's placeholder names
  (`Obnd`, `Wfir`, `Flld2`, ...) are replaced by the xEdit-backed layout (`kit schema
  --type WEAP --deep`).
- ~~**Lane E -> lane B (round 2).** `crates/engine/src/edit/lstrings.rs` is a corpus-built
  stand-in for lstring typing the schema lacks (105 signatures, e.g. ACTI/FLOR/FURN/NPC_
  `ATTX`, NPC_ `SHRT`, INFO `NAM1`, MESG `ITXT`, QUST `CNAM`, WEAP `WABB`). As round 3
  transcribes definitions from the pinned xEdit file, please type these as `lstring`.~~
  Done in lane B's M3.B8 commit: all 105 are schema `lstring` members (`corpus`, citing
  `lstring_table_matches_the_corpus`, plus the xEdit line for 103 of them; CHAL `DESC` is
  not in xEdit's CHAL definition, the corpus wins). `crates/schema/tests/defs.rs`
  `lstring_members_cover_the_corpus_table` keeps a copy of the 105 pairs. **Lane B -> lane E
  (M3.E4):** `layout_for` no longer reaches the table fallback for any of them; tests
  updated in the same commit (`edit::fields` layouts, `edit::patch_tests` untyped lstrings:
  ACTI `ATTX` now comes from the schema, no table limitation; `handlers::schema` uses ARMO
  as the undefined type). **Lane B -> lane A (FYI):** CELL and REFR now have lstring-only
  definitions, so `bridge` and `driver` tests use NAVM as the undefined type and count CELL
  and the three REFR as typed records.
